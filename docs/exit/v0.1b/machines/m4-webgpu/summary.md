# Run m4-webgpu

Machine class: **recommended**. 2026-10-28. Build: http://127.0.0.1:4174/Flying/

- Renderer requested: `auto`; used: webgpu on worker
- Adapter: apple metal-3 · apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: ok
- Flight (60 s scripted, air start): 120.0 FPS average, 119.5 minimum, frame p99 9.5 ms, longest frame 10.6 ms, sim 60.03 Hz
- PS-0 (120 s): PASS, sim 60.000 Hz, render 120.0 FPS
- Control latency: not measured
- Benchmark: profile minimum, preset medium
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] PS-0 strict gates hold
- [x] frames render without errors or device loss
- [x] FPS at display rate (120.0 of 121 Hz)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-ps0.json`.
