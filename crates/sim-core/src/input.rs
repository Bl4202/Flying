//! Player input as the simulation sees it (GDD 58.2, 61.4.3 stage 1).
//!
//! The host writes fixed 96-byte [`InputFrame`] records (layout in [`crate::layout::input_frame`]).
//! They queue until the tick boundary, then the Ingest stage validates and applies them in order:
//! axes are "latest wins", button presses are edges accumulated over every frame applied in the
//! tick, so a tap between two ticks is never lost.

use crate::hash::StateHasher;
use crate::layout::{axis, input_flag, input_frame as f};
use crate::rng::Xoshiro256StarStar;

/// Bytes in one encoded frame.
pub const FRAME_BYTES: usize = f::SIZE;
/// Axes per frame.
pub const AXIS_COUNT: usize = f::AXIS_COUNT;
/// Frames the host may queue between two ticks; extras are dropped and counted.
pub const PENDING_CAPACITY: usize = 64;

/// One sampled state of the player's controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputFrame {
    /// Increases by one per frame, wrapping; used to drop stale or duplicate frames.
    pub seq: u32,
    /// Bit flags (see [`crate::layout::input_flag`]).
    pub flags: u32,
    /// Host timestamp in milliseconds. Observation only: never part of simulation state.
    pub t_ms: f64,
    /// Pitch, roll, yaw, throttle, throttle2, brake L/R, trims, cursor (see [`crate::layout::axis`]).
    pub axes: [f32; AXIS_COUNT],
    /// One bit per action (see [`crate::layout::button`]).
    pub buttons: u64,
    /// Hat switches.
    pub hats: [u8; f::HAT_COUNT],
}

fn rd_u32(b: &[u8], o: usize) -> u32 {
    let mut a = [0_u8; 4];
    a.copy_from_slice(&b[o..o + 4]);
    u32::from_le_bytes(a)
}

fn rd_u64(b: &[u8], o: usize) -> u64 {
    let mut a = [0_u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}

impl InputFrame {
    /// A frame with every control released.
    #[must_use]
    pub const fn neutral(seq: u32) -> Self {
        Self {
            seq,
            flags: 0,
            t_ms: 0.0,
            axes: [0.0; AXIS_COUNT],
            buttons: 0,
            hats: [0; f::HAT_COUNT],
        }
    }

    /// Decodes one record. `None` unless `bytes` is exactly [`FRAME_BYTES`] long.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != FRAME_BYTES {
            return None;
        }
        let mut axes = [0.0_f32; AXIS_COUNT];
        for (i, a) in axes.iter_mut().enumerate() {
            *a = f32::from_bits(rd_u32(bytes, f::AXES + i * 4));
        }
        Some(Self {
            seq: rd_u32(bytes, f::SEQ),
            flags: rd_u32(bytes, f::FLAGS),
            t_ms: f64::from_bits(rd_u64(bytes, f::T_MS)),
            axes,
            buttons: rd_u64(bytes, f::BUTTONS),
            hats: [bytes[f::HATS], bytes[f::HATS + 1]],
        })
    }

    /// Encodes into a record. Reserved bytes are written as zero.
    pub fn encode(&self, out: &mut [u8; FRAME_BYTES]) {
        out.fill(0);
        out[f::SEQ..f::SEQ + 4].copy_from_slice(&self.seq.to_le_bytes());
        out[f::FLAGS..f::FLAGS + 4].copy_from_slice(&self.flags.to_le_bytes());
        out[f::T_MS..f::T_MS + 8].copy_from_slice(&self.t_ms.to_bits().to_le_bytes());
        for (i, a) in self.axes.iter().enumerate() {
            out[f::AXES + i * 4..f::AXES + i * 4 + 4].copy_from_slice(&a.to_bits().to_le_bytes());
        }
        out[f::BUTTONS..f::BUTTONS + 8].copy_from_slice(&self.buttons.to_le_bytes());
        out[f::HATS] = self.hats[0];
        out[f::HATS + 1] = self.hats[1];
    }
}

/// Axes that are one-sided levers or pedals (0 to 1); every other axis is bipolar (-1 to 1).
const fn is_unipolar(index: usize) -> bool {
    matches!(
        index,
        axis::THROTTLE | axis::THROTTLE2 | axis::BRAKE_L | axis::BRAKE_R
    )
}

