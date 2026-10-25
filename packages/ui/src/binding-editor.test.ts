// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { memoryKv } from '@flying/platform-web';
import { GamepadHub, SETTLE_MS } from '@flying/input';
import { BindingEditor, EDITOR_POLL_MS } from './binding-editor.ts';
import { ControlsStore } from './controls-store.ts';
import { type IntervalTimers } from './debug-overlay.ts';
import { EditorModel } from './editor-model.ts';
import { FakePads, fakePad, fakeStick } from './test-pads.ts';

class FakeIntervals implements IntervalTimers {
  readonly active = new Map<number, () => void>();
  #next = 1;
  setInterval(cb: () => void): number {
    this.active.set(this.#next, cb);
    return this.#next++;
  }
  clearInterval(id: number): void {
    this.active.delete(id);
  }
  tick(): void {
    for (const cb of [...this.active.values()]) cb();
  }
}

const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

// An editor left open keeps a key listener on the window: close every one a test made.
const opened: BindingEditor[] = [];
afterEach(() => {
  for (const e of opened.splice(0)) e.close();
  document.body.replaceChildren();
});

async function setup(initial = 'default-km') {
  const root = document.createElement('div');
  document.body.append(root);
  const store = new ControlsStore(memoryKv());
  await store.load();
  const pads = new FakePads();
  const hub = new GamepadHub(pads.get);
  let now = 0;
  const chosen: string[] = [];
  const model = new EditorModel(
    { store, hub, nowMs: () => now, onProfile: (p) => chosen.push(p.id) },
    initial,
  );
  const timers = new FakeIntervals();
  const downloads: { name: string; text: string }[] = [];
  let closed = 0;
  let fileText = '';
  const editor = new BindingEditor(root, {
    model,
    hub,
    timers,
    download: (name, text) => downloads.push({ name, text }),
    readFile: () => Promise.resolve(fileText),
    onClose: () => {
      closed += 1;
    },
  });
  opened.push(editor);
  const q = (id: string): HTMLElement => {
    const el = root.querySelector<HTMLElement>(`[data-testid="${id}"]`);
    if (!el) throw new Error(`no ${id}`);
    return el;
  };
  const has = (id: string): boolean => root.querySelector(`[data-testid="${id}"]`) !== null;
  const press = (code: string): void => {
    window.dispatchEvent(new KeyboardEvent('keydown', { code, cancelable: true }));
  };
  return {
    root,
    store,
    pads,
    hub,
    model,
    editor,
    timers,
    downloads,
    chosen,
    q,
    has,
    press,
    closed: () => closed,
    setFile: (text: string) => {
      fileText = text;
    },
    advance: (ms: number) => {
      now += ms;
    },
  };
}

type Rig = Awaited<ReturnType<typeof setup>>;
const chipsOf = (t: Rig, action: string): string[] =>
  [...t.q(`be-row-${action}`).querySelectorAll('.chip > span:first-child')].map(
    (e) => e.textContent,
  );

describe('BindingEditor: opening and closing', () => {
  it('builds nothing until opened, ticks while open, and removes everything on close', async () => {
    const t = await setup();
    expect(t.root.children).toHaveLength(0);
    expect(t.timers.active.size).toBe(0);
    t.editor.open();
    expect(t.editor.isOpen).toBe(true);
    expect(t.root.querySelectorAll('[role="dialog"]')).toHaveLength(1);
    expect(t.timers.active.size).toBe(1);
    t.editor.open(); // again: no second copy
    expect(t.root.querySelectorAll('[data-testid="binding-editor"]')).toHaveLength(1);
    t.editor.close();
    expect(t.root.children).toHaveLength(0);
    expect(t.timers.active.size).toBe(0);
    expect(EDITOR_POLL_MS).toBeGreaterThan(0);
  });

  it('is a modal dialog with its heading, and the Back button calls onClose', async () => {
    const t = await setup();
    t.editor.open();
    expect(t.q('binding-editor').getAttribute('aria-labelledby')).toBe('be-title');
    expect(document.getElementById('be-title')?.textContent).toBe('Controls');
    expect(document.activeElement).toBe(t.q('be-close'));
    t.q('be-close').click();
    expect(t.closed()).toBe(1);
  });
});

describe('BindingEditor: the list', () => {
  it('shows every group and action with the chips of the profile', async () => {
    const t = await setup();
    t.editor.open();
    for (const g of ['axes', 'engine', 'systems', 'brakes', 'view', 'system']) {
      expect(t.has(`be-group-${g}`), g).toBe(true);
    }
    expect(t.root.querySelectorAll('[data-testid^="be-row-"]')).toHaveLength(24);
    expect(chipsOf(t, 'flight.pitch')).toEqual(['W −', 'S +']);
    expect(chipsOf(t, 'flight.gear')).toEqual(['L']);
    expect(chipsOf(t, 'flight.throttle_up')).toEqual([
      'ShiftLeft',
      'ShiftRight',
      'PageUp',
      'Wheel up',
    ]);
    expect(t.q('be-row-flight.gear').querySelector('.label')?.textContent).toBe('Landing gear');
  });

  it('filters as the person types', async () => {
    const t = await setup();
    t.editor.open();
    const search = t.q('be-search') as HTMLInputElement;
    search.value = 'gear';
    search.dispatchEvent(new Event('input'));
    expect(t.root.querySelectorAll('[data-testid^="be-row-"]')).toHaveLength(1);
    expect(t.has('be-row-flight.gear')).toBe(true);
    expect(t.has('be-group-engine')).toBe(false);
    search.value = 'zzz';
    search.dispatchEvent(new Event('input'));
    expect(t.has('be-none')).toBe(true);
    search.value = '';
    search.dispatchEvent(new Event('input'));
    expect(t.root.querySelectorAll('[data-testid^="be-row-"]')).toHaveLength(24);
  });

  it('offers two add buttons on a bipolar axis, one elsewhere, and disables a full action', async () => {
    const t = await setup();
    t.editor.open();
    expect(t.has('be-add-negative-flight.pitch')).toBe(true);
    expect(t.has('be-add-positive-flight.pitch')).toBe(true);
    expect(t.has('be-add-flight.gear')).toBe(true);
    expect(t.has('be-add-flight.pitch')).toBe(false);
    expect((t.q('be-add-flight.throttle_up') as HTMLButtonElement).disabled).toBe(true);
    expect((t.q('be-add-flight.gear') as HTMLButtonElement).disabled).toBe(false);
  });
});

describe('BindingEditor: profiles', () => {
  it('lists the built-in profiles by name, with the current one selected', async () => {
    const t = await setup('gamepad-standard');
    t.editor.open();
    const select = t.q('be-profile') as HTMLSelectElement;
    expect([...select.options].map((o) => o.textContent)).toEqual([
      'Keyboard and mouse',
      'Gamepad',
      'Joystick or HOTAS',
    ]);
    expect(select.value).toBe('gamepad-standard');
    expect(t.has('be-builtin-note')).toBe(true);
    expect((t.q('be-delete') as HTMLButtonElement).disabled).toBe(true);
    expect((t.q('be-rename') as HTMLButtonElement).disabled).toBe(true);
  });

  it('switches profile and redraws the list', async () => {
    const t = await setup();
    t.editor.open();
    const select = t.q('be-profile') as HTMLSelectElement;
    select.value = 'gamepad-standard';
    select.dispatchEvent(new Event('change'));
    await flush();
    expect(t.chosen.at(-1)).toBe('gamepad-standard');
    expect(chipsOf(t, 'flight.gear')).toEqual(['Pad Button 3', 'L']);
  });

  it('saves the first change to a copy, and the note goes away', async () => {
    const t = await setup();
    t.editor.open();
    t.q(
      'be-remove-flight.gear-' +
        String(t.model.profile.bindings.findIndex((b) => b.action === 'flight.gear')),
    ).click();
    await flush();
    expect(t.model.isBuiltin).toBe(false);
    expect(t.has('be-builtin-note')).toBe(false);
    expect((t.q('be-profile') as HTMLSelectElement).value).toBe('keyboard-and-mouse-custom');
    expect(
      [...(t.q('be-profile') as HTMLSelectElement).options].map((o) => o.textContent),
    ).toContain('Keyboard and mouse (custom)');
    expect(chipsOf(t, 'flight.gear')).toEqual([]);
    expect((t.q('be-rename') as HTMLButtonElement).disabled).toBe(false);
  });

  it('renames, duplicates, creates and deletes from the bar', async () => {
    const t = await setup();
    t.editor.open();
    (t.q('be-name') as HTMLInputElement).value = 'Fighter keys';
    t.q('be-duplicate').click();
    await flush();
    expect(t.model.profile.id).toBe('fighter-keys');
    expect((t.q('be-profile') as HTMLSelectElement).value).toBe('fighter-keys');
    (t.q('be-name') as HTMLInputElement).value = 'Fighter keys 2';
    t.q('be-rename').click();
    await flush();
    expect(t.model.profile.name).toBe('Fighter keys 2');
    (t.q('be-class') as HTMLSelectElement).value = 'joystick';
    (t.q('be-name') as HTMLInputElement).value = 'My stick';
    t.q('be-new').click();
    await flush();
    expect(t.model.profile.id).toBe('my-stick');
    expect(t.model.profile.deviceClass).toBe('joystick');
    t.q('be-delete').click();
    await flush();
    expect(t.model.profile.id).toBe('generic-joystick');
    expect(t.store.find('my-stick')).toBeUndefined();
  });
});

describe('BindingEditor: adding a binding', () => {
  it('waits for a key, shows what it waits for, and adds the chip', async () => {
    const t = await setup();
    t.editor.open();
    expect(t.q('be-capture').hidden).toBe(true);
    t.q('be-add-flight.gear').click();
    expect(t.q('be-capture').hidden).toBe(false);
    expect(t.q('be-capture-prompt').textContent).toBe(
      'Press the key or button, or move the axis, for Landing gear. Esc cancels.',
    );
    expect(t.editor.isCapturing).toBe(true);
    t.press('KeyG');
    await flush();
    expect(t.editor.isCapturing).toBe(false);
    expect(t.q('be-capture').hidden).toBe(true);
    expect(chipsOf(t, 'flight.gear')).toEqual(['L', 'G']);
    expect(t.model.isBuiltin).toBe(false);
  });

  it('adds a key to the end of a bipolar axis that the button names', async () => {
    const t = await setup();
    t.editor.open();
    t.q('be-add-negative-flight.yaw').click();
    t.press('KeyJ');
    await flush();
    expect(chipsOf(t, 'flight.yaw')).toEqual(['Q −', 'E +', 'J −']);
    t.q('be-add-positive-flight.yaw').click();
    t.press('KeyK');
    await flush();
    expect(chipsOf(t, 'flight.yaw')).toEqual(['Q −', 'E +', 'J −', 'K +']);
  });

  it('Esc and the Cancel button give up without adding', async () => {
    const t = await setup();
    t.editor.open();
    t.q('be-add-flight.gear').click();
    t.press('Escape');
    await flush();
    expect(t.editor.isCapturing).toBe(false);
    expect(chipsOf(t, 'flight.gear')).toEqual(['L']);
    t.q('be-add-flight.gear').click();
    t.q('be-capture-cancel').click();
    expect(t.editor.isCapturing).toBe(false);
    expect(t.q('be-capture').hidden).toBe(true);
    expect(t.model.isBuiltin).toBe(true);
  });

  it('takes the mouse buttons and the wheel pressed on the capture box', async () => {
    const t = await setup();
    t.editor.open();
    t.q('be-add-flight.gear').click();
    const down = new Event('pointerdown', { bubbles: true });
    Object.defineProperty(down, 'button', { value: 2 });
    t.q('be-capture').dispatchEvent(down);
    await flush();
    expect(chipsOf(t, 'flight.gear')).toEqual(['L', 'Mouse 3']);
    t.q('be-add-flight.recover').click();
    const wheel = new Event('wheel', { bubbles: true, cancelable: true });
    Object.defineProperty(wheel, 'deltaY', { value: -100 });
    t.q('be-capture').dispatchEvent(wheel);
    await flush();
    expect(chipsOf(t, 'flight.recover')).toEqual(['U', 'Wheel up']);
  });

  it('takes a pad control while it polls', async () => {
    const t = await setup('gamepad-standard');
    t.pads.set(0, fakePad());
    t.hub.poll();
    t.editor.open();
    t.q('be-add-flight.recover').click();
    t.hub.poll();
    t.timers.tick(); // settling
    t.advance(SETTLE_MS + 1);
    t.hub.poll();
    t.timers.tick();
    t.pads.set(0, fakePad({ buttons: { 5: 1 } }));
    t.hub.poll();
    t.timers.tick();
    await flush();
    expect(chipsOf(t, 'flight.recover')).toContain('Pad Button 5');
    expect(t.editor.isCapturing).toBe(false);
  });

  it('says why a key cannot be the throttle lever', async () => {
    const t = await setup();
    t.editor.open();
    t.q('be-add-flight.throttle').click();
    t.press('KeyT');
    await flush();
    expect(t.q('be-rejected').textContent).toBe(
      'The throttle lever needs an axis or an analogue button.',
    );
    expect(chipsOf(t, 'flight.throttle')).toEqual([]);
  });

  it('stops capturing when the editor closes', async () => {
    const t = await setup();
    t.editor.open();
    t.q('be-add-flight.gear').click();
    t.editor.close();
    expect(t.model.capturing).toBeUndefined();
    t.press('KeyG'); // nobody is listening
    await flush();
    expect(t.model.isBuiltin).toBe(true);
  });
});

describe('BindingEditor: removing and restoring', () => {
  it('removes a chip with its button', async () => {
    const t = await setup();
    t.editor.open();
    const at = t.model.profile.bindings.findIndex((b) => b.action === 'flight.gear');
    const remove = t.q(`be-remove-flight.gear-${String(at)}`);
    expect(remove.getAttribute('aria-label')).toBe('Remove L');
    remove.click();
    await flush();
    expect(chipsOf(t, 'flight.gear')).toEqual([]);
  });

  it('restores an action, a group and everything', async () => {
    const t = await setup();
    t.editor.open();
    const at = t.model.profile.bindings.findIndex((b) => b.action === 'flight.gear');
    t.q(`be-remove-flight.gear-${String(at)}`).click();
    expect(chipsOf(t, 'flight.gear')).toEqual([]);
    t.q('be-restore-flight.gear').click();
    expect(chipsOf(t, 'flight.gear')).toEqual(['L']);
    const brakes = t.model.profile.bindings.findIndex((b) => b.action === 'flight.brakes');
    t.q(`be-remove-flight.brakes-${String(brakes)}`).click();
    t.q('be-restore-group-brakes').click();
    expect(chipsOf(t, 'flight.brakes')).toEqual(['B']);
    t.q('be-add-flight.gear').click();
    t.press('KeyG');
    await flush();
    t.q('be-restore-all').click();
    expect(chipsOf(t, 'flight.gear')).toEqual(['L']);
    await flush();
  });
});

describe('BindingEditor: conflicts', () => {
  it('tags both sides of a clash in words, and counts them', async () => {
    const t = await setup();
    t.editor.open();
    expect(t.q('be-conflicts').hidden).toBe(true);
    t.q('be-add-flight.recover').click();
    t.press('KeyL');
    await flush();
    const tags = [...t.root.querySelectorAll('[data-testid="be-conflict"]')];
    expect(tags).toHaveLength(2);
    expect(tags[0]?.textContent).toBe('⚠ Conflict');
    expect(t.q('be-conflicts').textContent).toBe('2 bindings share a control with another action.');
    expect(t.q('be-conflicts').hidden).toBe(false);
  });

  it('tags a key the browser keeps with Ctrl', async () => {
    const t = await setup();
    t.editor.open();
    expect(
      t.q('be-row-flight.pitch').querySelector('[data-testid="be-reserved"]')?.textContent,
    ).toBe('⚠ Browser shortcut with Ctrl');
    expect(
      t.q('be-row-flight.assist_cycle').querySelector('[data-testid="be-reserved"]'),
    ).toBeNull();
  });
});

describe('BindingEditor: shaping an axis', () => {
  it('shows the current values, follows the slider, and stores the change on release', async () => {
    const t = await setup();
    t.editor.open();
    const gain = t.q('be-gain-flight.pitch') as HTMLInputElement;
    expect(gain.value).toBe('1');
    gain.value = '1.5';
    gain.dispatchEvent(new Event('input'));
    expect(gain.parentElement?.querySelector('output')?.textContent).toBe('1.5');
    expect(t.model.isBuiltin).toBe(true); // not stored until the slider is let go
    gain.dispatchEvent(new Event('change'));
    await flush();
    expect(t.model.profile.axes['flight.pitch']?.gain).toBe(1.5);
    expect(t.model.isBuiltin).toBe(false);
    const invert = t.q('be-invert-flight.roll') as HTMLInputElement;
    invert.checked = true;
    invert.dispatchEvent(new Event('change'));
    await flush();
    expect(t.model.profile.axes['flight.roll']?.invert).toBe(true);
  });

  it('has the five sliders and a checkbox on each axis and rate, and none elsewhere', async () => {
    const t = await setup();
    t.editor.open();
    const shaped = [...t.root.querySelectorAll('[data-testid^="be-shape-"]')];
    expect(shaped.map((e) => e.getAttribute('data-testid'))).toEqual([
      'be-shape-flight.pitch',
      'be-shape-flight.roll',
      'be-shape-flight.yaw',
      'be-shape-flight.throttle',
      'be-shape-flight.throttle_up',
      'be-shape-flight.throttle_down',
      'be-shape-flight.brake_left',
      'be-shape-flight.brake_right',
    ]);
    for (const e of shaped) {
      expect(e.querySelectorAll('input[type="range"]')).toHaveLength(5);
      expect(e.querySelectorAll('input[type="checkbox"]')).toHaveLength(1);
    }
  });
});

describe('BindingEditor: files', () => {
  it('exports the profile as a download', async () => {
    const t = await setup();
    t.editor.open();
    t.q('be-export').click();
    expect(t.downloads).toHaveLength(1);
    expect(t.downloads[0]?.name).toBe('default-km.controls.json');
    expect(JSON.parse(t.downloads[0]?.text ?? '')).toMatchObject({ id: 'default-km' });
  });

  it('imports a file, selecting what it holds', async () => {
    const t = await setup();
    t.editor.open();
    t.q('be-export').click();
    t.setFile(t.downloads[0]?.text ?? '');
    const input = t.q('be-import') as HTMLInputElement;
    Object.defineProperty(input, 'files', { value: [new File(['x'], 'p.json')] });
    input.dispatchEvent(new Event('change'));
    await flush();
    await flush();
    expect(t.model.profile.id).toBe('default-km-2');
    expect((t.q('be-profile') as HTMLSelectElement).value).toBe('default-km-2');
    expect(t.q('be-import-errors').hidden).toBe(true);
  });

  it('lists the problems of a bad file by pointer', async () => {
    const t = await setup();
    t.editor.open();
    t.setFile(
      JSON.stringify({
        schema: 'controls/1',
        id: 'x',
        name: 'x',
        deviceClass: 'foot',
        bindings: [],
      }),
    );
    const input = t.q('be-import') as HTMLInputElement;
    Object.defineProperty(input, 'files', { value: [new File(['x'], 'p.json')] });
    input.dispatchEvent(new Event('change'));
    await flush();
    await flush();
    const lines = [...t.root.querySelectorAll('li')].map((e) => e.textContent);
    expect(lines[0]).toBe('The file could not be used: 1 problems.');
    expect(lines[1]).toBe('/deviceClass: expected one of keyboard-mouse, gamepad, joystick');
    expect(t.q('be-import-errors').hidden).toBe(false);
    expect(t.model.profile.id).toBe('default-km');
  });
});

describe('BindingEditor: devices', () => {
  it('says when no controller is found, and lists one that appears', async () => {
    const t = await setup();
    t.editor.open();
    expect(t.q('be-no-devices').textContent).toBe(
      'No game controller found. Press a button on it to wake it up.',
    );
    t.pads.set(0, fakePad());
    t.hub.poll();
    t.timers.tick();
    expect(t.has('be-no-devices')).toBe(false);
    expect(t.q('be-device-0').textContent).toContain('Xbox Wireless Controller');
    t.pads.set(0, null);
    t.hub.poll();
    t.timers.tick();
    expect(t.has('be-no-devices')).toBe(true);
  });

  it('shows the live position of the axes, the pressed buttons and the hat', async () => {
    const t = await setup();
    t.editor.open();
    t.pads.set(0, fakePad({ axes: { 2: 0.5 }, buttons: { 3: 1, 7: 1, 12: 1 } }));
    t.hub.poll();
    t.timers.tick();
    t.timers.tick();
    const axis = t.q('be-axis-0-2') as HTMLMeterElement;
    expect(axis.getAttribute('value')).toBe('0.5');
    expect(t.q('be-pressed-0').textContent).toBe('buttons 3 7 12 ↑');
    t.pads.set(0, fakePad({ axes: { 2: -0.25 } }));
    t.hub.poll();
    t.timers.tick();
    expect(t.q('be-axis-0-2').getAttribute('value')).toBe('-0.25');
    expect(t.q('be-pressed-0').textContent).toBe('');
  });

  it('calibrates a stick from the page and stores the result', async () => {
    const t = await setup('generic-joystick');
    t.pads.set(0, fakeStick({ axes: { 0: 0.05 } }));
    t.hub.poll();
    t.editor.open();
    t.q('be-calibrate-0').click();
    expect(t.q('be-calibrate-text').textContent).toBe(
      'Leave every stick and lever at rest, wait a moment, then press Next.',
    );
    for (let i = 0; i < 12; i++) t.timers.tick();
    t.q('be-calibrate-next').click();
    expect(t.q('be-calibrate-text').textContent).toBe(
      'Move every stick and lever through its full travel, then press Done.',
    );
    t.pads.set(0, fakeStick({ axes: { 0: -0.9 } }));
    t.hub.poll();
    t.timers.tick();
    t.pads.set(0, fakeStick({ axes: { 0: 0.95 } }));
    t.hub.poll();
    t.timers.tick();
    t.q('be-calibrate-next').click();
    await flush();
    expect(t.has('be-calibration')).toBe(false);
    expect(t.model.profile.calibration['joy:044f:b10a:a:0']).toBeDefined();
  });

  it('cancels a calibration', async () => {
    const t = await setup();
    t.pads.set(0, fakeStick());
    t.hub.poll();
    t.editor.open();
    t.q('be-calibrate-0').click();
    t.q('be-calibrate-cancel').click();
    expect(t.has('be-calibration')).toBe(false);
    expect(t.model.calibrating).toBeUndefined();
  });
});
