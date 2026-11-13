//! The synthetic fixture end to end: GP1 (two runs, identical bytes), and the archives' structure
//! through the reader (complete to the max level, the island where it should be).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use geo::angles::RAD;
use geo::sample::TilePosts;
use geo::tile::{TileId, tile_for};
use geo_pack::Archive;
use geo_pipeline::synth;

fn files(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.insert(
                    p.strip_prefix(dir).unwrap().to_path_buf(),
                    std::fs::read(&p).unwrap(),
                );
            }
        }
    }
    out
}

fn fresh(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn gp1_two_runs_give_identical_bytes() {
    let (a, b) = (fresh("synth-a"), fresh("synth-b"));
    synth::run(&a).unwrap();
    synth::run(&b).unwrap();
    let (fa, fb) = (files(&a), files(&b));
    assert_eq!(fa.keys().collect::<Vec<_>>(), fb.keys().collect::<Vec<_>>());
    for (k, v) in &fa {
        assert!(v == &fb[k], "{} differs between two runs", k.display());
    }
    let total: usize = fa
        .iter()
        .filter(|(k, _)| k.extension().is_some_and(|x| x == "gpk"))
        .map(|(_, v)| v.len())
        .sum();
    assert!(
        total <= 6_000_000,
        "the fixture's archives are {total} bytes (target ≤ 6 MB)"
    );
}

#[test]
fn the_fixture_is_complete_and_the_island_is_where_it_should_be() {
    let d = fresh("synth-c");
    synth::run(&d).unwrap();
    let alps = Archive::from_file(&d.join("cells/5-33-7.gpk")).unwrap();
    assert_eq!((alps.header().min_level, alps.header().max_level), (6, 12));
    assert_eq!(alps.header_json().cell, Some((33, 7)));
    assert_eq!(alps.header_json().licences, vec!["synthetic".to_string()]);
    // Every stored tile decodes; a stored tile's parent is stored (decimation from it).
    for e in alps.entries() {
        let t = TileId::from_id(e.tile_id).unwrap();
        assert!(alps.tile(t).unwrap().is_some());
        if t.z > 6 {
            assert!(
                alps.entry(t.parent().unwrap()).is_some(),
                "{t:?}'s parent is missing"
            );
        }
    }
    // The island's centre: the z12 tile is stored and its bicubic height matches the function.
    let (lat, lon) = (46.55 * RAD, 8.0 * RAD);
    let t = tile_for(lat, lon, 12);
    let dt = alps.tile(t).unwrap().expect("the island's centre tile");
    let posts = TilePosts::new(t, &dt.posts[..], dt.h_min, dt.h_max);
    let want = synth::height(lat, lon);
    let got = posts.bicubic(lat, lon);
    assert!((got - want).abs() < 2.0, "{got} against {want}");
    // The sea far from it is not stored.
    assert!(alps.entry(tile_for(46.55 * RAD, 10.5 * RAD, 12)).is_none());
    // The antimeridian island has tiles on both sides.
    let east = Archive::from_file(&d.join("cells/5-63-6.gpk")).unwrap();
    let west = Archive::from_file(&d.join("cells/5-0-6.gpk")).unwrap();
    assert!(!east.entries().is_empty() && !west.entries().is_empty());
    assert_eq!(east.header().max_level, 10);
    let base = Archive::from_file(&d.join("base.gpk")).unwrap();
    assert_eq!((base.header().min_level, base.header().max_level), (3, 5));
    assert_eq!(base.header_json().cell, None);
    assert!(base.entry(TileId::new(5, 33, 7).unwrap()).is_some());
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(d.join("datapack.json")).unwrap()).unwrap();
    assert_eq!(manifest["latest"], "geo-fixture");
    assert_eq!(
        manifest["versions"][0]["cells"].as_array().unwrap().len(),
        3
    );
}
