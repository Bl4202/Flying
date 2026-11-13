//! The battlefield size presets (GDD 0.6.4) and the airport corridor (GDD 8.6).

/// A size class (GDD 0.6.4): the edge of the square playable area.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SizeClass {
    /// 50 km.
    Small = 0,
    /// 120 km.
    Medium = 1,
    /// 250 km.
    Large = 2,
    /// 500 km.
    Huge = 3,
    /// 1 000 km.
    Extreme = 4,
}

/// Every class, smallest first.
pub const ALL: [SizeClass; 5] = [
    SizeClass::Small,
    SizeClass::Medium,
    SizeClass::Large,
    SizeClass::Huge,
    SizeClass::Extreme,
];

/// The corridor's least half-width, m. // DATA: GDD 8.6
const CORRIDOR_MIN_RADIUS_M: f64 = 30_000.0;
/// The corridor's half-width as a share of its length. // DATA: GDD 8.6
const CORRIDOR_RADIUS_SHARE: f64 = 0.30;

impl SizeClass {
    /// The edge of the class's square, m (GDD 0.6.4).
    #[must_use]
    pub const fn edge_m(self) -> f64 {
        match self {
            Self::Small => 50_000.0,
            Self::Medium => 120_000.0,
            Self::Large => 250_000.0,
            Self::Huge => 500_000.0,
            Self::Extreme => 1_000_000.0,
        }
    }

    /// The class's area, m².
    #[must_use]
    pub const fn area_m2(self) -> f64 {
        self.edge_m() * self.edge_m()
    }

    /// The smallest class whose area is at least `area_m2`; `None` above Extreme (GDD 8.6).
    #[must_use]
    pub fn for_area(area_m2: f64) -> Option<Self> {
        ALL.into_iter().find(|c| c.area_m2() >= area_m2)
    }

    /// The class's name in data (`region/1`, the UI).
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Huge => "huge",
            Self::Extreme => "extreme",
        }
    }
}

/// An airport-to-airport corridor (GDD 8.6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corridor {
    /// Half-width `r = max(30 km, 0.30 L)`, m.
    pub radius_m: f64,
    /// `2 r L + π r²`, m².
    pub area_m2: f64,
    /// The smallest class holding it; `None` above Extreme.
    pub class: Option<SizeClass>,
}

/// The corridor for a geodesic length `L` between two airports (GDD 8.6).
#[must_use]
pub fn corridor(length_m: f64) -> Corridor {
    let r = CORRIDOR_MIN_RADIUS_M.max(CORRIDOR_RADIUS_SHARE * length_m);
    let area = 2.0 * r * length_m + core::f64::consts::PI * r * r;
    Corridor {
        radius_m: r,
        area_m2: area,
        class: SizeClass::for_area(area),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bf3_the_gdd_worked_example() {
        let c = corridor(300_000.0);
        assert_eq!(c.radius_m, 90_000.0);
        assert!(
            (c.area_m2 / 1e6 - 79_446.9).abs() < 0.1,
            "{}",
            c.area_m2 / 1e6
        );
        assert_eq!(c.class, Some(SizeClass::Huge));
        assert_eq!(corridor(10_000.0).radius_m, 30_000.0);
    }

    #[test]
    fn bf7_the_presets_and_the_smallest_class_rule() {
        let km2 = [2_500.0, 14_400.0, 62_500.0, 250_000.0, 1_000_000.0];
        let edges_km = [50.0, 120.0, 250.0, 500.0, 1_000.0];
        for (i, c) in ALL.into_iter().enumerate() {
            assert_eq!(c.edge_m(), edges_km[i] * 1e3);
            assert_eq!(c.area_m2(), km2[i] * 1e6);
            assert_eq!(
                SizeClass::for_area(km2[i] * 1e6),
                Some(c),
                "{km2:?}[{i}] is the class itself"
            );
            let above = SizeClass::for_area(km2[i] * 1e6 + 1.0);
            assert_eq!(above, ALL.get(i + 1).copied(), "1 m² above {} km²", km2[i]);
        }
        assert_eq!(SizeClass::for_area(1.0), Some(SizeClass::Small));
        assert_eq!(SizeClass::Huge.name(), "huge");
    }
}
