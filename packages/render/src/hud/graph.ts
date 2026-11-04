/**
 * The HUD's drawing list (ADR-41): straight segments in the body-fixed tangent plane, in
 * milliradians. A point `(u, v)` is the body direction `(1, u / 1000, -v / 1000)`, so a symbol
 * stays where the aircraft's nose and the combiner glass put it, and a conformal symbol (the
 * ladder, the flight-path marker) is simply drawn where the world direction falls in that plane.
 * The vertex shader turns each segment into a capsule on the screen.
 *
 * The list is one preallocated `Float32Array`, `SEGMENT_FLOATS` per segment, refilled each frame
 * without making any garbage. Text is drawn from the stroke font's polylines, one segment per
 * stroke edge; a number is written digit by digit through a scratch buffer, so no string is made.
 */
import type { FontSpec, GlyphSpec } from '../present.ts';

/** Floats per segment: x0, y0, x1, y1 (mrad), width scale, alpha, tint (0 normal, 1 amber, 2 red), reserved. */
export const SEGMENT_FLOATS = 8;
/** Most segments a frame may hold. */
export const MAX_SEGMENTS = 4096;

export type Align = 'left' | 'centre' | 'right';

/** Character codes the font needs for a number. */
const CH_0 = 48;
const CH_MINUS = 45;
const CH_DOT = 46;

export class HudGraph {
  /** The segments: `count` of them are valid. */
  readonly segments = new Float32Array(MAX_SEGMENTS * SEGMENT_FLOATS);
  count = 0;
  /** Segments that did not fit (a layout drawing too much): reported, never fatal. */
  dropped = 0;
  /** Characters the font lacked, since the last reset (a test sweeps the HUD and wants none). */
  missing = 0;
  /** Width scale, alpha and tint (0 normal, 1 amber, 2 red) for what is drawn next. */
  weight = 1;
  alpha = 1;
  tint = 0;

  readonly #glyphs: (GlyphSpec | undefined)[] = [];
  /** Glyph grid size and advance, in grid units. */
  readonly #cellW: number;
  readonly #cellH: number;
  readonly #advance: number;
  /** The digits of a number being written, as character codes. */
  readonly #digits = new Uint8Array(24);

  constructor(font: FontSpec) {
    this.#cellW = font.cell[0];
    this.#cellH = font.cell[1];
    this.#advance = font.advance;
    for (const [ch, glyph] of font.glyphs) this.#glyphs[ch.codePointAt(0) ?? 0] = glyph;
  }

  /** Empties the list for a new frame. */
  reset(): void {
    this.count = 0;
    this.dropped = 0;
    this.missing = 0;
    this.weight = 1;
    this.alpha = 1;
    this.tint = 0;
  }

  /** Does the font draw this character? */
  has(code: number): boolean {
    return this.#glyphs[code] !== undefined;
  }

  /** A segment from `(x0, y0)` to `(x1, y1)`, in mrad. */
  line(x0: number, y0: number, x1: number, y1: number): void {
    if (this.count >= MAX_SEGMENTS) {
      this.dropped += 1;
      return;
    }
    const s = this.segments;
    const at = this.count * SEGMENT_FLOATS;
    s[at] = x0;
    s[at + 1] = y0;
    s[at + 2] = x1;
    s[at + 3] = y1;
    s[at + 4] = this.weight;
    s[at + 5] = this.alpha;
    s[at + 6] = this.tint;
    s[at + 7] = 0;
    this.count += 1;
  }

  /** A line drawn as dashes: `dashes` equal pieces, half of each drawn. */
  dashed(x0: number, y0: number, x1: number, y1: number, dashes: number): void {
    const n = Math.max(1, Math.floor(dashes));
    for (let i = 0; i < n; i++) {
      const a = i / n;
      const b = (i + 0.5) / n;
      this.line(x0 + (x1 - x0) * a, y0 + (y1 - y0) * a, x0 + (x1 - x0) * b, y0 + (y1 - y0) * b);
    }
  }

  /** The outline of an axis-aligned box. */
  box(x0: number, y0: number, x1: number, y1: number): void {
    this.line(x0, y0, x1, y0);
    this.line(x1, y0, x1, y1);
    this.line(x1, y1, x0, y1);
    this.line(x0, y1, x0, y0);
  }

