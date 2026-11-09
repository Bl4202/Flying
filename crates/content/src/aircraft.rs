//! The aircraft entry (kind 1): airframe, mass, geometry, engines, limits, FCS data and the
//! authored performance envelope (GDD 12.2, Appendix A.2 and C).
//!
//! Positions are metres in body axes (x forward, y right, z down) from the airframe's reference
//! point. Floats are `f32`; the baker refuses authored values that `f32` cannot hold exactly.

use crate::codec::{Reader, Writer};
use crate::cpack::CpackError;
use crate::enums::{Category, EnvelopeConfig, Era, Metric, Requirement};
use crate::registry::ContentId;

/// One fuel tank.
#[derive(Clone, Debug, PartialEq)]
pub struct Tank {
    /// Name within the aircraft (`fwd`, `wing_l`, ...).
    pub id: String,
    /// Capacity, kg.
    pub capacity_kg: f32,
    /// Centroid, m (body axes).
    pub pos_m: [f32; 3],
    /// Feed order: lower feeds first; tanks with the same priority feed together, in proportion
    /// to their contents.
    pub feed_priority: u8,
}

/// Mass properties (GDD 11.9).
#[derive(Clone, Debug, PartialEq)]
pub struct Mass {
    /// Empty mass, kg.
    pub empty_kg: f32,
    /// Internal fuel, kg (the sum of the tank capacities).
    pub internal_fuel_kg: f32,
    /// Maximum take-off mass, kg.
    pub max_takeoff_kg: f32,
    /// Maximum landing mass, kg.
    pub max_landing_kg: f32,
    /// Empty centre of gravity, m.
    pub cg_empty_m: [f32; 3],
    /// Empty inertia about the empty CG (roll, pitch, yaw), kg m^2.
    pub inertia_kgm2: [f32; 3],
    /// Empty product of inertia `integral(x z dm)`, kg m^2.
    pub inertia_xz_kgm2: f32,
    /// Fuel density, kg/L.
    pub fuel_density_kgpl: f32,
    /// Fuel tanks.
    pub tanks: Vec<Tank>,
}

/// One landing-gear leg: a massless strut with a wheel at its end (GDD 11.13, A.2).
#[derive(Clone, Debug, PartialEq)]
pub struct GearLeg {
    /// Name within the aircraft (`nose`, `main_l`, ...).
    pub id: String,
    /// Wheel contact point with the leg fully extended, m (body axes).
    pub pos_m: [f32; 3],
    /// Strut travel, m.
    pub stroke_m: f32,
    /// Spring rate, N/m.
    pub k_npm: f32,
    /// Damping while compressing, N s/m.
    pub c_comp_nspm: f32,
    /// Damping while extending, N s/m.
    pub c_ext_nspm: f32,
    /// Largest nose-wheel steering angle, degrees (0 if the leg does not steer).
    pub steer_max_deg: f32,
    /// True if the wheel has a brake.
    pub brake: bool,
    /// True for a main leg (the ones that carry the weight on landing).
    pub main: bool,
}

/// A point of the airframe that can scrape the ground: nose, tail, wingtips, belly.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactPoint {
    /// Name within the aircraft.
    pub id: String,
    /// Position, m (body axes).
    pub pos_m: [f32; 3],
}

/// Reference geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// Wing reference area, m^2.
    pub wing_area_m2: f32,
    /// Wing span, m.
    pub span_m: f32,
    /// Mean aerodynamic chord, m.
    pub mac_m: f32,
    /// Aerodynamic moment reference point, m.
    pub ref_point_m: [f32; 3],
    /// Bounding box (length, span, height), m.
    pub bbox_m: [f32; 3],
    /// Landing-gear legs.
    pub gear: Vec<GearLeg>,
    /// Airframe contact points.
    pub contacts: Vec<ContactPoint>,
}

/// One engine installation.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineInstall {
    /// The engine entry.
    pub engine: ContentId,
    /// Number of engines of this type at this position.
    pub count: u8,
    /// Thrust point, m.
    pub position_m: [f32; 3],
}

/// Structural load limits that depend on the configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NMaxByConfig {
    /// Positive load limit with the gear extended, g.
    pub gear: f32,
    /// Positive load limit with the flaps out, g.
    pub flaps: f32,
}

