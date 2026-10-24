import { describe, expect, it } from 'vitest';
import {
  AXIS,
  BUTTON,
  decodeInputFrame,
  INPUT_FLAG,
  INPUT_FRAME,
  type InputFrame,
  type InputSink,
} from '@flying/sim-bridge';
import { type ActionId } from './actions.ts';
import { GamepadHub, PAD_AXIS } from './devices/gamepad.ts';
import { KeyboardState } from './keyboard.ts';
import { builtinProfile, type Binding, type ControlsProfile } from './profile.ts';
import { InputPump, type PumpObservation } from './pump.ts';
import { shapeStatic, STICK_SHAPE } from './shaping.ts';
import { FakeMouse, FakePads, fakePad, fakeStick } from './test-support.ts';
import { MIL_DETENT } from './throttle.ts';

const SHARED0 = 1_700_000_000_000;

interface RigOptions {
  minIntervalMs?: number;
  accept?: boolean;
  profile?: ControlsProfile;
  mouse?: boolean;
}

function rig(opts: RigOptions = {}) {
  const frames: InputFrame[] = [];
  const actions: ActionId[] = [];
  let accept = opts.accept ?? true;
  const sink: InputSink = {
    send: (bytes) => {
      if (!accept) return false;
      expect(bytes.byteLength).toBe(INPUT_FRAME.SIZE);
      frames.push(decodeInputFrame(bytes));
      return true;
    },
    close: () => undefined,
  };
  const keyboard = new KeyboardState();
  const pads = new FakePads();
  const hub = new GamepadHub(pads.get);
  const mouse = opts.mouse === true ? new FakeMouse() : undefined;
  let mono = 1000;
  const pump = new InputPump({
    sink,
    keyboard,
    pads: hub,
    ...(mouse === undefined ? {} : { mouse }),
    ...(opts.profile === undefined ? {} : { profile: opts.profile }),
    nowMs: () => mono,
    sharedNowMs: () => SHARED0 + mono,
    ...(opts.minIntervalMs === undefined ? {} : { minIntervalMs: opts.minIntervalMs }),
    onAction: (id) => actions.push(id),
  });
  return {
    pump,
    keyboard,
    pads,
    hub,
    mouse,
    frames,
    actions,
    last: (): InputFrame => {
      const f = frames.at(-1);
      if (f === undefined) throw new Error('no frame yet');
      return f;
    },
    advance: (ms: number) => {
      mono += ms;
    },
    now: () => mono,
    setAccept: (a: boolean) => {
      accept = a;
    },
    /** Updates every `step` ms for `ms` total. */
    run: (ms: number, step = 4) => {
      for (let t = 0; t < ms; t += step) {
        mono += step;
        pump.update();
      }
    },
    press: (code: string) => {
      keyboard.keyDown(code, false);
    },
    release: (code: string) => {
      keyboard.keyUp(code);
    },
  };
}

type Rig = ReturnType<typeof rig>;
const bit = (b: number): bigint => 1n << BigInt(b);
const padRig = (opts: RigOptions = {}): Rig =>
  rig({ ...opts, profile: builtinProfile('gamepad-standard') });

describe('InputPump: when frames are written', () => {
  it('writes the first frame at once', () => {
    const r = rig();
    expect(r.pump.update()).toBe(true);
    expect(r.frames).toHaveLength(1);
    expect(r.frames[0]?.seq).toBe(1);
  });

  it('writes nothing while nothing changes and less than the interval has passed', () => {
    const r = rig();
    r.pump.update();
    r.advance(4);
    expect(r.pump.update()).toBe(false);
    r.advance(3);
    expect(r.pump.update()).toBe(false);
    expect(r.frames).toHaveLength(1);
  });

  it('writes a heartbeat once the interval has passed', () => {
    const r = rig();
    r.pump.update();
    r.advance(8);
    expect(r.pump.update()).toBe(true);
    expect(r.frames).toHaveLength(2);
    expect(r.pump.stats.heartbeats).toBe(1);
  });

  it('writes at once on a change, however soon after the last frame', () => {
    const r = rig();
    r.pump.update();
    r.advance(1);
    r.press('KeyD');
    r.advance(1);
    expect(r.pump.update()).toBe(true);
    expect(r.last().axes[AXIS.ROLL]).toBeGreaterThan(0);
  });

  it('numbers frames upward and stamps each with the shared clock', () => {
    const r = rig();
    r.run(100, 8);
    expect(r.frames.map((f) => f.seq)).toEqual(r.frames.map((_, i) => i + 1));
    expect(r.last().tSentMs).toBe(SHARED0 + r.now());
  });

  it('uses the interval it is given (33 ms for the message channel)', () => {
    const r = rig({ minIntervalMs: 33 });
    r.run(200, 4);
    expect(r.frames.length).toBeGreaterThanOrEqual(5);
    expect(r.frames.length).toBeLessThanOrEqual(8);
  });

  it('counts frames the channel refuses, and sends the change again when it can', () => {
    const r = rig({ accept: false });
    r.pump.update();
    expect(r.pump.stats).toMatchObject({ sent: 0, refused: 1 });
    r.setAccept(true);
    r.advance(1);
    expect(r.pump.update()).toBe(true); // still unsent, so still a change
    expect(r.pump.stats).toMatchObject({ sent: 1, refused: 1 });
    expect(r.last().flags & INPUT_FLAG.CHANGED).toBe(INPUT_FLAG.CHANGED);
  });
});

