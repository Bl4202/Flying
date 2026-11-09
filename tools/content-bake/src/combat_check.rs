//! Loading and validating the combat data: `weapon/1`, `ai_skill/1` and the additions to
//! `aircraft/1` (ADR-46, ADR-47). Same order as [`crate::check`]: syntax, structure, the numeric
//! ranges of the schema, `f32` exactness, then the physics and the references.

use std::collections::{BTreeMap, BTreeSet};

use crate::check::{Ctx, baker_bug, check_id, check_locale_key, check_names, increasing, load};
use crate::combat_model::{
    GunFile, MissileFile, SKILL_SCHEMA, SkillFile, TACTICS, WEAPON_SCHEMA, WeaponFile,
};
use crate::diag::Diagnostic;
use crate::model::{AircraftFile, CurveFile};
use content::{SkillDef, WeaponClass, WeaponDef};

/// The name of a curve's only axis in the combat data.
const MACH_AXIS: &str = "mach";
/// How far (m) beyond a capsule's radius an airframe contact point may lie from it (ADR-46).
const CONTACT_SLACK_M: f64 = 0.5;

/// A validated weapon file and its runtime form.
#[derive(Debug)]
pub struct LoadedWeapon {
    /// The authoring form.
    pub file: WeaponFile,
    /// The runtime form.
    pub def: WeaponDef,
}

/// Loads and validates one weapon file.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_weapon(
    file: &str,
    text: &str,
    pack_id: &str,
    locale: &BTreeMap<String, String>,
) -> Result<LoadedWeapon, Vec<Diagnostic>> {
    let w = load(file, text, &crate::schema::weapon_schema(), |c, w| {
        check_weapon(c, w, pack_id, locale);
    })?;
    let def = w.to_def().ok_or_else(|| baker_bug(file))?;
    Ok(LoadedWeapon { file: w, def })
}

/// A validated skill file and its runtime form.
#[derive(Debug)]
pub struct LoadedSkill {
    /// The authoring form.
    pub file: SkillFile,
    /// The runtime form.
    pub def: SkillDef,
}

/// Loads and validates one AI skill file.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_skill(
    file: &str,
    text: &str,
    pack_id: &str,
    locale: &BTreeMap<String, String>,
) -> Result<LoadedSkill, Vec<Diagnostic>> {
    let s = load(file, text, &crate::schema::skill_schema(), |c, s| {
        check_skill(c, s, pack_id, locale);
    })?;
    let def = s.to_def().ok_or_else(|| baker_bug(file))?;
    Ok(LoadedSkill { file: s, def })
}

// ---------------------------------------------------------------------------------------------
// Weapons.

fn check_weapon(c: &mut Ctx<'_>, w: &WeaponFile, pack_id: &str, locale: &BTreeMap<String, String>) {
    if w.schema != WEAPON_SCHEMA {
        c.err(
            "/schema",
            format!("`{}` is not a weapon file", w.schema),
            format!("set it to `{WEAPON_SCHEMA}`"),
        );
    }
    if let Err(hint) = check_id(&w.id, pack_id, "wpn") {
        c.err("/id", format!("`{}` is not a valid weapon id", w.id), hint);
    }
    check_locale_key(c, &w.name_key, locale);
    check_names(
        c,
        "/class",
        &w.class,
        "weapon class",
        WeaponClass::ALL,
        WeaponClass::from_name,
        |x| x.name(),
    );
    match WeaponClass::from_name(&w.class) {
        Some(WeaponClass::Gun) => {
            if w.gun.is_none() {
                c.err(
                    "/gun",
                    "a gun has no gun block",
                    "add `gun` with the calibre, muzzle velocity and rate of fire",
                );
            }
            if w.missile.is_some() {
                c.err(
                    "/missile",
                    "a gun cannot have a missile block",
                    "remove `missile`, or set the class to `srm`",
                );
            }
        }
        Some(WeaponClass::Srm) => {
            if w.missile.is_none() {
                c.err(
                    "/missile",
                    "a missile has no missile block",
                    "add `missile` with the motor, aerodynamics, seeker and warhead",
                );
            }
            if w.gun.is_some() {
                c.err(
                    "/gun",
                    "a missile cannot have a gun block",
                    "remove `gun`, or set the class to `gun`",
                );
            }
        }
        None => {}
    }
    if let Some(g) = &w.gun {
        check_gun(c, g);
    }
    if let Some(m) = &w.missile {
        check_missile(c, m, w.mass_kg);
    }
}

