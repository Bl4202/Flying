/**
 * Snapshot interpolation: the renderer draws between the two newest snapshots so motion is smooth
 * at any display rate, not only at 60 Hz.
 *
 * It keeps its own copies of the two newest snapshots (the channel only ever shows the latest).
 * Drawing happens `delayMs` in the past, by default one snapshot interval, so that there are
 * usually two snapshots on either side of the moment being drawn. The blend factor comes from the
 * publish times the Sim Worker stamped (shared clock), not from when this thread happened to read
 * them, so read jitter does not become motion jitter.
 *
 * Positions blend in f64, orientations by normalised linear interpolation along the short arc.
 * It copes with having none, one, a stale newest, a gap of several ticks, and a body or the
 * observer teleporting between snapshots (it snaps instead of sweeping across the map).
 */
import { SNAPSHOT } from '@flying/sim-bridge/layout';
import { SnapshotReader } from '@flying/sim-bridge/snapshot';

/** What the scene builder needs: the interpolated world, in BF. */
export interface SceneState {
  tick: number;
  coreState: number;
  /** When the newest snapshot was published (shared clock) and when its input was sampled. */
  publishTimeMs: number;
  inputTimeMs: number;
  observerPosBf: Float64Array;
  /** Orientation `[w, x, y, z]`. */
  observerQuat: Float32Array;
  observerSpeed: number;
  observerThrottle: number;
  bodyCount: number;
  bodyPosBf: Float64Array;
  bodyQuat: Float32Array;
}

export interface InterpStatus {
  /** Blend factor between the older (0) and newer (1) snapshot. */
  alpha: number;
  /** The newest snapshot is older than `staleMs`: the sim has stopped or fallen behind. */
  stale: boolean;
  /** The two snapshots are more than one tick apart (a snapshot was skipped). */
  gapped: boolean;
  /** Something moved further than `teleportM` between them and was not blended. */
  snapped: boolean;
}

export interface InterpOptions {
  /** How far in the past to draw, milliseconds. */
  delayMs?: number;
  /** Age of the newest snapshot beyond which the world counts as stale, milliseconds. */
  staleMs?: number;
  /** Movement between snapshots beyond which a body is not blended, metres. */
  teleportM?: number;
}

const MAX_BODIES = SNAPSHOT.BODY_CAPACITY;

export class SnapshotInterpolator {
  readonly delayMs: number;
  readonly staleMs: number;
  readonly teleportM: number;
  /** The newest sample; overwritten by every `sample` call. */
  readonly state: SceneState = {
    tick: 0,
    coreState: 0,
    publishTimeMs: 0,
    inputTimeMs: 0,
    observerPosBf: new Float64Array(3),
    observerQuat: new Float32Array([1, 0, 0, 0]),
    observerSpeed: 0,
    observerThrottle: 0,
    bodyCount: 0,
    bodyPosBf: new Float64Array(MAX_BODIES * 3),
    bodyQuat: new Float32Array(MAX_BODIES * 4),
  };
  readonly status: InterpStatus = { alpha: 1, stale: false, gapped: false, snapped: false };

  #prev = new SnapshotReader(new Uint8Array(SNAPSHOT.SIZE));
  #curr = new SnapshotReader(new Uint8Array(SNAPSHOT.SIZE));
  #count = 0;
  #prevTick = 0;
  #currTick = 0;
  /** Snapshots taken in (accepted), and ignored as duplicates or invalid. */
  accepted = 0;
  ignored = 0;

  // Scratch, so sampling allocates nothing.
  readonly #a3 = new Float64Array(3);
  readonly #b3 = new Float64Array(3);
  readonly #a4 = new Float32Array(4);
  readonly #b4 = new Float32Array(4);

  constructor(opts: InterpOptions = {}) {
    this.delayMs = opts.delayMs ?? 1000 / 60;
    this.staleMs = opts.staleMs ?? 250;
    this.teleportM = opts.teleportM ?? 500;
  }

  /** How many snapshots are held (0, 1 or 2). */
  get held(): number {
    return this.#count;
  }

  /**
   * Takes a copy of `bytes` if it is a valid snapshot newer than the newest held. A snapshot with
   * an older tick means the sim restarted: everything held is dropped and this becomes the first.
   * Returns whether it was taken.
   */
  push(bytes: Uint8Array): boolean {
    if (bytes.byteLength < SNAPSHOT.SIZE) {
      this.ignored += 1;
      return false;
    }
    const incoming = new SnapshotReader(bytes);
    if (!incoming.valid()) {
      this.ignored += 1;
      return false;
    }
    const tick = incoming.tick();
    if (this.#count > 0 && tick === this.#currTick) {
      this.ignored += 1;
      return false;
    }
    if (this.#count > 0 && tick < this.#currTick) {
      this.#count = 0; // the sim restarted
    }
    const spare = this.#prev;
    this.#prev = this.#curr;
    this.#curr = spare;
    this.#prevTick = this.#currTick;
    this.#currTick = tick;
    this.#curr.bytes.set(bytes.subarray(0, SNAPSHOT.SIZE));
    this.#count = Math.min(this.#count + 1, 2);
    this.accepted += 1;
    return true;
  }

