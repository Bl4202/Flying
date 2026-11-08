/**
 * The game's wiring: starts the Sim Worker and the renderer, connects the channels between them,
 * feeds the keyboard, mouse and pads into the input channel, reads the aircraft's state back for
 * the warning strip and the throttle, opens the menus, and attaches the debug overlay. Nothing
 * here is game logic; it only wires the parts together (the composition root's composition root).
 */
import {
  NullAudioBackend,
  resumeOnFirstGesture,
  WebAudioBackend,
  webAudioSupported,
  type FlightAudio,
  type ToneOnset,
} from '@flying/audio';
import flightWorkletUrl from '@flying/audio/flight-processor?worker&url';
import { ViewInput, type ActionId, type PumpStats, type ViewSettings } from '@flying/input';
import { RenderClient, type PixelSummary } from '@flying/render';
import { SHADERS } from '@flying/render/shader-sources';
import type { RenderEvent, RenderStats } from '@flying/render/messages';
import {
  defaultFlags,
  monotonicMs,
  scriptUrl,
  type DeepFeatures,
  type Flags,
  type InstallPrompt,
  type Kv,
  type UpdateWatcher,
} from '@flying/platform-web';
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
  type Scenario,
} from '@flying/perf';
import simWorkerUrl from '@flying/sim-host/sim.worker?worker&url';
import renderWorkerUrl from '@flying/render/render.worker?worker&url';
import {
  ControlsStore,
  DebugOverlay,
  installOverlayHotkey,
  t,
  ToastHost,
  WarningStrip,
  type CompatReport,
  type DemoMenu,
  type FatalSource,
  type FlowEvent,
  type FlowState,
  type OverlayModel,
  type Settings,
  type SettingsPatch,
  type SettingsStore,
  type StartTarget,
  type StringKey,
} from '@flying/ui';
import { FlightControls } from './controls.ts';
import { MenuController } from './menus.ts';
import { loadPresentation } from './presentation.ts';
import {
  busesOf,
  qualityOf,
  renderSettingsOf,
  viewSettingsOf,
  type BenchmarkPreset,
} from './present-settings.ts';
import { loadPresentBundle } from './present-bundle.ts';
import { kindOfMapping, ScreenController } from './screens.ts';
import { coreContentUrl, runSelfTestWorker, selfTestRequest, wasmUrl } from './selftest.ts';
import { judgeSelfTest, goldenCheckpoint, SEED } from './selftest-judge.ts';
import { countLongTasks } from './long-tasks.ts';
import { sessionFromFlags, sessionOverrides } from './session.ts';
import { StatusFeed } from './status-feed.ts';
import { Supervisor } from './supervisor.ts';
import { makeViewBuffer, ViewBridge } from './view-bridge.ts';

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
  /** The player's settings, already loaded. */
  settings: SettingsStore;
  /** The browser's offer to install the game, if it makes one. */
  install: InstallPrompt;
  /** A newer build waiting to take over; undefined without a Service Worker (development). */
  update: UpdateWatcher | undefined;
  /** Where the visit starts: the first run, the main menu, or the flight (`?fly=1`, a scripted run). */
  start: StartTarget;
  /** A performance run: the scripted pilot flies, and the keyboard, mouse and pads are left alone. */
  perf: boolean;
  /** The scenario a performance run flies (its start, its script and what it shows). */
  scenario: Scenario | undefined;
  /** The stored benchmark result as `profile / preset`, for the overlay. */
  benchmarkSummary: string | undefined;
  /** The preset the first-run benchmark chose, if it has run. */
  benchmarkPreset: BenchmarkPreset | undefined;
  features: DeepFeatures;
  report: CompatReport;
  roots: DemoRoots;
  flow: { dispatch(event: FlowEvent): FlowState; state(): FlowState };
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
  /** The application state's name (`MAIN_MENU`, `DEMO`, ...). */
  flow(): string;
  /** The sound: which backend, whether it runs, the output level, and the warning tones heard. */
  audio(): TestAudio;
  /** What the view is doing: the camera, the HUD's detail and brightness. */
  view(): {
    camera: string;
    declutter: number;
    brightness: number;
    fovDeg: number;
    lookYawRate: number;
    lookPitchRate: number;
    zoom: boolean;
    freeLook: boolean;
  };
  /** Why the aircraft's presentation could not be loaded, if it could not. */
  presentationError(): string | undefined;
}

