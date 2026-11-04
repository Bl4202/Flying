//! The presentation formats (ADR-43): HUD layout, cockpit, exterior model, HUD stroke font and
//! audio profile. Strict serde types like [`crate::model`], and the source of their JSON Schemas.
//!
//! They are **not** part of the sim pack: presentation data must never move the state hash. They
//! are validated here and baked into a separate bundle (`core.present.json`, [`crate::present`]).
//! Numbers carry their unit in their key, as everywhere (`_mrad`, `_db`, `_hz`, ...).
//!
//! Frames. `at_mrad` and other HUD positions are in the HUD's tangent plane `(u, v) = (tan az,
//! tan el)` scaled to milliradians: `u` to the right, `v` up, boresight at the origin. Cockpit and
//! model positions are metres in body axes (x forward, y right, z down) from the aircraft's
//! reference point.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Tag of the HUD layout format.
pub const HUD_SCHEMA: &str = "hud/1";
/// Tag of the cockpit format.
pub const COCKPIT_SCHEMA: &str = "cockpit_layout/1";
/// Tag of the audio profile format.
pub const AUDIO_SCHEMA: &str = "audio_profile/1";
/// Tag of the exterior model format.
pub const MODEL_SCHEMA: &str = "model/1";
/// Tag of the HUD stroke font format.
pub const FONT_SCHEMA: &str = "stroke_font/1";
/// Tag of the bundle the baker writes.
pub const BUNDLE_SCHEMA: &str = "present-bundle/1";

/// The values a HUD tape (or other element) may be bound to: what `HudState` in the renderer
/// provides. An element names its channel; the baker rejects any other (GDD 54.13, AC-54.8).
pub const HUD_SPEED_CHANNELS: &[&str] = &["cas", "tas", "ground_speed"];
/// Channels of the altitude tape.
pub const HUD_ALTITUDE_CHANNELS: &[&str] = &["altitude_msl"];
/// Channels of the load-factor meter.
pub const HUD_LOAD_CHANNELS: &[&str] = &["nz", "g_lagged"];

/// Animation channels of an exterior model part, as the renderer derives them from the snapshot:
/// surface deflections in degrees (positive as the flight model defines them), the gear as
/// retraction (0 down, 1 up) and the rest as fractions of travel.
pub const MODEL_CHANNELS: &[&str] = &[
    "elevator_deg",
    "aileron_deg",
    "rudder_deg",
    "flap_deg",
    "airbrake",
    "gear_retract",
    "afterburner",
];

// ---------------------------------------------------------------------------------------------
// HUD.

/// `hud/1`: one aircraft's head-up display (GDD 54.13). Symbology is conformal and collimated by
/// construction; the layers say which elements are drawn and where.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HudFile {
    /// Format tag, always `hud/1`.
    #[schemars(extend("const" = "hud/1"))]
    pub schema: String,
    /// `core:hud/<name>`.
    pub id: String,
    /// The aircraft this HUD belongs to, `core:ac/<name>`.
    pub aircraft: String,
    /// Total field of view of the combiner, horizontal then vertical.
    #[schemars(extend("x-unit" = "deg"))]
    pub tfov_deg: [f64; 2],
    /// Eye box (width, height, depth) outside which the HUD fades.
    #[schemars(extend("x-unit" = "mm"))]
    pub eye_box_mm: [f64; 3],
    /// The stroke font, `core:fnt/<name>`.
    pub font: String,
    /// Default colour, `#rrggbb` (the player's palette setting overrides it).
    pub colour: String,
    /// Glow around a stroke.
    #[schemars(extend("x-unit" = "px"), range(min = 0.0, max = 4.0))]
    pub glow_px: f64,
    /// Stroke width.
    #[schemars(extend("x-unit" = "mrad"), range(min = 0.3, max = 5.0))]
    pub stroke_mrad: f64,
    /// Text height.
    #[schemars(extend("x-unit" = "mrad"), range(min = 4.0, max = 40.0))]
    pub text_mrad: f64,
    /// The elements, drawn in this order.
    #[schemars(length(min = 1, max = 32))]
    pub layers: Vec<HudLayer>,
}

