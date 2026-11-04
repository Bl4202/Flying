/**
 * The renderer's slices of the presentation bundle (ADR-43): the HUD layout, the cockpit, the
 * exterior model and the HUD stroke font, parsed from the baked JSON into typed objects. The
 * baker has already validated all of it; this parser checks the shape again so that a bundle from
 * a mismatched build becomes an error naming the value, not an `undefined` three calls later.
 *
 * Frames. HUD positions are in the HUD's tangent plane, milliradians, `u` right and `v` up from
 * the boresight. Cockpit and model positions are metres in body axes (x forward, y right, z down)
 * from the aircraft's reference point.
 */
import {
  type AircraftPresentation,
  type PresentBundle,
  PresentError,
  type Slice,
} from '@flying/sim-bridge/present';

export type Vec3 = readonly [number, number, number];
export type Vec2 = readonly [number, number];

const vec3 = (s: Slice): Vec3 => s.tuple(3) as unknown as Vec3;
const vec2 = (s: Slice): Vec2 => s.tuple(2) as unknown as Vec2;

// ---------------------------------------------------------------------------------------------
// HUD.

export const HUD_SPEED_CHANNELS = ['cas', 'tas', 'ground_speed'] as const;
export const HUD_ALTITUDE_CHANNELS = ['altitude_msl'] as const;
export const HUD_LOAD_CHANNELS = ['nz', 'g_lagged'] as const;
export type SpeedChannel = (typeof HUD_SPEED_CHANNELS)[number];
export type AltitudeChannel = (typeof HUD_ALTITUDE_CHANNELS)[number];
export type LoadChannel = (typeof HUD_LOAD_CHANNELS)[number];

/** The HUD element types, as written in `type`. */
export const HUD_LAYER_TYPES = [
  'boresight',
  'flight_path_marker',
  'pitch_ladder',
  'heading_tape',
  'speed_tape',
  'altitude_tape',
  'vs_caret',
  'g_meter',
  'aoa_bracket',
  'bank_scale',
  'engine_fuel_block',
  'mode_line',
  'warnings',
] as const;
export type HudLayerType = (typeof HUD_LAYER_TYPES)[number];

export interface HudLayerBase {
  readonly id: string;
}
export type HudLayerSpec =
  | (HudLayerBase & { type: 'boresight'; sizeMrad: number })
  | (HudLayerBase & { type: 'flight_path_marker'; sizeMrad: number; cageBelowMps: number })
  | (HudLayerBase & {
      type: 'pitch_ladder';
      stepDeg: number;
      labelStepDeg: number;
      halfWidthMrad: number;
      gapMrad: number;
      declutterAboveDeg: number;
    })
  | (HudLayerBase & {
      type: 'heading_tape';
      atMrad: Vec2;
      spanMrad: number;
      windowDeg: number;
      tickDeg: number;
      labelDeg: number;
    })
  | (HudLayerBase & {
      type: 'speed_tape';
      atMrad: Vec2;
      spanMrad: number;
      channel: SpeedChannel;
      windowKn: number;
      tickKn: number;
      labelKn: number;
      machFrom: number;
    })
  | (HudLayerBase & {
      type: 'altitude_tape';
      atMrad: Vec2;
      spanMrad: number;
      channel: AltitudeChannel;
      windowFt: number;
      tickFt: number;
      labelFt: number;
      radarBelowFt: number;
    })
  | (HudLayerBase & { type: 'vs_caret'; atMrad: Vec2; spanMrad: number; rangeFpm: number })
  | (HudLayerBase & {
      type: 'g_meter';
      atMrad: Vec2;
      channel: LoadChannel;
      amberPct: number;
      redPct: number;
    })
  | (HudLayerBase & {
      type: 'aoa_bracket';
      atMrad: Vec2;
      onSpeedDeg: number;
      halfRangeDeg: number;
    })
  | (HudLayerBase & { type: 'bank_scale'; radiusMrad: number; tickDeg: number })
  | (HudLayerBase & { type: 'engine_fuel_block'; atMrad: Vec2 })
  | (HudLayerBase & { type: 'mode_line'; atMrad: Vec2 })
  | (HudLayerBase & { type: 'warnings'; atMrad: Vec2; maxLines: number });

export interface HudSpec {
  readonly id: string;
  readonly aircraft: string;
  /** Total field of view of the combiner, horizontal then vertical, degrees. */
  readonly tfovDeg: Vec2;
  readonly eyeBoxMm: Vec3;
  readonly font: string;
  /** `#rrggbb`. */
  readonly colour: string;
  readonly glowPx: number;
  readonly strokeMrad: number;
  readonly textMrad: number;
  readonly layers: readonly HudLayerSpec[];
}

