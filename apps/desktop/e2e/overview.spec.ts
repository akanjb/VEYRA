import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`the overview has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/", { theme });
    await expect(page.getByRole("heading", { name: "Daily tokens by application" })).toBeVisible();
    await expect(
      page.getByRole("list", { name: "Highlights" }).getByText(/^Busiest day:/),
    ).toBeVisible();
    await expectAccessible(page);
  });
}

test("a period change is a history entry that Back undoes", async ({ page }) => {
  await open(page, "/");
  await page.getByRole("button", { name: "Period: Last 30 days" }).click();
  await page.getByRole("menuitemradio", { name: "Last 7 days" }).click();
  await expect(page).toHaveURL(/range=7d/);
  await expect(page.getByRole("button", { name: "Period: Last 7 days" })).toBeVisible();

  await page.goBack();
  await expect(page).not.toHaveURL(/range=/);
  await expect(page.getByRole("button", { name: "Period: Last 30 days" })).toBeVisible();
});

test("a filter chosen in the sheet shows as a named chip and survives navigation", async ({
  page,
}) => {
  await open(page, "/");
  await page.getByRole("button", { name: "Filters" }).click();
  const sheet = page.getByRole("dialog", { name: "Filters" });
  await sheet.getByText("Claude Opus 5.5", { exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page).toHaveURL(/model=claude-opus-5-5/);
  await expect(page.getByRole("button", { name: "Remove filter Claude Opus 5.5" })).toBeVisible();

  await page
    .getByRole("navigation", { name: "Pages" })
    .getByRole("link", { name: "Sessions" })
    .click();
  await expect(page).toHaveURL(/\/sessions\?.*model=claude-opus-5-5/);
});
