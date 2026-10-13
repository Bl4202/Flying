/**
 * Runs a built WASM module under Node and prints state hashes at requested ticks, in the same
 * JSON-lines format as `fg hash`, so the determinism check can compare native and WASM (ADR-10).
 *
 * Usage: node wasm-hash.ts --variant single|threads --seed HEX --at T1,T2,... [--dir wasm-out]
 *                           [--script none|erratic] [--content PATH.cpack]
 */
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { INPUT_SCRIPT } from '@flying/sim-bridge/layout';
import { HASH_KIND, hex64, instantiateSim } from '@flying/sim-bridge/sim-handle';

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

const variant = arg('variant');
if (variant !== 'single' && variant !== 'threads') {
  console.error('--variant single|threads is required');
  process.exit(2);
}
const seedText = arg('seed');
const atText = arg('at');
if (seedText === undefined || atText === undefined) {
  console.error('--seed and --at are required');
  process.exit(2);
}
const dir = resolve(arg('dir') ?? 'wasm-out');
const ticks = atText.split(',').map((t) => BigInt(t.trim()));

const manifest = JSON.parse(readFileSync(join(dir, 'wasm-manifest.json'), 'utf8')) as {
  threads_initial_pages: number;
};
const bytes = readFileSync(join(dir, `sim.${variant}.wasm`));
const sim = await instantiateSim(bytes, variant, manifest.threads_initial_pages);

const scriptText = arg('script') ?? 'none';
const script =
  scriptText === 'none'
    ? INPUT_SCRIPT.NONE
    : scriptText === 'erratic'
      ? INPUT_SCRIPT.ERRATIC
      : undefined;
if (script === undefined) {
  console.error('--script none|erratic');
  process.exit(2);
}
const contentPath = arg('content');

// Same order as `fg hash`: content first (only an idle core takes it), then the script, then start.
sim.init(BigInt(`0x${seedText.replace(/^0x/, '')}`));
if (contentPath !== undefined) {
  sim.loadContent(readFileSync(contentPath));
}
if (!sim.setInputScript(script) || !sim.start()) {
  console.error('the core would not start');
  process.exit(1);
}

const CHUNK = 100_000n;
for (const target of ticks) {
  while (sim.tick() < target) {
    const remaining = target - sim.tick();
    sim.step(Number(remaining < CHUNK ? remaining : CHUNK));
  }
  const line = {
    tick: Number(sim.tick()),
    combined: hex64(sim.hash(HASH_KIND.COMBINED)),
    tick_h: hex64(sim.hash(HASH_KIND.TICK)),
    rng: hex64(sim.hash(HASH_KIND.RNG)),
    bodies: hex64(sim.hash(HASH_KIND.BODIES)),
    observer: hex64(sim.hash(HASH_KIND.OBSERVER)),
    input: hex64(sim.hash(HASH_KIND.INPUT)),
    content: hex64(sim.hash(HASH_KIND.CONTENT)),
  };
  console.log(JSON.stringify(line));
}
