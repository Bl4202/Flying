# Run m4-webgl2-throttled

Machine class: **minimum**. 2026-10-28. Build: http://127.0.0.1:4174/Flying/

- Renderer requested: `webgl2`; used: webgl2 on worker
- Adapter: apple metal-3 · ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Ve
- CPU throttling: 4x (main thread only; workers are not throttled)
- Display rate (requestAnimationFrame): 118 Hz
- Compat verdict: degraded
- Flight (60 s scripted, air start): 120.0 FPS average, 119.1 minimum, frame p99 10.4 ms, longest frame 10.4 ms, sim 60.00 Hz
- PS-0 (120 s): PASS, sim 60.002 Hz, render 120.0 FPS
- Control latency: not measured
- Benchmark: profile minimum, preset medium
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (degraded)
- [x] PS-0 strict gates hold
- [x] frames render without errors or device loss
- [x] at least 30 FPS on a Minimum-class run (120.0)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-ps0.json`.
