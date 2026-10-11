// Shared definitions. Positions arrive camera-relative: the CPU subtracts the camera position in
// f64 and casts to f32 (GDD 44.8, relative-to-eye), so f32 precision is spent near the camera.

struct Frame {
  view_proj : mat4x4<f32>,
  sun_dir   : vec4<f32>,  // xyz: unit vector towards the sun in render space
  params    : vec4<f32>,  // x: time in seconds, yzw: reserved
};

@group(0) @binding(0) var<uniform> frame : Frame;

fn lambert(n : vec3<f32>, l : vec3<f32>) -> f32 {
  return max(dot(normalize(n), normalize(l)), 0.0);
}
