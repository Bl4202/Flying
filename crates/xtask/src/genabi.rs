//! `cargo xtask gen-abi [--check]`: generated files shared with TypeScript.
//!
//! * `packages/sim-bridge/src/layout.gen.ts`: every constant of `sim_core::layout::TABLE`, and the
//!   `f64` world constants of `sim_core::layout::WORLD` as the object `WORLD`.
//! * `tests/vectors/*.json`: reference data produced by the Rust implementation and consumed by
//!   Vitest, so the two languages are tested against the same numbers.
//! * `tests/vectors/present_bundle.json`: the Kestrel's presentation bundle (ADR-43), so the
//!   TypeScript parsers are tested against what the baker writes without a content build.
//! * `tests/vectors/rugged_heights.bin` and its description `rugged_heights.json`: the `rugged`
//!   arena's ground around the airfield, for the camera-clearance test (ADR-39 K1), which runs in
//!   the `test` job without WebAssembly.
//!
//! With `--check` nothing is written; any file that differs from what would be generated is an
//! error (CI runs this in the `lint` job).

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use serde_json::{Value, json};
use sim_core::frame_math::{to_render_quat, to_render_vec};
use sim_core::input::{ErraticPilot, FRAME_BYTES, InputFrame};
use sim_core::layout::{self, axis, button, input_flag, snapshot as snap};
use sim_core::math::{DQuat, DVec3};
use sim_core::rng::RngStreams;
use sim_core::seams::geo::{GeoService, ProceduralGeo};
use sim_core::{DEFAULT_SEED, SimCore};

use crate::util::{hex, workspace_root};

/// A file this command owns.
struct Generated {
    path: PathBuf,
    contents: Vec<u8>,
}

impl Generated {
    fn text(path: PathBuf, contents: String) -> Self {
        Self {
            path,
            contents: contents.into_bytes(),
        }
    }
}

pub fn run_command(args: &[String]) -> Result<(), String> {
    let check = match args.first().map(String::as_str) {
        None => false,
        Some("--check") => true,
        Some(other) => return Err(format!("unknown flag {other}; use --check or nothing")),
    };
    let files = generate();
    if check {
        let stale: Vec<String> = files
            .iter()
            .filter(|f| fs::read(&f.path).ok().as_deref() != Some(f.contents.as_slice()))
            .map(|f| f.path.display().to_string())
            .collect();
        if stale.is_empty() {
            println!("gen-abi: {} generated files are up to date", files.len());
            Ok(())
        } else {
            Err(format!(
                "generated files are stale (run `cargo xtask gen-abi` and commit):\n  {}",
                stale.join("\n  ")
            ))
        }
    } else {
        for f in &files {
            if let Some(dir) = f.path.parent() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            fs::write(&f.path, &f.contents).map_err(|e| format!("{}: {e}", f.path.display()))?;
            println!("wrote {}", f.path.display());
        }
        Ok(())
    }
}

fn generate() -> Vec<Generated> {
    let root = workspace_root();
    let (heights_bin, heights_json) = rugged_heights();
    let bundle = present_bundle();
    vec![
        Generated::text(
            root.join("packages/sim-bridge/src/layout.gen.ts"),
            layout_ts(),
        ),
        Generated::text(
            root.join("tests/vectors/frame_math.json"),
            frame_math_vectors(),
        ),
        Generated::text(
            root.join("tests/vectors/input_frames.json"),
            input_frame_vectors(),
        ),
        Generated::text(
            root.join("tests/vectors/snapshot_v5.json"),
            snapshot_vectors(),
        ),
        Generated {
            path: root.join("tests/vectors/present_bundle.json"),
            contents: bundle,
        },
        Generated {
            path: root.join("tests/vectors/rugged_heights.bin"),
            contents: heights_bin,
        },
        Generated::text(root.join("tests/vectors/rugged_heights.json"), heights_json),
    ]
}

/// The core pack's presentation bundle, exactly as `content-bake bake` writes it.
fn present_bundle() -> Vec<u8> {
    let dir = workspace_root().join("content/core");
    let baked =
        content_bake::bake::bake_dir(&dir).unwrap_or_else(|d| panic!("the core pack bakes: {d:?}"));
    content_bake::present::bake_present_dir(&dir, &baked)
        .unwrap_or_else(|d| panic!("the presentation files bake: {d:?}"))
        .expect("the core pack has presentation files")
        .json
}

