import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { SEEKER_MODE, WARNING } from '@flying/sim-bridge';
import { levelDb, peakFrequency, render } from '../test-support/analysis.ts';
import { kestrelProfile } from '../test-support/fixtures.ts';
import { FlightDsp, MAX_BLOCK, P, rpmFor, softLimit, TONE } from './flight-dsp.ts';

/** The profile as shipped, with its loudness calibration (A5 tunes it; the soft limiter is near at full power). */
const shipped = kestrelProfile();
/** The kernel tests measure at the reference gain, so the soft limiter never bends a level comparison. */
const profile = { ...shipped, mix: { ...shipped.mix, levelOffsetDb: 0, chaseOffsetDb: 0 } };

/** A synthesis held at one state (the spool has settled) with its cockpit mix off, so the engine is heard bare. */
function held(set: (p: Float64Array) => void, seconds = 4): { dsp: FlightDsp; out: Float32Array } {
  const dsp = new FlightDsp(profile);
  set(dsp.params);
  dsp.params[P.COCKPIT] = 0;
  const { left } = render(dsp, seconds, () => {
    set(dsp.params);
  });
  return { dsp, out: left.subarray(left.length - 48_000) };
}

/**
 * The level of what `set` adds to `base`. The noise is seeded and the same noise is drawn in every
 * state, so two renders differ only by the voice that was changed: subtracting them isolates it from
 * the engine's floor (a stopped engine still windmills).
 */
function added(
  base: (p: Float64Array) => void,
  set: (p: Float64Array) => void,
  seconds = 3,
): number {
  const a = held(base, seconds).out;
  const b = held(set, seconds).out;
  return levelDb(Float32Array.from(b, (v, i) => v - (a[i] as number)));
}

const engineOn =
  (core: number, ab = 0) =>
  (p: Float64Array) => {
    p[P.ENGINE_BITS] = ab > 0 ? 3 : 1;
    p[P.CORE] = core;
    p[P.AB_LEVEL] = ab;
  };

describe('rpmFor', () => {
  const curve = profile.engine.rpmByCore;
  it('follows the data curve: idle at zero, military at one, linear between the points', () => {
    expect(rpmFor(curve, 0)).toBe(2000);
    expect(rpmFor(curve, 0.06)).toBeCloseTo(8000, 6);
    expect(rpmFor(curve, 0.28)).toBeCloseTo(8650, 6);
    expect(rpmFor(curve, 1)).toBe(10_400);
    expect(rpmFor(curve, 2)).toBe(10_400);
    expect(rpmFor(curve, -1)).toBe(2000);
  });
  it('is rising everywhere', () => {
    let last = -1;
    for (let c = 0; c <= 1.0001; c += 0.01) {
      const r = rpmFor(curve, c);
      expect(r).toBeGreaterThanOrEqual(last);
      last = r;
    }
  });
});

describe('A1: the blade-pass tone is at rpm / 60 x blades (within 1 %)', () => {
  /** The Kestrel's blade count, from GDD C.13 and not from the profile under test: a wrong profile misses. */
  const BLADES = 24;
  const cases: [string, number, number][] = [
    ['idle', 0.06, 0],
    ['military', 1, 0],
    ['afterburner', 1, 1],
  ];
  for (const [name, core, ab] of cases) {
    it(`at ${name}`, () => {
      const { dsp, out } = held(engineOn(core, ab));
      const expected = (dsp.rpm / 60) * BLADES;
      const found = peakFrequency(out, 48_000, 0.5 * expected, 1.2 * expected);
      expect(Math.abs(found - expected) / expected).toBeLessThan(0.01);
    });
  }

  it('is 3200 Hz at the military rpm of 8 000 (idle) and 4160 Hz at 10 400', () => {
    expect((8000 / 60) * 24).toBeCloseTo(3200, 6);
    expect((10_400 / 60) * 24).toBeCloseTo(4160, 6);
  });

  it('would miss with the wrong blade count (the seeded fault)', () => {
    const { dsp, out } = held(engineOn(1));
    const wrong = (dsp.rpm / 60) * 12;
    const found = peakFrequency(out, 48_000, 0.5 * wrong, 1.2 * wrong);
    // The 12-blade "tone" is not where the profile's own 24-blade tone is.
    expect(Math.abs(found - (dsp.rpm / 60) * 24) / ((dsp.rpm / 60) * 24)).toBeGreaterThan(0.01);
  });
});

