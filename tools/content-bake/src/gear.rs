//! Lints for the landing-gear data (GDD 62.6, ADR-35): geometry that cannot work is a data error.
//!
//! The checks are small closed-form versions of what the ground model will do: the static load
//! on each leg group from the moment balance, the static compression of the progressive spring
//! `N = k x (1 + (x / x_max)^2)`, the natural frequency and damping ratio of each leg carrying its
//! share of the aircraft, tip-back, and the clearance of the airframe's contact points at the
//! touchdown attitude.

use simmath::dmath;

use crate::check::eval_curve;
use crate::model::{AeroFile, AircraftFile, ContactFile, GearLegFile, GridFile};

/// Standard gravity, m/s².
const G: f64 = 9.806_65;
/// The CG range, as fractions of the MAC from the empty CG: aft (negative x) and forward.
/// // DATA: GDD C.1 CG limits 24-36 % MAC around a reference point at 27 %.
const CG_RANGE_MAC: (f64, f64) = (-0.09, 0.03);
/// The non-main legs carry this share of the weight, at the CG range limits.
const NOSE_SHARE: (f64, f64) = (0.05, 0.20);
/// Static compression as a fraction of the stroke, at the nominal CG, at landing and take-off mass.
const COMPRESSION: (f64, f64) = (0.20, 0.60);
/// Highest strut natural frequency, rad/s (GDD 11.10: the RK4 step resolves 15 rad/s).
const BOUNCE_OMEGA_MAX: f64 = 15.0;
/// The damping ratio of a compressing strut.
const ZETA_COMP: (f64, f64) = (0.15, 1.0);
/// The CG must stay this far inside the legs' fore-and-aft span, m.
const TIP_BACK_MARGIN_M: f64 = 0.05;
/// Approach flight-path angle used for the touchdown attitude, degrees. // DATA: GDD 11.13.3
const APPROACH_GAMMA_DEG: f64 = 3.0;
/// Clearance of the tail over the touchdown pitch, degrees.
const TAIL_MARGIN_DEG: f64 = 1.0;
/// Margin over the stall speed at touchdown (GDD C.8 `V_TD = 1.15 V_s`).
const TOUCHDOWN_SPEED_RATIO: f64 = 1.15;
/// Mach number used for the lift curve at touchdown speed.
const TOUCHDOWN_MACH: f64 = 0.2;

/// One finding: a JSON pointer, what is wrong and how to fix it.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    /// JSON pointer into the aircraft file.
    pub ptr: String,
    /// What is wrong.
    pub message: String,
    /// How to fix it.
    pub hint: String,
}

fn problem(ptr: impl Into<String>, message: impl Into<String>, hint: impl Into<String>) -> Problem {
    Problem {
        ptr: ptr.into(),
        message: message.into(),
        hint: hint.into(),
    }
}

/// The static compression of a leg under `load_n`, m (bisection on the monotone spring curve).
#[must_use]
pub fn compression_m(leg: &GearLegFile, load_n: f64) -> f64 {
    let spring = |x: f64| {
        let u = x / leg.stroke_m;
        leg.k_npm * x * (1.0 + u * u)
    };
    let (mut lo, mut hi) = (0.0_f64, 4.0 * leg.stroke_m);
    for _ in 0..80 {
        let mid = lo.midpoint(hi);
        if spring(mid) < load_n {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo.midpoint(hi)
}

/// The tangent stiffness at compression `x`, N/m.
fn tangent_npm(leg: &GearLegFile, x: f64) -> f64 {
    {
        let u = x / leg.stroke_m;
        leg.k_npm * (1.0 + 3.0 * u * u)
    }
}

fn mean(xs: impl Iterator<Item = f64>) -> f64 {
    let (sum, n) = xs.fold((0.0, 0.0), |(s, n), x| (s + x, n + 1.0));
    sum / n
}

/// The legs split into main legs and the others, with their mean fore-and-aft positions.
struct Groups<'a> {
    mains: Vec<&'a GearLegFile>,
    others: Vec<&'a GearLegFile>,
    x_main: f64,
    x_other: f64,
}

fn groups(a: &AircraftFile) -> Option<Groups<'_>> {
    let (mains, others): (Vec<_>, Vec<_>) = a.geometry.gear.iter().partition(|l| l.main);
    if mains.is_empty() || others.is_empty() {
        return None;
    }
    let x_main = mean(mains.iter().map(|l| l.pos_m[0]));
    let x_other = mean(others.iter().map(|l| l.pos_m[0]));
    if (x_other - x_main).abs() < 0.1 {
        return None;
    }
    Some(Groups {
        mains,
        others,
        x_main,
        x_other,
    })
}

