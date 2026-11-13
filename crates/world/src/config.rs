//! The world configuration block (ABI v6, ADR-52): what the host writes into the core's staging
//! buffer before `fg_world_apply`. Little-endian, 1 688 bytes; the layout constants are the source
//! of `sim_core::layout`'s `world_config` group.

use crate::boundary::Policy;
use crate::shape::{LatLonRad, MAX_AUTHORED_VERTICES, Shape};

/// The block's size, bytes. // DATA: ADR-52, Appendix A of the v0.2a plan
pub const SIZE: usize = 1_688;
/// The block's version.
pub const VERSION: u32 = 1;
/// Cells a block can list. // DATA: ADR-52
pub const MAX_COVERAGE: usize = 64;

/// Byte offsets of the fields.
#[allow(missing_docs)]
pub mod off {
    pub const VERSION: usize = 0;
    pub const SHAPE_KIND: usize = 4;
    pub const P0_LAT: usize = 8;
    pub const P0_LON: usize = 16;
    pub const P1_LAT: usize = 24;
    pub const P1_LON: usize = 32;
    pub const SIZE_A: usize = 40;
    pub const SIZE_B: usize = 48;
    pub const ROTATION: usize = 56;
    pub const VERTEX_COUNT: usize = 64;
    pub const POLICY: usize = 68;
    pub const CEILING: usize = 72;
    pub const WARN_WIDTH: usize = 80;
    pub const COUNTDOWN: usize = 88;
    pub const SPAWN_LAT: usize = 96;
    pub const SPAWN_LON: usize = 104;
    pub const SPAWN_ALT: usize = 112;
    pub const SPAWN_HEADING: usize = 120;
    pub const SPAWN_TAS: usize = 128;
    pub const UTC_START: usize = 136;
    pub const SPAWN_LEVEL: usize = 144;
    pub const COVERAGE_COUNT: usize = 148;
    pub const COVERAGE: usize = 152;
    pub const COVERAGE_STRIDE: usize = 8;
    pub const VERTICES: usize = 664;
    pub const VERTEX_STRIDE: usize = 16;
}

/// Shape kinds.
#[allow(missing_docs)]
pub mod shape_kind {
    pub const RECT: u32 = 0;
    pub const CIRCLE: u32 = 1;
    pub const ELLIPSE: u32 = 2;
    pub const CAPSULE: u32 = 3;
    pub const POLYGON: u32 = 4;
}

/// The earliest world clock, s since 1970 (1950-01-01T00:00:00Z). // DATA: GDD 7.6
pub const UTC_MIN_S: f64 = -631_152_000.0;
/// The latest world clock, s since 1970 (2100-12-31T23:59:59Z). // DATA: GDD 7.6
pub const UTC_MAX_S: f64 = 4_133_980_799.0;

/// One cell of the datapack's coverage: the z5 tile and the finest level its archive holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CellCoverage {
    /// The z5 column, 0 to 63.
    pub x: u16,
    /// The z5 row, 0 to 31.
    pub y: u16,
    /// The finest level the cell's archive holds (10 or 12 in v0.2a).
    pub max_level: u8,
}

/// Why a block was refused (`world_status::BAD_CONFIG`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// Fewer than 1 688 bytes.
    TooShort,
    /// The version is not 1.
    BadVersion,
    /// The shape kind is not 0 to 4.
    BadShapeKind,
    /// A polygon without 3 to 64 vertices.
    BadVertexCount,
    /// The policy is not 0 to 2.
    BadPolicy,
    /// More than 64 cells, or a cell outside the z5 grid or past z13.
    BadCoverage,
    /// A number is NaN or infinite.
    NotFinite,
    /// A number is out of its range (a size, a time, a latitude, the spawn level).
    OutOfRange,
}

/// The decoded world configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldConfig {
    /// The battlefield's shape.
    pub shape: Shape,
    /// The enforcement policy.
    pub policy: Policy,
    /// The hard ceiling, m MSL.
    pub ceiling_m: f64,
    /// The warning band, m (0 = computed, GDD 8.5).
    pub warn_width_m: f64,
    /// The countdown, s.
    pub countdown_s: f64,
    /// The air spawn's position.
    pub spawn: LatLonRad,
    /// The air spawn's altitude, m MSL.
    pub spawn_alt_msl_m: f64,
    /// The air spawn's true heading, clockwise from north, rad.
    pub spawn_heading_rad: f64,
    /// The air spawn's true airspeed, m/s.
    pub spawn_tas_mps: f64,
    /// The world clock at tick 0, s since 1970-01-01T00:00:00Z (clamped to 1950 to 2100).
    pub utc_start_s: f64,
    /// The level wanted under the spawn (10 or 12).
    pub spawn_level: u8,
    /// The cells with data; a cell not listed is ocean.
    pub coverage: [CellCoverage; MAX_COVERAGE],
    /// How many of `coverage` are used.
    pub coverage_count: usize,
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn f64_at(b: &[u8], o: usize) -> f64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    f64::from_le_bytes(a)
}

