#include "common.wgsl"

// Posts per patch edge (GDD ADR-06: 129 x 129 height posts, 128 intervals).
#define PATCH_INTERVALS 128.0
#define SNOW_LINE_M 3000.0

struct VsOut {
  @builtin(position) clip : vec4<f32>,
  @location(0) normal     : vec3<f32>,
  @location(1) height_m   : f32,
  @location(2) cell       : vec2<f32>,
};

@vertex
fn vs_main(
  @location(0) pos_rel : vec3<f32>,  // camera-relative position, metres (x east, y up, z south)
  @location(1) normal  : vec3<f32>,
  @location(2) height  : f32,        // height above MSL, metres
) -> VsOut {
  var out : VsOut;
  out.clip = frame.view_proj * vec4<f32>(pos_rel, 1.0);
  out.normal = normal;
  out.height_m = height;
  out.cell = pos_rel.xz / PATCH_INTERVALS;
  return out;
}

@fragment
fn fs_main(in : VsOut) -> @location(0) vec4<f32> {
  let shade = lambert(in.normal, frame.sun_dir.xyz);
  let lowland = vec3<f32>(0.20, 0.35, 0.15);
  let highland = vec3<f32>(0.55, 0.50, 0.45);
  let base = mix(lowland, highland, clamp(in.height_m / SNOW_LINE_M, 0.0, 1.0));
  // Faint cell grid so the patch structure is visible before real terrain exists.
  let grid = 1.0 - 0.06 * step(0.98, max(fract(in.cell.x), fract(in.cell.y)));
  return vec4<f32>(base * (0.25 + 0.75 * shade) * grid, 1.0);
}
