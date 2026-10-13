import { Worker } from 'node:worker_threads';
import { describe, expect, it } from 'vitest';
import { fillRecord, recordIs, TEST_RING } from './fixtures.ts';
import {
  initRing,
  ringBytes,
  SpscRingConsumer,
  SpscRingProducer,
  type RingLayout,
} from './spsc-ring.ts';

function make(layout: RingLayout = TEST_RING) {
  const sab = new SharedArrayBuffer(ringBytes(layout));
  initRing(sab, layout);
  return {
    sab,
    producer: new SpscRingProducer(sab, layout),
    consumer: new SpscRingConsumer(sab, layout),
  };
}

function rec(seq: number): Uint8Array {
  const r = new Uint8Array(TEST_RING.recordBytes);
  fillRecord(r, seq);
  return r;
}

describe('spsc ring', () => {
  it('delivers records in order, byte for byte', () => {
    const { producer, consumer } = make();
    for (let i = 0; i < 10; i++) expect(producer.push(rec(i))).toBe(true);
    expect(consumer.pending()).toBe(10);
    const out = new Uint8Array(10 * 16);
    expect(consumer.drain(out, 10)).toBe(10);
    for (let i = 0; i < 10; i++) {
      expect(recordIs(out.subarray(i * 16, (i + 1) * 16), i)).toBe(true);
    }
    expect(consumer.drain(out, 10)).toBe(0);
    expect(consumer.pending()).toBe(0);
  });

  it('drains in parts and keeps order across the wrap', () => {
    const { producer, consumer } = make();
    const out = new Uint8Array(5 * 16);
    let next = 0;
    let expected = 0;
    for (let round = 0; round < 100; round++) {
      for (let i = 0; i < 7; i++) producer.push(rec(next++));
      const n = consumer.drain(out, 5);
      for (let i = 0; i < n; i++) {
        expect(recordIs(out.subarray(i * 16, (i + 1) * 16), expected++)).toBe(true);
      }
      const rest = consumer.drain(out, 5);
      for (let i = 0; i < rest; i++) {
        expect(recordIs(out.subarray(i * 16, (i + 1) * 16), expected++)).toBe(true);
      }
    }
    expect(expected).toBe(next);
  });

  it('drops the new record when full and counts it', () => {
    const { producer, consumer } = make();
    for (let i = 0; i < TEST_RING.capacity; i++) expect(producer.push(rec(i))).toBe(true);
    expect(producer.push(rec(999))).toBe(false);
    expect(producer.push(rec(1000))).toBe(false);
    expect(consumer.dropped()).toBe(2);
    const out = new Uint8Array(TEST_RING.capacity * 16);
    expect(consumer.drain(out, TEST_RING.capacity)).toBe(TEST_RING.capacity);
    expect(recordIs(out.subarray(0, 16), 0)).toBe(true);
    expect(recordIs(out.subarray(63 * 16, 64 * 16), 63)).toBe(true);
    expect(producer.push(rec(1001))).toBe(true);
  });

  it('survives the 32-bit counters wrapping', () => {
    const { sab, producer, consumer } = make();
    const header = new Int32Array(sab, 0, TEST_RING.headerBytes / 4);
    const nearWrap = (2 ** 32 - 3) | 0;
    header[TEST_RING.hWrite / 4] = nearWrap;
    header[TEST_RING.hRead / 4] = nearWrap;
    const out = new Uint8Array(16);
    for (let i = 0; i < 20; i++) {
      expect(producer.push(rec(i))).toBe(true);
      expect(consumer.drain(out, 1)).toBe(1);
      expect(recordIs(out, i)).toBe(true);
    }
    expect(header[TEST_RING.hWrite / 4]).toBe((nearWrap + 20) | 0);
  });

  it('refuses a buffer that is not this ring', () => {
    const raw = new SharedArrayBuffer(ringBytes(TEST_RING));
    expect(() => new SpscRingProducer(raw, TEST_RING)).toThrow(/SYS-020/);
    const { sab } = make();
    expect(() => new SpscRingConsumer(sab, { ...TEST_RING, recordBytes: 32 })).toThrow(/SYS-020/);
    expect(() => new SpscRingConsumer(sab, { ...TEST_RING, magic: 1 })).toThrow(/SYS-020/);
    expect(() => {
      initRing(new SharedArrayBuffer(8), TEST_RING);
    }).toThrow(RangeError);
    expect(() => {
      initRing(sab, { ...TEST_RING, capacity: 48 });
    }).toThrow(RangeError);
  });

  it('rejects records and buffers of the wrong size', () => {
    const { producer, consumer } = make();
    expect(() => producer.push(new Uint8Array(15))).toThrow(RangeError);
    expect(() => consumer.drain(new Uint8Array(15), 1)).toThrow(RangeError);
  });
});

describe('spsc ring across threads', () => {
  it('moves a million records with none lost, duplicated or corrupted', async () => {
    const COUNT = 1_000_000;
    const { sab, consumer } = make();
    const worker = new Worker(new URL('./spsc-ring.stress.worker.ts', import.meta.url), {
      workerData: { sab, count: COUNT },
    });
    const finished = new Promise<{ fullRetries: number }>((resolve, reject) => {
      worker.once('message', resolve);
      worker.once('error', reject);
    });

    const out = new Uint8Array(32 * 16);
    let received = 0;
    const deadline = Date.now() + 60_000;
    while (received < COUNT && Date.now() < deadline) {
      const n = consumer.drain(out, 32);
      for (let i = 0; i < n; i++) {
        if (!recordIs(out.subarray(i * 16, (i + 1) * 16), received)) {
          throw new Error(`record ${String(received)} is lost, duplicated or corrupt`);
        }
        received += 1;
      }
    }
    const { fullRetries } = await finished;
    await worker.terminate();
    expect(received).toBe(COUNT);
    expect(consumer.pending()).toBe(0);
    // Every refused push was retried, so the ring's drop count is exactly the retry count.
    expect(consumer.dropped()).toBe(fullRetries);
  }, 90_000);
});
