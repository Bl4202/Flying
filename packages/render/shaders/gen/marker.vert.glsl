#version 300 es

precision highp float;
precision highp int;

uniform uint naga_vs_first_instance;

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

layout(std140) uniform Frame_block_0Vertex { Frame _group_0_binding_0_vs; };

layout(location = 0) in vec3 _p2vs_location0;
layout(location = 1) in vec3 _p2vs_location1;
layout(location = 2) in vec4 _p2vs_location2;
layout(location = 3) in vec4 _p2vs_location3;
layout(location = 4) in vec4 _p2vs_location4;
smooth out vec3 _vs2fs_location0;
smooth out vec3 _vs2fs_location1;
smooth out float _vs2fs_location2;
smooth out float _vs2fs_location3;

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
    float _e5 = _group_0_binding_0_vs.params.w;
    float _e9 = _group_0_binding_0_vs.params.w;
    vec3 _e10 = apply_haze(colour_1, distance_m, _e5, _e9);
    return _e10;
}

vec3 qrot(vec4 q, vec3 v) {
    vec3 t = (2.0 * cross(q.xyz, v));
    return ((v + (q.w * t)) + cross(q.xyz, t));
}

void main() {
    vec3 pos = _p2vs_location0;
    vec3 normal = _p2vs_location1;
    vec4 pos_scale = _p2vs_location2;
    vec4 quat = _p2vs_location3;
    vec4 tint = _p2vs_location4;
    uint index = (uint(gl_InstanceID) + naga_vs_first_instance);
    VsOut out_ = VsOut(vec4(0.0), vec3(0.0), vec3(0.0), 0.0, 0.0);
    vec3 _e9 = qrot(quat, (pos * pos_scale.w));
    vec3 pos_rel = (pos_scale.xyz + _e9);
    mat4x4 _e15 = _group_0_binding_0_vs.view_proj;
    out_.clip = (_e15 * vec4(pos_rel, 1.0));
    vec3 _e20 = qrot(quat, normal);
    out_.normal = _e20;
    vec3 hue = (vec3((float(index) * 2.399963)) + vec3(0.0, 2.094395, 4.18879));
    out_.tint = ((tint.w > 0.5) ? tint.xyz : (vec3(0.55) + (0.45 * sin(hue))));
    out_.distance_m = length(pos_scale.xyz);
    out_.cue = tint.w;
    VsOut _e47 = out_;
    gl_Position = _e47.clip;
    _vs2fs_location0 = _e47.normal;
    _vs2fs_location1 = _e47.tint;
    _vs2fs_location2 = _e47.distance_m;
    _vs2fs_location3 = _e47.cue;
    return;
}

