//! The v0.2a geodesy gates that run natively (ADR-51 section 1): GE2, GE3, GE4, GE7 and GE8's
//! haversine part. GE1 and GE8's table are unit tests (`geodetic.rs`, `distance.rs`); GE5 repeats
//! GE2 and GE3 in WebAssembly through `fg_selftest`; GE6 and GE9 are elsewhere.

use geo::angles::{DEG, RAD};
use geo::distance::{haversine_angle_rad, haversine_m, initial_bearing_rad, inverse};
use geo::testkit::{Rng, ge2_points, ge3_anchor, ge3_point};
use geo::{Accuracy, Wgs84Frame, ecef_to_geodetic, geodetic_to_ecef};
use geographiclib_rs::{Geodesic as Karney, InverseGeodesic};
use simmath::dmath;
use simmath::math::DVec3;

/// The angle between two longitudes or bearings, normalised to (−180°, 180°], degrees.
fn angle_diff_deg(a: f64, b: f64) -> f64 {
    let d = (a - b) % 360.0;
    if d > 180.0 {
        d - 360.0
    } else if d <= -180.0 {
        d + 360.0
    } else {
        d
    }
}

#[test]
fn ge2_a_million_geodetic_round_trips() {
    let (mut dh, mut dlat, mut dlon) = (0.0f64, 0.0f64, 0.0f64);
    for g in ge2_points(1).take(1_000_000) {
        let back = ecef_to_geodetic(geodetic_to_ecef(g));
        dh = dh.max((back.h_m - g.h_m).abs());
        dlat = dlat.max(((back.lat_rad - g.lat_rad) * DEG).abs());
        dlon = dlon.max(angle_diff_deg(back.lon_rad * DEG, g.lon_rad * DEG).abs());
    }
    println!("GE2: max height error {dh:.3e} m, latitude {dlat:.3e}°, longitude {dlon:.3e}°");
    assert!(dh < 1e-6, "height error {dh} m");
    assert!(dlat < 1e-12, "latitude error {dlat}°");
    assert!(dlon < 1e-12, "longitude error {dlon}°");
}

#[test]
fn ge3_a_hundred_thousand_bf_round_trips() {
    let mut r = Rng::new(2);
    let mut worst = 0.0f64;
    for _ in 0..100 {
        let (lat, lon) = ge3_anchor(&mut r);
        let f = Wgs84Frame::new(lat, lon);
        for _ in 0..1_000 {
            let p = ge3_point(&mut r);
            let back = f.geo_to_bf(f.bf_to_geo(p));
            worst = worst.max((back - p).length());
        }
    }
    println!("GE3: max BF round-trip error {worst:.3e} m");
    assert!(worst < 1e-6, "{worst} m");
}

