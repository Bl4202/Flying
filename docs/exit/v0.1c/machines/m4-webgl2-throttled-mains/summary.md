# Run m4-webgl2-throttled-mains

Machine class: **minimum**. 2026-11-09. Build: https://bl4202.github.io/Flying/

- Renderer requested: `webgl2`; used: webgl2 on worker
- Adapter: apple metal-3 · ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Ve
- CPU throttling: 4x (main thread only; workers are not throttled)
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: degraded
- Flight (60 s scripted, air start): 120.0 FPS average, 119.5 minimum, frame p99 9.4 ms, longest frame 9.4 ms, sim 60.00 Hz
- PS-0 (120 s): PASS, sim 60.001 Hz, render 120.0 FPS
- Control latency: not measured
- Benchmark: profile high, preset ultra
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (degraded)
- [x] PS-0 strict gates hold
- [x] frames render without errors or device loss
- [x] at least 30 FPS on a Minimum-class run (120.0)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-ps0.json`.
