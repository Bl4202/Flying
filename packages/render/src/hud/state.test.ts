import { describe, expect, it } from 'vitest';
import { WORLD, OWNSHIP_FLAG2 } from '@flying/sim-bridge/layout';
import { bytesPerCall } from '../test-support.ts';
import { CAGE_BELOW_MPS, HS, HudState } from './state.ts';
import { attitude, DEG, input } from './test-support.ts';

function state(s: Parameters<typeof input>[0] = {}): HudState {
  const h = new HudState();
  h.update(input(s));
  return h;
}

type V3 = readonly [number, number, number];

describe('attitude against the local horizon', () => {
  it('has pitch up positive, in degrees from the nose', () => {
    for (const p of [-60, -10, 0, 5, 30, 85]) {
      expect(state({ pitch: p * DEG }).at(HS.PITCH) / DEG).toBeCloseTo(p, 6);
    }
  });

  it('has bank positive with the right wing down', () => {
    for (const b of [-120, -45, 0, 10, 90, 170]) {
      expect(state({ bank: b * DEG }).at(HS.BANK) / DEG).toBeCloseTo(b, 6);
    }
  });

  it('has heading true, clockwise from north, in [0, 360)', () => {
    for (const h of [0, 45, 90, 180, 270, 359]) {
      expect(state({ heading: h * DEG }).at(HS.HEADING) / DEG).toBeCloseTo(h, 6);
    }
    // North is 0, not 360, and a heading just west of north is just under 360.
    expect(state({ heading: -1 * DEG }).at(HS.HEADING) / DEG).toBeCloseTo(359, 6);
  });

  it('keeps the heading it had when the nose points straight up or down', () => {
    const h = new HudState();
    h.update(input({ heading: 70 * DEG, pitch: 10 * DEG }));
    expect(h.at(HS.HEADING) / DEG).toBeCloseTo(70, 6);
    h.update(input({ heading: 70 * DEG, pitch: 90 * DEG }));
    expect(h.at(HS.HEADING) / DEG).toBeCloseTo(70, 6);
  });

  it('is the same wherever the aircraft is, once the local frame is followed: 400 km away', () => {
    // BF is a tangent plane at the anchor, so far from it local up leans away from BF up, and a
    // nose that is level against BF is not level against the local horizon.
    const near = state({ heading: 90 * DEG, pos: [0, 0, 500] });
    const far = state({ heading: 90 * DEG, pos: [400_000, 0, 500] });
    expect(near.at(HS.PITCH)).toBeCloseTo(0, 9);
    // 400 km east: up has tilted 400/6371 rad towards east; a nose still pointing east (BF) is that
    // far above the local horizontal.
    expect(far.at(HS.PITCH)).toBeCloseTo(400_000 / WORLD.EARTH_RADIUS_M, 3);
  });
});