/// Posts per side of the rugged height grid.
const HEIGHT_POSTS: usize = 513;
/// Their spacing, m.
const HEIGHT_SPACING_M: f64 = 62.5;
/// The grid's centre in BF (east, north): the middle of the runway.
const HEIGHT_CENTRE_BF: [f64; 2] = [0.0, -4_500.0];

/// The `rugged` arena's ground (`ground_z_bf`, so the Earth's curvature is in it) at
/// [`HEIGHT_POSTS`]^2 posts round the airfield, as little-endian `f32`, rows running north.
fn rugged_heights() -> (Vec<u8>, String) {
    let geo = ProceduralGeo::default_frame();
    let half = (HEIGHT_POSTS - 1) as f64 / 2.0;
    let mut bin = Vec::with_capacity(HEIGHT_POSTS * HEIGHT_POSTS * 4);
    for j in 0..HEIGHT_POSTS {
        let y = HEIGHT_CENTRE_BF[1] + (j as f64 - half) * HEIGHT_SPACING_M;
        for i in 0..HEIGHT_POSTS {
            let x = HEIGHT_CENTRE_BF[0] + (i as f64 - half) * HEIGHT_SPACING_M;
            bin.extend_from_slice(&(geo.ground_z_bf(x, y) as f32).to_le_bytes());
        }
    }
    let json = format!(
        "{{\n  \"note\": \"Generated by `cargo xtask gen-abi` from sim_core::seams::geo (arena rugged). Do not edit.\",\n  \"file\": \"rugged_heights.bin\",\n  \"value\": \"ground_z_bf, metres, f32 little-endian; post (i east, j north) at index j * posts + i\",\n  \"posts\": {HEIGHT_POSTS},\n  \"spacing_m\": {HEIGHT_SPACING_M:?},\n  \"centre_bf\": [{:?}, {:?}],\n  \"bytes\": {}\n}}\n",
        HEIGHT_CENTRE_BF[0],
        HEIGHT_CENTRE_BF[1],
        bin.len()
    );
    (bin, json)
}

/// `input_frame` becomes `INPUT_FRAME`.
fn const_name(group: &str) -> String {
    group.to_ascii_uppercase()
}

fn layout_ts() -> String {
    let mut out = String::from(
        "// GENERATED by `cargo xtask gen-abi` from crates/sim-core/src/layout.rs. Do not edit.\n",
    );
    for (group, entries) in layout::TABLE {
        let _ = writeln!(out, "\nexport const {} = {{", const_name(group));
        for (name, value) in *entries {
            if *name == "MAGIC" {
                let _ = writeln!(out, "  {name}: 0x{value:08x},");
            } else {
                let _ = writeln!(out, "  {name}: {value},");
            }
        }
        out.push_str("} as const;\n");
    }
    // `{:?}` prints the shortest decimal that reads back as the same double, which JavaScript
    // parses to the same value.
    out.push_str(
        "\n/** World constants as `f64` (`sim_core::layout::WORLD`). */\nexport const WORLD = {\n",
    );
    for (name, value) in layout::WORLD {
        let _ = writeln!(out, "  {name}: {value:?},");
    }
    out.push_str("} as const;\n");
    out
}

fn arr3(v: DVec3) -> Value {
    json!([v.x, v.y, v.z])
}

fn arr4(q: DQuat) -> Value {
    json!([q.w, q.x, q.y, q.z])
}

/// One entry per line, so diffs stay readable.
fn lines(entries: &[Value]) -> String {
    let body: Vec<String> = entries.iter().map(|e| format!("    {e}")).collect();
    format!("[\n{}\n  ]", body.join(",\n"))
}

