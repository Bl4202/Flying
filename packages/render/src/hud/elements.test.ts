import { describe, expect, it } from 'vitest';
import { OWNSHIP_FLAG2 } from '@flying/sim-bridge/layout';
import { WARNING } from '@flying/sim-bridge';
import { bounds, DEG, draw, kestrel, type Segment } from './test-support.ts';
import { HS } from './state.ts';

/** Segments inside a rectangle (all of both ends). */
const within = (s: readonly Segment[], x0: number, y0: number, x1: number, y1: number): Segment[] =>
  s.filter(
    (g) =>
      [g[0], g[2]].every((x) => x >= x0 && x <= x1) &&
      [g[1], g[3]].every((y) => y >= y0 && y <= y1),
  );

describe('the layout the Kestrel carries (GDD 54.13)', () => {
  const hud = kestrel().hud;
  it('has the combiner of 28 by 20 degrees and an eye box of 120 by 80 by 160 mm', () => {
    expect(hud.tfovDeg).toEqual([28, 20]);
    expect(hud.eyeBoxMm).toEqual([120, 80, 160]);
    expect(hud.textMrad).toBe(12);
  });
  it('lists thirteen layers, each type once', () => {
    expect(hud.layers).toHaveLength(13);
    expect(new Set(hud.layers.map((l) => l.type)).size).toBe(13);
  });
});

describe('the boresight and the flight-path marker', () => {
  it('draws the boresight as a cross with a gap, 10 mrad each way', () => {
    const { segments } = draw('boresight');
    expect(segments).toHaveLength(4);
    const b = bounds(segments);
    expect([b.x0, b.x1, b.y0, b.y1]).toEqual([-10, 10, -10, 10]);
    // The middle is empty: no segment crosses the centre.
    for (const s of segments)
      expect(Math.hypot((s[0] + s[2]) / 2, (s[1] + s[3]) / 2)).toBeGreaterThan(5);
  });

  it('puts the flight-path marker where the velocity points: below the nose by the angle of attack', () => {
    // Nose 8 degrees up, flying level: the marker is 8 degrees below the boresight.
    const { segments, state } = draw('flight_path_marker', { pitch: 8 * DEG, vel: [0, 130, 0] });
    expect(state[HS.FPM_V]).toBeCloseTo(-Math.tan(8 * DEG) * 1000, 3);
    const b = bounds(segments);
    const centre = (b.y0 + b.y1) / 2;
    // The circle is centred on the marker, with a fin above it: its middle is a little above.
    expect(centre).toBeGreaterThan(-Math.tan(8 * DEG) * 1000 - 5);
    expect(centre).toBeLessThan(-Math.tan(8 * DEG) * 1000 + 15);
  });

  it('draws a circle, two wings and a fin', () => {
    const { segments } = draw('flight_path_marker');
    // 12 circle edges, 2 wings, 1 fin.
    expect(segments).toHaveLength(15);
  });

  it('cages below 20 m/s: a dashed circle at the boresight', () => {
    const fast = draw('flight_path_marker', { vel: [0, 130, 0] });
    const slow = draw('flight_path_marker', { vel: [0, 12, 0], pitch: 10 * DEG });
    expect(slow.state[HS.FPM_CAGED]).toBe(1);
    const b = bounds(slow.segments);
    expect(Math.abs((b.x0 + b.x1) / 2)).toBeLessThan(1);
    expect(Math.abs((b.y0 + b.y1) / 2)).toBeLessThan(5);
    // Dashed: fewer circle pieces than the solid one.
    expect(slow.segments.length).toBeLessThan(fast.segments.length);
  });

  it('is not drawn when the velocity is outside the combiner or behind', () => {
    expect(draw('flight_path_marker', { vel: [0, -130, 0] }).segments).toEqual([]);
    // Flying 40 degrees off the nose: outside the field.
    expect(
      draw('flight_path_marker', { vel: [130 * Math.sin(40 * DEG), 130 * Math.cos(40 * DEG), 0] })
        .segments,
    ).toEqual([]);
  });
});

