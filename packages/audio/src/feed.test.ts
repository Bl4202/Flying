import { describe, expect, it } from 'vitest';
import {
  COMBAT_COUNTER,
  COMBAT_FLAG,
  newAircraftStatus,
  newCombatStatus,
  OWNSHIP_FLAG,
  OWNSHIP_FLAG2,
  SEEKER_MODE,
  WARNING,
} from '@flying/sim-bridge';
import { P, PARAM_COUNT, PARAM_NAMES, PARAM_RANGE } from './dsp/flight-dsp.ts';
import { defaultBuses, fillParams } from './feed.ts';
import { bytesPerCall } from './test-support/garbage.ts';

const flying = () => ({
  ...newAircraftStatus(),
  assist: 2,
  engineCore: 0.8,
  abLevel: 0.5,
  engineBits: 3,
  qbar: 12_345,
  alpha: 0.2,
  alphaWarn: 0.45,
  airbrake: 0.25,
  groundSpeed: 123,
  flags2: OWNSHIP_FLAG2.WOW | OWNSHIP_FLAG2.GEAR_WARNING,
});

describe('the parameter tables', () => {
  it('have one entry per parameter, in the order of P, each default inside its range', () => {
    expect(PARAM_NAMES).toHaveLength(PARAM_COUNT);
    expect(PARAM_RANGE).toHaveLength(PARAM_COUNT);
    expect(new Set(PARAM_NAMES).size).toBe(PARAM_COUNT);
    for (const [min, max, def] of PARAM_RANGE) {
      expect(min).toBeLessThan(max);
      expect(def).toBeGreaterThanOrEqual(min);
      expect(def).toBeLessThanOrEqual(max);
    }
    expect(PARAM_NAMES[P.CORE]).toBe('core');
    expect(PARAM_NAMES[P.WARNINGS]).toBe('warnings');
    expect(PARAM_NAMES[P.WARNINGS_BUS]).toBe('warningsBus');
  });

  it('can carry every warning bit and every engine bit exactly (float32 and the range)', () => {
    const all = Object.values(WARNING).reduce((a, b) => a | b, 0);
    expect(all).toBeLessThanOrEqual(PARAM_RANGE[P.WARNINGS]?.[1] ?? 0);
    expect(Math.fround(all)).toBe(all);
    expect(PARAM_RANGE[P.ENGINE_BITS]?.[1]).toBeGreaterThanOrEqual(15);
  });
});

