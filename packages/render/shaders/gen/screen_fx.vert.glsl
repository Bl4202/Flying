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

smooth out vec2 _vs2fs_location0;

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
    uint i = uint(gl_VertexID);
    FxOut out_ = FxOut(vec4(0.0), vec2(0.0));
    float x = ((float((i & 1u)) * 4.0) - 1.0);
    float y = ((float((i >> 1u)) * 4.0) - 1.0);
    out_.clip = vec4(x, y, 0.0, 1.0);
    out_.p = vec2(x, y);
    FxOut _e22 = out_;
    gl_Position = _e22.clip;
    _vs2fs_location0 = _e22.p;
    return;
}