describe('the pitch ladder', () => {
  it('draws the horizon level with the boresight in level flight', () => {
    const { segments } = draw('pitch_ladder');
    // The horizon is the longest: 2.5 times the half-width on each side of a 10 mrad gap.
    const onHorizon = segments.filter((s) => Math.abs(s[1]) < 1 && Math.abs(s[3]) < 1);
    expect(onHorizon).toHaveLength(2);
    expect(Math.min(...onHorizon.flatMap((s) => [s[0], s[2]]))).toBeCloseTo(-100, 0);
    expect(Math.max(...onHorizon.flatMap((s) => [s[0], s[2]]))).toBeCloseTo(100, 0);
  });

  it('draws the horizon below the boresight by the pitch: 6 degrees up puts it 105 mrad down', () => {
    const { segments } = draw('pitch_ladder', { pitch: 6 * DEG });
    const lowest = segments.filter((s) => Math.abs(s[1] + Math.tan(6 * DEG) * 1000) < 2);
    expect(lowest.length).toBeGreaterThanOrEqual(2);
  });

  it('steps every 5 degrees, with the 5 degree line 87 mrad above the horizon in level flight', () => {
    const { segments } = draw('pitch_ladder');
    const ys = new Set(segments.map((s) => Math.round(s[1] / 10)));
    expect(ys.has(Math.round((Math.tan(5 * DEG) * 1000) / 10))).toBe(true);
    // Lines at 5 and 10 degrees are both inside the 20 degree tall field (+-10 degrees plus margin).
    expect(ys.has(Math.round((Math.tan(10 * DEG) * 1000) / 10))).toBe(true);
  });

  it('dashes the lines below the horizon and leaves those above solid', () => {
    const { segments } = draw('pitch_ladder', { pitch: 3 * DEG });
    const below = segments.filter((s) => s[1] < -100 && s[3] < -100);
    const above = segments.filter((s) => s[1] > 10 && s[3] > 10);
    // Dashes are short; solid half-lines are long.
    const mean = (xs: Segment[]): number =>
      xs.reduce((n, s) => n + Math.hypot(s[2] - s[0], s[3] - s[1]), 0) / Math.max(xs.length, 1);
    expect(below.length).toBeGreaterThan(0);
    expect(mean(below)).toBeLessThan(mean(above));
  });

  it('labels the lines every 10 degrees', () => {
    const full = draw('pitch_ladder', { pitch: 5 * DEG });
    const none = draw('pitch_ladder', { pitch: 5 * DEG }, 1);
    expect(full.segments.length).toBeGreaterThan(none.segments.length);
  });

  it('thins out to its labelled lines when steeper than 60 degrees', () => {
    const shallow = draw('pitch_ladder', { pitch: 40 * DEG });
    const steep = draw('pitch_ladder', { pitch: 65 * DEG });
    const lineCount = (segs: Segment[]): number =>
      segs.filter((s) => Math.hypot(s[2] - s[0], s[3] - s[1]) > 15).length;
    expect(lineCount(steep.segments)).toBeLessThan(lineCount(shallow.segments) + 1);
  });

  it('rolls with the aircraft: banked 30 degrees right, the horizon tilts 30 degrees', () => {
    const { segments } = draw('pitch_ladder', { bank: 30 * DEG });
    // The horizon passes through the middle of the field; find its two outermost endpoints.
    const near = segments.filter(
      (s) => Math.abs((s[1] + s[3]) / 2) < 70 && Math.hypot(s[2] - s[0], s[3] - s[1]) > 30,
    );
    expect(near.length).toBeGreaterThan(0);
    const left = near.reduce((a, s) => (s[0] < a[0] ? s : a));
    const slope = Math.atan2(left[3] - left[1], left[2] - left[0]) / DEG;
    // Right wing down: the horizon rises to the right, by the bank.
    expect(Math.abs(slope - 30)).toBeLessThan(2);
  });
});

