/**
 * Probe worker: loads the sim WASM, steps it a fixed number of ticks and reports the state hash.
 * This is the "sim runs in a Worker" proof for v0.0a; the real fixed-rate loop is v0.0b.
 */
import { hex64, loadSim, type Variant } from '@flying/platform-web/wasm-loader';

export interface ProbeRequest {
  variant: Variant;
  url: string;
  threadsInitialPages: number;
  /** Hex master seed. */
  seed: string;
  ticks: number;
}

export type ProbeResult =
  | { ok: true; variant: Variant; abi: number; tick: string; hash: string; ms: number }
  | { ok: false; error: string };

interface WorkerScope {
  onmessage: ((event: MessageEvent<ProbeRequest>) => void) | null;
  postMessage(message: ProbeResult): void;
}

const scope = self as unknown as WorkerScope;

scope.onmessage = (event) => {
  void run(event.data).then((result) => {
    scope.postMessage(result);
  });
};

async function run(req: ProbeRequest): Promise<ProbeResult> {
  try {
    const sim = await loadSim({
      variant: req.variant,
      url: req.url,
      threadsInitialPages: req.threadsInitialPages,
    });
    sim.init(BigInt(`0x${req.seed}`));
    const start = performance.now();
    sim.step(req.ticks);
    const ms = performance.now() - start;
    return {
      ok: true,
      variant: sim.variant,
      abi: sim.abiVersion(),
      tick: sim.tick().toString(),
      hash: hex64(sim.hash('combined')),
      ms,
    };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}
