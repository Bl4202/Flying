/**
 * The determinism self-test: the same run as `cargo xtask determinism` at its 10 000-tick
 * checkpoint, in a worker of its own, compared with the committed golden. `?selftest=1` shows it
 * as a status table (the end-to-end tests read that table); the debug overlay's Self-test button
 * runs it while the game flies.
 */
import { scriptUrl, type Variant } from '@flying/platform-web';
import { INPUT_SCRIPT } from '@flying/sim-bridge/layout';
import { SEED, SELFTEST_TICKS } from './selftest-judge.ts';
import coreContentUrl from '../../../content-out/core.cpack?url';
import singleUrl from '../../../wasm-out/sim.single.wasm?url';
import threadsUrl from '../../../wasm-out/sim.threads.wasm?url';
import type { SelfTestRequest, SelfTestResult } from './selftest.worker.ts';
import selfTestWorkerUrl from './selftest.worker.ts?worker&url';

export function wasmUrl(variant: Variant): string {
  return variant === 'threads' ? threadsUrl : singleUrl;
}

export { coreContentUrl };

export function selfTestRequest(variant: Variant): SelfTestRequest {
  return {
    variant,
    url: wasmUrl(variant),
    threadsInitialPages: __THREADS_INITIAL_PAGES__,
    contentUrl: coreContentUrl,
    seed: SEED,
    script: INPUT_SCRIPT.ERRATIC,
    ticks: SELFTEST_TICKS,
  };
}

/** Runs the request in a throwaway worker. Rejects if the worker fails to start. */
export function runSelfTestWorker(request: SelfTestRequest): Promise<SelfTestResult> {
  return new Promise<SelfTestResult>((resolve, reject) => {
    const worker = new Worker(scriptUrl(selfTestWorkerUrl), { type: 'module' });
    worker.onmessage = (event: MessageEvent<SelfTestResult>) => {
      worker.terminate();
      resolve(event.data);
    };
    worker.onerror = (event) => {
      worker.terminate();
      reject(new Error(`SYS-033: the self-test worker failed (${event.message})`));
    };
    worker.postMessage(request);
  });
}