/// Aerodynamic references and operating limits.
#[derive(Clone, Debug, PartialEq)]
pub struct AeroLimits {
    /// The aero entry holding the coefficient tables.
    pub tables: ContentId,
    /// Highest Mach the tables are meant for.
    pub max_mach: f32,
    /// Maximum operating Mach number.
    pub mmo: f32,
    /// Maximum operating speed, m/s.
    pub vmo_mps: f32,
    /// Service ceiling, m.
    pub service_ceiling_m: f32,
    /// Structural positive load limit, g.
    pub n_max: f32,
    /// Structural negative load limit, g (negative).
    pub n_min: f32,
    /// Positive load limits with the gear or the flaps out.
    pub n_max_by_config: NMaxByConfig,
    /// Gear-extended speed limit, m/s.
    pub gear_limit_mps: f32,
    /// Flap-extended speed limit, m/s.
    pub flap_limit_mps: f32,
    /// Static margin as a fraction of MAC (negative if unstable).
    pub static_margin_mac: f32,
}

/// One control-surface actuator (GDD 11.7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Actuator {
    /// Travel (minimum, maximum), degrees.
    pub range_deg: [f32; 2],
    /// Rate limit, degrees per second.
    pub rate_dps: f32,
    /// First-order time constant, s.
    pub tau_s: f32,
}

/// The airbrake, which moves as a fraction 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Airbrake {
    /// Rate limit, fraction per second.
    pub rate_per_s: f32,
    /// First-order time constant, s.
    pub tau_s: f32,
}

/// The landing-gear actuator: the gear travels from up to down in a fixed time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GearActuator {
    /// Time to travel the whole way, s.
    pub transit_s: f32,
}

/// Flight-control data (GDD 11.11): limits and actuators, and the handle of the gains.
#[derive(Clone, Debug, PartialEq)]
pub struct Fcs {
    /// True if the airframe needs stability augmentation to be flyable.
    pub sas_required: bool,
    /// Angle-of-attack limit, degrees.
    pub alpha_limit_deg: f32,
    /// Largest commanded load factor, g.
    pub n_cmd_max: f32,
    /// Largest commanded roll rate, degrees per second.
    pub roll_rate_max_dps: f32,
    /// Stabilator (pitch).
    pub stab: Actuator,
    /// Ailerons or flaperons (roll).
    pub aileron: Actuator,
    /// Rudder (yaw).
    pub rudder: Actuator,
    /// Leading-edge flaps.
    pub lef: Actuator,
    /// Trailing-edge flaps.
    pub tef: Actuator,
    /// Airbrake.
    pub airbrake: Airbrake,
    /// Landing gear.
    pub gear: GearActuator,
    /// The control-law gains (an `fcs` entry).
    pub gains: ContentId,
}

/// One authored performance target (GDD 11.19, C.9).
#[derive(Clone, Debug, PartialEq)]
pub struct EnvelopePoint {
    /// Name of the point.
    pub id: String,
    /// Aircraft mass, kg.
    pub mass_kg: f32,
    /// Height above MSL, m.
    pub alt_m: f32,
    /// Flight Mach number, for metrics measured at a given speed (`n_sust`, `turn_dps`).
    pub mach: Option<f32>,
    /// Configuration.
    pub config: EnvelopeConfig,
    /// What is measured.
    pub metric: Metric,
    /// Target value, in the metric's unit.
    pub value: f32,
    /// Tolerance, percent.
    pub tol_pct: f32,
    /// A capability the point needs, if any.
    pub requires: Option<Requirement>,
}

/// Most rails an aircraft may carry.
pub const MAX_RAILS: usize = 8;
/// Most hit capsules an aircraft may have.
pub const MAX_CAPSULES: usize = 16;

/// The internal gun (GDD 15.2): which weapon, how many rounds and where it points from.
#[derive(Clone, Debug, PartialEq)]
pub struct GunInstall {
    /// The gun's weapon entry.
    pub weapon: ContentId,
    /// Rounds loaded at the start of a sortie.
    pub rounds: u16,
    /// Muzzle position, m (body axes). The barrel points along the body's x axis.
    pub muzzle_m: [f32; 3],
    /// Angle between the barrel and the body's x axis in the pitch plane, mrad (nose up positive).
    pub harmonisation_mrad: f32,
}

/// A launch rail on one hardpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct Rail {
    /// Hardpoint number, 1-based.
    pub station: u8,
    /// Position of the store's centre, m (body axes).
    pub pos_m: [f32; 3],
    /// The store's weapon entry.
    pub store: ContentId,
}

