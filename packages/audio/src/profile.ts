/**
 * The audio profile of an aircraft (`audio_profile/1`, GDD 56.3, A.12), parsed from the
 * presentation bundle (ADR-43). Everything the engine, wind, buffet, tyre and warning voices need
 * is a number here; nothing is a sample. Levels are dB relative to the master bus.
 *
 * The baker has validated all of it; this parser checks the shape again, so a bundle from a
 * mismatched build is an error naming the value (`/engine/idle_rpm`).
 */
import {
  type AircraftPresentation,
  type PresentBundle,
  PresentError,
  type Slice,
} from '@flying/sim-bridge/present';

export interface Formant {
  readonly fHz: number;
  readonly q: number;
  readonly gainDb: number;
}

export interface EngineProfile {
  /** Compressor blades; the blade-pass tone is `rpm / 60 * bladeCount`. */
  readonly bladeCount: number;
  readonly idleRpm: number;
  readonly militaryRpm: number;
  /** `[core thrust fraction, rpm]`, rising, from 0 to 1. */
  readonly rpmByCore: readonly (readonly [number, number])[];
  /** dB below the fundamental, fundamental first. */
  readonly harmonicLevelsDb: readonly number[];
  readonly bladeLevelDb: number;
  readonly shaftLevelDb: number;
  readonly hissLevelDb: number;
  readonly jetLevelDb: number;
  readonly burnerRoarDb: number;
  readonly lightoffThumpDb: number;
  readonly lightoffThumpHz: number;
  readonly formants: readonly Formant[];
  readonly followS: number;
}

export interface AeroProfile {
  readonly windRefQbarPa: number;
  readonly windRefLevelDb: number;
  /** Rise in level for ten times the dynamic pressure. */
  readonly windSlopeDb: number;
  readonly windLowHz: number;
  readonly windHighHz: number;
  readonly airbrakeDb: number;
  readonly buffetMarginDeg: number;
  readonly buffetLevelDb: number;
  readonly buffetHz: number;
  readonly tyreRefSpeedMps: number;
  readonly tyreRefLevelDb: number;
}

export interface CockpitProfile {
  readonly canopyLowpassHz: number;
  readonly canopyAttenuationDb: number;
  readonly interiorHumDb: number;
  readonly interiorHumHz: number;
}

export interface Chime {
  readonly freqHz: number;
  readonly repeatS: number;
  readonly levelDb: number;
}

export interface WarningProfile {
  readonly stallHorn: {
    readonly freqHz: number;
    readonly pulseMinHz: number;
    readonly pulseMaxHz: number;
    readonly levelDb: number;
  };
  readonly overG: { readonly freqHz: number; readonly levelDb: number };
  readonly pullUp: {
    readonly lowHz: number;
    readonly highHz: number;
    readonly rateHz: number;
    readonly levelDb: number;
  };
  readonly gearChime: Chime;
  readonly cautionChime: Chime;
}

export interface MixProfile {
  /** The aircraft bus is lowered by this much (a negative number) while a warning sounds. */
  readonly duckDb: number;
  readonly levelOffsetDb: number;
  /** The aircraft's voices in the chase view, relative to the cockpit view, dB. */
  readonly chaseOffsetDb: number;
}

/**
 * The duel's sounds (ADR-42 amendment 1). Their levels are relative to the master, like the warning
 * tones': the gun, enemy gun, launch, hit and explosion go on the aircraft bus, the seeker's growl
 * and lock tone on the warnings bus.
 */
