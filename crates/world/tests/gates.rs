//! The v0.2a battlefield gates that run natively (ADR-51 section 2): BF1, BF4 (timed natively for
//! information; the gate is the WebAssembly time), BF5, BF6 and WS1. BF3, BF7 and BF8 are unit
//! tests (`preset.rs`, `boundary.rs`); BF2 flies in `sim-core` (WP5).

use simmath::dmath;
use simmath::math::DVec3;

use geo::angles::{DEG, RAD};
use geo::tile::TileId;
use geo::{Wgs84Frame, geodetic_to_ecef};
use world::config::CellCoverage;
use world::plan::{PlanInputs, SimTerrainPlan};
use world::polygon::dist2_to_segment;
use world::validate::{
    Reject, ValidateInputs, WarnFlags, check_plan_size, spawn_clearance, validate,
};
use world::{LatLonRad, Placed, Sdf, Shape, SizeClass, place};

const ANCHORS: [(f64, f64); 3] = [(46.55, 8.0), (0.0, 0.0), (64.0, 179.5)];

fn ll(lat_deg: f64, lon_deg: f64) -> LatLonRad {
    LatLonRad::new(lat_deg * RAD, lon_deg * RAD)
}

/// The ground point at BF (x, y) of the frame at (`lat`, `lon`).
fn offset(lat: f64, lon: f64, x: f64, y: f64) -> LatLonRad {
    let g = Wgs84Frame::new(lat * RAD, lon * RAD).bf_to_geo(DVec3::new(x, y, 0.0));
    LatLonRad::new(g.lat_rad, g.lon_rad)
}

/// The signed distance to a box of half-sizes (`hw`, `hh`) turned clockwise by `rot`.
fn box_sd(p: [f64; 2], hw: f64, hh: f64, rot: f64) -> f64 {
    let (s, c) = (dmath::sin(-rot), dmath::cos(-rot));
    let (lx, ly) = (p[0] * c + p[1] * s, -p[0] * s + p[1] * c);
    let (qx, qy) = (lx.abs() - hw, ly.abs() - hh);
    let outside = (qx.max(0.0) * qx.max(0.0) + qy.max(0.0) * qy.max(0.0)).sqrt();
    outside + qx.max(qy).min(0.0)
}

/// Checks every cell centre within `D` of the boundary against `analytic`; the worst error.
fn check_sdf(name: &str, sdf: &Sdf, analytic: impl Fn([f64; 2]) -> f64) -> f64 {
    let (nx, ny) = sdf.dims();
    let mut worst = 0.0f64;
    let mut checked = 0usize;
    for j in 0..ny {
        for i in 0..nx {
            let (x, y, v) = sdf.cell(i, j);
            let a = analytic([x, y]);
            if a.abs() <= sdf.d_clamp_m() {
                let e = (f64::from(v) - a).abs();
                worst = worst.max(e);
                checked += 1;
                assert!(
                    e <= sdf.cell_m(),
                    "{name}: cell ({i}, {j}) at ({x}, {y}): {v} against {a}"
                );
            }
        }
    }
    assert!(checked > 1_000, "{name}: only {checked} cells checked");
    worst
}

