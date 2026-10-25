/**
 * The game's wiring: starts the Sim Worker and the renderer, connects the channels between them,
 * feeds the keyboard, mouse and pads into the input channel, reads the aircraft's state back for
 * the warning strip and the throttle, opens the menus, and attaches the debug overlay. Nothing
 * here is game logic; it only wires the parts together (the composition root's composition root).
 */
import { NullAudioBackend, resumeOnFirstGesture } from '@flying/audio';
import type { ActionId, PumpStats } from '@flying/input';
import { RenderClient, type PixelSummary } from '@flying/render';
import { SHADERS } from '@flying/render/shader-sources';
import type { RenderEvent, RenderStats } from '@flying/render/messages';
import { monotonicMs, type DeepFeatures, type Flags, type Kv } from '@flying/platform-web';
import {
  type AircraftStatus,
  makeInputChannel,
  makeSnapshotChannel,
  openInputSink,
  openSnapshotSource,
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
  ControlsStore,
  DebugOverlay,
  installOverlayHotkey,
  SettingsStore,
  t,
  ToastHost,
  WarningStrip,
  type CompatReport,
  type DemoMenu,
  type FatalSource,
  type FlowEvent,
  type OverlayModel,
  type StringKey,
} from '@flying/ui';
import { FlightControls } from './controls.ts';
import { MenuController } from './menus.ts';
import { coreContentUrl, runSelfTestWorker, selfTestRequest, wasmUrl } from './selftest.ts';
import { judgeSelfTest, goldenCheckpoint, SEED } from './selftest-judge.ts';
import { sessionFromFlags, sessionOverrides } from './session.ts';
import { StatusFeed } from './status-feed.ts';
import { Supervisor } from './supervisor.ts';

export interface DemoRoots {
  /** Where the canvas goes. */
  stage: HTMLElement;
  overlay: HTMLElement;
  toast: HTMLElement;
  /** The warning strip. */
  strip: HTMLElement;
  /** The pause menu and the binding editor. */
  menu: HTMLElement;
}

export interface DemoContext {
  flags: Flags;
  /** Where the settings and the control profiles are kept. */
  kv: Kv;
  /** A performance run: the scripted pilot flies, and the keyboard, mouse and pads are left alone. */
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
  /** The aircraft as the newest snapshot says (undefined before the first one). */
  status(): AircraftStatus | undefined;
  /** What the input pump is doing now. */
  input(): TestInput | undefined;
  /** Which menu is open. */
  menu(): DemoMenu;
}

export interface TestInput {
  mouseMode: string;
  context: string;
  profile: string;
  throttle: number;
  assistRequest: number;
  stats: PumpStats;
}

const MOUSE_LABELS: Record<string, StringKey> = {
  off: 'pause.mouse.off',
  aim: 'pause.mouse.aim',
  stick: 'pause.mouse.stick',
};
/** Indexed by `ASSIST_LEVEL`. */
const ASSIST_LABELS: (StringKey | undefined)[] = [
  undefined,
  'assist.full',
  'assist.standard',
  'assist.reduced',
  'assist.manual',
];