describe('InputPump: keyboard', () => {
  it('flies with flight-stick conventions: W pushes forward (nose down), S pulls back (nose up)', () => {
    const r = rig();
    r.press('KeyW');
    r.run(400);
    expect(r.last().axes[AXIS.PITCH]).toBe(-1);
    r.release('KeyW');
    r.press('KeyS');
    r.run(1000);
    expect(r.last().axes[AXIS.PITCH]).toBe(1);
  });

  it('rolls right with D and yaws right with E', () => {
    const r = rig();
    r.press('KeyD');
    r.press('KeyE');
    r.run(500);
    expect(r.last().axes[AXIS.ROLL]).toBe(1);
    expect(r.last().axes[AXIS.YAW]).toBe(1);
  });

  it('ramps at 3 per second, not instantly', () => {
    const r = rig();
    r.pump.update();
    r.press('KeyD');
    r.run(100);
    const roll = r.last().axes[AXIS.ROLL] ?? 0;
    expect(roll).toBeGreaterThan(0.25);
    expect(roll).toBeLessThan(0.35);
  });

  it('uses the ramp rates of the profile', () => {
    const profile = builtinProfile('default-km');
    profile.keyboard = { rampUpPerSecond: 6, returnPerSecond: 4 };
    const r = rig({ profile });
    r.pump.update();
    r.press('KeyD');
    r.run(100);
    expect(r.last().axes[AXIS.ROLL] ?? 0).toBeGreaterThan(0.55);
  });

  it('moves the throttle lever with Shift and Ctrl, from 0.5, pausing at military power', () => {
    const r = rig();
    r.pump.update();
    expect(r.frames[0]?.axes[AXIS.THROTTLE]).toBe(0.5);
    r.press('ShiftLeft');
    r.run(500); // 0.5 per second
    expect(r.last().axes[AXIS.THROTTLE]).toBeCloseTo(0.75, 2);
    r.run(200);
    expect(r.last().axes[AXIS.THROTTLE]).toBeCloseTo(MIL_DETENT, 6); // the detent holds it
    r.run(300); // the 300 ms pause ends; at the pause's end the lever moves on
    r.run(5000);
    expect(r.last().axes[AXIS.THROTTLE]).toBe(1);
    r.release('ShiftLeft');
    r.press('ControlLeft');
    r.run(10_000);
    expect(r.last().axes[AXIS.THROTTLE]).toBe(0);
    expect(r.pump.throttle).toBe(0);
  });

  it('toggles the afterburner with Z: full, then military power', () => {
    const r = rig();
    r.pump.update();
    r.press('KeyZ');
    r.run(8);
    expect(r.last().axes[AXIS.THROTTLE]).toBe(1);
    r.release('KeyZ');
    r.run(8);
    r.press('KeyZ');
    r.run(8);
    expect(r.last().axes[AXIS.THROTTLE]).toBeCloseTo(MIL_DETENT, 6);
  });

  it('steps the throttle with the wheel, landing on military power first', () => {
    const r = rig({ mouse: true });
    r.pump.update();
    expect(r.pump.throttle).toBe(0.5);
    r.mouse?.notch('up', 3); // 0.65
    r.run(8);
    expect(r.pump.throttle).toBeCloseTo(0.65, 6);
    r.mouse?.notch('up', 1); // wheel step 0.05 each
    r.run(8);
    expect(r.pump.throttle).toBeCloseTo(0.7, 6);
    r.mouse?.notch('down', 2);
    r.run(8);
    expect(r.pump.throttle).toBeCloseTo(0.6, 6);
  });

  it('presses reset with R', () => {
    const r = rig();
    r.press('KeyR');
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.RESET));
    r.release('KeyR');
    r.advance(4);
    r.pump.update();
    expect(r.last().buttons).toBe(0n);
  });

  it('holds pitch trim with the brackets and resets it with backslash', () => {
    const r = rig();
    r.press('BracketRight');
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.TRIM_NOSE_UP));
    r.release('BracketRight');
    r.press('BracketLeft');
    r.advance(4);
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.TRIM_NOSE_DOWN));
    r.release('BracketLeft');
    r.press('Backslash');
    r.advance(4);
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.TRIM_RESET));
  });

  it('presses the systems buttons: gear, flaps, parking brake, recover', () => {
    const cases: [string, number][] = [
      ['KeyL', BUTTON.GEAR],
      ['KeyV', BUTTON.FLAPS_CYCLE],
      ['KeyP', BUTTON.PARKING_BRAKE],
      ['KeyU', BUTTON.RECOVER],
    ];
    for (const [code, b] of cases) {
      const r = rig();
      r.press(code);
      r.pump.update();
      expect(r.last().buttons, code).toBe(bit(b));
    }
  });

  it('brakes both wheels with B', () => {
    const r = rig();
    r.press('KeyB');
    r.pump.update();
    expect(r.last().axes[AXIS.BRAKE_L]).toBe(1);
    expect(r.last().axes[AXIS.BRAKE_R]).toBe(1);
    r.release('KeyB');
    r.advance(4);
    r.pump.update();
    expect(r.last().axes[AXIS.BRAKE_L]).toBe(0);
  });

  it('toggles the airbrake with N: a held button while open', () => {
    const r = rig();
    r.press('KeyN');
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.AIRBRAKE));
    r.release('KeyN');
    r.advance(4);
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.AIRBRAKE)); // stays open
    r.press('KeyN');
    r.advance(4);
    r.pump.update();
    expect(r.last().buttons).toBe(0n);
  });

  it('holds the airbrake only while the key is down when the profile says so', () => {
    const profile = builtinProfile('default-km');
    profile.toggles.airbrake = 'hold';
    const r = rig({ profile });
    r.press('KeyN');
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.AIRBRAKE));
    r.release('KeyN');
    r.advance(4);
    r.pump.update();
    expect(r.last().buttons).toBe(0n);
  });

  it('never writes a negative zero', () => {
    const r = rig();
    r.press('KeyW');
    r.run(100);
    r.release('KeyW');
    r.run(1000);
    for (const f of r.frames) {
      for (const a of f.axes) expect(Object.is(a, -0)).toBe(false);
    }
  });
});

