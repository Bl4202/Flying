# Roadmap: v0.0 → v1.0

Companion to [GDD.md](GDD.md), mainly §66 (Development Roadmap), §67 (Prototype Requirements) and §68 (Design Priorities). Section numbers (§) refer to the GDD.

## How to read this

- **Version = milestone.** Each minor version (v0.1, v0.2, …) maps to one GDD milestone (M0–M6). Sub-milestones (a, b, c, d) split the bigger leaps, and the number of sub-milestones reflects how large the leap is.
- **Every sub-milestone ends with something runnable** on the staging URL and a short, testable exit check.
- **Durations are planning estimates** for a core team of 8–12 (GDD §66). They will be revised after v0.0.
- **Priority order governs cuts** (§68): flight physics → geographic sandbox → combat → AI → procedural missions → large battles → LAN/VLAN → 1v1 → campaign → advanced systems. Nothing already integrated gets removed; cut systems keep their interface and a stub (§61.5).
- **Gates (G1–G9)** are defined in §66.4. A version is closed only when all its exit items pass.
- v1.0 = M6 (Campaign) complete + release gate G9 (§71.6). Versions v0.7–v0.9 are the release-hardening stages that §66 folds into the M6 exit and G9. They are listed separately here so the path to 1.0 is visible.

## Overview

| Version | Name | GDD milestone | Sub-milestones | Est. duration | Playable result |
|---|---|---|---|---|---|
| **v0.0** | Foundations | M0 | a, b | 2 mo | Tech demo: render + sim + input + CI loop |
| **v0.1** | Flight Prototype | M1 | a, b, c, d | 4 mo | One aircraft, flying well |
| **v0.2** | Geographic Sandbox | M2 | a, b, c, d | 5 mo | Pick two places on Earth and fly between them |
| **v0.3** | Combat | M3 | a, b, c, d | 6 mo | **First Playable** (v0.3b), **Second Playable** (v0.3d) |
| **v0.4** | Procedural Missions | M4 | a, b, c | 5 mo | Generated, varied, escalating missions |
| **v0.5** | Multiplayer | M5 | a, b, c, d | 5 mo | **Third Playable**: LAN/VLAN co-op and 1v1 |
| **v0.6** | Campaign | M6 | a, b, c, d | 6 mo | **Campaign Build** |
| **v0.7** | Content & Polish | G7–G8 work | a, b | 3 mo | Roster, regions, audio and accessibility complete |
| **v0.8** | Public Beta | G5/G6 re-run | a, b | 2 mo | Opt-in public beta channel |
| **v0.9** | Release Candidate | G9 prep | a | 1 mo | Feature freeze, bug-fix only |
| **v1.0** | Release | G9 | — | — | Stable channel |

Total to the Campaign Build is about 33 months (M0–M6), plus about 6 months for v0.7–v1.0. Art, audio and data tracks run in parallel with the code and are off the critical path unless noted.

---

## v0.0: Foundations (M0, about 2 months)

**Goal:** prove the architecture end to end with the least possible content, so that every later version is "replace a stub", not "rewire".

### v0.0a: Repository, build, CI (about 4 weeks)
- Monorepo layout (§61.3): Rust workspace, TypeScript workspace, `/content`, `/tools`, `/server`, `/docs`.
- Build pipelines: Rust → WASM (threads and single-thread builds), TS bundling, WGSL validation, content-addressed outputs.
- CI: lint, unit tests, native and WASM builds, **cross-build state-hash test** (ADR-10), reproducible-build check.
- ADR repository seeded with ADR-01…20 (§0.4); open questions OQ-01…10 assigned owners.
- Staging deploy with COOP/COEP headers and Service Worker shell (§61.12).

**Exit:** green pipeline; staging URL serves a cross-origin-isolated page; WASM and native hashes equal after 10⁶ ticks.

### v0.0b: The empty loop (about 4 weeks)
- Sim Worker: fixed 60 Hz tick, drift-correcting accumulator, snapshot buffer (seqlock) (§0.6.1, §61.2).
- Render Worker: WebGPU on `OffscreenCanvas` with WebGL2 fallback; lit terrain patch; camera-relative f32 rendering (§44.8).
- Input ring and shaping pipeline (§58.2); compat-check screen (§53.3); debug overlay (FPS, stage timings) (§63).
- Content baker stub with one aircraft file; extension seams with stub implementations (§61.5).
- Performance harness skeleton (§60.9); first-run benchmark prototype (§60.8).