  /** The world as it should be drawn at `nowMs` (shared clock), or `null` before any snapshot. */
  sample(nowMs: number): SceneState | null {
    if (this.#count === 0) {
      return null;
    }
    const cur = this.#curr;
    const prev = this.#prev;
    const s = this.state;
    const st = this.status;
    st.stale = nowMs - cur.publishTimeMs() > this.staleMs;
    st.gapped = this.#count === 2 && this.#currTick - this.#prevTick > 1;
    st.snapped = false;

    let alpha = 1;
    if (this.#count === 2) {
      const span = cur.publishTimeMs() - prev.publishTimeMs();
      if (span > 0) {
        alpha = Math.min(Math.max((nowMs - this.delayMs - prev.publishTimeMs()) / span, 0), 1);
      }
    }
    st.alpha = alpha;

    s.tick = this.#currTick;
    s.coreState = cur.coreState();
    s.publishTimeMs = cur.publishTimeMs();
    s.inputTimeMs = cur.inputTimeMs();
    s.observerSpeed = cur.observerSpeed();
    s.observerThrottle = cur.observerThrottle();
    s.bodyCount = Math.min(cur.bodyCount(), MAX_BODIES);

    // The observer.
    cur.observerPos(this.#b3);
    cur.observerQuat(this.#b4);
    if (alpha < 1) {
      prev.observerPos(this.#a3);
      prev.observerQuat(this.#a4);
      if (distance(this.#a3, this.#b3) > this.teleportM) {
        st.snapped = true;
        s.observerPosBf.set(this.#b3);
        s.observerQuat.set(this.#b4);
      } else {
        lerp3(s.observerPosBf, 0, this.#a3, 0, this.#b3, 0, alpha);
        nlerp(s.observerQuat, 0, this.#a4, 0, this.#b4, 0, alpha);
      }
    } else {
      s.observerPosBf.set(this.#b3);
      s.observerQuat.set(this.#b4);
    }

    // The bodies.
    const blendBodies = alpha < 1 ? Math.min(prev.bodyCount(), s.bodyCount) : 0;
    for (let i = 0; i < s.bodyCount; i++) {
      cur.bodyPos(i, this.#b3);
      cur.bodyQuat(i, this.#b4);
      if (i < blendBodies) {
        prev.bodyPos(i, this.#a3);
        if (distance(this.#a3, this.#b3) <= this.teleportM) {
          prev.bodyQuat(i, this.#a4);
          lerp3(s.bodyPosBf, i * 3, this.#a3, 0, this.#b3, 0, alpha);
          nlerp(s.bodyQuat, i * 4, this.#a4, 0, this.#b4, 0, alpha);
          continue;
        }
        st.snapped = true;
      }
      s.bodyPosBf.set(this.#b3, i * 3);
      s.bodyQuat.set(this.#b4, i * 4);
    }
    return s;
  }
}

function distance(a: Float64Array, b: Float64Array): number {
  return Math.hypot(
    (a[0] ?? 0) - (b[0] ?? 0),
    (a[1] ?? 0) - (b[1] ?? 0),
    (a[2] ?? 0) - (b[2] ?? 0),
  );
}

function lerp3(
  out: Float64Array,
  o: number,
  a: Float64Array,
  ao: number,
  b: Float64Array,
  bo: number,
  t: number,
): void {
  for (let k = 0; k < 3; k++) {
    const x = a[ao + k] ?? 0;
    out[o + k] = x + ((b[bo + k] ?? 0) - x) * t;
  }
}

/** Normalised linear interpolation along the short arc. */
function nlerp(
  out: Float32Array,
  o: number,
  a: Float32Array,
  ao: number,
  b: Float32Array,
  bo: number,
  t: number,
): void {
  let dot = 0;
  for (let k = 0; k < 4; k++) dot += (a[ao + k] ?? 0) * (b[bo + k] ?? 0);
  const sign = dot < 0 ? -1 : 1;
  let norm = 0;
  for (let k = 0; k < 4; k++) {
    const x = a[ao + k] ?? 0;
    const v = x + (sign * (b[bo + k] ?? 0) - x) * t;
    out[o + k] = v;
    norm += v * v;
  }
  const inv = norm > 0 ? 1 / Math.sqrt(norm) : 1;
  for (let k = 0; k < 4; k++) out[o + k] = (out[o + k] ?? 0) * inv;
}
