import { describe, expect, it } from 'vitest';
import { newAircraftStatus, type AircraftStatus } from './aircraft-status.ts';
import { ASSIST_LEVEL, OWNSHIP_FLAG, OWNSHIP_FLAG2 } from './layout.gen.ts';
import {
  CAUTION_MASK,
  WARNING,
  WARNING_MASK,
  WARNINGS,
  warningBits,
  warningSeverity,
  type WarningId,
} from './warnings.ts';

/** The v0.1b rule (`ui/src/flight-warnings.ts` before the move), kept as the reference. */
function v01bWarnings(s: AircraftStatus): Set<WarningId> {
  const out = new Set<WarningId>();
  if (s.assist === 0) return out;
  if ((s.flags & (OWNSHIP_FLAG.SPECTATOR | OWNSHIP_FLAG.CRASHED)) !== 0) return out;
  const on = (bit: number): boolean => (s.flags2 & bit) !== 0;
  const landing = (s.gearBits & (1 << 6)) !== 0 && s.cas < 100;
  const sinking = -s.vs;
  const groundComing =
    !on(OWNSHIP_FLAG2.WOW) && !landing && s.agl < 400 && sinking >= 1 && s.agl / sinking < 5;
  const add = (id: WarningId, when: boolean): void => {
    if (when) out.add(id);
  };
  add('stall', (s.flags & OWNSHIP_FLAG.STALL_WARN) !== 0);
  add('pullup', groundComing);
  add('departed', on(OWNSHIP_FLAG2.DEPARTED));
  add('overstress', on(OWNSHIP_FLAG2.OVERSTRESS));
  add('alpha', on(OWNSHIP_FLAG2.ALPHA_LIMIT));
  add('g', on(OWNSHIP_FLAG2.G_LIMIT));
  add('overspeed', on(OWNSHIP_FLAG2.OVERSPEED));
  add('gear-speed', on(OWNSHIP_FLAG2.GEAR_OVERSPEED));
  add('flap-speed', on(OWNSHIP_FLAG2.FLAP_OVERSPEED));
  add('gear', on(OWNSHIP_FLAG2.GEAR_WARNING));
  return out;
}

const idsOf = (bits: number): Set<WarningId> =>
  new Set(WARNINGS.filter((w) => (bits & w.bit) !== 0).map((w) => w.id));

/** A small xorshift so the generated statuses are the same on every run. */
function rng(seed: number): () => number {
  let x = seed >>> 0 || 1;
  return () => {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    return (x >>> 0) / 2 ** 32;
  };
}

describe('warningBits', () => {
  it('flags exactly what the v0.1b rule flagged, over 20 000 generated statuses', () => {
    const r = rng(7);
    const pick = <T>(list: readonly T[]): T => list[Math.floor(r() * list.length)] as T;
    for (let i = 0; i < 20_000; i++) {
      const s: AircraftStatus = {
        ...newAircraftStatus(),
        assist: pick([0, ASSIST_LEVEL.FULL, ASSIST_LEVEL.STANDARD, ASSIST_LEVEL.MANUAL]),
        flags: Math.floor(r() * 32),
        flags2: Math.floor(r() * 65_536),
        gearBits: pick([0, 1 << 6, 0xc0]),
        cas: pick([40, 99.9, 100, 150, 250]),
        agl: pick([0, 5, 20, 150, 240, 399, 400, 600, 3_000]),
        vs: pick([-200, -100, -50, -4, -1, -0.5, 0, 5]),
      };
      expect(idsOf(warningBits(s)), JSON.stringify(s)).toEqual(v01bWarnings(s));
    }
  });

  it('numbers the bits in GDD 54.8 priority order: ground collision first', () => {
    expect(WARNINGS.map((w) => w.bit)).toEqual(
      [...WARNINGS.map((w) => w.bit)].sort((a, b) => a - b),
    );
    expect(WARNINGS[0]?.id).toBe('pullup');
    expect(WARNINGS[1]?.id).toBe('stall');
    expect(WARNING_MASK & CAUTION_MASK).toBe(0);
    for (const w of WARNINGS) {
      expect((w.severity === 'warning' ? WARNING_MASK : CAUTION_MASK) & w.bit, w.id).toBe(w.bit);
    }
  });

  it('has none without an aircraft, for the spectator or a wreck', () => {
    const stall = { ...newAircraftStatus(), assist: 2, flags: OWNSHIP_FLAG.STALL_WARN };
    expect(warningBits(stall)).toBe(WARNING.STALL);
    expect(warningBits(undefined)).toBe(0);
    expect(warningBits({ ...stall, assist: 0 })).toBe(0);
    expect(warningBits({ ...stall, flags: stall.flags | OWNSHIP_FLAG.SPECTATOR })).toBe(0);
    expect(warningBits({ ...stall, flags: stall.flags | OWNSHIP_FLAG.CRASHED })).toBe(0);
  });

  it('gives the most serious severity', () => {
    expect(warningSeverity(0)).toBe('none');
    expect(warningSeverity(WARNING.GEAR)).toBe('caution');
    expect(warningSeverity(WARNING.GEAR | WARNING.OVERSTRESS)).toBe('warning');
  });
});
