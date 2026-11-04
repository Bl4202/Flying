import { describe, expect, it } from 'vitest';
import { fft, integratedLoudness, levelDb, peakFrequency, truePeakDb } from './analysis.ts';

const sine = (hz: number, seconds: number, amp = 1, phase = 0, fs = 48_000): Float32Array =>
  Float32Array.from(
    { length: Math.round(seconds * fs) },
    (_, i) => amp * Math.sin((2 * Math.PI * hz * i) / fs + phase),
  );

describe('fft and peakFrequency', () => {
  it('transforms a single bin to a single peak', () => {
    const n = 64;
    const re = Float64Array.from({ length: n }, (_, i) => Math.cos((2 * Math.PI * 5 * i) / n));
    const im = new Float64Array(n);
    fft(re, im);
    const mag = (k: number): number => Math.hypot(re[k] as number, im[k] as number);
    expect(mag(5)).toBeCloseTo(n / 2, 6);
    expect(mag(7)).toBeLessThan(1e-9);
  });

  it('finds a tone to a hundredth of a per cent, between bins too', () => {
    for (const hz of [440, 3200.5, 4160, 9999]) {
      expect(peakFrequency(sine(hz, 1), 48_000, 100, 12_000)).toBeCloseTo(hz, 0);
      expect(Math.abs(peakFrequency(sine(hz, 1), 48_000, 100, 12_000) - hz) / hz).toBeLessThan(
        0.0002,
      );
    }
  });

  it('looks only between the limits it is given', () => {
    const two = Float32Array.from(
      sine(1000, 1).map((v, i) => v + 0.5 * Math.sin((2 * Math.PI * 5000 * i) / 48_000)),
    );
    expect(peakFrequency(two, 48_000, 100, 3000)).toBeCloseTo(1000, 0);
    expect(peakFrequency(two, 48_000, 3000, 8000)).toBeCloseTo(5000, 0);
  });
});

describe('levelDb', () => {
  it('is -3 dB for a full-scale sine and -6 dB for half of it', () => {
    expect(levelDb(sine(1000, 1))).toBeCloseTo(-3.01, 2);
    expect(levelDb(sine(1000, 1, 0.5))).toBeCloseTo(-9.03, 2);
  });
});

describe('integratedLoudness (BS.1770-4)', () => {
  it('reads 0 LUFS for a full-scale 997 Hz sine in both channels, and -3 in one', () => {
    const s = sine(997, 5);
    expect(integratedLoudness(s, s)).toBeCloseTo(0, 1);
    expect(integratedLoudness(s, new Float32Array(s.length))).toBeCloseTo(-3.01, 1);
  });

  it('falls 1 LU for every dB the signal does', () => {
    const loud = sine(997, 5, 0.5);
    const quiet = sine(997, 5, 0.25);
    expect(integratedLoudness(loud, loud) - integratedLoudness(quiet, quiet)).toBeCloseTo(6.02, 1);
  });

  it('ignores silence: a long quiet tail barely moves the figure (the gates)', () => {
    const tone = sine(997, 4, 0.1);
    const padded = new Float32Array(tone.length * 3);
    padded.set(tone);
    const alone = integratedLoudness(tone, tone);
    // Blocks that straddle the tone's end are partly loud and pass the gates, so the figure
    // moves a little; an ungated mean over the padding would be 4.8 dB lower.
    expect(Math.abs(integratedLoudness(padded, padded) - alone)).toBeLessThan(0.3);
  });

  it('is below the absolute gate for silence', () => {
    expect(integratedLoudness(new Float32Array(48_000 * 2), new Float32Array(48_000 * 2))).toBe(
      -Infinity,
    );
  });
});

describe('truePeakDb', () => {
  it('finds the peak between samples: a sine at a quarter of the sample rate, caught off its crest', () => {
    // Samples of +-0.7071: the sample peak is -3 dB but the waveform reaches 0 dBFS.
    const x = sine(12_000, 0.1, 1, Math.PI / 4);
    expect(Math.max(...x)).toBeLessThan(0.72);
    expect(truePeakDb(x)).toBeGreaterThan(-0.4);
    expect(truePeakDb(x)).toBeLessThan(0.3);
  });

  it('is the sample peak for a low tone', () => {
    expect(truePeakDb(sine(200, 0.2, 0.5))).toBeCloseTo(-6.02, 1);
  });
});
