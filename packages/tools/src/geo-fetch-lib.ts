/**
 * Fetching geodata source files for the pipeline (ADR-54): HEAD for the size, skip a file already
 * present at that size, GET to a `.part` file with the size checked, then rename. Every file gets a
 * provenance record (URL, bytes, SHA-256, fetch time) in `<out>/provenance.json`. Fetch times are
 * provenance only; no archive records them, so rebuilding stays reproducible.
 */
import { createHash } from 'node:crypto';
import {
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { basename, join } from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import type { ReadableStream as WebReadableStream } from 'node:stream/web';

export interface FetchedFile {
  url: string;
  name: string;
  bytes: number;
  sha256: string;
  fetched_at: string;
}

export interface FetchOptions {
  /** Files fetched at once. */
  concurrency: number;
  /** The waits before each retry, ms: the first try, then one retry after each. */
  waitsMs: readonly number[];
  fetchImpl?: typeof fetch;
  now?: () => Date;
  log?: (line: string) => void;
}

/** The first try, then retries after 1, 2, 4, 8 and 16 s (ADR-54). */
export const DEFAULT_WAITS_MS: readonly number[] = [1_000, 2_000, 4_000, 8_000, 16_000];
/** A generic client name; no personal data in requests. */
const USER_AGENT = 'flying-geo-fetch/1';

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}

async function sha256File(path: string): Promise<string> {
  const h = createHash('sha256');
  await pipeline(createReadStream(path), h);
  return h.digest('hex');
}

/** The file name of a URL (its last path segment). */
export function fileNameOf(url: string): string {
  return basename(new URL(url).pathname);
}

async function attempt(url: string, path: string, f: typeof fetch): Promise<number> {
  const head = await f(url, { method: 'HEAD', headers: { 'user-agent': USER_AGENT } });
  if (!head.ok) throw new Error(`HEAD ${url}: ${String(head.status)}`);
  const len = Number(head.headers.get('content-length'));
  if (!Number.isFinite(len) || len <= 0) throw new Error(`HEAD ${url}: no Content-Length`);
  if (existsSync(path) && statSync(path).size === len) return len;
  const get = await f(url, { headers: { 'user-agent': USER_AGENT } });
  if (!get.ok || get.body === null) throw new Error(`GET ${url}: ${String(get.status)}`);
  const part = `${path}.part`;
  await pipeline(
    Readable.fromWeb(get.body as WebReadableStream<Uint8Array>),
    createWriteStream(part),
  );
  const got = statSync(part).size;
  if (got !== len) {
    rmSync(part, { force: true });
    throw new Error(`GET ${url}: ${String(got)} bytes, Content-Length ${String(len)}`);
  }
  renameSync(part, path);
  return len;
}

/** Fetches one URL into `outDir`, retrying; its provenance. */
export async function fetchOne(
  url: string,
  outDir: string,
  opts: FetchOptions,
): Promise<FetchedFile> {
  const f = opts.fetchImpl ?? fetch;
  const name = fileNameOf(url);
  const path = join(outDir, name);
  let last: unknown;
  for (let k = 0; k <= opts.waitsMs.length; k++) {
    if (k > 0) await sleep(opts.waitsMs[k - 1] ?? 0);
    try {
      const bytes = await attempt(url, path, f);
      return {
        url,
        name,
        bytes,
        sha256: await sha256File(path),
        fetched_at: (opts.now ?? (() => new Date()))().toISOString(),
      };
    } catch (e) {
      last = e;
      opts.log?.(
        `geo-fetch: ${String(e)} (try ${String(k + 1)} of ${String(opts.waitsMs.length + 1)})`,
      );
    }
  }
  throw last instanceof Error ? last : new Error(String(last));
}

/** Merges `files` into `<outDir>/provenance.json` (by URL, newest wins), sorted by URL. */
export function writeProvenance(outDir: string, files: readonly FetchedFile[]): void {
  const path = join(outDir, 'provenance.json');
  const byUrl = new Map<string, FetchedFile>();
  if (existsSync(path)) {
    const old = JSON.parse(readFileSync(path, 'utf8')) as { files?: FetchedFile[] };
    for (const f of old.files ?? []) byUrl.set(f.url, f);
  }
  for (const f of files) byUrl.set(f.url, f);
  const sorted = [...byUrl.values()].sort((a, b) => (a.url < b.url ? -1 : a.url > b.url ? 1 : 0));
  writeFileSync(
    path,
    `${JSON.stringify({ schema: 'geo_provenance/1', files: sorted }, null, 2)}\n`,
  );
}

/** Fetches every URL with `opts.concurrency` at once; the fetched files and the failures. */
export async function fetchAll(
  urls: readonly string[],
  outDir: string,
  opts: FetchOptions,
): Promise<{ files: FetchedFile[]; failed: { url: string; error: string }[] }> {
  mkdirSync(outDir, { recursive: true });
  const files: FetchedFile[] = [];
  const failed: { url: string; error: string }[] = [];
  let next = 0;
  const worker = async (): Promise<void> => {
    for (;;) {
      const i = next++;
      const url = urls[i];
      if (url === undefined) return;
      try {
        files.push(await fetchOne(url, outDir, opts));
      } catch (e) {
        failed.push({ url, error: String(e) });
      }
    }
  };
  await Promise.all(Array.from({ length: Math.max(1, opts.concurrency) }, worker));
  writeProvenance(outDir, files);
  return { files, failed };
}