fn frame_math_vectors() -> String {
    let mut rng = RngStreams::new(sim_core::DEFAULT_SEED).stream("vectors/frame_math");
    let mut vectors = vec![DVec3::X, DVec3::Y, DVec3::Z, DVec3::new(10.0, -20.0, 30.0)];
    for _ in 0..16 {
        vectors.push(DVec3::new(
            rng.range_f64(-500_000.0, 500_000.0),
            rng.range_f64(-500_000.0, 500_000.0),
            rng.range_f64(-12_000.0, 12_000.0),
        ));
    }
    let mut quats = vec![DQuat::IDENTITY];
    for _ in 0..12 {
        let axis = DVec3::new(
            rng.range_f64(-1.0, 1.0),
            rng.range_f64(-1.0, 1.0),
            rng.range_f64(-1.0, 1.0),
        )
        .normalized_or(DVec3::Z);
        quats.push(DQuat::from_axis_angle(axis, rng.range_f64(-3.1, 3.1)));
    }

    let vec_entries: Vec<Value> = vectors
        .iter()
        .map(|&v| json!({ "bf": arr3(v), "render": arr3(to_render_vec(v)) }))
        .collect();
    let quat_entries: Vec<Value> = quats
        .iter()
        .map(|&q| json!({ "q_bf": arr4(q), "q_render": arr4(to_render_quat(q)) }))
        .collect();
    let rotation_entries: Vec<Value> = quats
        .iter()
        .zip(vectors.iter().cycle())
        .map(|(&q, &v)| {
            json!({
                "q_bf": arr4(q),
                "v_bf": arr3(v),
                "rotated_render": arr3(to_render_vec(q.rotate(v))),
            })
        })
        .collect();

    format!(
        "{{\n  \"note\": \"Generated by `cargo xtask gen-abi` from sim_core::frame_math. Do not edit.\",\n  \"tolerance\": 1e-9,\n  \"vectors\": {},\n  \"quaternions\": {},\n  \"rotations\": {}\n}}\n",
        lines(&vec_entries),
        lines(&quat_entries),
        lines(&rotation_entries)
    )
}

/// A frame as JSON. `buttons` is a decimal string because a `u64` does not fit a JSON number.
fn frame_json(f: &InputFrame) -> Value {
    let mut bytes = [0_u8; FRAME_BYTES];
    f.encode(&mut bytes);
    json!({
        "seq": f.seq,
        "flags": f.flags,
        "t_ms": f.t_ms,
        "t_sent_ms": f.t_sent_ms,
        "assist_req": f.assist_req,
        "spawn_ack": f.spawn_ack,
        "spawn_req": f.spawn_req,
        "axes": f.axes.iter().map(|&a| f64::from(a)).collect::<Vec<_>>(),
        "buttons": f.buttons.to_string(),
        "hats": f.hats,
        "hex": hex(&bytes),
    })
}

fn input_frame_vectors() -> String {
    let mut frames = vec![InputFrame::neutral(0)];

    let mut typical = InputFrame::neutral(1);
    typical.t_ms = 16.666_666_666_666_668;
    typical.axes = [
        0.25, -0.5, 0.0, 0.75, 0.0, 0.0, 0.0, 0.01, -0.01, 0.0, 0.5, -0.5,
    ];
    typical.buttons = 1;
    frames.push(typical);

    let mut extremes = InputFrame::neutral(u32::MAX);
    extremes.flags = 1;
    extremes.t_ms = 1.7e12;
    extremes.axes = [
        -1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0,
    ];
    extremes.buttons = u64::MAX;
    extremes.hats = [255, 1];
    frames.push(extremes);

    let mut rounding = InputFrame::neutral(2);
    rounding.axes = [
        0.1,
        -0.2,
        0.3,
        0.4,
        0.5,
        0.6,
        0.7,
        0.8,
        0.9,
        0.123_456_79,
        -0.987_654_3,
        1.0e-7,
    ];
    rounding.buttons = 0x8000_0000_0000_0000;
    frames.push(rounding);

    // A v3 frame: a changed control, an assist request, a spawn request and both host times.
    let mut v3 = InputFrame::neutral(3);
    v3.flags = (layout::input_flag::CHANGED | layout::input_flag::MOUSE_AIM) as u32;
    v3.t_ms = 1_000.25;
    v3.t_sent_ms = 1_003.75;
    v3.axes[layout::axis::BRAKE_L] = 0.5;
    v3.axes[layout::axis::CURSOR_X] = -0.25;
    v3.buttons = (1 << layout::button::GEAR) | (1 << layout::button::FLAPS_CYCLE);
    v3.assist_req = 4;
    v3.spawn_ack = 200;
    v3.spawn_req = 2;
    frames.push(v3);

    // Frames from the scripted pilot, which is what the determinism run feeds the core.
    let mut pilot = ErraticPilot::new(RngStreams::new(DEFAULT_SEED).stream("input/erratic"));
    for tick in 0..1_800_u64 {
        let f = pilot.next_frame(tick);
        if matches!(tick, 0 | 1 | 599 | 1_799) {
            frames.push(f);
        }
    }

    let entries: Vec<Value> = frames.iter().map(frame_json).collect();
    format!(
        "{{\n  \"note\": \"Generated by `cargo xtask gen-abi` from sim_core::input. Do not edit.\",\n  \"record_bytes\": {FRAME_BYTES},\n  \"frames\": {}\n}}\n",
        lines(&entries)
    )
}

