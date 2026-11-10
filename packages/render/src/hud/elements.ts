/**
 * The HUD's symbols (ADR-41, GDD 54.2, 54.3): each element writes segments into the drawing list
 * from the aircraft's state and its layer in the HUD data. All coordinates are milliradians in the
 * body-fixed tangent plane (see `graph.ts`), right and up positive.
 *
 * Conformal symbols (the pitch ladder, the flight-path marker, the horizon) are placed by taking a
 * world direction (against the local horizon, from `state.ts`) into body axes and then into the
 * plane `(u, v) = (y / x, -z / x) * 1000`; a test checks that those directions are the world's, to
 * 0.01 degrees. Tapes scroll under a fixed pointer. Nothing here allocates: numbers go through the
 * graph's digit buffer, strings are constants.
 *
 * Every symbol has a cue that is not colour (a box, a flash, a word), so the picture reads in any
 * palette (AC-57.1's spirit): the g meter is boxed and flashes from 85 per cent of the limit, a
 * warning is boxed and flashes, the AoA caret has its bracket.
 */
import { COMBAT_FLAG, SEEKER_MODE } from '@flying/sim-bridge/layout';
import type { HudLayerSpec, HudSpec } from '../present.ts';
import { HUD_WARNING_IDS, type HudTexts } from './texts.ts';
import type { HudGraph } from './graph.ts';
import { HS } from './state.ts';

const DEG = Math.PI / 180;

/** The declutter levels: 0 full, 1 reduced, 2 minimum. */
export type Declutter = 0 | 1 | 2;

/** What every element draws from. */
export interface HudContext {
  readonly g: HudGraph;
  /** `HudState.v`. */
  readonly s: Float64Array;
  readonly spec: HudSpec;
  readonly texts: HudTexts;
  declutter: Declutter;
  /** Half the combiner's field, mrad (tan of half the angle): where symbols may be. */
  readonly halfU: number;
  readonly halfV: number;
}

/** Scratch for the conformal placement: `(u, v)` of the last direction. */
const placed = new Float64Array(2);
/** The body direction being placed, and the ladder's elevation and azimuth offset: scratch, so no number crosses a call. */
const direction = new Float64Array(5);

/**
 * Where a direction given in body axes (written to `direction[0..3]`) falls in the HUD plane, in
 * mrad, into `placed`. False if it is behind the aircraft (it has no place in the plane).
 */
function placeDirection(): boolean {
  const dx = direction[0] as number;
  if (dx < 1e-6) return false;
  placed[0] = ((direction[1] as number) / dx) * 1000;
  placed[1] = (-(direction[2] as number) / dx) * 1000;
  return true;
}

/** `placeDirection` for a direction given as arguments (tests and the zenith and nadir). */
export function placeBodyDirection(dx: number, dy: number, dz: number): boolean {
  direction[0] = dx;
  direction[1] = dy;
  direction[2] = dz;
  return placeDirection();
}

/** The last `placeBodyDirection`'s result. */
export function lastPlaced(): Readonly<Float64Array> {
  return placed;
}

// ---------------------------------------------------------------------------------------------
// The fixed symbols.

function boresight(c: HudContext, size: number): void {
  const g = c.g;
  const gap = size * 0.35;
  g.line(-size, 0, -gap, 0);
  g.line(gap, 0, size, 0);
  g.line(0, -size, 0, -gap);
  g.line(0, gap, 0, size);
}

function flightPathMarker(c: HudContext, size: number): void {
  const g = c.g;
  if ((c.s[HS.FPM_AHEAD] as number) === 0) return;
  const caged = (c.s[HS.FPM_CAGED] as number) !== 0;
  const x = c.s[HS.FPM_U] as number;
  const y = c.s[HS.FPM_V] as number;
  if (Math.abs(x) > c.halfU + size || Math.abs(y) > c.halfV + size) return;
  const r = size * 0.35;
  if (caged) {
    // Caged below the cage speed: a dashed circle at the boresight, wings only.
    for (let i = 0; i < 8; i += 2) {
      const a0 = (i * Math.PI) / 4;
      const a1 = ((i + 1) * Math.PI) / 4;
      g.line(
        x + r * Math.cos(a0),
        y + r * Math.sin(a0),
        x + r * Math.cos(a1),
        y + r * Math.sin(a1),
      );
    }
  } else {
    g.circle(x, y, r, 12);
  }
  g.line(x - r, y, x - r - size * 0.55, y);
  g.line(x + r, y, x + r + size * 0.55, y);
  g.line(x, y + r, x, y + r + size * 0.4);
}

// ---------------------------------------------------------------------------------------------
// The pitch ladder.

