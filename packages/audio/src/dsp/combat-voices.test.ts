/**
 * The duel's voices (ADR-42 amendment 1, ADR-45 AU1, AU2): the gun's pulse rate, the growl's pitch
 * and the lock tone's, the one-shot sounds the counters start, and the distance laws.
 *
 * Each voice is measured on its own: the duel's voices draw their own noise, so a render with the duel
 * and one without differ only by what the duel adds, and subtracting them isolates it.
 */
import { describe, expect, it } from 'vitest';
import { SEEKER_MODE, WARNING } from '@flying/sim-bridge';
import { levelDb, peakFrequency, render } from '../test-support/analysis.ts';
import { kestrelProfile } from '../test-support/fixtures.ts';
import type { AudioProfile, WeaponsProfile } from '../profile.ts';
import { FlightDsp, ONE_SHOT, P } from './flight-dsp.ts';

const FS = 48_000;
const shipped = kestrelProfile();
const weapons = shipped.weapons as WeaponsProfile;
/** The engine idling quietly underneath, as in the kernel tests. */
const profile: AudioProfile = {
  ...shipped,
  mix: { ...shipped.mix, levelOffsetDb: 0, chaseOffsetDb: 0 },
};

type Set = (p: Float64Array, block: number) => void;
const idle: Set = (p) => {
  p[P.ENGINE_BITS] = 1;
  p[P.CORE] = 0.1;
  p[P.QBAR] = 2000;
  p[P.COCKPIT] = 0;
  // Quiet enough that the limiter never bends what the duel adds.
  p[P.AIRCRAFT_BUS] = 0.5;
  p[P.WARNINGS_BUS] = 0.5;
};

/** What `set` adds to the idle aircraft (and to `base`, if given) over `seconds`, sample by sample. */
function added(
  set: Set,
  seconds: number,
  prof: AudioProfile = profile,
  base: Set = () => undefined,
): Float32Array {
  const run = (duel: boolean): Float32Array => {
    const dsp = new FlightDsp(prof);
    return render(dsp, seconds, (block) => {
      idle(dsp.params, block);
      base(dsp.params, block);
      if (duel) set(dsp.params, block);
    }).left;
  };
  const without = run(false);
  const with_ = run(true);
  return Float32Array.from(with_, (v, i) => v - (without[i] as number));
}

/** Blocks in `seconds` (128 frames each). */
const blocks = (seconds: number): number => Math.round((seconds * FS) / 128);

/** The pulse rate of `x`: the lag of the strongest autocorrelation of its envelope near `nearHz`. */
function pulseRate(x: Float32Array, nearHz: number): number {
  // The envelope: rectified and smoothed over about 1 ms, then without its mean.
  const env = new Float64Array(x.length);
  const k = 1 - Math.exp(-1 / (0.001 * FS));
  let e = 0;
  for (let i = 0; i < x.length; i++) {
    e += (Math.abs(x[i] as number) - e) * k;
    env[i] = e;
  }
  const mean = env.reduce((a, b) => a + b, 0) / env.length;
  for (let i = 0; i < env.length; i++) env[i] = (env[i] as number) - mean;
  const ac = (lag: number): number => {
    let sum = 0;
    for (let i = 0; i + lag < env.length; i++) sum += (env[i] as number) * (env[i + lag] as number);
    return sum / (env.length - lag);
  };
  const lo = Math.floor(FS / nearHz / 1.4);
  const hi = Math.ceil(FS / nearHz / 0.6);
  let best = lo;
  let bestV = -Infinity;
  for (let lag = lo; lag <= hi; lag++) {
    const v = ac(lag);
    if (v > bestV) {
      bestV = v;
      best = lag;
    }
  }
  const a = ac(best - 1);
  const c = ac(best + 1);
  const shift = (0.5 * (a - c)) / (a - 2 * bestV + c);
  return FS / (best + (Number.isFinite(shift) ? shift : 0));
}

