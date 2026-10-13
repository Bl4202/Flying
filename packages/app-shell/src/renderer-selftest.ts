/**
 * `?selftest=renderer`: the renderer contract (`runRendererContract`) against each real backend in
 * this browser. One row per backend, with a stable test id: `contract-webgl2` and
 * `contract-webgpu` read `pass (N checks)`, `fail: <first failing check>` or `skipped: <why>`.
 * WebGPU is skipped where the browser has no adapter, which is the case on CI runners; the
 * reference-machine runs are where it must pass.
 */
import { runRendererContract, WebGl2Renderer, WebGpuRenderer } from '@flying/render';
import { SHADERS } from '@flying/render/shader-sources';
import type { Renderer } from '@flying/render/renderer';
import type { DeepFeatures } from '@flying/platform-web';

type Backend = 'webgl2' | 'webgpu';

export interface RendererSelfTestArgs {
  root: HTMLElement;
  features: DeepFeatures;
}

export interface BackendOutcome {
  state: 'pass' | 'fail' | 'skipped';
  text: string;
}

/** A canvas that nobody has drawn on: every renderer the contract makes needs its own. */
function freshCanvas(): OffscreenCanvas | HTMLCanvasElement {
  return typeof OffscreenCanvas === 'undefined'
    ? document.createElement('canvas')
    : new OffscreenCanvas(160, 90);
}

async function runBackend(backend: Backend, features: DeepFeatures): Promise<BackendOutcome> {
  if (backend === 'webgpu' && !features.webgpuAdapter) {
    return { state: 'skipped', text: 'skipped: no WebGPU adapter in this browser' };
  }
  if (backend === 'webgl2' && !features.webgl2) {
    return { state: 'skipped', text: 'skipped: no WebGL2 in this browser' };
  }
  const make = (): Promise<Renderer> =>
    backend === 'webgl2'
      ? Promise.resolve(WebGl2Renderer.create(freshCanvas(), SHADERS))
      : WebGpuRenderer.create(freshCanvas(), SHADERS);
  try {
    const checks = await runRendererContract(make, { draws: true });
    const failed = checks.filter((c) => !c.ok);
    const first = failed[0];
    return first === undefined
      ? { state: 'pass', text: `pass (${String(checks.length)} checks)` }
      : { state: 'fail', text: `fail: ${first.name}: ${first.detail}` };
  } catch (e) {
    return { state: 'fail', text: `fail: ${e instanceof Error ? e.message : String(e)}` };
  }
}

export async function runRendererSelfTest(args: RendererSelfTestArgs): Promise<void> {
  const table = args.root.querySelector('table');
  if (!table) {
    throw new Error('SYS-010: the self-test table is missing from the page');
  }
  args.root.hidden = false;
  const row = (id: string, label: string, value: string, state: string): void => {
    const tr = document.createElement('tr');
    const th = document.createElement('th');
    th.scope = 'row';
    th.textContent = label;
    const td = document.createElement('td');
    td.dataset['testid'] = id;
    td.dataset['state'] = state;
    td.textContent = value;
    tr.append(th, td);
    table.append(tr);
  };
  const status = document.createElement('td');
  status.dataset['testid'] = 'status';
  status.textContent = 'running';
  const head = document.createElement('tr');
  const th = document.createElement('th');
  th.scope = 'row';
  th.textContent = 'Status';
  head.append(th, status);
  table.append(head);

  for (const backend of ['webgl2', 'webgpu'] as const) {
    const outcome = await runBackend(backend, args.features);
    row(
      `contract-${backend}`,
      `Renderer contract, ${backend}`,
      outcome.text,
      outcome.state === 'pass' ? 'ok' : 'warn',
    );
    // The data attribute carries the machine-readable result next to the text.
    const cell = table.querySelector<HTMLElement>(`[data-testid="contract-${backend}"]`);
    if (cell) cell.dataset['result'] = outcome.state;
  }
  status.textContent = 'done';
}
