/**
 * Light, dark or system appearance: one store shared by every control that changes it,
 * remembered per device (a convenience, not data).
 */
import { useSyncExternalStore } from "react";

export type ThemePreference = "light" | "dark" | "system";

const KEY = "veyra.theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

function stored(): ThemePreference {
  try {
    const value = localStorage.getItem(KEY);
    return value === "light" || value === "dark" ? value : "system";
  } catch {
    return "system";
  }
}

let preference: ThemePreference = "system";
const listeners = new Set<() => void>();

/** Sets `data-theme` on the root element, resolving "system" from the OS setting. */
function apply(): void {
  const dark = preference === "dark" || (preference === "system" && matchMedia(DARK_QUERY).matches);
  document.documentElement.dataset["theme"] = dark ? "dark" : "light";
}

/** Applies the remembered theme before the first render and follows OS changes. */
export function initTheme(): void {
  preference = stored();
  apply();
  matchMedia(DARK_QUERY).addEventListener("change", () => {
    if (preference === "system") apply();
  });
}

export function setTheme(next: ThemePreference): void {
  preference = next;
  try {
    localStorage.setItem(KEY, next);
  } catch {
    // Storage may be unavailable; the choice still applies for this session.
  }
  apply();
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** The current preference; re-renders when any control changes it. */
export function useTheme(): [ThemePreference, (next: ThemePreference) => void] {
  return [useSyncExternalStore(subscribe, () => preference), setTheme];
}
