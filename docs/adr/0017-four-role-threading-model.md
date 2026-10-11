# ADR-17: Four-role threading model

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §33, §60, §61

## Decision

**Threading model: four roles.** *Main thread* (DOM UI, input, WebRTC, audio-graph control); *Sim Worker* (WASM Sim Core, owns authoritative state); *Render Worker* (WebGPU on `OffscreenCanvas`, main-thread fallback where unsupported [verify]); *Stream Workers* pool (fetch, decompress, decode, mesh build). State moves between them through `SharedArrayBuffer` ring buffers and seqlock-protected snapshot buffers. This **requires cross-origin isolation** (`Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp`), so every tile/asset origin MUST send `Cross-Origin-Resource-Policy`/CORS headers. A **degraded single-worker mode** (no SharedArrayBuffer, `postMessage` transfer of snapshots, reduced caps) exists for hosts that cannot set the headers.

## Alternatives considered

Everything on the main thread; WASM threads for all work

## Rationale

Keeps UI responsive, isolates sim cost from render cost, and lets the 60 Hz sim run independent of `requestAnimationFrame` throttling. `RTCPeerConnection` is generally unavailable inside Workers [verify], hence networking sits on the main thread and forwards datagrams to the Sim Worker.
