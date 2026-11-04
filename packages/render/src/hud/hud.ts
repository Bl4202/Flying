/**
 * Builds one frame of HUD (ADR-41): the drawing list from the aircraft's state and the layer list
 * in the HUD data, and the uniform block the shader needs. The HUD is drawn in the main pass as
 * directions at infinity: a segment's endpoints are body directions `(1, u, -v)`, projected with
 * `w = 0`, so moving the eye moves nothing (collimation, GDD 54.3). In the cockpit view each
 * fragment is also tested against the combiner glass: the ray from the eye along its direction
 * must pass through the glass rectangle, which cuts the symbols at the combiner's edge and, as the
 * eye leaves the eye box, fades them. In the chase view (or when asked) the plane is drawn straight
 * on the screen as an overlay.
 *
 * Uniform block `Hud` (binding 3), 48 floats = 192 bytes after the 16-float matrix:
 *
 * | floats | field | contents |
 * |--------|-------|----------|
 * | 0-15   | `rot` | body to render rotation (a mat4, no translation) |
 * | 16-19  | `colour` | rgb of the palette, brightness |
 * | 20-23  | `viewport` | width px, height px, stroke width px, glow px |
 * | 24-27  | `mode` | overlay (0 or 1), overlay NDC per tangent unit, aspect, eye-box fade |
 * | 28-31  | `eye` | the eye in body axes, metres |
 * | 32-35  | `glass0` | glass corner 0 (body), width m |
 * | 36-39  | `glass_a` | unit vector along the glass's first edge |
 * | 40-43  | `glass_b` | unit vector along its second edge, height m |
 * | 44-47  | `glass_n` | the glass's normal |
 */
import type { CockpitSpec, FontSpec, HudSpec } from '../present.ts';
import type { FrameData } from '../scene.ts';
import { drawLayer, type Declutter, type HudContext } from './elements.ts';
import { HudGraph } from './graph.ts';
import { HudState, type HudInput } from './state.ts';
import { DEFAULT_HUD_TEXTS, type HudTexts } from './texts.ts';

export const HUD_UNIFORM_FLOATS = 48;
export const HUD_UNIFORM_BYTES = HUD_UNIFORM_FLOATS * 4;

export type HudPalette = 'green' | 'amber' | 'white' | 'cyan' | 'high-contrast';
export const HUD_PALETTES: readonly HudPalette[] = [
  'green',
  'amber',
  'white',
  'cyan',
  'high-contrast',
];

/** The colour of each palette other than the data's own green. */
const PALETTE_RGB: Readonly<
  Record<Exclude<HudPalette, 'green'>, readonly [number, number, number]>
> = {
  amber: [1, 0.7, 0.1],
  white: [1, 1, 1],
  cyan: [0.2, 0.9, 1],
  'high-contrast': [1, 1, 1],
};

/** What the player and the camera say about this frame's HUD. */
export interface HudOptions {
  declutter: Declutter;
  /** 0 to 1. */
  brightness: number;
  palette: HudPalette;
  /** Draw the plane straight on the screen (the chase view), not on the glass. */
  overlay: boolean;
  /** Overlay size, 0.5 to 2. */
  overlayScale: number;
  widthPx: number;
  heightPx: number;
  /** Vertical field of view, radians. */
  fovY: number;
  /** The eye, BF metres (f64): the camera's. */
  eyeBf: ArrayLike<number>;
}