/// `x` if finite.
fn finite(x: f64) -> Result<f64, ConfigError> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(ConfigError::NotFinite)
    }
}

/// `x` if finite and within [`lo`, `hi`].
fn ranged(x: f64, lo: f64, hi: f64) -> Result<f64, ConfigError> {
    let x = finite(x)?;
    if (lo..=hi).contains(&x) {
        Ok(x)
    } else {
        Err(ConfigError::OutOfRange)
    }
}

fn lat_lon(b: &[u8], lat_off: usize, lon_off: usize) -> Result<LatLonRad, ConfigError> {
    let half = core::f64::consts::FRAC_PI_2;
    Ok(LatLonRad::new(
        ranged(f64_at(b, lat_off), -half, half)?,
        finite(f64_at(b, lon_off))?,
    ))
}

impl WorldConfig {
    /// Decodes and checks a block.
    ///
    /// # Errors
    /// The first field that is malformed.
    pub fn from_bytes(b: &[u8]) -> Result<Self, ConfigError> {
        if b.len() < SIZE {
            return Err(ConfigError::TooShort);
        }
        if u32_at(b, off::VERSION) != VERSION {
            return Err(ConfigError::BadVersion);
        }
        let shape = decode_shape(b)?;
        let policy = Policy::from_code(u32_at(b, off::POLICY)).ok_or(ConfigError::BadPolicy)?;
        let count = u32_at(b, off::COVERAGE_COUNT) as usize;
        if count > MAX_COVERAGE {
            return Err(ConfigError::BadCoverage);
        }
        let mut coverage = [CellCoverage::default(); MAX_COVERAGE];
        for (k, c) in coverage.iter_mut().take(count).enumerate() {
            let o = off::COVERAGE + k * off::COVERAGE_STRIDE;
            *c = CellCoverage {
                x: u16_at(b, o),
                y: u16_at(b, o + 2),
                max_level: b[o + 4],
            };
            if c.x >= 64 || c.y >= 32 || c.max_level > geo::tile::MAX_LEVEL {
                return Err(ConfigError::BadCoverage);
            }
        }
        let spawn_level = u32_at(b, off::SPAWN_LEVEL);
        if !(5..=u32::from(geo::tile::MAX_LEVEL)).contains(&spawn_level) {
            return Err(ConfigError::OutOfRange);
        }
        Ok(Self {
            shape,
            policy,
            ceiling_m: ranged(f64_at(b, off::CEILING), 1.0, 100_000.0)?,
            warn_width_m: ranged(f64_at(b, off::WARN_WIDTH), 0.0, 1_000_000.0)?,
            countdown_s: ranged(f64_at(b, off::COUNTDOWN), 1.0, 3_600.0)?,
            spawn: lat_lon(b, off::SPAWN_LAT, off::SPAWN_LON)?,
            spawn_alt_msl_m: ranged(f64_at(b, off::SPAWN_ALT), -500.0, 30_000.0)?,
            spawn_heading_rad: finite(f64_at(b, off::SPAWN_HEADING))?,
            spawn_tas_mps: ranged(f64_at(b, off::SPAWN_TAS), 0.0, 1_000.0)?,
            utc_start_s: finite(f64_at(b, off::UTC_START))?.clamp(UTC_MIN_S, UTC_MAX_S),
            spawn_level: spawn_level as u8,
            coverage,
            coverage_count: count,
        })
    }

    /// The listed cells.
    #[must_use]
    pub fn cells(&self) -> &[CellCoverage] {
        &self.coverage[..self.coverage_count]
    }

