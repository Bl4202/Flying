//! Loading, cross-checking and bundling the presentation files (ADR-43).
//!
//! HUD layouts, cockpits, exterior models, the HUD font and audio profiles are authoring data like
//! the sim pack's, but they are **not** packed into it: the pack's SHA-256 is part of the state
//! hash, so presentation data in it would move every determinism golden. They go into a separate
//! bundle, `core.present.json`, which records the SHA-256 of the sim pack it was baked with, so
//! the app can check the two belong together.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::{Value, json};

use crate::bake::{Baked, hex, sha256};
use crate::check::load;
use crate::diag::Diagnostic;
use crate::present_check::{
    check_audio, check_cockpit, check_font, check_hud, check_model, glass_covers, inside_cabin,
};
use crate::present_model::{AudioFile, BUNDLE_SCHEMA, CockpitFile, FontFile, HudFile, ModelFile};
use crate::schema;

/// The text of every presentation file of one pack, with the names to report errors against.
#[derive(Default)]
pub struct PresentSources {
    /// `(name, text)` of each `hud/*.json`.
    pub hud: Vec<(String, String)>,
    /// `(name, text)` of each `cockpit/*.json`.
    pub cockpit: Vec<(String, String)>,
    /// `(name, text)` of each `audio/*.json`.
    pub audio: Vec<(String, String)>,
    /// `(name, text)` of each `models/*.json`.
    pub models: Vec<(String, String)>,
    /// `(name, text)` of each `fonts/*.json`.
    pub fonts: Vec<(String, String)>,
}

impl PresentSources {
    /// True if there is nothing to bake.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hud.is_empty()
            && self.cockpit.is_empty()
            && self.audio.is_empty()
            && self.models.is_empty()
            && self.fonts.is_empty()
    }
}

/// A successful presentation bake.
#[derive(Debug)]
pub struct PresentBaked {
    /// The bundle, canonical JSON (keys sorted, no whitespace).
    pub json: Vec<u8>,
    /// SHA-256 of [`PresentBaked::json`].
    pub sha256: [u8; 32],
    /// SHA-256 of the sim pack the bundle was baked with (the `.cpack` header's content hash).
    pub sim_pack_sha256: [u8; 32],
    /// The HUD layouts that went in.
    pub huds: Vec<HudFile>,
    /// The cockpits.
    pub cockpits: Vec<CockpitFile>,
    /// The exterior models.
    pub models: Vec<ModelFile>,
    /// The audio profiles.
    pub audio: Vec<AudioFile>,
    /// The fonts.
    pub fonts: Vec<FontFile>,
}

/// Loads and validates one HUD layout.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_hud(file: &str, text: &str, pack_id: &str) -> Result<HudFile, Vec<Diagnostic>> {
    load(file, text, &schema::hud_schema(), |c, h| {
        check_hud(c, h, pack_id);
    })
}

/// Loads and validates one cockpit.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_cockpit(file: &str, text: &str, pack_id: &str) -> Result<CockpitFile, Vec<Diagnostic>> {
    load(file, text, &schema::cockpit_schema(), |c, k| {
        check_cockpit(c, k, pack_id);
    })
}

/// Loads and validates one exterior model.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_model(file: &str, text: &str, pack_id: &str) -> Result<ModelFile, Vec<Diagnostic>> {
    load(file, text, &schema::model_schema(), |c, m| {
        check_model(c, m, pack_id);
    })
}

/// Loads and validates one stroke font.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_font(file: &str, text: &str, pack_id: &str) -> Result<FontFile, Vec<Diagnostic>> {
    load(file, text, &schema::font_schema(), |c, f| {
        check_font(c, f, pack_id);
    })
}

/// Loads and validates one audio profile.
///
/// # Errors
/// Every problem found, each with its pointer.
pub fn load_audio(file: &str, text: &str, pack_id: &str) -> Result<AudioFile, Vec<Diagnostic>> {
    load(file, text, &schema::audio_schema(), |c, a| {
        check_audio(c, a, pack_id);
    })
}

fn keep<T>(r: Result<T, Vec<Diagnostic>>, problems: &mut Vec<Diagnostic>) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(mut e) => {
            problems.append(&mut e);
            None
        }
    }
}

