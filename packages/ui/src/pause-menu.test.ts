// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge';
import { countNodes } from './dom.ts';
import { PauseMenu, type PauseMenuDeps, type SettingsPatch } from './pause-menu.ts';
import { DEFAULT_SETTINGS, type Settings } from './settings.ts';

function setup(extra: Partial<PauseMenuDeps> = {}) {
  const root = document.createElement('div');
  document.body.append(root);
  let settings: Settings = { ...DEFAULT_SETTINGS };
  const changes: SettingsPatch[] = [];
  const calls = { resume: 0, controls: 0, respawn: 0 };
  let flying = ASSIST_LEVEL.STANDARD as number;
  const menu = new PauseMenu(root, {
    settings: () => settings,
    assistInForce: () => flying,
    onChange: (patch) => {
      changes.push(patch);
      settings = { ...settings, ...patch };
    },
    onResume: () => {
      calls.resume += 1;
    },
    onControls: () => {
      calls.controls += 1;
    },
    onRespawn: () => {
      calls.respawn += 1;
    },
    ...extra,
  });
  const q = (id: string): HTMLElement => {
    const el = root.querySelector<HTMLElement>(`[data-testid="${id}"]`);
    if (!el) throw new Error(`no ${id}`);
    return el;
  };
  const pressed = (id: string): boolean => q(id).getAttribute('aria-pressed') === 'true';
  return {
    root,
    menu,
    changes,
    calls,
    q,
    pressed,
    set: (s: Partial<Settings>) => {
      settings = { ...settings, ...s };
    },
    fly: (n: number) => {
      flying = n;
    },
  };
}

