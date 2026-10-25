/**
 * Choosing a backend, and keeping one alive, for whatever hosts the renderer (the Render Worker or
 * the main thread). The host supplies canvases and a schedule; this decides what to draw with.
 *
 * Order for `auto` is WebGPU, then WebGL2; if neither starts the session fails with `SYS-103` and
 * the compat screen says so. An explicit choice tries only that backend. A canvas can hold only one
 * context type, so after a backend fails *after* touching its canvas the next one is given a fresh
 * canvas (the host is asked for one); after a failure that left the canvas alone the same canvas is
 * reused.
 *
 * A lost WebGPU device is rebuilt once on the same canvas; losing that one too means falling back
 * to WebGL2 on a fresh canvas. A lost WebGL2 context is restored by the browser if it can be
 * (the backend rebuilds itself); the session only reports it.
 */
import type { RendererChoice } from '@flying/platform-web/flags';
import type { SnapshotSource } from '@flying/sim-bridge';
import { SnapshotInterpolator } from './interp.ts';
import type { RenderEvent } from './messages.ts';
import { NullRenderer } from './null-renderer.ts';
import { RenderLoop } from './render-loop.ts';
import {
  CLEAR_COLOUR,
  RendererUnavailable,
  type CanvasLike,
  type Renderer,
  type ShaderSources,
} from './renderer.ts';
import { SceneBuilder } from './scene.ts';
import { buildTerrainMesh } from './terrain-mesh.ts';
import { WebGl2Renderer } from './webgl2-renderer.ts';
import { WebGpuRenderer } from './webgpu-renderer.ts';

export interface CanvasSource {
  /** The canvas to draw on now. */
  current(): CanvasLike;
  /** Asks the host for a new canvas, because the current one is committed to the wrong context type. */
  fresh(reason: string): Promise<CanvasLike>;
}

export interface RendererFactories {
  webgpu(canvas: CanvasLike, shaders: ShaderSources): Promise<Renderer>;
  webgl2(canvas: CanvasLike, shaders: ShaderSources): Renderer;
}

export const REAL_FACTORIES: RendererFactories = {
  webgpu: (canvas, shaders) => WebGpuRenderer.create(canvas, shaders),
  webgl2: (canvas, shaders) => WebGl2Renderer.create(canvas, shaders),
};

/**
 * Starts the first backend that works. `tried` is called with a line per failure, for the log.
 *
 * @throws Error `SYS-103` if none starts.
 */
export async function createRenderer(
  choice: RendererChoice,
  canvases: CanvasSource,
  shaders: ShaderSources,
  factories: RendererFactories = REAL_FACTORIES,
  tried: (line: string) => void = () => undefined,
): Promise<Renderer> {
  if (choice === 'null') {
    return new NullRenderer();
  }
  const order = choice === 'auto' ? (['webgpu', 'webgl2'] as const) : ([choice] as const);
  let needFresh = false;
  const failures: string[] = [];
  for (const backend of order) {
    try {
      const canvas = needFresh
        ? await canvases.fresh(`${backend} needs an unused canvas`)
        : canvases.current();
      needFresh = false;
      return backend === 'webgpu'
        ? await factories.webgpu(canvas, shaders)
        : factories.webgl2(canvas, shaders);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      failures.push(`${backend}: ${message}`);
      tried(`${backend} did not start: ${message}`);
      // Anything but a clean "unavailable" is assumed to have used the canvas up.
      needFresh = !(e instanceof RendererUnavailable) || e.canvasTouched;
    }
  }
  throw new Error(`SYS-103: no renderer could start (${failures.join('; ')})`);
}

export interface RenderSessionOptions {
  choice: RendererChoice;
  canvases: CanvasSource;
  shaders: ShaderSources;
  source: SnapshotSource;
  emit: (event: RenderEvent) => void;
  sharedNowMs: () => number;
  monoMs: () => number;
  factories?: RendererFactories;
  log?: (line: string) => void;
}

export class RenderSession {
  readonly #o: RenderSessionOptions;
  readonly #factories: RendererFactories;
  #loop: RenderLoop | undefined;
  #nextReport = 0;
  #webgpuLosses = 0;
  #stopped = false;
  #settings: { gfxStrength: number } | undefined;

  constructor(options: RenderSessionOptions) {
    this.#o = options;
    this.#factories = options.factories ?? REAL_FACTORIES;
  }

  get renderer(): Renderer | undefined {
    return this.#loop?.renderer;
  }

  get loop(): RenderLoop | undefined {
    return this.#loop;
  }