/// Counters for diagnostics. They are deterministic functions of the frames received.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputCounters {
    /// Frames applied.
    pub applied: u32,
    /// Frames that carried a NaN or infinite axis (the axis was zeroed).
    pub nonfinite: u32,
    /// Frames dropped as stale or duplicate.
    pub stale: u32,
    /// Frames dropped because the pending queue was full.
    pub overflow: u32,
}

/// What happened to a frame offered to [`InputState::push`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accept {
    /// The frame became the latest.
    Applied,
    /// An older or repeated frame; ignored.
    Stale,
}

/// Validated controls, plus the queue of frames waiting for the next tick.
#[derive(Clone, Debug)]
pub struct InputState {
    latest: InputFrame,
    /// The three frames before `latest`, newest first (network redundancy later, GDD 0.6.1).
    recent: [InputFrame; 3],
    have_any: bool,
    prev_buttons: u64,
    edges: u64,
    throttle_cmd: f32,
    counters: InputCounters,
    pending: [InputFrame; PENDING_CAPACITY],
    pending_len: usize,
    /// `t_ms` of the last applied frame, for latency measurement. Never hashed.
    last_applied_t_ms: f64,
    script: Option<Script>,
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}

impl InputState {
    /// Throttle before any input arrives: half.
    pub const DEFAULT_THROTTLE: f32 = 0.5;

    /// No input yet.
    #[must_use]
    pub fn new() -> Self {
        Self {
            latest: InputFrame::neutral(0),
            recent: [InputFrame::neutral(0); 3],
            have_any: false,
            prev_buttons: 0,
            edges: 0,
            throttle_cmd: Self::DEFAULT_THROTTLE,
            counters: InputCounters::default(),
            pending: [InputFrame::neutral(0); PENDING_CAPACITY],
            pending_len: 0,
            last_applied_t_ms: 0.0,
            script: None,
        }
    }

    /// Installs (or removes) the scripted pilot that stands in for a person.
    pub fn set_script(&mut self, script: Option<Script>) {
        self.script = script;
    }

    /// Queues a frame for the next tick. Returns false if the queue is full.
    pub fn enqueue(&mut self, frame: InputFrame) -> bool {
        if self.pending_len == PENDING_CAPACITY {
            self.counters.overflow = self.counters.overflow.wrapping_add(1);
            return false;
        }
        self.pending[self.pending_len] = frame;
        self.pending_len += 1;
        true
    }

    /// Start of a tick's Ingest: forget last tick's press edges, run the script, then apply
    /// everything queued by the host, in order.
    pub fn ingest(&mut self, tick: u64) {
        self.edges = 0;
        if let Some(script) = self.script.as_mut() {
            let frame = match script {
                Script::Erratic(pilot) => pilot.next_frame(tick),
                Script::Flight60 => flight60_frame(tick),
            };
            // Exercise the real wire path: encode, then decode what a host would have sent.
            let mut wire = [0_u8; FRAME_BYTES];
            frame.encode(&mut wire);
            if let Some(decoded) = InputFrame::decode(&wire) {
                self.push(decoded);
            }
        }
        for i in 0..self.pending_len {
            let frame = self.pending[i];
            self.push(frame);
        }
        self.pending_len = 0;
    }

    /// Validates and applies one frame immediately.
    pub fn push(&mut self, mut frame: InputFrame) -> Accept {
        if self.have_any && frame.seq.wrapping_sub(self.latest.seq).cast_signed() <= 0 {
            self.counters.stale = self.counters.stale.wrapping_add(1);
            return Accept::Stale;
        }
        let mut saw_nonfinite = false;
        for (i, a) in frame.axes.iter_mut().enumerate() {
            if !a.is_finite() {
                *a = 0.0;
                saw_nonfinite = true;
            }
            *a = if is_unipolar(i) {
                a.clamp(0.0, 1.0)
            } else {
                a.clamp(-1.0, 1.0)
            };
        }
        if saw_nonfinite {
            self.counters.nonfinite = self.counters.nonfinite.wrapping_add(1);
        }
        self.edges |= frame.buttons & !self.prev_buttons;
        self.prev_buttons = frame.buttons;
        if frame.flags & (input_flag::NEUTRAL as u32) == 0 {
            self.throttle_cmd = frame.axes[axis::THROTTLE];
        }
        self.recent = [self.latest, self.recent[0], self.recent[1]];
        self.latest = frame;
        self.have_any = true;
        self.last_applied_t_ms = frame.t_ms;
        self.counters.applied = self.counters.applied.wrapping_add(1);
        Accept::Applied
    }

    /// The latest validated frame.
    #[must_use]
    pub fn latest(&self) -> &InputFrame {
        &self.latest
    }

