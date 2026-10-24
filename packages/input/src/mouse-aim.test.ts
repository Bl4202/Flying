import { describe, expect, it } from 'vitest';
import { AIM_CONE_RAD, AIM_RAD_PER_PX, angleBetween, MouseAim, wrapPi } from './mouse-aim.ts';

const deg = (d: number): number => (d * Math.PI) / 180;
const toDeg = (r: number): number => (r * 180) / Math.PI;

describe('angles', () => {
  it('wraps to [-pi, pi]', () => {
    expect(wrapPi(0)).toBe(0);
    expect(wrapPi(deg(190))).toBeCloseTo(deg(-170), 12);
    expect(wrapPi(deg(-190))).toBeCloseTo(deg(170), 12);
    expect(wrapPi(deg(720 + 30))).toBeCloseTo(deg(30), 9);
  });

  it('measures the angle between two directions on the sphere', () => {
    expect(angleBetween(0, 0, 0, 0)).toBeCloseTo(0, 12);
    expect(angleBetween(0, 0, deg(90), 0)).toBeCloseTo(deg(90), 12);
    expect(angleBetween(0, deg(90), deg(123), 0)).toBeCloseTo(deg(90), 12);
    expect(angleBetween(deg(170), 0, deg(-170), 0)).toBeCloseTo(deg(20), 12); // across the seam
    expect(angleBetween(0, deg(45), 0, deg(-45))).toBeCloseTo(deg(90), 12);
  });
});

describe('MouseAim: starting and moving', () => {
  it('starts aiming where the nose points', () => {
    const a = new MouseAim();
    expect(a.active).toBe(false);
    a.enable(deg(30), deg(10));
    expect(a.active).toBe(true);
    expect([toDeg(a.az), toDeg(a.el)]).toEqual([expect.closeTo(30, 9), expect.closeTo(10, 9)]);
  });

  it('turns clockwise with mouse right and rises with mouse up', () => {
    const a = new MouseAim();
    a.enable(0, 0);
    a.move(100, 0, 1);
    expect(a.az).toBeCloseTo(100 * AIM_RAD_PER_PX, 12);
    a.move(0, -100, 1);
    expect(a.el).toBeCloseTo(100 * AIM_RAD_PER_PX, 12);
    a.move(0, 300, 1);
    expect(a.el).toBeLessThan(0);
  });

  it('scales with the sensitivity and does nothing while off', () => {
    const a = new MouseAim();
    a.move(100, 100, 1);
    expect([a.az, a.el]).toEqual([0, 0]);
    a.enable(0, 0);
    a.move(100, 0, 3);
    expect(a.az).toBeCloseTo(300 * AIM_RAD_PER_PX, 12);
    a.disable();
    a.move(100, 0, 3);
    expect(a.az).toBeCloseTo(300 * AIM_RAD_PER_PX, 12);
  });

  it('wraps the azimuth and stops the elevation at the poles', () => {
    const a = new MouseAim();
    a.enable(deg(179), 0);
    a.move(Math.round(deg(5) / AIM_RAD_PER_PX), 0, 1);
    expect(a.az).toBeLessThan(0);
    expect(Math.abs(a.az)).toBeGreaterThan(deg(170));
    a.move(0, -1e6, 1);
    expect(a.el).toBeCloseTo(Math.PI / 2, 12);
  });
});

describe('MouseAim: the cone around the nose', () => {
  it('leaves an aim inside the cone alone', () => {
    const a = new MouseAim();
    a.enable(0, 0);
    a.move(Math.round(deg(20) / AIM_RAD_PER_PX), 0, 1);
    const az = a.az;
    a.follow(0, 0, 16, 0);
    expect(a.az).toBe(az);
  });

  it('pulls an aim outside the cone back onto its edge, along the line to the nose', () => {
    const a = new MouseAim();
    a.enable(0, 0);
    a.move(Math.round(deg(80) / AIM_RAD_PER_PX), 0, 1);
    a.follow(0, 0, 16, 0);
    expect(angleBetween(0, 0, a.az, a.el)).toBeCloseTo(AIM_CONE_RAD, 9);
    expect(toDeg(a.az)).toBeCloseTo(35, 6);
    expect(Math.abs(a.el)).toBeLessThan(1e-9);
  });

  it('follows a nose that turns: the aim is re-clamped around the latest nose', () => {
    const a = new MouseAim();
    a.enable(0, 0);
    a.move(Math.round(deg(30) / AIM_RAD_PER_PX), 0, 1);
    a.follow(deg(60), 0, 16, 0); // the nose swung 60 deg right: the aim is now 30 deg behind it
    expect(angleBetween(deg(60), 0, a.az, a.el)).toBeCloseTo(deg(30), 3); // the move was rounded to pixels
    a.follow(deg(-20), 0, 16, 0); // and now it is 50 deg from the nose: clamped to 35
    expect(angleBetween(deg(-20), 0, a.az, a.el)).toBeCloseTo(AIM_CONE_RAD, 9);
  });

  it('clamps across the north-south seam and near the pole', () => {
    const a = new MouseAim();
    a.enable(deg(175), deg(80));
    a.follow(deg(-175), deg(10), 16, 0);
    expect(angleBetween(deg(-175), deg(10), a.az, a.el)).toBeCloseTo(AIM_CONE_RAD, 9);
  });

  it('drifts back towards the nose at the spring rate', () => {
    const a = new MouseAim();
    a.enable(0, 0);
    a.move(Math.round(deg(30) / AIM_RAD_PER_PX), 0, 1);
    const before = angleBetween(0, 0, a.az, a.el);
    a.follow(0, 0, 1000, 1); // one second at 1/s closes 1 - 1/e of the gap
    const after = angleBetween(0, 0, a.az, a.el);
    expect(after / before).toBeCloseTo(Math.exp(-1), 6);
    a.follow(0, 0, 60_000, 5);
    expect(angleBetween(0, 0, a.az, a.el)).toBeLessThan(1e-6);
  });

  it('does nothing when off', () => {
    const a = new MouseAim();
    a.follow(1, 1, 16, 1);
    expect([a.az, a.el]).toEqual([0, 0]);
  });
});

describe('MouseAim: the frame encoding', () => {
  it('sends azimuth / pi and elevation / (pi/2)', () => {
    const a = new MouseAim();
    a.enable(deg(90), deg(45));
    expect(a.cursorX).toBeCloseTo(0.5, 12);
    expect(a.cursorY).toBeCloseTo(0.5, 12);
    a.enable(deg(-180), deg(-90));
    expect(Math.abs(a.cursorX)).toBeCloseTo(1, 12);
    expect(a.cursorY).toBeCloseTo(-1, 12);
  });

  it('stays inside -1..1 for every aim', () => {
    const a = new MouseAim();
    for (let az = -180; az <= 180; az += 15) {
      for (let el = -90; el <= 90; el += 15) {
        a.enable(deg(az), deg(el));
        expect(Math.abs(a.cursorX)).toBeLessThanOrEqual(1 + 1e-12);
        expect(Math.abs(a.cursorY)).toBeLessThanOrEqual(1 + 1e-12);
      }
    }
  });
});
