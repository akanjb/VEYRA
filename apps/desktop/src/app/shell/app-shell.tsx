import { useCallback, useMemo, useState } from "react";
import { Outlet, useLocation, useNavigate } from "@tanstack/react-router";

import { Welcome } from "@/modules/sources";
import { useCommand, useCommandMutation, useLiveRefresh } from "@/shared/api";
import { useFilters, type Filters } from "@/shared/filters";
import { TipProvider } from "@/shared/ui";

import { ErrorBoundary } from "../error-boundary";
import { pageFor } from "../navigation";
import { SCOPED_ACTION_PREFIX, actions, type Action } from "./actions";
import { CommandPalette, ShortcutHelp } from "./command-palette";
import { Header } from "./header";
import { QueryBar } from "./query-bar";
import { Sidebar } from "./sidebar";
import { useShortcuts } from "./use-shortcuts";

/** Below this width the sidebar starts as an icon rail (§2.2). */
const RAIL_WIDTH = 1200;

/** Open/closed state of the shell's panels. */
function usePanels() {
  const [collapsed, setCollapsed] = useState(() => window.innerWidth < RAIL_WIDTH);
  const [palette, setPalette] = useState(false);
  const [help, setHelp] = useState(false);
  const toggleSidebar = useCallback(() => {
    setCollapsed((c) => !c);
  }, []);
  const openPalette = useCallback(() => {
    setPalette(true);
  }, []);
  return { collapsed, toggleSidebar, palette, setPalette, openPalette, help, setHelp };
}

/** The action list for the palette and shortcuts. */
function useActions(filters: Filters, panels: ReturnType<typeof usePanels>): Action[] {
  const navigate = useNavigate();
  const rescan = useCommandMutation("request_rescan");
  const { toggleSidebar, setHelp } = panels;
  return useMemo(
    () =>
      actions({
        filters,
        navigate: (to) => {
          void navigate({ to, search: (previous: Record<string, unknown>) => previous } as never);
        },
        toggleSidebar,
        openHelp: () => {
          setHelp(true);
        },
        rescan: () => {
          rescan.mutate({});
        },
      }),
    [filters, navigate, rescan, toggleSidebar, setHelp],
  );
}

/** The product frame: sidebar, header, query bar, page, palette and shortcuts. */
export function AppShell() {
  const lastChange = useLiveRefresh();
  const sources = useCommand("list_sources", {}).data?.sources;
  const filters = useFilters();
  const { pathname } = useLocation();
  const page = pageFor(pathname);
  const panels = usePanels();
  const list = useActions(filters, panels);
  useShortcuts(
    page.scoped ? list : list.filter((a) => !a.id.startsWith(SCOPED_ACTION_PREFIX)),
    panels.openPalette,
  );
  const [skipWelcome, setSkipWelcome] = useState(false);
  const firstRun =
    sources !== undefined &&
    !sources.some((s) => s.enabled) &&
    !skipWelcome &&
    page.to !== "/sources";
  return (
    <TipProvider delayDuration={250}>
      <a
        href="#main"
        className="sr-only focus:not-sr-only focus:absolute focus:top-2 focus:left-2 focus:z-50 focus:rounded-control focus:bg-raised focus:px-3 focus:py-2"
      >
        Skip to content
      </a>
      <div className="flex h-screen overflow-hidden">
        <Sidebar collapsed={panels.collapsed} onToggle={panels.toggleSidebar} />
        <div className="flex min-w-0 flex-1 flex-col">
          <Header
            page={page}
            sources={sources ?? []}
            lastChange={lastChange}
            onOpenPalette={panels.openPalette}
          />
          {page.scoped && !firstRun ? <QueryBar filters={filters} /> : null}
          <main id="main" tabIndex={-1} className="min-h-0 flex-1 overflow-auto px-6 py-5">
            <ErrorBoundary key={pathname}>
              {firstRun ? (
                <Welcome
                  sources={sources}
                  onSkip={() => {
                    setSkipWelcome(true);
                  }}
                />
              ) : (
                <Outlet />
              )}
            </ErrorBoundary>
          </main>
        </div>
      </div>
      <CommandPalette open={panels.palette} onOpenChange={panels.setPalette} actions={list} />
      <ShortcutHelp open={panels.help} onOpenChange={panels.setHelp} actions={list} />
    </TipProvider>
  );
}
