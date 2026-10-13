import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, sampleAfter, simRate } from './support.ts';

const URL = `${HEADERS_BASE}?debug=1&flags=stall`;

/** The loop is running and has been for long enough that its first-frame costs are behind it. */
async function settled(page: Parameters<typeof openGame>[0]) {
  await openGame(page, URL);
  await page.waitForTimeout(1000);
  await page.evaluate(() => {
    window.__flying?.resetRun();
  });
  return sampleAfter(page, 500);
}

test('a second of busy main thread leaves the sim at 60 Hz and the picture moving', async ({
  page,
}) => {
  const before = await settled(page);
  await page.evaluate(() => {
    const end = performance.now() + 1000;
    while (performance.now() < end) {
      // Deliberately busy: the main thread does nothing else for a second.
    }
  });
  const after = await sampleAfter(page, 1500);

  const rate = simRate(before, after);
  expect(rate).toBeGreaterThan(59);
  expect(rate).toBeLessThan(61);
  expect(after.sim.droppedMs - before.sim.droppedMs).toBe(0);
  // The renderer is in its own worker: no frame was held up anywhere near the length of the stall.
  expect(after.render.frameRunMaxMs).toBeLessThan(100);
  expect(after.render.frames - before.render.frames).toBeGreaterThan(60);
});

test('a 300 ms render stall does not touch the sim', async ({ page }) => {
  const before = await settled(page);
  await page.evaluate(() => {
    window.__flying?.stallRender(300);
  });
  const after = await sampleAfter(page, 1500);

  // The stall happened: some frame interval is as long as it.
  expect(after.render.frameRunMaxMs).toBeGreaterThan(250);
  // And the sim carried on regardless.
  const rate = simRate(before, after);
  expect(rate).toBeGreaterThan(59);
  expect(rate).toBeLessThan(61);
  expect(after.sim.droppedMs - before.sim.droppedMs).toBe(0);
  expect(after.sim.stepRunMaxMs).toBeLessThan(20);
});

test('a 300 ms sim stall: five catch-up ticks, the rest dropped, and the frames go on', async ({
  page,
}) => {
  const before = await settled(page);
  await page.evaluate(() => {
    window.__flying?.stallSim(300);
  });
  const after = await sampleAfter(page, 1500);

  // The sim was away for 300 ms (18 ticks owed); at most 5 are run, so about 13 ticks are given up.
  const dropped = after.sim.droppedMs - before.sim.droppedMs;
  expect(dropped).toBeGreaterThan(150);
  expect(dropped).toBeLessThan(260);
  expect(after.sim.droppedEvents - before.sim.droppedEvents).toBe(1);
  // Ticks lost over the window match the dropped time: nothing else went missing.
  const window_ = (after.sim.atMs - before.sim.atMs) / 1000;
  const lost = window_ * 60 - (after.sim.tick - before.sim.tick);
  expect(lost).toBeGreaterThan(8);
  expect(lost).toBeLessThan(20);
  // The renderer did not wait for it: its longest frame interval is nowhere near 300 ms.
  expect(after.render.frameRunMaxMs).toBeLessThan(100);
});

test('pause freezes the tick, the frames continue, and resuming does not burst', async ({
  page,
}) => {
  await settled(page);
  await byId(page, 'ov-pause').click();
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'paused');
  const paused1 = await sampleAfter(page, 600);
  const paused2 = await sampleAfter(page, 1200);
  expect(paused2.sim.tick).toBe(paused1.sim.tick);
  expect(paused2.render.frames - paused1.render.frames).toBeGreaterThan(30);

  await byId(page, 'ov-pause').click();
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'running');
  const resumed1 = await sampleAfter(page, 500);
  const resumed2 = await sampleAfter(page, 2000);
  // No catch-up for the time spent paused: the rate is the normal one, and nothing was dropped.
  const rate = simRate(resumed1, resumed2);
  expect(rate).toBeGreaterThan(59);
  expect(rate).toBeLessThan(61);
  expect(resumed2.sim.droppedMs - paused2.sim.droppedMs).toBe(0);
  // Nor more ticks than the wall time allows: the time spent paused was not made up.
  const elapsedS = (resumed2.sim.atMs - paused2.sim.atMs) / 1000;
  expect(resumed2.sim.tick - paused2.sim.tick).toBeLessThanOrEqual(elapsedS * 60 + 2);
});
