#include "common.wgsl"

// Height above MSL at which the lowland colour has fully become the highland colour.
#define HIGHLAND_M 1200.0

// Extra instances lay further copies of the patch out on a grid, east then south, one patch apart.
// The first instance is the patch itself; more of them are how the benchmark loads the GPU.
const PATCH_GRID_COLUMNS : u32 = 8u;

struct VsOut {
  @builtin(position) clip : vec4<f32>,
  @location(0) normal     : vec3<f32>,
  @location(1) height_m   : f32,
  @location(2) cell       : vec2<f32>,
  @location(3) distance_m : f32,
  @location(4) local_xz   : vec2<f32>,  // patch-local east and south, metres
  @location(5) runway_on  : f32,        // 1 on the patch itself, 0 on the benchmark's extra copies
};

@vertex
fn vs_main(
  @location(0) pos_local : vec3<f32>,  // patch-local position, render axes, metres
  @location(1) normal    : vec3<f32>,  // unit normal, render axes
  @location(2) height    : f32,        // height above MSL, metres
  @builtin(instance_index) instance : u32,
) -> VsOut {
  // The patch is positioned per draw: its origin relative to the eye was subtracted in f64.
  let step = vec3<f32>(f32(instance % PATCH_GRID_COLUMNS), 0.0, f32(instance / PATCH_GRID_COLUMNS));
  let pos_rel = pos_local + frame.patch_offset.xyz + step * frame.params.z;
  var out : VsOut;
  out.clip = frame.view_proj * vec4<f32>(pos_rel, 1.0);
  out.normal = normal;
  out.height_m = height;
  out.cell = pos_local.xz / frame.params.y;
  out.distance_m = length(pos_rel);
  out.local_xz = pos_local.xz;
  out.runway_on = select(0.0, 1.0, instance == 0u);
  return out;
}

// The runway is painted on the terrain from its position in the frame uniform, so it needs no
// geometry and cannot fight the ground in depth. Markings are a few tenths of a metre wide, far
// narrower than a pixel at any distance, so every line is drawn with a coverage that falls off
// with the width of a pixel on the ground (`aa`, from `fwidth`), and fades into the asphalt as it
// shrinks below a pixel.
const ASPHALT : vec3<f32> = vec3<f32>(0.20, 0.20, 0.22);
const PAINT : vec3<f32> = vec3<f32>(0.90, 0.90, 0.88);
const LINE_WIDTH_M : f32 = 0.9;
const DASH_ON_M : f32 = 30.0;
const DASH_PERIOD_M : f32 = 50.0;
const DASH_CLEAR_M : f32 = 12.0;
const EDGE_INSET_M : f32 = 1.5;
const BARS_FROM_M : f32 = 6.0;
const BARS_TO_M : f32 = 36.0;
const BAR_WIDTH_M : f32 = 1.8;
const BAR_PITCH_M : f32 = 3.6;
const BAR_CLEAR_M : f32 = 3.0;

/// How much of a pixel a stripe covers: 1 inside, 0 outside, soft over one pixel width `aa`.
fn stripe(distance_from_centre : f32, half_width : f32, aa : f32) -> f32 {
  return 1.0 - smoothstep(half_width - aa, half_width + aa, abs(distance_from_centre));
}

/// Fades a feature narrower than two pixels into its surroundings instead of letting it shimmer.
fn visible(width_m : f32, aa : f32) -> f32 {
  return clamp(width_m / max(aa * 2.0, 0.0001), 0.0, 1.0);
}

/// The runway at a patch-local point: rgb, and alpha 1 on the runway, 0 off it. `aa_x` and `aa_z`
/// are the size of a pixel on the ground along each axis.
fn runway_colour(local_xz : vec2<f32>, aa_x : f32, aa_z : f32) -> vec4<f32> {
  let length_m = frame.runway.z;
  let half_width = frame.runway.w * 0.5;
  // Along runs from the threshold towards the far end (north, which is -z in render axes).
  let along = frame.runway.y - local_xz.y;
  let across = local_xz.x - frame.runway.x;
  let aa_across = max(aa_x, 0.001);
  let aa_along = max(aa_z, 0.001);
  let abs_across = abs(across);

  let started = smoothstep(-aa_along, aa_along, along);
  let not_ended = 1.0 - smoothstep(length_m - aa_along, length_m + aa_along, along);
  let on = stripe(across, half_width, aa_across) * started * not_ended;

  // Edge lines, just inside the pavement edge, the whole length.
  let edge = stripe(abs_across - (half_width - EDGE_INSET_M), LINE_WIDTH_M * 0.5, aa_across) *
    visible(LINE_WIDTH_M, aa_across);

  // Centreline: 30 m dashes every 50 m, clear of both ends.
  let on_fraction = DASH_ON_M / DASH_PERIOD_M;
  let soft = aa_along / DASH_PERIOD_M;
  let phase = fract((along - DASH_CLEAR_M) / DASH_PERIOD_M);
  let dash = 1.0 - smoothstep(on_fraction - soft, on_fraction + soft, phase);
  let in_dashes = smoothstep(0.0, aa_along, along - DASH_CLEAR_M) *
    (1.0 - smoothstep(0.0, aa_along, along - (length_m - DASH_CLEAR_M)));
  let centre = stripe(across, LINE_WIDTH_M * 0.5, aa_across) * dash * in_dashes *
    visible(LINE_WIDTH_M, aa_across);

  // Threshold bars either side of the centre, near the threshold.
  let bar_cell = fract((abs_across - BAR_CLEAR_M) / BAR_PITCH_M) - 0.5;
  let bar_inside = smoothstep(-aa_across, aa_across, abs_across - BAR_CLEAR_M) *
    (1.0 - smoothstep(-aa_across, aa_across, abs_across - (half_width - EDGE_INSET_M - 1.0)));
  let bar = stripe(bar_cell * BAR_PITCH_M, BAR_WIDTH_M * 0.5, aa_across) *
    visible(BAR_WIDTH_M, aa_across) * bar_inside *
    smoothstep(BARS_FROM_M - aa_along, BARS_FROM_M + aa_along, along) *
    (1.0 - smoothstep(BARS_TO_M - aa_along, BARS_TO_M + aa_along, along));

  let paint = clamp(edge + centre + bar, 0.0, 1.0);
  return vec4<f32>(mix(ASPHALT, PAINT, paint), on);
}

@fragment
fn fs_main(in : VsOut) -> @location(0) vec4<f32> {
  // Derivatives have to be taken where every fragment of a quad takes them.
  let aa_x = fwidth(in.local_xz.x);
  let aa_z = fwidth(in.local_xz.y);

  let shade = lambert(in.normal, frame.sun_dir.xyz);
  let lowland = vec3<f32>(0.20, 0.35, 0.15);
  let highland = vec3<f32>(0.55, 0.50, 0.45);
  var base = mix(lowland, highland, clamp(in.height_m / HIGHLAND_M, 0.0, 1.0));
  // Faint cell grid so the patch structure is visible before real terrain exists.
  let edge = max(fract(in.cell.x), fract(in.cell.y));
  var grid = 1.0 - 0.07 * step(0.96, edge);
  if (frame.runway.z > 0.0 && in.runway_on > 0.5) {
    let r = runway_colour(in.local_xz, aa_x, aa_z);
    base = mix(base, r.rgb, r.a);
    grid = mix(grid, 1.0, r.a);
  }
  let lit = base * (0.25 + 0.75 * shade) * grid;
  return vec4<f32>(fog(lit, in.distance_m), 1.0);
}
