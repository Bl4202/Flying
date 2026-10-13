/** Producer thread for the ring stress test: pushes `count` numbered records, spinning when full. */
import { parentPort, workerData } from 'node:worker_threads';
import { fillRecord, TEST_RING } from './fixtures.ts';
import { SpscRingProducer } from './spsc-ring.ts';

const { sab, count } = workerData as { sab: SharedArrayBuffer; count: number };
const producer = new SpscRingProducer(sab, TEST_RING);
const record = new Uint8Array(TEST_RING.recordBytes);
let fullRetries = 0;
for (let seq = 0; seq < count; seq++) {
  fillRecord(record, seq);
  while (!producer.push(record)) {
    fullRetries += 1;
  }
}
parentPort?.postMessage({ fullRetries });
