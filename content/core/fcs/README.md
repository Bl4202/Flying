# Kestrel flight-control gains

`kestrel.json` is the `fcs/1` data of the Kestrel's control laws (GDD 11.11, ADR-34). Authoring
data is the file itself: there is no generator. The laws are in `crates/fcs`; this file only says
how they are tuned. Values are *targets* (starting points) until the handling suite and the
circuit gate (ADR-33) judge them; every change made while tuning is logged below.

| Block | Keys | Meaning |
|---|---|---|
| `pitch` | `k_q_per_s`, `k_n`, `k_ni_per_s`, `tau_cmd_s` | pitch-rate damping of the inner loop; proportional and integral gain on the load-factor error (rate per g, scaled by `g / V` in the law); command prefilter |
| | `alpha_band_deg`, `k_alpha_per_s`, `alpha_min_deg` | the angle-of-attack limiter: it acts in the band below the limit (`fcs.limiters.alpha_limit_deg` in the aircraft file); the lowest angle it allows |
| | `q_max_dps` | the Reduced law's largest commanded pitch rate, against dynamic pressure (kPa) |
| `roll` | `k_p_per_s`, `k_pi_per_s`, `tau_cmd_s` | roll-rate loop |
| | `p_max_qbar`, `p_max_alpha`, `p_max_nz` | the fraction of the largest roll rate (`fcs.limiters.roll_rate_max_dps`) available, against dynamic pressure, angle of attack and load factor; the three multiply |
| `yaw` | `k_beta_per_s`, `k_r_per_s`, `beta_max_deg` | sideslip and yaw-rate feedback; the largest sideslip the pedals command, against dynamic pressure |
| | `yaw_damper_s`, `washout_s` | the Reduced law's yaw damper |
| `full` | `gamma_rate_max_dps`, `k_gamma_per_s`, `bank_max_deg`, `k_bank_per_s`, `alpha_margin_deg` | the Full law: flight-path-rate command and hold, bank-angle command, and how much tighter than Standard its angle-of-attack limit is |
| `overspeed` | `onset`, `k_pull` | the soft wall: from this fraction of `vmo` and `mmo` the load-factor command gains `k_pull` g per unit of ratio |
| `config` | `n_min_config` | the negative load limit with the gear or flaps out |
| `ground` | `wow_debounce_s`, `liftoff_blend_s`, `touchdown_blend_s`, `k_q_ground_s`, `nws` | ground mode: weight-on-wheels debounce, the blends to and from the flight law, pitch damping on the ground, and nose-wheel steering (full range below `low_speed_mps`, the small range above `high_speed_mps`) |
| `assist_blend_s` | | time over which an assist switch blends the surface command |
| `mouse_aim` | `k_bank`, `k_pitch_per_rad`, `deadzone_deg`, `ramp_per_s` | GDD 58.4 |
| `recover`, `gcas` | | the recovery button's pull-up and automatic ground-collision avoidance |

Sources: the numbers are the starting values of the v0.1b plan (the structure follows GDD 11.11.2
and 58.4). `p_max_qbar` falls at high dynamic pressure to keep the structural roll rate in check;
`p_max_alpha` halves the roll rate at the 26 deg limit. Nothing here is aircraft physics: the
physics is in the airframe's aero tables, so a gain change moves no conformance point.

## Tuning log

Every change made while tuning, newest last: what changed, which test moved, by how much.

| Date | Change | Effect |
|---|---|---|
| 2026-10-22 | Initial values (v0.1b plan) | (baseline; the control laws do not exist yet) |