/// A loaded file with the name it was loaded under.
struct Named<T> {
    file: String,
    item: T,
}

fn load_all<T>(
    sources: &[(String, String)],
    problems: &mut Vec<Diagnostic>,
    load_one: impl Fn(&str, &str) -> Result<T, Vec<Diagnostic>>,
) -> Vec<Named<T>> {
    sources
        .iter()
        .filter_map(|(file, text)| {
            keep(load_one(file, text), problems).map(|item| Named {
                file: file.clone(),
                item,
            })
        })
        .collect()
}

/// Validates and bundles `src` for the sim pack `baked`. Collects every diagnostic it can.
///
/// # Errors
/// All diagnostics found, in file order.
///
/// # Panics
/// Never in practice: the bundle is built from values that serialise.
pub fn bake_present(src: &PresentSources, baked: &Baked) -> Result<PresentBaked, Vec<Diagnostic>> {
    let pack_id = baked.pack.id.as_str();
    let mut problems = Vec::new();
    let huds = load_all(&src.hud, &mut problems, |f, t| load_hud(f, t, pack_id));
    let cockpits = load_all(&src.cockpit, &mut problems, |f, t| {
        load_cockpit(f, t, pack_id)
    });
    let models = load_all(&src.models, &mut problems, |f, t| load_model(f, t, pack_id));
    let audio = load_all(&src.audio, &mut problems, |f, t| load_audio(f, t, pack_id));
    let fonts = load_all(&src.fonts, &mut problems, |f, t| load_font(f, t, pack_id));
    if !problems.is_empty() {
        return Err(problems);
    }
    check_unique_ids(&huds, &cockpits, &models, &audio, &fonts, &mut problems);
    check_aircraft_refs(baked, &huds, &cockpits, &models, &audio, &mut problems);
    check_gun_voices(&audio, baked, &mut problems);
    check_cross_references(&huds, &cockpits, &models, &fonts, baked, &mut problems);
    if !problems.is_empty() {
        return Err(problems);
    }
    let sim_pack_sha256 = sim_pack_hash(baked);
    let refs = aircraft_refs(&huds, &cockpits, &models, &audio);
    let bundle = json!({
        "schema": BUNDLE_SCHEMA,
        "pack": pack_id,
        "sim_pack_sha256": hex(&sim_pack_sha256),
        "aircraft": refs,
        "huds": by_id(&huds, |h| h.id.as_str()),
        "cockpits": by_id(&cockpits, |k| k.id.as_str()),
        "models": by_id(&models, |m| m.id.as_str()),
        "audio_profiles": by_id(&audio, |a| a.id.as_str()),
        "fonts": by_id(&fonts, |f| f.id.as_str()),
    });
    // `Value` keeps its keys sorted, so the same data always gives the same bytes.
    let json = serde_json::to_vec(&bundle).expect("the bundle is JSON");
    Ok(PresentBaked {
        sha256: sha256(&json),
        json,
        sim_pack_sha256,
        huds: huds.into_iter().map(|n| n.item).collect(),
        cockpits: cockpits.into_iter().map(|n| n.item).collect(),
        models: models.into_iter().map(|n| n.item).collect(),
        audio: audio.into_iter().map(|n| n.item).collect(),
        fonts: fonts.into_iter().map(|n| n.item).collect(),
    })
}

fn sim_pack_hash(baked: &Baked) -> [u8; 32] {
    let mut out = [0_u8; 32];
    out.copy_from_slice(&baked.cpack[content::cpack::header::CONTENT_HASH..][..32]);
    out
}

fn by_id<T: Serialize>(items: &[Named<T>], id: impl Fn(&T) -> &str) -> Value {
    let map: BTreeMap<&str, Value> = items
        .iter()
        .map(|n| {
            (
                id(&n.item),
                serde_json::to_value(&n.item).expect("serialises"),
            )
        })
        .collect();
    json!(map)
}

