# Kestrel flight-control gains

`kestrel.json` is the `fcs/1` data of the Kestrel's control laws (GDD 11.11, ADR-34). Authoring
data is the file itself: there is no generator. The laws are in `crates/fcs`; this file only says
how they are tuned. Values are *targets* (starting points) until the handling suite and the
circuit gate (ADR-33) judge them; every change made while tuning is logged below.

| Block | Keys | Meaning |
|---|---|---|
| `pitch` | `k_q_per_s`, `k_n_per_s`, `k_ni_per_s`, `tau_cmd_s` | pitch-rate loop bandwidth; bandwidth of the load-factor loop (it closes `n` on `n_cmd` as a first-order lag of `1 / k_n_per_s` at every dynamic pressure, by dividing by the airframe's `dn/dalpha`); the integral gain relative to it; command prefilter |
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
| `mouse_aim` | `k_pitch_per_rad`, `lead_s`, `rate_cap_dps`, `deadzone_deg` | GDD 58.4. How fast the nose is asked to close on the cursor (per radian of error, looked ahead along the rate it already closes at), the most it is asked for, and the soft zone round the cursor in which the ask fades to nothing |
| | `k_bank`, `k_bank_far`, `k_roll_rate` | the roll stick per radian of the bank wanted (for a near cursor and a far one) and the roll rate taken off as damping |
| | `pitch_stick_max`, `ramp_per_s` | the most pull it asks for as a fraction of the stick (it bounds the energy a long turn bleeds) and the rate limit on both sticks |
| `recover`, `gcas` | | the recovery button's pull-up and automatic ground-collision avoidance |

Sources: the numbers are the starting values of the v0.1b plan (the structure follows GDD 11.11.2
and 58.4). `p_max_qbar` falls at high dynamic pressure to keep the structural roll rate in check;
`p_max_alpha` halves the roll rate at the 26 deg limit. Nothing here is aircraft physics: the
physics is in the airframe's aero tables, so a gain change moves no conformance point.

## Tuning log

Every change made while tuning, newest last: what changed, which test moved, by how much.

| Date | Change | Effect |
|---|---|---|
| 2026-10-22 | Initial values (v0.1b plan) | (baseline; the control laws did not exist yet) |
| 2026-10-23 | `pitch.k_n_per_s` 1.5 to 3.4; `pitch.k_ni_per_s` 1.0 to 0.15; `pitch.k_q_per_s` 6 to 8; `pitch.tau_cmd_s` 0.10 to 0.12 | H1 (load-factor step): overshoot 2.3 %, 90 % settling 1.25 s against the gate of 10 % and 1.5 s. 1.5 /s closed the load factor too slowly to settle in time; the integral only collects errors under 0.25 g, so a large `k_ni` left a slow tail |
| 2026-10-23 | `roll.k_pi_per_s` 0.5 to 0; `roll.tau_cmd_s` 0.08 to 0.10 | H2 (roll-rate step): overshoot 2.3 %, settling 0.60 s. The roll integral wound up against the aileron limit during the step; with the prefilter at 0.1 s the proportional loop alone settles inside the gate |
| 2026-10-23 | `overspeed.onset` 0.97 to 0.92; `overspeed.k_pull` 30 to 70 | H6 (afterburner dive from M 0.9 at 3 km): calibrated airspeed peaks at 1.002 `vmo` against the gate of 1.03. The wall has to start earlier because the dive gains speed faster than a soft pull takes it back |
| 2026-10-23 | Algorithm constants fixed in `standard.rs`/`recover.rs` while the departure suite ran: `BRAKE_SHARE` 0.4, `STOP_DELAY_S` 0.3, `ALPHA_LEAD_S` 0.4, `COUPLING_SHARE` 0.5, `COUPLING_Q_FLOOR_RPS` 0.1 rad/s, eased anti-spin between 15 and 30 deg | H5 alpha 26.1 deg and 9.16 g against alpha_limit + 2 deg and n_max + 0.3; H5b over the vertical the same; D1 alpha 26.2 deg and a stability-axis yaw rate of 23.9 deg/s against the 30 deg/s gate; D3 recovered in 10.0 s against 15 s. These are the law's structure (ADR-34), not gains, and are not data |
| 2026-10-24 | `mouse_aim`: the GDD's 1.6 /rad bank gain, 3 /rad pitch gain and 3 /s ramp replaced by `k_bank` 0.30, `k_bank_far` 0.57, `k_pitch_per_rad` 8.1, `ramp_per_s` 50, plus the new `lead_s` 0.23, `rate_cap_dps` 30.5, `pitch_stick_max` 0.55, `k_roll_rate` 0.49 | AC-58.4 (ADR-33 amendment 1): from 0 of 500 with the GDD's gains to 493 of 500 (98.6 %), 96.6 to 98.8 % on four other draws. The GDD's structure (roll to the cursor, pull as far as the lift points at it) is kept, but the lift now includes gravity and the pitch stick is a pitch-rate inversion, so the gains are not the GDD's numbers; a 3 /s ramp alone costs 0.3 s of the 1.0 s the time allows. About a dozen numbers were searched on the gate's draw and checked on four others (a random coordinate search, some 2 000 evaluations of 500 runs) |
