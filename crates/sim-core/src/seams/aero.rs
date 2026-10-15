//! `AeroModel`: forces and moments on an airframe (GDD 11.6, 61.5). The seam lives in the flight
//! model (ADR-29) and is re-exported here so it sits in the seam table with the others. Its real
//! implementation is the table model, `TableAero`, which the aircraft tick calls directly (no
//! `dyn` on the hot path, GDD 61.4.2); `NoAero` and `LinearAero` are contract-passing stand-ins.

pub use flight::aero::{AeroInput, AeroModel, AeroOutput, LinearAero, NoAero, TableAero, contract};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stand_ins_pass_the_contract() {
        contract(&NoAero);
        contract(&LinearAero {
            wing_area_m2: 28.0,
            cl_alpha: 3.6,
            cl_max: 1.6,
            cd0: 0.0235,
            k: 0.135,
        });
    }
}
