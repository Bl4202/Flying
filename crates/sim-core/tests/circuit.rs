//! The circuit gate (ADR-33 section 1): the virtual tester takes off, flies the circuit and lands
//! under every assist level, through the input frame, the control system, the gear and the
//! ground, and the judge reads the nine criteria off the published snapshot.

use std::path::Path;
use std::sync::OnceLock;

use sim_core::circuit::report::{self, CircuitReport};
use sim_core::layout::input_script;
use sim_core::{Assist, GEffects, SessionConfig, SimCore, Spawn};

fn core_pack() -> &'static [u8] {
    static PACK: OnceLock<Vec<u8>> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
        content_bake::bake::bake_dir(&dir)
            .expect("the core pack bakes")
            .cpack
    })
}

const ALL: [Assist; 4] = [
    Assist::Full,
    Assist::Standard,
    Assist::Reduced,
    Assist::Manual,
];

fn circuit_core(pack: &[u8], assist: Assist, fuel: f64) -> SimCore {
    let mut core = SimCore::new(sim_core::DEFAULT_SEED);
    core.load_content(pack).expect("the pack loads");
    assert!(core.set_spawn_fuel(fuel));
    core.configure(SessionConfig {
        spawn: Spawn::Runway,
        assist,
        g_effects: GEffects::Full,
    })
    .expect("idle");
    assert!(core.set_input_script(input_script::CIRCUIT as u32));
    assert!(core.start());
    core
}

fn fly(pack: &[u8], assist: Assist, fuel: f64) -> (CircuitReport, SimCore) {
    let mut core = circuit_core(pack, assist, fuel);
    let r = report::fly(&mut core, assist.code(), |_| {});
    (r, core)
}

fn table(r: &CircuitReport) -> String {
    r.criteria
        .iter()
        .map(|c| {
            format!(
                "  {} {} {:>9.2} (limit {:>6.1}) {}",
                c.id,
                if c.pass { "pass" } else { "FAIL" },
                c.value,
                c.limit,
                c.what
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_virtual_tester_flies_take_off_circuit_and_landing_under_every_assist() {
    for assist in ALL {
        let (r, core) = fly(core_pack(), assist, 0.6);
        println!(
            "{assist:?}: {:.0} s, touchdown {:.1} m/s at {:.0} m, stopped after {:.0} m, max alpha {:.1} deg, max nz {:.2} g\n{}",
            r.total_s,
            r.touchdown_sink_mps,
            r.touchdown_along_m,
            r.stop_distance_m,
            r.max_alpha_deg,
            r.max_nz_g,
            table(&r)
        );
        assert!(r.pass, "{assist:?} did not pass:\n{}", table(&r));
        let c = core.counters();
        assert_eq!(
            (c.crashes, c.fdm_faults, c.gloc_events),
            (0, 0, 0),
            "{assist:?}"
        );
        println!(
            "{assist:?}: touchdowns {} hard {}",
            c.touchdowns, c.hard_landings
        );
        assert!(c.touchdowns >= 1 && c.hard_landings == 0, "{assist:?}");
    }
}

#[test]
fn the_circuit_is_repeatable_to_the_bit() {
    let (a, ca) = fly(core_pack(), Assist::Standard, 0.6);
    let (b, cb) = fly(core_pack(), Assist::Standard, 0.6);
    assert_eq!(a, b);
    assert_eq!(ca.subsystem_hashes(), cb.subsystem_hashes());
}

/// Informational (ADR-33): the circuit at other masses. The default mass is the gate and is
/// covered above; the others are printed so a regression shows. The Kestrel's mass runs from 8 400
/// kg empty to 11 500 kg with full tanks (an empty tank is a flameout); heavier needs external
/// stores, which v0.1b does not have.
#[test]
fn the_circuit_at_other_masses_is_reported() {
    for (label, fuel) in [
        ("8 710 kg (10 % fuel)", 0.1),
        ("9 330 kg (30 %)", 0.3),
        ("11 500 kg (full)", 1.0),
    ] {
        for assist in ALL {
            let (r, _) = fly(core_pack(), assist, fuel);
            println!(
                "{label} {assist:?}: {}\n{}",
                if r.pass { "pass" } else { "FAIL" },
                if r.pass { String::new() } else { table(&r) }
            );
        }
    }
}
