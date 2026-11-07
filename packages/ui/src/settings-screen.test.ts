// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { ARENA } from '@flying/sim-bridge';
import {
  SETTINGS_TABS,
  SettingsScreen,
  tabDefaults,
  type SettingsScreenDeps,
  type SettingsTab,
} from './settings-screen.ts';
import {
  applySettingsPatch,
  DEFAULT_SETTINGS,
  exportSettings,
  type Settings,
  type SettingsPatch,
} from './settings.ts';

const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

function setup(extra: Partial<SettingsScreenDeps> = {}) {
  const root = document.createElement('div');
  document.body.append(root);
  let settings: Settings = applySettingsPatch(DEFAULT_SETTINGS, { setupDone: true });
  const changes: SettingsPatch[] = [];
  const calls = {
    close: 0,
    controls: 0,
    setup: 0,
    reset: 0,
    replaced: [] as Settings[],
    saved: [] as { name: string; text: string }[],
  };
  const screen = new SettingsScreen(root, {
    settings: () => settings,
    onChange: (patch) => {
      changes.push(patch);
      settings = applySettingsPatch(settings, patch);
    },
    onReplace: (next) => {
      calls.replaced.push(next);
      settings = next;
    },
    onReset: () => {
      calls.reset += 1;
      settings = applySettingsPatch(DEFAULT_SETTINGS, { setupDone: true });
    },
    onClose: () => {
      calls.close += 1;
    },
    onControls: () => {
      calls.controls += 1;
    },
    onRunSetup: () => {
      calls.setup += 1;
    },
    save: (name, text) => {
      calls.saved.push({ name, text });
    },
    profileName: () => 'Keyboard and mouse',
    info: { storage: 'indexeddb', build: 'abcdef0123456789' },
    ...extra,
  });
  const q = (id: string): HTMLElement => {
    const el = root.querySelector<HTMLElement>(`[data-testid="${id}"]`);
    if (!el) throw new Error(`no ${id}`);
    return el;
  };
  const has = (id: string): boolean => root.querySelector(`[data-testid="${id}"]`) !== null;
  const slide = (id: string, value: number): void => {
    const input = q(id) as HTMLInputElement;
    input.value = String(value);
    input.dispatchEvent(new Event('input'));
  };
  const key = (target: HTMLElement, k: string): KeyboardEvent => {
    const e = new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true });
    target.dispatchEvent(e);
    return e;
  };
  return { root, screen, calls, changes, q, has, slide, key, settings: () => settings };
}

describe('the tabs', () => {
  it('are a tablist of six with one selected, the first by default', () => {
    const { screen, root, q } = setup();
    screen.open();
    const tabs = [...root.querySelectorAll<HTMLElement>('[role="tab"]')];
    expect(tabs.map((t) => t.textContent)).toEqual([
      'Gameplay',
      'Controls',
      'Video',
      'Audio',
      'Accessibility',
      'Data',
    ]);
    expect(tabs.map((t) => t.getAttribute('aria-selected'))).toEqual([
      'true',
      'false',
      'false',
      'false',
      'false',
      'false',
    ]);
    // Only the selected tab is in the Tab order (a roving tabindex).
    expect(tabs.map((t) => t.tabIndex)).toEqual([0, -1, -1, -1, -1, -1]);
    expect(root.querySelector('[role="tablist"]')).not.toBeNull();
    expect(q('settings-panel-gameplay').getAttribute('role')).toBe('tabpanel');
    expect(document.activeElement).toBe(q('settings-tab-gameplay'));
  });

  it('move with the arrows, wrapping, and with Home and End', () => {
    const { screen, q, key } = setup();
    screen.open();
    key(q('settings-tab-gameplay'), 'ArrowRight');
    expect(screen.tab).toBe('controls');
    expect(document.activeElement).toBe(q('settings-tab-controls'));
    key(q('settings-tab-controls'), 'ArrowLeft');
    key(q('settings-tab-gameplay'), 'ArrowLeft');
    expect(screen.tab).toBe('data');
    key(q('settings-tab-data'), 'ArrowRight');
    expect(screen.tab).toBe('gameplay');
    key(q('settings-tab-gameplay'), 'End');
    expect(screen.tab).toBe('data');
    key(q('settings-tab-data'), 'Home');
    expect(screen.tab).toBe('gameplay');
    expect(key(q('settings-tab-gameplay'), 'x').defaultPrevented).toBe(false);
  });

  it('build one panel at a time', () => {
    const { screen, root, q } = setup();
    screen.open();
    for (const tab of SETTINGS_TABS) {
      q(`settings-tab-${tab}`).click();
      expect(root.querySelectorAll('[role="tabpanel"]')).toHaveLength(1);
      expect(root.querySelector('[role="tabpanel"]')?.getAttribute('data-testid')).toBe(
        `settings-panel-${tab}`,
      );
    }
  });

  it('open at the tab asked for, and move there if already open', () => {
    const { screen, q } = setup();
    screen.open('audio');
    expect(screen.tab).toBe('audio');
    expect(q('settings-tab-audio').getAttribute('aria-selected')).toBe('true');
    screen.open('video');
    expect(screen.tab).toBe('video');
  });

  it('close removes everything, and close sends the player back', () => {
    const { screen, root, q, calls } = setup();
    screen.open();
    q('settings-close').click();
    expect(calls.close).toBe(1);
    screen.close();
    expect(root.childNodes).toHaveLength(0);
    expect(screen.isOpen).toBe(false);
    screen.select('video'); // nothing open: nothing happens
    expect(root.childNodes).toHaveLength(0);
  });
});

