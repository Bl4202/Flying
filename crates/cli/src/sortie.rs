//! `fg sortie`: the duel gates (ADR-45, ADR-48).
//!
//! - **BP1** (`--matrix bp`): the bot's sortie from the runway against a Recruit, seeds × the four
//!   assists; at least 40 of 50 successes under Standard, Reduced and Manual, Full informational
//!   (ADR-45 amendment 1). **BP2** (also `--matrix bp`): Standard against a Regular and a Veteran,
//!   informational. Or one cell with `--assist` and `--skill`.
//! - **MS5** (`--sweep`): the missile sweep, 2 aspects × 3 ranges × 200 shots against a target
//!   holding its course and a defending Veteran.
//! - **AI1, AI2, AI8** (`--ai-gates`): pursuit (50 seeds × 3 skills), the defence against the gun
//!   (50 seeds × Regular, Veteran and a non-manoeuvring copy), and holding fire.
//! - **AI4, AI5** over every BP, AI1 and AI2 run flown.
//!
//! Runs are spread over `--threads` threads, one core each. Prints a table per gate, writes the
//! `sortie_report/1` JSON (`--json`) and a 10 Hz CSV trace per bot sortie (`--traces`), and exits
//! 1 when a gate fails.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use sim_core::combat::report::{
    self, BanditWatch, CombatTrace, DefendReport, Opponent, PursueReport, ShotReport,
    SortieOutcome, SortieReport, SweepCell,
};
use sim_core::layout::crash_cause;
use sim_core::{Arena, Assist};

use crate::circuit::{TRACE_HEADER, trace_row};

/// The assist levels by name.
const LEVELS: [(&str, Assist); 4] = [
    ("full", Assist::Full),
    ("standard", Assist::Standard),
    ("reduced", Assist::Reduced),
    ("manual", Assist::Manual),
];

/// The skill names.
const SKILLS: [&str; 3] = ["recruit", "regular", "veteran"];

/// Parsed flags.
#[derive(Debug, PartialEq)]
pub struct Args {
    content: PathBuf,
    /// The bot sortie cells: (assist, skill, gate).
    cells: Vec<(&'static str, Assist, u8, bool)>,
    seeds: (u64, u64),
    arena: Arena,
    sweep: bool,
    ai_gates: bool,
    threads: usize,
    json: Option<PathBuf>,
    traces: Option<PathBuf>,
}

fn level(name: &str) -> Result<(&'static str, Assist), String> {
    LEVELS
        .iter()
        .copied()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| format!("unknown assist {name}"))
}

