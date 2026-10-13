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
        VERSION = 2,
        // Frames the host can hand over per `fg_push_inputs` call.
        INPUT_BATCH = 8,
        SNAPSHOT_LAYOUT_VERSION = 1,
    }

    /// `InputFrame`, 96 bytes (GDD 58.2).
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

    /// Bit indices into `InputFrame.buttons`.
    button {
        RESET = 0,
    }

    /// Bits of `InputFrame.flags`.
    input_flag {
        // The host neutralised the controls (focus lost, tab hidden).
        NEUTRAL = 1,
    }

    /// Snapshot v1: header, observer block, bodies in structure-of-arrays form.
    snapshot {
        MAGIC = 0x4E53_4746,
        VERSION = 1,
        HEADER_SIZE = 64,
        H_MAGIC = 0,
        H_VERSION = 4,
        H_TICK = 8,
        H_PUBLISH_TIME_MS = 16,
        H_LAST_INPUT_SEQ = 24,
        H_BODY_COUNT = 28,
        H_CORE_STATE = 32,
        H_INPUT_T_MS = 40,
        OBSERVER = 64,
        O_POS_BF = 64,
        O_QUAT = 88,
        O_VEL_BF = 104,
        O_SPEED = 116,
        O_THROTTLE = 120,
        BODY_CAPACITY = 256,
        B_X = 128,
        B_Y = 2176,
        B_Z = 4224,
        B_QUAT = 6272,
        SIZE = 10368,
    }

    /// Shared snapshot buffer: three slots under a seqlock.
    snapshot_sab {
        MAGIC = 0x4253_4746,
        VERSION = 1,
        HEADER_SIZE = 64,
        SLOT_COUNT = 3,
        SLOT_BYTES = 10368,
        SIZE = 31168,
        H_MAGIC = 0,
        H_VERSION = 4,
        H_SLOT_BYTES = 8,
        H_SLOT_COUNT = 12,
        H_PUBLISHED = 16,
        H_LATEST = 20,
        H_SLOT_SEQ = 24,
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
        SIZE = 144,
        TICKS = 0,
        TICK_MS_LAST = 8,
        TICK_MS_EWMA = 12,
        STAGE_COUNT = 11,
        STAGE_MS_LAST = 16,
        STAGE_MS_EWMA = 60,
        COUNTERS = 104,
        COUNTER_COUNT = 8,
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
        OBSERVER_RESETS = 4,
        GROUND_CLAMPS = 5,
        BOUNDARY_CLAMPS = 6,
        SNAPSHOTS = 7,
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
        OBSERVER = 4,
        INPUT = 5,
        CONTENT = 6,
    }

    /// `kind` argument of `fg_set_input_script`.
    input_script {
        NONE = 0,
        ERRATIC = 1,
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
    assert!(f::HATS + f::HAT_COUNT <= f::SIZE);

    // Snapshot: header, observer, then four structure-of-arrays blocks that end exactly at SIZE.
    assert!(s::H_INPUT_T_MS + 8 <= s::HEADER_SIZE);
    assert!(s::OBSERVER == s::HEADER_SIZE);
    assert!(s::O_POS_BF == s::OBSERVER);
    assert!(s::O_POS_BF + 24 == s::O_QUAT);
    assert!(s::O_QUAT + 16 == s::O_VEL_BF);
    assert!(s::O_VEL_BF + 12 == s::O_SPEED);
    assert!(s::O_SPEED + 4 == s::O_THROTTLE);
    assert!(s::O_THROTTLE + 4 <= s::OBSERVER + 64);
    assert!(s::B_X == s::OBSERVER + 64);
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
    assert!(counter::SNAPSHOTS + 1 == p::COUNTER_COUNT);

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
