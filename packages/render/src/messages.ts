/**
 * Messages between the main thread and the Render Worker. As with the Sim Worker's, bulk data
 * (snapshots, the terrain patch) travels through its own channel; these are commands and reports.
 */
import type { RendererChoice } from '@flying/platform-web/flags';
import type { Quantiles, SnapshotEndpoint } from '@flying/sim-bridge';
import type { ViewState } from '@flying/sim-bridge/view-channel';
import type { CockpitView } from './camera-rig.ts';
import type { RenderPresentation } from './present.ts';
import type { Backend, SampleCount } from './renderer.ts';
import type { DebugView, RenderFault } from './scene.ts';

/** Rendering settings the player (or a test) chose. */
export interface RenderSettings {
  /** How strongly the pilot's g and red-out show, 0 to 100 per cent. */
  gfxStrength: number;
  /** 4 for 4x MSAA (default 1, none). A backend that cannot do it stays at 1. */
  sampleCount?: SampleCount;
  /** `classify` draws flat key colours (tests); default `normal`. */
  debugView?: DebugView;
  /** A seeded render fault, honoured only by a debug build (tests that must fail). */
  fault?: RenderFault | null;
}

export interface RenderInit {
  type: 'init';
  canvas: OffscreenCanvas;
  renderer: RendererChoice;
  snapshot: SnapshotEndpoint;
  /** Where the terrain levels arrive (`{ type: 'terrain', level, patch }`), or `null` for none. */
  terrainPort: MessagePort | null;
  /** The view channel (`sim-bridge/view-channel`), if the page is isolated; else views arrive as messages. */
  viewBuffer: SharedArrayBuffer | null;
  /** Drawing-buffer size in device pixels. */
  width: number;
  height: number;
  /** Test hook: honour `stall` commands. */
  allowStall: boolean;
}

export type RenderCommand =
  | RenderInit
  /** A new canvas, sent in answer to `need-canvas`. */
  | { type: 'canvas'; canvas: OffscreenCanvas }
  | { type: 'resize'; width: number; height: number }
  | ({ type: 'settings' } & RenderSettings)
  /** What the player is doing with the view, when there is no shared channel. */
  | { type: 'view'; state: ViewState }
  /** The cockpit to look out of. */
  | { type: 'cockpit'; cockpit: CockpitView }
  /** The aircraft's presentation data: its meshes, HUD and font. */
  | { type: 'presentation'; presentation: RenderPresentation }
  | { type: 'read-pixels'; id: number }
  | { type: 'reset-run' }
  | { type: 'stall'; ms: number }
  | { type: 'stop' };

export interface RenderStats {
  /** When this report was made, on the shared clock. */
  atMs: number;
  backend: Backend;
  description: string;
  width: number;
  height: number;
  /** Frames drawn per second over the last interval. */
  fps: number;
  /** Time between frames, and CPU time spent in `render`, for the interval and for the run. */
  frameMs: Quantiles;
  /** Frame intervals since `reset-run` (the run's low percentiles come from here). */
  frameMsRun: Quantiles;
  renderCpuMs: Quantiles;
  renderCpuRun: Quantiles;
  /** From the controls being sampled to the frame that shows them being drawn (CPU side). */
  inputToDrawMs: Quantiles;
  inputToDrawRun: Quantiles;
  /** How old the newest snapshot was when drawn. */
  snapshotAgeMs: Quantiles;
  frames: number;
  snapshotsTaken: number;
  /** Frames drawn with a stale newest snapshot, across a gap, or after a snap. */
  staleFrames: number;
  gappedFrames: number;
  snappedFrames: number;
  /** Reads of the shared snapshot buffer that gave up. */
  contendedReads: number;
}

export type RenderEvent =
  | { type: 'ready'; backend: Backend; description: string; zRange: string }
  | { type: 'stats'; stats: RenderStats }
  /** The canvas has been committed to a context type that did not work out: send another. */
  | { type: 'need-canvas'; reason: string }
  /** The worker cannot render (for example no `requestAnimationFrame` in workers): draw on the main thread. */
  | { type: 'need-main-thread'; reason: string }
  | { type: 'lost'; reason: string }
  | {
      type: 'pixels';
      id: number;
      width: number;
      height: number;
      distinctColours: number;
      skyFraction: number;
      meanLuma: number;
    }
  | { type: 'error'; message: string };
