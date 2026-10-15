/**
 * The input pump: reads the keyboard and the gamepad, runs the shaping pipeline, and writes one
 * `InputFrame` to the input channel (GDD 58.2).
 *
 * `update()` is called from key events, every animation frame, and a 4 ms timer while the tab has
 * focus, so a change reaches the channel within a few milliseconds whatever the display rate. It
 * writes a frame when something changed, and at least every `minIntervalMs` otherwise (8 ms
 * through the shared ring, 33 ms when frames go by `postMessage`), so the Sim Worker always has a
 * recent sample and a fresh timestamp to measure latency from.
 *
 * When focus is lost or the tab is hidden, `neutral()` sends one frame with the NEUTRAL flag: the
 * core zeroes the sticks and holds the throttle where it was.
 */
import {
  AXIS,
  BUTTON,
  encodeInputFrame,
  INPUT_FLAG,
  INPUT_FRAME,
  neutralFrame,
  type InputFrame,
  type InputSink,
} from '@flying/sim-bridge';
import { type Bindings, DEFAULT_BINDINGS } from './bindings.ts';
import { type GamepadSample, type GamepadReader } from './gamepad.ts';
import { AxisEmulator, type KeyboardState, ThrottleLever } from './keyboard.ts';
import { AxisShaper, calibrate, shapeStatic } from './shaping.ts';

export interface PumpDeps {
  sink: InputSink;
  keyboard: KeyboardState;
  gamepad: Pick<GamepadReader, 'poll'>;
  bindings?: Bindings;
  /** Monotonic milliseconds: time steps. */
  nowMs: () => number;
  /** Milliseconds on the shared clock: the timestamp written into each frame. */
  sharedNowMs: () => number;
  /** Longest gap between frames when nothing changes. */
  minIntervalMs?: number;
}

export interface PumpStats {
  /** Frames handed to the channel. */
  sent: number;
  /** Frames the channel refused (its queue was full). */
  refused: number;
  /** Frames sent only because the heartbeat interval passed. */
  heartbeats: number;
  neutralFrames: number;
}

/** Steps longer than this (a stalled tab) are treated as this long, so nothing jumps. */
const MAX_STEP_MS = 100;
/** Axis changes smaller than this do not count as a change. */
const EPSILON = 1e-5;

export class InputPump {
  readonly stats: PumpStats = { sent: 0, refused: 0, heartbeats: 0, neutralFrames: 0 };
  readonly #d: PumpDeps;
  readonly #b: Bindings;
  readonly #minInterval: number;
  readonly #pitchKeys: AxisEmulator;
  readonly #rollKeys: AxisEmulator;
  readonly #yawKeys: AxisEmulator;
  readonly #pitchPad: AxisShaper;
  readonly #rollPad: AxisShaper;
  readonly #yawPad: AxisShaper;
  readonly #lever = new ThrottleLever(0.5);
  readonly #frame: InputFrame = neutralFrame(0);
  readonly #bytes = new Uint8Array(INPUT_FRAME.SIZE);
  readonly #lastSent = { pitch: 0, roll: 0, yaw: 0, throttle: Number.NaN, buttons: 0n };
  #seq = 0;
  #lastUpdate = Number.NaN;
  #lastSentAt = Number.NEGATIVE_INFINITY;
  #padWasThere = false;

  constructor(deps: PumpDeps) {
    this.#d = deps;
    this.#b = deps.bindings ?? DEFAULT_BINDINGS;
    this.#minInterval = deps.minIntervalMs ?? 8;
    this.#pitchKeys = new AxisEmulator(this.#b.keyboardAxis);
    this.#rollKeys = new AxisEmulator(this.#b.keyboardAxis);
    this.#yawKeys = new AxisEmulator(this.#b.keyboardAxis);
    this.#pitchPad = new AxisShaper(this.#b.gamepad.pitch.shape);
    this.#rollPad = new AxisShaper(this.#b.gamepad.roll.shape);
    this.#yawPad = new AxisShaper(this.#b.gamepad.yaw.shape);
  }

  /** The virtual throttle lever, 0 to 1. */
  get throttle(): number {
    return this.#lever.value;
  }

