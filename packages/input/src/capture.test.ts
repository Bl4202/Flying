import { describe, expect, it } from 'vitest';
import { Capture, CAPTURE_AXIS_DELTA, SETTLE_MS } from './capture.ts';
import { findConflicts } from './conflicts.ts';
import { GamepadHub } from './devices/gamepad.ts';
import { builtinProfile, type Binding, type ControlsProfile } from './profile.ts';
import { sourceKey, type Source } from './sources.ts';
import { FakePads, fakePad, fakeStick, HAT_CENTRED } from './test-support.ts';
import { type ActionId } from './actions.ts';
import { KeyboardState } from './keyboard.ts';
import { InputPump } from './pump.ts';
import { AXIS, BUTTON, decodeInputFrame, type InputFrame } from '@flying/sim-bridge';

function rig() {
  const pads = new FakePads();
  const hub = new GamepadHub(pads.get);
  const capture = new Capture();
  let now = 0;
  const poll = (ms = 0): void => {
    now += ms;
    hub.poll();
    capture.poll(hub, now);
  };
  /** Starts capture and lets the settle window pass with the device at rest. */
  const begin = (axis: 'whole' | 'half' = 'whole'): void => {
    hub.poll();
    capture.start(hub, now, axis);
    poll(0);
    poll(SETTLE_MS + 1);
  };
  return { pads, hub, capture, poll, begin, now: () => now };
}

const captured = (c: Capture): Source | undefined =>
  c.state.state === 'captured' ? c.state.source : undefined;

describe('Capture: keys, mouse and wheel', () => {
  it('takes the first key pressed', () => {
    const r = rig();
    r.capture.start(undefined, 0);
    expect(r.capture.active).toBe(true);
    r.capture.keyDown('KeyJ');
    r.capture.keyDown('KeyK');
    expect(captured(r.capture)).toEqual({ kind: 'key', code: 'KeyJ' });
    expect(r.capture.active).toBe(false);
  });

  it('cancels on Escape', () => {
    const r = rig();
    r.capture.start(undefined, 0);
    r.capture.keyDown('Escape');
    expect(r.capture.state.state).toBe('cancelled');
    r.capture.keyDown('KeyJ');
    expect(r.capture.state.state).toBe('cancelled');
  });

  it('takes a mouse button or a wheel notch', () => {
    const r = rig();
    r.capture.start(undefined, 0);
    r.capture.mouseButton(2);
    expect(captured(r.capture)).toEqual({ kind: 'mouse-button', button: 2 });
    r.capture.start(undefined, 0);
    r.capture.wheel('down');
    expect(captured(r.capture)).toEqual({ kind: 'wheel', dir: 'down' });
  });

  it('ignores everything when not listening, and can be cancelled or reset', () => {
    const r = rig();
    r.capture.keyDown('KeyJ');
    r.capture.mouseButton(0);
    r.capture.wheel('up');
    expect(r.capture.state.state).toBe('idle');
    r.capture.start(undefined, 0);
    r.capture.cancel();
    expect(r.capture.state.state).toBe('cancelled');
    r.capture.reset();
    expect(r.capture.state.state).toBe('idle');
  });
});

