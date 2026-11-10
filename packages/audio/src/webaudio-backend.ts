/**
 * The Web Audio backend (ADR-42): an `AudioContext` at 48 kHz holding the flight worklet, a master
 * gain and a UI bus for short cues. It implements the `AudioBackend` contract and the `FlightAudio`
 * additions. The context is created with the backend and stays suspended until `resume()`, which the
 * game calls from the Fly click (a user gesture); it is never asked to run before.
 *
 * Everything that touches the browser comes through `WebAudioEnv`, so the state machine and the
 * parameter feed are tested in Node with a fake context, and the real one in the browser
 * (`?selftest=audio`).
 */
import type { AircraftStatus, CombatStatus } from '@flying/sim-bridge';
import { clampVolume, type AudioCue, type AudioState } from './backend.ts';
import { PARAM_COUNT, PARAM_NAMES, PARAM_RANGE } from './dsp/flight-dsp.ts';
import { defaultBuses, fillParams, type BusLevels, type CameraMode } from './feed.ts';
import type { FlightAudio, ToneOnset } from './flight-audio.ts';
import type { AudioProfile } from './profile.ts';
import { isToneOnset, PROCESSOR_NAME } from './worklet/protocol.ts';

export interface AudioParamLike {
  value: number;
  setTargetAtTime(target: number, startTime: number, timeConstant: number): unknown;
  setValueAtTime(value: number, time: number): unknown;
  linearRampToValueAtTime(value: number, time: number): unknown;
}
export interface AudioNodeLike {
  connect(destination: AudioNodeLike): unknown;
  disconnect(): void;
}
export interface GainNodeLike extends AudioNodeLike {
  readonly gain: AudioParamLike;
}
export interface OscillatorLike extends AudioNodeLike {
  type: string;
  readonly frequency: AudioParamLike;
  onended: (() => void) | null;
  start(when?: number): void;
  stop(when?: number): void;
}
export interface WorkletNodeLike extends AudioNodeLike {
  readonly parameters: { get(name: string): AudioParamLike | undefined };
  readonly port: { onmessage: ((event: { data: unknown }) => void) | null };
}
export interface AnalyserLike extends AudioNodeLike {
  fftSize: number;
  getFloatTimeDomainData(buffer: Float32Array): void;
}
export interface AudioContextLike {
  readonly state: string;
  readonly currentTime: number;
  readonly destination: AudioNodeLike;
  readonly audioWorklet: { addModule(url: string): Promise<unknown> };
  resume(): Promise<void>;
  suspend(): Promise<void>;
  close(): Promise<void>;
  createGain(): GainNodeLike;
  createOscillator(): OscillatorLike;
  createAnalyser(): AnalyserLike;
}

/** Everything the backend needs from the browser. */
export interface WebAudioEnv {
  createContext(): AudioContextLike;
  createWorkletNode(ctx: AudioContextLike, profile: AudioProfile): WorkletNodeLike;
  /** Milliseconds, monotonic. */
  now(): number;
}

/** Whether this browser can run the flight audio at all. */
export function webAudioSupported(): boolean {
  return typeof AudioContext !== 'undefined' && typeof AudioWorkletNode !== 'undefined';
}

/** The real thing. */
export function browserEnv(): WebAudioEnv {
  return {
    createContext: () =>
      new AudioContext({
        sampleRate: 48_000,
        latencyHint: 'interactive',
      }) as unknown as AudioContextLike,
    createWorkletNode: (ctx, profile) =>
      new AudioWorkletNode(ctx as unknown as BaseAudioContext, PROCESSOR_NAME, {
        numberOfInputs: 0,
        numberOfOutputs: 1,
        outputChannelCount: [2],
        processorOptions: { profile },
      }) as unknown as WorkletNodeLike,
    now: () => performance.now(),
  };
}

export interface WebAudioOptions {
  profile: AudioProfile;
  /** URL of the built worklet module, already passed through `scriptUrl()`. */
  workletUrl: string;
  env?: WebAudioEnv;
}

/** The parameters are passed on at most this often (about 80 times a second). */
export const UPDATE_MIN_INTERVAL_MS = 12;

