/**
 * The renderer on the main thread, for browsers whose workers cannot render (no `OffscreenCanvas`
 * transfer, or no `requestAnimationFrame` in workers) and for the `mainrender` test flag. It is the
 * same session and loop as the Render Worker's, scheduled by the window's `requestAnimationFrame`;
 * the cost is that a busy main thread now delays frames.
 */
import { monotonicMs, sharedNowMs } from '@flying/platform-web';
import type { RendererChoice } from '@flying/platform-web/flags';
import type { SnapshotSource } from '@flying/sim-bridge';
import type { RenderEvent } from './messages.ts';
import { RenderSession, type CanvasSource, type RendererFactories } from './render-session.ts';
import type { ShaderSources } from './renderer.ts';

export interface MainThreadHostOptions {
  /** The element a canvas lives in; the host creates and replaces canvases there. */
  container: HTMLElement;
  choice: RendererChoice;
  shaders: ShaderSources;
  source: SnapshotSource;
  emit: (event: RenderEvent) => void;
  makeCanvas: () => HTMLCanvasElement;
  factories?: RendererFactories;
  log?: (line: string) => void;
  /** Replaces `window.requestAnimationFrame`, for tests. */
  requestFrame?: (callback: () => void) => number;
}

export class MainThreadRenderHost {
  readonly session: RenderSession;
  #canvas: HTMLCanvasElement;
  #stopped = false;

  private constructor(session: RenderSession, canvas: HTMLCanvasElement) {
    this.session = session;
    this.#canvas = canvas;
  }

  static async create(
    opts: MainThreadHostOptions,
    widthPx: number,
    heightPx: number,
  ): Promise<MainThreadRenderHost> {
    let current = opts.makeCanvas();
    opts.container.append(current);
    const canvases: CanvasSource = {
      current: () => current,
      fresh: (reason) => {
        opts.log?.(`new canvas: ${reason}`);
        const next = opts.makeCanvas();
        current.replaceWith(next);
        current = next;
        host.#canvas = next;
        return Promise.resolve(next);
      },
    };
    const session = new RenderSession({
      choice: opts.choice,
      canvases,
      shaders: opts.shaders,
      source: opts.source,
      emit: opts.emit,
      sharedNowMs,
      monoMs: monotonicMs,
      ...(opts.factories ? { factories: opts.factories } : {}),
      ...(opts.log ? { log: opts.log } : {}),
    });
    const host = new MainThreadRenderHost(session, current);
    await session.start(widthPx, heightPx);
    const request = opts.requestFrame ?? ((cb: () => void) => requestAnimationFrame(cb));
    const tick = (): void => {
      if (host.#stopped) return;
      session.frame();
      request(tick);
    };
    request(tick);
    return host;
  }

  get canvas(): HTMLCanvasElement {
    return this.#canvas;
  }

  stop(): void {
    this.#stopped = true;
    this.session.stop();
  }
}
