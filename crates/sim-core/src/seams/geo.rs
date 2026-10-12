//! `GeoService`: how the simulation sees the Earth (GDD 7.7, 61.5, ADR-15, ADR-16).
//!
//! The Sim Core never reads the render world; it asks this trait for heights, normals, line of
//! sight and rays. v0.0b ships two procedural implementations behind it: [`ProceduralGeo`] (a
//! sphere with value-noise terrain, so BF really is curved, ADR-04) and [`FlatGeo`]. Both pass
//! [`contract`]. The streamed `GeoPack` implementation replaces them in M2 without touching callers.
//!
//! Everything here is a pure function of its inputs (ADR-15) and uses only IEEE operations and
//! `dmath`, so results are bit-identical across targets (ADR-10).

use crate::dmath;
use crate::math::DVec3;

/// Mean Earth radius for sphere approximations (GDD 0.6.5), metres.
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;
/// Terrain heights are clamped to this range (GDD 7.8), metres.
pub const MIN_TERRAIN_M: f64 = -11_000.0;
/// See [`MIN_TERRAIN_M`].
pub const MAX_TERRAIN_M: f64 = 9_000.0;
/// Anchor latitude of the default battlefield, radians (36.2 N, the GDD 63.2 example).
pub const DEFAULT_ANCHOR_LAT_RAD: f64 = 36.2 * (core::f64::consts::PI / 180.0);
/// Anchor longitude of the default battlefield, radians (121.8 W).
pub const DEFAULT_ANCHOR_LON_RAD: f64 = -121.8 * (core::f64::consts::PI / 180.0);
/// Data version of the procedural terrain. Bump when the height function changes.
pub const TERRAIN_DATA_VERSION: u64 = 1;

const LOS_STEP_M: f64 = 100.0;
const RAY_STEP_M: f64 = 50.0;
const RAY_BISECTIONS: u32 = 24;
/// Half-width of the finite difference used for terrain normals, metres.
const NORMAL_EPS_M: f64 = 10.0;
/// A point this far under the ground is blocked; absorbs floating-point noise at touchdown.
const LOS_TOLERANCE_M: f64 = 0.01;

/// Geodetic position: latitude and longitude in radians, height in metres above mean sea level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geodetic {
    /// Latitude, radians.
    pub lat_rad: f64,
    /// Longitude, radians.
    pub lon_rad: f64,
    /// Height above MSL, metres (ADR-05).
    pub h_m: f64,
}

/// A surface position without height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatLon {
    /// Latitude, radians.
    pub lat_rad: f64,
    /// Longitude, radians.
    pub lon_rad: f64,
}

/// How a height sample was obtained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleQuality {
    /// From source data.
    Source,
    /// Generated procedurally.
    Procedural,
    /// No data; a deterministic fallback was used (never `NaN`, GDD 7.7).
    Missing,
}

/// Terrain height with its provenance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeightSample {
    /// Height above MSL, metres.
    pub h: f32,
    /// Where it came from.
    pub quality: SampleQuality,
}

/// Coarse surface type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceClass {
    /// Open water.
    Water,
    /// Bare or vegetated land.
    Land,
    /// Ice or snow.
    Ice,
    /// Built-up area.
    Urban,
}

/// Result of a line-of-sight query.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LosResult {
    /// True if nothing blocks the segment.
    pub visible: bool,
    /// First blocked sample along the segment from `a`, in BF.
    pub blocker: Option<DVec3>,
}

/// A ray hit on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Hit point in BF.
    pub point: DVec3,
    /// Distance from the ray origin, metres.
    pub distance_m: f64,
}

/// Reference to an airport record (none exist before M2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AirportRef(pub u32);

/// The battlefield frame: anchor point and Earth radius (GDD 0.3.2, 7.2.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BattlefieldFrame {
    /// Anchor latitude, radians.
    pub anchor_lat_rad: f64,
    /// Anchor longitude, radians.
    pub anchor_lon_rad: f64,
    /// Sphere radius, metres.
    pub radius_m: f64,
}

