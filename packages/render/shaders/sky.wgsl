#include "common.wgsl"

// The sky pass, drawn first as one triangle over the whole screen with no depth write (ADR-40).
//
// Per pixel it builds the view ray in render space and tests it against the analytic globe, a
// sphere of radius R + floor under the whole terrain. A ray that points below the geometric
// horizon (elevation under -dip) meets the globe: that is the ground beyond the last terrain
// level, drawn in the ground haze. Any other ray is sky: a gradient from the horizon colour up to
// the zenith blue, with the sun. The terrain levels are drawn over it and, where they reach the
// horizon, hide it; where they do not, the globe has the same dip.
//
// `frame.earth` comes from the CPU in f64 (`sky.ts`) in the form that keeps its precision:
// h, sin(dip), and h * (2 R + h). The distance to the globe is written the stable way,
// c / (b + sqrt(b^2 - c)), not as a difference of two large numbers.

const SUN_DISC_COS : f32 = 0.99999;     // about 0.26 degrees
const SUN_GLOW_POWER : f32 = 96.0;

struct SkyOut {
  @builtin(position) clip : vec4<f32>,
  @location(0) p          : vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i : u32) -> SkyOut {
  // One triangle that covers the screen: (-1,-1), (3,-1), (-1,3).
  let x = f32(i & 1u) * 4.0 - 1.0;
  let y = f32(i >> 1u) * 4.0 - 1.0;
  var out : SkyOut;
  out.clip = vec4<f32>(x, y, 0.0, 1.0);
  out.p = vec2<f32>(x, y);
  return out;
}

@fragment
fn fs_main(in : SkyOut) -> @location(0) vec4<f32> {
  // The ray through this pixel, in render space.
  let cam = vec3<f32>(in.p.x * frame.params.y * frame.params.z, in.p.y * frame.params.y, -1.0);
  let ray = normalize((frame.inv_view * vec4<f32>(cam, 0.0)).xyz);
  // Sine of the ray's elevation above the local horizontal at the eye.
  let s = dot(ray, frame.eye_up.xyz);
  let sin_dip = frame.earth.z;
  let height = frame.earth.x;

  if (s < -sin_dip) {
    if (frame.debug.x > 0.5) {
      return vec4<f32>(KEY_GLOBE, 1.0);
    }
    var t : f32;
    if (frame.debug.y > 0.5) {
      // The seeded fault: a flat earth, whose horizon is the eye line.
      t = height / max(-s, 0.000001);
    } else {
      let b = (frame.earth.y + height) * (-s);
      t = frame.earth.w / (b + sqrt(max(b * b - frame.earth.w, 0.0)));
    }
    return vec4<f32>(apply_haze(GLOBE_BASE, t, frame.params.w, 0.0), 1.0);
  }
  if (frame.debug.x > 0.5) {
    return vec4<f32>(KEY_SKY, 1.0);
  }

  // Sky: brightest at the geometric horizon, bluer towards the zenith.
  let above = clamp((s + sin_dip) / (1.0 + sin_dip), 0.0, 1.0);
  var colour = mix(HAZE_SKY, SKY_ZENITH, pow(above, 0.45));
  let sun = dot(ray, frame.sun_dir.xyz);
  colour = colour + vec3<f32>(1.0, 0.85, 0.6) * pow(max(sun, 0.0), SUN_GLOW_POWER) * 0.35;
  if (sun > SUN_DISC_COS) {
    colour = vec3<f32>(1.0, 0.97, 0.88);
  }
  return vec4<f32>(colour, 1.0);
}
