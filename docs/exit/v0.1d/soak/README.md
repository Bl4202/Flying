# v0.1d soak reports

Each folder holds one `soak_report/1` written by `tests/e2e/soak/soak.spec.ts` (gates L1 to L7 of
ADR-45). All runs are against staging on the merge-1 commit `261d08b`, with the soak's page
`?debug=1&fly=1&scenario=duel&skill=2&script=sortie&res=640x360&quality=low`.

| Folder | Where | Settings | Verdict |
|---|---|---|---|
| `m4/` | Mac M4, headed Chrome, WebGPU `apple metal-3`, mains | `SOAK_SECONDS=1800` after the 120 s warm-up | L1 to L7 pass (60.02 Hz, 7 sortie starts) |
| `m4-first/` | the same | the same, before the harness fix of ADR-45 amendment 2 | L1 to L6 pass; L7 fails (56.24 Hz, two samples taken while the debrief held the sim) |
| `ci-nightly/` | CI `perf-nightly`, run 43009393657 | `SOAK_SECONDS=1800` | L1 to L7 pass |
| `fault-leak-js/` | Mac M4 | `fault=leak-js`, `SOAK_SECONDS=300` | L1 fails, as it must |
| `fault-leak-gpu/` | Mac M4 | `fault=leak-gpu`, `SOAK_WARMUP_SECONDS=0`, `SOAK_SECONDS=900` | L5 fails, as it must |

The two fault runs used the harness from before the fix, so their L7 failures come from the same
cause as `m4-first/`.

Command for the full M4 run:

```sh
PW_CHANNEL=chrome SOAK_BASE_URL=https://bl4202.github.io/Flying/ SOAK_SECONDS=1800 \
  SOAK_REPORT_DIR=docs/exit/v0.1d/soak/m4 pnpm soak --headed
```