**Exit:** sim runs stably at 60 Hz in a worker while the render runs independently; runs on the three reference machines (Minimum / Recommended / High).

**Status (2026-10-14):** all work packages are merged to `main` and green in CI, and the scripted reference runs on the Mac (A: WebGPU, B: forced WebGL2) pass. The exit is **open on two reference runs on the project owner's laptop**: C (RTX 5060, WebGPU, High) and D (Chrome forced onto the integrated GPU, Minimum-class GPU). The owner will send those reports later; the `v0.0b` tag waits for them. Steps and pass criteria: [docs/runbooks/reference-machines.md](runbooks/reference-machines.md). Evidence so far: [docs/exit/v0.0b.md](exit/v0.0b.md).

---

## v0.1: Flight Prototype (M1, about 4 months)

**Goal:** "The aircraft must feel good to fly before adding extensive content" (outline §126). Priority P1.

### v0.1a: Flight core (about 5 weeks)
- 6-DOF rigid body with quaternion attitude, RK4 at 120 Hz for T0 (§11.2, §11.10).
- ISA atmosphere and air data (§11.3, §11.5); table-driven aerodynamics (§11.6) and propulsion (§11.8); mass properties (§11.9).
- Kestrel data (Appendix C) and the **Performance Conformance Test** harness (§11.19), running headless in CI.

**Exit:** the point-performance targets pass within ±3 % / ±5 % (`AC-3.2`); WASM vs native hashes equal for a 60 s scripted flight (`AC-11.7`).

**Note (2026-10-14):** the conformance method is fixed in [ADR-31](adr/0031-performance-conformance-test-method.md). The two ground-roll points of the envelope need the gear model, so at v0.1a they are reported as deferred and `AC-3.2` passes on the ten airborne points; it closes fully at v0.1b. Work is on branch `v0.1a` and merges to `main` only after `v0.0b` is tagged.

### v0.1b: Control and ground (about 4 weeks)
- Flight control system: Full / Standard / Reduced / Manual assist (§11.11); limiters (α, g, overspeed).
- Ground handling: gear model, friction, take-off and landing, crash criteria (§11.13).
- G-physiology and stall/departure behaviour (§11.14, §11.15).
- Keyboard, mouse (mouse-aim), gamepad and HOTAS input with binding editor v1 (§58).

**Exit:** a tester can take off, fly a pattern and land under every assist level; input-to-control latency ≤ 25 ms p95; the take-off and landing ground-roll conformance points deferred from v0.1a pass (`AC-3.2` complete).

### v0.1c: Presentation (about 4 weeks)
- Cockpit and chase cameras (§54.11); core HUD symbology and collimation (§54.2, §54.3).
- Procedural test arenas plus basic terrain (low-resolution global + local patch) (§44, basic).
- Engine, wind and warning-tone audio (§56 basic).
- First-run flow, settings, PWA install and offline shell (§53 minimal).

**Exit:** 60 FPS on the Recommended profile in the arena; horizon dip and curvature correct in screenshot tests.

### v0.1d: Basic combat stub and the feel gate (about 3 weeks)
- One gun and one IR missile (simple proportional navigation); one AI bandit (state machine) that pursues and defends.
- Debrief and statistics stub (§65 basic).
- **Gate G1 (flight feel)** with ≥ 12 blind testers (§66.4); 30-minute soak with no leaks.

**Exit:** G1 passes; ≥ 80 % of external playtesters take off, fly, shoot a bandit and land within 10 minutes (`AC-4.5` precursor).

---

## v0.2: Geographic Sandbox (M2, about 5 months)

**Goal:** "The player should be able to select two geographic locations and fly between them" (outline §127). Priority P2.

### v0.2a: Globe, frames and the pipeline (about 6 weeks)
- Geodesy, BF frame, battlefield system and validator (§7, §8).
- `geo-pipeline` and the `.gpk` GeoPack format for a pilot set of ~6 regions (§45.5).
- Streaming architecture, LOD selection and residency invariant (§44.3–§44.5).
- Globe rendering and atmosphere (§9.9, §24 basic).

**Exit:** seamless orbit-to-ground zoom; no holes in a scripted descent (`AC-44.1` precursor); round-trip geodesy tests (`AC-7.*`).

