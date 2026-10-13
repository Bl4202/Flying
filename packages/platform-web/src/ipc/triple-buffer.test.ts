import { Worker } from 'node:worker_threads';
import { describe, expect, it } from 'vitest';
import { TEST_SEQLOCK } from './fixtures.ts';
import {
  CONTENDED,
  initSeqlock,
  SeqlockReader,
  SeqlockWriter,
  seqlockBytes,
  type SeqlockLayout,
} from './triple-buffer.ts';

function make(layout: SeqlockLayout = TEST_SEQLOCK) {
  const sab = new SharedArrayBuffer(seqlockBytes(layout));
  initSeqlock(sab, layout);
  return {
    sab,
    writer: new SeqlockWriter(sab, layout),
    reader: new SeqlockReader(sab, layout),
  };
}

function snapshot(n: number): Uint8Array {
  const s = new Uint8Array(TEST_SEQLOCK.slotBytes).fill(n & 0xff);
  new DataView(s.buffer).setUint32(0, n, true);
  return s;
}

describe('seqlock snapshot buffer', () => {
  it('reports nothing before the first publication', () => {
    const { reader } = make();
    const out = new Uint8Array(TEST_SEQLOCK.slotBytes).fill(0xee);
    expect(reader.read(out)).toBe(0);
    expect(out.every((b) => b === 0xee)).toBe(true);
  });

  it('returns the newest publication and its number', () => {
    const { writer, reader } = make();
    const out = new Uint8Array(TEST_SEQLOCK.slotBytes);
    for (let n = 1; n <= 10; n++) {
      writer.publish(snapshot(n));
      expect(reader.read(out)).toBe(n);
      expect(new DataView(out.buffer).getUint32(0, true)).toBe(n);
      expect(out[100]).toBe(n & 0xff);
    }
    // Reading again without a new publication gives the same one.
    expect(reader.read(out)).toBe(10);
  });

  it('lets several readers see the same snapshot', () => {
    const { sab, writer, reader } = make();
    const other = new SeqlockReader(sab, TEST_SEQLOCK);
    writer.publish(snapshot(1));
    writer.publish(snapshot(2));
    const a = new Uint8Array(TEST_SEQLOCK.slotBytes);
    const b = new Uint8Array(TEST_SEQLOCK.slotBytes);
    expect([reader.read(a), other.read(b)]).toEqual([2, 2]);
    expect(a).toEqual(b);
  });

  it('lets the writer stamp the published copy but not the source', () => {
    const { writer, reader } = make();
    const src = snapshot(7);
    writer.publish(src, (slot) => {
      slot[8] = 0x55;
    });
    const out = new Uint8Array(TEST_SEQLOCK.slotBytes);
    reader.read(out);
    expect(out[8]).toBe(0x55);
    expect(src[8]).toBe(7);
  });

  it('walks the three slots and keeps working past a wrap of the slot index', () => {
    const { writer, reader } = make();
    const out = new Uint8Array(TEST_SEQLOCK.slotBytes);
    for (let n = 1; n <= 301; n++) {
      writer.publish(snapshot(n));
      if (n % 7 === 0) expect(reader.read(out)).toBe(n);
    }
  });

  it('reports contention instead of returning a half-written slot', () => {
    const { sab, writer, reader } = make();
    writer.publish(snapshot(1));
    // Hold slot 0 "mid-write" by leaving its sequence counter odd, as a stalled writer would.
    const header = new Int32Array(sab, 0, TEST_SEQLOCK.headerBytes / 4);
    header[TEST_SEQLOCK.hSlotSeq / 4] = (header[TEST_SEQLOCK.hSlotSeq / 4] ?? 0) + 1;
    const out = new Uint8Array(TEST_SEQLOCK.slotBytes).fill(0xee);
    expect(reader.read(out)).toBe(CONTENDED);
    expect(reader.contended).toBe(1);
    expect(reader.retries).toBe(reader.maxTries);
    expect(out.every((b) => b === 0xee)).toBe(true);
  });

  it('refuses buffers that are not this seqlock, and wrong sizes', () => {
    const raw = new SharedArrayBuffer(seqlockBytes(TEST_SEQLOCK));
    expect(() => new SeqlockWriter(raw, TEST_SEQLOCK)).toThrow(/SYS-021/);
    const { sab, writer, reader } = make();
    expect(() => new SeqlockReader(sab, { ...TEST_SEQLOCK, slotBytes: 8 })).toThrow(/SYS-021/);
    expect(() => {
      writer.publish(new Uint8Array(10));
    }).toThrow(RangeError);
    expect(() => reader.read(new Uint8Array(10))).toThrow(RangeError);
    expect(() => {
      initSeqlock(new SharedArrayBuffer(8), TEST_SEQLOCK);
    }).toThrow(RangeError);
  });
});

describe('seqlock across threads', () => {
  it('never hands out a torn snapshot while a writer hammers it', async () => {
    const COUNT = 1_000_000;
    const { sab, reader } = make();
    const worker = new Worker(new URL('./triple-buffer.stress.worker.ts', import.meta.url), {
      workerData: { sab, count: COUNT },
    });
    const finished = new Promise<void>((resolve, reject) => {
      worker.once('message', () => {
        resolve();
      });
      worker.once('error', reject);
    });

    const out = new Uint8Array(TEST_SEQLOCK.slotBytes);
    const dv = new DataView(out.buffer);
    let last = 0;
    let good = 0;
    const deadline = Date.now() + 60_000;
    while (last < COUNT && Date.now() < deadline) {
      const published = reader.read(out);
      if (published <= 0) continue;
      const n = dv.getUint32(0, true);
      const fill = n & 0xff;
      // Whole: every byte after the number is that number's fill byte.
      for (let i = 4; i < out.length; i++) {
        if (out[i] !== fill) {
          throw new Error(
            `torn snapshot: header says ${String(n)} but byte ${String(i)} is ${String(out[i])}`,
          );
        }
      }
      // At least as new as the number returned, and never older than what was seen before.
      expect(n).toBeGreaterThanOrEqual(published);
      expect(n).toBeGreaterThanOrEqual(last);
      last = n;
      good += 1;
    }
    await finished;
    await worker.terminate();
    expect(last).toBe(COUNT);
    expect(good).toBeGreaterThan(100);
  }, 90_000);
});
