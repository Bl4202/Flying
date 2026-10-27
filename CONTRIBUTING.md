# Contributing

## Layering (GDD §61.7)

Lower layers never import higher ones. `simmath` depends on nothing but `libm`; `flight` on
`simmath` and `content`; `fcs` (the control laws) on `flight`, `simmath` and `content`;
`flight-test` on `flight`, `simmath`, `content` and `serde_json`; `sim-core` on `content`,
`simmath`, `flight` and `fcs`; `cli` additionally on `flight-test` and `serde_json`; `protocol` and
`content` depend on no other crate. `cargo xtask check-layers` enforces an explicit allow-list per crate (the table is in
`crates/xtask/src/layers.rs`).

In TypeScript the tiers, low to high, are:

| Tier | Packages |
|---|---|
| 1 | `platform-web` |
| 2 | `sim-bridge` |
| 3 | `sim-host`, `input`, `render`, `audio` |
| 4 | `perf` |
| 5 | `ui` |
| 6 | `app-shell` |

A package imports only from lower tiers, and packages in the same tier do not import each other.
`tools` is build-time only: it may use `platform-web` and `sim-bridge`, and only `app-shell` (the
Vite config) may import it. dependency-cruiser enforces all of this (`.dependency-cruiser.cjs`).

## Seams (GDD §61.5, ADR-27)

Anything the first build cannot afford goes behind an interface with a simple implementation and a
**contract function** that every implementation must pass. To add an implementation, add it to the
contract's test; to replace a stub, make it pass the contract and change no caller. If a caller has
to change, the seam was wrong: fix the seam first. A seam marked "provisional" in
`crates/sim-core/src/seams/mod.rs` may change its signature when its first real user arrives.

## Generated files

Four sets of files are generated and committed; lint fails if they are stale, so regenerate and commit
them with the change that needs them.

| Files | Source | Regenerate |
|---|---|---|
| `packages/sim-bridge/src/layout.gen.ts`, `tests/vectors/*.json` | `crates/sim-core/src/layout.rs` | `cargo xtask gen-abi` |
| `content/schemas/*.schema.json` | `tools/content-bake/src/model.rs` | `cargo xtask content --schemas` |
| `packages/render/shaders/gen/*` | `packages/render/shaders/*.wgsl` | `pnpm wgsl:gen` |
| `tests/golden/determinism.json`, `tests/golden/flight60.json`, `tests/golden/circuit.json` | the simulation | `cargo xtask determinism --bless` (see below) |
| `content/core/aero/kestrel.json` | `content/core/aero/kestrel_gen.rs` | `KESTREL_AERO_WRITE=1 cargo test -p content-bake --test kestrel_aero` |

## No allocation on the hot paths

The tick (`fg_step`, `fg_push_inputs`, the pointer getters) allocates nothing; a counting allocator
test fails the build if it does. The same discipline applies to the Sim Worker's loop, the Render
Worker's frame and the input pump: allocate at start-up, reuse after. The statistics classes in
`platform-web` (`Histogram`, the rings) exist so measuring does not allocate either.

## TypeScript conventions

Strict type-checked ESLint: no non-null assertions, no `any`, no void-returning arrow shorthand,
`import type` for types. `erasableSyntaxOnly` is on, so no parameter properties or enums.
Build DOM with `textContent` (the `ui` helpers in `dom.ts`), never `innerHTML`. Every
user-visible string is a key in `packages/ui/src/strings.ts`.

## Testing

Unit tests sit next to the code (`*.test.ts`, `#[cfg(test)]`). A seam's contract lives in a
`*.contract.ts` file or a `contract` function and is run against every implementation. End-to-end
specs are in `tests/e2e/`, run on the bundled Chromium headless shell (what CI uses), never on a
locally installed Chrome alone (`PW_CHANNEL=chrome` is a quick extra check): a v0.0a Service Worker bug
passed on Chrome 146 and failed on the bundled browser. WebGPU cannot be presented there, so
specs that need it skip, and `PW_REQUIRE_WEBGPU=1` makes them required on a machine with a GPU.

## Keys and browser shortcuts

