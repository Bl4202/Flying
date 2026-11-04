/**
 * The presentation bundle (ADR-43): HUD layouts, cockpits, exterior models, the HUD font and audio
 * profiles, baked by `content-bake` into `core.present.json` next to the sim pack and never part
 * of it (so it cannot move the state hash).
 *
 * This file is the shared part of reading it: a pointer-tracking reader that turns a wrong value
 * into an error naming where it is (`/layers/3/channel`), the bundle's own envelope, and the check
 * that the bundle was baked with the sim pack the app loaded. `render` and `audio` (which may not
 * import each other) each parse their own slices with the reader.
 */
import { CPACK_HEADER } from './layout.gen.ts';

/** A value in the bundle is not what the parser needs. `pointer` says where, RFC 6901 style. */
export class PresentError extends Error {
  readonly pointer: string;

  constructor(pointer: string, message: string) {
    super(`SYS-040: presentation data ${pointer === '' ? '(whole file)' : pointer}: ${message}`);
    this.name = 'PresentError';
    this.pointer = pointer;
  }
}

function describe(value: unknown): string {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'an array';
  return typeof value === 'object' ? 'an object' : `a ${typeof value}`;
}

/** A JSON value together with where it sits, with typed accessors that fail with that place. */
export class Slice {
  readonly value: unknown;
  readonly pointer: string;

  constructor(value: unknown, pointer = '') {
    this.value = value;
    this.pointer = pointer;
  }

  /** Throws a `PresentError` at this value. */
  fail(message: string): never {
    throw new PresentError(this.pointer, message);
  }

  #record(): Record<string, unknown> {
    if (typeof this.value !== 'object' || this.value === null || Array.isArray(this.value)) {
      return this.fail(`expected an object, found ${describe(this.value)}`);
    }
    return this.value as Record<string, unknown>;
  }

  /** The keys of an object value. */
  keys(): string[] {
    return Object.keys(this.#record());
  }

  /** True if this is an object with `key`. */
  has(key: string): boolean {
    return (
      typeof this.value === 'object' &&
      this.value !== null &&
      !Array.isArray(this.value) &&
      key in (this.value as Record<string, unknown>)
    );
  }

  /** The member `key` of an object value; a missing key is an error. */
  at(key: string): Slice {
    const record = this.#record();
    if (!(key in record)) {
      return this.fail(`required key \`${key}\` is missing`);
    }
    return new Slice(
      record[key],
      `${this.pointer}/${key.replace(/~/g, '~0').replace(/\//g, '~1')}`,
    );
  }

  /** The member `key`, or undefined if the object has none. */
  opt(key: string): Slice | undefined {
    return this.has(key) ? this.at(key) : undefined;
  }

  /** The elements of an array value, each with its place. */
  items(): Slice[] {
    if (!Array.isArray(this.value)) {
      return this.fail(`expected an array, found ${describe(this.value)}`);
    }
    return (this.value as unknown[]).map((v, i) => new Slice(v, `${this.pointer}/${String(i)}`));
  }

  /** A number, optionally within `[min, max]`. */
  num(min = -Infinity, max = Infinity): number {
    const v = this.value;
    if (typeof v !== 'number' || !Number.isFinite(v)) {
      return this.fail(`expected a number, found ${describe(v)}`);
    }
    if (v < min || v > max) {
      return this.fail(`${String(v)} is outside the range ${String(min)} to ${String(max)}`);
    }
    return v;
  }

  /** A whole number, optionally within `[min, max]`. */
  int(min = -Infinity, max = Infinity): number {
    const v = this.num(min, max);
    if (!Number.isInteger(v)) {
      return this.fail(`${String(v)} is not a whole number`);
    }
    return v;
  }

  /** A string. */
  str(): string {
    if (typeof this.value !== 'string') {
      return this.fail(`expected a string, found ${describe(this.value)}`);
    }
    return this.value;
  }

  /** A string that is one of `allowed`. */
  oneOf<T extends string>(allowed: readonly T[]): T {
    const v = this.str();
    const found = allowed.find((a) => a === v);
    if (found === undefined) {
      return this.fail(`\`${v}\` is not one of ${allowed.join(', ')}`);
    }
    return found;
  }

  /** A boolean. */
  bool(): boolean {
    if (typeof this.value !== 'boolean') {
      return this.fail(`expected true or false, found ${describe(this.value)}`);
    }
    return this.value;
  }

  /** An array of exactly `n` numbers. */
  tuple(n: number): number[] {
    const items = this.items();
    if (items.length !== n) {
      return this.fail(`expected ${String(n)} numbers, found ${String(items.length)}`);
    }
    return items.map((s) => s.num());
  }

  /** An array of numbers. */
  numbers(): number[] {
    return this.items().map((s) => s.num());
  }

  // Shorthands for the member of an object.
  n(key: string, min?: number, max?: number): number {
    return this.at(key).num(min, max);
  }
  s(key: string): string {
    return this.at(key).str();
  }
}

