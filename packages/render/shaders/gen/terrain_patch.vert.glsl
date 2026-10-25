#version 300 es

precision highp float;
precision highp int;

uniform uint naga_vs_first_instance;

struct Frame {
    mat4x4 view_proj;
    vec4 sun_dir;
    vec4 params;
    vec4 patch_offset;
    vec4 runway;
    vec4 fx;
};
struct VsOut {
    vec4 clip;
    vec3 normal;
    float height_m;
    vec2 cell;
    float distance_m;
    vec2 local_xz;
    float runway_on;
};
const vec3 SKY = vec3(0.45, 0.62, 0.85);
const float FOG_PER_METRE = 2.5e-5;
const uint PATCH_GRID_COLUMNS = 8u;
const vec3 ASPHALT = vec3(0.2, 0.2, 0.22);
const vec3 PAINT = vec3(0.9, 0.9, 0.88);
const float LINE_WIDTH_M = 0.9;
const float DASH_ON_M = 30.0;
const float DASH_PERIOD_M = 50.0;
const float DASH_CLEAR_M = 12.0;
const float EDGE_INSET_M = 1.5;
const float BARS_FROM_M = 6.0;
const float BARS_TO_M = 36.0;
const float BAR_WIDTH_M = 1.8;
const float BAR_PITCH_M = 3.6;
const float BAR_CLEAR_M = 3.0;

layout(std140) uniform Frame_block_0Vertex { Frame _group_0_binding_0_vs; };

layout(location = 0) in vec3 _p2vs_location0;
layout(location = 1) in vec3 _p2vs_location1;
layout(location = 2) in float _p2vs_location2;
smooth out vec3 _vs2fs_location0;
smooth out float _vs2fs_location1;
smooth out vec2 _vs2fs_location2;
smooth out float _vs2fs_location3;
smooth out vec2 _vs2fs_location4;
smooth out float _vs2fs_location5;

float lambert(vec3 n, vec3 l) {
    return max(dot(normalize(n), normalize(l)), 0.0);
}

vec3 fog(vec3 colour, float distance_m) {
    float amount = (1.0 - exp((-(distance_m) * FOG_PER_METRE)));
    return mix(colour, SKY, clamp(amount, 0.0, 1.0));
}

float stripe(float distance_from_centre, float half_width, float aa) {
    return (1.0 - smoothstep((half_width - aa), (half_width + aa), abs(distance_from_centre)));
}

float visible(float width_m, float aa_1) {
    return clamp((width_m / max((aa_1 * 2.0), 0.0001)), 0.0, 1.0);
}

vec4 runway_colour(vec2 local_xz, float aa_x, float aa_z) {
    float length_m = _group_0_binding_0_vs.runway.z;
    float _e10 = _group_0_binding_0_vs.runway.w;
    float half_width_1 = (_e10 * 0.5);
    float _e16 = _group_0_binding_0_vs.runway.y;
    float along = (_e16 - local_xz.y);
    float _e23 = _group_0_binding_0_vs.runway.x;
    float across = (local_xz.x - _e23);
    float aa_across = max(aa_x, 0.001);
    float aa_along = max(aa_z, 0.001);
    float abs_across = abs(across);
    float started = smoothstep(-(aa_along), aa_along, along);
    float not_ended = (1.0 - smoothstep((length_m - aa_along), (length_m + aa_along), along));
    float _e37 = stripe(across, half_width_1, aa_across);
    float on = ((_e37 * started) * not_ended);
    float _e44 = stripe((abs_across - (half_width_1 - EDGE_INSET_M)), 0.45, aa_across);
    float _e46 = visible(LINE_WIDTH_M, aa_across);
    float edge = (_e44 * _e46);
    float soft = (aa_along / DASH_PERIOD_M);
    float phase = fract(((along - DASH_CLEAR_M) / DASH_PERIOD_M));
    float dash = (1.0 - smoothstep((0.6 - soft), (0.6 + soft), phase));
    float in_dashes = (smoothstep(0.0, aa_along, (along - DASH_CLEAR_M)) * (1.0 - smoothstep(0.0, aa_along, (along - (length_m - DASH_CLEAR_M)))));
    float _e74 = stripe(across, 0.45, aa_across);
    float _e78 = visible(LINE_WIDTH_M, aa_across);
    float centre = (((_e74 * dash) * in_dashes) * _e78);
    float bar_cell = (fract(((abs_across - BAR_CLEAR_M) / BAR_PITCH_M)) - 0.5);
    float bar_inside = (smoothstep(-(aa_across), aa_across, (abs_across - BAR_CLEAR_M)) * (1.0 - smoothstep(-(aa_across), aa_across, (abs_across - ((half_width_1 - EDGE_INSET_M) - 1.0)))));
    float _e104 = stripe((bar_cell * BAR_PITCH_M), 0.9, aa_across);
    float _e106 = visible(BAR_WIDTH_M, aa_across);
    float bar = ((((_e104 * _e106) * bar_inside) * smoothstep((BARS_FROM_M - aa_along), (BARS_FROM_M + aa_along), along)) * (1.0 - smoothstep((BARS_TO_M - aa_along), (BARS_TO_M + aa_along), along)));
    float paint = clamp(((edge + centre) + bar), 0.0, 1.0);
    return vec4(mix(ASPHALT, PAINT, paint), on);
}

void main() {
    vec3 pos_local = _p2vs_location0;
    vec3 normal = _p2vs_location1;
    float height = _p2vs_location2;
    uint instance = (uint(gl_InstanceID) + naga_vs_first_instance);
    VsOut out_ = VsOut(vec4(0.0), vec3(0.0), 0.0, vec2(0.0), 0.0, vec2(0.0), 0.0);
    vec3 step_ = vec3(float((instance % PATCH_GRID_COLUMNS)), 0.0, float((instance / PATCH_GRID_COLUMNS)));
    vec4 _e14 = _group_0_binding_0_vs.patch_offset;
    float _e20 = _group_0_binding_0_vs.params.z;
    vec3 pos_rel = ((pos_local + _e14.xyz) + (step_ * _e20));
    mat4x4 _e27 = _group_0_binding_0_vs.view_proj;
    out_.clip = (_e27 * vec4(pos_rel, 1.0));
    out_.normal = normal;
    out_.height_m = height;
    float _e38 = _group_0_binding_0_vs.params.y;
    out_.cell = (pos_local.xz / vec2(_e38));
    out_.distance_m = length(pos_rel);
    out_.local_xz = pos_local.xz;
    out_.runway_on = ((instance == 0u) ? 1.0 : 0.0);
    VsOut _e51 = out_;
    gl_Position = _e51.clip;
    _vs2fs_location0 = _e51.normal;
    _vs2fs_location1 = _e51.height_m;
    _vs2fs_location2 = _e51.cell;
    _vs2fs_location3 = _e51.distance_m;
    _vs2fs_location4 = _e51.local_xz;
    _vs2fs_location5 = _e51.runway_on;
    return;
}