    /// The three frames before the latest, newest first.
    #[must_use]
    pub fn recent(&self) -> &[InputFrame; 3] {
        &self.recent
    }

    /// Value of one axis in the latest frame, as `f64`.
    #[must_use]
    pub fn axis(&self, index: usize) -> f64 {
        f64::from(self.latest.axes[index])
    }

    /// The throttle setting in force (held while a frame carries the NEUTRAL flag).
    #[must_use]
    pub fn throttle_cmd(&self) -> f64 {
        f64::from(self.throttle_cmd)
    }

    /// True if button `bit` went from released to pressed in frames applied this tick.
    #[must_use]
    pub fn pressed(&self, bit: usize) -> bool {
        bit < 64 && self.edges & (1_u64 << bit) != 0
    }

    /// True if button `bit` is down in the latest frame (for held controls such as trim).
    #[must_use]
    pub fn held(&self, bit: usize) -> bool {
        bit < 64 && self.latest.buttons & (1_u64 << bit) != 0
    }

    /// Counters since creation.
    #[must_use]
    pub fn counters(&self) -> InputCounters {
        self.counters
    }

    /// Sequence number of the latest applied frame.
    #[must_use]
    pub fn last_seq(&self) -> u32 {
        self.latest.seq
    }

    /// Host timestamp of the latest applied frame (observation only).
    #[must_use]
    pub fn last_applied_t_ms(&self) -> f64 {
        self.last_applied_t_ms
    }

    /// Feeds the deterministic part of the state into `h`. Host timestamps are excluded.
    pub fn hash_into(&self, h: &mut StateHasher) {
        let frame = |h: &mut StateHasher, fr: &InputFrame| {
            h.write_u64(u64::from(fr.seq) | (u64::from(fr.flags) << 32));
            for a in fr.axes {
                h.write_u64(u64::from(a.to_bits()));
            }
            h.write_u64(fr.buttons);
            h.write_u64(u64::from(fr.hats[0]) | (u64::from(fr.hats[1]) << 8));
        };
        frame(h, &self.latest);
        for fr in &self.recent {
            frame(h, fr);
        }
        h.write_u64(u64::from(self.have_any));
        h.write_u64(self.prev_buttons);
        h.write_u64(u64::from(self.throttle_cmd.to_bits()));
        let c = self.counters;
        h.write_u64(u64::from(c.applied) | (u64::from(c.nonfinite) << 32));
        h.write_u64(u64::from(c.stale) | (u64::from(c.overflow) << 32));
        match &self.script {
            Some(Script::Erratic(p)) => {
                h.write_u64(1);
                p.hash_into(h);
            }
            // A pure function of the tick: nothing to hash but which script it is.
            Some(Script::Flight60) => h.write_u64(2),
            None => h.write_u64(0),
        }
    }
}

/// A scripted pilot that stands in for a person (see [`crate::layout::input_script`]).
#[derive(Clone, Debug)]
pub enum Script {
    /// Random-walk stick and throttle with an occasional reset.
    Erratic(ErraticPilot),
    /// The 60 s scripted flight of `AC-11.7`, [`flight60_frame`].
    Flight60,
}

/// Length of the Flight60 script, ticks (60 s).
pub const FLIGHT60_TICKS: u64 = 3_600;