### v0.2b: World map and airports (about 6 weeks)
- World map UI: search (offline FTS), coordinate entry, selection tools, ruler, bookmarks (§9).
- Airport data merge, flattening, procedural completion and improvised strips (§10).
- Sim terrain layer and determinism with streaming (§44.6).

**Exit:** `AC-10.1` (≥ 98 % usable airports) and `AC-3.1` at ≥ 95 % (1 000 random locations playable).

### v0.2c: Terrain and world content (about 5 weeks)
- Terrain shading, biomes, water, vegetation tiers (§46.1–§46.9).
- Buildings, roads, bridges, power lines at the basic tiers (§47).
- Weather and time of day, basic (§24); sandbox configuration v1 (§30).

**Exit:** look-development reference boards reviewed (`AC-46.5`); building budget met in the dense-city scenario (`AC-47.1`).

### v0.2d: Load plan, cache and the two-location test (about 4 weeks)
- Load plan, prefetch, pipeline cache (§44.9); OPFS cache and the Data & Storage UI (§45.4).
- Failure matrix and fault injection (§45.10, §63.8).
- Terrain-character metrics and geography generation hooks (§46.11).

**Exit:** the two-location test passes on a cold cache and offline after caching; streaming budgets met (`AC-44.4`, `AC-44.5`).

---

## v0.3: Combat (M3, about 6 months)

**Goal:** weapons, radar, RWR, countermeasures, AI, ground targets and damage that work well. Priorities P3–P4. This version contains the two earliest playable builds from the outline.

### v0.3a: Weapons and sensors (about 7 weeks)
- Guns and ballistics; the missile flight model with seekers; the DLZ baker (§15).
- Radar model with modes, notch, RCS; RWR; LOS service with max-mip (§16).
- Chaff and flares (§17.5).
- HUD weapons and DLZ displays; radar and RWR pages (§54.4–§54.6).

**Exit:** hit-rate sweeps within targets (`AC-15.5`); LOS agrees with the reference (`AC-16.*`).

### v0.3b: **First Playable Build** (about 5 weeks)
The 11-step script (§67.1): map → location → battlefield → terrain → aircraft → fly → enemy → detect → missile → kill → back to the map.
- Aggregate damage model; one-bandit AI at three skill levels; simplified radar.
- Explosions, kill feedback, debrief.

**Exit:** the script passes as an automated smoke test and as a human checklist. **Gate G2 (fun gate):** ≥ 20 testers, ≥ 70 % would play another sortie. This is the project's go/no-go on the core concept.

### v0.3c: AI, squadrons and ground war (about 7 weeks)
- Pilot AI with utility + predictive BFM, full skill ladder (§19); formations and wingman orders (§20).
- Ground units and SAM networks (§21); component damage model (§18).
- Tactical map (§55); radio v1 (§56.6).

**Exit:** the AI ladder is monotone (`AC-59.4` precursor); the same-physics audit passes (`AC-3.3`).

### v0.3d: **Second Playable Build** (about 6 weeks)
- Multiple aircraft (≥ 5) and weapons (≥ 10).
- Airbase operations: take-off, landing, rearm, refuel, scramble basic (§48).
- Long-range operations: autopilot, cruise, time acceleration, AAR (§49); navigation and waypoints (§50).
- Objectives and reinforcements via hand-authored and early generator templates.

**Exit:** the §67.2 run completes in ≤ 45 minutes; ≥ 8/10 testers reach step 6.

---

## v0.4: Procedural Missions (M4, about 5 months)

**Goal:** a replayable war at any location. Priority P5.

### v0.4a: Mission system and generator (about 8 weeks)
- Mission model, DSL, VM and trigger engine (§25).
- Generator pipeline: determinism, seeds, placement, force budgeting, validation (§26).
- Mission types, first 8 of the 16 (§27); seed links and the Generator Lab (§26.3, §26.12).

**Exit:** 10⁴-seed batch validity ≥ 99 % (`AC-26.*`); WASM vs native golden hashes agree.

### v0.4b: Escalation, bosses and anti-repetition (about 7 weeks)
- Phases, the intensity director and reinforcement logic (§28).
- Boss and ace system (§29); remaining mission types.
- Anti-repetition: DNA, history penalties, twists, solution diversity (§69).

