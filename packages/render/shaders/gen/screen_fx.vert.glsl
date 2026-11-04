#version 300 es

precision highp float;
precision highp int;

struct Frame {
    mat4x4 view_proj;
    mat4x4 inv_view;
    vec4 sun_dir;
    vec4 params;
    vec4 earth;
    vec4 eye_up;
    vec4 fx;
    vec4 debug;
};
struct FxOut {
    vec4 clip;
    vec2 p;
};
const vec3 HAZE_GROUND = vec3(0.6, 0.66, 0.72);
const vec3 HAZE_SKY = vec3(0.74, 0.82, 0.92);
const vec3 SKY_ZENITH = vec3(0.2, 0.42, 0.8);
const vec3 GLOBE_BASE = vec3(0.28, 0.34, 0.24);
const float HAZE_DENSITY = 5e-5;
const float HAZE_SCALE_M = 7000.0;
const vec3 KEY_SKY = vec3(0.0, 0.0, 1.0);
const vec3 KEY_GLOBE = vec3(1.0, 0.0, 1.0);
const vec3 KEY_MARKER = vec3(1.0, 1.0, 0.0);
const float GREY_FROM = 0.35;
const float BLACKOUT = 0.7;
const float BLACK_FROM = 0.6;
const float RED_FROM = 0.45;
const float RED_STRONG = 0.65;
const float SIGHT_LOSS_FROM = 0.85;
const float VIGNETTE_MIN = 0.3;

smooth out vec2 _vs2fs_location0;

vec3 key_level(float level) {
    return vec3(0.0, ((255.0 - (level * 64.0)) / 255.0), 0.0);
}

float lambert(vec3 n, vec3 l) {
    return max(dot(normalize(n), normalize(l)), 0.0);
}

float optical_depth(float length_m, float y0_, float y1_) {
    float mean = 0.0;
    float a = clamp(y0_, -500.0, 40000.0);
    float b = clamp(y1_, -500.0, 40000.0);
    mean = exp((-(a) * 0.00014285714));
    if (((abs((b - a)) * 0.00014285714) > 0.001)) {
        mean = ((exp((-(a) * 0.00014285714)) - exp((-(b) * 0.00014285714))) / ((b - a) * 0.00014285714));
    }
    float _e31 = mean;
    return ((HAZE_DENSITY * length_m) * _e31);
}

vec3 apply_haze(vec3 colour, float length_m_1, float y0_1, float y1_1) {
    float _e4 = optical_depth(length_m_1, y0_1, y1_1);
    float amount = (1.0 - exp(-(_e4)));
    return mix(colour, HAZE_GROUND, clamp(amount, 0.0, 1.0));
}

vec4 over(vec4 top, vec4 bottom) {
    float a_1 = (top.w + (bottom.w * (1.0 - top.w)));
    vec3 rgb = (((top.xyz * top.w) + ((bottom.xyz * bottom.w) * (1.0 - top.w))) / vec3(max(a_1, 0.0001)));
    return vec4(rgb, a_1);
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