describe('Gameplay', () => {
  it('changes the arena, the camera, the field of view and the mouse', () => {
    const { screen, q, slide, changes } = setup();
    screen.open();
    q(`settings-arena-${String(ARENA.PLAIN)}`).click();
    q('settings-camera-chase').click();
    slide('settings-fov', 100);
    q('settings-mouse-aim').click();
    expect(changes).toEqual([
      { arena: ARENA.PLAIN },
      { camera: 'chase' },
      { fovDeg: 100 },
      { mouseMode: 'aim' },
    ]);
    expect(q('settings-fov-value').textContent).toBe('100°');
  });
});

describe('Controls', () => {
  it('names the profile in use and opens the editor', () => {
    const { screen, q, calls } = setup();
    screen.open('controls');
    expect(q('settings-profile').textContent).toBe('Keyboard and mouse');
    q('settings-controls').click();
    expect(calls.controls).toBe(1);
  });
});

describe('Video', () => {
  it('changes the renderer, the quality preset, the scale and the HUD, each inside its group', () => {
    const { screen, q, slide, changes, settings } = setup();
    screen.open('video');
    q('settings-renderer-webgl2').click();
    q('settings-preset-low').click();
    slide('settings-scale', 130);
    q('settings-palette-amber').click();
    slide('settings-brightness', 60);
    q('settings-declutter-2').click();
    slide('settings-overlay-scale', 150);
    expect(changes).toEqual([
      { video: { renderer: 'webgl2' } },
      { video: { preset: 'low' } },
      { video: { resolutionScale: 130 } },
      { hud: { palette: 'amber' } },
      { hud: { brightness: 60 } },
      { hud: { declutter: 2 } },
      { hud: { overlayScale: 150 } },
    ]);
    expect(settings().video).toEqual({ renderer: 'webgl2', preset: 'low', resolutionScale: 130 });
    expect(settings().hud).toEqual({
      palette: 'amber',
      brightness: 60,
      declutter: 2,
      overlayScale: 150,
    });
  });

  it('says the renderer is chosen at the next start', () => {
    const { screen, q } = setup();
    screen.open('video');
    expect(q('settings-renderer-note').textContent).toContain('when the game starts');
  });
});

