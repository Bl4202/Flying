import { execSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { build as esbuild } from 'esbuild';
import { defineConfig, type Plugin } from 'vite';
import { cspValue, ISOLATION_HEADERS, securityHeaders } from '@flying/platform-web/headers';
import { wgsl } from '@flying/tools/vite-plugin-wgsl';

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

function requireContent(command: 'build' | 'serve'): void {
  if (command !== 'build') return;
  for (const file of ['core.cpack', 'core.present.json']) {
    if (!existsSync(resolve(repoRoot, 'content-out', file))) {
      throw new Error(`content-out/${file} is missing: run \`cargo xtask content\` first`);
    }
  }
}

/** Puts the CSP into index.html's <meta> so the very first (pre-Service-Worker) load is covered. */
function cspMeta(): Plugin {
  return {
    name: 'fg-csp-meta',
    apply: 'build',
    transformIndexHtml: (html) => html.replace('__CSP__', cspValue()),
  };
}

/** The icons the manifest names, rendered once by `pnpm icons` from the SVGs beside them. */
const ICONS = [
  { file: 'icon-192.png', sizes: '192x192', purpose: 'any' },
  { file: 'icon-512.png', sizes: '512x512', purpose: 'any' },
  { file: 'icon-maskable-512.png', sizes: '512x512', purpose: 'maskable' },
] as const;
const THEME_COLOR = '#0b2a5b';
const BACKGROUND_COLOR = '#101418';

/**
 * The installable app (ADR-44): emits `manifest.webmanifest` under that fixed name (a browser looks it
 * up by the URL in the page, and the Service Worker precaches it with everything else) and the icons
 * it names under hashed names, and adds the manifest link, the icon and the theme colour to the page.
 * The head is written without whitespace between the tags: the DOM budget counts every text node.
 */
function webManifest(): Plugin {
  let base = '/';
  const iconDir = resolve(import.meta.dirname, 'icons');
  const refs = new Map<string, string>();
  return {
    name: 'fg-web-manifest',
    apply: 'build',
    configResolved(config) {
      base = config.base;
    },
    buildStart() {
      for (const icon of [...ICONS.map((i) => i.file), 'icon.svg']) {
        const path = resolve(iconDir, icon);
        if (!existsSync(path)) {
          throw new Error(`packages/app-shell/icons/${icon} is missing: run \`pnpm icons\``);
        }
        refs.set(icon, this.emitFile({ type: 'asset', name: icon, source: readFileSync(path) }));
      }
    },
    generateBundle() {
      const file = (name: string): string => {
        const ref = refs.get(name);
        if (ref === undefined) throw new Error(`no emitted icon ${name}`);
        return this.getFileName(ref);
      };
      const manifest = {
        id: base,
        name: 'Flying',
        short_name: 'Flying',
        description: 'A flight simulator that runs in your browser.',
        start_url: base,
        scope: base,
        display: 'standalone',
        orientation: 'landscape',
        background_color: BACKGROUND_COLOR,
        theme_color: THEME_COLOR,
        icons: ICONS.map((i) => ({
          src: file(i.file),
          sizes: i.sizes,
          type: 'image/png',
          purpose: i.purpose,
        })),
      };
      this.emitFile({
        type: 'asset',
        fileName: 'manifest.webmanifest',
        source: `${JSON.stringify(manifest, null, 2)}\n`,
      });
      // Remembered for the page, which links the icon by its hashed name.
      refs.set('favicon', file('icon.svg'));
      refs.set('apple', file('icon-192.png'));
    },
    transformIndexHtml: {
      order: 'post',
      handler(html) {
        const favicon = refs.get('favicon') ?? '';
        const apple = refs.get('apple') ?? '';
        const tags =
          `<link rel="manifest" href="${base}manifest.webmanifest" />` +
          `<meta name="theme-color" content="${THEME_COLOR}" />` +
          `<link rel="icon" type="image/svg+xml" href="${base}${favicon}" />` +
          `<link rel="apple-touch-icon" href="${base}${apple}" />`;
        const withTags = html.replace('<link rel="icon" href="data:," />', tags);
        // No whitespace between the tags of the head.
        return withTags.replace(/<head>([\s\S]*?)<\/head>/, (_m, inner: string) => {
          return `<head>${inner.replace(/>\s+</g, '><').trim()}</head>`;
        });
      },
    },
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
  requireContent(command);
  const buildId = gitSha();
  const headersOn = process.env['HEADERS'] !== 'off';
  return {
    root: import.meta.dirname,
    base: process.env['BASE_PATH'] ?? (command === 'build' || isPreview ? STAGING_BASE : '/'),
    define: {
      __BUILD_ID__: JSON.stringify(buildId),
      __THREADS_INITIAL_PAGES__: JSON.stringify(threadsInitialPages(command)),
    },
    plugins: [wgsl(), cspMeta(), webManifest(), serviceWorker(buildId)],
    server: {
      headers: ISOLATION_HEADERS,
      fs: { allow: [repoRoot] },
    },
    preview: {
      // HEADERS=off emulates a static host that cannot set headers (GitHub Pages).
      headers: headersOn ? securityHeaders() : {},
    },
    // The Render Worker imports the WGSL shaders too, so its bundle needs the plugin as well.
    worker: { format: 'es', plugins: () => [wgsl()] },
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
