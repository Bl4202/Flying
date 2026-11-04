/**
 * The camera rig's garbage, measured: a step makes no objects or arrays, in either camera (the
 * method is the input pump's garbage test; see `test-support.ts`).
 */
import { newViewState } from '@flying/sim-bridge/view-channel';
import { describe, expect, it } from 'vitest';
import { CameraRig, type RigState } from './camera-rig.ts';
import { NORTH } from './snapshot-fixture.ts';
import { TerrainSet } from './terrain-levels.ts';
import { bytesPerCall } from './test-support.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

const state: RigState = {
  ownshipPosBf: new Float64Array([1000, -3000, 120]),
  ownshipQuat: Float32Array.from(NORTH),
  ownshipVelBf: new Float32Array([5, 200, 0]),
  ownshipSpeed: 200,
  gLagged: 3,
  alpha: 0.31,
  alphaWarn: 0.3,
  groundSpeed: 0,
  abLevel: 0.5,
  onGround: false,
};

describe('CameraRig: garbage', () => {
  for (const camera of ['cockpit', 'chase'] as const) {
    it(`steps the ${camera} camera over terrain without making any`, async () => {
      // The real terrain lookup, on the drawn triangles: the aircraft is over level 0 (250 m posts).
      const terrain = new TerrainSet();
      terrain.set(0, makeTestPatch({ centreEast: 1000, centreNorth: -3000, baseM: 20 }));
      const rig = new CameraRig(
        {
          eyeBodyM: [3.6, 0, -0.85],
          headLimits: { yawDeg: 135, pitchUpDeg: 60, pitchDownDeg: 60 },
        },
        terrain,
      );
      // The view holds still: a harness that stores a fresh number into an object every call would
      // be measuring itself.
      const view = { ...newViewState(), camera, lookYawRate: 0.2, lookPitchRate: -0.1 };
      const bytes = await bytesPerCall(() => {
        rig.step(state, view, 1 / 60);
      });
      // V8 boxes a fractional number that crosses a call it does not inline (16 bytes, gone at the
      // next young-generation collection). The cockpit step makes a handful of those (the terrain
      // lookup takes two numbers and returns one); the chase step makes them for every terrain
      // sample along its arm. Measured on the development machine: 160 B and 631 B a frame; an array
      // or object made per step would show as a multiple of that, and a leak as growth over time.
      expect(bytes).toBeLessThan(camera === 'cockpit' ? 320 : 2048);
    });
  }
});