describe('InputPump: gamepad', () => {
  it('shapes the right stick through the stick pipeline', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ axes: { [PAD_AXIS.RIGHT_X]: 0.5, [PAD_AXIS.RIGHT_Y]: -0.8 } }));
    r.pump.update();
    expect(r.last().axes[AXIS.ROLL]).toBeCloseTo(shapeStatic(0.5, STICK_SHAPE), 6);
    expect(r.last().axes[AXIS.PITCH]).toBeCloseTo(shapeStatic(-0.8, STICK_SHAPE), 6);
  });

  it('ignores a stick resting inside the dead zone', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ axes: { 2: 0.02, 3: -0.02, 1: 0.02 } }));
    r.run(1000);
    expect([r.last().axes[AXIS.ROLL], r.last().axes[AXIS.PITCH]]).toEqual([0, 0]);
    expect(r.last().axes[AXIS.THROTTLE]).toBe(0.5);
  });

  it('yaws with the triggers: right minus left', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ buttons: { 7: 1 } }));
    r.pump.update();
    expect(r.last().axes[AXIS.YAW]).toBeCloseTo(1, 6);
    r.pads.set(0, fakePad({ buttons: { 6: 0.5 } }));
    r.advance(4);
    r.pump.update();
    expect(r.last().axes[AXIS.YAW]).toBeLessThan(-0.4);
  });

  it('raises the throttle while the left stick is held up (negative Y)', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ axes: { 1: -1 } }));
    r.run(250);
    expect(r.last().axes[AXIS.THROTTLE]).toBeGreaterThan(0.7);
    r.pads.set(0, fakePad({ axes: { 1: 1 } }));
    r.run(5000);
    expect(r.last().axes[AXIS.THROTTLE]).toBe(0);
  });

  it('trims with the d-pad like the stick: down is nose up', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ buttons: { 13: 1 } }));
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.TRIM_NOSE_UP));
    r.pads.set(0, fakePad({ buttons: { 12: 1 } }));
    r.advance(4);
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.TRIM_NOSE_DOWN));
  });

  it('works the flaps from the d-pad sides, the gear from Y and the brakes from X', () => {
    const r = padRig();
    const press = (buttons: Record<number, number>): bigint => {
      r.pads.set(0, fakePad({ buttons }));
      r.advance(4);
      r.pump.update();
      return r.last().buttons;
    };
    expect(press({ 15: 1 })).toBe(bit(BUTTON.FLAPS_DOWN));
    expect(press({ 14: 1 })).toBe(bit(BUTTON.FLAPS_UP));
    expect(press({ 3: 1 })).toBe(bit(BUTTON.GEAR));
    expect(press({ 0: 1 })).toBe(bit(BUTTON.PARKING_BRAKE));
    expect(press({ 4: 1 })).toBe(bit(BUTTON.RECOVER));
    expect(press({ 2: 1 })).toBe(0n);
    expect(r.last().axes[AXIS.BRAKE_L]).toBe(1);
  });

  it('presses reset with Back', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ buttons: { 8: 1 } }));
    r.pump.update();
    expect(r.last().buttons).toBe(bit(BUTTON.RESET));
  });

  it('holds the afterburner on L3 while held and drops to military power on release', () => {
    const r = padRig();
    r.pump.update();
    r.pads.set(0, fakePad({ buttons: { 10: 1 } }));
    r.run(8);
    expect(r.last().axes[AXIS.THROTTLE]).toBe(1);
    r.pads.set(0, fakePad());
    r.run(8);
    expect(r.last().axes[AXIS.THROTTLE]).toBeCloseTo(MIL_DETENT, 6);
  });

  it('uses whichever of keyboard and pad is pushed harder', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ axes: { 2: 1 } })); // full right
    r.press('KeyA'); // gentle left, still ramping
    r.run(100);
    expect(r.last().axes[AXIS.ROLL]).toBe(1);
  });

  it('lets go at once when the pad is unplugged, instead of holding the last position', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ axes: { 2: 1 } }));
    r.run(50);
    expect(r.last().axes[AXIS.ROLL]).toBe(1);
    r.pads.set(0, null);
    r.advance(4);
    r.pump.update();
    expect(r.last().axes[AXIS.ROLL]).toBe(0);
  });
});

