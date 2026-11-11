/**
 * The production build under the production Content Security Policy: every page and
 * overlay must work without a single policy violation.
 */
import { expect, test, type Page } from "@playwright/test";

import { open } from "./app";

/** Collects violations reported by the page from the first script on. */
async function recordViolations(page: Page): Promise<() => Promise<string[]>> {
  await page.addInitScript(() => {
    const seen: string[] = [];
    (window as unknown as { __violations: string[] }).__violations = seen;
    document.addEventListener("securitypolicyviolation", (e) => {
      seen.push(`${e.violatedDirective} blocked ${e.blockedURI || "inline"} ${e.sample}`.trim());
    });
  });
  return () => page.evaluate(() => (window as unknown as { __violations: string[] }).__violations);
}

/** Hovers the middle of the first chart so its tooltip renders. */
async function hoverChart(page: Page): Promise<void> {
  const chart = page.getByRole("img").first();
  await chart.scrollIntoViewIfNeeded();
  const box = await chart.boundingBox();
  if (box !== null) await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
}

/** Opens a page from the sidebar, waits for one of its headings and hovers a chart. */
async function visit(page: Page, link: string, heading: string): Promise<void> {
  await page.getByRole("navigation", { name: "Pages" }).getByRole("link", { name: link }).click();
  await expect(page.getByRole("heading", { name: heading })).toBeVisible();
  await hoverChart(page);
}

test("every page and overlay runs without a CSP violation", async ({ page }) => {
  const violations = await recordViolations(page);

  await open(page, "/");
  await expect(page.getByRole("heading", { name: "Daily tokens by application" })).toBeVisible();
  await hoverChart(page);
  await page.getByRole("button", { name: /Open command palette/ }).click();
  await expect(page.getByRole("combobox", { name: "Search actions" })).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Filters" }).click();
  await expect(page.getByRole("dialog", { name: "Filters" })).toBeVisible();
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /^Period:/ }).click();
  await expect(page.getByRole("menuitemradio").first()).toBeVisible();
  await page.keyboard.press("Escape");

  await visit(page, "Usage", "Token composition");

  await visit(page, "Cost", "Daily cost");

  await visit(page, "Cache", "Cache reads and writes");

  await visit(page, "Models & Providers", "Model leaderboard");

  await visit(page, "Activity", "Longest sessions");

  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("link", { name: "Projects" })
    .click();
  await page.getByRole("table", { name: "Projects" }).locator("tbody tr").first().click();
  await expect(page.getByRole("heading", { name: "Most expensive sessions" })).toBeVisible();
  await hoverChart(page);
  await page.getByRole("button", { name: "Rename" }).click();
  await expect(page.getByRole("dialog", { name: "Rename project" })).toBeVisible();
  await page.keyboard.press("Escape");

  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("link", { name: "Sessions" })
    .click();
  await page.locator("tbody tr").first().click();
  await expect(page.getByRole("heading", { name: "Timeline" })).toBeVisible();
  await hoverChart(page);

  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("link", { name: "Sources & Health" })
    .click();
  await expect(page.getByRole("switch", { name: "Read Claude Code usage" })).toBeVisible();

  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("link", { name: "Settings" })
    .click();
  await page.getByRole("button", { name: "Add budget" }).click();
  await expect(page.getByRole("dialog", { name: "Add budget" })).toBeVisible();
  await page.keyboard.press("Escape");

  expect(await violations()).toEqual([]);
});

test("the first run works without a CSP violation", async ({ page }) => {
  const violations = await recordViolations(page);
  await open(page, "/", { fresh: true });
  await page.getByRole("switch", { name: "Read Claude Code usage" }).click();
  await expect(page.getByRole("heading", { name: "Daily tokens by application" })).toBeVisible();
  expect(await violations()).toEqual([]);
});
