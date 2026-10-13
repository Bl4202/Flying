# Staging runbook

Staging URL: https://bl4202.github.io/Flying/ (GitHub Pages, project site for `Bl4202/Flying`).

## How a deploy happens

1. A push to `main` runs `.github/workflows/ci.yml`.
2. `lint`, `test`, `build`, `determinism`, `determinism-arm64`, `repro`, `e2e` and `perf-smoke` must
   all pass. (`e2e-compat`, the Firefox and WebKit boot check, runs too but does not block.)
3. `deploy-staging` downloads the `built` artifact (`dist/` from the `build` job), uploads it as the
   Pages artifact and deploys it. Nothing is rebuilt at deploy time: what was tested is what ships.
4. `staging-smoke` loads the live URL in Chromium and checks that the deployed build id equals the
   commit, that the page becomes cross-origin isolated, runs the threads build and reports the
   golden state hash (the `?selftest=1` page). It then opens the game itself and checks that the sim
   ticks, frames are drawn and the picture is not blank. It polls for up to 3 minutes because Pages
   can serve the previous deploy for a short while.

Pull requests run everything except the two deploy jobs.

## One-time setup

Repository Settings, Pages, Source: **GitHub Actions**. (CLI: `gh api -X POST repos/Bl4202/Flying/pages -f build_type=workflow`.)
Branch protection on `main` should require the eight check jobs above.

A nightly workflow (`perf-nightly.yml`) builds `main` and runs PS-0 for two minutes; the report is
kept as an artifact. It never fails the build (ADR-28).

## How isolation works on Pages

GitHub Pages cannot send `Cross-Origin-Opener-Policy` / `Cross-Origin-Embedder-Policy`, which
`SharedArrayBuffer` and the threads WASM build need. The Service Worker (`sw.js`) adds them to every
same-origin response (ADR-22). So:

- First visit: the page loads un-isolated, registers the worker, and reloads once when it takes over.
- Later visits: isolated from the first byte.
- If workers are unavailable or blocked the app shows **Mode: degraded** and runs the single-thread
  build. That is expected, not an error.

To check by hand, open the staging URL with `?selftest=1` and read the status table:
`Cross-origin isolated: true`, `Mode: full`, `WASM variant: threads`, `Build` equal to the commit
SHA. The plain URL starts the game (a compat screen first, on the first visit); `?debug=1` adds the
overlay, whose `Mode` row reads `threads · shared · atomics` when isolated.

## Rolling back

Re-run the **deploy-staging** job of an earlier successful run (Actions, pick the run, "Re-run
jobs"). Its artifact is retained for 7 days. After that, revert on `main` and let the pipeline
redeploy.

A user with the old Service Worker keeps the old shell until their next launch (new workers wait,
GDD 61.12); a hard refresh with DevTools "Update on reload" forces it.

## Moving to a host that can set headers

1. Build with `BASE_PATH` set for the new host (default is `/Flying/`).
2. `dist/_headers` is already generated from the same header source; Netlify and Cloudflare Pages
   read it as is. Other hosts: copy the values from `packages/platform-web/src/headers.ts`.
3. Replace the deploy job. Keep `staging-smoke`; point it at the new URL.
4. The Service Worker keeps working; its header injection becomes redundant but harmless.
5. Record the decision as an ADR and close OQ-03 if this is the production host.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| Page shows `Mode: degraded` on a normal browser | Service Worker blocked (private window, site data blocked) or the page was opened as `file://` |
| Page reloads once on first visit | Normal: the worker takes over and the page reloads to become isolated |
| `staging-smoke` times out on the build id | Pages CDN lag, or the deploy job was skipped; check the Pages environment history |
| Blank page, console shows a Trusted Types or CSP error | A script URL went around `scriptUrl()`; see `packages/platform-web/src/trusted-types.ts` |
| `cargo xtask determinism` reports a divergence | Read the first diverging tick and subsystem; see ADR-23 |