describe('InputPump: hot-plug (AC-58.6)', () => {
  it('goes neutral at once when the stick is lost, holds the throttle, and takes it back within 2 s', () => {
    const profile = builtinProfile('generic-joystick');
    const r = rig({ profile });
    r.pads.set(0, fakeStick({ axes: { 0: 0.8, 1: -0.6, 2: 0.2 } }));
    r.run(100);
    const flying = r.last();
    expect(flying.axes[AXIS.ROLL]).toBeGreaterThan(0.5);
    expect(flying.axes[AXIS.PITCH]).toBeLessThan(-0.3);
    const throttle = r.last().axes[AXIS.THROTTLE] ?? 0;

    // Unplugged: the very next update is neutral, with the throttle where it was.
    r.pads.set(0, null);
    r.advance(4);
    r.pump.update();
    expect(r.last().axes[AXIS.ROLL]).toBe(0);
    expect(r.last().axes[AXIS.PITCH]).toBe(0);
    expect(r.last().axes[AXIS.THROTTLE]).toBeCloseTo(throttle, 6);
    r.run(1500);
    expect(r.last().axes[AXIS.ROLL]).toBe(0);
    expect(r.last().axes[AXIS.THROTTLE]).toBeCloseTo(throttle, 6);

    // Plugged back in, in another slot: flying again at the next update, well inside 2 s.
    const back = r.now();
    r.pads.set(3, fakeStick({ index: 3, axes: { 0: 0.8, 1: -0.6, 2: 0.2 } }));
    r.advance(4);
    r.pump.update();
    expect(r.now() - back).toBeLessThan(2000);
    expect(r.last().axes[AXIS.ROLL]).toBeGreaterThan(0.5);
    expect(r.last().axes[AXIS.PITCH]).toBeLessThan(-0.3);
  });

  it('keeps the keyboard flying throughout', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ axes: { 2: 1 } }));
    r.press('KeyD');
    r.run(500);
    r.pads.set(0, null);
    r.run(20);
    expect(r.last().axes[AXIS.ROLL]).toBe(1);
  });

  it('shows no stuck input when a pad drops while a button is held', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ buttons: { 2: 1, 7: 1 } }));
    r.run(20);
    expect(r.last().axes[AXIS.BRAKE_L]).toBe(1);
    r.pads.set(0, null);
    r.advance(4);
    r.pump.update();
    expect(r.last().axes[AXIS.BRAKE_L]).toBe(0);
    expect(r.last().axes[AXIS.YAW]).toBe(0);
  });
});

describe('InputPump: an absolute throttle lever', () => {
  const lever = (value: number, extra: Record<number, number> = {}) =>
    fakeStick({ axes: { 2: value, ...extra } });

  it("takes the lever's position (a lever pushed forward reads -1) and follows its movement", () => {
    const r = rig({ profile: builtinProfile('generic-joystick') });
    r.pads.set(0, lever(-1));
    r.run(20);
    expect(r.last().axes[AXIS.THROTTLE]).toBe(1);
    r.pads.set(0, lever(0));
    r.run(20);
    expect(r.last().axes[AXIS.THROTTLE]).toBeCloseTo(0.5, 6);
    r.pads.set(0, lever(1));
    r.run(20);
    expect(r.last().axes[AXIS.THROTTLE]).toBe(0);
  });

  it('lets the keyboard move the lever when the physical one has not moved', () => {
    const r = rig({ profile: builtinProfile('generic-joystick') });
    r.pads.set(0, lever(1)); // idle
    r.run(20);
    expect(r.pump.throttle).toBe(0);
    r.press('ShiftLeft');
    r.run(400);
    expect(r.pump.throttle).toBeGreaterThan(0.1);
    r.release('ShiftLeft');
    r.pads.set(0, lever(0.5)); // moving the lever takes over again
    r.run(20);
    expect(r.pump.throttle).toBeCloseTo(0.25, 6);
  });
});

