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
struct Hud {
    mat4x4 rot;
    vec4 colour;
    vec4 viewport;
    vec4 mode;
    vec4 eye;
    vec4 glass0_;
    vec4 glass_a;
    vec4 glass_b;
    vec4 glass_n;
};
struct VsOut {
    vec4 clip;
    float along;
    float across;
    float length_px;
    vec2 uv0_;
    vec2 uv1_;
    vec3 style;
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
const vec3 AMBER = vec3(1.0, 0.7, 0.1);
const vec3 RED = vec3(1.0, 0.22, 0.16);

layout(std140) uniform Frame_block_0Fragment { Frame _group_0_binding_0_fs; };

layout(std140) uniform Hud_block_1Fragment { Hud _group_0_binding_3_fs; };

smooth in float _vs2fs_location0;
smooth in float _vs2fs_location1;
smooth in float _vs2fs_location2;
smooth in vec2 _vs2fs_location3;
smooth in vec2 _vs2fs_location4;
smooth in vec3 _vs2fs_location5;
layout(location = 0) out vec4 _fs2p_location0;

vec3 key_level(float level) {
    return vec3(0.0, ((255.0 - (level * 64.0)) / 255.0), 0.0);
}

float lambert(vec3 n, vec3 l) {
    return max(dot(normalize(n), normalize(l)), 0.0);
}

float optical_depth(float length_m, float y0_, float y1_) {
    float mean = 0.0;
    float a_1 = clamp(y0_, -500.0, 40000.0);
    float b = clamp(y1_, -500.0, 40000.0);
    mean = exp((-(a_1) * 0.00014285714));
    if (((abs((b - a_1)) * 0.00014285714) > 0.001)) {
        mean = ((exp((-(a_1) * 0.00014285714)) - exp((-(b) * 0.00014285714))) / ((b - a_1) * 0.00014285714));
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

vec3 plane_to_ndc(vec2 uv_mrad) {
    vec2 uv = (uv_mrad * 0.001);
    float _e6 = _group_0_binding_3_fs.mode.x;
    if ((_e6 > 0.5)) {
        float _e13 = _group_0_binding_3_fs.mode.y;
        float _e18 = _group_0_binding_3_fs.mode.z;
        float _e24 = _group_0_binding_3_fs.mode.y;
        return vec3(((uv.x * _e13) / _e18), (uv.y * _e24), 1.0);
    }
    vec4 body = vec4(1.0, uv.x, -(uv.y), 0.0);
    mat4x4 _e36 = _group_0_binding_3_fs.rot;
    vec3 dir = (_e36 * body).xyz;
    mat4x4 _e41 = _group_0_binding_0_fs.view_proj;
    vec4 c = (_e41 * vec4(dir, 0.0));
    return vec3((c.xy / vec2(max(c.w, 1e-6))), ((c.w > 1e-6) ? 1.0 : 0.0));
}

void main() {
    VsOut in_ = VsOut(gl_FragCoord, _vs2fs_location0, _vs2fs_location1, _vs2fs_location2, _vs2fs_location3, _vs2fs_location4, _vs2fs_location5);
    float a = 0.0;
    vec3 colour = vec3(0.0);
    vec2 d = vec2((in_.along - clamp(in_.along, 0.0, in_.length_px)), in_.across);
    float dist = length(d);
    float _e13 = _group_0_binding_3_fs.viewport.z;
    float half_stroke = ((_e13 * in_.style.x) * 0.5);
    float core = clamp(((half_stroke + 0.5) - dist), 0.0, 1.0);
    float _e29 = _group_0_binding_3_fs.viewport.w;
    float glow = (clamp((1.0 - ((dist - half_stroke) / max((_e29 + 0.001), 0.001))), 0.0, 1.0) * 0.3);
    float _e49 = _group_0_binding_3_fs.colour.w;
    a = ((max(core, glow) * in_.style.y) * _e49);
    float _e55 = _group_0_binding_3_fs.mode.x;
    if ((_e55 < 0.5)) {
        float tp = clamp((in_.along / max(in_.length_px, 0.0001)), 0.0, 1.0);
        vec2 uv_1 = mix(in_.uv0_, in_.uv1_, tp);
        vec3 dir_1 = normalize(vec3(1.0, uv_1.x, -(uv_1.y)));
        vec4 _e77 = _group_0_binding_3_fs.glass_n;
        float den = dot(dir_1, _e77.xyz);
        vec4 _e82 = _group_0_binding_3_fs.glass0_;
        vec4 _e86 = _group_0_binding_3_fs.eye;
        vec4 _e91 = _group_0_binding_3_fs.glass_n;
        float t_1 = (dot((_e82.xyz - _e86.xyz), _e91.xyz) / ((abs(den) < 1e-6) ? 1e-6 : den));
        vec4 _e102 = _group_0_binding_3_fs.eye;
        vec4 _e108 = _group_0_binding_3_fs.glass0_;
        vec3 rel = ((_e102.xyz + (dir_1 * t_1)) - _e108.xyz);
        vec4 _e113 = _group_0_binding_3_fs.glass_a;
        float s = dot(rel, _e113.xyz);
        vec4 _e118 = _group_0_binding_3_fs.glass_b;
        float r = dot(rel, _e118.xyz);
        float _e124 = _group_0_binding_3_fs.glass0_.w;
        float _e130 = _group_0_binding_3_fs.glass_b.w;
        float edge = min(min(s, (_e124 - s)), min(r, (_e130 - r)));
        float inside = ((t_1 > 0.0) ? smoothstep(-0.001, 0.004, edge) : 0.0);
        float _e141 = a;
        float _e146 = _group_0_binding_3_fs.mode.w;
        a = ((_e141 * inside) * _e146);
    }
    float _e151 = _group_0_binding_0_fs.debug.x;
    if ((_e151 > 0.5)) {
        float _e154 = a;
        if ((_e154 < 0.5)) {
            discard;
        }
        _fs2p_location0 = vec4(vec3(1.0, 0.0, 0.0), 1.0);
        return;
    }
    vec4 _e165 = _group_0_binding_3_fs.colour;
    colour = _e165.xyz;
    if ((in_.style.z > 1.5)) {
        colour = RED;
    } else {
        if ((in_.style.z > 0.5)) {
            colour = AMBER;
        }
    }
    vec3 _e178 = colour;
    float _e179 = a;
    _fs2p_location0 = vec4(_e178, _e179);
    return;
}

