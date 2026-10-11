/**
 * Expands every shader under packages/render/shaders and validates it with `naga`.
 * Usage: node wgsl-check.ts [shaders-dir] [out-dir]
 */
import { spawnSync } from 'node:child_process';
import { mkdirSync, readdirSync, writeFileSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';
import { preprocess } from './wgsl-pp.ts';

const root = resolve(import.meta.dirname, '../../..');
const shaderDir = resolve(process.argv[2] ?? join(root, 'packages/render/shaders'));
const outDir = resolve(process.argv[3] ?? join(root, 'target/wgsl-expanded'));

function* walk(dir: string): Generator<string> {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(p);
    } else if (entry.name.endsWith('.wgsl')) {
      yield p;
    }
  }
}

mkdirSync(outDir, { recursive: true });
let failures = 0;
let count = 0;
for (const file of [...walk(shaderDir)].sort()) {
  count += 1;
  const rel = relative(shaderDir, file);
  const expanded = join(outDir, rel.split('/').join('__'));
  try {
    writeFileSync(expanded, preprocess(file));
  } catch (e) {
    console.error(`FAIL ${rel}: ${e instanceof Error ? e.message : String(e)}`);
    failures += 1;
    continue;
  }
  const result = spawnSync('naga', [expanded], { encoding: 'utf8' });
  if (result.error) {
    console.error(
      `cannot run naga (install with: cargo install naga-cli --locked): ${result.error.message}`,
    );
    process.exit(2);
  }
  if (result.status === 0) {
    console.log(`ok   ${rel}`);
  } else {
    console.error(`FAIL ${rel} (expanded copy: ${expanded})\n${result.stderr}${result.stdout}`);
    failures += 1;
  }
}
if (count === 0) {
  console.error(`no .wgsl files under ${shaderDir}`);
  process.exit(1);
}
process.exit(failures === 0 ? 0 : 1);
