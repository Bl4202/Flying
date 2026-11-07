// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { ASSIST_LEVEL, G_EFFECTS } from '@flying/sim-bridge';
import {
  FirstRun,
  stepAfter,
  stepBefore,
  stepsFor,
  type DetectedDevice,
  type DeviceKind,
  type FirstRunDeps,
} from './first-run.ts';
import type { FirstRunStep } from './gameflow.ts';
import {
  applySettingsPatch,
  DEFAULT_SETTINGS,
  type Settings,
  type SettingsPatch,
} from './settings.ts';

function setup(initial: DeviceKind = 'km', found: DetectedDevice[] = []) {
  const root = document.createElement('div');
  document.body.append(root);
  let settings: Settings = applySettingsPatch(DEFAULT_SETTINGS, {});
  let device: DeviceKind = initial;
  const changes: SettingsPatch[] = [];
  const calls = {
    finish: 0,
    calibrate: 0,
    devices: [] as DeviceKind[],
    steps: [] as FirstRunStep[],
  };
  // Stands in for the app: the flow state follows the steps it is told about.
  const run: FirstRun = new FirstRun(root, {
    settings: () => settings,
    onChange: (patch) => {
      changes.push(patch);
      settings = applySettingsPatch(settings, patch);
    },
    onStep: (step) => {
      calls.steps.push(step);
      run.show(step);
    },
    onFinish: () => {
      calls.finish += 1;
    },
    devices: () => found,
    device: () => device,
    onDevice: (kind) => {
      calls.devices.push(kind);
      device = kind;
    },
    onCalibrate: () => {
      calls.calibrate += 1;
    },
  } satisfies FirstRunDeps);
  const q = (id: string): HTMLElement => {
    const el = root.querySelector<HTMLElement>(`[data-testid="${id}"]`);
    if (!el) throw new Error(`no ${id}`);
    return el;
  };
  const has = (id: string): boolean => root.querySelector(`[data-testid="${id}"]`) !== null;
  return { root, run, calls, changes, q, has, settings: () => settings };
}

describe('the steps', () => {
  it('skip the calibration unless the device is a joystick', () => {
    expect(stepsFor('km')).toEqual(['device', 'assist', 'comfort', 'notice', 'done']);
    expect(stepsFor('gamepad')).toEqual(['device', 'assist', 'comfort', 'notice', 'done']);
    expect(stepsFor('joystick')).toEqual([
      'device',
      'calibration',
      'assist',
      'comfort',
      'notice',
      'done',
    ]);
  });
  it('lead on and back', () => {
    expect(stepAfter('device', 'km')).toBe('assist');
    expect(stepAfter('device', 'joystick')).toBe('calibration');
    expect(stepAfter('done', 'km')).toBeUndefined();
    expect(stepBefore('assist', 'km')).toBe('device');
    expect(stepBefore('assist', 'joystick')).toBe('calibration');
    expect(stepBefore('device', 'km')).toBeUndefined();
  });
});