The flying keys follow a real stick (`W` is stick forward, nose down). The GDD's default for
"throttle down" is `Ctrl`, but browsers keep `Ctrl+W` (close tab), `Ctrl+R` and `Ctrl+S` for
themselves and a page cannot stop them, so holding `Ctrl` and pitching can close the tab. `Page Up`
and `Page Down` also move the throttle and have no such side effects. The debug overlay is
`Ctrl+Shift+D`. Bindings are data (`controls/1` profiles, `docs/architecture/controls.md`): when
adding a default, check it against the browser's reserved shortcuts (the conflict check in
`packages/input/src/conflicts.ts` lists them), and keep an action to four bindings at most.
Gamepad and HOTAS defaults are tested only with stubbed devices; do not claim more.

## Determinism (ADR-10)

- No transcendental maths outside `simmath::dmath`, also reachable as `sim_core::dmath` (clippy
  `disallowed-methods` enforces this).
- No `HashMap`/`HashSet` in sim crates; iterate in sorted, stable-id order.
- No fast-math flags, no `relaxed-simd`.
- Integer RNG only (`sim_core::rng`).

## Frames (GDD §0.3.2)

Name every vector with its frame: `posBF`, `velBody`, `dirECEF`. Mixed-frame arithmetic is a defect.
Angles in radians internally; degrees only with a `_deg` suffix.

## Golden hashes

`tests/golden/determinism.json` (the 10⁶-tick erratic run, air spawn, Standard assist),
`tests/golden/flight60.json` (the 60 s scripted flight, `AC-11.7`, air spawn, Manual assist) and
`tests/golden/circuit.json` (the virtual tester's circuit from the runway, Standard assist) hold the
expected state hashes. Each suite names its session (spawn, assist, g-effects) in
`crates/xtask/src/determinism.rs`. A change that alters sim
behaviour must re-bless them (`cargo xtask determinism --bless`), in a commit of its own whose
message says why and which hash columns changed. Never bless to make CI green without
understanding the diff.

While a change is in progress, `cargo xtask determinism --compare-only` checks that native, debug
and both WASM builds agree with each other without comparing them to the goldens. Use it to keep
WASM and native equal before the final bless.

## Aircraft data and the conformance test (ADR-31, ADR-32)

`cargo xtask conformance` (also in `pnpm verify` and CI) flies each aircraft's performance
envelope and fails on any point outside its tolerance. When a point fails, **retune the data, not
the target**: change the aircraft, engine or aero files, never the envelope's `value` or
`tol_pct`. Record every change, which point it moved and by how much in the tuning log of
`content/core/aero/README.md` (engine data in `content/core/engines/README.md`). Changing a target
or a tolerance needs an ADR (AC-66.4). How to read the report is in `docs/architecture/flight.md`.

Two more gates fly the aircraft the way a player does. `cargo xtask circuit` (a step of the CI
`conformance` job) has the virtual tester take off, fly the pattern and land under every assist
level; control-law gains are data (`content/core/fcs/kestrel.json`), every change is logged in
that folder's README, and the handling and departure suites in `crates/fcs/tests` judge them
(`docs/architecture/fcs.md`). The input-to-control latency of `AC-58.3` is measured by the sim on
frames that carry a change: CI holds it to two ticks (`tests/e2e/latency.spec.ts`), and the strict
25 ms p95 gate is judged on the reference machine with `pnpm perf:local --latency 500`
(`docs/runbooks/reference-machines.md`).

The simulation crates never name content: they find an aircraft by handle and kind, not by id
(`AC-12.4`). `cargo xtask check-content-ids` (in `pnpm lint`) fails on a content id in a string, or
on any content entry's name used as a word, anywhere in `simmath`, `flight`, `fcs`,
`flight-test`, `sim-core` or `sim-wasm` outside comments and tests.

## Decisions

Significant decisions get an ADR in `docs/adr/` (copy `TEMPLATE.md`). Changing a gate threshold
needs an ADR (AC-66.4).

## Dependencies

Allow-listed licences only (MIT, Apache-2.0, BSD, ISC, MPL). Exact versions, lockfiles committed,
no postinstall scripts.