/// A drag table: against Mach number, strictly increasing, every value positive. Unlike the
/// aerodynamic tables of [`crate::check`] it is not tested for spikes: a transonic drag rise is a
/// real peak.
fn check_drag_curve(c: &mut Ctx<'_>, ptr: &str, t: &CurveFile) {
    if t.axis != MACH_AXIS {
        c.err(
            &format!("{ptr}/axis"),
            format!("this table is against `{MACH_AXIS}`, not `{}`", t.axis),
            format!("set axis to `{MACH_AXIS}`"),
        );
        return;
    }
    if let Some(i) = increasing(t.points.iter().map(|p| p[0])) {
        c.err(
            &format!("{ptr}/points/{i}"),
            "the x values are not strictly increasing",
            "sort the points by x and remove duplicates",
        );
        return;
    }
    if let Some(i) = t.points.iter().position(|p| p[0] < 0.0 || p[1] <= 0.0) {
        c.err(
            &format!("{ptr}/points/{i}"),
            format!(
                "({}, {}) is not a Mach number of at least 0 with a positive drag coefficient",
                t.points[i][0], t.points[i][1]
            ),
            "write points as [mach, coefficient] with a positive coefficient",
        );
    }
}

fn check_gun(c: &mut Ctx<'_>, g: &GunFile) {
    check_drag_curve(c, "/gun/drag_cd", &g.drag_cd);
    let mut seen = BTreeSet::new();
    for (i, a) in g.ammo_types.iter().enumerate() {
        if !seen.insert(a.id.as_str()) {
            c.err(
                &format!("/gun/ammo_types/{i}/id"),
                format!("round type `{}` is listed twice", a.id),
                "give each round type its own id",
            );
        }
    }
}

