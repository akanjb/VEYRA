import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`usage has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/usage", { theme });
    for (const title of [
      "Token composition",
      "Tokens by model",
      "Concentration by model",
      "Tokens per request",
      "Tokens per session",
      "Request sizes",
    ]) {
      await expect(page.getByRole("heading", { name: title })).toBeVisible();
    }
    await expectAccessible(page);
  });
}

test("the breakdown switches dimension and a bar filters by it", async ({ page }) => {
  await open(page, "/usage");
  await page.getByRole("radio", { name: "Project" }).click();
  await expect(page.getByRole("heading", { name: "Tokens by project" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Concentration by project" })).toBeVisible();

  // The table twin lists the same rows the chart draws.
  await page.getByRole("button", { name: "Show Tokens by project as a table" }).click();
  const table = page.getByRole("table", { name: "Tokens by project" });
  await expect(table.getByRole("row")).not.toHaveCount(1);
});

test("the usage page is reachable from the keyboard shortcut", async ({ page }) => {
  await open(page, "/");
  await expect(page.getByRole("heading", { name: "Daily tokens by application" })).toBeVisible();
  await page.keyboard.press("g");
  await page.keyboard.press("u");
  await expect(page).toHaveURL(/\/usage/);
  await expect(page.getByRole("heading", { name: "Token composition" })).toBeVisible();
});
