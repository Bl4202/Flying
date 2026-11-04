import { describe, expect, it } from 'vitest';
import { integratedLoudness, render, truePeakDb } from '../test-support/analysis.ts';
import { circuitTrace, feedRow, kestrelProfile } from '../test-support/fixtures.ts';
import { FlightDsp } from './flight-dsp.ts';

const profile = kestrelProfile();
const trace = circuitTrace();

/** 60 s of the recorded circuit through the synthesis, the state changing at the trace's 10 Hz. */
function mixOfCircuit(cockpit: boolean): { left: Float32Array; right: Float32Array } {
  const dsp = new FlightDsp(profile);
  const seconds = trace.length / 10;
  return render(dsp, seconds, (_block, t) => {
    const row = trace[Math.min(Math.floor(t * 10), trace.length - 1)];
    if (row !== undefined) feedRow(dsp, row, cockpit);
  });
}

describe('A5: the 60 s circuit mix (GDD 56.6)', () => {
  const trace60 = trace.length;
  it('is 60 seconds of trace', () => {
    expect(trace60).toBe(600);
  });

  const { left, right } = mixOfCircuit(true);
  const lufs = integratedLoudness(left, right);
  const tp = Math.max(truePeakDb(left), truePeakDb(right));

  it('measures -16 +/- 1.5 LUFS integrated', () => {
    console.info(
      `A5: ${lufs.toFixed(2)} LUFS, ${tp.toFixed(2)} dBTP (level_offset_db ${profile.mix.levelOffsetDb.toString()})`,
    );
    expect(lufs).toBeGreaterThan(-17.5);
    expect(lufs).toBeLessThan(-14.5);
  });

  it('peaks at no more than -1 dBTP', () => {
    expect(tp).toBeLessThanOrEqual(-1);
  });

  it('is not silent or clipped anywhere: every second of it is audible', () => {
    for (let s = 0; s < 60; s++) {
      let sum = 0;
      for (let i = s * 48_000; i < (s + 1) * 48_000; i++) sum += (left[i] as number) ** 2;
      const db = 10 * Math.log10(sum / 48_000 + 1e-20);
      expect(db).toBeGreaterThan(-45);
    }
  });

  it('the chase view is just as loud (the canopy is not there to quieten it)', () => {
    const chase = mixOfCircuit(false);
    const chaseLufs = integratedLoudness(chase.left, chase.right);
    const chaseTp = Math.max(truePeakDb(chase.left), truePeakDb(chase.right));
    console.info(
      `A5 chase: ${chaseLufs.toFixed(2)} LUFS, ${chaseTp.toFixed(2)} dBTP (chase_offset_db ${profile.mix.chaseOffsetDb.toString()})`,
    );
    expect(chaseLufs).toBeGreaterThan(-17.5);
    expect(chaseLufs).toBeLessThan(-14.5);
    expect(chaseTp).toBeLessThanOrEqual(-1);
  }, 60_000);
});
