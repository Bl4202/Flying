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
 * ownship teleporting between snapshots (it snaps instead of sweeping across the map).
 *
 * The duel (snapshot v5, ADR-48) blends the same way where a thing is the same thing in both
 * snapshots: the bandit (unless it respawned: `BD_SPAWN_SEQ` moved), the pipper point (while both
 * have one), each missile (the same slot, in flight in both) and each tracer (the same round, by
 * `TR_ID`). Everything else of the duel comes from the newest snapshot.
 */
import {
  BANDIT_LIFE,
  COMBAT_FLAG,
  MISSILE_STATE,
  OWNSHIP_FLAG2,
  SNAPSHOT,
} from '@flying/sim-bridge/layout';
import { SnapshotReader } from '@flying/sim-bridge/snapshot';

/** What the scene builder needs: the interpolated world, in BF. */
export interface SceneState {
  tick: number;
  coreState: number;
  /** The test arena the world is (`ARENA`): it decides where the globe sits. */
  arena: number;
  /** When the newest snapshot was published (shared clock) and when its input was sampled. */
  publishTimeMs: number;
  inputTimeMs: number;
  ownshipPosBf: Float64Array;
  /** Orientation `[w, x, y, z]`. */
  ownshipQuat: Float32Array;
  /** True airspeed, m/s. */
  ownshipSpeed: number;
  /** Velocity in BF (east, north, up), m/s: the camera's chase direction. */
  ownshipVelBf: Float32Array;
  /** Speed over the ground, m/s. */
  groundSpeed: number;
  /** The load factor the pilot feels, lagged (g): the cockpit camera's sag. */
  gLagged: number;
  /** Angle of attack and the angle at which the warning starts, radians: the buffet's drive. */
  alpha: number;
  alphaWarn: number;
  /** Afterburner level, 0 to 1. */
  abLevel: number;
  /** Control surfaces and flaps, radians; the airbrake, 0 stowed to 1 open; the gear, 0 up to 1 down: the model's channels. */
  elevatorRad: number;
  aileronRad: number;
  rudderRad: number;
  flapRad: number;
  airbrake: number;
  gearPos: number;
  /** Weight is on the wheels. */
  onGround: boolean;
  ownshipThrottle: number;
  /** `OWNSHIP_FLAG2` bits. */
  ownshipFlags2: number;
  /** The pilot's g stress `S` and red-out `R` (what the picture shows; zero with the dial off). */
  gStress: number;
  redout: number;
  /** The mouse-aim direction in use: azimuth clockwise from north, elevation up, radians. */
  aimAz: number;
  aimEl: number;
  bodyCount: number;
  bodyPosBf: Float64Array;
  bodyQuat: Float32Array;

