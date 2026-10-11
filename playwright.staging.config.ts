import { defineConfig } from '@playwright/test';

/** Smoke test against the live staging URL (STAGING_URL, EXPECTED_SHA). No local servers. */
export default defineConfig({
  testDir: 'tests/staging',
  fullyParallel: false,
  workers: 1,
  retries: 1,
  reporter: [['list']],
  timeout: 240_000,
  use: { trace: 'retain-on-failure' },
});
