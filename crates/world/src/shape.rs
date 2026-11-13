//! The authored battlefield shapes (GDD 8.2) and their boundary polygons in a frame's horizontal
//! plane (ADR-53).
//!
//! Every shape becomes a polygon: a rect 4 vertices, a circle and an ellipse 256, a capsule
//! 2 x 129, a polygon its own (3 to 64). The polygon is built in the provisional frame `F0` at the
//! shape's reference point, then re-expressed in the final frame at the anchor ([`crate::anchor`]).
//! Rotations are clockwise from north: +30° turns a rect's height axis from north towards east.

use simmath::dmath;

use crate::polygon::BfPolygon;
use geo::{Geodetic, Wgs84Frame};

/// A surface point, rad.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct LatLonRad {
    /// Latitude, rad.
    pub lat: f64,
    /// Longitude, rad.
    pub lon: f64,
}

impl LatLonRad {
    /// A point from latitude and longitude, rad.
    #[must_use]
    pub const fn new(lat: f64, lon: f64) -> Self {
        Self { lat, lon }
    }

    /// The point at height 0.
    #[must_use]
    pub const fn ground(self) -> Geodetic {
        Geodetic::new(self.lat, self.lon, 0.0)
    }
}

/// The most vertices of an authored polygon (GDD 8.2). // DATA: GDD 8.2
pub const MAX_AUTHORED_VERTICES: usize = 64;

/// Vertices of a circle or an ellipse. // ALGO: ADR-53, ≤ 4.5 m from the true curve at r = 60 km
pub const ROUND_VERTICES: usize = 256;

/// Vertices of each of a capsule's two end caps. // ALGO: ADR-53, half of 256 plus the join
pub const CAP_VERTICES: usize = 129;

/// An authored battlefield shape.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // ALGO: a fixed array, so applying a world never allocates for it
pub enum Shape {
    /// A rectangle: centre, width (east–west before rotation), height, clockwise rotation.
    Rect {
        /// Centre.
        center: LatLonRad,
        /// Width, m.
        width_m: f64,
        /// Height, m.
        height_m: f64,
        /// Clockwise rotation from north, rad.
        rotation_rad: f64,
    },
    /// A circle.
    Circle {
        /// Centre.
        center: LatLonRad,
        /// Radius, m.
        radius_m: f64,
    },
    /// An ellipse: half-axis `a` east–west and `b` north–south before rotation.
    Ellipse {
        /// Centre.
        center: LatLonRad,
        /// East–west half-axis, m.
        a_m: f64,
        /// North–south half-axis, m.
        b_m: f64,
        /// Clockwise rotation from north, rad.
        rotation_rad: f64,
    },
    /// A capsule (corridor): the points within `radius_m` of the segment `a`–`b`.
    Capsule {
        /// One end.
        a: LatLonRad,
        /// The other end.
        b: LatLonRad,
        /// Half-width, m.
        radius_m: f64,
    },
    /// A simple polygon of 3 to 64 vertices, concave allowed.
    Polygon {
        /// The vertices; only the first `count` are used.
        verts: [LatLonRad; MAX_AUTHORED_VERTICES],
        /// The number of vertices.
        count: u8,
    },
}

/// (x, y) rotated clockwise by `rot` (x east, y north).
fn rotate_cw(x: f64, y: f64, rot: f64) -> (f64, f64) {
    let (s, c) = (dmath::sin(rot), dmath::cos(rot));
    (x * c + y * s, -x * s + y * c)
}

impl Shape {
    /// The shape's area by its own formula (the polygon's shoelace area in `f0` for a polygon), m².
    /// The size class is taken from this, so a 120 km square is exactly Medium.
    #[must_use]
    pub fn area_m2(&self, f0: &Wgs84Frame) -> f64 {
        match *self {
            Self::Rect {
                width_m, height_m, ..
            } => width_m * height_m,
            Self::Circle { radius_m, .. } => core::f64::consts::PI * radius_m * radius_m,
            Self::Ellipse { a_m, b_m, .. } => core::f64::consts::PI * a_m * b_m,
            Self::Capsule { a, b, radius_m } => {
                let (pa, pb) = (f0.geo_to_bf(a.ground()), f0.geo_to_bf(b.ground()));
                let l = ((pb.x - pa.x) * (pb.x - pa.x) + (pb.y - pa.y) * (pb.y - pa.y)).sqrt();
                2.0 * radius_m * l + core::f64::consts::PI * radius_m * radius_m
            }
            Self::Polygon { .. } => self.to_polygon_f0(f0).area_m2(),
        }
    }

    /// The authored vertices (a polygon's, at most 64 even if `count` says more) or none.
    #[must_use]
    pub fn vertices(&self) -> &[LatLonRad] {
        match self {
            Self::Polygon { verts, count } => {
                &verts[..usize::from(*count).min(MAX_AUTHORED_VERTICES)]
            }
            _ => &[],
        }
    }

    /// The number of authored vertices (polygons) or 0.
    #[must_use]
    pub fn authored_vertices(&self) -> usize {
        match *self {
            Self::Polygon { count, .. } => usize::from(count),
            _ => 0,
        }
    }

