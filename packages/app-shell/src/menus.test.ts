// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { GamepadHub, InputPump, KeyboardState } from '@flying/input';
import { memoryKv } from '@flying/platform-web';
import {
  ASSIST_LEVEL,
  BUTTON,
  decodeInputFrame,
  type InputSink,
  SPAWN_KIND,
} from '@flying/sim-bridge';
import { ControlsStore, type DemoMenu, SettingsStore } from '@flying/ui';
import { MenuController, type SettingsOverrides } from './menus.ts';

const created: MenuController[] = [];
afterEach(() => {
  for (const m of created.splice(0)) m.destroy();
  document.body.replaceChildren();
});

async function setup(overrides: SettingsOverrides = {}) {
  const root = document.createElement('div');
  document.body.append(root);
  const frames: Uint8Array[] = [];
  const sink: InputSink = {
    send: (frame) => {
      frames.push(frame.slice());
      return true;
    },
    close: () => undefined,
  };
  let mono = 1000;
  const pump = new InputPump({
    sink,
    keyboard: new KeyboardState(),
    nowMs: () => mono,
    sharedNowMs: () => mono,
    minIntervalMs: 8,
  });
  const settings = new SettingsStore(memoryKv());
  const store = new ControlsStore(memoryKv());
  await Promise.all([settings.load(), store.load()]);
  const log = {
    holds: [] as boolean[],
    menus: [] as DemoMenu[],
    released: 0,
    synced: 0,
    patches: [] as unknown[],
    profiles: [] as string[],
  };
  const menus = new MenuController({
    root,
    settings,
    store,
    pads: new GamepadHub(() => []),
    pump,
    overrides,
    assistInForce: () => ASSIST_LEVEL.STANDARD,
    hold: (on) => log.holds.push(on),
    setMenu: (menu) => log.menus.push(menu),
    releaseMouse: () => {
      log.released += 1;
    },
    syncMouseLock: () => {
      log.synced += 1;
    },
    onProfile: (p) => log.profiles.push(p.id),
    onSettings: (patch) => log.patches.push(patch),
    nowMs: () => mono,
  });
  created.push(menus);
  const id = (testid: string): HTMLElement => {
    const el = root.querySelector<HTMLElement>(`[data-testid="${testid}"]`);
    if (!el) throw new Error(`no ${testid}`);
    return el;
  };
  /** Runs the pump long enough to send everything pending. */
  const run = (): void => {
    for (let i = 0; i < 4; i++) {
      mono += 10;
      pump.update();
    }
  };
  return { root, menus, pump, settings, store, log, frames, id, run };
}

