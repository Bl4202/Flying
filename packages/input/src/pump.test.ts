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
import { type GamepadSample } from './gamepad.ts';
import { PAD_AXIS, PAD_BUTTON } from './gamepad.ts';
import { KeyboardState } from './keyboard.ts';
import { InputPump } from './pump.ts';
import { shapeStatic, STICK_SHAPE } from './shaping.ts';

function rig(opts: { minIntervalMs?: number; accept?: boolean } = {}) {
  const frames: InputFrame[] = [];
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
  let pad: GamepadSample | null = null;
  let mono = 1000;
  const pump = new InputPump({
    sink,
    keyboard,
    gamepad: { poll: () => pad },
    nowMs: () => mono,
    sharedNowMs: () => 1_700_000_000_000 + mono,
    ...(opts.minIntervalMs !== undefined ? { minIntervalMs: opts.minIntervalMs } : {}),
  });
  const padWith = (
    axes: Record<number, number>,
    buttons: Record<number, number> = {},
  ): GamepadSample => ({
    index: 0,
    id: 'pad',
    axes: Array.from({ length: 4 }, (_, i) => axes[i] ?? 0),
    buttons: Array.from({ length: 17 }, (_, i) => buttons[i] ?? 0),
  });
  return {
    pump,
    keyboard,
    frames,
    advance: (ms: number) => {
      mono += ms;
    },
    now: () => mono,
    setPad: (p: GamepadSample | null) => {
      pad = p;
    },
    padWith,
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
  };
}

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
    r.keyboard.keyDown('KeyD', false);
    r.advance(1);
    expect(r.pump.update()).toBe(true);
    expect(r.frames[1]?.axes[AXIS.ROLL]).toBeGreaterThan(0);
  });

  it('numbers frames upward and stamps each with the shared clock', () => {
    const r = rig();
    r.run(100, 8);
    expect(r.frames.map((f) => f.seq)).toEqual(r.frames.map((_, i) => i + 1));
    expect(r.frames.at(-1)?.tMs).toBe(1_700_000_000_000 + r.now());
  });

  it('uses the interval it is given (33 ms for the message channel)', () => {
    const r = rig({ minIntervalMs: 33 });
    r.run(200, 4);
    expect(r.frames.length).toBeGreaterThanOrEqual(5);
    expect(r.frames.length).toBeLessThanOrEqual(8);
  });

  it('counts frames the channel refuses', () => {
    const r = rig({ accept: false });
    r.pump.update();
    expect(r.pump.stats).toMatchObject({ sent: 0, refused: 1 });
    r.setAccept(true);
    r.advance(10);
    r.pump.update();
    expect(r.pump.stats).toMatchObject({ sent: 1, refused: 1 });
  });
});

