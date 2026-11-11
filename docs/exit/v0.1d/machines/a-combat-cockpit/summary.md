# Run a-combat-cockpit

Machine class: **recommended**. 2026-11-11. Build: https://bl4202.github.io/Flying/

- Renderer requested: `auto`; used: webgpu on worker
- Adapter: apple metal-3 · apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: ok
- Flight: not run
- COMBAT (120 s, cockpit, 1920x1080, high): PASS, sim 60.000 Hz, render 120.0 FPS
- Control latency (648 changes): avg 9.71 ms, p95 16.75 ms, p99 18.25 ms, max 20.83 ms, at 121 Hz
- Benchmark: profile high, preset ultra
- Soak: not run
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] COMBAT strict gates hold
- [x] frames render without errors or device loss
- [x] control latency p95 16.75 ms within 25 ms, over 648 changes (at least 500)
- [x] FPS at display rate (120.0 of 121 Hz)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-combat.json`.