/// Infrared signature of the aircraft, relative to the Kestrel's afterburner (GDD 15.7, C.11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IrSignature {
    /// Engine intensity at idle.
    pub engine_idle: f32,
    /// Engine intensity at military power.
    pub engine_mil: f32,
    /// Engine intensity in full afterburner.
    pub engine_ab: f32,
    /// Factor on the engine intensity seen from the side.
    pub aspect_side: f32,
    /// Factor on the engine intensity seen from the front.
    pub aspect_front: f32,
    /// Skin-heating intensity per Mach number squared.
    pub skin_per_mach2: f32,
}

/// One hit volume: a capsule (a segment with a radius) in body axes.
#[derive(Clone, Debug, PartialEq)]
pub struct CapsuleDef {
    /// Name within the aircraft (`fuselage`, `wing_l`, ...).
    pub id: String,
    /// One end of the segment, m.
    pub a_m: [f32; 3],
    /// The other end, m.
    pub b_m: [f32; 3],
    /// Radius, m.
    pub radius_m: f32,
}

/// How much punishment the aircraft takes (GDD 18, aggregate model) and its hit volumes.
#[derive(Clone, Debug, PartialEq)]
pub struct DamageDef {
    /// Hit points when new.
    pub hp: f32,
    /// Energy that removes one hit point, J.
    pub joules_per_hp: f32,
    /// The hit volumes.
    pub capsules: Vec<CapsuleDef>,
}

/// What the sim knows about one aircraft type.
#[derive(Clone, Debug, PartialEq)]
pub struct AircraftDef {
    /// Locale key of the display name.
    pub name_key: String,
    /// Role.
    pub category: Category,
    /// Technology generation.
    pub era: Era,
    /// Crew count.
    pub crew: u8,
    /// Hardpoint count.
    pub hardpoints: u8,
    /// Mass properties.
    pub mass: Mass,
    /// Reference geometry.
    pub geometry: Geometry,
    /// Engine installations.
    pub engines: Vec<EngineInstall>,
    /// Aerodynamic references and limits.
    pub aero: AeroLimits,
    /// Flight-control data.
    pub fcs: Fcs,
    /// Authored performance targets.
    pub envelope: Vec<EnvelopePoint>,
    /// The internal gun, if any.
    pub gun: Option<GunInstall>,
    /// Launch rails, at most [`MAX_RAILS`].
    pub rails: Vec<Rail>,
    /// Infrared signature, if authored.
    pub ir: Option<IrSignature>,
    /// Effective size to the eye, m (0 when not authored; GDD 16.6.2).
    pub visual_l_eff_m: f32,
    /// Damage points and hit volumes, if authored.
    pub damage: Option<DamageDef>,
}

impl AircraftDef {
    /// Total engine count.
    #[must_use]
    pub fn engine_count(&self) -> u32 {
        self.engines.iter().map(|e| u32::from(e.count)).sum()
    }

    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        w.str(&self.name_key)?;
        w.u8(self.category.code());
        w.u8(self.era.code());
        w.u8(self.crew);
        w.u8(self.hardpoints);

        let m = &self.mass;
        w.f32s(&[
            m.empty_kg,
            m.internal_fuel_kg,
            m.max_takeoff_kg,
            m.max_landing_kg,
        ]);
        w.f32s(&m.cg_empty_m);
        w.f32s(&m.inertia_kgm2);
        w.f32(m.inertia_xz_kgm2);
        w.f32(m.fuel_density_kgpl);
        w.count(m.tanks.len())?;
        for t in &m.tanks {
            w.str(&t.id)?;
            w.f32(t.capacity_kg);
            w.f32s(&t.pos_m);
            w.u8(t.feed_priority);
        }

        self.geometry.encode(w)?;

        w.count(self.engines.len())?;
        for e in &self.engines {
            w.handle(e.engine);
            w.u8(e.count);
            w.f32s(&e.position_m);
        }

        let a = &self.aero;
        w.handle(a.tables);
        w.f32s(&[
            a.max_mach,
            a.mmo,
            a.vmo_mps,
            a.service_ceiling_m,
            a.n_max,
            a.n_min,
            a.n_max_by_config.gear,
            a.n_max_by_config.flaps,
            a.gear_limit_mps,
            a.flap_limit_mps,
            a.static_margin_mac,
        ]);

