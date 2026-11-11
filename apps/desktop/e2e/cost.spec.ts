import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`cost has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/cost", { theme });
    await expect(page.getByRole("heading", { name: "Daily cost" })).toBeVisible();
    await expectAccessible(page);
  });

  test(`settings have no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/settings", { theme });
    await expect(page.getByRole("heading", { name: "Billing entries" })).toBeVisible();
    await expectAccessible(page);
    await page.getByRole("button", { name: "Add entry" }).click();
    await expect(page.getByRole("dialog", { name: "Add billing entry" })).toBeVisible();
    await expectAccessible(page);
  });
}

test("a subscription shows beside the estimate, never added to it", async ({ page }) => {
  await open(page, "/settings");
  await page.getByRole("button", { name: "Add entry" }).click();
  const dialog = page.getByRole("dialog", { name: "Add billing entry" });
  await dialog.getByLabel("Name").fill("Team plan");
  await dialog.getByLabel("Amount").fill("20");
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog).toBeHidden();
  await expect(
    page.getByRole("table", { name: "Billing entries" }).getByText("Team plan"),
  ).toBeVisible();

  await page.getByRole("navigation", { name: "Pages" }).getByRole("link", { name: "Cost" }).click();
  await expect(page.getByText(/Subscriptions \$.* compared, never added/)).toBeVisible();
});

test("a budget appears with its state, and refused input shows the core's error", async ({
  page,
}) => {
  await open(page, "/settings");
  await page.getByRole("button", { name: "Add budget" }).click();
  const dialog = page.getByRole("dialog", { name: "Add budget" });
  await dialog.getByLabel("Name").fill("Monthly AI spend");
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog.getByRole("alert")).toHaveText("the amount must be greater than zero");
  await dialog.getByLabel("Monthly limit (USD)").fill("0.01");
  await dialog.getByRole("button", { name: "Save" }).click();
  await expect(dialog).toBeHidden();

  await page.getByRole("navigation", { name: "Pages" }).getByRole("link", { name: "Cost" }).click();
  await expect(page.getByRole("meter", { name: "Monthly AI spend: Over budget" })).toBeVisible();
});

test("breakdowns and the waterfall have tables, and the waterfall switches dimension", async ({
  page,
}) => {
  await open(page, "/cost");
  await page.getByRole("button", { name: "Show Cost by model as a table" }).click();
  await expect(page.getByRole("table", { name: "Cost by model" })).toBeVisible();
  const change = page.getByRole("region", { name: "Change from the previous period" });
  await change.getByRole("radio", { name: "Project" }).click();
  await change.getByRole("button", { name: /as a table/ }).click();
  await expect(change.getByRole("cell", { name: "Previous period" })).toBeVisible();
});
