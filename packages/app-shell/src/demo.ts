/**
 * The v0.0b demo: the empty loop. Starts the Sim Worker and the renderer, connects the channels
 * between them, feeds the keyboard and gamepad into the input channel, and attaches the debug
 * overlay. Nothing here is game logic; it only wires the parts together (the composition root's
 * composition root).
 */
import { NullAudioBackend, resumeOnFirstGesture } from '@flying/audio';
import {
  GamepadHub,
  InputPump,
  KeyboardState,
  attachInput,
  builtinProfiles,
  chooseProfile,
} from '@flying/input';
import { RenderClient, type PixelSummary } from '@flying/render';
import { SHADERS } from '@flying/render/shader-sources';
import type { RenderEvent, RenderStats } from '@flying/render/messages';
import { monotonicMs, sharedNowMs, type DeepFeatures, type Flags } from '@flying/platform-web';
import {
  makeInputChannel,
  makeSnapshotChannel,
  openInputSink,
  type SimStats,
} from '@flying/sim-bridge';
import { INPUT_SCRIPT } from '@flying/sim-bridge/layout';
import { createWakeWord, SimClient } from '@flying/sim-host';
import {
  browserMachineEnv,
  describeMachine,
  type BuildInfo,
  type MachineInfo,
  type PerfTarget,
  type RunModes,
} from '@flying/perf';
import simWorkerUrl from '@flying/sim-host/sim.worker?worker&url';
import renderWorkerUrl from '@flying/render/render.worker?worker&url';
import {
  DebugOverlay,
  installOverlayHotkey,
  isOverlayChord,
  t,
  ToastHost,
  type CompatReport,
  type FatalSource,
  type FlowEvent,
  type OverlayModel,
} from '@flying/ui';
import { coreContentUrl, runSelfTestWorker, selfTestRequest, wasmUrl } from './selftest.ts';
import { judgeSelfTest, goldenCheckpoint, SEED } from './selftest-judge.ts';
import { sessionFromFlags } from './session.ts';
import { Supervisor } from './supervisor.ts';

export interface DemoRoots {
  /** Where the canvas goes. */
  stage: HTMLElement;
  overlay: HTMLElement;
  toast: HTMLElement;
}

export interface DemoContext {
  flags: Flags;
  /** A performance run: the scripted pilot flies, and the keyboard and gamepad are left alone. */
  perf: boolean;
  /** The stored benchmark result as `profile / preset`, for the overlay. */
  benchmarkSummary: string | undefined;
  features: DeepFeatures;
  report: CompatReport;
  roots: DemoRoots;
  flow: { dispatch(event: FlowEvent): void };
  /** A part died: show the fatal screen. */
  fail(source: FatalSource, code: string, detail: string): void;
}

export interface Demo {
  stop(): void;
  /** What the performance runner needs to measure this demo. */
  perf: {
    target: PerfTarget;
    build(): BuildInfo;
    machine(): MachineInfo;
    modes(): RunModes;
  };
}

/** What tests may reach through `window.__flying` (with `?debug=` or `?flags=stall`). */
export interface TestHooks {
  stallSim(ms: number): void;
  stallRender(ms: number): void;
  pause(): void;
  resume(): void;
  resetRun(): void;
  readPixels(): Promise<PixelSummary>;
  stats(): { sim: SimStats | undefined; render: RenderStats | undefined };
  host(): 'worker' | 'main';
  backend(): string;
}

/** The first `SYS-nnn` in a message, or `fallback`. */
export function codeOf(message: string, fallback: string): string {
  return /SYS-\d{3}/.exec(message)?.[0] ?? fallback;
}