describe('Capture: pad buttons and hats', () => {
  it('takes a button pressed after capture began, naming the device by class', () => {
    const r = rig();
    r.pads.set(0, fakePad());
    r.begin();
    r.pads.set(0, fakePad({ buttons: { 3: 1 } }));
    r.poll();
    expect(captured(r.capture)).toEqual({ kind: 'pad-button', device: 'pad', index: 3 });
  });

  it('names a raw stick by its vendor and product', () => {
    const r = rig();
    r.pads.set(0, fakeStick());
    r.begin();
    r.pads.set(0, fakeStick({ buttons: { 5: 1 } }));
    r.poll();
    expect(captured(r.capture)).toEqual({ kind: 'pad-button', device: 'joy:044f:b10a', index: 5 });
  });

  it('does not count a button that was already down until it is let go and pressed again', () => {
    const r = rig();
    r.pads.set(0, fakePad({ buttons: { 3: 1 } }));
    r.begin();
    expect(r.capture.state.state).toBe('waiting');
    r.pads.set(0, fakePad());
    r.poll();
    r.pads.set(0, fakePad({ buttons: { 3: 1 } }));
    r.poll();
    expect(captured(r.capture)).toEqual({ kind: 'pad-button', device: 'pad', index: 3 });
  });

  it('takes a hat direction, and prefers the hat to the d-pad buttons it also presses', () => {
    const r = rig();
    r.pads.set(0, fakePad());
    r.begin();
    r.pads.set(0, fakePad({ buttons: { 12: 1 } }));
    r.poll();
    expect(captured(r.capture)).toEqual({ kind: 'hat', device: 'pad', hat: 0, dir: 'up' });
  });

  it("takes a raw stick's axis-encoded hat as a hat, not as axis 9", () => {
    const r = rig();
    r.pads.set(0, fakeStick());
    r.begin();
    r.pads.set(0, fakeStick({ axes: { 9: 0.4285714 } }));
    r.poll();
    expect(captured(r.capture)).toEqual({
      kind: 'hat',
      device: 'joy:044f:b10a',
      hat: 0,
      dir: 'down',
    });
  });

  it('takes a diagonal as its first cardinal direction', () => {
    const r = rig();
    r.pads.set(0, fakePad());
    r.begin();
    r.pads.set(0, fakePad({ buttons: { 13: 1, 14: 1 } })); // down + left
    r.poll();
    expect(captured(r.capture)).toMatchObject({ kind: 'hat', dir: 'down' });
  });
});

describe('Capture: axes', () => {
  it('takes an axis moved more than half its travel from where it rested', () => {
    const r = rig();
    r.pads.set(0, fakePad());
    r.begin();
    r.pads.set(0, fakePad({ axes: { 2: CAPTURE_AXIS_DELTA - 0.05 } }));
    r.poll();
    expect(r.capture.state.state).toBe('waiting');
    r.pads.set(0, fakePad({ axes: { 2: CAPTURE_AXIS_DELTA + 0.1 } }));
    r.poll();
    expect(captured(r.capture)).toEqual({ kind: 'pad-axis', device: 'pad', index: 2 });
  });

  it('measures from rest, so a lever resting at 0.8 needs to move, not to be there', () => {
    const r = rig();
    r.pads.set(0, fakeStick({ axes: { 2: 0.8 } }));
    r.begin();
    r.pads.set(0, fakeStick({ axes: { 2: 0.9 } }));
    r.poll();
    expect(r.capture.state.state).toBe('waiting');
    r.pads.set(0, fakeStick({ axes: { 2: -0.2 } }));
    r.poll();
    expect(captured(r.capture)).toMatchObject({ kind: 'pad-axis', index: 2 });
  });

  it('names the half moved when asked for one', () => {
    const r = rig();
    r.pads.set(0, fakePad());
    r.begin('half');
    r.pads.set(0, fakePad({ axes: { 1: -0.9 } }));
    r.poll();
    expect(captured(r.capture)).toEqual({
      kind: 'pad-axis',
      device: 'pad',
      index: 1,
      half: 'negative',
    });
    r.capture.start(r.hub, r.now(), 'half');
    r.poll(0);
    r.poll(SETTLE_MS + 1);
    r.pads.set(0, fakePad({ axes: { 1: 0.9 } }));
    r.poll();
    expect(captured(r.capture)).toMatchObject({ half: 'positive' });
  });

  it('ignores an axis that wanders while nothing is touched (a ghost axis)', () => {
    const r = rig();
    r.pads.set(0, fakeStick());
    r.hub.poll();
    r.capture.start(r.hub, r.now());
    // Axis 4 swings 0.6 on its own during the settle window.
    for (let i = 0; i < 6; i++) {
      r.pads.set(0, fakeStick({ axes: { 4: i % 2 === 0 ? -0.3 : 0.3 } }));
      r.poll(20);
    }
    r.poll(SETTLE_MS);
    expect(r.capture.ghosts()).toEqual([[0, 4]]);
    r.pads.set(0, fakeStick({ axes: { 4: 0.9 } }));
    r.poll();
    expect(r.capture.state.state).toBe('waiting');
    // A real axis still works.
    r.pads.set(0, fakeStick({ axes: { 1: 0.9 } }));
    r.poll();
    expect(captured(r.capture)).toMatchObject({ kind: 'pad-axis', index: 1 });
  });

  it('accepts nothing from the pads while it settles', () => {
    const r = rig();
    r.pads.set(0, fakePad());
    r.hub.poll();
    r.capture.start(r.hub, 0);
    r.pads.set(0, fakePad({ buttons: { 0: 1 } }));
    r.poll(10); // still inside the settle window
    expect(r.capture.state.state).toBe('waiting');
  });

  it('works with no pads at all', () => {
    const r = rig();
    r.begin();
    expect(r.capture.state.state).toBe('waiting');
    r.capture.keyDown('KeyQ');
    expect(captured(r.capture)).toEqual({ kind: 'key', code: 'KeyQ' });
  });
});

