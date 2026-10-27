//! `fg`: developer command line for the simulation core.
//!
//! ```text
//! fg version
//! fg hash  [--seed HEX] [--script none|erratic|flight60|circuit] [--content PATH]
//!          [--spawn runway|air] [--assist full|standard|reduced|manual] [--g off|reduced|full]
//!          (--ticks N | --at T1,T2,... | --every N --ticks N)
//! fg bench [--seed HEX] [--script none|erratic|flight60|circuit] [--content PATH] [--ticks N] [--json]
//! fg bench --aircraft N --content PATH [--assist full|standard|reduced|manual] [--ground]
//!          [--ticks N] [--json]
//! fg conformance --content PATH [--aircraft-index N] [--json PATH] [--traces DIR]
//!                [--only ID,...] [--expect-deferred ID,...]
//! fg circuit --content PATH [--assist all|full|standard|reduced|manual] [--json PATH]
//!            [--traces DIR] [--robustness] [--seed HEX]
//! ```
//!
//! `fg hash` prints one JSON object per requested tick with the combined and per-subsystem state
//! hashes as 16-digit hex strings. The determinism check compares this output to the WebAssembly
//! builds (ADR-10). `fg bench` times the tick; its wall-clock numbers are observations about the
//! machine and never feed state; with `--aircraft N` it times N copies of the spawned aircraft
//! through the control system (`--assist`, Standard by default) and the flight model, the T0 cost
//! per aircraft-tick (GDD 60.2, PS-9); `--ground` starts them on the runway with the gear in play.
//! `fg conformance` runs the Performance Conformance Test
//! (ADR-31) and exits 1 when its gate fails. `fg circuit` flies the virtual tester's take-off,
//! circuit and landing under each assist level (ADR-33) and exits 1 when a gate run fails.

mod circuit;
mod conformance;

use std::io::Write;
use std::process::ExitCode;
use std::time::Instant;

use sim_core::layout::input_script;
use sim_core::{Assist, DEFAULT_SEED, GEffects, SessionConfig, SimCore, Spawn};

const USAGE: &str = "usage:
  fg version
  fg hash  [--seed HEX] [--script none|erratic|flight60|circuit] [--content PATH]
           [--spawn runway|air] [--assist full|standard|reduced|manual] [--g off|reduced|full]
           (--ticks N | --at T1,T2,... | --every N --ticks N)
  fg bench [--seed HEX] [--script none|erratic|flight60|circuit] [--content PATH] [--ticks N] [--json]
  fg bench --aircraft N --content PATH [--assist full|standard|reduced|manual] [--ground]
           [--ticks N] [--json]
  fg conformance --content PATH [--aircraft-index N] [--json PATH] [--traces DIR]
                 [--only ID,...] [--expect-deferred ID,...]
  fg circuit --content PATH [--assist all|full|standard|reduced|manual] [--json PATH]
             [--traces DIR] [--robustness] [--seed HEX]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("version") => {
            println!(
                "fg {} {}",
                env!("CARGO_PKG_VERSION"),
                option_env!("GIT_SHA").unwrap_or("unknown")
            );
            Ok(())
        }
        Some("hash") => run_hash(&args[1..]),
        Some("bench") => run_bench(&args[1..]),
        Some("conformance") => {
            let build = format!(
                "fg {} {}",
                env!("CARGO_PKG_VERSION"),
                option_env!("GIT_SHA").unwrap_or("unknown")
            );
            match conformance::run(&args[1..], &build) {
                Ok(true) => Ok(()),
                Ok(false) => return ExitCode::from(1),
                Err(e) => Err(e),
            }
        }
        Some("circuit") => {
            let build = format!(
                "fg {} {}",
                env!("CARGO_PKG_VERSION"),
                option_env!("GIT_SHA").unwrap_or("unknown")
            );
            match circuit::run(&args[1..], &build) {
                Ok(true) => Ok(()),
                Ok(false) => return ExitCode::from(1),
                Err(e) => Err(e),
            }
        }
        _ => Err(String::new()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("fg: {msg}");
            }
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Flags shared by `hash` and `bench`.
struct Setup {
    seed: u64,
    script: u32,
    content: Option<Vec<u8>>,
    /// The session (spawn, assist, g-effects); the game's defaults unless flags say otherwise.
    session: SessionConfig,
}

