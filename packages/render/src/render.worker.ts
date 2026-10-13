/**
 * The Render Worker: draws the newest snapshots onto an `OffscreenCanvas`, on its own schedule
 * (`requestAnimationFrame` in the worker) and independent of the main thread and the Sim Worker
 * (ADR-17). If a main-thread stall or a sim stall happens, frames keep coming.
 *
 * If this worker cannot render at all (no `requestAnimationFrame` here), it says so before touching
 * the canvas and the main thread draws instead.
 */
import { monotonicMs, sharedNowMs } from '@flying/platform-web';
import { openSnapshotSource } from '@flying/sim-bridge';
import type { RenderCommand, RenderEvent, RenderInit } from './messages.ts';
import { RenderSession, type CanvasSource } from './render-session.ts';
import { SHADERS } from './shader-sources.ts';

interface WorkerScope {
  onmessage: ((event: MessageEvent<RenderCommand>) => void) | null;
  postMessage(message: RenderEvent): void;
  close(): void;
  requestAnimationFrame?: (callback: () => void) => number;
}
const scope = self as unknown as WorkerScope;

let session: RenderSession | undefined;
let allowStall = false;
let stopped = false;
let canvas: OffscreenCanvas | undefined;
let waitingForCanvas: ((c: OffscreenCanvas) => void) | undefined;

function post(event: RenderEvent): void {
  scope.postMessage(event);
}

scope.onmessage = (event) => {
  const command = event.data;
  void handle(command).catch((e: unknown) => {
    post({ type: 'error', message: e instanceof Error ? e.message : String(e) });
  });
};

async function handle(command: RenderCommand): Promise<void> {
  switch (command.type) {
    case 'init':
      await init(command);
      break;
    case 'canvas':
      canvas = command.canvas;
      waitingForCanvas?.(command.canvas);
      waitingForCanvas = undefined;
      break;
    case 'resize':
      session?.resize(command.width, command.height);
      break;
    case 'read-pixels': {
      const summary = (await session?.pixelSummary()) ?? {
        width: 0,
        height: 0,
        distinctColours: 0,
        skyFraction: 0,
      };
      post({ type: 'pixels', id: command.id, ...summary });
      break;
    }
    case 'reset-run':
      session?.resetRun();
      break;
    case 'stall':
      if (allowStall) {
        const until = monotonicMs() + command.ms;
        while (monotonicMs() < until) {
          // Deliberately busy: this is the stall the independence tests inject.
        }
      }
      break;
    case 'stop':
      stopped = true;
      session?.stop();
      scope.close();
      break;
  }
}

async function init(cmd: RenderInit): Promise<void> {
  if (typeof scope.requestAnimationFrame !== 'function') {
    post({ type: 'need-main-thread', reason: 'this worker has no requestAnimationFrame' });
    return;
  }
  allowStall = cmd.allowStall;
  canvas = cmd.canvas;
  const canvases: CanvasSource = {
    current: () => {
      if (canvas === undefined) throw new Error('SYS-105: no canvas');
      return canvas;
    },
    fresh: (reason) =>
      new Promise<OffscreenCanvas>((resolve) => {
        waitingForCanvas = resolve;
        post({ type: 'need-canvas', reason });
      }),
  };
  const source = openSnapshotSource(cmd.snapshot);
  const s = new RenderSession({
    choice: cmd.renderer,
    canvases,
    shaders: SHADERS,
    source,
    emit: post,
    sharedNowMs,
    monoMs: monotonicMs,
  });

  // The terrain patch may arrive before the renderer exists; hold it until it does.
  let patch: ArrayBuffer | undefined;
  if (cmd.terrainPort !== null) {
    cmd.terrainPort.onmessage = (event: MessageEvent<{ type: string; patch: ArrayBuffer }>) => {
      if (event.data.type !== 'terrain') return;
      if (session) session.setTerrain(event.data.patch);
      else patch = event.data.patch;
    };
  }
  await s.start(cmd.width, cmd.height);
  session = s;
  if (patch !== undefined) s.setTerrain(patch);

  const tick = (): void => {
    if (stopped) return;
    s.frame();
    scope.requestAnimationFrame?.(tick);
  };
  scope.requestAnimationFrame(tick);
}
