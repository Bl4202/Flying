import { createHash } from 'node:crypto';
import { mkdtempSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';
import { fetchAll, fetchOne, fileNameOf } from './geo-fetch-lib.ts';

const BODY = Buffer.from('a'.repeat(5_000) + 'b'.repeat(3_000));
let server: Server;
let base = '';
let failsLeft = 0;
let gets = 0;

beforeAll(async () => {
  server = createServer((req, res) => {
    if (req.url === '/flaky.tif' && failsLeft > 0) {
      failsLeft--;
      res.writeHead(503).end();
      return;
    }
    if (req.url === '/missing.tif') {
      res.writeHead(404).end();
      return;
    }
    if (req.url === '/short.tif') {
      // HEAD promises the whole file; GET sends 100 bytes (a server that changed under us).
      if (req.method === 'HEAD')
        res.writeHead(200, { 'content-length': String(BODY.length) }).end();
      else res.writeHead(200, { 'content-length': '100' }).end(BODY.subarray(0, 100));
      return;
    }
    res.writeHead(200, { 'content-length': String(BODY.length) });
    if (req.method === 'GET') gets++;
    res.end(req.method === 'HEAD' ? undefined : BODY);
  });
  await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
  base = `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`;
});

afterAll(() => {
  server.close();
});

const opts = {
  concurrency: 2,
  waitsMs: [1, 1, 1, 1, 1],
  now: () => new Date('2026-11-13T00:00:00Z'),
};

describe('geo-fetch', () => {
  it('fetches, hashes and records provenance; skips a file already there', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'geo-fetch-'));
    const r = await fetchAll([`${base}/Copernicus_A.tif`], dir, opts);
    expect(r.failed).toEqual([]);
    expect(statSync(join(dir, 'Copernicus_A.tif')).size).toBe(BODY.length);
    const sha = createHash('sha256').update(BODY).digest('hex');
    expect(r.files[0]).toEqual({
      url: `${base}/Copernicus_A.tif`,
      name: 'Copernicus_A.tif',
      bytes: BODY.length,
      sha256: sha,
      fetched_at: '2026-11-13T00:00:00.000Z',
    });
    const before = gets;
    await fetchAll([`${base}/Copernicus_A.tif`, `${base}/Copernicus_B.tif`], dir, opts);
    expect(gets - before).toBe(1); // A was skipped
    const prov = JSON.parse(readFileSync(join(dir, 'provenance.json'), 'utf8')) as {
      files: { url: string }[];
    };
    expect(prov.files.map((f) => f.url)).toEqual([
      `${base}/Copernicus_A.tif`,
      `${base}/Copernicus_B.tif`,
    ]);
  });

  it('retries a flaky server and refetches a file of the wrong size', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'geo-fetch-'));
    failsLeft = 3;
    const f = await fetchOne(`${base}/flaky.tif`, dir, opts);
    expect(f.bytes).toBe(BODY.length);
    writeFileSync(join(dir, 'other.tif'), 'stale');
    const g = await fetchOne(`${base}/other.tif`, dir, opts);
    expect(g.bytes).toBe(BODY.length);
  });

  it('reports a missing file and a truncated body', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'geo-fetch-'));
    const r = await fetchAll([`${base}/missing.tif`, `${base}/short.tif`], dir, opts);
    expect(r.files).toEqual([]);
    expect(r.failed.map((x) => x.url).sort()).toEqual([`${base}/missing.tif`, `${base}/short.tif`]);
    expect(r.failed.find((x) => x.url.endsWith('short.tif'))?.error).toContain('100 bytes');
    expect(fileNameOf('https://h.example/a/b/c.tif?x=1')).toBe('c.tif');
  });
});
