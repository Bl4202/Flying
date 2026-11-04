/**
 * A small aircraft for the renderer's self-test and the unit tests: every kind of solid and every
 * kind of part (a fuselage, a canopy, a mirrored wing, a moving tail, intakes, a gear leg that
 * retracts and a glowing nozzle), in a few dozen lines. Not production data: the shipped aircraft
 * come from the presentation bundle.
 */
import type { CockpitSpec, FontSpec, GlyphSpec, HudSpec, ModelSpec } from './present.ts';

export const FIXTURE_MODEL: ModelSpec = {
  id: 'fixture:mdl/dart',
  aircraft: 'fixture:ac/dart',
  fuselage: [
    { xM: 7, halfWidthM: 0.05, topZM: 0.45, bottomZM: 0.75 },
    { xM: 4, halfWidthM: 0.62, topZM: -0.62, bottomZM: 1.3 },
    { xM: 0, halfWidthM: 0.8, topZM: -0.75, bottomZM: 1.4 },
    { xM: -4, halfWidthM: 0.62, topZM: -0.5, bottomZM: 1 },
    { xM: -5.2, halfWidthM: 0.5, topZM: -0.2, bottomZM: 0.7 },
  ],
  canopy: [
    { xM: 4.6, halfWidthM: 0.3, topZM: -0.95, bottomZM: -0.55 },
    { xM: 3.2, halfWidthM: 0.5, topZM: -1.55, bottomZM: -0.65 },
    { xM: 1.2, halfWidthM: 0.3, topZM: -1, bottomZM: -0.72 },
  ],
  nozzle: { part: 'glow', atM: [-5.2, 0, 0.25], radiusM: 0.42, lengthM: 0.8 },
  surfaces: [
    {
      id: 'wing',
      part: undefined,
      mirror: true,
      rootLeM: [1.2, 0.75, 0.45],
      rootChordM: 3.3,
      tipLeM: [-0.8, 4.9, 0.4],
      tipChordM: 0.9,
      rootThicknessM: 0.3,
      tipThicknessM: 0.1,
    },
    {
      id: 'stab',
      part: 'elevator',
      mirror: false,
      rootLeM: [-3.6, 0.55, 0.15],
      rootChordM: 2.2,
      tipLeM: [-5, 2.3, 0.2],
      tipChordM: 1,
      rootThicknessM: 0.12,
      tipThicknessM: 0.05,
    },
  ],
  boxes: [{ id: 'intake', part: undefined, minM: [1.2, 0.6, 0.15], maxM: [3.6, 0.95, 0.95] }],
  gear: [
    {
      id: 'nose',
      part: 'gear',
      contactM: [4.58, 0, 2.29],
      topM: [4.58, 0, 1.15],
      wheelRadiusM: 0.28,
      wheelWidthM: 0.16,
    },
  ],
  parts: [
    {
      motion: 'rotate',
      id: 'elevator',
      channel: 'elevator_deg',
      pivotM: [-4.5, 0.55, 0.15],
      axis: [0, 1, 0],
      rotateDegPerUnit: 1,
    },
    {
      motion: 'rotate',
      id: 'gear',
      channel: 'gear_retract',
      pivotM: [4.58, 0, 1.15],
      axis: [0, 1, 0],
      rotateDegPerUnit: -90,
    },
    { motion: 'glow', id: 'glow', channel: 'afterburner' },
  ],
};

export const FIXTURE_COCKPIT: CockpitSpec = {
  id: 'fixture:cpt/dart',
  aircraft: 'fixture:ac/dart',
  eyePointM: [3.6, 0, -0.85],
  headLimitsDeg: { yaw: 135, pitchUp: 60, pitchDown: 60 },
  hudGlassM: [
    [4.15, -0.1375, -0.9475],
    [4.15, 0.1375, -0.9475],
    [4.15, 0.1375, -0.7525],
    [4.15, -0.1375, -0.7525],
  ],
  canopyBows: [
    { xM: 4.75, halfWidthM: 0.5, sillZM: -0.45, topZM: -1.5, thicknessM: 0.05 },
    { xM: 1.9, halfWidthM: 0.62, sillZM: -0.35, topZM: -1.35, thicknessM: 0.05 },
  ],
  glareShield: { id: 'glare', minM: [4, -0.45, -0.72], maxM: [4.9, 0.45, -0.62] },
  panel: { id: 'panel', minM: [4.3, -0.4, -0.6], maxM: [4.8, 0.4, 0.3] },
  consoles: [
    { id: 'console_l', minM: [2.9, -0.62, -0.45], maxM: [4, -0.45, 0.2] },
    { id: 'console_r', minM: [2.9, 0.45, -0.45], maxM: [4, 0.62, 0.2] },
  ],
};

/** Every character the HUD writes, drawn as a box with a diagonal: legible enough for a self-test. */
const FIXTURE_CHARS = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ .-+/%:\u00b0\u03b1';

export const FIXTURE_FONT: FontSpec = {
  id: 'fixture:fnt/boxes',
  cell: [4, 6],
  advance: 6,
  glyphs: new Map<string, GlyphSpec>(
    Array.from(FIXTURE_CHARS, (ch): [string, GlyphSpec] => [
      ch,
      {
        ch,
        strokes:
          ch === ' '
            ? []
            : [
                [
                  [0, 0],
                  [4, 0],
                  [4, 6],
                  [0, 6],
                  [0, 0],
                ],
                [
                  [0, 0],
                  [4, 6],
                ],
              ],
      },
    ]),
  ),
};

/** A HUD with the symbols a self-test looks for: the boresight, the flight-path marker, the ladder and the tapes. */
export const FIXTURE_HUD: HudSpec = {
  id: 'fixture:hud/dart',
  aircraft: 'fixture:ac/dart',
  tfovDeg: [28, 20],
  eyeBoxMm: [120, 80, 160],
  font: 'fixture:fnt/boxes',
  colour: '#35ff6a',
  glowPx: 0.6,
  strokeMrad: 1.2,
  textMrad: 12,
  layers: [
    { type: 'boresight', id: 'boresight', sizeMrad: 10 },
    { type: 'flight_path_marker', id: 'fpm', sizeMrad: 14, cageBelowMps: 20 },
    {
      type: 'pitch_ladder',
      id: 'ladder',
      stepDeg: 5,
      labelStepDeg: 10,
      halfWidthMrad: 40,
      gapMrad: 10,
      declutterAboveDeg: 60,
    },
    {
      type: 'speed_tape',
      id: 'speed',
      atMrad: [-175, 0],
      spanMrad: 120,
      channel: 'cas',
      windowKn: 100,
      tickKn: 10,
      labelKn: 20,
      machFrom: 0.6,
    },
    {
      type: 'altitude_tape',
      id: 'altitude',
      atMrad: [175, 0],
      spanMrad: 120,
      channel: 'altitude_msl',
      windowFt: 1000,
      tickFt: 100,
      labelFt: 200,
      radarBelowFt: 1500,
    },
  ],
};