function parseLayer(s: Slice): HudLayerSpec {
  const type = s.at('type').oneOf(HUD_LAYER_TYPES);
  const id = s.s('id');
  const at = (): Vec2 => vec2(s.at('at_mrad'));
  switch (type) {
    case 'boresight':
      return { type, id, sizeMrad: s.n('size_mrad', 0) };
    case 'flight_path_marker':
      return { type, id, sizeMrad: s.n('size_mrad', 0), cageBelowMps: s.n('cage_below_mps', 0) };
    case 'pitch_ladder':
      return {
        type,
        id,
        stepDeg: s.n('step_deg', 0.01),
        labelStepDeg: s.n('label_step_deg', 0.01),
        halfWidthMrad: s.n('half_width_mrad', 0),
        gapMrad: s.n('gap_mrad', 0),
        declutterAboveDeg: s.n('declutter_above_deg', 0),
      };
    case 'heading_tape':
      return {
        type,
        id,
        atMrad: at(),
        spanMrad: s.n('span_mrad', 0),
        windowDeg: s.n('window_deg', 0.01),
        tickDeg: s.n('tick_deg', 0.01),
        labelDeg: s.n('label_deg', 0.01),
      };
    case 'speed_tape':
      return {
        type,
        id,
        atMrad: at(),
        spanMrad: s.n('span_mrad', 0),
        channel: s.at('channel').oneOf(HUD_SPEED_CHANNELS),
        windowKn: s.n('window_kn', 0.01),
        tickKn: s.n('tick_kn', 0.01),
        labelKn: s.n('label_kn', 0.01),
        machFrom: s.n('mach_from', 0),
      };
    case 'altitude_tape':
      return {
        type,
        id,
        atMrad: at(),
        spanMrad: s.n('span_mrad', 0),
        channel: s.at('channel').oneOf(HUD_ALTITUDE_CHANNELS),
        windowFt: s.n('window_ft', 0.01),
        tickFt: s.n('tick_ft', 0.01),
        labelFt: s.n('label_ft', 0.01),
        radarBelowFt: s.n('radar_below_ft', 0),
      };
    case 'vs_caret':
      return {
        type,
        id,
        atMrad: at(),
        spanMrad: s.n('span_mrad', 0),
        rangeFpm: s.n('range_fpm', 1),
      };
    case 'g_meter':
      return {
        type,
        id,
        atMrad: at(),
        channel: s.at('channel').oneOf(HUD_LOAD_CHANNELS),
        amberPct: s.n('amber_pct', 0),
        redPct: s.n('red_pct', 0),
      };
    case 'aoa_bracket':
      return {
        type,
        id,
        atMrad: at(),
        onSpeedDeg: s.n('on_speed_deg', 0),
        halfRangeDeg: s.n('half_range_deg', 0.01),
      };
    case 'bank_scale':
      return { type, id, radiusMrad: s.n('radius_mrad', 0), tickDeg: s.n('tick_deg', 0.01) };
    case 'engine_fuel_block':
      return { type, id, atMrad: at() };
    case 'mode_line':
      return { type, id, atMrad: at() };
    case 'warnings':
      return { type, id, atMrad: at(), maxLines: s.at('max_lines').int(1, 3) };
  }
}

