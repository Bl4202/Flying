/** Helpers for the HUD tests. Not production code. */
import { readFileSync } from 'node:fs';
import { newAircraftStatus, type AircraftStatus } from '@flying/sim-bridge';
import { parsePresentBundle } from '@flying/sim-bridge/present';
import { quatFromColumns } from '../snapshot-fixture.ts';
import { renderPresentation, type RenderPresentation } from '../present.ts';
import { HudGraph, SEGMENT_FLOATS } from './graph.ts';
import { drawLayer, type HudContext } from './elements.ts';
import { HudState, type HudInput } from './state.ts';
import { DEFAULT_HUD_TEXTS } from './texts.ts';

export const DEG = Math.PI / 180;

let cached: RenderPresentation | undefined;
/** The shipped Kestrel presentation, from the test vector. */
export function kestrel(): RenderPresentation {
  if (cached) return cached;
  const bundle = parsePresentBundle(
    JSON.parse(
      readFileSync(
        new URL('../../../../tests/vectors/present_bundle.json', import.meta.url),
        'utf8',
      ),
    ),
  );
  const p = renderPresentation(bundle, 'core:ac/kestrel');
  if (!p) throw new Error('no Kestrel');
  cached = p;
  return p;
}

/**
 * Body-to-BF attitude for a heading (clockwise from north), pitch and bank (right wing down), all
 * in radians, on a spherical Earth at the BF origin (local up is BF up there).
 */
export function attitude(
  heading: number,
  pitch: number,
  bank: number,
): [number, number, number, number] {
  const f: [number, number, number] = [
    Math.cos(pitch) * Math.sin(heading),
    Math.cos(pitch) * Math.cos(heading),
    Math.sin(pitch),
  ];
  const r0: [number, number, number] = [Math.cos(heading), -Math.sin(heading), 0];
  const d0: [number, number, number] = [
    f[1] * r0[2] - f[2] * r0[1],
    f[2] * r0[0] - f[0] * r0[2],
    f[0] * r0[1] - f[1] * r0[0],
  ];
  const [c, s] = [Math.cos(bank), Math.sin(bank)];
  const r: [number, number, number] = [
    r0[0] * c + d0[0] * s,
    r0[1] * c + d0[1] * s,
    r0[2] * c + d0[2] * s,
  ];
  const d: [number, number, number] = [
    d0[0] * c - r0[0] * s,
    d0[1] * c - r0[1] * s,
    d0[2] * c - r0[2] * s,
  ];
  return quatFromColumns(f, r, d);
}

export function status(over: Partial<AircraftStatus> = {}): AircraftStatus {
  return { ...newAircraftStatus(), assist: 2, nMax: 7.5, ...over };
}

export interface Scenario {
  heading?: number;
  pitch?: number;
  bank?: number;
  pos?: [number, number, number];
  vel?: [number, number, number];
  status?: Partial<AircraftStatus>;
  warnings?: number;
  timeS?: number;
}

/** A HUD input for a scenario: attitude from the angles, velocity along the nose unless given. */
export function input(s: Scenario = {}): HudInput {
  const q = attitude(s.heading ?? 0, s.pitch ?? 0, s.bank ?? 0);
  const speed = 130;
  const f: [number, number, number] = [
    Math.cos(s.pitch ?? 0) * Math.sin(s.heading ?? 0),
    Math.cos(s.pitch ?? 0) * Math.cos(s.heading ?? 0),
    Math.sin(s.pitch ?? 0),
  ];
  return {
    posBf: s.pos ?? [0, 0, 500],
    quatBf: q,
    velBf: s.vel ?? [f[0] * speed, f[1] * speed, f[2] * speed],
    status: status(s.status),
    warnings: s.warnings ?? 0,
    timeS: s.timeS ?? 0,
  };
}

export type Segment = [number, number, number, number, number, number, number];

export function segmentsOf(g: HudGraph): Segment[] {
  const out: Segment[] = [];
  for (let i = 0; i < g.count; i++) {
    const at = i * SEGMENT_FLOATS;
    out.push([0, 1, 2, 3, 4, 5, 6].map((k) => g.segments[at + k] ?? 0) as Segment);
  }
  return out;
}

/** A context for drawing one layer of the Kestrel HUD from a scenario. */
export function draw(
  type: string,
  s: Scenario = {},
  declutter: 0 | 1 | 2 = 0,
): { g: HudGraph; segments: Segment[]; state: Float64Array } {
  const p = kestrel();
  const g = new HudGraph(p.font);
  const state = new HudState();
  state.update(input(s));
  const ctx: HudContext = {
    g,
    s: state.v,
    spec: p.hud,
    texts: DEFAULT_HUD_TEXTS,
    declutter,
    halfU: Math.tan(((p.hud.tfovDeg[0] / 2) * Math.PI) / 180) * 1000,
    halfV: Math.tan(((p.hud.tfovDeg[1] / 2) * Math.PI) / 180) * 1000,
  };
  const layer = p.hud.layers.find((l) => l.type === type);
  if (!layer) throw new Error(`no ${type} layer`);
  drawLayer(ctx, layer);
  return { g, segments: segmentsOf(g), state: state.v };
}

/** The bounding box of some segments. */
export function bounds(segs: readonly Segment[]): {
  x0: number;
  x1: number;
  y0: number;
  y1: number;
} {
  const xs = segs.flatMap((s) => [s[0], s[2]]);
  const ys = segs.flatMap((s) => [s[1], s[3]]);
  return { x0: Math.min(...xs), x1: Math.max(...xs), y0: Math.min(...ys), y1: Math.max(...ys) };
}
