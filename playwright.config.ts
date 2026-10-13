import { defineConfig, devices } from '@playwright/test';

/** Preview of the production build with all security headers (a host that can set headers). */
export const HEADERS_PORT = 4173;
/** Preview with no headers at all: emulates GitHub Pages. */
export const PAGES_PORT = 4174;

const preview = (port: number, headers: 'on' | 'off') => ({
  command: `pnpm exec vite preview --config packages/app-shell/vite.config.ts --host 127.0.0.1 --port ${String(port)} --strictPort`,
  url: `http://127.0.0.1:${String(port)}/Flying/`,
  env: { HEADERS: headers },
  reuseExistingServer: !process.env['CI'],
  timeout: 60_000,
});

// PW_CHANNEL=chrome uses the locally installed Chrome instead of a downloaded Chromium.
const chromium = process.env['PW_CHANNEL'] ? { channel: process.env['PW_CHANNEL'] } : {};

export default defineConfig({
  testDir: 'tests/e2e',
  fullyParallel: false,
  workers: 1,
  retries: process.env['CI'] ? 1 : 0,
  reporter: process.env['CI'] ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: { trace: 'retain-on-failure' },
  projects: [
    // The blocking end-to-end suite, on the CI browser (the bundled Chromium headless shell).
    { name: 'e2e', testIgnore: ['**/perf/**', '**/browsers/**'], use: { ...chromium } },
    // PS-0 and the first-run benchmark. Lenient unless PERF_STRICT=1 (reference machines).
    { name: 'perf', testMatch: '**/perf/**/*.spec.ts', use: { ...chromium } },
    // Other engines: the page boots and the compat check speaks. Informational (ADR-28).
    {
      name: 'firefox',
      testMatch: '**/browsers/**/*.spec.ts',
      use: { ...devices['Desktop Firefox'] },
    },
    {
      name: 'webkit',
      testMatch: '**/browsers/**/*.spec.ts',
      use: { ...devices['Desktop Safari'] },
    },
  ],
  webServer: [preview(HEADERS_PORT, 'on'), preview(PAGES_PORT, 'off')],
});
