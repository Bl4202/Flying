/**
 * The combat readers' garbage, measured (in their own file, as the input pump's: the measurement is
 * of optimised code). The page reads the combat status every animation frame and drains the event
 * ring as often; neither may make objects.
 */
import { describe, expect, it } from 'vitest';
import { EventCursor } from './combat-events.ts';
import { newCombatStatus, readCombatStatus } from './combat-status.ts';
import { SNAPSHOT } from './layout.gen.ts';
import { SnapshotReader } from './snapshot.ts';

/** Bytes of heap a call leaves behind, the smallest of a few rounds (the pump test's method). */
async function bytesPerCall(fn: (i: number) => void): Promise<number> {
  const v8 = await import('node:v8');
  const vm = await import('node:vm');
  v8.setFlagsFromString('--expose_gc');
  const gc = vm.runInNewContext('gc') as () => void;
  const median = (xs: number[]): number => [...xs].sort((a, b) => a - b)[xs.length >> 1] ?? 0;
  const chunkSize = 2000;
  const chunk = (body: (i: number) => void): number => {
    const before = process.memoryUsage().heapUsed;
    for (let i = 0; i < chunkSize; i++) body(i);
    return process.memoryUsage().heapUsed - before;
  };
  const noop = (): void => undefined;
  let best = Number.POSITIVE_INFINITY;
  for (let round = 0; round < 6; round++) {
    for (let i = 0; i < 100_000; i++) fn(i);
    await new Promise((resolve) => setTimeout(resolve, 30));
    gc();
    const overhead = median(Array.from({ length: 41 }, () => chunk(noop)));
    const deltas = Array.from({ length: 41 }, () => chunk(fn));
    best = Math.min(best, Math.max(0, median(deltas) - overhead) / chunkSize);
    if (best < 0.5) break;
  }
  return best;
}

describe('the combat readers: garbage', () => {
  it('read the status and drain a new event each call without making objects', async () => {
    const bytes = new Uint8Array(SNAPSHOT.SIZE);
    const view = new DataView(bytes.buffer);
    const reader = new SnapshotReader(bytes);
    const status = newCombatStatus();
    const cursor = new EventCursor();
    let kinds = 0;
    const onEvent = (e: { kind: number }): void => {
      kinds += e.kind;
    };
    let seq = 0;
    const perCall = await bytesPerCall(() => {
      seq += 1;
      const at = SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * ((seq - 1) % SNAPSHOT.EVENT_CAPACITY);
      view.setUint32(at + SNAPSHOT.EV_SEQ, seq, true);
      view.setUint8(at + SNAPSHOT.EV_KIND, 4);
      view.setUint32(SNAPSHOT.C_EVENT_SEQ, seq, true);
      cursor.drain(reader, onEvent);
      readCombatStatus(reader, status);
    });
    expect(perCall).toBeLessThan(1);
    expect(kinds).toBe(4 * seq);
    expect(cursor.lost).toBe(0);
  });
});
