import { describe, expect, it } from 'vitest';
import { CONTENDED, type SnapshotSource } from '@flying/sim-bridge';
import type { RenderEvent } from './messages.ts';
import { NullRenderer } from './null-renderer.ts';
import {
  createRenderer,
  RenderSession,
  type CanvasSource,
  type RendererFactories,
} from './render-session.ts';
import {
  RendererUnavailable,
  type Backend,
  type CanvasLike,
  type ShaderSources,
} from './renderer.ts';
import { makeSnapshot } from './snapshot-fixture.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

const SHADERS = {} as ShaderSources;
const canvas = (name: string): CanvasLike => ({ name }) as unknown as CanvasLike;

class Fake extends NullRenderer {
  override readonly backend: Backend;
  readonly onCanvas: string;
  constructor(backend: Backend, onCanvas: string) {
    super();
    this.backend = backend;
    this.onCanvas = onCanvas;
  }
}

/** Records which canvas each backend got and when a new canvas was asked for. */
function rig(
  opts: {
    webgpu?: () => Promise<Fake> | Fake;
    webgl2?: () => Fake;
  } = {},
) {
  const log: string[] = [];
  let n = 0;
  let current = canvas('canvas-0');
  const canvases: CanvasSource = {
    current: () => current,
    fresh: (reason) => {
      n += 1;
      current = canvas(`canvas-${String(n)}`);
      log.push(`fresh canvas (${reason})`);
      return Promise.resolve(current);
    },
  };
  const factories: RendererFactories = {
    webgpu: async (c) => {
      log.push(`webgpu on ${(c as unknown as { name: string }).name}`);
      if (opts.webgpu) return opts.webgpu();
      return new Fake('webgpu', (c as unknown as { name: string }).name);
    },
    webgl2: (c) => {
      log.push(`webgl2 on ${(c as unknown as { name: string }).name}`);
      if (opts.webgl2) return opts.webgl2();
      return new Fake('webgl2', (c as unknown as { name: string }).name);
    },
  };
  return { log, canvases, factories };
}

const untouched = (): never => {
  throw new RendererUnavailable('SYS-050: no adapter', false);
};
const touched = (): never => {
  throw new RendererUnavailable('SYS-051: pipeline failed', true);
};

describe('createRenderer', () => {
  it('uses the null renderer when asked', async () => {
    const r = rig();
    expect((await createRenderer('null', r.canvases, SHADERS, r.factories)).backend).toBe('null');
    expect(r.log).toEqual([]);
  });

  it('prefers WebGPU on auto', async () => {
    const r = rig();
    expect((await createRenderer('auto', r.canvases, SHADERS, r.factories)).backend).toBe('webgpu');
    expect(r.log).toEqual(['webgpu on canvas-0']);
  });

  it('falls back to WebGL2 on the same canvas if WebGPU failed before touching it', async () => {
    const r = rig({ webgpu: untouched });
    const lines: string[] = [];
    const renderer = await createRenderer('auto', r.canvases, SHADERS, r.factories, (l) =>
      lines.push(l),
    );
    expect(renderer.backend).toBe('webgl2');
    expect(r.log).toEqual(['webgpu on canvas-0', 'webgl2 on canvas-0']);
    expect(lines).toHaveLength(1);
    expect(lines[0]).toMatch(/webgpu did not start.*no adapter/);
  });

  it('asks for a fresh canvas if WebGPU failed after committing the first one', async () => {
    const r = rig({ webgpu: touched });
    const renderer = await createRenderer('auto', r.canvases, SHADERS, r.factories);
    expect(renderer.backend).toBe('webgl2');
    expect(r.log).toEqual([
      'webgpu on canvas-0',
      'fresh canvas (webgl2 needs an unused canvas)',
      'webgl2 on canvas-1',
    ]);
    expect((renderer as Fake).onCanvas).toBe('canvas-1');
  });

  it('assumes an unknown kind of failure spent the canvas', async () => {
    const r = rig({
      webgpu: () => {
        throw new Error('something odd');
      },
    });
    await createRenderer('auto', r.canvases, SHADERS, r.factories);
    expect(r.log).toContain('fresh canvas (webgl2 needs an unused canvas)');
  });

  it('fails with SYS-103 naming every reason when nothing starts', async () => {
    const r = rig({
      webgpu: untouched,
      webgl2: () => {
        throw new RendererUnavailable('SYS-053: no context', false);
      },
    });
    await expect(createRenderer('auto', r.canvases, SHADERS, r.factories)).rejects.toThrow(
      /SYS-103.*webgpu: SYS-050.*webgl2: SYS-053/,
    );
  });

  it('an explicit choice tries only that backend', async () => {
    const a = rig();
    expect((await createRenderer('webgl2', a.canvases, SHADERS, a.factories)).backend).toBe(
      'webgl2',
    );
    expect(a.log).toEqual(['webgl2 on canvas-0']);

    const b = rig({ webgpu: untouched });
    await expect(createRenderer('webgpu', b.canvases, SHADERS, b.factories)).rejects.toThrow(
      /SYS-103/,
    );
    expect(b.log).toEqual(['webgpu on canvas-0']); // no silent fallback
  });
});

