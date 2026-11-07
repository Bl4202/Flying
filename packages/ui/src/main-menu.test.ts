// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { ARENA, ASSIST_LEVEL } from '@flying/sim-bridge';
import { MainMenu, type MainMenuDeps } from './main-menu.ts';
import {
  applySettingsPatch,
  DEFAULT_SETTINGS,
  type Settings,
  type SettingsPatch,
} from './settings.ts';

function setup(extra: Partial<MainMenuDeps> = {}) {
  const root = document.createElement('div');
  document.body.append(root);
  let settings: Settings = applySettingsPatch(DEFAULT_SETTINGS, {});
  const changes: SettingsPatch[] = [];
  const calls = { fly: 0, settings: 0 };
  const menu = new MainMenu(root, {
    settings: () => settings,
    onChange: (patch) => {
      changes.push(patch);
      settings = applySettingsPatch(settings, patch);
    },
    onFly: () => {
      calls.fly += 1;
    },
    onSettings: () => {
      calls.settings += 1;
    },
    build: '0123456789abcdef',
    ...extra,
  });
  const q = (id: string): HTMLElement => {
    const el = root.querySelector<HTMLElement>(`[data-testid="${id}"]`);
    if (!el) throw new Error(`no ${id}`);
    return el;
  };
  const pressed = (id: string): boolean => q(id).getAttribute('aria-pressed') === 'true';
  return { root, menu, calls, changes, q, pressed, settings: () => settings };
}

describe('MainMenu', () => {
  it('opens with Fly focused, and removes everything on close', () => {
    const { root, menu, q } = setup();
    expect(menu.isOpen).toBe(false);
    menu.open();
    expect(menu.isOpen).toBe(true);
    expect(document.activeElement).toBe(q('menu-fly'));
    expect(root.querySelector('[role="dialog"]')?.getAttribute('aria-modal')).toBe('true');
    menu.open(); // already open: nothing doubles
    expect(root.querySelectorAll('[data-testid="main-menu"]')).toHaveLength(1);
    menu.close();
    expect(root.childNodes).toHaveLength(0);
    expect(menu.isOpen).toBe(false);
  });

  it('starts a sortie with Fly and opens the settings', () => {
    const { menu, q, calls } = setup();
    menu.open();
    q('menu-fly').click();
    q('menu-settings').click();
    expect(calls).toEqual({ fly: 1, settings: 1 });
  });

  it('shows the arena and the assist level, and changes them', () => {
    const { menu, q, pressed, changes, settings } = setup();
    menu.open();
    expect(pressed(`menu-arena-${String(ARENA.RUGGED)}`)).toBe(true);
    expect(pressed(`menu-assist-${String(ASSIST_LEVEL.STANDARD)}`)).toBe(true);
    q(`menu-arena-${String(ARENA.PLAIN)}`).click();
    q(`menu-assist-${String(ASSIST_LEVEL.FULL)}`).click();
    expect(changes).toEqual([{ arena: ARENA.PLAIN }, { assist: ASSIST_LEVEL.FULL }]);
    expect(pressed(`menu-arena-${String(ARENA.PLAIN)}`)).toBe(true);
    expect(pressed(`menu-arena-${String(ARENA.RUGGED)}`)).toBe(false);
    expect(settings()).toMatchObject({ arena: ARENA.PLAIN, assist: ASSIST_LEVEL.FULL });
  });

  it('explains a choice while it has focus', () => {
    const { menu, q } = setup();
    menu.open();
    q(`menu-assist-${String(ASSIST_LEVEL.MANUAL)}`).dispatchEvent(new Event('focus'));
    expect(q('menu-assist-preview').textContent).not.toBe('');
  });

  it('shows the short build, and re-reads the settings on refresh', () => {
    const { menu, q, pressed, settings } = setup();
    menu.open();
    expect(q('menu-version').textContent).toContain('01234567');
    expect(q('menu-version').textContent).not.toContain('89abcdef');
    expect(pressed(`menu-arena-${String(ARENA.PLAIN)}`)).toBe(false);
    // Something else changed the arena (an import in the settings screen).
    menu.close();
    menu.open();
    expect(settings().arena).toBe(ARENA.RUGGED);
  });

  it('has no install, update or sound row unless it was given one', () => {
    const { menu, root } = setup();
    menu.open();
    for (const id of ['menu-install', 'menu-update', 'menu-audio']) {
      expect(root.querySelector(`[data-testid="${id}"]`)).toBeNull();
    }
  });

  it('shows an optional row only while it applies, and runs it', () => {
    const state = { install: false, update: false, audio: true };
    const ran: string[] = [];
    const row = (key: keyof typeof state) => ({
      available: () => state[key],
      run: () => {
        ran.push(key);
        state[key] = false;
      },
    });
    const { menu, q } = setup({
      install: row('install'),
      update: row('update'),
      audio: row('audio'),
    });
    menu.open();
    expect(q('menu-install-row').hidden).toBe(true);
    expect(q('menu-update-row').hidden).toBe(true);
    expect(q('menu-audio-row').hidden).toBe(false);
    expect(q('menu-audio-row').textContent).toContain('Sound is off');
    state.update = true;
    state.install = true;
    menu.refresh();
    expect(q('menu-update-row').hidden).toBe(false);
    expect(q('menu-update').textContent).toBe('Update ready — reload');
    expect(q('menu-install-row').hidden).toBe(false);
    q('menu-update').click();
    q('menu-audio').click();
    expect(ran).toEqual(['update', 'audio']);
    // After running, the row has no reason to stay.
    expect(q('menu-update-row').hidden).toBe(true);
    expect(q('menu-audio-row').hidden).toBe(true);
    expect(q('menu-install-row').hidden).toBe(false);
  });

  it('keeps Tab inside the menu', () => {
    const { menu, root, q } = setup();
    menu.open();
    const buttons = [...root.querySelectorAll<HTMLElement>('button')];
    const last = buttons.at(-1);
    expect(last).toBeDefined();
    last?.focus();
    const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
    last?.dispatchEvent(tab);
    expect(tab.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(buttons[0]);
    const back = new KeyboardEvent('keydown', {
      key: 'Tab',
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    q('menu-fly').dispatchEvent(back);
    // Fly is the first button, so Shift+Tab wraps to the last.
    expect(document.activeElement).toBe(last);
  });
});
