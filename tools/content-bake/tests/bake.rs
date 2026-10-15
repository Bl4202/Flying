//! End-to-end tests of the baker against the real core pack and deliberately broken variants of it.

use std::fs;
use std::path::{Path, PathBuf};

use content::{CpackError, EngineKind, Metric, Registry, Requirement};
use content_bake::bake::{self, Baked, Sources, bake_dir, bake_sources, sha256};
use content_bake::diag::Diagnostic;
use content_bake::export::export;
use content_bake::model::{AeroFile, AircraftFile, EngineFile};
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

/// Bakes the real pack with the given files in place of its entity files.
fn bake_files(
    aircraft: &[(&str, String)],
    engines: &[(&str, String)],
    aero: &[(&str, String)],
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
    )
}

#[test]
fn the_core_pack_bakes_and_holds_the_kestrel() {
    let baked = bake_dir(&core()).expect("the core pack is valid");
    let reg = Registry::from_cpack(&baked.cpack).unwrap();
    assert_eq!(reg.counts(), (1, 1, 1));
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
    assert_eq!(exported.engines, vec![engine]);
    assert_eq!(exported.aero, vec![aero]);
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
        json!({ "aircraft": 1, "engines": 1, "aero": 1 })
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
            r#"{ "ac.kestrel.name": "Kestrel", "eng.f129.name": "F129", "ac.ghost.name": "Ghost" }"#
                .into(),
        ),
        aircraft: vec![(KESTREL.into(), read(KESTREL))],
        engines: vec![(F129.into(), read(F129))],
        aero: vec![(AERO.into(), read(AERO))],
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
            r#"{ "ac.kestrel.name": "  ", "eng.f129.name": "F129" }"#.into(),
        ),
        aircraft: vec![(KESTREL.into(), read(KESTREL))],
        engines: vec![(F129.into(), read(F129))],
        aero: vec![(AERO.into(), read(AERO))],
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
