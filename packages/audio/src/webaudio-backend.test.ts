import { describe, expect, it } from 'vitest';
import { newAircraftStatus, OWNSHIP_FLAG, WARNING } from '@flying/sim-bridge';
import { audioBackendContract } from './audio-backend.contract.ts';
import { P, PARAM_NAMES, PARAM_RANGE } from './dsp/flight-dsp.ts';
import { kestrelProfile } from './test-support/fixtures.ts';
import {
  UPDATE_MIN_INTERVAL_MS,
  WebAudioBackend,
  type AnalyserLike,
  type AudioContextLike,
  type AudioParamLike,
  type GainNodeLike,
  type OscillatorLike,
  type WebAudioEnv,
  type WorkletNodeLike,
} from './webaudio-backend.ts';

class FakeParam implements AudioParamLike {
  value: number;
  readonly calls: string[] = [];
  constructor(value = 0) {
    this.value = value;
  }
  setTargetAtTime(target: number): void {
    this.calls.push(`target ${String(target)}`);
    this.value = target;
  }
  setValueAtTime(value: number): void {
    this.calls.push(`set ${String(value)}`);
  }
  linearRampToValueAtTime(value: number): void {
    this.calls.push(`ramp ${String(value)}`);
  }
}

class FakeNode {
  readonly connected: unknown[] = [];
  disconnected = false;
  connect(to: unknown): void {
    this.connected.push(to);
  }
  disconnect(): void {
    this.disconnected = true;
  }
}

class FakeGain extends FakeNode implements GainNodeLike {
  readonly gain = new FakeParam(1);
}

class FakeOsc extends FakeNode implements OscillatorLike {
  type = '';
  readonly frequency = new FakeParam();
  onended: (() => void) | null = null;
  started: number | undefined;
  start(when?: number): void {
    this.started = when;
  }
  stop(): void {
    this.onended?.();
  }
}

class FakeWorklet extends FakeNode implements WorkletNodeLike {
  readonly params = new Map(PARAM_NAMES.map((n, i) => [n, new FakeParam(PARAM_RANGE[i]?.[2])]));
  readonly parameters = { get: (name: string) => this.params.get(name as never) };
  readonly port: WorkletNodeLike['port'] = { onmessage: null };
}

class FakeAnalyser extends FakeNode implements AnalyserLike {
  fftSize = 0;
  /** What the output looks like to the meter: a constant. */
  sample = 0;
  getFloatTimeDomainData(buffer: Float32Array): void {
    buffer.fill(this.sample);
  }
}

class FakeContext extends FakeNode implements AudioContextLike {
  state = 'suspended';
  currentTime = 0;
  readonly destination = new FakeNode();
  modules: string[] = [];
  failModule: string | undefined;
  readonly gains: FakeGain[] = [];
  readonly oscillators: FakeOsc[] = [];
  readonly analyser = new FakeAnalyser();
  readonly audioWorklet = {
    addModule: (url: string): Promise<void> => {
      this.modules.push(url);
      return this.failModule === undefined
        ? Promise.resolve()
        : Promise.reject(new Error(this.failModule));
    },
  };
  resume(): Promise<void> {
    this.state = 'running';
    return Promise.resolve();
  }
  suspend(): Promise<void> {
    this.state = 'suspended';
    return Promise.resolve();
  }
  close(): Promise<void> {
    this.state = 'closed';
    return Promise.resolve();
  }
  createGain(): FakeGain {
    const g = new FakeGain();
    this.gains.push(g);
    return g;
  }
  createAnalyser(): FakeAnalyser {
    return this.analyser;
  }
  createOscillator(): FakeOsc {
    const o = new FakeOsc();
    this.oscillators.push(o);
    return o;
  }
}

function setup(failModule?: string) {
  const ctx = new FakeContext();
  ctx.failModule = failModule;
  const clock = { ms: 1000 };
  let node: FakeWorklet | undefined;
  const env: WebAudioEnv = {
    createContext: () => ctx,
    createWorkletNode: () => (node = new FakeWorklet()),
    now: () => clock.ms,
  };
  const backend = new WebAudioBackend({
    profile: kestrelProfile(),
    workletUrl: 'blob:flight',
    env,
  });
  const param = (i: number): number =>
    node?.params.get(PARAM_NAMES[i] as never)?.value ?? Number.NaN;
  return { ctx, clock, backend, node: () => node, param };
}

