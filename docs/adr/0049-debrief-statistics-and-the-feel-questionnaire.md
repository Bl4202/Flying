# ADR-49: The dogfight in the app: mode, debrief, statistics and the feel questionnaire

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-10
- **Affects:** GDD 4.5, 53.3, 59, 64.6, 65, 66.4; ADR-37 (amendment 1), ADR-41 (amendment 1),
  ADR-42 (amendment 2), ADR-44, ADR-45, ADR-48; `packages/ui` (`stats/`, `debrief.ts`,
  `feel-survey.ts`, `statistics-screen.ts`, `main-menu.ts`, `pause-menu.ts`, `overlay-rows.ts`,
  `settings.ts`), `packages/app-shell` (`demo.ts`, `debrief-flow.ts`, `screens.ts`, `session.ts`,
  `status-feed.ts`, `menus.ts`), `packages/platform-web/src/flags.ts`

## Context

v0.1d's exit needs the duel to be playable from the menu, its outcome shown, and its numbers kept
(GDD 65 basic, ADR-45 ST1 to ST6), and it needs the owner's own ratings of the feel, once per assist
level (ADR-45 G1-owner). The core already counts what happened (ADR-48: the session counters in the
snapshot and the event ring); the page must turn that into a mission's record without trusting a
frame it might miss, and keep it in the browser.

## Decision

**Mode and bandit.** Settings (still `settings/2`) gain `scenario` (free flight or a duel) and
`banditSkill` (1 Recruit, 2 Regular, 3 Veteran, default 1); a stored v0.1c entry without them reads
with the defaults, and the strict import accepts files with or without them. The main menu offers
Mode, and Bandit only for a dogfight; free flight flies with skill 0. URL flags lay `?scenario=`
(`free`, `duel`, `merge`) and `?skill=` over the settings for a visit, and the menus show them until
the player changes them. `merge` is the duel's test start (both aircraft in the air, 3 km apart): it
forces `spawn=air`, the menus never offer it, and it is recorded as a duel. `?script=sortie` puts the
bot playtester in the seat (ADR-47).

**The mission recorder** (`ui/src/stats/recorder.ts`) watches one mission from Fly to its end on the
simulation's clock (the snapshot's tick at 60 Hz), so a held simulation adds no time:

- Sorties follow GDD 65.2: one starts after 10 s continuously off the wheels (dated from lift-off) or
  at an air start (the first sample, or a respawn, off the ground), and ends after 5 s on the wheels
  below 5 m/s, with the aircraft's destruction, or with the mission. A touchdown and a roll-out below
  15 m/s with no crash is a successful landing; lifting off again takes it back. Records (highest
  altitude, fastest true airspeed, Mach) are per sortie and kept while alive.
- The duel's numbers are the **rise of the snapshot's session counters** since the first sample,
  summed sample by sample (a counter that falls, a new session, counts from zero). The plan read
  "kills and deaths by kind from events"; the counters carry the same kinds (gun, missile, crash
  kills; deaths) and, unlike the ring of 32 events, cannot lose one to a missed frame or to the message
  channel, which carries no events. Events (shared channel only) give the first kill's moment.
- The task (GDD 4.5): from Fly to the first stop below 2 m/s on the wheels, after a landing, following
  the first kill.
- A clock that runs backwards (a stale sample from the previous session, then the fresh core) starts
  the measurement again. `update` makes no garbage; a sortie becomes an object only when it ends.

**`mission_result/1`** (`stats/types.ts`) is GDD 65.4's subset: id (`m-<date>-<time>-<4 hex>`),
build, date, mode, assist, skill, arena, duration, sorties, the combat numbers, the task, and the
outcome (`landed`, `ended`, `died`).

**The statistics store** (`stats/store.ts`, key `stats/1`) holds totals, bests (each with its
mission id), the newest 50 results and every counted id. A result is aggregated by its id, once; the
new value is written in one `kv.set` and the copy in memory changes only after it succeeds, so a
failed write leaves both unchanged and a retry counts the mission exactly once (AC-65.2, ST2). The
app retries a failed write once and otherwise says so. Export writes `stats_export/1` (the statistics
and the feel answers) as canonical JSON, rebuilt in a fixed key order; import is strict (every bad
value named by JSON pointer, nothing applied) and merges by summing totals and keeping the better
bests (AC-65.6). A file whose missions are all counted already adds nothing to the totals, so
importing the same file twice does not double them.

**The debrief** opens on its own when a duel's sortie ends with a stop (the recorder's
`sortieEnded()`), and from the pause menu's new End sortie in any mode; it holds the simulation and
silences the flight controls. It shows time in the air, kills (gun and missile), gun accuracy,
missiles, aircraft lost, landings, highest altitude (ft), fastest (kn and Mach) and the task time.
Fly again starts the same session afresh without showing the main menu; Rate this flight opens the
questionnaire; Main menu leaves. The mission is finished (and counted) when the debrief opens, at
Main menu, at Fly again and at `pagehide`. The debrief and the questionnaire live in a small
controller of their own (`debrief-flow.ts`), not in the pause menu's, because a scripted pilot has
no menus and the soak flies the bot's sortie again and again through Fly again.

**The feel questionnaire** asks three statements on a 1-to-5 scale (believable, satisfying, easy to
learn) and takes an optional comment of up to 500 characters; the answers are kept in `feel/1` with
the assist, the bandit, the build, the date and the mission id, go out with the statistics' export,
and are summarised (count and medians) on the statistics screen. Nothing leaves the device.

**The statistics screen** (from the main menu) shows the totals and bests, each with its GDD 65.2
definition as a tooltip, the last ten missions, the questionnaire's count and medians, and Export,
Import, Clear (pressed twice) and Back. Esc goes back.

**All of it is built on open and removed on close**, with native controls, focus on the first
action, and Tab kept inside each dialog (a radio group counts as one stop).

**The debug overlay** keeps 40 rows: the `g` row's text joins the `nz` row (its leading number stays
the load factor, which `input.spec.ts` reads) and the freed slot shows the duel: the bandit's state,
the range, both healths, rounds and missiles.

**Test hooks** (with `?debug=`): `combat()` (a copy of the newest duel status), `debrief()` (the
open debrief's numbers), and `memory()` (one soak sample: JavaScript memory by
`measureUserAgentSpecificMemory` where the page is isolated, else the heap size; WebAssembly
memory; live GPU buffers and textures; live audio nodes; silences; worker errors; device losses;
workers; the sim's rate; debriefs opened). No hook moves the bandit: tests that need it close use
`scenario=merge`.

## Alternatives considered

- **Counting from events alone**, as the plan first wrote: exact in the shared channel, but the
  message channel has no events and a long frame can lose some from the ring. The counters are the
  core's own totals; events only sharpen the first kill's time.
- **Wall-clock time for sorties and the task**: simpler, but a paused or menu-held simulation would
  inflate airborne time and the task. The tick is already in every snapshot.
- **A separate store write per field** (totals, then log, then seen): a failure between writes could
  count a mission without remembering it, or the reverse. One value, one write.
- **The debrief inside the pause-menu controller**: shorter, but the scripted pilot (and so the
  soak) has no menu controller.

## Consequences

- The plan's `perf=combat` scenario and the soak's `leak-js` and `leak-gpu` faults arrive with their
  consumers in WP9; this part adds `scenario`, `skill` and `script=sortie` only.
- Statistics are local and not tamper-proof (GDD 65.5): the eligibility flags (invulnerable, unlimited
  ammunition) do not exist yet, so every mission counts.
- A mission ended by closing the tab is counted only if the browser lets the store's write finish.
