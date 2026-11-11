/** What the page and the flight worklet agree on. Safe to import on either side: it registers nothing. */

/** The name the processor is registered under. */
export const PROCESSOR_NAME = 'fg-flight';

/**
 * Sent through the worklet's port when a warning tone, or one of the duel's one-shot sounds, starts.
 */
export interface ToneOnsetMessage {
  readonly type: 'onset';
  /** `TONE` value, or an `ONSET` code for one of the duel's one-shot sounds. */
  readonly tone: number;
  /** The audio clock, seconds, at the start of the tone. */
  readonly time: number;
}

export function isToneOnset(data: unknown): data is ToneOnsetMessage {
  if (typeof data !== 'object' || data === null) return false;
  const d = data as Record<string, unknown>;
  return d['type'] === 'onset' && typeof d['tone'] === 'number' && typeof d['time'] === 'number';
}