/// `aircraft id -> { hud, cockpit, model, audio }` for what exists.
fn aircraft_refs(
    huds: &[Named<HudFile>],
    cockpits: &[Named<CockpitFile>],
    models: &[Named<ModelFile>],
    audio: &[Named<AudioFile>],
) -> Value {
    let mut map: BTreeMap<String, BTreeMap<&'static str, String>> = BTreeMap::new();
    let mut add = |aircraft: &str, kind: &'static str, id: &str| {
        map.entry(aircraft.to_owned())
            .or_default()
            .insert(kind, id.to_owned());
    };
    for n in huds {
        add(&n.item.aircraft, "hud", &n.item.id);
    }
    for n in cockpits {
        add(&n.item.aircraft, "cockpit", &n.item.id);
    }
    for n in models {
        add(&n.item.aircraft, "model", &n.item.id);
    }
    for n in audio {
        add(&n.item.aircraft, "audio", &n.item.id);
    }
    json!(map)
}

/// No id is used by two files.
fn check_unique_ids(
    huds: &[Named<HudFile>],
    cockpits: &[Named<CockpitFile>],
    models: &[Named<ModelFile>],
    audio: &[Named<AudioFile>],
    fonts: &[Named<FontFile>],
    problems: &mut Vec<Diagnostic>,
) {
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    let all = huds
        .iter()
        .map(|n| (&n.file, &n.item.id))
        .chain(cockpits.iter().map(|n| (&n.file, &n.item.id)))
        .chain(models.iter().map(|n| (&n.file, &n.item.id)))
        .chain(audio.iter().map(|n| (&n.file, &n.item.id)))
        .chain(fonts.iter().map(|n| (&n.file, &n.item.id)));
    for (file, id) in all {
        if let Some(first) = seen.get(id.as_str()) {
            problems.push(Diagnostic::new(
                file,
                "/id",
                format!("`{id}` is already defined in {first}"),
                "ids are unique within a pack; rename one of them",
            ));
        } else {
            seen.insert(id.clone(), file.clone());
        }
    }
}

/// Every file belongs to an aircraft of the sim pack, and an aircraft has at most one of each.
fn check_aircraft_refs(
    baked: &Baked,
    huds: &[Named<HudFile>],
    cockpits: &[Named<CockpitFile>],
    models: &[Named<ModelFile>],
    audio: &[Named<AudioFile>],
    problems: &mut Vec<Diagnostic>,
) {
    let known = |id: &str| baked.aircraft.iter().any(|a| a.id == id);
    let mut owners: BTreeMap<(String, &'static str), String> = BTreeMap::new();
    let mut each = |file: &str, aircraft: &str, kind: &'static str| {
        if !known(aircraft) {
            problems.push(Diagnostic::new(
                file,
                "/aircraft",
                format!("there is no aircraft `{aircraft}` in the pack"),
                "name an aircraft of the sim pack, or fix the id",
            ));
        } else if let Some(first) = owners.insert((aircraft.to_owned(), kind), file.to_owned()) {
            problems.push(Diagnostic::new(
                file,
                "/aircraft",
                format!("`{aircraft}` already has a {kind} ({first})"),
                "an aircraft has one of each kind of presentation file",
            ));
        }
    };
    for n in huds {
        each(&n.file, &n.item.aircraft, "HUD");
    }
    for n in cockpits {
        each(&n.file, &n.item.aircraft, "cockpit");
    }
    for n in models {
        each(&n.file, &n.item.aircraft, "model");
    }
    for n in audio {
        each(&n.file, &n.item.aircraft, "audio profile");
    }
}

/// How far (Hz) the audio profile's gun rate may differ from the gun's `rpm / 60`.
const GUN_RATE_TOLERANCE_HZ: f64 = 0.01;

/// A profile's gun voice pulses at the rate of the gun its aircraft carries (ADR-49): the sound is
/// one pulse per round, so a different rate would be a gun that sounds wrong.
fn check_gun_voices(audio: &[Named<AudioFile>], baked: &Baked, problems: &mut Vec<Diagnostic>) {
    for n in audio {
        let Some(w) = &n.item.weapons else { continue };
        let Some(aircraft) = baked.aircraft.iter().find(|a| a.id == n.item.aircraft) else {
            continue; // reported by `check_aircraft_refs`
        };
        let gun = aircraft
            .internal_weapons
            .as_ref()
            .and_then(|i| i.gun.as_ref())
            .and_then(|g| baked.weapons.iter().find(|x| x.id == g.weapon_ref))
            .and_then(|x| x.gun.as_ref());
        match gun {
            None => problems.push(Diagnostic::new(
                &n.file,
                "/weapons/gun",
                format!("`{}` has no gun for this voice to be", aircraft.id),
                "remove the gun voice, or give the aircraft an internal gun",
            )),
            Some(g) => {
                let want = g.rpm / 60.0;
                if (w.gun.rate_hz - want).abs() > GUN_RATE_TOLERANCE_HZ {
                    problems.push(Diagnostic::new(
                        &n.file,
                        "/weapons/gun/rate_hz",
                        format!(
                            "the gun voice pulses at {} Hz but the gun fires {} rounds a minute ({want:.5} Hz)",
                            w.gun.rate_hz, g.rpm
                        ),
                        format!("set rate_hz to {want:.5}"),
                    ));
                }
            }
        }
    }
}