/// Parses `fg sortie` flags.
///
/// # Errors
/// A message for a missing value, an unknown flag or value, or a missing `--content`.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        content: PathBuf::new(),
        cells: Vec::new(),
        seeds: (1, 50),
        arena: Arena::Rugged,
        sweep: false,
        ai_gates: false,
        threads: std::thread::available_parallelism().map_or(1, usize::from),
        json: None,
        traces: None,
    };
    let mut content = None;
    let mut matrix = false;
    let mut assist = None;
    let mut skill = None;
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--sweep" => {
                a.sweep = true;
                continue;
            }
            "--ai-gates" => {
                a.ai_gates = true;
                continue;
            }
            _ => {}
        }
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--content" => content = Some(PathBuf::from(value)),
            "--matrix" if value == "bp" => matrix = true,
            "--matrix" => return Err(format!("unknown --matrix {value} (bp)")),
            "--assist" => assist = Some(level(value)?),
            "--skill" => {
                skill = Some(match value.as_str() {
                    "1" => 1,
                    "2" => 2,
                    "3" => 3,
                    other => return Err(format!("unknown --skill {other} (1, 2 or 3)")),
                });
            }
            "--seeds" => {
                let (lo, hi) = value.split_once('-').unwrap_or((value, value));
                let p = |s: &str| s.parse::<u64>().map_err(|e| format!("bad --seeds: {e}"));
                a.seeds = (p(lo)?, p(hi)?);
                if a.seeds.0 > a.seeds.1 {
                    return Err("--seeds A-B needs A ≤ B".into());
                }
            }
            "--arena" => {
                a.arena =
                    Arena::from_name(value).ok_or_else(|| format!("unknown arena {value}"))?;
            }
            "--threads" => {
                a.threads = value
                    .parse::<usize>()
                    .map_err(|e| format!("bad --threads: {e}"))?
                    .max(1);
            }
            "--json" => a.json = Some(PathBuf::from(value)),
            "--traces" => a.traces = Some(PathBuf::from(value)),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    a.content = content.ok_or("--content is required")?;
    if matrix {
        if assist.is_some() || skill.is_some() {
            return Err("--matrix bp names its own cells; drop --assist and --skill".into());
        }
        // Full is reported, not gated (owner decision D6, ADR-45 amendment 1): its 60° of bank and
        // 10°/s of flight-path rate cannot turn with a Recruit.
        for (name, lv) in LEVELS {
            a.cells.push((name, lv, 1, lv != Assist::Full));
        }
        a.cells.push(("standard", Assist::Standard, 2, false));
        a.cells.push(("standard", Assist::Standard, 3, false));
    } else if assist.is_some() || skill.is_some() {
        let (name, lv) = assist.unwrap_or(("standard", Assist::Standard));
        a.cells.push((name, lv, skill.unwrap_or(1), false));
    }
    if a.cells.is_empty() && !a.sweep && !a.ai_gates {
        return Err("nothing to fly: --matrix bp, --assist/--skill, --sweep or --ai-gates".into());
    }
    Ok(a)
}

/// A `crash_cause` by name.
fn cause_name(code: u8) -> &'static str {
    match usize::from(code) {
        crash_cause::AIRFRAME_IMPACT => "ground",
        crash_cause::SINK_RATE => "sink rate",
        crash_cause::GEAR_COLLAPSE_STOP => "gear collapse",
        crash_cause::TUNNELLING => "tunnelling",
        crash_cause::NUMERICAL => "numerical fault",
        crash_cause::SHOT_DOWN => "shot down",
        crash_cause::COLLISION => "collision",
        _ => "unknown",
    }
}

/// One job for a worker.
#[derive(Clone, Copy, Debug)]
enum Job {
    Sortie { cell: usize, seed: u64 },
    Pursue { skill: u8, seed: u64 },
    Defend { opponent: Opponent, seed: u64 },
    Shot { cell: usize, shot: u32 },
}

/// What a job gave.
#[derive(Clone, Debug)]
enum Done {
    Sortie {
        cell: usize,
        seed: u64,
        report: SortieReport,
        trace: String,
    },
    Pursue {
        skill: u8,
        report: PursueReport,
    },
    Defend {
        opponent: Opponent,
        report: DefendReport,
    },
    Shot {
        cell: usize,
        report: ShotReport,
    },
}

/// The sweep's cells, in the order of the table: target behaviour, then aspect, then range.
fn sweep_cells() -> Vec<SweepCell> {
    let mut out = Vec::new();
    for defending in [false, true] {
        for aspect_rad in report::SWEEP_ASPECTS_RAD {
            for range_m in report::SWEEP_RANGES_M {
                out.push(SweepCell {
                    aspect_rad,
                    range_m,
                    defending,
                });
            }
        }
    }
    out
}

/// The trace columns of a bot sortie: the circuit's, the bot's phase and the duel's.
fn sortie_trace_header() -> String {
    format!(
        "{},sortie_phase,gun_firing,enemy_gun_dist_m,seeker,signal,launches,hits_taken,explosions,explosion_dist_m\n",
        TRACE_HEADER.trim_end()
    )
}

fn sortie_trace_row(
    out: &mut String,
    s: &sim_core::circuit::report::Sample,
    phase: u8,
    c: &CombatTrace,
) {
    let mut row = String::new();
    trace_row(&mut row, s);
    let _ = writeln!(
        out,
        "{},{phase},{},{:.1},{},{:.3},{},{},{},{:.1}",
        row.trim_end(),
        u8::from(c.gun_firing),
        c.enemy_gun_dist_m,
        c.seeker,
        c.signal,
        c.launches,
        c.hits_taken,
        c.explosions,
        c.explosion_dist_m
    );
}

