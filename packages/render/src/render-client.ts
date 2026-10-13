/**
 * The main thread's side of rendering: owns the canvas element, decides where to render (the
 * Render Worker, or the main thread if workers cannot), keeps the drawing buffer at the element's
 * size in device pixels, and hands out a fresh canvas when a backend has spent the current one.
 */
import { scriptUrl } from '@flying/platform-web';
import type { Flags } from '@flying/platform-web/flags';
import {
  snapshotTransferables,
  type SnapshotEndpoint,
  type SnapshotSource,
  openSnapshotSource,
} from '@flying/sim-bridge';
import { MainThreadRenderHost } from './main-thread-host.ts';
import type { RenderCommand, RenderEvent, RenderInit } from './messages.ts';
import type { RendererFactories } from './render-session.ts';
import type { ShaderSources } from './renderer.ts';

/** The slice of `Worker` the client uses. */
export interface RenderWorkerLike {
  postMessage(message: unknown, transfer?: Transferable[]): void;
  terminate(): void;
  onmessage: ((event: { data: unknown }) => void) | null;
  onerror: ((event: { message?: string }) => void) | null;
}

export interface RenderClientOptions {
  container: HTMLElement;
  /** URL of `render.worker.ts` as built. */
  workerUrl: string;
  flags: Pick<Flags, 'renderer' | 'mainrender' | 'stall'>;
  snapshot: SnapshotEndpoint;
  /** The end of the terrain channel that comes here; the other end is with the Sim Worker. */
  terrainPort: MessagePort | null;
  onEvent: (event: RenderEvent) => void;
  /** Used only when rendering on the main thread. */
  shaders?: ShaderSources;
  factories?: RendererFactories;
  createWorker?: (url: string) => RenderWorkerLike;
  readyTimeoutMs?: number;
}

export interface PixelSummary {
  width: number;
  height: number;
  distinctColours: number;
  skyFraction: number;
}

function newCanvas(): HTMLCanvasElement {
  const c = document.createElement('canvas');
  c.dataset['testid'] = 'render-canvas';
  c.style.display = 'block';
  c.style.width = '100%';
  c.style.height = '100%';
  return c;
}

/** The element's size in device pixels. */
function devicePixelSize(
  entry: ResizeObserverEntry | undefined,
  el: HTMLElement,
): { width: number; height: number } {
  // Absent in browsers without device-pixel boxes, though the DOM typings say it is always there.
  const sizes: readonly ResizeObserverSize[] | undefined = entry?.devicePixelContentBoxSize;
  const dp = sizes?.[0];
  if (dp) return { width: Math.max(1, dp.inlineSize), height: Math.max(1, dp.blockSize) };
  const box = entry?.contentBoxSize[0];
  const [w, h] = box ? [box.inlineSize, box.blockSize] : [el.clientWidth, el.clientHeight];
  return {
    width: Math.max(1, Math.round(w * devicePixelRatio)),
    height: Math.max(1, Math.round(h * devicePixelRatio)),
  };
}

export class RenderClient {
  readonly host: 'worker' | 'main';
  #worker: RenderWorkerLike | undefined;
  #mainHost: MainThreadRenderHost | undefined;
  #canvas: HTMLCanvasElement;
  #observer: ResizeObserver | undefined;
  #pending = new Map<number, (s: PixelSummary) => void>();
  #nextId = 1;
  #opts: RenderClientOptions;
  #size = { width: 1, height: 1 };

  private constructor(
    opts: RenderClientOptions,
    host: 'worker' | 'main',
    canvas: HTMLCanvasElement,
  ) {
    this.#opts = opts;
    this.host = host;
    this.#canvas = canvas;
  }

  /** Starts rendering and resolves once a backend is up. Rejects with `SYS-103` if none starts. */
  static async create(opts: RenderClientOptions): Promise<RenderClient> {
    const canTransfer =
      typeof OffscreenCanvas !== 'undefined' &&
      'transferControlToOffscreen' in HTMLCanvasElement.prototype;
    const useWorker = canTransfer && !opts.flags.mainrender;
    const startSize = devicePixelSize(undefined, opts.container);

    if (useWorker) {
      const attempt = await RenderClient.#startWorker(opts, startSize);
      if (attempt !== 'need-main-thread') {
        return attempt;
      }
      // The canvas was transferred to a worker that cannot draw; the main thread starts with a new one.
    }
    return RenderClient.#startMain(opts, startSize);
  }