describe('the heading tape', () => {
  it('scrolls under a fixed pointer: the current heading is at the middle', () => {
    const a = draw('heading_tape', { heading: 90 * DEG });
    const b = draw('heading_tape', { heading: 95 * DEG });
    const majorAt = (segs: Segment[], x: number): Segment | undefined =>
      segs.find(
        (s) => Math.abs(s[0] - x) < 1e-3 && Math.abs(s[0] - s[2]) < 1e-6 && s[3] - s[1] > 7,
      );
    // At 90 the 90 degree tick is at the centre; at 95 it is 5 degrees to the left.
    expect(majorAt(a.segments, 0)).toBeDefined();
    const moved = b.segments.find(
      (s) => Math.abs(s[0] - -5 * (200 / 60)) < 1e-6 && s[3] - s[1] > 7,
    );
    expect(moved).toBeDefined();
  });

  it('draws ticks every 5 degrees across a 60 degree window of 200 mrad', () => {
    const { segments } = draw('heading_tape', { heading: 0 });
    const ticks = segments.filter(
      (s) => Math.abs(s[0] - s[2]) < 1e-6 && s[1] === 150 && s[3] > 150 && s[3] - s[1] <= 9,
    );
    // -30 to +30 in steps of 5 is 13 ticks.
    expect(ticks).toHaveLength(13);
  });

  it('wraps through north: ticks past 360 continue from 0', () => {
    const { segments } = draw('heading_tape', { heading: 355 * DEG });
    expect(segments.length).toBeGreaterThan(30);
    const b = bounds(segments);
    expect(b.x0).toBeGreaterThan(-110);
    expect(b.x1).toBeLessThan(110);
  });
});

describe('the speed tape', () => {
  const cx = -175;
  it('scrolls: calibrated airspeed at the middle, higher speeds above', () => {
    const { segments } = draw('speed_tape', { status: { cas: 129.8 } }); // 252.3 kn
    const kn = 129.8 * 1.943844492;
    const scale = 120 / 100;
    // The 250 kn tick is 2.3 kn below the pointer.
    const tick250 = segments.find(
      (s) =>
        Math.abs(s[0] - cx) < 1e-6 && Math.abs(s[1] - (0 + (250 - kn) * scale)) < 1e-3 && s[2] > cx,
    );
    expect(tick250).toBeDefined();
    // A tick for a higher speed is higher (more positive y).
    const tick260 = segments.find(
      (s) => Math.abs(s[1] - (260 - kn) * scale) < 1e-3 && Math.abs(s[0] - cx) < 1e-6,
    );
    expect(tick260).toBeDefined();
  });

  it('keeps the readout in a box on the outside of the scale', () => {
    const { segments } = draw('speed_tape', { status: { cas: 100 } });
    const box = within(segments, -215, -10, -170, 10).filter(
      (s) => Math.abs(s[1]) < 10 && Math.abs(s[3]) < 10,
    );
    expect(box.length).toBeGreaterThan(8);
  });

  it('shows Mach from 0.6 up, and not below', () => {
    const below = draw('speed_tape', { status: { cas: 150, mach: 0.59 } });
    const above = draw('speed_tape', { status: { cas: 150, mach: 0.6 } });
    expect(above.segments.length).toBeGreaterThan(below.segments.length + 10);
  });

  it('is only the readout at minimum declutter', () => {
    const full = draw('speed_tape', { status: { cas: 150 } }, 0);
    const minimum = draw('speed_tape', { status: { cas: 150 } }, 2);
    expect(minimum.segments.length).toBeLessThan(full.segments.length / 2);
  });
});

describe('the altitude tape and the radar altitude', () => {
  it('scrolls with 100 ft ticks over a 1 000 ft window', () => {
    const { segments } = draw('altitude_tape', {
      status: { altitude: 3000 / 3.280839895, agl: 5000 },
    });
    const ticks = segments.filter((s) => Math.abs(s[0] - 175) < 1e-6 && Math.abs(s[2] - 175) > 5);
    // -500 to +500 ft around the pointer in 100 ft steps: 11 ticks.
    expect(ticks).toHaveLength(11);
  });

  it('shows the radar altitude below 1 500 ft above the ground, rounded to ten feet', () => {
    const low = draw('altitude_tape', {
      status: { altitude: 300, agl: (150 / 3.280839895) * 3.280839895 * 0.3048 },
    });
    const high = draw('altitude_tape', { status: { altitude: 300, agl: 2000 * 0.3048 } });
    expect(low.segments.length).toBeGreaterThan(high.segments.length);
  });

  it('has no radar altitude on the ground', () => {
    const ground = draw('altitude_tape', {
      status: { altitude: 40, agl: 1.5, flags2: OWNSHIP_FLAG2.WOW },
    });
    const air = draw('altitude_tape', { status: { altitude: 40, agl: 1.5 } });
    expect(air.segments.length).toBeGreaterThan(ground.segments.length);
  });
});

