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
| L | Mac M4, WebGPU, the control-latency gate | Recommended | `m4-latency` | scripted by the maintainer (v0.1b) |
| A-arena | Mac M4, WebGPU, the arena scenario in the cockpit and in the chase view (ADR-39 P1, P2) | Recommended | `a-arena-cockpit`, `a-arena-chase` | scripted by the maintainer (v0.1c) |
| B-arena | Mac M4, `renderer=webgl2`, 4x CPU throttle, the arena scenario (P3) | Minimum, emulated | `b-arena` | scripted by the maintainer (v0.1c) |

Reports go to `docs/exit/v0.0b/machines/<folder>/` for v0.0b, to `docs/exit/v0.1b/machines/<folder>/` for v0.1b
and to `docs/exit/v0.1c/machines/<folder>/` from v0.1c (the scripted route's default).

## Before any run

- Current **stable** Chrome (not Canary, not a beta), up to date. Check `chrome://gpu`: for WebGPU
  runs, *WebGPU* must say "Hardware accelerated"; for WebGL2 runs, *WebGL2* must.
- On a laptop: **plugged in** (a run on battery is not a reference run; at 10 % charge the machine
  may also throttle or sleep), the power mode on "best performance", display at its native refresh
  rate.
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
pnpm perf:local --name m4-latency --class recommended --latency 500   # the AC-58.3 gate (v0.1b)
pnpm perf:local --name a-arena-cockpit --class recommended --scenario arena           # P1 (v0.1c)
pnpm perf:local --name a-arena-chase --class recommended --scenario arena --camera chase   # P2
pnpm perf:local --name b-arena --class minimum --scenario arena --renderer webgl2 --throttle 4   # P3
```

The arena runs need **mains power**: a headed run at 1920 x 1080 with 4x MSAA is the heaviest thing the game
does, and a laptop on battery or in low-power mode is not a reference machine. `--base http://127.0.0.1:4173/Flying/`
measures a local `pnpm preview` of the build (the summary records the base), for a run before staging has the
build. The screenshot gates on the reference GPU (S1 to S5 on WebGPU, ADR-39) are a separate command:

```sh
pnpm build:web
SCREENS_RENDERER=webgpu PW_CHANNEL=chrome CI=1 pnpm exec playwright test tests/e2e/horizon.spec.ts \
  tests/e2e/hud.spec.ts tests/e2e/terrain.spec.ts --project=e2e --retries=0
```

It opens real Chrome (headed), records the compat verdict, flies a scripted minute (from the air
under Standard assist, `--spawn air`; the runway start is `--spawn runway`), takes the
control-latency samples if `--latency N` is given, runs the full benchmark and PS-0 for 120 s, and
writes `compat.json`, `flight.json`, `flight.png`, `latency.json`, `benchmark.json`,
`perf-ps0.json` (`perf-arena.json` with `--scenario arena`, which skips the scripted minute unless
`--fly` says otherwise) and `summary.md` under `docs/exit/v0.1c/machines/<name>/`. The summary lists each
pass criterion. Options: `--base URL` (default the Pages build), `--seconds`, `--fly`, `--spawn`,
`--latency`, `--renderer`, `--throttle`, `--headless`, `--out`, `--scenario ps0|arena`, `--camera`, `--res`,
`--quality`. Leave the machine alone while it
runs and keep the game tab visible.

## Pass criteria, per run

- The compat verdict is not fatal.
- PS-0's strict gates hold for the whole 120 s: sim rate 60.0 +- 0.1 Hz, nothing dropped, tick cost
  p99 at most 7.0 ms and average at most 4.0 ms, render CPU average at most 7.0 ms. An arena run adds the
  frame interval's average (at most 16.7 ms) and p99 (at most 22.2 ms), the render CPU's p99 (11.0 ms), no
  lost device and no main-thread task over 50 ms (`docs/perf/README.md`).
- Frames render without errors or device loss (no console errors, no "graphics device was lost"
  notice).
- At least 30 FPS on Minimum-class runs (B, D); on the others, the display rate (95 % of what
  `requestAnimationFrame` runs at).

- With `--latency N` (AC-58.3, ADR-33 section 7): at least N changes measured (500 for the gate) and
  the control latency's p95 at most **25 ms**. The display rate is recorded in the summary; the
  sim counts only frames that carry a change, from the key event to the end of the tick that acted
  on it. Run it with the staging build, headed, on power.

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

## The owner's look-and-listen checklist (v0.1c, not blocking)

The bots gate the exit; the owner's own look is recorded and does not block the tag. On the staging build,
in a stable headed Chrome with sound on, note what you saw and heard:

1. **First visit** (a fresh profile or a private window): the compat screen, then the first-run steps in
   order (try it with the keyboard alone), then the main menu. A second visit goes straight to the menu.
2. **Fly** in each arena (Rugged, Plain) and **Main menu**, then Fly again: a clean sortie each time.
3. **Cockpit and chase** (`C`): head look (`,` `.` `ArrowUp` `ArrowDown`, `X` with the mouse), zoom (right
   button), `/` to recentre. The chase camera should sit behind and above the aircraft at every speed.
4. **The HUD**: readable on the runway, in the climb and on final; `I` for the detail levels, `-` and `=` for
   brightness. Is anything out of place against the horizon?
5. **The horizon**: climb to 10 km (the overlay with `?debug=1` shows the height) and look: the dip should
   look right, the edge crisp, no gaps in the ground.
6. **Sound**: engine at idle, military and afterburner (the light-off thump); wind rising with speed; the
   stall horn, the over-G tone and PULL UP; the gear chime. Anything harsh, clipped or too quiet?
7. **Settings** (all six tabs), a change that persists over a reload, export and import of the settings file.
8. **Install** (the menu's Install button, or the browser's), then **offline**: close the tab, go offline
   and open the installed app; it should start and fly. With a newer build deployed, "Update ready" appears in
   the menu, never in flight.

Record per item what you saw or heard and anything that surprised you. Gamepad and HOTAS look are not built;
the owner's devices are not tested for the view keys.

## The owner's manual circuit (v0.1b, not blocking)

The virtual tester gates the exit (`cargo xtask circuit`, ADR-33); the owner's own flights are
recorded and do not block the tag. On the staging build with a keyboard and mouse, once per assist
level (pause menu, `Esc`, then the level; or `?assist=full|standard|reduced|manual`), note what
happened:

1. Runway start: parking brake off (`P`), full afterburner (`Z`), keep the centreline with `Q`/`E`.
2. Rotate at about 70 m/s, gear up (`L`) once climbing, flaps up (`V`) by 120 m/s.
3. Fly a left-hand circuit at about 300 m above the field.
4. Gear down (`L`), flaps to landing (`V` twice), airbrake (`N`), about 3 degrees on final.
5. Flare, touch down, brake (`B`), stop on the runway.
6. Also once: mouse-aim (`O`, click the picture), a hard pull until the screen greys out, and
   rebinding one key in the pause menu's Controls and reloading.

Record per level: whether it flew, anything that surprised you (a control too sensitive, a warning
at the wrong moment, a crash and its cause), and the display rate. Gamepad and HOTAS are not
hand-verified for v0.1b (no hardware).