/// The rules that need two files: the font a HUD names, the glass against the HUD's field of view,
/// the eye inside the fuselage, and the model against the aircraft's data.
fn check_cross_references(
    huds: &[Named<HudFile>],
    cockpits: &[Named<CockpitFile>],
    models: &[Named<ModelFile>],
    fonts: &[Named<FontFile>],
    baked: &Baked,
    problems: &mut Vec<Diagnostic>,
) {
    for h in huds {
        if !fonts.iter().any(|f| f.item.id == h.item.font) {
            problems.push(Diagnostic::new(
                &h.file,
                "/font",
                format!("there is no font `{}` in the pack", h.item.font),
                "add the font file under fonts/, or fix the id",
            ));
        }
        let Some(k) = cockpits.iter().find(|k| k.item.aircraft == h.item.aircraft) else {
            continue;
        };
        match glass_covers(&k.item, h.item.tfov_deg) {
            Some(true) => {}
            Some(false) | None => problems.push(Diagnostic::new(
                &k.file,
                "/hud_glass/corners_m",
                format!(
                    "the glass does not cover the HUD's {} by {} degree field of view from the design eye",
                    h.item.tfov_deg[0], h.item.tfov_deg[1]
                ),
                "enlarge the glass, or move it nearer the eye, or reduce tfov_deg in the HUD",
            )),
        }
    }
    for m in models {
        check_model_against_aircraft(m, cockpits, baked, problems);
    }
}

fn check_model_against_aircraft(
    m: &Named<ModelFile>,
    cockpits: &[Named<CockpitFile>],
    baked: &Baked,
    problems: &mut Vec<Diagnostic>,
) {
    let Some(a) = baked.aircraft.iter().find(|a| a.id == m.item.aircraft) else {
        return;
    };
    let g = &a.geometry;
    let mut bad = |ptr: &str, message: String, hint: &str| {
        problems.push(Diagnostic::new(&m.file, ptr, message, hint));
    };
    // The wing tips at the span, the nose and the tail at their contact points.
    let tip = m
        .item
        .surfaces
        .iter()
        .flat_map(|s| [s.root_le_m[1].abs(), s.tip_le_m[1].abs()])
        .fold(0.0_f64, f64::max);
    if (tip - g.span_m / 2.0).abs() > 0.05 {
        bad(
            "/surfaces",
            format!(
                "the widest surface reaches {tip:.2} m from the centreline; the aircraft's span is {} m",
                g.span_m
            ),
            "make the wing tips reach half the span (geometry.span_m / 2) within 5 cm",
        );
    }
    let nose = m.item.fuselage.first().map_or(0.0, |s| s.x_m);
    let tail = m
        .item
        .fuselage
        .last()
        .map_or(0.0, |s| s.x_m)
        .min(m.item.nozzle.at_m[0] - m.item.nozzle.length_m);
    for c in &g.contacts {
        let (ptr, found, name) = match c.id.as_str() {
            "nose_tip" => ("/fuselage/0/x_m", nose, "nose"),
            "tail" => ("/nozzle/at_m", tail, "tail"),
            _ => continue,
        };
        if (found - c.pos_m[0]).abs() > 0.1 {
            bad(
                ptr,
                format!(
                    "the model's {name} is at x = {found:.2} m; the aircraft's `{}` contact is at {} m",
                    c.id, c.pos_m[0]
                ),
                "match the contact point of the aircraft's geometry within 10 cm",
            );
        }
    }
    if tail_to_nose(nose, tail) > g.bbox_m[0] + 0.1 {
        bad(
            "/fuselage",
            format!(
                "the model is {:.2} m long; the aircraft's bounding box is {} m",
                tail_to_nose(nose, tail),
                g.bbox_m[0]
            ),
            "shorten the model, or correct geometry.bbox_m",
        );
    }
    for (i, leg) in m.item.gear.iter().enumerate() {
        match g.gear.iter().find(|l| l.id == leg.id) {
            None => bad(
                &format!("/gear/{i}/id"),
                format!("the aircraft has no gear leg `{}`", leg.id),
                "use the id of one of geometry.gear",
            ),
            Some(l) => {
                let off = (0..3)
                    .map(|a| (l.pos_m[a] - leg.contact_m[a]).abs())
                    .fold(0.0_f64, f64::max);
                if off > 0.01 {
                    bad(
                        &format!("/gear/{i}/contact_m"),
                        format!(
                            "the wheel touches down {off:.3} m from where the aircraft's data puts it"
                        ),
                        "copy pos_m of the gear leg from the aircraft's geometry",
                    );
                }
            }
        }
    }
    for l in &g.gear {
        if !m.item.gear.iter().any(|leg| leg.id == l.id) {
            bad(
                "/gear",
                format!("the model has no gear leg `{}`", l.id),
                "add a leg for each of geometry.gear",
            );
        }
    }
    // The design eye sits inside the fuselage.
    if let Some(k) = cockpits.iter().find(|k| k.item.aircraft == m.item.aircraft)
        && !inside_cabin(&m.item, k.item.eye_point_m)
    {
        problems.push(Diagnostic::new(
            &k.file,
            "/eye_point_m",
            "the design eye is not inside the model's fuselage or canopy",
            "move the eye into the cockpit, or reshape the fuselage and canopy round it",
        ));
    }
}

