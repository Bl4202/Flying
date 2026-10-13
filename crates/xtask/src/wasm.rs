//! WebAssembly build pipeline (GDD 61.3).
//!
//! Two artifacts, built in separate target directories so flags cannot leak between them:
//!
//! * `single`: stable toolchain, `+simd128`; the module owns its memory.
//! * `threads`: pinned nightly with `-Z build-std` (std must be rebuilt with `+atomics`);
//!   imports a shared memory; needs cross-origin isolation at runtime (ADR-17, ADR-21).
//!
//! Each is then optimised with `wasm-opt -O3`, with the `--enable-*` flags derived from the
//! module's own `target_features` section so they cannot drift from what rustc emitted.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;
use wasmparser::{ExternalKind, Parser, Payload, TypeRef};

use crate::util::{home, run, sha256_hex, workspace_root};

const TARGET: &str = "wasm32-unknown-unknown";
/// Brotli budget for one sim-core module (GDD 61.3).
const BROTLI_BUDGET_BYTES: usize = 8 * 1024 * 1024;
/// Linear memory ceiling for the threads build: 2 GiB (GDD 0.6.3).
const MAX_MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// Initial shared memory the loader must create: 256 pages = 16 MiB.
pub const THREADS_INITIAL_PAGES: u64 = 256;

/// Every function the host calls (ABI v2, `crates/sim-wasm/src/abi.rs`).
const REQUIRED_EXPORTS: &[&str] = &[
    "fg_abi_version",
    "fg_init",
    "fg_core_state",
    "fg_content_reserve",
    "fg_load_content",
    "fg_start",
    "fg_pause",
    "fg_resume",
    "fg_set_input_script",
    "fg_input_ptr",
    "fg_input_capacity",
    "fg_push_inputs",
    "fg_set_profiling",
    "fg_step",
    "fg_snapshot_ptr",
    "fg_snapshot_len",
    "fg_profile_ptr",
    "fg_profile_len",
    "fg_terrain_ptr",
    "fg_terrain_fill",
    "fg_tick_lo",
    "fg_tick_hi",
    "fg_state_hash_lo",
    "fg_state_hash_hi",
    "fg_subsystem_hash_lo",
    "fg_subsystem_hash_hi",
];

/// The imports a module may have, as `module.name`. Anything else means the host would have to
/// supply something the contract does not mention: `env.fg_now` is the profiler's clock and
/// `env.memory` the shared memory of the threads build.
fn allowed_imports(v: Variant) -> BTreeSet<String> {
    let mut set: BTreeSet<String> = std::iter::once("env.fg_now".to_string()).collect();
    if v == Variant::Threads {
        set.insert("env.memory".to_string());
    }
    set
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Single,
    Threads,
}

impl Variant {
    fn name(self) -> &'static str {
        match self {
            Variant::Single => "single",
            Variant::Threads => "threads",
        }
    }
}

pub fn run_command(args: &[String]) -> Result<(), String> {
    let which = match args.first().map(String::as_str) {
        None | Some("all") => vec![Variant::Single, Variant::Threads],
        Some("single") => vec![Variant::Single],
        Some("threads") => vec![Variant::Threads],
        Some(other) => {
            return Err(format!(
                "unknown variant {other:?}; use single, threads or all"
            ));
        }
    };
    let root = workspace_root();
    let out_dir = root.join("wasm-out");
    fs::create_dir_all(&out_dir).map_err(|e| e.to_string())?;
    let mut entries = Vec::new();
    for v in which {
        entries.push(build_variant(&root, &out_dir, v)?);
    }
    // Merge with an existing manifest so building one variant keeps the other's entry.
    let manifest_path = out_dir.join("wasm-manifest.json");
    let mut all: Vec<serde_json::Value> = fs::read(&manifest_path)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v["variants"].as_array().cloned())
        .unwrap_or_default();
    for e in entries {
        all.retain(|x| x["variant"] != e["variant"]);
        all.push(e);
    }
    all.sort_by_key(|x| x["variant"].as_str().unwrap_or_default().to_string());
    let manifest = json!({ "abi_version": 1, "threads_initial_pages": THREADS_INITIAL_PAGES, "variants": all });
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("wrote {}", manifest_path.display());
    Ok(())
}

