import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`activity has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/activity", { theme });
    for (const title of [
      "Active and wall time per day",
      "Session lengths",
      "Peak hours and busiest days",
      "Longest sessions",
      "Activity calendar",
      "Hours of the week",
    ]) {
      await expect(page.getByRole("heading", { name: title })).toBeVisible();
    }
    await expectAccessible(page);
  });
}

test("session lengths switch between active and wall time", async ({ page }) => {
  await open(page, "/activity");
  await page.getByRole("radio", { name: "Wall time" }).click();
  await expect(page.getByRole("img", { name: "Sessions by wall time" })).toBeVisible();
});

test("a long session opens its detail", async ({ page }) => {
  await open(page, "/activity");
  const table = page.getByRole("table", { name: "Longest sessions" });
  await table.locator("tbody tr").first().click();
  await expect(page).toHaveURL(/\/sessions\/\d+/);
});