fn le_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn le_u64(b: &[u8], at: usize) -> u64 {
    let mut a = [0_u8; 8];
    a.copy_from_slice(&b[at..at + 8]);
    u64::from_le_bytes(a)
}

fn le_f64(b: &[u8], at: usize) -> f64 {
    f64::from_bits(le_u64(b, at))
}

fn le_f32(b: &[u8], at: usize) -> f64 {
    f64::from(f32::from_bits(le_u32(b, at)))
}

/// The snapshot's ownship block, decoded straight from the layout constants.
fn ownship_json(b: &[u8]) -> Value {
    // Built in two parts: `json!` recurses once per token and the whole object is too large.
    let mut ownship = json!({
        "pos_bf": [le_f64(b, snap::O_POS_BF), le_f64(b, snap::O_POS_BF + 8), le_f64(b, snap::O_POS_BF + 16)],
        "quat": (0..4).map(|k| le_f32(b, snap::O_QUAT + 4 * k)).collect::<Vec<_>>(),
        "vel_bf": (0..3).map(|k| le_f32(b, snap::O_VEL_BF + 4 * k)).collect::<Vec<_>>(),
        "tas": le_f32(b, snap::O_TAS),
        "cas": le_f32(b, snap::O_CAS),
        "mach": le_f32(b, snap::O_MACH),
        "alpha": le_f32(b, snap::O_ALPHA),
        "beta": le_f32(b, snap::O_BETA),
        "nz": le_f32(b, snap::O_NZ),
        "throttle": le_f32(b, snap::O_THROTTLE),
        "thrust": le_f32(b, snap::O_THRUST),
        "fuel": le_f32(b, snap::O_FUEL),
        "mass": le_f32(b, snap::O_MASS),
        "omega": (0..3).map(|k| le_f32(b, snap::O_OMEGA + 4 * k)).collect::<Vec<_>>(),
        "surfaces": (0..3).map(|k| le_f32(b, snap::O_SURFACES + 4 * k)).collect::<Vec<_>>(),
        "altitude": le_f32(b, snap::O_ALTITUDE),
        "ab_stage": b[snap::O_AB_STAGE],
        "flags": b[snap::O_FLAGS]
    });
    let ownship_v4 = json!({
        "arena": b[snap::O_ARENA],
        "engine_bits": b[snap::O_ENGINE_BITS],
        "qbar": le_f32(b, snap::O_QBAR),
        "fuel_flow": le_f32(b, snap::O_FUEL_FLOW),
        "engine_core": le_f32(b, snap::O_ENGINE_CORE),
        "ab_level": le_f32(b, snap::O_AB_LEVEL),
        "max_nz": le_f32(b, snap::O_MAX_NZ),
        "n_max": le_f32(b, snap::O_N_MAX),
        "alpha_warn": le_f32(b, snap::O_ALPHA_WARN),
    });
    let ownship_v3 = json!({
            "assist": b[snap::O_ASSIST],
            "fcs_mode": b[snap::O_FCS_MODE],
            "flags2": u16::from_le_bytes([b[snap::O_FLAGS2], b[snap::O_FLAGS2 + 1]]),
            "spawn_seq": b[snap::O_SPAWN_SEQ],
            "flap_detent": b[snap::O_FLAP_DETENT],
            "gear_pos": le_f32(b, snap::O_GEAR_POS),
            "tef": le_f32(b, snap::O_TEF),
            "airbrake": le_f32(b, snap::O_AIRBRAKE),
            "brake": [le_f32(b, snap::O_BRAKE_L), le_f32(b, snap::O_BRAKE_R)],
            "steer": le_f32(b, snap::O_STEER),
            "ground_speed": le_f32(b, snap::O_GROUND_SPEED),
            "vs": le_f32(b, snap::O_VS),
            "agl": le_f32(b, snap::O_AGL),
            "g_lagged": le_f32(b, snap::O_G_LAGGED),
            "g_stress": le_f32(b, snap::O_G_STRESS),
            "redout": le_f32(b, snap::O_REDOUT),
            "n_cmd": le_f32(b, snap::O_N_CMD),
            "touchdown_vs": le_f32(b, snap::O_TOUCHDOWN_VS),
            "aim": [le_f32(b, snap::O_AIM_AZ), le_f32(b, snap::O_AIM_EL)],
            "crash_cause": b[snap::O_CRASH_CAUSE],
            "gear_bits": b[snap::O_GEAR_BITS],
    });
    let fields = ownship.as_object_mut().expect("an object");
    fields.extend(ownship_v3.as_object().expect("an object").clone());
    fields.extend(ownship_v4.as_object().expect("an object").clone());
    ownship
}

