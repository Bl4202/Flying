/** Sleeps in an AtomicsSleeper for a long time and reports how long it really slept. */
import { parentPort, workerData } from 'node:worker_threads';
import { AtomicsSleeper } from './sleeper.ts';

const { wake, ms, sleepsBeforeReport } = workerData as {
  wake: SharedArrayBuffer;
  ms: number;
  sleepsBeforeReport: number;
};
const sleeper = new AtomicsSleeper(wake);
parentPort?.postMessage({ ready: true });
const t0 = performance.now();
for (let i = 0; i < sleepsBeforeReport; i++) {
  sleeper.sleep(ms);
}
parentPort?.postMessage({ slept: performance.now() - t0 });