audioBackendContract('web audio, fake context', () => setup().backend);

const status = () => ({
  ...newAircraftStatus(),
  assist: 2,
  engineCore: 0.6,
  engineBits: 1,
  qbar: 4000,
  flags: OWNSHIP_FLAG.STALL_WARN,
});

describe('WebAudioBackend', () => {
  it('loads the worklet at construction, and stays suspended until resumed', async () => {
    const { ctx, backend } = setup();
    expect(ctx.modules).toEqual(['blob:flight']);
    expect(ctx.state).toBe('suspended');
    await backend.whenReady();
    expect(ctx.state).toBe('suspended');
    expect(backend.state).toBe('suspended');
    await backend.resume();
    expect(ctx.state).toBe('running');
  });

  it('wires the worklet and the cues through the master gain to the output', async () => {
    const { ctx, backend, node } = setup();
    await backend.whenReady();
    const [master, ui] = ctx.gains;
    expect(ui?.connected).toEqual([master]);
    expect(master?.connected).toEqual([ctx.destination, ctx.analyser]); // the analyser is the level meter
    expect(node()?.connected).toEqual([master]);
  });

  it('writes the aircraft state to the worklet parameters, and only what changed', async () => {
    const { backend, param, node, clock } = setup();
    await backend.whenReady();
    backend.update(status());
    expect(param(P.CORE)).toBe(0.6);
    expect(param(P.QBAR)).toBe(4000);
    expect(param(P.WARNINGS)).toBe(WARNING.STALL);
    const writes = backend.paramWrites;
    expect(writes).toBeGreaterThan(3);
    // The same state again, later: nothing to write.
    clock.ms += 100;
    backend.update(status());
    expect(backend.paramWrites).toBe(writes);
    // One value changes: one write.
    clock.ms += 100;
    backend.update({ ...status(), qbar: 5000 });
    expect(backend.paramWrites).toBe(writes + 1);
    expect(node()?.params.get('qbar')?.value).toBe(5000);
  });

  it('passes updates on at most about 80 times a second', async () => {
    const { backend, param, clock } = setup();
    await backend.whenReady();
    backend.update({ ...status(), qbar: 1000 });
    clock.ms += UPDATE_MIN_INTERVAL_MS - 2;
    backend.update({ ...status(), qbar: 2000 });
    expect(param(P.QBAR)).toBe(1000);
    clock.ms += 3;
    backend.update({ ...status(), qbar: 3000 });
    expect(param(P.QBAR)).toBe(3000);
  });

  it('applies the camera and the bus volumes at once, without waiting for an update', async () => {
    const { backend, param, ctx } = setup();
    await backend.whenReady();
    backend.update(status());
    expect(param(P.COCKPIT)).toBe(1);
    backend.setCamera('chase');
    expect(param(P.COCKPIT)).toBe(0);
    backend.setBuses({ aircraft: 0.5, warnings: 0.25, ui: 0.4, master: 0.8 });
    expect(param(P.AIRCRAFT_BUS)).toBe(0.5);
    expect(param(P.WARNINGS_BUS)).toBe(0.25);
    expect(backend.volume).toBe(0.8);
    expect(ctx.gains[1]?.gain.value).toBe(0.4); // the UI bus
    expect(ctx.gains[0]?.gain.value).toBe(0.8); // the master
    backend.setBuses({ aircraft: 7, warnings: Number.NaN });
    expect(param(P.AIRCRAFT_BUS)).toBe(1);
    expect(param(P.WARNINGS_BUS)).toBe(0);
  });

  it('mutes without losing the volume, and unmutes to it', async () => {
    const { backend, ctx } = setup();
    await backend.whenReady();
    backend.setVolume(0.6);
    backend.setMuted(true);
    expect(ctx.gains[0]?.gain.value).toBe(0);
    expect(backend.volume).toBe(0.6);
    backend.setMuted(false);
    expect(ctx.gains[0]?.gain.value).toBe(0.6);
  });

  it('is silent when there is no status: the engine off', async () => {
    const { backend, param, clock } = setup();
    await backend.whenReady();
    backend.update(status());
    expect(param(P.ENGINE_BITS)).toBe(1);
    clock.ms += 100;
    backend.update(undefined);
    expect(param(P.ENGINE_BITS)).toBe(0);
    expect(param(P.CORE)).toBe(0);
    expect(param(P.WARNINGS)).toBe(0);
  });

  it('reports a tone onset from the worklet, and ignores other messages', async () => {
    const { backend, node } = setup();
    await backend.whenReady();
    const heard: { tone: number; time: number }[] = [];
    const off = backend.onOnset((o) => heard.push(o));
    node()?.port.onmessage?.({ data: { type: 'onset', tone: 2, time: 1.5 } });
    node()?.port.onmessage?.({ data: { type: 'other' } });
    node()?.port.onmessage?.({ data: 'nonsense' });
    node()?.port.onmessage?.({ data: null });
    expect(heard).toEqual([{ tone: 2, time: 1.5 }]);
    off();
    node()?.port.onmessage?.({ data: { type: 'onset', tone: 3, time: 2 } });
    expect(heard).toHaveLength(1);
  });

  it('plays cues through the UI bus when running, and not before', async () => {
    const { backend, ctx } = setup();
    await backend.whenReady();
    backend.cue('ui-confirm');
    expect(ctx.oscillators).toHaveLength(0);
    await backend.resume();
    backend.cue('ui-confirm');
    expect(ctx.oscillators.map((o) => o.frequency.value)).toEqual([660, 880]);
    backend.cue('warning');
    expect(ctx.oscillators).toHaveLength(4); // two pulses and a rest
    backend.cue('nonsense' as never);
    expect(ctx.oscillators).toHaveLength(4);
    // Each note's oscillator is released when it ends.
    expect(ctx.oscillators.every((o) => o.disconnected)).toBe(true);
  });

  it('keeps working for cues if the worklet cannot load, and says why', async () => {
    const { backend, ctx } = setup('CSP blocked it');
    await backend.whenReady();
    expect(backend.workletError).toBe('CSP blocked it');
    backend.update(status());
    expect(backend.paramWrites).toBe(0);
    await backend.resume();
    expect(backend.state).toBe('running');
    backend.cue('ui-error');
    expect(ctx.oscillators).toHaveLength(1);
  });

  it('does nothing after close, and releases the graph', async () => {
    const { backend, ctx, node } = setup();
    await backend.whenReady();
    const writes = backend.paramWrites;
    await backend.close();
    expect(ctx.state).toBe('closed');
    expect(node()?.disconnected).toBe(true);
    expect(backend.state).toBe('closed');
    backend.update(status());
    backend.setBuses({ aircraft: 0.1 });
    backend.setCamera('chase');
    backend.setMuted(true);
    expect(backend.paramWrites).toBe(writes);
    await backend.close();
  });

  it('can be closed while the worklet is still loading', async () => {
    const { backend, node } = setup();
    await backend.close();
    await backend.whenReady();
    expect(node()).toBeUndefined();
    await backend.resume();
    expect(backend.state).toBe('closed');
  });

  it('reads the output level and the audio clock', async () => {
    const { backend, ctx } = setup();
    await backend.whenReady();
    expect(ctx.analyser.fftSize).toBe(2048);
    expect(backend.levelDb()).toBe(-200);
    ctx.analyser.sample = 0.1;
    expect(backend.levelDb()).toBeCloseTo(-20, 6);
    ctx.currentTime = 12.5;
    expect(backend.audioTime).toBe(12.5);
  });

  it('treats an interrupted context (Safari) as suspended', async () => {
    const { backend, ctx } = setup();
    await backend.resume();
    ctx.state = 'interrupted';
    expect(backend.state).toBe('suspended');
  });
});
