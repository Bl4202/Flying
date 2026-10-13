/** Storage throughput through the Origin Private File System, the way large downloads will use it. */

export interface OpfsFileLike {
  createWritable(): Promise<{ write(data: BufferSource): Promise<void>; close(): Promise<void> }>;
  getFile(): Promise<{ arrayBuffer(): Promise<ArrayBuffer> }>;
}
export interface OpfsDirLike {
  getFileHandle(name: string, options: { create: boolean }): Promise<OpfsFileLike>;
  removeEntry(name: string): Promise<void>;
}

export interface StorageResult {
  /** Whether the file system could be used at all. */
  available: boolean;
  /** Megabytes (10^6 bytes) per second, or null if it could not be measured. */
  writeMBps: number | null;
  readMBps: number | null;
  /** What happened, when it did not work. */
  note: string;
}

const FILE_NAME = 'fg-benchmark.tmp';
const CHUNK_BYTES = 1 << 20;

/**
 * Writes `megabytes` MB of incompressible data, reads it back, and deletes it. Never throws:
 * a browser without the API, or one that refuses (private windows, quota), gives `available:
 * false` and a note.
 */
export async function measureOpfs(
  getDirectory: (() => Promise<OpfsDirLike>) | undefined,
  megabytes: number,
  nowMs: () => number,
): Promise<StorageResult> {
  if (getDirectory === undefined) {
    return { available: false, writeMBps: null, readMBps: null, note: 'no OPFS in this browser' };
  }
  let dir: OpfsDirLike | undefined;
  try {
    dir = await getDirectory();
    const handle = await dir.getFileHandle(FILE_NAME, { create: true });
    // A cheap xorshift fill: random enough that nothing can compress it away.
    const chunk = new Uint32Array(CHUNK_BYTES / 4);
    let x = 0x9e3779b9;
    for (let i = 0; i < chunk.length; i++) {
      x ^= x << 13;
      x ^= x >>> 17;
      x ^= x << 5;
      chunk[i] = x >>> 0;
    }
    const total = Math.max(1, Math.round((megabytes * 1e6) / CHUNK_BYTES));
    const bytes = total * CHUNK_BYTES;

    const w0 = nowMs();
    const writable = await handle.createWritable();
    for (let i = 0; i < total; i++) await writable.write(chunk);
    await writable.close();
    const writeMs = nowMs() - w0;

    const r0 = nowMs();
    const file = await handle.getFile();
    const read = await file.arrayBuffer();
    const readMs = nowMs() - r0;
    if (read.byteLength !== bytes) {
      return {
        available: true,
        writeMBps: null,
        readMBps: null,
        note: `read back ${String(read.byteLength)} bytes, wrote ${String(bytes)}`,
      };
    }
    return {
      available: true,
      writeMBps: bytes / 1e6 / Math.max(writeMs / 1000, 1e-6),
      readMBps: bytes / 1e6 / Math.max(readMs / 1000, 1e-6),
      note: '',
    };
  } catch (e) {
    return {
      available: dir !== undefined,
      writeMBps: null,
      readMBps: null,
      note: e instanceof Error ? e.message : String(e),
    };
  } finally {
    try {
      await dir?.removeEntry(FILE_NAME);
    } catch {
      // nothing left to remove
    }
  }
}
