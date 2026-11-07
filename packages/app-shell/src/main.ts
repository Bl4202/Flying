/**
 * The composition root: decides, in order, whether this load can run at all (isolation, features,
 * the compat check), then hands over to the self-test page or the demo. Every other module in the
 * app takes what it needs as arguments; only this file reads the URL, the DOM roots and the
 * stored acknowledgement.
 */
import {
  detectAsync,
  ensureIsolation,
  InstallPrompt,
  installGlobalHandlers,
  openKv,
  parseFlags,
  RingLogger,
  UpdateWatcher,
  type IsolationOutcome,
  type ServiceWorkerStatus,
} from '@flying/platform-web';
import { loadBenchmark } from '@flying/perf';
import {
  evaluateCompat,
  flowReducer,
  INITIAL_FLOW,
  mountCompatScreen,
  mountFatalScreen,
  readCompatAck,
  SettingsStore,
  shouldShowCompat,
  t,
  writeCompatAck,
  type FatalSource,
  type FlowEvent,
  type FlowState,
  type StartTarget,
} from '@flying/ui';
import { runAudioSelfTest } from './audio-selftest.ts';
import { runBenchmarkPage } from './benchmark-page.ts';
import { codeOf, startDemo, type Demo } from './demo.ts';
import { runPerfPage } from './perf-run.ts';
import { runRendererSelfTest } from './renderer-selftest.ts';
import { runSelfTestPage } from './selftest-page.ts';

function element(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) {
    throw new Error(`SYS-010: #${id} is missing from the page`);
  }
  return el;
}

const logger = new RingLogger();
installGlobalHandlers(logger, window, 'main');

let flow: FlowState = INITIAL_FLOW;
function dispatch(event: FlowEvent): FlowState {
  flow = flowReducer(flow, event);
  return flow;
}
/** A function rather than a comparison inline, so the compiler does not carry an earlier narrowing past an `await`. */
const isFatal = (): boolean => flow.name === 'FATAL';