**Exit:** `AC-3.4` (mean DNA distance ≥ 0.45); SDI ≥ 3 for primary objectives; **Gate G3 (replayability)**.

### v0.4c: Player-facing systems (about 5 weeks)
- Difficulty dials and Realism Index (§59); accessibility baseline (§57).
- Replay (snapshot) and photo mode v1 (§52); statistics and logbook (§65).
- Sandbox configuration complete (§30); single-machine 1v1 arenas (§31).

**Exit:** G7 baseline; the ET-1 sandbox experience test passes in its quantitative parts (§70.2).

---

## v0.5: Multiplayer (M5, about 5 months)

**Goal:** a large cooperative air operation over LAN/VLAN, with no internet required. Priorities P6–P8.

### v0.5a: Transport and replication (about 6 weeks)
- WebRTC data channels, WebSocket LAN fallback, serverless invite blobs and QR (§33.3).
- Server-authoritative sim, snapshot replication, relevance filtering, priority accumulator (§33.5).
- Network debug panel and impairment simulator (§63.6).

**Exit:** a two-client session is stable under the impairment matrix.

### v0.5b: Prediction and hosting (about 5 weeks)
- Client prediction, reconciliation, interpolation and lag compensation (§33.6).
- Listen server in a tab, plus the native dedicated-server build and container (§33.9, §33.10).
- Join and state transfer, tile relay by the host (§33.4); voice (§56.12).

**Exit:** hit registration meets §33.14 at RTT 80 ms / 2 % loss; the 16-client headless soak passes.

### v0.5c: Co-op, 1v1 and scale (about 6 weeks)
- Lobby, roles, packages, EW console (§32, §17.8); 1v1 modes and arenas (§31).
- Ground and naval forces, carriers (§21, §22).
- Performance Governor and Huge battles (§60.7).

**Exit:** the PS-2 gate (150 aircraft); bandwidth budget met (`AC-60.8`).

### v0.5d: **Third Playable Build** (about 3 weeks)
- At least 20 packaged regions; replay completeness (omniscient replay) (§52); UGC editor v1 (§51).
- The §67.3 acceptance run: 4 humans + 70 AI, 30 minutes, no desync.

**Exit:** **Gate G5 (network)**; ET-3 passes; LAN-only play verified (`AC-61.8`).

---

## v0.6: Campaign (M6, about 6 months)

**Goal:** airports become strategic assets in a persistent roguelite war. Priority P9.

### v0.6a: Strategic layer (about 7 weeks)
- Airport graph, strategic map, frontlines, factions (§35, §37).
- Campaign structure and generation, operations calling the same generator (§34).
- Save system complete: checkpoints, migration, export/import (§64).

**Exit:** a run starts, plays an operation, saves and resumes; kill-the-tab test (`AC-64.1`).

### v0.6b: Capture, damage and resources (about 6 weeks)
- Airport capture and counter-capture (§36); persistent airbase damage and repair (§48.7).
- Resources, aircraft losses, supply logistics (§39).
- Campaign events and strategic decisions (§38.4, §40).

**Exit:** a capture changes reach and supply; sortie rates follow facility health (`AC-48.4`).

### v0.6c: People and knowledge (about 6 weeks)
- Pilot progression, traits and relationships (§41).
- Fog of war and intelligence (§42); emergent storytelling and the chronicle (§43).
- Recurring aces with identity continuity (§29.9, §43.5).

**Exit:** the chronicle test (§43.7); a recurring ace re-appears within 5 operations in ≥ 70 % of automated runs.

### v0.6d: **Campaign Build** (about 5 weeks)
- Enemy adaptation and Legacy Points meta-progression (§38.3, §38.5).
- Campaign UI layers (§35.6); run summary.
- The §67.4 acceptance run and the 20-run × 3-seed AI-played soak.

**Exit:** **Gate G6 (campaign loop)**; no soft-locks in the soak; ET-2 passes.

---

## v0.7: Content & Polish (about 3 months)

**Goal:** fill out and finish everything that is data or presentation, without changing the engine.

### v0.7a: Content completion (about 6 weeks)
- Roster to 12 aircraft, 26 weapons + 6 store types, 4 factions, all 16 mission types (§13.4, §15.12, §37.3).
- Hero-polished airbases (≥ 30); regions up to the release set plus worldwide streaming.
- Full audio set, music director, voice banks (§56.8, §56.6).

