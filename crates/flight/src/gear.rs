//! Landing gear, ground contact and the touchdown criteria (GDD 11.13, ADR-35).
//!
//! Each leg is a massless strut with a wheel at its end. Per RK4 stage the leg's contact point
//! (with the strut fully extended) is placed in BF, the ground below it is sampled through
//! [`FlightEnv::surface`], and the penetration along the surface normal is the strut compression:
//!
//! ```text
//! N = max(0, k x (1 + (x / x_max)^2) + c(xdot) xdot)      c: compression or extension damping
//! F_roll  = -mu_r N tanh(v_long / 0.2)                     rolling and braking add
//! F_brake = -mu_b b N tanh(v_long / 0.05)                  mu_b x 0.9 with anti-skid
//! F_lat   = -mu_lat N tanh(v_lat / 0.5)                    smooth Coulomb
//! ```
//!
//! The steering angle turns the wheel's axes. The friction circle limits the tangential force to
//! `mu_peak N`. Airframe contact points (tail, wingtips, belly) and collapsed legs push back like
//! a stiff spring and drag with a skid friction of 0.5. Everything is a pure function of the
//! state, so it runs in every RK4 stage; state changes (touchdown, hard landing, collapse,
//! destruction) are decided once per sub-step by [`events`].
//!
//! Signs: the compression `x` is positive when compressed and `xdot > 0` means compressing;
//! steering is positive when the nose wheel turns right; brake commands are 0 to 1 per wheel.

use content::AircraftDef;
use simmath::dmath;
use simmath::math::DVec3;

use crate::body::RigidState;
use crate::env::FlightEnv;
use crate::error::ModelError;
use crate::frames::{Euler, attitude};
use crate::mass::MassProps;
use crate::surface::{Friction, SurfaceSample, friction};

/// Most gear legs an aircraft may have.
pub const MAX_GEAR: usize = 4;
/// Most airframe contact points an aircraft may have.
pub const MAX_CONTACTS: usize = 8;
/// Leg slots plus contact slots: the size of the per-point contact flags.
pub const MAX_SLOTS: usize = MAX_GEAR + MAX_CONTACTS;

/// Rolling-resistance regularisation speed, m/s. // ALGO: tanh scale; λ·dt ≈ 1 at the 120 Hz step
const V_S_ROLL: f64 = 0.2;
/// Braking regularisation speed, m/s. // ALGO: GDD 11.13.2 "smooth Coulomb"; small enough to hold
const V_S_BRAKE: f64 = 0.05;
/// Sideways regularisation speed, m/s. // DATA: GDD 11.13.2
const V_S_LAT: f64 = 0.5;
/// Skid regularisation speed, m/s. // ALGO: as the sideways one
const V_S_SKID: f64 = 0.5;
/// Brake scale with anti-skid on. // DATA: GDD 11.13.2
pub const ANTI_SKID_FACTOR: f64 = 0.9;
/// Brake scale when the wheels lock up (anti-skid off, hard braking, fast). // DATA: GDD 11.13.2
const LOCK_UP_FACTOR: f64 = 0.6;
/// Brake command above which a wheel locks up without anti-skid. // DATA: GDD 11.13.2
const LOCK_UP_BRAKE: f64 = 0.8;
/// Ground speed above which a wheel locks up without anti-skid, m/s. // DATA: GDD 11.13.2
const LOCK_UP_SPEED_MPS: f64 = 5.0;
/// Friction of a fuselage, wing or collapsed strut dragging on the ground. // DATA: ADR-35
const SKID_MU: f64 = 0.5;
/// The gear counts as down and locked from this extension.
pub const GEAR_DOWN_MIN: f64 = 0.99;
/// Below this extension the gear counts as up; between the two it is in transit.
pub const GEAR_UP_MAX: f64 = 0.02;
/// How far a retracting wheel travels from its extended position, m. // ALGO: ADR-35
const RETRACT_TRAVEL_M: f64 = 0.8;
/// The stop spring a bottomed strut presses against is this many times the strut's.
const STOP_STIFFNESS_RATIO: f64 = 10.0;
/// A strut pushed this far past its stroke collapses, m. // DATA: ADR-35
const COLLAPSE_BEYOND_M: f64 = 0.05;
/// Sink rate at first contact up to which a landing is normal, m/s. // DATA: GDD 11.13.3
pub const SINK_NORMAL_MPS: f64 = 3.0;
/// Sink rate above which the strut collapses, m/s. // DATA: GDD 11.13.3
pub const SINK_COLLAPSE_MPS: f64 = 4.5;
/// Sink rate above which the aircraft is destroyed, m/s. // DATA: GDD 11.13.3
pub const SINK_DESTROY_MPS: f64 = 7.0;
/// Ground speed above which fuselage, wing or collapsed-strut contact destroys the aircraft, m/s.
/// // DATA: GDD 11.13.3
pub const SKID_DESTROY_MPS: f64 = 30.0;
/// The CG this far below the ground is a numerical tunnelling, not a crash, m.
pub const TUNNEL_DEPTH_M: f64 = 5.0;
/// Weight on wheels needs a ground speed below this, m/s. // DATA: GDD 11.13.4
pub const WOW_SPEED_MAX_MPS: f64 = 120.0;
/// Time off the ground before the next contact counts as a new touchdown, s. // ALGO: ADR-35
const TOUCHDOWN_REARM_S: f64 = 0.5;
/// How long the hard-landing flag stays up, s. // ALGO: ADR-35
pub const HARD_LANDING_LATCH_S: f64 = 3.0;
/// Natural frequency of the airframe's contact spring, rad/s. // ALGO: ADR-35
const AIRFRAME_OMEGA_RPS: f64 = 15.0;
/// Damping ratio of the airframe's contact spring. // ALGO: ADR-35
const AIRFRAME_ZETA: f64 = 0.6;
/// The ground normal is never allowed to be flatter than this against local up.
const MIN_NORMAL_UP: f64 = 0.05;