describe('Audio', () => {
  it('has the four volumes and the mute-when-hidden switch', () => {
    const { screen, q, slide, changes, settings } = setup();
    screen.open('audio');
    slide('settings-master', 30);
    slide('settings-aircraft', 40);
    slide('settings-warnings', 90);
    slide('settings-ui', 10);
    const mute = q('settings-mute-hidden') as HTMLInputElement;
    expect(mute.checked).toBe(true);
    mute.checked = false;
    mute.dispatchEvent(new Event('change'));
    expect(changes).toEqual([
      { audio: { master: 30 } },
      { audio: { aircraft: 40 } },
      { audio: { warnings: 90 } },
      { audio: { ui: 10 } },
      { audio: { muteWhenHidden: false } },
    ]);
    expect(settings().audio).toEqual({
      master: 30,
      aircraft: 40,
      warnings: 90,
      ui: 10,
      muteWhenHidden: false,
    });
  });
});

describe('Accessibility', () => {
  it('has reduced motion, camera shake, the screen effect strength and a high-contrast HUD', () => {
    const { screen, q, slide, changes } = setup();
    screen.open('accessibility');
    q('settings-motion-on').click();
    slide('settings-shake', 0);
    slide('settings-strength', 50);
    q('settings-palette-high-contrast').click();
    expect(changes).toEqual([
      { reducedMotion: 'on' },
      { cameraShake: 0 },
      { gfxStrength: 50 },
      { hud: { palette: 'high-contrast' } },
    ]);
  });
});

describe('every setting has a default and a reset', () => {
  it('puts a tab back to the defaults of what it shows, and nothing else', () => {
    const { screen, q, slide, settings } = setup();
    screen.open('video');
    q('settings-renderer-webgl2').click();
    slide('settings-brightness', 40);
    q('settings-reset-tab').click();
    expect(settings().video).toEqual(DEFAULT_SETTINGS.video);
    expect(settings().hud).toEqual(DEFAULT_SETTINGS.hud);
    expect(q('settings-brightness-value').textContent).toBe('100%');
    expect(q('settings-renderer-auto').getAttribute('aria-pressed')).toBe('true');
    screen.select('audio');
    slide('settings-master', 5);
    q('settings-reset-tab').click();
    expect(settings().audio).toEqual(DEFAULT_SETTINGS.audio);
    // The first-run flag is not on any tab.
    expect(settings().setupDone).toBe(true);
  });

  it('covers every setting that is not a flag: each is on some tab with a reset', () => {
    const covered = new Set<string>();
    for (const tab of SETTINGS_TABS) {
      const patch = tabDefaults(tab);
      if (patch === undefined) continue;
      for (const [k, v] of Object.entries(patch)) {
        if (typeof v === 'object') for (const inner of Object.keys(v)) covered.add(`${k}.${inner}`);
        else covered.add(k);
      }
    }
    const expected = [
      'assist',
      'gEffects',
      'gfxStrength',
      'spawn',
      'arena',
      'mouseMode',
      'mouseSensitivity',
      'mouseSpring',
      'camera',
      'fovDeg',
      'cameraShake',
      'reducedMotion',
      ...Object.keys(DEFAULT_SETTINGS.hud).map((k) => `hud.${k}`),
      ...Object.keys(DEFAULT_SETTINGS.video).map((k) => `video.${k}`),
      ...Object.keys(DEFAULT_SETTINGS.audio).map((k) => `audio.${k}`),
    ];
    // autoGcas is reserved (it does nothing yet), so it has no control; version and setupDone are not settings a tab shows.
    expect(expected.filter((k) => !covered.has(k))).toEqual([]);
    expect(tabDefaults('controls')).toBeUndefined();
    expect(tabDefaults('data')).toBeUndefined();
  });
});

