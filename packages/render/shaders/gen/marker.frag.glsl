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
struct VsOut {
    vec4 clip;
    vec3 normal;
    vec3 tint;
    float distance_m;
    float cue;
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
const vec3 KEY_MODEL = vec3(1.0, 0.5019608, 0.0);
const vec3 KEY_COCKPIT = vec3(0.5019608, 0.0, 1.0);
const vec3 KEY_HUD = vec3(1.0, 0.0, 0.0);

layout(std140) uniform Frame_block_0Fragment { Frame _group_0_binding_0_fs; };

smooth in vec3 _vs2fs_location0;
smooth in vec3 _vs2fs_location1;
smooth in float _vs2fs_location2;
smooth in float _vs2fs_location3;
layout(location = 0) out vec4 _fs2p_location0;

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

vec3 fog(vec3 colour_1, float distance_m) {
    float _e5 = _group_0_binding_0_fs.params.w;
    float _e9 = _group_0_binding_0_fs.params.w;
    vec3 _e10 = apply_haze(colour_1, distance_m, _e5, _e9);
    return _e10;
}

vec3 qrot(vec4 q, vec3 v) {
    vec3 t = (2.0 * cross(q.xyz, v));
    return ((v + (q.w * t)) + cross(q.xyz, t));
}

void main() {
    VsOut in_ = VsOut(gl_FragCoord, _vs2fs_location0, _vs2fs_location1, _vs2fs_location2, _vs2fs_location3);
    vec4 _e4 = _group_0_binding_0_fs.sun_dir;
    float _e6 = lambert(in_.normal, _e4.xyz);
    vec3 lit = (in_.tint * (0.35 + (0.65 * _e6)));
    vec3 _e14 = fog(lit, in_.distance_m);
    _fs2p_location0 = vec4(((in_.cue > 0.5) ? in_.tint : _e14), 1.0);
    return;
}

