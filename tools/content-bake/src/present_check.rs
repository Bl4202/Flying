//! Checks on one presentation file at a time (ADR-43): what the schema cannot say. The rules that
//! need two files (an aircraft, a font, a cockpit and a HUD) are in [`crate::present`].

use std::collections::BTreeSet;

use simmath::dmath;

use crate::check::{Ctx, check_id, is_slug};
use crate::present_model::{
    AudioFile, CockpitFile, FontFile, HUD_ALTITUDE_CHANNELS, HUD_LOAD_CHANNELS, HUD_SPEED_CHANNELS,
    HudFile, HudLayer, MODEL_CHANNELS, ModelFile, PartFile, Station,
};

/// Every cockpit surface keeps at least this far from the design eye, m: the cockpit camera's
/// near plane is 0.3 m (GDD 44.8), and anything nearer would be clipped away.
pub const MIN_EYE_CLEARANCE_M: f64 = 0.35;
/// The exterior model may use at most this many triangles (owner decision, 2026-11-02).
pub const MAX_MODEL_TRIANGLES: u32 = 8_000;
/// The total field of view of a HUD, per axis, degrees.
const TFOV_DEG: (f64, f64) = (10.0, 60.0);
/// The eye box of a HUD, per axis, millimetres.
const EYE_BOX_MM: (f64, f64) = (20.0, 400.0);
/// The element types every HUD must have.
pub const REQUIRED_HUD_KINDS: &[&str] = &[
    "boresight",
    "flight_path_marker",
    "pitch_ladder",
    "warnings",
];
/// The characters every HUD font must draw: the digits and capitals, and the signs the HUD's
/// numbers and texts use (`α` for the angle-of-attack limit, `°` for degrees).
pub const REQUIRED_FONT_CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ .-+/%:°α";

// The triangle counts of the renderer's mesh builder (`model-mesh.ts`): the contract that makes
// the budget checkable here. The builder's own test asserts it never exceeds this estimate.
/// Sides of the fuselage rings.
pub const FUSELAGE_SIDES: u32 = 16;
/// Sides of the canopy rings.
pub const CANOPY_SIDES: u32 = 12;
/// Sides of the nozzle cone.
pub const NOZZLE_SIDES: u32 = 12;
/// Triangles of a slab or a box (a hexahedron).
pub const SLAB_TRIANGLES: u32 = 12;
/// Triangles of one gear leg: a box strut and a 12-sided wheel.
pub const GEAR_LEG_TRIANGLES: u32 = SLAB_TRIANGLES + 48;

