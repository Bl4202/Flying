/**
 * `?selftest=audio`: the flight audio in this browser, end to end. The real `AudioContext`, the real
 * worklet (built by Vite, loaded through `scriptUrl()` under the page's CSP and Trusted Types), the
 * real parameter feed. Rows, each with a stable test id and a `data-result`:
 *
 * | id | what |
 * |---|---|
 * | `worklet` | the module loaded and the processor was created |
 * | `context` | the context runs at 48 kHz (waits for a gesture if the browser asks for one) |
 * | `engine` | the output is louder at military power than at idle, and quiet when muted |
 * | `onset` | a warning tone starts within 100 ms of the state that raises it (p95 of 20 events, AC-56.5) |
 * | `onset-launch`, `onset-hit`, `onset-explosion` | the duel's one-shot sound starts within 100 ms of the status whose counter rose (p95 of 10 events each, ADR-45 AU3) |
 */
import {
  audioProfileFor,
  ONSET,
  resumeOnFirstGesture,
  WebAudioBackend,
  type FlightAudio,
} from '@flying/audio';
import flightWorkletUrl from '@flying/audio/flight-processor?worker&url';
import { scriptUrl } from '@flying/platform-web';
import {
  COMBAT_COUNTER,
  COMBAT_FLAG,
  newAircraftStatus,
  newCombatStatus,
  OWNSHIP_FLAG,
  type AircraftStatus,
  type CombatStatus,
} from '@flying/sim-bridge';
import { loadPresentBundle } from './present-bundle.ts';
import { mountSelfTestTable, type RowResult } from './selftest-table.ts';

const TRIALS = 20;
/** Events per one-shot sound (AU3). */
const COMBAT_TRIALS = 10;
const ONSET_BUDGET_MS = 100;

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** The ownship at one engine state, optionally with the stall warning raised. */
function status(core: number, stall = false): AircraftStatus {
  return {
    ...newAircraftStatus(),
    assist: 2,
    engineCore: core,
    engineBits: 1,
    qbar: 0, // still air, so the wind does not mask the engine
    flags: stall ? OWNSHIP_FLAG.STALL_WARN : 0,
  };
}

/** Feeds `s` for `ms` milliseconds (the feed is throttled, as in the game), then reads the level. */
async function settle(
  audio: FlightAudio & { levelDb(): number },
  s: AircraftStatus,
  ms: number,
  combat?: CombatStatus,
): Promise<number> {
  const until = performance.now() + ms;
  while (performance.now() < until) {
    audio.update(s, combat);
    await sleep(16);
  }
  return audio.levelDb();
}

/** Waits for the context to run, or a gesture to start it; false if neither came in `ms`. */
async function running(audio: WebAudioBackend, ms: number): Promise<boolean> {
  const stop = resumeOnFirstGesture(audio, window);
  try {
    void audio.resume();
    const until = performance.now() + ms;
    while (audio.state !== 'running' && performance.now() < until) await sleep(50);
    return audio.state === 'running';
  } finally {
    stop();
  }
}

function p95(xs: number[]): number {
  const sorted = [...xs].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * 0.95) - 1)] ?? Number.NaN;
}

