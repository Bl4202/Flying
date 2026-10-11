import { execSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { build as esbuild } from 'esbuild';
import { defineConfig, type Plugin } from 'vite';
import { cspValue, ISOLATION_HEADERS, securityHeaders } from '@flying/platform-web/headers';

const repoRoot = resolve(import.meta.dirname, '../..');
const outDir = resolve(repoRoot, 'dist');
/** GitHub Pages serves the repository under /<repo>/. Override with BASE_PATH. */
const STAGING_BASE = '/Flying/';

function gitSha(): string {
  if (process.env['GIT_SHA']) {
    return process.env['GIT_SHA'];
  }
  try {
    return execSync('git rev-parse HEAD', { cwd: repoRoot, stdio: ['ignore', 'pipe', 'ignore'] })
      .toString()
      .trim();
  } catch {
    return 'unknown';
  }
}

function threadsInitialPages(command: 'build' | 'serve'): number {
  const manifest = resolve(repoRoot, 'wasm-out/wasm-manifest.json');
  if (!existsSync(manifest)) {
    if (command === 'build') {
      throw new Error('wasm-out/wasm-manifest.json is missing: run `cargo xtask wasm` first');
    }
    return 256;
  }
  return (JSON.parse(readFileSync(manifest, 'utf8')) as { threads_initial_pages: number })
    .threads_initial_pages;
}

/** Puts the CSP into index.html's <meta> so the very first (pre-Service-Worker) load is covered. */
function cspMeta(): Plugin {
  return {
    name: 'fg-csp-meta',
    apply: 'build',
    transformIndexHtml: (html) => html.replace('__CSP__', cspValue()),
  };
}

/** Emits build-manifest.json (the precache list) and bundles the Service Worker as sw.js. */
function serviceWorker(buildId: string): Plugin {
  return {
    name: 'fg-service-worker',
    apply: 'build',
    enforce: 'post',
    generateBundle(_options, bundle) {
      const files = new Set(['index.html', ...Object.keys(bundle)]);
      files.delete('build-manifest.json');
      files.delete('sw.js');
      this.emitFile({
        type: 'asset',
        fileName: 'build-manifest.json',
        source: `${JSON.stringify({ buildId, files: [...files].sort() }, null, 2)}\n`,
      });
    },
    async closeBundle() {
      // A classic (non-module) worker, so it runs in every browser that has Service Workers.
      await esbuild({
        entryPoints: [resolve(import.meta.dirname, 'src/sw.ts')],
        outfile: resolve(outDir, 'sw.js'),
        bundle: true,
        format: 'iife',
        target: 'es2022',
        minify: true,
        legalComments: 'none',
        define: { __BUILD_ID__: JSON.stringify(buildId) },
      });
    },
  };
}

export default defineConfig(({ command, isPreview }) => {
  const buildId = gitSha();
  const headersOn = process.env['HEADERS'] !== 'off';
  return {
    root: import.meta.dirname,
    base: process.env['BASE_PATH'] ?? (command === 'build' || isPreview ? STAGING_BASE : '/'),
    define: {
      __BUILD_ID__: JSON.stringify(buildId),
      __THREADS_INITIAL_PAGES__: JSON.stringify(threadsInitialPages(command)),
    },
    plugins: [cspMeta(), serviceWorker(buildId)],
    server: {
      headers: ISOLATION_HEADERS,
      fs: { allow: [repoRoot] },
    },
    preview: {
      // HEADERS=off emulates a static host that cannot set headers (GitHub Pages).
      headers: headersOn ? securityHeaders() : {},
    },
    worker: { format: 'es' },
    build: {
      outDir,
      emptyOutDir: true,
      target: 'es2022',
      sourcemap: false,
      assetsInlineLimit: 0,
      reportCompressedSize: false,
    },
  };
});
