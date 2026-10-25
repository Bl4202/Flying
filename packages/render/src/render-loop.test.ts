import { describe, expect, it } from 'vitest';
import { CONTENDED, type SnapshotSource } from '@flying/sim-bridge';
import { SnapshotInterpolator } from './interp.ts';
import { NullRenderer } from './null-renderer.ts';
import { RenderLoop } from './render-loop.ts';
import { SceneBuilder } from './scene.ts';
import { makeSnapshot } from './snapshot-fixture.ts';
import { buildTerrainMesh } from './terrain-mesh.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

function rig() {
  let mono = 0;
  let shared = 10_000;
  let publication = 0;
  let bytes: Uint8Array | undefined;
  const source: SnapshotSource = {
    read: (out) => {
      if (publication === CONTENDED) return CONTENDED;
      if (!bytes) return 0;
      out.set(bytes);
      return publication;
    },
    close: () => undefined,
  };
  const interp = new SnapshotInterpolator();
  const renderer = new NullRenderer();
  const loop = new RenderLoop(
    { source, interp, scene: new SceneBuilder(), sharedNowMs: () => shared, monoMs: () => mono },
    renderer,
  );
  return {
    loop,
    renderer,
    interp,
    advance: (ms: number) => {
      mono += ms;
      shared += ms;
    },
    publish: (n: number, extra: Parameters<typeof makeSnapshot>[0] = {}) => {
      publication = n;
      bytes = makeSnapshot({ tick: n, publishMs: shared - 2, inputMs: shared - 6, ...extra });
    },
    contended: () => {
      publication = CONTENDED;
    },
  };
}

describe('RenderLoop', () => {
  it('has nothing to draw until a snapshot arrives', () => {
    const t = rig();
    expect(t.loop.frame()).toBe(false);
    expect(t.renderer.frames).toBe(0);
    t.publish(1);
    expect(t.loop.frame()).toBe(true);
    expect(t.renderer.frames).toBe(1);
  });

  it('takes each publication in once, however many frames are drawn between', () => {
    const t = rig();
    t.publish(1);
    for (let i = 0; i < 5; i++) {
      t.advance(8);
      t.loop.frame();
    }
    expect(t.interp.accepted).toBe(1);
    t.publish(2);
    t.advance(8);
    t.loop.frame();
    expect(t.interp.accepted).toBe(2);
  });

  it('counts contended reads and keeps drawing what it has', () => {
    const t = rig();
    t.publish(1);
    t.loop.frame();
    t.contended();
    t.advance(8);
    expect(t.loop.frame()).toBe(true);
    expect(t.loop.contendedReads).toBe(1);
  });

  it("builds the frame for the renderer's depth range, with the terrain origin when there is one", () => {
    const t = rig();
    t.loop.resize(320, 180);
    t.publish(1);
    t.loop.frame();
    expect(t.renderer.lastFrame?.uniform[24]).toBe(0); // no patch yet: no offset
    t.loop.setTerrain(buildTerrainMesh(makeTestPatch({ centreEast: 0, centreNorth: 0 })));
    t.advance(8);
    t.loop.frame();
    // The observer is 8 km south of the patch centre at 1.5 km up: the patch is 8 km ahead in -z... plus 1.5 km below.
    expect(t.renderer.lastFrame?.uniform[25]).toBeCloseTo(-1000, 5);
    expect(t.renderer.lastFrame?.uniform[26]).toBeCloseTo(-6000, 5);
  });

  it('reports frame rate, render cost, snapshot age and input-to-draw latency', () => {
    const t = rig();
    for (let i = 1; i <= 120; i++) {
      t.advance(1000 / 60);
      t.publish(i);
      t.loop.frame();
    }
    const s = t.loop.report();
    expect(s.backend).toBe('null');
    expect(s.frames).toBe(120);
    expect(s.fps).toBeGreaterThan(55);
    expect(s.fps).toBeLessThan(65);
    expect(s.frameMs.p50).toBeGreaterThan(15);
    expect(s.frameMs.p50).toBeLessThan(18);
    expect(s.renderCpuMs.count).toBe(120);
    expect(s.snapshotAgeMs.p50).toBeCloseTo(2, 0); // published 2 ms before the draw
    expect(s.inputToDrawMs.p50).toBeCloseTo(6, 0); // sampled 6 ms before the draw
    expect(s.snapshotsTaken).toBe(120);
    // The next report covers only what came after.
    expect(t.loop.report().renderCpuMs.count).toBe(0);
    expect(t.loop.report().renderCpuRun.count).toBe(120);
    expect(t.loop.report().frameMsRun.count).toBe(119); // the first frame has no interval
    t.loop.resetRun();
    expect(t.loop.report().renderCpuRun.count).toBe(0);
    expect(t.loop.report().frameMsRun.count).toBe(0);
  });

  it('counts frames drawn from a stale or gapped world', () => {
    const t = rig();
    t.publish(1);
    t.loop.frame();
    t.advance(10);
    t.publish(9);
    t.loop.frame();
    expect(t.loop.gappedFrames).toBeGreaterThan(0);
    t.advance(1000); // the sim stops publishing
    t.loop.frame();
    expect(t.loop.staleFrames).toBeGreaterThan(0);
  });

  it('moves everything to a new backend: size and terrain', () => {
    const t = rig();
    t.loop.resize(640, 360);
    t.loop.setTerrain(buildTerrainMesh(makeTestPatch()));
    const next = new NullRenderer();
    t.loop.setRenderer(next);
    expect(t.loop.renderer).toBe(next);
    expect([next.width, next.height]).toEqual([640, 360]);
    expect(next.terrain?.vertexCount).toBe(16_641);
  });

  it('clamps nonsense sizes', () => {
    const t = rig();
    t.loop.resize(0, -5);
    expect([t.loop.width, t.loop.height]).toEqual([1, 1]);
  });

  it('draws with the effect strength it was given, from 0 to 100 per cent', () => {
    const t = rig();
    t.publish(1, { ownship: { pos: [0, 0, 1000], gStress: 0.8 } });
    t.loop.frame();
    expect(t.renderer.lastFrame?.uniform[34]).toBe(1); // the default is full strength
    expect(t.renderer.lastFrame?.fx).toBe(true);
    t.loop.setSettings({ gfxStrength: 25 });
    t.loop.frame();
    expect(t.renderer.lastFrame?.uniform[34]).toBeCloseTo(0.25, 6);
    t.loop.setSettings({ gfxStrength: 0 });
    t.loop.frame();
    expect(t.renderer.lastFrame?.fx).toBe(false);
    t.loop.setSettings({ gfxStrength: 900 });
    t.loop.frame();
    expect(t.renderer.lastFrame?.uniform[34]).toBe(1);
  });
});
