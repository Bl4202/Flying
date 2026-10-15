//! Generator of `content/core/aero/kestrel.json`, the Kestrel's aerodynamic tables.
//!
//! The JSON is the source of truth the baker reads; this file documents how it was derived and
//! reproduces it exactly. A test in `tools/content-bake/tests/kestrel_aero.rs` fails if the two
//! differ; to regenerate after editing this file run
//!
//! ```text
//! KESTREL_AERO_WRITE=1 cargo test -p content-bake --test kestrel_aero
//! ```
//!
//! Sources: GDD Appendix C.4 (polar, wave drag, C_L,max), C.5 (derivatives), C.8 (the reference
//! calculator, whose breakpoints the one-dimensional tables reuse exactly) and the v0.1a design
//! review (static margin, elevator power and Mach tuck by Mach number). Values marked *target*
//! in the GDD are authored starting points; the conformance test (ADR-31) is the judge, and every
//! change made while tuning is logged in `content/core/aero/README.md`.
#![allow(
    clippy::doc_markdown,
    reason = "the docs use aerodynamic notation (C_L,max, C_m,de), not code"
)]

use simmath::dmath;

/// Angle-of-attack grid, degrees: 2 degree spacing through the stall caps interpolation loss at
/// the lift peak to about 0.2 %.
const ALPHA_DEG: [f64; 31] = [
    -30.0, -20.0, -15.0, -10.0, -6.0, -3.0, 0.0, 3.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0,
    22.0, 24.0, 26.0, 28.0, 30.0, 32.0, 34.0, 37.0, 40.0, 45.0, 50.0, 60.0, 70.0, 80.0, 90.0,
];

/// Mach grid of the two-dimensional tables; it contains every C_L,max breakpoint of C.4.
const MACH: [f64; 12] = [0.0, 0.3, 0.6, 0.8, 0.9, 0.95, 1.05, 1.2, 1.5, 2.0, 2.5, 3.0];

/// Trimmed whole-aircraft C_L,max against Mach (C.4, C.8 `CLMAX`).
const CL_MAX: [(f64, f64); 6] = [
    (0.0, 1.60),
    (0.6, 1.60),
    (0.9, 1.30),
    (1.2, 1.00),
    (2.0, 0.70),
    (2.5, 0.60),
];

/// Wave-drag increment against Mach (C.4, C.8 `WAVE`).
const CD_WAVE: [(f64, f64); 9] = [
    (0.0, 0.0),
    (0.75, 0.0),
    (0.85, 0.0030),
    (0.95, 0.0100),
    (1.05, 0.0260),
    (1.20, 0.0250),
    (1.50, 0.0190),
    (2.00, 0.0130),
    (2.40, 0.0120),
];

/// Lift-curve slope, per radian (*target*). 3.6 through M 1.2 keeps the C.5 short-period table
/// reproducible; supersonic fall-off after GDD 11.6.4.
const CL_ALPHA: [(f64, f64); 6] = [
    (0.0, 3.6),
    (1.2, 3.6),
    (1.5, 3.1),
    (2.0, 2.5),
    (2.5, 2.1),
    (3.0, 1.8),
];

/// Per Mach grid point: static margin (fraction of MAC; the aerodynamic centre moves aft through
/// the transonic range), C_m at zero lift (Mach tuck), and elevator power C_m,de per radian
/// (falling transonically, GDD 11.6.4). Chosen so that the trimmed elevator, after the
/// control-effectiveness fall-off, stays within 23 degrees everywhere in the envelope up to
/// C_L,max or 9 g (*target*; see the tuning log).
const PITCH: [(f64, f64, f64); 12] = [
    (0.06, 0.010, -0.65),
    (0.06, 0.010, -0.65),
    (0.06, 0.010, -0.65),
    (0.06, 0.010, -0.65),
    (0.08, 0.005, -0.60),
    (0.10, 0.0, -0.55),
    (0.12, -0.010, -0.50),
    (0.13, -0.015, -0.50),
    (0.14, -0.015, -0.48),
    (0.15, -0.015, -0.42),
    (0.15, -0.015, -0.38),
    (0.15, -0.015, -0.34),
];

