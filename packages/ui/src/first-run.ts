/**
 * The first-run flow (GDD 53.11, ADR-44): shown once, on a first visit, before the main menu. It asks
 * what the player flies with, calibrates a joystick if there is one, sets how much help the flight
 * assist gives, offers the comfort and accessibility basics and the photosensitivity notice (GDD 57.4),
 * and says it is done. Every step can be skipped, and so can the whole flow (`Skip setup`); a choice
 * is applied as it is made, so skipping keeps what was already chosen.
 *
 * One step is on screen at a time and its DOM is replaced when the step changes. The step heading
 * takes focus then, so a keyboard or a screen reader lands at the top of what is new. Every control
 * is native: Tab moves between them (and wraps), Enter and Space press them.
 */
import { G_EFFECTS, ASSIST_LEVEL } from '@flying/sim-bridge';
import { h } from './dom.ts';
import { Form, trapTab, type Choice } from './form.ts';
import { type FirstRunStep, FIRST_RUN_STEPS } from './gameflow.ts';
import {
  HUD_PALETTE_SETTINGS,
  REDUCED_MOTIONS,
  type HudPaletteSetting,
  type ReducedMotionSetting,
  type Settings,
  type SettingsPatch,
} from './settings.ts';
import { t, type StringKey } from './strings.ts';

/** What the player flies with. */
export type DeviceKind = 'km' | 'gamepad' | 'joystick';

/** A gamepad or joystick the browser reports. */
export interface DetectedDevice {
  kind: Exclude<DeviceKind, 'km'>;
  name: string;
}

export interface FirstRunDeps {
  settings(): Readonly<Settings>;
  onChange(patch: SettingsPatch): void;
  /** Show another step (the flow's state follows). */
  onStep(step: FirstRunStep): void;
  /** The flow is over, finished or skipped. */
  onFinish(): void;
  /** Pads and joysticks the browser reports now. */
  devices(): readonly DetectedDevice[];
  /** The device the controls profile in use is for. */
  device(): DeviceKind;
  /** The player chose a device: use its controls profile. */
  onDevice(kind: DeviceKind): void;
  /** Open the calibration for the joystick. */
  onCalibrate(): void;
}

/** The steps for a device: calibration is only for a joystick. */
export function stepsFor(device: DeviceKind): readonly FirstRunStep[] {
  return FIRST_RUN_STEPS.filter((s) => s !== 'calibration' || device === 'joystick');
}

/** The step after `step`, or undefined at the end. */
export function stepAfter(step: FirstRunStep, device: DeviceKind): FirstRunStep | undefined {
  const steps = stepsFor(device);
  return steps[steps.indexOf(step) + 1];
}

/** The step before `step`, or undefined at the start. */
export function stepBefore(step: FirstRunStep, device: DeviceKind): FirstRunStep | undefined {
  const steps = stepsFor(device);
  const i = steps.indexOf(step);
  return i > 0 ? steps[i - 1] : undefined;
}

const DEVICES: Choice<DeviceKind>[] = [
  { value: 'km', label: 'firstrun.device.km' },
  { value: 'gamepad', label: 'firstrun.device.gamepad' },
  { value: 'joystick', label: 'firstrun.device.joystick' },
];
const ASSISTS: Choice<number>[] = [
  { value: ASSIST_LEVEL.FULL, label: 'assist.full', preview: 'assist.full.preview' },
  { value: ASSIST_LEVEL.STANDARD, label: 'assist.standard', preview: 'assist.standard.preview' },
  { value: ASSIST_LEVEL.REDUCED, label: 'assist.reduced', preview: 'assist.reduced.preview' },
  { value: ASSIST_LEVEL.MANUAL, label: 'assist.manual', preview: 'assist.manual.preview' },
];
const G_CHOICES: Choice<number>[] = [
  { value: G_EFFECTS.OFF, label: 'pause.g.off', preview: 'pause.g.off.preview' },
  { value: G_EFFECTS.REDUCED, label: 'pause.g.reduced', preview: 'pause.g.reduced.preview' },
  { value: G_EFFECTS.FULL, label: 'pause.g.full', preview: 'pause.g.full.preview' },
];
const MOTIONS: Choice<ReducedMotionSetting>[] = REDUCED_MOTIONS.map((value) => ({
  value,
  label: `settings.motion.${value}` as const,
}));
const PALETTES: Choice<HudPaletteSetting>[] = HUD_PALETTE_SETTINGS.map((value) => ({
  value,
  label: `settings.palette.${value}` as const,
}));

const TITLES: Record<FirstRunStep, StringKey> = {
  device: 'firstrun.device.title',
  calibration: 'firstrun.calibrate.title',
  assist: 'firstrun.assist.title',
  comfort: 'firstrun.comfort.title',
  notice: 'firstrun.notice.title',
  done: 'firstrun.done.title',
};

export class FirstRun {
  readonly #root: HTMLElement;
  readonly #d: FirstRunDeps;
  #panel: HTMLElement | undefined;
  #form: Form | undefined;
  #step: FirstRunStep = 'device';

  constructor(root: HTMLElement, deps: FirstRunDeps) {
    this.#root = root;
    this.#d = deps;
  }

  get isOpen(): boolean {
    return this.#panel !== undefined;
  }

  get step(): FirstRunStep {
    return this.#step;
  }

  get element(): HTMLElement | undefined {
    return this.#panel;
  }

