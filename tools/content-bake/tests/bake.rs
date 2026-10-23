//! End-to-end tests of the baker against the real core pack and deliberately broken variants of it.

use std::fs;
use std::path::{Path, PathBuf};

use content::{CpackError, EngineKind, Metric, Registry, Requirement};
use content_bake::bake::{self, Baked, Sources, bake_dir, bake_sources, sha256};
use content_bake::diag::Diagnostic;
use content_bake::export::export;
use content_bake::model::{AeroFile, AircraftFile, EngineFile, FcsGainsFile};
use content_bake::schema;
use serde_json::{Value, json};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn core() -> PathBuf {
    root().join("content/core")
}

fn read(rel: &str) -> String {
    fs::read_to_string(core().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn json_of(rel: &str) -> Value {
    serde_json::from_str(&read(rel)).unwrap()
}

const KESTREL: &str = "aircraft/kestrel.json";
const F129: &str = "engines/f129.json";
const AERO: &str = "aero/kestrel.json";
const FCS: &str = "fcs/kestrel.json";

/// Bakes the real pack with the given files in place of its entity files.
fn bake_files(
    aircraft: &[(&str, String)],
    engines: &[(&str, String)],
    aero: &[(&str, String)],
    fcs: &[(&str, String)],
) -> Result<Baked, Vec<Diagnostic>> {
    let own = |v: &[(&str, String)]| -> Vec<(String, String)> {
        v.iter()
            .map(|(n, t)| ((*n).to_owned(), t.clone()))
            .collect()
    };
    bake_sources(&Sources {
        pack: ("pack.json".into(), read("pack.json")),
        locale: ("locales/en.json".into(), read("locales/en.json")),
        aircraft: own(aircraft),
        engines: own(engines),
        aero: own(aero),
        fcs: own(fcs),
    })
}

/// The real pack with one file replaced by `text`, reported under `name`.
fn bake_replacing(which: &str, name: &str, text: &str) -> Result<Baked, Vec<Diagnostic>> {
    fn pick<'a>(rel: &'a str, which: &str, name: &'a str, text: &str) -> (&'a str, String) {
        if rel == which {
            (name, text.to_owned())
        } else {
            (rel, read(rel))
        }
    }
    bake_files(
        &[pick(KESTREL, which, name, text)],
        &[pick(F129, which, name, text)],
        &[pick(AERO, which, name, text)],
        &[pick(FCS, which, name, text)],
    )
}

#[test]
fn the_core_pack_bakes_and_holds_the_kestrel() {
    let baked = bake_dir(&core()).expect("the core pack is valid");
    let reg = Registry::from_cpack(&baked.cpack).unwrap();
    assert_eq!(reg.counts(), (1, 1, 1, 1));
    let h = reg.find("core:ac/kestrel").expect("kestrel is in the pack");
    let k = reg.aircraft(h).unwrap();

    // Appendix C.1, C.2, C.6.
    assert_eq!(k.name_key, "ac.kestrel.name");
    assert_eq!(k.category, content::Category::LightFighter);
    assert_eq!(k.era, content::Era::G4);
    assert_eq!((k.crew, k.hardpoints, k.engine_count()), (1, 9, 1));
    assert_eq!(
        (
            k.mass.empty_kg,
            k.mass.internal_fuel_kg,
            k.mass.max_takeoff_kg
        ),
        (8400.0, 3100.0, 14500.0)
    );
    let tank_sum: f32 = k.mass.tanks.iter().map(|t| t.capacity_kg).sum();
    assert_eq!(tank_sum, 3100.0);
    assert_eq!(
        (k.geometry.wing_area_m2, k.geometry.span_m, k.geometry.mac_m),
        (28.0, 9.8, 3.2)
    );
    assert_eq!(
        (k.aero.n_max, k.aero.n_min, k.aero.mmo, k.aero.vmo_mps),
        (9.0, -3.0, 2.0, 360.0)
    );
    assert!(!k.fcs.sas_required);
    assert_eq!(k.fcs.stab.range_deg, [-25.0, 25.0]);
    assert_eq!(k.fcs.gear.transit_s, 6.0);
    assert_eq!(reg.id(k.fcs.gains), Some("core:fcs/kestrel"));
    let gains = reg.fcs(k.fcs.gains).expect("the gains reference resolves");
    assert_eq!(gains.pitch.k_q_per_s, 6.0);
    assert_eq!(gains.ground.nws.low_deg, 60.0);

    // The landing gear: a steerable braked nose leg and two braked mains (ADR-35).
    let legs: Vec<(&str, bool, bool)> = k
        .geometry
        .gear
        .iter()
        .map(|l| (l.id.as_str(), l.main, l.steer_max_deg > 0.0))
        .collect();
    assert_eq!(
        legs,
        [
            ("nose", false, true),
            ("main_l", true, false),
            ("main_r", true, false)
        ]
    );
    assert!(k.geometry.gear.iter().all(|l| l.brake));
    let contacts: Vec<&str> = k.geometry.contacts.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(contacts, ["nose_tip", "tail", "wing_l", "wing_r", "belly"]);
    assert_eq!(
        (k.aero.n_max_by_config.gear, k.aero.n_max_by_config.flaps),
        (4.0, 4.0)
    );

    // C.9: twelve points, the two ground rolls need the gear model (ADR-31).
    assert_eq!(k.envelope.len(), 12);
    let deferred: Vec<&str> = k
        .envelope
        .iter()
        .filter(|p| p.requires == Some(Requirement::Gear))
        .map(|p| p.id.as_str())
        .collect();
    assert_eq!(deferred, ["to_roll", "ldg_roll"]);
    assert!(
        k.envelope
            .iter()
            .any(|p| p.metric == Metric::RocMps && (p.value - 283.1).abs() < 1e-3)
    );

    // C.3: the F129.
    let e = reg
        .engine(k.engines[0].engine)
        .expect("the engine reference resolves");
    assert_eq!(e.kind, EngineKind::Turbofan);
    assert_eq!(
        (e.sl_dry_n, e.sl_ab_n, e.ab_stages),
        (79000.0, 128_000.0, 4)
    );
    // The reference weight of C.2 gives its quoted T/W of 1.243 at 10 500 kg.
    let t_w = f64::from(e.sl_ab_n) / (10_500.0 * 9.806_65);
    assert!((t_w - 1.243).abs() < 0.001, "{t_w}");

    // C.4: the aero set.
    let a = reg
        .aero(k.aero.tables)
        .expect("the aero reference resolves");
    assert_eq!(a.lift.cl_base.xs.len(), 31);
    assert_eq!(a.lift.cl_base.ys.len(), 12);
    assert_eq!(a.drag.cd_wave.xs.len(), 9);

    // C.5 values hold up to 15 degrees; the lateral tables then depart from them (ADR-38).
    let at = |c: &content::Curve, x: f32| {
        let i = c.xs.partition_point(|&v| v <= x).saturating_sub(1);
        (c.xs[i], c.ys[i])
    };
    assert_eq!(at(&a.lateral.cn_beta_per_rad, 10.0), (0.0, 0.14));
    assert_eq!(at(&a.lateral.cl_beta_per_rad, 10.0), (0.0, -0.1));
    assert_eq!(a.lateral.cy_beta_per_rad.ys, [-0.9, -0.9]);
    assert!(a.lateral.cn_beta_per_rad.ys.contains(&-0.08));
}

#[test]
fn export_gives_back_the_normalised_input() {
    let baked = bake_dir(&core()).unwrap();
    let exported = export(&baked.cpack).unwrap();
    // The source files, read back through the same typed model the baker uses.
    let aircraft: AircraftFile = serde_json::from_str(&read(KESTREL)).unwrap();
    let engine: EngineFile = serde_json::from_str(&read(F129)).unwrap();
    let aero: AeroFile = serde_json::from_str(&read(AERO)).unwrap();
    assert_eq!(exported.aircraft, vec![aircraft]);
    let fcs: FcsGainsFile = serde_json::from_str(&read(FCS)).unwrap();
    assert_eq!(exported.engines, vec![engine]);
    assert_eq!(exported.aero, vec![aero]);
    assert_eq!(exported.fcs, vec![fcs]);
    assert_eq!(exported.aircraft, baked.aircraft);
    assert!(exported.content_hash.starts_with("sha256:"));
}

#[test]
fn the_output_is_byte_stable() {
    let a = bake_dir(&core()).unwrap();
    let b = bake_dir(&core()).unwrap();
    assert_eq!(a.cpack, b.cpack);
    assert_eq!(
        bake::manifest(&a, "core.cpack"),
        bake::manifest(&b, "core.cpack")
    );
    // Same data in differently formatted files still bakes to the same bytes.
    let compact = |rel: &str| serde_json::to_string(&json_of(rel)).unwrap();
    let c = bake_files(
        &[(KESTREL, compact(KESTREL))],
        &[(F129, compact(F129))],
        &[(AERO, compact(AERO))],
        &[(FCS, compact(FCS))],
    )
    .unwrap();
    assert_eq!(a.cpack, c.cpack);
}

#[test]
fn the_content_hash_is_the_sha256_of_everything_after_the_header() {
    let baked = bake_dir(&core()).unwrap();
    let reg = Registry::from_cpack(&baked.cpack).unwrap();
    let expect = sha256(&baked.cpack[content::cpack::HEADER_BYTES..]);
    assert_eq!(reg.content_hash(), &expect);
    let manifest = bake::manifest(&baked, "core.cpack");
    assert_eq!(
        manifest["packs"][0]["content_hash"],
        json!(format!("sha256:{}", bake::hex(&expect)))
    );
    assert_eq!(manifest["packs"][0]["bytes"], json!(baked.cpack.len()));
}

#[test]
fn a_corrupt_byte_fails_the_checksum() {
    let baked = bake_dir(&core()).unwrap();
    // A flip in the last record: the layout is still valid, only the checksum notices.
    let mut bad = baked.cpack.clone();
    let at = bad.len() - 16;
    bad[at] ^= 0x40;
    assert!(matches!(
        Registry::from_cpack(&bad),
        Err(CpackError::Checksum { .. })
    ));
}

#[test]
fn the_baked_pack_is_what_the_manifest_says() {
    let baked = bake_dir(&core()).unwrap();
    let m = bake::manifest(&baked, "core.cpack");
    assert_eq!(m["schema"], "content-manifest/1");
    assert_eq!(m["packs"][0]["id"], "core");
    assert_eq!(
        m["packs"][0]["entities"],
        json!({ "aircraft": 1, "engines": 1, "aero": 1, "fcs": 1 })
    );
}

// ---------------------------------------------------------------------------------------------
// Malformed input: each case breaks one thing and must be reported at exactly that pointer.

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

/// Runs each case against `which`, expecting exactly one problem at `pointer` in `report_file`.
fn expect_cases(which: &str, report_file: &str, cases: Vec<(&str, Edit, &str, &str)>) {
    for (what, edit, pointer, message) in cases {
        let mut v = json_of(which);
        edit(&mut v);
        let text = serde_json::to_string_pretty(&v).unwrap();
        let problems = bake_replacing(which, "test.json", &text).expect_err(what);
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
#[allow(
    clippy::too_many_lines,
    reason = "a table of cases; splitting it would hide the table"
)]
fn malformed_aircraft_are_rejected_at_the_right_pointer() {
    expect_cases(
        KESTREL,
        "",
        vec![
            (
                "id is not a pattern",
                set("/id", json!("Kestrel")),
                "/id",
                "not a valid aircraft id",
            ),
            (
                "id in another namespace",
                set("/id", json!("other:ac/kestrel")),
                "/id",
                "not a valid aircraft id",
            ),
            (
                "id of another type",
                set("/id", json!("core:wpn/kestrel")),
                "/id",
                "not a valid aircraft id",
            ),
            (
                "wrong format tag",
                set("/schema", json!("aircraft/2")),
                "/schema",
                "not an aircraft file",
            ),
            (
                "unknown key",
                insert("/mass", "payload_kg", json!(5.0)),
                "/mass/payload_kg",
                "unknown key",
            ),
            (
                "unknown top-level key",
                insert("", "colour", json!("red")),
                "/colour",
                "unknown key",
            ),
            (
                "missing key",
                remove("/geometry", "mac_m"),
                "/geometry/mac_m",
                "missing",
            ),
            (
                "number given as text",
                set("/mass/empty_kg", json!("heavy")),
                "/mass/empty_kg",
                "invalid type",
            ),
            (
                "array of the wrong size",
                set("/mass/inertia_kgm2", json!([1.0, 2.0])),
                "/mass/inertia_kgm2",
                "invalid length",
            ),
            (
                "negative mass",
                set("/mass/empty_kg", json!(-5.0)),
                "/mass/empty_kg",
                "outside the allowed range",
            ),
            (
                "zero fuel",
                set("/mass/internal_fuel_kg", json!(0.0)),
                "/mass/internal_fuel_kg",
                "outside the allowed range",
            ),
            (
                "take-off mass below dry mass plus fuel",
                set("/mass/max_takeoff_kg", json!(11000.0)),
                "/mass/max_takeoff_kg",
                "below empty mass plus internal fuel",
            ),
            (
                "landing mass above take-off mass",
                set("/mass/max_landing_kg", json!(15000.0)),
                "/mass/max_landing_kg",
                "maximum landing mass",
            ),
            (
                "tanks do not add up",
                set("/mass/tanks/0/capacity_kg", json!(1000.0)),
                "/mass/tanks",
                "tanks hold",
            ),
            (
                "duplicate tank",
                set("/mass/tanks/1/id", json!("fwd")),
                "/mass/tanks/1/id",
                "listed twice",
            ),
            (
                "wing far too large",
                set("/geometry/wing_area_m2", json!(500.0)),
                "/geometry/wing_area_m2",
                "wing loading",
            ),
            (
                "span below chord",
                set("/geometry/span_m", json!(3.0)),
                "/geometry/span_m",
                "not larger than the chord",
            ),
            (
                "inertia breaks the triangle inequality",
                set("/mass/inertia_kgm2", json!([1000.0, 1000.0, 90000.0])),
                "/mass/inertia_kgm2",
                "rigid body",
            ),
            (
                "zero inertia",
                set("/mass/inertia_kgm2/1", json!(0.0)),
                "/mass/inertia_kgm2/1",
                "not positive",
            ),
            (
                "product of inertia too large",
                set("/mass/inertia_xz_kgm2", json!(40000.0)),
                "/mass/inertia_xz_kgm2",
                "positive definite",
            ),
            (
                "negative load limit above zero",
                set("/aero/n_min", json!(2.0)),
                "/aero/n_min",
                "outside the allowed range",
            ),
            (
                "positive load limit below zero",
                set("/aero/n_max", json!(-1.0)),
                "/aero/n_max",
                "outside the allowed range",
            ),
            (
                "unstable airframe without SAS",
                set("/aero/static_margin_mac", json!(-0.05)),
                "/fcs/sas_required",
                "unstable",
            ),
            (
                "empty actuator travel",
                set("/fcs/actuators/stab/range_deg", json!([25.0, -25.0])),
                "/fcs/actuators/stab/range_deg",
                "is empty",
            ),
            (
                "engine reference of the wrong type",
                set("/engines/0/ref", json!("core:ac/f129")),
                "/engines/0/ref",
                "not a valid engine id",
            ),
            (
                "aero reference of the wrong type",
                set("/aero/tables_ref", json!("core:eng/kestrel")),
                "/aero/tables_ref",
                "not a valid aero-table id",
            ),
            (
                "envelope mass the aircraft cannot have",
                set("/envelope/points/0/mass_kg", json!(20000.0)),
                "/envelope/points/0/mass_kg",
                "mass range",
            ),
            (
                "a turn point without its Mach",
                remove("/envelope/points/5", "mach"),
                "/envelope/points/5",
                "has no `mach`",
            ),
            (
                "unknown envelope metric",
                set("/envelope/points/0/metric", json!("speed")),
                "/envelope/points/0/metric",
                "not a known metric",
            ),
            (
                "unknown requirement",
                set("/envelope/points/10/requires", json!("wings")),
                "/envelope/points/10/requires",
                "not a known requirement",
            ),
            (
                "duplicate envelope point",
                set("/envelope/points/1/id", json!("vmax_sl")),
                "/envelope/points/1/id",
                "listed twice",
            ),
            (
                "unknown locale key",
                set("/name_key", json!("ac.nobody.name")),
                "/name_key",
                "not in the default locale",
            ),
            (
                "unknown category",
                set("/category", json!("blimp")),
                "/category",
                "not a known category",
            ),
            (
                "unknown era",
                set("/era", json!("G9")),
                "/era",
                "not a known era",
            ),
            (
                "more precision than stored",
                set("/geometry/span_m", json!(9.800_000_000_001)),
                "/geometry/span_m",
                "more precision",
            ),
            (
                "an unknown key on a gear leg",
                insert("/geometry/gear/0", "wheel_dia_m", json!(0.5)),
                "/geometry/gear/0/wheel_dia_m",
                "unknown key",
            ),
            (
                "a gear leg without its brake flag",
                remove("/geometry/gear/0", "brake"),
                "/geometry/gear/0/brake",
                "missing",
            ),
            (
                "a spring rate below the range",
                set("/geometry/gear/1/k_npm", json!(500.0)),
                "/geometry/gear/1/k_npm",
                "outside the allowed range",
            ),
            (
                "a negative strut travel",
                set("/geometry/gear/2/stroke_m", json!(-0.3)),
                "/geometry/gear/2/stroke_m",
                "outside the allowed range",
            ),
            (
                "a flaps-out load limit above the clean limit",
                set("/aero/n_max_by_config/flaps", json!(12.0)),
                "/aero/n_max_by_config/flaps",
                "above the clean limit",
            ),
            (
                "a control-gains reference of the wrong type",
                set("/fcs/gains_ref", json!("core:aero/kestrel")),
                "/fcs/gains_ref",
                "not a valid control-gains id",
            ),
            (
                "a gear that takes no time",
                set("/fcs/actuators/gear/transit_s", json!(0.0)),
                "/fcs/actuators/gear/transit_s",
                "outside the allowed range",
            ),
        ],
    );
}

#[test]
fn malformed_fcs_gains_are_rejected_at_the_right_pointer() {
    expect_cases(
        FCS,
        "",
        vec![
            (
                "id of another type",
                set("/id", json!("core:ac/kestrel")),
                "/id",
                "not a valid control-gains id",
            ),
            (
                "wrong format tag",
                set("/schema", json!("fcs/2")),
                "/schema",
                "not a control-gains file",
            ),
            (
                "unknown locale key",
                set("/name_key", json!("fcs.nobody.name")),
                "/name_key",
                "not in the default locale",
            ),
            (
                "unknown key",
                insert("/pitch", "k_x", json!(1.0)),
                "/pitch/k_x",
                "unknown key",
            ),
            (
                "missing key",
                remove("/roll", "k_p_per_s"),
                "/roll/k_p_per_s",
                "missing",
            ),
            (
                "a gain above its range",
                set("/pitch/k_q_per_s", json!(99.0)),
                "/pitch/k_q_per_s",
                "outside the allowed range",
            ),
            (
                "wrong axis",
                set("/pitch/q_max_dps/axis", json!("mach")),
                "/pitch/q_max_dps/axis",
                "this table is against",
            ),
            (
                "schedule not increasing",
                set("/roll/p_max_qbar/points/2/0", json!(2.0)),
                "/roll/p_max_qbar/points/2",
                "not strictly increasing",
            ),
            (
                "a sideslip limit of zero",
                set("/yaw/beta_max_deg/points/2/1", json!(0.0)),
                "/yaw/beta_max_deg/points/2",
                "not positive",
            ),
            (
                "a roll authority fraction above 1.5",
                set("/roll/p_max_alpha/points/0/1", json!(2.0)),
                "/roll/p_max_alpha/points/0",
                "at most 1.5",
            ),
            (
                "steering speeds reversed",
                set("/ground/nws/low_speed_mps", json!(40.0)),
                "/ground/nws/high_speed_mps",
                "not an interval",
            ),
            (
                "steering range growing with speed",
                set("/ground/nws/low_deg", json!(3.0)),
                "/ground/nws/high_deg",
                "grows with speed",
            ),
            (
                "a look-ahead of too many steps",
                set("/gcas/step_s", json!(0.01)),
                "/gcas/step_s",
                "more than 200 steps",
            ),
            (
                "more precision than stored",
                set("/assist_blend_s", json!(0.500_000_000_1)),
                "/assist_blend_s",
                "more precision",
            ),
        ],
    );
}

#[test]
fn malformed_engines_are_rejected_at_the_right_pointer() {
    expect_cases(
        F129,
        "",
        vec![
            (
                "id of another type",
                set("/id", json!("core:ac/f129")),
                "/id",
                "not a valid engine id",
            ),
            (
                "wrong format tag",
                set("/schema", json!("engine/2")),
                "/schema",
                "not an engine file",
            ),
            (
                "unknown kind",
                set("/kind", json!("rocket")),
                "/kind",
                "not a known engine kind",
            ),
            (
                "unknown locale key",
                set("/name_key", json!("eng.nobody.name")),
                "/name_key",
                "not in the default locale",
            ),
            (
                "afterburner weaker than dry",
                set("/thrust/sl_ab_n", json!(50000.0)),
                "/thrust/sl_ab_n",
                "below military",
            ),
            (
                "stages without an afterburner",
                set("/thrust/sl_ab_n", json!(79000.0)),
                "/dynamics/ab_stages",
                "afterburner stages",
            ),
            (
                "afterburner TSFC below dry",
                set("/tsfc/ab_kg_per_ns", json!(1.0e-5)),
                "/tsfc/ab_kg_per_ns",
                "below dry",
            ),
            (
                "wrong axis",
                set("/thrust/mach_factor_dry/axis", json!("alpha_deg")),
                "/thrust/mach_factor_dry/axis",
                "not `alpha_deg`",
            ),
            (
                "grid not increasing",
                set("/thrust/mach_factor_dry/points/2/0", json!(0.4)),
                "/thrust/mach_factor_dry/points/2",
                "not strictly increasing",
            ),
            (
                "a spike",
                set("/thrust/mach_factor_ab/points/3/1", json!(12.0)),
                "/thrust/mach_factor_ab/points/3",
                "spike",
            ),
            (
                "a negative thrust factor",
                set("/thrust/mach_factor_dry/points/0/1", json!(-0.5)),
                "/thrust/mach_factor_dry/points/0",
                "not positive",
            ),
        ],
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "a table of cases; splitting it would hide the table"
)]
fn malformed_aero_tables_are_rejected_at_the_right_pointer() {
    let drop_last_of_row: Edit = Box::new(|v| {
        v.pointer_mut("/lift/cl_base/z/3")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .pop();
    });
    expect_cases(
        AERO,
        "",
        vec![
            (
                "id of another type",
                set("/id", json!("core:eng/kestrel")),
                "/id",
                "not a valid aero-table id",
            ),
            (
                "wrong axes",
                set("/lift/cl_base/axes", json!(["mach", "alpha_deg"])),
                "/lift/cl_base/axes",
                "this table is against",
            ),
            (
                "short row",
                drop_last_of_row,
                "/lift/cl_base/z/3",
                "values for 31 x values",
            ),
            (
                "alpha grid not increasing",
                set("/pitch/cm_0/x/5", json!(-10.0)),
                "/pitch/cm_0/x/5",
                "not strictly increasing",
            ),
            (
                "alpha grid too short",
                set("/lift/cl_base/x/0", json!(-25.0)),
                "/lift/cl_base/x",
                "alpha covers",
            ),
            (
                "lift peak far above C_L,max",
                set("/lift/cl_base/z/0/21", json!(3.0)),
                "/lift/cl_base/z/0/21",
                "times the trimmed",
            ),
            (
                "blend inside the stall",
                set("/post_stall/blend_start_deg/points/0/1", json!(30.0)),
                "/post_stall/blend_start_deg",
                "within 5 deg",
            ),
            (
                "a lateral table against the wrong axis",
                set("/lateral/cn_beta_per_rad/axis", json!("mach")),
                "/lateral/cn_beta_per_rad/axis",
                "this table is against",
            ),
            (
                "a lateral table that stops short of 90 degrees",
                set("/lateral/cl_p_per_rad/points/4/0", json!(80.0)),
                "/lateral/cl_p_per_rad/points",
                "covers",
            ),
            (
                "yaw damping that is not negative at zero alpha",
                Box::new(|v| {
                    for i in [0, 1] {
                        v["lateral"]["cn_r_per_rad"]["points"][i][1] = json!(0.1);
                    }
                }),
                "/lateral/cn_r_per_rad/points/0",
                "not negative",
            ),
            (
                "a lateral value beyond the bound",
                set("/lateral/cy_beta_per_rad/points/1/1", json!(12.0)),
                "/lateral/cy_beta_per_rad/points/1",
                "outside the allowed range",
            ),
            (
                "negative zero-lift drag",
                set("/drag/cd0/points/0/1", json!(-0.01)),
                "/drag/cd0/points/0",
                "not positive",
            ),
            (
                "control power above one",
                set("/control_effectiveness/points/0/1", json!(1.5)),
                "/control_effectiveness/points/0",
                "in (0, 1]",
            ),
            (
                "a spike in the wave drag",
                set("/drag/cd_wave/points/4/1", json!(0.5)),
                "/drag/cd_wave/points/4",
                "spike",
            ),
        ],
    );
}

#[test]
fn cross_file_problems_are_reported_against_the_aircraft() {
    expect_cases(
        KESTREL,
        "test.json",
        vec![
            (
                "missing engine",
                set("/engines/0/ref", json!("core:eng/nope")),
                "/engines/0/ref",
                "no engine",
            ),
            (
                "missing aero tables",
                set("/aero/tables_ref", json!("core:aero/nope")),
                "/aero/tables_ref",
                "no aero tables",
            ),
            (
                "missing control gains",
                set("/fcs/gains_ref", json!("core:fcs/nope")),
                "/fcs/gains_ref",
                "no control gains",
            ),
            (
                "a tail that scrapes at the touchdown attitude",
                set("/geometry/contacts/1/pos_m", json!([-6.0, 0.0, 1.0])),
                "/geometry/contacts/1/pos_m",
                "touches the ground",
            ),
            (
                "far too much thrust",
                set("/engines/0/count", json!(8)),
                "/engines",
                "thrust-to-weight",
            ),
        ],
    );
    // A pitching moment that rises with alpha is unstable; the aircraft must then require SAS.
    let mut aero = json_of(AERO);
    *aero.pointer_mut("/pitch/cm_0/z/0/7").unwrap() = json!(0.02);
    let problems = bake_files(
        &[(KESTREL, read(KESTREL))],
        &[(F129, read(F129))],
        &[(AERO, serde_json::to_string(&aero).unwrap())],
        &[(FCS, read(FCS))],
    )
    .unwrap_err();
    let found: Vec<(&str, &str)> = problems
        .iter()
        .map(|p| (p.file.as_str(), p.pointer.as_str()))
        .collect();
    assert_eq!(found, [(KESTREL, "/fcs/sas_required")], "{problems:#?}");
    assert!(problems[0].message.contains("rises with angle of attack"));
}

#[test]
fn syntax_errors_are_reported_for_the_whole_file() {
    for text in ["", "{", "{\"id\": }", "[1, 2"] {
        let problems = bake_replacing(KESTREL, "test.json", text).unwrap_err();
        assert_eq!(problems.len(), 1, "{text:?}");
        assert_eq!(problems[0].pointer, "", "{text:?}");
        assert!(
            problems[0].message.starts_with("not valid JSON"),
            "{text:?}"
        );
    }
}

#[test]
fn every_problem_in_a_file_is_reported_together() {
    let mut v = json_of(KESTREL);
    v["id"] = json!("bad");
    v["mass"]["empty_kg"] = json!(-1.0);
    v["aero"]["n_min"] = json!(3.0);
    let problems = bake_replacing(KESTREL, "test.json", &v.to_string()).unwrap_err();
    let mut pointers: Vec<&str> = problems.iter().map(|p| p.pointer.as_str()).collect();
    pointers.sort_unstable();
    assert_eq!(pointers, ["/aero/n_min", "/id", "/mass/empty_kg"]);
}

#[test]
fn problems_in_several_files_are_all_reported() {
    let mut a = json_of(KESTREL);
    a["id"] = json!("bad");
    let mut e = json_of(F129);
    e["kind"] = json!("rocket");
    let problems = bake_files(
        &[("aircraft/a.json", a.to_string())],
        &[("engines/b.json", e.to_string())],
        &[(AERO, read(AERO))],
        &[(FCS, read(FCS))],
    )
    .unwrap_err();
    let found: Vec<(&str, &str)> = problems
        .iter()
        .map(|p| (p.file.as_str(), p.pointer.as_str()))
        .collect();
    assert_eq!(
        found,
        [("aircraft/a.json", "/id"), ("engines/b.json", "/kind")]
    );
}

#[test]
fn duplicate_ids_across_files_are_refused() {
    let k = read(KESTREL);
    let problems = bake_files(
        &[("aircraft/a.json", k.clone()), ("aircraft/b.json", k)],
        &[(F129, read(F129))],
        &[(AERO, read(AERO))],
        &[(FCS, read(FCS))],
    )
    .unwrap_err();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].file, "aircraft/b.json");
    assert_eq!(problems[0].pointer, "/id");
    assert!(problems[0].message.contains("aircraft/a.json"));
}

