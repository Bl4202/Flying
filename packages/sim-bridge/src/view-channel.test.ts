import { describe, expect, it } from 'vitest';
import {
  copyViewState,
  newViewState,
  sanitizeViewState,
  VIEW,
  ViewReader,
  ViewWriter,
  type ViewState,
} from './view-channel.ts';

function sample(): ViewState {
  return {
    camera: 'chase',
    lookYawRate: 1.25,
    lookPitchRate: -0.5,
    mouseYawTotal: 123.456789012345,
    mousePitchTotal: -7.25,
    recentreCount: 3,
    zoom: true,
    freeLook: true,
    reducedMotion: true,
    fovDeg: 90,
    shake: 0.5,
    chaseDistanceM: 80,
    hudDeclutter: 2,
    hudBrightness: 40,
  };
}

describe('the view channel layout', () => {
  it('is 64 bytes, with every field inside it and none overlapping', () => {
    expect(VIEW.SIZE).toBe(64);
    const fields: [string, number, number][] = [
      ['seq', VIEW.H_SEQ, 4],
      ['flags', VIEW.O_FLAGS, 4],
      ['declutter', VIEW.O_HUD_DECLUTTER, 1],
      ['brightness', VIEW.O_HUD_BRIGHTNESS, 1],
      ['look yaw', VIEW.O_LOOK_YAW_RATE, 4],
      ['look pitch', VIEW.O_LOOK_PITCH_RATE, 4],
      ['mouse yaw', VIEW.O_MOUSE_YAW, 8],
      ['mouse pitch', VIEW.O_MOUSE_PITCH, 8],
      ['recentre', VIEW.O_RECENTRE, 4],
      ['fov', VIEW.O_FOV_DEG, 4],
      ['shake', VIEW.O_SHAKE, 4],
      ['chase', VIEW.O_CHASE_DISTANCE, 4],
    ];
    const used = new Set<number>();
    for (const [name, at, size] of fields) {
      expect(at + size, name).toBeLessThanOrEqual(VIEW.SIZE);
      for (let b = at; b < at + size; b++) {
        expect(used.has(b), `${name} overlaps at byte ${String(b)}`).toBe(false);
        used.add(b);
      }
    }
    // The 8-byte totals are 8-aligned so a reader can load them whole.
    expect(VIEW.O_MOUSE_YAW % 8).toBe(0);
    expect(VIEW.O_MOUSE_PITCH % 8).toBe(0);
  });
});

describe('ViewWriter and ViewReader', () => {
  it('round-trips every field', () => {
    const buffer = new SharedArrayBuffer(VIEW.SIZE);
    new ViewWriter(buffer).write(sample());
    const out = newViewState();
    expect(new ViewReader(buffer).read(out)).toBe(true);
    const want = sample();
    expect(out.camera).toBe('chase');
    expect(out.mouseYawTotal).toBe(want.mouseYawTotal); // f64: exact
    expect(out.mousePitchTotal).toBe(want.mousePitchTotal);
    expect(out.recentreCount).toBe(3);
    expect([out.zoom, out.freeLook, out.reducedMotion]).toEqual([true, true, true]);
    expect([out.hudDeclutter, out.hudBrightness]).toEqual([2, 40]);
    for (const k of [
      'lookYawRate',
      'lookPitchRate',
      'fovDeg',
      'shake',
      'chaseDistanceM',
    ] as const) {
      expect(out[k], k).toBeCloseTo(want[k], 5);
    }
  });

  it('works on a plain ArrayBuffer too, and refuses a buffer of the wrong size', () => {
    const buffer = new ArrayBuffer(VIEW.SIZE);
    new ViewWriter(buffer).write(sample());
    expect(new ViewReader(buffer).read(newViewState())).toBe(true);
    expect(() => new ViewWriter(new ArrayBuffer(63))).toThrow(/SYS-040/);
    expect(() => new ViewReader(new ArrayBuffer(65))).toThrow(/SYS-040/);
  });

  it('reads nothing before anything was written', () => {
    const out = newViewState();
    expect(new ViewReader(new ArrayBuffer(VIEW.SIZE)).read(out)).toBe(false);
    expect(out).toEqual(newViewState());
  });

  it('does not read a record that is being written, and counts it', () => {
    const buffer = new ArrayBuffer(VIEW.SIZE);
    new ViewWriter(buffer).write(sample());
    // The writer was interrupted halfway: the sequence is odd.
    new Int32Array(buffer, 0, 1)[0] = 3;
    const reader = new ViewReader(buffer);
    const out = newViewState();
    expect(reader.read(out)).toBe(false);
    expect(reader.contended).toBe(1);
    expect(out).toEqual(newViewState());
  });

  it('keeps the sequence even after each write, and advancing', () => {
    const buffer = new ArrayBuffer(VIEW.SIZE);
    const writer = new ViewWriter(buffer);
    const seq = new Int32Array(buffer, 0, 1);
    writer.write(sample());
    expect(seq[0]).toBe(2);
    writer.write(sample());
    expect(seq[0]).toBe(4);
  });

  it('clamps whatever a damaged channel holds', () => {
    const buffer = new ArrayBuffer(VIEW.SIZE);
    const writer = new ViewWriter(buffer);
    writer.write({ ...sample(), fovDeg: 500, shake: 9, chaseDistanceM: 1, hudBrightness: 255 });
    const dv = new DataView(buffer);
    dv.setFloat32(VIEW.O_LOOK_YAW_RATE, Number.NaN, true);
    dv.setUint8(VIEW.O_HUD_DECLUTTER, 77);
    const out = newViewState();
    expect(new ViewReader(buffer).read(out)).toBe(true);
    expect(out.fovDeg).toBe(120);
    expect(out.shake).toBe(1);
    expect(out.chaseDistanceM).toBe(40);
    expect(out.hudBrightness).toBe(100);
    expect(out.hudDeclutter).toBe(2);
    expect(out.lookYawRate).toBe(0);
  });
});

describe('view state helpers', () => {
  it('start with the cockpit camera, a 75 degree field of view and the HUD full', () => {
    const s = newViewState();
    expect([s.camera, s.fovDeg, s.hudDeclutter, s.hudBrightness]).toEqual(['cockpit', 75, 0, 100]);
  });

  it('copy into an existing state', () => {
    const into = newViewState();
    copyViewState(into, sample());
    expect(into).toEqual(sample());
  });

  it('sanitise non-finite numbers to defaults', () => {
    const s = { ...sample(), fovDeg: Number.NaN, mouseYawTotal: Number.POSITIVE_INFINITY };
    sanitizeViewState(s);
    expect(s.fovDeg).toBe(75);
    expect(s.mouseYawTotal).toBe(0);
  });
});