/// One HUD element, by `type`. The registry of types is this enum: an unknown type is an error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HudLayer {
    /// The aircraft datum: a small cross at the boresight.
    Boresight(Boresight),
    /// Where the aircraft is going (velocity vector), caged to the boresight at low speed.
    FlightPathMarker(FlightPathMarker),
    /// Pitch ladder with the horizon line.
    PitchLadder(PitchLadder),
    /// True heading, a horizontal tape.
    HeadingTape(HeadingTape),
    /// Airspeed, a vertical tape.
    SpeedTape(SpeedTape),
    /// Altitude, a vertical tape, with the radar altitude when low.
    AltitudeTape(AltitudeTape),
    /// Vertical speed caret beside the altitude tape.
    VsCaret(VsCaret),
    /// Load factor, with the maximum so far.
    GMeter(GMeter),
    /// Angle-of-attack bracket, shown with the gear down.
    AoaBracket(AoaBracket),
    /// Bank angle arc.
    BankScale(BankScale),
    /// Throttle, afterburner, fuel and flow.
    EngineFuelBlock(EngineFuelBlock),
    /// Navigation mode, assist level and the gear, flap and brake state.
    ModeLine(ModeLine),
    /// The most urgent warnings (at most `max_lines`).
    Warnings(WarningsLayer),
}

/// The aircraft datum.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Boresight {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Size of the cross.
    #[schemars(extend("x-unit" = "mrad"), range(min = 2.0, max = 60.0))]
    pub size_mrad: f64,
}

/// The flight-path marker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FlightPathMarker {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Size of the circle.
    #[schemars(extend("x-unit" = "mrad"), range(min = 2.0, max = 60.0))]
    pub size_mrad: f64,
    /// Below this ground speed the marker is caged to the boresight.
    #[schemars(extend("x-unit" = "m/s"), range(min = 0.0, max = 100.0))]
    pub cage_below_mps: f64,
}

/// The pitch ladder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PitchLadder {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Angle between ladder bars.
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 30.0))]
    pub step_deg: f64,
    /// Angle between numbered bars.
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 90.0))]
    pub label_step_deg: f64,
    /// Half the length of a bar.
    #[schemars(extend("x-unit" = "mrad"), range(min = 5.0, max = 200.0))]
    pub half_width_mrad: f64,
    /// Gap in the horizon line round the flight-path marker.
    #[schemars(extend("x-unit" = "mrad"), range(min = 0.0, max = 100.0))]
    pub gap_mrad: f64,
    /// Above this pitch the ladder is thinned to the numbered bars.
    #[schemars(extend("x-unit" = "deg"), range(min = 10.0, max = 90.0))]
    pub declutter_above_deg: f64,
}

/// The heading tape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeadingTape {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Centre of the tape.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
    /// Length of the tape.
    #[schemars(extend("x-unit" = "mrad"), range(min = 20.0, max = 480.0))]
    pub span_mrad: f64,
    /// Heading shown across the whole tape.
    #[schemars(extend("x-unit" = "deg"), range(min = 10.0, max = 180.0))]
    pub window_deg: f64,
    /// Spacing of ticks.
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 45.0))]
    pub tick_deg: f64,
    /// Spacing of numbers.
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 90.0))]
    pub label_deg: f64,
}

/// The airspeed tape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpeedTape {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Centre of the tape.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
    /// Length of the tape.
    #[schemars(extend("x-unit" = "mrad"), range(min = 20.0, max = 480.0))]
    pub span_mrad: f64,
    /// What it shows: one of `cas`, `tas`, `ground_speed`.
    pub channel: String,
    /// Speed shown across the whole tape.
    #[schemars(extend("x-unit" = "kn"), range(min = 10.0, max = 1000.0))]
    pub window_kn: f64,
    /// Spacing of ticks.
    #[schemars(extend("x-unit" = "kn"), range(min = 1.0, max = 200.0))]
    pub tick_kn: f64,
    /// Spacing of numbers.
    #[schemars(extend("x-unit" = "kn"), range(min = 1.0, max = 500.0))]
    pub label_kn: f64,
    /// Mach number is shown beside the speed from this Mach number.
    #[schemars(range(min = 0.1, max = 5.0))]
    pub mach_from: f64,
}

