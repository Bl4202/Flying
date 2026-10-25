import { describe, expect, it } from 'vitest';
import { quatRotate } from '@flying/sim-bridge/frame-math';
import { NullRenderer } from './null-renderer.ts';
import { runRendererContract } from './renderer-contract.ts';
import { LOOKING_DOWN, LOOKING_UP } from './snapshot-fixture.ts';
import { airfieldShifted, SceneBuilder } from './scene.ts';
import { SnapshotInterpolator } from './interp.ts';
import { makeSnapshot } from './snapshot-fixture.ts';
import { buildTerrainMesh } from './terrain-mesh.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

describe('the renderer contract', () => {
  it('is satisfied by the null renderer for everything that does not look at pixels', async () => {
    const checks = await runRendererContract(() => Promise.resolve(new NullRenderer()), {
      draws: false,
    });
    const failed = checks.filter((c) => !c.ok);
    expect(failed).toEqual([]);
    expect(checks.map((c) => c.name)).toEqual([
      'reads back width x height x 4 bytes',
      'resize changes the size of the readback',
      'finish resolves, also with nothing drawn yet and after drawing',
      'onLost accepts listeners',
      'dispose is idempotent and drawing afterwards does nothing',
    ]);
  });

  it('reports a failure instead of throwing when a backend misbehaves', async () => {
    class Broken extends NullRenderer {
      override debugReadPixels(): Promise<Uint8Array> {
        return Promise.resolve(new Uint8Array(3));
      }
    }
    const checks = await runRendererContract(() => Promise.resolve(new Broken()), { draws: false });
    expect(checks.filter((c) => !c.ok).map((c) => c.name)).toContain(
      'reads back width x height x 4 bytes',
    );
  });

  it('asks the drawing questions of a drawing backend', async () => {
    const checks = await runRendererContract(() => Promise.resolve(new NullRenderer()), {
      draws: true,
    });
    const names = checks.map((c) => c.name);
    expect(names).toContain('the same scene moved 400 km draws the same picture (relative-to-eye)');
    expect(names).toContain('looking at the sky shows only the sky colour');
    expect(names).toContain('many patch instances draw without breaking the frame');
    expect(names).toContain('a pilot at full g stress sees a dark picture');
    expect(names).toContain('a pilot in red-out sees a red picture');
    expect(names).toContain('the effect strength setting switches the effects off');
    expect(names).toContain('over the runway the picture shows asphalt and paint');
    expect(names).toContain('mouse-aim draws the aim and nose cues');
    // The null renderer cannot draw, so it must fail the pictures it is asked about.
    expect(checks.find((c) => c.name.startsWith('looking at the world'))?.ok).toBe(false);
  });
});

describe('the scene the contract draws', () => {
  it('LOOKING_DOWN points the nose at the ground with the right wing east', () => {
    const nose = quatRotate(LOOKING_DOWN, [1, 0, 0]);
    const right = quatRotate(LOOKING_DOWN, [0, 1, 0]);
    const down = quatRotate(LOOKING_DOWN, [0, 0, 1]);
    expect(nose.map((x) => Math.round(x) + 0)).toEqual([0, 0, -1]);
    expect(right.map((x) => Math.round(x) + 0)).toEqual([1, 0, 0]);
    expect(down.map((x) => Math.round(x) + 0)).toEqual([0, -1, 0]); // the body's down is south
  });

  it('LOOKING_UP points the nose at the sky with the right wing east', () => {
    const nose = quatRotate(LOOKING_UP, [1, 0, 0]);
    const right = quatRotate(LOOKING_UP, [0, 1, 0]);
    expect(nose.map((x) => Math.round(x) + 0)).toEqual([0, 0, 1]);
    expect(right.map((x) => Math.round(x) + 0)).toEqual([1, 0, 0]);
  });

  it('moving everything 400 km changes no number the GPU receives, beyond f32 rounding', () => {
    const build = (shift: number) => {
      const interp = new SnapshotInterpolator();
      interp.push(
        makeSnapshot({
          publishMs: 1000,
          ownship: { pos: [shift, shift - 8000, 1500] },
          bodies: [{ pos: [shift + 400, shift - 3800, 1450] }],
        }),
      );
      const mesh = buildTerrainMesh(makeTestPatch({ centreEast: shift, centreNorth: shift }));
      const frame = new SceneBuilder().build(
        interp.sample(2000) ??
          (() => {
            throw new Error('no state');
          })(),
        {
          aspect: 16 / 9,
          zRange: '0to1',
          timeS: 0,
          patchOriginBf: mesh.originBf,
          spacingM: mesh.spacingM,
          // The airfield moves with the rest of the scene.
          airfield: airfieldShifted(shift, shift),
        },
      );
      return {
        uniform: Float32Array.from(frame.uniform),
        instances: Float32Array.from(frame.instances.subarray(0, 8)),
      };
    };
    const [a, b] = [build(0), build(400_000)];
    for (let i = 0; i < a.uniform.length; i++) {
      expect(
        Math.abs((a.uniform[i] ?? 0) - (b.uniform[i] ?? 0)),
        `uniform[${String(i)}]`,
      ).toBeLessThan(1e-3);
    }
    for (let i = 0; i < 8; i++) {
      expect(
        Math.abs((a.instances[i] ?? 0) - (b.instances[i] ?? 0)),
        `instance[${String(i)}]`,
      ).toBeLessThan(1e-3);
    }
  });
});
