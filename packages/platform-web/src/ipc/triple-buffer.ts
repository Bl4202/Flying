/**
 * Three-slot seqlock snapshot buffer in a `SharedArrayBuffer`: one writer, any number of readers,
 * nobody ever waits.
 *
 * ```text
 * header: magic, version, slot bytes, slot count, published count, one sequence counter per slot
 * body:   slot count x slot bytes
 * ```
 *
 * Publication number `n` (1, 2, 3, ...) lives in slot `(n - 1) % slotCount`, so the single
 * `published` word says both which slot is newest and how new it is. Writing slot `s` goes
 * `seq[s] odd -> copy -> seq[s] even -> published = n`, all atomic stores (sequentially
 * consistent). A reader loads `published`, then `seq`, copies the slot, and loads `seq` again: if
 * the two sequence values are equal and even, no write touched the slot meanwhile and the copy is
 * whole. Otherwise it retries; after a few tries it reports contention and the caller keeps its
 * previous copy. With three slots the writer is normally two publications away from any slot a
 * reader is looking at, so contention is rare, and the sequence check makes it harmless.
 *
 * The copy itself is ordinary (non-atomic) memory access, as in any seqlock; a torn copy is
 * detected and discarded, never used.
 *
 * Offsets come from the caller (`sim-bridge` passes the Rust-generated layout).
 */

export interface SeqlockLayout {
  magic: number;
  version: number;
  headerBytes: number;
  slotCount: number;
  slotBytes: number;
  hMagic: number;
  hVersion: number;
  hSlotBytes: number;
  hSlotCount: number;
  hPublished: number;
  /** Offset of the first of `slotCount` sequence counters. */
  hSlotSeq: number;
}

export function seqlockBytes(layout: SeqlockLayout): number {
  return layout.headerBytes + layout.slotCount * layout.slotBytes;
}

/** Result of `SeqlockReader.read` when the writer kept overwriting the slot. */
export const CONTENDED = -1;

export function initSeqlock(sab: SharedArrayBuffer, layout: SeqlockLayout): void {
  if (sab.byteLength < seqlockBytes(layout)) {
    throw new RangeError(
      `buffer needs ${String(seqlockBytes(layout))} bytes, got ${String(sab.byteLength)}`,
    );
  }
  const header = new Int32Array(sab, 0, layout.headerBytes / 4);
  header.fill(0);
  header[layout.hVersion / 4] = layout.version;
  header[layout.hSlotBytes / 4] = layout.slotBytes;
  header[layout.hSlotCount / 4] = layout.slotCount;
  Atomics.store(header, layout.hMagic / 4, layout.magic | 0);
}

class SeqlockEnd {
  protected readonly header: Int32Array;
  protected readonly layout: SeqlockLayout;
  protected readonly slots: Uint8Array;

  constructor(sab: SharedArrayBuffer, layout: SeqlockLayout) {
    this.layout = layout;
    this.header = new Int32Array(sab, 0, layout.headerBytes / 4);
    if (
      Atomics.load(this.header, layout.hMagic / 4) !== (layout.magic | 0) ||
      this.header[layout.hVersion / 4] !== layout.version ||
      this.header[layout.hSlotBytes / 4] !== layout.slotBytes ||
      this.header[layout.hSlotCount / 4] !== layout.slotCount
    ) {
      throw new Error('SYS-021: shared snapshot buffer header does not match');
    }
    this.slots = new Uint8Array(sab, layout.headerBytes, layout.slotCount * layout.slotBytes);
  }

  /** Publications so far (0 = none), wrapping at 2^32 and skipping 0. */
  published(): number {
    return Atomics.load(this.header, this.layout.hPublished / 4) >>> 0;
  }

  protected slotOf(published: number): number {
    return ((published - 1) >>> 0) % this.layout.slotCount;
  }
}

export class SeqlockWriter extends SeqlockEnd {
  /**
   * Publishes `src` (exactly `slotBytes`). `prepare`, if given, runs on the slot after the copy
   * and before the slot is released, so the host can stamp its own fields into the published copy.
   */
  publish(src: Uint8Array, prepare?: (slot: Uint8Array) => void): void {
    const l = this.layout;
    if (src.byteLength !== l.slotBytes) {
      throw new RangeError(
        `snapshot is ${String(src.byteLength)} bytes, slots hold ${String(l.slotBytes)}`,
      );
    }
    let next = (this.published() + 1) >>> 0;
    if (next === 0) next = 1;
    const slot = this.slotOf(next);
    const seqAt = l.hSlotSeq / 4 + slot;
    const seq = Atomics.load(this.header, seqAt);
    Atomics.store(this.header, seqAt, (seq + 1) | 0); // odd: write in progress
    const view = this.slots.subarray(slot * l.slotBytes, (slot + 1) * l.slotBytes);
    view.set(src);
    prepare?.(view);
    Atomics.store(this.header, seqAt, (seq + 2) | 0); // even: whole again
    Atomics.store(this.header, l.hPublished / 4, next | 0);
  }
}

export class SeqlockReader extends SeqlockEnd {
  /** Reads that found the slot mid-write or overwritten and had to try again. */
  retries = 0;
  /** Reads that gave up (`CONTENDED`). */
  contended = 0;
  readonly maxTries: number;

  constructor(sab: SharedArrayBuffer, layout: SeqlockLayout, maxTries = 8) {
    super(sab, layout);
    this.maxTries = maxTries;
  }

  /**
   * Copies the newest snapshot into `out` (at least `slotBytes`). Returns its publication number,
   * 0 if nothing has been published, or `CONTENDED` if the writer kept overwriting; in the last
   * two cases `out` is untouched. The copy is at least as new as the number returned.
   */
  read(out: Uint8Array): number {
    const l = this.layout;
    if (out.byteLength < l.slotBytes) {
      throw new RangeError('output buffer is smaller than a slot');
    }
    for (let attempt = 0; attempt < this.maxTries; attempt++) {
      const published = this.published();
      if (published === 0) return 0;
      const slot = this.slotOf(published);
      const seqAt = l.hSlotSeq / 4 + slot;
      const before = Atomics.load(this.header, seqAt);
      if ((before & 1) === 0) {
        out.set(this.slots.subarray(slot * l.slotBytes, (slot + 1) * l.slotBytes));
        if (Atomics.load(this.header, seqAt) === before) {
          return published;
        }
      }
      this.retries += 1;
    }
    this.contended += 1;
    return CONTENDED;
  }
}