fn run_job(
    a: &Args,
    pack: &[u8],
    alpha: f64,
    cells: &[SweepCell],
    job: Job,
) -> Result<Done, String> {
    match job {
        Job::Sortie { cell, seed } => {
            let (_, assist, skill, _) = a.cells[cell];
            let mut trace = if a.traces.is_some() {
                sortie_trace_header()
            } else {
                String::new()
            };
            let mut tick = 0_u64;
            let want = a.traces.is_some();
            let report = report::bot_sortie(pack, a.arena, assist, skill, seed, |core, s| {
                if want && tick.is_multiple_of(6) {
                    let phase = core.input().sortie().map_or(0, |b| b.phase() as u8);
                    if let Some(c) = report::combat_trace(core) {
                        sortie_trace_row(&mut trace, s, phase, &c);
                    }
                }
                tick += 1;
            })?;
            Ok(Done::Sortie {
                cell,
                seed,
                report,
                trace,
            })
        }
        Job::Pursue { skill, seed } => Ok(Done::Pursue {
            skill,
            report: report::pursue_run(pack, a.arena, skill, seed)?,
        }),
        Job::Defend { opponent, seed } => Ok(Done::Defend {
            opponent,
            report: report::defend_run(pack, a.arena, opponent, seed)?,
        }),
        Job::Shot { cell, shot } => Ok(Done::Shot {
            cell,
            report: report::sweep_shot(pack, a.arena, &cells[cell], alpha, shot)?,
        }),
    }
}

/// Flies every job on `threads` threads.
fn fly_all(
    a: &Args,
    pack: &[u8],
    alpha: f64,
    cells: &[SweepCell],
    jobs: &[Job],
) -> Result<Vec<Done>, String> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<Result<Done, String>>>> = Mutex::new(vec![None; jobs.len()]);
    std::thread::scope(|scope| {
        for _ in 0..a.threads.min(jobs.len()).max(1) {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&job) = jobs.get(i) else { break };
                    let r = run_job(a, pack, alpha, cells, job);
                    if let Ok(mut o) = out.lock() {
                        o[i] = Some(r);
                    }
                }
            });
        }
    });
    let results = out
        .into_inner()
        .map_err(|_| "a worker panicked".to_owned())?;
    results
        .into_iter()
        .map(|r| r.unwrap_or_else(|| Err("a job did not run".into())))
        .collect()
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        f64::midpoint(v[n / 2 - 1], v[n / 2])
    }
}

/// The result of one gate.
struct Gate {
    id: &'static str,
    pass: bool,
    detail: String,
}