describe('InputPump: what each frame says about when things happened (AC-58.3)', () => {
  it('stamps t_ms with the time of the event and t_sent_ms with the time of the write', () => {
    const r = rig();
    r.pump.update();
    r.advance(10);
    r.press('KeyD');
    r.pump.poke(r.now() - 3); // the key event happened 3 ms ago
    r.pump.update();
    const f = r.last();
    expect(f.tSentMs).toBe(SHARED0 + r.now());
    expect(f.tMs).toBe(SHARED0 + r.now() - 3);
    expect(f.flags & INPUT_FLAG.CHANGED).toBe(INPUT_FLAG.CHANGED);
  });

  it('uses the earliest of several events not yet sent', () => {
    const r = rig();
    r.pump.update();
    r.advance(20);
    r.press('KeyD');
    r.pump.poke(r.now() - 2);
    r.pump.poke(r.now() - 9);
    r.pump.poke(r.now() - 5);
    r.pump.update();
    expect(r.last().tMs).toBe(SHARED0 + r.now() - 9);
  });

  it('uses the poll time for a change nothing announced', () => {
    const r = rig();
    r.pump.update();
    r.advance(10);
    r.press('KeyD');
    r.pump.update();
    expect(r.last().tMs).toBe(r.last().tSentMs);
    expect(r.last().flags & INPUT_FLAG.CHANGED).toBe(INPUT_FLAG.CHANGED);
  });

  it("uses a pad's own timestamp when its reading changed", () => {
    const r = padRig();
    r.pump.update();
    r.advance(10);
    r.pads.set(0, fakePad({ axes: { 2: 0.9 }, timestamp: r.now() - 6 }));
    r.pump.update();
    expect(r.last().tMs).toBe(SHARED0 + r.now() - 6);
    // A stale timestamp (an idle device) is not believed.
    r.advance(10);
    r.pads.set(0, fakePad({ axes: { 2: 0.2 }, timestamp: r.now() - 5000 }));
    r.pump.update();
    expect(r.last().tMs).toBe(r.last().tSentMs);
  });

  it('flags CHANGED when an axis moves, a button changes or a flag changes, and not on a heartbeat', () => {
    const r = rig();
    r.pump.update();
    expect(r.last().flags & INPUT_FLAG.CHANGED).toBe(INPUT_FLAG.CHANGED); // the first frame sets everything
    r.advance(8);
    r.pump.update(); // heartbeat
    expect(r.last().flags & INPUT_FLAG.CHANGED).toBe(0);
    expect(r.pump.stats.heartbeats).toBe(1);
    r.press('KeyL');
    r.advance(1);
    r.pump.update();
    expect(r.last().flags & INPUT_FLAG.CHANGED).toBe(INPUT_FLAG.CHANGED);
    r.release('KeyL');
    r.advance(1);
    r.pump.update();
    expect(r.last().flags & INPUT_FLAG.CHANGED).toBe(INPUT_FLAG.CHANGED);
    r.pump.setAutoGcas(true);
    r.advance(1);
    r.pump.update();
    expect(r.last().flags).toBe(INPUT_FLAG.CHANGED | INPUT_FLAG.AUTO_GCAS);
    expect(r.pump.stats.changed).toBeGreaterThanOrEqual(4);
  });

  it('does not count a movement of 1e-5 or less as a change', () => {
    const r = padRig();
    r.pads.set(0, fakePad({ axes: { 2: 0.5 } }));
    r.pump.update();
    const sent = r.frames.length;
    const shaped = shapeStatic(0.5, STICK_SHAPE);
    const tiny = shapeStatic(0.5 + 5e-6, STICK_SHAPE) - shaped;
    expect(Math.abs(tiny)).toBeLessThan(1e-5);
    r.pads.set(0, fakePad({ axes: { 2: 0.5 + 5e-6 } }));
    r.advance(1);
    expect(r.pump.update()).toBe(false);
    expect(r.frames).toHaveLength(sent);
  });

  it('applies a step on a key press so the frame at the event already shows a change', () => {
    const r = rig();
    r.pump.update(); // the clock has not moved since: a key press and its update land on the same millisecond
    r.press('KeyD');
    r.pump.poke(r.now());
    expect(r.pump.update()).toBe(true);
    expect(r.last().axes[AXIS.ROLL] ?? 0).toBeGreaterThan(0);
    expect(r.last().flags & INPUT_FLAG.CHANGED).toBe(INPUT_FLAG.CHANGED);
  });

  it('drops an event that changed nothing, so it cannot date a later change', () => {
    const r = rig();
    r.pump.update();
    r.advance(2);
    r.pump.poke(r.now()); // an event that moved nothing
    r.pump.update();
    r.advance(40);
    r.press('KeyD');
    r.pump.update();
    expect(r.last().tMs).toBe(r.last().tSentMs);
  });

  it('keeps an event that could not be sent for the retry', () => {
    const r = rig();
    r.pump.update();
    r.advance(10);
    r.press('KeyD');
    r.pump.poke(r.now() - 4);
    r.setAccept(false);
    r.pump.update();
    r.setAccept(true);
    r.advance(6);
    r.pump.update();
    expect(r.last().tMs).toBe(SHARED0 + r.now() - 10);
  });

  it('does not believe an event time from the future or long ago', () => {
    const r = rig();
    r.pump.update();
    r.advance(10);
    r.press('KeyD');
    r.pump.poke(r.now() + 5000);
    r.pump.poke(r.now() - 5000);
    r.pump.poke(Number.NaN);
    r.pump.update();
    expect(r.last().tMs).toBe(r.last().tSentMs);
  });

  it('never marks a neutral frame CHANGED', () => {
    const r = rig();
    r.press('KeyD');
    r.run(100);
    r.pump.neutral();
    expect(r.last().flags).toBe(INPUT_FLAG.NEUTRAL);
  });
});