#[test]
fn unused_locale_keys_are_refused() {
    let problems = bake_sources(&Sources {
        pack: ("pack.json".into(), read("pack.json")),
        locale: (
            "locales/en.json".into(),
            r#"{ "ac.kestrel.name": "Kestrel", "eng.f129.name": "F129", "fcs.kestrel.name": "FCS", "ac.ghost.name": "Ghost" }"#
                .into(),
        ),
        aircraft: vec![(KESTREL.into(), read(KESTREL))],
        engines: vec![(F129.into(), read(F129))],
        aero: vec![(AERO.into(), read(AERO))],
        fcs: vec![(FCS.into(), read(FCS))],
    })
    .unwrap_err();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].file, "locales/en.json");
    assert_eq!(problems[0].pointer, "/ac.ghost.name");
}

#[test]
fn a_pack_with_no_entities_and_no_locale_keys_is_valid_and_empty() {
    let baked = bake_sources(&Sources {
        pack: ("pack.json".into(), read("pack.json")),
        locale: ("locales/en.json".into(), "{}".into()),
        aircraft: vec![],
        engines: vec![],
        aero: vec![],
        fcs: vec![],
    })
    .unwrap();
    assert!(Registry::from_cpack(&baked.cpack).unwrap().is_empty());
}

#[test]
fn bad_pack_manifests_and_locales_are_refused() {
    let try_pack = |pack: &str| {
        bake_sources(&Sources {
            pack: ("pack.json".into(), pack.into()),
            locale: ("locales/en.json".into(), read("locales/en.json")),
            aircraft: vec![(KESTREL.into(), read(KESTREL))],
            engines: vec![(F129.into(), read(F129))],
            aero: vec![(AERO.into(), read(AERO))],
            fcs: vec![(FCS.into(), read(FCS))],
        })
        .unwrap_err()
    };
    let good = json_of("pack.json");
    for (key, value, pointer) in [
        ("id", json!("Core"), "/id"),
        ("version", json!("1.0"), "/version"),
        ("schema", json!("pack/9"), "/schema"),
        ("authors", json!([]), "/authors"),
    ] {
        let mut p = good.clone();
        p[key] = value;
        let problems = try_pack(&p.to_string());
        assert!(
            problems
                .iter()
                .any(|d| d.pointer == pointer && d.file == "pack.json"),
            "{key}: {problems:?}"
        );
    }
    let mut p = good;
    p["surprise"] = json!(1);
    assert!(
        try_pack(&p.to_string())
            .iter()
            .any(|d| d.pointer == "/surprise")
    );

    let problems = bake_sources(&Sources {
        pack: ("pack.json".into(), read("pack.json")),
        locale: (
            "locales/en.json".into(),
            r#"{ "ac.kestrel.name": "  ", "eng.f129.name": "F129", "fcs.kestrel.name": "FCS" }"#
                .into(),
        ),
        aircraft: vec![(KESTREL.into(), read(KESTREL))],
        engines: vec![(F129.into(), read(F129))],
        aero: vec![(AERO.into(), read(AERO))],
        fcs: vec![(FCS.into(), read(FCS))],
    })
    .unwrap_err();
    assert_eq!(problems[0].pointer, "/ac.kestrel.name");
}

#[test]
fn a_missing_input_file_is_reported_with_its_name() {
    let dir = std::env::temp_dir().join(format!("content-bake-missing-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let problems = bake_dir(&dir).unwrap_err();
    assert_eq!(problems[0].file, "pack.json");
    assert!(problems[0].message.starts_with("cannot read"));
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_checked_in_schemas_are_current() {
    let stale = schema::stale(&root().join("content/schemas"));
    assert!(
        stale.is_empty(),
        "run `cargo run -p content-bake -- schema content/schemas` and commit: {stale:?}"
    );
}
