/**
 * Probe worker: loads the sim WASM and the core content pack, steps a fixed number of ticks with
 * the scripted pilot and reports the state hash. The same setup as `cargo xtask determinism`, so
 * the result can be compared with the golden. (Replaced by the self-test worker in WP10.)
 */
import { hex64, loadSim, type InputScript, type Variant } from '@flying/platform-web/wasm-loader';

export interface ProbeRequest {
  variant: Variant;
  url: string;
  threadsInitialPages: number;
  /** URL of the baked `.cpack`. */
  contentUrl: string;
  /** Hex master seed. */
  seed: string;
  script: InputScript;
  ticks: number;
}

export type ProbeResult =
  | {
      ok: true;
      variant: Variant;
      abi: number;
      tick: string;
      hash: string;
      content: string;
      ms: number;
    }
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
    const response = await fetch(req.contentUrl);
    if (!response.ok) {
      throw new Error(`SYS-005: ${req.contentUrl} returned ${String(response.status)}`);
    }
    const pack = new Uint8Array(await response.arrayBuffer());
    // Same order as `fg hash`: content first (only an idle core takes it), then the script.
    sim.init(BigInt(`0x${req.seed}`));
    sim.loadContent(pack);
    if (!sim.setInputScript(req.script) || !sim.start()) {
      throw new Error('SYS-004: the core would not start');
    }
    const start = performance.now();
    sim.step(req.ticks);
    const ms = performance.now() - start;
    return {
      ok: true,
      variant: sim.variant,
      abi: sim.abiVersion(),
      tick: sim.tick().toString(),
      hash: hex64(sim.hash('combined')),
      content: hex64(sim.hash('content')),
      ms,
    };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}
