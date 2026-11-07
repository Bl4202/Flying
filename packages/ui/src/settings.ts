/**
 * The player's settings (GDD 59, ADR-37, ADR-44), kept in the browser between visits under
 * `settings/2`.
 *
 * The values use the numbers the simulation's session config uses (`ASSIST_LEVEL`, `G_EFFECTS`,
 * `SPAWN_KIND`, `ARENA`), so the page hands them over as they are. Reading is forgiving: a stored
 * value that is missing or out of range falls back to its default, so a half-written or hand-edited
 * entry never stops the game from starting. Importing a file is strict: it names every field it
 * cannot use, by JSON pointer, and stores nothing.
 *
 * `settings/1` (v0.1b) is read once, when `settings/2` does not exist yet, and every field it has is
 * kept; it is left in place, so going back to the older build still finds its settings.
 */
import type { Kv } from '@flying/platform-web';
import { ARENA, ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge';

export const SETTINGS_KEY = 'settings/2';
/** Where v0.1b kept them. */
export const LEGACY_SETTINGS_KEY = 'settings/1';
export const SETTINGS_VERSION = 2;
/** The file name the Data tab saves under. */
export const SETTINGS_FILE_NAME = 'flying-settings.json';

export type MouseModeSetting = 'off' | 'aim' | 'stick';
export const MOUSE_MODES: readonly MouseModeSetting[] = ['off', 'aim', 'stick'];

export type CameraSetting = 'cockpit' | 'chase';
export const CAMERAS: readonly CameraSetting[] = ['cockpit', 'chase'];

export type ReducedMotionSetting = 'auto' | 'on' | 'off';
export const REDUCED_MOTIONS: readonly ReducedMotionSetting[] = ['auto', 'on', 'off'];

/** The HUD's colours (`render`'s `HUD_PALETTES`; a test in the app shell keeps them equal). */
export type HudPaletteSetting = 'green' | 'amber' | 'white' | 'cyan' | 'high-contrast';
export const HUD_PALETTE_SETTINGS: readonly HudPaletteSetting[] = [
  'green',
  'amber',
  'white',
  'cyan',
  'high-contrast',
];

export type RendererSetting = 'auto' | 'webgpu' | 'webgl2';
export const RENDERERS: readonly RendererSetting[] = ['auto', 'webgpu', 'webgl2'];

export type PresetSetting = 'auto' | 'low' | 'medium' | 'high';
export const PRESETS: readonly PresetSetting[] = ['auto', 'low', 'medium', 'high'];

export interface HudSettings {
  palette: HudPaletteSetting;
  /** 20 to 100 per cent. */
  brightness: number;
  /** 0 full, 1 reduced, 2 minimum. */
  declutter: number;
  /** The size of the HUD drawn over the picture (chase view), 50 to 200 per cent. */
  overlayScale: number;
}

export interface VideoSettings {
  /** Takes effect at the next start. */
  renderer: RendererSetting;
  /** `auto` follows the first-run benchmark. */
  preset: PresetSetting;
  /** 50 to 200 per cent of the window. */
  resolutionScale: number;
}

export interface AudioSettings {
  /** Each 0 to 100 per cent. */
  master: number;
  aircraft: number;
  warnings: number;
  ui: number;
  muteWhenHidden: boolean;
}

export interface Settings {
  version: typeof SETTINGS_VERSION;
  /** `ASSIST_LEVEL`: 1 Full, 2 Standard, 3 Reduced, 4 Manual. */
  assist: number;
  /** `G_EFFECTS`: 0 Off, 1 Reduced, 2 Full. */
  gEffects: number;
  /** How strong the greying and blackout look, 0 to 100 per cent (the sim is unaffected). */
  gfxStrength: number;
  /** `SPAWN_KIND`: where a respawn goes, 0 the runway, 1 the air. */
  spawn: number;
  /** `ARENA`: 0 rugged, 1 plain. */
  arena: number;
  mouseMode: MouseModeSetting;
  /** 0 to 3 times. */
  mouseSensitivity: number;
  /** How hard the mouse aim or stick returns to centre, 0 to 1. */
  mouseSpring: number;
  /** Opt-in automatic ground-collision avoidance (Full assist). Reserved: no effect yet. */
  autoGcas: boolean;
  camera: CameraSetting;
  /** Vertical field of view in the cockpit, 50 to 120 degrees. */
  fovDeg: number;
  /** How much the view shakes (buffet, rolling, afterburner), 0 to 100 per cent. */
  cameraShake: number;
  /** `auto` follows the system's `prefers-reduced-motion`. */
  reducedMotion: ReducedMotionSetting;
  hud: HudSettings;
  video: VideoSettings;
  audio: AudioSettings;
  /** The first-run flow has been through (or skipped). */
  setupDone: boolean;
}

/** A change to some settings: the groups may be partial too. */
export type SettingsPatch = Partial<Omit<Settings, 'version' | 'hud' | 'video' | 'audio'>> & {
  hud?: Partial<HudSettings>;
  video?: Partial<VideoSettings>;
  audio?: Partial<AudioSettings>;
};

export const DEFAULT_SETTINGS: Readonly<Settings> = {
  version: SETTINGS_VERSION,
  assist: ASSIST_LEVEL.STANDARD,
  gEffects: G_EFFECTS.FULL,
  gfxStrength: 100,
  spawn: SPAWN_KIND.RUNWAY,
  arena: ARENA.RUGGED,
  mouseMode: 'off',
  mouseSensitivity: 1,
  mouseSpring: 0,
  autoGcas: false,
  camera: 'cockpit',
  fovDeg: 75,
  cameraShake: 100,
  reducedMotion: 'auto',
  hud: { palette: 'green', brightness: 100, declutter: 0, overlayScale: 100 },
  video: { renderer: 'auto', preset: 'auto', resolutionScale: 100 },
  audio: { master: 80, aircraft: 100, warnings: 100, ui: 100, muteWhenHidden: true },
  setupDone: false,
};

/** Whether motion effects are reduced: the setting, or the system's preference when it says `auto`. */
export function reducedMotionOn(
  setting: ReducedMotionSetting,
  systemPrefersReduced: boolean,
): boolean {
  return setting === 'on' || (setting === 'auto' && systemPrefersReduced);
}

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

function whole(v: unknown, min: number, max: number, fallback: number): number {
  return typeof v === 'number' && Number.isInteger(v) && v >= min && v <= max ? v : fallback;
}

function ranged(v: unknown, min: number, max: number, fallback: number): number {
  return typeof v === 'number' && Number.isFinite(v) && v >= min && v <= max ? v : fallback;
}

function percent(v: unknown, min: number, max: number, fallback: number): number {
  return Math.round(ranged(v, min, max, fallback));
}

function oneOf<T extends string>(v: unknown, choices: readonly T[], fallback: T): T {
  return choices.find((c) => c === v) ?? fallback;
}

/**
 * Moves an entry written by an older version to the current shape. Version 1 (v0.1b) has a subset
 * of the fields, under the same names, so it is the same object; the first-run flow, which v0.1b did
 * not have, is still to be seen.
 */
export function migrateSettings(raw: unknown): unknown {
  return raw;
}

function overlayHud(base: Readonly<HudSettings>, raw: unknown): HudSettings {
  const r = isObject(raw) ? raw : {};
  return {
    palette: oneOf(r['palette'], HUD_PALETTE_SETTINGS, base.palette),
    brightness: percent(r['brightness'], 20, 100, base.brightness),
    declutter: whole(r['declutter'], 0, 2, base.declutter),
    overlayScale: percent(r['overlayScale'], 50, 200, base.overlayScale),
  };
}

function overlayVideo(base: Readonly<VideoSettings>, raw: unknown): VideoSettings {
  const r = isObject(raw) ? raw : {};
  return {
    renderer: oneOf(r['renderer'], RENDERERS, base.renderer),
    preset: oneOf(r['preset'], PRESETS, base.preset),
    resolutionScale: percent(r['resolutionScale'], 50, 200, base.resolutionScale),
  };
}

function overlayAudio(base: Readonly<AudioSettings>, raw: unknown): AudioSettings {
  const r = isObject(raw) ? raw : {};
  return {
    master: percent(r['master'], 0, 100, base.master),
    aircraft: percent(r['aircraft'], 0, 100, base.aircraft),
    warnings: percent(r['warnings'], 0, 100, base.warnings),
    ui: percent(r['ui'], 0, 100, base.ui),
    muteWhenHidden:
      typeof r['muteWhenHidden'] === 'boolean' ? r['muteWhenHidden'] : base.muteWhenHidden,
  };
}

/**
 * `base` with the fields of `raw` that are usable laid over it; a field that is missing or out of
 * range keeps its value from `base`. The keys come out in one fixed order, so the same settings
 * always serialise to the same text.
 */
function overlay(base: Readonly<Settings>, raw: unknown): Settings {
  if (!isObject(raw)) return structuredClone(base);
  return {
    version: SETTINGS_VERSION,
    assist: whole(raw['assist'], ASSIST_LEVEL.FULL, ASSIST_LEVEL.MANUAL, base.assist),
    gEffects: whole(raw['gEffects'], G_EFFECTS.OFF, G_EFFECTS.FULL, base.gEffects),
    gfxStrength: percent(raw['gfxStrength'], 0, 100, base.gfxStrength),
    spawn: whole(raw['spawn'], SPAWN_KIND.RUNWAY, SPAWN_KIND.AIR, base.spawn),
    arena: whole(raw['arena'], ARENA.RUGGED, ARENA.PLAIN, base.arena),
    mouseMode: oneOf(raw['mouseMode'], MOUSE_MODES, base.mouseMode),
    mouseSensitivity: ranged(raw['mouseSensitivity'], 0, 3, base.mouseSensitivity),
    mouseSpring: ranged(raw['mouseSpring'], 0, 1, base.mouseSpring),
    autoGcas: typeof raw['autoGcas'] === 'boolean' ? raw['autoGcas'] : base.autoGcas,
    camera: oneOf(raw['camera'], CAMERAS, base.camera),
    fovDeg: ranged(raw['fovDeg'], 50, 120, base.fovDeg),
    cameraShake: percent(raw['cameraShake'], 0, 100, base.cameraShake),
    reducedMotion: oneOf(raw['reducedMotion'], REDUCED_MOTIONS, base.reducedMotion),
    hud: overlayHud(base.hud, raw['hud']),
    video: overlayVideo(base.video, raw['video']),
    audio: overlayAudio(base.audio, raw['audio']),
    setupDone: typeof raw['setupDone'] === 'boolean' ? raw['setupDone'] : base.setupDone,
  };
}

/** `base` with `patch` laid over it (a group may be given in part); an unusable value keeps `base`'s. */
export function applySettingsPatch(base: Readonly<Settings>, patch: SettingsPatch): Settings {
  return overlay(base, patch);
}

/** Reads a stored value (of any version), replacing anything unusable with its default. */
export function parseSettings(raw: unknown): Settings {
  return overlay(DEFAULT_SETTINGS, migrateSettings(raw));
}

// ---------------------------------------------------------------------------------------------
// Export and import (the Data tab).

/** The settings as a file's text: stable key order, two-space indent, a final newline. */
export function exportSettings(settings: Readonly<Settings>): string {
  return `${JSON.stringify(parseSettings(settings), null, 2)}\n`;
}

export interface SettingsImportProblem {
  /** JSON pointer to the field, `''` for the whole file. */
  pointer: string;
  message: string;
}

export type SettingsImportResult =
  { ok: true; settings: Settings } | { ok: false; problems: SettingsImportProblem[] };

type Rule = (v: unknown) => string | undefined;

const isWhole =
  (min: number, max: number): Rule =>
  (v) =>
    typeof v === 'number' && Number.isInteger(v) && v >= min && v <= max
      ? undefined
      : `expected a whole number from ${String(min)} to ${String(max)}`;
const isRange =
  (min: number, max: number): Rule =>
  (v) =>
    typeof v === 'number' && Number.isFinite(v) && v >= min && v <= max
      ? undefined
      : `expected a number from ${String(min)} to ${String(max)}`;
const isChoice =
  (choices: readonly string[]): Rule =>
  (v) =>
    typeof v === 'string' && choices.includes(v)
      ? undefined
      : `expected one of ${choices.join(', ')}`;
const isBool: Rule = (v) => (typeof v === 'boolean' ? undefined : 'expected true or false');

const RULES: Readonly<Record<string, Rule | Readonly<Record<string, Rule>>>> = {
  assist: isWhole(ASSIST_LEVEL.FULL, ASSIST_LEVEL.MANUAL),
  gEffects: isWhole(G_EFFECTS.OFF, G_EFFECTS.FULL),
  gfxStrength: isRange(0, 100),
  spawn: isWhole(SPAWN_KIND.RUNWAY, SPAWN_KIND.AIR),
  arena: isWhole(ARENA.RUGGED, ARENA.PLAIN),
  mouseMode: isChoice(MOUSE_MODES),
  mouseSensitivity: isRange(0, 3),
  mouseSpring: isRange(0, 1),
  autoGcas: isBool,
  camera: isChoice(CAMERAS),
  fovDeg: isRange(50, 120),
  cameraShake: isRange(0, 100),
  reducedMotion: isChoice(REDUCED_MOTIONS),
  hud: {
    palette: isChoice(HUD_PALETTE_SETTINGS),
    brightness: isRange(20, 100),
    declutter: isWhole(0, 2),
    overlayScale: isRange(50, 200),
  },
  video: {
    renderer: isChoice(RENDERERS),
    preset: isChoice(PRESETS),
    resolutionScale: isRange(50, 200),
  },
  audio: {
    master: isRange(0, 100),
    aircraft: isRange(0, 100),
    warnings: isRange(0, 100),
    ui: isRange(0, 100),
    muteWhenHidden: isBool,
  },
  setupDone: isBool,
};

/**
 * Reads the text of a settings file. A file of version 1 or 2 is accepted; every field it has must
 * be usable, or the result lists each that is not (a field it lacks takes its default, so files
 * from before a setting existed still import). Nothing is applied here.
 */
export function importSettings(text: string): SettingsImportResult {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { ok: false, problems: [{ pointer: '', message: 'this is not JSON' }] };
  }
  if (!isObject(raw)) {
    return { ok: false, problems: [{ pointer: '', message: 'expected an object' }] };
  }
  const problems: SettingsImportProblem[] = [];
  const version = raw['version'];
  if (version !== 1 && version !== SETTINGS_VERSION) {
    problems.push({ pointer: '/version', message: 'expected 1 or 2' });
  }
  for (const [key, rule] of Object.entries(RULES)) {
    if (!(key in raw)) continue;
    const value = raw[key];
    if (typeof rule === 'function') {
      const message = rule(value);
      if (message !== undefined) problems.push({ pointer: `/${key}`, message });
      continue;
    }
    if (!isObject(value)) {
      problems.push({ pointer: `/${key}`, message: 'expected an object' });
      continue;
    }
    for (const [inner, innerRule] of Object.entries(rule)) {
      if (!(inner in value)) continue;
      const message = innerRule(value[inner]);
      if (message !== undefined) problems.push({ pointer: `/${key}/${inner}`, message });
    }
  }
  return problems.length > 0 ? { ok: false, problems } : { ok: true, settings: parseSettings(raw) };
}

