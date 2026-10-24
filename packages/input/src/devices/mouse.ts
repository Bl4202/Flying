/**
 * The mouse under Pointer Lock (GDD 58.1, ADR-37).
 *
 * Motion, wheel notches and buttons are accumulated as events arrive and handed over by
 * `consume()` once per pump update; nothing is written to the input channel per `pointermove`
 * (the ring takes eight frames per tick, and a fast mouse sends far more events than that). Pointer
 * Lock is asked for with `unadjustedMovement` (raw motion, no OS acceleration); where the browser
 * refuses, it falls back to the ordinary lock and says so once.
 */

export interface PointerEventLike {
  readonly movementX: number;
  readonly movementY: number;
  readonly button?: number;
  readonly timeStamp: number;
  getCoalescedEvents?(): readonly { readonly movementX: number; readonly movementY: number }[];
}

export interface WheelEventLike {
  readonly deltaY: number;
  /** 0 pixels, 1 lines, 2 pages. */
  readonly deltaMode?: number;
  readonly timeStamp: number;
  preventDefault(): void;
}

export interface MouseTargetLike extends EventTarget {
  requestPointerLock(options?: { unadjustedMovement?: boolean }): Promise<void> | void;
}

export interface MouseDocumentLike extends EventTarget {
  readonly pointerLockElement: unknown;
  exitPointerLock(): void;
}

export type MouseWarning = 'unadjusted-unavailable';

export interface MouseOptions {
  target: MouseTargetLike;
  document: MouseDocumentLike;
  /** Called with the event time of every event that can change a control. */
  onEvent?: (timeStampMs: number) => void;
  onWarn?: (warning: MouseWarning) => void;
}

/** What the pump reads each update. */
export interface MouseSource {
  readonly locked: boolean;
  /** Motion since the last `consume()`, in pixels. */
  readonly dx: number;
  readonly dy: number;
  /** Wheel notches since the last `consume()`. */
  readonly wheelUp: number;
  readonly wheelDown: number;
  /** Moves the accumulated motion into `dx`, `dy` and the wheel counts, and clears it. */
  consume(): void;
  isDown(button: number): boolean;
}

/** One wheel notch is this many pixels of `deltaY` (Chrome reports 100). */
const NOTCH_PX = 100;
/** Firefox and others report lines. */
const LINE_PX = 100 / 3;
const PAGE_PX = 800;

export class MouseDevice implements MouseSource {
  wheelUp = 0;
  wheelDown = 0;
  readonly #o: MouseOptions;
  /** dx, dy handed over; dx, dy accumulating; wheel pixels (a Float64Array: no garbage). */
  readonly #v = new Float64Array(5);
  #accUp = 0;
  #accDown = 0;
  #buttons = 0;
  #locked = false;
  #warned = false;

  constructor(options: MouseOptions) {
    this.#o = options;
  }

  get locked(): boolean {
    return this.#locked;
  }

  get dx(): number {
    return this.#v[0] ?? 0;
  }

  get dy(): number {
    return this.#v[1] ?? 0;
  }

  isDown(button: number): boolean {
    return button >= 0 && button < 31 && (this.#buttons & (1 << button)) !== 0;
  }

  consume(): void {
    this.#v[0] = this.#v[2] ?? 0;
    this.#v[1] = this.#v[3] ?? 0;
    this.wheelUp = this.#accUp;
    this.wheelDown = this.#accDown;
    this.#v[2] = 0;
    this.#v[3] = 0;
    this.#accUp = 0;
    this.#accDown = 0;
  }

  /** Asks for Pointer Lock. Needs a user gesture, so call it from a click. */
  lock(): void {
    const t = this.#o.target;
    const plain = (): void => {
      try {
        void Promise.resolve(t.requestPointerLock()).catch(() => undefined);
      } catch {
        // refused: stays unlocked
      }
    };
    try {
      const p = t.requestPointerLock({ unadjustedMovement: true });
      if (p !== undefined) {
        p.catch(() => {
          this.#warnOnce();
          plain();
        });
      }
    } catch {
      this.#warnOnce();
      plain();
    }
  }

  unlock(): void {
    this.#o.document.exitPointerLock();
  }

  /** Starts listening. Returns a function that stops. */
  attach(): () => void {
    const { target, document, onEvent } = this.#o;
    const onMove = (e: Event): void => {
      if (!this.#locked) return;
      const p = e as unknown as PointerEventLike;
      const list = p.getCoalescedEvents?.();
      if (list !== undefined && list.length > 0) {
        for (const c of list) {
          this.#v[2] = (this.#v[2] ?? 0) + c.movementX;
          this.#v[3] = (this.#v[3] ?? 0) + c.movementY;
        }
      } else {
        this.#v[2] = (this.#v[2] ?? 0) + p.movementX;
        this.#v[3] = (this.#v[3] ?? 0) + p.movementY;
      }
      onEvent?.(p.timeStamp);
    };
    const onDown = (e: Event): void => {
      if (!this.#locked) return;
      const p = e as unknown as PointerEventLike;
      this.#buttons |= 1 << (p.button ?? 0);
      onEvent?.(p.timeStamp);
    };
    const onUp = (e: Event): void => {
      const p = e as unknown as PointerEventLike;
      this.#buttons &= ~(1 << (p.button ?? 0));
      if (this.#locked) onEvent?.(p.timeStamp);
    };
    const onWheel = (e: Event): void => {
      if (!this.#locked) return;
      const w = e as unknown as WheelEventLike;
      w.preventDefault();
      const unit = w.deltaMode === 1 ? LINE_PX : w.deltaMode === 2 ? PAGE_PX : 1;
      let px = (this.#v[4] ?? 0) + w.deltaY * unit;
      while (px <= -NOTCH_PX) {
        this.#accUp += 1;
        px += NOTCH_PX;
      }
      while (px >= NOTCH_PX) {
        this.#accDown += 1;
        px -= NOTCH_PX;
      }
      this.#v[4] = px;
      onEvent?.(w.timeStamp);
    };
    const onMenu = (e: Event): void => {
      e.preventDefault();
    };
    const onLock = (): void => {
      this.#locked = document.pointerLockElement === target;
      if (!this.#locked) {
        this.#buttons = 0;
        this.#v[2] = 0;
        this.#v[3] = 0;
        this.#v[4] = 0;
      }
    };
    target.addEventListener('pointermove', onMove);
    target.addEventListener('pointerdown', onDown);
    target.addEventListener('pointerup', onUp);
    target.addEventListener('wheel', onWheel, { passive: false });
    target.addEventListener('contextmenu', onMenu);
    document.addEventListener('pointerlockchange', onLock);
    return () => {
      target.removeEventListener('pointermove', onMove);
      target.removeEventListener('pointerdown', onDown);
      target.removeEventListener('pointerup', onUp);
      target.removeEventListener('wheel', onWheel);
      target.removeEventListener('contextmenu', onMenu);
      document.removeEventListener('pointerlockchange', onLock);
    };
  }

  #warnOnce(): void {
    if (this.#warned) return;
    this.#warned = true;
    this.#o.onWarn?.('unadjusted-unavailable');
  }
}
