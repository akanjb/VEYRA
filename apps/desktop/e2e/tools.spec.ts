import { expect, test } from "@playwright/test";

import { THEMES, expectAccessible, open } from "./app";

for (const theme of THEMES) {
  test(`tools has no accessibility violations (${theme})`, async ({ page }) => {
    await open(page, "/tools", { theme });
    await expect(page.getByRole("heading", { name: "Tool calls by kind" })).toBeVisible();
    await expectAccessible(page);
  });
}

test("tools shows MCP servers and the tools that fail", async ({ page }) => {
  await open(page, "/tools");
  await expect(page.getByRole("table", { name: "MCP servers" }).getByText("github")).toBeVisible();
  await expect(
    page.getByRole("table", { name: "Tools that fail most" }).getByText("Bash"),
  ).toBeVisible();
});
