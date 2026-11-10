# ADR-23: State hash algorithm and golden-file policy

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-10
- **Affects:** ADR-10, GDD 61.4.4, 63.2 (`hash` command), AC-11.7; `crates/sim-core/src/hash.rs`, `tests/golden/determinism.json`

## Context

ADR-10 needs a cheap, portable way to tell whether two builds, or two commits, produced the same
simulation state. It is a regression and desync *detector*, not a security measure.

## Decision

- **Algorithm:** FNV-1a 64 over canonical little-endian bytes. `f64` values hash by IEEE-754 bit
  pattern; every NaN collapses to `0x7FF8000000000000` (payloads may legitimately differ between
  targets); `-0.0` and `+0.0` stay distinct.
- **Granularity:** per-subsystem hashes (`tick`, `rng`, `bodies`) plus a `combined` hash of those,
  so a mismatch points at a subsystem. Hashes cross the WASM boundary as two `u32` halves and print
  as 16-digit lower-case hex.
- **What is checked:** native release, native debug (at 10^4 ticks), `single` and `threads` WASM
  under Node must agree at 15 checkpoints up to 10^6 ticks (every power of ten, every 100 000, and
  the end). The native result must equal `tests/golden/determinism.json`. The golden must also hold
  on arm64 (CI `determinism-arm64`).
- **Stand-in workload.** Until the flight model exists, the hash covers a 32-body probe world that
  exercises division, `sqrt`, `exp`, `pow`, `sin`, `cos`, `atan2`, integer RNG, quaternion
  integration and renormalisation. When `flight` lands (v0.1a) the golden is re-blessed and
  `AC-11.7`'s 60 s scripted flight takes over as the primary check.
- **Blessing policy.** The golden changes only through `cargo xtask determinism --bless`, in a
  commit whose message says why simulation behaviour changed. Blessing to make CI pass without
  understanding the diff is not allowed; WASM and native must agree *before* a bless is accepted.

## Alternatives considered

- *SHA-256:* stronger than needed, slower on the hot path, and no benefit for change detection.
- *Hash only the final state:* a divergence early in a long run is much harder to localise.
- *Bit-exactness guarantee for gameplay:* explicitly not claimed (ADR-10); the server is
  authoritative, and a mismatch in play is a bug report, not a desync.

## Consequences

- Any intentional change to `sim-core` behaviour touches the golden file and is visible in review.
- The probe world is real code that the v0.0b fixed-rate loop will step.

## Amendment 1 (2026-11-10, v0.1d): the combat column and the one re-bless

- **Seven columns.** The per-subsystem hashes are now `tick_h`, `rng`, `bodies`, `ownship`,
  `input`, `content` and `combat`, and `combined` hashes all seven. `combat` (ADR-48) covers every
  number of a duel's combat state and nothing observational; in a session without combat it is the
  hash of the tag `NOCOMBAT`, a constant (`31ef7ff32813f286`). `hash_kind::COMBAT = 7`.
- **Five suites.** `cargo xtask determinism` checks `determinism`, `flight60`, `circuit`,
  `plain-circuit` and, new, `combat` (`tests/golden/combat.json`: the bot's 10-minute duel sortie
  against a Regular, every 600 ticks), native release, native debug and both WASM builds alike.
- **The v0.1d re-bless.** v0.1d puts the weapon and AI data in the simulation pack (owner decision
  D2, ADR-45), and the pack's SHA-256 is hashed, so the `content` and `combined` columns of the four
  free-flight goldens move. They are re-blessed **once**, at the end of WP4 after the last data
  change, in a commit that does nothing else. The proof is `cargo xtask golden-diff --against
  v0.1c`: same seeds, sessions and checkpoints; `tick_h`, `rng`, `bodies`, `ownship` and `input`
  identical everywhere; `content` one value in every file; `combat` the no-combat constant; and
  `combined` exactly what the seven columns hash to. Its output goes in the commit's body. Before
  the re-bless, `golden-diff --against v0.1c --fresh` compares a fresh run with the old goldens.
  A second re-bless of those four in v0.1d needs the owner's OK.
- **The combat golden** follows the blessing policy above. It holds the bot's code as well as the
  core's, so a change to `ai-pilot`'s behaviour moves it too.
