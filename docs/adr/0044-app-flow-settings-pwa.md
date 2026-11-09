# ADR-44: Application flow, first run, the main menu, settings v2, the PWA and offline

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-11-05
- **Affects:** ADR-19, ADR-22 (amended in the PWA part), ADR-37; GDD 53, 57.4, 59, 61.12;
  `packages/ui`, `packages/app-shell`, `packages/input`, `packages/platform-web`

This ADR is written in parts as the work packages land. It holds the application-flow and settings
part (WP8) and the PWA and offline part (WP9), which amends ADR-22's update policy.

## Context

Until v0.1b the game started the simulation as soon as the compat check passed: there was no way to
choose an arena, to come back from a flight, or to learn the controls, and the settings had nowhere to
live but the pause menu. v0.1c adds a first-run flow, a main menu, a settings screen and a second arena,
and a session can now be reset (ADR-43: `fg_configure` with an arena, `fg_reset`).

## Decision

### The state machine

```text
BOOT -> COMPAT_CHECK -> FIRST_RUN -> MAIN_MENU <-> DEMO
             \_____________________^      ^
  (?fly=1, ?perf= and ?script= go straight to DEMO)
                \-> FATAL  <- any
```

`flowReducer` (`ui/src/gameflow.ts`) stays a pure reducer. `FIRST_RUN` carries the step; `MAIN_MENU`
carries which screen (the home screen or the settings); `DEMO` is unchanged, and the pause menu's
**Main menu** returns from it. The compat check remembers where the player goes next (`after`), because
the compat screen may or may not be shown first.

### Where the visit starts

`main.ts` loads the settings before the compat check and decides: a scripted run, a performance run or
`?fly=1` start flying at once (the specs and the performance harness keep their meaning); a visit whose
settings do not say the setup is done starts the **first run**; any other visit starts at the **main
menu**.

### Fly, and what a sortie is

The workers start at boot and the core stays **idle** (nothing runs, nothing is held) until the player
chooses Fly. **Fly** is: `configure(session, script)` (the sim worker regenerates every terrain level
for the arena), the input pump forgets the last sortie (`newSortie`), the controls take the settings and
the flight context, `start`, then audio resumes. The click is the gesture browsers require for audio
and Pointer Lock.

The pump reset is not optional. A fresh core numbers its spawns from 1 again; the pump, still holding the
last sortie's acknowledgement (1), would take the new first spawn for one it had already answered and
never release the throttle the sim holds for it, leaving the lever where the previous flight left it.
`menu.spec.ts` flies, raises the throttle, returns to the menu and flies again, and checks that the lever
follows the new aircraft.

**Main menu** (from the pause menu) is: close the menus without resuming, `reset` the core to idle, the
pump back to the menu context, and the flow to `MAIN_MENU`. The overlay's pause, the visibility pause
and the pause key do nothing while no sortie is running.

### The first run

Six steps, each skippable and so is the whole flow, shown once: what the player flies with (a pad that
is plugged in selects its profile), calibration (a joystick only), the assist level, comfort and
accessibility (g effects, camera shake, reduced motion, HUD colour, volume), the photosensitivity notice
(GDD 57.4) and a closing step. A choice is applied as it is made, so skipping keeps it. Every control is
a native button or slider with the browser's focus ring; Tab wraps inside the dialog and the step
heading takes focus when the step changes. Finishing or skipping stores `setupDone` and asks the browser
to keep the player's storage (ADR-19). The First Flight mission waits for GDD 25; the first run ends at
the menu (a GDD 0.2.3 clarification).

### The main menu and the settings screen

The main menu holds Fly, the arena and the assist level, Settings, and three rows that exist only when
they apply: **Install**, **Update ready — reload** and **Turn sound on**. They are only ever in the
menu, never over a flight.

The settings screen has six tabs (Gameplay, Controls, Video, Audio, Accessibility, Data) as a WAI-ARIA
tablist: arrows, Home and End move between them. Only the open tab's DOM exists. Every setting has a
default and each tab has a button that restores its own. The Controls tab opens the v0.1b editor over the
screen and returns to the tab. The Data tab exports and imports the settings, resets them (two
presses), runs the setup again, and shows where the settings are kept.

### Settings v2

