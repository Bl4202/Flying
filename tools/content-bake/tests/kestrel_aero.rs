//! The Kestrel's aero tables are generated; the committed JSON must be exactly what the generator
//! writes, so the data and its derivation cannot drift apart.

use std::fs;
use std::path::Path;

#[path = "../../../content/core/aero/kestrel_gen.rs"]
mod kestrel_gen;

#[test]
fn the_committed_tables_are_what_the_generator_writes() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core/aero/kestrel.json");
    let text = kestrel_gen::render();
    if std::env::var_os("KESTREL_AERO_WRITE").is_some() {
        fs::write(&path, &text).expect("write kestrel.json");
    }
    let committed = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        committed == text,
        "content/core/aero/kestrel.json differs from its generator; run \
         `KESTREL_AERO_WRITE=1 cargo test -p content-bake --test kestrel_aero` and commit"
    );
}
