import { describe, expect, it } from 'vitest';
import {
  MouseDevice,
  type MouseDocumentLike,
  type MouseOptions,
  type MouseTargetLike,
  type MouseWarning,
} from './mouse.ts';

class FakeTarget extends EventTarget implements MouseTargetLike {
  requests: ({ unadjustedMovement?: boolean } | undefined)[] = [];
  /** What the next unadjusted request does: resolve, reject, or throw. */
  unadjusted: 'resolve' | 'reject' | 'throw' | 'undefined' = 'resolve';
  requestPointerLock(options?: { unadjustedMovement?: boolean }): Promise<void> | undefined {
    this.requests.push(options);
    if (options?.unadjustedMovement !== true) return Promise.resolve();
    if (this.unadjusted === 'throw') throw new Error('NotSupportedError');
    if (this.unadjusted === 'reject') return Promise.reject(new Error('NotSupportedError'));
    if (this.unadjusted === 'undefined') return undefined;
    return Promise.resolve();
  }
}

class FakeDocument extends EventTarget implements MouseDocumentLike {
  pointerLockElement: unknown = null;
  exits = 0;
  exitPointerLock(): void {
    this.exits += 1;
  }
}

function event(type: string, props: Record<string, unknown>): Event {
  const e = new Event(type, { cancelable: true });
  for (const [k, v] of Object.entries(props)) Object.defineProperty(e, k, { value: v });
  return e;
}

function rig(extra: Partial<MouseOptions> = {}) {
  const target = new FakeTarget();
  const doc = new FakeDocument();
  const events: number[] = [];
  const warnings: MouseWarning[] = [];
  const mouse = new MouseDevice({
    target,
    document: doc,
    onEvent: (t) => events.push(t),
    onWarn: (w) => warnings.push(w),
    ...extra,
  });
  const detach = mouse.attach();
  const lock = (): void => {
    doc.pointerLockElement = target;
    doc.dispatchEvent(new Event('pointerlockchange'));
  };
  const unlock = (): void => {
    doc.pointerLockElement = null;
    doc.dispatchEvent(new Event('pointerlockchange'));
  };
  const move = (dx: number, dy: number, t = 1) =>
    target.dispatchEvent(event('pointermove', { movementX: dx, movementY: dy, timeStamp: t }));
  return { target, doc, mouse, events, warnings, detach, lock, unlock, move };
}