/// One landing-gear leg, in the flight model's units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearLeg {
    /// Wheel contact with the strut fully extended, m (body axes from the reference point).
    pub pos_ext_body_m: DVec3,
    /// Strut travel, m.
    pub stroke_m: f64,
    /// Spring rate, N/m.
    pub k_npm: f64,
    /// Damping while compressing, N s/m.
    pub c_comp_nspm: f64,
    /// Damping while extending, N s/m.
    pub c_ext_nspm: f64,
    /// Largest steering angle, rad (0 if the leg does not steer).
    pub steer_max_rad: f64,
    /// True if the wheel has a brake.
    pub braked: bool,
    /// True for a main leg.
    pub main: bool,
}

impl GearLeg {
    const UNUSED: Self = Self {
        pos_ext_body_m: DVec3::ZERO,
        stroke_m: 1.0,
        k_npm: 0.0,
        c_comp_nspm: 0.0,
        c_ext_nspm: 0.0,
        steer_max_rad: 0.0,
        braked: false,
        main: false,
    };
}

/// The gear of one airframe: legs and contact points, built once from content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearModel {
    legs: [GearLeg; MAX_GEAR],
    n_legs: usize,
    contacts_body_m: [DVec3; MAX_CONTACTS],
    n_contacts: usize,
    reach_m: f64,
    /// Anti-skid braking (GDD 11.13.2). On unless a test turns it off.
    pub anti_skid: bool,
}

/// The pilot's wheel commands, held over a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelCommands {
    /// Brake command per leg, 0 to 1 (ignored for legs without a brake).
    pub brake: [f64; MAX_GEAR],
    /// Nose-wheel steering angle, rad, positive to the right.
    pub steer_rad: f64,
}

impl WheelCommands {
    /// Brakes off, wheel straight.
    pub const NONE: Self = Self {
        brake: [0.0; MAX_GEAR],
        steer_rad: 0.0,
    };

    /// Every command in range, with NaN treated as neutral.
    #[must_use]
    pub fn sanitized(&self) -> Self {
        let mut out = *self;
        for b in &mut out.brake {
            *b = if b.is_nan() { 0.0 } else { b.clamp(0.0, 1.0) };
        }
        if out.steer_rad.is_nan() {
            out.steer_rad = 0.0;
        }
        out
    }
}

/// What the ground model remembers between sub-steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundState {
    /// Legs that have collapsed (latched until the aircraft is respawned).
    pub collapsed: [bool; MAX_GEAR],
    /// Which legs and contact points touched the ground at the end of the last sub-step: legs
    /// first, then contact points.
    pub in_contact: [bool; MAX_SLOTS],
    /// Weight on wheels at the end of the last sub-step (GDD 11.13.4).
    pub wow: bool,
    /// Time since any wheel last touched the ground, s.
    pub air_time_s: f64,
    /// Sink rate at the last touchdown, m/s.
    pub touchdown_vs_mps: f64,
    /// Time left on the hard-landing flag, s.
    pub hard_landing_s: f64,
    /// A fuselage, wing or collapsed strut is dragging on the ground.
    pub skid: bool,
    /// The steering angle in force, rad.
    pub steer_rad: f64,
}

