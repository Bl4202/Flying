/**
 * The ownship's state for the page: the warning strip, the overlay's flight rows, and the input
 * pump (which needs the spawn number, the throttle, the assist level and where the nose points,
 * ADR-36).
 *
 * With the shared snapshot channel the page reads the newest snapshot itself every animation
 * frame; a seqlock read does not disturb the renderer's. With the message channel the renderer
 * owns the only end of the channel, so the page takes the status the sim reports with its 4 Hz
 * stats. The duel's status (weapons, target, bandit; ADR-48) comes the same two ways, from the
 * same snapshot read, and so does the simulation's tick (the mission recorder's clock). The duel's
 * events come only from the shared channel: each new one is handed to `onEvent` once.
 */
import type { PumpObservation } from '@flying/input';
import {
  type AircraftStatus,
  type CombatEventView,
  type CombatStatus,
  CONTENDED,
  EventCursor,
  newAircraftStatus,
  newCombatStatus,
  readAircraftStatus,
  readCombatStatus,
  SNAPSHOT,
  SnapshotReader,
  type SnapshotSource,
} from '@flying/sim-bridge';

export interface StatusFeedDeps {
  /** The page's own end of the shared channel; undefined with the message channel. */
  source: SnapshotSource | undefined;
  /** The status from the sim's periodic stats (the message channel's only way). */
  reported: () => AircraftStatus | undefined;
  /** The duel's status from the same stats (the message channel); absent: none. */
  reportedCombat?: () => CombatStatus | undefined;
  /** The tick of those stats (the message channel). */
  reportedTick?: () => number | undefined;
  /** Each new combat event, once, oldest first (the shared channel only). */
  onEvent?: (event: CombatEventView) => void;
  requestFrame: (callback: () => void) => number;
  cancelFrame: (id: number) => void;
  /** Called with the new status whenever it has been read afresh. */
  onStatus: (status: AircraftStatus, observation: PumpObservation) => void;
}

export class StatusFeed {
  readonly #d: StatusFeedDeps;
  readonly #buf = new Uint8Array(SNAPSHOT.SIZE);
  readonly #reader = new SnapshotReader(this.#buf);
  readonly #own = newAircraftStatus();
  readonly #combat = newCombatStatus();
  /** One object, filled in place for every call to `onStatus`. */
  readonly #observation: PumpObservation = {
    spawnSeq: 0,
    throttle: 0,
    assist: 0,
    noseAzRad: 0,
    noseElRad: 0,
  };
  readonly #cursor = new EventCursor();
  #tick = 0;
  #have = false;
  #lastPublication = 0;
  #frame: number | undefined;

  constructor(deps: StatusFeedDeps) {
    this.#d = deps;
  }

  /** The newest status, or undefined before the first snapshot. */
  get status(): AircraftStatus | undefined {
    return this.#d.source === undefined ? this.#d.reported() : this.#have ? this.#own : undefined;
  }

  /** The duel's newest status (all zero without combat), or undefined before the first snapshot. */
  get combat(): CombatStatus | undefined {
    if (this.#d.source === undefined) return this.#d.reportedCombat?.();
    return this.#have ? this.#combat : undefined;
  }

  /** The simulation's tick as of the newest status (0 before the first). */
  get tick(): number {
    if (this.#d.source === undefined) return this.#d.reportedTick?.() ?? 0;
    return this.#tick;
  }

  start(): void {
    if (this.#frame !== undefined) return;
    const loop = (): void => {
      this.poll();
      this.#frame = this.#d.requestFrame(loop);
    };
    this.#frame = this.#d.requestFrame(loop);
  }

  stop(): void {
    if (this.#frame !== undefined) this.#d.cancelFrame(this.#frame);
    this.#frame = undefined;
    this.#d.source?.close();
  }

  /** Reads the newest status and passes it on. Returns whether there was one. */
  poll(): boolean {
    const source = this.#d.source;
    let status: AircraftStatus | undefined;
    if (source === undefined) {
      status = this.#d.reported();
    } else {
      const n = source.read(this.#buf);
      if (n !== CONTENDED && n > 0 && n !== this.#lastPublication && this.#reader.valid()) {
        this.#lastPublication = n;
        readAircraftStatus(this.#reader, this.#own);
        readCombatStatus(this.#reader, this.#combat);
        this.#tick = this.#reader.tick();
        const onEvent = this.#d.onEvent;
        if (onEvent !== undefined) this.#cursor.drain(this.#reader, onEvent);
        this.#have = true;
      }
      status = this.#have ? this.#own : undefined;
    }
    if (status === undefined) return false;
    const o = this.#observation;
    o.spawnSeq = status.spawnSeq;
    o.throttle = status.throttle;
    o.assist = status.assist;
    o.noseAzRad = status.noseAz;
    o.noseElRad = status.noseEl;
    this.#d.onStatus(status, o);
    return true;
  }
}
