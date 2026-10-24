/**
 * Fakes shared by the tests: gamepads shaped as the Gamepad API gives them, and a list the hub
 * polls that a test can change between polls.
 */
import { type GamepadLike } from './devices/gamepad.ts';
import { type MouseSource } from './devices/mouse.ts';

/** A standard-mapping pad as Chrome names it. */
export const PAD_ID = 'Xbox Wireless Controller (STANDARD GAMEPAD Vendor: 045e Product: 0b13)';
/** A raw joystick as Chrome names it. */
export const STICK_ID = 'T.16000M (Vendor: 044f Product: b10a)';

export interface FakePadSpec {
  index?: number;
  id?: string;
  mapping?: string;
  axes?: Record<number, number>;
  axisCount?: number;
  buttons?: Record<number, number>;
  buttonCount?: number;
  timestamp?: number;
  connected?: boolean;
}

/** A pad with the given axes and buttons set and everything else at rest. */
export function fakePad(spec: FakePadSpec = {}): GamepadLike {
  const mapping = spec.mapping ?? 'standard';
  const axisCount = spec.axisCount ?? 4;
  const buttonCount = spec.buttonCount ?? (mapping === 'standard' ? 17 : 12);
  return {
    connected: spec.connected ?? true,
    mapping,
    index: spec.index ?? 0,
    id: spec.id ?? (mapping === 'standard' ? PAD_ID : STICK_ID),
    timestamp: spec.timestamp ?? 0,
    axes: Array.from({ length: axisCount }, (_, i) => spec.axes?.[i] ?? 0),
    buttons: Array.from({ length: buttonCount }, (_, i) => {
      const value = spec.buttons?.[i] ?? 0;
      return { pressed: value > 0.5, value };
    }),
  };
}

/** A raw joystick: `axes` axes (a hat on axis 9 when there are ten or more) and 12 buttons. */
export function fakeStick(spec: FakePadSpec = {}): GamepadLike {
  const axisCount = spec.axisCount ?? 10;
  const axes = { ...(axisCount > 9 ? { 9: HAT_CENTRED } : {}), ...spec.axes };
  return fakePad({ mapping: '', axisCount, ...spec, axes });
}

/** The value a centred hat reports on axis 9. */
export const HAT_CENTRED = 1.2857142857142856;

/** The list `navigator.getGamepads()` would return, changeable between polls. */
export class FakePads {
  list: (GamepadLike | null)[] = [];
  readonly get = (): readonly (GamepadLike | null)[] => this.list;

  set(index: number, pad: GamepadLike | null): void {
    while (this.list.length <= index) this.list.push(null);
    this.list[index] = pad;
  }
}

/** A mouse the test drives directly: `push` and `notch` queue motion for the next `consume`. */
export class FakeMouse implements MouseSource {
  locked = true;
  dx = 0;
  dy = 0;
  wheelUp = 0;
  wheelDown = 0;
  readonly down = new Set<number>();
  #qx = 0;
  #qy = 0;
  #qup = 0;
  #qdown = 0;

  push(dx: number, dy: number): void {
    this.#qx += dx;
    this.#qy += dy;
  }

  notch(dir: 'up' | 'down', count = 1): void {
    if (dir === 'up') this.#qup += count;
    else this.#qdown += count;
  }

  consume(): void {
    this.dx = this.#qx;
    this.dy = this.#qy;
    this.wheelUp = this.#qup;
    this.wheelDown = this.#qdown;
    this.#qx = 0;
    this.#qy = 0;
    this.#qup = 0;
    this.#qdown = 0;
  }

  isDown(button: number): boolean {
    return this.down.has(button);
  }
}

/**
 * How many bytes of garbage one call of `fn` makes, on average, once the code is optimised.
 *
 * `heapUsed` is read around chunks of calls small enough that the young generation does not fill
 * and get collected inside one; the median over many chunks, less what reading `heapUsed` itself
 * costs, is the answer for that round. A function that allocates nothing measures about 0; an
 * array literal measures its size.
 *
 * V8 optimises hot code on a background thread and installs the result when it is done, and code
 * that has not been optimised yet boxes every fractional number it touches. So the measurement
 * warms up, gives the compiler time, measures, and repeats; it returns the smallest round, which
 * is the steady state if there was one and the truth if there was none.
 */
export async function bytesPerCall(
  fn: (i: number) => void,
  options: { rounds?: number; chunks?: number; chunkSize?: number } = {},
): Promise<number> {
  const { rounds = 6, chunks = 41, chunkSize = 2000 } = options;
  const v8 = await import('node:v8');
  const vm = await import('node:vm');
  v8.setFlagsFromString('--expose_gc');
  const gc = vm.runInNewContext('gc') as () => void;
  const median = (xs: number[]): number => [...xs].sort((a, b) => a - b)[xs.length >> 1] ?? 0;
  const chunk = (body: (i: number) => void): number => {
    const before = process.memoryUsage().heapUsed;
    for (let i = 0; i < chunkSize; i++) body(i);
    return process.memoryUsage().heapUsed - before;
  };
  const noop = (): void => undefined;
  let best = Number.POSITIVE_INFINITY;
  for (let round = 0; round < rounds; round++) {
    for (let i = 0; i < 100_000; i++) fn(i);
    await new Promise((resolve) => setTimeout(resolve, 30)); // let the compiler finish
    gc();
    const overhead = median(Array.from({ length: chunks }, () => chunk(noop)));
    const deltas = Array.from({ length: chunks }, () => chunk(fn));
    best = Math.min(best, Math.max(0, median(deltas) - overhead) / chunkSize);
    if (best < 0.5) break;
  }
  return best;
}
