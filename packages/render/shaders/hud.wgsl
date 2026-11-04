#include "common.wgsl"

// The HUD (ADR-41): segments in the body-fixed tangent plane, drawn as capsules of constant pixel
// width in the main pass.
//
// Each instance is one segment: endpoints (u0, v0) and (u1, v1) in milliradians, a width scale, an
// alpha and a tint. A point (u, v) is the body direction (1, u / 1000, -v / 1000). In the cockpit
// view it is projected as a *direction at infinity* (w = 0), so moving the eye moves nothing: the
// symbols are collimated by construction. Each fragment is then tested against the combiner glass:
// the ray from the eye along that direction has to pass through the glass rectangle, which cuts the
// picture at the combiner's edge and fades it as the eye leaves the eye box. In the chase view the
// plane is drawn straight on the screen, at the overlay scale.

struct Hud {
  rot      : mat4x4<f32>,  // body to render rotation, no translation
  colour   : vec4<f32>,    // rgb of the palette, brightness
  viewport : vec4<f32>,    // width px, height px, stroke width px, glow px
  mode     : vec4<f32>,    // overlay (0 or 1), overlay NDC per tangent unit, aspect, eye-box fade
  eye      : vec4<f32>,    // the eye in body axes, metres
  glass0   : vec4<f32>,    // glass corner 0 (body), width in metres
  glass_a  : vec4<f32>,    // unit vector along the glass's first edge
  glass_b  : vec4<f32>,    // unit vector along its second edge, height in metres
  glass_n  : vec4<f32>,    // the glass's normal
};

@group(0) @binding(3) var<uniform> hud : Hud;

const AMBER : vec3<f32> = vec3<f32>(1.0, 0.70, 0.10);
const RED : vec3<f32> = vec3<f32>(1.0, 0.22, 0.16);

struct VsOut {
  @builtin(position) clip : vec4<f32>,
  @location(0) along      : f32,
  @location(1) across     : f32,
  @location(2) length_px  : f32,
  @location(3) uv0        : vec2<f32>,
  @location(4) uv1        : vec2<f32>,
  @location(5) style      : vec3<f32>,
};

/// A plane point in milliradians to NDC; z is 1 if it can be drawn, 0 if it is behind the camera.
fn plane_to_ndc(uv_mrad : vec2<f32>) -> vec3<f32> {
  let uv = uv_mrad * 0.001;
  if (hud.mode.x > 0.5) {
    return vec3<f32>(uv.x * hud.mode.y / hud.mode.z, uv.y * hud.mode.y, 1.0);
  }
  let body = vec4<f32>(1.0, uv.x, -uv.y, 0.0);
  let dir = (hud.rot * body).xyz;
  // A direction at infinity: w = 0, so only the rotation and the projection act on it.
  let c = frame.view_proj * vec4<f32>(dir, 0.0);
  return vec3<f32>(c.xy / max(c.w, 0.000001), select(0.0, 1.0, c.w > 0.000001));
}

@vertex
fn vs_main(
  @location(0) seg : vec4<f32>,    // u0, v0, u1, v1 in mrad
  @location(1) style : vec4<f32>,  // width scale, alpha, tint, unused
  @builtin(vertex_index) vi : u32,
) -> VsOut {
  let a = plane_to_ndc(seg.xy);
  let b = plane_to_ndc(seg.zw);
  var out : VsOut;
  out.uv0 = seg.xy * 0.001;
  out.uv1 = seg.zw * 0.001;
  out.style = style.xyz;
  if (a.z < 0.5 || b.z < 0.5) {
    // A segment with an end behind the camera is not drawn.
    out.clip = vec4<f32>(2.0, 2.0, 0.0, 1.0);
    out.along = 0.0;
    out.across = 0.0;
    out.length_px = 0.0;
    return out;
  }
  let half = hud.viewport.xy * 0.5;
  let p0 = a.xy * half;
  let p1 = b.xy * half;
  let delta = p1 - p0;
  let len = length(delta);
  var t = vec2<f32>(1.0, 0.0);
  if (len > 0.001) {
    t = delta / len;
  }
  let n = vec2<f32>(-t.y, t.x);
  let reach = hud.viewport.z * style.x * 0.5 + hud.viewport.w + 1.0;
  let at_end = vi >= 2u;
  let side = select(-1.0, 1.0, (vi & 1u) == 1u);
  let pos = select(p0, p1, at_end) + t * select(-reach, reach, at_end) + n * side * reach;
  out.clip = vec4<f32>(pos / half, 0.0, 1.0);
  out.along = dot(pos - p0, t);
  out.across = side * reach;
  out.length_px = len;
  return out;
}

@fragment
fn fs_main(in : VsOut) -> @location(0) vec4<f32> {
  let d = vec2<f32>(in.along - clamp(in.along, 0.0, in.length_px), in.across);
  let dist = length(d);
  let half_stroke = hud.viewport.z * in.style.x * 0.5;
  let core = clamp(half_stroke + 0.5 - dist, 0.0, 1.0);
  let glow = clamp(1.0 - (dist - half_stroke) / max(hud.viewport.w + 0.001, 0.001), 0.0, 1.0) * 0.3;
  var a = max(core, glow) * in.style.y * hud.colour.w;

  if (hud.mode.x < 0.5) {
    // The combiner: where does the ray from the eye along this direction cross the glass?
    let tp = clamp(in.along / max(in.length_px, 0.0001), 0.0, 1.0);
    let uv = mix(in.uv0, in.uv1, tp);
    let dir = normalize(vec3<f32>(1.0, uv.x, -uv.y));
    let den = dot(dir, hud.glass_n.xyz);
    let t = dot(hud.glass0.xyz - hud.eye.xyz, hud.glass_n.xyz) / select(den, 0.000001, abs(den) < 0.000001);
    let rel = hud.eye.xyz + dir * t - hud.glass0.xyz;
    let s = dot(rel, hud.glass_a.xyz);
    let r = dot(rel, hud.glass_b.xyz);
    let edge = min(min(s, hud.glass0.w - s), min(r, hud.glass_b.w - r));
    let inside = select(0.0, smoothstep(-0.001, 0.004, edge), t > 0.0);
    a = a * inside * hud.mode.w;
  }

  if (frame.debug.x > 0.5) {
    if (a < 0.5) {
      discard;
    }
    return vec4<f32>(KEY_HUD, 1.0);
  }
  var colour = hud.colour.rgb;
  if (in.style.z > 1.5) {
    colour = RED;
  } else if (in.style.z > 0.5) {
    colour = AMBER;
  }
  return vec4<f32>(colour, a);
}
