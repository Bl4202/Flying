/**
 * Generates the WebGL2 shaders from the WGSL sources, so there is one source of truth for what is
 * drawn (GDD 61.3). The generated GLSL ES 3.00 is committed; the build never runs `naga`, and
 * lint fails if the committed files are stale.
 *
 * For each shader in `packages/render/shaders/manifest.json` this expands `#include`s, then runs
 *
 *   naga expanded.wgsl out.vert --entry-point <vertex>   --profile es300 --keep-coordinate-space
 *   naga expanded.wgsl out.frag --entry-point <fragment> --profile es300 --keep-coordinate-space
 *
 * `--keep-coordinate-space` leaves clip space as WebGPU defines it (depth 0..1, y up). The WebGL2
 * backend asks for the same with `EXT_clip_control`, or else uses a projection for -1..1.
 *
 * It also writes `gen/manifest.json`, which records the uniform block name naga gave each stage
 * so the backend does not have to guess it.
 *
 * Usage: node wgsl-gen.ts [--check] [shaders-dir]
 */
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { preprocess } from './wgsl-pp.ts';

export interface ShaderEntry {
  name: string;
  wgsl: string;
  vertex: string;
  fragment: string;
}

export interface GeneratedShader {
  vertex: string;
  fragment: string;
  /** Uniform block names naga chose, by stage. */
  blocks: { vertex: string; fragment: string };
}

/** The uniform blocks declared in GLSL text. */
export function uniformBlocks(glsl: string): string[] {
  return [...glsl.matchAll(/\buniform\s+(\w+)\s*\{/g)].flatMap((m) =>
    m[1] === undefined ? [] : [m[1]],
  );
}

function naga(args: string[]): void {
  const r = spawnSync('naga', args, { encoding: 'utf8' });
  if (r.error) {
    throw new Error(
      `cannot run naga (install with: cargo install naga-cli --locked): ${r.error.message}`,
    );
  }
  if (r.status !== 0) {
    throw new Error(`naga ${args.join(' ')} failed:\n${r.stderr}${r.stdout}`);
  }
}

/** Generates every shader's GLSL into `outDir`. */
export function generate(shaderDir: string, outDir: string): void {
  const manifest = JSON.parse(readFileSync(join(shaderDir, 'manifest.json'), 'utf8')) as {
    shaders: ShaderEntry[];
  };
  const scratch = mkdtempSync(join(tmpdir(), 'fg-wgsl-'));
  try {
    mkdirSync(outDir, { recursive: true });
    const result: Record<string, GeneratedShader> = {};
    for (const s of manifest.shaders) {
      const expanded = join(scratch, `${s.name}.wgsl`);
      writeFileSync(expanded, preprocess(join(shaderDir, s.wgsl)));
      const out = {
        vertex: `${s.name}.vert.glsl`,
        fragment: `${s.name}.frag.glsl`,
      };
      const flags = ['--profile', 'es300', '--keep-coordinate-space'];
      // naga picks the stage from the extension, so write `.vert`/`.frag` and rename.
      const v = join(scratch, `${s.name}.vert`);
      const f = join(scratch, `${s.name}.frag`);
      naga([expanded, v, '--entry-point', s.vertex, ...flags]);
      naga([expanded, f, '--entry-point', s.fragment, ...flags]);
      const vText = readFileSync(v, 'utf8');
      const fText = readFileSync(f, 'utf8');
      const [vBlock] = uniformBlocks(vText);
      const [fBlock] = uniformBlocks(fText);
      if (vBlock === undefined || fBlock === undefined) {
        throw new Error(`${s.name}: expected a uniform block in both stages`);
      }
      writeFileSync(join(outDir, out.vertex), vText);
      writeFileSync(join(outDir, out.fragment), fText);
      result[s.name] = { ...out, blocks: { vertex: vBlock, fragment: fBlock } };
    }
    writeFileSync(join(outDir, 'manifest.json'), `${JSON.stringify(result, null, 2)}\n`);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
}

/** Files whose content differs between two directories (by name, both ways). */
export function differences(a: string, b: string): string[] {
  const names = (d: string): string[] => {
    try {
      return readdirSync(d).sort();
    } catch {
      return [];
    }
  };
  const all = [...new Set([...names(a), ...names(b)])].sort();
  return all.filter((n) => {
    try {
      return readFileSync(join(a, n), 'utf8') !== readFileSync(join(b, n), 'utf8');
    } catch {
      return true;
    }
  });
}

if (import.meta.main) {
  const check = process.argv.includes('--check');
  const dirArg = process.argv.slice(2).find((a) => !a.startsWith('--'));
  const root = resolve(import.meta.dirname, '../../..');
  const shaderDir = resolve(dirArg ?? join(root, 'packages/render/shaders'));
  const genDir = join(shaderDir, 'gen');
  if (!check) {
    generate(shaderDir, genDir);
    console.log(`wrote ${genDir}`);
  } else {
    const scratch = mkdtempSync(join(tmpdir(), 'fg-wgsl-check-'));
    try {
      generate(shaderDir, scratch);
      const stale = differences(genDir, scratch);
      if (stale.length > 0) {
        console.error(`stale generated shaders in ${genDir}: ${stale.join(', ')}`);
        console.error('run `pnpm wgsl:gen` and commit the result');
        process.exit(1);
      }
      console.log('generated shaders are up to date');
    } finally {
      rmSync(scratch, { recursive: true, force: true });
    }
  }
}
