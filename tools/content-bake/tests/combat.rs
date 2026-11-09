//! The combat data (ADR-46, ADR-47): `weapon/1`, `ai_skill/1` and the additions to `aircraft/1`.
//! Each case breaks one thing in the real core pack and must be reported at exactly that pointer,
//! with a message and a fix.

use std::fs;
use std::path::{Path, PathBuf};

use content_bake::bake::{Baked, Sources, bake_sources};
use content_bake::diag::Diagnostic;
use serde_json::{Value, json};

fn core() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core")
}

fn read(rel: &str) -> String {
    fs::read_to_string(core().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn json_of(rel: &str) -> Value {
    serde_json::from_str(&read(rel)).unwrap()
}

const KESTREL: &str = "aircraft/kestrel.json";
const GS27: &str = "weapons/gs27.json";
const DART: &str = "weapons/dart1.json";
const RECRUIT: &str = "skills/recruit.json";
const REGULAR: &str = "skills/regular.json";
const VETERAN: &str = "skills/veteran.json";
const WEAPONS: [&str; 2] = [DART, GS27];
const SKILLS: [&str; 3] = [RECRUIT, REGULAR, VETERAN];

/// What to change in the real pack: a file replaced by other text (reported under `test.json`),
/// and files left out.
struct Change<'a> {
    replace: Option<(&'a str, String)>,
    without: &'a [&'a str],
}

fn bake_real(change: &Change<'_>) -> Result<Baked, Vec<Diagnostic>> {
    let pick = |rels: &[&str]| -> Vec<(String, String)> {
        rels.iter()
            .filter(|r| !change.without.contains(r))
            .map(|rel| match &change.replace {
                Some((which, text)) if which == rel => ("test.json".to_owned(), text.clone()),
                _ => ((*rel).to_owned(), read(rel)),
            })
            .collect()
    };
    bake_sources(&Sources {
        pack: ("pack.json".into(), read("pack.json")),
        locale: ("locales/en.json".into(), read("locales/en.json")),
        aircraft: pick(&[KESTREL]),
        engines: pick(&["engines/f129.json"]),
        aero: pick(&["aero/kestrel.json"]),
        fcs: pick(&["fcs/kestrel.json"]),
        weapons: pick(&WEAPONS),
        skills: pick(&SKILLS),
    })
}

type Edit = Box<dyn FnOnce(&mut Value)>;

fn set(ptr: &'static str, value: Value) -> Edit {
    Box::new(move |v| *v.pointer_mut(ptr).unwrap_or_else(|| panic!("no {ptr}")) = value)
}

fn remove(parent: &'static str, key: &'static str) -> Edit {
    Box::new(move |v| {
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key)
            .unwrap();
    })
}

fn insert(parent: &'static str, key: &'static str, value: Value) -> Edit {
    Box::new(move |v| {
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
    })
}

/// Runs each case against `which`, expecting exactly one problem, in `report_file` (the edited
/// file, reported as `test.json`, when that is empty), at `pointer`, whose message contains
/// `message`.
fn expect_cases(which: &str, report_file: &str, cases: Vec<(&str, Edit, &str, &str)>) {
    for (what, edit, pointer, message) in cases {
        let mut v = json_of(which);
        edit(&mut v);
        let text = serde_json::to_string_pretty(&v).unwrap();
        let problems = bake_real(&Change {
            replace: Some((which, text)),
            without: &[],
        })
        .expect_err(what);
        let found: Vec<(&str, &str)> = problems
            .iter()
            .map(|p| (p.file.as_str(), p.pointer.as_str()))
            .collect();
        let file = if report_file.is_empty() {
            "test.json"
        } else {
            report_file
        };
        assert_eq!(found, [(file, pointer)], "{what}: {problems:#?}");
        let p = &problems[0];
        assert!(
            p.message.contains(message),
            "{what}: message `{}` lacks `{message}`",
            p.message
        );
        assert!(!p.hint.is_empty(), "{what}: every error says how to fix it");
    }
}

