# Run a-arena-cockpit

Machine class: **recommended**. 2026-11-09. Build: http://127.0.0.1:4173/Flying/

- Renderer requested: `auto`; used: webgpu on worker
- Adapter: apple metal-3 · apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 122 Hz
- Compat verdict: ok
- Flight: not run
- ARENA (120 s, cockpit, 1920x1080, high): PASS, sim 60.000 Hz, render 120.0 FPS
- Control latency (600 changes): avg 9.83 ms, p95 16.75 ms, p99 19.00 ms, max 22.05 ms, at 122 Hz
- Benchmark: profile high, preset ultra
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] ARENA strict gates hold
- [x] frames render without errors or device loss
- [x] control latency p95 16.75 ms within 25 ms, over 600 changes (at least 500)
- [x] FPS at display rate (120.0 of 122 Hz)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-arena.json`.
