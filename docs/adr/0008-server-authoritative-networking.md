# ADR-08: Server-authoritative networking

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §33, §32, §31

## Decision

**Networking: server-authoritative, with client-side prediction of the locally owned aircraft and snapshot interpolation of everything else. No lockstep.** Transports, in preference order: **WebRTC DataChannel** (unordered, `maxRetransmits: 0`) → **WebTransport datagrams** [verify support] → **WebSocket** (TCP; LAN-only fallback). The authority is either a browser tab hosting the sim in a Worker ("listen server") or the native dedicated-server build of the same Sim Core. Raw UDP is not available to web pages.

## Alternatives considered

Peer-to-peer ownership; deterministic lockstep

## Rationale

AI-heavy battles make lockstep input delay unacceptable; authoritative hits/launches are required by the outline (§65). WebRTC DataChannels are the only browser-native unreliable transport usable between two pages with no server.
