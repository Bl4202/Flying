//! `fg circuit`: the circuit gate (ADR-33 section 1).
//!
//! The virtual tester takes off from the runway, flies the circuit and lands, under each assist
//! level, through the real input path. Prints one table per run, optionally writes the
//! `circuit_report/1` JSON and the 10 Hz CSV traces, and fails (exit 1) when a gate run fails. The
//! gate is the default load under all four assist levels, in the arena `--arena` names (`rugged`
//! by default, `all` for both, ADR-39 D1); with `--robustness` the circuit is also flown at other
//! fuel loads and reported, which does not affect the exit code.

use std::fmt::Write as _;
use std::path::PathBuf;

use serde_json::{Value, json};
use sim_core::circuit::Phase;
use sim_core::circuit::report::{self, CircuitReport, Sample};
use sim_core::layout::input_script;
use sim_core::{Arena, Assist, DEFAULT_SEED, GEffects, Scenario, SessionConfig, SimCore, Spawn};

/// The fuel the gate flies with, as a fraction of capacity (the spawn's default).
pub const GATE_FUEL: f64 = 0.6;
/// The other loads of the robustness runs: 10 %, 30 % and full tanks.
pub const ROBUSTNESS_FUELS: [f64; 3] = [0.1, 0.3, 1.0];

/// The assist levels, in the order they are flown.
const LEVELS: [(&str, Assist); 4] = [
    ("full", Assist::Full),
    ("standard", Assist::Standard),
    ("reduced", Assist::Reduced),
    ("manual", Assist::Manual),
];

/// Parsed flags.
#[derive(Debug, PartialEq)]
pub struct Args {
    content: PathBuf,
    assists: Vec<(&'static str, Assist)>,
    arenas: Vec<Arena>,
    json: Option<PathBuf>,
    traces: Option<PathBuf>,
    robustness: bool,
    seed: u64,
}

/// Parses `fg circuit` flags.
///
/// # Errors
/// A message for a missing value, an unknown flag, a bad assist or a missing `--content`.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        content: PathBuf::new(),
        assists: LEVELS.to_vec(),
        arenas: vec![Arena::Rugged],
        json: None,
        traces: None,
        robustness: false,
        seed: DEFAULT_SEED,
    };
    let mut content = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if flag == "--robustness" {
            a.robustness = true;
            continue;
        }
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--content" => content = Some(PathBuf::from(value)),
            "--assist" => {
                a.assists = if value == "all" {
                    LEVELS.to_vec()
                } else {
                    vec![
                        *LEVELS
                            .iter()
                            .find(|(name, _)| name == value)
                            .ok_or_else(|| format!("unknown assist {value}"))?,
                    ]
                };
            }
            "--arena" => {
                a.arenas = if value == "all" {
                    Arena::ALL.to_vec()
                } else {
                    vec![Arena::from_name(value).ok_or_else(|| format!("unknown arena {value}"))?]
                };
            }
            "--json" => a.json = Some(PathBuf::from(value)),
            "--traces" => a.traces = Some(PathBuf::from(value)),
            "--seed" => {
                a.seed = u64::from_str_radix(value.trim_start_matches("0x"), 16)
                    .map_err(|e| format!("bad --seed: {e}"))?;
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    a.content = content.ok_or("--content is required")?;
    Ok(a)
}

/// One flown circuit.
struct Run {
    name: &'static str,
    arena: Arena,
    fuel: f64,
    gate: bool,
    report: CircuitReport,
    trace: String,
}