describe('InputPump: losing control', () => {
  it('sends a neutral frame: sticks zero, buttons clear, NEUTRAL flag, throttle kept', () => {
    const r = rig();
    r.press('KeyD');
    r.press('ShiftLeft');
    r.run(600);
    const throttle = r.pump.throttle;
    expect(throttle).toBeGreaterThan(0.5);
    r.pump.neutral();
    const f = r.last();
    expect(f.flags).toBe(INPUT_FLAG.NEUTRAL);
    expect([f.axes[AXIS.PITCH], f.axes[AXIS.ROLL], f.axes[AXIS.YAW]]).toEqual([0, 0, 0]);
    expect(f.axes[AXIS.THROTTLE]).toBeCloseTo(throttle, 6);
    expect(f.buttons).toBe(0n);
    expect(r.pump.stats.neutralFrames).toBe(1);
  });

  it('forgets emulated state, so nothing stale returns with focus', () => {
    const r = rig();
    r.press('KeyD');
    r.run(500);
    r.keyboard.releaseAll();
    r.pump.neutral();
    r.advance(1000); // away for a second
    r.pump.update();
    expect(r.last().flags).toBe(0);
    expect(r.last().axes[AXIS.ROLL]).toBe(0);
  });

  it('does not replay a long absence as one giant step', () => {
    const r = rig();
    r.press('KeyD');
    r.pump.update();
    r.advance(60_000); // the tab was frozen for a minute
    r.pump.update();
    // Capped at 100 ms of ramp at 3 per second.
    expect(r.last().axes[AXIS.ROLL]).toBeLessThanOrEqual(0.31);
  });

  it('does not take keys that were already down for new presses when focus returns', () => {
    const r = rig();
    r.press('KeyL');
    r.pump.update();
    r.pump.neutral();
    r.advance(4);
    r.pump.update();
    expect(r.last().buttons).toBe(0n); // the gear key is still down, but it was pressed before
  });
});

describe('InputPump: simulation requests', () => {
  const obs = (extra: Partial<PumpObservation> = {}): PumpObservation => ({
    spawnSeq: 1,
    throttle: 0,
    assist: 2,
    noseAzRad: 0,
    noseElRad: 0,
    ...extra,
  });

  it('sends no request until asked', () => {
    const r = rig();
    r.pump.update();
    expect([r.last().assistReq, r.last().spawnReq, r.last().spawnAck]).toEqual([0, 0, 0]);
  });

  it('synchronises the lever to a new spawn and acknowledges it at once', () => {
    const r = rig();
    r.pump.update();
    r.pump.observe(obs({ spawnSeq: 1, throttle: 0.05 }));
    expect(r.pump.throttle).toBe(0.05);
    r.advance(1);
    expect(r.pump.update()).toBe(true);
    expect(r.last().spawnAck).toBe(1);
    expect(r.last().axes[AXIS.THROTTLE]).toBeCloseTo(0.05, 6);
    // The same spawn again changes nothing, even if the lever has moved since.
    r.press('ShiftLeft');
    r.run(400);
    const moved = r.pump.throttle;
    r.pump.observe(obs({ spawnSeq: 1, throttle: 0.05 }));
    expect(r.pump.throttle).toBe(moved);
  });

  it('hands the lever back at every respawn', () => {
    const r = rig();
    r.pump.observe(obs({ spawnSeq: 1, throttle: 0 }));
    r.press('ShiftLeft');
    r.run(400);
    expect(r.pump.throttle).toBeGreaterThan(0.1);
    r.pump.observe(obs({ spawnSeq: 2, throttle: 0.62 }));
    expect(r.pump.throttle).toBe(0.62);
    r.advance(1);
    r.pump.update();
    expect(r.last().spawnAck).toBe(2);
  });

  it("takes an absolute lever's position after the spawn is acknowledged", () => {
    const r = rig({ profile: builtinProfile('generic-joystick') });
    r.pads.set(0, fakeStick({ axes: { 2: -0.5 } })); // lever three quarters forward
    r.run(20);
    r.pump.observe(obs({ spawnSeq: 1, throttle: 0 }));
    expect(r.pump.throttle).toBe(0);
    r.advance(4);
    r.pump.update();
    expect(r.pump.throttle).toBeCloseTo(0.75, 6);
  });

  it('keeps asking for an assist level until told otherwise', () => {
    const r = rig();
    r.pump.requestAssist(4);
    r.pump.update();
    expect(r.last().assistReq).toBe(4);
    r.run(100, 8);
    expect(r.frames.every((f) => f.assistReq === 4)).toBe(true);
    r.pump.requestAssist(9); // not a level: ignored
    r.pump.requestAssist(0);
    expect(r.pump.assistRequest).toBe(4);
  });

  it('sends a request at once, without waiting for the heartbeat', () => {
    const r = rig();
    r.pump.update();
    r.advance(1);
    r.pump.requestAssist(1);
    expect(r.pump.update()).toBe(true);
    expect(r.last().assistReq).toBe(1);
    expect(r.last().flags & INPUT_FLAG.CHANGED).toBe(0); // not a stick movement
  });

  it('cycles the assist level with F8: Full, Standard, Reduced, Manual, Full', () => {
    const r = rig();
    r.pump.observe(obs({ assist: 2 }));
    const levels: number[] = [];
    for (let i = 0; i < 5; i++) {
      r.press('F8');
      r.advance(4);
      r.pump.update();
      levels.push(r.last().assistReq);
      r.release('F8');
      r.advance(4);
      r.pump.update();
    }
    expect(levels).toEqual([3, 4, 1, 2, 3]);
    expect(r.actions.filter((a) => a === 'flight.assist_cycle')).toHaveLength(5);
  });

  it('asks for the spawn a respawn should go to', () => {
    const r = rig();
    r.pump.setRespawnKind(2);
    r.pump.update();
    expect(r.last().spawnReq).toBe(2);
    r.pump.setRespawnKind(7); // not a spawn: the session\'s
    r.advance(1);
    r.pump.update();
    expect(r.last().spawnReq).toBe(0);
  });

  it('tells the page about respawn, mouse mode and assist presses, once each', () => {
    const r = rig();
    r.press('KeyR');
    r.run(20);
    r.release('KeyR');
    r.press('KeyO');
    r.run(20);
    r.release('KeyO');
    expect(r.actions).toEqual(['system.respawn', 'flight.mouse_mode']);
  });
});