/// The frame of the Flight60 script for `tick`: a fixed, input-only flight that exercises the
/// whole flight model (`AC-11.7`). By second:
///
/// | s | input |
/// |---|---|
/// | 0-4 | neutral, throttle 0.5 |
/// | 5-9 | throttle 1.0 (afterburner light-off) |
/// | 10-14 | pitch +0.5 |
/// | 15-19 | roll +1.0 |
/// | 20-24 | neutral, throttle 0.8 |
/// | 25-29 | pitch -0.3 |
/// | 30-39 | aileron then rudder doublets, +-0.5 for 1 s each |
/// | 40-49 | throttle 0, pitch +0.8 (towards the stall) |
/// | 50-54 | neutral, throttle 0.8 |
/// | 55-59 | trim nose up held 2.5 s, then nose down held 2.5 s |
///
/// After 60 s it holds neutral at 0.8. Sequence numbers and timestamps come from the tick.
#[must_use]
pub fn flight60_frame(tick: u64) -> InputFrame {
    use crate::layout::button;
    let mut frame = InputFrame::neutral(u32::try_from((tick + 1) & 0xFFFF_FFFF).unwrap_or(0));
    frame.t_ms = tick as f64 * (1_000.0 / 60.0);
    let s = tick / 60;
    let (pitch, roll, yaw, throttle) = match s {
        0..=4 => (0.0, 0.0, 0.0, 0.5),
        5..=9 => (0.0, 0.0, 0.0, 1.0),
        10..=14 => (0.5, 0.0, 0.0, 1.0),
        15..=19 => (0.0, 1.0, 0.0, 1.0),
        25..=29 => (-0.3, 0.0, 0.0, 0.8),
        30..=39 => match (s - 30) % 4 {
            0 => (0.0, 0.5, 0.0, 0.8),
            1 => (0.0, -0.5, 0.0, 0.8),
            2 => (0.0, 0.0, 0.5, 0.8),
            _ => (0.0, 0.0, -0.5, 0.8),
        },
        40..=49 => (0.8, 0.0, 0.0, 0.0),
        _ => (0.0, 0.0, 0.0, 0.8),
    };
    frame.axes[axis::PITCH] = pitch;
    frame.axes[axis::ROLL] = roll;
    frame.axes[axis::YAW] = yaw;
    frame.axes[axis::THROTTLE] = throttle;
    if (3_300..3_450).contains(&tick) {
        frame.buttons |= 1 << button::TRIM_NOSE_UP;
    } else if (3_450..FLIGHT60_TICKS).contains(&tick) {
        frame.buttons |= 1 << button::TRIM_NOSE_DOWN;
    }
    frame
}

/// A seeded random-walk pilot: smooth stick wandering, drifting throttle, an occasional reset
/// press. It produces real frames, so scripted runs exercise the same decode and validation path
/// as a person, and it is a pure function of the seed and the tick (hashes depend on it).
#[derive(Clone, Debug)]
pub struct ErraticPilot {
    rng: Xoshiro256StarStar,
    pitch: f64,
    roll: f64,
    yaw: f64,
    throttle: f64,
}

/// Ticks between scripted reset presses (30 s at 60 Hz).
const RESET_EVERY_TICKS: u64 = 1_800;

impl ErraticPilot {
    /// Builds a pilot from its own random stream.
    #[must_use]
    pub fn new(rng: Xoshiro256StarStar) -> Self {
        Self {
            rng,
            pitch: 0.0,
            roll: 0.0,
            yaw: 0.0,
            throttle: f64::from(InputState::DEFAULT_THROTTLE),
        }
    }

    /// The frame for `tick`. Sequence numbers start at 1; timestamps come from the tick, not a clock.
    pub fn next_frame(&mut self, tick: u64) -> InputFrame {
        let mut step =
            |v: f64, gain: f64| (v * 0.995 + (self.rng.next_f64() - 0.5) * gain).clamp(-1.0, 1.0);
        self.pitch = step(self.pitch, 0.10);
        self.roll = step(self.roll, 0.16);
        self.yaw = step(self.yaw, 0.06);
        self.throttle = (self.throttle + (self.rng.next_f64() - 0.5) * 0.01).clamp(0.2, 1.0);

        let mut frame = InputFrame::neutral(u32::try_from((tick + 1) & 0xFFFF_FFFF).unwrap_or(0));
        frame.t_ms = tick as f64 * (1_000.0 / 60.0);
        frame.axes[axis::PITCH] = self.pitch as f32;
        frame.axes[axis::ROLL] = self.roll as f32;
        frame.axes[axis::YAW] = self.yaw as f32;
        frame.axes[axis::THROTTLE] = self.throttle as f32;
        if tick % RESET_EVERY_TICKS == RESET_EVERY_TICKS - 1 {
            frame.buttons |= 1 << crate::layout::button::RESET;
        }
        frame
    }

