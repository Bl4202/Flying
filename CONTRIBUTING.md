# Contributing

## Layering (GDD §61.7)

Lower layers never import higher ones. `sim-core` depends on nothing but `libm`; `protocol` and
`content` depend on no other crate. `cargo xtask check-layers` enforces an explicit allow-list per
crate (the table is in `crates/xtask/src/layers.rs`). In TypeScript,
`platform-web`, `render` and `tools` never import `app-shell` (dependency-cruiser).

## Determinism (ADR-10)

- No transcendental maths outside `sim_core::dmath` (clippy `disallowed-methods` enforces this).
- No `HashMap`/`HashSet` in sim crates; iterate in sorted, stable-id order.
- No fast-math flags, no `relaxed-simd`.
- Integer RNG only (`sim_core::rng`).

## Frames (GDD §0.3.2)

Name every vector with its frame: `posBF`, `velBody`, `dirECEF`. Mixed-frame arithmetic is a defect.
Angles in radians internally; degrees only with a `_deg` suffix.

## Golden hashes

`tests/golden/determinism.json` holds the expected state hashes. A change that alters sim
behaviour must re-bless it (`cargo xtask determinism --bless`), and the PR description must say
why. Never bless to make CI green without understanding the diff.

## Decisions

Significant decisions get an ADR in `docs/adr/` (copy `TEMPLATE.md`). Changing a gate threshold
needs an ADR (AC-66.4).

## Dependencies

Allow-listed licences only (MIT, Apache-2.0, BSD, ISC, MPL). Exact versions, lockfiles committed,
no postinstall scripts.
