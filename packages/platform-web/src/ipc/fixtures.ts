/** Layouts and helpers shared by the IPC tests and their worker threads. Not production code. */
import type { RingLayout } from './spsc-ring.ts';
import type { SeqlockLayout } from './triple-buffer.ts';

/** A small ring: 16-byte records so a million of them move quickly. */
export const TEST_RING: RingLayout = {
  magic: 0x74657374,
  headerBytes: 32,
  recordBytes: 16,
  capacity: 64,
  hRecordBytes: 0,
  hCapacity: 4,
  hWrite: 8,
  hRead: 12,
  hDropped: 16,
  hMagic: 20,
};

/** Slots big enough that copying one takes long enough for a writer to lap a reader. */
export const TEST_SEQLOCK: SeqlockLayout = {
  magic: 0x73657165,
  version: 1,
  headerBytes: 64,
  slotCount: 3,
  slotBytes: 4096,
  hMagic: 0,
  hVersion: 4,
  hSlotBytes: 8,
  hSlotCount: 12,
  hPublished: 16,
  hSlotSeq: 20,
};

/** Fills a 16-byte record from its sequence number: `seq` (u32), then bytes derived from it. */
export function fillRecord(out: Uint8Array, seq: number): void {
  new DataView(out.buffer, out.byteOffset, 16).setUint32(0, seq, true);
  for (let i = 4; i < 16; i++) {
    out[i] = (seq * 31 + i * 7) & 0xff;
  }
}

/** True if `rec` is exactly `fillRecord(_, seq)`. */
export function recordIs(rec: Uint8Array, seq: number): boolean {
  if (new DataView(rec.buffer, rec.byteOffset, 16).getUint32(0, true) !== seq) return false;
  for (let i = 4; i < 16; i++) {
    if (rec[i] !== ((seq * 31 + i * 7) & 0xff)) return false;
  }
  return true;
}