impl BattlefieldFrame {
    /// The frame used by the demo: anchored at 36.2 N, 121.8 W on a 6 371 km sphere.
    pub const DEFAULT: Self = Self {
        anchor_lat_rad: DEFAULT_ANCHOR_LAT_RAD,
        anchor_lon_rad: DEFAULT_ANCHOR_LON_RAD,
        radius_m: EARTH_RADIUS_M,
    };
}

/// The simulation's window on the Earth.
pub trait GeoService: Send {
    /// The battlefield frame.
    fn frame(&self) -> &BattlefieldFrame;
    /// BF position to geodetic.
    fn bf_to_geo(&self, p: DVec3) -> Geodetic;
    /// Geodetic to BF position.
    fn geo_to_bf(&self, g: Geodetic) -> DVec3;
    /// Local "up" (the radial) at a BF position, as a unit vector in BF (GDD 7.2.4).
    fn up_bf(&self, p: DVec3) -> DVec3;
    /// Terrain height at a surface position. A pure function of position and data version.
    fn terrain_height(&self, lat_rad: f64, lon_rad: f64) -> HeightSample;
    /// Unit terrain normal at a surface position, in BF.
    fn terrain_normal(&self, lat_rad: f64, lon_rad: f64) -> DVec3;
    /// Surface type at a surface position.
    fn surface_class(&self, lat_rad: f64, lon_rad: f64) -> SurfaceClass;
    /// Line of sight between two BF points. `k_factor` is the effective-Earth-radius factor
    /// (1.0 visual, 4/3 radar); implementations may ignore it until the LOS service lands (M3).
    fn los(&self, a: DVec3, b: DVec3, k_factor: f64) -> LosResult;
    /// First ground hit along a ray (`dir` need not be unit length), up to `max_m` metres.
    fn raycast(&self, origin: DVec3, dir: DVec3, max_m: f64) -> Option<Hit>;
    /// Nearest airport, if any.
    fn nearest_airport(&self, p: DVec3) -> Option<AirportRef>;
    /// Ground height in the BF vertical axis at horizontal BF position `(x, y)`: terrain height
    /// plus the drop of the curved Earth under the tangent plane. This is what ground collision
    /// uses (an extension of GDD 7.7 that saves every caller from doing the same two steps).
    fn ground_z_bf(&self, x: f64, y: f64) -> f64;

