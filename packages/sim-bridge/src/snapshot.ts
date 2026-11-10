/**
 * Snapshot v5: what the sim publishes each tick and the renderer draws from. Layout in
 * `layout.gen.ts` (from Rust); this file reads and stamps it.
 *
 * Positions are battlefield-frame `f64`, orientations `f32` quaternions `[w, x, y, z]`, bodies
 * are structure-of-arrays so a whole axis is contiguous. The ownship block is the player's
 * aircraft (its reference point, attitude, air data, engine and fuel) or, with no aircraft
 * content, the spectator (`OWNSHIP_FLAG.SPECTATOR`; air data zero). Angles are radians. v3 adds the
 * flight control system, gear, wheel, g-physiology and mouse-aim state (`O_ASSIST` onward); v4 the
 * arena, engine and limit fields the HUD and the audio need (`O_ARENA` onward), in the bytes v3
 * reserved. v5 (ADR-48) appends the combat block after the bodies: the player's weapons and cues
 * (`C_*`), one bandit (`BD_*`), four missiles, 48 tracers, 8 explosions and a ring of the last 32
 * combat events, all zero without combat.
 */
import { SNAPSHOT } from './layout.gen.ts';

/** Reads a snapshot in place. Accessors copy out; nothing here keeps a reference to a result. */
export class SnapshotReader {
  readonly bytes: Uint8Array;
  readonly #dv: DataView;

  constructor(bytes: Uint8Array) {
    if (bytes.byteLength < SNAPSHOT.SIZE) {
      throw new RangeError(
        `a snapshot is ${String(SNAPSHOT.SIZE)} bytes, got ${String(bytes.byteLength)}`,
      );
    }
    this.bytes = bytes;
    this.#dv = new DataView(bytes.buffer, bytes.byteOffset, SNAPSHOT.SIZE);
  }

  /** True if the magic and version are the ones this build reads. */
  valid(): boolean {
    return (
      this.#dv.getUint32(SNAPSHOT.H_MAGIC, true) === SNAPSHOT.MAGIC &&
      this.#dv.getUint32(SNAPSHOT.H_VERSION, true) === SNAPSHOT.VERSION
    );
  }

  tickBig(): bigint {
    return this.#dv.getBigUint64(SNAPSHOT.H_TICK, true);
  }

  /** The tick as a number (exact below 2^53 ticks, which is 4.7 million years). */
  tick(): number {
    return Number(this.tickBig());
  }

  /** When the host published this copy, milliseconds on the shared clock (0 if not stamped). */
  publishTimeMs(): number {
    return this.#dv.getFloat64(SNAPSHOT.H_PUBLISH_TIME_MS, true);
  }

  lastInputSeq(): number {
    return this.#dv.getUint32(SNAPSHOT.H_LAST_INPUT_SEQ, true);
  }

  bodyCount(): number {
    return this.#dv.getUint32(SNAPSHOT.H_BODY_COUNT, true);
  }

  /** `CORE_STATE` of the core when the snapshot was written. */
  coreState(): number {
    return this.#dv.getUint32(SNAPSHOT.H_CORE_STATE, true);
  }

  /** When the host sampled the newest input the core had applied, milliseconds. */
  inputTimeMs(): number {
    return this.#dv.getFloat64(SNAPSHOT.H_INPUT_T_MS, true);
  }

