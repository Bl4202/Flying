# Run b-arena

Machine class: **minimum**. 2026-11-09. Build: http://127.0.0.1:4173/Flying/

- Renderer requested: `webgl2`; used: webgl2 on worker
- Adapter: apple metal-3 · ANGLE (Apple, ANGLE Metal Renderer: Apple M4, Unspecified Ve
- CPU throttling: 4x (main thread only; workers are not throttled)
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: degraded
- Flight: not run
- ARENA (120 s, cockpit, 1920x1080, high): PASS, sim 60.000 Hz, render 120.0 FPS
- Control latency: not measured
- Benchmark: profile high, preset ultra
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (degraded)
- [x] ARENA strict gates hold
- [x] frames render without errors or device loss
- [x] at least 30 FPS on a Minimum-class run (120.0)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-arena.json`.
