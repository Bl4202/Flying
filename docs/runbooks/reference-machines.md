# Reference-machine runs

v0.0b's exit is that the sim holds 60 Hz in its worker while the renderer runs independently, on
the reference machines. This runbook is how a run is made and what counts as a pass. The reasoning,
and what each run only approximates, is ADR-28.

| Run | Machine | Class | Name (folder) | Who |
|---|---|---|---|---|
| A | Mac M4, WebGPU | Recommended | `m4-webgpu` | scripted by the maintainer |
| B | Mac M4, `renderer=webgl2`, 4x CPU throttle | Minimum, emulated | `m4-webgl2-throttled` | scripted by the maintainer |
| C | Laptop, RTX 5060 | High | `rtx5060-webgpu` | project owner |
| D | Laptop, Chrome forced onto the integrated GPU | Minimum-class GPU | `rtx5060-igpu` | project owner |

Reports go to `docs/exit/v0.0b/machines/<folder>/`.

## Before any run

- Current **stable** Chrome (not Canary, not a beta), up to date. Check `chrome://gpu`: for WebGPU
  runs, *WebGPU* must say "Hardware accelerated"; for WebGL2 runs, *WebGL2* must.
- On a laptop: plugged in, the power mode on "best performance", display at its native refresh rate.
- Close other tabs and heavy applications. Keep the game tab **visible** for the whole run: a hidden
  tab pauses the simulation by design, and the run would fail.
- Run D only: Windows Settings, System, Display, Graphics, add Chrome, Options, **Power saving**
  (the integrated GPU), then restart Chrome. Open the game with `?debug=1` and read the overlay: the
  `Adapter` and `Device` rows must name the integrated GPU (Intel or AMD), not the NVIDIA one.

## The manual route (no checkout needed)

1. Open `https://bl4202.github.io/Flying/?debug=1`. Note what the compat screen says (and press
   Continue). The overlay's `Adapter` and `Device` rows say which GPU is in use.
2. Fly for a minute (`W`/`S` pitch, `A`/`D` roll, `Q`/`E` yaw, `Shift` throttle up, `Page Down`
   throttle down, `R` resets). Look for stutter, tearing, a black or flickering picture, and any
   notice at the bottom of the screen (a lost graphics device raises one).
3. Press **Benchmark** in the overlay (it opens its own page and takes about 25 s). When it
   finishes, press **Copy report** and paste it into a file called `benchmark.json`.
4. Open `https://bl4202.github.io/Flying/?perf=ps0&seconds=120` and leave the tab alone for two
   minutes and a few seconds. When the status says "Finished.", press **Copy report** and paste it
   into `perf-ps0.json`. The verdict line says PASS or FAIL and lists the gates that failed.
5. Send the two files, the compat verdict, the adapter lines, and anything you noticed in step 2.

## The scripted route (needs the repository, Node and Chrome)

```sh
pnpm install --frozen-lockfile
pnpm perf:local --name rtx5060-webgpu --class high
pnpm perf:local --name rtx5060-igpu --class minimum          # run D, after the Windows setting
pnpm perf:local --name m4-webgl2-throttled --class minimum --renderer webgl2 --throttle 4
```

It opens real Chrome (headed), records the compat verdict, flies a scripted minute, runs the full
benchmark and PS-0 for 120 s, and writes `compat.json`, `flight.json`, `flight.png`,
`benchmark.json`, `perf-ps0.json` and `summary.md` under `docs/exit/v0.0b/machines/<name>/`. The
summary lists each pass criterion. Options: `--base URL` (default the Pages build), `--seconds`,
`--fly`, `--renderer`, `--throttle`, `--headless`, `--out`. Leave the machine alone while it runs.

## Pass criteria, per run

- The compat verdict is not fatal.
- PS-0's strict gates hold for the whole 120 s: sim rate 60.0 +- 0.1 Hz, nothing dropped, tick cost
  p99 at most 7.0 ms and average at most 4.0 ms, render CPU average at most 7.0 ms.
- Frames render without errors or device loss (no console errors, no "graphics device was lost"
  notice).
- At least 30 FPS on Minimum-class runs (B, D); on the others, the display rate (95 % of what
  `requestAnimationFrame` runs at).

The benchmark's own classification is reported but is not a pass criterion: its thresholds are
provisional and these reports are what they will be tuned against.

## What the approximations are

- **B** is a Minimum-like renderer path (WebGL2) with a slowed main thread. DevTools CPU throttling
  may not reach workers, so the sim and the Render Worker may run at full speed: B proves the WebGL2
  path under load, not a slow CPU.
- **D** has a real Minimum-class GPU, but a faster CPU and more memory than the Minimum.
- True Minimum hardware (4 cores, 8 GB, integrated GPU) is an open item under a waiver (ADR-28).

## When something fails

| Symptom | Look at |
|---|---|
| PS-0 `sim-rate` fails by a little, once | a background task; repeat the run on an idle machine before drawing conclusions |
| `dropped` is not zero | a stall during the window: check for thermal throttling, a battery-saver mode, another app; the overlay's `Dropped` row shows when |
| `render-cpu` fails | the renderer is spending CPU in `render`; the GPU is not the problem. Try `renderer=webgl2` to compare |
| FPS is half the display rate | the page is on the wrong GPU, or the tab is throttled; read the `Device` row |
| Compat verdict fatal, `SYS-103` | hardware acceleration is off, or the GPU is on the browser's blocklist (`chrome://gpu`) |
| A "graphics device was lost" notice | note when it happened; the renderer rebuilds once, then falls back to WebGL2. Send the console log |
