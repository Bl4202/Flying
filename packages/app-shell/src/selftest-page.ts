/**
 * `?selftest=1`: the page the end-to-end tests read. A status table with one cell per fact (each
 * with a stable `data-testid`): what isolation and features this load got, which build ran, and
 * the state hash after the self-test's 10 000 ticks.
 */
import {
  chooseVariant,
  type DeepFeatures,
  type IsolationOutcome,
  type ServiceWorkerStatus,
} from '@flying/platform-web';
import { runSelfTestWorker, selfTestRequest } from './selftest.ts';
import { goldenCheckpoint, judgeSelfTest, SELFTEST_TICKS } from './selftest-judge.ts';

type State = 'ok' | 'warn' | '';

export interface SelfTestPageArgs {
  /** The `<main>` that holds the status table. */
  root: HTMLElement;
  features: DeepFeatures;
  outcome: IsolationOutcome;
  serviceWorker: ServiceWorkerStatus | 'dev';
}

export async function runSelfTestPage(args: SelfTestPageArgs): Promise<void> {
  const table = args.root.querySelector('table');
  if (!table) {
    throw new Error('SYS-010: the self-test table is missing from the page');
  }
  args.root.hidden = false;

  const row = (id: string, label: string, value: string, state: State = ''): void => {
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
  };

  const f = args.features;
  row('status', 'Status', 'booting');
  row('build-id', 'Build', __BUILD_ID__);
  row(
    'isolated',
    'Cross-origin isolated',
    String(f.crossOriginIsolated),
    f.crossOriginIsolated ? 'ok' : 'warn',
  );
  row('sab', 'SharedArrayBuffer', String(f.sharedArrayBuffer), f.sharedArrayBuffer ? 'ok' : 'warn');
  row(
    'mode',
    'Mode',
    args.outcome === 'isolated' ? 'full' : 'degraded',
    args.outcome === 'isolated' ? 'ok' : 'warn',
  );
  row('sw', 'Service Worker', args.serviceWorker, args.serviceWorker === 'active' ? 'ok' : 'warn');

  const variant = chooseVariant(f);
  if (!variant) {
    row('variant', 'WASM variant', 'none (WebAssembly SIMD required)', 'warn');
    row('status', 'Status', 'error', 'warn');
    return;
  }
  row('variant', 'WASM variant', variant, 'ok');

  try {
    const result = await runSelfTestWorker(selfTestRequest(variant));
    if (result.ok) {
      row('abi', 'Sim ABI version', String(result.abi), 'ok');
      row('ticks', 'Ticks run', result.tick);
      row('hash', `State hash @ ${String(SELFTEST_TICKS)} ticks`, result.hash);
      row('content-hash', 'Content pack hash', result.content);
      const verdict = judgeSelfTest(result, goldenCheckpoint());
      row('golden', 'Matches the golden', verdict.ok ? 'yes' : 'no', verdict.ok ? 'ok' : 'warn');
      row('status', 'Status', 'done', 'ok');
    } else {
      row('status', 'Status', `error: ${result.error}`, 'warn');
    }
  } catch (e) {
    row('status', 'Status', `error: ${e instanceof Error ? e.message : String(e)}`, 'warn');
  }
}