/// The altitude tape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AltitudeTape {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Centre of the tape.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
    /// Length of the tape.
    #[schemars(extend("x-unit" = "mrad"), range(min = 20.0, max = 480.0))]
    pub span_mrad: f64,
    /// What it shows: `altitude_msl`.
    pub channel: String,
    /// Altitude shown across the whole tape.
    #[schemars(extend("x-unit" = "ft"), range(min = 50.0, max = 100_000.0))]
    pub window_ft: f64,
    /// Spacing of ticks.
    #[schemars(extend("x-unit" = "ft"), range(min = 1.0, max = 10000.0))]
    pub tick_ft: f64,
    /// Spacing of numbers.
    #[schemars(extend("x-unit" = "ft"), range(min = 1.0, max = 20000.0))]
    pub label_ft: f64,
    /// Below this height above the ground the radar altitude is shown (`R 450`).
    #[schemars(extend("x-unit" = "ft"), range(min = 0.0, max = 10000.0))]
    pub radar_below_ft: f64,
}

/// The vertical speed caret.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VsCaret {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Centre of the scale.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
    /// Length of the scale.
    #[schemars(extend("x-unit" = "mrad"), range(min = 20.0, max = 480.0))]
    pub span_mrad: f64,
    /// Vertical speed at the ends of the scale (both directions).
    #[schemars(extend("x-unit" = "fpm"), range(min = 500.0, max = 60000.0))]
    pub range_fpm: f64,
}

/// The load-factor meter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GMeter {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Where the readout starts.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
    /// What it shows: one of `nz`, `g_lagged`.
    pub channel: String,
    /// Amber from this share of the aircraft's current load limit.
    #[schemars(extend("x-unit" = "pct"), range(min = 10.0, max = 100.0))]
    pub amber_pct: f64,
    /// Red from this share of the limit.
    #[schemars(extend("x-unit" = "pct"), range(min = 10.0, max = 150.0))]
    pub red_pct: f64,
}

/// The angle-of-attack bracket.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AoaBracket {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Centre of the bracket.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
    /// The on-speed angle of attack for an approach (a design target of the aircraft).
    #[schemars(extend("x-unit" = "deg"), range(min = 1.0, max = 40.0))]
    pub on_speed_deg: f64,
    /// Angle of attack shown either side of on-speed.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.5, max = 20.0))]
    pub half_range_deg: f64,
}

/// The bank scale.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BankScale {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Radius of the arc round the boresight.
    #[schemars(extend("x-unit" = "mrad"), range(min = 20.0, max = 400.0))]
    pub radius_mrad: f64,
    /// Spacing of ticks.
    #[schemars(extend("x-unit" = "deg"), range(min = 5.0, max = 45.0))]
    pub tick_deg: f64,
}

/// The engine and fuel block.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EngineFuelBlock {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Where the block starts.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
}

/// The mode line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModeLine {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Where the line starts.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
}

/// The warning lines.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WarningsLayer {
    /// Layer name, unique in the HUD.
    pub id: String,
    /// Where the first line is centred.
    #[schemars(extend("x-unit" = "mrad"))]
    pub at_mrad: [f64; 2],
    /// Most warnings shown at once (GDD 54.8 says three).
    #[schemars(range(min = 1, max = 3))]
    pub max_lines: u32,
}