describe('A2: the engine gets louder with thrust, and the afterburner adds 6 dB', () => {
  it('rises monotonically with the core thrust', () => {
    let last = -Infinity;
    for (const core of [0.06, 0.2, 0.4, 0.6, 0.8, 1]) {
      const { out } = held(engineOn(core));
      const db = levelDb(out);
      expect(db).toBeGreaterThan(last);
      last = db;
    }
  });

  it('adds 6 +/- 1 dB with the afterburner lit', () => {
    const mil = levelDb(held(engineOn(1, 0)).out);
    const ab = levelDb(held(engineOn(1, 1)).out);
    expect(ab - mil).toBeGreaterThan(5);
    expect(ab - mil).toBeLessThan(7);
  });

  it('is nearly silent with the engine stopped, and a flameout spools down', () => {
    const running = levelDb(held(engineOn(0.6)).out);
    const stopped = levelDb(held((p) => (p[P.ENGINE_BITS] = 0)).out);
    expect(running - stopped).toBeGreaterThan(10);
    const dsp = new FlightDsp(profile);
    engineOn(1)(dsp.params);
    render(dsp, 3);
    const rpmRunning = dsp.rpm;
    dsp.params[P.ENGINE_BITS] = 0;
    render(dsp, 3);
    expect(dsp.rpm).toBeLessThan(rpmRunning - 2000);
  });

  it('spools up after a throttle change: slowly, by the profile time constant', () => {
    const dsp = new FlightDsp(profile);
    engineOn(0.06)(dsp.params);
    render(dsp, 3);
    const idle = dsp.rpm;
    engineOn(1)(dsp.params);
    render(dsp, profile.engine.followS);
    const after1 = dsp.rpm;
    // After one time constant 63 % of the way, give or take the curve's shape.
    expect((after1 - idle) / (10_400 - idle)).toBeGreaterThan(0.45);
    expect((after1 - idle) / (10_400 - idle)).toBeLessThan(0.8);
  });
});

describe('A3: the wind rises 10 +/- 2 dB per decade of dynamic pressure', () => {
  const calm = (p: Float64Array): void => {
    p[P.QBAR] = 1;
  };
  const windAt = (q: number): number =>
    added(calm, (p) => {
      p[P.QBAR] = q;
    });

  it('over three decades of q', () => {
    const levels = [100, 1000, 10_000].map(windAt);
    for (const step of [
      (levels[1] as number) - (levels[0] as number),
      (levels[2] as number) - (levels[1] as number),
    ]) {
      expect(step).toBeGreaterThan(8);
      expect(step).toBeLessThan(12);
    }
  });

  it('the airbrake adds to it', () => {
    const plain = windAt(5000);
    const braked = added(calm, (p) => {
      p[P.QBAR] = 5000;
      p[P.AIRBRAKE] = 1;
    });
    expect(braked - plain).toBeCloseTo(profile.aero.airbrakeDb, 0);
  });
});

describe('tyres and buffet', () => {
  const still = (p: Float64Array): void => {
    p[P.QBAR] = 1;
  };

  it('the tyres rumble with the ground speed, and only on the wheels', () => {
    const roll = (v: number, wow: number): number =>
      added(still, (p) => {
        p[P.QBAR] = 1;
        p[P.GROUND_SPEED] = v;
        p[P.ON_GROUND] = wow;
      });
    expect(roll(60, 1) - roll(30, 1)).toBeGreaterThan(4);
    expect(roll(60, 1) - roll(30, 1)).toBeLessThan(8);
    expect(roll(60, 0)).toBeLessThan(-150);
  });

  it('the buffet builds as alpha passes the margin below the warning angle', () => {
    const at = (deg: number): number =>
      added(
        (p) => {
          p[P.ALPHA_WARN] = (20 * Math.PI) / 180;
          p[P.ALPHA] = 0;
        },
        (p) => {
          p[P.ALPHA_WARN] = (20 * Math.PI) / 180;
          p[P.ALPHA] = (deg * Math.PI) / 180;
        },
      );
    expect(at(10)).toBeLessThan(-150);
    expect(at(19)).toBeGreaterThan(-100);
    expect(at(20) - at(19)).toBeGreaterThan(4);
    expect(at(20) - at(19)).toBeLessThan(8);
  });
});