#[test]
fn the_real_pack_bakes_with_its_combat_data() {
    let baked = bake_real(&Change {
        replace: None,
        without: &[],
    })
    .expect("the core pack is valid");
    assert_eq!(baked.weapons.len(), 2);
    assert_eq!(baked.skills.len(), 3);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "a table of cases; splitting it would hide the table"
)]
fn malformed_missiles_are_rejected_at_the_right_pointer() {
    expect_cases(
        DART,
        "",
        vec![
            (
                "gimbal below half the field of view",
                set("/missile/seeker/gimbal_deg", json!(1.5)),
                "/missile/seeker/gimbal_deg",
                "below half the field of view",
            ),
            (
                "fuze arming after the shortest launch range",
                set("/missile/fuze/arm_dist_m", json!(500)),
                "/missile/fuze/arm_dist_m",
                "arms after 500 m",
            ),
            (
                "fuze triggering beyond the lethal radius",
                set("/missile/fuze/trigger_radius_m", json!(9)),
                "/missile/fuze/trigger_radius_m",
                "lethal radius",
            ),
            (
                "propellant not less than the mass",
                set("/missile/motor/propellant_kg", json!(90)),
                "/missile/motor/propellant_kg",
                "not less than the missile's mass",
            ),
            (
                "navigation constant out of range (inside an optional block)",
                set("/missile/guidance/n_prime", json!(9)),
                "/missile/guidance/n_prime",
                "outside the allowed range",
            ),
            (
                "lock time out of range",
                set("/missile/seeker/lock_time_s", json!(4)),
                "/missile/seeker/lock_time_s",
                "outside the allowed range",
            ),
            (
                "burn time of zero",
                set("/missile/motor/burn_s", json!(0)),
                "/missile/motor/burn_s",
                "outside the allowed range",
            ),
            (
                "unknown guidance law",
                set("/missile/guidance/law", json!("apn")),
                "/missile/guidance/law",
                "not a known guidance law",
            ),
            (
                "unknown seeker type",
                set("/missile/seeker/type", json!("radar")),
                "/missile/seeker/type",
                "not a known seeker type",
            ),
            (
                "unknown fuze type",
                set("/missile/fuze/type", json!("contact")),
                "/missile/fuze/type",
                "not a known fuze type",
            ),
            (
                "unknown warhead type",
                set("/missile/warhead/kind", json!("nuclear")),
                "/missile/warhead/kind",
                "not a known warhead type",
            ),
            (
                "a drag table against the wrong axis",
                set("/missile/aero/cd0_boost/axis", json!("alpha_deg")),
                "/missile/aero/cd0_boost/axis",
                "against `mach`",
            ),
            (
                "a drag table that does not increase",
                set(
                    "/missile/aero/cd0_coast/points",
                    json!([[0, 0.4], [0, 0.5]]),
                ),
                "/missile/aero/cd0_coast/points/1",
                "not strictly increasing",
            ),
            (
                "a drag coefficient of zero",
                set("/missile/aero/cd0_coast/points", json!([[0, 0.4], [1, 0]])),
                "/missile/aero/cd0_coast/points/1",
                "positive drag coefficient",
            ),
            (
                "an unknown key in a block",
                insert("/missile/motor", "surprise", json!(1)),
                "/missile/motor/surprise",
                "unknown key",
            ),
            (
                "a missing key in a block",
                remove("/missile/launch", "safe_sep_s"),
                "/missile/launch/safe_sep_s",
                "required key",
            ),
            (
                "a gun block on a missile",
                insert("", "gun", json_of(GS27)["gun"].clone()),
                "/gun",
                "cannot have a gun block",
            ),
            (
                "a missile with no missile block",
                remove("", "missile"),
                "/missile",
                "has no missile block",
            ),
            (
                "an id of the wrong type",
                set("/id", json!("core:ac/dart1")),
                "/id",
                "not a valid weapon id",
            ),
            (
                "a name key that is not in the locale",
                set("/name_key", json!("wpn.ghost.name")),
                "/name_key",
                "not in the default locale",
            ),
            (
                "a value single precision cannot hold",
                set("/missile/seeker/noise_deg", json!(0.123_456_789_012_3)),
                "/missile/seeker/noise_deg",
                "single precision",
            ),
        ],
    );
}

#[test]
fn malformed_guns_are_rejected_at_the_right_pointer() {
    expect_cases(
        GS27,
        "",
        vec![
            (
                "a rate of fire that is too low",
                set("/gun/rpm", json!(50)),
                "/gun/rpm",
                "outside the allowed range",
            ),
            (
                "a rate of fire that is too high",
                set("/gun/rpm", json!(20000)),
                "/gun/rpm",
                "outside the allowed range",
            ),
            (
                "a dispersion that is too wide",
                set("/gun/dispersion_mrad", json!(25)),
                "/gun/dispersion_mrad",
                "outside the allowed range",
            ),
            (
                "two round types with one name",
                insert_ammo(),
                "/gun/ammo_types/1/id",
                "listed twice",
            ),
            (
                "a missile block on a gun",
                insert("", "missile", json_of(DART)["missile"].clone()),
                "/missile",
                "cannot have a missile block",
            ),
            (
                "a gun with no gun block",
                remove("", "gun"),
                "/gun",
                "has no gun block",
            ),
            (
                "an unknown class",
                set("/class", json!("bomb")),
                "/class",
                "not a known weapon class",
            ),
            (
                "an unknown key",
                insert("/gun", "surprise", json!(1)),
                "/gun/surprise",
                "unknown key",
            ),
            (
                "a drag table that is not against Mach",
                set("/gun/drag_cd/axis", json!("qbar_kpa")),
                "/gun/drag_cd/axis",
                "against `mach`",
            ),
        ],
    );
}

