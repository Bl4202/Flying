//! Snapshot v2: the render-relevant state published at the end of every tick
//! (GDD 61.2 "Sim -> Render", layout in [`crate::layout::snapshot`]).
//!
//! The core writes into a buffer it owns and never reallocates; the host copies it out into a
//! shared, seqlock-protected slot. Positions are `f64` in BF so the renderer can subtract the
//! camera in `f64` before casting to `f32` (ADR-03, GDD 44.8); attitudes and velocities are `f32`.

use crate::layout::snapshot as s;
use crate::ownship::OwnshipView;
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
    /// The ownship, already reduced to what the snapshot shows.
    pub ownship: &'a OwnshipView,
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

        let o = src.ownship;
        let (p, v, q) = (o.pos_bf, o.vel_bf, o.att);
        put_f64(b, s::O_POS_BF, p.x);
        put_f64(b, s::O_POS_BF + 8, p.y);
        put_f64(b, s::O_POS_BF + 16, p.z);
        for (k, c) in [q.w, q.x, q.y, q.z].into_iter().enumerate() {
            put_f32(b, s::O_QUAT + 4 * k, c as f32);
        }
        put_f32(b, s::O_VEL_BF, v.x as f32);
        put_f32(b, s::O_VEL_BF + 4, v.y as f32);
        put_f32(b, s::O_VEL_BF + 8, v.z as f32);
        for (at, value) in [
            (s::O_TAS, o.tas_mps),
            (s::O_CAS, o.cas_mps),
            (s::O_MACH, o.mach),
            (s::O_ALPHA, o.alpha_rad),
            (s::O_BETA, o.beta_rad),
            (s::O_NZ, o.nz_g),
            (s::O_THROTTLE, o.throttle),
            (s::O_THRUST, o.thrust_n),
            (s::O_FUEL, o.fuel_kg),
            (s::O_MASS, o.mass_kg),
            (s::O_OMEGA, o.omega_body.x),
            (s::O_OMEGA + 4, o.omega_body.y),
            (s::O_OMEGA + 8, o.omega_body.z),
            (s::O_SURFACES, o.surfaces_rad[0]),
            (s::O_SURFACES + 4, o.surfaces_rad[1]),
            (s::O_SURFACES + 8, o.surfaces_rad[2]),
            (s::O_ALTITUDE, o.altitude_m),
        ] {
            put_f32(b, at, value as f32);
        }
        b[s::O_AB_STAGE] = o.ab_stage;
        b[s::O_FLAGS] = o.flags;

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
    use crate::layout::ownship_flag;
    use crate::math::{DQuat, DVec3};

    fn u32_at(b: &[u8], o: usize) -> u32 {
        u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }
    fn f64_at(b: &[u8], o: usize) -> f64 {
        f64::from_le_bytes(b[o..o + 8].try_into().unwrap())
    }
    fn f32_at(b: &[u8], o: usize) -> f32 {
        f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }

    fn sample() -> (SnapshotBuffer, OwnshipView, ProbeWorld) {
        let rng = crate::rng::RngStreams::new(1);
        let probe = ProbeWorld::new(
            rng.stream("probe/init"),
            rng.stream("probe/gust"),
            rng.stream("probe/reseed"),
        );
        let ownship = OwnshipView {
            pos_bf: DVec3::new(1.5, -6000.25, 2100.125),
            att: DQuat::new(0.5, 0.5, -0.5, 0.5),
            vel_bf: DVec3::new(1.0, 200.0, -3.0),
            tas_mps: 200.5,
            cas_mps: 180.25,
            mach: 0.6,
            alpha_rad: 0.05,
            beta_rad: -0.01,
            nz_g: 1.25,
            throttle: 0.5,
            thrust_n: 41_000.0,
            fuel_kg: 1860.0,
            mass_kg: 10_260.0,
            omega_body: DVec3::new(0.1, -0.2, 0.3),
            surfaces_rad: [-0.04, 0.02, -0.01],
            altitude_m: 2900.5,
            ab_stage: 3,
            flags: (ownship_flag::STALL_WARN | ownship_flag::FLAMEOUT) as u8,
        };
        (SnapshotBuffer::new(), ownship, probe)
    }

    #[test]
    fn a_fresh_buffer_has_only_the_header_identity() {
        let b = SnapshotBuffer::new();
        assert_eq!(b.bytes().len(), s::SIZE);
        assert_eq!(&b.bytes()[0..4], b"FGSN");
        assert_eq!(u32_at(b.bytes(), s::H_VERSION), 2);
        assert!(b.bytes()[8..].iter().all(|&x| x == 0));
    }

    #[test]
    fn written_fields_sit_at_their_documented_offsets() {
        let (mut buf, o, probe) = sample();
        buf.write(&SnapshotSource {
            tick: 0x0102_0304_0506,
            core_state: 2,
            last_input_seq: 77,
            input_t_ms: 1234.5,
            ownship: &o,
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

        assert_eq!(f64_at(b, s::O_POS_BF), 1.5);
        assert_eq!(f64_at(b, s::O_POS_BF + 8), -6000.25);
        assert_eq!(f64_at(b, s::O_POS_BF + 16), 2100.125);
        assert_eq!(f32_at(b, s::O_QUAT), 0.5);
        assert_eq!(f32_at(b, s::O_QUAT + 8), -0.5);
        assert_eq!(f32_at(b, s::O_VEL_BF + 4), 200.0);
        for (at, v) in [
            (s::O_TAS, 200.5),
            (s::O_CAS, 180.25),
            (s::O_MACH, 0.6),
            (s::O_ALPHA, 0.05),
            (s::O_BETA, -0.01),
            (s::O_NZ, 1.25),
            (s::O_THROTTLE, 0.5),
            (s::O_THRUST, 41_000.0),
            (s::O_FUEL, 1860.0),
            (s::O_MASS, 10_260.0),
            (s::O_OMEGA + 4, -0.2),
            (s::O_SURFACES, -0.04),
            (s::O_SURFACES + 8, -0.01),
            (s::O_ALTITUDE, 2900.5),
        ] {
            assert_eq!(f32_at(b, at), v as f32, "offset {at}");
        }
        assert_eq!(b[s::O_AB_STAGE], 3);
        assert_eq!(
            u32::from(b[s::O_FLAGS]),
            (ownship_flag::STALL_WARN | ownship_flag::FLAMEOUT) as u32
        );
        assert!(
            b[s::O_FLAGS + 1..s::B_X].iter().all(|&x| x == 0),
            "reserved bytes are zero"
        );

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
        let (mut buf, o, probe) = sample();
        let src = SnapshotSource {
            tick: 5,
            core_state: 2,
            last_input_seq: 0,
            input_t_ms: 0.0,
            ownship: &o,
            probe: &probe,
        };
        buf.write(&src);
        let first = buf.bytes().to_vec();
        buf.write(&src);
        assert_eq!(first, buf.bytes());
    }
}
