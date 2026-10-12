//! The aircraft record: authoring-side meaning in [`AircraftDef`], wire format in [`field`].

use crate::cpack::CpackError;

/// Bytes in one encoded aircraft record.
pub const RECORD_BYTES: usize = 144;

/// Byte offsets inside an encoded aircraft record (little-endian). Floats are `f32`; the baker
/// refuses authored values that `f32` cannot hold exactly, so nothing is silently rounded.
pub mod field {
    /// `u32`: offset of the locale key in the string table.
    pub const NAME_KEY_OFF: usize = 0;
    /// `u32`: length of the locale key in bytes.
    pub const NAME_KEY_LEN: usize = 4;
    /// `u8`: [`super::Category`] code.
    pub const CATEGORY: usize = 8;
    /// `u8`: [`super::Era`] code.
    pub const ERA: usize = 9;
    /// `u8`: crew count.
    pub const CREW: usize = 10;
    /// `u8`: engine count.
    pub const ENGINE_COUNT: usize = 11;
    /// `u8`: hardpoint count.
    pub const HARDPOINTS: usize = 12;
    /// `u8`: 1 if the airframe needs its stability augmentation to be flyable, else 0.
    pub const SAS_REQUIRED: usize = 13;
    /// `f32`: empty mass, kg.
    pub const EMPTY_KG: usize = 16;
    /// `f32`: internal fuel, kg.
    pub const INTERNAL_FUEL_KG: usize = 20;
    /// `f32`: maximum take-off mass, kg.
    pub const MAX_TAKEOFF_KG: usize = 24;
    /// `f32`: wing reference area, m².
    pub const WING_AREA_M2: usize = 28;
    /// `f32`: wing span, m.
    pub const SPAN_M: usize = 32;
    /// `f32`: mean aerodynamic chord, m.
    pub const MAC_M: usize = 36;
    /// `f32`: roll inertia, kg·m².
    pub const INERTIA_XX: usize = 40;
    /// `f32`: pitch inertia, kg·m².
    pub const INERTIA_YY: usize = 44;
    /// `f32`: yaw inertia, kg·m².
    pub const INERTIA_ZZ: usize = 48;
    /// `f32`: product of inertia XZ, kg·m².
    pub const INERTIA_XZ: usize = 52;
    /// `f32`: structural positive load limit, g.
    pub const N_MAX: usize = 56;
    /// `f32`: structural negative load limit, g (negative).
    pub const N_MIN: usize = 60;
    /// `f32`: maximum operating Mach number.
    pub const MMO: usize = 64;
    /// `f32`: maximum operating speed, m/s.
    pub const VMO_MS: usize = 68;
    /// `f32`: service ceiling, m.
    pub const SERVICE_CEILING_M: usize = 72;
    /// `f32`: sea-level static thrust at military power, N (all engines).
    pub const THRUST_MIL_N: usize = 76;
    /// `f32`: sea-level static thrust with afterburner, N (all engines).
    pub const THRUST_AB_N: usize = 80;
    /// `f32`: zero-lift drag coefficient.
    pub const CD0: usize = 84;
    /// `f32`: induced-drag factor K.
    pub const INDUCED_K: usize = 88;
    /// `f32`: lift-curve slope, per radian.
    pub const CL_ALPHA_PER_RAD: usize = 92;
    /// `f32`: clean maximum lift coefficient at low Mach.
    pub const CL_MAX: usize = 96;
    /// `f32`: static margin as a fraction of MAC.
    pub const STATIC_MARGIN_MAC: usize = 100;
    /// First of the reserved bytes, which must be zero in version 1.
    pub const RESERVED: usize = 104;
}

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
    /// Engine count.
    pub engine_count: u8,
    /// Hardpoint count.
    pub hardpoints: u8,
    /// True if the airframe needs stability augmentation to be flyable.
    pub sas_required: bool,
    /// Empty mass, kg.
    pub empty_kg: f32,
    /// Internal fuel, kg.
    pub internal_fuel_kg: f32,
    /// Maximum take-off mass, kg.
    pub max_takeoff_kg: f32,
    /// Wing reference area, m².
    pub wing_area_m2: f32,
    /// Wing span, m.
    pub span_m: f32,
    /// Mean aerodynamic chord, m.
    pub mac_m: f32,
    /// Roll inertia, kg·m².
    pub inertia_xx_kgm2: f32,
    /// Pitch inertia, kg·m².
    pub inertia_yy_kgm2: f32,
    /// Yaw inertia, kg·m².
    pub inertia_zz_kgm2: f32,
    /// Product of inertia XZ, kg·m².
    pub inertia_xz_kgm2: f32,
    /// Structural positive load limit, g.
    pub n_max: f32,
    /// Structural negative load limit, g (negative).
    pub n_min: f32,
    /// Maximum operating Mach number.
    pub mmo: f32,
    /// Maximum operating speed, m/s.
    pub vmo_ms: f32,
    /// Service ceiling, m.
    pub service_ceiling_m: f32,
    /// Sea-level static military thrust, N (all engines).
    pub thrust_mil_n: f32,
    /// Sea-level static afterburner thrust, N (all engines).
    pub thrust_ab_n: f32,
    /// Zero-lift drag coefficient.
    pub cd0: f32,
    /// Induced-drag factor K.
    pub induced_k: f32,
    /// Lift-curve slope, per radian.
    pub cl_alpha_per_rad: f32,
    /// Clean maximum lift coefficient at low Mach.
    pub cl_max: f32,
    /// Static margin as a fraction of MAC.
    pub static_margin_mac: f32,
}

