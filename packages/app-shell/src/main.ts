import {
  chooseVariant,
  detect,
  ensureIsolation,
  scriptUrl,
  type IsolationOutcome,
  type ServiceWorkerStatus,
} from '@flying/platform-web';
import coreContentUrl from '../../../content-out/core.cpack?url';
import singleUrl from '../../../wasm-out/sim.single.wasm?url';
import threadsUrl from '../../../wasm-out/sim.threads.wasm?url';
import type { ProbeRequest, ProbeResult } from './probe.worker.ts';
import probeWorkerUrl from './probe.worker.ts?worker&url';

/** Default master seed, equal to `sim_core::DEFAULT_SEED`. */
const SEED = '1f1e5eed00000001';
const PROBE_TICKS = 10_000;

type State = 'ok' | 'warn' | '';

const statusEl = document.getElementById('status');
if (!(statusEl instanceof HTMLTableElement)) {
  throw new Error('SYS-010: #status table missing');
}
const table: HTMLTableElement = statusEl;

function row(id: string, label: string, value: string, state: State = ''): void {
  let tr = table.querySelector<HTMLTableRowElement>(`tr[data-row="${id}"]`);
  if (!tr) {
    tr = document.createElement('tr');
    tr.dataset['row'] = id;
    const th = document.createElement('th');
    th.scope = 'row';
    th.textContent = label;
    const td = document.createElement('td');
    td.dataset['testid'] = id;
    tr.append(th, td);
    table.append(tr);
  }
  const td = tr.querySelector('td');
  if (td) {
    td.textContent = value;
    td.dataset['state'] = state;
  }
}

async function main(): Promise<void> {
  row('status', 'Status', 'booting');
  row('build-id', 'Build', __BUILD_ID__);

  let outcome: IsolationOutcome = crossOriginIsolated ? 'isolated' : 'degraded';
  let sw: ServiceWorkerStatus | 'dev' = 'dev';
  if (import.meta.env.PROD) {
    const base = import.meta.env.BASE_URL;
    const result = await ensureIsolation({ swUrl: `${base}sw.js`, scope: base });
    if (result.outcome === 'reloading') {
      return; // the page reloads under the Service Worker; render nothing now
    }
    outcome = result.outcome;
    sw = result.serviceWorker;
  }

  const features = detect();
  row(
    'isolated',
    'Cross-origin isolated',
    String(features.crossOriginIsolated),
    features.crossOriginIsolated ? 'ok' : 'warn',
  );
  row(
    'sab',
    'SharedArrayBuffer',
    String(features.sharedArrayBuffer),
    features.sharedArrayBuffer ? 'ok' : 'warn',
  );
  row(
    'mode',
    'Mode',
    outcome === 'isolated' ? 'full' : 'degraded',
    outcome === 'isolated' ? 'ok' : 'warn',
  );
  row('sw', 'Service Worker', sw, sw === 'active' ? 'ok' : 'warn');

  const variant = chooseVariant(features);
  if (!variant) {
    row('variant', 'WASM variant', 'none (WebAssembly SIMD required)', 'warn');
    row('status', 'Status', 'error', 'warn');
    return;
  }
  row('variant', 'WASM variant', variant, 'ok');

  const request: ProbeRequest = {
    variant,
    url: variant === 'threads' ? threadsUrl : singleUrl,
    threadsInitialPages: __THREADS_INITIAL_PAGES__,
    contentUrl: coreContentUrl,
    seed: SEED,
    script: 'erratic',
    ticks: PROBE_TICKS,
  };
  const worker = new Worker(scriptUrl(probeWorkerUrl), { type: 'module' });
  worker.onmessage = (event: MessageEvent<ProbeResult>) => {
    const r = event.data;
    if (r.ok) {
      row('abi', 'Sim ABI version', String(r.abi), 'ok');
      row('ticks', 'Ticks run', r.tick);
      row('hash', `State hash @ ${String(PROBE_TICKS)} ticks`, r.hash);
      row('content-hash', 'Content pack hash', r.content);
      row('status', 'Status', 'done', 'ok');
    } else {
      row('status', 'Status', `error: ${r.error}`, 'warn');
    }
    worker.terminate();
  };
  worker.onerror = (event) => {
    row('status', 'Status', `error: worker failed (${event.message})`, 'warn');
  };
  worker.postMessage(request);
}

main().catch((e: unknown) => {
  row('status', 'Status', `error: ${e instanceof Error ? e.message : String(e)}`, 'warn');
});
