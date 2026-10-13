/**
 * The loop for real: the built WASM sim, the monotonic clock, `Atomics.wait` sleeping and the
 * shared channels, for two seconds, the way the Sim Worker runs them. Same guard as the sim-handle
 * test: skipped when nothing is built, a failure under REQUIRE_WASM=1.
 */
import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { monotonicMs, sharedNowMs } from '@flying/platform-web';
import {
  ABI,
  CORE_STATE,
  encodeInputFrame,
  INPUT_FRAME,
  INPUT_SCRIPT,
  instantiateSim,
  makeInputChannel,
  makeSnapshotChannel,
  neutralFrame,
  openInputSink,
  openInputSource,
  openSnapshotSink,
  openSnapshotSource,
  SNAPSHOT,
  SnapshotReader,
} from '@flying/sim-bridge';
import { FixedStepClock } from './clock.ts';
import { MAX_CATCH_UP_TICKS, SIM_DT_MS } from './constants.ts';
import { SimLoop } from './sim-loop.ts';
import { AtomicsSleeper, createWakeWord } from './sleeper.ts';

const root = new URL('../../../', import.meta.url);
const built =
  existsSync(new URL('wasm-out/wasm-manifest.json', root)) &&
  existsSync(new URL('content-out/core.cpack', root));
if (!built && process.env['REQUIRE_WASM'] === '1') {
  throw new Error('REQUIRE_WASM=1 but wasm-out/ or content-out/ is missing');
}

describe.skipIf(!built)('the fixed-rate loop on the real sim', () => {
  it('holds 60 Hz for two seconds, publishes every tick, and carries input through', async () => {
    const manifest = JSON.parse(
      readFileSync(new URL('wasm-out/wasm-manifest.json', root), 'utf8'),
    ) as { threads_initial_pages: number };
    const sim = await instantiateSim(
      readFileSync(new URL('wasm-out/sim.single.wasm', root)),
      'single',
      manifest.threads_initial_pages,
    );
    sim.init(0x1f1e_5eed_0000_0001n);
    sim.loadContent(new Uint8Array(readFileSync(new URL('content-out/core.cpack', root))));
    sim.setInputScript(INPUT_SCRIPT.NONE);
    sim.start();

    const snap = makeSnapshotChannel('shared');
    const inp = makeInputChannel('shared');
    const snapshots = openSnapshotSink(snap.sink);
    const source = openSnapshotSource(snap.source);
    const sink = openInputSink(inp.sink);
    const inputs = openInputSource(inp.source);

    const clock = new FixedStepClock({ dtMs: SIM_DT_MS, maxTicksPerIteration: MAX_CATCH_UP_TICKS });
    const loop = new SimLoop({
      sim,
      inputs,
      snapshots,
      clock,
      nowMs: monotonicMs,
      sharedNowMs,
      sleeper: 'atomics',
      profiling: false,
    });
    const sleeper = new AtomicsSleeper(createWakeWord());

    // Warm up so JIT and caches do not count, then measure.
    loop.start();
    const warmEnd = monotonicMs() + 300;
    while (monotonicMs() < warmEnd) {
      loop.iterate();
      sleeper.sleep(clock.nextDue() - monotonicMs());
    }
    loop.resetRun();
    const tick0 = Number(sim.tick());
    const t0 = monotonicMs();
    const frame = new Uint8Array(INPUT_FRAME.SIZE);
    let seq = 1;
    while (monotonicMs() - t0 < 2000) {
      // The "main thread": a fresh control sample every ~8 ms.
      const f = neutralFrame(seq++);
      f.tMs = sharedNowMs();
      f.axes[0] = 0.5;
      encodeInputFrame(f, frame);
      sink.send(frame);
      loop.iterate();
      sleeper.sleep(Math.min(8, clock.nextDue() - monotonicMs()));
    }
    const ticks = Number(sim.tick()) - tick0;
    const seconds = (monotonicMs() - t0) / 1000;
    const stats = loop.report();

    // 60 Hz to within the width of one wake-up either side, and no time given up.
    expect(ticks / seconds).toBeGreaterThan(59);
    expect(ticks / seconds).toBeLessThan(61);
    expect(stats.droppedEvents).toBe(0);
    expect(stats.stepRun.count).toBeGreaterThanOrEqual(ticks - 1);
    expect(stats.stepRun.p99).toBeLessThan(7);
    expect(stats.inputLatencyRun.count).toBeGreaterThan(50);
    expect(stats.inputLatencyRun.p99).toBeLessThan(40);
    expect(stats.inputsDropped).toBe(0);
    expect(stats.coreState).toBe(CORE_STATE.RUNNING);

    // The newest published snapshot is the newest tick and carries our publish stamp and input.
    const out = new Uint8Array(SNAPSHOT.SIZE);
    expect(source.read(out)).toBeGreaterThan(0);
    const r = new SnapshotReader(out);
    expect(r.valid()).toBe(true);
    expect(r.tick()).toBe(Number(sim.tick()));
    expect(Math.abs(r.publishTimeMs() - sharedNowMs())).toBeLessThan(100);
    expect(r.lastInputSeq()).toBeGreaterThan(100);
    expect(ABI.INPUT_BATCH).toBeGreaterThan(0);
  }, 20_000);
});
