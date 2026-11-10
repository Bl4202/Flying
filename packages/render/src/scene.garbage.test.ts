/**
 * The scene builder's garbage, measured. `build` runs every frame: it makes no objects, arrays or
 * closures, only writes into the buffers it owns. Its own file because the measurement is of
 * optimised code (see the input pump's garbage test for the method).
 */
import { describe, expect, it } from 'vitest';
import { COMBAT_FLAG, EXPLOSION_KIND, MISSILE_STATE } from '@flying/sim-bridge/layout';
import { SnapshotInterpolator } from './interp.ts';
import { FIXTURE_MODEL } from './model-fixture.ts';
import { ModelRig } from './model-rig.ts';
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

  it('builds a duel frame (the bandit, missiles, tracers and explosions) without making any', async () => {
    const { terrain, scene } = rig();
    const interp = new SnapshotInterpolator();
    const snap = (k: number) =>
      makeSnapshot({
        tick: k,
        publishMs: 1000 + k * 16,
        ownship: { pos: [100, -6000 + k, 1500], quat: NORTH },
        combat: {
          flags: COMBAT_FLAG.ACTIVE | COMBAT_FLAG.PIPPER,
          pipper: [100, -5000 + k, 1500],
          bandit: { pos: [140, -5200 + k, 1520], quat: NORTH, vel: [0, 200, 0] },
          missileSlots: [
            { pos: [100, -5500 + k, 1500], vel: [0, 600, 0], state: MISSILE_STATE.BOOST },
          ],
          tracers: Array.from({ length: 24 }, (_, i) => ({
            pos: [100 + i, -5800 + i * 10 + k, 1500] as [number, number, number],
            vel: [0, 1000, 10] as [number, number, number],
            id: i,
          })),
          explosions: [{ pos: [200, -5000, 1500], age: 0.3, kind: EXPLOSION_KIND.AIRCRAFT }],
        },
      });
    interp.push(snap(1));
    interp.push(snap(2));
    const state = interp.sample(1000 + 2 * 16 + 8);
    if (!state) throw new Error('no state');
    const bandit = new ModelRig(FIXTURE_MODEL);
    let t = 0;
    const frame = scene.build(state, {
      aspect: 16 / 9,
      zRange: '0to1',
      timeS: t,
      terrain,
      banditModel: bandit,
    });
    expect(frame.bandit).toBe(true);
    expect(frame.instanceCount).toBe(1 + 24 + 4);
    const bytes = await bytesPerCall(() => {
      t += 1;
      scene.build(state, {
        aspect: 16 / 9,
        zRange: '0to1',
        timeS: t,
        terrain,
        banditModel: bandit,
      });
    });
    // Measured on the development machine: about 160 B, the same boxing as the plain frame.
    expect(bytes).toBeLessThan(320);
    // And the interpolation that feeds it: about 30 B (the blend factor and two times boxed).
    let n = 0;
    const sampling = await bytesPerCall(() => {
      n += 1;
      interp.sample(1000 + 2 * 16 + (n % 16));
    });
    expect(sampling).toBeLessThan(64);
    // Two measurements of a heavier frame take longer than vitest's five-second default.
  }, 60_000);
});