describe('InputPump: keyboard', () => {
  it('flies with flight-stick conventions: W pushes forward (nose down), S pulls back (nose up)', () => {
    const r = rig();
    r.keyboard.keyDown('KeyW', false);
    r.run(400);
    expect(r.frames.at(-1)?.axes[AXIS.PITCH]).toBe(-1);
    r.keyboard.keyUp('KeyW');
    r.keyboard.keyDown('KeyS', false);
    r.run(1000);
    expect(r.frames.at(-1)?.axes[AXIS.PITCH]).toBe(1);
  });

  it('rolls right with D and yaws right with E', () => {
    const r = rig();
    r.keyboard.keyDown('KeyD', false);
    r.keyboard.keyDown('KeyE', false);
    r.run(500);
    const f = r.frames.at(-1);
    expect(f?.axes[AXIS.ROLL]).toBe(1);
    expect(f?.axes[AXIS.YAW]).toBe(1);
  });

  it('ramps at 3 per second, not instantly', () => {
    const r = rig();
    r.pump.update();
    r.keyboard.keyDown('KeyD', false);
    r.run(100);
    const roll = r.frames.at(-1)?.axes[AXIS.ROLL] ?? 0;
    expect(roll).toBeGreaterThan(0.25);
    expect(roll).toBeLessThan(0.35);
  });

  it('moves the throttle lever with Shift and Ctrl, from 0.5, within 0..1', () => {
    const r = rig();
    r.pump.update();
    expect(r.frames[0]?.axes[AXIS.THROTTLE]).toBe(0.5);
    r.keyboard.keyDown('ShiftLeft', false);
    r.run(500); // 0.5 per second
    expect(r.frames.at(-1)?.axes[AXIS.THROTTLE]).toBeCloseTo(0.75, 2);
    r.run(5000);
    expect(r.frames.at(-1)?.axes[AXIS.THROTTLE]).toBe(1);
    r.keyboard.keyUp('ShiftLeft');
    r.keyboard.keyDown('ControlLeft', false);
    r.run(10_000);
    expect(r.frames.at(-1)?.axes[AXIS.THROTTLE]).toBe(0);
    expect(r.pump.throttle).toBe(0);
  });

  it('presses reset with R', () => {
    const r = rig();
    r.keyboard.keyDown('KeyR', false);
    r.pump.update();
    expect(r.frames.at(-1)?.buttons).toBe(1n << BigInt(BUTTON.RESET));
    r.keyboard.keyUp('KeyR');
    r.advance(4);
    r.pump.update();
    expect(r.frames.at(-1)?.buttons).toBe(0n);
  });

  it('never writes a negative zero', () => {
    const r = rig();
    r.keyboard.keyDown('KeyW', false);
    r.run(100);
    r.keyboard.keyUp('KeyW');
    r.run(1000);
    for (const f of r.frames) {
      for (const a of f.axes) expect(Object.is(a, -0)).toBe(false);
    }
  });
});

describe('InputPump: gamepad', () => {
  it('shapes the right stick through the stick pipeline', () => {
    const r = rig();
    r.setPad(r.padWith({ [PAD_AXIS.RIGHT_X]: 0.5, [PAD_AXIS.RIGHT_Y]: -0.8 }));
    r.pump.update();
    const f = r.frames.at(-1);
    expect(f?.axes[AXIS.ROLL]).toBeCloseTo(shapeStatic(0.5, STICK_SHAPE), 6);
    expect(f?.axes[AXIS.PITCH]).toBeCloseTo(shapeStatic(-0.8, STICK_SHAPE), 6);
  });

  it('ignores a stick resting inside the dead zone', () => {
    const r = rig();
    r.setPad(
      r.padWith({ [PAD_AXIS.RIGHT_X]: 0.02, [PAD_AXIS.RIGHT_Y]: -0.02, [PAD_AXIS.LEFT_Y]: 0.02 }),
    );
    r.run(1000);
    const f = r.frames.at(-1);
    expect([f?.axes[AXIS.ROLL], f?.axes[AXIS.PITCH]]).toEqual([0, 0]);
    expect(f?.axes[AXIS.THROTTLE]).toBe(0.5);
  });

  it('yaws with the triggers: right minus left', () => {
    const r = rig();
    r.setPad(r.padWith({}, { [PAD_BUTTON.RIGHT_TRIGGER]: 1 }));
    r.pump.update();
    expect(r.frames.at(-1)?.axes[AXIS.YAW]).toBeCloseTo(1, 6);
    r.setPad(r.padWith({}, { [PAD_BUTTON.LEFT_TRIGGER]: 0.5 }));
    r.advance(4);
    r.pump.update();
    expect(r.frames.at(-1)?.axes[AXIS.YAW]).toBeLessThan(-0.4);
  });

  it('raises the throttle while the left stick is held up (negative Y)', () => {
    const r = rig();
    r.setPad(r.padWith({ [PAD_AXIS.LEFT_Y]: -1 }));
    r.run(250);
    expect(r.frames.at(-1)?.axes[AXIS.THROTTLE]).toBeGreaterThan(0.7);
    r.setPad(r.padWith({ [PAD_AXIS.LEFT_Y]: 1 }));
    r.run(5000);
    expect(r.frames.at(-1)?.axes[AXIS.THROTTLE]).toBe(0);
  });

  it('holds pitch trim with the brackets', () => {
    const r = rig();
    r.keyboard.keyDown('BracketRight', false);
    r.pump.update();
    expect(r.frames.at(-1)?.buttons).toBe(1n << BigInt(BUTTON.TRIM_NOSE_UP));
    r.keyboard.keyUp('BracketRight');
    r.keyboard.keyDown('BracketLeft', false);
    r.advance(4);
    r.pump.update();
    expect(r.frames.at(-1)?.buttons).toBe(1n << BigInt(BUTTON.TRIM_NOSE_DOWN));
  });

  it('trims with the d-pad like the stick: down is nose up', () => {
    const r = rig();
    r.setPad(r.padWith({}, { [PAD_BUTTON.DPAD_DOWN]: 1 }));
    r.pump.update();
    expect(r.frames.at(-1)?.buttons).toBe(1n << BigInt(BUTTON.TRIM_NOSE_UP));
  });

  it('presses reset with Back', () => {
    const r = rig();
    r.setPad(r.padWith({}, { [PAD_BUTTON.BACK]: 1 }));
    r.pump.update();
    expect(r.frames.at(-1)?.buttons).toBe(1n << BigInt(BUTTON.RESET));
  });

  it('uses whichever of keyboard and pad is pushed harder', () => {
    const r = rig();
    r.setPad(r.padWith({ [PAD_AXIS.RIGHT_X]: 1 })); // full right
    r.keyboard.keyDown('KeyA', false); // gentle left, still ramping
    r.run(100);
    expect(r.frames.at(-1)?.axes[AXIS.ROLL]).toBe(1);
  });

  it('lets go when the pad is unplugged, instead of holding the last position', () => {
    const r = rig();
    r.setPad(r.padWith({ [PAD_AXIS.RIGHT_X]: 1 }));
    r.run(50);
    expect(r.frames.at(-1)?.axes[AXIS.ROLL]).toBe(1);
    r.setPad(null);
    r.run(50);
    expect(r.frames.at(-1)?.axes[AXIS.ROLL]).toBe(0);
  });
});