describe('the warning tones', () => {
  it('pick the most urgent warning', () => {
    expect(FlightDsp.toneFor(0)).toBe(TONE.NONE);
    expect(FlightDsp.toneFor(WARNING.PULLUP | WARNING.STALL)).toBe(TONE.PULL_UP);
    expect(FlightDsp.toneFor(WARNING.STALL | WARNING.GEAR)).toBe(TONE.STALL);
    expect(FlightDsp.toneFor(WARNING.DEPARTED)).toBe(TONE.STALL);
    expect(FlightDsp.toneFor(WARNING.OVERSTRESS | WARNING.G)).toBe(TONE.OVER_G);
    expect(FlightDsp.toneFor(WARNING.GEAR | WARNING.ALPHA)).toBe(TONE.GEAR);
    expect(FlightDsp.toneFor(WARNING.OVERSPEED)).toBe(TONE.CAUTION);
  });

  const tonePeak = (bits: number, seconds = 1): number => {
    const dsp = new FlightDsp(profile);
    const set = (): void => {
      dsp.params[P.COCKPIT] = 0;
      dsp.params[P.AIRCRAFT_BUS] = 0; // the tone alone: a stopped engine still windmills at 800 Hz
      dsp.params[P.WARNINGS] = bits;
    };
    set();
    const { left } = render(dsp, seconds, set);
    return peakFrequency(left, 48_000, 300, 2000);
  };

  it('the stall horn is at its data frequency', () => {
    expect(Math.abs(tonePeak(WARNING.STALL) - profile.warnings.stallHorn.freqHz)).toBeLessThan(15);
  });
  it('the over-g tone is at its data frequency', () => {
    expect(Math.abs(tonePeak(WARNING.G) - profile.warnings.overG.freqHz)).toBeLessThan(15);
  });
  it('the gear chime is at its data frequency', () => {
    expect(Math.abs(tonePeak(WARNING.GEAR) - profile.warnings.gearChime.freqHz)).toBeLessThan(15);
  });
  it('PULL UP warbles between its two pitches', () => {
    const dsp = new FlightDsp(profile);
    const set = (): void => {
      dsp.params[P.AIRCRAFT_BUS] = 0;
      dsp.params[P.WARNINGS] = WARNING.PULLUP;
    };
    set();
    const { left } = render(dsp, 1, set);
    // At 3 Hz each pitch lasts a sixth of a second: the high one first, then the low one.
    const window = (from: number, to: number): Float32Array =>
      left.subarray(Math.round(from * 48_000), Math.round(to * 48_000));
    const high = peakFrequency(window(0.02, 0.15), 48_000, 300, 2000);
    const low = peakFrequency(window(0.19, 0.31), 48_000, 300, 2000);
    expect(Math.abs(high - profile.warnings.pullUp.highHz)).toBeLessThan(30);
    expect(Math.abs(low - profile.warnings.pullUp.lowHz)).toBeLessThan(30);
  });

  it('the stall horn pulses faster as alpha passes the warning angle', () => {
    const edges = (over: number): number => {
      const dsp = new FlightDsp(profile);
      dsp.params[P.ALPHA_WARN] = 0.4;
      dsp.params[P.ALPHA] = 0.4 + over;
      dsp.params[P.WARNINGS] = WARNING.STALL;
      dsp.params[P.COCKPIT] = 0;
      dsp.params[P.AIRCRAFT_BUS] = 0; // the horn alone
      const { left } = render(dsp, 2);
      // Count the on-to-off changes of the horn's gate over 2 s.
      let count = 0;
      let on = false;
      for (let i = 0; i < left.length; i += 480) {
        let peak = 0;
        for (let j = i; j < Math.min(i + 480, left.length); j++)
          peak = Math.max(peak, Math.abs(left[j] as number));
        const now = peak > 0.05;
        if (on && !now) count++;
        on = now;
      }
      return count / 2;
    };
    const slow = edges(0);
    const fast = edges(0.1);
    expect(slow).toBeGreaterThanOrEqual(profile.warnings.stallHorn.pulseMinHz - 1);
    expect(fast).toBeGreaterThan(slow + 2);
  });

  it('reports the onset of a tone, once', () => {
    const dsp = new FlightDsp(profile);
    const l = new Float32Array(128);
    const r = new Float32Array(128);
    dsp.process(l, r, 128);
    expect(dsp.onsetFrame).toBe(-1);
    dsp.params[P.WARNINGS] = WARNING.STALL;
    dsp.process(l, r, 128);
    expect(dsp.onsetFrame).toBe(0);
    dsp.onsetFrame = -1;
    dsp.process(l, r, 128);
    expect(dsp.onsetFrame).toBe(-1);
  });
});

