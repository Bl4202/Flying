//! The registry: baked content behind `u32` handles.

use crate::aero::AeroDef;
use crate::aircraft::AircraftDef;
use crate::cpack::{self, CpackError, Entry};
use crate::engine::EngineDef;
use crate::fcs::FcsDef;

/// Handle of a baked content entry: its index in the pack's table of contents, which is sorted
/// by id. Handles are stable for a given pack and meaningless across packs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContentId(pub u32);

/// Loaded content: immutable, shared by reference, looked up by handle (`O(1)`) or id (`O(log n)`).
#[derive(Clone, Debug)]
pub struct Registry {
    ids: Vec<String>,
    entries: Vec<Entry>,
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
            entries: Vec::new(),
            content_hash: [0; 32],
        }
    }

    /// Reads a `.cpack`, checking its checksum, its structure and its references.
    ///
    /// # Errors
    /// Any [`CpackError`] other than the writer-only ones.
    pub fn from_cpack(bytes: &[u8]) -> Result<Self, CpackError> {
        let p = cpack::parse(bytes)?;
        Ok(Self {
            ids: p.ids,
            entries: p.entries,
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

    /// The entry behind a handle.
    #[must_use]
    pub fn entry(&self, handle: ContentId) -> Option<&Entry> {
        self.entries.get(handle.0 as usize)
    }

    /// The aircraft behind a handle, if it is one.
    #[must_use]
    pub fn aircraft(&self, handle: ContentId) -> Option<&AircraftDef> {
        match self.entry(handle)? {
            Entry::Aircraft(d) => Some(d),
            _ => None,
        }
    }

    /// The engine behind a handle, if it is one.
    #[must_use]
    pub fn engine(&self, handle: ContentId) -> Option<&EngineDef> {
        match self.entry(handle)? {
            Entry::Engine(d) => Some(d),
            _ => None,
        }
    }

    /// The aero set behind a handle, if it is one.
    #[must_use]
    pub fn aero(&self, handle: ContentId) -> Option<&AeroDef> {
        match self.entry(handle)? {
            Entry::Aero(d) => Some(d),
            _ => None,
        }
    }

    /// The control-law gains behind a handle, if it is one.
    #[must_use]
    pub fn fcs(&self, handle: ContentId) -> Option<&FcsDef> {
        match self.entry(handle)? {
            Entry::Fcs(d) => Some(d),
            _ => None,
        }
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
    pub fn iter(&self) -> impl Iterator<Item = (ContentId, &str, &Entry)> {
        self.ids
            .iter()
            .zip(&self.entries)
            .enumerate()
            .map(|(i, (id, e))| (ContentId(i as u32), id.as_str(), e))
    }

    /// Handles of the aircraft, in handle order.
    pub fn aircraft_handles(&self) -> impl Iterator<Item = ContentId> + '_ {
        self.iter()
            .filter(|(_, _, e)| matches!(e, Entry::Aircraft(_)))
            .map(|(h, _, _)| h)
    }

    /// Number of entries of each kind: (aircraft, engines, aero sets, control gains).
    #[must_use]
    pub fn counts(&self) -> (usize, usize, usize, usize) {
        self.entries
            .iter()
            .fold((0, 0, 0, 0), |(a, e, r, f), entry| match entry {
                Entry::Aircraft(_) => (a + 1, e, r, f),
                Entry::Engine(_) => (a, e + 1, r, f),
                Entry::Aero(_) => (a, e, r + 1, f),
                Entry::Fcs(_) => (a, e, r, f + 1),
            })
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
        assert_eq!(r.len(), 5);
        for (i, id) in [
            "core:ac/alpha",
            "core:ac/zulu",
            "core:aero/one",
            "core:eng/one",
            "core:fcs/one",
        ]
        .iter()
        .enumerate()
        {
            let h = r.find(id).unwrap();
            assert_eq!(h, ContentId(i as u32));
            assert_eq!(r.id(h), Some(*id));
        }
        let ids: Vec<_> = r.iter().map(|(_, id, _)| id).collect();
        assert_eq!(
            ids,
            [
                "core:ac/alpha",
                "core:ac/zulu",
                "core:aero/one",
                "core:eng/one",
                "core:fcs/one"
            ]
        );
        assert_eq!(r.counts(), (2, 1, 1, 1));
    }

    #[test]
    fn typed_lookups_follow_the_kind() {
        let r = Registry::from_cpack(&pack()).unwrap();
        let ac: Vec<_> = r.aircraft_handles().collect();
        assert_eq!(ac, [ContentId(0), ContentId(1)]);
        let a = r.aircraft(ac[0]).unwrap();
        assert!(r.engine(a.engines[0].engine).is_some());
        assert!(r.aero(a.aero.tables).is_some());
        assert!(r.fcs(a.fcs.gains).is_some());
        assert!(r.aircraft(ContentId(2)).is_none());
        assert!(r.engine(ContentId(0)).is_none());
        assert!(r.aero(ContentId(3)).is_none());
        assert!(r.fcs(ContentId(0)).is_none());
    }

    #[test]
    fn unknown_ids_and_handles_are_none() {
        let r = Registry::from_cpack(&pack()).unwrap();
        assert_eq!(r.find("core:ac/nope"), None);
        assert_eq!(r.find(""), None);
        assert!(r.entry(ContentId(5)).is_none());
        assert!(r.id(ContentId(u32::MAX)).is_none());
    }

    #[test]
    fn the_empty_registry_has_a_zero_hash() {
        let r = Registry::empty();
        assert!(r.is_empty());
        assert_eq!(r.content_hash(), &[0; 32]);
        assert_eq!(r.aircraft_handles().count(), 0);
    }

    #[test]
    fn the_content_hash_is_the_one_in_the_pack() {
        let bytes = pack();
        let r = Registry::from_cpack(&bytes).unwrap();
        assert_eq!(r.content_hash(), &bytes[48..80]);
    }
}
