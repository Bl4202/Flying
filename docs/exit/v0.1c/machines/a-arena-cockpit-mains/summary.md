# Run a-arena-cockpit-mains

Machine class: **recommended**. 2026-11-09. Build: https://bl4202.github.io/Flying/

- Renderer requested: `auto`; used: webgpu on worker
- Adapter: apple metal-3 · apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: ok
- Flight: not run
- ARENA (120 s, cockpit, 1920x1080, high): PASS, sim 60.001 Hz, render 120.0 FPS
- Control latency (631 changes): avg 9.39 ms, p95 17.25 ms, p99 21.25 ms, max 23.40 ms, at 121 Hz
- Benchmark: profile high, preset ultra
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] ARENA strict gates hold
- [x] frames render without errors or device loss
- [x] control latency p95 17.25 ms within 25 ms, over 631 changes (at least 500)
- [x] FPS at display rate (120.0 of 121 Hz)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-arena.json`.