describe('InputPump: the menu context', () => {
  it('silences the flight controls and keeps the throttle', () => {
    const r = rig();
    r.press('KeyD');
    r.press('ShiftLeft');
    r.run(500);
    const throttle = r.pump.throttle;
    r.pump.setContext('menu');
    r.run(100);
    const f = r.last();
    expect(f.axes[AXIS.ROLL]).toBe(0);
    expect(f.axes[AXIS.THROTTLE]).toBeCloseTo(throttle, 6);
    expect(f.buttons).toBe(0n);
    r.press('KeyL');
    r.run(20);
    expect(r.last().buttons).toBe(0n);
  });

  it('still reports the pause key, and does not fire for keys already held on returning', () => {
    const r = rig();
    r.press('Escape');
    r.run(8);
    r.release('Escape');
    r.run(8);
    expect(r.actions).toEqual(['system.pause']);
    r.pump.setContext('menu');
    r.run(8);
    r.press('Escape');
    r.run(8);
    r.release('Escape');
    r.run(8);
    expect(r.actions).toEqual(['system.pause', 'system.pause']);
    r.press('KeyL'); // held while the menu closes
    r.run(8);
    r.pump.setContext('flight');
    r.run(8);
    expect(r.last().buttons).toBe(0n);
    r.release('KeyL');
    r.run(8);
    r.press('KeyL');
    r.run(8);
    expect(r.last().buttons).toBe(bit(BUTTON.GEAR));
  });

  it('is not fooled by pause being held while the page changes the context from the callback', () => {
    const profile = builtinProfile('default-km');
    const frames: InputFrame[] = [];
    const keyboard = new KeyboardState();
    let mono = 0;
    const holder: { pump?: InputPump } = {};
    const pump = new InputPump({
      sink: {
        send: (b) => {
          frames.push(decodeInputFrame(b));
          return true;
        },
        close: () => undefined,
      },
      keyboard,
      profile,
      nowMs: () => mono,
      sharedNowMs: () => mono,
      onAction: (id) => {
        if (id === 'system.pause') {
          holder.pump?.setContext(holder.pump.context === 'flight' ? 'menu' : 'flight');
        }
      },
    });
    holder.pump = pump;
    keyboard.keyDown('Escape', false);
    for (let i = 0; i < 10; i++) {
      mono += 4;
      pump.update();
    }
    expect(pump.context).toBe('menu'); // one press, one change, however long it is held
  });
});

