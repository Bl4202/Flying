/**
 * The first-run benchmark's CPU kernel under Node: how many ticks per second one thread of a build
 * steps the hands-off Kestrel, for re-basing the benchmark's CPU thresholds (ADR-28, amendment 1).
 * It does what `packages/perf/src/benchmark/kernel.worker.ts` does, from the raw exports of the
 * build it is pointed at, so it can measure a tag's build beside the current one:
 *
 *   node packages/tools/src/kernel-rate.ts --root . --seconds 10 --repeat 3
 *   node packages/tools/src/kernel-rate.ts --root ../wt-v0.1a --seconds 10 --repeat 3
 *
 * `--root` is a checkout whose `wasm-out/sim.single.wasm` and `content-out/core.cpack` are built.
 * A build with `fg_configure` flies the scripted session (an air start under Standard assist, the
 * rugged arena), as the kernel does since v0.1b; an older build has no such export and flies its
 * default session, as the kernel did then. It prints one JSON line.
 */
import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

export interface Options {
  root: string;
  /** Seconds of stepping in each repeat. */
  seconds: number;
  repeat: number;
  /** Seconds of stepping before the first repeat, so the engine has tiered up. */
  warmupSeconds: number;
}

export function parseArgs(argv: readonly string[]): Options {
  const o: Options = { root: '.', seconds: 10, repeat: 3, warmupSeconds: 2 };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const value = (): string => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${flag ?? ''} needs a value`);
      return v;
    };
    const positive = (): number => {
      const n = Number(value());
      if (!Number.isFinite(n) || n <= 0) throw new Error(`${flag ?? ''} must be a positive number`);
      return n;
    };
    if (flag === '--root') o.root = value();
    else if (flag === '--seconds') o.seconds = positive();
    else if (flag === '--repeat') o.repeat = Math.round(positive());
    else if (flag === '--warmup') o.warmupSeconds = positive();
    else throw new Error(`unknown argument ${flag ?? ''}`);
  }
  return o;
}

/** The median of a non-empty list. */
export function median(xs: readonly number[]): number {
  const sorted = [...xs].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  const hi = sorted[mid] ?? Number.NaN;
  const lo = sorted[mid - 1] ?? hi;
  return sorted.length % 2 === 1 ? hi : (lo + hi) / 2;
}

type Fn = (...args: number[]) => number;

function fnOf(x: WebAssembly.Exports, name: string): Fn {
  const f = x[name];
  if (typeof f !== 'function') throw new Error(`the build has no ${name}`);
  return f as Fn;
}

// The scripted session's codes (`SCRIPTED_SESSION`: air start, Standard assist, full g, rugged).
const SPAWN_AIR = 1;
const ASSIST_STANDARD = 2;
const G_FULL = 2;
const ARENA_RUGGED = 0;
const SEED = 0x1f1e5eed00000001n;
/** Ticks per call into the module, as the kernel does. */
const CHUNK_TICKS = 200;

async function boot(root: string): Promise<{ step: Fn; abi: number; configured: boolean }> {
  const wasm = readFileSync(join(root, 'wasm-out', 'sim.single.wasm'));
  const content = readFileSync(join(root, 'content-out', 'core.cpack'));
  const { instance } = await WebAssembly.instantiate(wasm, {
    env: { fg_now: () => performance.now() },
  });
  const x = instance.exports;
  const memory = x['memory'];
  if (!(memory instanceof WebAssembly.Memory)) throw new Error('the build exports no memory');
  const abi = fnOf(x, 'fg_abi_version')();
  if (fnOf(x, 'fg_init')(Number(SEED & 0xffff_ffffn), Number(SEED >> 32n)) !== 0) {
    throw new Error('fg_init failed');
  }
  const ptr = fnOf(x, 'fg_content_reserve')(content.length);
  if (ptr === 0) throw new Error('cannot reserve the content');
  new Uint8Array(memory.buffer, ptr, content.length).set(content);
  fnOf(x, 'fg_load_content')(content.length);
  const configured = typeof x['fg_configure'] === 'function';
  if (configured) fnOf(x, 'fg_configure')(SPAWN_AIR, ASSIST_STANDARD, G_FULL, ARENA_RUGGED);
  if (fnOf(x, 'fg_start')() !== 1) throw new Error('the core would not start');
  return { step: fnOf(x, 'fg_step'), abi, configured };
}

function rate(step: Fn, seconds: number): number {
  const t0 = performance.now();
  let ticks = 0;
  let now = t0;
  while (now - t0 < seconds * 1000) {
    ticks += step(CHUNK_TICKS);
    now = performance.now();
  }
  return (ticks / (now - t0)) * 1000;
}

async function main(): Promise<void> {
  const o = parseArgs(process.argv.slice(2));
  const root = resolve(o.root);
  const { step, abi, configured } = await boot(root);
  rate(step, o.warmupSeconds);
  const runs = Array.from({ length: o.repeat }, () => rate(step, o.seconds));
  console.log(
    JSON.stringify({
      root,
      abi,
      session: configured ? 'scripted (fg_configure)' : 'default (no fg_configure)',
      seconds: o.seconds,
      ticksPerSecond: runs.map(Math.round),
      median: Math.round(median(runs)),
    }),
  );
}

if (import.meta.main) {
  main().catch((e: unknown) => {
    console.error(e instanceof Error ? e.message : String(e));
    process.exit(1);
  });
}
