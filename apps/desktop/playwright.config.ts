/**
 * UI tests of the dashboard in a browser, against the synthetic fixture client (the
 * same one `pnpm dev` serves). They cover the M3 journeys, keyboard use and automated
 * accessibility checks in both themes; the Rust core has its own tests.
 *
 * `PLAYWRIGHT_CHANNEL` picks an installed browser (e.g. `msedge`) instead of the
 * bundled Chromium, so a local run needs no browser download.
 */
import { defineConfig, devices } from "@playwright/test";

const CI = process.env["CI"] !== undefined;
const channel = process.env["PLAYWRIGHT_CHANNEL"];

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
    baseURL: "http://127.0.0.1:1420",
    navigationTimeout: 45_000,
    viewport: { width: 1440, height: 900 },
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"], ...(channel === undefined ? {} : { channel }) },
    },
  ],
  webServer: {
    command: "pnpm dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !CI,
    timeout: 120_000,
  },
});