  /** Shows `step`, opening the flow if it is not open. */
  show(step: FirstRunStep): void {
    this.#step = step;
    const before = this.#panel;
    const panel = this.#build(step);
    if (before) before.replaceWith(panel);
    else this.#root.append(panel);
    this.#panel = panel;
    panel.addEventListener('keydown', (e) => {
      trapTab(panel, this.#root.ownerDocument, e);
    });
    panel.querySelector<HTMLElement>('[data-testid="firstrun-heading"]')?.focus();
  }

  close(): void {
    this.#panel?.remove();
    this.#panel = undefined;
    this.#form = undefined;
  }

  destroy(): void {
    this.close();
  }

  /** Re-reads the settings and the devices, for a change made elsewhere (a pad plugged in). */
  refresh(): void {
    if (this.#panel && this.#step === 'device') this.show('device');
    else this.#form?.refresh();
  }

  #build(step: FirstRunStep): HTMLElement {
    const doc = this.#root.ownerDocument;
    const form = new Form(doc, 'firstrun');
    this.#form = form;
    const settings = (): Readonly<Settings> => this.#d.settings();
    const device = this.#d.device();
    const steps = stepsFor(device);

    const body: HTMLElement[] = [];
    switch (step) {
      case 'device': {
        body.push(
          h(doc, 'p', { class: 'note', text: t('firstrun.device.hint') }),
          form.group(
            'device',
            'firstrun.device.title',
            DEVICES,
            () => this.#d.device(),
            (kind) => {
              this.#d.onDevice(kind);
              // The calibration step comes and goes with the choice, so the step count changes.
              this.show('device');
            },
          ),
        );
        const found = this.#d.devices();
        body.push(
          found.length === 0
            ? h(doc, 'p', {
                class: 'note',
                testid: 'firstrun-detected',
                text: t('firstrun.device.none'),
              })
            : h(
                doc,
                'ul',
                { class: 'detected', testid: 'firstrun-detected' },
                ...found.map((d) =>
                  h(doc, 'li', { text: t('firstrun.device.detected', { name: d.name }) }),
                ),
              ),
        );
        break;
      }
      case 'calibration':
        body.push(
          h(doc, 'p', { class: 'note', text: t('firstrun.calibrate.hint') }),
          form.button('calibrate', 'firstrun.calibrate.start', () => {
            this.#d.onCalibrate();
          }),
        );
        break;
      case 'assist':
        body.push(
          h(doc, 'p', { class: 'note', text: t('firstrun.assist.hint') }),
          form.group(
            'assist',
            'menu.assist',
            ASSISTS,
            () => settings().assist,
            (assist) => {
              this.#d.onChange({ assist });
            },
          ),
        );
        break;
      case 'comfort':
        body.push(
          h(doc, 'p', { class: 'note', text: t('firstrun.comfort.hint') }),
          form.group(
            'g',
            'pause.g',
            G_CHOICES,
            () => settings().gEffects,
            (gEffects) => {
              this.#d.onChange({ gEffects });
            },
          ),
          form.slider(
            'shake',
            'settings.shake',
            { min: 0, max: 100, step: 5 },
            () => settings().cameraShake,
            (cameraShake) => {
              this.#d.onChange({ cameraShake });
            },
            (v) => `${String(Math.round(v))}%`,
          ),
          form.group(
            'motion',
            'settings.motion',
            MOTIONS,
            () => settings().reducedMotion,
            (reducedMotion) => {
              this.#d.onChange({ reducedMotion });
            },
            'settings.motion.note',
          ),
          form.group(
            'palette',
            'settings.hud.palette',
            PALETTES,
            () => settings().hud.palette,
            (palette) => {
              this.#d.onChange({ hud: { palette } });
            },
          ),
          form.slider(
            'volume',
            'settings.audio.master',
            { min: 0, max: 100, step: 5 },
            () => settings().audio.master,
            (master) => {
              this.#d.onChange({ audio: { master } });
            },
            (v) => `${String(Math.round(v))}%`,
          ),
        );
        break;
      case 'notice':
        body.push(h(doc, 'p', { testid: 'firstrun-notice', text: t('firstrun.notice.body') }));
        break;
      case 'done':
        body.push(h(doc, 'p', { text: t('firstrun.done.body') }));
        break;
    }

    const next = stepAfter(step, device);
    const previous = stepBefore(step, device);
    const nav = h(doc, 'div', { class: 'actions' });
    if (previous !== undefined) {
      nav.append(
        form.button('back', 'firstrun.back', () => {
          this.#d.onStep(previous);
        }),
      );
    }
    const forward = form.button(
      'next',
      next === undefined
        ? 'firstrun.finish'
        : step === 'notice'
          ? 'firstrun.notice.ok'
          : 'firstrun.next',
      () => {
        if (next === undefined) this.#d.onFinish();
        else this.#d.onStep(next);
      },
    );
    forward.classList.add('primary');
    nav.append(forward);
    if (next !== undefined) {
      nav.append(
        form.button('skip', 'firstrun.skip', () => {
          this.#d.onFinish();
        }),
      );
    }

    return h(
      doc,
      'section',
      {
        class: 'panel-screen first-run',
        testid: 'first-run',
        data: { step },
        attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'firstrun-heading' },
      },
      h(doc, 'p', { class: 'eyebrow', text: t('firstrun.title') }),
      h(doc, 'p', {
        class: 'step',
        testid: 'firstrun-step',
        text: t('firstrun.step', { n: steps.indexOf(step) + 1, total: steps.length }),
      }),
      h(doc, 'h2', {
        text: t(TITLES[step]),
        testid: 'firstrun-heading',
        attrs: { id: 'firstrun-heading', tabindex: '-1' },
      }),
      ...body,
      nav,
    );
  }
}
