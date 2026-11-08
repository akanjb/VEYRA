/** Shared steps of the UI tests: opening the dashboard and checking accessibility. */
import AxeBuilder from "@axe-core/playwright";
import { expect, type Page } from "@playwright/test";

export type Theme = "light" | "dark";
export const THEMES: readonly Theme[] = ["light", "dark"];

/** The key the app remembers its theme under (src/app/theme.ts). */
const THEME_KEY = "veyra.theme";

/**
 * Opens a dashboard path in a theme and waits for the shell. `fresh` starts like a
 * first run: the source is detected but not yet enabled.
 */
export async function open(
  page: Page,
  path: string,
  { theme = "dark", fresh = false }: { theme?: Theme; fresh?: boolean } = {},
): Promise<void> {
  await page.addInitScript(
    ([key, value]) => {
      localStorage.setItem(key, value);
    },
    [THEME_KEY, theme] as const,
  );
  const url = fresh ? `${path}${path.includes("?") ? "&" : "?"}fresh` : path;
  await page.goto(url);
  await expect(page.locator("html")).toHaveAttribute("data-theme", theme);
  await expect(page.getByRole("navigation")).toBeVisible();
}

/** Fails on any WCAG 2.2 A/AA violation found by axe on the current page. */
export async function expectAccessible(page: Page): Promise<void> {
  const results = await new AxeBuilder({ page })
    .withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
    .analyze();
  const summary = results.violations.flatMap((v) =>
    v.nodes.map(
      (n) =>
        `${v.id} (${v.impact ?? "n/a"}) at ${n.target.join(" ")}: ${n.html.slice(0, 160)} — ${n.failureSummary ?? ""}`,
    ),
  );
  expect(summary).toEqual([]);
}
