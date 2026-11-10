//! The combat data of the player's aircraft type, built once from the registry when content loads
//! (`Services::combat`): its gun, its rails and the missile they carry, its infrared signature,
//! its size to the eye, its damage points and hit capsules, and the AI skills by rank. The bandit
//! flies the same type. Nothing here is state; the content hash covers it.

use content::aircraft::{MAX_CAPSULES, MAX_RAILS};
use content::{ContentId, Registry};
use flight::aircraft::AircraftModel;
use weapons::ballistics::GunBallistics;
use weapons::capsule::Capsule;
use weapons::ir::IrSignature;
use weapons::missile::MissileParams;
use weapons::sight::GunMount;

use ai_pilot::SkillParams;

use crate::math::DVec3;
use crate::time::SIM_DT;

/// The highest skill rank (Veteran).
pub const MAX_RANK: usize = crate::session::MAX_SKILL as usize;

/// What combat needs of the aircraft type and the skills.
#[derive(Clone, Debug)]
pub struct CombatModels {
    /// The gun's ballistics.
    pub gun: GunBallistics,
    /// Where it sits.
    pub mount: GunMount,
    /// Rounds loaded at a spawn.
    pub gun_rounds: u16,
    /// The missile the rails carry.
    pub missile: MissileParams,
    /// The rails' store positions, body axes, in station order (S1 first).
    pub rails: [DVec3; MAX_RAILS],
    /// How many rails carry it.
    pub rail_count: u8,
    /// The infrared signature.
    pub ir: IrSignature,
    /// The size to the eye, m.
    pub visual_l_eff_m: f64,
    /// Hit points when new.
    pub hp: f64,
    /// Energy per hit point, J.
    pub joules_per_hp: f64,
    /// The hit capsules, body axes from the reference point.
    pub capsules: [Capsule; MAX_CAPSULES],
    /// How many there are.
    pub capsule_count: usize,
    /// The skills by rank (index 0 is rank 1), with the load cap for this airframe.
    pub skills: [Option<SkillParams>; MAX_RANK],
}

impl CombatModels {
    /// The combat data of the aircraft behind `handle` (the player's type). `None` unless it has a
    /// gun, at least one rail with a missile, a signature and damage data.
    #[must_use]
    pub fn from_registry(reg: &Registry, handle: ContentId, model: &AircraftModel) -> Option<Self> {
        let ac = reg.aircraft(handle)?;
        let install = ac.gun.as_ref()?;
        let gun = GunBallistics::from_def(reg.weapon(install.weapon)?)?;
        let store = ac.rails.first()?.store;
        let missile = MissileParams::from_def(reg.weapon(store)?, SIM_DT)?;
        let mut rails_sorted: [(u8, DVec3); MAX_RAILS] = [(u8::MAX, DVec3::ZERO); MAX_RAILS];
        let mut rail_count = 0;
        for r in ac.rails.iter().filter(|r| r.store == store).take(MAX_RAILS) {
            rails_sorted[rail_count] = (r.station, vec3(r.pos_m));
            rail_count += 1;
        }
        rails_sorted[..rail_count].sort_by_key(|r| r.0);
        let mut rails = [DVec3::ZERO; MAX_RAILS];
        for (out, r) in rails.iter_mut().zip(&rails_sorted[..rail_count]) {
            *out = r.1;
        }
        let damage = ac.damage.as_ref()?;
        let mut capsules = [Capsule {
            a: DVec3::ZERO,
            b: DVec3::ZERO,
            radius: 0.0,
        }; MAX_CAPSULES];
        let capsule_count = damage.capsules.len().min(MAX_CAPSULES);
        for (out, d) in capsules.iter_mut().zip(&damage.capsules) {
            *out = Capsule::from_def(d);
        }
        let mut skills = [None; MAX_RANK];
        for (_, d) in reg.skills() {
            let rank = usize::from(d.rank);
            if (1..=MAX_RANK).contains(&rank) && skills[rank - 1].is_none() {
                skills[rank - 1] = Some(SkillParams::from_def(d, model.limits.n_max));
            }
        }
        Some(Self {
            mount: GunMount {
                muzzle_m: vec3(install.muzzle_m),
                harmonisation_rad: f64::from(install.harmonisation_mrad) * 1e-3, // PHYS: mrad
            },
            gun_rounds: install.rounds,
            gun,
            missile,
            rails,
            rail_count: rail_count as u8,
            ir: IrSignature::from_def(ac.ir.as_ref()?),
            visual_l_eff_m: f64::from(ac.visual_l_eff_m),
            hp: f64::from(damage.hp),
            joules_per_hp: f64::from(damage.joules_per_hp),
            capsules,
            capsule_count,
            skills,
        })
    }

    /// The skill of a rank (1 to 3), if the pack has one.
    #[must_use]
    pub fn skill(&self, rank: u8) -> Option<SkillParams> {
        let i = usize::from(rank).checked_sub(1)?;
        self.skills.get(i).copied().flatten()
    }

    /// The hit capsules in use.
    #[must_use]
    pub fn capsules(&self) -> &[Capsule] {
        &self.capsules[..self.capsule_count]
    }
}

fn vec3(v: [f32; 3]) -> DVec3 {
    DVec3::new(f64::from(v[0]), f64::from(v[1]), f64::from(v[2]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::pack;

    #[test]
    fn the_core_pack_gives_the_kestrels_combat_data() {
        let reg = Registry::from_cpack(&pack(8400)).expect("the pack loads");
        let h = reg.aircraft_handles().next().expect("an aircraft");
        let model = AircraftModel::from_registry(&reg, h, SIM_DT).expect("a model");
        let m = CombatModels::from_registry(&reg, h, &model).expect("combat data");
        assert_eq!((m.gun_rounds, m.rail_count, m.capsule_count), (150, 2, 5));
        assert!((m.gun.rate_rps - 1_700.0 / 60.0).abs() < 1e-9);
        // S1 on the left wingtip, S9 on the right: the first launch is from the left.
        assert!(
            m.rails[0].y < 0.0 && m.rails[1].y > 0.0,
            "{:?}",
            &m.rails[..2]
        );
        assert_eq!((m.hp, m.joules_per_hp), (100.0, 6_370.0));
        assert_eq!(m.missile.mass_kg, 85.0);
        for rank in 1..=3 {
            let s = m.skill(rank).expect("a skill");
            assert_eq!(s.rank, rank);
            assert!((s.n_cap - s.g_frac * model.limits.n_max).abs() < 1e-12);
        }
        assert!(m.skill(0).is_none() && m.skill(4).is_none());
        assert!(m.visual_l_eff_m > 6.0);
    }
}