describe('A4 (kernel half): the aircraft bus ducks 6 dB under a warning and the warnings bus never does', () => {
  it('ducks the aircraft bus by the profile amount, quickly, and releases slowly', () => {
    const dsp = new FlightDsp(profile);
    engineOn(1)(dsp.params);
    render(dsp, 2, () => {
      engineOn(1)(dsp.params);
    });
    expect(dsp.duckDb).toBeCloseTo(0, 6);
    dsp.params[P.WARNINGS] = WARNING.STALL;
    render(dsp, 0.1, () => (dsp.params[P.WARNINGS] = WARNING.STALL));
    expect(dsp.duckDb).toBeCloseTo(profile.mix.duckDb, 0);
    expect(profile.mix.duckDb).toBe(-6);
    dsp.params[P.WARNINGS] = 0;
    render(dsp, 0.25, () => (dsp.params[P.WARNINGS] = 0));
    // After one release time constant it is about 63 % of the way back.
    expect(dsp.duckDb).toBeLessThan(-1);
    expect(dsp.duckDb).toBeGreaterThan(-4);
    render(dsp, 2, () => (dsp.params[P.WARNINGS] = 0));
    expect(dsp.duckDb).toBeCloseTo(0, 1);
  });

  it('the engine level falls 6 dB under a warning, and the warning tone itself does not', () => {
    const level = (bits: number, from: number, to: number, bus: 'engine' | 'tone') => {
      const dsp = new FlightDsp(profile);
      dsp.params[P.COCKPIT] = 0;
      // Mute the bus we are not measuring.
      const set = (): void => {
        if (bus === 'engine') engineOn(1)(dsp.params);
        dsp.params[P.WARNINGS] = bits;
      };
      set();
      const { left } = render(dsp, to, set);
      return levelDb(left, Math.round(from * 48_000), Math.round(to * 48_000));
    };
    // Engine only, with the ducking warning "on" but the horn bus turned off: only the duck acts.
    const withDuck = (() => {
      const dsp = new FlightDsp(profile);
      dsp.params[P.COCKPIT] = 0;
      dsp.params[P.WARNINGS_BUS] = 0;
      const set = (): void => {
        engineOn(1)(dsp.params);
        dsp.params[P.WARNINGS] = WARNING.STALL;
        dsp.params[P.WARNINGS_BUS] = 0;
      };
      set();
      const { left } = render(dsp, 3, set);
      return levelDb(left, 2 * 48_000, 3 * 48_000);
    })();
    const noDuck = (() => {
      const dsp = new FlightDsp(profile);
      dsp.params[P.COCKPIT] = 0;
      const set = (): void => {
        engineOn(1)(dsp.params);
        dsp.params[P.WARNINGS_BUS] = 0;
      };
      set();
      const { left } = render(dsp, 3, set);
      return levelDb(left, 2 * 48_000, 3 * 48_000);
    })();
    expect(noDuck - withDuck).toBeGreaterThan(5);
    expect(noDuck - withDuck).toBeLessThan(7);
    // The horn alone is the same level whether or not the aircraft is making noise.
    const horn = level(WARNING.STALL, 0.5, 1.5, 'tone');
    const hornOverEngine = (() => {
      const dsp = new FlightDsp(profile);
      dsp.params[P.COCKPIT] = 0;
      dsp.params[P.AIRCRAFT_BUS] = 0;
      const set = (): void => {
        engineOn(1)(dsp.params);
        dsp.params[P.AIRCRAFT_BUS] = 0;
        dsp.params[P.WARNINGS] = WARNING.STALL;
      };
      set();
      const { left } = render(dsp, 1.5, set);
      return levelDb(left, Math.round(0.5 * 48_000), Math.round(1.5 * 48_000));
    })();
    expect(Math.abs(horn - hornOverEngine)).toBeLessThan(0.5);
  });

  it('a caution (gear chime) does not duck', () => {
    const dsp = new FlightDsp(profile);
    const set = (): void => {
      engineOn(1)(dsp.params);
      dsp.params[P.WARNINGS] = WARNING.GEAR;
    };
    set();
    render(dsp, 1, set);
    expect(dsp.duckDb).toBeCloseTo(0, 6);
  });
});