fn snapshot_vectors() -> String {
    const TICKS: u32 = 300;
    let pack = content_bake::bake::bake_dir(&workspace_root().join("content/core"))
        .unwrap_or_else(|d| panic!("the core pack bakes: {d:?}"))
        .cpack;
    let mut core = SimCore::new(DEFAULT_SEED);
    core.load_content(&pack).expect("the core pack loads");
    // The plain arena, so the reader sees a non-zero arena byte; otherwise the default session.
    core.configure(sim_core::SessionConfig {
        arena: sim_core::Arena::Plain,
        ..sim_core::SessionConfig::default()
    })
    .expect("idle");
    core.start();
    // The default session: lined up on the runway under Standard assist. The pilot rolls with the
    // parking brake off, the toe brakes unequal, the pedals over, the flaps at landing and the
    // mouse pointing, so that the fields the ground and the pilot write are not zero and a reader
    // that looks in the wrong place cannot pass.
    for tick in 0..TICKS {
        let mut f = InputFrame::neutral(tick + 1);
        f.t_ms = f64::from(tick) * (1_000.0 / 60.0);
        f.spawn_ack = 1;
        f.flags = (input_flag::MOUSE_AIM | input_flag::CHANGED) as u32;
        f.axes[axis::THROTTLE] = 0.9;
        f.axes[axis::YAW] = 0.4;
        f.axes[axis::BRAKE_L] = 0.3;
        f.axes[axis::BRAKE_R] = 0.6;
        f.axes[axis::CURSOR_X] = 0.25;
        f.axes[axis::CURSOR_Y] = -0.1;
        if tick == 5 {
            f.buttons = (1 << button::PARKING_BRAKE) | (1 << button::FLAPS_DOWN);
        }
        let mut wire = [0_u8; FRAME_BYTES];
        f.encode(&mut wire);
        core.push_inputs(&wire);
        core.step(1);
    }
    let b = core.snapshot_bytes();

    // Decode independently of the writer, straight from the layout constants.
    let body = |i: usize| {
        json!({
            "pos_bf": [le_f64(b, snap::B_X + 8 * i), le_f64(b, snap::B_Y + 8 * i), le_f64(b, snap::B_Z + 8 * i)],
            "quat": (0..4).map(|k| le_f32(b, snap::B_QUAT + 16 * i + 4 * k)).collect::<Vec<_>>(),
        })
    };
    let body_count = le_u32(b, snap::H_BODY_COUNT) as usize;
    let bodies: Vec<Value> = [0, 1, 2, body_count - 1].into_iter().map(body).collect();
    let ownship = ownship_json(b);
    let decoded = json!({
        "magic": le_u32(b, snap::H_MAGIC),
        "version": le_u32(b, snap::H_VERSION),
        "tick": le_u64(b, snap::H_TICK),
        "publish_time_ms": le_f64(b, snap::H_PUBLISH_TIME_MS),
        "last_input_seq": le_u32(b, snap::H_LAST_INPUT_SEQ),
        "body_count": body_count,
        "core_state": le_u32(b, snap::H_CORE_STATE),
        "input_t_ms": le_f64(b, snap::H_INPUT_T_MS),
        "ownship": ownship,
        "bodies": bodies,
    });
    let (duel_ticks, duel_decoded, duel_hex) = duel_snapshot(&pack);
    format!(
        "{{\n  \"note\": \"Generated by `cargo xtask gen-abi` from sim_core::snapshot. Do not edit.\",\n  \"seed\": \"{DEFAULT_SEED:016x}\",\n  \"script\": \"runway roll in the plain arena: toe brakes, pedals, mouse-aim\",\n  \"content\": \"core\",\n  \"ticks\": {TICKS},\n  \"size\": {},\n  \"decoded\": {decoded},\n  \"hex\": \"{}\",\n  \"duel\": {{\n    \"script\": \"a duel: 2 km behind a bandit flying straight, the gun held, a missile at the lock\",\n    \"ticks\": {duel_ticks},\n    \"decoded\": {duel_decoded},\n    \"hex\": \"{duel_hex}\"\n  }}\n}}\n",
        b.len(),
        hex(b)
    )
}

