/**
 * "Press the key you want": the capture state machine behind the binding editor (GDD 58.5).
 *
 * `start` begins listening; the editor then feeds it whatever happens (`keyDown`, `mouseButton`,
 * `wheel`, and `poll` for pads and joysticks) and reads `state`. The first usable input wins:
 *
 * - a key, a mouse button or the wheel;
 * - a pad button, or a hat direction (a hat wins over the d-pad buttons it also presses);
 * - an axis moved more than half its travel from where it rested when capture began. Axes that
 *   wander on their own when untouched ("ghost" axes: a loose pot, a floating trigger) are noticed
 *   during the first moments and ignored.
 *
 * Escape cancels. Nothing here runs in flight, so it favours clarity over avoiding allocation.
 */
import { HAT_BIT, MAX_AXES, MAX_PADS, type GamepadHub } from './devices/gamepad.ts';
import { type HatDirection, type Source } from './sources.ts';

/** How far from rest an axis must move to be taken as the one the person means. */
export const CAPTURE_AXIS_DELTA = 0.5;
/** An axis that moves this much untouched while capture settles is a ghost. */
export const GHOST_NOISE = 0.2;
/** How long capture watches the pads before it accepts an axis, milliseconds. */
export const SETTLE_MS = 300;

export type CaptureAxis =
  /** The whole axis: for pitch, roll, yaw and the absolute throttle. */
  | 'whole'
  /** One half of it: for buttons and rates (the stick pushed up is one binding, down another). */
  | 'half';

export type CaptureState =
  | { readonly state: 'idle' }
  | { readonly state: 'waiting' }
  | { readonly state: 'captured'; readonly source: Source }
  | { readonly state: 'cancelled' };

const HAT_ORDER: readonly HatDirection[] = ['up', 'right', 'down', 'left'];

export class Capture {
  #state: CaptureState = { state: 'idle' };
  #axis: CaptureAxis = 'whole';
  #startedAt = 0;
  #settled = false;
  readonly #rest = Array.from({ length: MAX_PADS }, () => new Float64Array(MAX_AXES));
  readonly #lo = Array.from({ length: MAX_PADS }, () => new Float64Array(MAX_AXES));
  readonly #hi = Array.from({ length: MAX_PADS }, () => new Float64Array(MAX_AXES));
  readonly #ghost = Array.from({ length: MAX_PADS }, () => new Uint8Array(MAX_AXES));
  /** Buttons and hats already down when capture began do not count until released. */
  readonly #held = Array.from({ length: MAX_PADS }, () => new Set<number>());

  get state(): CaptureState {
    return this.#state;
  }

  get active(): boolean {
    return this.#state.state === 'waiting';
  }

