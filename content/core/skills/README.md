# AI skills

One skill level per file (`ai_skill/1`, ADR-47). A skill says how well a computer pilot flies and
fights; the sim finds it by `rank` (1 for the lowest, then 2, 3, ... with no gaps), never by id, so
a pack may rename or add levels without a code change. Ids use the type `skl`
(`core:skl/recruit`). The skills are in the **sim pack**.

| Field | Meaning |
|---|---|
| `fires_gun` | The pilot fires its gun. **The Recruit never does** (owner decision D3, 2026-11-08: a deviation from GDD 19.6, where the Recruit fires; recorded in ADR-47) |
| `perception_delay_s` | Delay between something becoming visible and the pilot acting on it |
| `notice_prob` | Probability per decision that the pilot notices an incoming missile it can see |
| `g_frac` | Fraction of the aircraft's load-factor limit the pilot will pull |
| `gun_aim_sigma_mrad` | Standard deviation of the gun aim error |
| `defend_delay_s`, `break_timing_sigma_s` | Delay before reacting to being fired at, and the spread of the break's timing |
| `terrain_min_agl_m` | Lowest height above the ground the pilot will fly at |
| `commitment_s` | Shortest time a decision is kept before it may change |
| `extend_below_cas_mps`, `extend_until_cas_mps` | The pilot extends away from a fight below the first calibrated speed and until the second; 0 and 0 for never. `extend` is in `tactics` exactly when the first is above zero |
| `tactics` | Any of `pure_pursuit`, `lead_pursuit`, `lag_pursuit`, `break_turn`, `jink`, `extend`; the baker stores them as a bit mask in that order |

| | Recruit | Regular | Veteran |
|---|---|---|---|
| `rank` | 1 | 2 | 3 |
| `fires_gun` | no | yes | yes |
| `perception_delay_s` | 1.5 | 1.0 | 0.6 |
| `notice_prob` | 0.5 | 0.7 | 0.85 |
| `g_frac` | 0.65 | 0.8 | 0.9 |
| `gun_aim_sigma_mrad` | 12 | 8 | 5 |
| `defend_delay_s` | 3.0 | 2.0 | 1.2 |
| `break_timing_sigma_s` | 1.5 | 1.0 | 0.6 |
| `terrain_min_agl_m` | 300 | 250 | 150 |
| `commitment_s` | 3.0 | 2.5 | 2.0 |
| extend (below, until) | never | never | 140, 220 m/s |

The rows follow the ladder of GDD 19.6; the numbers are *targets*, to be tuned against the
v0.1d gates (AI1 to AI8, ADR-45). The ranks must be exactly 1 to n, each once; the baker reports
the file whose rank repeats or leaves a gap.

## Tuning log

| Date | Change | Why |
|---|---|---|
| | | |