describe('Data', () => {
  it('exports the settings as a file the player keeps', () => {
    const { screen, q, calls, settings } = setup();
    screen.open('data');
    q('settings-export').click();
    expect(calls.saved).toEqual([
      { name: 'flying-settings.json', text: exportSettings(settings()) },
    ]);
    expect(q('settings-status').textContent).toBe('Saved as flying-settings.json.');
  });

  const chooseFile = async (q: (id: string) => HTMLElement, text: string): Promise<void> => {
    const input = q('settings-import') as HTMLInputElement;
    Object.defineProperty(input, 'files', {
      value: [new File([text], 'x.json')],
      configurable: true,
    });
    input.dispatchEvent(new Event('change'));
    await flush();
  };

  it('imports a file, replacing every setting; the file the export wrote comes back identical', async () => {
    const custom = applySettingsPatch(DEFAULT_SETTINGS, {
      camera: 'chase',
      hud: { palette: 'cyan' },
      setupDone: true,
    });
    const { screen, q, calls, settings } = setup({
      readFile: () => Promise.resolve(exportSettings(custom)),
    });
    screen.open('data');
    await chooseFile(q, 'ignored');
    expect(calls.replaced).toHaveLength(1);
    expect(settings()).toEqual(custom);
    expect(q('settings-status').textContent).toBe('Settings imported.');
    expect(exportSettings(settings())).toBe(exportSettings(custom));
  });

  it('lists every problem with an import, by pointer, and changes nothing', async () => {
    const bad = JSON.stringify({ version: 2, camera: 'orbit', hud: { brightness: 900 } });
    const { screen, q, calls, root } = setup({ readFile: () => Promise.resolve(bad) });
    screen.open('data');
    await chooseFile(q, 'ignored');
    expect(calls.replaced).toHaveLength(0);
    const items = [...root.querySelectorAll('[data-testid="settings-problems"] li')].map(
      (li) => li.textContent,
    );
    expect(items).toHaveLength(2);
    expect(items[0]).toContain('/camera');
    expect(items[1]).toContain('/hud/brightness');
    expect(q('settings-status').textContent).toBe('');
  });

  it('says a file that is not JSON is not', async () => {
    const { screen, q, root } = setup({ readFile: () => Promise.resolve('<html>') });
    screen.open('data');
    await chooseFile(q, 'ignored');
    expect(root.querySelector('[data-testid="settings-problems"] li')?.textContent).toContain(
      'not JSON',
    );
  });

  it('resets everything only on the second press', () => {
    const { screen, q, calls } = setup();
    screen.open('data');
    q('settings-reset-all').click();
    expect(calls.reset).toBe(0);
    expect(q('settings-reset-all').textContent).toBe('Select again to reset every setting.');
    q('settings-reset-all').click();
    expect(calls.reset).toBe(1);
    expect(q('settings-reset-all').textContent).toBe('Reset all settings');
    expect(q('settings-status').textContent).toBe('Settings reset.');
  });

  it('forgets the first press when focus leaves', () => {
    const { screen, q, calls } = setup();
    screen.open('data');
    q('settings-reset-all').click();
    q('settings-reset-all').dispatchEvent(new Event('blur'));
    q('settings-reset-all').click();
    expect(calls.reset).toBe(0);
  });

  it('runs the setup again, and shows where the settings are kept and the build', () => {
    const { screen, q, calls } = setup();
    screen.open('data');
    q('settings-setup').click();
    expect(calls.setup).toBe(1);
    expect(q('settings-storage').textContent).toBe('Settings are kept in: indexeddb');
    expect(q('settings-build').textContent).toBe('Build abcdef01');
  });
});

describe('the screen as a whole', () => {
  it('keeps Tab inside, wrapping at both ends', () => {
    const { screen, root, q } = setup();
    screen.open();
    const controls = [...root.querySelectorAll<HTMLElement>('button, input')].filter(
      (el) => el.tabIndex >= 0,
    );
    controls.at(-1)?.focus();
    controls
      .at(-1)
      ?.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true }),
      );
    expect(document.activeElement).toBe(controls[0]);
    q('settings-tab-gameplay').focus();
    const back = new KeyboardEvent('keydown', {
      key: 'Tab',
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    controls[0]?.dispatchEvent(back);
    expect(document.activeElement).toBe(controls.at(-1));
  });

  it('gives every tab a heading-level label and a panel with a name', () => {
    const { screen, q } = setup();
    screen.open();
    for (const tab of SETTINGS_TABS as readonly SettingsTab[]) {
      screen.select(tab);
      expect(q(`settings-panel-${tab}`).getAttribute('aria-label')).not.toBe('');
    }
  });
});