impl HudLayer {
    /// The element type as written in `type`.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Boresight(_) => "boresight",
            Self::FlightPathMarker(_) => "flight_path_marker",
            Self::PitchLadder(_) => "pitch_ladder",
            Self::HeadingTape(_) => "heading_tape",
            Self::SpeedTape(_) => "speed_tape",
            Self::AltitudeTape(_) => "altitude_tape",
            Self::VsCaret(_) => "vs_caret",
            Self::GMeter(_) => "g_meter",
            Self::AoaBracket(_) => "aoa_bracket",
            Self::BankScale(_) => "bank_scale",
            Self::EngineFuelBlock(_) => "engine_fuel_block",
            Self::ModeLine(_) => "mode_line",
            Self::Warnings(_) => "warnings",
        }
    }

    /// The layer's name.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Boresight(e) => &e.id,
            Self::FlightPathMarker(e) => &e.id,
            Self::PitchLadder(e) => &e.id,
            Self::HeadingTape(e) => &e.id,
            Self::SpeedTape(e) => &e.id,
            Self::AltitudeTape(e) => &e.id,
            Self::VsCaret(e) => &e.id,
            Self::GMeter(e) => &e.id,
            Self::AoaBracket(e) => &e.id,
            Self::BankScale(e) => &e.id,
            Self::EngineFuelBlock(e) => &e.id,
            Self::ModeLine(e) => &e.id,
            Self::Warnings(e) => &e.id,
        }
    }

    /// Where the element is anchored in the HUD plane (mrad), for those that have an anchor.
    #[must_use]
    pub fn anchor_mrad(&self) -> Option<[f64; 2]> {
        match self {
            Self::HeadingTape(e) => Some(e.at_mrad),
            Self::SpeedTape(e) => Some(e.at_mrad),
            Self::AltitudeTape(e) => Some(e.at_mrad),
            Self::VsCaret(e) => Some(e.at_mrad),
            Self::GMeter(e) => Some(e.at_mrad),
            Self::AoaBracket(e) => Some(e.at_mrad),
            Self::EngineFuelBlock(e) => Some(e.at_mrad),
            Self::ModeLine(e) => Some(e.at_mrad),
            Self::Warnings(e) => Some(e.at_mrad),
            Self::Boresight(_)
            | Self::FlightPathMarker(_)
            | Self::PitchLadder(_)
            | Self::BankScale(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Cockpit.

/// `cockpit_layout/1`: the pilot's view from inside (GDD 54.10, A.11), built as simple solids.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CockpitFile {
    /// Format tag, always `cockpit_layout/1`.
    #[schemars(extend("const" = "cockpit_layout/1"))]
    pub schema: String,
    /// `core:cpt/<name>`.
    pub id: String,
    /// The aircraft this cockpit belongs to, `core:ac/<name>`.
    pub aircraft: String,
    /// The design eye in body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub eye_point_m: [f64; 3],
    /// How far the head turns from straight ahead.
    pub head_limits_deg: HeadLimits,
    /// The combiner glass: a flat quadrilateral the HUD is seen through.
    pub hud_glass: HudGlass,
    /// Canopy frame arches, front to back.
    #[schemars(length(max = 8))]
    pub canopy_bows: Vec<CanopyBow>,
    /// The glare shield above the panel.
    pub glare_shield: BoxFile,
    /// The instrument panel.
    pub panel: BoxFile,
    /// Side consoles.
    #[schemars(length(max = 8))]
    pub consoles: Vec<BoxFile>,
}

/// Head travel (GDD A.11); the unit is named by the parent key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HeadLimits {
    /// Turn left or right.
    #[schemars(range(min = 30.0, max = 180.0))]
    pub yaw: f64,
    /// Look up.
    #[schemars(range(min = 10.0, max = 90.0))]
    pub pitch_up: f64,
    /// Look down.
    #[schemars(range(min = 10.0, max = 90.0))]
    pub pitch_down: f64,
}

/// The combiner glass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HudGlass {
    /// The four corners, in order round the edge, body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub corners_m: [[f64; 3]; 4],
}

/// One arch of the canopy frame in a plane of constant x: a half ellipse from the sill on the
/// left, over the top, to the sill on the right.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanopyBow {
    /// Station, body x.
    #[schemars(extend("x-unit" = "m"))]
    pub x_m: f64,
    /// Half the width at the sill.
    #[schemars(extend("x-unit" = "m"), range(min = 0.1, max = 2.0))]
    pub half_width_m: f64,
    /// Height of the sill (body z, down is positive).
    #[schemars(extend("x-unit" = "m"))]
    pub sill_z_m: f64,
    /// Height of the top of the arch (body z; more negative than the sill).
    #[schemars(extend("x-unit" = "m"))]
    pub top_z_m: f64,
    /// Thickness of the frame.
    #[schemars(extend("x-unit" = "m"), range(min = 0.01, max = 0.2))]
    pub thickness_m: f64,
}

/// An axis-aligned box in body axes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoxFile {
    /// Name, unique in the file.
    pub id: String,
    /// The corner with the smallest x, y and z.
    #[schemars(extend("x-unit" = "m"))]
    pub min_m: [f64; 3],
    /// The corner with the largest x, y and z.
    #[schemars(extend("x-unit" = "m"))]
    pub max_m: [f64; 3],
}

// ---------------------------------------------------------------------------------------------
// Exterior model.