/**
 * The ladder's direction at elevation `direction[3]` and azimuth offset `direction[4]` from the
 * heading, in the HUD plane (into `placed`).
 */
function ladderPoint(c: HudContext): boolean {
  const s = c.s;
  const theta = direction[3] as number;
  const delta = direction[4] as number;
  const ct = Math.cos(theta);
  const st = Math.sin(theta);
  const cd = Math.cos(delta);
  const sd = Math.sin(delta);
  // P = H x U, to the right of the heading.
  const hx = s[HS.HEAD_X] as number;
  const hy = s[HS.HEAD_Y] as number;
  const hz = s[HS.HEAD_Z] as number;
  const ux = s[HS.UP_X] as number;
  const uy = s[HS.UP_Y] as number;
  const uz = s[HS.UP_Z] as number;
  const px = hy * uz - hz * uy;
  const py = hz * ux - hx * uz;
  const pz = hx * uy - hy * ux;
  direction[0] = ct * (cd * hx + sd * px) + st * ux;
  direction[1] = ct * (cd * hy + sd * py) + st * uy;
  direction[2] = ct * (cd * hz + sd * pz) + st * uz;
  return placeDirection();
}

/** Places the ladder point at elevation `theta` and offset `delta`. */
function ladderAt(c: HudContext, theta: number, delta: number): boolean {
  direction[3] = theta;
  direction[4] = delta;
  return ladderPoint(c);
}

function pitchLadder(c: HudContext, l: Extract<HudLayerSpec, { type: 'pitch_ladder' }>): void {
  const g = c.g;
  const pitchDeg = Math.abs((c.s[HS.PITCH] as number) / DEG);
  // Steeper than the limit the ladder thins out to its labelled lines.
  const step = pitchDeg > l.declutterAboveDeg ? l.labelStepDeg : l.stepDeg;
  const text = c.spec.textMrad * 0.8;
  const tick = 6;
  const first = Math.ceil(-85 / step);
  const last = Math.floor(85 / step);
  for (let k = first; k <= last; k++) {
    const deg = k * step;
    const theta = deg * DEG;
    const horizon = deg === 0;
    // Half-widths as angles; a line at elevation theta spans delta = w / cos(theta) either side.
    const cosT = Math.max(Math.cos(theta), 0.2);
    const w = (horizon ? l.halfWidthMrad * 2.5 : l.halfWidthMrad) / 1000 / cosT;
    const gap = l.gapMrad / 1000 / 2 / cosT;
    if (!ladderAt(c, theta, -w)) continue;
    const ax = placed[0] as number;
    const ay = placed[1] as number;
    if (!ladderAt(c, theta, -gap)) continue;
    const bx = placed[0] as number;
    const by = placed[1] as number;
    if (!ladderAt(c, theta, gap)) continue;
    const cx = placed[0] as number;
    const cy = placed[1] as number;
    if (!ladderAt(c, theta, w)) continue;
    const dx = placed[0] as number;
    const dy = placed[1] as number;
    // Skip lines wholly outside the field.
    const margin = 30;
    const limitU = c.halfU + margin;
    const limitV = c.halfV + margin;
    if (!(
      (Math.abs(ax) <= limitU && Math.abs(ay) <= limitV) ||
      (Math.abs(dx) <= limitU && Math.abs(dy) <= limitV) ||
      (Math.abs(bx) <= limitU && Math.abs(by) <= limitV)
    )) {
      continue;
    }
    // The ends turn towards the horizon: down for the lines above it, up for those below.
    const sign = deg > 0 ? -1 : 1;
    // Unit vector of the line, to place the end ticks perpendicular to it.
    const lx = dx - ax;
    const ly = dy - ay;
    const ln = Math.hypot(lx, ly) || 1;
    const nx = (-ly / ln) * tick * sign * (horizon ? 0 : 1);
    const ny = (lx / ln) * tick * sign * (horizon ? 0 : 1);
    if (deg < 0) {
      g.dashed(ax, ay, bx, by, 3);
      g.dashed(cx, cy, dx, dy, 3);
    } else {
      g.line(ax, ay, bx, by);
      g.line(cx, cy, dx, dy);
    }
    if (!horizon) {
      g.line(ax, ay, ax + nx, ay + ny);
      g.line(dx, dy, dx + nx, dy + ny);
    }
    if (!horizon && deg % l.labelStepDeg === 0 && c.declutter === 0) {
      const label = Math.abs(deg);
      g.integer(label, 1, ax - 3, ay - text / 2, text, 'right');
      g.integer(label, 1, dx + 3, dy - text / 2, text, 'left');
    }
  }
  // The zenith and the nadir, where the lines converge.
  if (c.declutter === 0) {
    const ux = c.s[HS.UP_X] as number;
    const uy = c.s[HS.UP_Y] as number;
    const uz = c.s[HS.UP_Z] as number;
    if (
      placeBodyDirection(ux, uy, uz) &&
      Math.abs(placed[0] as number) <= c.halfU &&
      Math.abs(placed[1] as number) <= c.halfV
    ) {
      g.circle(placed[0] as number, placed[1] as number, 8, 10);
      g.line(
        (placed[0] as number) - 12,
        placed[1] as number,
        (placed[0] as number) + 12,
        placed[1] as number,
      );
    }
    if (
      placeBodyDirection(-ux, -uy, -uz) &&
      Math.abs(placed[0] as number) <= c.halfU &&
      Math.abs(placed[1] as number) <= c.halfV
    ) {
      g.circle(placed[0] as number, placed[1] as number, 8, 10);
      const [x, y] = [placed[0] as number, placed[1] as number];
      g.line(x - 6, y - 6, x + 6, y + 6);
      g.line(x - 6, y + 6, x + 6, y - 6);
    }
  }
}