  /** A circle of `n` segments. */
  circle(cx: number, cy: number, r: number, n: number): void {
    const step = (2 * Math.PI) / n;
    let px = cx + r;
    let py = cy;
    for (let i = 1; i <= n; i++) {
      const a = i * step;
      const x = cx + r * Math.cos(a);
      const y = cy + r * Math.sin(a);
      this.line(px, py, x, y);
      px = x;
      py = y;
    }
  }

  /** The width of `count` characters at `height` mrad, mrad. */
  textWidth(count: number, height: number): number {
    return (count * this.#advance * height) / this.#cellH;
  }

  /**
   * One character with its lower left corner at `(x, y)`, `height` mrad tall (the font's grid
   * height). Characters the font lacks are skipped (a test checks that every string the HUD can
   * make is covered).
   */
  glyph(code: number, x: number, y: number, height: number): void {
    const g = this.#glyphs[code];
    if (g === undefined) {
      this.missing += 1;
      return;
    }
    const k = height / this.#cellH;
    for (let s = 0; s < g.strokes.length; s++) {
      const stroke = g.strokes[s];
      if (stroke === undefined) continue;
      for (let i = 1; i < stroke.length; i++) {
        const a = stroke[i - 1];
        const b = stroke[i];
        if (a === undefined || b === undefined) continue;
        this.line(x + a[0] * k, y + a[1] * k, x + b[0] * k, y + b[1] * k);
      }
    }
  }

  /** A string, aligned about `x`. Makes no garbage: it reads character codes. */
  text(s: string, x: number, y: number, height: number, align: Align = 'left'): void {
    const width = this.textWidth(s.length, height);
    let at = align === 'left' ? x : align === 'centre' ? x - width / 2 : x - width;
    const advance = (this.#advance * height) / this.#cellH;
    for (let i = 0; i < s.length; i++) {
      this.glyph(s.charCodeAt(i), at, y, height);
      at += advance;
    }
  }

  /**
   * A whole number, rounded, with at least `minDigits` digits (zero padded) and a minus sign if
   * negative, aligned about `x`.
   */
  integer(
    value: number,
    minDigits: number,
    x: number,
    y: number,
    height: number,
    align: Align,
  ): void {
    const n = this.#writeNumber(value, minDigits, 0);
    this.#drawDigits(n, x, y, height, align);
  }

  /** A number with `decimals` places (at most 3), aligned about `x`. */
  fixed(value: number, decimals: number, x: number, y: number, height: number, align: Align): void {
    const n = this.#writeNumber(value, 1, decimals);
    this.#drawDigits(n, x, y, height, align);
  }

  /** The digit characters of `value` into the scratch buffer; returns how many. */
  #writeNumber(value: number, minDigits: number, decimals: number): number {
    const d = this.#digits;
    const scale = decimals === 0 ? 1 : decimals === 1 ? 10 : decimals === 2 ? 100 : 1000;
    let v = Math.round(Math.abs(Number.isFinite(value) ? value : 0) * scale);
    const negative = value < 0 && v !== 0;
    // Written backwards, then reversed in place.
    let n = 0;
    let placed = 0;
    do {
      d[n++] = CH_0 + (v % 10);
      v = Math.floor(v / 10);
      placed += 1;
      if (decimals > 0 && placed === decimals) d[n++] = CH_DOT;
    } while (v > 0 || placed < Math.max(minDigits, decimals + 1));
    if (negative) d[n++] = CH_MINUS;
    for (let i = 0, j = n - 1; i < j; i++, j--) {
      const t = d[i] ?? 0;
      d[i] = d[j] ?? 0;
      d[j] = t;
    }
    return n;
  }

  #drawDigits(n: number, x: number, y: number, height: number, align: Align): void {
    const width = this.textWidth(n, height);
    let at = align === 'left' ? x : align === 'centre' ? x - width / 2 : x - width;
    const advance = (this.#advance * height) / this.#cellH;
    for (let i = 0; i < n; i++) {
      this.glyph(this.#digits[i] ?? 0, at, y, height);
      at += advance;
    }
  }

  /** The font's cell width over its height: how wide a glyph's drawing is relative to its height. */
  get glyphAspect(): number {
    return this.#cellW / this.#cellH;
  }
}
