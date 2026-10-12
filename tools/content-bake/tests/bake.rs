//! End-to-end tests of the baker against the real core pack and deliberately broken variants of it.

use std::fs;
use std::path::{Path, PathBuf};

use content::{CpackError, Registry};
use content_bake::bake::{self, Baked, Sources, bake_dir, bake_sources, sha256};
use content_bake::diag::Diagnostic;
use content_bake::export::export;
use content_bake::model::AircraftFile;
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

fn kestrel() -> Value {
    serde_json::from_str(&read("aircraft/kestrel.json")).unwrap()
}

/// Bakes the real pack with `aircraft` in place of its aircraft files.
fn bake_with(aircraft: &[(&str, String)]) -> Result<Baked, Vec<Diagnostic>> {
    bake_sources(&Sources {
        pack: ("pack.json".into(), read("pack.json")),
        locale: ("locales/en.json".into(), read("locales/en.json")),
        aircraft: aircraft
            .iter()
            .map(|(n, t)| ((*n).to_owned(), t.clone()))
            .collect(),
    })
}

fn bake_one(text: String) -> Result<Baked, Vec<Diagnostic>> {
    bake_with(&[("aircraft/test.json", text)])
}

#[test]
fn the_core_pack_bakes_and_holds_the_kestrel() {
    let baked = bake_dir(&core()).expect("the core pack is valid");
    let reg = Registry::from_cpack(&baked.cpack).unwrap();
    assert_eq!(reg.len(), 1);
    let h = reg.find("core:ac/kestrel").expect("kestrel is in the pack");
    assert_eq!(reg.id(h), Some("core:ac/kestrel"));
    let k = reg.aircraft(h).unwrap();

    // Appendix C.1-C.6.
    assert_eq!(k.name_key, "ac.kestrel.name");
    assert_eq!(k.category, content::Category::LightFighter);
    assert_eq!(k.era, content::Era::G4);
    assert_eq!((k.crew, k.engine_count, k.hardpoints), (1, 1, 9));
    assert!((k.empty_kg - 8400.0).abs() < f32::EPSILON);
    assert!((k.internal_fuel_kg - 3100.0).abs() < f32::EPSILON);
    assert!((k.max_takeoff_kg - 14500.0).abs() < f32::EPSILON);
    assert!((k.wing_area_m2 - 28.0).abs() < f32::EPSILON);
    assert!((k.span_m - 9.8).abs() < 1e-6);
    assert!((k.mac_m - 3.2).abs() < 1e-6);
    assert_eq!(
        (
            k.inertia_xx_kgm2,
            k.inertia_yy_kgm2,
            k.inertia_zz_kgm2,
            k.inertia_xz_kgm2
        ),
        (13000.0, 76000.0, 86000.0, 1300.0)
    );
    assert_eq!((k.n_max, k.n_min, k.mmo, k.vmo_ms), (9.0, -3.0, 2.0, 360.0));
    assert!((k.service_ceiling_m - 15500.0).abs() < f32::EPSILON);
    assert_eq!((k.thrust_mil_n, k.thrust_ab_n), (79000.0, 128_000.0));
    assert!(!k.sas_required);

    // The reference weight of Appendix C.2 gives its quoted T/W of 1.243 at 10 500 kg.
    let t_w = f64::from(k.thrust_ab_n) / (10_500.0 * 9.806_65);
    assert!((t_w - 1.243).abs() < 0.001, "{t_w}");
}

#[test]
fn export_gives_back_the_normalised_input() {
    let baked = bake_dir(&core()).unwrap();
    let exported = export(&baked.cpack).unwrap();
    // The source file, read back through the same typed model the baker uses.
    let source: AircraftFile = serde_json::from_str(&read("aircraft/kestrel.json")).unwrap();
    assert_eq!(exported.aircraft, vec![source]);
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
    // Same data in a differently ordered, differently formatted file still bakes to the same bytes.
    let reformatted = serde_json::to_string(&kestrel()).unwrap();
    let c = bake_one(reformatted).unwrap();
    let a_body = &a.cpack[content::cpack::HEADER_BYTES..];
    let c_body = &c.cpack[content::cpack::HEADER_BYTES..];
    assert_eq!(a_body, c_body);
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
    // A flip anywhere in the record section: the layout is still valid, only the checksum notices.
    let records_at = baked.cpack.len() - 144 + 16;
    let mut bad = baked.cpack.clone();
    bad[records_at] ^= 0x40;
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
    assert_eq!(m["packs"][0]["entities"]["aircraft"], 1);
}

// ---------------------------------------------------------------------------------------------
// Malformed input: each case breaks one thing and must be reported at exactly that pointer.