        let f = &self.fcs;
        w.bool(f.sas_required);
        w.f32s(&[f.alpha_limit_deg, f.n_cmd_max, f.roll_rate_max_dps]);
        for act in [&f.stab, &f.aileron, &f.rudder, &f.lef, &f.tef] {
            w.f32s(&act.range_deg);
            w.f32(act.rate_dps);
            w.f32(act.tau_s);
        }
        w.f32(f.airbrake.rate_per_s);
        w.f32(f.airbrake.tau_s);
        w.f32(f.gear.transit_s);
        w.handle(f.gains);

        w.count(self.envelope.len())?;
        for p in &self.envelope {
            w.str(&p.id)?;
            w.f32(p.mass_kg);
            w.f32(p.alt_m);
            w.bool(p.mach.is_some());
            w.f32(p.mach.unwrap_or(0.0));
            w.u8(p.config.code());
            w.u8(p.metric.code());
            w.f32(p.value);
            w.f32(p.tol_pct);
            w.u8(p.requires.map_or(0, Requirement::code));
        }
        self.encode_armament(w)
    }

    /// The gun, rails, signature and damage data, written after everything else (cpack v4).
    fn encode_armament(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        w.bool(self.gun.is_some());
        if let Some(g) = &self.gun {
            w.handle(g.weapon);
            w.u16(g.rounds);
            w.f32s(&g.muzzle_m);
            w.f32(g.harmonisation_mrad);
        }
        check_count(
            self.rails.len(),
            MAX_RAILS,
            "an aircraft has too many rails",
        )?;
        w.count(self.rails.len())?;
        for rail in &self.rails {
            w.u8(rail.station);
            w.f32s(&rail.pos_m);
            w.handle(rail.store);
        }
        w.bool(self.ir.is_some());
        if let Some(i) = &self.ir {
            w.f32s(&[
                i.engine_idle,
                i.engine_mil,
                i.engine_ab,
                i.aspect_side,
                i.aspect_front,
                i.skin_per_mach2,
            ]);
        }
        w.f32(self.visual_l_eff_m);
        w.bool(self.damage.is_some());
        if let Some(d) = &self.damage {
            check_count(
                d.capsules.len(),
                MAX_CAPSULES,
                "an aircraft has too many hit capsules",
            )?;
            w.f32s(&[d.hp, d.joules_per_hp]);
            w.count(d.capsules.len())?;
            for c in &d.capsules {
                w.str(&c.id)?;
                w.f32s(&c.a_m);
                w.f32s(&c.b_m);
                w.f32(c.radius_m);
            }
        }
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let name_key = r.str()?;
        let category =
            Category::from_code(r.u8()?).ok_or(CpackError::Record("unknown aircraft category"))?;
        let era = Era::from_code(r.u8()?).ok_or(CpackError::Record("unknown era"))?;
        let crew = r.u8()?;
        let hardpoints = r.u8()?;
        let mass = Mass::decode(r)?;
        let geometry = Geometry::decode(r)?;
        let engines = (0..r.u8()?)
            .map(|_| {
                Ok(EngineInstall {
                    engine: r.handle()?,
                    count: r.u8()?,
                    position_m: r.vec3()?,
                })
            })
            .collect::<Result<Vec<_>, CpackError>>()?;
        let aero = AeroLimits {
            tables: r.handle()?,
            max_mach: r.f32()?,
            mmo: r.f32()?,
            vmo_mps: r.f32()?,
            service_ceiling_m: r.f32()?,
            n_max: r.f32()?,
            n_min: r.f32()?,
            n_max_by_config: NMaxByConfig {
                gear: r.f32()?,
                flaps: r.f32()?,
            },
            gear_limit_mps: r.f32()?,
            flap_limit_mps: r.f32()?,
            static_margin_mac: r.f32()?,
        };
        let fcs = Fcs::decode(r)?;
        let envelope = (0..r.u8()?)
            .map(|_| EnvelopePoint::decode(r))
            .collect::<Result<Vec<_>, CpackError>>()?;
        let (gun, rails, ir, visual_l_eff_m, damage) = decode_armament(r)?;
        Ok(Self {
            name_key,
            category,
            era,
            crew,
            hardpoints,
            mass,
            geometry,
            engines,
            aero,
            fcs,
            envelope,
            gun,
            rails,
            ir,
            visual_l_eff_m,
            damage,
        })
    }
}