/// `model/1`: an aircraft's exterior as a handful of generated solids, with the parts that move
/// bound to the sim's channels (ADR-41). The renderer builds one mesh from it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelFile {
    /// Format tag, always `model/1`.
    #[schemars(extend("const" = "model/1"))]
    pub schema: String,
    /// `core:mdl/<name>`.
    pub id: String,
    /// The aircraft this model belongs to, `core:ac/<name>`.
    pub aircraft: String,
    /// Fuselage cross-sections, nose to tail.
    #[schemars(length(min = 3, max = 24))]
    pub fuselage: Vec<Station>,
    /// Canopy bubble cross-sections, front to back, over the fuselage.
    #[schemars(length(min = 2, max = 12))]
    pub canopy: Vec<Station>,
    /// The engine nozzle at the tail.
    pub nozzle: Nozzle,
    /// Wings, tails and fins: one trapezoid slab each.
    #[schemars(length(min = 1, max = 24))]
    pub surfaces: Vec<Surface>,
    /// Intakes, pylons and other boxes.
    #[schemars(length(max = 24))]
    pub boxes: Vec<ModelBox>,
    /// The landing gear.
    #[schemars(length(max = 8))]
    pub gear: Vec<GearLegModel>,
    /// The parts that move.
    #[schemars(length(max = 16))]
    pub parts: Vec<PartFile>,
}

/// A fuselage or canopy cross-section: an ellipse-like ring at station `x_m`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Station {
    /// Station, body x.
    #[schemars(extend("x-unit" = "m"))]
    pub x_m: f64,
    /// Half the width.
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 3.0))]
    pub half_width_m: f64,
    /// Top of the section (body z; negative is up).
    #[schemars(extend("x-unit" = "m"))]
    pub top_z_m: f64,
    /// Bottom of the section (body z; larger than the top).
    #[schemars(extend("x-unit" = "m"))]
    pub bottom_z_m: f64,
}

/// The nozzle: a short cone behind the last fuselage station that glows with the afterburner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Nozzle {
    /// The `glow` part that lights it.
    pub part: String,
    /// Where the nozzle's front is, body axes.
    #[schemars(extend("x-unit" = "m"))]
    pub at_m: [f64; 3],
    /// Radius at the front.
    #[schemars(extend("x-unit" = "m"), range(min = 0.05, max = 2.0))]
    pub radius_m: f64,
    /// Length.
    #[schemars(extend("x-unit" = "m"), range(min = 0.05, max = 5.0))]
    pub length_m: f64,
}

/// A trapezoid slab: a wing half, a tail half or a fin. Written out per side (an animated surface
/// moves on its own); a static surface may set `mirror` to add its left twin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Surface {
    /// Name, unique in the file.
    pub id: String,
    /// The part that moves it, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    /// Also draw the mirror image across the centre plane (not with `part`).
    #[serde(default)]
    pub mirror: bool,
    /// Leading edge at the root.
    #[schemars(extend("x-unit" = "m"))]
    pub root_le_m: [f64; 3],
    /// Chord at the root.
    #[schemars(extend("x-unit" = "m"), range(min = 0.05, max = 20.0))]
    pub root_chord_m: f64,
    /// Leading edge at the tip.
    #[schemars(extend("x-unit" = "m"))]
    pub tip_le_m: [f64; 3],
    /// Chord at the tip.
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 20.0))]
    pub tip_chord_m: f64,
    /// Thickness at the root.
    #[schemars(extend("x-unit" = "m"), range(min = 0.005, max = 1.0))]
    pub root_thickness_m: f64,
    /// Thickness at the tip.
    #[schemars(extend("x-unit" = "m"), range(min = 0.002, max = 1.0))]
    pub tip_thickness_m: f64,
}

/// A box in the model, optionally on a moving part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ModelBox {
    /// Name, unique in the file.
    pub id: String,
    /// The part that moves it, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part: Option<String>,
    /// The corner with the smallest x, y and z.
    #[schemars(extend("x-unit" = "m"))]
    pub min_m: [f64; 3],
    /// The corner with the largest x, y and z.
    #[schemars(extend("x-unit" = "m"))]
    pub max_m: [f64; 3],
}

