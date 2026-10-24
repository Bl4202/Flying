/**
 * Runs a built WASM module under Node and prints state hashes at requested ticks, in the same
 * JSON-lines format as `fg hash`, so the determinism check can compare native and WASM (ADR-10).
 *
 * Usage: node wasm-hash.ts --variant single|threads --seed HEX --at T1,T2,... [--dir wasm-out]
 *                           [--script none|erratic|flight60|circuit] [--content PATH.cpack]
 *                           [--spawn runway|air] [--assist full|standard|reduced|manual]
 *                           [--g off|reduced|full]
 */
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { ASSIST_LEVEL, G_EFFECTS, INPUT_SCRIPT, SPAWN_KIND } from '@flying/sim-bridge/layout';
import {
  DEFAULT_SESSION,
  HASH_KIND,
  hex64,
  instantiateSim,
  type SessionConfig,
} from '@flying/sim-bridge/sim-handle';

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
const scripts: Record<string, number> = {
  none: INPUT_SCRIPT.NONE,
  erratic: INPUT_SCRIPT.ERRATIC,
  flight60: INPUT_SCRIPT.FLIGHT60,
  circuit: INPUT_SCRIPT.CIRCUIT,
};
const script = scripts[scriptText];
if (script === undefined) {
  console.error('--script none|erratic|flight60|circuit');
  process.exit(2);
}
const contentPath = arg('content');

// The same words as `fg hash`; absent flags give the game's defaults, as there.
function choose(name: string, table: Record<string, number>, fallback: number): number {
  const text = arg(name);
  if (text === undefined) return fallback;
  const value = table[text];
  if (value === undefined) {
    console.error(`--${name} ${Object.keys(table).join('|')}`);
    process.exit(2);
  }
  return value;
}
const session: SessionConfig = {
  spawn: choose('spawn', { runway: SPAWN_KIND.RUNWAY, air: SPAWN_KIND.AIR }, DEFAULT_SESSION.spawn),
  assist: choose(
    'assist',
    {
      full: ASSIST_LEVEL.FULL,
      standard: ASSIST_LEVEL.STANDARD,
      reduced: ASSIST_LEVEL.REDUCED,
      manual: ASSIST_LEVEL.MANUAL,
    },
    DEFAULT_SESSION.assist,
  ),
  gEffects: choose(
    'g',
    { off: G_EFFECTS.OFF, reduced: G_EFFECTS.REDUCED, full: G_EFFECTS.FULL },
    DEFAULT_SESSION.gEffects,
  ),
};

// Same order as `fg hash`: content first (only an idle core takes it), then the session, then the
// script, then start.
sim.init(BigInt(`0x${seedText.replace(/^0x/, '')}`));
if (contentPath !== undefined) {
  sim.loadContent(readFileSync(contentPath));
}
sim.configure(session);
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
    ownship: hex64(sim.hash(HASH_KIND.OWNSHIP)),
    input: hex64(sim.hash(HASH_KIND.INPUT)),
    content: hex64(sim.hash(HASH_KIND.CONTENT)),
  };
  console.log(JSON.stringify(line));
}
