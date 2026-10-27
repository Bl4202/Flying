# GAME DESIGN DOCUMENT
## Real-World Sandbox Combat Flight Simulator (Browser-Native)

| | |
|---|---|
| **Working Title** | TBD |
| **Document Version** | 0.2.2 (Draft 2 — technical expansion of `GDD_Outline.md`, retargeted to run in a web browser) |
| **Document Date** | 2026-10-07 |
| **Primary Platform** | Desktop web browser (installable PWA). WebGPU renderer, WebAssembly simulation core. |
| **Multiplayer** | LAN / VLAN / direct-connect co-op and 1v1 over WebRTC |
| **World** | Real-world Earth geography (WGS84), fictional factions |
| **Source Document** | `GDD_Outline.md` |

---

# TABLE OF CONTENTS

0. [Document Guide, Conventions & Decision Log](#0-document-guide-conventions--decision-log)
1. [Game Overview](#1-game-overview)
2. [Vision Statement](#2-vision-statement)
3. [Design Pillars](#3-design-pillars)
4. [Target Player Experience](#4-target-player-experience)
5. [Core Gameplay Loop](#5-core-gameplay-loop)
6. [Game Modes](#6-game-modes)
7. [World & Geographic System](#7-world--geographic-system)
8. [Battlefield System](#8-battlefield-system)
9. [World Map](#9-world-map)
10. [Airport System](#10-airport-system)
11. [Flight Model](#11-flight-model)
12. [Aircraft System](#12-aircraft-system)
13. [Aircraft Categories](#13-aircraft-categories)
14. [Aircraft Customization](#14-aircraft-customization)
15. [Weapons System](#15-weapons-system)
16. [Radar & Avionics](#16-radar--avionics)
17. [Electronic Warfare](#17-electronic-warfare)
18. [Damage Model](#18-damage-model)
19. [AI System](#19-ai-system)
20. [Squadron System](#20-squadron-system)
21. [Ground Warfare](#21-ground-warfare)
22. [Naval Warfare](#22-naval-warfare)
23. [Civilian World](#23-civilian-world)
24. [Weather & Environment](#24-weather--environment)
25. [Mission System](#25-mission-system)
26. [Mission Generator](#26-mission-generator)
27. [Mission Types](#27-mission-types)
28. [Dynamic Mission Escalation](#28-dynamic-mission-escalation)
29. [Boss & Ace System](#29-boss--ace-system)
30. [Sandbox Configuration](#30-sandbox-configuration)
31. [1v1 Mode](#31-1v1-mode)
32. [LAN/VLAN Co-op](#32-lanvlan-co-op)
33. [Multiplayer Architecture](#33-multiplayer-architecture)
34. [Roguelite Conquest Campaign](#34-roguelite-conquest-campaign)
35. [Strategic Map](#35-strategic-map)
36. [Airport Capture](#36-airport-capture)
37. [Factions](#37-factions)
38. [Campaign Progression](#38-campaign-progression)
39. [Campaign Resources](#39-campaign-resources)
40. [Campaign Events](#40-campaign-events)
41. [Pilot Progression](#41-pilot-progression)
42. [Fog of War & Intelligence](#42-fog-of-war--intelligence)
43. [Emergent Storytelling](#43-emergent-storytelling)
44. [World Streaming](#44-world-streaming)
45. [Geographic Data](#45-geographic-data)
46. [Terrain & Biomes](#46-terrain--biomes)
47. [Cities & Infrastructure](#47-cities--infrastructure)
48. [Airbase Operations](#48-airbase-operations)
49. [Long-Range Operations](#49-long-range-operations)
50. [Navigation](#50-navigation)
51. [User-Generated Content](#51-user-generated-content)
52. [Replay & Photo Mode](#52-replay--photo-mode)
53. [UI / UX](#53-ui--ux)
54. [HUD](#54-hud)
55. [Tactical Map](#55-tactical-map)
56. [Audio](#56-audio)
57. [Accessibility](#57-accessibility)
58. [Controls](#58-controls)
59. [Difficulty & Realism](#59-difficulty--realism)
60. [Performance](#60-performance)
61. [Technical Architecture](#61-technical-architecture)
62. [Data-Driven Content](#62-data-driven-content)
63. [Debugging Tools](#63-debugging-tools)
64. [Save System](#64-save-system)
65. [Statistics](#65-statistics)
66. [Development Roadmap](#66-development-roadmap)
67. [Prototype Requirements](#67-prototype-requirements)
68. [Design Priorities](#68-design-priorities)
69. [Anti-Repetition Systems](#69-anti-repetition-systems)
70. [Final Player Experience](#70-final-player-experience)
71. [Definition of Success](#71-definition-of-success)

**Appendices**

- [Appendix A: Data Schemas](#appendix-a-data-schemas)
- [Appendix B: Formula & Algorithm Reference](#appendix-b-formula--algorithm-reference)
- [Appendix C: Reference Aircraft Data Sheet (Kestrel)](#appendix-c-reference-aircraft-data-sheet-kestrel)
- [Appendix D: Launch Content Rosters](#appendix-d-launch-content-rosters)
- [Appendix E: Test Plan & QA Matrix](#appendix-e-test-plan--qa-matrix)
- [Appendix F: Risk Register](#appendix-f-risk-register)
- [Appendix G: Glossary & Radio Brevity](#appendix-g-glossary--radio-brevity)
- [Appendix H: Geodata Sources & License Matrix](#appendix-h-geodata-sources--license-matrix)
- [Appendix I: Document Maintenance](#appendix-i-document-maintenance)

---

# 0. DOCUMENT GUIDE, CONVENTIONS & DECISION LOG

This chapter is normative for every other chapter. Where a later chapter contradicts a value defined here, **this chapter wins** and the later chapter is a bug.

## 0.1 Purpose and audience

This document expands the outline into an implementable specification. The product runs **in a web browser** (§61.2): there is nothing to install, the game loads from a URL, and it can be installed as a PWA for offline use. This single constraint shapes the engine, networking, storage, threading and performance chapters.

| Audience | What they should read first |
|---|---|
| Designers | §1–6, §25–43, §69–71 |
| Gameplay / simulation engineers | §0, §11–24, §60–63, Appendix B |
| Networking / tools / pipeline engineers | §0, §33, §44–47, §61–64, Appendix A and H |
| Producers / QA | §0.5, §66–68, Appendix E and F |

Every chapter follows the same skeleton where applicable:

1. **Intent** – what the system is for and what the player should feel.
2. **Rules** – player-visible behaviour.
3. **Specification** – data structures, algorithms, parameters, formulas.
4. **Edge cases and failure handling.**
5. **Tuning knobs** – the values exposed in data files.
6. **Acceptance criteria** – testable statements with IDs of the form `AC-<section>.<n>`.

## 0.2 Normative language and tags

- **MUST / MUST NOT / SHOULD / MAY** are used in the RFC-2119 sense.
- **Design target** marks a number that is an intended starting value to be tuned by playtesting. Design targets are not claims about real aircraft or real weapons. Every real-world-sounding number in this document is a *gameplay* number unless it is a physical constant, a standard-atmosphere value, or a geodetic constant.
- **Priority tags** map to the outline's priority order (§68): `P1` flight physics, `P2` geographic sandbox, `P3` combat, `P4` AI, `P5` procedural missions, `P6` large battles, `P7` LAN/VLAN, `P8` 1v1, `P9` campaign, `P10` advanced systems.
- **Milestone tags** `M0`–`M7` map to the development milestones in §66: `M0` foundations, `M1` flight prototype, `M2` geographic sandbox, `M3` combat, `M4` procedural missions, `M5` multiplayer, `M6` campaign, `M7` long-term content.
- Section references use `§N` or `§N.M`. Appendix references use `Appendix X`.
- Browser-specific platform facts (API availability, limits) change over time. They are marked **[verify]** where an implementer MUST re-check current browser support before relying on them.

## 0.3 Units, coordinate frames, and math conventions

### 0.3.1 Units

All simulation state is stored in **SI units**. Conversion happens only at the UI boundary.

| Quantity | Internal unit | Notes |
|---|---|---|
| Length | metre (m) | Altitude is always metres above mean sea level (MSL) unless named `agl`. |
| Time | second (s) | Simulation time is a 64-bit tick counter × Δt, never accumulated floats. |
| Mass | kilogram (kg) | |
| Force | newton (N) | |
| Angle | radian (rad) | Degrees only in data files and UI, always with a `_deg` suffix in field names. |
| Speed | metre/second (m/s) | UI offers knots, km/h, mph, Mach. |
| Pressure | pascal (Pa) | |
| Temperature | kelvin (K) | UI offers °C. |
| RCS | square metre (m²) | Stored linear; authored in dBsm in data (`dBsm = 10·log10(m²)`). |

Display conversions (exact definitions): 1 kn = 0.514444… m/s (1 m/s = 1.943844 kn); 1 ft = 0.3048 m (1 m = 3.28084 ft); 1 nmi = 1852 m; 1 lbf = 4.4482216 N; 1 lb = 0.45359237 kg.

### 0.3.2 Coordinate frames

Six frames exist. Code MUST name every vector with its frame (e.g. `posBF`, `velBody`, `dirECEF`). Mixed-frame arithmetic is a defect.

| Frame | Symbol | Definition | Used for |
|---|---|---|---|
| Geodetic | GEO | WGS84 latitude φ, longitude λ, height h. Height is **MSL height** (see ADR-05). | Data, UI, search, bookmarks, seeds |
| Earth-Centred Earth-Fixed | ECEF | Cartesian, origin at Earth's centre, +Z through the north pole, +X through (0°, 0°). | Intermediate for conversions only |
| **Battlefield Frame** | **BF** | Rigid transform of ECEF: origin at the **anchor** point, axes **East (+x), North (+y), Up (+z)** of the tangent plane at the anchor. Earth's centre is therefore *below* the origin. | **All simulation state, physics, AI, networking** |
| Body | BODY | Aircraft-fixed: **+x forward, +y right, +z down** (aerospace FRD). Origin at the aircraft reference point (CG for dynamics). | Flight dynamics, weapons, sensors |
| Wind / stability | WIND | x along the airspeed vector (wind axes) or its projection (stability axes). | Aerodynamic coefficient application |
| Render | RF | Camera-relative, 32-bit float, renderer axes (see below). | Rendering and audio only |

**BF is curved.** Positions are 64-bit floats in BF. "Up" at any point is the local ellipsoid normal, **not** the +z axis of BF (they coincide only at the anchor). Gravity points along the local down direction. This is what makes a 1000 km battlefield behave like Earth (§7.4).

**Renderer axis mapping.** The reference renderer (ADR-18) is right-handed, Y-up. BF → renderer: `r.x = BF.x (East)`, `r.y = BF.z (Up)`, `r.z = −BF.y (−North)`. This is a proper rotation (determinant +1) of **−90° about the East axis**, so no handedness flip is needed. Quaternions convert by conjugation with that rotation, implemented once in `frame_math::to_render()` (Rust) with a mirrored TypeScript test vector set, and unit-tested. Any engine with a different convention changes only this function.

**Euler angles** (display and authoring only; never integrated): yaw-pitch-roll (3-2-1) of BODY relative to local **NED** (north-east-down) at the aircraft's position. Heading ψ is the angle of the projected nose vector clockwise from true north. Flight state is **always** integrated as a unit quaternion.

### 0.3.3 Math types

| Type | Definition | Where |
|---|---|---|
| `DVec3`, `DQuat` (Rust `glam` f64 types; `Float64Array` in TypeScript) | 64-bit position/velocity/orientation | Sim Core state in BF |
| `Vec3`, `Quat` (f32) | 32-bit | Render, GPU buffers, particle systems, local (body-frame) maths, UI |
| `Fixed32` (Q16.16) | integer fixed point | Deterministic generation only (§26.3) |

Quaternions are renormalised every integration step; drift tolerance `| |q| − 1 | < 1e-12`.

**Precision analysis (why 64-bit).** A 32-bit float has a unit-in-the-last-place (ulp) of 0.0078 m at 100 km, 0.0156 m at 200 km, 0.031 m at 500 km from the origin. This is tolerable for rendering when camera-relative but **not** for integrating velocities at 120 Hz or for resolving hit tests at a 5 m closing distance 400 km from the anchor. A 64-bit double has ulp ≈ 5.8·10⁻¹¹ m at 500 km. JavaScript numbers and WebAssembly `f64` are already doubles, and **WebGPU shaders have no 64-bit float**. Therefore: **simulation state is `f64`; the renderer receives positions already subtracted from the camera position in `f64` (on the CPU) and then cast to `f32`** (the "relative-to-eye" technique, §44.8).

## 0.4 Architectural decision log

Each decision records the choice, the alternatives, and the reason. Changing one requires an update to every section listed under *Affects*.

| ID | Decision | Alternatives considered | Rationale | Affects |
|---|---|---|---|---|
| **ADR-01** | **Delivery platform: desktop web browser, installable as a PWA.** Primary renderer **WebGPU**; a reduced **WebGL2 "Compatibility" renderer** is a fallback for browsers/GPUs without WebGPU. Mobile/touch is out of scope. | Native Unity/Unreal build; Electron/Tauri wrapper (possible later as a thin shell around the same web build) | Required by the project owner: the game must run in a browser. Zero-install distribution, instant updates, trivially shareable seeds and links. | All |
| **ADR-02** | **Sim Core is written in Rust and compiled to WebAssembly**, with no browser or renderer dependency. The same crate builds natively for the dedicated server, mission-generator fuzzing, headless tests and CLI tools. It owns all gameplay state and advances on a fixed tick. | TypeScript-only sim (GC pauses, slower numeric loops); C++/Emscripten (viable, weaker safety); C#/.NET WASM (large runtime, startup cost) | No GC, predictable performance, first-class WASM SIMD and threads, strong determinism control, one code base for client-host and dedicated server. Risk: Rust talent — see Appendix F. | §11, §19, §33, §61 |
| **ADR-03** | Sim state is stored in **64-bit float BF**; rendering is camera-relative 32-bit float. | Floating-origin world shifting | Shifting breaks networking, replays, and long-range sensor maths. | §0.3, §44, §33 |
| **ADR-04** | **BF is a curved-Earth frame** (rigid ECEF transform), not a flat tangent plane. | Flat ENU plane with curvature "faked" in sensors; azimuthal projection | At 300 km a flat plane omits 7.06 km of Earth drop (500 km: 19.6 km). That error changes the horizon, radar line-of-sight, terrain masking, and low-level visibility — core gameplay (outline §28, §139). | §7, §16, §24, §44 |
| **ADR-05** | **Vertical datum: MSL height is treated directly as height above the WGS84 ellipsoid** ("MSL-as-ellipsoid"). No geoid undulation correction is applied by default. | Full EGM2008 geoid correction | DEMs, bathymetry and airport elevations are all MSL-referenced, so terrain, ocean surface, runways and aircraft altitude share one consistent surface. The geoid-ellipsoid separation (roughly −106 m … +85 m worldwide) would only matter when comparing against GNSS ellipsoidal heights, which the game never does. Residual inter-dataset datum differences (EGM96 vs EGM2008, typically on the order of a metre) are recorded per dataset and normalised in the pipeline (§45.6). | §7, §45 |
| **ADR-06** | **Terrain tiling: geodetic (lat/lon) quadtree**, level 0 = two 180° × 180° tiles; tiles carry 129 × 129 height posts (128 intervals) plus skirts. Deepest source-backed level is **z13** (≈19.1 m post spacing at the equator, consistent with 30 m source DEMs). Levels z14+ are procedural amplification only (visual). | Web Mercator quadtree (poles unusable); cube-sphere quadtree (uniform, but costlier tooling and re-projection) | Maps directly to lat/lon search and DEM sources; poles representable (to ±85° at launch, §45.8). Known inefficiency: E–W oversampling at high latitude (factor 1/cos φ), mitigated by LOD selection on the *larger* metric dimension. | §44, §45, §46 |
| **ADR-07** | **Fixed 60 Hz simulation tick** (Δt = 1/60 s) with multi-rate sub-systems (§0.6). | Variable timestep; fixed 30 Hz; fixed 120 Hz global | 60 Hz bounds latency, matches common refresh rates, and gives 16.7 ms of sim time per tick; 120 Hz only where needed (Tier-0 flight dynamics and terminal-phase weapons). | §11, §19, §33, §60 |
| **ADR-08** | **Networking: server-authoritative, with client-side prediction of the locally owned aircraft and snapshot interpolation of everything else. No lockstep.** Transports, in preference order: **WebRTC DataChannel** (unordered, `maxRetransmits: 0`) → **WebTransport datagrams** [verify support] → **WebSocket** (TCP; LAN-only fallback). The authority is either a browser tab hosting the sim in a Worker ("listen server") or the native dedicated-server build of the same Sim Core. Raw UDP is not available to web pages. | Peer-to-peer ownership; deterministic lockstep | AI-heavy battles make lockstep input delay unacceptable; authoritative hits/launches are required by the outline (§65). WebRTC DataChannels are the only browser-native unreliable transport usable between two pages with no server. | §33, §32, §31 |
| **ADR-09** | **Replays are snapshot-based** (periodic entity states + discrete events), not deterministic re-simulation. | Input-recording deterministic replay | Works across versions of non-gameplay code, allows free scrubbing/seeking, tolerant of tick-level changes. | §52 |
| **ADR-10** | **The Sim Core is determinism-capable; procedural generation is deterministic by requirement.** WebAssembly specifies IEEE-754 results for `+ − × ÷ √` (only NaN payloads may differ), so the Rust core is built with: no fast-math, no `relaxed-simd`, software `libm` for all transcendentals (`sin`, `cos`, `atan2`, `exp`, `pow`), integer RNG, sorted iteration. A CI job compares state hashes between the WASM build and the native x86-64 build after N ticks. Generators additionally use `Fixed32` where ordering matters (§26.3). | Global bit-exact guarantee | Seeds must reproduce "the same basic mission configuration" (outline §56) across machines and browsers. Runtime play does not depend on bit-exactness (server is authoritative), so a hash mismatch in play is a bug report, not a desync. | §26, §34, §51, §61 |
| **ADR-11** | **Aircraft, weapons, and factions are original/fictional designs** inspired by real performance classes. Geography is real. No real national flags, no real orders of battle, political borders hidden by default. | Licensed real aircraft; real nations | Avoids trademark/licensing exposure and political sensitivity of simulating real-world conflicts over real territory; matches outline §70 ("factions abstract and fictional"). | §12, §15, §37, §53 |
| **ADR-12** | **Open-data-first geodata, delivered as single-file tile archives fetched with HTTP range requests** (PMTiles-style directory + tile blobs) and cached locally. Default sources: Copernicus DEM / SRTM-class elevation, GEBCO bathymetry, ESA WorldCover, OpenStreetMap/Overture features, OurAirports, GeoNames, Natural Earth. **Google Earth / Google Maps data is not used.** | Streaming a commercial globe; shipping all data in the app | Offline operation (outline §123) and caching rights (outline §124) are incompatible with most commercial globe terms of service. The whole Earth cannot be bundled in a web app (§45.3). Every provider carries a machine-readable license descriptor (§45.9). | §9, §45 |
| **ADR-13** | **All content is versioned data** (JSON authoring → baked binary), never code. | Hard-coded content tables | Outline §115 and §147: adding content must not require engine changes; supports modding and user content. | §62 |
| **ADR-14** | **The campaign is a strategic layer that calls the same mission generator** (an *Operation* is a generator input set plus persistence rules). | Separate campaign mission pipeline | Outline §6: campaign "added without rebuilding the underlying flight and mission architecture". | §34–43 |
| **ADR-15** | **Terrain collision, radar masking and object placement use only the base DEM** (source-backed levels). Procedural detail displacement is bounded (≤ 1.5 m) and visual-only. | Collide against displayed mesh | Guarantees identical results on every client and server, which compute LOS and collision without rendering (the dedicated server has no GPU). | §44, §46, §16 |
| **ADR-16** | **Gameplay sensors and AI never read the render world.** They use sim-side queries (terrain height service, LOS service, spatial hash). | Raycasts against render geometry | Headless servers; performance; authority. | §16, §19, §61 |
| **ADR-17** | **Threading model: four roles.** *Main thread* (DOM UI, input, WebRTC, audio-graph control); *Sim Worker* (WASM Sim Core, owns authoritative state); *Render Worker* (WebGPU on `OffscreenCanvas`, main-thread fallback where unsupported [verify]); *Stream Workers* pool (fetch, decompress, decode, mesh build). State moves between them through `SharedArrayBuffer` ring buffers and seqlock-protected snapshot buffers. This **requires cross-origin isolation** (`Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp`), so every tile/asset origin MUST send `Cross-Origin-Resource-Policy`/CORS headers. A **degraded single-worker mode** (no SharedArrayBuffer, `postMessage` transfer of snapshots, reduced caps) exists for hosts that cannot set the headers. | Everything on the main thread; WASM threads for all work | Keeps UI responsive, isolates sim cost from render cost, and lets the 60 Hz sim run independent of `requestAnimationFrame` throttling. `RTCPeerConnection` is generally unavailable inside Workers [verify], hence networking sits on the main thread and forwards datagrams to the Sim Worker. | §33, §60, §61 |
| **ADR-18** | **Reference renderer stack: Three.js `WebGPURenderer` for scene graph, glTF/KTX2 asset loading and standard materials, with custom WebGPU passes for planetary terrain, atmosphere, volumetric clouds, FFT ocean, and GPU particles.** Babylon.js is the evaluated alternative; a fully custom thin renderer is the fallback if either library constrains the terrain system. *Recommendation — see OQ-01.* | Babylon.js; PlayCanvas; custom engine from scratch | The unusual rendering workloads are custom regardless of library; the library is chosen for asset pipeline, tooling and ecosystem. | §44, §24, §54, §60 |
| **ADR-19** | **Storage and offline: Service Worker + Cache Storage for the app shell, Origin Private File System (OPFS) for tile archives/replays/large blobs, IndexedDB for structured saves and settings; persistent storage is requested.** Everything needed to fly a previously cached region works with no network. | Server-side saves; localStorage | Offline play is a non-negotiable (NN-8). Browser storage can be evicted — see §64.6 (export/import and persistence request). | §45, §64 |
| **ADR-20** | **UI split: DOM/CSS for menus, world map chrome, lobbies and settings; GPU/canvas rendering for HUD, MFDs, and cockpit displays** (render-to-texture so the same HUD renders in 3D cockpit and overlay modes). | All-canvas UI; all-DOM HUD | DOM gives accessibility, text layout, IME and localization for free; the HUD must live in the 3D scene and run at render rate. | §53, §54, §57 |

## 0.5 Assumptions and open questions

Assumptions are the premises this document makes. Open questions (OQ) must be resolved by the owner before the milestone shown.

| ID | Item | Needed by |
|---|---|---|
| **OQ-01** | Confirm the stack in ADR-02 (Rust → WASM sim) and ADR-18 (Three.js WebGPURenderer + custom passes). If the team prefers TypeScript-only, the sim moves to a Worker using typed arrays; the budgets in §0.6 drop roughly 30–40 % and ADR-10's cross-build hash test is lost. | Before M1. **Closed 2026-10-14 (v0.1a):** the Sim Core stays Rust → WASM (ADR-02 stands); the renderer half is revisited at v0.1c (`docs/open-questions.md`) |
| **OQ-02** | Team size and target release scale (solo / small team / studio). This document sizes work with relative t-shirt sizes (§66), not calendar estimates. | Before M1 planning |
| **OQ-03** | Hosting and distribution: static host + CDN with range-request support and the COOP/COEP headers (assumed), who pays for tile bandwidth, and whether a publisher-hosted CDN or a community-mirror model is used. | Before M2 |
| **OQ-04** | Commercial or free release. Several open datasets have attribution (CC-BY) or share-alike (ODbL) duties that interact with redistribution (§45.9). | Before M2 |
| **OQ-05** | Aircraft/weapon art budget: modular kit-bashed aircraft vs fully bespoke models (affects roster size at M3). Browser asset budget (§60.6) also limits fidelity. | Before M3 |
| **OQ-06** | In-game voice chat. The browser provides WebRTC audio, so this is cheap (assumed on, optional per lobby, §32.9). | Before M5 |
| **OQ-07** | WebXR/VR support (not in outline; designed-for-compatibility only, §58.8). | Before M3 |
| **OQ-08** | Target content rating and regional constraints on map/border depiction (§9.8). | Before public release |
| **OQ-09** | Whether a desktop wrapper (Electron/Tauri) is wanted later for storefront distribution. The architecture keeps this possible and does not depend on it. | Before M5 |
| **OQ-10** | Community hub scope and moderation for user missions (§51.4): hosted service vs files/links only; moderation policy; abuse handling. | Before M7 |

Assumptions: desktop browsers only (Chromium-based browsers are the primary test target; Firefox and Safari supported on a best-effort/feature-detected basis [verify]); keyboard/mouse, gamepad, and joystick/HOTAS must all reach full functionality; no cheat protection beyond host authority (LAN/VLAN trust model); online matchmaking and NAT traversal services are out of scope; the user can reach the game URL at least once (to install/cache) before going offline.

## 0.6 Global constants, tiers, and budgets (normative)

### 0.6.1 Simulation tick and rate tiers

The simulation advances on a fixed tick `SIM_DT = 1/60 s` inside the Sim Worker. The loop is a drift-correcting accumulator driven by `performance.now()`; at most **5 catch-up ticks per loop iteration** are run (after that the sim runs slow rather than spiralling), and the loop yields with `MessageChannel`/`scheduler.postTask` rather than relying on `setTimeout`'s clamped granularity. Individual systems run at divisors of the tick.

| System | Rate | Notes |
|---|---|---|
| Sim tick (master) | 60 Hz | Network tick, input sampling, event dispatch |
| Flight dynamics, Tier 0 | 120 Hz (2 sub-steps/tick) | RK4 |
| Flight dynamics, Tier 1 | 60 Hz | Semi-implicit (symplectic) Euler |
| Flight dynamics, Tier 2 | 30 Hz | Reduced-order model (§11.17) |
| Flight dynamics, Tier 3 | 10 Hz | Point-mass energy model |
| Strategic (Tier 4) | 1 Hz or event-driven | Node-level abstraction, off-battlefield |
| Missiles | 120 Hz while within terminal range of target or in launch phase; else 60 Hz | |
| Gun projectiles | 60 Hz tick with analytic sub-stepping and swept-segment collision | |
| Radar / IRST / visual detection | 10 Hz (staggered by `entityId mod 6`) | STT track update 20 Hz |
| RWR evaluation | 10 Hz; missile-warning 30 Hz | |
| AI tactical decision | 5 Hz | |
| AI maneuver controller | 30 Hz | |
| Squadron AI | 1 Hz | |
| Mission AI | 0.5 Hz | |
| Network client input send | 60 Hz | Redundant last 3 inputs per packet |
| Network snapshot send | 20 Hz default (10–30 Hz configurable) | |
| Audio listener / spatialisation | 60 Hz | |
| UI HUD refresh | Render rate | HUD reads interpolated view buffers |

**Tier assignment** is computed on the authority each 1 Hz "relevance pass" using the *minimum* distance to any human-controlled aircraft:

| Tier | Assignment rule (first match wins) |
|---|---|
| **T0** | Human-piloted aircraft; any AI within 8 km of a human; any aircraft firing guns on or being tracked by gun fire from a T0 aircraft |
| **T1** | AI within 40 km of a human; any AI within 30 km of a hostile T0/T1; any aircraft currently supporting a missile (data-link/illumination) |
| **T2** | AI within 150 km of a human or currently detected by any human's sensors |
| **T3** | All other AI inside the battlefield |
| **T4** | Off-battlefield / campaign-only forces |

Promotion is immediate; demotion requires 5 s of continuous eligibility (hysteresis). State conversion on tier change MUST be continuous (position, velocity and heading are preserved; attitude is reconstructed from velocity and estimated bank when promoting from T3).

### 0.6.2 Battle scale and entity budgets

| Preset | Aircraft (all sides) | Notes |
|---|---|---|
| Small | 2–8 | Dogfights, 1v1 |
| Medium | 10–30 | |
| Large | 30–75 | |
| Huge | 75–150+ | Hardware-dependent |
| Chaos | up to 200 | Entertainment-first; enforces tier degradation (§30.4) |

Hard caps by hardware profile (the game picks a profile at first run from a 30-second benchmark, §60.8, and the user can override). Caps are lower than a native build would allow because of browser memory limits and WASM/JS overheads.

| Entity class | Minimum | Recommended | High |
|---|---|---|---|
| Aircraft in T0–T2 (full/reduced dynamics) | 30 | 70 | 120 |
| Aircraft total (incl. T3) | 60 | 120 | 200 |
| Missiles in flight | 80 | 160 | 250 |
| Gun projectiles in flight | 1,200 | 2,500 | 5,000 |
| Active ground units (simulated) | 120 | 300 | 600 |
| Ships | 15 | 50 | 80 |
| Civilian entities | 15 | 60 | 150 |

If a cap would be exceeded, the mission director defers or merges spawns (§28.5) rather than failing. Without cross-origin isolation (degraded single-worker mode, ADR-17) all caps are halved.

### 0.6.3 Browser and hardware profiles

| Profile | CPU | RAM | GPU | Browser | Local storage |
|---|---|---|---|---|---|
| Minimum | 4 cores | 8 GB | Integrated GPU with WebGPU (Intel Iris Xe / Radeon 680M class) or WebGL2 fallback | Current stable Chrome/Edge (primary); Firefox, Safari best-effort [verify] | ≥ 2 GB free for cache |
| Recommended | 6 cores | 16 GB | 6 GB VRAM, GTX 1660 / RTX 2060 class | Current stable Chrome/Edge | ≥ 10 GB free |
| High | 8+ cores | 32 GB | 8–12 GB VRAM, RTX 3070 class or better | Current stable Chrome/Edge | ≥ 40 GB free (offline regions) |

Network: a first visit downloads the app shell and base globe pack (§0.6.6). Cached play needs no network.

Frame budget at the Recommended profile, 1080p, High preset, Large battle (≈ 75 aircraft): **60 FPS** (16.67 ms/frame), 1 % lows ≥ 45 FPS. Minimum profile: **30 FPS** stable at 720p–900p Low, Medium battle. These are *targets* enforced by the performance tests in §60.9.

| Budget (Recommended, Large battle) | Target (avg) | p99 |
|---|---|---|
| Sim tick, wall time in the Sim Worker | ≤ 4.0 ms | ≤ 7.0 ms |
| Main thread (UI, input, WebRTC, audio control) | ≤ 4.0 ms | ≤ 8.0 ms |
| Render Worker CPU (command encoding, culling) | ≤ 7.0 ms | ≤ 11.0 ms |
| GPU | ≤ 14.0 ms | ≤ 16.0 ms |
| Terrain streaming, render-thread cost | ≤ 0.7 ms | ≤ 2.0 ms |
| JS allocations per frame in sim/render hot paths | 0 | < 4 KB |
| Main-thread GC pause | ≤ 2 ms | ≤ 6 ms |
| WASM linear memory (Sim Core) | ≤ 1.5 GB | |
| JS heap | ≤ 600 MB | |
| GPU resources (Recommended / Minimum) | ≤ 4.5 GB / ≤ 2.5 GB | |
| Total tab memory at Large battle | ≤ 6 GB | |

WebAssembly 32-bit memory is addressable up to 4 GiB; the design deliberately stays well inside 2 GiB of linear memory so no profile depends on 64-bit WASM memory [verify].

### 0.6.4 Battlefield size presets

Edge length of the square playable area (circular or irregular shapes use the same area class). The *visible horizon* area is separate and larger (§8.3).

| Preset | Edge (km) | Area (km²) | Edge crossing at 250 m/s |
|---|---|---|---|
| Small | 50 | 2,500 | 3.3 min |
| Medium | 120 | 14,400 | 8.0 min |
| Large | 250 | 62,500 | 16.7 min |
| Huge | 500 | 250,000 | 33.3 min |
| Extreme | 1,000 | 1,000,000 | 66.7 min |

At the Extreme preset the Earth's surface drops 19.6 km below the tangent plane 500 km from the anchor, which is why BF is curved (ADR-04).

### 0.6.5 Physical constants

| Constant | Value |
|---|---|
| WGS84 semi-major axis a | 6 378 137.0 m |
| WGS84 flattening f | 1 / 298.257223563 |
| WGS84 semi-minor axis b | 6 356 752.314245 m |
| First eccentricity² e² | 0.00669437999014 |
| Mean Earth radius R (for sphere approximations) | 6 371 000 m |
| Standard gravity g₀ | 9.80665 m/s² |
| Specific gas constant of dry air R_air | 287.05287 J/(kg·K) |
| Ratio of specific heats γ | 1.4 |
| Sea-level ISA: T₀, p₀, ρ₀ | 288.15 K, 101 325 Pa, 1.2250 kg/m³ |
| Speed of sound, ISA sea level | 340.29 m/s |

### 0.6.6 Download and startup budgets (browser-specific)

| Budget | Target |
|---|---|
| App shell (JS + WASM + UI + core shaders), Brotli-compressed | ≤ 40 MB |
| Base globe pack (terrain through z5, coastlines, airport index, place-name search index, low-res albedo) | ≤ 120 MB compressed, streamed progressively; the menu is usable before it finishes |
| Time to globe visible, uncached, 100 Mbit/s | ≤ 8 s |
| Time to globe visible, cached | ≤ 3 s |
| WASM instantiation | `WebAssembly.instantiateStreaming`, compiled module cached in IndexedDB where supported |
| Per-battlefield pre-fetch before launch (Large, §44.9) | ≈ 120 MB raw / ≈ 40–50 MB compressed (estimate) |
| Aircraft model + textures (each, compressed GPU textures) | ≤ 12 MB; lazy-loaded when selected |

## 0.7 Outline coverage map

Every one of the outline's 153 numbered items is covered. The outline's table of contents (71 sections) is the structure of this document.

| Outline items | GDD section(s) |
|---|---|
| 1–6 | §1 Game Overview, §2 Vision, §3 Pillars, §4 Player Experience, §5 Core Loop, §6 Modes |
| 7, 81–83 | §7 World & Geographic System, §44, §45, §46 |
| 8, 14, 91 | §8 Battlefield System |
| 9, 92–94, 99 | §9 World Map |
| 10–13, 140–141 | §10 Airport System, §48 Airbase Operations |
| 15–17, 46 | §11 Flight Model |
| 18 | §12 Aircraft System |
| 19–20 | §13 Aircraft Categories |
| 21–22 | §14 Aircraft Customization |
| 23–24 | §15 Weapons System |
| 26–28, 95 | §16 Radar & Avionics |
| 25, 143–144 | §17 Electronic Warfare |
| 29–30 | §18 Damage Model |
| 31–32, 36–37 | §19 AI System |
| 33–35 | §20 Squadron System |
| 38–39 | §21 Ground Warfare |
| 40–42 | §22 Naval Warfare |
| 43, 103 | §23 Civilian World |
| 44–47 | §24 Weather & Environment |
| 48, 52–54 | §25 Mission System |
| 55–56 | §26 Mission Generator |
| 49 | §27 Mission Types |
| 50–51 | §28 Dynamic Mission Escalation |
| 57 | §29 Boss & Ace System |
| 58–59 | §30 Sandbox Configuration |
| 60–61 | §31 1v1 Mode |
| 62–64 | §32 LAN/VLAN Co-op |
| 65, 112 | §33 Multiplayer Architecture |
| 66, 71–72 | §34 Roguelite Conquest Campaign |
| 67, 69, 142 | §35 Strategic Map |
| 68 | §36 Airport Capture |
| 70 | §37 Factions |
| 72, 77 | §38 Campaign Progression |
| 73–74 | §39 Campaign Resources |
| 76–77 | §40 Campaign Events |
| 75 | §41 Pilot Progression |
| 78–79 | §42 Fog of War & Intelligence |
| 80 | §43 Emergent Storytelling |
| 81 | §44 World Streaming |
| 82, 123–124 | §45 Geographic Data |
| 83–84, 139 | §46 Terrain & Biomes |
| 85 | §47 Cities & Infrastructure |
| 86 | §49 Long-Range Operations |
| 87–89 | §50 Navigation |
| 90–91 | §51 User-Generated Content |
| 96, 104–105 | §52 Replay & Photo Mode, §54 (camera) |
| 99, 107 | §53 UI / UX |
| 95, 97 | §54 HUD |
| 98 | §55 Tactical Map |
| 100–102 | §56 Audio |
| 108 | §57 Accessibility |
| 106–107 | §58 Controls |
| 109 | §59 Difficulty & Realism |
| 110–111 | §60 Performance |
| 116–117, 122, 125 | §61 Technical Architecture |
| 115 | §62 Data-Driven Content |
| 118–121 | §63 Debugging Tools |
| 113 | §64 Save System |
| 114 | §65 Statistics |
| 125–131, 145 | §66 Development Roadmap |
| 132–135 | §67 Prototype Requirements |
| 146–147 | §68 Design Priorities |
| 136–138 | §69 Anti-Repetition Systems (137 also §52.10) |
| 148–150 | §70 Final Player Experience |
| 151–153 | §71 Definition of Success |

---

# 1. GAME OVERVIEW

## 1.1 Summary

The game is a **sandbox combat flight simulator built on the real geography of Earth that runs in a web browser**. Instead of choosing from a handful of authored maps, the player opens a globe, picks a place, and the game generates a playable war around it: terrain from real elevation data, airports from real airport data, forces from a procedural mission generator, and objectives that evolve as the battle unfolds.

The same simulation underlies every mode: the **Sandbox Skirmish**, **1v1**, **LAN/VLAN co-op**, and the later **Roguelite Conquest Campaign**. A campaign operation is a sandbox mission with persistence attached (ADR-14).

## 1.2 One-sentence pitch

> **A realistic sandbox combat flight simulator where the entire real world becomes your battlefield — one click away in your browser.**

## 1.3 Product pillars at a glance

| Pillar | One-line promise | Primary sections |
|---|---|---|
| Geographic freedom | Fight anywhere on Earth | §7–9, §44–47 |
| Believable flight | Energy, stalls, G-load, density altitude all matter | §11–12 |
| Dynamic combat | Enemies arrive, coordinate, retreat, reinforce | §19–20, §28 |
| Procedural replayability | Same place, different war | §26–28, §69 |
| Strategic persistence | Airports are strategic nodes | §34–43 |
| Player freedom | Every battle parameter configurable | §30, §51 |
| Multiplayer | Co-op and duels on one architecture | §31–33 |
| Emergent stories | Narrative from systems, not scripts | §43 |
| Zero-friction access | Open a link, fly; shareable seeds and links | §1.6, §51, §61 |

## 1.4 Platform, scope, and deliverables

- **Platform:** desktop web browser, installable as a PWA (ADR-01). Input: keyboard + mouse, gamepad, joystick/HOTAS, optional head tracking (webcam-based, §58.7).
- **Scope of the first shippable product:** Sandbox Skirmish, 1v1, LAN/VLAN co-op, one base geodata pack plus streamed region packs, ~12 launch aircraft (§13.4), 26 weapons plus 6 non-weapon store types (§15.12), 4 fictional factions (§37). The campaign ships as a later content release but its data model is reserved from M1 (§66.2).
- **Not in scope:** mobile/touch, online matchmaking, NAT-traversal services, ranked ladders, console ports, real-world nation/political simulation, real aircraft licences.

## 1.5 Genre positioning

| Reference | What we take | What we avoid |
|---|---|---|
| Project Wingman | Cinematic mission pacing, escalating set-pieces, boss pilots with identity | Fixed handcrafted missions as the only content |
| War Thunder | Weapon/aircraft interaction depth, modular damage, per-aircraft identity | Grind economy, arcade-vs-sim mode fragmentation |
| Ace Combat | Spectacle, music/radio presence, large mixed battles | Impossible physics, bullet-sponge bosses |
| Geographic simulators | Real-world place as the content | Requiring full-detail streaming of the entire Earth |
| Roguelite strategy | Run-based consequences, meta-progression | Spreadsheet management |

## 1.6 Why a browser (design consequences)

| Browser property | Design consequence |
|---|---|
| Open a URL, no install | Shareable mission links: a seed + version + data-pack id fits in a URL fragment (§26.5, §51.4) |
| Sandboxed, no raw sockets | Multiplayer uses WebRTC/WebTransport/WebSocket (§33.3) |
| Limited memory and no direct disk access | Streamed, cached geodata in OPFS (§45); lower entity caps (§0.6.2) |
| Tabs can be backgrounded, throttled, or closed | Pause/auto-save policy (§5.6); hosting multiplayer requires a visible tab or a dedicated server (§33.9) |
| Storage can be evicted | Persistence request plus export/import of saves (§64.6) |
| Strong accessibility and localization primitives in the DOM | DOM-based menus (ADR-20, §53, §57) |

---

# 2. VISION STATEMENT

## 2.1 Central fantasy

> **Open a globe representing the real Earth, choose a place, create a war, choose an aircraft, and fly.**

The player looks at a map and thinks, *"I want to fight there,"* and the game makes that possible. The world itself is the mission generator (outline §2).

## 2.2 Experience statements

These are the statements every design decision is tested against. A feature that does not support at least one is a candidate for cutting.

1. **Place matters.** The player's choice of location changes the fight (mountains mask radar, deserts expose, oceans demand range).
2. **Skill is rewarded by physics, not by stats.** A well-flown inferior aircraft can beat a poorly-flown superior one because energy, geometry and sensors are real.
3. **The sky is busy but legible.** Many entities act, yet the player can always tell what threatens them.
4. **No two wars are the same.** Anti-repetition (§69) is a hard requirement.
5. **The game respects the player's time.** Long transits can be accelerated; setup is fast; no menu gauntlets between wanting to fly and flying.
6. **Click to fly.** From link to cockpit in seconds on a cached install (§4.4).

## 2.3 Tone

Serious but accessible. Military aviation aesthetic without real-world political content: fictional factions (ADR-11), neutral radio language, no depiction of real nations as combatants.

---

# 3. DESIGN PILLARS

Each pillar has a **statement**, **mechanisms** (how the game delivers it), **anti-patterns** (what violates it), and **tests** (how we know).

## 3.1 Geographic Freedom

**Statement.** The player has extraordinary freedom over where combat occurs.

**Mechanisms.** Globe world map with search and coordinate entry (§9); any two airports or any arbitrary point (§10, §8); battlefield sizes Small–Extreme (§0.6.4); streaming terrain on a WGS84 quadtree (§44); procedural fill where data is missing (§46).

**Anti-patterns.** A maintained list of "supported maps"; features that only work in curated locations; location-specific hard-coding.

**Tests.** `AC-3.1`: A scripted test selects 1,000 random land/ocean points (stratified by biome and latitude within ±85°) and generates a battlefield at each without error; ≥ 99 % reach "playable" state without manual intervention, and 100 % either play or fail with a recoverable, user-readable message (§61.9).

## 3.2 Believable Flight

**Statement.** Aircraft behave like aircraft, accessible but not arcade.

**Mechanisms.** 6-DOF rigid-body model with table-driven aerodynamics, ISA atmosphere, mass/CG/inertia changes with fuel and stores (§11); selectable assistance levels from Full Assist to Manual (§58.5, §59).

**Anti-patterns.** Speed as a free variable; turn rate independent of energy; zero-inertia pointing.

**Tests.** `AC-3.2`: the Performance Conformance Test (§11.19) reproduces each aircraft's authored point-performance envelope within the tolerances stated there (±3 % for speeds, climb, stall and fuel flow; ±5 % for sustained turn performance and ground rolls).

## 3.3 Dynamic Combat

**Statement.** Combat develops organically; the enemy reacts.

**Mechanisms.** Layered AI (strategic → mission → squadron → pilot → flight control), perception through sensors only, morale/retreat, reinforcement director (§19, §20, §28).

**Anti-patterns.** AI that knows the player's position perfectly; AI that flies physically impossible trajectories.

**Tests.** `AC-3.3`: Telemetry audit confirms AI aircraft in all tiers T0–T2 never exceed the authored envelope (speed, G, roll rate) by more than 1 % (numeric tolerance only); AI target acquisition events always originate from a sensor model event (§19.9).

## 3.4 Procedural Replayability

**Statement.** The same region produces different missions.

**Mechanisms.** Seeded generator with composable phases, objectives, force packages, weather, events; recency penalties (§26, §69).

**Tests.** `AC-3.4`: For 200 consecutive generations at a fixed location with random seeds, the pairwise "mission DNA" distance (§69.2) has a mean ≥ 0.45 and no more than 5 % of pairs with distance < 0.15.

## 3.5 Strategic Persistence

**Statement.** Capturing an airport changes what happens at its neighbours.

**Mechanisms.** Airport graph, supply and influence propagation, counterattack AI, persistent damage (§35–40).

## 3.6 Player Freedom

**Statement.** Almost every aspect of a battle is configurable, and the same objective supports many solutions.

**Mechanisms.** Sandbox configuration (§30), multiple solution paths per objective (§69.5), user-generated missions (§51).

## 3.7 Multiplayer

**Statement.** Cooperative and competitive multiplayer share the single-player simulation architecture.

**Mechanisms.** Authoritative sim hosted by a player's browser tab or a dedicated native build (ADR-02, ADR-08); single-player is a host with zero remote clients and an in-process loopback transport (§33.2).

## 3.8 Emergent Storytelling

**Statement.** Memorable stories arise from system interactions.

**Mechanisms.** Persistent enemy identities (aces, squadrons), a chronicle that records notable events (§43), campaign continuity (§38).

## 3.9 Non-negotiable rules (from outline §147)

The game MUST NOT:

| # | Rule | Enforcement |
|---|---|---|
| NN-1 | Rely entirely on static maps | §3.1 test; no authored-only content path |
| NN-2 | Use completely arcade flight physics | §11 model; no "turn at fixed rate" fallback in any assist mode |
| NN-3 | Make missiles guaranteed hits | §15.6 miss model, §15.8 countermeasure interaction |
| NN-4 | Make bosses bullet sponges | §29: no HP multipliers; capability-based bosses only |
| NN-5 | Make AI strong through impossible physics | §19.2 "same physics, same sensors" rule |
| NN-6 | Make every mission identical | §69 |
| NN-7 | Make the campaign a linear mission list | §35, §40 |
| NN-8 | Require internet for LAN | §33.8; after first cache, no network dependency at runtime |
| NN-9 | Bury players in simulator complexity | §59, §57; Standard Assist is the default |
| NN-10 | Sacrifice flight feel for menus | §53 menu budgets |
| NN-11 | Hard-code content that should be procedural | §62; content lint in CI |
| NN-12 | Sacrifice future expandability for short-term convenience | ADRs; extension points in §61 |

A CI check (`NoHardcodedContentLint`, §62.8) fails the build if gameplay constants for aircraft/weapons/units appear in code outside the data-loading layer.

---

# 4. TARGET PLAYER EXPERIENCE

## 4.1 Positioning

The intended experience sits between arcade combat flight games and hardcore simulators. A new player can fly and fight within **two minutes** of first launch (benchmarked by §4.5); a veteran can disable nearly all assistance and fly a demanding model.

## 4.2 Skill ladder

Mastery is visible in what the player *does*, not what they unlock.

| Tier | Player understands | Game supports it by |
|---|---|---|
| 1 – Flier | Pitch/roll/throttle, using the HUD, firing missiles | Full Assist, lock cues, DLZ in HUD |
| 2 – Fighter | Energy (speed vs altitude), turning radius, gun solutions | Energy-state HUD cues (optional), kill feedback |
| 3 – Tactician | Radar modes, RWR, notching, terrain masking, fuel | Radar and RWR fidelity (§16), tactical map |
| 4 – Leader | Formation and role play, squadron orders, EW | Orders UI (§20.6), EW role (§17) |
| 5 – Ace | BVR geometry, missile defeat timing, deception | Realistic missile model, simulator difficulty |

## 4.3 Assistance as dials, not modes

Realism is a set of independent dials (§59.2), grouped into five presets. The player can move any dial. The *default* preset is "Normal".

## 4.4 Pacing targets

| Moment | Target |
|---|---|
| Link opened → globe visible (cached) | ≤ 3 s; uncached ≤ 8 s at 100 Mbit/s (§0.6.6) |
| Select place → battlefield preview | ≤ 6 s (data cached), ≤ 12 s (streamed) |
| Preview → aircraft on runway/in air | ≤ 25 s for Medium battlefield |
| First enemy contact after launch (default Quick Dogfight) | 60–120 s |
| Death → restart/respawn decision | ≤ 5 s |
| Mission end → world map | ≤ 4 s |

## 4.5 First-time experience

A guided "first sortie" (not a separate tutorial level): the globe auto-selects a nearby training location with Full Assist; short contextual prompts (never modal) teach throttle, climb, targeting, firing, landing; a "skip all hints" toggle is always visible. Browsers require a user gesture before audio plays and before pointer lock or fullscreen; the first sortie's "Click to start" screen provides that gesture (§56.2, §58.3). Success criterion `AC-4.5`: ≥ 80 % of external playtesters complete take-off and one kill within 10 minutes without reading text outside the prompts.

---

# 5. CORE GAMEPLAY LOOP

## 5.1 Sandbox loop

```text
WORLD MAP
    ↓
SELECT LOCATION  (airports / region / arbitrary point)
    ↓
CONFIGURE BATTLE  (size, forces, weather, time, rules)
    ↓
SELECT AIRCRAFT → SELECT LOADOUT
    ↓
LAUNCH  (runway, carrier, or air spawn)
    ↓
FLY → DETECT → ENGAGE → COMPLETE OBJECTIVES → RESPOND TO EVENTS
    ↓
DEFEAT ENEMY / BOSS
    ↓
RETURN TO BASE OR EXTRACT
    ↓
RESULTS (kills, losses, replay save, seed share)
    ↓
WORLD MAP
```

## 5.2 Campaign loop

```text
STRATEGIC MAP → SELECT TARGET AIRPORT → GENERATE OPERATION
    ↓
FLY MISSION → CAPTURE / DEFEND / DESTROY
    ↓
UPDATE FRONTLINE → REPAIR / REARM / RESUPPLY
    ↓
CHOOSE NEXT OPERATION → REPEAT
```

## 5.3 Loop timing model

Three nested loops with target durations:

| Loop | Target duration | Player decisions |
|---|---|---|
| Moment-to-moment (combat) | 5–30 s | Maneuver, shoot, defend |
| Sortie | 12–40 min (default Large), 3–8 min (Quick Dogfight) | Route, target priority, fuel |
| Session / campaign turn | 45–120 min | Where to attack, what to risk |

## 5.4 Failure and recovery

Failure is never a dead-end screen. Every failure state offers: restart with same seed, reconfigure and retry, view replay, return to map. In the campaign, failure produces consequences (§38) but the run continues unless the strategic loss condition is met.

## 5.5 State machine of the application

```text
BOOT → MAIN MENU → { WORLD MAP | CAMPAIGN MAP | LOBBY | SETTINGS | REPLAYS | LIBRARY }
WORLD MAP → BATTLE SETUP → LOADOUT → LOADING → IN-MISSION → DEBRIEF → WORLD MAP
IN-MISSION ⇄ PAUSE / PHOTO / REPLAY-VIEWER
LOBBY → (host) BATTLE SETUP … | (client) LOADOUT → LOADING → IN-MISSION
```

The application state machine is owned by a `GameFlow` service on the main thread; the Sim Core has only `Idle / Loading / Running / Paused / Ended` states (§61.4).

## 5.6 Tab lifecycle policy (browser-specific)

Browsers throttle or freeze background tabs, so the game defines explicit behaviour:

| Event | Single-player | Multiplayer client | Multiplayer host (in-browser) |
|---|---|---|---|
| Tab hidden (`visibilitychange`) | Auto-pause sim; audio ducked to 0; resume prompt on return | Client keeps receiving; render suspended; if hidden > 10 s the aircraft goes on autopilot "hold" (§49.4) and the player is told on return | **Warn before it happens** (host banner "keep this tab visible"); sim keeps running in its Worker but may be throttled by the browser [verify]; if the tick falls below 50 % real time for 3 s, all clients see a "host throttled" notice |
| Tab closing / navigation (`pagehide`) | Write emergency checkpoint to IndexedDB (§64.4); `beforeunload` prompt only in a live mission | Graceful disconnect message | Host migration is **not** supported; session ends with results saved to each client |
| Window resize / DPI change | Recompute render targets within one frame | Same | Same |
| Network offline event | No effect after caching | Connection-lost UI (§33.7) | Same |

A **Screen Wake Lock** is requested during missions to stop display sleep [verify].

---

# 6. GAME MODES

## 6.1 Mode matrix

| Mode | Humans | AI | Persistence | Milestone |
|---|---|---|---|---|
| Sandbox Skirmish | 1 | any | none (stats only) | M2–M4 |
| 1v1 | 2 | optional spectators/none | none | M5 |
| LAN/VLAN Co-op | 2–16 (design target; hard cap 16; browser-hosted sessions recommended ≤ 8, §33.9) | any | none (mission stats) | M5 |
| Roguelite Conquest Campaign | 1 (later co-op) | any | full | M6 |
| Free Flight / Training | 1 | optional | none | M1 |
| Replay Viewer | n/a | n/a | replay files | M5+ |

## 6.2 Shared architecture

All modes use:

- the **same Sim Core**, started with a `SessionConfig`;
- the **same Mission Generator** (modes differ in generator inputs and rule sets);
- the **same netcode** (single-player is a loopback host).

Mode-specific logic is expressed as **Rule Sets** (data): victory conditions, spawn rules, respawn policy, scoring, allowed equipment (§25.9, §31.4).

## 6.3 `SessionConfig`

```json
{
  "schema": "session_config/1",
  "mode": "sandbox",
  "seed": "FA3K-9T2M-QX71",
  "battlefield": { "shape": "rect", "center": { "lat": 36.2, "lon": -121.8 }, "size_km": [250, 250], "rotation_deg": 0 },
  "environment": { "date": "2026-06-21", "time_preset": "sunset", "weather": "cloudy", "wind": { "dir_deg": 270, "speed_mps": 8 } },
  "rules": "ruleset/air_superiority",
  "players": [{ "slot": 0, "aircraft": "kestrel", "loadout": "kestrel/aa_heavy", "team": "blue" }],
  "forces": { "enemy_strength": "heavy", "ally_strength": "moderate", "ai_skill": "veteran", "ground": true, "naval": false, "civilian": "low" },
  "difficulty": "normal",
  "dials": { }
}
```

## 6.4 Mode-specific rules summary

| Aspect | Sandbox | 1v1 | Co-op | Campaign |
|---|---|---|---|---|
| Generator | full | none (arena templates) | full | full + strategic inputs |
| Respawn | configurable | by rule set | configurable | consequences (loss of airframe) |
| Time acceleration | yes | no | host vote | yes |
| Pause | yes | no | host-only, only if all consent | yes |
| Save mid-mission | quick-save allowed | no | no | campaign checkpoint on landing |
| Weapons restrictions | configurable | rule set | host | faction/inventory |

## 6.5 Architectural obligation

Campaign-only concepts (inventory, repair, airframe hours, pilot XP) MUST be reachable through the generic interfaces `IForceProvider`, `IAirframeSource`, `IPersistenceSink` (§34.6) so sandbox runs with these interfaces stubbed. This is verified by `AC-6.5`: the campaign build can be compiled out without changing Sim Core code.

---

# 7. WORLD & GEOGRAPHIC SYSTEM

## 7.1 Intent and detail levels

The entire Earth is the conceptual world. The game never loads all of it at high detail (§44); it works at several **levels of geographic detail (LoG)** that map onto the terrain quadtree of ADR-06:

| LoG | Used for | Quadtree levels | Post spacing at equator (128-interval tile) | Content |
|---|---|---|---|---|
| Global | Globe view, orbit camera | z0–z5 | 156.5 km … 4.89 km | Globe, continents, oceans, major features, city lights |
| Continental | Zoomed-out map, strategic overview | z4–z7 | 9.78 km … 1.22 km | Low-detail terrain, coastlines, major roads/rail as vector lines |
| Regional | Map zoom, battlefield horizon ring | z6–z10 | 2.45 km … 153 m | Terrain, cities (footprints as masses), roads, airports, coastlines |
| Battlefield | Playable area | z9–z12 | 306 m … 38.2 m | Detailed terrain, vegetation classes, structures, roads, airfields, installations |
| Close | Near-ground flight, take-off/landing, ground attack | z11–z13 + procedural detail | 76 m … 19.1 m, then ≤ 1.5 m visual detail | Maximum terrain detail, buildings, trees, aircraft-scale objects |

Levels overlap deliberately; the active level for any tile is chosen by screen-space error (§44.4), not by a fixed band.

The geographic system provides, at all times, a single authoritative answer to:

- *Where is this latitude/longitude in the battlefield frame?* (§7.4)
- *How high is the ground here?* (§44.6 `TerrainService`)
- *What is the biome, land cover and surface type here?* (§46)
- *Is there line of sight between these two points?* (§16.9 `LosService`)
- *What is the nearest airport/road/city?* (§45.7)

## 7.2 Geodesy primitives

### 7.2.1 Geodetic → ECEF

For geodetic latitude φ, longitude λ, height h (MSL-as-ellipsoid, ADR-05):

```text
N(φ)  = a / sqrt(1 − e² sin²φ)                       prime-vertical radius of curvature
X = (N + h) cosφ cosλ
Y = (N + h) cosφ sinλ
Z = (N (1 − e²) + h) sinφ
```

**Test vector (MUST pass in the unit tests, tolerance 1 mm):**
φ = 36.2°, λ = −121.8°, h = 1500 m → ECEF = (−2 716 000.612 m, −4 380 460.552 m, 3 747 008.625 m).

### 7.2.2 ECEF → Geodetic

Iterative (fixed-point) method, robust everywhere except exactly at Earth's centre:

```text
λ  = atan2(Y, X)
p  = sqrt(X² + Y²)
φ₀ = atan2(Z, p (1 − e²))                      initial guess
repeat:
    N = a / sqrt(1 − e² sin²φ)
    h = p / cosφ − N                           (use Z / sinφ − N(1 − e²) when |φ| > 80° to avoid cosφ → 0)
    φ = atan2(Z, p (1 − e² N / (N + h)))
until |Δφ| < 1e-12 rad  (typically 3–4 iterations for |h| < 100 km)
```

Round-trip error MUST be < 1 µm in height and < 1e-12° in angle for |h| ≤ 100 km. A closed-form alternative (Heikkinen / Vermeille) MAY replace the loop on the hot path if profiling requires it; the iterative version is the reference implementation and the oracle in tests.

### 7.2.3 ECEF ↔ Battlefield Frame

Given the anchor (φ₀, λ₀, h₀ = 0) with ECEF position **A**, define the rotation matrix **R** whose rows are the East, North and Up unit vectors in ECEF:

```text
e = (−sinλ₀,            cosλ₀,           0     )
n = (−sinφ₀ cosλ₀, −sinφ₀ sinλ₀,  cosφ₀)
u = ( cosφ₀ cosλ₀,  cosφ₀ sinλ₀,  sinφ₀)

p_BF   = R · (p_ECEF − A)
p_ECEF = A + Rᵀ · p_BF
```

Velocities and directions transform with **R** only (no translation). **R** and **A** are computed once per session and stored in `BattlefieldFrame`.

### 7.2.4 Fast "up" vector

For a position in ECEF, the ellipsoid normal direction (accurate to < 2×10⁻⁵ rad for |h| ≤ 20 km) is

```text
up_ECEF ≈ normalize( X / a², Y / a², Z / b² )
up_BF   = R · up_ECEF
```

This avoids the iterative solve for orientation, gravity direction and "level flight" horizons. Anything that needs an exact latitude/longitude (terrain lookups, UI) uses §7.2.2.

## 7.3 Distances, bearings, and map scale

| Use | Method | Error |
|---|---|---|
| Sim-side range/LOS inside a battlefield | Euclidean distance in BF (curved frame, so chord distance) | Exact for the chord; arc differs from chord by < 0.1 % up to 700 km |
| UI distance on the world map | Spherical great-circle (haversine) on R = 6 371 000 m | up to ≈ 0.5 % vs ellipsoid |
| Route planning, "likely engagement" geometry, seed metadata | WGS84 geodesic (Karney or Vincenty inverse) | sub-millimetre |
| Bearings in UI | Initial bearing θ = atan2(sinΔλ cosφ₂, cosφ₁ sinφ₂ − sinφ₁ cosφ₂ cosΔλ) | — |

Haversine: `a = sin²(Δφ/2) + cosφ₁ cosφ₂ sin²(Δλ/2)`, `d = 2R·asin(√a)`.

Metres per degree depend on latitude (WGS84, computed from M(φ) and N(φ)):

| Latitude | m per ° latitude | m per ° longitude |
|---|---|---|
| 0° | 110 574 | 111 319 |
| 35° | 110 941 | 91 288 |
| 45° | 111 132 | 78 847 |
| 60° | 111 412 | 55 800 |
| 70° | 111 562 | 38 187 |
| 80° | 111 660 | 19 393 |

Vincenty's iteration can fail to converge for near-antipodal points; the geodesic implementation MUST use Karney's algorithm or fall back to the spherical solution with a flagged accuracy.

## 7.4 The Battlefield Frame is curved (why and how)

### 7.4.1 Why a flat tangent plane is not acceptable

The Earth's surface falls away from a tangent plane by approximately `d² / (2R)`:

| Distance from anchor | Surface drop below the tangent plane |
|---|---|
| 10 km | 7.8 m |
| 50 km | 196 m |
| 100 km | 785 m |
| 300 km | 7.06 km |
| 500 km | 19.6 km |

A flat model would place the sea surface 7 km too high at the edge of a Huge battlefield. That alters the geometric horizon, radar line-of-sight, ship visibility, terrain masking at range and the apparent altitude of everything — the exact effects this game builds gameplay on (outline §28, §139). Therefore BF is the rigid ECEF transform of §7.2.3 and the sim handles curvature natively.

### 7.4.2 Horizon reference values

Geometric horizon distance for an observer at height h over a smooth sphere: `d = √(2Rh + h²)`. Radar/radio horizon uses an effective radius of 4/3 R to account for standard atmospheric refraction.

| Observer height | Visual dip angle | Geometric horizon | Radar horizon (4/3 R) |
|---|---|---|---|
| 100 m | 0.32° | 35.7 km | 41.2 km |
| 1 000 m | 1.02° | 112.9 km | 130.3 km |
| 5 000 m | 2.27° | 252.5 km | 291.5 km |
| 10 000 m | 3.21° | 357.1 km | 412.2 km |
| 15 000 m | 3.93° | 437.4 km | 504.8 km |

The renderer MUST reproduce the dip angle (a visible horizon that sits 3.2° below the eye line at 10 km altitude); a flat-plane horizon is a known-fail screenshot test (§60.10).

### 7.4.3 What BF implies for every system

| Concern | Rule |
|---|---|
| Gravity | `g(h) = g₀ · (R / (R + h))²` directed along `−up_BF(p)`. Optional (Simulator preset): Somigliana normal gravity `γ = 9.7803253359 (1 + 0.00193185265241 sin²φ) / sqrt(1 − e² sin²φ) − 3.086×10⁻⁶ h`. |
| Coriolis | Off by default. Optional (Simulator preset) for projectiles/missiles: `a = −2 Ω × v`, Ω = 7.2921159×10⁻⁵ rad/s about ECEF Z (rotated into BF). At 1000 m/s this is 0.146 m/s². |
| Level flight / horizon | "Level" means velocity perpendicular to `up_BF(p)`. Aircraft flying a straight great-circle path continuously rotates relative to BF axes. |
| Heading | Heading is measured in the local tangent plane at the aircraft (not BF's x/y). Autopilots (§50.4) steer on this heading. |
| Altitude | MSL altitude = ellipsoid height h of the point (ADR-05). AGL = h − terrainHeight(φ, λ). |
| Terrain meshes | Built in ECEF then transformed to BF per tile with a tile-local origin (§44.8). |
| Ocean | The sea surface is the ellipsoid at h = 0 (plus waves). |
| Spatial hash | Uses BF Cartesian cells (3D hash, 2 km cells default); valid because BF is Cartesian. |
| Replays / net | Quantisation is in BF (§33.5). |

### 7.4.4 Valid extent of a BF

A BF is valid within 1 000 km of its anchor. Across that range the 32-bit-render concern is handled by camera-relative rendering (§0.3.3). The ellipsoid is *not* replaced by a sphere anywhere in the sim; a sphere-of-curvature approximation differs from the ellipsoid by ≈ 66 m in height at 500 km and would break terrain collision at coastlines. The campaign (§35) creates a new BF per Operation.

## 7.5 Altitude definitions

| Term | Definition | Used for |
|---|---|---|
| MSL altitude | Height above the MSL-as-ellipsoid surface (ADR-05) | Stored position, ISA atmosphere, HUD "ALT" default |
| AGL / radar altitude | MSL altitude minus base-DEM terrain height under the aircraft (or sea level over water) | Radar altimeter, ground proximity warning, terrain following |
| Pressure altitude | ISA altitude that produces the local static pressure | Altimeter set to 1013.25 hPa (standard); QNH/QFE settings for realism levels |
| Density altitude | ISA altitude that produces the local density | Performance planning in loadout (§14.5) |
| Geometric vs geopotential | Geometric altitude h; ISA formulas take geopotential altitude `H = R₀h / (R₀ + h)`, R₀ = 6 356 766 m | The atmosphere module converts internally; difference is 0.16 % at 10 km |

## 7.6 World clock

- Every session has a **UTC start instant** (date + time) and a monotonically increasing `simTick` (64-bit). `utc(t) = start + tick × Δt`.
- All astronomy (§24.7) is derived from UTC and the observer's latitude/longitude. Supported date range: 1950-01-01 to 2100-12-31.
- Time-of-day presets are expressed as **sun-elevation conditions** and resolved to a clock time for the chosen location and date (§24.5, §24.6), so they remain geographically correct.
- Time acceleration (§49) scales how many ticks run per real second; it never changes Δt.

## 7.7 Geo service interface (sim-side)

The Sim Core accesses the world only through these traits (ADR-16). The browser build implements them against streamed tile data; the headless server implements them against a local archive; tests implement them with synthetic terrain.

```rust
pub trait GeoService {
    fn frame(&self) -> &BattlefieldFrame;
    fn bf_to_geo(&self, p: DVec3) -> Geodetic;                  // lat, lon (rad), h (m)
    fn geo_to_bf(&self, g: Geodetic) -> DVec3;
    fn up_bf(&self, p: DVec3) -> DVec3;                          // §7.2.4
    fn terrain_height(&self, lat: f64, lon: f64) -> HeightSample; // MSL m + validity/lod flags
    fn terrain_normal(&self, lat: f64, lon: f64) -> DVec3;
    fn surface_class(&self, lat: f64, lon: f64) -> SurfaceClass; // water/land/ice/urban/...
    fn los(&self, a: DVec3, b: DVec3, k_factor: f64) -> LosResult; // k = 1.0 visual, 4/3 radar
    fn raycast(&self, o: DVec3, d: DVec3, max: f64) -> Option<Hit>;
    fn nearest_airport(&self, p: DVec3) -> Option<AirportRef>;
}
pub struct HeightSample { pub h: f32, pub quality: SampleQuality } // quality: Source, Procedural, Missing
```

Rules: (1) `terrain_height` MUST be a pure function of position and data version (ADR-15). (2) Missing data returns `quality = Missing` and a deterministic fallback (§46.9), never `NaN`. (3) Calls are batch-friendly: a `terrain_height_batch(&[LatLon], &mut [HeightSample])` variant exists for AI and sensors.

## 7.8 Edge cases

| Case | Rule |
|---|---|
| Antimeridian (±180°) | Longitudes are normalised to (−180°, 180°]. Differences use `Δλ = ((λ₂ − λ₁ + 540°) mod 360°) − 180°`. Tile column index wraps modulo `2^(z+1)`. BF is Cartesian, so battlefields may straddle the antimeridian with no special handling. |
| Poles | Anchor latitude is clamped to ±85° at launch (§45.8). Inside BF the sim is singularity-free (Cartesian). True-north heading is undefined within 1° of a pole; HUD switches to grid heading (the heading in the BF frame, labelled "GRID"). |
| Open ocean | Terrain height is bathymetry-negative (visual/sonar) but collision treats sea level (h = 0) as the surface for aircraft; ships use bathymetry for depth checks (§22.3). |
| Missing/failed tiles | Fallback to the next coarser available level, then to a procedural surrogate (§46.9). A mission MUST NOT start without at least z≤10 coverage of the playable area (§8.8). |
| Very high / low terrain | Terrain heights outside [−11 000, 9 000] m are clamped; ceiling is 30 000 m (§8.4). |
| Date outside supported range | Clamped, with a UI warning. |

## 7.9 Acceptance criteria

- `AC-7.1` The §7.2.1 test vector passes within 1 mm; round-trip geodetic→ECEF→geodetic over 10⁶ random points (|φ| ≤ 89.9°, h ∈ [−500, 30 000] m) has max error < 1 µm height, < 1e-12° angle.
- `AC-7.2` For 10⁵ random BF points within 1 000 km of 100 random anchors, `bf → geo → bf` returns within 1 µm.
- `AC-7.3` The curvature table (§7.4.1) is reproduced by sampling `up_BF` and ranges within 0.5 %.
- `AC-7.4` Headless WASM and native builds produce identical `bf_to_geo` results bit-for-bit (ADR-10).
- `AC-7.5` A battlefield straddling ±180° longitude runs a 5-minute scripted flight with no discontinuity in terrain height (max step < 1 m at 120 Hz sampling).

---

# 8. BATTLEFIELD SYSTEM

## 8.1 Definition

A **battlefield** is the bounded region in which the simulation runs. Every combat mission creates one. The playable area is separate from the surrounding visible geography so the player can see distant terrain without the whole region being simulated.

```json
{
  "schema": "battlefield/1",
  "anchor": { "lat": 36.2, "lon": -121.8 },
  "shape": { "type": "capsule", "a": { "lat": 36.6, "lon": -121.9 }, "b": { "lat": 35.1, "lon": -117.6 }, "radius_m": 90000 },
  "rotation_deg": 0,
  "ceiling_m": 30000,
  "size_class": "huge",
  "horizon_ring_km": 250,
  "boundary": { "warn_width_m": 7500, "countdown_s": 30, "policy": "mission_fail" },
  "data_requirements": { "min_level": 10, "airport_level": 13 }
}
```

## 8.2 Shapes

| Shape | Parameters | Inside test |
|---|---|---|
| Rectangle | center, width, height, rotation | Rotate into box frame, compare half-extents |
| Circle | center, radius | `\|p − c\| ≤ r` (BF distance) |
| Ellipse | center, a, b, rotation | `(x/a)² + (y/b)² ≤ 1` in rotated frame |
| Capsule ("corridor") | two points, radius | Distance to segment ≤ r (used for airport-to-airport) |
| Polygon (irregular) | ≤ 64 vertices (simple polygon, concave allowed) | Winding-number test |
| Campaign-defined | Union/intersection of the above generated from the Operation | Evaluated through the SDF (below) |

All shapes are authored in geodetic coordinates and projected to BF at load. Shapes are always evaluated in the horizontal BF (x, y) plane; because the Earth curves, the "horizontal plane" for a point is the tangent plane at the anchor, which is correct to well under 0.1 % of area up to the Extreme preset.

### Signed distance field (SDF)

For fast boundary queries the loader rasterises each shape to a **signed distance field**: a float32 grid, negative inside, positive outside, bilinear-sampled. Grid size is `min(1024, ceil(extent / 100 m))` cells per side (≤ 4 MB). From it the sim gets, in O(1): distance to boundary, boundary normal (gradient), and inside/outside. AI avoidance (§19.10), boundary warnings (§8.5) and spawn validation all use the SDF.

## 8.3 Playable area vs visible horizon

| Region | Simulated | Rendered | Streamed terrain level |
|---|---|---|---|
| Playable area | Yes | Yes | Per screen-space error, up to z13 |
| Horizon ring (outside boundary, up to `horizon_ring_km` ≈ distance to the radar horizon of a 15 km-high observer capped at 250 km) | No entities | Terrain and sea only, optional haze/dim shader | Capped at z9 |
| Beyond horizon ring | No | Distant terrain impostor from base globe pack (z≤7) | z≤7 |

Horizon-ring terrain is purely visual (ADR-15): it does not participate in LOS or collision except as "no-entity" backdrop. Aircraft are never simulated there; leaving the area triggers §8.5.

## 8.4 Vertical limits

| Limit | Value | Notes |
|---|---|---|
| Floor | Terrain surface / sea surface | Collision per §18.8 |
| Operating ceiling | Aircraft service ceiling (data) | Aerodynamics enforce it naturally |
| Hard ceiling | 30 000 m MSL | Above this, controls are disabled and the aircraft is "out of airspace" (treated like a Stage 3 boundary violation) |
| Minimum safe altitude (optional rule) | e.g. 100 m AGL in 1v1 "hard deck" rule sets | §31.5 |

## 8.5 Boundary enforcement

The player must always understand the combat area. Leaving it triggers progressively stronger responses (outline §14).

| Stage | Trigger | Player feedback | Parameters |
|---|---|---|---|
| 1 – Advisory | Inside the **warning band**: SDF in `[−W, 0]`, `W = max(2 km, 3 % of the shorter edge)` | Amber HUD edge marker, short tone, radio "approaching boundary" | `warn_width_m` |
| 2 – Restricted | SDF > 0 (outside) | Red HUD border, continuous tone, map highlight, "RETURN TO BATTLEFIELD" | — |
| 3 – Countdown | Outside for > 3 s | Countdown timer on HUD (default 30 s; Hard 15 s; Simulator 10 s). Autopilot-assist offers "turn back" (Easy only: engages automatically) | `countdown_s` |
| 4 – Enforcement | Countdown expires | Policy per realism preset: `assist_turn` (Easy: autopilot steers inward and holds until inside), `mission_fail` (Normal/Hard: aircraft declared lost — "Out of Area"), `deserter` (Simulator: also counts as an aircraft loss in campaign) | `policy` |

Rules:

- Countdown resets only after being inside for 5 s (prevents edge hovering).
- **Extraction zones** (waypoint type `Extraction`, §50.6) may lie on the boundary; passing through one is a legal exit and ends the sortie with "extracted".
- AI follow the same rules logically: an AI that exits for any reason and is not in pursuit is despawned after 60 s outside; a pursuing AI breaks off at Stage 2 (§19.10).
- In multiplayer each player has an independent countdown.
- The boundary is drawn in-world as a translucent vertical curtain visible within 15 km and on the tactical map always (§55).

## 8.6 Battlefield size presets and sizing from airports

Presets (§0.6.4) are the *named* sizes. For an airport-to-airport battle the game computes a corridor:

```text
L      = geodesic length A→B
r      = max(30 km, 0.30 · L)                 capsule half-width
shape  = capsule(A, B, r)
area   = 2 r L + π r²
class  = smallest preset whose area ≥ shape.area
```

Example: L = 300 km → r = 90 km → area = 2·90·300 + π·90² = 54 000 + 25 447 = 79 447 km² → **Huge** (62 500 < 79 447 ≤ 250 000). If `class` is Extreme and the capsule's longest dimension exceeds 1 000 km the setup UI refuses with "Too far — choose closer airports or use a Long-Range Operation (§49)".

The player may override the shape and size manually (drag handles on the map preview, §9.7). A manual size smaller than the A–B corridor is allowed but the second airport is then out of area and shown as "outside battlefield" (usable only as an extraction/waypoint target).

## 8.7 Anchor and local frame

The BF anchor is the geodetic position of the shape's centroid at h = 0, computed in ECEF (average of ECEF samples of the boundary, then projected to the ellipsoid) so that it is stable for shapes crossing the antimeridian. `BattlefieldFrame { anchor, A_ecef, R }` is part of the `SessionConfig` and is sent to every client at connect (§33.4).

## 8.8 Battlefield validator

Before a mission may start, `BattlefieldValidator` runs on the authority and must pass:

| Check | Fail behaviour |
|---|---|
| Shape is simple, area ≥ 1 km², extent ≤ 1 000 km | Reject in UI |
| Terrain coverage ≥ z10 for 100 % of the area; z13 within 10 km of each airport and each human spawn | Offer "download missing (N MB)" or auto-lower detail; if the user is offline and data is missing, fall back per §46.9 and show a banner "synthetic terrain in N % of area" |
| Spawns are on valid land/water, slope and clearance | Auto-correct (nudge ≤ 3 km), else regenerate with next RNG stream (§26.4) |
| Entity budget fits hardware profile (§0.6.2) | Scale forces down proportionally with a message |
| At least one side has a viable operating base or air-spawn | Else regenerate |

## 8.9 Acceptance criteria

- `AC-8.1` SDF queries match analytic distance for rectangle/circle/capsule within 1 grid cell.
- `AC-8.2` The Stage 1→4 sequence fires at the specified distances and times (±0.1 s) in a scripted flight for each realism preset.
- `AC-8.3` The worked example (L = 300 km) yields `class = huge`.
- `AC-8.4` A battlefield with a polygon of 64 vertices rasterises to an SDF in < 50 ms on the Recommended profile.

---

# 9. WORLD MAP

## 9.1 Intent

The world map is one of the most important interfaces: a Google-Earth-like experience built on our own data (ADR-12). It is where the player decides *where to fight*. It MUST be fast, legible, and capable of handing a selection straight to the battle generator.

## 9.2 Camera model

The map camera is an **orbit camera on the ellipsoid**:

```text
state: { lat, lon, height_m (above ellipsoid), heading, tilt, roll=0 }
height range: 300 m … 40 000 km   (log-scaled control)
tilt: 0° (looking straight down) … 80° (near-horizon oblique)
```

| Input | Action |
|---|---|
| Left-drag | Pan: the point under the cursor stays under the cursor (computed with the ray-ellipsoid pick in §9.3) |
| Wheel / pinch | Zoom toward the cursor: new height = old × 0.85^steps, camera translates so the picked point keeps its screen position |
| Right-drag / Alt+drag | Rotate heading and tilt |
| Double-click | Fly-to the picked point at a context-appropriate height (airport: 6 km; terrain: ½ current height) |
| Arrow keys / WASD, +/− | Pan / zoom |
| `Home` | Reset to full globe |
| `/` or `Ctrl+F` | Focus search |

Smoothing: all camera parameters follow a **critically damped spring** with time constant 0.12 s (pan/zoom) and flights use ease-in-out over `clamp(0.6 s + distance_term, 0.6 s, 2.5 s)`. Camera height is clamped to stay ≥ 100 m above local terrain at the camera foot-point to prevent clipping. Reduced-motion mode (§57) replaces animated flights with a 150 ms cross-fade.

## 9.3 Picking

**Ray vs ellipsoid.** To intersect a ray `o + t·d` with the WGS84 ellipsoid, scale space by `(1/a, 1/a, 1/b)` so the ellipsoid becomes the unit sphere: `o' = (oₓ/a, o_y/a, o_z/b)`, `d' = (dₓ/a, d_y/a, d_z/b)`. Solve `|o' + t d'|² = 1`: `A = d'·d'`, `B = 2 o'·d'`, `C = o'·o' − 1`, `t = (−B − √(B² − 4AC)) / 2A` (nearest root); no real root ⇒ the cursor is off the globe (space).

**Terrain refinement.** When camera height < 200 km, the pick is refined by marching the ray through the currently loaded height tiles (step ≤ half the post spacing, then 8-step bisection) so mountains are selectable at their true height. If tiles are not yet loaded, the ellipsoid hit is used and refined when data arrives (the selection marker animates to the new height).

## 9.4 Layers and overlays

| Layer | Content | Default |
|---|---|---|
| Base | Terrain + albedo from base pack and procedural land cover; night-side city lights | On |
| Atmosphere | Scattering shell and limb glow (§9.9) | On |
| Coastlines / water | Vector polylines from Natural Earth / OSM at appropriate scale | On |
| Place labels | Cities, regions, seas (GeoNames / Natural Earth) with declutter | On |
| Airports | Markers by class; runway glyphs at ≤ 80 km height | On |
| Roads / rail / power | Vector at ≤ 400 km height | Off |
| Political borders | Admin-0/1 lines | **Off** (§9.8) |
| Selection | A, B markers, route geodesic, battlefield footprint (ghost polygon), size handles | Contextual |
| Measurement | Multi-point path with cumulative distance and bearing | Tool |
| Campaign | Faction control tint, frontlines, supply lines, intel fog (§35, §42) | Campaign only |
| Graticule / scale bar / compass | Optional | Scale bar on |

**Marker declutter.** Screen-space greedy algorithm each time the camera settles: sort candidates by `importance` descending; maintain a 48-px occupancy grid; place a marker if its cell and neighbours are free, otherwise hide it; fade over 120 ms. Importance: airport class weight (Major 100, Military 80, Regional 50, Airstrip 20), city population (log₁₀), and user bookmarks (+1000).

## 9.5 Location search

Search is fully **offline** after the base pack is cached.

**Index.** Built in the data pipeline (§45.5) from GeoNames (cities ≥ 1 000 population, plus physical-feature classes), Natural Earth (physical regions, marine areas, countries/states as *geographic* extents), and the airport database (name, ICAO, IATA, local codes). Stored as an **SQLite database with FTS5** (`unicode61` tokenizer with diacritic removal) shipped as a single file, opened in the browser with SQLite compiled to WebAssembly and OPFS persistence [verify]. Estimated size: ≈ 40–60 MB compressed.

**Query handling:**

| Query form | Example | Result |
|---|---|---|
| Place | `Tokyo`, `Reno` | Fly to place centroid; height by feature class |
| Region/physical | `Alps`, `Nevada`, `Pacific Ocean` | Fly to feature bounding extent; outline shown briefly |
| Airport | `KLAX`, `LAX`, `Heathrow` | Select airport and fly to it |
| Coordinates | see §9.6 | Fly to point |
| Disambiguation | `Springfield` | Ranked list with admin context |

**Ranking:** `score = 0.50·text + 0.30·importance + 0.20·proximity`, where `text` = 1.0 exact, 0.8 prefix, 0.6 token-prefix, 0.4 fuzzy (Levenshtein ≤ 2 for tokens ≥ 5 chars); `importance` = normalised `log₁₀(population)` or airport-class weight; `proximity` = `exp(−d / 2000 km)` from the camera foot-point (0 if camera is above 5 000 km). Debounce 120 ms; show ≤ 12 results; target latency ≤ 50 ms for a 140 k-row table.

## 9.6 Coordinate entry

Accepted formats (case-insensitive, tolerant of `°`, `'`, `"`, commas, and extra whitespace):

| Format | Example |
|---|---|
| Decimal degrees, signed | `35.1234, -117.5678` |
| Decimal degrees, hemisphere letters | `35.1234N 117.5678W` |
| Degrees-minutes-seconds | `35°07'24"N 117°34'04"W` |
| Degrees-decimal-minutes | `N35 07.404 W117 34.068` |

Validation: `lat ∈ [−90, 90]`, `lon ∈ [−180, 360)` then normalised to (−180, 180]; seconds and minutes ∈ [0, 60). Errors produce inline text ("Latitude must be between −90 and 90"). On success the camera flies to the point and drops a pin. Separate lat/lon numeric fields (as in the outline's example) are also available. MGRS and Plus Codes are `P10` stretch formats.

## 9.7 Selection tools

| Tool | Behaviour |
|---|---|
| Airport A/B | Click, search, or list; A = player base, B = enemy base (swap button). Both appear with runway glyphs and a geodesic line with distance, bearing, and estimated transit time at a configurable cruise speed (default 250 m/s) |
| Arbitrary point | Shift-click or "Drop pin"; battle generated around it with air spawn |
| Region | Drag rectangle / circle / draw polygon; becomes the battlefield shape |
| Measure | Click points; shows leg and total distance, bearing; Esc to clear |
| Battlefield preview | Ghost footprint with size-class label ("Huge — 79 447 km²") and corner handles; dimming outside |
| Terrain profile | Elevation chart along A–B sampled every 1 km (shows highest terrain, radar-shadow bands for a 100 m AGL flyer using the LOS service) |
| Data readiness | Shows % of required tiles cached, MB to download, offline readiness (§45.4) |

**Likely engagement zone** (shown as a translucent disc): centre at fraction `t = 0.5 + 0.2·s` along A→B with `s = (S_A − S_B)/(S_A + S_B)` (S = configured strength of each side, so a stronger player pushes the fight toward the enemy), radius `0.2·L`.

## 9.8 Political depiction and content policy

- Political borders and sovereignty labels are **hidden by default**. Labels show geographic names (settlements, physical features, seas) in their local primary name with a transliteration.
- An optional "Show administrative boundaries" toggle uses a *de facto* boundary dataset and carries an on-screen disclaimer that borders are for orientation only.
- Territory shading and flags are only used for **game factions** (§37), never for real nations.
- No real national insignia or flags appear on aircraft, ships or UI. Faction names are fictional.
- Region-specific publishing constraints are tracked as OQ-08; the policy is implemented as data (a `map_policy.json` that can swap boundary and label sets by build/region), not code.

## 9.9 Globe rendering

- The globe is the **same terrain quadtree** at coarse LOD (z0–z5 from the base pack); there is no separate globe mesh, so zooming from space to a runway is continuous.
- **Atmosphere:** precomputed transmittance/multiple-scattering/sky-view LUTs generated at load by compute shaders (Bruneton / Hillaire-style), recomputed only when the planet's atmosphere parameters change; per-frame cost is a LUT fetch per pixel. Night side shows city lights (public-domain night-lights imagery) blended by the sun's elevation.
- **Albedo:** low-resolution global albedo (public-domain Blue-Marble-class imagery) in the base pack, blended with procedural land-cover colours (§46.5) as height decreases.
- **Terminator and time:** the map uses the session's UTC time (default: now) so the globe shows a correct day/night boundary; the date/time control in the setup panel updates it live.

## 9.10 Bookmarks

```json
{ "schema": "bookmark/1", "id": "uuid", "name": "Sierra valley arena", "lat": 37.7, "lon": -119.5, "height_m": 60000,
  "tags": ["mountain_arena", "multiplayer"], "notes": "", "created": "2026-10-07T12:00:00Z", "kind": "location|airport|battlefield" }
```

Tags: `favorite_airport`, `dogfight_area`, `carrier_area`, `mountain_arena`, `multiplayer`. Stored in IndexedDB (§64), exportable/importable as JSON, shareable via URL fragment (a bookmark encodes to ≤ 120 characters).

## 9.11 Performance and UX budgets

| Item | Target |
|---|---|
| Globe frame time on Recommended | ≤ 8 ms GPU |
| Search latency | ≤ 50 ms |
| Pick latency (ellipsoid) | < 0.1 ms; terrain refine < 1 ms |
| Fly-to completes without hitching | frame-time p99 ≤ 20 ms |
| Memory for map layers | ≤ 250 MB |

## 9.12 Acceptance criteria

- `AC-9.1` Picking the same screen pixel at 100 camera positions yields the same lat/lon as an independent reference implementation within 1 m (ellipsoid) / 5 m (terrain-refined).
- `AC-9.2` All formats in §9.6 parse to the same coordinates within 1e-4°; invalid inputs yield errors, never exceptions.
- `AC-9.3` Search for the 200 most-populated cities and 200 largest airports by name returns the target in the top 3.
- `AC-9.4` With the borders toggle off, no political boundary pixels are drawn (visual regression test).
- `AC-9.5` Selecting A, B and Huge size triggers a battlefield preview within 1 s and reports cached/uncached tile counts.

---

# 10. AIRPORT SYSTEM

## 10.1 Intent

Airports are persistent geographic objects that feel like physical places, not menu screens. They are real (from data) wherever possible, procedurally completed where not, and they are the nodes of the campaign (§35).

## 10.2 Airport types

| Type | Typical runway | Capabilities | Default defences | Default capacity (parked aircraft) |
|---|---|---|---|---|
| **Major International** | 2–4 runways, 2 500–4 000 m, paved | Full services, large hangars, many stands | Light (short-range SAM optional) | 40–120 (mostly civilian) |
| **Military Airbase** | 1–3 runways, hardened shelters | Full military services, rearm + repair, QRA (alert) shelters, SAM site | Medium–heavy | 20–80 |
| **Regional Airport** | 1–2 runways, 1 200–2 500 m | Fuel, limited rearm, small hangars | Light | 8–30 |
| **Remote Airstrip** | 1 runway, 600–1 500 m, may be unpaved | Fuel optional, no repair | None–light | 2–8 |
| **Improvised Airfield** | Procedural strip, 900–1 500 m | Emergency landing, limited rearm | None | 2–6 |
| **Aircraft Carrier** | Angled/straight deck (§22.6) | Launch/recover, rearm, refuel, hangar | Self-defence | 20–70 depending on class |

### Classification from data

| Source field | Rule → game type |
|---|---|
| OSM `military=airfield` or name/operator tag indicates military | Military Airbase |
| OurAirports `large_airport` | Major International (unless military tag above) |
| `medium_airport` | Regional Airport |
| `small_airport`, longest runway ≥ 1 200 m and paved | Regional Airport |
| `small_airport` otherwise | Remote Airstrip |
| `heliport`, `seaplane_base`, `closed` | Not selectable as fixed-wing bases (shown as features); `closed` with a surviving runway may be offered as Improvised Airfield |
| Missing runway data | Improvised Airfield generated at the site (§10.8) |

## 10.3 Data model

```json
{
  "schema": "airport/1",
  "id": "KMRY",
  "name": "Monterey Regional",
  "type": "regional",
  "position": { "lat": 36.587, "lon": -121.843, "elev_m": 77 },
  "runways": [
    { "id": "10R/28L", "le": { "lat": 36.5871, "lon": -121.8512, "elev_m": 77, "heading_true_deg": 99.6 },
      "he": { "lat": 36.5874, "lon": -121.8317, "elev_m": 77 }, "length_m": 2300, "width_m": 45, "surface": "asphalt", "lighted": true, "displaced_le_m": 0 }
  ],
  "taxi": { "nodes": [], "edges": [] },
  "stands": [], "buildings": [],
  "facilities": { "fuel": true, "rearm": false, "repair": "none", "tower": true },
  "defenses": [],
  "faction": null,
  "confidence": { "runways": "source", "taxi": "procedural", "buildings": "osm" }
}
```

(Values shown are illustrative; real data come from the pipeline.)

**Sources and merge order (highest priority first):** OSM `aeroway=*` geometry (runway lines, taxiways, aprons, hangars, towers) → OurAirports runway endpoints (thresholds, lengths, surfaces, true headings) → procedural completion (§10.6). Each field records a `confidence` source so debug tools can show what is real.

**Runway endpoint recovery.** If threshold coordinates are missing: take airport centre, runway length `L`, and true heading ψ, then place the thresholds at the geodesic direct solutions ±L/2 along ψ. If only a runway designator (e.g. `09L`) exists, heading ≈ number × 10° **magnetic** — convert to true using World Magnetic Model declination (public-domain coefficients) and mark `confidence: low`. Parallel designators `L/C/R` are offset laterally by 120 m default.

**Deduplication.** Records within 2 km having overlapping runway lines are merged; the higher-quality source wins.

## 10.4 Runway and surface model

| Surface | Rolling resistance μ_r | Braking friction μ_b dry | wet | snow/ice | Notes |
|---|---|---|---|---|---|
| Asphalt / concrete | 0.02 | 0.70 | 0.40 | 0.15 | Design targets |
| Grass | 0.05 | 0.40 | 0.30 | 0.15 | Mass limit 15 000 t-class aircraft warn |
| Dirt / gravel | 0.04 | 0.50 | 0.35 | 0.20 | Dust effect on engines (cosmetic + FOD risk flag) |
| Snow (packed) / ice | 0.04 | 0.20 | n/a | 0.10 | Reduced steering authority |

Surface selection per point comes from the runway polygon first, then land cover. Wet/snow state derives from the weather system (§24.4).

**Take-off / landing distance planner.** The loadout screen (§14.5) runs the real ground model (§11.13) headlessly at ≥ 8× real time for the chosen runway, density altitude, wind, and mass, and reports ground roll and 15 m obstacle distance (take-off) and ground roll (landing). Rule: a runway is "OK" if `length ≥ 1.3 × required`, "MARGINAL" if `≥ 1.0 ×`, else "TOO SHORT".

## 10.5 Airfield layout and ground network

- **Runway** polylines with threshold, displaced threshold, and touchdown-zone markings.
- **Taxi graph:** nodes (hold-short, junction, stand entry) and directed edges with width and max speed; used by ground-AI A* with reservation (a node may be reserved by one aircraft at a time; reservations expire after 20 s) to avoid gridlock.
- **Stands:** each has position, heading, size class (small ≤ 18 m wingspan, medium ≤ 35 m, large ≤ 60 m), and shelters.
- **Buildings:** hangars, control tower (also sets visual-ATC radio voice), fuel farm (target), maintenance, barracks, radar dome, hardened shelters (military). Each has HP/armour class for campaign damage (§48.7).

## 10.6 Procedural completion

When taxiway/apron/building data are missing, `AirportLayoutGenerator` builds a plausible layout deterministically from `hash(airport_id, data_version)`:

1. Take runway centerlines.
2. Parallel taxiway on the side with more free flat land, offset `90 m + width/2`; high-speed exits every 800 m after the first 1 000 m on runways > 1 800 m.
3. Connectors between runway and taxiway at thresholds and every 600–900 m.
4. Apron at mid-length, sized by type: Major 400×300 m, Military 300×200 m, Regional 150×100 m, Airstrip 60×40 m.
5. Stands in rows with spacing = `wingspan + 6 m`; hangars (count ≈ 1 per 4 stands, min 1 for Regional+).
6. Tower on the apron edge; fuel farm ≥ 100 m from stands; military: shelters ring, radar dome 400 m away.
7. Validate against slope (< 2 %) and land cover (no water/forest); discard and shift by 50 m up to 6 times.

The result is *never* presented as real: UI shows "layout: generated" in the airport info panel.

## 10.7 Airport pad flattening

DEMs have 30 m resolution and noise, so raw terrain under a runway is bumpy. `AirportFlatten` modifies the base DEM deterministically (so collision agrees everywhere, ADR-15):

```text
for each height post within the airport footprint buffered by B (B = 300 m):
    d  = distance from the post to the nearest runway/taxi/apron polygon (0 inside)
    w  = smoothstep(1 − clamp(d / B, 0, 1))              1 inside, 0 at buffer edge
    hp = pad height: linear along runway axis between threshold elevations
         (recorded elevations, else DEM median along the runway), 0 cross-slope
    h' = lerp(h_dem, hp, w)
```

The patch is baked into the airport's tile-delta layer in the pipeline so streamed tiles already contain it; the runtime applies no per-load mutation. Max runway longitudinal gradient: 2 % (steeper real runways keep their real gradient from the elevation pair).

## 10.8 Improvised airfield generation (arbitrary locations and missing data)

Search procedure (deterministic, seeded):

1. Candidate centres: Poisson-disc samples (spacing 1.5 km) in the requested region.
2. For each, test 18 headings (0°–170° by 10°). Sample a strip `L × 40 m` (L = 1 200 m default, 900 m min) on the base DEM at 20 m spacing.
3. Reject if any longitudinal slope > 2.5 %, cross-slope > 1.5 %, elevation std-dev (roughness) > 0.5 m, or land cover is water, forest, urban, wetland.
4. Score: `score = −(2·max_slope + roughness) + 0.5·approach_clearance − 0.2·distance_to_target`, where approach clearance = terrain clearance on 3° glide paths at both ends.
5. Select the best; place a threshold-marked strip, a small apron, and generic equipment per type; mark `improvised: true`.
6. If nothing qualifies in the region, fall back to **air-spawn** (the mission uses an air start, §48.2) and report it in the briefing.

## 10.9 Navigation aids and landing assist

| Aid | Behaviour |
|---|---|
| Runway lights, approach lights, PAPI | Night/low-visibility rendering; PAPI 3.0° glidepath |
| ILS-like beams | Localiser full-scale ±2.5°, glideslope 3.0° with full-scale ±0.7°; usable ≤ 30 km and within ±35° of the runway axis |
| TACAN-like beacon | Range and bearing readout to the airfield when selected |
| Landing assist (optional) | Overlay of the ideal path and speed cues; Standard Assist and below only |

## 10.10 Airport state and ownership

Each airport has `faction`, `alert_level` (§48.5), per-facility `health` (§48.7), and `stock` (fuel, ammunition, spares; sandbox = infinite). In campaign these persist (§35–39).

## 10.11 Edge cases

| Case | Handling |
|---|---|
| Runway on steep terrain after flatten | Flag `unsafe`, allow with warning |
| Runway length < 600 m | "STOL only"; aircraft data flag `stol` gates use |
| High elevation (> 2 000 m) | Reduced thrust/lift handled by atmosphere (§11.3); planner shows density altitude |
| Airport over water (data error) | Dropped if > 50 % of runway is water, unless seaplane/carrier |
| Overlapping or duplicate airports | Dedup per §10.3 |
| Polar/snow surfaces | Surface forced to snow/ice by land cover and latitude |
| Missing tower or lights data | Generic assets by type |

## 10.12 Acceptance criteria

- `AC-10.1` For 500 randomly sampled real airports, ≥ 98 % produce a usable runway (not flagged `unsafe`) after merge, flatten and procedural completion.
- `AC-10.2` Runway thresholds recovered from (centre, length, heading) match source thresholds within 1 m for 1 000 airports that have both.
- `AC-10.3` The taxi graph for generated layouts is strongly connected from every stand to every runway end.
- `AC-10.4` Improvised airfield search succeeds (finds a strip or reports air-spawn fallback) for 1 000 random land points in < 500 ms each on the Recommended profile.
- `AC-10.5` Take-off planner distance matches a full-FDM take-off within ±3 % for the reference aircraft (Appendix C) at three weights and three densities.

---

# 11. FLIGHT MODEL

`P1 · M1` — The flight model is the most important system in the game (outline §146, Priority 1). The aircraft must feel excellent before any other content is added (§67).

## 11.1 Intent and fidelity philosophy

- **Physically coherent, not a textbook simulator.** The model is a 6-DOF rigid body with table-driven aerodynamics, a real atmosphere, real mass properties and a real energy budget. Players can disable assistance until the aircraft is demanding; they can never get arcade shortcuts that violate energy (NN-2).
- **One model for everybody.** Human aircraft and AI aircraft in tiers T0–T1 use the *same* model and the *same* control law (the AI feeds stick/throttle commands to the flight-control system, §19.12). AI never gets better physics (NN-5).
- **Data, not code.** Every aircraft is a data file (§12); the same solver flies all of them.
- **Accessible by default.** The default assist (Standard) gives a g-command/roll-rate-command FBW feel; physics remains underneath.

The model reproduces: lift, drag, thrust, gravity, air density, angle of attack, stall, overspeed, control-surface effectiveness, acceleration/deceleration, climb/descent, turning, G-loading, energy retention, fuel consumption, and mass change (outline §15).

## 11.2 State and equations of motion

### 11.2.1 State vector (per aircraft)

| Symbol | Type | Frame | Meaning |
|---|---|---|---|
| **p** | `DVec3` | BF | Position of the centre of gravity |
| **v** | `DVec3` | BF | Velocity relative to the Earth-fixed BF |
| **q** | `DQuat` | BODY→BF | Orientation |
| **ω** | `DVec3` | BODY | Angular velocity (p, q, r) |
| m, **I**, **r_cg** | scalars/matrix | BODY | Mass properties (§11.9) |
| δ_e, δ_a, δ_r, flaps, gear, brake, airbrake | scalars | — | Actuator states (§11.7) |
| engine states | struct | — | Spool, AB stage, fuel flow, damage (§11.8) |
| fuel[tank] | array | — | Fuel per tank (§11.9) |

### 11.2.2 Equations

With `R(q)` the rotation BODY→BF and `g(p)` from §7.4.3:

```text
ṗ = v
m v̇ = R(q) · F_body + m · g(p)  [− 2m Ω×v  if Coriolis enabled]
q̇ = ½ q ⊗ (0, ω)
I ω̇ = M_body − ω × (I ω)
```

`F_body` is aerodynamic + propulsive + gear + external (catapult, arrestor, tow) force in BODY; `M_body` is the corresponding moment about the CG. BF is Earth-fixed (rotating with the Earth), so ground speed is what is integrated; Earth-rotation effects are the optional Coriolis term only. Mass is non-constant: `ṁ = −Σ fuel_flow − Σ store_release`; jet momentum is already contained in the net thrust definition.

### 11.2.3 Air-relative quantities

```text
v_air_BF   = v − w(p, t)                         wind from §11.4
v_air_BODY = R(q)ᵀ · v_air_BF = (u, v_b, w_b)
V_T = |v_air|                                    true airspeed
α   = atan2(w_b, u)                              angle of attack
β   = asin(v_b / V_T)                            sideslip
q̄   = ½ ρ V_T²                                   dynamic pressure
M   = V_T / a(T)                                 Mach number
```

The angular-rate non-dimensionals used by the aerodynamic model are `p̂ = p·b/(2V_T)`, `q̂ = q·c̄/(2V_T)`, `r̂ = r·b/(2V_T)`, guarded by `V_T ≥ 1 m/s` to avoid division by zero.

### 11.2.4 Wind-to-body force transformation

Aerodynamic force coefficients are computed in wind axes (drag D opposes airspeed, lift L is perpendicular in the plane of symmetry, side force Y). In BODY axes (x forward, y right, z down):

```text
X = −D cosα cosβ − Y cosα sinβ + L sinα
Y_b = −D sinβ + Y cosβ
Z = −D sinα cosβ − Y sinα sinβ − L cosα
```

Check: with β = 0 and D = Y = 0 the force is `(L sinα, 0, −L cosα)`, perpendicular to the velocity direction `(cosα, 0, sinα)` as required.

*Corrected in 0.2.1:* the side-force term of `Z` was printed `+ Y sinα sinβ`. The wind-to-body rotation applied to `(−D, Y, −L)` gives `− Y sinα sinβ`, and with that sign a pure side force stays perpendicular to the velocity at any α and β (the v0.1a flight model's test).

## 11.3 Atmosphere

International Standard Atmosphere (ISA), with geopotential altitude `H = R₀h / (R₀ + h)`, `R₀ = 6 356 766 m`:

```text
0 ≤ H ≤ 11 000:        T = 288.15 − 0.0065 H           p = 101 325 · (T / 288.15)^5.25588
11 000 < H ≤ 20 000:   T = 216.65                       p = 22 632.06 · exp(−g₀ (H − 11 000) / (R_air · 216.65))
20 000 < H ≤ 32 000:   T = 216.65 + 0.001 (H − 20 000)  p = 5 474.89 · (T / 216.65)^(−34.1632)
ρ = p / (R_air · T)        a = √(γ R_air T)
```

Reference values (geopotential altitude; computed from the formulas above):

| H (m) | H (ft) | T (K) | T (°C) | p (Pa) | ρ (kg/m³) | a (m/s) |
|---|---|---|---|---|---|---|
| 0 | 0 | 288.15 | 15.00 | 101 325.0 | 1.2250 | 340.29 |
| 1 000 | 3 281 | 281.65 | 8.50 | 89 874.6 | 1.1116 | 336.43 |
| 3 000 | 9 843 | 268.65 | −4.50 | 70 108.5 | 0.9091 | 328.58 |
| 5 000 | 16 404 | 255.65 | −17.50 | 54 019.9 | 0.7361 | 320.53 |
| 8 000 | 26 247 | 236.15 | −37.00 | 35 599.8 | 0.5252 | 308.06 |
| 10 000 | 32 808 | 223.15 | −50.00 | 26 436.2 | 0.4127 | 299.46 |
| 11 000 | 36 089 | 216.65 | −56.50 | 22 632.0 | 0.3639 | 295.07 |
| 15 000 | 49 213 | 216.65 | −56.50 | 12 044.6 | 0.1937 | 295.07 |
| 20 000 | 65 617 | 216.65 | −56.50 | 5 474.9 | 0.0880 | 295.07 |

(These tables are in geopotential altitude. Tables quoted in *geometric* altitude differ slightly; e.g. density at 10 000 m geometric, which is 9 984 m geopotential, is 0.4135 kg/m³. The unit tests use the geopotential table above.)

**Non-standard days.** Weather supplies a temperature offset `ΔT` (ISA+ΔT, range −40…+40 K, default from latitude/season climatology, §24.2) and a sea-level pressure `p₀'` (QNH-like; default 101 325 Pa, range 95 000–105 000 Pa). Implementation: `T = T_ISA(H) + ΔT`, `p = p_ISA(H) · p₀'/101 325`, `ρ = p/(R_air T)`. The pressure schedule is held at ISA shape (a deliberate simplification: the hydrostatic effect of ΔT on pressure aloft is neglected). Effects flow naturally into thrust (via ρ), lift/drag (via ρ, a) and Mach; density altitude is reported in the loadout planner.

**Humidity** is ignored in aerodynamics (dry air) and contributes only to visibility and clouds.

**Performance.** The atmosphere module is a pure function with a 1 024-entry lookup table in H (linear interpolation; max error < 0.01 %) used on hot paths; the analytic version is the oracle in tests.

## 11.4 Wind, turbulence, and orographic effects

`w(p, t) = w_mean(H) + w_orog(p) + w_turb(p, t)`

- **Mean wind** comes from the weather system as a vertical profile table `[(altitude_msl, speed, direction_from)]`, linearly interpolated, constant horizontally across the battlefield by default (optional frontal gradient §24.3). Default near-surface profile: `V(z) = V_ref · (z / 10 m)^0.14` for z in [10 m, 600 m] AGL, then follows the table.
- **Orographic (ridge) lift and lee sink:** `w_orog,z = (U_h · ∇h_terrain) · exp(−AGL / λ)`, λ = 300 m, with `U_h` the horizontal wind; the gradient is taken from the base DEM smoothed over 300 m. Lee-side values (negative) produce sink; amplitude clamped to ±8 m/s.
- **Turbulence** per axis is a first-order Gauss–Markov process with correlation length `L_c = clamp(AGL, 100 m, 500 m)` and standard deviation σ from the weather preset: calm 0, light 0.5, moderate 1.5, severe 3.5, extreme 6 m/s. The exact discretisation each update is
  `x_{k+1} = e^{−V Δt / L_c} · x_k + σ √(1 − e^{−2 V Δt / L_c}) · ξ_k`, ξ ~ N(0,1) from the per-aircraft deterministic RNG stream (so replays of a server snapshot never need it). Updated at 30 Hz, interpolated.
- **Wake turbulence and jet blast** (`P10`): optional; a wake is a trailing vortex pair persisting 60 s, modelled as a short list of line vortices per heavy aircraft.

## 11.5 Air data and speed definitions

| Quantity | Definition | Use |
|---|---|---|
| TAS | `V_T` | Dynamics |
| EAS | `V_T √(ρ/ρ₀)` | Structural limits, stall speeds |
| CAS (IAS in HUD) | Calibrated airspeed from impact pressure `q_c`: subsonic `q_c = p[(1 + 0.2M²)^3.5 − 1]`; supersonic (Rayleigh) `q_c = p[166.92 M⁷ / (7M² − 1)^2.5 − 1]`; `CAS = a₀ √(5[(q_c/p₀ + 1)^(2/7) − 1])` (subsonic branch inverted numerically above Mach 1) | HUD airspeed, gear/flap limits |
| Mach | `V_T / a` | Wave drag, thrust map, speed limits |
| Ground speed | `\|v\|` projected on the local horizontal | Landing, navigation |
| Vertical speed | `v · up_BF` | VSI |

## 11.6 Aerodynamic model

### 11.6.1 Structure

Aerodynamics use a **component-buildup-by-table** approach: coefficient tables measured/estimated per aircraft, interpolated multilinearly, summed with derivative terms.

```text
C_L = C_L,base(α, M) + ΔC_L,flap + ΔC_L,δe·δ_e + ΔC_L,ground
C_D = C_D0(M) + ΔC_D,wave(M) + K(M)·(C_L − C_L0)² + ΔC_D,config + ΔC_D,stores + ΔC_D,gear + ΔC_D,brake + ΔC_D,damage
C_Y = C_Yβ β + C_Yδr δ_r
C_l = C_lβ β + C_lp p̂ + C_lr r̂ + C_lδa δ_a + C_lδr δ_r + ΔC_l,asym
C_m = C_m0(α, M) + C_mq q̂ + C_mα̇ α̇̂ + C_mδe δ_e + C_m,cg-shift
C_n = C_nβ β + C_np p̂ + C_nr r̂ + C_nδa δ_a + C_nδr δ_r + ΔC_n,asym
```

Forces `F = q̄ S C`, moments `M = q̄ S b C_l`, `q̄ S c̄ C_m`, `q̄ S b C_n` about a reference point; moments are transferred to the CG with `M_cg = M_ref + (r_ref − r_cg) × F`. Coefficient derivatives marked "per rad" use radians.

### 11.6.2 Required tables

| Table | Axes | Notes |
|---|---|---|
| `C_L,base` | α ∈ [−30°, +90°], M ∈ [0, 3] | Includes vortex-lift region; extended to ±180° by §11.6.3 |
| `C_D0` | M | Zero-lift drag, clean |
| `ΔC_D,wave` | M | Transonic/supersonic drag rise |
| `K` | M | Induced-drag factor |
| `C_L,max` | M | Used by limiters, reduced-order models |
| `C_Lα` | M | Used in linearisation/diagnostics |
| `C_m0` | α, M | Trimmed by δ_e |
| `C_mδe, C_lδa, C_nδr, …` | M (and α for control effectiveness fall-off) | |
| Stability derivatives | scalar or M | See Appendix C for the reference aircraft |
| Damping derivatives | M | `C_lp`, `C_mq`, `C_nr` |
| `ΔC_D,config` | gear, flaps, airbrake | |

All tables are monotone-checked by the content linter (no spikes > 25 % between adjacent grid points unless flagged `discontinuity_ok`).

### 11.6.3 Post-stall extension

Beyond the table's validity, lift and drag are blended to a Newtonian flat-plate model so every α ∈ [−180°, 180°] has a defined, smooth result:

```text
t(α)  = clamp((|α| − α_start(M)) / (α_end(M) − α_start(M)), 0, 1)
σ(α)  = t² (3 − 2t)          C¹ smoothstep: exactly 0 below α_start, exactly 1 above α_end
C_L   = (1 − σ) C_L,table + σ · C_D90 · sinα cosα
C_D   = (1 − σ) C_D,table + σ · (C_D0 + C_D90 sin²α)
C_D90 ≈ 1.2        (flat plate, low aspect ratio)
```

`α_start(M)`, `α_end(M)`, `C_D90` and `x_ac,post` are aircraft data (the `post_stall` block of
`aero/1`). The stall break and the fall of lift just past the peak belong in `C_L,table`; the blend
only reshapes the region beyond it. The baker requires `α_start` to be at least 5° past each Mach
row's lift peak (the Kestrel uses peak + 10°, with `α_end = α_start + 20°`).

*Clarified in 0.2.1 (ADR-30):* this section first defined σ as a logistic centred on α_stall with
w = 2°. That weights the table only 0.5 at the stall itself, which cuts the Kestrel's C_L,max from
1.60 to 1.444 and raises its stall speed by 5.3 %, outside the C.9 tolerance.

Pitching and yawing moments in the post-stall regime follow the same blend with an aerodynamic-centre shift to mid-chord (`C_m,post = −C_N · (x_ac,post − x_cg)/c̄`). This yields tumbling, flat-spin tendencies and rapid energy loss without special-casing.

### 11.6.4 Mach effects

- `C_Lα(M)` rises approximately with Prandtl–Glauert (`1/√(1−M²)`) subsonically, but the table authoritative value is used; a supersonic fall-off is encoded in the table.
- `ΔC_D,wave(M)` is a bump beginning at the critical Mach, peaking near M 1.05, decaying beyond.
- Aerodynamic-centre shift aft with Mach (the "Mach tuck/trim change") appears as `C_m0(α,M)` and requires pitch trim.
- Control-surface effectiveness reduces transonically via `C_mδe(M)`.

### 11.6.5 Ground effect

Induced drag is reduced near the ground (McCormick):

```text
φ_IGE = (16 h/b)² / (1 + (16 h/b)²)         h = height of the wing above ground, b = span
C_Di,IGE = φ_IGE · K (C_L − C_L0)²
ΔC_L,ground = +k_g · C_Lα · α · exp(−h / (0.5 b))    (small, k_g ≈ 0.1)
```

### 11.6.6 Store and configuration effects

Each loadout item contributes `ΔC_D0` (frontal-area-derived, from the item's data), mass/inertia (§11.9), and RCS (§16.4). Asymmetric stores add `ΔC_l,asym`, `ΔC_n,asym` from their lateral offset times drag (and a trim penalty).

### 11.6.7 Damage effects on aerodynamics

Wing, tail, and control-surface damage (§18) modify the aero model by scaling or offsetting coefficients: lost wing area `ΔS`, asymmetric `ΔC_l`, `ΔC_n`, reduced `C_lδa`, etc. (§18.5).

## 11.7 Control surfaces and actuators

Each control channel has a **commanded** deflection from the FCS (§11.11) and an **actual** deflection:

```text
δ̇ = clamp((δ_cmd − δ) / τ_a, −δ̇_max, +δ̇_max);   δ = clamp(δ + δ̇ Δt, δ_min, δ_max)
```

| Surface (reference fighter) | Range | Rate limit | τ_a |
|---|---|---|---|
| Stabilator (pitch) | ±25° | 60°/s | 0.05 s |
| Flaperons/ailerons | ±20° | 80°/s | 0.05 s |
| Rudder | ±30° | 120°/s | 0.05 s |
| Leading-edge flaps (scheduled) | 0–25° | 15°/s | 0.1 s |
| Trailing-edge flaps (if separate) | 0–30° | 20°/s | 0.1 s |
| Airbrake | 0–1 | 1.0/s | 0.3 s |
| Gear | 0–1 | 1/6 s⁻¹ (6 s) | — |

(Design targets.) Hydraulic damage scales `δ̇_max` by the remaining hydraulic fraction (§18.4). Effective control power falls with deflection beyond ±15° via a table `η(δ)`.

## 11.8 Propulsion

### 11.8.1 Thrust

Thrust is a table-driven function of Mach, altitude and throttle state:

```text
F_net = F_SL(mode) · f_alt(ρ/ρ₀) · f_M(M) · η_inlet(α, β) · η_damage
```

Reference placeholder generator for new engines (tuned later by the authoring tool): `f_alt = (ρ/ρ₀)^0.75`, `f_M` from a per-engine table (e.g. 1.00, 0.98, 1.02, 1.12, 1.20, 1.08, 0.85 at Mach 0, 0.5, 0.9, 1.2, 1.6, 2.0, 2.4). Shipped engines carry full 2-D decks (`Mach × altitude`, separate dry and afterburner tables).

`η_inlet = 1 − 0.5·s²` with `s = clamp((|α| − 15°)/25°, 0, 1)` (inlet distortion at high α); `η_damage` from §18.

### 11.8.2 Throttle mapping and afterburner

Throttle input `t ∈ [0,1]`: `[0, 0.8]` maps idle→military (dry) thrust linearly; a detent at 0.8; `(0.8, 1.0]` selects afterburner with 4 stages. Engine thrust follows the command through a spool model:

```text
F' = (F_cmd − F) / τ,  τ = τ_up (1.8 s) if F_cmd > F, else τ_down (1.2 s)
AB light delay 0.6 s; AB stage time constant 0.5 s; AB blow-out when T < min_ab_press
```

### 11.8.3 Fuel flow

`ṁ_fuel = TSFC · F_net` with `TSFC = TSFC_ref · √(T/288.15)` (reference: dry 1.98×10⁻⁵ kg/(N·s) ≈ 0.70 lb/(lbf·h); afterburner 5.38×10⁻⁵ kg/(N·s) ≈ 1.9 lb/(lbf·h)). For the reference fighter (79 kN dry, 128 kN afterburner at sea level) this is ≈ 1.57 kg/s dry maximum and ≈ 6.89 kg/s in full afterburner, so 3 100 kg of internal fuel lasts ≈ 33 min at full dry thrust or ≈ 7.5 min in full afterburner.

### 11.8.4 Failure and edge conditions

| Condition | Effect |
|---|---|
| Fuel exhausted / feed interrupted | Flameout (thrust spools down τ = 1.2 s) |
| Sustained n_z < −0.5 g for > 8 s without negative-G sump | Fuel starvation flameout (data-driven per aircraft) |
| α > 45° at low speed | Compressor stall: thrust × 0.3 until α < 30° and relight (3 s) |
| Altitude/Mach outside relight envelope | Relight impossible |
| Heavy rain/ingestion (weather preset) | Flameout chance per second proportional to ingestion factor (`P10`) |

Piston/turboprop and ramjet engine classes are supported via the same engine interface (`EngineModel` trait) with different decks; lift-fan/VTOL is out of scope.

## 11.9 Mass properties

```text
m   = m_empty + Σ m_fuel,i + Σ m_store,j
r_cg = (m_empty r_empty + Σ m_fuel,i r_i + Σ m_store,j r_j) / m
I   = I_empty + Σ [ I_k + m_k (|d_k|² E − d_k d_kᵀ) ],   d_k = r_k − r_cg, E = identity
```

Tanks have capacity, feed priority, and a position function of fill (a small polynomial). Stores contribute point masses with their own inertia. Recomputed at 10 Hz or whenever stores/fuel change by > 0.5 %; interpolated in between. The reference fighter (Appendix C) uses `I ≈ (13 000, 76 000, 86 000) kg·m²` with `I_xz ≈ 1 300 kg·m²` at 10 000 kg.

## 11.10 Numerical integration

**Tier 0 (RK4, 120 Hz).** The state `(p, v, q, ω, m)` is advanced with classical RK4, holding controls and actuator positions constant over a sub-step (zero-order hold). The quaternion is re-normalised after each sub-step.

```rust
fn step_rk4(s: &State, dt: f64, ctl: &Controls, env: &Env) -> State {
    let k1 = deriv(s, ctl, env);
    let k2 = deriv(&s.add_scaled(&k1, dt * 0.5), ctl, env);
    let k3 = deriv(&s.add_scaled(&k2, dt * 0.5), ctl, env);
    let k4 = deriv(&s.add_scaled(&k3, dt), ctl, env);
    let mut n = s.add_weighted(&[k1, k2, k3, k4], dt, [1.0, 2.0, 2.0, 1.0], 1.0/6.0);
    n.q = n.q.normalize();
    n
}
```

**Tier 1 (semi-implicit Euler, 60 Hz).**

```text
ω ← ω + I⁻¹ (M − ω × Iω) Δt
q ← q ⊗ exp(½ ω Δt)        exp(θ⃗/2) = (cos(|θ|/2), sin(|θ|/2) θ⃗/|θ|),  θ⃗ = ω Δt
v ← v + (R(q) F/m + g) Δt
p ← p + v Δt                 (uses the updated velocity)
```

Stability: all fast loops (actuators, FCS inner loops, damping) have bandwidth below 15 rad/s, below one tenth of the 120 Hz Nyquist rate (≈ 377 rad/s). The integrator rejects NaN/Inf states: on detection it rolls back one step, logs `FDM_NAN` with the aircraft id, clamps controls, and if the second attempt also fails, destroys the aircraft with cause `numerical_fault` (never crashes the sim).

## 11.11 Flight control system and assist levels

### 11.11.1 Pipeline

```text
Pilot input (stick, pedals, throttle, trim, buttons)
   → input shaping (§58.2: deadzone, expo, saturation)
   → assist layer (Full / Standard / Reduced / Manual)
   → FCS control laws (loops, limiters, schedules)
   → surface commands → actuators (§11.7) → aerodynamics
```

### 11.11.2 Control law structure (Standard Assist)

| Axis | Command | Loop |
|---|---|---|
| Pitch | Stick → load factor command `n_cmd` | `n_cmd = 1 + s(n_max − 1)` for `s ≥ 0`; `n_cmd = 1 + s(1 − n_min)` for `s < 0` (s ∈ [−1,1]; `n_min` e.g. −3); PI on `n` error + pitch-rate damping + α feedback; feed-forward trim `δ_e,trim(M, H, n)` |
| Roll | Stick → stability-axis roll rate `p_s,cmd = s · p_max(q̄, config, α)` | PI + damping; roll-rate limit shrinks with α and g |
| Yaw | Pedals → sideslip target; zero pedal → coordinated flight | `δ_r = K_β(β_cmd − β) + K_r r + K_ay a_y` (a_y = lateral acceleration) |
| Throttle | Direct, optional auto-throttle (speed hold) | Autothrottle PI on CAS |

Gains are scheduled on `q̄` and Mach (2-D tables in the aircraft file) and are produced by the offline autotuner (§11.18). Limiters:

| Limiter | Behaviour |
|---|---|
| α limiter | `α_cmd ≤ α_lim` (soft limit ~ stall α − 2°); beyond, pitch authority scales to zero (no departure) |
| g limiter | `n_cmd ∈ [n_min, n_max(config)]`; stores reduce `n_max` per loadout (§14.4) |
| Roll-rate limiter | Reduces `p_max` at high α and high g to protect against inertial coupling |
| Overspeed | Pitch-up bias near Vne/Mne (soft "wall") |
| Ground collision avoidance (opt-in) | Auto-GCAS: when predicted terrain contact < 4 s ahead at current g-capability, takes control briefly and rolls wings level + pulls up |

*Clarified in 0.2.2 (ADR-34):* the loops above are built as **incremental dynamic inversion** and not as scheduled PIs. The control system asks for an angular acceleration and the surfaces move by the inverse of the airframe's own control power (the tables it flies on) applied to the difference from the acceleration it measures, so no gain schedule over `q̄` is needed. The classical structure is still visible: a load-factor loop (a PI on `n` whose integral collects only small errors, closing `n` as a first-order lag at every dynamic pressure), pitch-rate damping, an angle-of-attack limiter that acts in a 3° band below the limit and looks 0.4 s ahead along the true α rate, and the trim feed-forward that the inversion implies. Stick centre is 1 g, as the formulas read. The gains are scalar data (`fcs/1`, A.1) with schedules for the roll-rate authority only; the overspeed "wall" is a load-factor bias from 0.92 of `V_mo`/`M_mo`. The autotuner of §11.18, the auto-throttle and Auto-GCAS are not built in v0.1b; the handling suite of ADR-33 is the gate for the gains, and the timing of Auto-GCAS is noted at §54.9.

### 11.11.3 Assist presets (outline §107)

| Preset | Pitch | Roll | Yaw | Limiters | Intended player |
|---|---|---|---|---|---|
| **Full Assist** | Flight-path-rate command; stick centred holds current flight-path angle | Bank-angle command; stick centred levels wings below 60° | Auto | α, g, overspeed, stall, auto-GCAS, unusual-attitude recovery button | New players |
| **Standard Assist** (default) | g-command | Roll-rate command | Auto-coordination | α, g, overspeed | General |
| **Reduced Assist** | Pitch-rate command | Roll-rate command | Yaw damper only | g soft (warning + structural risk), no α hard limiter (departure possible, warnings) | Experienced |
| **Manual** | Direct stabilator/aileron/rudder via actuators | — | — | None, except an inner-loop stability augmentation for statically unstable airframes (these cannot be flown without it; the tuning file marks `sas_required: true`) | Simulator |

Mouse-aim control (a War-Thunder-style "fly to cursor") sits above the assist layer: it converts the aim error into stick commands (§58.4) and is available in Full and Standard.

*Clarified in 0.2.2 (ADR-34):* **Full Assist is a bank-angle command**: the stick sideways commands a bank up to 60° and centred levels the wings; the stick back commands a flight-path rate (at most 10°/s) and centred holds the path. Its angle-of-attack limit is 2° tighter than Standard's, and in G-LOC it holds the wings level and a level or climbing path. The recovery button (`U`, held) rolls the wings level the short way and pulls to a 5° climb at 4 g in Full and Standard; in Reduced and Manual it flies anti-spin controls only if the aircraft has departed (a stability-axis yaw rate above 30°/s, or α above 35°). Switching the level in flight is bumpless (the surface command blends over 0.5 s). Manual is the stick moving the surfaces about the trim; the trim keys move the elevator in Manual only, as the other laws trim themselves. The Kestrel's `sas_required` is false.


## 11.12 Energy management

The model makes energy tangible:

```text
E_s = H + V²/(2g)                                  specific energy (m)
P_s = (F_net − D) V / (m g)                         specific excess power (m/s)
n_sust: solves F_net = D(n) with D(n) = q̄ S [C_D0 + C_Dw + K (n m g/(q̄ S))²]
ω_turn = g √(n² − 1) / V       r_turn = V² / (g √(n² − 1))
```

Corner speed is where `n` limited by `C_L,max` equals structural `n_max`. Consequences the player experiences: high-G turns bleed speed; zoom climbs trade speed for altitude; low energy means low turn authority; heavy loads reduce sustained g. The optional energy HUD cue (§54.7) shows `P_s` as a caret; baked **energy-maneuverability tables** (`P_s` over Mach × altitude × n, turn rate, radius) per aircraft/loadout are used by AI decision-making (§19.5).

## 11.13 Ground handling

### 11.13.1 Gear model

Each wheel assembly `i` has a contact point in BODY and a strut. Per wheel per sub-step:

1. Transform contact point to BF; query `terrain_height` (runway-flattened, §10.7) and surface class.
2. `penetration x = max(0, h_terrain − h_wheel_bottom)` along the surface normal; strut rate `ẋ` from point velocity.
3. Normal force `N = max(0, k_i (1 + (x/x_max)²) x + c_i(ẋ) ẋ)` (progressive spring; `c` is asymmetric: more damping on extension than compression); if `x > x_max` the strut bottoms out and the excess goes to the airframe (damage).
4. Tangential forces from friction (below); applied at the contact point, producing moments about the CG.

### 11.13.2 Friction

| Force | Model |
|---|---|
| Rolling | `F_roll = −μ_r N sign(v_long)` |
| Braking | `F_brake = −μ_b(surface, wetness) · brake_cmd · N · sign(v_long)`; with anti-skid on, effective `μ_b` scaled 0.9; off and brake > 0.8 at speeds > 5 m/s → lock-up factor 0.6 |
| Lateral | `F_lat = −μ_lat N · tanh(v_lat / v_s)`, `v_s = 0.5 m/s` (smooth Coulomb); steering angle rotates the wheel's axes |
| Nose-wheel steering | ±60° below 15 m/s, scheduled down to ±5° above 30 m/s; rudder blends in above 40 m/s |

Surface friction coefficients: §10.4. Brake heat and tyre burst are optional (`P10`).

*Clarified in 0.2.2 (ADR-35):* a sharp `sign(v)` chatters at rest and makes a parked aircraft creep, so every tangential term is regularised: rolling resistance and braking are `−μ N tanh(v / v_s)` with `v_s` 0.2 m/s and 0.05 m/s, the lateral force keeps the 0.5 m/s of the table, and rolling and braking **add** along the wheel. A friction circle of `μ_peak N` limits the tangential vector, and the sideways peak is the surface's braking coefficient. Steering is positive for the nose wheel turned right. The anti-skid factor (0.9), the lock-up factor (0.6) and the surface table are as written.


### 11.13.3 Touchdown and crash criteria (design targets)

| Vertical speed at gear contact | Result |
|---|---|
| ≤ 3.0 m/s | Normal |
| 3.0–4.5 m/s | Hard landing: strut/gear damage roll (§18.8) |
| > 4.5 m/s | Gear collapse; fuselage contact |
| > 7.0 m/s, or fuselage/wing/stores contact at > 30 m/s | Destroyed |

Carrier-arrested landings and catapult launches add external forces from the ship systems (§22.6): catapult as a constant-acceleration force along the deck (e.g. 85 m/s in 94 m ≈ 3.9 g, 2.2 s), arrestor wire as a force-vs-runout curve (e.g. 70 m/s stopped in ~100 m ≈ 2.5 g).

*Clarified in 0.2.2 (ADR-35):* the table is read as follows. The vertical speed is the speed into the ground of the **first contact of a touchdown** (a leg that arrives later in the same landing, as the nose wheel does, is not judged a second hard landing, and a touchdown re-arms after 0.5 s in the air). Up to 3.0 m/s is normal; 3.0 to 4.5 m/s is a hard landing (counted, flagged for 3 s); 4.5 to 7.0 m/s collapses **that strut**, after which the airframe rests on it; above 7.0 m/s the aircraft is destroyed. Airframe contact (tail, wing tip, nose, belly) at a ground speed above 30 m/s, or a collapsed strut at that speed, destroys it; below that the aircraft skids at μ 0.5 and survives. A gear that is in transit when it meets the ground collapses. A CG more than 5 m below the ground is reported as a numerical failure, not a crash. §18.8.1 words the speed rule differently (normal speed component); **§11.13.3 governs until the damage chapter (§18) is built.**

### 11.13.4 Weight on wheels

`WoW` is true when any main-gear `N > 0` and ground speed < 120 m/s; it switches flight-control gains, enables nose-wheel steering, disables certain sensors (radar standby), and enables parking brake.

*Clarified in 0.2.2 (ADR-34):* the 0.1 s debounce belongs to the control system; the flight model reports the raw state. The control system switches every law but Manual to the ground law on weight on wheels, blending over 0.5 s at touchdown and 2.0 s after lift-off.

## 11.14 G-forces and pilot physiology

Load factor along the pilot's head-to-toe axis is `n_z = −a_z,body/g + …` (specific force along −z_body in g units). Effects are optional per realism (§59) and accessibility (§57).

```text
G_f' = (n_z − G_f) / τ_f,   τ_f = 2.0 s         lagged "heart-level" G (blood-pressure lag)
G_tol = G_base + G_suit + G_agsm − 0.4·F         base 4.5; suit +1.5; AGSM skill 0…+1.5 (player setting, default 0); fatigue F ∈ [0,1]
dS/dt = k_g (G_f − G_tol)/G_tol         for G_f > G_tol, k_g = 0.25 s⁻¹
dS/dt = −S / τ_rec                       otherwise, τ_rec = 4 s
dF/dt = +0.01 s⁻¹ when n_z > 5, −0.005 s⁻¹ otherwise
```

| Stress S | Effect (default strength 100 %) |
|---|---|
| 0.35 – 0.7 | Greyout/tunnel vision: vignette radius 100 % → 30 %, desaturation |
| 0.7 – 1.0 | Blackout: screen black, controls retained with 300 ms input delay |
| ≥ 1.0 | G-LOC: controls zeroed 6 s (+4 s disorientation); auto-recover prompt if equipped |

Negative G: `dR/dt = (−1 − n_z)/τ_r` for `n_z < −1` (`τ_r = 3 s`): red tint at `R ≥ 0.5`, vision loss at `R ≥ 1.0`.

Because `G_f` lags `n_z`, brief 9 g pulls are survivable while sustained pulls are not — a deliberate skill expression. Accessibility: effect strength 0–100 %, "never lose control" toggle (visual only), reduced screen shake (§57).

*Clarified in 0.2.2 (ADR-38):* `n_z` is the accelerometer value at the CG; the rotational terms at the pilot's station (a pilot 5 m ahead of the CG feels `q̇` times the arm) are a recorded simplification. `S` integrates the **mean** `G_f` over a tick, which is exact for a held `n_z` and independent of the step. A 9 g pull from 1 g reaches `S` = 0.7 at 9.50 s and 1.0 at 11.93 s with fatigue frozen at 0 (AC-11.9's bands are 9.6 ± 0.5 and 12.0 ± 0.5), and at 9.42 s and 11.80 s with fatigue building; a 3 s pull peaks at 0.040. The accessibility choice is a three-way dial: **Full** (default) applies the grey-out, blackout (controls 300 ms late, 18 ticks) and G-LOC (zeroed 6 s, then a 4 s linear return); **Reduced** keeps the visuals and adds 1.5 g of tolerance, with no loss of control; **Off** applies nothing. The state advances in every mode. The strength of the picture (0 to 100 %) is the renderer's local setting. G-LOC cannot re-trigger while it runs and does again if the pilot is still above tolerance when it ends. The dial is a session setting, applied at the next start.

## 11.15 Stall, departure, and structural limits

- **Stall** emerges from the table (`C_L,max`) and §11.6.3. Warnings: shaker/audio tone at `α = α_stall − 3°`, HUD flash.
- **Departure criterion** (diagnostic): `C_nβ,dyn = C_nβ cosα − (I_z/I_x) C_lβ sinα`; negative values indicate directional divergence and are flagged by the tuning tools; the tables for Reduced/Manual intentionally preserve departure behaviour.
- **Spins** emerge from yaw–roll coupling in the extended tables; the "Recover" button commands anti-spin controls through the FCS where equipped.
- **Structural limits** per config: `n_max`, `n_min`, `V_ne`, `M_ne`, gear-extended speed (design target 154 m/s ≈ 300 kn), flap-extended speed. `|n| > 1.1 n_max` accumulates structural damage; `> 1.5 n_max` or `V > 1.2 V_ne` yields break-up chance per second (§18.7).

*Clarified in 0.2.2 (ADR-38):* the lateral and directional derivatives are tables against α, equal to the C.5 values up to 15° and shaped above it, so the airborne points of C.9 do not move. `C_nβ,dyn` and the lateral control departure parameter are computed from them (the Kestrel's `C_nβ,dyn` turns negative at α = 34.05°, 8° above the 26° limiter, and the aileron loses its authority from about 25°). Over-stress (`|n|` above 1.1 of the configuration's limit; 4 g with the gear or flaps out), over-speed, and gear and flap overspeed are timed and raise warnings; **nothing breaks** before the damage chapter (§18.7).

## 11.16 Fuel, range, and aerial refuelling hooks

Fuel is tracked per tank; feed order protects CG. Drop tanks can be jettisoned (restoring drag and mass). Aerial refuelling (probe-drogue and boom) is a docking mini-state-machine in the Sim Core (§49.5): relative-position envelope, contact detection (probe tip within 0.3 m of the drogue basket centre, closing speed < 2 m/s), then flow at a data rate (design targets: 20 kg/s probe-drogue, 45 kg/s boom). Bingo and joker fuel (AI and radio, §19.7) derive from range calculations on `TSFC` and `P_s` tables.

## 11.17 Reduced-order models (Tiers 2 and 3)

**Tier 2 (30 Hz): point-mass with commanded load factor and bank.** A 3-DOF vector formulation with first-order roll-rate lag; uses the same polar and engine deck as Tier 0/1:

```text
v̂ = v / |v|;  ê_z = normalize(up − (up·v̂) v̂);  r̂ = v̂ × ê_z
l̂ = cosμ ê_z + sinμ r̂                                   lift direction for bank μ
L = n m g  (limited to q̄ S C_L,max and n_max);  D from the polar at C_L = L/(q̄ S)
v̇ = ((F_net cosα − D)/m) v̂ + (L/m) l̂ + g(p)
μ̇ = clamp((μ_cmd − μ)/τ_roll, −μ̇_max, +μ̇_max)
```

Attitude is derived (nose along `v̂` rotated by α about the wing axis; wings banked μ), so HUD and visuals of distant aircraft look right.

**Tier 3 (10 Hz): energy model.** `dE_s/dt = P_s(M, H, n_cmd)`; speed follows `E_s` at fixed altitude change rate; heading changes at `ω_turn(n_cmd)` limited by baked tables. No ground collision (terrain avoidance is handled by the AI's route planner and a 20 s look-ahead clearance check against the base DEM).

**Parity requirement.** For the same commands, Tier 2 sustained turn rate and `P_s` must be within 5 % of Tier 0 for the reference fighter (`AC-11.8`). Tier changes are continuous (§0.6.1).

## 11.18 Authoring and tuning workflow

Tools (native Rust CLI plus a browser-based "FDM Lab" page sharing the same crate):

1. **Trim solver:** Newton iteration on `(α, δ_e, throttle)` for steady level flight, constant-g turns, and glide.
2. **Linearisation:** central-difference Jacobians `A, B` about a trim; eigenvalue report for short-period, phugoid, Dutch roll, roll, spiral modes.
3. **Handling-quality targets** (derived from MIL-F-8785C Level 1 for highly manoeuvrable aircraft; implementers MUST verify against the cited standard): short-period damping ratio 0.35–1.3; phugoid damping ≥ 0.04; Dutch roll damping ≥ 0.19, `ζω ≥ 0.35 rad/s`, `ω_n ≥ 1.0 rad/s`; roll mode time constant ≤ 1.0 s; spiral time-to-double ≥ 12 s (open-loop; closed-loop FBW dominates).
4. **FCS autotuner:** optimises scheduled gains to step-response targets (overshoot < 10 %, 90 % settling < 1.5 s, no limit-cycle).
5. **Flight-test harness:** scripted manoeuvres (level acceleration, wind-up turns, rolling pulls, stall approaches, take-off/landing runs, zoom climbs, dive recovery) writing CSV and plots, comparing to golden runs.
6. **Envelope report:** `P_s` contours, `V–n` diagram, turn-performance charts, range/endurance charts.
7. **Lint:** coefficient monotonicity, unit sanity, symmetry, completeness (`C_L,max` for every Mach row, etc.).

## 11.19 Performance Conformance Test

Each aircraft file includes an `envelope` block (authored target performance). The test flies the actual 6-DOF model through scripted procedures and compares:

| Metric | Tolerance |
|---|---|
| Maximum level Mach at listed altitudes and weights (dry/AB) | ±3 % |
| Maximum rate of climb and best-climb speed | ±3 % |
| 1 g stall speed (by configuration) | ±3 % |
| Fuel flow at listed points | ±3 % |
| Sustained load factor / turn rate at listed points | ±5 % |
| Take-off and landing ground roll | ±5 % |
| Corner speed | ±5 % |

*Clarified in 0.2.1 (ADR-31):* the procedures, cross-checks and gates are fixed in ADR-31. Sustained
load factor is the **wind-axis** load factor of a level turn, `n = √(1 + (V·ω/g₀)²)`, not the
body-axis accelerometer reading `n_z`, which is lower by the thrust's lift component (about 6.0
against 6.4 at M 0.5). Points flown at a given speed carry it as data (`mach`). Points that need a
model which does not exist yet (`requires: "gear"`) are reported as deferred, and the gate checks
that the deferred set is exactly the expected one.

It runs headlessly in CI (native build) and in the browser FDM Lab. The reference fighter's authored envelope (Appendix C, e.g. clean 9 500 kg: maximum level Mach 1.22 at sea level, 1.83 at 11 km; 1 g stall speed 58.3 m/s ≈ 113 kn at sea level; maximum climb rate ≈ 283 m/s at Mach 0.83) is the first acceptance target.

## 11.20 Edge cases

| Case | Handling |
|---|---|
| α, β undefined at `V_T < 1 m/s` | Use body-axis force model only; angles clamped to 0 |
| Tier promotion mid-manoeuvre | State copy + attitude reconstruction (§0.6.1) |
| Very high altitude (> 25 km) | Aerodynamic control authority vanishes naturally; hard ceiling at 30 km (§8.4) |
| Negative fuel | Clamped to 0; flameout |
| Time acceleration | Same Δt, more ticks per wall second; HUD updates at render rate |
| Frame-rate drops | Sim runs at its own rate; render interpolates (§33.6 reuse of the interpolation buffer) |
| Large load-factor commands near `n_max` | Soft saturation via FCS limiter, never integrator windup (anti-windup back-calculation) |

## 11.21 Acceptance criteria

- `AC-11.1` Trim solver finds level-flight trim at 50 test points across the envelope with residual acceleration < 0.01 m/s².
- `AC-11.2` Linearised modes meet §11.18 targets at five representative flight conditions.
- `AC-11.3` Performance Conformance Test passes for the reference aircraft within §11.19 tolerances.
- `AC-11.4` Energy conservation: a frictionless-drag, zero-thrust test of a ballistic zoom conserves `E_s` within 0.5 % over 30 s.
- `AC-11.5` ISA reference table (§11.3) reproduced within 0.05 % by the implementation.
- `AC-11.6` Quaternion norm error < 1e-12 after 10⁶ steps; no NaN in a 10-minute random-input fuzz run for each assist preset.
- `AC-11.7` WASM and native builds produce identical state hashes for a 60 s scripted flight (ADR-10).
- `AC-11.8` Tier 2 and Tier 3 parity (§11.17) holds.
- `AC-11.9` G-model with default tolerance (`G_tol` = 6.0 g, fatigue 0): a sustained 9 g pull (from 1 g) reaches blackout (S = 0.7) at 9.6 s ± 0.5 s and G-LOC (S = 1.0) at 12.0 s ± 0.5 s; a 3 s 9 g pull peaks at S < 0.05 and causes no blackout. (Hand-integrated from the §11.14 equations with `G_f` lagging `n_z` by τ = 2 s.)
- `AC-11.10` Player-facing: in blinded playtests, ≥ 70 % of experienced flight-sim players rate Standard Assist "responsive and believable" (≥ 4/5).

---

# 12. AIRCRAFT SYSTEM

## 12.1 Intent

Aircraft are **modular, data-driven entities** (outline §18). No aircraft is special-cased in code (NN-11). Adding an aircraft means adding a data package (JSON + binary tables + glTF assets), not changing the engine.

## 12.2 Composition of an aircraft definition

An aircraft package is a set of components, each independently versioned and re-usable by variants.

| Component | Contains | Consumed by |
|---|---|---|
| `identity` | id, display name, role, category (§13), era, manufacturer (fictional), country-neutral faction tags, tags (`carrier`, `stol`, `crew=2`) | UI, generator, balance lint |
| `geometry` | S, b, c̄, reference points, gear geometry, bounding box | FDM, collision, UI |
| `mass` | empty mass, empty CG and inertia, tank list, fuel density (JP-8 ≈ 0.80 kg/L), payload limits (MTOW, MLW) | FDM, loadout |
| `aero` | coefficient tables (§11.6), flap/gear/airbrake deltas | FDM |
| `propulsion` | engines (count, positions), decks (dry, AB), spool constants, TSFC, inlet model | FDM |
| `fcs` | control-law gains and schedules, limiter values, SAS requirement flag | FCS |
| `limits` | n_max, n_min per configuration, Vne, Mne, gear/flap limit speeds, ceiling | FCS, damage |
| `hardpoints` | stations (§12.4) | Loadout, FDM, RCS |
| `sensors` | radar, IRST, RWR, MAWS, targeting pod slots referencing the sensor library (§16) | Sensors |
| `avionics` | HUD/MFD layout, available radar modes, datalink, fidelity tier | UI, sensors |
| `countermeasures` | dispenser positions and capacity, programs | CM system |
| `signatures` | RCS table (az × el, dBsm), IR signature (engine, plume, skin), acoustic profile | Sensors, audio |
| `damage` | hit volumes, components, HP/armour, fire/leak rules (§18) | Damage |
| `cockpit` | cockpit scene, instrument bindings, eye-point, camera limits | Rendering |
| `visual` | LOD chain, material slots, animation nodes, livery mask, decal layers | Rendering |
| `audio` | engine loops by RPM/throttle, cockpit sounds, external signature | Audio |
| `ai` | AI hints: preferred speeds, energy-managment tables ref, weapon doctrine | AI |
| `envelope` | authored target performance (§11.19) | Tests |
| `loadouts` | role presets | UI |

## 12.3 File layout and inheritance

```text
content/aircraft/kestrel/
  aircraft.json        main manifest (schema: aircraft/1)
  aero.bin             packed coefficient tables (built from aero/*.csv)
  engines/f129.json    engine definition & decks
  rcs.bin              RCS table
  model/kestrel.glb    glTF 2.0 (LOD0–3, cockpit separate)
  textures/*.ktx2      Basis-compressed textures
  audio/*.ogg|opus     sound assets
  loadouts/*.json
  README.md            authoring notes
```

**Inheritance.** `"extends": "kestrel"` creates a variant (e.g. `kestrel_b`): the variant JSON patches fields with JSON-Merge-Patch semantics; array fields can be replaced or appended with explicit operators (`"$append"`). Resolved definitions are produced at bake time; the runtime never resolves inheritance.

**Versioning.** Each package declares `schema` and `content_version` (semver). The Sim Core refuses to load a package whose major schema version it does not support; minor increments are backward compatible.

## 12.4 Hardpoints

```json
{ "id": "wing_l_2", "station": 2, "pos_body_m": [-0.5, -3.1, 0.35], "dir_body": [1, 0, 0],
  "mount": ["rail_aim9", "pylon_2000lb", "tank_wing"], "max_mass_kg": 1500, "sym": "wing_r_2",
  "drag_when_empty_cd0": 0.0004, "rcs_when_empty_m2": 0.05, "clearance": { "gear": true, "flap": false } }
```

| Field | Meaning |
|---|---|
| `mount` | Accepted mount types; a weapon declares the mount types it needs |
| `max_mass_kg` | Per-station structural mass limit |
| `sym` | Symmetric partner station (for balanced loading rules) |
| `drag_when_empty_*`, `rcs_when_empty_*` | Pylon/rail penalty remaining when the station is empty but fitted |
| `bay` | Internal weapon bay id, door opening time (default 0.8 s), RCS/drag penalty while open |
| `interference` | List of station ids that cannot be loaded simultaneously with certain items (e.g. large bomb on `stn_4` blocks `stn_3` AAM) |
| `launch` | Release geometry: eject velocity, position offset, canted direction (for safe separation) |

Mount classes: `rail` (launch rail), `pylon` (bomb/rocket/tank), `ejector` (bays), `centerline`, `conformal`, `internal_gun`.

## 12.5 Runtime entity components

```rust
pub struct Aircraft {
    pub id: EntityId, pub def: AircraftDefId, pub owner: OwnerId, pub faction: FactionId, pub callsign: CallsignId,
    pub tier: SimTier,
    pub kin: Kinematics,            // p, v, q, ω (§11.2)
    pub mass: MassState,            // m, r_cg, I, fuel[], stores[]
    pub ctl: ControlState,          // commands, assist level, trim
    pub act: ActuatorState,
    pub eng: Vec<EngineState>,
    pub gear: GearState,
    pub sensors: SensorSuite,       // radar, rwr, irst, maws state
    pub cm: CountermeasureState,
    pub wpn: WeaponStations,        // per station: item, count, status
    pub dmg: DamageState,           // component HP/flags (§18)
    pub ai: Option<PilotAi>,        // None for humans
    pub net: NetState,              // replication info (§33)
}
```

Storage is struct-of-arrays in the ECS-style world so hot loops (FDM, sensors) touch contiguous memory.

## 12.6 Asset pipeline and budgets (browser)

| Item | Requirement |
|---|---|
| Format | glTF 2.0 binary; KTX2 (Basis Universal UASTC/ETC1S) textures transcoded to the GPU's native block format at load; meshes quantised and meshopt-compressed |
| Node naming | `ctrl_stab_l/r`, `ctrl_ail_l/r`, `ctrl_rud`, `ctrl_lef_*`, `gear_nose/main_l/main_r`, `abrake`, `canopy`, `hook`, `probe`, `bay_*`, `hp_*` (hardpoint attach nodes), `eng_*_nozzle`, `hit_*` (hit-proxy meshes, not rendered) |
| Animation | Rigid node transforms driven by the sim (no skinning); canopy, gear, flaps, surfaces |
| LOD chain | LOD0 ≤ 60 k tris, LOD1 ≤ 20 k, LOD2 ≤ 6 k, LOD3 ≤ 1.5 k, impostor sprite sheet (16 azimuths × 3 elevations) beyond 12 km |
| Draw calls | LOD0 ≤ 40; LOD2+ instanced |
| Textures | Aircraft atlas 2 K (albedo, normal, ORM) per aircraft, cockpit 2 K, shared detail maps |
| Package size | ≤ 12 MB compressed per aircraft (including cockpit), lazy-loaded when selected or spawned within 40 km |
| Livery | Mask-based recolouring in the shader (§14.7); no per-livery full texture |

## 12.7 Validation (content linter)

Run in CI and in the in-game content loader (hard errors block load; warnings are listed):

- Mass: `MTOW ≥ empty + max fuel + max payload` is *not* required (aircraft can be limited), but `empty + full internal fuel ≤ MTOW` is.
- T/W and wing loading within category ranges (§13.1) or flagged `outlier_ok`.
- Every `mount` referenced by a weapon exists; symmetric stations exist and are mutually consistent.
- Coefficient tables: monotone, no NaN, α-range coverage, `C_L,max` for all Mach rows, symmetry where required.
- Static margin at ref weight within [−5 %, +15 %] c̄ (negative requires `sas_required`).
- `envelope` reachable by the FDM (headless run) — the conformance test (§11.19).
- Asset: all named nodes exist; triangle/texture budgets (§12.6) respected.
- Balance: ParetoLint (§13.2).

## 12.8 Acceptance criteria

- `AC-12.1` Adding the `kestrel_b` variant by JSON patch alone (no code change) yields a flyable aircraft with different stats.
- `AC-12.2` A package with a missing mount type fails with a message naming the hardpoint and weapon.
- `AC-12.3` The reference aircraft loads (data → ready to fly) in ≤ 1.5 s cached on the Recommended profile.
- `AC-12.4` Removing all aircraft packages except one leaves the game launchable (no hard-coded aircraft ids anywhere; verified by a build-time grep lint).

---

# 13. AIRCRAFT CATEGORIES

## 13.1 Category definitions

The outline's twelve initial categories. The numeric ranges are *design envelopes* used by the content linter and as authoring guides; individual aircraft may exceed them with an `outlier_ok` justification. Weights are at the category's reference combat weight (empty + 60 % internal fuel + the role's standard air-to-air fit).

| Category | Role / identity | T/W (ref wt) | Wing loading (kg/m²) | Max Mach (clean) | n_max | Radar class | Typical payload |
|---|---|---|---|---|---|---|---|
| **Light Fighter** | Agile dogfighter; short range; cheap | 1.1–1.3 | 330–420 | 1.6–2.0 | 9 | Medium | 3 000–5 000 kg |
| **Multirole Fighter** | A2A and A2G balance; versatile | 1.0–1.2 | 380–450 | 1.8–2.1 | 9 | High | 5 000–8 000 kg |
| **Heavy Fighter** | Large radar, large weapon load, long range; poor acceleration | 0.95–1.15 | 400–480 | 2.0–2.3 | 8–8.5 | Very high | 7 000–10 000 kg |
| **Interceptor** | Extremely fast, excellent climb, weaker manoeuvre, high fuel burn | 1.05–1.25 | 380–450 | 2.4–3.0 | 6–7 | High | 2 000–4 000 kg (long-range AAMs) |
| **Strike Fighter** | Excellent ground attack, moderate A2A | 0.85–1.05 | 380–450 | 1.4–1.8 | 7.5–8.5 | Medium (strong A2G modes) | 6 000–9 000 kg |
| **Close Air Support** | Slow, armoured, heavy cannon, long loiter | 0.5–0.7 | 280–340 | 0.55–0.8 | 5.5–6.5 | Low/none | 6 000–8 000 kg |
| **Bomber** | Large payload, long range; low agility | 0.3–0.5 | 330–420 | 0.8–1.4 | 2–3.5 | Medium (nav/strike) | 15 000–30 000 kg |
| **Electronic Warfare Aircraft** | Jammers, ESM, SEAD enabler; fighter-class or business-jet-class airframe | 0.8–1.1 | 380–450 | 1.4–1.9 | 6–7 | Medium + EW suite | EW pods 1 000–3 000 kg |
| **AWACS** | Airborne radar picture; command; vulnerable | 0.25–0.45 | 330–420 | 0.7–0.85 | 2–2.5 | Surveillance (very long range, low accuracy) | — |
| **Carrier Aircraft** | Robust gear, hook, folding wings, good low-speed handling | 1.0–1.2 | 360–430 | 1.6–2.0 | 7–7.5 | High | 5 000–8 000 kg |
| **Stealth Aircraft** | Very low RCS, internal bays, high-end sensors, limited external load | 1.1–1.35 | 280–350 | 1.8–2.2 (supercruise ~1.5) | 9 | Very high (LPI modes) | 4 AAM internal; 2 000 kg bombs internal |
| **Experimental Aircraft** | Unusual capability, high skill ceiling (TVC, extreme speed, etc.) | any | any | any | up to 12 | Varies | Limited |

Support (non-player-default) classes needed by missions: **Tanker** (boom/drogue), **Transport** (convoy escort and airlift missions), **Reconnaissance UAV** (optional player aircraft; no G limits, low signature). Rotary-wing is out of scope; the aircraft interface (`FlightModel`, `Aircraft` components) is general enough to host it later.

## 13.2 Balance rules

**R1 – No strictly dominant aircraft.** Each aircraft has a stat vector of eight normalised axes: *instantaneous turn*, *sustained turn / P_s*, *top speed*, *climb*, *sensor power*, *payload*, *range/endurance*, *survivability + stealth*. **ParetoLint** compares aircraft within the same era tier:

- No aircraft may be ≥ another on all axes and > on any (strict Pareto dominance) unless the dominated aircraft is flagged `legacy` or `training` and is therefore excluded from the "same-cost" comparison.
- Each aircraft must rank in the top 25 % of its era tier on at least one axis and the bottom 25 % on at least one axis (identity and cost).

**R2 – Cost parity via threat value.** Every aircraft+loadout has a **Threat Value (TV)** used by the mission generator to size forces (§26.8). `TV = w_a2a·A + w_a2g·G + w_sens·S + w_surv·V + w_range·R`, each term derived from stats and the loadout; weights live in data. Parity means force budgets are comparable; it is *not* a matchmaking system.

**R3 – Counter-play exists.** Every category has at least one natural counter and one natural prey (§13.7).

**R4 – Era coherence.** An era tier defines the sensor/weapon generation available (§13.6). A mission uses one era per side by default (mixed-era options exist for sandbox).

## 13.3 Archetype specifications (from outline §20)

| Archetype | Required traits | Forbidden combination |
|---|---|---|
| **Fast Interceptor** | Max Mach ≥ 2.4; climb rate ≥ 1.3× reference fighter; n_max ≤ 7; fuel flow ≥ 1.2× reference; radar class ≥ High | Also best-in-class instantaneous turn |
| **Dogfighter** | Instantaneous and sustained turn in top 25 %; low wing loading; short range (internal fuel ≤ 3 500 kg or equivalent) | Also top-quartile range |
| **Heavy Fighter** | Radar and payload top quartile; T/W ≤ 1.15; poor acceleration at low altitude | Also top-quartile turn |
| **Strike Aircraft** | Air-to-ground payload/accuracy top quartile; A2A radar modes limited or fewer AAM stations | Also top-quartile A2A sensors |

## 13.4 Launch roster (working names; design targets)

Working names are placeholders pending an IP/naming review (ADR-11). All numbers are design targets (§0.2) for the data author; the first aircraft (**Kestrel**) is fully specified and validated in Appendix C. Reference weight = empty + 60 % internal fuel + standard AAM fit.

| # | Name | Category | Era | Crew | Empty (kg) | Int. fuel (kg) | Engines (SL thrust, AB) | Ref wt (kg) | T/W | S (m²) | WL (kg/m²) | Max Mach (clean) | n_max | Hardpoints |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Kestrel | Light Fighter | G4 | 1 | 8 400 | 3 100 | 1 × 128 kN | 10 500 | 1.24 | 28.0 | 375 | 1.83 @ 11 km | 9 | 9 |
| 2 | Peregrine | Multirole Fighter | G4.5 | 1 | 9 800 | 3 900 | 1 × 146 kN | 12 700 | 1.17 | 32.0 | 397 | 2.0 | 9 | 11 |
| 3 | Condor | Heavy Fighter | G4 | 2 (pilot + WSO) | 16 800 | 7 500 | 2 × 125 kN | 22 700 | 1.12 | 56.0 | 405 | 2.2 | 8.5 | 12 |
| 4 | Swift | Interceptor | G4 | 1 | 14 500 | 6 800 | 2 × 118 kN | 20 600 | 1.17 | 50.0 | 412 | 2.8 | 6.5 | 6 |
| 5 | Bittern | Strike Fighter | G4 | 1 | 11 800 | 4 500 | 1 × 135 kN | 15 300 | 0.90 | 38.0 | 403 | 1.6 | 8 | 11 |
| 6 | Badger | Close Air Support | G3 | 1 | 11 300 | 4 800 | 2 × 45 kN (no AB) | 14 800 | 0.62 | 47.0 | 315 | 0.65 | 6 | 11 |
| 7 | Albatross | Bomber | G3 | 3 | 38 000 | 24 000 | 4 × 60 kN (no AB) | 70 400 (incl. 18 t bombs) | 0.35 | 200 | 352 | 0.85 | 2.5 | 14 (bay + wing) |
| 8 | Magpie | EW Aircraft | G4 | 2 (pilot + EWO) | 15 200 | 6 000 | 2 × 90 kN | 20 300 | 0.90 | 50.0 | 406 | 1.8 | 6.5 | 8 |
| 9 | Argus | AWACS | G4 | 5+ (pilot, copilot, 3 ABM seats) | 78 000 | 40 000 | 4 × 98 kN (no AB) | 105 000 | 0.38 | 280 | 375 | 0.8 | 2.0 | 0 |
| 10 | Gannet | Carrier Aircraft | G4 | 1 | 14 000 | 6 000 | 2 × 98 kN | 18 000 | 1.11 | 46.0 | 391 | 1.8 | 7.5 | 10 |
| 11 | Wraith | Stealth Aircraft | G5 | 1 | 13 500 | 6 300 | 2 × 110 kN | 18 000 | 1.25 | 62.0 | 290 | 2.0 (supercruise 1.5) | 9 | 4 internal + 4 external |
| 12 | Chimera | Experimental | G5 | 1 | 9 800 | 2 800 | 2 × 105 kN, ±20° 2-D thrust vectoring | 11 800 | 1.82 | 34.0 | 347 | 2.5 | 12 | 2 + gun |

(T/W = sea-level AB thrust ÷ (ref wt × 9.80665); for aircraft without AB it is dry thrust. Kestrel's 1.24 is verified in Appendix C; the others are targets to be validated through §11.19 once authored. Support aircraft — Tanker "Stratus", Transport "Hauler", UAV "Scout" — are specified in Appendix D.)

## 13.5 Roles in multiplayer

Mapping of categories to co-op roles (§32.4): fighter/interceptor ← Light, Multirole, Heavy, Interceptor, Carrier, Stealth; strike ← Strike, CAS, Multirole; bomber ← Bomber; electronic warfare ← EW aircraft; reconnaissance ← UAV, Stealth, Light; support ← AWACS, Tanker. Crewed aircraft with a second seat (Condor, Magpie, Argus) allow a second human to take the WSO/EWO/ABM seat (§17.8, §32.6).

## 13.6 Era and technology tiers

| Era | Description | Radar | Missiles | Other |
|---|---|---|---|---|
| **G3** | Early supersonic / early BVR | Mechanically scanned, search + STT, no look-down clutter rejection at low altitude (reduced), notch ~ 120 m/s | IR WVR; semi-active radar (SARH) MRAAM (requires illumination until impact) | RWR basic (direction + threat class), chaff/flare manual |
| **G4** | Digital multirole | Pulse-Doppler, look-down/shoot-down, TWS, notch ~ 60 m/s | All-aspect IR with HOBS option; SARH + early active radar MRAAM | RWR with threat ID, semi-automatic CM, MAWS optional |
| **G4.5** | AESA | Fast electronic scan, LPI modes, better ECCM, notch ~ 40 m/s | Datalinked active-radar MRAAM, extended-range LRAAM | Digital RWR/ESM, towed decoy |
| **G5** | Stealth / networked | LPI, very low-RCS signature, sensor fusion | Internal carriage, datalink to non-launch platforms | Cooperative engagement; IRST |
| **Experimental** | Near-future / unique | Varies | Varies | Unique systems |

The mission generator's *Aircraft Era* parameter (outline §55) picks the tier for each side; **Mixed** allows different tiers per side.

## 13.7 Counter-play matrix (design intent)

| Attacker \ Defender | Light F. | Multirole | Heavy F. | Interceptor | Strike | CAS | Bomber | EW | AWACS | Stealth |
|---|---|---|---|---|---|---|---|---|---|---|
| **Light Fighter** | = | + | + (in the merge) | + (in the merge) | + | + | − (needs approach) | + | ✕ (out of reach) | = |
| **Multirole** | − | = | − | + | + | + | + | + | + | − |
| **Heavy Fighter** | − (at merge) | + | = | + (BVR) | + | + | + | + | + | = |
| **Interceptor** | − (at merge) | − | − | = | + | + | ++ | + | ++ | − |
| **Strike** | − | − | − | − | = | + (ground) | = | = | − | − |
| **Stealth** | + | + | + | + | + | + | + | + | + | = |

(`++` strongly favoured, `+` favoured, `=` even, `−` disfavoured, `✕` practical impossibility; the matrix is intent, not a hard rule; ground-attack counter-play is in §21.) Stealth is "favoured" only while its signature advantage holds (terrain, EMCON, distance), and it pays with limited internal load and low external capacity.

## 13.8 Acceptance criteria

- `AC-13.1` ParetoLint passes on the shipped roster with zero strict dominance and each aircraft meeting the top/bottom quartile rule.
- `AC-13.2` Each category in §13.1 has at least one launch aircraft whose `aircraft.json` passes the linter without `outlier_ok`.
- `AC-13.3` Playtest AI-vs-AI batch (1 000 duels per pair, same era, same skill, equal-cost loadouts, random geography): no aircraft has a win rate over 65 % against every other aircraft; archetype outcomes match §13.7 qualitatively.

---

# 14. AIRCRAFT CUSTOMIZATION

## 14.1 Scope

Players customize **weapons, fuel, countermeasures, radar presets, paint, camouflage, insignia and callsign** (outline §21). The loadout system (outline §22) displays hardpoints, weapon counts, weight, estimated drag, role and expected range, and — crucially — **weapon selection physically changes aircraft performance** via mass, inertia, CG, drag, RCS and g-limits.

## 14.2 Loadout editor

- **Hardpoint diagram:** top and side silhouettes with clickable stations. Station numbering follows the aircraft file. Hovering shows mount type, mass limit, interference.
- **Weapon palette** filtered by compatibility with the selected station, era, faction and (campaign) inventory.
- **Symmetric placement** on by default (placing on a station fills `sym`); toggle to override.
- **Role presets** (data): *Air Superiority*, *BVR Intercept*, *WVR Dogfight*, *Escort*, *Strike (precision)*, *Strike (dumb)*, *SEAD*, *Anti-Ship*, *Reconnaissance*, *Ferry (tanks only)*, *Chaos (max stores)*. Presets adapt to the aircraft's stations.
- **Fuel** slider: internal 0–100 %, external tanks as stores. A "Bingo/Joker" helper suggests fuel for the planned sortie distance.
- **Countermeasures:** dispenser bucket allocation (chaff/flare/decoy counts within dispenser capacity), and a **CM program** (burst count, interval, auto/manual threshold) chosen from 4 slots.
- **Radar presets:** default mode and search volume (azimuth ±60°/±30°, bars 2/4), emission policy (normal / EMCON-silent), RWR sensitivity (full / threat-only), jammer pod mode (§17).

## 14.3 Computed readouts

| Readout | Formula / source |
|---|---|
| Takeoff mass | `m_empty + fuel + Σ store masses` |
| CG position and **static margin** | From §11.9; warns if outside limits; shows stability bar |
| Inertia (I_x, I_y, I_z) | §11.9 |
| **Drag index** | `100 · Σ ΔC_D0,i / C_D0,clean` (%) |
| **RCS multiplier** | Ratio of frontal-sector mean RCS to clean (§16.4) |
| T/W, wing loading | `F_SL/(m g)`, `m/S` |
| Load-factor limit | `n_max` from the limit rules of §14.4 |
| Speed limit | `Vne, Mne` (reduced by certain stores) |
| **Range** | Quick Breguet preview and a simulated-profile estimate (below) |
| Take-off distance | §10.4 planner at the chosen runway/density altitude |
| Role tag | A2A / A2G / Multi / Support derived from stores and radar modes |

**Quick range estimate.** For a jet at constant lift/drag and TSFC: `R = (V / c) · (L/D) · ln(W₁/W₂)` with `c = TSFC·g` (units 1/s). The planner uses this for the preview and runs the Tier-2 reduced model (§11.17) along a standard profile (climb, cruise at best-range speed, descent, reserve) for the displayed *combat radius* and *ferry range*.

## 14.4 Store-dependent limits

Aircraft files carry `limits_by_config` rules (evaluated top to bottom, first match wins):

| Condition (example for the reference fighter) | n_max | Vne/Mne effect | Notes |
|---|---|---|---|
| Any wing-station store > 900 kg or lateral asymmetry > 600 kg | 5.5 | Mne 1.4 | Heavy/asymmetric |
| External tanks fitted (any) | 7.0 | Mne 1.6 | Tanks may be jettisoned (restores limit) |
| Only wingtip and fuselage AAMs | 9.0 | none | Clean combat |
| Bay doors open (stealth aircraft) | — | Mne 1.2 | RCS spike, speed limit |

If total takeoff mass exceeds MTOW the editor blocks launch (1v1, campaign) or warns and allows with a performance penalty (sandbox "overload": structural limits scaled by `MTOW/m`).

## 14.5 Loadout planner and validation

On any change the editor re-evaluates in < 50 ms:

1. Mount compatibility and interference.
2. Mass ≤ MTOW, per-station mass limits, CG within limits.
3. Runway suitability (take-off planner; "TOO SHORT / MARGINAL / OK").
4. Campaign inventory (only stocked items; stock consumed on launch).
5. Rule-set restrictions (1v1 rules, co-op host restrictions).

The server re-validates every player's loadout at join/spawn (§33.4); clients cannot launch with a loadout that fails validation.

## 14.6 Weapon and store data hookup

Stores are weapon definitions (§15.2) with `mass`, `drag_cd0`, `rcs_m2`, `mount`, `length/diameter` (for visuals and clearance), and `jettison` behaviour. The editor builds `WeaponStations` for the Sim Core; there is no loadout-specific code in the simulation.

## 14.7 Cosmetics

**Livery system.** A livery is a small JSON file (< 2 KB without custom decals) selecting:

```json
{ "schema": "livery/1", "aircraft": "kestrel", "base": "std_mask_v1",
  "palette": { "primary": "#7a8a99", "secondary": "#4f5a63", "accent": "#d9a441", "underside": "#a9b4bd" },
  "camo": { "pattern": "wave", "scale": 1.0, "contrast": 0.35, "seed": 4821 },
  "weathering": { "dirt": 0.3, "wear": 0.2, "exhaust": 0.5 },
  "decals": [ { "id": "stripe_a", "slot": "tail_l", "color": "#d9a441" }, { "id": "user:3f2a", "slot": "nose_r", "scale": 0.8 } ],
  "callsign": "VIPER 2-1", "tail_number": "073", "squadron_emblem": "emblem_wolf" }
```

- **Mask-based recolouring.** The aircraft texture has an RGB mask: R = primary, G = secondary, B = accent areas; the shader tints them from the palette. Camouflage patterns (splinter, wave, digital, desert, arctic, ghost-grey, sea-blue, none) are **procedural** (tileable noise thresholds) blended through the mask; no pattern is a stored texture.
- **Decals.** Built-in decal library plus **user decals** (PNG ≤ 256×256, ≤ 64 KB, validated and re-encoded). User decals stay on the device; in LAN lobbies they are shared only when the owner enables "share custom decals" (a lobby-level consent; any client can disable viewing them). No images are uploaded to servers.
- **Callsign / tail number.** Text ≤ 12 characters, filtered by a local profanity list in public displays; callsign feeds the radio system (§56.6) as a spoken name (TTS-free: the radio uses a callsign vocabulary; custom callsigns fall back to alphanumeric phonetics).
- **Faction rules.** The faction (§37) supplies default paint schemes and constrains insignia; real-world insignia are unavailable (ADR-11).

## 14.8 Presets and sharing

Loadout and livery presets are saved per aircraft (IndexedDB, §64), exportable as JSON, and embeddable in a **share link** (loadout ≈ 200–400 bytes after compression, base64url in the URL fragment). Importing validates against the current content version and reports missing items.

## 14.9 Edge cases

| Case | Handling |
|---|---|
| Item removed in a new content version | Slot becomes empty; preset flagged "updated"; user notified |
| Fuel leak or unbalanced fuel in flight | Handled by sim (§18); loadout is the starting state only |
| Campaign inventory shortage | Editor shows "unavailable"; presets auto-substitute with equivalents when the player accepts |
| Custom decal fails validation | Rejected with reason (size, type, dimensions) |
| Asymmetric load | Allowed with trim penalty; warning if beyond limits |

## 14.10 Acceptance criteria

- `AC-14.1` Changing any store updates mass, CG, drag index, RCS multiplier, `n_max`, and range within 50 ms and the Sim Core's launched aircraft matches the displayed mass/CG within 0.1 %.
- `AC-14.2` An invalid loadout (wrong mount type) cannot be launched via UI, and a hand-edited invalid loadout is rejected by the server with a specific error.
- `AC-14.3` Loadout share link round-trips for all shipped presets (encode→decode→equal).
- `AC-14.4` A livery with 3 colours and one built-in decal applies in < 100 ms on the Recommended profile and uses < 4 MB GPU memory beyond the base aircraft.

---

# 15. WEAPONS SYSTEM

`P3 · M3` — Weapons are independent modular entities (outline §23). This chapter specifies the *game model* of weapons: data, flight behaviour at gameplay fidelity, seekers, lethality, and release rules. All performance numbers are **design targets** for a fictional arsenal (ADR-11), not real-weapon data.

## 15.1 Intent

- Weapons must feel like *systems with envelopes*, not guaranteed-hit buttons (NN-3). Position, speed, altitude, target aspect, countermeasures, terrain and pilot decisions all matter.
- Different weapons behave differently; players learn their envelopes through the HUD launch-zone display (§54.5) and experience.
- Weapons are data (§62). The weapon simulation is generic: a missile is a body with a motor, aerodynamics, an autopilot, a seeker, a guidance law, a fuze and a warhead, all parameterised.

## 15.2 Weapon definition

Every weapon shares a common header; class-specific blocks follow.

```json
{
  "schema": "weapon/1", "id": "lance_4", "name": "Lance-4", "class": "mrm", "era": "G4.5",
  "mass_kg": 155, "length_m": 3.7, "diameter_m": 0.178, "cd0_carriage": 0.0006, "rcs_m2": 0.04, "ir_signature": 0.2,
  "mount": ["rail_mrm", "bay_ejector"], "faction_tags": ["common"], "threat_value": 6.5,
  "motor": { "stages": [ { "thrust_n": [[0,13000],[2.5,11000],[8.5,0]], "burn_s": 8.5, "propellant_kg": 52, "mdot_curve": "linear" } ] },
  "aero": { "s_ref_m2": 0.0249, "cd0_boost": [[0,0.28],[1.2,0.42],[2.5,0.33],[4,0.26]], "cd0_coast": [[0,0.34],[1.2,0.52],[2.5,0.40],[4,0.31]],
            "cn_alpha_per_rad": 24, "cn_max": 14, "k_induced": 0.045 },
  "autopilot": { "tau_s": 0.25, "g_max": 40, "g_min_speed_mps": 120 },
  "guidance": { "law": "apn", "n_prime": 4.0, "midcourse": "datalink_inertial", "loft": [[20000,0],[60000,15],[100000,25]] },
  "seeker": { "type": "arh", "activate_range_m": 18000, "fov_deg": 60, "gimbal_deg": 55, "lock_ref_range_m": 22000, "ref_rcs_m2": 5, "noise_deg": 0.2, "eccm": 0.35 },
  "fuze": { "type": "proximity", "arm_dist_m": 250, "trigger_radius_m": 9, "cone_deg": 360 },
  "warhead": { "kind": "frag", "he_kg": 18, "lethal_radius_m": 9, "frag_cone_deg": 50 },
  "launch": { "min_speed_mps": 60, "max_g": 2.5, "max_alpha_deg": 20, "eject_mps": 6, "safe_sep_s": 0.5 }
}
```

(Illustrative; shipped values are tuned in §15.12 and Appendix D.)

## 15.3 Weapon categories

| Class | Gameplay role | Guidance family | Typical engagement |
|---|---|---|---|
| **Aircraft gun** | Close-range, skill shot; unlimited-feeling, no countermeasure | Ballistic | 150–1 200 m |
| **Short-range missile (SRM)** | Dogfight; IR; fire-and-forget | IR | 0.5–25 km |
| **Medium-range missile (MRM)** | BVR/merge; semi-active or active radar | SARH / ARH + datalink | 15–100 km |
| **Long-range missile (LRM)** | Stand-off BVR; lofted; lethal but defeatable | Datalink + ARH | 50–160 km |
| **Unguided bomb** | Area/point target; CCIP/CCRP aiming | Ballistic | Dive/level release |
| **Guided bomb** | Precision; laser/GPS; glide range extension | Laser / GPS-INS | 5–40 km |
| **Rocket** | Area suppression, soft targets | Ballistic salvo | 1–5 km |
| **Anti-radiation missile (ARM)** | SEAD; homes on emitters | Passive radar | 20–70 km |
| **Anti-ship missile** | Sea-skimming; active radar | INS + active radar | 30–90 km |
| **Cruise missile** | Stand-off land attack; terrain-following | INS/TERCOM-like (DEM) | up to 300 km |
| **Torpedo** | ASW (§22.7) | Acoustic (abstracted) | 3–10 km |
| **Special** | Decoy drones, mines (optional), sensor-seeded munitions | Varies | — |

## 15.4 Guns and ballistics

### 15.4.1 Gun definition

| Field | Meaning |
|---|---|
| `rate_rps` | Rounds per second (rotary 6 000 rpm = 100 rps; revolver 1 700 rpm ≈ 28 rps) |
| `muzzle_velocity_mps`, `caliber_mm`, `proj_mass_kg`, `cd_table(M)` | Exterior ballistics |
| `dispersion_mrad` | Gaussian spread (σ) at the muzzle; grows with firing duration (barrel heat) and aircraft g/slip |
| `spin_up_s` | Delay before full rate (rotary) |
| `ammo_belt` | Mix pattern, e.g. `[HEI, AP, HEI, tracer]` |
| `convergence_m` | Distance at which multiple guns' axes cross (default 400 m) |
| `harmonisation_pitch_mrad` | Gun axis relative to the aircraft datum |
| `recoil_n` | Recoil force on the airframe (small but real) |

Shot types: **HEI** (high-explosive incendiary: small blast, incendiary chance on fuel/hydraulics), **AP/API** (higher penetration), **tracer** (visual; 1 in 4–5 rounds). Each type has a damage table by component armour class (§18.3).

### 15.4.2 Projectile simulation

Projectiles are simulated individually (affordable: ≤ 6 000 in flight at the High cap, §0.6.2) as point masses with quadratic drag:

```text
dv/dt = −(ρ · C_d(M) · A_ref / (2 m)) · |v_rel| · v_rel + g(p)         v_rel = v − w
```

Per 60 Hz tick, each projectile is integrated analytically with 2 sub-steps (RK2), then a **swept segment** test is run against: (1) target hit volumes via the 3-D spatial hash (cell size 2 km, entity AABB broadphase, then oriented-box/capsule narrowphase, §18.2), (2) terrain via a DEM ray-march of the segment, (3) the sea surface. First contact within the tick wins. A projectile expires at 3 s of flight or when its speed falls below 300 m/s.

Projectile state is stored in struct-of-arrays pools (`pos[]`, `vel[]`, `age[]`, `shooter[]`, `type[]`) with free lists; zero allocation during play.

### 15.4.3 Gunsight (Lead Computing Optical Sight)

For target position `p_t` and velocity `v_t` relative to the shooter `p_s`, the sight solves for time-of-flight `t` such that the projectile reaches the target:

```text
find t:  | (p_t − p_s) + v_t t − ½ a_t t² − (v_proj t + ½ g t² − drag_loss(t)) | = 0       (iterate 3–5 times, Newton on t)
pipper direction = normalize( p_t + v_t t − p_s − ½ g t² ... in the shooter's frame, corrected for aircraft rotation ω × r )
```

The HUD draws the **pipper** (lead solution) and a **range-to-target** bracket; accuracy of the sight depends on radar range input (if no radar lock: manual range estimation via span bracketing — a reduced-assist skill expression, §54.4).

### 15.4.4 Damage per hit

A hit deposits energy-based damage into the struck component (§18.3): `dmg = k_type · (½ m v_impact²)^{0.7} · cos θ_incidence_factor` modified by component armour class and ricochet at grazing angles `θ > 70°` with probability `1 − 1/(1 + e^{(θ−75°)/4°})`. Calibration targets (design): 20 mm HEI hits needed to destroy a light fighter's wing: ~8–12; 30 mm AP hits to stop a CAS-class armoured engine nacelle: ~15–20.

## 15.5 Missile flight model (game fidelity)

### 15.5.1 State machine

```text
CAPTIVE (on rail; seeker cooling/aligning)
  → LAUNCH (ejection / rail travel; safe separation window)
  → BOOST (motor stage 1)
  → [SUSTAIN] (motor stage 2 or ramjet cruise)
  → MIDCOURSE (INS + datalink updates; loft)
  → TERMINAL (seeker active; PN homing)
  → COAST (motor out; energy decays)
  → { DETONATE (fuze) | SELF-DESTRUCT (timer/speed < v_min/ground proximity) | IMPACT }
```

### 15.5.2 Equations of motion (3-DOF point mass with lateral acceleration capability)

```text
m v̇ = T·t̂ − D·v̂_rel + a_lat · m + m g(p)
D   = q̄ S_ref [C_D0(M, boost/coast) + k_induced · C_N²]
C_N = m |a_lat| / (q̄ S_ref)          ≤ C_N,max        (aerodynamic limit on lateral acceleration)
|a_lat| ≤ min(g_max, q̄ S_ref C_N,max / m)           structural and aerodynamic limits
a_lat tracks a_cmd through a first-order lag τ_ap
```

The missile loses manoeuvrability as speed and density fall (the aerodynamic limit scales with `q̄`), reproducing the "missile energy" mechanic that rewards forcing missiles to turn at long range or high altitude. Mass decreases with propellant burn via the motor's mass-flow curve.

Integration rate: 120 Hz when within 8 km of the target or in launch/terminal phases; otherwise 60 Hz (§0.6.1). Position error accumulates less than 0.2 m over 60 s versus a 480 Hz reference (`AC-15.3`).

### 15.5.3 Guidance laws

| Law | Command | Use |
|---|---|---|
| Pure/lead pursuit | Turn toward (lead) target | Unguided rockets with seekers (rare) |
| **Proportional navigation (PN)** | `a_cmd = N′ · (Ω × v_m)`, Ω = (R × V_rel)/(R·R), R = r_t − r_m, V_rel = v_t − v_m | Default homing |
| **Augmented PN (APN)** | PN plus `½ N′ · a_t⊥` (target lateral acceleration estimate) | MRM/LRM terminal |
| Lofted/energy-managed midcourse | Pitch bias profile `loft(range)` plus altitude cap, then dive | LRM/MRM at long range |
| Waypoint/terrain-following | Follow pre-planned path at constant AGL using base DEM | Cruise, anti-ship (sea-skimming 5–15 m) |
| Inertial + datalink | Aim at last datalinked target position/velocity extrapolated | Midcourse |
| Passive homing on emission | PN toward emitter bearing; memory if emitter ceases | ARM |

`N′` is data (nominal 3–5). Commands are limited by the lateral acceleration limits above, so a late-turning target can saturate the missile.

### 15.5.4 Midcourse information

- **Datalinked missiles** receive target updates at 1–2 Hz from the launching aircraft (or any friendly datalink node) while that node maintains a track; update quality depends on track quality (§16.5) and latency (0.3 s). On loss of track the missile flies on INS to the last known intercept point, with growing uncertainty `σ_pos(t) = σ_0 + v_t,max · (t − t_loss) · 0.5`, and the seeker (if active) opens its search around that point.
- **Semi-active radar** missiles require continuous target illumination by the launcher until impact ("support"); the launcher must keep a Single Target Track (STT), constraining manoeuvre, notching and crank options (a core tactical trade).
- **Active radar** missiles go active at `activate_range_m` ("pitbull"); the target's RWR shows a missile launch at fire and a missile-active tone at activation.
- **Fire-and-forget IR** missiles need lock before launch (or cueing) and are independent thereafter.

### 15.5.5 Launch envelope and the Dynamic Launch Zone (DLZ)

The HUD shows **R_max** (kinematic maximum), **R_ne** (no-escape: even a maximally-manoeuvring target of the given class cannot defeat it kinematically), **R_min**, and the current range. These come from baked tables:

1. A CI tool (**DLZ Baker**) runs the actual missile model against scripted targets over a grid: launch altitude (6 values: 0–15 km), launch Mach (5), target altitude (4), target aspect angle (0°, 30°, 60°, 90°, 120°, 150°, 180°), target speed class (3), and target manoeuvre (none / break at fuze arming) → ≈ 2 500 cells × 2 values per missile.
2. Runtime interpolates the table with the live geometry; the pilot's HUD and AI both query it.
3. Tables are versioned with the weapon; a weapon edit invalidates and rebakes (CI gate).

R_max is also scaled live by the launcher's energy (current Mach/altitude) and the target's current closure.

## 15.6 Miss model and lethality (no guaranteed hits — NN-3)

A missile hit is the outcome of this chain; each link can fail:

| Link | Failure modes (non-exhaustive) |
|---|---|
| Launch | Outside launch constraints (g, α, speed) → misfire/hung; missile hits launcher (§15.10) |
| Lock / track | Seeker cannot see target (FOV, range, aspect, RCS/IR); track lost to notch or terrain; jammed |
| Guidance | Target manoeuvre exceeds missile energy/g; late turn; saturation |
| Countermeasures | Flares/chaff/ECM/decoys defeat seeker (§17.5) |
| Fuzing | Closest approach outside trigger radius; fuze fails to arm (inside `arm_dist_m`); cone mismatch |
| Warhead effect | Fragments strike non-critical components; damage below kill threshold (§18.6) |
| Self-destruct | Energy exhausted (`speed < v_min`) or flight time exceeded |

**Fuze and lethality.** The proximity fuze triggers at the first sampled local minimum of range inside `trigger_radius_m` after arming; the miss distance `d_m` at detonation determines damage:

```text
x     = d_m / R_lethal
f(x)  = (1 − x)²   for x < 1, 0 otherwise                         radial lethality
damage to component c in the fragment cone = base_dmg · f(x) · A_c / A_exposed · cos(θ_c)
```

Fragments are not simulated individually; instead, per detonation, the hit volumes within `R_lethal` and within the fragment cone each receive a damage roll (§18.6). Larger warheads scale `R_lethal ∝ (W_HE)^{1/3}` (Hopkinson–Cranz scaling), with the numbers set in data.

**Rule check for NN-3:** a unit test fires 10 000 missiles with randomised realistic geometry against a competent evader AI and requires a hit rate that varies with the conditions as intended (§15.14), never ~100 %.

## 15.7 Seekers

| Seeker | Detection model (gameplay) | Susceptibilities | Lock behaviour |
|---|---|---|---|
| **IR (rear-aspect, G3)** | Irradiance threshold; strong dependence on engine heat, aspect within ±35° of tail | Flares (high), sun, ground clutter low-altitude | Audio growl rises in pitch with signal; lock tone |
| **IR (all-aspect, G4+)** | Lower sensitivity front aspect; HOBS option with cueing | Flares (medium, IRCCM), sun | Fire-and-forget |
| **Imaging IR (G4.5+)** | Shape-based; hard to decoy | Dazzle (rare), smoke/clouds | Fire-and-forget |
| **Semi-active radar** | Radar-equation detection of illuminated echo | Notch, chaff, jammers | Needs launcher STT |
| **Active radar (ARH)** | Own transmitter, smaller power: detection range from `lock_ref_range_m·(σ/σ_ref)^0.25` | Notch, chaff, jammers (burn-through), terrain | Fire-and-forget after activation |
| **Anti-radiation** | Receives emitter; bearing error ∝ emitter bandwidth/power | Emitter shutdown (memory), decoys (emitter spoofs) | Passive |
| **EO/TV (A2G)** | Contrast/size on screen; pilot slews | Weather, obscurants | Lock by pilot via pod/screen |
| **Laser** | Needs designator; spot visible | Obscurants; designator loss | Semi-active laser |
| **GPS/INS** | Position error growth | Jamming (GPS denial option) | Pre-programmed target |

**IR irradiance model (gameplay).** Received irradiance `E = I_t · τ(R) / R²` with atmospheric transmittance `τ = exp(−β R)`; lock if `E ≥ E_min`, giving a lock range that scales as `√(I_t/I_ref)` in clear air. Target intensity `I_t` is the sum of: engine term (idle 0.2, mil 1.0, AB 3.5, relative units) × aspect pattern (rear 1.0, side 0.45, front 0.15) + airframe heating term ∝ M² + decoy terms. Altitude and ambient temperature scale intensity modestly (cold air → higher contrast).

**Radar seeker model.** Detection range vs. a target of RCS σ: `R = R_ref (σ/σ_ref)^{1/4} · f_aspect · f_clutter · f_jam`; track quality (§16.5) governs guidance noise: `σ_los = σ_0 / √(SNR_ratio)`.

**Gimbal and slew.** Seeker boresight is limited to `gimbal_deg` from the missile axis; if the target leaves FOV the seeker breaks lock (and the missile continues inertially). Pre-launch **cueing** (radar slave, IRST, helmet cue if equipped) slews the seeker; time to acquire lock is `t_lock = t_0 + k·(angular error)`.

## 15.8 Countermeasure interaction (summary)

Details are in §17.5; the weapons side is:

- **Flares vs IR:** each flare burn spawns an IR source (intensity 2–3× AB for 3–5 s, falling over time and drifting aft). The seeker tracks the highest-weighted source within its FOV with `P_decoy = f(intensity_ratio, angular_separation, seeker IRCCM, flare_pattern)`; a stable lock on the aircraft is retained when separation is small and IRCCM high.
- **Chaff vs radar:** creates a cloud of RCS that spreads; relevant when the aircraft's Doppler (closure rate) is within the cloud's decorrelation (notch: aircraft beaming the radar while chaffing) — otherwise the Doppler filter rejects chaff (low velocity).
- **Jamming:** raises the seeker's `f_jam` penalty and noise (burn-through range, §17.3).
- **Maneuvers:** high-G turns beyond the missile's remaining lateral capability, notching (beaming), terrain masking and diving all emerge from the model.
- **Active protection** (`P10`): hard-kill/directed IR countermeasures are future work.

## 15.9 Air-to-ground and other munitions

### 15.9.1 Dumb bombs and release computation

Bombs are ballistic bodies with `C_D(M)` and fin stabilisation. The HUD offers:

- **CCIP** (continuously computed impact point): at each frame the Sim Core integrates the bomb trajectory from the current aircraft state to the ground (base DEM) with the current wind and returns the impact point; drawn as a pipper. Cost: ≤ 40 integration steps at 20 ms.
- **CCRP** (continuously computed release point): the player flies to a pre-selected target; the system computes the release point and times the release (auto-release optional).
- **Fuzing:** impact, delay (0.01–0.3 s), airburst (height above ground), proximity.

### 15.9.2 Guided bombs

Glide kits yield `L/D ≈ 4–8`: the bomb flies a PN or glide-slope guidance toward target coordinates (GPS/INS) or laser spot (designator required — own pod or an ally); range extension is computed in the HUD as a release footprint.

### 15.9.3 Rockets

Unguided rockets: salvo with ripple interval; spread σ ≈ 4–8 mrad; effective vs soft targets within 4 km. Pods are stores.

### 15.9.4 Anti-radiation missiles

Modes: *self-protect* (launches on the most threatening emitter detected), *target-of-opportunity* (pilot picks), *pre-briefed* (location). After launch the missile homes on the emitter's bearing; if the emitter shuts down (EMCON) it flies to the last known position (memory time `t_mem` 10–20 s). SAM operators' counter-play is detailed in §21.5.

### 15.9.5 Anti-ship and cruise missiles

Sea-skimming at 5–15 m AGL (sea state affects minimum height), terminal pop-up or dive; active radar seeker with ship radar cross-section (large). Defence layers: ship SAM, CIWS, chaff, decoys (§22.4). Cruise missiles follow a pre-planned route computed on the base DEM with terrain-following altitude `h_AGL = 60–150 m`; speed subsonic (design 240–260 m/s); vulnerable to interception by fighters if detected.

### 15.9.6 Blast effect on ground targets

Targets have `armour_class` and `hp`. Damage: `dmg(r) = W_dmg · exp(−(r/R0)²)` with `R0 = k_target · W_HE^{1/3}`; `k_target` is 3.0 for soft/structural targets, 1.8 for armoured vehicles, 4.5 for large buildings (design targets). Cluster munitions apply a distribution of sub-munition impacts (area footprint with Gaussian density). Runway-penetrator munitions create craters affecting runway operability (§48.7).

## 15.10 Release and launch logic

| Rule | Detail |
|---|---|
| Arming | Master arm must be on; weapon selected; station not jammed by damage |
| Constraints | Aircraft speed ≥ `launch.min_speed_mps`; load factor within `[0.5, max_g]` (stores); α, β within limits; bay doors open for internal weapons (+0.8 s delay); gear up for some stores. Violations: *launch inhibited* (HUD cue) in assisted modes, *misfire/hung* risk in Reduced/Manual |
| Safe separation | The new missile is flagged `no_collide_with_launcher` for `safe_sep_s` (0.5 s) or until 30 m distance; rockets and bombs separate by ballistic motion; ejection impulse `eject_mps` along the station's `launch.dir` |
| Salvo | Ripple interval 0.15–0.5 s; consecutive launches from alternating stations to preserve lateral balance |
| Jettison | Bombs/tanks/pods can be jettisoned; emergency jettison drops all external stores |
| Station state | `Loaded`, `Empty`, `Hung`, `Damaged`, `Jettisoned` |

## 15.11 Performance and networking

- Missiles are entities with mini-LOD: far from any human, a missile on a long coast can run at 10 Hz with analytic prediction (still checks proximity every tick via swept sphere).
- Replication: a launch is a **reliable event** (id, type, launcher, target, seed); the missile entity is then replicated at 20 Hz while within 30 km of any client, otherwise 5 Hz plus a predicted trajectory (§33.5). Clients create the cosmetic missile immediately on their own trigger press (prediction) and reconcile when the authoritative launch event arrives.
- Gun fire is replicated as a **fire event** with seed and parameters (start tick, burst length, aim), not per-round state; each client regenerates tracers locally, and only the authority resolves hits (§33.6).

## 15.12 Launch weapon roster (design targets; fictional names)

Weapon names are placeholders (naming/IP review required). Max range figures are for a head-on shot from ~10 km altitude at Mach 0.9 against a non-manoeuvring target; they are inputs for the DLZ Baker and should be treated as design targets until baked.

| ID | Name | Class | Era | Mass (kg) | Guidance | Design max range (km) | Warhead (kg HE) | Mount |
|---|---|---|---|---|---|---|---|---|
| gr20 | GR-20 | Gun (rotary 20 mm, 100 rps, 500 rds) | G3+ | 120 + ammo | Ballistic | 1.2 effective | 0.01/round | internal |
| gs27 | GS-27 | Gun (revolver 27 mm, 28 rps, 150 rds) | G4 | 100 | Ballistic | 1.4 | 0.03/round | internal |
| gs30 | GS-30 | Gun (30 mm, 30 rps, 180 rds) | G3+ | 110 | Ballistic | 1.5 | 0.05/round | internal |
| gh30 | GH-30 | Heavy rotary 30 mm (65 rps, 1 100 rds, AP/HEI) | G3+ | 280 + ammo | Ballistic | 1.8 | 0.05/round | CAS-class internal |
| dart1 | Dart-1 | SRM, rear-aspect IR | G3 | 85 | IR | 9 | 9 | rail |
| dart4 | Dart-4 | SRM, all-aspect imaging IR, HOBS | G4.5 | 88 | Imaging IR | 25 | 10 | rail |
| lance2 | Lance-2 | MRM, semi-active radar | G3/G4 | 190 | SARH | 40 | 25 | rail/pylon |
| lance4 | Lance-4 | MRM, active radar | G4 | 155 | INS + ARH | 70 | 18 | rail/bay |
| lance6 | Lance-6 | MRM, active radar + datalink | G4.5 | 160 | INS+datalink + ARH | 100 | 18 | rail/bay |
| pike9 | Pike-9 | LRM, lofted, throttleable motor | G5 | 200 | Datalink + ARH | 160 | 22 | pylon/bay |
| talon1 | Talon-1 | AGM, EO/IR imaging | G4 | 210 | EO lock-on | 25 | 55 | pylon |
| talon3 | Talon-3 | AGM, laser-guided, light | G4 | 45 | Semi-active laser | 8 | 8 | pylon |
| hunter2 | Hunter-2 | ARM | G4 | 180 | Passive radar | 70 | 66 | pylon |
| skua1 | Skua-1 | Light anti-ship missile | G4 | 140 | INS + active radar | 30 | 45 | pylon |
| tern2 | Tern-2 | Anti-ship missile (sea-skimmer) | G4 | 520 | INS + active radar | 85 | 170 | pylon |
| dirkc | Dirk-C | Cruise missile (terrain-following) | G4.5 | 1 300 | INS + DEM-matching | 300 | 450 | pylon/bay |
| b250 | B-250 | General-purpose bomb | G3+ | 250 | Ballistic | — | 100 | pylon |
| b500 | B-500 | General-purpose bomb | G3+ | 500 | Ballistic | — | 220 | pylon |
| b900 | B-900 | Heavy bomb | G3+ | 900 | Ballistic | — | 420 | pylon |
| cb300 | CB-300 | Cluster bomb (area) | G3+ | 300 | Ballistic | — | 60 (dispersed) | pylon |
| rp1 | RP-1 | Runway-penetrator bomb | G4 | 900 | Ballistic | — | 250 | pylon |
| gb250l | GB-250L | Laser-guided bomb | G4 | 250 | Semi-active laser | 15 | 100 | pylon |
| gb500g | GB-500G | GPS/INS glide bomb | G4.5 | 500 | GPS/INS | 40 | 220 | pylon/bay |
| rkt19 | RKT-19 | 19-tube 70 mm rocket pod | G3+ | 280 loaded | Ballistic | 4 | 19 × 1.0 | pylon |
| rkt7 | RKT-7 | 7-tube 70 mm rocket pod | G3+ | 120 loaded | Ballistic | 4 | 7 × 1.0 | pylon |
| tt1 | TT-1 | Air-dropped lightweight torpedo | G4 | 250 | Acoustic homing | 8 | 45 | pylon |

Non-weapon stores (share the loadout system): ECM pod (self-protect, `ecm1`), stand-off jammer pod (`ecm3`), targeting/designation pod (`tp2`), reconnaissance pod, drop tanks (`ft1400`, 1 400 L), towed decoy (`td1`), chaff/flare dispenser loads (internal buckets).

Totals: 26 weapons + 6 non-weapon store types at launch (§1.4).

## 15.13 Edge cases

| Case | Handling |
|---|---|
| Target destroyed in flight | Missile retains last known point; self-destructs after 2 s or detonates on closest approach to wreckage if within trigger radius (suppressed for friendly wreckage) |
| Missile vs terrain | Detonates on ground contact; proximity fuze ignores terrain (surface masking) |
| Friendly fire | Governed by rule set (`friendly_fire` flag); IFF on seeker prevents lock on friendlies for ARH/IR (configurable "dumb seeker" mode for Simulator) |
| Chaff + notch + diving | Intended combination to defeat radar missiles: the model rewards it |
| Missile launched near boundary | Missile obeys boundary: destroyed at Stage 4 (§8.5) |
| Hang fire / dud | Per-weapon dud probability (data; default 1 %) in realism ≥ Hard |
| Weapon removed by content update | Loadouts migrate (§14.9) |

## 15.14 Acceptance criteria

- `AC-15.1` Muzzle velocity and drop at 1 000 m match the analytic solution within 0.5 % in vacuum; with drag, within 1 % of a 10× finer reference integration.
- `AC-15.2` Gunsight solution error < 1 mrad on a non-manoeuvring crossing target at 600 m in a scripted test.
- `AC-15.3` Missile position error versus a 480 Hz reference integration < 0.2 m over a 60 s flight; no energy gain without thrust.
- `AC-15.4` DLZ Baker output: R_max ordered correctly across aspects (head-on > beam > tail for a closing launch at equal speed); R_ne < R_max in 100 % of cells.
- `AC-15.5` Hit-rate sweep (10 000 shots per cell, competent evader AI): MRM hit rate vs. a non-manoeuvring target at 0.5 R_max ≥ 80 %, at 1.0 R_max ≤ 40 %; vs. a notching/chaffing evader at 0.5 R_max ≤ 35 %; IR missile vs. flaring target at 4 km ≤ 55 %. (Targets are design intent; tuning may revise numbers but the monotonic ordering is mandatory.)
- `AC-15.6` No weapon in the launch roster is "guaranteed hit": the maximum hit rate over all cells with a defending human-like evader AI < 95 %.
- `AC-15.7` Launch constraints reject a missile launched above `max_g` in assisted modes, and in Reduced/Manual the launch occurs with the documented failure probability.
- `AC-15.8` Gun swept-collision catches 100 % of 1 000 m/s rounds passing through a 1 m target at 60 Hz (no tunnelling).

---

# 16. RADAR & AVIONICS

`P3 · M3` — Radar is a major gameplay system (outline §26). It must **not** simply reveal every object within a circular radius. Detection depends on range, aspect, altitude, radar cross-section (RCS), terrain masking, jamming, weather and track quality. The same sensor model serves human aircraft, AI aircraft, ground radars, ships and AWACS (ADR-16, NN-5).

## 16.1 Intent

- Information is a resource (outline §144): what you know depends on what your sensors, your allies and your intelligence give you.
- Players should be able to *reason* about sensors: why they were detected, why a lock broke, how to hide behind a mountain.
- Cost must scale: hundreds of emitters and targets, evaluated at 10 Hz, in a browser (§16.12).

## 16.2 Sensor architecture

```text
Emitters / signatures (RCS, IR, emissions)
        │                       ┌──────────────┐
        ▼                       │ LosService   │  terrain / curvature / cloud occlusion
Sensor models ──detections──►   │ (§16.9)      │
  radar · IRST · visual ·       └──────────────┘
  RWR/ESM · MAWS · sonar-lite
        │
        ▼
Local Track File (per observer)  ──datalink──►  Shared Picture (team) ──► UI / AI perception
```

| Sensor | Passive/active | Output | Notes |
|---|---|---|---|
| Radar (air-to-air, air-to-ground, sea, ground-based, ship, AWACS) | Active | Range, bearing, elevation, closing rate | §16.3 |
| IRST | Passive | Bearing/elevation (angle-only), IR intensity | §16.6 |
| Visual | Passive | Bearing/elevation, size class, type at close range | §16.6 |
| RWR / ESM | Passive | Emitter bearing, type, mode | §16.8 |
| MAWS (missile approach warning) | Passive | Missile bearing, time-to-impact estimate | Optional by avionics tier |
| Datalink | Receive | Track updates from allies/AWACS | §16.5 |
| Targeting pod (EO/IR/laser) | Active/passive | Ground target lock, ranging, designation | §15.9, §21 |
| Sonar-lite | Active/passive | Submarine detection by sonobuoys/dipping (abstracted) | §22.7 |

## 16.3 Radar model

### 16.3.1 Range model

Each radar is parameterised by a **reference range** `R_ref`: the single-dwell 50 % detection range against a reference target `σ_ref = 5 m²` at head-on aspect in clean air, above clutter and unjammed. This captures the radar equation's fourth-root dependence:

```text
R_det(σ) = R_ref · (σ / σ_ref)^{1/4} · f_aspect-free · f_clutter · f_weather · f_jam · f_mode
```

(`σ` is the RCS from the target's table at the actual aspect, §16.4; so aspect enters through σ.) Worked scaling for `R_ref = 120 km`:

| Target RCS σ | Example | R_det |
|---|---|---|
| 0.001 m² | Very low-observable aircraft (front) | 14.3 km |
| 0.01 m² | Low-observable | 25.4 km |
| 0.1 m² | Cruise missile / small UAV | 45.1 km |
| 1 m² | Small fighter, nose-on | 80.2 km |
| 5 m² | Reference | 120 km |
| 20 m² | Large fighter/bomber broadside-ish | 169.7 km |
| 100 m² | Large transport | 253.8 km |

Radar **classes** (`R_ref`): Low 50 km, Medium 90 km, High 120 km, Very high 160 km, Ship 200 km, Ground early-warning 300 km, AWACS surveillance 400 km (design targets).

### 16.3.2 Detection probability

Define the signal-to-threshold ratio `ρ = (R_det / R)^4` (the radar equation's `R⁻⁴` dependence). A single dwell detects with

```text
P_d(ρ) = ρ^s / (1 + ρ^s),       s = 4        (50 % at ρ = 1; 93 % at ρ = 1.5; 6 % at ρ = 0.5)
```

During a scan frame a target in coverage receives several dwells (beam passes), so cumulative detection is high just inside `R_det` and falls quickly beyond it; each radar mode (§16.3.5) defines the number of dwells per frame. A detection event is sampled once per dwell using the sim's per-entity RNG (not render-side randomness).

### 16.3.3 Look-down clutter and the Doppler notch

Pulse-Doppler radars separate moving targets from ground clutter by Doppler frequency. Mainlobe clutter sits at the Doppler of the radar platform's own velocity projected on the line of sight. A target's Doppler is its closing rate; relative to the clutter line, the target is displaced by **the target's own ground-velocity component along the radar line of sight**. When that component is smaller than the radar's notch width, the target is rejected with the clutter ("beaming" or "notching").

Rule: if the beam intersects the ground behind the target (target below the radar's local horizontal plane by more than half the beamwidth — a *look-down* geometry) and

```text
| v_t · û_los | < v_notch     (û_los: unit vector radar→target; v_t: target ground velocity)
```

then `P_d` is multiplied by `f_notch = 0.02` (not exactly zero: track can be recovered by luck or by loft). When the target is above the radar horizon beyond half a beamwidth (look-up), there is no mainlobe clutter and the notch does not apply (sidelobe clutter is ignored).

The beaming window around the perpendicular (half-angle) is `θ_n = asin(v_notch / V_t)`. Examples at target speed 250 m/s:

| Era | v_notch | Window half-angle |
|---|---|---|
| G3 | 120 m/s | 28.7° |
| G4 | 60 m/s | 13.9° |
| G4.5 / G5 | 40 m/s | 9.2° |

This gives the classic defensive tactic ("turn beam/notch") and the offensive counter ("crank/offset to keep the target out of the notch window"); the HUD exposes a **notch indicator** in realistic modes (§54.6).

### 16.3.4 Other factors

| Factor | Rule |
|---|---|
| Low-altitude clutter `f_clutter` | Target below 150 m AGL over land: ×0.5; over sea: ×0.7 (sea state scales); above 300 m AGL: ×1.0; linear between |
| Weather `f_weather` | Rain attenuation ×(1 − 0.10·rain_intensity) for medium-band radars; ×(1 − 0.25·rain_intensity) for short-wavelength seekers; none for dry cloud |
| Jamming `f_jam` | §17.3 (burn-through model) |
| Mode `f_mode` | RWS 1.0; TWS 0.85 (dwell split); STT 1.25 (full dwell on target); ACM modes 0.4 (short range); LPI modes 0.8 (reduced emission detectability) |
| Altitude of radar | Radar horizon, not the formula, bounds long-range look-down; handled by LOS (§16.9) |
| Resolution | Targets within one resolution cell (default 150 m range, 3° azimuth at 10 km) merge into one plot until separation (the "bricking" effect for formations; formation size is estimated from plot RCS/range extent) |

### 16.3.5 Radar modes

| Mode | Behaviour | Output |
|---|---|---|
| **RWS** (range-while-search) | Scans a volume (azimuth ±60° / ±30° / ±10°, 1/2/4/6 bars). Frame time `T_f = bars · az_width / ω_scan + retrace` (ω_scan ≈ 120°/s mechanical, 200°/s electronic) | Plots with range and closing rate; M-of-N track initiation (2 hits in 3 frames) |
| **TWS** (track-while-scan) | Same scan, maintains tracks for up to 12 targets, updates each frame; split dwell | Smoothed tracks; weapon cueing for datalink-capable missiles; RWR may show it as search (G4.5+) |
| **STT** (single-target track) | Full dwell on one target at 20 Hz | Best track quality; required for SARH illumination; RWR sees TRACK/LAUNCH |
| **ACM** (boresight / vertical / HUD / slewable) | Small fixed pattern for short-range acquisition (≤ 20 km) | Automatic STT on first detection |
| **Air-to-ground** (GMTI, SAR, sea search) | Moving-target indicator or terrain mapping (abstracted into "ground contacts" with RCS and speed gates) | See §21 |
| **Standby/EMCON** | No emission | Silent running |

AESA-era radars interleave modes (e.g., RWS with TWS on selected targets) and add LPI options.

### 16.3.6 Emitter registry

Every active radar registers an `Emission` record (`entity, band_class, power_class, mode, scan_center, scan_az, prf_class, lpi_factor`) with a global **EmissionIndex** (spatial hash); RWR/ESM sensors query the index (§16.8) so only nearby emitters are evaluated.

## 16.4 Radar cross-section

```jsonc
{ "rcs": { "az_deg_step": 10, "el_deg": [-90,-60,-30,0,30,60,90], "dbsm": [[ ... 36 values ... ], ... 7 rows ...],
           "bay_open_delta_m2": 0.5, "pylon_delta_m2": 0.1 } }
```

- Table in dBsm: 36 azimuth bins × 7 elevation bins, bilinear interpolation, converted to m² for the radar equation.
- Dynamic modifiers: `σ = σ_table + Σ σ_store,i + σ_bay_open + σ_damage` (all linear m²). External pylons, tanks and missiles add RCS (a "clean" stealth aircraft becomes visible when carrying external stores).
- Class defaults (mean RCS, design targets): Light 3 m², Multirole 4, Heavy 8, Interceptor 6, Strike 5, CAS 12, Bomber 25, EW 7, AWACS 100, Carrier 4, Stealth 0.01 (front sector 0.002), UAV 0.5, missile 0.04, bomb 0.05.
- Aspect lobes: head-on ≈ 0.3× mean, beam ≈ 2.5× mean, tail ≈ 0.7× mean for conventional aircraft (the table encodes this).
- Ground units and ships have table-based or scalar RCS.

## 16.5 Tracks, track quality, identification, and datalink

### 16.5.1 Track estimator

Each observer keeps a `TrackFile`. Per track: a constant-velocity Kalman filter on `[x,y,z,vx,vy,vz]` in BF with process noise from the class's maximum acceleration (`q = (0.5 · a_max)²`), and measurement noise from the sensor:

```text
σ_θ = θ_bw / (1.6 √(2 · SNR)),    SNR = 20 ρ        (monopulse angle accuracy; 13 dB at ρ = 1)
σ_R = c_R / √(SNR)                                    (range accuracy; c_R ≈ 30 m for a medium radar)
σ_Ṙ = c_D / √(SNR)                                    (closing-rate accuracy; ≈ 1.5 m/s)
```

**Track quality** `Q ∈ [0,1]` maps the position covariance trace to a normalised score (`Q = exp(−σ_pos/σ_ref)`); `Q ≥ 0.8` is "fire-control grade", `0.4 ≤ Q < 0.8` is "cue-grade" (weapons may launch with degraded accuracy and datalinked missiles update with larger error), `< 0.4` is "search-grade".

Track loss: coasting time without update before drop: search tracks 6 s, TWS 4 s, STT 2 s (mode-dependent); coasting predicts with the filter and grows covariance.

### 16.5.2 Classification and identification

Contacts progress through levels: **Detected** (blip) → **Tracked** (kinematics) → **Classified** (category: fighter/bomber/missile/…; by speed, RCS signature, flight profile, or radar NCTR cue at close range) → **Identified** (friend/foe/neutral; by IFF interrogation, datalink, visual). NCTR (non-cooperative target recognition) is an advanced-radar capability that classifies by engine modulation within 40 km at suitable aspect (G4.5+). IFF is on by default for allies; hostile identification by `declared hostile` rules per mission (rules of engagement, §25.9). Wrong-ID events can occur when classification is inferred rather than confirmed (design intent: unknown contacts are not shoot-on-sight in Realistic rule sets).

### 16.5.3 Datalink and shared picture

- A team's **shared picture** merges tracks from all datalink-equipped members and AWACS/ground radars using nearest-neighbour association with a Mahalanobis gate (χ² 99 % for 3 DOF = 11.34), preserving a stable global `trackId`.
- Link model: update rate 1–2 Hz, latency 0.3 s, link range bounded by LOS (k = 4/3), degraded by jammers (§17.4). Link quality scales track covariance.
- **AWACS service:** an AWACS (AI or human ABM) pushes its picture; recipients see contacts marked `DL`.
- **Realism gating (outline §144):** Easy shows full true positions on the tactical map; Normal shows datalink + own sensors + visual; Hard/Simulator show only what the aircraft's avionics provide (§59, §55).

## 16.6 Passive and visual detection

### 16.6.1 IRST

Passive IR search with `R_irst,ref` against a reference IR intensity `I_ref` (rear aspect, mil power); `R = R_irst,ref · √(I/I_ref) · τ(R)^{1/2}` solved numerically with `τ = e^{−βR}`; returns bearing/elevation only (an **angle-only track**). Range is estimated by triangulation over time (own motion), by data fusion with another sensor, or by laser ranging in short-range modes. IRST is unaffected by radar jamming and is the stealth-era counter to radar silence.

### 16.6.2 Visual detection

Gameplay visual detection uses apparent size and contrast (not screen pixels), so servers and AI evaluate it without rendering.

```text
θ = L_eff / R                                    apparent angular size,  L_eff = √(projected area of the target)
C(R) = C₀ · e^{−βR},   β = 3.912 / V_meteo       Koschmieder (2 % contrast threshold ↔ meteorological visibility V_meteo)
Detect  iff  θ ≥ θ_min (0.9 mrad ≈ 3 arcmin)  and  C(R) ≥ C_min (0.05)  and  LOS clear (terrain, clouds)
R_size = L_eff / θ_min;     R_contrast = ln(C₀ / C_min) / β;     R_vis = min(R_size, R_contrast)
```

Example: L_eff = 15 m, V_meteo = 40 km, C₀ = 0.5 → `R_size = 16.7 km`, `β = 9.78×10⁻⁵ m⁻¹`, `R_contrast = 23.5 km`, `R_vis = 16.7 km`. Identification (type recognised) at `R_vis/3`. Modifiers: sun glare ×0.5 within 20° of the sun; night ×(0.1…0.3) unless lit by lights, afterburner glow, moonlight, or flares; clouds block LOS if the integrated cloud density along the ray exceeds a threshold; contrails and smoke trails extend `L_eff`.

**Rendering link.** The renderer draws a minimum-size "spot" for any visually detectable contact beyond the point where its true projected size is below ~1.5 px, scaled to represent what a human eye would see (up to `R_vis`), so what the player sees matches what the AI "sees" (§19.9).

## 16.7 Missile-approach warning

MAWS (optional by avionics tier): detects missile plumes (IR) or active seekers (RF) out to 12–25 km depending on class; reports bearing and a time-to-impact estimate `t ≈ R/ V_closing` with ±20 % error; triggers the MISSILE state of the RWR and cues automatic countermeasures if enabled (§17.5).

## 16.8 Radar warning receiver (RWR)

### 16.8.1 States (outline §27)

| State | Meaning | Trigger |
|---|---|---|
| **SEARCH** | An enemy radar is illuminating you in a search mode | Emitter in RWS/TWS(+detectable) with you inside its main beam and `R ≤ R_rwr` |
| **TRACK** | An enemy radar has a track on you | STT, or TWS "track" (as per era), or ground fire-control radar lock |
| **LAUNCH** | A missile was launched at you | Launch event from an emitter with your track, or a visual/IR launch cue within MAWS range |
| **MISSILE** | A missile is actively threatening you | Missile entity targeting you (active seeker on or within `t_impact < 30 s`) |

### 16.8.2 Detection of emissions

One-way link: an RWR detects a radar farther than the radar detects a target. Gameplay rule: `R_rwr = k_rwr · R_emitter,ref · (P_eff / P_ref)^{1/2}` with `k_rwr` 2.0 (G3), 2.5 (G4), 3.0 (G4.5/G5) — i.e. the RWR typically sees an emitter 2–3× farther than that emitter would detect a 5 m² target. LPI emitters multiply `R_rwr` by `lpi_factor` (0.3–0.5). LOS (k = 1.33) must exist.

### 16.8.3 Capability tiers

| Tier | Information |
|---|---|
| G3 | Direction in 8 sectors, threat class (fighter/SAM/AAA) by audio signature |
| G4 | Bearing ±5°, threat ID (specific radar type), mode (search/track/launch), priority list |
| G4.5 | Bearing ±2°, ESM-derived range estimate via triangulation/amplitude, lethality ring on map |
| G5 | Fused ESM with passive geolocation (cooperative), threat overlay in the tactical map |

### 16.8.4 Display and audio

Scope with threat symbols (letter/number), priority ordering, blinking for new threats; **audio cues**: search = soft periodic tick at the radar's scan rate; track = steady tone; launch = rapid warble; missile = continuous alarm; direction cues are spatialised (§56). An RWR "volume/mute" and "threat-only" filters exist (accessibility visual-only mode adds screen-edge indicators, §57).

## 16.9 Line-of-sight service and terrain masking

Terrain masking is a core mechanic (outline §28): mountains block radar; low-level flight shortens detection; valley routes; breaking locks by diving behind ridges.

### 16.9.1 LOS algorithm

For endpoints `A, B` at heights `h_A, h_B` above the ellipsoid separated by ground distance `S`, an intermediate point at distance `s` from `A` is visible if

```text
h_terrain(s) + s (S − s) / (2 k R)  ≤  h_A + (h_B − h_A) · s / S − clearance
```

where `k = 1.0` for visual/IR LOS and `k = 4/3` for radar/radio (standard refraction), `R = 6 371 000 m`, and `clearance` accounts for target size (default 0). Terrain heights are from the base DEM (ADR-15). The bulge term makes the Earth itself a horizon: e.g. two aircraft at 100 m AGL over flat ground are mutually beyond radar LOS at ≈ 82 km (2 × 41.2 km).

### 16.9.2 Acceleration structure

1. **Max-mip pyramid:** each terrain tile stores, per 8×8 post block, the maximum height; a hierarchical march skips blocks whose max height is safely below the ray (+ bulge).
2. **Adaptive step:** at most 512 samples per query, step = `max(post spacing, S/512)`, with refinement at the first near-contact.
3. **Query cache:** results are cached on the quantised pair `(cell(A), cell(B))` with 250 m cells and 0.5 s TTL for non-static endpoints.
4. **Horizon profiles for static emitters:** SAM sites, ground radars, ships' home positions precompute a polar horizon profile (720 azimuth bins × range steps of 250 m up to the instrumented range; storing the maximum elevation angle blocked at each range). A target at (azimuth, range, elevation angle ε) is visible iff `ε ≥ horizon(azimuth, range)`. Memory ≈ 1.4 MB per emitter for 250 km range. Computed in a worker at mission load (≤ 100 ms per emitter).

### 16.9.3 Clouds, smoke and terrain-following gameplay support

- Clouds block visual/IR LOS (integrated optical depth > 2); radar passes (with weather attenuation).
- **Radar altimeter** and **terrain-following** (opt-in autopilot, §50.4): hold AGL 60–300 m using look-ahead terrain profile at 2 Hz.
- **Ground Proximity Warning:** "TERRAIN" when predicted ground contact along the velocity vector inside 8 s; "PULL UP" inside 4 s, accounting for current g-capability (not infinite).

## 16.10 Avionics and cockpit information (outline §95)

### 16.10.1 Fidelity tiers

| Tier | HUD | MFDs | Radar | Intended |
|---|---|---|---|---|
| **A – Arcade-lite** | Full overlay (all info), floating target markers | none | Simplified scope + auto-lock cues | Easy |
| **B – Standard** (default) | Real-style HUD, target designator box, DLZ, RWR | Radar, stores, tactical map pages | Realistic modes, simplified controls | Normal |
| **C – Full** | HUD with mode logic | Complete page set, per-aircraft | Full mode set, manual gain/range/azimuth/bar | Hard/Simulator |

### 16.10.2 Display pages

**HUD** (speed, altitude, heading, flight-path vector, g, AoA, weapon, ammo, target info, objective cue, fuel, RWR cues). **Radar page** (B-scope / PPI with range scale 10–320 km, azimuth sector, cursors, tracks, closing rate). **Stores page** (stations, counts, selection, master arm, jettison). **Tactical map page** (own-ship, tracks, threats, waypoints, battlefield boundary; content gated by realism, §55). **Engine/fuel page**, **CM page**, **Systems/damage page** (§18), **Navigation page** (waypoints, ETE), **Targeting-pod page**. The set is declared per aircraft in `cockpit_layout.json` (data): pages, which MFD hosts which page, and softkey bindings.

### 16.10.3 Interaction

Default input is keyboard/HOTAS bindings (§58); an optional **clickable cockpit** (ray-pick switches) is a `P10` feature. All avionics modes are accessible by bindings with an on-screen "cheat sheet" overlay.

## 16.11 Information-warfare rules by realism

| Realism | Source of contacts | Friend/foe marks | Distance/closure info |
|---|---|---|---|
| Easy | True picture, labelled | Always | Always |
| Normal | Own sensors + datalink + visual | Via IFF/datalink | From sensors |
| Hard | Own sensors + datalink; no free labels | IFF only on interrogation | From sensors |
| Simulator | As above + imperfect ID; jamming affects display | Interrogate to ID | Range via track only |

## 16.12 Performance

Budget (Recommended, Large battle): ≤ 1.2 ms per sim tick for all sensing, ≤ 0.3 ms for RWR/ESM. Techniques: pair culling by `R_max_possible` using the spatial hash; staggered 10 Hz scheduling; LOS cache; per-tier sensor rates (Tier 3 AI uses statistical detection by range thresholds, no LOS ray march except for the final engagement step); SIMD-friendly structure-of-arrays loops in Rust/WASM; sensor work packaged as parallel jobs when SharedArrayBuffer threads are available (ADR-17).

## 16.13 Edge cases

| Case | Handling |
|---|---|
| Two targets in one resolution cell | One plot; splits when separation > cell; NCTR shows "multiple" |
| Target at the radar's own altitude | Treated as look-up/neutral; no notch |
| Terrain data missing | Falls back to coarser LOD (conservative masking: assumes the higher of the two samples), never no-block |
| Time acceleration | Sensor rates scale with sim ticks; AI reactions unaffected |
| Friendly radar emission | Reveals position to enemy RWR/ESM; EMCON policy trades awareness for stealth |
| Chaff/decoys | Create false plots (§17.5) |
| Network | Sensor results are authoritative on the server; clients render the tracks the server sends for their aircraft (§33.6) |

## 16.14 Acceptance criteria

- `AC-16.1` Radar scaling table (§16.3.1) reproduced within 0.5 % by the implementation for the nine listed RCS values.
- `AC-16.2` LOS tests: flat Earth with k = 4/3, observers at 100 m AGL: LOS cut-off at 82.4 km ± 1 km; for an observer at 100 m AGL, a 3 km ridge midway (at 40 km of an 80 km path, bulge term 94 m) blocks a target at 500 m but not a target at 8 km altitude (ray height at the ridge 4 050 m > 3 094 m), per the bulge formula.
- `AC-16.3` Notch behaviour: a target flying exactly perpendicular at 250 m/s in look-down from a G4 radar has `P_d` multiplied by 0.02; tilting by more than 14° removes the penalty.
- `AC-16.4` RWR states fire in the correct order for a scripted engagement (SEARCH → TRACK → LAUNCH → MISSILE) with timings ±0.2 s.
- `AC-16.5` Visual detection example in §16.6.2 gives `R_vis` = 16.7 km ± 0.2 km.
- `AC-16.6` Sensor tick time ≤ 1.2 ms for 100 aircraft + 40 ground emitters (Recommended profile, native and WASM).
- `AC-16.7` In a blind test, ≥ 80 % of players can explain, after one mission, why a lock broke when they dove behind a ridge.

---

# 17. ELECTRONIC WARFARE

`P10 · M3 (countermeasures) / M6+ (advanced EW)` — Countermeasures and radar/missile warning are core combat (outline §25, built in M3). Advanced electronic warfare — jamming, emission detection, deception, seeker interference, electronic support and dedicated EW aircraft (outline §143) — creates a distinct tactical role and is staged after the core combat loop.

## 17.1 Intent

- A distinct role: the EW aircraft shapes what others can see and shoot.
- Everything is a trade: jamming reveals your position; silence blinds you; decoys are finite; stealth is conditional.
- Understandable: every EW effect has an on-screen explanation (burn-through range ring, strobe on radar scope, "jamming" icon on affected contacts).

## 17.2 Taxonomy and implementation map

| Branch | Capability | Section |
|---|---|---|
| **Electronic Support (ES/ESM)** | Detect, classify, and geolocate emitters | §17.6 |
| **Electronic Attack (EA)** | Noise jamming, deception jamming, false targets, stand-off jamming, comms/datalink jamming, GPS denial | §17.3–17.4 |
| **Decoys & expendables** | Chaff, flares, towed decoys, active expendable decoys, decoy drones | §17.5 |
| **Electronic Protection (EP/ECCM)** | Frequency agility, LPI, sidelobe blanking, home-on-jam, burn-through | §17.3.4 |
| **Emission control (EMCON)** | Silent running, limited emissions | §17.6.3 |
| **SEAD/DEAD interplay** | ARMs vs. SAM operator counter-play | §17.7, §21.5 |

## 17.3 Jamming model

### 17.3.1 Physical basis

For a jammer at range `R_j` from the victim radar and a target at range `R_t`, the signal-to-jam ratio is

```text
S/J = ( P_t G_t σ B_j R_j² ) / ( 4π P_j G_j B_r R_t⁴ )
```

(`B_j` jammer bandwidth, `B_r` radar receiver bandwidth.) For a **self-screening jammer** (SSJ: the target carries the jammer, `R_j = R_t = R`) this becomes `S/J = P_t G_t σ B_j / (4π P_j G_j B_r R²)`, falling as `R⁻²`, whereas the target's thermal signal-to-noise falls as `R⁻⁴`; hence a **burn-through range** `R_bt` exists inside which the radar again detects the target:

```text
R_bt = √( P_t G_t σ B_j / ( 4π P_j G_j B_r · (S/J)_min ) )
```

Jamming is therefore strongest at long range and loses to the radar as the target closes; larger RCS, more radar power, narrower jammer bandwidth, and ECCM all increase `R_bt`.

### 17.3.2 Game model

The sim does not carry watts; it carries **relative effective radiated power (ERP)** values and calibrates so the formulas give intended ranges. For each (radar, jammer, target) triple:

```text
ρ_thermal = (R_det / R_t)^4                                     (from §16.3.2)
ρ_jam     = κ · ( ERP_r · σ · (B_j/B_r) · R_j² ) / ( ERP_j,eff · R_t⁴ ) / (S/J)_min
ρ_eff     = min( ρ_thermal , ρ_jam )                             → P_d(ρ_eff) as in §16.3.2
ERP_j,eff = ERP_j · G_j(θ) · (1 − eccm_radar) · coverage_factor
```

`κ` is solved per jammer so that `R_bt = R_bt,ref` when `σ = σ_ref` against the reference radar (authoring knob, e.g. a self-protect pod `R_bt,ref = 12 km` against a Medium radar). `G_j(θ)` is the jammer antenna gain toward the radar (full for main-beam, 0.3 for sidelobe/off-axis geometry; a pod's coverage cone depends on its mount). Defensive jamming from a *stand-off jammer* uses `R_j ≫ R_t` so the numerator grows by `R_j²/R_t²` terms; the main visible effect is a **strobe**.

### 17.3.3 Effects on the victim

| Jam type | Effect | Counter |
|---|---|---|
| **Noise (barrage/spot)** | Raises the noise floor: detection becomes `ρ_jam`-limited; on the radar scope a noise **strobe** appears along the jammer bearing (width ±beamwidth for main-beam, wider for sidelobe) | Burn-through (close range), triangulation, home-on-jam, IRST, multiple radars |
| **Deception: range-gate pull-off (RGPO) / velocity-gate pull-off (VGPO)** | Against a target in STT, a false return walks the track gates away; track breaks after a pull-off time unless ECCM | Frequency agility, track-on-jam, switch to TWS/IR |
| **False targets (ghost plots)** | `N` ghost plots appear near the true bearing at random ranges for 2–5 s; contaminate TWS file (ghosts need confirmation) | NCTR, multi-frame correlation, datalink cross-check |
| **Angle deception (cross-eye/cross-pol)** | Increases angular noise `σ_θ ← σ_θ · (1 + 3·ρ_dec)` on the affected track (lock-on-lines drifts) | ECCM monopulse processing |
| **Stand-off jamming** | Denies a sector (strobe) to the victim radar; stand-off jammer is itself a target for HOJ | Fly outside the strobe, ARM/HOJ shots |
| **Comms/datalink jamming** | Reduces link quality (§17.4) | Frequency hopping (G4.5+), line-of-sight relays |
| **GPS denial** (optional rule) | GPS/INS munitions CEP grows with time under denial | INS-only navigation, anti-jam kits |

Deception effectiveness: `p_break/s = clamp(k_d · (ρ_dec − 0.5), 0, 0.5)` while a deception program is active against an STT track, with `ρ_dec` computed like `ρ_jam` but with the deception mode's efficiency multiplier; ECCM-equipped radars multiply `k_d` by `(1 − eccm)`.

### 17.3.4 Electronic protection (ECCM)

Radar `eccm` ∈ [0, 1] (design: G3 0.1, G4 0.3, G4.5 0.55, G5 0.7). Features that play out in the model: *frequency agility* (reduces spot jamming), *sidelobe blanking* (reduces sidelobe-jam coverage), *home-on-jam* (active seekers switch to angle-only guidance when jam-limited, preserving a hit chance at degraded accuracy), *LPI waveforms* (reduce RWR range, §16.8), *wide-band processing*.

### 17.3.5 Power budget and heat

Each jammer has a power budget `P_total` split among beams/emitters; effective ERP per emitter `ERP_j = P_total · N^{-0.8}` for `N` simultaneous targets (diminishing returns). Continuous operation above `duty_max` (default 0.6 over any 120 s) forces a 20 s cooldown unless the pod is liquid-cooled (`cooling: good`). Active jamming is itself an emission: enemy ES/RWR sees it at long range (`R_rwr` rules, §16.8), enabling HOJ/ARM shots and triangulation.

## 17.4 Datalink and communications jamming

`link_quality = 1 / (1 + J/S_link)`, with `J/S_link` from the same ERP/range logic against the receiving node. Effects: lower quality increases track covariance and latency; below 0.2 the link drops (AWACS picture lost, missile midcourse updates denied); radio voice gets static (cosmetic) and AI squadrons beyond voice range of their leader fall back to autonomous tactics (§20.5).

## 17.5 Countermeasures and decoys (outline §25)

### 17.5.1 Warning and dispense logic

Aircraft have a radar-warning receiver (§16.8), optional missile-approach warning (§16.7) and dispensers with a bucket allocation (chaff, flare, decoy counts within dispenser capacity). **Dispensing modes**: *Manual* (key/button), *Semi-automatic* (programs offered on warning; pilot confirms), *Automatic* (MAWS/RWR-triggered programs). Programs are data:

```json
{ "cm_programs": [
  { "id": 1, "name": "IR MISSILE", "flare": { "n": 2, "interval_s": 0.4, "repeat": 3, "gap_s": 1.0 }, "chaff": null },
  { "id": 2, "name": "RADAR MISSILE", "chaff": { "n": 2, "interval_s": 0.5, "repeat": 3, "gap_s": 1.0 }, "flare": null },
  { "id": 3, "name": "MIXED", "flare": { "n": 1, "interval_s": 0.5, "repeat": 4, "gap_s": 1.0 }, "chaff": { "n": 1, "interval_s": 0.5, "repeat": 4, "gap_s": 1.0 } },
  { "id": 4, "name": "BURST", "flare": { "n": 6, "interval_s": 0.1, "repeat": 1 }, "chaff": { "n": 6, "interval_s": 0.1, "repeat": 1 } } ] }
```

Reference capacity (Kestrel): 120 chaff + 60 flares; ejection velocity 10 m/s rearward/downward.

### 17.5.2 Flares

A flare is a short-lived IR emitter entity: intensity `I_f(t)` rises to its peak over 0.2 s then decays linearly to zero at `t_burn = 4 s`; peak is 2–3× the aircraft's afterburner intensity (design). The flare is decelerated by drag relative to the aircraft with time constant ≈ 1.2 s, so it separates aft/below while the aircraft continues.

Seeker decision at each update: the seeker computes a weighted centroid of all sources inside its instantaneous FOV; if sources become resolvable (angular separation > `θ_res`, default 3 mrad), it must choose one:

```text
P(choose flare) = σ_logistic( k · (I_f / I_t − 1) ) · (1 − r_ircm)
     k = 3 ;  r_ircm = IRCCM rejection: G3 0.1, G4 0.4, G4.5 (imaging) 0.8
```

Pilot actions that tilt the odds: pulling the engine to idle/mil lowers `I_t`; turning perpendicular increases separation; releasing multiple flares in sequence; terrain/cloud/sun backgrounds (seeker confusion modifiers). Imaging seekers add a shape-consistency term that further rejects flares (`r_ircm`).

### 17.5.3 Chaff

A chaff bundle spawns a cloud entity with RCS blooming over 1.5 s to `σ_peak` (20–50 m² per bundle at the reference band), decaying over 20–30 s as it disperses; the cloud decelerates relative to the aircraft with time constant ≈ 2 s and drifts with the wind (so its ground speed ≈ wind speed, near zero). Interaction with radar/seekers:

- A seeker or STT track gates on the target's predicted closing rate; chaff with a different closing rate is rejected unless the target is **in the Doppler notch** (beaming) — then the target's own return vanishes and the seeker may capture the chaff: `P_capture = 0.6 · bloom(t)` (target in notch) or `0.1 · (1 − eccm) · bloom(t)` (otherwise).
- This reproduces the defensive combination: **dive + beam + chaff** (and why it works less in look-up geometry and against high-ECCM seekers).
- Chaff also creates plots in search radars (false contacts) which are filtered by velocity gating after 2–3 frames.

### 17.5.4 Towed decoys and active expendables

| Item | Behaviour |
|---|---|
| **Towed decoy (TD-1)** | Trails 80 m behind; an RF repeater; active-radar seekers and STT tracks may capture it. `P_capture = 0.5` for G3/G4 seekers at terminal if the missile lacks angular discrimination (`monopulse: false`); 0.2 for G4.5; the cable is cut automatically when a missile is captured or by the pilot |
| **Active expendable decoy (AED)** | Launched from the dispenser; short-lived (10 s) jammer/repeater; creates a false closest-approach point |
| **Decoy drone (ADM)** | Store; flies a pre-programmed route at 0.6–0.8 Mach emulating a fighter RCS (5 m²) and radar emission-induced responses; used to saturate SAM engagement and trigger emitters |

### 17.5.5 Effects of damage and supply

Dispenser or bus damage reduces capacity or disables a side. Campaign resources track chaff/flare stock (§39). Sandbox default is full capacity per loadout with a "CM unlimited" option (§30).

## 17.6 Electronic support (ESM) and emission control

### 17.6.1 Detection and ID

ES sensors (RWR for fighters; ESM suites for EW aircraft/ships/SAM sites) query the `EmissionIndex`: a contact record includes bearing, signal class, mode, and—depending on suite tier—emitter identity. Detection range uses the same one-way formula as RWR with an ES gain factor (EW aircraft `k_es` = 3.5).

### 17.6.2 Geolocation

Two or more bearing observations (from different aircraft over time, or from a single aircraft's motion) produce a position fix by least-squares intersection (Stansfield estimator): for bearings `θ_i` from known positions `p_i`, minimise `Σ (θ_i − atan2(...))² / σ_θ,i²`; the output is a position estimate with a **confidence ellipse** (σ_θ = 0.5° EW suite, 2° RWR). Ellipses shrink as bearing geometry (baseline) improves; the Magpie's EWO can request orbit geometry hints ("fly crossing leg for better fix"). Geolocated emitters appear on the tactical map as ellipses and can be cued to ARMs or strike aircraft.

### 17.6.3 Emission control

| EMCON level | Behaviour |
|---|---|
| **Active** | Normal emissions |
| **Limited** | Radar emits only in STT/ACM bursts (≤ 10 s) or LPI modes; datalink silent; RWR/IRST fine |
| **Silent** | No emissions; rely on IRST, visual, passive datalink reception (receive-only) and ESM |

SAM operators and AI fighters use EMCON tactically (§17.10). Players choose via loadout default and a quick key.

## 17.7 SEAD/DEAD and ARM interplay

Anti-radiation missiles (§15.9.4) home on emitters; SAM operators respond by shutting down radars when an ARM launch is detected (a visible tactic: SAM goes quiet; ARM flies to memory point; operator turns radar back on when threat passes). Dedicated EW support (jamming SAM search radars to hide strike aircraft; spoofing) completes the triangle (§21.5).

## 17.8 EW console and roles

**EW officer (EWO)** is a human or AI second-seat role (Magpie) or pilot-assist function:

| Function | UI |
|---|---|
| Spectrum display | Emitters plotted by bearing and signal strength; mode and type tags; threat priority |
| Jam assignment | Drag a jammer beam to an emitter; select NOISE / SPOT / RGPO-VGPO / FALSE TGT |
| Power allocation | Slider per beam; diminishing returns shown |
| Timeline | Schedule "jam on/off" windows with the strike package (visual ruler synchronised to mission timeline) |
| ESM list/map | Geolocated emitters with ellipses; "request geometry" |
| EMCON control | Package-level (suggests flight-wide EMCON) |

When there is no human EWO, an **AI EWO** performs priority assignment with player override (single-seat EW aircraft use "auto-EW" with one-button toggles). In co-op the EW aircraft is a dedicated role (§32.4) with team-wide effects: friendly radar **interference** is possible in Simulator realism (friendly jammers in a friendly radar's beam degrade it).

## 17.9 Information warfare

Per outline §144, awareness is a resource: jammers can deny the picture; false targets can contaminate the shared picture until correlation resolves them; comms jamming degrades coordination; the player's UI must show "your picture is degraded" with cause (icon + text) so it feels like a fair, comprehensible effect rather than a bug.

## 17.10 AI behaviour with EW

| Actor | Behaviour |
|---|---|
| SAM operator | Uses ECCM modes when jammed; switches to low-PRF/LPI; shuts down radar on ARM launch and returns after the threat passes or a timeout; coordinates with a battery's second radar |
| Fighter | Uses jammer/CM per programs; picks EMCON by doctrine; closes range to burn through; uses IRST/HOJ when jammed |
| AWACS | Pushes picture; reports jamming ("Picture degraded, jammer bearing 270") |
| EW aircraft AI | Flies orbit/racetrack at stand-off distance on the strike package's ingress side; prioritises strongest threat emitters |

## 17.11 Tuning knobs

`κ`/`R_bt,ref` per jammer; `ERP_r` per radar class; `eccm`; `k_d`; flare intensity/decay; chaff σ_peak/decay; `P_capture`s; towed-decoy efficiency; EMCON thresholds; `k_es`; ellipse σ.

## 17.12 Edge cases

| Case | Handling |
|---|---|
| Multiple jammers | Sum ERP in the victim's direction (power addition with a cap) |
| Jammer destroyed | Effect ceases immediately (the contribution term disappears) |
| Friendly jammer vs friendly radar | Applies only when `friendly_interference` is on (Simulator) |
| Flare in clouds | Reduced visibility factor 0.6 for the seeker (IR attenuation) |
| Missile switches to HOJ | Guidance continues angle-only with larger noise; lethal only within reduced `R_lethal` accuracy (miss distance scaled up) |
| Too many CM entities | Capped by `max_cm_entities` (default 600) — oldest despawn first; counted against the projectile pool |

## 17.13 Acceptance criteria

- `AC-17.1` Burn-through test: for a Medium radar and a self-protect pod (R_bt,ref = 12 km at σ_ref), the computed burn-through range for σ = 20 m² is 12 × (20/5)^{1/2} = 24 km (±1 %) and detection inside it recovers to P_d ≥ 0.5 at R_bt.
- `AC-17.2` Flare test: for a G4 IR seeker and a flare 2.5× target intensity at 2× the resolution angle, the capture probability over 10 000 trials is within ±3 % of the formula value.
- `AC-17.3` Chaff + notch scenario yields ≥ 55 % capture vs G3/G4 seekers and ≤ 15 % without the notch condition in look-up.
- `AC-17.4` Geolocation: with two platforms 40 km apart observing a stationary emitter 100 km away with σ_θ = 0.5°, the position error ellipse semi-major axis is within 20 % of the analytic value.
- `AC-17.5` UI: every jammed contact shows a visible cause, verified by UI test (no unexplained loss of tracks).

---

# 18. DAMAGE MODEL

`P3 · M3` — Aircraft use **modular damage** (outline §29): components with health, armour and behaviour that alter the flight model, sensors, weapons and systems. Collisions and crashes (outline §30) are believable without simulating unnecessary debris.

## 18.1 Intent

- Damage tells a story: smoke trail, a vibrating airframe, a failing system, a decision (fight or run?).
- Damage changes performance in ways the player can feel and diagnose (reduced thrust, roll to one side, leaking fuel gauge).
- Outcomes are rarely binary: survivable damage is common; catastrophic failure is possible.
- Cost: hit detection runs on the server only; clients receive compact damage states (§18.9).

## 18.2 Hit volumes

Each aircraft has 25–40 **hit volumes** (oriented boxes, capsules, or small convex hulls) authored from `hit_*` meshes (§12.6), each assigned to exactly one component. Volumes are in BODY; per tick the Sim Core transforms the aircraft pose and updates broadphase AABBs in the spatial hash.

| Test | Method |
|---|---|
| Projectile vs aircraft | Swept segment vs. oriented box/capsule; returns entry/exit points and ordered list of volumes along the segment; penetration residual energy chooses how many volumes are damaged (AP may pass through 2) |
| Missile fuze proximity | Distance from the fuze point to the nearest hit volumes; cone filter (§15.6) |
| Bomb/blast | Radius query on hit volumes (§15.9.6) |
| Aircraft vs aircraft | Volume–volume overlap (OBB–OBB SAT) between hit volumes of two aircraft |
| Aircraft vs terrain/sea | Sample points (gear contact points, wingtips, nose, tail, belly hull points) vs base DEM and sea surface (§18.8) |

Hit volumes are separate from the render mesh (ADR-16); they are intentionally slightly generous (+10 %) so hits feel fair.

## 18.3 Components, health, and armour

Components (reference fighter has ~28; others vary):

| Group | Components | HP (relative) | Armour class |
|---|---|---|---|
| Propulsion | Engine (per engine), inlet, nozzle, fuel pump | 100 | Medium |
| Airframe | Wing inboard L/R, wing outboard L/R, horizontal stab L/R, vertical stab(s), fuselage forward/mid/aft | 80–140 | Light–Medium |
| Control | Ailerons/flaperons L/R, elevators/stabilators L/R, rudder(s), leading/trailing-edge flaps, airbrake | 40–60 | Light |
| Landing gear | Nose, main L/R, gear doors | 50 | Light |
| Fuel | Tanks (forward, centre, aft, wing L/R, external) | 60–120 | Light |
| Sensors | Radar, IRST, RWR antennas, targeting pod | 40 | Light |
| Cockpit | Canopy, pilot, seat/ejection system | 30 (pilot 100) | Light (canopy), Medium (tub) |
| Systems | Hydraulics A/B lines, electrical bus/generators, flight-control computer, avionics bay, cooling, APU | 40–60 | Light |
| Weapons | Each station + its store | 40 | Light |
| Hardened items | CAS-class armoured tub (pilot + critical lines): Heavy | — | Heavy |

**Armour class** gives per-damage-type resistance (kinetic, blast, fragment, fire): `dmg_applied = dmg_raw · (1 − R_class,type)`, with `R_light` 0.0–0.1, `R_medium` 0.25, `R_heavy` 0.6 (kinetic) / 0.4 (blast) (design targets).

**Damage state per component:**

| HP fraction | State | Typical behaviour |
|---|---|---|
| 100–70 % | Intact | Full function |
| 70–35 % | Damaged | Efficiency scaled by `hp^{0.7}`; intermittent faults; fire/leak chance |
| 35–1 % | Critical | Efficiency scaled by `hp`; high chance of progressive failure (fire, leak, jam) |
| 0 % | Destroyed | Component lost; sections detach (wing/tail); structure-linked cascade |

## 18.4 Systems model and cascades

Systems are small graphs: **hydraulic A/B** (pumps → lines → actuators), **electrical** (generators → buses → avionics, radar, sensors), **flight-control computer (FCC)**, **fuel feed**, **cooling**.

| Fault | Effect |
|---|---|
| Hydraulic A loss | Actuators on A lose power; rate limit reduced to `δ̇_max · (B_available)`; both lost → **manual reversion** (control surfaces move only by aerodynamic loads; practically uncontrollable except via RAT/emergency system) |
| Hydraulic leak | Pressure decays at `leak_rate · (1 − hp)` until empty |
| Electrical bus fail | Radar/MFDs/CM blank; HUD flickers; battery (10 min) maintains essential bus |
| FCC fail | Control law degrades **Standard → Direct** (no limiters; `sas_required` aircraft can become unflyable) |
| Engine damage | Thrust `η_damage = 0.2 + 0.8·hp`; at Critical fire chance 0.15/s; compressor stall chance at Damaged |
| Fuel tank breach | Leak `ṁ_leak = k_leak (1 − hp) √(fill)` (kg/s); trailing white vapour; may ignite if fire nearby |
| Fire | Compartment fire severity `F ∈ [0,1]` rising 0.1/s when ignited; extinguish by on-board system (one shot, `P_success = 0.6`) or manual shutdown; damage to adjacent components `F · 3 HP/s`; at `F ≥ 0.8` adjacent fuel tank explosion chance 0.2/s |
| Canopy loss | Drag ΔC_D0 +0.002, noise, pilot injury chance at speed |
| Pilot injury | Wounded: control input noise ×2, G tolerance −1 g; killed: aircraft lost |

Warnings follow aircraft-specific patterns: master caution, system-specific messages in the damage page and audio ("HYDRAULIC", "ENGINE FIRE").

## 18.5 Effects on flight and sensors

All effects flow through data-driven mappings applied to the models of §11:

| Component state | Applied modification |
|---|---|
| Wing section damage | `C_L,max` scaled by `0.7 + 0.3·hp`; asymmetry `ΔC_l = k·(hp_R − hp_L)`; at destroyed: section removed → effective wing area `S ← S − S_section`, mass and inertia updated (§11.9), asymmetric moments from missing lift; spin tendency increases |
| Horizontal/vertical stab | `C_mδe, C_nδr` scaled by `hp`; destroyed: loss of pitch or directional stability (departure) |
| Control surface | Effectiveness = `hp^{1.0}` (aileron etc.); jam chance at Critical: surface sticks at a random deflection within ±40 % of range |
| Engines | §11.8 `η_damage`; asymmetric thrust for multi-engine |
| Fuel | Mass changes via leaks; CG shift automatically |
| Radar damaged | Output power ×hp, scan sector narrowed; Destroyed → none |
| IRST/RWR/MAWS | Fidelity degrades (noise); destroyed → none |
| Gear | Fail to extend / stuck up → belly landing consequences (§18.8) |
| Weapon station | Store lost; HE stores can cook-off (blast within 10 m, §15.9.6) |

## 18.6 Warhead and gun damage resolution

**Gun hit:** `dmg = k_type · (½ m v²)^{0.7} · (1 − R_class) · cos(θ)`; AP has `k` 1.4× HEI vs heavy armour, HEI has incendiary chance on fuel/hydraulic components (`p_inc = 0.15` per HEI hit on fuel). Overlapping hit volumes along the path receive residual energy.

**Warhead:** per §15.6; component list within `R_lethal` and fragment cone each receive `base_dmg · f(x) · exposure · (1 − R_class,frag) · U(0.8, 1.2)`. Critical components adjacent to the burst take proportionally more. Blast overpressure damages light structures (control surfaces first).

**Destruction thresholds:** an aircraft is **destroyed** when (a) pilot killed, (b) wing or tail separation causing uncontrolled flight above `k` seconds, (c) fuel tank explosion, (d) engine fire not extinguished for 12 s with fuel nearby, (e) total airframe HP below 25 %, or (f) structural failure (§18.7).

## 18.7 Structural overload and overspeed

Per wing/tail `D_struct` ∈ [0, 1]:

```text
dD/dt = k_ov · (max(0, |n| / n_limit − 1.1))² + k_q · (max(0, q̄ / q̄_limit − 1.05))²       (overload, overspeed)
```

At `D ≥ 0.8` creaking/buffet cues and a 5 % per second failure chance; at `D = 1` the section fails (detaches). Instant failure occurs if `|n| > 1.5 n_limit` or `V > 1.2 V_ne` with probability `1 − exp(−3 Δt)` per second above threshold. Limits are per configuration (§14.4). Gear and flap over-speed extension limits similarly damage doors/surfaces.

## 18.8 Collisions and crashes (outline §30)

### 18.8.1 Terrain and water

| Event | Rule |
|---|---|
| Wheels-down landing | §11.13 gear model |
| Belly/wing/pod contact at low speed (< 30 m/s normal speed, < 20° pitch) | Skid: friction deceleration, damage rolls on contacted components; survivable |
| Contact at speed or steep angle (normal speed component > 30 m/s, or sink rate > 7 m/s with fuselage) | **Destroyed** → crater/explosion |
| Water ditching | Survivable if sink rate < 3 m/s and speed < 90 m/s and pitch within ±15°; else destroyed. Sinks in 15–60 s unless carrier-type flotation; pilot may eject earlier |
| Terrain flyby (wingtip contact) | Damage to wingtip, then cartwheel or loss of control depending on speed |

*Clarified in 0.2.2 (ADR-35):* until this chapter is built, the thresholds of §11.13.3 govern the wheels and the airframe on the ground (ground speed above 30 m/s, or sink above 7 m/s, destroys; below it a skid). The normal-speed wording above applies to terrain contact in flight and to water.

### 18.8.2 Aircraft-to-aircraft

Overlapping hit volumes produce an impulse using reduced mass `μ = m₁m₂/(m₁ + m₂)` and relative velocity `v_rel`: collision energy `E = ½ μ v_rel²`. Damage to each colliding component is proportional to `E` and its HP; a threshold (e.g. `E > 200 kJ` or tail/wing involvement) means destruction. Pieces that detach follow the break-up model.

### 18.8.3 Break-up and wreckage

On destruction the aircraft becomes a **Wreck** with a short phase of degraded flight (ballistic with a tumbling aerodynamic model from the post-stall tables) and spawns 4–8 debris pieces (wing sections, tail, engine, cockpit section) with simple ballistic + drag physics (no per-piece collision with active aircraft except the biggest pieces tagged `hazard` which can damage aircraft in their path, mainly for formation flying). Constraints:

- Total debris entities ≤ 200 (pool); excess culled by lowest priority.
- On impact: explosion VFX, crater decal, ground fire for 60–120 s, smoke columns visible at distance; wreck mesh persists as a static hull until the budget needs it (max age 120 s).
- Believable but cheap: the goal is a convincing crash, not full destruction simulation (outline §30).

### 18.8.4 Ejection and pilot fate

If a destroyed/uncontrollable aircraft still has a living pilot, ejection is available (auto-eject option on `sink rate` or uncontrolled spin). Ejection spawns a seat+pilot entity (parachute after 1.5 s), with survival probability a function of altitude and speed (`P_survive = 1` within the envelope; minimum 60 m AGL at level flight, above 0 with a rocket seat; high-speed ejection above 250 m/s EAS injures: `P = 0.7`). In sandbox it is cosmetic plus respawn rules. In the campaign it feeds pilot survival (recovered/captured/lost depending on territory control, §41).

## 18.9 Replication and storage

Per aircraft: component HP quantised to 4 bits (16 levels, 28 components = 14 bytes), fire/leak/flags bitmask (4 bytes), systems state (2 bytes). Sent as deltas at 10 Hz max or immediately on state change (reliable-channel event for destruction). The authority holds full-precision HP. Replay stores states and events (§52).

## 18.10 Player-facing options

| Option | Values |
|---|---|
| Damage taken by player aircraft | 0 % (invulnerable), 25 %, 50 %, 100 % (default), 200 %, 400 % |
| Damage dealt to enemies | 50 %–200 % |
| Fire/leak/system failures | On/Off |
| Structural failure | On/Off |
| Auto-eject | Off/Smart/On |
| Damage readout | Off/Minimal/Full wireframe |

## 18.11 Edge cases

| Case | Handling |
|---|---|
| Hit on already destroyed component | Damage redirects to adjacent (structure) or ignored |
| Two shooters kill simultaneously | First `kill` event in authoritative tick order wins; both credited with assist in scoring |
| Disconnected owner | Aircraft autopilot "hold" for 15 s then uncontrolled glide |
| Time acceleration | Damage events unaffected; fires/leaks scale by ticks |
| Aircraft with unusual layouts | Hit volumes defined in data; no assumptions on wings/tail |

## 18.12 Acceptance criteria

- `AC-18.1` Swept-segment hit detection finds 100 % of 1 000 m/s rounds passing through a hit volume at 60 Hz (shared with AC-15.8), and ≤ 0.1 % false positives for rounds missing by 0.2 m.
- `AC-18.2` A destroyed outer wing reduces C_L,max and area per §18.5 and the reference fighter rolls toward the damaged side at a rate within 15 % of the analytic asymmetric-lift estimate.
- `AC-18.3` Engine destroyed on a twin: remaining engine allows controlled flight at reduced performance (sustained level flight possible at 70 % of max speed or the test documents why not).
- `AC-18.4` Overstress test: a 12 g sustained pull at limit 9 g produces structural failure within the documented probability window.
- `AC-18.5` Crash test matrix (speed × angle × surface) matches §18.8.1 outcomes for 30 scripted cases.
- `AC-18.6` Debris cap (200) is never exceeded in a 100-aircraft mass-kill stress test, and frame time stays within 10 % of baseline.

---

# 19. AI SYSTEM

`P4 · M1 (basic) → M3 (combat) → M4 (mission-level)` — AI should be one of the strongest components of the game (outline §31). AI aircraft have objectives, awareness, tactics, fuel state, damage state, weapon preferences, threat assessment, morale and retreat behaviour. They do not simply fly at the player.

## 19.1 Intent

- Opponents who feel like pilots: they use energy, geometry, terrain, sensors, support and countermeasures.
- Allies who contribute: the player is not required to handle every threat (outline §35).
- Difficulty comes from **behaviour**, not from inflated health or damage (outline §32).
- Scale: up to 75 AI aircraft in a Large battle at Recommended settings, with budgeted cost per tier (§19.15).

## 19.2 Core principles (non-negotiable)

1. **Same physics.** AI aircraft in tiers T0–T1 run the *same* flight model, with the *same* flight-control law as the player, via virtual stick inputs (§19.12). T2/T3 use the parity-tested reduced models (§11.17). No teleporting, no speed cheats, no free energy (NN-5).
2. **Same sensors.** AI knowledge comes only from sensor events, datalink, and radio messages (§19.9). A `PerceptionAudit` test (§19.17) fails if any AI decision reads an entity's true state without a corresponding detection record.
3. **Skill changes behaviour.** Skill levels modify reaction time, attention, aim error, energy discipline, tactic repertoire, defensive accuracy and teamwork. They never multiply health, damage, speed or sensor range (§19.6).
4. **Reproducible variation.** Every AI pilot has a seeded RNG stream (`hash(session_seed, entity_id)`), so behaviours vary but are reproducible in tests.
5. **Fail-soft.** An AI in an invalid state (NaN, stuck) is reset to a safe behaviour (`Cruise`) and logged; it never crashes the sim.

## 19.3 Layered architecture

```text
STRATEGIC AI (campaign: where forces move)                    — event-driven (§35, §40)
        ↓ operations
MISSION AI / DIRECTOR (objectives, phases, reinforcements)    — 0.5 Hz (§25, §28)
        ↓ tasks
SQUADRON AI (assignments, formations, doctrine)               — 1 Hz (§20)
        ↓ orders to each pilot
PILOT AI (perception, assessment, tactic selection)           — 5 Hz
        ↓ goals
MANEUVER CONTROLLER (trajectory targets → virtual stick)      — 30 Hz
        ↓ stick/pedal/throttle
FLIGHT-CONTROL SYSTEM + FLIGHT MODEL (identical to player)    — 60/120 Hz
        ↑ reports (contacts, status, kills) flow upward
```

Each layer sees only the layer below's reports and its own sensors. Layers are separate Rust modules behind traits (`StrategicAi`, `MissionAi`, `SquadronAi`, `PilotAi`, `ManeuverController`) so they can be unit-tested and swapped (e.g. a scripted AI for tutorials).

## 19.4 Pilot AI: assessment and tactic selection

### 19.4.1 Engagement geometry

For ownship `S` and target `T` the pilot AI computes (each decision tick):

| Metric | Definition |
|---|---|
| Range `R`, closure `Ṙ` | `\|r\|`, `r·v_rel/\|r\|` |
| ATA (antenna train angle) | Angle between ownship nose and line of sight to target |
| AA (aspect angle) | Angle between target tail and line of sight from target to ownship (0° = we are dead astern, 180° = head-on) |
| HCA (heading crossing angle) | Angle between velocity vectors |
| Energy `E_s`, `P_s` | §11.12 for both aircraft (estimated for target from track data and class tables) |
| Turn room / geometry class | Offensive (target inside our nose cone and we are outside its), Defensive (reverse), Neutral, Head-on |

Classification rule (simplified): *Offensive* if `ATA < 60°` and `AA < 90°` and `R < R_wez`; *Defensive* if the target meets the same criteria on us; *Head-on* if `AA > 150°` and `ATA < 30°`; otherwise *Neutral*. `R_wez` is the weapon engagement zone for the best available weapon.

### 19.4.2 Tactics catalogue

| Group | Tactics |
|---|---|
| Transit | `Cruise`, `Patrol`, `Orbit`, `Escort` (weave/station), `Rendezvous` |
| BVR offence | `Intercept` (collision course with lead), `Commit`, `Crank` (offset to keep target in the missile cone while reducing closure), `Pump` (turn away to bleed missile energy then return), `Drag` (hot-turn egress), `Flank/Pincer` |
| Merge/WVR offence | `LeadPursuit`, `PurePursuit`, `LagPursuit`, `HighYoYo`, `LowYoYo`, `BarrelRollAttack`, `GunAttack`, `EnergyZoom`, `VerticalFight` |
| Defence | `BreakTurn`, `Jink`, `Scissors`, `SplitS`, `DefensiveSpiral`, `Notch` (beam + descend vs radar missile), `FlareBreak` (IR missile), `TerrainMask`, `Extend` |
| Ground/naval attack | `DiveBomb`, `LevelBomb`, `Toss`, `PopUpAttack`, `StrafeRun`, `RocketRun`, `StandoffLaunch`, `SEADLoft` |
| Support | `Refuel` (tanker join + contact), `Land`, `Takeoff`, `RTB`, `Scramble` |
| Special | `Retreat` (morale), `Eject`, `Kamikaze` (doctrine-gated, off by default) |

### 19.4.3 Utility-based selection

Tactic selection is **utility AI with commitment**:

```text
U_i = Σ_k w_{i,k} · curve_k(x_k)           x_k: normalised considerations (threat, geometry, fuel, weapons, orders, morale, ...)
choose argmax over eligible tactics (gated by skill repertoire §19.6 and doctrine)
hysteresis: a new tactic must exceed the current by margin δ (0.1) or the current must be finished/invalid
commitment: a tactic holds for at least t_min (BVR 5–10 s, BFM 1.5–3 s)
```

Considerations include: *missile threat level*, *offensive/defensive state*, *weapon availability and DLZ*, *energy advantage*, *support available (wingman, AWACS)*, *fuel margin*, *orders from squadron*, *morale*, *terrain proximity*, *objective pressure*. Curves are data (piecewise-linear). Each tactic is implemented as a small **behaviour tree** (selector/sequence) producing goals for the maneuver controller and discrete actions (radar mode, weapon release, countermeasures, radio calls).

### 19.4.4 Predictive maneuver selection (BFM core)

Within the BFM tactics, the pilot AI chooses a maneuver by *forward simulation*:

1. Candidate commands: `n ∈ {n_min_desired…n_cap}` (5 values) × bank/roll targets `μ ∈ {-135°…+135°}` (7 values) = 35 candidates (plus "extend straight", "unload").
2. For each, simulate ownship for horizon `H = 6 s` with the Tier-2 reduced model at 30 Hz (§11.17); assume the target continues its current manoeuvre type (pursuit / escape / constant-turn) from its tracked state.
3. Score `J = w₁·(−|ATA_end|) + w₂·f_range(R_end) + w₃·(E_s,end − E_s,start)/E_ref + w₄·offence_bonus − w₅·terrain_risk − w₆·overstress`.
4. Execute the first segment (0.5 s) of the best, replan at 5 Hz.

Cost: ≈ 35 candidates × 180 steps ≈ 6 300 Tier-2 steps per decision (≈ 0.1 ms in Rust/WASM); decisions are staggered across the 12 ticks of the 5 Hz window, so 40 engaged AIs cost ≈ 0.35 ms/tick (§19.15).

**Intercept (lead) solution.** To fly toward the predicted point where speed `V_s` meets a target with position offset `r = p_t − p_s` and velocity `v_t`, solve `(v_t·v_t − V_s²) t² + 2 (r·v_t) t + r·r = 0` for the smallest positive `t`; the aim point is `p_t + v_t t`. If no positive root exists, fly pure pursuit.

### 19.4.5 Weapons employment

| Weapon | Employment rule |
|---|---|
| Guns | Fire when `R < R_gun` (≈ 1 000 m), aim error < threshold (skill-based), and no friendly in the line of fire |
| SRM (IR) | Lock tone + inside DLZ with `R < f_fire · R_max,aspect`; prefers rear-hemisphere shots for G3 seekers |
| MRM/LRM | Fire at `f_fire · R_max,aspect` where `f_fire` is doctrine/skill-dependent (Recruit 0.95 – wasteful shots at the edge; Ace 0.55–0.65 – high-probability shots); respects `R_ne` for self-preservation; supports SARH by maintaining STT |
| Salvo | Fire 2 missiles at targets with high `TV` or when evasion probable; ripple interval 1–3 s |
| Preserve | Keeps ≥ 1 SRM for the merge unless doctrine says otherwise; hoards LRMs for high-value targets (bombers, AWACS, tankers) |
| A2G | Weapon chosen by target class (soft/armoured/structure/radar/ship) and defences; standoff weapons preferred when SAMs active |

Post-launch behaviour: *support* (SARH), *crank* (hold target at the edge of the radar gimbal while maximising missile energy), *F-pole awareness* (track the opponent's missile's position), and *pump/drag* after the missile goes active.

### 19.4.6 Missile defence

Pipeline: **detect** (RWR LAUNCH/MISSILE, MAWS, visual) → **classify** (IR vs radar via cue) → **react after the skill delay** → **maneuver** → **countermeasures**:

| Threat | Response |
|---|---|
| Radar missile, range > 10 km | Turn to the notch (aspect ≈ 90°) with descent; dispense chaff when closure < 8 km; keep out of the missile's gimbal cone |
| Radar missile, final 5 km | Max-g break into the missile; chaff + flare burst |
| IR missile | Flare program; high-g break perpendicular to the missile line; reduce engine to mil if safe |
| Unknown | Notch + mixed CM |
| Gun | Break and jink; scissors if overshoot |

Execution quality is skill-dependent: reaction delay (§19.6), notch aspect error σ (Ace 2° … Recruit 25°), timing error of CM (Recruit ±1.5 s).

## 19.5 Energy awareness

AI use the baked energy-manoeuvrability tables of their own aircraft (and the class tables of the target) to avoid the classic AI failure of pulling max-g forever. Rules: avoid `P_s < 0` in a neutral fight unless forced; prefer vertical fights when the opponent is slower/turn-limited; extend to regain speed when `E_s` deficit exceeds a threshold and no missile threat; use the corner-speed band for gun fights; never turn below corner speed when an escape (extend) is available. The strength of this behaviour is a skill parameter (`energy_awareness` ∈ [0, 1]).

## 19.6 Skill levels (outline §32)

Skill is a vector of behavioural parameters. Values are design targets; every field is data.

| Parameter | Recruit | Regular | Veteran | Elite | Ace |
|---|---|---|---|---|---|
| Perception delay (s) | 1.5 | 1.0 | 0.6 | 0.35 | 0.2 |
| Attention (targets monitored) | 3 | 5 | 8 | 12 | 16 |
| Probability to notice a new threat per update | 0.5 | 0.7 | 0.85 | 0.95 | 1.0 |
| Max sustained G as fraction of limit | 0.65 | 0.80 | 0.90 | 0.97 | 1.00 |
| Energy awareness | 0.1 | 0.4 | 0.7 | 0.9 | 1.0 |
| Gun aim error σ (mrad) | 12 | 8 | 5 | 3 | 1.5 |
| Missile fire fraction `f_fire` of R_max | 0.95 | 0.85 | 0.75 | 0.65 | 0.60 |
| Defensive reaction delay (s) | 3.0 | 2.0 | 1.2 | 0.7 | 0.4 |
| Notch aspect error σ (°) | 25 | 15 | 8 | 4 | 2 |
| CM timing error (s) | 1.5 | 1.0 | 0.6 | 0.3 | 0.15 |
| Terrain use (min AGL for masking, m) | 300 | 250 | 150 | 100 | 80 |
| Teamwork (mutual support probability) | 0.3 | 0.55 | 0.75 | 0.9 | 0.97 |
| Tactic repertoire | pure pursuit, basic break, straight BVR | + lead pursuit, yo-yos, notch, jink | + energy tactics, crank, pump, terrain mask | + coordinated attacks, drag, sandwich | full, including baiting, feints, out-of-plane defence |
| Decision commitment min (s) | 3 | 2.5 | 2 | 1.5 | 1.2 |

**What skill does not change:** aircraft performance, sensor range, weapon performance, health. The player may choose AI skill per side or "mixed" (a distribution), and the campaign assigns skill by unit identity (§29, §37).

A separate **personality** vector (aggression, caution, discipline) adds variety independent of skill and is set by squadron doctrine.

## 19.7 Fuel, damage, morale, and retreat

### 19.7.1 Fuel

Per aircraft: `bingo` fuel = fuel required to reach the nearest suitable friendly base plus reserve (reserve = max(10 % internal, 15 min at best-endurance flow)) using Tier-2 range planning; `joker` = bingo + fuel for current mission task. At `joker` the AI reports and starts disengaging if its task allows; at `bingo` it executes `RTB`/`Refuel` (tanker if available) regardless of engagement unless doctrine forbids.

### 19.7.2 Damage

State from the damage model (§18): the AI reasons on a *capability score* (thrust, control, sensors, weapons). Capability < 0.5 → seek disengage and RTB; critical fire or loss of control → eject (probability by skill). Damage knowledge is local (the pilot's cockpit warnings), not omniscient.

### 19.7.3 Morale and retreat

```text
morale m ∈ [0,1]  baseline m₀ = base(doctrine, skill)
Δm events:  friendly loss nearby (within 20 km, last 60 s) −0.06 each; leader killed −0.20; missile launched at me −0.04;
            under missile attack (active) −0.08; own kill +0.06; outnumbered (ratio > 2:1) −0.02/s;
            damaged (capability loss) −0.3·(1 − capability); fuel < joker −0.05; objective achieved +0.15; ally reinforcements arrive +0.1
recovery: m → m₀ at +0.01/s when not under threat
retreat: if m < θ_ret (0.25–0.40 by doctrine): switch to Retreat/Disengage; wingmen follow leader; resume if m > θ_ret + 0.15
```

"Fanatical" doctrines (boss squadrons, certain factions) ignore retreat. Morale gives the sense of a living opponent: a flight breaks off after losing its leader, a battered flight limps home, a squadron that wins aggressively presses.

## 19.8 Weapon and target preferences

Target value `TV` (from content threat values) and threat `Θ` (to self) combine into priority: `prio = α·TV + β·Θ + γ·opportunity(R, geometry) − δ·risk`. Preferences per doctrine: *kill bombers first* (interceptor squadron), *defend the AWACS*, *hunt the player* (Elite/Ace boss), *protect the escort target*. Squadron AI deconflicts (§20.5.3).

## 19.9 Perception

AI uses **tracks**, not entities:

- Each pilot owns a `TrackFile` fed by its sensors (§16) and by datalink/radio (§16.5.3).
- Per decision tick only up to `attention` tracks are processed (highest priority by threat/novelty); the rest keep last-known data.
- Position noise follows track covariance; unknown classification means "bogey" behaviour (cautious, avoids committing).
- Memory: lost tracks coast for 20 s (kinematic extrapolation) with growing uncertainty; AI may search that area ("search pattern").
- Visual spotting uses the same R_vis model (§16.6.2) and LOS; sun/cloud/terrain obscuration apply to AI equally.
- Hearing/radio: squadron calls (e.g. "Bandits 12 o'clock") reduce the perception delay for those tracks by half, with probability set by teamwork skill.

Validation: the `PerceptionAudit` (§19.17) enforces that decision code reads entities only through `TrackView`.

## 19.10 Navigation and avoidance

### 19.10.1 Route planning

Mission and squadron layers provide waypoints; a **route planner** refines them:

1. Build a coarse 2-D cost map over the battlefield at 1–2 km cells (≤ 512×512): `cost = 1 + λ_threat · threat(cell) + λ_terrain · roughness(cell) + λ_boundary · boundary_penalty`.
2. Threat map from known SAM sites/ships and horizon profiles (§16.9.2): exposure = coverage fraction at planned altitude; updated when sites are destroyed or discovered.
3. Any-angle path planning (Theta*) with smoothing; altitude profile by terrain-following with minimum AGL per mission type (high cruise vs low penetration).
4. Replan on new threats (event-driven, ≤ 2 Hz globally).

### 19.10.2 Local avoidance

| Hazard | Method |
|---|---|
| Terrain | Look-ahead 20 s along the velocity vector, using the base DEM; compute required pull-up g to clear by `margin(skill)`; command override if required g > 70 % of the available |
| Boundary | Repulsive steering when SDF > −W (inside the warning band) and a hard turn-back at Stage 2; pursuers break off at Stage 2 (§8.5) |
| Mid-air collision | Predict closest point of approach; if CPA < 150 m within 6 s, the lower-priority aircraft (by ID) deconflicts vertically/laterally |
| Friendly fire | Check line-of-fire clearance before gun/missile launch |
| Water | Maintain minimum altitude over water (≥ 50 m AGL) unless attacking |

## 19.11 Ground and special-mission behaviours

- **Strike:** ingress along low-threat route, pop-up or dive release profile by weapon; abort conditions (SAM launch, fuel, damage); egress with a defensive turn.
- **SEAD:** stand-off ARM shots from loft; "shoot and scoot".
- **Escort:** stay within `escort_radius` (e.g. 5 km) of the escortee; weave; break to engage threats inside `engage_radius`; return after threat killed.
- **CAP:** racetrack with a defined leg length; vector to threats on GCI/AWACS call.
- **Tanker join:** approach, formation, contact sequencing (AAR) with the tanker AI (§49.5).
- **Carrier ops:** pattern, approach, trap (§22.6).

## 19.12 Flight-control interface (virtual stick)

AI never touches the flight model directly. The **Maneuver Controller** turns *goals* (desired flight-path vector, load factor, bank angle, speed or throttle, heading/altitude/point) into the same inputs as a player:

```text
pitch stick  = clamp((n_cmd − n_trim_scale)/(n_cap − 1), −1, 1)  mapped through the inverse of the FCS command mapping (§11.11.2)
roll stick   = clamp(K_φ(φ_cmd − φ)/p_max_norm, −1, 1) with rate damping
yaw          = 0 (auto-coordination)
throttle     = PI on speed, with AB logic (AB only if tactic requests and fuel OK)
```

Skill sets `n_cap = skill.G_frac · n_max`. The AI applies the *same* assist (Standard) as a Standard-assist player, so it inherits limiter behaviour (no over-g). Tier-2 AI emits equivalent `(n, μ, throttle)` commands to the reduced model.

## 19.13 Radio communications (outline §36)

AI communicates through **procedurally generated, contextual radio messages**. The AI layer emits **semantic events**; the radio system (§56.6) maps them to phrases:

| Event | Example phrase |
|---|---|
| Contact | "Bandit group, bullseye zero-nine-zero, forty, angels two-five, two contacts." |
| Commit | "Viper one-one, committing." |
| Missile launch | "Fox three." / "Fox two." |
| Defensive | "Viper one-two, defensive, missile in the air!" |
| Kill | "Splash one." |
| Damage | "Taking fire!" / "Hydraulics failing." |
| Fuel | "Joker." / "Bingo." |
| Reinforcements | "Enemy reinforcements entering the battlespace." |
| Objective | "Radar site destroyed." |

Rules: message priority queue per channel; preemption by higher priority; cooldown per type (≥ 8 s for repeated contact calls); variation by weighted template selection with a recency penalty so the same phrasing does not repeat within 10 messages; distance- and jamming-based degradation (static); voice persona per callsign. Details of the grammar and brevity code are in §56.6 and Appendix G.

## 19.14 Debug visualisation (outline §119)

For a selected AI aircraft the debug overlay shows: current mission, current target, threat level, fuel, damage, weapons, formation slot, desired heading/altitude/speed, current tactic and its utility scores, tracks and perception state, predicted trajectories from the maneuver planner (§63.5).

## 19.15 Performance

| Tier | AI cost per aircraft per tick (Recommended) |
|---|---|
| T0/T1 | ≤ 20 µs average (decision amortised) + FDM cost |
| T2 | ≤ 8 µs |
| T3 | ≤ 2 µs |
| Squadron/mission layers | ≤ 0.2 ms per tick total |

Total AI budget ≤ 1.5 ms per tick for 75 aircraft. Staggering: each pilot decision at tick `(id mod 12)` of the 5 Hz window; the maneuver controller at `(id mod 2)` of the 30 Hz window. Predictive maneuver selection is skipped for T2+ (rule-based).

## 19.16 Edge cases

| Case | Handling |
|---|---|
| Target becomes invalid (destroyed, despawned) | Task is re-evaluated at the next tick; no dangling references (entity ids carry generation counters) |
| Two AIs stuck in a scissors | Break symmetry via commitment timers and an energy-based tiebreak |
| Low fuel at start (loadout) | Honour bingo; AI takes `RTB` immediately |
| Network jitter | AI runs authority-side only; clients only see results |
| All friendly airports lost | RTB → nearest viable airfield or improvised airstrip or ditch/eject |
| Time acceleration | AI decision rates scale with ticks; reaction delays remain in sim seconds |

## 19.17 Acceptance criteria

- `AC-19.1` **Perception audit:** a build-time and runtime audit finds zero AI code paths reading `Aircraft` true state of other-side entities except via `TrackView`.
- `AC-19.2` **Physical parity:** across a 1 000-aircraft-hour AI telemetry sample, no AI exceeds speed, g, roll-rate, or turn-rate limits by > 1 %.
- `AC-19.3` **Skill ordering:** in 1 000 AI-vs-AI duels per pair (identical aircraft, random geography), win rates are strictly ordered Recruit < Regular < Veteran < Elite < Ace with pairwise adjacent-skill win rate between 55 % and 80 %.
- `AC-19.4` **Behavioural coverage:** every tactic in §19.4.2 is exercised in a 1 000-scenario random run and each exits to a valid state ≥ 99 % of the time.
- `AC-19.5` **Missile defence:** Veteran AI survives ≥ 50 % of Pk-modelled MRM shots at 0.6 R_max while notching; Recruit ≤ 25 %.
- `AC-19.6` **Terrain safety:** AI controlled flight into terrain < 0.5 % of aircraft-hours on a mountainous test set (non-damaged aircraft).
- `AC-19.7` **Performance:** budget in §19.15 met for 75 AI on the Recommended profile.
- `AC-19.8` **Human assessment:** ≥ 70 % of playtesters say that AI "fly like pilots rather than homing missiles" in a blind test.

---

# 20. SQUADRON SYSTEM

`P4 · M4` — Aircraft operate as squadrons rather than isolated entities (outline §33–35). A squadron is the unit of tactics, doctrine, identity, and (in the campaign) persistence.

## 20.1 Intent

Squadrons make the sky feel organised: flights with callsigns, leaders and wingmen, formations that break in combat and re-form afterwards, shared targeting, coordinated attacks, and consistent personalities.

## 20.2 Squadron definition (outline §33)

```json
{
  "schema": "squadron/1", "id": "sqn_vulture_3", "callsign": "VULTURE", "faction": "meridian_pact",
  "aircraft": "swift", "count": 8, "flights": 2,
  "pilot_skill": { "dist": [["veteran", 0.5], ["elite", 0.4], ["ace", 0.1]] },
  "formation": "combat_spread",
  "tactics": { "preferred": ["intercept", "commit", "crank", "pump"], "forbidden": ["kamikaze"] },
  "aggression": 0.7, "preferred_engagement_range_km": 70,
  "missile_doctrine": { "style": "shoot_look_shoot", "salvo_on_high_value": true, "f_fire_bias": 0.0, "hoard_lrm": true },
  "radar_behavior": { "emcon": "intermittent", "tws": true, "datalink": true },
  "retreat": { "morale_threshold": 0.30, "casualty_threshold": 0.5, "fanatical": false },
  "morale": 0.8,
  "objective": null, "command_aircraft": "VULTURE-1-1",
  "identity": { "livery": "dark_wedge", "voice": "voice_b", "known_by_player": false }
}
```

| Field | Notes |
|---|---|
| `callsign` | Word from the callsign table; flights numbered `VULTURE 1`, `VULTURE 2`; members `1-1` (lead), `1-2`, `1-3`, `1-4` |
| `aircraft` | Aircraft id(s); mixed squadrons allowed (list with proportions) |
| `pilot_skill` | Distribution over §19.6 levels, sampled deterministically per pilot |
| `formation` | One of the formations in §20.4 |
| `tactics` | Allowed/preferred tactics (affects utility weights) |
| `aggression` | 0–1; scales commit thresholds and tolerance for risk |
| `preferred_engagement_range_km` | Preferred first-shot range; BVR squadrons stand off, merge-fighters close |
| `missile_doctrine` | Salvo size, shoot-look-shoot vs ripple, hoarding, firing-fraction bias |
| `radar_behavior` | Emission policy (continuous/intermittent/silent), TWS/STT preference, datalink usage |
| `retreat` | Morale and casualty triggers (§19.7.3), fanatical flag |
| `morale` | Squadron-level aggregate (mean of members, plus leadership modifier) |
| `objective` | Current mission-level assignment (set by Mission AI) |
| `command_aircraft` | Leader (callsign id); succession order if killed |

## 20.3 Callsigns

A data-driven table of ~200 callsign words (animals, weather, colours, mythic names, neutral nouns) excluding real unit callsigns and offensive terms. Callsign assignment: unique per side per mission, deterministic from the seed; AWACS/GCI and tankers use role-specific sets (e.g. `SENTRY`, `HORIZON`, `SHELL`). Player callsign is chosen in customization (§14.7); duplicates resolved by a numeric suffix.

## 20.4 Formations (outline §34)

Formations are slot offsets in the **leader frame** (x forward, y right, z down) with distance parameters per spacing class.

| Formation | Slots (wingmen relative to lead) | Typical use |
|---|---|---|
| **Line (line abreast)** | `(0, ±s, 0)` | Sweep, search |
| **Wedge (V)** | `(−s, ±s, 0)` for 2 wings; `(−2s, ±2s, 0)` next | Transit, show |
| **Echelon L/R** | `(−s·k, ±s·k, 0)` stacked to one side | Attack setup |
| **Spread (combat spread)** | Wing at `(−0.3d, ±d, +0.1d)`, element 2 at `(−d, ±2d, 0)` | Combat patrol, mutual support |
| **Defensive** | Trail / box with overlapping fields of fire: `(−d, 0, ±0.1d)` | Threat-heavy ingress |
| **Attack** | Staggered, offset 20–30° from the lead's flight path (sandwich ready) | Pre-merge |

Spacing classes (distance `s` or `d`): **Tight** 40 m (parade/show), **Route** 150 m (cruise), **Combat spread** 1 850–2 800 m (1.0–1.5 nmi), **Wide** 5 000 m (area search). Spacing and height offsets come from the squadron doctrine.

**Station keeping.** Each wingman has a target point `p_slot = p_lead + R_lead · offset` and velocity `v_lead`. A PD controller produces desired velocity `v_des = v_lead + K_p (p_slot − p) + K_d (v_lead − v)`; speed capped at 1.15 V_lead (catch-up bonus) and limited by AB logic; the maneuver controller (§19.12) executes. Collision-safety overrides station keeping inside 30 m closure.

**Formation breaking and re-forming.** Station-keeping weights go to zero when a pilot's tactic exceeds a priority (any engagement tactic, missile defence, damage RTB). When a flight disengages, `Regroup` orders issue a rendezvous (lead flies a reduced-speed orbit; wingmen use a join-up schedule: closing speed `v_c = clamp(0.1·range, 5, 50) m/s` until within slot capture radius 100 m). Combat naturally breaks formations, which is intended (outline §34).

## 20.5 Coordination

### 20.5.1 Elements and roles

A flight has 2 elements (lead + wingman each); roles: *Flight lead*, *Element lead*, *Wingman*. Engaged roles: **Engaged**, **Supporting** (covers, looks for third parties), **Free**. Wingmen cover the lead's tail with probability `teamwork` per check (§19.6); "Lead, break!" calls precede dangerous situations.

### 20.5.2 Coordinated tactics

Supported two- and four-ship tactics: *Sandwich* (one aircraft engages head-on while the other flanks), *Loose deuce* (mutual support at spread distance), *Bracket* (opposite flanks), *Trail* (follow-on attack), *Pincer*, *Defensive split* (hard turn away in opposite directions), *Bait and switch* (Elite/Ace).

### 20.5.3 Target assignment

Every 1 Hz the squadron AI builds a cost matrix between friendly elements and enemy groups (a *group* is contacts within 3 km of each other):

```text
C_{ij} = a·dist_ij + b·time_to_intercept_ij − c·TV_j − d·weapon_suitability_ij + e·fuel_penalty_i + f·already_assigned_j
```

Assignment is solved with a greedy minimum-cost matching refined by 2-opt swaps (≤ 12×12; optimal Hungarian is overkill at this size but allowed). Over-assignment is penalised unless the target is high priority. Calls: "Lead has north group, two has south group." Reassignment occurs when assignments become invalid (kills, new threats) with a 5 s minimum hold.

### 20.5.4 Communication range and jamming

Voice/datalink coordination requires link quality (§17.4) and range (≤ 200 km LOS, standard). Without a link, members use *autonomous tactics*: independent decisions with fixed doctrine, no shared targeting, reduced teamwork (−0.3). This makes comms jamming and separation consequential.

## 20.6 Friendly AI and player orders (outline §35)

### 20.6.1 What friendly AI does

Attack enemies; defend objectives; escort bombers; attack ground targets; intercept threats; request assistance; retreat when damaged. The player is not required to personally handle every threat.

### 20.6.2 Command interface

A radial **command wheel** (hold a key / button) and hotkeys:

| Command | Effect |
|---|---|
| Attack my target | Wingmen engage the player's locked target |
| Cover me / Engage defensively | Wingmen take supporting role |
| Break and attack | Free engagement of nearby threats |
| Form on me | Rejoin and adopt a formation (submenu: wedge, spread, line, echelon) |
| Defend base / Defend [location] | Assigns a CAP |
| Escort [unit] | Escort order |
| Weapons free / tight / hold | ROE for the flight |
| Return to base / Refuel | |
| Report status | AI reports fuel, ammo, damage |
| Hold position / Orbit here | |

AI acknowledges by radio ("Two, wilco"), may refuse when impossible ("Negative, bingo fuel"). Orders have priorities and expire (or persist until replaced). In co-op any player who is flight lead can command AI wingmen; the host sets the policy.

### 20.6.3 AI requests

AIs ask for help ("Lead, I'm defensive! Need help!"), report threats ("Bandits, nose, 30 miles"), and request permission when ROE restricts. Requests appear as HUD alerts and radio lines and can be accepted with the wheel.

## 20.7 Ground Control Intercept and AWACS AI (`ControllerAI`)

Controllers consume the shared picture (§16.5.3) and provide:

- **Picture calls** on request or when the situation changes: groups with bullseye bearing/range, altitude (angels), count, heading.
- **Vectors:** "Viper, vector 270, forty, angels twenty" for intercept geometry.
- **Warnings:** "Bandits launching," "Threat your six."
- **Declarations:** "Declare" requests answered with hostile/friendly/neutral per IFF/datalink.
- **Tanker/airbase info:** vectors to the tanker, field status.

Update policy: high-priority groups every 10 s, others every 30 s; controllers are subject to the same sensor limits and jamming (§17.9). A human can occupy the Air Battle Manager seat on the Argus (§32.4).

## 20.8 Packages (strike packages)

A **package** bundles squadrons with roles: *Strike* (bombers/strike), *Escort*, *SEAD*, *EW*, *Tanker*, *CAP*. Package planning computes **time on target (TOT)** and per-element takeoff time:

```text
t_takeoff,i = TOT − (distance_i / V_cruise,i) − t_climb,i − t_form,i − t_buffer
```

with rendezvous points (join-up) before the ingress. Escorts fly alongside at specified offsets; SEAD arrives 1–3 minutes before TOT; EW flies the stand-off racetrack.

## 20.9 Lifecycle, identity, and persistence

- **Spawn/despawn** by the Mission AI (§25, §28) with spawn effects (air start in formation or ground start at airbase).
- **Reinforcement queues:** a squadron may be a pool of aircraft with a reinforcement rate (§28.5).
- **Identity:** named squadrons (callsign, livery, voice, personality) survive across missions in the campaign; the player's knowledge of them evolves (§42.3) and aces get dossiers (§29.5).
- **Replacement pilots:** when a pilot dies, the next in the roster takes the slot (campaign: pilot persistence, §41).

## 20.10 Networking and performance

Squadron AI runs only on the authority. Clients never simulate it. Orders by human players travel as reliable commands (`command_id`, params) and are validated by the server. Squadron layer budget ≤ 0.2 ms/tick (§19.15).

## 20.11 Edge cases

| Case | Handling |
|---|---|
| Leader killed | Succession by roster order; 3 s "lead change" radio; morale penalty |
| Wingman out of range | AI rejoin; if > 100 km, becomes autonomous |
| Mixed-speed squadrons | Formation speed = min(cruise) of members; afterburner catch-up for slower types prohibited |
| Player leaves the flight | AI reassigns lead |
| Zero targets assigned | `Free` role; default to CAP/escort per objective |
| Order conflict | Latest player order wins; AI says "Say again" for contradictory orders |

## 20.12 Acceptance criteria

- `AC-20.1` Formation test: a 4-ship flight in each formation stays within 15 % of spacing over a 10-minute wedge-to-spread-to-line transit with bank turns up to 45°.
- `AC-20.2` Combat break/re-form: after a scripted engagement, ≥ 90 % of surviving aircraft rejoin their formation within 4 minutes.
- `AC-20.3` Target assignment: for 8 elements vs 8 groups, the assignment is within 5 % of the optimal total cost in ≥ 99 % of random instances.
- `AC-20.4` Command wheel: all commands deliver an AI acknowledgement within 2 s and a visible behavioural change within 5 s in a scripted test.
- `AC-20.5` Controller AI: picture calls match the shared picture's ground truth within ±5° bearing / ±10 % range for at least 95 % of groups when sensors permit.

---

# 21. GROUND WARFARE

`P3/P5 · M3–M4` — Ground entities exist physically in the world (outline §38). They are targets, threats, objectives, and scenery that reacts. This chapter covers units, movement, air-defence networks, infrastructure, and the simulation tiers that keep them affordable.

## 21.1 Intent

- Ground forces give air power something to do and something to fear: convoys to hit, SAM networks to suppress, factories to bomb, bridges to cut.
- Every ground target is understandable from the air: visible silhouette, thermal signature, a clear role.
- Cost scales: hundreds of units cheaply, with detailed simulation only where it matters.

## 21.2 Unit classes

| Class | Examples (fictional designs) | Role |
|---|---|---|
| **Main battle tanks** | MBT-A (heavy), MBT-B (light) | Armoured targets; convoy escort; objective |
| **Armoured vehicles** | IFV, APC, scout car | Convoys, base security |
| **Artillery** | Self-propelled howitzer, multiple-rocket launcher (MRL) | Area fire on friendly positions (cosmetic + objective pressure) |
| **SAM systems** | Short-range (point) SAM, medium-range SAM, long-range SAM, MANPADS team | Air denial (§21.5) |
| **AAA** | Self-propelled gun (twin 30 mm), towed 57 mm, radar-directed battery | Low-altitude denial |
| **Radar stations** | Early-warning radar, GCI radar, 3-D surveillance, fire-control radar | Detection, cueing (§16, §21.5) |
| **Command centres** | Sector command bunker, battery command post | Network node (§21.5) |
| **Logistics** | Fuel truck, ammo truck, supply column, rail transport | Convoy targets; campaign supply (§39) |
| **Industrial** | Factory, refinery, depot | Strategic targets |
| **Infrastructure** | Bridge, dam, rail yard, power plant, substation, pylon line, comm tower | Interdiction targets |
| **Military installations** | Bunkers, hardened shelters, hangars, barracks, fuel farm, ammunition depot | Airbase and base objectives |

### Unit definition schema

```json
{
  "schema": "ground_unit/1", "id": "mbt_a", "class": "armor", "faction_tags": ["common"],
  "mobility": { "type": "tracked", "v_max_road_mps": 18, "v_max_offroad_mps": 11, "turn_rate_dps": 40, "slope_max_deg": 28,
                "terrain_speed_factor": { "grass": 1.0, "forest": 0.45, "sand": 0.7, "snow": 0.6, "mud": 0.4, "urban": 0.8, "water": 0.0 } },
  "hp": 220, "armor_class": "heavy", "crew": 4,
  "signatures": { "rcs_m2": 6.0, "ir_intensity": 0.6, "visual_size_m": 7.5 },
  "sensors": { "visual_m": 3500, "radar": null },
  "weapons": [ { "id": "gun_120", "range_m": 3500, "role": "ground" } ],
  "threat_value": 4.0, "target_priority": 0.6,
  "model": { "lod": ["mbt_a_lod0", "mbt_a_lod1", "mbt_a_imp"], "wreck": "mbt_a_wreck" }
}
```

## 21.3 Physical presence and simulation tiers

- Position on terrain: height from the base DEM (ADR-15); orientation aligned to the terrain normal with a simple suspension spring for visuals; ground units do not collide with each other (they use spacing rules) but are blocked by water/steep slopes via pathing.
- Collision with aircraft: ground units are hit volumes (boxes) usable for gun/bomb impact and aircraft–ground collisions (crash on a building/vehicle).

**Simulation tiers (ground):**

| Tier | Condition | Movement rate | Sensors/AI | Replication |
|---|---|---|---|---|
| **G0 Active** | Within 40 km of a human or engaged | 10 Hz | Full (detect, engage) | 5 Hz when within 30 km of a client |
| **G1 Passive** | 40–150 km | 1 Hz | Detection only for SAM/radars (event-driven) | 1 Hz |
| **G2 Dormant** | > 150 km | On-rails: position = function of path and time (no per-tick sim) | None | None; recreated on promotion |

Hard cap: active ground units per §0.6.2 (300 on Recommended). SAM sites and radars are always at least G1 so they can cue and engage when a target enters their range.

## 21.4 Movement and pathfinding

- **Road network** from OSM `highway=*`/`railway=*`: nodes at junctions, edges with length, class and speed (motorway 25 m/s … track 6 m/s), bridges as edges flagged `bridge:<id>`.
- **Off-road grid:** 100 m cell cost map from land cover and slope: `cost = 1 / terrain_speed_factor(landcover) · (1 + (slope/slope_max)²)`; impassable if slope > `slope_max` or water.
- **Planner:** A\* on the road graph for road-bound units (wheeled convoys), hierarchical A\* (roads + off-road grid) for tracked units; path smoothing; convoy spacing 30–50 m with speed matching; bridges/choke points are cut when destroyed (`bridge` destroyed removes the edge and triggers re-planning with a 5–10 s delay; stuck columns halt).
- **Behaviour modes:** *march*, *hold*, *defend* (stationary), *ambush* (hold fire until the target is inside a kill radius), *retreat* (on morale/damage).
- **Fuel/supply** are abstract in mission mode; campaign supply is a node-level resource (§39).

## 21.5 Air-defence networks (outline §39)

SAM systems operate as **networks**, not isolated turrets. This creates meaningful SEAD missions.

### 21.5.1 Network model

```text
EARLY-WARNING RADAR (EWR) ──┐                            ┌── LONG-RANGE SAM battery(s)
GCI / 3-D SEARCH RADAR ─────┼──►  SECTOR COMMAND (C2) ──►├── MEDIUM-RANGE SAM battery(s)
(OPTICAL / PASSIVE SENSORS) ┘     (track fusion, tasking) ├── SHORT-RANGE SAM / AAA
                                                          └── FIRE-CONTROL radars (per battery)
```

Nodes are units; links are logical (datalink/wire) with LOS or "hardened" flags. Network state is computed by a `NetworkGraph` evaluated at 1 Hz:

| Condition | Effect |
|---|---|
| EWR destroyed/jammed | Batteries lose early warning; they rely on their own search radars (shorter range, later detection), `reaction_time += 8–15 s` |
| Sector C2 destroyed | Track sharing lost; batteries become autonomous (no cueing, narrower coverage); `engage_efficiency × 0.6` |
| Fire-control radar destroyed | Launchers cannot guide (SARH/command guidance); battery is combat-ineffective until it relocates a spare radar (5–10 min) |
| Launcher destroyed | Reduced missile count/salvo capability |
| Link jammed | Link quality degrade (§17.4) → delays and duplicate engagements |
| Decoy emitters | Waste fire / trigger emission (SEAD baiting) |

### 21.5.2 Battery state machine

```text
IDLE (radar off/low-power search) → SEARCH (emitting) → ACQUIRE (target in envelope) → TRACK (fire-control lock)
   → ENGAGE (launch 1–2 missiles) → ASSESS (kill? re-engage?) → RELOAD → { SEARCH | RELOCATE (shoot-and-scoot) | EMCON (on ARM warning) }
```

Parameters (design targets): range classes — short 8 km (altitude 0–3.5 km), medium 30 km (0.05–18 km), long 120 km (0.1–27 km); minimum ranges; reaction time (3–12 s by class and ECCM); salvo size 2; reload 60–180 s; channels (simultaneous engagements): short 1, medium 2, long 4; Pk model uses the same missile entity flight model (§15.5) flagged `surface_launched`; radar detection uses §16.3 with the site's `R_ref` and the **precomputed horizon profile** (§16.9.2).

### 21.5.3 Doctrine and counter-play

| Doctrine | Behaviour |
|---|---|
| **Persistent area denial** | Radar always emitting; strong deterrent, vulnerable to ARM |
| **Ambush** | Radar silent until target enters the kill zone (relies on passive/optical cueing or EWR) |
| **Shoot-and-scoot** | Relocates 1–5 km after each launch; 4–6 min displace cycle; mobile launchers |
| **Integrated defence** | Network shares tracks; layered coverage; coordinated ARM response (one radar emits while another is quiet) |

Operators react to ARM launches by shutting radars down for `t_blank = 30–60 s` (§17.10), hiding in the shadow, and re-emitting when the threat passes — a visible, learnable cat-and-mouse.

### 21.5.4 AAA and MANPADS

- **AAA:** direction by radar or optical director. Fire control computes a lead point; the site fires *bursts* rather than individual rounds: each burst is a proximity-fuzed airburst at the predicted point with lethal radius `R_l` (8–15 m by calibre); burst rate 4–10 per second per gun; effective altitude ≤ 3–4 km by calibre; accuracy degrades with target speed and crossing angle; flak visual puffs are cosmetic and driven by the burst events. A battery of 8 guns can produce a dangerous barrage at 500 m AGL, ineffective at 6 km.
- **MANPADS:** infantry IR missiles, 4 km range, 3.5 km altitude, hidden (visual size 1.5 m; detected only by visual/IR sensors); defeated by flares (high effect) and by high speed/altitude.

## 21.6 Ground combat among AI

AI ground forces fight each other to give missions a living front and objectives (convoys advancing, bases under attack). To keep cost low the exchange is **abstracted**: engaged clusters resolve at 1 Hz with a pairwise attrition model (attacker class vs defender class kill-rate matrix with range factor), visualised by tracers, muzzle flashes and impacts generated from the abstract hits. Air support modifies the model (a destroyed vehicle removed immediately; suppression bonus for CAS). Outcomes feed mission objectives (e.g. "defend the bridge until relief").

## 21.7 Damage, destruction and infrastructure effects

- Units have HP and an armour class (§15.9.6, §18.3 tables); small arms/gun damage uses the gun-hit formula of §18.6; blast uses `R0 = k·W^{1/3}`.
- **Secondary effects:** fuel/ammo vehicles explode (blast radius 15–40 m), fire burns 60–180 s, wrecks block roads.
- **Infrastructure functions (game effects):**

| Target | Effect when destroyed |
|---|---|
| Bridge | Edge removed from road graph; convoys halt or reroute; repair after T_repair (campaign 20–60 min; mission mode permanent) |
| Power plant / substation | Dependent area loses power: city lights off, radar sites on that grid fall back to generators (−30 % range), airfield lighting off |
| Dam | Downstream flooding visual (optional), power loss |
| Factory / refinery | Reduces enemy production/supply in campaign (§39); mission objective |
| Rail yard | Rail logistics halted |
| Fuel depot | Airbase fuel stock −X; large fire |
| Ammunition depot | Chain explosion (cook-off sequence) |
| Command bunker | C2 node loss (§21.5) |
| Hardened shelter | Needs penetrator bombs to damage contents (armour class `hardened`) |

## 21.8 Placement (procedural generation hooks)

The mission generator (§26.7) uses these constraints:

| Unit | Constraints |
|---|---|
| Tanks/armour | Slope ≤ 15°, non-water, near roads or open land; column spacing 30–50 m |
| SAM sites | Slope ≤ 5°, flat pad ≥ 100×100 m, high **viewshed score** (maximise approach-sector coverage using horizon profiles), 3–8 km from the protected asset, avoiding ridge tops that expose the battery to ARM geometry unless doctrine says so |
| AAA | Within 2–4 km of protected objects, around perimeter and approach lanes |
| Radar stations | Elevated sites with wide LOS (prefer hilltops), within 15 km of command centre |
| Command centre | Hardened position within the defended area |
| Logistics | On the road graph |
| Industrial | Industrial land cover or near urban fringe |
| Decoys | Positions consistent with real sites to bait emitters |

## 21.9 Rendering and performance

- Instanced draws with 3 LODs per unit (≤ 3 000 / 800 / 200 triangles) and an impostor at > 6 km; dust/track trails as GPU particles; muzzle flashes pooled.
- Budget (Recommended): ≤ 0.6 ms sim for 300 active units; ≤ 1.0 ms render CPU for ground units + static installations.
- Static installations are baked into the tile **feature layer** (instanced), not entities, until interacted with (promoted to destructible on first damage or proximity < 5 km).

## 21.10 Edge cases

| Case | Handling |
|---|---|
| Unit stuck | Timeout 60 s → re-plan; second timeout → teleport to nearest valid cell (logged) |
| Missing roads | Fall back to off-road grid / procedural roads |
| Overlapping placements | Resolve with Poisson-disc minimum spacing |
| Bridge over river is the only path | Detour or halt; mission validation ensures objectives remain solvable (§26.9) |
| Unit placed in water by data error | Rejected by placement validator |

## 21.11 Acceptance criteria

- `AC-21.1` A destroyed fire-control radar disables its battery's guidance within one tick; a destroyed C2 reduces `engage_efficiency` by the specified factor at the next 1 Hz network update.
- `AC-21.2` Ambush doctrine SAM remains radar-silent until the target enters its kill zone in 95 % of 500 test runs.
- `AC-21.3` Convoy detour: destroying a bridge on a 3-bridge route causes a re-plan in ≤ 10 s and a stable new route.
- `AC-21.4` 300 active ground units simulate within the §21.9 budget.
- `AC-21.5` Horizon-profile SAM coverage matches an exact LOS ray march within 1 % of cells on 20 sample sites.

---

# 22. NAVAL WARFARE

`P10 · M4 (basic ships and carriers) → M6+ (submarines)` — Ships are moving objectives, threats and bases (outline §40–42).

## 22.1 Intent

Ships can move, detect aircraft, launch weapons, defend themselves, and serve as objectives. Aircraft carriers function as mobile airports (§22.6). Submarines appear later with stealth and anti-submarine gameplay (§22.7).

## 22.2 Ship classes

| Class | Length (m) | Max speed (kn / m/s) | Role |
|---|---|---|---|
| **Aircraft carrier (large, CATOBAR)** | 330 | 30 / 15.4 | Mobile air base; fleet core |
| **Aircraft carrier (light, ski-jump STOBAR)** | 280 | 28 / 14.4 | Smaller air group |
| **Cruiser** | 180 | 32 / 16.5 | Area air defence, long-range strike |
| **Destroyer** | 150 | 32 / 16.5 | Air/surface defence, escort |
| **Frigate** | 125 | 30 / 15.4 | Escort, ASW |
| **Patrol vessel** | 60–80 | 35 / 18.0 | Coastal defence, small arms |
| **Transport / cargo / tanker** | 120–220 | 16 / 8.2 | Convoys, logistics targets |
| **Submarine** | 90–110 | 25 submerged / 12.9 | Stealth attack (§22.7) |

(1 kn = 0.5144 m/s.) Data schema parallels the ground unit with additional fields: displacement, draft, turning circle (tactical diameter in lengths), acceleration, hull sections and compartments, sensor suite, weapons mounts with arcs, CIWS layout, damage-control rate.

## 22.3 Movement and navigation

### 22.3.1 Kinematics

Surface ships use a 3-DOF planar model on the sea surface (ellipsoid at h = 0 plus wave heave for visuals):

```text
V̇ = clamp((V_cmd − V) / τ_V, −a_max, +a_max)                   τ_V ≈ 20–60 s by displacement
ψ̇ = clamp(ψ̇_cmd, ±V / R_turn)                                 R_turn = k_T · L, k_T ≈ 2.5 (tactical diameter ≈ 5 L)
V_loss in turn = 1 − 0.25 (|ψ̇| / ψ̇_max)                        speed bleed
```

Heading is held by a PD autopilot; waypoint following uses an L1-style guidance law (lateral acceleration `a = 2V²/L₁ · sin η`).

### 22.3.2 Sea navigation grid

The ocean navigation grid (250 m cells, built at battlefield load from the land mask, coastline distance field, and bathymetry) provides: passability (depth ≥ `1.5 × draft + 5 m` for surface ships; submarines need depth ≥ operating depth + 20 m), shore buffers (≥ 1 km), and costs for shipping-lane preference. Fleet paths: A\* on the grid with turning-radius-aware smoothing; formation stations (§22.8) follow the flagship. Coastal operation and narrow straits are meaningful because land masks are real.

### 22.3.3 Sea state

Sea state is derived from wind (Beaufort scale, §24.4). Effects: speed reduction `V_max · (1 − 0.03·Sea_state_index)` for small ships; deck motion amplitude (carrier flight operations limits, §22.6.5); radar sea clutter (§16.3.4); wake visibility.

## 22.4 Ship sensors and defence

| Layer | Systems | Notes |
|---|---|---|
| **Detection** | 3-D air search radar (`R_ref` ~ 200 km class), surface search, fire-control radars, ESM, EO/IR, sonar | Radar horizon matters: low-flying aircraft can hide in the sea-skimming band |
| **Area air defence** | Long-range SAM (range 120 km class), medium (30 km) | Channel-limited, §22.4.2 |
| **Point defence** | Short-range SAM (8 km), CIWS (close-in gun, 2.5 km) | Last-ditch |
| **Soft kill** | Chaff rocket launchers, decoy flares, jammers | Defeat anti-ship missile seekers |
| **Offence** | Anti-ship missiles (80+ km), land attack/cruise (300 km), main gun (surface fire, cosmetic vs aircraft), torpedoes | Mission-relevant |

### 22.4.1 Detection of low flyers

Sea-skimming targets at 5–15 m are visible to a ship radar at radar-horizon range: `d ≈ 4.12 (√h_radar + √h_target)` km (h in metres, 4/3 Earth) — e.g. radar at 30 m and a missile at 10 m: 4.12 × (5.48 + 3.16) = 35.6 km. This sets the practical warning time for anti-ship missiles (≈ 100 s at 360 m/s).

### 22.4.2 Layered defence resolution

For each incoming missile, defence layers engage in range order if the ship has free fire-control **channels**:

| Layer | Envelope (range) | Channels | Pk per engagement (design) |
|---|---|---|---|
| Long-range SAM | 60 → 20 km | 4 | 0.55 |
| Medium-range SAM | 20 → 6 km | 4 | 0.50 |
| Short-range SAM | 8 → 2 km | 2 | 0.45 |
| CIWS | 2.5 → 0.5 km | 1 per mount | 0.40 per burst, up to 2 bursts |
| Soft kill (chaff/decoy) | 8 → 1 km | n/a | reduces seeker lock: `P_seduce` 0.15–0.35 by seeker ECCM |

The layer Pk applies to the actual missile entity via the generic engagement (a "kill" removes it; a miss lets it continue). **Saturation:** simultaneous incoming missiles beyond channel capacity leak through, so salvo size, timing, multi-axis approach and decoys decide outcomes. Sea-skimming reduces the SAM Pk by a factor (0.6) beyond 10 km because of low-altitude clutter and masking.

## 22.5 Ship damage

Compartmentalised: **hull sections** (bow, forward, midships, aft, stern), **superstructure**, **engine room**, **magazine**, **flight deck/hangar** (carriers), **radar mast/electronics**, **weapon mounts**. Each section has HP and armour class; states Intact / Damaged / Critical / Destroyed with effects:

| Section | Effects |
|---|---|
| Engine room | Speed limit ×(0.2 + 0.8·hp); dead at 0 |
| Magazine | Cook-off chain at ≤ 25 %: explosion destroying adjacent sections |
| Radar/electronics | Sensors degrade |
| Weapon mounts | Offline at 0 |
| Hull | Flooding rate `f ∝ (1 − hp)`; list angle increases; sinks when buoyancy < 0 |
| Flight deck | Launch/recovery disabled when hp < 40 % (catapults/wires per element) |
| Fire | Spreads between adjacent compartments at 0.05/s; damage control reduces rate by crew factor |

Sinking: scaled for gameplay to resolve within 90–240 s after loss of buoyancy (large ships longer); ships can sink bow/stern first with visual list and secondary explosions. Aircraft on deck are destroyed/lost with the carrier.

## 22.6 Aircraft carriers (outline §41)

Carriers are mobile airports: they can launch aircraft, recover aircraft, refuel, rearm, move, defend themselves, and be attacked.

### 22.6.1 Flight deck and equipment

| Element | Large CATOBAR | Light STOBAR |
|---|---|---|
| Flight deck length | 330 m (angled deck 9°) | 280 m (ski-jump 12°) |
| Catapults | 3–4 (stroke 94 m) | none |
| Arresting wires | 4 (spacing ≈ 14 m) | 3 |
| Elevators | 3–4 | 2 |
| Air group (design) | 60–70 | 24–32 |
| Hangar capacity | 50 % of air group | 60 % |
| Fuel / ordnance stores | Large (campaign tracked) | Medium |

### 22.6.2 Launch operations

- **Wind over deck (WOD)** target 15–20 m/s: the carrier AI turns into the wind and sets speed to meet WOD (`V_ship = max(0, WOD_target − V_wind_component)`), limited by max speed and the fleet's wishes. Example: wind 5 m/s on the bow plus ship 15.4 m/s = 20.4 m/s.
- **Catapult:** aircraft taxi to the catapult (auto-taxi aid), hook up, launch (§11.13.3): end speed `V_end ≥ 1.2 V_stall + margin`; for ~85 m/s over 94 m acceleration is 38.4 m/s² (3.9 g) for 2.2 s. Cycle time between launches per catapult 30–45 s. "Cold shot" failure if the aircraft exceeds the catapult's weight-setting (sandbox warning; failure → ditch).
- **STOBAR:** ski-jump, arrested recovery; aircraft need `stobar` flag; take-off roll 150–200 m.

### 22.6.3 Recovery

Simplified Case I (VFR) and Case III (IFR) patterns: the AI/pilot flies a left-hand pattern to the angled-deck approach; **glideslope 3.5°** to the deck, with an optical landing cue (meatball) and radio calls from the landing signal officer (LSO). Touchdown at the target wire (wire 3): arresting hook engages when the hook point crosses a wire; arrestor force-runout curve brings the aircraft to rest in ≈ 100 m (≈ 2.5 g from 70 m/s). **Trap** probability depends on the hook-to-wire alignment and sink rate; a **bolter** (missed wires) requires full power go-around (the aircraft takes off from the angled deck's end). Landing inspections: sink rate ≤ 5 m/s at the ramp (design), lateral alignment ±3 m.

### 22.6.4 Hangar, rearm, refuel, repair

Servicing rates (design targets): refuel 30 kg/s; rearm one weapon per 25 s per crew slot (max 4 simultaneous); repair damaged components per class (§18) at campaign-defined rates; turnaround time ≈ 8–15 min depending on workload. In sandbox, a "quick turnaround" option shortens it.

### 22.6.5 Deck motion and sea state limits

Deck motion is a deterministic sum of sinusoids: `pitch = A_p sin(2π f_p t + φ_p)`, `roll = A_r sin(2π f_r t + φ_r)`, `heave = A_h sin(…)`, with `A_p, A_r` (degrees) = `clamp(0.35·Hs, 0, 4)` and periods 8–14 s; `Hs` from the Beaufort table (§24.4). Carrier ops are restricted when `A_p > 2.5°` or deck wind outside limits (warning; may not be allowed at Hard).

### 22.6.6 Carriers as targets and AI

Large radar cross-section, thick defences (layered), damage per §22.5. The **carrier AI** schedules launch/recovery cycles (alternating 45-min deck cycles: launch, CAP, recovery), steers into wind, and reacts to threats by requesting CAP and increasing readiness. Carrier defence missions (§27.7) revolve around this loop.

## 22.7 Submarines (staged, `P10`)

- **Stealth model:** acoustic signature `S(V, depth, noise_class)` rising sharply above quiet speed (≈ 5 kn) and falling with depth; thermocline layers may shield from sonar above/below (an environment parameter).
- **States:** *Submerged* (invisible except to sonar/MAD), *Periscope depth* (mast RCS 0.1–1 m²; visual/radar detectable in good conditions), *Surfaced* (full visual/radar signature).
- **Sensors:** passive sonar (detection range vs. surface ship wake noise), ESM mast, periscope.
- **Weapons:** torpedoes (range 8–15 km, speed 25 m/s), anti-ship/cruise missiles (VLS surfacing launch), mines (optional).
- **Anti-submarine aircraft gameplay:** sonobuoy patterns and magnetic anomaly detection by ASW aircraft: each sonobuoy (passive) has detection radius `R = R₀ · exp(−0.04·(V_kn − 5))` clamped (R₀ = 3 km at quiet speeds; faster means louder and easier); detection probability per 10 s inside radius 0.5–0.9 depending on the thermocline; localisation via 2+ buoys; the sub reacts (dive deeper, evade, counterfire). Lightweight torpedoes drop on a localised contact. The ASW aircraft ("Marlin" class) is a post-launch content addition (Appendix D).

## 22.8 Formations and task groups

Standard task group: flagship (carrier) at centre; screen ring stations at 5–15 km (destroyers, cruiser) arranged by threat axis; picket ships forward; oiler/transport inside. Formation keeping by station-keeping controller (like §20.4) with slow dynamics. AI handles zig-zagging (optional vs submarine threat) and emergency turns (anti-torpedo). Fleet sensors share a naval datalink (shared picture like §16.5.3).

## 22.9 Rendering and performance

Ocean: FFT-based displacement (Tessendorf) on a 256² cascade ×3 with foam, in WebGPU compute (cost ≈ 1.2 ms at High); ships: instanced LODs, wakes (trail strip), bow spray; submarine periscope wake. Budget: ≤ 0.3 ms sim for 50 ships (movement 10 Hz, sensors 2 Hz, defence resolution 10 Hz when threatened); render ≤ 1 ms for ships.

## 22.10 Edge cases

| Case | Handling |
|---|---|
| Ship aground (data error) | Snap to nearest water cell; flagged in logs |
| Fleet path blocked | Re-plan at 5-s intervals up to 3 times; else holds position |
| Carrier sunk with aircraft airborne | Divert to alternative base or ditching; "no deck" warning |
| Rough sea | Carrier launches suspended above limits (Hard realism) |
| Time acceleration | Naval movement scales with ticks |

## 22.11 Acceptance criteria

- `AC-22.1` Carrier launch: catapult end-speed accuracy within 1 % of target; 94 m in 2.2 s ± 0.1 s for the reference case.
- `AC-22.2` WOD controller achieves target wind-over-deck within ±1.5 m/s in 95 % of wind cases (0–15 m/s) within 4 min.
- `AC-22.3` Arrest: 70 m/s aircraft stopped within 100 ± 10 m under the wire curve.
- `AC-22.4` Layered defence: with 2 SAM channels and 4 simultaneous incoming missiles, leak-through rate matches the analytical expectation within 5 % over 10 000 Monte Carlo runs.
- `AC-22.5` Land avoidance: no ship path crosses land/shallow cells in 1 000 random fleet paths on coastal test maps.

---

# 23. CIVILIAN WORLD

`P10 · M4–M6` — The world should feel alive outside the immediate combat (outline §43, §103). The purpose is **immersion, not civilian combat**.

## 23.1 Intent and policy

- Civilian aircraft, ships, road and rail traffic, cities and infrastructure make the world feel inhabited.
- Civilians **avoid active combat zones**; airspace and sea lanes close around battles.
- Civilians are **non-targetable** by default: no missile lock, AI will not select them, bombs auto-hold on civil-flagged targets in assist modes. The game does not depict civilian casualties; a civilian entity destroyed by player action triggers a **rules-of-engagement violation** (mission penalty in "Realistic ROE" rule sets; harmless in sandbox if "civilians vulnerable" is enabled, §30). This protects tone and avoids platform/content issues.

## 23.2 Analytic ambient traffic

Ambient traffic is deliberately **not simulated**: its position is a **pure function** of `(route id, session time, seed)`, so the authority never replicates it and each client computes the same result locally.

```text
route k: polyline P_k (geodesic or road/sea path), length L_k, speed v_k, period Δ_k (departures every Δ_k seconds, offset φ_k = hash(seed, k) mod Δ_k)
instances: n = floor((t − φ_k)/Δ_k) − j  for j = 0 … ceil(L_k/(v_k Δ_k))
position of instance n: s = v_k · (t − φ_k − n Δ_k), valid for 0 ≤ s ≤ L_k;  p = P_k(s)
```

Only the visible/relevant subset (within sensor/render range of any client) is evaluated each frame. Civil entities are considered by the sim only when they might interact: on radar as low-priority "civil" tracks (clutter/identification practice), in AI collision-avoidance checks, and in the airspace-closure rule. Cost is O(visible instances).

## 23.3 Civilian air traffic

| Class | Altitude (MSL) | Speed | Routes |
|---|---|---|---|
| Airliners | 9 000–12 000 m | 220–250 m/s | Great-circle between airports with `scheduled_service = yes` (OurAirports) and large hubs; climb/descent profiles near terminals |
| Regional turboprops | 4 500–7 500 m | 130–160 m/s | Short routes (< 800 km) |
| General aviation | 300–3 000 m | 45–75 m/s | Local circuits near small airports |
| Cargo | 8 000–11 000 m | 200–240 m/s | Hub-to-hub |

Counts: up to 15 (Minimum) / 60 (Recommended) / 150 (High) simultaneously visible (§0.6.2). Density scales with distance to major airports (hub proximity), time of day (less at night), and region (remote ocean: oceanic tracks 2–6). Aircraft carry nav/anti-collision lights at night and contrails when `T < −40 °C` at altitude (ISA: above ≈ 8.5 km) with persistence by humidity.

**Airspace closure.** When a mission starts, the **combat zone** is the battlefield's bounding circle plus a 50 km margin. Routes intersecting it are excluded at generation time; aircraft inside are never spawned. If the combat zone grows (events, expanding war) civil instances inside the new radius divert (fade out over 20 s). Closure state is a deterministic function of mission events (replicated as events), so the clients stay consistent.

## 23.4 Civilian ships

Lanes: connect ports (Natural Earth ports and OSM `harbour`/`seamark:type=harbour`) by A\* on the sea navigation grid (§22.3.2) with preferences for deep water and straight segments, producing a lane graph; routes between port pairs weighted by port size. Fishing fleets cluster within 80 km of coastal towns (stationary patterns, 0–5 m/s). Speeds 6–13 m/s. Count 10 (Minimum) – 100 (High). Closure rule same as aircraft (naval combat zones: lanes avoid, vessels turn back).

## 23.5 Road, rail, and settlements

- **Road traffic:** vehicles along OSM roads, density by road class and nearby urban fraction, drawn only within 3 km of the camera (LOD: tiny instanced boxes with lights), at most 5 000 instances per frame; at night headlights are emissive points.
- **Rail:** trains on `railway=rail` lines drawn within 4 km; analytic position along the line.
- **Cities and towns:** night lights (emissive building windows and road lamp layers), smoke stacks, cooling towers, stadium floodlights, port cranes, wind turbines (OSM `generator:source=wind`) and radio towers as **obstacles** (hazards for low flying, registered in the collision layer if > 60 m), power lines (OSM `power=line`) as low-altitude hazards with wires drawn when within 1.5 km.
- **Landmarks:** a small curated list of globally recognisable physical landmarks (bridges, dams, mountains) can receive hand-tuned LOD meshes in later content packs (§47.5).

## 23.6 Interaction rules

| Interaction | Rule |
|---|---|
| Radar | Civil aircraft appear as tracks tagged `CIV` (transponder IFF code); AI and automatic targeting ignore them |
| Lock-on | Weapons cannot lock `CIV`-flagged entities in assisted modes; in Manual, unguided weapons can hit |
| Collision | Civil aircraft are real hit volumes only when within 8 km of a human-controlled aircraft (promotion to a simulated entity with simple FDM); a collision destroys both and triggers an ROE violation |
| Avoidance | Simulated civil aircraft apply TCAS-like avoidance against military aircraft within 2 km / 10 s |
| Sound | Distant airliner and ambient ship sounds (positional) |

## 23.7 Data sources and fallbacks

Airport schedules (OurAirports flag), ports (Natural Earth + OSM), roads and rail (OSM), settlements (GeoNames, OSM landuse), land cover (WorldCover) for urban fraction. Where data are missing, procedural defaults generate plausible traffic from population estimates.

## 23.8 Performance and networking

Evaluating ambient traffic costs ≤ 0.2 ms/frame (visible routes only); rendering instances ≤ 0.8 ms. No network traffic except the closure event and the seed. Civil entities promoted to simulation (§23.6) replicate like AI aircraft.

## 23.9 Acceptance criteria

- `AC-23.1` Two clients with the same seed and session time compute civilian positions within 1 m (3-D) for all visible instances.
- `AC-23.2` No civil route intersects the combat zone at mission start in 10 000 random generations.
- `AC-23.3` With "civilians vulnerable" off, no weapon in assisted modes can lock or auto-target a `CIV` entity (automated test over the weapon roster).
- `AC-23.4` Ambient evaluation cost ≤ 0.2 ms/frame with 150 visible aircraft and 100 ships.

---

# 24. WEATHER & ENVIRONMENT

`P2/P3 · M2 (sky, time, wind) → M4 (full weather, dynamic)` — Weather, time, atmosphere and sky (outline §44–47) change visibility, radar, handling, and visual identification, and provide the look of each place on Earth.

## 24.1 Intent

- Weather is a gameplay variable, not just a skybox: clouds hide you, rain degrades sensors, wind makes landings harder, storms are hazards.
- Time and place are geographically correct: the sun, moon and stars are where they should be.
- Everything is data-driven and deterministic from the seed (clouds and wind consistent across clients).

## 24.2 Weather model inputs

The `WeatherState` for a mission is produced by the **Weather Director** from:

1. **User choice:** preset (clear, cloudy, overcast, rain, storms, fog, snow, dust, heavy winds) or *Realistic/Random*.
2. **Climatology:** a ~1° monthly climatology grid (surface temperature, humidity, precipitation probability, cloud fraction, prevailing wind) derived from open reanalysis data (≈ 3–6 MB compressed; licence per Appendix H) so "Random" at a desert in July differs from a Siberian January.
3. **Seed:** deterministic variation.

Derived values:

```text
ΔT (ISA offset) = (T_surface_clim + 0.0065 · elevation_m) − 15 °C           (sea-level equivalent minus ISA 15 °C)
cloud base AGL ≈ 125 m · (T − T_dew)   (°C)                                   (lifting condensation level approximation)
sea-level pressure p₀' = 101 325 Pa + N(0, 800 Pa) clipped [95 000, 105 000]  (lows for storms, highs for clear)
```

## 24.3 Presets

| Preset | Cloud cover (oktas) | Cloud base AGL (m) | Precipitation | Visibility | Mean wind (m/s) | Turbulence σ (m/s) | Notes |
|---|---|---|---|---|---|---|---|
| **Clear** | 0–1 | > 2 500 | none | 40–80 km | 0–6 | 0–0.5 | Best visibility, long visual range |
| **Cloudy** | 3–5 | 1 500–3 000 (cumulus), 500–1 500 thick | none | 25 km | 3–10 | 0.5 | Cloud cover for ambush and hiding |
| **Overcast** | 8 | 600–1 500 (stratus) | optional drizzle | 10 km | 3–10 | 0.5 | Low ceiling, flat light |
| **Rain** | 7–8 | 400–1 000 | 2–10 mm/h (heavy to 25) | 4 km (heavy 1.5 km) | 5–12 | 1.0 | Wet runways, radar attenuation |
| **Storms** | 8 with Cb cells | 600–1 000, tops 9–13 km | 10–50 mm/h inside cells | 1.5 km in rain | 8–20 (gusts +50 %) | 1.5 (cells 6) | Lightning, severe turbulence in cells |
| **Fog** | ground layer | 0 (depth 50–300 m) | none | 100–1 000 m | 0–3 | 0.0 | Low-visibility approaches; high humidity |
| **Snow** | 6–8 | 500–1 200 | 1–5 mm/h water equivalent | 1–5 km | 3–12 | 0.5 | ΔT ≤ −5, snowy/icy runways |
| **Dust** | haze to 3 000 m | — | none | 200 m–5 km | 8–18 | 1.0 | Reduced contrast, sand abrasion (`P10`) |
| **Heavy winds** | 3–6 scattered | 1 000–2 500 | none–light | 15–30 km | 15–25 (gusts to 35) | 3.0 | Crosswinds, high sea state |

Rain-rate to visibility (mm/h → km, gameplay): 0 → 40; 1 → 10; 5 → 5; 10 → 3; 25 → 1.5; 50 → 0.8.

**Spatial structure.** Small battlefields use uniform weather. Large+ battlefields may use a **weather field**: a 32×32 grid of parameters (cloud cover, base, precipitation, wind, turbulence), bilinearly interpolated, optionally containing **fronts** (a line moving at 5–15 m/s with a transition band of 20–60 km) and **storm cells** (moving cylinders, radius 5–15 km, top 12 km).

**Dynamic changes.** Mission events (§28) can alter the target `WeatherState`; transitions interpolate over 5–15 min, with gust fronts producing sudden wind shifts. All changes are events in the mission timeline and replicated.

## 24.4 Weather effects across systems

| System | Effect |
|---|---|
| Visibility | Koschmieder: extinction `β = 3.912 / V_meteo`; used for fog rendering and visual detection (§16.6.2) |
| Radar | Rain attenuation (§16.3.4); clutter from heavy rain cells near the beam (target detection ×0.8 inside cell) |
| IR | Clouds block IRST/IR seekers (LOS); rain attenuates (τ ×0.6); fog severely attenuates |
| Visual | Clouds/fog block LOS (§16.9.3); haze reduces contrast |
| Flight | Wind, turbulence (§11.4), icing (`P10`: airframe/engine icing in cloud below 0 °C), thunderstorm cells: severe turbulence, hail damage to canopy/radome (`P10`) |
| Surface | Wet (rain in last 30 min): braking μ ×0.57; snow/ice per §10.4 |
| Ocean | Sea state from wind (Beaufort table below) |
| Audio | Wind noise, rain on canopy, thunder |
| Contrails | Form at altitude when `T < −40 °C` (ISA ≳ 8.5 km), longer-lasting in humid air; affects visual detection |
| Sensors/EO | Cloud cover blocks EO/TV targeting pods |

**Beaufort scale → sea state (typical; used for ocean rendering, ship speed, deck motion):**

| Bft | Wind (m/s) | Significant wave height Hs (m, typical) | Description |
|---|---|---|---|
| 0 | < 0.5 | 0 | Calm |
| 1 | 0.5–1.5 | 0.1 | Light air |
| 2 | 1.6–3.3 | 0.3 | Light breeze |
| 3 | 3.4–5.5 | 0.8 | Gentle breeze |
| 4 | 5.5–7.9 | 1.3 | Moderate breeze |
| 5 | 8.0–10.7 | 2.3 | Fresh breeze |
| 6 | 10.8–13.8 | 3.5 | Strong breeze |
| 7 | 13.9–17.1 | 4.8 | Near gale |
| 8 | 17.2–20.7 | 6.5 | Gale |
| 9 | 20.8–24.4 | 8.5 | Strong gale |
| 10 | 24.5–28.4 | 10.5 | Storm |
| 11 | 28.5–32.6 | 13.5 | Violent storm |
| 12 | ≥ 32.7 | 16 | Hurricane |

(Descriptive wave heights are approximate for fully developed open-ocean seas; the game uses them as gameplay parameters.)

## 24.5 Time of day and date (outline §45)

Players choose a **date** (default: today's date) and a **time preset**: Dawn, Morning, Afternoon, Evening, Sunset, Night, Random. Lighting must be geographically appropriate, so each preset is defined by a **sun-elevation condition**, resolved at the chosen location and date:

| Preset | Condition (sun-centre elevation, with the sun moving) |
|---|---|
| Dawn | −3°, rising |
| Morning | +25°, rising |
| Afternoon | +40°, setting |
| Evening | +12°, setting |
| Sunset | −0.83°, setting (official sunset: upper limb on the refracted horizon) |
| Night | Sun below −18° (astronomical night), at local solar midnight |
| Random | Uniform over the day, excluding midday glare conditions with probability 0.3 |

**Resolution algorithm.** Sample the sun elevation over the UTC day at 5-minute steps at the location, find the interval where the elevation crosses the target with the right slope sign, refine by bisection to 1 s. **Polar edge cases:** if the target elevation is never reached on that date (midnight sun or polar night), choose the closest achieved elevation and show a note ("Sun does not set on this date at this latitude"). The resolved UTC instant becomes the session start (§7.6).

## 24.6 Astronomy

All formulas are low-precision closed forms adequate for rendering and gameplay (visual error ≲ 0.05° for the sun, ≲ 1.5° for the moon).

**Julian date** from calendar date (Y, M, D) and UT hours (valid 1901–2099):

```text
JD = 367Y − floor( 7 (Y + floor((M + 9)/12)) / 4 ) + floor(275 M / 9) + D + 1721013.5 + UT/24 − 0.5·sgn(100Y + M − 190002.5) + 0.5
```

Check: 2000-01-01 12:00 UT → JD 2451545.0. Let `n = JD − 2451545.0`.

**Sun** (Astronomical Almanac low-precision):

```text
L = 280.460° + 0.9856474° n            mean longitude
g = 357.528° + 0.9856003° n            mean anomaly
λ = L + 1.915° sin g + 0.020° sin 2g   ecliptic longitude
ε = 23.439° − 0.0000004° n             obliquity
RA = atan2(cosε sinλ, cosλ);   δ = asin(sinε sinλ)
GMST = 280.46061837° + 360.98564736629° n;  LMST = GMST + λ_east;  H = LMST − RA   (hour angle)
```

Direction in local ENU (observer latitude φ):

```text
E = −cosδ sinH
N =  sinδ cosφ − cosδ cosH sinφ
U =  sinδ sinφ + cosδ cosH cosφ              elevation = asin(U), azimuth from north = atan2(E, N)
```

**Refraction (Bennett):** apparent elevation correction `R_arcmin = 1 / tan( h + 7.31/(h + 4.4) )` with `h` in degrees (add `R/60` degrees to the geometric elevation for rendering the sun disc at the horizon).

**Moon:** mean elements `L' = 218.316° + 13.176396° n`, `M' = 134.963° + 13.064993° n`, `F = 93.272° + 13.229350° n`; ecliptic longitude `λ_m ≈ L' + 6.289° sin M'`, latitude `β_m ≈ 5.128° sin F`, then the same equatorial/horizontal conversion. **Phase:** `phase = ((JD − 2451550.26) mod 29.530588) / 29.530588` (new moon reference 2000-01-06 18:14 UT); illuminated fraction `k = (1 − cos ψ)/2` with ψ the sun–moon elongation. Moonlight adds ambient illumination `0.25 lux · k` at the surface (night rendering) and affects visual detection (§16.6.2).

**Stars:** a packed catalogue of ≈ 9 000 stars to magnitude 6.5 (position RA/Dec, magnitude, colour index; < 150 KB) from the public-domain Yale Bright Star / Hipparcos-derived catalogues, plus a Milky Way panorama (public-domain star map). Stars are rotated into the local sky by the same `H = LMST − RA` transform, drawn as magnitude-scaled point sprites with atmospheric extinction near the horizon and fading with sky brightness (twilight).

## 24.7 Atmospheric simulation (outline §46)

Altitude, temperature, air density and pressure come from the ISA with `ΔT` and `p₀'` offsets (§11.3). Performance changes with altitude through ρ and a. Weather supplies winds aloft and temperature offsets; the **density-altitude** readout informs take-off planning (§14.5). The visual atmosphere (scattering) uses the same transmittance LUTs as the globe (§9.9) with weather-modulated aerosol density and Mie scattering coefficient.

## 24.8 Sky and clouds (outline §47)

### 24.8.1 Components

- Sun disc and corona with glare (limited to avoid unreadable HUD; reduced by accessibility settings).
- Atmospheric scattering (Rayleigh + Mie) with multiple-scattering LUT, aerial perspective (distance haze), horizon haze.
- Moon with phase, earthshine.
- Stars and Milky Way (§24.6).
- Clouds (below) and precipitation.

### 24.8.2 Cloud types

| Type | Base (m AGL) | Thickness (m) | Look |
|---|---|---|---|
| Stratus | 100–700 | 200–800 | Flat grey sheets |
| Stratocumulus | 500–1 500 | 300–1 000 | Rolls |
| Cumulus (fair) | 800–3 000 | 500–2 000 | Puffy, scattered |
| Towering cumulus / cumulonimbus | 800–2 000 | up to 12 000 | Anvils; storms |
| Altostratus/altocumulus | 2 500–6 000 | 500–2 000 | Mid-level sheets |
| Cirrus | 6 000–12 000 | 200–1 500 | Wispy ice crystals |

### 24.8.3 Two-layer representation (render vs gameplay)

- **Gameplay density function** `ρ_c(x, y, z, t)`: a deterministic low-frequency analytic function of the weather field, layer parameters and a seeded 3-D value-noise with 2–4 octaves (integer hashing and basic arithmetic only, so it is bit-reproducible per ADR-10) used by the sim for LOS occlusion (§16.6.2/16.9.3), in-cloud visibility, icing and lightning probabilities. Sampled by marching with 16–32 samples per ray.
- **Render clouds:** volumetric ray-marched in WebGPU at ¼ resolution with temporal reprojection (≈ 3–4 ms at 1080p High) using the same coverage and base/top definitions plus high-frequency Worley–Perlin detail textures (generated at load by compute: 128³ shape, 32³ detail). Because both layers share the low-frequency function, **what the player sees is what the sim uses**; the extra detail never changes occlusion by more than ± 5 % optical depth.

### 24.8.4 Flying through clouds

Inside a cloud: visibility drops to 30–100 m (density-dependent), white-out shader, water droplets on the canopy, lighting falls to diffuse; HUD remains. Icing and turbulence apply per weather. Leaving a cloud sunward restores glare. Clouds can hide aircraft from visual detection and IR (not radar).

### 24.8.5 Precipitation, fog, dust, lightning

Rain: screen-space streaks + near-field particles; canopy rivulets; splash on ground. Snow: flakes and accumulation tint on terrain. Fog: height-based exponential `ρ_fog(z) = ρ₀ e^{−z/H_f}` with `H_f` 150 m. Dust: dense wall (haboob) as a moving volumetric front. Lightning: flashes illuminate clouds (light probe pulses) and bolts to ground in Cb cells; thunder delayed by distance/343 m/s.

## 24.9 Wind-field authority and replication

Server holds the authoritative weather field; clients receive `WeatherState` (≈ 200 bytes) at join and on change, and evolve cosmetic cloud motion locally (wind advects clouds). Per-aircraft wind and turbulence used for the FDM are computed on the authority only (clients predicting their own aircraft use the same function and server-issued turbulence seed; mismatch is corrected by reconciliation, §33.5).

## 24.10 Performance

| Item | Budget (Recommended, 1080p High) |
|---|---|
| Sky + scattering | ≤ 0.8 ms |
| Volumetric clouds | ≤ 3.5 ms (adaptive resolution; falls back to billboard clouds on Minimum) |
| Precipitation + fog | ≤ 0.6 ms |
| Ocean FFT | ≤ 1.2 ms (see §22.9) |
| Gameplay cloud sampling | ≤ 0.1 ms per tick |

## 24.11 Edge cases

| Case | Handling |
|---|---|
| Dates outside 1950–2100 | Clamped with warning |
| Polar day/night | §24.5 fallback |
| High altitude (> 12 km) | Clouds below; clear dark-blue sky; stars at day above 30 km only |
| Missing climatology | Falls back to latitude/season formulae |
| Weather change during landing | Smooth transition; no step changes in wind > 3 m/s per second |
| Low-end devices | Cloud and ocean quality tiers (§60.3) |

## 24.12 Acceptance criteria

- `AC-24.1` The Julian date formula gives 2451545.0 for 2000-01-01 12:00 UT, and solar elevation matches a reference almanac (e.g. NOAA solar calculator values) within 0.05° for 100 random (date, location) pairs in 1960–2090.
- `AC-24.2` Time-of-day preset resolution places the sun within 0.05° of target elevation (excluding polar fallbacks), and flags polar fallbacks correctly.
- `AC-24.3` Moon phase within ±0.02 of a reference for 50 dates; illuminated fraction monotonic through the cycle.
- `AC-24.4` Gameplay cloud occlusion and rendered cloud density correlate (Pearson ≥ 0.9) across 1 000 sampled rays; occlusion decisions agree between two builds (WASM/native).
- `AC-24.5` Weather presets produce visibility within ±15 % of the table and radar attenuation per §16.3.4.
- `AC-24.6` Sea-state mapping matches the Beaufort table for 13 wind speeds.

---

# 25. MISSION SYSTEM

`P5 · M4` — The mission system creates combat scenarios from reusable components (outline §48). Missions are **not** primarily hand-scripted levels. They are assembled dynamically from location, objectives, aircraft, factions, terrain, weather, AI, reinforcements and strategic conditions, and run by a data-driven director.

## 25.1 Intent

- A mission is *data*: a graph of phases, objectives, groups and triggers that the Sim Core executes. The same executor runs generated missions, user-made missions (§51), 1v1 arenas (§31), and campaign operations (§34).
- Objectives evolve during play so missions never feel like static checklists (outline §52).
- Every mission is explainable: briefings, in-mission objective text, and debriefs say *what is happening and why*.

## 25.2 Mission model

```text
Mission
 ├── provenance        (seed, versions, data pack, request hash)
 ├── battlefield       (§8)
 ├── environment       (date/time, weather, wind) (§24)
 ├── rules             (rule set: ROE, respawn, scoring, restrictions) (§25.9)
 ├── factions          (side → faction)
 ├── bases             (airports/carriers/improvised strips with roles)
 ├── spawns            (player slots)
 ├── groups            (forces: air, ground, naval, civil; activation; routes; tasks)
 ├── objectives        (graph with states and conditions)
 ├── phases            (state machine; escalation, §28)
 ├── triggers/events   (condition → action rules)
 ├── reinforcements    (waves and director parameters, §28.5)
 └── success / failure (boolean expressions over objectives and events)
```

### Example `MissionDef` (abridged)

```json
{
  "schema": "mission/1",
  "provenance": { "seed": "FA3K-9T2M-QX71", "game_version": "0.1.0", "content_version": "2026.10.1", "datapack": "geo-2026.09", "request_hash": "6c1f0a9e" },
  "title_key": "mission.air_superiority.title",
  "battlefield": { "ref": "battlefield/1", "size_class": "huge" },
  "environment": { "utc": "2026-06-21T02:10:00Z", "time_preset": "sunset", "weather": "cloudy", "wind": { "dir_deg": 270, "speed_mps": 8 } },
  "rules": "ruleset/air_superiority",
  "factions": { "blue": "azure_league", "red": "meridian_pact" },
  "bases": [ { "id": "base_blue", "airport": "KMRY", "side": "blue", "role": "home" },
             { "id": "base_red", "airport": "KBFL", "side": "red", "role": "enemy_home" } ],
  "spawns": [ { "slot": 0, "base": "base_blue", "start": "runway_hot" } ],
  "groups": [
    { "id": "g_red_cap1", "side": "red", "squadron": "sqn_vulture_3", "count": 4, "activation": { "phase": "p2" },
      "route": [ { "wp": "patrol", "pos": [35.6, -119.2], "alt_m": 8000 } ], "task": "cap" },
    { "id": "g_red_radar", "side": "red", "kind": "ground", "units": [ { "type": "ewr_1", "count": 1 } ], "activation": { "phase": "p1" } }
  ],
  "objectives": [
    { "id": "o_radar", "kind": "destroy", "target": { "group": "g_red_radar" }, "priority": "primary", "state": "active", "text_key": "obj.destroy_radar" },
    { "id": "o_strike", "kind": "intercept", "target": { "group": "g_red_strike" }, "priority": "primary", "state": "hidden", "text_key": "obj.intercept_strike" }
  ],
  "phases": [
    { "id": "p1", "name": "approach", "on_enter": [ { "do": "activate_objective", "id": "o_radar" } ],
      "exit": [ { "when": { "op": "detected", "group": "g_red_cap1", "by": "blue" }, "goto": "p2" } ] },
    { "id": "p2", "name": "contact", "on_enter": [ { "do": "activate_group", "id": "g_red_cap1" }, { "do": "radio", "key": "radio.bandits_detected" } ],
      "exit": [ { "when": { "op": "objective_state", "id": "o_radar", "is": "complete" }, "goto": "p3" } ] }
  ],
  "triggers": [
    { "id": "t_strike", "when": { "op": "objective_state", "id": "o_radar", "is": "complete" },
      "do": [ { "do": "spawn_group", "id": "g_red_strike" }, { "do": "activate_objective", "id": "o_strike" } ], "once": true }
  ],
  "reinforcements": { "director": "default", "rate_per_hour": 3 },
  "success": { "all": [ "o_radar", "o_strike" ] },
  "failure": { "any": [ "player_dead", "timeout", "home_base_lost" ] }
}
```

## 25.3 Groups

A **group** is the unit of spawning and tasking:

| Field | Meaning |
|---|---|
| `side`, `faction`, `squadron` | Ownership and identity (§20) |
| `kind` | `air`, `ground`, `naval`, `civil`, `static` |
| `units` / `count` | Composition (aircraft ids with loadouts; ground units; ships) |
| `activation` | `immediate`, `phase:<id>`, `trigger:<id>`, `time:<s>`, `detected:<group>`, `distance:<entity,m>` |
| `spawn` | `airbase:<id>` (ground/hot/cold start), `air:<pos,alt,hdg,speed>`, `boundary:<edge,pos>` (entering the battlefield), `carrier:<id>`, `ground:<pos>` |
| `route`, `task` | Waypoint list with types (§50.6) and task (`cap`, `intercept`, `escort`, `strike`, `sead`, `patrol`, `orbit`, `convoy`, `defend`, `hold`) |
| `skill`, `doctrine`, `roe` | AI parameters |
| `despawn` | Conditions (destroyed, left area, mission end, landed) |
| `tags` | For selection in triggers (`aa`, `strike_package`, `boss`) |

## 25.4 Objectives

### 25.4.1 Objective kinds

| Kind | Success condition (default) | Typical failure |
|---|---|---|
| **Destroy** | Target group/structure destroyed (all or ≥ n %) | Timeout; target escapes the area |
| **Defend** | Protected entity survives until time T or event E | Entity destroyed or captured |
| **Escort** | Escortee reaches destination alive | Escortee destroyed or stalls |
| **Intercept** | Group destroyed or turned back before reaching a line/zone | Group reaches zone |
| **Reach** | Player/formation enters a zone (optionally with altitude/speed constraints) | Timeout |
| **Survive** | Remain alive/operational for duration | Death |
| **Capture** | Airport/base captured per §36 rules | Counter-capture |
| **Recon** | Observe/identify (detect within range with sensor/visual/pod; hold for N s) and survive extraction | Observer killed, timeout |
| **Suppress/SEAD** | Network degraded below threshold (§21.5) | Strike package losses > X |
| **Deliver** | Cargo transport reaches destination | Transport destroyed |
| **Neutralise (boss)** | Boss destroyed/disabled or forced to retreat | — |
| **Return** | Land at a base (or extraction) | Fuel out |

### 25.4.2 Objective properties

```jsonc
{ "id": "o_x", "kind": "destroy", "priority": "primary|secondary|bonus", "state": "hidden|active|complete|failed|cancelled",
  "target": { "group": "g1" }, "progress": { "current": 0, "required": 1, "unit": "units|percent|seconds" },
  "visible_to": ["blue"], "marker": { "type": "area", "pos": [lat, lon], "radius_m": 3000, "reveal": "known|approx|none" },
  "on_complete": [ ... ], "on_failed": [ ... ], "text_key": "obj.destroy_radar", "reward": { "score": 500, "campaign": { "intel": 1 } } }
```

State machine: `hidden → active → { complete | failed | cancelled }`. **Chaining** (outline §52): `on_complete`/`on_failed` actions activate other objectives, spawn forces, or change phases. Objectives are bound to marker types (area, point, entity) with *reveal level* depending on intelligence/realism (§42): the player may know only an approximate area.

## 25.5 Conditions, actions, and the trigger engine

Mission logic is expressed in a **small declarative language**, compiled into bytecode for a sandboxed stack VM (no arbitrary scripting, essential for user-shared missions, §51.5).

| Condition ops (examples) | Meaning |
|---|---|
| `units_alive(group) ≤ n`, `units_destroyed(group) ≥ n`, `percent_destroyed(group) ≥ p` | Group status |
| `entered(zone, side, min_count)`, `left(zone, …)`, `inside(entity, zone)` | Geometry |
| `time_elapsed(s)`, `time_since(event, s)`, `utc_after(t)` | Time |
| `objective_state(id) == complete` | Objective logic |
| `detected(group, by=side)`, `identified(group, by)` | Information |
| `fuel_below(entity, frac)`, `damage_above(entity, frac)` | State |
| `weather_is(preset)`, `visibility_below(m)` | Environment |
| `random(p, stream)` | Seeded chance |
| `and/or/not`, comparison | Boolean logic |

| Action ops (examples) | Meaning |
|---|---|
| `spawn_group`, `activate_group`, `despawn_group`, `order_group(task…)` | Forces |
| `activate_objective`, `complete_objective`, `fail_objective`, `add_objective` | Objectives |
| `goto_phase` | Phase machine |
| `radio(key, params)`, `show_text`, `set_marker` | Communication/UI |
| `set_weather`, `set_time_scale`, `set_roe`, `activate_sam`, `set_emcon` | World/rules |
| `reinforce(wave)`, `set_boundary`, `win`, `lose`, `trigger_cinematic_cue` | Flow |

Evaluation: event-driven re-evaluation of the conditions that depend on changed state, plus a 4 Hz polling pass for time/geometry conditions. Order is deterministic (triggers sorted by id). A trigger fires `once` or `repeat` with a cooldown. All state transitions emit `MissionEvent`s recorded in the replay and used by statistics (§65).

## 25.6 Phases

Phases structure escalation (§28). A `Phase` has: `enter` actions, `exit` rules with targets (`goto`), `timeout` (optional), `objectives_on_enter`, forces activated, and a *budget slice* for the reinforcement director. Phase graphs may branch (e.g. Crisis → Elite Response or → Climax). Not every mission uses every phase. A mission always has at least a `Resolution` phase.

## 25.7 Failure and success (outline §53–54)

**Failure conditions** (each mission type selects a subset):

- Player destroyed (and out of respawns).
- Friendly forces destroyed (≥ threshold of the player's side).
- Primary objective lost/failed.
- Protected target destroyed.
- Timer expires.
- Base captured (home base lost).
- Insufficient operational aircraft.
- Out of area (boundary Stage 4, §8.5).

**Success conditions:** destroy priority targets; protect an objective; capture an airport; destroy strategic infrastructure; survive a specified duration; complete an escort; return safely to base.

Both are Boolean expressions: `success = all/any(objectives, events)`, `failure = any(...)`. When both become true on the same tick, failure wins unless a rule set says otherwise. On resolution the director runs the **Resolution phase** (extraction, landing, debrief) with a grace period (default 20 s) for pending kills to register.

## 25.8 Mission director runtime

```text
EventBus ──► TriggerEngine ──► actions ──► ObjectiveManager / PhaseMachine / GroupSpawner / Radio / ReinforcementDirector
     ▲                                           │
     └──────────── world events (kills, entries, detections, damage) ◄─── Sim Core systems
```

Runs only on the authority (host/server). Costs: ≤ 0.1 ms/tick typical. All spawns go through the `GroupSpawner`, which enforces entity caps (§0.6.2) and spawn-safety rules (§28.5).

## 25.9 Rule sets (data)

A **rule set** is a reusable configuration of mission rules:

```json
{ "schema": "ruleset/1", "id": "ruleset/air_superiority",
  "victory": { "mode": "mission_defined" }, "respawn": { "mode": "limited", "count": 2, "delay_s": 20, "location": "home_base" },
  "roe": { "default": "weapons_free_hostile", "bvr_requires_id": "iff_or_datalink", "civilians_vulnerable": false },
  "friendly_fire": false, "scoring": "default", "time_limit_s": 2400,
  "equipment": { "era": "any", "aircraft_allow": ["*"], "weapons_deny": [], "max_assist": "full", "cm_unlimited": false },
  "boundary": { "policy": "mission_fail", "countdown_s": 30 },
  "dials_lock": [] }
```

ROE values: *weapons hold*, *weapons tight* (fire only when attacked / identified hostile), *weapons free (hostile-declared)*, *weapons free (all non-friendly)*. `dials_lock` forces realism dials for fair play in multiplayer (§59). 1v1 rule sets live in §31.4.

## 25.10 Briefing and debrief

**Briefing** is auto-generated from the mission data using template text with parameter substitution (localized): situation, objectives (primary/secondary), forces (known friendly, estimated enemy subject to intel), threat overview (SAM rings, known airbases), route and waypoints, weather/time, rules/ROE, communications plan (callsigns, AWACS), and loadout recommendations. A map pane shows the battlefield, routes and threat rings (revealed per intel level). Reading time target ≤ 40 s; "Skip" always available; briefing text is accessible (screen-reader friendly, §57).

**Debrief:** objective outcomes, kills by category, losses, weapons used and hit rate, flight time, fuel used, damage taken, score and grade, timeline graph of key events, replay save prompt, seed share card, and (campaign) strategic consequences.

## 25.11 Scoring

```text
score = Σ kill_points(class) + Σ objective_rewards + survival_bonus + accuracy_bonus − penalties
kill_points: fighter 100, bomber 150, AWACS/tanker 250, SAM site 120, radar 100, armour 30, ship (small) 150, carrier 800, boss 600 (design)
penalties: ROE violation −500, friendly fire kill −300, aircraft lost −200
grade: S ≥ 1.3 × par, A ≥ 1.0, B ≥ 0.75, C ≥ 0.5, D otherwise;  par = Σ expected rewards computed by the generator (§26.8)
```

## 25.12 File format and provenance

Missions serialise to canonical JSON (stable key order) and a compact binary pack. `provenance` records seed, game version, content version, data pack and request hash so a mission can be regenerated or verified (§26.3). User edits mark a mission `derived` with a link to the parent. Schema versioning: `mission/1` → migration chain in the loader (§64.5).

## 25.13 Edge cases

| Case | Handling |
|---|---|
| Objective target already destroyed before activation | Auto-complete with `pre_satisfied` flag |
| Deadlock (no active objective, no success) | Director safety: after 120 s, activate `fallback_objective` (return to base) |
| Trigger loops | `once` default; repeat triggers require cooldown ≥ 1 s; the VM has an instruction budget per tick |
| Player joins late (co-op) | Receives current objective state; spawns per rule set; briefing summary |
| Time acceleration | Timers use sim time; triggers evaluate each tick |

## 25.14 Acceptance criteria

- `AC-25.1` The trigger VM executes 10 000 trigger evaluations in < 1 ms and rejects scripts exceeding instruction budgets.
- `AC-25.2` A mission file round-trips (canonical JSON → binary → JSON) bit-identically.
- `AC-25.3` Objective chains from the outline §52 example (radar → strike package → elite squadron → command aircraft) run correctly in a scripted test.
- `AC-25.4` Failure precedence rule verified (simultaneous success and failure → failure).
- `AC-25.5` Every user-visible mission string is a localization key (lint).

---

# 26. MISSION GENERATOR

`P5 · M4` — The generator takes a request and produces a complete mission (outline §55–56). It is the heart of replayability and of the campaign's operations (ADR-14).

## 26.1 Contract

```text
GeneratorRequest ──► MissionGenerator(seed) ──► MissionDef
```

Inputs (outline §55): **Location, Battlefield Size, Player Count, Aircraft Era, Enemy Strength, Mission Type, Weather, Time, Difficulty, Airbase Availability, Naval Presence, Ground Combat, Boss Probability, AI Skill.**

Outputs: **Battlefield, Spawn Locations, Objectives, Enemy Forces, Friendly Forces, Ground Units, Naval Forces, Airbases, Mission Phases, Reinforcements, Victory Conditions, Defeat Conditions.**

## 26.2 Request parameters

| Parameter | Domain | Notes / mapping |
|---|---|---|
| Location | Airports A/B, or point, or region polygon | §8, §9 |
| Battlefield size | Small…Extreme or custom | §0.6.4 |
| Player count | 1–16 | Scales friendly/enemy budgets (§26.8) |
| Aircraft era | G3, G4, G4.5, G5, Experimental, Mixed | §13.6 |
| Enemy strength | Light 0.5 / Moderate 0.8 / Even 1.0 / Heavy 1.5 / Overwhelming 2.5 | Multiplier on player-side budget |
| Mission type | Any of §27, or *Auto* | Auto picks by geography and history |
| Weather | Preset or *Realistic/Random* | §24 |
| Time | Preset or exact | §24.5 |
| Difficulty | Easy…Simulator | Affects AI skill distribution, resource and dial defaults (§59) |
| Airbase availability | None (air start) / Home only / Both / Many (all usable) | §10, §48 |
| Naval presence | Off / Low / Medium / High (only where ocean ≥ threshold) | §22 |
| Ground combat | Off / Low / Medium / High | §21 |
| Boss probability | 0–1 | §29 |
| AI skill | Fixed level or distribution | §19.6 |
| Seed | 60-bit | §26.3 |
| Factions | Pair or *Auto* | §37 |
| Rule set | id | §25.9 |
| Context | Campaign inputs (strategic situation), history ring, hardware caps | §34, §69 |

## 26.3 Determinism and seeds (ADR-10)

The generator is **deterministic**: identical `(provenance, request)` produce identical `MissionDef` bytes. Rules:

1. **Seed:** a 60-bit master seed. Share code: 12 characters of **Crockford Base32** (`0123456789ABCDEFGHJKMNPQRSTVWXYZ`, which excludes I, L, O, U) in groups of four, e.g. `FA3K-9T2M-QX71`. Numeric seeds (e.g. `847291`, as in the outline) are accepted and widened: `seed = SplitMix64(n) & (2^60 − 1)`.
2. **RNG streams:** `rng(stream) = xoshiro256**` seeded by `SplitMix64(master ^ fnv1a64(stream_name))`. Each pipeline stage and each sub-decision uses a *named* stream (`"forces"`, `"placement/sam"`, `"events"`, …) so adding randomness to one stage never perturbs another.
3. **Iteration order:** all collections iterate in sorted stable-id order; no hash-map iteration; sorting is stable with explicit tie-breakers.
4. **Numerics:** integer arithmetic and `Fixed32` (Q16.16) for grid indices, scores used for tie-breaking, and weights; floating-point is allowed for geometry from geodata but only through the project's `DMath` (software `sin/cos/atan2/exp/ln/pow` via the `libm` crate; `sqrt` and basic ops are IEEE exact in WebAssembly). Scores that decide a choice are quantised to `Fixed32` before comparison.
5. **Provenance binding:** the generator output depends on the **game version, content version and geodata pack**. Provenance is stored in the mission; loading a seed under a different provenance yields a warning ("data pack differs: placements may differ; high-level configuration preserved").
6. **Verification:** CI golden test: 1 000 (seed, location, request) tuples hash to stored values; the WASM and native builds must agree.

**Share links.** A mission request is encoded into a URL fragment: `#m=<seed>&v=<game>&d=<datapack>&c=<base64url(config)>` where `config` is a ≤ 64-byte packed struct (location, size, mission type, era, enemy strength, weather, time, difficulty, toggles). A "seed card" (outline §56) renders the same data:

```text
MISSION SEED: FA3K-9T2M-QX71     LOCATION: 36.2° N, 121.8° W     SIZE: LARGE
MISSION: AIR SUPERIORITY         WEATHER: CLOUDY                ENEMY: HEAVY
```

## 26.4 Pipeline

```text
S0 Context analysis ──► S1 Scenario selection ──► S2 Phase graph ──► S3 Force composition ──► S4 Placement
                                                                                                  │
S8 Packaging ◄── S7 Validate & repair ◄── S6 Environment ◄── S5 Objectives, events, reinforcements ◄┘
```

Each stage is a pure function `(state, request, rng_stream) → state'`. Validation failures retry the failing stage with the next sub-stream (`stream#2`, up to 8 attempts) before relaxing constraints.

### S0 Context analysis (geography)

Sample the battlefield at a 1–2 km analysis grid (≤ 512×512) using the base DEM (z ≤ 10 first, refined near features):

- Elevation statistics, relief (std-dev), slope map, land cover fractions, water/coast, drainage.
- **Place features** (reused by placement and anti-repetition): ridgelines (for masking and SAM placement), valleys (low-level routes), passes/chokepoints, islands, coast segments, flat areas (improvised airfields, §10.8), city clusters, airports (inside and within 50 km of the boundary), roads/rails.
- **Character tags:** `mountain`, `desert`, `ocean`, `arctic`, `forest`, `urban`, `coastal`, `islands`, `plains`, with weights; used to bias mission types and force mixes (§46).

### S1 Scenario selection

Pick a mission type (if Auto) and a *template variant* from the catalog, weighting by geography fit (e.g., Carrier Defense needs ocean fraction > 40 %, Convoy Attack needs a road network, Boss Hunt needs enough room), faction/era compatibility, and **anti-repetition** penalties from recent history (§69.3). Add optional **twists** (§69.4) from the twist table (e.g. `weather_turn`, `defector`, `decoys`, `low_fuel_start`, `awacs_lost`).

### S2 Phase graph

Instantiate the escalation pattern (§28.2) for the type: select a sequence/branching of phases from the pattern library, bind slots (e.g. `Threat Expansion` ← one of {bomber raid, SAM activation, naval sortie}).

### S3 Force composition (budget-based)

See §26.8: compute budgets and fill roles to meet them, respecting caps and doctrine.

### S4 Placement

Place bases/spawns, objectives, ground units, SAM networks, naval groups, patrol orbits, tanker tracks, AWACS orbit, entry corridors, and civilian closure (§26.7).

### S5 Objectives, events, and reinforcements

Create the objective graph, schedule phase events, define reinforcement waves and their entry points, assign the boss (if rolled) and its introduction event (§29).

### S6 Environment

Set date/time and weather per request or climatology; ensure compatibility with mission type and rule set (e.g., carrier ops at Hard realism not in Beaufort > 7).

### S7 Validate and repair

See §26.9.

### S8 Packaging

Canonicalise, hash, write provenance, produce briefing text and the overview map data.

## 26.5 Pseudo-code

```rust
pub fn generate(req: &GeneratorRequest, prov: &Provenance, geo: &dyn GeoService, catalog: &Catalog) -> Result<MissionDef, GenError> {
    let master = req.seed;
    for attempt in 0..MAX_ATTEMPTS {
        let mut ctx = GenCtx::new(req, prov, geo, catalog, master, attempt);
        let analysis = stage_context(&mut ctx)?;                       // S0
        let scenario = stage_scenario(&mut ctx, &analysis)?;           // S1
        let phases   = stage_phases(&mut ctx, &scenario)?;             // S2
        let forces   = stage_forces(&mut ctx, &scenario, &phases)?;    // S3
        let placed   = stage_placement(&mut ctx, &analysis, &forces)?; // S4
        let plan     = stage_objectives(&mut ctx, &scenario, &placed, &phases)?; // S5
        let env      = stage_environment(&mut ctx, &scenario)?;        // S6
        let mission  = assemble(req, prov, analysis, scenario, phases, forces, placed, plan, env);
        match validate(&mission, &ctx) {                               // S7
            Ok(report) => return Ok(package(mission, report)),         // S8
            Err(issues) if attempt + 1 < MAX_ATTEMPTS => ctx.record_issues(issues),
            Err(issues) => return relax_and_retry(req, prov, geo, catalog, issues),
        }
    }
    unreachable!()
}
```

## 26.6 Output (`MissionDef`)

The output covers every item in the outline's list: battlefield (`§8`), spawn locations (`spawns`), objectives, enemy forces (`groups` with `side: red`), friendly forces, ground units, naval forces, airbases (`bases`), mission phases (`phases`), reinforcements, victory conditions (`success`), defeat conditions (`failure`) — see §25.2.

## 26.7 Placement algorithms

All placements use **candidate generation + scoring + weighted top-k selection**:

```text
candidates ← Poisson-disc samples (spacing s) within the region  ∪ feature-derived points (ridges, valleys, cities, airports)
score(c) = Σ_i w_i · f_i(c)      (f_i normalised 0..1; weights per object type)
pick: take top K (K = 5), choose with probability ∝ exp(score / T)  using rng(stream)   (T = 0.15)
```

| Object | Key score terms |
|---|---|
| **Player base / spawn** | Airport suitability (runway length vs aircraft), distance to battlefield centre, protected from the enemy approach side, tanker access |
| **Enemy base / objectives** | Distance from the player base in `[0.5, 0.85]·L`, airfield or feature suitability, terrain access for ingress routes with at least one low-exposure corridor |
| **Air start points** | At least `R_min_sep` from enemy radar coverage (default 80 km for BVR types, 25 km for Quick Dogfight), above terrain by 1 000 m, heading toward the objective |
| **SAM sites** | §21.8: viewshed coverage of approach sectors, flat pads, 3–8 km from protected assets, road access, mutual support (overlapping fire zones), depth layering |
| **EWR/GCI radar** | Elevated sites with wide LOS; within 15 km of command post |
| **Ground columns** | Road graph paths between objectives; avoid cities if the rule set penalises collateral |
| **Naval groups** | Sea grid with depth ≥ requirement; spacing from coast ≥ 5 km; formations oriented to threat axis |
| **CAP stations** | Racetrack orbits with leg 40–80 km, positioned on the threat axis, outside the enemy's immediate SAM umbrella |
| **Tanker tracks** | 80 km leg at 7 000–8 500 m, behind friendly lines, ≥ 120 km from enemy fighters' CAP |
| **AWACS orbit** | Behind friendly lines, ≥ 1.2 × enemy fighter radar `R_ref` from the nearest enemy airbase, 9 000–10 500 m |
| **Reinforcement entry** | Boundary points on the enemy side or enemy airbase spawn; never within 40 km of any human unless via airbase |
| **Ambush points** | Terrain-masked positions along the player's likely route (valleys, behind ridges) |
| **Improvised airstrips** | §10.8 |

## 26.8 Force budgeting

**Threat value (TV).** Every aircraft+loadout and ground/naval unit has a TV (content data; formula in §13.2 R2). Calibration anchor: Kestrel with a standard A2A loadout = **10.0**. Reference values (design targets): Light fighter 10, Multirole 14, Heavy fighter 18, Interceptor 13, Strike 12, CAS 8, Bomber 20, EW 12, AWACS 15, Carrier aircraft 13, Stealth 24, Experimental 30; ground: MBT 4, IFV 2, short-range SAM 6, medium SAM 14, long-range SAM 22, EWR 8, AAA battery 5, command post 6; naval: patrol 8, frigate 20, destroyer 35, cruiser 55, carrier 120.

**Budgets.**

```text
B_player = Σ TV(player aircraft) + Σ TV(allied AI assigned)
B_enemy  = B_player · m_strength · m_difficulty · m_mission
m_strength ∈ {0.5, 0.8, 1.0, 1.5, 2.5};  m_difficulty ∈ {0.85, 1.0, 1.1, 1.2, 1.3} (Easy…Simulator);  m_mission = 1.0 default (type-specific overrides)
```

(Enemy numbers can instead be set explicitly in Sandbox, §30.) Enemy budget is split by **role shares** per mission type:

| Mission type | Fighters | Strike/bombers | SAM/AAA/radar | Armour/logistics | Naval | Support (AWACS/tanker/EW) | Reserve for reinforcements |
|---|---|---|---|---|---|---|---|
| Air Superiority | 70 % | 5 % | 5 % | 0 | 0 | 5 % | 15 % |
| Interception | 20 % (escorts) | 55 % | 5 % | 0 | 0 | 5 % | 15 % |
| Strike / Bombing | 20 % | 0 | 40 % | 25 % | 0 | 5 % | 10 % |
| SEAD / Suppression | 15 % | 0 | 65 % | 5 % | 0 | 5 % | 10 % |
| Fleet Battle / Carrier Defence | 25 % | 10 % | 5 % | 0 | 50 % | 5 % | 5 % |
| Convoy Attack | 15 % | 0 | 20 % | 55 % | 0–25 % | 0 | 10 % |
| Mixed Strike | 30 % | 5 % | 30 % | 20 % | 5 % | 5 % | 5 % |

Within a role, the filler chooses units by weighted random draw (faction roster, era, doctrine), subject to *minimum viable packages* (e.g., a SAM network needs ≥ 1 radar + ≥ 1 launcher + C2 if the budget allows) and entity caps. Friendly forces use the same machinery with budget `B_friendly = α · B_enemy` (α default 0.6 + 0.1 per human beyond 1, tuned per mission type).

**Par score** for the grade (§25.11) is the sum of expected rewards for nominal completion.

## 26.9 Validation and solvability

Static checks (fast, run on every generation):

| Check | Rule |
|---|---|
| Geometry | Spawns/objectives on valid surfaces; no overlap; clearance from terrain; units not on steep slopes |
| Reachability | Fuel feasibility: distance to every primary objective and back ≤ 0.85 × the player aircraft's combat range with the default loadout, or a tanker/airbase is available |
| Route exposure | At least one ingress corridor to each primary objective with SAM exposure < `X` % at the mission's planned altitudes, unless the mission is SEAD/Suppress |
| Start safety | No enemy within `R_min_sep` of any player at start; SAM umbrella does not cover the start position |
| Timeline | Reinforcement arrival times ≥ 90 s after the phase start; no more than `N_threat` simultaneous major threats near the player |
| Budgets and caps | TV within ± 10 % of target; entity counts within hardware caps |
| Data coverage | §8.8 |
| Objective solvability | Every objective has a satisfiable success condition under the rule set (graph analysis: no objective depends on an unreachable one) |
| Rule compliance | Equipment restrictions satisfied by the default loadouts and spawn positions |
| Text | All strings resolve to localization keys |

Repair strategies: re-place offending objects, reduce budget by one step, change weather, switch template variant, or fall back to air start. If all fail the generator returns a descriptive error (never an unwinnable mission).

**Fuzz validation (CI, not in-game):** 10 000 random requests are generated; for a sampled 1 000 the headless sim fast-forwards 180 s at 8× with simple AI to detect crashes, deadlocks, and missing spawns.

## 26.10 Performance

Target generation time on the Recommended profile (in a worker): Small ≤ 150 ms, Medium ≤ 400 ms, Large ≤ 800 ms, Huge ≤ 1.5 s, Extreme ≤ 3 s. Terrain analysis dominates; it uses coarse z≤10 samples with local refinement. Progress is reported to the UI; the main thread is never blocked.

## 26.11 Anti-repetition integration

Each generated mission computes a **DNA** vector (§69.2) and compares to the history ring (last 20 missions, stored locally); if too close to any recent mission (distance < 0.15) the generator re-rolls scenario/variant once. Twists add variance (§69.4).

## 26.12 Tools

**Generator Lab** (browser page): sliders for every parameter, a seed scanner (generate N seeds and show thumbnails/stats), a placement-score visualiser, DNA distribution plots, batch validation. Used by designers and CI.

## 26.13 Edge cases

| Case | Handling |
|---|---|
| Ocean-only battlefield with land-based mission | Template excludes land-only types; carrier/naval templates preferred; air start |
| No airports within area and `Airbase: Both` | Improvised airfields (§10.8) or fallback to air start with notice |
| Arctic/desert with sparse roads | Convoy templates excluded |
| Extremely small area + Overwhelming strength | Budget reduced to cap; message in setup UI |
| Seed provided without full config | Defaults + warning; deterministic |
| Different data pack | Warning; high-level config preserved (§26.3) |

## 26.14 Acceptance criteria

- `AC-26.1` Golden-hash determinism over 1 000 requests holds in WASM and native builds.
- `AC-26.2` ≥ 99.5 % of 10 000 random requests produce a validated mission within the time budgets of §26.10; the rest return an actionable error.
- `AC-26.3` Share link round-trip: encode → decode yields an identical request and mission hash.
- `AC-26.4` Budget accuracy: realised enemy TV within ±10 % of target for ≥ 95 % of runs.
- `AC-26.5` Anti-repetition (shared with AC-3.4): mean pairwise DNA distance ≥ 0.45 over 200 generations at a fixed place.
- `AC-26.6` No generated mission starts an enemy within `R_min_sep` of a player or an uncovered start position (verified over 10 000 runs).

---

# 27. MISSION TYPES

`P5 · M4` — The generator supports the sixteen mission types of the outline (§49). Each type is a **template family**: an objective structure, a phase pattern (§28), a force-role mix (§26.8), geography preferences, and win/lose logic. Every type can be tuned by the request parameters (§26.2) and combined with twists (§69.4).

**Common template fields.** `objectives` (primary/secondary/bonus), `phase_pattern`, `force_shares`, `geo_fit` (terrain/coast/road requirements), `spawn_style` (airbase, air start, carrier), `typical_duration`, `fail_extra`, `variants` (named alternatives), `player_roles` (suggested multiplayer roles, §32.4).

## 27.1 Quick Dogfight

- **Premise:** Simple air-to-air combat for fast action.
- **Objectives:** Destroy N hostile aircraft (N by size) or survive T minutes; bonus: no losses.
- **Phase pattern:** Contact → Climax (2 phases); reinforcements optional (Survival-lite).
- **Forces:** Fighters only (100 % role share), skill per request; no ground/naval.
- **Geography fit:** Any; mountain variants for terrain masking; ocean variants for clean arenas.
- **Spawn:** Air start, 25–40 km separation (WVR/short BVR), or runway-hot both sides (airport-to-airport).
- **Duration:** 3–8 min. **Fail:** player dead (with respawn rule), timer.
- **Variants:** *Duel* (1v1), *Gaggle* (4v6), *Ambush* (start defensive), *Dogfight over mountains*.

## 27.2 Interception

- **Premise:** Stop enemy aircraft (bombers/strike) before they reach a protected target.
- **Objectives:** Intercept group(s) before the "defended line" (a radius around the target); primary: destroy ≥ 70 % of bombers or turn them back; secondary: escort fighters killed.
- **Phase pattern:** Approach → Contact → Escalation (escorts reveal) → Crisis (leakers) → Resolution.
- **Forces:** Enemy strike package (bombers/strike 55 %, escort fighters 20 %); AWACS/GCI guidance for the player.
- **Geography:** Open approaches with the defended asset (city, base, fleet) inland; terrain for low-level approach variants.
- **Timing:** Package timing computed by `TOT` (§20.8); player scramble from alert (§48.3) or CAP.
- **Fail:** Target hit (damage threshold) or destroyed; player loss.
- **Variants:** *High-speed raid*, *Low-level penetrators*, *Decoy-heavy raid*, *Night intercept*.

## 27.3 Escort

- **Premise:** Protect friendly aircraft (bomber, transport, strike package, tanker).
- **Objectives:** Escortee reaches destination alive; secondary: kill threats; bonus: no escortee damage.
- **Phases:** Approach → Contact → Threat Expansion (SAM belt or fighter sweep) → Crisis → Resolution.
- **Forces:** Friendly escortee group (AI); enemy interceptors, SAM belt along route.
- **Geography:** Route across contested terrain; avoid straight lines (routes use the planner of §19.10).
- **Fail:** Escortee lost; mission timer.
- **Variants:** *Tanker track protection*, *Transport convoy over water*, *Damaged bomber escort home*.

## 27.4 Strike

- **Premise:** Attack ground targets (military installations, depots, bridges, armour).
- **Objectives:** Destroy designated targets (precision list) with a damage threshold; secondary: avoid collateral; bonus: all targets in one pass.
- **Phases:** Approach → Contact (early warning) → Escalation (fighters scramble) → Climax (target area defended) → Resolution (egress).
- **Forces:** Ground targets and defences (AAA, short/medium SAM), fighters in response; budgets weighted to SAM/AAA/ground (§26.8).
- **Geography:** Target sites near features that allow masking routes; urban collateral constraints.
- **Fail:** Targets survive at timeout; player lost; ROE violation.
- **Variants:** *Dawn raid*, *Pop-up on convoy*, *Bridge cut*, *Fuel depot*.

## 27.5 Air Superiority

- **Premise:** Destroy hostile aircraft and hold the battlefield.
- **Objectives:** Destroy ≥ X % of enemy air forces or achieve the kill ratio; secondary: protect AWACS/base; bonus: boss kill.
- **Phases:** full 8-phase pattern commonly used (Approach → Contact → Escalation → Threat Expansion → Crisis → Elite Response → Climax → Resolution).
- **Forces:** Mixed fighter packages in waves; AWACS and tanker support both sides; optional SAM.
- **Geography:** Any; large battlefields enable BVR.
- **Fail:** Friendly losses > threshold; home base lost; timer.
- **Variants:** *Sweep*, *Defensive counter-air*, *Sector hold*.

## 27.6 Suppression (Suppression of enemy air defences — area denial)

- **Premise:** Destroy or neutralise air defences broadly (not just radars) to open an air corridor.
- **Objectives:** Reduce SAM/AAA coverage below a threshold (coverage computed from horizon profiles); destroy key command nodes; secondary: ARM usage restrictions; bonus: zero losses to SAM.
- **Phases:** Approach → Contact → Threat Expansion (SAM activation) → Crisis (package under fire) → Climax → Resolution.
- **Forces:** Integrated defence network (§21.5) with decoys; supported by friendly strike package.
- **Fail:** Package losses > X; timeline overrun.
- **Variants:** *Corridor opening*, *Island air defence suppression*.

## 27.7 Carrier Defense

- **Premise:** Protect naval forces from air and surface attack.
- **Objectives:** Carrier (and escorts) survives for T; secondary: destroy attackers before missile launch lines; bonus: no escort losses.
- **Phases:** Approach → Contact (AWACS report) → Escalation (multi-axis raid) → Threat Expansion (anti-ship missiles) → Crisis (carrier hit) → Climax → Resolution.
- **Forces:** Friendly task group (§22.8), enemy strike aircraft with anti-ship missiles and SEAD, surface/submarine threat (staged).
- **Geography:** Open ocean (≥ 60 % water), islands as flanks.
- **Spawn:** Carrier deck (cat launch/recovery), CAP station.
- **Fail:** Carrier sunk or flight deck disabled for > T; timer.
- **Variants:** *Night raid*, *Storm defence*, *Reduced radar (EW)*.

## 27.8 Fleet Battle

- **Premise:** Large-scale naval/air combat between fleets.
- **Objectives:** Destroy/neutralise the enemy flagship or ≥ X % of fleet TV; defend own capital ships.
- **Phases:** Approach → Contact → Escalation → Climax (missile exchanges) → Resolution.
- **Forces:** Two task groups (naval 50 % share), air wings on both sides, land-based assets if islands.
- **Geography:** Open ocean; straits for variants.
- **Fail:** Flagship loss; timer.
- **Variants:** *Raid on port*, *Blockade run*.

## 27.9 Convoy Attack

- **Premise:** Attack a moving convoy (ground or sea).
- **Objectives:** Destroy ≥ X % of cargo vehicles/ships before arrival; secondary: kill escorts; bonus: bridge denial.
- **Phases:** Approach → Contact → Escalation (SAM/AAA escorts, fighter cover) → Climax → Resolution.
- **Forces:** Logistics vehicles, escort armour/AAA/SAM; or merchant ships with frigate escorts.
- **Geography:** Requires a road/sea route with chokepoints; bridges/rail crossing valuable.
- **Fail:** Convoy arrives; player lost.
- **Variants:** *Fuel convoy*, *Rail interdiction*, *Night convoy*.

## 27.10 Reconnaissance

- **Premise:** Reach a target area, gather information, survive extraction.
- **Objectives:** Identify/photograph targets (hold sensor/visual contact for N s within range), return; secondary: no detection by radar (EMCON/low-level); bonus: reveal hidden SAMs (campaign intel).
- **Phases:** Approach (stealth) → Contact (spotted?) → Crisis (pursuit) → Resolution (extraction).
- **Forces:** Static or moving assets of interest; reactive defences and QRA fighters.
- **Geography:** Any with strategic features.
- **Fail:** Aircraft lost before data delivered; timer.
- **Variants:** *Photo run*, *ELINT sweep (EW)*, *Damage assessment*.

## 27.11 Bombing

- **Premise:** Destroy designated infrastructure (dams, bridges, factories, power).
- **Objectives:** Destroy infrastructure items; effect-based success (e.g. "power grid loss ≥ 60 %").
- **Phases:** Approach → Contact → Escalation → Climax (run-in) → Resolution.
- **Forces:** Defences moderate; reaction fighters; target hardened vs penetrators.
- **Geography:** Industrial areas, rivers (dams), valleys.
- **Fail:** Targets survive; player lost.
- **Variants:** *Dam buster low-level*, *Penetrator on hardened shelter*, *Rail yard*.

## 27.12 SEAD

- **Premise:** Destroy radar and SAM systems (focused, often as a package role).
- **Objectives:** Destroy N radars/launchers or degrade the network below threshold; secondary: preserve strike package; bonus: ARM efficiency.
- **Phases:** Approach → Contact (emitters detected) → Threat Expansion (SAM launches) → Climax → Resolution.
- **Forces:** Networked defence with ambush/shoot-and-scoot doctrines.
- **Geography:** Hills for SAM sites, valleys for approach.
- **Fail:** SAM kills > X friendly aircraft; timer.
- **Variants:** *EW-supported*, *Decoy-baited*.

## 27.13 Mixed Strike

- **Premise:** Simultaneous air-to-air and air-to-ground combat.
- **Objectives:** Primary ground target(s) plus secondary air interception and optional naval.
- **Phases:** all phases; Escalation delivers air threats during the strike window.
- **Forces:** Balanced (fighters 30 %, SAM 30 %, ground 20 %, naval 5 %).
- **Fail:** Primary lost; heavy losses.
- **Variants:** *Counter-attack*, *Airhead assault*.

## 27.14 Boss Hunt

- **Premise:** Destroy an elite enemy (§29).
- **Objectives:** Neutralise the boss and/or break its squadron; secondary: preserve wingmen.
- **Phases:** Approach → Contact → Escalation → Elite Response (boss engages) → Climax → Resolution.
- **Forces:** Boss squadron + supporting fighters; AWACS tracks boss intermittently.
- **Geography:** Terrain/cloud-rich for ambush behaviours.
- **Fail:** Player loss without kills; timer.
- **Variants:** *Ace duel*, *Ace with escorts*, *Revenge* (campaign).

## 27.15 Survival

- **Premise:** Survive escalating waves.
- **Objectives:** Survive T minutes or N waves; bonus: kills; score for time alive.
- **Phases:** repeated Contact → Escalation cycles with increasing intensity until Climax.
- **Forces:** Reinforcement director (§28.5) controlling waves; resource scarcity (fuel/ammo) optional with rearm points.
- **Geography:** Defended airbase or open area.
- **Fail:** Player dead without respawns; base lost.
- **Variants:** *Last stand*, *Endless*.

## 27.16 Dynamic Frontline

- **Premise:** Fight over strategically important locations where control shifts during the mission.
- **Objectives:** Hold/seize control points (airfields, ridges, crossings); control measured by a frontline value that moves with kills and presence.
- **Phases:** Rolling; the frontline controller creates tasks (defend/attack) as control changes.
- **Forces:** Ground armies (abstract fights), air wings, SAM networks; reinforcements pulled by control.
- **Fail:** Loss of key points; timer.
- **Variants:** *Campaign frontline operation* (derived from the strategic map, §35).

## 27.17 Template summary

| Type | Typical duration | Primary skill tested | Default roles share (see §26.8) |
|---|---|---|---|
| Quick Dogfight | 3–8 min | BFM | Fighters |
| Interception | 8–20 | BVR, geometry | Strike + fighters |
| Escort | 10–25 | Awareness, positioning | Fighters + SAM |
| Strike | 10–25 | Navigation, masking, delivery | SAM/ground |
| Air Superiority | 15–35 | Overall air combat | Fighters |
| Suppression | 12–30 | Network understanding | SAM/AAA |
| Carrier Defense | 12–30 | Defensive CAP, missile defence | Naval + strike |
| Fleet Battle | 20–40 | Anti-ship, EW | Naval |
| Convoy Attack | 10–25 | Low-level, target selection | Ground/logistics |
| Reconnaissance | 10–25 | Stealth, sensors | Defensive reactive |
| Bombing | 10–25 | Precision, penetrators | Static defences |
| SEAD | 12–25 | ARM, EW | SAM |
| Mixed Strike | 20–40 | Multi-tasking | Balanced |
| Boss Hunt | 8–20 | BFM + tactics vs ace | Elite squadron |
| Survival | 10–30 | Endurance | Waves |
| Dynamic Frontline | 20–45 | Strategic awareness | Mixed |

## 27.18 Acceptance criteria

- `AC-27.1` Each of the 16 types is generated at least 100 times on 10 distinct geographies without validation failure (types excluded by geography fit are explicitly reported).
- `AC-27.2` Each type's success and failure paths are exercised by scripted AI runs (success reachable, failure reachable).
- `AC-27.3` Type-specific forces match the §26.8 share table within ±10 %.
- `AC-27.4` Duration distribution of each type has median within the ranges of §27.17 over 1 000 AI-assisted runs.

---

# 28. DYNAMIC MISSION ESCALATION

`P5 · M4` — Missions escalate dynamically (outline §50–52). The mission plays like a story with rising and falling tension, using the same simulation as any other mission: reinforcement waves, objective changes, and event cues driven by a director.

## 28.1 Intent

- Avoid static checklists; make the player respond to developing situations.
- Provide cinematic peaks (formation entries, missile barrages, aces from cloud) through systemic choreography (outline §137), not heavy scripting.
- Stay fair: threats arrive with warning; the player is never ambushed with no counter.

## 28.2 Phase pattern (outline §50)

| # | Phase | Typical content | Exit conditions |
|---|---|---|---|
| 1 | **Approach** | Minimal threats; navigation, climb, rendezvous | Contact detected or location reached |
| 2 | **Contact** | Initial enemies detected (CAP, patrol) | First kill or distance < X |
| 3 | **Escalation** | Additional fighters arrive | Primary objective progress or time |
| 4 | **Threat Expansion** | Bombers, SAMs, or strike aircraft appear | Threat addressed or objective turn |
| 5 | **Crisis** | A friendly objective becomes threatened | Crisis resolved or failed |
| 6 | **Elite Response** | Enemy ace squadron/boss arrives | Boss neutralised or retreats |
| 7 | **Climax** | Major combat | Primary objective decided |
| 8 | **Resolution** | Victory, defeat, or extraction | End |

Not every mission uses every phase; patterns are templates with optional/branching phases. The `Phase` data (§25.6) can carry an *intensity target* and a *budget slice*.

## 28.3 Intensity director

The director shapes pacing with a **target intensity curve** `I*(t)` and a **measured pressure** `P(t)`.

```text
P(t) = Σ_threats w_threat(type) · f_proximity(range) · f_engaged
     + 0.5 · missiles_active_on_player + 0.2 · (1 − damage_capability) + 0.15 · (low fuel/ammo)         (normalised 0..1)
I*(t): from the phase's template (ramp, plateau, double-peak, sawtooth, flat)
control law:  if P < I* − m_low:  schedule next beat earlier (spawn/trigger within constraints)
              if P > I* + m_high: delay next beat; allow lull (reduce new threats)
margins m_low = 0.15, m_high = 0.25
```

Beats are **pre-planned** (the generator defines the candidate beats and their budget costs); the director adjusts their *timing* within windows rather than inventing content. Defaults: `adaptive_pacing = true` for Sandbox/Campaign, off for 1v1 and Simulator-locked rule sets. Dynamic difficulty adjustment (scaling enemy counts) is **off by default**; if enabled, enemy beat budgets scale within ±25 % based on a smoothed player performance score.

## 28.4 Event types (outline §51)

| Event | Description | Typical trigger |
|---|---|---|
| **Enemy reinforcements** | New enemy groups from airbases or boundary | Time window, phase entry, objective progress |
| **Friendly reinforcements** | Allies arrive (relief, CAP) | Crisis, low friendly count |
| **SAM activation** | Dormant/ambush SAMs begin emitting | Player enters zone, ARM launch, strike approach |
| **Bomber arrival** | Strike package inbound to a protected asset | Phase timing, objective chain |
| **Carrier movement** | Fleet repositions/launches | Time/proximity |
| **Objective changes** | New/changed objectives | On completion/failure of others |
| **Friendly losses** | Scripted/systemic loss (e.g. wingman downed) for narrative | Combat outcomes |
| **Enemy ace appearance** | Boss/elite squadron enters (§29) | Phase 6 or intensity peak |
| **Weather changes** | Front arrives, visibility drops | Timeline |
| **Emergency defence missions** | Sudden attack on a friendly base/asset (switch to Defend) | Crisis |
| **Equipment failures / twists** | Defector, decoys, low fuel (§69.4) | Twist table |

Each event template defines: `preconditions`, `selection weights` by phase, `placement rule` (entry corridor/airbase), `composition` (budget share), `announcement` (radio/AWACS), `objective effects`, and `cooldown`.

## 28.5 Reinforcement director

The reinforcement director owns *spawning discipline*:

**Rules.**

1. **Never spawn in view.** No spawn within 40 km of any human-controlled aircraft *unless* it is a ground start at an airbase (scramble, §48.3) or arrives through a carrier hangar; air entries spawn at the boundary or at airbase locations and then fly in physically.
2. **Warning first.** Major waves are announced by AWACS/GCI or radio at least 20 s before they reach player sensor range (if the player has a controller/datalink); otherwise, sensors' natural detection applies.
3. **Rate limits.** Waves per hour (`rate_per_hour`, sandbox "reinforcement rate"), minimum gap between waves 45 s, maximum simultaneous hostile TV near the player `TV_max = 3.0 · B_player/N_players` unless the type is Survival/Chaos.
4. **Budget draw.** Each wave consumes budget from the phase's slice (§26.8); when the budget is exhausted no more waves except twist events.
5. **Entity caps.** If a spawn would exceed hardware caps (§0.6.2), apply in order: (a) despawn civil ambient simulation (cheap), (b) compress far AI to Tier 3, (c) defer the wave up to 120 s, (d) merge with the next wave into a single larger group (preserving TV), (e) reduce group size by ≤ 25 % (message in log).
6. **Entry geometry.** Waves enter along a bearing consistent with the story (enemy home side), at altitudes appropriate to the type, and use cloud banks when available as cinematic entry points (§28.8).
7. **Airbase scramble.** When enemy airbases are in range, reinforcement aircraft actually take off (taxi, take-off) using the scramble system (§48.3); distant bases use the simplified version.

**Queue structure:** `ReinforcementQueue` entries `{ time_window, group, entry, budget, priority, announce }`, processed at 1 Hz.

## 28.6 Objective changes and chaining (outline §52)

Example chain as data:

```json
{ "objectives": [
  { "id": "o1", "text_key": "obj.destroy_radar", "kind": "destroy", "state": "active", "on_complete": [ { "do": "activate_objective", "id": "o2" }, { "do": "radio", "key": "radio.strike_package_detected" }, { "do": "spawn_group", "id": "g_strike" } ] },
  { "id": "o2", "text_key": "obj.intercept_strike", "kind": "intercept", "state": "hidden", "on_complete": [ { "do": "activate_objective", "id": "o3" }, { "do": "spawn_group", "id": "g_elite" }, { "do": "radio", "key": "radio.elite_squadron_detected" } ] },
  { "id": "o3", "text_key": "obj.destroy_command_aircraft", "kind": "neutralise", "state": "hidden", "target": { "group": "g_elite", "role": "leader" } } ] }
```

Reactive injections: the director can add objectives in response to emergent events (e.g., player destroyed the radar early → the strike package launches earlier; player ignores a crisis → objective failure triggers a "recover" objective).

## 28.7 Crisis design

A **crisis** makes a friendly objective threatened: an enemy package heads to a defended asset; an airfield is hit; a convoy is ambushed. Rules: the crisis has a visible clock (ETA of the threat), at least two valid responses (intercept with player, assign AI/allies, or accept loss with partial penalty), and AWACS guidance. Crises are generated from the `crisis_catalog` in the mission template with parameters tuned to the player's position and fuel.

## 28.8 Elite response and cinematic cues

- **Elite response** (phase 6): the boss/ace squadron arrives (§29). Cues: AWACS "unknown fast mover", RWR signature tone, special radio chatter; arrival geometry uses terrain/clouds to create dramatic emergence (spawn inside a cloud bank where the gameplay cloud density `ρ_c` exceeds threshold along the entry vector; otherwise from the boundary).
- **Cinematic systemic cues** (outline §137): large formations entering the battlespace (waves announced with the camera-free "radio swell" and music stem), missile trails crossing the sky (natural from the sim), bombers passing overhead (route over player base/airfield), SAM barrages (ambush SAMs triggered simultaneously by a trigger group), carrier launches (deck cycle at mission start), valley engagements (ambush points in mountain terrain), large dogfights (swarm waves). The director avoids stacking cues closer than 60 s unless the type is Chaos/Survival.

## 28.9 Resolution and extraction

After the climax objective resolves, the director runs *wind-down*: remaining hostiles disengage or retreat per morale (§19.7.3), the player receives RTB/extraction instructions, and `Return` objectives activate. Landing is optional in sandbox ("end mission now" is always available once success is locked in).

## 28.10 Fairness and safety rails

| Rail | Rule |
|---|---|
| No hidden death | A missile launched at the player always has a launch cue (RWR/MAWS) unless the player is in a rule set that removes it (Simulator) |
| Minimum separation | Reinforcement spawn ≥ 40 km from players (air entries) |
| No over-budget spikes | `TV_max` near the player (above) |
| Lull guarantee | After a major beat (TV > 40 % of the budget) at least 30 s of "calm" unless Chaos/Survival |
| Resource sanity | Missions avoid starving the player of weapons: if ammo < 10 % and enemies > 0, an objective/point for rearm (tanker/base) is surfaced |
| Out-of-fuel recovery | If fuel is critical and a tanker/base is in range, the AWACS suggests a vector |

## 28.11 Acceptance criteria

- `AC-28.1` The intensity director keeps measured pressure within ±0.25 of the target curve at least 80 % of mission time on 200 AI-flown missions.
- `AC-28.2` No reinforcement spawns within 40 km of a human-controlled aircraft in 10 000 runs (excluding airbase scrambles).
- `AC-28.3` Entity-cap degradation order (§28.5) is applied in the documented order in an artificial overload test.
- `AC-28.4` The example objective chain of §28.6 reproduces the outline §52 sequence exactly.
- `AC-28.5` In playtests, ≥ 75 % of players report "the mission changed in interesting ways" (survey item).

---

# 29. BOSS & ACE SYSTEM

`P5 · M4 (single-session bosses) → M6 (persistent identities)` — Bosses and aces are **dangerous because they are good, not because they are big** (outline §57, NN-4). There are no inflated health pools or damage multipliers.

## 29.1 Intent

- A boss encounter is a memorable duel/problem: a distinct aircraft, better equipment, exceptional tactics, capable wingmen, a unique identity.
- The player can beat any boss by outflying, out-thinking, or out-preparing them; the fight is decided by energy, sensors, weapons and teamwork.
- In the campaign, bosses are persistent characters (§29.5).

## 29.2 Sources of danger (the capability sheet)

A boss is composed from capability axes (never HP):

| Axis | Examples |
|---|---|
| **Superior aircraft** | Higher-TV airframes (Wraith, Chimera), better thrust-to-weight, thrust vectoring, low RCS |
| **Advanced weapons** | Longer-range or more agile missiles (Pike-9, Dart-4), better ECCM, more missiles |
| **Exceptional AI** | Ace skill row (§19.6), extra repertoire (baiting, feints, out-of-plane defence, better CM timing) |
| **Better tactics** | Coordinated four-ship, terrain-mask ambushes, sandwich attacks, coordinated jamming |
| **Experienced wingmen** | Elite-skill wingmen with high teamwork |
| **Unique equipment** | Towed decoy + EW pod, jammer escort, IRST-only (silent) approach |
| **Unusual behaviour** | Cloud-edge ambush, mountain shadow, decoy-drone gambit, hit-and-run energy fights |

### 29.2.1 Signature behaviour modules

Boss definitions list one to three **signature behaviours** (behaviour-tree modules integrated into the utility AI):

| Behaviour | Description |
|---|---|
| `cloud_ambush` | Hides in cloud/low sun, emerges to launch; uses `ρ_c` to find cover |
| `terrain_shadow` | Approaches via valleys to appear inside MRM no-escape range |
| `decoy_gambit` | Launches decoy drones/active decoys to bleed the player's missiles |
| `energy_sniper` | Refuses turning fights; extends and re-attacks with energy advantage |
| `mirror` | Adapts weights based on the player's observed habits (below) |
| `jammer_escort` | Wingman EW aircraft provides jamming cover (§17) |
| `bait_switch` | One aircraft presents a target while the other flanks |

**Adaptation (`mirror`).** The boss AI maintains three per-mission counters: *BVR preference* (fraction of player shots beyond 20 km), *merge preference* (time spent below 3 km range), and *evasion style* (notch vs break vs dive). It shifts selected utility weights by up to ±20 % within the mission (bounded, never beyond skill limits) — an observable "this ace studies me" effect that is visible in the debrief (“Vulture adjusted to your long-range tactics”). It does not change physics or perception.

## 29.3 Boss definition (data)

```json
{
  "schema": "boss/1", "id": "boss_vulture_lead", "callsign": "VULTURE 1-1", "name_key": "boss.vulture.name",
  "aircraft": "swift", "livery": "dark_wedge_scarred", "loadout": "swift/lr_hunter",
  "skill": "ace", "skill_overrides": { "energy_awareness": 1.0, "defensive_reaction_delay_s": 0.3 },
  "doctrine": { "aggression": 0.8, "preferred_engagement_range_km": 80, "missile_doctrine": "shoot_look_shoot", "hoard_lrm": false },
  "signature": [ "terrain_shadow", "mirror" ],
  "wingmen": { "count": 3, "skill": "elite", "aircraft": "swift" },
  "retreat": { "morale_threshold": 0.20, "fanatical": false },
  "radio": { "voice": "voice_f", "intro_set": "vulture_intro", "taunt_set": "vulture_taunt", "damaged_set": "vulture_damaged", "death_set": "vulture_death" },
  "intro": { "style": "radar_ghost", "warning_key": "awacs.unknown_fast_mover" },
  "music_stem": "boss_vulture",
  "dossier": { "known_levels": ["unknown", "named", "type", "doctrine"], "faction": "meridian_pact" },
  "persistent": true
}
```

## 29.4 Encounter design

- **Probability:** the request's *Boss Probability* `p` (0–1): at S5 the generator rolls `u < p` using the `"boss"` stream. In *Boss Hunt* missions `p = 1`.
- **Selection:** from the boss pool filtered by faction, era, location character (a mountain ambusher needs mountains), and what the player has already met (campaign).
- **Telegraphing:** an AWACS/radio cue, a distinctive RWR signature (a unique emitter ID shown as a named threat once identified), and a visible entrance (§28.8). Where possible the boss is first *detected* before it attacks.
- **Fairness:** same physics, same sensors, same ammunition limits (NN-4/NN-5); the boss obeys fuel, bingo, morale and retreat rules (§19.7), so it can retreat and return another day.
- **Defeat definition:** boss destroyed, disabled (cannot fly or fight), or forced to retreat with morale broken (counts as "neutralised" for objectives; in campaign a retreating boss survives with probability and returns, §29.5).

## 29.5 Dossiers and persistent identity (campaign)

Each boss/ace has a **dossier**:

```json
{ "id": "boss_vulture_lead", "encounters": 3, "kills_on_player_side": 2, "losses_to_player": 0, "last_seen": { "lat": 36.2, "lon": -121.8, "utc": "…" },
  "known_level": "type", "known_facts": ["flies_swift", "uses_terrain_masking"], "scars": ["tail_damaged"] }
```

Knowledge progresses with encounters and intelligence (§42.3): *Unknown Squadron* → *Squadron Vulture identified* → *Vulture operates long-range interceptors* (outline §79). Survival: a boss shot down may eject (probability by terrain and altitude) and return after `N` operations with a replacement aircraft; damage leaves **scars** (visible livery changes). The campaign can promote aces from regular squadrons based on their kill counts (a living enemy roster, §37.5).

## 29.6 Identity: paint, name, voice, music

Unique livery (palette + emblem + scars), callsign, radio voice persona and dialogue sets (intro, taunt, damaged, retreat, kill, death), HUD nameplate once identified, and an adaptive music stem that fades in when the boss is within 30 km and engaging. All content is data (§62).

## 29.7 Multiplayer scaling

Bosses scale with `N` human players by **adding capability, not HP**: more wingmen (count = 1 + ⌊N/2⌋ up to 5), a larger support package, or an additional signature behaviour; total TV scales `× (1 + 0.35 (N − 1))`.

## 29.8 Tuning targets

| Metric | Target |
|---|---|
| Median engagement time (boss first engagement to neutralisation) | 2–5 min for competent players at Normal |
| Player kill share vs boss (Normal, Veteran-equivalent player) | 40–60 % wins on first encounter |
| Boss shots at long range | Fired at ≥ 0.6 R_max, using DLZ (not magical) |
| Variation | No two consecutive boss encounters use the same signature module combination in a campaign (§69) |

## 29.9 Edge cases

| Case | Handling |
|---|---|
| Boss killed by friendly AI | Credited to the squadron; achievement logic uses `assist` rules |
| Player left the area | Boss may chase until boundary Stage 2, then disengages |
| Multiple bosses | Max 2 concurrent; others queued |
| Boss spawn blocked by caps | Priority over ordinary waves; others dropped first |
| Boss with no valid weapons due to fuel/ammo | Retreats (RTB) |

## 29.10 Acceptance criteria

- `AC-29.1` Audit: no boss data file contains HP/damage multipliers; boss aircraft use the same damage tables (lint).
- `AC-29.2` AI-vs-AI test: a boss squadron beats a same-TV non-boss squadron 60–75 % of the time (capability, not stats), but loses > 25 % — i.e., defeats are plausible.
- `AC-29.3` Signature behaviours trigger in at least 80 % of encounters in suitable geography (`cloud_ambush` requires clouds, `terrain_shadow` requires mountains).
- `AC-29.4` Mirror adaptation changes at least one utility weight by ≥ 5 % after 2 minutes in ≥ 70 % of runs and never beyond ±20 %.
- `AC-29.5` Dossier progression persists across sessions (round-trip save/load).

---

# 30. SANDBOX CONFIGURATION

`P2/P5 · M2→M4` — Players configure almost every aspect of their battles (outline §58, §3.6) while the setup remains fast (NN-10: at most three clicks from the map to launch with defaults).

## 30.1 Intent

- **Quick by default, deep on demand.** A single "Launch" uses sane defaults; every dial is reachable in an *Advanced* panel.
- **Everything is a request parameter** to the generator (§26.2) or a runtime rule (§25.9, §59), never a hidden hard-code.
- **Shareable.** The configuration serialises into the `SessionConfig` (§6.3) and into share links (§26.3).

## 30.2 Parameter catalogue

| Parameter (outline §58) | Values | Default | Where it acts |
|---|---|---|---|
| Enemy count | *Auto from strength* or explicit 0–200 aircraft | Auto | Generator S3 (override budget) |
| Enemy strength | Light … Overwhelming | Even (1.0) | Budget multiplier (§26.8) |
| Friendly count | *Auto* or explicit 0–100 | Auto | Generator S3 |
| AI skill | Recruit…Ace, or *Mixed* distribution | Veteran (Normal) | AI parameters (§19.6) |
| Fuel | Unlimited / Realistic / Limited (×0.5) | Realistic | Loadout & AI bingo logic |
| Ammunition | Unlimited / Realistic / Limited | Realistic | Weapons stations |
| Damage (taken by player / dealt to enemy) | 0 % … 400 % / 50 %…200 % | 100 % | Damage model (§18.10) |
| Ground forces | Off / Low / Medium / High | Medium | Generator S3/S4 |
| Naval forces | Off / Low / Medium / High | Off unless ocean ≥ 40 % | Generator S3/S4 |
| Civilian traffic | Off / Low / Medium / High | Medium | Ambient (§23) |
| Weather | Preset or Realistic/Random | Clear/Cloudy | §24 |
| Time | Preset/exact + date | Afternoon, today | §24.5 |
| Wind | Direction, speed, gust, turbulence | From weather | §11.4 |
| Missile availability | Full / No long-range / No radar-guided / WVR only / Guns only / Custom list | Full | Loadout filter and AI doctrine |
| Radar realism | Simplified / Standard / Full | Standard | Avionics tier (§16.10) |
| Battlefield size | Small…Extreme or custom | Large | §8 |
| Respawns | None / Limited (n) / Unlimited; delay; location | Limited (2) | Rule set (§25.9) |
| Mission timer | Off / 5…180 min | Off (type default) | Rule set |
| Boss probability | 0–100 % | 20 % | Generator S5 (§29.4) |
| Reinforcement rate | Waves/hour 0–12 or *Auto* | Auto | Director (§28.5) |
| Mission type | Auto or any of §27 | Auto | Generator S1 |
| Spawn | Runway hot / runway cold / parking cold / air start / carrier | Runway hot | §48.2 |
| Time acceleration | 1× … 16× (single-player) | 1× | §49 |
| Friendly fire | On / Off | Off | Rule set |
| Civilians vulnerable | On / Off | Off | Rule set (§23.1) |
| Unlimited countermeasures | On / Off | Off | CM system |
| Invulnerable | On / Off (flag recorded, disables stats) | Off | Damage model |
| Assist level / dials | See §59.2 | Standard | FCS and HUD |
| Era | G3…G5, Experimental, Mixed | G4 | Roster, sensors (§13.6) |
| Factions | Pair or Auto | Auto | §37 |
| Seed | Auto or manual | Auto | §26.3 |

Parameters have **validators** that explain conflicts instead of silently resetting (e.g. "Naval Forces: High requires an ocean battlefield; reduce to Off or choose a coastal location").

## 30.3 UI organisation

- **Setup panel** (DOM): left column categories (Forces, Environment, Rules, Player, Advanced), right column preview (map footprint, estimated enemy TV, data download size, estimated mission length), bottom bar `Launch`, `Randomize`, `Save preset`, `Copy link`.
- **Presets:** *Skirmish* (default), *Realistic*, *Hardcore* (Simulator dials), *Training*, *Chaos*, *Custom*. Presets are data (`sandbox_preset/1`).
- Last-used settings persist per mode (IndexedDB, §64).
- **Budget meter:** a bar shows entity budget usage versus the hardware profile caps (green/yellow/red); selecting beyond the cap shows the scaling that will be applied.

## 30.4 Chaos Mode (outline §59)

An optional mode for deliberately extreme scenarios; entertainment over realism.

| Preset | Description |
|---|---|
| **1 vs 100** | One player vs up to 100 fighters (staggered over 6 minutes) |
| **Missile Storm** | Elevated missile availability; AI uses salvo doctrine; both sides saturate |
| **Carrier Armageddon** | Two fleets plus air wings; carrier launches continuously |
| **Everything War** | Mixed air/ground/naval war with huge reinforcement waves |

Rules: Chaos limits are the *High* hardware caps up to 200 aircraft (§0.6.2); the **Performance Governor** (below) prevents slideshow failure; assistance caps relax (unlimited CM optional, enlarged HUD cues), but there are no HP multipliers on enemies and no cheating AI (NN-4, NN-5). Chaos scores are stored separately from standard stats.

### Performance Governor

A closed-loop controller watches frame time and sim time (3-s moving average) and raises a *load level* `L ∈ {0..4}`:

| L | Triggers | Actions |
|---|---|---|
| 0 | Normal | none |
| 1 | frame > 20 ms or sim tick > 6 ms | Reduce VFX (smoke/explosion particles), shadow cascades −1, distant AI to Tier 3 sooner (radius ×0.8) |
| 2 | frame > 24 ms or sim tick > 8 ms | AI decision rate 5 Hz → 3 Hz for T2+, civil ambient off, projectile tracer thinning (1-in-3) |
| 3 | frame > 30 ms | Cloud/ocean quality down, defer reinforcement waves ≤ 60 s, cap gun projectiles to 60 % |
| 4 | sustained > 40 ms | Merge/despawn low-priority AI; message "Performance governor active" |

Recovery steps down one level after 10 s of headroom (hysteresis). The governor is also active outside Chaos (with milder triggers) so the game degrades gracefully on low-end machines.

## 30.5 Interactions and constraints

- **Unlimited fuel** disables AI bingo logic for players' wingmen only (enemies follow their own rules unless "all unlimited").
- **Radar realism: Simplified** implements full-information radar (no notch) and auto-lock; Standard enables notch/RWS; Full adds manual radar control, TWS management, jamming interplay.
- **Missile availability** filters both the loadout editor and AI loadout generation consistently.
- **Time acceleration** is disabled when hostile tracks are within the threshold radius (§49.3).

## 30.6 Persistence and replication

`SessionConfig` + `dials` serialise to JSON; presets stored locally (IndexedDB); share links carry the config (compressed). In multiplayer the host's configuration is authoritative and replicated at join; some dials are *client-local* (HUD, accessibility) and not enforced; `dials_lock` enforces competitive fairness.

## 30.7 Edge cases

| Case | Handling |
|---|---|
| Conflicting parameters | Validator message; auto-fix suggestion |
| Parameter beyond hardware caps | Scales down proportionally with a visible note (§8.8) |
| Imported preset from newer version | Unknown keys ignored with a warning; defaults used |
| Chaos on low-end | Governor starts at L1 and warns up-front |

## 30.8 Acceptance criteria

- `AC-30.1` From the map, a new mission launches in ≤ 3 clicks/keypresses with defaults.
- `AC-30.2` Every parameter in §30.2 changes the generated mission or runtime behaviour in a verifiable way (automated parameter coverage test).
- `AC-30.3` Chaos "1 vs 100" runs on the Recommended profile with the governor holding ≥ 30 FPS and reaching ≥ 60 simultaneous enemy aircraft at the peak.
- `AC-30.4` Presets round-trip through export/import without loss.

---

# 31. 1V1 MODE

`P8 · M5` — 1v1 is a focused competitive mode on real geographic arenas (outline §60–61). Fairness, clarity and fast rematches matter more than scale.

## 31.1 Intent

- Pure skill expression: two players, mirrored or parity-matched aircraft, clear rules, fast rounds.
- Geography still matters: terrain shapes the duel (mountains, deserts, oceans, snow, coast, urban).
- Hosting fairness: the architecture must not give the host an obvious advantage (§31.6).

## 31.2 Flow

```text
Lobby (2 players) → choose Arena (map) → choose Aircraft/Loadout (parity rules) → choose Weather/Time/Altitude → choose Rule Set
→ both Ready → Countdown (5 s) → Round → Result (replay offered) → Next round or Match result → Rematch / Change settings / Exit
```

## 31.3 Setup parameters (outline §60)

Players select: **location**, **aircraft**, **loadout**, **weather**, **time**, **altitude**, **rules**. The host proposes; the guest must accept (or counter-propose within the lobby). A "fair settings" banner shows parity status (aircraft/loadout/time/weather identical or point-balanced).

## 31.4 Rule sets (outline §60)

A 1v1 rule set composes five modifiers: **weapon set**, **start geometry**, **scoring**, **life policy**, **boundaries**. The outline's named rules are presets of these modifiers:

| Rule | Weapons | Start geometry | Scoring | Notes |
|---|---|---|---|---|
| **Unlimited** | Everything unrestricted, full loadout, unlimited CM | Neutral head-on 5 km | First to 5 kills; instant respawn at start positions after 5 s | Casual |
| **Realistic** | Real ammo/fuel limits, Hard dials locked | Neutral head-on 20 km, or airborne from opposite bases | One life per round; match = best of 3 | Realism dials locked at Hard (§59.3) |
| **Guns Only** | Guns only | Neutral head-on 2 km (same altitude, 250 m/s) | First kill | Pure BFM |
| **Limited Missiles** | 2 WVR + 2 MRM (configurable n) + guns | Neutral head-on 20 km | First kill / best of 3 | Missile discipline |
| **BVR** | MRM/LRM + datalink only (guns removed) | Head-on 80 km, 10 000 m, 300 m/s each | First kill; round ends on kill or crash | Sensors, notch, crank |
| **Dogfight** | Short-range weapons + guns | Neutral head-on 5 km | First kill / best of 3 | Close fight |
| **First Kill** | Any | Selected geometry | Round ends at first kill | Scoring modifier |
| **Best of Three** | Any | Selected geometry | Match = first to 2 round wins | Scoring modifier |

**Start geometries:** *neutral head-on* (symmetric about the midpoint, random side assignment by seeded coin flip), *offensive/defensive* (one 1.5 km behind and 20° off the other's tail, same altitude and speed; swapped in the next round for balance), *tail-chase*, *line-abreast* (parallel, 2 km), *scissors setup*, *BVR merge*. Altitude/speed defaults: 6 000 m MSL (terrain-clear), 250 m/s CAS-equivalent; adjustable.

## 31.5 Boundaries and adjudication

| Rule | Default | Behaviour |
|---|---|---|
| **Bubble** (arena radius) | Dogfight 25 km, Realistic 60 km, BVR 120 km | Stage 1–3 warnings per §8.5; expiry = round loss |
| **Hard deck** (min altitude AGL) | 500 m | More than 3 s below = round loss ("deck violation") |
| **Ceiling** | 15 000 m | Same enforcement |
| **Kill definition** | Destroyed **or disabled** (cannot sustain flight: loss of control + below hard deck, engines out and no glide to safe altitude, pilot incapacitated) | "Disabled" adjudicated automatically after 8 s of unrecoverable state |
| **Simultaneous kills** | Both destroyed within 1.0 s | Round is a draw; replay highlights |
| **Crash credit** | Opponent crashes within 10 s due to damage you caused | Credited to you; self-inflicted crash → opponent wins |
| **Collision** | Both destroyed: draw; one survives: survivor wins unless deliberate ram flagged (sustained closing at < 50 m for > 3 s) → ram offender forfeits | |
| **Time limit** | 5 min per round (Realistic 10 min) | Draw (or sudden death: next kill wins, max 2 min) |
| **Disconnect** | Forfeit after 20 s (can reconnect within 20 s) | |

## 31.6 Fairness

1. **Parity.** Default is *same aircraft, same loadout* (mirror). Optional *class parity*: aircraft within ±10 % TV with loadout point budgets (§26.8 TV tables) — shown in the lobby.
2. **Environment parity.** Same weather, time, wind and arena for both.
3. **Start fairness.** Symmetric geometry; sides swapped across rounds; seeded coin flip recorded in the replay.
4. **Network fairness (host advantage).** In a listen-server match the host has zero latency. Mitigations, in order of preference: (a) **dedicated server** (native or a third machine's browser tab) for competitive play; (b) **lag equalisation** (default ON for 1v1): the authority adds a fixed input delay to the host's own controls equal to half the measured RTT to the client (clamped 0–80 ms), so both pilots experience equal effective control delay; (c) server-side lag compensation for guns (§33.6); (d) ping/jitter display and a "fair play" warning if RTT > 100 ms or jitter > 30 ms.
5. **Information fairness.** Opponent state replicated only as sensors allow (§33.5, "fog-of-information replication" is forced ON for 1v1).
6. **No hidden advantages.** Both pilots have the same assist caps (rule set `max_assist`), same HUD tier, same countermeasures.

## 31.7 Spectating

Up to 8 spectators join as clients without aircraft: free camera, follow cam (chase/cockpit/missile), scoreboard, replay of the last round. Spectator streams use a 3-second delay option (to prevent coaching in tournaments). Spectators receive full true-state replication (no fog) for the broadcast camera.

## 31.8 Geographic arenas (outline §61)

Arenas are real places; the setup UI lists recommended arenas (curated bookmarks, §9.10) but any location works.

| Category | Example arena (curated bookmark) | Gameplay character |
|---|---|---|
| Mountains | Aosta Valley, Alps (45.7° N, 7.3° E) | Valley masking, terrain-hugging merges |
| Mountains (high) | Khumbu, Himalaya (27.9° N, 86.9° E) | High-density-altitude performance penalties; ridges |
| Sierra | Yosemite region, Sierra Nevada (37.8° N, 119.5° W) | Granite walls, canyons |
| Desert | Nevada basin (37.2° N, 115.8° W) | Long sightlines, little concealment |
| Dunes | Sahara erg (24° N, 10° E) | Low contrast terrain |
| Ocean | Mid-Pacific (30° N, 140° W) | Clean arena; no terrain |
| Snowfields | Greenland ice sheet (72° N, 40° W) | White-out contrast, high elevation |
| Coastlines | Norwegian fjords (61.0° N, 6.9° E) | Narrow valleys + sea |
| Urban | Tokyo Bay (35.6° N, 139.8° E) | Tall structures (hard deck raised to 800 m by default), dense lights at night |
| Plateau | Atacama (23.9° S, 68.0° W) | Dry, high altitude |

Arena validity checks: minimum terrain clearance vs the hard deck, data availability at z≥11 inside the bubble (download prompt), obstacle density (urban rule: min altitude).

## 31.9 UI

Pre-round: arena preview, rule summary, parity status, ready toggle, ping indicator. In-round HUD: round score, timer, bubble distance, deck warnings. Post-round: result (win/loss/draw with reason), key stats (shots fired/hit, missile launches/defeats, time in offensive position), replay button (instant, snapshot-based §52), rematch button. Local match history and optional local Elo per profile (LAN groups only; no online ranking).

## 31.10 Round state machine

```text
SETUP → COUNTDOWN(5 s, controls frozen, aircraft airborne at start altitude with frozen sim ticks until zero)
 → LIVE → { KILL | DRAW | FORFEIT | TIME } → ADJUDICATE (replay snapshot saved) → RESET (despawn, refuel, rearm, swap sides if needed) → COUNTDOWN | MATCH_END
```

The authority freezes the simulation for the countdown (both clients receive the first live snapshot simultaneously) and unfreezes at a shared server tick; the countdown is timed with the clock offset estimate (§33.3) so both players "go" at the same instant within ±20 ms.

## 31.11 Edge cases

| Case | Handling |
|---|---|
| One player's data not ready (tiles) | Countdown blocked; prompt shows who is loading |
| Aircraft failure at start (invalid loadout) | Lobby blocks ready |
| Weather change mid-round | Disabled in 1v1 |
| Third player joins | Becomes spectator |
| Network spike during live round | Authority continues; client sees "poor connection"; no pause (rule set may allow a single 20-s technical pause per player) |

## 31.12 Acceptance criteria

- `AC-31.1` Mirror-match symmetry: over 1 000 AI-vs-AI mirror rounds with side swapping, side win-rate in [48 %, 52 %].
- `AC-31.2` All rule presets in §31.4 are selectable and produce the specified start states (automated scenario tests).
- `AC-31.3` Lag equalisation reduces the measured control-to-response latency difference between host and client to ≤ 15 ms in a simulated 80 ms RTT test.
- `AC-31.4` Countdown release skew between clients ≤ 20 ms (measured at the simulated tick).
- `AC-31.5` Adjudication test matrix (simultaneous kills, crash credit, collision, ram, hard-deck, bubble, disconnect) matches §31.5 in 100 % of scripted cases.

---

# 32. LAN/VLAN CO-OP

`P7 · M5` — Several players fight together against AI over a shared battlefield (outline §62–64, §150). The result should be a **large cooperative air operation**, not several players independently fighting AI.

## 32.1 Intent

- Real combined-arms play: air superiority, strike, EW, escort, recon and support roles with genuine interdependence.
- Easy to host on a LAN, a VPN/virtual LAN, or by direct connection — without any online account or service.
- Works in a browser, honestly: the connection experience is designed around what browsers can and cannot do (§32.2).

## 32.2 How players connect (browser reality)

A web page cannot open raw UDP/TCP sockets or broadcast LAN discovery packets. The game therefore offers a **layered connection model** (transport details in §33.3):

| Method | Needs | Typical use | Notes |
|---|---|---|---|
| **A. Invite blob (serverless WebRTC)** | Nothing but a way to copy text/QR between players (chat, QR scan, messaging) | Fully offline LAN, VPN/virtual LAN, direct IP | The host creates a lobby and gets an *invite blob* (compressed WebRTC offer, ≈ 500–900 characters) shown as text and QR; each guest pastes/scans it and returns a *reply blob*; the host pastes/scans it. One round trip per guest; the lobby then runs peer-to-host over WebRTC DataChannels |
| **B. Lobby relay (signalling only)** | Internet access for signalling | Convenience when online | A tiny signalling relay (publisher-hosted or self-hosted) exchanges the blobs automatically using a short **room code** (e.g. `FALCON-4821`); game traffic remains peer-to-host; relay sees no gameplay data. **Same-network discovery:** guests behind the same public IP can list hosts that opted in |
| **C. Dedicated server address** | A native `gdd-server` running on a LAN/VPN machine | 8+ players, competitive play, always-on servers | Players enter `host:port` plus a short **certificate code** (hash) shown by the server; transports: WebTransport (self-signed with pinned hash) → WebSocket (`wss`) fallback. No internet needed after the app is cached |
| **D. Same device / split-screen** | — | Out of scope | — |

Notes for implementers:

- Browsers hide local IP addresses in ICE candidates behind mDNS names; peers on the same LAN resolve these, but virtual LANs/VPNs may not. If the user grants **microphone permission** (needed for voice chat anyway), browsers expose real local addresses in candidates, which also fixes most VPN cases [verify]. The connection UI explains this plainly ("Voice permission improves local connections").
- A public-origin page (HTTPS) connecting to private-network addresses may trigger the browser's **local network access** permission prompt [verify]; the UI pre-explains the prompt.
- STUN/TURN are optional settings (empty by default; NAT traversal across the internet is out of scope, but advanced users can supply their own).
- The *app shell* is served from the game's HTTPS origin (cached by the service worker for offline LAN use). A plain `http://192.168.x.x` page is **not** a secure context and cannot use WebGPU, so LAN helpers never serve the game page themselves.

## 32.3 Lobby and host settings (outline §63)

The lobby is a shared DOM screen replicated from the host.

| Host setting | Notes |
|---|---|
| Location, battlefield size | Map preview and download readiness for each member (§45.4) |
| Mission type, objectives mode | §27; "Auto" allowed |
| Enemy count / strength, AI skill | §30.2 |
| Aircraft restrictions | Allow list, era, categories, one-of-each caps |
| Weather, time | §24 |
| Friendly fire | On/off |
| Respawns | None / limited (shared pool or per-player) / unlimited |
| Difficulty and dials | With `dials_lock` for fairness (§59) |
| Victory conditions | Mission-defined, time limit, kill target, survival |
| Voice chat | Off / on (§32.9); push-to-talk required Y/N |
| Max players (≤ 16), spectators (≤ 8), password | Password via challenge-response (§33.11) |
| Late join | Allowed (spawns at base/air entry) / locked once started |
| Ping limit | Reject clients above X ms |

Lobby UI: players (name, role, aircraft, ready state, ping), chat, mission preview and briefing, **loadout validation** (§14.5) per member, host controls (kick, change settings, start/countdown), copyable invite blob and room code. A **ready check** precedes launch; members still downloading data are highlighted; the host can wait or start without them (they join late).

## 32.4 Co-op roles (outline §64)

Players choose a role; roles shape recommended aircraft, UI panels, and how the mission rewards contributions.

| Role | Recommended aircraft | Responsibilities | Role UI |
|---|---|---|---|
| **Fighter** | Light, Multirole, Heavy, Carrier, Stealth | Air superiority, escort, general CAP | Standard radar/HUD |
| **Interceptor** | Interceptor, Heavy | Long-range intercept of strike packages | BVR planner, DLZ emphasis |
| **Strike aircraft** | Strike, Multirole, CAS | Attack ground/naval targets | Targeting pod page, target list sharing |
| **Bomber** | Bomber | Heavy ordnance on infrastructure | Bomb-run planner (route, TOT, release point preview) |
| **Electronic warfare** | EW aircraft (Magpie) | Jamming, ESM, emitter geolocation | EW console (§17.8) |
| **Reconnaissance** | UAV, Stealth, Light | Find/identify targets, share intel | Camera/pod, photo objectives, intel upload |
| **Support** | AWACS (Argus), tanker, CAS | Picture, vectors, refuelling | ABM console (picture + vectoring), tanker controls |

Missions support mixed roles: the generator receives the roster of roles present (`roles_present`) and ensures every role has meaningful tasks (e.g. a bomber and EW player together produce a strike with SAM belt; a recon player yields hidden targets to be found before the strike is possible). Role contribution scoring awards **assists** (jam assist, tanker offload kg, spotted-target kills, escort protection time) to encourage teamwork (§25.11).

## 32.5 Scaling with player count

Generator budgets scale with `N` (§26.8). Roles unfilled are covered by AI wingmen (configurable) so a 2-player session never lacks escorts or tankers. Player cap is 16 (design target); browser-hosted sessions are recommended up to 8 (§33.9). Where `N` is high, the Director increases wave sizes and objective count rather than enemy skill.

## 32.6 Multi-crew aircraft

Aircraft with two or more seats (Condor WSO, Magpie EWO, Argus ABM seats) let additional humans occupy seats of the **same entity**.

| Aspect | Rule |
|---|---|
| Authority | The pilot seat owns flight controls; other seats own sensors/weapons/EW consoles per aircraft data (`seats` block lists capabilities) |
| Input | Each seat's client sends its inputs; the server applies per-seat authority masks to merge commands into the entity |
| Prediction | Only the pilot predicts the airframe; other seats predict their local console state only |
| Hand-over | Pilot can hand control to the other seat ("You have control") with a short confirmation; AI fills empty seats |
| View | Each seat has its own camera; external cameras are shared |

## 32.7 Communications (non-voice)

Text chat (all/team/flight), **map pings** (shared markers on the tactical map with 30 s lifetime), shared **target designation** (mark a target for allies: `designate(target)` creates a cue visible to flight members), and quick-radio wheel messages ("Need cover", "Engaging", "Bingo fuel"). AI radio runs alongside (§56.6).

## 32.8 Join, leave, and drop-out

| Event | Behaviour |
|---|---|
| Late join | Spawns per rule set (base, carrier, or entering formation); briefed with objective state |
| Leave | Aircraft goes to AI (`hold`/RTB) for 15 s then despawns if empty seats; stats retained |
| Disconnect | Slot reserved 120 s; reconnect with the session token (no re-brief); aircraft in AI hold for 15 s then glide/autopilot RTB |
| Host leaves | Session ends (no host migration); each client saves a local result record and replay buffer (§52.6) |
| Kick | Host only; token banned for the session |

## 32.9 Voice chat (outline §148, OQ-06)

Voice is optional per lobby and uses the browser's WebRTC audio stack (echo cancellation, noise suppression, AGC).

- **Topology:** star through the authority. Each client sends one microphone stream (Opus, 24 kbit/s, DTX) to the host; the host mixes per **radio channel** (Flight, Team, All) and sends each client up to two mixed tracks (their active channels). Mixing happens in Web Audio on the host; ≤ 16 talkers is within budget (≈ 0.4 ms/frame).
- **Controls:** push-to-talk (keyboard, mouse, gamepad/HOTAS button — works while the tab is focused), voice-activation, channel select (Flight/Team/All), per-player volume and mute, mic test. Push-to-talk outside the window is impossible in a browser; the UI says so.
- **Radio effect:** receive-side band-pass 300–3 400 Hz, mild soft-clip, squelch tick and noise floor scaled by link quality (jamming reduces intelligibility); optional "clean voice" setting.
- **Bandwidth:** ≈ 3 KB/s per active talker.
- **Privacy:** microphone access is opt-in; no voice is recorded or uploaded; replay files never store voice by default.

## 32.10 Co-op rules and flow

| Aspect | Rule |
|---|---|
| Objectives | Shared; role-tagged objectives show assignment (e.g. "Strike: destroy bridge") |
| Scoring | Team score (mission result) + personal score (kills, assists) + role bonuses |
| Respawn | Shared ticket pool or per-player; respawn at base or after a delay; mid-air respawn only if "air respawn" enabled |
| Pause | Host-only and only if all consent (vote), otherwise disabled |
| Time acceleration | Requires a unanimous vote and no hostile tracks within the safe threshold (§49.3) |
| Friendly fire | Host option; when on, warnings are shown on lock |
| End of mission | Debrief with per-player stats and role contribution graphs |

## 32.11 Co-op in the campaign

When the campaign (§34) exists, a host may run an Operation with friends: the host's save is authoritative, guests receive aircraft slots from the host's inventory, and pilot XP accrues to guest profiles locally. (Staged to M6+.)

## 32.12 Edge cases

| Case | Handling |
|---|---|
| Players with different cached data | Host offers *tile relay* (§33.4); join blocked until minimum coverage satisfied |
| Hardware mismatch | Each client chooses its own graphics profile; authority caps entity count to the weakest *host* profile, not clients |
| Large lag spike | Authority doesn't pause; affected player's aircraft stays predicted; auto-hold if disconnected > 5 s |
| Invite blob expired | Offers have a 5-minute validity; UI generates a new blob |
| Voice permission denied | Text chat and pings still work |

## 32.13 Acceptance criteria

- `AC-32.1` Two browsers on the same LAN with no internet complete the invite-blob exchange and reach a running mission in ≤ 90 s with a cached app.
- `AC-32.2` An 8-player co-op mission with 60 AI aircraft runs for 30 min on the Recommended host profile without the host sim tick exceeding its budget (§0.6.3) and with every client within 100 ms of the authoritative state (measured).
- `AC-32.3` Role-mix test: for each combination of 3 roles, the generator produces ≥ 1 role-specific objective per role.
- `AC-32.4` Multi-crew authority: a WSO cannot move flight controls; a pilot cannot fire the WSO-only weapons (negative tests pass).
- `AC-32.5` Voice: speech heard on the intended channel only; channel leak tests pass (100 % of 200 scripted checks).

---

# 33. MULTIPLAYER ARCHITECTURE

`P7 · M5 (design from M1)` — The multiplayer architecture uses authoritative state management where practical (outline §65, §112) and treats single-player as a degenerate case of the same design.

## 33.1 Requirements

| # | Requirement |
|---|---|
| R1 | The authority validates hits, missile launches, aircraft destruction, objective completion, and major entity state (outline §65). |
| R2 | The owning client's aircraft responds immediately to input (client-side prediction). |
| R3 | Bandwidth scales to 16 humans and 150 aircraft with interpolation, prediction, compression, relevance filtering, entity prioritisation, variable update rates (outline §65, §112). |
| R4 | Works over browser transports (WebRTC DataChannel, WebTransport, WebSocket) without internet after the app is cached (NN-8). |
| R5 | Single-player uses the same pipeline (loopback). |
| R6 | Degrades gracefully with latency, jitter, loss, and slow hosts. |
| R7 | Cheating resistance is limited to authority validation (LAN/VLAN trust model, §33.11). |

## 33.2 Topology and roles

```text
            ┌──────────────── AUTHORITY ────────────────┐
            │ Sim Core (Rust/WASM in a Worker, or native) │
            │ + MissionDirector + AI + Replication        │
            └───────▲──────────────────────────▲─────────┘
        inputs/cmds │           snapshots/events │
        ┌───────────┴───┐                 ┌─────┴──────────┐
        │ Client 1      │ …               │ Client N       │
        │ prediction +  │                 │ prediction +   │
        │ interpolation │                 │ interpolation  │
        └───────────────┘                 └────────────────┘
```

- **Listen server:** the host's browser tab runs the Authority in its Sim Worker and *also* plays as Client 0 (zero-latency transport via in-process `MessageChannel`).
- **Dedicated server:** the native `gdd-server` binary runs the Authority headless (§33.10).
- **Single-player:** a listen server with no remote clients; the "network" is the same `Transport` trait with an in-process implementation (`LoopbackTransport`) that still serialises messages, ensuring the single-player path exercises the real protocol and prediction code (R5).
- **Roles on the authority:** world simulation at 60 Hz; per-client replication scheduler at the snapshot rate; mission director and AI; event validation; recording of the replay stream (§52).

## 33.3 Transport, handshake, and clock

### 33.3.1 Transport abstraction

```rust
pub trait Transport {
    fn send_unreliable(&mut self, peer: PeerId, data: &[u8]);       // state, inputs: may be dropped / reordered
    fn send_reliable(&mut self, peer: PeerId, data: &[u8]);         // events, commands, chat: ordered, reliable
    fn poll(&mut self) -> Option<TransportEvent>;                    // Connected, Disconnected, Message{peer, channel, bytes}
    fn stats(&self, peer: PeerId) -> LinkStats;
}
```

| Transport | Channels | Notes |
|---|---|---|
| **WebRTC DataChannel** (primary) | `reliable` (ordered, id 0) and `unreliable` (`ordered: false, maxRetransmits: 0`, id 1), both **negotiated** with fixed ids to skip in-band setup; `binaryType = "arraybuffer"` | Encrypted (DTLS), works peer-to-host with no server; lives on the main thread (RTCPeerConnection is generally not available in Workers [verify]) and forwards bytes to/from the Sim Worker via SharedArrayBuffer ring buffers |
| **WebTransport** (secondary) | Datagrams (unreliable) + one bidirectional stream (reliable) | For dedicated servers, with `serverCertificateHashes` pinning of a short-lived self-signed certificate [verify support/limits]; available in Workers |
| **WebSocket** (fallback) | One ordered reliable channel | LAN/VPN only; head-of-line blocking mitigated by *sender-side staleness dropping*: queued state snapshots older than one interval are discarded before send; the reliable queue has priority |
| **Loopback** | In-process | Single-player and listen-server host client |

**Payload limits:** unreliable messages ≤ **1 200 bytes** (safe below the IPv6 minimum MTU of 1 280 minus IP/UDP/DTLS/SCTP overhead, and over VPN overhead); reliable messages are fragmented at the application layer into ≤ 16 KiB chunks to stay within every browser's SCTP limits.

### 33.3.2 Packet header (unreliable channel)

```text
u8   version_type      4 bits protocol version, 4 bits message type (Input, Snapshot, Ping, Voice-ctl, ...)
u8   flags             bit0 has_ack, bit1 fragment, bit2 compressed, bit3 server_time
u16  seq               sender sequence
u16  ack               latest received remote seq
u32  ack_bits          bitfield for the 32 packets before ack
[u32 server_tick]      present when bit3 set
```

Overhead: 10–14 bytes.

### 33.3.3 Handshake

```text
Client → Hello { protocol_version, content_manifest_hash, client_nonce, name, capabilities, requested_role }
Server → Challenge { server_nonce }                                     (only when a password is set)
Client → Proof { HMAC-SHA256(password, server_nonce ‖ client_nonce) }
Server → Welcome { client_id (u8), session_config, battlefield_frame, mission_hash, server_tick, tick_rate, snapshot_rate }
         or Reject { reason: version | content_mismatch(list) | full | password | banned | ping }
```

`content_manifest_hash` is a SHA-256 over the content pack versions/hashes (computed with `SubtleCrypto`). A mismatch returns the list of differing packs so the client can sync (§33.4).

### 33.3.4 Clock synchronisation

NTP-style ping/pong at 1 Hz (4 Hz during the first 3 s): client sends `t0`; server replies with `t1` (receive time) and `t2` (send time); client records `t3`.

```text
offset = ((t1 − t0) + (t2 − t3)) / 2          RTT = (t3 − t0) − (t2 − t1)
```

The estimator keeps the 8 most recent samples, selects the minimum-RTT sample, and slews the local offset (±0.5 ms per update) to avoid visible time jumps. `server_tick_estimate = floor((local_monotonic + offset − t_session_start) / SIM_DT)`. Time is `performance.now()` based (monotonic); wall-clock is never used for gameplay.

## 33.4 Join and state transfer

```text
Welcome → client loads: SessionConfig + MissionDef (compressed ≈ 20–60 KB, sent reliably in chunks)
        → content availability check (aircraft/weapon packs, geodata coverage per §8.8)
            missing? → request from host: PackRequest{pack_id, range} → host streams chunks (tile/pack relay, ≤ 4 MB/s)
        → client builds world (terrain streaming starts, assets load)
        → Ready{loadout, role}
Server validates loadout (§14.5) → spawns aircraft / assigns seat → sends full Snapshot (baseline, baseline_tick = 0)
Client acks → server begins delta snapshots; client begins sending Input
```

**Tile/pack relay.** On a fully offline LAN the host can serve geodata and content packs it already has to guests over the reliable channel (range-request-like `PackRequest`), with integrity verified by pack hashes. This honours "LAN gameplay should not require internet access" (outline §123) even when guests lack the region cache.

**Compression:** `CompressionStream('deflate-raw')` for messages ≥ 256 bytes in reliable channels; zstd (WASM) for large packs.

## 33.5 State replication

### 33.5.1 Entity classes and rates

| Class | Content | Base rate (relevant) | Reliability |
|---|---|---|---|
| Player aircraft | Full state | 20 Hz (30 Hz optional) | Unreliable (delta) |
| AI aircraft | Full state | 5–20 Hz by relevance | Unreliable |
| Missiles | Position/velocity/state | 20 Hz within 30 km of a client, else 5 Hz + predicted path | Unreliable; launch/destroy = reliable event |
| Projectiles | Not replicated; **fire events** (seed, start tick, burst length, aim) | event | Reliable |
| Ground units | Position/heading/state (low motion) | 1–5 Hz | Unreliable; destruction events reliable |
| Ships | Position/heading/speed | 2 Hz | Unreliable |
| Mission/objective state | Phase, objective states | on change | Reliable |
| Weather | Parameters | on change | Reliable |
| Civilian/ambient | Not replicated (analytic, §23.2) | — | — |

### 33.5.2 Quantisation (BF-based)

| Field | Encoding | Range / resolution |
|---|---|---|
| Position (BF x, y, z) | 3 × 24-bit signed integers | ±838.86 km at 0.1 m (BF z can be −19.6 km at a 500 km radius because BF is curved; well within range) |
| Orientation | Smallest-three, 32 bits (2-bit index + 3 × 10 bits); the owner's own aircraft uses 3 × 15 bits | ≈ 0.15° worst-case error at 10 bits |
| Velocity | 3 × 16-bit signed | ±3 276 m/s at 0.1 m/s |
| Angular velocity | 3 × 8-bit signed | ±6.35 rad/s at 0.05 rad/s |
| Visual state | throttle 6 bits, AB 1, gear 2, flaps 3, airbrake 2, three control surfaces × 6 bits = 32 bits | |

Full state per aircraft ≈ **26 bytes** (9 + 4 + 6 + 3 + 4) plus ~4 bytes record header. With delta against the acked baseline (position residual to a dead-reckoned prediction, packed with bit-widths from a per-snapshot width header) typical updates are **14–18 bytes**.

### 33.5.3 Delta baselines and acknowledgements

The authority keeps the last 64 snapshots per client. The client acknowledges received snapshots through `ack/ack_bits`. For each entity record the authority uses the newest *acked* snapshot containing the entity as the baseline; if none newer than 1 s exists, a full record is sent. Entities not in the baseline (new spawns) are sent full and also announced by a reliable spawn event.

### 33.5.4 Relevance, prioritisation, and variable rates

Each client has a **priority accumulator** per entity: `acc_i += w_i · dt`, with

```text
w_i = base_class · f_dist(d) · f_engage · f_motion · f_role
f_dist: ≤ 20 km → 1.0; ≤ 80 km → 0.5; ≤ 200 km → 0.25; > 200 km → 0.05
f_engage: ×3 if targeted by/targeting the client's aircraft, ×2 if a missile is tracking it, ×1.5 if a squadron mate
f_motion: 1 + min(|a|/(5 g), 1)              (manoeuvring entities need updates more often)
```

Each outgoing packet is filled with the highest accumulators that fit the byte budget; sent entities reset to 0. This yields the **variable update rates** of the outline: nearby/engaged entities at ~20 Hz, far entities at ~1–5 Hz.

**Per-client bandwidth budget:** default 96 KB/s (768 kbit/s), hard cap 128 KB/s; the budget adapts to measured throughput (§33.7). The scheduler never exceeds it; excess entities simply wait (their accumulators grow).

### 33.5.5 Worked bandwidth example (before delta/relevance savings)

For a Large session: 8 players at 20 Hz, 150 AI aircraft at an average of 5 Hz, 40 missiles at 20 Hz, 26-byte records:

```text
players   8 × 26 B × 20 Hz  =  4 160 B/s
AI      150 × 26 B ×  5 Hz  = 19 500 B/s
missiles 40 × 20 B × 20 Hz  = 16 000 B/s
total                       ≈ 39 660 B/s ≈ 318 kbit/s per client (+ ~10 % header overhead)
```

Delta compression (~35–45 % smaller) and relevance filtering (clients rarely need 150 AI at 5 Hz) bring typical use under 25 KB/s (200 kbit/s). Host upstream for 8 clients ≈ 8 × 40 KB/s = 320 KB/s ≈ 2.6 Mbit/s unfiltered, well within a LAN. Upstream per client: inputs at 60 Hz with 3-tick redundancy ≈ 60 × (48 B + 12 B header) = 3.6 KB/s ≈ 29 kbit/s.

### 33.5.6 Fog-of-information replication

For rule sets with realism ≥ Hard (and always in 1v1/BVR), the authority replicates **only what the client can know**: true state for entities within visual range (by the visual detection model, §16.6.2) and *track* records (with noise from the client's sensors, datalink, ESM) for the rest. The client's tactical map and HUD are built from these track records. This enforces information warfare (§16.11, §42) and halves bandwidth at long range. In Easy/Normal sandbox it is off (full picture).

### 33.5.7 Events

Reliable, ordered events: spawn/despawn, launch (with launcher, weapon, target, seed, `launch_seq`), hit/kill (attacker, victim, weapon, result), damage state changes, objective/phase updates, radio events (semantic ids + params, §19.13), chat, weather change, lobby changes, boundary warnings, countermeasure dispense (for visuals), detonations, crash. Typical size 8–40 bytes; events are idempotent (ids + `event_seq`), and re-sent on reconnect from the client's last acked `event_seq`.

## 33.6 Prediction, reconciliation, interpolation, lag compensation

### 33.6.1 Owned aircraft: prediction with reconciliation

1. The client samples inputs each sim tick (60 Hz), applies them immediately to a **local copy** of the Sim Core (flight model only, with environment query services), and stores `(tick, input, predicted_state)` in a 1-second ring.
2. Inputs are sent with 3-tick redundancy; the server consumes the input for tick `t` when available. A server-side **input jitter buffer** of 2 ticks (33 ms; adaptive 1–6) absorbs arrival variance; if an input is missing the last input is held and flagged.
3. Every snapshot carries `last_processed_input_tick` and the authoritative state of the client's own aircraft at that tick.
4. On receipt: compute the error against the stored predicted state for that tick. If `|Δp| < 0.5 m` and `|Δatt| < 0.2°`, ignore. Otherwise **rewind and replay**: set state to authoritative, re-apply the stored inputs for ticks `t+1 … now` (≤ 60 ticks; ≈ 0.02 ms per tick), obtaining the corrected present. The *visual* aircraft converges to the corrected state by an exponential offset decay (`τ = 100 ms`); errors > 50 m snap.
5. Because the Sim Core is determinism-capable (ADR-10) and server and client run the same FDM code, steady-state corrections are near zero; corrections occur at collisions, damage, server-applied forces (catapult, arrestor), and wind-turbulence seed differences (turbulence seeds are server-issued per aircraft and used by prediction).

### 33.6.2 Remote entities: snapshot interpolation

Rendering time for remote entities is `render_time = server_time_estimate − interp_delay`, where `interp_delay = 2 × snapshot_interval + jitter margin`, adaptively between 70 and 200 ms (default 100 ms at 20 Hz). Positions use **cubic Hermite** interpolation using velocities from the snapshots; orientations use slerp with angular-velocity-based tangents. If snapshots are late, extrapolate (dead reckoning with velocity and clamped acceleration) for up to 250 ms, blending back on arrival.

### 33.6.3 Weapons

- **Missiles:** server-authoritative. The launching client spawns a *cosmetic* missile immediately on trigger press (using the aircraft's predicted state) and attaches it to the authoritative missile when the first state with the matching `launch_seq` arrives, blending position over 200 ms. Missile guidance and fuzing are never run on clients except for cosmetics.
- **Guns:** the client plays tracers and sound immediately from its own aim. The **fire event** (start tick, burst length, aim, seed) goes to the server. The server resolves hits with **lag compensation** (below). Only the authority applies damage.
- **Countermeasures:** the dispense command is a client input; the authority spawns the CM entities (flares/chaff) and replicates them as events; clients show cosmetic CM immediately.

### 33.6.4 Lag compensation for guns

The authority keeps a **pose history ring** of every aircraft's hit-volume transforms for the last 300 ms at 60 Hz. For a burst fired at client-perceived time `t_v = t_server_at_receipt − (RTT/2) − interp_delay`, define the constant offset `Δ' = clamp(t_fire_server − t_v, 0, 250 ms)`. For the whole life of each round in that burst the authority tests the projectile at server time `t_s` against **target poses at `t_s − Δ'`** (always in the past, hence available from the ring). The shooter pose is validated against the server's own history (tolerance 2 m, 1°) and the burst is rejected (rate-limited, logged) if out of tolerance. Result: the player hits what they saw (to within the interpolation fidelity); the target takes the damage Δ' later than the shooter saw it. Rewinds above 250 ms are clamped (an unavoidable fairness trade-off for high-ping players). Missiles use current server state (no compensation), which gives high-latency players a slightly worse defensive experience; RWR/MAWS cues are server-timed to preserve the sense of "I was warned".

## 33.7 Connection quality and failure handling

| Mechanism | Detail |
|---|---|
| Measurements | RTT (min-filter), jitter (EWMA of inter-arrival variance), loss (from ack bits), bandwidth (EWMA), server tick time |
| Adaptive rate | If loss > 10 % or RTT > 150 ms for 3 s: snapshot rate 20 → 10 Hz, drop far-class updates, widen interpolation delay to 150 ms; recover after 10 s stable |
| Unstable link | No packets for 1 s: "Connection unstable" banner; 3 s: input autopilot "hold" on server; 10 s: client considered disconnected (slot reserved 120 s) |
| Congestion | Sender stops adding entities beyond the budget; reliable queue backpressure with priority classes (chat < events < control) |
| Server overload | If the authority's tick slips (>25 ms for 3 s), it lowers snapshot rate, sheds low-priority AI to Tier 3, and warns clients "host overloaded"; if the host tab is throttled (§5.6) it shows the instructions |
| Client disconnect | Server holds the aircraft on AI `hold` for 15 s; reconnect restores control; replay buffer retained |
| Server disconnect | Client freezes interpolation, shows the "Server lost" screen, saves the last 60 s ring as a local replay, offers return to menu |
| Version/content drift | Reject at handshake with human-readable differences |

## 33.8 Offline and LAN operation (NN-8, outline §123)

After the first visit, the **service worker** serves the app shell and previously cached packs; no network is needed to play single-player or LAN. Connection setup works offline via invite blobs (§32.2 method A). Geodata missing on a guest can be relayed from the host (§33.4). The only features requiring internet are first-time installs, region pack downloads, the optional lobby relay, and updates.

## 33.9 Hosting in a browser tab

Constraints and mitigations:

| Constraint | Mitigation |
|---|---|
| The page must stay visible; background tabs are throttled [verify] | Host banner; the Sim Worker keeps running but timers and rendering may throttle: keep an **audio context** playing (silent loop) and hold a **Web Lock** to reduce throttling [verify]; measure real tick rate and warn clients on slippage |
| One machine runs authority + a render client | Host budget model: sim ≤ 5.5 ms/tick for the sim + 1.5 ms replication; render budget as usual; capacity guidance: ≤ 8 humans and ≤ 120 aircraft on Recommended; ≤ 4 humans and ≤ 60 aircraft on Minimum |
| Memory | Authority state ≈ 200–400 MB at Large; counted in §0.6.3 budgets |
| Network | WebRTC SCTP throughput ≈ tens of Mbit/s: fine; message rate ≤ 3 000 msg/s per connection; the replication scheduler batches into ≤ 1 200 B packets |
| Tab crash | Session ends (no migration); clients save replays |

For ≥ 8 humans or tournament play, use the dedicated server (below), optionally with the host as a pure client.

## 33.10 Dedicated server

`gdd-server` is a native Rust binary reusing the Sim Core crate and the replication layer.

- **Headless:** no renderer; terrain from local pack archives (it only needs base-DEM heights, land cover and airports); audio and UI absent.
- **Transports:** WebTransport (self-signed ECDSA certificate with a validity ≤ 14 days, auto-rotated; the server prints a **server code** `host:port#certhash`), WebSocket (plain `ws` on LAN or `wss` with a user-provided certificate), and a WebRTC endpoint (data channels) for hosts that can supply signalling [verify].
- **Config (TOML) and admin:** ports, max players, password, rule defaults, content directory, tick/snapshot rates; console commands (`kick`, `ban-token`, `status`, `set`, `say`, `save-replay`); optional JSON metrics endpoint (tick time, players, bandwidth) for monitoring.
- **Resource:** target ≤ 1 core for 8 humans/120 aircraft at 60 Hz; memory ≤ 1 GB.
- **Distribution:** single static binary per OS and an OCI container; documentation for LAN/VLAN use.
- **No account services:** none.

## 33.11 Security and trust (LAN/VLAN model)

- **Authority validation:** all state changes originate on the authority; clients send *intents* (inputs, commands). Every command is bounds-checked (rate, range, enum validity); unknown fields dropped; messages with sizes beyond limits dropped.
- **Rate limits:** inputs ≤ 120/s, commands ≤ 20/s, chat ≤ 5/s, voice-control ≤ 10/s per client; offenders throttled then kicked.
- **Transport security:** WebRTC and WebTransport are encrypted by default (DTLS/TLS 1.3); WebSocket is plaintext unless `wss`; passwords never travel in the clear (challenge-response).
- **Integrity:** content pack hashes verified on load; replays carry a pack-hash header.
- **Identity:** a local pseudonymous profile (name + random token) — no accounts. Bans are token-based for the session.
- **Out of scope:** anti-cheat for open internet play. The browser client is inspectable; fog-of-information replication (§33.5.6) limits information advantage for honest-play rule sets but does not stop a determined cheater.

## 33.12 Observability (outline §120)

The network debug panel (§63.6) shows per-peer RTT, jitter, packet loss, bandwidth (in/out), server tick time and rate, snapshot rate, entity counts, relevance buckets, correction events (count, mean magnitude), interpolation delay, input buffer depth, and a rolling histogram of prediction error. The panel is available in release builds under `?debug=net`.

## 33.13 Testing

| Test | Method |
|---|---|
| Network emulator | A `ConditionedTransport` wrapper injects latency (0–500 ms), jitter, loss (0–20 %), reordering, duplication, bandwidth caps; used in CI and exposed in the debug panel |
| Soak | 16 simulated clients (headless WASM) against a 150-aircraft mission for 30 min; compare client reconstructions to the authority (state hash per snapshot tick) |
| Protocol fuzzing | Random/mutated packets into the parser: no panics, bounded memory |
| Quantisation properties | Round-trip error bounds for each field (property tests) |
| Determinism | WASM vs native state hash after N ticks (ADR-10) |
| Prediction | Scripted maneuvers across latency/loss matrix; maximum visual error and correction counts |
| Lag compensation | Scripted gun duels at fixed pings; hit-rate parity vs zero latency within ±10 % |
| Browser matrix | Chromium, Firefox, WebKit builds for transport features (feature-detect and graceful fallback) |

## 33.14 Acceptance criteria

- `AC-33.1` LAN (≤ 2 ms RTT): median prediction correction magnitude < 0.05 m; 99th percentile < 0.5 m over a 10-minute dogfight.
- `AC-33.2` At 100 ms RTT and 5 % loss, the owned aircraft has no visible rubber-banding > 2 m and remote aircraft interpolate without teleports > 5 m.
- `AC-33.3` Bandwidth per client with 150 aircraft and 40 missiles ≤ 128 KB/s peak and ≤ 60 KB/s average (relevance + delta on).
- `AC-33.4` Authority tick time ≤ 5.5 ms avg for 8 humans/120 aircraft on the Recommended profile (browser-hosted) and ≤ 4 ms on the native dedicated server.
- `AC-33.5` Reconnect within 120 s restores control and state in ≤ 2 s after reconnection.
- `AC-33.6` Lag-compensated gun hit-rate at 100 ms RTT is within ±10 % of the 0-ms hit-rate for the same scripted aim.
- `AC-33.7` The invite-blob exchange works offline on two machines with no internet (validated in a network-isolated test).
- `AC-33.8` Fuzzed packet corpus (10⁷ packets) causes no crashes or unbounded allocations.
- `AC-33.9` Loopback single-player and a 1-client LAN session produce identical authority-side state hashes when the same scripted inputs are applied at the same ticks (network arrival timing excluded) (R5).

---

# 34. ROGUELITE CONQUEST CAMPAIGN

`P9 · M6` — The long-term campaign transforms the sandbox into a persistent strategic war (outline §66, §71–72, §149). The player moves from airport to airport; each airport is a strategic node; winning battles captures territory. The campaign is a **strategic layer over the same mission architecture** (ADR-14), not a separate game.

## 34.1 Intent

- A war that remembers: losses, captures, enemy personalities and decisions carry forward.
- Choices rather than a mission list: where to strike, what to risk, what to defend (NN-7).
- Roguelite pacing: runs end in victory or strategic collapse; meta-progression broadens future runs.
- Flying stays primary: logistics is light (§39), pilots are light (§41), the map is a decision surface not a spreadsheet.

## 34.2 Campaign structure

```text
CAMPAIGN RUN (seeded)
 ├── Region & Airport Graph (§35)
 ├── Factions & Forces (§37)
 ├── Strategic State: ownership, garrisons, supplies, intel, clock
 ├── Event Deck (§40)
 ├── Pilot/Airframe roster (§39, §41)
 └── Operation Cycle (below)
```

### Operation cycle

```text
STRATEGIC MAP → review situation (frontline, intel, readiness, events)
 → CHOOSE OPERATION (3–6 offers, §38.4)
 → GENERATE OPERATION (GeneratorRequest + StrategicContext, §26)
 → BRIEFING (intel-limited) → LOADOUT (from inventory) → FLY MISSION
 → DEBRIEF → APPLY RESULTS (losses, XP, ownership, resources, intel)
 → ENEMY TURN (strategic AI, auto-resolve elsewhere, §35.5)
 → EVENTS (draw from deck, §40) → CAMPAIGN CLOCK ADVANCES → repeat
```

**Campaign clock.** Each operation advances time by 6–24 hours (operation-dependent; "Rest/Repair" advances 12–48 h). The clock sets the date/season used for environment generation, resource accrual (per day), repair progress, and enemy turn cadence.

## 34.3 Campaign generation (outline §71)

A campaign generates, from a **campaign seed** (same Crockford Base32 format as mission seeds, §26.3):

| Element | Generation |
|---|---|
| Region | Centre + radius (preset regions or chosen on the world map) with a data-coverage check |
| Airport ownership | Partition of the airport graph into player / enemy / neutral territories (§35.1) |
| Starting location | Player's base selected for runway quality, defensibility, and ≥ 2 neighbours |
| Enemy composition | Strategic force pool (TV) distributed over nodes by strategic value |
| Aircraft availability | Starting airframe pool from the faction roster and era (§37) |
| Mission chains | Pre-planned operation "arcs" (e.g., a three-step approach to a fortified hub) offered as suggestions |
| Objectives | Strategic victory conditions (below) |
| Special events | Event deck composition (§40) |
| Elite squadrons | Named enemy squadrons placed at nodes/rear areas (§37.5) |
| Boss encounters | Aces from the boss pool seeded with identities and schedules (§29) |
| Resource availability | Starting supplies, credits, production sites (§39) |

**Campaign sizes:** *Skirmish campaign* (≈ 10 nodes, 8–12 operations), *Standard* (≈ 25 nodes, 25–40 operations), *Grand* (≈ 50 nodes, 60+ operations). Difficulty presets: Recruit, Standard, Veteran, Iron (permadeath of the player's pilot).

**Strategic victory conditions** (any one, chosen per campaign): *Decapitation* (capture the enemy HQ node), *Dominance* (control ≥ 70 % of nodes by strategic value for 3 consecutive turns), *Attrition* (destroy the enemy's production nodes and reduce their field strength below a threshold), *Survival* (hold your HQ and ≥ 3 nodes through a countdown).

**Strategic defeat:** loss of the HQ node with no recapture option, or no operational aircraft and insufficient credits to procure any within 3 turns, or the enemy's victory condition satisfied.

## 34.4 Roguelite structure

Persistent consequences within a run (outline §72): aircraft destroyed or damaged; resources consumed; airports captured or lost; enemy adaptation (§38.3); special missions; elite enemy appearances. **Meta-progression** across runs unlocks aircraft, weapons, cosmetics, faction options, starting bonuses and difficulty options via **Legacy Points** (§38.5).

Campaign unlocks affect **campaign starts only**: sandbox and multiplayer have all shipped content available, so progression never gates the "pick a place, pick an aircraft, fly" fantasy.

## 34.5 Operations

An **Operation** is `GeneratorRequest + StrategicContext + PersistenceRules`.

| Operation | Maps to mission types (§27) | Strategic effect |
|---|---|---|
| **Attack node** (capture) | Strike / SEAD / Mixed / Air Superiority with capture objective (§36) | Node ownership change |
| **Defend node** | Interception / Air Superiority / Survival (enemy counterattack) | Prevent loss; attrition on enemy |
| **Escort supplies** | Escort | Delivers supplies/credits to a node |
| **Intercept reinforcements** | Interception / Convoy Attack | Reduces enemy strength at a target node |
| **Reconnaissance** | Reconnaissance | Intel reveal |
| **Strategic strike** | Bombing / Strike | Reduces production/supply/C2 |
| **Naval operation** | Carrier Defense / Fleet Battle | Fleet movement and sea control |
| **Rescue** | Escort/Recon variant | Recover a downed pilot (§41) |
| **Special mission** | Boss Hunt / unique scenarios | Elite elimination; rare unlocks |

`StrategicContext` feeds the generator with: the target node's defences, known/estimated enemy forces (fog-adjusted), frontline geometry (for Dynamic Frontline), available friendly squadrons/airframes (§39), weather season/date, supply states, and relevant campaign flags. `PersistenceRules` define what persists back (§34.6).

## 34.6 Architecture hooks

The campaign plugs into the generic architecture through four interfaces so the sandbox runs with stubs (AC-6.5):

```rust
pub trait ForceProvider {        // which forces exist for a side in this operation
    fn friendly_forces(&self, op: &OperationId) -> ForceList;
    fn enemy_forces(&self, op: &OperationId) -> ForceList;           // true strengths (fog applied only to UI)
}
pub trait AirframeSource {       // inventory and condition of the player's aircraft
    fn available(&self, filter: &AircraftFilter) -> Vec<AirframeRecord>;
    fn reserve(&mut self, id: AirframeId) -> Result<()>;
}
pub trait PersistenceSink {      // results back into the campaign
    fn apply(&mut self, result: &MissionResult) -> CampaignDelta;
}
pub trait StrategicContext {     // strategic inputs for generation
    fn context_for(&self, op: &OperationId) -> GeneratorContext;
}
```

`MissionResult` includes: per-airframe condition (component HP at landing, fuel/ammo used), kills (by unit/aircraft ids), losses, objective states, intel gained, pilot outcomes, node damage, and the replay id. The sandbox implementations return infinite airframes and discard results.

## 34.7 Seeds and reproducibility

`mission_seed = SplitMix64(campaign_seed ^ fnv1a64("op") ^ op_index ^ node_id)`. A campaign can be re-created from its seed + choices log (decisions + mission outcomes), enabling shareable "campaign seeds" and debugging. Strategic randomness (events, auto-resolves) uses named streams (`"events"`, `"autoresolve"`, `"ai/strategic"`).

## 34.8 Edge cases

| Case | Handling |
|---|---|
| Player loses the only airfield | Strategic defeat unless a carrier/improvised strip is available (emergency relocation operation) |
| No valid operations available | Director inserts a *Hold* or *Recon* operation; if still none, prompts "Pass turn" |
| Region has too few airports | Improvised airfields as nodes (low SV) |
| Player ignores threatened node | Auto-resolve with a penalty to local morale |
| Save/quit mid-operation | Resume at last checkpoint (§64.4) or restart the mission with the same seed |

## 34.9 Acceptance criteria

- `AC-34.1` A 25-node campaign generates in ≤ 2 s and validates (connected graph, start with ≥ 2 neighbours, victory condition achievable per static analysis).
- `AC-34.2` Re-running a campaign from seed + choices log reproduces node ownership at every turn (golden test).
- `AC-34.3` The interfaces of §34.6 allow the sandbox build to compile with campaign code excluded.
- `AC-34.4` 20 simulated campaigns with an AI "player" produce a spread of outcomes (victory 30–70 %, defeat remainder) at Standard difficulty.

---

# 35. STRATEGIC MAP

`P9 · M6` — The strategic map is the world map (§9) plus the airport graph, ownership, frontlines, supply and intelligence.

## 35.1 Airport graph (outline §67)

Airports form a graph: nodes are strategic locations; edges are operational connections.

```text
BASE A
 ├── AIRPORT B
 │    ├── AIRPORT C
 │    └── AIRPORT D
 └── AIRPORT E
      └── AIRPORT F
```

The tree in the outline is a *view* (BFS from the player's base) of an undirected, planar-ish graph that usually has cycles (more choice, flanking, counterattacks).

### 35.1.1 Node selection

```text
candidates  ← usable airports in the region (types: Military, Regional+, Remote with runway ≥ 900 m; Improvised strips as fillers)
importance  ← I = w_type · (1 + 0.5·runway_count) · (1 + 0.3·hangar_count/10) · (1 + 0.4·pop_nearby_norm)       w_type: Military 4, Major 3, Regional 2, Remote 1
nodes       ← greedy weighted farthest-point sampling: start with highest-I; repeatedly add the candidate maximising I · min(dist to chosen)/d_min, subject to dist ≥ d_min
d_min       ← 60 km (Skirmish), 90 km (Standard), 120 km (Grand);   stop at N nodes (10 / 25 / 50)
```

Carriers are **mobile nodes** attached to a home sea region; they join the graph with edges to any node within `R_edge_max` of their station.

### 35.1.2 Edge construction

1. Project node positions with an azimuthal equidistant projection at the region centre.
2. Compute the Delaunay triangulation (planar).
3. **Prune:** drop edges longer than `R_edge_max` (500 km Skirmish/Standard, 800 km Grand) or longer than `2.2 ×` the shorter of the two nodes' nearest-neighbour distances; drop edges that cross a sea gap wider than 300 km with no carrier/island node.
4. **Connectivity repair:** add edges of the minimum spanning tree (Kruskal) over all nodes if the pruned graph is disconnected (allowing up to `1.5 · R_edge_max` for MST edges).
5. **Density:** ensure each node has degree ≥ 2 (add nearest-neighbour edges) and target mean degree 3–4.
6. **Attributes:** length `d_e` (geodesic), terrain difficulty (mean ridge crossing from the DEM along the great circle), threat (SAM/AAA coverage along the path), supply capacity (road/rail density or air capacity).

Validation: connected; no node of degree 0; player start has ≥ 2 neighbours; every node reachable within `k` hops (k ≤ 8 Standard).

### 35.1.3 Strategic value

```text
SV = w_type · (1 + 0.5·betweenness_norm) · (1 + 0.25·facility_score) · (1 + 0.5·is_production) · (1 + 0.5·is_hq)
```

`betweenness_norm` ∈ [0,1] (graph centrality), `facility_score` from hangars/fuel/repair, `is_production`/`is_hq` flags. SV drives the enemy AI's targeting and the auto-resolve stakes.

## 35.2 Node state

| Field | Meaning |
|---|---|
| `airport_id`, `type`, `position` | Static |
| `owner` (faction) | Player/enemy/neutral |
| `garrison` | Squadrons based (aircraft counts by type and state), ground defences (SAM tier, AAA tier), naval presence |
| `facilities` | Runway, taxi, hangars, fuel, repair, C2, radar, defences — each with health (§48.7) |
| `stock` | Supplies, fuel, ammo, spares (§39) |
| `strategic_value` | §35.1.3 |
| `intel` | Knowledge map for the player (§42) |
| `alert_level` | Readiness state (§48.5) |
| `weather` | Climatology-driven forecast |

## 35.3 Frontlines (outline §69, §142)

Airport ownership creates a dynamic frontline. Map representation:

- **Ownership regions:** a *power diagram* (weighted Voronoi) over node positions with weight = `SV`, clipped to the campaign region polygon, tinted by owner.
- **Frontline edges:** graph edges whose endpoints have different owners; these define legal attack/defend targets and are drawn as bold lines.
- **Frontline polyline:** boundaries between differently-owned power cells, drawn as a smooth curve (Catmull-Rom through boundary vertices) with arrowheads indicating recent movement.
- **Dynamic Frontline missions** (§27.16) use this geometry: the mission's control points are placed along the local front and the mission result shifts the front.

Frontlines move when airports change hands; enemy forces counterattack; supply lines reroute.

## 35.4 Reach and legal operations

An operation target must be (a) adjacent to an owned node via an edge with length ≤ the operating radius of the player's available aircraft (combat radius from the loadout planner, §14.3), or (b) reachable via a **long-range operation** (tanker support or carrier relocation; additional cost, §49). Defensive operations apply to owned nodes under threat.

## 35.5 Strategic AI (enemy turn)

The enemy acts once per campaign day or per operation (whichever comes first). The strategic AI chooses actions to maximise a utility with a small look-ahead:

| Action | Cost / effect |
|---|---|
| Reinforce node | Move TV from the reserve/production to the node; improves defence |
| Attack adjacent player node | Requires strength ratio ≥ threshold; creates a **Defend** offer (player may fight it) or auto-resolve |
| Airlift/resupply | Increases stock |
| Repair | Restores facility health at capacity rate |
| Raid | Small strike causing facility damage and local morale loss (event, §40) |
| Hold / reorganise | Rest to build strength |

Utility terms: `gain = Σ SV × P(success)`, `risk`, `exposure` to player operations, `doctrine` (aggression), and **adaptation** (§38.3). The strategic AI sees only what its faction would know (own fog), so feints and surprise operations are possible.

### Auto-resolve model

For an enemy attack not played by the player (or a player-skipped defence):

```text
r = A / (m_def · D)            A = attacking TV, D = defending TV, m_def = 1.25 + 0.1·defence_tier (fortified nodes)
P(attacker wins) = r² / (1 + r²)            (r=1 → 0.5; r=2 → 0.8; r=0.5 → 0.2)
attacker losses  = A · min(0.6, 0.30 · r^{−0.5} · (0.5 + 0.5u₁))          u₁, u₂ ~ U(0,1) from "autoresolve" stream
defender losses  = D · min(0.6, 0.30 · r^{+0.5} · (0.5 + 0.5u₂))
winning attacker captures the node; survivors occupy it; facility damage ∝ attacker strength
```

The player's own forces at the node participate with their TV (airframes at the node may be destroyed in an auto-resolved defeat).

## 35.6 Map UI (campaign layers)

Reuses the world map (§9) with: ownership tint, frontline curves, node icons (garrison pip, damage glyph, supply bar, alert state), edges (thickness = capacity; dashed = long-range), intel fog overlay, operation offers as pins with difficulty stars, event markers, a timeline panel (past/next events), and tooltips with **estimated** enemy TV and confidence (§42). Interaction: select node → panel with intel, attackable status, quick-launch operations; keyboard shortcuts; end-turn only through operations or "Hold".

## 35.7 Performance

Graph sizes (≤ 60 nodes) are trivial; the map renders at globe speed. Power-diagram recalculation on ownership change < 5 ms.

## 35.8 Acceptance criteria

- `AC-35.1` Graph validity checks of §35.1.2 pass for 1 000 random regions of each size.
- `AC-35.2` Frontline edges equal the set of edges with differing owners, and the drawn front passes through every such edge midpoint ± 10 % of the edge length.
- `AC-35.3` Auto-resolve outcome frequencies match the formula (P(attacker wins) within ±2 % over 10⁵ trials for r ∈ {0.5, 1, 2}).
- `AC-35.4` Operation offers always respect legality rules (range, ownership) in 1 000 random states.

---

# 36. AIRPORT CAPTURE

`P9 · M6` — Capturing an airport turns an enemy node into a usable strategic asset (outline §68).

## 36.1 Capture difficulty levels

| Level | Requirements (all must hold) |
|---|---|
| **Simple** | R1: enemy armed forces at the base (aircraft on ground/air within 5 km, armed ground units) reduced below 10 % of initial TV |
| **Standard** | R1 + R2: ≥ 80 % of defence nodes (SAM, AAA, radar) destroyed + R3: no enemy aircraft airborne within 30 km for 60 s + R4: *secure* the base — maintain R1–R3 for 120 s |
| **Hard** | R1–R4 + R5: command infrastructure destroyed (C2 bunker, control tower, radar) + R6: a friendly aircraft lands and comes to a stop on the runway/apron, then 30 s to secure |

The campaign difficulty selects the default level; operations can have custom requirements by node type (HQ nodes use Hard).

## 36.2 Capture meter and feedback

A **control meter** `C ∈ [0, 1]` summarises progress for UX:

```text
C = w1·R1 + w2·R2 + w3·R3 + w4·min(1, t_secure/120) + w5·R5 + w6·R6                    (weights per level; normalised)
```

The HUD shows requirements as a checklist with live status ("Defences: 5/6 destroyed", "Air cover: 40 s / 60 s") and a meter on the tactical map. Failing a requirement resets only its timer, not the others.

## 36.3 After capture

- Ownership flips; the node's `garrison` is replaced by surviving friendly forces (and any reinforcements flown in as a follow-up operation).
- **Facility state** is what the battle left: runway craters (closed until repaired), destroyed hangars, damaged fuel farms, etc. Repair times (design): runway crater 4–8 h per crater (capacity-dependent), taxiway 2 h, hangar 24 h, fuel farm 12–24 h, radar 12 h, SAM site 24–48 h. Captured assets: stock (50 % of surviving supplies), possible abandoned aircraft (10 % per intact enemy aircraft at the base).
- Use of the node: spawn point for operations, repair/rearm/refuel, forward base extending reach (§35.4).
- **Garrison need:** a freshly captured node has `alert_level` low and defence tier 0; the enemy strategic AI is more likely to counterattack nodes with low defence and high SV (§35.5).

## 36.4 Counter-capture

Enemy capture of player nodes follows the same rules (auto-resolve or played as a Defend operation, in which the enemy requirement set is used as the failure condition for the player: if the enemy meets R1–R4 the node is lost).

## 36.5 Edge cases

| Case | Handling |
|---|---|
| Capture condition met while the player is dead | Counts if the AI wingmen maintain; else deferred to next operation |
| Hard capture with an unusable runway | Landing on a cleared taxiway/apron strip or via heli-pad-equivalent (extraction zone) accepted; UI explains |
| Base abandoned by defenders | R1 auto-satisfied; remaining requirements still apply |
| Multiple players (co-op) | Any player's aircraft counts; landing by any player |

## 36.6 Acceptance criteria

- `AC-36.1` Requirement evaluators (R1–R6) match ground-truth in scripted scenarios (100 % pass).
- `AC-36.2` Post-capture facility states persist in the save and are reloaded exactly.
- `AC-36.3` The checklist UI updates within 1 s of any state change.
- `AC-36.4` A Hard capture cannot complete without a landing (negative test).

---

# 37. FACTIONS

`P9 · M6 (data model from M3)` — Factions are initially **abstract and fictional** (outline §70). Real geography does not require real-world nations (ADR-11).

## 37.1 Intent

Give each side an identity — colours, hardware, doctrine, voices, base style — without mapping to real countries, and make the identity matter in play.

## 37.2 Faction definition

```json
{
  "schema": "faction/1", "id": "azure_league", "name_key": "faction.azure_league",
  "colors": { "primary": "#2f6fb5", "secondary": "#e9eef3", "accent": "#f2b134" },
  "emblem": { "grammar": "wing_shield", "seed": 4417 },
  "eras": ["G4", "G4.5", "G5"],
  "roster": { "air": ["peregrine", "gannet", "wraith:limited", "argus", "magpie", "stratus"], "ground": ["mbt_b", "sam_med", "sam_long", "ewr_1"], "naval": ["carrier_large", "destroyer", "frigate"] },
  "weapons": ["dart4", "lance6", "pike9:limited", "talon1", "hunter2", "tern2", "gb500g"],
  "doctrine": { "style": "network_centric", "aggression": 0.55, "preferred_range_km": 70, "emcon": "intermittent", "cm_discipline": 0.8, "sead_priority": 0.7 },
  "ai_skill_dist": [["veteran", 0.5], ["elite", 0.4], ["ace", 0.1]],
  "communication": { "callsign_sets": ["raptor_birds", "weather"], "voice_personas": ["voice_a", "voice_c"], "phrasebook": "azure" },
  "airbase_style": { "layout": "open_apron_dispersed", "shelters": 0.3, "palette": "light_grey" },
  "strategic_ai": { "personality": "opportunist", "risk_tolerance": 0.5, "adaptation_rate": 1.0 }
}
```

## 37.3 Launch factions (fictional)

| Faction | Identity | Typical hardware (era) | Doctrine |
|---|---|---|---|
| **Azure League** | Maritime coalition; carriers, networked sensors | Peregrine, Gannet, Wraith (limited), Argus, Magpie; Lance-6, Dart-4, Pike-9 (G4.5/G5) | Network-centric BVR, EW, stand-off strikes; fights at range, avoids merges |
| **Meridian Pact** | Continental bloc; mass interceptors and layered air defence | Swift, Condor, Bittern, Albatross, Kestrel; Lance-4, Dart-1/4 (G4) | Layered SAM belts, massed raids, GCI-led intercepts |
| **Concord of Free Ports** | Mixed coalition, improvised airfields, adaptable | Kestrel, Badger, Bittern + mixed lend-lease (G3/G4) | Opportunistic, dispersed bases, shoot-and-scoot SAMs |
| **Iron Marches** | Land power; armour and CAS heavy, rugged airstrips | Badger, Bittern, Albatross, Kestrel (G3) | Armoured pushes, heavy AAA, brute-force ground support |

(Factions can be mixed with the era filter in sandbox; the roster lists inform generation weights, not restrictions, except in campaign.)

## 37.4 Emblems and liveries

Procedural emblem grammar (shape + symbol + stripe sets) generates a faction/squadron emblem from a seed; a **similarity lint** compares emblems against a blacklist of known national/organisational symbols (shape-hash comparison) and rejects close matches. No real flags, insignia or roundels. Liveries derive from faction palettes with squadron-level variants (stripe, tail colour).

## 37.5 Living enemy roster

At campaign creation each enemy faction receives **named squadrons** (callsign, aircraft, skill distribution, personality) and a **pilot roster** with individual XP; the best pilots can be promoted to **aces** with dossiers (§29.5, §42.3). Squadrons have identities that persist: location, losses, replacement flow. Attrition and promotion are modelled (a squadron that loses its leader promotes a deputy; heavy losses reduce effectiveness until reinforced). This produces recurring antagonists.

## 37.6 Relations

Campaign configurations: player vs 1 enemy (default), vs 2 rivals (a three-way war where rivals also fight), with a neutral third faction (civilian authority) or allied faction providing occasional reinforcements/events. Relations drive event availability (§40).

## 37.7 Communication style

Each faction has callsign sets, a voice persona list, and a phrasebook mapping semantic radio events to phrase variants (shared brevity vocabulary, variations in word choice and cadence). All communication is in a neutral invented-flavoured English (localization keys) to avoid stereotyping real nationalities; voice personas avoid caricature accents.

## 37.8 Content pipeline

Adding a faction is data-only: JSON + palette + emblem seed + voice set + phrasebook. Factions have `era`, `tech_level`, and `rarity` for generator weights. Validation: all referenced ids exist; roster has ≥ 1 fighter, ≥ 1 strike, ≥ 1 SAM.

## 37.9 Acceptance criteria

- `AC-37.1` The emblem lint rejects 100 % of a test corpus of real national/organisation symbols and accepts ≥ 95 % of random seeds.
- `AC-37.2` Faction data validation passes for all four launch factions and rejects intentionally broken test factions.
- `AC-37.3` Faction identity is perceptible: in a blind test players identify the faction from 30 s of gameplay footage (radio, paint, tactics) with ≥ 60 % accuracy.

---

# 38. CAMPAIGN PROGRESSION

`P9 · M6` — Progression happens at three levels: operation (moment-to-moment), run (strategic), and meta (across runs). Decisions matter: the player chooses among operations rather than following a list (outline §77).

## 38.1 Operation-level progression

Mission score, pilot XP (§41), and objective rewards (supplies, credits, intel). Operation difficulty is shown as a 1–5 star rating computed from `TV_enemy_known / TV_player_expected` with a fog-uncertainty band.

## 38.2 Run-level progression (persistent consequences)

| Persistent element | Carried across operations |
|---|---|
| Airframes | Condition (component HP), hours, scars, modifications, loss |
| Pilots | XP, rank, traits, injuries, deaths |
| Resources | Supplies, credits, intel (§39) |
| Territory | Airport ownership and facility health (§36) |
| Enemy | Squadron identities, losses, adaptation counters (§38.3) |
| Narrative | Chronicle entries, relationships (§43) |
| Clock/season | Date, weather trends |

## 38.3 Enemy adaptation

The enemy faction maintains **adaptation counters** measuring how the player wins:

| Counter (player behaviour) | Enemy adaptation (bounded +10–25 % weight shift) |
|---|---|
| Heavy BVR use | More IRST/EMCON-capable fighters, more decoys, more terrain-masking CAP |
| ARM-heavy SEAD | SAMs adopt ambush/shoot-and-scoot, more decoy emitters, better EMCON discipline |
| Low-level penetration | More AAA, MANPADS, and short-range SAM at approach valleys |
| Stealth/EMCON reliance | More IRST, passive sensors, AWACS patrols |
| Naval strikes | Stronger CIWS/soft-kill, more screening |
| Guns/merge preference | More vertical-fight tactics from enemy aces, head-on missile use |

Adaptation is **visible**: the debrief states "Intel: enemy air defences are using ambush doctrine more often." It uses the same generator knobs (force composition biases and doctrine fields) — no hidden stat changes (NN-5). Rate and cap are per campaign difficulty.

## 38.4 Strategic decisions (outline §77)

Before each operation the game presents **3–6 offers**, composed from valid operations plus events:

| Offer | Description (example) | Difficulty | Reward | Risk |
|---|---|---|---|---|
| Attack Northern Airport | Capture a heavily defended hub | High | High strategic value | High losses |
| Attack Southern Airbase | Capture a lightly defended base | Lower | Lower SV | Moderate |
| Defend Current Base | Repel a counterattack | Medium | Protect territory; preserve assets | Base loss if declined |
| Escort Supplies | Convoy to a forward node | Low–Medium | Supplies/credits | Convoy loss |
| Intercept Reinforcements | Strike enemy transport/airlift | Medium | Weakens target node for later capture | Lost chance to heal/repair |
| Recon | Reveal defences | Low | Intel | Exposure |
| Rest & repair | Pass time to repair | none | Airframes repaired | Enemy gains a turn |

**Offer generation.** Valid operations are scored by `appeal = reward_value − k·risk + variety_bonus − recency_penalty`; the final list ensures at least one offensive operation, at least one logistics/defence/recon operation when legal, and never repeats the same type three times consecutively. Offers carry **fog-adjusted** information. The player can always select "Hold/Rest".

## 38.5 Meta-progression: Legacy Points (LP)

```text
LP = 10·nodes_captured + 5·bosses_neutralised + 2·operations_won + 1·aircraft_survival_rate·10 + (50 if victory else 0)
LP *= difficulty_multiplier   (Recruit 0.8, Standard 1.0, Veteran 1.25, Iron 1.6)
```

Unlock catalogue (design targets; costs in LP):

| Category | Examples | Cost |
|---|---|---|
| Aircraft (campaign start rosters) | Additional launch aircraft types | 40–120 |
| Weapons | Extended missile families | 30–80 |
| Cosmetics | Liveries, emblems, callsign sets | 10–30 |
| Faction options | Play as another faction | 100–150 |
| Starting bonuses | +1 airframe, +credits, +intel at start | 25–60 |
| Difficulty/modifiers | Iron Run, Enemy Aces, Grand campaign | 20–80 |

Meta unlocks persist in the profile (IndexedDB + export). They never reduce sandbox access.

## 38.6 Campaign difficulty

Campaign difficulty modifies **economy** (income, prices), **enemy strategic aggression**, **fog** (intel decay rate), **death rules** (Iron: pilot permadeath ends the run), and the **realism dials default** — not enemy stats (§59).

## 38.7 Run end and recap

At victory or defeat the game shows a **run summary**: timeline of captures/losses, bosses met, aircraft lost, LP earned, and a generated chronicle (§43). Players can start a new run with the same seed or a new seed.

## 38.8 Acceptance criteria

- `AC-38.1` Offer generator always yields ≥ 3 distinct legal offers in 10 000 random strategic states (or a "Hold" fallback), with diversity constraints satisfied.
- `AC-38.2` Adaptation counters shift composition weights within the specified bounds and the debrief message is shown when shifts exceed 10 %.
- `AC-38.3` LP computation matches the formula in unit tests and persists across save/load.
- `AC-38.4` Persistent state round-trips (airframes, pilots, nodes, resources) with exact equality.

---

# 39. CAMPAIGN RESOURCES

`P9/P10 · M6` — Resources add meaning to losses without becoming a spreadsheet (outline §73–74). The resource system must remain secondary to flying.

## 39.1 Design rules

1. **At most three numbers matter at a decision point** (Supplies, Credits, Intel); other quantities are derived.
2. **Automation by default:** "Auto-prepare" chooses fuel/ammo and assigns airframes sensibly; manual control is opt-in.
3. **One readiness screen** (no nested economy menus).
4. **Resources create stories**, not busywork: shortages trigger events and decisions (§40).

## 39.2 Resources

| Resource | Use | Sources | Sinks |
|---|---|---|---|
| **Supplies (S)** | Universal logistics: fuel, ammunition, spare parts | Node production, convoys, captured stock, objectives | Sorties, repairs, rearming |
| **Credits (C)** | Procurement of replacement aircraft/upgrades | Objective rewards, node income, captured assets | Buying airframes, upgrades |
| **Intel (I)** | Reveals (§42) | Recon, kills, captured documents, ESM | Reveal actions |
| **Fuel** (derived) | Per sortie | `1 S = 400 kg fuel` | Sortie consumption |
| **Ammunition** (derived) | Weapons | Weapon cost in S (SRM 2, MRM 4, LRM 8, ARM 5, bomb 1–3, rockets pod 3, gun rounds 0.001/round) | Used weapons |
| **Repair capacity** (derived) | Repairs per hour | Node facilities | Repair queue |
| **Aircraft availability** | Which airframes can fly | Pool + procurement + captures | Losses, damage |

## 39.3 Income and costs (design targets)

```text
node_income_per_day = base_type · facility_health · supply_connectivity
    base_type: Major 12, Military 10, Regional 8, Remote 4, Improvised 2   (Supplies/day)
sortie_cost ≈ fuel_kg/400 + Σ weapon_cost + wear(2 S)      typical 15–25 S
starting resources: 120 S, 200 C, 3 I
repair: damaged aircraft 1 S per 1 % component HP restored (min 2 S); heavily damaged 2.5 S per 1 %
procurement: Light fighter 120 C, Multirole 160 C, Heavy 220 C, Bomber 260 C, EW 200 C, AWACS 300 C, Stealth 400 C
```

Targets: a typical campaign supports ~6–10 sorties between crises; losing a hub meaningfully hurts, but a single lost aircraft never stalls a run.

## 39.4 Aircraft states and persistence (outline §74)

| State | Meaning | Flyable | Transition |
|---|---|---|---|
| **Operational** | Within 10 % of full health | Yes | Damage → Damaged |
| **Damaged** | Some components below 70 % | Yes (warnings) | Repair → Operational |
| **Heavily damaged** | Critical components or structural margin reduced | Limited (not in some mission types) | Repair → Operational (long) |
| **Destroyed** | Lost | No | Replace by procurement |
| **Unavailable** | Reassigned/awaiting parts/weather-grounded | No | Time/event |
| **Repairing** | In queue at a base | No | Completes after `T_repair` |

**Repair times** (design): damaged 4–12 h; heavily damaged 24–72 h; shortened with repair capacity; repair needs the aircraft to be at a base with facilities (a damaged airframe landed at a forward strip stays "grounded" until a ferry/repair operation). **Airframe record:** hours, damage history, scars (cosmetic), accumulated structural fatigue carry-over (20 % of in-mission `D_struct` persists, §18.7), modifications.

## 39.5 Supply logistics

Supplies are held per node; sorties draw from the *launching node*. Resupply is by **convoys/airlifts** created by events and operations ("Escort supplies"), or automatically at a transfer rate limited by edge capacity if a safe route exists (auto-transfer rate `min(edge_capacity, 20 % of source stock)` per day, only along owned edges). Cut supply lines (enemy-held or damaged infrastructure) slow transfers, motivating interdiction and defence. The UI shows a single **supply health** indicator per node and highlights bottlenecks; logistics is otherwise automatic.

## 39.6 Readiness screen

One page: airframe list (state, location, repair ETA), supplies bar with projected cost of the selected sortie, "Auto-prepare" button, procurement drawer. Pilot assignment is automatic unless the player opts in (§41).

## 39.7 Edge cases

| Case | Handling |
|---|---|
| Zero supplies | Sorties limited to guns-only/minimum fuel; events offer emergency airlift |
| No operational airframes | Procurement or captured airframes; if none affordable → strategic defeat check (§34.3) |
| Repair queue overflow | Priority by SV of node and player-pinned airframes |
| Captured airframes of foreign type | Usable if the player's faction has the tech flag; else scrapped for S |
| Negative balances | Prevented by validators; debts disallowed |

## 39.8 Acceptance criteria

- `AC-39.1` Economy simulation with an AI player shows sorties-between-crisis within 6–10 (median) on Standard difficulty over 100 campaigns.
- `AC-39.2` The readiness screen shows ≤ 3 primary numbers and "Auto-prepare" completes any sortie setup in one click.
- `AC-39.3` Repair-time and cost formulas match the table in unit tests; persistence of airframe records is lossless.
- `AC-39.4` No state exists with negative resources (property test across 10⁵ random sequences).

---

# 40. CAMPAIGN EVENTS

`P9 · M6` — Events inject surprises and decisions that make the strategic layer feel alive (outline §76–77). Events create **strategic decisions**, not just modifiers.

## 40.1 Event types (outline §76)

| Event | Effect (examples) | Choices |
|---|---|---|
| **Enemy counterattack** | A player node is attacked in N days | Defend (play), auto-resolve, reinforce node (cost S) |
| **Surprise reinforcement** | Enemy stack arrives at a node | Intercept (operation), accept, bypass |
| **Aircraft shortage** | Airframes grounded for parts | Pay C to expedite, cannibalise, rest |
| **Airbase damage** | Raid damages facilities | Repair now (cost S), accept reduced capacity |
| **Intelligence discovery** | Reveal enemy convoy route/SAM site | Strike now (op), hold for combined op, share |
| **Enemy ace sighting** | A named ace operates near a node | Hunt (Boss Hunt op), avoid, set trap |
| **Supply shortage** | Supplies −X or convoy delayed | Emergency escort, ration (limit sortie size) |
| **Weather event** | Storm closes ops for 1–3 days or bad visibility | Wait, fly anyway (risk), shift targets |
| **Friendly reinforcements** | Allied squadron arrives | Assign to node, hold in reserve |
| **Naval fleet movement** | Enemy/allied fleet repositions | Intercept, avoid, request support |

## 40.2 Event definition

```json
{
  "schema": "campaign_event/1", "id": "evt_enemy_convoy_spotted", "weight": 1.0, "cooldown_ops": 6,
  "when": { "all": [ { "op": "enemy_nodes_adjacent", "min": 1 }, { "op": "intel_level_ge", "value": 1 } ] },
  "text_key": "event.convoy_spotted", "tags": ["intel", "logistics"],
  "choices": [
    { "id": "intercept", "text_key": "event.choice.intercept", "effect": [ { "do": "offer_operation", "type": "intercept_reinforcements", "target": "$node", "expires_ops": 2 } ] },
    { "id": "ignore", "text_key": "event.choice.ignore", "effect": [ { "do": "modify_node", "node": "$node", "enemy_strength_pct": 10 } ] },
    { "id": "report", "text_key": "event.choice.report", "cost": { "intel": 0 }, "effect": [ { "do": "grant", "resource": "credits", "amount": 40 } ] } ]
}
```

## 40.3 Event deck and scheduling

- **Deck:** all events whose `when` conditions hold form the current candidate set with weights modified by difficulty, doctrine, adaptation counters, and recent history (cooldowns, anti-repetition).
- **Draw:** after each operation, draw `k` events (`k = 1` normally, 2 with 25 % probability when pressure is low; 0 with 15 %).
- **Tension curve:** a campaign-level scalar `T_c ∈ [0,1]` measures strategic pressure (enemy strength near the player's nodes, resource scarcity, recent losses). Event weights favour **relief** events when `T_c > 0.75` (friendly reinforcements, intel) and **pressure** events when `T_c < 0.3` (counterattack, raid).
- **Decision guarantee:** at least one event with a meaningful choice every two operations.
- **Determinism:** draw uses the `"events"` stream seeded by campaign seed and operation index.

```text
weight' = weight · diff_mod · doctrine_mod · adapt_mod · relief_or_pressure_mod(T_c) · recency_penalty
choose k distinct events by weighted sampling without replacement
```

## 40.4 Event → operation coupling

Events frequently create **operation offers** (Defend X, Intercept Y, Hunt Z) with expiry (ops count). Declining an event with consequences triggers the consequence (auto-resolve). This keeps the loop: *events → decisions → operations → results → events*.

## 40.5 Example events (initial deck, ~24 entries)

Counterattack on forward base; counterattack on HQ (rare); raid on fuel depot; raid on radar; supply convoy ambushed; weather front closes base; fog bank covers airfield; enemy ace sighted (named); defector offers intel; allied squadron offers support (cost); equipment shortage; pilot fatigue; local militia reports; rival faction skirmish; enemy carrier group sighted; submarine sighted (staged); airfield sabotage; recon photo reveals SAM site; ceasefire rumour (reduces enemy aggression for 2 ops but reveals nothing); sandstorm; heavy snowfall; high-value target spotted (bounty); enemy reinforcements delayed; production site discovered.

## 40.6 Edge cases

| Case | Handling |
|---|---|
| Event references a destroyed/changed node | Preconditions re-evaluated at draw time; invalid choices hidden |
| Player never chooses (idle) | Default choice applied after 2 ops (consequence) |
| Conflicting events | Mutual exclusion tags |
| Save compatibility | Events stored by id; unknown ids dropped with log |

## 40.7 Acceptance criteria

- `AC-40.1` 10 000 simulated draws: the distribution of event types matches expected weights ±5 %, no event repeats within its cooldown, and relief/pressure weighting follows `T_c`.
- `AC-40.2` The decision guarantee holds in 99 % of 1 000 simulated campaigns.
- `AC-40.3` All event choices produce valid, applied effects (automated effect-coverage test).

---

# 41. PILOT PROGRESSION

`P9/P10 · M6` — Veteran pilots should feel valuable, but pilot management stays lightweight (outline §75).

## 41.1 Model

Pilots (the player's pilot and AI wingmen in the player's squadrons) have:

| Attribute | Meaning |
|---|---|
| **Experience (XP)** | Gained from operations: `XP = score/20 + 5·kills_air + 3·kills_ground + 10·objective_primary + survival_bonus` |
| **Kills** | Counts by category |
| **Rank** | 1 Recruit, 2 Pilot, 3 Veteran, 4 Flight Leader, 5 Squadron Leader, 6 Wing Leader, 7 Ace (generic ranks; no real military rank insignia) |
| **Traits** | Small bonuses chosen at rank-ups |
| **Survivability** | Ejection survival, injury recovery, KIA risk |
| **Skill row (AI wingmen)** | The AI skill level (§19.6) which improves slowly with XP |

Rank thresholds (XP): 0 / 100 / 300 / 700 / 1 400 / 2 500 / 4 000.

## 41.2 Traits

Traits are intentionally small (±5–10 %) so they never break balance; offered 1-of-3 on each rank-up (player pilot) or auto-assigned by pilot personality (AI):

| Trait | Effect |
|---|---|
| Iron Lungs | +0.5 g G-tolerance |
| Eagle Eye | +10 % visual detection range |
| Steady Hand | −10 % gun aim error (AI) / reduced stick noise when wounded (player) |
| Cool Head | AI wingmen only: −25 % morale loss from events (§19.7.3) |
| Fuel Miser | AI wingmen only: −5 % fuel burn |
| Wingman's Friend | Wingman mutual support probability +0.1 |
| Missile Sense | MAWS/RWR warnings 0.3 s earlier perception (AI) |
| Lucky | +10 % ejection survival |

Traits for the human pilot are restricted to those with player-meaningful, bounded effects (G-tolerance, visual range, ejection survival, small economy perks).

## 41.3 Survivability and injuries

Ejection (§18.8.4) outcomes: *recovered* (over friendly territory or sea rescue chance), *captured* (over enemy territory: pilot unavailable; rescue operation offered), *injured* (unavailable N operations), *KIA*. Probabilities depend on altitude/speed/territory/trait. The player pilot's KIA ends the run only in Iron difficulty; otherwise a **successor** continues at rank −2 with a "memory" (the old pilot appears in the chronicle).

## 41.4 Roster and UI

A single **Roster** page: names, ranks, XP bars, traits, state (available/injured/captured/KIA). **Auto-assignment** by default (best pilot to the most demanding flight); manual assignment optional. Wingman skill progression: each +1 rank adds a small improvement to relevant AI skill parameters (reaction −5 %, aim error −5 %) up to the row of the next skill level.

## 41.5 Edge cases

| Case | Handling |
|---|---|
| No pilots available | Recruit replacements at a cost; at worst AI conscripts (Recruit skill) |
| Pilot captured then rescued | Returns with reduced morale for 1 op |
| Renamed or duplicated names | Unique ids; names from a data table with localised sets |

## 41.6 Acceptance criteria

- `AC-41.1` XP/rank progression follows the formula and thresholds in unit tests.
- `AC-41.2` Trait effects are bounded as specified (automated check on all traits).
- `AC-41.3` Pilot state transitions (recovered/captured/injured/KIA) respect probabilities within ±2 % over 10⁵ trials.
- `AC-41.4` Roster screen requires ≤ 1 interaction to launch with auto-assigned pilots.

---

# 42. FOG OF WAR & INTELLIGENCE

`P9 · M6` — The campaign map uses fog of war (outline §78–79). The player may not know exact enemy strength, aircraft composition, SAM locations or enemy movements; reconnaissance can reveal information. Intelligence also gradually identifies enemy units.

## 42.1 Strategic knowledge model

For every enemy-held node, unit group and installation the player holds **knowledge records**:

```json
{ "target": "node:KBFL", "attributes": {
    "strength_tv": { "estimate": 140, "confidence": 0.5, "age_h": 30 },
    "aircraft_types": { "estimate": ["swift", "condor"], "confidence": 0.7, "age_h": 30 },
    "sam_sites": { "estimate": [ {"pos":[35.4,-119.1], "type":"sam_med"} ], "confidence": 0.9, "age_h": 120 },
    "movements": { "estimate": "convoy_toward:KMRY", "confidence": 0.3, "age_h": 4 } } }
```

**Confidence decay:** `c(t) = c₀ · exp(−t/τ)`, with `τ = 3 days` for mobile forces and `14 days` for fixed installations. **Display:** estimates render as ranges `estimate · (1 ± (1 − c)·0.5)` and icons grey out with decreasing confidence (a `?` below `c < 0.2`).

## 42.2 Sources of intelligence

| Source | Reveals |
|---|---|
| Reconnaissance operations (§27.10) | High-confidence, targeted (installations, defences) |
| Own sensors during sorties | Radar/ESM contacts logged to the knowledge base; emitters geolocated (§17.6.2) become SAM site records |
| AWACS/allied sharing | Contact feeds in range |
| Captured nodes/documents | Bulk reveal of nearby nodes (confidence 0.8) |
| Events | Intelligence discovery, defectors, sightings |
| Intel spending | `5 I` reveal a node's defences at confidence 0.9 |
| Debrief analysis | Kills/identifications update squadron/ace knowledge |

## 42.3 Identification progression (outline §79)

Enemy squadrons and aces progress through knowledge levels:

| Level | Name | What is known | Example |
|---|---|---|---|
| 0 | Unknown | Nothing beyond a contact | "Unknown Squadron" |
| 1 | Seen | Aircraft type observed | "Unidentified Swift-type unit" |
| 2 | Named | Callsign/squadron identified | "Squadron Vulture identified." |
| 3 | Profiled | Doctrine and tendencies | "Vulture operates long-range interceptors." |
| 4 | Dossier | Pilots, loadouts, signature behaviours (bosses) | "Vulture Lead uses terrain masking." |

Level-up triggers: encountering the unit (+1 evidence point), hearing callsigns on radio (+1), visual ID (+1), kill (+2), reconnaissance reveal (+3), intel spending (+3); thresholds per level: 1 / 3 / 6 / 10 evidence points. This builds persistent enemy personalities.

## 42.4 In-mission fog and briefing

In-mission information follows realism (§16.11). The **briefing** uses campaign knowledge: unknown defences appear as uncertainty areas; discovered SAMs appear as exact positions; estimated enemy TV is shown as a range. SAMs discovered in a mission become permanent intel at confidence 0.9. Briefings explicitly mark uncertainty ("Estimated 2–4 fighters").

## 42.5 Fog rendering on the strategic map

- **Coverage map:** union of detection coverage from owned nodes' radars and recent sorties; areas outside coverage are darkened with a soft vignette.
- **Node icons:** unknown (`?`), rumoured (hollow), observed (outlined), confirmed (filled), detailed (filled with glyphs).
- **Timestamps:** hovering shows "last seen N h ago".
- **Estimated vs true:** the UI never displays true enemy numbers beyond the player's knowledge.

## 42.6 Edge cases

| Case | Handling |
|---|---|
| Enemy relocates after recon | Old records age and are flagged "possibly moved" |
| Conflicting reports | Latest high-confidence report wins; others merged |
| Player has perfect info setting (Easy) | `fog = off` option reveals true values |
| Sandbox | Fog replaced by realism dials (§16.11) |

## 42.7 Acceptance criteria

- `AC-42.1` Confidence decay and display ranges match the formulas in unit tests.
- `AC-42.2` Identification progression reaches each level at the stated evidence thresholds in scripted encounters.
- `AC-42.3` The strategic UI never exposes a true enemy value when confidence < 1 (data-flow audit: UI reads only `KnowledgeView`).
- `AC-42.4` Emitters geolocated in a mission appear as SAM records in the campaign after debrief (round-trip test).

---

# 43. EMERGENT STORYTELLING

`P9/P10 · M6` — The campaign should naturally generate stories (outline §80, §149). The narrative comes from system interactions, with no requirement that anything be scripted.

## 43.1 Approach

Three components: the **event stream** (everything notable that happens), **storylets** (pattern detectors that recognise narratively interesting configurations), and the **chronicle** (the player-facing journal and run recap). Optional relationship memory lets characters (wingmen, aces) refer back to shared history.

## 43.2 Event stream

Structured, deterministic records: `{ t, type, actors, location, outcome, tags }`, e.g. `wingman_lost{pilot=Reyes, killer=Vulture1-1, node=KMRY}`, `node_captured{node=KBFL}`, `ace_encounter{ace=Vulture1-1, count=3}`, `landing_emergency{fuel=4%}`. Source: Sim Core events, mission results, campaign changes, events, pilot outcomes.

## 43.3 Storylets

A storylet is a pattern over the stream with a template:

| Storylet | Pattern | Output (example, localised templates) |
|---|---|---|
| **Comeback** | Lost ≥ 2 aircraft defending node X, then captured X's neighbour within 3 ops | "After losing two aircraft at Mercer, the squadron struck back and took Hallow Strip." |
| **Nemesis** | Same enemy ace encountered ≥ 3 times without defeat | "Vulture Lead had met them three times; none had ended it." |
| **Redemption** | Player neutralises the ace that killed a wingman | "For Reyes." |
| **Last stand** | Defended a node with ≤ 1 operational aircraft and won | "Down to one airframe, they held the line at Cedar Ridge." |
| **Lone survivor** | Only survivor of a flight on a mission | |
| **Daring capture** | Captured a node with Hard rules and below-par losses | |
| **Close call** | Landed with < 5 % fuel or damaged aircraft after a crisis | |
| **Hunter becomes hunted** | Went from offensive to defensive within a mission (state flip) | |
| **Bridge cut** | Destroyed a bridge that stalled an enemy advance | |
| **Quiet before** | Lull then major enemy attack | |

Each storylet has **conditions** (pattern), **cooldown**, **priority**, and **text templates** with slots (names, places, numbers). Selection avoids repetition (recency penalties) and weights by importance.

## 43.4 The chronicle

- **Campaign journal:** chronological list with expandable entries and linked locations/replays.
- **Debrief highlights:** top 3 storylets from the mission.
- **Run recap:** an ordered summary generated from the highest-priority storylets, with statistics; shareable as text and an image card.
- Entirely offline and template-based (no online language models); localisable.

## 43.5 Relationship memory and callbacks

The system tracks **bonds** (number of shared operations between the player and each wingman, kills together, rescues) and **grudges** (against aces). The radio system uses these in callback phrases occasionally (≤ 1 per mission, rate-limited): "Lead, this is the same valley where we lost Reyes." Callback phrases are generated from storylet tags and phrasebook entries (no free-form text).

## 43.6 Example derived story (from the outline)

> The player loses two aircraft defending a remote airbase, launches a counterattack from a mountain strip, destroys the enemy SAM network, captures the airport, and later encounters the same elite squadron that attacked them earlier.

Events: `aircraft_lost ×2 @ node_defend`, `op_attack{node=…, type=sead}`, `network_destroyed`, `node_captured`, `ace_encounter{count=2}` → storylets Comeback + Nemesis produce the narrative with no scripting beyond templates.

## 43.7 Acceptance criteria

- `AC-43.1` Replaying the scripted event sequence of §43.6 produces the Comeback and Nemesis entries in the chronicle.
- `AC-43.2` In 100 simulated campaigns each produces ≥ 8 chronicle entries and ≤ 20 % repeated storylet templates.
- `AC-43.3` Callback phrases appear ≤ 1 per mission and only when the referenced event exists (audit).
- `AC-43.4` All chronicle strings are localisation-key based and render without missing slots (lint).

---

# 44. WORLD STREAMING

`P2 · M2` — The world uses multiple levels of streaming; the entire Earth never needs to be loaded at maximum quality (outline §81). In a browser, streaming is the central technical risk: it must be incremental, cancellable, memory-bounded, and invisible to the player.

## 44.1 Intent and constraints

- Seamless zoom from orbit to a runway (one terrain system, §9.9).
- No holes, no pops: coarser data always covers until finer data arrives (§44.5.4).
- Bounded memory and main-thread cost (§0.6.3): ≤ 0.7 ms average, ≤ 2 ms p99 on the render thread.
- Works from a cold cache (network) and fully offline (cache/OPFS/host relay).
- Sim consistency: collision, radar and placement use a **deterministic sim terrain layer** that every participant has (§44.6).

## 44.2 Streaming levels (outline §81)

| Level | Used for | Quadtree levels | Source of data | Typical residency |
|---|---|---|---|---|
| **Global** | Globe, orbit | z0–z5 | Base pack (always local) | Always resident (≈ 68 MB raw / 25 MB packed) |
| **Continental** | Zoomed-out map; far horizon | z4–z8 | Global archive (range requests) | LRU cache |
| **Regional** | Map zoom, horizon ring | z6–z10 | Archive/regional packs | LRU + pinned for battlefield |
| **Battlefield** | Playable area | z9–z12 | Pre-fetched + streamed | Pinned (sim-critical set) |
| **Close** | Ground-level detail | z12–z13 (+ procedural z14+) | Streamed near the aircraft | LRU near the camera |

## 44.3 Quadtree and tile identity

```text
tile (z, x, y):   Δ = 180° / 2^z
  x ∈ [0, 2^(z+1)),  y ∈ [0, 2^z)         y = 0 at the north
  lon_west  = −180° + x·Δ ;  lon_east = lon_west + Δ
  lat_north =  90° − y·Δ ;   lat_south = lat_north − Δ
tile_for(lat, lon, z): x = floor((lon + 180°)/Δ), y = floor((90° − lat)/Δ)       (clamped; x wraps modulo 2^(z+1))
```

Each tile has 129 × 129 height posts (128 intervals, edges shared with neighbours). **Post spacing** (equator, 128 intervals): z5 4 891.97 m; z8 611.50 m; z10 152.87 m; z11 76.44 m; z12 38.22 m; z13 19.11 m. Tile counts: `2^(2z+1)` tiles at level z.

**Per-tile layers (separate byte ranges so they stream independently):**

| Layer | Content | Size (raw, per z12 tile) |
|---|---|---|
| `height` | u16 quantised: `h = h_min + q·(h_max − h_min)/65 535`, per-tile `h_min/h_max` (≈ 0.14 m resolution for a 9 km relief) | 33 282 B |
| `error` | Geometric error ε relative to the next finer level (float) + min/max bounds | 16 B |
| `landcover` | 64 × 64 class indices (u8) | 4 096 B |
| `features` | Vector features: water lines/polygons, roads/rails, buildings, power, aeroway (§47) | variable |
| `airport_patch` | Flatten delta for airports within the tile (§10.7) | variable |
| `albedo` (optional) | Low-frequency colour tint (from base pack, mostly derived at runtime) | optional |

## 44.4 Level-of-detail selection

Screen-space error (SSE) metric: a tile at level `z` with geometric error `ε_z` (metres) at distance `d` (to the closest point of its bounding volume) projects to

```text
ρ = ε_z · W / (2 · d · tan(fov_h / 2))            (pixels)         W = viewport width in pixels, fov_h = horizontal FOV
refine (split) if ρ > τ  (τ = 2 px default; 1.5 px at High, 3 px at Low);  merge if ρ < 0.85 τ  (hysteresis)
```

With `W = 1920`, `τ = 2`, `fov_h = 90°` the constant `K = W/(2τ tan 45°) = 480`, so the **distance at which a level is chosen** is `D_z = ε_z · 480`. Using the design estimate `ε_z ≈ 0.35 × post spacing` (the pipeline bakes the true `ε_z` per tile):

| Level | Post spacing (m) | ε (m) | Selected out to ≈ |
|---|---|---|---|
| z13 | 19.1 | 6.7 | 3.2 km |
| z12 | 38.2 | 13.4 | 6.4 km |
| z11 | 76.4 | 26.8 | 12.8 km |
| z10 | 152.9 | 53.5 | 25.7 km |
| z9 | 305.7 | 107 | 51 km |
| z8 | 611.5 | 214 | 103 km |
| z7 | 1 223 | 428 | 205 km |
| z6 | 2 446 | 856 | 411 km |
| z5 | 4 892 | 1 712 | 822 km |

Consequence: near-ground flight needs z12–z13 only within a few kilometres, which keeps streaming and triangle budgets small. Tiles outside the frustum, beyond the Earth horizon (horizon culling using the bounding sphere vs the horizon plane for the camera height), or behind opaque cloud layers (optional) are culled. A tile's **sub-LOD** may render the shared grid at 128, 64, or 32 intervals depending on distance within its level (reducing triangles 4–16×). **Triangle budget:** ≤ 1.5 M terrain triangles at Recommended, ≤ 0.6 M at Minimum.

For non-nadir views, `d` uses the closest point of the tile's oriented bounding box; very wide FOV or high-DPI scales `K` accordingly. Aspect-aware: the tile's level uses the *larger* ground dimension (E–W is shorter at high latitude; the extra E–W resolution is wasted but harmless, ADR-06).

## 44.5 Streaming pipeline

```text
Camera & flight prediction → Request scheduler → Fetch → Decompress → Decode/Build → Upload → Resident set → Render
                                  ▲                                                       │
                                  └────────────── Eviction (LRU, budgets) ◄──────────────┘
```

### 44.5.1 Request scheduling

Every frame (cheaply, at 10 Hz) the scheduler computes wanted tiles `W` = tiles needed by the LOD selector for the current camera plus a **look-ahead set**: tiles the aircraft will need within 20 s along its velocity vector (and along the planned route in the next 60 s at lower priority). Each wanted-but-missing tile has priority

```text
prio = ρ_potential (error it would remove, pixels) × view_weight (1.0 in frustum, 0.3 behind) × (1 + 2·in_battlefield) × (1 + route_bias)
```

A bounded priority queue limits in-flight requests (default 12 concurrent, 64 queued); stale requests (no longer wanted) are cancelled with `AbortController`.

### 44.5.2 Fetch

Order of resolution: (1) in-memory LRU, (2) OPFS archive/cache (`SyncAccessHandle` in a Stream Worker), (3) network `fetch` with `Range` requests to the `.gpk` archive (reads aligned to 256 KiB chunks for CDN cacheability; adjacent tile requests are coalesced into one range), (4) host relay (multiplayer, §33.4). HTTP/2 or HTTP/3 multiplexing is assumed. Integrity: per-tile CRC32C; mismatch → re-fetch once → quarantine and fall back (§44.7).

### 44.5.3 Decode and upload

Stream Workers decompress (zstd WASM or `DecompressionStream`), unpack u16 heights, build derived data (normals are computed on the GPU in a compute pass; per-tile min/max/bounds; max-mip for LOS, §16.9.2), assemble the tile into a staging buffer, then post to the Render Worker which uploads into a **texture-array atlas** (height `R16Unorm` layers 129×129, normal `RG8` layers, landcover `R8` 64×64). Upload budget: ≤ 2 tiles or 8 MB per frame (adaptive). GPU memory ≈ 70 KB per resident tile: a 3 000-tile resident set ≈ 210 MB.

Meshes: one shared 129×129 grid with three sub-LOD index buffers; vertices displace from the height array layer in the vertex shader. Crack prevention: **skirts** (vertical flaps of depth `max(2ε, 30 m)`) plus edge-vertex morphing toward the coarser neighbour using the neighbour's level difference (`geomorph` factor from SSE blend).

### 44.5.4 Residency invariant (no holes)

For every tile that is rendered, all ancestors down to the base-pack level are resident (the base pack provides z0–z5; ancestors z6–z11 are loaded as part of the pre-fetch or kept by LRU pinning). When a requested child is missing the parent is drawn, optionally upsampled with the procedural detail function (§46.6). Children fade in with a 250 ms geomorph/cross-fade.

### 44.5.5 Eviction and budgets

| Budget | Minimum | Recommended | High |
|---|---|---|---|
| Resident tiles (GPU atlas layers) | 1 200 | 3 000 | 6 000 |
| CPU-side decoded cache | 128 MB | 384 MB | 768 MB |
| OPFS cache (disk) | quota-bound (setting; default 4 GB) | default 10 GB | default 40 GB |
| In-flight requests | 6 | 12 | 24 |

LRU by last-used frame; pinned sets: battlefield sim-critical tiles, base pack, tiles within 3 km of any human aircraft. Eviction never removes an ancestor of a resident descendant.

### 44.5.6 Throughput expectations

Cruising at 300 m/s over land at 500 m AGL, the swath needing z12 detail is ≈ 6 km wide: about 1 new z12 tile column per 16 s per 4.9 km tile ⇒ < 1 new tile/s including neighbours; with landcover and features the sustained rate is well below **3 MB/s** (≈ 24 Mbit/s) even at supersonic low level over dense urban areas, and typically < 1 MB/s. Pre-fetch (§44.9) covers the first minutes.

## 44.6 Sim terrain layer (determinism with streaming)

Because collision, radar masking, placement and AI use the terrain (ADR-15), every participant (including the headless server) MUST evaluate the **same** height function regardless of what the renderer has streamed.

1. **Sim terrain level.** Each GeoPack declares `sim_level` (default **12**, i.e. 38 m posts, matched to 30 m source data; packs with real finer data declare 13). The sim surface is the **bicubic (Catmull–Rom) interpolation** of the `sim_level` posts. Render levels above `sim_level` are either *derived* (bicubic upsample of the same posts — so the render surface and sim surface coincide at the render posts) or *detail* (bounded procedural displacement ≤ 1.5 m, visual only). Hence a render-only tile never changes where the ground is by more than that bound.
2. **Sim-critical set.** The battlefield defines which tiles must be resident at which sim level, deterministically from the `MissionDef`:

| Battlefield | Sim level | Notes |
|---|---|---|
| Small / Medium / Large | z12 everywhere in the playable area (+ 10 km margin) | |
| Huge | z12 in **fine zones** (radius 40 km around every airport, objective, spawn, carrier station, and planned route polyline), z11 elsewhere | |
| Extreme | z12 in fine zones, z10 elsewhere | |

Fine zones and levels are computed by `SimTerrainPlan(mission)` — a pure function of the mission data — so server and clients agree. Clients and servers MUST have the plan's tiles before the countdown (§44.9); a join with missing tiles is blocked or served by host relay (§33.4).
3. **Memory (sim layer).** z12 height tiles (33 KB) over a Large area: 2 704 tiles ≈ 90 MB raw (equator; ×1/cos φ in E–W tile count at latitude φ, e.g. ×1.41 at 45°); Huge ≈ 353 MB raw if fully z12 (fine-zone plan reduces this by ~50–70 %); Extreme fully z12 would be ≈ 1.4 GB (hence z10 outside fine zones).
4. **Query cost.** Bicubic height lookup in Rust/WASM ≈ 40 ns; tile table lookup via a hash from `(z,x,y)`; batched; the max-mip pyramid is built on load.
5. **Airports.** Flattened patches (§10.7) are part of the z12/z13 sim data (`airport_patch` layer applied during pipeline build, not at runtime).

## 44.7 Failure handling

| Failure | Behaviour |
|---|---|
| Tile 404 / network error | Retry with backoff (0.5, 1, 2, 4 s); meanwhile ancestor drawn; after 3 failures mark `missing` and use fallback (§46.9) |
| Corrupt tile (CRC mismatch) | Re-fetch once; then quarantine and fallback |
| Quota exceeded | Evict LRU beyond pinned sets; if still insufficient, stream memory-only and show "cache full" |
| Out of memory | Reduce resident budget by 25 % (and tell the Performance Governor, §30.4); never crash |
| Offline with missing region | Fallback + banner "synthetic terrain / region not installed" with a download button |
| Worker crash | Restart Stream Worker; re-queue |

## 44.8 Precision: relative-to-eye rendering (ADR-03)

The sim and camera are `f64`. Per frame the CPU computes for every drawn tile `offset = tile_origin_BF − camera_BF` in `f64`, casts to `f32`, and uploads in a per-instance buffer; vertex positions are stored tile-local (`f32`, within ±1.3 km at z12/z13 ⇒ ulp ≈ 0.1 mm). Offsets up to 500 km have `f32` ulp ≈ 0.03 m but apply only to distant tiles (error below a pixel). The view matrix is built from the camera orientation only (translation removed). Depth uses **reverse-Z with an infinite far plane** and a 32-bit float depth buffer (near plane 0.3 m in the cockpit, 5 m in external views), giving uniform depth precision from the cockpit to the horizon; the globe/orbit view uses a larger near plane.

## 44.9 Pre-fetch and loading

Loading a mission runs the **Load Plan** (shown as a progress bar with MB and tile counts):

1. Resolve the sim-critical set (§44.6) and the **render must-have set**: all tiles z ≤ 10 covering the battlefield + horizon ring to z ≤ 9; z11–z13 within 15 km of spawns, airports, objectives.
2. Fetch in priority order: spawn area first (so the player can start), then objectives, then the rest. The mission can start when (a) sim-critical tiles in the spawn fine-zone and z ≤ 10 everywhere are resident and (b) `T_est_remaining < T_launch_buffer` (the remaining downloads will finish before the player flies there), or the player chooses "Start now".
3. Estimated sizes (raw, equator; compressed ≈ one third — design estimate):

| Battlefield (edge) | z10 tiles | z11 | z12 | Raw z≤12 |
|---|---|---|---|---|
| Large (250 km) | 169 | 676 | 2 704 | ≈ 118 MB |
| Huge (500 km) | 625 | 2 500 | 10 609 | ≈ 450 MB (reduced by fine-zone plan to ≈ 200 MB) |
| Medium (120 km) | 49 | 169 | 625 | ≈ 28 MB |
| Small (50 km) | 9 | 36 | 121 | ≈ 5.5 MB |

(Counts: tiles needed to cover the area at each level on the equator; ×~1.41 at 45° N/S.) The ≈ 120 MB raw / 40–50 MB compressed figure for Large matches §0.6.6.

4. In multiplayer all participants run the Load Plan; the lobby shows per-player readiness (§32.3).

## 44.10 Concurrency and threading

Fetch/decode run in 2–4 Stream Workers (one per ~2 cores up to 4); the Render Worker owns GPU uploads; the Sim Worker owns the sim-terrain tile table (read-only views into SharedArrayBuffers filled by Stream Workers), so sim queries never wait on the render thread. Without cross-origin isolation (single-worker fallback), the Sim Worker receives copies by `postMessage` transfer and the budgets in §0.6.2 halve.

## 44.11 Edge cases

| Case | Handling |
|---|---|
| Antimeridian | `x` wraps; neighbour lookup uses modular arithmetic; the camera-relative RTE avoids numerical seams |
| Poles | Tile widths shrink; neighbour stitching at the pole row uses a pole-cap tile (single vertex ring collapsed); limited to ±85° (§45.8) |
| Very fast time-acceleration | Look-ahead set scales with speed (≤ 60 s horizon); streaming may lag; renderer holds coarser tiles |
| Ocean tiles | Constant-height (sea level) tiles are stored as a 4-byte marker (no payload) except where bathymetry matters (z≤8 for sonar-lite) |
| Teleport (debug) | Sets a priority burst; shows a loading veil if sim tiles are missing |

## 44.12 Acceptance criteria

- `AC-44.1` Flying a scripted 20-minute supersonic low-level route over rugged terrain with a cold cache never shows holes (residency invariant verified every frame) and the render-thread streaming cost stays ≤ 0.7 ms avg / 2 ms p99 on the Recommended profile.
- `AC-44.2` LOD ring distances for the test camera (1920 px, 90° FOV, τ = 2 px) match the table in §44.4 within 5 %.
- `AC-44.3` Height queries are bit-identical (as `f32`) across WASM, native and two browsers for 10⁶ random points given the same GeoPack (determinism of the sim terrain layer).
- `AC-44.4` Residency under a 10-minute globe-to-ground zoom/flight test stays within the budgets of §44.5.5 and the GPU atlas never exceeds its capacity.
- `AC-44.5` Offline mode: with the Load Plan fully cached, a mission loads with zero network requests (verified by network blocking).
- `AC-44.6` A tile corrupted on disk is detected and replaced via fallback within 2 s without visual holes.

---

# 45. GEOGRAPHIC DATA

`P2 · M2` — The geographic system represents latitude, longitude, elevation, terrain, coastlines, roads, settlements, airports, and bodies of water (outline §82). The world uses geographic coordinates; the active battlefield uses a local coordinate system (BF) to avoid floating-point problems (§7.4). The implementation must respect the licensing and usage requirements of each data provider (outline §124).

## 45.1 Intent

- Real data wherever possible, graceful synthesis where not.
- Open, redistributable data only (ADR-12) so the game can cache, relay and bundle subsets legally.
- Offline-capable after caching (outline §123).
- A reproducible, versioned build pipeline.

## 45.2 Data layers and sources (defaults)

Licence statements are summaries for planning; **implementers MUST verify the current terms of each dataset before shipping** (tracked in Appendix H and per-provider descriptors, §45.9).

| Layer | Default source | Native resolution | Licence (summary) | Notes |
|---|---|---|---|---|
| Elevation (land) | Copernicus DEM GLO-30 (fallback: SRTM 1″ between ~60° N–56° S, public domain) | 30 m | Free with attribution/notice | Global land coverage; EGM2008 heights |
| Elevation (ocean/bathymetry) | GEBCO global grid | 15 arc-seconds (~460 m) | Free use with attribution | Land/sea merge in pipeline |
| Land cover | ESA WorldCover | 10 m | CC BY 4.0 | Coverage excludes the highest latitudes [verify]; fallback heuristics |
| Vector features | OpenStreetMap (roads, rail, water, buildings, aeroway, power, landuse); Overture Maps as alternative/complement | vector | ODbL 1.0 (OSM): attribution + share-alike for derived databases | Packed feature tiles are a *Derived Database*: published under ODbL |
| Airports | OurAirports (airports, runways) | point/line | Public domain | Merged with OSM aeroway |
| Place names / search | GeoNames; Natural Earth (populated places, physical regions, marine areas) | point/polygon | GeoNames CC BY 4.0; Natural Earth public domain | Offline FTS index (§9.5) |
| Climate | ERA5-derived monthly climatology (1° grid) | 1° | Copernicus licence (free, attribution) [verify] | §24.2 |
| Magnetic declination | World Magnetic Model (WMM) | model | Public domain | HUD true/magnetic |
| Globe albedo | NASA Blue Marble-class imagery | 500 m–1 km | Public domain (NASA) [verify] | Globe base colour |
| Night lights | NASA Black Marble-class | 500 m | Public domain (NASA) [verify] | Globe night side |
| Stars | Yale Bright Star / Hipparcos-derived | catalogue | Public/ESA terms [verify] | §24.6 |
| Imagery (optional) | **None by default.** Optional user-configured provider through the provider abstraction (e.g. Sentinel-2-based layers, subject to their terms) | 10 m+ | Per provider | Cached only if the provider permits (§45.9) |

**Not used:** Google Earth / Google Maps imagery or terrain (cannot be cached/redistributed offline under their terms), and any dataset whose licence forbids caching or redistribution within a game package.

## 45.3 Why the Earth cannot be bundled; delivery model

Data volume arithmetic (computed from the quadtree of ADR-06 at 33 282 B per 129×129 `u16` height tile, all tiles including ocean):

| Level | Tiles (global) | Raw height data |
|---|---|---|
| z5 | 2 048 | 68 MB |
| z6 | 8 192 | 273 MB |
| z7 | 32 768 | 1.09 GB |
| z8 | 131 072 | 4.4 GB |
| z9 | 524 288 | 17.4 GB |
| z10 | 2 097 152 | 69.8 GB |
| z11 | 8 388 608 | 279 GB |
| z12 | 33 554 432 | 1.12 TB |
| z13 | 134 217 728 | 4.5 TB |

Land is ~29 % of the surface, ocean tiles beyond z8 are mostly constant, and heights compress ≈ 3:1 (delta + zstd), so z12 for the whole Earth is of order **100 GB** compressed — impossible to ship, feasible to *host*. (For reference: the 30 m land DEM is ≈ 1.65×10¹¹ pixels ≈ 331 GB as 16-bit raw; GEBCO 15″ is 3.73×10⁹ cells ≈ 7.5 GB.)

Therefore (ADR-12):

1. **Base pack (shipped with the app, ≤ 120 MB):** terrain z0–z5, coastlines, airport and place indexes, low-resolution albedo/night lights/stars, climate grid.
2. **Global archive (hosted):** a single immutable `world.gpk` per data version on a static host/CDN with HTTP range support; clients fetch only the tiles they need and cache them locally.
3. **Region downloads (optional):** 5° × 5° cells (≈ 556 km at the equator) containing z6–z12 land tiles, features and airports: roughly 40–150 MB compressed per cell (varies with land fraction and relief); a Large battlefield spans 1–4 cells.
4. **Sharing:** exported `.gpk` subsets and host-to-guest relay (§33.4) let a LAN with one internet-connected machine serve everyone.

## 45.4 Offline readiness and cache management (outline §124)

Settings and UI ("Data & Storage"):

| Setting | Options |
|---|---|
| Cache size limit | Automatic (≤ 50 % of reported quota, capped by profile) or fixed 1–200 GB |
| Region management | Map view of cached coverage (by level), per-region download/delete, pin regions (never evicted) |
| Quality | Low (z ≤ 10), Standard (z ≤ 12), High (z ≤ 13 + all features) |
| Automatic caching | Cache what you fly (default on), pre-cache around bookmarks, background update when idle |
| Network use | Allow on metered connections (default off), bandwidth cap |
| Persistence | Request `navigator.storage.persist()`; show whether granted [verify]; warn about eviction risk, offer export |
| Integrity | Verify all (checks hashes), repair |

The **offline readiness meter** on the battlefield preview (§9.7) shows `cached tiles / required tiles` for the sim-critical set and render must-have set and the MB to download. Browsers can evict origin storage under pressure (and some apply time-based eviction [verify]); hence persistence requests, user-visible status, and the export/import path.

## 45.5 Data pipeline (`geo-pipeline`)

An offline, reproducible build (Rust + GDAL-class libraries) that produces a versioned **GeoPack** (`geo-YYYY.MM`):

1. **Ingest** source tiles; record provenance (URL, date, hash).
2. **Normalise:** reproject to WGS84 geographic; fill voids (interpolation / secondary DEM / sea level); convert vertical datums (§45.6); merge bathymetry (coast blend, shoreline from WorldCover water + DEM zero-crossing).
3. **Water flattening:** lake and reservoir polygons set to their local level; rivers left to DEM.
4. **Tile:** resample to the quadtree (area-average with min/max retention; bicubic when upsampling); compute per-tile `error ε_z` (max abs difference to the next finer level), `h_min/h_max`, and the max-mip block maxima for LOS.
5. **Airport patches:** apply flattening deltas (§10.7) and bake them into z12/z13 tiles; record patch ids.
6. **Quantise and pack:** u16 heights, delta-coded across rows, zstd; landcover class maps; features.
7. **Features:** extract OSM/Overture layers; simplify per level (Douglas–Peucker with tolerance by level); quantise to tile-local 16-bit coordinates; building heights from `height`/`building:levels × 3 m` or class defaults; produce the road graph for ground AI (node/edge arrays).
8. **Indexes:** airports (merged OurAirports/OSM), place-name FTS DB, climate grid, WMM coefficients.
9. **Archive:** write `world.gpk` (below) plus region cell manifests; compute SHA-256 manifest; produce coverage and QA reports (void %, ε histograms, missing tiles, licence audit).
10. **Publish:** upload immutable versioned URLs; update the `datapack.json` manifest (latest version, compatible game versions).

### GeoPack archive format (`.gpk`)

```text
Header (fixed 128 bytes):  magic "GPK1" | version | header_json_len | root_dir_offset | root_dir_len | leaf_dirs_offset | leaf_dirs_len | tile_data_offset | tile_data_len | flags
Header JSON:   { datapack: "geo-2026.09", scheme: "geodetic_quadtree", tile_posts: 129, sim_level: 12, max_level: 13, layers: [...], bounds, licenses: [...], hashes }
Directories:   two-level PMTiles-style: root directory lists leaf directory ranges; leaf entries are
               { tile_id: u64, offset: u48, length: u32, run_length: u16 }  sorted by tile_id; contiguous identical tiles use run_length
tile_id:       prefix-sum offset of level z + Morton (Z-order) index of (x, y) within the level → spatially clustered for range requests
Tile data:     per tile per layer blob (zstd), each with CRC32C trailer
```

The format is intentionally close to the open PMTiles v3 design so existing tooling ideas apply; the game's reader reads the root directory (~16 KB) once, caches leaf directories, and issues coalesced range requests.

## 45.6 Vertical datum normalisation (ADR-05)

All heights are treated as MSL heights directly on the ellipsoid. Sources use slightly different geoid models (e.g. EGM96 vs EGM2008); the pipeline records each dataset's datum and converts to a single reference (EGM2008-based) with a geoid-difference grid so seams between datasets do not create steps. Typical differences are on the order of a metre or less and are removed rather than ignored where datasets abut. The runtime never applies a geoid correction.

## 45.7 Query services and indexes

| Service | Implementation | Cost |
|---|---|---|
| `terrain_height`, `terrain_normal` | §44.6 | ≈ 40 ns |
| `surface_class` | Landcover map lookup (64 × 64 per z12 tile) | ≈ 20 ns |
| `nearest_airport(p, k)` | Grid index (0.5°) over ~70 000 records, then exact distance | < 50 µs |
| `roads_near(p, r)` | Feature-tile R-tree (per tile) | < 0.2 ms |
| `biome(lat, lon)` | Landcover + climate + relief rules (§46.3) | ≈ 50 ns |
| `search(text)` | SQLite FTS5 (WASM, OPFS) | ≤ 50 ms |
| `GeoInspector(lat, lon)` | Aggregates the above | for the debug click tool |

**World debugging (outline §121):** `GeoInspector` returns `{ lat, lon, elevation_msl, biome, terrain_source (source/derived/procedural/synthetic), nearest_airport, distance_to_airport, streaming_level (z) }` and drives the debug overlay (§63.7).

## 45.8 Latitude limits

At launch the playable latitude range is **±85°** (anchor and flight region). Reasons: tile geometry degenerates toward the poles (E–W post spacing → 0), WorldCover and OSM coverage thin out at high latitudes, and heading/true-north becomes ill-defined. Within the supported range, tiles near ±85° oversample E–W by 1/cos 85° ≈ 11.5× (accepted). Battlefields whose anchor lies beyond ±85° are rejected by the setup UI; flights may cross into the polar caps at coarse (z ≤ 8) terrain with the grid-heading HUD (§7.8). Polar support is a post-launch enhancement (e.g. dedicated polar tile schemes).

## 45.9 Licensing and attribution

Every data source has a **provider descriptor** (machine-readable) enforced at build and run time:

```json
{ "id": "osm", "name": "OpenStreetMap", "licence": "ODbL-1.0", "attribution": "© OpenStreetMap contributors", "url": "https://www.openstreetmap.org/copyright",
  "commercial_ok": true, "cache_allowed": true, "redistribution_allowed": true, "share_alike": true, "terms_checked": "2026-10-07",
  "required_notices": ["Contains information from OpenStreetMap, made available under the ODbL"] }
```

Rules:

- The in-game **Credits & Data** screen is generated from descriptors; required notices are always visible in the credits and the data screen; a short attribution line is shown on the world map footer.
- ODbL (OSM) obligations: attribution; share-alike for a *Derived Database* (our packed feature tiles must be published under ODbL; the game code and rendered "Produced Works" are not affected). The pipeline emits a public ODbL data dump alongside each datapack.
- CC BY sources (WorldCover, GeoNames, ERA5-derived): attribution notices.
- **Cache policy:** a provider with `cache_allowed: false` can only be used memory-resident for the session; the cache layer enforces this by flag (no OPFS write, no export).
- **User-supplied providers** (e.g. imagery with the user's own API key) are off by default, clearly marked, and obey their descriptors.
- The release owner (OQ-04) audits descriptors before each datapack release; a CI gate fails the pack if any descriptor is missing or `terms_checked` is older than 12 months.

## 45.10 Error handling (outline §122)

Incomplete geographic information must not crash the game. The failure matrix below is the contract for the whole product (cross-referenced by §61.9):

| Failure | Behaviour |
|---|---|
| Missing terrain | Ancestor tile + procedural detail; then synthetic terrain (§46.9); banner |
| Unavailable geographic data (offline, region absent) | Same, with "download region" action |
| Incomplete airport data | Procedural completion (§10.6); missing runway → improvised airfield (§10.8) |
| Network failure | Retry/backoff; continue from cache; no gameplay interruption |
| Missing assets | Placeholder meshes/textures; telemetry log; mission continues |
| Invalid mission generation | Retry with next sub-stream; relax constraints; actionable error (§26.4) |
| Insufficient memory | Reduce caps/profile (governor), warn, never crash |
| Server disconnect | §33.7 |
| Corrupted save / cache | Detect by checksum; restore backup/quarantine (§64.5) |

## 45.11 Versioning and updates

`datapack.json` lists the latest datapack and the game versions it supports. A new datapack downloads *lazily* (tiles are fetched from the new archive on demand); old cached tiles remain valid for missions pinned to the old provenance (seed reproducibility, §26.3); old data are evicted LRU after the new pack covers the cached regions. Mission seeds state the datapack; mismatches warn.

## 45.12 Acceptance criteria

- `AC-45.1` The pipeline reproduces bit-identical GeoPacks from the same inputs (hash equality), and its QA report lists 0 unfilled voids in supported latitudes.
- `AC-45.2` The `.gpk` reader returns identical bytes for 10⁵ random tile requests via (a) full-file read, (b) range requests, (c) OPFS-cached read.
- `AC-45.3` Data volume table (§45.3) verified by the pipeline's counters (tile counts and raw sizes).
- `AC-45.4` Licence audit: every layer in a shipped GeoPack maps to a descriptor with `terms_checked` ≤ 12 months old; the credits screen lists all required notices (automated).
- `AC-45.5` Failure matrix (§45.10): injected faults (404s, corrupted tiles, quota exhaustion, missing airports) never crash the app and produce the specified behaviour (fault-injection test suite).
- `AC-45.6` Offline: with a region cached, all geographic services function with the network disabled.
- `AC-45.7` `GeoInspector` returns consistent fields for 1 000 random clicks and matches the biome rules of §46.3.

---

# 46. TERRAIN & BIOMES

`P2 · M2` — Terrain must be recognisable from a cockpit at 10 km altitude and believable at 30 m. It is a layered system: real elevation → landcover/biome classification → shading and materials → procedural micro-detail → vegetation, water and surface-dependent gameplay (outline §83).

## 46.1 Intent and layers

| Layer | Provides | Feeds |
|---|---|---|
| Elevation | Shape, slopes, masking | Collision, radar/IR LOS, flight (ground effect), AI pathing |
| Landcover | Class per 10 m cell (aggregated to 64 × 64 per z12 tile) | Biome, materials, vegetation, friction, concealment |
| Climate | Temperature, precipitation, snow line | Biome tint, snow, weather defaults (§24) |
| Features | Rivers, lakes, coast, roads, buildings (§47) | Navigation, targets, rendering |
| Procedural detail | Micro-relief ≤ 1.5 m, texture detail | Visual only |

## 46.2 Landcover classes

Landcover is reduced from ESA WorldCover classes to a stable internal set (the pipeline holds the mapping table, so source changes do not alter game data):

| ID | Class | Albedo (sRGB, summer) | Roughness length z₀ (m) | Tree height (m) | Ground friction μ (braking) | Concealment (IR/visual) |
|---|---|---|---|---|---|---|
| 0 | Water (ocean) | — | 0.0002 | 0 | n/a | 0 |
| 1 | Water (inland) | — | 0.0002 | 0 | n/a | 0 |
| 2 | Tree cover (broadleaf) | (52, 72, 38) | 1.0 | 18 | 0.35 | 0.8 |
| 3 | Tree cover (needleleaf) | (36, 58, 38) | 1.0 | 20 | 0.35 | 0.85 |
| 4 | Shrubland | (112, 108, 78) | 0.25 | 2 | 0.45 | 0.4 |
| 5 | Grassland | (110, 128, 70) | 0.05 | 0 | 0.45 | 0.15 |
| 6 | Cropland | (140, 130, 80) | 0.08 | 0 | 0.45 | 0.15 |
| 7 | Built-up | (120, 118, 112) | 1.5 | — | 0.7 (paved) | 0.3 |
| 8 | Bare / sparse | (150, 128, 100) | 0.01 | 0 | 0.4 | 0.05 |
| 9 | Snow / ice | (235, 238, 245) | 0.002 | 0 | 0.15 | 0 |
| 10 | Wetland / mangrove | (70, 94, 70) | 0.3 | 4 | 0.2 | 0.4 |
| 11 | Moss / tundra | (118, 120, 92) | 0.03 | 0 | 0.35 | 0.1 |
| 12 | Sand / desert | (198, 170, 120) | 0.001 | 0 | 0.35 | 0.05 |
| 13 | Rock / scree | (122, 114, 106) | 0.02 | 0 | 0.5 | 0.1 |

(Albedo values are art starting points; the final palette is tuned by the look-development gate, §66.)

## 46.3 Biome rules

A **biome** is a derived label used by rendering, flora, weather defaults, animals/ambient audio and the `GeoInspector`. It is a deterministic function of landcover, climate and relief:

```text
inputs:  lc (landcover id), T_ann (mean annual temp °C), T_cold (coldest-month °C), P_ann (mm), elev (m), slope (deg), lat, dist_coast (km)
rules (first match wins):
  lc ∈ {0,1}                                        → OCEAN / LAKE
  lc = 9 or (elev > snowline(lat))                  → GLACIER / ICE
  lc = 11 or (T_ann < −2 and lc ∈ {5,4,8})          → TUNDRA
  lc = 12 or (P_ann < 250 and lc ∈ {8,5,4})         → DESERT
  lc ∈ {2,3} and T_cold < −10                       → BOREAL FOREST
  lc ∈ {2,3} and T_ann > 20 and P_ann > 1500        → TROPICAL RAINFOREST
  lc ∈ {2,3} and T_ann > 18 and P_ann ≤ 1500        → TROPICAL / DRY FOREST
  lc ∈ {2,3}                                        → TEMPERATE FOREST
  lc ∈ {4,5} and T_ann > 18 and P_ann in [500,1500] → SAVANNA
  lc ∈ {4}                                          → SHRUBLAND / CHAPARRAL
  lc ∈ {5,6}                                        → GRASSLAND / FARMLAND
  lc = 10                                           → WETLAND
  lc = 13 or slope > 35°                            → ALPINE / ROCK
  lc = 7                                            → URBAN
snowline(lat) = 5 200 m − 55 m·|lat|  for |lat| ≤ 60° ; then linear to 0 m at |lat| = 80°        (rough; modulated by season, §24.2)
```

Thresholds are data-driven (`biomes.json`, §62). Example check vector (for AC-45.7): `lc=3, T_cold=−18, T_ann=−1, P_ann=500` ⇒ BOREAL FOREST; `lc=12` ⇒ DESERT irrespective of the rest.

## 46.4 Seasonal and diurnal variation

Biome appearance changes with season (northern/southern hemisphere handled by sign of latitude) and time: deciduous canopy tint blends summer → autumn → bare (using monthly climatology); snow cover is probabilistic by `(T_month, elev, lat)` and the weather history (§24.3); crops vary by hue; water colour varies with depth/turbidity. All derive from the sim date, so a mission seed + date fully determines the look (visual only; no gameplay dependence except snow friction and concealment, which use the same deterministic function).

## 46.5 Terrain shading and materials

- **Shading.** Normals computed from height in a GPU compute pass at tile upload (central differences on the 129² grid; for finer lighting, a second normal derived from the procedural detail). Lighting: directional sun + sky irradiance (SH from the atmosphere model, §24.7) + ambient occlusion from a coarse height-based horizon map (≤ 8 directions, baked per tile at z ≤ 12) + cascaded shadow maps near the camera (§60.4).
- **Texturing.** No source imagery by default: albedo is `palette(class, season) × detail_texture(class)` where detail textures are tiling PBR sets (one per class, 512² BC7/ASTC, texture arrays) blended by the landcover map with a 2-tap noise-perturbed boundary to avoid grid-aligned edges. Triplanar sampling on slopes > 45°. Far terrain uses the same function evaluated per pixel from the landcover mip chain (no separate low-res albedo).
- **Slope-driven rock and snow overrides** (altitude and slope thresholds from `biomes.json`) to add structure to coarse landcover.
- **Atmospheric perspective** is applied through the aerial-perspective LUT (§24.7); haze colour uses weather visibility.
- **Wetness/Snow** parameters from weather change roughness and albedo (§24).

## 46.6 Procedural detail and upsampling

Where source data are coarser than needed (z13+ on 30 m data, or any missing finer tile), the renderer applies **detail synthesis**:

```text
h_render(p) = h_bicubic(p) + A(class, slope) · fbm(p; octaves = 4, base wavelength 60 m, lacunarity 2, gain 0.5) · mask(distance, level)
A ∈ [0.2 m (cropland) … 1.5 m (rock)] ;  mask → 0 at the tile's native post spacing so the detail never contradicts source data
```

The noise is integer-hash based (bit-reproducible, §24.8.3) and anchored to world coordinates (stable across tile boundaries and sessions). Sim uses only the bicubic surface (§44.6), so detail is within ≤ 1.5 m of it and never affects gameplay except gear contact at very low levels, which uses the sim height + a ground-effect-free margin of 0.5 m.

## 46.7 Vegetation and ground cover

| Item | Method | Budget (Recommended) |
|---|---|---|
| Forests (far) | Per-tile density-textured impostor sheets and billboard clouds in the canopy shader (height-field extrusion) | ≤ 0.4 ms |
| Forests (mid, ≤ 3 km) | GPU-instanced tree models in 3 LODs (mesh → cross billboard → impostor), placed by deterministic per-tile hashing from landcover density | ≤ 120 k instances |
| Grass/crops (near, ≤ 300 m) | Geometry strips or shaded texture, only drawn below 800 m AGL | ≤ 0.3 ms |
| Rocks, shrubs | Instanced, 2 LODs | ≤ 40 k instances |
| Collision with trees | Sim treats forest as a **canopy height** layer: terrain + tree height × canopy mask (collision with trunks ignored; entering canopy at speed = crash per §18.8) | negligible |

Tree placement is a pure function of `(tile_id, cell)` so all clients see the same trees, enabling trees as cover/hazards consistently. Vegetation is optional at Minimum profile (replaced by shaded canopy texture).

## 46.8 Water

- **Ocean.** Heightfield-free: analytic planar water in BF with a displaced mesh patch near the camera: sum of 8–32 Gerstner waves (wind-driven spectrum, JONSWAP-inspired amplitude from sea state, §24.4), foam from wave-crest Jacobian, depth-based colour (Beer–Lambert absorption: `T = exp(−k_λ d)` with `k = (0.45, 0.07, 0.04) m⁻¹` for R, G, B in clear deep water), Fresnel reflection of the sky model, sun glitter, planar/SSR hybrid reflections (SSR + sky fallback). Far ocean shading uses the same BRDF on a normal map from a FFT-free cascaded tiling.
- **Sea state.** Beaufort 0–9 drives wave height `H_s ≈ 0.0213 v_w² / g'` (fully developed sea approximation with `v_w` wind m/s) which carriers and ships consume (§22.6).
- **Lakes and rivers.** Lakes are flat at their stored level; rivers are textured strips along feature polylines with flow direction and width by Strahler order; very narrow rivers are drawn only at z ≥ 11.
- **Coasts.** Shoreline from the land/sea mask with foam band and shallow-water colour ramp from bathymetry.
- **Gameplay surface:** water is non-landable except for seaplane types (post-launch), has no radar shadow, and has sea-clutter models for radar at low grazing angles (§16.8).

## 46.9 Missing-data fallback: synthetic terrain

If neither source data nor ancestors exist for a region (offline without a base-pack-level hole, or corrupted data), the game shows **synthetic terrain**:

```text
h_syn(lat, lon) = h_base(z5 interpolation) + fbm_ridged(lat, lon; wavelengths 40 km→300 m; amplitude scaled by |slope of z5| and biome)
```

Its seed is derived from the datapack + coordinates, so the same place always looks the same. Gameplay works normally (sim layer uses the same function for that region — deterministic), the world map shows a "synthetic" hatching, and mission difficulty modifiers disable "real-world navigation" claims. A banner offers the download.

## 46.10 Terrain features that matter for gameplay

| Feature | Representation | Gameplay effect |
|---|---|---|
| Valleys and ridges | Heightfield | Terrain masking (radar, SAM, visual), terrain-following routes, mountain wave/turbulence zones (§24.4) |
| Canyons narrower than posts | Sub-post detail absent (limit; accepted) | Not flyable as canyons; documented |
| Mountains | z12+ with slope-based detail | Orographic lift/rotor, icing levels |
| Coastlines | Land/sea mask | Radar clutter, sea-breeze weather cell, carrier operations |
| Roads, rails | Feature polylines | Road-runway improvisation (§10.8), convoys (§21), navigation cues |
| Bridges | Feature + model; collision proxy | Strike targets |
| Dams, power plants | Feature objects | Infrastructure strategic targets |

## 46.11 Geography as gameplay (outline §139)

Terrain is an actual gameplay system, not a decorative background. Three simulation layers make it so: (1) **line-of-sight and masking** computed from the sim terrain (ADR-15; radar, IR, visual, comms), (2) **surface properties** (friction, concealment, canopy height, infrastructure density), and (3) **climate-driven conditions** (temperature, density altitude, visibility, icing, turbulence). The mission generator reads the *terrain character* of the battlefield (below) and chooses templates, placements and twists accordingly (§26.7, §69.4).

| Terrain | Gameplay effects (simulation) | Generator and AI behaviour | Player tactics enabled |
|---|---|---|---|
| **Mountains** | **Radar masking** (ridges block LOS; shadow zones behind ridges, §16.9); **valleys** as low-level corridors and ambush lanes; orographic lift/rotor turbulence and mountain wave (§24.4); reduced air density at high airfields (thrust and lift scale with ρ, §11.3); higher stall speeds in TAS; limited visual horizons; sharp IR/visual occlusion | SAMs and early-warning radars sited on high ground with valley-shaped coverage holes; `ambush_valley` twist; objectives in basins; AI uses terrain masking at skill-dependent minimum AGL (§19.6); tankers and AWACS orbit away from ridges | Terrain-masked ingress, break radar contact behind peaks (notch + mask), emerge on an enemy's beam, valley escapes; hazard: controlled flight into terrain |
| **Desert** | **Long sight lines** (visual detection at 30–60 km on clear days, Koschmieder with visibility > 50 km, §16.7); **limited concealment** (sand/rock concealment 0.05, §46.2); heat-driven low-level thermals and turbulence by day; dust storms (visibility collapse, `weather_turn`); hot-day density altitude penalty (take-off roll +20–40 % at 45 °C, 1 000 m elevation); hot ground clutter degrades low-altitude IR contrast | Open-field engagements, long-range BVR, wide dispersal of ground assets; mobile SAM shoot-and-scoot; few masking options so standoff weapons and EW matter | Early detection and long-range shots; low altitude offers little protection; thermal-aware flying |
| **Forest** | **Terrain obstruction**: canopy height layer (collision and masking of low targets from ground sensors, §46.7); high concealment of ground units (0.8–0.85 against visual/IR, so detection range is scaled and strikes need recon); wildfire/smoke from strikes persists | Hidden ground targets (armour, AAA) requiring recon or EO/IR sensors; CAS missions with concealed positions; AAA in clearings | Low-level approach along forest edges; IR/EO targeting pods, forward air control cues |
| **Cities** | **Dense infrastructure**: tall buildings (collision, LOS occlusion at low level, §47), civilians and collateral rules (ROE, protected buildings, §47.6), power lines hazards (§47.5), strong urban radar clutter, night lights and blackout states | Protected zones generated around dense areas; point-defence clusters; strategic targets (power, bridges, ports) with collateral-aware scoring (§25.11); `civilian_airliner` twist near large airports | Precision weapons, standoff strikes, careful ROE; low-level urban flying is deadly and detectable |
| **Ocean** | **Long-range combat**: no terrain masking, radar horizon dominates (`4/3` Earth, §16.9: two aircraft at 100 m AGL see each other at ≈ 82 km); sea clutter and low-grazing-angle effects; **carrier warfare** (deck motion limits by sea state, §22.6); navigation limited to INS/GNSS/TACAN (§50.2); ditching/bail-out survival rules; weather fronts matter | Naval task groups, carrier defence/strike missions, maritime strike; ocean-fraction ≥ 40 % required for those templates (§26.4) | Long-range BVR geometry, sea-skimming ingress, tanker reliance, carrier ops |
| **Islands** | **Limited airfields** (few runways, improvised strips, long gaps); **strategic chokepoints** (straits, passes); scarce diversions make fuel planning and bingo logic decisive; airfield denial is strategic | Campaign graph topology becomes chains/clusters (island hopping, §35.1); base defence and airfield-denial missions; naval + air combined arms | Choose launch sites carefully, deny the enemy's few runways, plan tankers |
| **Arctic** | **Harsh weather** (cold → icing, whiteout, low sun angles, polar day/night); **sparse infrastructure** (long supply lines, few airports, snow/ice runways with μ_b = 0.10–0.20, §10.4); large magnetic declination (true/grid heading modes, §7.8, §50.3); cold-soak engine start effects | High supply-cost multipliers in campaign (§39.5); improvised ice strips; weather-driven twists; reduced civilian traffic | Careful ground handling, weather windows, navigation discipline |

**Terrain character (computed per battlefield).** The battlefield preview and the generator use four quantitative descriptors computed in the pipeline/loader from the sim terrain:

```text
relief R       = 90th-percentile height range in 20 km windows over the battlefield (m)
masking M      = fraction of random point pairs (A: 150 m AGL, B: 3 000 m MSL, horizontal range uniform 20–80 km) with LOS blocked by terrain (LOS service §16.9, 2 000 samples, seeded)
openness O     = mean visual sight distance at 150 m AGL over 200 random points and 16 azimuths (km, capped at 80)
coverage       = fractions of {forest, urban, desert/bare, water, snow/ice, open} (landcover, §46.2)
class(M)       = Flat (M < 0.05) · Rolling (0.05–0.15) · Rugged (0.15–0.35) · Alpine (> 0.35)       (thresholds are design targets; calibrate at M2)
```

The preview shows e.g. "Terrain character: Rugged · masking 24 % · openness 31 km · forest 38 %". Generation rules use these: missions that depend on masking (SEAD with terrain ingress, valley ambush) need `M ≥ 0.10`; naval templates need water ≥ 40 %; long-range BVR templates prefer `O ≥ 40 km`; CAS vs concealed targets prefer forest ≥ 25 %; arctic supply modifiers apply when snow/ice ≥ 40 % or `|lat| > 66°`.

## 46.12 Acceptance criteria

- `AC-46.1` Biome rules (§46.3) produce the expected label for the table of 40 vector cases (data-driven unit tests), including boundaries.
- `AC-46.2` Detail synthesis is bit-reproducible across devices (hash compare) and keeps `|h_render − h_bicubic| ≤ 1.5 m`.
- `AC-46.3` Tree placement is identical on two clients for a given tile (hash of placement list equal).
- `AC-46.4` Synthetic terrain for a masked region is identical across runs and across server/client sim queries.
- `AC-46.5` Look-development gate: reference locations (five biomes × day/dusk × clear/overcast) pass art review against a signed-off reference board (§66.4).
- `AC-46.6` Water: sea states 0–9 render within the frame budget (ocean pass ≤ 1.2 ms at Recommended) and wave height statistics match the table within 15 %.
- `AC-46.7` Terrain character: `M`, `R`, `O` computation is deterministic (seeded) and reproducible on WASM and native; the reference regions classify as expected (design targets, to be calibrated at M2): Alpine for a 100 km box in the Swiss Alps, Rugged for Nevada Basin-and-Range, Flat for the US Great Plains, Rolling for Normandy; mission templates requiring masking are never generated with `M < 0.10` (10⁴ seeds).
- `AC-46.8` In 200 generated missions in Alpine/Rugged regions, ≥ 70 % contain at least one logged terrain-masking event (LOS blocked between a hostile sensor and a friendly aircraft due to terrain) in baseline-pilot runs; island regions produce ≥ 1 airfield-denial option in ≥ 90 % of base-defence missions.

---

# 47. CITIES & INFRASTRUCTURE

`P2–P3 · M2–M3` — Cities, roads, bridges, ports, power and industry give the world purpose: they are navigation cues, targets, hazards, and civilian traffic carriers. Building every real structure at full fidelity is infeasible; the system uses **real footprints and heights with procedural facades and tiered levels of detail** (outline §84).

## 47.1 Intent

- A pilot recognises their city from altitude by its shape, coast, rivers, main roads, airport and dense cores.
- Buildings provide cover, hazards and (rarely) targets without extreme asset counts.
- All placement is deterministic from data and tile id (clients agree).

## 47.2 Data

OSM/Overture features per tile (pipeline §45.5 step 7):

| Feature | Attributes used | Storage per z12 tile (typical urban) |
|---|---|---|
| Building footprint | polygon (tile-local 16-bit coordinates), height (from tag or `levels × 3 m`, or class default by area), roof shape tag, use tag | 5–60 k buildings dense; ≈ 1.1 MB compressed for a dense downtown tile (design estimate) |
| Road | polyline, class (motorway…track), lanes, bridge/tunnel flags | ≈ 0.2 MB |
| Rail | polyline, class | small |
| Water | polygons/lines | small |
| Power | lines (kV), towers (spacing derived), substations | small |
| Aeroway | runways, taxiways, aprons, hangars, ATC towers | per airport |
| Landuse | residential, commercial, industrial, farmland polygons | small |
| Place | name, population, class | index |

A city of 1–10 M people covers 300–1 500 km²; downtown cores have 5–50 k buildings per z12 tile. The feature layer is streamed with the tile pyramid (z ≥ 11 for buildings; roads from z ≥ 9; major roads and railways in z7–z8 simplified).

## 47.3 Building rendering tiers

| Tier | Distance | Representation | Budget (Recommended) |
|---|---|---|---|
| T-Far | > 4 km | **Land-use block shading**: urban texture/colour fields with height-based roughness and night lights; skyline silhouettes for buildings taller than 80 m via extruded footprint instances | ≤ 0.4 ms |
| T-Mid | 1–4 km | **Extruded footprints** with flat or simple roofs, one material per class, vertex-baked AO, merged per sub-tile (≤ 4 draw calls per sub-tile via indirect draw) | ≤ 250 k buildings drawn, ≤ 1.2 M triangles |
| T-Near | < 1 km | Extruded footprints + procedural facade (window grid shader from `levels`, `use`, deterministic hash; roof props) | ≤ 40 k buildings |
| T-Hero | landmarks, airports, bases | Hand-authored meshes (towers, stadiums, bridges, power plants), up to 200 landmark entries per launch region, plus all airport buildings | budgeted separately |

Meshing runs in Stream Workers (earcut-style triangulation of footprints, roof generation from tag or flat), results cached by `(tile_id, feature_version)` in OPFS to avoid recomputation. Night: window emissive pattern from a hash (≈ 35 % lit at 23:00 local, scaled by urban density; blackout in war zones, §47.9).

## 47.4 Roads, rail, bridges

- **Roads.** Rendered as decals/ribbon meshes draped on the terrain from z ≥ 11; road class sets width (motorway 24 m … track 3 m), colour and markings. At distance they appear via the landcover/feature mask. Roads are the substrate for the **ground traffic graph** (nodes/edges with speed limits, lanes) used for civilian traffic (§23.3) and military convoys (§21.4).
- **Rail.** Ribbon + sleepers texture; trains as instances following the graph (§23.5).
- **Bridges.** Footprint polylines with a deck height profile (from OSM `layer`/`bridge` or computed as the spanned terrain extremum + clearance); a bridge mesh with piers; collision proxy (oriented box segments); named landmark bridges use hero meshes. Strike modelling: a bridge has `HP_section` per span and fails by span (§18.9).
- **Tunnels.** Hidden entry portals; vehicles disappear; radar/visual masked.
- **Ports and harbours.** Aeroway-like polygons; cranes and ships as instanced models; naval AI uses harbour polygons for berthing (§22.2).

## 47.5 Power, utilities and industry

- **Power lines.** Polylines with towers every `~ 300 m (HV) / 80 m (MV)`, wire catenary by sag formula `s = w L² / (8 H)`; lines are a **flight hazard below 100 m AGL** (collision proxy per span with collision radius 0.5 m, damage on contact per §18.8; HUD obstruction warning cue in low-altitude mode, §54.9).
- **Substations and power plants.** Cluster of models; destruction reduces local lighting (blackout) and increases the regional "infrastructure damage" index used by the campaign (§39.5).
- **Industrial areas / fuel depots / refineries.** Procedural layout inside landuse polygons from templates (tanks, pipes, stacks); strategic targets with area damage and secondary explosions (§18.6, §15.9).
- **Dams, towers, wind farms.** Hero models or instanced props; wind farms are visual hazards and radar clutter sources (modelled as Doppler returns with blade rate, §16.8).

## 47.6 Targets from infrastructure

The mission generator (§26) selects targets from feature entities with categories (bridge, depot, radar site, airfield, power, port, rail yard, bunker, headquarters). Each target type has a **Target Definition** (`targets.json`): footprint, hit boxes, armour class, HP, destruction states (intact → damaged → destroyed with a rubble mesh swap), required weapon effect (blast/penetration/incendiary) and the `valid_ordnance` table; rubble and fire effects persist for the mission duration (and into the campaign's infrastructure state when applicable). Civilian-dense buildings are tagged `protected` and never selected as targets; collateral damage is tracked (§29.7, §38.7).

## 47.7 Spatial structures and performance

- **Grid index** per z12 tile (32 × 32 cells) for buildings and objects; broad-phase queries for gun/missile impact, LOS (buildings occlude visual/IR/EO, partially radar) and AI.
- **Sim representation.** For determinism only a *sim feature subset* is authoritative: bridges, power towers/lines, hero buildings, targets, airport structures, and buildings taller than 30 m (collision boxes). The other buildings are visual only (ground-level collision uses a coarse 8 m height raster from footprints, `building_height_raster`, for low-level flight crash detection: aircraft below the raster height at the cell crash).
- **Culling.** Per sub-tile frustum and Hi-Z occlusion culling on the GPU (compute), instance buffers built once per tile load; the render thread only updates the camera-visible list.
- **Memory.** Mid-tier mesh data ≈ 12 bytes/vertex packed; a downtown tile ≈ 6–18 MB GPU; cap 600 MB for building geometry in Recommended with LRU eviction.

## 47.8 Civilian and urban dynamics (cross-reference)

Street lights, traffic density by hour, airliners to/from airports and ground vehicles are specified in §23; this section provides their substrate (graph, footprints, landuse). Population drives night-light intensity and the civilian-presence factor used by ROE and the intensity director (§29.4).

## 47.9 Wartime state

Urban areas adopt war states from the campaign/mission: *normal*, *alert* (reduced lights), *blackout* (no lights, optional), *damaged* (rubble tiles, fires, smoke columns). State changes are applied as overlays keyed by object id and sent as part of the replicated world state (§33.5).

## 47.10 Failure behaviour

Missing feature data: roads/buildings are simply absent (terrain remains); the mission generator avoids feature-dependent objectives in such regions (target-category availability check); airports fall back to procedural completion (§10.6). Never a crash; a data-quality indicator is shown in the battlefield preview (§9.7).

## 47.11 Acceptance criteria

- `AC-47.1` A downtown z12 tile (≥ 30 k buildings) renders within the T-Mid budget at 60 fps on Recommended with ≤ 1.2 M triangles drawn.
- `AC-47.2` Building meshing for a dense tile completes in a Stream Worker in ≤ 400 ms (p95) and the cached result loads in ≤ 50 ms.
- `AC-47.3` Collision with a bridge, tower and a power line span triggers the expected damage and warning on two clients simultaneously (determinism test).
- `AC-47.4` Every target type in `targets.json` validates (hit boxes, states, valid_ordnance) and a randomised mission generation never selects a `protected` building (10⁴ seeds).
- `AC-47.5` Low-flying crash test: aircraft descending into a building footprint raster cell below its height is destroyed in both server and client sim identically.
- `AC-47.6` Night-light pattern is reproducible for a given (tile, time) and respects blackout overlays.

---

# 48. AIRBASE OPERATIONS

`P3 · M3 (core) → M6 (persistent damage)` — Airbases are living places: they detect threats, go on alert, scramble fighters physically (engine start, taxi, take-off, form-up), rearm and refuel aircraft, and can be damaged until repaired (outline §11 operations, §140 scramble, §141 damage, §142 dynamic frontline). This chapter specifies the **Airbase Operations Service (AOS)** that runs inside the Sim Core and is used by the sandbox, co-op, 1v1 and the campaign.

## 48.1 Intent and principles

- Operations are **physical**: aircraft are real entities on the taxi graph (§10.5) and runway, not menu abstractions, within the fidelity tier of the base (§48.8).
- Bases near humans run the full model; distant bases run a simplified, statistically equivalent model (outline §140).
- Everything that matters for fairness is deterministic and server-authoritative; visual-only ground clutter (personnel, trucks) is client-side.
- Player-friendly: the player can start anywhere in the sequence (§48.2) and is never forced to wait through a long routine.

## 48.2 Start types (spawn states)

| Start | State at T0 | Time to airborne (Kestrel-class, design targets) | Notes |
|---|---|---|---|
| **Air start** | Airborne, at the specified point/speed/altitude; gear/flaps per phase | 0 s | Default fallback when no airfield qualifies (§10.8) |
| **Runway hot** (default sandbox) | Lined up on the runway, engine running, brakes on | ≈ 8–15 s (Kestrel at 10 500 kg, SL ISA: roll ≈ 200 m with afterburner, ≈ 320 m at military power, Appendix C; heavy, hot or high-elevation take-offs reach 600–900 m) | Quick action |
| **Runway cold** | Lined up, engine off, ground power connected | ≈ 70–90 s | Start sequence |
| **Parking hot** | On a stand, engine running | taxi + take-off ≈ 2–4 min | Visual ATC cues |
| **Parking cold** | On a stand, engine off, canopy open | pre-flight + start + taxi + take-off ≈ 4–8 min | Full ground ops, skippable via "Fast start" |
| **Alert scramble** | In the alert shelter or on the alert pad, engine off, crew in the cockpit | ≈ 2.5–4.5 min to wheels-up (§48.3) | QRA missions |
| **Carrier cold / hot** | On the deck (§22.6) | cat-ready ≈ 60–120 s | Launch bar, catapult |

*Clarified in 0.2.2 (ADR-36):* the sandbox default is **Runway hot** from v0.1b, on the test airfield of the procedural world: 150 m past the threshold of a 3 000 m by 45 m runway (heading 000, 40 m MSL), at rest on the gear (pitch about 3°), engine idling, parking brake set, gear down, take-off flaps, 60 % fuel, under Standard Assist. `?spawn=air` starts airborne; the player's respawn choice (pause menu) and the reset key's request override the session's start. The host hands its throttle lever over at every spawn (the core holds the spawn throttle until the host acknowledges) so a respawn never launches the aircraft from a stale lever.

**Engine start sequence (Standard Assist: one key "Auto-start"; Realistic: stepwise):** battery → fuel pumps → APU/ground air → engine spool (idle reached 30–45 s, Kestrel class) → generators → avionics align (INS ≈ 90 s full, 25 s "fast"; skippable in Standard) → flight controls check → canopy closed. Auto-start runs the entire sequence in ~50 s of game time; a "skip to ready" option is available in any mode with cost 0 (sandbox) or a time penalty (campaign scoring has none; time matters only for scramble pressure).

**Takeoff distance** uses the ground model (§11.13) with the planner in §10.4; **taxi** speed limits 15 m/s straight, 5 m/s on turns > 45°, 3 m/s on apron; the taxi graph (§10.5) provides a visual **taxi guide line** (opt-in) and the tower voice gives routing in plain brevity ("Kestrel Two, taxi runway two seven via Alpha, hold short").

## 48.3 Scramble system

### 48.3.1 State machine (per interceptor flight)

```text
IDLE → ALERT_PENDING → CREW_RUN → ENGINE_START → TAXI → LINE_UP → TAKEOFF_ROLL → AIRBORNE_CLIMB → FORM_UP → INTERCEPT
 (any state) → ABORT_SHELTER on base under attack (§48.5 level 4)
```

| Stage | Duration (design target, fighter on 5-min alert) | Condition for next stage |
|---|---|---|
| Alert declared → crew run | 15–25 s | Klaxon event; crew at aircraft |
| Crew in cockpit → engines at idle | 40–55 s | Auto-start (parallel dual-engine light-up) |
| Taxi to runway | 20–120 s (distance ÷ 12 m/s + hold at junctions; shelters near runway ends are ≤ 400 m) | Runway free |
| Line-up + take-off roll | 15–25 s per flight (pair simultaneously on runways ≥ 40 m wide; else 25 s separation) | Wheels up |
| Climb-out + form-up | 45–90 s | Wingman within 1.5 km, both above 3 000 m AGL |
| **Total, alert pair** | **≈ 2.5–4.5 min** (median 3.2 min) | |

### 48.3.2 Scramble decision

The base commander AI (a sub-agent of the squadron layer, §19.4 / §20.5) evaluates each hostile track `i` with a range `R_i`, closing speed `v_c,i`, and an estimated ETA to the base's *defended area edge* (radius `R_def`, default 60 km for a Military Airbase, 30 km Regional):

```text
ETA_i  = max(0, R_i − R_def) / max(v_c,i, 50 m/s)
scramble if  ETA_i  ≤  T_launch_min + T_climb_buffer + T_margin
   T_launch_min = 240 s (alert 5 pair median incl. taxi),  T_climb_buffer = 120 s,  T_margin = 180·(1 − readiness)         (readiness ∈ [0,1], §48.5)
```

At readiness 1.0 (alert pad, engines warm) the trigger is ETA ≤ 360 s; at 0.5 ETA ≤ 450 s. For a Mach 0.9 raid (≈ 270 m/s) that is a 97–122 km trigger radius beyond `R_def`, which keeps the intercept in front of the base. Sizing: the number of interceptors launched `n = clamp(ceil(1.5 × n_hostile_estimate), 2, available_alert_aircraft)`, never more than two flights per runway per 3 minutes (runway throughput, §48.6).

Information limits: the decision uses the base's **own sensor and C2 picture** (radars, link from AWACS and neighbouring bases, §42) — a base blind to a low-flying raider does not scramble until the visual/sensor picture updates. This enables low-level surprise attacks and rewards SEAD and EW (§17).

### 48.3.3 Fidelity and player participation

- **Player on alert (QRA mission, §27):** the player is spawned in the "Alert scramble" start; the klaxon starts the timeline; the AI wingman follows the stages above while the player flies their own take-off; an optional "auto-taxi/auto-takeoff" assist exists (Standard).
- **AI-only interceptors** are simulated at Tier T1 on the taxi graph and runway (full physics ground model at 30 Hz) when within 50 km of any human aircraft or any camera; otherwise at the simplified model (§48.8).
- **Hot runway rule:** a flight cannot line up while another aircraft is on final within 60 s of touchdown; landing aircraft yield to emergency (fuel/damage) priority. Ties broken by tower queue order.

## 48.4 Base sensor and C2 model

| Component | Function | Dependencies |
|---|---|---|
| Early-warning radar | Long-range air picture (range from `radar` data, typically 250–400 km at reference RCS) | Power, facility health (§48.7) |
| Short-range radar / IRST | Terminal picture | Power |
| Command centre | Fuses tracks, issues scramble orders, assigns CAP | Command facility health |
| Comms | Radio/data-link to flights | Comms health; jamming (§17) |
| Visual observers | Reports within 15 km with probability of detection by visibility | Always |

Degradation: radar destroyed ⇒ picture falls back to visual observers + neighbours (range ≤ 15 km, latency +20 s); command centre destroyed ⇒ scramble decisions are delayed by `+90 s` and sizes limited to `n ≤ 2`; comms jammed ⇒ AI flights fly the last orders.

## 48.5 Alert levels

Each base has an **alert level** (levels 0–4) and a continuous **readiness** value (0–1) used in §48.3.2.

| Level | Name | Entry condition | Effects |
|---|---|---|---|
| 0 | **Normal** | No hostile tracks within 600 km | Readiness 0.3; alert aircraft in shelters, engines off |
| 1 | **Increased** | Hostile track within 400 km or intel warning | Readiness 0.5; crews at aircraft; AI CAP opportunistically launched |
| 2 | **Alert** | Hostile ETA to `R_def` < 15 min or missile/air-defence warning | Readiness 0.8; alert flights at the runway threshold, engines idling; SAM sites weapons-hot |
| 3 | **Scramble** | Scramble decision (§48.3.2) | Readiness 1.0; launch in progress; further sorties paused until formed up |
| 4 | **Under attack** | Weapons impact or inbound missile within 15 km | Personnel shelter; refuelling/rearm paused; take-off allowed only for flights already on the runway; point defences fire |

Level changes use hysteresis: dropping a level requires the condition to remain clear for 120 s. Levels are visible on the world map (colour ring) and tactical map and are shared with allied AI. The alert level is part of the airport state (§10.10) and replicated (§33.5).

## 48.6 Ground operations model

**Parking and servicing.** Each stand has a servicing crew model (abstract): when an aircraft parks (engines off), the service queue starts.

| Service | Time (design targets) | Condition |
|---|---|---|
| Refuel | `fuel_needed ÷ flow` with flow 20 kg/s (hose) per available fuel point; Kestrel 3 100 kg internal ≈ 155 s (≈ 275 s with 5 500 kg including drop tanks); hot refuel ≈ +30 s | Fuel facility health ≥ 25 %, stock > 0 |
| Rearm | Per store: air-to-air missile 90 s, air-to-ground bomb 120 s, pod 60 s, gun ammo 60 s; limited by rearm crews (1 per 4 stands) | Rearm facility, ordnance stock |
| Repair | Light damage: 2 min per 10 % airframe HP (up to 40 %); systems: 1–3 min each; above 40 % damage requires a **maintenance bay** (hangar) and 10 min per 10 % | Repair facility (hangar), spares stock |
| Turn-around (full) | Typical 6–12 min for a fighter (refuel + rearm + inspect) | |
| Player quick-service | Sandbox: instant ("Quick turn") on the stand; campaign: crew times above, accelerated by time acceleration (§49.3) | |

**Runway throughput.** Take-off interval single stream 40 s (or 25 s when pairs line up simultaneously), landing interval 60 s (fighter-class, same runway), so ≈ 60–90 movements per hour per runway; mixed with arrests/go-arounds reduces by 20 %. Runway **occupancy** is a resource: one aircraft may hold a runway at a time (tower queue), enforced for AI and humans (humans receive a warning and radio clearance; in Arcade Assist ATC is ignored).

**Stocks.** Fuel, ordnance classes, spares, and personnel. Sandbox: unlimited. Campaign: stocks consume and replenish via supply (§39.5).

## 48.7 Airbase damage and repair (outline §141)

Persistent facility damage applies when the airbase is in a campaign (and optionally in sandbox "persistent damage" for the duration of the session). Each facility has `health ∈ [0,1]` and a function:

| Facility | Capability when degraded | Repair |
|---|---|---|
| **Runway** (per runway) | Craters: each crater (runway penetrator hits create crater radius `r_c`; typical 8–15 m) removes usable length; usable length = longest crater-free strip; take-off needs `L_avail ≥ 1.0×` required roll (else "TOO SHORT"). Fixed-wing ops stop when no strip ≥ min length (or aircraft with STOL flag) | Engineers: 4 craters/hour per crew (design), `repair_crews` 1–4; crater repair is a visible task (rapid repair kits) |
| **Taxiways/aprons** | Blocked nodes reroute traffic; capacity reduced | 20 min per node |
| **Radar** | §48.4 | 30 min to 4 h (replacement) |
| **Hangars** | Repair capacity (parked aircraft inside destroyed) | 24 h+ (campaign scale) |
| **Fuel** | `stock` loss (fire), refuel speed reduced proportionally | 12 h+ |
| **Ordnance depot** | Rearm capacity; secondary explosions | 12 h+ |
| **Defences** | SAM/AAA sites individually destroyed | Replacement from stock |
| **Command** | §48.4 | 6–12 h |
| **Power** | Lighting off, radar off at 0 | Generators partial restoration |

The overall **operational capability** `C_base = min(runway_factor, 0.35 + 0.65·mean(other_factors))` ∈ [0,1] caps sorties per hour: `sorties/h = C_base × sorties_nominal`. Hit boxes and strike effects: §15.9 and §47.6. Repairs are simulated in the campaign time model (§34.2, §39.3) with `repair_rate` modifiers by faction and resources; in a live mission, repairs proceed in real time at the above rates (accelerated by time acceleration).

Intel interplay: attackers see only observed damage (BDA photo/recon, §42.2), not true health; defenders see the truth.

## 48.8 Fidelity tiers for bases

| Tier | Where | Content |
|---|---|---|
| **Full (B0)** | Within 50 km of any human aircraft/camera; any base with an active mission objective | Real entities on the taxi graph, per-aircraft state machines, runway occupancy, services with crews |
| **Reduced (B1)** | 50–250 km | Flights are entity-light "pending sorties": a timeline of {scramble start, wheels up, formed up} computed from the §48.3.1 durations plus a runway-slot delay; aircraft appear (spawn on the runway threshold at the computed time) only when they would enter B0 |
| **Strategic (B2)** | > 250 km or off-mission | Aggregate: sortie rate = `C_base × nominal`, damage recovery per the campaign clock; no entities |

Transitions are deterministic: a promoted B1 flight spawns at its computed state (position along the take-off roll or climb-out); a demoted flight on departure becomes a B1 flight with a destination.

## 48.9 Ownership and dynamic frontline (outline §142)

Capture and counter-attack are specified in §36 and §35.3; this chapter provides their mechanics: on ownership change the base's alert level resets to 4 → 1, alert aircraft are removed or captured per policy, stocks halve (looting rule) and facility health persists. Enemy counterattacks (strategic AI) select weakened bases by `C_base` and supply state (§35.5, §39.5), so base damage changes the front line.

## 48.10 Multiplayer and determinism

AOS state (alert level, facility health, stocks, runway queue, scramble timeline) is simulation state, replicated to clients as part of world deltas (low frequency, 1 Hz plus events). The scramble timeline uses the same RNG sub-stream as the mission (§26.3), so a replay reproduces identical launch times. Visual servicing animation is client-side.

## 48.11 Acceptance criteria

- `AC-48.1` A scripted alert-pair scramble from a shelter 300 m from the runway reaches wheels-up in 150–270 s (median ≈ 190–230 s) and forms up within 90 s of take-off over 200 random seeds.
- `AC-48.2` With an inbound Mach 0.9 raid at 100 m AGL under radar cover, the base scrambles when `ETA ≤ trigger`; without radar cover it scrambles only after the visual/neighbour picture detects (verified by log).
- `AC-48.3` Runway cratered to a 700 m strip refuses take-off to an aircraft requiring 900 m with a "TOO SHORT" message; repairs restore capacity at the stated rates (±10 %).
- `AC-48.4` Capability formula: injecting facility health vectors yields sortie rates matching §48.7 within 1 sortie/h.
- `AC-48.5` Tier promotion/demotion (B0↔B1) neither duplicates nor loses a scrambling flight (property test over 10⁴ randomised transitions).
- `AC-48.6` Server and clients agree on alert level and facility health at all times (hash comparison every 5 s, method as in §33.13).

---

# 49. LONG-RANGE OPERATIONS

`P2–P3 · M2–M4` — Not every sortie should immediately become a dogfight (outline §86). Long-range operations structure the *transit → detect → intercept → combat → return → land* arc of large battlefields (200–500+ km), and give players tools — cruise assistance, autopilot, time acceleration — that make long distances interesting rather than tedious while preserving risk.

## 49.1 Intent

- Distance is gameplay (fuel, timing, exposure) but not boredom: transit ≤ 5 minutes of *attention-free* time per sortie by default through autopilot and time acceleration.
- The sortie arc is explicit in the HUD, tactical map and briefing, and the mission generator ensures meaningful events during transit (§29.4).
- Safe, deterministic time acceleration for single-player (and unanimous-vote co-op).

## 49.2 Sortie phases

```text
START → TAKEOFF → CLIMB → TRANSIT (cruise) → DETECTION/ISR → INTERCEPT/INGRESS → COMBAT → EGRESS → RETURN → RECOVERY (approach, landing, taxi)
```

Phases are derived (not scripted) from the aircraft state, route and mission context; they drive HUD mode hints, the music director (§56.8), and statistics (§65). Fuel planning (below) is displayed per phase.

## 49.3 Time acceleration

| Setting | Factor | Conditions |
|---|---|---|
| Real time | 1× | Always |
| Fast transit | 2×, 4×, 8× | Single-player or unanimous co-op vote; autopilot engaged; all human aircraft in cruise or on route |
| Skip to next event | up to 16× then auto-stop | As above; stops at the next triggered event/contact |
| Fast ground ops | 4× (taxi/start) | On ground; combat zone clear |

**Safety rule (hard):** time acceleration is disabled and any active acceleration drops to 1× when any of the following holds:

1. A hostile track (air or SAM engagement zone) is within the **threshold radius** `R_th = max(80 km, 120 s × closing speed)` of any human aircraft;
2. A missile launch warning/RWR missile state occurs;
3. A human aircraft is below the safe altitude `h_safe` (500 m AGL) and not in an approved terrain-following mode;
4. A human aircraft is within 30 km of the landing airfield or in landing config;
5. A crew command (e.g. disengage autopilot) or a damage/fuel emergency occurs.

Mechanism: the Sim Core runs `k` fixed 60 Hz ticks per presentation frame (up to 8 per frame; ≥ 16× is achieved by running lower tiers at reduced rate: T3/T4 at ×k with doubled step, aircraft in the autopilot T1 at a 30 Hz step) bounded by the CPU budget; if the sim cannot sustain the factor the effective factor drops and a "limited" indicator appears. Weather/time-of-day progress consistently with sim time. AI and world run identically (determinism preserved because accelerated stepping is the same fixed-step loop). Multiplayer: only with unanimous vote (server checks conditions each tick and cancels automatically).

## 49.4 Cruise assistance and autopilot hold

- **Cruise mode** (one key): engage autopilot modes (§50.4) on the active route at the aircraft's economical cruise profile: altitude from the *optimum cruise altitude table* (§49.6), speed at *max range* or *loiter* speed as selected, automatic climb/descent, throttle management. The autopilot disengages (with a tone and a HUD message) on pilot override, damage, missile warning, or hostile contact within `R_th`.
- **Autopilot hold on tab loss:** when the browser tab is hidden for > 10 s (§0.7 browser lifecycle), the aircraft enters "hold": level flight on the current heading, speed for max endurance, altitude held (no terrain-following; a safe-altitude check climbs to `max(current, terrain_max_ahead + 300 m)`); for the host or single-player the sim is paused instead. The pilot is told on return; hold ends on input.
- **Cruise safety:** the autopilot never flies below a safe altitude (`h_safe` = 500 m AGL above ground except in terrain-following mode) and avoids restricted zones by planning around them. It performs no combat actions.

## 49.5 Aerial refuelling (AAR)

Tankers (the fictional "Stratus" tanker, Appendix D) orbit on racetrack tracks. Procedure and mini-state-machine (`§11.16` mechanics):

| State | Conditions |
|---|---|
| **Approach / pre-contact** | Join from the rear-left; speed 250–280 kn IAS (tanker 250 kn), altitude 20 000 ft typical, within 300 m behind and 50 m below the tanker |
| **Observation** | Aircraft in 10–30 m position; the HUD shows the **refuel guide** (3-axis cue: lateral, vertical, longitudinal error from the contact point, in metres) |
| **Contact** | Probe tip within 0.3 m of basket centre and closure < 2 m/s (drogue) or boom envelope ±3 m lateral, ±2 m vertical (boom: operator AI) |
| **Transfer** | Flow 20 kg/s (drogue) / 45 kg/s (boom); stops at full, at the tanker's offload limit, or on disconnect |
| **Disconnect** | Hose-limit exceedance (±8 m) or command; reverse speed < 3 m/s |

Basket dynamics: second-order spring-damper with buffet noise from the tanker's wake and turbulence, amplitude by weather (§24.4); approach on the final 10 m is the hard part and is assisted in Standard Assist (soft capture assist: autopilot nudges the probe when within 3 m; hidden in Realistic). Tanker AI flies the track at constant speed, turns ≤ 25° bank, avoids threats (retreating if a hostile track within 100 km), announces "cleared pre-contact", "contact", "fuel transferred 2000", "disconnect", and exposes `tanker_offload` (kg available). Tanker loss strands aircraft (bingo logic §19.7). Without the player, AI aircraft refuel using the same machine (timed at the data rates).

## 49.6 Range, endurance and fuel planning

The loadout screen (§14.5) and briefing show a fuel plan per route using the aircraft's range tables:

```text
Breguet range:  R = (V / TSFC) · (L/D) · ln(W_i / W_f)         (jet; per segment, using altitude/Mach-dependent TSFC and L/D from the aero tables)
Endurance:      E = (1/TSFC) · (L/D) · ln(W_i / W_f)
Optimum cruise: altitude/speed that maximises (M · L/D) / TSFC  → baked table (Mach × altitude × weight) per aircraft/loadout
```

The Kestrel with a typical air-to-air loadout (internal fuel 3 100 kg, 2 drop tanks +2 400 kg ≈ 5 500 kg total), cruise at M 0.85 / 11 000 m has an estimated **combat radius ≈ 520 km** (design target; derived from estimated from the aircraft's `TSFC` and `L/D` tables; the figure is a design target to be validated against the conformance test §11.19, not an authored value). The mission generator uses the radius to place spawns and objectives within reach (§26.5 range feasibility check) and to decide on tankers.

**Bingo/joker.** `joker = reserve_to_divert + combat_allowance`, `bingo = fuel to reach the nearest friendly airfield (or tanker) + 10 % + 3 min at sea level`. The HUD shows both; AI uses them (§19.7). A route planner flags "insufficient fuel" before launch.

## 49.7 Detection and intercept in transit

Early-warning and AWACS provide long-range cuing (§43), giving the intercept a structured arc: contact → vector → radar lock → merge. **Intercept geometry helper** (opt-in assist): displays the intercept heading `ψ_int` and time-to-intercept using the lead-collision course

```text
v_t, ψ_t = target velocity;  r = target position relative to own;  solve  |r + v_t·t| = v_own·t  for t   (smallest positive root)
ψ_int = bearing of (r + v_t·t)
```

AWACS "picture" calls (§56.6) and the datalink provide the tracks; absent in hardcore settings.

## 49.8 Return and recovery

Return-to-base guidance: the "RTB" command (key or radio) sets the route to the nearest suitable friendly base given fuel and damage, with the autopilot offering ILS-like approach capture (opt-in assist §10.9); diversion logic picks alternates when the primary is damaged or under attack (§48.5). Carrier recovery uses the landing pattern (§22.6).

## 49.9 Edge cases

| Case | Handling |
|---|---|
| Time acceleration while a multiplayer peer joins | Cancelled to 1× at join |
| Fuel runs out in autopilot | Autopilot declares emergency, flies best-glide to a safe landing area (deadstick approach to the nearest airfield if reachable; otherwise ejection prompt) |
| Autopilot over rugged terrain with stale tiles | Hold altitude ≥ max sim terrain along the path + 300 m using the sim layer (§44.6), independent of render tiles |
| Hosting throttle | §0.7 |

## 49.10 Acceptance criteria

- `AC-49.1` A 400 km route flown with cruise assistance and 8× time acceleration completes in ≤ 7 minutes of wall time with the sim stable (no tier starvation) on the Recommended profile.
- `AC-49.2` Time acceleration disables within 1 sim second of any of the five safety conditions (§49.3) in scripted tests, including in co-op.
- `AC-49.3` Contact transfer: a scripted approach with 0.5 m offset and 1.5 m/s closure connects; 0.4 m offset or 2.5 m/s fails; flow rates match ±5 %.
- `AC-49.4` Breguet range/endurance computed in the planner agrees with a simulated cruise to within ±7 % for 20 aircraft/loadouts.
- `AC-49.5` The lead-collision solver returns an intercept within 0.1° of the numerical solution for 10⁴ random geometries (including no-solution detection when the target is faster and diverging).

---

# 50. NAVIGATION

`P2 · M2` — Navigation is waypoints, headings, GPS-like position, TACAN/ILS-like aids, and the tactical map; hardcore settings can disable assistance (outline §87–§89, §88). This chapter specifies the **navigation system**, the **autopilot modes**, and the **waypoint model** shared by player, AI and mission definitions.

## 50.1 Intent

- A pilot can always answer "where am I, where am I going, how long until I get there, what's my fuel margin" with the information their aircraft and assistance setting allow.
- Real-world navigation is grounded: true vs magnetic north (WMM), geodesics, great-circle distance, wind triangle.
- Assistance degrades gracefully with realism (§59.2): GPS-like on → INS only → map/compass only.

## 50.2 Position source model

| Source | Accuracy | Failure / degradation |
|---|---|---|
| **GNSS** (GPS-like) | 5 m (1σ), 1 Hz | Jammed/spoofed in EW scenarios (§17.9): error grows to 100–1 000 m or biased |
| **INS** | Drift 0.5 nm/hour (≈ 926 m per hour ≈ 15 m per minute; model below) | Alignment time 90 s; drifts without update; GNSS/TACAN/visual fix resets |
| **TACAN/VOR-like** | Bearing ±1°, DME ±0.1 nm | Line-of-sight limited (§16.9), range 200 nm at altitude |
| **Visual / map** | Pilot-estimated | Always available |

INS drift model: velocity error random walk with `σ_v = 0.15 m/s per √h` plus a slowly varying bias such that the position error after one hour is ≈ 0.5 nm (≈ 0.93 km) median; in code: `pos_err(t) = b·t + w(t)` where `b ~ N(0, (0.2 m/s)²)` per axis (Rayleigh median 0.2 × 1.177 × 3 600 s ≈ 850 m, plus ≈ 70 m from the random walk ⇒ ≈ 0.5 nm) chosen at alignment, `w` random walk. The nav fix (GNSS or TACAN) resets the error with Kalman-like blending (`k = 0.2` per fix). In "GPS always" assistance, position is exact.

The player's aircraft **navigation solution** (ownship position, velocity, wind estimate) feeds the HUD, MFD, autopilot and planners. The true sim state is always exact; the displayed state differs only under jamming/INS drift when the realism dial is set accordingly.

## 50.3 Heading, bearing, distance

- **Reference:** the player sees magnetic or true heading selectable (default magnetic). Declination `δ(lat, lon, date)` from WMM (spherical-harmonic degree 12, coefficients bundled; evaluated on a 1° grid + bicubic interpolation; ~µs per lookup, error < 0.5°).
- **Great-circle navigation:** bearing/distance between waypoints use the geodesic (Vincenty or Karney inverse, §7.9): bearing is computed at the aircraft's current position each frame (so courses are "straight lines" on the globe), which makes long routes correct.
- **Track vs heading:** `track = heading + drift`, wind triangle `sin(drift)/V_w = sin(ψ_w − track)/TAS`; the HUD flight path marker shows the actual track, the heading tape shows nose direction (§54).
- **ETA/ETE:** `ETE_leg = d_leg / GS`; ground speed from velocity over ground; fuel at waypoint estimated from current flow.
- **Rhumb vs great circle:** only the great circle is exposed to players; the tactical map draws geodesic segments as curves in Mercator-like projections.

## 50.4 Autopilot and flight director modes

| Mode | Behaviour | Controller |
|---|---|---|
| **HDG hold** | Holds selected magnetic/true heading with bank ≤ 30° (Standard) | Bank-to-turn: `φ_cmd = clamp(K_ψ·Δψ, ±φ_max)` with rate limit 4°/s |
| **TRK hold** | Holds ground track (wind-corrected) | Same, using track error |
| **ALT hold** | Holds barometric altitude ± 10 m; vertical rate ≤ 25 m/s | PID on altitude → pitch rate; trims throttle with speed hold |
| **VS mode** | Holds vertical speed | |
| **SPD hold** | Holds IAS/Mach via throttle | Throttle PID with AB lockout |
| **NAV (waypoint following)** | Great-circle leg tracking with cross-track error correction: lateral guidance law `ψ_cmd = bearing_to_wp + atan(K_xt · XTE / V)` limited to ±45°, leg switch at turn-anticipation distance `d_turn = V² / (g · tan φ_max)` (fly-by) or on overfly | Follows `Route` (§50.6), handles `Fly-over` and `Fly-by` |
| **RTB** | Direct-to nearest suitable base (§49.8) | NAV + descent profile |
| **CRUISE** | NAV + ALT/SPD at optimum cruise (§49.4, §49.6) | |
| **TF (terrain-following, opt-in)** | Holds AGL 60–300 m using a look-ahead terrain profile at 2 Hz on the sim terrain (§44.6): required pull-up `n_req` from profile; limit 3 g; never engaged over unknown terrain; disengages on obstruction/radar-alt fault | Radar-altimeter assist; hazards from power lines/buildings not covered → HUD warns (§54.9) |
| **LAND (opt-in)** | ILS-captured approach with auto flare (Arcade/Standard only) | §10.9 |
| **ORBIT/LOITER** | Racetrack or circular orbit about a point, radius from airspeed/bank | For CAP/tanker/escort |

All modes sit **above** the FCS (§11.11): they emit pitch/roll/throttle commands as an AI-like "virtual stick"; the FCS limits (alpha, g) always apply. Altitude-hold quality target: ±15 m rms in smooth air; heading hold ±1°. Autopilot disconnect: any stick deflection beyond 15 % or `A/P` key press; automatic on stall warning, G > 4, missile warning (CRUISE/NAV only), or sensor failure.

Mode annunciation: the HUD/MFD shows engaged lateral/vertical/speed modes; mode changes produce a short chime. In Realistic assistance the autopilot is simplified to HDG/ALT/SPD hold and NAV is withheld.

## 50.5 Navigation aids and map aids

- **Navigation sources:** waypoints, airports (TACAN/ILS-like §10.9), bookmarks (§9.10), mission markers, tanker tracks, carriers (TACAN), and "bullseye" (a mission reference point from which contacts are called as bearing/range, e.g. "Bandit, bullseye 270, 40").
- **Bullseye:** per-mission reference point (default at the battlefield centre or the front line); the HUD shows bearing/range to bullseye; AI and AWACS calls use it (§56.6, Appendix G).
- **Tactical map (§55)** and MFD map show position, route, threat rings, and with assistance enabled the **moving map** is georeferenced to the loaded terrain.
- **Hardcore options (realism dial §59.2):** hide ownship position on the tactical map; disable GNSS (INS only); hide the route line (only waypoints list); no waypoint auto-sequence; require manual tuning of TACAN.

## 50.6 Waypoint model

```json
{ "id": "wp-04", "type": "target", "pos": { "lat": 36.20, "lon": -121.80, "alt_m": 4500, "alt_ref": "msl" },
  "speed": { "mode": "ias", "value_mps": 230 }, "turn": "flyby", "name": "BRIDGE ALPHA", "tot_s": 2340, "tolerance_s": 60, "task": "attack", "notes": "" }
```

| Type | Meaning | Mission/AI use |
|---|---|---|
| **Start** | Route origin (spawn or take-off point) | |
| **Patrol** | Point on a patrol or CAP route | CAP orbit |
| **Target** | Target area / initial point to the target | Strike, SEAD |
| **Escort** | Join and protect a friendly asset | Escort missions |
| **Attack** | Attack run initial point / weapon release | Triggers weapons-free logic |
| **Rendezvous** | Meet other aircraft/tanker at time/place | Join-up, tanker |
| **Refuel** | Aerial refuelling track | AAR (§49.5) |
| **Extraction** | Exit zone (boundary legal, §8.5) | Mission ends "extracted" |
| **Airbase** | Landing airfield or diversion | RTB, landing |

Waypoints sit in `Route`s `[wp…]` with a unique `route_id` and an owner (player flight, AI flight). Time-on-target (TOT) legs recompute speed: `v_req = d_remaining / (TOT − t_now)` limited to the aircraft's cruise envelope.

**Player-created waypoints.** On the tactical map: click to add, drag to move, right-click to insert/delete, a type picker, altitude and speed editing, "Direct-To" (activate a waypoint instantly), coordinate entry (lat/lon, MGRS-like grid, bearing/range from reference), import from bookmarks. Max 99 waypoints per route, 8 routes per flight. Custom waypoints are saved in the mission file (§51.2) and the campaign (as flight plans).

**Auto-sequencing** switches to the next waypoint at the turn point or overfly; **hold-down** on a waypoint pauses auto-sequence. **Route validation** checks: boundary compliance (inside battlefield, §8.5), restricted zones, minimum altitude, fuel margin, speed feasibility, obstruction; warnings are shown in the planner.

## 50.7 Data model and API

```rust
pub struct Route { pub id: RouteId, pub owner: EntityOrFlight, pub waypoints: Vec<Waypoint>, pub active_idx: u8, pub mode: RouteMode /* Auto|Hold|Direct */ }
pub struct NavSolution { pub pos_est: Geo, pub vel_est: Vec3, pub pos_err_est_m: f32, pub gnss_valid: bool, pub ins_age_s: f32, pub wind_est: Vec3 }
pub trait NavService {
    fn solution(&self, e: EntityId) -> NavSolution;
    fn leg_info(&self, r: &Route) -> LegInfo; // bearing, distance, XTE, ETE, ETA, fuel-at-wp
    fn set_route(&mut self, e: EntityId, r: Route) -> Result<(), RouteError>;
    fn direct_to(&mut self, e: EntityId, wp: WaypointId);
    fn plan_tf_profile(&self, path: &[Geo], limits: TfLimits) -> TfProfile; // look-ahead
}
```

All route changes are **commands** through the same input/command channel as flight inputs (§33.6) so they replicate to the server and replays.

## 50.8 Edge cases

| Case | Handling |
|---|---|
| Waypoint outside boundary | Rejected by the validator (except Extraction on boundary) |
| Route across the antimeridian or pole | Geodesic inverse handles it; distance/bearing continuous |
| Autopilot with stale terrain tiles | Uses sim layer (§44.6) for TF/safe altitude |
| Magnetic declination change over a long route | Evaluated per frame at the aircraft position |
| Jammed GNSS during NAV | Autopilot switches to INS-based navigation; position uncertainty is shown; route tracking degrades by the INS error |
| Dual-pilot / wingman waypoints | AI wingman uses leader's route offset by formation slot |

## 50.9 Acceptance criteria

- `AC-50.1` Great-circle distance and bearing agree with the geodesic reference (Karney) within 1 m and 0.01° over 10⁴ random point pairs (including antipodal-adjacent).
- `AC-50.2` NAV mode tracks a 200 km multi-leg route with cross-track error rms ≤ 150 m in a 15 m/s crosswind and completes leg switching without overshoot > 500 m at Mach 0.8.
- `AC-50.3` ALT hold within ±15 m rms and HDG hold within ±1° in smooth air; TF mode maintains the commanded AGL ± 25 m over a 50 km rugged test route without ground contact in 100 trials.
- `AC-50.4` INS drift: median position error after 60 minutes unaided is 0.5 nm ± 0.2 nm (100 simulated runs); a GNSS fix resets it to < 20 m within 3 s.
- `AC-50.5` Declination from the bundled WMM agrees with the published model values within 0.5° at 100 test points.
- `AC-50.6` Waypoint validator rejects all invalid test routes (out-of-boundary, below minimum altitude, impossible TOT) and accepts all valid ones in the 200-case corpus.

---

# 51. USER-GENERATED CONTENT

`P5 · M4 (seed/config sharing, file import/export) → M5 (full editor) → M7 (community hub)` — Players can save and share mission configurations and define their own battlefields (outline §90–§94). The design rule is **content is data, data is validated, validated data runs in a sandbox** (ADR-09, NN-6). A shared mission can never execute arbitrary code, fetch arbitrary URLs, or exceed entity/time budgets.

## 51.1 Intent

- Sharing a mission should be as easy as pasting a link; editing one should be possible without a manual.
- The generator is the on-ramp: "Generate → tweak → save → share".
- Browser-native distribution without a mandatory server: files, links and (later) an optional community hub.

## 51.2 What a user mission contains (outline §90)

A user mission is a `MissionDef` (§25.2) with these outline-required fields mapped to schema keys:

| Outline field | `MissionDef` key |
|---|---|
| LOCATION, BOUNDARY | `battlefield` (anchor lat/lon, shape, dimensions, altitude limits) |
| AIRBASES | `bases` (+ airport overrides/improvised strips) |
| FACTIONS | `factions` |
| PLAYER AIRCRAFT | `spawns[].allowed_aircraft`, default loadout, livery |
| ENEMY AIRCRAFT | `groups[]` with aircraft ids/loadouts/skill |
| WEAPONS | Loadout restrictions (`rules.allowed_stores`) and group loadouts |
| MISSION TYPE | `meta.type` (template family, §27) and objective graph |
| OBJECTIVES | `objectives[]` |
| WEATHER, TIME | `environment` |
| AI SETTINGS | `rules.ai`, group `skill/doctrine/roe` |
| SEED | `provenance.seed` (+ per-stage streams for generated parts) |

**Package formats.**

| Format | Extension | Content | Use |
|---|---|---|---|
| Canonical JSON | `.mission.json` | Stable key order, UTF-8, ≤ 2 MB | Authoring, diffs, tests |
| Binary pack | `.msnpack` | zstd of canonical CBOR + thumbnail (WebP 640 × 360) + metadata + SHA-256 | Sharing (typically 8–120 KB) |
| Mission link | URL fragment | See §51.4 | Messaging |
| Mission bundle | `.msnbundle` (zip) | One or more missions + custom GeoPack subset (optional) + custom battlefields | Campaign-style packs, LAN parties |

Metadata: `title`, `author` (free text, optional), `description` (≤ 1 000 chars), `tags`, `recommended_players`, `estimated_minutes`, `content_requirements` (aircraft/weapon ids and versions, GeoPack id), `license` (default CC BY 4.0 for user text; user may choose), `created`, `parent` (content hash of the mission this derives from).

## 51.3 Mission Editor (browser)

A DOM-based editor (ADR-20) sharing the world map renderer (§9):

| Panel | Function |
|---|---|
| **Map canvas** | Place/move/rotate bases, spawn slots, groups, objectives, zones (circle/polygon), routes (waypoint types, §50.6), SAM sites with coverage rings, markers; snap to airports/roads; measure tool |
| **Layers** | Terrain, radar coverage (line-of-sight preview using the actual LOS service §16.9), threat rings, boundary, civilian traffic |
| **Groups & units** | Add from the content registry filtered by faction/era; set count, skill, loadout, doctrine, ROE, activation |
| **Objectives & logic** | Visual node graph (condition → action blocks) that compiles to the mission DSL (§25.5) with live type-checking; text view of the DSL for power users |
| **Environment** | Date/time (with a sun-position preview), weather presets and sliders, wind, season |
| **Test & validate** | "Play from here", "Fast-forward simulation" (headless sim at 8–16× to check solvability), validator panel (see §51.5), force-balance estimator (§26.8) |
| **Generate & edit** | Run the generator (§26) with a request, then edit any entity; edits mark entities `user_edited` (preserved on regenerate-with-same-seed in "merge" mode) |
| **Undo/redo** | Command-pattern history (500 steps), autosave to IndexedDB every 30 s |

Mission editing always works offline. The editor refuses to publish a mission that fails the validator (§51.5) but allows saving drafts.

## 51.4 Sharing

1. **Seed link** (generated missions): `#m=<seed>&v=<game>&d=<datapack>&c=<config>` ≈ 70–120 characters (§26.3). Regenerates identically for the same provenance.
2. **Compact link** (edited generated missions): `#m=…&p=<base64url(deflate(patch))>` where `patch` is a CBOR diff against the regenerated mission (entity adds/removes/field changes). Practical limit ≈ 1.5 KB of URL (~1 100 bytes of patch); larger edits use a file.
3. **File import/export:** drag-and-drop a `.msnpack`/`.mission.json` into the window; file picker; OPFS storage as "My Missions" with thumbnails and tags; sort/filter/search.
4. **Direct share to a lobby:** in a multiplayer lobby the host's mission is transferred to guests over the data channel (§33.4); no third-party hosting required.
5. **QR code** for the compact/seed link (LAN party, mobile viewing).
6. **Community hub (M7, optional):** a separate service for publishing/rating/downloading; out of scope for launch (OQ-10 added below); the file and link formats are designed so that a hub is a thin layer over them. If built, hub content is validated server-side with the same validator and hosted as `.msnpack` only.

See OQ-10 (§0.5) for the open hub scope and moderation question.

## 51.5 Validation and sandboxing (NN-6)

All imported content passes the same pipeline whether it comes from a file, link, lobby or hub:

| Stage | Checks |
|---|---|
| **Syntax** | Size limits (JSON ≤ 2 MB; pack ≤ 512 KB compressed; ≤ 10 MB bundle), UTF-8, depth ≤ 24, string lengths (title ≤ 80, text ≤ 1 000), schema `mission/N` known |
| **Schema** | JSON Schema validation; unknown keys are rejected in strict mode (warned in lenient mode); numeric ranges (coordinates, altitudes, speeds) |
| **Referential integrity** | All ids resolve (aircraft, weapons, factions, airports, groups, zones); required content installed (else "missing content" with a download prompt) |
| **Caps** | ≤ 500 groups, ≤ 2 000 triggers, ≤ 99 waypoints per route, entity caps per profile (§0.6.4: a mission exceeding the *High* cap is rejected; above *Recommended* it is flagged "heavy") |
| **Logic compile** | DSL → bytecode; static checks: types, reachable phases, no trigger loops without cooldown, a Resolution phase exists |
| **VM limits (runtime)** | Bytecode ≤ 256 KB; VM memory ≤ 1 MB; ≤ 50 000 VM instructions per sim tick (budget shared by all triggers; exceeding disables the offending trigger and logs `MissionEvent::TriggerBudgetExceeded`); no unbounded loops (only `for_each(group)` with ≤ 512 iterations); no I/O, no clocks other than sim time, no randomness except seeded `random(p, stream)` |
| **Solvability** | Same as §26.9: path to success exists; spawn safety; boundary legality; fuel feasibility for the player aircraft; optional headless AI run |
| **Content safety** | All user text is plain text (HTML-escaped on display, no markup); no external URLs rendered as links (shown as text); profanity filter optional in public contexts; image thumbnails re-encoded (not trusted) |
| **Integrity** | SHA-256 over canonical JSON; the content hash is the mission id; tampering changes the hash (not a security boundary: missions are trusted only as data) |

Malicious or malformed content fails closed with an actionable message ("Trigger t_17 loops without cooldown"). Fuzz tests (cargo-fuzz targets for the parser/validator/VM, §61.10) run in CI.

## 51.6 User-defined battlefields (outline §91)

The *Battlefield Designer* (a mode of the world map, §9.7) lets players define:

| Parameter | Control | Constraints |
|---|---|---|
| Centre point | Click, coordinate entry, airport, bookmark | Latitude within ±85° (§45.8) |
| Width × height (or radius) | Handles and numeric fields; presets §8.6 | 10 km–500 km (Extreme 1 000 km requires High profile) |
| Shape | Rectangle, ellipse/circle, polygon (§8.2) | Convex or concave polygon with ≤ 32 vertices; area ≤ cap |
| Altitude limits | Floor/ceiling | §8.4 |
| Spawn locations | Per side: airports, carriers, air-start boxes | ≥ 1 per side; ≥ 40 km apart unless 1v1 close-range |
| Airports | Auto-detected + manual add/remove/convert to improvised | §10 |
| Weather & time | Presets and manual | §24 |
| Objectives | Predefined templates or logic | §25 |

The designer shows the live **battlefield validator** report (§8.8), the offline readiness meter (§45.4), and estimated download size (§44.9). A saved battlefield is `battlefield/1` JSON and can be reused by the generator ("generate a mission in my battlefield") or the sandbox.

## 51.7 Location tools (outline §92–§94)

Search, coordinate entry and bookmarks are specified with the world map (§9.5, §9.6, §9.10) and are reused by the editor: bookmarks can be pinned as "favourite airports", "dogfight areas", "carrier battle areas", "mountain arenas" and "multiplayer locations" categories; bookmarks are included in `.msnbundle` files and can be shared as links (`#b=<lat>,<lon>,<z>,<name>`).

## 51.8 Versioning and migration

Every user file carries `schema` and `content_requirements`. The loader keeps a migration chain `mission/1 → mission/2 → …` (pure functions, tested on a corpus of saved files, §64.5). Missions made for an older content version load with the content versions they pin when available; otherwise a *compatibility report* lists substitutions (e.g. missing weapon replaced by the closest class) and asks to continue. Forward-incompatible files (newer than the game) are refused with an "update required" message.

## 51.9 Modding boundary

Launch scope is **data-only UGC** (missions, battlefields, bookmarks, liveries). Custom aircraft, weapons, models and scripts are **out of scope for launch**; the data-driven content architecture (§62) makes content packs possible later through signed manifests and the same validator. Custom liveries are 2048² images validated for size/format and sandboxed to the livery slot (§14.6).

## 51.10 Acceptance criteria

- `AC-51.1` Round-trip: any generated mission exported as `.msnpack` and re-imported produces a byte-identical `MissionDef` (canonical JSON hash equal) on WASM and native.
- `AC-51.2` A seed link regenerates an identical mission for the same provenance on two browsers (golden hash) and a compact link applies its patch to produce the same mission as the original export.
- `AC-51.3` Fuzzing: 10⁷ random/mutated inputs through the parser+validator yield no crash, no unbounded memory (≤ 64 MB) and no validation pass of a limit violation.
- `AC-51.4` VM: a mission with an infinite trigger chain is terminated by the budget within one tick, logs the event, and the sim stays above 55 fps.
- `AC-51.5` The editor can build and run a mission with 3 groups, 2 objectives and 2 triggers in ≤ 10 minutes by a first-time tester (usability test, 5 participants, ≥ 4 succeed).
- `AC-51.6` User text containing `<script>` and HTML is displayed verbatim as text in all UI surfaces.
- `AC-51.7` A bundle containing a custom battlefield outside ±85° is rejected with a precise message.

---

# 52. REPLAY & PHOTO MODE

`P5 · M5` — Missions are replayable; players can pause or slow combat and use a cinematic camera (outline §104, §105, §137). The design is **snapshot-based** (ADR-09) and reuses the multiplayer snapshot protocol (§33.5), so replays are small, seekable and tolerant of version changes, and cinematic moments emerge from play without scripting.

## 52.1 Intent

- "Watch the fight again": inspect kills, follow missiles, review battle flow and AI behaviour.
- "Make a shot": photo mode for stills; clip export for video — fully offline in the browser.
- Replays are first-class: always recorded by default, small, and shareable as files.

## 52.2 Replay data

Two complementary recordings, chosen by role:

| Kind | Content | Fidelity | Typical size (10 min, 75 aircraft) | Who records |
|---|---|---|---|---|
| **State-stream replay (SSR)** | The sequence of replicated world snapshots the *client* received (delta-compressed, §33.5), plus events, plus a keyframe every 10 s | What the client was told (fog-of-information applies, so the player's own view) | ≈ 5–8 MB after zstd | Every client (single-player: local sim snapshot stream at 10 Hz) |
| **Omniscient snapshot replay (OSR)** | Authority-side snapshots of **every** entity (no relevance filtering or fog) at 20 Hz in the same delta format, plus a compact **AI debug track** (per AI aircraft at 5 Hz: task, target, tactic, utility scores, perception set) and all events; keyframe every 10 s | Ground truth; supports AI inspection and "what the bandit saw" | ≈ 8–15 MB after zstd (150 entities) | Host / server / single-player |

Both kinds are **snapshot-based** (ADR-09): playback never re-runs the simulation, so replays survive Sim Core changes and allow free scrubbing.

- **SSR** is version-tolerant for *viewing* (the decoder keeps old schema decoders), renders what the player could know, and is the default shareable format.
- **OSR** is the "director's cut": omniscient, supports AI debug overlays and the truth-view map, and re-rendering at any quality. It is also version-tolerant through the same decoder chain; missing newer fields are shown as "n/a". The determinism of the Sim Core (ADR-10) is used only for *testing* (golden mission hashes), not for playback.

**Container** `.replay` (zstd'd chunk file): header (`{kind, game_version, content_version, mission_hash, duration_s, start_tick, players, thumbnail}`) | index (chunk offsets by time) | chunks: keyframe + deltas for 10 s each | event track (kills, launches, hits, objective changes, radio) | marker track.

**Events index** (for search/jump): `launch(weapon, shooter, target)`, `hit`, `kill`, `death`, `objective_change`, `phase_change`, `near_miss (< 20 m)`, `ejection`, `landing`, `carrier_trap`, `scramble`, `radio_call`, `tag(user)`.

## 52.3 Recording

- Always on by default in a **rolling ring buffer** (last 15 min at ≈ 0.6 MB/min) in memory; at mission end (or on pause→save) the buffer is committed to OPFS as a `.replay` (user can disable; "auto-keep last 5 unpinned, pinned unlimited").
- In multiplayer each client records SSR; the host/server also records OSR. If the **host leaves**, clients commit their buffers immediately (§33.7) so the player still has a replay of the session.
- Recording cost: writing deltas is a by-product of replication (no extra serialisation on clients; single-player produces the same stream at 10 Hz, ≤ 0.1 ms/frame).
- Compression in a worker (`CompressionStream('zstd')` where available, else WASM zstd); chunk write via OPFS `SyncAccessHandle` in a worker.

## 52.4 Playback and seeking

- Playback reconstructs world state by applying deltas to the nearest preceding keyframe; the renderer consumes the same interpolated entity stream as live gameplay (the replay camera is just another view; interpolation delay 0 for replays because all data are known).
- **Seek:** O(1) to a keyframe (every 10 s) then replay ≤ 600 deltas ≈ 25 ms; scrubbing at 60 fps is smooth. OSR uses the same mechanism (its keyframes are larger, ≤ 1.5 MB, still seeking in ≤ 100 ms).
- **Speed:** 0.05×–8× (and reverse at ≤ 1× via keyframe stepping with cache); frame-step; pause with free camera; 120 fps slow-mo interpolation uses the tick history (no sub-tick physics: Hermite interpolation of position/attitude between 60 Hz states).
- **Timeline UI:** scrub bar with event icons (kills, launches, hits, objective), phase bands, intensity graph (the director's intensity curve, §29.4), bookmarks; keyboard: `J/K/L`-style speed controls, `,`/`.` frame step.

## 52.5 Replay camera and inspection tools (outline §105)

| Tool | Behaviour |
|---|---|
| **Camera modes** | Cockpit (of any aircraft with a cockpit model), chase (smoothed behind), tracking (fixed point following a target), orbit, free-fly (6-DOF), fixed tripod, **missile cam** (rides a selected missile: nose-mounted or trailing; seeker view with FOV and gimbal limits), **target cam** (picture-in-picture for the locked target), **kill cam** |
| **Switch targets** | Cycle through entities, click on map/world, or jump to the next event of a type |
| **Inspect kills** | Kill list with shooter/weapon/target/time/range/aspect; jump to 10 s before; "show geometry" draws the launch envelope, missile path and the reason for the hit/miss (e.g. `flare decoy at t+3.2 s`) |
| **Follow missiles** | Auto-chase of any selected missile through intercept; overlays: guidance mode, seeker lock cone, time-to-impact, notch/chaff events |
| **Battle-flow review** | Overlay of tracks, radar coverage, detection lines (who saw whom), intensity graph, ownership/frontline timeline (campaign) |
| **Observe AI behaviour** | OSR only: per-aircraft AI debug (§63.5) — state machine, target, utility scores, perception, predicted trajectories |
| **HUD replay** | HUD of the recorded aircraft reproduced from the recorded aircraft state (OSR: complete; SSR: reconstructed from replicated fields, approximate for the rest) |
| **Audio** | Re-synthesised from events and entity states (the audio engine is driven by the same state; no audio recording stored) |

## 52.6 Multiplayer replay rules

- **Fog-of-information respect.** SSR contains only what that client received; sharing an SSR does not leak hidden information. OSR is omniscient and is saved only by the host/server/single-player; it is offered to clients after the mission ends (opt-in of the host: "Share full replay") to prevent mid-mission leaks.
- **Host leaves:** each client keeps its own SSR buffer (§33.7) and the lobby shows "Session ended; replay saved".
- **Anti-cheat note:** replays are not authoritative for any competitive result (the server result record is).
- **Spectating live** uses the same renderer/decoder with +2 s delay (OQ: allowed in 1v1 only for spectators, §31).

## 52.7 Instant replay and kill cam

On death, kill, or a flagged cinematic event, the player may view a 6–12 s clip from the ring buffer: tracks from the missile's perspective, 3rd-person of the shot, optional slow-motion segment around impact (0.25× for 1.5 s). Key `R` to skip; setting for auto-offer; disabled in competitive 1v1 until the round ends. Kill-cam content is limited to information the player may know in that mode (SSR-based).

## 52.8 Photo mode (outline §104)

**Entry.** Single-player: pauses (or slows to 0.05×). Co-op/live multiplayer: the sim keeps running; the player's aircraft is left on autopilot "hold" for ≤ 20 s (or the player enters as a **spectator camera** after death); photo mode in live multiplayer is therefore a *free camera detached from the player* using the SSR stream. Replay: the full toolset.

| Control | Range / behaviour |
|---|---|
| Camera | Free-fly (6-DOF, speed 1 m/s–2 km/s, collision with terrain optional), orbit about a target, tripod; roll; "follow point" |
| Lens | FOV 8°–110° (equivalent focal length 14 mm–400 mm full-frame via `f = 18 mm / tan(fov_v/2)` with sensor 24 mm height; formula `fov_v = 2·atan(12 mm / f)`), tilt/shift (shift ±20 %) |
| Depth of field | Focus distance (pick on screen), aperture f/1.2–f/22, bokeh shape (circular, blade count 5–9); implemented as a gather-based DOF in post |
| Exposure | EV ±6, white balance (2 500–12 000 K), contrast, saturation, vignette, film grain, bloom, lens flare, chromatic aberration (all toggles) |
| Look presets / LUT | 12 colour grades (neutral, cinematic, sunset, mono, …) as 3D LUTs, user-importable `.cube` (≤ 65³) |
| Scene | Hide HUD/UI, hide specific aircraft/effects, clean sky (clouds off), **time of day** and **cloud/haze visual overrides** (render-only; in a mission it never affects the sim), sun/moon position |
| Smoke/explosions | Freeze/step effects by pausing particle time; effect-time scrub (−2…+5 s of replay) to catch the perfect frame |
| Formation tools | "Gather" helper to line up aircraft for formation shots (replay/sandbox only; moves *visual* copies in replay; in sandbox moves actual entities under photo-mode control) |
| Composition | Rule-of-thirds/golden grid, horizon level, aspect crop (16:9, 21:9, 3:2, 1:1, 9:16), safe-area |
| Capture | Resolution up to **7680 × 4320** via **tiled rendering** (4× or 16× tiles of ≤ 2048² with sub-pixel jitter accumulation, 16–64 samples for AA) — capture takes seconds to a minute; PNG (8/16-bit) or JPEG/WebP; EXIF-like metadata (lat, lon, altitude MSL, sim UTC time, mission seed) in the file; auto-saved to the in-app **Gallery** (OPFS) and offered as a download |
| Subjects | Aircraft shots, missile shots (missile-follow camera), landscapes, explosions, formations, sunsets, geographic landmarks (bookmark list jumps to e.g. major landmarks), carrier ops |

Photo mode never alters mission results; in competitive modes it is disabled until the round ends.

## 52.9 Video capture

- **Real-time clip:** `MediaRecorder`/`WebCodecs` records the canvas (+ audio via a `MediaStreamAudioDestinationNode`) while playing; limited by device performance; H.264/AV1/VP9 depending on support [verify].
- **Offline render (replay → video):** deterministic frame-by-frame render at a fixed dt (e.g. 60 fps, 4K), independent of live performance: for each output frame the replay is positioned, the engine renders, `VideoEncoder` (WebCodecs) encodes and a muxer (MP4/WebM) writes to OPFS. 1080p60 averages 16 Mbit/s; 4K60 ≈ 45 Mbit/s. A 60 s clip renders in 1–10 minutes depending on GPU.
- **Camera paths:** keyframed Catmull–Rom (centripetal, α = 0.5) splines on position and slerp/squad on orientation; keyframe fields: position, orientation, FOV, focus distance, exposure, time, speed; ease in/out; speed ramps (time remap curve) for the classic slow-mo. Path preview in a mini-viewport.
- **Audio in exports:** engine, weapons, ambience, radio (re-synthesised), music optional (own tracks only; the game's music is licensed for in-game use and video export allowed under the licence stated at export [verify with licensing]).

## 52.10 Cinematic director (outline §137)

An automatic camera director can produce highlight reels and drives the kill cam:

```text
event interest score  I(e) = w_type · (1 + 0.5·rarity) · (1 + 0.3·proximity_to_player) · (1 + 0.4·intensity_at(t))
   w_type: ace_kill 1.0, boss_kill 1.0, multi_kill 0.9, kill 0.7, missile_evasion 0.6, near_miss 0.5, carrier_trap 0.4, launch 0.2, ...
   rarity from event frequency in the session (ln(1/p_event)/ln(100) clipped [0,1])
```

Events are clustered (merge within 6 s); each selected cluster is shot with a **shot template**: *Missile chase* (rear-quarter, 80 m), *Fly-by* (tripod ahead of the path, tracking), *Orbit* (360° over 8 s), *Cockpit cut* (pilot view), *Wide establish* (battle overview), *Impact close* (slow-mo). Templates use geometric constraints (terrain clearance, line of sight, no occluder) evaluated via the LOS service and a short sample of candidate camera positions scored for subject size, framing, and background interest. Cinematic cues from gameplay (`trigger_cinematic_cue`, §25.5, and escalation moments §28) are tagged. Output: a timeline the user can accept, edit or export.

Emergent cinematics (outline §137) are not scripted: the dimensional requirements — missile trails, dozens of aircraft entering, bombers overhead, aces from clouds, SAM barrages, carrier launches, valley engagements, large dogfights — are achieved by the mission systems, and the director merely *finds* them; a **Cinematic Score** metric (events-per-minute weighted by I) is logged per mission and used in generator tuning (§69).

## 52.11 Storage and management

| Item | Default limit | Behaviour |
|---|---|---|
| Replays | 20 auto (50 MB each max) + pinned unlimited | Auto-rotate oldest unpinned; sizes shown; export/import files |
| Gallery | 500 images | Thumbnails (256 px), tags, search by location/mission; bulk export |
| Clips/videos | 20 | Stored in OPFS; downloadable; quota-aware |
| Metadata index | IndexedDB | Fast listing without loading files |

Storage pressure follows the global policy (§44.5.5, §64.6): replays are evicted before saves and caches are never evicted before replays unless pinned.

## 52.12 Edge cases

| Case | Handling |
|---|---|
| Replay from an older build | SSR and OSR: decoder chain for old schemas; unknown newer fields shown as "n/a"; a replay newer than the game is refused with an "update required" message |
| Missing content (aircraft not installed) | Placeholder model; warning |
| Truncated file | Playable up to the last complete chunk |
| Huge replay (> 1 h, 150 aircraft) | Chunked streaming from OPFS; seek by index |
| Photo mode with streaming terrain | Streaming continues; capture waits until the tile set is complete (tile-complete gate) before tiled render |
| Photo mode in the midst of a multiplayer match | Allowed as spectator camera only; the player's aircraft is protected by autopilot hold only in cooperative PvE (not 1v1) |

## 52.13 Acceptance criteria

- `AC-52.1` A 10-minute, 75-aircraft SSR is ≤ 8 MB; seeking to any time completes in ≤ 100 ms (p95) on the Recommended profile.
- `AC-52.2` OSR playback reproduces the recorded authority state exactly: for 100 recorded missions, every entity's replayed position/attitude at each keyframe equals the recorded value (bit-exact) and interpolated values between snapshots deviate by ≤ 0.5 m from the sim's own 60 Hz states.
- `AC-52.3` Missile-cam follows a missile from launch to impact without losing the subject (angular error < 5° from the missile axis) for 1 000 recorded shots.
- `AC-52.4` Tiled capture at 7680 × 4320 produces seam-free output (maximum edge difference between tiles ≤ 1 LSB in a test scene) and completes in ≤ 90 s on the Recommended profile.
- `AC-52.5` Offline video render is deterministic: two renders of the same replay/camera path hash-match frame-by-frame (same GPU/driver) and the audio is in sync within 1 frame.
- `AC-52.6` Ring buffer commit on host loss: killing the host connection at a random time leaves a valid replay on the client in ≥ 99/100 trials.
- `AC-52.7` Photo mode in a live co-op session never changes any gameplay state on the server except the autopilot-hold flag (verified by state-hash comparison).

---

# 53. UI / UX

`P1 → P10 · M1 (shell, settings) → M4 (full flow)` — The application shell, menus, world-map interface, briefing, loadout, lobbies, debrief and all out-of-cockpit UI (outline §99 world-map UI, §107 flight-assistance presets as presented to the player). In-cockpit displays are §54; the tactical map is §55.

## 53.1 Intent and principles

1. **Fly in under 60 seconds.** From a cold start of the installed PWA to being in the air: ≤ 5 clicks, with defaults that work (last used location, aircraft and mission type).
2. **Depth on demand.** Every screen has a *simple* presentation and an *advanced* panel; advanced is never required.
3. **Geography first.** The world map is the hub of sandbox, 1v1 and co-op; it is always one click away.
4. **Never lose the player's work.** Autosave of drafts, loadouts, bookmarks and settings; confirmation only for destructive actions with undo where feasible.
5. **Everything is reachable by keyboard and gamepad** (focus order, visible focus, no hover-only actions) and works at 1280 × 720 minimum up to 4K and ultrawide.
6. **Readable at a glance in the heat of combat:** in-flight overlays are sparse, high-contrast, and removable (§57).
7. **Truthful states:** loading, downloading, caching and network states are shown with real numbers (bytes, tiles, percent) — never fake progress.

## 53.2 UI technology (ADR-20)

| Concern | Choice |
|---|---|
| Menu/overlay layer | DOM + CSS using **web components** (Lit-class, ≤ 60 KB gzipped runtime), CSS custom properties for theme tokens, CSS `contain`/`content-visibility` for heavy lists, and the **View Transitions API** where available for screen changes (fallback: CSS fade) |
| State | A small signals-based store; screens subscribe to slices of `AppState` (settings, profile, session, lobby, content registry); no global mutable singletons |
| HUD, MFDs, in-cockpit displays | GPU render-to-texture in the Render Worker (§54.2) — *not* DOM |
| In-flight overlays (radio text, objective banner, chat, scoreboard, pause menu) | DOM layer over the canvas, `pointer-events: none` except while a modal is open; at most 150 DOM nodes active during flight, updated ≤ 10 Hz (budgeted in §0.6.3) |
| Map canvases (world map, tactical map) | Canvas/WebGPU map layer with DOM chrome (toolbars, panels) |
| Fonts | Variable sans (UI), tabular-numerals monospace (instruments), and an HUD stroke font (SDF, §54.2); Latin subsets embedded, CJK/Cyrillic/Arabic subsets loaded on demand |
| Icons | SVG sprite (≈ 250 icons) with `currentColor`; icon + text labels in menus; no icon-only critical controls |

## 53.3 Application state machine and screen graph

`GameFlow` (main thread, §61.2) owns the app state; the Sim Core has a smaller state machine (§61.4).

```text
BOOT → COMPAT_CHECK → MAIN_MENU
MAIN_MENU → { QUICK_FLIGHT | SKIRMISH_SETUP | DUEL_SETUP | COOP_LOBBY | CAMPAIGN | MY_MISSIONS | HANGAR | REPLAYS_GALLERY | SETTINGS | DATA_STORAGE | CREDITS }
(setup screens) → WORLD_MAP → BATTLEFIELD_PREVIEW → MISSION_CONFIG → BRIEFING → LOADOUT → LOAD_PLAN (§44.9) → MISSION_RUNNING → DEBRIEF → { RETRY | NEW_SEED | MAIN_MENU | REPLAY }
MISSION_RUNNING ⇄ PAUSE_MENU ; MISSION_RUNNING → PHOTO_MODE (§52.8)
```

| Screen | Purpose | Key elements |
|---|---|---|
| **Compat check** (first run or on failure) | Verify WebGPU/WebGL2, SharedArrayBuffer (cross-origin isolation), OPFS, WASM SIMD/threads, gamepad API | Green/amber/red per feature, remedies ("enable hardware acceleration", "use a Chromium-based browser"), the degraded modes in §0.6.2 |
| **Main menu** | Entry | Big "Fly" button (resumes last config), mode tiles, profile chip, online/offline indicator, storage meter, update banner |
| **Quick Flight** | One-click sandbox at last/random location | Location chip, aircraft chip, difficulty chip, "Surprise me" (random hot spot + seed) |
| **World map** (§9) | Choose location, airports, battlefield | See §53.5 |
| **Mission config** | Sandbox parameters (§30) with presets; seed card | Collapsible groups; "Randomise" per group; seed share |
| **Briefing** | Situation, objectives, forces, threats, weather, timeline, radio freqs, routes | Map with overlay layers; collapsible text; radio "play" of briefing audio (optional); airbase and tanker info |
| **Loadout (Hangar)** (§14) | Aircraft, stores, fuel, livery, performance planner | Aircraft carousel, stations diagram, mass/CG/range/take-off planner (§10.4), presets and saved loadouts |
| **Load plan** | Download/cache progress | Real byte counts, tile counts, ETA; "Start now" when allowed (§44.9); mission tips |
| **Pause menu** | In-flight | Resume, settings (subset), controls, restart, abandon (confirm), help, photo mode, save/quit (campaign/solo checkpoints §64.4) |
| **Debrief** | Results | Objectives, score breakdown (§25.11), kills timeline, flight path replay thumbnail, statistics (§65), rewards (campaign), buttons: Replay (§52), Retry, New seed, Share seed |
| **Lobby** (§32) | Co-op/1v1 | Player slots, roles, ready state, mission summary, load readiness (tiles), chat, voice, host controls, invite blob / LAN code (§33.3) |
| **Campaign** (§34–§43) | Strategic map etc. | Campaign layer UI (§35) |
| **My Missions** (§51) | Library | Cards with thumbnails, tags, search, import/export |
| **Hangar** | Browse aircraft/weapons | 3D viewer, specs, performance charts, unlock status |
| **Replays & Gallery** (§52) | Review and media | Timeline thumbnails, share, export |
| **Settings** | See §53.7 | Tabs: Gameplay, Controls (§58), Video, Audio (§56), Accessibility (§57), Network, Data & Storage (§45.4), Language, Advanced |
| **Credits & Data** | Licences and attribution | Generated from provider descriptors (§45.9) |

## 53.4 In-flight UI layers (outside the cockpit displays)

| Layer | Content | Behaviour |
|---|---|---|
| **Objective banner** | Current primary objective; updates with a short highlight when changed | Auto-hide after 6 s; recallable with a key |
| **Radio subtitles** | Speaker, text of each radio message (§56.6), priority colouring | On by default; ≤ 3 lines; fade; large-text option (§57) |
| **Notifications** | Kills, assists, weapon events, warnings not shown on the HUD ("Wingman down", "Reinforcements entering") | Stack of ≤ 3; severity colours + icons (not colour alone) |
| **Quick comms wheel** | Radio commands (§19.13, §20.6): wingman orders, request tanker/AWACS, bingo | Radial menu, hold key/button; gamepad stick select |
| **Scoreboard / roster** | Players, kills, status (co-op/1v1) | Hold key |
| **Chat** | Text chat (§33.5), with history | Opens on key; click-through otherwise |
| **Map overlay** | Tactical map as full-screen overlay (§55) | Freezes nothing in multiplayer; pauses in single-player if the option is set |
| **Help overlay** | Control cheat sheet (generated from current bindings), mode-specific tips | Key toggle |
| **Network indicator** | Ping/loss glyph in multiplayer | Hover/expand shows the debug panel (§63.6) |

All overlay positions snap to a 12-column safe-area grid and respect user scale (80–150 %) and the notch/ultrawide safe margins.

## 53.5 World Map UI (outline §99)

The behaviours are specified in §9 (camera, picking, layers, search, coordinate entry, selection, bookmarks); this section fixes the **UI contract**:

| Required capability (outline §99) | UI element | Spec reference |
|---|---|---|
| Zoom | Wheel/pinch/`+ −`, scale bar, zoom-to-selection; globe↔flat blend | §9.2 |
| Pan | Drag, arrow keys, edge-scroll option, inertia | §9.2 |
| Location search | Search field (`/` to focus), suggestions with type icons (city, airport, region, coordinates, bookmark), recent history | §9.5 |
| Airport selection | Click airport glyph → info card with runway(s), type, facilities, ownership, ILS, "Set as start / target / home" | §9.4, §10 |
| Arbitrary location selection | Click-and-hold or "Place battlefield here" tool; coordinate entry | §9.6, §9.7 |
| Distance measurement | Ruler tool: great-circle distance (km/nm/mi), bearing true/magnetic, multi-point polyline, ETE at selected speed | §9.7, §50.3 |
| Battlefield preview | Live boundary overlay, airports inside, sim-critical tile coverage and offline readiness meter, estimated download size, validator warnings | §8.8, §44.9 |
| Campaign ownership | Faction colouring of airports, frontline, supply lines, fog (§42) | §35 |

Right-side panel has three states: *Location* (info), *Battlefield* (size, shape, rules), *Forces* (preview of expected sides). A persistent footer shows attribution (§45.9), coordinates under the cursor, elevation, and `GeoInspector` data on `Alt+click`.

## 53.6 Flight assistance presentation (outline §107)

Assist is selected by **chips** (Full / Standard / Reduced / Manual, §11.11.3) in the mission configuration, pause menu and first-run, with a *Details* disclosure revealing every dial (§59.2). A preview strip shows what each chip changes (graphic: protection envelopes, auto-trim, auto-rudder, G-limiter, stall protection). Switching mid-mission is allowed in sandbox/co-op host-permitting; competitive modes lock it (§31).

## 53.7 Settings

| Tab | Contents |
|---|---|
| Gameplay | Units (aviation/metric/imperial), realism dials (§59), assists, auto-start, hints, time acceleration defaults, kill cam, language |
| Controls | Bindings editor (§58), calibration, profiles |
| Video | Renderer (WebGPU/WebGL2/auto), resolution scale (50–200 %), upscaling (FSR-class/TAA), quality presets (Low/Medium/High/Ultra) and per-feature toggles (§60.4), frame cap, VSync (via rAF), FOV, HUD brightness/colour |
| Audio | Master/bus volumes, output device (where supported), dynamic range, voice settings, subtitles (§56, §57) |
| Accessibility | §57 |
| Network | Transport preference, voice, bandwidth cap, display name, interpolation delay override |
| Data & Storage | §45.4, replay/gallery quotas, clear cache, export/import all data (§64.6) |
| Advanced | Developer flags (`?debug=…`), telemetry consent, experimental features |

Settings are stored in IndexedDB (versioned with a migration chain) and are exportable/importable as JSON; every setting has a default, tooltip, and "reset" affordance. A **performance advisor** suggests presets from the benchmark (§60.8).

## 53.8 Design system and layout

- **Tokens:** CSS variables for colour roles (`--surface-0…3`, `--text`, `--accent`, `--danger`, `--warn`, `--ok`, `--friend`, `--foe`, `--neutral`), spacing scale (4, 8, 12, 16, 24, 32 px), radii, elevations, type scale (12/14/16/20/28/40 px), and motion tokens (150/250/400 ms).
- **Themes:** *Dark* (default), *Light*, *Night-Cockpit* (red-shifted, low luminance), and *High Contrast* (WCAG AAA colours). Theme honours `prefers-color-scheme`/`prefers-contrast`/`prefers-reduced-motion`.
- **Colour semantics:** blue = friendly, red = hostile, yellow/amber = unknown or caution, green = neutral/OK, white = own ship. Colour is **never the sole carrier**: each affiliation also has a shape (friend circle, foe diamond, unknown quatrefoil, neutral square), consistent between tactical map, radar and HUD (§57).
- **Components:** button, toggle, slider with numeric field, segmented control, dropdown, chip, tabs, list/grid card, dialog, toast, tooltip, tree, table, map toolbar, key-binding capture field. Each component has keyboard, gamepad and screen-reader behaviour, specified once and tested in a component gallery (Storybook-class) as part of CI.
- **Responsive:** layouts defined for widths 1280–1919 (compact), 1920–2559 (standard), ≥ 2560 (large/ultrawide); no horizontal page scroll; panels collapse to drawers on compact.
- **Motion:** transitions ≤ 250 ms; parallax and large motion disabled by reduced-motion (§57).

## 53.9 Input in menus

Mouse/keyboard (Tab order, arrow navigation in grids, type-ahead), **gamepad** (D-pad/stick spatial navigation, A/B accept/back, shoulder buttons for tabs, on-screen prompts matching the detected controller family), and **HOTAS** (hat/trigger mapped to menu navigation when no flight input is active). Touch is not a launch input (desktop target) but the DOM layer uses pointer events and 44 px minimum hit targets so that a later tablet release is feasible.

## 53.10 Localisation and units

- All user-visible strings are **keys** (`title_key`, `text_key`, …) into locale bundles (ICU MessageFormat for plurals/gender/selectors); no string concatenation in code.
- Launch language: English; architecture and string-length tolerance (+40 % expansion, pseudo-localisation test) ready from M1; target languages by M5: FR, DE, ES, PT-BR, JA, KO, ZH-Hans, RU (OQ for the final list).
- `Intl.*` for numbers/dates/lists; logical CSS properties for RTL; the HUD uses fixed aviation conventions (not localised) except for text warnings (`Bingo`, `Pull up` are localised separately, §56.9 uses fixed English brevity voice at launch).
- **Units:** altitude ft/m, speed kn/km/h/m/s/Mach, distance nm/km/mi, pressure inHg/hPa/mmHg, mass kg/lb, temperature °C/°F; defaults by locale with aviation default (ft, kn, nm). Units affect display only; the sim is SI.
- Radio voice and text are locale-aware (§56.6).

## 53.11 Onboarding, training and help

- **First-run flow:** compat check → choose control device (auto-detect, prompt) → quick calibration → choose assist (Standard default, "I'm new" → Full) → *First Flight* mini-mission.
- **Training missions** (data-driven, using the mission system §25): 1 Takeoff & landing, 2 Basic manoeuvres & energy, 3 Guns, 4 IR missiles, 5 Radar & BVR, 6 Defensive (RWR, flares, notch), 7 Navigation & air-to-air refuelling, 8 Ground attack, 9 Carrier ops (post-launch). Each has a success rubric, a replay of a "good example", and skip-ahead; training missions are ordinary `MissionDef`s (no special code).
- **Contextual hints:** a hint engine watches player events (e.g., stalls, repeated missile deaths, crashed on landing) and suggests a short tip (max 1 per 5 min, dismissible, per-hint "never show again"); hints are text with optional highlight of HUD elements.
- **Help:** searchable in-game encyclopaedia (aircraft, weapons, systems, brevity codes, glossary — Appendix G), control cheat sheet, links to the community docs.
- **Hardcore opt-outs:** all assists, hints and overlays can be disabled.

## 53.12 Loading and startup experience

| Phase | Target |
|---|---|
| App shell paint (cached) | ≤ 1.0 s; first visit ≤ 2.5 s on a 50 Mbit/s link |
| Interactive main menu | ≤ 2.0 s cached; ≤ 5 s first visit (base pack streams in the background, menu does not wait) |
| First visit download | Shell ≤ 40 MB and base pack ≤ 120 MB (§0.6.6; progress with pause/resume); the player can use the menus and hangar immediately |
| Mission load | Per §44.9; shows tips, aircraft specs, and the briefing for reading |

An **update** is delivered via the Service Worker; the new version applies on next launch or by explicit "Reload to update" (never mid-mission; multiplayer sessions pin the build).

## 53.13 Telemetry and privacy

No analytics by default. Opt-in anonymous telemetry (performance counters, crash reports with scrubbed stack traces, feature usage) is requested once with a plain-language explanation; the data schema is documented in the Advanced tab; local logs are exportable. No network identity is required to play.

## 53.14 Acceptance criteria

- `AC-53.1` From a cached cold start, Quick Flight reaches "in the air, wheels up" within 60 s in a scripted test (Recommended profile, auto-start enabled).
- `AC-53.2` Every interactive element is reachable and operable via keyboard alone and via gamepad alone (automated focus-traversal test + manual checklist), with a visible focus indicator (contrast ≥ 3:1).
- `AC-53.3` Pseudo-localisation (+40 % length, accented characters, RTL mirror) produces no clipped text in any screen at 1280 × 720.
- `AC-53.4` In flight, the DOM node count stays ≤ 150 and UI scripting ≤ 0.3 ms/frame (p95).
- `AC-53.5` The Compat check correctly identifies each missing feature in a matrix of 12 simulated browser configurations and offers the specified degraded mode or remedy.
- `AC-53.6` Settings export/import round-trips all settings exactly; a settings file from the previous minor version migrates without loss.
- `AC-53.7` Usability test (8 new players): ≥ 7 can start a free flight, take off and land in the first session with Standard assist (post-tutorial).

---

# 54. HUD

`P3 · M1 (core HUD) → M3 (full symbology) → M5 (MFD page set)` — The cockpit, its displays, the HUD and the camera system (outline §95 cockpit, §96 camera, §97 HUD). HUD behaviour reflects aircraft avionics (outline §97). The fidelity tiers A/B/C and the display page list are defined in §16.10; this chapter specifies how they are drawn, driven and used.

## 54.1 Intent

- Give the pilot the right information at the right time: instant flight state at a glance, tactical awareness without staring at a menu.
- Be *aircraft-specific*: a 1990s-style HUD shows different things than an advanced helmet-cued one (data-driven layouts, §62).
- Render at display rate on the GPU, conformally with the world, in cockpit and overlay modes.

## 54.2 Rendering architecture (ADR-20)

1. **Retained-mode vector HUD scene** (Rust/TS "HUD graph": lines, polylines, arcs, text runs, ladders, brackets) built per frame from the `HudState` produced by the Sim Core (read-only view of the aircraft state, sensors, weapons, nav; at 60 Hz, double-buffered in shared memory).
2. **Rendering:** a dedicated pass in the Render Worker draws the HUD graph with an **SDF stroke renderer** (anti-aliased lines with constant physical width) and **MSDF glyph atlas** for the HUD font into a **HUD render target** (2048 × 2048 RGBA16F for the main HUD; 512–1024² per MFD), updated every frame at ≥ 60 Hz and only when dirty for MFDs not on screen. Cost: ≤ 0.35 ms (HUD) + ≤ 0.25 ms per visible MFD (Recommended, GPU).
3. **Collimation (HUD glass).** The HUD symbology is conformal: an angle `(az, el)` from the aircraft boresight maps to the same direction in the world regardless of small head movement. The HUD target is composited onto the combiner-glass quad by a shader that, for each pixel on the glass, computes the view ray from the eye through the pixel in aircraft axes and samples the HUD texture at

```text
u = tan(az) / tan(TFOV_h / 2),   v = tan(el) / tan(TFOV_v / 2)       az = atan2(r_y, r_x), el = atan2(−r_z, √(r_x² + r_y²))   (r: ray in aircraft body axes, x forward, y right, z down)
```

   with a glass-aperture mask and a soft vignette from the eye box (the HUD fades when the eye leaves a 120 mm × 80 mm × 160 mm eye-box volume). Total field of view (TFOV) is per aircraft data (Kestrel: 28° × 20°). In **overlay mode** (3rd person or a user-selected "HUD overlay" setting) the same texture is drawn as a flat screen-space overlay at a configurable scale.
4. **Colour and brightness.** Default HUD green (`#35ff6a` core, adding 0.6 px glow), alternate palettes (amber, white, cyan, red for night) and day/night brightness auto-adjusting from scene luminance (`L_hud = clamp(0.3 + 0.7·L_scene, 0.3, 1)`) with a manual override; the HUD never saturates against bright sky (contrast ≥ 3:1 against the background modelled by sampling a downsampled scene luminance under symbology, optional).
5. **Resolution independence:** all symbols are authored in milliradians/angle units; text height is set in mrad (default 12 mrad ≈ 0.69°) to remain legible at any resolution.
6. **Failure/degradation rendering:** flicker, partial blanking, fading and "FAIL" annunciators driven by the damage and electrical models (§18, §16.10), plus jitter under extreme vibration (G/buffet) — accessibility setting reduces jitter (§57).

## 54.3 Core symbology (all aircraft)

| Element | Definition / formula | Notes |
|---|---|---|
| **Boresight cross / waterline** | Fixed at the HUD datum | Reference for aircraft nose |
| **Flight path marker (FPM)** | Angles `(γ_h, γ_v) = (atan2(v_y, v_x), atan2(v_z, v_x))` in body axes (velocity vector relative to nose); caged at low speed (< 20 m/s) | Drawn as a circle with wings and a fin |
| **Pitch ladder** | Lines every 5° (labelled every 10°), conformal with the true horizon (using the local tangent plane at the aircraft, §7.8); negative pitch dashed; zenith/nadir symbols; horizon line with gap at the FPM | Declutters above 60° pitch |
| **Heading tape** | Top, ±30° window, tick every 5°, labels every 10°; selected-heading bug; magnetic/true annunciation | Scale from `NavSolution` (§50.3) |
| **Airspeed** | Left scale tape: IAS (kn) with Mach digital below 1.2 and above M 0.6; groundspeed/TAS selectable | Selected-speed bug (autopilot) |
| **Altitude** | Right scale tape: barometric MSL (ft), **radar altitude** below 1 500 ft AGL (shown as "R 450"), QNH set; altitude bug | Terrain warning interplay §54.9 |
| **Vertical speed** | Small caret beside the altitude tape (±6 000 ft/min) | |
| **G-meter** | Digital current (`1.0 G`), max stored (`MAX 7.2`), warning above 90 % of the structural limit | Colour amber at 85 %, red at 100 % |
| **AoA bracket / indexer** | Bracket on the left of the FPM in approach configuration (gear down): AoA error to the on-speed value | |
| **Bank scale / roll indicator** | Arc at the top/bottom with tick marks | |
| **Throttle/AB/Fuel** | Fuel remaining (kg or lb), flow, endurance (min); bingo/joker flags (§49.6) | Lower-left block |
| **Mode / master mode** | NAV, A/A, A/G, LAND, GUNS annunciation | |
| **Steering cue** | Waypoint steering line/diamond, time-to-go, distance | From the active route (§50) |
| **Objective cue** | Optional marker of the current primary objective's direction/distance (hidden in realistic modes) | Bearing & range text |
| **Autopilot modes** | Engaged lateral/vertical/speed modes and selected values | §50.4 |

## 54.4 Gun sight and ranging

- **Gun cross + LCOS (lead-computing optical sight) pipper.** When a target is locked (or in "snapshot" mode with a target designator), the pipper position solves

```text
p_t(t_f)  = r_t + v_t·t_f + ½·a_t·t_f²                      (predicted target position, own-ship position r_o at the moment of firing)
R(t_f)    = |p_t(t_f) − r_o|                                  t_f solved from  t_f = TOF(R(t_f))  by 3 fixed-point/Newton iterations,
                                                              TOF(R) from the gun-round ballistic table (muzzle velocity + drag)
bullet path (inertial): r_o + (v_o + v_muzzle·û_bore)·t + ½·g·t²           ⇒ û_aim chosen so that the bullet position at t_f equals p_t(t_f)
pipper_angle = angle(û_aim, boresight) projected into HUD coordinates (az, el)
```

  The **gun funnel** (a tapering corridor) is shown in Standard tier: its width at range `R` corresponds to the target wingspan `b` (`θ = b/R`, with `b` from the target's data when identified, else 11 m) so that a target filling the funnel is "in range". A "SHOOT" cue (flashing) appears when `R < R_gun,max` (default 1 000 m for A/A gun effective range) and the pipper is within the target's silhouette ± tolerance.
- **No radar lock (Reduced Assist / manual ranging).** The pipper becomes a reticle ring; the pilot **manually ranges** by span bracketing: the ring diameter `θ_ring` is adjustable (HOTAS slew) and the displayed range is `R = b / θ_ring` (small-angle), where `b` is the assumed span (set by target class preset: fighter 11 m, bomber 30 m, transport 40 m, small 8 m). A 11 m target filling a 11 mrad ring is at 1 000 m. Lead solution uses this manual range (error in range yields lead error, a skill expression). This is the "reduced-assist skill expression" referenced in §15.
- **Air-to-ground:** CCIP (continuously computed impact point) for unguided bombs and rockets, CCRP (release point) for dive/level: the CCIP marker is the ballistic impact point of the current velocity and release conditions (drag-modified trajectory integrated at 10 ms steps up to the ground or 40 s; ground intersection via the sim terrain layer, §44.6); the **bomb fall line** and release cue appear when the impact point is within the target designator.
- **Radar/target designator box:** a box around the locked target (bracket size ~ 40 mrad), with a range/closure/aspect readout (e.g. `R 8.4  Vc +310  A 20`), IFF symbol, and target ID if identified.

## 54.5 Weapons display and launch-zone indicators (DLZ)

Per selected weapon the HUD shows: weapon name, quantity, mode (e.g. `AAM-IR 4`, `GUN 480`), seeker status (`TONE` for IR lock / `SEEK` / `LOCK`), master arm state, and the **Dynamic Launch Zone (DLZ)** ladder beside the target box:

```text
range scale 0 – R_scale (auto-ranged to 1.25·Rmax) ;  markers:  Rmin ▸  Rne ▸  Rmax ▸  and the current range caret ◂ ;  fill colours: inside Rne = solid; between Rne and Rmax = hatched
```

- `Rmax`, `Rne` (no-escape), `Rmin` come from the DLZ baker (§15.8) evaluated for the current launch conditions (own speed, altitude, target aspect and speed) at 10 Hz via a precomputed interpolation table; the table query takes ≈ 1 µs.
- "**SHOOT**" appears inside `Rne` with valid lock; "**IN RANGE**" between `Rne` and `Rmax`; "**OUT**" beyond. For semi-active missiles an additional "**ILLUM**/SUPPORT" indicator (continuous STT required) and a time-to-impact countdown after launch ("TTI 14"); for active missiles "**PITBULL**" (active) after seeker activation. For IR missiles the audio growl and the cage box convey seeker lock (§56).
- Launch authorisation logic: master arm on; weapon selected; stores ready; lock criteria; own-ship constraints (g-limits, AoA); an "**Launch aborted**" message names the reason (e.g. `NO LOCK`, `TOO CLOSE`, `OFF BORESIGHT 45°`).
- Weapons-camera option (outline §96): see §54.11.

## 54.6 RWR, radar and notch indicators

- **RWR** (§16.10, §17) is drawn on a dedicated page (and optionally a reduced HUD strip): a polar scope with the player at centre; bearing azimuth, threat-class glyph, lethality ring (closer = nearer the centre), state (search / track / launch / missile guided), blinking for new/launch, audio per §56. HUD strip shows the top-priority threat bearing as an arrow at the HUD edge.
- **Radar page** (B-scope/PPI): range 10–320 km, azimuth sectors ±10°/±30°/±60°/±90°, bars, cursors, tracks with velocity vectors, TWS/STT annunciation, range/altitude readouts; presented for each radar mode (§16).
- **Notch indicator (realistic modes).** For each *track* the pilot holds (offence) the page draws a bar `closure rate vs v_notch`; the target is at risk of being lost when `|v_t·û_los| < v_notch` (§16.3.3). For *defence*, when an RWR emitter is tracking the player, the HUD shows the **beam angle**: the difference between the current aspect to the emitter and the perpendicular (90°) beam, and the "notch window" `θ_n = asin(v_notch/V)` using the threat's estimated generation (from RWR threat ID): green when inside the window, amber approaching it, with a "BEAM!" cue — allowing the classic notching manoeuvre to be performed instrumentally (not hand-held by AI).

## 54.7 Energy cue

Optional (Standard and Full tiers): a **specific excess power caret** `P_s = (T − D)·V / W` shown on the FPM as a small chevron whose vertical offset represents `P_s` (±300 ft/s full-scale, up/down) together with a **corner-speed marker** on the airspeed tape at `V_corner` for the current weight/altitude (the speed at which `n_aero,max = n_struct,max`, §11.12). The cue is computed from the same energy-manoeuvrability tables used by AI (§19.5). In Reduced/Manual assist it is hidden by default.

## 54.8 Warnings, cautions and advisories

| Level | Visual | Audio | Examples |
|---|---|---|---|
| **Warning** (red, immediate action) | Flashing text in the centre, red master warning light, border pulse | Continuous or repeating tone + voice | `MISSILE`, `PULL UP`, `ENGINE FIRE`, `STALL`, `OVER-G` |
| **Caution** (amber) | Steady/blinking text in the caution area; master caution | Chime + voice | `BINGO`, `HYD`, `GEN`, `LOW OIL`, `FUEL LEAK` |
| **Advisory** (cyan/white) | Message in the advisory line | Soft tick | `MASTER ARM ON`, `TANKER IN RANGE`, `WAYPOINT` |

Message rules: priority queue; maximum 3 simultaneous visible messages (the rest in the *Warnings* MFD page); acknowledge by key (silences audio, not the cue); deduplicated and rate-limited to avoid spam; localised text (§53.10). Order of priority: missile-terminal > ground collision > stall/departure > engine/fire > structural > hydraulic/electrical > fuel > others.

## 54.9 Low-altitude and obstruction warnings

- **GPWS-style:** "TERRAIN" when predicted ground contact along the velocity vector falls inside 8 s; "PULL UP" inside 4 s, accounting for the aircraft's current g-capability: `t_req = √(2·ΔH / (a_max))`-style check using the sim terrain layer (§44.6) look-ahead path (§16.9.3).
- **Obstruction cue:** within 100 m AGL (low-level mode), power lines, towers and bridges (feature layer, §47.5) within 1.5 km ahead are indicated by a bracket with distance and an audio "OBSTACLE" call; the cue is hidden if the realism dial disables it. Wire strikes are lethal.
- **Radar altimeter low-altitude alert:** audible tone and bug when AGL < the set minimum (default 150 m); changes with phases (landing config).
- **Ground-collision auto-recovery (Full Assist only):** pull-up manoeuvre takes control when `t_to_impact < 1.5 s` (optional "Auto-GCAS"), disabled by realism dial.

*Clarified in 0.2.2 (ADR-34):* this line (1.5 s) and §11.11.2 (4 s at the current g-capability) disagree. Auto-GCAS is not built in v0.1b; the opt-in flag in the input frame (§58.2) and the `gcas` block of `fcs/1` are kept, the pause menu leaves the option out, and the timing is decided when the feature is built. The warning strip's PULL UP is a time-to-impact rule (under 5 s at the present sink and below 400 m, silent on the wheels and in the landing configuration).


## 54.10 Cockpit and MFD system

- **Cockpit model:** each aircraft (that has one) provides a 3D cockpit mesh with named nodes for HUD glass, MFD screens, switches (visual state only), canopy, mirrors, instruments (analogue backup: airspeed, altitude, attitude, compass), pilot arms/hands (optional) and a canopy frame occluder. Minimum content per outline §95: HUD, speed, altitude, heading, radar, weapon, ammunition, fuel, target information. "More advanced aircraft can have additional displays": the `cockpit_layout.json` declares MFD count (0–3), page assignments, and softkeys.
- **MFD pages** (list in §16.10.2): Radar, Stores, Tactical Map (§55), Engine/Fuel, CM, Systems/Damage, Navigation, Targeting-pod, RWR, Warnings, Data-link. Page switching is by softkeys/buttons or HOTAS; the "cheat sheet" overlay lists all bindings (§16.10.3). Each MFD has 20 softkeys (5 per side) drawn by the HUD renderer with consistent labels; page content is rendered at a fixed 30–60 Hz independent of frame rate.
- **Minimal-cockpit (Tier A):** no MFDs; the HUD shows everything (overlay mode default), with a bottom info bar and floating target markers.
- **Clickable cockpit (P10):** ray-picked switches via mouse; initial release uses bindings only.
- **Cockpit comfort:** head-bob/inertia smoothing (G-based shifts, ≤ 2 cm), reduced option (§57); seat position adjust; canopy reflections subtle; windshield rain/ice effects tied to weather (§24).
- **Failures:** displays fail per the damage model (§18.8): MFDs blank, HUD degraded, analogue instruments remain; electrical bus handling per §16.10.

## 54.11 Camera system (outline §96)

| Camera | Description | Parameters |
|---|---|---|
| **Cockpit** (primary) | Eye at the design eye point with head motion (look-around via mouse/hat/head-tracking §58.7), G-lag offsets, canopy frame occlusion. Default FOV 70–90° vertical-equivalent (user adjustable 50–120°), zoom (hold) to 30° for target ID | Head limits ±135° yaw, ±60° pitch; smoothing 60 ms |
| **Third person** | Behind and above, locked to the aircraft's velocity-direction with a spring-damper (follow smoothing 0.25 s), pitch limit to avoid ground; reticle for aiming (Arcade mouse-aim) | Distance 20–60 m (scales with wingspan), height 25 % of distance |
| **Chase** | Wider, cinematic, slightly lagging (0.6–1.2 s lag), with speed-dependent FOV (+8° at max speed) and tasteful shake from buffet | Distance 40–150 m |
| **Free camera** | 6-DOF, speed 1 m/s–2 km/s, optional terrain collision; available in sandbox/replay/photo mode; in multiplayer: spectator only | |
| **Weapon camera** | Optional view from a launched missile (nose or trailing 15 m), gun-camera for cannon rounds, bomb-cam; PiP or full-screen; auto-return after impact + 2 s | Seeker-limited FOV (60°) for realism; disabled in competitive modes by option |
| **Target camera / padlock** | Camera keeps the locked/selected target in view (look-at lock with head limits) | Padlock disabled in Realistic mode |
| **Tower / external fixed** | Tripod cameras at airfields for take-off/landing (cosmetic) | |
| **Map camera** | World/tactical map overlays (§9, §55) | |

All camera parameters follow critically-damped springs (time constant tuned per camera); camera positions are clamped to remain 1.5 m above terrain/ship decks to avoid clipping; occlusion avoidance for third-person/chase (raycast against terrain/buildings with spring arm). Camera switching is instantaneous with a short cross-fade (80 ms) for chase/3rd-person transitions. Horizon-level (roll-stabilised) option for chase/free. Camera shake sources: buffet (AoA), gunfire, explosions, ground roll, afterburner; magnitude scaled by user setting (0–100 %, §57).

## 54.12 Behaviour by avionics, realism and damage

- HUD content is built from a layer list declared in `hud_layout.json` (data): each element has `required_systems` (e.g. radar for target box, INS for FPM), `min_tier` (A/B/C), and `realism_gate` (e.g. `objective_cue: easy-only`). Missing systems hide elements or substitute degraded variants (FPM without INS → "attitude only").
- The **declutter** keys cycle through 3 levels (full / reduced / minimum); automatic declutter in high-G/under-fire is optional.
- **Information gates** follow §16.11: in Normal and Hard, enemy markers appear only when sensed (radar/visual/datalink); labels differ; "floating target markers" (Arcade) show a 3D marker with distance for sensed targets.
- Damage effects: HUD pitch ladder drift (INS fail), lost speed tape (pitot fail → fallback to GPS speed with annunciation), "FAIL" boxes.

## 54.13 Data-driven definition

```json
{ "schema": "hud/1", "id": "hud_kestrel", "tfov_deg": [28, 20], "eye_box_mm": [120, 80, 160], "font": "hudstroke",
  "layers": [
    { "id": "ladder", "type": "pitch_ladder", "step_deg": 5, "label_every_deg": 10, "min_tier": "A" },
    { "id": "fpm", "type": "flight_path_marker", "requires": ["ins"], "cage_below_mps": 20 },
    { "id": "ias", "type": "tape", "channel": "ias_kn", "side": "left", "window": 200, "tick": 10 },
    { "id": "dlz", "type": "dlz", "requires": ["radar","weapon_selected"], "realism_gate": "normal+" }
  ],
  "warnings": [ { "id": "stall", "level": "warning", "text_key": "hud.stall", "audio": "stall_tone", "when": "aoa > aoa_stall - 1.5" } ]
}
```

Layers are implemented as a registry of Rust/TS "HUD element" types, so new aircraft can reuse elements; the validator rejects layouts referencing unknown element types or channels.

## 54.14 Acceptance criteria

- `AC-54.1` Pitch-ladder conformity: at 10 pitch/bank/heading combinations the horizon line aligns with the rendered terrain horizon within 0.1° (screenshot comparison) and the FPM aligns with the actual velocity direction within 0.1°.
- `AC-54.2` LCOS/lead solution accuracy: firing from the pipper at a constant-velocity target at 300–1 000 m produces hit rates ≥ 90 % in a simulation without pilot error; with a 5 % ranging error the miss distance matches the analytical prediction within 15 %.
- `AC-54.3` HUD rendering cost ≤ 0.35 ms (HUD) + 0.25 ms per MFD on the Recommended profile at 1080p, including text; no per-frame allocations (heap snapshot test).
- `AC-54.4` Collimation: moving the head within the eye-box leaves conformal symbols within 0.05° of their world directions (test with 100 head positions).
- `AC-54.5` DLZ values displayed equal the DLZ baker values for the same conditions within 2 % over 1 000 random shots.
- `AC-54.6` Camera modes: terrain-clearance clamp holds in 1 000 random low-level paths (no camera below terrain + 1.5 m); chase/third-person spring arm never clips through terrain.
- `AC-54.7` Warnings: the priority queue shows the correct top-3 messages in 50 scripted multi-failure scenarios; each warning's audio fires within 100 ms of the condition.
- `AC-54.8` Layout validator accepts all shipped `hud_layout.json` files and rejects a corpus of 40 invalid layouts with precise messages.

---

# 55. TACTICAL MAP

`P3 · M2 (basic) → M4 (full information model)` — The in-mission map shows the player, allies, enemies, objectives, airports, SAM sites, the battlefield boundary and geographic features; **what is shown depends on realism settings** and on what the player's side actually knows (outline §98). It is available as a full-screen overlay, as an MFD page (§54.10), as a picture-in-picture panel, and as the briefing map (§53.3).

## 55.1 Intent

- Answer: *where am I, where are my friends, where is the threat, where do I need to go, what am I allowed to know?*
- The same renderer as the world map (§9) with a different data source: the **player-side knowledge picture** (§42), not the ground truth (except in Easy/God view).
- Zero cost when closed; ≤ 0.6 ms GPU when open (Recommended).

## 55.2 Content and information policy

| Item | Easy | Normal | Hard | Simulator |
|---|---|---|---|---|
| Own aircraft + heading + track history | Yes | Yes | Yes (position may be INS-drifted, §50.2) | Yes (INS/GNSS state) |
| Allies (datalink) | All, labelled | All on datalink, labelled | Flight members + datalink contacts | Datalink-connected only; GPS-denied degrades |
| Enemy aircraft | True picture, labelled | Sensed tracks (radar/RWR/visual/datalink); class labels | Sensed tracks, no free labels; ID by interrogation/visual | As Hard + imperfect ID, ghost tracks under jamming |
| Enemy ground/naval | True picture | Seen/detected + intel-known fixed sites | Seen/detected; fixed sites by briefing intel | Seen/detected; sites as per intel level (§42.3) |
| SAM sites and threat rings | All, with ranges | Known sites with rings; unknown sites hidden until detected/emitting | Rings only for identified sites; ESM/RWR bearings otherwise | Estimated rings from ESM; no free data |
| Objectives | Markers + text | Markers | Brief icons; no distances | Briefing-only (waypoints) |
| Airports | All | All with ownership | Known | Known; status via intel |
| Battlefield boundary | Yes | Yes | Yes | Yes (always, for safety) |
| Geographic features | Terrain, rivers, roads, towns, names | Terrain, coast, roads, major towns | Terrain contours, coast | Coarse terrain contours only |
| Weather | Radar overlay | Radar overlay | Observed only | Briefing-only |

Rules: (1) The map never reveals information the pilot could not know in that realism tier. (2) Every non-ground-truth item shows its **provenance and age** (see §55.5). (3) Settings can individually override the table (§59.2 dials: `map_enemies`, `map_threat_rings`, `map_own_position`, `map_labels`).

## 55.3 Rendering and projection

- **Projection.** A local **azimuthal equidistant** projection about the map centre `(φ₀, λ₀)` gives true distances and bearings from the centre, which is exactly what a pilot reads off a tactical map:

```text
c = acos( sin φ₀ sin φ + cos φ₀ cos φ cos(λ − λ₀) )       (angular distance; c = 0 → point at centre)
k' = c / sin c   (k' = 1 at c = 0)
x = R · k' · cos φ · sin(λ − λ₀)                          (east)
y = R · k' · ( cos φ₀ sin φ − sin φ₀ cos φ cos(λ − λ₀) )   (north)         R = 6 371 008.8 m (mean radius, display only)
```

   Inverse for picking is the standard inverse azimuthal equidistant. Scale distortion at 500 km from the centre is < 1 % in the radial direction, so the display is accurate for every battlefield preset (Extreme = 1 000 km side → 500 km half-extent).
- **Orientation.** North-up (default), track-up, or heading-up; rotating maps keep text upright. Centre: ownship (default), free pan, or bullseye.
- **Scale.** Range ring set 5, 10, 20, 50, 100, 200, 500, 1 000 km (full-screen) with continuous zoom; scale bar; automatic declutter (labels and icons thinned by priority at small scales).
- **Base map.** GPU-rendered hillshade (from the terrain height atlas, §44.5.3), hypsometric tint, coastline and water, rivers (by Strahler order), roads and rail (by class), towns (by population) and the lat/lon graticule; map base resolution follows the same tile pyramid with its own LOD (map tiles from z ≤ 10 for a 100 km view). Contour lines (every 100/200/500 m by scale) via marching squares in a compute pass. Style presets: *Tactical* (muted greys, high-contrast symbols), *Terrain* (colour relief), *Night* (red-shift low luminance).
- **Grid.** Optional alphanumeric 10 km grid (A–Z columns, 1–99 rows, `K14` style) and bullseye rings (every 20 km) aligned to the mission's bullseye (§50.5); callouts such as "bandits bullseye 270 for 40" map to the same geometry.
- **Cost.** One draw for the base layer (cached per zoom/pan in a texture, re-rendered when moved > 15 % of the view), instanced symbols (≤ 4 000), thin-line pass for trails. Updates at 10 Hz for tracks (interpolated at display rate), 1 Hz for slow layers.

## 55.4 Symbology

Symbols follow a MIL-STD-2525-*inspired* scheme (shapes/frames; **not** reproducing official symbol artwork):

| Affiliation | Shape | Fill | Notes |
|---|---|---|---|
| Own | Chevron / arrow | White | Heading arrow with 60 s velocity leader |
| Friend | Circle | Blue | Callsign label, optional flight lines |
| Hostile | Diamond | Red | Label: class (if known) or `BANDIT`/`BOGEY` |
| Unknown | Quatrefoil/clover | Yellow | Label: `UNK` |
| Neutral / civilian | Square | Green | Airliners, civilian shipping |
| Suspect / pending | Diamond with dashed outline | Orange | After partial ID |

**Domain frames:** air (open at the bottom), ground (box), sea surface (circle with wave), subsurface (hemisphere), installation (flat bar). **Modifiers:** velocity leader (line length ∝ speed × 60 s), altitude digits (flight level / 1 000 ft), track number, weapon icon if engaged/launched, jamming indicator, radar-emitting flag (for ESM), lock-on indicator (to own ship). Colour is never alone (§57) — shapes carry the meaning.

Other symbol families: airports (runway glyph with orientation, ownership colour ring, alert level ring §48.5), SAM sites (hexagon with engagement ring), ships/carriers, waypoints (typed icons, §50.6), objectives (flag + number), tankers (T + racetrack), markers/pings, missiles (small triangles with seeker cone), threat arrows from RWR (bearing-only lines with arcs).

## 55.5 Tracks and uncertainty

The map renders **track files** from the player's side knowledge model (§42.1), not entities:

```text
Track { id, side_known, class_level (§42.3), pos_est, vel_est, cov (2×2 position covariance), last_update_t, sources[], quality }
display: position ellipse (1σ) if σ > 200 m; fade alpha = clamp(1 − age/τ, 0.25, 1), τ = 20 s (sensor track), 120 s (visual/datalink), 600 s (intel)
dead-reckon: pos(t) = pos_est + vel_est·(t − t_last),   Δt = t − t_last
             σ(t) = σ₀ + k_σ · v_max(class) · Δt      (k_σ = 0.3: 1σ prior; hard bound: the true position is never farther than σ₀ + v_max·Δt from the last fix)
drop track: quality < 0.1 or age > 2·τ for sensed, or no update 15 min for intel
```

Provenance is shown on hover (`RADAR 12 s ago`, `RWR bearing only`, `DATALINK (Eagle 2)`, `INTEL 8 min`). Bearing-only RWR/ESM lines extend from ownship; two bearings from different aircraft or successive positions triangulate to a **fix** (ellipse shrinks). Under jamming, ghost tracks and range-gate pulls appear with low quality and an `ECM` flag; the player learns to cross-check.

## 55.6 Overlays

| Overlay | Description |
|---|---|
| **Threat rings** | SAM engagement envelopes: simple circles (known max range) in Easy/Normal; in Hard/Simulator, *terrain-masked* envelopes computed from the site's polar horizon profile (§16.9.2) at the player's altitude (recomputed on altitude change > 200 m): the region where the site could see an aircraft at that altitude; dashed where uncertain |
| **Radar coverage** | Own/allied radar coverage at a selectable target altitude and RCS reference; shows blind zones and horizon limits |
| **Route** | Waypoints with leg bearings/distances, ETE/ETA, fuel at each waypoint, TOT markers, turning radius arcs |
| **Restricted zones / boundary** | No-fly, kill boxes, boundary penalty zone (red fade) |
| **Range rings** | From ownship: weapon DLZ rings for the selected weapon (Rmax/Rne), fuel range (bingo ring), radar range |
| **Weather** | Cloud cover, precipitation (radar-style), turbulence, wind barbs, freezing level, fronts |
| **Bullseye** | Reference point with radials; bullseye callouts mirrored |
| **Tracks history** | Fading trails (30–300 s), optional |
| **Datalink/Network** | Links between friendly units; jamming areas |
| **Campaign layers** | Ownership, frontline, supply (campaign only; §35) |

## 55.7 Interaction

| Action | Input |
|---|---|
| Open/close map | Key `M` / gamepad button (configurable); `Tab` cycles overlay ↔ MFD ↔ PiP |
| Pan/zoom | Mouse drag/wheel; stick + triggers; arrows; "centre on me" key |
| Select entity | Click: info card with provenance, range, bearing, closure, aspect, class, speed/altitude (if known), threat assessment |
| Designate target | Right-click → `Designate`: shares an on-map cue with flight members (§33.5 `designate(target)`) and the radar (cue to slew/lock) |
| Ping/mark | `Ctrl+click` ping (30 s lifetime, visible to team), shape/colour marker, text label |
| Measure | `R` ruler: distance/bearing between two points or from ownship; shows ETE at current ground speed |
| Waypoint edit | Click to add, drag to move, right-click to delete/insert; type picker; altitude/speed fields (§50.6) |
| Orders to AI | Right-click on map → AI flight menu (go to / attack / escort / RTB / hold), subject to command rights (§20.6) |
| Filter | Toggle layers by category; quick presets (*Air picture*, *Ground picture*, *Navigation*, *Clean*) |

In single-player, an option **pauses** the sim while the map is open (default off; on in Easy). In multiplayer it never pauses. When open as an overlay the 3D view dims by 40 % and continues rendering at reduced rate (30 Hz) to save GPU, unless the map is a PiP.

## 55.8 Modes of presentation

- **Full-screen overlay** (default `M`): full information, drawing and orders.
- **MFD page:** 1:1 scope; limited interaction via softkeys (zoom, centring, declutter, cursor slew) and HOTAS; renders into the MFD texture (§54.2).
- **PiP:** 25 % screen corner minimap, north-up/track-up, ≤ 2 range scales, no interaction; hidden by default except Easy.
- **Briefing map:** same renderer with editing disabled and the mission's planned overlays (routes, threats, objectives).
- **Spectator/replay map:** truth view (OSR only) vs player-knowledge view toggle (§52).

## 55.9 Multiplayer and information sharing

Team knowledge is merged through the datalink model (fused tracks from teammates' sensors) and arrives as part of replicated state (§33.5), subject to relevance and fog-of-information. The map shows the *merged* picture; tracks from a teammate under jamming or without a datalink path are excluded. Pings and markers replicate as small events (≤ 64 per team). In 1v1 the map defaults to **off** or ownship-only in Realistic rule sets (§31).

## 55.10 Edge cases

| Case | Handling |
|---|---|
| Map centred beyond ±85° or across the antimeridian | Azimuthal projection handles it; wrap-around not needed |
| Too many symbols (> 4 000) | Declutter by priority (threats > friendlies > neutral), clusters at small scales (count badge) |
| Loss of GNSS/INS drift | Ownship symbol shows an error circle (§50.2) |
| Streaming tiles missing | Map uses coarser levels; no blank base map |
| Jammed datalink | Allies disappear after datalink loss timeout (10 s); shows "LINK LOST" |
| Colour-vision modes | Shapes + patterns (§57) |

## 55.11 Acceptance criteria

- `AC-55.1` Azimuthal equidistant projection: forward/inverse round-trip error < 1 m for 10⁴ random points within 1 000 km of the centre; radial distance from the centre equals the geodesic distance within 0.5 % (WGS84 vs sphere display tolerance).
- `AC-55.2` Information policy: for each realism tier, no hidden entity appears on the map in a 100-mission automated test comparing displayed tracks to the player's knowledge set (zero leaks).
- `AC-55.3` Map open cost ≤ 0.6 ms GPU/≤ 0.5 ms CPU (p95) with 1 000 tracks and 20 threat rings at 1080p (Recommended).
- `AC-55.4` Terrain-masked threat envelopes agree with the LOS service (§16.9) within 100 m at 200 random bearings per site.
- `AC-55.5` Track uncertainty: the true position lies within the displayed 2σ ellipse for ≥ 93 % of samples across 1 000 simulated tracks.
- `AC-55.6` Orders issued through the map change AI tasks within 1 s and are reflected on all clients (multiplayer test).

---

# 56. AUDIO

`P3 · M1 (engine sounds, tones) → M4 (full mix, radio)` — Audio contributes to both immersion and situational awareness (outline §100–§102): engines, afterburners, guns, missiles, explosions, radar, warning tones, radio, wind, ambience; aircraft sound distinct; positional audio lets players determine direction of aircraft, missiles, explosions, nearby and distant combat. Radio provides contextual information without becoming repetitive.

## 56.1 Intent and principles

1. **Information first, spectacle second:** every threat has an unmistakable, directional cue (§56.7).
2. **Physical where cheap:** speed-of-sound delay, Doppler, air absorption, occlusion, and cockpit/outside filtering emerge from the same state as the sim; they are never faked per event.
3. **Procedural over sampled where scale demands it:** engines and wind are synthesised from state (RPM, thrust, Mach, AoA) so every aircraft/regime sounds right without thousands of loops.
4. **Never repetitive:** variation by randomisation within perceptual bounds, recency penalties for voice lines, and layered stems.
5. **Browser-realistic budgets:** ≤ 8 % of one core in the audio thread; ≤ 200 MB decoded audio memory; ≤ 40 ms output latency.

## 56.2 Engine architecture (Web Audio)

| Layer | Implementation |
|---|---|
| Graph | **Web Audio API** `AudioContext` (48 kHz, `latencyHint: "interactive"`), master bus → limiter → destination |
| DSP | Custom **AudioWorklets** (WASM, Rust) for: engine synthesis, spatialisation/mixing of positional voices, Doppler resampler, reverb sends; the main thread only sends parameter updates |
| State feed | The Render Worker/Sim Worker write an `AudioState` ring in SharedArrayBuffer at 60 Hz: listener pose/velocity, up to 96 emitters `{pos, vel, type, params}`, cockpit state; the worklet reads it lock-free (single-writer ring with sequence numbers). Without SAB: messages at 30 Hz via `MessagePort` |
| One-shots | Pre-decoded `AudioBuffer`s (Opus → PCM at load) for gunfire, explosions, switches; streamed `MediaElement`/chunked decoding for music and long ambience |
| Spatialisation | Per-voice distance model + HRTF binaural (headphones) or VBAP panning over 2.0/5.1/7.1 speakers (selected by output channel count); first-order ambisonic (FOA) bus for ambience beds; head rotation from the listener pose (including head tracking, §58.7) |
| Voices | Hard cap 96, soft cap 64 per profile (Minimum 32); stealing by priority × audibility `A = priority·gain/(1 + d/50 m)`; stolen voices fade over 30 ms |
| Loudness | Target −16 LUFS integrated (game mix), true peak ≤ −1 dBTP; a final soft limiter; user dynamic-range presets (§56.10) |
| Autoplay policy | The `AudioContext` is created/resumed on the first user gesture (the main-menu "Fly" click); a "click to enable audio" fallback is shown if suspended |

## 56.3 Sound design inventory (outline §100)

| Category | Content | Method |
|---|---|---|
| **Engines** | Per aircraft: idle spool, mil power, afterburner light-off thump, AB roar, spool-down, flameout, windmill; internal (cockpit) vs external perspectives | **Hybrid synthesis:** harmonic oscillator bank (fundamental = RPM/60 × blade count, with blade-pass and shaft harmonics) + filtered noise (turbine hiss, jet noise ∝ v_jet⁸ Lighthill-style scaling for level: `L ∝ 80 log₁₀(v_j)`) + sample layers for AB and compressor stall; per-aircraft parameter set in data (`audio_profile.json`: blade count, spool curves, formant filters, burner character, level offsets) so each aircraft sounds different |
| **Aerodynamic** | Wind rush (∝ dynamic pressure q), buffet rumble (AoA), transonic rumble, sonic boom (event), gear/flap/canopy/airbrake motion, landing-gear thump, tyre squeal/rumble, runway texture | Procedural noise bands driven by q, AoA, Mach, gear state; one-shot samples for boom/gear |
| **Weapons** | Cannon (burst/continuous, dry fire; barrel spin-up), rocket launch, missile launch (rail whoosh + motor), missile in flight (motor hiss, Doppler), bombs releasing/falling whistle, flares/chaff dispensing | Layered one-shots (3–6 variations each) + procedural motor loops with Doppler |
| **Impacts & explosions** | Small/large/fuel-air/ordnance explosions, ground impacts, debris, structure creak, aircraft hit (rounds on metal), cockpit hit | Layered one-shots with distance filters; delay = d/c |
| **Sensors/cockpit** | RWR tones, radar lock tone, STT, missile alert, stall horn, GPWS voice, master warning/caution, switch clicks, G-strain breathing & grunts, canopy sealed/unsealed | Samples + synthesised tones; priorities (§54.8) |
| **Radio** | Voices (AI, tower, AWACS, tanker, wingman), static, squelch, push-to-talk clicks, voice chat | §56.6 |
| **Environment** | Wind, rain, thunder, ocean, wildlife (biome), city hum, airfield ambience, carrier deck, crowd? (none) | Spatial beds (FOA) + point emitters; biome tables (§46.3) |
| **Music** | Adaptive score | §56.8 |
| **UI** | Menu navigation, confirm/deny, map pings | Short samples |

**Aircraft distinctiveness (outline requirement).** Each aircraft gets a unique parameter set and at least two signature samples (AB light-off, spool). Validation: a blind-listening test with 8 testers must identify ≥ 80 % of aircraft pairs as different at equal throttle (art/audio gate, §66.4).

## 56.4 Propagation model

For a sound emitter at distance `d` with source level `L₀` (dB SPL at 1 m) and speed of sound `c = 20.05·√T` m/s (T in kelvin, from the atmosphere model §11.3; ≈ 340.3 m/s at sea level ISA):

```text
L(f, d) = L₀ − 20·log₁₀(d / 1 m) − α(f)·d / 1000 − A_occ − A_ground(…)        dB          (spherical spreading + air absorption + occlusion)
delay   = d / c                                                                   s           (clamped to ≤ 30 s; explosions at 3 km arrive after ≈ 8.8 s)
Doppler: f' = f · c / (c − v_r)                v_r = closing speed of the source towards the listener (negative if receding); clamp |pitch shift| ≤ ±0.45 octaves
```

Air absorption `α(f)` (design values, dB/km at 20 °C, 70 % RH — ISO 9613-2 Table 2 [verify]):

| f (Hz) | 63 | 125 | 250 | 500 | 1 000 | 2 000 | 4 000 | 8 000 |
|---|---|---|---|---|---|---|---|---|
| α (dB/km) | 0.1 | 0.4 | 1.0 | 1.9 | 3.7 | 9.7 | 32.8 | 117 |

Implementation: 4-band shelving filter per voice (125 Hz–8 kHz bands) with gains `−α_band · d`, updated at 30 Hz; temperature/humidity effects scaled from the weather system (humidity 10–100 % interpolated with the table ±30 % in the 2–8 kHz bands). Beyond 5 km the high-frequency content is below −100 dB and effectively filtered, giving the distant "rumble" for far explosions and jets.

- **Occlusion/diffraction.** Terrain between source and listener (LOS test at 10 Hz, §16.9 service) applies `A_occ = 12 dB` + a low-pass at 1.5 kHz, with a 150 ms crossfade (no pops). Inside buildings/canyons: reverb send increase.
- **Cockpit vs external.** `exterior mix` → canopy transfer function (low-pass 3 kHz, −18 dB broadband at closed canopy; open canopy −3 dB), plus interior cockpit sounds. 3rd-person cameras use exterior mix at the camera position with the player's aircraft as a close emitter.
- **Reverb.** Three convolution/FDN reverbs by environment class: *open air* (short, quiet), *valley/canyon* (long 1.5–3 s), *hangar/airbase* (0.8 s), *interior cockpit* (0.05 s); sends driven by terrain geometry scans (8-ray sky/terrain openness estimator at 5 Hz).
- **Sonic boom.** When a supersonic aircraft's Mach cone passes the listener (i.e. the angle between the vector from the source's retarded position to the listener and the negative velocity vector crosses `μ = asin(1/M)`), a boom event plays: double-bang N-wave sample, level falling off with altitude roughly as `h^{-3/4}` (far-field N-wave scaling) and growing with aircraft length/weight, delay by the cone geometry. Booms are audible in the cockpit only to others (the pilot outruns it) — except when the aircraft decelerates through Mach 1.
- **Speed-of-sound consistency.** The delay applies to *all* distant events; the sim never waits for sound. A distant explosion's flash precedes its bang.

## 56.5 Spatial audio (outline §101)

- **Direction:** HRTF (Web Audio `PannerNode` with `panningModel: "HRTF"` for ≤ 24 nearest emitters; custom block-based HRTF convolution for more) or VBAP; elevation cues through HRTF spectral cues; front/back disambiguation via a slight low-pass for rear sources and head-tracking (when available).
- **Distance cues:** level, air absorption, reverb ratio, initial time delay; far sources are narrowed (reduced angular width) for localisation clarity.
- **What the player can determine:** *aircraft direction* (engine emitter with Doppler), *missile direction* (motor hiss + RWR/MAWS tone panning), *explosions* (delay + bass), *nearby combat* (gun bursts, missile launches within 5 km get priority), *distant combat* (distant rumble layer: a procedural "battle bed" level that grows with the intensity director's count of recent events within 30 km, filtered by absorption).
- **Accessibility:** mono down-mix, left/right balance, visual sound indicators (screen-edge direction rings for threat/launch/lock sounds, §57), and sound-event captions ("[Missile launch, right rear]").

## 56.6 Radio system (outline §102)

The AI emits **semantic events** (§19.13); the radio system maps them to phrases:

```text
event → { speaker, channel, priority, params } → template selection (weights, recency penalty) → grammar expansion → voice clip assembly → playback
```

**Channels:** Flight (intra-flight), Team (all friendlies), Tower/ATC (visual-ATC voice from the control tower), AWACS/GCI, Tanker, Ground (JTAC-like), Carrier ops, Enemy (intercepted, optional in Hard). Each message has priority 0–5 (0 = emergency); a queue per channel with **preemption** of lower priority, **cooldowns** per type (≥ 8 s for repeated contact calls), and a rate limit of ≤ 12 messages/min per channel in busy combat.

**Phrase library (brevity codes).** Based on public-domain-style brevity vocabulary (Appendix G): `Fox One/Two/Three`, `Splash`, `Bandit`, `Bogey`, `Bullseye`, `Tally`, `Visual`, `Blind`, `Padlock`, `Break`, `Defending`, `Pitbull`, `Bingo`, `Joker`, `Winchester`, `Mud`, `Spiked`, `Naked`, `Mayday`, `Eject`, `Cleared hot`, `Abort`, etc. Example (matches the outline's examples):

```text
[AWACS]   "Bandit group, bullseye two-seven-zero, forty, angels twenty, hot."       (picture/threat call)
[WINGMAN] "Fox two."                                                                 (IR missile launch)
[AWACS]   "Warning. Missile launch detected."                                        (missile warning)
[AWACS]   "Enemy reinforcements entering the battlespace."                           (escalation, §28)
```

**Voice production.** Launch: high-quality recorded phrase banks (≈ 900 phrases/fragments × 6 voices (3 male, 3 female) per language) with **concatenative assembly** for numbers (0–9, 'niner', 'tree', 'fife'), bearings, callsigns (from a 120-word callsign vocabulary, `data/callsigns.json`; custom callsigns fall back to phonetic alphabet spelling), altitudes ('angels' + number), and directions (clock positions). On-device neural TTS is **not** used at launch (cost/size). Prosody: per-phrase stress/urgency variants (calm / urgent / panicked), chosen from threat level.

**Realism effects:** distance/jamming-dependent static (noise filtered bandpass 300–3 000 Hz, level ∝ jam power, drops at J/S > 20 dB), squelch tails, clipped transmissions under damage, bandpass radio EQ, position-based "over-the-horizon" loss using the LOS service, and encrypted-data-link icons when relevant. Enemy radio: not translated; subtitles show `[ENEMY] …` as flavour (Hard).

**Non-repetition.** Template pools (≥ 6 variants per common message), weighted random with recency penalty `w = w₀·(1 − 0.8·e^{−Δ/10 messages})`, speaker personality variation, and context terms (altitude, weather) to avoid sameness.

**Subtitles.** All radio is subtitled (§53.4, §57); text derives from the same event/params (so they always match), localised.

**Player voice interactions.** The player's radio menu (comms wheel §53.4) issues commands; the corresponding player "voice line" is played (TTS-free recorded) and the AI acknowledges (`"Two."`, `"Wilco."`, `"Unable, bingo fuel."`).

## 56.7 Warning tones and cockpit audio

| Cue | Sound | Spatialisation |
|---|---|---|
| RWR search | Soft periodic tick at the scan rate (per threat class pitch) | At threat bearing (HRTF) |
| RWR track/lock | Steady tone, rising with threat priority | Bearing |
| RWR launch | Rapid warble (6 Hz) | Bearing |
| Missile guided on ownship | Continuous alarm + voice "Missile, [clock position]" | Bearing + clock call |
| Own radar lock tone | Short chirp on STT | Centre |
| IR seeker tone | Growl, pitch ∝ signal strength (lock = high steady) | Centre |
| Stall/AoA | Stall horn (or voice "Stall") at α > α_warn; rising pulse rate with AoA | Centre |
| Over-G | Voice "Overstress" + tone | Centre |
| GPWS | "Terrain, terrain" / "Pull up, pull up" (§54.9) | Centre |
| Bingo/joker | "Bingo" with caution chime | Centre |
| Master warning/caution | Distinct tones (red/amber priorities, §54.8) | Centre |

All cues respect mute filters (RWR volume/mute, threat-only filters, §16.10), accessibility remaps (visual-only mode), and hearing-aid mode (limit dynamic range, boost 1–4 kHz).

## 56.8 Music director

- **Adaptive score**: stems (percussion, bass, pads, brass/strings, motif) crossfaded by an **intensity state** from the mission director (§29.4 intensity director): *calm*, *tension*, *contact*, *combat*, *climax*, *resolution*, with hysteresis (min 12 s per state) and beat-aligned transitions (quantised to 2 bars).
- **Rules:** music is silent during critical audio (missile warning, radio priority ≥ 3) by ducking −12 dB and low-pass; **silence is a feature**: ≥ 30 % of transit/cruise time has no music; the carrier/airfield themes are subtle.
- Content: 12–15 composed pieces at launch, ~60 min of stems; streamed Opus (~96 kbps) ≈ 45 MB; music volume separate bus; "no music" option; user's own music is **not** supported at launch (browser limitations; OQ).
- Exported video audio (§52.9) can include the game's music only if the licence allows (note in export dialog).

## 56.9 Localisation of voice and text

- Launch: English voice (brevity codes are English by convention). Text subtitles localised from M5; voice localisation for later languages optional (callsigns and numbers per language).
- Warnings can be spoken in the player's language (GPWS-type voice set), configurable.
- Placeholder rule: a missing voice clip falls back to a subtitle + non-speech tone, never silent.

## 56.10 Mixing, buses, and options

```text
Master ─┬─ Cockpit/Aircraft (engine, wind, structure)   ─ ducked by warnings
        ├─ Weapons & Explosions
        ├─ World/Ambience (FOA bed, point emitters)
        ├─ Radio (voices, static)                       ─ priority ducks others −6 dB
        ├─ Warnings & Tones                              ─ top priority, never ducked
        ├─ Music                                         ─ ducked by radio/warnings
        ├─ Voice chat                                    ─ separate, per-player gain
        └─ UI
```

Options: per-bus volume, dynamic range (**Cinema** / **Standard** / **Night** (compressed), **Hearing aid**), output mode (auto / headphones HRTF / stereo / 5.1 / 7.1 where the browser exposes channels), mono, audio device selection (`setSinkId` where available [verify]), radio subtitles/captions, sound-source indicators (§57), mute-on-focus-loss, and voice-chat controls (§33.5 voice, push-to-talk limitations stated).

## 56.11 Assets, pipeline and budgets

| Item | Budget (launch) |
|---|---|
| Total audio on disk | ≤ 180 MB (Opus 64–96 kbps for music/voice; 16-bit 48 kHz FLAC→Opus for SFX at 128 kbps) |
| Decoded memory resident | ≤ 200 MB (Recommended), ≤ 90 MB (Minimum, with lower sample counts/ more streaming) |
| Load time | Core sounds < 2 s after first interaction; others lazily on demand/ahead of mission with the load plan (§44.9) |
| Audio thread CPU | ≤ 8 % of one core avg, ≤ 15 % peak (Recommended); sim-driven emitter updates ≤ 0.15 ms/frame |

Pipeline: source WAV (48 kHz/24-bit) → loudness normalisation (−23 LUFS per asset reference) → encoded to Opus → packed into `.audpack` with metadata (loop points, priorities, falloff, variations). The content registry (§62) references audio by id.

## 56.12 Multiplayer voice

The WebRTC star voice channel (§33.5) feeds the Voice bus: Opus 24 kbps mono, VAD, per-team channels (Flight/Team/All), 3D positioning optional (off by default: voice is radio-style with bandpass EQ and the same degradation by distance/jamming as AI radio), push-to-talk only while the tab is focused (browser limitation, stated in settings), echo cancellation via the browser's `getUserMedia` constraints.

## 56.13 Acceptance criteria

- `AC-56.1` Propagation: a test emitter at 3 km produces a 8.8 s ± 0.1 s delay, and the received spectrum matches the air-absorption table within ±3 dB per band; Doppler shift of a 300 m/s approach matches `c/(c − v)` within 1 %.
- `AC-56.2` Directional localisation: in a blind test (12 listeners, headphones, HRTF), azimuth error ≤ 30° median for front/back emitters at 10 test angles; missile-warning direction identification ≥ 85 % correct to the 90° quadrant.
- `AC-56.3` Voice and CPU budgets met under a 150-aircraft battle: ≤ 96 voices, audio thread ≤ 15 % peak, no buffer under-runs in a 30-minute soak (underrun count = 0 at 128-frame quantum on the Recommended profile).
- `AC-56.4` Radio variety: over 200 simulated minutes of combat, no identical message repeats within 10 messages of the same type, and ≥ 95 % of chosen templates differ from the previous occurrence.
- `AC-56.5` Warnings: each warning audio plays within 100 ms of the HUD cue and obeys priority ducking (verified by waveform analysis).
- `AC-56.6` Loudness: integrated loudness of the standard mix over a reference mission is −16 LUFS ± 1.5 and true peak ≤ −1 dBTP.
- `AC-56.7` Aircraft distinctiveness: blind-test criterion in §56.3 met for all launch aircraft pairs.

---

# 57. ACCESSIBILITY

`P1 → P10 · M1 (baseline) → M5 (complete)` — The game must be playable by as many people as possible without removing depth (outline §108). Options include controller, keyboard/mouse and HOTAS support; an adjustable HUD; a colour-blind-friendly UI; reduced screen shake, camera motion and G effects; a simplified flight model; and simplified avionics. Accessibility is a **cross-cutting requirement** with its own checklist, not a late polish pass.

## 57.1 Principles

1. **Options, not modes:** each option is independent and persists per profile; none locks out features, leaderboards or campaign progress (there is no external ranking).
2. **No information by colour or sound alone:** every colour has a shape/pattern/text; every critical sound has a visual and every critical visual a sound option.
3. **Core rules unchanged:** assistance changes *how* a player achieves things, not the difficulty rating of rewards (campaign has a "assist used" tag purely informational).
4. **Accessible by default:** the DOM UI (ADR-20) provides semantics for assistive tech; the in-flight HUD offers equivalents.
5. **Tested with people:** accessibility checks are part of the release gate (§66.4), not optional.

## 57.2 Input and motor accessibility (outline §108: controller/keyboard/HOTAS)

| Feature | Details |
|---|---|
| Full remapping | Every action bindable to keyboard, mouse, gamepad, joystick/HOTAS, with chords and layers (§58) |
| One-handed / reduced input | Mouse-only flight (mouse-aim, §11.11), keyboard-only flight, gamepad-only; **toggle vs hold** for every held action (zoom, padlock, airbrake, radio, boost); adjustable hold thresholds |
| Input shaping | Deadzone, expo, saturation, smoothing, per-axis invert; stick sensitivity; a "tremor filter" (low-pass 2–8 Hz adjustable); "slow stick" mode |
| Assistive devices | Any HID device the browser exposes via the Gamepad API (adaptive controllers); switch-style devices bind to discrete actions with autopilot/assist actions for sequencing |
| Timing relief | Adjustable QTE-free design: no timed button sequences in core play; refuel/landing/catapult have assist levels; auto-start; auto-landing assist; autopilot; AI wingmen can handle sub-tasks |
| Simplified controls | "Essentials" scheme with ≤ 12 actions (pitch/roll/yaw/throttle/fire/weapon select/target/radar/countermeasures/map/pause) with auto-managed everything else |

## 57.3 Simplified flight model and avionics (outline §108)

- **Simplified flight model** = Full Assist (§11.11.3) + arcade-lite handling toggles: auto-rudder, auto-trim, G-limiter, stall/spin protection, energy-hold throttle assist, auto-brake/auto-landing flare, ground-collision avoidance (Auto-GCAS, §54.9). Tuned so a first-time player can complete the First Flight mission.
- **Simplified avionics** = Fidelity Tier A (§16.10.1): HUD-only, auto radar lock cues, floating target markers, no MFD management, auto-countermeasures (optional), auto-stores selection by target type.
- Each can be applied independently of the difficulty preset; the campaign records "assist flags" for transparency only.

## 57.4 Visual accessibility

| Area | Options |
|---|---|
| **Colour-vision deficiency** | Presets: protanopia, deuteranopia, tritanopia (palettes tested with simulation matrices — Machado et al. 2009-style transforms — and ΔE ≥ 30 between affiliations); high-contrast HUD palette; per-affiliation custom colours; shapes always on (§53.8, §55.4); patterns for ownership regions (campaign map) |
| **HUD adjustments** | Scale 50–200 %, opacity, brightness, colour, thickness of strokes, font size (≥ 10 mrad up to 24 mrad), position offsets, declutter presets, HUD overlay mode in cockpit (bigger, brighter), "Large text" mode for the DOM overlays (up to 200 %) |
| **Contrast** | WCAG 2.2 AA minimum for all DOM text (≥ 4.5:1; large text ≥ 3:1) and AAA in the High Contrast theme; HUD symbols ≥ 3:1 against the sampled background (§54.2) |
| **Visual sound indicators** | Screen-edge rings/arrows for threat bearings, missile launches, lock tones, incoming gunfire, radio messages (speaker icon), with severity colours and shapes (also for deaf/hard-of-hearing) |
| **Photosensitivity** | Flash limiter (no more than 3 flashes/s above the WCAG general flash threshold) — muzzle flashes, explosions, lightning and master-warning pulses are rate-limited; "reduced flashing" option; a first-run warning screen for photosensitive users |
| **Motion** | Reduced camera motion (head bob, G-shift, FOV kick → off), reduced screen shake (0–100 %), disable motion blur, disable chromatic aberration/film grain, reduce parallax in menus; `prefers-reduced-motion` honoured at first run |
| **G effects (outline §108)** | Strength 0–100 % of vision tunnel/blackout/greyout; "never lose control" toggle (visual only, no loss of control; tone/haptic cues instead); optional haptic/audio cues for G-load; G-LOC disabled option (§11.14) |
| **Text and icons** | Scalable fonts; dyslexia-friendly font option (OpenDyslexic-class); icons accompany text; no text embedded in textures |
| **Screen reader** | DOM menus expose ARIA roles/labels/live regions (kills, objectives, warnings can be announced via polite/assertive live regions; HUD-critical callouts via an optional "speak HUD" feature using speech synthesis `speechSynthesis` for altitude/speed/heading/threat callouts every N seconds) |
| **Zoom/magnifier** | Browser zoom supported for menus; in-flight magnifier on the HUD region (picture-in-picture) |

## 57.5 Auditory accessibility

Closed captions for radio and *sound events* ("[Missile launch – right rear]", "[Explosion, 3 km]"); visual indicators (§57.4); mono/balance; independent bus volumes; boost for 1–4 kHz speech; hearing-aid dynamic range; vibration/haptic output through gamepad rumble (Gamepad Haptics API where supported) for lock tones/missile warnings/G.

## 57.6 Cognitive accessibility

- Clear, consistent language; plain-text mission briefings with an optional **summary card** (3 bullet objectives).
- Pause anywhere in single-player (and a "pause-on-map" option); no time-limited menus; optional **tutorial replays**; "hints on" persistently; step-by-step checklists for start-up/landing; optional objective markers and navigation helpers; adjustable game speed in sandbox (0.5×–1× slow-mo, not in competitive modes) so players can learn at their own pace.
- Reduced information density: declutter presets; "Essentials" HUD with six items (speed, altitude, heading, target, weapon, threat).
- Limits on simultaneous radio chatter and message frequency (setting).

## 57.7 Platform specifics (browser)

- Keyboard focus management follows WAI-ARIA authoring practices; focus is never trapped; `Esc` closes dialogs; skip links on main screens.
- The canvas has an accessible label and a status text (`aria-live`) describing mission state in plain language (optional).
- Respect OS-level settings via `prefers-reduced-motion`, `prefers-contrast`, `prefers-color-scheme`, `forced-colors`.
- All accessibility settings are available **before** the first flight (first-run flow, §53.11) and from the pause menu; they can be exported with the profile (§64).

## 57.8 Testing and compliance

- Automated: axe-core-class checks on DOM screens in CI (zero critical violations), colour-contrast checks on tokens, keyboard-traversal tests, reduced-motion snapshot tests.
- Manual: checklist per release (screen reader walkthrough, colour-blindness simulation, one-handed play, flash analysis (Harding-type tool) of cinematic sequences).
- External review: accessibility consultants/playtesters before M5 and M7 (budgeted in §66).

## 57.9 Acceptance criteria

- `AC-57.1` Every colour-coded element has a non-colour channel (shape, pattern, text): static audit of the symbol library finds zero violations; simulated deuteranopia/protanopia/tritanopia views of the tactical map distinguish friend/foe/unknown with ΔE ≥ 30 (computed).
- `AC-57.2` The game is completable (First Flight mission, one sandbox sortie with an air kill and landing) using (a) keyboard only, (b) mouse only, (c) gamepad only, (d) the "Essentials" scheme.
- `AC-57.3` With reduced-motion on, no camera/head/FOV motion exceeds 0.5° amplitude from G/buffet effects, no shake in menus, and animations are ≤ 150 ms fades.
- `AC-57.4` Flash limiter: frame-sequence analysis of 20 worst-case combat clips shows no violation of the three-flashes-per-second threshold with the limiter on.
- `AC-57.5` Closed captions: 100 % of radio messages and listed sound events produce captions within 150 ms of the sound; verified on a 30-minute recorded mission.
- `AC-57.6` DOM accessibility: axe-style automated scan reports zero critical/serious issues on all screens; manual screen-reader walkthrough completes "start a sandbox mission" without sighted assistance.
- `AC-57.7` Text scaling to 200 % causes no clipped or overlapping text on any screen at 1280 × 720 (automated screenshot diffs).

---

# 58. CONTROLS

`P1 · M1` — Support keyboard and mouse, controllers, HOTAS and joysticks; controls are **fully remappable** (outline §106), and flight assistance presets (Full / Standard / Reduced / Manual, outline §107) select how input becomes control-surface commands (§11.11). This chapter specifies the **browser input stack**, shaping, bindings, mouse-aim, head tracking and the VR-compatibility stance.

## 58.1 Intent and device support

| Device | API | Support level |
|---|---|---|
| Keyboard | `KeyboardEvent.code` (layout-independent physical keys) | Full |
| Mouse / trackpad | Pointer Events, Pointer Lock (`unadjustedMovement`), Coalesced events | Full (mouse-aim, mouse-look, menus) |
| Gamepads (Xbox/PlayStation/Switch-Pro-class) | Gamepad API (`standard` mapping) + rumble via `vibrationActuator` | Full |
| Joysticks, throttles, pedals, HOTAS | Gamepad API (non-`standard` mapping: raw axes/buttons/hats) + optional **WebHID** (Chromium; for devices with >32 buttons, hat decoding, LED control) [verify] | Full (profile-based) |
| Head tracking | Webcam ML (local) or external tracker through a local WebSocket bridge | Optional |
| VR controllers / HMD | WebXR Device API | Out of launch scope; design-for-compatibility (§58.8) |
| Touch | Pointer Events | Menus only |

Principles: (1) every action is bindable on every device class; (2) a first-time player gets a working default without opening a settings screen; (3) devices can be mixed (stick + throttle + pedals + keyboard + mouse); (4) the control layer never blocks accessibility options (§57.2).

## 58.2 Input pipeline and shaping

```text
Device events/polls (main thread)  →  Device layer (normalise to axes[−1,1] / buttons / hats)  →  Binding layer (actions)  →  Shaping (per axis)  →  InputFrame  →  SharedArrayBuffer ring  →  Sim Worker (per tick, 60 Hz)  →  assist layer / FCS (§11.11)
```

- **Sampling.** Keyboard/mouse: event-driven, timestamped (`event.timeStamp`); pointer-raw mouse deltas accumulated per frame (coalesced events for sub-frame precision). Gamepad API: polled each animation frame (`navigator.getGamepads()` — a Window-only API, so polling is on the main thread) and additionally on a 4 ms timer while the tab is focused (to catch sub-frame changes at 144 Hz+ displays [verify]). The Sim Worker consumes the **latest** `InputFrame` each 60 Hz tick; the previous 3 frames are retained for network redundancy (§0.6.1).
- **Latency budget (input → control surface command):** ≤ 25 ms p95 (≤ 8 ms for polling + ≤ 16.7 ms tick quantisation), excluding display latency; measured by the debug overlay (§63).
- **InputFrame** (fixed layout, 96 bytes): `{ seq: u32, t_ms: f64, axes: [f32; 12] (pitch, roll, yaw, throttle, throttle2, brake_l, brake_r, trim_p, trim_r, trim_y, cursor_x, cursor_y), hats: [u8; 2], buttons: u64 (bit per action), flags: u32 }`.
- **Axis calibration** (per device, per axis): wizard records min, centre, max (and detects ghost axes); stored with the device profile.
- **Per-axis shaping pipeline** (order matters):

```text
x₀  = calibrate(raw)                              → [−1, 1]
x₁  = deadzone:  x₁ = sign(x₀)·max(0, |x₀| − d_c) / (1 − d_c − d_o)        (centre deadzone d_c default 0.03; outer d_o default 0.01; clamp to [−1,1])
x₂  = curve:     x₂ = sign(x₁)·( (1 − k)·|x₁| + k·|x₁|³ )                   (expo k ∈ [0,1], default 0.3 stick, 0 throttle)  or power curve |x₁|^γ, γ ∈ [0.5, 3]
x₃  = saturation: x₃ = clamp(x₂ · g, −1, 1)                                 (gain g ∈ [0.5, 2])
x₄  = smoothing: x₄ ← x₄ + (x₃ − x₄)·(1 − e^{−Δt/τ})                        (τ ∈ [0, 120] ms; default 0 for stick)
x₅  = rate limit: |Δx₅| ≤ s_max·Δt                                            (optional; accessibility "slow stick")
x₆  = invert/offset (trim)
```

  Optional **tremor filter** (accessibility): 2nd-order low-pass with cutoff 2–8 Hz, applied before the curve. Keyboard "axis emulation": press-and-hold accelerates the virtual axis (ramp rate `r_up` default 3.0 /s, return rate `r_dn` 4.0 /s, nonlinear), with tap trimming (±2 %), and *both* ramp rates tunable.
- **Throttle** modes: absolute (physical lever), incremental (buttons/rocker), detents (idle, mil, AB gate) with soft stops; **AB detent** via a configurable position (default 0.92) or a button.
- **Hat/POV** decoding for both `axes[9]`-style encodings and button-based hats.

*Clarified in 0.2.2 (ADR-36, ADR-37):* **InputFrame v3** keeps the 96 bytes and the layout above and gives meaning to the reserved bytes: `flags` bit 0 neutral, bit 1 **changed** (a control actually changed, the frames the latency of AC-58.3 is measured on), bit 2 mouse-aim (the cursor axes carry a direction: `cursor_x = azimuth / π`, `cursor_y = elevation / (π / 2)`), bit 3 auto-GCAS; `t_ms` is the time of the **source event** on the shared clock and `t_sent_ms` (offset 80) the time the frame was written, neither of them hashed; byte 74 `assist_req` (0 keep, 1 to 4 an absolute level), byte 75 `spawn_ack` (the snapshot's spawn counter the host has synchronised its throttle lever with) and byte 76 `spawn_req` (read on the reset press: 0 the session's spawn, 1 runway, 2 air). Buttons: 0 reset, 1 and 2 trim nose up and down (held), 3 gear (toggles the lever), 4 and 5 flaps down and up, 6 airbrake (held), 7 parking brake (toggles), 8 recover (held), 9 trim reset, 10 flap cycle. The input is polled each animation frame and on a 4 ms timer, and a device event writes its frame at once, so the polling wait in the budget above applies only to devices that raise no events (pads). The `controls/1` profile (§58.6) and the bindings are in ADR-37.

## 58.3 Browser specifics (gesture, pointer lock, fullscreen, reserved keys)

| Topic | Rule |
|---|---|
| User gesture | Audio, pointer lock, fullscreen, WebXR and WebHID permission all need a user gesture: the main menu's first "Fly" press performs them in one gesture (`audioContext.resume()`, optional `requestFullscreen()`, optional `requestPointerLock({unadjustedMovement: true})`) |
| Pointer lock | Used for mouse-look/mouse-aim in flight; `Esc` is reserved by the browser to release it, so the default **pause** binding is `Esc` and the game also shows the pause menu when the lock is lost. Re-lock requires a click (shown as a prompt) |
| Fullscreen + Keyboard Lock | With fullscreen, `navigator.keyboard.lock(['Escape', 'KeyW', …])` (Chromium) captures system keys so a long-held `Esc` isn't necessary [verify]; otherwise defaults avoid reserved shortcuts |
| Reserved browser shortcuts | Cannot be captured: `Ctrl+W/T/N/Shift+T`, `F5`, `F11` (fullscreen), `Alt+F4`, `Ctrl+Tab`, `Alt+Left` etc. Default bindings never use them; the binding editor warns when a user chooses one |
| Layout independence | `KeyboardEvent.code` identifies physical keys; labels shown with `navigator.keyboard.getLayoutMap()` (where available) so AZERTY/QWERTZ users see correct names |
| Focus loss / tab hidden | Input axes zeroed (except autopilot-held states), buttons released, pointer lock released; resume prompt on return |
| Gamepad connect | `gamepadconnected` events; some browsers expose a gamepad only after a button press; the device list shows "press a button to activate" |
| Default context menu & selection | Suppressed over the canvas; `touch-action: none` |
| Mouse acceleration | Raw (unadjusted) movement requested; fallback with OS acceleration warns once |
| Key repeat | OS auto-repeat ignored (`event.repeat`) for actions; axis emulation uses own timing |

## 58.4 Mouse-aim control ("fly to cursor")

Mouse-aim sits above the assist layer (Full and Standard only, §11.11.3) and converts a desired pointing direction into stick commands. Two variants:

1. **Mouse-aim (pointing).** A virtual cursor moves in a cone around the camera/aircraft (cone half-angle default 35°, spring return optional). The desired direction `û_d` is the world ray through the cursor. The controller computes the error in the aircraft's body axes `e = (e_y, e_z)` (lateral, vertical angles, rad):

```text
φ_des = atan2(e_y, −e_z)                 (bank to put the lift vector toward the target; −e_z = target above the nose)
roll_cmd  = clamp(K_φ·wrap(φ_des − φ), −1, 1)   gated by |e| > e_min (no roll for tiny errors) ; K_φ = 1.6
pitch_cmd = clamp(K_p·(|e|)·cos(wrap(φ_des − φ)), −1, 1)   (pull only when the lift vector points at the target; K_p scaled by q̄ schedule)
yaw_cmd   = coordination from the FCS (Auto)
throttle  unchanged (separate axis)
```

   Rate-limited (max |e|-based command ramp 3 /s), with an **assist-level scaling** (more aggressive in Full), a deadzone (1°), and a dwell (5 ms) to avoid chatter.
2. **Mouse-as-joystick (virtual stick).** Mouse deltas move a virtual stick (sensitivity 0–3×, recentre spring 0–1) producing pitch/roll commands; wheel = throttle increments.

Free-look (hold `C`/mouse button) moves the camera independently; padlock (hold) points the camera at the selected target (disabled in Realistic). In Reduced Assist and Manual, mouse-aim is **unavailable** (only mouse-as-joystick); the first-run flow explains this.

*Clarified in 0.2.2 (ADR-34, ADR-37):* mouse-aim runs **in the simulator** (so that AC-58.4 is tested headless and deterministically); the page keeps the world-frame aim direction, limited to a 35° cone around the latest nose, and sends it in the cursor axes. In place of the bank-and-pull of the listing, the controller asks the nose to close on the target at a rate proportional to the error (looking ahead along the rate it already closes at), builds the lift needed (the turn toward the target plus the lift that holds the path against gravity), and uses its **direction as the roll stick** and its part along the lift axis as the pitch stick through the load-factor loop. The listing's gains (`K_φ` 1.6, `K_p`, the 3 /s ramp) did not survive the test (none of 500 runs passed with them); the structure did, and the numbers are `fcs/1` data (`content/core/fcs/README.md`). Mouse-as-joystick stays on the page.

## 58.5 Action catalogue and default bindings

Actions (≈ 170) are grouped; every action has an id, label key, type (`axis` / `button` / `toggle` / `hold` / `radial`), default bindings per device class, and optional layer (shift modifier).

| Group | Representative actions |
|---|---|
| **Flight** | Pitch, roll, yaw, throttle (abs/inc), afterburner, trim (3 axes + reset), airbrake, flaps ±/auto, gear, hook, wheel brakes (L/R/both), parking brake, canopy, launch bar, catapult ready, arrester hook, refuelling probe, autopilot modes (§50.4), auto-start, unusual-attitude recovery, mouse-aim toggle |
| **Weapons** | Fire (primary), fire (secondary), weapon cycle ±, weapon select (A/A, A/G, guns), master arm, release mode, jettison (selective/all stores), countermeasures (flare, chaff, program ±), missile uncage, boresight/slave toggle, laser/TGP controls |
| **Sensors** | Radar mode ±, range ±, azimuth/bar ±, elevation tilt ±, target designation (cycle, nearest, boresight, hook), lock/unlock, IFF interrogate, RWR page/mode/mute, ESM, EMCON, jammer on/off, datalink |
| **Navigation** | Next/prev waypoint, direct-to, TACAN set, bullseye toggle, nav mode, map |
| **View** | Cockpit/3rd/chase/free/weapon camera, look (hat/mouse/head), padlock, zoom, snap views (front/left/right/back/down), HUD declutter/brightness, toggle UI, screenshot, photo mode (§52) |
| **Systems** | Engine start/stop, fuel pumps, generator, lights (nav/landing/formation/anti-collision), master caution reset, ejection (with guard: double press) |
| **Comms** | Radio menu, quick wheel, PTT (voice), request tanker/AWACS/landing, wingman orders (§20.6), chat |
| **System/UI** | Pause, scoreboard, help, objectives, settings, replay marker |

Defaults:

| Action | Keyboard/Mouse | Gamepad (standard) | HOTAS (typical) |
|---|---|---|---|
| Pitch/roll | `W`/`S` pitch, `A`/`D` roll (axis emulation) or mouse-aim | Right stick | Stick |
| Yaw | `Q`/`E` | Triggers (LT/RT) or left stick X (selectable) | Pedals / stick twist |
| Throttle | `Shift`/`Ctrl` incremental, `1–0` presets | Left stick Y (incremental) / D-pad | Throttle lever |
| Afterburner | `Z` toggle | Hold L3 | Detent / button |
| Fire gun / missile | `LMB` / `RMB` or `Space` / `Enter` | `RT` / `RB` | Trigger / button |
| Weapon cycle | `X` | `Y` | Weapon hat |
| Target select | `T` (cycle), `Y` (nearest) | `LB` | Target hat |
| Flares / chaff | `F` / `G` | `B` (hold: program) | Thumb buttons |
| Gear / flaps / brake | `L` / `V` / `B` | D-pad (context radial) | Switches |
| Map | `M` | Back/Select | Button |
| View | `C` free look, `1`–`4` cameras, `F1`–`F4` snap | R3 (cycle) | Hat |
| Pause | `Esc` | Start | Button |

*Gamepad scheme:* modifier layers (`LB` held = secondary function set), radial menus for infrequent actions, auto-managed countermeasures if enabled. *HOTAS scheme:* ships with named presets for common device families (identified by `vendorId:productId` and `id` strings); unknown devices use the generic wizard ("move the axis you want to assign").

## 58.6 Binding editor, profiles and conflict handling

- **Binding editor** (DOM): list by group with search; capture mode ("press a key/button, move an axis"), multi-binding per action (up to 4), chords, shift layers, hold/toggle/double-tap modifiers, device-specific bindings, per-aircraft overrides, conflict highlighting (same input bound to multiple actions in the same context), "restore default" per action/group, test panel (live axis/button display), axis curve editor with preview, calibration wizard.
- **Contexts:** bindings are scoped by context (Flight, Menu, Map, Replay, Photo mode, Chat) so the same key can have different actions.
- **Profiles:** named profiles (Default K/M, Gamepad, my HOTAS, ...), auto-selected by connected device set; stored in IndexedDB; export/import as JSON; shareable.
- **Schema** (excerpt):

```json
{ "schema": "controls/1", "profile": "hotas-warthog-like",
  "devices": [ { "match": { "vendorId": "044f", "productId": "0402" }, "axes": { "0": { "dz": 0.02, "k": 0.35, "invert": false }, "1": { "dz": 0.02, "k": 0.35 } } } ],
  "bindings": [ { "action": "flight.pitch", "src": { "dev": 0, "axis": 1 } },
                { "action": "weapons.fire", "src": { "dev": 0, "button": 0 } },
                { "action": "weapons.release_mode", "src": { "dev": 0, "button": 3 }, "layer": "shift", "mode": "hold" } ] }
```

- **Cheat sheet** overlay generated from the active profile and device (images of generic gamepad/HOTAS silhouettes with labels).

## 58.7 Head tracking

Optional, for cockpit look-around and padlock-free awareness.

- **Webcam tracking (local):** a lightweight ML face-landmark model (WASM/WebGPU) runs in a worker at 30–60 Hz (≈ 2–4 ms/frame on the Recommended profile, off by default), estimating head pose (yaw/pitch/roll + position). Frames never leave the device and are not stored; explicit camera permission and an always-visible indicator are required (§53.13).
- **External tracker bridge:** supports a documented **WebSocket JSON pose** protocol (`{x,y,z,yaw,pitch,roll,t}` at ≤ 120 Hz) on `ws://127.0.0.1:<port>`, produced by a small open-source helper that forwards common tracker protocols (UDP) — a browser cannot listen to UDP directly.
- **Processing:** One-Euro filter (`f_c,min = 1.0 Hz`, `β = 0.007`, `d_c = 1.0 Hz`) per axis; response curves (yaw gain 1–3×, pitch 1–2× with a "look-ahead" non-linear mapping), centre key, drift correction, limits (±135° yaw, ±60° pitch after gain), dead area, and 6-DOF lean (±15 cm) used for the cockpit eye offset. End-to-end latency target ≤ 60 ms (camera frame to rendered view).
- **Failure:** tracking lost for > 0.5 s → view eases back to centre over 0.5 s; no input to the sim.

## 58.8 VR/WebXR compatibility (OQ-07)

Not in launch scope, but the architecture must not preclude it:

- The renderer is written against a `View[]` abstraction (1 or 2 eyes) with per-view matrices and viewports; the HUD/MFD render-to-texture design (§54.2) already lives in 3D; the camera system (§54.11) can be driven by a head pose.
- DOM UI is replaced in XR by a 3D panel renderer (`WebXR` layers / a texture quad) fed from the same state; menus use ray-pointing.
- Performance: stereo at 72–90 Hz roughly doubles GPU load ⇒ High profile + reduced quality (foveation via `XRWebGLLayer`/WebGPU where available [verify]).
- Controllers map to a virtual HOTAS (hand-tracked stick/throttle as a post-launch research item).
- Comfort: horizon-locked vignette, reduced G effects by default.

## 58.9 Force feedback and haptics

Gamepad rumble (`vibrationActuator.playEffect("dual-rumble", …)`) provides: gun fire pulses, stall buffet (intensity ∝ AoA beyond onset), G-load rumble, missile warning, damage, landing touchdown. HOTAS force feedback (DirectInput FFB) is not available to browsers; WebHID FFB is a research item (OQ). Haptic intensity is a setting (0–100 %).

## 58.10 Edge cases and compatibility

| Case | Handling |
|---|---|
| Device unplugged/replugged in flight | Pause prompt in single-player; in multiplayer inputs hold last value for 0.5 s then neutral (throttle holds); re-binding is automatic via the device profile match |
| Two devices with identical ids | Distinguished by `index` and by "learn" binding; warning shown |
| Axis order differences across OS/browsers (HOTAS) | Profile keyed by browser+OS+id; calibration wizard re-detects; known-device table updated via data (not code) |
| Gamepad with `standard` mapping false | Treated as raw device; wizard assigns |
| Ghost inputs (noisy axes) | Per-axis noise threshold detection in calibration; "ignore axis" |
| Hat as two axes | Auto-detected pattern |
| Browser not exposing a device | Message with troubleshooting steps and a "test page" (web gamepad tester) |
| Input repeat storms on slow frames | Events coalesced; actions are edge-triggered with sequence ids |
| Rapid focus changes | Debounce 100 ms |

## 58.11 Acceptance criteria

- `AC-58.1` A gamepad-only and a keyboard-only player can complete the First Flight mission with default bindings and no configuration (usability test).
- `AC-58.2` Input shaping unit tests: deadzone/curve/saturation formulas produce the specified outputs for a 200-case table to 1e-6; smoothing step response reaches 63 % at `τ` ± 5 %.
- `AC-58.3` Latency: stick movement to control-surface command ≤ 25 ms p95 in an instrumented test (high-speed capture or software timestamps) on the Recommended profile at 60 and 144 Hz displays.
- `AC-58.4` Mouse-aim: from a random initial attitude, the aircraft points within 1° of a commanded direction 10° to 120° away within 4 s (Standard assist, M 0.6, 5 km) in ≥ 95 % of 500 runs, with no stall or g-limit violation.
- `AC-58.5` Binding editor: conflicts are detected for 100 % of duplicate bindings in the same context; profile export/import round-trips identically; unknown devices can be fully bound via the wizard.
- `AC-58.6` Hot-plug test: unplug/replug of stick, throttle and pedals during flight never crashes, never leaves stuck inputs, and restores the profile within 2 s.
- `AC-58.7` Head tracking filter: step input settles within 150 ms with overshoot < 5 %; jitter at rest < 0.15° rms.

*Clarified in 0.2.2 (ADR-33 section 7 and Amendment 1):*

- `AC-58.3` is measured from the **source event** (`event.timeStamp` of a key or pointer event, the Gamepad API `timestamp` or the poll time of a pad), mapped to the shared clock, to the **end of the sim tick** that applied the frame, on frames flagged *changed* only; display latency is excluded. The instrumented test is the sim's own histogram: at least 500 such changes at random phases of the tick on the reference machine (headed Chrome, the staging build, the display rate recorded), p95 at most 25.0 ms; CI runs the same with a bound of two ticks (33.3 ms).
- `AC-58.4` is scaled to the aircraft: the aircraft must be within 1° of the commanded direction **no later than** `t_max = 1.0 s + θ / ω_lim`, where `θ` is the aim error and `ω_lim` the aircraft's own instantaneous α-limited turn rate at the start (`g √(n_lim² − 1) / V`, never below 1.5 g), **and then stay within 1° for the next 0.5 s** (the dwell shows it arrived and is not part of the time budget). The pilot holds full throttle. Everything else is as written: Standard assist, M 0.6, 5 km, random attitude, 10° to 120°, 500 runs, at least 95 %, no stall (α at most the limit + 2°) and no g violation (`n` at most `n_max` + 0.3). The 4 s of the listing cannot be met by a turn of 120° at about 17°/s, and as first scaled (the dwell inside the time, throttle as trimmed) the criterion passes 79 % and cannot pass 95 % for targets high above the nose. The owner chose this reading on 2026-10-24. Full assist is not held to it (its path rate is limited to 10°/s).


---

# 59. DIFFICULTY & REALISM

`P3 → P10 · M2 (presets) → M4 (all dials)` — Difficulty modifies **simulation and AI**, not just enemy health (outline §109). Realism is a set of independent dials grouped into five presets: Easy, Normal, Hard, Expert, Simulator. This chapter defines the dials, their per-preset values, which are authoritative in multiplayer, and the realism index used for scoring.

## 59.1 Intent and rules

1. **Dials, not modes** (§4.3): a preset is a named dial vector; any dial can be changed afterwards (the preset becomes "Custom").
2. **Enemies are never inflated.** Difficulty does not change enemy health or damage tolerance; it changes what the player *knows* and *must manage*, how skilled and coordinated the AI is, and how forgiving the player's own aircraft is (NN-4, NN-9).
3. **Fair AI.** Higher difficulty improves AI perception, decisions and execution (skill vector, §19.6) and resource pressure; AI never receives information or performance the player's side could not have.
4. **Multiplayer fairness.** Simulation-affecting dials are part of `SessionConfig` and identical for every participant (set by the host); local aids are per-player where they cannot affect others (§59.3).
5. **Transparent.** The briefing displays the active preset, the Realism Index, and which dials differ from the preset.

## 59.2 The dials

Class **S** = simulation-affecting (replicated in `SessionConfig`, identical for all players); class **L** = local presentation/aid (per player, no sim effect); class **A** = AI/mission rules (host).

| Category | Dial | Values | Class |
|---|---|---|---|
| **Flight** | `assist_level` | Full / Standard / Reduced / Manual (§11.11.3) | S (per-player allowed in co-op; fixed in competitive) |
| | `g_effects` | Off / Reduced / Full (§11.14) | S |
| | `stall_spin` | Protected / Warned / Full | S (coupled to assist) |
| | `engine_management` | Auto / Semi / Manual (start, compressor stall, overtemp) | S |
| | `ground_handling` | Arcade / Standard / Realistic (brakes, nose-wheel, crosswind) | S |
| | `fuel_weight_effects` | Simplified / Full (CG, trim shifts) | S |
| | `turbulence_scale` | 0–1.5 × | S |
| **Systems** | `avionics_tier` | A / B / C (§16.10.1) | S |
| | `radar_control` | Auto-assisted / Standard / Manual (§16) | S |
| | `rwr_detail` | Direction only / Threat class / Full ID | S |
| | `countermeasures` | Auto / Semi / Manual (programs) | S |
| | `ew_complexity` | Off / Simplified / Full (§17) | S |
| | `datalink` | Always on / Standard / Degradable | S |
| **Combat** | `player_damage_taken` | 25 %…400 % (§30.2) | S |
| | `enemy_damage_taken` | 50 %…200 % (default 100 %; **not** changed by presets) | S |
| | `enemy_weapon_performance` | 90 %–130 % (range/agility multiplier on hostile *AI-launched* missiles) | A |
| | `missile_lethality` | Forgiving (proximity radius ×0.7) / Standard / Harsh (×1.15) — affects *both* sides | S |
| | `ordnance_limits` | Unlimited / Realistic / Limited | S |
| | `friendly_fire` | Off / On | A |
| | `structural_failure` | Off / Warned / Realistic (§18.10) | S |
| **Information** | `map_info` | God / Sensed+labels / Sensed / Minimal (§55.2) | L→S in competitive |
| | `labels` | Always / On identification / Off | L→S in competitive |
| | `ident` | Auto-ID / Visual+IFF / Imperfect | S |
| | `radio_info` | Full / Reduced / Minimal | A |
| | `fog_of_war` | Off / Mission / Campaign (§42) | A |
| **AI** | `ai_skill` | Recruit…Ace or mixed (§19.6) | A |
| | `ai_coordination` | Low / Normal / High (teamwork, tactics) | A |
| | `ai_reaction_scale` | 0.75–1.25 × on perception/reaction times | A |
| | `ai_aggression` | Cautious / Balanced / Aggressive | A |
| **Resources** | `fuel`, `ammunition`, `repair`, `reinforcement_rate`, `respawns` | See §30.2 | A |
| **Environment** | `weather_severity`, `night`, `sea_state`, `wind` | Presets | A |
| **HUD / UI aids** | `hud_assist` | Full / Standard / Minimal / None (§54.12) | L |
| | `objective_markers` | Always / Mission / Briefing only | L |
| | `target_markers` | 3D markers / HUD box only / none | L→S in competitive |
| | `flight_aids` | Landing assist, energy cue, DLZ, notch indicator | L |
| | `hints` | On / Contextual / Off | L |
| | `kill_cam` | On / Off | L |
| **Rules** | `lives`, `time_limit`, `time_acceleration` | See §30.2 | A |

"L→S in competitive" means: local in single-player and co-op, but forced identical (host-chosen) in competitive rule sets (§31) to prevent informational advantage.

## 59.3 Presets (outline §109)

| Dial | Easy | Normal (default) | Hard | Expert | Simulator |
|---|---|---|---|---|---|
| `assist_level` | Full | Standard | Standard | Reduced | Manual |
| `g_effects` | Reduced | Full | Full | Full | Full |
| `stall_spin` | Protected | Protected | Warned | Warned | Full |
| `engine_management` | Auto | Auto | Auto | Semi | Manual |
| `ground_handling` | Arcade | Standard | Standard | Realistic | Realistic |
| `fuel_weight_effects` | Simplified | Simplified | Full | Full | Full |
| `avionics_tier` | A | B | B | C | C |
| `radar_control` | Auto-assisted | Standard | Standard | Manual | Manual |
| `rwr_detail` | Full ID | Full ID | Threat class | Threat class | Direction + class |
| `countermeasures` | Auto | Semi | Semi | Manual | Manual |
| `ew_complexity` | Simplified | Simplified | Full | Full | Full |
| `player_damage_taken` | 60 % | 100 % | 100 % | 100 % | 100 % |
| `enemy_weapon_performance` | 90 % | 100 % | 110 % | 120 % | 120 % |
| `missile_lethality` | Forgiving | Standard | Harsh | Harsh | Harsh |
| `ordnance_limits` | Unlimited | Realistic | Realistic | Limited | Limited |
| `structural_failure` | Off | Warned | Warned | Realistic | Realistic |
| `map_info` | God | Sensed+labels | Sensed | Sensed | Minimal |
| `ident` | Auto-ID | Visual+IFF | Visual+IFF | Imperfect | Imperfect |
| `fog_of_war` | Off | Mission | Mission | Campaign | Campaign |
| `ai_skill` (distribution) | Recruit–Regular | Regular–Veteran | Veteran–Elite | Elite–Ace | Ace–Elite |
| `ai_coordination` | Low | Normal | High | High | High |
| `fuel`/`ammunition` | Unlimited/Unlimited | Realistic | Realistic−10 % | Limited | Limited |
| `reinforcement_rate` | ×0.6 | ×1.0 | ×1.25 | ×1.5 | ×1.5 |
| `hud_assist` | Full | Standard | Standard | Minimal | Minimal |
| `target_markers` | 3D markers | HUD box | HUD box | none | none |
| `flight_aids` | All | Most | Few | Few | None |
| `hints` | On | Contextual | Contextual | Off | Off |

Outline mapping: *Easy* = strong assistance, weaker AI, forgiving combat; *Normal* = standard experience; *Hard* = stronger AI, more dangerous weapons, fewer resources; *Expert* = reduced HUD assistance, strong AI, realistic systems; *Simulator* = maximum realism, minimal assistance, advanced avionics.

**Locks.** Competitive rule sets (§31 *Realistic*) lock S-class dials to Hard/Expert/Simulator values set by the host; co-op lets each player pick assist (`assist_level`, `hud_assist`, `target_markers`) individually within host-defined bounds; the campaign picks the preset at run start and may change it only at between-operation boundaries (with a recorded "assist changed" flag).

## 59.4 Realism Index and scoring

The **Realism Index (RI)** summarises the dial vector on a 0–100 scale for scoring, difficulty tags and leaderboards-free comparison:

```text
RI = 100 · Σ_i  w_i · d_i / Σ_i w_i                   d_i ∈ [0,1] dial position (0 easiest, 1 most realistic)
weights w: assist 3.0, avionics 2.0, radar_control 1.5, ground_handling 1.0, g_effects 1.0, stall_spin 1.0, engine_mgmt 1.0, fuel_weight 0.5,
           map_info 1.5, ident 1.0, ordnance_limits 1.0, ai_skill (mean level) 2.0, ai_coordination 1.0, fog_of_war 1.0, hud_assist 1.0, countermeasures 1.0, ew 1.0
Score multiplier  M = 0.6 + 0.8 · RI/100          (0.6 at all-easy … 1.4 at all-max)       (§25.11)
```

Calibration targets for the presets (to be confirmed by evaluating the formula on the preset table at implementation time; they are not yet computed): Easy ≈ 10–15, Normal ≈ 35–40, Hard ≈ 55–60, Expert ≈ 75–80, Simulator ≈ 95–100. A unit test asserts the ordering is strictly monotone. The multiplier applies to mission score, campaign rewards (Legacy Points, §38.5) and statistic categories; invulnerability/unlimited-ammo flags disable stats (§65).

## 59.5 AI adaptation (optional)

An **Adaptive Opponent** (default off) can nudge the AI skill distribution by at most ±1 level between *rounds/sorties* using a smoothed estimate of player performance (kill/death ratio vs expectation, survival time); it never changes aircraft performance, sensor range or weapon behaviour, never cheats, and always shows its adjustment in the debrief ("Opponents were set to Elite"). In campaign, enemy adaptation (§38.3) is separate and strategic.

## 59.6 Dials in play: design notes per category

- **Flight:** `assist_level` is the master dial; `stall_spin`, `g_effects`, `engine_management` refine. In Manual the Kestrel's full envelope matters (conformance test, §11.19). Statically unstable airframes always keep the SAS (§11.11.3).
- **Systems:** `avionics_tier` determines whether the player must manage radar modes, MFD pages and countermeasure programs; the AI uses the same systems at its skill level. `rwr_detail` changes how much the pilot knows about a threat (direction only vs ID).
- **Combat:** `player_damage_taken` is the "forgiving combat" lever; there is no enemy-health lever in presets (a sandbox override exists, §30.2, marked as "custom"). `enemy_weapon_performance` models "more dangerous weapons" through range/agility of hostile missiles, never through damage multipliers on the player beyond the visible `missile_lethality` radius.
- **Information:** the cheapest realism dials; they determine whether the radar picture and labels do the thinking (§55.2, §16.11).
- **AI:** skill vector §19.6, coordination (mutual support probabilities, role-based tactics §20), reaction scale ±25 %; AI aggression is personality-level.
- **Resources:** make endurance and logistics matter (bingo calls, ammunition conservation).

## 59.7 Validation and balancing

- **AI ladder:** automated AI-vs-AI matches (fixed scenarios, 2 000 runs per pair of skill levels) must show strictly monotone win rates by skill (Ace beats Elite beats Veteran beats Regular beats Recruit at ≥ 60 % pairwise).
- **Player-proxy bots:** a scripted "baseline pilot" (fixed skill vector) flies standard missions at each preset; the survival time and objective completion must decrease monotonically from Easy to Simulator (telemetry targets: Easy ≥ 90 % objective success, Normal 70–85 %, Hard 50–70 %, Expert 30–50 %, Simulator 20–40 % for the baseline bot) [tuning ranges; adjusted after playtests].
- **Dial-isolation tests:** varying a single dial changes only its designated metrics (e.g. `map_info` must not change AI behaviour).
- **Fairness audit:** a test asserts that AI decisions use only information in the AI's side knowledge model for every preset (no omniscience), by instrumenting perception queries.

## 59.8 UI

The difficulty selector is a horizontal slider with five stops plus "Custom"; hovering a stop lists the changes; an expandable matrix lists every dial with its current value, preset default (greyed if equal), and a "?" explanation; the Realism Index and its multiplier are always visible. In multiplayer lobbies, the host's S-class dials are shown to guests as read-only.

## 59.9 Acceptance criteria

- `AC-59.1` The preset table is loaded from data (`difficulty_presets.json`) and produces the `SessionConfig` dials exactly as listed; changing one dial yields preset = "Custom" and leaves the other dials unchanged.
- `AC-59.2` RI is strictly monotone over the five presets; RI computed in WASM and in the UI agree.
- `AC-59.3` Fairness audit passes for all presets (zero violations over 10⁴ AI decisions logged per preset).
- `AC-59.4` AI ladder win-rate ordering holds for all adjacent skill pairs (≥ 60 %, 95 % CI above 55 %).
- `AC-59.5` Enemy health/damage tolerance is identical across presets (inspection of resolved damage parameters for 12 aircraft types).
- `AC-59.6` In a multiplayer test, S-class dials are identical on every client (hash of `SessionConfig`), and L-class dials differ without desynchronising the sim.

---

# 60. PERFORMANCE

`P1 → P10 · M1 (budgets and tooling) → every milestone (gates)` — Performance is a core design requirement (outline §110). The techniques named by the outline — level of detail, occlusion culling, terrain streaming, object pooling, asynchronous loading, GPU instancing, simplified distant AI, network relevance filtering, and physics update scaling — are each assigned an owner, a budget and a test here. Simulation priority (outline §111) and network performance (outline §112) are specified in §60.7 and §60.11. The global targets are in §0.6.3.

## 60.1 Intent and rules

1. **Budgets first.** Every system has a time and memory allocation (below) before it is built; a feature that exceeds its allocation is redesigned or cut, not waived (NN-9 spirit).
2. **60 FPS at Recommended / 30 FPS at Minimum** are *targets enforced by gates* (§60.9), measured at the 1 % low (≥ 45 FPS at Recommended).
3. **Graceful degradation, never failure.** The Performance Governor (§60.7.3) sheds load in a fixed, reversible order; critical simulation (the player's aircraft, nearby aircraft, missiles, guns) is never degraded.
4. **No allocation in hot paths.** Rust Sim Core: arena/slab allocation, `Vec` capacity reserved at load, SoA layouts. JS/TS: pooled objects, typed arrays, no per-frame closures in the render and HUD loops. Budget: 0 allocations per frame (< 4 KB p99) in sim/render hot paths (§0.6.3).
5. **Measure in the real environment:** the primary measurements run in real Chromium-based browsers on lab hardware, not synthetic micro-benchmarks only.

## 60.2 Frame structure and thread budgets

```text
Main thread      : input poll → UI (DOM) → network pump (WebRTC) → audio control → present hints            ≤ 4.0 ms avg
Sim Worker       : [drain inputs] → tick(s) → publish snapshot (seqlock)                                      ≤ 4.0 ms/tick avg (7.0 p99)
Render Worker    : acquire latest snapshot → interpolate → cull → encode GPU commands → submit                ≤ 7.0 ms avg (11.0 p99)
GPU              : terrain → sky → clouds → ocean → objects → vegetation → shadows → fx → post → HUD → present ≤ 14.0 ms (16.0 p99)
Stream Workers   : fetch/decompress/decode/mesh build (off the critical path; budgeted CPU share ≤ 1 core)
Audio worklet    : ≤ 8 % of a core avg
```

Sim, render and main run **concurrently** with a one-snapshot pipeline; end-to-end input-to-photon latency target ≤ 70 ms at 60 Hz (input sampling ≤ 8 ms + tick ≤ 16.7 ms + render ≤ 16.7 ms + display ≈ 25 ms). The Sim Worker is driven by its own accumulator loop (§0.6.1), not `requestAnimationFrame`, so a render stall does not slow the simulation (it can run up to 5 catch-up ticks).

**GPU allocation (Recommended, 1080p, High preset, Large battle) — budget table:**

| Pass | Budget (ms) |
|---|---|
| Terrain (heightfield draw, normals, materials) | 2.00 |
| Sky + atmosphere (LUT update, aerial perspective) | 0.60 |
| Volumetric clouds (half-res, temporally reprojected) | 2.50 |
| Ocean | 1.20 |
| Objects (aircraft, ground, naval, buildings; instanced/indirect) | 2.40 |
| Vegetation / ground cover | 0.80 |
| Shadows (3 cascades + contact) | 1.50 |
| Particles / effects (trails, smoke, explosions) | 0.75 |
| Post (TAA/upscale, bloom, tone map, optional DOF) | 1.20 |
| HUD + MFDs (§54.2: 0.35 + 2 × 0.25) | 0.85 |
| Composition / UI present | 0.20 |
| **Total** | **14.00** |

(The remaining 2.67 ms of the 16.67 ms frame is headroom for driver/compositor variance; p99 ≤ 16.0 ms.)

## 60.3 Rendering quality tiers (cloud, ocean and other scalable features)

Four presets (Low / Medium / High / Ultra) set the tier of each scalable feature; each can be overridden individually. Auto-selection comes from the benchmark (§60.8).

| Feature | Low (Minimum profile) | Medium | High (Recommended) | Ultra |
|---|---|---|---|---|
| **Internal resolution / upscaling** | 66 % + spatial upscale | 77 % + temporal upscale | 100 % TAA | 100 % TAA + sharpening (supersample option 125 %) |
| **Clouds** | 2.5D layered impostors + billboard cumulus (no ray-march) | Ray-march ¼-res, 32 steps, 1 light sample | ½-res, 64 steps, 2 light samples, temporal | ½-res, 96 steps, multi-scattering approximation, high-detail erosion |
| **Ocean** | Scrolling normal maps + 4 analytic waves | 8 Gerstner waves, foam texture | 32 Gerstner waves + crest foam | Near-field 2-cascade 256² FFT (compute) + Gerstner far field |
| **Shadows** | 1 cascade 1024², 1.5 km | 2 cascades 2048², 3 km | 3 cascades 2048², 6 km, soft | 4 cascades 4096², 10 km, contact shadows |
| **Ambient occlusion** | Off (baked terrain AO only) | Horizon-based, half-res | GTAO full-res | GTAO + ray-cone near field |
| **Terrain** | τ = 3 px, ≤ 0.6 M tris, no detail synthesis | τ = 2 px, ≤ 1.0 M tris | τ = 2 px, ≤ 1.5 M tris, detail synthesis | τ = 1.5 px, ≤ 2.5 M tris |
| **Vegetation** | Texture only | Impostors ≤ 40 k | ≤ 120 k instances | ≤ 300 k instances |
| **Buildings (T-Near/Mid)** | Mid only, ≤ 100 k | ≤ 180 k | ≤ 250 k | ≤ 400 k |
| **Particles** | ≤ 4 k, no soft particles | ≤ 8 k | ≤ 16 k | ≤ 32 k, soft + lit |
| **Reflections** | Sky probe only | Sky + SSR (limited) | SSR + planar for water | SSR + planar + cube probe cockpit |
| **Atmosphere** | Precomputed LUT only | LUT + aerial perspective | LUT + multi-scattering | + dynamic time-of-day LUT refresh 2×/s |
| **Texture quality** | Half-res mip bias, 1 K cockpit | Full, 2 K | 2 K/4 K hero | 4 K hero + virtual-texture-style streaming |
| **Anisotropic / MSAA** | 4× / off | 8× / off | 16× / off (TAA) | 16× / off (TAA) |
| **Target frame rate** | 30 FPS | 45–60 | 60 | 60–120 |

*WebGL2 Compatibility renderer* (fallback): equivalent to Low with no compute-based features: ray-marched clouds, FFT ocean, GPU-driven culling, and compute normals are unavailable; terrain normals are baked in the pipeline (`normal` atlas) and culling is CPU-based. Capped at Medium-equivalent visuals; flagged in the compat check (§53.3).

## 60.4 Render techniques and settings

- **Dynamic resolution scaling (DRS):** a controller tracks GPU frame time (`timestamp-query` where supported, else frame-interval estimates): if p90 GPU time > 15.5 ms for 2 s, scale −5 % (min 55 %); if < 12.5 ms for 5 s, scale +5 % (max 100 %). Scale changes ease over 0.25 s and are shown in the debug overlay. The HUD/MFD render at native resolution (separate targets, §54.2).
- **Upscaling:** spatial (EASU/RCAS-style) and temporal upscalers implemented in WGSL; no vendor SDKs.
- **Shadow cascades** use stable (texel-snapped) projections, 3–4 cascades with logarithmic/uniform blend λ = 0.6, depth bias per cascade.
- **GPU-driven rendering:** instance data in storage buffers; compute-based frustum + Hi-Z occlusion culling feeding `drawIndexedIndirect` (multi-draw emulated by batched indirect calls); a cull pass on the previous frame's Hi-Z with a two-phase test (outline: *occlusion culling*).
- **Instancing:** all repeated objects (ground units, buildings, trees, ships, missiles, particles) are instanced with per-instance data (transform, LOD, tint); aircraft use 3–4 LODs (§60.6) with morph/cross-fade.
- **Shader compile hitches:** pipelines are created ahead of time asynchronously (`createRenderPipelineAsync`) during the load plan with a **pipeline cache manifest** (list of variants used by the mission); unseen variants fall back to a pre-compiled generic pipeline for ≤ 3 frames.
- **Frame pacing:** present via `requestAnimationFrame` in the Render Worker (`OffscreenCanvas`); `desynchronized` canvas hint where available; adaptive vsync handled by the browser; frame cap setting (30/45/60/90/120/unlimited).
- **Memory:** texture arrays and atlases with explicit budgets; sparse residency emulated through the tile atlas (§44.5.3); compressed textures (KTX2/Basis → BC7/ASTC/ETC2 by device).

## 60.5 Object pooling, async loading and zero-allocation patterns

| Concern | Technique | Where |
|---|---|---|
| Entities | Slab/arena storage with generational ids; free-list reuse; no per-entity heap allocation | Sim Core |
| Projectiles/particles | Fixed-capacity pools (e.g. 5 000 bullets, 32 k particles), ring reuse on overflow with oldest-first eviction | Sim Core / Render |
| Messages and events | Fixed-size event ring buffers (SharedArrayBuffer), no JSON on hot paths | Between workers |
| Network packets | Pooled `ArrayBuffer`s, `DataView` encoders reused | Network |
| Asset loading | Fully asynchronous (streams, `ImageBitmap`, `createImageBitmap` off-thread, KTX2 transcoding in workers); loads never block a frame; a loading budget of ≤ 2 ms/frame for main-thread finalisation; placeholder assets for in-flight loads | Content |
| Mission start | Parallel: GeoPack tiles, models, textures, audio, pipelines, mission VM compile; progress is the weighted sum | §44.9 |
| GC control | Avoid closures/arrays in loops; reuse scratch objects; measure with `PerformanceObserver` (long tasks > 50 ms are test failures in hot scenarios) | JS |

## 60.6 Asset budgets

| Asset | LOD0 | LOD1 | LOD2 | LOD3 / impostor | Texture set (compressed) |
|---|---|---|---|---|---|
| **Player-grade aircraft (hero, with cockpit)** | ≤ 90 k tris (+ ≤ 120 k cockpit) | ≤ 30 k | ≤ 8 k | ≤ 2 k / impostor sheet at > 6 km | ≤ 12 MB (2 × 2 K sets: exterior, cockpit; ASTC 6×6/BC7) |
| **AI aircraft (non-cockpit)** | ≤ 50 k | ≤ 18 k | ≤ 5 k | ≤ 1.2 k / impostor | ≤ 6 MB |
| **Missiles / bombs / drop tanks** | ≤ 4 k | ≤ 1.5 k | ≤ 400 | billboard | ≤ 1 MB shared atlas |
| **Ground vehicles** | ≤ 12 k | ≤ 4 k | ≤ 1 k | ≤ 300 | ≤ 4 MB atlas per faction |
| **Ships** | ≤ 60 k | ≤ 20 k | ≤ 6 k | ≤ 1.5 k | ≤ 8 MB |
| **Carrier** | ≤ 250 k | ≤ 90 k | ≤ 25 k | ≤ 5 k | ≤ 16 MB |
| **Static buildings (hero landmarks)** | ≤ 80 k | ≤ 25 k | ≤ 6 k | ≤ 1 k | ≤ 6 MB each |
| **Frame-level limits** | Draw calls ≤ 2 500 (indirect batches counted once); unique materials ≤ 150; unique pipelines ≤ 120; total drawn triangles ≤ 8 M (Recommended) | | | | |

Texture formats: KTX2 with Basis Universal (UASTC) transcoded at load to BC7 (desktop), ASTC (Apple/mobile GPUs), ETC2 as a last resort; normal maps BC5; PBR packed (ORM). Models: glTF 2.0 + `EXT_meshopt_compression` and `KHR_mesh_quantization`; LODs authored/generated offline with a tool (meshoptimizer) and verified by a budget checker in CI. **Texture memory budget:** ≤ 1.2 GB for all resident textures (Recommended); LRU texture streaming for aircraft not currently visible. (This answers OQ-05's browser-fidelity constraint: a kit-bashed aircraft roster fits these budgets; fully bespoke models for 12 aircraft are feasible within ≤ 150 MB of compressed aircraft assets.)

## 60.7 Simulation priority and load shedding (outline §111)

### 60.7.1 Priority classes

| Class | Entities | Sim treatment |
|---|---|---|
| **Highest** | Player aircraft; nearby aircraft (≤ 8 km of a human, T0; ≤ 40 km, T1); nearby missiles (any missile tracking a human or within 10 km of one); critical objectives (objects whose state gates success/failure) | Full fidelity: 120/60 Hz flight dynamics, 120 Hz terminal missiles, per-tick damage, 10–20 Hz sensors, immediate event processing |
| **Medium** | Nearby friendlies (≤ 150 km, T2), distant enemies currently detected by humans, important ground units (SAM sites, radars, convoys near the action) | Reduced-order dynamics (30 Hz), 5 Hz AI decisions, 10 Hz sensors, simplified damage |
| **Low** | Distant aircraft (T3), civilian traffic, distant environment simulation (weather far from humans, far ocean state), off-battlefield (T4) | 10 Hz point-mass or 1 Hz abstraction, statistical detection (no LOS ray-march except final engagements), round-robin updates |

Classification is computed on the authority every second (relevance pass, §0.6.1) and obeys the promotion/demotion hysteresis; conversions are continuous (position, velocity preserved). Outline "simplified distant AI" is realised by the T2/T3 AI levels: T2 runs the full decision layer at 2 Hz with simplified manoeuvre generation; T3 runs a script-like state machine (route following, simple engagement resolution by probability) at 1 Hz.

### 60.7.2 Sim time budget (design allocation, Recommended, Large battle ≈ 75 aircraft)

| Item | Entities (typical) | Cost per entity per tick (µs, allocation) | Subtotal (ms) |
|---|---|---|---|
| T0 aircraft (RK4 @120 Hz, damage, FCS) | 4 | 45 | 0.18 |
| T1 aircraft (60 Hz) | 14 | 18 | 0.25 |
| T2 aircraft (30 Hz reduced) | 30 | 6 | 0.18 |
| T3 aircraft (10 Hz point mass) | 27 | 1.5 | 0.04 |
| Missiles in flight (average) | 24 | 8 | 0.19 |
| Gun projectiles (average) | 400 | 0.05 | 0.02 |
| Ground units (active) | 150 | 2 | 0.30 |
| Ships | 10 | 4 | 0.04 |
| Civilian entities | 40 | 1 | 0.04 |
| Sensors (§16.12) | — | — | 1.20 |
| AI decisions (amortised: 75 × 60 µs × 5/60) | 75 | — | 0.38 |
| Network encode/decode, relevance (host) | — | — | 0.30 |
| Mission VM, AOS, damage events, misc. | — | — | 0.50 |
| **Total** | | | **3.62** |

(Allocation, not measurement: 3.62 ms ≤ 4.0 ms average budget with 0.38 ms headroom; measured values replace these in the M3 performance review. The `microbench` suite (§61.10) reports each line item.)

### 60.7.3 Performance Governor (load-shedding ladder)

The governor monitors EWMA of: Sim Worker tick time, Render Worker CPU, GPU time (when available), main-thread long tasks, memory pressure and network send queue. When a budget is exceeded for its trigger window it applies the next step; when all budgets are under 85 % for 10 s it reverts one step. Steps (each reversible):

| # | Step | Trigger (sustained) | Effect |
|---|---|---|---|
| 1 | Dynamic resolution (§60.4) | GPU p90 > 15.5 ms / 2 s | Scale to 55 % |
| 2 | Cloud / ocean / AO tier −1 | GPU still over after step 1 | Lower tier in order clouds → AO → ocean → reflections |
| 3 | Shadow quality −1; particles cap −50 % | GPU over | |
| 4 | Vegetation/building LOD bias +1 | GPU or Render CPU over | |
| 5 | Terrain τ + 0.5 px; triangle budget −25 % | Render CPU/GPU over | |
| 6 | Tier radii ×0.8 (T0 8 → 6.4 km, T1 40 → 32 km, T2 150 → 120 km) | Sim tick p90 > 3.6 ms / 2 s | More aircraft at lower fidelity |
| 7 | Civilian/ambient entities −50 %; far environment sim paused | Sim tick still over | |
| 8 | Sensor rate for T2/T3 reduced (10 Hz → 5 Hz), LOS cache TTL ×2 | Sim tick over | |
| 9 | Spawn deferral / merge (mission director, §28.5) | Sim tick over, caps at 90 % | Fewer simultaneous aircraft |
| 10 | Network: snapshot rate 20 → 15 → 10 Hz | Send queue over / host CPU over | |
| 11 | Warn user (suggest lower preset); log | All steps used | |

**Never degraded:** T0 flight dynamics, missile guidance/fuzing, gun hit detection, authority of hits/damage, and anything affecting fairness (the same sim in all clients/servers). Rendering reductions are local to the client; simulation reductions are decided by the authority and take effect for everyone in the session.

## 60.8 Hardware profiling and the first-run benchmark

At first run (and on demand) a **30-second benchmark** (headless scene, fixed camera path, no gameplay) measures: WebGPU/WebGL2 availability and adapter info (`adapter.info`/limits), GPU frame time at three load levels (terrain+clouds+objects), CPU single-thread speed (a Rust/WASM kernel), multi-thread scaling (workers with `Atomics`), memory limits (`navigator.deviceMemory`, `performance.measureUserAgentSpecificMemory()` where available), and storage throughput (OPFS). Results map to the **Minimum / Recommended / High** profile and the preset (Low…Ultra), with explanation ("Medium selected: GPU takes 11.2 ms at High"). The user can override anything; a re-benchmark button appears in Video settings. Results are stored locally and, if the user opted in to telemetry (§53.13), uploaded in anonymised form to tune thresholds.

## 60.9 Performance tests and gates

| Scenario | Content | Gate (Recommended unless noted) |
|---|---|---|
| **PS-1 Large battle** | 75 aircraft, rugged terrain, 4 human-equivalent cameras scripted, clouds, 20-minute run | Avg frame ≤ 16.7 ms; 1 % low ≥ 45 FPS; sim avg ≤ 4.0 ms; render CPU ≤ 7.0 ms; JS heap growth < 5 % |
| **PS-2 Huge battle** | 150 aircraft (High profile) | 60 FPS avg; sim ≤ 4.5 ms; governor reaches step ≤ 5 |
| **PS-3 Cold streaming flight** | Mach 1.0 at 150 m AGL over mountains, cold cache, 20 min | No holes (AC-44.1), streaming ≤ 0.7 ms avg |
| **PS-4 Dense city low-level** | Downtown 40 k buildings, night | ≥ 55 FPS avg, building triangles ≤ 1.2 M |
| **PS-5 Carrier + naval** | Carrier ops with 50 ships, high sea state | ≥ 55 FPS avg |
| **PS-6 Minimum profile** | Medium battle, Low preset, 900p | ≥ 30 FPS avg, 1 % low ≥ 25 |
| **PS-7 Soak** | 2-hour mixed mission | Memory growth < 1 % per hour; no leaks (heap snapshot diff); no worker restarts |
| **PS-8 Load time** | Cold and cached mission loads | Within §0.6.6 and §4.4 budgets |
| **PS-9 Microbench** | Per-system benchmarks (flight tier steps, LOS, DLZ lookup, tile decode, mesh build, netcodec) | No regression > 5 % vs the baseline commit |

Metrics collection: `performance.mark/measure`, `PerformanceObserver` (longtask, event-timing, layout-shift), WebGPU timestamp queries, WASM-side cycle counters (via `performance.now()` deltas), `measureUserAgentSpecificMemory`, Chrome DevTools protocol traces on lab runners. Nightly runs on three hardware classes; any gate failure blocks the release branch; trend dashboards track p50/p99 per scenario. **A regression budget** of 5 % per milestone is allowed only with a documented justification.

## 60.10 Visual regression and screenshot tests

Deterministic scenes (fixed seed, camera, time, weather) render at 1920 × 1080 on a reference GPU/driver and are compared against goldens using SSIM ≥ 0.985 and per-region checks. The suite includes:

| Test | Purpose |
|---|---|
| **Horizon dip** | The horizon at 10 km altitude sits 3.2° below the eye line (a flat-plane horizon is a known-fail screenshot test, §7) |
| **Terrain LOD ring** | No cracks/popping across LOD boundaries (edge-matching check) |
| **Tile-completeness** | No holes after the load plan |
| **HUD conformity** | Pitch ladder vs horizon alignment (AC-54.1) |
| **Clouds/sky at 6 times of day** | Atmosphere regressions |
| **Ocean sea states 0, 4, 9** | Wave statistics and foam |
| **Night lights/blackout** | Urban lighting |
| **Cockpit instruments** | MFD/HUD states |
| **Accessibility palettes** | CVD palettes preserve symbol distinctness |

Differences are triaged with image diffs; goldens are per GPU vendor family (tolerance tuned) because floating-point/filtering differences are expected.

## 60.11 Network performance (outline §112)

Network synchronisation prioritises important entities and never transmits everything at the maximum rate. The mechanism is specified in §33.5; the budgets and tests are:

| Item | Budget |
|---|---|
| Client downlink (Large battle, 75 aircraft, Recommended) | ≤ 256 kbit/s average, ≤ 512 kbit/s peak (before DTLS/SCTP overhead; ≈ 39 660 B/s ≈ 318 kbit/s *before* delta/relevance savings is the worst-case estimate of §33.5, so savings of ≥ 20 % are required) |
| Client uplink | ≤ 20 kbit/s (60 Hz inputs, 3-input redundancy ≈ 30 B/packet ≈ 14.4 kbit/s + overhead) |
| Host (listen server) uplink at 8 clients | ≤ 2.5 Mbit/s average |
| Snapshot rate | 20 Hz default; per-entity rates by priority: own aircraft 20 Hz, aircraft ≤ 5 km 20 Hz, ≤ 40 km 10 Hz, ≤ 150 km 5 Hz, beyond 2 Hz; missiles tracking you 20 Hz, otherwise by distance; ground units 2–5 Hz; civilians 1 Hz |
| Techniques | Interpolation (100 ms default delay), prediction (own aircraft), delta compression against the last acked snapshot, quantisation (position 1 cm @ 2 km local frame / 0.1 m far, attitude smallest-three 10 bits, velocity 0.1 m/s), relevance filtering (distance, sensor knowledge, mission criticality), priority accumulator with per-packet byte budget (MTU 1 200 B), reliable event channel only for discrete state changes |
| CPU | ≤ 0.3 ms/tick (host, 8 clients), ≤ 0.15 ms/frame (client decode + interpolation) |
| Resilience | Handles 5 % random loss and 150 ms RTT at ≤ 10 % degradation in hit registration (§33.14) |

Tests: netem-style impairment matrix (RTT 20/80/150/300 ms × loss 0/2/5/10 % × jitter 0/20/50 ms), bandwidth counters per packet type, and 16-client headless soak (§33.13).

## 60.12 Memory management

| Pool | Budget (Recommended) | Notes |
|---|---|---|
| WASM linear memory (Sim Core) | ≤ 1.5 GB (typical use < 400 MB) | Preallocated arenas; no `memory.grow` after load; if growth fails → degrade (reduce caps) |
| JS heap | ≤ 600 MB | Includes UI, content registry, caches |
| GPU resources | ≤ 4.5 GB (Minimum 2.5 GB) | Atlases (terrain ≈ 210 MB), textures ≤ 1.2 GB, buffers ≤ 800 MB, targets ≤ 500 MB, building meshes ≤ 600 MB |
| Terrain CPU cache | 384 MB | §44.5.5 |
| Audio decoded | ≤ 200 MB | §56.11 |
| OPFS | quota-bound | §44.5.5 |
| **Tab total at Large battle** | ≤ 6 GB | §0.6.3 |

Out-of-memory handling is part of the failure contract (§61.9): the app reduces budgets by 25 % steps and warns; it never crashes silently. Memory is monitored (`measureUserAgentSpecificMemory()` when available, otherwise internal counters).

## 60.13 Browser-specific behaviours

| Behaviour | Handling |
|---|---|
| Background tabs throttle timers/rAF (≥ 1 s timers) | Sim Worker keeps running (workers are less throttled but can be [verify]); for solo play auto-pause on `visibilitychange`; multiplayer warns the host |
| Display refresh variety (60/120/144/240 Hz) | Render at display rate with interpolation; sim fixed at 60 Hz |
| Power/thermal throttling | Governor reacts to frame-time trends; "Eco mode" caps to 30 FPS and Low tier |
| Hybrid GPUs | Request `powerPreference: "high-performance"` for the adapter |
| Browser-specific bugs | Feature matrix with workarounds table maintained in data (`browser_quirks.json`) |
| WebGPU device lost | See §61.9 |

## 60.14 Acceptance criteria

- `AC-60.1` PS-1 and PS-2 gates (§60.9) pass on the Recommended and High profiles respectively; PS-6 passes on Minimum.
- `AC-60.2` The governor, given a synthetic overload (sim ticks +50 % slower), applies steps in order and restores them within 30 s after the overload ends; T0 fidelity is unchanged throughout (state-hash comparison for the player aircraft).
- `AC-60.3` Dynamic resolution holds ≥ 55 FPS in a GPU-bound scene while keeping the scale ≥ 70 % at the Recommended profile.
- `AC-60.4` A 2-hour soak shows heap growth < 1 %/h and zero worker restarts; GPU memory is stable (±3 %).
- `AC-60.5` Asset budget checker rejects any aircraft/ship/ground asset exceeding §60.6 limits and passes the shipped roster.
- `AC-60.6` The 30-second benchmark classifies 10 reference machines to the expected profile (manual labels) with ≥ 90 % agreement, and repeated runs on one machine give the same profile in 19/20 trials.
- `AC-60.7` Screenshot suite (§60.10) passes on the reference GPU families after a clean build; a seeded regression (e.g. flat-horizon) fails it.
- `AC-60.8` Network: with RTT 80 ms and 2 % loss, client downlink averages ≤ 256 kbit/s in the Large battle scenario and hit registration satisfies §33.14.

---

# 61. TECHNICAL ARCHITECTURE

`P1 → P10 · M0 (skeleton) → M1+` — The project is separated into major systems (outline §116), built with clean interfaces and maintainable code (outline §117), handles errors gracefully (outline §122), works offline (outline §123) and is developed in layers with abstractions for deferred features (outline §125). This chapter is the engineering blueprint; it is normative where it says MUST.

## 61.1 Principles

1. **One simulation, many shells.** The Sim Core has no browser, renderer, audio or network dependency; the web client, dedicated server, test harness and tools are shells around it (ADR-02).
2. **Data in, events out.** The core consumes content data, commands and inputs; it emits snapshots and events. Nothing reads the render world (ADR-16).
3. **Authority is explicit.** Exactly one authority per session (host tab or dedicated server); everything else is a view.
4. **Interfaces before implementations.** Any system the first build cannot afford ships behind a trait with a simple implementation (§61.5).
5. **Everything fails soft** (§61.9): missing data, device loss, worker crashes and low memory degrade the experience but do not end the session.
6. **Observability is a feature:** counters, traces, state hashes and debug views exist from M0 (§63).

## 61.2 Runtime topology in the browser

```text
                              ┌─────────────────────────── Main thread ───────────────────────────┐
 Keyboard / mouse / pad ─────▶│ Input pump → InputFrame ring        DOM UI (menus, overlays, map chrome) │
                              │ WebRTC / WebSocket / WebTransport   Audio-graph control (Web Audio)       │
                              │ GameFlow state machine              Service Worker registration           │
                              └──────▲───────────────▲───────────────────▲──────────────────────┘
        InputRing (SAB, SPSC)          │ NetRing (SAB)         │ UI events (postMessage)
                              ┌────────┴───────┐     ┌─────────┴───────┐        ┌─────────────────────┐
                              │  Sim Worker    │────▶│ Snapshot buffer │───────▶│ Render Worker       │
                              │  WASM Sim Core │     │ (SAB, seqlock)  │        │ WebGPU/OffscreenCanvas│
                              │  (+ job pool)  │     └─────────────────┘        │ HUD/MFD · Map · Fx  │
                              └───────▲────────┘            │ AudioState ring    └──────────▲──────────┘
                                      │ Tile/Content data   ▼                               │ GPU uploads
                              ┌───────┴────────┐     ┌───────────────┐             ┌────────┴────────┐
                              │ Stream Workers │────▶│ Audio worklet │             │ Storage: OPFS · IDB │
                              │ fetch/decode   │     └───────────────┘             │ Cache Storage · SW  │
                              └────────────────┘                                   └─────────────────────┘
```

| Channel | Mechanism | Rate / size | Notes |
|---|---|---|---|
| Input → Sim | SAB SPSC ring of `InputFrame` (96 B) | ≥ 125 Hz writes, consumed per tick | Lock-free; latest-wins plus redundancy window |
| Network → Sim | SAB ring of packets (≤ 1 200 B) + `Atomics.notify` | up to 400 packets/s | Main thread receives from WebRTC; Sim parses |
| Sim → Network | SAB ring of outbound packets | 20 Hz snapshots + input echo | Main thread sends |
| Sim → Render | **Snapshot buffer** (triple-buffered SoA in SAB, seqlock) of render-relevant state + events ring | 60 Hz | Render interpolates with `alpha` |
| Sim → Audio | `AudioState` ring | 60 Hz | §56.2 |
| Sim → UI/HUD | `HudState` double buffer + low-rate JSON-free events | 60 Hz / 10 Hz | HUD reads on the Render Worker |
| Stream → Sim/Render | Transferable `ArrayBuffer`s / SAB tile table | on demand | §44.10 |
| Control | `postMessage` with typed messages (schema-checked in dev) | low | Load/unload, config changes |

**Degraded single-worker mode** (no cross-origin isolation): Sim and Render workers still exist, but SAB is replaced by transferable `ArrayBuffer`s every frame (snapshots) and 30 Hz `postMessage` (inputs/audio); caps are halved (§0.6.2); documented in the compat screen.

**Module map** (outline §116 mapped to code):

| Outline system | Packages (Rust crate `sim-*` unless noted) | Chapters |
|---|---|---|
| WORLD: globe, coordinates, terrain streaming, geographic data, airports, environment | `geo`, `world` (Rust); `terrain-stream`, `world-render` (TS/WGSL); `datapack` tools | §7–§10, §24, §44–§47 |
| FLIGHT: aerodynamics, engines, controls, physics, damage | `flight`, `fcs`, `damage` | §11, §12, §18 |
| COMBAT: weapons, missiles, guns, radar, RWR, ECM, damage | `weapons`, `sensors`, `ew` | §15–§17 |
| AI: pilot, squadron, mission, strategic | `ai-pilot`, `ai-squadron`, `ai-mission`, `ai-strategic` | §19, §20, §35 |
| MISSIONS: generator, objectives, events, reinforcements, bosses | `mission-core`, `mission-gen`, `director` | §25–§29 |
| CAMPAIGN: airports, factions, frontline, resources, progression, roguelite | `campaign` | §34–§43 |
| MULTIPLAYER: lobby, networking, replication, synchronisation | `net-proto`, `net-sim` (Rust); `net-transport`, `lobby-ui` (TS) | §32, §33 |
| UI: world map, loadout, mission setup, HUD, tactical map, campaign map | `ui`, `map`, `hud` (TS) | §9, §14, §53–§55 |
| Cross-cutting | `content`, `protocol`, `savegame`, `replay`, `audio`, `platform-web`, `tools` | §52, §56, §62, §64 |

## 61.3 Repository and build

```text
/crates      Rust workspace (sim-core, geo, flight, ..., protocol, content, server, cli)
/packages    TypeScript workspace (app-shell, ui, render, hud, map, audio, platform-web, tools)
/content     Authoring data (JSON), schemas, audio sources, models (glTF), textures (source)
/tools       Pipelines: geo-pipeline (Rust), content-bake, model-optimiser, audio-pack
/server      Dedicated-server container (native sim-core + WebRTC/WebSocket gateway)
/docs        GDD, ADRs, schema docs, runbooks
```

- **Rust → WASM:** `wasm32-unknown-unknown`, `wasm-bindgen`-free hand-written ABI for the hot interface (pointers into shared memory) plus `wasm-bindgen` for tooling-only calls; `wasm-opt -O3 --enable-simd`. Two builds: **threads** (`+atomics,+bulk-memory,+simd128`, shared memory; needs cross-origin isolation) and **single** (no threads); feature-detected at start (`SharedArrayBuffer`, `crossOriginIsolated`). Size budget: sim-core `.wasm` ≤ 8 MB Brotli per build.
- **TypeScript:** strict mode, ES modules, bundled by an esbuild/Vite-class tool; WGSL shaders are modules compiled with a preprocessor (`#include`, `#define`) and validated by `naga`/`tint` in CI.
- **Hashing/reproducibility:** content-addressed output filenames (`<name>.<hash>.js`), immutable caching headers, lockfiles committed, reproducible builds verified in CI (two builds, byte-equal).
- **CI pipeline:** lint → unit tests → build (native + WASM) → cross-build determinism hash (ADR-10) → content bake + validate → integration (headless sim) → browser E2E → performance gates (nightly) → visual regression → package → deploy to staging.

## 61.4 Sim Core

### 61.4.1 State machine

```text
Idle ──load_content──▶ Idle(content ready) ──load_mission──▶ Loading ──ready──▶ Paused ──start──▶ Running ⇄ Paused ──end──▶ Ended ──unload──▶ Idle
```

`GameFlow` (main thread, §53.3) maps app states onto it; the Sim Core itself exposes only `Idle / Loading / Running / Paused / Ended`.

### 61.4.2 Data layout

Data-oriented ECS-lite: entities are `EntityId { index: u32, generation: u32 }`; components live in per-type sparse-set tables with dense SoA arrays (positions, velocities, attitudes as `f64` triples/quaternions); systems iterate dense arrays filtered by tier lists. No dynamic dispatch in hot loops; polymorphism by enum/tagged storage. Content is accessed by `u32` handles from the baked `Registry` (§62), never by string at runtime.

```rust
pub struct SimCore { world: World, schedule: Schedule, svc: Services, tick: u64, state: CoreState, rng: RngStreams, events: EventBus, cmds: CommandQueue }

pub trait System: Send {
    fn name(&self) -> &'static str;
    fn rate(&self) -> Rate;                                   // EveryTick | Divisor(n) | Hz(f) | OnEvent
    fn run(&mut self, ctx: &mut TickCtx<'_>);
}

pub struct TickCtx<'a> {
    pub world: &'a mut World, pub svc: &'a Services, pub dt: f64, pub tick: u64,
    pub events: &'a mut EventBus, pub cmds: &'a CommandQueue, pub rng: &'a mut RngStreams,
}

pub struct Services { pub geo: Box<dyn GeoService>, pub los: Box<dyn LosService>, pub spatial: SpatialIndex, pub registry: Registry, pub weather: Box<dyn WeatherProvider> }
```

### 61.4.3 Tick pipeline (fixed order, 60 Hz)

| # | Stage | Contents |
|---|---|---|
| 1 | **Ingest** | Drain InputFrames and network/mission/UI commands (validated); apply at the tick boundary |
| 2 | **Environment** | Weather field advance (1 Hz), wind samples, time of day, ocean state |
| 3 | **Relevance & spatial** | 1 Hz tier assignment; spatial hash updates for movers |
| 4 | **AI** | Mission AI (0.5 Hz) → squadron (1 Hz) → pilot decisions (5 Hz, staggered) → maneuver controllers (30 Hz) → virtual stick |
| 5 | **Control & dynamics** | FCS + flight dynamics by tier (T0 ×2 sub-steps), ground and naval movement |
| 6 | **Weapons** | Launch resolution, missile guidance & motion, guns & swept collision, countermeasures |
| 7 | **Sensing** | Radar/IRST/visual (staggered), RWR/ESM, datalink, knowledge update (§42) |
| 8 | **Collision & damage** | Collisions (terrain, objects, aircraft), hit/fuze events, damage application, destruction, crash |
| 9 | **Systems & logistics** | Fuel, stores, airbase operations (§48), repairs, resources |
| 10 | **Mission director** | Triggers (VM), objectives, phases, reinforcements, intensity (§25, §28) |
| 11 | **Publish** | Event drain, snapshot buffer swap, network replication encode (20 Hz), audio state, replay record |

Parallelism: stages 4, 5, 7 may be executed by the job pool (WASM threads) over disjoint entity ranges; results are merged in id order so the output is independent of scheduling (deterministic reductions); a single-thread fallback runs the same jobs sequentially.

### 61.4.4 Interface (WASM ABI, simplified)

```text
init(config_json_ptr, len) → status           load_content(cpack_ptr, len)         load_mission(msn_ptr, len)
push_inputs(ptr, count)                         push_commands(ptr, len)              step(max_ticks: u32) → TickReport{ticks_run, ms}
snapshot_ptr() → *const u8                      drain_events(out_ptr, cap) → n       state_hash() → u64
save_state(out_ptr, cap) → n                    load_state(ptr, len)                 debug_query(kind, args…) → ptr
```

All pointers refer to shared linear memory; there is no per-tick allocation across the boundary.

### 61.4.5 Persistence of state

Versioned binary serialisation (`serde` with a compact format) of the complete mutable state (excluding caches) for: autosave/checkpoint (§64.4), join-state transfer (§33.4), crash recovery (§61.9) and tests (golden hashes). Version tag + migration chain (§64.5).

## 61.5 Extension seams (outline §125: abstractions for deferred features)

If a feature is too complex for the first build, create an abstraction that lets it be added later. The following interfaces exist from the milestone shown; the first-build implementation is deliberately simple but real.

| Capability | Interface | First implementation (milestone) | Final implementation |
|---|---|---|---|
| Weather | `WeatherProvider` | Static cloud layers + uniform wind (M1) | Dynamic fronts, turbulence, orographic effects (M4) |
| Terrain heights | `GeoService` | Procedural + base-pack heights (M0) | Full streamed GeoPack (M2) |
| Line of sight | `LosService` | Terrain-only ray-march (M3) | Max-mip + horizon profiles + clouds/buildings (M4) |
| Aerodynamics | `AeroModel` | Table lookup (M1). *Wired in v0.1a:* `TableAero` in `crates/flight` (the full §11.6.1 build-up from the `aero/1` tables), re-exported as the seam and called concretely by the aircraft tick (ADR-29) | Full table set with store/damage effects (M3) |
| Damage | `DamageModel` | Aggregate health bar (M3) | Component/subsystem model (M4) |
| Sensors | `RadarModel` | Range/RCS threshold detection (M3) | Radar equation + Doppler + notch + ECM (M4) |
| Missiles | `MissileModel` | Simple proportional navigation (M3) | Seeker/guidance/DLZ models (M4) |
| AI pilot | `PilotBrain` | State machine (M3) | Utility AI + predictive BFM (M4) |
| Mission source | `MissionProvider` | Hand-written missions (M3) | Generator + escalation + bosses (M4) |
| Persistence | `PersistenceSink` | In-memory (M1) | Campaign save/ledger (M6) |
| Strategic layer | `StrategicContext` | None / stub (M0) | Campaign (M6) |
| Naval | `SurfaceSim` | Static ships (M3) | Moving ships, carriers (M5) |
| Network | `Transport` | Loopback (M0), WebSocket LAN (M5) | WebRTC + WebTransport (M5) |
| Rendering | `Renderer` | WebGL2 or WebGPU basic (M1) | Full WebGPU stack (M2+) |
| Audio | `AudioBackend` | Beeps and samples (M1) | Worklet synthesis + spatial (M4) |
| Replay | `ReplaySink` | Ring buffer in memory (M2) | OSR/SSR files (M5) |

Each seam has a *contract test* suite that every implementation must pass, so upgrades are drop-in.

## 61.6 Rendering architecture (summary; details in §44, §54, §60)

A frame graph (declarative pass list with resource lifetimes) executes in the Render Worker: *scene extraction* from the snapshot buffer (render proxies with interpolation) → culling (GPU) → passes (terrain, atmosphere, clouds, ocean, opaque, vegetation, shadow, transparent/fx, post, HUD/MFD, UI composition). Resources use pooled buffer/texture allocators with per-frame ring buffers for dynamic uniforms. Shader variants are managed by a shader-permutation cache. A `Renderer` abstraction hides WebGPU vs WebGL2 differences for the shared scene description; the WebGL2 path uses the same proxies but a reduced pass list (§60.3).

## 61.7 Code quality rules (outline §117)

| Rule | Enforcement |
|---|---|
| Modular architecture; no monolithic scripts | Crates/packages ≤ 15 kLOC each; files ≤ 1 500 lines; functions ≤ 100 lines (soft; lint warns, review justifies); dependency graph linted (`cargo-deny`/dependency-cruiser): lower layers never import higher layers (e.g. `flight` cannot depend on `ui`) |
| No hard-coded gameplay values | `NoHardcodedContentLint` (§62.8) |
| Separate data from simulation | Content only through the `Registry`; sim crates have no content literals |
| Expose configuration | Every tunable has a data key with default, units, and range; the debug UI lists them (§63) |
| Clear interfaces | Public traits documented with invariants; `#![deny(missing_docs)]` on public API crates; ADR per significant decision |
| Maintainability | Rust: `cargo fmt`, `clippy -D warnings`, `#![forbid(unsafe_code)]` except audited modules (`platform`, `ffi`, `simd`) which carry an `UNSAFE.md` with invariants; TypeScript: strict, ESLint, no `any` in public APIs; naming and layering guide; code ownership files |
| Tests as documentation | Each public function with non-trivial maths has a unit test with a worked example from this GDD |
| Dependencies | Allow-listed licences (MIT/Apache-2.0/BSD/ISC/MPL), SBOM generated per release, `cargo audit`/`npm audit` gates, pinned versions, no postinstall scripts |
| Performance hygiene | No allocation in hot paths (checked by an allocation-counting test allocator in sim-core tests), `microbench` regression gates (§60.9) |
| Reviews | Two-person review for sim-core and protocol changes; protocol changes require a version bump and compatibility test |

## 61.8 Logging, metrics and tracing

Structured logging (`level`, `module`, `tick`, `fields`) to an in-memory ring (64k entries) and optionally to OPFS (rotating, 20 MB), exportable as a bundle (with replay, config, system info) from the Advanced settings. Metrics registry: counters, gauges, histograms exposed to the debug overlay and the performance harness. Tracing spans for ticks/stages (Perfetto-compatible JSON, `?trace=1`). No personal data is logged; telemetry upload is opt-in (§53.13).

## 61.9 Error handling and resilience (outline §122)

### 61.9.1 Error taxonomy

| Class | Meaning | Response |
|---|---|---|
| **Recoverable** | Expected failures (tile 404, packet loss, validation error) | Retry/fallback; no user interruption beyond optional notice |
| **Degraded** | Capability reduced (device lost → WebGL2, OOM → smaller caps, jammed data) | Automatic adaptation; banner; log |
| **Session-fatal** | The mission cannot continue (sim worker unrecoverable, host gone) | Save what is possible (replay, checkpoint), explain, return to menu |
| **App-fatal** | Cannot run (no WASM, no WebGL2/WebGPU) | Compat screen with remedies |

### 61.9.2 Failure contract

The matrix of §45.10 applies (missing terrain, unavailable geodata, incomplete airport data, network failure, missing assets, invalid mission generation, insufficient memory, server disconnect, corrupted save/cache). Additional rows:

| Failure | Behaviour |
|---|---|
| **Sim Worker trap / panic** (WASM `unreachable`, OOM) | Supervisor detects (`worker.onerror`, heartbeat > 500 ms), terminates and restarts the worker, loads the last checkpoint (autosave ≤ 30 s old; solo/host) or requests a full state from the host/server (client); shows "Simulation restarted" and logs the error with a state dump for bug reports |
| **NaN/Inf in physics state** | Sanitiser at the end of stage 5: non-finite state ⇒ aircraft restored from the last good sample (kept per 1 s), flagged `unstable`, an event is logged; repeated 3× ⇒ aircraft destroyed ("structural failure") to protect the sim |
| **WebGPU device lost** (`device.lost`) | Recreate device and all GPU resources (tile atlas re-uploaded from CPU cache, pipelines rebuilt); if it recurs twice within 60 s ⇒ switch to the WebGL2 renderer |
| **WebGL context lost** | `webglcontextrestored` handling, same as above |
| **WASM memory growth failure** | Reduce caps by 25 %, drop caches, notify; if impossible ⇒ session-fatal with checkpoint |
| **Storage quota exceeded** (`QuotaExceededError`) | Evict by policy (§44.5.5, §52.11); if saves are at risk ⇒ block new data and prompt export (§64.6) |
| **Audio context failure** | Continue silent with a banner and a retry button |
| **Network protocol mismatch** | Refuse to join with an explicit version message; offer to update |
| **Content validation failure** | Skip the offending asset with a placeholder, log it, mark the session `content_degraded` |
| **Mission VM fault** | Disable the trigger, emit `MissionEvent::TriggerFault`, continue |
| **Unhandled JS exception** | Global handlers (`error`, `unhandledrejection`) log and show a non-blocking toast; repeated identical errors rate-limited; crash reporter (opt-in) |

### 61.9.3 Error codes

User-visible errors carry stable codes for support: `GEO-xxx` (data), `NET-xxx`, `SIM-xxx`, `REN-xxx`, `AUD-xxx`, `STO-xxx` (storage), `CNT-xxx` (content), `MSN-xxx` (mission), `SYS-xxx` (platform). Example: `GEO-104` "Region data unavailable", `NET-210` "Version mismatch", `REN-301` "GPU device lost". The message text is localised; the code is not.

## 61.10 Testing strategy

| Layer | Tooling | Targets |
|---|---|---|
| **Unit** | `cargo test`, `vitest` | Maths (geodesy §7, FDM steps, DLZ interpolation, shaping §58.2), parsers, validators; sim-core line coverage ≥ 80 %, critical modules (geo, flight, weapons, net-proto, mission VM) ≥ 90 % |
| **Property-based** | `proptest` | Round-trips (BF↔ECEF, codecs), invariants (energy conservation bounds, no NaNs, monotonic IDs), tier conversion continuity |
| **Fuzz** | `cargo-fuzz`/libFuzzer | Mission/content parsers, network packet decoder, mission VM, save loader, `.gpk` reader |
| **Golden/determinism** | Hash of state after N ticks and generator output | WASM vs native hash equality (ADR-10) for 1 000 seeds (AC-26 and AC-11.7) |
| **Conformance** | Performance Conformance Test (§11.19), weapons envelopes (§15), sensor ranges (§16) | Per-aircraft/weapon data gates |
| **Integration** | Headless sim with scripted scenarios | Mission flows, AI engagements, campaign operations, airbase ops |
| **Simulation soak** | AI-vs-AI batch (nightly, 10⁴ missions) | Crash-free, solvability (AC-26), difficulty ladder (AC-59.4) |
| **Browser E2E** | Playwright (Chromium; Firefox/WebKit smoke) | Boot, compat check, menus, mission start, replay, settings, offline reload |
| **Network** | netem-style impairment harness, 16 headless clients | §33.13, §60.11 |
| **Performance** | Lab hardware runs | §60.9 |
| **Visual** | Screenshot suite | §60.10 |
| **Accessibility** | axe, keyboard traversal | §57.8 |
| **Security** | Static analysis, dependency audit, CSP validation | §61.11 |
| **Manual playtests** | Structured sessions each milestone | §66.4 gates |

Test data: fixed seeds, a minimal GeoPack (3 regions), reference aircraft (Kestrel) and a "content fixture" pack. Flaky tests are quarantined within 24 h and fixed within a week.

## 61.11 Security and privacy (browser threat model)

| Threat | Mitigation |
|---|---|
| Malicious shared missions/liveries/bundles | Validation + sandboxed mission VM (§51.5); images re-encoded; text escaped; no code execution; size limits |
| XSS / injection | Strict CSP (`default-src 'self'`; `script-src 'self' 'wasm-unsafe-eval'` only if required; no inline scripts; Trusted Types enforced); no `eval`, no `innerHTML` with user data (DOM helpers escape text) |
| Third-party scripts/CDN compromise | Self-host all code; SRI for any external resource; tile origins serve data only (CORS + CORP) |
| Cross-origin isolation requirements | COOP `same-origin`, COEP `require-corp`; all subresources opt in |
| Network abuse (malformed packets, flooding) | Authority validates every command (rate limits, plausibility); packet size and rate caps; fuzzed decoder; bans (host kick) |
| Signaling data leaks (invite blobs) | Blobs contain only ICE/DTLS fingerprints and a session id; DTLS fingerprint pinned to prevent MITM on the data channel |
| Cheating | Server-authoritative sim; no client trust; LAN/VLAN trust model accepts that a malicious host can cheat (documented); no competitive ranking (§0.5) |
| Storage tampering | Saves carry checksums (not security); corrupted data handled (§61.9) |
| Privacy | No analytics by default; no accounts; camera frames local-only; telemetry opt-in with documented schema (§53.13) |
| Supply chain | Locked dependencies, SBOM, review of new deps, reproducible builds (§61.3) |

## 61.12 Deployment and operations

- **Hosting:** static site on a CDN with correct headers (COOP/COEP/CORP, CSP, `Cache-Control: immutable` for hashed assets, short TTL for the entry HTML and `datapack.json`), HTTP/2/3, Brotli.
- **Service Worker:** precaches the app shell for the current version; new versions download in the background and activate on the next launch or explicit reload (never mid-mission); version pinned in multiplayer sessions (protocol version in the handshake).
- **Feature flags:** remote-free (shipped in config) with URL overrides for testing (`?flags=a,b`).
- **Dedicated server:** container image of the native sim-core with a WebRTC/WebSocket gateway (§33.10); health endpoint, metrics, structured logs; configuration via env/JSON; horizontal scaling by session (no shared state).
- **Rollbacks:** previous versions remain available via versioned URLs; content/datapacks are versioned independently.
- **Environments:** `dev` (hot reload), `staging` (CI deploys), `prod`.

## 61.13 Offline operation (outline §123)

The simulator works offline after caching (ADR-19, NN-8); LAN play requires no internet.

| Feature | Offline? | Notes |
|---|---|---|
| App shell, menus, settings | Yes (Service Worker) | After first load |
| Sandbox/1v1/co-op (LAN) with cached regions | Yes | Region must be cached; else procedural fallback (§46.9) |
| Campaign | Yes | Data cached with the campaign region set |
| Replays, gallery, saves | Yes | OPFS/IDB |
| Multiplayer over LAN with WebRTC | Yes | Serverless invite blob exchanged out-of-band (QR/paste) with ICE host candidates on the LAN [verify mDNS candidate behaviour] |
| Region download/update, new datapack | No | Needs network |
| Community hub (future) | No | |

## 61.14 Development philosophy (outline §125)

Develop in layers (§66): the architecture always preserves the eventual vision; every deferred feature has an interface (§61.5) and a placeholder that exercises the integration path from the earliest milestone so later work is replacement, not rewiring.

## 61.15 Acceptance criteria

- `AC-61.1` `sim-core` builds natively and for WASM from the same sources with no `cfg` divergence in gameplay modules; the dependency lint confirms no sim crate depends on a UI/render/audio/network crate.
- `AC-61.2` Killing the Sim Worker at a random time in 100 solo and 100 multiplayer-client sessions yields a restored session within 3 s with ≤ 30 s of lost progress (solo) or ≤ 1 s (client).
- `AC-61.3` Injecting NaN into the state of 10 random aircraft never propagates beyond the aircraft; the sanitiser logs the event and the sim stays finite for 10 minutes.
- `AC-61.4` Forced `device.lost` in 50 runs: rendering resumes within 2 s in ≥ 49; two losses within 60 s switch to WebGL2.
- `AC-61.5` Every seam in §61.5 has a contract test suite and at least one implementation passing it in the milestone shown.
- `AC-61.6` The CSP audit reports no `unsafe-inline`/`unsafe-eval` (except the documented `wasm-unsafe-eval`) and the fuzzers run 10 minutes per target in CI without failures.
- `AC-61.7` Reproducible build check: two clean builds on the CI runners produce byte-identical outputs for all hashed artifacts.
- `AC-61.8` Offline matrix (§61.13) is verified by an E2E test with the network disabled after the first load and a cached region.

---

# 62. DATA-DRIVEN CONTENT

`P1 → P10 · M1 (registry, aircraft, weapons) → M4 (all content types)` — Aircraft, weapons, factions, missions, and locations are data-driven (outline §115). The principle: **adding content must not require rewriting the core engine** (ADR-13, outline §147). This chapter specifies the content model, formats, registry, packs, validation, and the lint that keeps gameplay constants out of code. Detailed schemas are in Appendix A.

## 62.1 Content types

| Type | Id prefix | Contents (summary) | Chapter |
|---|---|---|---|
| `aircraft` | `ac:` | Mass properties, aero tables, engines, FCS gains, hardpoints, avionics, sensors, cockpit/HUD layout, audio profile, damage zones, model/skin refs | §12, §13 |
| `engine` | `eng:` | Thrust/fuel flow tables, spool dynamics, AB, limits | §11.8 |
| `weapon` | `wpn:` | Class, seeker, motor, aero, warhead, DLZ parameters, launch constraints | §15 |
| `store` | `sto:` | Non-weapon stores (tanks, pods, CM) | §14, §15.12 |
| `sensor` | `sns:` | Radar/IRST/RWR/ESM parameters | §16, §17 |
| `ew_system` | `ew:` | Jammers, decoys | §17 |
| `ground_unit`, `sam_system` | `gu:`, `sam:` | Movement, weapons, sensors, armour, models | §21 |
| `ship`, `carrier`, `submarine` | `shp:` | Hull, sensors, weapons, deck data | §22 |
| `faction` | `fac:` | Names, doctrine, roster, colour schemes, callsigns, ROE | §37 |
| `squadron` | `sqn:` | Identity, aircraft, skill, doctrine | §20 |
| `airport_override` | `apt:` | Manual corrections to merged airport data | §10 |
| `mission_template` | `mt:` | Phase patterns, objective structures, force-role mixes | §27 |
| `ruleset` | `rs:` | ROE, respawn, scoring, restrictions | §25.9 |
| `difficulty_preset`, `sandbox_preset` | `dp:`, `sp:` | Dial vectors | §59, §30 |
| `biome`, `landcover_map` | `bio:` | Rules and palettes | §46 |
| `target_def` | `tgt:` | Hit boxes, states, valid ordnance | §47.6 |
| `hud_layout`, `cockpit_layout` | `hud:`, `cpt:` | Layers and pages | §54 |
| `audio_profile`, `audio_pack` | `aud:` | Synthesis parameters, assets | §56 |
| `livery` | `liv:` | Texture refs, decals | §14.6 |
| `storylet`, `event_card`, `callsign_set` | `sty:`, `evt:`, `cs:` | Narrative/campaign content | §40, §43 |
| `locale` | `loc:` | String tables | §53.10 |
| `controls_preset` | `ctl:` | Binding presets and device profiles | §58 |

## 62.2 Authoring format and example (outline §115)

Authoring is **strict JSON** (with `$schema`), one file per entity, in `/content/<pack>/<type>/`. SI units only in data (key suffixes `_kg`, `_m`, `_mps`, `_n`, `_deg`, `_s`, `_pa`); UI converts for display. The outline's example expands to:

```json
{
  "$schema": "https://schemas.example/aircraft/1.json",
  "schema": "aircraft/1",
  "id": "core:ac/kestrel",
  "name_key": "ac.kestrel.name",
  "extends": null,
  "category": "multirole_fighter",
  "era": "G4",
  "mass": { "empty_kg": 8400, "internal_fuel_kg": 3100, "max_takeoff_kg": 14500, "cg_empty_m": [6.1, 0.0, 0.0], "inertia_kgm2": [18000, 130000, 140000] },
  "geometry": { "wing_area_m2": 28.0, "span_m": 9.8, "mac_m": 3.2 },
  "engines": [ { "ref": "core:eng/f129", "count": 1, "position_m": [-5.2, 0.0, 0.0] } ],
  "aero": { "tables_ref": "core:aero/kestrel", "max_mach": 1.83, "service_ceiling_m": 15500, "n_max": 9.0, "n_min": -3.0 },
  "radar": "core:sns/kestrel_radar",
  "hardpoints": [
    { "id": "S1", "pos_m": [1.0, -4.8, 0.2], "type": ["rail_wvr"], "max_kg": 200 },
    { "id": "S4", "pos_m": [2.5, 0.0, 0.5], "type": ["pylon", "rail_mrm"], "max_kg": 1500 }
  ],
  "weapons": { "internal_gun": { "ref": "core:wpn/gun_25mm", "rounds": 400 }, "default_loadout": "core:loadout/kestrel_cap" },
  "cockpit": "core:cpt/kestrel", "hud": "core:hud/kestrel", "audio": "core:aud/kestrel",
  "model": { "mesh": "models/kestrel.glb", "lods": 4, "liveries": ["core:liv/kestrel_azure"] },
  "generated": { "fcs_gains_hash": "a91f…", "inputs_hash": "77c2…" }
}
```

(Abridged: the real file lists all 9 hardpoints. Mass, wing area, engine count, Mach and g limits follow §13.4; MTOW, span, MAC, CG and inertia here are illustrative placeholders — the validated reference data live in Appendix C.)

**Build pipeline (`content-bake`):** load → resolve `extends` (inheritance chain ≤ 4, child overrides parent; arrays replaced unless a `"$merge": "append"` directive) → JSON Schema validation (Draft 2020-12) → semantic validators (§62.6) → generated sections (e.g. FCS gains from the autotuner, DLZ tables from the baker, energy-manoeuvrability tables, horizon profiles) → resolve references to `u32` handles → pack into a **binary `.cpack`** (zero-copy layout, table of contents, content hash) + a debug string table (stripped in release).

## 62.3 Registry, ids and versions

- **Ids** are `namespace:type/name` (e.g. `core:wpn/aim-ir-wvr`). References are by id; the baker resolves them to handles. A missing reference is a build error (core pack) or a load warning with placeholder substitution (user packs).
- **Versioning:** each entity has `schema` (format version) and each pack has `version` (SemVer). Saves/replays/missions store the content versions they used (`content_requirements`, §51.2).
- **Registry API (Rust):** `registry.aircraft(handle) -> &AircraftDef`; iteration in sorted id order; O(1) handle lookup; immutable after load (shared across threads without locks).
- **Overrides and patches:** a pack may override an entity by id (explicit `"override": true`) or apply a **JSON Merge Patch** (RFC 7396) to another pack's entity (`"patches": "core:ac/kestrel"`) — used for balance patches and DLC variants. Conflicts between packs are resolved by load order; conflicts are reported in the content log.

## 62.4 Content packs

```json
{ "schema": "pack/1", "id": "core", "version": "2026.10.1", "requires": [], "game": ">=0.1.0 <0.2.0", "license": "Proprietary", "authors": ["…"],
  "entities": "cpack/core.cpack", "assets": ["models/", "textures/", "audio/"], "size_bytes": 148000000, "hash": "sha256:…", "provides": ["ac", "wpn", "sam", "fac"] }
```

- **Core pack** ships with the game. **Expansion packs** (campaign content, aircraft, factions) are additional packs fetched on demand and cached; **user packs** (liveries, missions, battlefields, bookmarks) are data-only (§51.9).
- **Lazy asset loading:** the baked pack lists asset bundles by entity (model + textures + audio); only assets needed by the selected mission/loadout are fetched (the load plan, §44.9), with prefetch of likely aircraft.
- **Integrity:** each asset bundle has a SHA-256; packs are optionally signed (minisign/Ed25519) for first-party content; unsigned user packs are data-only and run in the validator/sandbox.
- **Compatibility:** a pack declares a `game` SemVer range; the loader refuses incompatible packs with a clear message.

## 62.5 Tunables and balance data

Every gameplay number is a data field with a documented unit and range in the schema (`"minimum"`, `"maximum"`, `"x-unit"`). Tuning aids:

- **Tuning tables** as CSV → JSON import for spreadsheet-based balancing (aero tables, thrust curves, DLZ margins).
- **Generated sections** record the hash of their inputs; the baker refuses stale generated data.
- **Live tuning** (dev builds): the debug UI edits a tunable; the change is applied via a patch to the live registry copy (not the shipped data) and can be exported as a JSON Merge Patch.
- **Balance dashboards** compare aircraft/weapon performance across the roster (energy-manoeuvrability charts, missile envelopes) generated from data in CI.

## 62.6 Validation (schema and semantic)

| Level | Examples |
|---|---|
| **Schema** | Types, required fields, enums, ranges, string patterns, unit-key suffix check |
| **Referential** | All ids resolve; no cycles in `extends`; hardpoint store compatibility |
| **Physical sanity** | Thrust-to-weight within [0.3, 2.5]; wing loading within [100, 900] kg/m²; CG inside the aerodynamic envelope; inertia positive-definite and satisfies triangle inequalities; max Mach consistent with drag tables; fuel capacity vs endurance; control surface limits |
| **Gameplay sanity** | Weapon `Rmax` ≥ `Rne` ≥ `Rmin`; DLZ monotonic; sensor ranges plausible vs RCS reference; roster slots and balance bands (§13.6) |
| **Asset** | Model budget (§60.6), texture formats, LOD triangles, naming, licence metadata |
| **Localisation** | Every key used exists in the default locale; no orphan keys |
| **Conformance** | Aircraft performance vs targets (§11.19), weapons vs envelope targets (§15) — failing is a CI error for core content |

Errors are reported with file, JSON pointer, message and a fix suggestion; the baker exits non-zero on any error in the core pack.

## 62.7 Runtime content access rules

- The Sim Core receives only baked packs; the browser app fetches `.cpack` + asset bundles via the content manifest; no JSON parsing in hot paths.
- Content is **immutable at runtime**; sessions reference content by (id, pack version) and verify equality across participants in multiplayer (hash of the active content set in the handshake, §33.3); mismatch ⇒ refusal with "content differs" (or an automatic fetch of the matching pack).
- Hot reload (dev builds only) swaps the registry between missions and reloads UI/audio/HUD definitions instantly.

## 62.8 `NoHardcodedContentLint` (CI gate)

Purpose: ensure gameplay constants live in data (outline §117).

1. **Scope:** all Rust crates under `sim-*` and TS packages `hud`, `map`, `ui` (excluding `tests/`, `constants/physical.rs`, `algo/` allow-listed modules).
2. **Rule R1:** numeric literals other than `{0, 1, 2, 0.5, -1}` and powers of two used as array sizes in expression context are flagged unless annotated with a trailing comment `// ALGO: <reason>` (for algorithm constants such as integration coefficients) or `// PHYS: <source>` (physical constants).
3. **Rule R2:** identifiers matching known content names (`kestrel`, `aim9`, …) or string literals matching registered content ids are flagged anywhere outside tests and the data-loading layer.
4. **Rule R3:** `tunable!("key")` / `Registry::tunable` is the only way to read a tuning constant at runtime; tunables must exist in `tunables.json` with unit/range.
5. **Mechanism:** a `dylint`/custom clippy lint for Rust and an ESLint plugin for TS; allow-list file reviewed in code review (growth tracked).
6. **Failure:** the CI step fails the build with file/line and a hint ("move to `tunables.json` as `ai.pilot.pursuit_gain`").

## 62.9 Developer workflow

`content watch` (dev server) re-bakes changed files, hot-reloads HUD/audio/AI/weapon data in the running dev session and shows validation errors in an in-game overlay; `content diff` shows balance changes between commits (e.g. "Kestrel: T/W 1.243 → 1.25; sustained turn @ M0.9/5 km 14.2 → 14.6 °/s"); templates generate skeleton entities (`content new aircraft`); a **content gallery** page renders every aircraft/weapon/unit with specs and charts for review.

## 62.10 Localisation data

String tables per locale (`locales/<lang>.json`, ICU MessageFormat), loaded lazily; keys referenced by `*_key` fields; the baker fails on missing default-locale keys; translation tooling exports/imports (XLIFF/PO) with context notes; pseudo-localisation locale in dev builds (§53.10).

## 62.11 Extending the game without engine changes (examples)

| Change | What is edited | Engine change? |
|---|---|---|
| Add an aircraft | One `aircraft` file + aero table + engine ref + model/cockpit/HUD/audio data + optional liveries; run the autotuner and conformance test | None |
| Add a weapon | `weapon` file; add to loadout presets; DLZ baked automatically | None (new seeker *types* need code, new parameterisations do not) |
| Add a faction | `faction` + `squadron` files, callsigns, colour schemes | None |
| Add a mission type | `mission_template` + objective/trigger logic using the existing DSL | None (new DSL ops need code) |
| Add a location override | `airport_override` or `bookmark` files | None |
| Rebalance AI | `tunables.json` / skill tables | None |

The limits are explicit: new *mechanics* (a new seeker physics model, a new DSL op, a new subsystem) require code, but they extend an existing interface (§61.5) rather than rewriting core systems.

## 62.12 Acceptance criteria

- `AC-62.1` An aircraft, a weapon and a faction authored only as data (no code changes) load, pass validation, appear in the Hangar/loadout/roster, and fly/fire/spawn in a sandbox mission (end-to-end test "content-only addition").
- `AC-62.2` The baker rejects 60 malformed content fixtures (schema, referential, physical sanity) with precise JSON-pointer messages and accepts the core pack with zero errors.
- `AC-62.3` Baked pack load time ≤ 150 ms (core, Recommended) and registry lookups allocate nothing at runtime (allocation-counting test).
- `AC-62.4` `NoHardcodedContentLint` flags all planted violations in a corpus of 30 snippets and reports zero violations on the main branch.
- `AC-62.5` In multiplayer, a content-hash mismatch is detected at handshake in 100 % of tests and produces the specified message.
- `AC-62.6` JSON Merge Patch overrides apply deterministically (same output regardless of pack load order for non-conflicting patches; conflicting patches follow declared load order) across 100 randomised pack sets.
- `AC-62.7` Round trip: bake → load → export-to-JSON equals the authoring JSON after normalisation, for every core entity.

---

# 63. DEBUGGING TOOLS

`P1 → P10 · M0 (core tools) → M4 (AI/network/world panels)` — A developer/debug mode makes every system observable and controllable (outline §118 debugging tools, §119 AI debugging, §120 network debugging, §121 world debugging). Tools are built in from M0 because the simulation is only as good as the ability to see it. They ship in release builds behind explicit flags (so players can also produce useful bug reports), but never alter competitive results.

## 63.1 Access and safety

| Mechanism | Behaviour |
|---|---|
| Enable | URL flag `?debug=1` (all), or scoped: `?debug=net`, `?debug=ai`, `?debug=world`, `?debug=perf`; or the Advanced settings toggle "Developer mode"; shortcut `Ctrl+Shift+D` toggles the overlay once enabled |
| Release builds | A subset (FPS, net panel, world inspector, log export) works for all players; **mutating** tools (teleport, spawn, damage, time/weather control, god mode) require `?debug=cheats` and a visible banner |
| Multiplayer | Mutating tools are available only to the **host** in sessions flagged "debug"; sessions using them are marked `debug_tainted`; statistics and records are disabled (§65.5) |
| Competitive rule sets | All mutating tools disabled |
| Production telemetry | Debug usage is not uploaded unless the user explicitly exports a bundle |

## 63.2 Developer tools (outline §118)

| Tool | Function | Controls |
|---|---|---|
| **Free camera** | Detach the camera from any aircraft: 6-DOF fly, speed 1 m/s–5 km/s, focus on entity, orbit, lock to selected unit | `F2`; mouse + WASD/QE; wheel = speed |
| **Teleport** | Move the player aircraft (or a selected entity) to lat/lon/alt, to an airport, to the camera, or to a picked point on the map; velocity preserved or zeroed | Console `tp 36.2 -121.8 3000`, map right-click |
| **Spawn aircraft** | Choose type, loadout, side, skill, formation, state (air/ground/carrier); count; placement at the camera ray / map point | Spawn panel (content registry list) |
| **Spawn missiles** | Launch any weapon from a point toward a target/point with chosen guidance; test seekers and DLZ | Console `fire <weapon> from <entity> at <entity>` |
| **Spawn ground units** | Place SAM sites, convoys, radars, AAA; set ROE and emissions | Spawn panel |
| **Weather controls** | Select preset; sliders for cloud cover/base/ceiling, visibility, wind speed/direction/gust, turbulence, precipitation, temperature, QNH; "freeze weather" | Panel; changes are logged as `debug.weather` events |
| **Time controls** | Pause, step 1 tick / 1 frame, slow-mo (0.05–1×), fast-forward (2–16×, also in the debug builds beyond safety conditions of §49.3), set UTC time/date, "jump to sunset" | `Pause`, `.` step |
| **Damage controls** | Per-entity: repair, set health, set subsystem damage (engine, wing, hydraulics, avionics), kill, invulnerable toggle, infinite fuel/ammo | Entity inspector |
| **FPS counter** | Frame time, FPS, 1 % low, per-thread ms, GPU ms (timestamp queries), draw calls, triangles, DRS scale, governor step | Overlay (§63.4) |
| **Network statistics** | See §63.6 | |
| **Physics vectors** | Per aircraft: velocity, acceleration, lift/drag/thrust/gravity/weight vectors (scaled), body axes, α/β markers, control surface deflections, ground contact forces | Overlay toggle |
| **Radar visualisation** | Radar beams/scan volumes, detection ranges per target, track quality, LOS rays and blockers, RWR bearing lines, notch window, jam strobes | Overlay toggle |
| **AI target visualisation** | Lines from AI to current targets, intended intercept points, predicted trajectories, formation slots | Overlay (§63.5) |
| **Objective visualisation** | Objective locations, completion radii, trigger zones, phase graph with active state, reinforcement spawn points | Overlay |
| **World coordinates** | Cursor/entity lat/lon/alt, BF coordinates, ECEF, tile id and level, grid overlay | Overlay (§63.7) |
| **Terrain boundaries** | Battlefield boundary, tier radii around humans, sim-critical fine zones (§44.6), tile borders by level, airport footprints, flatten patches | Overlay |

## 63.3 Console and scripting

A command console (`` ` ``) provides a typed command language (not arbitrary JS): `tp`, `spawn`, `fire`, `set <tunable> <value>`, `weather`, `time`, `damage`, `kill`, `god`, `godfuel`, `seed`, `mission goto <phase>`, `objective complete <id>`, `tier set <entity> <T>`, `net sim loss 5`, `perf trace 10s`, `hash`, `dump state <entity>`, `replay mark`. Commands are registered by systems (name, args, help) and logged in the session record. Tunables (`set`) apply to the live registry copy (§62.5). A scripting hook for headless tests uses the same command bus (JSON lines).

## 63.4 Overlay system and inspectors

- **Debug overlay layers** are drawn by a dedicated pass (lines/points/text in world space and screen space) from a debug draw buffer filled by systems via `debug_draw::line/sphere/arrow/text(label, pos, color, ttl)`; zero cost when disabled (compile-time feature `debug_draw` in native, a runtime gate in WASM with the draw functions reduced to a branch).
- **Entity inspector:** click an entity (or select from a list) to see all components (formatted from reflection metadata), editable where safe; **watch** values chart in real time; **history** ring (last 10 s) for key signals (α, g, Mach, thrust, AGL).
- **Stage profiler:** per-tick-stage timings (§61.4.3), top-N systems by time, allocations, parallel job utilisation; flame view by `?trace=1` (Perfetto-compatible JSON).
- **State hash:** `hash` command prints the Sim Core state hash and per-subsystem hashes (for desync/determinism debugging); the network panel shows hash mismatches (§63.6).
- **Event log:** the `MissionEvent` and combat event stream with filters (kills, launches, hits, triggers, radio), searchable and exportable.
- **Bug-report bundle:** a button/command exports a zip: logs (ring buffer), config/SessionConfig, mission provenance (seed, versions), last 60 s replay (SSR/OSR), screenshots, system info (browser, GPU adapter info, memory), performance counters, and a state dump — all local (§61.8).

## 63.5 AI debugging (outline §119)

For a selected AI aircraft the panel (and optional in-world labels) displays:

| Field | Source |
|---|---|
| **Current Mission** | Group/task (`cap`, `intercept`, …), objective link, phase |
| **Current Target** | Entity id/type/range/aspect, track quality, how it was acquired |
| **Threat Level** | Aggregate threat score and top-3 contributors (missile warning, closure, energy deficit) |
| **Fuel** | kg, endurance, bingo/joker thresholds, state |
| **Damage** | Subsystem damage summary |
| **Weapons** | Stores by station, selected weapon, DLZ status vs the current target |
| **Formation** | Role, slot, leader, spacing error |
| **Desired Heading / Altitude / Speed** | The virtual-stick controller setpoints (§19.12) vs actual |
| **Current Maneuver** | Maneuver name and phase (e.g. `lag_pursuit → break_left`), time since selected, commitment timer |
| **Tactic utilities** | Bars of all candidate tactics with scores and the chosen one, with the decisive factor |
| **Perception** | Tracks held, ages, uncertainty, what it sees vs the truth (ghost lines for misperceived targets) |
| **Predicted trajectories** | Planned manoeuvre path (1–5 s), predicted target path, intercept point, missile defeat plan |
| **Skill vector** | Parameters in force with any dial modifications |
| **History** | Last N decisions with timestamps and reason codes |

Fairness instrumentation (AC-59.3) logs every perception query so the debug tools can show *why* an AI knew something. Available for all AI tiers (T2/T3 show their simplified state). In replays, OSR provides the same data (§52.5).

## 63.6 Network debugging (outline §120)

The network panel (`?debug=net`) shows, per peer and aggregate:

| Item | Detail |
|---|---|
| **Ping / RTT** | Current, min, avg, p95, jitter (RFC 3550 style) with sparkline |
| **Packet loss** | Inbound/outbound % over 1 s/10 s/60 s windows; out-of-order count |
| **Server tick rate** | Authority tick duration and rate, tick overruns |
| **Bandwidth** | In/out bytes/s by channel (inputs, snapshots, events, voice), per-entity-class breakdown, compression ratio |
| **Entity count** | Entities replicated, by relevance bucket and update rate |
| **Synchronisation errors** | Prediction corrections (count/s, mean and max magnitude), snap events, state-hash mismatches, sequence gaps, rewinds |
| **Interpolation** | Delay in use, buffer depth, extrapolation time |
| **Input buffer** | Server-side jitter buffer depth and late inputs |
| **Transport** | Selected transport, ICE state/candidate types, DTLS state, SCTP stats (`RTCPeerConnection.getStats()`) |
| **Lag compensation** | Rewind amounts used for hit validation |
| **Impairment simulator** | Client-side latency/loss/jitter injection (`net sim …`) |
| **Prediction-error histogram** | Rolling histogram of position error vs authority |

A "desync analyser" captures a 2 s window of state before/after a hash mismatch and writes it to the bug bundle.

## 63.7 World debugging (outline §121)

Clicking anywhere on the world map or in the 3D world (`Alt+click` or the Inspector tool) displays:

```text
Latitude / Longitude (WGS84, 6 decimals)        Elevation (m MSL)       Biome (+ landcover class)
Terrain Source (source | derived | procedural | synthetic)     Nearest Airport (id, name)     Distance to Airport (km)
Streaming Level (z of the resident tile + target level)       Tile id (z, x, y)       Sim level / fine-zone membership
```

Plus: datapack id, tile age, cache state (memory/OPFS/network), LOS test between two clicks (blocked/unblocked and blocker location), horizon distance, sun elevation, local magnetic declination. The inspector uses `GeoInspector` (§45.7). The overlay can draw the streaming heat-map (resident level per tile), tile-boundary grid, sim-critical set, and download queue.

## 63.8 Test harness integration

All debug capabilities are scriptable through the command bus so E2E and soak tests (§61.10) use the same tools: spawn scenarios, force failures (`fault inject tile_404`, `fault inject device_lost`, `fault inject worker_crash`), set seeds, capture hashes and traces. The fault-injection catalogue is the basis for the failure-matrix tests (AC-45.5, AC-61.2–61.4).

## 63.9 Acceptance criteria

- `AC-63.1` Every tool listed in §63.2 exists, is reachable from the overlay or console, and has a smoke test in the E2E suite; mutating tools are inert without `?debug=cheats` (tested).
- `AC-63.2` Enabling all overlays at Large battle (75 aircraft) costs ≤ 3 ms CPU and ≤ 2 ms GPU per frame; disabled overlays cost < 0.01 ms.
- `AC-63.3` The AI panel displays every field in §63.5 for 100 random AI aircraft across tiers without errors; the chosen tactic always matches the highest utility (or the commitment-timer rule).
- `AC-63.4` The network panel's values match independent measurements (packet capture of the loopback harness) within 5 %.
- `AC-63.5` `GeoInspector` data in §63.7 agrees with the pipeline data for 1 000 random points (AC-45.7).
- `AC-63.6` A bug bundle exported after a forced crash contains all listed parts, is ≤ 25 MB, and re-imports into the replay viewer.
- `AC-63.7` Sessions using mutating tools are marked `debug_tainted` and excluded from statistics in all cases (§65.5).

---

# 64. SAVE SYSTEM

`P2 → P9 · M1 (settings/profile) → M2 (checkpoints) → M6 (campaign)` — The game saves settings, unlocked aircraft, campaign progress, campaign seed, airport ownership, pilot progression, statistics and discovered locations (outline §113). Because this is a browser game, saves live in the user's browser storage (IndexedDB/OPFS), with explicit **export/import** and **persistence requests** to mitigate eviction (ADR-19).

## 64.1 Intent and guarantees

1. **No lost progress:** an interrupted session (tab close, crash, power loss) loses at most the last 30 s of in-mission progress and never corrupts a campaign.
2. **Atomic and verified:** every write is atomic; every read is checksum-verified; the previous good generation is always retained.
3. **Portable:** the full profile can be exported to a single file and imported on another machine/browser.
4. **Honest:** the UI states where data lives, whether storage is persistent, and how much is used.
5. **Versioned:** all saved data carries a schema version and a content/datapack provenance, with a tested migration chain.

## 64.2 What is saved and where

| Data (outline §113) | Store | Format | Typical size | Save trigger |
|---|---|---|---|---|
| **Settings** (gameplay, video, audio, accessibility, network, controls profiles) | IndexedDB `settings`, `controls` | JSON (versioned) | < 100 KB | On change (debounced 500 ms) |
| **Unlocked aircraft / weapons / liveries / cosmetics** | IDB `profile.unlocks` | Sorted id set + timestamps | < 20 KB | On unlock |
| **Campaign progress** (run state, operation index, ledger) | IDB `campaign_runs` | CBOR chunks | 1–5 MB per run | After each operation + checkpoints |
| **Campaign seed** | In `campaign_runs.header` | 12-char seed + versions + datapack | < 1 KB | Run start |
| **Airport ownership** (strategic map state) | IDB `campaign_state` | CBOR (node states, fronts, supply) | 100–800 KB (500 nodes) | After each operation |
| **Pilot progression** (pilots, traits, history, relationships) | IDB `pilots` | CBOR | 50–300 KB | After each sortie |
| **Statistics** | IDB `stats` (+ ring of mission records) | JSON/CBOR | 100 KB + 500 × 3 KB | After each mission (§65) |
| **Discovered locations** | IDB `discoveries` | Compact set of tile ids/airports/regions (RLE-compressed) | < 200 KB | On discovery (batched) |
| **Bookmarks, loadouts, liveries (user), presets** | IDB `bookmarks`, `loadouts`, `presets` | JSON | < 200 KB | On change |
| **User missions & battlefields** | OPFS `/missions/` + IDB index | `.msnpack` | 10–120 KB each | On save |
| **Checkpoints** (in-mission) | IDB `checkpoints` (ring of 3) | Compressed binary state (§61.4.5) | 0.5–6 MB | Every 30 s (solo/host) and on events |
| **Replays, gallery, video** | OPFS | §52 | See §52.11 | After mission |
| **Geodata cache** | OPFS | §44–§45 | GBs | Streaming |
| **Telemetry consent, install id (random, local)** | IDB `kv` | JSON | tiny | Settings |

Everything is namespaced by `profile_id` (multiple local profiles supported; default "Pilot 1"). No server-side storage exists.

## 64.3 Storage layout and transactions

```text
IndexedDB "fgame/v1"  (versioned stores; migrations via onupgradeneeded)
   kv, settings, controls, profile, pilots, stats, mission_records, discoveries, bookmarks, loadouts, presets,
   campaign_runs, campaign_state, campaign_ledger, checkpoints, replays_meta, gallery_meta, pack_registry, saves_manifest
OPFS
   /geo/…   /replays/…   /gallery/…   /missions/…   /quarantine/…   /tmp/…
```

- All multi-record updates use a single IDB transaction (`readwrite`), which is atomic.
- Large blobs (checkpoints, campaign state > 1 MB) are chunked (256 KB) into the IDB store with a **manifest record** carrying `{generation, chunk_count, sha256, schema, provenance}`. A save is written as generation `n+1` (chunks first), then the manifest is switched in one transaction; the previous generation `n` remains until `n+2` is committed (keeping 2 generations).
- OPFS file writes use a temp name, flush via `SyncAccessHandle.flush()`, verify the CRC, then update the manifest in IDB; incomplete temp files are garbage-collected at start-up.

## 64.4 Autosave, checkpoints and the emergency checkpoint

- **Autosave cadence:** settings immediately (debounced), stats at mission end, campaign after each operation (the *operation boundary* is the canonical campaign save point), pilot data after each sortie.
- **In-mission checkpoints** (solo and host): every 30 s of Running time, plus on discrete events (objective completed, phase change, landing, respawn), written asynchronously in a worker (≤ 20 ms of main-thread time); ring of 3. A checkpoint is the serialised Sim Core state (§61.4.5) + mission VM state + RNG stream positions + HUD/UI state needed to resume.
- **Emergency checkpoint:** on `pagehide`/`visibilitychange→hidden` the game starts a checkpoint write immediately (IndexedDB transactions started during `pagehide` usually complete, but this is not guaranteed [verify]); if the write is interrupted, the previous checkpoint (≤ 30 s old) is intact.
- **Resume:** on next launch, the main menu offers "Resume mission" if a valid checkpoint exists (shows time, mission, location). Resuming restores the exact state at the checkpoint time; multiplayer sessions cannot be resumed (they end with results saved, §32/§33).
- **Quit/save rules:** Sandbox: "Save & quit" writes a checkpoint; Campaign: mid-operation quit resumes from the last checkpoint or restarts the operation with the same seed (§34.8); in the roguelite campaign, quitting does not allow save-scumming of outcomes — a resume restores the *same* RNG stream positions, so repeated resumes produce the same future (the design intent is anti-exploit consistency, not punishment).

## 64.5 Versioning, migration, integrity and recovery

- **Schema versions:** every saved document has `schema: "<type>/<n>"` and the writing `game_version`, `content_version`, and `datapack` ids.
- **Migration chain:** `migrate_<type>_<n>_to_<n+1>` pure functions registered in order; loading a document runs the chain up to the current schema. Each migration has a fixture corpus (a real save from every released version) and a round-trip test. If a migration fails, the original is untouched, the app shows `STO-xxx` with an export option, and the document is loaded read-only if possible.
- **Forward compatibility:** a document with a newer schema than the app understands is refused for writing ("update required") but preserved.
- **Integrity:** per-document `sha256` in the manifest, per-chunk CRC32C; mismatch → fall back to the previous generation; if both fail → quarantine to `/quarantine/` (kept for export) and start from defaults for that store. User-visible: "Some data could not be read; restored from backup (age: 2 days)".
- **Content provenance:** campaign saves record the content packs and datapack in use; loading with a different content version applies the compatibility policy (§51.8, §62.7): substitutions listed, or "keep old pack" if available.
- **Backups:** campaign state keeps 3 rolling operation-boundary backups; settings keep 5 generations; a daily automatic "profile snapshot" (compressed, ≤ 20 MB, last 3) is stored in OPFS when the app is idle.
- **Write-failure policy:** `QuotaExceededError` → free space by evicting low-priority data (§44.5.5, §52.11), retry once; else block the write and prompt the user with options (export profile, delete replays/cache).

## 64.6 Persistence, eviction, export and import

- **Persistence request:** at first meaningful use (after the first mission) call `navigator.storage.persist()`; show the result in Data & Storage ("Persistent: granted / Best-effort: the browser may clear data when disk is low"). Installing the PWA often improves persistence [verify per browser].
- **Eviction detection:** on start, if the profile database is missing but a marker in a `localStorage`/Cache entry exists, show "Your saved data appears to have been cleared by the browser" with an import prompt.
- **Export (single file `.fsave`):** a zip containing `manifest.json` (profile, versions, hashes), CBOR/JSON exports of every IDB store, selected OPFS files (user missions, bookmarks; optionally replays/gallery), and checksums. Optional passphrase encryption (WebCrypto AES-GCM, PBKDF2/Argon2 [verify support] key derivation) for portability. Size typically 5–50 MB without replays.
- **Import:** validate manifest/hashes/schema; choose *Replace* or *Merge* (merge rules: settings → prefer imported or current by timestamp with a diff UI; stats → sum; unlocks → union; campaign runs → keep both under distinct run ids); migrations are applied to imported data; a pre-import snapshot allows undo.
- **Backup reminders:** after significant progress (e.g. 10 hours played since the last export) a non-modal reminder offers an export; configurable.
- **No cloud sync at launch:** cloud/sync would require accounts; out of scope (OQ). The `.fsave` format is the interchange.
- **Concurrency:** multiple tabs/windows are coordinated with the **Web Locks API** (`navigator.locks.request("fgame:profile:<id>", {mode:"exclusive"})` for writers) and `BroadcastChannel` (invalidation messages). A second tab opens read-only (menus) with a banner; only one tab can run a mission with the profile.
- **Privacy:** saves are local; the install id is random and never leaves the device without consent.

## 64.7 Data model excerpts

```json
{ "schema": "campaign_run/1", "run_id": "7K3M-2QX9-FA14", "profile": "pilot1", "seed": "7K3M-2QX9-FA14",
  "versions": { "game": "0.5.0", "content": "2026.10.1", "datapack": "geo-2026.09" },
  "difficulty": { "preset": "normal", "ri": 38, "assist_flags": [] },
  "operation_index": 14, "day": 37, "resources": { "credits": 1240, "fuel": 88000, "munitions": 5400, "spares": 310, "intel": 45 },
  "airports": [ { "id": "KMRY", "owner": "azure", "health": [1,1,0.8,1,1,0.6], "garrison": 4, "supply": 0.9 } ],
  "fronts": [], "fleet": [ { "tail": "AZ-114", "type": "core:ac/kestrel", "state": "ready", "hours": 41.2, "damage": 0 } ],
  "pilots": ["p-001", "p-002"], "ledger_head": "e-0412", "rng": { "campaign": "…state…", "events": "…state…" }, "legacy": { "lp_banked": 120 } }
```

```json
{ "schema": "saves_manifest/1", "store": "campaign_runs", "key": "7K3M-2QX9-FA14", "generation": 38, "chunks": 4,
  "sha256": "ab12…", "written": "2026-10-07T09:12:44Z", "prev_generation": 37 }
```

## 64.8 Acceptance criteria

- `AC-64.1` Kill-the-tab test: terminating the browser process at random times during writes (1 000 trials) never corrupts a campaign; the load always yields a valid state ≤ 30 s (checkpoint) or one operation boundary old (campaign).
- `AC-64.2` Migration corpus: saves from every released schema version load, migrate, and round-trip (migrate → export → import) with equal normalised content.
- `AC-64.3` Corruption injection (bit flips in 100 random chunks): detection rate 100 %; recovery from the previous generation succeeds in 100 % when a prior generation exists; quarantine contains the damaged data.
- `AC-64.4` Export → delete all site data → import restores settings, unlocks, campaign state, pilots, statistics, discoveries, bookmarks exactly (hash of normalised data equal); with a passphrase, a wrong passphrase fails without leaking data.
- `AC-64.5` Two tabs: the second tab cannot write; concurrent write attempts are serialised by Web Locks with no lost updates in a 10 000-operation stress test.
- `AC-64.6` Checkpoint write costs ≤ 20 ms main-thread time and ≤ 60 ms worker time for a Large battle (75 aircraft); resume restores state with identical state hash to the checkpointed value.
- `AC-64.7` Quota exhaustion test: filling the origin quota triggers the documented eviction/prompt flow and never loses saves.

---

# 65. STATISTICS

`P5 → P9 · M4 (mission stats) → M6 (career, campaign)` — The game tracks kills, deaths, sorties, missiles fired, missile hits, gun kills, air-to-ground kills, aircraft losses, successful landings, longest sortie, highest altitude, fastest speed, largest battle, and campaign victories (outline §114). Statistics are **derived from the authoritative event stream**, are local to the profile, and are protected against inflation.

## 65.1 Intent

- A satisfying, honest record of a pilot's career that supports mastery (accuracy, efficiency), not just totals.
- Feedback loops: debrief numbers (§53.3) and career screens reinforce skill expression (§4.2).
- No online leaderboards at launch (local only); the data model could feed leaderboards later.

## 65.2 Definitions (normative)

All events are produced by the Sim Core authority (`CombatEvent`, `FlightEvent`, `MissionEvent`) with entity ids, timestamps, and attributing player/slot where applicable.

| Statistic | Definition |
|---|---|
| **Sortie** | Begins when a player's aircraft leaves the ground (weight-on-wheels false continuously for ≥ 10 s) or an air start is created; ends at touchdown + stopped/taxi speed < 5 m/s for ≥ 5 s, ejection, destruction, or mission end while airborne. A mission may contain multiple sorties (e.g. after landing and rearming). |
| **Kill** | An enemy aircraft destroyed (state `destroyed` per §18) where the player is the **credited killer**: the entity whose weapon inflicted the damage event that moved the target over the destruction threshold, or the highest-contributing damager in the last 30 s if the destruction is by crash/fire after damage ("assist rule": contribution ≥ 50 % of accumulated damage credited as a kill; 20–49 % as an **assist**). Friendly fire, collisions with the ground by the target without damage in the last 30 s, and scripted despawns award nothing. |
| **Gun kill** | A kill where the credited damage source is a gun/cannon (§15.5). |
| **Missile kill** | A kill credited to a guided weapon (any seeker class). |
| **Missiles fired** | Count of guided weapon launches (`Launch` events by the player's aircraft); includes dud/failed launches that left the rail. |
| **Missile hit** | A fired missile whose warhead detonation (or proximity fuze event) produces **non-zero damage** on a hostile target (aircraft, ground, naval). Hit rate = hits ÷ fired. |
| **Air-to-ground kill** | A destroyed ground or naval target (vehicle, SAM, building flagged as a target, ship) credited to the player under the same attribution rules; civilian/protected targets excluded and recorded as **collateral** (negative). |
| **Death** | The player's aircraft is destroyed (including crashes, shoot-downs, structural failure) or the pilot dies/ejects and is captured/lost (campaign rule). Ejecting and surviving is an **ejection**, a loss of the aircraft but not a death (campaign pilot states, §41). |
| **Aircraft loss** | The player's aircraft destroyed or abandoned (death or ejection) — also counts AI wingmen losses in a separate "squadron losses" stat. |
| **Successful landing** | Touchdown on a runway/carrier deck satisfying survivability criteria (§11.13.3: sink rate, bank, gear state, speed) and a roll-out to taxi speed (< 15 m/s) with the aircraft intact (no crash event). Carrier traps count as landings (with wire number logged). |
| **Longest sortie** | Maximum airborne duration of a single sortie (seconds). |
| **Highest altitude** | Maximum geometric altitude MSL reached by the player's aircraft while alive (ft/m). |
| **Fastest speed** | Maximum **true airspeed** (and separately maximum Mach number) in level or climbing flight, including dives; stored in m/s and Mach, displayed in the user's units. |
| **Largest battle** | Maximum number of aircraft (all sides, alive) simultaneously within the battlefield during a mission the player participated in, evaluated at 1 Hz; also records largest engaged battle (aircraft within 100 km of the player). |
| **Campaign victories** | Count of campaign runs completed with a victory condition (§34.2), plus per-faction/difficulty breakdown; operations won are a separate counter. |

Additional tracked stats (for debriefs and career): shots fired/hit with guns (accuracy), flares/chaff expended, missiles defeated (evaded), time airborne, distance flown, fuel used, mission score, objectives completed, landings by type (runway/carrier), air-to-air refuelling count, assists, collateral incidents, ejections, bosses defeated (§29), airports captured (campaign), RI-weighted score (§59.4).

## 65.3 Event-sourced pipeline

```text
Sim authority events ──▶ MissionRecorder (per mission) ──▶ MissionResult (immutable record) ──▶ StatsAggregator ──▶ Profile stats (IndexedDB)
                                      └────────────────▶ Debrief UI (§53.3)                         └─▶ Career UI, exports
```

- The **MissionRecorder** (client side for local players, authority for all) accumulates counters from events; attribution is computed on the authority to avoid client disagreements; clients receive their final record in the end-of-mission message (and compute the same counters from their SSR as a cross-check).
- The **MissionResult** record is written once at mission end (and incrementally to checkpoints so a crash doesn't lose it): immutable; aggregated into profile totals idempotently (by `mission_id`), so retries never double-count.
- **Aggregations:** totals; per aircraft type; per weapon; per mode (sandbox/1v1/co-op/campaign); per RI band; per location (region); per month; personal bests (records with date and mission id).

## 65.4 Data model

```json
{ "schema": "mission_result/1", "mission_id": "m-20261007-0912-1", "profile": "pilot1", "mode": "sandbox", "provenance": { "seed": "FA3K-9T2M-QX71", "versions": {} },
  "ri": 38, "flags": { "debug_tainted": false, "invulnerable": false, "unlimited_ammo": false, "god_map": false, "time_accel_used": true },
  "duration_s": 1710, "sorties": [ { "aircraft": "core:ac/kestrel", "airborne_s": 1420, "landed": true, "ejected": false, "max_alt_m": 10400, "max_tas_mps": 395, "max_mach": 1.32 } ],
  "combat": { "kills": 4, "gun_kills": 1, "missile_kills": 3, "ag_kills": 0, "assists": 1, "deaths": 0, "missiles_fired": 7, "missile_hits": 4, "gun_rounds": 220, "gun_hits": 31, "cm_used": { "flare": 12, "chaff": 4 }, "missiles_defeated": 2, "collateral": 0 },
  "battle": { "peak_aircraft": 62, "peak_engaged": 24 },
  "objectives": { "completed": 3, "failed": 0 }, "score": 8420, "outcome": "success" }
```

Profile stats store totals, bests (with `mission_id` back-references), and histograms (e.g. engagement range at kill, kill altitude, g at kill).

## 65.5 Integrity and anti-inflation

Statistics count only when the session is **eligible**:

| Condition | Effect |
|---|---|
| `invulnerable`, `unlimited ammo/fuel` (sandbox flags), the debug "god" overlay (`god_map`; the Easy `map_info` dial is allowed and does not count), debug mutating tools (`debug_tainted`) | Mission result stored but **excluded from totals/records** (kept in the logbook with a flag) |
| Custom damage multipliers outside the allowed band (player 25 %–400 %, enemy 50 %–200 %) | Flag `custom_damage`; included in totals but excluded from personal bests |
| Time acceleration | Allowed; excluded from "longest sortie" in real-time terms (uses sim time), noted |
| Friendly fire/teamkills | Negative stat; no kills credited |
| Multiplayer | Authority-computed; clients cannot submit their own results; co-op kills credited by the attribution rule; 1v1 results recorded per round and match |
| Host-edited stats | Not possible; profile data is local, and the game does not promise tamper-proofness (no leaderboards) — local edits via devtools are possible and accepted |

## 65.6 Logbook and career

- **Logbook:** each sortie/mission appended (date, location, aircraft, mode, duration, outcome, score, kills, notable events); filter, search, export CSV/JSON; links to the replay if still stored (§52).
- **Career screen:** totals (outline list), accuracy (gun, missile), K/D, survival rate, favourite aircraft/weapon, charts (kills over time, altitude/speed records), personal bests with mission links, campaign wins by difficulty.
- **Milestones (optional, P10):** lightweight local milestones (e.g. "First carrier trap", "10 missiles defeated in a mission", "Fly 1 000 h"); no rewards that affect gameplay; hidden unless achieved by default; fully disable-able (accessibility/privacy).

## 65.7 UI

- **Debrief:** summary + kill timeline, accuracy bars, missile outcomes (hit/miss/defeated with reason from the replay event, §52.5), objectives, score with RI multiplier.
- **Career/Profile:** grid cards for the 14 outline statistics + detailed tabs; each number has a tooltip with its definition (§65.2).
- **In-flight:** no stats overlay by default (optional kill counter).

## 65.8 Acceptance criteria

- `AC-65.1` Attribution tests: 60 scripted engagements (guns, missiles, collisions, combined damage, friendly fire, crash after damage) produce the expected kill/assist/none credit and identical results on the authority and the client cross-check.
- `AC-65.2` Idempotency: re-aggregating the same `MissionResult` any number of times leaves totals unchanged; a crash during aggregation does not lose or duplicate a mission (fault-injection test).
- `AC-65.3` Eligibility: sessions with invulnerable/unlimited flags or `debug_tainted` never change totals or personal bests (property test over flag combinations).
- `AC-65.4` Definitions: sortie, successful landing, longest sortie, highest altitude, fastest speed, and largest battle match the definitions on 40 scripted flights (including touch-and-go, go-around, ejection, carrier trap).
- `AC-65.5` All 14 outline statistics are present in the Career screen and in the exports with correct values after a scripted 20-mission campaign of known outcomes.
- `AC-65.6` Import/export of statistics round-trips (§64.6) and merges by summing totals while retaining the better personal bests.

---

# 66. DEVELOPMENT ROADMAP

`Planning` — The game is developed in layers (outline §125): never the whole vision at once, but an architecture that always preserves it. The roadmap follows the outline's phases (§126–§131), adds a foundations milestone (M0) and a long-term content track (outline §145), and attaches testable exit criteria and quality gates to every milestone. Durations are planning estimates for a core team of 8–12 (engineering 6–8, art/audio 2–3, design/production 1–2) and are expected to be revised after M0.

## 66.1 Philosophy and rules

1. **Feel first.** "The aircraft must feel good to fly before adding extensive content" (outline §126): no content breadth work starts until the flight-feel gate (G1) passes.
2. **Vertical slices.** Every milestone ends with a playable build that exercises the full pipeline (data → sim → render → audio → UI) at its scope.
3. **Abstractions for the future.** Anything deferred has an interface and a stub that runs from M0 (§61.5). Replacing a stub must not change callers.
4. **Cut by priority, not by area.** When time is short, the order in §68 decides: flight physics → geographic sandbox → combat → AI → procedural missions → large battles → LAN/VLAN → 1v1 → campaign → advanced systems.
5. **Always shippable.** The main branch is deployable to the staging URL on every merge; features land behind flags.
6. **Measure continuously.** Performance (§60.9), conformance (§11.19), determinism (ADR-10) and accessibility (§57.8) checks run in CI from the milestone in which they become applicable.

## 66.2 Milestone overview

| ID | Milestone | Outline phase | Est. duration | Playable build | Primary chapters |
|---|---|---|---|---|---|
| **M0** | Foundations | — | 2 months | Tech demo: Kestrel-less "hello world" loop (render + sim + input + CI) | §61, §60.2, §62 |
| **M1** | Flight Prototype | §126 | 4 months | One aircraft, basic terrain, basic weapon, basic AI | §11, §12 (Kestrel), §44 (basic), §54 (basic), §58 |
| **M2** | Geographic Sandbox | §127 | 5 months | Globe, location selection, streaming, airports, battlefield generation | §7–§10, §44–§47, §53 (world map) |
| **M3** | Combat | §128 | 6 months | **First Playable Build** (§67.1) at month 3; **Second Playable Build** (§67.2) at month 6 | §15–§19, §21, §18, §48, §49 |
| **M4** | Procedural Missions | §129 | 5 months | Generated missions with phases, reinforcements, bosses, events | §25–§29, §30, §20, §69 |
| **M5** | Multiplayer | §130 | 5 months | **Third Playable Build** (§67.3): LAN/VLAN co-op + 1v1, regions, ground/naval, larger battles | §31–§33, §21–§23, §52, §60 |
| **M6** | Campaign | §131 | 6 months | **Campaign Build** (§67.4) | §34–§43, §41, §48.7 |
| **M7** | Long-term content | §145 | ongoing | Content releases, community hub (OQ-10), advanced systems | §66.7 |

Total to Campaign Build ≈ 33 months from the start of M0 on a single team; art, audio and data tracks run in parallel and are therefore not on the critical path unless noted. M0 and M1 may overlap by up to one month.

**Data models reserved from M1.** To avoid retrofitting persistence and campaign hooks, from M1 the following exist as real (if mostly empty) types: stable entity and airframe ids; `PilotId` and pilot record stubs; `AirframeId` with hours/damage state; `PersistenceSink`, `IForceProvider`, `IAirframeSource`, `StrategicContext` interfaces (§34.6); `MissionEvent` stream and `MissionResult` record (§65); `provenance` blocks on missions (§25.12); content ids and pack versions (§62.3); `profile_id` namespacing (§64.2). The campaign build can be compiled out without changing Sim Core code (`AC-6.5`).

## 66.3 Phase specifications

### M0 — Foundations

*Scope:* repository, CI (lint, unit, WASM+native builds, cross-build hash test), Rust workspace skeleton, Sim Worker loop with fixed tick, snapshot buffer and Render Worker with a lit terrain patch (WebGPU + WebGL2 fallback), input ring and shaping, content baker stub with one aircraft file, performance harness, debug overlay (FPS, stage timing), compat check screen, Service Worker/COOP-COEP deployment, ADR repository. Extension seams (§61.5) with stub implementations.
*Exit criteria:* build and deploy pipeline green; sim 60 Hz stable in a worker while render runs independently; WASM vs native hash equal after 10⁶ ticks (`AC-11.7` precursor); cross-origin isolation works on the staging host; a debug build runs on the three reference machines (Minimum, Recommended, High).

### M1 — Flight Prototype (outline §126)

*Scope (outline):* one aircraft (Kestrel), flight physics, camera, basic terrain, basic weapon, basic AI. *Specifically:* §11 flight model (RK4, ISA, aero tables, FCS Standard/Full/Reduced/Manual), ground handling and takeoff/landing, HUD core symbology (§54.3), cockpit and chase cameras, keyboard/mouse/gamepad/HOTAS input (§58), simple terrain (low-resolution global + local patch, §44 basic) and procedural flat/rugged test arenas, one gun and one IR missile (simple PN), one AI "bandit" (state machine) that flies a pursuit and defends, audio engine sounds and warning tones (§56 basic), Performance Conformance Test for the Kestrel (§11.19), first-run flow and settings (§53 minimal), PWA install/offline shell.
*Exit criteria:* **Gate G1 (flight feel)** passes (§66.4); conformance test within tolerances (`AC-3.2`); 60 FPS on Recommended with the arena; input-to-control-surface ≤ 25 ms p95; 30-minute soak with no leaks; ≥ 80 % of external playtesters can take off, fly, shoot a bandit and land within 10 minutes (`AC-4.5` precursor).

### M2 — Geographic Sandbox (outline §127)

*Scope (outline):* globe/world map, geographic coordinates, location selection, terrain streaming, airports, battlefield generation, boundaries. *Specifically:* §7 geodesy and frames, §8 battlefield system and validator, §9 world map (search, coordinate entry, bookmarks, selection tools), §10 airports (data merge, flattening, procedural completion, improvised strips), §44 streaming pipeline, `.gpk` GeoPack and the `geo-pipeline` for a pilot set of regions (e.g. 6 regions), §45 offline cache UI, §46 terrain shading/biomes/water, §47 buildings/roads (basic tiers), atmosphere/sky (§24 basic), sandbox configuration (§30 basic), load plan (§44.9).
*Exit criteria:* the outline's test — **the player can select two geographic locations and fly between them** — with continuous streaming and no holes (`AC-44.1`); `AC-3.1` (1 000 random locations → playable) at ≥ 95 % in M2 (≥ 99 % by M4); horizon dip and curvature visible (screenshot tests); streaming budgets (`AC-44.4`); airport data merge `AC-10.1`.

### M3 — Combat (outline §128)

*Scope (outline):* guns, missiles, radar, RWR, countermeasures, AI, ground targets, damage. *Specifically:* §15 weapons (26-weapon target; M3 delivers the core set of ≥ 10), missile model with DLZ baker, §16 radar and avionics (modes, LOS service with max-mip, notching), §17 EW basics (chaff/flares, jamming simple), §18 damage model (component), §19 pilot AI (utility + predictive BFM at skill levels), §20 squadrons/formations/wingman orders, §21 ground units and SAM sites, §23 civilian traffic (basic), §48 airbase operations (takeoff, landing, rearm/refuel, scramble basic), §49 long-range operations (autopilot, cruise, time acceleration, AAR), §50 navigation, §54 HUD (DLZ, RWR, radar page), §55 tactical map, §56 audio (weapons, RWR, radio v1), fixed objectives in hand-authored missions.
*Exit criteria:* **First Playable Build** (§67.1) at the middle of M3; **Second Playable Build** (§67.2) at the end; hit-rate sweeps within targets (`AC-15.5`); AI never exceeds the authored envelope (`AC-3.3`); sensor budgets (`AC-16.*`); damage model tests (`AC-18.*`); Gate G2 (fun gate) passes at the First Playable Build.

### M4 — Procedural Missions (outline §129)

*Scope (outline):* objectives, reinforcement logic, mission phases, friendly forces, enemy forces, boss encounters, dynamic events. *Specifically:* §25 mission model/DSL/VM, §26 generator with solvability checks, §27 mission types (16), §28 escalation and the intensity director, §29 bosses and aces, §69 anti-repetition (DNA, history, twists, approach diversity), §30 sandbox configuration complete, §31 1v1 geographic arenas (single-machine vs AI), seeds and share links (§26.3), debrief and statistics (§65), replay (SSR) and photo mode v1 (§52), difficulty dials (§59), accessibility baseline (§57).
*Exit criteria:* 10⁴-seed batch validity ≥ 99.5 % (`AC-26.*`); diversity (`AC-3.4`: mean DNA distance ≥ 0.45); solvability; escalation tests (`AC-28.*`); Gate G3 (replayability): playtesters report "substantially different battle" at the same location in ≥ 80 % of paired sessions (§71.3).

### M5 — Multiplayer (outline §130)

*Scope (outline):* LAN, VLAN/direct IP, co-op, synchronisation, player aircraft, weapons, AI replication, lobby. *Specifically (browser):* §33 transport stack (WebRTC primary, WebSocket LAN fallback, WebTransport where available), serverless invite blobs and QR, listen-server in a tab and the dedicated server build, prediction/reconciliation/lag compensation, snapshot replication with relevance filtering, voice, §32 lobby/roles/co-op, §31 1v1 online modes, ground/naval forces (§21–§22), larger battles (Huge) and the Performance Governor (§60.7.3), additional regions (≥ 20), improved procedural generation, replay completeness (OSR), UGC editor v1 (§51).
*Exit criteria:* 16-client soak (`AC-33.*`), hit registration under impairment (§33.14), bandwidth ≤ budget (`AC-60.8`), Gate G5 (network), LAN-only play without internet verified (`AC-61.8`).

### M6 — Campaign (outline §131)

*Scope (outline):* airport graph, factions, strategic map, airport ownership, capture, persistent damage, resources, progression, elite squadrons, campaign bosses. *Specifically:* §34–§43, persistent airbase damage (§48.7), fog of war and intelligence (§42), emergent storytelling (§43), pilot progression (§41), Legacy Points meta-progression (§38.5), save system complete (§64), campaign UI (§35.6), campaign generation (§34.3).
*Exit criteria:* the **Campaign Build** (§67.4): start with one airport, capture neighbours, face counterattacks, meet a recurring ace, push across a region and reach a strategic victory/defeat; 20-run automated campaign soak (no soft-locks; every run terminates); save/migration tests (`AC-64.*`); Gate G6 (campaign loop retention test, §71.3).

### M7 — Long-term content (outline §145)

See §66.7.

## 66.4 Quality gates and playtests

| Gate | When | Criteria (pass = all) |
|---|---|---|
| **G1 Flight feel** | End of M1 | (a) Blind A/B vs a reference sim and vs an arcade reference with ≥ 12 testers: feel rated ≥ 4.0/5 for "believable" and "satisfying" (median) and ≥ 3.8 for "easy to learn" on Full Assist; (b) Kestrel conformance test passes; (c) no assist level contains a fixed-rate-turn fallback (NN-2) |
| **G2 Fun gate (first playable)** | End of FP1 | ≥ 20 external playtesters complete the 11-step script (§67.1); ≥ 70 % would "play another sortie"; median "fun" ≥ 3.8/5; blocking issues list empty. The outline's criterion: "If this is fun, the project has successfully demonstrated its core." |
| **G3 Replayability** | End of M4 | Pairs of sessions at the same location with different seeds: ≥ 80 % of testers rate them "substantially different"; `AC-3.4` passes; no seed shows an unwinnable setup in 10⁴ samples |
| **G4 Art/audio** | M2 look-dev, M4 audio, before each content drop | Reference boards signed off (AC-46.5), aircraft sound distinctiveness (AC-56.7), HUD readability checks, icon/symbol audit (AC-57.1) |
| **G5 Network** | End of M5 | §33.14 criteria; 16-client soak; LAN-only verified; NAT-less invite flow success ≥ 95 % on 20 home networks [target] |
| **G6 Campaign loop** | End of M6 | ≥ 60 % of testers continue past their third operation voluntarily; campaign completes in 6–15 hours for Normal; zero soft-locks in the 20-run soak |
| **G7 Accessibility** | M4 baseline, M5 full, M7 external review | §57.9 criteria; external accessibility testers |
| **G8 Licence & IP** | Each datapack/content release | Descriptor audit (AC-45.4); naming/IP review of fictional content (ADR-11); music/audio licences |
| **G9 Release readiness** | Before each public release | All AC for the milestone green; no P0/P1 bugs; performance gates; security/CSP audit (AC-61.6); crash-free session rate ≥ 99.5 % in beta (opt-in telemetry) |

Playtest programme: weekly internal sessions from M1; monthly external sessions from M2; structured protocol (script, think-aloud, SUS usability score ≥ 75 target at M4, telemetry with consent). Findings feed the design priorities (§68).

## 66.5 Workstreams and critical path

| Workstream | Contents | Owners (roles) |
|---|---|---|
| Sim core & flight | §11, §18, §61.4 | Gameplay/sim engineers |
| Weapons & sensors | §15–§17 | Gameplay engineers + designer |
| AI | §19–§20 | AI engineer + designer |
| World & data | §7–§10, §44–§47, `geo-pipeline` | World/tools engineers |
| Rendering | §44, §46, §54, §60 | Graphics engineers |
| Missions & campaign | §25–§43, §69 | Designers + systems engineers |
| Networking | §33 | Network engineer |
| UI/UX & accessibility | §53–§58 | UI engineers + designer |
| Audio | §56 | Audio engineer + sound designer |
| Content (aircraft, weapons, units) | §12–§15, §62 | Technical artists + designers |
| QA & tools | §61.10, §63 | QA/tools engineers |

**Critical path:** M0 → flight model (§11) → terrain/streaming (§44) → weapons/sensors (§15–§16) → AI (§19) → mission system/generator (§25–§26) → networking (§33) → campaign (§34+). Art and audio pipelines (models, cockpit, HUD, sounds) start by M1 and are budgeted to avoid blocking (OQ-05: kit-bashed aircraft acceptable).

## 66.6 Cut lines and scope control

If the schedule slips, features are cut in the order of §68 from the bottom, within these rules:

1. A cut system keeps its interface and a stub (§61.5): nothing already integrated is removed.
2. M3 may ship with 12 weapons instead of 26 (stores keep the schema), with 8 aircraft instead of 12 (roster slots keep the schema).
3. M5 may ship 1v1 and co-op without WebTransport/voice (WebRTC data channels and WebSocket LAN remain).
4. M6 may ship a 2-faction campaign without the storylet system (the event stream remains).
5. The WebGL2 compatibility path may be dropped if WebGPU coverage in the target browsers is sufficient at release [verify], with a documented compat message.

## 66.7 Long-term content (outline §145)

The architecture must eventually support: many aircraft; many weapons; fictional factions; real-world geography; large-scale air battles; naval warfare; advanced air defence; electronic warfare; campaign progression; multiplayer; user-generated missions; replay tools; and cinematic tools. The roadmap tracks them as release trains after M6:

| Track | Planned content / capability | Enabled by |
|---|---|---|
| **Aircraft & weapons** | Roster growth beyond 12 aircraft (target 30), new weapon classes (anti-ship, cruise missiles, glide bombs, directed-energy research item), era packs (G3 → G5) | §62 data packs |
| **Factions** | Additional fictional factions and variant doctrines; faction-specific campaigns | §37, §62 |
| **Regions & data** | More pre-packaged regions; improved building coverage; seasonal imagery-free styles; polar support (§45.8) | §45 |
| **Large-scale battles** | Increase of caps with performance work (WASM threads, GPU culling); 250+ aircraft entertainment mode | §60 |
| **Naval & subs** | Deeper naval warfare, ASW, submarines, port operations | §22 |
| **Air defence** | IADS networks with C2, mobile SAM tactics, passive sensors | §21 |
| **EW** | Expanded EW roles, ESM trees, deception, dedicated EW multiplayer play | §17 |
| **Campaign** | New operations, strategic modifiers, narrative arcs, daily/weekly challenge seeds (seed links) | §34–§43 |
| **Multiplayer** | Larger sessions (32 humans with dedicated server), spectator mode, tournament tooling | §33 |
| **UGC** | Community hub (OQ-10), custom aircraft (signed packs), scripting expansion in the DSL | §51, §62 |
| **Replay & cinematic tools** | Expanded camera tools, editing timeline, export presets | §52 |
| **Platform** | WebXR, desktop wrapper (OQ-09), mobile/tablet UI (research) | §58.8 |

Each release train follows the same gates (G7–G9). Content releases are data-only whenever possible (hot-updatable through the Service Worker).

## 66.8 Release plan (web)

| Channel | Audience | Cadence |
|---|---|---|
| `nightly` | Developers | Every merge |
| `alpha` | Internal + invited (from M3) | Weekly |
| `beta` | Public opt-in (from M5; opt-in telemetry) | Bi-weekly |
| `stable` | Everyone | Monthly (content) / quarterly (features) |

The multiplayer protocol version is pinned per release train; mismatches are refused cleanly (§62.7). A "what's new" panel and migration notes accompany each release.

## 66.9 Risks (pointer)

The top risks (Rust/WASM talent and tooling, WebGPU coverage and driver variance, streaming performance, flight-feel tuning, AI quality at scale, browser networking constraints, data licensing, scope) are tracked with mitigations and owners in Appendix F.

## 66.10 Acceptance criteria (process)

- `AC-66.1` Every milestone has an exit checklist whose items are each testable (automation or scripted playtest) and recorded in the project tracker; a milestone is closed only when all items pass.
- `AC-66.2` Each stub in §61.5 has a contract test in CI from its introduction milestone; the "replace a stub" change set touches no caller code (verified by code-ownership diff in two sample replacements).
- `AC-66.3` The staging URL always serves a build that passes the smoke suite (boot, compat check, one mission start, offline reload).
- `AC-66.4` Gates G1–G9 are defined with thresholds before the milestone starts and are reviewed (not weakened) at the gate meeting; any change is recorded as an ADR.

---

# 67. PROTOTYPE REQUIREMENTS

`Planning · M1–M6` — The playable builds prove the concept in increasing depth (outline §132–§135). Each build has a scripted acceptance run so "it works" is objective. The first playable build "only needs to prove the central concept."

## 67.1 First Playable Build (outline §132)

*Target:* middle of M3. *Thesis:* choosing a place on Earth and fighting there is fun.

**The 11-step script (outline) with measurable conditions:**

| # | Outline step | Acceptance condition | Time budget |
|---|---|---|---|
| 1 | Open a geographic map | Globe visible, rotate/zoom works, search box works | ≤ 3 s cached (§0.6.6) |
| 2 | Select a location | Click on land (or choose an airport/search result): coordinates shown, battlefield footprint preview | ≤ 1 s feedback |
| 3 | Generate a battlefield | Medium battlefield (120 km), boundary validated (§8.8), two airports found or improvised (§10.8) | ≤ 6 s (data cached), ≤ 12 s (streamed) |
| 4 | Load terrain | Load plan completes; sim-critical tiles resident (§44.9) | ≤ 25 s for Medium to launch |
| 5 | Spawn an aircraft | Kestrel on the runway (hot start) or air start | Immediately after load |
| 6 | Fly | Take off, climb, turn, land possible; curvature/horizon correct | — |
| 7 | Spawn an enemy | One AI bandit (G4 fighter, Veteran) spawned at the boundary or from an enemy base | 60–120 s after launch (§4.4) |
| 8 | Detect it | Radar (simplified modes) or visual + RWR cues; HUD target box | — |
| 9 | Fire a missile | Launch with DLZ cues (one radar missile + one IR missile) | — |
| 10 | Destroy it | Hit/kill with a proper explosion, wreckage; kill feedback | — |
| 11 | Return to the map | Land (optional) or end mission; debrief; globe re-opens | ≤ 4 s (§4.4) |

*Included:* §7–§10 (basic), §11 (Kestrel), §15 (2 missiles + gun), §16 (simplified radar + RWR), §18 (aggregate damage), §19 (single bandit AI at three skill levels), §44 (streaming for Medium), §54 (core HUD), §56 (essentials), §53 (world map + minimal flow).
*Explicitly not included:* multiplayer, campaign, mission generator beyond a single "dogfight" template, ground and naval forces, weather (static clear/cloudy), carriers, replay, user content, photo mode, advanced EW.
*Success measure:* **Gate G2** (§66.4). *Failure response:* if the thesis is not fun, iterate on pacing, enemy behaviour and presentation before building further breadth (the core is the product).

## 67.2 Second Playable Build (outline §133)

*Target:* end of M3 (plus early M4). *Adds:* multiple aircraft (≥ 5: Kestrel, Peregrine, Swift, Bittern, Badger), better AI (utility + BFM, skill ladder §19.6), more weapons (≥ 10 incl. bombs/rockets/SARH), airports, **takeoff, landing, rearming, refuelling** (§48), objectives and **reinforcements** (hand-authored and early generator templates: Air Superiority, Strike, Intercept), friendly wingmen with orders, tactical map (§55), RWR/countermeasures complete.
*Acceptance run (scripted):* (1) select an airbase pair 200 km apart; (2) pick Peregrine with 4×MRM + 2×WVR; (3) launch; (4) intercept a raid detected by AWACS; (5) objective changes (strike appears); (6) kill a SAM site; (7) a reinforcement wave scrambles from the enemy base (visible taxi/take-off at a nearby base); (8) land, rearm, refuel and take off again within the same session; (9) debrief with statistics. Passes when the run completes in ≤ 45 minutes of play with no sim errors and at least 8/10 external testers complete the first six steps.

## 67.3 Third Playable Build (outline §134)

*Target:* end of M5. *Adds:* **multiplayer** (LAN/VLAN, co-op with roles), **multiple geographic regions** (≥ 20 packaged), **ground forces**, **naval forces**, **larger battles** (Large/Huge with the governor), improved procedural generation (16 mission types, twists, DNA anti-repetition).
*Acceptance run:* 4 human clients + dedicated server/host + 70 AI, Large battlefield in a new region, ground convoy and naval task group objectives, air superiority + strike + EW roles, escalation to the Climax phase, a boss encounter; all clients finish with consistent results; replays saved; network budgets met (`AC-60.8`); no desync alerts in a 30-minute session; frame-time gates on Recommended hardware.

## 67.4 Campaign Build (outline §135)

*Target:* end of M6. *Adds:* strategic map, airport graph, faction ownership, persistent damage, resources, roguelite progression, elite squadrons, campaign victory/defeat.
*Acceptance run (automated + human):* (1) start a new run (seed S); (2) one airport owned, neighbours hostile; (3) pick an operation; (4) win; (5) capture the airport, the map changes (new reach, supply); (6) the enemy counterattacks a base (defence operation); (7) an ace squadron appears, survives, reappears two operations later (identity continuity, §29/§41/§43); (8) persistent damage affects sortie rates (§48.7); (9) resources constrain loadouts; (10) progress to a strategic victory; (11) run summary and Legacy Points awarded (§38.5), new run unlocks; automated AI-vs-AI campaign soak: 20 runs × 3 seeds terminate with defined outcomes and no soft-locks.

## 67.5 Prototype hygiene

- Prototype code is production code behind stubs (no throwaway rewrite): the milestone builds are the real engine at limited scope.
- Each playable build ships with a **README card** in-game (what's in, what's stubbed, known issues, feedback link) and a feedback form (local export/mail-to; no server).
- Telemetry (opt-in) records the funnel of the scripted steps to measure drop-off.

## 67.6 Acceptance criteria

- `AC-67.1` The First Playable script (§67.1) is automated as a smoke test (headless sim steps 5–11 with scripted inputs) and as a human checklist; both pass on the Recommended profile.
- `AC-67.2` The Second Playable run (§67.2) completes with ≥ 8/10 external testers through step 6 and 100 % through step 9 in the automated version.
- `AC-67.3` The Third Playable run (§67.3) passes the 30-minute 4-human soak with no desync alerts and p99 frame time within budget.
- `AC-67.4` The Campaign Build soak (§67.4) completes 60 runs with zero soft-locks and ≥ 95 % of runs reaching a victory/defeat within the expected operation count range (§34).

---

# 68. DESIGN PRIORITIES

`Governance` — If development constraints force prioritisation, the order of the outline (§146) decides; the non-negotiable design rules (outline §147) are never traded away. This chapter makes the priorities operational: how they break ties, what each priority's "done" means, and how each rule is enforced.

## 68.1 Priority order (outline §146)

| Priority | Area | Statement (outline) | "Good enough to proceed" definition | Primary chapters | Milestone |
|---|---|---|---|---|---|
| **P1** | Flight Physics | Aircraft must feel excellent | Gate G1; conformance test green; assist presets tuned; ground handling and stall behaviour credible | §11, §12 | M1 |
| **P2** | Geographic Sandbox | Players must be able to choose where they fight | Two-location test (§66.3 M2); `AC-3.1` ≥ 99 %; streaming without holes | §7–§10, §44–§47 | M2 |
| **P3** | Combat | Weapons, radar, missiles, countermeasures and damage must work well | Hit-rate sweeps (`AC-15.5`); sensor tests; damage tests; Gate G2 | §15–§18, §48 | M3 |
| **P4** | AI | Enemy and friendly aircraft must behave intelligently | AI ladder monotone (`AC-59.4`); same-physics audit (`AC-3.3`); blind-test "feels like a pilot" ≥ 4/5 | §19–§20 | M3–M4 |
| **P5** | Procedural Missions | The game needs replayable scenarios | `AC-3.4`; Gate G3; solvability 100 % | §25–§29, §69 | M4 |
| **P6** | Large Battles | The simulation should scale | PS-1/PS-2 gates; governor behaviour | §60 | M5 |
| **P7** | LAN/VLAN | Reliable multiplayer | Gate G5 | §32–§33 | M5 |
| **P8** | 1v1 | Competitive combat | Fair-match audit, 1v1 modes tests | §31 | M5 |
| **P9** | Roguelite Campaign | Persistent strategic warfare | Gate G6; soak | §34–§43 | M6 |
| **P10** | Advanced Systems | EW, naval depth, logistics, civilian systems, detailed airports, pilot progression, etc. | Per-system criteria | §17, §22, §23, §41 | M3–M7 |

## 68.2 How priorities are applied

1. **Resource allocation:** engineering effort is allocated top-down. A lower priority may start only when the higher priorities' exit criteria are met, except for *interfaces and data models* (always reserved, §66.2) and *art/audio pipelines*.
2. **Conflict resolution:** when two requirements conflict (e.g. a menu feature vs the flight experience), the higher priority wins; the decision is recorded as an ADR.
3. **Regression rule:** a change that degrades a higher-priority metric (e.g. flight latency, G1 results) to benefit a lower-priority feature is rejected unless explicitly waived by the product owner with a recorded rationale and an expiry.
4. **Cut ladder:** (§66.6) cuts proceed from P10 upwards. Within a priority, cut by player value per cost.
5. **Priority in tooling:** the issue tracker labels every item P1–P10 (the tags used in this document's chapter headers); sprint planning reviews the distribution weekly (target: ≥ 60 % of capacity on the current milestone's top two priorities).

## 68.3 Non-negotiable design rules (outline §147) and their enforcement

The game should not … (rules NN-1…NN-12 in §3.9). Operational enforcement:

| Rule | What the game does instead | Tests / audits | Owner |
|---|---|---|---|
| **NN-1 Rely entirely on static maps** | Every battlefield derives from real geography; static authored arenas are *only* tutorial/test content and are marked as such | `AC-3.1`; content audit (no mission template references a fixed battlefield id) | World/design |
| **NN-2 Use completely arcade flight physics** | Full 6-DOF in all assists; Full Assist is a control law above the same physics | G1(c); code audit for "fixed-rate turn" paths; Manual envelope tests | Flight |
| **NN-3 Make missiles guaranteed hits** | Seeker/guidance/kinematic/countermeasure model; Pk emerges | `AC-15.5` monotonic ordering; no "auto-hit" flag in code (lint) | Weapons |
| **NN-4 Make bosses bullet sponges** | Capability-based bosses; no HP multipliers | `AC-29.*` inspection of resolved damage parameters; difficulty fairness `AC-59.5` | Missions/AI |
| **NN-5 Make AI strong through impossible physics** | Same physics, same sensors; skill = decisions and execution | `AC-3.3` telemetry audit; `AC-59.3` fairness audit | AI |
| **NN-6 Make every mission identical** | Anti-repetition systems (§69) | `AC-3.4`; DNA distribution CI plots | Missions |
| **NN-7 Make the campaign a linear mission list** | Graph-based strategic map, dynamic enemy, events | Campaign soak: branching factor and path diversity metrics (§34.9) | Campaign |
| **NN-8 Require internet connectivity for LAN gameplay** | Serverless invite blobs, LAN WebRTC/WebSocket, offline caches | `AC-61.8` offline matrix; LAN test with the network disabled | Network/platform |
| **NN-9 Bury the player in simulator complexity** | Dials, Standard Assist default, contextual hints | Usability metrics (SUS ≥ 75), first-flight completion `AC-53.7`, `AC-4.5` | UX |
| **NN-10 Sacrifice the core flight experience for excessive menus** | Menu budgets: ≤ 5 clicks to flight; in-flight UI minimal; menu code never runs on the flight critical path | `AC-53.1`, `AC-53.4` | UX |
| **NN-11 Hard-code content that should be procedural** | Missions, forces, placement, events are generated from rules/data | `NoHardcodedContentLint` (`AC-62.4`); mission template audit | Engineering |
| **NN-12 Sacrifice future expandability for short-term convenience** | ADRs, extension seams, versioned data | `AC-61.5`, `AC-66.2`; ADR review at each milestone | Architecture |

**Waivers:** none for NN-1…NN-12. A temporary violation during a milestone (e.g. a static arena in M1) is allowed only if tracked as technical debt with an exit milestone.

## 68.4 Decision heuristics (tie-breakers)

When the specification is silent, apply in order:

1. **Does it make flight feel better or worse?** (P1 always wins.)
2. **Does it preserve geographic freedom?** (A solution that requires pre-authored maps is wrong.)
3. **Is the outcome determined by systems the player can understand and exploit?** (Emergent over scripted.)
4. **Does the AI play fair?** (Same physics, same information, §19.2.)
5. **Does it work offline and on LAN?** (NN-8.)
6. **Is it data, not code?** (NN-11, ADR-13.)
7. **Can the simplest version be extended later without rewrites?** (NN-12.)

## 68.5 Acceptance criteria

- `AC-68.1` Every backlog item carries a priority tag P1–P10 and a milestone; the dashboard shows effort distribution per priority each sprint; a review is triggered if < 60 % of capacity is on the top two active priorities for two consecutive sprints.
- `AC-68.2` Each NN rule has at least one automated test/audit in CI or a scripted review at each milestone gate; the audit report is attached to the gate record.
- `AC-68.3` Regression rule: the gate checks for flight latency, G1 metrics and the conformance test run on every release candidate; any degradation > 5 % blocks the release unless waived in an ADR.

---

# 69. ANTI-REPETITION SYSTEMS

`P5 · M4 (core) → M6 (campaign integration)` — Procedural generation must actively avoid repetitive gameplay (outline §136). The game should not repeatedly produce "Destroy five fighters." This chapter specifies how variation is **measured** (mission DNA), **enforced** (history penalties and re-rolls), **injected** (twists, support/threat variety, phases), and **protected** (player freedom, outline §138; cinematic moments, outline §137).

## 69.1 Intent and design rules

1. **Combine systems, not just numbers.** Variety comes from combining independent generators (objectives, terrain use, enemy composition, weather, timing, reinforcements, support, threats, aircraft, phases), not from jittering counts.
2. **Measure it.** If repetition cannot be measured, it will regress. Every generated mission has a **DNA vector**; CI tracks distribution statistics (`AC-3.4`).
3. **Variety must be playable.** Variation is only valid if the mission remains solvable, fair and fun (validators in §26.9 remain authoritative).
4. **Same place, different war.** At a fixed location, 200 generations must be substantially different (mean DNA distance ≥ 0.45).
5. **Player-controlled variety.** The player can lock any axis (e.g. fix the mission type) and the system still varies the rest.

## 69.2 Mission DNA

The DNA is a fixed-structure descriptor computed from a `MissionDef` (not from play):

| # | Component | Encoding | Distance `d_k` ∈ [0,1] | Weight `w_k` |
|---|---|---|---|---|
| 1 | **Mission type / template variant** | categorical (type id, variant id) | 0 if equal type and variant; 0.5 if same type, different variant; 1 otherwise | 0.15 |
| 2 | **Objective structure** | canonical hash of the objective graph shape (kinds, ordering, dependencies, hidden/revealed) | multiset Jaccard distance of the objectives' (kind, priority, depends-on-kind) tuples (a metric; approximates graph-edit distance) | 0.15 |
| 3 | **Terrain use** | histogram over {open, mountains/valley, coast/ocean, urban, forest, desert} of the mission-relevant area (objective neighbourhoods + ingress routes), 6 bins | total-variation distance ½‖p−q‖₁ | 0.08 |
| 4 | **Enemy composition** | distribution over roles {fighter, interceptor, strike, bomber, EW, AWACS, helicopter, ground-AD, armour, naval, boss} (11 bins) weighted by threat value | total-variation distance | 0.15 |
| 5 | **Weather** | class {clear, scattered, broken, overcast, storm, fog, snow} + visibility band | 0 same class & band; 0.5 same class; 1 otherwise | 0.05 |
| 6 | **Time** | local solar time (hours) | circular distance `min(\|Δ\|, 24−\|Δ\|)/12` | 0.04 |
| 7 | **Reinforcement pattern** | (wave count, mean spacing, composition class sequence) | 0.5 × (absolute difference of wave counts)/max(wave counts, 1) + 0.5·normalised edit distance of composition sequences | 0.10 |
| 8 | **Support units** | set of {tanker, AWACS, EW support, SEAD package, carrier, JTAC/forward base, none} | Jaccard distance | 0.06 |
| 9 | **Threat mix** | vector {SAM density, AAA density, ship AD, boss present, EW intensity, fighter CAP level} (6 values in [0,1]) | mean absolute difference | 0.08 |
| 10 | **Aircraft types** | sets of enemy aircraft types and suggested player types | Jaccard distance (enemy 0.7, friendly 0.3 blend) | 0.07 |
| 11 | **Phase structure** | ordered phase-id sequence with branching signatures | normalised Levenshtein distance in the Yujian–Bo form `2·ed/(\|a\|+\|b\|+ed)` (a true metric; the same form is used for the reinforcement composition sequences) | 0.07 |

```text
D(A, B) = Σ_k w_k · d_k(A, B)          Σ w_k = 1.00        D ∈ [0,1]
```

Two missions are **too similar** if `D < 0.15`; a set is **diverse** if its mean pairwise `D ≥ 0.45`. The DNA is computed in the generator at the end of the pipeline (and stored in `provenance` for analysis), uses only data in the `MissionDef`, and is deterministic. Worked example: two missions of the same type but different variants, same objective structure, the same terrain bins, different enemy composition (TV 0.4), same weather class and band, 6 h apart, different reinforcement patterns (0.5), identical support units, identical threat mix, different aircraft sets (Jaccard 0.6), different phase sequences (0.4) give `D = 0.15·0.5 + 0.15·0 + 0.08·0 + 0.15·0.4 + 0.05·0 + 0.04·0.5 + 0.10·0.5 + 0.06·0 + 0.08·0 + 0.07·0.6 + 0.07·0.4 = 0.075 + 0.06 + 0.02 + 0.05 + 0.042 + 0.028 = 0.275` — above the "too similar" threshold, below the diverse mean, i.e. acceptable but not remarkable.

## 69.3 History and penalties

The generator keeps a **history ring** of the last 20 missions per profile (stored locally; campaign operations keep their own ring) with each mission's DNA and component choices. When selecting any categorical option (mission type, template variant, objective kind, enemy composition archetype, support package, twist, weather class, time band, boss archetype), its sampling weight is multiplied by a recency factor:

```text
m(option) = max( 0.05, Π_{j ∈ history, option used in mission j} ( 1 − 0.5 · 0.85^(j−1) ) )         j = 1 for the most recent mission
```

Examples: used only in the last mission → m = 0.5; used in the last three → m = 0.5 × 0.575 × 0.639 = 0.184; used five missions ago only → m = 1 − 0.5·0.85⁴ = 0.739. Options never used are unaffected. After generation the **DNA check** applies: if `D < 0.15` against any of the last 20 missions, the generator re-rolls the scenario/variant once (new sub-stream) and, if still too close, accepts the more distant of the two. Penalties are *soft*; they never make a user-locked option impossible. In multiplayer, the host's history is used. Players can clear the history ("fresh start"), and seeds shared via links bypass the history (a shared seed must reproduce).

## 69.4 Twists

Twists are optional modifiers layered on top of a base mission to alter its structure and tension. Each twist is a data record with preconditions, effects (mission-graph patches), and a cost in **twist points**; the generator draws up to `T_max` twists (Easy/Normal 1–2, Hard/Expert 2–3, Simulator 3) subject to compatibility.

| Twist | Effect | Applicable to | Incompatible with |
|---|---|---|---|
| `weather_turn` | Weather deteriorates at phase 2 (cloud base lowers, visibility drops); RTB becomes harder | Any, not in storm | `fog_bank` |
| `defector` | An enemy pilot signals intent to defect; ROE challenge, rescue/escort sub-objective | Air missions | `second_front` (too busy) |
| `decoys` | Dummy radar emitters and inflatable decoys; false SAM sites that waste weapons | Strike, SEAD, Recon | — |
| `low_fuel_start` | Player flight starts at 60 % fuel; tanker or nearby base required | Long-range missions | `tanker_damaged` |
| `awacs_lost` | AWACS destroyed or jammed early; no free picture; datalink degraded | Air superiority, intercept | `ew_storm` (double info loss) |
| `ew_storm` | Heavy jamming zone around objectives; radar burn-through limits | Strike, escort | `awacs_lost` |
| `fog_bank` | Valley/coastal fog at night/dawn limiting visual play | Low-terrain missions | `weather_turn` |
| `civilian_airliner` | A civilian airliner crosses the battlespace; ROE, identification pressure | Any with civilian corridor | — |
| `friendly_in_trouble` | A damaged friendly asks for escort/cover | Any | `defector` |
| `second_front` | Mid-mission second axis of attack on a different part of the map | Large+ battlefields | `defector` |
| `ambush_valley` | Concealed fighters/SAM in a valley on the likely ingress route | Mountain terrain | — |
| `sam_relocate` | Mobile SAMs relocate after firing; shoot-and-scoot | SEAD/DEAD | — |
| `tanker_damaged` | Tanker damaged and leaves early | Long-range missions | `low_fuel_start` |
| `runway_cratered` | Home base starts with a cratered runway (shorter strip) | Defence | `late_reinforce` |
| `late_reinforce` | Allied reinforcements arrive late or not at all | Any | `runway_cratered` |
| `comms_degraded` | Radio interference, delayed calls | Any | — |
| `boss_cameo` | A named ace appears briefly (identity hook) | Any (with boss probability) | — |

Twists are selected by seeded sampling weighted by recency penalties (§69.3) and by geography fit; each twist records its parameters in the `MissionDef` so briefings can disclose or hide it (depending on realism, e.g. `decoys` hidden in Hard).

## 69.5 Player freedom: multiple solutions (outline §138)

The same objective must support multiple approaches; there should rarely be one mandatory solution. For example, **Objective: Destroy enemy airbase** can be solved by attacking at high altitude, approaching at low altitude, using terrain masking, destroying SAMs first, attacking from another direction, coordinating with AI or with human players, or performing a direct strike.

**Design requirements:**

1. **Approach affordances.** Objective types declare the approach axes they support (`altitude_profile`, `ingress_azimuth`, `sequencing` (SEAD-first, decoy-first, direct), `cooperation` (solo, AI package, human package), `weapon_choice`). The generator places forces so that each axis has a viable option (e.g. a terrain-masked ingress corridor exists, a SAM ring has gaps or a defeat-first route, standoff weapons reach the target).
2. **No mandatory solution.** A mission is rejected if only a single approach is viable.
3. **Approach viability check (generator validation).** For each candidate approach profile (≥ 8 profiles: {high, medium, low-masked} × {2–3 azimuth sectors} + SEAD-first + direct strike + standoff), the validator evaluates a *fast survivability model*: sample the route, accumulate exposure to the enemy detection/engagement envelopes (radar LOS service, SAM ranges with terrain masking, fighter CAP intercept geometry), and estimate the success probability for a baseline pilot (Veteran vs Regular). Viable = estimated success ≥ 40 % and exposure under the survivability budget.
4. **Solution Diversity Index (SDI)** = number of *distinct viable approaches* (clustered by the axes). Requirement: `SDI ≥ 3` for primary objectives (≥ 2 for time-critical types such as Intercept); logged per mission; CI reports the distribution over 10⁴ seeds (target: median SDI ≥ 4, 5th percentile ≥ 3).
5. **Cooperation paths.** Where AI or humans are available, a cooperative approach (e.g. SEAD flight + strike flight) is offered and rewarded by lower exposure, never required.
6. **Scoring neutrality.** The score formula (§25.11) rewards outcomes (objectives, survival, efficiency, collateral avoidance), not a particular approach; no hidden "correct" route.
7. **Dynamic reactions.** The enemy responds to the chosen approach (e.g. scrambles when detected, relocates SAMs after use) so that tactics have consequences, not scripts.

## 69.6 Cinematic moments (outline §137)

Procedural missions should naturally produce cinematic events — missile trails crossing the sky; dozens of aircraft entering the battlefield; bombers passing overhead; enemy aces appearing from clouds; massive SAM barrages; carrier launches; mountain-valley engagements; large-scale dogfights — **emerging from gameplay rather than scripted sequences.** Mechanisms:

| Moment | Emergent source | System hooks |
|---|---|---|
| Missile trails crossing the sky | Long-range exchanges with visible motor smoke and contrails | §15, §24 (contrails/visibility), §60 (particles) |
| Dozens of aircraft entering the battlefield | Reinforcement waves entering at the boundary or launching from bases | §28.5 director, §48.3 scramble |
| Bombers passing overhead | Strike packages routed over/near the player's area, escort fights | §20.8 packages |
| Enemy aces appearing from clouds | Boss/ace encounters with weather-aware ingress using cloud cover | §29, §24 |
| Massive SAM barrages | Dense SAM belts with salvo doctrine triggered by raids | §21, §28 |
| Carrier launches | Carrier groups in ocean missions with deck operations | §22.6 |
| Mountain-valley engagements | Terrain-use templates with valley ambush twists | §46.10, §69.4 |
| Large-scale dogfights | Intensity director spawning merges when density and fuel allow | §28.4 |

**Measurement:** a per-mission **Cinematic Score** = Σ over events of the interest score `I(e)` (§52.10) normalised per 10 minutes; generator tuning aims for a median ≥ 6 and a 10th percentile ≥ 2 in standard missions; the Cinematic Score is logged with the DNA and reported in CI (distribution over 10⁴ seeds). Scripted cinematics are limited to optional cues (`trigger_cinematic_cue`) and never gate progress.

## 69.7 Variation axes and generator hooks (outline §136)

| Outline axis | Mechanism | Where |
|---|---|---|
| Objectives | Template families × objective kinds × hidden/reveal rules × dependencies × dynamic objective changes (§25.4) | §25–§27 |
| Terrain | Mission geography fit scoring, placement strategies per terrain class, ingress corridors, terrain-aware force positioning | §26.7, §46.10 |
| Enemy composition | Role mixes by faction/era/budget, squadron archetypes, boss archetypes | §26.8, §19–§20, §29 |
| Weather | Realistic/random with fronts, phase-based changes (`weather_turn`) | §24 |
| Timing | Time-of-day bands, mission timers, TOT constraints, phase timers | §24.5, §25.6 |
| Reinforcements | Wave count/spacing/composition/entry points, scramble vs boundary entry | §28.5 |
| Support units | Tanker, AWACS, EW, SEAD, carriers, JTAC with availability and loss twists | §20.7, §49 |
| Threats | SAM/AAA density, naval AD, EW intensity, fighter CAP, bosses | §21, §17 |
| Aircraft | Enemy and friendly types by era/faction; suggested player aircraft | §13, §37 |
| Mission phases | Phase graphs with branches and escalation patterns | §25.6, §28 |

"Procedural generation should combine multiple systems to create unique scenarios." The generator therefore samples these axes in dependency order (type → geography fit → forces → support → threats → weather/time → phases/reinforcements → twists → validate → DNA check).

## 69.8 Campaign anti-repetition

Campaign operations use the same history ring per run plus **strategic modifiers** (§40): the same airfield can be attacked under different weather/forces/twists, and the enemy adapts (§38.3) so that repeated tactics face counters. Mission DNA distance across consecutive operations in a run must average ≥ 0.4 (campaign missions are more constrained by the strategic map).

## 69.9 Tools

The **Generator Lab** (§26.12) includes DNA distribution plots, a similarity matrix for N seeds, a twist coverage report, the SDI histogram and the Cinematic Score histogram; CI publishes these for each content version and fails on threshold regressions.

## 69.10 Acceptance criteria

- `AC-69.1` DNA is deterministic: the same `MissionDef` yields the same DNA on WASM and native; `D` is symmetric, `D(A,A) = 0`, and satisfies the triangle inequality over 10⁴ random triples (property test).
- `AC-69.2` Diversity: for 200 generations at each of 20 fixed locations, mean pairwise `D ≥ 0.45` and ≤ 5 % of pairs with `D < 0.15` (extends `AC-3.4`).
- `AC-69.3` History penalty: option frequencies follow the recency formula within ±10 % relative over 10⁵ simulated selections; locked options are always honoured.
- `AC-69.4` Twists: every twist is applicable, solvable and compatible as declared in 10⁴ seeds each (no unsolvable combinations); incompatible pairs are never co-selected.
- `AC-69.5` Player freedom: `SDI ≥ 3` for primary objectives in 100 % of accepted missions (10⁴ seeds); the median SDI ≥ 4.
- `AC-69.6` Cinematic Score distribution meets the targets in §69.6 (median ≥ 6, 10th percentile ≥ 2) for standard missions on Medium/Large battlefields.
- `AC-69.7` Human review: 20 randomly chosen missions from different seeds at one location, shown as briefing cards, are judged "clearly different" in ≥ 85 % of 190 pairwise comparisons by 6 reviewers.

---

# 70. FINAL PLAYER EXPERIENCE

`Acceptance narratives` — The ideal game session (outline §148), the campaign experience (outline §149) and the multiplayer experience (outline §150) are specified here as **beat sheets**: each beat names what the player experiences, which systems must deliver it, and how it is verified. They are the end-to-end acceptance tests of the whole design; every chapter exists to make these beats real.

## 70.1 How to read the beat sheets

- **Beat** — a moment the player must be able to experience.
- **Systems** — chapters that must work together to produce it.
- **Stamp** — a typical time offset within the session (design target) used for pacing review, not a script.
- **Check** — what the verification looks for. Experience Tests **ET-1** (sandbox), **ET-2** (campaign) and **ET-3** (multiplayer) are run as scripted playtests with human testers and, where possible, as automated headless runs with scripted pilots.

## 70.2 The ideal sandbox session (outline §148)

| # | Beat (outline) | Systems | Stamp | Check |
|---|---|---|---|---|
| 1 | The player launches the game and sees **a globe** | §53.3, §9.9, §0.6.6 | 0:00–0:03 | Globe visible ≤ 3 s cached; atmosphere and lighting correct |
| 2 | They **zoom into a region** of the world | §9.2, §44 | 0:10 | Seamless zoom from orbit to regional view without loading screens |
| 3 | They **search for an airport** | §9.5, §45.7 | 0:20 | Offline FTS returns the airport in ≤ 50 ms; name/IATA/ICAO/city all work |
| 4 | They **select it** | §9.4, §10 | 0:25 | Info card with runways, type, facilities; set as start |
| 5 | They select **another airport hundreds of kilometres away** | §9.7, §50.3 | 0:40 | Distance/bearing shown; ruler tool; battlefield auto-sizes (e.g. 400 km Large/Huge) |
| 6 | The game **previews the battlefield**: terrain, mountains, cities, coastlines, airports, distance, likely combat area | §8.6, §8.8, §9.7, §44.9, §26.4 | 0:55 | Preview shows boundary, terrain relief, airports inside, estimated engagement area (the overlap of both bases' reach, weighted toward the midpoint), data readiness, validator warnings |
| 7 | They choose **Large Battlefield, Cloudy, Sunset, High Enemy Strength, Modern Aircraft** | §30.2, §24, §13.6 | 1:10 | All five settable in ≤ 3 clicks each (presets or sliders); era "Modern" = G4/G4.5 |
| 8 | They **select their aircraft** | §13, §14 | 1:25 | Hangar shows era-appropriate aircraft with stats and charts |
| 9 | They **configure their weapons** | §14.4, §15.12 | 1:50 | Loadout planner with mass/CG/range/take-off distance; presets for the mission type |
| 10 | They **launch** | §44.9, §53.12 | 2:00 | Load plan completes; mission starts with real byte progress shown |
| 11 | The player **takes off from the actual airport** | §10, §48.2, §11.13 | 3:00 | Real runway geometry and heading; correct elevation; ground handling credible |
| 12 | They **climb over the real terrain** | §44, §46, §24 | 3:30–6:00 | Recognisable landscape, cloud layers, sunset lighting; horizon dip correct |
| 13 | **AWACS reports** enemy aircraft | §20.7, §56.6 | 6:00 | Picture call with bullseye reference, varied phrasing |
| 14 | The player **detects the contacts** | §16, §55 | 6:30 | Radar tracks appear at expected ranges; RWR cues if scanned |
| 15 | **Long-range missiles are exchanged** | §15.6–§15.8, §17 | 8:00 | Visible launches, trails; DLZ; missile warnings; defensive options |
| 16 | The player **dives behind a mountain to break radar contact** | §16.3.3, §16.9, §46.10, §55 | 8:30 | Contact lost due to terrain masking/notch (LOS service), not scripting; threat arrow behaviour credible |
| 17 | They **emerge behind an enemy formation** | §19.9, §19.4 | 10:00 | Enemy AI perception delay and surprise reaction |
| 18 | **A dogfight begins** | §11, §19, §18 | 10:30 | Energy-fighting viable; guns/IR missiles; damage visible |
| 19 | **Friendly aircraft engage other enemies** | §20, §19 | concurrent | Wingmen/AI flights fight their own engagements with radio calls |
| 20 | **Enemy strike aircraft appear** | §28, §27 | 14:00 | Escalation trigger (not random spawn in view); bombers/strike with escort heading to an objective |
| 21 | **The mission objective changes** | §25.4, §28 | 14:30 | New objective announced (radio + banner + map), old one resolves or demotes |
| 22 | The player **attacks a SAM network** | §21, §17, §46, §69.5 | 18:00 | Multiple viable approaches (terrain masking, SEAD, low-level); ARM/EW options |
| 23 | **The enemy launches additional fighters from its airbase** | §48.3, §28.5 | 20:00 | Visible scramble from the enemy base (taxi/take-off or simplified variant) |
| 24 | **An elite enemy squadron enters the battlefield** | §29, §28.5 | 24:00 | Named ace flight with distinct behaviour/capability (no HP inflation) |
| 25 | The player **fights them** | §19, §29, §18 | 24:30–30:00 | Challenging, fair; same physics; learnable patterns |
| 26 | **The primary objective is destroyed** | §25.7, §47.6, §18 | 30:00 | Hit confirmation, destruction states, objective complete |
| 27 | **Fuel is running low** | §49.6, §54.8 | 31:00 | Bingo/joker calls and HUD cues; RTB decision |
| 28 | The player **returns to base** | §49.8, §50.4 | 32:00 | Route/ILS-like aids per realism; threats en route |
| 29 | They **land** | §11.13, §10.9, §48 | 36:00 | Landing credible; carrier alternative |
| 30 | **The aircraft is damaged**; the player **repairs and rearms** | §18, §48.6 | 37:00 | Damage persisted from the sortie; service times; quick-turn in sandbox |
| 31 | They **return to the world map** | §53.3, §65 | 38:00 | Debrief → map in ≤ 4 s; statistics updated |

*(Total typical session 35–45 minutes; the experience must also work compressed to 15 minutes — Medium battlefield — and extended to 90 minutes — Huge.)*

**ET-1 acceptance.** With the exact settings of beat 7 (Large, Cloudy, Sunset, High enemy strength, Modern), for 100 generated seeds across 10 regions: (a) ≥ 95 % of seeds are *playable* and contain beats 11–14 and 26–31; (b) ≥ 80 % contain the escalation beats 15–25 in some form (long-range exchange, strike package appearance, objective change, SAM objective, airbase scramble, elite squadron) when the player plays the scripted "baseline pilot"; (c) the scripted human playtest of 12 testers reaches beat 31 with ≥ 10 reporting "this felt like a real air campaign" ≥ 4/5. Beats 16 and 22 must be demonstrably *emergent* — terrain masking and approach alternatives — in logs (LOS service events, SDI ≥ 3).

## 70.3 The campaign experience (outline §149)

| # | Beat (outline) | Systems | Check |
|---|---|---|---|
| C1 | The player **begins with one airport** | §34.2, §35 | Run starts with a single owned node and a small force |
| C2 | **Nearby airports belong to an opposing faction** | §35.1, §37 | Reachable hostile neighbours (graph edges within reach, §35.4) |
| C3 | The player **selects one** | §35.6, §34.5 | Operation choice with intel-limited information (§42) |
| C4 | **A mission is generated** | §26, §34.5 | Operation → generator request (ADR-14) with campaign context (forces, weather, resources) |
| C5 | **The battle takes place over the actual geographic region** | §7–§10, §44 | Real terrain, real airports at the node |
| C6 | **The player wins**; the **airport becomes friendly** | §36, §25.7 | Capture meter completes (§36.2), ownership flips, spoils |
| C7 | **The strategic map changes**; **access to a new airbase** | §35.2, §35.4, §39.5 | New reach, supply, basing options |
| C8 | **The enemy launches a counterattack** | §35.5, §38.3, §40 | Strategic AI picks a weakened node (§48.9), operation to defend |
| C9 | The player **must defend another airport** | §27 (Base Defence), §48.3, §48.7 | Defence mission with scramble, damage state |
| C10 | **An enemy ace squadron appears** | §29, §37, §41 | Named unit with identity |
| C11 | The player **encounters them; the ace survives** | §29, §41, §43.5 | Escape/survival rules, damage carried over |
| C12 | **Several missions later, the player encounters them again** | §43.5, §29.9 | Identity continuity: same callsign/tail, adapted tactics, remembered history in dialogue |
| C13 | The player **builds a network of captured airports and pushes across the region** | §35, §39.5, §42 | Supply logistics, fog of war, intelligence, frontline movement |
| C14 | **The campaign ends when a faction achieves its strategic victory condition** | §34.2, §38.7 | Victory/defeat evaluation; run summary; Legacy Points |

**ET-2 acceptance.** Automated: 60 AI-played campaigns terminate with defined outcomes; recurring aces re-encounter probability ≥ 70 % within five operations after first survival; counterattacks occur in ≥ 90 % of runs by operation 8. Human: 10 testers play 3 hours; ≥ 8 can retell a "story" including a recurring enemy and a captured-base decision (chronicle test, §43.7).

## 70.4 The multiplayer experience (outline §150)

| # | Beat (outline) | Systems | Check |
|---|---|---|---|
| P1 | Several players **connect over LAN or VLAN** | §33.3, §32 | Invite/QR/LAN code flows; no internet needed |
| P2 | They **select a geographic region** | §9, §32.3 | Host chooses; guests see the preview and download status |
| P3 | They **choose aircraft** | §32.4, §14 | Role-appropriate aircraft per slot |
| P4 | They **configure a large battlefield** | §30, §32.3 | Host dials; read-only for guests |
| P5 | **The host starts the mission** | §32.3, §44.9 | Everyone's load plan complete; synchronised start |
| P6 | **Players launch from the same airbase** | §48, §33 | Shared runway traffic and tower coordination |
| P7 | **One flies air superiority, another strike, another electronic warfare, another escorts** | §32.4, §17.8, §20.8 | Distinct roles with meaningful tools (EW console, packages) |
| P8 | **AI aircraft fill the rest of the battlefield** | §19, §20, §33.5 | Authority-controlled AI, replicated efficiently |
| P9 | **Players coordinate through voice/radio** | §33.5 (voice), §56.12, §56.6 | Voice + AI radio blend; ping/designation |
| P10 | **The battle dynamically escalates** | §28, §29 | Phases scale with the number of players (budget per human) |
| P11 | Result: **a large cooperative air operation**, not independent fights against AI | §32, §20.8 | Package-based objectives and shared tasks; measured by cooperation index (§70.5) |

**ET-3 acceptance.** 4–8 humans + 70 AI complete a 30-minute operation with a package plan (SEAD + strike + escort + air superiority + EW); the **cooperation index** (share of objective progress that requires ≥ 2 humans' contributions, plus mutual-support events per player per 10 minutes) ≥ 0.5 in ≥ 80 % of sessions; post-session survey ≥ 4/5 on "we operated as a team".

## 70.5 Experience metrics

| Metric | Definition | Target |
|---|---|---|
| **Time-to-first-contact** | Launch → first hostile contact | 60–120 s (Quick Dogfight); 5–8 min in Large missions with transit |
| **Intensity curve** | Director intensity over time (§29.4) | Rises with phases; ≥ 2 distinct peaks and ≥ 2 quiet troughs in a 40-minute mission |
| **Moments per 10 minutes** | Cinematic Score (§69.6) | Median ≥ 6 |
| **Cooperation index** | See ET-3 | ≥ 0.5 |
| **Story retellability** | Testers' ability to narrate memorable moments | ≥ 80 % cite ≥ 2 specific moments |
| **Frustration events** | Unfair deaths reported / session | < 0.3 per hour (post-hoc replay analysis) |

## 70.6 Presentation principles for the final experience

1. **Show the world.** Camera, lighting, weather and atmosphere (§24, §46) are first-class so that "the real world is the battlefield" is felt in screenshots.
2. **Silence and scale.** Quiet transits and then sudden density; audio and music respect this (§56.8).
3. **Information as drama.** What the player knows (and doesn't) shapes tension (§42, §55).
4. **Always a next decision.** Between sorties, the map, loadout, and strategy choices keep momentum (§53, §38).
5. **Respect time.** Short loops are available (Medium battlefield in ≤ 15 minutes), long ones are optional.

## 70.7 Acceptance criteria

- `AC-70.1` ET-1 passes as defined in §70.2 (quantitative and human parts).
- `AC-70.2` ET-2 passes as defined in §70.3.
- `AC-70.3` ET-3 passes as defined in §70.4.
- `AC-70.4` Every beat in the three sheets maps to at least one automated check or logged observable (traceability table maintained in the test plan, Appendix E).
- `AC-70.5` The experience metrics of §70.5 are logged for every playtest and reported at gates G3, G5 and G6.

---

# 71. DEFINITION OF SUCCESS

`Vision` — The game should ultimately be recognisable from one sentence (outline §151), the project is successful when specific experiences are possible (outline §152), and everything exists to serve one player fantasy (outline §153). This chapter makes those statements measurable so that "done" has a definition.

## 71.1 Core identity (outline §151)

> **A realistic sandbox combat flight simulator where the entire real world becomes your battlefield.**

The player is not limited to "Map 1, Map 2, Map 3, Map 4" — the game's question is: **"Where do you want to fight?"** The answer can be almost anywhere.

*Operationalisation:* the first screen after the menu is the globe (§53.3); there is no fixed map list in the UI (NN-1); any point within ±85° latitude can be chosen (`AC-3.1`); the product copy, store text and tutorials use the same framing. A **content audit** at each release verifies that no mode requires an authored-only map.

## 71.2 Success criteria (outline §152) with measurable evidence

| Area | Statement (outline) | Measurable evidence | Tests / gates |
|---|---|---|---|
| **World** | The player can select a meaningful location anywhere on Earth | 99 % of 1 000 random points yield playable battlefields (`AC-3.1`); search finds ≥ 99 % of the top 10 000 named places in the index; every supported airport is selectable | `AC-3.1`, `AC-9.*`, `AC-10.1`; G3 |
| **Flight** | The aircraft feels physically believable and satisfying | Conformance within tolerance (`AC-3.2`); blind-test ratings ≥ 4.0/5 (G1); no fixed-rate-turn path (NN-2) | `AC-11.*`, G1 |
| **Combat** | Weapons, radar, missiles, guns, countermeasures and damage interact convincingly | Hit-rate ordering (`AC-15.5`); countermeasure effectiveness curves; sensor-range tests; damage consistency; tester survey ≥ 4/5 for "shots feel earned" | `AC-15.*`, `AC-16.*`, `AC-17.*`, `AC-18.*`, G2 |
| **AI** | Enemies and allies behave like competent pilots and military units | Same-physics audit (`AC-3.3`); monotone skill ladder (`AC-59.4`); blind identification: testers distinguish AI from human at ≤ 60 % (near chance is the goal) in 1v1 clips; wingmen follow orders correctly (`AC-20.*`) | `AC-19.*`, `AC-20.*` |
| **Missions** | The game can generate varied, dynamic, cinematic combat scenarios | 10⁴-seed validity ≥ 99.5 %; DNA diversity (`AC-3.4`); SDI ≥ 3; Cinematic Score targets (`AC-69.6`) | `AC-26.*`, `AC-69.*`, G3 |
| **Geography** | Terrain meaningfully affects combat | Terrain-masking events occur in ≥ 70 % of mountain-region missions; geography-fit generation (§46.11); testers report terrain use as decisive in ≥ 70 % of sessions in mountain/ocean/island missions | `AC-16.*`, `AC-46.7`, `AC-69.5`, G3 |
| **Scale** | Anything from a 1v1 duel to large aerial battles | Small 2 aircraft to Huge 150 aircraft within performance gates (PS-1/PS-2); governor stability | `AC-60.*`, PS-1…PS-6 |
| **Multiplayer** | Players can reliably fight together over LAN/VLAN | 16-client soak; hit registration under impairment; LAN-only operation; session completion rate ≥ 95 % in beta | `AC-33.*`, G5, ET-3 |
| **Campaign** | Airports become strategic assets in a persistent roguelite war | ET-2; soak without soft-locks; retention gate G6 | `AC-34.*`–`AC-43.*`, G6 |
| **Replayability** | The player can return to the same region and experience a substantially different battle | Paired-session test: ≥ 80 % of testers rate sessions "substantially different"; `AC-69.2` | G3, `AC-69.7` |
| **Freedom** | The player can create scenarios rather than only consuming developer-created missions | Sandbox configuration without code; user missions/battlefields shareable via links/files (`AC-51.*`); 10 testers create and share a mission in ≤ 15 minutes (≥ 8 succeed) | `AC-30.*`, `AC-51.*` |

## 71.3 Evidence programme (how success is established)

| Study | Protocol | Sample | Pass |
|---|---|---|---|
| **Flight feel** (G1) | Blind A/B/C: Kestrel vs reference sim vs arcade; fixed tasks (pattern, energy fight, landing); 7-point scales + free text | ≥ 12 players (mix of novice/expert) | Median ≥ 4.0/5 equivalent for believable & satisfying |
| **Fun gate** (G2) | First Playable script; post-session survey | ≥ 20 | ≥ 70 % "play another"; median fun ≥ 3.8/5 |
| **Replayability** (G3) | Each tester plays 3 sessions at the same location (different seeds), then rates differences, memorability and repetition | ≥ 15 testers, ≥ 45 sessions | ≥ 80 % "substantially different"; repetition complaints ≤ 10 % |
| **AI believability** | 1v1 and 2v2 clips with AI vs human pilots (blinded) | ≥ 20 reviewers × 20 clips | Identification accuracy ≤ 60 % |
| **Geography use** | Replay analysis + survey: how often terrain masking/valley tactics used | ≥ 200 missions | Terrain-masking events in ≥ 70 % of mountain missions |
| **Multiplayer** (ET-3, G5) | LAN parties, home networks, VLAN setups | ≥ 20 sessions | ≥ 95 % completion; cooperation index ≥ 0.5 |
| **Campaign retention** (G6) | 3-hour play sessions; follow-up after one week | ≥ 15 testers | ≥ 60 % continue past operation 3 voluntarily |
| **Usability** | SUS questionnaire; time-to-first-flight | ≥ 20 testers | SUS ≥ 75; first flight ≤ 10 min (`AC-4.5`) |
| **Creation** | Mission/battlefield creation task | ≥ 10 testers | ≥ 8 succeed in ≤ 15 min |

Where the numbers above are targets rather than measured facts, they are *thresholds to be met at the corresponding gate*, set before testing begins (anti-p-hacking) and recorded in the gate record (`AC-66.4`).

## 71.4 The ultimate design goal (outline §153)

The game combines six ingredients into **one coherent system**:

| Ingredient (outline) | How the design realises it | Chapters |
|---|---|---|
| **Google Earth-scale geographic freedom** | Real-world geodata on a curved-Earth frame, seamless globe-to-runway streaming, search, coordinate entry, bookmarks, user battlefields | §7–§10, §44–§47, §51 |
| **War Thunder-style aircraft and combat depth** | Physically based 6-DOF flight with assist dials, detailed sensors/weapons/damage; depth without requiring study (dials, Standard default) | §11–§18, §59 |
| **Project Wingman-style cinematic missions** | Escalation, bosses/aces, set-piece-capable emergent moments, camera and photo tools | §25–§29, §52, §69.6 |
| **Ace Combat-style spectacle** | Large battles, SAM barrages, carrier launches, adaptive music, weather and light | §22, §24, §28, §56.8, §60 |
| **Roguelite strategic progression** | Persistent graph war, resources, losses, pilots, Legacy Points | §34–§43 |
| **LAN/VLAN cooperative warfare** | Serverless browser multiplayer, roles, packages, voice | §32–§33, §56.12 |
| **A genuinely persistent geographic world** | Airport ownership, damage, frontlines, chronicle; persistent save system | §35, §48, §43, §64 |

(The titles named by the outline are references to *qualities*, not to be copied; all aircraft, factions and weapons here are original, ADR-11.)

## 71.5 The player fantasy and the feature test

> **"I can look at Earth, pick a place, pick an aircraft, and start a war there."**

Everything else in the game exists to make that fantasy **more convincing, more interactive, more replayable, and more fun.** This is operationalised as the **feature test** applied to every proposal, change request and cut decision:

1. Does it make *picking a place* more meaningful or easier? (World)
2. Does it make *the place itself* matter in combat? (Geography)
3. Does it make *flying and fighting* more convincing? (Flight, combat, AI)
4. Does it make *starting a war* more interactive — more ways to shape and fight it? (Sandbox, missions, campaign)
5. Does it make *coming back* more rewarding — varied, persistent, shareable? (Replayability, persistence, freedom)
6. If none, it is a candidate for cut (§68.2).

## 71.6 Release criteria (version 1.0)

Version 1.0 = M6 complete. Go/no-go checklist:

| Area | Criterion |
|---|---|
| Experience | ET-1, ET-2, ET-3 pass |
| Success areas | All eleven rows of §71.2 meet their thresholds |
| Quality | No open P0/P1 bugs; crash-free sessions ≥ 99.5 % (beta telemetry, opt-in); performance gates on the three profiles |
| Content | Roster ≥ 12 aircraft, 26 weapons + 6 stores, 4 factions, ≥ 20 pre-packaged regions + worldwide streaming, 16 mission types, ≥ 30 airbases hero-polished |
| Platform | Compat matrix (Chrome/Edge primary; Firefox/Safari status published), offline verified, PWA install verified, CSP/COOP-COEP verified |
| Legal | Data licence audit green (AC-45.4); fictional content IP review; music/audio licences; privacy statement |
| Accessibility | G7 complete; external review done |
| Documentation | Player manual, controls cheat sheets, modding/UGC docs, support runbook |

## 71.7 Post-launch indicators (opt-in telemetry)

D1/D7/D30 retention; sessions per week; median session length (target 25–45 min); mission completion rate (target 70–85 % on Normal); campaign run completion rate; share of players using non-default regions (target > 50 % outside the starter regions); user mission creation rate; multiplayer session success rate; crash-free rate; performance profile distribution. Indicators inform tuning, not design goals themselves.

## 71.8 Acceptance criteria

- `AC-71.1` The Definition-of-Success checklist (§71.2) has an owner, evidence link and pass/fail status for each row at every gate review; the 1.0 release requires all green.
- `AC-71.2` The product's first interactive screen asks "Where do you want to fight?" (UI string present) and shows the globe/map, with no fixed map selection list (UI audit).
- `AC-71.3` Each study in §71.3 is executed with the stated protocol at its gate and the results archived with raw data (anonymised) in the project records.
- `AC-71.4` The feature test (§71.5) is attached to the proposal template; 100 % of accepted features record their answers; features failing all five are not scheduled.

---

# APPENDIX A: DATA SCHEMAS

This appendix is the normative index of data formats. Schemas are written in a TypeScript-like notation for readability; the machine-readable JSON Schema (Draft 2020-12) files are **generated from the Rust definitions** (`schemars`) so that code and schema cannot drift (§62.6). Conventions apply to all schemas:

- `schema: "<type>/<major>"` is mandatory; minor additions are backward compatible; a major bump requires a migration (§64.5).
- **SI units only**, with unit suffixes in keys: `_kg`, `_m`, `_mps`, `_n`, `_pa`, `_s`, `_deg` (degrees in data files), `_hz`, `_m2`, `_kgm2`. Fractions are `0..1` unless the key says `_pct`.
- Ids are namespaced strings `namespace:type/name` (§62.3); `Ref<T>` denotes an id of type `T`.
- `Table1D = [x, y][]` (monotone `x`), `Table2D = { x: number[], y: number[], z: number[][] }` (row-major `z[y][x]`), multilinear interpolation, clamped at the ends.
- `*_key` fields are localisation keys (§53.10). Optional fields have defaults documented in the schema.
- `generated` blocks are written by the baker and must not be hand-edited (inputs hash enforced).

## A.1 Schema index

| Schema | Defined in | Notes |
|---|---|---|
| `aircraft/1` | A.2, §12, §62.2 | Includes hardpoints (§12.4) |
| `fcs/1` | A.2 (`fcs.gains_ref`), ADR-34 | Control-law gains (v0.1b); `content/core/fcs/` |
| `engine/1` | A.3 | Thrust/fuel decks |
| `weapon/1` | §15.2, A.4 | Class blocks in A.4 |
| `sensor/1` (radar, irst, rwr, esm, pod) | A.5, §16 | |
| `ew_system/1` (jammer, decoy, CM dispenser) | A.5, §17 | |
| `ground_unit/1`, `sam_system/1` | §21.2, A.6 | |
| `ship/1` | §22.2, A.6 | Carriers add `deck` block |
| `faction/1` | §37.2 | |
| `squadron/1` | A.7, §20.2 | |
| `airport/1`, `airport_override/1` | §10.3, A.7 | |
| `battlefield/1` | A.8, §8 | |
| `mission/1` | §25.2 | |
| `session_config/1` | §6.3 | Sandbox/lobby configuration |
| `loadout/1` | A.9, §14.8 | |
| `target_def/1` | A.10, §47.6 | |
| `hud/1`, `cockpit_layout/1` | §54.13, A.11 | |
| `audio_profile/1` | A.12, §56.3 | |
| `difficulty_preset/1`, `sandbox_preset/1` | §59.3, §30.3 | |
| `controls/1` | §58.6 | |
| `pack/1`, `tunables/1` | §62.4, A.13 | |
| `replay/1` (header + chunks) | A.14, §52.2 | |
| `mission_result/1`, `campaign_run/1`, `saves_manifest/1` | §65.4, §64.7 | |
| `locale/1`, `datapack/1`, `gpk` | A.15, §45.5 | |

## A.2 `aircraft/1`

```ts
interface Aircraft {
  schema: "aircraft/1"; id: Ref<Aircraft>; name_key: string; extends?: Ref<Aircraft>;
  category: "light_fighter"|"multirole_fighter"|"heavy_fighter"|"interceptor"|"strike_fighter"|"cas"|"bomber"|"ew"|"awacs"|"carrier"|"stealth"|"experimental"|"tanker"|"transport"|"uav";
  era: "G3"|"G4"|"G4.5"|"G5"|"EXP"; crew: number; tags: string[];            // e.g. ["carrier","stol"]
  mass: { empty_kg: number; internal_fuel_kg: number; max_takeoff_kg: number; max_landing_kg?: number;
          cg_empty_m: [number, number, number]; inertia_kgm2: [number, number, number]; inertia_xz_kgm2?: number;
          tanks: { id: string; capacity_kg: number; pos_m: [number, number, number]; feed_priority: number }[]; fuel_density_kgpl: number /* 0.80 */ };
  geometry: { wing_area_m2: number; span_m: number; mac_m: number; ref_point_m: [number, number, number];
              gear: { id: string; pos_m: [number, number, number]; stroke_m: number; k_npm: number; c_nspm: number; steer_deg?: number; brake?: boolean }[];
              bbox_m: [number, number, number] };
  engines: { ref: Ref<Engine>; count: number; position_m: [number, number, number]; thrust_vector?: { max_deg: number; rate_dps: number; axes: "pitch"|"pitch_yaw" } }[];
  aero: { tables_ref: Ref<AeroSet>; max_mach: number; service_ceiling_m: number;
          n_max: number; n_min: number; n_max_by_config?: Record<string, number>; vmo_mps: number; mmo: number; gear_limit_mps: number; flap_limit_mps: number;
          config_deltas: { gear_cd0: number; flap_to_cd0: number; flap_ld_cd0: number; brake_cd0: number; flap_cl: [number, number] }; clmax_table: Table1D /* by Mach */ };
  fcs: { sas_required: boolean; gains_ref: Ref<FcsGains>; limiters: { alpha_limit_deg: number; n_cmd_max: number; roll_rate_max_dps: number };
         actuators: { surface: string; rate_dps: number; limit_deg: [number, number]; bandwidth_hz: number }[] };
  hardpoints: Hardpoint[];                                   // §12.4
  internal_weapons: { gun?: { ref: Ref<Weapon>; rounds: number }; bays?: { id: string; slots: number; door_s: number }[] };
  sensors: { radar?: Ref<Sensor>; irst?: Ref<Sensor>; rwr?: Ref<Sensor>; maws?: Ref<Sensor>; pod_slots?: string[] };
  countermeasures: { chaff: number; flares: number; programs: Ref<CmProgram>[]; towed_decoy?: boolean };
  signatures: { rcs_ref: string /* rcs.bin */; ir: { engine_dry: number; engine_ab: number; skin_per_mach: number }; acoustic_ref?: Ref<AudioProfile> };
  damage: { hit_volumes_ref: string; components: DamageComponent[]; fire_rules_ref?: string };      // §18
  cockpit?: Ref<CockpitLayout>; hud?: Ref<HudLayout>; audio?: Ref<AudioProfile>;
  ai?: { preferred_speed_mps: [number, number]; ps_tables_ref?: string; doctrine_tags: string[] };
  envelope: PerformanceEnvelope;                              // §11.19, Appendix C.9
  loadouts: Ref<Loadout>[]; default_loadout: Ref<Loadout>;
  model: { mesh: string; lods: number; liveries: Ref<Livery>[]; animation_nodes?: Record<string, string> };
  generated?: { fcs_gains_hash: string; inputs_hash: string; ps_tables_hash?: string };
}
interface Hardpoint { id: string; station: number; pos_body_m: [number, number, number]; dir_body: [number, number, number]; mount: string[]; max_mass_kg: number; sym?: string;
                      drag_when_empty_cd0?: number; rcs_when_empty_m2?: number; bay?: string; interference?: string[]; clearance?: { gear?: boolean; flap?: boolean };
                      launch?: { eject_mps: number; offset_m: [number, number, number]; cant_deg?: number } }
interface DamageComponent { id: string; kind: "wing"|"fuselage"|"tail"|"engine"|"canopy"|"radar"|"hydraulics"|"avionics"|"fuel_tank"|"gear"|"control_surface"|"weapon_station";
                            hp: number; armour_class: "none"|"light"|"medium"|"heavy"; volume: string; fire_prob?: number; leak_kgps?: number; effects: string[] }
interface PerformanceEnvelope { points: { id: string; mass_kg: number; alt_m: number; mach?: number /* required for n_sust, turn_dps */;
                                          config: "clean"|"clean_mil"|"clean_ab"|"ref_ab"|"to_flaps_ab"|"ldg_brakes";
                                          metric: "max_mach"|"stall_mps"|"roc_mps"|"n_sust"|"turn_dps"|"ff_kgps"|"roll_m"; value: number; tol_pct: number;
                                          requires?: "gear" /* deferred until that model exists (§11.19) */ }[] }
```

*Clarified in 0.2.1 (ADR-30): what v0.1a implements.* The generated `content/schemas/aircraft.schema.json` is
normative for the fields that exist; the interface above remains the target. Differences:

- Present now: identity, `mass` (with `tanks`), `geometry` without `gear`, `engines`, `aero`, `fcs`,
  `hardpoints` (empty) and `envelope`. The rest arrives with the systems that read it.
- `aero.config_deltas` and `aero.clmax_table` live in the aero tables (`aero/1`: its `config` block and
  `lift.cl_max`), next to the coefficients they modify. `aero` gains `static_margin_mac`, checked
  by the baker against the tables.
- `fcs.gains_ref` is absent until the FCS (v0.1b). `fcs.actuators` is keyed by surface (`stab`,
  `aileron`, `rudder`, `lef`, `tef`, `airbrake`), each with `range_deg`, `rate_dps` and a first-order
  `tau_s` instead of `bandwidth_hz`; the airbrake uses `rate_per_s` over 0 to 1.

*Clarified in 0.2.2 (ADR-34, ADR-35): what v0.1b adds to `aircraft/1`.*

- `geometry.gear` is present: per leg `id`, `pos_m` (the contact point at full extension, BODY), `stroke_m`, `k_npm`, **`c_comp_nspm` and `c_ext_nspm`** in place of the single `c_nspm` (damping is asymmetric, §11.13.1), `steer_max_deg` (0 for a fixed leg), `brake` and `main` (a main leg counts for weight on wheels). `geometry.contacts` lists the airframe contact points (`id`, `pos_m`: nose tip, tail, wing tips, belly). The baker checks the nose leg's share of the load, static compression, strut bounce, tail clearance at the touchdown attitude, tip-back, the contacts inside `bbox_m`, at least one braked leg and at most one steerable leg (ADR-35).
- `aero.n_max_by_config` is present (`gear` and `flaps`, the limit with either out).
- `fcs.gains_ref` is present and names an `fcs/1` entry (A.1); `fcs.actuators.gear` carries `transit_s` (the gear moves at that rate, with no lag).
- Envelope points with `requires: "gear"` are no longer deferred: `to_roll` and `ldg_roll` are flown (C.8). A model without a braked main leg still defers them.

## A.3 `engine/1`

```ts
interface Engine {
  schema: "engine/1"; id: Ref<Engine>; name_key: string; kind: "turbofan"|"turbojet"|"turboprop";
  thrust: { sl_dry_n: number; sl_ab_n?: number; density_exp: number /* 0.75 */; mach_factor_dry: Table1D; mach_factor_ab?: Table1D;
            alt_lapse?: Table1D /* optional override of density_exp */ };
  tsfc: { dry_kg_per_ns: number; ab_kg_per_ns?: number; idle_fraction: number; cruise_mult?: Table1D };      // kg/(N·s); table mult by Mach
  dynamics: { spool_up_s: number; spool_down_s: number; ab_lightoff_s: number; ab_cutoff_s: number; start_time_s: number; idle_rpm_pct: number };
  limits: { egt_max_k: number; overheat_time_s?: number; compressor_stall_alpha_deg?: number; compressor_stall_mach_min?: number; windmill_relight_mps: number };
  inlet?: { type: "fixed"|"variable"|"dsi"|"scoop"; recovery: Table1D /* pressure recovery by Mach */ };
  mass_kg: number; length_m: number; diameter_m: number;
  audio: { rpm_idle: number; rpm_max: number; blade_count: number; harmonics: number[] }
}
```

*Clarified in 0.2.1 (ADR-30): what v0.1a implements.* `kind` is `turbofan`. `idle_fraction` moved to
`thrust` (it scales thrust, not fuel flow). `dynamics` gains `ab_stage_s` (the stage time
constant of §11.8.2) and `ab_stages`; `start_time_s`, `idle_rpm_pct`, `egt_max_k`, `inlet.recovery`,
the masses and dimensions, and `audio` arrive with the systems that read them. The inlet's
angle-of-attack loss (§11.8.1) is data: `inlet.alpha_start_deg` and `inlet.alpha_span_deg`.

## A.4 `weapon/1` class blocks

Common header: §15.2. Class-specific blocks (all optional unless the class requires them):

```ts
interface WeaponClassBlocks {
  gun?:   { caliber_mm: number; muzzle_mps: number; rpm: number; spinup_s: number; dispersion_mrad: number; round_mass_kg: number; drag_cd: number;
            rounds: number; ammo_types: { id: string; hit_dmg: number; pen_mm: number; tracer_every: number }[]; heat?: { per_round: number; cool_s: number } };
  rocket?: { motor: Motor; dispersion_mrad: number; warhead: Warhead; salvo: number[] };
  missile?: { motor: Motor[]; aero: MissileAero; autopilot: { tau_s: number; g_max: number; g_min_speed_mps: number };
              guidance: { law: "pn"|"apn"|"pursuit"|"lead"; n_prime: number; midcourse?: "inertial"|"datalink_inertial"|"gps"; loft?: Table1D };
              seeker: Seeker; fuze: Fuze; warhead: Warhead; launch: LaunchConstraints };
  bomb?:  { aero: { cd0: number; s_ref_m2: number; fins?: boolean }; guidance?: { kind: "laser"|"gps_ins"|"imaging"; steer_authority_deg: number; glide_ratio: number };
            fuze: Fuze; warhead: Warhead; release: { min_alt_m: number; min_speed_mps: number; max_g: number } };
  cluster?: { submunitions: number; dispersal_radius_m: number; sub: Warhead };
  pod?:     { kind: "ecm"|"tgp"|"recon"|"jammer"; sensor_ref?: Ref<Sensor>; power_w?: number };
  tank?:    { capacity_kg: number; jettison_cd0: number; drag_cd0: number };
}
interface Motor   { thrust_n: Table1D; burn_s: number; propellant_kg: number; mdot_curve: "linear"|"thrust_prop" }
interface Seeker  { type: "ir_rear"|"ir_all"|"ir_imaging"|"sarh"|"arh"|"laser_sal"|"eo"|"arm"|"inertial"|"gps";
                    activate_range_m?: number; fov_deg: number; gimbal_deg: number; lock_ref_range_m: number; ref_rcs_m2?: number; noise_deg: number; eccm: number; cage_time_s?: number }
interface Fuze    { type: "impact"|"proximity"|"delay"|"altitude"; arm_dist_m: number; trigger_radius_m?: number; cone_deg?: number; delay_s?: number }
interface Warhead { kind: "frag"|"blast"|"shaped"|"ap"|"incendiary"|"cluster"; he_kg: number; lethal_radius_m: number; frag_cone_deg?: number; pen_mm?: number }
interface LaunchConstraints { min_speed_mps: number; max_g: number; max_alpha_deg: number; eject_mps: number; safe_sep_s: number; min_alt_m?: number }
```

## A.5 `sensor/1` and `ew_system/1`

```ts
interface Sensor {
  schema: "sensor/1"; id: Ref<Sensor>; kind: "radar"|"irst"|"rwr"|"maws"|"esm"|"tgp"|"radar_alt"|"iff";
  radar?: { band: "S"|"C"|"X"|"Ku"; r_ref_m: number; sigma_ref_m2: number; az_deg: number; el_deg: number; bars: number[]; scan_hz: number;
            modes: ("rws"|"tws"|"stt"|"stt_air"|"air_to_ground"|"sea"|"ground_map"|"bvr_boresight"|"wvr_acm")[]; max_tracks: number; stt_tracks: number;
            doppler: { pd: boolean; v_notch_mps: number; look_down: boolean; mainlobe_clutter_mps?: number };
            lpi?: boolean; eccm: number; power_w_rel: number; sidelobe_db: number; range_scales_m: number[] };
  irst?: { band: "mwir"|"lwir"; r_ref_m: number; ir_ref: number; fov_deg: [number, number]; scan_hz: number; cooled: boolean };
  rwr?:  { bands: string[]; detect_margin_db: number; id_db: number; tiers: "direction"|"class"|"id"; lpi_penalty_db: number; update_hz: number; library_ref: string };
  esm?:  { freq_range_ghz: [number, number]; sensitivity_db: number; df_accuracy_deg: number; geolocation: boolean };
  maws?: { type: "uv"|"ir"|"radar"; fov_deg: number; range_m: number; false_alarm_rate_hz: number };
  tgp?:  { fov_deg: number[]; zoom: number; laser: boolean; range_m: number; gimbal_deg: [number, number] };
  mass_kg: number; power_kw: number; cooling?: "air"|"liquid";
}
interface EwSystem {
  schema: "ew_system/1"; id: Ref<EwSystem>; kind: "jammer_pod"|"jammer_internal"|"standoff_jammer"|"decoy_towed"|"decoy_expendable"|"cm_dispenser";
  jammer?: { erp_rel: number; bandwidth_ratio: number; coverage_deg: number; modes: ("noise"|"rgpo"|"vgpo"|"ghost"|"cross_eye")[]; r_bt_ref_m: number; duty_max: number; cooling: "poor"|"good"; programs: string[] };
  decoy?:  { rcs_m2: number; ir_signature: number; lifetime_s: number; trail_m?: number };
  dispenser?: { chaff: number; flares: number; programs: CmProgram[]; salvo_interval_s: number };
}
interface CmProgram { id: string; chaff_per_burst: number; flares_per_burst: number; bursts: number; interval_s: number }
```

## A.6 Ground, SAM and ship extensions

```ts
interface SamSystem { schema: "sam_system/1"; id: Ref<SamSystem>; tier: "manpad"|"point"|"short"|"medium"|"long";
  radars: { search: Ref<Sensor>; track: Ref<Sensor>; illuminate?: Ref<Sensor> };
  launchers: { count: number; missiles_per: number; reload_s: number; missile: Ref<Weapon>; ready_s: number };
  envelope: { r_min_m: number; r_max_m: number; h_min_m: number; h_max_m: number; ph_vs_rcs?: Table1D };
  doctrine: { emcon: "always"|"intermittent"|"cued"; shoot_and_scoot: boolean; relocate_s?: number; salvo: number };
  c2: { links: ("autonomous"|"battery"|"sector")[]; cue_delay_s: number }; mobility: Mobility; vulnerability: { hp: number; armour_class: string; radar_hp: number } }
interface Ship { schema: "ship/1"; id: Ref<Ship>; class: "carrier_catobar"|"carrier_stobar"|"cruiser"|"destroyer"|"frigate"|"patrol"|"transport"|"submarine"|"tanker";
  hull: { length_m: number; beam_m: number; draft_m: number; displacement_t: number; max_speed_mps: number; accel_mps2: number; tactical_diameter_lengths: number };
  sections: { id: string; hp: number; compartments: number; flood_rate_kgps: number }[];
  sensors: Ref<Sensor>[]; weapons: { mount: string; ref: Ref<Weapon>; arc_deg: [number, number]; ammo: number }[]; ciws?: { layout: string; rounds: number };
  deck?: { length_m: number; width_m: number; angle_deg: number; catapults: number; wires: number[]; ski_jump_deg?: number; elevators: number; hangar_slots: number; air_wing_max: number; sea_state_limit: number };
  damage_control: { rate: number }; signatures: { rcs_m2: number; ir: number; acoustic_db: number } }
```

## A.7 `squadron/1`, `airport_override/1`

```ts
interface Squadron { schema: "squadron/1"; id: Ref<Squadron>; faction: Ref<Faction>; callsign_set: Ref<CallsignSet>; name_key: string; role: string;
  aircraft: { type: Ref<Aircraft>; count: number; livery: Ref<Livery> }[]; skill_dist: [string, number][]; personality: { aggression: number; caution: number; discipline: number };
  doctrine: Ref<Doctrine>; base?: Ref<Airport>; emblem: { grammar: string; seed: number }; elite?: boolean; pilots?: { name_seed: number; skill: string; traits: string[] }[] }
interface AirportOverride { schema: "airport_override/1"; id: Ref<Airport>; set?: Partial<AirportRecord>; runways?: Runway[]; remove_runways?: string[]; add_stands?: Stand[]; notes?: string }
```

## A.8 `battlefield/1`

```ts
interface Battlefield {
  schema: "battlefield/1"; name?: string;
  anchor: { lat_deg: number; lon_deg: number };                                     // §8.7
  shape: { kind: "rect"; width_m: number; height_m: number; rotation_deg: number } | { kind: "ellipse"; a_m: number; b_m: number; rotation_deg: number } | { kind: "polygon"; verts_deg: [number, number][] };
  altitude_m: { floor: number; ceiling: number };                                     // MSL, §8.4
  boundary: { warn_m: number; penalty_m: number; kill_m?: number; policy: "return"|"extract"|"destroy" };
  spawns: { side: "blue"|"red"|"neutral"; kind: "airport"|"carrier"|"air"|"boundary"; ref?: string; pos?: [number, number, number]; hdg_deg?: number }[];
  airports: { ref: Ref<Airport>; role: string; side?: string }[]; improvised?: { id: string; center_deg: [number, number]; hdg_deg: number; length_m: number }[];
  zones?: { id: string; kind: "restricted"|"killbox"|"safe"|"objective"; poly_deg: [number, number][]; floor_m?: number; ceiling_m?: number }[];
  environment?: { weather?: string; utc?: string; wind?: { dir_deg: number; speed_mps: number } }; objectives?: string[];
  bookmarks?: string[]; datapack?: string; terrain_character?: { relief_m: number; masking: number; openness_km: number }      // cached, §46.11
}
```

## A.9 `loadout/1`

```ts
interface Loadout { schema: "loadout/1"; id: Ref<Loadout>; aircraft: Ref<Aircraft>; name_key?: string; name?: string;
  stations: { id: string; store: Ref<Weapon>|Ref<Store>|null; count?: number; sym_locked?: boolean }[];
  internal: { bay?: Ref<Weapon>[]; gun_rounds?: number; chaff?: number; flares?: number };
  fuel_pct: number; livery?: Ref<Livery>; tags?: string[]; computed?: { mass_kg: number; cg_pct_mac: number; drag_index: number; range_km: number; takeoff_roll_m: number } }
```

## A.10 `target_def/1`

```ts
interface TargetDef { schema: "target_def/1"; id: Ref<TargetDef>; category: "bridge"|"depot"|"radar_site"|"airfield"|"power"|"port"|"rail_yard"|"bunker"|"hq"|"ship"|"vehicle"|"sam"|"building";
  footprint_m: [number, number]; hit_boxes: { id: string; shape: "box"|"cyl"; pos_m: [number, number, number]; size_m: [number, number, number]; hp: number; armour_class: string }[];
  states: { id: "intact"|"damaged"|"destroyed"; hp_threshold_pct: number; model: string; effects: string[] }[];
  valid_ordnance: ("blast"|"penetration"|"incendiary"|"shaped"|"gun")[]; protected?: boolean; strategic_value: number; collateral_radius_m?: number; secondary_explosion?: { fuel_kg: number; radius_m: number } }
```

## A.11 `cockpit_layout/1`

```ts
interface CockpitLayout { schema: "cockpit_layout/1"; id: Ref<CockpitLayout>; tier_min: "A"|"B"|"C"; eye_point_m: [number, number, number]; head_limits_deg: { yaw: number; pitch_up: number; pitch_down: number };
  hud_glass: { node: string; tfov_deg: [number, number]; eye_box_mm: [number, number, number] };
  mfds: { id: string; node: string; softkeys: number; default_page: string; pages: string[]; resolution: [number, number] }[];
  instruments: { kind: "airspeed"|"altimeter"|"attitude"|"compass"|"fuel"|"rpm"|"gear_lights"|"warning_panel"; node: string; channel: string }[];
  switches?: { id: string; node: string; action: string }[]; mirrors?: { node: string; fov_deg: number }[]; canopy_node: string }
```

## A.12 `audio_profile/1`

```ts
interface AudioProfile { schema: "audio_profile/1"; id: Ref<AudioProfile>;
  engine: { blade_count: number; rpm_idle: number; rpm_mil: number; harmonics: number[]; whine_hz_per_rpm: number; jet_level_db: number; burner_roar_db: number; formants: { f_hz: number; q: number; gain_db: number }[]; samples: { lightoff?: string; spoolup?: string; spooldown?: string; flameout?: string; stall?: string } };
  aero: { wind_gain: number; buffet_gain: number; transonic_rumble: number; gear_samples: Record<string, string> };
  cockpit: { canopy_lowpass_hz: number; interior_gain_db: number; switch_samples: Record<string, string> };
  warnings: Record<string, string>; level_offset_db: number }
```

## A.13 `tunables/1`

```ts
interface Tunables { schema: "tunables/1"; values: Record<string, { value: number|string|boolean|number[]; unit?: string; min?: number; max?: number; default: number|string|boolean|number[]; doc: string; owner: string }> }
// keys are dot paths, e.g. "ai.pilot.pursuit_gain", "weapons.dlz.margin_pct", "net.interp_delay_ms". Read only through `Registry::tunable("key")` (§62.8).
```

## A.14 `replay/1`

```ts
interface ReplayHeader { schema: "replay/1"; kind: "ssr"|"osr"; game_version: string; content_version: string; datapack?: string; mission_hash: string; provenance?: Provenance;
  duration_s: number; tick_rate_hz: number; snapshot_rate_hz: number; keyframe_interval_s: number; players: { slot: number; name: string; side: string; aircraft: string }[];
  entity_schema_version: number; thumbnail?: string; index_offset: number; event_track_offset: number; marker_track_offset: number }
// Chunk = zstd(keyframe | delta*) ; Event = { t: f32, type: u8, a: u32, b: u32, pos?: i32[3], data?: bytes }
```

## A.15 `locale/1`, `datapack/1`

```ts
interface Locale { schema: "locale/1"; lang: string /* BCP-47 */; fallback?: string; strings: Record<string, string> /* ICU MessageFormat */; plural_rules?: string }
interface DatapackManifest { schema: "datapack/1"; latest: string; versions: { id: string; released: string; compatible_game: string; archive_url: string; size_bytes: number; sha256: string; licenses_ref: string }[] }
```

---

# APPENDIX B: FORMULA & ALGORITHM REFERENCE

A consolidated reference of the formulas used across the document, with symbols, the section where each is applied, and the verification that covers it. All formulas use SI units unless stated. Where a formula appears in a chapter, the chapter text governs; this appendix is a convenience index and MUST be updated with it (Appendix I).

## B.1 Geodesy and frames (§7)

| Item | Formula |
|---|---|
| WGS84 | `a = 6 378 137 m`, `f = 1/298.257223563`, `e² = f(2 − f) = 0.00669437999014`, `b = a(1 − f)` |
| Prime-vertical radius | `N(φ) = a / √(1 − e² sin²φ)` |
| Geodetic → ECEF | `X = (N + h) cosφ cosλ`, `Y = (N + h) cosφ sinλ`, `Z = (N(1 − e²) + h) sinφ` |
| Check vector | φ = 36.2°, λ = −121.8°, h = 1 500 m → ECEF ≈ (−2 716 000.612, −4 380 460.552, 3 747 008.625) m |
| ECEF → geodetic | Bowring/Zhu closed-form seed + one Newton refinement: `p = √(X²+Y²)`, `θ = atan2(Z a, p b)`, `φ = atan2(Z + e'² b sin³θ, p − e² a cos³θ)`, `h = p/cosφ − N` (error < 1 µm after one refinement) |
| ENU basis at (φ,λ) | `E = (−sinλ, cosλ, 0)`, `N = (−sinφ cosλ, −sinφ sinλ, cosφ)`, `U = (cosφ cosλ, cosφ sinλ, sinφ)` |
| BF | `p_BF = Rᵀ (p_ECEF − p_anchor)` with `R = [E N U]` at the anchor (§7.4) |
| Sphere drop (flat-tangent error) | `Δ ≈ d²/(2R)` → d = 10 km: 7.8 m; 50 km: 196 m; 100 km: 785 m; 300 km: 7.06 km; 500 km: 19.6 km (R = 6 371 km) |
| Geometric horizon | `d = √(2Rh + h²) ≈ 3.57 √h` km (h in m); radar/radio horizon with k = 4/3: `≈ 4.12 √h` km |
| Great-circle distance (spherical) | `d = 2R asin √( sin²(Δφ/2) + cosφ₁ cosφ₂ sin²(Δλ/2) )` |
| Geodesic (ellipsoid) | Karney inverse/direct (algorithm of record); Vincenty only for tests, with its known antipodal limits |

## B.2 Atmosphere and air data (§11.3, §11.5)

| Item | Formula |
|---|---|
| ISA troposphere (h ≤ 11 km) | `T = 288.15 − 0.0065 h`, `p = 101 325 (T/288.15)^5.25588` |
| ISA lower stratosphere (11–20 km) | `T = 216.65`, `p = 22 632.06 · exp(−g₀ (h − 11 000)/(287.05287 · 216.65))` |
| Density, sound speed | `ρ = p/(R T)`, `a = √(γ R T)`, `R = 287.05287`, `γ = 1.4` |
| Dynamic pressure, Mach | `q̄ = ½ ρ V²`, `M = V/a` |
| Impact pressure (subsonic) | `q_c = p[(1 + 0.2 M²)^3.5 − 1]` |
| Calibrated airspeed | `V_CAS = a₀ √( 5[(q_c/p₀ + 1)^(2/7) − 1] )` (subsonic; supersonic uses the Rayleigh pitot relation) |
| Equivalent airspeed | `V_EAS = V_TAS √(ρ/ρ₀)` |
| Density altitude | ISA altitude with the same ρ: `h_D = (T₀/L)[1 − (ρ/ρ₀)^(1/4.25588)]` for ρ ≥ ρ(11 km) |
| ISA reference values | h = 0: 288.15 K, 101 325 Pa, 1.2250 kg/m³, 340.29 m/s · 5 km: 255.65 K, 54 019 Pa, 0.7361 kg/m³ · 11 km: 216.65 K, 22 632 Pa, 0.3639 kg/m³, 295.07 m/s |

## B.3 Flight dynamics (§11)

| Item | Formula |
|---|---|
| Translation | `m dV/dt = F_aero + F_thrust + F_gravity + F_gear`, `F_gravity = m g(h) ĝ`, `g(h) = g₀ (R/(R + h))²` |
| Attitude kinematics | `q̇ = ½ q ⊗ (0, ω_body)` (renormalise each step) |
| Rotation dynamics | `I ω̇ + ω × (I ω) = M` (I includes the `I_xz` product term) |
| RK4 | `k₁ = f(x), k₂ = f(x + ½Δt k₁), k₃ = f(x + ½Δt k₂), k₄ = f(x + Δt k₃), x⁺ = x + Δt/6 (k₁ + 2k₂ + 2k₃ + k₄)`, Δt = 1/120 s (T0) |
| Aerodynamic forces and moments | `L = q̄ S C_L`, `D = q̄ S C_D`, `Y = q̄ S C_Y`; `M_x = q̄ S b C_l`, `M_y = q̄ S c̄ C_m`, `M_z = q̄ S b C_n` |
| Drag polar (reference) | `C_D = C_D0 + ΔC_D,wave(M) + K C_L²` |
| Load factor | `n = L/W` (aerodynamic); `n_z` = specific force along −z_body in g |
| Turn rate / radius | `ω = g √(n² − 1) / V`, `r = V² / (g √(n² − 1))` |
| Bank for a level turn | `cos φ = 1/n` |
| Stall speed | `V_s = √( 2 W / (ρ S C_Lmax) )`; at load factor `n`: `V_s,n = V_s √n` |
| Specific excess power | `P_s = (T − D) V / W`; energy height `E_s = h + V²/(2g)` |
| Thrust model (game) | `T = T_SL · (ρ/ρ₀)^0.75 · f(M)` (`f` from the engine's Mach table) |
| Corner speed | `½ ρ V² S C_Lmax / W = n_max` → `V_c = √(2 n_max W/(ρ S C_Lmax))` |
| Ground effect | `φ_IGE = (16 h/b)² / (1 + (16 h/b)²)`; `C_Di,IGE = φ_IGE · K C_L²` |
| Sustained n (solve) | find `n` such that `T = q̄ S (C_D0 + C_Dw + K (n W/(q̄ S))²)` |
| Breguet (jet) | `R = (V/(g · TSFC)) (L/D) ln(W_i/W_f)` with TSFC in kg/(N·s); endurance `E = (1/(g · TSFC))(L/D) ln(W_i/W_f)` |
| G-physiology | `dG_f/dt = (n_z − G_f)/2.0 s`; `G_tol = 4.5 + 1.5 + AGSM − 0.4 F`; `dS/dt = 0.25 (G_f − G_tol)/G_tol` for `G_f > G_tol` (§11.14) |

## B.4 Sensors and detection (§16)

| Item | Formula |
|---|---|
| Radar equation | `P_r = P_t G² λ² σ / ((4π)³ R⁴)` ⇒ `R_det ∝ σ^{1/4}`; game scaling `R(σ) = R_ref (σ/σ_ref)^{1/4}` (R_ref = 120 km at σ_ref = 5 m²: σ = 0.1 → 45.1 km; 1 → 80.2 km; 5 → 120 km; 20 → 169.7 km; 100 → 253.8 km) |
| Detection probability | `P_d(ρ)` logistic/Swerling-style map of `ρ = (R_det/R)⁴` (§16.3.2) |
| Doppler notch | Target rejected when `\|v_t · û_los\| < v_notch`; window half-angle `θ_n = asin(v_notch / V_t)` (V_t = 250 m/s: 60 m/s → 13.9°; 120 m/s → 28.7°) |
| LOS with Earth bulge | Ray-march with `h_ray(s) = h_A + (h_B − h_A) s/S − s(S − s)/(2 k R)`; blocked if terrain ≥ `h_ray`; `k = 1.0` (visual/IR), `4/3` (radio/radar) |
| Mutual radar horizon, two aircraft at 100 m AGL | `2 × 4.12 √100 km = 82.4 km` |
| Visual detection (Koschmieder) | `C(R) = C₀ e^{−σ_ext R}`, `σ_ext = 3.912 / V_vis`; detected if `\|C(R)\| ≥ C_th (≈ 0.02)` and angular size `≥ θ_min` |
| Jammer burn-through (self-screening) | `R_bt = √( P_t G_t σ B_j / ( 4π P_j G_j B_r (S/J)_min ) )` (§17.3.1) |
| Bearing-only triangulation | Two bearings from `P₁, P₂` with unit vectors `û₁, û₂`: `P = P₁ + t₁ û₁`, `t₁ = ((P₂ − P₁) × û₂)/(û₁ × û₂)` (2-D cross product) |

## B.5 Weapons (§15)

| Item | Formula |
|---|---|
| Proportional navigation | `a_c = N′ V_c λ̇`, N′ = 3–5; augmented: `a_c = N′ V_c λ̇ + (N′/2) a_t⊥` |
| Zero-effort miss (planar) | `ZEM = r_rel + v_rel t_go (+ ½ a_t t_go²)` perpendicular to the LOS; `a_c = N′ ZEM_⊥ / t_go²` |
| Missile kinematics | `m dV/dt = T − D − m g sinγ`; `D = q̄ S_ref C_D`; lateral acceleration limited by `min(g_max, q̄ S C_N,max/m)` |
| Gun time of flight | Table `TOF(R)` from ballistic integration with drag `a_d = −½ ρ v \|v\| C_d A/m`; lead solution fixed point `t = TOF(\|p_t(t) − r_o\|)` (§54.4) |
| CCIP | Integrate the released store's ballistic trajectory at 10 ms steps to terrain intersection (sim terrain) |
| Fragment lethality (game) | `dmg(r) = W_dmg · exp(−(r/R₀)²)`, `R₀ = k_target · W_HE^{1/3}` (k_target 3.0 soft, 1.8 armour, 4.5 large buildings) |
| Lead-collision intercept | Solve `\|r + v_t t\| = v_own t`: `t = [−(r·v_t) ± √((r·v_t)² + (v_own² − \|v_t\|²)\|r\|²)] / (v_own² − \|v_t\|²)`, smallest positive root; `ψ_int` = bearing of `r + v_t t` |

## B.6 Navigation and guidance (§50)

| Item | Formula |
|---|---|
| Wind triangle | `sin(drift)/V_w = sin(ψ_w − track)/TAS`; `GS = TAS cos(drift) + V_w cos(ψ_w − track)` |
| Cross-track guidance | `ψ_cmd = bearing_to_wp + atan(K_xt · XTE / V)`, limited to ±45° |
| Turn anticipation (fly-by) | `d_turn = V² / (g tan φ_max)` |
| INS drift model | `pos_err(t) = b t + w(t)`, `b ~ N(0, 0.2² m²/s²)` per axis; median ≈ 0.5 nm per hour |
| Magnetic declination | WMM spherical harmonics (degree 12) on a 1° grid with bicubic interpolation |

## B.7 Terrain, streaming, rendering (§44–§47, §60)

| Item | Formula |
|---|---|
| Tile geometry | `Δ = 180°/2^z`; `lon_w = −180° + xΔ`; `lat_n = 90° − yΔ`; tiles at level z = `2^(2z+1)` |
| Post spacing at the equator | `s_z = Δ × 111 319.49 m / 128` (z8: 611.5 m; z12: 38.2 m; z13: 19.1 m) |
| LOD selection | `ρ = ε_z W / (2 d tan(fov_h/2))`; refine if `ρ > τ` |
| Height decode | `h = h_min + q (h_max − h_min)/65 535` |
| Relative-to-eye | `offset = tile_origin − camera` in f64, cast to f32 |
| Reverse-Z infinite projection | `P = [[f/aspect,0,0,0],[0,f,0,0],[0,0,0,n],[0,0,−1,0]]`, `f = 1/tan(fov_v/2)`, depth `= n/z_view` |
| Azimuthal equidistant (map) | `c = acos(sinφ₀ sinφ + cosφ₀ cosφ cos Δλ)`, `k′ = c/sin c`, `x = R k′ cosφ sin Δλ`, `y = R k′ (cosφ₀ sinφ − sinφ₀ cosφ cos Δλ)` |
| Camera lens | `fov_v = 2 atan(12 mm / f)` (24 mm sensor height) |
| Critically damped spring | `v⁺ = v + (ω²(x_t − x) − 2ω v) dt`, `x⁺ = x + v⁺ dt`, `ω = 2/τ` (use an implicit form for large dt) |

## B.8 Audio (§56)

| Item | Formula |
|---|---|
| Speed of sound | `c = 20.05 √T` (T in K) |
| Spreading and absorption | `L = L₀ − 20 log₁₀(d) − α(f) d/1000`, α from the ISO 9613-2 table at 20 °C / 70 % RH [verify] |
| Delay | `τ = d/c` |
| Doppler | `f′ = f c/(c − v_r)` (v_r positive when approaching) |
| Mach cone | `μ = asin(1/M)` |
| Loudness | LUFS per ITU-R BS.1770 (K-weighting, gating) |

## B.9 Networking (§33, §60.11)

| Item | Formula |
|---|---|
| Snapshot interpolation | render time `t_r = t_now − d_interp` (default 100 ms); `x(t_r) = Hermite(x_a, v_a, x_b, v_b, u)`, `u = (t_r − t_a)/(t_b − t_a)` |
| Prediction error | `e = \|x_pred − x_auth\|`; smoothing `x ← x + (x_auth − x)(1 − e^{−dt/τ_c})` for `e < e_snap`, else snap |
| Quantisation | position 1 cm in a local 2 km cell / 0.1 m far; velocity 0.1 m/s; quaternion "smallest three" (10 bits each + 2-bit index) |
| Bandwidth estimate (reference) | 8 humans × 26 B × 20 Hz + 150 AI × 26 B × 5 Hz + 40 missiles × 20 B × 20 Hz ≈ 39 660 B/s ≈ 318 kbit/s before delta/relevance savings |
| Jitter (RFC 3550) | `J ← J + (\|D\| − J)/16` |
| Lag compensation | Rewind target hit volumes to `t_shot − d_interp − RTT/2` (clamped to 250 ms) |

## B.10 Procedural generation (§26, §69)

| Item | Algorithm |
|---|---|
| SplitMix64 | `z = (x += 0x9E3779B97F4A7C15); z = (z ^ (z >> 30)) × 0xBF58476D1CE4E5B9; z = (z ^ (z >> 27)) × 0x94D049BB133111EB; return z ^ (z >> 31)` |
| xoshiro256** | Standard; seeded from `SplitMix64(master ^ fnv1a64(stream_name))` |
| Crockford Base32 seed | 60 bits → 12 characters of `0123456789ABCDEFGHJKMNPQRSTVWXYZ`, grouped in fours |
| Poisson-disc | Bridson's algorithm with deterministic seeding |
| DNA distance | `D = Σ w_k d_k` (§69.2) |
| History weight | `m = max(0.05, Π (1 − 0.5·0.85^(j−1)))` (§69.3) |
| Realism Index | `RI = 100 Σ w_i d_i / Σ w_i`, multiplier `0.6 + 0.8 RI/100` (§59.4) |

## B.11 Control and filtering (§58, §54)

| Item | Formula |
|---|---|
| Deadzone and rescale | `x₁ = sgn(x₀) max(0, \|x₀\| − d_c)/(1 − d_c − d_o)` |
| Expo | `x₂ = sgn(x₁)((1 − k)\|x₁\| + k\|x₁\|³)` |
| Exponential smoothing | `y ← y + (x − y)(1 − e^{−Δt/τ})` |
| One-Euro filter | `α = 1/(1 + τ/Δt)`, `τ = 1/(2π f_c)`, `f_c = f_c,min + β \|dx̂\|` |
| HUD collimation | `u = tan(az)/tan(TFOV_h/2)`, `v = tan(el)/tan(TFOV_v/2)` |

## B.12 Verification map

Each formula family has a test: geodesy `AC-7.*`, atmosphere `AC-11.*`, flight `AC-3.2` / §11.19, radar `AC-16.*`, weapons `AC-15.*`, navigation `AC-50.*`, streaming `AC-44.*`, map `AC-55.1`, audio `AC-56.1`, network `AC-33.*` / `AC-60.8`, generation `AC-26.*` / `AC-69.*`, control `AC-58.2`.

---

# APPENDIX C: REFERENCE AIRCRAFT DATA SHEET (KESTREL)

The Kestrel is the reference light fighter (G4) and the first aircraft built (M1). Its point-performance numbers below were produced by a **reference calculator** (C.8) that implements the formulas of Appendix B with the stated parameters; every "computed" value can be reproduced by running the listing. Items marked *target* are authored starting points for the autotuner and are **not** validated by the calculator. The 6-DOF model must reproduce the computed point performance within the tolerances of §11.19 (±3 % / ±5 %).

## C.1 Identity and geometry

| Item | Value |
|---|---|
| Name / category / era | Kestrel — light fighter — G4 |
| Crew | 1 |
| Engines | 1 × F129 (afterburning turbofan) |
| Wing area S | 28.0 m² |
| Span b | 9.8 m (aspect ratio 3.43) |
| Mean aerodynamic chord c̄ | 3.2 m |
| Length / height | 14.6 m / 4.5 m (illustrative) |
| Undercarriage | tricycle; main track 2.9 m; wheelbase 5.2 m (illustrative) |
| Hardpoints | 9 (2 wingtip rails, 2 outboard, 2 inboard, 1 centreline, 2 fuselage semi-recessed) |
| Internal gun | 27 mm revolver (GS-27, 150 rounds) |

## C.2 Mass properties

| Item | Value |
|---|---|
| Empty mass | 8 400 kg |
| Internal fuel | 3 100 kg (fuselage fwd 900, fuselage aft 1 100, wing tanks 2 × 550; JP-8 0.80 kg/L → 3 875 L) |
| Reference weight (empty + 60 % fuel + 2 AAMs + gun) | 10 500 kg (8 400 + 1 860 + 240) |
| Clean combat weight used for envelope points | 9 500 kg (empty + ≈ 1 100 kg fuel) |
| Max take-off weight | 14 500 kg (illustrative) |
| Max landing weight | 9 500 kg (illustrative) |
| Inertia at 10 000 kg (Ixx, Iyy, Izz) | (13 000, 76 000, 86 000) kg·m², Ixz = 1 300 kg·m² |
| CG range | 24–36 % MAC (empty 27 % MAC) |
| Wing loading at 10 500 kg | 375 kg/m² |
| Thrust-to-weight | 1.243 at 10 500 kg (128 kN AB); 1.374 at 9 500 kg |

## C.3 Propulsion: F129 engine

| Parameter | Value |
|---|---|
| Sea-level static thrust | 79 kN military (dry), 128 kN afterburner |
| Thrust vs density and Mach | `T = T_SL (ρ/ρ₀)^0.75 f(M)` |
| Mach factor `f(M)` (dry and AB share the table) | M 0: 1.00 · 0.5: 0.98 · 0.9: 1.02 · 1.2: 1.12 · 1.6: 1.20 · 2.0: 1.08 · 2.4: 0.85 |
| TSFC dry (SL static) | 0.70 lb/(lbf·h) = 1.983×10⁻⁵ kg/(N·s) |
| TSFC AB (SL static) | 1.90 lb/(lbf·h) = 5.382×10⁻⁵ kg/(N·s) |
| Fuel flow, SL static | military 1.566 kg/s; afterburner 6.889 kg/s |
| Endurance at SL static, full internal fuel (3 100 kg) | military 1 979 s (33 min); afterburner 450 s (7.5 min) |
| Cruise TSFC (M 0.85, 11 km) | 0.85 lb/(lbf·h) |
| Dynamics (*target*) | idle → military spool 4.0 s; military → AB light-off 1.5 s; AB cut-off 0.8 s; start 40 s |

*Clarified in 0.2.1:* the two spool figures are times to near full thrust, not time constants. The
§11.8.2 model with τ_up = 1.8 s reaches 90 % of military 4.1 s after selection; with the 0.6 s
light-off delay and 0.5 s stage time constant, the afterburner reaches 90 % of its thrust 1.75 s
after selection. The engine data uses §11.8.2's form (`content/core/engines/README.md`). No steady
point of C.7 depends on these figures.

## C.4 Aerodynamics (reference polar)

| Parameter | Value |
|---|---|
| `C_D0` (clean) | 0.0235 |
| Induced-drag factor `K` | 0.135 |
| `C_Lα` (subsonic) | 3.6 /rad |
| Wave-drag increment `ΔC_D,wave(M)` | 0: 0 · 0.75: 0 · 0.85: 0.0030 · 0.95: 0.0100 · 1.05: 0.0260 · 1.20: 0.0250 · 1.50: 0.0190 · 2.00: 0.0130 · 2.40: 0.0120 |
| `C_Lmax` clean vs Mach | 0–0.6: 1.60 · 0.9: 1.30 · 1.2: 1.00 · 2.0: 0.70 · 2.5: 0.60 |
| `C_Lmax` with take-off / landing flaps | 1.90 / 1.90 (*target*) |
| Configuration `ΔC_D0` | gear down +0.015 (*target*) · take-off flaps +0.020 · landing flaps +0.030 · airbrake +0.020 |
| Static margin | +6 % MAC (mildly stable airframe; `sas_required: false`) |

## C.5 Stability and control derivatives (*targets*, per rad)

| Derivative | Value | Derivative | Value |
|---|---|---|---|
| `C_mα` | −0.216 (= −C_Lα × SM) | `C_Yβ` | −0.90 |
| `C_mq` | −9.0 | `C_lβ` | −0.10 |
| `C_mα̇` | −3.0 | `C_nβ` | +0.14 |
| `C_mδe` | −0.65 | `C_lp` | −0.30 |
| `C_lδa` | +0.16 | `C_nr` | −0.30 |
| `C_nδr` | −0.07 | `C_Yδr` | +0.20 |

Short-period characteristics from the 2-state approximation (computed from `C_mα`, `C_mq`, `C_mα̇`, `C_Lα`, `I_yy = 76 000 kg·m²`, mass 10 000 kg):

| Condition | q̄ (Pa) | ωₙ (rad/s) | Frequency (Hz) | ζ |
|---|---|---|---|---|
| SL, M 0.5 | 17 732 | 2.52 | 0.40 | 0.68 |
| SL, M 0.9 | 57 451 | 4.54 | 0.72 | 0.68 |
| 3 km, M 0.8 | 31 409 | 3.23 | 0.51 | 0.60 |
| 6 km, M 0.9 | 26 752 | 2.88 | 0.46 | 0.53 |
| 9 km, M 1.2 | 30 988 | 3.02 | 0.48 | 0.46 |

The FCS in Standard Assist augments damping and response; Manual relies on these raw values.

## C.6 Limits

| Limit | Value |
|---|---|
| Structural `n_max` / `n_min` | +9.0 g / −3.0 g (clean; reduced by heavy stores, §14.4) |
| Vmo / Mmo | 700 kn IAS (360 m/s) / M 2.0 |
| Gear / flap limit speeds | 250 kn / 300 kn (*targets*) |
| Service ceiling | 15 500 m (*target*) |
| Max AoA (FCS limit) | 26° (Standard) |
| Max roll rate (stability axis, clean, M 0.6) | 270 °/s (*target*) |

## C.7 Computed point performance (reference calculator)

**Maximum level Mach, clean 9 500 kg, afterburner**

| Altitude (m) | M_max | TAS (m/s) | TAS (kn) |
|---|---|---|---|
| 0 | 1.22 | 415.2 | 807 |
| 3 000 | 1.38 | 453.4 | 881 |
| 6 000 | 1.57 | 496.8 | 966 |
| 9 000 | 1.72 | 522.5 | 1 016 |
| 11 000 | 1.83 | 540.0 | 1 050 |
| 13 000 | 1.90 | 560.6 | 1 090 |
| 15 000 | 1.96 | 578.3 | 1 124 |

Dry (military power), clean 9 500 kg: M_max 1.00 at SL, 1.07 at 6 km, 1.29 at 11 km.

**Climb.** Maximum rate of climb at SL, 9 500 kg, afterburner: **283.1 m/s (55 700 ft/min) at Mach 0.83**.

*Clarified in 0.2.1 (ADR-31):* the rate of climb is the maximum over Mach of the 1 g specific excess
power `P_s = (T − D)·V/W`, as C.8 computes it. The Mach of the maximum is informative only: P_s is
within 3 % of its maximum from M 0.74 to 0.88, so the best-climb Mach is not a gated metric.

**1 g stall speeds (SL, ISA)**

| Configuration | Mass (kg) | `C_Lmax` | V_s (m/s) | V_s (kn) |
|---|---|---|---|---|
| Clean | 9 500 | 1.60 | 58.3 | 113.3 |
| Clean | 10 500 | 1.60 | 61.3 | 119.1 |
| Take-off flaps | 10 500 | 1.90 | 56.2 | 109.3 |
| Landing flaps | 8 500 | 1.90 | 50.6 | 98.3 |

**Sustained load factor and turn rate, 10 500 kg (clean + 2 AAM + 60 % fuel), afterburner, limited to 9 g and `C_Lmax`**

| Altitude (m) | Mach | TAS (m/s) | n sustained | Rate (°/s) |
|---|---|---|---|---|
| 0 | 0.5 | 170 | 6.28 | 20.5 |
| 0 | 0.7 | 238 | 8.45 | 19.8 |
| 0 | 0.9 | 306 | 9.00 (thrust allows 9.62; capped at 9 g) | 16.4 |
| 0 | 1.1 | 374 | 5.93 | 8.8 |
| 3 000 | 0.7 | 230 | 6.38 | 15.4 |
| 3 000 | 0.9 | 296 | 7.43 | 14.0 |
| 6 000 | 0.7 | 221 | 4.70 | 11.7 |
| 6 000 | 0.9 | 285 | 5.58 | 10.8 |
| 6 000 | 1.1 | 348 | 5.03 | 8.0 |

Level flight at SL, M 1.3 is not sustainable in this configuration.

**Corner speed (9 g, `C_Lmax`), 10 500 kg**

| Altitude (m) | Mach | TAS (m/s) | TAS (kn) | Instantaneous rate at 9 g (°/s) |
|---|---|---|---|---|
| 0 | 0.55 | 187 | 364 | 26.9 |
| 3 000 | 0.67 | 220 | 428 | 22.8 |
| 6 000 | 0.87 | 275 | 535 | 18.3 |

**Specific excess power P_s (m/s), 10 500 kg, 1 g, afterburner**

| Altitude (m) | M 0.4 | M 0.6 | M 0.8 | M 0.9 | M 1.0 | M 1.2 | M 1.5 |
|---|---|---|---|---|---|---|---|
| 0 | 151 | 214 | 255 | 242 | 171 | 16 | −194 |
| 3 000 | 114 | 166 | 204 | 200 | 159 | 69 | −53 |
| 6 000 | 81 | 125 | 157 | 160 | 138 | 90 | 27 |
| 9 000 | 52 | 88 | 116 | 121 | 112 | 91 | 64 |

**V–n points at SL, 10 000 kg (lift-limited speed at load factor n):** n = 1: 59.8 m/s (116 kn) · 3: 103.5 m/s (201 kn) · 5: 133.7 m/s (260 kn) · 7: 158.2 m/s (307 kn) · 9: 179.3 m/s (349 kn).

**Take-off ground roll (SL ISA, dry asphalt, take-off flaps, `V_LOF = 1.2 V_s`)**

| Mass (kg) | V_s (kn) | V_LOF (kn) | Roll, AB (m) | Roll, military (m) |
|---|---|---|---|---|
| 9 500 | 104 | 125 | 159 | 263 |
| 10 500 | 109 | 131 | 195 | 323 |
| 12 500 | 119 | 143 | 278 | 464 |
| 14 500 | 128 | 154 | 377 | 632 |

Density sensitivity (10 500 kg, AB): SL ISA+20 → 220 m; 1 500 m elevation → 253 m (ISA+20: 286 m); 2 500 m elevation → 302 m. The 15 m obstacle distance at 10 500 kg (AB) is about 440 m (estimate).

**Landing ground roll (SL ISA, dry asphalt, brakes μ_b = 0.55, airbrake, touchdown at 1.15 V_s)**

| Mass (kg) | Approach 1.3 V_s (kn) | Touchdown (kn) | Roll (m) |
|---|---|---|---|
| 7 500 | 120 | 106 | 281 |
| 8 500 | 128 | 113 | 322 |
| 9 500 | 135 | 120 | 362 |

Wet runway (μ_b = 0.40), 8 500 kg: 420 m.

**Cruise and range (10 000 kg, M 0.85, 11 000 m):** TAS 250.8 m/s, `C_L` 0.306, `C_D` 0.0391, L/D 7.82, drag 12.5 kN, fuel flow 0.30 kg/s (1 087 kg/h). Breguet cruise range with 60 % of the fuel burned in cruise: 1 362 km with 3 100 kg internal; 2 111 km with 5 500 kg (internal plus two 1 400 L tanks). The 520 km combat radius in §49.6 is a design target consistent with these figures after climb, combat allowance and reserves.

## C.8 Reference calculator (reproduces C.7)

```python
import math, bisect
g0 = 9.80665
def isa(h):                                     # returns T, p, rho, a
    if h <= 11000: T = 288.15 - 0.0065*h; p = 101325*(T/288.15)**5.255876
    else:          T = 216.65; p = 22632.06*math.exp(-g0*(h-11000)/(287.05287*216.65))
    rho = p/(287.05287*T); return T, p, rho, math.sqrt(1.4*287.05287*T)
def interp(tab, x):
    xs = [t[0] for t in tab]
    if x <= xs[0]: return tab[0][1]
    if x >= xs[-1]: return tab[-1][1]
    i = bisect.bisect_right(xs, x) - 1; (x0, y0), (x1, y1) = tab[i], tab[i+1]
    return y0 + (y1-y0)*(x-x0)/(x1-x0)
S, CD0, K = 28.0, 0.0235, 0.135
WAVE  = [(0,0),(0.75,0),(0.85,0.0030),(0.95,0.0100),(1.05,0.0260),(1.20,0.0250),(1.50,0.0190),(2.00,0.0130),(2.40,0.0120)]
MFAC  = [(0,1.0),(0.5,0.98),(0.9,1.02),(1.2,1.12),(1.6,1.20),(2.0,1.08),(2.4,0.85)]
CLMAX = [(0,1.60),(0.6,1.60),(0.9,1.30),(1.2,1.00),(2.0,0.70),(2.5,0.60)]
def thrust(M, h, ab=True):  return (128e3 if ab else 79e3) * (isa(h)[2]/1.225)**0.75 * interp(MFAC, M)
def drag(M, h, W, n=1.0):
    T, p, rho, a = isa(h); q = 0.5*rho*(M*a)**2; CL = n*W/(q*S)
    return q*S*(CD0 + interp(WAVE, M) + K*CL*CL), CL, q
def max_level_mach(h, mass, ab=True):          # largest M with T >= D and CL <= CLmax(M)
    W = mass*g0; best = 0
    for i in range(30, 260):
        M = i/100; D, CL, q = drag(M, h, W)
        if thrust(M, h, ab) >= D and CL <= interp(CLMAX, M): best = M
    return best
def sustained_n(M, h, mass, ab=True):          # bisection on D(n) = T
    W = mass*g0; T = thrust(M, h, ab); lo, hi = 0.0, 12.0
    if T < drag(M, h, W, 1.0)[0]: return None
    for _ in range(60):
        mid = (lo+hi)/2
        if drag(M, h, W, mid)[0] > T: hi = mid
        else: lo = mid
    return lo
# max_level_mach(11000, 9500) -> 1.83 ; sustained_n(0.9, 0, 10500) -> 9.62 (then capped at 9 g and CLmax)
```

The take-off and landing integrations use the same polar with ground `C_L` of 0.55 (take-off) / 0.45 (landing), rolling resistance μ_r = 0.02, thrust `T_SL (ρ/ρ₀)^0.75`, and the configuration drag of C.4, integrated with a 0.01 s Euler step to `V_LOF` (take-off) or to 2 m/s (landing).

*Clarified in 0.2.1 (ADR-31):*

- **Altitude datum.** `isa(h)` above treats `h` as geopotential altitude, while the game's ISA
  takes geometric height h_MSL and converts it (§11.3), and envelope `alt_m` is geometric like every
  altitude in the game. The difference is not compensated: it moves M_max at 11 km by −0.05 % and
  n_sust at 6 km by +0.25 %, both well inside the tolerances.
- **Mach grid.** `max_level_mach` scans in steps of 0.01 and keeps the last Mach that passes, so it
  floors. The true crossings are M 1.2288 (SL, AB), 1.8336 (11 km) and 1.0005 (SL, dry). The
  conformance test measures a continuous Mach, so it reads about +0.7 % against `vmax_sl` by
  construction.

*Clarified in 0.2.2 (ADR-32):*

- **The ground rolls are initialised ground-roll segments, as the paragraph above defines them.** `to_roll` runs from brake release at steady maximum afterburner, on the gear at its static attitude (about 3°) with the stick neutral, to the ground speed `V_LOF` = 1.2 `V_s,TO`, with no rotation (a 6-DOF aircraft whose mains are 0.62 m behind the CG cannot rotate below about 64 m/s, so a run that must lift off needs about 245 m). `ldg_roll` runs from `V_TD` = 1.15 `V_s,LDG` on all wheels at the static attitude, idle, airbrake out, landing flaps, full brakes with anti-skid from t = 0, to a ground speed under 2 m/s. The world is paved, dry, at sea level. Lift-off and the flare are covered by the virtual tester's circuit, not by the envelope.
- **The landing target stays 322 m, and C.7's figure sits 3.1 % below what C.8's own text produces.** With C.8's rolling resistance of 0.02 added to a braking μ of 0.55 on the lift-relieved load and no thrust, the integration gives **332 m**. The Kestrel's F129 idles at 4.7 kN, which C.8 does not model, and that would add about 13 %; the model meets 322 m with a **braked nose wheel** (`brake: true` on every leg; with the nose wheel unbraked the roll grows by 42 %), the §10.4 dry-asphalt μ_b of 0.70 under the 0.9 anti-skid factor, and the idle fraction as authored. Measured: `to_roll` 195.72 m (+0.37 %) and `ldg_roll` 322.23 m (+0.07 %). The target and tolerance are not changed.
- The wet-runway landing of C.7 (420 m) is not gated: the same landing at wetness 1 (μ_b 0.40) rolls 569.8 m (informational).

## C.9 Authored envelope block (conformance test input)

```json
{ "envelope": { "points": [
  { "id": "vmax_sl",      "mass_kg": 9500,  "alt_m": 0,     "config": "clean_ab",   "metric": "max_mach",  "value": 1.22,  "tol_pct": 3 },
  { "id": "vmax_11k",     "mass_kg": 9500,  "alt_m": 11000, "config": "clean_ab",   "metric": "max_mach",  "value": 1.83,  "tol_pct": 3 },
  { "id": "vmax_dry_sl",  "mass_kg": 9500,  "alt_m": 0,     "config": "clean_mil",  "metric": "max_mach",  "value": 1.00,  "tol_pct": 3 },
  { "id": "roc_sl",       "mass_kg": 9500,  "alt_m": 0,     "config": "clean_ab",   "metric": "roc_mps",   "value": 283.1, "tol_pct": 3 },
  { "id": "stall_clean",  "mass_kg": 9500,  "alt_m": 0,     "config": "clean",      "metric": "stall_mps", "value": 58.3,  "tol_pct": 3 },
  { "id": "nsust_sl_m07", "mass_kg": 10500, "alt_m": 0,     "mach": 0.7, "config": "ref_ab", "metric": "n_sust",   "value": 8.45,  "tol_pct": 5 },
  { "id": "turn_sl_m05",  "mass_kg": 10500, "alt_m": 0,     "mach": 0.5, "config": "ref_ab", "metric": "turn_dps", "value": 20.5,  "tol_pct": 5 },
  { "id": "nsust_6k_m09", "mass_kg": 10500, "alt_m": 6000,  "mach": 0.9, "config": "ref_ab", "metric": "n_sust",   "value": 5.58,  "tol_pct": 5 },
  { "id": "ff_mil_sl",    "mass_kg": 9500,  "alt_m": 0,     "config": "clean_mil",  "metric": "ff_kgps",   "value": 1.566, "tol_pct": 3 },
  { "id": "ff_ab_sl",     "mass_kg": 9500,  "alt_m": 0,     "config": "clean_ab",   "metric": "ff_kgps",   "value": 6.889, "tol_pct": 3 },
  { "id": "to_roll",      "mass_kg": 10500, "alt_m": 0,     "config": "to_flaps_ab","metric": "roll_m",    "value": 195,   "tol_pct": 5, "requires": "gear" },
  { "id": "ldg_roll",     "mass_kg": 8500,  "alt_m": 0,     "config": "ldg_brakes", "metric": "roll_m",    "value": 322,   "tol_pct": 5, "requires": "gear" }
] } }
```

The 6-DOF model has trim, control and unsteady effects the point-mass calculator lacks. The authored values are the targets; differences larger than the tolerance are resolved by retuning the data, not by changing the targets, unless the target itself is shown to be wrong.

*Clarified in 0.2.1 (ADR-30, ADR-31):* `mach` is required on `n_sust` and `turn_dps` points (the id's
`m07` is a label, the field is the data). `requires` names a model the point needs; until it exists
the point is reported as deferred. `config` is one of `clean`, `clean_mil`, `clean_ab`, `ref_ab`
(clean aerodynamics at the reference mass, afterburner; C.8 adds no store drag), `to_flaps_ab` and
`ldg_brakes`.

## C.10 Hardpoints and stations

| Station | Position | Mounts | Max mass (kg) | Symmetric partner |
|---|---|---|---|---|
| S1 / S9 | wingtip L/R | rail (WVR) | 120 | S9 / S1 |
| S2 / S8 | outboard pylon L/R | pylon, rail (WVR/MRM) | 700 | S8 / S2 |
| S3 / S7 | inboard pylon L/R | pylon, tank, rail (MRM) | 1 500 | S7 / S3 |
| S4 / S6 | fuselage semi-recessed L/R | rail (MRM) | 250 | S6 / S4 |
| S5 | centreline | pylon, tank, bomb | 1 800 | — |

Interference: a ≥ 900 kg store on S5 limits S4/S6 to ≤ 250 kg stores and bans SARH missiles there. Station numbering is illustrative; the data file is authoritative (general rules in §14.4).

## C.11 Sensors, signatures, countermeasures

| Item | Value |
|---|---|
| Radar | Pulse-Doppler, G4; `r_ref` 120 km vs `σ_ref` 5 m²; ±60° azimuth, ±60° elevation; bars 1/2/4; RWS / TWS (10 tracks) / STT; look-down; `v_notch` 60 m/s; ECCM 0.30 |
| RWR | G4 with threat ID; 360° azimuth; update 10 Hz |
| MAWS | None (optional pod) |
| Targeting pod | Optional pod slot (S5) |
| RCS (clean, *target*) | front 3.0 m², side 12.0 m², rear 6.0 m²; +1.5 m² per pylon carrying a weapon |
| IR signature (*target*) | engine dry 0.5, AB 1.0 (relative); skin heating ∝ M² |
| Countermeasures | 120 chaff + 60 flares; programs: single, salvo-4, auto |

## C.12 Damage components (*targets*)

Wing L/R (HP 100 each, with fuel tanks inside), fuselage fwd/mid/aft (80/120/80), engine (HP 70, fire probability 0.35 on hit), vertical tail (60), horizontal stabilisers L/R (50), canopy/cockpit (40), radar (40), hydraulic systems A/B (30 each), avionics bay (40), fuel tanks (4), gear (3), control surfaces (6). Damage effects: §18.

## C.13 Cockpit, HUD, audio

Tier B default: one wide HUD (28° × 20°), two 6" MFDs (radar / stores / map / engine / CM / nav / RWR / warnings pages), analogue backup (airspeed, altitude, attitude, compass), eye point at the design eye, head limits ±135° yaw. Audio profile `aud:kestrel`: single turbofan, blade count 24, idle 8 000 rpm, afterburner roar +6 dB (§56.3).

---

# APPENDIX D: LAUNCH CONTENT ROSTERS

All names are fictional working names pending the IP/naming review (ADR-11, gate G8). All numbers are design targets (§0.2) unless Appendix C says otherwise. This appendix indexes the launch content; the authoritative data are the content files (§62).

## D.1 Launch content counts

| Content | Count at 1.0 | Source |
|---|---|---|
| Combat aircraft (crewed) | 12 | §13.4 |
| Support aircraft | 3 (Stratus tanker, Hauler transport, Scout UAV) | D.2 |
| Weapons | 26 | §15.12 |
| Non-weapon store types | 6 | §15.12 |
| Ground unit classes | 11 (≈ 30 unit types) | §21.2, D.3 |
| Ship classes | 8 (≈ 10 ship types) | §22.2, D.4 |
| Factions | 4 | §37.3 |
| Mission types | 16 | §27 |
| Pre-packaged regions | ≥ 20 (worldwide streaming for all) | §66.3 |
| Hero-polished airbases | ≥ 30 | §71.6 |
| Twists | 17 | §69.4 |
| Training missions | 8 (+ carrier ops post-launch) | §53.11 |

## D.2 Aircraft rosters

**Combat aircraft.** The 12-aircraft table (Kestrel, Peregrine, Condor, Swift, Bittern, Badger, Albatross, Magpie, Argus, Gannet, Wraith, Chimera) with empty mass, fuel, thrust, reference weight, T/W, wing area, wing loading, maximum Mach, `n_max` and hardpoints is in §13.4. The Kestrel is fully specified in Appendix C. Roles in multiplayer: §13.5. Era assignments: §13.6.

**Support aircraft (design targets):**

| Aircraft | Role | Empty (kg) | Fuel (kg) | Engines | Key figures | Notes |
|---|---|---|---|---|---|---|
| **Stratus** | Tanker | 55 000 | 90 000 | 4 × 70 kN | Offload ≈ 40 000 kg at 1 000 km; cruise M 0.80 at 9–10 km; refuelling 250–280 kn; 2 wing drogues (20 kg/s each) + centreline boom (45 kg/s) | Racetrack AI (§49.5); unarmed; flies away from threats within 100 km |
| **Hauler** | Transport | 36 000 | 18 000 | 4 turboprops, 3 MW each | Payload 25 t; range 4 500 km at 25 t; STOL (take-off roll 900 m at 70 t); cruise M 0.55 | Campaign logistics and civilian-world traffic (§23, §39.5) |
| **Scout** | UAV (recon, optional light strike) | 1 100 | 700 | 1 turboprop, 450 kW | Endurance 18 h; cruise 55–130 m/s; ceiling 12 000 m; EO/IR turret + SAR; optional 2 × Talon-3 | Reconnaissance role (§13.5, §42.2); remote-piloted by a human or autonomous |

**Faction rosters (air).** See §37.3. Summary: Azure League (Peregrine, Gannet, Wraith limited, Argus, Magpie, Stratus), Meridian Pact (Swift, Condor, Bittern, Albatross, Kestrel), Concord of Free Ports (Kestrel, Badger, Bittern, lend-lease mix), Iron Marches (Badger, Bittern, Albatross, Kestrel, G3).

## D.3 Ground units (fictional; design targets)

| Id | Class | Notes (max range / altitude where relevant) |
|---|---|---|
| `mbt_a` | Main battle tank, heavy | HP 220, armour heavy (§21.2 schema example) |
| `mbt_b` | Main battle tank, light | HP 150 |
| `ifv_1`, `apc_1`, `scout_1` | Armoured vehicles | Convoy and base security |
| `spg_1`, `mrl_1` | Self-propelled howitzer, multiple-rocket launcher | Area fire (visual/objective pressure) |
| `sam_man` "Sling" | MANPADS team | 4.5 km / 3 km; IR; no radar |
| `sam_pt` "Gnat" | Point-defence SAM (vehicle) | 8 km / 5 km; radar-cued optical/IR |
| `sam_med` "Halberd" | Medium-range SAM | 40 km / 20 km; search + track radar; 4 launchers × 2 missiles |
| `sam_long` "Bastion" | Long-range SAM | 150 km / 30 km; large search radar; 6 launchers × 2 missiles; slow reload |
| `aaa_spg` | Twin 30 mm self-propelled AAA | 3 km / 2 km |
| `aaa_57` | Towed 57 mm battery with fire-control radar | 6 km / 4 km |
| `ewr_1` | Early-warning radar | R_ref 250 km vs 5 m² |
| `gci_1` | GCI/3-D surveillance radar | R_ref 180 km vs 5 m² |
| `fcr_1` | Fire-control radar (SAM-associated) | 80 km vs 5 m² |
| `cmd_sector`, `cmd_battery` | Command bunker / battery command post | Network nodes (§21.5) |
| `log_fuel`, `log_ammo`, `log_col`, `log_rail` | Logistics | Convoy and rail targets (§39.5) |
| `ind_factory`, `ind_refinery`, `ind_depot` | Industrial | Strategic targets (§47.6) |
| Infrastructure | Bridge, dam, rail yard, power plant, substation, pylon line, comm tower | Target definitions (§47.6, A.10) |
| Installations | Hardened shelters, hangars, barracks, fuel farm, ammunition depot | Airbase objectives (§48.7) |

## D.4 Ships (fictional; design targets)

| Id | Class | Length (m) | Speed (kn) | Notes |
|---|---|---|---|---|
| `carrier_large` | Carrier, CATOBAR | 330 | 30 | 2–3 catapults, 4 arrester wires, air wing ≤ 60 (§22.6) |
| `carrier_light` | Carrier, STOBAR (ski-jump) | 280 | 28 | Smaller air wing ≤ 36 |
| `cruiser` | Area air defence | 180 | 32 | Long-range SAM, strike |
| `destroyer` | Air/surface defence | 150 | 32 | Medium SAM, CIWS |
| `frigate` | Escort | 125 | 30 | Point defence, ASW |
| `patrol` | Coastal patrol | 60–80 | 35 | Light weapons |
| `transport`, `tanker_ship` | Merchant | 120–220 | 16 | Convoy and logistics targets |
| `sub_1` | Submarine (staged, P10) | 90–110 | 25 submerged | §22.7 |

## D.5 Weapons roster

The full 26-weapon table with class, era, mass, guidance, design maximum range, warhead and mount is in §15.12. Non-weapon stores: ECM pod `ecm1`, stand-off jammer pod `ecm3`, targeting pod `tp2`, reconnaissance pod, drop tank `ft1400` (1 400 L), towed decoy `td1`, chaff/flare dispenser loads. Faction weapon lists: §37.2–§37.3. *Missile ranges are inputs to the DLZ baker and design targets; they have not been calibrated numerically (see Appendix I, known limitations).*

## D.6 Callsigns and naming

Callsign sets (`data/callsigns.json`, 120 words at launch): `raptor_birds` (birds-of-prey names, e.g. Raptor, Merlin, Osprey), `weather` (Squall, Gale, Sleet), `tools` (Anvil, Chisel), `mountains`. Rules: ≤ 12 characters, pronounceable, none that match real unit callsigns; flight position numbers 1–4 (e.g. "Raptor 2-1"); fictional support callsigns: AWACS "Watchtower", tanker "Reservoir", ground controller "Lighthouse". Naming review (G8) checks all names, callsigns and emblems against a blacklist (§37.4).

## D.7 Era mapping of launch content

| Era | Aircraft | Missiles (examples) |
|---|---|---|
| G3 | Badger, Albatross, Kestrel (G3 variant) | Dart-1, Lance-2, GR-20, GH-30 |
| G4 | Kestrel, Condor, Swift, Bittern, Magpie, Argus, Gannet | Dart-1/4, Lance-2/4, Talon-1/3, Hunter-2 |
| G4.5 | Peregrine | Dart-4, Lance-6, Dirk-C, GB-500G |
| G5 | Wraith, Chimera (EXP) | Pike-9, Lance-6 |

---

# APPENDIX E: TEST PLAN & QA MATRIX

Complements §61.10 (strategy), §60.9–§60.10 (performance and visual tests) and §66.4 (gates). Every acceptance criterion in this document has an ID `AC-<section>.<n>`; this appendix defines how they are executed, grouped and traced.

## E.1 Test levels and ownership

| Level | Scope | Runs | Owner |
|---|---|---|---|
| L0 Static | Lint, formatting, dependency graph, licence/CSP audits, `NoHardcodedContentLint`, schema validation of content | Every commit | Engineering |
| L1 Unit/property | Maths, codecs, parsers, validators, shaping, DNA metric, history weights | Every commit | Engineering |
| L2 Component | Subsystem tests with fixtures: flight model, radar/LOS, missile, damage, mission VM, generator, save/migrate | Every commit (fast subset), nightly (full) | Engineering |
| L3 Headless integration | Sim Core scenarios with scripted pilots: missions, AI engagements, airbase ops, campaign operations | Every merge to main | Engineering/QA |
| L4 Determinism/golden | WASM vs native state hashes; generator golden hashes over 1 000 seeds | Every merge | Engineering |
| L5 Browser E2E | Playwright: boot, compat check, menus, mission start, replay, settings, offline reload, multiplayer smoke | Every merge (smoke), nightly (full) | QA |
| L6 Network | Impairment matrix, 16-client headless soak, LAN-only | Nightly/weekly | Network engineer |
| L7 Performance | PS-1…PS-9 on lab hardware | Nightly (three classes) | Performance engineer |
| L8 Visual regression | Screenshot suite (§60.10) | Nightly | Graphics |
| L9 Soak/long-run | 2 h mixed mission, 20-run campaign soak, AI-vs-AI batches (10⁴ missions) | Nightly/weekly | QA |
| L10 Human playtests | Gates G1–G7, ET-1/2/3, usability (SUS), accessibility sessions | Per milestone | Design/QA |
| L11 Security | Fuzzers (parser, network decoder, VM, loader, `.gpk` reader), dependency audit, CSP | Nightly (fuzz 10 min/target in CI) | Engineering |

## E.2 Acceptance-criteria index by chapter

| Chapter | AC range | Main verification level |
|---|---|---|
| §3 Pillars | AC-3.1 … 3.4 | L2/L3/L9 (playable battlefields, conformance, AI envelope, DNA diversity) |
| §4–§6 | AC-4.5 … | L10 (first sortie), L5 |
| §7 Geodesy | AC-7.* | L1/L4 |
| §8–§10 | AC-8.*, 9.*, 10.* | L1/L2/L5 |
| §11–§12 | AC-11.*, 12.* | L2/L4 (conformance test, hash equality) |
| §13–§14 | AC-13.*, 14.* | L0/L2 (roster balance, loadout validation) |
| §15–§18 | AC-15.* … 18.* | L2/L9 (hit-rate sweeps, sensor tests, EW, damage) |
| §19–§20 | AC-19.*, 20.* | L3/L9 (AI ladder, orders, formations) |
| §21–§24 | AC-21.* … 24.* | L2/L3/L8 |
| §25–§31 | AC-25.* … 31.* | L2/L3/L9 (VM, generator validity, escalation, bosses, configs) |
| §32–§33 | AC-32.*, 33.* | L6/L9 |
| §34–§43 | AC-34.* … 43.* | L3/L9/L10 (campaign soak, chronicle test) |
| §44–§47 | AC-44.* … 47.* | L2/L7/L8 (streaming, data pipeline, terrain, cities) |
| §48–§50 | AC-48.* … 50.* | L2/L3 |
| §51–§52 | AC-51.*, 52.* | L1/L11/L5 (UGC validation, replay) |
| §53–§58 | AC-53.* … 58.* | L5/L8/L10 (UX, HUD, map, audio, accessibility, controls) |
| §59–§60 | AC-59.*, 60.* | L7/L9 |
| §61–§65 | AC-61.* … 65.* | L2/L5/L9 (resilience, content, debug, save, statistics) |
| §66–§71 | AC-66.* … 71.* | Process/L10 (gates, experience tests) |

## E.3 Environments and compatibility matrix

| Dimension | Values | Notes |
|---|---|---|
| Browsers | Chrome (stable, stable−1), Edge (stable), Firefox (stable), Safari (stable) | Chromium primary; Firefox/Safari smoke + feature-detected status published [verify] |
| OS | Windows 11, macOS (current−1), Linux (Ubuntu LTS) | Gamepad/HOTAS axis mapping tested per OS/browser |
| GPU classes | Integrated (Xe/680M class), mid (GTX 1660/RTX 2060), high (RTX 3070+) | Plus one Apple-silicon GPU and one AMD discrete |
| Hardware profiles | Minimum, Recommended, High (§0.6.3) | Lab machines for gates |
| Display | 1280×720, 1920×1080, 2560×1440, 3840×2160, 21:9 ultrawide; 60/144 Hz | |
| Input devices | Keyboard+mouse, Xbox-class pad, PlayStation-class pad, 3 HOTAS families, pedals, head tracker | |
| Network | LAN, Wi-Fi, home broadband, emulated impairments | RTT 20/80/150/300 ms × loss 0/2/5/10 % × jitter 0/20/50 ms |
| Storage conditions | Normal, low quota, persistence denied, private window, cleared data | |
| Isolation | Cross-origin isolated, non-isolated (degraded mode) | |

## E.4 Test data and fixtures

- **Minimal GeoPack** (3 regions: alpine, desert coast, island chain) and a **full pilot pack** (6 regions → 20+).
- **Reference content:** Kestrel plus a content fixture pack (synthetic aircraft/weapons covering schema edges, malformed cases for the baker).
- **Seeds:** 1 000 golden seeds with stored mission hashes; 200 "pathological" seeds from earlier failures.
- **Scripted pilots:** baseline (Veteran), perfect (Ace), erratic (random) for AI and mission tests.
- **Save fixtures:** one save from every released schema version.
- **Replay fixtures:** SSR/OSR for regression of decoders.
- **Corpora:** 60 malformed content files, 40 invalid HUD layouts, 200 route cases, 30 lint snippets, 100 attribution scenarios (statistics).

## E.5 Defect severity and release rules

| Severity | Definition | Rule |
|---|---|---|
| **P0** | Crash/data loss/security flaw/blocks play on supported setups | Fix before any build ships |
| **P1** | Major feature broken, desync, severe perf regression (> 10 % budget), accessibility blocker | Fix before milestone close/release |
| **P2** | Significant but has a workaround; visual or balance defect | Planned within the milestone or next |
| **P3** | Minor/cosmetic | Backlog |

Gate G9 requires zero open P0/P1. A flaky test is quarantined within 24 h and fixed within a week; quarantined tests count against the milestone exit.

## E.6 Traceability

A traceability table (maintained in the tracker, exported per gate) links: requirement (GDD §) → AC id → automated test id / playtest protocol → latest result/build. The three experience tests (ET-1/2/3, §70) link each beat to an AC or logged observable (`AC-70.4`). Changes to a chapter's AC list require updating this table in the same change.

## E.7 Regression strategy

1. Every bug fix adds a regression test at the lowest viable level (L1/L2) or a fixture.
2. Golden hashes change only with an ADR/PR note explaining why (generator or sim change), never silently.
3. Performance baselines are per commit; a > 5 % regression blocks the merge unless waived.
4. Nightly runs report trends (frame time, memory, hit-rates, DNA diversity, SDI, Cinematic Score); threshold regressions open P1 tickets.

## E.8 Manual test checklists

Smoke (per build): first-run flow, Quick Flight, take-off/landing, one kill, pause/resume, settings round-trip, offline reload, replay playback. Release (per RC): full install/update, data export/import, accessibility walkthrough, 4-human LAN session, region download/delete, quota-exhaustion scenario, browser compat sweep, credits/licence screen review.

---

# APPENDIX F: RISK REGISTER

Scale: likelihood L and impact I ∈ {1 low … 5 high}; exposure = L × I. Reviewed at every milestone gate; owners are roles.

| ID | Risk | L | I | Exp. | Mitigation | Early indicator | Owner |
|---|---|---|---|---|---|---|---|
| R-01 | **Rust/WASM expertise and tooling friction** (debugging, threads, build size) | 3 | 4 | 12 | Hire/train early; thin FFI; native-first development and tests; fall back to TypeScript sim (OQ-01) with −30–40 % budgets | M0 build/debug pain; velocity | Tech lead |
| R-02 | **WebGPU coverage/driver variance** (crashes, perf cliffs, Safari/Firefox gaps) | 3 | 4 | 12 | WebGL2 compat renderer; device-lost recovery (§61.9); pipeline cache; GPU/driver quirks table; lab hardware matrix | Device-lost rate in beta telemetry | Graphics lead |
| R-03 | **Terrain streaming holes/hitches** at supersonic low level | 3 | 5 | 15 | Residency invariant, look-ahead, sim terrain layer independent of render, PS-3 gate each milestone | Hole counter in debug overlay | World/tools lead |
| R-04 | **Flight feel misses the bar** (too arcade or too brittle) | 3 | 5 | 15 | G1 gate before content; assist presets; conformance tests; early blind A/B; autotuner | G1 pilot sessions | Flight lead |
| R-05 | **Core concept not fun** (First Playable fails G2) | 2 | 5 | 10 | Build the 11-step script first; iterate on pacing/AI/presentation; defer breadth | Internal playtests from M1 | Design lead |
| R-06 | **AI quality at scale** (weak or exploitative; cost explosion) | 3 | 4 | 12 | Utility AI + predictive BFM; tiering; AI ladder and fairness audits; skill vector tuning | AI ladder tests; playtest feedback | AI lead |
| R-07 | **Browser networking limits** (NAT, WebRTC without server, background-tab throttling for hosts) | 4 | 4 | 16 | Serverless invite blobs; LAN-first design; dedicated server build; host warnings; impairment testing; WebSocket LAN fallback | NAT-less invite success rate (target ≥ 95 %) | Network lead |
| R-08 | **Determinism drift** between WASM and native / browsers | 2 | 3 | 6 | No fast-math, software libm, CI hash tests; authority is server (runtime play doesn't depend on bit-exactness) | CI hash mismatches | Sim lead |
| R-09 | **Performance budget overruns** (sim, GPU, memory) | 4 | 4 | 16 | Budgets per system (§60.2); governor; nightly perf gates; scalable tiers; cut lines | PS-1/PS-2 trend | Performance engineer |
| R-10 | **Geodata licensing/attribution error** (ODbL share-alike, CC BY notices, provider terms change) | 2 | 5 | 10 | Descriptors per provider; credits generated; `terms_checked` ≤ 12 months gate; legal review; ODbL derived-database publication | Audit failures; provider ToS notices | Release owner |
| R-11 | **IP/naming conflicts** for aircraft, weapons, factions, callsigns | 3 | 3 | 9 | Fictional designs; emblem similarity lint; name review (G8); avoid real unit callsigns | Review findings | Producer |
| R-12 | **Scope creep** (campaign, EW, naval depth) | 4 | 4 | 16 | Priority order §68; cut lines §66.6; stubs behind interfaces; feature test §71.5 | Burn-down vs milestone | Producer |
| R-13 | **Storage eviction/quota** loses saves or caches | 3 | 3 | 9 | Persistence request, export/import, checkpoints, status UI, reminders (§64.6) | Eviction reports | Platform engineer |
| R-14 | **Audio autoplay/latency/mix issues** across browsers | 3 | 2 | 6 | User-gesture flow; worklet DSP; latency tests; fallback paths | QA matrix results | Audio lead |
| R-15 | **Input device fragmentation** (HOTAS mapping per OS/browser) | 4 | 3 | 12 | Profile-based mapping in data; calibration wizard; known-device table; community device reports | Support tickets | UX/input engineer |
| R-16 | **Mission generator produces unsolvable/boring missions** | 3 | 4 | 12 | Validation, solvability, DNA, SDI, Cinematic Score in CI; 10⁴-seed batches; Generator Lab | Batch failure rates | Missions lead |
| R-17 | **Campaign soft-locks or imbalance** | 3 | 4 | 12 | AI-played soak; branching metrics; event deck tuning; save-state invariants | Soak failures | Campaign lead |
| R-18 | **Accessibility gaps** in HUD/canvas | 3 | 3 | 9 | DOM for menus (ADR-20); equivalents for HUD; external reviews (G7); automated checks | axe findings; tester reports | UX lead |
| R-19 | **Security** (malicious shared missions, XSS, supply chain) | 2 | 4 | 8 | Sandboxed VM, validation, CSP/Trusted Types, SBOM, fuzzing, pinned deps | Fuzz crashes; audit alerts | Security owner |
| R-20 | **Art/audio throughput** (roster, cockpits, voice banks) | 3 | 4 | 12 | Kit-bashed roster (OQ-05); asset budgets; parallel pipeline from M1; outsourcing plan | Asset burn-down | Art/audio leads |
| R-21 | **Memory limits in tabs** (WASM 32-bit, JS heap, GPU) | 3 | 4 | 12 | Budgets, caps by profile, OOM handling, streaming discipline | Memory telemetry | Performance engineer |
| R-22 | **Dependency on open-data quality** (OSM gaps, DEM voids, airport errors) | 3 | 3 | 9 | Procedural completion, improvised airfields, fallback terrain, QA reports in pipeline | Pipeline QA metrics | World/tools lead |
| R-23 | **Key-person risk** (small team) | 3 | 4 | 12 | Documentation (this GDD, ADRs), code ownership with backups, pairing on sim-core/protocol | Bus-factor review | Producer |
| R-24 | **Missile/sensor tuning unvalidated** (ranges are design targets) | 3 | 3 | 9 | DLZ baker + calibration pass at M3 against §15 targets; hit-rate sweeps (AC-15.5) | Sweep results | Weapons lead |

Top exposures (≥ 15): R-03, R-04, R-07, R-09, R-12. Their mitigations are scheduled as explicit milestone tasks, not left to chance.

---

# APPENDIX G: GLOSSARY & RADIO BREVITY

## G.1 Glossary

| Term | Meaning |
|---|---|
| **AB** | Afterburner (reheat): extra thrust from burning fuel in the exhaust |
| **ADR** | Architectural Decision Record (§0.4) |
| **AGL / MSL** | Above ground level / mean sea level (here: MSL treated as height above the ellipsoid, ADR-05) |
| **AAA** | Anti-aircraft artillery |
| **AAR** | Air-to-air refuelling (§49.5) |
| **AC-x.y** | Acceptance criterion identifier |
| **ARH / SARH** | Active / semi-active radar homing |
| **ARM** | Anti-radiation missile (homes on emitters) |
| **AoA (α)** | Angle of attack |
| **AWACS** | Airborne early warning and control aircraft (GCI-capable) |
| **Bandit / Bogey** | Hostile / unidentified aircraft |
| **BF** | Battlefield Frame: the curved local frame used by the simulation (§0.3.2, §7) |
| **BFM / ACM** | Basic fighter manoeuvres / air combat manoeuvring |
| **Bingo / Joker** | Fuel state requiring return / a warning fuel state above bingo (§49.6) |
| **Bullseye** | Mission reference point for position calls (§50.5) |
| **BVR / WVR** | Beyond / within visual range |
| **CAP** | Combat air patrol |
| **CAS** | Close air support (also Calibrated airspeed in air-data context) |
| **CCIP / CCRP** | Continuously computed impact / release point |
| **CG / MAC** | Centre of gravity / mean aerodynamic chord |
| **CM** | Countermeasures (chaff, flares, decoys, jamming) |
| **CRC32C / SHA-256** | Checksums used for tile and save integrity |
| **DEM** | Digital elevation model |
| **DLZ** | Dynamic launch zone (weapon envelope shown to the pilot, §54.5) |
| **DNA (mission)** | Descriptor vector used to measure mission similarity (§69.2) |
| **DOF** | Degrees of freedom (6-DOF rigid body) |
| **ECCM / ECM / EW / ESM** | Electronic counter-countermeasures / countermeasures / warfare / support measures |
| **ECEF** | Earth-centred Earth-fixed coordinates |
| **EMCON** | Emission control (restricting radar/radio emissions) |
| **ENU / NED** | East-North-Up / North-East-Down local frames |
| **ERP** | Effective radiated power |
| **FBW / FCS** | Fly-by-wire / flight control system |
| **FDM** | Flight dynamics model |
| **FLIR / IRST / EO/IR** | Forward-looking infrared / infrared search and track / electro-optical-infrared |
| **FOA / HRTF / VBAP** | First-order ambisonics / head-related transfer function / vector-base amplitude panning (audio, §56) |
| **GCI** | Ground-controlled intercept |
| **GeoPack / `.gpk`** | Versioned geodata archive format (§45.5) |
| **GNSS / INS** | Satellite navigation / inertial navigation system (§50.2) |
| **GPWS / GCAS** | Ground proximity warning / ground-collision avoidance system |
| **HOTAS** | Hands on throttle and stick |
| **HUD / MFD** | Head-up display / multi-function display (§54) |
| **IADS** | Integrated air defence system |
| **IAS / TAS / EAS / CAS** | Indicated / true / equivalent / calibrated airspeed (§11.5) |
| **IFF** | Identification friend or foe |
| **ISA** | International Standard Atmosphere |
| **JTAC** | Joint terminal attack controller (ground controller role) |
| **LCOS** | Lead-computing optical sight |
| **LOD / SSE** | Level of detail / screen-space error (§44.4) |
| **LOS** | Line of sight (service in §16.9) |
| **LPI** | Low probability of intercept |
| **LP** | Legacy Points (campaign meta-progression, §38.5) |
| **MANPADS** | Man-portable air-defence system |
| **MRM / LRM / SRM** | Medium / long / short-range missile |
| **MSL height** | Height above mean sea level |
| **NCTR** | Non-cooperative target recognition |
| **NN-x** | Non-negotiable design rule (§3.9) |
| **OPFS** | Origin Private File System (browser storage) |
| **OSR / SSR** | Omniscient / state-stream replay (§52.2) |
| **PN / APN** | Proportional / augmented proportional navigation |
| **P_s** | Specific excess power (energy rate) |
| **PWA** | Progressive Web App |
| **QRA** | Quick reaction alert (scramble readiness) |
| **RCS** | Radar cross-section |
| **RI** | Realism Index (§59.4) |
| **RTE** | Relative-to-eye rendering (§44.8) |
| **RWR** | Radar warning receiver (§16.8) |
| **SAB** | `SharedArrayBuffer` |
| **SAM / SEAD / DEAD** | Surface-to-air missile / suppression / destruction of enemy air defences |
| **SDI** | Solution Diversity Index (§69.5) |
| **STT / TWS / RWS** | Single-target track / track-while-scan / range-while-search (radar modes) |
| **T0–T4** | Simulation fidelity tiers (§0.6.1) |
| **TACAN** | Tactical air navigation beacon (range/bearing) |
| **TFOV** | Total field of view (HUD) |
| **TOT** | Time on target |
| **TSFC** | Thrust-specific fuel consumption |
| **WGS84** | World Geodetic System 1984 |
| **WMM** | World Magnetic Model |
| **WSO / EWO** | Weapon systems officer / electronic warfare officer (second-seat roles) |
| **z (tile level)** | Quadtree level of the terrain tiling (§44.3) |

## G.2 Phonetics and number pronunciation

Alphabet: Alfa, Bravo, Charlie, Delta, Echo, Foxtrot, Golf, Hotel, India, Juliett, Kilo, Lima, Mike, November, Oscar, Papa, Quebec, Romeo, Sierra, Tango, Uniform, Victor, Whiskey, X-ray, Yankee, Zulu. Numbers: zero, one, two, **tree**, **fower**, **fife**, six, seven, eight, **niner**; altitude "angels" (thousands of feet) for air, "flight level"; bearings are three digits; ranges in nautical miles (or km by option).

## G.3 Brevity terms

| Term | Meaning (game usage) |
|---|---|
| **Abort** | Cancel the current action/attack |
| **Angels** | Altitude in thousands of feet |
| **Bandit** | Confirmed hostile |
| **Bingo** | Fuel at the minimum needed to return |
| **Blind** | No visual contact with friendly |
| **Bogey** | Unidentified contact |
| **Break (left/right)** | Immediate maximum-performance turn to defeat a threat |
| **Bullseye** | Reference point; used as "bullseye 270 for 40" = bearing 270°, 40 nm from bullseye |
| **Buddy spike** | RWR indication from a friendly emitter |
| **Cleared hot** | Authorised to attack/release |
| **Cold** | Heading away from the threat/bandits (hot = toward) |
| **Contact** | Radar contact |
| **Defending** | Reacting to a threat, not engaging |
| **Engaged** | Committed to the fight |
| **Feet wet / dry** | Over water / over land |
| **Fox One** | Semi-active radar missile launch |
| **Fox Two** | Infrared missile launch |
| **Fox Three** | Active radar missile launch |
| **Fuel (state)** | Remaining fuel in thousands of pounds / kg |
| **Gimbal** | Target outside the gimbal limits of the radar/seeker |
| **Joker** | Fuel state above bingo to start considering return |
| **Judy** | Pilot has the picture and is taking control of the intercept |
| **Mayday** | Emergency |
| **Merge / Merged** | Aircraft have passed each other / entered the visual fight |
| **Mud** | Surface-to-air threat (with type/spike) |
| **Naked** | No RWR indication |
| **Notching** | Turning to beam the threat radar to exploit the Doppler notch |
| **Padlock** | Visual lock-on to a target |
| **Picture** | AWACS summary of the air situation |
| **Pitbull** | Active missile seeker has gone autonomous |
| **Pump** | Turn away to reduce closure/pressure without fully disengaging |
| **Rifle** | Air-to-ground missile launch |
| **Shack** | Direct hit/kill |
| **Skip it** | Do not engage a particular target |
| **Splash** | Target destroyed |
| **Spiked** | RWR indicates radar lock on me |
| **Tally** | Visual contact with the enemy |
| **Visual** | Visual contact with a friendly |
| **Weapons free / tight / hold** | Engagement authorisation levels |
| **Winchester** | Out of ordnance |

## G.4 Example calls (grammar and variants)

| Event | Template | Example |
|---|---|---|
| Picture | `[Speaker]: [Group], bullseye [brg], [rng], angels [alt], [aspect], [track direction]` | "Bandit group, bullseye two-seven-zero, forty, angels twenty, hot." |
| Launch warning | `[Speaker]: Warning. [Threat type] launch, [clock/bearing]` | "Warning. Missile launch detected, six o'clock." |
| Own launch | `[Callsign]: Fox [one/two/three], [target brief]` | "Raptor two-one, fox three, group bullseye two-seven-zero forty." |
| Splash | `[Callsign]: Splash [n]` | "Raptor two-two, splash one." |
| Bingo | `[Callsign]: Bingo, [position]` | "Raptor two-one, bingo, request RTB." |
| Tanker | `[Tanker]: [Callsign], cleared pre-contact` / `contact` / `disconnect` | "Raptor two-one, cleared pre-contact." |
| Tower | `[Tower]: [Callsign], runway [nn], cleared for take-off, wind [dir] at [spd]` | "Kestrel two, runway two-seven, wind two-six-zero at one-two, cleared for take-off." |
| Escalation | `[AWACS]: [Report]` | "Enemy reinforcements entering the battlespace." |

Each template has ≥ 6 weighted variants and urgency variants (calm/urgent/panicked); the recency penalty of §56.6 avoids repetition.

---

# APPENDIX H: GEODATA SOURCES & LICENSE MATRIX

**Status: planning summary, not legal advice.** Licence terms change. Every entry marked [verify] MUST be re-checked against the provider's current terms by the release owner before each datapack release, and the result recorded in the provider descriptor's `terms_checked` field (§45.9). A datapack release is blocked if any descriptor is missing or older than 12 months.

## H.1 Source matrix

| Dataset | Provider | Used for | Resolution / scale | Licence (summary) | Attribution / notice (indicative) | Cache offline? | Redistribute in packs? | Share-alike? |
|---|---|---|---|---|---|---|---|---|
| Copernicus DEM GLO-30 | ESA / Copernicus | Land elevation | 30 m | Free, open access with attribution and notice requirements [verify] | "Contains Copernicus DEM data (© DLR e.V. 2010–2014 and © Airbus Defence and Space GmbH 2014–2018, provided under COPERNICUS by the European Union and ESA; all rights reserved)" [verify exact wording] | Yes | Yes (derived tiles) [verify] | No |
| SRTM 1″ | NASA/USGS | Land elevation fallback (≈ 60° N–56° S) | 30 m | Public domain | Courtesy NASA/USGS (credit requested) | Yes | Yes | No |
| GEBCO grid | GEBCO | Bathymetry | 15″ (~460 m) | Free use with acknowledgement | "GEBCO Compilation Group (year) GEBCO Grid" | Yes | Yes | No |
| ESA WorldCover | ESA | Land cover | 10 m | CC BY 4.0 | "© ESA WorldCover project / Contains modified Copernicus Sentinel data (year) processed by ESA WorldCover consortium" | Yes | Yes | No |
| OpenStreetMap | OSM contributors | Roads, rail, buildings, water, power, aeroway, landuse | Vector | **ODbL 1.0** | "© OpenStreetMap contributors"; link to copyright page | Yes | Yes, with ODbL conditions | **Yes** for derived databases |
| Overture Maps | Overture Maps Foundation | Alternative/complement for buildings, places, transportation | Vector | Per theme: mostly CDLA Permissive 2.0 / ODbL for some themes [verify] | Per theme attribution [verify] | Yes | Per theme [verify] | Per theme |
| OurAirports | OurAirports.com | Airports, runways | Point/line | Public domain | Courtesy (credit optional) | Yes | Yes | No |
| GeoNames | GeoNames | Place names/search | Point | CC BY 4.0 | "GeoNames (geonames.org), CC BY 4.0" | Yes | Yes | No |
| Natural Earth | NACIS / community | Populated places, regions, marine areas, coastlines (global scale) | 1:10m | Public domain | Optional credit | Yes | Yes | No |
| ERA5 (derived climatology) | Copernicus Climate Change Service / ECMWF | Monthly climate grid | 0.25°→1° aggregated | Copernicus licence / CC BY 4.0 depending on release [verify] | "Contains modified Copernicus Climate Change Service information" | Yes | Yes (derived grid) [verify] | No |
| World Magnetic Model | NOAA/NCEI & BGS | Magnetic declination | Model coefficients | Public domain (US) / open [verify] | Credit NOAA/BGS | Yes | Yes | No |
| NASA Blue Marble / Black Marble class imagery | NASA Earth Observatory | Globe albedo, night lights | 500 m–1 km | NASA media usage guidelines (generally public domain) [verify] | "NASA Earth Observatory" | Yes | Yes [verify] | No |
| Star catalogue (Yale BSC / Hipparcos-derived) | Public / ESA | Sky rendering | Catalogue | Public domain / ESA terms [verify] | Credit as required [verify] | Yes | Yes [verify] | No |
| Sentinel-2 based imagery (optional user-selected) | Copernicus | Optional imagery layer | 10 m | Free and open [verify]; third-party renderings vary | "Contains modified Copernicus Sentinel data" | Per provider descriptor | Per provider | No |

**Not used:** Google Earth / Google Maps imagery or terrain; Bing/Mapbox/other commercial tiles (caching/redistribution restrictions); any dataset whose terms forbid caching or redistribution within a game package.

## H.2 Obligations checklist

| Obligation | Source | Implementation |
|---|---|---|
| Attribution notices visible | CC BY, ODbL, Copernicus | Credits & Data screen generated from descriptors; map footer line (§45.9, §53.5) |
| ODbL: public availability of the derived database | OSM | The pipeline publishes an ODbL dump of the packed feature tiles with each datapack; the game code and rendered "produced works" are not affected |
| ODbL: notice of the licence on the derived database | OSM | `LICENSE` file inside the dump and the `.gpk` metadata (`licenses` list) |
| No restriction on downstream use | ODbL | Packs do not add technical restrictions (DRM) to the ODbL-derived layers |
| Share changes | ODbL | Corrections to OSM are not required but are encouraged; the dump satisfies the derivative obligation |
| Copernicus notices | Copernicus | Notice string in the credits and in `.gpk` header |
| Cache policy flags | Per descriptor | `cache_allowed`, `redistribution_allowed`, enforced by the cache layer (§45.9) |
| Third-party software licences | npm/crates | SBOM at release; licences screen lists them |
| Audio/music/fonts | Content licences | Licence records in the asset registry (G8) |
| Privacy | GDPR-style | No personal data collected by default (§53.13) |

## H.3 Descriptor schema (summary)

```json
{ "id": "osm", "name": "OpenStreetMap", "licence": "ODbL-1.0", "attribution": "© OpenStreetMap contributors",
  "url": "https://www.openstreetmap.org/copyright", "commercial_ok": true, "cache_allowed": true, "redistribution_allowed": true,
  "share_alike": true, "terms_checked": "2026-10-07", "required_notices": ["Contains information from OpenStreetMap, made available under the ODbL"] }
```

## H.4 Release process

1. Pipeline run produces provenance for every layer (URL, date, hash).
2. Release owner re-checks each descriptor's terms; updates `terms_checked`.
3. CI licence gate: all layers map to a descriptor; none older than 12 months; ODbL dump present for OSM-derived layers.
4. Legal review for any new provider or terms change.
5. Publish datapack + ODbL dump + credits; update `datapack.json`.

---

# APPENDIX I: DOCUMENT MAINTENANCE

## I.1 Document control

| Item | Value |
|---|---|
| Title | Game Design Document — Real-World Sandbox Combat Flight Simulator |
| Source | `GDD_Outline.md` (153 numbered items, 71-section table of contents) |
| Platform | Desktop web browser (installable PWA) — ADR-01 |
| Format | Single Markdown file (`GDD.md`), headings per the table of contents; Appendices A–I |
| Roadmap companion | `roadmap.md` (version plan v0.0 → v1.0) |
| Version scheme | `MAJOR.MINOR.PATCH`: MAJOR = change of vision/platform; MINOR = new or changed chapters/ADRs; PATCH = corrections/clarifications |
| Current version | 0.2.2 (v0.1b control and ground clarifications) |

## I.2 Change log

| Version | Change |
|---|---|
| 0.1.x | First draft, engine-agnostic/desktop assumptions (superseded) |
| 0.2.0 | Browser-native specification: WebGPU/WebGL2 renderer, Rust→WASM Sim Core, worker topology, WebRTC networking, OPFS/IndexedDB storage; all 71 chapters and Appendices A–I written; replay decision aligned with ADR-09 (snapshot-based); Kestrel reference data computed |
| 0.2.1 | Clarifications from building the v0.1a flight model (ADR-29, ADR-30, ADR-31): §11.2.4 sign of the side-force term of `Z`; §11.6.3 post-stall blend is a data-driven smoothstep; §11.19 sustained load factor is wind-axis, `mach` and `requires` on envelope points; §61.5 Aero seam wired; A.2 and A.3 notes on what `aircraft/1` and `engine/1` implement now; C.3 spool figures against §11.8.2; C.7 rate of climb is the maximum 1 g P_s; C.8 altitude datum and 0.01 Mach floor; C.9 envelope fields; OQ-01 closed. No gate or target changed |
| 0.2.2 | Clarifications from building the v0.1b control and ground work (ADR-32 to ADR-38): §11.11.2 the control laws are incremental dynamic inversion; §11.11.3 Full Assist is a bank-angle command and the recovery button's behaviour; §11.13.2 tanh-regularised, additive friction; §11.13.3 how the touchdown thresholds are judged, and that they govern over §18.8.1; §11.13.4 the debounce; §11.14 `n_z` at the CG, the dial, the AC-11.9 figures; §11.15 lateral tables against α, `C_nβ,dyn` and no break-up before §18; §48.2 Runway hot as built; §54.9 the Auto-GCAS timing conflict; §58.2 InputFrame v3; §58.4 mouse-aim runs in the sim; AC-58.3 measured from the source event; AC-58.4 scaled to the aircraft (owner decision, ADR-33 amendment 1); A.1 `fcs/1`; A.2 gear, contacts, `n_max_by_config`, `fcs.gains_ref`; C.8 the ground-roll segments and the C.7/C.8 landing-roll note (target kept). The header version, left at 0.2.0 in 0.2.1, is corrected. No gate or target changed except as the owner decided (AC-58.4) |

## I.3 Ownership

Each chapter has a content owner (role) responsible for keeping text, tables and acceptance criteria consistent. Suggested owners: Flight (§7, §11–§12, App. C), Weapons/Sensors (§15–§18), AI (§19–§20, §59), World/tools (§7–§10, §44–§47, App. H), Missions/Campaign (§25–§43, §69), Network (§33), UX/Audio (§53–§58), Engineering/Platform (§60–§65, App. A, E), Producer (§66–§68, §70–§71, App. F, I).

## I.4 Change process

1. A change that alters a normative value, a decision or an interface starts as an ADR (or an update to one) in `/docs/adr`.
2. The change set updates every chapter listed under the ADR's *Affects* column, the relevant Appendix B formula and the acceptance criteria.
3. If a value defined in §0 changes, §0 is updated first; later chapters that contradict §0 are defects.
4. Cross-references (`§N.M`), AC identifiers and the table of contents anchors are validated by a script in CI before merge.
5. The change log (I.2) and the version are updated; the traceability table (App. E.6) is updated for AC changes.

## I.5 Consistency rules

- **Units:** SI in specifications; display conversions only in UI sections; unit suffixes in data keys.
- **Tags:** `design target` for tunable starting values; `[verify]` for external facts that can change (browser APIs, licences, standards); `*target*` in Appendix C for non-validated values.
- **IDs:** `ADR-nn`, `OQ-nn`, `NN-nn`, `AC-<section>.<n>`, `R-nn`, gates `G1…G9`, milestones `M0…M7`, builds `PS-n`/`ET-n`.
- **Tables over prose** for parameters; each chapter ends with acceptance criteria.
- **Fictional content only:** names for aircraft, weapons and factions are working names (ADR-11).

## I.6 Verification status of numbers (honest scope)

| Category | Status |
|---|---|
| ISA tables, geodetic conversion check vector, curvature-drop table, tile counts and data volumes, radar-scaling table, catapult/arrest accelerations, bandwidth estimate, DNA worked example, history-penalty examples, INS drift median | **Computed or hand-verified** during authoring |
| Kestrel point performance, take-off/landing rolls, short-period figures (Appendix C) | **Computed** with the reference calculator (C.8); the 6-DOF model must match within tolerance |
| Aircraft roster values other than the Kestrel (§13.4) | **Design targets**; arithmetic (T/W, wing loading) checked, performance not yet validated |
| **Missile and weapon ranges (§15.12) and DLZ values** | **Design targets, not calibrated.** No numerical missile-range calibration was run while writing this document; they are inputs to the DLZ baker and must be calibrated at M3 against the hit-rate targets of §15 (risk R-24) |
| Sensor ranges, SAM envelopes, EW burn-through values | Design targets (formula-consistent, uncalibrated) |
| Performance allocations (§60.2, §60.7.2), memory budgets, asset budgets | Planning allocations, to be replaced by measurements at M3 |
| Gate thresholds (§66.4, §71.3) | Proposed thresholds, to be fixed before each study begins |
| Browser API facts marked [verify] | Must be re-checked at implementation time |
| Cross-reference and anchor integrity, JSON examples | Reviewed by inspection; an automated link/anchor/JSON check should be added to CI (I.4) |

## I.7 Known open items

1. OQ-01…OQ-10 (§0.5) remain open and have deadlines before the milestones shown.
2. Final names of aircraft/weapons/factions are pending the IP/naming review (G8).
3. Calibration of weapons, sensors and AI tuning tables (M3–M4).
4. Final list of target languages (OQ in §53.10).
5. Legal review of datasets and music licences (Appendix H, gate G8).
6. Automated validation of document links, anchors and embedded JSON examples.

## I.8 Review checklist (before each version bump)

- [ ] Table of contents anchors resolve; every chapter has acceptance criteria.
- [ ] Every `§` reference points to an existing heading; every `AC-` id is unique.
- [ ] All JSON examples parse; all code blocks are syntactically plausible.
- [ ] Values in §0 agree with all chapters; budgets in §60 agree with §0.6.
- [ ] ADR *Affects* lists are current; new decisions are recorded.
- [ ] Outline coverage map (§0.7) lists every outline item 1–153.
- [ ] Appendix B formulas match chapter text; Appendix C numbers reproduce from C.8.
- [ ] Open questions and risk register are current.
