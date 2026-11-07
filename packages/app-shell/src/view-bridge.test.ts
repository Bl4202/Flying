import { describe, expect, it } from 'vitest';
import { VI, ViewInput } from '@flying/input';
import { newViewState, VIEW, ViewReader, type ViewState } from '@flying/sim-bridge/view-channel';
import { makeViewBuffer, ViewBridge } from './view-bridge.ts';

function rig(shared: boolean) {
  const input = new ViewInput();
  const buffer = makeViewBuffer(shared);
  const sent: ViewState[] = [];
  const cameras: string[] = [];
  const frames: (() => void)[] = [];
  let cancelled = 0;
  const bridge = new ViewBridge({
    input,
    buffer,
    send: (s) => sent.push({ ...s }),
    requestFrame: (cb) => frames.push(cb),
    cancelFrame: () => {
      cancelled += 1;
    },
    onCamera: (c) => cameras.push(c),
  });
  const frame = (): void => {
    frames.shift()?.();
  };
  const read = (): ViewState => {
    const out = newViewState();
    if (buffer === null) throw new Error('not shared');
    new ViewReader(buffer).read(out);
    return out;
  };
  return { input, buffer, bridge, sent, cameras, frame, read, cancelled: () => cancelled };
}

describe('makeViewBuffer', () => {
  it("is a shared record of the channel's size, or nothing without shared memory", () => {
    expect(makeViewBuffer(true)?.byteLength).toBe(VIEW.SIZE);
    expect(makeViewBuffer(false)).toBeNull();
  });
});

describe('ViewBridge with a shared channel', () => {
  it('writes the state at once and on every change, and the renderer reads it', () => {
    const r = rig(true);
    r.bridge.configure({ fovDeg: 100, shakePercent: 50, hudBrightness: 70, reducedMotion: true });
    const state = r.read();
    expect(state.fovDeg).toBe(100);
    expect(state.shake).toBeCloseTo(0.5, 6);
    expect(state.hudBrightness).toBe(70);
    expect(state.reducedMotion).toBe(true);
    expect(r.sent).toEqual([]); // no messages: the record carries it
  });

  it('follows the camera key: each press toggles, and it says so', () => {
    const r = rig(true);
    r.bridge.start();
    expect(r.bridge.camera).toBe('cockpit');
    r.input.v[VI.CAMERA_PRESSES] = 1;
    r.frame();
    expect(r.bridge.camera).toBe('chase');
    expect(r.read().camera).toBe('chase');
    expect(r.cameras).toEqual(['chase']);
    r.input.v[VI.CAMERA_PRESSES] = 2;
    r.frame();
    expect(r.cameras).toEqual(['chase', 'cockpit']);
  });

  it('carries look, zoom and the HUD keys', () => {
    const r = rig(true);
    r.bridge.start();
    r.input.v[VI.LOOK_YAW_RATE] = 1.2;
    r.input.v[VI.ZOOM] = 1;
    r.input.v[VI.DECLUTTER_PRESSES] = 1;
    r.input.v[VI.BRIGHTNESS_STEPS] = -2;
    r.frame();
    const s = r.read();
    expect(s.lookYawRate).toBeCloseTo(1.2, 5);
    expect(s.zoom).toBe(true);
    expect(s.hudDeclutter).toBe(1);
    expect(s.hudBrightness).toBe(80);
  });

  it('sets the camera and the declutter level from the page, clamped', () => {
    const r = rig(true);
    r.bridge.setCamera('chase');
    expect(r.read().camera).toBe('chase');
    expect(r.cameras).toEqual(['chase']);
    r.bridge.setDeclutter(2);
    expect(r.read().hudDeclutter).toBe(2);
    r.bridge.setDeclutter(9);
    expect(r.read().hudDeclutter).toBe(2);
    r.bridge.setDeclutter(-3);
    expect(r.read().hudDeclutter).toBe(0);
  });

  it('stops asking for frames when stopped', () => {
    const r = rig(true);
    r.bridge.start();
    r.bridge.start(); // twice is once
    r.bridge.stop();
    expect(r.cancelled()).toBe(1);
    r.frame();
    r.frame();
    expect(r.cameras).toEqual([]);
  });
});

describe('ViewBridge without a shared channel', () => {
  it('sends the state when it changes, and not when it does not', () => {
    const r = rig(false);
    r.bridge.start();
    expect(r.sent).toHaveLength(1); // the first state
    r.frame();
    r.frame();
    expect(r.sent).toHaveLength(1);
    r.input.v[VI.ZOOM] = 1;
    r.frame();
    expect(r.sent).toHaveLength(2);
    expect(r.sent[1]?.zoom).toBe(true);
    r.input.v[VI.CAMERA_PRESSES] = 1;
    r.frame();
    expect(r.sent).toHaveLength(3);
    expect(r.sent[2]?.camera).toBe('chase');
  });

  it('sends a configuration change at once', () => {
    const r = rig(false);
    r.bridge.configure({ fovDeg: 90 });
    expect(r.sent.at(-1)?.fovDeg).toBe(90);
  });
});