fn fly(
    a: &Args,
    bytes: &[u8],
    (name, assist): (&'static str, Assist),
    arena: Arena,
    fuel: f64,
) -> Result<Run, String> {
    let mut core = SimCore::new(a.seed);
    core.load_content(bytes)
        .map_err(|e| format!("cannot load content: {e}"))?;
    if !core.set_spawn_fuel(fuel) {
        return Err(format!("bad fuel fraction {fuel}"));
    }
    core.configure(SessionConfig {
        spawn: Spawn::Runway,
        assist,
        g_effects: GEffects::Full,
        arena,
        scenario: Scenario::Free,
        skill: 0,
    })
    .map_err(|e| format!("cannot configure the session: {e}"))?;
    if !core.set_input_script(input_script::CIRCUIT as u32) {
        return Err("the circuit script is not available".into());
    }
    if !core.start() {
        return Err("the core would not start".into());
    }
    let mut trace = String::from(TRACE_HEADER);
    let mut tick = 0_u64;
    let report = report::fly(&mut core, assist.code(), |s| {
        if tick.is_multiple_of(6) {
            trace_row(&mut trace, s);
        }
        tick += 1;
    });
    Ok(Run {
        name,
        arena,
        fuel,
        gate: (fuel - GATE_FUEL).abs() < 1e-9,
        report,
        trace,
    })
}

/// The trace columns: the v0.1b set, then the snapshot v4 fields and what the audio needs to
/// replay a sortie offline (ADR-39 A5).
const TRACE_HEADER: &str = "t_s,phase,along_m,across_m,agl_m,cas_mps,gs_mps,alpha_deg,nz_g,wow,gear_pos,stall_warning,\
tas_mps,qbar_pa,throttle,engine_core,ab_level,engine_bits,airbrake,fuel_flow_kgps,alpha_warn_deg,n_max_g,max_nz_g,vs_mps,flags,flags2\n";

fn trace_row(out: &mut String, s: &Sample) {
    let _ = writeln!(
        out,
        "{:.1},{},{:.1},{:.2},{:.2},{:.2},{:.2},{:.2},{:.3},{},{:.2},{},{:.2},{:.1},{:.4},{:.4},{:.4},{},{:.3},{:.4},{:.2},{:.2},{:.3},{:.2},{},{}",
        s.t_s,
        s.phase.name(),
        s.along_m,
        s.across_m,
        s.agl_m,
        s.cas_mps,
        s.gs_mps,
        s.alpha_rad.to_degrees(),
        s.nz_g,
        u8::from(s.wow),
        s.gear_pos,
        u8::from(s.stall_warning),
        s.tas_mps,
        s.qbar_pa,
        s.throttle,
        s.engine_core,
        s.ab_level,
        s.engine_bits,
        s.airbrake,
        s.fuel_flow_kgps,
        s.alpha_warn_rad.to_degrees(),
        s.n_max_g,
        s.max_nz_g,
        s.vs_mps,
        s.flags,
        s.flags2
    );
}

fn markdown(r: &Run) -> String {
    let mut s = String::new();
    let rep = &r.report;
    let _ = writeln!(
        s,
        "\n### {} assist, {}, {:.0} kg{}: {}",
        r.name,
        r.arena.name(),
        rep.mass_kg,
        if r.gate { "" } else { " (informational)" },
        if rep.pass { "pass" } else { "FAIL" }
    );
    let _ = writeln!(
        s,
        "\n| # | Criterion | Measured | Limit | Status |\n|---|---|---|---|---|"
    );
    for c in &rep.criteria {
        let _ = writeln!(
            s,
            "| {} | {} | {:.2} | {:.1} | {} |",
            c.id,
            c.what,
            c.value,
            c.limit,
            if c.pass { "pass" } else { "FAIL" }
        );
    }
    let _ = writeln!(
        s,
        "\ntotal {:.0} s, lift-off at {:.2} V_s,TO after {:.0} m, touchdown {:.1} m/s at {:.0} m, stopped after {:.0} m, max alpha {:.1} deg, max n {:.2} g{}",
        rep.total_s,
        rep.lift_off_speed_ratio,
        rep.lift_off_roll_m,
        rep.touchdown_sink_mps,
        rep.touchdown_along_m,
        rep.stop_distance_m,
        rep.max_alpha_deg,
        rep.max_nz_g,
        if rep.timeout { ", timed out" } else { "" }
    );
    s
}

fn run_json(r: &Run) -> Value {
    let rep = &r.report;
    let phases: serde_json::Map<String, Value> = Phase::ALL
        .iter()
        .map(|p| (p.name().to_owned(), json!(rep.phase_times_s[*p as usize])))
        .collect();
    json!({
        "assist": r.name,
        "arena": r.arena.name(),
        "fuel_fraction": r.fuel,
        "mass_kg": rep.mass_kg,
        "gate": r.gate,
        "pass": rep.pass,
        "timeout": rep.timeout,
        "criteria": rep.criteria.iter().map(|c| json!({
            "id": c.id,
            "what": c.what,
            "pass": c.pass,
            "value": c.value,
            "limit": c.limit,
        })).collect::<Vec<_>>(),
        "phase_times_s": phases,
        "lift_off_roll_m": rep.lift_off_roll_m,
        "lift_off_speed_ratio": rep.lift_off_speed_ratio,
        "max_alpha_deg": rep.max_alpha_deg,
        "max_nz_g": rep.max_nz_g,
        "touchdown_sink_mps": rep.touchdown_sink_mps,
        "touchdown_along_m": rep.touchdown_along_m,
        "stop_distance_m": rep.stop_distance_m,
        "total_s": rep.total_s,
    })
}

/// The `circuit_report/1` document.
fn report_json(runs: &[Run], build: &str) -> Value {
    let failed: Vec<String> = runs
        .iter()
        .filter(|r| r.gate && !r.report.pass)
        .map(|r| format!("{}/{}", r.arena.name(), r.name))
        .collect();
    json!({
        "schema": "circuit_report/1",
        "build": build,
        "gate": { "pass": failed.is_empty(), "failed": failed },
        "runs": runs.iter().map(run_json).collect::<Vec<_>>(),
    })
}

/// Runs the gate. `Ok(true)` when every gate run passes.
///
/// # Errors
/// A message when the pack cannot be read or loaded, or a file cannot be written.
pub fn run(args: &[String], build: &str) -> Result<bool, String> {
    let a = parse(args)?;
    let bytes = std::fs::read(&a.content)
        .map_err(|e| format!("cannot read {}: {e}", a.content.display()))?;
    let mut runs = Vec::new();
    for &arena in &a.arenas {
        for &level in &a.assists {
            runs.push(fly(&a, &bytes, level, arena, GATE_FUEL)?);
            // The robustness loads are flown in the default arena only (informational).
            if a.robustness && arena == Arena::Rugged {
                for fuel in ROBUSTNESS_FUELS {
                    runs.push(fly(&a, &bytes, level, arena, fuel)?);
                }
            }
        }
    }
    println!("## Circuit gate (ADR-33)");
    for r in &runs {
        print!("{}", markdown(r));
    }
    if let Some(path) = &a.json {
        let text =
            serde_json::to_string_pretty(&report_json(&runs, build)).map_err(|e| e.to_string())?;
        std::fs::write(path, text + "\n")
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    if let Some(dir) = &a.traces {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        for r in &runs {
            // v0.1b's names for the default arena; the other arena is prefixed.
            let prefix = if r.arena == Arena::Rugged {
                String::new()
            } else {
                format!("{}_", r.arena.name())
            };
            let path = dir.join(format!("{prefix}{}_{:.0}.csv", r.name, r.fuel * 100.0));
            std::fs::write(&path, &r.trace)
                .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
    }
    let failed: Vec<String> = runs
        .iter()
        .filter(|r| r.gate && !r.report.pass)
        .map(|r| format!("{}/{}", r.arena.name(), r.name))
        .collect();
    println!(
        "\ngate: {}",
        if failed.is_empty() {
            "pass".to_owned()
        } else {
            format!("FAIL ({})", failed.join(", "))
        }
    );
    Ok(failed.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn flags_are_parsed() {
        let a = parse(&args(&[
            "--content",
            "x.cpack",
            "--assist",
            "reduced",
            "--json",
            "r.json",
            "--traces",
            "t",
            "--robustness",
            "--seed",
            "ff",
        ]))
        .unwrap();
        assert_eq!(a.content, PathBuf::from("x.cpack"));
        assert_eq!(a.assists, [("reduced", Assist::Reduced)]);
        assert_eq!(a.arenas, [Arena::Rugged]);
        assert!(a.robustness);
        assert_eq!(a.seed, 0xff);
        let all = parse(&args(&["--content", "x", "--assist", "all"])).unwrap();
        assert_eq!(all.assists.len(), 4);
        let both = parse(&args(&["--content", "x", "--arena", "all"])).unwrap();
        assert_eq!(both.arenas, Arena::ALL);
        let plain = parse(&args(&["--content", "x", "--arena", "plain"])).unwrap();
        assert_eq!(plain.arenas, [Arena::Plain]);
        assert!(parse(&args(&["--content", "x", "--arena", "flat"])).is_err());
        assert!(parse(&args(&[])).is_err(), "--content is required");
        assert!(parse(&args(&["--content", "x", "--assist", "wat"])).is_err());
        assert!(parse(&args(&["--content", "x", "--wat", "1"])).is_err());
        assert!(parse(&args(&["--content"])).is_err());
    }

    #[test]
    fn the_report_has_the_documented_shape() {
        let rep = CircuitReport {
            assist: 2,
            mass_kg: 10_260.0,
            pass: false,
            timeout: false,
            criteria: report::Judge::new(2, 10_260.0).finish(false).criteria,
            phase_times_s: [f64::NAN; Phase::COUNT],
            lift_off_roll_m: f64::NAN,
            lift_off_speed_ratio: f64::NAN,
            max_alpha_deg: 16.0,
            max_nz_g: 2.0,
            touchdown_sink_mps: f64::NAN,
            touchdown_along_m: f64::NAN,
            stop_distance_m: f64::NAN,
            total_s: 12.0,
        };
        let runs = [Run {
            name: "standard",
            arena: Arena::Rugged,
            fuel: GATE_FUEL,
            gate: true,
            report: rep,
            trace: String::new(),
        }];
        let v = report_json(&runs, "fg test");
        assert_eq!(v["schema"], "circuit_report/1");
        assert_eq!(v["gate"]["pass"], false);
        assert_eq!(v["gate"]["failed"][0], "rugged/standard");
        let run = &v["runs"][0];
        assert_eq!(run["arena"], "rugged");
        assert_eq!(run["criteria"].as_array().unwrap().len(), 9);
        assert_eq!(run["criteria"][0]["id"], "C1");
        assert!(run["phase_times_s"]["rollout"].is_null(), "NaN is null");
        assert_eq!(run["phase_times_s"].as_object().unwrap().len(), 11);
        assert!(run["touchdown_sink_mps"].is_null());
    }
}
