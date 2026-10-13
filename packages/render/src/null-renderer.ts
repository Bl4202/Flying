/**
 * A renderer that draws nothing. It is the fallback when no GPU path works but the rest of the app
 * should still run (the sim, the overlay, the input), and what tests use for the render loop.
 */
import type { ZRange } from './camera.ts';
import { CLEAR_COLOUR, type Backend, type Renderer } from './renderer.ts';
import type { FrameData } from './scene.ts';
import type { TerrainMesh } from './terrain-mesh.ts';

export class NullRenderer implements Renderer {
  readonly backend: Backend = 'null';
  readonly description = 'null renderer (nothing is drawn)';
  readonly zRange: ZRange = '0to1';
  width = 1;
  height = 1;
  frames = 0;
  terrain: TerrainMesh | undefined;
  lastFrame: FrameData | undefined;
  disposed = false;
  readonly #lost: ((reason: string) => void)[] = [];

  resize(widthPx: number, heightPx: number): void {
    this.width = Math.max(1, Math.floor(widthPx));
    this.height = Math.max(1, Math.floor(heightPx));
  }

  setTerrainPatch(mesh: TerrainMesh): void {
    this.terrain = mesh;
  }

  render(frame: FrameData): void {
    if (this.disposed) return;
    this.frames += 1;
    this.lastFrame = frame;
  }

  finish(): Promise<void> {
    return Promise.resolve();
  }

  debugReadPixels(): Promise<Uint8Array> {
    const out = new Uint8Array(this.width * this.height * 4);
    const [r, g, b] = CLEAR_COLOUR.map((c) => Math.round(c * 255)) as [number, number, number];
    for (let i = 0; i < out.length; i += 4) {
      out[i] = r;
      out[i + 1] = g;
      out[i + 2] = b;
      out[i + 3] = 255;
    }
    return Promise.resolve(out);
  }

  onLost(listener: (reason: string) => void): void {
    this.#lost.push(listener);
  }

  /** Test hook: behave as if the device was lost. */
  simulateLoss(reason: string): void {
    for (const l of this.#lost) l(reason);
  }

  dispose(): void {
    this.disposed = true;
    this.terrain = undefined;
    this.lastFrame = undefined;
  }
}
