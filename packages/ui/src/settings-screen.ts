/**
 * The settings screen (GDD 53, 59, ADR-44), opened from the main menu: six tabs, Gameplay, Controls,
 * Video, Audio, Accessibility and Data. A change is applied and stored as it is made. Every setting
 * has a default, and a tab with settings has a button that puts its own back to them.
 *
 * Only the open tab's DOM exists: opening another tab replaces it, and closing the screen removes
 * all of it. The tabs are a WAI-ARIA tablist: Left and Right (and Home and End) move between them,
 * and Tab leaves the list for the panel.
 */
import { ARENA, ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge';
import { h } from './dom.ts';
import { Form, trapTab, type Choice } from './form.ts';
import {
  CAMERAS,
  DEFAULT_SETTINGS,
  exportSettings,
  HUD_PALETTE_SETTINGS,
  importSettings,
  MOUSE_MODES,
  PRESETS,
  REDUCED_MOTIONS,
  RENDERERS,
  SETTINGS_FILE_NAME,
  type CameraSetting,
  type HudPaletteSetting,
  type MouseModeSetting,
  type PresetSetting,
  type ReducedMotionSetting,
  type RendererSetting,
  type Settings,
  type SettingsPatch,
} from './settings.ts';
import { t, type StringKey } from './strings.ts';

export const SETTINGS_TABS = [
  'gameplay',
  'controls',
  'video',
  'audio',
  'accessibility',
  'data',
] as const;
export type SettingsTab = (typeof SETTINGS_TABS)[number];

export interface SettingsScreenDeps {
  settings(): Readonly<Settings>;
  onChange(patch: SettingsPatch): void;
  /** An imported set of settings replaces every setting. */
  onReplace(settings: Settings): void;
  /** Every setting back to its default. */
  onReset(): void;
  /** Back to the main menu. */
  onClose(): void;
  /** Open the controls editor. */
  onControls(): void;
  /** Run the first-run flow again. */
  onRunSetup(): void;
  /** Gives the player a file to keep (the export). */
  save(name: string, text: string): void;
  /** The name of the controls profile in use, for the Controls tab. */
  profileName?(): string;
  /** Where the settings are kept and which build this is, for the Data tab. */
  info: { storage: string; build: string };
  /** Reads a chosen file (default: `file.text()`). */
  readFile?(file: File): Promise<string>;
}

const SPAWNS: Choice<number>[] = [
  { value: SPAWN_KIND.RUNWAY, label: 'pause.spawn.runway' },
  { value: SPAWN_KIND.AIR, label: 'pause.spawn.air' },
];
const ARENAS: Choice<number>[] = [
  { value: ARENA.RUGGED, label: 'menu.arena.rugged', preview: 'menu.arena.rugged.preview' },
  { value: ARENA.PLAIN, label: 'menu.arena.plain', preview: 'menu.arena.plain.preview' },
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
const MICE: Choice<MouseModeSetting>[] = MOUSE_MODES.map((value) => ({
  value,
  label: `pause.mouse.${value}` as const,
  preview: `pause.mouse.${value}.preview` as const,
}));
const CAMERA_CHOICES: Choice<CameraSetting>[] = CAMERAS.map((value) => ({
  value,
  label: `settings.camera.${value}` as const,
}));
const RENDERER_CHOICES: Choice<RendererSetting>[] = RENDERERS.map((value) => ({
  value,
  label: `settings.renderer.${value}` as const,
}));
const PRESET_CHOICES: Choice<PresetSetting>[] = PRESETS.map((value) => ({
  value,
  label: `settings.preset.${value}` as const,
}));
const PALETTES: Choice<HudPaletteSetting>[] = HUD_PALETTE_SETTINGS.map((value) => ({
  value,
  label: `settings.palette.${value}` as const,
}));
const DECLUTTER: Choice<number>[] = [
  { value: 0, label: 'settings.hud.declutter.0' },
  { value: 1, label: 'settings.hud.declutter.1' },
  { value: 2, label: 'settings.hud.declutter.2' },
];
const MOTIONS: Choice<ReducedMotionSetting>[] = REDUCED_MOTIONS.map((value) => ({
  value,
  label: `settings.motion.${value}` as const,
}));

const TAB_LABELS: Record<SettingsTab, StringKey> = {
  gameplay: 'settings.tab.gameplay',
  controls: 'settings.tab.controls',
  video: 'settings.tab.video',
  audio: 'settings.tab.audio',
  accessibility: 'settings.tab.accessibility',
  data: 'settings.tab.data',
};

const percent = (v: number): string => `${String(Math.round(v))}%`;

/** What a tab's reset button puts back (the defaults of the settings the tab shows). */
export function tabDefaults(tab: SettingsTab): SettingsPatch | undefined {
  const d = DEFAULT_SETTINGS;
  switch (tab) {
    case 'gameplay':
      return {
        assist: d.assist,
        spawn: d.spawn,
        arena: d.arena,
        gEffects: d.gEffects,
        gfxStrength: d.gfxStrength,
        mouseMode: d.mouseMode,
        mouseSensitivity: d.mouseSensitivity,
        mouseSpring: d.mouseSpring,
        camera: d.camera,
        fovDeg: d.fovDeg,
      };
    case 'video':
      return { video: { ...d.video }, hud: { ...d.hud } };
    case 'audio':
      return { audio: { ...d.audio } };
    case 'accessibility':
      return {
        cameraShake: d.cameraShake,
        reducedMotion: d.reducedMotion,
        gfxStrength: d.gfxStrength,
        hud: { palette: d.hud.palette },
      };
    case 'controls':
    case 'data':
      return undefined;
  }
}

export class SettingsScreen {
  readonly #root: HTMLElement;
  readonly #d: SettingsScreenDeps;
  #panel: HTMLElement | undefined;
  #body: HTMLElement | undefined;
  #form: Form | undefined;
  #tab: SettingsTab = 'gameplay';

  constructor(root: HTMLElement, deps: SettingsScreenDeps) {
    this.#root = root;
    this.#d = deps;
  }

  get isOpen(): boolean {
    return this.#panel !== undefined;
  }

  get tab(): SettingsTab {
    return this.#tab;
  }

  get element(): HTMLElement | undefined {
    return this.#panel;
  }

  open(tab: SettingsTab = 'gameplay'): void {
    if (this.#panel) {
      this.select(tab);
      return;
    }
    const doc = this.#root.ownerDocument;
    this.#tab = tab;
    const body = h(doc, 'div', { class: 'tab-body' });
    this.#body = body;
    const panel = h(
      doc,
      'section',
      {
        class: 'panel-screen settings-screen',
        testid: 'settings-screen',
        attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'settings-title' },
      },
      h(doc, 'h2', { text: t('settings.title'), attrs: { id: 'settings-title' } }),
      this.#tabList(doc),
      body,
      h(
        doc,
        'div',
        { class: 'actions' },
        h(doc, 'button', {
          text: t('settings.close'),
          testid: 'settings-close',
          attrs: { type: 'button' },
        }),
      ),
    );
    panel.querySelector('[data-testid="settings-close"]')?.addEventListener('click', () => {
      this.#d.onClose();
    });
    panel.addEventListener('keydown', (e) => {
      trapTab(panel, doc, e);
    });
    this.#panel = panel;
    this.#root.append(panel);
    this.#fill();
    panel.querySelector<HTMLElement>(`[data-testid="settings-tab-${tab}"]`)?.focus();
  }

  close(): void {
    this.#panel?.remove();
    this.#panel = undefined;
    this.#body = undefined;
    this.#form = undefined;
  }

  destroy(): void {
    this.close();
  }

  /** Shows another tab. */
  select(tab: SettingsTab): void {
    if (!this.#panel) return;
    this.#tab = tab;
    for (const el of this.#panel.querySelectorAll<HTMLElement>('[role="tab"]')) {
      const on = el.dataset['tab'] === tab;
      el.setAttribute('aria-selected', String(on));
      el.tabIndex = on ? 0 : -1;
    }
    this.#fill();
  }

  /** Re-reads the settings, for a change made elsewhere. */
  refresh(): void {
    this.#form?.refresh();
  }

  #tabList(doc: Document): HTMLElement {
    const list = h(doc, 'div', {
      class: 'tabs',
      testid: 'settings-tabs',
      attrs: { role: 'tablist', 'aria-label': t('settings.title') },
    });
    for (const tab of SETTINGS_TABS) {
      const b = h(doc, 'button', {
        text: t(TAB_LABELS[tab]),
        testid: `settings-tab-${tab}`,
        data: { tab },
        attrs: {
          type: 'button',
          role: 'tab',
          'aria-selected': String(tab === this.#tab),
          'aria-controls': 'settings-tabpanel',
          tabindex: tab === this.#tab ? '0' : '-1',
        },
      });
      b.addEventListener('click', () => {
        this.select(tab);
      });
      list.append(b);
    }
    list.addEventListener('keydown', (e) => {
      const move = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
      const i = SETTINGS_TABS.indexOf(this.#tab);
      let to: SettingsTab | undefined;
      if (move !== undefined) {
        to = SETTINGS_TABS[(i + move + SETTINGS_TABS.length) % SETTINGS_TABS.length];
      } else if (e.key === 'Home') {
        to = SETTINGS_TABS[0];
      } else if (e.key === 'End') {
        to = SETTINGS_TABS.at(-1);
      }
      if (to === undefined) return;
      e.preventDefault();
      this.select(to);
      this.#panel?.querySelector<HTMLElement>(`[data-testid="settings-tab-${to}"]`)?.focus();
    });
    return list;
  }

  /** Replaces the tab body with the selected tab's. */
  #fill(): void {
    const body = this.#body;
    if (!body) return;
    const doc = this.#root.ownerDocument;
    const form = new Form(doc, 'settings');
    this.#form = form;
    const settings = (): Readonly<Settings> => this.#d.settings();
    const change = (patch: SettingsPatch): void => {
      this.#d.onChange(patch);
    };
    const parts: HTMLElement[] = [];
    switch (this.#tab) {
      case 'gameplay':
        parts.push(
          form.group(
            'assist',
            'menu.assist',
            ASSISTS,
            () => settings().assist,
            (assist) => {
              change({ assist });
            },
          ),
          form.group(
            'arena',
            'menu.arena',
            ARENAS,
            () => settings().arena,
            (arena) => {
              change({ arena });
            },
          ),
          form.group(
            'spawn',
            'pause.spawn',
            SPAWNS,
            () => settings().spawn,
            (spawn) => {
              change({ spawn });
            },
          ),
          form.group(
            'camera',
            'settings.camera',
            CAMERA_CHOICES,
            () => settings().camera,
            (camera) => {
              change({ camera });
            },
          ),
          form.slider(
            'fov',
            'settings.fov',
            { min: 50, max: 120, step: 1 },
            () => settings().fovDeg,
            (fovDeg) => {
              change({ fovDeg });
            },
            (v) => `${String(Math.round(v))}°`,
          ),
          form.group(
            'g',
            'pause.g',
            G_CHOICES,
            () => settings().gEffects,
            (gEffects) => {
              change({ gEffects });
            },
          ),
          form.slider(
            'strength',
            'pause.strength',
            { min: 0, max: 100, step: 5 },
            () => settings().gfxStrength,
            (gfxStrength) => {
              change({ gfxStrength });
            },
            percent,
          ),
          form.group(
            'mouse',
            'pause.mouse',
            MICE,
            () => settings().mouseMode,
            (mouseMode) => {
              change({ mouseMode });
            },
          ),
          form.slider(
            'sensitivity',
            'pause.sensitivity',
            { min: 0, max: 3, step: 0.05 },
            () => settings().mouseSensitivity,
            (mouseSensitivity) => {
              change({ mouseSensitivity });
            },
            (v) => `${v.toFixed(2)}×`,
          ),
          form.slider(
            'spring',
            'pause.spring',
            { min: 0, max: 1, step: 0.05 },
            () => settings().mouseSpring,
            (mouseSpring) => {
              change({ mouseSpring });
            },
            (v) => percent(v * 100),
          ),
        );
        break;
      case 'controls': {
        const name = this.#d.profileName?.();
        parts.push(
          h(doc, 'p', { class: 'note', text: t('settings.controls.devices') }),
          ...(name === undefined ? [] : [h(doc, 'p', { testid: 'settings-profile', text: name })]),
          form.button('controls', 'settings.controls.open', () => {
            this.#d.onControls();
          }),
        );
        break;
      }
      case 'video':
        parts.push(
          form.group(
            'renderer',
            'settings.renderer',
            RENDERER_CHOICES,
            () => settings().video.renderer,
            (renderer) => {
              change({ video: { renderer } });
            },
            'settings.renderer.restart',
          ),
          form.group(
            'preset',
            'settings.preset',
            PRESET_CHOICES,
            () => settings().video.preset,
            (preset) => {
              change({ video: { preset } });
            },
          ),
          form.slider(
            'scale',
            'settings.scale',
            { min: 50, max: 200, step: 5 },
            () => settings().video.resolutionScale,
            (resolutionScale) => {
              change({ video: { resolutionScale } });
            },
            percent,
          ),
          form.group(
            'palette',
            'settings.hud.palette',
            PALETTES,
            () => settings().hud.palette,
            (palette) => {
              change({ hud: { palette } });
            },
          ),
          form.slider(
            'brightness',
            'settings.hud.brightness',
            { min: 20, max: 100, step: 5 },
            () => settings().hud.brightness,
            (brightness) => {
              change({ hud: { brightness } });
            },
            percent,
          ),
          form.group(
            'declutter',
            'settings.hud.declutter',
            DECLUTTER,
            () => settings().hud.declutter,
            (declutter) => {
              change({ hud: { declutter } });
            },
          ),
          form.slider(
            'overlay-scale',
            'settings.hud.scale',
            { min: 50, max: 200, step: 5 },
            () => settings().hud.overlayScale,
            (overlayScale) => {
              change({ hud: { overlayScale } });
            },
            percent,
          ),
        );
        break;
      case 'audio':
        for (const [id, label] of [
          ['master', 'settings.audio.master'],
          ['aircraft', 'settings.audio.aircraft'],
          ['warnings', 'settings.audio.warnings'],
          ['ui', 'settings.audio.ui'],
        ] as const) {
          parts.push(
            form.slider(
              id,
              label,
              { min: 0, max: 100, step: 5 },
              () => settings().audio[id],
              (value) => {
                change({ audio: { [id]: value } });
              },
              percent,
            ),
          );
        }
        parts.push(
          form.check(
            'mute-hidden',
            'settings.audio.mute-hidden',
            () => settings().audio.muteWhenHidden,
            (muteWhenHidden) => {
              change({ audio: { muteWhenHidden } });
            },
          ),
        );
        break;
      case 'accessibility':
        parts.push(
          form.group(
            'motion',
            'settings.motion',
            MOTIONS,
            () => settings().reducedMotion,
            (reducedMotion) => {
              change({ reducedMotion });
            },
            'settings.motion.note',
          ),
          form.slider(
            'shake',
            'settings.shake',
            { min: 0, max: 100, step: 5 },
            () => settings().cameraShake,
            (cameraShake) => {
              change({ cameraShake });
            },
            percent,
          ),
          form.slider(
            'strength',
            'pause.strength',
            { min: 0, max: 100, step: 5 },
            () => settings().gfxStrength,
            (gfxStrength) => {
              change({ gfxStrength });
            },
            percent,
          ),
          form.group(
            'palette',
            'settings.hud.palette',
            PALETTES,
            () => settings().hud.palette,
            (palette) => {
              change({ hud: { palette } });
            },
          ),
        );
        break;
      case 'data':
        parts.push(...this.#dataTab(doc, form));
        break;
    }

    const defaults = tabDefaults(this.#tab);
    if (defaults !== undefined) {
      parts.push(
        h(
          doc,
          'div',
          { class: 'actions' },
          form.button('reset-tab', 'settings.reset-tab', () => {
            change(defaults);
            form.refresh();
          }),
        ),
      );
    }
    body.replaceChildren(
      h(
        doc,
        'div',
        {
          class: 'tab-panel',
          testid: `settings-panel-${this.#tab}`,
          attrs: {
            role: 'tabpanel',
            id: 'settings-tabpanel',
            'aria-label': t(TAB_LABELS[this.#tab]),
          },
        },
        ...parts,
      ),
    );
  }

  #dataTab(doc: Document, form: Form): HTMLElement[] {
    const status = h(doc, 'p', {
      class: 'note',
      testid: 'settings-status',
      attrs: { role: 'status', 'aria-live': 'polite' },
    });
    const problems = h(doc, 'ul', { class: 'problems', testid: 'settings-problems' });

    const exportButton = form.button('export', 'settings.data.export', () => {
      this.#d.save(SETTINGS_FILE_NAME, exportSettings(this.#d.settings()));
      status.textContent = t('settings.data.exported', { name: SETTINGS_FILE_NAME });
    });

    const file = h(doc, 'input', {
      testid: 'settings-import',
      attrs: {
        type: 'file',
        accept: 'application/json,.json',
        'aria-label': t('settings.data.import'),
      },
    });
    file.addEventListener('change', () => {
      const chosen = file.files?.[0];
      if (chosen === undefined) return;
      void (this.#d.readFile?.(chosen) ?? chosen.text()).then((text) => {
        const result = importSettings(text);
        problems.replaceChildren();
        if (result.ok) {
          this.#d.onReplace(result.settings);
          form.refresh();
          status.textContent = t('settings.data.import.ok');
        } else {
          status.textContent = '';
          for (const p of result.problems) {
            problems.append(
              h(doc, 'li', {
                text: t('settings.data.import.error', {
                  problem: p.pointer === '' ? p.message : `${p.pointer}: ${p.message}`,
                }),
              }),
            );
          }
        }
      });
    });

    const reset = form.button('reset-all', 'settings.data.reset', () => {
      if (reset.dataset['confirming'] !== 'true') {
        reset.dataset['confirming'] = 'true';
        reset.textContent = t('settings.data.reset.confirm');
        return;
      }
      delete reset.dataset['confirming'];
      reset.textContent = t('settings.data.reset');
      this.#d.onReset();
      status.textContent = t('settings.data.reset.done');
    });
    reset.addEventListener('blur', () => {
      delete reset.dataset['confirming'];
      reset.textContent = t('settings.data.reset');
    });

    return [
      h(doc, 'div', { class: 'actions' }, exportButton),
      h(doc, 'label', { class: 'file' }, h(doc, 'span', { text: t('settings.data.import') }), file),
      problems,
      h(
        doc,
        'div',
        { class: 'actions' },
        reset,
        form.button('setup', 'settings.data.setup', () => {
          this.#d.onRunSetup();
        }),
      ),
      status,
      h(doc, 'p', {
        class: 'note',
        testid: 'settings-storage',
        text: t('settings.data.storage', { backend: this.#d.info.storage }),
      }),
      h(doc, 'p', {
        class: 'note',
        testid: 'settings-build',
        text: t('settings.data.build', { build: this.#d.info.build.slice(0, 8) }),
      }),
    ];
  }
}
