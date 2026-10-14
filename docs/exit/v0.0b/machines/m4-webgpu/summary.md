# Run m4-webgpu

Machine class: **recommended**. 2026-10-14. Build: https://bl4202.github.io/Flying/

- Renderer requested: `auto`; used: webgpu on worker apple metal-3
- Adapter: apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 122 Hz
- Compat verdict: ok
- Flight (60 s scripted): 120.0 FPS average, 110.2 minimum, frame p99 9.5 ms, longest frame 10.6 ms, sim 60.01 Hz
- PS-0 (120 s): PASS, sim 60.000 Hz, render 120.1 FPS
- Benchmark: profile high, preset ultra
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] PS-0 strict gates hold
- [x] frames render without errors or device loss
- [x] FPS at display rate (120.1 of 122 Hz)

Raw reports: `compat.json`, `flight.json`, `benchmark.json`, `perf-ps0.json`.