describe('InputPump: the mouse', () => {
  it('sends the aim as a cursor with the MOUSE_AIM flag, from the nose, within the cone', () => {
    const r = rig({ mouse: true });
    r.pump.observe({ spawnSeq: 1, throttle: 0, assist: 2, noseAzRad: 0.5, noseElRad: 0.1 });
    r.pump.setMouseMode('aim');
    r.run(8);
    expect(r.last().flags & INPUT_FLAG.MOUSE_AIM).toBe(INPUT_FLAG.MOUSE_AIM);
    expect(r.last().axes[AXIS.CURSOR_X]).toBeCloseTo(0.5 / Math.PI, 5);
    expect(r.last().axes[AXIS.CURSOR_Y]).toBeCloseTo(0.1 / (Math.PI / 2), 5);
    r.mouse?.push(100, -50);
    r.run(8);
    expect(r.last().axes[AXIS.CURSOR_X] ?? 0).toBeGreaterThan(0.5 / Math.PI);
    expect(r.last().axes[AXIS.CURSOR_Y] ?? 0).toBeGreaterThan(0.1 / (Math.PI / 2));
    // Far past the cone: held at 35 degrees from the nose.
    r.mouse?.push(100_000, 0);
    r.run(8);
    const az = (r.last().axes[AXIS.CURSOR_X] ?? 0) * Math.PI;
    expect(Math.abs(az - 0.5)).toBeLessThan((36 * Math.PI) / 180);
    expect(Math.abs(az - 0.5)).toBeGreaterThan((30 * Math.PI) / 180);
  });

  it('sends no aim when the pointer is not locked or the mode is off', () => {
    const r = rig({ mouse: true });
    r.pump.setMouseMode('aim');
    if (r.mouse) r.mouse.locked = false;
    r.mouse?.push(100, 100);
    r.run(8);
    expect(r.last().flags & INPUT_FLAG.MOUSE_AIM).toBe(0);
    if (r.mouse) r.mouse.locked = true;
    r.pump.setMouseMode('off');
    r.run(8);
    expect(r.last().flags & INPUT_FLAG.MOUSE_AIM).toBe(0);
    expect([r.last().axes[AXIS.CURSOR_X], r.last().axes[AXIS.CURSOR_Y]]).toEqual([0, 0]);
  });

  it('flies the mouse as a stick: right is roll right, back is nose up, and it springs back', () => {
    const r = rig({ mouse: true });
    r.pump.setMouseMode('stick');
    r.mouse?.push(200, 100);
    r.run(8);
    expect(r.last().axes[AXIS.ROLL]).toBeCloseTo(0.5, 6);
    expect(r.last().axes[AXIS.PITCH]).toBeCloseTo(0.25, 6);
    expect(r.last().flags & INPUT_FLAG.MOUSE_AIM).toBe(0);
    r.pump.setMouseTuning(1, 1);
    r.run(3000);
    expect(r.last().axes[AXIS.ROLL]).toBe(0);
  });

  it('scales with the sensitivity setting', () => {
    const r = rig({ mouse: true });
    r.pump.setMouseMode('stick');
    r.pump.setMouseTuning(2, 0);
    r.mouse?.push(100, 0);
    r.run(8);
    expect(r.last().axes[AXIS.ROLL]).toBeCloseTo(0.5, 6);
  });

  it('cycles aim, stick, off with O', () => {
    const r = rig({ mouse: true });
    const seen: string[] = [];
    for (let i = 0; i < 4; i++) {
      r.press('KeyO');
      r.run(8);
      seen.push(r.pump.mouseMode);
      r.release('KeyO');
      r.run(8);
    }
    expect(seen).toEqual(['aim', 'stick', 'off', 'aim']);
  });

  it('takes the mouse for a button too', () => {
    const profile = builtinProfile('default-km');
    profile.bindings = [
      ...profile.bindings,
      { action: 'flight.brakes', source: { kind: 'mouse-button', button: 2 } },
    ];
    const r = rig({ profile, mouse: true });
    r.mouse?.down.add(2);
    r.run(8);
    expect(r.last().axes[AXIS.BRAKE_L]).toBe(1);
  });
});

describe('InputPump: profiles', () => {
  it('flies with a rebound profile at once, keeping the throttle lever', () => {
    const r = rig();
    r.press('ShiftLeft');
    r.run(300);
    const throttle = r.pump.throttle;
    r.release('ShiftLeft');
    const profile = builtinProfile('default-km');
    const bindings: Binding[] = profile.bindings.filter((b) => b.action !== 'flight.pitch');
    bindings.push(
      { action: 'flight.pitch', source: { kind: 'key', code: 'KeyI' }, direction: -1 },
      { action: 'flight.pitch', source: { kind: 'key', code: 'KeyK' }, direction: 1 },
    );
    r.pump.setProfile({ ...profile, bindings });
    expect(r.pump.bindings.keyCodes.has('KeyI')).toBe(true);
    expect(r.pump.bindings.keyCodes.has('KeyW')).toBe(false);
    r.press('KeyW');
    r.run(300);
    expect(r.last().axes[AXIS.PITCH]).toBe(0);
    r.release('KeyW');
    r.press('KeyI');
    r.run(400);
    expect(r.last().axes[AXIS.PITCH]).toBe(-1);
    expect(r.pump.throttle).toBeCloseTo(throttle, 6);
  });

  it('picks up pad bindings of a new profile without waiting for a device change', () => {
    const r = rig();
    r.pads.set(0, fakePad({ axes: { 2: 1 } }));
    r.run(50);
    expect(r.last().axes[AXIS.ROLL]).toBe(0);
    r.pump.setProfile(builtinProfile('gamepad-standard'));
    r.run(8);
    expect(r.last().axes[AXIS.ROLL]).toBe(1);
  });
});