/// A duel snapshot with a live bandit, a missile in flight, tracers and events: the player 2 km
/// behind (and a little below) a bandit flying straight, the gun held from the arming and the
/// missile button pressed once the seeker locks.
fn duel_snapshot(pack: &[u8]) -> (u32, Value, String) {
    use sim_core::combat::{AirStart, BrainKind, CombatSetup};
    const TICKS: u32 = 150;
    let mut core = SimCore::new(DEFAULT_SEED);
    core.load_content(pack).expect("the core pack loads");
    core.configure(sim_core::SessionConfig {
        spawn: sim_core::Spawn::Air,
        scenario: sim_core::Scenario::Duel,
        skill: 2,
        ..sim_core::SessionConfig::default()
    })
    .expect("idle");
    let at = |north_m: f64, alt_msl_m: f64| AirStart {
        east_m: 0.0,
        north_m,
        alt_msl_m,
        heading_rad: 0.0,
        tas_mps: 230.0,
    };
    assert!(core.combat_setup(CombatSetup {
        player: Some(at(0.0, 4_000.0)),
        bandit: Some(at(2_000.0, 4_100.0)),
        brain: BrainKind::HoldCourse,
        bandit_active: true,
        gci: true,
        unarmed: false,
    }));
    core.start();
    let mut pressed = false;
    for tick in 0..TICKS {
        let mut f = InputFrame::neutral(tick + 1);
        f.t_ms = f64::from(tick) * (1_000.0 / 60.0);
        f.spawn_ack = core.ownship().aircraft().map_or(0, |a| a.spawn_seq);
        f.axes[axis::THROTTLE] = 0.8;
        let c = core.combat().expect("a duel");
        let fire = !pressed
            && c.player.armed
            && usize::from(c.player.seeker.mode.code()) == layout::seeker_mode::LOCKED;
        pressed |= fire;
        f.buttons = (1 << button::FIRE_GUN) | (u64::from(fire) << button::FIRE_MISSILE);
        let mut wire = [0_u8; FRAME_BYTES];
        f.encode(&mut wire);
        core.push_inputs(&wire);
        core.step(1);
    }
    let b = core.snapshot_bytes();
    (TICKS, combat_json(b), hex(b))
}

