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
struct Model {
    mat4x4 parts[16];
    vec4 glow;
};
struct VsOut {
    vec4 clip;
    vec3 normal;
    float material;
    float distance_m;
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

layout(std140) uniform Frame_block_0Vertex { Frame _group_0_binding_0_vs; };

layout(std140) uniform Model_block_1Vertex { Model _group_0_binding_2_vs; };

layout(location = 0) in vec3 _p2vs_location0;
layout(location = 1) in vec3 _p2vs_location1;
layout(location = 2) in vec2 _p2vs_location2;
smooth out vec3 _vs2fs_location0;
smooth out float _vs2fs_location1;
smooth out float _vs2fs_location2;

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

vec3 paint(float material) {
    if ((material < 0.5)) {
        return vec3(0.56, 0.59, 0.63);
    }
    if ((material < 1.5)) {
        return vec3(0.13, 0.13, 0.15);
    }
    if ((material < 2.5)) {
        return vec3(0.32, 0.46, 0.62);
    }
    if ((material < 3.5)) {
        return vec3(0.0, 0.0, 0.0);
    }
    if ((material < 4.5)) {
        return vec3(0.07, 0.07, 0.08);
    }
    if ((material < 5.5)) {
        return vec3(0.42, 0.43, 0.46);
    }
    return vec3(0.11, 0.12, 0.13);
}

void main() {
    vec3 pos = _p2vs_location0;
    vec3 normal = _p2vs_location1;
    vec2 info = _p2vs_location2;
    VsOut out_ = VsOut(vec4(0.0), vec3(0.0), 0.0, 0.0);
    mat4x4 m = _group_0_binding_2_vs.parts[uint(info.x)];
    vec4 p = (m * vec4(pos, 1.0));
    mat4x4 _e16 = _group_0_binding_0_vs.view_proj;
    out_.clip = (_e16 * vec4(p.xyz, 1.0));
    out_.normal = (m * vec4(normal, 0.0)).xyz;
    out_.material = info.y;
    out_.distance_m = length(p.xyz);
    VsOut _e31 = out_;
    gl_Position = _e31.clip;
    _vs2fs_location0 = _e31.normal;
    _vs2fs_location1 = _e31.material;
    _vs2fs_location2 = _e31.distance_m;
    return;
}

