// Shared definitions. Positions arrive camera-relative: the CPU subtracts the eye position in f64
// and casts to f32 (GDD 44.8, relative-to-eye), so f32 precision is spent near the camera.
//
// Render space is right-handed, Y up: x east, y up, z south (GDD 0.3.2).

struct Frame {
  view_proj    : mat4x4<f32>,
  sun_dir      : vec4<f32>,  // xyz: unit vector towards the sun in render space
  params       : vec4<f32>,  // x: time in seconds, y: terrain post spacing in metres, z: patch span in metres, w: reserved
  patch_offset : vec4<f32>,  // xyz: terrain patch origin minus eye, render axes, metres
};

@group(0) @binding(0) var<uniform> frame : Frame;

// The sky is the clear colour; fog fades distant surfaces into it.
const SKY : vec3<f32> = vec3<f32>(0.45, 0.62, 0.85);
const FOG_PER_METRE : f32 = 0.000025;

fn lambert(n : vec3<f32>, l : vec3<f32>) -> f32 {
  return max(dot(normalize(n), normalize(l)), 0.0);
}

fn fog(colour : vec3<f32>, distance_m : f32) -> vec3<f32> {
  let amount = 1.0 - exp(-distance_m * FOG_PER_METRE);
  return mix(colour, SKY, clamp(amount, 0.0, 1.0));
}
