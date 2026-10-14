# ADR-31: Performance Conformance Test method and gates

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-14
- **Affects:** GDD 11.19, 11.21 (AC-3.2, AC-11.3), Appendix C.7 to C.9, AC-66.4; `crates/flight-test`, `crates/cli`, `crates/xtask`, `.github/workflows/`, `content/core/aero/`

## Context

v0.1a exits when the Kestrel's point performance matches its authored envelope (GDD C.9) within
3 % or 5 % (AC-3.2). The envelope targets come from a point-mass reference calculator (C.8). The
6-DOF model differs from that calculator in known ways: it trims with the elevator, applies thrust
along the body axis, uses g(h), geopotential ISA and a curved Earth, and the calculator floors
Mach to a 0.01 grid. Without a fixed method, a failing point could be fixed by changing the
method, the target or the data, and the choice would be made after seeing the numbers. AC-66.4
requires gates to be defined before the work starts and not weakened afterwards.

This ADR fixes the method before any tuning. GDD C.9 already says that differences larger than
the tolerance are resolved by retuning data, not by changing targets.

## Decision

### Scope and gate

- The gate is **ten airborne points**: `vmax_sl`, `vmax_11k`, `vmax_dry_sl`, `roc_sl`,
  `stall_clean`, `nsust_sl_m07`, `turn_sl_m05`, `nsust_6k_m09`, `ff_mil_sl`, `ff_ab_sl`.
- `to_roll` and `ldg_roll` need the gear model, which is v0.1b. They are reported as `deferred`
  (`requires: gear`). The gate requires the deferred set to be **exactly** `{to_roll, ldg_roll}`,
  so a point cannot be silently dropped. AC-3.2 closes fully at v0.1b.
- Tolerances are those of GDD 11.19 and C.9 (3 % or 5 % per point). They are not changed.
- Each point must also pass its **static cross-check** (below). A disagreement outside the
  stated band fails the run even if the headline value is inside tolerance.
- Report a warning when a point uses more than 60 % of its tolerance. The tuning goal is at most
  60 %.

### Common rules

- Mass and CG are frozen at the point's `mass_kg`, with fuel in feed order. Fuel flow is still
  computed.
- Environment: a sphere-over-sea world (R = 6 371 km, still air, standard day). Altitude hold
  uses h_MSL and `v . up(p)`, never BF z. `alt_m` means geometric h_MSL.
- ISA uses geopotential altitude (GDD 11.3). The calculator feeds h straight into ISA; the effect
  is -0.05 % on M_max at 11 km and +0.25 % on n_sust at 6 km, and is not compensated.
- P_s is computed from forces, `P_s = F_nongrav . v / (m g0)`, never by differencing. P_s and n are
  reported with g0 to match C.8.
- Measured values are continuous. They are not floored to the calculator's 0.01 Mach grid.
- Every run has a sim-time cap; a timeout is a failure (`timeout`), not a hang. A run that does
  not converge is `no_convergence`.
- Test pilots command the surfaces directly (dynamic inversion inner loops). They are test
  instruments, not the FCS of v0.1b.

### Procedures and cross-checks

| Point(s) | Procedure | Cross-check |
|---|---|---|
| `vmax_*` | Level acceleration with altitude and wings held. Once P_s < 5 m/s, fit P_s(M) linearly over the last 0.03 Mach and extrapolate to 0. Valid while the altitude error stays below 15 m and `abs(n - 1)` below 0.01 | Static level trim at fixed throttle, within 0.3 % |
| `roc_sl` | The **maximum 1 g level P_s**, binned in 0.005 Mach steps, from the SL acceleration run. C.7's 283.1 m/s equals the TAS at M 0.83, so it cannot be a steady climb. The Mach at the maximum is informational because P_s(M) is flat | Maximum of the trimmed P_s over M 0.5 to 1.1 |
| `stall_clean` | Start at 1.3 V_s, idle, then ramp alpha at 0.5 deg/s until 4 deg past the C_L peak. V_s = sqrt(2 m g0 / (rho0 S max C_L,aero)). Holding altitude at 1 kn/s is rejected: it needs about 16.6 kN of thrust and biases V_s by -4.5 % | Zero-thrust minimum-speed trim |
| `nsust_*`, `turn_*` | Trim, then a 40 s dynamic hold at full AB controlling n through Mach. Average the last 20 s. n is the wind-axis load factor `sqrt(1 + (V w / g)^2)`, capped at `n_max` and the C_L,max limit | Throttle-fixed turn trim, within 0.5 % |
| `ff_*` | Engine alone, M 0, h 0, ISA, from idle, read at 15 s | Analytic TSFC times thrust |

### Expected deviations from the calculator

Recorded here so they are not discovered during tuning: M_max +0.72 % (SL AB), +0.20 % (11 km),
+0.05 % (SL dry) from the 0.01 grid; roc about 0 %; n_sust and turn rate +1.5 % to +3.3 % from the
thrust-line alpha component, partly offset by the inlet loss to about +2.1 % at M 0.5; stall within
+-0.5 % once `C_L,base` is authored so the trimmed C_L,max is 1.600. All fit inside the
tolerances. 1 % more drag moves M_max by -0.18 % (SL dry), -0.78 % (SL AB) and -0.53 % (11 km).

### Tuning rule

When a point is out of tolerance, **retune data, not targets and not the method.** Every data
change is logged in `content/core/aero/README.md` (what changed, which point moved, by how much).
A change to a target, tolerance or procedure needs a new ADR (AC-66.4). If a target itself is shown
to be wrong, that is also an ADR, with the evidence.

### Tooling

`fg conformance` runs the procedures and writes a `conformance_report/1` JSON, a Markdown table
and CSV traces. `cargo xtask conformance` bakes, builds in release and gates on the exit code.
A required CI job `conformance` runs it on every push.

## Alternatives considered

- **Hold altitude through the stall.** Closer to a flight test, but at 1 kn/s it needs a large
  thrust and biases V_s low by 4.5 %. The FAR 25.103 style C_LMAX is the standard definition.
- **Retune the targets to what the 6-DOF gives.** Rejected by C.9 and AC-66.4: it would make the
  test pass by construction.
- **Compare against the calculator at every point instead of the authored envelope.** The
  envelope is the contract in the aircraft file; the calculator is the derivation of it.
- **Include the ground rolls now with an approximate gear.** The gear model is v0.1b; an
  approximation would be tuned and then thrown away.

## Consequences

- A failing point is diagnosed with two independent numbers (dynamic run and static cross-check),
  which separates a method fault from a data fault.
- The exit has a visible gap, the two ground rolls, which v0.1b must close.
- `content/core/aero/README.md` becomes the tuning log and must stay current.
- Any later change to the Kestrel tables requires re-running `conformance` and, because the sim
  hash depends on content, a reasoned golden re-bless.