describe('the vertical speed caret', () => {
  it('rides a scale of +-6 000 ft a minute over 100 mrad and stops at the ends', () => {
    const caretY = (fpm: number): number => {
      const { segments } = draw('vs_caret', { status: { vs: fpm / 196.8503937 } });
      // The caret's tip is the point shared by two segments at x = 215.
      const tips = segments.filter((s) => s[0] === 215 && s[2] === 222);
      return tips[0]?.[1] ?? Number.NaN;
    };
    expect(caretY(0)).toBeCloseTo(0, 6);
    expect(caretY(3000)).toBeCloseTo(25, 3);
    expect(caretY(-6000)).toBeCloseTo(-50, 3);
    // Clamped.
    expect(caretY(20_000)).toBeCloseTo(50, 6);
    expect(caretY(-20_000)).toBeCloseTo(-50, 6);
  });
});

describe('the g meter', () => {
  // The G and its number carry the tint (the highest-load line below stays plain).
  const tint = (nz: number, t = 0): number => {
    const { segments } = draw('g_meter', { status: { nz, nMax: 8 }, timeS: t });
    return segments[0]?.[6] ?? Number.NaN;
  };
  it('is plain below 85 per cent of the limit, amber from there, red from 100', () => {
    expect(tint(6.7)).toBe(0); // 83.75 %
    expect(tint(6.8)).toBe(1); // 85 %
    expect(tint(7.9)).toBe(1);
    expect(tint(8)).toBe(2); // 100 %
    expect(tint(-8.1)).toBe(2); // negative g counts by its size
  });

  it('is also boxed and flashing, so colour is not the only cue', () => {
    // The same number against three limits: 35, 87.5 and 116 per cent.
    const plain = draw('g_meter', { status: { nz: 7, nMax: 20 } });
    const amber = draw('g_meter', { status: { nz: 7, nMax: 8 } });
    const red = draw('g_meter', { status: { nz: 7, nMax: 6 } });
    expect(amber.segments.length).toBe(plain.segments.length + 4);
    expect(red.segments.length).toBe(plain.segments.length + 8);
    // Amber flashes at 2 Hz: bright, then dim, within a second.
    const alphas = [0, 0.13, 0.26, 0.4].map(
      (t) => draw('g_meter', { status: { nz: 7, nMax: 8 }, timeS: t }).segments[0]?.[5],
    );
    expect(new Set(alphas).size).toBeGreaterThan(1);
  });

  it('shows the highest load of the flight, and drops it at minimum declutter', () => {
    const full = draw('g_meter', { status: { nz: 2, nMax: 8, maxNz: 4.2 } }, 0);
    const minimum = draw('g_meter', { status: { nz: 2, nMax: 8, maxNz: 4.2 } }, 2);
    expect(full.segments.length).toBeGreaterThan(minimum.segments.length);
  });
});

describe('the angle-of-attack bracket', () => {
  it('shows only with the gear down', () => {
    expect(draw('aoa_bracket', { status: { gearPos: 0 } }).segments).toEqual([]);
    expect(draw('aoa_bracket', { status: { gearPos: 1 } }).segments.length).toBeGreaterThan(5);
  });

  it('puts the caret at the centre on speed (12.8 degrees) and at the ends 3 degrees off', () => {
    const caretY = (alphaDeg: number): number => {
      const { segments } = draw('aoa_bracket', { status: { gearPos: 1, alpha: alphaDeg * DEG } });
      // The caret's tip is where two segments start at x = -112.
      return segments.find((s) => s[0] === -112 && s[2] === -120)?.[1] ?? Number.NaN;
    };
    expect(caretY(12.8)).toBeCloseTo(0, 5);
    expect(caretY(15.8)).toBeCloseTo(30, 5);
    expect(caretY(9.8)).toBeCloseTo(-30, 5);
    expect(caretY(30)).toBeCloseTo(30, 5); // clamped
  });
});