impl AircraftDef {
    /// Writes the record. `name_key` is the already-interned `(offset, length)` of
    /// [`Self::name_key`] in the string table.
    pub(crate) fn encode(&self, name_key: (u32, u32), out: &mut [u8; RECORD_BYTES]) {
        use field as f;
        *out = [0; RECORD_BYTES];
        put_u32(out, f::NAME_KEY_OFF, name_key.0);
        put_u32(out, f::NAME_KEY_LEN, name_key.1);
        out[f::CATEGORY] = self.category.code();
        out[f::ERA] = self.era.code();
        out[f::CREW] = self.crew;
        out[f::ENGINE_COUNT] = self.engine_count;
        out[f::HARDPOINTS] = self.hardpoints;
        out[f::SAS_REQUIRED] = u8::from(self.sas_required);
        for (at, v) in self.floats() {
            put_f32(out, at, v);
        }
    }

    /// Reads a record. `strings` is the whole string section, already known to be UTF-8.
    pub(crate) fn decode(rec: &[u8], strings: &str) -> Result<Self, CpackError> {
        use field as f;
        let rec: &[u8; RECORD_BYTES] = rec
            .try_into()
            .map_err(|_| CpackError::Layout("aircraft record has the wrong size"))?;
        if rec[f::RESERVED..].iter().any(|&b| b != 0) || rec[14] != 0 || rec[15] != 0 {
            return Err(CpackError::Record("reserved bytes are not zero"));
        }
        let off = get_u32(rec, f::NAME_KEY_OFF) as usize;
        let len = get_u32(rec, f::NAME_KEY_LEN) as usize;
        let name_key = off
            .checked_add(len)
            .and_then(|end| strings.get(off..end))
            .ok_or(CpackError::Record("name key is outside the string table"))?
            .to_owned();
        let category = Category::from_code(rec[f::CATEGORY])
            .ok_or(CpackError::Record("unknown aircraft category"))?;
        let era = Era::from_code(rec[f::ERA]).ok_or(CpackError::Record("unknown era"))?;
        let sas_required = match rec[f::SAS_REQUIRED] {
            0 => false,
            1 => true,
            _ => return Err(CpackError::Record("sas_required is not 0 or 1")),
        };
        let float = |at| get_f32(rec, at);
        let def = Self {
            name_key,
            category,
            era,
            crew: rec[f::CREW],
            engine_count: rec[f::ENGINE_COUNT],
            hardpoints: rec[f::HARDPOINTS],
            sas_required,
            empty_kg: float(f::EMPTY_KG),
            internal_fuel_kg: float(f::INTERNAL_FUEL_KG),
            max_takeoff_kg: float(f::MAX_TAKEOFF_KG),
            wing_area_m2: float(f::WING_AREA_M2),
            span_m: float(f::SPAN_M),
            mac_m: float(f::MAC_M),
            inertia_xx_kgm2: float(f::INERTIA_XX),
            inertia_yy_kgm2: float(f::INERTIA_YY),
            inertia_zz_kgm2: float(f::INERTIA_ZZ),
            inertia_xz_kgm2: float(f::INERTIA_XZ),
            n_max: float(f::N_MAX),
            n_min: float(f::N_MIN),
            mmo: float(f::MMO),
            vmo_ms: float(f::VMO_MS),
            service_ceiling_m: float(f::SERVICE_CEILING_M),
            thrust_mil_n: float(f::THRUST_MIL_N),
            thrust_ab_n: float(f::THRUST_AB_N),
            cd0: float(f::CD0),
            induced_k: float(f::INDUCED_K),
            cl_alpha_per_rad: float(f::CL_ALPHA_PER_RAD),
            cl_max: float(f::CL_MAX),
            static_margin_mac: float(f::STATIC_MARGIN_MAC),
        };
        if def.floats().iter().any(|&(_, v)| !v.is_finite()) {
            return Err(CpackError::Record("a number is not finite"));
        }
        Ok(def)
    }