  /** Creates the renderer and reports `ready`. Rejects with `SYS-103` if nothing starts. */
  async start(widthPx: number, heightPx: number): Promise<void> {
    const o = this.#o;
    const renderer = await createRenderer(o.choice, o.canvases, o.shaders, this.#factories, o.log);
    const loop = new RenderLoop(
      {
        source: o.source,
        interp: new SnapshotInterpolator(),
        scene: new SceneBuilder(),
        sharedNowMs: o.sharedNowMs,
        monoMs: o.monoMs,
      },
      renderer,
    );
    loop.resize(widthPx, heightPx);
    if (this.#settings) loop.setSettings(this.#settings);
    this.#loop = loop;
    this.#watch(renderer);
    this.#nextReport = o.monoMs() + 250;
    o.emit({
      type: 'ready',
      backend: renderer.backend,
      description: renderer.description,
      zRange: renderer.zRange,
    });
  }

  /** Installs a terrain patch from the sim (the bytes in `TERRAIN` layout). */
  setTerrain(patch: ArrayBuffer): void {
    this.#loop?.setTerrain(buildTerrainMesh(new Uint8Array(patch)));
  }

  resize(widthPx: number, heightPx: number): void {
    this.#loop?.resize(widthPx, heightPx);
  }

  /** Applies the player's rendering settings (before the renderer exists they are kept). */
  setSettings(settings: { gfxStrength: number }): void {
    this.#settings = settings;
    this.#loop?.setSettings(settings);
  }

  resetRun(): void {
    this.#loop?.resetRun();
  }

  /** One frame: draw, and report statistics every 250 ms. Call it from the host's schedule. */
  frame(): void {
    const loop = this.#loop;
    if (!loop || this.#stopped) return;
    loop.frame();
    const now = this.#o.monoMs();
    if (now >= this.#nextReport) {
      this.#o.emit({ type: 'stats', stats: loop.report() });
      this.#nextReport = now + 250;
    }
  }

  /** Reads the last frame back and summarises it. */
  async pixelSummary(): Promise<{
    width: number;
    height: number;
    distinctColours: number;
    skyFraction: number;
    meanLuma: number;
  }> {
    const loop = this.#loop;
    if (!loop) return { width: 0, height: 0, distinctColours: 0, skyFraction: 0, meanLuma: 0 };
    const px = await loop.renderer.debugReadPixels();
    const sky = CLEAR_COLOUR.map((c) => Math.round(c * 255));
    const seen = new Set<number>();
    let skyPixels = 0;
    let luma = 0;
    for (let i = 0; i < px.length; i += 4) {
      const r = px[i] ?? 0;
      const g = px[i + 1] ?? 0;
      const b = px[i + 2] ?? 0;
      seen.add((r << 16) | (g << 8) | b);
      luma += 0.2126 * r + 0.7152 * g + 0.0722 * b; // Rec. 709 weights
      if (
        Math.abs(r - (sky[0] ?? 0)) <= 1 &&
        Math.abs(g - (sky[1] ?? 0)) <= 1 &&
        Math.abs(b - (sky[2] ?? 0)) <= 1
      ) {
        skyPixels += 1;
      }
    }
    const total = Math.max(1, px.length / 4);
    return {
      width: loop.width,
      height: loop.height,
      distinctColours: seen.size,
      skyFraction: skyPixels / total,
      meanLuma: luma / total / 255,
    };
  }

  stop(): void {
    this.#stopped = true;
    this.#loop?.renderer.dispose();
  }

  #watch(renderer: Renderer): void {
    renderer.onLost((reason) => {
      void this.#lost(renderer, reason);
    });
  }

  async #lost(renderer: Renderer, reason: string): Promise<void> {
    const loop = this.#loop;
    if (this.#stopped || !loop || renderer !== loop.renderer) return;
    const o = this.#o;
    o.emit({ type: 'lost', reason });
    o.log?.(`renderer lost: ${reason}`);
    if (renderer.backend !== 'webgpu') {
      return; // WebGL2 rebuilds itself if the browser restores the context
    }
    this.#webgpuLosses += 1;
    renderer.dispose();
    try {
      let next: Renderer;
      if (this.#webgpuLosses === 1) {
        next = await this.#factories.webgpu(o.canvases.current(), o.shaders); // once, on the same canvas
      } else {
        next = this.#factories.webgl2(await o.canvases.fresh('WebGPU was lost twice'), o.shaders);
      }
      loop.setRenderer(next);
      this.#watch(next);
      o.emit({
        type: 'ready',
        backend: next.backend,
        description: next.description,
        zRange: next.zRange,
      });
    } catch (e) {
      o.emit({
        type: 'error',
        message: `SYS-104: could not recover the renderer: ${e instanceof Error ? e.message : String(e)}`,
      });
    }
  }
}
