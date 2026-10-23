//! The core pack's aircraft passes its Performance Conformance Test, deterministically: twelve
//! points, the ten airborne ones (ADR-31) and the two ground rolls (ADR-32).

use std::path::Path;

use content::Registry;
use flight_test::report::Status;
use flight_test::{Options, run};

fn registry() -> Registry {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
    let baked = content_bake::bake::bake_dir(&dir).expect("the core pack bakes");
    Registry::from_cpack(&baked.cpack).unwrap()
}

#[test]
fn the_kestrel_passes_and_the_run_is_deterministic() {
    let r = registry();
    let h = r.aircraft_handles().next().unwrap();
    let a = run(&r, h, "test", &Options::default()).unwrap();
    // Nothing is deferred: the ground rolls are flown now (ADR-32), so AC-3.2 is complete.
    let expected: Vec<String> = Vec::new();
    let failures = a.report.gate_failures(&expected);
    assert!(
        failures.is_empty(),
        "{failures:?}\n{}",
        a.report.to_markdown(&expected)
    );
    let passed = a
        .report
        .points
        .iter()
        .filter(|p| p.status == Status::Pass)
        .count();
    assert_eq!(
        passed, 12,
        "ten airborne points (ADR-31) and two ground rolls (ADR-32)"
    );
    for id in ["to_roll", "ldg_roll"] {
        let p = a.report.points.iter().find(|p| p.id == id).unwrap();
        assert!(
            p.tol_used_pct() < 60.0,
            "{id} uses {:.0} % of its tolerance",
            p.tol_used_pct()
        );
        assert!(
            p.cross_check.is_some_and(|c| c.ok()),
            "{id}: point-mass cross-check"
        );
        assert!(
            p.extra_checks.iter().all(|e| e.check.ok()),
            "{id}: energy closure"
        );
    }
    // (Compared as JSON: deferred points measure NaN, which is never equal to itself.)
    let b = run(&r, h, "test", &Options::default()).unwrap();
    assert_eq!(a.report.to_json(&expected), b.report.to_json(&expected));
    assert_eq!(a.traces, b.traces, "the same pack gives the same traces");
    // One point alone gives the same result as in the full run.
    let only = run(
        &r,
        h,
        "test",
        &Options {
            only: vec!["roc_sl".into()],
        },
    )
    .unwrap();
    assert_eq!(only.report.points.len(), 1);
    let full = a.report.points.iter().find(|p| p.id == "roc_sl").unwrap();
    assert_eq!(&only.report.points[0], full);
}