/// The part of the record [`AircraftDef::encode_armament`] wrote.
#[allow(
    clippy::type_complexity,
    reason = "five fields handed straight to the one struct literal that wants them"
)]
fn decode_armament(
    r: &mut Reader<'_>,
) -> Result<
    (
        Option<GunInstall>,
        Vec<Rail>,
        Option<IrSignature>,
        f32,
        Option<DamageDef>,
    ),
    CpackError,
> {
    let gun = if r.bool()? {
        Some(GunInstall {
            weapon: r.handle()?,
            rounds: r.u16()?,
            muzzle_m: r.vec3()?,
            harmonisation_mrad: r.f32()?,
        })
    } else {
        None
    };
    let n_rails = usize::from(r.u8()?);
    check_count(n_rails, MAX_RAILS, "an aircraft has too many rails")?;
    let rails = (0..n_rails)
        .map(|_| {
            Ok(Rail {
                station: r.u8()?,
                pos_m: r.vec3()?,
                store: r.handle()?,
            })
        })
        .collect::<Result<Vec<_>, CpackError>>()?;
    let ir = if r.bool()? {
        Some(IrSignature {
            engine_idle: r.f32()?,
            engine_mil: r.f32()?,
            engine_ab: r.f32()?,
            aspect_side: r.f32()?,
            aspect_front: r.f32()?,
            skin_per_mach2: r.f32()?,
        })
    } else {
        None
    };
    let visual_l_eff_m = r.f32()?;
    let damage = if r.bool()? {
        let (hp, joules_per_hp) = (r.f32()?, r.f32()?);
        let n = usize::from(r.u8()?);
        check_count(n, MAX_CAPSULES, "an aircraft has too many hit capsules")?;
        let capsules = (0..n)
            .map(|_| {
                Ok(CapsuleDef {
                    id: r.str()?,
                    a_m: r.vec3()?,
                    b_m: r.vec3()?,
                    radius_m: r.f32()?,
                })
            })
            .collect::<Result<Vec<_>, CpackError>>()?;
        Some(DamageDef {
            hp,
            joules_per_hp,
            capsules,
        })
    } else {
        None
    };
    Ok((gun, rails, ir, visual_l_eff_m, damage))
}

fn check_count(n: usize, max: usize, what: &'static str) -> Result<(), CpackError> {
    if n > max {
        Err(CpackError::Record(what))
    } else {
        Ok(())
    }
}

impl Geometry {
    fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        w.f32s(&[self.wing_area_m2, self.span_m, self.mac_m]);
        w.f32s(&self.ref_point_m);
        w.f32s(&self.bbox_m);
        w.count(self.gear.len())?;
        for leg in &self.gear {
            w.str(&leg.id)?;
            w.f32s(&leg.pos_m);
            w.f32s(&[
                leg.stroke_m,
                leg.k_npm,
                leg.c_comp_nspm,
                leg.c_ext_nspm,
                leg.steer_max_deg,
            ]);
            w.bool(leg.brake);
            w.bool(leg.main);
        }
        w.count(self.contacts.len())?;
        for p in &self.contacts {
            w.str(&p.id)?;
            w.f32s(&p.pos_m);
        }
        Ok(())
    }

    fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let (wing_area_m2, span_m, mac_m) = (r.f32()?, r.f32()?, r.f32()?);
        let (ref_point_m, bbox_m) = (r.vec3()?, r.vec3()?);
        let gear = (0..r.u8()?)
            .map(|_| {
                Ok(GearLeg {
                    id: r.str()?,
                    pos_m: r.vec3()?,
                    stroke_m: r.f32()?,
                    k_npm: r.f32()?,
                    c_comp_nspm: r.f32()?,
                    c_ext_nspm: r.f32()?,
                    steer_max_deg: r.f32()?,
                    brake: r.bool()?,
                    main: r.bool()?,
                })
            })
            .collect::<Result<Vec<_>, CpackError>>()?;
        let contacts = (0..r.u8()?)
            .map(|_| {
                Ok(ContactPoint {
                    id: r.str()?,
                    pos_m: r.vec3()?,
                })
            })
            .collect::<Result<Vec<_>, CpackError>>()?;
        Ok(Self {
            wing_area_m2,
            span_m,
            mac_m,
            ref_point_m,
            bbox_m,
            gear,
            contacts,
        })
    }
}

