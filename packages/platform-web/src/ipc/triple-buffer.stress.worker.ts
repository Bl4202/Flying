/**
 * Writer thread for the seqlock stress test. Publication `n` fills every byte of the slot with
 * `n & 0xff` after writing `n` into the first four bytes, so any torn copy is visible.
 */
import { parentPort, workerData } from 'node:worker_threads';
import { TEST_SEQLOCK } from './fixtures.ts';
import { SeqlockWriter } from './triple-buffer.ts';

const { sab, count } = workerData as { sab: SharedArrayBuffer; count: number };
const writer = new SeqlockWriter(sab, TEST_SEQLOCK);
const src = new Uint8Array(TEST_SEQLOCK.slotBytes);
const dv = new DataView(src.buffer);
for (let n = 1; n <= count; n++) {
  src.fill(n & 0xff);
  dv.setUint32(0, n, true);
  writer.publish(src);
}
parentPort?.postMessage({ done: true });
