/**
 * What every snapshot channel must do, whatever carries the bytes. `snapshotChannelContract` is
 * called once per implementation from `snapshot-channel.test.ts`.
 */
import { afterEach, describe, expect, it } from 'vitest';
import { waitFor } from './channel-test-support.ts';
import { SNAPSHOT } from './layout.gen.ts';
import {
  CONTENDED,
  makeSnapshotChannel,
  openSnapshotSink,
  openSnapshotSource,
  type SnapshotSink,
  type SnapshotSource,
} from './snapshot-channel.ts';

/** A snapshot-sized buffer whose first four bytes are `n` and whose other bytes are all `n & 0xff`. */
function snapshotNumber(n: number): Uint8Array {
  const b = new Uint8Array(SNAPSHOT.SIZE).fill(n & 0xff);
  new DataView(b.buffer).setUint32(0, n, true);
  return b;
}

function numberOf(b: Uint8Array): number {
  return new DataView(b.buffer, b.byteOffset).getUint32(0, true);
}

function isWhole(b: Uint8Array): boolean {
  const fill = numberOf(b) & 0xff;
  for (let i = 4; i < b.length; i++) {
    if (b[i] !== fill) return false;
  }
  return true;
}

export function snapshotChannelContract(kind: 'shared' | 'message'): void {
  const open: { close(): void }[] = [];
  afterEach(() => {
    for (const o of open.splice(0)) o.close();
  });

  function channel(): { sink: SnapshotSink; source: SnapshotSource } {
    const { sink, source } = makeSnapshotChannel(kind);
    const pair = { sink: openSnapshotSink(sink), source: openSnapshotSource(source) };
    open.push(pair.sink, pair.source);
    return pair;
  }

  describe(`snapshot channel (${kind})`, () => {
    it('has nothing to read before the first publication', () => {
      const { source } = channel();
      const out = new Uint8Array(SNAPSHOT.SIZE).fill(0xee);
      expect(source.read(out)).toBe(0);
      expect(out.every((b) => b === 0xee)).toBe(true);
    });

    it('delivers a published snapshot byte for byte', async () => {
      const { sink, source } = channel();
      const sent = snapshotNumber(1);
      for (let i = 100; i < 200; i++) sent[i] = (i * 7) & 0xff;
      sink.publish(sent);
      const out = new Uint8Array(SNAPSHOT.SIZE);
      await waitFor(() => source.read(out) > 0, 'the first snapshot');
      expect(out).toEqual(sent);
    });

    it('lets the latest publication win', async () => {
      const { sink, source } = channel();
      for (let n = 1; n <= 5; n++) sink.publish(snapshotNumber(n));
      const out = new Uint8Array(SNAPSHOT.SIZE);
      await waitFor(() => source.read(out) === 5, 'publication 5');
      expect(numberOf(out)).toBe(5);
      expect(isWhole(out)).toBe(true);
    });

    it('returns the same publication again when nothing new was published', async () => {
      const { sink, source } = channel();
      sink.publish(snapshotNumber(1));
      const a = new Uint8Array(SNAPSHOT.SIZE);
      await waitFor(() => source.read(a) === 1, 'publication 1');
      const b = new Uint8Array(SNAPSHOT.SIZE);
      expect(source.read(b)).toBe(1);
      expect(b).toEqual(a);
    });

    it('numbers publications upward', async () => {
      const { sink, source } = channel();
      const out = new Uint8Array(SNAPSHOT.SIZE);
      let last = 0;
      for (let n = 1; n <= 20; n++) {
        sink.publish(snapshotNumber(n));
        await waitFor(() => source.read(out) >= n, `publication ${String(n)}`);
        const got = source.read(out);
        expect(got).toBeGreaterThanOrEqual(last);
        last = got;
      }
      expect(last).toBe(20);
    });

    it('lets the sink stamp the published copy but not the caller’s buffer', async () => {
      const { sink, source } = channel();
      const sent = snapshotNumber(9);
      sink.publish(sent, (copy) => {
        copy[8] = 0x42;
      });
      const out = new Uint8Array(SNAPSHOT.SIZE);
      await waitFor(() => source.read(out) > 0, 'the stamped snapshot');
      expect(out[8]).toBe(0x42);
      expect(sent[8]).toBe(9);
    });

    it('never delivers a torn or out-of-order snapshot under a stream of publications', async () => {
      const { sink, source } = channel();
      const out = new Uint8Array(SNAPSHOT.SIZE);
      let lastNumber = 0;
      let reads = 0;
      for (let n = 1; n <= 3000; n++) {
        sink.publish(snapshotNumber(n));
        if (n % 3 === 0) {
          const got = source.read(out);
          if (got > 0) {
            reads += 1;
            expect(isWhole(out), `snapshot ${String(numberOf(out))} is whole`).toBe(true);
            expect(numberOf(out)).toBeGreaterThanOrEqual(lastNumber);
            lastNumber = numberOf(out);
          } else {
            expect(got === 0 || got === CONTENDED).toBe(true);
          }
        }
        if (n % 50 === 0) await new Promise<void>((r) => setTimeout(r, 0));
      }
      await waitFor(() => source.read(out) > 0 && numberOf(out) === 3000, 'the last snapshot');
      expect(reads).toBeGreaterThan(0);
      expect(isWhole(out)).toBe(true);
    });

    it('refuses wrong-sized buffers', () => {
      const { sink, source } = channel();
      expect(() => {
        sink.publish(new Uint8Array(SNAPSHOT.SIZE - 1));
      }).toThrow(RangeError);
      expect(() => source.read(new Uint8Array(SNAPSHOT.SIZE - 1))).toThrow(RangeError);
    });
  });
}
