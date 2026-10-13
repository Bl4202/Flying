/**
 * The Rust-generated shared-memory layouts, in the shape the `platform-web` IPC primitives take.
 * Those primitives know nothing about the simulation; these two objects are where the
 * simulation's numbers enter.
 */
import type { RingLayout, SeqlockLayout } from '@flying/platform-web';
import { INPUT_RING, SNAPSHOT_SAB } from './layout.gen.ts';

export const INPUT_RING_LAYOUT: RingLayout = {
  magic: INPUT_RING.MAGIC,
  headerBytes: INPUT_RING.HEADER_SIZE,
  recordBytes: INPUT_RING.RECORD_BYTES,
  capacity: INPUT_RING.CAPACITY,
  hRecordBytes: INPUT_RING.H_RECORD_BYTES,
  hCapacity: INPUT_RING.H_CAPACITY,
  hWrite: INPUT_RING.H_WRITE,
  hRead: INPUT_RING.H_READ,
  hDropped: INPUT_RING.H_DROPPED,
  hMagic: INPUT_RING.H_MAGIC,
};

export const SNAPSHOT_SAB_LAYOUT: SeqlockLayout = {
  magic: SNAPSHOT_SAB.MAGIC,
  version: SNAPSHOT_SAB.VERSION,
  headerBytes: SNAPSHOT_SAB.HEADER_SIZE,
  slotCount: SNAPSHOT_SAB.SLOT_COUNT,
  slotBytes: SNAPSHOT_SAB.SLOT_BYTES,
  hMagic: SNAPSHOT_SAB.H_MAGIC,
  hVersion: SNAPSHOT_SAB.H_VERSION,
  hSlotBytes: SNAPSHOT_SAB.H_SLOT_BYTES,
  hSlotCount: SNAPSHOT_SAB.H_SLOT_COUNT,
  hPublished: SNAPSHOT_SAB.H_PUBLISHED,
  hSlotSeq: SNAPSHOT_SAB.H_SLOT_SEQ,
};
