import { describe, expect, it } from 'vitest';
import { bytesPerCall } from '../test-support.ts';
import { HudGraph, MAX_SEGMENTS, SEGMENT_FLOATS } from './graph.ts';
import { kestrel, segmentsOf } from './test-support.ts';

const graph = (): HudGraph => new HudGraph(kestrel().font);

describe('HudGraph', () => {
  it('holds segments of eight floats, up to 4 096 a frame', () => {
    expect(SEGMENT_FLOATS).toBe(8);
    expect(MAX_SEGMENTS).toBe(4096);
    expect(graph().segments.length).toBe(4096 * 8);
  });

  it('records a line with the width, alpha and tint it is drawn with', () => {
    const g = graph();
    g.weight = 1.6;
    g.alpha = 0.5;
    g.tint = 2;
    g.line(1, 2, 3, 4);
    expect(segmentsOf(g)).toEqual([[1, 2, 3, 4, 1.600000023841858, 0.5, 2]]);
    g.reset();
    expect(g.count).toBe(0);
    g.line(0, 0, 1, 1);
    expect(segmentsOf(g)[0]?.slice(4)).toEqual([1, 1, 0]);
  });

  it('counts what does not fit instead of failing', () => {
    const g = graph();
    for (let i = 0; i < MAX_SEGMENTS + 5; i++) g.line(0, 0, 1, 1);
    expect(g.count).toBe(MAX_SEGMENTS);
    expect(g.dropped).toBe(5);
  });

  it('draws a box as four lines, a circle as n, dashes as half of each piece', () => {
    const g = graph();
    g.box(0, 0, 10, 5);
    expect(g.count).toBe(4);
    g.circle(0, 0, 5, 12);
    expect(g.count).toBe(16);
    g.reset();
    g.dashed(0, 0, 100, 0, 4);
    expect(segmentsOf(g).map((s) => [s[0], s[2]])).toEqual([
      [0, 12.5],
      [25, 37.5],
      [50, 62.5],
      [75, 87.5],
    ]);
  });

  describe('text', () => {
    it('is as wide as its characters: 6 grid units each of a 6-unit-tall font', () => {
      const g = graph();
      // The Kestrel font has a 4x6 cell and an advance of 6: at 12 mrad tall a character is 12 wide.
      expect(g.textWidth(1, 12)).toBe(12);
      expect(g.textWidth(5, 12)).toBe(60);
      expect(g.glyphAspect).toBeCloseTo(4 / 6, 12);
    });

    it('draws a glyph from its strokes, scaled to the height and placed at the corner', () => {
      const g = graph();
      g.glyph('1'.charCodeAt(0), 100, 50, 12);
      // The "1" is two strokes: three points and two (two edges, then one).
      expect(g.count).toBe(3);
      const s = segmentsOf(g);
      // Stroke [[1,5],[2,6],[2,0]] at 2 mrad per unit.
      expect(s[0]?.slice(0, 4)).toEqual([102, 60, 104, 62]);
      for (const seg of s) {
        for (const v of [seg[0], seg[2]]) expect(v).toBeGreaterThanOrEqual(100);
        for (const v of [seg[1], seg[3]]) expect(v).toBeGreaterThanOrEqual(50);
      }
    });

    it('aligns a string left, centred or right about x', () => {
      const [l, c, r] = [graph(), graph(), graph()];
      l.text('10', 0, 0, 12, 'left');
      c.text('10', 0, 0, 12, 'centre');
      r.text('10', 0, 0, 12, 'right');
      const xs = (g: HudGraph): [number, number] => {
        const all = segmentsOf(g).flatMap((s) => [s[0], s[2]]);
        return [Math.min(...all), Math.max(...all)];
      };
      expect(xs(l)[0]).toBeGreaterThanOrEqual(0);
      expect(xs(c)[0]).toBeGreaterThanOrEqual(-12);
      expect(xs(c)[1]).toBeLessThanOrEqual(12);
      expect(xs(r)[1]).toBeLessThanOrEqual(0);
    });

    it('skips a character the font does not have, and says whether it has one', () => {
      const g = graph();
      expect(g.has('A'.charCodeAt(0))).toBe(true);
      expect(g.has('~'.charCodeAt(0))).toBe(false);
      g.text('~', 0, 0, 12);
      expect(g.count).toBe(0);
    });
  });

  describe('numbers', () => {
    // Count the glyphs by drawing the same number as text and comparing the segments.
    const same = (draw: (g: HudGraph) => void, text: string): boolean => {
      const [a, b] = [graph(), graph()];
      draw(a);
      b.text(text, 0, 0, 12, 'left');
      return JSON.stringify(segmentsOf(a)) === JSON.stringify(segmentsOf(b));
    };

    it('writes whole numbers, rounded, with padding', () => {
      expect(
        same((g) => {
          g.integer(253, 1, 0, 0, 12, 'left');
        }, '253'),
      ).toBe(true);
      expect(
        same((g) => {
          g.integer(252.6, 1, 0, 0, 12, 'left');
        }, '253'),
      ).toBe(true);
      expect(
        same((g) => {
          g.integer(7, 3, 0, 0, 12, 'left');
        }, '007'),
      ).toBe(true);
      expect(
        same((g) => {
          g.integer(0, 1, 0, 0, 12, 'left');
        }, '0'),
      ).toBe(true);
      expect(
        same((g) => {
          g.integer(123_456, 1, 0, 0, 12, 'left');
        }, '123456'),
      ).toBe(true);
    });

    it('writes a minus sign for negative numbers, and none for a negative that rounds to zero', () => {
      expect(
        same((g) => {
          g.integer(-15, 1, 0, 0, 12, 'left');
        }, '-15'),
      ).toBe(true);
      expect(
        same((g) => {
          g.integer(-0.2, 1, 0, 0, 12, 'left');
        }, '0'),
      ).toBe(true);
    });

    it('writes fixed decimals', () => {
      expect(
        same((g) => {
          g.fixed(3.14159, 2, 0, 0, 12, 'left');
        }, '3.14'),
      ).toBe(true);
      expect(
        same((g) => {
          g.fixed(0.85, 2, 0, 0, 12, 'left');
        }, '0.85'),
      ).toBe(true);
      expect(
        same((g) => {
          g.fixed(0.05, 1, 0, 0, 12, 'left');
        }, '0.1'),
      ).toBe(true);
      expect(
        same((g) => {
          g.fixed(2, 1, 0, 0, 12, 'left');
        }, '2.0'),
      ).toBe(true);
      expect(
        same((g) => {
          g.fixed(-1.5, 1, 0, 0, 12, 'left');
        }, '-1.5'),
      ).toBe(true);
    });

    it('treats a number that is not finite as zero', () => {
      expect(
        same((g) => {
          g.integer(Number.NaN, 1, 0, 0, 12, 'left');
        }, '0'),
      ).toBe(true);
    });
  });

  it('draws text and numbers without making garbage (U3)', async () => {
    const g = graph();
    let n = 0;
    const bytes = await bytesPerCall(() => {
      n += 1;
      if (n % 400 === 0) g.reset();
      g.text('STALL', 0, 0, 12, 'centre');
      g.integer(n % 999, 3, 0, 0, 12, 'right');
      g.fixed((n % 100) / 10, 1, 0, 0, 12, 'left');
    });
    expect(bytes).toBeLessThan(64);
  });
});
