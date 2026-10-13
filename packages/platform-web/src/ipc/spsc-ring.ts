/**
 * Single-producer single-consumer ring of fixed-size records in a `SharedArrayBuffer`.
 *
 * ```text
 * header: record bytes, capacity, write counter, read counter, dropped counter, magic (u32 each)
 * body:   capacity x record bytes
 * ```
 *
 * `write` and `read` are free-running `u32` counters (wrapping), so full is `write - read ==
 * capacity` and the slot is `counter & (capacity - 1)`; capacity is a power of two. The producer
 * owns `write` and `dropped`, the consumer owns `read`. A record is copied in first and `write`
 * published after with an atomic store, so the consumer never sees a half-written record.
 *
 * When the ring is full the producer drops the *new* record and counts it. That keeps the
 * protocol lock-free on both sides; a consumer that is alive drains every few milliseconds, so a
 * full ring means it has stalled, and the count says how much it missed.
 *
 * The header offsets come from the caller (`sim-bridge` passes the Rust-generated layout), so this
 * module has no layout of its own to drift.
 */

export interface RingLayout {
  /** Distinguishes a ring from other buffers; checked when attaching. */
  magic: number;
  headerBytes: number;
  recordBytes: number;
  /** Power of two. */
  capacity: number;
  /** Header offsets, bytes (4-byte aligned). */
  hRecordBytes: number;
  hCapacity: number;
  hWrite: number;
  hRead: number;
  hDropped: number;
  hMagic: number;
}

export function ringBytes(layout: RingLayout): number {
  return layout.headerBytes + layout.capacity * layout.recordBytes;
}

function check(layout: RingLayout): void {
  if (
    layout.capacity < 1 ||
    (layout.capacity & (layout.capacity - 1)) !== 0 ||
    layout.recordBytes < 1
  ) {
    throw new RangeError('ring capacity must be a power of two and records non-empty');
  }
}

/** Writes the header of a fresh ring. Call once, before either side attaches. */
export function initRing(sab: SharedArrayBuffer, layout: RingLayout): void {
  check(layout);
  if (sab.byteLength < ringBytes(layout)) {
    throw new RangeError(
      `ring needs ${String(ringBytes(layout))} bytes, got ${String(sab.byteLength)}`,
    );
  }
  const header = new Int32Array(sab, 0, layout.headerBytes / 4);
  header.fill(0);
  header[layout.hRecordBytes / 4] = layout.recordBytes;
  header[layout.hCapacity / 4] = layout.capacity;
  Atomics.store(header, layout.hMagic / 4, layout.magic | 0);
}

/** State common to both ends. */
class RingEnd {
  protected readonly header: Int32Array;
  protected readonly body: Uint8Array;
  protected readonly layout: RingLayout;
  protected readonly mask: number;

  constructor(sab: SharedArrayBuffer, layout: RingLayout) {
    check(layout);
    this.layout = layout;
    this.header = new Int32Array(sab, 0, layout.headerBytes / 4);
    const magic = Atomics.load(this.header, layout.hMagic / 4);
    if (
      magic !== (layout.magic | 0) ||
      this.header[layout.hRecordBytes / 4] !== layout.recordBytes ||
      this.header[layout.hCapacity / 4] !== layout.capacity
    ) {
      throw new Error(
        'SYS-020: shared ring header does not match (not initialised, or wrong layout)',
      );
    }
    this.mask = layout.capacity - 1;
    this.body = new Uint8Array(sab, layout.headerBytes, layout.capacity * layout.recordBytes);
  }

  /** Records the producer dropped because the ring was full. */
  dropped(): number {
    return Atomics.load(this.header, this.layout.hDropped / 4) >>> 0;
  }

  /** Records waiting. */
  pending(): number {
    const w = Atomics.load(this.header, this.layout.hWrite / 4);
    const r = Atomics.load(this.header, this.layout.hRead / 4);
    return (w - r) >>> 0;
  }
}

export class SpscRingProducer extends RingEnd {
  /** Copies `record` in. Returns false (and counts a drop) if the ring is full. */
  push(record: Uint8Array): boolean {
    const l = this.layout;
    if (record.byteLength !== l.recordBytes) {
      throw new RangeError(
        `record is ${String(record.byteLength)} bytes, ring holds ${String(l.recordBytes)}`,
      );
    }
    const w = Atomics.load(this.header, l.hWrite / 4);
    const r = Atomics.load(this.header, l.hRead / 4);
    if ((w - r) >>> 0 >= l.capacity) {
      Atomics.add(this.header, l.hDropped / 4, 1);
      return false;
    }
    this.body.set(record, (w & this.mask) * l.recordBytes);
    Atomics.store(this.header, l.hWrite / 4, (w + 1) | 0);
    return true;
  }
}

export class SpscRingConsumer extends RingEnd {
  /**
   * Copies up to `maxRecords` waiting records into `out`, oldest first, back to back. Returns how
   * many. `out` must hold `maxRecords * recordBytes`.
   */
  drain(out: Uint8Array, maxRecords: number): number {
    const l = this.layout;
    if (out.byteLength < maxRecords * l.recordBytes) {
      throw new RangeError('output buffer is smaller than maxRecords records');
    }
    const r = Atomics.load(this.header, l.hRead / 4);
    const w = Atomics.load(this.header, l.hWrite / 4);
    const n = Math.min((w - r) >>> 0, maxRecords);
    for (let i = 0; i < n; i++) {
      const at = ((r + i) & this.mask) * l.recordBytes;
      out.set(this.body.subarray(at, at + l.recordBytes), i * l.recordBytes);
    }
    if (n > 0) {
      Atomics.store(this.header, l.hRead / 4, (r + n) | 0);
    }
    return n;
  }
}
