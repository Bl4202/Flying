/**
 * One frame of rendering, independent of where it runs (Render Worker or main thread) and of what
 * schedules it: read the newest snapshot, interpolate, build the frame, draw, and keep the numbers
 * the overlay and the perf harness show.
 */
import { Histogram } from '@flying/platform-web/metrics';
import { CONTENDED, SNAPSHOT, type Quantiles, type SnapshotSource } from '@flying/sim-bridge';
import type { SnapshotInterpolator } from './interp.ts';
import type { RenderStats } from './messages.ts';
import type { Renderer } from './renderer.ts';
import type { SceneBuilder } from './scene.ts';
import type { TerrainMesh } from './terrain-mesh.ts';

export interface RenderLoopDeps {
  source: SnapshotSource;
  interp: SnapshotInterpolator;
  scene: SceneBuilder;
  /** Milliseconds on the clock all threads share (publish and input timestamps use it). */
  sharedNowMs: () => number;
  /** Monotonic milliseconds on this thread: durations. */
  monoMs: () => number;
}

function quantiles(h: Histogram): Quantiles {
  return {
    count: h.count,
    avg: h.mean,
    p50: h.quantile(0.5),
    p95: h.quantile(0.95),
    p99: h.quantile(0.99),
    max: h.peak,
  };
}

export class RenderLoop {
  readonly #d: RenderLoopDeps;
  readonly #buf = new Uint8Array(SNAPSHOT.SIZE);
  // Frame interval and CPU cost: 0.25 ms bins to 100 ms. Latency and age: 0.5 ms bins to 200 ms.
  readonly #interval = new Histogram(100, 400);
  readonly #intervalRun = new Histogram(100, 400);
  readonly #cpu = new Histogram(100, 400);
  readonly #cpuRun = new Histogram(100, 400);
  readonly #latency = new Histogram(200, 400);
  readonly #latencyRun = new Histogram(200, 400);
  readonly #age = new Histogram(200, 400);
  #renderer: Renderer;
  #terrain: TerrainMesh | undefined;
  #lastPublication = 0;
  #lastFrameAt = Number.NaN;
  #windowStart: number;
  #windowFrames = 0;
  #aspect = 1;
  /** How strongly the pilot's g and red-out show on the screen, 0 to 100 per cent. */
  #gfxStrength = 100;
  width = 1;
  height = 1;
  frames = 0;
  staleFrames = 0;
  gappedFrames = 0;
  snappedFrames = 0;
  contendedReads = 0;

  constructor(deps: RenderLoopDeps, renderer: Renderer) {
    this.#d = deps;
    this.#renderer = renderer;
    this.#windowStart = deps.monoMs();
  }

  get renderer(): Renderer {
    return this.#renderer;
  }

  /** Replaces the backend (after a loss or a fallback) and gives it what the old one had. */
  setRenderer(renderer: Renderer): void {
    this.#renderer = renderer;
    renderer.resize(this.width, this.height);
    if (this.#terrain) renderer.setTerrainPatch(this.#terrain);
  }

  setTerrain(mesh: TerrainMesh): void {
    this.#terrain = mesh;
    this.#renderer.setTerrainPatch(mesh);
  }

  resize(widthPx: number, heightPx: number): void {
    this.width = Math.max(1, Math.floor(widthPx));
    this.height = Math.max(1, Math.floor(heightPx));
    this.#aspect = this.width / this.height;
    this.#renderer.resize(this.width, this.height);
  }

  /** Applies the player's rendering settings. */
  setSettings(settings: { gfxStrength: number }): void {
    this.#gfxStrength = Math.min(Math.max(settings.gfxStrength, 0), 100);
  }

  resetRun(): void {
    this.#intervalRun.reset();
    this.#cpuRun.reset();
    this.#latencyRun.reset();
  }

  /** Draws one frame. Returns false if there was nothing to draw yet (no snapshot has arrived). */
  frame(): boolean {
    const d = this.#d;
    const now = d.monoMs();
    const shared = d.sharedNowMs();

    const n = d.source.read(this.#buf);
    if (n === CONTENDED) {
      this.contendedReads += 1;
    } else if (n > 0 && n !== this.#lastPublication) {
      this.#lastPublication = n;
      d.interp.push(this.#buf);
    }
    const state = d.interp.sample(shared);
    if (state === null) {
      return false;
    }

    const frame = d.scene.build(state, {
      aspect: this.#aspect,
      zRange: this.#renderer.zRange,
      timeS: now / 1000,
      patchOriginBf: this.#terrain?.originBf ?? null,
      spacingM: this.#terrain?.spacingM ?? 1,
      gfxStrength: this.#gfxStrength,
    });
    const t0 = d.monoMs();
    this.#renderer.render(frame);
    const spent = d.monoMs() - t0;

    this.frames += 1;
    this.#windowFrames += 1;
    this.#cpu.record(spent);
    this.#cpuRun.record(spent);
    if (!Number.isNaN(this.#lastFrameAt)) {
      this.#interval.record(now - this.#lastFrameAt);
      this.#intervalRun.record(now - this.#lastFrameAt);
    }
    this.#lastFrameAt = now;
    this.#age.record(shared - state.publishTimeMs);
    if (state.inputTimeMs > 0) {
      const latency = shared - state.inputTimeMs;
      this.#latency.record(latency);
      this.#latencyRun.record(latency);
    }
    const st = d.interp.status;
    if (st.stale) this.staleFrames += 1;
    if (st.gapped) this.gappedFrames += 1;
    if (st.snapped) this.snappedFrames += 1;
    return true;
  }

  /** A report on the interval since the last one; starts a new interval. */
  report(): RenderStats {
    const d = this.#d;
    const now = d.monoMs();
    const seconds = Math.max((now - this.#windowStart) / 1000, 1e-9);
    const stats: RenderStats = {
      atMs: d.sharedNowMs(),
      backend: this.#renderer.backend,
      description: this.#renderer.description,
      width: this.width,
      height: this.height,
      fps: this.#windowFrames / seconds,
      frameMs: quantiles(this.#interval),
      frameMsRun: quantiles(this.#intervalRun),
      renderCpuMs: quantiles(this.#cpu),
      renderCpuRun: quantiles(this.#cpuRun),
      inputToDrawMs: quantiles(this.#latency),
      inputToDrawRun: quantiles(this.#latencyRun),
      snapshotAgeMs: quantiles(this.#age),
      frames: this.frames,
      snapshotsTaken: d.interp.accepted,
      staleFrames: this.staleFrames,
      gappedFrames: this.gappedFrames,
      snappedFrames: this.snappedFrames,
      contendedReads: this.contendedReads,
    };
    this.#interval.reset();
    this.#cpu.reset();
    this.#latency.reset();
    this.#age.reset();
    this.#windowStart = now;
    this.#windowFrames = 0;
    return stats;
  }
}
