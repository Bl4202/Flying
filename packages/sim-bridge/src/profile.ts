/**
 * The profile block the core writes while profiling is on: tick and per-stage times in
 * milliseconds (last and smoothed) and a few counters. Observations about the machine, never
 * state. Layout in `layout.gen.ts`.
 */
import { PROFILE } from './layout.gen.ts';

/** Stage names in execution order (GDD 61.4.3), indexed by `STAGE`. */
export const STAGE_NAMES = [
  'ingest',
  'environment',
  'relevance',
  'ai',
  'dynamics',
  'weapons',
  'sensing',
  'collision',
  'systems',
  'director',
  'publish',
] as const;

/** Counter names, indexed by `COUNTER`. */
export const COUNTER_NAMES = [
  'input_applied',
  'input_nonfinite',
  'input_stale',
  'input_overflow',
  'fdm_faults',
  'crashes',
  'respawns',
  'snapshots',
  'trim_failures',
  'touchdowns',
  'hard_landings',
  'gloc_events',
  'assist_changes',
] as const;

export interface Profile {
  ticks: number;
  tickMsLast: number;
  tickMsEwma: number;
  stageMsLast: Float32Array;
  stageMsEwma: Float32Array;
  counters: Uint32Array;
}

/** Copies a profile block into a fresh object. */
export function decodeProfile(bytes: Uint8Array): Profile {
  if (bytes.byteLength < PROFILE.SIZE) {
    throw new RangeError(
      `a profile block is ${String(PROFILE.SIZE)} bytes, got ${String(bytes.byteLength)}`,
    );
  }
  const dv = new DataView(bytes.buffer, bytes.byteOffset, PROFILE.SIZE);
  const stageMsLast = new Float32Array(PROFILE.STAGE_COUNT);
  const stageMsEwma = new Float32Array(PROFILE.STAGE_COUNT);
  for (let i = 0; i < PROFILE.STAGE_COUNT; i++) {
    stageMsLast[i] = dv.getFloat32(PROFILE.STAGE_MS_LAST + 4 * i, true);
    stageMsEwma[i] = dv.getFloat32(PROFILE.STAGE_MS_EWMA + 4 * i, true);
  }
  const counters = new Uint32Array(PROFILE.COUNTER_COUNT);
  for (let i = 0; i < PROFILE.COUNTER_COUNT; i++) {
    counters[i] = dv.getUint32(PROFILE.COUNTERS + 4 * i, true);
  }
  return {
    ticks: Number(dv.getBigUint64(PROFILE.TICKS, true)),
    tickMsLast: dv.getFloat32(PROFILE.TICK_MS_LAST, true),
    tickMsEwma: dv.getFloat32(PROFILE.TICK_MS_EWMA, true),
    stageMsLast,
    stageMsEwma,
    counters,
  };
}
