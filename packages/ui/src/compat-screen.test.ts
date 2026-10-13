// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { defaultFlags, type DeepFeatures } from '@flying/platform-web';
import { mountCompatScreen } from './compat-screen.ts';
import { evaluateCompat } from './compat.ts';
import { mountFatalScreen } from './fatal-screen.ts';

const FULL: DeepFeatures = {
  crossOriginIsolated: true,
  sharedArrayBuffer: true,
  wasm: true,
  wasmSimd: true,
  wasmThreads: true,
  serviceWorker: true,
  webgpu: true,
  offscreenCanvas: true,
  webgpuAdapter: true,
  adapterInfo: 'apple metal-3',
  webgl2: true,
  opfs: true,
  gamepad: true,
};

function root(): HTMLElement {
  const el = document.createElement('div');
  document.body.append(el);
  return el;
}

const byId = (el: ParentNode, id: string): HTMLElement | null =>
  el.querySelector<HTMLElement>(`[data-testid="${id}"]`);

describe('compat screen', () => {
  it('shows every row with its state in words and a mark, not colour alone', () => {
    const report = evaluateCompat({
      features: { ...FULL, opfs: false },
      flags: defaultFlags(),
    });
    const r = root();
    mountCompatScreen(r, report, { onContinue: () => undefined });

    expect(r.querySelectorAll('[data-testid^="compat-row-"]')).toHaveLength(9);
    expect(byId(r, 'compat-state-wasm')?.textContent).toBe('✓ OK');
    expect(byId(r, 'compat-state-opfs')?.textContent).toBe('! Reduced');
    expect(byId(r, 'compat-row-opfs')?.dataset['state']).toBe('degraded');
    expect(byId(r, 'compat-remedy-opfs')?.textContent).toContain('What to do:');
    expect(byId(r, 'compat-remedy-wasm')).toBeNull();
    expect(byId(r, 'compat-verdict')?.dataset['verdict']).toBe('degraded');
    expect(byId(r, 'compat-modes')?.textContent).toContain('multi-threaded WebAssembly');
  });

  it('focuses Continue, and Continue calls back', () => {
    const report = evaluateCompat({ features: FULL, flags: defaultFlags() });
    const onContinue = vi.fn();
    const r = root();
    mountCompatScreen(r, report, { onContinue });
    const go = byId(r, 'compat-continue');
    expect(go).not.toBeNull();
    expect(document.activeElement).toBe(go);
    go?.click();
    expect(onContinue).toHaveBeenCalledTimes(1);
  });

  it('a fatal verdict has the codes and no Continue', () => {
    const report = evaluateCompat({
      features: { ...FULL, webgpuAdapter: false, webgl2: false },
      flags: defaultFlags(),
    });
    const onReload = vi.fn();
    const r = root();
    mountCompatScreen(r, report, { onContinue: () => undefined, onReload });
    expect(byId(r, 'compat-continue')).toBeNull();
    expect(byId(r, 'compat-codes')?.textContent).toBe('SYS-103');
    expect(byId(r, 'compat-state-renderer')?.textContent).toBe('✗ Missing');
    byId(r, 'compat-reload')?.click();
    expect(onReload).toHaveBeenCalledTimes(1);
  });

  it('replaces whatever was in the root and can be removed', () => {
    const report = evaluateCompat({ features: FULL, flags: defaultFlags() });
    const r = root();
    r.append(document.createElement('p'));
    const screen = mountCompatScreen(r, report, { onContinue: () => undefined });
    expect(r.children).toHaveLength(1);
    screen.destroy();
    expect(r.children).toHaveLength(0);
  });

  it('puts adapter text in as text, never as markup', () => {
    const report = evaluateCompat({
      features: { ...FULL, adapterInfo: '<img src=x onerror=alert(1)>' },
      flags: defaultFlags(),
    });
    const r = root();
    mountCompatScreen(r, report, { onContinue: () => undefined });
    expect(r.querySelector('img')).toBeNull();
    expect(r.textContent).toContain('<img src=x onerror=alert(1)>');
  });
});

describe('fatal screen', () => {
  it('shows what stopped, the code and the detail, and reloads on request', () => {
    const onReload = vi.fn();
    const r = root();
    mountFatalScreen(r, {
      source: 'sim-worker',
      code: 'SYS-032',
      detail: 'line one\nline two',
      onReload,
    });
    expect(byId(r, 'fatal-message')?.textContent).toBe('The simulation stopped responding.');
    expect(byId(r, 'fatal-code')?.textContent).toBe('Code SYS-032');
    expect(byId(r, 'fatal-detail')?.textContent).toBe('line one\nline two');
    expect(document.activeElement).toBe(byId(r, 'fatal-reload'));
    byId(r, 'fatal-reload')?.click();
    expect(onReload).toHaveBeenCalledTimes(1);
  });
});