describe('PauseMenu', () => {
  it('builds nothing until it opens, and removes everything when it closes', () => {
    const { root, menu } = setup();
    expect(root.children).toHaveLength(0);
    menu.open();
    expect(menu.isOpen).toBe(true);
    expect(root.querySelector('[role="dialog"]')).not.toBeNull();
    menu.open(); // again: no second copy
    expect(root.querySelectorAll('[data-testid="pause-menu"]')).toHaveLength(1);
    menu.close();
    expect(menu.isOpen).toBe(false);
    expect(root.children).toHaveLength(0);
    menu.close(); // harmless
  });

  it('is a modal dialog named by its title, with the resume button focused', () => {
    const { menu, q } = setup();
    menu.open();
    const panel = q('pause-menu');
    expect(panel.getAttribute('aria-modal')).toBe('true');
    expect(panel.getAttribute('aria-labelledby')).toBe('pause-title');
    expect(document.getElementById('pause-title')?.textContent).toBe('Paused');
    expect(document.activeElement).toBe(q('pause-resume'));
  });

  it('shows the assist levels, the selected one pressed, and the one in force marked', () => {
    const { menu, pressed, q, fly } = setup();
    fly(ASSIST_LEVEL.REDUCED);
    menu.open();
    expect(pressed('pause-assist-2')).toBe(true);
    expect(pressed('pause-assist-1')).toBe(false);
    expect(q('pause-assist-2').textContent).toBe('Standard');
    expect(q('pause-assist-3').dataset['flying']).toBe('true');
    expect(q('pause-assist-2').dataset['flying']).toBe('false');
  });

  it('changes the assist at once when a level is chosen', () => {
    const { menu, q, changes, pressed } = setup();
    menu.open();
    q('pause-assist-4').click();
    expect(changes).toEqual([{ assist: ASSIST_LEVEL.MANUAL }]);
    expect(pressed('pause-assist-4')).toBe(true);
    expect(pressed('pause-assist-2')).toBe(false);
  });

  it('previews what a level does while it has focus or the pointer, and shows the chosen one otherwise', () => {
    const { menu, q } = setup();
    menu.open();
    const preview = () => q('pause-assist-preview').textContent;
    expect(preview()).toMatch(/^Standard: /);
    q('pause-assist-1').dispatchEvent(new Event('focus'));
    expect(preview()).toMatch(/^Full: /);
    q('pause-assist-1').dispatchEvent(new Event('blur'));
    expect(preview()).toMatch(/^Standard: /);
    q('pause-assist-4').dispatchEvent(new Event('mouseenter'));
    expect(preview()).toMatch(/^Manual: /);
    q('pause-assist-4').dispatchEvent(new Event('mouseleave'));
    expect(preview()).toMatch(/^Standard: /);
  });

  it('chooses where a respawn goes', () => {
    const { menu, q, changes, pressed } = setup();
    menu.open();
    expect(pressed('pause-spawn-0')).toBe(true);
    q('pause-spawn-1').click();
    expect(changes).toEqual([{ spawn: SPAWN_KIND.AIR }]);
    expect(pressed('pause-spawn-1')).toBe(true);
  });

  it('chooses the mouse mode and tunes it', () => {
    const { menu, q, changes, pressed } = setup();
    menu.open();
    expect(pressed('pause-mouse-off')).toBe(true);
    q('pause-mouse-aim').click();
    q('pause-mouse-stick').click();
    expect(changes).toEqual([{ mouseMode: 'aim' }, { mouseMode: 'stick' }]);
    const sens = q('pause-sensitivity') as HTMLInputElement;
    expect(sens.value).toBe('1');
    expect(q('pause-sensitivity-value').textContent).toBe('1.00×');
    sens.value = '2.5';
    sens.dispatchEvent(new Event('input'));
    expect(changes.at(-1)).toEqual({ mouseSensitivity: 2.5 });
    expect(q('pause-sensitivity-value').textContent).toBe('2.50×');
    const spring = q('pause-spring') as HTMLInputElement;
    spring.value = '0.5';
    spring.dispatchEvent(new Event('input'));
    expect(changes.at(-1)).toEqual({ mouseSpring: 0.5 });
    expect(q('pause-spring-value').textContent).toBe('50%');
  });

  it('chooses the g-effects and how strong the screen shows them', () => {
    const { menu, q, changes, pressed } = setup();
    menu.open();
    expect(pressed(`pause-g-${String(G_EFFECTS.FULL)}`)).toBe(true);
    q(`pause-g-${String(G_EFFECTS.REDUCED)}`).click();
    expect(changes).toEqual([{ gEffects: G_EFFECTS.REDUCED }]);
    const strength = q('pause-strength') as HTMLInputElement;
    strength.value = '35';
    strength.dispatchEvent(new Event('input'));
    expect(changes.at(-1)).toEqual({ gfxStrength: 35 });
    expect(q('pause-strength-value').textContent).toBe('35%');
  });

  it('says that the g-effects setting is used at the next start (the core takes it only when idle)', () => {
    const { menu, q } = setup();
    menu.open();
    expect(q('pause-g-note').textContent).toBe(
      'The g-effects setting is used when the game next starts.',
    );
  });

  it('leaves out the ground-collision option unless it exists, and offers it when it does', () => {
    const none = setup();
    none.menu.open();
    expect(none.root.querySelector('[data-testid="pause-autogcas"]')).toBeNull();
    const some = setup({ autoGcas: true });
    some.menu.open();
    const box = some.q('pause-autogcas') as HTMLInputElement;
    expect(box.checked).toBe(false);
    box.checked = true;
    box.dispatchEvent(new Event('change'));
    expect(some.changes).toEqual([{ autoGcas: true }]);
  });

  it('calls resume, controls and respawn', () => {
    const { menu, q, calls } = setup();
    menu.open();
    q('pause-resume').click();
    q('pause-controls').click();
    q('pause-respawn').click();
    expect(calls).toEqual({ resume: 1, controls: 1, respawn: 1 });
  });

  it('follows a setting changed elsewhere when refreshed', () => {
    const { menu, pressed, set, q } = setup();
    menu.open();
    set({ assist: ASSIST_LEVEL.FULL, gfxStrength: 10 });
    menu.refresh();
    expect(pressed('pause-assist-1')).toBe(true);
    expect(pressed('pause-assist-2')).toBe(false);
    expect((q('pause-strength') as HTMLInputElement).value).toBe('10');
  });

  it('keeps Tab inside the menu, wrapping at both ends', () => {
    const { menu, q } = setup();
    menu.open();
    const first = q('pause-resume');
    const last = q('pause-respawn');
    last.focus();
    const forward = new KeyboardEvent('keydown', { key: 'Tab', cancelable: true, bubbles: true });
    last.dispatchEvent(forward);
    expect(forward.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(first);
    const back = new KeyboardEvent('keydown', {
      key: 'Tab',
      shiftKey: true,
      cancelable: true,
      bubbles: true,
    });
    first.dispatchEvent(back);
    expect(back.defaultPrevented).toBe(true);
    expect(document.activeElement).toBe(last);
    // In the middle Tab is left to the browser.
    q('pause-assist-1').focus();
    const middle = new KeyboardEvent('keydown', { key: 'Tab', cancelable: true, bubbles: true });
    q('pause-assist-1').dispatchEvent(middle);
    expect(middle.defaultPrevented).toBe(false);
  });

  it('is small while open', () => {
    const { menu, root } = setup({ autoGcas: true });
    menu.open();
    expect(countNodes(root)).toBeLessThan(160);
  });
});
