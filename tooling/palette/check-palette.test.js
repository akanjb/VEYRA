// @ts-check
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import {
  checkPalette,
  checkTheme,
  contrast,
  deltaE2000,
  parseHex,
  simulate,
  variables,
} from "./check-palette.js";

const TOKENS = new URL("../../apps/desktop/src/styles/tokens.css", import.meta.url);

describe("palette check", () => {
  it("passes for the shipped design tokens, in both themes", () => {
    expect(checkPalette(readFileSync(TOKENS, "utf8"))).toEqual([]);
  });

  it("measures contrast and colour difference like the references", () => {
    expect(contrast(parseHex("#000000"), parseHex("#ffffff"))).toBeCloseTo(21, 5);
    expect(deltaE2000(parseHex("#3567d6"), parseHex("#3567d6"))).toBe(0);
    // Opposite hues are far apart.
    expect(deltaE2000(parseHex("#ff0000"), parseHex("#00ff00"))).toBeGreaterThan(80);
    // Red and green collapse under deuteranopia.
    const [red, green] = [parseHex("#d03030"), parseHex("#30a030")];
    expect(deltaE2000(simulate(red, "deuteranopia"), simulate(green, "deuteranopia"))).toBeLessThan(
      deltaE2000(red, green),
    );
  });

  it("reports low contrast, near-duplicates and a wrong slot count", () => {
    const failures = checkTheme(
      { surface: "#ffffff", "data-1": "#fefefe", "data-2": "#fdfdfd" },
      "test",
    );
    expect(failures.some((f) => f.includes("expected 10 data slots"))).toBe(true);
    expect(failures.some((f) => f.includes("data-1 contrast"))).toBe(true);
    expect(failures.some((f) => f.includes("data-1/data-2 ΔE"))).toBe(true);
  });

  it("reads only #rrggbb variables of the selected block", () => {
    const css = ':root { --a: #010203; --b: var(--a); } :root[data-theme="dark"] { --a: #ffffff; }';
    expect(variables(css, ":root")).toEqual({ a: "#010203" });
    expect(() => parseHex("red")).toThrow("not a #rrggbb colour");
  });
});