fn build_variant(root: &Path, out_dir: &Path, v: Variant) -> Result<serde_json::Value, String> {
    println!("== wasm {}", v.name());
    let target_dir = root.join("target").join(format!("wasm-{}", v.name()));
    let mut flags: Vec<String> = vec![
        "-C".into(),
        "target-feature=+simd128".into(),
        format!("--remap-path-prefix={}=/workspace", root.display()),
    ];
    let home = home()?;
    let cargo_home =
        std::env::var_os("CARGO_HOME").map_or_else(|| home.join(".cargo"), PathBuf::from);
    let rustup_home =
        std::env::var_os("RUSTUP_HOME").map_or_else(|| home.join(".rustup"), PathBuf::from);
    flags.push(format!(
        "--remap-path-prefix={}=/cargo",
        cargo_home.display()
    ));
    flags.push(format!(
        "--remap-path-prefix={}=/rustup",
        rustup_home.display()
    ));

    let mut cmd = Command::new("cargo");
    cmd.current_dir(root);
    cmd.env_remove("RUSTUP_TOOLCHAIN").env_remove("RUSTFLAGS");
    if v == Variant::Threads {
        let nightly = fs::read_to_string(root.join("rust-nightly-version"))
            .map_err(|e| format!("rust-nightly-version: {e}"))?;
        cmd.arg(format!("+{}", nightly.trim()));
        flags.extend([
            "-C".to_string(),
            "target-feature=+atomics,+bulk-memory,+mutable-globals".into(),
            "-C".into(),
            "link-arg=--shared-memory".into(),
            "-C".into(),
            "link-arg=--import-memory".into(),
            "-C".into(),
            format!(
                "link-arg=--initial-memory={}",
                THREADS_INITIAL_PAGES * 65_536
            ),
            "-C".into(),
            format!("link-arg=--max-memory={MAX_MEMORY_BYTES}"),
        ]);
    }
    cmd.args([
        "build",
        "-p",
        "sim-wasm",
        "--profile",
        "wasm",
        "--locked",
        "--target",
        TARGET,
    ]);
    if v == Variant::Threads {
        cmd.args(["-Z", "build-std=std,panic_abort"]);
    }
    cmd.arg("--target-dir").arg(&target_dir);
    cmd.env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"));
    run(&mut cmd)?;

    let raw = target_dir.join(TARGET).join("wasm").join("sim_wasm.wasm");
    let raw_bytes = fs::read(&raw).map_err(|e| format!("{}: {e}", raw.display()))?;
    let features = inspect(&raw_bytes)?.features;
    let opt_path = out_dir.join(format!("sim.{}.wasm", v.name()));
    let mut opt = Command::new("wasm-opt");
    opt.arg("-O3")
        .arg("--strip-debug")
        .args(opt_flags(&features)?)
        .arg("--emit-target-features");
    opt.arg(&raw).arg("-o").arg(&opt_path);
    run(&mut opt)?;

    let bytes = fs::read(&opt_path).map_err(|e| e.to_string())?;
    let info = inspect(&bytes)?;
    check(v, &info)?;
    let brotli_len = brotli_size(&bytes);
    if brotli_len > BROTLI_BUDGET_BYTES {
        return Err(format!(
            "{} is {brotli_len} B brotli, over the {BROTLI_BUDGET_BYTES} B budget",
            v.name()
        ));
    }
    println!(
        "   {} bytes, {} brotli, features: {}",
        bytes.len(),
        brotli_len,
        info.features.iter().cloned().collect::<Vec<_>>().join(",")
    );
    Ok(json!({
        "variant": v.name(),
        "file": format!("sim.{}.wasm", v.name()),
        "sha256": sha256_hex(&bytes),
        "bytes": bytes.len(),
        "brotli_bytes": brotli_len,
        "features": info.features.iter().collect::<Vec<_>>(),
        "imports_shared_memory": info.memory_import.is_some_and(|m| m.shared),
    }))
}

fn brotli_size(bytes: &[u8]) -> usize {
    let mut out = Vec::new();
    let params = brotli::enc::BrotliEncoderParams {
        quality: 11,
        ..Default::default()
    };
    brotli::BrotliCompress(&mut &bytes[..], &mut out, &params).expect("in-memory brotli");
    out.len()
}

/// What we need to know about a module.
#[derive(Debug, Default)]
pub struct Inspect {
    pub features: BTreeSet<String>,
    pub memory_import: Option<wasmparser::MemoryType>,
    /// Every import as `module.name`.
    pub imports: BTreeSet<String>,
    pub defines_memory: bool,
    pub exports: BTreeSet<String>,
}