    /// Encodes the block (for `fg`, the tests and the selftests).
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![0u8; SIZE];
        let put32 = |b: &mut [u8], o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        let put64 = |b: &mut [u8], o: usize, v: f64| b[o..o + 8].copy_from_slice(&v.to_le_bytes());
        put32(&mut b, off::VERSION, VERSION);
        encode_shape(&mut b, &self.shape);
        put32(&mut b, off::POLICY, self.policy as u32);
        put64(&mut b, off::CEILING, self.ceiling_m);
        put64(&mut b, off::WARN_WIDTH, self.warn_width_m);
        put64(&mut b, off::COUNTDOWN, self.countdown_s);
        put64(&mut b, off::SPAWN_LAT, self.spawn.lat);
        put64(&mut b, off::SPAWN_LON, self.spawn.lon);
        put64(&mut b, off::SPAWN_ALT, self.spawn_alt_msl_m);
        put64(&mut b, off::SPAWN_HEADING, self.spawn_heading_rad);
        put64(&mut b, off::SPAWN_TAS, self.spawn_tas_mps);
        put64(&mut b, off::UTC_START, self.utc_start_s);
        put32(&mut b, off::SPAWN_LEVEL, u32::from(self.spawn_level));
        put32(&mut b, off::COVERAGE_COUNT, self.coverage_count as u32);
        for (k, c) in self.cells().iter().enumerate() {
            let o = off::COVERAGE + k * off::COVERAGE_STRIDE;
            b[o..o + 2].copy_from_slice(&c.x.to_le_bytes());
            b[o + 2..o + 4].copy_from_slice(&c.y.to_le_bytes());
            b[o + 4] = c.max_level;
        }
        b
    }
}

fn decode_shape(b: &[u8]) -> Result<Shape, ConfigError> {
    let kind = u32_at(b, off::SHAPE_KIND);
    let positive = |o: usize| ranged(f64_at(b, o), f64::MIN_POSITIVE, 10_000_000.0);
    let rot = || finite(f64_at(b, off::ROTATION));
    Ok(match kind {
        shape_kind::RECT => Shape::Rect {
            center: lat_lon(b, off::P0_LAT, off::P0_LON)?,
            width_m: positive(off::SIZE_A)?,
            height_m: positive(off::SIZE_B)?,
            rotation_rad: rot()?,
        },
        shape_kind::CIRCLE => Shape::Circle {
            center: lat_lon(b, off::P0_LAT, off::P0_LON)?,
            radius_m: positive(off::SIZE_A)?,
        },
        shape_kind::ELLIPSE => Shape::Ellipse {
            center: lat_lon(b, off::P0_LAT, off::P0_LON)?,
            a_m: positive(off::SIZE_A)?,
            b_m: positive(off::SIZE_B)?,
            rotation_rad: rot()?,
        },
        shape_kind::CAPSULE => Shape::Capsule {
            a: lat_lon(b, off::P0_LAT, off::P0_LON)?,
            b: lat_lon(b, off::P1_LAT, off::P1_LON)?,
            radius_m: positive(off::SIZE_A)?,
        },
        shape_kind::POLYGON => {
            let n = u32_at(b, off::VERTEX_COUNT) as usize;
            if !(3..=MAX_AUTHORED_VERTICES).contains(&n) {
                return Err(ConfigError::BadVertexCount);
            }
            let mut verts = [LatLonRad::default(); MAX_AUTHORED_VERTICES];
            for (k, v) in verts.iter_mut().take(n).enumerate() {
                let o = off::VERTICES + k * off::VERTEX_STRIDE;
                *v = lat_lon(b, o, o + 8)?;
            }
            Shape::Polygon {
                verts,
                count: n as u8,
            }
        }
        _ => return Err(ConfigError::BadShapeKind),
    })
}

