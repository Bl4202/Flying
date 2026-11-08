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
import type { ViewState } from '@flying/sim-bridge/view-channel';
import type { CockpitView } from './camera-rig.ts';
import type { RenderPresentation } from './present.ts';
import type { RenderCommand, RenderEvent, RenderInit, RenderSettings } from './messages.ts';
import { RenderSession, type CanvasSource } from './render-session.ts';
import { SHADERS } from './shader-sources.ts';

interface WorkerScope {
  onmessage: ((event: MessageEvent<RenderCommand>) => void) | null;
  postMessage(message: RenderEvent, transfer?: Transferable[]): void;
  close(): void;
  requestAnimationFrame?: (callback: () => void) => number;
}
const scope = self as unknown as WorkerScope;

/** A terrain level from the Sim Worker. */
interface TerrainMessage {
  type: string;
  level: number;
  patch: ArrayBuffer;
}

let session: RenderSession | undefined;
let allowStall = false;
let stopped = false;
let canvas: OffscreenCanvas | undefined;
/** Settings that arrive before the session exists are applied to it when it does. */
let settings: RenderSettings | undefined;
/** The view and the cockpit that arrive before the session exists. */
let pendingView: ViewState | undefined;
let pendingCockpit: CockpitView | undefined;
let pendingPresentation: RenderPresentation | undefined;
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
    case 'settings':
      settings = {
        gfxStrength: command.gfxStrength,
        ...(command.sampleCount === undefined ? {} : { sampleCount: command.sampleCount }),
        ...(command.debugView === undefined ? {} : { debugView: command.debugView }),
        ...(command.fault === undefined ? {} : { fault: command.fault }),
        ...(command.hudPalette === undefined ? {} : { hudPalette: command.hudPalette }),
        ...(command.hudOverlay === undefined ? {} : { hudOverlay: command.hudOverlay }),
        ...(command.hudScale === undefined ? {} : { hudScale: command.hudScale }),
        ...(command.hudTexts === undefined ? {} : { hudTexts: command.hudTexts }),
        ...(command.debugFovDeg === undefined ? {} : { debugFovDeg: command.debugFovDeg }),
      };
      session?.setSettings(settings);
      break;
    case 'view':
      if (session) session.setView(command.state);
      else pendingView = command.state;
      break;
    case 'cockpit':
      if (session) session.setCockpit(command.cockpit);
      else pendingCockpit = command.cockpit;
      break;
    case 'presentation':
      if (session) session.setPresentation(command.presentation);
      else pendingPresentation = command.presentation;
      break;
    case 'read-pixels': {
      const summary = (await session?.pixelSummary()) ?? {
        width: 0,
        height: 0,
        distinctColours: 0,
        skyFraction: 0,
        meanLuma: 0,
      };
      post({ type: 'pixels', id: command.id, ...summary });
      break;
    }
    case 'read-image': {
      const image = (await session?.imageData()) ?? {
        width: 0,
        height: 0,
        pixels: new Uint8Array(0),
      };
      const pixels = image.pixels.buffer as ArrayBuffer;
      scope.postMessage(
        {
          type: 'image',
          id: command.id,
          width: image.width,
          height: image.height,
          pixels,
        } satisfies RenderEvent,
        [pixels],
      );
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

  // Terrain levels may arrive before the renderer exists; hold them until it does.
  const pending: { level: number; patch: ArrayBuffer }[] = [];
  if (cmd.terrainPort !== null) {
    cmd.terrainPort.onmessage = (event: MessageEvent<TerrainMessage>) => {
      if (event.data.type !== 'terrain') return;
      if (session) session.setTerrain(event.data.level, event.data.patch);
      else pending.push({ level: event.data.level, patch: event.data.patch });
    };
  }
  if (settings) s.setSettings(settings);
  if (cmd.viewBuffer) s.setViewBuffer(cmd.viewBuffer);
  if (pendingView) s.setView(pendingView);
  if (pendingCockpit) s.setCockpit(pendingCockpit);
  if (pendingPresentation) s.setPresentation(pendingPresentation);
  await s.start(cmd.width, cmd.height);
  session = s;
  for (const { level, patch } of pending.splice(0)) s.setTerrain(level, patch);

  const tick = (): void => {
    if (stopped) return;
    s.frame();
    scope.requestAnimationFrame?.(tick);
  };
  scope.requestAnimationFrame(tick);
}