impl GroundState {
    /// In the air, nothing touched, nothing broken.
    pub const AIRBORNE: Self = Self {
        collapsed: [false; MAX_GEAR],
        in_contact: [false; MAX_SLOTS],
        wow: false,
        air_time_s: 1.0e3,
        touchdown_vs_mps: 0.0,
        hard_landing_s: 0.0,
        skid: false,
        steer_rad: 0.0,
    };
}

/// Why the aircraft was destroyed on the ground (the codes of `layout::crash_cause`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CrashCause {
    /// Fuselage, wing or stores contact too fast.
    AirframeImpact = 1,
    /// Sink rate at first gear contact above the limit.
    SinkRate = 2,
    /// A strut collapsed at speed, so the aircraft is on its belly too fast.
    GearCollapse = 3,
    /// The CG went through the ground (a numerical, not a physical, event).
    Tunnelling = 4,
}

impl CrashCause {
    /// The number carried in the snapshot.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// What the ground did during a sub-step or a tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroundEvents {
    /// Touchdowns: from no contact to contact after at least half a second in the air.
    pub touchdowns: u32,
    /// Hard landings (sink 3.0 to 4.5 m/s).
    pub hard_landings: u32,
    /// Struts that collapsed.
    pub collapses: u32,
    /// The aircraft was destroyed, and why.
    pub destroyed: Option<CrashCause>,
    /// The largest sink rate at a touchdown, m/s.
    pub touchdown_vs_mps: f64,
}

impl GroundEvents {
    /// Nothing happened.
    pub const NONE: Self = Self {
        touchdowns: 0,
        hard_landings: 0,
        collapses: 0,
        destroyed: None,
        touchdown_vs_mps: 0.0,
    };

    /// Adds the events of a later sub-step.
    pub fn merge(&mut self, o: &Self) {
        self.touchdowns += o.touchdowns;
        self.hard_landings += o.hard_landings;
        self.collapses += o.collapses;
        if self.destroyed.is_none() {
            self.destroyed = o.destroyed;
        }
        self.touchdown_vs_mps = self.touchdown_vs_mps.max(o.touchdown_vs_mps);
    }
}

/// The ground's force and moment on the aircraft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundForces {
    /// Force, body axes, N.
    pub force_body_n: DVec3,
    /// Moment about the CG, body axes, N m.
    pub moment_body_nm: DVec3,
    /// Normal force of each leg, N.
    pub n_leg_n: [f64; MAX_GEAR],
    /// Sum of the main legs' normal forces, N.
    pub n_main_n: f64,
    /// Sum of every normal force, legs and airframe, N.
    pub n_total_n: f64,
}

impl GroundForces {
    /// No contact.
    pub const ZERO: Self = Self {
        force_body_n: DVec3::ZERO,
        moment_body_nm: DVec3::ZERO,
        n_leg_n: [0.0; MAX_GEAR],
        n_main_n: 0.0,
        n_total_n: 0.0,
    };
}

/// What the ground model reads of the aircraft besides its rigid state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearHeld {
    /// Gear extension, 0 up to 1 down.
    pub gear_pos: f64,
    /// Brakes and steering.
    pub wheels: WheelCommands,
    /// Collapsed legs.
    pub collapsed: [bool; MAX_GEAR],
}

impl GearHeld {
    /// The gear down, brakes off, nothing broken.
    pub const DOWN: Self = Self {
        gear_pos: 1.0,
        wheels: WheelCommands::NONE,
        collapsed: [false; MAX_GEAR],
    };
}

