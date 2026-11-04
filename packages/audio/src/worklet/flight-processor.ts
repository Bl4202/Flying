/**
 * The flight audio's `AudioWorkletProcessor` (ADR-42). It runs `FlightDsp` on the audio thread: the
 * page sets the worklet's k-rate `AudioParam`s (see `PARAM_NAMES`) and this copies them into the
 * synthesis once per render quantum. When a warning tone starts it tells the page, with the audio
 * clock's time, so the page can measure how long the tone took (AC-56.5).
 *
 * Built by Vite as a same-origin module (`?worker&url`) and loaded through `scriptUrl()`. It knows
 * nothing but the DSP and the profile it is given in `processorOptions`.
 */
import { FlightDsp, PARAM_COUNT, PARAM_NAMES, PARAM_RANGE } from '../dsp/flight-dsp.ts';
import type { AudioProfile } from '../profile.ts';
import { PROCESSOR_NAME, type ToneOnsetMessage } from './protocol.ts';

// The worklet's global scope, which TypeScript's DOM library does not describe.
interface AudioParamDescriptor {
  name: string;
  defaultValue: number;
  minValue: number;
  maxValue: number;
  automationRate: 'a-rate' | 'k-rate';
}
declare const sampleRate: number;
declare const currentTime: number;
declare class AudioWorkletProcessor {
  readonly port: MessagePort;
  constructor(options?: unknown);
}
declare function registerProcessor(name: string, processor: new (options: never) => unknown): void;

interface ProcessorOptions {
  processorOptions?: { profile?: AudioProfile };
}

class FlightProcessor extends AudioWorkletProcessor {
  static get parameterDescriptors(): AudioParamDescriptor[] {
    return PARAM_NAMES.map((name, i) => {
      const [minValue, maxValue, defaultValue] = PARAM_RANGE[i] as readonly [
        number,
        number,
        number,
      ];
      return { name, minValue, maxValue, defaultValue, automationRate: 'k-rate' as const };
    });
  }

  readonly #dsp: FlightDsp | undefined;

  constructor(options: ProcessorOptions) {
    super();
    const profile = options.processorOptions?.profile;
    this.#dsp = profile === undefined ? undefined : new FlightDsp(profile, sampleRate);
  }

  process(
    _inputs: Float32Array[][],
    outputs: Float32Array[][],
    parameters: Record<string, Float32Array>,
  ): boolean {
    const dsp = this.#dsp;
    const channels = outputs[0];
    const left = channels?.[0];
    if (dsp === undefined || left === undefined) return true;
    const right = channels?.[1] ?? left;
    const p = dsp.params;
    for (let i = 0; i < PARAM_COUNT; i++) {
      p[i] = (parameters[PARAM_NAMES[i] as string] as Float32Array)[0] as number;
    }
    dsp.process(left, right, left.length);
    if (dsp.onsetFrame >= 0) {
      const message: ToneOnsetMessage = {
        type: 'onset',
        tone: dsp.tone,
        time: currentTime + dsp.onsetFrame / sampleRate,
      };
      dsp.onsetFrame = -1;
      this.port.postMessage(message);
    }
    return true;
  }
}

registerProcessor(PROCESSOR_NAME, FlightProcessor);
