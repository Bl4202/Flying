/**
 * The flight audio's synthesis, as a pure function of numbers (ADR-42, GDD 56): engine, wind, buffet,
 * tyre rumble and the warning tones, mixed into an aircraft bus (ducked under a warning) and a
 * warnings bus (never ducked), then through the cockpit or chase mix, the master gain and a soft
 * limiter. There are no samples: every sound is made from the numbers of the `audio_profile/1` and
 * the state fed to `params`.
 *
 * It runs inside an `AudioWorkletProcessor`, but this file knows nothing of Web Audio: the same code
 * renders offline in Node, which is how the loudness, spectrum, level and timing gates are tested.
 * `process` makes no garbage: all state is in typed arrays, read into locals for a block and written
 * back, so a render quantum allocates nothing (A7).
 *
 * Levels are decibels relative to the master bus (`audio_profile/1`). Sources:
 *
 * - **Engine** (GDD 56.3, a hybrid, no samples): a spool whose speed follows the engine's core thrust
 *   through the profile's curve with the profile's time constant; a blade-pass tone at
 *   `rpm / 60 * blade_count` with harmonics and a shaft tone (their level rises as rpm^5); turbine hiss; a jet roar of coloured
 *   noise whose level is `jet_level_db + 40 log10(thrust fraction)` (exhaust speed goes as the
 *   square root of thrust, noise power as its eighth power); the afterburner raises the whole engine
 *   by `burner_roar_db` and lights with a thump. Flameout and relight follow the engine bits.
 * - **Wind** is band-passed noise whose level rises `wind_slope_db` per decade of dynamic pressure.
 * - **Buffet** is a low rumble pulsing at `buffet_hz`, from `buffet_margin_deg` below the stall warning.
 * - **Tyres** rumble in proportion to ground speed on the wheels.
 * - **Tones** (GDD 56.7): the stall horn pulses faster as the angle of attack passes the warning angle,
 *   the PULL UP warble alternates two pitches, the over-g tone beeps, the gear chime and the master
 *   caution chime repeat. The most urgent warning is the one heard.
 * - **The duel** (ADR-42 amendment 1, from the profile's `weapons` block): the gun (a pulse per round:
 *   a crack of band-passed noise and a decaying low sine), the bandit's gun (the same, low-passed and
 *   quieter with distance), a missile launch (a decaying rush of band-passed noise), a hit taken (a
 *   short click) and an explosion (a thump and a rumble, quieter with distance) on the aircraft bus;
 *   the seeker's growl (its pitch rising with the signal) and the lock tone on the warnings bus. The
 *   one-shot sounds start when the session's counters rise; the first values after a reset are
 *   adopted without a sound. Their levels are relative to the master, like the warning tones'.
 */
import { WARNING } from '@flying/sim-bridge/warnings';
import { SEEKER_MODE } from '@flying/sim-bridge/layout';
import type { AudioProfile, WeaponsProfile } from '../profile.ts';
import { blockCoefficient, Biquad, dbToLin, fillNoise5, noiseState } from './primitives.ts';

/** Indices into `FlightDsp.params`: the state the page feeds the synthesis. */
export const P = {
  /** Core thrust as a fraction of military, 0 to 1. */
  CORE: 0,
  /** Afterburner level, 0 to 1. */
  AB_LEVEL: 1,
  /** `ENGINE_BIT` bits: running 1, afterburner lit 2, compressor stalled 4, relighting 8. */
  ENGINE_BITS: 2,
  /** Dynamic pressure, Pa. */
  QBAR: 3,
  /** Angle of attack and the angle where the stall warning starts, radians. */
  ALPHA: 4,
  ALPHA_WARN: 5,
  /** Airbrake, 0 to 1. */
  AIRBRAKE: 6,
  /** Speed over the ground, m/s, and 1 if the weight is on the wheels. */
  GROUND_SPEED: 7,
  ON_GROUND: 8,
  /** The `WARNING` bits that are on. */
  WARNINGS: 9,
  /** 1 for the cockpit mix (outside sound through the canopy), 0 for the chase mix. */
  COCKPIT: 10,
  /** The master volume, linear 0 to 1 (the player's setting). */
  MASTER: 11,
  /** The aircraft and warning bus volumes, linear 0 to 1 (the player's settings). */
  AIRCRAFT_BUS: 12,
  WARNINGS_BUS: 13,
  /** 1 while the player's gun fires. */
  GUN_FIRING: 14,
  /** How far away the bandit's gun is firing, m; 0 while it is not. */
  ENEMY_GUN_DIST_M: 15,
  /** The next missile's `SEEKER_MODE`, and its signal ratio (1 at the lock range). */
  SEEKER: 16,
  SIGNAL: 17,
  /** The session's missiles launched, hits taken and explosions: a sound when one rises. */
  LAUNCHES: 18,
  HITS_TAKEN: 19,
  EXPLOSIONS: 20,
  /** How far away the newest explosion is, m (0 when not known: heard at its reference distance). */
  EXPLOSION_DIST_M: 21,
} as const;

export const PARAM_COUNT = 22;

/**
 * The names the page and the worklet know the parameters by: the worklet's `AudioParam`s, in the
 * order of `P`. Each is k-rate; a bitmask is set, never ramped, so no intermediate value is heard.
 */
export const PARAM_NAMES = [
  'core',
  'abLevel',
  'engineBits',
  'qbar',
  'alpha',
  'alphaWarn',
  'airbrake',
  'groundSpeed',
  'onGround',
  'warnings',
  'cockpit',
  'master',
  'aircraftBus',
  'warningsBus',
  'gunFiring',
  'enemyGunDist',
  'seeker',
  'signal',
  'launches',
  'hitsTaken',
  'explosions',
  'explosionDist',
] as const;

