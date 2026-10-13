#include "common.wgsl"

// Height above MSL at which the lowland colour has fully become the highland colour.
#define HIGHLAND_M 1200.0

struct VsOut {
  @builtin(position) clip : vec4<f32>,
  @location(0) normal     : vec3<f32>,
  @location(1) height_m   : f32,
  @location(2) cell       : vec2<f32>,
  @location(3) distance_m : f32,
};

@vertex
fn vs_main(
  @location(0) pos_local : vec3<f32>,  // patch-local position, render axes, metres
  @location(1) normal    : vec3<f32>,  // unit normal, render axes
  @location(2) height    : f32,        // height above MSL, metres
) -> VsOut {
  // The patch is positioned per draw: its origin relative to the eye was subtracted in f64.
  let pos_rel = pos_local + frame.patch_offset.xyz;
  var out : VsOut;
  out.clip = frame.view_proj * vec4<f32>(pos_rel, 1.0);
  out.normal = normal;
  out.height_m = height;
  out.cell = pos_local.xz / frame.params.y;
  out.distance_m = length(pos_rel);
  return out;
}

@fragment
fn fs_main(in : VsOut) -> @location(0) vec4<f32> {
  let shade = lambert(in.normal, frame.sun_dir.xyz);
  let lowland = vec3<f32>(0.20, 0.35, 0.15);
  let highland = vec3<f32>(0.55, 0.50, 0.45);
  let base = mix(lowland, highland, clamp(in.height_m / HIGHLAND_M, 0.0, 1.0));
  // Faint cell grid so the patch structure is visible before real terrain exists.
  let edge = max(fract(in.cell.x), fract(in.cell.y));
  let grid = 1.0 - 0.07 * step(0.96, edge);
  let lit = base * (0.25 + 0.75 * shade) * grid;
  return vec4<f32>(fog(lit, in.distance_m), 1.0);
}
