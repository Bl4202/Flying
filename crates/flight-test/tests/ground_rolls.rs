//! The ground-roll gate catches the faults it exists for (ADR-32), and the instrument behaves.

use std::path::{Path, PathBuf};

use content::Registry;
use flight::aircraft::AircraftModel;
use flight_test::procedures::ground_roll::{self, Kind};
use flight_test::report::Status;
use flight_test::{Options, run};

fn core_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core")
}

fn registry_of(dir: &Path) -> Registry {
    let baked = content_bake::bake::bake_dir(dir).expect("the pack bakes");
    Registry::from_cpack(&baked.cpack).unwrap()
}

/// A copy of the core pack in a scratch directory, with `edit` applied to the Kestrel file.
fn patched_pack(name: &str, edit: impl Fn(String) -> String) -> PathBuf {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            let target = to.join(e.file_name());
            if e.file_type().unwrap().is_dir() {
                copy(&e.path(), &target);
            } else {
                std::fs::copy(e.path(), target).unwrap();
            }
        }
    }
    let to = std::env::temp_dir().join(format!("fg-ground-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&to);
    copy(&core_dir(), &to);
    let file = to.join("aircraft/kestrel.json");
    let text = std::fs::read_to_string(&file).unwrap();
    let edited = edit(text.clone());
    assert_ne!(text, edited, "the edit changed nothing");
    std::fs::write(&file, edited).unwrap();
    to
}

fn only(r: &Registry, id: &str) -> flight_test::report::PointResult {
    let h = r.aircraft_handles().next().unwrap();
    let out = run(
        r,
        h,
        "test",
        &Options {
            only: vec![id.into()],
        },
    )
    .unwrap();
    out.report.points.into_iter().next().unwrap()
}

#[test]
fn an_unbraked_nose_wheel_fails_the_landing_by_about_forty_percent() {
    // The deliberate fault of the plan: with only the mains braked 71 % of the load is braked and
    // the roll grows by more than 40 % (ADR-32: "a braked nose wheel is required data").
    let dir = patched_pack("nose-brake", |t| {
        let at = t.find("\"id\": \"nose\"").expect("the nose leg");
        let brake = t[at..].find("\"brake\": true").expect("its brake") + at;
        let mut out = t.clone();
        out.replace_range(brake..brake + "\"brake\": true".len(), "\"brake\": false");
        out
    });
    let p = only(&registry_of(&dir), "ldg_roll");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(p.status, Status::Fail, "{p:?}");
    assert!(p.error_pct() > 25.0, "{:.1} %", p.error_pct());
    println!("unbraked nose wheel: {:+.1} %", p.error_pct());
}

#[test]
fn the_centreline_loop_recovers_from_a_heading_error_and_an_offset() {
    let r = registry_of(&core_dir());
    let h = r.aircraft_handles().next().unwrap();
    let model = AircraftModel::from_registry(&r, h, 1.0 / 60.0).unwrap();
    let ac = r.aircraft(h).unwrap();
    let p = ac.envelope.iter().find(|p| p.id == "to_roll").unwrap();
    let env = ground_roll::world(0.0);
    // 2 degrees off and 1.5 m to the side: back to the line, and the run is still valid.
    let run = ground_roll::fly_from(&model, p, Kind::TakeOff, &env, 2f64.to_radians(), 1.5)
        .expect("flown");
    assert!(run.invalid.is_none(), "{:?}", run.invalid);
    assert!(!run.timed_out && run.faults == 0);
    // The offset shrinks over the roll and the heading comes back (columns: lateral_m, heading).
    let last = run.trace.rows.last().unwrap();
    assert!(last[2].abs() < 1.0, "{} m off at the end of 1.5", last[2]);
    assert!(last[6].abs() < 0.5, "{} deg off at the end of 2", last[6]);
    assert!(run.lateral_max_m < 2.0, "{}", run.lateral_max_m);
    // The same, braking to a stop.
    let q = ac.envelope.iter().find(|p| p.id == "ldg_roll").unwrap();
    let run = ground_roll::fly_from(&model, q, Kind::Landing, &env, -1f64.to_radians(), -0.5)
        .expect("flown");
    assert!(run.invalid.is_none(), "{:?}", run.invalid);
    assert!(run.lateral_max_m < 2.0, "{}", run.lateral_max_m);
}

#[test]
fn a_wetter_runway_stops_later_and_a_dry_one_matches_the_report() {
    let r = registry_of(&core_dir());
    let h = r.aircraft_handles().next().unwrap();
    let model = AircraftModel::from_registry(&r, h, 1.0 / 60.0).unwrap();
    let ac = r.aircraft(h).unwrap();
    let q = ac.envelope.iter().find(|p| p.id == "ldg_roll").unwrap();
    let dry = ground_roll::fly(&model, q, Kind::Landing, &ground_roll::world(0.0)).unwrap();
    let wet = ground_roll::fly(&model, q, Kind::Landing, &ground_roll::world(1.0)).unwrap();
    assert!(
        wet.distance_m > 1.3 * dry.distance_m,
        "{} vs {}",
        wet.distance_m,
        dry.distance_m
    );
    let report = only(&r, "ldg_roll");
    assert_eq!(report.measured, dry.distance_m);
    assert!(report.notes.iter().any(|n| n.starts_with("wet runway")));
}
