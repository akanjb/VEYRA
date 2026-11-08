import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

test("UC-01 first run: the privacy promise, consent, then data", async ({ page }) => {
  await open(page, "/", { fresh: true });
  await expect(page.getByRole("heading", { name: "Welcome to Veyra" })).toBeVisible();
  await expect(page.getByText("Prompts, responses or conversation text")).toBeVisible();
  await expect(page.getByRole("link", { name: "No sources enabled" })).toBeVisible();
  await expectAccessible(page);

  await page.getByRole("switch", { name: "Read Claude Code usage" }).click();
  await expect(page.getByRole("heading", { name: "Welcome to Veyra" })).toBeHidden();
  await expect(page.getByRole("link", { name: "1 source" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Daily tokens by application" })).toBeVisible();
});

test("skipping consent shows unknown values, never zeros", async ({ page }) => {
  await open(page, "/", { fresh: true });
  await page.getByRole("button", { name: "Not now" }).click();
  await expect(page.getByText("No usage recorded in this period")).toBeVisible();
  // Tokens, requests and cache hit ratio are unknown without data.
  await expect(page.locator("main").getByText("—", { exact: true })).toHaveCount(3);
});

for (const theme of THEMES) {
  test(`sources and health has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/sources", { theme });
    await page.getByText(/problems? found in the last 7 days/).click();
    await expect(page.getByText("Lines that are not valid JSON")).toBeVisible();
    await expectAccessible(page);
  });
}