export interface WeaponsProfile {
  /** The own gun: a pulse per round, `rateHz` a second (the baker checks it against the gun's rpm). */
  readonly gun: {
    readonly rateHz: number;
    readonly levelDb: number;
    /** The decaying sine of each pulse. */
    readonly bodyHz: number;
    /** The band-passed noise of each pulse, and how long it lasts. */
    readonly crackHz: number;
    readonly pulseMs: number;
  };
  /** The bandit's gun: the same pulses, low-passed, falling 6 dB per doubling of distance. */
  readonly enemyGun: {
    readonly levelDb: number;
    readonly lowpassHz: number;
    readonly refDistanceM: number;
  };
  /** A missile leaving the rail: band-passed noise, decaying. */
  readonly launch: {
    readonly levelDb: number;
    readonly lowHz: number;
    readonly highHz: number;
    readonly decayS: number;
  };
  /** The seeker's growl: its pitch rises with the signal. */
  readonly growl: {
    readonly baseHz: number;
    readonly spanHz: number;
    readonly amHz: number;
    readonly levelDb: number;
  };
  readonly lockTone: { readonly freqHz: number; readonly levelDb: number };
  /** A hit taken: a short band-passed click. */
  readonly hit: { readonly levelDb: number; readonly bandHz: number; readonly lengthMs: number };
  /** A thump and noise, falling 6 dB per doubling of distance beyond the reference. */
  readonly explosion: {
    readonly levelDb: number;
    readonly thumpHz: number;
    readonly decayS: number;
    readonly refDistanceM: number;
  };
}

export interface AudioProfile {
  readonly id: string;
  readonly aircraft: string;
  readonly engine: EngineProfile;
  readonly aero: AeroProfile;
  readonly cockpit: CockpitProfile;
  readonly warnings: WarningProfile;
  readonly mix: MixProfile;
  /** The duel's sounds; without them the duel is silent apart from the aircraft. */
  readonly weapons?: WeaponsProfile;
}

const DB = [-120, 60] as const;
const HZ = [1, 24_000] as const;

function db(s: Slice, key: string): number {
  return s.n(key, DB[0], DB[1]);
}
function hz(s: Slice, key: string): number {
  return s.n(key, HZ[0], HZ[1]);
}

function chime(s: Slice): Chime {
  return { freqHz: hz(s, 'freq_hz'), repeatS: s.n('repeat_s', 0.01), levelDb: db(s, 'level_db') };
}

function parseWeapons(s: Slice): WeaponsProfile {
  const gun = s.at('gun');
  const enemy = s.at('enemy_gun');
  const launch = s.at('launch');
  const growl = s.at('growl');
  const lock = s.at('lock_tone');
  const hit = s.at('hit');
  const boom = s.at('explosion');
  return {
    gun: {
      rateHz: gun.n('rate_hz', 1, 200),
      levelDb: db(gun, 'level_db'),
      bodyHz: hz(gun, 'body_hz'),
      crackHz: hz(gun, 'crack_hz'),
      pulseMs: gun.n('pulse_ms', 1, 100),
    },
    enemyGun: {
      levelDb: db(enemy, 'level_db'),
      lowpassHz: hz(enemy, 'lowpass_hz'),
      refDistanceM: enemy.n('ref_distance_m', 1),
    },
    launch: {
      levelDb: db(launch, 'level_db'),
      lowHz: hz(launch, 'low_hz'),
      highHz: hz(launch, 'high_hz'),
      decayS: launch.n('decay_s', 0.01, 10),
    },
    growl: {
      baseHz: hz(growl, 'base_hz'),
      spanHz: growl.n('span_hz', 0, HZ[1]),
      amHz: growl.n('am_hz', 0, 200),
      levelDb: db(growl, 'level_db'),
    },
    lockTone: { freqHz: hz(lock, 'freq_hz'), levelDb: db(lock, 'level_db') },
    hit: {
      levelDb: db(hit, 'level_db'),
      bandHz: hz(hit, 'band_hz'),
      lengthMs: hit.n('length_ms', 1, 1000),
    },
    explosion: {
      levelDb: db(boom, 'level_db'),
      thumpHz: hz(boom, 'thump_hz'),
      decayS: boom.n('decay_s', 0.01, 10),
      refDistanceM: boom.n('ref_distance_m', 1),
    },
  };
}