/** One aircraft's presentation, as unparsed slices of the bundle. */
export interface AircraftPresentation {
  readonly hud: Slice | undefined;
  readonly cockpit: Slice | undefined;
  readonly model: Slice | undefined;
  readonly audio: Slice | undefined;
  /** The font the HUD names. */
  readonly font: Slice | undefined;
}

/** The bundle's envelope and its tables. */
export interface PresentBundle {
  /** SHA-256 of the sim pack the bundle was baked with, 64 hex digits. */
  readonly simPackSha256: string;
  /** Aircraft ids that have presentation data. */
  aircraftIds(): string[];
  /** The slices for an aircraft, or undefined if the bundle has nothing for it. */
  forAircraft(id: string): AircraftPresentation | undefined;
}

/** The format tag of the bundle. */
export const PRESENT_BUNDLE_SCHEMA = 'present-bundle/1';

/**
 * Reads the bundle's envelope.
 *
 * @throws PresentError if it is not a bundle of this format.
 */
export function parsePresentBundle(json: unknown): PresentBundle {
  const root = new Slice(json);
  const schema = root.s('schema');
  if (schema !== PRESENT_BUNDLE_SCHEMA) {
    root.at('schema').fail(`\`${schema}\` is not \`${PRESENT_BUNDLE_SCHEMA}\``);
  }
  const hashSlice = root.at('sim_pack_sha256');
  const simPackSha256 = hashSlice.str();
  if (!/^[0-9a-f]{64}$/.test(simPackSha256)) {
    hashSlice.fail('expected 64 lower-case hex digits');
  }
  const aircraft = root.at('aircraft');
  const huds = root.at('huds');
  const cockpits = root.at('cockpits');
  const models = root.at('models');
  const audio = root.at('audio_profiles');
  const fonts = root.at('fonts');
  const pick = (table: Slice, ref: Slice | undefined): Slice | undefined =>
    ref === undefined ? undefined : table.at(ref.str());
  return {
    simPackSha256,
    aircraftIds: () => aircraft.keys(),
    forAircraft(id) {
      if (!aircraft.has(id)) return undefined;
      const refs = aircraft.at(id);
      const hud = pick(huds, refs.opt('hud'));
      return {
        hud,
        cockpit: pick(cockpits, refs.opt('cockpit')),
        model: pick(models, refs.opt('model')),
        audio: pick(audio, refs.opt('audio')),
        font: hud === undefined ? undefined : pick(fonts, hud.at('font')),
      };
    },
  };
}

/**
 * The SHA-256 of a sim pack's content (the pack header's content hash) as 64 hex digits.
 *
 * @throws RangeError if `bytes` is not a pack of the supported format.
 */
export function cpackContentHash(bytes: Uint8Array): string {
  const end = CPACK_HEADER.H_CONTENT_HASH + CPACK_HEADER.CONTENT_HASH_BYTES;
  if (bytes.byteLength < end) {
    throw new RangeError('SYS-041: not a content pack (too short for its header)');
  }
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (dv.getUint32(CPACK_HEADER.H_MAGIC, true) !== CPACK_HEADER.MAGIC) {
    throw new RangeError('SYS-041: not a content pack (wrong magic)');
  }
  if (dv.getUint32(CPACK_HEADER.H_VERSION, true) !== CPACK_HEADER.VERSION) {
    throw new RangeError('SYS-041: the content pack is of another format version');
  }
  let out = '';
  for (let i = CPACK_HEADER.H_CONTENT_HASH; i < end; i++) {
    out += (bytes[i] ?? 0).toString(16).padStart(2, '0');
  }
  return out;
}

/**
 * Checks that a bundle belongs with a sim pack.
 *
 * @throws PresentError naming both hashes if it does not (a stale or mismatched build).
 */
export function checkBundleMatchesPack(bundle: PresentBundle, pack: Uint8Array): void {
  const packHash = cpackContentHash(pack);
  if (bundle.simPackSha256 !== packHash) {
    throw new PresentError(
      '/sim_pack_sha256',
      `the presentation data was baked with sim pack ${bundle.simPackSha256}, but the loaded pack is ${packHash}; rebuild the content`,
    );
  }
}
