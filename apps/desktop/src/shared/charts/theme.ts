/**
 * Chart theme read from the design tokens at runtime, so charts follow light and dark
 * themes with no second palette (docs/planning/03-ux.md §5.1).
 */
import { useSyncExternalStore } from "react";

export interface ChartTheme {
  text: string;
  muted: string;
  subtle: string;
  border: string;
  surface: string;
  raised: string;
  accent: string;
  font: string;
  data: string[];
  seq: string[];
}

function read(): ChartTheme {
  const style = getComputedStyle(document.documentElement);
  const v = (name: string) => style.getPropertyValue(`--${name}`).trim();
  return {
    text: v("text") || v("gray-12"),
    muted: v("text-muted") || v("gray-11"),
    subtle: v("text-subtle") || v("gray-10"),
    border: v("border"),
    surface: v("surface"),
    raised: v("surface-raised"),
    accent: v("accent"),
    font: v("font-sans") || "system-ui",
    data: Array.from({ length: 10 }, (_, i) => v(`data-${String(i + 1)}`)),
    seq: Array.from({ length: 7 }, (_, i) => v(`seq-${String(i)}`)),
  };
}

let cached: ChartTheme | null = null;
let themeAttribute: string | undefined;

function snapshot(): ChartTheme {
  const current = document.documentElement.dataset["theme"];
  if (cached === null || current !== themeAttribute) {
    themeAttribute = current;
    cached = read();
  }
  return cached;
}

function subscribe(onChange: () => void): () => void {
  const observer = new MutationObserver(onChange);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  return () => {
    observer.disconnect();
  };
}

/** The current chart theme; components re-render when the theme changes. */
export function useChartTheme(): ChartTheme {
  return useSyncExternalStore(subscribe, snapshot);
}
