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
    float height_m;
    vec2 cell;
    float distance_m;
};
const vec3 SKY = vec3(0.45, 0.62, 0.85);
const float FOG_PER_METRE = 2.5e-5;

layout(std140) uniform Frame_block_0Vertex { Frame _group_0_binding_0_vs; };

layout(location = 0) in vec3 _p2vs_location0;
layout(location = 1) in vec3 _p2vs_location1;
layout(location = 2) in float _p2vs_location2;
smooth out vec3 _vs2fs_location0;
smooth out float _vs2fs_location1;
smooth out vec2 _vs2fs_location2;
smooth out float _vs2fs_location3;

float lambert(vec3 n, vec3 l) {
    return max(dot(normalize(n), normalize(l)), 0.0);
}

vec3 fog(vec3 colour, float distance_m) {
    float amount = (1.0 - exp((-(distance_m) * FOG_PER_METRE)));
    return mix(colour, SKY, clamp(amount, 0.0, 1.0));
}

void main() {
    vec3 pos_local = _p2vs_location0;
    vec3 normal = _p2vs_location1;
    float height = _p2vs_location2;
    VsOut out_ = VsOut(vec4(0.0), vec3(0.0), 0.0, vec2(0.0), 0.0);
    vec4 _e5 = _group_0_binding_0_vs.patch_offset;
    vec3 pos_rel = (pos_local + _e5.xyz);
    mat4x4 _e12 = _group_0_binding_0_vs.view_proj;
    out_.clip = (_e12 * vec4(pos_rel, 1.0));
    out_.normal = normal;
    out_.height_m = height;
    float _e23 = _group_0_binding_0_vs.params.y;
    out_.cell = (pos_local.xz / vec2(_e23));
    out_.distance_m = length(pos_rel);
    VsOut _e28 = out_;
    gl_Position = _e28.clip;
    _vs2fs_location0 = _e28.normal;
    _vs2fs_location1 = _e28.height_m;
    _vs2fs_location2 = _e28.cell;
    _vs2fs_location3 = _e28.distance_m;
    return;
}

