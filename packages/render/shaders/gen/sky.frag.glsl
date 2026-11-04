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
struct SkyOut {
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
const vec3 KEY_MODEL = vec3(1.0, 0.5019608, 0.0);
const vec3 KEY_COCKPIT = vec3(0.5019608, 0.0, 1.0);
const vec3 KEY_HUD = vec3(1.0, 0.0, 0.0);
const float SUN_DISC_COS = 0.99999;
const float SUN_GLOW_POWER = 96.0;

layout(std140) uniform Frame_block_0Fragment { Frame _group_0_binding_0_fs; };

smooth in vec2 _vs2fs_location0;
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

vec3 apply_haze(vec3 colour_1, float length_m_1, float y0_1, float y1_1) {
    float _e4 = optical_depth(length_m_1, y0_1, y1_1);
    float amount = (1.0 - exp(-(_e4)));
    return mix(colour_1, HAZE_GROUND, clamp(amount, 0.0, 1.0));
}

vec3 fog(vec3 colour_2, float distance_m) {
    float _e5 = _group_0_binding_0_fs.params.w;
    float _e9 = _group_0_binding_0_fs.params.w;
    vec3 _e10 = apply_haze(colour_2, distance_m, _e5, _e9);
    return _e10;
}

void main() {
    SkyOut in_ = SkyOut(gl_FragCoord, _vs2fs_location0);
    float t = 0.0;
    vec3 colour = vec3(0.0);
    float _e6 = _group_0_binding_0_fs.params.y;
    float _e11 = _group_0_binding_0_fs.params.z;
    float _e18 = _group_0_binding_0_fs.params.y;
    vec3 cam = vec3(((in_.p.x * _e6) * _e11), (in_.p.y * _e18), -1.0);
    mat4x4 _e24 = _group_0_binding_0_fs.inv_view;
    vec3 ray = normalize((_e24 * vec4(cam, 0.0)).xyz);
    vec4 _e32 = _group_0_binding_0_fs.eye_up;
    float s = dot(ray, _e32.xyz);
    float sin_dip = _group_0_binding_0_fs.earth.z;
    float height = _group_0_binding_0_fs.earth.x;
    if ((s < -(sin_dip))) {
        float _e48 = _group_0_binding_0_fs.debug.x;
        if ((_e48 > 0.5)) {
            _fs2p_location0 = vec4(vec3(1.0, 0.0, 1.0), 1.0);
            return;
        }
        float _e61 = _group_0_binding_0_fs.debug.y;
        if ((_e61 > 0.5)) {
            t = (height / max(-(s), 1e-6));
        } else {
            float _e71 = _group_0_binding_0_fs.earth.y;
            float b_1 = ((_e71 + height) * -(s));
            float _e78 = _group_0_binding_0_fs.earth.w;
            float _e83 = _group_0_binding_0_fs.earth.w;
            t = (_e78 / (b_1 + sqrt(max(((b_1 * b_1) - _e83), 0.0))));
        }
        float _e91 = t;
        float _e95 = _group_0_binding_0_fs.params.w;
        vec3 _e97 = apply_haze(GLOBE_BASE, _e91, _e95, 0.0);
        _fs2p_location0 = vec4(_e97, 1.0);
        return;
    }
    float _e103 = _group_0_binding_0_fs.debug.x;
    if ((_e103 > 0.5)) {
        _fs2p_location0 = vec4(vec3(0.0, 0.0, 1.0), 1.0);
        return;
    }
    float above = clamp(((s + sin_dip) / (1.0 + sin_dip)), 0.0, 1.0);
    colour = mix(HAZE_SKY, SKY_ZENITH, pow(above, 0.45));
    vec4 _e127 = _group_0_binding_0_fs.sun_dir;
    float sun = dot(ray, _e127.xyz);
    vec3 _e130 = colour;
    colour = (_e130 + ((vec3(1.0, 0.85, 0.6) * pow(max(sun, 0.0), SUN_GLOW_POWER)) * 0.35));
    if ((sun > SUN_DISC_COS)) {
        colour = vec3(1.0, 0.97, 0.88);
    }
    vec3 _e149 = colour;
    _fs2p_location0 = vec4(_e149, 1.0);
    return;
}

