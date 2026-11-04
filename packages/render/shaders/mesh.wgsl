#include "common.wgsl"

// The procedural aircraft and its cockpit (ADR-41). One mesh in body axes; each vertex names the
// part it belongs to (0 is the fixed airframe), and the part's matrix, written each frame from the
// snapshot, takes it to eye-relative render axes: the part's hinge rotation, the aircraft's
// attitude and its position relative to the eye all in one. The nozzle's glow is the afterburner
// level in `model.glow.x`.

struct Model {
  parts : array<mat4x4<f32>, 16>,
  glow  : vec4<f32>,
};

@group(0) @binding(2) var<uniform> model : Model;

struct VsOut {
  @builtin(position) clip : vec4<f32>,
  @location(0) normal     : vec3<f32>,
  @location(1) material   : f32,
  @location(2) distance_m : f32,
};

@vertex
fn vs_main(
  @location(0) pos    : vec3<f32>,  // body axes, metres
  @location(1) normal : vec3<f32>,  // body axes
  @location(2) info   : vec2<f32>,  // x: part index, y: material
) -> VsOut {
  let m = model.parts[u32(info.x)];
  let p = m * vec4<f32>(pos, 1.0);
  var out : VsOut;
  out.clip = frame.view_proj * vec4<f32>(p.xyz, 1.0);
  out.normal = (m * vec4<f32>(normal, 0.0)).xyz;
  out.material = info.y;
  out.distance_m = length(p.xyz);
  return out;
}

// Materials, by the number the mesh builder gives them (`MATERIAL` in model-mesh.ts).
fn paint(material : f32) -> vec3<f32> {
  if (material < 0.5) { return vec3<f32>(0.56, 0.59, 0.63); }   // paint
  if (material < 1.5) { return vec3<f32>(0.13, 0.13, 0.15); }   // dark: intakes, the nozzle's sides
  if (material < 2.5) { return vec3<f32>(0.32, 0.46, 0.62); }   // glass
  if (material < 3.5) { return vec3<f32>(0.0, 0.0, 0.0); }      // glow: drawn below
  if (material < 4.5) { return vec3<f32>(0.07, 0.07, 0.08); }   // tyre
  if (material < 5.5) { return vec3<f32>(0.42, 0.43, 0.46); }   // metal
  return vec3<f32>(0.11, 0.12, 0.13);                           // cockpit
}

@fragment
fn fs_main(in : VsOut) -> @location(0) vec4<f32> {
  if (frame.debug.x > 0.5) {
    return vec4<f32>(select(KEY_MODEL, KEY_COCKPIT, in.material > 5.5), 1.0);
  }
  if (in.material > 2.5 && in.material < 3.5) {
    // The nozzle's rear face: cold and dark, hot orange with the afterburner.
    return vec4<f32>(mix(vec3<f32>(0.12, 0.08, 0.07), vec3<f32>(1.0, 0.55, 0.15), model.glow.x), 1.0);
  }
  let shade = lambert(in.normal, frame.sun_dir.xyz);
  let lit = paint(in.material) * (0.35 + 0.65 * shade);
  return vec4<f32>(fog(lit, in.distance_m), 1.0);
}
