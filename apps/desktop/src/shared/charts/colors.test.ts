import { describe, expect, it } from "vitest";

import { categoryColor, modelColors, providerColor, readableOn, tint } from "./colors";
import type { ChartTheme } from "./theme";

const theme: ChartTheme = {
  text: "#000000",
  muted: "#111111",
  subtle: "#222222",
  border: "#333333",
  surface: "#ffffff",
  raised: "#ffffff",
  accent: "#0000ff",
  font: "Inter",
  data: [
    "#d0602a",
    "#0e8f7e",
    "#3567d6",
    "#b5179e",
    "#8f5b00",
    "#5d6b80",
    "#0a84a3",
    "#448a26",
    "#8550e6",
    "#b33f3f",
  ],
  seq: [],
};

describe("identity colours", () => {
  it("gives each provider a fixed hue", () => {
    expect(providerColor(theme, "anthropic")).toBe("#d0602a");
    expect(providerColor(theme, "openai")).toBe("#0e8f7e");
    expect(providerColor(theme, "google")).toBe("#3567d6");
    expect(providerColor(theme, "mistral", 0)).toBe(categoryColor(theme, 0));
  });

  it("keeps categories off the provider hues", () => {
    expect(categoryColor(theme, 0)).toBe("#b5179e");
    expect(categoryColor(theme, 7)).toBe("#b5179e");
  });

  it("tints later models of a provider within its hue", () => {
    const colors = modelColors(theme, [
      { id: "claude-opus-5-5", provider: "anthropic" },
      { id: "gpt-6", provider: "openai" },
      { id: "claude-sonnet-5-5", provider: "anthropic" },
    ]);
    expect(colors.get("claude-opus-5-5")).toBe("#d0602a");
    expect(colors.get("gpt-6")).toBe("#0e8f7e");
    expect(colors.get("claude-sonnet-5-5")).toBe(tint("#d0602a", 0.26, "#ffffff"));
  });

  it("mixes colours channel by channel", () => {
    expect(tint("#000000", 0.5, "#ffffff")).toBe("#808080");
    expect(tint("not a colour", 0.5, "#ffffff")).toBe("not a colour");
  });
});

describe("label contrast", () => {
  it("puts dark text on light fills and white text on dark fills", () => {
    expect(readableOn("#d9b44a")).toBe("#0b0d11");
    expect(readableOn("#3567d6")).toBe("#ffffff");
  });
});
