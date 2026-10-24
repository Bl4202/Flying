import { describe, expect, it } from 'vitest';
import {
  encodeInputFrame,
  INPUT_FLAG,
  decodeInputFrame,
  INPUT_FRAME,
  type InputFrame,
  type InputSink,
} from '@flying/sim-bridge';
import { attachInput, type KeyEventLike, type Scheduler } from './dom.ts';
import { KeyboardState } from './keyboard.ts';
import { builtinProfile } from './profile.ts';
import { InputPump } from './pump.ts';

class FakeDocument extends EventTarget {
  visibilityState = 'visible';
}

function key(
  type: 'keydown' | 'keyup',
  code: string,
  extra: {
    repeat?: boolean;
    target?: unknown;
    timeStamp?: number;
    ctrl?: boolean;
    shift?: boolean;
  } = {},
) {
  const e = new Event(type, { cancelable: true });
  Object.defineProperty(e, 'code', { value: code });
  Object.defineProperty(e, 'repeat', { value: extra.repeat ?? false });
  Object.defineProperty(e, 'ctrlKey', { value: extra.ctrl ?? false });
  Object.defineProperty(e, 'shiftKey', { value: extra.shift ?? false });
  if (extra.timeStamp !== undefined)
    Object.defineProperty(e, 'timeStamp', { value: extra.timeStamp });
  if (extra.target !== undefined) Object.defineProperty(e, 'target', { value: extra.target });
  return e;
}

function rig(opts: { hasFocus?: boolean; swallow?: (e: KeyEventLike) => boolean } = {}) {
  const frames: InputFrame[] = [];
  const sink: InputSink = {
    send: (b) => {
      frames.push(decodeInputFrame(b));
      return true;
    },
    close: () => undefined,
  };
  let mono = 0;
  const keyboard = new KeyboardState();
  const pump = new InputPump({
    sink,
    keyboard,
    nowMs: () => mono,
    sharedNowMs: () => mono,
  });
  const win = new EventTarget();
  const doc = new FakeDocument();
  const timers = new Map<number, { cb: () => void; ms: number }>();
  const frameCbs = new Map<number, () => void>();
  let next = 1;
  const scheduler: Scheduler = {
    requestFrame: (cb) => {
      frameCbs.set(next, cb);
      return next++;
    },
    cancelFrame: (id) => {
      frameCbs.delete(id);
    },
    setInterval: (cb, ms) => {
      timers.set(next, { cb, ms });
      return next++;
    },
    clearInterval: (id) => {
      timers.delete(id);
    },
  };
  const detach = attachInput(
    { window: win, document: doc },
    {
      keyboard,
      pump,
      scheduler,
      ...(opts.hasFocus !== undefined ? { hasFocus: opts.hasFocus } : {}),
      ...(opts.swallow !== undefined ? { swallow: opts.swallow } : {}),
    },
  );
  return {
    win,
    doc,
    keyboard,
    pump,
    frames,
    timers,
    frameCbs,
    detach,
    advance: (ms: number) => {
      mono += ms;
    },
    now: () => mono,
  };
}

