import { describe, expect, it } from 'vitest';
import { measureGpu } from './gpu-bench.ts';
import { NullRenderer } from './null-renderer.ts';

/** A renderer whose frames cost a set amount of fake time, waited out in `finish`. */
class CostedRenderer extends NullRenderer {
  pending = 0;
  readonly drawn: number[] = [];
  readonly clock: { now: number };
  readonly costMsPerPatch: number;
  constructor(clock: { now: number }, costMsPerPatch: number) {
    super();
    this.clock = clock;
    this.costMsPerPatch = costMsPerPatch;
  }
  override render(frame: Parameters<NullRenderer['render']>[0]): void {
    super.render(frame);
    this.drawn.push(frame.patchInstances);
    this.pending += frame.patchInstances * this.costMsPerPatch;
  }
  override finish(): Promise<void> {
    this.clock.now += this.pending;
    this.pending = 0;
    return Promise.resolve();
  }
}

describe('measureGpu', () => {
  it('times each level and finds the cost per frame, whatever the number of frames', async () => {
    const clock = { now: 0 };
    const r = new CostedRenderer(clock, 0.25);
    const result = await measureGpu({
      renderer: r,
      width: 1920,
      height: 1080,
      levels: [1, 16, 64],
      budgetMs: 400,
      nowMs: () => clock.now,
    });
    expect(result.levels.map((l) => l.patches)).toEqual([1, 16, 64]);
    expect(result.levels.map((l) => l.msPerFrame)).toEqual([0.25, 4, 16]);
    expect(result).toMatchObject({ backend: 'null', width: 1920, height: 1080 });
    expect([r.width, r.height]).toEqual([1920, 1080]);
    expect(r.terrain).toBeDefined();
  });

  it('draws the requested number of patches in every timed frame', async () => {
    const clock = { now: 0 };
    const r = new CostedRenderer(clock, 1);
    await measureGpu({
      renderer: r,
      width: 160,
      height: 90,
      levels: [4],
      budgetMs: 100,
      nowMs: () => clock.now,
    });
    expect(new Set(r.drawn)).toEqual(new Set([4]));
  });

  it('keeps to the budget: fewer frames when each is expensive, never fewer than three', async () => {
    const clock = { now: 0 };
    const slow = new CostedRenderer(clock, 500);
    const result = await measureGpu({
      renderer: slow,
      width: 160,
      height: 90,
      levels: [1],
      budgetMs: 400,
      nowMs: () => clock.now,
    });
    expect(result.levels[0]?.frames).toBe(3);

    const fastClock = { now: 0 };
    const fast = new CostedRenderer(fastClock, 0.001);
    const quick = await measureGpu({
      renderer: fast,
      width: 160,
      height: 90,
      levels: [1],
      budgetMs: 400,
      nowMs: () => fastClock.now,
    });
    expect(quick.levels[0]?.frames).toBe(240);
  });

  it('reports each level as it finishes', async () => {
    const clock = { now: 0 };
    const seen: number[] = [];
    await measureGpu({
      renderer: new CostedRenderer(clock, 1),
      width: 160,
      height: 90,
      levels: [1, 2],
      budgetMs: 50,
      nowMs: () => clock.now,
      onLevel: (l) => seen.push(l.patches),
    });
    expect(seen).toEqual([1, 2]);
  });
});