fn insert_ammo() -> Edit {
    Box::new(|v| {
        v.pointer_mut("/gun/ammo_types")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .push(json!({ "id": "hei", "tracer_every": 0 }));
    })
}

#[test]
fn malformed_skills_are_rejected_at_the_right_pointer() {
    expect_cases(
        REGULAR,
        "",
        vec![
            (
                "an unknown tactic",
                set("/tactics/1", json!("snap_roll")),
                "/tactics/1",
                "not a known tactic",
            ),
            (
                "a tactic listed twice",
                set("/tactics/1", json!("pure_pursuit")),
                "/tactics/1",
                "listed twice",
            ),
            (
                "extend speed without the extend tactic",
                set("/extend_below_cas_mps", json!(100)),
                "/extend_below_cas_mps",
                "`extend` tactic is not listed",
            ),
            (
                "a probability above one",
                set("/notice_prob", json!(1.5)),
                "/notice_prob",
                "outside the allowed range",
            ),
            (
                "a fraction below zero",
                set("/g_frac", json!(-0.1)),
                "/g_frac",
                "outside the allowed range",
            ),
            (
                "a missing key",
                remove("", "commitment_s"),
                "/commitment_s",
                "required key",
            ),
            (
                "an unknown key",
                insert("", "bravery", json!(1)),
                "/bravery",
                "unknown key",
            ),
            (
                "an id of the wrong type",
                set("/id", json!("core:wpn/regular")),
                "/id",
                "not a valid skill id",
            ),
        ],
    );
    expect_cases(
        VETERAN,
        "",
        vec![
            (
                "the extend tactic with no speed to start at",
                set("/extend_below_cas_mps", json!(0)),
                "/extend_below_cas_mps",
                "needs an `extend_below_cas_mps` above zero",
            ),
            (
                "stopping to extend below the speed it starts at",
                set("/extend_until_cas_mps", json!(140)),
                "/extend_until_cas_mps",
                "not above the speed it starts at",
            ),
        ],
    );
}

#[test]
fn the_skill_ranks_must_be_one_to_n_each_once() {
    // The regular takes the recruit's rank: reported against the later file.
    expect_cases(
        REGULAR,
        "",
        vec![(
            "a rank used twice",
            set("/rank", json!(1)),
            "/rank",
            "already used by skills/recruit.json",
        )],
    );
    // The veteran jumps to rank 5 in a pack of three.
    expect_cases(
        VETERAN,
        "",
        vec![(
            "a gap in the ranks",
            set("/rank", json!(5)),
            "/rank",
            "leaves a gap",
        )],
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "a table of cases; splitting it would hide the table"
)]
fn malformed_aircraft_combat_data_is_rejected_at_the_right_pointer() {
    expect_cases(
        KESTREL,
        "",
        vec![
            (
                "a gun that is a missile",
                set("/internal_weapons/gun/ref", json!("core:wpn/dart1")),
                "/internal_weapons/gun/ref",
                "is a srm",
            ),
            (
                "a gun that is not in the pack",
                set("/internal_weapons/gun/ref", json!("core:wpn/nothing")),
                "/internal_weapons/gun/ref",
                "there is no weapon",
            ),
            (
                "a gun id of the wrong type",
                set("/internal_weapons/gun/ref", json!("core:eng/f129")),
                "/internal_weapons/gun/ref",
                "not a valid weapon id",
            ),
            (
                "a rail store that is a gun",
                set("/loadout/rails/0/store", json!("core:wpn/gs27")),
                "/loadout/rails/0/store",
                "is a gun",
            ),
            (
                "two rails on one station",
                set("/loadout/rails/1/station", json!(1)),
                "/loadout/rails/1/station",
                "has two rails",
            ),
            (
                "a station beyond the hardpoints",
                set("/loadout/rails/1/station", json!(10)),
                "/loadout/rails/1/station",
                "beyond the aircraft's 9 hardpoints",
            ),
            (
                "a capsule outside the bounding box",
                set("/damage/capsules/0/a_m", json!([70, 0, 0.3])),
                "/damage/capsules/0/a_m",
                "outside the bounding box",
            ),
            (
                "two capsules with one name",
                set("/damage/capsules/1/id", json!("fuselage")),
                "/damage/capsules/1/id",
                "listed twice",
            ),
            (
                "hit points of zero",
                set("/damage/hp", json!(0)),
                "/damage/hp",
                "outside the allowed range",
            ),
            (
                "engine intensities out of order",
                set("/signatures/ir/engine_mil", json!(1.2)),
                "/signatures/ir",
                "do not rise from idle",
            ),
            (
                "an intensity factor above one",
                set("/signatures/ir/aspect_side", json!(1.5)),
                "/signatures/ir/aspect_side",
                "outside the allowed range",
            ),
            (
                "an unknown key in the damage block",
                insert("/damage", "armour", json!(1)),
                "/damage/armour",
                "unknown key",
            ),
            (
                "a missing key in the gun installation",
                remove("/internal_weapons/gun", "rounds"),
                "/internal_weapons/gun/rounds",
                "required key",
            ),
        ],
    );
}