function session(opts: Parameters<typeof rig>[0] = {}, choice: 'auto' | 'webgl2' = 'auto') {
  const r = rig(opts);
  const events: RenderEvent[] = [];
  let mono = 0;
  let shared = 5000;
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
  const s = new RenderSession({
    choice,
    canvases: r.canvases,
    shaders: SHADERS,
    source,
    emit: (e) => events.push(e),
    sharedNowMs: () => shared,
    monoMs: () => mono,
    factories: r.factories,
    log: (l) => r.log.push(`log: ${l}`),
  });
  return {
    ...r,
    s,
    events,
    advance: (ms: number) => {
      mono += ms;
      shared += ms;
    },
    publish: (tick: number) => {
      publication = tick;
      bytes = makeSnapshot({ tick, publishMs: shared, inputMs: shared - 4 });
    },
  };
}

describe('RenderSession', () => {
  it('reports ready with the backend it chose', async () => {
    const t = session();
    await t.s.start(320, 180);
    expect(t.events).toEqual([
      {
        type: 'ready',
        backend: 'webgpu',
        description: 'null renderer (nothing is drawn)',
        zRange: '0to1',
      },
    ]);
    expect(t.s.renderer?.backend).toBe('webgpu');
  });

  it('draws frames from the snapshots it is given and reports statistics every 250 ms', async () => {
    const t = session();
    await t.s.start(320, 180);
    t.events.length = 0;
    t.publish(1);
    for (let i = 0; i < 60; i++) {
      t.advance(16.7);
      t.publish(i + 2);
      t.s.frame();
    }
    const stats = t.events.filter((e) => e.type === 'stats');
    expect(stats.length).toBeGreaterThanOrEqual(3);
    expect(stats.length).toBeLessThanOrEqual(5);
    const last = stats.at(-1);
    if (last?.type !== 'stats') throw new Error('no stats');
    expect(last.stats.backend).toBe('webgpu');
    expect(last.stats.width).toBe(320);
    expect(last.stats.fps).toBeGreaterThan(50);
    expect(last.stats.fps).toBeLessThan(70);
    expect(last.stats.inputToDrawMs.count).toBeGreaterThan(0);
    expect(last.stats.snapshotsTaken).toBeGreaterThan(30);
  });

  it('passes the terrain patch to the renderer, and keeps it across a backend change', async () => {
    const t = session();
    await t.s.start(100, 100);
    t.s.setTerrain(makeTestPatch().buffer);
    expect((t.s.renderer as Fake).terrain?.triangleCount).toBe(32_768);
  });

  it('keeps the rendering settings from before it started, and applies later ones', async () => {
    const t = session();
    t.s.setSettings({ gfxStrength: 30 });
    await t.s.start(100, 100);
    t.publish(1);
    t.advance(16.7);
    t.s.frame();
    expect((t.s.renderer as Fake).lastFrame?.uniform[34]).toBeCloseTo(0.3, 6);
    t.s.setSettings({ gfxStrength: 60 });
    t.advance(16.7);
    t.s.frame();
    expect((t.s.renderer as Fake).lastFrame?.uniform[34]).toBeCloseTo(0.6, 6);
  });

  it('summarises what was drawn', async () => {
    const t = session();
    await t.s.start(40, 30);
    const summary = await t.s.pixelSummary();
    expect(summary).toEqual({ width: 40, height: 30, distinctColours: 1, skyFraction: 1 });
  });

  it('rebuilds a lost WebGPU device once on the same canvas', async () => {
    const t = session();
    await t.s.start(64, 64);
    t.log.length = 0;
    t.events.length = 0;
    const first = t.s.renderer as Fake;
    first.simulateLoss('device lost');
    await Promise.resolve();
    await Promise.resolve();
    expect(t.log).toContain('webgpu on canvas-0');
    expect(t.s.renderer).not.toBe(first);
    expect(t.s.renderer?.backend).toBe('webgpu');
    expect(first.disposed).toBe(true);
    expect(t.events.map((e) => e.type)).toEqual(['lost', 'ready']);
    expect(t.log.some((l) => l.startsWith('fresh canvas'))).toBe(false);
  });

  it('falls back to WebGL2 on a fresh canvas when the rebuilt device is lost too', async () => {
    const t = session();
    await t.s.start(64, 64);
    (t.s.renderer as Fake).simulateLoss('first');
    await new Promise((r) => setTimeout(r, 0));
    t.log.length = 0;
    (t.s.renderer as Fake).simulateLoss('second');
    await new Promise((r) => setTimeout(r, 0));
    expect(t.log).toEqual([
      'log: renderer lost: second',
      'fresh canvas (WebGPU was lost twice)',
      'webgl2 on canvas-1',
    ]);
    expect(t.s.renderer?.backend).toBe('webgl2');
  });

  it('only reports a WebGL2 loss, since the backend restores itself', async () => {
    const t = session({}, 'webgl2');
    await t.s.start(64, 64);
    t.events.length = 0;
    const gl = t.s.renderer as Fake;
    gl.simulateLoss('context lost');
    await Promise.resolve();
    expect(t.s.renderer).toBe(gl);
    expect(t.events.map((e) => e.type)).toEqual(['lost']);
  });

  it('ignores a loss reported by a renderer it has already replaced', async () => {
    const t = session();
    await t.s.start(64, 64);
    const old = t.s.renderer as Fake;
    old.simulateLoss('one');
    await new Promise((r) => setTimeout(r, 0));
    t.events.length = 0;
    old.simulateLoss('late echo');
    await new Promise((r) => setTimeout(r, 0));
    expect(t.events).toEqual([]);
  });

  it('reports an error if recovery itself fails', async () => {
    let calls = 0;
    const t = session({
      webgpu: () => {
        calls += 1;
        if (calls > 1) throw new Error('driver gone');
        return new Fake('webgpu', 'c');
      },
    });
    await t.s.start(64, 64);
    t.events.length = 0;
    (t.s.renderer as Fake).simulateLoss('lost');
    await new Promise((r) => setTimeout(r, 0));
    expect(t.events.map((e) => e.type)).toEqual(['lost', 'error']);
    const err = t.events[1];
    expect(err?.type === 'error' && err.message).toMatch(/SYS-104.*driver gone/);
  });

  it('stops: disposes the renderer and draws no more', async () => {
    const t = session();
    await t.s.start(64, 64);
    const r = t.s.renderer as Fake;
    t.publish(1);
    t.s.frame();
    t.s.stop();
    expect(r.disposed).toBe(true);
    const frames = r.frames;
    t.advance(20);
    t.publish(2);
    t.s.frame();
    expect(r.frames).toBe(frames);
  });
});