fn edited(f: impl FnOnce(&mut Value)) -> String {
    let mut v = kestrel();
    f(&mut v);
    serde_json::to_string_pretty(&v).unwrap()
}

fn set(ptr: &'static str, value: Value) -> impl FnOnce(&mut Value) {
    move |v| *v.pointer_mut(ptr).unwrap_or_else(|| panic!("no {ptr}")) = value
}

fn remove(parent: &'static str, key: &'static str) -> impl FnOnce(&mut Value) {
    move |v| {
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key)
            .unwrap();
    }
}

fn insert(parent: &'static str, key: &'static str, value: Value) -> impl FnOnce(&mut Value) {
    move |v| {
        v.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.into(), value);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "a table of cases; splitting it would hide the table"
)]
fn malformed_aircraft_are_rejected_at_the_right_pointer() {
    type Edit = Box<dyn FnOnce(&mut Value)>;
    let cases: Vec<(&str, Edit, &str, &str)> = vec![
        (
            "id is not a pattern",
            Box::new(set("/id", json!("Kestrel"))),
            "/id",
            "not a valid aircraft id",
        ),
        (
            "id in another namespace",
            Box::new(set("/id", json!("other:ac/kestrel"))),
            "/id",
            "not a valid aircraft id",
        ),
        (
            "id of another type",
            Box::new(set("/id", json!("core:wpn/kestrel"))),
            "/id",
            "not a valid aircraft id",
        ),
        (
            "wrong format tag",
            Box::new(set("/schema", json!("aircraft/2"))),
            "/schema",
            "not an aircraft file",
        ),
        (
            "unknown key",
            Box::new(insert("/mass", "payload_kg", json!(5.0))),
            "/mass/payload_kg",
            "unknown key",
        ),
        (
            "unknown top-level key",
            Box::new(insert("", "colour", json!("red"))),
            "/colour",
            "unknown key",
        ),
        (
            "missing key",
            Box::new(remove("/geometry", "mac_m")),
            "/geometry/mac_m",
            "missing",
        ),
        (
            "number given as text",
            Box::new(set("/mass/empty_kg", json!("heavy"))),
            "/mass/empty_kg",
            "invalid type",
        ),
        (
            "array of the wrong size",
            Box::new(set("/mass/inertia_kgm2", json!([1.0, 2.0]))),
            "/mass/inertia_kgm2",
            "invalid length",
        ),
        (
            "negative mass",
            Box::new(set("/mass/empty_kg", json!(-5.0))),
            "/mass/empty_kg",
            "outside the allowed range",
        ),
        (
            "zero fuel",
            Box::new(set("/mass/internal_fuel_kg", json!(0.0))),
            "/mass/internal_fuel_kg",
            "outside the allowed range",
        ),
        (
            "take-off mass below dry mass plus fuel",
            Box::new(set("/mass/max_takeoff_kg", json!(10000.0))),
            "/mass/max_takeoff_kg",
            "below empty mass plus internal fuel",
        ),
        (
            "wing far too large",
            Box::new(set("/geometry/wing_area_m2", json!(500.0))),
            "/geometry/wing_area_m2",
            "wing loading",
        ),
        (
            "span below chord",
            Box::new(set("/geometry/span_m", json!(3.0))),
            "/geometry/span_m",
            "not larger than the chord",
        ),
        (
            "inertia breaks the triangle inequality",
            Box::new(set("/mass/inertia_kgm2", json!([1000.0, 1000.0, 90000.0]))),
            "/mass/inertia_kgm2",
            "rigid body",
        ),
        (
            "zero inertia",
            Box::new(set("/mass/inertia_kgm2/1", json!(0.0))),
            "/mass/inertia_kgm2/1",
            "not positive",
        ),
        (
            "product of inertia too large",
            Box::new(set("/mass/inertia_xz_kgm2", json!(40000.0))),
            "/mass/inertia_xz_kgm2",
            "positive definite",
        ),
        (
            "negative load limit above zero",
            Box::new(set("/limits/n_min", json!(2.0))),
            "/limits/n_min",
            "outside the allowed range",
        ),
        (
            "positive load limit below zero",
            Box::new(set("/limits/n_max", json!(-1.0))),
            "/limits/n_max",
            "outside the allowed range",
        ),
        (
            "afterburner weaker than military",
            Box::new(set("/engines/thrust_afterburner_n", json!(50000.0))),
            "/engines/thrust_afterburner_n",
            "below military",
        ),
        (
            "engine far too weak",
            Box::new(|v| {
                v["engines"]["thrust_military_n"] = json!(15000.0);
                v["engines"]["thrust_afterburner_n"] = json!(20000.0);
            }),
            "/engines/thrust_afterburner_n",
            "thrust-to-weight",
        ),
        (
            "engine far too strong",
            Box::new(set("/engines/thrust_afterburner_n", json!(400_000.0))),
            "/engines/thrust_afterburner_n",
            "thrust-to-weight",
        ),
        (
            "unstable airframe without SAS",
            Box::new(set("/aero/static_margin_mac", json!(-0.05))),
            "/aero/sas_required",
            "unstable",
        ),
        (
            "unknown locale key",
            Box::new(set("/name_key", json!("ac.nobody.name"))),
            "/name_key",
            "not in the default locale",
        ),
        (
            "unknown category",
            Box::new(set("/category", json!("blimp"))),
            "/category",
            "not a known category",
        ),
        (
            "unknown era",
            Box::new(set("/era", json!("G9"))),
            "/era",
            "not a known era",
        ),
        (
            "more precision than stored",
            Box::new(set("/geometry/span_m", json!(9.800_000_000_001))),
            "/geometry/span_m",
            "more precision",
        ),
    ];
    for (what, edit, pointer, message) in cases {
        let problems = bake_one(edited(edit)).expect_err(what);
        let pointers: Vec<&str> = problems.iter().map(|p| p.pointer.as_str()).collect();
        assert_eq!(pointers, [pointer], "{what}: {problems:#?}");
        let p = &problems[0];
        assert!(
            p.message.contains(message),
            "{what}: message `{}` lacks `{message}`",
            p.message
        );
        assert_eq!(p.file, "aircraft/test.json", "{what}");
        assert!(!p.hint.is_empty(), "{what}: every error says how to fix it");
    }
}