export async function startDemo(ctx: DemoContext): Promise<Demo> {
  const { flags, features, report, roots } = ctx;
  const modes = report.modes;
  if (modes.variant === undefined) {
    throw new Error('SYS-101: no WebAssembly build can run in this browser');
  }
  const variant = modes.variant;

  const toasts = new ToastHost(roots.toast);
  const snapshots = makeSnapshotChannel(modes.snapshotChannel);
  const inputs = makeInputChannel(modes.inputChannel);
  const terrain = new MessageChannel();

  let latestSim: SimStats | undefined;
  let latestRender: RenderStats | undefined;
  let paused = false;
  let readyEvents = 0;

  const onRenderEvent = (event: RenderEvent): void => {
    switch (event.type) {
      case 'stats':
        latestRender = event.stats;
        break;
      case 'ready':
        readyEvents += 1;
        if (readyEvents > 1) {
          toasts.show(t('toast.renderer-fallback', { backend: event.backend }), 'warn');
        }
        break;
      case 'lost':
        toasts.show(t('toast.renderer-lost', { reason: event.reason }), 'warn');
        break;
      case 'error':
        ctx.fail('render-worker', codeOf(event.message, 'SYS-107'), event.message);
        break;
      default:
        break;
    }
  };

  const [sim, render] = await Promise.all([
    SimClient.create({
      workerUrl: simWorkerUrl,
      init: {
        variant,
        wasmUrl: wasmUrl(variant),
        threadsInitialPages: __THREADS_INITIAL_PAGES__,
        contentUrl: coreContentUrl,
        seed: SEED,
        script: ctx.perf ? INPUT_SCRIPT.ERRATIC : INPUT_SCRIPT.NONE,
        session: sessionFromFlags(flags, ctx.perf),
        snapshot: snapshots.sink,
        input: inputs.source,
        terrainPort: terrain.port1,
        wake: modes.sleeper === 'atomics' ? createWakeWord() : null,
        forceTimer: flags.timer,
        profiling: false,
        allowStall: flags.stall,
      },
      onStats: (stats) => {
        latestSim = stats;
      },
      onError: (message) => {
        ctx.fail('sim-worker', codeOf(message, 'SYS-032'), message);
      },
    }),
    RenderClient.create({
      container: roots.stage,
      workerUrl: renderWorkerUrl,
      flags,
      snapshot: snapshots.source,
      terrainPort: terrain.port2,
      onEvent: onRenderEvent,
      shaders: SHADERS,
    }),
  ]);

  // Input: keyboard and gamepad -> shaping -> the channel the Sim Worker drains each tick.
  const sink = openInputSink(inputs.sink);
  const keyboard = new KeyboardState();
  const pads = new GamepadHub();
  const profiles = builtinProfiles();
  const pump = new InputPump({
    sink,
    keyboard,
    pads,
    nowMs: monotonicMs,
    sharedNowMs,
    minIntervalMs: modes.inputChannel === 'shared' ? 8 : 33,
  });
  // The profile follows the devices that are connected (a device plugged in or out re-binds).
  let profileRevision = -1;
  const followDevices = (): void => {
    pads.poll();
    if (pads.revision === profileRevision) return;
    profileRevision = pads.revision;
    const chosen = chooseProfile(profiles, pads.deviceNames());
    if (chosen.id !== pump.profile.id) pump.setProfile(chosen);
  };
  followDevices();
  const watchDevices = window.setInterval(followDevices, 250);
  // A performance run flies the scripted pilot; frames from the keyboard would fight it.
  // The overlay's Ctrl+Shift+D chord is the page's, not a roll command.
  const detachInput = ctx.perf
    ? () => undefined
    : attachInput(
        { window, document },
        {
          keyboard,
          pump,
          swallow: (e) =>
            isOverlayChord({
              code: e.code,
              ctrlKey: e.ctrlKey === true,
              shiftKey: e.shiftKey === true,
              altKey: e.altKey === true,
              metaKey: e.metaKey === true,
              repeat: e.repeat,
            }),
        },
      );

  // Audio waits for a gesture; the null backend only records that it was asked.
  const audio = new NullAudioBackend();
  const cancelAudio = resumeOnFirstGesture(audio, window);

  /** Paused by the page being hidden (not by the person), so showing it again resumes. */
  let pausedByHiding = false;
  const setPaused = (value: boolean, quiet = false): void => {
    if (value === paused) return;
    paused = value;
    if (value) sim.pause();
    else sim.resume();
    ctx.flow.dispatch({ type: value ? 'pause' : 'resume' });
    if (!quiet) toasts.show(t(value ? 'toast.paused' : 'toast.resumed'));
  };
  // A hidden tab pauses the sim, so nothing runs where nobody can see it (GDD 5.6, single-player).
  const onVisibility = (): void => {
    if (document.visibilityState === 'hidden') {
      if (!paused) {
        pausedByHiding = true;
        setPaused(true, true);
      }
    } else if (pausedByHiding) {
      pausedByHiding = false;
      setPaused(false, true);
    }
  };
  document.addEventListener('visibilitychange', onVisibility);

  const model = (): OverlayModel => ({
    build: __BUILD_ID__,
    variant: sim.variant,
    snapshotChannel: modes.snapshotChannel,
    renderHost: render.host,
    sleeper: sim.ready.sleeper,
    abi: sim.ready.abi,
    contentHash: sim.ready.contentHash,
    adapter: features.adapterInfo,
    sim: latestSim,
    render: latestRender,
    paused,
    input: { sent: pump.stats.sent, refused: pump.stats.refused },
    benchmark: ctx.benchmarkSummary,
    // Read from the snapshot and the control-latency stats by the wiring work package.
    flight: undefined,
    latency: undefined,
  });

  const overlay = new DebugOverlay(roots.overlay, {
    model,
    actions: {
      hash: () => sim.requestHash(),
      selfTest: async () =>
        judgeSelfTest(await runSelfTestWorker(selfTestRequest(variant)), goldenCheckpoint()),
      benchmark: () => {
        // Its own page, so nothing else is running while it measures.
        const next = new URL(location.href);
        next.searchParams.set('bench', 'full');
        next.searchParams.delete('perf');
        location.assign(next);
        return Promise.resolve('Starting the benchmark…');
      },
      togglePause: () => {
        pausedByHiding = false;
        setPaused(!paused);
      },
    },
    onVisibleChange: (visible) => {
      // The core times its stages only while someone is looking.
      sim.setProfiling(visible);
    },
  });
  const removeHotkey = installOverlayHotkey(window, overlay);

  const supervisor = new Supervisor({
    lastReportAt: () => sim.lastStatsAt,
    now: monotonicMs,
    onSilent: (seconds) => {
      toasts.show(t('toast.sim-silent', { seconds }), 'warn');
    },
  });
  const watchdog = window.setInterval(() => {
    supervisor.check();
  }, 1000);

  sim.start();
  if (flags.debug !== false) overlay.show();
  // Tests (and the curious) can tell that everything is wired and the loop is going.
  roots.stage.dataset['state'] = 'running';

  if (flags.debug !== false || flags.stall) {
    const hooks: TestHooks = {
      stallSim: (ms) => {
        sim.stall(ms);
      },
      stallRender: (ms) => {
        render.stall(ms);
      },
      pause: () => {
        pausedByHiding = false;
        setPaused(true);
      },
      resume: () => {
        pausedByHiding = false;
        setPaused(false);
      },
      resetRun: () => {
        sim.resetRun();
        render.resetRun();
      },
      readPixels: () => render.readPixels(),
      stats: () => ({ sim: latestSim, render: latestRender }),
      host: () => render.host,
      backend: () => latestRender?.backend ?? '',
    };
    (window as unknown as { __flying?: TestHooks }).__flying = hooks;
  }

  const stop = (): void => {
    delete roots.stage.dataset['state'];
    window.clearInterval(watchdog);
    removeHotkey();
    document.removeEventListener('visibilitychange', onVisibility);
    detachInput();
    window.clearInterval(watchDevices);
    cancelAudio();
    overlay.destroy();
    sink.close();
    sim.stop();
    render.stop();
    void audio.close();
  };
  window.addEventListener('pagehide', stop, { once: true });

  const rendererName = (): string =>
    latestRender === undefined ? '' : `${latestRender.backend} ${latestRender.description}`;
  return {
    stop,
    perf: {
      target: {
        stats: () => ({ sim: latestSim, render: latestRender }),
        resetRun: () => {
          sim.resetRun();
          render.resetRun();
        },
      },
      build: () => ({
        id: __BUILD_ID__,
        abi: sim.ready.abi,
        contentHash: sim.ready.contentHash,
      }),
      machine: () => describeMachine(browserMachineEnv(), features.adapterInfo, rendererName()),
      modes: () => ({
        variant: sim.variant,
        snapshotChannel: modes.snapshotChannel,
        inputChannel: modes.inputChannel,
        renderHost: render.host,
        sleeper: sim.ready.sleeper,
        renderer: latestRender?.backend ?? modes.renderer,
      }),
    },
  };
}
