//! Extension seams (GDD 61.5): every capability the first build cannot afford ships behind a
//! trait with a simple, real implementation and a contract function that any replacement must
//! pass, so upgrades are drop-in (`AC-61.5`, `AC-66.2`).

pub mod geo;
pub mod weather;