describe('AU1: the gun voice repeats at 1 700 / 60 Hz (within 1 %)', () => {
  const fire: Set = (p) => {
    p[P.GUN_FIRING] = 1;
  };
  const measured = (prof: AudioProfile): number =>
    pulseRate(added(fire, 2.2, prof).subarray(Math.round(0.2 * FS)), 1700 / 60);

  it('over 2 s of fire, rendered offline', () => {
    const rate = measured(profile);
    console.info(`AU1: ${rate.toFixed(3)} Hz (want ${(1700 / 60).toFixed(3)})`);
    expect(Math.abs(rate / (1700 / 60) - 1)).toBeLessThan(0.01);
  });

  it('would miss with the wrong rate (the seeded fault)', () => {
    const wrong = { ...profile, weapons: { ...weapons, gun: { ...weapons.gun, rateHz: 25 } } };
    expect(Math.abs(measured(wrong) / (1700 / 60) - 1)).toBeGreaterThan(0.05);
  });

  it('fires its first pulse at once and stops when the trigger is released', () => {
    const x = added((p, b) => {
      p[P.GUN_FIRING] = b < blocks(0.5) ? 1 : 0;
    }, 1.2);
    // Loud within the first 5 ms of the press.
    expect(levelDb(x, 0, Math.round(0.005 * FS))).toBeGreaterThan(-40);
    // Silent a tenth of a second after the release.
    expect(levelDb(x, Math.round(0.65 * FS), Math.round(1.2 * FS))).toBeLessThan(-100);
  });

  it('ducks with the aircraft bus under a warning', () => {
    const fireUnder = (warn: number): number => {
      const x = added(
        (p) => {
          p[P.GUN_FIRING] = 1;
        },
        1.5,
        profile,
        (p) => {
          p[P.WARNINGS] = warn;
        },
      );
      return levelDb(x, Math.round(0.5 * FS), x.length);
    };
    expect(fireUnder(WARNING.STALL) - fireUnder(0)).toBeCloseTo(shipped.mix.duckDb, 0);
  });
});

describe('AU2: the seeker growl and the lock tone', () => {
  const seek =
    (mode: number, signal: number): Set =>
    (p) => {
      p[P.SEEKER] = mode;
      p[P.SIGNAL] = signal;
    };
  const pitch = (set: Set, from: number, to: number): number =>
    peakFrequency(added(set, 1.2).subarray(Math.round(0.2 * FS)), FS, from, to);

  it('rises monotonically with the signal ratio over 1 to 4, as the data says', () => {
    let last = 0;
    for (const signal of [1, 1.5, 2, 2.5, 3, 3.5, 4]) {
      const f = pitch(seek(SEEKER_MODE.SEARCH, signal), 300, 1200);
      const want =
        weapons.growl.baseHz +
        weapons.growl.spanHz * Math.min(Math.max((signal - 0.3) / 3.7, 0), 1);
      expect(f, `signal ${String(signal)}`).toBeGreaterThan(last);
      expect(Math.abs(f / want - 1), `signal ${String(signal)}`).toBeLessThan(0.01);
      last = f;
    }
  });

  it('growls in TONE whatever the signal, and not in SEARCH without one or with the seeker off', () => {
    expect(levelDb(added(seek(SEEKER_MODE.TONE, 0.1), 0.5))).toBeGreaterThan(-60);
    expect(levelDb(added(seek(SEEKER_MODE.SEARCH, 0.3), 0.5))).toBeLessThan(-150);
    expect(levelDb(added(seek(SEEKER_MODE.OFF, 3), 0.5))).toBeLessThan(-150);
  });

  it('the lock tone is within 1 % of the profile, and replaces the growl', () => {
    const f = pitch(seek(SEEKER_MODE.LOCKED, 3), 300, 3000);
    console.info(`AU2: lock tone ${f.toFixed(1)} Hz (want ${String(weapons.lockTone.freqHz)})`);
    expect(Math.abs(f / weapons.lockTone.freqHz - 1)).toBeLessThan(0.01);
  });
});

