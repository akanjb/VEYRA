import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`storage has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/storage", { theme });
    await expect(page.getByRole("table", { name: "Events recorded per month" })).toBeVisible();
    await expectAccessible(page);
  });
}

test("a backup can be taken and previewed before restoring", async ({ page }) => {
  await open(page, "/storage");
  await expect(page.getByText("No backups yet.")).toBeVisible();
  await page.getByRole("button", { name: "Back up now" }).click();
  const backups = page.getByRole("list", { name: "Backups" });
  await expect(backups.getByRole("listitem")).toHaveCount(1);

  await backups.getByRole("button", { name: /^Restore the backup of/ }).click();
  const dialog = page.getByRole("dialog", { name: "Restore backup" });
  await expect(dialog.getByText("Stored now")).toBeVisible();
  await expectAccessible(page);
  await dialog.getByRole("button", { name: "Restore and restart" }).click();
  await expect(dialog.getByRole("status")).toHaveText("Restarting…");
});
