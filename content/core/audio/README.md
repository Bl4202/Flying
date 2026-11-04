# Audio profiles (`audio_profile/1`)

How an aircraft sounds (GDD 56.3, A.12, ADR-42). There are no samples: the engine, wind, buffet,
tyre and warning voices are synthesised from the numbers in this file by a worklet in the
browser. Levels are dB relative to the master bus; frequencies in Hz.

**Engine.** The blade-pass tone is `rpm / 60 x blade_count`; `rpm_by_core` gives the shaft speed
from the engine's core thrust fraction (0 stopped, 1 military power) and must pass through
`idle_rpm` and end at `[1, military_rpm]`. The roar's level is `jet_level_db` at military power
and follows the exhaust speed; the afterburner adds `burner_roar_db` (GDD C.13 says +6 dB; the
test measures 6 +- 1 dB) and a light-off thump.

**Aero.** Wind rises with dynamic pressure by `wind_slope_db` for ten times the pressure (test:
10 +- 2 dB per decade). Buffet starts `buffet_margin_deg` below the stall-warning angle of attack.

**Cockpit.** In the cockpit view the outside sound is cut above `canopy_lowpass_hz` and lowered by
`canopy_attenuation_db`, with a quiet interior hum; the chase view has neither.

**Mix.** `duck_db` lowers the aircraft bus while a warning sounds; the warning bus is never
ducked (AC-56.5).

**Kestrel (`kestrel.json`).**

| Value | Source |
|---|---|
| `blade_count` 24, `idle_rpm` 8 000, `burner_roar_db` 6 | GDD C.13 |
| `military_rpm` 10 400 | *target* (GDD C.13 gives no number) |
| `rpm_by_core` | *target*: 2 000 rpm stopped (windmilling), 8 000 at the engine's idle thrust fraction 0.06, 9 300 at half, 10 400 at military |
| every level, frequency and formant | *target*: first values, tuned in WP7 against the loudness gate (-16 +- 1.5 LUFS over the circuit's first 60 s, AC-56.6); changes are logged here |
| `duck_db` -6 | ADR-39 A4 |

Tuning log: none yet.