describe('the one-shot sounds', () => {
  /** The level of what the duel adds in `[from, to)` seconds. */
  const window = (x: Float32Array, from: number, to: number): number =>
    levelDb(x, Math.round(from * FS), Math.round(to * FS));

  it('adopt the first counters after a reset without a sound, and sound once per rise', () => {
    // The page's first values (a session already under way) are not news; the rise at 1 s is.
    const x = added((p, b) => {
      p[P.LAUNCHES] = b < blocks(1) ? 5 : 6;
    }, 4);
    expect(window(x, 0, 0.99)).toBeLessThan(-150);
    expect(window(x, 1, 1.3)).toBeGreaterThan(-40);
    // It decays with the data's time constant (two of them: 17.4 dB) and is not started again while
    // the counter holds.
    const tau = weapons.launch.decayS;
    const fall = window(x, 1.1, 1.3) - window(x, 1.1 + 2 * tau, 1.3 + 2 * tau);
    expect(fall).toBeGreaterThan(20 * Math.log10(Math.E ** 2) - 1.5);
    expect(fall).toBeLessThan(20 * Math.log10(Math.E ** 2) + 1.5);
  });

  it('a counter that falls (a new session) is adopted silently', () => {
    const x = added((p, b) => {
      p[P.EXPLOSIONS] = b < blocks(0.5) ? 7 : 0;
      p[P.HITS_TAKEN] = b < blocks(0.5) ? 3 : 0;
    }, 1);
    expect(levelDb(x)).toBeLessThan(-150);
  });

  it("a hit taken is a click of the data's length", () => {
    const x = added((p, b) => {
      p[P.HITS_TAKEN] = b < blocks(0.5) ? 0 : 1;
    }, 1);
    const len = weapons.hit.lengthMs / 1000;
    expect(window(x, 0.5, 0.5 + len)).toBeGreaterThan(-45);
    expect(window(x, 0.5 + len + 0.02, 0.9)).toBeLessThan(window(x, 0.5, 0.5 + len) - 40);
  });

  it('an explosion is 6 dB quieter per doubling of distance beyond its reference', () => {
    const boom = (distM: number): number => {
      const x = added((p, b) => {
        p[P.EXPLOSIONS] = b < blocks(0.2) ? 0 : 1;
        p[P.EXPLOSION_DIST_M] = distM;
      }, 1);
      return window(x, 0.2, 1);
    };
    const ref = weapons.explosion.refDistanceM;
    expect(boom(ref) - boom(4 * ref)).toBeCloseTo(20 * Math.log10(4), 0);
    // Closer than the reference is no louder, and an unknown distance is heard at the reference.
    expect(boom(ref / 5)).toBeCloseTo(boom(ref), 1);
    expect(boom(0)).toBeCloseTo(boom(ref), 1);
  });
});

describe('the one-shot reports (AU3)', () => {
  it('says which one-shot sounds started this block, once per rise, and not on adoption', () => {
    const dsp = new FlightDsp(profile);
    const l = new Float32Array(128);
    const r = new Float32Array(128);
    const block = (set: (p: Float64Array) => void): number => {
      idle(dsp.params, 0);
      set(dsp.params);
      dsp.process(l, r, 128);
      const shots = dsp.oneShots;
      dsp.oneShots = 0; // as the worklet does once it has reported them
      return shots;
    };
    expect(block((p) => (p[P.LAUNCHES] = 3))).toBe(0);
    expect(
      block((p) => {
        p[P.LAUNCHES] = 4;
        p[P.EXPLOSIONS] = 1;
      }),
    ).toBe(ONE_SHOT.LAUNCH | ONE_SHOT.EXPLOSION);
    expect(block(() => undefined)).toBe(0);
    expect(block((p) => (p[P.HITS_TAKEN] = 1))).toBe(ONE_SHOT.HIT);
  });
});

describe("the bandit's gun", () => {
  const at = (distM: number): Float32Array =>
    added((p) => {
      p[P.ENEMY_GUN_DIST_M] = distM;
    }, 1.5);

  it('falls 6 dB per doubling of distance beyond its reference, and is silent at 0', () => {
    const ref = weapons.enemyGun.refDistanceM;
    const level = (d: number): number => levelDb(at(d), Math.round(0.3 * FS));
    expect(level(ref) - level(4 * ref)).toBeCloseTo(20 * Math.log10(4), 0);
    expect(levelDb(at(0))).toBeLessThan(-150);
  });

  it('is duller than the own gun: low-passed at its data frequency', () => {
    const own = added((p) => {
      p[P.GUN_FIRING] = 1;
    }, 1.5);
    const theirs = at(weapons.enemyGun.refDistanceM);
    const highShare = (x: Float32Array): number => {
      // The share of the level above the low-pass, by a first difference (a crude high-pass).
      const d = Float32Array.from(x, (v, i) => v - (x[i - 1] ?? 0));
      return levelDb(d) - levelDb(x);
    };
    expect(highShare(theirs)).toBeLessThan(highShare(own) - 6);
  });
});

describe('without a weapons block', () => {
  it('the duel makes no sound at all', () => {
    const bare: AudioProfile = { ...profile };
    delete (bare as { weapons?: WeaponsProfile }).weapons;
    const x = added(
      (p, b) => {
        p[P.GUN_FIRING] = 1;
        p[P.ENEMY_GUN_DIST_M] = 300;
        p[P.SEEKER] = SEEKER_MODE.TONE;
        p[P.SIGNAL] = 2;
        p[P.LAUNCHES] = b;
        p[P.EXPLOSIONS] = b;
      },
      0.5,
      bare,
    );
    expect(x.every((v) => v === 0)).toBe(true);
  });
});
