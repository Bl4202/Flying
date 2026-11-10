# Weapons

One weapon per file (`weapon/1`, ADR-46). `class` is `gun` (with a `gun` block) or `srm` (a
short-range missile, with a `missile` block); a file has exactly the block of its class. An
aircraft names its internal gun in `internal_weapons.gun.ref` and its rail stores in
`loadout.rails[].store`; the baker checks the class of each (a gun on a rail, or a missile as the
gun, is refused). Ids use the type `wpn` (`core:wpn/gs27`). The weapons are in the **sim pack**, so
a change here changes the pack's SHA-256 and with it every determinism golden.

Numbers carry their unit in the key. Two keys are their own unit: `rpm` (rounds per minute) and
`rounds`. `cn_max`, `k_induced`, `n_prime` and `ref_intensity` are ratios. Angles are degrees in
the data (`_deg`) or milliradians (`_mrad`), and radians in the sim. A drag table is
`{ "axis": "mach", "points": [[mach, coefficient], ...] }`, strictly increasing in Mach, with
positive coefficients. Unlike the aerodynamic tables it is not tested for spikes: a transonic drag
rise is a real peak.

**Which values are the GDD's and which are targets.** GDD 15.12 and C.10 give: the GS-27 as a 27 mm
revolver firing about 28 rounds a second from 150 rounds, 100 kg; the Dart-1 as a rear-aspect IR
missile of 85 kg with a 9 kg warhead on the wingtip rails S1 and S9. Everything else is a *target*
until the GDD's weapons chapters are baked: the muzzle velocity, dispersion, round mass and drag
table, the motor, the missile's aerodynamics and autopilot, the seeker's lock range and cut-offs,
the fuze, the warhead's lethal radius and energy, the launch limits and the visual sizes. The
lock range (`lock_ref_range_m` 6 000 m at intensity 1) and the lock time are the values the
v0.1d gates (ADR-45, MS2) are written against.

## GS-27 (`gs27.json`)

| Field | Value | Source |
|---|---|---|
| `caliber_mm`, `rounds`, `mass_kg` | 27 mm, 150, 100 kg | GDD 15.12, C.1 |
| `rpm` | 1 700 (28.33 rounds/s) | GDD 15.12 ("28 rps"); the audio profile's `rate_hz` must equal `rpm / 60` within 0.01 Hz |
| `muzzle_mps` | 1 025 m/s | *target* |
| `round_mass_kg` | 0.26 kg | *target*; a hit delivers `½ m v_rel²` joules, so the damage gate GN6 is `½ · 0.26 · v² / 6 370` hit points |
| `dispersion_mrad` | 0.5 | *target*; the standard deviation σ of a Gaussian spread about the gun axis, on each of two axes (GDD 15.4.1); tuned in v0.1d (see the log) |
| `drag_cd` | 0.25 to Mach 0.8, 0.38 at 1.2, 0.31 at 2, 0.26 at 3 | *target* |
| `ammo_types` | `hei`, a tracer every 4th round | *target* |
| `max_age_s`, `min_speed_mps` | 3 s, 300 m/s | *target*: a round is removed after either |

## Dart-1 (`dart1.json`)

| Field | Value | Source |
|---|---|---|
| `mass_kg`, `warhead.he_kg` | 85 kg, 9 kg | GDD 15.12 |
| `motor` | 11 000 N for 2.5 s, 11.5 kg of propellant | *target* |
| `aero` | 0.01227 m² (a 125 mm body), boost and coast drag tables, `cn_max` 8, `k_induced` 0.047 | *target*; tuned in v0.1d (see the log) |
| `autopilot` | first-order lag of 0.4 s; 30 g; no manoeuvre below 150 m/s | *target*; the lag tuned in v0.1d (see the log) |
| `guidance` | proportional navigation, N' = 4 | GDD 15.5 (simple PN); the constant is a *target* |
| `seeker` | rear-aspect IR; 4 deg caged field of view; 30 deg gimbal; locks a target of intensity 1 at 6 000 m (range scales with the square root of the intensity); sees the target only within 35 deg of its tail; 0.1 deg noise; 0.5 s of tone to lock | *target* |
| `fuze` | proximity; arms after 300 m; triggers within 8 m | *target* |
| `warhead` | fragmentation; lethal radius 8 m; 2 548 000 J at zero distance (400 hit points: a burst within 4 m kills) | *target* |
| `launch` | at least 80 m/s, at most 7.5 g and 25 deg angle of attack, no launch inside 450 m; the fuze cannot arm in the first 0.5 s (`safe_sep_s`; the motor lights at launch) | *target* |
| `limits` | self-destructs at 25 s, below 150 m/s after burnout, or 2 s after its target is destroyed | *target* |
| `visual` | 3 m while the motor burns, 0.5 m coasting (the eye sees it at about 3.3 km and 0.6 km) | *target* |

Checks the baker applies beyond the schema's ranges: the gimbal limit is at least half the caged
field of view; the fuze arms before the shortest launch range; it triggers inside the lethal
radius; the propellant is less than the mass.

## Tuning log

Every change to a *target* value, with the reason and the gate that wanted it, goes here. The
four determinism goldens are re-blessed once, at the end of WP4 of v0.1d (ADR-45, D2), after the
last change; a change after that needs the owner.

| Date | Change | Why |
|---|---|---|
| 2026-11-09 | GS-27 `dispersion_mrad` 1.5 → 0.5, read as the Gaussian σ of GDD 15.4.1 (the WP1 text had called it a uniform cone's half-angle) | GN3 (`AC-54.2`: ≥ 90 of 100 rounds from the pipper hit at 300 to 1 000 m). At 1 000 m, over 10 runs of 100 rounds per aspect: σ 1.5 gave 63 to 73 % on average; a uniform 1.5 mrad cone 86 to 97 % (worst run 81); σ 0.5 gives 96 to 100 % (worst run 94). The owner chose σ 0.5 (WP2). Before the one re-bless, so no extra re-bless |
| 2026-11-09 | Dart-1 `cn_max` 12 → 8 and `autopilot.tau_s` 0.25 → 0.4 | MS5 (`fg sortie --sweep`): with the old values a defending Veteran was hit 100 % of the time at 1 km dead astern (it notices the missile at once and breaks at about 3.8 s to go, but the missile needs only about 16 g to follow, inside any `g_max` of the allowed 20 to 40). Each value alone left the 1 km cell at 100 %; together: holding target 100 % in every cell the seeker locks, defending Veteran 80 % at 1 km and 0 % at 2 and 3 km (the other combinations tried inside the ranges: `g_max` 20, `n_prime` 3, `cn_max` 9 and 10, `tau_s` 0.35, gave 1 km at 97 to 100 % or 0 %, or 52.5 %). Before the one re-bless, so no extra re-bless |
