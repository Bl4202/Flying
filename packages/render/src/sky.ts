/**
 * Where the eye is relative to the Earth, as the sky pass needs it (ADR-40). The sky pass draws an
 * analytic globe under the terrain levels, so the horizon sits at the right dip at any height
 * however far the terrain reaches (GDD 7.4.2).
 *
 * BF is a tangent plane at the anchor on a sphere of radius R, so the Earth's centre is at BF
 * `(anchor east, anchor north, -R)`. The globe the sky pass draws is a sphere of radius
 * `R + floor`, where `floor` is a proven lower bound of the arena's terrain height, so the globe
 * never rises through the terrain (any pixel that shows it is a hole). From height `h` above that
 * globe the horizon dips by `acos(Rg / (Rg + h))` below the eye line.
 *
 * Everything here is `f64`: the quantities are differences of numbers near 6.4e6 m, and the sky
 * shader gets only the stable results (`h`, `sin dip`, and `h * (2 Rg + h)`).
 */
import { WORLD } from '@flying/sim-bridge/layout';

/** The Earth's radius the sim's frame uses, metres. */
export const EARTH_RADIUS_M = WORLD.EARTH_RADIUS_M;

/** The lowest terrain of each arena above the sphere, metres: the globe sits this far up. */
export function globeFloorM(arena: number): number {
  return arena === 1 ? WORLD.PLAIN_FLOOR_M : WORLD.RUGGED_FLOOR_M;
}

export interface EyeGeometry {
  /** Height of the eye above the globe, metres. */
  heightM: number;
  /** Radius of the globe, metres. */
  globeRadiusM: number;
  /** Height of the eye above the sphere of radius R (MSL), metres: what the haze uses. */
  eyeMslM: number;
  /** Sine of the horizon dip: the horizon is at elevation `-asin(sinDip)`. */
  sinDip: number;
  /** `h * (2 Rg + h)`: the square of the distance from the eye to the horizon. */
  chordTerm: number;
  /** Local up at the eye in render axes (east, up, south). */
  up: [number, number, number];
}

export function newEyeGeometry(): EyeGeometry {
  return {
    heightM: 0,
    globeRadiusM: EARTH_RADIUS_M,
    eyeMslM: 0,
    sinDip: 0,
    chordTerm: 0,
    up: [0, 1, 0],
  };
}

/**
 * Fills `out` for an eye at BF `eyeBf`, with the sphere's tangent point at `anchorBf` (east,
 * north) and the globe `floorM` above the sphere. `flat` is the seeded flat-Earth fault: the
 * radius is taken to be infinite, so the horizon is at the eye line and up is the anchor's up.
 */
export function eyeGeometry(
  out: EyeGeometry,
  eyeBf: ArrayLike<number>,
  anchorBf: readonly [number, number],
  floorM: number,
  flat = false,
): EyeGeometry {
  const radius = EARTH_RADIUS_M;
  const globe = radius + floorM;
  const px = (eyeBf[0] ?? 0) - anchorBf[0];
  const py = (eyeBf[1] ?? 0) - anchorBf[1];
  const ez = eyeBf[2] ?? 0;
  const pz = ez + radius;
  const dist = Math.hypot(px, py, pz);
  const horizontal = px * px + py * py + ez * ez + 2 * ez * radius;
  // |p|^2 - Rg^2, written without the large terms so it keeps its precision near the ground.
  const chord = horizontal - 2 * radius * floorM - floorM * floorM;
  out.globeRadiusM = globe;
  out.heightM = chord / (dist + globe);
  out.eyeMslM = horizontal / (dist + radius);
  // `up` is written in place: this runs every frame and allocates nothing.
  if (flat) {
    out.sinDip = 0;
    out.chordTerm = 0;
    out.up[0] = 0;
    out.up[1] = 1;
    out.up[2] = 0;
    return out;
  }
  out.chordTerm = Math.max(chord, 0);
  out.sinDip = chord > 0 ? Math.sqrt(chord) / dist : 0;
  out.up[0] = px / dist;
  out.up[1] = pz / dist;
  out.up[2] = -py / dist + 0; // + 0: never -0
  return out;
}

/** The horizon dip in radians for an eye `heightM` above a globe of radius `R + floorM`. */
export function horizonDip(heightM: number, floorM = 0): number {
  const globe = EARTH_RADIUS_M + floorM;
  return Math.acos(globe / (globe + heightM));
}

/** Distance from the eye to the geometric horizon, metres. */
export function horizonDistance(heightM: number, floorM = 0): number {
  const globe = EARTH_RADIUS_M + floorM;
  return Math.sqrt(2 * globe * heightM + heightM * heightM);
}