    /// Every `f32` field with its record offset.
    fn floats(&self) -> [(usize, f32); 22] {
        use field as f;
        [
            (f::EMPTY_KG, self.empty_kg),
            (f::INTERNAL_FUEL_KG, self.internal_fuel_kg),
            (f::MAX_TAKEOFF_KG, self.max_takeoff_kg),
            (f::WING_AREA_M2, self.wing_area_m2),
            (f::SPAN_M, self.span_m),
            (f::MAC_M, self.mac_m),
            (f::INERTIA_XX, self.inertia_xx_kgm2),
            (f::INERTIA_YY, self.inertia_yy_kgm2),
            (f::INERTIA_ZZ, self.inertia_zz_kgm2),
            (f::INERTIA_XZ, self.inertia_xz_kgm2),
            (f::N_MAX, self.n_max),
            (f::N_MIN, self.n_min),
            (f::MMO, self.mmo),
            (f::VMO_MS, self.vmo_ms),
            (f::SERVICE_CEILING_M, self.service_ceiling_m),
            (f::THRUST_MIL_N, self.thrust_mil_n),
            (f::THRUST_AB_N, self.thrust_ab_n),
            (f::CD0, self.cd0),
            (f::INDUCED_K, self.induced_k),
            (f::CL_ALPHA_PER_RAD, self.cl_alpha_per_rad),
            (f::CL_MAX, self.cl_max),
            (f::STATIC_MARGIN_MAC, self.static_margin_mac),
        ]
    }
}

fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_f32(b: &mut [u8], at: usize, v: f32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn get_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn get_f32(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A valid definition with the Kestrel's headline numbers.
    pub(crate) fn sample() -> AircraftDef {
        AircraftDef {
            name_key: "ac.kestrel.name".into(),
            category: Category::LightFighter,
            era: Era::G4,
            crew: 1,
            engine_count: 1,
            hardpoints: 9,
            sas_required: false,
            empty_kg: 8400.0,
            internal_fuel_kg: 3100.0,
            max_takeoff_kg: 14500.0,
            wing_area_m2: 28.0,
            span_m: 9.8,
            mac_m: 3.2,
            inertia_xx_kgm2: 13000.0,
            inertia_yy_kgm2: 76000.0,
            inertia_zz_kgm2: 86000.0,
            inertia_xz_kgm2: 1300.0,
            n_max: 9.0,
            n_min: -3.0,
            mmo: 2.0,
            vmo_ms: 360.0,
            service_ceiling_m: 15500.0,
            thrust_mil_n: 79000.0,
            thrust_ab_n: 128_000.0,
            cd0: 0.0235,
            induced_k: 0.135,
            cl_alpha_per_rad: 3.6,
            cl_max: 1.6,
            static_margin_mac: 0.06,
        }
    }

    #[test]
    fn names_and_codes_round_trip() {
        for &c in Category::ALL {
            assert_eq!(Category::from_code(c.code()), Some(c));
            assert_eq!(Category::from_name(c.name()), Some(c));
        }
        for &e in Era::ALL {
            assert_eq!(Era::from_code(e.code()), Some(e));
            assert_eq!(Era::from_name(e.name()), Some(e));
        }
        assert_eq!(Category::from_code(0), None);
        assert_eq!(Era::from_name("G9"), None);
        assert_eq!(Era::G4_5.name(), "G4.5");
    }

    #[test]
    fn the_float_table_covers_the_whole_float_area_once() {
        let mut seen = [false; RECORD_BYTES];
        for (at, _) in sample().floats() {
            for b in &mut seen[at..at + 4] {
                assert!(!*b, "offset {at} overlaps another field");
                *b = true;
            }
        }
        assert!(seen[field::EMPTY_KG..field::RESERVED].iter().all(|&b| b));
    }

    #[test]
    fn a_record_round_trips() {
        let def = sample();
        let strings = "xxx".to_owned() + &def.name_key;
        let mut rec = [0_u8; RECORD_BYTES];
        def.encode((3, def.name_key.len() as u32), &mut rec);
        assert_eq!(AircraftDef::decode(&rec, &strings).unwrap(), def);
    }

    #[test]
    fn a_record_with_bad_content_is_refused() {
        let def = sample();
        let strings = def.name_key.clone();
        let mut good = [0_u8; RECORD_BYTES];
        def.encode((0, def.name_key.len() as u32), &mut good);
        let mut cases: Vec<(&str, [u8; RECORD_BYTES])> = Vec::new();
        for (what, at, value) in [
            ("reserved tail", RECORD_BYTES - 1, 1_u8),
            ("reserved pad", 14, 1),
            ("category", field::CATEGORY, 99),
            ("era", field::ERA, 0),
            ("sas flag", field::SAS_REQUIRED, 2),
        ] {
            let mut r = good;
            r[at] = value;
            cases.push((what, r));
        }
        let mut r = good;
        r[field::MMO..field::MMO + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        cases.push(("NaN", r));
        let mut r = good;
        put_u32(&mut r, field::NAME_KEY_LEN, 1000);
        cases.push(("name key range", r));
        for (what, rec) in cases {
            assert!(
                AircraftDef::decode(&rec, &strings).is_err(),
                "{what} should be refused"
            );
        }
        assert!(AircraftDef::decode(&good[..100], &strings).is_err());
    }
}
