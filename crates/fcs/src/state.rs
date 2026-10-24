//! The state of the control system: everything that evolves between ticks (hashed, `Copy`).

/// The assist level (GDD 11.11.3). The numbers are the `assist_level` codes of the ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Assist {
    /// Flight-path-rate and bank-angle commands, every limiter.
    Full = 1,
    /// Load-factor and roll-rate commands with auto-coordination (the default).
    Standard = 2,
    /// Pitch-rate and roll-rate commands, a yaw damper, no hard limiters.
    Reduced = 3,
    /// The stick moves the surfaces.
    Manual = 4,
}

impl Assist {
    /// The ABI code.
    #[must_use]
    pub fn code(self) -> u8 {
        self as u8
    }

    /// The level of an ABI code.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Full),
            2 => Some(Self::Standard),
            3 => Some(Self::Reduced),
            4 => Some(Self::Manual),
            _ => None,
        }
    }
}

/// What the control system is doing. The numbers are the `fcs_mode` codes of the ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The assist law in flight.
    Flight = 0,
    /// The ground law on the wheels.
    Ground = 1,
    /// Blending between the ground and flight laws, or between two assist levels.
    Blend = 2,
    /// The recovery button has control.
    Recover = 3,
    /// Automatic ground-collision avoidance has control.
    Gcas = 4,
    /// The pilot is incapacitated.
    Gloc = 5,
}

/// Which limiters are holding the command back this tick (the warning strip reads them).
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent flags, one per snapshot bit"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limiting {
    /// The angle-of-attack limiter.
    pub alpha: bool,
    /// The load-factor limiter.
    pub g: bool,
    /// The overspeed wall.
    pub overspeed: bool,
}

/// How far the recovery button has got.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoverPhase {
    /// Not engaged.
    Idle,
    /// Rolling the wings level (Full, Standard) or applying anti-spin controls (Reduced, Manual).
    Levelling,
    /// Pulling up to the recovery flight-path angle.
    Pulling,
}

/// The control system's memory.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent latched conditions; the layout is the hashed word order"
)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FcsState {
    /// The assist level.
    pub assist: Assist,
    /// What it is doing.
    pub mode: Mode,
    /// Weight on wheels, debounced.
    pub on_ground: bool,
    /// How long the raw weight-on-wheels signal has disagreed with `on_ground`, s.
    pub wow_timer_s: f64,
    /// Weight of the ground law in the command: 1 on the wheels, 0 in the air.
    pub ground_weight: f64,
    /// The flight law's filters and integrators must start from the present state.
    pub init_pending: bool,
    /// Seconds left of the surface blend after an assist switch.
    pub blend_left_s: f64,
    /// The surface command (elevator, aileron, rudder) when the blend began, rad.
    pub blend_from: [f64; 3],
    /// The surface command last issued, rad.
    pub last_cmd: [f64; 3],
    /// The flight law is frozen at `held` from the first contact with the ground until the ground
    /// law has taken over: the impact is not the pilot's to fight.
    pub holding: bool,
    /// The flight law's surface command (elevator, aileron, rudder) when it was frozen, rad.
    pub held: [f64; 3],
    /// Prefiltered load-factor command, g.
    pub n_filt: f64,
    /// Integral of the load-factor error, g s.
    pub n_int: f64,
    /// The last load-factor command after limiting, g.
    pub n_cmd: f64,
    /// Prefiltered stability-axis roll-rate command, rad/s.
    pub p_filt: f64,
    /// Integral of the roll-rate error, rad.
    pub p_int: f64,
    /// Prefiltered pitch-rate command (Reduced), rad/s.
    pub q_filt: f64,
    /// Flight-path angle held while the stick is centred (Full), rad.
    pub gamma_ref: f64,
    /// High-passed yaw rate (the Reduced yaw damper), rad/s.
    pub r_wash: f64,
    /// The yaw rate the washout last saw, rad/s.
    pub r_prev: f64,
    /// Mouse-aim pitch stick after the ramp.
    pub aim_pitch: f64,
    /// Mouse-aim roll stick after the ramp.
    pub aim_roll: f64,
    /// The recovery button.
    pub recover: RecoverPhase,
    /// What is limiting the command.
    pub limiting: Limiting,
    /// Dynamic pressure at the last tick, Pa (for its rate of change).
    pub qbar_prev: f64,
    /// The aircraft has departed from controlled flight (a fast yaw about the velocity vector or a
    /// deep stall, [`crate::recover::departed`]) at the last tick.
    pub departed: bool,
}

impl FcsState {
    /// The number of words [`FcsState::words`] returns.
    pub const WORDS: usize = 32;

    /// A system at an assist level, on the ground or in the air.
    #[must_use]
    pub fn new(assist: Assist, on_ground: bool) -> Self {
        Self {
            assist,
            mode: if on_ground {
                Mode::Ground
            } else {
                Mode::Flight
            },
            on_ground,
            wow_timer_s: 0.0,
            ground_weight: if on_ground { 1.0 } else { 0.0 },
            init_pending: true,
            blend_left_s: 0.0,
            blend_from: [0.0; 3],
            last_cmd: [0.0; 3],
            holding: false,
            held: [0.0; 3],
            n_filt: 1.0,
            n_int: 0.0,
            n_cmd: 1.0,
            p_filt: 0.0,
            p_int: 0.0,
            q_filt: 0.0,
            gamma_ref: 0.0,
            r_wash: 0.0,
            r_prev: 0.0,
            aim_pitch: 0.0,
            aim_roll: 0.0,
            recover: RecoverPhase::Idle,
            limiting: Limiting::default(),
            qbar_prev: 0.0,
            departed: false,
        }
    }

    /// Whether every number of the state is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.words().iter().all(|w| {
            // The words are the bit patterns of the floats and small integers; a float that is
            // NaN or infinite has all of its exponent bits set.
            *w >> 52 & 0x7ff != 0x7ff
        })
    }

    /// Every number of the state as words, for the state hash.
    #[must_use]
    pub fn words(&self) -> [u64; Self::WORDS] {
        let b = |v: bool| u64::from(v);
        let f = |v: f64| v.to_bits();
        [
            u64::from(self.assist.code()),
            self.mode as u64,
            b(self.on_ground),
            f(self.wow_timer_s),
            f(self.ground_weight),
            b(self.init_pending),
            f(self.blend_left_s),
            f(self.blend_from[0]),
            f(self.blend_from[1]),
            f(self.blend_from[2]),
            f(self.last_cmd[0]),
            f(self.last_cmd[1]),
            f(self.last_cmd[2]),
            f(self.n_filt),
            f(self.n_int),
            f(self.n_cmd),
            f(self.p_filt),
            f(self.p_int),
            f(self.q_filt),
            f(self.gamma_ref),
            f(self.r_wash),
            f(self.r_prev),
            f(self.aim_pitch),
            f(self.aim_roll),
            self.recover as u64,
            b(self.limiting.alpha) | b(self.limiting.g) << 1 | b(self.limiting.overspeed) << 2,
            f(self.qbar_prev),
            b(self.holding),
            f(self.held[0]),
            f(self.held[1]),
            f(self.held[2]),
            b(self.departed),
        ]
    }
}
