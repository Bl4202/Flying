// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { MAX_TOASTS, ToastHost, type Timers } from './toast.ts';

class FakeTimers implements Timers {
  now = 0;
  #next = 1;
  readonly pending = new Map<number, { at: number; cb: () => void }>();
  set(cb: () => void, ms: number): number {
    const id = this.#next++;
    this.pending.set(id, { at: this.now + ms, cb });
    return id;
  }
  clear(id: number): void {
    this.pending.delete(id);
  }
  advance(ms: number): void {
    this.now += ms;
    for (const [id, t] of [...this.pending]) {
      if (t.at <= this.now) {
        this.pending.delete(id);
        t.cb();
      }
    }
  }
}

function host() {
  const root = document.createElement('div');
  const timers = new FakeTimers();
  return { root, timers, toasts: new ToastHost(root, timers) };
}

describe('ToastHost', () => {
  it('shows the kind in words and announces politely', () => {
    const { root, toasts } = host();
    toasts.show('Switched to WebGL2', 'warn');
    expect(root.getAttribute('aria-live')).toBe('polite');
    const el = root.querySelector('[data-testid="toast"]');
    expect(el?.textContent).toBe('Warning: Switched to WebGL2');
    expect(el?.getAttribute('role')).toBe('status');
  });

  it('uses the alert role for problems', () => {
    const { root, toasts } = host();
    toasts.show('device lost', 'error');
    expect(root.querySelector('[data-testid="toast"]')?.getAttribute('role')).toBe('alert');
  });

  it('goes away by itself, sooner for notices than for problems', () => {
    const { root, timers, toasts } = host();
    toasts.show('a', 'info');
    toasts.show('b', 'error');
    expect(toasts.count).toBe(2);
    timers.advance(4001);
    expect(root.textContent).not.toContain('a');
    expect(toasts.count).toBe(1);
    timers.advance(8000);
    expect(toasts.count).toBe(0);
    expect(root.children).toHaveLength(0);
  });

  it('keeps at most three, dropping the oldest, and leaves no stray timers', () => {
    const { root, timers, toasts } = host();
    for (let i = 0; i < MAX_TOASTS + 2; i++) toasts.show(`m${String(i)}`);
    expect(toasts.count).toBe(MAX_TOASTS);
    expect(root.children).toHaveLength(MAX_TOASTS);
    expect(root.textContent).not.toContain('m0');
    expect(root.textContent).toContain('m4');
    expect(timers.pending.size).toBe(MAX_TOASTS);
  });
});
