//! `cargo xtask golden-diff --against <git ref> [--fresh]`: the proof that a re-bless of the
//! determinism goldens changed what ADR-45 (D2, DT1) says it may and nothing else.
//!
//! v0.1d puts the weapon and AI data into the simulation pack, and the pack's SHA-256 is in the
//! state hash, so the `content` and `combined` columns of every golden move; the seventh column,
//! `combat`, is new. The four free-flight goldens are re-blessed once, and this command shows that
//! the re-bless is only that. For each of `determinism`, `flight60`, `circuit` and `plain-circuit`
//! it reads the golden as it was at `<ref>` and compares it with the golden as it is now (or, with
//! `--fresh`, with the hashes of a fresh native run, which is the check while the goldens are still
//! the old ones). It fails unless, for every file:
//!
//! 1. the seed, script, session and tick count are the same, with the same checkpoints;
//! 2. `tick_h`, `rng`, `bodies`, `ownship` and `input` are identical at every checkpoint;
//! 3. `content` is one value at every checkpoint (and the same value in every file);
//! 4. `combat` is the no-combat constant at every checkpoint;
//! 5. `combined` is what the seven columns hash to, so it moved only because `content` and
//!    `combat` did.

use std::fs;
use std::process::Command;

use serde_json::Value;
use sim_core::{SubsystemHashes, no_combat_hash};

use crate::determinism::{bake_and_build_release, fresh_golden};
use crate::util::{run_capture, workspace_root};

/// The goldens that must keep their meaning, by file name under `tests/golden/`.
const FILES: [&str; 4] = ["determinism", "flight60", "circuit", "plain-circuit"];
/// Columns that must not move.
const UNCHANGED: [&str; 5] = ["tick_h", "rng", "bodies", "ownship", "input"];
/// Length of the `erratic` suite's golden, ticks (as `cargo xtask determinism` runs it).
const ERRATIC_TICKS: u64 = 1_000_000;

struct Options {
    against: String,
    fresh: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut against = None;
    let mut fresh = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--against" => against = Some(it.next().ok_or("--against needs a git ref")?.clone()),
            "--fresh" => fresh = true,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Options {
        against: against.ok_or("--against <git ref> is required (for example `v0.1c`)")?,
        fresh,
    })
}

pub fn run_command(args: &[String]) -> Result<(), String> {
    let opts = parse(args)?;
    let root = workspace_root();
    let git = |what: &str| -> Result<String, String> {
        run_capture(Command::new("git").current_dir(&root).args(["show", what]))
    };
    run_capture(
        Command::new("git")
            .current_dir(&root)
            .args(["rev-parse", "--verify", "--quiet"])
            .arg(format!("{}^{{commit}}", opts.against)),
    )
    .map_err(|_| format!("`{}` is not a git ref of this repository", opts.against))?;

    let content = if opts.fresh {
        Some(bake_and_build_release(&root)?)
    } else {
        None
    };
    let mut pairs = Vec::new();
    for name in FILES {
        let rel = format!("tests/golden/{name}.json");
        let old = parse_golden(&rel, &git(&format!("{}:{rel}", opts.against))?)?;
        let new = match &content {
            Some(c) => fresh_golden(&root, &rel, c, ERRATIC_TICKS)?,
            None => parse_golden(
                &rel,
                &fs::read_to_string(root.join(&rel)).map_err(|e| format!("{rel}: {e}"))?,
            )?,
        };
        pairs.push((name, old, new));
    }
    let report = compare_all(&pairs)?;
    for line in &report {
        println!("{line}");
    }
    println!(
        "OK: against {} the goldens differ only in `content` and `combined`, and `combat` is the \
         no-combat constant{}",
        opts.against,
        if opts.fresh {
            " (fresh native hashes; the files are not re-blessed yet)"
        } else {
            ""
        }
    );
    Ok(())
}

fn parse_golden(what: &str, text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|e| format!("{what}: not JSON: {e}"))
}

/// A checkpoint column as a hex word.
fn word(cp: &Value, key: &str) -> Result<u64, String> {
    let text = cp[key]
        .as_str()
        .ok_or_else(|| format!("tick {}: no `{key}` column", cp["tick"]))?;
    u64::from_str_radix(text, 16)
        .map_err(|_| format!("tick {}: `{key}` is not a hex word: {text}", cp["tick"]))
}

/// Compares every pair and the content value across them; one line per file.
fn compare_all(pairs: &[(&str, Value, Value)]) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    let mut content_values = Vec::new();
    for (name, old, new) in pairs {
        let (line, content) = compare(name, old, new)?;
        lines.push(line);
        content_values.push((name, content));
    }
    if let Some((first, rest)) = content_values.split_first()
        && let Some((other, _)) = rest.iter().find(|(_, c)| *c != first.1)
    {
        return Err(format!(
            "`content` is {:016x} in {}.json but differs in {other}.json: every golden loads the \
             same pack, so it must be one value",
            first.1, first.0
        ));
    }
    Ok(lines)
}

