#version 300 es

precision highp float;
precision highp int;

struct Frame {
    mat4x4 view_proj;
    vec4 sun_dir;
    vec4 params;
    vec4 patch_offset;
    vec4 runway;
    vec4 fx;
};
struct FxOut {
    vec4 clip;
    vec2 p;
};
const vec3 SKY = vec3(0.45, 0.62, 0.85);
const float FOG_PER_METRE = 2.5e-5;
const float GREY_FROM = 0.35;
const float BLACKOUT = 0.7;
const float BLACK_FROM = 0.6;
const float RED_FROM = 0.45;
const float RED_STRONG = 0.65;
const float SIGHT_LOSS_FROM = 0.85;
const float VIGNETTE_MIN = 0.3;

layout(std140) uniform Frame_block_0Fragment { Frame _group_0_binding_0_fs; };

smooth in vec2 _vs2fs_location0;
layout(location = 0) out vec4 _fs2p_location0;

float lambert(vec3 n, vec3 l) {
    return max(dot(normalize(n), normalize(l)), 0.0);
}

vec3 fog(vec3 colour, float distance_m) {
    float amount = (1.0 - exp((-(distance_m) * FOG_PER_METRE)));
    return mix(colour, SKY, clamp(amount, 0.0, 1.0));
}

vec4 over(vec4 top, vec4 bottom) {
    float a = (top.w + (bottom.w * (1.0 - top.w)));
    vec3 rgb = (((top.xyz * top.w) + ((bottom.xyz * bottom.w) * (1.0 - top.w))) / vec3(max(a, 0.0001)));
    return vec4(rgb, a);
}

void main() {
    FxOut in_ = FxOut(gl_FragCoord, _vs2fs_location0);
    vec4 layer = vec4(0.0);
    float _e4 = _group_0_binding_0_fs.fx.z;
    float k = clamp(_e4, 0.0, 1.0);
    float s = _group_0_binding_0_fs.fx.x;
    float r = _group_0_binding_0_fs.fx.y;
    float grey = (clamp(((s - GREY_FROM) / 0.35), 0.0, 1.0) * k);
    float radius = (1.4142 * mix(1.0, VIGNETTE_MIN, grey));
    float dist = length(in_.p);
    float closing = (smoothstep((radius * 0.55), radius, dist) * min((grey * 2.0), 1.0));
    float veil = (grey * 0.45);
    float black = (smoothstep(BLACK_FROM, BLACKOUT, s) * k);
    float red = (smoothstep(RED_FROM, RED_STRONG, r) * k);
    float sight = (smoothstep(SIGHT_LOSS_FROM, 1.0, r) * k);
    layer = vec4(0.4, 0.4, 0.42, veil);
    vec4 _e64 = layer;
    vec4 _e65 = over(vec4(0.8, 0.04, 0.04, (red * 0.85)), _e64);
    layer = _e65;
    vec4 _e74 = layer;
    vec4 _e75 = over(vec4(0.0, 0.0, 0.0, clamp((closing + sight), 0.0, 1.0)), _e74);
    layer = _e75;
    vec4 _e80 = layer;
    vec4 _e81 = over(vec4(0.0, 0.0, 0.0, black), _e80);
    layer = _e81;
    vec4 _e82 = layer;
    _fs2p_location0 = _e82;
    return;
}