describe('attachInput', () => {
  it('feeds key presses to the keyboard, claims bound keys and writes a frame at once', () => {
    const r = rig();
    const down = key('keydown', 'KeyD');
    r.win.dispatchEvent(down);
    expect(r.keyboard.isDown('KeyD')).toBe(true);
    expect(down.defaultPrevented).toBe(true);
    expect(r.frames.length).toBeGreaterThanOrEqual(1);
    r.win.dispatchEvent(key('keyup', 'KeyD'));
    expect(r.keyboard.isDown('KeyD')).toBe(false);
  });

  it('leaves keys it does not use to the browser', () => {
    const r = rig();
    const f5 = key('keydown', 'F5');
    r.win.dispatchEvent(f5);
    expect(f5.defaultPrevented).toBe(false);
    expect(r.keyboard.isDown('F5')).toBe(true); // tracked, harmlessly
  });

  it('ignores auto-repeat', () => {
    const r = rig();
    r.win.dispatchEvent(key('keydown', 'KeyW'));
    const sent = r.frames.length;
    r.advance(40);
    r.win.dispatchEvent(key('keydown', 'KeyW', { repeat: true }));
    expect(r.frames.length).toBe(sent);
  });

  it('does not take keys typed into a text field', () => {
    const r = rig();
    const typed = key('keydown', 'KeyW', { target: { tagName: 'INPUT' } });
    r.win.dispatchEvent(typed);
    expect(typed.defaultPrevented).toBe(false);
    expect(r.keyboard.isDown('KeyW')).toBe(false);
  });

  it('samples on every animation frame and on a 4 ms timer while focused', () => {
    const r = rig();
    expect(r.frameCbs.size).toBe(1);
    expect([...r.timers.values()].map((t) => t.ms)).toEqual([4]);
    r.advance(10);
    [...r.frameCbs.values()][0]?.();
    expect(r.frames.length).toBeGreaterThanOrEqual(1);
  });

  it('on blur: releases every key, sends one neutral frame, and stops sampling', () => {
    const r = rig();
    r.win.dispatchEvent(key('keydown', 'KeyD'));
    r.win.dispatchEvent(new Event('blur'));
    expect(r.keyboard.count).toBe(0);
    expect(r.frames.at(-1)?.flags).toBe(INPUT_FLAG.NEUTRAL);
    expect(r.frameCbs.size).toBe(0);
    expect(r.timers.size).toBe(0);
    const sent = r.frames.length;
    r.win.dispatchEvent(new Event('blur')); // a second blur does nothing more
    expect(r.frames.length).toBe(sent);
  });

  it('on focus: starts sampling again', () => {
    const r = rig();
    r.win.dispatchEvent(new Event('blur'));
    r.win.dispatchEvent(new Event('focus'));
    expect(r.frameCbs.size).toBe(1);
    expect(r.timers.size).toBe(1);
  });

  it('treats a hidden tab like lost focus, and a visible one like regained focus', () => {
    const r = rig();
    r.doc.visibilityState = 'hidden';
    r.doc.dispatchEvent(new Event('visibilitychange'));
    expect(r.frames.at(-1)?.flags).toBe(INPUT_FLAG.NEUTRAL);
    expect(r.timers.size).toBe(0);
    r.doc.visibilityState = 'visible';
    r.doc.dispatchEvent(new Event('visibilitychange'));
    expect(r.timers.size).toBe(1);
  });

  it('does not sample before it has focus', () => {
    const r = rig({ hasFocus: false });
    expect(r.frameCbs.size).toBe(0);
    expect(r.timers.size).toBe(0);
    r.win.dispatchEvent(new Event('focus'));
    expect(r.timers.size).toBe(1);
  });

  it('detaches everything', () => {
    const r = rig();
    r.detach();
    expect(r.frameCbs.size + r.timers.size).toBe(0);
    r.win.dispatchEvent(key('keydown', 'KeyD'));
    expect(r.keyboard.isDown('KeyD')).toBe(false);
  });

  it('writes frames the sim can decode (round trip through the codec)', () => {
    const r = rig();
    r.win.dispatchEvent(key('keydown', 'KeyS'));
    const bytes = new Uint8Array(INPUT_FRAME.SIZE);
    const f = r.frames.at(-1);
    if (!f) throw new Error('no frame');
    encodeInputFrame(f, bytes);
    expect(decodeInputFrame(bytes)).toEqual(f);
  });

  it('tells the pump when a key event happened, so the frame is dated by the event', () => {
    const r = rig();
    r.advance(100);
    r.win.dispatchEvent(key('keydown', 'KeyD', { timeStamp: r.now() - 4 }));
    const f = r.frames.at(-1);
    expect(f?.flags).toBe(INPUT_FLAG.CHANGED);
    expect(f?.tSentMs).toBe(r.now());
    expect(f?.tMs).toBe(r.now() - 4);
    r.advance(30);
    r.win.dispatchEvent(key('keyup', 'KeyD', { timeStamp: r.now() - 2 }));
    expect(r.frames.at(-1)?.tMs).toBe(r.now() - 2);
  });

  it('does not date a frame by an auto-repeat', () => {
    const r = rig();
    r.win.dispatchEvent(key('keydown', 'KeyW', { timeStamp: 1 }));
    r.advance(40);
    const sent = r.frames.length;
    r.win.dispatchEvent(key('keydown', 'KeyW', { repeat: true, timeStamp: r.now() }));
    expect(r.frames.length).toBe(sent);
  });

  it('claims the keys of the profile in use, and follows a change of profile', () => {
    const r = rig();
    const f8 = key('keydown', 'F8');
    r.win.dispatchEvent(f8);
    expect(f8.defaultPrevented).toBe(true); // the assist key is bound
    const profile = builtinProfile('default-km');
    profile.bindings = profile.bindings.filter(
      (b) => !(b.source.kind === 'key' && b.source.code === 'F8'),
    );
    r.pump.setProfile(profile);
    const again = key('keydown', 'F9');
    r.win.dispatchEvent(again);
    expect(again.defaultPrevented).toBe(false);
    const gone = key('keydown', 'F8');
    r.win.dispatchEvent(gone);
    expect(gone.defaultPrevented).toBe(false);
  });

  it('leaves the keys the page keeps for itself (the debug chord) out of the flight controls', () => {
    const chord = (e: KeyEventLike): boolean =>
      e.code === 'KeyD' && e.ctrlKey === true && e.shiftKey === true;
    const r = rig({ swallow: chord });
    r.win.dispatchEvent(key('keydown', 'KeyD'));
    expect(r.keyboard.isDown('KeyD')).toBe(true);
    r.win.dispatchEvent(key('keyup', 'KeyD'));
    const swallowed = key('keydown', 'KeyD', { ctrl: true, shift: true });
    r.win.dispatchEvent(swallowed);
    expect(r.keyboard.isDown('KeyD')).toBe(false);
    expect(swallowed.defaultPrevented).toBe(false); // the page's own handler decides that
  });

  it('samples at once when a gamepad is connected or disconnected', () => {
    const r = rig();
    const before = r.frames.length;
    r.advance(10);
    r.win.dispatchEvent(new Event('gamepadconnected'));
    expect(r.frames.length).toBeGreaterThan(before);
    const mid = r.frames.length;
    r.advance(10);
    r.win.dispatchEvent(new Event('gamepaddisconnected'));
    expect(r.frames.length).toBeGreaterThan(mid);
  });
});
