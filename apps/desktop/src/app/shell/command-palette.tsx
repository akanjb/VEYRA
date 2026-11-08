import { useEffect, useState, type KeyboardEvent } from "react";
import { Search } from "lucide-react";

import { Kbd, Modal, cn } from "@/shared/ui";

import type { Action } from "./actions";

const LIST_ID = "palette-list";
const optionId = (index: number) => `${LIST_ID}-${String(index)}`;

/** Actions whose label contains the query (case-insensitive). */
export function matchActions(actions: readonly Action[], query: string): Action[] {
  const needle = query.trim().toLowerCase();
  return actions.filter((a) => a.label.toLowerCase().includes(needle));
}

/** Search state and keyboard handling of the palette. */
function usePalette(actions: readonly Action[], close: () => void) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const matches = matchActions(actions, query);
  const run = (action: Action | undefined) => {
    if (action === undefined) return;
    close();
    setQuery("");
    action.run();
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter") run(matches[active]);
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const step = event.key === "ArrowDown" ? 1 : -1;
    setActive((i) => (i + step + matches.length) % Math.max(matches.length, 1));
  };
  const search = (value: string) => {
    setQuery(value);
    setActive(0);
  };
  return { query, search, active, setActive, matches, run, onKeyDown };
}

function PaletteList({
  matches,
  active,
  onHover,
  onRun,
}: {
  matches: readonly Action[];
  active: number;
  onHover: (index: number) => void;
  onRun: (action: Action) => void;
}) {
  useEffect(() => {
    document.getElementById(optionId(active))?.scrollIntoView({ block: "nearest" });
  }, [active]);
  if (matches.length === 0) {
    return (
      <p role="status" className="px-3 py-6 text-center text-sm text-muted">
        No matching action
      </p>
    );
  }
  return (
    <ul id={LIST_ID} role="listbox" aria-label="Actions" className="max-h-80 overflow-auto p-1">
      {matches.map((action, i) => (
        <li
          key={action.id}
          id={optionId(i)}
          role="option"
          aria-selected={i === active}
          onMouseEnter={() => {
            onHover(i);
          }}
          onClick={() => {
            onRun(action);
          }}
          className={cn(
            "flex cursor-default items-center gap-3 rounded-control px-3 py-2 text-sm",
            i === active && "bg-gray-3",
          )}
        >
          <span className="w-20 shrink-0 text-xs text-subtle">{action.group}</span>
          <span className="flex-1">{action.label}</span>
          {action.keys === undefined ? null : <Kbd>{action.keys}</Kbd>}
        </li>
      ))}
    </ul>
  );
}

/** Ctrl/⌘ K: search every action by name; arrows move, Enter runs (§9). */
export function CommandPalette({
  open,
  onOpenChange,
  actions,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  actions: readonly Action[];
}) {
  const palette = usePalette(actions, () => {
    onOpenChange(false);
  });
  return (
    <Modal open={open} onOpenChange={onOpenChange} title="Command palette">
      <div className="flex items-center gap-2 border-b border-border px-4">
        <Search size={16} className="text-subtle" aria-hidden />
        <input
          autoFocus
          role="combobox"
          aria-expanded={palette.matches.length > 0}
          aria-controls={LIST_ID}
          aria-autocomplete="list"
          aria-activedescendant={palette.matches.length > 0 ? optionId(palette.active) : undefined}
          aria-label="Search actions"
          placeholder="Search actions…"
          value={palette.query}
          onChange={(e) => {
            palette.search(e.target.value);
          }}
          onKeyDown={palette.onKeyDown}
          className="h-12 flex-1 bg-transparent text-sm outline-none"
        />
      </div>
      <PaletteList
        matches={palette.matches}
        active={palette.active}
        onHover={palette.setActive}
        onRun={palette.run}
      />
    </Modal>
  );
}

/** "?": every keyboard shortcut, from the same action list the handler uses. */
export function ShortcutHelp({
  open,
  onOpenChange,
  actions,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  actions: readonly Action[];
}) {
  const listed = [
    { id: "palette", label: "Open the command palette", keys: "Ctrl K" },
    ...actions.filter((a) => a.keys !== undefined),
  ];
  return (
    <Modal open={open} onOpenChange={onOpenChange} title="Keyboard shortcuts" showTitle>
      <div className="p-5 pt-3">
        <dl className="grid grid-cols-[1fr_auto] gap-x-6 gap-y-2 text-sm">
          {listed.map((a) => (
            <div key={a.id} className="contents">
              <dt>{a.label}</dt>
              <dd>
                <Kbd>{a.keys}</Kbd>
              </dd>
            </div>
          ))}
        </dl>
      </div>
    </Modal>
  );
}
