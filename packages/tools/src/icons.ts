/**
 * Renders the app icons (`pnpm icons`): the PNG sizes the web app manifest names, from the original
 * SVGs in `packages/app-shell/icons`. Run it when an SVG changes and commit the PNGs; the build only
 * reads them, so it does not need a browser.
 *
 *     icon-192.png            192 x 192, the icon
 *     icon-512.png            512 x 512, the icon
 *     icon-maskable-512.png   512 x 512, full bleed with the picture in the middle 80%
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '@playwright/test';

const dir = resolve(import.meta.dirname, '../../app-shell/icons');

const JOBS = [
  { svg: 'icon.svg', png: 'icon-192.png', size: 192 },
  { svg: 'icon.svg', png: 'icon-512.png', size: 512 },
  { svg: 'icon-maskable.svg', png: 'icon-maskable-512.png', size: 512 },
] as const;

const browser = await chromium.launch();
try {
  for (const job of JOBS) {
    const svg = readFileSync(resolve(dir, job.svg), 'utf8');
    const page = await browser.newPage({ viewport: { width: job.size, height: job.size } });
    await page.setContent(
      `<!doctype html><style>html,body{margin:0;background:transparent}svg{display:block;width:${String(job.size)}px;height:${String(job.size)}px}</style>${svg}`,
    );
    const png = await page.screenshot({ omitBackground: true, type: 'png' });
    writeFileSync(resolve(dir, job.png), png);
    console.info(`icons: ${job.png} (${String(png.length)} bytes)`);
    await page.close();
  }
} finally {
  await browser.close();
}
