//! Named values stored as one-byte codes. Authoring data uses the names; packs store the codes.

macro_rules! coded_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident = $code:literal, $text:literal;)+ }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum $name {
            $($(#[$vmeta])* $variant = $code,)+
        }

        impl $name {
            /// Every value, in code order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The code stored in the pack.
            #[must_use]
            pub const fn code(self) -> u8 {
                self as u8
            }

            /// The value for a stored code.
            #[must_use]
            pub const fn from_code(code: u8) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// The name used in authoring data.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)+
                }
            }

            /// The value for an authoring-data name.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

coded_enum! {
    /// Aircraft role (GDD 13.4).
    Category {
        /// Light fighter.
        LightFighter = 1, "light_fighter";
        /// Multirole fighter.
        MultiroleFighter = 2, "multirole_fighter";
        /// Heavy fighter.
        HeavyFighter = 3, "heavy_fighter";
        /// Interceptor.
        Interceptor = 4, "interceptor";
        /// Strike fighter.
        Strike = 5, "strike";
        /// Close air support.
        Cas = 6, "cas";
        /// Bomber.
        Bomber = 7, "bomber";
        /// Electronic-warfare aircraft.
        ElectronicWarfare = 8, "electronic_warfare";
        /// Airborne early warning.
        Awacs = 9, "awacs";
        /// Carrier-based aircraft.
        Carrier = 10, "carrier";
        /// Stealth aircraft.
        Stealth = 11, "stealth";
        /// Experimental aircraft.
        Experimental = 12, "experimental";
        /// Tanker.
        Tanker = 13, "tanker";
        /// Transport.
        Transport = 14, "transport";
        /// Unmanned aircraft.
        Uav = 15, "uav";
    }
}

coded_enum! {
    /// Technology generation (GDD 13.6).
    Era {
        /// Generation 1.
        G1 = 1, "G1";
        /// Generation 2.
        G2 = 2, "G2";
        /// Generation 3.
        G3 = 3, "G3";
        /// Generation 4.
        G4 = 4, "G4";
        /// Generation 4.5.
        G4_5 = 5, "G4.5";
        /// Generation 5.
        G5 = 6, "G5";
    }
}

coded_enum! {
    /// Engine type (GDD A.3).
    EngineKind {
        /// Turbofan.
        Turbofan = 1, "turbofan";
        /// Turbojet.
        Turbojet = 2, "turbojet";
        /// Turboprop.
        Turboprop = 3, "turboprop";
    }
}

coded_enum! {
    /// Aircraft configuration of a performance-envelope point (GDD C.9, ADR-31).
    EnvelopeConfig {
        /// Clean, idle power.
        Clean = 1, "clean";
        /// Clean, military (dry) power.
        CleanMil = 2, "clean_mil";
        /// Clean, full afterburner.
        CleanAb = 3, "clean_ab";
        /// Reference loadout (clean aerodynamics at the point's mass), full afterburner.
        RefAb = 4, "ref_ab";
        /// Take-off flaps, full afterburner.
        ToFlapsAb = 5, "to_flaps_ab";
        /// Landing configuration with brakes.
        LdgBrakes = 6, "ldg_brakes";
    }
}

coded_enum! {
    /// What a performance-envelope point measures (GDD A.2, 11.19).
    Metric {
        /// Maximum level Mach number.
        MaxMach = 1, "max_mach";
        /// 1 g stall speed, m/s.
        StallMps = 2, "stall_mps";
        /// Maximum 1 g specific excess power, m/s (ADR-31).
        RocMps = 3, "roc_mps";
        /// Sustained load factor.
        NSust = 4, "n_sust";
        /// Sustained turn rate, deg/s.
        TurnDps = 5, "turn_dps";
        /// Fuel flow, kg/s.
        FfKgps = 6, "ff_kgps";
        /// Ground roll, m.
        RollM = 7, "roll_m";
    }
}

coded_enum! {
    /// A capability a performance point needs that the model may not have yet (ADR-31).
    Requirement {
        /// The landing-gear model (v0.1b).
        Gear = 1, "gear";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_codes_round_trip() {
        fn check<T: Copy + PartialEq + core::fmt::Debug>(
            all: &[T],
            code: fn(T) -> u8,
            from_code: fn(u8) -> Option<T>,
            name: fn(T) -> &'static str,
            from_name: fn(&str) -> Option<T>,
        ) {
            for &v in all {
                assert_eq!(from_code(code(v)), Some(v));
                assert_eq!(from_name(name(v)), Some(v));
            }
            assert_eq!(from_code(0), None);
            assert_eq!(from_name("nope"), None);
        }
        check(
            Category::ALL,
            Category::code,
            Category::from_code,
            Category::name,
            Category::from_name,
        );
        check(
            Era::ALL,
            Era::code,
            Era::from_code,
            Era::name,
            Era::from_name,
        );
        check(
            EngineKind::ALL,
            EngineKind::code,
            EngineKind::from_code,
            EngineKind::name,
            EngineKind::from_name,
        );
        check(
            EnvelopeConfig::ALL,
            EnvelopeConfig::code,
            EnvelopeConfig::from_code,
            EnvelopeConfig::name,
            EnvelopeConfig::from_name,
        );
        check(
            Metric::ALL,
            Metric::code,
            Metric::from_code,
            Metric::name,
            Metric::from_name,
        );
        check(
            Requirement::ALL,
            Requirement::code,
            Requirement::from_code,
            Requirement::name,
            Requirement::from_name,
        );
        assert_eq!(Era::G4_5.name(), "G4.5");
    }
}