/// One landing-gear leg: a strut and a wheel that retract with the gear part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GearLegModel {
    /// The aircraft's gear id (`nose`, `main_l`, ...): the contact point must match its data.
    pub id: String,
    /// The part that retracts it.
    pub part: String,
    /// The wheel's contact point with the strut fully extended (as `geometry.gear[].pos_m`).
    #[schemars(extend("x-unit" = "m"))]
    pub contact_m: [f64; 3],
    /// Where the strut meets the airframe.
    #[schemars(extend("x-unit" = "m"))]
    pub top_m: [f64; 3],
    /// Wheel radius.
    #[schemars(extend("x-unit" = "m"), range(min = 0.05, max = 1.0))]
    pub wheel_radius_m: f64,
    /// Wheel width.
    #[schemars(extend("x-unit" = "m"), range(min = 0.02, max = 0.6))]
    pub wheel_width_m: f64,
}

/// A part of the model that moves or glows with one of the sim's channels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "motion", rename_all = "snake_case")]
pub enum PartFile {
    /// Turns about an axis by `rotate_deg_per_unit` times the channel's value.
    Rotate(RotatePart),
    /// Lights up with the channel's value (the nozzle with the afterburner).
    Glow(GlowPart),
}

/// A rotating part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RotatePart {
    /// Name, unique in the file.
    pub id: String,
    /// One of the model channels.
    pub channel: String,
    /// A point on the hinge line.
    #[schemars(extend("x-unit" = "m"))]
    pub pivot_m: [f64; 3],
    /// The hinge direction (need not be a unit vector).
    pub axis: [f64; 3],
    /// Rotation, in degrees, for each unit of the channel (a sign flips the direction).
    #[schemars(extend("x-unit" = "deg"))]
    pub rotate_deg_per_unit: f64,
}

/// A glowing part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GlowPart {
    /// Name, unique in the file.
    pub id: String,
    /// One of the model channels.
    pub channel: String,
}

impl PartFile {
    /// The part's name.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Rotate(p) => &p.id,
            Self::Glow(p) => &p.id,
        }
    }

    /// The channel it follows.
    #[must_use]
    pub fn channel(&self) -> &str {
        match self {
            Self::Rotate(p) => &p.channel,
            Self::Glow(p) => &p.channel,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Stroke font.

/// `stroke_font/1`: the HUD's text, drawn as strokes (no glyph atlas). Each glyph is polylines on
/// a small grid; the HUD scales the grid to its text height.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FontFile {
    /// Format tag, always `stroke_font/1`.
    #[schemars(extend("const" = "stroke_font/1"))]
    pub schema: String,
    /// `core:fnt/<name>`.
    pub id: String,
    /// Width and height of the glyph grid, in grid units.
    #[schemars(range(min = 1, max = 32))]
    pub cell: [u32; 2],
    /// Distance from one glyph's origin to the next, in grid units.
    #[schemars(range(min = 1, max = 64))]
    pub advance: u32,
    /// The glyphs.
    #[schemars(length(min = 1, max = 256))]
    pub glyphs: Vec<Glyph>,
}

/// One character as strokes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Glyph {
    /// The character (one Unicode scalar).
    pub ch: String,
    /// Polylines of `[x, y]` points in grid units, `y` up from the baseline; a space has none.
    pub strokes: Vec<Vec<[f64; 2]>>,
}

// ---------------------------------------------------------------------------------------------
// Audio profile.

/// `audio_profile/1`: how an aircraft sounds (GDD 56.3, A.12). Everything is synthesised from
/// these numbers; there are no samples. Levels are dB relative to the master bus.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioFile {
    /// Format tag, always `audio_profile/1`.
    #[schemars(extend("const" = "audio_profile/1"))]
    pub schema: String,
    /// `core:aud/<name>`.
    pub id: String,
    /// The aircraft this profile belongs to, `core:ac/<name>`.
    pub aircraft: String,
    /// The engine.
    pub engine: EngineAudio,
    /// Wind, buffet and rolling noise.
    pub aero: AeroAudio,
    /// The cockpit mix.
    pub cockpit: CockpitAudio,
    /// The warning tones.
    pub warnings: WarningTones,
    /// The mix.
    pub mix: MixAudio,
}