#[test]
fn bf1_the_sdf_against_analytic_distances() {
    for (lat, lon) in ANCHORS {
        let c = ll(lat, lon);
        let rect = place(&Shape::Rect {
            center: c,
            width_m: 120_000.0,
            height_m: 80_000.0,
            rotation_rad: 30.0 * RAD,
        });
        let s = Sdf::build(&rect.polygon, 0.0);
        let w = check_sdf("rect", &s, |p| box_sd(p, 60_000.0, 40_000.0, 30.0 * RAD));
        println!(
            "BF1 rect at {lat}, {lon}: worst {w:.2} m, cell {:.1} m",
            s.cell_m()
        );

        let circle = place(&Shape::Circle {
            center: c,
            radius_m: 60_000.0,
        });
        let s = Sdf::build(&circle.polygon, 0.0);
        let w = check_sdf("circle", &s, |p| {
            (p[0] * p[0] + p[1] * p[1]).sqrt() - 60_000.0
        });
        println!(
            "BF1 circle at {lat}, {lon}: worst {w:.2} m, cell {:.1} m",
            s.cell_m()
        );

        let az = 60.0 * RAD;
        let (dx, dy) = (150_000.0 * dmath::sin(az), 150_000.0 * dmath::cos(az));
        let (a, b) = (offset(lat, lon, -dx, -dy), offset(lat, lon, dx, dy));
        let cap = place(&Shape::Capsule {
            a,
            b,
            radius_m: 90_000.0,
        });
        let (pa, pb) = (
            cap.frame.geo_to_bf(a.ground()),
            cap.frame.geo_to_bf(b.ground()),
        );
        let s = Sdf::build(&cap.polygon, 0.0);
        let w = check_sdf("capsule", &s, |p| {
            dist2_to_segment(p, [pa.x, pa.y], [pb.x, pb.y]).sqrt() - 90_000.0
        });
        println!(
            "BF1 capsule at {lat}, {lon}: worst {w:.2} m, cell {:.1} m",
            s.cell_m()
        );
    }
}

/// BF4's polygon: 64 vertices alternating 400 and 250 km from the centre.
fn star(lat: f64, lon: f64) -> Shape {
    let mut verts = [LatLonRad::default(); 64];
    for (k, v) in verts.iter_mut().enumerate() {
        let r = if k % 2 == 0 { 400_000.0 } else { 250_000.0 };
        let t = core::f64::consts::TAU * k as f64 / 64.0;
        *v = offset(lat, lon, r * dmath::sin(t), r * dmath::cos(t));
    }
    Shape::Polygon { verts, count: 64 }
}

#[test]
fn bf4_the_64_gon_natively() {
    let p = place(&star(46.55, 8.0));
    assert!(p.polygon.is_simple());
    let t0 = std::time::Instant::now();
    let s = Sdf::build(&p.polygon, 0.0);
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    println!(
        "BF4 (native, informational; this build's profile): {ms:.1} ms, grid {:?}, cell {:.0} m",
        s.dims(),
        s.cell_m()
    );
    assert!(s.sample(0.0, 0.0) < 0.0);
}

fn alps_coverage(max_level: u8) -> [CellCoverage; 1] {
    [CellCoverage {
        x: 33,
        y: 7,
        max_level,
    }]
}

fn run_validator(
    shape: &Shape,
    spawn: LatLonRad,
    spawn_level: u8,
    coverage: &[CellCoverage],
) -> (Placed, world::Validation) {
    let placed = place(shape);
    let sdf = Sdf::build(&placed.polygon, 0.0);
    let v = validate(&ValidateInputs {
        shape,
        placed: &placed,
        sdf: &sdf,
        spawn,
        spawn_level,
        coverage,
    });
    (placed, v)
}

fn circle(lat: f64, lon: f64, r: f64) -> Shape {
    Shape::Circle {
        center: ll(lat, lon),
        radius_m: r,
    }
}

