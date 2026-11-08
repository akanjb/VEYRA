/**
 * Identity colours, the single place they are assigned (docs/planning/03-ux.md §1):
 * a provider always has the same hue, models are tints within their provider's hue, and
 * other categories (applications, projects) take the remaining palette slots in order.
 */
import type { ChartTheme } from "./theme";

/** Palette slots reserved for providers; slots 1–3 in tokens.css. */
const PROVIDER_SLOTS: Readonly<Record<string, number>> = { anthropic: 0, openai: 1, google: 2 };
/** First slot for non-provider categories, so they never look like a provider. */
const CATEGORY_START = 3;
/** How far successive models of one provider move toward the surface colour. */
const TINT_STEP = 0.26;

/** `hex` mixed toward `toward` by `amount` (0–1). */
export function tint(hex: string, amount: number, toward: string): string {
  const parse = (h: string) => [1, 3, 5].map((i) => Number.parseInt(h.slice(i, i + 2), 16));
  const [a, b] = [parse(hex), parse(toward)];
  if (a.some(Number.isNaN) || b.some(Number.isNaN)) return hex;
  return `#${a
    .map((c, i) =>
      Math.round(c + ((b[i] ?? c) - c) * amount)
        .toString(16)
        .padStart(2, "0"),
    )
    .join("")}`;
}

/** The colour of the `index`-th non-provider category (application, project, …). */
export function categoryColor(theme: ChartTheme, index: number): string {
  const slots = theme.data.length - CATEGORY_START;
  return theme.data[CATEGORY_START + (index % slots)] ?? theme.accent;
}

/** The identity hue of a provider; unknown providers take category slots. */
export function providerColor(
  theme: ChartTheme,
  provider: string | null,
  fallbackIndex = 0,
): string {
  const slot = provider === null ? undefined : PROVIDER_SLOTS[provider];
  return slot === undefined
    ? categoryColor(theme, fallbackIndex)
    : (theme.data[slot] ?? theme.accent);
}

/**
 * Colours for models, in the given order (most used first): the first model of a provider
 * takes its pure hue, later ones progressively lighter tints of it.
 */
export function modelColors(
  theme: ChartTheme,
  models: readonly { id: string; provider: string | null }[],
): Map<string, string> {
  const seen = new Map<string, number>();
  const colors = new Map<string, string>();
  for (const [index, model] of models.entries()) {
    if (colors.has(model.id)) continue;
    const family = model.provider ?? "unknown";
    const rank = seen.get(family) ?? 0;
    seen.set(family, rank + 1);
    const base = providerColor(theme, model.provider, index);
    colors.set(model.id, tint(base, Math.min(rank * TINT_STEP, 0.7), theme.surface));
  }
  return colors;
}

/** Neutral colour for "Other" and unknown groups. */
export function otherColor(theme: ChartTheme): string {
  return theme.subtle;
}

/** Relative luminance (WCAG) of a `#rrggbb` colour. */
function luminance(hex: string): number {
  const channel = (i: number) => {
    const c = Number.parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

/** Black or white, whichever reads better on `background` (labels inside data marks). */
export function readableOn(background: string): string {
  return luminance(background) > 0.32 ? "#0b0d11" : "#ffffff";
}
