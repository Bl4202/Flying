//! Reading a `.cpack` back into authoring form, for diffing and for the round-trip test.

use serde_json::{Value, json};

use crate::bake::hex;
use crate::model::{AeroFile, AircraftFile, EngineFile, FcsGainsFile};
use content::{CpackError, Entry, Registry};

/// What a pack contains, in authoring form.
#[derive(Debug, PartialEq)]
pub struct Exported {
    /// `sha256:<hex>` of the pack.
    pub content_hash: String,
    /// The aircraft, in handle (sorted id) order.
    pub aircraft: Vec<AircraftFile>,
    /// The engines, in handle order.
    pub engines: Vec<EngineFile>,
    /// The aero tables, in handle order.
    pub aero: Vec<AeroFile>,
    /// The control-law gains, in handle order.
    pub fcs: Vec<FcsGainsFile>,
}

/// Decodes a pack.
///
/// # Errors
/// Whatever [`Registry::from_cpack`] refuses.
pub fn export(bytes: &[u8]) -> Result<Exported, CpackError> {
    let reg = Registry::from_cpack(bytes)?;
    let name_of = |h| reg.id(h).unwrap_or_default().to_owned();
    let mut out = Exported {
        content_hash: format!("sha256:{}", hex(reg.content_hash())),
        aircraft: Vec::new(),
        engines: Vec::new(),
        aero: Vec::new(),
        fcs: Vec::new(),
    };
    for (_, id, entry) in reg.iter() {
        match entry {
            Entry::Aircraft(d) => out.aircraft.push(AircraftFile::from_def(id, d, &name_of)),
            Entry::Engine(d) => out.engines.push(EngineFile::from_def(id, d)),
            Entry::Aero(d) => out.aero.push(AeroFile::from_def(id, d)),
            Entry::Fcs(d) => out.fcs.push(FcsGainsFile::from_def(id, d)),
        }
    }
    Ok(out)
}

impl Exported {
    /// The document `content-bake export` prints.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": "cpack-export/3",
            "content_hash": self.content_hash,
            "aircraft": self.aircraft,
            "engines": self.engines,
            "aero": self.aero,
            "fcs": self.fcs,
        })
    }
}