impl GearModel {
    /// Builds the gear from an aircraft's geometry.
    ///
    /// # Errors
    /// [`ModelError::TooMany`] for more legs or contact points than the model has room for;
    /// [`ModelError::Invalid`] for a leg with no stroke or spring.
    pub fn from_def(ac: &AircraftDef) -> Result<Self, ModelError> {
        let g = &ac.geometry;
        if g.gear.len() > MAX_GEAR {
            return Err(ModelError::TooMany {
                what: "gear legs",
                found: g.gear.len(),
                max: MAX_GEAR,
            });
        }
        if g.contacts.len() > MAX_CONTACTS {
            return Err(ModelError::TooMany {
                what: "contact points",
                found: g.contacts.len(),
                max: MAX_CONTACTS,
            });
        }
        let v3 = |a: [f32; 3]| DVec3::new(f64::from(a[0]), f64::from(a[1]), f64::from(a[2]));
        let mut legs = [GearLeg::UNUSED; MAX_GEAR];
        let mut reach = 0.0_f64;
        for (slot, l) in legs.iter_mut().zip(&g.gear) {
            if !(l.stroke_m > 0.0 && l.k_npm > 0.0) {
                return Err(ModelError::Invalid("gear leg"));
            }
            *slot = GearLeg {
                pos_ext_body_m: v3(l.pos_m),
                stroke_m: f64::from(l.stroke_m),
                k_npm: f64::from(l.k_npm),
                c_comp_nspm: f64::from(l.c_comp_nspm),
                c_ext_nspm: f64::from(l.c_ext_nspm),
                steer_max_rad: f64::from(l.steer_max_deg).to_radians(),
                braked: l.brake,
                main: l.main,
            };
            reach = reach.max(slot.pos_ext_body_m.length());
        }
        let mut contacts = [DVec3::ZERO; MAX_CONTACTS];
        for (slot, c) in contacts.iter_mut().zip(&g.contacts) {
            *slot = v3(c.pos_m);
            reach = reach.max(slot.length());
        }
        // The CG moves with fuel and loadout; a metre of allowance covers it, and the strut
        // stroke and the wheel radius are inside the extended position already.
        Ok(Self {
            legs,
            n_legs: g.gear.len(),
            contacts_body_m: contacts,
            n_contacts: g.contacts.len(),
            reach_m: reach + 2.0,
            anti_skid: true,
        })
    }

    /// A model with no gear at all (the aircraft cannot touch the ground without crashing).
    #[must_use]
    pub fn none() -> Self {
        Self {
            legs: [GearLeg::UNUSED; MAX_GEAR],
            n_legs: 0,
            contacts_body_m: [DVec3::ZERO; MAX_CONTACTS],
            n_contacts: 0,
            reach_m: 0.0,
            anti_skid: true,
        }
    }

    /// The legs.
    #[must_use]
    pub fn legs(&self) -> &[GearLeg] {
        &self.legs[..self.n_legs]
    }

    /// The airframe contact points, m (body axes from the reference point).
    #[must_use]
    pub fn contacts(&self) -> &[DVec3] {
        &self.contacts_body_m[..self.n_contacts]
    }

    /// True if there is no gear and no contact point.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.n_legs == 0 && self.n_contacts == 0
    }

    /// The distance from the CG beyond which no part of the gear can touch the ground, m.
    #[must_use]
    pub fn reach_m(&self) -> f64 {
        self.reach_m
    }
}

/// Where a point of the aircraft is, how it moves, and how deep it is in the ground.
struct Probe {
    /// Offset from the CG in BF, m.
    r_bf: DVec3,
    /// Velocity of the point in BF, m/s.
    v_bf: DVec3,
    /// Ground normal.
    n: DVec3,
    /// Penetration along the normal, m (positive below the ground).
    depth: f64,
    /// Rate of penetration, m/s (positive going in).
    depth_rate: f64,
    surface: SurfaceSample,
}

fn probe<E: FlightEnv>(st: &RigidState, r_body: DVec3, env: &E) -> Probe {
    let r_bf = st.att.rotate(r_body);
    let p_bf = st.pos_bf + r_bf;
    let v_bf = st.vel_bf + st.att.rotate(st.omega_body.cross(r_body));
    let surface = env.surface(p_bf);
    let up = env.up_bf(p_bf);
    let n = surface.normal_bf;
    let cos = n.dot(up).max(MIN_NORMAL_UP);
    Probe {
        r_bf,
        v_bf,
        n,
        depth: (surface.height_msl_m - env.height_msl_m(p_bf)) * cos,
        depth_rate: -v_bf.dot(n),
        surface,
    }
}

/// The normal force of a strut compressed by `x` and compressing at `xdot`.
fn strut_normal(leg: &GearLeg, x: f64, xdot: f64) -> f64 {
    let u = x / leg.stroke_m;
    let c = if xdot >= 0.0 {
        leg.c_comp_nspm
    } else {
        leg.c_ext_nspm
    };
    let mut n = leg.k_npm * x * (1.0 + u * u) + c * xdot;
    if x > leg.stroke_m {
        n += STOP_STIFFNESS_RATIO * leg.k_npm * (x - leg.stroke_m);
    }
    n.max(0.0)
}

/// Everything the tangential force of one wheel needs.
struct Wheel {
    /// Normal force, N.
    n_force: f64,
    /// Ground normal.
    n: DVec3,
    /// The wheel's rolling direction on the ground (unit, perpendicular to `n`).
    e_long: DVec3,
    /// Velocity of the contact point relative to the ground, BF.
    v_bf: DVec3,
    friction: Friction,
    /// Brake command, 0 to 1.
    brake: f64,
    anti_skid: bool,
}

