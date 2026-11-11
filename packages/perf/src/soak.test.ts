import { describe, expect, it } from 'vitest';
import {
  slopePerTenMinutes,
  soakVerdict,
  type SoakReport,
  type SoakSample,
  type SortieStart,
} from './soak.ts';

/** A seeded noise in [-1, 1), the same every run. */
function noise(seed: number): () => number {
  let x = seed >>> 0 || 1;
  return () => {
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    return (x >>> 0) / 0x8000_0000 - 1;
  };
}

/** 31 samples over 30 minutes, every figure steady unless `vary` changes it. */
function report(
  vary: (s: SoakSample, i: number) => void = () => undefined,
  starts: SortieStart[] = [0, 480, 960, 1440, 1700].map((tS) => ({
    tS,
    nodes: 900,
    listeners: 40,
    liveBuffers: 31,
    liveTextures: 12,
  })),
): SoakReport {
  const samples = Array.from({ length: 31 }, (_, i) => {
    const s: SoakSample = {
      tS: i * 60,
      jsBytes: 60e6,
      jsSource: 'uasm',
      wasmBytes: 33_554_432,
      liveBuffers: 31,
      liveTextures: 12,
      liveAudioNodes: 4,
      silences: 0,
      workerErrors: 0,
      deviceLosses: 0,
      workers: 2,
      simHz: 60,
      sorties: Math.floor(i / 8),
    };
    vary(s, i);
    return s;
  });
  return {
    schema: 'soak_report/1',
    build: 'b1',
    url: 'http://127.0.0.1:4173/Flying/',
    warmupS: 120,
    seconds: 1800,
    samples,
    sortieStarts: starts,
    consoleErrors: [],
  };
}

describe('slopePerTenMinutes', () => {
  it('is the least-squares slope per 600 s', () => {
    expect(slopePerTenMinutes([])).toBe(0);
    expect(slopePerTenMinutes([{ t: 0, v: 5 }])).toBe(0);
    expect(slopePerTenMinutes([0, 60, 120].map((t) => ({ t, v: 1000 + t })))).toBeCloseTo(600, 9);
    expect(slopePerTenMinutes([0, 60, 120].map((t) => ({ t, v: 7 })))).toBe(0);
  });
});

describe('soakVerdict (ADR-45 L1 to L7)', () => {
  it('passes a flat run', () => {
    const v = soakVerdict(report());
    expect(v.pass).toBe(true);
    for (const k of ['L1', 'L2', 'L3', 'L4', 'L5', 'L6', 'L7'] as const) {
      expect(v[k]?.pass, `${k}: ${v[k]?.detail ?? ''}`).toBe(true);
    }
  });

  it('passes a flat run with noise of a few megabytes', () => {
    const r = noise(7);
    const v = soakVerdict(
      report((s) => {
        s.jsBytes += 3e6 * r();
      }),
    );
    expect(v.L1?.pass, v.L1?.detail).toBe(true);
  });

  it('fails L1 for a leak of 2 MB per 10 minutes (the leak-js fault leaks far more)', () => {
    const v = soakVerdict(
      report((s) => {
        s.jsBytes += (2e6 * s.tS) / 600;
      }),
    );
    expect(v.L1?.pass).toBe(false);
    expect(v.L1?.detail).toMatch(/^2\.00 MB per 10 min/);
    expect(v.pass).toBe(false);
  });

  it('fails L2 when the DOM or the listeners grow from sortie to sortie', () => {
    const grow = (k: number): SortieStart => ({
      tS: k * 480,
      nodes: 900 + k * 15,
      listeners: 40 + k,
      liveBuffers: 31,
      liveTextures: 12,
    });
    const v = soakVerdict(report(undefined, [0, 1, 2, 3, 4].map(grow)));
    expect(v.L2?.pass).toBe(false);
    expect(v.L2?.detail).toBe('nodes 915 -> 960, listeners 41 -> 44');
  });

  it('fails L3 when the WebAssembly memory grows after the first sortie', () => {
    const v = soakVerdict(
      report((s, i) => {
        if (i > 20) s.wasmBytes += 65_536;
      }),
    );
    expect(v.L3?.pass).toBe(false);
  });

  it('fails L4 on a silence, a worker error, a lost device, or a worker more', () => {
    for (const k of ['silences', 'workerErrors', 'deviceLosses'] as const) {
      const v = soakVerdict(
        report((s, i) => {
          if (i === 30) s[k] = 1;
        }),
      );
      expect(v.L4?.pass, k).toBe(false);
    }
    expect(
      soakVerdict(
        report((s, i) => {
          if (i === 10) s.workers = 3;
        }),
      ).L4?.pass,
    ).toBe(false);
  });

  it('fails L5 when a sortie start finds a GPU buffer more (the leak-gpu fault)', () => {
    const leak = [0, 480, 960, 1440, 1700].map((tS, k) => ({
      tS,
      nodes: 900,
      listeners: 40,
      liveBuffers: 31 + k,
      liveTextures: 12,
    }));
    const v = soakVerdict(report(undefined, leak));
    expect(v.L5?.pass).toBe(false);
    expect(v.L5?.detail).toBe('buffers 32, 33, 34, 35; textures 12, 12, 12, 12');
  });

  it('fails L6 when the audio nodes do not stay put', () => {
    const v = soakVerdict(
      report((s, i) => {
        s.liveAudioNodes = 4 + (i % 2);
      }),
    );
    expect(v.L6?.pass).toBe(false);
  });

  it('fails L7 on a slow simulation, a console error, or too few sorties in a full run', () => {
    expect(
      soakVerdict(
        report((s) => {
          s.simHz = 58;
        }),
      ).L7?.pass,
    ).toBe(false);
    expect(
      soakVerdict(
        report((s) => {
          s.simHz = 58;
        }),
        { ci: true },
      ).L7?.pass,
    ).toBe(true);
    const errors = report();
    errors.consoleErrors = ['TypeError: x is undefined'];
    expect(soakVerdict(errors).L7?.pass).toBe(false);
    const short = report(
      undefined,
      [0, 480].map((tS) => ({ tS, nodes: 1, listeners: 1, liveBuffers: 1, liveTextures: 1 })),
    );
    expect(soakVerdict(short).L7?.pass).toBe(false);
  });

  it('judges only L3, L4, L6 and L7 in the quick check, without the sortie count', () => {
    const short = report(
      (s) => {
        s.jsBytes += (5e6 * s.tS) / 600;
      },
      [{ tS: 0, nodes: 1, listeners: 1, liveBuffers: 1, liveTextures: 1 }],
    );
    const v = soakVerdict(short, { gates: 'quick', ci: true });
    expect([v.L1, v.L2, v.L5]).toEqual([null, null, null]);
    expect(v.pass).toBe(true);
  });
});
