/**
 * The mesh every moving body is drawn with: a four-sided dart, nose along body +x, in body axes
 * (forward, right, down), one metre long. Flat-shaded, so each face has its own three vertices.
 * Instances scale it (`MARKER_SIZE_M`) and rotate it into place on the GPU.
 */

/** Length of a drawn marker, metres. */
export const MARKER_SIZE_M = 300;
/** Floats per vertex: position then normal. */
export const MARKER_VERTEX_FLOATS = 6;
/** Floats per instance: eye-relative position and size, then orientation as (x, y, z, w). */
export const MARKER_INSTANCE_FLOATS = 8;

type V3 = [number, number, number];

const NOSE: V3 = [0.5, 0, 0];
// Tail corners in order right, down, left, up (viewed from behind, that is a clockwise ring).
const TAIL: V3[] = [
  [-0.5, 0.3, 0],
  [-0.5, 0, 0.12],
  [-0.5, -0.3, 0],
  [-0.5, 0, -0.18],
];

function sub(a: V3, b: V3): V3 {
  return [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
}
function cross(a: V3, b: V3): V3 {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}
function unit(a: V3): V3 {
  const n = Math.hypot(...a);
  return [a[0] / n, a[1] / n, a[2] / n];
}

/** The six triangles (counter-clockwise seen from outside): four sides and a two-triangle tail. */
export const MARKER_TRIANGLES: readonly (readonly [V3, V3, V3])[] = [
  [NOSE, TAIL[0] as V3, TAIL[1] as V3],
  [NOSE, TAIL[1] as V3, TAIL[2] as V3],
  [NOSE, TAIL[2] as V3, TAIL[3] as V3],
  [NOSE, TAIL[3] as V3, TAIL[0] as V3],
  [TAIL[0] as V3, TAIL[2] as V3, TAIL[1] as V3],
  [TAIL[0] as V3, TAIL[3] as V3, TAIL[2] as V3],
];

/** Interleaved position and flat normal for all triangles. */
export function buildMarkerMesh(): Float32Array {
  const out = new Float32Array(MARKER_TRIANGLES.length * 3 * MARKER_VERTEX_FLOATS);
  let at = 0;
  for (const [a, b, c] of MARKER_TRIANGLES) {
    const n = unit(cross(sub(b, a), sub(c, a)));
    for (const p of [a, b, c]) {
      out.set(p, at);
      out.set(n, at + 3);
      at += MARKER_VERTEX_FLOATS;
    }
  }
  return out;
}

export const MARKER_VERTEX_COUNT = MARKER_TRIANGLES.length * 3;
