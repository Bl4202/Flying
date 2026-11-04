/**
 * One frame of rendering, independent of where it runs (Render Worker or main thread) and of what
 * schedules it: read the newest snapshot, interpolate, build the frame, draw, and keep the numbers
 * the overlay and the perf harness show.
 */
import { Histogram } from '@flying/platform-web/metrics';
import {
  CONTENDED,
  newAircraftStatus,
  readAircraftStatus,
  SNAPSHOT,
  SnapshotReader,
  warningBits,
  type Quantiles,
  type SnapshotSource,
} from '@flying/sim-bridge';
import {
  copyViewState,
  newViewState,
  type ViewReader,
  type ViewState,
} from '@flying/sim-bridge/view-channel';
import {
  CameraRig,
  cockpitView,
  DEFAULT_COCKPIT,
  type CameraPose,
  type CockpitView,
} from './camera-rig.ts';
import { bodyToRender } from './camera.ts';
import type { Declutter } from './hud/elements.ts';
import { Hud } from './hud/hud.ts';
import { buildCockpitMesh, buildModelMesh } from './model-mesh.ts';
import { ModelRig } from './model-rig.ts';
import type { RenderPresentation } from './present.ts';
import type { SceneState, SnapshotInterpolator } from './interp.ts';
import type { RenderSettings, RenderStats } from './messages.ts';
import type { Renderer } from './renderer.ts';
import type { FrameData, SceneBuilder } from './scene.ts';
import { TerrainSet } from './terrain-levels.ts';

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
  readonly #terrain = new TerrainSet();
  readonly #rig: CameraRig;
  readonly #view: ViewState = newViewState();
  #viewReader: ViewReader | undefined;
  /** The aircraft's presentation, once known: its meshes are uploaded and its parts animated. */
  #presentation: RenderPresentation | undefined;
  #modelRig: ModelRig | undefined;
  #hud: Hud | undefined;
  /** The newest snapshot's aircraft status, for the HUD and the warnings. */
  readonly #status = newAircraftStatus();
  readonly #reader: SnapshotReader;
  readonly #rot = new Float32Array(16);
  /** The camera rig drives the picture once it is told anything about the view (else the eye is the reference point). */
  #rigActive = false;
  #lastRigAt = Number.NaN;
  #lastPublication = 0;
  #lastFrameAt = Number.NaN;
  #windowStart: number;
  #windowFrames = 0;
  #aspect = 1;
  /** How strongly the pilot's g and red-out show on the screen, 0 to 100 per cent. */
  #gfxStrength = 100;
  #settings: RenderSettings = { gfxStrength: 100 };
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
    this.#rig = new CameraRig(DEFAULT_COCKPIT, this.#terrain);
    this.#reader = new SnapshotReader(this.#buf);
    this.#windowStart = deps.monoMs();
  }

  get renderer(): Renderer {
    return this.#renderer;
  }

  /** Replaces the backend (after a loss or a fallback) and gives it what the old one had. */
  setRenderer(renderer: Renderer): void {
    this.#renderer = renderer;
    renderer.resize(this.width, this.height);
    renderer.setSampleCount(this.#settings.sampleCount ?? 1);
    this.#uploadAircraft(renderer);
    this.#terrain.installed().forEach((mesh) => {
      renderer.setTerrainLevel(mesh.level, mesh);
    });
  }

  /** Builds this frame's HUD into `frame`: on the glass in the cockpit view, as an overlay in the chase view. */
  #drawHud(frame: FrameData, state: SceneState, camera: CameraPose, nowMs: number): void {
    const hud = this.#hud;
    const view = this.#view;
    if (!hud || this.#status.assist === 0 || view.hudBrightness <= 0) return;
    bodyToRender(this.#rot, state.ownshipQuat);
    hud.fillFrame(
      frame,
      {
        posBf: state.ownshipPosBf,
        quatBf: state.ownshipQuat,
        velBf: state.ownshipVelBf,
        status: this.#status,
        warnings: warningBits(this.#status),
        timeS: nowMs / 1000,
      },
      this.#rot,
      {
        declutter: view.hudDeclutter as Declutter,
        brightness: view.hudBrightness / 100,
        palette: this.#settings.hudPalette ?? 'green',
        overlay: camera.mode === 'chase' || this.#settings.hudOverlay === true,
        overlayScale: this.#settings.hudScale ?? 1,
        widthPx: this.width,
        heightPx: this.height,
        fovY: camera.fovY,
        eyeBf: camera.eyeBf,
      },
    );
  }

  /** The camera rig. */
  get rig(): CameraRig {
    return this.#rig;
  }

  /** The view the rig last used (what the player is asking of the camera). */
  get view(): Readonly<ViewState> {
    return this.#view;
  }

  /** Takes the view from a shared channel, read every frame; switches the camera rig on. */
  setViewReader(reader: ViewReader | undefined): void {
    this.#viewReader = reader;
    this.#rigActive = true;
  }

  /** Takes the view from a message (no shared channel); switches the camera rig on. */
  setView(state: ViewState): void {
    copyViewState(this.#view, state);
    this.#rigActive = true;
  }

  /**
   * The aircraft's presentation: builds its exterior and cockpit meshes and uploads them, animates
   * its parts from the snapshot, and looks out of its cockpit (which switches the camera rig on).
   *
   * @throws RangeError (`SYS-040`) if a mesh does not fit its limits.
   */
  setPresentation(presentation: RenderPresentation): void {
    this.#presentation = presentation;
    this.#modelRig = new ModelRig(presentation.model);
    this.#hud = new Hud(presentation.hud, presentation.cockpit, presentation.font);
    this.#uploadAircraft(this.#renderer);
    this.setCockpit(cockpitView(presentation.cockpit));
  }

  /** Builds the aircraft's meshes and gives them to `renderer`. */
  #uploadAircraft(renderer: Renderer): void {
    const p = this.#presentation;
    if (!p) return;
    renderer.setMesh('exterior', buildModelMesh(p.model));
    renderer.setMesh('cockpit', buildCockpitMesh(p.cockpit));
  }

  /** The cockpit to look out of (from the presentation data); switches the camera rig on. */
  setCockpit(cockpit: CockpitView): void {
    this.#rig.setCockpit(cockpit);
    this.#rigActive = true;
  }

  /** The terrain levels as installed so far. */
  get terrain(): TerrainSet {
    return this.#terrain;
  }

  /**
   * Installs terrain level `level` from the sim's bytes and uploads it, and the next coarser level
   * if its hole moved.
   *
   * @throws RangeError (`SYS-040`) if the bytes are not a terrain patch.
   */
  setTerrain(level: number, patch: Uint8Array): void {
    for (const changed of this.#terrain.set(level, patch)) {
      const mesh = this.#terrain.mesh(changed);
      if (mesh) this.#renderer.setTerrainLevel(changed, mesh);
    }
  }

  resize(widthPx: number, heightPx: number): void {
    this.width = Math.max(1, Math.floor(widthPx));
    this.height = Math.max(1, Math.floor(heightPx));
    this.#aspect = this.width / this.height;
    this.#renderer.resize(this.width, this.height);
  }

  /** Applies the player's rendering settings. */
  setSettings(settings: RenderSettings): void {
    this.#settings = settings;
    this.#gfxStrength = Math.min(Math.max(settings.gfxStrength, 0), 100);
    this.#renderer.setSampleCount(settings.sampleCount ?? 1);
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
      readAircraftStatus(this.#reader, this.#status);
    }
    const state = d.interp.sample(shared);
    if (state === null) {
      return false;
    }

    let camera: CameraPose | undefined;
    if (this.#rigActive) {
      this.#viewReader?.read(this.#view);
      const dt = Number.isNaN(this.#lastRigAt) ? 0 : (now - this.#lastRigAt) / 1000;
      this.#lastRigAt = now;
      camera = this.#rig.step(state, this.#view, dt);
    }
    const frame = d.scene.build(state, {
      ...(camera ? { camera } : {}),
      ...(this.#modelRig ? { model: this.#modelRig } : {}),
      aspect: this.#aspect,
      zRange: this.#renderer.zRange,
      timeS: now / 1000,
      terrain: this.#terrain,
      gfxStrength: this.#gfxStrength,
      debugView: this.#settings.debugView ?? 'normal',
      fault: this.#settings.fault ?? null,
    });
    if (camera) this.#drawHud(frame, state, camera, now);
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
