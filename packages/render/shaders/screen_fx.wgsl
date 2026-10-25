#include "common.wgsl"

// The pilot's body on the screen (GDD 11.14): drawn over the finished picture as one triangle with
// alpha blending, so it needs no copy of the picture.
//
//   stress S      0.35 to 0.7   greyout: the picture desaturates and the edges close in, the
//                               vignette radius going from the whole screen to 30 %
//                 0.6 to 0.7    the picture fades to black; from 0.7 it is black
//   red-out R     from 0.45     a red veil, strong by 0.65; from 0.85 the sight goes
//
// `frame.fx.z` is the effect strength (the player's setting, 0 to 1) and scales every effect.

const GREY_FROM : f32 = 0.35;
const BLACKOUT : f32 = 0.7;
const BLACK_FROM : f32 = 0.6;
const RED_FROM : f32 = 0.45;
const RED_STRONG : f32 = 0.65;
const SIGHT_LOSS_FROM : f32 = 0.85;
const VIGNETTE_MIN : f32 = 0.3;

struct FxOut {
  @builtin(position) clip : vec4<f32>,
  @location(0) p          : vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) i : u32) -> FxOut {
  // One triangle that covers the screen: (-1,-1), (3,-1), (-1,3).
  let x = f32(i & 1u) * 4.0 - 1.0;
  let y = f32(i >> 1u) * 4.0 - 1.0;
  var out : FxOut;
  out.clip = vec4<f32>(x, y, 0.0, 1.0);
  out.p = vec2<f32>(x, y);
  return out;
}

/// `top` laid over `bottom`, both as colour with alpha (not premultiplied).
fn over(top : vec4<f32>, bottom : vec4<f32>) -> vec4<f32> {
  let a = top.a + bottom.a * (1.0 - top.a);
  let rgb = (top.rgb * top.a + bottom.rgb * bottom.a * (1.0 - top.a)) / max(a, 0.0001);
  return vec4<f32>(rgb, a);
}

@fragment
fn fs_main(in : FxOut) -> @location(0) vec4<f32> {
  let k = clamp(frame.fx.z, 0.0, 1.0);
  let s = frame.fx.x;
  let r = frame.fx.y;

  // How far the greyout has gone, 0 to 1 over S = 0.35 to 0.7.
  let grey = clamp((s - GREY_FROM) / (BLACKOUT - GREY_FROM), 0.0, 1.0) * k;
  // The edges close in: the clear radius goes from the whole screen (the corner is at 1.414) to 30 %.
  let radius = 1.4142 * mix(1.0, VIGNETTE_MIN, grey);
  let dist = length(in.p);
  let closing = smoothstep(radius * 0.55, radius, dist) * min(grey * 2.0, 1.0);
  let veil = grey * 0.45;
  let black = smoothstep(BLACK_FROM, BLACKOUT, s) * k;

  let red = smoothstep(RED_FROM, RED_STRONG, r) * k;
  let sight = smoothstep(SIGHT_LOSS_FROM, 1.0, r) * k;

  var layer = vec4<f32>(0.40, 0.40, 0.42, veil);                          // desaturates and flattens
  layer = over(vec4<f32>(0.80, 0.04, 0.04, red * 0.85), layer);           // red-out
  layer = over(vec4<f32>(0.0, 0.0, 0.0, clamp(closing + sight, 0.0, 1.0)), layer);  // tunnel
  layer = over(vec4<f32>(0.0, 0.0, 0.0, black), layer);                   // blackout
  return layer;
}