fn check_missile(c: &mut Ctx<'_>, m: &MissileFile, mass_kg: f64) {
    check_drag_curve(c, "/missile/aero/cd0_boost", &m.aero.cd0_boost);
    check_drag_curve(c, "/missile/aero/cd0_coast", &m.aero.cd0_coast);
    check_names(
        c,
        "/missile/guidance/law",
        &m.guidance.law,
        "guidance law",
        content::GuidanceLaw::ALL,
        content::GuidanceLaw::from_name,
        |x| x.name(),
    );
    check_names(
        c,
        "/missile/seeker/type",
        &m.seeker.kind,
        "seeker type",
        content::SeekerKind::ALL,
        content::SeekerKind::from_name,
        |x| x.name(),
    );
    check_names(
        c,
        "/missile/fuze/type",
        &m.fuze.kind,
        "fuze type",
        content::FuzeKind::ALL,
        content::FuzeKind::from_name,
        |x| x.name(),
    );
    check_names(
        c,
        "/missile/warhead/kind",
        &m.warhead.kind,
        "warhead type",
        content::WarheadKind::ALL,
        content::WarheadKind::from_name,
        |x| x.name(),
    );
    if c.clean(&["/missile/seeker/fov_deg", "/missile/seeker/gimbal_deg"])
        && m.seeker.gimbal_deg < m.seeker.fov_deg / 2.0
    {
        c.err(
            "/missile/seeker/gimbal_deg",
            format!(
                "the gimbal limit {} deg is below half the field of view ({} deg)",
                m.seeker.gimbal_deg,
                m.seeker.fov_deg / 2.0
            ),
            "the seeker must be able to point at least as far as its caged view reaches",
        );
    }
    if c.clean(&["/missile/fuze/arm_dist_m", "/missile/launch/min_range_m"])
        && m.fuze.arm_dist_m >= m.launch.min_range_m
    {
        c.err(
            "/missile/fuze/arm_dist_m",
            format!(
                "the fuze arms after {} m but the shortest launch range is {} m",
                m.fuze.arm_dist_m, m.launch.min_range_m
            ),
            "the fuze must arm before the closest allowed target, or a legal shot cannot detonate",
        );
    }
    if c.clean(&[
        "/missile/fuze/trigger_radius_m",
        "/missile/warhead/lethal_radius_m",
    ]) && m.fuze.trigger_radius_m > m.warhead.lethal_radius_m
    {
        c.err(
            "/missile/fuze/trigger_radius_m",
            format!(
                "the fuze triggers at {} m, beyond the warhead's lethal radius of {} m",
                m.fuze.trigger_radius_m, m.warhead.lethal_radius_m
            ),
            "a fuze must not fire where the warhead does no damage",
        );
    }
    if c.clean(&["/mass_kg", "/missile/motor/propellant_kg"]) && m.motor.propellant_kg >= mass_kg {
        c.err(
            "/missile/motor/propellant_kg",
            format!(
                "the propellant ({} kg) is not less than the missile's mass ({mass_kg} kg)",
                m.motor.propellant_kg
            ),
            "the mass is the launch mass; it includes the propellant and more",
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Skills.

fn check_skill(c: &mut Ctx<'_>, s: &SkillFile, pack_id: &str, locale: &BTreeMap<String, String>) {
    if s.schema != SKILL_SCHEMA {
        c.err(
            "/schema",
            format!("`{}` is not an AI skill file", s.schema),
            format!("set it to `{SKILL_SCHEMA}`"),
        );
    }
    if let Err(hint) = check_id(&s.id, pack_id, "skl") {
        c.err("/id", format!("`{}` is not a valid skill id", s.id), hint);
    }
    check_locale_key(c, &s.name_key, locale);

    let mut seen = BTreeSet::new();
    for (i, name) in s.tactics.iter().enumerate() {
        let at = format!("/tactics/{i}");
        if !TACTICS.iter().any(|t| t.0 == name) {
            let names: Vec<&str> = TACTICS.iter().map(|t| t.0).collect();
            c.err(
                &at,
                format!("`{name}` is not a known tactic"),
                format!("use one of: {}", names.join(", ")),
            );
        } else if !seen.insert(name.as_str()) {
            c.err(
                &at,
                format!("tactic `{name}` is listed twice"),
                "list each tactic once",
            );
        }
    }
    let extends = s.tactics.iter().any(|t| t == "extend");
    let below = s.extend_below_cas_mps;
    if c.clean(&["/extend_below_cas_mps", "/tactics"]) && extends != (below > 0.0) {
        c.err(
            "/extend_below_cas_mps",
            if extends {
                "the `extend` tactic needs an `extend_below_cas_mps` above zero"
            } else {
                "`extend_below_cas_mps` is above zero but the `extend` tactic is not listed"
            },
            "list `extend` and set both speeds, or remove it and set `extend_below_cas_mps` to 0",
        );
    }
    if c.clean(&["/extend_below_cas_mps", "/extend_until_cas_mps"])
        && below > 0.0
        && s.extend_until_cas_mps <= below
    {
        c.err(
            "/extend_until_cas_mps",
            format!(
                "the pilot stops extending at {} m/s, which is not above the speed it starts at ({below} m/s)",
                s.extend_until_cas_mps
            ),
            "set extend_until_cas_mps above extend_below_cas_mps",
        );
    }
}

/// The skills of a pack, `(file name, skill)`: ranks must be exactly `1..=n`, each once. Reports
/// against the file whose rank is wrong.
#[must_use]
pub fn check_skill_ladder(skills: &[(&str, &SkillFile)]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let mut first: BTreeMap<u8, &str> = BTreeMap::new();
    for (name, s) in skills {
        if let Some(other) = first.insert(s.rank, name) {
            problems.push(Diagnostic::new(
                name,
                "/rank",
                format!("rank {} is already used by {other}", s.rank),
                "give each skill its own rank",
            ));
        }
    }
    let n = skills.len();
    for (name, s) in skills {
        if usize::from(s.rank) > n && n > 0 {
            problems.push(Diagnostic::new(
                name,
                "/rank",
                format!("rank {} leaves a gap: the pack has {n} skills", s.rank),
                format!("ranks run from 1 to {n} with no gaps"),
            ));
        }
    }
    problems
}

// ---------------------------------------------------------------------------------------------
// The aircraft's side of it.

type Vec3 = [f64; 3];

fn distance_to_segment(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if len2 > 0.0 {
        ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let d = [ap[0] - ab[0] * t, ap[1] - ab[1] * t, ap[2] - ab[2] * t];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// The combat additions of an aircraft file that need nothing but the file: id shapes, stations,
/// capsule placement, signature order. References to weapons are resolved by [`crate::bake`].
pub(crate) fn check_armament(c: &mut Ctx<'_>, a: &AircraftFile, pack_id: &str) {
    if let Some(g) = a.internal_weapons.as_ref().and_then(|w| w.gun.as_ref())
        && let Err(hint) = check_id(&g.weapon_ref, pack_id, "wpn")
    {
        c.err(
            "/internal_weapons/gun/ref",
            format!("`{}` is not a valid weapon id", g.weapon_ref),
            hint,
        );
    }
    if let Some(l) = &a.loadout {
        let mut stations = BTreeSet::new();
        for (i, r) in l.rails.iter().enumerate() {
            let at = format!("/loadout/rails/{i}");
            if let Err(hint) = check_id(&r.store, pack_id, "wpn") {
                c.err(
                    &format!("{at}/store"),
                    format!("`{}` is not a valid weapon id", r.store),
                    hint,
                );
            }
            if !stations.insert(r.station) {
                c.err(
                    &format!("{at}/station"),
                    format!("station {} has two rails", r.station),
                    "give each rail its own station",
                );
            }
            if r.station > a.hardpoints {
                c.err(
                    &format!("{at}/station"),
                    format!(
                        "station {} is beyond the aircraft's {} hardpoints",
                        r.station, a.hardpoints
                    ),
                    "use a station from 1 to `hardpoints`, or raise `hardpoints`",
                );
            }
        }
    }
    if let Some(s) = &a.signatures
        && let Some(i) = &s.ir
        && c.clean(&["/signatures/ir"])
        && !(i.engine_idle <= i.engine_mil && i.engine_mil <= i.engine_ab)
    {
        c.err(
            "/signatures/ir",
            "the engine intensities do not rise from idle through military to afterburner",
            "set engine_idle <= engine_mil <= engine_ab",
        );
    }
    if let Some(d) = &a.damage {
        check_capsules(c, a, &d.capsules);
    }
}

/// Capsule names are unique, every end lies inside the bounding box (centred on the reference
/// point) grown by the capsule's radius, and every airframe contact point is near a capsule.
fn check_capsules(
    c: &mut Ctx<'_>,
    a: &AircraftFile,
    capsules: &[crate::combat_model::CapsuleFile],
) {
    let mut seen = BTreeSet::new();
    for (i, cap) in capsules.iter().enumerate() {
        let at = format!("/damage/capsules/{i}");
        if !seen.insert(cap.id.as_str()) {
            c.err(
                &format!("{at}/id"),
                format!("hit capsule `{}` is listed twice", cap.id),
                "give each capsule its own name",
            );
        }
        if !c.clean(&["/geometry/bbox_m"]) {
            continue;
        }
        for (end, p) in [("a_m", cap.a_m), ("b_m", cap.b_m)] {
            let outside = (0..3).any(|k| p[k].abs() > a.geometry.bbox_m[k] / 2.0 + cap.radius_m);
            if outside {
                c.err(
                    &format!("{at}/{end}"),
                    format!(
                        "({}, {}, {}) m is outside the bounding box {} x {} x {} m grown by the radius",
                        p[0], p[1], p[2], a.geometry.bbox_m[0], a.geometry.bbox_m[1],
                        a.geometry.bbox_m[2]
                    ),
                    "positions are body axes in metres from the reference point; check the sign and the unit",
                );
            }
        }
    }
    if !c.clean(&["/geometry/contacts"]) {
        return;
    }
    for (i, p) in a.geometry.contacts.iter().enumerate() {
        let near = capsules.iter().any(|cap| {
            distance_to_segment(p.pos_m, cap.a_m, cap.b_m) <= cap.radius_m + CONTACT_SLACK_M
        });
        if !near {
            c.err(
                &format!("/geometry/contacts/{i}"),
                format!(
                    "contact point `{}` is more than {CONTACT_SLACK_M} m outside every hit capsule",
                    p.id
                ),
                "move a capsule over the contact point, or the point onto the airframe",
            );
        }
    }
}

/// The weapons an aircraft file names must exist and be of the right class. `class_of` gives the
/// class of the weapon with an id, `None` if the pack has no weapon with it.
pub(crate) fn check_weapon_references(
    a: &AircraftFile,
    class_of: &dyn Fn(&str) -> Option<WeaponClass>,
) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut check = |ptr: String, id: &str, want: WeaponClass, what: &str| match class_of(id) {
        None => out.push((
            ptr,
            format!("there is no weapon `{id}` in the pack"),
            "add the weapon file under weapons/, or fix the id".to_owned(),
        )),
        Some(got) if got != want => out.push((
            ptr,
            format!("`{id}` is a {}, not {what}", got.name()),
            format!("use a weapon of class `{}`", want.name()),
        )),
        Some(_) => {}
    };
    if let Some(g) = a.internal_weapons.as_ref().and_then(|w| w.gun.as_ref()) {
        check(
            "/internal_weapons/gun/ref".to_owned(),
            &g.weapon_ref,
            WeaponClass::Gun,
            "a gun",
        );
    }
    if let Some(l) = &a.loadout {
        for (i, r) in l.rails.iter().enumerate() {
            check(
                format!("/loadout/rails/{i}/store"),
                &r.store,
                WeaponClass::Srm,
                "a short-range missile",
            );
        }
    }
    out
}