impl Groups<'_> {
    /// The share of the weight on the non-main legs for a CG at `x_cg`.
    fn other_share(&self, x_cg: f64) -> f64 {
        (x_cg - self.x_main) / (self.x_other - self.x_main)
    }
}

/// Checks that need only the aircraft file. `ptr` strings are JSON pointers into it.
#[must_use]
pub fn lint_geometry(a: &AircraftFile) -> Vec<Problem> {
    let mut out = Vec::new();
    let g = &a.geometry;
    ids_unique(
        &mut out,
        "/geometry/gear",
        g.gear.iter().map(|l| l.id.as_str()),
    );
    ids_unique(
        &mut out,
        "/geometry/contacts",
        g.contacts.iter().map(|p| p.id.as_str()),
    );
    contacts_inside(&mut out, a);
    if g.gear.is_empty() {
        return out;
    }
    if !g.gear.iter().any(|l| l.brake) {
        out.push(problem(
            "/geometry/gear",
            "no leg has a brake",
            "set brake to true on at least one leg",
        ));
    }
    let steerable: Vec<usize> = g
        .gear
        .iter()
        .enumerate()
        .filter(|(_, l)| l.steer_max_deg > 0.0)
        .map(|(i, _)| i)
        .collect();
    if steerable.len() > 1 {
        out.push(problem(
            format!("/geometry/gear/{}/steer_max_deg", steerable[1]),
            "more than one leg steers",
            "give steer_max_deg to one leg only (the nose leg), or none",
        ));
    }
    let Some(grp) = groups(a) else {
        out.push(problem(
            "/geometry/gear",
            "the legs are not a set of main legs plus at least one other leg a hand's breadth away",
            "mark the weight-bearing legs `main: true`, and add a nose or tail leg at a different x",
        ));
        return out;
    };

    static_balance(&mut out, a, &grp);
    tip_back(&mut out, a, &grp);
    out
}

fn ids_unique<'a>(out: &mut Vec<Problem>, ptr: &str, ids: impl Iterator<Item = &'a str>) {
    let mut seen = Vec::new();
    for (i, id) in ids.enumerate() {
        if seen.contains(&id) {
            out.push(problem(
                format!("{ptr}/{i}/id"),
                format!("`{id}` is listed twice"),
                "give each entry its own id",
            ));
        }
        seen.push(id);
    }
}

/// Every wheel and contact point lies within the bounding box's span and extent.
fn contacts_inside(out: &mut Vec<Problem>, a: &AircraftFile) {
    let g = &a.geometry;
    let [len, span, height] = g.bbox_m;
    let points: Vec<(String, [f64; 3])> = g
        .gear
        .iter()
        .enumerate()
        .map(|(i, l)| (format!("/geometry/gear/{i}/pos_m"), l.pos_m))
        .chain(
            g.contacts
                .iter()
                .enumerate()
                .map(|(i, p)| (format!("/geometry/contacts/{i}/pos_m"), p.pos_m)),
        )
        .collect();
    for (ptr, p) in &points {
        if p[1].abs() > 0.5 * span + 1e-9 {
            out.push(problem(
                format!("{ptr}/1"),
                format!("y = {} m is outside the span of {span} m", p[1]),
                "keep wheels and contact points within half the span of the centreline",
            ));
        }
    }
    for (axis, name, size) in [(0, "length", len), (2, "height", height)] {
        let lo = points
            .iter()
            .map(|(_, p)| p[axis])
            .fold(f64::INFINITY, f64::min);
        let hi = points
            .iter()
            .map(|(_, p)| p[axis])
            .fold(f64::NEG_INFINITY, f64::max);
        if points.len() > 1 && hi - lo > size {
            out.push(problem(
                "/geometry/contacts",
                format!(
                    "the wheels and contact points span {:.2} m in {name}, more than the bounding box's {size} m",
                    hi - lo
                ),
                "check the positions, or enlarge bbox_m",
            ));
        }
    }
}