**Exit:** content gates pass (balance bands §13.2, `AC-56.7`); the `NoHardcodedContentLint` is clean.

### v0.7b: Accessibility, localisation and UX (about 6 weeks)
- Full accessibility set (§57); external accessibility review.
- Localisation to the target languages (§53.10); training missions (§53.11).
- Photo mode and the video-export pipeline complete (§52.8–§52.10).

**Exit:** **Gate G7** complete; usability SUS ≥ 75 and first flight ≤ 10 minutes.

---

## v0.8: Public Beta (about 2 months)

### v0.8a: Beta channel (about 4 weeks)
- Public opt-in `beta` channel with opt-in telemetry (§53.13, §66.8).
- Crash-free session tracking, performance-profile distribution.
- Licence and IP audit: **Gate G8** (descriptors, naming, music).

### v0.8b: Beta fixes (about 4 weeks)
- Re-run gates G5 and G6 on real home networks and a wider hardware set.
- Balance and tuning from telemetry and playtests; fix list burn-down.

**Exit:** crash-free sessions ≥ 99.5 %; no open P0/P1 bugs.

---

## v0.9: Release Candidate (about 1 month)

### v0.9a: Freeze and verify
- Feature freeze; bug-fix only.
- Full regression: all acceptance criteria, performance gates PS-1…PS-9, screenshot suite, security/CSP audit, reproducible-build check.
- Documentation: player manual, controls cheat sheets, UGC docs, support runbook.

**Exit:** the release checklist (§71.6) is all green.

---

## v1.0: Release

- **Stable channel** published; `datapack.json` and content packs live on the CDN.
- Definition-of-success review: the eleven areas of §71.2, each with an owner and evidence.
- Post-launch track begins (M7, §66.7): aircraft and weapon growth, regions, naval depth, IADS, EW roles, daily seeds, community hub (OQ-10), WebXR research.

---

## Cross-cutting tracks

| Track | Starts | Notes |
|---|---|---|
| Art (aircraft, cockpits, environments) | v0.1 | Kit-bashed roster is acceptable (OQ-05); budgets in §60.6 |
| Audio | v0.1 (engines, tones) | Full mix v0.4+, music v0.7 |
| Geodata pipeline and licensing | v0.2 | Descriptors audited every datapack release (§45.9) |
| Performance gates | v0.0 | PS-1…PS-9 introduced as their scenarios become possible (§60.9) |
| Accessibility | v0.1 | Baseline v0.4, full v0.7 |
| Playtests | v0.1 weekly internal; external monthly from v0.2 | Protocol in §66.4 |

## Dependency order (critical path)

Foundations (v0.0) → flight model (v0.1) → terrain and streaming (v0.2) → weapons and sensors (v0.3a) → AI (v0.3c) → mission system and generator (v0.4) → networking (v0.5) → campaign (v0.6) → polish and release (v0.7–v1.0).

## Cut lines

If the schedule slips, cut from the bottom of the priority list (§66.6):
1. Advanced systems (deep naval, logistics detail, EW depth) move to M7.
2. v0.3 may ship 12 weapons and 8 aircraft; the schema is unchanged.
3. v0.5 may ship without WebTransport or voice.
4. v0.6 may ship 2 factions and no storylet system (the event stream stays).
5. The WebGL2 fallback may be dropped if WebGPU coverage is sufficient (§66.6).

## Key risks by version

| Version | Main risk | Mitigation |
|---|---|---|
| v0.0 | Cross-origin isolation / WASM threads unavailable on the host | Degraded single-worker mode designed in from the start (§61.2) |
| v0.1 | Flight feel not good enough | G1 blocks all content work until it passes |
| v0.2 | Streaming holes or hitches | Residency invariant and sim terrain layer (§44.5.4, §44.6) |
| v0.3 | The core concept isn't fun | G2 at v0.3b, before any breadth is built |
| v0.4 | Unsolvable or repetitive missions | Solvability validation and DNA metrics in CI |
| v0.5 | Browser networking limits | Serverless invite blobs, WebSocket LAN fallback, extensive impairment testing |
| v0.6 | Campaign soft-locks | AI-played campaign soak every build |
| v0.7–v1.0 | Scope and licensing | Cut lines, G8 audit |

Full register: Appendix F of the GDD.
