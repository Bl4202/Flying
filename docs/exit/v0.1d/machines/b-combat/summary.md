# Run b-combat

Machine class: **minimum**. 2026-11-11. Build: https://bl4202.github.io/Flying/

- Renderer requested: `webgl2`; used: webgl2 on worker
- Adapter: apple metal-3 · ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Ve
- CPU throttling: 4x (main thread only; workers are not throttled)
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: degraded
- Flight: not run
- COMBAT (120 s, cockpit, 1920x1080, high): PASS, sim 59.999 Hz, render 121.5 FPS
- Control latency: not measured
- Benchmark: profile high, preset ultra
- Soak: not run
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (degraded)
- [x] COMBAT strict gates hold
- [x] frames render without errors or device loss
- [x] at least 30 FPS on a Minimum-class run (121.5)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-combat.json`.