describe('MouseDevice', () => {
  it('ignores motion until the pointer is locked', () => {
    const r = rig();
    r.move(5, 5);
    r.mouse.consume();
    expect([r.mouse.dx, r.mouse.dy]).toEqual([0, 0]);
    expect(r.mouse.locked).toBe(false);
    r.lock();
    expect(r.mouse.locked).toBe(true);
    r.move(5, -3);
    r.mouse.consume();
    expect([r.mouse.dx, r.mouse.dy]).toEqual([5, -3]);
  });

  it('adds up the motion between consumes and hands it over once', () => {
    const r = rig();
    r.lock();
    r.move(1, 2);
    r.move(3, 4);
    r.mouse.consume();
    expect([r.mouse.dx, r.mouse.dy]).toEqual([4, 6]);
    r.mouse.consume();
    expect([r.mouse.dx, r.mouse.dy]).toEqual([0, 0]);
  });

  it('adds up the coalesced events inside one pointermove', () => {
    const r = rig();
    r.lock();
    r.target.dispatchEvent(
      event('pointermove', {
        movementX: 3,
        movementY: 0,
        timeStamp: 1,
        getCoalescedEvents: () => [
          { movementX: 1, movementY: 1 },
          { movementX: 1, movementY: 1 },
          { movementX: 1, movementY: -2 },
        ],
      }),
    );
    r.mouse.consume();
    expect([r.mouse.dx, r.mouse.dy]).toEqual([3, 0]);
  });

  it('tells the pump when a locked event happened', () => {
    const r = rig();
    r.move(1, 1, 5);
    expect(r.events).toEqual([]); // not locked: not a control
    r.lock();
    r.move(1, 1, 7);
    r.target.dispatchEvent(event('pointerdown', { button: 0, timeStamp: 9 }));
    expect(r.events).toEqual([7, 9]);
  });

  it('tracks buttons while locked and lets go when the lock ends', () => {
    const r = rig();
    r.target.dispatchEvent(event('pointerdown', { button: 0, timeStamp: 1 }));
    expect(r.mouse.isDown(0)).toBe(false); // the click that asks for the lock is not a control
    r.lock();
    r.target.dispatchEvent(event('pointerdown', { button: 0, timeStamp: 2 }));
    r.target.dispatchEvent(event('pointerdown', { button: 2, timeStamp: 3 }));
    expect([r.mouse.isDown(0), r.mouse.isDown(1), r.mouse.isDown(2)]).toEqual([true, false, true]);
    r.target.dispatchEvent(event('pointerup', { button: 0, timeStamp: 4 }));
    expect(r.mouse.isDown(0)).toBe(false);
    r.unlock();
    expect(r.mouse.isDown(2)).toBe(false);
  });

  it('counts wheel notches of 100 px, with fractions carried over, and claims the wheel', () => {
    const r = rig();
    r.lock();
    const wheel = (deltaY: number, deltaMode = 0) => {
      const e = event('wheel', { deltaY, deltaMode, timeStamp: 1 });
      r.target.dispatchEvent(e);
      return e;
    };
    expect(wheel(-100).defaultPrevented).toBe(true);
    wheel(-100);
    wheel(100);
    r.mouse.consume();
    expect([r.mouse.wheelUp, r.mouse.wheelDown]).toEqual([2, 1]);
    // A touchpad's small steps add up.
    for (let i = 0; i < 10; i++) wheel(10);
    r.mouse.consume();
    expect([r.mouse.wheelUp, r.mouse.wheelDown]).toEqual([0, 1]);
    // Firefox reports lines: three lines is one notch.
    wheel(3, 1);
    r.mouse.consume();
    expect(r.mouse.wheelDown).toBe(1);
  });

  it('leaves the wheel alone while unlocked', () => {
    const r = rig();
    const e = event('wheel', { deltaY: 100, timeStamp: 1 });
    r.target.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(false);
  });

  it('suppresses the context menu on its target', () => {
    const r = rig();
    const e = event('contextmenu', {});
    r.target.dispatchEvent(e);
    expect(e.defaultPrevented).toBe(true);
  });

  it('asks for unadjusted movement first', async () => {
    const r = rig();
    r.mouse.lock();
    await Promise.resolve();
    expect(r.target.requests).toEqual([{ unadjustedMovement: true }]);
    expect(r.warnings).toEqual([]);
  });

  it('falls back to a plain lock where unadjusted movement is refused, and says so once', async () => {
    const r = rig();
    r.target.unadjusted = 'reject';
    r.mouse.lock();
    await new Promise((res) => setTimeout(res, 0));
    expect(r.target.requests).toEqual([{ unadjustedMovement: true }, undefined]);
    expect(r.warnings).toEqual(['unadjusted-unavailable']);
    r.mouse.lock();
    await new Promise((res) => setTimeout(res, 0));
    expect(r.warnings).toEqual(['unadjusted-unavailable']);
  });

  it('falls back the same way when the request throws', () => {
    const r = rig();
    r.target.unadjusted = 'throw';
    r.mouse.lock();
    expect(r.target.requests).toEqual([{ unadjustedMovement: true }, undefined]);
    expect(r.warnings).toEqual(['unadjusted-unavailable']);
  });

  it('asks the document to give the pointer back', () => {
    const r = rig();
    r.mouse.unlock();
    expect(r.doc.exits).toBe(1);
  });

  it('stops listening when detached', () => {
    const r = rig();
    r.lock();
    r.detach();
    r.move(9, 9);
    r.mouse.consume();
    expect(r.mouse.dx).toBe(0);
  });
});