/// Rolling, braking and sideways friction with the friction circle, as a BF force.
fn wheel_friction(w: &Wheel) -> DVec3 {
    let e_side = w.e_long.cross(w.n);
    let v_t = w.v_bf - w.n * w.v_bf.dot(w.n);
    let (v_long, v_side) = (v_t.dot(w.e_long), v_t.dot(e_side));
    let speed = v_t.length();
    let mu_b = if w.anti_skid {
        w.friction.mu_brake * ANTI_SKID_FACTOR
    } else if w.brake > LOCK_UP_BRAKE && speed > LOCK_UP_SPEED_MPS {
        w.friction.mu_brake * LOCK_UP_FACTOR
    } else {
        w.friction.mu_brake
    };
    let f_long = -w.n_force
        * (w.friction.mu_roll * dmath::tanh(v_long / V_S_ROLL)
            + mu_b * w.brake * dmath::tanh(v_long / V_S_BRAKE));
    let f_side = -w.n_force * w.friction.mu_lat * dmath::tanh(v_side / V_S_LAT);
    // The friction circle (GDD 11.13.2): the tyre cannot give more than the surface holds.
    let mag = dmath::hypot(f_long, f_side);
    let limit = w.friction.mu_lat.max(w.friction.mu_roll) * w.n_force;
    let scale = if mag > limit && mag > 0.0 {
        limit / mag
    } else {
        1.0
    };
    (w.e_long * f_long + e_side * f_side) * scale
}

/// The wheel's rolling direction: body forward turned by the steering angle, projected onto the
/// ground plane.
fn rolling_direction(st: &RigidState, steer_rad: f64, n: DVec3) -> DVec3 {
    let dir_body = DVec3::new(dmath::cos(steer_rad), dmath::sin(steer_rad), 0.0);
    let dir_bf = st.att.rotate(dir_body);
    let on_plane = dir_bf - n * dir_bf.dot(n);
    let fallback = {
        let fwd = st.att.rotate(DVec3::X);
        fwd - n * fwd.dot(n)
    };
    on_plane.normalized_or(fallback.normalized_or(DVec3::X))
}

/// A fuselage, wing, stores or collapsed-strut contact: a stiff spring and a skid.
fn skid_force(p: &Probe, mass_kg: f64) -> (f64, DVec3) {
    let k = mass_kg * AIRFRAME_OMEGA_RPS * AIRFRAME_OMEGA_RPS;
    let c = 2.0 * AIRFRAME_ZETA * AIRFRAME_OMEGA_RPS * mass_kg;
    let n_force = (k * p.depth + c * p.depth_rate).max(0.0);
    let v_t = p.v_bf - p.n * p.v_bf.dot(p.n);
    let speed = v_t.length();
    let drag = if speed > 1e-9 {
        -v_t * (SKID_MU * n_force * dmath::tanh(speed / V_S_SKID) / speed)
    } else {
        DVec3::ZERO
    };
    (n_force, p.n * n_force + drag)
}

