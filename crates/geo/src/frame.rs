//! The battlefield frame on the WGS84 ellipsoid (GDD 7.2.3, 7.2.4, 7.4).

use simmath::dmath;
use simmath::math::{DMat3, DVec3};

use crate::geodetic::{Geodetic, ecef_to_geodetic, geodetic_to_ecef};
use crate::wgs84::{A, B};

/// The battlefield frame (BF): East-North-Up at an anchor on the ellipsoid, curved (GDD 7.2.3).
///
/// `p_BF = R (p_ECEF − A)` and `p_ECEF = A + Rᵀ p_BF`, where `A` is the anchor's ECEF position
/// (height 0) and `R`'s rows are the East, North and Up unit vectors at the anchor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wgs84Frame {
    /// The anchor; its height is always 0.
    pub anchor: Geodetic,
    /// The anchor in ECEF, m.
    pub a_ecef: DVec3,
    /// Rows e, n, u at the anchor: `p_bf = rot · (p_ecef − a_ecef)`.
    pub rot: DMat3,
}

impl Wgs84Frame {
    /// The frame anchored at (`lat_rad`, `lon_rad`, 0).
    #[must_use]
    pub fn new(lat_rad: f64, lon_rad: f64) -> Self {
        let anchor = Geodetic::new(lat_rad, lon_rad, 0.0);
        let (sp, cp) = (dmath::sin(lat_rad), dmath::cos(lat_rad));
        let (sl, cl) = (dmath::sin(lon_rad), dmath::cos(lon_rad));
        let rot = DMat3::from_rows([
            [-sl, cl, 0.0],
            [-sp * cl, -sp * sl, cp],
            [cp * cl, cp * sl, sp],
        ]);
        Self {
            anchor,
            a_ecef: geodetic_to_ecef(anchor),
            rot,
        }
    }

    /// A BF position to ECEF: `A + Rᵀ p`.
    #[must_use]
    pub fn bf_to_ecef(&self, p_bf: DVec3) -> DVec3 {
        self.a_ecef + self.rot.transpose_mul_vec(p_bf)
    }

    /// An ECEF position to BF: `R (q − A)`.
    #[must_use]
    pub fn ecef_to_bf(&self, q_ecef: DVec3) -> DVec3 {
        self.rot.mul_vec(q_ecef - self.a_ecef)
    }

    /// A BF direction (or velocity) to ECEF: `Rᵀ d`.
    #[must_use]
    pub fn dir_bf_to_ecef(&self, d_bf: DVec3) -> DVec3 {
        self.rot.transpose_mul_vec(d_bf)
    }

    /// An ECEF direction (or velocity) to BF: `R d`.
    #[must_use]
    pub fn dir_ecef_to_bf(&self, d_ecef: DVec3) -> DVec3 {
        self.rot.mul_vec(d_ecef)
    }

    /// A BF position to geodetic (GDD 7.2.2 after `bf_to_ecef`).
    #[must_use]
    pub fn bf_to_geo(&self, p_bf: DVec3) -> Geodetic {
        ecef_to_geodetic(self.bf_to_ecef(p_bf))
    }

    /// A geodetic position to BF.
    #[must_use]
    pub fn geo_to_bf(&self, g: Geodetic) -> DVec3 {
        self.ecef_to_bf(geodetic_to_ecef(g))
    }

    /// The ellipsoid normal ("up") at a BF position, in BF (GDD 7.2.4):
    /// `R · normalize(X / a², Y / a², Z / b²)`.
    #[must_use]
    pub fn up_bf(&self, p_bf: DVec3) -> DVec3 {
        let q = self.bf_to_ecef(p_bf);
        let n = DVec3::new(q.x / (A * A), q.y / (A * A), q.z / (B * B));
        self.dir_ecef_to_bf(n.normalized_or(DVec3::Z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::angles::RAD;

    #[test]
    fn the_anchor_is_the_origin_and_up_is_z() {
        let f = Wgs84Frame::new(46.55 * RAD, 8.0 * RAD);
        let o = f.geo_to_bf(f.anchor);
        assert!(o.length() < 1e-6, "{o:?}");
        let up = f.up_bf(DVec3::ZERO);
        assert!((up - DVec3::Z).length() < 1e-12, "{up:?}");
    }

    #[test]
    fn east_and_north_point_where_they_should() {
        let f = Wgs84Frame::new(0.0, 0.0);
        // 1 km north of the anchor along the meridian has y > 0 and x ≈ 0.
        let n = f.geo_to_bf(Geodetic::new(1_000.0 / 6_378_137.0, 0.0, 0.0));
        assert!(n.y > 990.0 && n.x.abs() < 1e-6, "{n:?}");
        let e = f.geo_to_bf(Geodetic::new(0.0, 1_000.0 / 6_378_137.0, 0.0));
        assert!(e.x > 990.0 && e.y.abs() < 1e-6, "{e:?}");
    }

    #[test]
    fn directions_round_trip() {
        let f = Wgs84Frame::new(-33.0 * RAD, 151.0 * RAD);
        let d = DVec3::new(0.3, -0.5, 0.8);
        let back = f.dir_ecef_to_bf(f.dir_bf_to_ecef(d));
        assert!((back - d).length() < 1e-15);
    }
}