/// Runs the gates. `Ok(true)` when every gate flown passes.
///
/// # Errors
/// A message when the pack cannot be read or loaded, a run cannot be set up, or a file cannot be
/// written.
#[allow(clippy::too_many_lines)] // ALGO: one table per gate, in order
pub fn run(args: &[String], build: &str) -> Result<bool, String> {
    let a = parse(args)?;
    let pack = std::fs::read(&a.content)
        .map_err(|e| format!("cannot read {}: {e}", a.content.display()))?;
    let cells = sweep_cells();
    let alpha = if a.sweep {
        report::sweep_alpha(&pack, a.arena)?
    } else {
        0.0
    };
    let mut jobs = Vec::new();
    for cell in 0..a.cells.len() {
        for seed in a.seeds.0..=a.seeds.1 {
            jobs.push(Job::Sortie { cell, seed });
        }
    }
    if a.ai_gates {
        for skill in 1..=3 {
            for seed in a.seeds.0..=a.seeds.1 {
                jobs.push(Job::Pursue { skill, seed });
            }
        }
        for opponent in [Opponent::Skill(2), Opponent::Skill(3), Opponent::HoldCourse] {
            for seed in a.seeds.0..=a.seeds.1 {
                jobs.push(Job::Defend { opponent, seed });
            }
        }
    }
    if a.sweep {
        for cell in 0..cells.len() {
            for shot in 0..report::SWEEP_SHOTS {
                jobs.push(Job::Shot { cell, shot });
            }
        }
    }
    let t0 = std::time::Instant::now();
    let done = fly_all(&a, &pack, alpha, &cells, &jobs)?;
    eprintln!(
        "fg sortie: {} runs on {} threads in {:.1} s",
        jobs.len(),
        a.threads,
        t0.elapsed().as_secs_f64()
    );

    let mut gates: Vec<Gate> = Vec::new();
    let mut watches: Vec<(String, BanditWatch)> = Vec::new();
    let mut recruit_rounds = 0_u32;
    let mut out_json =
        json!({ "schema": "sortie_report/1", "build": build, "arena": a.arena.name() });

    // The bot's sorties.
    let mut sortie_json = Vec::new();
    if !a.cells.is_empty() {
        println!("## Bot sorties (BP1, BP2)");
        println!("\n| assist | skill | successes | deaths | kills | missiles | failures |");
        println!("|---|---|---|---|---|---|---|");
    }
    for (ci, &(name, _, skill, gate)) in a.cells.iter().enumerate() {
        let runs: Vec<(u64, &SortieReport)> = done
            .iter()
            .filter_map(|d| match d {
                Done::Sortie {
                    cell, seed, report, ..
                } if *cell == ci => Some((*seed, report)),
                _ => None,
            })
            .collect();
        let ok = runs
            .iter()
            .filter(|(_, r)| r.outcome == SortieOutcome::Success)
            .count();
        let deaths: u32 = runs.iter().map(|(_, r)| r.deaths).sum();
        let kills: u32 = runs.iter().map(|(_, r)| r.kills).sum();
        let missiles: u32 = runs.iter().map(|(_, r)| r.missiles_fired).sum();
        let mut failures = String::new();
        for (seed, r) in runs
            .iter()
            .filter(|(_, r)| r.outcome != SortieOutcome::Success)
        {
            if r.outcome == SortieOutcome::Destroyed {
                let _ = write!(
                    failures,
                    "{seed}: destroyed ({}); ",
                    cause_name(r.crash_cause)
                );
            } else {
                let _ = write!(failures, "{seed}: {}; ", r.outcome.name());
            }
        }
        println!(
            "| {name} | {} | {ok}/{} | {deaths} | {kills} | {missiles} | {} |",
            SKILLS[usize::from(skill - 1)],
            runs.len(),
            failures.trim_end_matches("; ")
        );
        for (seed, r) in &runs {
            watches.push((format!("bp/{name}/{skill}/{seed}"), r.bandit));
            if skill == 1 {
                recruit_rounds += r.bandit.rounds_fired;
            }
            sortie_json.push(json!({
                "assist": name,
                "skill": skill,
                "seed": seed,
                "outcome": r.outcome.name(),
                "crash_cause": cause_name(r.crash_cause),
                "phase": format!("{:?}", r.phase),
                "lift_off_s": r.lift_off_s,
                "kill_s": r.kill.map(|k| k.0),
                "kill_weapon": r.kill.map(|k| k.1),
                "stop_s": r.stop.map(|s| s.0),
                "stop_along_m": r.stop.map(|s| s.1),
                "stop_across_m": r.stop.map(|s| s.2),
                "deaths": r.deaths,
                "kills": r.kills,
                "rounds_fired": r.rounds_fired,
                "missiles_fired": r.missiles_fired,
                "missile_hits": r.missile_hits,
                "hits_taken": r.hits_taken,
                "end_s": r.end_s,
                "bandit_rounds": r.bandit.rounds_fired,
            }));
        }
        if gate {
            gates.push(Gate {
                id: "BP1",
                pass: ok >= report::BP1_NEEDED * runs.len() / 50,
                detail: format!("{name}: {ok}/{}", runs.len()),
            });
        }
    }
    out_json["sorties"] = Value::Array(sortie_json);

    if a.ai_gates {
        // AI1 and AI8.
        println!("\n## The bandit pursues (AI1, AI8)");
        println!("\n| skill | gun position in 180 s | median time, s | fired in |");
        println!("|---|---|---|---|");
        let mut ai1 = Vec::new();
        for skill in 1..=3_u8 {
            let runs: Vec<&PursueReport> = done
                .iter()
                .filter_map(|d| match d {
                    Done::Pursue { skill: k, report } if *k == skill => Some(report),
                    _ => None,
                })
                .collect();
            let n = runs.len().max(1);
            let reached = runs.iter().filter(|r| r.reached_s.is_some()).count();
            let fired = runs.iter().filter(|r| r.fired).count();
            let t = median(runs.iter().filter_map(|r| r.reached_s).collect());
            println!(
                "| {} | {reached}/{n} | {t:.1} | {fired}/{n} |",
                SKILLS[usize::from(skill - 1)]
            );
            for (i, r) in runs.iter().enumerate() {
                watches.push((format!("ai1/{skill}/{i}"), r.bandit));
                if skill == 1 {
                    recruit_rounds += r.bandit.rounds_fired;
                }
            }
            gates.push(Gate {
                id: "AI1",
                pass: reached as f64 >= report::AI1_NEEDED * n as f64,
                detail: format!("{}: {reached}/{n}", SKILLS[usize::from(skill - 1)]),
            });
            if skill > 1 {
                gates.push(Gate {
                    id: "AI8",
                    pass: fired as f64 >= report::AI8_FIRE_NEEDED * n as f64,
                    detail: format!("{} fired in {fired}/{n}", SKILLS[usize::from(skill - 1)]),
                });
            }
            ai1.push(json!({ "skill": skill, "reached": reached, "runs": n, "median_s": t, "fired": fired }));
        }
        out_json["ai1"] = Value::Array(ai1);

        // AI2.
        println!("\n## The bandit defends against the gun (AI2)");
        println!("\n| opponent | median solution held, s |");
        println!("|---|---|");
        let held = |o: Opponent| -> Vec<f64> {
            done.iter()
                .filter_map(|d| match d {
                    Done::Defend { opponent, report } if *opponent == o => Some(report.held_s),
                    _ => None,
                })
                .collect()
        };
        let mut medians = Vec::new();
        for (name, o) in [
            ("regular", Opponent::Skill(2)),
            ("veteran", Opponent::Skill(3)),
            ("hold course", Opponent::HoldCourse),
        ] {
            let m = median(held(o));
            println!("| {name} | {m:.2} |");
            medians.push((name, m));
        }
        for d in &done {
            if let Done::Defend { opponent, report } = d {
                watches.push((format!("ai2/{opponent:?}"), report.bandit));
            }
        }
        let hold = medians[2].1;
        gates.push(Gate {
            id: "AI2",
            pass: medians[0].1 < hold && medians[1].1 < hold,
            detail: format!(
                "regular {:.2} s, veteran {:.2} s, hold course {hold:.2} s",
                medians[0].1, medians[1].1
            ),
        });
        out_json["ai2"] = json!(
            medians
                .iter()
                .map(|(n, m)| json!({ "opponent": n, "median_held_s": m }))
                .collect::<Vec<_>>()
        );
    }

    if a.sweep {
        println!(
            "\n## The missile sweep (MS5; launcher alpha {:.2}°)",
            alpha.to_degrees()
        );
        println!(
            "\n| target | aspect | range | launched | IN RNG | hits | hit rate | hit rate IN RNG |"
        );
        println!("|---|---|---|---|---|---|---|---|");
        let mut rates = vec![(0.0_f64, 0.0_f64, 0_usize, 0_usize); cells.len()];
        let mut sweep_json = Vec::new();
        for (ci, cell) in cells.iter().enumerate() {
            let shots: Vec<&ShotReport> = done
                .iter()
                .filter_map(|d| match d {
                    Done::Shot { cell, report } if *cell == ci => Some(report),
                    _ => None,
                })
                .collect();
            let launched = shots.iter().filter(|s| s.launched).count();
            let in_rng = shots.iter().filter(|s| s.launched && s.in_range).count();
            let hits = shots.iter().filter(|s| s.hit).count();
            let hits_in = shots.iter().filter(|s| s.hit && s.in_range).count();
            let rate = hits as f64 / launched.max(1) as f64;
            let rate_in = hits_in as f64 / in_rng.max(1) as f64;
            rates[ci] = (rate, rate_in, in_rng, launched);
            println!(
                "| {} | {:.0}° | {:.0} m | {launched}/{} | {in_rng} | {hits} | {:.1} % | {} |",
                if cell.defending {
                    "defending"
                } else {
                    "holding"
                },
                cell.aspect_rad.to_degrees(),
                cell.range_m,
                shots.len(),
                100.0 * rate,
                if in_rng > 0 {
                    format!("{:.1} %", 100.0 * rate_in)
                } else {
                    "-".into()
                }
            );
            sweep_json.push(json!({
                "defending": cell.defending,
                "aspect_deg": cell.aspect_rad.to_degrees(),
                "range_m": cell.range_m,
                "shots": shots.len(),
                "launched": launched,
                "in_range": in_rng,
                "hits": hits,
                "hits_in_range": hits_in,
            }));
        }
        out_json["sweep"] = json!({ "alpha_deg": alpha.to_degrees(), "cells": sweep_json });
        // A cell is judged when the seeker locked the holding target there and the predictor
        // marked IN RNG; where it cannot lock (the crossing 30° shot at 1 km drifts out of the
        // caged field of view before the lock, the 30° shot at 3 km is too faint), no missile
        // leaves and there is no rate to judge.
        let half = cells.len() / 2;
        let judged: Vec<usize> = (0..half)
            .filter(|&i| rates[i].2 > 0 && rates[half + i].3 > 0)
            .collect();
        let holding_ok = rates[..half]
            .iter()
            .all(|&(_, r_in, n_in, _)| n_in == 0 || r_in >= report::SWEEP_HIT_NEEDED);
        let lower = judged.iter().all(|&i| rates[half + i].0 < rates[i].0);
        let below = judged
            .iter()
            .all(|&i| rates[half + i].0 < report::SWEEP_DEFEND_MAX);
        let above = judged
            .iter()
            .any(|&i| rates[half + i].0 > report::SWEEP_DEFEND_MIN);
        gates.push(Gate {
            id: "MS5",
            pass: holding_ok && lower && below && above && !judged.is_empty(),
            detail: format!(
                "{} of {half} cells judged; holding ≥ 85 % where IN RNG: {holding_ok}; defending lower: {lower}, < 95 %: {below}, > 5 % somewhere: {above}",
                judged.len()
            ),
        });
    }

    if !watches.is_empty() {
        let over: Vec<&String> = watches
            .iter()
            .filter(|(_, w)| !w.same_physics())
            .map(|(n, _)| n)
            .collect();
        let max_cas = watches
            .iter()
            .map(|(_, w)| w.max_cas_over_vmo)
            .fold(0.0, f64::max);
        let impacts: u32 = watches.iter().map(|(_, w)| w.live_ground_impacts).sum();
        let impact_runs: Vec<&String> = watches
            .iter()
            .filter(|(_, w)| w.live_ground_impacts > 0)
            .map(|(n, _)| n)
            .collect();
        let sorties: u32 = watches.iter().map(|(_, w)| w.bandits).sum();
        println!(
            "\n## The bandit's airframe and ground (AI4, AI5), over {} runs and {sorties} bandits",
            watches.len()
        );
        println!(
            "\nOVERSTRESS or CAS over V_mo × 1.01 in {} runs; the highest CAS was {:.3} V_mo; {impacts} ground impacts by a living bandit; the Recruit fired {recruit_rounds} rounds.",
            over.len(),
            max_cas
        );
        gates.push(Gate {
            id: "AI4",
            pass: over.is_empty(),
            detail: format!("{} runs over, highest CAS {max_cas:.3} V_mo", over.len()),
        });
        gates.push(Gate {
            id: "AI5",
            pass: impacts == 0,
            detail: format!("{impacts} impacts over {sorties} bandits"),
        });
        gates.push(Gate {
            id: "AI8",
            pass: recruit_rounds == 0,
            detail: format!("the Recruit fired {recruit_rounds} rounds"),
        });
        out_json["bandit"] = json!({
            "runs": watches.len(),
            "bandits": sorties,
            "over": over,
            "max_cas_over_vmo": max_cas,
            "live_ground_impacts": impacts,
            "impact_runs": impact_runs,
            "recruit_rounds": recruit_rounds,
        });
    }

    println!("\n## Gates\n");
    for g in &gates {
        println!(
            "- {} {}: {}",
            g.id,
            if g.pass { "pass" } else { "FAIL" },
            g.detail
        );
    }
    let pass = gates.iter().all(|g| g.pass);
    out_json["gates"] = json!(
        gates
            .iter()
            .map(|g| json!({ "id": g.id, "pass": g.pass, "detail": g.detail }))
            .collect::<Vec<_>>()
    );
    out_json["pass"] = json!(pass);
    if let Some(path) = &a.json {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(&out_json).map_err(|e| e.to_string())?;
        std::fs::write(path, text + "\n")
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    if let Some(dir) = &a.traces {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
        for d in &done {
            if let Done::Sortie {
                cell, seed, trace, ..
            } = d
            {
                let (name, _, skill, _) = a.cells[*cell];
                let path = dir.join(format!(
                    "{name}_{}_{seed:02}.csv",
                    SKILLS[usize::from(skill - 1)]
                ));
                std::fs::write(&path, trace)
                    .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
            }
        }
    }
    println!("\ngate: {}", if pass { "pass" } else { "FAIL" });
    Ok(pass)
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
            "--matrix",
            "bp",
            "--seeds",
            "3-7",
            "--sweep",
            "--ai-gates",
            "--threads",
            "4",
            "--json",
            "r.json",
            "--traces",
            "t",
        ]))
        .unwrap();
        assert_eq!(a.cells.len(), 6);
        assert_eq!(
            a.cells
                .iter()
                .filter(|c| c.3)
                .map(|c| c.0)
                .collect::<Vec<_>>(),
            ["standard", "reduced", "manual"],
            "BP1 gates three assists; Full is informational"
        );
        assert_eq!(
            (a.seeds, a.threads, a.sweep, a.ai_gates),
            ((3, 7), 4, true, true)
        );
        let one = parse(&args(&[
            "--content",
            "x",
            "--assist",
            "manual",
            "--skill",
            "3",
            "--seeds",
            "9",
        ]))
        .unwrap();
        assert_eq!(one.cells, [("manual", Assist::Manual, 3, false)]);
        assert_eq!(one.seeds, (9, 9));
        assert!(parse(&args(&["--content", "x"])).is_err(), "nothing to fly");
        assert!(parse(&args(&["--content", "x", "--matrix", "bp", "--skill", "2"])).is_err());
        assert!(parse(&args(&["--content", "x", "--skill", "4"])).is_err());
        assert!(parse(&args(&["--content", "x", "--seeds", "9-3", "--sweep"])).is_err());
        assert!(parse(&args(&["--sweep"])).is_err(), "--content is required");
    }

    #[test]
    fn the_median_of_an_even_count_is_the_mean_of_the_middle_two() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), 2.5);
        assert!(median(Vec::new()).is_nan());
    }

    #[test]
    fn the_sweep_has_twelve_cells_holding_first() {
        let c = sweep_cells();
        assert_eq!(c.len(), 12);
        assert!(c[..6].iter().all(|c| !c.defending) && c[6..].iter().all(|c| c.defending));
    }
}