#[test]
fn bf5_each_validator_check_passes_and_fails() {
    let alps = ll(46.55, 8.0);
    let cov = alps_coverage(12);
    let ok = |s: &Shape, spawn: LatLonRad| run_validator(s, spawn, 12, &cov).1;

    // V1: simple.
    let mut bow = [LatLonRad::default(); 64];
    for (k, (x, y)) in [(-20e3, -20e3), (20e3, 20e3), (20e3, -20e3), (-20e3, 20e3)]
        .into_iter()
        .enumerate()
    {
        bow[k] = offset(46.55, 8.0, x, y);
    }
    let bow = Shape::Polygon {
        verts: bow,
        count: 4,
    };
    assert_eq!(ok(&bow, alps).reject, Some(Reject::SelfIntersecting));
    let mut sq = [LatLonRad::default(); 64];
    for (k, (x, y)) in [(-20e3, -20e3), (20e3, -20e3), (20e3, 20e3), (-20e3, 20e3)]
        .into_iter()
        .enumerate()
    {
        sq[k] = offset(46.55, 8.0, x, y);
    }
    let square = Shape::Polygon {
        verts: sq,
        count: 4,
    };
    assert_eq!(ok(&square, alps).reject, None);
    // V1: area ≥ 1 km².
    assert_eq!(
        ok(&circle(46.55, 8.0, 500.0), alps).reject,
        Some(Reject::TooSmall)
    );
    assert_eq!(ok(&circle(46.55, 8.0, 600.0), alps).reject, None);
    // V1: extent ≤ 1 000 km.
    assert_eq!(
        ok(&circle(46.55, 8.0, 600_000.0), alps).reject,
        Some(Reject::TooLarge)
    );
    assert_eq!(ok(&circle(46.55, 8.0, 400_000.0), alps).reject, None);
    // V1: ≤ 64 authored vertices.
    let Shape::Polygon { verts, .. } = star(46.55, 8.0) else {
        unreachable!()
    };
    assert_eq!(ok(&Shape::Polygon { verts, count: 64 }, alps).reject, None);
    assert_eq!(
        ok(&Shape::Polygon { verts, count: 65 }, alps).reject,
        Some(Reject::TooManyVertices)
    );
    // V2: the anchor within ±85°.
    assert_eq!(
        ok(&circle(86.0, 8.0, 20_000.0), ll(86.0, 8.0)).reject,
        Some(Reject::PolarAnchor)
    );
    assert_eq!(ok(&circle(84.0, 8.0, 20_000.0), ll(84.0, 8.0)).reject, None);
    // V3: every cell meeting the area holds z10.
    let medium = Shape::Rect {
        center: alps,
        width_m: 120_000.0,
        height_m: 120_000.0,
        rotation_rad: 0.0,
    };
    assert_eq!(
        run_validator(&medium, alps, 12, &alps_coverage(8)).1.reject,
        Some(Reject::NoCoverage)
    );
    assert_eq!(
        run_validator(&medium, alps, 10, &alps_coverage(10))
            .1
            .reject,
        None
    );
    // V4: the fine zone holds spawn_level, else a warning.
    let v = run_validator(&medium, alps, 12, &alps_coverage(10)).1;
    assert_eq!(v.reject, None);
    assert!(v.warn.has(WarnFlags::LOW_DETAIL));
    assert!(
        !run_validator(&medium, alps, 12, &alps_coverage(12))
            .1
            .warn
            .has(WarnFlags::LOW_DETAIL)
    );
    // V5: the spawn inside.
    assert_eq!(
        ok(&medium, ll(48.0, 8.0)).reject,
        Some(Reject::SpawnOutside)
    );
    assert_eq!(ok(&medium, alps).reject, None);
    // V6: raised when lower than 150 m above the terrain.
    assert_eq!(spawn_clearance(4_000.0, 3_900.0), (4_200.0, true));
    assert_eq!(spawn_clearance(4_000.0, 3_800.0), (4_000.0, false));
    // V7 and V8 always pass in v0.2a (one aircraft, an air spawn): no reject above for them.
    // The plan size.
    assert_eq!(check_plan_size(8_193), Some(Reject::TooLarge));
    assert_eq!(check_plan_size(1), None);
}

fn distance_m(a: geo::Geodetic, b: geo::Geodetic) -> f64 {
    (geodetic_to_ecef(a) - geodetic_to_ecef(b)).length()
}

#[test]
fn bf6_anchors_across_the_antimeridian() {
    let expect = geo::Geodetic::new(0.0, core::f64::consts::PI, 0.0);
    let c = place(&circle(0.0, 180.0, 400_000.0));
    let d = distance_m(c.frame.anchor, expect);
    println!(
        "BF6 circle: anchor {:.9}°, {:.9}°, {d:.3e} m off",
        c.frame.anchor.lat_rad * DEG,
        c.frame.anchor.lon_rad * DEG
    );
    assert!(d < 1.0, "{d} m");
    let mut verts = [LatLonRad::default(); 64];
    for (k, (lat, lon)) in [(-2.0, 178.0), (-2.0, -178.0), (2.0, -178.0), (2.0, 178.0)]
        .into_iter()
        .enumerate()
    {
        verts[k] = ll(lat, lon);
    }
    let p = place(&Shape::Polygon { verts, count: 4 });
    let d = distance_m(p.frame.anchor, expect);
    println!(
        "BF6 polygon: anchor {:.9}°, {:.9}°, {d:.3e} m off",
        p.frame.anchor.lat_rad * DEG,
        p.frame.anchor.lon_rad * DEG
    );
    assert!(d < 1.0, "{d} m");
    assert!(p.polygon.contains([0.0, 0.0]));
}

