import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`models and providers has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/models", { theme });
    for (const title of [
      "Provider share over time",
      "Model leaderboard",
      "Providers by application",
      "Models by application",
      "When each model was used",
    ]) {
      await expect(page.getByRole("heading", { name: title })).toBeVisible();
    }
    await expectAccessible(page);
  });
}

test("ticking two models compares them side by side", async ({ page }) => {
  await open(page, "/models");
  const boxes = page.getByRole("checkbox", { name: /^Compare / });
  await expect(page.getByRole("heading", { name: "Model comparison" })).toBeHidden();
  await boxes.nth(0).check();
  await boxes.nth(1).check();
  await expect(page.getByRole("heading", { name: "Model comparison" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Cost per 1M output tokens" })).toBeVisible();
  await expectAccessible(page);
});