/** Reads an `audio_profile/1` slice. */
export function parseAudioProfile(s: Slice): AudioProfile {
  const weapons = s.opt('weapons');
  const e = s.at('engine');
  const rpmByCore = e
    .at('rpm_by_core')
    .items()
    .map((p) => p.tuple(2) as unknown as readonly [number, number]);
  if (rpmByCore.length < 2) e.at('rpm_by_core').fail('expected at least two points');
  const a = s.at('aero');
  const c = s.at('cockpit');
  const w = s.at('warnings');
  const stall = w.at('stall_horn');
  const overG = w.at('over_g');
  const pull = w.at('pull_up');
  const mix = s.at('mix');
  return {
    id: s.s('id'),
    aircraft: s.s('aircraft'),
    engine: {
      bladeCount: e.at('blade_count').int(1, 200),
      idleRpm: e.n('idle_rpm', 1),
      militaryRpm: e.n('military_rpm', 1),
      rpmByCore,
      harmonicLevelsDb: e.at('harmonic_levels_db').numbers(),
      bladeLevelDb: db(e, 'blade_level_db'),
      shaftLevelDb: db(e, 'shaft_level_db'),
      hissLevelDb: db(e, 'hiss_level_db'),
      jetLevelDb: db(e, 'jet_level_db'),
      burnerRoarDb: e.n('burner_roar_db', 0, 24),
      lightoffThumpDb: db(e, 'lightoff_thump_db'),
      lightoffThumpHz: hz(e, 'lightoff_thump_hz'),
      formants: e
        .at('formants')
        .items()
        .map((f) => ({ fHz: hz(f, 'f_hz'), q: f.n('q', 0.01), gainDb: db(f, 'gain_db') })),
      followS: e.n('follow_s', 0),
    },
    aero: {
      windRefQbarPa: a.n('wind_ref_qbar_pa', 1),
      windRefLevelDb: db(a, 'wind_ref_level_db'),
      windSlopeDb: a.n('wind_slope_db', 0),
      windLowHz: hz(a, 'wind_low_hz'),
      windHighHz: hz(a, 'wind_high_hz'),
      airbrakeDb: a.n('airbrake_db', 0),
      buffetMarginDeg: a.n('buffet_margin_deg', 0),
      buffetLevelDb: db(a, 'buffet_level_db'),
      buffetHz: hz(a, 'buffet_hz'),
      tyreRefSpeedMps: a.n('tyre_ref_speed_mps', 0.1),
      tyreRefLevelDb: db(a, 'tyre_ref_level_db'),
    },
    cockpit: {
      canopyLowpassHz: hz(c, 'canopy_lowpass_hz'),
      canopyAttenuationDb: c.n('canopy_attenuation_db', DB[0], 0),
      interiorHumDb: db(c, 'interior_hum_db'),
      interiorHumHz: hz(c, 'interior_hum_hz'),
    },
    warnings: {
      stallHorn: {
        freqHz: hz(stall, 'freq_hz'),
        pulseMinHz: stall.n('pulse_min_hz', 0.01),
        pulseMaxHz: stall.n('pulse_max_hz', 0.01),
        levelDb: db(stall, 'level_db'),
      },
      overG: { freqHz: hz(overG, 'freq_hz'), levelDb: db(overG, 'level_db') },
      pullUp: {
        lowHz: hz(pull, 'low_hz'),
        highHz: hz(pull, 'high_hz'),
        rateHz: pull.n('rate_hz', 0.01),
        levelDb: db(pull, 'level_db'),
      },
      gearChime: chime(w.at('gear_chime')),
      cautionChime: chime(w.at('caution_chime')),
    },
    mix: {
      duckDb: mix.n('duck_db', DB[0], 0),
      levelOffsetDb: db(mix, 'level_offset_db'),
      chaseOffsetDb: db(mix, 'chase_offset_db'),
    },
    ...(weapons === undefined ? {} : { weapons: parseWeapons(weapons) }),
  };
}

/**
 * The audio profile for `aircraftId`, or undefined if the bundle has none (the game is then
 * silent apart from the null backend).
 *
 * @throws PresentError if the profile is malformed.
 */
export function audioProfileFor(
  bundle: PresentBundle,
  aircraftId: string,
): AudioProfile | undefined {
  const a: AircraftPresentation | undefined = bundle.forAircraft(aircraftId);
  if (a === undefined) return undefined;
  if (a.audio === undefined) {
    throw new PresentError(`/aircraft/${aircraftId}`, 'has no audio profile');
  }
  return parseAudioProfile(a.audio);
}