/// Load share, compression, bounce frequency and damping at the nominal CG.
fn static_balance(out: &mut Vec<Problem>, a: &AircraftFile, grp: &Groups<'_>) {
    let m = &a.mass;
    let mac = a.geometry.mac_m;
    let cg_x = m.cg_empty_m[0];

    // The share on the non-main legs over the CG range.
    for (label, x_cg) in [
        ("aft", cg_x + CG_RANGE_MAC.0 * mac),
        ("forward", cg_x + CG_RANGE_MAC.1 * mac),
    ] {
        let share = grp.other_share(x_cg);
        if !(NOSE_SHARE.0..=NOSE_SHARE.1).contains(&share) {
            out.push(problem(
                "/geometry/gear",
                format!(
                    "at the {label} CG limit the nose leg carries {:.1} % of the weight, outside {:.0}-{:.0} %",
                    100.0 * share,
                    100.0 * NOSE_SHARE.0,
                    100.0 * NOSE_SHARE.1
                ),
                "move the nose leg or the mains fore or aft (too little load means no steering, too much overloads the nose strut)",
            ));
        }
    }

    // Compression, frequency and damping at empty, landing and take-off mass.
    let share_other = grp.other_share(cg_x);
    let shares = [(&grp.mains, 1.0 - share_other), (&grp.others, share_other)];
    for (mass_name, mass_kg) in [
        ("empty", m.empty_kg),
        ("maximum landing", m.max_landing_kg),
        ("maximum take-off", m.max_takeoff_kg),
    ] {
        for (legs, share) in &shares {
            for leg in *legs {
                let i = a
                    .geometry
                    .gear
                    .iter()
                    .position(|l| l.id == leg.id)
                    .unwrap_or(0);
                let leg_mass = mass_kg * share / legs.len() as f64;
                let load = leg_mass * G;
                let x = compression_m(leg, load);
                let ratio = x / leg.stroke_m;
                if mass_name != "empty" && !(COMPRESSION.0..=COMPRESSION.1).contains(&ratio) {
                    out.push(problem(
                        format!("/geometry/gear/{i}/k_npm"),
                        format!(
                            "at {mass_name} mass the `{}` strut compresses {:.0} % of its stroke, outside {:.0}-{:.0} %",
                            leg.id,
                            100.0 * ratio,
                            100.0 * COMPRESSION.0,
                            100.0 * COMPRESSION.1
                        ),
                        "a stiffer spring compresses less; the stroke must also absorb the landing",
                    ));
                }
                let kt = tangent_npm(leg, x);
                let omega = (kt / leg_mass).sqrt();
                if omega >= BOUNCE_OMEGA_MAX {
                    out.push(problem(
                        format!("/geometry/gear/{i}/k_npm"),
                        format!(
                            "at {mass_name} mass the `{}` strut rings at {omega:.1} rad/s, above {BOUNCE_OMEGA_MAX} rad/s that the 120 Hz step resolves",
                            leg.id
                        ),
                        "lower the spring rate",
                    ));
                }
                let zeta = leg.c_comp_nspm / (2.0 * (kt * leg_mass).sqrt());
                if mass_name != "empty" && !(ZETA_COMP.0..=ZETA_COMP.1).contains(&zeta) {
                    out.push(problem(
                        format!("/geometry/gear/{i}/c_comp_nspm"),
                        format!(
                            "at {mass_name} mass the `{}` strut's compression damping ratio is {zeta:.2}, outside {}-{}",
                            leg.id, ZETA_COMP.0, ZETA_COMP.1
                        ),
                        "scale c_comp_nspm with the square root of k_npm times the carried mass",
                    ));
                }
            }
        }
    }
}

/// The CG range stays inside the legs' fore-and-aft span.
fn tip_back(out: &mut Vec<Problem>, a: &AircraftFile, grp: &Groups<'_>) {
    let mac = a.geometry.mac_m;
    let cg_x = a.mass.cg_empty_m[0];
    let xs = a.geometry.gear.iter().map(|l| l.pos_m[0]);
    let (lo, hi) = xs.fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), x| {
        (l.min(x), h.max(x))
    });
    let (aft, fwd) = (cg_x + CG_RANGE_MAC.0 * mac, cg_x + CG_RANGE_MAC.1 * mac);
    if aft < lo + TIP_BACK_MARGIN_M || fwd > hi - TIP_BACK_MARGIN_M {
        let side = if grp.x_main < grp.x_other {
            "tail"
        } else {
            "nose"
        };
        out.push(problem(
            "/geometry/gear",
            format!(
                "the CG range {aft:.2} to {fwd:.2} m is not inside the legs' span {lo:.2} to {hi:.2} m by {TIP_BACK_MARGIN_M} m; the aircraft would tip onto its {side}"
            ),
            "move the main legs aft of the CG range, or check the CG",
        ));
    }
}