fn encode_shape(b: &mut [u8], s: &Shape) {
    let put32 = |b: &mut [u8], o: usize, v: u32| b[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let put64 = |b: &mut [u8], o: usize, v: f64| b[o..o + 8].copy_from_slice(&v.to_le_bytes());
    match *s {
        Shape::Rect {
            center,
            width_m,
            height_m,
            rotation_rad,
        } => {
            put32(b, off::SHAPE_KIND, shape_kind::RECT);
            put64(b, off::P0_LAT, center.lat);
            put64(b, off::P0_LON, center.lon);
            put64(b, off::SIZE_A, width_m);
            put64(b, off::SIZE_B, height_m);
            put64(b, off::ROTATION, rotation_rad);
        }
        Shape::Circle { center, radius_m } => {
            put32(b, off::SHAPE_KIND, shape_kind::CIRCLE);
            put64(b, off::P0_LAT, center.lat);
            put64(b, off::P0_LON, center.lon);
            put64(b, off::SIZE_A, radius_m);
        }
        Shape::Ellipse {
            center,
            a_m,
            b_m,
            rotation_rad,
        } => {
            put32(b, off::SHAPE_KIND, shape_kind::ELLIPSE);
            put64(b, off::P0_LAT, center.lat);
            put64(b, off::P0_LON, center.lon);
            put64(b, off::SIZE_A, a_m);
            put64(b, off::SIZE_B, b_m);
            put64(b, off::ROTATION, rotation_rad);
        }
        Shape::Capsule { a, b: bb, radius_m } => {
            put32(b, off::SHAPE_KIND, shape_kind::CAPSULE);
            put64(b, off::P0_LAT, a.lat);
            put64(b, off::P0_LON, a.lon);
            put64(b, off::P1_LAT, bb.lat);
            put64(b, off::P1_LON, bb.lon);
            put64(b, off::SIZE_A, radius_m);
        }
        Shape::Polygon { .. } => {
            put32(b, off::SHAPE_KIND, shape_kind::POLYGON);
            put32(b, off::VERTEX_COUNT, s.vertices().len() as u32);
            for (k, v) in s.vertices().iter().enumerate() {
                let o = off::VERTICES + k * off::VERTEX_STRIDE;
                put64(b, o, v.lat);
                put64(b, o + 8, v.lon);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::angles::RAD;

    pub(crate) fn alps() -> WorldConfig {
        let mut coverage = [CellCoverage::default(); MAX_COVERAGE];
        coverage[0] = CellCoverage {
            x: 33,
            y: 7,
            max_level: 12,
        };
        WorldConfig {
            shape: Shape::Rect {
                center: LatLonRad::new(46.55 * RAD, 8.0 * RAD),
                width_m: 120_000.0,
                height_m: 120_000.0,
                rotation_rad: 0.0,
            },
            policy: Policy::MissionFail,
            ceiling_m: 30_000.0,
            warn_width_m: 0.0,
            countdown_s: 30.0,
            spawn: LatLonRad::new(46.55 * RAD, 8.0 * RAD),
            spawn_alt_msl_m: 5_000.0,
            spawn_heading_rad: 90.0 * RAD,
            spawn_tas_mps: 200.0,
            utc_start_s: 1_782_043_200.0,
            spawn_level: 12,
            coverage,
            coverage_count: 1,
        }
    }

    #[test]
    fn a_block_round_trips() {
        let c = alps();
        let b = c.to_bytes();
        assert_eq!(b.len(), SIZE);
        assert_eq!(WorldConfig::from_bytes(&b), Ok(c));
        assert_eq!(
            off::VERTICES + MAX_AUTHORED_VERTICES * off::VERTEX_STRIDE,
            SIZE
        );
        assert_eq!(
            off::COVERAGE + MAX_COVERAGE * off::COVERAGE_STRIDE,
            off::VERTICES
        );
    }

    #[test]
    fn malformed_blocks_are_refused() {
        let good = alps().to_bytes();
        let with = |o: usize, bytes: &[u8]| {
            let mut b = good.clone();
            b[o..o + bytes.len()].copy_from_slice(bytes);
            WorldConfig::from_bytes(&b)
        };
        assert_eq!(
            WorldConfig::from_bytes(&good[..100]),
            Err(ConfigError::TooShort)
        );
        assert_eq!(
            with(off::VERSION, &2u32.to_le_bytes()),
            Err(ConfigError::BadVersion)
        );
        assert_eq!(
            with(off::SHAPE_KIND, &5u32.to_le_bytes()),
            Err(ConfigError::BadShapeKind)
        );
        assert_eq!(
            with(off::POLICY, &3u32.to_le_bytes()),
            Err(ConfigError::BadPolicy)
        );
        assert_eq!(
            with(off::COVERAGE_COUNT, &65u32.to_le_bytes()),
            Err(ConfigError::BadCoverage)
        );
        assert_eq!(
            with(off::SIZE_A, &f64::NAN.to_le_bytes()),
            Err(ConfigError::NotFinite)
        );
        assert_eq!(
            with(off::SIZE_A, &(-5.0f64).to_le_bytes()),
            Err(ConfigError::OutOfRange)
        );
        assert_eq!(
            with(off::SPAWN_HEADING, &f64::INFINITY.to_le_bytes()),
            Err(ConfigError::NotFinite)
        );
        assert_eq!(
            with(off::SPAWN_LEVEL, &14u32.to_le_bytes()),
            Err(ConfigError::OutOfRange)
        );
        let mut poly = good.clone();
        poly[off::SHAPE_KIND..off::SHAPE_KIND + 4].copy_from_slice(&4u32.to_le_bytes());
        poly[off::VERTEX_COUNT..off::VERTEX_COUNT + 4].copy_from_slice(&2u32.to_le_bytes());
        assert_eq!(
            WorldConfig::from_bytes(&poly),
            Err(ConfigError::BadVertexCount)
        );
        // The world clock clamps.
        let late = with(off::UTC_START, &1e10f64.to_le_bytes()).unwrap();
        assert_eq!(late.utc_start_s, UTC_MAX_S);
    }
}
