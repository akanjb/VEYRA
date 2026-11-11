import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`cache has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/cache", { theme });
    for (const title of [
      "Cache reads and writes",
      "Hit ratio by model",
      "Hit ratio by project",
      "Cache writes by lifetime",
      "Sessions where caching cost more than it saved",
    ]) {
      await expect(page.getByRole("heading", { name: title })).toBeVisible();
    }
    await expectAccessible(page);
  });
}

test("the hit-ratio table lists the cache reads behind each ratio", async ({ page }) => {
  await open(page, "/cache");
  await page.getByRole("button", { name: "Show Hit ratio by model as a table" }).click();
  const table = page.getByRole("table", { name: "Hit ratio by model" });
  await expect(table.getByRole("columnheader", { name: "Cache reads" })).toBeVisible();
  await expect(table.getByRole("row")).not.toHaveCount(1);
});
