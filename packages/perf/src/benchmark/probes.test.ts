import { describe, expect, it } from 'vitest';
import { readMemoryHints } from './memory.ts';
import { measureOpfs, type OpfsDirLike, type OpfsFileLike } from './storage.ts';

function fakeOpfs(over: { writeFails?: boolean; shortRead?: boolean } = {}) {
  const calls: string[] = [];
  let stored = 0;
  const file: OpfsFileLike = {
    createWritable: () => {
      if (over.writeFails) return Promise.reject(new Error('quota exceeded'));
      return Promise.resolve({
        write: (data) => {
          stored += data.byteLength;
          return Promise.resolve();
        },
        close: () => Promise.resolve(),
      });
    },
    getFile: () =>
      Promise.resolve({
        arrayBuffer: () => Promise.resolve(new ArrayBuffer(over.shortRead ? stored - 1 : stored)),
      }),
  };
  const dir: OpfsDirLike = {
    getFileHandle: (name) => {
      calls.push(`open ${name}`);
      return Promise.resolve(file);
    },
    removeEntry: (name) => {
      calls.push(`remove ${name}`);
      return Promise.resolve();
    },
  };
  return { dir, calls, stored: () => stored };
}

/** A clock that advances 100 ms each time it is read: writes and reads then take 100 ms apiece. */
function steppedClock() {
  let t = 0;
  return () => (t += 50);
}

describe('measureOpfs', () => {
  it('writes the requested amount, reads it back, removes the file and gives MB/s', async () => {
    const f = fakeOpfs();
    const r = await measureOpfs(() => Promise.resolve(f.dir), 8, steppedClock());
    expect(f.stored()).toBe(8 * (1 << 20));
    expect(r.available).toBe(true);
    expect(r.note).toBe('');
    // 8 MiB in 50 ms each way.
    const expected = (8 * (1 << 20)) / 1e6 / 0.05;
    expect(r.writeMBps).toBeCloseTo(expected, 3);
    expect(r.readMBps).toBeCloseTo(expected, 3);
    expect(f.calls).toEqual(['open fg-benchmark.tmp', 'remove fg-benchmark.tmp']);
  });

  it('says so when the browser has no OPFS', async () => {
    expect(await measureOpfs(undefined, 8, steppedClock())).toMatchObject({
      available: false,
      writeMBps: null,
    });
  });

  it('never throws: a refused write becomes a note, and the file is still removed', async () => {
    const f = fakeOpfs({ writeFails: true });
    const r = await measureOpfs(() => Promise.resolve(f.dir), 8, steppedClock());
    expect(r).toMatchObject({ available: true, writeMBps: null, readMBps: null });
    expect(r.note).toContain('quota exceeded');
    expect(f.calls.at(-1)).toBe('remove fg-benchmark.tmp');
  });

  it('does not report a rate for data that did not come back whole', async () => {
    const f = fakeOpfs({ shortRead: true });
    const r = await measureOpfs(() => Promise.resolve(f.dir), 8, steppedClock());
    expect(r.writeMBps).toBeNull();
    expect(r.note).toContain('read back');
  });

  it('survives a directory that cannot be opened', async () => {
    const r = await measureOpfs(
      () => Promise.reject(new Error('SecurityError')),
      8,
      steppedClock(),
    );
    expect(r).toMatchObject({ available: false, writeMBps: null });
    expect(r.note).toBe('SecurityError');
  });
});

describe('readMemoryHints', () => {
  it('reads what Chrome offers, in MB', async () => {
    const h = await readMemoryHints({
      navigator: { deviceMemory: 8 },
      performance: {
        memory: { jsHeapSizeLimit: 4096 * 1048576, usedJSHeapSize: 10 * 1048576 },
        measureUserAgentSpecificMemory: () => Promise.resolve({ bytes: 200 * 1048576 }),
      },
    });
    expect(h).toEqual({
      deviceMemoryGb: 8,
      jsHeapLimitMb: 4096,
      jsHeapUsedMb: 10,
      measuredMb: 200,
    });
  });

  it('gives nulls where the browser says nothing, and survives a refusal', async () => {
    expect(await readMemoryHints({ navigator: {}, performance: {} })).toEqual({
      deviceMemoryGb: null,
      jsHeapLimitMb: null,
      jsHeapUsedMb: null,
      measuredMb: null,
    });
    const refused = await readMemoryHints({
      navigator: {},
      performance: { measureUserAgentSpecificMemory: () => Promise.reject(new Error('no')) },
    });
    expect(refused.measuredMb).toBeNull();
  });
});