async function main(): Promise<void> {
  const flags = parseFlags(location.search);
  for (const warning of flags.warnings) {
    logger.warn('flags', warning);
  }
  document.title = t('app.name');
  // The browser fires `beforeinstallprompt` as soon as the page qualifies, which can be before the
  // game is up: listen from the start and hold the offer for the main menu.
  const install = new InstallPrompt(
    window,
    () =>
      matchMedia('(display-mode: standalone)').matches ||
      (navigator as { standalone?: boolean }).standalone === true,
  );
  const stage = element('stage');
  const screenRoot = element('screen-root');
  const loading = document.createElement('p');
  loading.className = 'loading';
  loading.textContent = t('app.loading');
  screenRoot.append(loading);

  let outcome: IsolationOutcome = crossOriginIsolated ? 'isolated' : 'degraded';
  let serviceWorker: ServiceWorkerStatus | 'dev' = 'dev';
  let update: UpdateWatcher | undefined;
  if (import.meta.env.PROD) {
    const base = import.meta.env.BASE_URL;
    const result = await ensureIsolation({ swUrl: `${base}sw.js`, scope: base });
    if (result.outcome === 'reloading') {
      return; // the page reloads under the Service Worker; render nothing now
    }
    outcome = result.outcome;
    serviceWorker = result.serviceWorker;
    if (serviceWorker === 'active') {
      const registration = await navigator.serviceWorker.getRegistration(base);
      if (registration) {
        update = new UpdateWatcher(registration, navigator.serviceWorker, () => {
          location.reload();
        });
        // Ask whether a newer build is out (it installs in the background and waits for the player).
        void update.check();
      }
    }
  }
  const features = await detectAsync();
  loading.remove();

  if (flags.selftest === 'hash') {
    await runSelfTestPage({ root: element('selftest-root'), features, outcome, serviceWorker });
    return;
  }

  if (flags.selftest === 'renderer') {
    await runRendererSelfTest({ root: element('selftest-root'), features });
    return;
  }

  if (flags.selftest === 'audio') {
    await runAudioSelfTest(element('selftest-root'));
    return;
  }

  dispatch({ type: 'begin-check' });
  const report = evaluateCompat({ features, flags });
  const kv = await openKv('flying');
  const settings = new SettingsStore(kv);
  await settings.load();
  // A scripted run and `?fly=1` go straight to the flight; a first visit sees the setup; the rest see the menu.
  const start: StartTarget =
    flags.fly || flags.perf !== false || flags.script !== false
      ? 'fly'
      : settings.value.setupDone
        ? 'menu'
        : 'first-run';
  const ack = await readCompatAck(kv);
  const showScreen = shouldShowCompat({ forced: flags.compat, report, ack });
  const firstCode = report.codes[0];
  dispatch({
    type: 'check-done',
    verdict: report.verdict,
    showScreen,
    after: start,
    ...(firstCode ? { code: firstCode } : {}),
  });

  if (flow.name === 'FATAL') {
    mountCompatScreen(screenRoot, report, {
      onContinue: () => undefined,
      onReload: () => {
        location.reload();
      },
    });
    return;
  }
  if (flow.name === 'COMPAT_CHECK') {
    await new Promise<void>((resolve) => {
      mountCompatScreen(screenRoot, report, {
        onContinue: () => {
          resolve();
        },
      });
    });
    dispatch({ type: 'acknowledge' });
    await writeCompatAck(kv, report, Date.now());
    screenRoot.replaceChildren();
  }

  const backToGame = (): void => {
    const next = new URL(location.href);
    next.searchParams.delete('bench');
    next.searchParams.delete('perf');
    next.searchParams.delete('seconds');
    location.assign(next);
  };
  if (flags.bench !== false) {
    // The benchmark has the machine to itself: no game is started behind it.
    await runBenchmarkPage({
      root: element('panel-root'),
      mode: flags.bench,
      flags,
      features,
      kv,
      onBack: backToGame,
    });
    return;
  }
  const stored = await loadBenchmark(kv);

  let demo: Demo | undefined;
  const fail = (source: FatalSource, code: string, detail: string): void => {
    if (isFatal()) {
      return;
    }
    dispatch({ type: 'fatal', code, message: detail });
    logger.error('main', `${code}: ${detail}`);
    demo?.stop();
    const tail = logger
      .entries()
      .slice(-30)
      .map((e) => `${e.level} ${e.source}: ${e.message}`)
      .join('\n');
    mountFatalScreen(screenRoot, {
      source,
      code,
      detail: `${detail}\n\n${tail}`,
      onReload: () => {
        location.reload();
      },
    });
  };

  try {
    demo = await startDemo({
      flags,
      perf: flags.perf !== false,
      benchmarkSummary: stored
        ? `${stored.classification.profile} / ${stored.classification.preset}`
        : undefined,
      benchmarkPreset: stored?.classification.preset,
      kv,
      settings,
      start,
      install,
      update,
      features,
      report,
      roots: {
        stage,
        overlay: element('overlay-root'),
        toast: element('toast-root'),
        strip: element('strip-root'),
        menu: element('menu-root'),
      },
      flow: { dispatch, state: () => flow },
      fail,
    });
    if (isFatal()) {
      demo.stop();
    } else if (flags.perf !== false) {
      await runPerfPage({
        root: element('panel-root'),
        demo,
        scenarioId: flags.perf,
        seconds: flags.seconds,
        onBack: backToGame,
      });
    }
  } catch (e) {
    const message = e instanceof Error ? e.message : String(e);
    fail('unknown', codeOf(message, 'SYS-010'), message);
  }
}

main().catch((e: unknown) => {
  const message = e instanceof Error ? e.message : String(e);
  logger.error('main', message);
  const status = document.getElementById('screen-root');
  if (status) {
    mountFatalScreen(status, {
      source: 'unknown',
      code: codeOf(message, 'SYS-010'),
      detail: message,
      onReload: () => {
        location.reload();
      },
    });
  }
});
