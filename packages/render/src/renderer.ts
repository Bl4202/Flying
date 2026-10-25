/**
 * The Renderer seam (GDD 61.6). A backend takes finished frame data (uniform block and marker
 * instances, built by `SceneBuilder`) and draws it; it knows nothing about snapshots, cameras or
 * the simulation. WebGPU, WebGL2 and a null renderer implement it, and all three must pass the
 * contract in `renderer-contract.ts`.
 */
import type { ZRange } from './camera.ts';
import type { FrameData } from './scene.ts';
import type { TerrainMesh } from './terrain-mesh.ts';

export type Backend = 'webgpu' | 'webgl2' | 'null';

/** The sky colour the frame is cleared to; the shaders fog towards the same colour. */
export const CLEAR_COLOUR: readonly [number, number, number] = [0.45, 0.62, 0.85];

export interface Renderer {
  readonly backend: Backend;
  /** The adapter or GL renderer string, for the overlay and benchmark reports. */
  readonly description: string;
  /** The depth range the scene must be built for. */
  readonly zRange: ZRange;
  /** Sets the drawing-buffer size in device pixels. */
  resize(widthPx: number, heightPx: number): void;
  /** Uploads the terrain patch (replacing any earlier one). */
  setTerrainPatch(mesh: TerrainMesh): void;
  /** Draws a frame. Does not wait for the GPU. */
  render(frame: FrameData): void;
  /** Resolves once the GPU has finished every frame drawn so far (for timing, never per frame). */
  finish(): Promise<void>;
  /**
   * Draws the most recent frame again to an off-screen target and returns it as RGBA bytes, top
   * row first, `width * height * 4` long.
   */
  debugReadPixels(): Promise<Uint8Array>;
  /** Calls `listener` if the GPU device or context is lost (it may then be restored or not). */
  onLost(listener: (reason: string) => void): void;
  /** Releases GPU resources. Safe to call twice; drawing after it does nothing. */
  dispose(): void;
}

/** One GLSL program as generated from WGSL (see `wgsl-gen`). */
export interface GlslProgram {
  vertex: string;
  fragment: string;
  /** The uniform block name in each stage, or null for a stage that reads no uniforms. */
  blocks: { vertex: string | null; fragment: string | null };
}

/** Everything the backends compile. */
export interface ShaderSources {
  terrainWgsl: string;
  markerWgsl: string;
  terrainGlsl: GlslProgram;
  markerGlsl: GlslProgram;
  /** The screen-effect pass (the pilot's g and red-out), drawn over the finished picture. */
  fxWgsl: string;
  fxGlsl: GlslProgram;
}

/** What a backend draws onto. */
export type CanvasLike = HTMLCanvasElement | OffscreenCanvas;

/**
 * A backend could not start. `canvasTouched` says whether the canvas had already committed to a
 * context type: a canvas holds one for its whole life, so after a touched failure the next backend
 * needs a fresh canvas, while after an untouched one it can use the same.
 */
export class RendererUnavailable extends Error {
  canvasTouched: boolean;
  constructor(message: string, canvasTouched: boolean) {
    super(message);
    this.name = 'RendererUnavailable';
    this.canvasTouched = canvasTouched;
  }
}