impl Mass {
    fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let (empty_kg, internal_fuel_kg, max_takeoff_kg, max_landing_kg) =
            (r.f32()?, r.f32()?, r.f32()?, r.f32()?);
        let cg_empty_m = r.vec3()?;
        let inertia_kgm2 = r.vec3()?;
        let inertia_xz_kgm2 = r.f32()?;
        let fuel_density_kgpl = r.f32()?;
        let tanks = (0..r.u8()?)
            .map(|_| {
                Ok(Tank {
                    id: r.str()?,
                    capacity_kg: r.f32()?,
                    pos_m: r.vec3()?,
                    feed_priority: r.u8()?,
                })
            })
            .collect::<Result<Vec<_>, CpackError>>()?;
        Ok(Self {
            empty_kg,
            internal_fuel_kg,
            max_takeoff_kg,
            max_landing_kg,
            cg_empty_m,
            inertia_kgm2,
            inertia_xz_kgm2,
            fuel_density_kgpl,
            tanks,
        })
    }
}

impl Fcs {
    fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let sas_required = r.bool()?;
        let (alpha_limit_deg, n_cmd_max, roll_rate_max_dps) = (r.f32()?, r.f32()?, r.f32()?);
        let mut act = || -> Result<Actuator, CpackError> {
            Ok(Actuator {
                range_deg: [r.f32()?, r.f32()?],
                rate_dps: r.f32()?,
                tau_s: r.f32()?,
            })
        };
        let (stab, aileron, rudder, lef, tef) = (act()?, act()?, act()?, act()?, act()?);
        let airbrake = Airbrake {
            rate_per_s: r.f32()?,
            tau_s: r.f32()?,
        };
        let gear = GearActuator {
            transit_s: r.f32()?,
        };
        let gains = r.handle()?;
        Ok(Self {
            sas_required,
            alpha_limit_deg,
            n_cmd_max,
            roll_rate_max_dps,
            stab,
            aileron,
            rudder,
            lef,
            tef,
            airbrake,
            gear,
            gains,
        })
    }
}

