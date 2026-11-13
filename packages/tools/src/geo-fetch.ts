/**
 * Fetches the pipeline's source files (ADR-54).
 *
 * Usage: node packages/tools/src/geo-fetch.ts --list <urls.txt> --out <dir> [--concurrency 8]
 *
 * One URL per line in the list. Files already present with the right size are skipped. Each file
 * gets its first try and up to five retries, after 1, 2, 4, 8 and 16 s. `<dir>/provenance.json` records
 * URL, bytes, SHA-256 and fetch time. Exits 1 if any file still fails.
 */
import { readFileSync } from 'node:fs';
import { DEFAULT_WAITS_MS, fetchAll } from './geo-fetch-lib.ts';

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

const list = arg('list');
const out = arg('out');
if (list === undefined || out === undefined) {
  console.error('usage: geo-fetch.ts --list <urls.txt> --out <dir> [--concurrency 8]');
  process.exit(2);
}
const urls = readFileSync(list, 'utf8')
  .split('\n')
  .map((l) => l.trim())
  .filter((l) => l.length > 0);
const concurrency = Number(arg('concurrency') ?? '8');
const { files, failed } = await fetchAll(urls, out, {
  concurrency,
  waitsMs: DEFAULT_WAITS_MS,
  log: (l) => {
    console.error(l);
  },
});
const bytes = files.reduce((s, f) => s + f.bytes, 0);
console.log(
  `geo-fetch: ${String(files.length)} files, ${(bytes / 1e6).toFixed(1)} MB, ${String(failed.length)} failed`,
);
for (const f of failed) console.error(`geo-fetch: FAILED ${f.url}: ${f.error}`);
process.exit(failed.length === 0 ? 0 : 1);