fn plan_for(class: SizeClass, lat: f64, lon: f64, coverage: &[CellCoverage]) -> SimTerrainPlan {
    let edge = class.edge_m();
    let shape = Shape::Rect {
        center: ll(lat, lon),
        width_m: edge,
        height_m: edge,
        rotation_rad: 0.0,
    };
    let placed = place(&shape);
    SimTerrainPlan::build(&PlanInputs {
        frame: &placed.frame,
        polygon: &placed.polygon,
        class,
        spawn: ll(lat, lon),
        coverage,
    })
}

#[test]
fn ws1_the_sim_terrain_plans() {
    let cov = alps_coverage(12);
    let mut counts = Vec::new();
    for class in world::preset::ALL {
        let p = plan_for(class, 46.55, 8.0, &cov);
        assert!(
            p.ids.windows(2).all(|w| w[0] < w[1]),
            "{class:?}: sorted and unique"
        );
        let (z5, z10, z12) = (p.count_at(5), p.count_at(10), p.count_at(12));
        assert_eq!(
            z5 + z10 + z12,
            p.ids.len(),
            "{class:?}: only levels 5, 10 and 12"
        );
        assert_eq!(p.fine_level, 12);
        assert_eq!(p.rest_level, 10);
        let big = matches!(class, SizeClass::Huge | SizeClass::Extreme);
        if big {
            // z12 only near the spawn: every z12 tile within the fine zone plus a tile.
            let f = Wgs84Frame::new(46.55 * RAD, 8.0 * RAD);
            for &id in &p.ids {
                let t = TileId::from_id(id).unwrap();
                if t.z == 12 {
                    let (lat, lon) = t.bounds().center();
                    let q = f.geo_to_bf(geo::Geodetic::new(lat, lon, 0.0));
                    assert!(
                        (q.x * q.x + q.y * q.y).sqrt() < 45_000.0,
                        "{class:?}: z12 tile {t:?} far from the spawn"
                    );
                }
            }
        } else {
            // Inside the one z12 cell, z12 replaces z10.
            assert_eq!(z10, 0, "{class:?}");
        }
        println!(
            "WS1 {class:?} at the Alps: z5 {z5}, z10 {z10}, z12 {z12}, total {}",
            p.ids.len()
        );
        counts.push((z5, z10, z12));
    }
    // The antimeridian fixture: z10 cells on both sides of ±180°.
    let am = [
        CellCoverage {
            x: 63,
            y: 6,
            max_level: 10,
        },
        CellCoverage {
            x: 0,
            y: 6,
            max_level: 10,
        },
    ];
    let p = plan_for(SizeClass::Medium, 53.4, 180.0, &am);
    assert_eq!(p.fine_level, 10);
    assert_eq!(p.count_at(12), 0);
    let xs: Vec<u32> = p
        .ids
        .iter()
        .map(|&id| TileId::from_id(id).unwrap())
        .filter(|t| t.z == 10)
        .map(|t| t.x)
        .collect();
    assert!(
        xs.iter().any(|&x| x < 4) && xs.iter().any(|&x| x > 2_043),
        "both sides of 180°: {xs:?}"
    );
    println!(
        "WS1 Medium at the antimeridian: z5 {}, z10 {}",
        p.count_at(5),
        p.count_at(10)
    );
    counts.push((p.count_at(5), p.count_at(10), 0));
    let pinned: [(usize, usize, usize); 6] = PINNED;
    assert_eq!(
        counts, pinned,
        "the plan counts moved; check the rules, then re-pin"
    );
}

/// The counts recorded on the first run (2026-11-13), pinned. // DATA: WS1
const PINNED: [(usize, usize, usize); 6] = [
    (3, 0, 326),
    (4, 0, 1_257),
    (6, 0, 4_539),
    (6, 1_101, 342),
    (15, 4_089, 342),
    (2, 104, 0),
];
