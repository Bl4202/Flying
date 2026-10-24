//! Helpers the unit tests share: the real core pack and a core flying its aircraft.

use crate::ownship::AircraftSlot;
use crate::session::SessionConfig;
use crate::sim::SimCore;

/// The real core pack, baked from `content/core` (the baker is a dev-dependency), with the
/// Kestrel's empty mass replaced so tests can tell two packs apart.
pub(crate) fn pack(empty_kg: u32) -> Vec<u8> {
    use content_bake::bake::{Sources, bake_sources};
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
    let read = |rel: &str| {
        let text = std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        (rel.to_owned(), text)
    };
    let mut kestrel = read("aircraft/kestrel.json");
    kestrel.1 = kestrel
        .1
        .replace("\"empty_kg\": 8400", &format!("\"empty_kg\": {empty_kg}"));
    bake_sources(&Sources {
        pack: read("pack.json"),
        locale: read("locales/en.json"),
        aircraft: vec![kestrel],
        engines: vec![read("engines/f129.json")],
        aero: vec![read("aero/kestrel.json")],
        fcs: vec![read("fcs/kestrel.json")],
    })
    .expect("the core pack bakes")
    .cpack
}

/// A started core flying the core pack's aircraft under `session`.
pub(crate) fn flying_under(seed: u64, session: SessionConfig) -> SimCore {
    let mut c = SimCore::new(seed);
    c.load_content(&pack(8400)).expect("the pack loads");
    c.configure(session).expect("idle");
    assert!(c.start());
    c
}

/// The aircraft of a core that has one.
pub(crate) fn slot(c: &SimCore) -> &AircraftSlot {
    c.ownship().aircraft().expect("an aircraft")
}
