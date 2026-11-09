//! What the bandit knows when it decides (GDD 19.9, ADR-47's perception stub).
//!
//! `sim-core` builds a [`Percept`] every tick: the bandit's own state (exact: it is its own
//! aircraft), the player as it last perceived it (from the GCI cue or by eye, already delayed by
//! the skill's perception delay and noised), the gun threat, a missile it has noticed, the
//! terrain clearance ahead, where home and its patrol are, and fresh random numbers.

use simmath::math::{DQuat, DVec3};

/// The bandit's own state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnView {
    /// Reference point, BF, m.
    pub pos_bf: DVec3,
    /// Velocity, BF, m/s.
    pub vel_bf: DVec3,
    /// Attitude, body to BF.
    pub att: DQuat,
    /// Calibrated airspeed, m/s.
    pub cas_mps: f64,
    /// True airspeed, m/s.
    pub tas_mps: f64,
    /// Height of the lowest point above the ground, m.
    pub agl_m: f64,
    /// Height above MSL, m.
    pub alt_msl_m: f64,
    /// Local up, BF.
    pub up_bf: DVec3,
    /// The airframe's load limit, g.
    pub n_max: f64,
}

/// Something the bandit perceives: where and how fast, how it knows, and how old the knowledge is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// Position, BF, m.
    pub pos_bf: DVec3,
    /// Velocity, BF, m/s.
    pub vel_bf: DVec3,
    /// Seen by eye (true) or told by GCI (false).
    pub visual: bool,
    /// How old it is, s.
    pub age_s: f64,
}

/// Random numbers drawn by `sim-core` for this tick: uniforms in `[0, 1)` and standard normals.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dice {
    /// Uniform.
    pub u: [f64; 4],
    /// Standard normal.
    pub g: [f64; 4],
}

/// Everything the bandit's brain may use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Percept {
    /// Simulation time, s.
    pub t_s: f64,
    /// The bandit itself.
    pub own: OwnView,
    /// The player, if known.
    pub contact: Option<Contact>,
    /// The player's antenna train angle on the bandit (how near its nose points at us), rad, when
    /// seen by eye.
    pub threat_ata_rad: Option<f64>,
    /// A missile it has noticed.
    pub missile: Option<Contact>,
    /// Its own gunsight's aim point on the player, BF, when it has one.
    pub gun_aim_point_bf: Option<DVec3>,
    /// The least terrain clearance along the path ahead, m.
    pub clearance_min_m: f64,
    /// Home (the battlefield anchor), BF.
    pub home_bf: DVec3,
    /// How far from home it may go, m.
    pub arena_radius_m: f64,
    /// How far from home it is, m.
    pub from_home_m: f64,
    /// The centre of its patrol orbit, BF.
    pub cap_centre_bf: DVec3,
    /// The orbit's radius, m.
    pub cap_radius_m: f64,
    /// The patrol height above MSL, m.
    pub cap_alt_msl_m: f64,
    /// Random numbers.
    pub dice: Dice,
}

fn finite3(v: DVec3) -> bool {
    v.is_finite()
}

fn contact_finite(c: Option<&Contact>) -> bool {
    c.is_none_or(|c| finite3(c.pos_bf) && finite3(c.vel_bf) && c.age_s.is_finite())
}

impl Percept {
    /// Every number in it is finite (AI6: a brain that sees anything else recovers).
    #[must_use]
    pub fn is_finite(&self) -> bool {
        let o = &self.own;
        let own = finite3(o.pos_bf)
            && finite3(o.vel_bf)
            && o.att.is_finite()
            && [o.cas_mps, o.tas_mps, o.agl_m, o.alt_msl_m, o.n_max]
                .iter()
                .all(|x| x.is_finite())
            && finite3(o.up_bf);
        let rest = self.t_s.is_finite()
            && contact_finite(self.contact.as_ref())
            && contact_finite(self.missile.as_ref())
            && self.threat_ata_rad.is_none_or(f64::is_finite)
            && self.gun_aim_point_bf.is_none_or(finite3)
            && [
                self.clearance_min_m,
                self.arena_radius_m,
                self.from_home_m,
                self.cap_radius_m,
                self.cap_alt_msl_m,
            ]
            .iter()
            .all(|x| x.is_finite())
            && finite3(self.home_bf)
            && finite3(self.cap_centre_bf)
            && self
                .dice
                .u
                .iter()
                .chain(&self.dice.g)
                .all(|x| x.is_finite());
        own && rest
    }

    /// The bandit's nose, BF.
    #[must_use]
    pub fn nose_bf(&self) -> DVec3 {
        self.own.att.rotate(DVec3::X)
    }
}