describe('FirstRun', () => {
  it('opens at a step, puts focus on its heading, and counts the steps', () => {
    const { run, q, root } = setup();
    run.show('device');
    expect(run.isOpen).toBe(true);
    expect(document.activeElement).toBe(q('firstrun-heading'));
    expect(q('firstrun-step').textContent).toBe('Step 1 of 5');
    expect(root.querySelector('[role="dialog"]')?.getAttribute('aria-labelledby')).toBe(
      'firstrun-heading',
    );
    run.close();
    expect(root.childNodes).toHaveLength(0);
  });

  it('asks what the player flies with, and shows what it found', () => {
    const found: DetectedDevice[] = [{ kind: 'joystick', name: 'Stick 3000' }];
    const { run, q, calls } = setup('km', found);
    run.show('device');
    expect(q('firstrun-detected').textContent).toContain('Stick 3000');
    expect(q('firstrun-device-km').getAttribute('aria-pressed')).toBe('true');
    q('firstrun-device-gamepad').click();
    expect(calls.devices).toEqual(['gamepad']);
    expect(q('firstrun-device-gamepad').getAttribute('aria-pressed')).toBe('true');
  });

  it('says so when no pad was found', () => {
    const { run, q } = setup();
    run.show('device');
    expect(q('firstrun-detected').textContent).toContain('No gamepad or joystick found');
  });

  it('adds the calibration step for a joystick, and its count', () => {
    const { run, q, calls, has } = setup();
    run.show('device');
    q('firstrun-device-joystick').click();
    // Choosing a joystick re-counts the steps.
    expect(q('firstrun-step').textContent).toBe('Step 1 of 6');
    q('firstrun-next').click();
    expect(calls.steps).toEqual(['calibration']);
    expect(has('firstrun-calibrate')).toBe(true);
    q('firstrun-calibrate').click();
    expect(calls.calibrate).toBe(1);
    expect(q('firstrun-step').textContent).toBe('Step 2 of 6');
    q('firstrun-next').click();
    expect(calls.steps).toEqual(['calibration', 'assist']);
  });

  it('goes from the device straight to the assist step for the keyboard', () => {
    const { run, q, calls } = setup();
    run.show('device');
    q('firstrun-next').click();
    expect(calls.steps).toEqual(['assist']);
    expect(run.step).toBe('assist');
  });

  it('applies the assist choice at once', () => {
    const { run, q, changes } = setup();
    run.show('assist');
    q(`firstrun-assist-${String(ASSIST_LEVEL.FULL)}`).click();
    expect(changes).toEqual([{ assist: ASSIST_LEVEL.FULL }]);
    expect(q(`firstrun-assist-${String(ASSIST_LEVEL.FULL)}`).getAttribute('aria-pressed')).toBe(
      'true',
    );
  });

  it('offers the comfort basics and applies each as it is chosen', () => {
    const { run, q, changes, settings } = setup();
    run.show('comfort');
    q(`firstrun-g-${String(G_EFFECTS.REDUCED)}`).click();
    q('firstrun-motion-on').click();
    q('firstrun-palette-high-contrast').click();
    const shake = q('firstrun-shake') as HTMLInputElement;
    shake.value = '20';
    shake.dispatchEvent(new Event('input'));
    const volume = q('firstrun-volume') as HTMLInputElement;
    volume.value = '45';
    volume.dispatchEvent(new Event('input'));
    expect(changes).toEqual([
      { gEffects: G_EFFECTS.REDUCED },
      { reducedMotion: 'on' },
      { hud: { palette: 'high-contrast' } },
      { cameraShake: 20 },
      { audio: { master: 45 } },
    ]);
    expect(settings()).toMatchObject({ gEffects: 1, reducedMotion: 'on', cameraShake: 20 });
    expect(q('firstrun-shake-value').textContent).toBe('20%');
  });

  it('shows the photosensitivity notice in full, with an acknowledgement that moves on', () => {
    const { run, q, calls } = setup();
    run.show('notice');
    expect(q('firstrun-notice').textContent).toContain('flashes');
    expect(q('firstrun-next').textContent).toBe('I understand');
    q('firstrun-next').click();
    expect(calls.steps).toEqual(['done']);
  });

  it('finishes on the last step, which has no skip or next', () => {
    const { run, q, has, calls } = setup();
    run.show('done');
    expect(has('firstrun-skip')).toBe(false);
    expect(q('firstrun-next').textContent).toBe('Go to the menu');
    q('firstrun-next').click();
    expect(calls.finish).toBe(1);
  });

  it('can be skipped from any earlier step, keeping what was chosen', () => {
    const { run, q, calls, settings } = setup();
    run.show('assist');
    q(`firstrun-assist-${String(ASSIST_LEVEL.REDUCED)}`).click();
    q('firstrun-skip').click();
    expect(calls.finish).toBe(1);
    expect(settings().assist).toBe(ASSIST_LEVEL.REDUCED);
  });

  it('goes back, except from the first step', () => {
    const { run, q, has, calls } = setup();
    run.show('device');
    expect(has('firstrun-back')).toBe(false);
    run.show('comfort');
    q('firstrun-back').click();
    expect(calls.steps).toEqual(['assist']);
  });

  it('can be walked end to end by the keyboard alone: every control is a native one, in a trapped Tab order', () => {
    const { run, root, q, calls } = setup();
    run.show('device');
    const seen: string[] = [];
    for (let guard = 0; guard < 10 && run.step !== 'done'; guard++) {
      seen.push(run.step);
      for (const el of root.querySelectorAll<HTMLElement>('[data-testid]')) {
        const tag = el.tagName;
        if (['BUTTON', 'INPUT'].includes(tag)) {
          expect(el.getAttribute('role')).toBeNull();
          expect(el.tabIndex).toBeGreaterThanOrEqual(0);
        }
      }
      // Tab wraps from the last control to the first.
      const controls = [...root.querySelectorAll<HTMLElement>('button, input')];
      controls.at(-1)?.focus();
      const tab = new KeyboardEvent('keydown', { key: 'Tab', bubbles: true, cancelable: true });
      controls.at(-1)?.dispatchEvent(tab);
      expect(document.activeElement).toBe(controls[0]);
      // Enter on a native button is a click.
      q('firstrun-next').click();
    }
    expect(seen).toEqual(['device', 'assist', 'comfort', 'notice']);
    expect(run.step).toBe('done');
    q('firstrun-next').click();
    expect(calls.finish).toBe(1);
  });

  it('re-reads the device list when a pad is plugged in on the device step', () => {
    const found: DetectedDevice[] = [];
    const { run, q } = setup('km', found);
    run.show('device');
    expect(q('firstrun-detected').textContent).toContain('No gamepad');
    found.push({ kind: 'gamepad', name: 'Pad One' });
    run.refresh();
    expect(q('firstrun-detected').textContent).toContain('Pad One');
  });
});