/** Reads a `hud/1` slice. */
export function parseHud(s: Slice): HudSpec {
  const colour = s.at('colour');
  const text = colour.str();
  if (!/^#[0-9a-fA-F]{6}$/.test(text)) colour.fail(`\`${text}\` is not #rrggbb`);
  const layers = s.at('layers').items().map(parseLayer);
  const seen = new Set<string>();
  for (const [i, l] of layers.entries()) {
    if (seen.has(l.type)) s.at('layers').items()[i]?.fail(`a second \`${l.type}\` layer`);
    seen.add(l.type);
  }
  return {
    id: s.s('id'),
    aircraft: s.s('aircraft'),
    tfovDeg: s.at('tfov_deg').tuple(2) as unknown as Vec2,
    eyeBoxMm: s.at('eye_box_mm').tuple(3) as unknown as Vec3,
    font: s.s('font'),
    colour: text,
    glowPx: s.n('glow_px', 0),
    strokeMrad: s.n('stroke_mrad', 0.01),
    textMrad: s.n('text_mrad', 0.01),
    layers,
  };
}

// ---------------------------------------------------------------------------------------------
// Cockpit.

export interface BoxSpec {
  readonly id: string;
  readonly minM: Vec3;
  readonly maxM: Vec3;
}

export interface CanopyBowSpec {
  readonly xM: number;
  readonly halfWidthM: number;
  readonly sillZM: number;
  readonly topZM: number;
  readonly thicknessM: number;
}

export interface CockpitSpec {
  readonly id: string;
  readonly aircraft: string;
  readonly eyePointM: Vec3;
  readonly headLimitsDeg: {
    readonly yaw: number;
    readonly pitchUp: number;
    readonly pitchDown: number;
  };
  /** Four corners round the glass, body axes. */
  readonly hudGlassM: readonly [Vec3, Vec3, Vec3, Vec3];
  readonly canopyBows: readonly CanopyBowSpec[];
  readonly glareShield: BoxSpec;
  readonly panel: BoxSpec;
  readonly consoles: readonly BoxSpec[];
}

function parseBox(s: Slice): BoxSpec {
  return { id: s.s('id'), minM: vec3(s.at('min_m')), maxM: vec3(s.at('max_m')) };
}

/** Reads a `cockpit_layout/1` slice. */
export function parseCockpit(s: Slice): CockpitSpec {
  const head = s.at('head_limits_deg');
  const corners = s.at('hud_glass').at('corners_m').items().map(vec3);
  if (corners.length !== 4) s.at('hud_glass').at('corners_m').fail('expected four corners');
  return {
    id: s.s('id'),
    aircraft: s.s('aircraft'),
    eyePointM: vec3(s.at('eye_point_m')),
    headLimitsDeg: {
      yaw: head.n('yaw', 0, 180),
      pitchUp: head.n('pitch_up', 0, 90),
      pitchDown: head.n('pitch_down', 0, 90),
    },
    hudGlassM: corners as unknown as CockpitSpec['hudGlassM'],
    canopyBows: s
      .at('canopy_bows')
      .items()
      .map((b) => ({
        xM: b.n('x_m'),
        halfWidthM: b.n('half_width_m', 0),
        sillZM: b.n('sill_z_m'),
        topZM: b.n('top_z_m'),
        thicknessM: b.n('thickness_m', 0),
      })),
    glareShield: parseBox(s.at('glare_shield')),
    panel: parseBox(s.at('panel')),
    consoles: s.at('consoles').items().map(parseBox),
  };
}

// ---------------------------------------------------------------------------------------------
// Exterior model.

export const MODEL_CHANNELS = [
  'elevator_deg',
  'aileron_deg',
  'rudder_deg',
  'flap_deg',
  'airbrake',
  'gear_retract',
  'afterburner',
] as const;
export type ModelChannel = (typeof MODEL_CHANNELS)[number];

export interface StationSpec {
  readonly xM: number;
  readonly halfWidthM: number;
  readonly topZM: number;
  readonly bottomZM: number;
}
export interface NozzleSpec {
  readonly part: string;
  readonly atM: Vec3;
  readonly radiusM: number;
  readonly lengthM: number;
}
export interface SurfaceSpec {
  readonly id: string;
  readonly part: string | undefined;
  readonly mirror: boolean;
  readonly rootLeM: Vec3;
  readonly rootChordM: number;
  readonly tipLeM: Vec3;
  readonly tipChordM: number;
  readonly rootThicknessM: number;
  readonly tipThicknessM: number;
}
export interface ModelBoxSpec extends BoxSpec {
  readonly part: string | undefined;
}
export interface GearLegSpec {
  readonly id: string;
  readonly part: string;
  readonly contactM: Vec3;
  readonly topM: Vec3;
  readonly wheelRadiusM: number;
  readonly wheelWidthM: number;
}
export type PartSpec =
  | {
      readonly motion: 'rotate';
      readonly id: string;
      readonly channel: ModelChannel;
      readonly pivotM: Vec3;
      readonly axis: Vec3;
      readonly rotateDegPerUnit: number;
    }
  | { readonly motion: 'glow'; readonly id: string; readonly channel: ModelChannel };

export interface ModelSpec {
  readonly id: string;
  readonly aircraft: string;
  readonly fuselage: readonly StationSpec[];
  readonly canopy: readonly StationSpec[];
  readonly nozzle: NozzleSpec;
  readonly surfaces: readonly SurfaceSpec[];
  readonly boxes: readonly ModelBoxSpec[];
  readonly gear: readonly GearLegSpec[];
  readonly parts: readonly PartSpec[];
}

function parseStation(s: Slice): StationSpec {
  return {
    xM: s.n('x_m'),
    halfWidthM: s.n('half_width_m', 0),
    topZM: s.n('top_z_m'),
    bottomZM: s.n('bottom_z_m'),
  };
}

function optPart(s: Slice): string | undefined {
  return s.opt('part')?.str();
}

/** Reads a `model/1` slice. */
export function parseModel(s: Slice): ModelSpec {
  const parts: PartSpec[] = s
    .at('parts')
    .items()
    .map((p): PartSpec => {
      const motion = p.at('motion').oneOf(['rotate', 'glow'] as const);
      const id = p.s('id');
      const channel = p.at('channel').oneOf(MODEL_CHANNELS);
      if (motion === 'glow') return { motion, id, channel };
      return {
        motion,
        id,
        channel,
        pivotM: vec3(p.at('pivot_m')),
        axis: vec3(p.at('axis')),
        rotateDegPerUnit: p.n('rotate_deg_per_unit'),
      };
    });
  const known = new Set(parts.map((p) => p.id));
  const partRef = (ref: Slice | undefined): void => {
    const name = ref?.str();
    if (ref !== undefined && name !== undefined && !known.has(name)) {
      ref.fail(`there is no part \`${name}\``);
    }
  };
  const nozzle = s.at('nozzle');
  partRef(nozzle.at('part'));
  const surfaces = s
    .at('surfaces')
    .items()
    .map((sf): SurfaceSpec => {
      partRef(sf.opt('part'));
      return {
        id: sf.s('id'),
        part: optPart(sf),
        mirror: sf.opt('mirror')?.bool() ?? false,
        rootLeM: vec3(sf.at('root_le_m')),
        rootChordM: sf.n('root_chord_m', 0),
        tipLeM: vec3(sf.at('tip_le_m')),
        tipChordM: sf.n('tip_chord_m', 0),
        rootThicknessM: sf.n('root_thickness_m', 0),
        tipThicknessM: sf.n('tip_thickness_m', 0),
      };
    });
  const boxes = s
    .at('boxes')
    .items()
    .map((b): ModelBoxSpec => {
      partRef(b.opt('part'));
      return { ...parseBox(b), part: optPart(b) };
    });
  const gear = s
    .at('gear')
    .items()
    .map((g): GearLegSpec => {
      partRef(g.at('part'));
      return {
        id: g.s('id'),
        part: g.s('part'),
        contactM: vec3(g.at('contact_m')),
        topM: vec3(g.at('top_m')),
        wheelRadiusM: g.n('wheel_radius_m', 0),
        wheelWidthM: g.n('wheel_width_m', 0),
      };
    });
  return {
    id: s.s('id'),
    aircraft: s.s('aircraft'),
    fuselage: s.at('fuselage').items().map(parseStation),
    canopy: s.at('canopy').items().map(parseStation),
    nozzle: {
      part: nozzle.s('part'),
      atM: vec3(nozzle.at('at_m')),
      radiusM: nozzle.n('radius_m', 0),
      lengthM: nozzle.n('length_m', 0),
    },
    surfaces,
    boxes,
    gear,
    parts,
  };
}

// ---------------------------------------------------------------------------------------------
// Stroke font.

export interface GlyphSpec {
  readonly ch: string;
  /** Polylines of `[x, y]` in grid units, `y` up from the baseline. */
  readonly strokes: readonly (readonly Vec2[])[];
}

export interface FontSpec {
  readonly id: string;
  /** Glyph grid, width and height, in grid units. */
  readonly cell: Vec2;
  readonly advance: number;
  readonly glyphs: ReadonlyMap<string, GlyphSpec>;
}

/** Reads a `stroke_font/1` slice. */
export function parseFont(s: Slice): FontSpec {
  const glyphs = new Map<string, GlyphSpec>();
  for (const g of s.at('glyphs').items()) {
    const ch = g.at('ch').str();
    if (Array.from(ch).length !== 1) g.at('ch').fail(`\`${ch}\` is not one character`);
    if (glyphs.has(ch)) g.at('ch').fail(`\`${ch}\` is defined twice`);
    glyphs.set(ch, {
      ch,
      strokes: g
        .at('strokes')
        .items()
        .map((stroke) => stroke.items().map(vec2)),
    });
  }
  return {
    id: s.s('id'),
    cell: s.at('cell').tuple(2) as unknown as Vec2,
    advance: s.at('advance').int(1),
    glyphs,
  };
}

// ---------------------------------------------------------------------------------------------
// Everything for one aircraft.

/** What the renderer needs to draw one aircraft's HUD, cockpit and exterior. */
export interface RenderPresentation {
  readonly hud: HudSpec;
  readonly cockpit: CockpitSpec;
  readonly model: ModelSpec;
  readonly font: FontSpec;
}

/**
 * The renderer's presentation for `aircraftId`, or undefined if the bundle has none (the game then
 * draws the world without a HUD, cockpit or model).
 *
 * @throws PresentError if the bundle has some of it but not all, or any of it is malformed.
 */
export function renderPresentation(
  bundle: PresentBundle,
  aircraftId: string,
): RenderPresentation | undefined {
  const a: AircraftPresentation | undefined = bundle.forAircraft(aircraftId);
  if (a === undefined) return undefined;
  const missing = (what: string): never => {
    throw new PresentError(`/aircraft/${aircraftId}`, `has no ${what}`);
  };
  const hudSlice = a.hud ?? missing('HUD');
  const hud = parseHud(hudSlice);
  const cockpit = parseCockpit(a.cockpit ?? missing('cockpit'));
  const model = parseModel(a.model ?? missing('model'));
  const font = parseFont(a.font ?? missing('font'));
  return { hud, cockpit, model, font };
}