#[test]
fn an_aircraft_naming_weapons_the_pack_lacks_is_refused_for_each_of_them() {
    let problems = bake_real(&Change {
        replace: None,
        without: &WEAPONS,
    })
    .unwrap_err();
    let found: Vec<(&str, &str)> = problems
        .iter()
        .map(|p| (p.file.as_str(), p.pointer.as_str()))
        .collect();
    // The two weapon names in the locale are then used by nothing, which is reported too.
    assert_eq!(
        found,
        [
            (KESTREL, "/internal_weapons/gun/ref"),
            (KESTREL, "/loadout/rails/0/store"),
            (KESTREL, "/loadout/rails/1/store"),
            ("locales/en.json", "/wpn.dart1.name"),
            ("locales/en.json", "/wpn.gs27.name"),
        ]
    );
    assert!(
        problems[..3]
            .iter()
            .all(|p| p.message.contains("no weapon"))
    );
}

#[test]
fn every_contact_point_must_be_near_a_hit_capsule() {
    // Leave only a thin fuselage: the wingtip contacts and the belly are then far from it.
    let mut k = json_of(KESTREL);
    let fuselage = k["damage"]["capsules"][0].clone();
    k["damage"]["capsules"] = json!([fuselage]);
    k["damage"]["capsules"][0]["radius_m"] = json!(0.2);
    let problems = bake_real(&Change {
        replace: Some((KESTREL, k.to_string())),
        without: &[],
    })
    .unwrap_err();
    let pointers: Vec<&str> = problems.iter().map(|p| p.pointer.as_str()).collect();
    assert_eq!(
        pointers,
        [
            "/geometry/contacts/2",
            "/geometry/contacts/3",
            "/geometry/contacts/4"
        ]
    );
    assert!(problems[0].message.contains("wing_l"));
}

#[test]
fn weapon_and_skill_names_that_nothing_uses_are_refused() {
    // Without the skills, their locale keys are unused: each is reported once.
    let problems = bake_real(&Change {
        replace: None,
        without: &SKILLS,
    })
    .unwrap_err();
    let pointers: Vec<&str> = problems.iter().map(|p| p.pointer.as_str()).collect();
    assert_eq!(
        pointers,
        [
            "/skl.recruit.name",
            "/skl.regular.name",
            "/skl.veteran.name"
        ]
    );
}

#[test]
fn an_aircraft_without_combat_data_still_bakes() {
    // The sections are optional: take them out of the Kestrel and drop the weapons and skills
    // with their locale keys.
    let mut k = json_of(KESTREL);
    for key in ["internal_weapons", "loadout", "signatures", "damage"] {
        k.as_object_mut().unwrap().remove(key).unwrap();
    }
    let mut locale = json_of("locales/en.json");
    for key in [
        "wpn.gs27.name",
        "wpn.dart1.name",
        "skl.recruit.name",
        "skl.regular.name",
        "skl.veteran.name",
    ] {
        locale.as_object_mut().unwrap().remove(key).unwrap();
    }
    let baked = bake_sources(&Sources {
        pack: ("pack.json".into(), read("pack.json")),
        locale: ("locales/en.json".into(), locale.to_string()),
        aircraft: vec![(KESTREL.into(), k.to_string())],
        engines: vec![("engines/f129.json".into(), read("engines/f129.json"))],
        aero: vec![("aero/kestrel.json".into(), read("aero/kestrel.json"))],
        fcs: vec![("fcs/kestrel.json".into(), read("fcs/kestrel.json"))],
        weapons: vec![],
        skills: vec![],
    })
    .expect("an unarmed aircraft is valid");
    let reg = content::Registry::from_cpack(&baked.cpack).unwrap();
    let k = reg.aircraft(reg.find("core:ac/kestrel").unwrap()).unwrap();
    assert!(k.gun.is_none() && k.rails.is_empty() && k.ir.is_none() && k.damage.is_none());
    assert_eq!(k.visual_l_eff_m, 0.0);
}
