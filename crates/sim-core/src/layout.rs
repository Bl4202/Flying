//! Byte layouts shared between Rust and TypeScript (GDD 61.2, 61.4.4, 58.2).
//!
//! Every offset is in bytes, every multi-byte value is little-endian. This file is the only place
//! the numbers are written down: `cargo xtask gen-abi` turns [`TABLE`] into
//! `packages/sim-bridge/src/layout.gen.ts`, and CI fails if the generated file is stale.
//!
//! Wall-clock fields (`T_MS`, `H_PUBLISH_TIME_MS`, the profile block) are observations about the
//! host and never feed simulation state or hashes.

macro_rules! layouts {
    ($( $(#[$doc:meta])* $group:ident { $( $name:ident = $val:expr ),* $(,)? } )*) => {
        $(
            $(#[$doc])*
            pub mod $group {
                $(
                    #[allow(missing_docs)]
                    pub const $name: usize = $val;
                )*
            }
        )*

        /// Every constant, grouped by module, for code generation.
        pub const TABLE: &[(&str, &[(&str, usize)])] = &[
            $( (stringify!($group), &[ $( (stringify!($name), $group::$name) ),* ]) ),*
        ];
    };
}

layouts! {
    /// Interface version of the exported C ABI and of these layouts.
    abi {
        VERSION = 3,
        // Frames the host can hand over per `fg_push_inputs` call.
        INPUT_BATCH = 8,
        SNAPSHOT_LAYOUT_VERSION = 3,
    }

    /// `InputFrame`, 96 bytes (GDD 58.2). `T_MS` is the host time of the source event (the key
    /// press, the pointer move, the gamepad sample) on the shared clock; `T_SENT_MS` is when the
    /// frame was written. Both are observations and never feed state or hashes (ADR-33).
    /// `ASSIST_REQ` is an absolute assist level (`assist_level`; 0 keeps the current one),
    /// `SPAWN_ACK` echoes the snapshot's spawn counter once the host has synchronised its
    /// throttle lever, and `SPAWN_REQ` (`spawn_request`) is read on the RESET press.
    input_frame {
        SIZE = 96,
        SEQ = 0,
        FLAGS = 4,
        T_MS = 8,
        AXES = 16,
        AXIS_COUNT = 12,
        BUTTONS = 64,
        HATS = 72,
        HAT_COUNT = 2,
        ASSIST_REQ = 74,
        SPAWN_ACK = 75,
        SPAWN_REQ = 76,
        T_SENT_MS = 80,
    }

    /// Indices into `InputFrame.axes`.
    axis {
        PITCH = 0,
        ROLL = 1,
        YAW = 2,
        THROTTLE = 3,
        THROTTLE2 = 4,
        BRAKE_L = 5,
        BRAKE_R = 6,
        TRIM_P = 7,
        TRIM_R = 8,
        TRIM_Y = 9,
        CURSOR_X = 10,
        CURSOR_Y = 11,
    }

    /// Bit indices into `InputFrame.buttons`. `RESET`, `GEAR`, the flap buttons, `PARKING_BRAKE`
    /// and `TRIM_RESET` act on the press; the trim, `AIRBRAKE` and `RECOVER` buttons act while
    /// held (the host turns a toggle binding into a held state).
    button {
        RESET = 0,
        TRIM_NOSE_UP = 1,
        TRIM_NOSE_DOWN = 2,
        // Toggles the gear lever.
        GEAR = 3,
        FLAPS_DOWN = 4,
        FLAPS_UP = 5,
        // Held = open.
        AIRBRAKE = 6,
        // Toggles the parking brake.
        PARKING_BRAKE = 7,
        // Unusual-attitude recovery (GDD 11.11.3).
        RECOVER = 8,
        TRIM_RESET = 9,
        // Up, take-off, landing, up.
        FLAPS_CYCLE = 10,
    }

    /// Bits of `InputFrame.flags`.
    input_flag {
        // The host neutralised the controls (focus lost, tab hidden).
        NEUTRAL = 1,
        // A control actually changed since the last frame (latency is measured on these only).
        CHANGED = 2,
        // The cursor axes carry a mouse-aim direction (azimuth, elevation).
        MOUSE_AIM = 4,
        // Opt-in automatic ground-collision avoidance (GDD 54.9).
        AUTO_GCAS = 8,
    }

    /// `spawn` argument of `fg_configure`.
    spawn_kind {
        // Lined up on the runway, engine idling, brakes on (GDD 48.2 "runway hot").
        RUNWAY = 0,
        // Airborne, trimmed.
        AIR = 1,
    }

    /// `InputFrame.spawn_req`, read on the RESET press.
    spawn_request {
        // The session's spawn.
        SESSION = 0,
        RUNWAY = 1,
        AIR = 2,
    }

    /// Assist level (GDD 11.11.3): `fg_configure`, `InputFrame.assist_req` and the snapshot.
    assist_level {
        FULL = 1,
        STANDARD = 2,
        REDUCED = 3,
        MANUAL = 4,
    }

    /// `g_effects` argument of `fg_configure` (GDD 11.14, 59.2).
    g_effects {
        OFF = 0,
        REDUCED = 1,
        FULL = 2,
    }

    /// Return value of `fg_configure`.
    config_status {
        OK = 0,
        // The core has started; the session must be configured first.
        NOT_IDLE = 1,
        // An argument is out of range.
        INVALID = 2,
        // `fg_init` has not been called.
        NO_CORE = 3,
    }

    /// Snapshot `O_FCS_MODE`: what the flight control system is doing.
    fcs_mode {
        FLIGHT = 0,
        GROUND = 1,
        // Blending between the ground and flight laws, or between two assist levels.
        BLEND = 2,
        RECOVER = 3,
        // Automatic ground-collision avoidance has control.
        GCAS = 4,
        // The pilot is incapacitated (G-LOC).
        GLOC = 5,
    }

    /// Bits of the snapshot's `O_FLAGS2` (a `u16`).
    ownship_flag2 {
        // Weight on wheels.
        WOW = 1,
        PARKING_BRAKE = 2,
        ALPHA_LIMIT = 4,
        G_LIMIT = 8,
        OVERSPEED = 16,
        OVERSTRESS = 32,
        BLACKOUT = 64,
        GLOC = 128,
        REDOUT = 256,
        // Latched for three seconds after a hard landing.
        HARD_LANDING = 512,
        GEAR_OVERSPEED = 1024,
        FLAP_OVERSPEED = 2048,
        DEPARTED = 4096,
        // Low, slow, throttle back and the gear is up.
        GEAR_WARNING = 8192,
        MOUSE_AIM = 16384,
        // The throttle is held at the spawn setting until the host acknowledges.
        THROTTLE_HELD = 32768,
    }

    /// Why the aircraft was destroyed (snapshot `O_CRASH_CAUSE`).
    crash_cause {
        NONE = 0,
        // Airframe contact at speed or at a steep angle (GDD 11.13.3).
        AIRFRAME_IMPACT = 1,
        // Sink rate above 7 m/s at gear contact.
        SINK_RATE = 2,
        // A gear strut bottomed out and collapsed past its stop.
        GEAR_COLLAPSE_STOP = 3,
        // The centre of gravity went through the ground.
        TUNNELLING = 4,
        NUMERICAL = 5,
    }

    /// The test airfield, so the renderer can mark the runway where the sim put it. Distances in
    /// metres, the threshold lies `THRESHOLD_SOUTH_M` south of the battlefield anchor, the
    /// runway heads north (heading 000) and its centreline is east = 0.
    airfield {
        LENGTH_M = 3000,
        WIDTH_M = 45,
        THRESHOLD_SOUTH_M = 6000,
        ELEVATION_M = 40,
    }

    /// Snapshot v3: header, the ownship block (the player's aircraft, or the spectator),
    /// bodies in structure-of-arrays form. `O_POS_BF` is the aircraft's reference point, not its
    /// centre of gravity, so the camera does not move as fuel burns. `O_SURFACES` is elevator,
    /// aileron, rudder in radians; `O_FLAGS` holds `ownship_flag` bits and `O_FLAGS2` the
    /// `ownship_flag2` bits. `O_ASSIST` is an `assist_level` (0 for the spectator), `O_FCS_MODE`
    /// an `fcs_mode`. `O_SPAWN_SEQ` counts spawns (wrapping) for the throttle hand-over.
    /// `O_GEAR_BITS` is, per leg `i`, bit `2i` weight-on-wheels and bit `2i+1` collapsed, bit 6
    /// gear lever down, bit 7 all gear down and locked. `O_AIM_*` echo the mouse-aim direction
    /// the core is using.
    snapshot {
        MAGIC = 0x4E53_4746,
        VERSION = 3,
        HEADER_SIZE = 64,
        H_MAGIC = 0,
        H_VERSION = 4,
        H_TICK = 8,
        H_PUBLISH_TIME_MS = 16,
        H_LAST_INPUT_SEQ = 24,
        H_BODY_COUNT = 28,
        H_CORE_STATE = 32,
        H_INPUT_T_MS = 40,
        OWNSHIP = 64,
        OWNSHIP_SIZE = 224,
        O_POS_BF = 64,
        O_QUAT = 88,
        O_VEL_BF = 104,
        O_TAS = 116,
        O_CAS = 120,
        O_MACH = 124,
        O_ALPHA = 128,
        O_BETA = 132,
        O_NZ = 136,
        O_THROTTLE = 140,
        O_THRUST = 144,
        O_FUEL = 148,
        O_MASS = 152,
        O_OMEGA = 156,
        O_SURFACES = 168,
        O_ALTITUDE = 180,
        O_AB_STAGE = 184,
        O_FLAGS = 185,
        O_ASSIST = 186,
        O_FCS_MODE = 187,
        O_FLAGS2 = 188,
        O_SPAWN_SEQ = 190,
        O_FLAP_DETENT = 191,
        O_GEAR_POS = 192,
        O_TEF = 196,
        O_AIRBRAKE = 200,
        O_BRAKE_L = 204,
        O_BRAKE_R = 208,
        O_STEER = 212,
        O_GROUND_SPEED = 216,
        O_VS = 220,
        O_AGL = 224,
        O_G_LAGGED = 228,
        O_G_STRESS = 232,
        O_REDOUT = 236,
        O_N_CMD = 240,
        O_TOUCHDOWN_VS = 244,
        O_AIM_AZ = 248,
        O_AIM_EL = 252,
        O_CRASH_CAUSE = 256,
        O_GEAR_BITS = 257,
        BODY_CAPACITY = 256,
        B_X = 288,
        B_Y = 2336,
        B_Z = 4384,
        B_QUAT = 6432,
        SIZE = 10528,
    }

    /// Bits of the snapshot's `O_FLAGS`.
    ownship_flag {
        // No aircraft content: the ownship is the kinematic spectator.
        SPECTATOR = 1,
        // Hit the ground; respawns shortly.
        CRASHED = 2,
        // An engine has flamed out.
        FLAMEOUT = 4,
        // Angle of attack within 3 degrees of the lift peak.
        STALL_WARN = 8,
        // The integrator gave up twice in a tick; respawns shortly.
        NUMERICAL_FAULT = 16,
    }

    /// Shared snapshot buffer: three slots under a seqlock. `H_PUBLISHED` counts publications
    /// (0 = none yet); publication `n` lives in slot `(n - 1) % SLOT_COUNT`, so that one word says
    /// both which slot is newest and how new it is. `H_SLOT_SEQ` holds one sequence counter per
    /// slot: odd while the writer is inside it, bumped to the next even value when done.
    snapshot_sab {
        MAGIC = 0x4253_4746,
        VERSION = 1,
        HEADER_SIZE = 64,
        SLOT_COUNT = 3,
        SLOT_BYTES = 10528,
        SIZE = 31648,
        H_MAGIC = 0,
        H_VERSION = 4,
        H_SLOT_BYTES = 8,
        H_SLOT_COUNT = 12,
        H_PUBLISHED = 16,
        H_SLOT_SEQ = 20,
    }

    /// Shared single-producer single-consumer ring of input frames.
    input_ring {
        MAGIC = 0x5249_4746,
        HEADER_SIZE = 32,
        RECORD_BYTES = 96,
        CAPACITY = 64,
        SIZE = 6176,
        H_RECORD_BYTES = 0,
        H_CAPACITY = 4,
        H_WRITE = 8,
        H_READ = 12,
        H_DROPPED = 16,
        H_MAGIC = 20,
    }

    /// Profile block written by the core when profiling is on (`f32` milliseconds).
    profile {
        SIZE = 160,
        TICKS = 0,
        TICK_MS_LAST = 8,
        TICK_MS_EWMA = 12,
        STAGE_COUNT = 11,
        STAGE_MS_LAST = 16,
        STAGE_MS_EWMA = 60,
        COUNTERS = 104,
        COUNTER_COUNT = 13,
    }

    /// Tick stages in execution order (GDD 61.4.3).
    stage {
        INGEST = 0,
        ENVIRONMENT = 1,
        RELEVANCE = 2,
        AI = 3,
        DYNAMICS = 4,
        WEAPONS = 5,
        SENSING = 6,
        COLLISION = 7,
        SYSTEMS = 8,
        DIRECTOR = 9,
        PUBLISH = 10,
    }

    /// Indices of the `u32` counters in the profile block.
    counter {
        INPUT_APPLIED = 0,
        INPUT_NONFINITE = 1,
        INPUT_STALE = 2,
        INPUT_OVERFLOW = 3,
        // Integrator faults (each a rolled-back tick; GDD 11.10).
        FDM_FAULTS = 4,
        CRASHES = 5,
        RESPAWNS = 6,
        SNAPSHOTS = 7,
        // Spawns where the trim solver failed and a fallback attitude was used.
        TRIM_FAILURES = 8,
        // First wheel contacts after being airborne.
        TOUCHDOWNS = 9,
        // Touchdowns with a sink rate of 3.0 m/s or more (GDD 11.13.3).
        HARD_LANDINGS = 10,
        // Times the pilot was incapacitated by sustained g.
        GLOC_EVENTS = 11,
        // Changes of the assist level.
        ASSIST_CHANGES = 12,
    }

    /// Terrain patch handed to the renderer once.
    terrain {
        HEADER_SIZE = 32,
        H_ORIGIN_BF = 0,
        H_POSTS = 24,
        H_SPACING_M = 28,
        POSTS = 129,
        VERTEX_BYTES = 28,
        V_POS = 0,
        V_NORMAL = 12,
        V_HEIGHT = 24,
        VERTEX_COUNT = 16641,
        SIZE = 465_980,
    }

    /// `CoreState` as exported by `fg_core_state`.
    core_state {
        IDLE = 0,
        LOADING = 1,
        RUNNING = 2,
        PAUSED = 3,
        ENDED = 4,
        // `fg_core_state` before `fg_init`.
        NONE = 255,
    }

    /// Return value of `fg_load_content`.
    content_status {
        OK = 0,
        // The core has started; content must be loaded first.
        NOT_IDLE = 1,
        // The bytes are not a valid pack (damaged, truncated, wrong version).
        INVALID = 2,
        // `fg_init` has not been called.
        NO_CORE = 3,
        // `fg_content_reserve` was not called, or the length exceeds what it reserved.
        NO_BUFFER = 4,
    }

    /// `kind` argument of `fg_subsystem_hash_lo/hi`.
    hash_kind {
        COMBINED = 0,
        TICK = 1,
        RNG = 2,
        BODIES = 3,
        OWNSHIP = 4,
        INPUT = 5,
        CONTENT = 6,
    }

    /// `kind` argument of `fg_set_input_script`.
    input_script {
        NONE = 0,
        ERRATIC = 1,
        // The 60 s scripted flight of AC-11.7.
        FLIGHT60 = 2,
        // The virtual tester: take-off, circuit and landing (ADR-33). Not available before the
        // tester exists; `fg_set_input_script` refuses it.
        CIRCUIT = 3,
    }
}

// Structural invariants, checked when the crate compiles: a layout that does not add up cannot
// be built, let alone shipped to the TypeScript side.
const _: () = {
    use {input_frame as f, input_ring as r, profile as p, snapshot as s, snapshot_sab as b};

    // InputFrame: seq, flags, t_ms, 12 f32 axes, u64 buttons, 2 hat bytes, in 96 bytes.
    assert!(f::AXES == f::T_MS + 8);
    assert!(f::AXES + f::AXIS_COUNT * 4 == f::BUTTONS);
    assert!(f::BUTTONS + 8 == f::HATS);
    assert!(f::HATS + f::HAT_COUNT == f::ASSIST_REQ);
    assert!(f::ASSIST_REQ + 1 == f::SPAWN_ACK);
    assert!(f::SPAWN_ACK + 1 == f::SPAWN_REQ);
    assert!(f::SPAWN_REQ < f::T_SENT_MS && f::T_SENT_MS.is_multiple_of(8));
    assert!(f::T_SENT_MS + 8 <= f::SIZE);

    // Snapshot: header, ownship, then four structure-of-arrays blocks that end exactly at SIZE.
    assert!(s::H_INPUT_T_MS + 8 <= s::HEADER_SIZE);
    assert!(s::OWNSHIP == s::HEADER_SIZE);
    assert!(s::O_POS_BF == s::OWNSHIP);
    assert!(s::O_POS_BF + 24 == s::O_QUAT);
    assert!(s::O_QUAT + 16 == s::O_VEL_BF);
    assert!(s::O_VEL_BF + 12 == s::O_TAS);
    assert!(s::O_TAS + 4 == s::O_CAS);
    assert!(s::O_CAS + 4 == s::O_MACH);
    assert!(s::O_MACH + 4 == s::O_ALPHA);
    assert!(s::O_ALPHA + 4 == s::O_BETA);
    assert!(s::O_BETA + 4 == s::O_NZ);
    assert!(s::O_NZ + 4 == s::O_THROTTLE);
    assert!(s::O_THROTTLE + 4 == s::O_THRUST);
    assert!(s::O_THRUST + 4 == s::O_FUEL);
    assert!(s::O_FUEL + 4 == s::O_MASS);
    assert!(s::O_MASS + 4 == s::O_OMEGA);
    assert!(s::O_OMEGA + 12 == s::O_SURFACES);
    assert!(s::O_SURFACES + 12 == s::O_ALTITUDE);
    assert!(s::O_ALTITUDE + 4 == s::O_AB_STAGE);
    assert!(s::O_AB_STAGE + 1 == s::O_FLAGS);
    // The v3 additions, one after the other: four bytes, then 4-byte floats, then the bytes.
    assert!(s::O_FLAGS + 1 == s::O_ASSIST);
    assert!(s::O_ASSIST + 1 == s::O_FCS_MODE);
    assert!(s::O_FCS_MODE + 1 == s::O_FLAGS2);
    assert!(s::O_FLAGS2 + 2 == s::O_SPAWN_SEQ);
    assert!(s::O_SPAWN_SEQ + 1 == s::O_FLAP_DETENT);
    assert!(s::O_FLAP_DETENT + 1 == s::O_GEAR_POS);
    assert!(s::O_GEAR_POS + 4 == s::O_TEF);
    assert!(s::O_TEF + 4 == s::O_AIRBRAKE);
    assert!(s::O_AIRBRAKE + 4 == s::O_BRAKE_L);
    assert!(s::O_BRAKE_L + 4 == s::O_BRAKE_R);
    assert!(s::O_BRAKE_R + 4 == s::O_STEER);
    assert!(s::O_STEER + 4 == s::O_GROUND_SPEED);
    assert!(s::O_GROUND_SPEED + 4 == s::O_VS);
    assert!(s::O_VS + 4 == s::O_AGL);
    assert!(s::O_AGL + 4 == s::O_G_LAGGED);
    assert!(s::O_G_LAGGED + 4 == s::O_G_STRESS);
    assert!(s::O_G_STRESS + 4 == s::O_REDOUT);
    assert!(s::O_REDOUT + 4 == s::O_N_CMD);
    assert!(s::O_N_CMD + 4 == s::O_TOUCHDOWN_VS);
    assert!(s::O_TOUCHDOWN_VS + 4 == s::O_AIM_AZ);
    assert!(s::O_AIM_AZ + 4 == s::O_AIM_EL);
    assert!(s::O_AIM_EL + 4 == s::O_CRASH_CAUSE);
    assert!(s::O_CRASH_CAUSE + 1 == s::O_GEAR_BITS);
    assert!(s::O_FLAGS2.is_multiple_of(2) && s::O_GEAR_POS.is_multiple_of(4));
    assert!(s::O_GEAR_BITS < s::OWNSHIP + s::OWNSHIP_SIZE);
    assert!(s::B_X == s::OWNSHIP + s::OWNSHIP_SIZE);
    assert!(s::B_Y == s::B_X + s::BODY_CAPACITY * 8);
    assert!(s::B_Z == s::B_Y + s::BODY_CAPACITY * 8);
    assert!(s::B_QUAT == s::B_Z + s::BODY_CAPACITY * 8);
    assert!(s::SIZE == s::B_QUAT + s::BODY_CAPACITY * 16);

    // Shared buffers hold exactly their records.
    assert!(b::SLOT_BYTES == s::SIZE);
    assert!(b::SIZE == b::HEADER_SIZE + b::SLOT_COUNT * b::SLOT_BYTES);
    assert!(b::H_SLOT_SEQ + b::SLOT_COUNT * 4 <= b::HEADER_SIZE);
    assert!(r::RECORD_BYTES == f::SIZE);
    assert!(r::SIZE == r::HEADER_SIZE + r::CAPACITY * r::RECORD_BYTES);
    assert!(r::CAPACITY.is_power_of_two());
    assert!(r::H_MAGIC + 4 <= r::HEADER_SIZE);

    // Profile block: stage timings, then their averages, then the counters.
    assert!(p::TICK_MS_EWMA + 4 == p::STAGE_MS_LAST);
    assert!(p::STAGE_MS_LAST + p::STAGE_COUNT * 4 == p::STAGE_MS_EWMA);
    assert!(p::STAGE_MS_EWMA + p::STAGE_COUNT * 4 == p::COUNTERS);
    assert!(p::COUNTERS + p::COUNTER_COUNT * 4 <= p::SIZE);
    assert!(stage::PUBLISH + 1 == p::STAGE_COUNT);
    assert!(counter::ASSIST_CHANGES + 1 == p::COUNTER_COUNT);

    // Terrain patch: header, then one packed vertex per post.
    assert!(terrain::VERTEX_COUNT == terrain::POSTS * terrain::POSTS);
    assert!(terrain::SIZE == terrain::HEADER_SIZE + terrain::VERTEX_COUNT * terrain::VERTEX_BYTES);
    assert!(terrain::V_HEIGHT + 4 == terrain::VERTEX_BYTES);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magics_spell_their_tags() {
        let tag = |m: usize| -> [u8; 4] { u32::try_from(m).unwrap().to_le_bytes() };
        assert_eq!(&tag(snapshot::MAGIC), b"FGSN");
        assert_eq!(&tag(snapshot_sab::MAGIC), b"FGSB");
        assert_eq!(&tag(input_ring::MAGIC), b"FGIR");
    }

    #[test]
    fn table_lists_every_group_once_and_its_values_match() {
        let mut names: Vec<_> = TABLE.iter().map(|(g, _)| *g).collect();
        let n = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), n);
        let snap = TABLE.iter().find(|(g, _)| *g == "snapshot").unwrap().1;
        assert!(snap.contains(&("SIZE", snapshot::SIZE)));
        assert!(snap.contains(&("B_QUAT", snapshot::B_QUAT)));
        assert!(names.contains(&"hash_kind"));
    }
}