`settings/2` adds `arena`, `camera`, `fovDeg`, `cameraShake`, `reducedMotion` (`auto` follows the
system), `hud {palette, brightness, declutter, overlayScale}`, `video {renderer, preset,
resolutionScale}`, `audio {master, aircraft, warnings, ui, muteWhenHidden}` and `setupDone`.

- **Reading is forgiving**: an unusable field takes its default and the rest are kept.
- **Migration** reads `settings/1` once, when `settings/2` does not exist, and keeps every field of it;
  the old entry is left in place, so the older build still finds its settings. A player coming from
  v0.1b has not seen the first run, so `setupDone` is false.
- **Export and import** (`flying-settings.json`): the text has a fixed key order, so export, import and
  export again is byte for byte the same file. Import is strict: it lists every field it cannot use by
  JSON pointer and stores nothing.

### The installable app

`manifest.webmanifest` is emitted at build time under that fixed name (a browser finds it by the URL in
the page, and the Service Worker precaches it with the other files), with `id`, `start_url` and `scope`
set to the base path, `display: standalone`, theme and background colours, and three icons: 192 and
512 pixels and a 512-pixel maskable one. The icons are rendered once by `pnpm icons` from original SVGs
in `packages/app-shell/icons` and committed; the build emits them under content-addressed names, so a
changed icon is a new file. The head carries the manifest link, the theme colour and the SVG icon,
written without whitespace between the tags because the in-flight DOM budget (AC-53.4) counts text nodes.

`beforeinstallprompt` is held from the start of the page, which can be before the game is up, and offered
only as the main menu's **Install** button. The browser's own bar is suppressed; the offer is spent by
asking (the browser honours one `prompt()` per event); `appinstalled` and `display-mode: standalone`
take the button away.

### Offline and updates (amends ADR-22)

Each build's Service Worker has a cache of its own, named for the build and filled when the worker
installs. The decisions are in `sw-strategy.ts`, pure and tested in Node with fake caches:

- **Navigations** are answered by the build's own cached shell; the network is asked only if the cache
  has none. A page that opens is therefore the build in control, with its own assets. A newer build
  installs in its own cache beside it and waits. The v0.1b worker wrote the network's `index.html` into
  the running build's cache; nothing writes to a cache after its worker has installed now.
- **Assets** come from the build's cache, then any other build's (an older tab still loading a chunk, or
  a newer build mid-install), then the network. Everything a build emits is content-addressed.
- **Updates**: on start, online, the page asks for a newer build (`registration.update()`). When one is
  installed and waiting, the main menu shows **Update ready — reload**. Choosing it sends the worker
  `skip-waiting`; the worker calls `skipWaiting()`, drops the old build's cache when it activates, and the
  page reloads on `controllerchange` — and only when the player asked, so the first install claiming the
  page never reloads it. The menu is the only offer: a flight is never interrupted. ADR-22's "never
  `skipWaiting`" becomes "only on the player's request".

`menu.spec`, `pwa.spec` and `offline.spec` cover it in the browser: the manifest and each icon fetched at
its declared size, a controlling worker with a fetch handler, a synthetic install prompt through the
menu, no cache written to on a visit, and the whole game (menu, Fly, ticks, frames, the audio worklet)
after a reload with the network off.

## Alternatives considered

- *Starting the core as before and treating the menu as an overlay.* The menu would sit over a
  running aircraft that falls out of the sky, and Fly would have to respawn, which cannot choose an
  arena.
- *Putting the first-run state in a cookie or `localStorage`.* The settings store already persists, is
  migrated and exported, and holds the flag with everything else.
- *A settings screen built from the pause menu.* The pause menu is the in-flight quick panel and must
  stay small; the screen has to hold more than fits there.

## Consequences

- Every spec that wants the flight opens the game with `fly=1` (`openGame` adds it); the menu specs open
  it without.
- `settings/1` stays in storage after migration. A later version may delete it once v0.1b is no longer a
  rollback target.
- A player who never opens the main menu never sees the update row, and keeps the build they have until
  they do; every build keeps working offline, so nothing is lost by waiting.
- The debug overlay (`?debug=1`) covers the menu, so specs that drive the menu use `?flags=stall` for the
  test hooks.