/// The rules for one golden. Returns the report line and the file's `content` value.
fn compare(name: &str, old: &Value, new: &Value) -> Result<(String, u64), String> {
    let fail = |msg: String| format!("{name}.json: {msg}");
    for key in ["seed", "script", "ticks"] {
        if old[key] != new[key] {
            return Err(fail(format!(
                "`{key}` is {} in the old golden and {} now",
                old[key], new[key]
            )));
        }
    }
    // A golden recorded before the session was written down has none; every v0.1c golden has one.
    if old["session"] != new["session"] {
        return Err(fail(format!(
            "the session was {} and is {}: the suite is not the same run",
            old["session"], new["session"]
        )));
    }
    let (old_cp, new_cp) = (checkpoints(old, name)?, checkpoints(new, name)?);
    if old_cp.len() != new_cp.len() {
        return Err(fail(format!(
            "{} checkpoints before, {} now",
            old_cp.len(),
            new_cp.len()
        )));
    }
    let no_combat = no_combat_hash();
    let mut content = None;
    let mut combined_changed = false;
    for (o, n) in old_cp.iter().zip(new_cp) {
        let tick = &n["tick"];
        if o["tick"] != *tick {
            return Err(fail(format!(
                "a checkpoint at tick {} became tick {tick}",
                o["tick"]
            )));
        }
        for col in UNCHANGED {
            if o[col] != n[col] {
                return Err(fail(format!(
                    "`{col}` moved at tick {tick}: {} before, {} now. Free-flight behaviour \
                     changed; find the change and undo it",
                    o[col], n[col]
                )));
            }
        }
        let c = word(n, "content").map_err(fail)?;
        if *content.get_or_insert(c) != c {
            return Err(fail(format!(
                "`content` is not one value: {:016x} at the first checkpoint, {c:016x} at tick \
                 {tick}",
                content.unwrap_or(c)
            )));
        }
        let combat = word(n, "combat").map_err(fail)?;
        if combat != no_combat {
            return Err(fail(format!(
                "`combat` is {combat:016x} at tick {tick}, not the no-combat constant \
                 {no_combat:016x}: this suite has combat state"
            )));
        }
        let hashes = SubsystemHashes::new(
            word(n, "tick_h").map_err(fail)?,
            word(n, "rng").map_err(fail)?,
            word(n, "bodies").map_err(fail)?,
            word(n, "ownship").map_err(fail)?,
            word(n, "input").map_err(fail)?,
            c,
            combat,
        );
        let combined = word(n, "combined").map_err(fail)?;
        if hashes.combined != combined {
            return Err(fail(format!(
                "`combined` at tick {tick} is {combined:016x} but its seven columns hash to \
                 {:016x}",
                hashes.combined
            )));
        }
        combined_changed |= o["combined"] != n["combined"];
    }
    let content = content.ok_or_else(|| fail("no checkpoints".into()))?;
    let line = format!(
        "{name}.json: {} checkpoints; {} identical; content one value ({content:016x}); combat = \
         no-combat; combined {}",
        new_cp_len(new),
        UNCHANGED.join(", "),
        if combined_changed {
            "changed"
        } else {
            "unchanged"
        }
    );
    Ok((line, content))
}

fn new_cp_len(golden: &Value) -> usize {
    golden["checkpoints"].as_array().map_or(0, Vec::len)
}