// ---------------------------------------------------------------------------------------------
// The heading tape.

function headingTape(c: HudContext, l: Extract<HudLayerSpec, { type: 'heading_tape' }>): void {
  const g = c.g;
  const [cx, cy] = l.atMrad;
  const heading = (c.s[HS.HEADING] as number) / DEG;
  const scale = l.spanMrad / l.windowDeg;
  const text = c.spec.textMrad * 0.8;
  const half = l.windowDeg / 2;
  g.line(cx - l.spanMrad / 2, cy, cx + l.spanMrad / 2, cy);
  if (c.declutter < 2) {
    const first = Math.ceil((heading - half) / l.tickDeg);
    const last = Math.floor((heading + half) / l.tickDeg);
    for (let k = first; k <= last; k++) {
      const deg = k * l.tickDeg;
      const x = cx + (deg - heading) * scale;
      const major = Math.abs(deg / l.labelDeg - Math.round(deg / l.labelDeg)) < 1e-6;
      g.line(x, cy, x, cy + (major ? 9 : 5));
      if (major) {
        const wrapped = ((Math.round(deg / 10) % 36) + 36) % 36;
        g.integer(wrapped, 2, x, cy + 11, text, 'centre');
      }
    }
  }
  // The pointer, and the number in a box above it.
  g.line(cx, cy, cx - 4, cy - 7);
  g.line(cx, cy, cx + 4, cy - 7);
  g.line(cx - 4, cy - 7, cx + 4, cy - 7);
  const bw = g.textWidth(3, text) / 2 + 3;
  g.box(cx - bw, cy - 9 - text - 4, cx + bw, cy - 9);
  g.integer(Math.round(heading) % 360 || 0, 3, cx, cy - 9 - text - 2, text, 'centre');
}

// ---------------------------------------------------------------------------------------------
// The scrolling tapes.

/** The tick marks and labels of a vertical tape, scrolling under the pointer at `(cx, cy)`. */
function verticalTape(
  c: HudContext,
  cx: number,
  cy: number,
  span: number,
  value: number,
  window: number,
  tick: number,
  labelEvery: number,
  labelDivisor: number,
): void {
  const g = c.g;
  const side = cx < 0 ? -1 : 1;
  const scale = span / window;
  const text = c.spec.textMrad * 0.8;
  g.line(cx, cy - span / 2, cx, cy + span / 2);
  if (c.declutter === 2) return;
  const half = window / 2;
  const first = Math.ceil((value - half) / tick);
  const last = Math.floor((value + half) / tick);
  for (let k = first; k <= last; k++) {
    const v = k * tick;
    const y = cy + (v - value) * scale;
    const major = Math.abs(v / labelEvery - Math.round(v / labelEvery)) < 1e-6;
    // Ticks point towards the middle of the display; labels stand outside.
    g.line(cx, y, cx - side * (major ? 10 : 6), y);
    if (major && Math.abs(y - cy) > text) {
      g.integer(
        v / labelDivisor,
        1,
        cx + side * 3,
        y - text / 2,
        text,
        side < 0 ? 'right' : 'left',
      );
    }
  }
}