/// The combat block, decoded straight from the layout constants.
fn combat_json(b: &[u8]) -> Value {
    let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let v64 = |o: usize| (0..3).map(|k| le_f64(b, o + 8 * k)).collect::<Vec<_>>();
    let v32 = |o: usize| (0..3).map(|k| le_f32(b, o + 4 * k)).collect::<Vec<_>>();
    let missiles: Vec<Value> = (0..snap::MISSILE_CAPACITY)
        .map(|i| {
            let at = snap::MISSILES + i * snap::MISSILE_STRIDE;
            json!({
                "pos_bf": v64(at + snap::MS_POS_BF),
                "vel_bf": v32(at + snap::MS_VEL_BF),
                "state": b[at + snap::MS_STATE],
                "owner": b[at + snap::MS_OWNER],
            })
        })
        .collect();
    let tracer_count = le_u32(b, snap::TR_COUNT) as usize;
    let tracers: Vec<Value> = (0..tracer_count.min(3))
        .map(|i| {
            let at = snap::TR_FIRST + i * snap::TRACER_STRIDE;
            json!({
                "pos_bf": v64(at + snap::TR_POS_BF),
                "vel_bf": v32(at + snap::TR_VEL_BF),
                "owner": b[at + snap::TR_OWNER],
                "id": b[at + snap::TR_ID],
            })
        })
        .collect();
    let explosions: Vec<Value> = (0..snap::EXPLOSION_CAPACITY)
        .map(|i| {
            let at = snap::EXPLOSIONS + i * snap::EXPLOSION_STRIDE;
            json!({
                "pos_bf": v64(at + snap::XP_POS_BF),
                "age": le_f32(b, at + snap::XP_AGE),
                "kind": b[at + snap::XP_KIND],
            })
        })
        .collect();
    let events: Vec<Value> = (0..snap::EVENT_CAPACITY)
        .map(|i| snap::EVENTS + i * snap::EVENT_STRIDE)
        .filter(|&at| le_u32(b, at + snap::EV_SEQ) != 0)
        .map(|at| {
            json!({
                "seq": le_u32(b, at + snap::EV_SEQ),
                "tick": le_u32(b, at + snap::EV_TICK),
                "kind": b[at + snap::EV_KIND],
                "actor": b[at + snap::EV_ACTOR],
                "target": b[at + snap::EV_TARGET],
                "arg": b[at + snap::EV_ARG],
                "value": le_f32(b, at + snap::EV_VALUE),
            })
        })
        .collect();
    json!({
        "flags": u16_at(snap::C_FLAGS),
        "seeker": b[snap::C_SEEKER],
        "inhibit": b[snap::C_INHIBIT],
        "rounds": u16_at(snap::C_ROUNDS),
        "missiles": b[snap::C_MISSILES],
        "bandits": b[snap::C_BANDITS],
        "event_seq": le_u32(b, snap::C_EVENT_SEQ),
        "pipper_bf": v64(snap::C_PIPPER_BF),
        "signal": le_f32(b, snap::C_SIGNAL),
        "tgt_range": le_f32(b, snap::C_TGT_RANGE),
        "tgt_closure": le_f32(b, snap::C_TGT_CLOSURE),
        "tgt_aspect": le_f32(b, snap::C_TGT_ASPECT),
        "health": le_f32(b, snap::C_HEALTH),
        "inhibit_age": le_f32(b, snap::C_INHIBIT_AGE),
        "counters": (0..12).map(|k| le_u32(b, snap::C_ROUNDS_FIRED + 4 * k)).collect::<Vec<_>>(),
        "bandit": {
            "pos_bf": v64(snap::BD_POS_BF),
            "quat": (0..4).map(|k| le_f32(b, snap::BD_QUAT + 4 * k)).collect::<Vec<_>>(),
            "vel_bf": v32(snap::BD_VEL_BF),
            "surfaces": v32(snap::BD_SURFACES),
            "tef": le_f32(b, snap::BD_TEF),
            "airbrake": le_f32(b, snap::BD_AIRBRAKE),
            "gear_pos": le_f32(b, snap::BD_GEAR_POS),
            "ab_level": le_f32(b, snap::BD_AB_LEVEL),
            "health": le_f32(b, snap::BD_HEALTH),
            "tas": le_f32(b, snap::BD_TAS),
            "life": b[snap::BD_LIFE],
            "ai": b[snap::BD_AI],
            "skill": b[snap::BD_SKILL],
            "flags": b[snap::BD_FLAGS],
            "spawn_seq": u16_at(snap::BD_SPAWN_SEQ),
        },
        "missile_slots": missiles,
        "tracer_count": tracer_count,
        "tracers": tracers,
        "explosions": explosions,
        "events": events,
    })
}