    /// Feeds the pilot's state into `h`.
    pub fn hash_into(&self, h: &mut StateHasher) {
        for w in self.rng.state() {
            h.write_u64(w);
        }
        h.write_f64s(&[self.pitch, self.roll, self.yaw, self.throttle]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::button;

    fn frame(seq: u32) -> InputFrame {
        InputFrame::neutral(seq)
    }

    #[test]
    fn encode_decode_round_trip_and_layout() {
        let mut fr = frame(0xDEAD_BEEF);
        fr.flags = 1;
        fr.t_ms = 12_345.678;
        fr.axes = [
            0.5, -0.25, 1.0, 0.75, 0.0, 0.1, 0.2, -0.3, 0.4, -0.5, 0.6, -0.7,
        ];
        fr.buttons = 0x8000_0000_0000_0005;
        fr.hats = [3, 7];
        let mut bytes = [0xAA_u8; FRAME_BYTES];
        fr.encode(&mut bytes);
        assert_eq!(InputFrame::decode(&bytes), Some(fr));
        // Spot-check the documented offsets.
        assert_eq!(&bytes[0..4], &0xDEAD_BEEF_u32.to_le_bytes());
        assert_eq!(&bytes[16..20], &0.5_f32.to_le_bytes());
        assert_eq!(&bytes[64..72], &0x8000_0000_0000_0005_u64.to_le_bytes());
        assert_eq!((bytes[72], bytes[73]), (3, 7));
        assert!(
            bytes[74..].iter().all(|&b| b == 0),
            "reserved bytes are zero"
        );
    }

    #[test]
    fn decode_rejects_wrong_lengths() {
        assert!(InputFrame::decode(&[0; FRAME_BYTES - 1]).is_none());
        assert!(InputFrame::decode(&[0; FRAME_BYTES + 1]).is_none());
        assert!(InputFrame::decode(&[]).is_none());
    }

    #[test]
    fn axes_are_clamped_and_non_finite_values_zeroed() {
        let mut s = InputState::new();
        let mut fr = frame(1);
        fr.axes[axis::PITCH] = 7.0;
        fr.axes[axis::ROLL] = f32::NAN;
        fr.axes[axis::YAW] = f32::NEG_INFINITY;
        fr.axes[axis::THROTTLE] = -0.5;
        fr.axes[axis::BRAKE_L] = 3.0;
        assert_eq!(s.push(fr), Accept::Applied);
        assert_eq!(s.axis(axis::PITCH), 1.0);
        assert_eq!(s.axis(axis::ROLL), 0.0);
        assert_eq!(s.axis(axis::YAW), 0.0);
        assert_eq!(s.axis(axis::THROTTLE), 0.0);
        assert_eq!(s.axis(axis::BRAKE_L), 1.0);
        assert_eq!(s.counters().nonfinite, 1);
    }

    #[test]
    fn stale_and_duplicate_sequence_numbers_are_dropped() {
        let mut s = InputState::new();
        assert_eq!(s.push(frame(10)), Accept::Applied);
        assert_eq!(s.push(frame(10)), Accept::Stale);
        assert_eq!(s.push(frame(9)), Accept::Stale);
        assert_eq!(s.push(frame(11)), Accept::Applied);
        assert_eq!(s.counters().stale, 2);
        assert_eq!(s.counters().applied, 2);
    }

    #[test]
    fn sequence_numbers_wrap() {
        let mut s = InputState::new();
        assert_eq!(s.push(frame(u32::MAX - 1)), Accept::Applied);
        assert_eq!(s.push(frame(u32::MAX)), Accept::Applied);
        assert_eq!(s.push(frame(0)), Accept::Applied, "0 follows u32::MAX");
        assert_eq!(s.push(frame(u32::MAX)), Accept::Stale);
    }

    #[test]
    fn a_button_tap_between_ticks_is_seen_as_an_edge() {
        let mut s = InputState::new();
        // Press and release both arrive before the next tick: the edge must survive.
        let mut down = frame(1);
        down.buttons = 1 << button::RESET;
        let up = frame(2);
        assert!(s.enqueue(down) && s.enqueue(up));
        s.ingest(0);
        assert!(s.pressed(button::RESET));
        // The next tick has no new press.
        s.ingest(1);
        assert!(!s.pressed(button::RESET));
        // Holding does not repeat the edge.
        let mut held = frame(3);
        held.buttons = 1 << button::RESET;
        let mut held2 = frame(4);
        held2.buttons = 1 << button::RESET;
        s.enqueue(held);
        s.ingest(2);
        assert!(s.pressed(button::RESET));
        s.enqueue(held2);
        s.ingest(3);
        assert!(!s.pressed(button::RESET));
    }

    #[test]
    fn a_neutral_frame_holds_the_throttle() {
        let mut s = InputState::new();
        let mut a = frame(1);
        a.axes[axis::THROTTLE] = 0.9;
        s.push(a);
        let mut neutral = frame(2);
        neutral.flags = input_flag::NEUTRAL as u32;
        s.push(neutral);
        assert!((s.throttle_cmd() - 0.9).abs() < 1e-6);
        assert_eq!(s.axis(axis::PITCH), 0.0);
    }

    #[test]
    fn the_queue_drops_extras_and_counts_them() {
        let mut s = InputState::new();
        for i in 0..PENDING_CAPACITY {
            assert!(s.enqueue(frame(u32::try_from(i + 1).unwrap())));
        }
        assert!(!s.enqueue(frame(1_000)));
        assert_eq!(s.counters().overflow, 1);
        s.ingest(0);
        assert_eq!(s.counters().applied as usize, PENDING_CAPACITY);
        assert!(s.enqueue(frame(2_000)), "the queue is empty again");
    }

    #[test]
    fn recent_frames_are_kept_newest_first() {
        let mut s = InputState::new();
        for seq in 1..=5 {
            s.push(frame(seq));
        }
        assert_eq!(s.latest().seq, 5);
        assert_eq!(
            s.recent().iter().map(|f| f.seq).collect::<Vec<_>>(),
            [4, 3, 2]
        );
    }

    #[test]
    fn the_erratic_pilot_is_reproducible_and_stays_in_range() {
        let make = || ErraticPilot::new(crate::rng::RngStreams::new(5).stream("input/erratic"));
        let (mut a, mut b) = (make(), make());
        let mut resets = 0;
        for tick in 0..5_000_u64 {
            let (fa, fb) = (a.next_frame(tick), b.next_frame(tick));
            assert_eq!(fa, fb);
            assert_eq!(u64::from(fa.seq), tick + 1);
            assert!(fa.axes.iter().all(|v| v.is_finite()));
            assert!(fa.axes[axis::PITCH].abs() <= 1.0 && fa.axes[axis::THROTTLE] >= 0.2);
            if fa.buttons & (1 << button::RESET) != 0 {
                resets += 1;
            }
        }
        assert_eq!(resets, 2, "one reset press per 1 800 ticks");
    }

    #[test]
    fn the_script_goes_through_the_wire_path_and_changes_the_hash() {
        let mut s = InputState::new();
        s.set_script(Some(Script::Erratic(ErraticPilot::new(
            crate::rng::RngStreams::new(1).stream("input/erratic"),
        ))));
        let mut h0 = StateHasher::new();
        s.hash_into(&mut h0);
        s.ingest(0);
        assert_eq!(s.counters().applied, 1);
        assert_eq!(s.last_seq(), 1);
        let mut h1 = StateHasher::new();
        s.hash_into(&mut h1);
        assert_ne!(h0.finish(), h1.finish());
    }

    #[test]
    fn host_timestamps_never_reach_the_hash() {
        let (mut a, mut b) = (InputState::new(), InputState::new());
        let (mut fa, mut fb) = (frame(1), frame(1));
        fa.t_ms = 1.0;
        fb.t_ms = 9_999_999.0;
        a.push(fa);
        b.push(fb);
        let (mut ha, mut hb) = (StateHasher::new(), StateHasher::new());
        a.hash_into(&mut ha);
        b.hash_into(&mut hb);
        assert_eq!(ha.finish(), hb.finish());
    }

    #[test]
    fn the_flight60_script_follows_its_table() {
        let at = |s: f64| flight60_frame((s * 60.0) as u64);
        assert_eq!(at(0.0).seq, 1);
        assert_eq!(at(2.0).axes[axis::THROTTLE], 0.5);
        assert_eq!(at(7.0).axes[axis::THROTTLE], 1.0);
        assert_eq!(at(12.0).axes[axis::PITCH], 0.5);
        assert_eq!(at(17.0).axes[axis::ROLL], 1.0);
        assert_eq!(at(30.5).axes[axis::ROLL], 0.5);
        assert_eq!(at(31.5).axes[axis::ROLL], -0.5);
        assert_eq!(at(32.5).axes[axis::YAW], 0.5);
        assert_eq!(at(33.5).axes[axis::YAW], -0.5);
        assert_eq!(at(45.0).axes[axis::PITCH], 0.8);
        assert_eq!(at(45.0).axes[axis::THROTTLE], 0.0);
        assert_eq!(at(56.0).buttons, 1 << button::TRIM_NOSE_UP);
        assert_eq!(at(58.0).buttons, 1 << button::TRIM_NOSE_DOWN);
        assert_eq!(at(61.0).buttons, 0);
        // Through the wire path, the trim buttons read as held.
        let mut st = InputState::new();
        st.set_script(Some(Script::Flight60));
        st.ingest(3_400);
        assert!(st.held(button::TRIM_NOSE_UP) && !st.held(button::TRIM_NOSE_DOWN));
        assert!(!st.held(64));
    }
}