  // --- The duel (all zero without combat). ---
  /** `COMBAT_FLAG` bits, `SEEKER_MODE`, `LAUNCH_INHIBIT` and how long ago it was, seconds. */
  combatFlags: number;
  seeker: number;
  inhibit: number;
  inhibitAge: number;
  /** Gun rounds and missiles left. */
  rounds: number;
  missilesLeft: number;
  /** The designated target's range (m) and closure (m/s). */
  tgtRange: number;
  tgtClosure: number;
  /** The player's health, 0 to 1. */
  health: number;
  /** The gunsight's aim point, BF metres (while `COMBAT_FLAG.PIPPER`). */
  pipperBf: Float64Array;
  /** `BANDIT_LIFE` and the spawn counter. */
  banditLife: number;
  banditSpawnSeq: number;
  /** The bandit's reference point (BF, f64), attitude `[w, x, y, z]` and velocity (BF m/s). */
  banditPosBf: Float64Array;
  banditQuat: Float32Array;
  banditVelBf: Float32Array;
  /** Its surfaces and flap (radians), airbrake, gear and afterburner (0 to 1): its model's channels. */
  banditElevatorRad: number;
  banditAileronRad: number;
  banditRudderRad: number;
  banditFlapRad: number;
  banditAirbrake: number;
  banditGearPos: number;
  banditAb: number;
  /** Missiles in flight or bursting, packed: position (BF), velocity, `MISSILE_STATE`. */
  missileCount: number;
  missilePosBf: Float64Array;
  missileVelBf: Float32Array;
  missileState: Uint8Array;
  /** Tracers, packed: position (BF) and velocity. */
  tracerCount: number;
  tracerPosBf: Float64Array;
  tracerVelBf: Float32Array;
  /** Explosions, packed: position (BF), age (s) and `EXPLOSION_KIND`. */
  explosionCount: number;
  explosionPosBf: Float64Array;
  explosionAge: Float32Array;
  explosionKind: Uint8Array;
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
const MISSILES = SNAPSHOT.MISSILE_CAPACITY;
const TRACERS = SNAPSHOT.TRACER_CAPACITY;
const EXPLOSIONS = SNAPSHOT.EXPLOSION_CAPACITY;
/** Round pool slots a tracer id can name (`TR_ID` is a byte). */
const TRACER_IDS = 256;

export class SnapshotInterpolator {
  readonly delayMs: number;
  readonly staleMs: number;
  readonly teleportM: number;
  /** The newest sample; overwritten by every `sample` call. */
  readonly state: SceneState = {
    tick: 0,
    coreState: 0,
    arena: 0,
    publishTimeMs: 0,
    inputTimeMs: 0,
    ownshipPosBf: new Float64Array(3),
    ownshipQuat: new Float32Array([1, 0, 0, 0]),
    ownshipSpeed: 0,
    ownshipVelBf: new Float32Array(3),
    groundSpeed: 0,
    gLagged: 1,
    alpha: 0,
    alphaWarn: 0,
    abLevel: 0,
    elevatorRad: 0,
    aileronRad: 0,
    rudderRad: 0,
    flapRad: 0,
    airbrake: 0,
    gearPos: 1,
    onGround: false,
    ownshipThrottle: 0,
    ownshipFlags2: 0,
    gStress: 0,
    redout: 0,
    aimAz: 0,
    aimEl: 0,
    bodyCount: 0,
    bodyPosBf: new Float64Array(MAX_BODIES * 3),
    bodyQuat: new Float32Array(MAX_BODIES * 4),
    combatFlags: 0,
    seeker: 0,
    inhibit: 0,
    inhibitAge: 0,
    rounds: 0,
    missilesLeft: 0,
    tgtRange: 0,
    tgtClosure: 0,
    health: 0,
    pipperBf: new Float64Array(3),
    banditLife: 0,
    banditSpawnSeq: 0,
    banditPosBf: new Float64Array(3),
    banditQuat: new Float32Array([1, 0, 0, 0]),
    banditVelBf: new Float32Array(3),
    banditElevatorRad: 0,
    banditAileronRad: 0,
    banditRudderRad: 0,
    banditFlapRad: 0,
    banditAirbrake: 0,
    banditGearPos: 0,
    banditAb: 0,
    missileCount: 0,
    missilePosBf: new Float64Array(MISSILES * 3),
    missileVelBf: new Float32Array(MISSILES * 3),
    missileState: new Uint8Array(MISSILES),
    tracerCount: 0,
    tracerPosBf: new Float64Array(TRACERS * 3),
    tracerVelBf: new Float32Array(TRACERS * 3),
    explosionCount: 0,
    explosionPosBf: new Float64Array(EXPLOSIONS * 3),
    explosionAge: new Float32Array(EXPLOSIONS),
    explosionKind: new Uint8Array(EXPLOSIONS),
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
  readonly #aim = new Float32Array(2);
  readonly #surf = new Float32Array(3);
  readonly #v3 = new Float32Array(3);
  /** Where each round's tracer is in the older snapshot's list, by `TR_ID` (-1: not there). */
  readonly #tracerAt = new Int16Array(TRACER_IDS).fill(-1);

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
    s.arena = cur.arena();
    s.publishTimeMs = cur.publishTimeMs();
    s.inputTimeMs = cur.inputTimeMs();
    s.ownshipSpeed = cur.ownshipTas();
    cur.ownshipVel(s.ownshipVelBf);
    s.groundSpeed = cur.ownshipGroundSpeed();
    s.gLagged = cur.ownshipGLagged();
    s.alpha = cur.ownshipAlpha();
    s.alphaWarn = cur.ownshipAlphaWarn();
    s.abLevel = cur.ownshipAbLevel();
    cur.ownshipSurfaces(this.#surf);
    s.elevatorRad = this.#surf[0] ?? 0;
    s.aileronRad = this.#surf[1] ?? 0;
    s.rudderRad = this.#surf[2] ?? 0;
    s.flapRad = cur.ownshipTef();
    s.airbrake = cur.ownshipAirbrake();
    s.gearPos = cur.ownshipGearPos();
    s.ownshipThrottle = cur.ownshipThrottle();
    // What the pilot's body does to the picture changes slowly and is not blended.
    s.ownshipFlags2 = cur.ownshipFlags2();
    s.onGround = (s.ownshipFlags2 & OWNSHIP_FLAG2.WOW) !== 0;
    s.gStress = cur.ownshipGStress();
    s.redout = cur.ownshipRedout();
    cur.ownshipAim(this.#aim);
    s.aimAz = this.#aim[0] ?? 0;
    s.aimEl = this.#aim[1] ?? 0;
    s.bodyCount = Math.min(cur.bodyCount(), MAX_BODIES);

    // The ownship.
    cur.ownshipPos(this.#b3);
    cur.ownshipQuat(this.#b4);
    if (alpha < 1) {
      prev.ownshipPos(this.#a3);
      prev.ownshipQuat(this.#a4);
      if (distance(this.#a3, this.#b3) > this.teleportM) {
        st.snapped = true;
        s.ownshipPosBf.set(this.#b3);
        s.ownshipQuat.set(this.#b4);
      } else {
        lerp3(s.ownshipPosBf, 0, this.#a3, 0, this.#b3, 0, alpha);
        nlerp(s.ownshipQuat, 0, this.#a4, 0, this.#b4, 0, alpha);
      }
    } else {
      s.ownshipPosBf.set(this.#b3);
      s.ownshipQuat.set(this.#b4);
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

    this.#combat(cur, prev, alpha);
    return s;
  }

  /** The duel's part of the state (see the file comment for what blends). */
  #combat(cur: SnapshotReader, prev: SnapshotReader, alpha: number): void {
    const s = this.state;
    const st = this.status;
    const blend = alpha < 1;
    s.combatFlags = cur.combatFlags();
    s.seeker = cur.seekerMode();
    s.inhibit = cur.launchInhibit();
    s.inhibitAge = cur.launchInhibitAge();
    s.rounds = cur.gunRounds();
    s.missilesLeft = cur.missilesLeft();
    s.tgtRange = cur.targetRange();
    s.tgtClosure = cur.targetClosure();
    s.health = cur.playerHealth();

    // The pipper point, while both snapshots have one.
    cur.pipperPos(this.#b3);
    if (
      blend &&
      (s.combatFlags & COMBAT_FLAG.PIPPER) !== 0 &&
      (prev.combatFlags() & COMBAT_FLAG.PIPPER) !== 0
    ) {
      prev.pipperPos(this.#a3);
      lerp3(s.pipperBf, 0, this.#a3, 0, this.#b3, 0, alpha);
    } else {
      s.pipperBf.set(this.#b3);
    }

    // The bandit: blended unless it respawned or jumped.
    s.banditLife = cur.banditLife();
    s.banditSpawnSeq = cur.banditSpawnSeq();
    cur.banditPos(this.#b3);
    cur.banditQuat(this.#b4);
    cur.banditVel(s.banditVelBf);
    const same =
      blend &&
      s.banditLife !== BANDIT_LIFE.ABSENT &&
      prev.banditLife() !== BANDIT_LIFE.ABSENT &&
      prev.banditSpawnSeq() === s.banditSpawnSeq;
    if (same) {
      prev.banditPos(this.#a3);
      prev.banditQuat(this.#a4);
    }
    if (same && distance(this.#a3, this.#b3) <= this.teleportM) {
      lerp3(s.banditPosBf, 0, this.#a3, 0, this.#b3, 0, alpha);
      nlerp(s.banditQuat, 0, this.#a4, 0, this.#b4, 0, alpha);
    } else {
      if (same) st.snapped = true;
      s.banditPosBf.set(this.#b3);
      s.banditQuat.set(this.#b4);
    }
    cur.banditSurfaces(this.#surf);
    s.banditElevatorRad = this.#surf[0] ?? 0;
    s.banditAileronRad = this.#surf[1] ?? 0;
    s.banditRudderRad = this.#surf[2] ?? 0;
    s.banditFlapRad = cur.banditTef();
    s.banditAirbrake = cur.banditAirbrake();
    s.banditGearPos = cur.banditGearPos();
    s.banditAb = cur.banditAbLevel();

    // Missiles: a slot in flight in both snapshots blends.
    let m = 0;
    for (let i = 0; i < MISSILES; i++) {
      const state = cur.missileState(i);
      if (state === MISSILE_STATE.FREE) continue;
      cur.missilePos(i, this.#b3);
      cur.missileVel(i, this.#v3);
      if (blend && prev.missileState(i) !== MISSILE_STATE.FREE) {
        prev.missilePos(i, this.#a3);
        if (distance(this.#a3, this.#b3) <= this.teleportM) {
          lerp3(s.missilePosBf, m * 3, this.#a3, 0, this.#b3, 0, alpha);
        } else {
          s.missilePosBf.set(this.#b3, m * 3);
        }
      } else {
        s.missilePosBf.set(this.#b3, m * 3);
      }
      s.missileVelBf.set(this.#v3, m * 3);
      s.missileState[m] = state;
      m += 1;
    }
    s.missileCount = m;

    // Tracers: the same round in both snapshots blends.
    const at = this.#tracerAt;
    const prevCount = blend ? Math.min(prev.tracerCount(), TRACERS) : 0;
    for (let i = 0; i < prevCount; i++) at[prev.tracerId(i)] = i;
    const count = Math.min(cur.tracerCount(), TRACERS);
    for (let i = 0; i < count; i++) {
      cur.tracerPos(i, this.#b3);
      cur.tracerVel(i, this.#v3);
      const j = at[cur.tracerId(i)] ?? -1;
      if (j >= 0) {
        prev.tracerPos(j, this.#a3);
        lerp3(s.tracerPosBf, i * 3, this.#a3, 0, this.#b3, 0, alpha);
      } else {
        s.tracerPosBf.set(this.#b3, i * 3);
      }
      s.tracerVelBf.set(this.#v3, i * 3);
    }
    s.tracerCount = count;
    for (let i = 0; i < prevCount; i++) at[prev.tracerId(i)] = -1;

    // Explosions, from the newest.
    let x = 0;
    for (let i = 0; i < EXPLOSIONS; i++) {
      const kind = cur.explosionKind(i);
      if (kind === 0) continue;
      cur.explosionPos(i, this.#b3);
      s.explosionPosBf.set(this.#b3, x * 3);
      s.explosionAge[x] = cur.explosionAge(i);
      s.explosionKind[x] = kind;
      x += 1;
    }
    s.explosionCount = x;
  }
}

function distance(a: Float64Array, b: Float64Array): number {
  // Not Math.hypot, which boxes its arguments: this runs for every blended thing every frame.
  const x = (a[0] ?? 0) - (b[0] ?? 0);
  const y = (a[1] ?? 0) - (b[1] ?? 0);
  const z = (a[2] ?? 0) - (b[2] ?? 0);
  return Math.sqrt(x * x + y * y + z * z);
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