  static async #startWorker(
    opts: RenderClientOptions,
    size: { width: number; height: number },
  ): Promise<RenderClient | 'need-main-thread'> {
    const canvas = newCanvas();
    opts.container.append(canvas);
    const offscreen = canvas.transferControlToOffscreen();
    const worker =
      opts.createWorker?.(opts.workerUrl) ??
      (new Worker(scriptUrl(opts.workerUrl), { type: 'module' }) as unknown as RenderWorkerLike);
    const client = new RenderClient(opts, 'worker', canvas);
    client.#worker = worker;
    client.#size = size;

    const outcome = await new Promise<'ready' | 'need-main-thread'>((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error('SYS-106: the Render Worker did not become ready in time'));
      }, opts.readyTimeoutMs ?? 15_000);
      worker.onmessage = (event) => {
        const e = event.data as RenderEvent;
        if (e.type === 'ready') {
          clearTimeout(timer);
          opts.onEvent(e);
          resolve('ready');
        } else if (e.type === 'need-main-thread') {
          clearTimeout(timer);
          resolve('need-main-thread');
        } else if (e.type === 'error') {
          clearTimeout(timer);
          reject(new Error(e.message));
        } else {
          client.#receive(e);
        }
      };
      worker.onerror = (event) => {
        clearTimeout(timer);
        reject(
          new Error(
            `SYS-107: the Render Worker failed to start (${event.message ?? 'unknown error'})`,
          ),
        );
      };
      const init: RenderInit = {
        type: 'init',
        canvas: offscreen,
        renderer: opts.flags.renderer,
        snapshot: opts.snapshot,
        terrainPort: opts.terrainPort,
        width: size.width,
        height: size.height,
        allowStall: opts.flags.stall,
      };
      const transfer: Transferable[] = [offscreen, ...snapshotTransferables(opts.snapshot)];
      if (opts.terrainPort) transfer.push(opts.terrainPort);
      worker.postMessage(init, transfer);
    }).catch((e: unknown) => {
      worker.terminate();
      canvas.remove();
      throw e;
    });

    if (outcome === 'need-main-thread') {
      worker.terminate();
      canvas.remove();
      return 'need-main-thread';
    }
    worker.onmessage = (event) => {
      client.#receive(event.data as RenderEvent);
    };
    client.#observe();
    return client;
  }

  static async #startMain(
    opts: RenderClientOptions,
    size: { width: number; height: number },
  ): Promise<RenderClient> {
    if (!opts.shaders) {
      throw new Error('SYS-108: rendering on the main thread needs shader sources');
    }
    const source: SnapshotSource = openSnapshotSource(opts.snapshot);
    const mainHost = await MainThreadRenderHost.create(
      {
        container: opts.container,
        choice: opts.flags.renderer,
        shaders: opts.shaders,
        source,
        emit: (e) => {
          opts.onEvent(e);
        },
        makeCanvas: newCanvas,
        ...(opts.factories ? { factories: opts.factories } : {}),
      },
      size.width,
      size.height,
    );
    const client = new RenderClient(opts, 'main', mainHost.canvas);
    client.#mainHost = mainHost;
    client.#size = size;
    if (opts.terrainPort) {
      opts.terrainPort.onmessage = (event: MessageEvent<{ type: string; patch: ArrayBuffer }>) => {
        if (event.data.type === 'terrain') mainHost.session.setTerrain(event.data.patch);
      };
    }
    client.#observe();
    return client;
  }

  get canvas(): HTMLCanvasElement {
    return this.#mainHost?.canvas ?? this.#canvas;
  }

  #observe(): void {
    const el = this.#opts.container;
    this.#observer = new ResizeObserver((entries) => {
      const entry = entries.find((e) => e.target === this.#canvas || e.target === el) ?? entries[0];
      this.#resize(devicePixelSize(entry, el));
    });
    // The canvas's own box is what the picture fills, so that is what to measure.
    try {
      this.#observer.observe(this.#canvas, { box: 'device-pixel-content-box' });
    } catch {
      this.#observer.observe(this.#canvas);
    }
  }

  #resize(size: { width: number; height: number }): void {
    if (size.width === this.#size.width && size.height === this.#size.height) return;
    this.#size = size;
    if (this.#worker) {
      this.#send({ type: 'resize', width: size.width, height: size.height });
    } else {
      this.#mainHost?.session.resize(size.width, size.height);
    }
  }

  #receive(event: RenderEvent): void {
    if (event.type === 'need-canvas') {
      this.#replaceCanvas();
      return;
    }
    if (event.type === 'pixels') {
      const resolve = this.#pending.get(event.id);
      this.#pending.delete(event.id);
      resolve?.({
        width: event.width,
        height: event.height,
        distinctColours: event.distinctColours,
        skyFraction: event.skyFraction,
      });
      return;
    }
    this.#opts.onEvent(event);
  }

  /** A backend spent the canvas on the wrong context type: swap in a new one and send it over. */
  #replaceCanvas(): void {
    const next = newCanvas();
    this.#canvas.replaceWith(next);
    this.#observer?.unobserve(this.#canvas);
    this.#canvas = next;
    const offscreen = next.transferControlToOffscreen();
    this.#send({ type: 'canvas', canvas: offscreen }, [offscreen]);
    try {
      this.#observer?.observe(next, { box: 'device-pixel-content-box' });
    } catch {
      this.#observer?.observe(next);
    }
  }

  #send(command: RenderCommand, transfer: Transferable[] = []): void {
    this.#worker?.postMessage(command, transfer);
  }

  /** Reads the last frame back and summarises it (distinct colours, share of sky). */
  readPixels(): Promise<PixelSummary> {
    if (this.#mainHost) {
      return this.#mainHost.session.pixelSummary();
    }
    const id = this.#nextId++;
    return new Promise<PixelSummary>((resolve) => {
      this.#pending.set(id, resolve);
      this.#send({ type: 'read-pixels', id });
    });
  }

  resetRun(): void {
    if (this.#worker) this.#send({ type: 'reset-run' });
    else this.#mainHost?.session.resetRun();
  }

  /** Blocks the renderer's thread for `ms` (test hook; ignored unless the flags allowed it). */
  stall(ms: number): void {
    if (this.#worker) {
      this.#send({ type: 'stall', ms });
    } else if (this.#opts.flags.stall) {
      const until = performance.now() + ms;
      while (performance.now() < until) {
        // Deliberately busy: a stall of the thread that is also the main thread.
      }
    }
  }

  stop(): void {
    this.#observer?.disconnect();
    if (this.#worker) {
      this.#send({ type: 'stop' });
      this.#worker.terminate();
    }
    this.#mainHost?.stop();
    this.#canvas.remove();
  }
}