    /// Batch variant of [`GeoService::terrain_height`] for AI and sensors (GDD 7.7 rule 3).
    /// `out` must be at least as long as `pts`; extra entries are left alone.
    fn terrain_height_batch(&self, pts: &[LatLon], out: &mut [HeightSample]) {
        for (p, o) in pts.iter().zip(out.iter_mut()) {
            *o = self.terrain_height(p.lat_rad, p.lon_rad);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Shared marching helpers.

/// Line of sight by sampling the ground every [`LOS_STEP_M`] metres.
fn los_by_marching(g: &dyn GeoService, a: DVec3, b: DVec3) -> LosResult {
    let d = b - a;
    let len = d.length();
    if !(len.is_finite() && a.is_finite() && b.is_finite()) {
        return LosResult {
            visible: false,
            blocker: None,
        };
    }
    let steps = (len / LOS_STEP_M).ceil().max(1.0);
    let n = steps as u32;
    for i in 0..=n {
        let p = a + d * (f64::from(i) / steps);
        if p.z + LOS_TOLERANCE_M < g.ground_z_bf(p.x, p.y) {
            return LosResult {
                visible: false,
                blocker: Some(p),
            };
        }
    }
    LosResult {
        visible: true,
        blocker: None,
    }
}

/// First ground hit along a ray: coarse march, then bisection on the crossing.
fn raycast_by_marching(g: &dyn GeoService, origin: DVec3, dir: DVec3, max_m: f64) -> Option<Hit> {
    let u = dir.normalized_or(DVec3::ZERO);
    if u == DVec3::ZERO || !origin.is_finite() || !(max_m > 0.0 && max_m.is_finite()) {
        return None;
    }
    let below = |t: f64| {
        let p = origin + u * t;
        p.z < g.ground_z_bf(p.x, p.y)
    };
    if below(0.0) {
        return Some(Hit {
            point: origin,
            distance_m: 0.0,
        });
    }
    let steps = (max_m / RAY_STEP_M).ceil().max(1.0);
    let n = steps as u32;
    let mut t_prev = 0.0;
    for i in 1..=n {
        let t = (f64::from(i) * RAY_STEP_M).min(max_m);
        if below(t) {
            let (mut lo, mut hi) = (t_prev, t);
            for _ in 0..RAY_BISECTIONS {
                let mid = f64::midpoint(lo, hi);
                if below(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(Hit {
                point: origin + u * hi,
                distance_m: hi,
            });
        }
        t_prev = t;
    }
    None
}

// ---------------------------------------------------------------------------------------------
// Procedural value-noise terrain.

/// Lattice value in `[0, 1)` from integer cell coordinates (integer ops only).
fn lattice(seed: u64, ix: i64, iy: i64) -> f64 {
    let mut z = seed
        ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn value_noise(seed: u64, x: f64, y: f64) -> f64 {
    let (fx, fy) = (x.floor(), y.floor());
    let (tx, ty) = (smooth(x - fx), smooth(y - fy));
    let (ix, iy) = (fx as i64, fy as i64);
    let v00 = lattice(seed, ix, iy);
    let v10 = lattice(seed, ix + 1, iy);
    let v01 = lattice(seed, ix, iy + 1);
    let v11 = lattice(seed, ix + 1, iy + 1);
    let top = v00 + (v10 - v00) * tx;
    let bottom = v01 + (v11 - v01) * tx;
    top + (bottom - top) * ty
}

const BASE_WAVELENGTH_M: f64 = 12_000.0;
const OCTAVES: u64 = 5;

/// Fractal noise in `[0, 1)` at metre coordinates `(u, v)`.
fn fbm(seed: u64, u: f64, v: f64) -> f64 {
    let (mut amp, mut freq, mut sum, mut norm) = (1.0, 1.0 / BASE_WAVELENGTH_M, 0.0, 0.0);
    for octave in 0..OCTAVES {
        sum += amp * value_noise(seed.wrapping_add(octave), u * freq, v * freq);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}

// ---------------------------------------------------------------------------------------------
// Spherical procedural Earth.

/// A sphere of [`EARTH_RADIUS_M`] with ridged value-noise terrain (0 to about 1.4 km).
///
/// BF is the tangent frame at the anchor (East, North, Up), so the ground really falls away
/// from the tangent plane: 20 m at 16 km, 7 km at 300 km (GDD 7.4).
#[derive(Clone, Debug)]
pub struct ProceduralGeo {
    frame: BattlefieldFrame,
    seed: u64,
    anchor_ecef: DVec3,
    east: DVec3,
    north: DVec3,
    up: DVec3,
    cos_lat0: f64,
}

impl ProceduralGeo {
    /// Builds the service for a frame.
    #[must_use]
    pub fn new(frame: BattlefieldFrame) -> Self {
        let (sp, cp) = (
            dmath::sin(frame.anchor_lat_rad),
            dmath::cos(frame.anchor_lat_rad),
        );
        let (sl, cl) = (
            dmath::sin(frame.anchor_lon_rad),
            dmath::cos(frame.anchor_lon_rad),
        );
        let up = DVec3::new(cp * cl, cp * sl, sp);
        Self {
            frame,
            seed: TERRAIN_DATA_VERSION.wrapping_mul(0xA076_1D64_78BD_642F),
            anchor_ecef: up * frame.radius_m,
            east: DVec3::new(-sl, cl, 0.0),
            north: DVec3::new(-sp * cl, -sp * sl, cp),
            up,
            cos_lat0: cp,
        }
    }

    /// The default battlefield.
    #[must_use]
    pub fn default_frame() -> Self {
        Self::new(BattlefieldFrame::DEFAULT)
    }

    fn bf_to_ecef(&self, p: DVec3) -> DVec3 {
        self.anchor_ecef + self.east * p.x + self.north * p.y + self.up * p.z
    }

    fn ecef_to_bf(&self, c: DVec3) -> DVec3 {
        let d = c - self.anchor_ecef;
        DVec3::new(d.dot(self.east), d.dot(self.north), d.dot(self.up))
    }

    fn height_m(&self, lat_rad: f64, lon_rad: f64) -> f64 {
        if !(lat_rad.is_finite() && lon_rad.is_finite()) {
            return 0.0;
        }
        let r = self.frame.radius_m;
        let u = (lon_rad - self.frame.anchor_lon_rad) * r * self.cos_lat0;
        let v = (lat_rad - self.frame.anchor_lat_rad) * r;
        let n = fbm(self.seed, u, v);
        let ridge = 1.0 - (2.0 * n - 1.0).abs();
        (1_300.0 * ridge * ridge + 80.0 * n).clamp(MIN_TERRAIN_M, MAX_TERRAIN_M)
    }

    /// Height of the sphere surface below tangent-plane position `(x, y)`, relative to BF z = 0.
    fn sphere_z(&self, x: f64, y: f64) -> f64 {
        let r = self.frame.radius_m;
        let rem = r * r - x * x - y * y;
        // Beyond the horizon of the sphere there is no surface "below"; stay finite.
        dmath::sqrt(rem.max(0.0)) - r
    }
}

impl GeoService for ProceduralGeo {
    fn frame(&self) -> &BattlefieldFrame {
        &self.frame
    }

    fn bf_to_geo(&self, p: DVec3) -> Geodetic {
        let c = self.bf_to_ecef(p);
        let r = c.length();
        if r == 0.0 || !r.is_finite() {
            return Geodetic {
                lat_rad: self.frame.anchor_lat_rad,
                lon_rad: self.frame.anchor_lon_rad,
                h_m: -self.frame.radius_m,
            };
        }
        Geodetic {
            lat_rad: dmath::asin((c.z / r).clamp(-1.0, 1.0)),
            lon_rad: dmath::atan2(c.y, c.x),
            h_m: r - self.frame.radius_m,
        }
    }

    fn geo_to_bf(&self, g: Geodetic) -> DVec3 {
        let r = self.frame.radius_m + g.h_m;
        let cp = dmath::cos(g.lat_rad);
        let c = DVec3::new(
            r * cp * dmath::cos(g.lon_rad),
            r * cp * dmath::sin(g.lon_rad),
            r * dmath::sin(g.lat_rad),
        );
        self.ecef_to_bf(c)
    }

    fn up_bf(&self, p: DVec3) -> DVec3 {
        let radial = self.bf_to_ecef(p).normalized_or(self.up);
        let u = DVec3::new(
            radial.dot(self.east),
            radial.dot(self.north),
            radial.dot(self.up),
        );
        u.normalized_or(DVec3::Z)
    }

    fn terrain_height(&self, lat_rad: f64, lon_rad: f64) -> HeightSample {
        HeightSample {
            h: self.height_m(lat_rad, lon_rad) as f32,
            quality: SampleQuality::Procedural,
        }
    }

    fn terrain_normal(&self, lat_rad: f64, lon_rad: f64) -> DVec3 {
        let p = self.geo_to_bf(Geodetic {
            lat_rad,
            lon_rad,
            h_m: 0.0,
        });
        let dzdx = (self.ground_z_bf(p.x + NORMAL_EPS_M, p.y)
            - self.ground_z_bf(p.x - NORMAL_EPS_M, p.y))
            / (2.0 * NORMAL_EPS_M);
        let dzdy = (self.ground_z_bf(p.x, p.y + NORMAL_EPS_M)
            - self.ground_z_bf(p.x, p.y - NORMAL_EPS_M))
            / (2.0 * NORMAL_EPS_M);
        DVec3::new(-dzdx, -dzdy, 1.0).normalized_or(DVec3::Z)
    }

    fn surface_class(&self, lat_rad: f64, lon_rad: f64) -> SurfaceClass {
        if self.height_m(lat_rad, lon_rad) > 1_250.0 {
            SurfaceClass::Ice
        } else {
            SurfaceClass::Land
        }
    }

    fn los(&self, a: DVec3, b: DVec3, _k_factor: f64) -> LosResult {
        los_by_marching(self, a, b)
    }

    fn raycast(&self, origin: DVec3, dir: DVec3, max_m: f64) -> Option<Hit> {
        raycast_by_marching(self, origin, dir, max_m)
    }

    fn nearest_airport(&self, _p: DVec3) -> Option<AirportRef> {
        None
    }

    fn ground_z_bf(&self, x: f64, y: f64) -> f64 {
        let zs = self.sphere_z(x, y);
        let g = self.bf_to_geo(DVec3::new(x, y, zs));
        zs + self.height_m(g.lat_rad, g.lon_rad)
    }
}

// ---------------------------------------------------------------------------------------------
// Flat plane.

/// A flat world: BF z = 0 is the ground everywhere, no curvature. Useful in tests that must not
/// depend on terrain, and the second implementation that keeps the contract honest.
#[derive(Clone, Debug)]
pub struct FlatGeo {
    frame: BattlefieldFrame,
    cos_lat0: f64,
}

impl FlatGeo {
    /// A flat world around the default anchor.
    #[must_use]
    pub fn new() -> Self {
        let frame = BattlefieldFrame::DEFAULT;
        Self {
            frame,
            cos_lat0: dmath::cos(frame.anchor_lat_rad),
        }
    }
}

impl Default for FlatGeo {
    fn default() -> Self {
        Self::new()
    }
}

impl GeoService for FlatGeo {
    fn frame(&self) -> &BattlefieldFrame {
        &self.frame
    }

    fn bf_to_geo(&self, p: DVec3) -> Geodetic {
        let r = self.frame.radius_m;
        Geodetic {
            lat_rad: self.frame.anchor_lat_rad + p.y / r,
            lon_rad: self.frame.anchor_lon_rad + p.x / (r * self.cos_lat0),
            h_m: p.z,
        }
    }

    fn geo_to_bf(&self, g: Geodetic) -> DVec3 {
        let r = self.frame.radius_m;
        DVec3::new(
            (g.lon_rad - self.frame.anchor_lon_rad) * r * self.cos_lat0,
            (g.lat_rad - self.frame.anchor_lat_rad) * r,
            g.h_m,
        )
    }

    fn up_bf(&self, _p: DVec3) -> DVec3 {
        DVec3::Z
    }

    fn terrain_height(&self, _lat_rad: f64, _lon_rad: f64) -> HeightSample {
        HeightSample {
            h: 0.0,
            quality: SampleQuality::Procedural,
        }
    }

    fn terrain_normal(&self, _lat_rad: f64, _lon_rad: f64) -> DVec3 {
        DVec3::Z
    }

    fn surface_class(&self, _lat_rad: f64, _lon_rad: f64) -> SurfaceClass {
        SurfaceClass::Land
    }

    fn los(&self, a: DVec3, b: DVec3, _k_factor: f64) -> LosResult {
        los_by_marching(self, a, b)
    }

    fn raycast(&self, origin: DVec3, dir: DVec3, max_m: f64) -> Option<Hit> {
        raycast_by_marching(self, origin, dir, max_m)
    }

    fn nearest_airport(&self, _p: DVec3) -> Option<AirportRef> {
        None
    }

    fn ground_z_bf(&self, _x: f64, _y: f64) -> f64 {
        0.0
    }
}

// ---------------------------------------------------------------------------------------------
// Contract.

/// Behaviour every `GeoService` must have (`AC-61.5`). Call it from an implementation's tests.
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(g: &dyn GeoService) {
    assert!(g.frame().radius_m > 1.0e6, "radius is a planetary radius");
    contract_frames(g);
    contract_terrain(g);
    contract_los_and_rays(g);
}

/// Conversions between BF and geodetic coordinates, and the local vertical.
fn contract_frames(g: &dyn GeoService) {
    let frame = *g.frame();
    for (x, y, z) in [
        (0.0, 0.0, 0.0),
        (1_500.0, -2_500.0, 800.0),
        (-120_000.0, 45_000.0, 9_000.0),
        (250_000.0, 250_000.0, 3_000.0),
        (-400_000.0, 10_000.0, 12_000.0),
    ] {
        let p = DVec3::new(x, y, z);
        let back = g.geo_to_bf(g.bf_to_geo(p));
        assert!(
            (back - p).length() < 1.0e-6,
            "bf -> geo -> bf drifted {} m at {p:?}",
            (back - p).length()
        );
        let up = g.up_bf(p);
        assert!(
            (up.length() - 1.0).abs() < 1.0e-12,
            "up_bf is a unit vector at {p:?}"
        );
        assert!(up.z > 0.5, "up has a positive vertical component at {p:?}");
    }
    let geo = Geodetic {
        lat_rad: frame.anchor_lat_rad + 0.01,
        lon_rad: frame.anchor_lon_rad - 0.02,
        h_m: 1_234.5,
    };
    let there_and_back = g.bf_to_geo(g.geo_to_bf(geo));
    assert!(
        (there_and_back.lat_rad - geo.lat_rad).abs() < 1.0e-12,
        "latitude round trip"
    );
    assert!(
        (there_and_back.lon_rad - geo.lon_rad).abs() < 1.0e-12,
        "longitude round trip"
    );
    assert!(
        (there_and_back.h_m - geo.h_m).abs() < 1.0e-6,
        "height round trip"
    );
}

/// Heights, normals, surface class and ground height: finite, in range, repeatable.
fn contract_terrain(g: &dyn GeoService) {
    let frame = *g.frame();
    let probes = [
        LatLon {
            lat_rad: frame.anchor_lat_rad,
            lon_rad: frame.anchor_lon_rad,
        },
        LatLon {
            lat_rad: frame.anchor_lat_rad + 0.003,
            lon_rad: frame.anchor_lon_rad + 0.004,
        },
        LatLon {
            lat_rad: -1.5,
            lon_rad: 3.1,
        },
        LatLon {
            lat_rad: 1.5,
            lon_rad: -3.1,
        },
        LatLon {
            lat_rad: f64::NAN,
            lon_rad: 0.0,
        },
        LatLon {
            lat_rad: 0.0,
            lon_rad: f64::INFINITY,
        },
    ];
    let mut batch = [HeightSample {
        h: -1.0,
        quality: SampleQuality::Missing,
    }; 6];
    g.terrain_height_batch(&probes, &mut batch);
    for (p, b) in probes.iter().zip(batch) {
        let s = g.terrain_height(p.lat_rad, p.lon_rad);
        assert!(s.h.is_finite(), "terrain height finite at {p:?}");
        assert!(
            (MIN_TERRAIN_M as f32..=MAX_TERRAIN_M as f32).contains(&s.h),
            "terrain height in range at {p:?}"
        );
        assert_eq!(
            s,
            g.terrain_height(p.lat_rad, p.lon_rad),
            "height is repeatable"
        );
        assert_eq!(s, b, "batch equals single at {p:?}");
    }

    let n = g.terrain_normal(frame.anchor_lat_rad + 0.001, frame.anchor_lon_rad + 0.001);
    assert!(
        (n.length() - 1.0).abs() < 1.0e-12,
        "normal is a unit vector"
    );
    assert!(n.z > 0.0, "normal points up");
    let _ = g.surface_class(frame.anchor_lat_rad, frame.anchor_lon_rad);

    for (x, y) in [(0.0, 0.0), (5_000.0, -7_000.0), (-30_000.0, 30_000.0)] {
        let z = g.ground_z_bf(x, y);
        assert!(z.is_finite(), "ground_z finite at ({x}, {y})");
        assert_eq!(
            z.to_bits(),
            g.ground_z_bf(x, y).to_bits(),
            "ground_z repeatable"
        );
    }
}

/// Line of sight and ray casting.
fn contract_los_and_rays(g: &dyn GeoService) {
    let high_a = DVec3::new(-3_000.0, -3_000.0, 5_000.0);
    let high_b = DVec3::new(4_000.0, 6_000.0, 5_200.0);
    assert!(
        g.los(high_a, high_b, 1.0).visible,
        "high points see each other"
    );
    assert_eq!(
        g.los(high_a, high_b, 1.0).visible,
        g.los(high_b, high_a, 1.0).visible,
        "los is symmetric"
    );
    assert!(g.los(high_a, high_a, 1.0).visible, "a point sees itself");
    let buried = DVec3::new(0.0, 0.0, g.ground_z_bf(0.0, 0.0) - 50.0);
    let blocked = g.los(high_a, buried, 1.0);
    assert!(
        !blocked.visible && blocked.blocker.is_some(),
        "buried point is blocked"
    );

    let origin = DVec3::new(1_000.0, 2_000.0, 6_000.0);
    let hit = g
        .raycast(origin, DVec3::new(0.0, 0.0, -3.0), 20_000.0)
        .expect("a ray pointing down hits the ground");
    let ground = g.ground_z_bf(1_000.0, 2_000.0);
    assert!(
        (hit.point.z - ground).abs() < 1.0e-3,
        "hit is on the ground"
    );
    assert!(
        (hit.distance_m - (origin.z - ground)).abs() < 1.0e-3,
        "hit distance matches"
    );
    assert!(
        g.raycast(origin, DVec3::new(0.0, 0.0, -1.0), 100.0)
            .is_none(),
        "ray too short to reach the ground misses"
    );
    assert!(
        g.raycast(origin, DVec3::ZERO, 1_000.0).is_none(),
        "zero direction misses"
    );
    let _ = g.nearest_airport(origin);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn procedural_geo_passes_the_contract() {
        contract(&ProceduralGeo::default_frame());
    }

    #[test]
    fn flat_geo_passes_the_contract() {
        contract(&FlatGeo::new());
    }

    #[test]
    fn terrain_is_a_pure_function_of_position() {
        let a = ProceduralGeo::default_frame();
        let b = ProceduralGeo::default_frame();
        for i in 0..50 {
            let lat = DEFAULT_ANCHOR_LAT_RAD + f64::from(i) * 1.7e-4;
            let lon = DEFAULT_ANCHOR_LON_RAD - f64::from(i) * 2.3e-4;
            assert_eq!(a.terrain_height(lat, lon), b.terrain_height(lat, lon));
        }
    }

    #[test]
    fn terrain_has_relief_but_stays_plausible() {
        let g = ProceduralGeo::default_frame();
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..40 {
            for j in 0..40 {
                let p = g.bf_to_geo(DVec3::new(
                    f64::from(i) * 800.0 - 16_000.0,
                    f64::from(j) * 800.0 - 16_000.0,
                    0.0,
                ));
                let h = g.terrain_height(p.lat_rad, p.lon_rad).h;
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
        assert!(lo >= 0.0 && hi <= 1_500.0, "range {lo}..{hi}");
        assert!(hi - lo > 400.0, "there is relief to look at: {lo}..{hi}");
    }

    #[test]
    fn the_ground_falls_away_with_curvature() {
        // GDD 7.4.1: about 20 m at 16 km. Compare against a flat world with the same terrain
        // contribution removed: sphere drop only.
        let g = ProceduralGeo::default_frame();
        let drop = g.sphere_z(16_000.0, 0.0);
        let expected = -16_000.0 * 16_000.0 / (2.0 * EARTH_RADIUS_M);
        assert!((drop - expected).abs() < 0.05, "drop {drop} vs {expected}");
        // 300 km: 7.06 km (GDD 0.6.4 / ADR-04).
        let far = -g.sphere_z(300_000.0, 0.0);
        assert!((far - 7_060.0).abs() < 60.0, "300 km drop {far}");
    }

    #[test]
    fn round_trip_over_many_random_points_is_micrometre_exact() {
        // AC-7.2 shape: BF -> geo -> BF within 1 micrometre.
        let g = ProceduralGeo::default_frame();
        let mut rng = crate::rng::Xoshiro256StarStar::from_seed(7);
        for _ in 0..2_000 {
            let p = DVec3::new(
                rng.range_f64(-500_000.0, 500_000.0),
                rng.range_f64(-500_000.0, 500_000.0),
                rng.range_f64(-500.0, 30_000.0),
            );
            let back = g.geo_to_bf(g.bf_to_geo(p));
            assert!(
                (back - p).length() < 1.0e-6,
                "{p:?} drifted {}",
                (back - p).length()
            );
        }
    }

    #[test]
    fn up_tilts_away_from_bf_z_as_the_earth_curves() {
        // 300 km from the anchor the local vertical is 2.7 degrees off BF +Z.
        let g = ProceduralGeo::default_frame();
        let up = g.up_bf(DVec3::new(300_000.0, 0.0, 0.0));
        let tilt = dmath::acos(up.z);
        assert!(
            (tilt - 300_000.0 / EARTH_RADIUS_M).abs() < 1.0e-4,
            "tilt {tilt}"
        );
        // East of the anchor the radial leans further east, away from the tangent-plane normal.
        assert!(up.x > 0.0, "up leans outward: {up:?}");
    }

    #[test]
    fn normal_matches_a_finite_difference_of_the_surface() {
        let g = ProceduralGeo::default_frame();
        let p = g.bf_to_geo(DVec3::new(2_000.0, 3_000.0, 0.0));
        let n = g.terrain_normal(p.lat_rad, p.lon_rad);
        // Tangent along +x should be perpendicular to the normal.
        let dz = (g.ground_z_bf(2_001.0, 3_000.0) - g.ground_z_bf(1_999.0, 3_000.0)) / 2.0;
        let tangent_x = DVec3::new(1.0, 0.0, dz).normalized_or(DVec3::X);
        assert!(n.dot(tangent_x).abs() < 1.0e-3, "dot {}", n.dot(tangent_x));
    }

    #[test]
    fn los_is_blocked_by_a_ridge_between_low_points() {
        let g = ProceduralGeo::default_frame();
        // Find the highest sample on a line and put both ends low behind it.
        let a = DVec3::new(-15_000.0, 0.0, 0.0);
        let b = DVec3::new(15_000.0, 0.0, 0.0);
        let a_low = DVec3::new(a.x, a.y, g.ground_z_bf(a.x, a.y) + 2.0);
        let b_low = DVec3::new(b.x, b.y, g.ground_z_bf(b.x, b.y) + 2.0);
        let mid_peak = (0..=300)
            .map(|i| g.ground_z_bf(-15_000.0 + f64::from(i) * 100.0, 0.0))
            .fold(f64::MIN, f64::max);
        let ends = a_low.z.max(b_low.z);
        if mid_peak > ends + 50.0 {
            assert!(!g.los(a_low, b_low, 1.0).visible);
        }
        // And the same two ends raised above every sample can see each other.
        let a_high = DVec3::new(a.x, a.y, mid_peak + 200.0);
        let b_high = DVec3::new(b.x, b.y, mid_peak + 200.0);
        assert!(g.los(a_high, b_high, 1.0).visible);
    }
}