/** The readout box at the pointer: `value` in a box `width` characters wide, outside the scale. */
function readout(
  c: HudContext,
  cx: number,
  cy: number,
  value: number,
  width: number,
  minDigits = width,
): void {
  const g = c.g;
  const digits = width;
  const side = cx < 0 ? -1 : 1;
  const text = c.spec.textMrad;
  const w = g.textWidth(digits, text) + 6;
  const x0 = side < 0 ? cx - 4 - w : cx + 4;
  g.box(x0, cy - text / 2 - 3, x0 + w, cy + text / 2 + 3);
  g.integer(value, minDigits, x0 + w / 2, cy - text / 2, text, 'centre');
  // The pointer from the box to the scale.
  g.line(side < 0 ? cx - 4 : cx + 4, cy, cx, cy);
}

function speedTape(c: HudContext, l: Extract<HudLayerSpec, { type: 'speed_tape' }>): void {
  const [cx, cy] = l.atMrad;
  const cas = c.s[HS.CAS_KN] as number;
  verticalTape(c, cx, cy, l.spanMrad, cas, l.windowKn, l.tickKn, l.labelKn, 1);
  readout(c, cx, cy, Math.round(cas), 3);
  const mach = c.s[HS.MACH] as number;
  if (mach >= l.machFrom) {
    const g = c.g;
    const text = c.spec.textMrad * 0.8;
    const y = cy - l.spanMrad / 2 - text - 4;
    g.text('M', cx - 24, y, text, 'left');
    g.fixed(mach, 2, cx - 24 + g.textWidth(1, text), y, text, 'left');
  }
}

function altitudeTape(c: HudContext, l: Extract<HudLayerSpec, { type: 'altitude_tape' }>): void {
  const g = c.g;
  const [cx, cy] = l.atMrad;
  const alt = c.s[HS.ALT_FT] as number;
  verticalTape(c, cx, cy, l.spanMrad, alt, l.windowFt, l.tickFt, l.labelFt, 100);
  readout(c, cx, cy, Math.round(alt / 10) * 10, 5, 1);
  const agl = c.s[HS.AGL_FT] as number;
  if (agl < l.radarBelowFt && (c.s[HS.ON_GROUND] as number) === 0) {
    const text = c.spec.textMrad * 0.8;
    const y = cy - l.spanMrad / 2 - text - 4;
    g.text(c.texts.radar, cx + 4, y, text, 'left');
    g.integer(Math.round(agl / 10) * 10, 1, cx + 4 + g.textWidth(2, text), y, text, 'left');
  }
}

function verticalSpeed(c: HudContext, l: Extract<HudLayerSpec, { type: 'vs_caret' }>): void {
  const g = c.g;
  const [cx, cy] = l.atMrad;
  const half = l.spanMrad / 2;
  g.line(cx, cy - half, cx, cy + half);
  for (let k = -3; k <= 3; k++) {
    const fpm = (k * l.rangeFpm) / 3;
    g.line(
      cx,
      cy + (fpm / l.rangeFpm) * half,
      cx - (k === 0 ? 8 : 4),
      cy + (fpm / l.rangeFpm) * half,
    );
  }
  const vs = Math.min(Math.max((c.s[HS.VS_FPM] as number) / l.rangeFpm, -1), 1);
  const y = cy + vs * half;
  g.line(cx, y, cx + 7, y - 4);
  g.line(cx, y, cx + 7, y + 4);
  g.line(cx + 7, y - 4, cx + 7, y + 4);
}

// ---------------------------------------------------------------------------------------------
// The blocks.

function gMeter(c: HudContext, l: Extract<HudLayerSpec, { type: 'g_meter' }>): void {
  const g = c.g;
  const [x, y] = l.atMrad;
  const text = c.spec.textMrad;
  const nz = c.s[HS.NZ] as number;
  const limit = Math.max(c.s[HS.N_MAX] as number, 1);
  const fraction = (Math.abs(nz) / limit) * 100;
  const red = fraction >= l.redPct;
  const amber = !red && fraction >= l.amberPct;
  const t = c.s[HS.TIME_S] as number;
  // Amber flashes at 2 Hz in a box; red flashes at 4 Hz in a double box.
  const on = !(amber || red) || Math.floor(t * (red ? 8 : 4)) % 2 === 0;
  g.tint = red ? 2 : amber ? 1 : 0;
  g.alpha = on ? 1 : 0.25;
  g.text('G', x, y, text, 'left');
  g.fixed(nz, 1, x + g.textWidth(2, text), y, text, 'left');
  if (amber || red) {
    const w = g.textWidth(6, text);
    g.box(x - 3, y - 3, x + w + 3, y + text + 3);
    if (red) g.box(x - 6, y - 6, x + w + 6, y + text + 6);
  }
  g.alpha = 1;
  g.tint = 0;
  if (c.declutter < 2) {
    const small = text * 0.8;
    g.text(c.texts.max, x, y - small - 6, small, 'left');
    g.fixed(c.s[HS.MAX_NZ] as number, 1, x + g.textWidth(4, small), y - small - 6, small, 'left');
  }
}

