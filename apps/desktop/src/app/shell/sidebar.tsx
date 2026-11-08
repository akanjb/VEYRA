import { Link } from "@tanstack/react-router";
import { Monitor, Moon, PanelLeftClose, PanelLeftOpen, Sun } from "lucide-react";

import { Segmented, Tip, cn } from "@/shared/ui";

import { PAGES } from "../navigation";
import { useTheme, type ThemePreference } from "../theme";

const THEMES: readonly { value: ThemePreference; label: string }[] = [
  { value: "light", label: "Light" },
  { value: "system", label: "Auto" },
  { value: "dark", label: "Dark" },
];

const THEME_ICON = { light: Sun, system: Monitor, dark: Moon } as const;

function Brand({ collapsed }: { collapsed: boolean }) {
  return (
    <div className="flex h-14 items-center gap-2 px-4">
      <span
        aria-hidden
        className="grid size-7 place-items-center rounded-control bg-accent text-sm font-bold text-on-accent"
      >
        V
      </span>
      {collapsed ? null : <span className="text-md font-semibold tracking-tight">Veyra</span>}
    </div>
  );
}

function NavList({ collapsed }: { collapsed: boolean }) {
  const sections = [...new Set(PAGES.map((p) => p.section))];
  return (
    <nav aria-label="Pages" className="grid gap-4 px-2">
      {sections.map((section) => (
        <div key={section} className="grid gap-0.5">
          {collapsed ? null : (
            <p className="px-2 pb-1 text-[11px] font-medium tracking-wide text-subtle uppercase">
              {section}
            </p>
          )}
          {PAGES.filter((p) => p.section === section).map((page) => {
            const Icon = page.icon;
            const link = (
              <Link
                to={page.to}
                search={(previous: Record<string, unknown>) => previous}
                activeOptions={{ exact: page.to === "/", includeSearch: false }}
                className="flex h-8 items-center gap-2.5 rounded-control px-2 text-sm text-muted hover:bg-gray-3 hover:text-text data-[status=active]:bg-accent-soft data-[status=active]:text-text"
              >
                <Icon size={16} aria-hidden />
                {collapsed ? <span className="sr-only">{page.title}</span> : page.title}
              </Link>
            );
            return collapsed ? (
              <Tip key={page.to} content={page.title}>
                {link}
              </Tip>
            ) : (
              <div key={page.to}>{link}</div>
            );
          })}
        </div>
      ))}
    </nav>
  );
}

/** Grouped navigation; collapses to an icon rail (shortcut "["). */
export function Sidebar({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  const [theme, setTheme] = useTheme();
  const ThemeIcon = THEME_ICON[theme];
  return (
    <aside
      className={cn(
        "flex shrink-0 flex-col border-r border-border bg-surface transition-[width] duration-200",
        collapsed ? "w-14" : "w-56",
      )}
    >
      <Brand collapsed={collapsed} />
      <div className="flex-1 overflow-y-auto py-2">
        <NavList collapsed={collapsed} />
      </div>
      <div className="grid gap-2 border-t border-border p-2">
        {collapsed ? (
          <Tip content={`Theme: ${theme}`}>
            <button
              type="button"
              aria-label="Change theme"
              onClick={() => {
                setTheme(theme === "dark" ? "light" : "dark");
              }}
              className="grid h-8 place-items-center rounded-control text-muted hover:bg-gray-3"
            >
              <ThemeIcon size={16} aria-hidden />
            </button>
          </Tip>
        ) : (
          <Segmented label="Theme" value={theme} options={THEMES} onChange={setTheme} />
        )}
        <button
          type="button"
          onClick={onToggle}
          aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
          className="flex h-8 items-center gap-2 rounded-control px-2 text-xs text-subtle hover:bg-gray-3 hover:text-text"
        >
          {collapsed ? (
            <PanelLeftOpen size={16} aria-hidden />
          ) : (
            <PanelLeftClose size={16} aria-hidden />
          )}
          {collapsed ? null : "Collapse"}
        </button>
      </div>
    </aside>
  );
}
