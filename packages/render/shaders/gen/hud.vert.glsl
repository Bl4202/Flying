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

layout(std140) uniform Frame_block_0Vertex { Frame _group_0_binding_0_vs; };

layout(std140) uniform Hud_block_1Vertex { Hud _group_0_binding_3_vs; };

layout(location = 0) in vec4 _p2vs_location0;
layout(location = 1) in vec4 _p2vs_location1;
smooth out float _vs2fs_location0;
smooth out float _vs2fs_location1;
smooth out float _vs2fs_location2;
smooth out vec2 _vs2fs_location3;
smooth out vec2 _vs2fs_location4;
smooth out vec3 _vs2fs_location5;

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
    float _e5 = _group_0_binding_0_vs.params.w;
    float _e9 = _group_0_binding_0_vs.params.w;
    vec3 _e10 = apply_haze(colour_2, distance_m, _e5, _e9);
    return _e10;
}

vec3 plane_to_ndc(vec2 uv_mrad) {
    vec2 uv = (uv_mrad * 0.001);
    float _e6 = _group_0_binding_3_vs.mode.x;
    if ((_e6 > 0.5)) {
        float _e13 = _group_0_binding_3_vs.mode.y;
        float _e18 = _group_0_binding_3_vs.mode.z;
        float _e24 = _group_0_binding_3_vs.mode.y;
        return vec3(((uv.x * _e13) / _e18), (uv.y * _e24), 1.0);
    }
    vec4 body = vec4(1.0, uv.x, -(uv.y), 0.0);
    mat4x4 _e36 = _group_0_binding_3_vs.rot;
    vec3 dir = (_e36 * body).xyz;
    mat4x4 _e41 = _group_0_binding_0_vs.view_proj;
    vec4 c = (_e41 * vec4(dir, 0.0));
    return vec3((c.xy / vec2(max(c.w, 1e-6))), ((c.w > 1e-6) ? 1.0 : 0.0));
}

void main() {
    vec4 seg = _p2vs_location0;
    vec4 style = _p2vs_location1;
    uint vi = uint(gl_VertexID);
    VsOut out_ = VsOut(vec4(0.0), 0.0, 0.0, 0.0, vec2(0.0), vec2(0.0), vec3(0.0));
    bool local = false;
    vec2 t = vec2(1.0, 0.0);
    vec3 _e4 = plane_to_ndc(seg.xy);
    vec3 _e6 = plane_to_ndc(seg.zw);
    out_.uv0_ = (seg.xy * 0.001);
    out_.uv1_ = (seg.zw * 0.001);
    out_.style = style.xyz;
    if (!((_e4.z < 0.5))) {
        local = (_e6.z < 0.5);
    } else {
        local = true;
    }
    bool _e28 = local;
    if (_e28) {
        out_.clip = vec4(2.0, 2.0, 0.0, 1.0);
        out_.along = 0.0;
        out_.across = 0.0;
        out_.length_px = 0.0;
        VsOut _e41 = out_;
        gl_Position = _e41.clip;
        _vs2fs_location0 = _e41.along;
        _vs2fs_location1 = _e41.across;
        _vs2fs_location2 = _e41.length_px;
        _vs2fs_location3 = _e41.uv0_;
        _vs2fs_location4 = _e41.uv1_;
        _vs2fs_location5 = _e41.style;
        return;
    }
    vec4 _e44 = _group_0_binding_3_vs.viewport;
    vec2 half_ = (_e44.xy * 0.5);
    vec2 p0_ = (_e4.xy * half_);
    vec2 p1_ = (_e6.xy * half_);
    vec2 delta = (p1_ - p0_);
    float len = length(delta);
    if ((len > 0.001)) {
        t = (delta / vec2(len));
    }
    float _e63 = t.y;
    float _e66 = t.x;
    vec2 n_1 = vec2(-(_e63), _e66);
    float _e71 = _group_0_binding_3_vs.viewport.z;
    float _e79 = _group_0_binding_3_vs.viewport.w;
    float reach = ((((_e71 * style.x) * 0.5) + _e79) + 1.0);
    bool at_end = (vi >= 2u);
    float side = (((vi & 1u) == 1u) ? 1.0 : -1.0);
    vec2 _e93 = t;
    vec2 pos = (((at_end ? p1_ : p0_) + (_e93 * (at_end ? reach : -(reach)))) + ((n_1 * side) * reach));
    out_.clip = vec4((pos / half_), 0.0, 1.0);
    vec2 _e108 = t;
    out_.along = dot((pos - p0_), _e108);
    out_.across = (side * reach);
    out_.length_px = len;
    VsOut _e113 = out_;
    gl_Position = _e113.clip;
    _vs2fs_location0 = _e113.along;
    _vs2fs_location1 = _e113.across;
    _vs2fs_location2 = _e113.length_px;
    _vs2fs_location3 = _e113.uv0_;
    _vs2fs_location4 = _e113.uv1_;
    _vs2fs_location5 = _e113.style;
    return;
}

