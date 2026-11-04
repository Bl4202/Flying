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
struct TerrainLevel {
    vec4 offset;
    vec4 info;
    vec4 runway;
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

layout(std140) uniform TerrainLevel_block_1Vertex { TerrainLevel _group_0_binding_1_vs; };

layout(location = 0) in vec3 _p2vs_location0;
layout(location = 1) in vec3 _p2vs_location1;
layout(location = 2) in float _p2vs_location2;
smooth out vec3 _vs2fs_location0;
smooth out float _vs2fs_location1;
smooth out vec2 _vs2fs_location2;
smooth out float _vs2fs_location3;
smooth out vec2 _vs2fs_location4;
smooth out float _vs2fs_location5;

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

float stripe(float distance_from_centre, float half_width, float aa) {
    return (1.0 - smoothstep((half_width - aa), (half_width + aa), abs(distance_from_centre)));
}

float visible(float width_m, float aa_1) {
    return clamp((width_m / max((aa_1 * 2.0), 0.0001)), 0.0, 1.0);
}

vec4 runway_colour(vec2 local_xz, float aa_x, float aa_z) {
    float length_m_2 = _group_0_binding_1_vs.runway.z;
    float _e10 = _group_0_binding_1_vs.runway.w;
    float half_width_1 = (_e10 * 0.5);
    float _e16 = _group_0_binding_1_vs.runway.y;
    float along = (_e16 - local_xz.y);
    float _e23 = _group_0_binding_1_vs.runway.x;
    float across = (local_xz.x - _e23);
    float aa_across = max(aa_x, 0.001);
    float aa_along = max(aa_z, 0.001);
    float abs_across = abs(across);
    float started = smoothstep(-(aa_along), aa_along, along);
    float not_ended = (1.0 - smoothstep((length_m_2 - aa_along), (length_m_2 + aa_along), along));
    float _e37 = stripe(across, half_width_1, aa_across);
    float on = ((_e37 * started) * not_ended);
    float _e44 = stripe((abs_across - (half_width_1 - EDGE_INSET_M)), 0.45, aa_across);
    float _e46 = visible(LINE_WIDTH_M, aa_across);
    float edge = (_e44 * _e46);
    float soft = (aa_along / DASH_PERIOD_M);
    float phase = fract(((along - DASH_CLEAR_M) / DASH_PERIOD_M));
    float dash = (1.0 - smoothstep((0.6 - soft), (0.6 + soft), phase));
    float in_dashes = (smoothstep(0.0, aa_along, (along - DASH_CLEAR_M)) * (1.0 - smoothstep(0.0, aa_along, (along - (length_m_2 - DASH_CLEAR_M)))));
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
    vec3 pos_rel = vec3(0.0);
    VsOut out_ = VsOut(vec4(0.0), vec3(0.0), 0.0, vec2(0.0), 0.0, vec2(0.0), 0.0);
    bool local = false;
    vec3 step_ = vec3(float((instance % PATCH_GRID_COLUMNS)), 0.0, float((instance / PATCH_GRID_COLUMNS)));
    vec4 _e14 = _group_0_binding_1_vs.offset;
    float _e20 = _group_0_binding_1_vs.info.y;
    pos_rel = ((pos_local + _e14.xyz) + (step_ * _e20));
    float _e27 = _group_0_binding_0_vs.debug.y;
    if ((_e27 > 0.5)) {
        float _e33 = _group_0_binding_0_vs.debug.z;
        float _e35 = pos_rel.x;
        float east = (_e33 + _e35);
        float _e40 = _group_0_binding_0_vs.debug.w;
        float _e42 = pos_rel.z;
        float north = (_e40 - _e42);
        float d2_ = ((east * east) + (north * north));
        float radius = _group_0_binding_0_vs.earth.y;
        float _e53 = pos_rel.y;
        pos_rel.y = (_e53 + (d2_ / (radius + sqrt(max(((radius * radius) - d2_), 0.0)))));
    }
    mat4x4 _e66 = _group_0_binding_0_vs.view_proj;
    vec3 _e67 = pos_rel;
    out_.clip = (_e66 * vec4(_e67, 1.0));
    out_.normal = normal;
    out_.height_m = height;
    float _e78 = _group_0_binding_1_vs.info.x;
    out_.cell = (pos_local.xz / vec2(_e78));
    vec3 _e82 = pos_rel;
    out_.distance_m = length(_e82);
    out_.local_xz = pos_local.xz;
    if ((instance == 0u)) {
        float _e94 = _group_0_binding_1_vs.runway.z;
        local = (_e94 > 0.0);
    } else {
        local = false;
    }
    bool _e98 = local;
    out_.runway_on = (_e98 ? 1.0 : 0.0);
    VsOut _e102 = out_;
    gl_Position = _e102.clip;
    _vs2fs_location0 = _e102.normal;
    _vs2fs_location1 = _e102.height_m;
    _vs2fs_location2 = _e102.cell;
    _vs2fs_location3 = _e102.distance_m;
    _vs2fs_location4 = _e102.local_xz;
    _vs2fs_location5 = _e102.runway_on;
    return;
}