export interface TestAudio {
  kind: string;
  state: string;
  levelDb: number;
  paramWrites: number;
  workletError: string | undefined;
  onsets: number;
  lastOnset: ToneOnset | undefined;
  muted: boolean;
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

/** Gives the player `text` as a file to keep. */
function downloadText(name: string, text: string): void {
  const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.append(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
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
  const settings = ctx.settings;
  const controlsStore = new ControlsStore(ctx.kv);
  await controlsStore.load();
  /** A scripted pilot flies (a performance run, or the circuit): the devices are left alone. */
  const scripted = ctx.perf || flags.script !== false;
  const session = sessionFromFlags(flags, ctx.scenario ?? ctx.perf, settings.value);
  /** The run measures the sound too: its context is resumed, and the status feed drives it. */
  const soundOn = ctx.scenario?.view?.audio === true;
  const overrides = scripted ? {} : sessionOverrides(session, settings.value);

  /** A sortie is running. Until the player chooses Fly the core is idle and nothing is held or paused. */
  let flying = ctx.start === 'fly';
  let latestSim: SimStats | undefined;
  let latestRender: RenderStats | undefined;
  /** Held by the person (the overlay, a test) or by the page being hidden. */
  let paused = false;
  /** Held because a menu is open. */
  let menuHold = false;
  let simHeld = false;
  let readyEvents = 0;
  /** Graphics devices lost since the page opened, for a performance run's gate. */
  let deviceLosses = 0;
  const longTasks = countLongTasks();

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
        deviceLosses += 1;
        toasts.show(t('toast.renderer-lost', { reason: event.reason }), 'warn');
        break;
      case 'error':
        ctx.fail('render-worker', codeOf(event.message, 'SYS-107'), event.message);
        break;
      default:
        break;
    }
  };

  // The aircraft's presentation loads beside the workers; the game flies without it if it cannot.
  const viewBuffer = makeViewBuffer(modes.snapshotChannel === 'shared');
  const presentation = loadPresentation({
    bundle: () => loadPresentBundle(),
    pack: async () => {
      const response = await fetch(coreContentUrl);
      if (!response.ok)
        throw new Error(`SYS-005: ${coreContentUrl} returned ${String(response.status)}`);
      return new Uint8Array(await response.arrayBuffer());
    },
  });

