/**
 * Reading the snapshot's combat event ring once each (ADR-48). The core numbers events from 1 and
 * keeps the newest 32; a reader remembers the last number it handled and, each time it reads a
 * snapshot, takes every newer event in order. Events that fell out of the ring before it looked are
 * counted in `lost`, not invented.
 */
import { SNAPSHOT } from './layout.gen.ts';
import type { SnapshotReader } from './snapshot.ts';

/** One event, as `EventCursor.drain` hands it out (the same object every time: copy what you keep). */
export interface CombatEventView {
  seq: number;
  /** The tick it happened, mod 2^32. */
  tick: number;
  /** `COMBAT_EVENT`. */
  kind: number;
  /** `COMBAT_ACTOR` of who did it, and of whom it was done to. */
  actor: number;
  target: number;
  /** The kind's detail (`KILL_WEAPON`, `CRASH_CAUSE`, `MISS_REASON` or `LAUNCH_INHIBIT`). */
  arg: number;
  /** `MISSILE_HIT`: the miss distance, m; `KILL`: the range, m. */
  value: number;
}

export class EventCursor {
  /** The newest sequence number handed out (0: none yet). */
  lastSeq = 0;
  /** Events that were overwritten in the ring before this cursor read them. */
  lost = 0;
  readonly #event: CombatEventView = {
    seq: 0,
    tick: 0,
    kind: 0,
    actor: 0,
    target: 0,
    arg: 0,
    value: 0,
  };

  /**
   * Calls `cb` once for every event newer than `lastSeq`, oldest first, through one reused
   * object, and moves `lastSeq` on. A snapshot whose newest event is older than `lastSeq` (a new
   * session) starts the cursor again from that snapshot's events. Returns how many it handed out.
   */
  drain(reader: SnapshotReader, cb: (event: CombatEventView) => void): number {
    const newest = reader.eventSeq();
    if (newest < this.lastSeq) this.lastSeq = 0;
    if (newest === this.lastSeq) return 0;
    const capacity = SNAPSHOT.EVENT_CAPACITY;
    const oldest = Math.max(1, newest - capacity + 1);
    if (oldest > this.lastSeq + 1) {
      this.lost += oldest - this.lastSeq - 1;
    }
    let handed = 0;
    for (let seq = Math.max(oldest, this.lastSeq + 1); seq <= newest; seq++) {
      const slot = (seq - 1) % capacity;
      if (reader.eventSeqAt(slot) !== seq) continue;
      const e = this.#event;
      e.seq = seq;
      e.tick = reader.eventTickAt(slot);
      e.kind = reader.eventKindAt(slot);
      e.actor = reader.eventActorAt(slot);
      e.target = reader.eventTargetAt(slot);
      e.arg = reader.eventArgAt(slot);
      e.value = reader.eventValueAt(slot);
      cb(e);
      handed++;
    }
    this.lastSeq = newest;
    return handed;
  }

  /** Forgets everything (a new session). */
  reset(): void {
    this.lastSeq = 0;
    this.lost = 0;
  }
}