/** Short sounds: `[hz, seconds]` notes (0 Hz is a rest), and their level. */
const CUES: Record<AudioCue, { notes: readonly (readonly [number, number])[]; level: number }> = {
  'ui-confirm': {
    notes: [
      [660, 0.06],
      [880, 0.09],
    ],
    level: 0.12,
  },
  'ui-error': { notes: [[240, 0.16]], level: 0.14 },
  warning: {
    notes: [
      [800, 0.1],
      [0, 0.05],
      [800, 0.1],
    ],
    level: 0.16,
  },
};

export class WebAudioBackend implements FlightAudio {
  readonly kind = 'webaudio';
  /** Why the worklet could not be loaded, if it could not; cues still work. */
  workletError: string | undefined;
  /** How many `AudioParam` values have been written (for tests and the overlay). */
  paramWrites = 0;
  /**
   * The audio nodes made here and not yet disconnected: the soak's leak gate L6 (ADR-45) wants it
   * the same at every sortie start. A cue's two nodes count until its note ends.
   */
  liveNodes = 0;

  readonly #env: WebAudioEnv;
  readonly #ctx: AudioContextLike;
  readonly #master: GainNodeLike;
  readonly #ui: GainNodeLike;
  readonly #analyser: AnalyserLike;
  readonly #meterBuffer = new Float32Array(2048);
  readonly #ready: Promise<void>;
  readonly #listeners = new Set<(onset: ToneOnset) => void>();
  readonly #values = Float64Array.from(PARAM_RANGE, (r) => r[2]);
  readonly #written = new Float64Array(PARAM_COUNT).fill(Number.NaN);
  readonly #params: (AudioParamLike | undefined)[] = new Array<AudioParamLike | undefined>(
    PARAM_COUNT,
  ).fill(undefined);
  #node: WorkletNodeLike | undefined;
  #volume = 1;
  #muted = false;
  #camera: CameraMode = 'cockpit';
  #buses: BusLevels = defaultBuses();
  #status: AircraftStatus | undefined;
  #combat: CombatStatus | undefined;
  #lastWriteMs = Number.NEGATIVE_INFINITY;
  #closed = false;

  constructor(options: WebAudioOptions) {
    this.#env = options.env ?? browserEnv();
    this.#ctx = this.#env.createContext();
    this.#master = this.#made(this.#ctx.createGain());
    this.#ui = this.#made(this.#ctx.createGain());
    this.#analyser = this.#made(this.#ctx.createAnalyser());
    this.#analyser.fftSize = this.#meterBuffer.length;
    this.#ui.connect(this.#master);
    this.#master.connect(this.#ctx.destination);
    this.#master.connect(this.#analyser);
    this.#ready = this.#loadWorklet(options);
  }

  get state(): AudioState {
    if (this.#closed) return 'closed';
    const s = this.#ctx.state;
    return s === 'running' ? 'running' : s === 'closed' ? 'closed' : 'suspended';
  }

  get volume(): number {
    return this.#volume;
  }

  async #loadWorklet(options: WebAudioOptions): Promise<void> {
    try {
      await this.#ctx.audioWorklet.addModule(options.workletUrl);
      if (this.#closed) return;
      const node = this.#made(this.#env.createWorkletNode(this.#ctx, options.profile));
      node.connect(this.#master);
      node.port.onmessage = (event) => {
        if (!isToneOnset(event.data)) return;
        const onset: ToneOnset = { tone: event.data.tone, time: event.data.time };
        for (const listener of this.#listeners) listener(onset);
      };
      PARAM_NAMES.forEach((name, i) => {
        this.#params[i] = node.parameters.get(name);
      });
      this.#node = node;
      this.#write();
    } catch (error) {
      this.workletError = error instanceof Error ? error.message : String(error);
    }
  }

  /** The audio context's clock, seconds: the time base of `ToneOnset.time`. */
  get audioTime(): number {
    return this.#ctx.currentTime;
  }

  /** The output's level over the last 2048 samples, dB relative to full scale (-200 when silent). */
  levelDb(): number {
    const buffer = this.#meterBuffer;
    this.#analyser.getFloatTimeDomainData(buffer);
    let sum = 0;
    for (let i = 0; i < buffer.length; i++) sum += (buffer[i] as number) ** 2;
    return 10 * Math.log10(Math.max(sum / buffer.length, 1e-20));
  }

  /** Resolves when the worklet has loaded or failed to; `workletError` says which. */
  whenReady(): Promise<void> {
    return this.#ready;
  }

  async resume(): Promise<void> {
    if (this.#closed) return;
    await this.#ready;
    if (this.state === 'closed') return; // closed while the worklet was loading
    await this.#ctx.resume();
  }

  async suspend(): Promise<void> {
    if (this.#closed) return;
    await this.#ctx.suspend();
  }

  async close(): Promise<void> {
    if (this.#closed) return;
    this.#closed = true;
    this.#listeners.clear();
    try {
      if (this.#node !== undefined) this.#disconnect(this.#node);
      this.#disconnect(this.#master);
      this.#disconnect(this.#analyser);
      this.#disconnect(this.#ui);
      await this.#ctx.close();
    } catch {
      // Already closed by the browser: nothing left to release.
    }
  }

  setVolume(volume: number): void {
    if (this.#closed) return;
    this.#volume = clampVolume(volume);
    this.#buses.master = this.#volume;
    this.#applyMaster();
  }

  setMuted(muted: boolean): void {
    if (this.#closed) return;
    this.#muted = muted;
    this.#applyMaster();
  }

  setCamera(camera: CameraMode): void {
    this.#camera = camera;
    this.#write();
  }

  setBuses(levels: Partial<BusLevels>): void {
    if (this.#closed) return;
    if (levels.master !== undefined) this.setVolume(levels.master);
    if (levels.aircraft !== undefined) this.#buses.aircraft = clampVolume(levels.aircraft);
    if (levels.warnings !== undefined) this.#buses.warnings = clampVolume(levels.warnings);
    if (levels.ui !== undefined) {
      this.#buses.ui = clampVolume(levels.ui);
      this.#ui.gain.setTargetAtTime(this.#buses.ui, this.#ctx.currentTime, 0.02);
    }
    this.#write();
  }

  update(status: AircraftStatus | undefined, combat?: CombatStatus): void {
    this.#status = status;
    this.#combat = combat;
    const now = this.#env.now();
    if (now - this.#lastWriteMs < UPDATE_MIN_INTERVAL_MS) return;
    this.#lastWriteMs = now;
    this.#write();
  }

  onOnset(listener: (onset: ToneOnset) => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  cue(name: AudioCue): void {
    if (this.state !== 'running') return;
    // A cue this backend does not know is ignored (the set grows with the game).
    const cue = (CUES as Partial<Record<string, (typeof CUES)[AudioCue]>>)[name];
    if (cue === undefined) return;
    let t = this.#ctx.currentTime;
    for (const [hz, seconds] of cue.notes) {
      if (hz > 0) {
        const osc = this.#made(this.#ctx.createOscillator());
        const env = this.#made(this.#ctx.createGain());
        osc.type = 'sine';
        osc.frequency.value = hz;
        env.gain.setValueAtTime(0, t);
        env.gain.linearRampToValueAtTime(cue.level, t + 0.006);
        env.gain.linearRampToValueAtTime(0, t + seconds);
        osc.connect(env);
        env.connect(this.#ui);
        osc.onended = () => {
          this.#disconnect(osc);
          this.#disconnect(env);
        };
        osc.start(t);
        osc.stop(t + seconds + 0.02);
      }
      t += seconds;
    }
  }

  /** Counts a node made. */
  #made<T extends AudioNodeLike>(node: T): T {
    this.liveNodes += 1;
    return node;
  }

  /** Disconnects a node and stops counting it. */
  #disconnect(node: AudioNodeLike): void {
    this.liveNodes -= 1;
    node.disconnect();
  }

  #applyMaster(): void {
    this.#master.gain.setTargetAtTime(this.#muted ? 0 : this.#volume, this.#ctx.currentTime, 0.02);
  }

  /** Computes the parameters from the last status and writes those that changed. */
  #write(): void {
    if (this.#node === undefined || this.#closed) return;
    const v = this.#values;
    fillParams(
      v,
      this.#status,
      this.#combat,
      this.#camera,
      this.#buses.aircraft,
      this.#buses.warnings,
    );
    for (let i = 0; i < PARAM_COUNT; i++) {
      const value = v[i] as number;
      if (value === this.#written[i]) continue;
      const param = this.#params[i];
      if (param !== undefined) {
        param.value = value;
        this.paramWrites += 1;
      }
      this.#written[i] = value;
    }
  }
}
