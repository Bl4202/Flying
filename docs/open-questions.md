# Open questions

Imported from GDD §0.5 at v0.0a. Owner is the project owner for all of them until a team exists.
"Provisional" records what v0.0a assumed in order to proceed; it is not a decision.

| ID | Item | Needed by | Owner | Status | Provisional (v0.0a) |
|---|---|---|---|---|---|
| **OQ-01** | Confirm the stack in ADR-02 (Rust → WASM sim) and ADR-18 (Three.js WebGPURenderer + custom passes). If the team prefers TypeScript-only, the sim moves to a Worker using typed arrays; the budgets in §0.6 drop roughly 30–40 % and ADR-10's cross-build hash test is lost. | Before M1 | Bl4202 | Open | Proceeding with Rust to WASM (ADR-02) and Three.js WebGPURenderer (ADR-18). v0.0a proves the Rust/WASM half end to end; confirm before M1. |
| **OQ-02** | Team size and target release scale (solo / small team / studio). This document sizes work with relative t-shirt sizes (§66), not calendar estimates. | Before M1 planning | Bl4202 | Open | Solo. |
| **OQ-03** | Hosting and distribution: static host + CDN with range-request support and the COOP/COEP headers (assumed), who pays for tile bandwidth, and whether a publisher-hosted CDN or a community-mirror model is used. | Before M2 | Bl4202 | Open | Staging on GitHub Pages (ADR-22). Production host still open. |
| **OQ-04** | Commercial or free release. Several open datasets have attribution (CC-BY) or share-alike (ODbL) duties that interact with redistribution (§45.9). | Before M2 | Bl4202 | Open | All rights reserved (LICENSE) until decided. The repository is public. |
| **OQ-05** | Aircraft/weapon art budget: modular kit-bashed aircraft vs fully bespoke models (affects roster size at M3). Browser asset budget (§60.6) also limits fidelity. | Before M3 | Bl4202 | Open |  |
| **OQ-06** | In-game voice chat. The browser provides WebRTC audio, so this is cheap (assumed on, optional per lobby, §32.9). | Before M5 | Bl4202 | Open |  |
| **OQ-07** | WebXR/VR support (not in outline; designed-for-compatibility only, §58.8). | Before M3 | Bl4202 | Open |  |
| **OQ-08** | Target content rating and regional constraints on map/border depiction (§9.8). | Before public release | Bl4202 | Open |  |
| **OQ-09** | Whether a desktop wrapper (Electron/Tauri) is wanted later for storefront distribution. The architecture keeps this possible and does not depend on it. | Before M5 | Bl4202 | Open |  |
| **OQ-10** | Community hub scope and moderation for user missions (§51.4): hosted service vs files/links only; moderation policy; abuse handling. | Before M7 | Bl4202 | Open |  |
