import { useEffect, useRef } from "react";

import type { Action } from "./actions";

/** How long after "g" the second key of a navigation shortcut is accepted. */
const CHORD_MS = 1_000;

function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

/**
 * Global keyboard shortcuts: Ctrl/⌘ K opens the palette; every other key comes from the
 * actions' `keys` (single keys, or "g" chords). Ignored while typing.
 */
export function useShortcuts(list: readonly Action[], openPalette: () => void): void {
  const latest = useRef(list);
  useEffect(() => {
    latest.current = list;
  }, [list]);
  useEffect(() => {
    let chordAt = 0;
    const onKey = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        openPalette();
        return;
      }
      if (event.ctrlKey || event.metaKey || event.altKey || typing(event.target)) return;
      const chord = Date.now() - chordAt < CHORD_MS;
      chordAt = 0;
      if (!chord && event.key === "g") {
        chordAt = Date.now();
        return;
      }
      const keys = chord ? `g ${event.key}` : event.key;
      const action = latest.current.find((a) => a.keys === keys);
      if (action !== undefined) {
        event.preventDefault();
        action.run();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [openPalette]);
}
