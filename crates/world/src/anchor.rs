//! The anchor and the playable area (GDD 8.7, ADR-53).
//!
//! 1. The shape's reference point: the authored centre (rect, circle, ellipse); for a capsule the
//!    surface point under the ECEF mean of its two ends; for a polygon the one under the ECEF mean
//!    of its vertices.
//! 2. The boundary polygon in the provisional frame `F0` at the reference point.
//! 3. Every edge densified to segments of at most 5 000 m; the ECEF positions of all samples summed
//!    in vertex order.
//! 4. The anchor: the ECEF mean projected to the ellipsoid (geodetic of the mean, height 0). Being an
//!    ECEF mean, it is stable for shapes across the antimeridian.
//! 5. The final frame at the anchor; each vertex (not the samples) re-expressed in it.

use simmath::math::DVec3;

use crate::polygon::BfPolygon;
use crate::shape::Shape;
use geo::{Geodetic, Wgs84Frame, ecef_to_geodetic, geodetic_to_ecef};

/// The longest densified segment, m. // ALGO: ADR-53, 5 km keeps the mean within metres
pub const DENSIFY_M: f64 = 5_000.0;

/// A shape placed on the Earth.
#[derive(Clone, Debug)]
pub struct Placed {
    /// The battlefield frame at the anchor.
    pub frame: Wgs84Frame,
    /// The playable area in the frame's horizontal plane, counter-clockwise.
    pub polygon: BfPolygon,
    /// The shape's area by its own formula, m² (for the size class).
    pub area_m2: f64,
}

/// The surface point under the mean of `sum / n` (geodetic latitude and longitude, height 0).
fn surface_under(sum: DVec3, n: usize) -> Geodetic {
    let g = ecef_to_geodetic(sum / n as f64);
    Geodetic::new(g.lat_rad, g.lon_rad, 0.0)
}

/// The shape's reference point (step 1).
fn reference(shape: &Shape) -> Geodetic {
    match *shape {
        Shape::Rect { center, .. }
        | Shape::Circle { center, .. }
        | Shape::Ellipse { center, .. } => center.ground(),
        Shape::Capsule { a, b, .. } => surface_under(
            geodetic_to_ecef(a.ground()) + geodetic_to_ecef(b.ground()),
            2,
        ),
        Shape::Polygon { .. } => {
            let n = shape.vertices().len().max(1);
            let mut sum = DVec3::ZERO;
            for v in shape.vertices() {
                sum += geodetic_to_ecef(v.ground());
            }
            surface_under(sum, n)
        }
    }
}

/// Places `shape`: the anchor, the frame and the playable area (steps 1 to 5).
#[must_use]
pub fn place(shape: &Shape) -> Placed {
    let r = reference(shape);
    let f0 = Wgs84Frame::new(r.lat_rad, r.lon_rad);
    let p0 = shape.to_polygon_f0(&f0);
    let mut sum = DVec3::ZERO;
    let mut samples = 0usize;
    for i in 0..p0.len() {
        let (a, b) = p0.edge(i);
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt();
        let segs = ((len / DENSIFY_M).ceil() as usize).max(1);
        for k in 0..segs {
            let t = k as f64 / segs as f64;
            sum += f0.bf_to_ecef(DVec3::new(a[0] + t * dx, a[1] + t * dy, 0.0));
            samples += 1;
        }
    }
    let anchor = surface_under(sum, samples.max(1));
    let frame = Wgs84Frame::new(anchor.lat_rad, anchor.lon_rad);
    let mut polygon = BfPolygon::new();
    for v in p0.points() {
        let q = frame.ecef_to_bf(f0.bf_to_ecef(DVec3::new(v[0], v[1], 0.0)));
        polygon.push(q.x, q.y);
    }
    polygon.make_ccw();
    Placed {
        frame,
        polygon,
        area_m2: shape.area_m2(&f0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shape::LatLonRad;
    use geo::angles::RAD;

    #[test]
    fn a_rect_is_anchored_at_its_centre() {
        let c = LatLonRad::new(46.55 * RAD, 8.0 * RAD);
        let p = place(&Shape::Rect {
            center: c,
            width_m: 120_000.0,
            height_m: 80_000.0,
            rotation_rad: 30.0 * RAD,
        });
        let d = (geodetic_to_ecef(p.frame.anchor) - geodetic_to_ecef(c.ground())).length();
        assert!(d < 1.0, "{d} m");
        assert_eq!(p.area_m2, 120_000.0 * 80_000.0);
        assert_eq!(p.polygon.len(), 4);
    }
}