function aoaBracket(c: HudContext, l: Extract<HudLayerSpec, { type: 'aoa_bracket' }>): void {
  // Only on approach: the gear is down.
  if ((c.s[HS.GEAR_DOWN] as number) === 0) return;
  const g = c.g;
  const [cx, cy] = l.atMrad;
  const half = 30;
  const alpha = c.s[HS.ALPHA_DEG] as number;
  // The bracket "[" with its centre mark, the on-speed angle.
  g.line(cx, cy - half, cx, cy + half);
  g.line(cx, cy - half, cx + 6, cy - half);
  g.line(cx, cy + half, cx + 6, cy + half);
  g.line(cx, cy, cx + 9, cy);
  const dev = Math.min(Math.max((alpha - l.onSpeedDeg) / l.halfRangeDeg, -1), 1);
  const y = cy + dev * half;
  g.line(cx - 2, y, cx - 10, y - 4);
  g.line(cx - 2, y, cx - 10, y + 4);
  g.line(cx - 10, y - 4, cx - 10, y + 4);
  g.glyph(945, cx - 20, cy + half + 4, c.spec.textMrad * 0.8);
}

function bankScale(c: HudContext, l: Extract<HudLayerSpec, { type: 'bank_scale' }>): void {
  const g = c.g;
  const r = l.radiusMrad;
  const bank = c.s[HS.BANK] as number;
  // The arc over the top, from -60 to +60 degrees, ticks every `tickDeg`, longer every 30.
  for (let deg = -60; deg <= 60; deg += l.tickDeg) {
    const a = deg * DEG;
    const len = deg % 30 === 0 ? 10 : 5;
    g.line(r * Math.sin(a), r * Math.cos(a), (r + len) * Math.sin(a), (r + len) * Math.cos(a));
  }
  // The sky pointer: where the world's up falls as the aircraft banks.
  const a = -Math.min(Math.max(bank, -75 * DEG), 75 * DEG);
  const tx = r * Math.sin(a);
  const ty = r * Math.cos(a);
  g.line(tx, ty, (r - 8) * Math.sin(a - 0.05), (r - 8) * Math.cos(a - 0.05));
  g.line(tx, ty, (r - 8) * Math.sin(a + 0.05), (r - 8) * Math.cos(a + 0.05));
  g.line(
    (r - 8) * Math.sin(a - 0.05),
    (r - 8) * Math.cos(a - 0.05),
    (r - 8) * Math.sin(a + 0.05),
    (r - 8) * Math.cos(a + 0.05),
  );
}

function engineFuelBlock(
  c: HudContext,
  l: Extract<HudLayerSpec, { type: 'engine_fuel_block' }>,
): void {
  const g = c.g;
  const [x, y] = l.atMrad;
  const text = c.spec.textMrad * 0.8;
  const line = text * 1.5;
  const t = c.texts;
  let row = 0;
  const label = (s: string, value: number, digits: number): void => {
    const at = y - row * line;
    g.text(s, x, at, text, 'left');
    g.integer(value, digits, x + g.textWidth(s.length + 1, text), at, text, 'left');
    row += 1;
  };
  label(t.throttle, Math.round(c.s[HS.THROTTLE_PCT] as number), 1);
  if ((c.s[HS.AB_LEVEL] as number) > 0.05)
    label(t.afterburner, Math.round((c.s[HS.AB_LEVEL] as number) * 100), 1);
  label(t.fuel, Math.round(c.s[HS.FUEL_KG] as number), 1);
  label(t.fuelFlow, Math.round(c.s[HS.FUEL_FLOW_KGPM] as number), 1);
  label(t.endurance, Math.round(c.s[HS.ENDURANCE_MIN] as number), 1);
}

function modeLine(c: HudContext, l: Extract<HudLayerSpec, { type: 'mode_line' }>): void {
  const g = c.g;
  const [x, y] = l.atMrad;
  const text = c.spec.textMrad * 0.8;
  const t = c.texts;
  const gap = text * 0.6;
  // Collect the words (constants, no strings made) and centre the run.
  const assist = Math.min(Math.max(Math.round(c.s[HS.ASSIST] as number), 1), 4);
  const words: string[] = scratchWords;
  let n = 0;
  words[n++] = t.assist[assist - 1] as string;
  if ((c.s[HS.GEAR_DOWN] as number) !== 0) words[n++] = t.gear;
  const flaps = c.s[HS.FLAPS] as number;
  if (flaps === 1) words[n++] = t.flapsTakeOff;
  else if (flaps === 2) words[n++] = t.flapsLanding;
  if ((c.s[HS.BRAKES] as number) > 0.1) words[n++] = t.brakes;
  if ((c.s[HS.PARKING] as number) !== 0) words[n++] = t.parking;
  let width = -gap;
  for (let i = 0; i < n; i++) width += g.textWidth((words[i] as string).length, text) + gap;
  let at = x - width / 2;
  for (let i = 0; i < n; i++) {
    const word = words[i] as string;
    g.text(word, at, y, text, 'left');
    at += g.textWidth(word.length, text) + gap;
  }
}
const scratchWords: string[] = ['', '', '', '', '', ''];

