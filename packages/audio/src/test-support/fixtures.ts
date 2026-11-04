/** The Kestrel's audio profile and the recorded circuit trace, for the audio tests. Not production code. */
import { readFileSync } from 'node:fs';
import { parsePresentBundle } from '@flying/sim-bridge/present';
import { newAircraftStatus, warningBits } from '@flying/sim-bridge';
import type { FlightDsp } from '../dsp/flight-dsp.ts';
import { P } from '../dsp/flight-dsp.ts';
import { audioProfileFor, type AudioProfile } from '../profile.ts';

const text = (rel: string): string => readFileSync(new URL(rel, import.meta.url), 'utf8');

/** The Kestrel's `audio_profile/1`, as baked into the presentation bundle vector. */
export function kestrelProfile(): AudioProfile {
  const bundle = parsePresentBundle(
    JSON.parse(text('../../../../tests/vectors/present_bundle.json')),
  );
  const profile = audioProfileFor(bundle, 'core:ac/kestrel');
  if (profile === undefined) throw new Error('the bundle has no Kestrel audio profile');
  return profile;
}

/** One 10 Hz row of `fg circuit --traces`. */
export type TraceRow = Record<string, number>;

/** The 60 s circuit trace (`test/fixtures/circuit-60s.csv`): numbers by column name. */
export function circuitTrace(): TraceRow[] {
  const [head, ...lines] = text('../../test/fixtures/circuit-60s.csv').trim().split('\n');
  const names = (head ?? '').split(',');
  return lines.map((line) => {
    const row: TraceRow = {};
    line.split(',').forEach((cell, i) => {
      row[names[i] ?? ''] = cell === '' ? 0 : Number(cell);
    });
    return row;
  });
}

/** Bit 6 of the gear bits: the lever is down. */
const GEAR_LEVER_DOWN = 1 << 6;

/** The warning bits the trace row would raise at Standard assist, by the same rule the game uses. */
export function traceWarnings(row: TraceRow): number {
  const status = {
    ...newAircraftStatus(),
    assist: 2,
    flags: row['flags'] ?? 0,
    flags2: row['flags2'] ?? 0,
    cas: row['cas_mps'] ?? 0,
    agl: row['agl_m'] ?? 0,
    vs: row['vs_mps'] ?? 0,
    gearBits: (row['gear_pos'] ?? 0) > 0.5 ? GEAR_LEVER_DOWN : 0,
  };
  return warningBits(status);
}

/** Feeds a trace row to the synthesis: the same mapping the game's feed makes from the status. */
export function feedRow(dsp: FlightDsp, row: TraceRow, cockpit = true): void {
  const p = dsp.params;
  p[P.CORE] = row['engine_core'] ?? 0;
  p[P.AB_LEVEL] = row['ab_level'] ?? 0;
  p[P.ENGINE_BITS] = row['engine_bits'] ?? 0;
  p[P.QBAR] = row['qbar_pa'] ?? 0;
  p[P.ALPHA] = ((row['alpha_deg'] ?? 0) * Math.PI) / 180;
  p[P.ALPHA_WARN] = ((row['alpha_warn_deg'] ?? 0) * Math.PI) / 180;
  p[P.AIRBRAKE] = row['airbrake'] ?? 0;
  p[P.GROUND_SPEED] = row['gs_mps'] ?? 0;
  p[P.ON_GROUND] = row['wow'] ?? 0;
  p[P.WARNINGS] = traceWarnings(row);
  p[P.COCKPIT] = cockpit ? 1 : 0;
}
