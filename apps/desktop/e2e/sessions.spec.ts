import { expect, test, type Page } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

/** Presses Tab until a session row has focus (keyboard only, no clicking). */
async function tabToFirstRow(page: Page): Promise<void> {
  for (let presses = 0; presses < 80; presses += 1) {
    await page.keyboard.press("Tab");
    const onRow = await page.evaluate(() => document.activeElement?.tagName === "TR");
    if (onRow) return;
  }
  throw new Error("no session row is reachable with Tab");
}

test("UC-02 by keyboard: palette to sessions, open the top session, read it", async ({ page }) => {
  await open(page, "/");
  await page.keyboard.press("Control+k");
  await page.keyboard.type("go to sessions");
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/sessions/);
  await expect(page.getByRole("heading", { level: 1, name: "Sessions" })).toBeVisible();

  await tabToFirstRow(page);
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/sessions\/\d+/);
  await expect(page.getByRole("heading", { name: "Timeline" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Tool calls" })).toBeVisible();

  await page.goBack();
  await expect(page).toHaveURL(/\/sessions(\?|$)/);
});

for (const theme of THEMES) {
  test(`the sessions pages have no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/sessions", { theme });
    await expect(page.locator("tbody tr").first()).toBeVisible();
    await expectAccessible(page);

    await page.locator("tbody tr").first().click();
    await expect(page.getByRole("heading", { name: "Timeline" })).toBeVisible();
    await expectAccessible(page);
  });
}
