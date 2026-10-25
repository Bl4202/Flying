#version 300 es

precision highp float;
precision highp int;

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

layout(std140) uniform Frame_block_0Fragment { Frame _group_0_binding_0_fs; };

smooth in vec3 _vs2fs_location0;
smooth in float _vs2fs_location1;
smooth in vec2 _vs2fs_location2;
smooth in float _vs2fs_location3;
smooth in vec2 _vs2fs_location4;
smooth in float _vs2fs_location5;
layout(location = 0) out vec4 _fs2p_location0;

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
    float length_m = _group_0_binding_0_fs.runway.z;
    float _e10 = _group_0_binding_0_fs.runway.w;
    float half_width_1 = (_e10 * 0.5);
    float _e16 = _group_0_binding_0_fs.runway.y;
    float along = (_e16 - local_xz.y);
    float _e23 = _group_0_binding_0_fs.runway.x;
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
    VsOut in_ = VsOut(gl_FragCoord, _vs2fs_location0, _vs2fs_location1, _vs2fs_location2, _vs2fs_location3, _vs2fs_location4, _vs2fs_location5);
    vec3 base = vec3(0.0);
    float grid = 0.0;
    bool local = false;
    float aa_x_1 = fwidth(in_.local_xz.x);
    float aa_z_1 = fwidth(in_.local_xz.y);
    vec4 _e10 = _group_0_binding_0_fs.sun_dir;
    float _e12 = lambert(in_.normal, _e10.xyz);
    vec3 lowland = vec3(0.2, 0.35, 0.15);
    vec3 highland = vec3(0.55, 0.5, 0.45);
    base = mix(lowland, highland, clamp((in_.height_m / 1200.0), 0.0, 1.0));
    float edge_1 = max(fract(in_.cell.x), fract(in_.cell.y));
    grid = (1.0 - (0.07 * step(0.96, edge_1)));
    float _e46 = _group_0_binding_0_fs.runway.z;
    if ((_e46 > 0.0)) {
        local = (in_.runway_on > 0.5);
    } else {
        local = false;
    }
    bool _e55 = local;
    if (_e55) {
        vec4 _e57 = runway_colour(in_.local_xz, aa_x_1, aa_z_1);
        vec3 _e58 = base;
        base = mix(_e58, _e57.xyz, _e57.w);
        float _e62 = grid;
        grid = mix(_e62, 1.0, _e57.w);
    }
    vec3 _e66 = base;
    float _e72 = grid;
    vec3 lit = ((_e66 * (0.25 + (0.75 * _e12))) * _e72);
    vec3 _e75 = fog(lit, in_.distance_m);
    _fs2p_location0 = vec4(_e75, 1.0);
    return;
}