impl EnvelopePoint {
    fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let id = r.str()?;
        let (mass_kg, alt_m) = (r.f32()?, r.f32()?);
        let has_mach = r.bool()?;
        let mach = r.f32()?;
        if !has_mach && mach.to_bits() != 0 {
            return Err(CpackError::Record("an absent envelope Mach is not zero"));
        }
        let mach = has_mach.then_some(mach);
        let config = EnvelopeConfig::from_code(r.u8()?)
            .ok_or(CpackError::Record("unknown envelope configuration"))?;
        let metric =
            Metric::from_code(r.u8()?).ok_or(CpackError::Record("unknown envelope metric"))?;
        let (value, tol_pct) = (r.f32()?, r.f32()?);
        let requires = match r.u8()? {
            0 => None,
            c => Some(
                Requirement::from_code(c)
                    .ok_or(CpackError::Record("unknown envelope requirement"))?,
            ),
        };
        Ok(Self {
            id,
            mass_kg,
            alt_m,
            mach,
            config,
            metric,
            value,
            tol_pct,
            requires,
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::codec::Strings;

    fn act(lo: f32, hi: f32, rate: f32, tau: f32) -> Actuator {
        Actuator {
            range_deg: [lo, hi],
            rate_dps: rate,
            tau_s: tau,
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "a fixture: one long literal that is clearer whole"
    )]
    /// A valid definition with the Kestrel's headline numbers, referring to an engine at handle
    /// `engine`, an aero set at handle `aero` and control gains at handle `gains`.
    pub(crate) fn sample(engine: ContentId, aero: ContentId, gains: ContentId) -> AircraftDef {
        AircraftDef {
            name_key: "ac.kestrel.name".into(),
            category: Category::LightFighter,
            era: Era::G4,
            crew: 1,
            hardpoints: 9,
            mass: Mass {
                empty_kg: 8400.0,
                internal_fuel_kg: 3100.0,
                max_takeoff_kg: 14500.0,
                max_landing_kg: 9500.0,
                cg_empty_m: [0.0; 3],
                inertia_kgm2: [13000.0, 72436.0, 82436.0],
                inertia_xz_kgm2: 1300.0,
                fuel_density_kgpl: 0.8,
                tanks: vec![
                    Tank {
                        id: "fwd".into(),
                        capacity_kg: 900.0,
                        pos_m: [1.65, 0.0, 0.0],
                        feed_priority: 2,
                    },
                    Tank {
                        id: "wing_l".into(),
                        capacity_kg: 550.0,
                        pos_m: [0.0, -2.2, 0.0],
                        feed_priority: 1,
                    },
                ],
            },
            geometry: Geometry {
                wing_area_m2: 28.0,
                span_m: 9.8,
                mac_m: 3.2,
                ref_point_m: [0.0; 3],
                bbox_m: [14.6, 9.8, 4.5],
                gear: vec![
                    GearLeg {
                        id: "nose".into(),
                        pos_m: [4.58, 0.0, 2.29],
                        stroke_m: 0.3,
                        k_npm: 130_000.0,
                        c_comp_nspm: 9000.0,
                        c_ext_nspm: 18_000.0,
                        steer_max_deg: 60.0,
                        brake: true,
                        main: false,
                    },
                    GearLeg {
                        id: "main_l".into(),
                        pos_m: [-0.62, -1.45, 2.05],
                        stroke_m: 0.3,
                        k_npm: 325_000.0,
                        c_comp_nspm: 30_000.0,
                        c_ext_nspm: 60_000.0,
                        steer_max_deg: 0.0,
                        brake: true,
                        main: true,
                    },
                ],
                contacts: vec![ContactPoint {
                    id: "tail".into(),
                    pos_m: [-6.0, 0.0, 0.35],
                }],
            },
            engines: vec![EngineInstall {
                engine,
                count: 1,
                position_m: [-5.2, 0.0, 0.0],
            }],
            aero: AeroLimits {
                tables: aero,
                max_mach: 2.0,
                mmo: 2.0,
                vmo_mps: 360.0,
                service_ceiling_m: 15500.0,
                n_max: 9.0,
                n_min: -3.0,
                n_max_by_config: NMaxByConfig {
                    gear: 4.0,
                    flaps: 4.0,
                },
                gear_limit_mps: 128.6,
                flap_limit_mps: 154.3,
                static_margin_mac: 0.06,
            },
            fcs: Fcs {
                sas_required: false,
                alpha_limit_deg: 26.0,
                n_cmd_max: 9.0,
                roll_rate_max_dps: 270.0,
                stab: act(-25.0, 25.0, 60.0, 0.05),
                aileron: act(-20.0, 20.0, 80.0, 0.05),
                rudder: act(-30.0, 30.0, 120.0, 0.05),
                lef: act(0.0, 25.0, 15.0, 0.1),
                tef: act(0.0, 30.0, 20.0, 0.1),
                airbrake: Airbrake {
                    rate_per_s: 1.0,
                    tau_s: 0.3,
                },
                gear: GearActuator { transit_s: 6.0 },
                gains,
            },
            envelope: vec![
                EnvelopePoint {
                    id: "vmax_sl".into(),
                    mass_kg: 9500.0,
                    alt_m: 0.0,
                    mach: None,
                    config: EnvelopeConfig::CleanAb,
                    metric: Metric::MaxMach,
                    value: 1.22,
                    tol_pct: 3.0,
                    requires: None,
                },
                EnvelopePoint {
                    id: "to_roll".into(),
                    mass_kg: 10500.0,
                    alt_m: 0.0,
                    mach: Some(0.0),
                    config: EnvelopeConfig::ToFlapsAb,
                    metric: Metric::RollM,
                    value: 195.0,
                    tol_pct: 5.0,
                    requires: Some(Requirement::Gear),
                },
            ],
            gun: None,
            rails: Vec::new(),
            ir: None,
            visual_l_eff_m: 0.0,
            damage: None,
        }
    }

    /// [`sample`] with the Kestrel's gun at handle `gun`, two rails holding the weapon at handle
    /// `srm`, an infrared signature, a visual size and five hit capsules.
    pub(crate) fn sample_armed(
        engine: ContentId,
        aero: ContentId,
        gains: ContentId,
        gun: ContentId,
        srm: ContentId,
    ) -> AircraftDef {
        let mut d = sample(engine, aero, gains);
        d.gun = Some(GunInstall {
            weapon: gun,
            rounds: 150,
            muzzle_m: [3.0, -0.6, 0.1],
            harmonisation_mrad: 0.0,
        });
        d.rails = vec![
            Rail {
                station: 1,
                pos_m: [-1.0, -4.95, 0.35],
                store: srm,
            },
            Rail {
                station: 9,
                pos_m: [-1.0, 4.95, 0.35],
                store: srm,
            },
        ];
        d.ir = Some(IrSignature {
            engine_idle: 0.1,
            engine_mil: 0.5,
            engine_ab: 1.0,
            aspect_side: 0.45,
            aspect_front: 0.15,
            skin_per_mach2: 0.05,
        });
        d.visual_l_eff_m = 7.0;
        let capsule = |id: &str, a: [f32; 3], b: [f32; 3], radius_m: f32| CapsuleDef {
            id: id.into(),
            a_m: a,
            b_m: b,
            radius_m,
        };
        d.damage = Some(DamageDef {
            hp: 100.0,
            joules_per_hp: 6370.0,
            capsules: vec![
                capsule("fuselage", [7.0, 0.0, 0.3], [-5.5, 0.0, 0.3], 1.1),
                capsule("wing_l", [-0.2, -0.8, 0.45], [-1.2, -4.9, 0.4], 1.3),
                capsule("wing_r", [-0.2, 0.8, 0.45], [-1.2, 4.9, 0.4], 1.3),
                capsule("tail", [-4.8, -2.4, 0.15], [-4.8, 2.4, 0.15], 0.8),
                capsule("fin", [-3.6, 0.0, -0.6], [-5.0, 0.0, -2.2], 0.7),
            ],
        });
        d
    }

    fn round_trip(def: &AircraftDef) -> Result<AircraftDef, CpackError> {
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        def.encode(&mut w)?;
        let rec = w.finish();
        let text = String::from_utf8(strings.bytes).unwrap();
        let mut r = Reader::new(&rec, &text);
        let back = AircraftDef::decode(&mut r)?;
        r.finish()?;
        Ok(back)
    }

    #[test]
    fn a_record_round_trips() {
        let def = sample(ContentId(0), ContentId(1), ContentId(2));
        assert_eq!(round_trip(&def).unwrap(), def);
        assert_eq!(def.engine_count(), 1);
    }

    #[test]
    fn an_armed_record_round_trips() {
        let def = sample_armed(
            ContentId(0),
            ContentId(1),
            ContentId(2),
            ContentId(3),
            ContentId(4),
        );
        assert_eq!(round_trip(&def).unwrap(), def);
    }

    #[test]
    fn too_many_rails_or_capsules_are_refused() {
        let armed = || {
            sample_armed(
                ContentId(0),
                ContentId(1),
                ContentId(2),
                ContentId(3),
                ContentId(4),
            )
        };
        let mut many = armed();
        many.rails = (0..=MAX_RAILS as u8)
            .map(|station| Rail {
                station,
                pos_m: [0.0; 3],
                store: ContentId(4),
            })
            .collect();
        assert!(matches!(round_trip(&many), Err(CpackError::Record(_))));
        let mut full = armed();
        full.rails.truncate(1);
        full.rails = (0..MAX_RAILS as u8)
            .map(|station| Rail {
                station,
                pos_m: [0.0; 3],
                store: ContentId(4),
            })
            .collect();
        assert_eq!(round_trip(&full).unwrap(), full);

        let mut capsules = armed();
        let first = capsules.damage.as_ref().unwrap().capsules[0].clone();
        capsules.damage.as_mut().unwrap().capsules = vec![first; MAX_CAPSULES + 1];
        assert!(matches!(round_trip(&capsules), Err(CpackError::Record(_))));
    }

    #[test]
    fn bad_codes_are_refused() {
        let def = sample(ContentId(0), ContentId(1), ContentId(2));
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        def.encode(&mut w).unwrap();
        let good = w.finish();
        let text = String::from_utf8(strings.bytes).unwrap();
        // Byte 8 is the category code (after the 8-byte name-key reference).
        for (at, value) in [(8, 99_u8), (9, 0)] {
            let mut bad = good.clone();
            bad[at] = value;
            let mut r = Reader::new(&bad, &text);
            assert!(AircraftDef::decode(&mut r).is_err(), "byte {at} = {value}");
        }
        // The last envelope point's requirement is the last field before padding.
        let mut cut = good;
        cut.truncate(cut.len() - 8);
        let mut r = Reader::new(&cut, &text);
        assert!(
            AircraftDef::decode(&mut r)
                .and_then(|_| r.finish())
                .is_err()
        );
    }
}