impl Setup {
    /// A started core: content loaded first (only an idle core accepts it), then the script.
    fn core(&self) -> Result<SimCore, String> {
        let mut core = SimCore::new(self.seed);
        if let Some(bytes) = &self.content {
            core.load_content(bytes)
                .map_err(|e| format!("cannot load content: {e}"))?;
        }
        core.configure(self.session)
            .map_err(|e| format!("cannot configure the session: {e}"))?;
        if !core.set_input_script(self.script) {
            return Err(format!("unknown input script {}", self.script));
        }
        core.start();
        Ok(core)
    }
}

/// Command-line arguments split into the shared setup and what is left.
struct Parsed {
    setup: Setup,
    /// Flags this command handles itself, as (name, value).
    rest: Vec<(String, String)>,
    /// Bare switches that were present.
    on: Vec<String>,
}

/// Splits `args` into the shared setup, the other flags (name, value) and bare switches.
fn parse(args: &[String], switches: &[&str]) -> Result<Parsed, String> {
    let mut setup = Setup {
        seed: DEFAULT_SEED,
        script: input_script::NONE as u32,
        content: None,
        session: SessionConfig::default(),
    };
    let mut rest = Vec::new();
    let mut on = Vec::new();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        if switches.contains(&flag.as_str()) {
            on.push(flag.clone());
            continue;
        }
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seed" => {
                let hex = value.trim_start_matches("0x");
                setup.seed =
                    u64::from_str_radix(hex, 16).map_err(|e| format!("bad --seed: {e}"))?;
            }
            "--spawn" => {
                setup.session.spawn = match value.as_str() {
                    "runway" => Spawn::Runway,
                    "air" => Spawn::Air,
                    other => return Err(format!("unknown --spawn {other:?} (runway or air)")),
                };
            }
            "--assist" => {
                setup.session.assist = match value.as_str() {
                    "full" => Assist::Full,
                    "standard" => Assist::Standard,
                    "reduced" => Assist::Reduced,
                    "manual" => Assist::Manual,
                    other => {
                        return Err(format!(
                            "unknown --assist {other:?} (full, standard, reduced or manual)"
                        ));
                    }
                };
            }
            "--g" => {
                setup.session.g_effects = match value.as_str() {
                    "off" => GEffects::Off,
                    "reduced" => GEffects::Reduced,
                    "full" => GEffects::Full,
                    other => return Err(format!("unknown --g {other:?} (off, reduced or full)")),
                };
            }
            "--script" => {
                setup.script = match value.as_str() {
                    "none" => input_script::NONE as u32,
                    "erratic" => input_script::ERRATIC as u32,
                    "flight60" => input_script::FLIGHT60 as u32,
                    "circuit" => input_script::CIRCUIT as u32,
                    other => {
                        return Err(format!(
                            "unknown --script {other:?} (none, erratic, flight60 or circuit)"
                        ));
                    }
                };
            }
            "--content" => {
                setup.content = Some(
                    std::fs::read(value)
                        .map_err(|e| format!("cannot read --content {value}: {e}"))?,
                );
            }
            _ => rest.push((flag.clone(), value.clone())),
        }
    }
    Ok(Parsed { setup, rest, on })
}