    /// The boundary polygon in the horizontal plane of `f0` (counter-clockwise).
    #[must_use]
    pub fn to_polygon_f0(&self, f0: &Wgs84Frame) -> BfPolygon {
        let mut p = BfPolygon::new();
        match *self {
            Self::Rect {
                width_m,
                height_m,
                rotation_rad,
                ..
            } => {
                let (hw, hh) = (0.5 * width_m, 0.5 * height_m);
                for (x, y) in [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)] {
                    let (rx, ry) = rotate_cw(x, y, rotation_rad);
                    p.push(rx, ry);
                }
            }
            Self::Circle { radius_m, .. } => {
                for k in 0..ROUND_VERTICES {
                    let t = core::f64::consts::TAU * k as f64 / ROUND_VERTICES as f64;
                    p.push(radius_m * dmath::sin(t), radius_m * dmath::cos(t));
                }
            }
            Self::Ellipse {
                a_m,
                b_m,
                rotation_rad,
                ..
            } => {
                for k in 0..ROUND_VERTICES {
                    let t = core::f64::consts::TAU * k as f64 / ROUND_VERTICES as f64;
                    let (rx, ry) =
                        rotate_cw(a_m * dmath::sin(t), b_m * dmath::cos(t), rotation_rad);
                    p.push(rx, ry);
                }
            }
            Self::Capsule { a, b, radius_m } => capsule_polygon(&mut p, f0, a, b, radius_m),
            Self::Polygon { .. } => {
                for v in self.vertices() {
                    let q = f0.geo_to_bf(v.ground());
                    p.push(q.x, q.y);
                }
            }
        }
        p.make_ccw();
        p
    }
}

/// A capsule's polygon: the cap around `b` from its right side through its tip to its left, then
/// the cap around `a` from its left through its tail to its right (counter-clockwise).
fn capsule_polygon(p: &mut BfPolygon, f0: &Wgs84Frame, a: LatLonRad, b: LatLonRad, r: f64) {
    let (pa, pb) = (f0.geo_to_bf(a.ground()), f0.geo_to_bf(b.ground()));
    let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
    let len = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = if len > 0.0 {
        (dx / len, dy / len)
    } else {
        (1.0, 0.0)
    };
    let (nx, ny) = (-uy, ux); // left of a → b
    let steps = (CAP_VERTICES - 1) as f64;
    for (cx, cy, start) in [
        (pb.x, pb.y, -core::f64::consts::FRAC_PI_2),
        (pa.x, pa.y, core::f64::consts::FRAC_PI_2),
    ] {
        for k in 0..CAP_VERTICES {
            let phi = start + core::f64::consts::PI * k as f64 / steps;
            let (c, s) = (dmath::cos(phi), dmath::sin(phi));
            p.push(cx + r * (c * ux + s * nx), cy + r * (c * uy + s * ny));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::angles::RAD;

    fn f0() -> Wgs84Frame {
        Wgs84Frame::new(46.55 * RAD, 8.0 * RAD)
    }

    #[test]
    fn a_rotated_rect_turns_clockwise() {
        let r = Shape::Rect {
            center: LatLonRad::new(46.55 * RAD, 8.0 * RAD),
            width_m: 0.0,
            height_m: 2.0,
            rotation_rad: 30.0 * RAD,
        };
        // The top-right corner (0, +1) turns towards the east: (sin 30°, cos 30°).
        let p = r.to_polygon_f0(&f0());
        let top = p
            .points()
            .iter()
            .copied()
            .fold(
                [0.0, f64::NEG_INFINITY],
                |m, q| if q[1] > m[1] { q } else { m },
            );
        assert!(
            (top[0] - 0.5).abs() < 1e-12 && (top[1] - 0.75f64.sqrt()).abs() < 1e-12,
            "{top:?}"
        );
    }

    #[test]
    fn vertex_counts_and_orientation() {
        let c = LatLonRad::new(46.55 * RAD, 8.0 * RAD);
        let circle = Shape::Circle {
            center: c,
            radius_m: 60_000.0,
        }
        .to_polygon_f0(&f0());
        assert_eq!(circle.len(), 256);
        assert!(circle.signed_area_m2() > 0.0);
        let rel = (circle.area_m2() - core::f64::consts::PI * 3.6e9).abs()
            / (core::f64::consts::PI * 3.6e9);
        assert!(rel < 1e-3);
        let cap = Shape::Capsule {
            a: LatLonRad::new(46.0 * RAD, 7.0 * RAD),
            b: LatLonRad::new(47.0 * RAD, 9.0 * RAD),
            radius_m: 30_000.0,
        };
        let poly = cap.to_polygon_f0(&f0());
        assert_eq!(poly.len(), 258);
        assert!(poly.signed_area_m2() > 0.0);
        assert!(poly.is_simple());
        let exact = cap.area_m2(&f0());
        assert!((poly.area_m2() - exact).abs() / exact < 1e-3);
    }
}