/// The BF height `z` at (`x`, `y`) where the geodetic height is 0, by 80 bisection steps.
fn surface_z(f: &Wgs84Frame, x: f64, y: f64) -> f64 {
    let (mut lo, mut hi) = (-100_000.0, 1_000.0);
    for _ in 0..80 {
        let mid = f64::midpoint(lo, hi);
        if f.bf_to_geo(DVec3::new(x, y, mid)).h_m > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    f64::midpoint(lo, hi)
}

#[test]
fn ge4_the_curvature_table() {
    let f = Wgs84Frame::new(46.55 * RAD, 8.0 * RAD);
    let mut worst = 0.0f64;
    for az in [0.0, 90.0, 180.0, 270.0] {
        for d_km in [10.0, 50.0, 100.0, 300.0, 500.0] {
            let d = d_km * 1_000.0;
            let (x, y) = (d * dmath::sin(az * RAD), d * dmath::cos(az * RAD));
            let drop = -surface_z(&f, x, y);
            let expect = d * d / (2.0 * 6_371_000.0);
            let rel = (drop - expect).abs() / expect;
            println!(
                "GE4: azimuth {az:>5}°, {d_km:>5} km: drop {drop:>10.3} m, d²/2R {expect:>10.3} m, {:.3} %",
                rel * 100.0
            );
            worst = worst.max(rel);
            assert!(rel < 0.005, "{az}° {d_km} km: {drop} against {expect}");
        }
    }
    println!("GE4: worst {:.3} %", worst * 100.0);
}

/// A point uniform on the sphere (lat, lon), rad.
fn uniform_point(r: &mut Rng) -> (f64, f64) {
    (dmath::asin(r.uniform(-1.0, 1.0)), r.lon())
}

#[test]
fn ge7_vincenty_against_geographiclib() {
    let karney = Karney::wgs84();
    let mut r = Rng::new(3);
    let (mut n, mut worst_m, mut worst_deg) = (0, 0.0f64, 0.0f64);
    while n < 10_000 {
        let (lat1, lon1) = uniform_point(&mut r);
        let (lat2, lon2) = uniform_point(&mut r);
        if haversine_angle_rad(lat1, lon1, lat2, lon2) >= 179.0 * RAD {
            continue;
        }
        n += 1;
        let g = inverse(lat1, lon1, lat2, lon2);
        let (s12, azi1, _azi2, _a12): (f64, f64, f64, f64) =
            karney.inverse(lat1 * DEG, lon1 * DEG, lat2 * DEG, lon2 * DEG);
        assert_eq!(
            g.accuracy,
            Accuracy::Ellipsoid,
            "pair {n} fell back to the sphere"
        );
        let dm = (g.distance_m - s12).abs();
        let da = angle_diff_deg(g.bearing_a_rad * DEG, azi1).abs();
        worst_m = worst_m.max(dm);
        worst_deg = worst_deg.max(da);
        assert!(dm < 1e-3, "pair {n}: {} m against {s12} m", g.distance_m);
        assert!(
            da < 1e-6,
            "pair {n}: bearing {}° against {azi1}°",
            g.bearing_a_rad * DEG
        );
    }
    println!("GE7: 10 000 pairs, worst distance {worst_m:.3e} m, worst bearing {worst_deg:.3e}°");

    // Pairs within 0.5° of antipodal: the flagged spherical answer, within 0.6 %.
    let mut m = 0;
    while m < 1_000 {
        let (lat1, lon1) = uniform_point(&mut r);
        let (lat2, lon2) = (
            -lat1 + r.uniform(-0.3, 0.3) * RAD,
            lon1 + core::f64::consts::PI + r.uniform(-0.3, 0.3) * RAD,
        );
        if haversine_angle_rad(lat1, lon1, lat2, lon2) < 179.5 * RAD {
            continue;
        }
        m += 1;
        let g = inverse(lat1, lon1, lat2, geo::angles::normalize_lon(lon2));
        let (s12, _, _, _): (f64, f64, f64, f64) =
            karney.inverse(lat1 * DEG, lon1 * DEG, lat2 * DEG, lon2 * DEG);
        assert_eq!(g.accuracy, Accuracy::Spherical);
        assert!(
            (g.distance_m - s12).abs() / s12 < 0.006,
            "{} against {s12}",
            g.distance_m
        );
    }
}

#[test]
fn ge8_haversine_and_bearing_against_their_formulas() {
    let mut r = Rng::new(8);
    for _ in 0..1_000 {
        let (p1, l1) = uniform_point(&mut r);
        let (p2, l2) = uniform_point(&mut r);
        // GDD 7.3, written out again: a = sin²(Δφ/2) + cos φ1 cos φ2 sin²(Δλ/2), d = 2R asin(√a).
        let s1 = dmath::sin((p2 - p1) / 2.0);
        let s2 = dmath::sin((l2 - l1) / 2.0);
        let a = s1 * s1 + dmath::cos(p1) * dmath::cos(p2) * s2 * s2;
        let d = 2.0 * 6_371_000.0 * dmath::asin(a.min(1.0).sqrt());
        let h = haversine_m(p1, l1, p2, l2);
        assert!((h - d).abs() <= 1e-9 * d.max(1.0), "{h} against {d}");
        // θ = atan2(sin Δλ cos φ2, cos φ1 sin φ2 − sin φ1 cos φ2 cos Δλ).
        let dl = l2 - l1;
        let th = dmath::atan2(
            dmath::sin(dl) * dmath::cos(p2),
            dmath::cos(p1) * dmath::sin(p2) - dmath::sin(p1) * dmath::cos(p2) * dmath::cos(dl),
        );
        let b = initial_bearing_rad(p1, l1, p2, l2);
        assert!(
            angle_diff_deg(b * DEG, th * DEG).abs() < 1e-9,
            "{b} against {th}"
        );
    }
}

#[test]
fn geodetic_and_frame_agree_at_the_poles() {
    // A frame anchored 85° N: a point 1 000 km away still round-trips.
    let f = Wgs84Frame::new(85.0 * RAD, -170.0 * RAD);
    let p = DVec3::new(-700_000.0, 700_000.0, 10_000.0);
    let back = f.geo_to_bf(f.bf_to_geo(p));
    assert!((back - p).length() < 1e-6);
    let g = f.bf_to_geo(DVec3::ZERO);
    assert!((g.lat_rad - 85.0 * RAD).abs() < 1e-14 && g.h_m.abs() < 1e-6);
}