fn checkpoints<'a>(golden: &'a Value, name: &str) -> Result<&'a [Value], String> {
    golden["checkpoints"]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{name}.json: no `checkpoints` list"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hex(v: u64) -> String {
        format!("{v:016x}")
    }

    /// An old golden: `n` checkpoints of six-column hashes (no `combat`), pack identity `content`.
    fn old_golden(n: u64, content: u64) -> Value {
        let cps: Vec<Value> = (0..n)
            .map(|i| {
                json!({
                    "tick": i * 60,
                    "tick_h": hex(100 + i), "rng": hex(200 + i), "bodies": hex(300 + i),
                    "ownship": hex(400 + i), "input": hex(500 + i),
                    "content": hex(content), "combined": hex(900 + i),
                })
            })
            .collect();
        json!({ "seed": "1f", "script": "circuit", "session": "--spawn runway", "ticks": n * 60,
                "checkpoints": cps })
    }

    /// What a re-bless produces from `old` with a new pack identity.
    fn rebless(old: &Value, content: u64) -> Value {
        let mut new = old.clone();
        for cp in new["checkpoints"].as_array_mut().unwrap() {
            let w = |k: &str| u64::from_str_radix(cp[k].as_str().unwrap(), 16).unwrap();
            let combat = no_combat_hash();
            let h = SubsystemHashes::new(
                w("tick_h"),
                w("rng"),
                w("bodies"),
                w("ownship"),
                w("input"),
                content,
                combat,
            );
            cp["content"] = json!(hex(content));
            cp["combat"] = json!(hex(combat));
            cp["combined"] = json!(hex(h.combined));
        }
        new
    }

    fn broken(new: &Value, edit: impl FnOnce(&mut Value)) -> Value {
        let mut b = new.clone();
        edit(&mut b);
        b
    }

    #[test]
    fn a_re_bless_that_moves_only_content_and_combined_passes() {
        let old = old_golden(4, 1);
        let new = rebless(&old, 2);
        let (line, content) = compare("circuit", &old, &new).unwrap();
        assert_eq!(content, 2);
        assert!(line.starts_with(
            "circuit.json: 4 checkpoints; tick_h, rng, bodies, ownship, input identical;"
        ));
        assert!(line.contains("content one value") && line.ends_with("combined changed"));
    }

    #[test]
    fn each_column_that_must_not_move_is_checked() {
        let old = old_golden(3, 1);
        let new = rebless(&old, 2);
        for col in UNCHANGED {
            let bad = broken(&new, |g| g["checkpoints"][1][col] = json!(hex(7)));
            let e = compare("circuit", &old, &bad).unwrap_err();
            assert!(e.contains(col) && e.contains("tick 60"), "{e}");
        }
    }

    #[test]
    fn the_seed_script_session_ticks_and_checkpoints_must_be_the_same() {
        let old = old_golden(3, 1);
        let new = rebless(&old, 2);
        for (key, value) in [
            ("seed", json!("2e")),
            ("script", json!("erratic")),
            ("session", json!("--spawn air")),
            ("ticks", json!(999)),
        ] {
            let bad = broken(&new, |g| g[key] = value);
            let e = compare("circuit", &old, &bad).unwrap_err();
            assert!(e.contains(key), "{e}");
        }
        let fewer = broken(&new, |g| {
            g["checkpoints"].as_array_mut().unwrap().pop();
        });
        assert!(
            compare("circuit", &old, &fewer)
                .unwrap_err()
                .contains("3 checkpoints before, 2 now")
        );
        let moved = broken(&new, |g| g["checkpoints"][2]["tick"] = json!(121));
        assert!(
            compare("circuit", &old, &moved)
                .unwrap_err()
                .contains("became tick 121")
        );
    }

    #[test]
    fn content_must_be_one_value_in_a_file_and_across_files() {
        let old = old_golden(3, 1);
        let new = rebless(&old, 2);
        let bad = broken(&new, |g| g["checkpoints"][2]["content"] = json!(hex(3)));
        assert!(
            compare("circuit", &old, &bad)
                .unwrap_err()
                .contains("not one value")
        );

        let other = rebless(&old_golden(3, 1), 5);
        let e = compare_all(&[
            ("circuit", old.clone(), new.clone()),
            ("flight60", old.clone(), other),
        ])
        .unwrap_err();
        assert!(e.contains("flight60"), "{e}");
        assert!(
            compare_all(&[
                ("circuit", old.clone(), new.clone()),
                ("flight60", old, new)
            ])
            .is_ok()
        );
    }

    #[test]
    fn combat_must_be_the_no_combat_constant() {
        let old = old_golden(3, 1);
        let new = rebless(&old, 2);
        let bad = broken(&new, |g| g["checkpoints"][1]["combat"] = json!(hex(1)));
        let e = compare("circuit", &old, &bad).unwrap_err();
        assert!(e.contains("not the no-combat constant"), "{e}");
        let missing = broken(&new, |g| {
            g["checkpoints"][0]
                .as_object_mut()
                .unwrap()
                .remove("combat");
        });
        assert!(
            compare("circuit", &old, &missing)
                .unwrap_err()
                .contains("no `combat` column")
        );
    }

    #[test]
    fn combined_must_follow_from_its_columns() {
        let old = old_golden(3, 1);
        let new = rebless(&old, 2);
        // The old combined value (six columns, old content) is not the new one.
        let bad = broken(&new, |g| {
            g["checkpoints"][1]["combined"] = old["checkpoints"][1]["combined"].clone();
        });
        let e = compare("circuit", &old, &bad).unwrap_err();
        assert!(e.contains("seven columns hash to"), "{e}");
    }

    #[test]
    fn the_constant_is_the_hash_of_nocombat() {
        // FNV-1a 64 of the eight bytes `NOCOMBAT`: the value the re-blessed goldens carry.
        assert_eq!(hex(no_combat_hash()), "31ef7ff32813f286");
    }

    #[test]
    fn the_flags_are_parsed() {
        let args = |a: &[&str]| a.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let o = parse(&args(&["--against", "v0.1c", "--fresh"])).unwrap();
        assert!(o.fresh && o.against == "v0.1c");
        assert!(!parse(&args(&["--against", "v0.1c"])).unwrap().fresh);
        assert!(parse(&args(&[])).is_err());
        assert!(parse(&args(&["--against"])).is_err());
        assert!(parse(&args(&["--against", "x", "--bogus"])).is_err());
    }
}