function warnings(c: HudContext, l: Extract<HudLayerSpec, { type: 'warnings' }>): void {
  const g = c.g;
  const bits = c.s[HS.WARNINGS] as number;
  if (bits === 0) return;
  const [x, y] = l.atMrad;
  const text = c.spec.textMrad;
  const t = c.s[HS.TIME_S] as number;
  let row = 0;
  // The lowest bit is the most urgent: the first lines are the top of GDD 54.8's priority.
  for (let i = 0; i < HUD_WARNING_IDS.length && row < l.maxLines; i++) {
    if ((bits & (1 << i)) === 0) continue;
    const id = HUD_WARNING_IDS[i] as string;
    const word = c.texts.warnings[id] ?? id;
    const red = i < 4; // the warnings; the rest are cautions
    const at = y - row * (text + 8);
    g.tint = red ? 2 : 1;
    // Warnings flash and are boxed (so they read without colour); cautions are boxed and steady.
    g.alpha = !red || Math.floor(t * 4) % 2 === 0 ? 1 : 0.3;
    const w = g.textWidth(word.length, text);
    g.text(word, x, at, text, 'centre');
    g.box(x - w / 2 - 4, at - 3, x + w / 2 + 4, at + text + 3);
    row += 1;
  }
  g.tint = 0;
  g.alpha = 1;
}

// ---------------------------------------------------------------------------------------------
// The duel's symbols (ADR-41 amendment). Drawn only in a duel (`COMBAT_FLAG.ACTIVE`). The pipper
// and the target are directions from the eye to points the core gives (the gunsight's aim point
// and the bandit), placed like the conformal symbols, so they sit on what they mark.

/** Segments in a full circle, and in the gun ring's arc per turn. */
const RING_SEGMENTS = 32;

function flag(c: HudContext, bit: number): boolean {
  return ((c.s[HS.COMBAT_FLAGS] as number) & bit) !== 0;
}

/**
 * The gun reticle while the gunsight has a solution: the pipper dot at the aim point, the ranging
 * ring around it with an arc of `range / ring_max_m` of a turn (clockwise from the top; a full
 * ring with no range), and SHOOT under the ring while the core says the bandit is on the pipper in
 * range.
 */
function gunReticle(c: HudContext, l: Extract<HudLayerSpec, { type: 'gun_reticle' }>): void {
  if (!flag(c, COMBAT_FLAG.PIPPER) || (c.s[HS.PIP_AHEAD] as number) === 0) return;
  const g = c.g;
  const x = c.s[HS.PIP_U] as number;
  const y = c.s[HS.PIP_V] as number;
  if (Math.abs(x) > c.halfU + l.ringMrad || Math.abs(y) > c.halfV + l.ringMrad) return;
  // The pipper: a small filled-looking dot (a tight circle and a cross).
  g.circle(x, y, l.pipperMrad, 8);
  g.line(x - l.pipperMrad * 0.5, y, x + l.pipperMrad * 0.5, y);
  g.line(x, y - l.pipperMrad * 0.5, x, y + l.pipperMrad * 0.5);
  const range = c.s[HS.TGT_RANGE_M] as number;
  const fraction = range > 0 ? Math.min(range / l.ringMaxM, 1) : 1;
  ringArc(c, x, y, l.ringMrad, fraction);
  if (flag(c, COMBAT_FLAG.SHOOT_GUN)) {
    const text = c.spec.textMrad;
    g.text(c.texts.shoot, x, y - l.ringMrad - text * 1.4, text, 'centre');
  }
}

/** An arc of `fraction` of a turn of radius `r` about `(x, y)`, clockwise from the top. */
export function ringArc(c: HudContext, x: number, y: number, r: number, fraction: number): void {
  const n = Math.max(1, Math.ceil(RING_SEGMENTS * fraction));
  const total = 2 * Math.PI * fraction;
  let px = x;
  let py = y + r;
  for (let i = 1; i <= n; i++) {
    const a = (total * i) / n;
    const qx = x + r * Math.sin(a);
    const qy = y + r * Math.cos(a);
    c.g.line(px, py, qx, qy);
    px = qx;
    py = qy;
  }
}