describe('fillParams', () => {
  it('copies the engine, air and ground state, and the mix', () => {
    const out = new Float64Array(PARAM_COUNT);
    fillParams(out, flying(), undefined, 'cockpit', 0.7, 0.9);
    expect(out[P.CORE]).toBe(0.8);
    expect(out[P.AB_LEVEL]).toBe(0.5);
    expect(out[P.ENGINE_BITS]).toBe(3);
    expect(out[P.QBAR]).toBe(12_345);
    expect(out[P.ALPHA]).toBe(0.2);
    expect(out[P.ALPHA_WARN]).toBe(0.45);
    expect(out[P.AIRBRAKE]).toBe(0.25);
    expect(out[P.GROUND_SPEED]).toBe(123);
    expect(out[P.ON_GROUND]).toBe(1);
    expect(out[P.WARNINGS]).toBe(WARNING.GEAR);
    expect(out[P.COCKPIT]).toBe(1);
    expect(out[P.AIRCRAFT_BUS]).toBe(0.7);
    expect(out[P.WARNINGS_BUS]).toBe(0.9);
  });

  it('chooses the chase mix, and leaves the master to whatever applies it', () => {
    const out = new Float64Array(PARAM_COUNT).fill(0.42);
    fillParams(out, flying(), undefined, 'chase', 1, 1);
    expect(out[P.COCKPIT]).toBe(0);
    expect(out[P.MASTER]).toBe(0.42);
  });

  it('is silent for no status, a spectator and a wreck, but keeps the mix', () => {
    const cases = [
      undefined,
      { ...flying(), flags: OWNSHIP_FLAG.SPECTATOR },
      { ...flying(), flags: OWNSHIP_FLAG.CRASHED },
    ];
    for (const status of cases) {
      const out = new Float64Array(PARAM_COUNT).fill(9);
      fillParams(out, status, undefined, 'chase', 0.5, 0.6);
      expect(out[P.ENGINE_BITS]).toBe(0);
      expect(out[P.CORE]).toBe(0);
      expect(out[P.QBAR]).toBe(0);
      expect(out[P.WARNINGS]).toBe(0);
      expect(out[P.COCKPIT]).toBe(0);
      expect(out[P.AIRCRAFT_BUS]).toBe(0.5);
      expect(out[P.WARNINGS_BUS]).toBe(0.6);
    }
  });

  it('raises the same warnings the strip and the HUD show (one rule set)', () => {
    const out = new Float64Array(PARAM_COUNT);
    fillParams(
      out,
      { ...flying(), flags: OWNSHIP_FLAG.STALL_WARN, flags2: 0 },
      undefined,
      'cockpit',
      1,
      1,
    );
    expect(out[P.WARNINGS]).toBe(WARNING.STALL);
    fillParams(out, { ...flying(), assist: 0 }, undefined, 'cockpit', 1, 1);
    expect(out[P.WARNINGS]).toBe(0);
  });

  describe('the duel', () => {
    const duel = () => {
      const c = newCombatStatus();
      c.flags = COMBAT_FLAG.ACTIVE | COMBAT_FLAG.GUN_FIRING | COMBAT_FLAG.BANDIT_GUN_FIRING;
      c.seeker = SEEKER_MODE.TONE;
      c.signal = 2.5;
      c.banditRange = 900;
      c.explosionRange = 1500;
      c.counters[COMBAT_COUNTER.MISSILES_FIRED] = 2;
      c.counters[COMBAT_COUNTER.HITS_TAKEN] = 3;
      c.counters[COMBAT_COUNTER.EXPLOSIONS] = 4;
      return c;
    };

    it('passes on the guns, the seeker, the counters and the explosion range', () => {
      const out = new Float64Array(PARAM_COUNT);
      fillParams(out, flying(), duel(), 'cockpit', 1, 1);
      expect(out[P.GUN_FIRING]).toBe(1);
      expect(out[P.ENEMY_GUN_DIST_M]).toBe(900);
      expect([out[P.SEEKER], out[P.SIGNAL]]).toEqual([SEEKER_MODE.TONE, 2.5]);
      expect([out[P.LAUNCHES], out[P.HITS_TAKEN], out[P.EXPLOSIONS]]).toEqual([2, 3, 4]);
      expect(out[P.EXPLOSION_DIST_M]).toBe(1500);
    });

    it("silences the bandit's gun while it is not firing, and all of it without a duel", () => {
      const out = new Float64Array(PARAM_COUNT);
      const c = duel();
      c.flags = COMBAT_FLAG.ACTIVE;
      fillParams(out, flying(), c, 'cockpit', 1, 1);
      expect([out[P.GUN_FIRING], out[P.ENEMY_GUN_DIST_M]]).toEqual([0, 0]);
      fillParams(out, flying(), undefined, 'cockpit', 1, 1);
      for (const k of [P.GUN_FIRING, P.ENEMY_GUN_DIST_M, P.SEEKER, P.SIGNAL, P.LAUNCHES]) {
        expect(out[k]).toBe(0);
      }
    });

    it('keeps the counters for a wreck (its own explosion is heard), but not the guns or seeker', () => {
      const out = new Float64Array(PARAM_COUNT);
      fillParams(out, { ...flying(), flags: OWNSHIP_FLAG.CRASHED }, duel(), 'cockpit', 1, 1);
      expect([out[P.GUN_FIRING], out[P.ENEMY_GUN_DIST_M], out[P.SEEKER]]).toEqual([0, 0, 0]);
      expect([out[P.EXPLOSIONS], out[P.EXPLOSION_DIST_M]]).toEqual([4, 1500]);
    });

    it('can carry the counters exactly (float32 and the range)', () => {
      for (const k of [P.LAUNCHES, P.HITS_TAKEN, P.EXPLOSIONS]) {
        const max = PARAM_RANGE[k]?.[1] ?? 0;
        expect(max).toBeGreaterThanOrEqual(1_000_000);
        expect(Math.fround(max)).toBe(max);
      }
    });
  });

  it('has default buses of full volume', () => {
    expect(defaultBuses()).toEqual({ master: 1, aircraft: 1, warnings: 1, ui: 1 });
  });

  it('makes no garbage', async () => {
    const out = new Float64Array(PARAM_COUNT);
    const status = flying();
    const combat = newCombatStatus();
    combat.flags = COMBAT_FLAG.ACTIVE | COMBAT_FLAG.GUN_FIRING | COMBAT_FLAG.BANDIT_GUN_FIRING;
    combat.banditRange = 640.5;
    combat.explosionRange = 1200.25;
    combat.signal = 1.5;
    const bytes = await bytesPerCall((i) => {
      fillParams(
        out,
        i % 7 === 0 ? undefined : status,
        i % 3 === 0 ? undefined : combat,
        i % 2 === 0 ? 'cockpit' : 'chase',
        1,
        1,
      );
    });
    expect(bytes).toBeLessThan(16);
  });
});
