# Run m4-latency

Machine class: **recommended**. 2026-10-28. Build: http://127.0.0.1:4174/Flying/

- Renderer requested: `auto`; used: webgpu on worker
- Adapter: apple metal-3 · apple metal-3
- CPU throttling: none
- Display rate (requestAnimationFrame): 121 Hz
- Compat verdict: ok
- Flight (60 s scripted, air start): 120.0 FPS average, 119.1 minimum, frame p99 10.4 ms, longest frame 10.4 ms, sim 59.99 Hz
- PS-0 (120 s): PASS, sim 59.999 Hz, render 120.0 FPS
- Control latency (567 changes): avg 9.22 ms, p95 17.25 ms, p99 22.00 ms, max 24.32 ms, at 121 Hz
- Benchmark: profile minimum, preset medium
- Console errors: none

## Pass criteria: PASS

- [x] compat verdict is not fatal (ok)
- [x] PS-0 strict gates hold
- [x] frames render without errors or device loss
- [x] control latency p95 17.25 ms within 25 ms, over 567 changes (at least 500)
- [x] FPS at display rate (120.0 of 121 Hz)

Raw reports: `compat.json`, `flight.json`, `latency.json`, `benchmark.json`, `perf-ps0.json`.
