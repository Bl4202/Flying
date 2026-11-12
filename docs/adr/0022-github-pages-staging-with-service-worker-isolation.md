# ADR-22: GitHub Pages staging with Service-Worker-assisted isolation

- **Status:** Accepted (staging only; the world data's host is decided in ADR-50, amendment 2)
- **Owner:** Bl4202
- **Date:** 2026-10-10
- **Affects:** GDD 61.12, ADR-17, ADR-19, NN-8; `packages/platform-web`, `packages/app-shell`, `.github/workflows/ci.yml`

## Context

ADR-17 requires cross-origin isolation (COOP `same-origin`, COEP `require-corp`) for
`SharedArrayBuffer` and the threads WASM build. GitHub Pages, the staging host chosen for v0.0a,
cannot set response headers.

## Decision

- **One header source.** `packages/platform-web/src/headers.ts` defines COOP, COEP, CORP, the CSP
  and hardening headers. It feeds the Vite dev and preview servers, a generated `dist/_headers` for
  hosts that honour it, and the Service Worker.
- **The Service Worker adds the headers.** `sw.js` re-wraps every same-origin response with them
  (`withSecurityHeaders`). The first visit loads without isolation; once the worker is active the
  page reloads exactly once (guarded by `sessionStorage`) and is isolated from then on.
- **Degraded mode is real and tested.** With workers unavailable or blocked, the app runs the
  `single` build, reports "degraded", and still produces the same state hash.
- **CSP is also a `<meta>` tag**, so the first, pre-worker load is covered. `frame-ancestors` and
  similar header-only directives are not available there.
- **Trusted Types are enforced** (`require-trusted-types-for 'script'`, policy `flying-app`).
  `new Worker(...)` and `serviceWorker.register(...)` go through `scriptUrl()`, which accepts only
  same-origin URLs.
- **Update policy** (GDD 61.12): a new worker installs and waits; it never calls `skipWaiting`, so a
  mission is never interrupted. Precache is named by build id; old caches are dropped on activation.

## Alternatives considered

- *Netlify or Cloudflare Pages:* real headers, preview deploys; rejected for v0.0a by the project
  owner in favour of zero extra accounts.
- *`coi-serviceworker`-style third-party shim:* same idea, but our worker also provides the offline
  shell and must own the CSP and caching anyway.
- *Local-only staging:* would leave "staging URL serves an isolated page" unproven.

## Consequences

- No `Cache-Control: immutable`, no Brotli and no `frame-ancestors` on Pages. The worker's
  content-addressed cache covers repeat loads.
- The worker's scope is the base path (`/Flying/`), so the app must be served under one.
- A production host with real headers makes the worker's header injection redundant but harmless;
  moving hosts is a configuration change (`BASE_PATH`, deploy job), not a code change.
- The live smoke test (`tests/staging`) checks that the deployed build id equals the commit.

## Amendment 1 (2026-11-05): updates on the player's request, and a shell that never changes under a running build

*Amended by [ADR-44](0044-app-flow-settings-pwa.md).* Two things in the original policy turned out
wrong once the game had a menu to put a choice in.

- **A running build's cache was written to.** The worker answered navigations from the network first
  and stored the answer under the *running* build's cache name, so a new `index.html` could end up
  beside the old build's assets and, offline, be served with chunks that did not match it. A worker now
  answers navigations from its own build's cache, and asks the network only when it has none (a first
  visit). Nothing writes to a cache after its worker installed.
- **"Never `skipWaiting`" becomes "only when the player asks".** A new worker still installs beside the
  running one and waits. The main menu shows **Update ready — reload**; choosing it sends the waiting
  worker a `skip-waiting` message, and the page reloads when the new worker is in control. The menu is the
  only place that offers it, so a flight is never interrupted; a player who never opens the menu keeps
  the build they have, which still works offline.


## Amendment 2 (2026-11-12): the world data's host

*Amended by [ADR-50](0050-entering-m2.md).* OQ-03 is decided for M2. The app stays on GitHub Pages
under this ADR. The GeoPacks (the base pack, `world.gpk` and the region cells) are served from a
Cloudflare R2 bucket, with range requests and CORS. The bucket's CORS rule must allow the staging
origin and the `Range` header, and expose `Content-Range`. The page is isolated with COEP
`require-corp`, so it fetches the packs in CORS mode.
