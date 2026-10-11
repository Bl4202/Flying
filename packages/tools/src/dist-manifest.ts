/**
 * Hashes every file in a dist directory (sorted, posix paths) for reproducible-build comparison,
 * and checks that everything except a short allow-list is content-addressed.
 *
 * Usage: node dist-manifest.ts <dist-dir> [--out file.json] [--check-hashed]
 */
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';

/** Files that legitimately keep a fixed name. */
export const FIXED_NAMES: readonly string[] = [
  'index.html',
  'sw.js',
  'build-manifest.json',
  '_headers',
];

/** Vite/Rolldown names hashed assets `name-<hash>.ext`. */
const HASHED = /-[A-Za-z0-9_-]{8,}\.[A-Za-z0-9]+$/;

export function isContentAddressed(path: string): boolean {
  return FIXED_NAMES.includes(path) || HASHED.test(path);
}

function* walk(dir: string): Generator<string> {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, entry.name);
    if (entry.isDirectory()) {
      yield* walk(p);
    } else {
      yield p;
    }
  }
}

export function manifest(dist: string): { path: string; sha256: string; bytes: number }[] {
  return [...walk(dist)]
    .map((p) => relative(dist, p).split('\\').join('/'))
    .sort()
    .map((path) => {
      const data = readFileSync(join(dist, path));
      return { path, sha256: createHash('sha256').update(data).digest('hex'), bytes: data.length };
    });
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const dist = resolve(args.find((a) => !a.startsWith('--')) ?? 'dist');
  const outIdx = args.indexOf('--out');
  const entries = manifest(dist);
  if (outIdx >= 0 && args[outIdx + 1] !== undefined) {
    writeFileSync(args[outIdx + 1] as string, `${JSON.stringify(entries, null, 2)}\n`);
  }
  if (args.includes('--check-hashed')) {
    const bad = entries.filter((e) => !isContentAddressed(e.path));
    if (bad.length > 0) {
      console.error(`not content-addressed:\n  ${bad.map((b) => b.path).join('\n  ')}`);
      process.exit(1);
    }
  }
  console.log(`${String(entries.length)} files in ${dist}`);
}
