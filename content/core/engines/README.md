# Engines

## F129 (`f129.json`)

The Kestrel's engine, GDD Appendix C.3. The steady values are C.3's exactly; the dynamics are
*targets*.

| Field | Value | Source |
|---|---|---|
| `sl_dry_n`, `sl_ab_n` | 79 000 N, 128 000 N | C.3 |
| `density_exp` | 0.75 | C.3, `(rho / rho0)^0.75` |
| `mach_factor_dry`, `mach_factor_ab` | 1.00, 0.98, 1.02, 1.12, 1.20, 1.08, 0.85 at M 0 to 2.4 | C.3; dry and afterburner share the table, clamped at the ends |
| `idle_fraction` | 0.06 | *target* |
| `tsfc.dry_kg_per_ns`, `tsfc.ab_kg_per_ns` | 1.983e-5, 5.382e-5 | C.3. Military at sea level burns 1.567 kg/s, full afterburner 6.889 kg/s (C.9 `ff_mil_sl`, `ff_ab_sl`) |
| `spool_up_s`, `spool_down_s` | 1.8 s, 1.2 s | GDD 11.8.2 time constants |
| `ab_lightoff_s` | 0.6 s | GDD 11.8.2 light-off delay |
| `ab_stage_s` | 0.5 s | GDD 11.8.2 afterburner stage time constant |
| `ab_cutoff_s` | 0.8 s | C.3 cut-off, used as the time constant of the afterburner running down |
| `ab_stages` | 4 | GDD 11.8.2 |
| `inlet` | loss from 15 deg over 25 deg | GDD 11.8.1 |
| `limits` | compressor stall above 45 deg, windmill relight from 120 m/s | GDD 11.8.4; relight speed *target* |

**Spool times.** C.3 quotes idle to military in 4.0 s and military to afterburner light-off in
1.5 s, while GDD 11.8.2 gives first-order time constants of 1.8 s up and 1.2 s down, a 0.6 s
light-off delay and a 0.5 s stage time constant. These agree as totals: with a 1.8 s time constant
the core is at 90 % of military after 4.1 s, and the afterburner reaches 90 % of its thrust 1.75 s
after selection. The model uses 11.8.2's form. No steady conformance point depends on them.

**Throttle.** The lever's 0 to 0.8 is idle to military and 0.8 is the military detent; past it
the four afterburner stages share 0.8 to 1.0 equally, and a lever exactly on an edge selects the
lower stage (`crates/flight/src/engine.rs`).
