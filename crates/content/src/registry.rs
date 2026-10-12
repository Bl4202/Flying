//! The registry: baked content behind `u32` handles.

use crate::aircraft::AircraftDef;
use crate::cpack::{self, CpackError};

/// Handle of a baked content entry: its index in the pack's table of contents, which is sorted
/// by id. Handles are stable for a given pack and meaningless across packs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContentId(pub u32);

/// Loaded content: immutable, shared by reference, looked up by handle (`O(1)`) or id (`O(log n)`).
#[derive(Clone, Debug)]
pub struct Registry {
    ids: Vec<String>,
    aircraft: Vec<AircraftDef>,
    content_hash: [u8; 32],
}

impl Default for Registry {
    fn default() -> Self {
        Self::empty()
    }
}

impl Registry {
    /// A registry with nothing in it. Its content hash is all zeros.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            ids: Vec::new(),
            aircraft: Vec::new(),
            content_hash: [0; 32],
        }
    }

    /// Reads a `.cpack`, checking its checksum and its structure.
    ///
    /// # Errors
    /// Any [`CpackError`] other than the writer-only ones.
    pub fn from_cpack(bytes: &[u8]) -> Result<Self, CpackError> {
        let p = cpack::parse(bytes)?;
        Ok(Self {
            ids: p.ids,
            aircraft: p.aircraft,
            content_hash: p.content_hash,
        })
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True if there are no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// The aircraft behind a handle.
    #[must_use]
    pub fn aircraft(&self, handle: ContentId) -> Option<&AircraftDef> {
        self.aircraft.get(handle.0 as usize)
    }

    /// The id behind a handle.
    #[must_use]
    pub fn id(&self, handle: ContentId) -> Option<&str> {
        self.ids.get(handle.0 as usize).map(String::as_str)
    }

    /// The handle of an id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<ContentId> {
        let i = self.ids.binary_search_by(|p| p.as_str().cmp(id)).ok()?;
        Some(ContentId(u32::try_from(i).ok()?))
    }

    /// Entries in handle (sorted id) order.
    pub fn iter(&self) -> impl Iterator<Item = (ContentId, &str, &AircraftDef)> {
        self.ids
            .iter()
            .zip(&self.aircraft)
            .enumerate()
            .map(|(i, (id, def))| (ContentId(i as u32), id.as_str(), def))
    }

    /// The pack's identity: SHA-256 of everything after its header. All zeros when empty.
    #[must_use]
    pub fn content_hash(&self) -> &[u8; 32] {
        &self.content_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpack::tests::pack;

    #[test]
    fn handles_are_indices_in_sorted_id_order() {
        let r = Registry::from_cpack(&pack()).unwrap();
        assert_eq!(r.len(), 3);
        for (i, id) in ["core:ac/alpha", "core:ac/mike", "core:ac/zulu"]
            .iter()
            .enumerate()
        {
            let h = r.find(id).unwrap();
            assert_eq!(h, ContentId(i as u32));
            assert_eq!(r.id(h), Some(*id));
            assert!(r.aircraft(h).is_some());
        }
        let ids: Vec<_> = r.iter().map(|(_, id, _)| id).collect();
        assert_eq!(ids, ["core:ac/alpha", "core:ac/mike", "core:ac/zulu"]);
    }

    #[test]
    fn unknown_ids_and_handles_are_none() {
        let r = Registry::from_cpack(&pack()).unwrap();
        assert_eq!(r.find("core:ac/nope"), None);
        assert_eq!(r.find(""), None);
        assert!(r.aircraft(ContentId(3)).is_none());
        assert!(r.id(ContentId(u32::MAX)).is_none());
    }

    #[test]
    fn the_empty_registry_has_a_zero_hash() {
        let r = Registry::empty();
        assert!(r.is_empty());
        assert_eq!(r.content_hash(), &[0; 32]);
        assert_eq!(r.find("core:ac/kestrel"), None);
    }

    #[test]
    fn the_content_hash_is_the_one_in_the_pack() {
        let bytes = pack();
        let r = Registry::from_cpack(&bytes).unwrap();
        assert_eq!(r.content_hash(), &bytes[48..80]);
    }
}