fn tail_to_nose(nose: f64, tail: f64) -> f64 {
    nose - tail
}

/// Reads `<dir>/{hud,cockpit,audio,models,fonts}/*.json`, if any exist.
fn read_dir_json(dir: &Path, sub: &str) -> Result<Vec<(String, String)>, Vec<Diagnostic>> {
    let path = dir.join(sub);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let listing = fs::read_dir(&path).map_err(|e| {
        vec![Diagnostic::new(
            sub,
            "",
            format!("cannot read the directory: {e}"),
            format!("check {}", path.display()),
        )]
    })?;
    let mut names: Vec<String> = listing
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| Path::new(n).extension().is_some_and(|x| x == "json"))
        .collect();
    names.sort();
    names
        .iter()
        .map(|n| {
            let rel = format!("{sub}/{n}");
            fs::read_to_string(dir.join(&rel))
                .map(|text| (rel.clone(), text))
                .map_err(|e| {
                    vec![Diagnostic::new(
                        &rel,
                        "",
                        format!("cannot read: {e}"),
                        format!("check {}", dir.join(&rel).display()),
                    )]
                })
        })
        .collect()
}

/// Reads and bakes the presentation files of a pack directory. `None` if the pack has none.
///
/// # Errors
/// Read failures and every diagnostic from [`bake_present`].
pub fn bake_present_dir(
    dir: &Path,
    baked: &Baked,
) -> Result<Option<PresentBaked>, Vec<Diagnostic>> {
    let src = PresentSources {
        hud: read_dir_json(dir, "hud")?,
        cockpit: read_dir_json(dir, "cockpit")?,
        audio: read_dir_json(dir, "audio")?,
        models: read_dir_json(dir, "models")?,
        fonts: read_dir_json(dir, "fonts")?,
    };
    if src.is_empty() {
        return Ok(None);
    }
    bake_present(&src, baked).map(Some)
}

/// The `presentation` entry of `content-manifest.json`.
#[must_use]
pub fn manifest_entry(p: &PresentBaked, file: &str) -> Value {
    json!({
        "file": file,
        "bytes": p.json.len(),
        "sha256": hex(&p.sha256),
        "sim_pack_sha256": hex(&p.sim_pack_sha256),
        "entities": {
            "hud": p.huds.len(),
            "cockpit": p.cockpits.len(),
            "model": p.models.len(),
            "audio_profile": p.audio.len(),
            "font": p.fonts.len(),
        },
    })
}