describe('the bank scale', () => {
  it('draws ticks every 10 degrees over +-60 degrees at 120 mrad, longer every 30', () => {
    const { segments } = draw('bank_scale');
    // The ticks come first, then the three lines of the sky pointer.
    const ticks = segments.slice(0, 13);
    for (const t of ticks) expect(Math.hypot(t[0], t[1])).toBeCloseTo(120, 3);
    expect(segments).toHaveLength(16);
    const long = ticks.filter((s) => Math.hypot(s[2], s[3]) > 129);
    expect(long).toHaveLength(5); // -60, -30, 0, 30, 60
  });

  it('moves the sky pointer opposite to the bank: right wing down, pointer to the left', () => {
    const level = draw('bank_scale', { bank: 0 });
    const right = draw('bank_scale', { bank: 30 * DEG });
    const tip = (segs: Segment[]): Segment => segs[segs.length - 3] as Segment;
    expect(tip(level.segments)[0]).toBeCloseTo(0, 6);
    expect(tip(right.segments)[0]).toBeCloseTo(-120 * Math.sin(30 * DEG), 4);
    expect(tip(right.segments)[1]).toBeCloseTo(120 * Math.cos(30 * DEG), 4);
  });
});

describe('the engine and fuel block, the mode line and the warnings', () => {
  it('lists throttle, fuel, flow and endurance, and the afterburner when it is lit', () => {
    const base = draw('engine_fuel_block', {
      status: { throttle: 0.8, fuel: 1800, fuelFlow: 0.5, abLevel: 0 },
    });
    const lit = draw('engine_fuel_block', {
      status: { throttle: 1, fuel: 1800, fuelFlow: 2, abLevel: 1 },
    });
    expect(lit.segments.length).toBeGreaterThan(base.segments.length);
  });

  it('names the assist level and what is out: gear, flaps, brakes, parking brake', () => {
    const bare = draw('mode_line', { status: { assist: 2 } });
    const all = draw('mode_line', {
      status: {
        assist: 2,
        gearPos: 1,
        flapDetent: 2,
        brakeL: 1,
        brakeR: 1,
        flags2: OWNSHIP_FLAG2.PARKING_BRAKE,
      },
    });
    expect(all.segments.length).toBeGreaterThan(bare.segments.length * 2);
    // Centred on the line.
    const b = bounds(all.segments);
    expect(Math.abs((b.x0 + b.x1) / 2)).toBeLessThan(10);
  });

  it('shows at most three warnings, the most urgent first, each boxed', () => {
    const none = draw('warnings', { warnings: 0 });
    expect(none.segments).toEqual([]);
    const four = draw('warnings', {
      warnings: WARNING.PULLUP | WARNING.STALL | WARNING.G | WARNING.GEAR,
    });
    // Three boxes of four lines each at least, and not four lines of text.
    const rows = new Set(four.segments.map((s) => Math.round(s[1] / 20)));
    expect(rows.size).toBeLessThanOrEqual(3 + 1);
    const lowestRow = Math.min(...four.segments.map((s) => s[1]));
    // The most urgent is at the top (y = 100): the fourth would be 3 rows down.
    expect(Math.max(...four.segments.map((s) => s[1]))).toBeGreaterThan(100);
    expect(lowestRow).toBeGreaterThan(100 - 3 * 20 - 10);
  });

  it('tints a warning red and a caution amber, and flashes the warning', () => {
    const w = draw('warnings', { warnings: WARNING.STALL });
    const c = draw('warnings', { warnings: WARNING.GEAR });
    expect(new Set(w.segments.map((s) => s[6]))).toEqual(new Set([2]));
    expect(new Set(c.segments.map((s) => s[6]))).toEqual(new Set([1]));
    const alphas = [0, 0.13, 0.26, 0.4].map(
      (t) => draw('warnings', { warnings: WARNING.STALL, timeS: t }).segments[0]?.[5],
    );
    expect(new Set(alphas).size).toBe(2);
    expect(
      new Set(
        [0, 0.13, 0.26, 0.4].map(
          (t) => draw('warnings', { warnings: WARNING.GEAR, timeS: t }).segments[0]?.[5],
        ),
      ),
    ).toEqual(new Set([1]));
  });
});

describe('declutter', () => {
  it('removes the bank scale, engine block and mode line at level 1, and the heading tape and vertical speed at 2', () => {
    for (const type of ['bank_scale', 'engine_fuel_block', 'mode_line']) {
      expect(draw(type, {}, 0).segments.length).toBeGreaterThan(0);
      expect(draw(type, {}, 1).segments).toEqual([]);
    }
    for (const type of ['vs_caret', 'heading_tape']) {
      expect(draw(type, {}, 1).segments.length).toBeGreaterThan(0);
      expect(draw(type, {}, 2).segments).toEqual([]);
    }
    // The boresight and the flight-path marker never go.
    expect(draw('boresight', {}, 2).segments.length).toBe(4);
  });
});
