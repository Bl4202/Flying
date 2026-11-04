// Shared definitions. Positions arrive camera-relative: the CPU subtracts the eye position in f64
// and casts to f32 (GDD 44.8, relative-to-eye), so f32 precision is spent near the camera.
//
// Render space is right-handed, Y up: x east, y up, z south (GDD 0.3.2).

struct Frame {
  view_proj : mat4x4<f32>,
  // Camera axes in render space: columns are right, up and back. Takes a view ray to render space.
  inv_view  : mat4x4<f32>,
  sun_dir   : vec4<f32>,  // xyz: unit vector towards the sun in render space
  params    : vec4<f32>,  // x: time in seconds, y: tan(fov_y / 2), z: aspect, w: eye height above MSL in metres
  // The analytic globe under the terrain (ADR-40), from the CPU in f64.
  earth     : vec4<f32>,  // x: eye height above the globe, y: globe radius, z: sin(horizon dip), w: h * (2 R + h)
  eye_up    : vec4<f32>,  // xyz: local up at the eye, render axes
  fx        : vec4<f32>,  // x: g stress S; y: red-out R; z: effect strength 0 to 1; w: reserved
  // x: 1 in the classify view (flat key colours); y: 1 with the seeded flat-earth fault;
  // zw: eye east and north of the anchor in BF metres (only the fault uses them).
  debug     : vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame : Frame;

// Colours of the picture. The haze the terrain fades into is the colour of the ground at the
// horizon; the sky at the horizon is brighter, so the horizon is a visible edge at the geometric dip
// at every height (the screenshot tests find it by that step).
const HAZE_GROUND : vec3<f32> = vec3<f32>(0.60, 0.66, 0.72);
const HAZE_SKY : vec3<f32> = vec3<f32>(0.74, 0.82, 0.92);
const SKY_ZENITH : vec3<f32> = vec3<f32>(0.20, 0.42, 0.80);
const GLOBE_BASE : vec3<f32> = vec3<f32>(0.28, 0.34, 0.24);

// Haze thins exponentially with height: density at MSL, and the height over which it falls by e.
const HAZE_DENSITY : f32 = 0.00005;
const HAZE_SCALE_M : f32 = 7000.0;

// The classify view draws flat key colours with no lighting or haze, so tests can tell what drew a
// pixel. Keep these in step with `CLASSIFY_KEYS` in classify.ts (a test compares them).
const KEY_SKY : vec3<f32> = vec3<f32>(0.0, 0.0, 1.0);
const KEY_GLOBE : vec3<f32> = vec3<f32>(1.0, 0.0, 1.0);
const KEY_MARKER : vec3<f32> = vec3<f32>(1.0, 1.0, 0.0);

fn key_level(level : f32) -> vec3<f32> {
  // L0 to L3 in four greens: 255, 191, 127 and 63.
  return vec3<f32>(0.0, (255.0 - level * 64.0) / 255.0, 0.0);
}

fn lambert(n : vec3<f32>, l : vec3<f32>) -> f32 {
  return max(dot(normalize(n), normalize(l)), 0.0);
}

/// Optical depth along a straight path of `length_m` from height `y0` to `y1` (both MSL, metres)
/// through haze whose density falls off exponentially with height.
fn optical_depth(length_m : f32, y0 : f32, y1 : f32) -> f32 {
  let k = 1.0 / HAZE_SCALE_M;
  let a = clamp(y0, -500.0, 40000.0);
  let b = clamp(y1, -500.0, 40000.0);
  var mean = exp(-a * k);
  if (abs(b - a) * k > 0.001) {
    mean = (exp(-a * k) - exp(-b * k)) / ((b - a) * k);
  }
  return HAZE_DENSITY * length_m * mean;
}

/// `colour` seen through the haze along a path of `length_m` from height `y0` to `y1`.
fn apply_haze(colour : vec3<f32>, length_m : f32, y0 : f32, y1 : f32) -> vec3<f32> {
  let amount = 1.0 - exp(-optical_depth(length_m, y0, y1));
  return mix(colour, HAZE_GROUND, clamp(amount, 0.0, 1.0));
}

/// Haze for things at the eye's own height (the markers).
fn fog(colour : vec3<f32>, distance_m : f32) -> vec3<f32> {
  return apply_haze(colour, distance_m, frame.params.w, frame.params.w);
}
