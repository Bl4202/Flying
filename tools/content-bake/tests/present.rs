//! The presentation bundle (ADR-43, ADR-39 U2): the Kestrel's HUD, cockpit, model, font and audio
//! profile bake, break in the ways they should, and never touch the sim pack.

use std::fs;
use std::path::{Path, PathBuf};

use content_bake::bake::{Baked, bake_dir, hex, sha256};
use content_bake::diag::Diagnostic;
use content_bake::present::{
    PresentSources, bake_present, bake_present_dir, load_audio, load_cockpit, load_font, load_hud,
    load_model,
};
use serde_json::{Value, json};

fn core() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core")
}

fn read(rel: &str) -> String {
    fs::read_to_string(core().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn value(rel: &str) -> Value {
    serde_json::from_str(&read(rel)).unwrap()
}

const HUD: &str = "hud/kestrel.json";
const COCKPIT: &str = "cockpit/kestrel.json";
const MODEL: &str = "models/kestrel.json";
const FONT: &str = "fonts/hudstroke.json";
const AUDIO: &str = "audio/kestrel.json";

fn baked() -> Baked {
    bake_dir(&core()).expect("the core pack bakes")
}

/// The real presentation files, with the named ones replaced by the given text.
fn sources(replace: &[(&str, String)]) -> PresentSources {
    let pick = |rel: &str| {
        let text = replace
            .iter()
            .find(|(r, _)| *r == rel)
            .map_or_else(|| read(rel), |(_, t)| t.clone());
        vec![(rel.to_owned(), text)]
    };
    PresentSources {
        hud: pick(HUD),
        cockpit: pick(COCKPIT),
        audio: pick(AUDIO),
        models: pick(MODEL),
        fonts: pick(FONT),
    }
}

fn edited(rel: &str, edit: impl FnOnce(&mut Value)) -> (String, String) {
    let mut v = value(rel);
    edit(&mut v);
    (rel.to_owned(), serde_json::to_string_pretty(&v).unwrap())
}

fn bake_with(rel: &str, edit: impl FnOnce(&mut Value)) -> Result<(), Vec<Diagnostic>> {
    let (name, text) = edited(rel, edit);
    bake_present(&sources(&[(&name, text)]), &baked()).map(|_| ())
}

/// A failure that names `file`, points at `pointer` (exactly, or by prefix if it ends in `/`) and
/// says what to do about it.
fn assert_fails_at(result: Result<(), Vec<Diagnostic>>, file: &str, pointer: &str, what: &str) {
    let problems = result.expect_err(what);
    let hit = problems.iter().find(|d| {
        d.file == file
            && (d.pointer == pointer || (pointer.ends_with('/') && d.pointer.starts_with(pointer)))
    });
    let d = hit.unwrap_or_else(|| panic!("{what}: nothing at {file} {pointer} in {problems:#?}"));
    assert!(!d.message.is_empty() && !d.hint.is_empty(), "{what}: {d:?}");
    assert!(
        d.to_string().contains(file),
        "{what}: the report names the file"
    );
}

#[test]
fn the_kestrels_presentation_files_all_load() {
    let pack = "core";
    assert!(load_hud(HUD, &read(HUD), pack).is_ok());
    assert!(load_cockpit(COCKPIT, &read(COCKPIT), pack).is_ok());
    assert!(load_model(MODEL, &read(MODEL), pack).is_ok());
    assert!(load_font(FONT, &read(FONT), pack).is_ok());
    assert!(load_audio(AUDIO, &read(AUDIO), pack).is_ok());
}

#[test]
fn the_bundle_is_canonical_and_deterministic() {
    let b = baked();
    let one = bake_present(&sources(&[]), &b).expect("bakes");
    let two = bake_present(&sources(&[]), &b).expect("bakes");
    assert_eq!(one.json, two.json, "the same files give the same bytes");
    assert_eq!(one.sha256, sha256(&one.json));
    let v: Value = serde_json::from_slice(&one.json).unwrap();
    assert_eq!(v["schema"], "present-bundle/1");
    assert_eq!(v["pack"], "core");
    // Canonical: re-serialising the parsed value gives the same bytes (sorted keys, no spaces).
    assert_eq!(serde_json::to_vec(&v).unwrap(), one.json);
    assert_eq!(
        v["aircraft"]["core:ac/kestrel"],
        json!({
            "hud": "core:hud/kestrel",
            "cockpit": "core:cpt/kestrel",
            "model": "core:mdl/kestrel",
            "audio": "core:aud/kestrel",
        })
    );
    for (table, id) in [
        ("huds", "core:hud/kestrel"),
        ("cockpits", "core:cpt/kestrel"),
        ("models", "core:mdl/kestrel"),
        ("audio_profiles", "core:aud/kestrel"),
        ("fonts", "core:fnt/hudstroke"),
    ] {
        assert!(v[table][id].is_object(), "{table} holds {id}");
    }
}

#[test]
fn the_bundle_records_the_sim_pack_it_was_baked_with() {
    let b = baked();
    let p = bake_present(&sources(&[]), &b).expect("bakes");
    let header = &b.cpack[content::cpack::header::CONTENT_HASH..][..32];
    assert_eq!(p.sim_pack_sha256, header);
    let v: Value = serde_json::from_slice(&p.json).unwrap();
    assert_eq!(v["sim_pack_sha256"], hex(header));
    // A different sim pack gives a bundle that says so.
    let manifest = content_bake::present::manifest_entry(&p, "core.present.json");
    assert_eq!(manifest["sim_pack_sha256"], hex(header));
    assert_eq!(manifest["sha256"], hex(&sha256(&p.json)));
}

/// The sim pack's SHA-256 is part of the state hash, so a presentation file that reached it would
/// move all three determinism goldens (ADR-43). This is the pack of v0.1b, byte for byte: the
/// presentation files must not change it. Update it only with a deliberate change to the sim
/// content (and then the goldens move too, which needs the owner).
#[test]
fn the_sim_pack_is_byte_identical_to_v0_1b() {
    let b = baked();
    let header = &b.cpack[content::cpack::header::CONTENT_HASH..][..32];
    assert_eq!(
        hex(header),
        "7d0e9d20ee9e2a443f9c82cda5f6a7d5ea6b9eba010f7b81345463e758029087"
    );
    assert_eq!(b.cpack.len(), 6376);
}

#[test]
fn a_pack_with_no_presentation_files_bakes_none() {
    let dir = std::env::temp_dir().join(format!("present-none-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for rel in [
        "pack.json",
        "locales/en.json",
        "aircraft/kestrel.json",
        "engines/f129.json",
        "aero/kestrel.json",
        "fcs/kestrel.json",
    ] {
        let to = dir.join(rel);
        fs::create_dir_all(to.parent().unwrap()).unwrap();
        fs::copy(core().join(rel), to).unwrap();
    }
    let b = bake_dir(&dir).expect("the sim pack bakes alone");
    assert!(bake_present_dir(&dir, &b).unwrap().is_none());
    let _ = fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------------------------
// ADR-39 U2: the shipped HUD passes and 40 invalid layouts fail, each with file, pointer and hint.

fn layer(v: &Value, kind: &str) -> usize {
    v["layers"]
        .as_array()
        .unwrap()
        .iter()
        .position(|l| l["type"] == kind)
        .unwrap_or_else(|| panic!("no {kind} layer"))
}

fn set(v: &mut Value, path: &str, to: Value) {
    *v.pointer_mut(path)
        .unwrap_or_else(|| panic!("no value at {path}")) = to;
}

fn remove(v: &mut Value, parent: &str, key: &str) {
    v.pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(key)
        .unwrap_or_else(|| panic!("no {key} in {parent}"));
}

type Case = (&'static str, fn(&mut Value) -> String);

#[allow(
    clippy::too_many_lines,
    reason = "a table of 42 cases reads best as one list"
)]
fn invalid_huds() -> Vec<Case> {
    vec![
        // Structure.
        ("an unknown top-level key", |v| {
            v["flavour"] = json!("x");
            "/flavour".into()
        }),
        ("a missing layers list", |v| {
            remove(v, "", "layers");
            "/layers".into()
        }),
        ("a field of view that is text", |v| {
            v["tfov_deg"] = json!("28");
            "/tfov_deg".into()
        }),
        ("an unknown element type", |v| {
            let i = layer(v, "speed_tape");
            set(v, &format!("/layers/{i}/type"), json!("radar_scope"));
            "/layers/".into()
        }),
        ("an unknown key in an element", |v| {
            let i = layer(v, "boresight");
            v["layers"][i]["colour"] = json!("#fff");
            format!("/layers/{i}/colour")
        }),
        ("a missing key in an element", |v| {
            let i = layer(v, "boresight");
            remove(v, &format!("/layers/{i}"), "size_mrad");
            format!("/layers/{i}/size_mrad")
        }),
        ("an element with no type", |v| {
            let i = layer(v, "boresight");
            remove(v, &format!("/layers/{i}"), "type");
            "/layers/".into()
        }),
        ("the wrong format tag", |v| {
            v["schema"] = json!("hud/2");
            "/schema".into()
        }),
        // Numeric ranges, from the schema.
        ("a stroke that is far too wide", |v| {
            v["stroke_mrad"] = json!(50);
            "/stroke_mrad".into()
        }),
        ("text that is far too small", |v| {
            v["text_mrad"] = json!(1);
            "/text_mrad".into()
        }),
        ("a negative glow", |v| {
            v["glow_px"] = json!(-1);
            "/glow_px".into()
        }),
        ("a pitch ladder step of zero", |v| {
            let i = layer(v, "pitch_ladder");
            set(v, &format!("/layers/{i}/step_deg"), json!(0));
            format!("/layers/{i}/step_deg")
        }),
        ("five warning lines", |v| {
            let i = layer(v, "warnings");
            set(v, &format!("/layers/{i}/max_lines"), json!(5));
            format!("/layers/{i}/max_lines")
        }),
        ("a speed window of 5 knots", |v| {
            let i = layer(v, "speed_tape");
            set(v, &format!("/layers/{i}/window_kn"), json!(5));
            format!("/layers/{i}/window_kn")
        }),
        ("a Mach threshold of zero", |v| {
            let i = layer(v, "speed_tape");
            set(v, &format!("/layers/{i}/mach_from"), json!(0));
            format!("/layers/{i}/mach_from")
        }),
        ("a cage speed of 500 m/s", |v| {
            let i = layer(v, "flight_path_marker");
            set(v, &format!("/layers/{i}/cage_below_mps"), json!(500));
            format!("/layers/{i}/cage_below_mps")
        }),
        // Identity.
        ("an id in another namespace", |v| {
            v["id"] = json!("other:hud/kestrel");
            "/id".into()
        }),
        ("an id of another kind", |v| {
            v["id"] = json!("core:ac/kestrel");
            "/id".into()
        }),
        ("an aircraft id of another kind", |v| {
            v["aircraft"] = json!("core:hud/kestrel");
            "/aircraft".into()
        }),
        ("a font id of another kind", |v| {
            v["font"] = json!("core:hud/kestrel");
            "/font".into()
        }),
        ("a colour that is a word", |v| {
            v["colour"] = json!("green");
            "/colour".into()
        }),
        ("a colour of five digits", |v| {
            v["colour"] = json!("#35ff6");
            "/colour".into()
        }),
        // Field of view and eye box.
        ("a horizontal field of view of 5 degrees", |v| {
            v["tfov_deg"][0] = json!(5);
            "/tfov_deg/0".into()
        }),
        ("a vertical field of view of 90 degrees", |v| {
            v["tfov_deg"][1] = json!(90);
            "/tfov_deg/1".into()
        }),
        ("an eye box a metre deep", |v| {
            v["eye_box_mm"][2] = json!(1000);
            "/eye_box_mm/2".into()
        }),
        // Layers.
        ("two layers with one name", |v| {
            let i = layer(v, "warnings");
            set(v, &format!("/layers/{i}/id"), json!("boresight"));
            format!("/layers/{i}/id")
        }),
        ("a layer name with a space", |v| {
            let i = layer(v, "heading_tape");
            set(v, &format!("/layers/{i}/id"), json!("Heading Tape"));
            format!("/layers/{i}/id")
        }),
        ("two boresights", |v| {
            let mut second = v["layers"][layer(v, "boresight")].clone();
            second["id"] = json!("boresight_two");
            v["layers"].as_array_mut().unwrap().push(second);
            let n = v["layers"].as_array().unwrap().len() - 1;
            format!("/layers/{n}/type")
        }),
        ("no boresight", |v| {
            let i = layer(v, "boresight");
            v["layers"].as_array_mut().unwrap().remove(i);
            "/layers".into()
        }),
        ("no warnings", |v| {
            let i = layer(v, "warnings");
            v["layers"].as_array_mut().unwrap().remove(i);
            "/layers".into()
        }),
        ("no pitch ladder", |v| {
            let i = layer(v, "pitch_ladder");
            v["layers"].as_array_mut().unwrap().remove(i);
            "/layers".into()
        }),
        // Channels.
        ("an unknown speed channel", |v| {
            let i = layer(v, "speed_tape");
            set(v, &format!("/layers/{i}/channel"), json!("ias"));
            format!("/layers/{i}/channel")
        }),
        ("an unknown altitude channel", |v| {
            let i = layer(v, "altitude_tape");
            set(v, &format!("/layers/{i}/channel"), json!("baro"));
            format!("/layers/{i}/channel")
        }),
        ("an unknown load channel", |v| {
            let i = layer(v, "g_meter");
            set(v, &format!("/layers/{i}/channel"), json!("alpha"));
            format!("/layers/{i}/channel")
        }),
        // Steps and limits.
        ("numbers that are not on ticks", |v| {
            let i = layer(v, "speed_tape");
            set(v, &format!("/layers/{i}/label_kn"), json!(25));
            format!("/layers/{i}/label_kn")
        }),
        ("ticks as wide as the window", |v| {
            let i = layer(v, "altitude_tape");
            set(v, &format!("/layers/{i}/tick_ft"), json!(1000));
            format!("/layers/{i}/tick_ft")
        }),
        ("ladder numbers that are not bars", |v| {
            let i = layer(v, "pitch_ladder");
            set(v, &format!("/layers/{i}/label_step_deg"), json!(7));
            format!("/layers/{i}/label_step_deg")
        }),
        ("amber after red", |v| {
            let i = layer(v, "g_meter");
            set(v, &format!("/layers/{i}/amber_pct"), json!(100));
            format!("/layers/{i}/amber_pct")
        }),
        // Placement.
        ("a heading tape above the field of view", |v| {
            let i = layer(v, "heading_tape");
            set(v, &format!("/layers/{i}/at_mrad"), json!([0, 400]));
            format!("/layers/{i}/at_mrad")
        }),
        ("a speed tape that runs off the field", |v| {
            let i = layer(v, "speed_tape");
            set(v, &format!("/layers/{i}/span_mrad"), json!(480));
            format!("/layers/{i}/at_mrad")
        }),
        ("a bank scale larger than the field", |v| {
            let i = layer(v, "bank_scale");
            set(v, &format!("/layers/{i}/radius_mrad"), json!(300));
            format!("/layers/{i}/radius_mrad")
        }),
        // Precision.
        ("a number that single precision would round", |v| {
            v["glow_px"] = json!(0.123_456_789_1);
            "/glow_px".into()
        }),
    ]
}

#[test]
fn the_shipped_hud_passes() {
    assert!(load_hud(HUD, &read(HUD), "core").is_ok());
}

#[test]
fn forty_invalid_hud_layouts_fail_each_with_file_pointer_and_hint() {
    let cases = invalid_huds();
    assert!(cases.len() >= 40, "the corpus has {} cases", cases.len());
    for (name, mutate) in cases {
        let mut v = value(HUD);
        let pointer = mutate(&mut v);
        let result = load_hud(HUD, &serde_json::to_string(&v).unwrap(), "core").map(|_| ());
        assert_fails_at(result, HUD, &pointer, name);
    }
}

// ---------------------------------------------------------------------------------------------
// Cockpit.

#[test]
fn cockpit_faults_are_caught() {
    let fail = |edit: fn(&mut Value), pointer: &str, what: &str| {
        let (name, text) = edited(COCKPIT, edit);
        assert_fails_at(
            load_cockpit(&name, &text, "core").map(|_| ()),
            &name,
            pointer,
            what,
        );
    };
    fail(
        |v| v["eye_point_m"] = json!([4.3, 0, -0.85]),
        "/glare_shield/min_m",
        "an eye inside the glare shield",
    );
    fail(
        |v| v["panel"]["min_m"][2] = json!(-1.2),
        "/panel/min_m",
        "a panel above the eye",
    );
    fail(
        |v| v["glare_shield"]["min_m"] = json!([3.7, -0.45, -0.72]),
        "/glare_shield/min_m",
        "a glare shield within the near plane",
    );
    fail(
        |v| v["canopy_bows"][1]["top_z_m"] = json!(-0.2),
        "/canopy_bows/1/top_z_m",
        "an arch that does not rise",
    );
    fail(
        |v| {
            v["canopy_bows"][1]["x_m"] = json!(3.6);
            v["canopy_bows"][1]["half_width_m"] = json!(0.2);
        },
        "/canopy_bows/1/x_m",
        "an arch through the eye",
    );
    fail(
        |v| v["hud_glass"]["corners_m"][2] = json!([4.15, 0.2, -0.7525]),
        "/hud_glass/corners_m",
        "glass that is not a rectangle",
    );
    fail(
        |v| {
            for c in v["hud_glass"]["corners_m"].as_array_mut().unwrap() {
                c[0] = json!(3.8);
            }
        },
        "/hud_glass/corners_m",
        "glass nearer than the near plane",
    );
    fail(
        |v| v["head_limits_deg"]["yaw"] = json!(200),
        "/head_limits_deg/yaw",
        "a head that turns past its shoulder",
    );
    fail(
        |v| v["consoles"][1]["id"] = json!("console_l"),
        "/consoles/1/id",
        "two boxes with one name",
    );
    fail(
        |v| v["panel"]["max_m"][0] = json!(4.0),
        "/panel/max_m",
        "a box with no depth",
    );
}

#[test]
fn glass_that_is_too_small_for_the_hud_is_caught_across_files() {
    // 0.1 m wide covers 10 degrees of the 28 the HUD needs.
    let result = bake_with(COCKPIT, |v| {
        let corners = v["hud_glass"]["corners_m"].as_array_mut().unwrap();
        for (c, y) in corners.iter_mut().zip([-0.05, 0.05, 0.05, -0.05]) {
            c[1] = json!(y);
        }
    });
    assert_fails_at(
        result,
        COCKPIT,
        "/hud_glass/corners_m",
        "glass smaller than the field",
    );
}

#[test]
fn an_eye_outside_the_fuselage_is_caught_across_files() {
    let result = bake_with(COCKPIT, |v| v["eye_point_m"] = json!([3.6, 0, -2.5]));
    assert_fails_at(result, COCKPIT, "/eye_point_m", "an eye above the canopy");
}

// ---------------------------------------------------------------------------------------------
// Model.

#[test]
fn model_faults_are_caught() {
    let fail = |edit: fn(&mut Value), pointer: &str, what: &str| {
        let (name, text) = edited(MODEL, edit);
        assert_fails_at(
            load_model(&name, &text, "core").map(|_| ()),
            &name,
            pointer,
            what,
        );
    };
    fail(
        |v| v["parts"][0]["channel"] = json!("elevator_rad"),
        "/parts/0/channel",
        "an unknown channel",
    );
    fail(
        |v| v["parts"][0]["axis"] = json!([0, 0, 0]),
        "/parts/0/axis",
        "a hinge with no direction",
    );
    fail(
        |v| v["parts"][1]["id"] = json!("stabilator_r"),
        "/parts/1/id",
        "two parts with one name",
    );
    fail(
        |v| v["surfaces"][1]["part"] = json!("ghost"),
        "/surfaces/1/part",
        "a surface on a missing part",
    );
    fail(
        |v| v["surfaces"][1]["mirror"] = json!(true),
        "/surfaces/1/mirror",
        "a moving surface that is mirrored",
    );
    fail(
        |v| v["nozzle"]["part"] = json!("rudder"),
        "/nozzle/part",
        "a nozzle lit by a rotating part",
    );
    fail(
        |v| v["gear"][0]["part"] = json!("nozzle_glow"),
        "/gear/0/part",
        "a gear leg on a glow part",
    );
    fail(
        |v| {
            v["parts"].as_array_mut().unwrap().push(json!({
                "motion": "glow", "id": "idle_glow", "channel": "afterburner"
            }));
        },
        "/parts",
        "a part that moves nothing",
    );
    fail(
        |v| {
            v["parts"].as_array_mut().unwrap().last_mut().unwrap()["channel"] = json!("airbrake");
        },
        "/parts/11/channel",
        "a glow that is not the afterburner",
    );
    fail(
        |v| v["fuselage"][2]["x_m"] = json!(6.5),
        "/fuselage/2/x_m",
        "stations out of order",
    );
    fail(
        |v| v["fuselage"][1]["top_z_m"] = json!(2.0),
        "/fuselage/1/top_z_m",
        "a section with no height",
    );
    fail(
        |v| v["canopy"][0]["x_m"] = json!(8.0),
        "/canopy",
        "a canopy past the nose",
    );
    fail(
        |v| v["surfaces"][0]["tip_le_m"] = json!([1.2, 0.75, 0.45]),
        "/surfaces/0/tip_le_m",
        "a tip on its root",
    );
    fail(
        |v| v["boxes"][0]["max_m"][0] = json!(1.0),
        "/boxes/0/max_m",
        "a box with no size",
    );
    fail(
        |v| v["gear"][1]["top_m"][2] = json!(2.5),
        "/gear/1/top_m",
        "a strut below its wheel",
    );
}

#[test]
fn the_models_triangle_estimate_follows_the_builders_rules() {
    // 10 fuselage rings of 16 (2 x 16 x 9 bands + 2 caps of 16), 5 canopy rings of 12, a 12-sided
    // nozzle (3 x 12), eight slabs and one mirrored slab (12 each, the mirror twice), three boxes
    // and three gear legs (a box strut and a 12-sided wheel: 60 each).
    let m = load_model(MODEL, &read(MODEL), "core").unwrap();
    let expected = (2 * 16 * 9 + 2 * 16)
        + (2 * 12 * 4 + 2 * 12)
        + 3 * 12
        + (8 * 12 + 2 * 12)
        + 3 * 12
        + 3 * 60;
    assert_eq!(content_bake::present_check::model_triangles(&m), expected);
    assert_eq!(expected, 812);
    assert!(expected < content_bake::present_check::MAX_MODEL_TRIANGLES);
}

#[test]
fn a_model_that_disagrees_with_the_aircrafts_data_is_caught() {
    let fail = |edit: fn(&mut Value), pointer: &str, what: &str| {
        assert_fails_at(bake_with(MODEL, edit), MODEL, pointer, what);
    };
    fail(
        |v| {
            v["surfaces"][0]["tip_le_m"][1] = json!(3.0);
            v["surfaces"][3]["tip_le_m"][1] = json!(3.0);
            v["surfaces"][4]["tip_le_m"][1] = json!(-3.0);
        },
        "/surfaces",
        "wing tips short of the span",
    );
    fail(
        |v| v["fuselage"][0]["x_m"] = json!(6.5),
        "/fuselage/0/x_m",
        "a nose short of its contact point",
    );
    fail(
        |v| v["nozzle"]["length_m"] = json!(0.3),
        "/nozzle/at_m",
        "a tail short of its contact point",
    );
    fail(
        |v| v["gear"][1]["contact_m"][2] = json!(2.4),
        "/gear/1/contact_m",
        "a wheel off its contact point",
    );
    fail(
        |v| v["gear"][0]["id"] = json!("tail_wheel"),
        "/gear/0/id",
        "a leg the aircraft lacks",
    );
    fail(
        |v| {
            v["gear"].as_array_mut().unwrap().pop();
            v["parts"]
                .as_array_mut()
                .unwrap()
                .retain(|p| p["id"] != "gear_main_r");
        },
        "/gear",
        "a leg missing from the model",
    );
}

// ---------------------------------------------------------------------------------------------
// Font and audio.

#[test]
fn font_faults_are_caught() {
    let fail = |edit: fn(&mut Value), pointer: &str, what: &str| {
        let (name, text) = edited(FONT, edit);
        assert_fails_at(
            load_font(&name, &text, "core").map(|_| ()),
            &name,
            pointer,
            what,
        );
    };
    fail(
        |v| v["glyphs"][0]["ch"] = json!("AB"),
        "/glyphs/0/ch",
        "a glyph of two characters",
    );
    fail(
        |v| v["glyphs"][1]["ch"] = json!("0"),
        "/glyphs/1/ch",
        "a character defined twice",
    );
    fail(
        |v| v["glyphs"][0]["strokes"][0][0] = json!([9, 0]),
        "/glyphs/0/strokes/0/0",
        "a point outside the cell",
    );
    fail(
        |v| v["glyphs"][0]["strokes"] = json!([]),
        "/glyphs/0/strokes",
        "a glyph that draws nothing",
    );
    fail(
        |v| v["glyphs"][0]["strokes"] = json!([[[1, 1]]]),
        "/glyphs/0/strokes/0",
        "a stroke of one point",
    );
    fail(
        |v| v["advance"] = json!(3),
        "/advance",
        "glyphs that overlap",
    );
    fail(
        |v| {
            v["glyphs"]
                .as_array_mut()
                .unwrap()
                .retain(|g| g["ch"] != "α");
        },
        "/glyphs",
        "no alpha",
    );
    fail(
        |v| {
            let space = v["glyphs"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|g| g["ch"] == " ")
                .unwrap();
            space["strokes"] = json!([[[0, 0], [1, 1]]]);
        },
        "/glyphs/36/strokes",
        "a space that draws",
    );
}

#[test]
fn audio_faults_are_caught() {
    let fail = |edit: fn(&mut Value), pointer: &str, what: &str| {
        let (name, text) = edited(AUDIO, edit);
        assert_fails_at(
            load_audio(&name, &text, "core").map(|_| ()),
            &name,
            pointer,
            what,
        );
    };
    fail(
        |v| v["engine"]["military_rpm"] = json!(7000),
        "/engine/military_rpm",
        "military below idle",
    );
    fail(
        |v| v["engine"]["rpm_by_core"][1][1] = json!(1000),
        "/engine/rpm_by_core/1",
        "a curve that falls",
    );
    fail(
        |v| v["engine"]["rpm_by_core"][3][1] = json!(10000),
        "/engine/rpm_by_core/3",
        "a curve that misses military",
    );
    fail(
        |v| v["engine"]["rpm_by_core"][0][0] = json!(0.01),
        "/engine/rpm_by_core/0",
        "a curve that starts above zero",
    );
    fail(
        |v| v["engine"]["harmonic_levels_db"][0] = json!(-3),
        "/engine/harmonic_levels_db/0",
        "a fundamental below 0 dB",
    );
    fail(
        |v| v["engine"]["harmonic_levels_db"][1] = json!(3),
        "/engine/harmonic_levels_db/1",
        "a harmonic louder than the fundamental",
    );
    fail(
        |v| v["engine"]["burner_roar_db"] = json!(10),
        "/engine/burner_roar_db",
        "an afterburner that is not +6 dB",
    );
    fail(
        |v| v["engine"]["blade_count"] = json!(2),
        "/engine/blade_count",
        "two blades",
    );
    fail(
        |v| v["aero"]["wind_low_hz"] = json!(3000),
        "/aero/wind_high_hz",
        "wind that does not rise",
    );
    fail(
        |v| v["warnings"]["stall_horn"]["pulse_max_hz"] = json!(1),
        "/warnings/stall_horn/pulse_max_hz",
        "a stall horn that slows down",
    );
    fail(
        |v| v["warnings"]["pull_up"]["low_hz"] = json!(1200),
        "/warnings/pull_up/high_hz",
        "a warble upside down",
    );
    fail(
        |v| v["mix"]["duck_db"] = json!(3),
        "/mix/duck_db",
        "ducking that raises the level",
    );
    fail(
        |v| v["cockpit"]["canopy_attenuation_db"] = json!(5),
        "/cockpit/canopy_attenuation_db",
        "a canopy that amplifies",
    );
}

// ---------------------------------------------------------------------------------------------
// Across files.

#[test]
fn files_for_unknown_aircraft_or_fonts_and_duplicates_are_caught() {
    assert_fails_at(
        bake_with(HUD, |v| v["aircraft"] = json!("core:ac/ghost")),
        HUD,
        "/aircraft",
        "a HUD for an aircraft that does not exist",
    );
    assert_fails_at(
        bake_with(HUD, |v| v["font"] = json!("core:fnt/missing")),
        HUD,
        "/font",
        "a HUD with a font that does not exist",
    );
    assert_fails_at(
        bake_with(MODEL, |v| v["id"] = json!("core:mdl/other")).and_then(|()| {
            // A second model for the same aircraft.
            let mut src = sources(&[]);
            let mut second = value(MODEL);
            second["id"] = json!("core:mdl/second");
            src.models
                .push(("models/second.json".into(), second.to_string()));
            bake_present(&src, &baked()).map(|_| ())
        }),
        "models/second.json",
        "/aircraft",
        "two models for one aircraft",
    );
    assert_fails_at(
        {
            let mut src = sources(&[]);
            src.fonts.push(("fonts/copy.json".into(), read(FONT)));
            bake_present(&src, &baked()).map(|_| ())
        },
        "fonts/copy.json",
        "/id",
        "an id used twice",
    );
}

#[test]
fn a_syntax_error_names_the_file_and_the_position() {
    let src = sources(&[(HUD, "{ \"schema\": ".to_owned())]);
    let problems = bake_present(&src, &baked()).expect_err("broken JSON");
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].file, HUD);
    assert!(problems[0].message.contains("not valid JSON"));
    assert!(problems[0].hint.contains("line"));
}

#[test]
fn every_problem_is_reported_not_just_the_first() {
    let (hud, hud_text) = edited(HUD, |v| v["colour"] = json!("nope"));
    let (audio, audio_text) = edited(AUDIO, |v| v["engine"]["military_rpm"] = json!(7000));
    let src = sources(&[(&hud, hud_text), (&audio, audio_text)]);
    let problems = bake_present(&src, &baked()).expect_err("two broken files");
    assert!(
        problems
            .iter()
            .any(|d| d.file == hud && d.pointer == "/colour")
    );
    assert!(
        problems
            .iter()
            .any(|d| d.file == audio && d.pointer == "/engine/military_rpm")
    );
}
