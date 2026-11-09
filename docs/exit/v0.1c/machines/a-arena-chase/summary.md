# Run a-arena-chase

Machine class: **recommended**. 2026-11-09. Build: http://127.0.0.1:4173/Flying/

- Renderer requested: `auto`; used: webgpu on worker
- Adapter: apple metal-3 · apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: ok
- Flight: not run
- ARENA (120 s, chase, 1920x1080, high): PASS, sim 60.000 Hz, render 120.0 FPS
- Control latency: not measured
- Benchmark: profile high, preset ultra
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] ARENA strict gates hold
- [x] frames render without errors or device loss
- [x] FPS at display rate (120.0 of 121 Hz)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-arena.json`.