describe('InputPump: losing control', () => {
  it('sends a neutral frame: sticks zero, buttons clear, NEUTRAL flag, throttle kept', () => {
    const r = rig();
    r.keyboard.keyDown('KeyD', false);
    r.keyboard.keyDown('ShiftLeft', false);
    r.run(600);
    const throttle = r.pump.throttle;
    expect(throttle).toBeGreaterThan(0.5);
    r.pump.neutral();
    const f = r.frames.at(-1);
    expect(f?.flags).toBe(INPUT_FLAG.NEUTRAL);
    expect([f?.axes[AXIS.PITCH], f?.axes[AXIS.ROLL], f?.axes[AXIS.YAW]]).toEqual([0, 0, 0]);
    expect(f?.axes[AXIS.THROTTLE]).toBeCloseTo(throttle, 6);
    expect(f?.buttons).toBe(0n);
    expect(r.pump.stats.neutralFrames).toBe(1);
  });

  it('forgets emulated state, so nothing stale returns with focus', () => {
    const r = rig();
    r.keyboard.keyDown('KeyD', false);
    r.run(500);
    r.keyboard.releaseAll();
    r.pump.neutral();
    r.advance(1000); // away for a second
    r.pump.update();
    const f = r.frames.at(-1);
    expect(f?.flags).toBe(0);
    expect(f?.axes[AXIS.ROLL]).toBe(0);
  });

  it('does not replay a long absence as one giant step', () => {
    const r = rig();
    r.keyboard.keyDown('KeyD', false);
    r.pump.update();
    r.advance(60_000); // the tab was frozen for a minute
    r.pump.update();
    // Capped at 100 ms of ramp at 3 per second.
    expect(r.frames.at(-1)?.axes[AXIS.ROLL]).toBeLessThanOrEqual(0.31);
  });
});
