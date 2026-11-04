/**
 * The model rig's garbage, measured: updating the part matrices every frame makes no objects or
 * arrays (the method is the input pump's garbage test; see `test-support.ts`).
 */
import { describe, expect, it } from 'vitest';
import { FIXTURE_MODEL } from './model-fixture.ts';
import { ModelRig, newModelUniform, type ModelState } from './model-rig.ts';
import { NORTH } from './snapshot-fixture.ts';
import { bytesPerCall } from './test-support.ts';

describe('ModelRig: garbage', () => {
  it('updates the part matrices without making any objects', async () => {
    const rig = new ModelRig(FIXTURE_MODEL);
    const out = newModelUniform();
    const state: ModelState = {
      ownshipPosBf: new Float64Array([100, -5000, 800]),
      ownshipQuat: Float32Array.from(NORTH),
      elevatorRad: 0.2,
      aileronRad: -0.1,
      rudderRad: 0.05,
      flapRad: 0.3,
      airbrake: 0.5,
      gearPos: 0.4,
      abLevel: 0.7,
    };
    const eye = new Float64Array([98, -5043, 806]);
    const bytes = await bytesPerCall(() => {
      rig.update(out, state, eye);
    });
    // Nothing is made per frame; a stray array or object would be tens of bytes at least.
    expect(bytes).toBeLessThan(64);
  });
});