/// The ground's force and moment on the aircraft in state `st` (a pure function of the state, so
/// every RK4 stage can call it). `height_agl_cg_m` is the CG's height above the ground; far above
/// the gear's reach nothing is evaluated.
#[must_use]
pub fn forces<E: FlightEnv>(
    model: &GearModel,
    st: &RigidState,
    mass: &MassProps,
    held: &GearHeld,
    env: &E,
    height_agl_cg_m: f64,
) -> GroundForces {
    if model.is_empty() || height_agl_cg_m.is_nan() || height_agl_cg_m > model.reach_m {
        return GroundForces::ZERO;
    }
    let mut out = GroundForces::ZERO;
    let mut force_bf = DVec3::ZERO;
    let mut moment_bf = DVec3::ZERO;
    let down = held.gear_pos >= GEAR_DOWN_MIN;
    let cg = mass.cg_body_m;
    let mut add = |r_bf: DVec3, f_bf: DVec3| {
        force_bf += f_bf;
        moment_bf += r_bf.cross(f_bf);
    };

    for (i, leg) in model.legs().iter().enumerate() {
        if !down {
            continue;
        }
        if held.collapsed[i] {
            // A collapsed strut is a skid at its compressed position.
            let r = leg.pos_ext_body_m - DVec3::new(0.0, 0.0, leg.stroke_m) - cg;
            let p = probe(st, r, env);
            if p.depth > 0.0 {
                let (n_force, f) = skid_force(&p, mass.mass_kg);
                out.n_total_n += n_force;
                add(p.r_bf, f);
            }
            continue;
        }
        let p = probe(st, leg.pos_ext_body_m - cg, env);
        if p.depth <= 0.0 {
            continue;
        }
        let n_force = strut_normal(leg, p.depth, p.depth_rate);
        if n_force <= 0.0 {
            continue;
        }
        let steer = if leg.steer_max_rad > 0.0 {
            held.wheels
                .steer_rad
                .clamp(-leg.steer_max_rad, leg.steer_max_rad)
        } else {
            0.0
        };
        let wheel = Wheel {
            n_force,
            n: p.n,
            e_long: rolling_direction(st, steer, p.n),
            v_bf: p.v_bf,
            friction: friction(p.surface.kind, p.surface.wetness),
            brake: if leg.braked {
                held.wheels.brake[i]
            } else {
                0.0
            },
            anti_skid: model.anti_skid,
        };
        out.n_leg_n[i] = n_force;
        out.n_total_n += n_force;
        if leg.main {
            out.n_main_n += n_force;
        }
        add(p.r_bf, p.n * n_force + wheel_friction(&wheel));
    }

    for c in model.contacts() {
        let p = probe(st, *c - cg, env);
        if p.depth > 0.0 {
            let (n_force, f) = skid_force(&p, mass.mass_kg);
            out.n_total_n += n_force;
            add(p.r_bf, f);
        }
    }

    out.force_body_n = st.att.conjugate().rotate(force_bf);
    out.moment_body_nm = st.att.conjugate().rotate(moment_bf);
    out
}

/// Judges the airframe contact points at `post`: those in the ground are marked in `now` (after
/// the legs' slots) and an impact at speed or at a high sink rate destroys the aircraft. True if
/// any point is in contact (the aircraft is skidding).
fn airframe_contacts<E: FlightEnv>(
    model: &GearModel,
    post: &RigidState,
    cg: DVec3,
    env: &E,
    gs: f64,
    now: &mut [bool; MAX_SLOTS],
    ev: &mut GroundEvents,
) -> bool {
    let mut skid = false;
    for (j, c) in model.contacts().iter().enumerate() {
        let p = probe(post, *c - cg, env);
        if p.depth > 0.0 {
            now[MAX_GEAR + j] = true;
            skid = true;
            let sink = p.depth_rate.max(0.0);
            if gs > SKID_DESTROY_MPS || sink > SINK_DESTROY_MPS {
                ev.destroyed.get_or_insert(CrashCause::AirframeImpact);
            }
        }
    }
    skid
}

