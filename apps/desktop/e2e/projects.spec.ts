import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`projects have no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/projects", { theme });
    await expect(page.getByRole("table", { name: "Projects" })).toBeVisible();
    await expectAccessible(page);

    await page.getByRole("table", { name: "Projects" }).locator("tbody tr").first().click();
    await expect(page.getByRole("heading", { name: "Most expensive sessions" })).toBeVisible();
    await expectAccessible(page);
  });
}

test("a project can be renamed, and the name shows everywhere", async ({ page }) => {
  await open(page, "/projects");
  await page.getByRole("button", { name: "Rename c:/users/dev/code/storefront" }).click();
  const dialog = page.getByRole("dialog", { name: "Rename project" });
  await dialog.getByLabel("Name").fill("Storefront");
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog).toBeHidden();
  await expect(
    page.getByRole("table", { name: "Projects" }).getByText("Storefront", { exact: true }),
  ).toBeVisible();
});

test("merging moves a project under another, and unmerging restores it", async ({ page }) => {
  await open(page, "/projects");
  const table = page.getByRole("table", { name: "Projects" });
  await page.getByRole("button", { name: "Merge /home/dev/scratch into another project" }).click();
  const dialog = page.getByRole("dialog", { name: "Merge project" });
  await dialog.getByLabel(/Merge .* into/).selectOption({ label: "c:/users/dev/code/veyra" });
  await dialog.getByRole("button", { name: "Merge" }).click();
  await expect(dialog).toBeHidden();
  await expect(table.getByText("/home/dev/scratch", { exact: true })).toBeHidden();
  await expect(table.getByText("+ 1 merged folder")).toBeVisible();

  await table.getByText("c:/users/dev/code/veyra", { exact: true }).click();
  await page.getByRole("button", { name: "Unmerge /home/dev/scratch" }).click();
  await expect(page.getByRole("list", { name: "Merged folders" })).toBeHidden();
});
