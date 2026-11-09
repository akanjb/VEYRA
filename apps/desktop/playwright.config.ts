/**
 * UI tests of the dashboard in a browser, against the synthetic fixture client (the
 * same one `pnpm dev` serves). They cover the M3 journeys, keyboard use and automated
 * accessibility checks in both themes; the Rust core has its own tests.
 *
 * Two projects:
 * - `dev`: the dev server, for journeys and accessibility;
 * - `production-csp`: a production build (fixture mode) served with the Content Security
 *   Policy of tauri.conf.json, to catch what only the strict policy blocks.
 *
 * `PLAYWRIGHT_CHANNEL` picks an installed browser (e.g. `msedge`) instead of the
 * bundled Chromium, so a local run needs no browser download.
 */
import { defineConfig, devices } from "@playwright/test";

const CI = process.env["CI"] !== undefined;
const channel = process.env["PLAYWRIGHT_CHANNEL"];
const browser = { ...devices["Desktop Chrome"], ...(channel === undefined ? {} : { channel }) };

const DEV_URL = "http://127.0.0.1:1420";
const CSP_URL = "http://127.0.0.1:4181";

export default defineConfig({
  testDir: "e2e",
  fullyParallel: false,
  workers: 1,
  forbidOnly: CI,
  retries: CI ? 1 : 0,
  // Failures (with axe's explanation) are printed, so the CI log is the report.
  reporter: "list",
  // The dev server compiles on first request, which can take a while on a cold start.
  timeout: 60_000,
  expect: { timeout: 10_000 },
  use: {
    viewport: { width: 1440, height: 900 },
    navigationTimeout: 45_000,
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "dev",
      testIgnore: "csp.spec.ts",
      use: { ...browser, baseURL: DEV_URL },
    },
    {
      name: "production-csp",
      testMatch: "csp.spec.ts",
      use: { ...browser, baseURL: CSP_URL },
    },
  ],
  webServer: [
    {
      command: "pnpm dev",
      url: DEV_URL,
      reuseExistingServer: !CI,
      timeout: 120_000,
    },
    {
      command:
        "pnpm exec vite build --mode fixtures --outDir dist-fixtures --emptyOutDir && node ../../tooling/csp/serve-with-csp.js dist-fixtures 4181",
      url: CSP_URL,
      reuseExistingServer: false,
      timeout: 180_000,
    },
  ],
});