/// What the ground did between `pre` and `post` (one sub-step of `dt_s` seconds), updating the
/// remembered `ground` state. Contacts are judged at `post`; the sink rate of a leg that has just
/// touched is its velocity into the ground at `pre`, before the strut had slowed it.
#[allow(
    clippy::too_many_arguments,
    reason = "the two states bracket the sub-step; the rest is the model's context"
)]
#[must_use]
pub fn events<E: FlightEnv>(
    model: &GearModel,
    pre: &RigidState,
    post: &RigidState,
    mass: &MassProps,
    held: &GearHeld,
    ground: &mut GroundState,
    env: &E,
    dt_s: f64,
) -> GroundEvents {
    let mut ev = GroundEvents::NONE;
    ground.hard_landing_s = (ground.hard_landing_s - dt_s).max(0.0);
    let cg = mass.cg_body_m;
    let up = env.up_bf(post.pos_bf);
    let gs = (post.vel_bf - up * post.vel_bf.dot(up)).length();

    // The CG through the ground is a numerical failure, not a crash.
    let cg_agl = env.height_msl_m(post.pos_bf) - env.ground_height_m(post.pos_bf);
    if !model.is_empty() && cg_agl < -TUNNEL_DEPTH_M {
        ev.destroyed = Some(CrashCause::Tunnelling);
    }

    // The leading contact of a touchdown is the first after a spell in the air. Legs that arrive
    // later in the same landing (the nose coming down) are not judged as hard landings, though a
    // strut driven hard enough still collapses.
    let leading = !ground.in_contact.iter().any(|&b| b) && ground.air_time_s >= TOUCHDOWN_REARM_S;
    let mut now = [false; MAX_SLOTS];
    let in_transit = held.gear_pos > GEAR_UP_MAX && held.gear_pos < GEAR_DOWN_MIN;
    let down = held.gear_pos >= GEAR_DOWN_MIN;
    let mut max_sink = 0.0_f64;
    let mut new_contact = false;
    let mut skid_now = false;
    let mut main_wow = false;

    for (i, leg) in model.legs().iter().enumerate() {
        if held.collapsed[i] {
            if down {
                let r = leg.pos_ext_body_m - DVec3::new(0.0, 0.0, leg.stroke_m) - cg;
                if probe(post, r, env).depth > 0.0 {
                    skid_now = true;
                    if gs > SKID_DESTROY_MPS {
                        ev.destroyed.get_or_insert(CrashCause::GearCollapse);
                    }
                }
            }
            continue;
        }
        if in_transit {
            // A wheel on its way up or down meeting the ground tears off.
            let lift = (1.0 - held.gear_pos) * RETRACT_TRAVEL_M;
            let r = leg.pos_ext_body_m + DVec3::new(0.0, 0.0, -lift) - cg;
            if probe(post, r, env).depth > 0.0 {
                ground.collapsed[i] = true;
                ev.collapses += 1;
            }
            continue;
        }
        if !down {
            continue;
        }
        let p = probe(post, leg.pos_ext_body_m - cg, env);
        let touching = p.depth > 0.0;
        now[i] = touching;
        if touching && leg.main && gs < WOW_SPEED_MAX_MPS {
            main_wow = true;
        }
        if touching && !ground.in_contact[i] {
            // First contact of this leg: judge it by the speed it arrived with.
            let before = probe(pre, leg.pos_ext_body_m - cg, env);
            let sink = before.depth_rate.max(0.0);
            if leading {
                new_contact = true;
                max_sink = max_sink.max(sink);
            }
            if sink > SINK_DESTROY_MPS {
                ev.destroyed.get_or_insert(CrashCause::SinkRate);
            } else if sink > SINK_COLLAPSE_MPS {
                ground.collapsed[i] = true;
                ev.collapses += 1;
                if gs > SKID_DESTROY_MPS {
                    ev.destroyed.get_or_insert(CrashCause::GearCollapse);
                }
            }
        }
        if touching && p.depth > leg.stroke_m + COLLAPSE_BEYOND_M && !ground.collapsed[i] {
            // Driven through its stop: the strut gives way.
            ground.collapsed[i] = true;
            ev.collapses += 1;
            if gs > SKID_DESTROY_MPS {
                ev.destroyed.get_or_insert(CrashCause::GearCollapse);
            }
        }
    }

    skid_now |= airframe_contacts(model, post, cg, env, gs, &mut now, &mut ev);

    let touching_now = now.iter().any(|&b| b);
    if touching_now {
        if new_contact {
            ev.touchdowns += 1;
            ev.touchdown_vs_mps = max_sink;
            ground.touchdown_vs_mps = max_sink;
            if max_sink > SINK_NORMAL_MPS {
                ev.hard_landings += 1;
                ground.hard_landing_s = HARD_LANDING_LATCH_S;
            }
        }
        ground.air_time_s = 0.0;
    } else {
        ground.air_time_s += dt_s;
    }
    ground.in_contact = now;
    ground.wow = main_wow;
    ground.skid = skid_now;
    ev
}

/// The lowest point of the gear and airframe above the ground, m, along local up: the radar
/// altitude of the aircraft's feet. Negative when something is in the ground.
#[must_use]
pub fn lowest_point_agl_m<E: FlightEnv>(
    model: &GearModel,
    st: &RigidState,
    mass: &MassProps,
    env: &E,
) -> f64 {
    let cg = mass.cg_body_m;
    let mut lowest = f64::INFINITY;
    let mut consider = |r_body: DVec3| {
        let p_bf = st.pos_bf + st.att.rotate(r_body);
        lowest = lowest.min(env.height_msl_m(p_bf) - env.surface(p_bf).height_msl_m);
    };
    for leg in model.legs() {
        consider(leg.pos_ext_body_m - cg);
    }
    for c in model.contacts() {
        consider(*c - cg);
    }
    if lowest.is_finite() {
        lowest
    } else {
        env.height_msl_m(st.pos_bf) - env.ground_height_m(st.pos_bf)
    }
}

/// The CG height and pitch at which linear springs of the legs' stiffness carry `load_n` with no
/// pitching moment, on a flat floor at small angles: `depth_i = z_i - x_i pitch - h`.
fn linear_guess(model: &GearModel, cg: DVec3, load_n: f64) -> Option<(f64, f64)> {
    let (mut k, mut kx, mut kxx, mut kz, mut kxz) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for leg in model.legs() {
        let (x, z) = (leg.pos_ext_body_m.x - cg.x, leg.pos_ext_body_m.z - cg.z);
        k += leg.k_npm;
        kx += leg.k_npm * x;
        kxx += leg.k_npm * x * x;
        kz += leg.k_npm * z;
        kxz += leg.k_npm * x * z;
    }
    // [k kx; kx kxx] [h pitch] = [kz - load, kxz]
    let det = k * kxx - kx * kx;
    if det.abs() < 1e-9 * k * kxx.abs().max(1.0) {
        return None;
    }
    let (b1, b2) = (kz - load_n, kxz);
    Some(((b1 * kxx - kx * b2) / det, (k * b2 - kx * b1) / det))
}

