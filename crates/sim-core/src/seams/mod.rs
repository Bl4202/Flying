//! Extension seams (GDD 61.5): every capability the first build cannot afford ships behind a
//! trait with a simple, real implementation and a contract function that any replacement must
//! pass, so upgrades are drop-in (`AC-61.5`, `AC-66.2`).
//!
//! | Seam | Trait | First implementation | Status | Replaced in |
//! |---|---|---|---|---|
//! | Geo | [`geo::GeoService`] | `ProceduralGeo`, `FlatGeo` | wired | M2 (streamed terrain) |
//! | Weather | [`weather::WeatherProvider`] | `StillAir`, `UniformWind` | wired | M4 |
//! | Line of sight | [`los::LosService`] | `OpenSky`, `TerrainLos` | in `Services` | M4 |
//! | Strategic | [`strategic::StrategicContext`] | `NoStrategicContext` | in `Services` | after 1.0 |
//! | Replay | [`replay::ReplaySink`] | `NullReplaySink`, `CountingReplaySink` | wired | M2 |
//! | Aero | [`aero::AeroModel`] | `NoAero`, `LinearAero` | provisional | M1 |
//! | Damage | [`damage::DamageModel`] | `AggregateHealth` | provisional | M4 |
//! | Radar | [`radar::RadarModel`] | `NoRadar`, `RangeRcsRadar` | provisional | M4 |
//! | Missile | [`missile::MissileModel`] | `ProportionalNav` | provisional | M3 |
//! | Pilot | [`pilot::PilotBrain`] | `HoldCourse` | provisional | M3 |
//! | Mission | [`mission::MissionProvider`] | `ScriptedMission` | provisional | M2 |
//! | Persistence | [`persistence::PersistenceSink`] | `DiscardingSink`, `InMemorySink` | provisional | M2 |
//! | Surface | [`surface::SurfaceSim`] | `StaticShips` | provisional | M3 |
//!
//! "Provisional" means the trait exists so later work has a place to land, but nothing in the
//! tick calls it yet and its signature may change when its first real user arrives. Three more
//! seams live outside this crate: `Transport` (`crates/protocol`), `Renderer` (`packages/render`)
//! and `AudioBackend` (`packages/audio`).

pub mod aero;
pub mod damage;
pub mod geo;
pub mod los;
pub mod missile;
pub mod mission;
pub mod persistence;
pub mod pilot;
pub mod radar;
pub mod replay;
pub mod strategic;
pub mod surface;
pub mod weather;
