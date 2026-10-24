/**
 * The pump's garbage, measured. Its own file because the measurement is of optimised code, and the
 * other pump tests run the same code against dozens of profiles, sinks and clocks, which makes
 * V8 compile it for all of them and optimise it for none.
 */
import { describe, expect, it } from 'vitest';
import { GamepadHub } from './devices/gamepad.ts';
import { KeyboardState } from './keyboard.ts';
import { builtinProfile } from './profile.ts';
import { InputPump } from './pump.ts';
import { bytesPerCall, FakeMouse, FakePads, fakePad } from './test-support.ts';

describe('InputPump: garbage', () => {
  // `update()` runs every few milliseconds all flight long. It makes no objects, arrays, strings
  // or closures: its state is in typed arrays and its frame is written into one buffer. What it
  // can still make is heap numbers: V8 boxes a fractional number that crosses a call it does not
  // inline (16 bytes each, gone at the next young-generation collection), and a browser clock read
  // boxes its result before the pump sees it. So the test has three parts: a pump at rest, with
  // whole-number clocks, makes exactly nothing, which catches anything added to the path every
  // update takes; a pump under load, with fractional clocks, stays under a generous ceiling; and
  // the frame buffer is the one buffer.
  function loaded(options: { fractional: boolean; held: string[]; pad: boolean; aim: boolean }) {
    const profile = builtinProfile('gamepad-standard');
    const keyboard = new KeyboardState();
    const pads = new FakePads();
    const hub = new GamepadHub(pads.get);
    const mouse = new FakeMouse();
    const pad = fakePad({
      axes: { 0: 0.3, 1: -0.4, 2: 0.2, 3: -0.1 },
      buttons: { 6: 0.2, 7: 0.7 },
    });
    if (options.pad) pads.set(0, pad);
    const clock = new Float64Array(1);
    clock[0] = 1000;
    const tick = options.fractional ? 4.1 : 4;
    let sent = 0;
    const frames: Uint8Array[] = [];
    const pump = new InputPump({
      sink: {
        send: (bytes) => {
          sent += 1;
          if (frames.length < 3 && !frames.includes(bytes)) frames.push(bytes);
          return true;
        },
        close: () => undefined,
      },
      keyboard,
      pads: hub,
      mouse,
      profile,
      nowMs: () => clock[0] ?? 0,
      sharedNowMs: () => (clock[0] ?? 0) + 1_000_000,
    });
    pump.observe({ spawnSeq: 1, throttle: 0.3, assist: 2, noseAzRad: 0.2, noseElRad: 0.1 });
    if (options.aim) pump.setMouseMode('aim');
    for (const code of options.held) keyboard.keyDown(code, false);
    const axes = pad.axes as number[];
    const step = (i: number): void => {
      clock[0] = (clock[0] ?? 0) + tick;
      if (options.pad) {
        axes[2] = Math.sin(i / 50) * 0.8;
        axes[3] = Math.cos(i / 70) * 0.6;
      }
      if (options.aim) mouse.push(i % 7 === 0 ? 3 : -1, i % 11 === 0 ? 2 : 0);
      pump.update();
    };
    return { step, sent: () => sent, frames };
  }

  it('measures garbage, so the checks below mean something', async () => {
    let sink: unknown;
    const array = await bytesPerCall((i) => {
      sink = [i, i + 1];
    });
    const nothing = await bytesPerCall((i) => {
      sink = i & 1;
    });
    expect(sink).toBeDefined();
    expect(array).toBeGreaterThanOrEqual(16);
    expect(nothing).toBeLessThan(1);
  }, 30_000);

  it('makes none at rest', async () => {
    const r = loaded({ fractional: false, held: [], pad: false, aim: false });
    const bytes = await bytesPerCall(r.step);
    expect(r.sent()).toBeGreaterThan(1000);
    expect(bytes).toBeLessThan(1);
  }, 30_000);

  it('makes next to none with whole-number clocks and a key held', async () => {
    const r = loaded({ fractional: false, held: ['KeyD', 'KeyB'], pad: false, aim: false });
    const bytes = await bytesPerCall(r.step);
    expect(bytes).toBeLessThan(1);
  }, 30_000);

  it('stays under a ceiling with a pad, the mouse aim and four keys in play, on fractional clocks', async () => {
    const r = loaded({
      fractional: true,
      held: ['KeyD', 'KeyW', 'ShiftLeft', 'KeyB'],
      pad: true,
      aim: true,
    });
    const bytes = await bytesPerCall(r.step);
    expect(r.sent()).toBeGreaterThan(1000);
    expect(bytes).toBeLessThan(512);
  }, 30_000);

  it('hands the sink the same buffer every time', () => {
    const r = loaded({ fractional: false, held: ['KeyD'], pad: false, aim: false });
    for (let i = 0; i < 200; i++) r.step(i);
    expect(r.frames).toHaveLength(1);
  });
});