/** The settings in memory, backed by the key-value store. */
export class SettingsStore {
  readonly #kv: Kv;
  readonly #listeners = new Set<(s: Settings) => void>();
  #value: Settings = structuredClone(DEFAULT_SETTINGS);

  constructor(kv: Kv) {
    this.#kv = kv;
  }

  /** Where the settings are kept: `memory` means they last only until the page closes. */
  get backend(): Kv['backend'] {
    return this.#kv.backend;
  }

  get value(): Readonly<Settings> {
    return this.#value;
  }

  /**
   * Reads the stored settings: `settings/2`, else v0.1b's `settings/1` moved to the current shape,
   * else the defaults (also if the store fails).
   */
  async load(): Promise<Settings> {
    try {
      const current = await this.#kv.get<unknown>(SETTINGS_KEY);
      this.#value = parseSettings(
        current !== undefined ? current : await this.#kv.get<unknown>(LEGACY_SETTINGS_KEY),
      );
    } catch {
      this.#value = structuredClone(DEFAULT_SETTINGS);
    }
    return this.#value;
  }

  /**
   * Changes some settings, tells the listeners and stores the result. A value that is out of
   * range is ignored and the old one kept; a group may be given in part.
   */
  async update(patch: SettingsPatch): Promise<Settings> {
    return this.#set(overlay(this.#value, patch));
  }

  /** Puts the settings back to their defaults (the first-run flow stays done). */
  async reset(): Promise<Settings> {
    return this.#set({ ...(structuredClone(DEFAULT_SETTINGS) as Settings), setupDone: true });
  }

  /** Replaces every setting (an imported file). */
  async replace(settings: Settings): Promise<Settings> {
    return this.#set(parseSettings(settings));
  }

  onChange(fn: (s: Settings) => void): () => void {
    this.#listeners.add(fn);
    return () => {
      this.#listeners.delete(fn);
    };
  }

  async #set(next: Settings): Promise<Settings> {
    this.#value = next;
    for (const fn of this.#listeners) fn(this.#value);
    try {
      await this.#kv.set(SETTINGS_KEY, this.#value);
    } catch {
      // Not saved: the settings still hold for this visit.
    }
    return this.#value;
  }
}