describe('the mix and the limiter', () => {
  it('the cockpit hears the engine through the canopy, much quieter and without the highs', () => {
    const at = (cockpit: number) => {
      const dsp = new FlightDsp(profile);
      const set = (): void => {
        engineOn(1)(dsp.params);
        dsp.params[P.COCKPIT] = cockpit;
      };
      set();
      const { left } = render(dsp, 3, set);
      return left.subarray(left.length - 48_000);
    };
    const inside = at(1);
    const outside = at(0);
    expect(levelDb(outside) - levelDb(inside)).toBeGreaterThan(8);
  });

  it('the chase view is lowered by the profile offset, and the cockpit view is not', () => {
    const at = (cockpit: number, chaseOffsetDb: number): number => {
      const dsp = new FlightDsp({ ...profile, mix: { ...profile.mix, chaseOffsetDb } });
      const set = (): void => {
        engineOn(0.8)(dsp.params);
        dsp.params[P.COCKPIT] = cockpit;
      };
      set();
      const { left } = render(dsp, 3, set);
      return levelDb(left.subarray(left.length - 48_000));
    };
    expect(at(0, -12) - at(0, 0)).toBeCloseTo(-12, 0);
    expect(at(1, -12)).toBeCloseTo(at(1, 0), 6);
  });

  it('the master and bus gains scale the output', () => {
    const at = (master: number) =>
      levelDb(
        held((p) => {
          engineOn(1)(p);
          p[P.MASTER] = master;
        }, 2).out,
      );
    expect(at(0.5) - at(1)).toBeCloseTo(-6.02, 0);
    expect(at(0)).toBeLessThan(-150);
  });

  it('never exceeds the ceiling, however loud the state', () => {
    const dsp = new FlightDsp(shipped);
    const set = (): void => {
      engineOn(1, 1)(dsp.params);
      dsp.params[P.QBAR] = 40_000;
      dsp.params[P.WARNINGS] = WARNING.PULLUP;
      dsp.params[P.COCKPIT] = 0;
      dsp.params[P.MASTER] = 1;
    };
    set();
    const { left } = render(dsp, 2, set);
    let peak = 0;
    for (const v of left) peak = Math.max(peak, Math.abs(v));
    expect(peak).toBeLessThanOrEqual(0.85 + 1e-6);
  });

  it('softLimit is transparent when quiet, and saturates at the ceiling', () => {
    expect(softLimit(0.1)).toBeCloseTo(0.1, 3);
    expect(softLimit(-0.1)).toBeCloseTo(-0.1, 3);
    expect(softLimit(100)).toBeLessThanOrEqual(0.85);
    expect(softLimit(100)).toBeGreaterThan(0.84);
    expect(softLimit(-100)).toBeCloseTo(-softLimit(100), 9);
  });

  it('is the same every time (no random seed from the clock)', () => {
    const run = (): Float32Array => {
      const dsp = new FlightDsp(profile);
      const set = (): void => {
        engineOn(0.7)(dsp.params);
      };
      set();
      return render(dsp, 0.5, set).left;
    };
    expect([...run()]).toEqual([...run()]);
  });

  it('resets to silence at idle', () => {
    const dsp = new FlightDsp(profile);
    engineOn(1, 1)(dsp.params);
    render(dsp, 1, () => {
      engineOn(1, 1)(dsp.params);
    });
    dsp.reset();
    expect(dsp.rpm).toBe(2000);
    expect(dsp.params[P.MASTER]).toBe(1);
    expect(dsp.tone).toBe(TONE.NONE);
  });
});

describe('the cockpit mix as shipped follows the throttle', () => {
  // The interior hum once sat at -34 dB, which the loudness offset lifted until it drowned the engine:
  // the cockpit read about -20 dB at every throttle setting. This keeps the engine audible there.
  const cockpitLevel = (core: number, running: boolean): number => {
    const dsp = new FlightDsp(shipped);
    const set = (): void => {
      dsp.params[P.ENGINE_BITS] = running ? 1 : 0;
      dsp.params[P.CORE] = core;
      dsp.params[P.ON_GROUND] = 1;
      dsp.params[P.GROUND_SPEED] = core * 60;
      dsp.params[P.COCKPIT] = 1;
    };
    set();
    const { left } = render(dsp, 4, set);
    return levelDb(left.subarray(left.length - 48_000));
  };

  it('is much louder at military power than at idle, and idle louder than a stopped engine', () => {
    const stopped = cockpitLevel(0, false);
    const idle = cockpitLevel(0.06, true);
    const military = cockpitLevel(1, true);
    expect(military - idle).toBeGreaterThan(8);
    expect(idle - stopped).toBeGreaterThan(3);
  });
});