#[test]
fn syntax_errors_are_reported_for_the_whole_file() {
    for text in ["", "{", "{\"id\": }", "[1, 2"] {
        let problems = bake_one(text.to_owned()).unwrap_err();
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
    let text = edited(|v| {
        v["id"] = json!("bad");
        v["mass"]["empty_kg"] = json!(-1.0);
        v["limits"]["n_min"] = json!(3.0);
    });
    let problems = bake_one(text).unwrap_err();
    let mut pointers: Vec<&str> = problems.iter().map(|p| p.pointer.as_str()).collect();
    pointers.sort_unstable();
    assert_eq!(pointers, ["/id", "/limits/n_min", "/mass/empty_kg"]);
}

#[test]
fn problems_in_several_files_are_all_reported() {
    let a = edited(set("/id", json!("bad")));
    let b = edited(set("/limits/n_min", json!(3.0)));
    let problems = bake_with(&[("aircraft/a.json", a), ("aircraft/b.json", b)]).unwrap_err();
    let found: Vec<(&str, &str)> = problems
        .iter()
        .map(|p| (p.file.as_str(), p.pointer.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            ("aircraft/a.json", "/id"),
            ("aircraft/b.json", "/limits/n_min")
        ]
    );
}

#[test]
fn duplicate_ids_across_files_are_refused() {
    let k = serde_json::to_string(&kestrel()).unwrap();
    let problems =
        bake_with(&[("aircraft/a.json", k.clone()), ("aircraft/b.json", k)]).unwrap_err();
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
            r#"{ "ac.kestrel.name": "Kestrel", "ac.ghost.name": "Ghost" }"#.into(),
        ),
        aircraft: vec![(
            "aircraft/kestrel.json".into(),
            read("aircraft/kestrel.json"),
        )],
    })
    .unwrap_err();
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].file, "locales/en.json");
    assert_eq!(problems[0].pointer, "/ac.ghost.name");
}

#[test]
fn a_pack_with_no_aircraft_and_no_locale_keys_is_valid_and_empty() {
    let baked = bake_sources(&Sources {
        pack: ("pack.json".into(), read("pack.json")),
        locale: ("locales/en.json".into(), "{}".into()),
        aircraft: vec![],
    })
    .unwrap();
    assert!(Registry::from_cpack(&baked.cpack).unwrap().is_empty());
}

#[test]
fn bad_pack_manifests_and_locales_are_refused() {
    let k = read("aircraft/kestrel.json");
    let try_pack = |pack: &str| {
        bake_sources(&Sources {
            pack: ("pack.json".into(), pack.into()),
            locale: ("locales/en.json".into(), read("locales/en.json")),
            aircraft: vec![("aircraft/kestrel.json".into(), k.clone())],
        })
        .unwrap_err()
    };
    let good: Value = serde_json::from_str(&read("pack.json")).unwrap();
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
            r#"{ "ac.kestrel.name": "  " }"#.into(),
        ),
        aircraft: vec![("aircraft/kestrel.json".into(), k)],
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