/// A rigid state at rest on the gear.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StaticPose {
    /// The state: at rest, wheels on the ground.
    pub body: RigidState,
    /// Pitch attitude, rad (nose up positive).
    pub pitch_rad: f64,
    /// Strut compression of each leg, m.
    pub compression_m: [f64; MAX_GEAR],
    /// Remaining out-of-balance vertical force, N.
    pub residual_n: f64,
}

/// Where the aircraft sits at rest on its gear: the CG height and pitch at which the struts
/// carry `vertical_load_n` (weight minus lift) with no pitching moment about the CG.
///
/// `ground_bf` is a BF point on the ground below the CG; `heading_rad` points the nose. A 2-DOF
/// Newton iteration over the same [`forces`] the integrator uses, with a fixed number of steps
/// and no allocation. `None` if it does not settle (no gear, or load and geometry that cannot
/// balance).
#[must_use]
pub fn static_pose<E: FlightEnv>(
    model: &GearModel,
    mass: &MassProps,
    vertical_load_n: f64,
    heading_rad: f64,
    ground_bf: DVec3,
    env: &E,
) -> Option<StaticPose> {
    if model.legs().is_empty() || vertical_load_n.is_nan() || vertical_load_n <= 0.0 {
        return None;
    }
    let up = env.up_bf(ground_bf);
    let cg = mass.cg_body_m;
    let state = |h: f64, pitch: f64| RigidState {
        pos_bf: ground_bf + up * h,
        vel_bf: DVec3::ZERO,
        att: attitude(
            up,
            Euler {
                heading_rad,
                pitch_rad: pitch,
                roll_rad: 0.0,
            },
        ),
        omega_body: DVec3::ZERO,
    };
    let held = GearHeld::DOWN;
    let residual = |h: f64, pitch: f64| -> (f64, f64) {
        let st = state(h, pitch);
        let f = forces(model, &st, mass, &held, env, 0.0);
        (
            st.att.rotate(f.force_body_n).dot(up) - vertical_load_n,
            f.moment_body_nm.y,
        )
    };
    // Start from the balance of linear springs on a flat floor (small angles), where every leg
    // is in contact, and refine with the real forces.
    let (mut h, mut pitch) = linear_guess(model, cg, vertical_load_n)?;
    let (eh, ep) = (1e-5, 1e-6);
    for _ in 0..40 {
        let (r1, r2) = residual(h, pitch);
        if r1.abs() < 1e-6 * vertical_load_n && r2.abs() < 1e-3 * vertical_load_n {
            break;
        }
        let (a1, a2) = residual(h + eh, pitch);
        let (b1, b2) = residual(h, pitch + ep);
        let (j11, j21) = ((a1 - r1) / eh, (a2 - r2) / eh);
        let (j12, j22) = ((b1 - r1) / ep, (b2 - r2) / ep);
        let det = j11 * j22 - j12 * j21;
        if det.abs() < 1e-12 {
            return None;
        }
        let (dh, dp) = ((j22 * r1 - j12 * r2) / det, (j11 * r2 - j21 * r1) / det);
        // ALGO: damped Newton, so the first steps cannot fling the aircraft off the ground.
        let damp = (0.2 / dh.abs().max(1e-9))
            .min(1.0)
            .min((0.05 / dp.abs().max(1e-12)).min(1.0));
        h -= dh * damp;
        pitch -= dp * damp;
    }
    let (r1, r2) = residual(h, pitch);
    if !(r1.abs() < 1e-3 * vertical_load_n && r2.abs() < 1e-2 * vertical_load_n) {
        return None;
    }
    let body = state(h, pitch);
    let mut compression_m = [0.0; MAX_GEAR];
    for (c, leg) in compression_m.iter_mut().zip(model.legs()) {
        *c = probe(&body, leg.pos_ext_body_m - cg, env).depth.max(0.0);
    }
    Some(StaticPose {
        body,
        pitch_rad: pitch,
        compression_m,
        residual_n: r1,
    })
}

#[cfg(test)]
mod tests;
