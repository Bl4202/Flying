// @vitest-environment happy-dom
import { afterEach, describe, expect, it } from 'vitest';
import { builtinProfile, type ControlsProfile } from '@flying/input';
import { memoryKv } from '@flying/platform-web';
import {
  applySettingsPatch,
  ControlsStore,
  DEFAULT_SETTINGS,
  flowReducer,
  type DetectedDevice,
  type FlowEvent,
  type FlowState,
  type Settings,
  type SettingsPatch,
} from '@flying/ui';
import { deviceOfProfile, kindOfMapping, ScreenController } from './screens.ts';

const created: ScreenController[] = [];
afterEach(() => {
  for (const s of created.splice(0)) s.destroy();
  document.body.replaceChildren();
});

async function setup(start: FlowState = { name: 'MAIN_MENU', screen: 'home' }) {
  const root = document.createElement('div');
  document.body.append(root);
  let settings: Settings = applySettingsPatch(DEFAULT_SETTINGS, {});
  let flow: FlowState = start;
  let profile: ControlsProfile = builtinProfile('default-km');
  let found: DetectedDevice[] = [];
  const store = new ControlsStore(memoryKv());
  await store.load();
  const log = {
    flights: 0,
    setupDone: 0,
    resets: 0,
    changes: [] as SettingsPatch[],
    replaced: [] as Settings[],
    profiles: [] as string[],
    saved: [] as string[],
    events: [] as FlowEvent[],
    editors: [] as (() => void)[],
  };
  const screens = new ScreenController({
    root,
    settings: () => settings,
    onChange: (patch) => {
      log.changes.push(patch);
      settings = applySettingsPatch(settings, patch);
    },
    onReplace: (next) => {
      log.replaced.push(next);
      settings = next;
    },
    onReset: () => {
      log.resets += 1;
      settings = applySettingsPatch(DEFAULT_SETTINGS, { setupDone: true });
    },
    onSetupDone: () => {
      log.setupDone += 1;
    },
    flow: {
      dispatch: (event) => {
        log.events.push(event);
        flow = flowReducer(flow, event);
        return flow;
      },
    },
    store,
    devices: () => found,
    profile: () => profile,
    useProfile: (p) => {
      log.profiles.push(p.id);
      profile = p;
    },
    openControls: (back) => {
      log.editors.push(back);
    },
    onFly: () => {
      log.flights += 1;
    },
    save: (name) => log.saved.push(name),
    info: { storage: 'memory', build: 'deadbeefcafe' },
  });
  created.push(screens);
  const q = (id: string): HTMLElement => {
    const el = root.querySelector<HTMLElement>(`[data-testid="${id}"]`);
    if (!el) throw new Error(`no ${id}`);
    return el;
  };
  const has = (id: string): boolean => root.querySelector(`[data-testid="${id}"]`) !== null;
  return {
    root,
    screens,
    log,
    q,
    has,
    store,
    flow: () => flow,
    settings: () => settings,
    plug: (d: DetectedDevice[]) => {
      found = d;
    },
  };
}

describe('the helpers', () => {
  it('sort profiles and pads into devices', () => {
    expect(deviceOfProfile({ deviceClass: 'keyboard-mouse' })).toBe('km');
    expect(deviceOfProfile({ deviceClass: 'gamepad' })).toBe('gamepad');
    expect(deviceOfProfile({ deviceClass: 'joystick' })).toBe('joystick');
    expect(kindOfMapping('standard')).toBe('gamepad');
    expect(kindOfMapping('')).toBe('joystick');
  });
});

describe('ScreenController: the main menu', () => {
  it('shows the menu for MAIN_MENU and nothing for the flight', async () => {
    const r = await setup();
    r.screens.show({ name: 'MAIN_MENU', screen: 'home' });
    expect(r.has('main-menu')).toBe(true);
    expect(r.screens.active).toBe(true);
    r.screens.show({ name: 'DEMO', paused: false, menu: null });
    expect(r.root.childNodes).toHaveLength(0);
    expect(r.screens.active).toBe(false);
  });

  it('flies from Fly, and a change of assist or arena goes to the settings', async () => {
    const r = await setup();
    r.screens.show(r.flow());
    r.q('menu-fly').click();
    expect(r.log.flights).toBe(1);
    r.q('menu-arena-1').click();
    expect(r.log.changes).toEqual([{ arena: 1 }]);
  });

  it('goes to the settings and back, one screen at a time', async () => {
    const r = await setup();
    r.screens.show(r.flow());
    r.q('menu-settings').click();
    expect(r.flow()).toEqual({ name: 'MAIN_MENU', screen: 'settings' });
    expect(r.has('settings-screen')).toBe(true);
    expect(r.has('main-menu')).toBe(false);
    r.q('settings-close').click();
    expect(r.has('main-menu')).toBe(true);
    expect(r.has('settings-screen')).toBe(false);
  });

  it('backs out of the settings with Esc, and leaves the menu and the first run alone', async () => {
    const r = await setup();
    r.screens.show(r.flow());
    r.screens.escape();
    expect(r.has('main-menu')).toBe(true);
    r.q('menu-settings').click();
    r.screens.escape();
    expect(r.flow()).toEqual({ name: 'MAIN_MENU', screen: 'home' });
    expect(r.has('main-menu')).toBe(true);
    const setupFlow = await setup({ name: 'FIRST_RUN', step: 'device' });
    setupFlow.screens.show(setupFlow.flow());
    setupFlow.screens.escape();
    expect(setupFlow.has('first-run')).toBe(true);
  });
});