export class Hud {
  readonly graph: HudGraph;
  readonly state = new HudState();
  readonly uniform = new Float32Array(HUD_UNIFORM_FLOATS);
  readonly spec: HudSpec;
  readonly #cockpit: CockpitSpec;
  readonly #ctx: HudContext;
  /** The eye in body axes, for the frame being built. */
  readonly #eye = new Float64Array(3);
  /** The glass's frame, worked out once. */
  readonly #glass = {
    corner: [0, 0, 0] as [number, number, number],
    a: [0, 0, 0] as [number, number, number],
    b: [0, 0, 0] as [number, number, number],
    n: [0, 0, 1] as [number, number, number],
    width: 0,
    height: 0,
  };

  constructor(
    spec: HudSpec,
    cockpit: CockpitSpec,
    font: FontSpec,
    texts: HudTexts = DEFAULT_HUD_TEXTS,
  ) {
    this.spec = spec;
    this.#cockpit = cockpit;
    this.graph = new HudGraph(font);
    this.#ctx = {
      g: this.graph,
      s: this.state.v,
      spec,
      texts,
      declutter: 0,
      halfU: Math.tan(((spec.tfovDeg[0] / 2) * Math.PI) / 180) * 1000,
      halfV: Math.tan(((spec.tfovDeg[1] / 2) * Math.PI) / 180) * 1000,
    };
    const [c0, c1, , c3] = cockpit.hudGlassM;
    const a: [number, number, number] = [c1[0] - c0[0], c1[1] - c0[1], c1[2] - c0[2]];
    const b: [number, number, number] = [c3[0] - c0[0], c3[1] - c0[1], c3[2] - c0[2]];
    const la = Math.hypot(a[0], a[1], a[2]);
    const lb = Math.hypot(b[0], b[1], b[2]);
    const au: [number, number, number] = [a[0] / la, a[1] / la, a[2] / la];
    const bu: [number, number, number] = [b[0] / lb, b[1] / lb, b[2] / lb];
    const n: [number, number, number] = [
      au[1] * bu[2] - au[2] * bu[1],
      au[2] * bu[0] - au[0] * bu[2],
      au[0] * bu[1] - au[1] * bu[0],
    ];
    this.#glass.corner = [c0[0], c0[1], c0[2]];
    this.#glass.a = au;
    this.#glass.b = bu;
    this.#glass.n = n;
    this.#glass.width = la;
    this.#glass.height = lb;
  }

  /** Replaces the words the HUD writes (from the app's strings). */
  setTexts(texts: HudTexts): void {
    (this.#ctx as { texts: HudTexts }).texts = texts;
  }

  /**
   * Fills the drawing list and the uniform block for this frame. `rot` is the body-to-render
   * rotation of the aircraft's attitude (the camera's view matrix is applied by the shader).
   */
  build(input: HudInput, rot: Float32Array, o: HudOptions): void {
    this.state.update(input);
    const g = this.graph;
    g.reset();
    const c = this.#ctx;
    c.declutter = o.declutter;
    for (let i = 0; i < this.spec.layers.length; i++) {
      const layer = this.spec.layers[i];
      if (layer) drawLayer(c, layer);
    }
    this.#uniform(rot, o, input);
  }

  /** Builds the HUD and hands it to `frame`: the segments, their count and the uniform block. */
  fillFrame(frame: FrameData, input: HudInput, rot: Float32Array, o: HudOptions): void {
    this.build(input, rot, o);
    frame.hud = this.graph.count > 0;
    frame.hudCount = this.graph.count;
    frame.hudSegments = this.graph.segments;
    frame.hudUniform = this.uniform;
  }

  /** The eye in body axes: the BF offset from the aircraft, rotated by the transpose of its attitude. */
  #eyeInBody(o: HudOptions, input: HudInput): void {
    const e = this.#eye;
    const dx = (o.eyeBf[0] ?? 0) - (input.posBf[0] ?? 0);
    const dy = (o.eyeBf[1] ?? 0) - (input.posBf[1] ?? 0);
    const dz = (o.eyeBf[2] ?? 0) - (input.posBf[2] ?? 0);
    const q = input.quatBf;
    const w = q[0] ?? 1;
    const x = q[1] ?? 0;
    const y = q[2] ?? 0;
    const z = q[3] ?? 0;
    // R transposed: component i is the sum of R[j][i] times the BF vector's j.
    e[0] = (1 - 2 * (y * y + z * z)) * dx + 2 * (x * y + w * z) * dy + 2 * (x * z - w * y) * dz;
    e[1] = 2 * (x * y - w * z) * dx + (1 - 2 * (x * x + z * z)) * dy + 2 * (y * z + w * x) * dz;
    e[2] = 2 * (x * z + w * y) * dx + 2 * (y * z - w * x) * dy + (1 - 2 * (x * x + y * y)) * dz;
  }

  #uniform(rot: Float32Array, o: HudOptions, input: HudInput): void {
    this.#eyeInBody(o, input);
    const eyeBody = this.#eye;
    const u = this.uniform;
    u.set(rot.subarray(0, 16), 0);
    const rgb =
      o.palette === 'green'
        ? hexToRgb(this.spec.colour)
        : (PALETTE_RGB[o.palette] as readonly number[]);
    u[16] = rgb[0] as number;
    u[17] = rgb[1] as number;
    u[18] = rgb[2] as number;
    u[19] = Math.min(Math.max(o.brightness, 0), 1);
    const pxPerTan = o.overlay
      ? (this.#overlayScale(o) * o.heightPx) / 2
      : o.heightPx / 2 / Math.tan(o.fovY / 2);
    const strokePx =
      Math.max(1.5, (this.spec.strokeMrad / 1000) * pxPerTan) *
      (o.palette === 'high-contrast' ? 1.6 : 1);
    u[20] = o.widthPx;
    u[21] = o.heightPx;
    u[22] = strokePx;
    u[23] = this.spec.glowPx;
    u[24] = o.overlay ? 1 : 0;
    u[25] = this.#overlayScale(o);
    u[26] = o.widthPx / o.heightPx;
    // The eye box: symbols fade as the eye leaves it.
    const eye = this.#cockpit.eyePointM;
    const box = this.spec.eyeBoxMm;
    const dy = Math.abs((eyeBody[1] as number) - eye[1]) / (box[0] / 2000);
    const dz = Math.abs((eyeBody[2] as number) - eye[2]) / (box[1] / 2000);
    const dx = Math.abs((eyeBody[0] as number) - eye[0]) / (box[2] / 2000);
    const out = Math.max(dx, dy, dz);
    const t = Math.min(Math.max((out - 0.8) / 0.4, 0), 1);
    u[27] = 1 - t * t * (3 - 2 * t);
    u[28] = eyeBody[0] as number;
    u[29] = eyeBody[1] as number;
    u[30] = eyeBody[2] as number;
    u[31] = 0;
    const gl = this.#glass;
    for (let k = 0; k < 3; k++) {
      u[32 + k] = gl.corner[k] as number;
      u[36 + k] = gl.a[k] as number;
      u[40 + k] = gl.b[k] as number;
      u[44 + k] = gl.n[k] as number;
    }
    u[35] = gl.width;
    u[39] = 0;
    u[43] = gl.height;
    u[47] = 0;
  }

  /** NDC per tangent unit in the overlay: the HUD's field fills 45 per cent of the half-height at 100 %. */
  #overlayScale(o: HudOptions): number {
    return (0.45 * o.overlayScale) / (this.#ctx.halfV / 1000);
  }
}

function hexToRgb(hex: string): [number, number, number] {
  const n = Number.parseInt(hex.slice(1), 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}