/** `[min, max, default]` of each parameter, in the order of `P` (an `AudioParam` clamps to them). */
export const PARAM_RANGE: readonly (readonly [number, number, number])[] = [
  [0, 1, 0],
  [0, 1, 0],
  [0, 15, 0],
  [0, 1e6, 0],
  [-3.2, 3.2, 0],
  [-3.2, 3.2, 0],
  [0, 1, 0],
  [0, 2000, 0],
  [0, 1, 0],
  [0, 1023, 0],
  [0, 1, 1],
  [0, 1, 1],
  [0, 1, 1],
  [0, 1, 1],
  [0, 1, 0],
  [0, 1e6, 0],
  [0, 3, 0],
  [0, 1000, 0],
  // The counters: an AudioParam is a 32-bit float, exact to 2^24.
  [0, 2 ** 24, 0],
  [0, 2 ** 24, 0],
  [0, 2 ** 24, 0],
  [0, 1e7, 0],
];

/** The largest block `process` is asked for (the Web Audio quantum is 128). */
export const MAX_BLOCK = 512;

/** What counts as a continuous warning that ducks the aircraft bus. */
export const DUCKING_BITS =
  WARNING.PULLUP | WARNING.STALL | WARNING.DEPARTED | WARNING.OVERSTRESS | WARNING.G;
/** The warnings the over-g tone sounds for. */
const OVER_G_BITS = WARNING.OVERSTRESS | WARNING.G;
/** The cautions the master caution chime sounds for. */
const CAUTION_BITS = WARNING.ALPHA | WARNING.OVERSPEED | WARNING.GEAR_SPEED | WARNING.FLAP_SPEED;

/** Which tone is sounding. */
export const TONE = { NONE: 0, PULL_UP: 1, STALL: 2, OVER_G: 3, GEAR: 4, CAUTION: 5 } as const;

// State slots (Float64Array).
const S_RPM = 0;
const S_CORE = 1;
const S_ENGINE_GAIN = 2;
const S_PH_SHAFT = 3;
const S_THUMP_ENV = 8;
const S_THUMP_PH = 9;
const S_PREV_AB = 10;
const S_BUFFET_PH = 11;
const S_TONE_PH = 12;
const S_PULSE_PH = 13;
const S_CHIME_T = 14;
const S_LAST_TONE = 16;
const S_DUCK = 17;
const S_HUM_PH = 18;
const S_WIND_G = 19;
const S_TYRE_G = 20;
const S_BUFFET_G = 21;
const S_COCKPIT = 22;
const S_BLADE_PH1 = 24;
const S_BLADE_PH2 = 25;
const S_BLADE_PH3 = 26;
const S_BLADE_PH4 = 27;
// The duel's voices.
const S_GUN_ACC = 32;
const S_GUN_SINCE = 33;
const S_GUN_BODY = 34;
const S_GUN_BODY_PH = 35;
const S_GUN_PREV = 36;
const S_EN_ACC = 37;
const S_EN_SINCE = 38;
const S_EN_BODY = 39;
const S_EN_BODY_PH = 40;
const S_EN_PREV = 41;
const S_LAUNCH_ENV = 42;
const S_HIT_SINCE = 43;
const S_BOOM_ENV = 44;
const S_BOOM_PH = 45;
const S_BOOM_AMP = 46;
const S_GROWL_PH = 47;
const S_GROWL_AM_PH = 48;
const S_GROWL_G = 49;
const S_LOCK_PH = 50;
const S_LOCK_G = 51;
const S_SEEN = 52;
const S_LAUNCHES = 53;
const S_HITS = 54;
const S_BOOMS = 55;
const S_STATE = 64;

// The duel's amplitudes (`#amp`).
const A_GUN = 0;
const A_ENEMY = 1;
const A_LAUNCH = 2;
const A_HIT = 3;
const A_GROWL = 4;
const A_LOCK = 5;
const A_BOOM = 6;

/** Long ago: a one-shot voice whose clock reads this is silent. */
const LONG_AGO_S = 1e9;
// ALGO: band-passed white noise keeps only a slice of its power, so the noise voices are scaled up to
// sit near their data level; the decaying sines are at it.
const CRACK_GAIN = 3;
const LAUNCH_GAIN = 2.5;
const HIT_GAIN = 3;
const BOOM_NOISE = 1.5;
/** How fast each pulse's low sine dies away, s (about a pulse's length). */
const GUN_BODY_S = 0.012;
/** The growl's amplitude modulation depth. */
const GROWL_AM = 0.35;

const TWO_PI = 2 * Math.PI;

/** The shaft speed for a core thrust fraction, by the profile's curve (piecewise linear, clamped). */
export function rpmFor(curve: readonly (readonly [number, number])[], core: number): number {
  const first = curve[0];
  if (first === undefined) return 0;
  if (core <= first[0]) return first[1];
  for (let i = 1; i < curve.length; i++) {
    const a = curve[i - 1] as readonly [number, number];
    const b = curve[i] as readonly [number, number];
    if (core <= b[0]) {
      const t = (core - a[0]) / Math.max(b[0] - a[0], 1e-9);
      return a[1] + (b[1] - a[1]) * t;
    }
  }
  return (curve[curve.length - 1] as readonly [number, number])[1];
}

export class FlightDsp {
  /** The state to synthesise: write it before each block (indices `P`). */
  readonly params = new Float64Array(PARAM_COUNT);
  /** The tone sounding now (`TONE`), and how far the aircraft bus is ducked, dB (negative). */
  tone: number = TONE.NONE;
  duckDb = 0;
  /** Set when a warning tone has just started: the frame count since the last block start, or -1. */
  onsetFrame = -1;

