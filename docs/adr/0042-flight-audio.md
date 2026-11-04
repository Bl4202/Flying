# ADR-42: Flight audio: synthesis in an AudioWorklet

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-03
- **Affects:** GDD 56.2, 56.3, 56.6, 56.7, 56.9, A.12; ADR-39 (A1 to A8); `packages/audio`,
  `packages/app-shell`, `packages/platform-web`, `content/core/audio`

## Context

v0.1c needs the sound of flight: the engine (spool, whine, roar, afterburner), the wind, buffet and tyre
rumble, and the warning tones (GDD 56 basic). There are no recordings and no spatial emitters yet
(GDD 56.9's placeholder rule: tones and HUD text stand in for voice). The sound must start within
100 ms of the state that raises a warning, must not allocate on the audio thread, and must be tested
without a browser as far as possible.

## Decision

### Synthesis is pure code over numbers

`FlightDsp` (`audio/src/dsp/flight-dsp.ts`) turns 14 numbers (`P`: engine core thrust, afterburner
level and bits, dynamic pressure, angle of attack and its warning angle, airbrake, ground speed and
weight on wheels, the warning bits, cockpit or chase, and the three volumes) into stereo samples. It
knows nothing of Web Audio, so the same code renders offline in Node, which is how every level,
spectrum, loudness and timing gate is tested. State is in typed arrays and `process` allocates nothing
(A7).

**Voices** (every number is in `audio_profile/1`, `content/core/audio/kestrel.json`):

- **Engine.** The shaft speed is the profile's curve of the (lagged) core thrust. The blade-pass tone is
  `rpm / 60 x blade_count` with four harmonics and a shaft tone; their level is the profile's at
  military rpm and falls as the fifth power of rpm (fan-tip noise), so idle is 11 dB below military.
  The jet roar is coloured noise at `jet_level_db + 40 log10(thrust fraction)` (exhaust speed goes as the
  square root of thrust, power as its eighth power). The afterburner lifts the whole engine by
  `burner_roar_db` (GDD C.13: +6 dB) and lights with a thump. A stopped engine windmills, quietly.
- **Wind** is band-passed noise whose level rises `wind_slope_db` (10 dB) per decade of dynamic pressure.
  **Buffet** is a 28 Hz rumble from 2 degrees below the stall-warning angle. **Tyres** rumble in
  proportion to ground speed on the wheels.
- **Tones** (GDD 56.7): the stall horn pulses 2 to 8 times a second as angle of attack passes the warning
  angle; the PULL UP warble alternates two pitches at 3 Hz; the over-g tone beeps; the gear and master
  caution chimes repeat. The most urgent warning is the one heard (the bit order of `warningBits`, the
  same rule set the warning strip and the HUD read).

**Mix.** The aircraft bus (engine, wind, buffet, tyres) is ducked 6 dB, in 10 ms and back in 250 ms,
while a stall, departure, pull-up, over-stress or g warning sounds; the warnings bus is never ducked
(AC-56.5). The cockpit hears the aircraft through a 3 kHz low-pass at -18 dB plus a quiet interior hum;
the chase view has no canopy. Master gain, then a soft limiter with its ceiling at 0.85 (-1.4 dBFS).

**Loudness (A5).** `mix.level_offset_db` lifts the aircraft's voices and `mix.chase_offset_db` lowers
them in the chase view (which has no canopy to take 18 dB off), so that a 60 s circuit measures
-16 +- 1.5 LUFS in both views with a true peak of at most -1 dBTP. The offsets are tuned against the
recorded circuit trace (`packages/audio/test/fixtures/circuit-60s.csv`); the tuning log is in
`content/core/audio/README.md`. The warning tones keep their own levels, which are relative to the master.

### Where it runs

An `AudioContext` at 48 kHz with `latencyHint: 'interactive'` is created with the backend and stays
suspended until `resume()`, which the game calls from the Fly click (a user gesture). The synthesis runs
in a **TypeScript `AudioWorkletProcessor`** (`worklet/flight-processor.ts`), not the Rust/WASM worklet
that GDD 56.2 names: at v0.1c's voice count plain TypeScript costs 23 microseconds per 128-frame
quantum on the M4 (A6 limit: 210), and the kernels are pure functions over preallocated blocks, so a
port is mechanical (a GDD 0.2.3 clarification). Vite builds the worklet as a same-origin module
(`?worker&url`); it is loaded through `scriptUrl()` under the page's CSP and Trusted Types and is
precached like the workers.

**The state feed is k-rate `AudioParam`s**, written at most about 80 times a second and only when a
value changed (`WebAudioBackend.update`). A bitmask (warnings, engine bits) is set, never ramped, so no
intermediate value is heard; the synthesis smooths what must be smooth. GDD 56.2's shared ring arrives
with spatial emitters. The worklet reports a tone's start through its port with the audio clock's
time, which is how the page measures the 100 ms gate (A4).

**Buses and the seam.** `WebAudioBackend` implements `AudioBackend` (so it passes `audioBackendContract`
like the null backend) and `FlightAudio`: `update(status)`, `setCamera`, `setBuses`, `setMuted` and
`onOnset`. Short UI cues come from oscillators through a separate UI bus, so a duck never silences them.
Everything that touches the browser comes through `WebAudioEnv`, so the state machine and the parameter
feed are tested in Node with a fake context, and the real one by `?selftest=audio`. If the worklet cannot
load, the backend still plays cues and says why (`workletError`).

## Alternatives considered

- *Samples and a mixer.* No recordings exist, and the engine's pitch follows rpm continuously, which a
  few loops do poorly; synthesis from data also lets the owner tune by editing numbers.
- *A Rust/WASM worklet now.* More build machinery (a third WASM target, module loading in the worklet
  scope) for a cost that is not a problem at this voice count.
- *Messages instead of `AudioParam`s.* Allocation on the audio thread and an ordering problem with the
  quantum; `AudioParam`s are read once per quantum with nothing to allocate.
- *`setTargetAtTime` for every parameter.* It smooths a bitmask through values that mean other
  warnings.

## Consequences

- The sound of the aircraft is data (`audio_profile/1`) plus one file of code; a second aircraft needs a
  profile, not code.
- The loudness gate is a number against a recorded trace: if the physics changes the circuit, the trace
  and the offsets are re-recorded and re-tuned together, and the tuning log says so.
- A real output device adds latency the 100 ms gate does not measure (it measures from the state change
  to the tone starting in the render thread); the owner's listening check covers the rest.
- Chrome, Firefox and Safari run the worklet; `webAudioSupported()` lets the game fall back to the null
  backend where `AudioWorkletNode` is missing.