  /** Samples the devices and writes a frame if one is due. Returns whether it wrote one. */
  update(): boolean {
    const d = this.#d;
    const now = d.nowMs();
    const dt = Number.isNaN(this.#lastUpdate)
      ? 0
      : Math.min(Math.max(now - this.#lastUpdate, 0), MAX_STEP_MS);
    this.#lastUpdate = now;

    const kb = d.keyboard;
    const k = this.#b.keyboard;
    const pitchK = this.#pitchKeys.update(
      dt,
      kb.anyDown(k.pitch.negative),
      kb.anyDown(k.pitch.positive),
    );
    const rollK = this.#rollKeys.update(
      dt,
      kb.anyDown(k.roll.negative),
      kb.anyDown(k.roll.positive),
    );
    const yawK = this.#yawKeys.update(dt, kb.anyDown(k.yaw.negative), kb.anyDown(k.yaw.positive));
    const throttleKeyRate =
      this.#b.throttleKeyRatePerSecond *
      (Number(kb.anyDown(k.throttleUp)) - Number(kb.anyDown(k.throttleDown)));

    const pad = d.gamepad.poll();
    let pitchP = 0;
    let rollP = 0;
    let yawP = 0;
    let throttlePadRate = 0;
    let padReset = false;
    let padTrimUp = false;
    let padTrimDown = false;
    if (pad) {
      const g = this.#b.gamepad;
      pitchP = this.#pitchPad.step(calibrate(pad.axes[g.pitch.axis] ?? 0), dt);
      rollP = this.#rollPad.step(calibrate(pad.axes[g.roll.axis] ?? 0), dt);
      yawP = this.#yawPad.step(
        (pad.buttons[g.yaw.positiveButton] ?? 0) - (pad.buttons[g.yaw.negativeButton] ?? 0),
        dt,
      );
      const lever = shapeStatic(calibrate(pad.axes[g.throttle.axis] ?? 0), g.throttle.shape);
      throttlePadRate = (g.throttle.invert ? -lever : lever) * g.throttle.ratePerSecond;
      padReset = (pad.buttons[g.reset.button] ?? 0) > 0.5;
      padTrimUp = (pad.buttons[g.trimNoseUp.button] ?? 0) > 0.5;
      padTrimDown = (pad.buttons[g.trimNoseDown.button] ?? 0) > 0.5;
      this.#padWasThere = true;
    } else if (this.#padWasThere) {
      // Unplugged mid-flight: drop the old readings rather than hold the last stick position.
      this.#pitchPad.reset();
      this.#rollPad.reset();
      this.#yawPad.reset();
      this.#padWasThere = false;
    }

    const throttle = this.#lever.update(dt, throttleKeyRate + throttlePadRate);
    const pitch = larger(pitchK, pitchP);
    const roll = larger(rollK, rollP);
    const yaw = larger(yawK, yawP);
    const bit = (on: boolean, b: number): bigint => (on ? 1n << BigInt(b) : 0n);
    const buttons =
      bit(kb.anyDown(k.reset) || padReset, BUTTON.RESET) |
      bit(kb.anyDown(k.trimNoseUp) || padTrimUp, BUTTON.TRIM_NOSE_UP) |
      bit(kb.anyDown(k.trimNoseDown) || padTrimDown, BUTTON.TRIM_NOSE_DOWN);
    return this.#maybeSend(now, { pitch, roll, yaw, throttle, buttons }, 0);
  }

  /**
   * Sends one neutral frame (focus lost, tab hidden) and forgets the emulated and smoothed state,
   * so nothing stale is replayed when control returns. The throttle lever keeps its setting.
   */
  neutral(): void {
    this.#pitchKeys.reset();
    this.#rollKeys.reset();
    this.#yawKeys.reset();
    this.#pitchPad.reset();
    this.#rollPad.reset();
    this.#yawPad.reset();
    this.#lastUpdate = Number.NaN;
    this.stats.neutralFrames += 1;
    this.#maybeSend(
      this.#d.nowMs(),
      { pitch: 0, roll: 0, yaw: 0, throttle: this.#lever.value, buttons: 0n },
      INPUT_FLAG.NEUTRAL,
      true,
    );
  }

  #maybeSend(
    now: number,
    v: { pitch: number; roll: number; yaw: number; throttle: number; buttons: bigint },
    flags: number,
    force = false,
  ): boolean {
    const last = this.#lastSent;
    const changed =
      Math.abs(v.pitch - last.pitch) > EPSILON ||
      Math.abs(v.roll - last.roll) > EPSILON ||
      Math.abs(v.yaw - last.yaw) > EPSILON ||
      !(Math.abs(v.throttle - last.throttle) <= EPSILON) || // also true while last is NaN
      v.buttons !== last.buttons;
    const due = now - this.#lastSentAt >= this.#minInterval;
    if (!force && !changed && !due) {
      return false;
    }
    if (!force && !changed) {
      this.stats.heartbeats += 1;
    }

    const f = this.#frame;
    this.#seq = (this.#seq + 1) >>> 0;
    f.seq = this.#seq;
    f.flags = flags;
    f.tMs = this.#d.sharedNowMs();
    f.axes[AXIS.PITCH] = v.pitch + 0;
    f.axes[AXIS.ROLL] = v.roll + 0;
    f.axes[AXIS.YAW] = v.yaw + 0;
    f.axes[AXIS.THROTTLE] = v.throttle;
    f.buttons = v.buttons;
    encodeInputFrame(f, this.#bytes);
    if (this.#d.sink.send(this.#bytes)) {
      this.stats.sent += 1;
    } else {
      this.stats.refused += 1;
    }
    last.pitch = v.pitch;
    last.roll = v.roll;
    last.yaw = v.yaw;
    last.throttle = v.throttle;
    last.buttons = v.buttons;
    this.#lastSentAt = now;
    return true;
  }
}

/** Whichever of two sources is pushed harder wins (the keyboard and a pad can both be in use). */
function larger(a: number, b: number): number {
  return Math.abs(a) >= Math.abs(b) ? a : b;
}

export type { GamepadSample };