  /** The axes capture has decided are ghosts: `[slot, axis]` pairs. */
  ghosts(): [number, number][] {
    const out: [number, number][] = [];
    this.#ghost.forEach((row, s) => {
      row.forEach((g, a) => {
        if (g === 1) out.push([s, a]);
      });
    });
    return out;
  }

  /** Starts listening. `hub` lets pad inputs count; `nowMs` is the editor's clock. */
  start(hub: GamepadHub | undefined, nowMs: number, axis: CaptureAxis = 'whole'): void {
    this.#state = { state: 'waiting' };
    this.#axis = axis;
    this.#startedAt = nowMs;
    this.#settled = false;
    for (let s = 0; s < MAX_PADS; s++) {
      this.#ghost[s]?.fill(0);
      this.#held[s]?.clear();
      const slot = hub?.slots[s];
      for (let a = 0; a < MAX_AXES; a++) {
        const v = slot?.present === true ? (slot.axes[a] ?? 0) : 0;
        this.#rest[s]?.fill(v, a, a + 1);
        this.#lo[s]?.fill(v, a, a + 1);
        this.#hi[s]?.fill(v, a, a + 1);
      }
      if (slot?.present === true) {
        for (let b = 0; b < slot.buttonCount; b++) {
          if ((slot.buttons[b] ?? 0) > 0.5) this.#held[s]?.add(b);
        }
        for (let h = 0; h < slot.hatCount; h++) {
          if ((slot.hats[h] ?? 0) !== 0) this.#held[s]?.add(1000 + h);
        }
      }
    }
  }

  cancel(): void {
    if (this.#state.state === 'waiting') this.#state = { state: 'cancelled' };
  }

  reset(): void {
    this.#state = { state: 'idle' };
  }

  keyDown(code: string): void {
    if (this.#state.state !== 'waiting') return;
    if (code === 'Escape') this.#state = { state: 'cancelled' };
    else this.#state = { state: 'captured', source: { kind: 'key', code } };
  }

  mouseButton(button: number): void {
    if (this.#state.state !== 'waiting') return;
    this.#state = { state: 'captured', source: { kind: 'mouse-button', button } };
  }

  wheel(dir: 'up' | 'down'): void {
    if (this.#state.state !== 'waiting') return;
    this.#state = { state: 'captured', source: { kind: 'wheel', dir } };
  }

  /** Looks at every connected device. Call it as often as the pads are polled. */
  poll(hub: GamepadHub, nowMs: number): void {
    if (this.#state.state !== 'waiting') return;
    const settling = nowMs - this.#startedAt < SETTLE_MS;
    for (let s = 0; s < hub.slots.length; s++) {
      const slot = hub.slots[s];
      if (slot?.present !== true) continue;
      if (settling) {
        this.#watchNoise(s, slot.axes, slot.axisCount);
        continue;
      }
      if (!this.#settled) this.#settle();
      const source = this.#hat(s, slot) ?? this.#button(s, slot) ?? this.#axisMoved(s, slot);
      if (source !== undefined) {
        this.#state = { state: 'captured', source };
        return;
      }
    }
    if (!settling && !this.#settled) this.#settle();
  }

  #watchNoise(s: number, axes: Float64Array, count: number): void {
    const lo = this.#lo[s];
    const hi = this.#hi[s];
    const rest = this.#rest[s];
    if (lo === undefined || hi === undefined || rest === undefined) return;
    for (let a = 0; a < count; a++) {
      const v = axes[a] ?? 0;
      if (v < (lo[a] ?? 0)) lo[a] = v;
      if (v > (hi[a] ?? 0)) hi[a] = v;
      // Where an axis rests is where it was when settling ended, not what it read at the start (a
      // device just woken by a button press may report zeros first).
      rest[a] = v;
    }
  }

  /** Decides which axes are ghosts, from how far each wandered while untouched. */
  #settle(): void {
    this.#settled = true;
    for (let s = 0; s < MAX_PADS; s++) {
      for (let a = 0; a < MAX_AXES; a++) {
        const span = (this.#hi[s]?.[a] ?? 0) - (this.#lo[s]?.[a] ?? 0);
        if (span > GHOST_NOISE) this.#ghost[s]?.fill(1, a, a + 1);
      }
    }
  }

  #hat(s: number, slot: GamepadHub['slots'][number]): Source | undefined {
    for (let h = 0; h < slot.hatCount; h++) {
      const mask = slot.hats[h] ?? 0;
      const held = this.#held[s];
      if (mask === 0) {
        held?.delete(1000 + h);
        continue;
      }
      if (held?.has(1000 + h) === true) continue;
      for (const dir of HAT_ORDER) {
        if ((mask & HAT_BIT[dir]) !== 0) return { kind: 'hat', device: slot.device, hat: h, dir };
      }
    }
    return undefined;
  }

  #button(s: number, slot: GamepadHub['slots'][number]): Source | undefined {
    const held = this.#held[s];
    for (let b = 0; b < slot.buttonCount; b++) {
      const down = (slot.buttons[b] ?? 0) > 0.5;
      if (!down) {
        held?.delete(b);
      } else if (held?.has(b) !== true) {
        return { kind: 'pad-button', device: slot.device, index: b };
      }
    }
    return undefined;
  }

  #axisMoved(s: number, slot: GamepadHub['slots'][number]): Source | undefined {
    for (let a = 0; a < slot.axisCount; a++) {
      if (a === slot.hatAxis || this.#ghost[s]?.[a] === 1) continue;
      const delta = (slot.axes[a] ?? 0) - (this.#rest[s]?.[a] ?? 0);
      if (Math.abs(delta) <= CAPTURE_AXIS_DELTA) continue;
      return this.#axis === 'whole'
        ? { kind: 'pad-axis', device: slot.device, index: a }
        : {
            kind: 'pad-axis',
            device: slot.device,
            index: a,
            half: delta > 0 ? 'positive' : 'negative',
          };
    }
    return undefined;
  }
}