  const [sim, render] = await Promise.all([
    SimClient.create({
      workerUrl: simWorkerUrl,
      init: {
        variant,
        wasmUrl: wasmUrl(variant),
        threadsInitialPages: __THREADS_INITIAL_PAGES__,
        contentUrl: coreContentUrl,
        seed: SEED,
        script: ctx.scenario
          ? ctx.scenario.script
          : ctx.perf
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
      viewBuffer,
      onEvent: onRenderEvent,
      shaders: SHADERS,
    }),
  ]);

  const loaded = await presentation;
  if (loaded.render !== undefined && loaded.cockpit !== undefined) {
    render.setPresentation(loaded.render);
    render.setCockpit(loaded.cockpit);
  } else {
    toasts.show(t('toast.presentation-failed', { reason: loaded.error ?? '' }), 'warn');
  }
  const systemReducedMotion = (): boolean => matchMedia('(prefers-reduced-motion: reduce)').matches;
  render.setQuality(qualityOf(settings.value, ctx.benchmarkPreset));
  render.setSettings(renderSettingsOf(settings.value));

  // The sound: the real thing where the browser can run it and the aircraft has a profile.
  const audio: FlightAudio =
    loaded.audio !== undefined && webAudioSupported()
      ? new WebAudioBackend({ profile: loaded.audio, workletUrl: scriptUrl(flightWorkletUrl) })
      : new NullAudioBackend();
  audio.setBuses(busesOf(settings.value));
  const onsets: ToneOnset[] = [];
  let onsetCount = 0;
  audio.onOnset((onset) => {
    onsetCount += 1;
    onsets.push(onset);
    if (onsets.length > 32) onsets.shift();
  });
  let audioMuted = false;

  /** The simulation runs unless the person, the page being hidden, or a menu holds it. */
  const syncHold = (): void => {
    if (!flying) return;
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
  let screens: ScreenController | undefined;
  const unsubscribers: (() => void)[] = [];

  /** What the pump hands to the page: the keys only the page can act on. */
  const onAction = (id: ActionId): void => {
    if (id === 'system.pause') {
      if (screens?.active === true) screens.escape();
      else if (flying) menus?.toggle();
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

  /**
   * Applies the settings that change what is drawn and heard. `patch` is what the player just changed
   * (undefined for all of it): only that part of the view is touched, so a setting changed in a menu
   * does not undo a HUD brightness the player set with a key.
   */
  const applyPresentation = (next: Readonly<Settings>, patch?: SettingsPatch): void => {
    render.setSettings(renderSettingsOf(next));
    if (patch === undefined || patch.video !== undefined) {
      render.setQuality(qualityOf(next, ctx.benchmarkPreset));
    }
    const all = viewSettingsOf(next, systemReducedMotion());
    const changed: ViewSettings = {};
    if (patch === undefined || patch.fovDeg !== undefined) changed.fovDeg = all.fovDeg;
    if (patch === undefined || patch.cameraShake !== undefined)
      changed.shakePercent = all.shakePercent;
    if (patch === undefined || patch.reducedMotion !== undefined)
      changed.reducedMotion = all.reducedMotion;
    if (patch === undefined || patch.hud?.brightness !== undefined)
      changed.hudBrightness = all.hudBrightness;
    view.configure(changed);
    if (patch?.hud?.declutter !== undefined) view.setDeclutter(next.hud.declutter);
    audio.setBuses(busesOf(next));
  };

  /**
   * Fly: the main menu's choice becomes a sortie. The core is idle (after boot or a reset), so it is
   * given the session and the script, the pump forgets the last sortie's spawn (a fresh core numbers
   * its spawns from 1 again), and the clock starts. The click that brought us here is the gesture
   * that lets audio and pointer lock start.
   */
  const fly = (): void => {
    if (flying) return;
    // What the menu shows is what flies: the settings with the URL's (until the player changed them)
    // laid over them, so the URL flags are not applied a second time over the player's choices.
    const now = menus?.settings() ?? settings.value;
    sim.configure(
      sessionFromFlags(defaultFlags(), false, now),
      flags.script === 'circuit' ? INPUT_SCRIPT.CIRCUIT : INPUT_SCRIPT.NONE,
    );
    if (controls) {
      controls.pump.newSortie();
      controls.applySettings(now);
      controls.pump.setContext('flight');
    }
    view.setCamera(flags.camera ?? now.camera);
    view.setDeclutter(now.hud.declutter);
    applyPresentation(now);
    paused = false;
    menuHold = false;
    simHeld = false;
    flying = true;
    sim.start();
    void audio.resume();
    screens?.closeAll();
    ctx.flow.dispatch({ type: 'fly' });
    controls?.syncMouseLock();
  };

  /** The pause menu's Main menu: the sortie is over, the core goes back to idle, the menu shows. */
  const toMainMenu = (): void => {
    if (!flying) return;
    flying = false;
    paused = false;
    menuHold = false;
    simHeld = false;
    sim.reset();
    controls?.pump.setContext('menu');
    controls?.releaseMouse();
    screens?.show(ctx.flow.dispatch({ type: 'main-menu' }));
  };

  /** Applies every setting to the parts that read them (an import, a reset). */
  const applyAllSettings = (next: Readonly<Settings>): void => {
    controls?.applySettings(next);
    controls?.pump.requestAssist(next.assist);
    controls?.pump.setRespawnKind(next.spawn + 1);
    applyPresentation(next);
    view.setDeclutter(next.hud.declutter);
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
      onMainMenu: toMainMenu,
      onProfile: (profile) => {
        c.useProfile(profile);
      },
      onSettings: (patch, next) => {
        c.applyChange(patch, next);
        applyPresentation(next, patch);
      },
      nowMs: monotonicMs,
    });
    c.start();

    const m = menus;
    screens = new ScreenController({
      root: roots.menu,
      settings: () => m.settings(),
      onChange: (patch) => {
        m.change(patch);
      },
      onReplace: (next) => {
        void settings.replace(next);
        applyAllSettings(next);
      },
      onReset: () => {
        void settings.reset().then(applyAllSettings);
      },
      onSetupDone: () => {
        void settings.update({ setupDone: true });
        // Ask the browser to keep the settings and profiles (ADR-19); it may say no, or have no storage API.
        try {
          void navigator.storage.persist().catch(() => false);
        } catch {
          // An insecure context or an old browser: the settings are kept as well as it allows.
        }
      },
      flow: ctx.flow,
      store: controlsStore,
      devices: () =>
        c.pads.slots
          .filter((slot) => slot.present)
          .map((slot) => ({ kind: kindOfMapping(slot.mapping), name: slot.device })),
      profile: () => c.pump.profile,
      useProfile: (profile) => {
        c.useProfile(profile);
      },
      openControls: (back) => {
        m.openControlsOver(back);
      },
      onFly: fly,
      save: downloadText,
      info: { storage: settings.backend, build: __BUILD_ID__ },
      ...(audio.kind === 'webaudio'
        ? {
            audio: {
              available: () => audio.state === 'suspended',
              run: () => {
                void audio.resume().then(() => {
                  screens?.refresh();
                });
              },
            },
          }
        : {}),
      install: {
        available: () => ctx.install.available,
        run: () => {
          void ctx.install.prompt();
        },
      },
      ...(ctx.update === undefined
        ? {}
        : {
            update: {
              available: () => ctx.update?.available === true,
              run: () => {
                ctx.update?.apply();
              },
            },
          }),
    });
    // The menu's Install and update rows come and go with the browser and the Service Worker.
    const refreshScreens = (): void => {
      screens?.refresh();
    };
    unsubscribers.push(ctx.install.onChange(refreshScreens));
    if (ctx.update !== undefined) unsubscribers.push(ctx.update.onChange(refreshScreens));
    // Before the first run and the menu, the keyboard is for the menus, not for flying.
    if (!flying) c.pump.setContext('menu');
  }

  // The view: what the player does with the camera and the HUD goes to the renderer, never the sim.
  const initial = menus?.settings() ?? settings.value;
  const view = new ViewBridge({
    input: controls?.pump.view ?? new ViewInput(),
    buffer: viewBuffer,
    send: (state) => {
      render.setView(state);
    },
    requestFrame: (callback) => requestAnimationFrame(callback),
    cancelFrame: (id) => {
      cancelAnimationFrame(id);
    },
    onCamera: (camera) => {
      audio.setCamera(camera);
    },
  });
  applyPresentation(initial);
  view.setCamera(flags.camera ?? initial.camera);
  view.setDeclutter(initial.hud.declutter);
  view.start();

  // The warning strip and the state the pump needs come from the aircraft's snapshot. A scripted
  // pilot (not a performance run) gets the strip too, so the circuit can be watched. A performance
  // run that measures the sound gets the feed that drives it, and no strip.
  if (!ctx.perf || soundOn) {
    const s = ctx.perf ? undefined : new WarningStrip(roots.strip);
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
        audio.update(status);
        if (!appliedStart && controls && menus) {
          // The mouse aim starts from where the nose points, so wait for the first snapshot.
          appliedStart = true;
          controls.applySettings(menus.settings());
        }
        const now = monotonicMs();
        if (now - lastStrip >= STRIP_MS) {
          lastStrip = now;
          s?.update(status);
        }
      },
    });
    feed.start();
  }

  // Audio waits for a gesture; the null backend only records that it was asked. A run that measures
  // the sound has no gesture to wait for, so it asks at once (the reference browser lets it).
  const cancelAudio = resumeOnFirstGesture(audio, window);
  if (soundOn) void audio.resume();

  /** Paused by the page being hidden (not by the person), so showing it again resumes. */
  let pausedByHiding = false;
  const setPaused = (value: boolean, quiet = false): void => {
    if (!flying || value === paused) return;
    paused = value;
    syncHold();
    ctx.flow.dispatch({ type: value ? 'pause' : 'resume' });
    if (!quiet) toasts.show(t(value ? 'toast.paused' : 'toast.resumed'));
  };
  // A hidden tab pauses the sim, so nothing runs where nobody can see it (GDD 5.6, single-player).
  const onVisibility = (): void => {
    if (settings.value.audio.muteWhenHidden) {
      audioMuted = document.visibilityState === 'hidden';
      audio.setMuted(audioMuted);
    }
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
  // A pad plugged in while the first run asks what the player flies with shows up there.
  let padRevision = controls?.pads.revision ?? 0;
  const padWatch = window.setInterval(() => {
    const revision = controls?.pads.revision ?? 0;
    if (revision === padRevision) return;
    padRevision = revision;
    if (screens?.firstRun.isOpen === true) screens.firstRun.refresh();
  }, 500);
  // The browser may let the sound start on its own (a key press anywhere): the menu's row follows.
  const soundWatch = window.setInterval(() => {
    if (screens?.menu.isOpen === true) screens.menu.refresh();
  }, 500);

  if (flying) sim.start();
  else screens?.show(ctx.flow.state());
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
      flow: () => ctx.flow.state().name,
      audio: () => ({
        kind: audio.kind,
        state: audio.state,
        levelDb: audio instanceof WebAudioBackend ? audio.levelDb() : -200,
        paramWrites: audio instanceof WebAudioBackend ? audio.paramWrites : 0,
        workletError: audio instanceof WebAudioBackend ? audio.workletError : undefined,
        onsets: onsetCount,
        lastOnset: onsets.at(-1),
        muted: audioMuted,
      }),
      view: () => ({
        camera: view.camera,
        declutter: view.state.hudDeclutter,
        brightness: view.state.hudBrightness,
        fovDeg: view.state.fovDeg,
        lookYawRate: view.state.lookYawRate,
        lookPitchRate: view.state.lookPitchRate,
        zoom: view.state.zoom,
        freeLook: view.state.freeLook,
      }),
      presentationError: () => loaded.error,
    };
    (window as unknown as { __flying?: TestHooks }).__flying = hooks;
  }

  const stop = (): void => {
    delete roots.stage.dataset['state'];
    window.clearInterval(watchdog);
    window.clearInterval(padWatch);
    window.clearInterval(soundWatch);
    view.stop();
    removeHotkey();
    document.removeEventListener('visibilitychange', onVisibility);
    feed?.stop();
    controls?.destroy();
    menus?.destroy();
    for (const off of unsubscribers.splice(0)) off();
    screens?.destroy();
    strip?.destroy();
    longTasks.stop();
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
        longTasks: () => longTasks.count(),
        deviceLosses: () => deviceLosses,
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
