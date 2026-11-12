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
| A-combat, L | Mac M4, WebGPU, the COMBAT scenario in the cockpit, and the control latency with a duel running (ADR-45 PF1, PF2) | Recommended | `a-combat-cockpit` | scripted by the maintainer (v0.1d) |
| B-combat | Mac M4, `renderer=webgl2`, 4x CPU throttle, the COMBAT scenario (PF3) | Minimum, emulated | `b-combat` | scripted by the maintainer (v0.1d) |
| Soak | Mac M4, WebGPU, headed, the 30-minute soak (L1 to L7) | Recommended | `docs/exit/v0.1d/soak/m4/` | scripted by the maintainer (v0.1d) |

Reports go to `docs/exit/v0.0b/machines/<folder>/` for v0.0b, to `docs/exit/v0.1b/machines/<folder>/` for v0.1b,
to `docs/exit/v0.1c/machines/<folder>/` for v0.1c and to `docs/exit/v0.1d/machines/<folder>/` from v0.1d (the
scripted route's default).

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
pnpm perf:local --name a-combat-cockpit --class recommended --scenario combat --latency 500   # PF1, PF2 (v0.1d)
pnpm perf:local --name b-combat --class minimum --scenario combat --renderer webgl2 --throttle 4  # PF3
SOAK_SECONDS=1800 SOAK_BASE_URL=https://bl4202.github.io/Flying/ SOAK_REPORT_DIR=docs/exit/v0.1d/soak/m4 \
  PW_CHANNEL=chrome pnpm soak --headed                                    # the soak, 32 minutes
```

With `--scenario combat` the control latency is measured with a duel running (the merge against a
Regular), so one command makes runs A and L. The soak flies the bot's sortie from the runway against a
Regular again and again; leave the machine alone, on mains, with the window visible, for its 32 minutes
(`docs/perf/README.md` has its gates and the seeded leak faults).

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

## The owner's M1 session (not blocking)

The four hands-on checklists of M1, in one sitting of about 90 minutes. Before 2026-11-12 they were
four lists, one per sub-milestone. The bots gate every one of those milestones; what you see and
hear is recorded beside their evidence and blocks nothing. The checklists stay open until you do
them (owner decision, 2026-11-12).

| Part | Closes |
|---|---|
| 1 First visit and the app | v0.1c items 9 and 17 (first run, settings) |
| 2 Free flight | v0.1a item 13 (manual flight on WebGPU and WebGL2); v0.1b's manual circuit per assist; v0.1c item 17 (views, HUD, horizon, sound) |
| 3 Dogfight | v0.1d item 19 (Appendix E: the feel questionnaire, the 10-minute task, look and sound) |
| 4 Install and offline | v0.1c items 9 and 17 (the PWA) |

**Setup:** staging (`https://bl4202.github.io/Flying/`), current stable Chrome, headed, sound on, the
Mac on mains, keyboard and mouse. Keep one notes file. For each step, write what you saw or heard and
anything that surprised you, such as a control too sensitive, a warning at the wrong moment, or a crash
and its cause. Send the file, and it is recorded in the exit docs. Gamepad and HOTAS are not part of it
(no hardware; their look is not built).

### Part 1: first visit and the app (about 10 minutes)

1. A fresh profile or a private window. Check the compat screen, then the first-run steps in order (try
   them with the keyboard alone), then the main menu. A second visit should go straight to the menu.
2. **Settings:** look through all six tabs. Make a change and check that it survives a reload. Export
   the settings file and import it again.

### Part 2: free flight (about 35 minutes)

Fly the circuit once per assist level (Full, Standard, Reduced, Manual). Choose the level in the
menu, or in the pause menu (`Esc`).

1. **The circuit**, each time:
   1. From the runway, take the parking brake off (`P`), go to full afterburner (`Z`) and keep the
      centreline with `Q`/`E`.
   2. Rotate at about 70 m/s. Gear up (`L`) once climbing, flaps up (`V`) by 120 m/s.
   3. Fly a left-hand circuit at about 300 m above the field.
   4. On final: gear down (`L`), flaps to landing (`V` twice), airbrake (`N`), about 3 degrees.
   5. Flare, touch down, brake (`B`) and stop on the runway.

   Note per level whether it flew and the display rate.
2. **Across the four circuits**, also:
   - fly once in each arena (Rugged, Plain);
   - **Main menu**, then Fly again: the sortie should start clean;
   - **Cockpit and chase** (`C`): head look (`,` `.` `ArrowUp` `ArrowDown`, `X` with the mouse), zoom
     (right button), `/` to recentre. The chase camera should sit behind and above the aircraft at
     every speed;
   - **the HUD:** it should be readable on the runway, in the climb and on final. `I` changes the
     detail level, `-` and `=` the brightness. Note anything out of place against the horizon. Two
     things the bots' look noticed at v0.1c, for you to judge:
     - the cockpit's glare shield and panel fill the lower third of the view;
     - in the chase view on the runway, the mode line runs to the edge of the engine block's column;
   - **sound:**
     - the engine at idle, military and afterburner (and the light-off thump);
     - the wind rising with speed;
     - the stall horn, the over-G tone and PULL UP;
     - the gear chime.

     Note anything harsh, clipped or too quiet.
3. **Once, away from the circuit:**
   - a pull to the stall warning;
   - trim;
   - a hard pull until the screen greys out;
   - mouse-aim (`O`, then click the picture);
   - a deliberate crash and the respawn (`R`).
4. **The horizon:** with `?debug=1` (the overlay shows the height), climb to 10 km and look. The dip
   should look right, the edge crisp, and there should be no gaps in the ground.
5. **Controls:** in the pause menu's Controls, rebind one key, reload, and check that the binding held.
6. **WebGL2:** open `?renderer=webgl2` and fly for a minute (take-off, afterburner, a turn). Look for
   stutter, a black or flickering picture, and any notice at the bottom of the screen.

`C` does nothing while a scripted pilot flies (`?script=…`). This is by design: a scripted run takes
no keyboard, mouse or pad input.

### Part 3: dogfight (about 35 minutes)

1. For each assist level (Full, Standard, Reduced, Manual): main menu, Mode **Dogfight**, Bandit
   **Recruit**, Fly.
   1. Take off, find the bandit, shoot it down (gun or missile), then land and stop.
   2. Note the debrief's "Take-off, kill and landing" time.
   3. Choose **Rate this flight**: three answers, and a comment if you like.
2. After the four: Statistics, **Export**. Send the file; it holds the four answers and times.
3. **Look and listen:**
   - the tracers against the sky and the ground;
   - the missile and its explosion;
   - the bandit's wreck;
   - the HUD's box, arrow, pipper, ring, seeker circle and diamond, readable in daylight;
   - the seeker's growl rising as you line up, and the lock tone;
   - the gun;
   - being hit by a **Regular** bandit.
4. Hold `Ctrl` (throttle down) and press `Space`. Does the gun fire, or does macOS switch the input
   source?
5. Anything that felt wrong, one line each.

### Part 4: install and offline (about 10 minutes)

1. Install the app (the menu's Install button, or the browser's).
2. Close the tab and go offline. Open the installed app: it should start and fly.
3. Once a newer build is deployed, "Update ready" should appear in the menu, and never in flight.