/// The engine voice: rotor tones, a hiss and a jet roar, and the afterburner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EngineAudio {
    /// Compressor blades; the blade-pass tone is `rpm / 60 * blade_count`.
    #[schemars(range(min = 4, max = 80))]
    pub blade_count: u32,
    /// Shaft speed at idle.
    #[schemars(extend("x-unit" = "rpm"), range(min = 1000.0, max = 30000.0))]
    pub idle_rpm: f64,
    /// Shaft speed at military power (a design target).
    #[schemars(extend("x-unit" = "rpm"), range(min = 1000.0, max = 30000.0))]
    pub military_rpm: f64,
    /// `[core thrust fraction, rpm]` pairs, rising, from the idle fraction to full military power.
    #[schemars(length(min = 2, max = 16))]
    pub rpm_by_core: Vec<[f64; 2]>,
    /// Levels of the blade-pass tone's harmonics (1st, 2nd, 3rd, ...), dB below the fundamental.
    #[schemars(extend("x-unit" = "dB"), length(min = 1, max = 6))]
    pub harmonic_levels_db: Vec<f64>,
    /// Level of the blade-pass tone at military power.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub blade_level_db: f64,
    /// Level of the once-per-revolution shaft tone at military power.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub shaft_level_db: f64,
    /// Level of the turbine hiss at military power.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub hiss_level_db: f64,
    /// Level of the jet roar at military power.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub jet_level_db: f64,
    /// How much louder the afterburner makes the roar (GDD C.13: +6 dB).
    #[schemars(extend("x-unit" = "dB"), range(min = 0.0, max = 12.0))]
    pub burner_roar_db: f64,
    /// Level of the light-off thump.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub lightoff_thump_db: f64,
    /// Tone of the light-off thump.
    #[schemars(extend("x-unit" = "Hz"), range(min = 20.0, max = 200.0))]
    pub lightoff_thump_hz: f64,
    /// Resonances the roar passes through.
    #[schemars(length(max = 6))]
    pub formants: Vec<Formant>,
    /// How quickly the voice follows a change of thrust.
    #[schemars(extend("x-unit" = "s"), range(min = 0.0, max = 5.0))]
    pub follow_s: f64,
}

/// A resonance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Formant {
    /// Centre frequency.
    #[schemars(extend("x-unit" = "Hz"), range(min = 20.0, max = 8000.0))]
    pub f_hz: f64,
    /// Sharpness.
    #[schemars(range(min = 0.3, max = 20.0))]
    pub q: f64,
    /// Gain at the centre.
    #[schemars(extend("x-unit" = "dB"), range(min = -24.0, max = 24.0))]
    pub gain_db: f64,
}

/// Wind, buffet and rolling noise.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AeroAudio {
    /// Dynamic pressure at which the wind is at `wind_ref_level_db`.
    #[schemars(extend("x-unit" = "Pa"), range(min = 100.0, max = 100_000.0))]
    pub wind_ref_qbar_pa: f64,
    /// Wind level at that pressure.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub wind_ref_level_db: f64,
    /// Rise in wind level for ten times the dynamic pressure.
    #[schemars(extend("x-unit" = "dB"), range(min = 1.0, max = 30.0))]
    pub wind_slope_db: f64,
    /// Centre of the wind's band at low and at high speed.
    #[schemars(extend("x-unit" = "Hz"), range(min = 50.0, max = 8000.0))]
    pub wind_low_hz: f64,
    /// See `wind_low_hz`.
    #[schemars(extend("x-unit" = "Hz"), range(min = 50.0, max = 8000.0))]
    pub wind_high_hz: f64,
    /// Extra wind noise with the airbrake fully out.
    #[schemars(extend("x-unit" = "dB"), range(min = 0.0, max = 12.0))]
    pub airbrake_db: f64,
    /// Buffet starts this far below the stall-warning angle of attack.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 10.0))]
    pub buffet_margin_deg: f64,
    /// Level of the buffet at the stall-warning angle.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub buffet_level_db: f64,
    /// Tone of the buffet's rumble.
    #[schemars(extend("x-unit" = "Hz"), range(min = 5.0, max = 100.0))]
    pub buffet_hz: f64,
    /// Ground speed at which the tyres are at `tyre_ref_level_db`.
    #[schemars(extend("x-unit" = "m/s"), range(min = 5.0, max = 200.0))]
    pub tyre_ref_speed_mps: f64,
    /// Rolling noise at that speed.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub tyre_ref_level_db: f64,
}