/// Elevator lift per unit of elevator moment: the stabilator acts 1.6 MAC behind the reference
/// point, so `C_L,de = |C_m,de| / 1.6`.
const TAIL_ARM_MAC: f64 = 1.6;
/// Moment reference point (the empty CG), fraction of MAC.
const X_REF_MAC: f64 = 0.27;
/// Flat-plate aerodynamic centre, fraction of MAC (GDD 11.6.3).
const X_AC_POST_MAC: f64 = 0.50;
/// Flat-plate normal-force coefficient (GDD 11.6.3).
const CD90: f64 = 1.2;
/// The negative stall is this fraction of the positive one.
const NEGATIVE_STALL: f64 = 0.6;

fn interp(t: &[(f64, f64)], x: f64) -> f64 {
    if x <= t[0].0 {
        return t[0].1;
    }
    for w in t.windows(2) {
        if x <= w[1].0 {
            return w[0].1 + (w[1].1 - w[0].1) * (x - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    t[t.len() - 1].1
}

/// The untrimmed lift peak that makes the trimmed C_L,max equal the table value. At the peak the
/// elevator must cancel `C_m,ac - SM C` and in doing so loses `C_L,de / C_m,de` of lift per unit
/// moment, so `trimmed = C (1 - SM / arm) + C_m,ac / arm`.
fn peak(cl_max: f64, sm: f64, cm_ac: f64) -> f64 {
    (cl_max - cm_ac / TAIL_ARM_MAC) / (1.0 - sm / TAIL_ARM_MAC)
}

/// Lift on one side of zero for |alpha| `x` (rad): linear to a knee at 0.8 C, a quadratic with
/// zero slope at the peak 1.2 C / a, a post-stall fall to 0.88 C at +6 deg and 0.80 C at +14 deg,
/// then a fade to the flat plate `cd90 sin cos` by 60 deg.
fn lift_side(x: f64, c: f64, a: f64) -> f64 {
    let knee = 0.8 * c / a;
    let top = 1.2 * c / a;
    if x <= knee {
        return a * x;
    }
    if x <= top {
        let d = top - x;
        return c - a / (2.0 * (top - knee)) * d * d;
    }
    let d = (x - top).to_degrees();
    if d <= 6.0 {
        return c - 0.12 * c * (d / 6.0) * (d / 6.0);
    }
    if d <= 14.0 {
        return 0.88 * c - 0.08 * c * (d - 6.0) / 8.0;
    }
    let plate = CD90 * dmath::sin(x) * dmath::cos(x);
    let fade_start = top.to_degrees() + 14.0;
    let w = ((x.to_degrees() - fade_start) / (60.0 - fade_start)).clamp(0.0, 1.0);
    (1.0 - w) * 0.80 * c + w * plate
}

/// One Mach row: (peak alpha in degrees, C_L row, C_m0 row).
fn row(j: usize) -> (f64, Vec<f64>, Vec<f64>) {
    let mach = MACH[j];
    let (sm, cm_ac, _) = PITCH[j];
    let a = interp(&CL_ALPHA, mach);
    let c = peak(interp(&CL_MAX, mach), sm, cm_ac);
    let top_deg = (1.2 * c / a).to_degrees();
    let mut cl = Vec::new();
    let mut cm = Vec::new();
    for &deg in &ALPHA_DEG {
        let x = deg.to_radians();
        let lift = if deg >= 0.0 {
            lift_side(x, c, a)
        } else {
            -lift_side(-x, NEGATIVE_STALL * c, a)
        };
        // Pitching moment: the attached-flow aerodynamic centre until the peak, then a fade over
        // 20 deg to the flat plate's normal force acting at mid-chord.
        let w = ((deg - top_deg) / 20.0).clamp(0.0, 1.0);
        let arm = sm + (X_AC_POST_MAC - X_REF_MAC - sm) * w;
        let normal = (1.0 - w) * lift + w * CD90 * dmath::sin(x);
        cl.push(lift);
        cm.push(cm_ac - arm * normal);
    }
    (top_deg, cl, cm)
}

/// A number as the shortest decimal of its `f32` value, rounded to `digits` decimals first.
fn num(v: f64, digits: i32) -> String {
    let scale = dmath::pow(10.0, f64::from(digits));
    let r = (v * scale).round() / scale;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{}", r as f32)
}

fn list(vs: &[f64], digits: i32) -> String {
    let items: Vec<String> = vs.iter().map(|v| num(*v, digits)).collect();
    format!("[{}]", items.join(", "))
}

fn curve(axis: &str, pts: &[(f64, f64)], digits: i32) -> String {
    let items: Vec<String> = pts
        .iter()
        .map(|(x, y)| format!("[{}, {}]", num(*x, 3), num(*y, digits)))
        .collect();
    format!(
        "{{ \"axis\": \"{axis}\", \"points\": [{}] }}",
        items.join(", ")
    )
}

fn grid(rows: &[Vec<f64>], indent: &str) -> String {
    let z: Vec<String> = rows
        .iter()
        .map(|r| format!("{indent}    {}", list(r, 4)))
        .collect();
    format!(
        "{{\n{indent}  \"axes\": [\"alpha_deg\", \"mach\"],\n{indent}  \"x\": {},\n{indent}  \"y\": {},\n{indent}  \"z\": [\n{}\n{indent}  ]\n{indent}}}",
        list(&ALPHA_DEG, 3),
        list(&MACH, 3),
        z.join(",\n")
    )
}

fn at_mach(f: impl Fn(usize) -> f64) -> Vec<(f64, f64)> {
    (0..MACH.len()).map(|j| (MACH[j], f(j))).collect()
}

/// The full text of `kestrel.json`.
pub fn render() -> String {
    let rows: Vec<(f64, Vec<f64>, Vec<f64>)> = (0..MACH.len()).map(row).collect();
    let cl_rows: Vec<Vec<f64>> = rows.iter().map(|r| r.1.clone()).collect();
    let cm_rows: Vec<Vec<f64>> = rows.iter().map(|r| r.2.clone()).collect();
    let blend_start = at_mach(|j| rows[j].0 + 10.0);
    let blend_end = at_mach(|j| rows[j].0 + 30.0);
    let cl_de = at_mach(|j| -PITCH[j].2 / TAIL_ARM_MAC);
    let cm_de = at_mach(|j| PITCH[j].2);
    let flat = |v: f64| [(0.0, v), (3.0, v)];
    format!(
        r#"{{
  "schema": "aero/1",
  "id": "core:aero/kestrel",
  "lift": {{
    "cl_base": {},
    "cl_max": {},
    "cl_alpha_per_rad": {},
    "cl0": 0,
    "cl_de_per_rad": {}
  }},
  "drag": {{
    "cd0": {},
    "cd_wave": {},
    "k_induced": {}
  }},
  "pitch": {{
    "cm_0": {},
    "cm_de_per_rad": {},
    "cm_q_per_rad": -9,
    "cm_alpha_dot_per_rad": -3
  }},
  "lateral": {{
    "cy_beta_per_rad": -0.9,
    "cl_beta_per_rad": -0.1,
    "cn_beta_per_rad": 0.14,
    "cl_p_per_rad": -0.3,
    "cl_r_per_rad": 0.1,
    "cn_p_per_rad": -0.03,
    "cn_r_per_rad": -0.3,
    "cl_da_per_rad": 0.16,
    "cn_da_per_rad": -0.01,
    "cl_dr_per_rad": 0.01,
    "cn_dr_per_rad": -0.07,
    "cy_dr_per_rad": 0.2
  }},
  "post_stall": {{
    "blend_start_deg": {},
    "blend_end_deg": {},
    "cd90": {},
    "x_ac_post_mac": {}
  }},
  "control_effectiveness": {},
  "config": {{
    "gear_cd0": 0.015,
    "flap_to_cd0": 0.02,
    "flap_ld_cd0": 0.03,
    "brake_cd0": 0.02,
    "flap_to_cl": 0.3,
    "flap_ld_cl": 0.3
  }},
  "x_ref_mac": {},
  "ground_effect_k": 0.1
}}
"#,
        grid(&cl_rows, "    "),
        curve("mach", &CL_MAX, 4),
        curve("mach", &CL_ALPHA, 4),
        curve("mach", &cl_de, 5),
        curve("mach", &flat(0.0235), 4),
        curve("mach", &CD_WAVE, 4),
        curve("mach", &flat(0.135), 4),
        grid(&cm_rows, "    "),
        curve("mach", &cm_de, 4),
        curve("mach", &blend_start, 2),
        curve("mach", &blend_end, 2),
        num(CD90, 3),
        num(X_AC_POST_MAC, 3),
        curve(
            "deflection_deg",
            &[
                (0.0, 1.0),
                (15.0, 1.0),
                (20.0, 0.95),
                (25.0, 0.9),
                (30.0, 0.85)
            ],
            3
        ),
        num(X_REF_MAC, 3),
    )
}
