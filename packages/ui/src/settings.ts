/**
 * The player's settings (GDD 59, ADR-37), kept in the browser between visits under `settings/1`.
 *
 * The values use the numbers the simulation's session config uses (`ASSIST_LEVEL`, `G_EFFECTS`,
 * `SPAWN_KIND`), so the page hands them over as they are. Reading is forgiving: a stored value
 * that is missing or out of range falls back to its default, so a half-written or hand-edited
 * entry never stops the game from starting.
 */
import type { Kv } from '@flying/platform-web';
import { ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge';

export const SETTINGS_KEY = 'settings/1';
export const SETTINGS_VERSION = 1;

export type MouseModeSetting = 'off' | 'aim' | 'stick';
export const MOUSE_MODES: readonly MouseModeSetting[] = ['off', 'aim', 'stick'];

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
  mouseMode: MouseModeSetting;
  /** 0 to 3 times. */
  mouseSensitivity: number;
  /** How hard the mouse aim or stick returns to centre, 0 to 1. */
  mouseSpring: number;
  /** Opt-in automatic ground-collision avoidance (Full assist). Reserved: no effect yet. */
  autoGcas: boolean;
}

export const DEFAULT_SETTINGS: Readonly<Settings> = {
  version: SETTINGS_VERSION,
  assist: ASSIST_LEVEL.STANDARD,
  gEffects: G_EFFECTS.FULL,
  gfxStrength: 100,
  spawn: SPAWN_KIND.RUNWAY,
  mouseMode: 'off',
  mouseSensitivity: 1,
  mouseSpring: 0,
  autoGcas: false,
};

const isObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

function whole(v: unknown, min: number, max: number, fallback: number): number {
  return typeof v === 'number' && Number.isInteger(v) && v >= min && v <= max ? v : fallback;
}

function ranged(v: unknown, min: number, max: number, fallback: number): number {
  return typeof v === 'number' && Number.isFinite(v) && v >= min && v <= max ? v : fallback;
}

/**
 * Moves an entry written by an older version to the current shape. There is only version 1; this
 * is where a version 2 would convert.
 */
export function migrateSettings(raw: unknown): unknown {
  return raw;
}

/**
 * `base` with the fields of `raw` that are usable laid over it; a field that is missing or out of
 * range keeps its value from `base`.
 */
function overlay(base: Readonly<Settings>, raw: unknown): Settings {
  if (!isObject(raw)) return { ...base };
  return {
    version: SETTINGS_VERSION,
    assist: whole(raw['assist'], ASSIST_LEVEL.FULL, ASSIST_LEVEL.MANUAL, base.assist),
    gEffects: whole(raw['gEffects'], G_EFFECTS.OFF, G_EFFECTS.FULL, base.gEffects),
    gfxStrength: Math.round(ranged(raw['gfxStrength'], 0, 100, base.gfxStrength)),
    spawn: whole(raw['spawn'], SPAWN_KIND.RUNWAY, SPAWN_KIND.AIR, base.spawn),
    mouseMode: MOUSE_MODES.find((m) => m === raw['mouseMode']) ?? base.mouseMode,
    mouseSensitivity: ranged(raw['mouseSensitivity'], 0, 3, base.mouseSensitivity),
    mouseSpring: ranged(raw['mouseSpring'], 0, 1, base.mouseSpring),
    autoGcas: typeof raw['autoGcas'] === 'boolean' ? raw['autoGcas'] : base.autoGcas,
  };
}

/** Reads a stored value, replacing anything unusable with its default. */
export function parseSettings(raw: unknown): Settings {
  return overlay(DEFAULT_SETTINGS, migrateSettings(raw));
}

/** The settings in memory, backed by the key-value store. */
export class SettingsStore {
  readonly #kv: Kv;
  readonly #listeners = new Set<(s: Settings) => void>();
  #value: Settings = { ...DEFAULT_SETTINGS };

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

  /** Reads the stored settings (the defaults if there are none, or the store fails). */
  async load(): Promise<Settings> {
    try {
      this.#value = parseSettings(await this.#kv.get<unknown>(SETTINGS_KEY));
    } catch {
      this.#value = { ...DEFAULT_SETTINGS };
    }
    return this.#value;
  }

  /**
   * Changes some settings, tells the listeners and stores the result. A value that is out of
   * range is ignored and the old one kept.
   */
  async update(patch: Partial<Omit<Settings, 'version'>>): Promise<Settings> {
    this.#value = overlay(this.#value, patch);
    for (const fn of this.#listeners) fn(this.#value);
    try {
      await this.#kv.set(SETTINGS_KEY, this.#value);
    } catch {
      // Not saved: the settings still hold for this visit.
    }
    return this.#value;
  }

  onChange(fn: (s: Settings) => void): () => void {
    this.#listeners.add(fn);
    return () => {
      this.#listeners.delete(fn);
    };
  }
}