describe('ScreenController: the first run', () => {
  it('walks the steps, and finishing records that the setup is done and opens the menu', async () => {
    const r = await setup({ name: 'FIRST_RUN', step: 'device' });
    r.screens.show(r.flow());
    expect(r.q('firstrun-step').textContent).toBe('Step 1 of 5');
    for (const step of ['assist', 'comfort', 'notice', 'done']) {
      r.q('firstrun-next').click();
      expect(r.flow()).toEqual({ name: 'FIRST_RUN', step });
    }
    expect(r.log.setupDone).toBe(0);
    r.q('firstrun-next').click();
    expect(r.log.setupDone).toBe(1);
    expect(r.flow()).toEqual({ name: 'MAIN_MENU', screen: 'home' });
    expect(r.has('main-menu')).toBe(true);
    expect(r.has('first-run')).toBe(false);
  });

  it('skips from any step, and the setup counts as done', async () => {
    const r = await setup({ name: 'FIRST_RUN', step: 'device' });
    r.screens.show(r.flow());
    r.q('firstrun-next').click();
    r.q('firstrun-skip').click();
    expect(r.log.setupDone).toBe(1);
    expect(r.flow().name).toBe('MAIN_MENU');
  });

  it('chooses the controls profile for the device, and shows the calibration step for a joystick', async () => {
    const r = await setup({ name: 'FIRST_RUN', step: 'device' });
    r.screens.show(r.flow());
    r.q('firstrun-device-joystick').click();
    expect(r.log.profiles).toEqual(['generic-joystick']);
    expect(r.store.activeId).toBe('generic-joystick');
    expect(r.q('firstrun-step').textContent).toBe('Step 1 of 6');
    r.q('firstrun-next').click();
    expect(r.flow()).toEqual({ name: 'FIRST_RUN', step: 'calibration' });
  });

  it('lists the pads that are connected', async () => {
    const r = await setup({ name: 'FIRST_RUN', step: 'device' });
    r.plug([{ kind: 'gamepad', name: 'Pad One' }]);
    r.screens.show(r.flow());
    expect(r.q('firstrun-detected').textContent).toContain('Pad One');
  });

  it('opens the controls editor from the calibration step and comes back to the same step', async () => {
    const r = await setup({ name: 'FIRST_RUN', step: 'calibration' });
    r.screens.show({ name: 'FIRST_RUN', step: 'calibration' });
    r.q('firstrun-calibrate').click();
    expect(r.log.editors).toHaveLength(1);
    expect(r.has('first-run')).toBe(false);
    r.log.editors[0]?.();
    expect(r.has('first-run')).toBe(true);
    expect(r.q('first-run').dataset['step']).toBe('calibration');
  });

  it('runs again from the settings, from the first step', async () => {
    const r = await setup();
    r.screens.show(r.flow());
    r.q('menu-settings').click();
    r.q('settings-tab-data').click();
    r.q('settings-setup').click();
    expect(r.flow()).toEqual({ name: 'FIRST_RUN', step: 'device' });
    expect(r.has('first-run')).toBe(true);
    expect(r.has('settings-screen')).toBe(false);
  });
});

describe('ScreenController: the settings', () => {
  it('passes a change on, and refreshes the screen when settings are reset', async () => {
    const r = await setup({ name: 'MAIN_MENU', screen: 'settings' });
    r.screens.show(r.flow());
    r.q('settings-camera-chase').click();
    expect(r.log.changes).toEqual([{ camera: 'chase' }]);
    r.q('settings-tab-data').click();
    r.q('settings-reset-all').click();
    r.q('settings-reset-all').click();
    expect(r.log.resets).toBe(1);
    r.q('settings-tab-gameplay').click();
    expect(r.q('settings-camera-cockpit').getAttribute('aria-pressed')).toBe('true');
  });

  it('opens the editor over the Controls tab and returns to that tab', async () => {
    const r = await setup({ name: 'MAIN_MENU', screen: 'settings' });
    r.screens.show(r.flow());
    r.q('settings-tab-controls').click();
    expect(r.q('settings-profile').textContent).toBe('Keyboard and mouse');
    r.q('settings-controls').click();
    expect(r.has('settings-screen')).toBe(false);
    r.log.editors[0]?.();
    expect(r.has('settings-screen')).toBe(true);
    expect(r.q('settings-tab-controls').getAttribute('aria-selected')).toBe('true');
  });

  it('gives the data tab the storage and the build', async () => {
    const r = await setup({ name: 'MAIN_MENU', screen: 'settings' });
    r.screens.show(r.flow());
    r.q('settings-tab-data').click();
    expect(r.q('settings-storage').textContent).toContain('memory');
    expect(r.q('settings-build').textContent).toBe('Build deadbeef');
    r.q('settings-export').click();
    expect(r.log.saved).toEqual(['flying-settings.json']);
  });
});