/**
 * The target: within `show_within_m`, a box on the bandit with `R <km>` under it and `VC <knots>`
 * beside it while it is in the field of view; outside it, an arrowhead on the locator circle
 * pointing at the bandit, with the range beside it.
 */
function targetBox(c: HudContext, l: Extract<HudLayerSpec, { type: 'target_box' }>): void {
  if (!flag(c, COMBAT_FLAG.TARGET) || (c.s[HS.BANDIT] as number) === 0) return;
  const range = c.s[HS.TGT_RANGE_M] as number;
  if (!(range <= l.showWithinM)) return;
  const g = c.g;
  const text = c.spec.textMrad * 0.8;
  const half = l.sizeMrad / 2;
  const x = c.s[HS.BANDIT_U] as number;
  const y = c.s[HS.BANDIT_V] as number;
  const inside =
    (c.s[HS.BANDIT_AHEAD] as number) !== 0 &&
    Math.abs(x) + half <= c.halfU &&
    Math.abs(y) + half <= c.halfV;
  const t = c.texts;
  if (inside) {
    g.box(x - half, y - half, x + half, y + half);
    const below = y - half - text * 1.4;
    g.text(t.rangeLabel, x - g.textWidth(1, text), below, text, 'right');
    g.fixed(range / 1000, 1, x, below, text, 'left');
    const side = x + half + text * 0.5;
    g.text(t.closureLabel, side, y - text / 2, text, 'left');
    g.integer(
      c.s[HS.TGT_CLOSURE_KN] as number,
      1,
      side + g.textWidth(t.closureLabel.length + 1, text),
      y - text / 2,
      text,
      'left',
    );
    return;
  }
  const a = c.s[HS.BANDIT_ANGLE] as number;
  const r = l.locatorRadiusMrad;
  const ca = Math.cos(a);
  const sa = Math.sin(a);
  const tipX = r * ca;
  const tipY = r * sa;
  const back = half * 0.6;
  // An arrowhead: two strokes back from the tip, either side of the line to the bandit.
  g.line(tipX, tipY, tipX - back * ca + back * 0.6 * sa, tipY - back * sa - back * 0.6 * ca);
  g.line(tipX, tipY, tipX - back * ca - back * 0.6 * sa, tipY - back * sa + back * 0.6 * ca);
  const lx = (r - back - text) * ca;
  const ly = (r - back - text) * sa;
  g.fixed(range / 1000, 1, lx, ly - text / 2, text, 'centre');
}

/**
 * The seeker: in SEARCH a circle at the boresight (the caged seeker's field), in TONE the same with
 * a doubled stroke, and LOCKED the circle and a diamond on the bandit.
 */
function seeker(c: HudContext, l: Extract<HudLayerSpec, { type: 'seeker' }>): void {
  const mode = c.s[HS.SEEKER] as number;
  if (mode === SEEKER_MODE.OFF) return;
  const g = c.g;
  g.circle(0, 0, l.circleMrad, RING_SEGMENTS);
  if (mode === SEEKER_MODE.TONE) {
    g.circle(0, 0, l.circleMrad + c.spec.strokeMrad * 1.5, RING_SEGMENTS);
  }
  if (mode === SEEKER_MODE.LOCKED && (c.s[HS.BANDIT_AHEAD] as number) !== 0) {
    const x = c.s[HS.BANDIT_U] as number;
    const y = c.s[HS.BANDIT_V] as number;
    const d = l.diamondMrad;
    g.line(x, y + d, x + d, y);
    g.line(x + d, y, x, y - d);
    g.line(x, y - d, x - d, y);
    g.line(x - d, y, x, y + d);
  }
}

/**
 * The weapon status: `GUN <rounds>  SRM <missiles>`, then `ARM` or `SAFE`, the seeker's `TONE` or
 * `LOCK`, and `SHOOT`, `IN RNG` or, for 1.5 s after a refused launch, why it was refused.
 */