  readonly #profile: AudioProfile;
  readonly #fs: number;
  readonly #s = new Float64Array(S_STATE);
  readonly #noise = noiseState(0x1234_5678);
  // Filters.
  readonly #roarLow: Biquad;
  readonly #formants: Biquad[];
  readonly #hissHigh: Biquad;
  readonly #windBand: Biquad;
  readonly #buffetLow: Biquad;
  readonly #tyreLow: Biquad;
  readonly #canopy: Biquad;
  // Per-block scratch: the noise draws in five planes (engine, hiss, wind, tyres, buffet), filtered
  // in place, and the outside mix before and after the canopy.
  readonly #planes = new Float64Array(MAX_BLOCK * 5);
  readonly #white = this.#planes.subarray(0, MAX_BLOCK);
  readonly #hiss = this.#planes.subarray(MAX_BLOCK, 2 * MAX_BLOCK);
  readonly #windN = this.#planes.subarray(2 * MAX_BLOCK, 3 * MAX_BLOCK);
  readonly #tyreN = this.#planes.subarray(3 * MAX_BLOCK, 4 * MAX_BLOCK);
  readonly #buffetN = this.#planes.subarray(4 * MAX_BLOCK, 5 * MAX_BLOCK);
  readonly #outside = new Float64Array(MAX_BLOCK);
  readonly #canopied = new Float64Array(MAX_BLOCK);
  readonly #aircraft = new Float32Array(MAX_BLOCK);
  readonly #harmonics: Float64Array;
  readonly #rpmCurve: readonly (readonly [number, number])[];
  // The duel's voices: their own noise (so the flight voices' noise is the same with or without
  // them), five planes (crack, enemy crack, launch, hit, explosion), their filters, and what they make
  // for each bus.
  readonly #weapons: WeaponsProfile | undefined;
  readonly #combatNoise = noiseState(0x2545_f491);
  readonly #combatPlanes = new Float64Array(MAX_BLOCK * 5);
  readonly #crackN = this.#combatPlanes.subarray(0, MAX_BLOCK);
  readonly #enemyN = this.#combatPlanes.subarray(MAX_BLOCK, 2 * MAX_BLOCK);
  readonly #launchN = this.#combatPlanes.subarray(2 * MAX_BLOCK, 3 * MAX_BLOCK);
  readonly #hitN = this.#combatPlanes.subarray(3 * MAX_BLOCK, 4 * MAX_BLOCK);
  readonly #boomN = this.#combatPlanes.subarray(4 * MAX_BLOCK, 5 * MAX_BLOCK);
  readonly #enemy = new Float64Array(MAX_BLOCK);
  readonly #combatAir = new Float64Array(MAX_BLOCK);
  readonly #combatWarn = new Float64Array(MAX_BLOCK);
  readonly #crackBand: Biquad;
  readonly #enemyBand: Biquad;
  readonly #enemyLow: Biquad;
  readonly #launchBand: Biquad;
  readonly #hitBand: Biquad;
  readonly #boomLow: Biquad;
  /** Whether the duel's voices made anything this block (when not, they are not mixed). */
  #combatOn = false;
  /**
   * The duel's levels as linear amplitudes (`A_*`), worked out once: a number returned from a call
   * per block would be boxed (A7).
   */
  readonly #amp = new Float64Array(7);