describe('A6: the cost of a render quantum', () => {
  it('stays under 0.21 ms (8 % of a core) for 128 frames, in the loudest state', () => {
    const dsp = new FlightDsp(profile);
    engineOn(1, 1)(dsp.params);
    dsp.params[P.QBAR] = 20_000;
    dsp.params[P.WARNINGS] = WARNING.STALL;
    dsp.params[P.GROUND_SPEED] = 60;
    dsp.params[P.ON_GROUND] = 1;
    // And the duel at its busiest: both guns and the growl.
    dsp.params[P.GUN_FIRING] = 1;
    dsp.params[P.ENEMY_GUN_DIST_M] = 300;
    dsp.params[P.SEEKER] = SEEKER_MODE.TONE;
    dsp.params[P.SIGNAL] = 2;
    const l = new Float32Array(128);
    const r = new Float32Array(128);
    for (let i = 0; i < 4000; i++) dsp.process(l, r, 128);
    const quanta = 20_000;
    const t0 = performance.now();
    for (let i = 0; i < quanta; i++) dsp.process(l, r, 128);
    const perQuantum = (performance.now() - t0) / quanta;
    console.info(`A6: ${(perQuantum * 1000).toFixed(1)} us per 128-frame quantum`);
    // Informational on slow machines (CI): a generous bound there, the gate on the reference machine.
    expect(perQuantum).toBeLessThan(process.env['CI'] === undefined ? 0.21 : 0.6);
  });
});

describe('A7: no allocation in process()', () => {
  it('makes no garbage but a boxed number or two a block, in states that exercise every voice', () => {
    // Measured in plain Node, by the sampling heap profiler (see the script): the test runner's module
    // transform stops V8 inlining across modules, and the numbers returned from the calls it does not
    // inline are boxed on the heap, which the shipped worklet (one bundle) does not do.
    const script = fileURLToPath(new URL('../test-support/process-garbage.ts', import.meta.url));
    const out = execFileSync(process.execPath, [script], { encoding: 'utf8', timeout: 110_000 });
    const result = JSON.parse(out.trim().split('\n').pop() ?? '{}') as {
      bytesPerCall: number;
      sites: { site: string; bytesPerCall: number }[];
    };
    // Two boxed doubles a block (32 bytes) are what the block-rate arithmetic leaves; an array, an
    // object or a closure made per block adds a multiple of that.
    expect(result.bytesPerCall, JSON.stringify(result.sites)).toBeLessThan(64);
  }, 120_000);
});

describe('block sizes', () => {
  it('renders the same signal in any block size, up to MAX_BLOCK', () => {
    const run = (block: number): Float32Array => {
      const dsp = new FlightDsp(profile);
      engineOn(0.8)(dsp.params);
      const out = new Float32Array(4096);
      const l = new Float32Array(MAX_BLOCK);
      const r = new Float32Array(MAX_BLOCK);
      for (let at = 0; at < out.length; at += block) {
        dsp.process(l, r, block);
        out.set(l.subarray(0, block), at);
      }
      return out;
    };
    const a = run(128);
    const b = run(128);
    expect([...a]).toEqual([...b]);
    // Spool smoothing is per block, so other sizes agree to the ear but not to the bit.
    expect(Math.abs(levelDb(run(64)) - levelDb(a))).toBeLessThan(0.5);
    expect(Math.abs(levelDb(run(256)) - levelDb(a))).toBeLessThan(0.5);
  });

  it('clamps an oversized request to MAX_BLOCK instead of writing past its scratch', () => {
    const dsp = new FlightDsp(profile);
    const l = new Float32Array(MAX_BLOCK * 2).fill(7);
    const r = new Float32Array(MAX_BLOCK * 2).fill(7);
    dsp.process(l, r, MAX_BLOCK * 2);
    expect(l[MAX_BLOCK]).toBe(7);
  });
});
