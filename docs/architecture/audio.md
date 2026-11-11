# Audio architecture, as built in v0.1d

The sound the player hears at v0.1d: the engine, wind, buffet, tyre rumble and the warning tones, and from
v0.1d the duel's voices. The decisions are in [ADR-42](../adr/0042-flight-audio.md) (amendment 2 for the
duel); the gates are A1 to A8 of [ADR-39](../adr/0039-v0-1c-gates.md) and AU1 to AU5 of
[ADR-45](../adr/0045-v0-1d-gates.md). There are no samples: the sound is synthesised from the numbers in
the aircraft's `audio_profile/1` (`content/core/audio/kestrel.json`, in the presentation bundle).

## Where it runs

```text
Main thread   status feed (reads the snapshot every animation frame)
                -> fillParams(status, camera, buses) -> 14 AudioParams, set per frame
              WebAudioBackend: AudioContext (48 kHz, 'interactive'), the worklet node, a master gain
Audio thread  AudioWorkletProcessor -> FlightDsp.process(left, right, 128)
```

The context is created at boot and resumed on the Fly click (or the first gesture); until then the main
menu shows the "enable audio" prompt (no in-flight DOM). The synthesis is plain TypeScript in an
`AudioWorkletProcessor` (the GDD names Rust and WASM; at this voice count TypeScript is cheap and a port is
mechanical). The state travels as **k-rate AudioParams**: a bitmask is set, not ramped. The worklet module is
a same-origin file emitted by Vite and loaded through `scriptUrl()`, precached like the workers. The tab
being hidden mutes the sound. Without a Web Audio context a null backend does the same bookkeeping, and both
pass `audio-backend.contract.ts`.

## The kernel (`packages/audio/src/dsp/`)

`FlightDsp.process` makes one 128-frame block from `params` (indices `P`): the engine (rpm from the thrust
fraction through a data curve; a blade-pass tone at rpm / 60 x 24 with harmonics, a shaft tone, turbine hiss,
jet roar as filtered noise with the afterburner's +6 dB and a light-off thump), wind (band-passed noise
rising with dynamic pressure), buffet (from the stall angle less 3 degrees), tyre rumble (with ground speed),
the warning tones (stall horn, over-G, PULL UP warble, gear and caution chimes), the aircraft bus (ducked by
6 dB under a continuing warning), the warnings bus (never ducked), the cockpit mix (the outside through a
3 kHz low-pass and an interior hum) or the chase mix, the master gain and a soft limiter (ceiling 0.85, so the
true peak stays under -1 dBTP).

**The duel's voices** (v0.1d), from the profile's `weapons` block: on the aircraft bus the gun (a pulse per
round at `rate_hz`, which the baker checks against the gun's rpm / 60), the bandit's gun (the same pulses
low-passed, quieter with distance), the launch, a hit taken and an explosion (quieter with distance); on the
warnings bus the seeker's growl (its pitch rising with the signal) and the lock tone. Eight more parameters
carry them: `gunFiring`, `enemyGunDist`, `seeker`, `signal`, and the session's counters `launches`,
`hitsTaken`, `explosions` with `explosionDist`. A one-shot voice starts at the block where its counter rises;
the first values after a reset, and a counter that falls (a new session), are adopted silently. The kernel
reports each one-shot's onset to the page (`ONSET` codes), which the audio self-test times (AU3). The duel's
voices draw their own noise, so the flight voices sound the same with or without them; their levels are
worked out once, at construction.

**It is written in passes over the block, not a call per sample.** V8 boxes a number returned from a call it
does not inline, and in a function this long it stops inlining early; the first version made about 23 KB of
garbage a block that way. Now the five noise draws are made in one call, each filter runs over the block in
one call with its registers in locals (`Biquad.processBlock`), and the limiter is written out in the loop.
The output is bit-identical to the per-sample version, and a block makes about 33 bytes (two boxed numbers
at block rate). The levels are calibrated in the profile (`mix.level_offset_db` for the aircraft's voices,
`mix.chase_offset_db`): the 60 s circuit mix measures about -16 LUFS integrated.

## Testing it

| Gate | What | Where |
|---|---|---|
| A1 | the blade-pass peak is within 1 % of rpm / 60 x 24 at idle, military and afterburner (FFT); the 24 is the GDD's, not the profile's | `dsp/flight-dsp.test.ts` |
| A2, A3 | engine level rises with thrust, the afterburner adds 6 dB; wind rises about 10 dB per decade of dynamic pressure | same |
| A4 | the aircraft bus ducks 6 dB and the warnings bus never does (kernel); in the browser a tone starts within 100 ms of the snapshot that raises its flag | same; `tests/e2e/audio.spec.ts` |
| A5 | the 60 s mix of a recorded circuit trace (`packages/audio/test/fixtures/circuit-60s.csv`) measures -16 +/- 1.5 LUFS and at most -1 dBTP (BS.1770, in `test-support`) | `dsp/loudness.test.ts` |
| A6 | kernel cost per quantum (informational in CI) | `dsp/flight-dsp.test.ts` |
| A7 | no allocation in `process()`: measured by V8's sampling heap profiler in plain Node, in a child process (`test-support/process-garbage.ts`), at most 64 bytes a block; an array or typed array made per block fails it | `dsp/flight-dsp.test.ts` |
| A8 | the context resumes on Fly; the null backend and the Web Audio backend pass the contract; `?selftest=audio` in the browser | `tests/e2e/audio.spec.ts` |
| AU1, AU2 | the gun's envelope repeats at 1 700 / 60 Hz within 1 %; the growl's pitch rises with the signal and the lock tone is within 1 % of the profile's | `dsp/combat-voices.test.ts` |
| AU3 | in the browser, the launch, hit and explosion sounds start within 100 ms of the counter that rose (p95 of 10 each) | `?selftest=audio`, `tests/e2e/combat.spec.ts` |
| AU4 | `process()` keeps A7's bound with every duel voice sounding | `dsp/flight-dsp.test.ts` |
| AU5 | the 60 s mix of a combat trace (`test/fixtures/combat-60s.csv`) peaks at no more than -1 dBTP in each view (measured -15.1 LUFS in the cockpit, -17.6 in chase) | `dsp/loudness.test.ts` |

A7 runs outside the test runner on purpose: the runner's module transform reaches every import through a
property lookup, which stops V8 inlining across modules and boxes numbers again, which the shipped worklet
(one bundle) does not. An earlier version of A7 measured the heap's growth over chunks of calls and could not
see an allocation made on every call; the sampling profiler attributes each allocation, short-lived ones
included (`includeObjectsCollectedByMinorGC`), to its call site. To change the kernel, run
`node packages/audio/src/test-support/process-garbage.ts` and read the sites it prints.

## What is not here

Spatial emitters, Doppler, propagation, reverb, the sonic boom, radio and voice lines (tones and HUD text
stand in, GDD 56.9), music, a 30 Hz `audio-feed` from the sim worker in message mode (the status feed uses the
4 Hz reports there) and the gear-motion, touchdown and airbrake sounds (cut list). The duel adds no missile
motor heard from outside, no incoming-missile warning and no voice calls.