/// Bilinear value of a grid (`z[y][x]`) at `(x, y)`, clamped at the edges.
fn eval_grid(g: &GridFile, x: f64, y: f64) -> f64 {
    let locate = |axis: &[f64], v: f64| -> (usize, f64) {
        if v <= axis[0] || axis.len() == 1 {
            return (0, 0.0);
        }
        if v >= axis[axis.len() - 1] {
            return (axis.len() - 2, 1.0);
        }
        let i = axis.partition_point(|&a| a <= v) - 1;
        (i, (v - axis[i]) / (axis[i + 1] - axis[i]))
    };
    let (i, fx) = locate(&g.x, x);
    let (j, fy) = locate(&g.y, y);
    let at = |jj: usize, ii: usize| g.z[jj.min(g.y.len() - 1)][ii.min(g.x.len() - 1)];
    let lo = at(j, i) * (1.0 - fx) + at(j, i + 1) * fx;
    let hi = at(j + 1, i) * (1.0 - fx) + at(j + 1, i + 1) * fx;
    lo * (1.0 - fy) + hi * fy
}

/// The pitch attitude at touchdown at 1.15 `V_s` in the landing configuration, degrees, or
/// `None` if the lift tables never reach the needed coefficient.
#[must_use]
pub fn touchdown_pitch_deg(aero: &AeroFile) -> Option<f64> {
    let cl_max = eval_curve(&aero.lift.cl_max.points, TOUCHDOWN_MACH) + aero.config.flap_ld_cl;
    let cl_td = cl_max / (TOUCHDOWN_SPEED_RATIO * TOUCHDOWN_SPEED_RATIO);
    let target = cl_td - aero.config.flap_ld_cl;
    let mut alpha = 0.0_f64;
    while alpha <= 45.0 {
        if eval_grid(&aero.lift.cl_base, alpha, TOUCHDOWN_MACH) >= target {
            return Some(alpha - APPROACH_GAMMA_DEG);
        }
        alpha += 0.05;
    }
    None
}

/// The nose-up pitch at which an airframe contact point behind the main legs touches the
/// ground with the mains compressed by their static load at landing mass, degrees.
#[must_use]
pub fn scrape_pitch_deg(a: &AircraftFile, contact: &ContactFile) -> Option<f64> {
    let grp = groups(a)?;
    let cg_x = a.mass.cg_empty_m[0];
    let share = 1.0 - grp.other_share(cg_x);
    let main = grp.mains[0];
    let load = a.mass.max_landing_kg * share / grp.mains.len() as f64 * G;
    let z_main = main.pos_m[2] - compression_m(main, load);
    let dx = grp.x_main - contact.pos_m[0];
    let dz = z_main - contact.pos_m[2];
    if dx <= 0.0 || dz <= 0.0 {
        return None;
    }
    Some(dmath::atan(dz / dx).to_degrees())
}

/// Tail clearance at the touchdown attitude (needs the aero tables).
#[must_use]
pub fn lint_touchdown(a: &AircraftFile, aero: &AeroFile) -> Vec<Problem> {
    let mut out = Vec::new();
    if groups(a).is_none() {
        return out;
    }
    let Some(pitch) = touchdown_pitch_deg(aero) else {
        out.push(problem(
            "/aero/tables_ref",
            "the lift table never reaches the lift needed at 1.15 times the landing stall speed",
            "check cl_base, cl_max and flap_ld_cl",
        ));
        return out;
    };
    for (i, c) in a.geometry.contacts.iter().enumerate() {
        if let Some(limit) = scrape_pitch_deg(a, c)
            && limit < pitch + TAIL_MARGIN_DEG
        {
            out.push(problem(
                format!("/geometry/contacts/{i}/pos_m"),
                format!(
                    "`{}` touches the ground at {limit:.1} deg of pitch, but the touchdown attitude at 1.15 V_s is {pitch:.1} deg (needs {TAIL_MARGIN_DEG} deg of clearance)",
                    c.id
                ),
                "raise or shorten the tail, or move the main legs aft",
            ));
        }
    }
    out
}