/// The cockpit mix.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CockpitAudio {
    /// The canopy cuts sound from outside above this frequency.
    #[schemars(extend("x-unit" = "Hz"), range(min = 200.0, max = 20000.0))]
    pub canopy_lowpass_hz: f64,
    /// ... and lowers it by this much.
    #[schemars(extend("x-unit" = "dB"), range(min = -40.0, max = 0.0))]
    pub canopy_attenuation_db: f64,
    /// Level of the interior hum.
    #[schemars(extend("x-unit" = "dB"), range(min = -60.0, max = 0.0))]
    pub interior_hum_db: f64,
    /// Tone of the interior hum.
    #[schemars(extend("x-unit" = "Hz"), range(min = 20.0, max = 1000.0))]
    pub interior_hum_hz: f64,
}

/// The warning tones (GDD 56.7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WarningTones {
    /// The stall horn: a pulsed tone whose pulse rate rises with angle of attack.
    pub stall_horn: StallHorn,
    /// The over-g tone.
    pub over_g: SteadyTone,
    /// The ground-collision warble.
    pub pull_up: Warble,
    /// The gear warning chime.
    pub gear_chime: Chime,
    /// The master caution chime.
    pub caution_chime: Chime,
}

/// The stall horn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StallHorn {
    /// Tone.
    #[schemars(extend("x-unit" = "Hz"), range(min = 100.0, max = 4000.0))]
    pub freq_hz: f64,
    /// Pulses per second at the warning angle of attack.
    #[schemars(extend("x-unit" = "Hz"), range(min = 0.5, max = 20.0))]
    pub pulse_min_hz: f64,
    /// Pulses per second three degrees past it.
    #[schemars(extend("x-unit" = "Hz"), range(min = 0.5, max = 20.0))]
    pub pulse_max_hz: f64,
    /// Level.
    #[schemars(extend("x-unit" = "dB"), range(min = -40.0, max = 0.0))]
    pub level_db: f64,
}

/// A steady tone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SteadyTone {
    /// Tone.
    #[schemars(extend("x-unit" = "Hz"), range(min = 100.0, max = 4000.0))]
    pub freq_hz: f64,
    /// Level.
    #[schemars(extend("x-unit" = "dB"), range(min = -40.0, max = 0.0))]
    pub level_db: f64,
}

/// A tone that alternates between two pitches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Warble {
    /// The lower pitch.
    #[schemars(extend("x-unit" = "Hz"), range(min = 100.0, max = 4000.0))]
    pub low_hz: f64,
    /// The higher pitch.
    #[schemars(extend("x-unit" = "Hz"), range(min = 100.0, max = 4000.0))]
    pub high_hz: f64,
    /// Alternations per second.
    #[schemars(extend("x-unit" = "Hz"), range(min = 0.5, max = 20.0))]
    pub rate_hz: f64,
    /// Level.
    #[schemars(extend("x-unit" = "dB"), range(min = -40.0, max = 0.0))]
    pub level_db: f64,
}

/// A repeating chime.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Chime {
    /// Tone.
    #[schemars(extend("x-unit" = "Hz"), range(min = 100.0, max = 4000.0))]
    pub freq_hz: f64,
    /// Time from the start of one chime to the next while the condition lasts.
    #[schemars(extend("x-unit" = "s"), range(min = 0.2, max = 10.0))]
    pub repeat_s: f64,
    /// Level.
    #[schemars(extend("x-unit" = "dB"), range(min = -40.0, max = 0.0))]
    pub level_db: f64,
}

/// The mix.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MixAudio {
    /// The aircraft bus is lowered this much while a warning sounds (never the warnings bus).
    #[schemars(extend("x-unit" = "dB"), range(min = -24.0, max = 0.0))]
    pub duck_db: f64,
    /// Overall level offset of the aircraft's voices (GDD A.12 `level_offset_db`).
    #[schemars(extend("x-unit" = "dB"), range(min = -24.0, max = 24.0))]
    pub level_offset_db: f64,
    /// The aircraft's voices in the chase view, relative to the cockpit view. The cockpit hears the
    /// outside through the canopy; the chase view has no canopy, and without this it would be
    /// louder by the canopy's attenuation.
    #[schemars(extend("x-unit" = "dB"), range(min = -24.0, max = 24.0))]
    pub chase_offset_db: f64,
}
