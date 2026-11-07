# Audio profiles (`audio_profile/1`)

How an aircraft sounds (GDD 56.3, A.12, ADR-42). There are no samples: the engine, wind, buffet,
tyre and warning voices are synthesised from the numbers in this file by a worklet in the
browser. Levels are dB relative to the master bus; frequencies in Hz.

**Engine.** The blade-pass tone is `rpm / 60 x blade_count`, and `blade_level_db` and `shaft_level_db` are its level at military rpm (it falls as the fifth power of rpm); `rpm_by_core` gives the shaft speed
from the engine's core thrust fraction (0 stopped, 1 military power) and must pass through
`idle_rpm` and end at `[1, military_rpm]`. The roar's level is `jet_level_db` at military power
and follows the exhaust speed; the afterburner adds `burner_roar_db` (GDD C.13 says +6 dB; the
test measures 6 +- 1 dB) and a light-off thump.

**Aero.** Wind rises with dynamic pressure by `wind_slope_db` for ten times the pressure (test:
10 +- 2 dB per decade). Buffet starts `buffet_margin_deg` below the stall-warning angle of attack.

**Cockpit.** In the cockpit view the outside sound is cut above `canopy_lowpass_hz` and lowered by
`canopy_attenuation_db`, with a quiet interior hum; the chase view has neither.

**Mix.** `duck_db` lowers the aircraft bus while a warning sounds; the warning bus is never
ducked (AC-56.5). `level_offset_db` and `chase_offset_db` calibrate the loudness of the cockpit and
chase views (GDD 56.6).

**Kestrel (`kestrel.json`).**

| Value | Source |
|---|---|
| `blade_count` 24, `idle_rpm` 8 000, `burner_roar_db` 6 | GDD C.13 |
| `military_rpm` 10 400 | *target* (GDD C.13 gives no number) |
| `rpm_by_core` | *target*: 2 000 rpm stopped (windmilling), 8 000 at the engine's idle thrust fraction 0.06, 9 300 at half, 10 400 at military |
| every level, frequency and formant | *target*: first values, tuned in WP7 against the loudness gate (-16 +- 1.5 LUFS over the circuit's first 60 s, AC-56.6); changes are logged here |
| `duck_db` -6 | ADR-39 A4 |

**Tuning log.** All other levels are the first values; only the calibration offset was tuned.

| Date | Change | Measured (circuit, first 60 s from brake release, cockpit mix) |
|---|---|---|
| 2026-11-03 | `level_offset_db` 0 → 15.4 | cockpit mix before: -31.41 LUFS, -20.40 dBTP; after: -16.01 LUFS, -5.01 dBTP (gate: -16 +- 1.5 LUFS, <= -1 dBTP) |
| 2026-11-03 | `chase_offset_db` (new) -20 | chase mix with no offset: +1.49 LUFS, +3.57 dBTP (the soft limiter pinned); at -20: -15.02 LUFS, -4.56 dBTP |
| 2026-11-03 | DSP: whine level scales as (rpm / `military_rpm`)^5 (the idle-to-military step was 1.7 dB, found by `?selftest=audio`) | cockpit -17.05 LUFS, chase -17.13 LUFS |
| 2026-11-03 | `level_offset_db` 15.4 → 16.4 | cockpit -16.05 LUFS, -4.03 dBTP; chase -16.13 LUFS, -3.6 dBTP |
| 2026-11-07 | `interior_hum_db` -34 → -58 (the browser run showed the cockpit at about -20 dB at every throttle: stopped -20.2, idle -20.1, military -18.7, because the hum, lifted by the offset, drowned the engine); after: stopped -42, idle -36, half -31, military -24 (ground, cockpit) | cockpit -19.48 LUFS before re-tuning |
| 2026-11-07 | `level_offset_db` 16.4 → 19.9 and `chase_offset_db` -20 → -23.5 (the hum had been holding the cockpit figure up; the chase view has no hum) | cockpit -15.98 LUFS, -2.60 dBTP; chase -16.13 LUFS, -3.67 dBTP |

`level_offset_db` lifts the aircraft's voices only (the schema says so); the warning tones keep their
own levels, which are relative to the master. `chase_offset_db` lowers them in the chase view, which
has no canopy to take 18 dB off: without it the chase mix is that much louder than the cockpit's. At full power with the afterburner lit the aircraft bus
reaches the soft limiter (ceiling 0.85), so the kernel tests run at an offset of 0 and compare
levels there. Re-tune the offset (and this table) whenever a level above or the trace fixture
(`packages/audio/test/fixtures/circuit-60s.csv`) changes.
