//! Reading a `.cpack` back into authoring form, for diffing and for the round-trip test.

use serde_json::{Value, json};

use crate::bake::hex;
use crate::model::AircraftFile;
use content::{CpackError, Registry};

/// What a pack contains, in authoring form.
#[derive(Debug, PartialEq)]
pub struct Exported {
    /// `sha256:<hex>` of the pack.
    pub content_hash: String,
    /// The aircraft, in handle (sorted id) order.
    pub aircraft: Vec<AircraftFile>,
}

/// Decodes a pack.
///
/// # Errors
/// Whatever [`Registry::from_cpack`] refuses.
pub fn export(bytes: &[u8]) -> Result<Exported, CpackError> {
    let reg = Registry::from_cpack(bytes)?;
    Ok(Exported {
        content_hash: format!("sha256:{}", hex(reg.content_hash())),
        aircraft: reg
            .iter()
            .map(|(_, id, def)| AircraftFile::from_def(id, def))
            .collect(),
    })
}

impl Exported {
    /// The document `content-bake export` prints.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "schema": "cpack-export/1",
            "content_hash": self.content_hash,
            "aircraft": self.aircraft,
        })
    }
}
