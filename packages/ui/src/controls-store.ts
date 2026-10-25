/**
 * The player's control profiles and which one is in use, kept in the browser (ADR-37).
 *
 * Under the key-value store:
 *
 * - `controls/active`: the id of the profile in use (absent: choose by the connected devices);
 * - `controls/index`: the ids of the player's own profiles, since the store cannot list its keys;
 * - `controls/profile/<id>`: one profile, as the canonical `controls/1` JSON text.
 *
 * The built-in profiles are not stored: they are code, and cannot be changed or deleted. A stored
 * profile that no longer parses is reported in `problems` and left alone in the store, so a bad
 * edit by hand costs that profile and nothing else.
 */
import type { Kv } from '@flying/platform-web';
import {
  BUILTIN_IDS,
  builtinProfiles,
  type ControlsProfile,
  isBuiltinId,
  parseControlsProfileText,
  type ProfileError,
  serializeProfile,
  uniqueProfileId,
} from '@flying/input';

export const ACTIVE_KEY = 'controls/active';
export const INDEX_KEY = 'controls/index';
export const profileKey = (id: string): string => `controls/profile/${id}`;

export interface StoreProblem {
  id: string;
  errors: ProfileError[];
}

export type ImportResult =
  { ok: true; profile: ControlsProfile } | { ok: false; errors: ProfileError[] };

export class ControlsStore {
  readonly #kv: Kv;
  readonly #listeners = new Set<() => void>();
  #user: ControlsProfile[] = [];
  #active: string | undefined;
  #problems: StoreProblem[] = [];

  constructor(kv: Kv) {
    this.#kv = kv;
  }

  get backend(): Kv['backend'] {
    return this.#kv.backend;
  }

  /** The built-in profiles, then the player's own. */
  all(): ControlsProfile[] {
    return [...builtinProfiles(), ...this.#user];
  }

  /** Only the player's own. */
  user(): readonly ControlsProfile[] {
    return this.#user;
  }

  /** The id of the profile the player chose, if it still exists. */
  get activeId(): string | undefined {
    return this.#active;
  }

  /** Profiles that were stored but could not be read. */
  get problems(): readonly StoreProblem[] {
    return this.#problems;
  }

  find(id: string): ControlsProfile | undefined {
    return this.all().find((p) => p.id === id);
  }

  isBuiltin(id: string): boolean {
    return isBuiltinId(id);
  }

  /** Reads everything from the store. Never throws: what cannot be read is skipped. */
  async load(): Promise<void> {
    this.#user = [];
    this.#problems = [];
    let index: unknown;
    try {
      index = await this.#kv.get<unknown>(INDEX_KEY);
    } catch {
      index = undefined;
    }
    const ids = Array.isArray(index)
      ? index.filter((v): v is string => typeof v === 'string' && !isBuiltinId(v))
      : [];
    for (const id of ids) {
      let text: unknown;
      try {
        text = await this.#kv.get<unknown>(profileKey(id));
      } catch {
        continue;
      }
      if (typeof text !== 'string') continue;
      const parsed = parseControlsProfileText(text);
      if (!parsed.ok) {
        this.#problems.push({ id, errors: parsed.errors });
      } else if (parsed.profile.id !== id || isBuiltinId(parsed.profile.id)) {
        this.#problems.push({
          id,
          errors: [
            {
              pointer: '/id',
              message: `the profile is stored as "${id}" but says it is "${parsed.profile.id}"`,
            },
          ],
        });
      } else {
        this.#user.push(parsed.profile);
      }
    }
    try {
      const active = await this.#kv.get<unknown>(ACTIVE_KEY);
      this.#active =
        typeof active === 'string' && this.find(active) !== undefined ? active : undefined;
    } catch {
      this.#active = undefined;
    }
  }

  /** Chooses the profile in use (`undefined`: let the connected devices decide). */
  async setActive(id: string | undefined): Promise<void> {
    this.#active = id !== undefined && this.find(id) !== undefined ? id : undefined;
    await this.#write(async () => {
      if (this.#active === undefined) await this.#kv.delete(ACTIVE_KEY);
      else await this.#kv.set(ACTIVE_KEY, this.#active);
    });
    this.#notify();
  }

  /** Stores a profile of the player's own, replacing one with the same id. False for a built-in id. */
  async save(profile: ControlsProfile): Promise<boolean> {
    if (isBuiltinId(profile.id)) return false;
    const at = this.#user.findIndex((p) => p.id === profile.id);
    if (at >= 0) this.#user[at] = profile;
    else this.#user.push(profile);
    await this.#write(async () => {
      await this.#kv.set(profileKey(profile.id), serializeProfile(profile));
      await this.#kv.set(
        INDEX_KEY,
        this.#user.map((p) => p.id),
      );
    });
    this.#notify();
    return true;
  }

  /** Deletes one of the player's profiles; if it was in use, the choice reverts to automatic. */
  async remove(id: string): Promise<void> {
    if (isBuiltinId(id)) return;
    this.#user = this.#user.filter((p) => p.id !== id);
    const wasActive = this.#active === id;
    if (wasActive) this.#active = undefined;
    await this.#write(async () => {
      await this.#kv.delete(profileKey(id));
      await this.#kv.set(
        INDEX_KEY,
        this.#user.map((p) => p.id),
      );
      if (wasActive) await this.#kv.delete(ACTIVE_KEY);
    });
    this.#notify();
  }

  /**
   * Reads a profile file. If it parses it is stored under an id of its own (the file's, unless that
   * is taken or reserved) and returned; if not, every problem is returned with its JSON pointer.
   */
  async importText(text: string): Promise<ImportResult> {
    const parsed = parseControlsProfileText(text);
    if (!parsed.ok) return parsed;
    const taken = this.all().map((p) => p.id);
    const id = taken.includes(parsed.profile.id)
      ? uniqueProfileId(taken, parsed.profile.id)
      : parsed.profile.id;
    const profile = { ...parsed.profile, id };
    await this.save(profile);
    return { ok: true, profile };
  }

  exportText(profile: ControlsProfile): string {
    return serializeProfile(profile);
  }

  onChange(fn: () => void): () => void {
    this.#listeners.add(fn);
    return () => {
      this.#listeners.delete(fn);
    };
  }

  /** Runs a store write; a store that fails leaves the change standing for this visit. */
  async #write(op: () => Promise<void>): Promise<void> {
    try {
      await op();
    } catch {
      // Not stored: the profile still holds for this visit.
    }
  }

  #notify(): void {
    for (const fn of this.#listeners) fn();
  }
}

/** The ids no profile of the player's may take. */
export const RESERVED_PROFILE_IDS: readonly string[] = BUILTIN_IDS;
