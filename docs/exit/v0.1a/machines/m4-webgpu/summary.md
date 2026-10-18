# Run m4-webgpu

Machine class: **recommended**. 2026-10-18. Build: https://bl4202.github.io/Flying/

- Renderer requested: `auto`; used: webgpu on worker apple metal-3
- Adapter: apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: ok
- Flight (60 s scripted): 120.0 FPS average, 119.5 minimum, frame p99 10.3 ms, longest frame 18.6 ms, sim 60.02 Hz
- PS-0 (120 s): PASS, sim 59.999 Hz, render 120.1 FPS
- Benchmark: profile high, preset ultra
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] PS-0 strict gates hold
- [x] frames render without errors or device loss
- [x] FPS at display rate (120.1 of 121 Hz)

Raw reports: `compat.json`, `flight.json`, `benchmark.json`, `perf-ps0.json`.