type V3 = [f64; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len(a: V3) -> f64 {
    dmath::sqrt(dot(a, a))
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Checks an id of the pack's namespace and the given kind.
fn ident(c: &mut Ctx<'_>, ptr: &str, id: &str, pack_id: &str, kind: &str, example: &str) {
    if let Err(hint) = check_id(id, pack_id, kind) {
        c.err(
            ptr,
            format!("`{id}` is not a valid id here"),
            format!("{hint}; for example `{example}`"),
        );
    }
}

fn tag(c: &mut Ctx<'_>, found: &str, want: &str) {
    if found != want {
        c.err(
            "/schema",
            format!("`{found}` is not the tag of this kind of file"),
            format!("set it to `{want}`"),
        );
    }
}

fn check_name(c: &mut Ctx<'_>, ptr: &str, name: &str, seen: &mut BTreeSet<String>) {
    if !is_slug(name) {
        c.err(
            ptr,
            format!("`{name}` is not a valid name"),
            "use lower-case letters, digits and `_`, starting with a letter",
        );
    } else if !seen.insert(name.to_owned()) {
        c.err(
            ptr,
            format!("the name `{name}` is used twice"),
            "names are unique within a file; rename one",
        );
    }
}

// ---------------------------------------------------------------------------------------------
// HUD.

fn valid_colour(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Half the field of view of an axis as a tangent-plane extent, mrad.
fn half_extent_mrad(tfov_deg: f64) -> f64 {
    1000.0 * dmath::tan(tfov_deg.to_radians() / 2.0)
}

fn is_multiple(value: f64, step: f64) -> bool {
    let q = value / step;
    (q - q.round()).abs() < 1.0e-9
}

/// Rules of a HUD layout.
pub(crate) fn check_hud(c: &mut Ctx<'_>, h: &HudFile, pack_id: &str) {
    tag(c, &h.schema, "hud/1");
    ident(c, "/id", &h.id, pack_id, "hud", "core:hud/kestrel");
    ident(
        c,
        "/aircraft",
        &h.aircraft,
        pack_id,
        "ac",
        "core:ac/kestrel",
    );
    ident(c, "/font", &h.font, pack_id, "fnt", "core:fnt/hudstroke");
    if !valid_colour(&h.colour) {
        c.err(
            "/colour",
            format!("`{}` is not a colour", h.colour),
            "write it as #rrggbb, for example #35ff6a",
        );
    }
    for (i, v) in h.tfov_deg.iter().enumerate() {
        if !(TFOV_DEG.0..=TFOV_DEG.1).contains(v) {
            c.err(
                &format!("/tfov_deg/{i}"),
                format!("a field of view of {v} degrees is not a head-up display's"),
                format!("use between {} and {} degrees", TFOV_DEG.0, TFOV_DEG.1),
            );
        }
    }
    for (i, v) in h.eye_box_mm.iter().enumerate() {
        if !(EYE_BOX_MM.0..=EYE_BOX_MM.1).contains(v) {
            c.err(
                &format!("/eye_box_mm/{i}"),
                format!("an eye box of {v} mm is not workable"),
                format!("use between {} and {} mm", EYE_BOX_MM.0, EYE_BOX_MM.1),
            );
        }
    }
    check_layers(c, h);
}

fn check_layers(c: &mut Ctx<'_>, h: &HudFile) {
    let mut names = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    let extent = (
        half_extent_mrad(h.tfov_deg[0]),
        half_extent_mrad(h.tfov_deg[1]),
    );
    for (i, layer) in h.layers.iter().enumerate() {
        let at = format!("/layers/{i}");
        check_name(c, &format!("{at}/id"), layer.id(), &mut names);
        if !kinds.insert(layer.kind()) {
            c.err(
                &format!("{at}/type"),
                format!("there are two `{}` layers", layer.kind()),
                "a HUD has at most one of each element; remove one",
            );
        }
        check_layer(c, layer, &at, extent);
    }
    for want in REQUIRED_HUD_KINDS {
        if !kinds.contains(want) {
            c.err(
                "/layers",
                format!("the HUD has no `{want}` element"),
                format!("add a layer of type `{want}`"),
            );
        }
    }
}

fn channel(c: &mut Ctx<'_>, at: &str, value: &str, allowed: &[&str]) {
    if !allowed.contains(&value) {
        c.err(
            &format!("{at}/channel"),
            format!("`{value}` is not a channel this element can show"),
            format!("use one of: {}", allowed.join(", ")),
        );
    }
}

/// The element's anchor, and whether its extent (`half_u`, `half_v`, mrad) stays in the field.
fn within(c: &mut Ctx<'_>, at: &str, centre: [f64; 2], half: (f64, f64), field: (f64, f64)) {
    let (u, v) = (centre[0].abs() + half.0, centre[1].abs() + half.1);
    if u > field.0 || v > field.1 {
        c.err(
            &format!("{at}/at_mrad"),
            format!(
                "the element reaches {u:.0} by {v:.0} mrad from the boresight, outside the field of view ({:.0} by {:.0})",
                field.0, field.1
            ),
            "move it towards the centre, or shorten it",
        );
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per element type; splitting hides the registry"
)]
fn check_layer(c: &mut Ctx<'_>, layer: &HudLayer, at: &str, field: (f64, f64)) {
    match layer {
        HudLayer::Boresight(e) => {
            ensure_inside(c, at, "size_mrad", e.size_mrad, field.0.min(field.1));
        }
        HudLayer::FlightPathMarker(e) => {
            ensure_inside(c, at, "size_mrad", e.size_mrad, field.0.min(field.1));
        }
        HudLayer::PitchLadder(e) => {
            if !is_multiple(e.label_step_deg, e.step_deg) {
                c.err(
                    &format!("{at}/label_step_deg"),
                    format!(
                        "{} is not a multiple of the bar spacing {}",
                        e.label_step_deg, e.step_deg
                    ),
                    "numbered bars must be bars: use a multiple of step_deg",
                );
            }
            ensure_inside(c, at, "half_width_mrad", e.half_width_mrad, field.0);
        }
        HudLayer::HeadingTape(e) => {
            within(c, at, e.at_mrad, (e.span_mrad / 2.0, 0.0), field);
            tape_steps(c, at, e.window_deg, e.tick_deg, e.label_deg, "deg");
        }
        HudLayer::SpeedTape(e) => {
            channel(c, at, &e.channel, HUD_SPEED_CHANNELS);
            within(c, at, e.at_mrad, (0.0, e.span_mrad / 2.0), field);
            tape_steps(c, at, e.window_kn, e.tick_kn, e.label_kn, "kn");
        }
        HudLayer::AltitudeTape(e) => {
            channel(c, at, &e.channel, HUD_ALTITUDE_CHANNELS);
            within(c, at, e.at_mrad, (0.0, e.span_mrad / 2.0), field);
            tape_steps(c, at, e.window_ft, e.tick_ft, e.label_ft, "ft");
        }
        HudLayer::VsCaret(e) => within(c, at, e.at_mrad, (0.0, e.span_mrad / 2.0), field),
        HudLayer::GMeter(e) => {
            channel(c, at, &e.channel, HUD_LOAD_CHANNELS);
            within(c, at, e.at_mrad, (0.0, 0.0), field);
            if e.amber_pct >= e.red_pct {
                c.err(
                    &format!("{at}/amber_pct"),
                    format!(
                        "amber ({}) must come before red ({})",
                        e.amber_pct, e.red_pct
                    ),
                    "lower amber_pct or raise red_pct",
                );
            }
        }
        HudLayer::AoaBracket(e) => {
            within(c, at, e.at_mrad, (0.0, 0.0), field);
        }
        HudLayer::BankScale(e) => {
            ensure_inside(c, at, "radius_mrad", e.radius_mrad, field.0.min(field.1));
        }
        HudLayer::EngineFuelBlock(e) => within(c, at, e.at_mrad, (0.0, 0.0), field),
        HudLayer::ModeLine(e) => within(c, at, e.at_mrad, (0.0, 0.0), field),
        HudLayer::Warnings(e) => within(c, at, e.at_mrad, (0.0, 0.0), field),
        HudLayer::GunReticle(e) => {
            ensure_inside(c, at, "ring_mrad", e.ring_mrad, field.0.min(field.1));
            if e.pipper_mrad >= e.ring_mrad {
                c.err(
                    &format!("{at}/pipper_mrad"),
                    format!(
                        "the pipper ({} mrad) is not smaller than the ring ({} mrad)",
                        e.pipper_mrad, e.ring_mrad
                    ),
                    "make the pipper smaller than the ring, or the ring larger",
                );
            }
            if e.shoot_m > e.ring_max_m {
                c.err(
                    &format!("{at}/shoot_m"),
                    format!(
                        "the shoot range ({} m) is beyond the range the ring is full at ({} m)",
                        e.shoot_m, e.ring_max_m
                    ),
                    "lower shoot_m, or raise ring_max_m",
                );
            }
        }
        HudLayer::TargetBox(e) => {
            ensure_inside(
                c,
                at,
                "locator_radius_mrad",
                e.locator_radius_mrad,
                field.0.min(field.1),
            );
            if e.size_mrad >= 2.0 * e.locator_radius_mrad {
                c.err(
                    &format!("{at}/size_mrad"),
                    format!(
                        "the box ({} mrad) is as wide as the locator circle ({} mrad)",
                        e.size_mrad,
                        2.0 * e.locator_radius_mrad
                    ),
                    "make the box smaller, or the locator circle larger",
                );
            }
        }
        HudLayer::Seeker(e) => {
            ensure_inside(c, at, "circle_mrad", e.circle_mrad, field.0.min(field.1));
            if e.diamond_mrad >= e.circle_mrad {
                c.err(
                    &format!("{at}/diamond_mrad"),
                    format!(
                        "the diamond ({} mrad) is not smaller than the circle ({} mrad)",
                        e.diamond_mrad, e.circle_mrad
                    ),
                    "make the diamond smaller than the seeker circle",
                );
            }
        }
        HudLayer::WeaponStatus(e) => within(c, at, e.at_mrad, (0.0, 0.0), field),
        HudLayer::KillCue(e) => within(c, at, e.at_mrad, (0.0, 0.0), field),
    }
}

fn ensure_inside(c: &mut Ctx<'_>, at: &str, key: &str, size: f64, limit: f64) {
    if size > limit {
        c.err(
            &format!("{at}/{key}"),
            format!("{size} mrad is larger than the field of view allows ({limit:.0} mrad)"),
            "use a smaller size",
        );
    }
}

fn tape_steps(c: &mut Ctx<'_>, at: &str, window: f64, tick: f64, label: f64, unit: &str) {
    if tick >= window {
        c.err(
            &format!("{at}/tick_{unit}"),
            format!("a tick spacing of {tick} {unit} does not fit a window of {window} {unit}"),
            "use a spacing well under the window",
        );
    }
    if !is_multiple(label, tick) {
        c.err(
            &format!("{at}/label_{unit}"),
            format!("{label} {unit} is not a multiple of the tick spacing {tick}"),
            "numbers sit on ticks: use a multiple of the tick spacing",
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Cockpit.

/// Distance from a point to an axis-aligned box (0 inside).
fn distance_to_box(p: V3, min: V3, max: V3) -> f64 {
    let d = [
        (min[0] - p[0]).max(0.0).max(p[0] - max[0]),
        (min[1] - p[1]).max(0.0).max(p[1] - max[1]),
        (min[2] - p[2]).max(0.0).max(p[2] - max[2]),
    ];
    len(d)
}

/// Points along a canopy arch, in body axes.
#[must_use]
pub fn bow_points(x: f64, half_width: f64, sill_z: f64, top_z: f64, n: u32) -> Vec<V3> {
    (0..=n)
        .map(|k| {
            let t = std::f64::consts::PI * f64::from(k) / f64::from(n);
            [
                x,
                half_width * dmath::cos(t),
                sill_z + (top_z - sill_z) * dmath::sin(t),
            ]
        })
        .collect()
}

/// Rules of a cockpit.
pub(crate) fn check_cockpit(c: &mut Ctx<'_>, k: &CockpitFile, pack_id: &str) {
    tag(c, &k.schema, "cockpit_layout/1");
    ident(c, "/id", &k.id, pack_id, "cpt", "core:cpt/kestrel");
    ident(
        c,
        "/aircraft",
        &k.aircraft,
        pack_id,
        "ac",
        "core:ac/kestrel",
    );
    let eye = k.eye_point_m;
    let mut names = BTreeSet::new();
    let boxes = std::iter::once(("/glare_shield".to_owned(), &k.glare_shield))
        .chain(std::iter::once(("/panel".to_owned(), &k.panel)))
        .chain(
            k.consoles
                .iter()
                .enumerate()
                .map(|(i, b)| (format!("/consoles/{i}"), b)),
        );
    for (at, b) in boxes {
        check_name(c, &format!("{at}/id"), &b.id, &mut names);
        if (0..3).any(|a| b.min_m[a] >= b.max_m[a]) {
            c.err(
                &format!("{at}/max_m"),
                "the box has no size on at least one axis",
                "min_m must be smaller than max_m on x, y and z",
            );
            continue;
        }
        let d = distance_to_box(eye, b.min_m, b.max_m);
        if d < MIN_EYE_CLEARANCE_M {
            c.err(
                &format!("{at}/min_m"),
                format!(
                    "the box is {d:.2} m from the design eye; the camera's near plane would cut it"
                ),
                format!("keep every surface at least {MIN_EYE_CLEARANCE_M} m from the eye"),
            );
        }
    }
    if k.panel.min_m[2] <= eye[2] {
        c.err(
            "/panel/min_m",
            "the top of the panel is not below the eye",
            "the eye (eye_point_m) must be above the panel: its z is smaller (z is down)",
        );
    }
    for (i, b) in k.canopy_bows.iter().enumerate() {
        if b.top_z_m >= b.sill_z_m {
            c.err(
                &format!("/canopy_bows/{i}/top_z_m"),
                "the arch does not rise above its sill",
                "top_z_m must be smaller than sill_z_m (z is down)",
            );
            continue;
        }
        let nearest = bow_points(b.x_m, b.half_width_m, b.sill_z_m, b.top_z_m, 64)
            .into_iter()
            .map(|p| len(sub(p, eye)))
            .fold(f64::MAX, f64::min);
        if nearest < MIN_EYE_CLEARANCE_M {
            c.err(
                &format!("/canopy_bows/{i}/x_m"),
                format!("the arch passes {nearest:.2} m from the design eye"),
                format!("keep every surface at least {MIN_EYE_CLEARANCE_M} m from the eye"),
            );
        }
    }
    check_glass(c, k);
}

fn check_glass(c: &mut Ctx<'_>, k: &CockpitFile) {
    let g = k.hud_glass.corners_m;
    let eye = k.eye_point_m;
    let a = sub(g[1], g[0]);
    let b = sub(g[3], g[0]);
    let fourth = sub(
        g[2],
        [
            g[0][0] + a[0] + b[0],
            g[0][1] + a[1] + b[1],
            g[0][2] + a[2] + b[2],
        ],
    );
    if len(fourth) > 1.0e-3 || dot(a, b).abs() > 1.0e-3 * len(a) * len(b) {
        c.err(
            "/hud_glass/corners_m",
            "the four corners are not a rectangle",
            "list the corners in order round the edge, with right angles",
        );
        return;
    }
    // The distance from the eye to the glass plane.
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let nl = len(n);
    if nl == 0.0 {
        c.err(
            "/hud_glass/corners_m",
            "the glass has no area",
            "spread the corners",
        );
        return;
    }
    let dist = dot(sub(g[0], eye), n).abs() / nl;
    if dist < MIN_EYE_CLEARANCE_M {
        c.err(
            "/hud_glass/corners_m",
            format!("the glass is {dist:.2} m from the design eye"),
            format!("keep every surface at least {MIN_EYE_CLEARANCE_M} m from the eye"),
        );
    }
}

/// True if the combiner glass covers a field of view `tfov_deg` seen from the design eye: the
/// four corner rays of the field, in the tangent plane `(1, u, -v)`, pass through the glass
/// rectangle (within half a millimetre). `None` if the glass is degenerate.
#[must_use]
pub fn glass_covers(k: &CockpitFile, tfov_deg: [f64; 2]) -> Option<bool> {
    let g = k.hud_glass.corners_m;
    let eye = k.eye_point_m;
    let a = sub(g[1], g[0]);
    let b = sub(g[3], g[0]);
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let (aa, bb) = (dot(a, a), dot(b, b));
    if aa == 0.0 || bb == 0.0 {
        return None;
    }
    let (tu, tv) = (
        dmath::tan(tfov_deg[0].to_radians() / 2.0),
        dmath::tan(tfov_deg[1].to_radians() / 2.0),
    );
    let tol = 5.0e-4;
    let mut covered = true;
    for (su, sv) in [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        // The ray from the eye towards body direction (1, u, -v).
        let d = [1.0, su * tu, -sv * tv];
        let denom = dot(d, n);
        if denom.abs() < 1.0e-12 {
            return None;
        }
        let t = dot(sub(g[0], eye), n) / denom;
        if t <= 0.0 {
            return Some(false);
        }
        let hit = [eye[0] + d[0] * t, eye[1] + d[1] * t, eye[2] + d[2] * t];
        let rel = sub(hit, g[0]);
        let (s, r) = (dot(rel, a) / dmath::sqrt(aa), dot(rel, b) / dmath::sqrt(bb));
        let (wa, wb) = (dmath::sqrt(aa), dmath::sqrt(bb));
        covered &= s >= -tol && s <= wa + tol && r >= -tol && r <= wb + tol;
    }
    Some(covered)
}

// ---------------------------------------------------------------------------------------------
// Exterior model.

/// The model's triangle count by the builder's rules.
#[must_use]
pub fn model_triangles(m: &ModelFile) -> u32 {
    let ring = |sides: u32, stations: usize| {
        let bands = stations.saturating_sub(1) as u32;
        2 * sides * bands + 2 * sides
    };
    let surfaces: u32 = m
        .surfaces
        .iter()
        .map(|s| SLAB_TRIANGLES * if s.mirror { 2 } else { 1 })
        .sum();
    ring(FUSELAGE_SIDES, m.fuselage.len())
        + ring(CANOPY_SIDES, m.canopy.len())
        + 3 * NOZZLE_SIDES
        + surfaces
        + SLAB_TRIANGLES * m.boxes.len() as u32
        + GEAR_LEG_TRIANGLES * m.gear.len() as u32
}

fn check_stations(c: &mut Ctx<'_>, key: &str, st: &[Station]) {
    for (i, s) in st.iter().enumerate() {
        if s.top_z_m >= s.bottom_z_m {
            c.err(
                &format!("/{key}/{i}/top_z_m"),
                "the section has no height",
                "top_z_m must be smaller than bottom_z_m (z is down)",
            );
        }
        if i > 0 && s.x_m >= st[i - 1].x_m {
            c.err(
                &format!("/{key}/{i}/x_m"),
                "the stations do not run from front to back",
                "list stations with decreasing x_m (x is forward)",
            );
        }
    }
}

/// The parts of a model and which of them something refers to.
struct Parts {
    /// `(id, is_glow)` of each.
    known: Vec<(String, bool)>,
    used: BTreeSet<String>,
}

impl Parts {
    /// Records a reference to `part` at `ptr`, which must name a part of the wanted kind.
    fn reference(&mut self, c: &mut Ctx<'_>, ptr: &str, part: &str, want_glow: bool) {
        self.used.insert(part.to_owned());
        match self.known.iter().find(|(id, _)| id == part) {
            None => c.err(
                ptr,
                format!("there is no part `{part}`"),
                "add it to `parts`, or fix the name",
            ),
            Some((_, glow)) if *glow != want_glow => c.err(
                ptr,
                format!("`{part}` is the wrong kind of part here"),
                if want_glow {
                    "use a `glow` part"
                } else {
                    "use a `rotate` part"
                },
            ),
            Some(_) => {}
        }
    }
}

/// Rules of an exterior model.
pub(crate) fn check_model(c: &mut Ctx<'_>, m: &ModelFile, pack_id: &str) {
    tag(c, &m.schema, "model/1");
    ident(c, "/id", &m.id, pack_id, "mdl", "core:mdl/kestrel");
    ident(
        c,
        "/aircraft",
        &m.aircraft,
        pack_id,
        "ac",
        "core:ac/kestrel",
    );
    check_stations(c, "fuselage", &m.fuselage);
    check_stations(c, "canopy", &m.canopy);
    if let (Some(f0), Some(fl), Some(c0), Some(cl)) = (
        m.fuselage.first(),
        m.fuselage.last(),
        m.canopy.first(),
        m.canopy.last(),
    ) && (c0.x_m > f0.x_m || cl.x_m < fl.x_m)
    {
        c.err(
            "/canopy",
            "the canopy sticks out past the fuselage",
            "keep its first and last stations between the fuselage's",
        );
    }
    let mut parts = Parts {
        known: check_parts(c, &m.parts),
        used: BTreeSet::new(),
    };
    parts.reference(c, "/nozzle/part", &m.nozzle.part, true);
    let mut names = BTreeSet::new();
    check_surfaces(c, m, &mut parts, &mut names);
    check_boxes(c, m, &mut parts, &mut names);
    check_gear(c, m, &mut parts, &mut names);
    for p in &m.parts {
        if !parts.used.contains(p.id()) {
            c.err(
                "/parts",
                format!("the part `{}` moves nothing", p.id()),
                "reference it from a surface, box, gear leg or the nozzle, or remove it",
            );
        }
    }
    let tris = model_triangles(m);
    if tris > MAX_MODEL_TRIANGLES {
        c.err(
            "",
            format!("the model needs about {tris} triangles; the budget is {MAX_MODEL_TRIANGLES}"),
            "remove stations, surfaces or boxes",
        );
    }
}

fn check_surfaces(c: &mut Ctx<'_>, m: &ModelFile, parts: &mut Parts, names: &mut BTreeSet<String>) {
    for (i, s) in m.surfaces.iter().enumerate() {
        let at = format!("/surfaces/{i}");
        check_name(c, &format!("{at}/id"), &s.id, names);
        if let Some(p) = &s.part {
            parts.reference(c, &format!("{at}/part"), p, false);
            if s.mirror {
                c.err(
                    &format!("{at}/mirror"),
                    "a surface that moves cannot also be mirrored",
                    "write the left and right surfaces separately",
                );
            }
        }
        if (0..3).all(|a| (s.root_le_m[a] - s.tip_le_m[a]).abs() < 1.0e-6) {
            c.err(
                &format!("{at}/tip_le_m"),
                "the root and the tip are at the same place",
                "move the tip away from the root",
            );
        }
    }
}

fn check_boxes(c: &mut Ctx<'_>, m: &ModelFile, parts: &mut Parts, names: &mut BTreeSet<String>) {
    for (i, b) in m.boxes.iter().enumerate() {
        let at = format!("/boxes/{i}");
        check_name(c, &format!("{at}/id"), &b.id, names);
        if (0..3).any(|a| b.min_m[a] >= b.max_m[a]) {
            c.err(
                &format!("{at}/max_m"),
                "the box has no size on at least one axis",
                "min_m must be smaller than max_m on x, y and z",
            );
        }
        if let Some(p) = &b.part {
            parts.reference(c, &format!("{at}/part"), p, false);
        }
    }
}

fn check_gear(c: &mut Ctx<'_>, m: &ModelFile, parts: &mut Parts, names: &mut BTreeSet<String>) {
    for (i, g) in m.gear.iter().enumerate() {
        let at = format!("/gear/{i}");
        check_name(c, &format!("{at}/id"), &g.id, names);
        parts.reference(c, &format!("{at}/part"), &g.part, false);
        if g.top_m[2] >= g.contact_m[2] {
            c.err(
                &format!("{at}/top_m"),
                "the strut's top is not above the wheel",
                "top_m.z must be smaller than contact_m.z (z is down)",
            );
        }
    }
}

/// Checks the parts and returns `(id, is_glow)` of each.
fn check_parts(c: &mut Ctx<'_>, parts: &[PartFile]) -> Vec<(String, bool)> {
    let mut names = BTreeSet::new();
    let mut out = Vec::new();
    for (i, p) in parts.iter().enumerate() {
        let at = format!("/parts/{i}");
        check_name(c, &format!("{at}/id"), p.id(), &mut names);
        if !MODEL_CHANNELS.contains(&p.channel()) {
            c.err(
                &format!("{at}/channel"),
                format!("`{}` is not a channel the sim publishes", p.channel()),
                format!("use one of: {}", MODEL_CHANNELS.join(", ")),
            );
        }
        match p {
            PartFile::Rotate(r) => {
                if len(r.axis) == 0.0 {
                    c.err(
                        &format!("{at}/axis"),
                        "the hinge axis has no direction",
                        "give a non-zero direction such as [0, 1, 0]",
                    );
                }
            }
            PartFile::Glow(g) => {
                if g.channel != "afterburner" {
                    c.err(
                        &format!("{at}/channel"),
                        "only the afterburner level lights a part",
                        "use the `afterburner` channel",
                    );
                }
            }
        }
        out.push((p.id().to_owned(), matches!(p, PartFile::Glow(_))));
    }
    out
}

/// The section of a station list at body `x` (linear between stations): `(half_width, top_z,
/// bottom_z)`, or `None` outside the stations.
#[must_use]
pub fn section_at(stations: &[Station], x: f64) -> Option<(f64, f64, f64)> {
    stations.windows(2).find_map(|w| {
        let (a, b) = (&w[0], &w[1]);
        (x <= a.x_m && x >= b.x_m).then(|| {
            let t = if (a.x_m - b.x_m).abs() < f64::EPSILON {
                0.0
            } else {
                (a.x_m - x) / (a.x_m - b.x_m)
            };
            (
                lerp(a.half_width_m, b.half_width_m, t),
                lerp(a.top_z_m, b.top_z_m, t),
                lerp(a.bottom_z_m, b.bottom_z_m, t),
            )
        })
    })
}

/// True if body point `p` is inside the fuselage or the canopy bubble.
#[must_use]
pub fn inside_cabin(m: &ModelFile, p: V3) -> bool {
    [&m.fuselage, &m.canopy].into_iter().any(|stations| {
        section_at(stations, p[0])
            .is_some_and(|(half, top, bottom)| p[1].abs() <= half && p[2] >= top && p[2] <= bottom)
    })
}

// ---------------------------------------------------------------------------------------------
// Font.

/// Rules of a stroke font.
#[allow(
    clippy::float_cmp,
    reason = "authored values are exactly representable (the loader checks), so equal means equal"
)]
pub(crate) fn check_font(c: &mut Ctx<'_>, f: &FontFile, pack_id: &str) {
    tag(c, &f.schema, "stroke_font/1");
    ident(c, "/id", &f.id, pack_id, "fnt", "core:fnt/hudstroke");
    if f.advance < f.cell[0] {
        c.err(
            "/advance",
            "the advance is narrower than a glyph",
            "make it at least the cell width, so glyphs do not overlap",
        );
    }
    let (w, h) = (f64::from(f.cell[0]), f64::from(f.cell[1]));
    let mut seen = BTreeSet::new();
    for (i, g) in f.glyphs.iter().enumerate() {
        let at = format!("/glyphs/{i}");
        if g.ch.chars().count() != 1 {
            c.err(
                &format!("{at}/ch"),
                format!("`{}` is not one character", g.ch),
                "write exactly one character per glyph",
            );
            continue;
        }
        if !seen.insert(g.ch.clone()) {
            c.err(
                &format!("{at}/ch"),
                format!("the glyph `{}` is defined twice", g.ch),
                "remove one",
            );
        }
        if g.ch == " " {
            if !g.strokes.is_empty() {
                c.err(
                    &format!("{at}/strokes"),
                    "the space has strokes",
                    "leave them out",
                );
            }
        } else if g.strokes.is_empty() {
            c.err(
                &format!("{at}/strokes"),
                format!("the glyph `{}` draws nothing", g.ch),
                "add at least one stroke",
            );
        }
        for (j, stroke) in g.strokes.iter().enumerate() {
            let sp = format!("{at}/strokes/{j}");
            if stroke.len() < 2 {
                c.err(
                    &sp,
                    "a stroke needs at least two points",
                    "add a point or remove the stroke",
                );
            }
            for (k, p) in stroke.iter().enumerate() {
                if !(0.0..=w).contains(&p[0]) || !(0.0..=h).contains(&p[1]) {
                    c.err(
                        &format!("{sp}/{k}"),
                        format!(
                            "the point ({}, {}) is outside the {w} by {h} cell",
                            p[0], p[1]
                        ),
                        "keep points within 0..width and 0..height",
                    );
                }
                if k > 0 && stroke[k - 1] == *p {
                    c.err(
                        &format!("{sp}/{k}"),
                        "two points in a row are the same",
                        "remove one",
                    );
                }
            }
        }
    }
    for ch in REQUIRED_FONT_CHARS.chars() {
        if !seen.contains(&ch.to_string()) {
            c.err(
                "/glyphs",
                format!("the font has no glyph for `{ch}`"),
                format!("the HUD draws `{ch}`; add a glyph for it"),
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Audio.

/// Rules of an audio profile.
pub(crate) fn check_audio(c: &mut Ctx<'_>, a: &AudioFile, pack_id: &str) {
    tag(c, &a.schema, "audio_profile/1");
    ident(c, "/id", &a.id, pack_id, "aud", "core:aud/kestrel");
    ident(
        c,
        "/aircraft",
        &a.aircraft,
        pack_id,
        "ac",
        "core:ac/kestrel",
    );
    let e = &a.engine;
    if e.idle_rpm >= e.military_rpm {
        c.err(
            "/engine/military_rpm",
            format!(
                "military speed {} is not above idle {}",
                e.military_rpm, e.idle_rpm
            ),
            "raise military_rpm or lower idle_rpm",
        );
    }
    check_rpm_curve(c, a);
    if e.harmonic_levels_db.first() != Some(&0.0) {
        c.err(
            "/engine/harmonic_levels_db/0",
            "the first level is the fundamental and must be 0",
            "write 0 first, then the harmonics in dB below it",
        );
    }
    for (i, v) in e.harmonic_levels_db.iter().enumerate() {
        if *v > 0.0 {
            c.err(
                &format!("/engine/harmonic_levels_db/{i}"),
                "a harmonic louder than the fundamental",
                "write levels as dB below the fundamental (0 or negative)",
            );
        }
    }
    if !(5.0..=7.0).contains(&e.burner_roar_db) {
        c.err(
            "/engine/burner_roar_db",
            format!(
                "{} dB is not the afterburner's +6 dB roar (GDD C.13)",
                e.burner_roar_db
            ),
            "use a value between 5 and 7 dB (the gate measures 6 +-1 dB)",
        );
    }
    let aero = &a.aero;
    if aero.wind_low_hz >= aero.wind_high_hz {
        c.err(
            "/aero/wind_high_hz",
            "the wind's band does not rise with speed",
            "make wind_high_hz larger than wind_low_hz",
        );
    }
    let w = &a.warnings;
    if w.stall_horn.pulse_min_hz >= w.stall_horn.pulse_max_hz {
        c.err(
            "/warnings/stall_horn/pulse_max_hz",
            "the stall horn does not speed up",
            "make pulse_max_hz larger than pulse_min_hz",
        );
    }
    if w.pull_up.low_hz >= w.pull_up.high_hz {
        c.err(
            "/warnings/pull_up/high_hz",
            "the warble's two pitches are not low and high",
            "make high_hz larger than low_hz",
        );
    }
    if let Some(w) = &a.weapons
        && w.launch.low_hz >= w.launch.high_hz
    {
        c.err(
            "/weapons/launch/high_hz",
            "the launch whoosh does not sweep down",
            "make high_hz larger than low_hz",
        );
    }
}

#[allow(
    clippy::float_cmp,
    reason = "authored values are exactly representable (the loader checks), so equal means equal"
)]
fn check_rpm_curve(c: &mut Ctx<'_>, a: &AudioFile) {
    let e = &a.engine;
    let curve = &e.rpm_by_core;
    for (i, p) in curve.iter().enumerate() {
        if i > 0 && (p[0] <= curve[i - 1][0] || p[1] <= curve[i - 1][1]) {
            c.err(
                &format!("/engine/rpm_by_core/{i}"),
                "the curve does not rise",
                "list points with rising thrust fraction and rising rpm",
            );
        }
    }
    if let (Some(first), Some(last)) = (curve.first(), curve.last()) {
        if first[0] != 0.0 {
            c.err(
                "/engine/rpm_by_core/0",
                "the curve does not start at zero thrust",
                "start at [0, windmill rpm]: a stopped engine still turns a little",
            );
        }
        if last[0] != 1.0 || last[1] != e.military_rpm {
            c.err(
                &format!("/engine/rpm_by_core/{}", curve.len() - 1),
                "the curve does not end at military power",
                "end at [1, military_rpm]",
            );
        }
    }
    if !curve.iter().any(|p| p[1] == e.idle_rpm) {
        c.err(
            "/engine/rpm_by_core",
            "no point of the curve is the idle speed",
            "add the point where the engine idles, with rpm equal to idle_rpm",
        );
    }
}
