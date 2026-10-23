# Architecture decision records

ADR-01 to ADR-20 were imported verbatim from GDD §0.4 at v0.0a. ADR-21 and later are written as decisions are made (ADR-21 to ADR-23 in v0.0a, ADR-24 to ADR-28 in v0.0b, ADR-29 to ADR-31 in v0.1a, ADR-32 onwards in v0.1b).
To add one, copy [TEMPLATE.md](TEMPLATE.md), take the next number, and add it below. A gate threshold change needs an ADR (AC-66.4).

| ID | Title | Status |
|---|---|---|
| [ADR-01](0001-delivery-platform-browser-and-pwa.md) | Delivery platform: browser and PWA | Accepted |
| [ADR-02](0002-sim-core-in-rust-compiled-to-webassembly.md) | Sim Core in Rust compiled to WebAssembly | Accepted |
| [ADR-03](0003-64-bit-bf-simulation-state-camera-relative-rende.md) | 64-bit BF simulation state, camera-relative rendering | Accepted |
| [ADR-04](0004-curved-earth-battlefield-frame.md) | Curved-Earth battlefield frame | Accepted |
| [ADR-05](0005-msl-as-ellipsoid-vertical-datum.md) | MSL-as-ellipsoid vertical datum | Accepted |
| [ADR-06](0006-geodetic-quadtree-terrain-tiling.md) | Geodetic quadtree terrain tiling | Accepted |
| [ADR-07](0007-fixed-60-hz-simulation-tick.md) | Fixed 60 Hz simulation tick | Accepted |
| [ADR-08](0008-server-authoritative-networking.md) | Server-authoritative networking | Accepted |
| [ADR-09](0009-snapshot-based-replays.md) | Snapshot-based replays | Accepted |
| [ADR-10](0010-determinism-capable-core.md) | Determinism-capable core | Accepted |
| [ADR-11](0011-original-and-fictional-content.md) | Original and fictional content | Accepted |
| [ADR-12](0012-open-data-first-geodata.md) | Open-data-first geodata | Accepted |
| [ADR-13](0013-content-is-versioned-data.md) | Content is versioned data | Accepted |
| [ADR-14](0014-campaign-calls-the-mission-generator.md) | Campaign calls the mission generator | Accepted |
| [ADR-15](0015-terrain-queries-use-the-base-dem-only.md) | Terrain queries use the base DEM only | Accepted |
| [ADR-16](0016-sensors-and-ai-never-read-the-render-world.md) | Sensors and AI never read the render world | Accepted |
| [ADR-17](0017-four-role-threading-model.md) | Four-role threading model | Accepted |
| [ADR-18](0018-reference-renderer-stack.md) | Reference renderer stack | Accepted |
| [ADR-19](0019-storage-and-offline.md) | Storage and offline | Accepted |
| [ADR-20](0020-dom-for-chrome-gpu-for-hud.md) | DOM for chrome, GPU for HUD | Accepted |
| [ADR-21](0021-toolchains-and-wasm-threads-build.md) | Toolchains and the WASM threads build | Accepted |
| [ADR-22](0022-github-pages-staging-with-service-worker-isolation.md) | GitHub Pages staging with Service-Worker-assisted isolation | Accepted |
| [ADR-23](0023-state-hash-and-golden-policy.md) | State hash algorithm and golden-file policy | Accepted |
| [ADR-24](0024-thin-custom-renderer.md) | A thin custom renderer over raw WebGPU and WebGL2 | Accepted |
| [ADR-25](0025-runtime-topology-ipc-and-abi-v2.md) | Runtime topology, IPC formats and ABI v2 | Accepted |
| [ADR-26](0026-sim-loop-pacing.md) | Simulation loop pacing | Accepted |
| [ADR-27](0027-seams-policy-and-own-math-types.md) | Seams policy and the project's own vector types | Accepted |
| [ADR-28](0028-perf-gating-and-reference-machines.md) | Performance gating and the reference-machine waiver | Accepted |
| [ADR-29](0029-flight-crates-and-integration.md) | Flight crates, the flight environment and the integration scheme | Accepted |
| [ADR-30](0030-aircraft-data-format-and-cpack-v2.md) | Aircraft data format, cpack v2 and the post-stall blend | Accepted |
| [ADR-31](0031-performance-conformance-test-method.md) | Performance Conformance Test method and gates | Accepted |
| [ADR-32](0032-ground-roll-conformance-method.md) | Ground-roll conformance method and the landing-roll finding | Accepted |
| [ADR-33](0033-v0-1b-gates.md) | v0.1b gates: circuit, FCS handling, departure, mouse-aim and latency | Accepted |

ADR-31 was written before ADR-29 and ADR-30 because its gates had to exist before any tuning
(AC-66.4).

ADR-32 and ADR-33 were written first in v0.1b, before any gear or FCS code, for the same reason
(AC-66.4). ADR-34 to ADR-38 (FCS, ground model and test airfield, session config / ABI v3 / input
v3 / snapshot v3, controls stack, G-physiology and departure) are added with their work packages.
