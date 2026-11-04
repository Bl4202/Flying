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

layout(std140) uniform Frame_block_0Fragment { Frame _group_0_binding_0_fs; };

layout(std140) uniform Model_block_1Fragment { Model _group_0_binding_2_fs; };

smooth in vec3 _vs2fs_location0;
smooth in float _vs2fs_location1;
smooth in float _vs2fs_location2;
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
    VsOut in_ = VsOut(gl_FragCoord, _vs2fs_location0, _vs2fs_location1, _vs2fs_location2);
    bool local = false;
    float _e4 = _group_0_binding_0_fs.debug.x;
    if ((_e4 > 0.5)) {
        _fs2p_location0 = vec4(((in_.material > 5.5) ? KEY_COCKPIT : KEY_MODEL), 1.0);
        return;
    }
    if ((in_.material > 2.5)) {
        local = (in_.material < 3.5);
    } else {
        local = false;
    }
    bool _e24 = local;
    if (_e24) {
        float _e36 = _group_0_binding_2_fs.glow.x;
        _fs2p_location0 = vec4(mix(vec3(0.12, 0.08, 0.07), vec3(1.0, 0.55, 0.15), _e36), 1.0);
        return;
    }
    vec4 _e43 = _group_0_binding_0_fs.sun_dir;
    float _e45 = lambert(in_.normal, _e43.xyz);
    vec3 _e47 = paint(in_.material);
    vec3 lit = (_e47 * (0.35 + (0.65 * _e45)));
    vec3 _e54 = fog(lit, in_.distance_m);
    _fs2p_location0 = vec4(_e54, 1.0);
    return;
}

