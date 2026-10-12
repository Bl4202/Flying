//! Snapshot v1: the render-relevant state published at the end of every tick
//! (GDD 61.2 "Sim -> Render", layout in [`crate::layout::snapshot`]).
//!
//! The core writes into a buffer it owns and never reallocates; the host copies it out into a
//! shared, seqlock-protected slot. Positions are `f64` in BF so the renderer can subtract the
//! camera in `f64` before casting to `f32` (ADR-03, GDD 44.8); attitudes and velocities are `f32`.

use crate::layout::snapshot as s;
use crate::observer::Observer;
use crate::probe::{BODY_COUNT, ProbeWorld};

/// Inputs to one snapshot.
pub struct SnapshotSource<'a> {
    /// Tick just completed.
    pub tick: u64,
    /// `CoreState` as a number (see [`crate::layout::core_state`]).
    pub core_state: u32,
    /// Sequence number of the last applied input frame.
    pub last_input_seq: u32,
    /// Host timestamp of that frame, milliseconds (observation only).
    pub input_t_ms: f64,
    /// The observer.
    pub observer: &'a Observer,
    /// The probe bodies.
    pub probe: &'a ProbeWorld,
}

/// A preallocated snapshot.
#[derive(Debug)]
pub struct SnapshotBuffer {
    buf: Box<[u8]>,
}

fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_u64(buf: &mut [u8], at: usize, v: u64) {
    buf[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

fn put_f64(buf: &mut [u8], at: usize, v: f64) {
    put_u64(buf, at, v.to_bits());
}

fn put_f32(buf: &mut [u8], at: usize, v: f32) {
    put_u32(buf, at, v.to_bits());
}

impl Default for SnapshotBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotBuffer {
    /// A zeroed snapshot carrying only the magic and version.
    #[must_use]
    pub fn new() -> Self {
        let mut buf = vec![0_u8; s::SIZE].into_boxed_slice();
        put_u32(&mut buf, s::H_MAGIC, s::MAGIC as u32);
        put_u32(&mut buf, s::H_VERSION, s::VERSION as u32);
        Self { buf }
    }

    /// The encoded snapshot, [`crate::layout::snapshot::SIZE`] bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.buf
    }

    /// Overwrites the snapshot. `publish_time_ms` is left at zero for the host to stamp.
    pub fn write(&mut self, src: &SnapshotSource<'_>) {
        let b = &mut self.buf[..];
        put_u64(b, s::H_TICK, src.tick);
        put_f64(b, s::H_PUBLISH_TIME_MS, 0.0);
        put_u32(b, s::H_LAST_INPUT_SEQ, src.last_input_seq);
        put_u32(b, s::H_BODY_COUNT, BODY_COUNT as u32);
        put_u32(b, s::H_CORE_STATE, src.core_state);
        put_f64(b, s::H_INPUT_T_MS, src.input_t_ms);

        let o = src.observer;
        let (p, v, q) = (o.pos_bf(), o.vel_bf(), o.attitude());
        put_f64(b, s::O_POS_BF, p.x);
        put_f64(b, s::O_POS_BF + 8, p.y);
        put_f64(b, s::O_POS_BF + 16, p.z);
        for (k, c) in [q.w, q.x, q.y, q.z].into_iter().enumerate() {
            put_f32(b, s::O_QUAT + 4 * k, c as f32);
        }
        put_f32(b, s::O_VEL_BF, v.x as f32);
        put_f32(b, s::O_VEL_BF + 4, v.y as f32);
        put_f32(b, s::O_VEL_BF + 8, v.z as f32);
        put_f32(b, s::O_SPEED, o.speed_mps() as f32);
        put_f32(b, s::O_THROTTLE, o.throttle() as f32);

        for i in 0..BODY_COUNT {
            let [x, y, z] = src.probe.pos_bf(i);
            put_f64(b, s::B_X + 8 * i, x);
            put_f64(b, s::B_Y + 8 * i, y);
            put_f64(b, s::B_Z + 8 * i, z);
            for (k, c) in src.probe.quat(i).into_iter().enumerate() {
                put_f32(b, s::B_QUAT + 16 * i + 4 * k, c as f32);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seams::geo::FlatGeo;

    fn u32_at(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }
    fn f64_at(b: &[u8], o: usize) -> f64 {
        f64::from_le_bytes(b[o..o + 8].try_into().unwrap())
    }
    fn f32_at(b: &[u8], o: usize) -> f32 {
        f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }

    fn sample() -> (SnapshotBuffer, Observer, ProbeWorld) {
        let rng = crate::rng::RngStreams::new(1);
        let probe = ProbeWorld::new(
            rng.stream("probe/init"),
            rng.stream("probe/gust"),
            rng.stream("probe/reseed"),
        );
        let observer = Observer::spawn(&FlatGeo::new());
        (SnapshotBuffer::new(), observer, probe)
    }

    #[test]
    fn a_fresh_buffer_has_only_the_header_identity() {
        let b = SnapshotBuffer::new();
        assert_eq!(b.bytes().len(), s::SIZE);
        assert_eq!(&b.bytes()[0..4], b"FGSN");
        assert_eq!(u32_at(b.bytes(), s::H_VERSION), 1);
        assert!(b.bytes()[8..].iter().all(|&x| x == 0));
    }

    #[test]
    fn written_fields_sit_at_their_documented_offsets() {
        let (mut buf, observer, probe) = sample();
        buf.write(&SnapshotSource {
            tick: 0x0102_0304_0506,
            core_state: 2,
            last_input_seq: 77,
            input_t_ms: 1234.5,
            observer: &observer,
            probe: &probe,
        });
        let b = buf.bytes();
        assert_eq!(&b[0..4], b"FGSN");
        assert_eq!(
            u64::from_le_bytes(b[s::H_TICK..s::H_TICK + 8].try_into().unwrap()),
            0x0102_0304_0506
        );
        assert_eq!(f64_at(b, s::H_PUBLISH_TIME_MS), 0.0);
        assert_eq!(u32_at(b, s::H_LAST_INPUT_SEQ), 77);
        assert_eq!(u32_at(b, s::H_BODY_COUNT), 32);
        assert_eq!(u32_at(b, s::H_CORE_STATE), 2);
        assert_eq!(f64_at(b, s::H_INPUT_T_MS), 1234.5);

        let p = observer.pos_bf();
        assert_eq!(f64_at(b, s::O_POS_BF), p.x);
        assert_eq!(f64_at(b, s::O_POS_BF + 8), p.y);
        assert_eq!(f64_at(b, s::O_POS_BF + 16), p.z);
        let q = observer.attitude();
        assert_eq!(f32_at(b, s::O_QUAT), q.w as f32);
        assert_eq!(f32_at(b, s::O_QUAT + 12), q.z as f32);
        assert_eq!(f32_at(b, s::O_SPEED), observer.speed_mps() as f32);
        assert_eq!(f32_at(b, s::O_THROTTLE), 0.5);

        for i in [0, 1, 31] {
            let [x, y, z] = probe.pos_bf(i);
            assert_eq!(f64_at(b, s::B_X + 8 * i), x);
            assert_eq!(f64_at(b, s::B_Y + 8 * i), y);
            assert_eq!(f64_at(b, s::B_Z + 8 * i), z);
            assert_eq!(f32_at(b, s::B_QUAT + 16 * i), probe.quat(i)[0] as f32);
        }
        // Slots past the body count are untouched.
        assert_eq!(f64_at(b, s::B_X + 8 * 32), 0.0);
        assert_eq!(b.len(), s::SIZE);
    }

    #[test]
    fn writing_is_idempotent() {
        let (mut buf, observer, probe) = sample();
        let src = SnapshotSource {
            tick: 5,
            core_state: 2,
            last_input_seq: 0,
            input_t_ms: 0.0,
            observer: &observer,
            probe: &probe,
        };
        buf.write(&src);
        let first = buf.bytes().to_vec();
        buf.write(&src);
        assert_eq!(first, buf.bytes());
    }
}