  constructor(profile: AudioProfile, sampleRate = 48_000) {
    this.#profile = profile;
    this.#fs = sampleRate;
    const e = profile.engine;
    const a = profile.aero;
    this.#roarLow = new Biquad('lowpass', sampleRate, 2200, 0.7);
    this.#formants = e.formants.map((f) => new Biquad('peaking', sampleRate, f.fHz, f.q, f.gainDb));
    this.#hissHigh = new Biquad('highpass', sampleRate, 3800, 0.7);
    const centre = Math.sqrt(a.windLowHz * a.windHighHz);
    this.#windBand = new Biquad(
      'bandpass',
      sampleRate,
      centre,
      centre / (a.windHighHz - a.windLowHz),
    );
    this.#buffetLow = new Biquad('lowpass', sampleRate, 90, 0.7);
    this.#tyreLow = new Biquad('lowpass', sampleRate, 420, 0.7);
    this.#canopy = new Biquad('lowpass', sampleRate, profile.cockpit.canopyLowpassHz, 0.7);
    // The blade-pass tone's first four harmonics; a profile with fewer has silence above them
    // (and the fundamental at full level if it lists none).
    this.#harmonics = new Float64Array(4);
    for (let i = 0; i < 4; i++) {
      const db = e.harmonicLevelsDb[i];
      this.#harmonics[i] = db === undefined ? (i === 0 ? 1 : 0) : dbToLin(db);
    }
    this.#rpmCurve = e.rpmByCore;
    // The duel's filters (placeholders at 1 kHz when the profile has no weapons: never run).
    const wp = profile.weapons;
    this.#weapons = wp;
    this.#crackBand = new Biquad('bandpass', sampleRate, wp?.gun.crackHz ?? 1000, 1.5);
    this.#enemyBand = new Biquad('bandpass', sampleRate, wp?.gun.crackHz ?? 1000, 1.5);
    this.#enemyLow = new Biquad('lowpass', sampleRate, wp?.enemyGun.lowpassHz ?? 1000, 0.7);
    const lo = wp?.launch.lowHz ?? 500;
    const hi = wp?.launch.highHz ?? 2000;
    const mid = Math.sqrt(lo * hi);
    this.#launchBand = new Biquad('bandpass', sampleRate, mid, mid / Math.max(hi - lo, 1));
    this.#hitBand = new Biquad('bandpass', sampleRate, wp?.hit.bandHz ?? 1000, 2);
    this.#boomLow = new Biquad('lowpass', sampleRate, 600, 0.7);
    if (wp !== undefined) {
      const amp = this.#amp;
      amp[A_GUN] = dbToLin(wp.gun.levelDb);
      amp[A_ENEMY] = dbToLin(wp.enemyGun.levelDb);
      amp[A_LAUNCH] = dbToLin(wp.launch.levelDb) * LAUNCH_GAIN;
      amp[A_HIT] = dbToLin(wp.hit.levelDb) * HIT_GAIN;
      amp[A_GROWL] = dbToLin(wp.growl.levelDb);
      amp[A_LOCK] = dbToLin(wp.lockTone.levelDb);
      amp[A_BOOM] = dbToLin(wp.explosion.levelDb);
    }
    this.reset();
  }

  /** Back to silence at idle: nothing sounding, the engine stopped. */
  reset(): void {
    const s = this.#s;
    s.fill(0);
    s[S_RPM] = rpmFor(this.#rpmCurve, 0);
    s[S_COCKPIT] = 1;
    s[S_ENGINE_GAIN] = 0;
    for (const f of [
      this.#roarLow,
      this.#hissHigh,
      this.#windBand,
      this.#buffetLow,
      this.#tyreLow,
      this.#canopy,
    ]) {
      f.reset();
    }
    for (const f of this.#formants) f.reset();
    for (const f of [
      this.#crackBand,
      this.#enemyBand,
      this.#enemyLow,
      this.#launchBand,
      this.#hitBand,
      this.#boomLow,
    ]) {
      f.reset();
    }
    s[S_GUN_SINCE] = LONG_AGO_S;
    s[S_EN_SINCE] = LONG_AGO_S;
    s[S_HIT_SINCE] = LONG_AGO_S;
    this.#combatOn = false;
    this.params.fill(0);
    this.params[P.MASTER] = 1;
    this.params[P.AIRCRAFT_BUS] = 1;
    this.params[P.WARNINGS_BUS] = 1;
    this.params[P.COCKPIT] = 1;
    this.tone = TONE.NONE;
    this.duckDb = 0;
    this.onsetFrame = -1;
  }

  /** The engine's shaft speed now, rpm (for tests and the overlay). */
  get rpm(): number {
    return this.#s[S_RPM] as number;
  }

  /** The most urgent tone the warning bits ask for. */
  static toneFor(bits: number): number {
    if ((bits & WARNING.PULLUP) !== 0) return TONE.PULL_UP;
    if ((bits & (WARNING.STALL | WARNING.DEPARTED)) !== 0) return TONE.STALL;
    if ((bits & OVER_G_BITS) !== 0) return TONE.OVER_G;
    if ((bits & WARNING.GEAR) !== 0) return TONE.GEAR;
    if ((bits & CAUTION_BITS) !== 0) return TONE.CAUTION;
    return TONE.NONE;
  }

  /**
   * Renders `frames` samples (at most `MAX_BLOCK`) into `left` and `right`, from the current
   * `params`. Makes no garbage.
   */
  process(left: Float32Array, right: Float32Array, frames: number): void {
    const n = Math.min(frames, MAX_BLOCK);
    const fs = this.#fs;
    const prof = this.#profile;
    const e = prof.engine;
    const a = prof.aero;
    const c = prof.cockpit;
    const w = prof.warnings;
    const p = this.params;
    const s = this.#s;

    // ---- block-rate decisions -------------------------------------------------------------
    const bits = p[P.ENGINE_BITS] as number;
    const running = (bits & 1) !== 0 || (bits & 8) !== 0;
    const ab = Math.min(Math.max(p[P.AB_LEVEL] as number, 0), 1);
    const coreTarget = running ? Math.min(Math.max(p[P.CORE] as number, 0), 1) : 0;
    // The spool follows the thrust with the profile's time constant.
    const follow = blockCoefficient(e.followS, fs, n);
    s[S_CORE] = (s[S_CORE] as number) + (coreTarget - (s[S_CORE] as number)) * follow;
    // One lag, not two: the shaft speed is the curve of the already-lagged thrust.
    s[S_RPM] = rpmFor(this.#rpmCurve, s[S_CORE]);
    const rpm = s[S_RPM];
    const fBlade = (rpm / 60) * e.bladeCount;
    const fShaft = rpm / 60;
    const core = Math.max(s[S_CORE], 1e-3);

    // Engine gains: a stopped engine only windmills; the afterburner raises the whole engine.
    const engineGainTarget = (running ? 1 : 0.15) * dbToLin(e.burnerRoarDb * ab);
    const engineGain0 = s[S_ENGINE_GAIN] as number;
    const engineGain1 =
      engineGain0 + (engineGainTarget - engineGain0) * blockCoefficient(0.15, fs, n);
    s[S_ENGINE_GAIN] = engineGain1;
    // ALGO: the whine is fan-tip noise, which climbs steeply with the tip speed: about the fifth power
    // of rpm in amplitude. The profile's blade and shaft levels are those at military rpm, so idle is
    // 11 dB below them and a windmilling engine is nearly silent.
    const whine = Math.min(rpm / e.militaryRpm, 1.25) ** 5;
    const bladeAmp = dbToLin(e.bladeLevelDb) * whine;
    const shaftAmp = dbToLin(e.shaftLevelDb) * whine;
    const hissAmp = dbToLin(e.hissLevelDb) * (0.3 + 0.7 * core);
    // The jet roar: 80 log10 of the exhaust speed, which goes as the square root of the thrust.
    const roarAmp = dbToLin(e.jetLevelDb + 40 * Math.log10(core)) * 1.6;
    const thumpAmp = dbToLin(e.lightoffThumpDb);
    const abLit = (bits & 2) !== 0 ? 1 : 0;
    if (abLit === 1 && (s[S_PREV_AB] as number) === 0) s[S_THUMP_ENV] = 1;
    s[S_PREV_AB] = abLit;
    const thumpDecay = Math.exp(-1 / (0.25 * fs));
    const thumpStep = (TWO_PI * e.lightoffThumpHz) / fs;

    // Wind, tyres, buffet.
    const qbar = Math.max(p[P.QBAR] as number, 1);
    const windDb =
      a.windRefLevelDb +
      a.windSlopeDb * Math.log10(qbar / a.windRefQbarPa) +
      a.airbrakeDb * (p[P.AIRBRAKE] as number);
    const windTarget = dbToLin(windDb) * 4;
    const onGround = (p[P.ON_GROUND] as number) > 0.5;
    const speed = Math.max(p[P.GROUND_SPEED] as number, 0);
    const tyreTarget =
      onGround && speed > 0.5 ? dbToLin(a.tyreRefLevelDb) * (speed / a.tyreRefSpeedMps) * 6 : 0;
    const margin = (a.buffetMarginDeg * Math.PI) / 180;
    const depth = Math.min(
      Math.max(
        ((p[P.ALPHA] as number) - ((p[P.ALPHA_WARN] as number) - margin)) / Math.max(margin, 1e-6),
        0,
      ),
      1,
    );
    const buffetTarget = dbToLin(a.buffetLevelDb) * depth * 4;
    const windG0 = s[S_WIND_G] as number;
    const tyreG0 = s[S_TYRE_G] as number;
    const buffetG0 = s[S_BUFFET_G] as number;
    const slow = blockCoefficient(0.12, fs, n);
    const windG1 = windG0 + (windTarget - windG0) * slow;
    const tyreG1 = tyreG0 + (tyreTarget - tyreG0) * slow;
    const buffetG1 = buffetG0 + (buffetTarget - buffetG0) * slow;
    s[S_WIND_G] = windG1;
    s[S_TYRE_G] = tyreG1;
    s[S_BUFFET_G] = buffetG1;

    // The tone, and the ducking it causes.
    const warningBits = p[P.WARNINGS] as number;
    const tone = FlightDsp.toneFor(warningBits);
    if (tone !== TONE.NONE && (s[S_LAST_TONE] as number) !== tone) {
      // A new tone starts at once: the pulse and chime start from their loud part.
      s[S_PULSE_PH] = 0;
      s[S_CHIME_T] = 0;
      s[S_TONE_PH] = 0;
      this.onsetFrame = 0;
    }
    s[S_LAST_TONE] = tone;
    this.tone = tone;
    const ducking = (warningBits & DUCKING_BITS) !== 0;
    const duckTarget = ducking ? dbToLin(prof.mix.duckDb) : 1;
    const duckNow = (s[S_DUCK] as number) || 1;
    // Fast to duck (10 ms), slow to come back (250 ms).
    const duck1 = duckNow + (duckTarget - duckNow) * blockCoefficient(ducking ? 0.01 : 0.25, fs, n);
    s[S_DUCK] = duck1;
    this.duckDb = 20 * Math.log10(duck1);
    const alphaOver = Math.min(
      Math.max(((p[P.ALPHA] as number) - (p[P.ALPHA_WARN] as number)) / 0.05, 0),
      1,
    );
    const pulseHz =
      w.stallHorn.pulseMinHz + (w.stallHorn.pulseMaxHz - w.stallHorn.pulseMinHz) * alphaOver;
    const stallAmp = dbToLin(w.stallHorn.levelDb);
    const overGAmp = dbToLin(w.overG.levelDb);
    const pullAmp = dbToLin(w.pullUp.levelDb);
    const gearAmp = dbToLin(w.gearChime.levelDb);
    const cautionAmp = dbToLin(w.cautionChime.levelDb);

    // The mix: the cockpit hears the outside through the canopy.
    const cockpitTarget = (p[P.COCKPIT] as number) > 0.5 ? 1 : 0;
    const cockpit0 = s[S_COCKPIT] as number;
    const cockpit1 = cockpit0 + (cockpitTarget - cockpit0) * blockCoefficient(0.05, fs, n);
    s[S_COCKPIT] = cockpit1;
    const canopyAtten = dbToLin(c.canopyAttenuationDb);
    const chaseGain = dbToLin(prof.mix.chaseOffsetDb);
    const humAmp = dbToLin(c.interiorHumDb);
    const humStep = (TWO_PI * c.interiorHumHz) / fs;
    // The profile's offset calibrates the aircraft's voices to the loudness target (GDD 56.6); the
    // warning tones keep their own levels, which are relative to the master.
    const offset = dbToLin(prof.mix.levelOffsetDb);
    const aircraftBus = (p[P.AIRCRAFT_BUS] as number) * (p[P.MASTER] as number) * offset;
    const warningsBus = (p[P.WARNINGS_BUS] as number) * (p[P.MASTER] as number);

    // ---- per-sample synthesis ------------------------------------------------------------
    let phShaft = s[S_PH_SHAFT] as number;
    let phB1 = s[S_BLADE_PH1] as number;
    let phB2 = s[S_BLADE_PH2] as number;
    let phB3 = s[S_BLADE_PH3] as number;
    let phB4 = s[S_BLADE_PH4] as number;
    const stepShaft = (TWO_PI * fShaft) / fs;
    const stepB = (TWO_PI * fBlade) / fs;
    const h = this.#harmonics;
    const h1 = h[0] as number;
    const h2 = h[1] as number;
    const h3 = h[2] as number;
    const h4 = h[3] as number;
    let thumpEnv = s[S_THUMP_ENV] as number;
    let thumpPh = s[S_THUMP_PH] as number;
    let buffetPh = s[S_BUFFET_PH] as number;
    const buffetStep = (TWO_PI * a.buffetHz) / fs;
    let tonePh = s[S_TONE_PH] as number;
    let pulsePh = s[S_PULSE_PH] as number;
    let chimeT = s[S_CHIME_T] as number;
    let humPh = s[S_HUM_PH] as number;
    let eg = engineGain0;
    const egStep = (engineGain1 - engineGain0) / n;
    let wg = windG0;
    const wgStep = (windG1 - windG0) / n;
    let tg = tyreG0;
    const tgStep = (tyreG1 - tyreG0) / n;
    let bg = buffetG0;
    const bgStep = (buffetG1 - buffetG0) / n;
    let cp = cockpit0;
    const cpStep = (cockpit1 - cockpit0) / n;
    let dk = duckNow;
    const dkStep = (duck1 - duckNow) / n;
    const aircraft = this.#aircraft;
    const white = this.#white;
    const hissN = this.#hiss;
    const windN = this.#windN;
    const tyreN = this.#tyreN;
    const buffetN = this.#buffetN;
    const outside = this.#outside;
    const canopied = this.#canopied;
    const formants = this.#formants;
    const nf = formants.length;

    // The block is made in passes over its samples, each voice's filters in one call a block rather
    // than one a sample: V8 does not inline everything into a function this long, and a number
    // returned from a call it does not inline is boxed on the heap (A7). The arithmetic, and so the
    // sound, is the same sample for sample.
    // Pass 1: the noise, five draws a sample in the order engine, hiss, wind, tyres, buffet, then
    // each voice's filters.
    fillNoise5(this.#noise, this.#planes, MAX_BLOCK, n);
    this.#roarLow.processBlock(white, white, n);
    for (let f = 0; f < nf; f++) (formants[f] as Biquad).processBlock(white, white, n);
    this.#hissHigh.processBlock(hissN, hissN, n);
    this.#windBand.processBlock(windN, windN, n);
    this.#tyreLow.processBlock(tyreN, tyreN, n);
    this.#buffetLow.processBlock(buffetN, buffetN, n);

    // Pass 2: the engine, wind, tyres and buffet, ducked, into the outside mix.
    for (let i = 0; i < n; i++) {
      // Engine.
      phShaft += stepShaft;
      phB1 += stepB;
      phB2 += stepB * 2;
      phB3 += stepB * 3;
      phB4 += stepB * 4;
      let tonal =
        bladeAmp *
          (h1 * Math.sin(phB1) + h2 * Math.sin(phB2) + h3 * Math.sin(phB3) + h4 * Math.sin(phB4)) +
        shaftAmp * Math.sin(phShaft);
      tonal += roarAmp * (white[i] as number) + hissAmp * (hissN[i] as number);
      thumpPh += thumpStep;
      thumpEnv *= thumpDecay;
      tonal += thumpAmp * thumpEnv * Math.sin(thumpPh);
      const engine = tonal * eg;
      eg += egStep;

      // Wind, tyres and buffet.
      const wind = wg * (windN[i] as number);
      wg += wgStep;
      const tyre = tg * (tyreN[i] as number);
      tg += tgStep;
      buffetPh += buffetStep;
      const buffet = bg * (buffetN[i] as number) * (0.6 + 0.4 * Math.sin(buffetPh));
      bg += bgStep;

      // The outside, ducked.
      outside[i] = (engine + wind + tyre + buffet) * dk;
      dk += dkStep;
    }

    // Pass 3: the canopy, and the duel's voices.
    this.#canopy.processBlock(outside, canopied, n);
    this.#combat(n);
    const combatOn = this.#combatOn;
    const combatAir = this.#combatAir;
    const combatWarn = this.#combatWarn;
    // The duel's aircraft-bus voices are near and loud: no canopy and no calibration offset (their
    // levels are the master's, as the warning tones' are), but the bus volume and the duck.
    const combatBus = (p[P.AIRCRAFT_BUS] as number) * (p[P.MASTER] as number);
    let dc = duckNow;

    // Pass 4: the cockpit's hum and the cabin mix, the warning tone, the master and the limiter.
    for (let i = 0; i < n; i++) {
      const filtered = (canopied[i] as number) * canopyAtten;
      humPh += humStep;
      const cabin = filtered + humAmp * (Math.sin(humPh) + 0.3 * Math.sin(humPh * 2.01));
      const bare = (outside[i] as number) * chaseGain;
      const mixed = bare + (cabin - bare) * cp;
      cp += cpStep;

      // The warning tone.
      let warn = 0;
      switch (tone) {
        case TONE.PULL_UP: {
          // A warble between two pitches, `rate_hz` times a second.
          pulsePh += (TWO_PI * w.pullUp.rateHz) / fs;
          const f = Math.sin(pulsePh) >= 0 ? w.pullUp.highHz : w.pullUp.lowHz;
          tonePh += (TWO_PI * f) / fs;
          warn = pullAmp * Math.sin(tonePh);
          break;
        }
        case TONE.STALL: {
          // The horn: a tone gated by a square pulse that speeds up past the warning angle.
          pulsePh += (TWO_PI * pulseHz) / fs;
          const gate = Math.sin(pulsePh) >= 0 ? 1 : 0;
          tonePh += (TWO_PI * w.stallHorn.freqHz) / fs;
          warn = stallAmp * gate * Math.sin(tonePh);
          break;
        }
        case TONE.OVER_G: {
          pulsePh += (TWO_PI * 5) / fs;
          const gate = Math.sin(pulsePh) >= 0 ? 1 : 0;
          tonePh += (TWO_PI * w.overG.freqHz) / fs;
          warn = overGAmp * gate * Math.sin(tonePh);
          break;
        }
        case TONE.GEAR:
        case TONE.CAUTION: {
          // A chime: a short ding that repeats; its first ding is at the start.
          const gear = tone === TONE.GEAR;
          const period = gear ? w.gearChime.repeatS : w.cautionChime.repeatS;
          const freq = gear ? w.gearChime.freqHz : w.cautionChime.freqHz;
          chimeT += 1 / fs;
          if (chimeT >= period) chimeT -= period;
          tonePh += (TWO_PI * freq) / fs;
          const env = Math.exp(-chimeT * 14) * (chimeT < 0.001 ? chimeT * 1000 : 1);
          warn = (gear ? gearAmp : cautionAmp) * env * Math.sin(tonePh);
          break;
        }
        default:
          break;
      }
      aircraft[i] = mixed * aircraftBus;
      if (combatOn) {
        aircraft[i] = (aircraft[i] as number) + (combatAir[i] as number) * combatBus * dc;
        warn += combatWarn[i] as number;
      }
      dc += dkStep;

      // Master, limiter (`softLimit`, written out: it is called a sample at a time).
      const x = (aircraft[i] as number) + warn * warningsBus;
      const ax = Math.abs(x);
      let y = x;
      if (ax > LIMITER_KNEE) {
        const room = CEILING - LIMITER_KNEE;
        const limited = LIMITER_KNEE + room * Math.tanh((ax - LIMITER_KNEE) / room);
        y = x < 0 ? -limited : limited;
      }
      left[i] = y;
      right[i] = y;
    }

    s[S_PH_SHAFT] = phShaft % TWO_PI;
    s[S_BLADE_PH1] = phB1 % TWO_PI;
    s[S_BLADE_PH2] = phB2 % TWO_PI;
    s[S_BLADE_PH3] = phB3 % TWO_PI;
    s[S_BLADE_PH4] = phB4 % TWO_PI;
    s[S_THUMP_ENV] = thumpEnv;
    s[S_THUMP_PH] = thumpPh % TWO_PI;
    s[S_BUFFET_PH] = buffetPh % TWO_PI;
    s[S_TONE_PH] = tonePh % TWO_PI;
    s[S_PULSE_PH] = pulsePh % TWO_PI;
    s[S_CHIME_T] = chimeT;
    s[S_HUM_PH] = humPh % TWO_PI;
  }

  /**
   * The duel's voices for `n` samples, into `#combatAir` (before the aircraft bus) and `#combatWarn`
   * (before the warnings bus), from the current `params`. Sets `#combatOn` to whether any of them is
   * sounding; when none is, it does no per-sample work.
   */
  #combat(n: number): void {
    const wp = this.#weapons;
    if (wp === undefined) return;
    const fs = this.#fs;
    const p = this.params;
    const s = this.#s;
    const amp = this.#amp;
    const dt = 1 / fs;

    // ---- block-rate decisions -------------------------------------------------------------
    // The counters: a rise starts a one-shot voice; the first values after a reset are adopted.
    const launches = p[P.LAUNCHES] as number;
    const hits = p[P.HITS_TAKEN] as number;
    const booms = p[P.EXPLOSIONS] as number;
    if ((s[S_SEEN] as number) !== 0) {
      if (launches > (s[S_LAUNCHES] as number)) s[S_LAUNCH_ENV] = 1;
      if (hits > (s[S_HITS] as number)) s[S_HIT_SINCE] = 0;
      if (booms > (s[S_BOOMS] as number)) {
        s[S_BOOM_ENV] = 1;
        s[S_BOOM_PH] = 0;
        // Its level by its distance, fixed when it starts.
        const d = p[P.EXPLOSION_DIST_M] as number;
        const ref = wp.explosion.refDistanceM;
        s[S_BOOM_AMP] = ((amp[A_BOOM] as number) * ref) / Math.max(d > 0 ? d : ref, ref);
      }
    }
    s[S_SEEN] = 1;
    s[S_LAUNCHES] = launches;
    s[S_HITS] = hits;
    s[S_BOOMS] = booms;

    // The guns: a rising edge fires the first pulse at once.
    const firing = (p[P.GUN_FIRING] as number) > 0.5;
    if (firing && (s[S_GUN_PREV] as number) === 0) s[S_GUN_ACC] = 1;
    s[S_GUN_PREV] = firing ? 1 : 0;
    const enemyDist = p[P.ENEMY_GUN_DIST_M] as number;
    const enemyFiring = enemyDist > 0;
    if (enemyFiring && (s[S_EN_PREV] as number) === 0) s[S_EN_ACC] = 1;
    s[S_EN_PREV] = enemyFiring ? 1 : 0;
    const enRef = wp.enemyGun.refDistanceM;
    const enemyAmp = enemyFiring
      ? ((amp[A_ENEMY] as number) * enRef) / Math.max(enemyDist, enRef)
      : 0;

    // The seeker: the growl in SEARCH with some signal and in TONE, the lock tone when LOCKED.
    const seeker = p[P.SEEKER] as number;
    const signal = p[P.SIGNAL] as number;
    const growlOn =
      seeker === SEEKER_MODE.TONE || (seeker === SEEKER_MODE.SEARCH && signal > 0.3) ? 1 : 0;
    const lockOn = seeker === SEEKER_MODE.LOCKED ? 1 : 0;
    const growlHz =
      wp.growl.baseHz + wp.growl.spanHz * Math.min(Math.max((signal - 0.3) / 3.7, 0), 1);
    const growlG0 = s[S_GROWL_G] as number;
    const lockG0 = s[S_LOCK_G] as number;
    s[S_GROWL_G] = growlOn;
    s[S_LOCK_G] = lockOn;

    const pulseS = wp.gun.pulseMs / 1000;
    const hitS = wp.hit.lengthMs / 1000;
    const quiet =
      !firing &&
      !enemyFiring &&
      (s[S_GUN_SINCE] as number) > pulseS + 10 * GUN_BODY_S &&
      (s[S_EN_SINCE] as number) > pulseS + 10 * GUN_BODY_S &&
      (s[S_LAUNCH_ENV] as number) < 1e-5 &&
      (s[S_HIT_SINCE] as number) > hitS &&
      (s[S_BOOM_ENV] as number) < 1e-5 &&
      growlOn === 0 &&
      growlG0 === 0 &&
      lockOn === 0 &&
      lockG0 === 0;
    this.#combatOn = !quiet;
    if (quiet) return;

    // ---- per-sample synthesis ------------------------------------------------------------
    fillNoise5(this.#combatNoise, this.#combatPlanes, MAX_BLOCK, n);
    const crackN = this.#crackN;
    const enemyN = this.#enemyN;
    const launchN = this.#launchN;
    const hitN = this.#hitN;
    const boomN = this.#boomN;
    this.#crackBand.processBlock(crackN, crackN, n);
    this.#enemyBand.processBlock(enemyN, enemyN, n);
    this.#launchBand.processBlock(launchN, launchN, n);
    this.#hitBand.processBlock(hitN, hitN, n);
    this.#boomLow.processBlock(boomN, boomN, n);

    const gunAmp = amp[A_GUN] as number;
    const gunInc = wp.gun.rateHz / fs;
    const bodyStep = (TWO_PI * wp.gun.bodyHz) / fs;
    const bodyDecay = Math.exp(-1 / (GUN_BODY_S * fs));
    const launchAmp = amp[A_LAUNCH] as number;
    const launchDecay = Math.exp(-1 / (wp.launch.decayS * fs));
    const hitAmp = amp[A_HIT] as number;
    const boomAmp = s[S_BOOM_AMP] as number;
    const boomDecay = Math.exp(-1 / (wp.explosion.decayS * fs));
    const thumpStep = (TWO_PI * wp.explosion.thumpHz) / fs;
    const growlAmp = amp[A_GROWL] as number;
    const growlStep = (TWO_PI * growlHz) / fs;
    const amStep = (TWO_PI * wp.growl.amHz) / fs;
    const lockAmp = amp[A_LOCK] as number;
    const lockStep = (TWO_PI * wp.lockTone.freqHz) / fs;

    let gunAcc = s[S_GUN_ACC] as number;
    let gunSince = s[S_GUN_SINCE] as number;
    let gunBody = s[S_GUN_BODY] as number;
    let gunBodyPh = s[S_GUN_BODY_PH] as number;
    let enAcc = s[S_EN_ACC] as number;
    let enSince = s[S_EN_SINCE] as number;
    let enBody = s[S_EN_BODY] as number;
    let enBodyPh = s[S_EN_BODY_PH] as number;
    let launchEnv = s[S_LAUNCH_ENV] as number;
    let hitSince = s[S_HIT_SINCE] as number;
    let boomEnv = s[S_BOOM_ENV] as number;
    let boomPh = s[S_BOOM_PH] as number;
    let growlPh = s[S_GROWL_PH] as number;
    let amPh = s[S_GROWL_AM_PH] as number;
    let lockPh = s[S_LOCK_PH] as number;
    let gg = growlG0;
    const ggStep = (growlOn - growlG0) / n;
    let lg = lockG0;
    const lgStep = (lockOn - lockG0) / n;
    const air = this.#combatAir;
    const warn = this.#combatWarn;
    const enemy = this.#enemy;

    for (let i = 0; i < n; i++) {
      // The gun: a pulse each round, `pulse_ms` of crack and a low sine dying away.
      if (firing) {
        gunAcc += gunInc;
        if (gunAcc >= 1) {
          gunAcc -= 1;
          gunSince = 0;
          gunBody = 1;
          gunBodyPh = 0;
        }
      }
      const crack = gunSince < pulseS ? 1 - gunSince / pulseS : 0;
      gunBodyPh += bodyStep;
      let a = gunAmp * (CRACK_GAIN * crack * (crackN[i] as number) + gunBody * Math.sin(gunBodyPh));
      gunBody *= bodyDecay;
      gunSince += dt;

      // The bandit's gun: the same pulses, low-passed below.
      if (enemyFiring) {
        enAcc += gunInc;
        if (enAcc >= 1) {
          enAcc -= 1;
          enSince = 0;
          enBody = 1;
          enBodyPh = 0;
        }
      }
      const enCrack = enSince < pulseS ? 1 - enSince / pulseS : 0;
      enBodyPh += bodyStep;
      enemy[i] =
        enemyAmp * (CRACK_GAIN * enCrack * (enemyN[i] as number) + enBody * Math.sin(enBodyPh));
      enBody *= bodyDecay;
      enSince += dt;

      // The launch, the hit and the explosion.
      a += launchAmp * launchEnv * (launchN[i] as number);
      launchEnv *= launchDecay;
      if (hitSince < hitS) {
        const k = 1 - hitSince / hitS;
        a += hitAmp * k * k * (hitN[i] as number);
      }
      hitSince += dt;
      boomPh += thumpStep;
      a += boomAmp * boomEnv * (Math.sin(boomPh) + BOOM_NOISE * (boomN[i] as number));
      boomEnv *= boomDecay;
      air[i] = a;

      // The growl: a square-ish tone (its first three odd harmonics), amplitude-modulated.
      growlPh += growlStep;
      amPh += amStep;
      const sq = Math.sin(growlPh) + Math.sin(3 * growlPh) / 3 + Math.sin(5 * growlPh) / 5;
      let w = growlAmp * gg * (1 - GROWL_AM + GROWL_AM * Math.sin(amPh)) * sq;
      gg += ggStep;
      lockPh += lockStep;
      w += lockAmp * lg * Math.sin(lockPh);
      lg += lgStep;
      warn[i] = w;
    }
    this.#enemyLow.processBlock(enemy, enemy, n);
    for (let i = 0; i < n; i++) air[i] = (air[i] as number) + (enemy[i] as number);

    s[S_GUN_ACC] = gunAcc;
    s[S_GUN_SINCE] = Math.min(gunSince, LONG_AGO_S);
    s[S_GUN_BODY] = gunBody;
    s[S_GUN_BODY_PH] = gunBodyPh % TWO_PI;
    s[S_EN_ACC] = enAcc;
    s[S_EN_SINCE] = Math.min(enSince, LONG_AGO_S);
    s[S_EN_BODY] = enBody;
    s[S_EN_BODY_PH] = enBodyPh % TWO_PI;
    s[S_LAUNCH_ENV] = launchEnv;
    s[S_HIT_SINCE] = Math.min(hitSince, LONG_AGO_S);
    s[S_BOOM_ENV] = boomEnv;
    s[S_BOOM_PH] = boomPh % TWO_PI;
    s[S_GROWL_PH] = growlPh % TWO_PI;
    s[S_GROWL_AM_PH] = amPh % TWO_PI;
    s[S_LOCK_PH] = lockPh % TWO_PI;
  }
}

/** The output ceiling: 0.85 is -1.4 dBFS, so the true peak stays under -1 dBTP. */
export const CEILING = 0.85;

/** Where the soft limiter starts to bend. */
const LIMITER_KNEE = 0.5;

/** A soft limiter: linear below the knee, saturating smoothly to the ceiling above it. */
export function softLimit(x: number): number {
  const ax = Math.abs(x);
  if (ax <= LIMITER_KNEE) return x;
  const over = ax - LIMITER_KNEE;
  const room = CEILING - LIMITER_KNEE;
  const y = LIMITER_KNEE + room * Math.tanh(over / room);
  return x < 0 ? -y : y;
}