function weaponStatus(c: HudContext, l: Extract<HudLayerSpec, { type: 'weapon_status' }>): void {
  const g = c.g;
  const [x, y] = l.atMrad;
  const text = c.spec.textMrad * 0.8;
  const line = text * 1.5;
  const t = c.texts;
  const gap = g.textWidth(1, text);
  let at = x;
  g.text(t.gun, at, y, text, 'left');
  at += g.textWidth(t.gun.length, text) + gap;
  g.integer(c.s[HS.ROUNDS] as number, 1, at, y, text, 'left');
  at += g.textWidth(4, text) + gap;
  g.text(t.srm, at, y, text, 'left');
  at += g.textWidth(t.srm.length, text) + gap;
  g.integer(c.s[HS.MISSILES] as number, 1, at, y, text, 'left');
  const words: string[] = scratchWords;
  let n = 0;
  words[n++] = flag(c, COMBAT_FLAG.ARMED) ? t.arm : t.safe;
  const mode = c.s[HS.SEEKER] as number;
  if (mode === SEEKER_MODE.LOCKED) words[n++] = t.lock;
  else if (mode === SEEKER_MODE.TONE) words[n++] = t.tone;
  const inhibit = c.s[HS.INHIBIT] as number;
  if (inhibit > 0 && (c.s[HS.INHIBIT_AGE] as number) < INHIBIT_SHOW_S) {
    words[n++] = t.inhibit[inhibit - 1] ?? '';
  } else if (flag(c, COMBAT_FLAG.SHOOT_MISSILE)) {
    words[n++] = t.shoot;
  } else if (flag(c, COMBAT_FLAG.IN_RANGE)) {
    words[n++] = t.inRange;
  }
  at = x;
  for (let i = 0; i < n; i++) {
    const word = words[i] as string;
    g.text(word, at, y - line, text, 'left');
    at += g.textWidth(word.length, text) + gap;
  }
}

/** How long the HUD shows why a launch was refused, s (as the core's `INHIBIT_SHOW_S`). */
const INHIBIT_SHOW_S = 1.5;

/** HIT for `hit_s` after one of the player's rounds hits, SPLASH for `splash_s` after a kill. */
function killCue(c: HudContext, l: Extract<HudLayerSpec, { type: 'kill_cue' }>): void {
  const [x, y] = l.atMrad;
  const text = c.spec.textMrad * 1.2;
  if ((c.s[HS.SPLASH_AGE] as number) < l.splashS) {
    c.g.text(c.texts.splash, x, y, text, 'centre');
  } else if ((c.s[HS.HIT_AGE] as number) < l.hitS) {
    c.g.text(c.texts.hit, x, y, text, 'centre');
  }
}

// ---------------------------------------------------------------------------------------------
// The whole HUD.

/** Which layers a declutter level removes (level 1 and above, level 2 and above). */
function hidden(type: HudLayerSpec['type'], level: Declutter): boolean {
  if (level >= 1) {
    if (type === 'bank_scale' || type === 'engine_fuel_block' || type === 'mode_line') return true;
  }
  if (level >= 2) {
    if (type === 'vs_caret' || type === 'heading_tape') return true;
  }
  return false;
}

/** Draws one layer. */
export function drawLayer(c: HudContext, layer: HudLayerSpec): void {
  if (hidden(layer.type, c.declutter)) return;
  switch (layer.type) {
    case 'boresight':
      boresight(c, layer.sizeMrad);
      break;
    case 'flight_path_marker':
      flightPathMarker(c, layer.sizeMrad);
      break;
    case 'pitch_ladder':
      pitchLadder(c, layer);
      break;
    case 'heading_tape':
      headingTape(c, layer);
      break;
    case 'speed_tape':
      speedTape(c, layer);
      break;
    case 'altitude_tape':
      altitudeTape(c, layer);
      break;
    case 'vs_caret':
      verticalSpeed(c, layer);
      break;
    case 'g_meter':
      gMeter(c, layer);
      break;
    case 'aoa_bracket':
      aoaBracket(c, layer);
      break;
    case 'bank_scale':
      bankScale(c, layer);
      break;
    case 'engine_fuel_block':
      engineFuelBlock(c, layer);
      break;
    case 'mode_line':
      modeLine(c, layer);
      break;
    case 'warnings':
      warnings(c, layer);
      break;
    case 'gun_reticle':
    case 'target_box':
    case 'seeker':
    case 'weapon_status':
    case 'kill_cue':
      if (flag(c, COMBAT_FLAG.ACTIVE)) drawCombat(c, layer);
      break;
  }
}

function drawCombat(
  c: HudContext,
  layer: Extract<
    HudLayerSpec,
    { type: 'gun_reticle' | 'target_box' | 'seeker' | 'weapon_status' | 'kill_cue' }
  >,
): void {
  switch (layer.type) {
    case 'gun_reticle':
      gunReticle(c, layer);
      break;
    case 'target_box':
      targetBox(c, layer);
      break;
    case 'seeker':
      seeker(c, layer);
      break;
    case 'weapon_status':
      weaponStatus(c, layer);
      break;
    case 'kill_cue':
      killCue(c, layer);
      break;
  }
}
