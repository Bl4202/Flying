#include "common.wgsl"

// Instanced darts: one draw for every moving body, and for the cues (the nose and the mouse-aim
// point). The mesh is in body axes (x forward, y right, z down); each instance gives its
// eye-relative position, a scale, its orientation as a quaternion (x, y, z, w) taking body axes to
// render axes, and a tint. A tint with w = 0 means "a colour of its own by index" (bodies); with
// w = 1 it is the flat colour to draw (a cue: unlit, not fogged, so it reads the same at any range).

struct VsOut {
  @builtin(position) clip : vec4<f32>,
  @location(0) normal     : vec3<f32>,
  @location(1) tint       : vec3<f32>,
  @location(2) distance_m : f32,
  @location(3) cue        : f32,
};

fn qrot(q : vec4<f32>, v : vec3<f32>) -> vec3<f32> {
  let t = 2.0 * cross(q.xyz, v);
  return v + q.w * t + cross(q.xyz, t);
}

@vertex
fn vs_main(
  @location(0) pos       : vec3<f32>,  // body axes, unit size
  @location(1) normal    : vec3<f32>,  // body axes
  @location(2) pos_scale : vec4<f32>,  // xyz: eye-relative position in render axes, w: size in metres
  @location(3) quat      : vec4<f32>,
  @location(4) tint      : vec4<f32>,  // rgb: flat colour; w: 1 to use it, 0 to colour by index
  @builtin(instance_index) index : u32,
) -> VsOut {
  let pos_rel = pos_scale.xyz + qrot(quat, pos * pos_scale.w);
  var out : VsOut;
  out.clip = frame.view_proj * vec4<f32>(pos_rel, 1.0);
  out.normal = qrot(quat, normal);
  // A different, bright colour per body, spread around the colour wheel.
  let hue = f32(index) * 2.399963 + vec3<f32>(0.0, 2.094395, 4.188790);
  out.tint = select(0.55 + 0.45 * sin(hue), tint.rgb, tint.w > 0.5);
  out.distance_m = length(pos_scale.xyz);
  out.cue = tint.w;
  return out;
}

@fragment
fn fs_main(in : VsOut) -> @location(0) vec4<f32> {
  let shade = lambert(in.normal, frame.sun_dir.xyz);
  let lit = in.tint * (0.35 + 0.65 * shade);
  let body = fog(lit, in.distance_m);
  return vec4<f32>(select(body, in.tint, in.cue > 0.5), 1.0);
}
