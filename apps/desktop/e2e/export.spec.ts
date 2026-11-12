import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`the export dialog has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/projects", { theme });
    await page.getByRole("button", { name: "Export report" }).click();
    const dialog = page.getByRole("dialog", { name: "Export report" });
    await expect(dialog.getByRole("region", { name: "Report preview" })).toBeVisible();
    await expectAccessible(page);
  });
}

test("a report starts from the page and can hide project names", async ({ page }) => {
  await open(page, "/projects");
  await page.getByRole("button", { name: "Export report" }).click();
  const dialog = page.getByRole("dialog", { name: "Export report" });
  await expect(dialog.getByLabel("Report")).toHaveValue("byProject");
  const preview = dialog.getByRole("region", { name: "Report preview" });
  await expect(preview.getByRole("columnheader", { name: "Path" })).toBeVisible();

  await dialog.getByRole("switch", { name: "Hash project names" }).click();
  await expect(preview.getByRole("columnheader", { name: "Path" })).toBeHidden();
  await expect(preview.getByText("c:/users/dev/code/veyra")).toBeHidden();
  await expect(dialog.getByRole("switch", { name: "Leave out folder paths" })).toBeDisabled();

  await dialog.getByRole("button", { name: "Save…" }).click();
  await expect(dialog.getByRole("status")).toHaveText(/Saved \d+ rows\./);
});

test("pages without a date range offer no export", async ({ page }) => {
  await open(page, "/settings");
  await expect(page.getByRole("heading", { level: 1, name: "Settings" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Export report" })).toBeHidden();
});
