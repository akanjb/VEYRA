/**
 * Everything the user can do from the keyboard: the single source for the command
 * palette, the shortcut handler and the "?" help list.
 */
import { RANGES, filterState, rangeLabel, stepRange, type Filters } from "@/shared/filters";

import { PAGES } from "../navigation";
import { setTheme, type ThemePreference } from "../theme";

export interface Action {
  id: string;
  label: string;
  group: string;
  /** Shortcut as shown to the user, if any. */
  keys?: string;
  run: () => void;
}

export interface ActionContext {
  filters: Filters;
  navigate: (to: string) => void;
  toggleSidebar: () => void;
  openHelp: () => void;
  rescan: () => void;
}

/** Ids of actions that only make sense on pages with the query bar. */
export const SCOPED_ACTION_PREFIX = "period:";

const THEMES: readonly { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Theme: light" },
  { value: "dark", label: "Theme: dark" },
  { value: "system", label: "Theme: follow system" },
];

function navigation(ctx: ActionContext): Action[] {
  return PAGES.map((page) => ({
    id: `go:${page.to}`,
    label: `Go to ${page.title}`,
    group: "Navigate",
    keys: `g ${page.shortcut}`,
    run: () => {
      ctx.navigate(page.to);
    },
  }));
}

function period({ filters }: ActionContext): Action[] {
  const { state, setState, today } = filters;
  const presets = RANGES.filter((r) => r !== "custom").map((range) => ({
    id: `${SCOPED_ACTION_PREFIX}${range}`,
    label: `Period: ${rangeLabel(filterState({ range }))}`,
    group: "Period",
    run: () => {
      setState({ ...state, range, from: null, to: null });
    },
  }));
  const step = (direction: -1 | 1, label: string, keys: string): Action => ({
    id: `${SCOPED_ACTION_PREFIX}step${String(direction)}`,
    label,
    group: "Period",
    keys,
    run: () => {
      setState(stepRange(state, direction, today));
    },
  });
  const compare: Action = {
    id: `${SCOPED_ACTION_PREFIX}compare`,
    label: state.compare ? "Turn comparison off" : "Compare with previous period",
    group: "Period",
    run: () => {
      setState({ ...state, compare: !state.compare });
    },
  };
  return [...presets, step(-1, "Previous period", ","), step(1, "Next period", "."), compare];
}

function appearance(ctx: ActionContext): Action[] {
  const themes = THEMES.map((t) => ({
    id: `theme:${t.value}`,
    label: t.label,
    group: "Appearance",
    run: () => {
      setTheme(t.value);
    },
  }));
  const sidebar = {
    id: "sidebar",
    label: "Collapse or expand the sidebar",
    group: "Appearance",
    keys: "[",
    run: ctx.toggleSidebar,
  };
  return [...themes, sidebar];
}

export function actions(ctx: ActionContext): Action[] {
  return [
    ...navigation(ctx),
    ...period(ctx),
    ...appearance(ctx),
    { id: "rescan", label: "Rescan sources now", group: "Data", run: ctx.rescan },
    { id: "help", label: "Keyboard shortcuts", group: "Help", keys: "?", run: ctx.openHelp },
  ];
}
