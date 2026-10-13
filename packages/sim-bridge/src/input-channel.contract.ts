/** What every input channel must do, whatever carries the bytes. */
import { afterEach, describe, expect, it } from 'vitest';
import { waitFor } from './channel-test-support.ts';
import { encodeInputFrame, decodeInputFrame, neutralFrame } from './input-frame.ts';
import {
  makeInputChannel,
  openInputSink,
  openInputSource,
  type InputSink,
  type InputSource,
} from './input-channel.ts';
import { AXIS, INPUT_FRAME, INPUT_RING } from './layout.gen.ts';

function frameBytes(seq: number): Uint8Array {
  const f = neutralFrame(seq);
  f.axes[AXIS.PITCH] = Math.fround((seq % 200) / 100 - 1);
  f.tMs = seq * 8;
  f.buttons = BigInt(seq);
  const b = new Uint8Array(INPUT_FRAME.SIZE);
  encodeInputFrame(f, b);
  return b;
}

function seqsIn(buf: Uint8Array, n: number): number[] {
  return Array.from(
    { length: n },
    (_, i) => decodeInputFrame(buf.subarray(i * INPUT_FRAME.SIZE, (i + 1) * INPUT_FRAME.SIZE)).seq,
  );
}

export function inputChannelContract(kind: 'shared' | 'message'): void {
  const open: { close(): void }[] = [];
  afterEach(() => {
    for (const o of open.splice(0)) o.close();
  });

  function channel(): { sink: InputSink; source: InputSource } {
    const { sink, source } = makeInputChannel(kind);
    const pair = { sink: openInputSink(sink), source: openInputSource(source) };
    open.push(pair.sink, pair.source);
    return pair;
  }

  describe(`input channel (${kind})`, () => {
    it('starts empty', () => {
      const { source } = channel();
      const out = new Uint8Array(8 * INPUT_FRAME.SIZE);
      expect(source.drain(out, 8)).toBe(0);
      expect([source.pending(), source.dropped()]).toEqual([0, 0]);
    });

    it('delivers frames in order, byte for byte', async () => {
      const { sink, source } = channel();
      for (let i = 0; i < 10; i++) expect(sink.send(frameBytes(i))).toBe(true);
      await waitFor(() => source.pending() === 10, '10 frames');
      const out = new Uint8Array(10 * INPUT_FRAME.SIZE);
      expect(source.drain(out, 10)).toBe(10);
      for (let i = 0; i < 10; i++) {
        expect(out.subarray(i * 96, (i + 1) * 96), `frame ${String(i)}`).toEqual(frameBytes(i));
      }
      expect(source.pending()).toBe(0);
    });

    it('drains in parts without losing order', async () => {
      const { sink, source } = channel();
      for (let i = 0; i < 20; i++) sink.send(frameBytes(i));
      await waitFor(() => source.pending() === 20, '20 frames');
      const seen: number[] = [];
      const out = new Uint8Array(8 * INPUT_FRAME.SIZE);
      for (;;) {
        const n = source.drain(out, 8);
        if (n === 0) break;
        seen.push(...seqsIn(out, n));
      }
      expect(seen).toEqual(Array.from({ length: 20 }, (_, i) => i));
    });

    it('keeps order across many send-and-drain rounds (the ring wraps)', async () => {
      const { sink, source } = channel();
      const out = new Uint8Array(8 * INPUT_FRAME.SIZE);
      let next = 0;
      let expected = 0;
      for (let round = 0; round < 60; round++) {
        for (let i = 0; i < 5; i++) sink.send(frameBytes(next++));
        await waitFor(() => source.pending() === next - expected, `round ${String(round)}`);
        const n = source.drain(out, 8);
        expect(seqsIn(out, n)).toEqual(Array.from({ length: n }, (_, i) => expected + i));
        expected += n;
      }
      expect(expected).toBe(next);
    });

    it('drops the newest frames when full, and counts them', async () => {
      const { sink, source } = channel();
      const total = INPUT_RING.CAPACITY + 36;
      for (let i = 0; i < total; i++) sink.send(frameBytes(i));
      await waitFor(
        () => source.pending() + source.dropped() === total,
        'every frame to be accounted for',
      );
      expect(source.dropped()).toBe(36);
      const out = new Uint8Array(INPUT_RING.CAPACITY * INPUT_FRAME.SIZE);
      expect(source.drain(out, INPUT_RING.CAPACITY)).toBe(INPUT_RING.CAPACITY);
      expect(seqsIn(out, INPUT_RING.CAPACITY)).toEqual(
        Array.from({ length: INPUT_RING.CAPACITY }, (_, i) => i),
      );
      // Space is free again.
      sink.send(frameBytes(1000));
      await waitFor(() => source.pending() === 1, 'a frame after the overflow');
      expect(source.drain(out, 1)).toBe(1);
      expect(seqsIn(out, 1)).toEqual([1000]);
    });

    it('refuses wrong-sized frames and buffers', () => {
      const { sink, source } = channel();
      expect(() => sink.send(new Uint8Array(INPUT_FRAME.SIZE - 1))).toThrow(RangeError);
      expect(() => source.drain(new Uint8Array(INPUT_FRAME.SIZE - 1), 1)).toThrow(RangeError);
    });
  });
}
