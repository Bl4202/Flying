# ADR-19: Storage and offline

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §45, §64

## Decision

**Storage and offline: Service Worker + Cache Storage for the app shell, Origin Private File System (OPFS) for tile archives/replays/large blobs, IndexedDB for structured saves and settings; persistent storage is requested.** Everything needed to fly a previously cached region works with no network.

## Alternatives considered

Server-side saves; localStorage

## Rationale

Offline play is a non-negotiable (NN-8). Browser storage can be evicted — see §64.6 (export/import and persistence request).
