// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import { PROFILE, STAGE, type SimStats } from '@flying/sim-bridge';
import type { RenderStats } from '@flying/render/messages';
import {
  DebugOverlay,
  installOverlayHotkey,
  isOverlayChord,
  OVERLAY_REFRESH_MS,
  type IntervalTimers,
  type OverlayActions,
} from './debug-overlay.ts';
import { countNodes } from './dom.ts';
import {
  attitudeDegrees,
  buildOverlayRows,
  formatHashReport,
  STAGE_NAMES,
  type OverlayModel,
} from './overlay-rows.ts';

const Q = { count: 10, avg: 0.5, p50: 0.4, p95: 0.9, p99: 1.2, max: 2 };

function sim(over: Partial<SimStats> = {}): SimStats {
  return {
    atMs: 1,
    tick: 600,
    hz: 60.01,
    droppedMs: 0,
    droppedEvents: 0,
    step: Q,
    stepRun: Q,
    inputLatency: Q,
    inputLatencyRun: Q,
    inputsDropped: 0,
    snapshotsDropped: 0,
    coreState: 2,
    sleeper: 'atomics',
    observer: {
      pos: [10, -6000, 1500],
      quat: [Math.SQRT1_2, 0, 0, Math.SQRT1_2],
      speed: 180,
      throttle: 0.5,
      inputSeq: 9,
    },
    profile: {
      tickMsLast: 0.1,
      tickMsEwma: 0.1,
      stageMsLast: Array.from({ length: PROFILE.STAGE_COUNT }, () => 0),
      stageMsEwma: Array.from({ length: PROFILE.STAGE_COUNT }, (_, i) => i / 1000),
      counters: Array.from({ length: PROFILE.COUNTER_COUNT }, () => 0),
    },
    ...over,
  };
}

function render(): RenderStats {
  return {
    atMs: 1,
    backend: 'webgl2',
    description: 'WebGL2 test',
    width: 1280,
    height: 720,
    fps: 59.9,
    frameMs: Q,
    renderCpuMs: Q,
    renderCpuRun: Q,
    inputToDrawMs: Q,
    inputToDrawRun: Q,
    snapshotAgeMs: Q,
    frames: 1000,
    snapshotsTaken: 600,
    staleFrames: 3,
    gappedFrames: 1,
    snappedFrames: 0,
    contendedReads: 0,
  };
}

function model(over: Partial<OverlayModel> = {}): OverlayModel {
  return {
    build: 'abcdef0123456789',
    variant: 'threads',
    snapshotChannel: 'shared',
    renderHost: 'worker',
    sleeper: 'atomics',
    abi: 2,
    contentHash: '0123456789abcdef',
    adapter: 'test gpu',
    sim: sim(),
    render: render(),
    paused: false,
    input: { sent: 100, refused: 0 },
    ...over,
  };
}

