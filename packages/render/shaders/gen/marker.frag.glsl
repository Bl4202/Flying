#version 300 es

precision highp float;
precision highp int;

struct Frame {
    mat4x4 view_proj;
    vec4 sun_dir;
    vec4 params;
    vec4 patch_offset;
};
struct VsOut {
    vec4 clip;
    vec3 normal;
    vec3 tint;
    float distance_m;
};
const vec3 SKY = vec3(0.45, 0.62, 0.85);
const float FOG_PER_METRE = 2.5e-5;

layout(std140) uniform Frame_block_0Fragment { Frame _group_0_binding_0_fs; };

smooth in vec3 _vs2fs_location0;
smooth in vec3 _vs2fs_location1;
smooth in float _vs2fs_location2;
layout(location = 0) out vec4 _fs2p_location0;

float lambert(vec3 n, vec3 l) {
    return max(dot(normalize(n), normalize(l)), 0.0);
}

vec3 fog(vec3 colour, float distance_m) {
    float amount = (1.0 - exp((-(distance_m) * FOG_PER_METRE)));
    return mix(colour, SKY, clamp(amount, 0.0, 1.0));
}

vec3 qrot(vec4 q, vec3 v) {
    vec3 t = (2.0 * cross(q.xyz, v));
    return ((v + (q.w * t)) + cross(q.xyz, t));
}

void main() {
    VsOut in_ = VsOut(gl_FragCoord, _vs2fs_location0, _vs2fs_location1, _vs2fs_location2);
    vec4 _e4 = _group_0_binding_0_fs.sun_dir;
    float _e6 = lambert(in_.normal, _e4.xyz);
    vec3 lit = (in_.tint * (0.35 + (0.65 * _e6)));
    vec3 _e14 = fog(lit, in_.distance_m);
    _fs2p_location0 = vec4(_e14, 1.0);
    return;
}