/** The strip is redrawn this often at most, milliseconds. */
const STRIP_MS = 100;

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

  // What the player chose last time, and the sortie it makes with the URL laid over it.
  const settings = new SettingsStore(ctx.kv);
  const controlsStore = new ControlsStore(ctx.kv);
  await Promise.all([settings.load(), controlsStore.load()]);
  /** A scripted pilot flies (a performance run, or the circuit): the devices are left alone. */
  const scripted = ctx.perf || flags.script !== false;
  const session = sessionFromFlags(flags, ctx.perf, settings.value);
  const overrides = scripted ? {} : sessionOverrides(session, settings.value);

  let latestSim: SimStats | undefined;
  let latestRender: RenderStats | undefined;
  /** Held by the person (the overlay, a test) or by the page being hidden. */
  let paused = false;
  /** Held because a menu is open. */
  let menuHold = false;
  let simHeld = false;
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
        script: ctx.perf
          ? INPUT_SCRIPT.ERRATIC
          : flags.script === 'circuit'
            ? INPUT_SCRIPT.CIRCUIT
            : INPUT_SCRIPT.NONE,
        session,
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

  /** The simulation runs unless the person, the page being hidden, or a menu holds it. */
  const syncHold = (): void => {
    const want = paused || menuHold;
    if (want === simHeld) return;
    simHeld = want;
    if (want) sim.pause();
    else sim.resume();
  };

  // Input: keyboard, mouse and pads -> profile -> shaping -> the channel the Sim Worker drains
  // each tick. A performance run flies the scripted pilot; the devices would fight it.
  const sink = openInputSink(inputs.sink);
  let controls: FlightControls | undefined;
  let menus: MenuController | undefined;
  let strip: WarningStrip | undefined;
  let feed: StatusFeed | undefined;

  /** What the pump hands to the page: the keys only the page can act on. */
  const onAction = (id: ActionId): void => {
    if (id === 'system.pause') {
      menus?.toggle();
    } else if (id === 'flight.mouse_mode' && controls && menus) {
      const mode = controls.pump.mouseMode;
      menus.persist({ mouseMode: mode });
      controls.syncMouseLock();
      toasts.show(t('toast.mouse-mode', { mode: t(MOUSE_LABELS[mode] ?? 'pause.mouse.off') }));
    } else if (id === 'flight.assist_cycle' && controls && menus) {
      const level = controls.pump.assistRequest;
      menus.persist({ assist: level });
      const label = ASSIST_LABELS[level];
      if (label !== undefined) toasts.show(t('toast.assist', { name: t(label) }));
    }
  };

  if (!scripted) {
    const c = new FlightControls({
      window,
      document,
      stage: roots.stage,
      sink,
      store: controlsStore,
      minIntervalMs: modes.inputChannel === 'shared' ? 8 : 33,
      onAction,
      onMouseWarning: () => {
        toasts.show(t('toast.mouse-raw'), 'warn');
      },
      onPointerLost: () => {
        if (menus && !menus.isOpen && c.pump.mouseMode !== 'off') menus.openPause();
      },
    });
    controls = c;
    menus = new MenuController({
      root: roots.menu,
      settings,
      store: controlsStore,
      pads: c.pads,
      pump: c.pump,
      overrides,
      assistInForce: () => feed?.status?.assist ?? 0,
      hold: (on) => {
        menuHold = on;
        syncHold();
      },
      setMenu: (menu) => {
        ctx.flow.dispatch({ type: 'menu', menu });
      },
      releaseMouse: () => {
        c.releaseMouse();
      },
      syncMouseLock: () => {
        c.syncMouseLock();
      },
      onProfile: (profile) => {
        c.useProfile(profile);
      },
      onSettings: (patch, next) => {
        c.applyChange(patch, next);
        if (patch.gfxStrength !== undefined) render.setSettings({ gfxStrength: patch.gfxStrength });
      },
      nowMs: monotonicMs,
    });
    c.start();
  }

  // The warning strip and the state the pump needs come from the aircraft's snapshot. A scripted
  // pilot (not a performance run) gets the strip too, so the circuit can be watched.
  if (!ctx.perf) {
    render.setSettings({ gfxStrength: settings.value.gfxStrength });
    const s = new WarningStrip(roots.strip);
    strip = s;
    let appliedStart = false;
    let lastStrip = Number.NEGATIVE_INFINITY;
    feed = new StatusFeed({
      source: modes.snapshotChannel === 'shared' ? openSnapshotSource(snapshots.source) : undefined,
      reported: () => latestSim?.status,
      requestFrame: (callback) => requestAnimationFrame(callback),
      cancelFrame: (id) => {
        cancelAnimationFrame(id);
      },
      onStatus: (status, observation) => {
        controls?.pump.observe(observation);
        if (!appliedStart && controls && menus) {
          // The mouse aim starts from where the nose points, so wait for the first snapshot.
          appliedStart = true;
          controls.applySettings(menus.settings());
        }
        const now = monotonicMs();
        if (now - lastStrip >= STRIP_MS) {
          lastStrip = now;
          s.update(status);
        }
      },
    });
    feed.start();
  }

  // Audio waits for a gesture; the null backend only records that it was asked.
  const audio = new NullAudioBackend();
  const cancelAudio = resumeOnFirstGesture(audio, window);

  /** Paused by the page being hidden (not by the person), so showing it again resumes. */
  let pausedByHiding = false;
  const setPaused = (value: boolean, quiet = false): void => {
    if (value === paused) return;
    paused = value;
    syncHold();
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
    paused: paused || menuHold,
    input: controls
      ? { sent: controls.pump.stats.sent, refused: controls.pump.stats.refused }
      : undefined,
    benchmark: ctx.benchmarkSummary,
    flight: feed?.status ?? latestSim?.status,
    latency:
      latestSim === undefined
        ? undefined
        : {
            control: latestSim.controlLatencyRun,
            poll: latestSim.pollLatencyRun,
            tick: latestSim.tickLatencyRun,
          },
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
      status: () => feed?.status ?? latestSim?.status,
      input: () =>
        controls === undefined
          ? undefined
          : {
              mouseMode: controls.pump.mouseMode,
              context: controls.pump.context,
              profile: controls.pump.profile.id,
              throttle: controls.pump.throttle,
              assistRequest: controls.pump.assistRequest,
              stats: controls.pump.stats,
            },
      menu: () => menus?.current ?? null,
    };
    (window as unknown as { __flying?: TestHooks }).__flying = hooks;
  }

  const stop = (): void => {
    delete roots.stage.dataset['state'];
    window.clearInterval(watchdog);
    removeHotkey();
    document.removeEventListener('visibilitychange', onVisibility);
    feed?.stop();
    controls?.destroy();
    menus?.destroy();
    strip?.destroy();
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