  /** Ownship position (the aircraft's reference point), BF metres. */
  ownshipPos(out: Float64Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat64(SNAPSHOT.O_POS_BF + 8 * i, true);
    }
  }

  /** Ownship attitude, body to BF, `[w, x, y, z]`. */
  ownshipQuat(out: Float32Array, at = 0): void {
    for (let i = 0; i < 4; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.O_QUAT + 4 * i, true);
    }
  }

  /** Ownship velocity, BF m/s. */
  ownshipVel(out: Float32Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.O_VEL_BF + 4 * i, true);
    }
  }

  /** True airspeed, m/s. */
  ownshipTas(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_TAS, true);
  }

  /** Calibrated airspeed, m/s. */
  ownshipCas(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_CAS, true);
  }

  ownshipMach(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_MACH, true);
  }

  /** Angle of attack, radians. */
  ownshipAlpha(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_ALPHA, true);
  }

  /** Sideslip, radians. */
  ownshipBeta(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_BETA, true);
  }

  /** Body-axis load factor, g (1 in level flight). */
  ownshipNz(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_NZ, true);
  }

  /** Throttle lever, 0 to 1. */
  ownshipThrottle(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_THROTTLE, true);
  }

  /** Net thrust, newtons. */
  ownshipThrust(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_THRUST, true);
  }

  /** Fuel on board, kg. */
  ownshipFuel(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_FUEL, true);
  }

  /** Mass, kg. */
  ownshipMass(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_MASS, true);
  }

  /** Body rates (roll, pitch, yaw), rad/s. */
  ownshipOmega(out: Float32Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.O_OMEGA + 4 * i, true);
    }
  }

  /** Elevator, aileron and rudder, radians. */
  ownshipSurfaces(out: Float32Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.O_SURFACES + 4 * i, true);
    }
  }

  /** Height above MSL, metres. */
  ownshipAltitude(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_ALTITUDE, true);
  }

  /** Afterburner stages burning. */
  ownshipAbStage(): number {
    return this.#dv.getUint8(SNAPSHOT.O_AB_STAGE);
  }

  /** `OWNSHIP_FLAG` bits. */
  ownshipFlags(): number {
    return this.#dv.getUint8(SNAPSHOT.O_FLAGS);
  }

  /** Assist level in force (`ASSIST_LEVEL`; 0 for the spectator). */
  ownshipAssist(): number {
    return this.#dv.getUint8(SNAPSHOT.O_ASSIST);
  }

  /** What the flight control system is doing (`FCS_MODE`). */
  ownshipFcsMode(): number {
    return this.#dv.getUint8(SNAPSHOT.O_FCS_MODE);
  }

  /** `OWNSHIP_FLAG2` bits. */
  ownshipFlags2(): number {
    return this.#dv.getUint16(SNAPSHOT.O_FLAGS2, true);
  }

  /** Spawn counter, wrapping at 256: changes whenever the aircraft is (re)spawned. */
  ownshipSpawnSeq(): number {
    return this.#dv.getUint8(SNAPSHOT.O_SPAWN_SEQ);
  }

  /** Flap detent: 0 up, 1 take-off, 2 landing. */
  ownshipFlapDetent(): number {
    return this.#dv.getUint8(SNAPSHOT.O_FLAP_DETENT);
  }

  /** Gear position, 0 up to 1 down. */
  ownshipGearPos(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_GEAR_POS, true);
  }

  /** Trailing-edge flap, radians. */
  ownshipTef(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_TEF, true);
  }

  /** Airbrake, 0 stowed to 1 open. */
  ownshipAirbrake(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_AIRBRAKE, true);
  }

  /** Left and right wheel brake commands, 0 to 1. */
  ownshipBrakes(out: Float32Array, at = 0): void {
    out[at] = this.#dv.getFloat32(SNAPSHOT.O_BRAKE_L, true);
    out[at + 1] = this.#dv.getFloat32(SNAPSHOT.O_BRAKE_R, true);
  }

  /** Nose-wheel steering angle, radians (positive turns right). */
  ownshipSteer(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_STEER, true);
  }

  /** Speed over the ground, m/s. */
  ownshipGroundSpeed(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_GROUND_SPEED, true);
  }

  /** Vertical speed, m/s (climb positive). */
  ownshipVs(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_VS, true);
  }

  /** Height of the lowest wheel above the ground, metres. */
  ownshipAgl(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_AGL, true);
  }

  /** The pilot's lagged ("heart-level") g. */
  ownshipGLagged(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_G_LAGGED, true);
  }

  /** G stress `S`: greyout from 0.35, blackout from 0.7, G-LOC at 1.0. */
  ownshipGStress(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_G_STRESS, true);
  }

  /** Red-out stress `R` (negative g): tint from 0.5, vision loss at 1.0. */
  ownshipRedout(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_REDOUT, true);
  }

  /** The flight control system's load-factor command, g. */
  ownshipNCmd(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_N_CMD, true);
  }

  /** Sink rate at the last touchdown, m/s. */
  ownshipTouchdownVs(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_TOUCHDOWN_VS, true);
  }

  /** The mouse-aim direction the core is using: azimuth and elevation, radians. */
  ownshipAim(out: Float32Array, at = 0): void {
    out[at] = this.#dv.getFloat32(SNAPSHOT.O_AIM_AZ, true);
    out[at + 1] = this.#dv.getFloat32(SNAPSHOT.O_AIM_EL, true);
  }

  /** Why the aircraft was destroyed (`CRASH_CAUSE`). */
  ownshipCrashCause(): number {
    return this.#dv.getUint8(SNAPSHOT.O_CRASH_CAUSE);
  }

  /** Per leg `i`: bit `2i` weight-on-wheels, bit `2i+1` collapsed; bit 6 lever down; bit 7 locked. */
  ownshipGearBits(): number {
    return this.#dv.getUint8(SNAPSHOT.O_GEAR_BITS);
  }

  /** The test arena the world is (`ARENA`). */
  arena(): number {
    return this.#dv.getUint8(SNAPSHOT.O_ARENA);
  }

  /** `ENGINE_BIT` bits of the first engine. */
  ownshipEngineBits(): number {
    return this.#dv.getUint8(SNAPSHOT.O_ENGINE_BITS);
  }

  /** Dynamic pressure, Pa. */
  ownshipQbar(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_QBAR, true);
  }

  /** Total fuel flow, kg/s. */
  ownshipFuelFlow(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_FUEL_FLOW, true);
  }

  /** Core thrust of the first engine as a fraction of military thrust. */
  ownshipEngineCore(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_ENGINE_CORE, true);
  }

  /** Afterburner level of the first engine, 0 to 1. */
  ownshipAbLevel(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_AB_LEVEL, true);
  }

  /** The highest load factor since the last spawn, g. */
  ownshipMaxNz(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_MAX_NZ, true);
  }

  /** The positive load limit for the configuration the aircraft is in, g. */
  ownshipNMax(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_N_MAX, true);
  }

  /** The angle of attack at which the stall warning starts, radians. */
  ownshipAlphaWarn(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_ALPHA_WARN, true);
  }

  // --- v5: the combat block (ADR-48). All zero when the session has no combat. ---

  /** `COMBAT_FLAG` bits. */
  combatFlags(): number {
    return this.#dv.getUint16(SNAPSHOT.C_FLAGS, true);
  }

  /** The next missile's seeker (`SEEKER_MODE`). */
  seekerMode(): number {
    return this.#dv.getUint8(SNAPSHOT.C_SEEKER);
  }

  /** Why the last launch was refused (`LAUNCH_INHIBIT`; 0 none). */
  launchInhibit(): number {
    return this.#dv.getUint8(SNAPSHOT.C_INHIBIT);
  }

  /** How long ago that refusal was, seconds. */
  launchInhibitAge(): number {
    return this.#dv.getFloat32(SNAPSHOT.C_INHIBIT_AGE, true);
  }

  /** Gun rounds left. */
  gunRounds(): number {
    return this.#dv.getUint16(SNAPSHOT.C_ROUNDS, true);
  }

  /** Missiles left on the rails. */
  missilesLeft(): number {
    return this.#dv.getUint8(SNAPSHOT.C_MISSILES);
  }

  /** Bandits present (0 or 1). */
  banditCount(): number {
    return this.#dv.getUint8(SNAPSHOT.C_BANDITS);
  }

  /** The newest combat event's sequence number (0: none yet). */
  eventSeq(): number {
    return this.#dv.getUint32(SNAPSHOT.C_EVENT_SEQ, true);
  }

  /** The gunsight's aim point, BF metres (meaningful while `COMBAT_FLAG.PIPPER` is set). */
  pipperPos(out: Float64Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat64(SNAPSHOT.C_PIPPER_BF + 8 * i, true);
    }
  }

  /** The seeker's signal ratio, `(I / I_ref)(R_ref / R)²` (1 at the lock range; can exceed 1). */
  seekerSignal(): number {
    return this.#dv.getFloat32(SNAPSHOT.C_SIGNAL, true);
  }

  /** The designated target's range (m), closure (m/s) and aspect (rad); zero without one. */
  targetRange(): number {
    return this.#dv.getFloat32(SNAPSHOT.C_TGT_RANGE, true);
  }

  targetClosure(): number {
    return this.#dv.getFloat32(SNAPSHOT.C_TGT_CLOSURE, true);
  }

  targetAspect(): number {
    return this.#dv.getFloat32(SNAPSHOT.C_TGT_ASPECT, true);
  }

  /** The player's health, 0 to 1. */
  playerHealth(): number {
    return this.#dv.getFloat32(SNAPSHOT.C_HEALTH, true);
  }

  /**
   * Combat counter `k` (0 to 11, `COMBAT_COUNTER` in `combat-status.ts`): rounds fired, round hits, missiles
   * fired, missile hits, kills, gun kills, missile kills, deaths, hits taken, the bandit's rounds,
   * explosions, bandit spawns.
   */
  combatCounter(k: number): number {
    return this.#dv.getUint32(SNAPSHOT.C_ROUNDS_FIRED + 4 * k, true);
  }

  /** The bandit's reference point, BF metres. */
  banditPos(out: Float64Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat64(SNAPSHOT.BD_POS_BF + 8 * i, true);
    }
  }

  /** The bandit's attitude, body to BF, `[w, x, y, z]`. */
  banditQuat(out: Float32Array, at = 0): void {
    for (let i = 0; i < 4; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.BD_QUAT + 4 * i, true);
    }
  }

  /** The bandit's velocity, BF m/s. */
  banditVel(out: Float32Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.BD_VEL_BF + 4 * i, true);
    }
  }

  /** The bandit's elevator, aileron and rudder, radians. */
  banditSurfaces(out: Float32Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.BD_SURFACES + 4 * i, true);
    }
  }

  /** The bandit's trailing-edge flap, radians. */
  banditTef(): number {
    return this.#dv.getFloat32(SNAPSHOT.BD_TEF, true);
  }

  banditAirbrake(): number {
    return this.#dv.getFloat32(SNAPSHOT.BD_AIRBRAKE, true);
  }

  banditGearPos(): number {
    return this.#dv.getFloat32(SNAPSHOT.BD_GEAR_POS, true);
  }

  banditAbLevel(): number {
    return this.#dv.getFloat32(SNAPSHOT.BD_AB_LEVEL, true);
  }

  /** The bandit's health, 0 to 1. */
  banditHealth(): number {
    return this.#dv.getFloat32(SNAPSHOT.BD_HEALTH, true);
  }

  /** The bandit's true airspeed, m/s. */
  banditTas(): number {
    return this.#dv.getFloat32(SNAPSHOT.BD_TAS, true);
  }

  /** `BANDIT_LIFE`. */
  banditLife(): number {
    return this.#dv.getUint8(SNAPSHOT.BD_LIFE);
  }

  /** `BANDIT_AI`: its state. */
  banditAi(): number {
    return this.#dv.getUint8(SNAPSHOT.BD_AI);
  }

  /** Its skill rank, 1 to 3. */
  banditSkill(): number {
    return this.#dv.getUint8(SNAPSHOT.BD_SKILL);
  }

  /** `BANDIT_FLAG` bits. */
  banditFlags(): number {
    return this.#dv.getUint8(SNAPSHOT.BD_FLAGS);
  }

  /** Bandits spawned, wrapping at 2^16. */
  banditSpawnSeq(): number {
    return this.#dv.getUint16(SNAPSHOT.BD_SPAWN_SEQ, true);
  }

  /** Missile slot `i` (0 to 3): position, BF metres. */
  missilePos(i: number, out: Float64Array, at = 0): void {
    const base = SNAPSHOT.MISSILES + SNAPSHOT.MISSILE_STRIDE * i + SNAPSHOT.MS_POS_BF;
    for (let k = 0; k < 3; k++) {
      out[at + k] = this.#dv.getFloat64(base + 8 * k, true);
    }
  }

  /** Missile slot `i`: velocity, BF m/s. */
  missileVel(i: number, out: Float32Array, at = 0): void {
    const base = SNAPSHOT.MISSILES + SNAPSHOT.MISSILE_STRIDE * i + SNAPSHOT.MS_VEL_BF;
    for (let k = 0; k < 3; k++) {
      out[at + k] = this.#dv.getFloat32(base + 4 * k, true);
    }
  }

  /** Missile slot `i`: `MISSILE_STATE` (0: the slot is free). */
  missileState(i: number): number {
    return this.#dv.getUint8(SNAPSHOT.MISSILES + SNAPSHOT.MISSILE_STRIDE * i + SNAPSHOT.MS_STATE);
  }

  /** Missile slot `i`: who launched it (`COMBAT_ACTOR`). */
  missileOwner(i: number): number {
    return this.#dv.getUint8(SNAPSHOT.MISSILES + SNAPSHOT.MISSILE_STRIDE * i + SNAPSHOT.MS_OWNER);
  }

  /** Tracers written this tick (the first `tracerCount()` entries are valid). */
  tracerCount(): number {
    return this.#dv.getUint32(SNAPSHOT.TR_COUNT, true);
  }

  /** Tracer `i`: position, BF metres. */
  tracerPos(i: number, out: Float64Array, at = 0): void {
    const base = SNAPSHOT.TR_FIRST + SNAPSHOT.TRACER_STRIDE * i + SNAPSHOT.TR_POS_BF;
    for (let k = 0; k < 3; k++) {
      out[at + k] = this.#dv.getFloat64(base + 8 * k, true);
    }
  }

  /** Tracer `i`: velocity, BF m/s. */
  tracerVel(i: number, out: Float32Array, at = 0): void {
    const base = SNAPSHOT.TR_FIRST + SNAPSHOT.TRACER_STRIDE * i + SNAPSHOT.TR_VEL_BF;
    for (let k = 0; k < 3; k++) {
      out[at + k] = this.#dv.getFloat32(base + 4 * k, true);
    }
  }

  /** Tracer `i`: who fired it (`COMBAT_ACTOR`). */
  tracerOwner(i: number): number {
    return this.#dv.getUint8(SNAPSHOT.TR_FIRST + SNAPSHOT.TRACER_STRIDE * i + SNAPSHOT.TR_OWNER);
  }

  /** Tracer `i`: its round's pool slot, stable while the round flies (for trails). */
  tracerId(i: number): number {
    return this.#dv.getUint8(SNAPSHOT.TR_FIRST + SNAPSHOT.TRACER_STRIDE * i + SNAPSHOT.TR_ID);
  }

  /** Explosion slot `i` (0 to 7): position, BF metres. */
  explosionPos(i: number, out: Float64Array, at = 0): void {
    const base = SNAPSHOT.EXPLOSIONS + SNAPSHOT.EXPLOSION_STRIDE * i + SNAPSHOT.XP_POS_BF;
    for (let k = 0; k < 3; k++) {
      out[at + k] = this.#dv.getFloat64(base + 8 * k, true);
    }
  }

  /** Explosion slot `i`: its age, seconds. */
  explosionAge(i: number): number {
    return this.#dv.getFloat32(
      SNAPSHOT.EXPLOSIONS + SNAPSHOT.EXPLOSION_STRIDE * i + SNAPSHOT.XP_AGE,
      true,
    );
  }

  /** Explosion slot `i`: `EXPLOSION_KIND` (0: the slot is empty). */
  explosionKind(i: number): number {
    return this.#dv.getUint8(
      SNAPSHOT.EXPLOSIONS + SNAPSHOT.EXPLOSION_STRIDE * i + SNAPSHOT.XP_KIND,
    );
  }

  /** Event ring entry `slot` (0 to 31): its sequence number (0: empty). */
  eventSeqAt(slot: number): number {
    return this.#dv.getUint32(
      SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * slot + SNAPSHOT.EV_SEQ,
      true,
    );
  }

  /** Event ring entry `slot`: the tick it happened, mod 2^32. */
  eventTickAt(slot: number): number {
    return this.#dv.getUint32(
      SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * slot + SNAPSHOT.EV_TICK,
      true,
    );
  }

  /** Event ring entry `slot`: `COMBAT_EVENT`. */
  eventKindAt(slot: number): number {
    return this.#dv.getUint8(SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * slot + SNAPSHOT.EV_KIND);
  }

  /** Event ring entry `slot`: who did it (`COMBAT_ACTOR`). */
  eventActorAt(slot: number): number {
    return this.#dv.getUint8(SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * slot + SNAPSHOT.EV_ACTOR);
  }

  /** Event ring entry `slot`: to whom (`COMBAT_ACTOR`). */
  eventTargetAt(slot: number): number {
    return this.#dv.getUint8(SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * slot + SNAPSHOT.EV_TARGET);
  }

  /** Event ring entry `slot`: the kind's detail (`KILL_WEAPON`, `CRASH_CAUSE`, ...). */
  eventArgAt(slot: number): number {
    return this.#dv.getUint8(SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * slot + SNAPSHOT.EV_ARG);
  }

  /** Event ring entry `slot`: `MISSILE_HIT` the miss distance, KILL the range, metres. */
  eventValueAt(slot: number): number {
    return this.#dv.getFloat32(
      SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * slot + SNAPSHOT.EV_VALUE,
      true,
    );
  }

  /** Body `i` position, BF metres. */
  bodyPos(i: number, out: Float64Array, at = 0): void {
    out[at] = this.#dv.getFloat64(SNAPSHOT.B_X + 8 * i, true);
    out[at + 1] = this.#dv.getFloat64(SNAPSHOT.B_Y + 8 * i, true);
    out[at + 2] = this.#dv.getFloat64(SNAPSHOT.B_Z + 8 * i, true);
  }

  /** Body `i` orientation `[w, x, y, z]`. */
  bodyQuat(i: number, out: Float32Array, at = 0): void {
    for (let k = 0; k < 4; k++) {
      out[at + k] = this.#dv.getFloat32(SNAPSHOT.B_QUAT + 16 * i + 4 * k, true);
    }
  }
}

/** Writes the host's publish time into a snapshot copy. Used by the Sim Worker only. */
export function stampPublishTime(bytes: Uint8Array, ms: number): void {
  new DataView(bytes.buffer, bytes.byteOffset, SNAPSHOT.SIZE).setFloat64(
    SNAPSHOT.H_PUBLISH_TIME_MS,
    ms,
    true,
  );
}