pub fn inspect(bytes: &[u8]) -> Result<Inspect, String> {
    let mut info = Inspect::default();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(|e| e.to_string())? {
            Payload::CustomSection(c) if c.name() == "target_features" => {
                info.features = parse_target_features(c.data())?;
            }
            Payload::ImportSection(r) => {
                for imp in r.into_imports() {
                    let imp = imp.map_err(|e| e.to_string())?;
                    info.imports.insert(format!("{}.{}", imp.module, imp.name));
                    if let TypeRef::Memory(m) = imp.ty {
                        info.memory_import = Some(m);
                    }
                }
            }
            Payload::MemorySection(r) => info.defines_memory = r.count() > 0,
            Payload::ExportSection(r) => {
                for e in r {
                    let e = e.map_err(|e| e.to_string())?;
                    if e.kind == ExternalKind::Func || e.kind == ExternalKind::Memory {
                        info.exports.insert(e.name.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(info)
}

/// Parses the tool-conventions `target_features` section: `vec(prefix:u8, name:string)`.
/// Returns the names of features marked used (`+`) or required (`=`).
pub fn parse_target_features(data: &[u8]) -> Result<BTreeSet<String>, String> {
    fn leb(data: &[u8], pos: &mut usize) -> Result<usize, String> {
        let (mut result, mut shift) = (0_usize, 0);
        loop {
            let b = *data.get(*pos).ok_or("truncated target_features")?;
            *pos += 1;
            result |= usize::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
    }
    let mut pos = 0;
    let count = leb(data, &mut pos)?;
    let mut out = BTreeSet::new();
    for _ in 0..count {
        let prefix = *data.get(pos).ok_or("truncated target_features")?;
        pos += 1;
        let len = leb(data, &mut pos)?;
        let name = data
            .get(pos..pos + len)
            .ok_or("truncated target_features")?;
        pos += len;
        if prefix == b'+' || prefix == b'=' {
            out.insert(String::from_utf8(name.to_vec()).map_err(|e| e.to_string())?);
        }
    }
    Ok(out)
}

/// Maps rustc/LLVM feature names to `wasm-opt --enable-*` flags. Unknown names are an error, so
/// a new default feature never silently goes un-enabled (wasm-opt would reject the module).
pub fn opt_flags(features: &BTreeSet<String>) -> Result<Vec<String>, String> {
    let mut flags = Vec::new();
    for f in features {
        let flag = match f.as_str() {
            "simd128" => "--enable-simd",
            "bulk-memory" => "--enable-bulk-memory",
            "bulk-memory-opt" => "--enable-bulk-memory-opt",
            "atomics" => "--enable-threads",
            "mutable-globals" => "--enable-mutable-globals",
            "sign-ext" => "--enable-sign-ext",
            "nontrapping-fptoint" => "--enable-nontrapping-float-to-int",
            "multivalue" => "--enable-multivalue",
            "reference-types" => "--enable-reference-types",
            "extended-const" => "--enable-extended-const",
            "call-indirect-overlong" => "--enable-call-indirect-overlong",
            "relaxed-simd" => return Err("relaxed-simd is forbidden (ADR-10)".into()),
            other => {
                return Err(format!(
                    "unmapped target feature {other:?}: add it to opt_flags"
                ));
            }
        };
        flags.push(flag.to_string());
    }
    Ok(flags)
}

fn check(v: Variant, info: &Inspect) -> Result<(), String> {
    if info.features.contains("relaxed-simd") {
        return Err(format!("{}: relaxed-simd present (ADR-10)", v.name()));
    }
    if !info.features.contains("simd128") {
        return Err(format!("{}: simd128 missing", v.name()));
    }
    for e in REQUIRED_EXPORTS {
        if !info.exports.contains(*e) {
            return Err(format!("{}: missing export {e}", v.name()));
        }
    }
    let allowed = allowed_imports(v);
    if info.imports != allowed {
        let unexpected: Vec<_> = info.imports.difference(&allowed).collect();
        let missing: Vec<_> = allowed.difference(&info.imports).collect();
        return Err(format!(
            "{}: imports must be exactly {allowed:?}; unexpected {unexpected:?}, missing {missing:?}",
            v.name()
        ));
    }
    match v {
        Variant::Single => {
            if info.memory_import.is_some() || !info.defines_memory {
                return Err("single: must define its own memory and import none".into());
            }
            if !info.exports.contains("memory") {
                return Err("single: must export its memory".into());
            }
            if info.features.contains("atomics") {
                return Err("single: must not use atomics".into());
            }
        }
        Variant::Threads => {
            let m = info
                .memory_import
                .ok_or("threads: must import its memory")?;
            if !m.shared {
                return Err("threads: imported memory must be shared".into());
            }
            if m.initial != THREADS_INITIAL_PAGES {
                return Err(format!(
                    "threads: initial memory {} pages, expected {THREADS_INITIAL_PAGES}",
                    m.initial
                ));
            }
            if !info.features.contains("atomics") {
                return Err("threads: atomics feature missing".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_target_features_section() {
        // count=3: +simd128, -foo, =atomics
        let mut d = vec![3];
        for (p, n) in [(b'+', "simd128"), (b'-', "foo"), (b'=', "atomics")] {
            d.push(p);
            d.push(u8::try_from(n.len()).unwrap());
            d.extend_from_slice(n.as_bytes());
        }
        let got = parse_target_features(&d).unwrap();
        assert_eq!(got.into_iter().collect::<Vec<_>>(), ["atomics", "simd128"]);
    }

    fn module(v: Variant, imports: &[&str]) -> Inspect {
        let mut info = Inspect {
            features: ["simd128", "atomics"]
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            imports: imports.iter().map(|s| (*s).to_string()).collect(),
            exports: REQUIRED_EXPORTS.iter().map(|s| (*s).to_string()).collect(),
            ..Inspect::default()
        };
        match v {
            Variant::Single => {
                info.features.remove("atomics");
                info.defines_memory = true;
                info.exports.insert("memory".into());
            }
            Variant::Threads => {
                info.memory_import = Some(wasmparser::MemoryType {
                    initial: THREADS_INITIAL_PAGES,
                    maximum: Some(32768),
                    shared: true,
                    memory64: false,
                    page_size_log2: None,
                });
            }
        }
        info
    }

    #[test]
    fn imports_must_match_the_allow_list_exactly() {
        let single = module(Variant::Single, &["env.fg_now"]);
        assert!(check(Variant::Single, &single).is_ok());
        let threads = module(Variant::Threads, &["env.fg_now", "env.memory"]);
        assert!(check(Variant::Threads, &threads).is_ok());

        let extra = module(Variant::Single, &["env.fg_now", "env.abort"]);
        let err = check(Variant::Single, &extra).unwrap_err();
        assert!(err.contains("env.abort"), "{err}");
        let missing = module(Variant::Single, &[]);
        assert!(
            check(Variant::Single, &missing)
                .unwrap_err()
                .contains("env.fg_now")
        );
        let wasi = module(
            Variant::Threads,
            &[
                "env.fg_now",
                "env.memory",
                "wasi_snapshot_preview1.fd_write",
            ],
        );
        assert!(
            check(Variant::Threads, &wasi)
                .unwrap_err()
                .contains("wasi_snapshot_preview1")
        );
        let no_clock = module(Variant::Threads, &["env.memory"]);
        assert!(check(Variant::Threads, &no_clock).is_err());
    }

    #[test]
    fn every_abi_export_is_required() {
        let mut m = module(Variant::Single, &["env.fg_now"]);
        m.exports.remove("fg_load_content");
        assert!(
            check(Variant::Single, &m)
                .unwrap_err()
                .contains("fg_load_content")
        );
        let mut m = module(Variant::Single, &["env.fg_now"]);
        m.exports.remove("memory");
        assert!(check(Variant::Single, &m).unwrap_err().contains("memory"));
    }

    #[test]
    fn rejects_truncated_section() {
        assert!(parse_target_features(&[2, b'+', 7, b'a']).is_err());
    }

    #[test]
    fn flags_follow_features_and_reject_unknown() {
        let f: BTreeSet<String> = ["simd128", "atomics"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(
            opt_flags(&f).unwrap(),
            ["--enable-threads", "--enable-simd"]
        );
        let bad: BTreeSet<String> = std::iter::once("relaxed-simd".to_string()).collect();
        assert!(opt_flags(&bad).is_err());
        let unknown: BTreeSet<String> = std::iter::once("future-thing".to_string()).collect();
        assert!(opt_flags(&unknown).unwrap_err().contains("future-thing"));
    }
}