describe('Capture: a device the profile has never seen is bound end to end (AC-58.5)', () => {
  it('captures one control per action and the result has no conflicts', () => {
    const r = rig();
    // An unknown raw device: five axes, twelve buttons, a hat on axis 9.
    const stick = (spec: Parameters<typeof fakeStick>[0] = {}) =>
      fakeStick({ id: 'No-name Stick', ...spec });
    r.pads.set(0, stick());

    const plan: [ActionId, 'whole' | 'half', () => void, Source][] = [
      [
        'flight.roll',
        'whole',
        () => {
          r.pads.set(0, stick({ axes: { 0: 1 } }));
        },
        { kind: 'pad-axis', device: 'joy', index: 0 },
      ],
      [
        'flight.pitch',
        'whole',
        () => {
          r.pads.set(0, stick({ axes: { 1: -1 } }));
        },
        { kind: 'pad-axis', device: 'joy', index: 1 },
      ],
      [
        'flight.throttle',
        'whole',
        () => {
          r.pads.set(0, stick({ axes: { 2: 1 } }));
        },
        { kind: 'pad-axis', device: 'joy', index: 2 },
      ],
      [
        'flight.yaw',
        'whole',
        () => {
          r.pads.set(0, stick({ axes: { 3: -1 } }));
        },
        { kind: 'pad-axis', device: 'joy', index: 3 },
      ],
      [
        'flight.throttle_up',
        'half',
        () => {
          r.pads.set(0, stick({ axes: { 4: -1 } }));
        },
        { kind: 'pad-axis', device: 'joy', index: 4, half: 'negative' },
      ],
      [
        'flight.throttle_down',
        'half',
        () => {
          r.pads.set(0, stick({ axes: { 4: 1 } }));
        },
        { kind: 'pad-axis', device: 'joy', index: 4, half: 'positive' },
      ],
      [
        'flight.gear',
        'whole',
        () => {
          r.pads.set(0, stick({ buttons: { 0: 1 } }));
        },
        { kind: 'pad-button', device: 'joy', index: 0 },
      ],
      [
        'flight.brakes',
        'whole',
        () => {
          r.pads.set(0, stick({ buttons: { 1: 1 } }));
        },
        { kind: 'pad-button', device: 'joy', index: 1 },
      ],
      [
        'flight.airbrake',
        'whole',
        () => {
          r.pads.set(0, stick({ buttons: { 11: 1 } }));
        },
        { kind: 'pad-button', device: 'joy', index: 11 },
      ],
      [
        'flight.trim_nose_down',
        'whole',
        () => {
          r.pads.set(0, stick({ axes: { 9: -1 } }));
        },
        { kind: 'hat', device: 'joy', hat: 0, dir: 'up' },
      ],
      [
        'flight.trim_nose_up',
        'whole',
        () => {
          r.pads.set(0, stick({ axes: { 9: 0.1428571 } }));
        },
        { kind: 'hat', device: 'joy', hat: 0, dir: 'down' },
      ],
    ];

    const bindings: Binding[] = [];
    for (const [action, axis, act, expected] of plan) {
      r.pads.set(0, stick());
      r.hub.poll();
      r.capture.start(r.hub, r.now(), axis);
      r.poll(0);
      r.poll(SETTLE_MS + 1);
      act();
      r.poll();
      const got = captured(r.capture);
      expect(got, action).toEqual(expected);
      if (got !== undefined) bindings.push({ action, source: got });
      r.capture.reset();
    }
    expect(bindings).toHaveLength(plan.length);

    const profile: ControlsProfile = { ...builtinProfile('generic-joystick'), bindings };
    expect(findConflicts(profile)).toEqual([]);
    expect(new Set(bindings.map((b) => sourceKey(b.source))).size).toBe(bindings.length);
    expect(HAT_CENTRED).toBeGreaterThan(1);
  });

  it('and a pump flies the aircraft with what was captured', () => {
    const r = rig();
    const stick = (spec: Parameters<typeof fakeStick>[0] = {}) =>
      fakeStick({ id: 'No-name Stick', ...spec });
    r.pads.set(0, stick());
    const grab = (axis: 'whole' | 'half', act: () => void): Source => {
      r.pads.set(0, stick());
      r.hub.poll();
      r.capture.start(r.hub, r.now(), axis);
      r.poll(0);
      r.poll(SETTLE_MS + 1);
      act();
      r.poll();
      const got = captured(r.capture);
      r.capture.reset();
      if (got === undefined) throw new Error('nothing captured');
      return got;
    };
    const roll = grab('whole', () => {
      r.pads.set(0, stick({ axes: { 0: 1 } }));
    });
    const pitch = grab('whole', () => {
      r.pads.set(0, stick({ axes: { 1: 1 } }));
    });
    const lever = grab('whole', () => {
      r.pads.set(0, stick({ axes: { 2: 1 } }));
    });
    const gear = grab('whole', () => {
      r.pads.set(0, stick({ buttons: { 3: 1 } }));
    });
    const profile: ControlsProfile = {
      ...builtinProfile('generic-joystick'),
      bindings: [
        { action: 'flight.roll', source: roll },
        { action: 'flight.pitch', source: pitch },
        { action: 'flight.throttle', source: lever },
        { action: 'flight.gear', source: gear },
      ],
    };
    const frames: InputFrame[] = [];
    let mono = 0;
    const pump = new InputPump({
      sink: {
        send: (b) => {
          frames.push(decodeInputFrame(b));
          return true;
        },
        close: () => undefined,
      },
      keyboard: new KeyboardState(),
      pads: r.hub,
      profile,
      nowMs: () => mono,
      sharedNowMs: () => mono,
    });
    r.pads.set(0, stick({ axes: { 0: 0.5, 1: -0.5, 2: 1 }, buttons: { 3: 1 } }));
    for (let i = 0; i < 5; i++) {
      mono += 4;
      pump.update();
    }
    const f = frames.at(-1);
    expect(f?.axes[AXIS.ROLL] ?? 0).toBeGreaterThan(0.2);
    expect(f?.axes[AXIS.PITCH] ?? 0).toBeLessThan(-0.2);
    expect(f?.axes[AXIS.THROTTLE]).toBe(1); // the lever at the end of the travel it was captured at
    expect(f?.buttons).toBe(1n << BigInt(BUTTON.GEAR));
  });
});