fn run_hash(args: &[String]) -> Result<(), String> {
    let Parsed { setup, rest, .. } = parse(args, &[])?;
    let mut at: Option<Vec<u64>> = None;
    let mut last: Option<u64> = None;
    let mut every: Option<u64> = None;
    for (flag, value) in &rest {
        match flag.as_str() {
            "--ticks" => last = Some(value.parse().map_err(|e| format!("bad --ticks: {e}"))?),
            "--every" => {
                every = Some(value.parse().map_err(|e| format!("bad --every: {e}"))?);
            }
            "--at" => at = Some(parse_ticks(value)?),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    let ticks = match (at, last, every) {
        (Some(at), None, None) => at,
        (None, Some(n), None) => vec![n],
        (None, Some(n), Some(k)) if k > 0 => every_ticks(n, k),
        _ => return Err("use --ticks N, --at T1,T2,... or --every K --ticks N".into()),
    };
    if ticks.windows(2).any(|w| w[0] >= w[1]) {
        return Err("--at ticks must be strictly increasing".into());
    }

    let mut core = setup.core()?;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for target in ticks {
        advance_to(&mut core, target);
        let h = core.subsystem_hashes();
        writeln!(
            out,
            "{{\"tick\":{},\"combined\":\"{:016x}\",\"tick_h\":\"{:016x}\",\"rng\":\"{:016x}\",\"bodies\":\"{:016x}\",\"ownship\":\"{:016x}\",\"input\":\"{:016x}\",\"content\":\"{:016x}\"}}",
            core.tick(),
            h.combined,
            h.tick,
            h.rng,
            h.bodies,
            h.ownship,
            h.input,
            h.content
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn advance_to(core: &mut SimCore, target: u64) {
    while core.tick() < target {
        let remaining = target - core.tick();
        let chunk = u32::try_from(remaining.min(1 << 20)).unwrap_or(1 << 20);
        core.step(chunk);
    }
}

/// Ticks in each timed chunk, and how many chunks: the median of the chunks is reported, which
/// ignores the odd interrupted one.
const BENCH_CHUNKS: u64 = 11;

fn run_bench(args: &[String]) -> Result<(), String> {
    let Parsed {
        mut setup,
        rest,
        on,
    } = parse(args, &["--json", "--ground"])?;
    let mut ticks = None;
    let mut aircraft = None;
    for (flag, value) in &rest {
        match flag.as_str() {
            "--ticks" => ticks = Some(value.parse().map_err(|e| format!("bad --ticks: {e}"))?),
            "--aircraft" => {
                aircraft = Some(value.parse().map_err(|e| format!("bad --aircraft: {e}"))?);
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    let json = on.iter().any(|f| f == "--json");
    let ground = on.iter().any(|f| f == "--ground");
    if ground && aircraft.is_none() {
        return Err("--ground goes with --aircraft".into());
    }
    if let Some(n) = aircraft {
        // The microbench flies from the air unless it is asked for the ground, whatever the
        // session default is.
        setup.session.spawn = if ground { Spawn::Runway } else { Spawn::Air };
        return bench_aircraft(&setup, n, ticks, json, ground);
    }
    let ticks = ticks.unwrap_or(220_000_u64);
    let per_chunk = ticks / BENCH_CHUNKS;
    if per_chunk == 0 {
        return Err(format!("--ticks must be at least {BENCH_CHUNKS}"));
    }

    let mut core = setup.core()?;
    // Warm-up: caches, branch predictors and every lazily grown buffer.
    advance_to(&mut core, 2_000);
    let mut ns_per_tick = Vec::new();
    for _ in 0..BENCH_CHUNKS {
        let target = core.tick() + per_chunk;
        let t0 = Instant::now();
        advance_to(&mut core, target);
        ns_per_tick.push(t0.elapsed().as_nanos() as f64 / per_chunk as f64);
    }
    ns_per_tick.sort_by(f64::total_cmp);
    let (min, median, max) = (
        ns_per_tick[0],
        ns_per_tick[ns_per_tick.len() / 2],
        ns_per_tick[ns_per_tick.len() - 1],
    );
    let script = match setup.script as usize {
        input_script::NONE => "none",
        input_script::ERRATIC => "erratic",
        input_script::CIRCUIT => "circuit",
        _ => "flight60",
    };
    if json {
        println!(
            "{{\"schema\":\"fg-bench/1\",\"ticks\":{},\"script\":\"{script}\",\"content\":{},\"ns_per_tick_min\":{min:.1},\"ns_per_tick_median\":{median:.1},\"ns_per_tick_max\":{max:.1}}}",
            per_chunk * BENCH_CHUNKS,
            setup.content.is_some()
        );
    } else {
        println!(
            "{} ticks, script {script}: median {median:.0} ns/tick (min {min:.0}, max {max:.0})",
            per_chunk * BENCH_CHUNKS
        );
    }
    Ok(())
}

/// Ticks an aircraft flies before the microbench puts it back at the spawn (20 s). Measured on the
/// Kestrel with the inputs below: height within 100 m, `n_z` 0.4 to 1.5, bank within 35 degrees.
const SEGMENT_TICKS: u64 = 1_200;

/// The throttle lever for the ground microbench: enough to roll and not enough to lift off in a
/// segment.
const TAXI_LEVER: f64 = 0.45;

/// What the microbench leaves alone: the throttle and the levers, and whether it is on the ground.
struct Levers {
    throttle: f64,
    gear: f64,
    flaps: f64,
    ground: bool,
}

/// The microbench pilot's controls at `tick` of a segment: small slow waves on the three axes and,
/// on the ground, a short touch of both brakes.
fn microbench_controls(tick: u64, levers: &Levers) -> flight::controls::PilotControls {
    let wave = |period: u64| match (tick / period) % 4 {
        0 | 3 => 1.0,
        _ => -1.0,
    };
    let brake = if levers.ground && (900..1_000).contains(&tick) {
        0.3
    } else {
        0.0
    };
    flight::controls::PilotControls {
        pitch: 0.03 * wave(45),
        roll: 0.1 * wave(60),
        yaw: 0.05 * wave(75),
        throttle: levers.throttle,
        gear: levers.gear,
        flaps: levers.flaps,
        brake_l: brake,
        brake_r: brake,
        ..flight::controls::PilotControls::default()
    }
}

/// The PS-9 microbench: `n` copies of the spawned aircraft stepped through the control system and
/// the flight model in the live world (procedural geo, still air), with no other system running.
/// Every `SEGMENT_TICKS` they restart from the spawn (a copy, negligible next to the ticks). By
/// default each timed chunk is at least 64 segments of aircraft-ticks (76 800), so even one
/// aircraft is timed over a long enough stretch. Reports ns per aircraft-tick.
///
/// In the air the stick makes small slow manoeuvres. With `ground` the aircraft start on the runway
/// at a taxi throttle, with the gear down and take-off flaps, and weave and brake a little, so the
/// gear forces, the friction and the ground mode of the control system are timed; it fails if one
/// leaves the ground, since the number would then not be the ground's.
fn bench_aircraft(
    setup: &Setup,
    n: usize,
    ticks: Option<u64>,
    json: bool,
    ground: bool,
) -> Result<(), String> {
    if setup.content.is_none() {
        return Err("--aircraft needs --content (the aircraft comes from the pack)".into());
    }
    if n == 0 {
        return Err("--aircraft must be at least 1".into());
    }
    let segments = 64_u64.div_ceil(n as u64);
    let per_chunk = ticks.unwrap_or(BENCH_CHUNKS * SEGMENT_TICKS * segments) / BENCH_CHUNKS;
    if per_chunk == 0 {
        return Err(format!("--ticks must be at least {BENCH_CHUNKS}"));
    }
    let core = setup.core()?;
    let svc = core.services();
    let model = svc.aircraft.as_ref().ok_or("the pack has no aircraft")?;
    let fcs_model = svc.fcs.as_ref().ok_or("the pack has no control gains")?;
    let spawned = *core
        .ownship()
        .aircraft()
        .ok_or("the core did not spawn an aircraft")?;
    let env = svc.flight_env();
    let mut fleet = vec![spawned; n];
    // Gentle inputs on every axis, so the controls, actuators and the whole aero build-up are
    // exercised. Each is +, -, -, + over four periods: the stick commands rates, so this brings the
    // attitude back to the spawn's instead of letting it drift. The throttle stays where the
    // spawn's trim put it (in steady state the core spool is the dry fraction of the lever); on the
    // ground it is a taxi setting. The levers stay as the spawn left them.
    let trimmed_lever = if ground {
        TAXI_LEVER
    } else {
        spawned.state.engines[0].core * f64::from(flight::engine::DETENT)
    };
    let gear = if spawned.gear_lever_down { 1.0 } else { 0.0 };
    let flaps = sim_core::spawn::FLAP_DETENT_FRACTION[usize::from(spawned.flap_detent).min(2)];
    let levers = Levers {
        throttle: trimmed_lever,
        gear,
        flaps,
        ground,
    };
    let run = |fleet: &mut [sim_core::ownship::AircraftSlot], ticks: u64| {
        for tick in 0..ticks {
            let tick = tick % SEGMENT_TICKS;
            if tick == 0 {
                fleet.fill(spawned);
            }
            let pilot = fcs::PilotInput {
                controls: microbench_controls(tick, &levers),
                ..fcs::PilotInput::default()
            };
            let t_s = tick as f64 * sim_core::time::SIM_DT;
            for slot in fleet.iter_mut() {
                let cmd = fcs_model.step(model, &mut slot.state, &mut slot.fcs, &pilot, &env, t_s);
                model.step_tick(&mut slot.state, &cmd, &env, t_s);
            }
        }
    };
    run(&mut fleet, 600); // warm-up
    let mut ns = Vec::new();
    for _ in 0..BENCH_CHUNKS {
        let t0 = Instant::now();
        run(&mut fleet, per_chunk);
        ns.push(t0.elapsed().as_nanos() as f64 / (per_chunk as f64 * n as f64));
    }
    if fleet
        .iter()
        .any(|s| s.state.numerical_fault || !s.state.body.pos_bf.x.is_finite())
    {
        return Err("an aircraft faulted during the benchmark".into());
    }
    if ground && fleet.iter().any(|s| !s.state.ground.wow) {
        return Err("an aircraft left the ground during the ground benchmark".into());
    }
    ns.sort_by(f64::total_cmp);
    let (min, median, max) = (ns[0], ns[ns.len() / 2], ns[ns.len() - 1]);
    let assist = match setup.session.assist {
        Assist::Full => "full",
        Assist::Standard => "standard",
        Assist::Reduced => "reduced",
        Assist::Manual => "manual",
    };
    if json {
        println!(
            "{{\"schema\":\"fg-bench-aircraft/1\",\"aircraft\":{n},\"assist\":\"{assist}\",\"ground\":{ground},\"ticks\":{},\"ns_per_aircraft_tick_min\":{min:.1},\"ns_per_aircraft_tick_median\":{median:.1},\"ns_per_aircraft_tick_max\":{max:.1}}}",
            per_chunk * BENCH_CHUNKS
        );
    } else {
        println!(
            "{n} aircraft, {assist} assist{}, {} ticks each: median {median:.0} ns per aircraft-tick (min {min:.0}, max {max:.0})",
            if ground { " on the ground" } else { "" },
            per_chunk * BENCH_CHUNKS
        );
    }
    Ok(())
}

/// `0, k, 2k, ..., n` (and `n` itself if it is not a multiple of `k`).
fn every_ticks(n: u64, k: u64) -> Vec<u64> {
    let mut v: Vec<u64> = (0..=n / k).map(|i| i * k).collect();
    if !n.is_multiple_of(k) {
        v.push(n);
    }
    v
}

fn parse_ticks(list: &str) -> Result<Vec<u64>, String> {
    list.split(',')
        .map(|t| {
            t.trim()
                .parse::<u64>()
                .map_err(|e| format!("bad tick {t:?}: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn shared_flags_are_parsed_and_the_rest_returned() {
        let Parsed { setup, rest, on } = parse(
            &args(&[
                "--seed", "0xff", "--script", "erratic", "--ticks", "5", "--json",
            ]),
            &["--json"],
        )
        .unwrap();
        assert_eq!(setup.seed, 0xff);
        assert_eq!(setup.script, input_script::ERRATIC as u32);
        assert_eq!(rest, [("--ticks".to_string(), "5".to_string())]);
        assert_eq!(on, ["--json"]);
    }

    #[test]
    fn bad_flags_are_reported() {
        assert!(parse(&args(&["--seed"]), &[]).is_err());
        assert!(parse(&args(&["--seed", "xyz"]), &[]).is_err());
        assert!(parse(&args(&["--script", "wild"]), &[]).is_err());
        assert!(parse(&args(&["--content", "/no/such/file"]), &[]).is_err());
    }

    #[test]
    fn the_microbench_pilot_brakes_only_on_the_ground_and_leaves_the_levers_alone() {
        let air = Levers {
            throttle: 0.4,
            gear: 0.0,
            flaps: 0.0,
            ground: false,
        };
        let ground = Levers {
            gear: 1.0,
            flaps: 0.5,
            ground: true,
            ..air
        };
        for tick in 0..SEGMENT_TICKS {
            let a = microbench_controls(tick, &air);
            assert_eq!((a.brake_l, a.brake_r), (0.0, 0.0), "no brakes in the air");
            assert_eq!((a.throttle, a.gear, a.flaps), (0.4, 0.0, 0.0));
            let g = microbench_controls(tick, &ground);
            assert_eq!((g.gear, g.flaps), (1.0, 0.5));
            assert_eq!(g.brake_l, g.brake_r);
            assert_eq!(g.brake_l > 0.0, (900..1_000).contains(&tick), "tick {tick}");
            assert!(g.pitch.abs() <= 0.03 && g.roll.abs() <= 0.1 && g.yaw.abs() <= 0.05);
        }
    }

    #[test]
    fn every_lists_the_checkpoints() {
        assert_eq!(every_ticks(180, 60), [0, 60, 120, 180]);
        assert_eq!(every_ticks(130, 60), [0, 60, 120, 130]);
        assert_eq!(every_ticks(3_600, 60).len(), 61);
    }

    #[test]
    fn the_script_changes_the_input_hash_and_the_observer() {
        let run = |script| {
            let mut setup = parse(&[], &[]).unwrap().setup;
            setup.script = script;
            let mut core = setup.core().unwrap();
            advance_to(&mut core, 600);
            core.subsystem_hashes()
        };
        let quiet = run(input_script::NONE as u32);
        let busy = run(input_script::ERRATIC as u32);
        assert_ne!(quiet.input, busy.input);
        assert_ne!(quiet.ownship, busy.ownship);
        // The probe world does not depend on input.
        assert_eq!((quiet.rng, quiet.bodies), (busy.rng, busy.bodies));
    }
}
