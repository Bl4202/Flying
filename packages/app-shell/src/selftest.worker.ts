/**
 * Self-test worker: loads the sim WASM and the core content pack in a core of its own, steps a
 * fixed number of ticks with the scripted pilot and reports the state hash. The same setup as
 * `cargo xtask determinism`, so the result can be compared with the golden. It never touches the
 * running game's core, so the overlay can run it mid-flight.
 */
import type { Variant } from '@flying/platform-web/wasm-loader';
import { HASH_KIND, hex64, loadSim, type SessionConfig } from '@flying/sim-bridge/sim-handle';

export interface SelfTestRequest {
  variant: Variant;
  url: string;
  threadsInitialPages: number;
  /** URL of the baked `.cpack`. */
  contentUrl: string;
  /** Hex master seed. */
  seed: string;
  /** `INPUT_SCRIPT` value. */
  script: number;
  /** The session the golden was recorded under (`SCRIPTED_SESSION`). */
  session: SessionConfig;
  ticks: number;
}

export type SelfTestResult =
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
  onmessage: ((event: MessageEvent<SelfTestRequest>) => void) | null;
  postMessage(message: SelfTestResult): void;
}

const scope = self as unknown as WorkerScope;

scope.onmessage = (event) => {
  void run(event.data).then((result) => {
    scope.postMessage(result);
  });
};

async function run(req: SelfTestRequest): Promise<SelfTestResult> {
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
    // Same order as `fg hash`: content first (only an idle core takes it), then the session, then
    // the script.
    sim.init(BigInt(`0x${req.seed}`));
    sim.loadContent(pack);
    sim.configure(req.session);
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
      hash: hex64(sim.hash(HASH_KIND.COMBINED)),
      content: hex64(sim.hash(HASH_KIND.CONTENT)),
      ms,
    };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message : String(e) };
  }
}