describe('MenuController', () => {
  it('opens the pause menu: holds the sim, silences the flight controls, gives the pointer back', async () => {
    const r = await setup();
    r.menus.openPause();
    expect(r.menus.current).toBe('pause');
    expect(r.menus.isOpen).toBe(true);
    expect(r.id('pause-menu')).toBeTruthy();
    expect(r.pump.context).toBe('menu');
    expect(r.log.holds).toEqual([true]);
    expect(r.log.released).toBe(1);
    expect(r.log.menus).toEqual(['pause']);
  });

  it('closes it again: the sim runs, the controls are back, the mouse is taken if it flies', async () => {
    const r = await setup();
    r.menus.openPause();
    r.menus.close();
    expect(r.menus.current).toBeNull();
    expect(r.root.querySelector('[data-testid="pause-menu"]')).toBeNull();
    expect(r.pump.context).toBe('flight');
    expect(r.log.holds).toEqual([true, false]);
    expect(r.log.synced).toBe(1);
    expect(r.log.menus).toEqual(['pause', null]);
  });

  it('closing with nothing open does nothing', async () => {
    const r = await setup();
    r.menus.close();
    expect(r.log.holds).toEqual([]);
    expect(r.log.menus).toEqual([]);
  });

  it('the pause key opens the menu, and resumes from it', async () => {
    const r = await setup();
    r.menus.toggle();
    expect(r.menus.current).toBe('pause');
    r.menus.toggle();
    expect(r.menus.current).toBeNull();
  });

  it('the Resume button resumes', async () => {
    const r = await setup();
    r.menus.openPause();
    r.id('pause-resume').click();
    expect(r.menus.current).toBeNull();
  });

  it('the pause key from the editor steps back to the pause menu, then resumes', async () => {
    const r = await setup();
    r.menus.openPause();
    r.id('pause-controls').click();
    expect(r.menus.current).toBe('controls');
    expect(r.id('binding-editor')).toBeTruthy();
    expect(r.root.querySelector('[data-testid="pause-menu"]')).toBeNull();
    r.menus.toggle();
    expect(r.menus.current).toBe('pause');
    expect(r.root.querySelector('[data-testid="binding-editor"]')).toBeNull();
    r.menus.toggle();
    expect(r.menus.current).toBeNull();
  });

  it('keeps the sim held and the controls silent all the way from the menu into the editor', async () => {
    const r = await setup();
    r.menus.openPause();
    r.menus.openControls();
    expect(r.log.holds).toEqual([true]);
    expect(r.pump.context).toBe('menu');
    r.id('be-close').click();
    expect(r.menus.current).toBe('pause');
    expect(r.log.holds).toEqual([true]);
    r.menus.close();
    expect(r.log.holds).toEqual([true, false]);
  });

  it('applies a change from the menu and stores it', async () => {
    const r = await setup();
    r.menus.openPause();
    r.id(`pause-assist-${String(ASSIST_LEVEL.MANUAL)}`).click();
    expect(r.log.patches).toEqual([{ assist: ASSIST_LEVEL.MANUAL }]);
    expect(r.settings.value.assist).toBe(ASSIST_LEVEL.MANUAL);
    expect(r.menus.settings().assist).toBe(ASSIST_LEVEL.MANUAL);
    expect(r.id(`pause-assist-${String(ASSIST_LEVEL.MANUAL)}`).getAttribute('aria-pressed')).toBe(
      'true',
    );
  });

  it('shows what the URL set until the player changes it', async () => {
    const r = await setup({ assist: ASSIST_LEVEL.MANUAL, spawn: SPAWN_KIND.AIR });
    expect(r.settings.value.assist).toBe(ASSIST_LEVEL.STANDARD);
    expect(r.menus.settings().assist).toBe(ASSIST_LEVEL.MANUAL);
    expect(r.menus.settings().spawn).toBe(SPAWN_KIND.AIR);
    r.menus.openPause();
    expect(r.id(`pause-assist-${String(ASSIST_LEVEL.MANUAL)}`).getAttribute('aria-pressed')).toBe(
      'true',
    );
    r.id(`pause-assist-${String(ASSIST_LEVEL.FULL)}`).click();
    expect(r.menus.settings().assist).toBe(ASSIST_LEVEL.FULL);
    // The spawn was not touched, so the URL still holds for it.
    expect(r.menus.settings().spawn).toBe(SPAWN_KIND.AIR);
  });

  it('stores a change the controls already made without applying it again', async () => {
    const r = await setup({ assist: ASSIST_LEVEL.MANUAL });
    r.menus.persist({ assist: ASSIST_LEVEL.REDUCED });
    expect(r.log.patches).toEqual([]);
    expect(r.settings.value.assist).toBe(ASSIST_LEVEL.REDUCED);
    expect(r.menus.settings().assist).toBe(ASSIST_LEVEL.REDUCED);
  });

  it('respawns from the menu: flies on, and presses the respawn button once', async () => {
    const r = await setup();
    r.run();
    r.menus.openPause();
    r.id('pause-respawn').click();
    expect(r.menus.current).toBeNull();
    r.run();
    const pressed = r.frames
      .map((f) => decodeInputFrame(f).buttons)
      .filter((b) => (b & (1n << BigInt(BUTTON.RESET))) !== 0n);
    expect(pressed).toHaveLength(1);
  });

  it('opens the editor on the profile in use and flies with the one the player picks', async () => {
    const r = await setup();
    r.menus.openControls();
    const select = r.id('be-profile') as HTMLSelectElement;
    expect(select.value).toBe(r.pump.profile.id);
    select.value = 'gamepad-standard';
    select.dispatchEvent(new Event('change'));
    expect(r.log.profiles).toEqual(['gamepad-standard']);
  });
});