describe('the local frame: up, east, north in body axes', () => {
  it('are orthonormal and right-handed (east x north = up) for any attitude and place', () => {
    for (const s of [
      { heading: 0, pitch: 0, bank: 0, pos: [0, 0, 100] as [number, number, number] },
      {
        heading: 2.1,
        pitch: 0.4,
        bank: -1.2,
        pos: [30_000, -80_000, 9000] as [number, number, number],
      },
      {
        heading: 5,
        pitch: -1,
        bank: 2.5,
        pos: [-500_000, 200_000, 30_000] as [number, number, number],
      },
    ]) {
      const v = state(s);
      const u: V3 = [v.at(HS.UP_X), v.at(HS.UP_Y), v.at(HS.UP_Z)];
      const e: V3 = [v.at(HS.EAST_X), v.at(HS.EAST_Y), v.at(HS.EAST_Z)];
      const n: V3 = [v.at(HS.NORTH_X), v.at(HS.NORTH_Y), v.at(HS.NORTH_Z)];
      for (const a of [u, e, n]) expect(Math.hypot(...a)).toBeCloseTo(1, 9);
      const dot = (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
      expect(dot(u, e)).toBeCloseTo(0, 9);
      expect(dot(u, n)).toBeCloseTo(0, 9);
      const cross: V3 = [
        e[1] * n[2] - e[2] * n[1],
        e[2] * n[0] - e[0] * n[2],
        e[0] * n[1] - e[1] * n[0],
      ];
      for (const k of [0, 1, 2] as const) expect(cross[k]).toBeCloseTo(u[k], 9);
    }
  });
});

describe('the velocity vector in the HUD plane', () => {
  it('sits on the boresight when the aircraft flies along its nose', () => {
    const v = state({ pitch: 8 * DEG, heading: 33 * DEG });
    expect(v.at(HS.FPM_U)).toBeCloseTo(0, 6);
    expect(v.at(HS.FPM_V)).toBeCloseTo(0, 6);
    expect(v.at(HS.FPM_AHEAD)).toBe(1);
    expect(v.at(HS.FPM_CAGED)).toBe(0);
  });

  it('shows the angle of attack: velocity below the nose is below the boresight', () => {
    // Nose 10 degrees up, flight path level (a 10 degree angle of attack), heading north.
    const v = state({ pitch: 10 * DEG, vel: [0, 130, 0] });
    expect(v.at(HS.FPM_U)).toBeCloseTo(0, 6);
    // tan(10 degrees) below, in mrad.
    expect(v.at(HS.FPM_V)).toBeCloseTo(-Math.tan(10 * DEG) * 1000, 3);
    expect(v.at(HS.GAMMA)).toBeCloseTo(0, 9);
  });

  it('shows the drift: velocity to the right of the nose is right of the boresight', () => {
    // Heading north, flying 5 degrees east of north.
    const v = state({ vel: [130 * Math.sin(5 * DEG), 130 * Math.cos(5 * DEG), 0] });
    expect(v.at(HS.FPM_U)).toBeCloseTo(Math.tan(5 * DEG) * 1000, 3);
    expect(v.at(HS.FPM_V)).toBeCloseTo(0, 6);
  });

  it('takes the flight path angle from the local horizon', () => {
    const v = state({ pitch: 0, vel: [0, 130 * Math.cos(6 * DEG), 130 * Math.sin(6 * DEG)] });
    expect(v.at(HS.GAMMA) / DEG).toBeCloseTo(6, 6);
  });

  it('is caged to the boresight below 20 m/s, where the direction means nothing', () => {
    expect(CAGE_BELOW_MPS).toBe(20);
    const v = state({ vel: [3, 4, 0] });
    expect(v.at(HS.FPM_CAGED)).toBe(1);
    expect([v.at(HS.FPM_U), v.at(HS.FPM_V)]).toEqual([0, 0]);
    const fast = state({ vel: [0, 20.5, 0] });
    expect(fast.at(HS.FPM_CAGED)).toBe(0);
  });

  it('is not drawn when the aircraft is moving backwards or sideways', () => {
    expect(state({ vel: [0, -130, 0] }).at(HS.FPM_AHEAD)).toBe(0);
    expect(state({ vel: [130, 0, 0] }).at(HS.FPM_AHEAD)).toBe(0);
  });
});

describe('the numbers, in the units the symbology uses', () => {
  it('converts metres per second to knots, metres to feet, and metres per second to feet a minute', () => {
    const v = state({
      status: {
        cas: 100,
        altitude: 1000,
        agl: 300,
        vs: -5,
        tas: 120,
        mach: 0.55,
        throttle: 0.85,
        alpha: 0.2,
      },
    });
    expect(v.at(HS.CAS_KN)).toBeCloseTo(194.3844, 3);
    expect(v.at(HS.ALT_FT)).toBeCloseTo(3280.84, 1);
    expect(v.at(HS.AGL_FT)).toBeCloseTo(984.25, 1);
    expect(v.at(HS.VS_FPM)).toBeCloseTo(-984.25, 1);
    expect(v.at(HS.MACH)).toBe(0.55);
    expect(v.at(HS.THROTTLE_PCT)).toBeCloseTo(85, 9);
    expect(v.at(HS.ALPHA_DEG)).toBeCloseTo(11.459, 2);
  });

  it('works out the endurance from the fuel and its flow, and shows none with no flow', () => {
    expect(state({ status: { fuel: 1800, fuelFlow: 0.5 } }).at(HS.ENDURANCE_MIN)).toBeCloseTo(
      60,
      9,
    );
    expect(state({ status: { fuel: 1800, fuelFlow: 0 } }).at(HS.ENDURANCE_MIN)).toBe(0);
    expect(state({ status: { fuelFlow: 0.5 } }).at(HS.FUEL_FLOW_KGPM)).toBeCloseTo(30, 9);
  });

  it('reads the configuration: gear, flaps, brakes, parking brake, weight on wheels, assist', () => {
    const v = state({
      status: {
        gearPos: 1,
        flapDetent: 2,
        brakeL: 0.5,
        brakeR: 1,
        flags2: OWNSHIP_FLAG2.PARKING_BRAKE | OWNSHIP_FLAG2.WOW,
        assist: 3,
      },
      warnings: 0b101,
      timeS: 12.5,
    });
    expect([
      v.at(HS.GEAR_DOWN),
      v.at(HS.FLAPS),
      v.at(HS.BRAKES),
      v.at(HS.PARKING),
      v.at(HS.ON_GROUND),
      v.at(HS.ASSIST),
    ]).toEqual([1, 2, 0.75, 1, 1, 3]);
    expect([v.at(HS.WARNINGS), v.at(HS.TIME_S)]).toEqual([5, 12.5]);
  });

  it('makes no garbage when it updates (U3)', async () => {
    const h = new HudState();
    const inp = input({ heading: 1, pitch: 0.2, bank: 0.3, pos: [1000, -5000, 800] });
    const bytes = await bytesPerCall(() => {
      h.update(inp);
    });
    // V8 boxes a fractional number that crosses a call it does not inline (16 bytes, gone at the
    // next young-generation collection). Measured on the development machine: about 176 B; an
    // array or an object made per update would add tens of bytes to that.
    expect(bytes).toBeLessThan(320);
  });

  it('has an attitude helper that is a unit quaternion', () => {
    const q = attitude(1, 0.3, -0.4);
    expect(Math.hypot(...q)).toBeCloseTo(1, 9);
  });
});
