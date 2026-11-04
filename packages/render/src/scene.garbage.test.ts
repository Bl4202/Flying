/**
 * The scene builder's garbage, measured. `build` runs every frame: it makes no objects, arrays or
 * closures, only writes into the buffers it owns. Its own file because the measurement is of
 * optimised code (see the input pump's garbage test for the method).
 */
import { describe, expect, it } from 'vitest';
import { SnapshotInterpolator } from './interp.ts';
import { SceneBuilder } from './scene.ts';
import { makeSnapshot, NORTH } from './snapshot-fixture.ts';
import { TerrainSet } from './terrain-levels.ts';
import { bytesPerCall } from './test-support.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

describe('SceneBuilder: garbage', () => {
  function rig() {
    const interp = new SnapshotInterpolator();
    interp.push(
      makeSnapshot({
        publishMs: 1000,
        ownship: { pos: [100, -6000, 1500], quat: NORTH, gStress: 0.5 },
        bodies: Array.from({ length: 8 }, (_, i) => ({
          pos: [i * 50, -5000, 1500] as [number, number, number],
        })),
      }),
    );
    const state = interp.sample(2000);
    if (!state) throw new Error('no state');
    const terrain = new TerrainSet();
    for (const [level, spacingM] of [
      [3, 16_000],
      [2, 4_000],
      [1, 1_000],
      [0, 250],
    ] as const) {
      terrain.set(level, makeTestPatch({ spacingM }));
    }
    return { state, terrain, scene: new SceneBuilder() };
  }

  it('measures garbage, so the check below means something', async () => {
    let sink: unknown;
    const array = await bytesPerCall((i) => {
      sink = [i, i + 1];
    });
    expect(sink).toBeDefined();
    expect(array).toBeGreaterThan(8);
  });

  it('builds a frame with the sky, four terrain levels and eight markers without making any', async () => {
    const { state, terrain, scene } = rig();
    let t = 0;
    const bytes = await bytesPerCall(() => {
      t += 1;
      scene.build(state, { aspect: 16 / 9, zRange: '0to1', timeS: t, terrain });
    });
    // No object, array or closure is made per frame. What is left is V8 boxing a fractional number
    // that crosses a call it does not inline (16 bytes each, gone at the next young-generation
    // collection): the field of view, the aspect and the near plane into the projection, the
    // globe's floor into the eye geometry. Measured on the development machine: about 136 B; a
    // stray array or object would add tens of bytes per body and per call.
    expect(bytes).toBeLessThan(320);
  });
});