export async function runAudioSelfTest(root: HTMLElement): Promise<void> {
  const table = mountSelfTestTable(root);
  const row = (id: string, label: string, text: string, result: RowResult): void => {
    table.row(id, label, text, result);
  };
  try {
    const profile = audioProfileFor(await loadPresentBundle(), 'core:ac/kestrel');
    if (profile === undefined) throw new Error('the bundle has no Kestrel audio profile');
    const audio = new WebAudioBackend({ profile, workletUrl: scriptUrl(flightWorkletUrl) });
    await audio.whenReady();
    if (audio.workletError !== undefined) {
      row('worklet', 'Worklet', `fail: ${audio.workletError}`, 'fail');
      table.done();
      return;
    }
    row('worklet', 'Worklet', 'pass (loaded, processor created)', 'pass');

    // The browser may keep the context suspended until a gesture: a press of any key, or a click, runs it.
    if (!(await running(audio, 3000))) {
      row('context', 'Context', 'skipped: suspended, no user gesture in this browser', 'skipped');
      table.done();
      return;
    }
    row('context', 'Context', `pass (running)`, 'pass');

    // The engine: louder at military power than at idle, silent when muted.
    audio.setCamera('chase');
    const idle = await settle(audio, status(0.06), 1500);
    const military = await settle(audio, status(1), 2500);
    audio.setMuted(true);
    const muted = await settle(audio, status(1), 400);
    audio.setMuted(false);
    const engineOk = military > idle + 3 && military > -60 && muted < -80;
    row(
      'engine',
      'Engine level',
      `${engineOk ? 'pass' : 'fail'}: idle ${idle.toFixed(1)} dB, military ${military.toFixed(1)} dB, muted ${muted.toFixed(1)} dB`,
      engineOk ? 'pass' : 'fail',
    );

    // A tone starts within 100 ms of the state that raises it.
    const latencies: number[] = [];
    const onsets: number[] = [];
    audio.onOnset((o) => {
      onsets.push(o.time);
    });
    for (let i = 0; i < TRIALS; i++) {
      await settle(audio, status(0.6), 250);
      onsets.length = 0;
      const t0 = audio.audioTime;
      const until = performance.now() + 500;
      while (onsets.length === 0 && performance.now() < until) {
        audio.update(status(0.6, true));
        await sleep(2);
      }
      const first = onsets[0];
      if (first !== undefined) latencies.push((first - t0) * 1000);
    }
    const slow = p95(latencies);
    const onsetOk = latencies.length === TRIALS && slow < ONSET_BUDGET_MS;
    row(
      'onset',
      'Warning tone onset',
      `${onsetOk ? 'pass' : 'fail'}: p95 ${slow.toFixed(1)} ms over ${String(latencies.length)}/${String(TRIALS)} events (limit ${String(ONSET_BUDGET_MS)} ms)`,
      onsetOk ? 'pass' : 'fail',
    );

    // The duel's one-shot sounds start within 100 ms of the status whose counter rose (AU3).
    const shots: [string, string, number, number][] = [
      ['onset-launch', 'Launch onset', COMBAT_COUNTER.MISSILES_FIRED, ONSET.LAUNCH],
      ['onset-hit', 'Hit-taken onset', COMBAT_COUNTER.HITS_TAKEN, ONSET.HIT],
      ['onset-explosion', 'Explosion onset', COMBAT_COUNTER.EXPLOSIONS, ONSET.EXPLOSION],
    ];
    for (const [id, label, counter, code] of shots) {
      const duel = newCombatStatus();
      duel.flags = COMBAT_FLAG.ACTIVE;
      duel.explosionRange = 800;
      const times: number[] = [];
      for (let i = 0; i < COMBAT_TRIALS; i++) {
        duel.counters[counter] = i;
        // Long enough for the explosion and the launch to die away between events.
        await settle(audio, status(0.6), 300, duel);
        onsets.length = 0;
        const heard: number[] = [];
        const off = audio.onOnset((o) => {
          if (o.tone === code) heard.push(o.time);
        });
        const t0 = audio.audioTime;
        duel.counters[counter] = i + 1;
        const until = performance.now() + 500;
        while (heard.length === 0 && performance.now() < until) {
          audio.update(status(0.6), duel);
          await sleep(2);
        }
        off();
        const first = heard[0];
        if (first !== undefined) times.push((first - t0) * 1000);
      }
      const p = p95(times);
      const ok = times.length === COMBAT_TRIALS && p < ONSET_BUDGET_MS;
      row(
        id,
        label,
        `${ok ? 'pass' : 'fail'}: p95 ${p.toFixed(1)} ms over ${String(times.length)}/${String(COMBAT_TRIALS)} events (limit ${String(ONSET_BUDGET_MS)} ms)`,
        ok ? 'pass' : 'fail',
      );
    }
    await audio.close();
  } catch (e) {
    row('error', 'Error', `fail: ${e instanceof Error ? e.message : String(e)}`, 'fail');
  }
  table.done();
}
