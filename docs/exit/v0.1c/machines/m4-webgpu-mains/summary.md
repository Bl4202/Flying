# Run m4-webgpu-mains

Machine class: **recommended**. 2026-11-09. Build: https://bl4202.github.io/Flying/

- Renderer requested: `auto`; used: webgpu on worker
- Adapter: apple metal-3 · apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: ok
- Flight (60 s scripted, air start): 120.0 FPS average, 119.4 minimum, frame p99 9.5 ms, longest frame 14.0 ms, sim 59.98 Hz
- PS-0 (120 s): PASS, sim 60.001 Hz, render 120.1 FPS
- Control latency: not measured
- Benchmark: profile high, preset ultra
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] PS-0 strict gates hold
- [x] frames render without errors or device loss
- [x] FPS at display rate (120.1 of 121 Hz)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-ps0.json`.
