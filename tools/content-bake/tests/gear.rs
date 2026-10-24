//! The landing-gear lints against the Kestrel's data and deliberately broken variants of it.

use std::path::Path;

use content_bake::gear::{self, Problem};
use content_bake::model::{AeroFile, AircraftFile};

fn core() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core")
}

fn kestrel() -> (AircraftFile, AeroFile) {
    let read = |rel: &str| std::fs::read_to_string(core().join(rel)).unwrap();
    (
        serde_json::from_str(&read("aircraft/kestrel.json")).unwrap(),
        serde_json::from_str(&read("aero/kestrel.json")).unwrap(),
    )
}

fn all(a: &AircraftFile, aero: &AeroFile) -> Vec<Problem> {
    let mut p = gear::lint_geometry(a);
    p.extend(gear::lint_touchdown(a, aero));
    p
}

#[test]
fn the_kestrel_gear_has_the_designed_loads_and_clearances() {
    let (a, aero) = kestrel();
    assert_eq!(all(&a, &aero), Vec::<Problem>::new());

    // Worked numbers (ADR-35): mains at x -0.62, nose at 4.58, CG at 0 -> 11.9 % on the nose.
    let share: f64 = 0.62 / 5.2;
    assert!((share - 0.1192).abs() < 1e-3);
    let compression = |leg: usize, mass_kg: f64, load_share: f64| {
        let l = &a.geometry.gear[leg];
        gear::compression_m(l, mass_kg * load_share * 9.806_65) / l.stroke_m
    };
    // Nose at landing mass, mains at landing and take-off mass.
    assert!((compression(0, 9500.0, share) - 0.268).abs() < 0.01);
    assert!((compression(1, 9500.0, (1.0 - share) / 2.0) - 0.37).abs() < 0.01);
    assert!((compression(1, 14500.0, (1.0 - share) / 2.0) - 0.51).abs() < 0.01);

    // The touchdown attitude at 1.15 V_s, landing flaps, on a 3 degree path, and the tail's limit.
    let td = gear::touchdown_pitch_deg(&aero).unwrap();
    assert!((td - 15.1).abs() < 0.2, "{td}");
    let tail = a.geometry.contacts.iter().find(|c| c.id == "tail").unwrap();
    let limit = gear::scrape_pitch_deg(&a, tail).unwrap();
    assert!((limit - 16.45).abs() < 0.1, "{limit}");
    assert!(limit >= td + 1.0);
}

#[test]
fn compression_solves_the_progressive_spring() {
    let (a, _) = kestrel();
    let mut leg = a.geometry.gear[0].clone();
    leg.k_npm = 100_000.0;
    leg.stroke_m = 0.3;
    // k x (1 + (x / 0.3)^2) at x = 0.1 is 11 111.1 N.
    let u = 0.1_f64 / 0.3;
    let n = 100_000.0 * 0.1 * (1.0 + u * u);
    assert!((gear::compression_m(&leg, n) - 0.1).abs() < 1e-9);
    assert!(gear::compression_m(&leg, 0.0) < 1e-12);
}

type Edit = Box<dyn Fn(&mut AircraftFile)>;

/// Each case breaks one thing and must raise a problem at `pointer` whose message has `needle`.
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "a table of cases; splitting it would hide the table"
)]
fn broken_gear_is_reported_at_the_right_pointer() {
    let cases: Vec<(&str, Edit, &str, &str)> = vec![
        (
            "a duplicate leg id",
            Box::new(|a| a.geometry.gear[1].id = "nose".into()),
            "/geometry/gear/1/id",
            "listed twice",
        ),
        (
            "a duplicate contact id",
            Box::new(|a| a.geometry.contacts[1].id = "nose_tip".into()),
            "/geometry/contacts/1/id",
            "listed twice",
        ),
        (
            "no brakes",
            Box::new(|a| a.geometry.gear.iter_mut().for_each(|l| l.brake = false)),
            "/geometry/gear",
            "no leg has a brake",
        ),
        (
            "two steering legs",
            Box::new(|a| a.geometry.gear[1].steer_max_deg = 5.0),
            "/geometry/gear/1/steer_max_deg",
            "more than one leg steers",
        ),
        (
            "all legs main",
            Box::new(|a| a.geometry.gear[0].main = true),
            "/geometry/gear",
            "main legs plus",
        ),
        (
            "a nose leg too far aft carries too much",
            Box::new(|a| a.geometry.gear[0].pos_m[0] = 2.0),
            "/geometry/gear",
            "nose leg carries",
        ),
        (
            "mains ahead of the CG tip the aircraft back",
            Box::new(|a| {
                a.geometry.gear[1].pos_m[0] = 0.4;
                a.geometry.gear[2].pos_m[0] = 0.4;
            }),
            "/geometry/gear",
            "tip onto its",
        ),
        (
            "a spring too soft for the take-off mass",
            Box::new(|a| {
                a.geometry.gear[1].k_npm = 200_000.0;
                a.geometry.gear[2].k_npm = 200_000.0;
            }),
            "/geometry/gear/1/k_npm",
            "compresses",
        ),
        (
            "a spring so stiff the strut rings above the step",
            Box::new(|a| a.geometry.gear[0].k_npm = 4_000_000.0),
            "/geometry/gear/0/k_npm",
            "rad/s",
        ),
        (
            "an underdamped strut",
            Box::new(|a| a.geometry.gear[1].c_comp_nspm = 3000.0),
            "/geometry/gear/1/c_comp_nspm",
            "damping ratio",
        ),
        (
            "a wheel outside the span",
            Box::new(|a| a.geometry.gear[2].pos_m[1] = 6.0),
            "/geometry/gear/2/pos_m/1",
            "outside the span",
        ),
        (
            "a tail that scrapes at the touchdown attitude",
            Box::new(|a| a.geometry.contacts[1].pos_m[2] = 1.0),
            "/geometry/contacts/1/pos_m",
            "touches the ground",
        ),
    ];
    for (what, edit, pointer, needle) in cases {
        let (mut a, aero) = kestrel();
        edit(&mut a);
        let problems = all(&a, &aero);
        assert!(
            problems
                .iter()
                .any(|p| p.ptr == pointer && p.message.contains(needle)),
            "{what}: no `{needle}` at {pointer} in {problems:#?}"
        );
        assert!(
            problems.iter().all(|p| !p.hint.is_empty()),
            "{what}: every problem says how to fix it"
        );
    }
}

#[test]
fn an_aircraft_with_no_gear_has_nothing_to_lint() {
    let (mut a, aero) = kestrel();
    a.geometry.gear.clear();
    a.geometry.contacts.clear();
    assert_eq!(all(&a, &aero), Vec::<Problem>::new());
}