describe('buildOverlayRows', () => {
  it('has one numeric row per stage, in tick order', () => {
    const rows = buildOverlayRows(model());
    const stages = rows.filter((r) => r.id.startsWith('stage-'));
    expect(stages.map((r) => r.id)).toEqual(STAGE_NAMES.map((n) => `stage-${n}`));
    expect(stages).toHaveLength(Object.keys(STAGE).length);
    for (const r of stages) expect(r.value).toMatch(/^\d+\.\d{3} ms$/);
    expect(stages[10]?.value).toBe('0.010 ms');
  });

  it('shows dashes, not NaN, before any report has arrived', () => {
    const rows = buildOverlayRows(model({ sim: undefined, render: undefined, input: undefined }));
    for (const r of rows) {
      expect(r.value, r.id).not.toMatch(/NaN|undefined/);
    }
    expect(rows.find((r) => r.id === 'tick')?.value).toBe('–');
    expect(rows.find((r) => r.id === 'stage-ingest')?.value).toBe('–');
  });

  it('shows dashes for stages while profiling is off', () => {
    const rows = buildOverlayRows(model({ sim: sim({ profile: null }) }));
    expect(rows.find((r) => r.id === 'stage-publish')?.value).toBe('–');
  });

  it('formats the headline numbers', () => {
    const get = (id: string) => buildOverlayRows(model()).find((r) => r.id === id)?.value;
    expect(get('tick')).toBe('600');
    expect(get('sim-hz')).toBe('60.01');
    expect(get('fps')).toBe('59.9');
    expect(get('core')).toBe('running');
    expect(get('build')).toBe('abcdef012345');
    expect(get('mode')).toBe('threads · shared · atomics');
    expect(get('renderer')).toBe('webgl2 on worker');
    expect(get('content')).toBe('0123456789abcdef');
    expect(get('speed')).toBe('180.0 m/s');
    expect(get('pos')).toBe('10 -6000 1500');
  });

  it('never shows a heading of 360', () => {
    // The spawn attitude turned a hair west of north (about 0.017 degrees): the heading is 359.983,
    // which would print as 360.0 unless it is folded after rounding.
    const eps = 0.0003;
    const h = Math.SQRT1_2;
    const c = Math.cos(eps / 2);
    const s = Math.sin(eps / 2);
    const quat: [number, number, number, number] = [0, h * (c - s), h * (c + s), 0];
    const m = model({ sim: sim({ observer: { ...sim().observer, quat } }) });
    const heading = buildOverlayRows(m).find((r) => r.id === 'heading')?.value;
    expect(heading).toBe('0.0°');
  });

  it('shows the renderer device, cut to a sane length', () => {
    const m = model({ render: { ...render(), description: 'x'.repeat(200) } });
    const device = buildOverlayRows(m).find((r) => r.id === 'device')?.value;
    expect(device).toHaveLength(60);
  });

  it('has unique ids', () => {
    const ids = buildOverlayRows(model()).map((r) => r.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe('attitudeDegrees', () => {
  it('reads level flight heading north as pitch 0, roll 0, heading 0', () => {
    // The spawn attitude: body x to north, body y to east, body z down.
    const a = attitudeDegrees([0, Math.SQRT1_2, Math.SQRT1_2, 0]);
    expect(a.pitch).toBeCloseTo(0, 5);
    expect(a.roll).toBeCloseTo(0, 5);
    expect(a.heading).toBeCloseTo(0, 4);
  });

  it('reads a nose-up attitude as positive pitch', () => {
    // Spawn attitude composed with a rotation of +10 degrees about the body y (right wing) axis.
    const h = Math.SQRT1_2;
    const spawn: [number, number, number, number] = [0, h, h, 0];
    const half = (10 * Math.PI) / 180 / 2;
    const pitch: [number, number, number, number] = [Math.cos(half), 0, -Math.sin(half), 0];
    // q = spawn * pitch (body-to-BF), Hamilton product.
    const [aw, ax, ay, az] = spawn;
    const [bw, bx, by, bz] = pitch;
    const q: [number, number, number, number] = [
      aw * bw - ax * bx - ay * by - az * bz,
      aw * bx + ax * bw + ay * bz - az * by,
      aw * by - ax * bz + ay * bw + az * bx,
      aw * bz + ax * by - ay * bx + az * bw,
    ];
    expect(Math.abs(attitudeDegrees(q).pitch)).toBeCloseTo(10, 3);
  });
});

describe('formatHashReport', () => {
  it('prints the tick and aligned columns', () => {
    expect(formatHashReport({ tick: 7, hashes: { combined: 'aa', rng: 'bb' } })).toBe(
      'tick 7\ncombined aa\nrng      bb',
    );
  });
});

class FakeIntervals implements IntervalTimers {
  readonly active = new Map<number, () => void>();
  #next = 1;
  setInterval(cb: () => void, ms: number): number {
    expect(ms).toBe(OVERLAY_REFRESH_MS);
    const id = this.#next++;
    this.active.set(id, cb);
    return id;
  }
  clearInterval(id: number): void {
    this.active.delete(id);
  }
  tick(): void {
    for (const cb of this.active.values()) cb();
  }
}

function actions(over: Partial<OverlayActions> = {}): OverlayActions {
  return {
    hash: () =>
      Promise.resolve({
        tick: 5,
        hashes: { combined: '0123456789abcdef', tick: 'fedcba9876543210' },
      }),
    selfTest: () => Promise.resolve({ ok: true, text: 'golden matches' }),
    togglePause: () => undefined,
    ...over,
  };
}

function setup(over: { model?: () => OverlayModel; actions?: OverlayActions } = {}) {
  const root = document.createElement('div');
  document.body.append(root);
  const timers = new FakeIntervals();
  const visible: boolean[] = [];
  const overlay = new DebugOverlay(root, {
    model: over.model ?? (() => model()),
    actions: over.actions ?? actions(),
    onVisibleChange: (v) => visible.push(v),
    timers,
  });
  const q = (id: string) => root.querySelector<HTMLElement>(`[data-testid="${id}"]`);
  return { root, timers, visible, overlay, q };
}

describe('DebugOverlay', () => {
  it('builds nothing until it is opened, and stops its timer when closed', () => {
    const { root, timers, overlay, visible } = setup();
    expect(root.children).toHaveLength(0);
    overlay.show();
    expect(timers.active.size).toBe(1);
    overlay.hide();
    expect(timers.active.size).toBe(0);
    expect(visible).toEqual([true, false]);
    expect(root.querySelector<HTMLElement>('[data-testid="overlay"]')?.hidden).toBe(true);
  });

  it('shows the eleven stage rows and keeps the DOM small', () => {
    const { root, overlay, q } = setup();
    overlay.show();
    for (const name of STAGE_NAMES) {
      expect(q(`ov-stage-${name}`)?.dataset['value']).toMatch(/^\d+\.\d{3} ms$/);
    }
    expect(countNodes(root)).toBeLessThanOrEqual(150);
  });

  it('refreshes from the model on each tick of the timer and only rewrites what changed', () => {
    let current = model();
    const { timers, overlay, q } = setup({ model: () => current });
    overlay.show();
    expect(q('ov-tick')?.dataset['value']).toBe('600');
    current = model({ sim: sim({ tick: 660 }) });
    expect(q('ov-tick')?.dataset['value']).toBe('600');
    timers.tick();
    expect(q('ov-tick')?.dataset['value']).toBe('660');
  });

  it('toggles', () => {
    const { overlay } = setup();
    overlay.toggle();
    expect(overlay.visible).toBe(true);
    overlay.toggle();
    expect(overlay.visible).toBe(false);
  });

  it('the Hash button prints sixteen-digit values', async () => {
    const { overlay, q } = setup();
    overlay.show();
    q('ov-hash')?.click();
    await vi.waitFor(() => {
      expect(q('ov-hash-out')?.textContent).toContain('combined 0123456789abcdef');
    });
    expect(q('ov-hash-out')?.textContent).toMatch(/^tick 5\n/);
    expect((q('ov-hash') as HTMLButtonElement).disabled).toBe(false);
  });

  it('the Self-test button reports PASS or FAIL with the text', async () => {
    const { overlay, q } = setup({
      actions: actions({ selfTest: () => Promise.resolve({ ok: false, text: 'mismatch' }) }),
    });
    overlay.show();
    q('ov-selftest')?.click();
    await vi.waitFor(() => {
      expect(q('ov-selftest-out')?.textContent).toBe('FAIL\nmismatch');
    });
  });

  it('an action that throws is shown, not lost', async () => {
    const { overlay, q } = setup({
      actions: actions({ hash: () => Promise.reject(new Error('worker gone')) }),
    });
    overlay.show();
    q('ov-hash')?.click();
    await vi.waitFor(() => {
      expect(q('ov-hash-out')?.textContent).toBe('FAIL: worker gone');
    });
  });

  it('has a Benchmark button only when the action exists', async () => {
    const without = setup();
    without.overlay.show();
    expect(without.q('ov-benchmark')).toBeNull();

    const withIt = setup({ actions: actions({ benchmark: () => Promise.resolve('Recommended') }) });
    withIt.overlay.show();
    withIt.q('ov-benchmark')?.click();
    await vi.waitFor(() => {
      expect(withIt.q('ov-benchmark-out')?.textContent).toBe('Recommended');
    });
  });

  it('the Pause button calls back, and its label follows the model', () => {
    const togglePause = vi.fn();
    let paused = false;
    const { overlay, timers, q } = setup({
      model: () => model({ paused }),
      actions: actions({ togglePause }),
    });
    overlay.show();
    expect(q('ov-pause')?.textContent).toBe('Pause');
    q('ov-pause')?.click();
    expect(togglePause).toHaveBeenCalledTimes(1);
    paused = true;
    timers.tick();
    expect(q('ov-pause')?.textContent).toBe('Resume');
  });

  it('puts text in as text', () => {
    const { overlay, q, root } = setup({ model: () => model({ adapter: '<b>gpu</b>' }) });
    overlay.show();
    expect(root.querySelector('b')).toBeNull();
    expect(q('ov-adapter')?.dataset['value']).toBe('<b>gpu</b>');
  });
});

describe('the overlay hotkey', () => {
  const chord = { code: 'KeyD', ctrlKey: true, shiftKey: true, altKey: false, metaKey: false };

  it('is Ctrl+Shift+D and nothing near it', () => {
    expect(isOverlayChord(chord)).toBe(true);
    expect(isOverlayChord({ ...chord, ctrlKey: false })).toBe(false);
    expect(isOverlayChord({ ...chord, shiftKey: false })).toBe(false);
    expect(isOverlayChord({ ...chord, altKey: true })).toBe(false);
    expect(isOverlayChord({ ...chord, metaKey: true })).toBe(false);
    expect(isOverlayChord({ ...chord, code: 'KeyS' })).toBe(false);
    expect(isOverlayChord({ ...chord, repeat: true })).toBe(false);
  });

  it('toggles the overlay, swallows the key, and can be removed', () => {
    const target = new EventTarget();
    const overlay = { toggle: vi.fn() };
    const remove = installOverlayHotkey(target, overlay);
    const event = new KeyboardEvent('keydown', {
      code: 'KeyD',
      ctrlKey: true,
      shiftKey: true,
      cancelable: true,
    });
    target.dispatchEvent(event);
    expect(overlay.toggle).toHaveBeenCalledTimes(1);
    expect(event.defaultPrevented).toBe(true);

    target.dispatchEvent(new KeyboardEvent('keydown', { code: 'KeyD', cancelable: true }));
    expect(overlay.toggle).toHaveBeenCalledTimes(1);

    remove();
    target.dispatchEvent(
      new KeyboardEvent('keydown', { code: 'KeyD', ctrlKey: true, shiftKey: true }),
    );
    expect(overlay.toggle).toHaveBeenCalledTimes(1);
  });
});
