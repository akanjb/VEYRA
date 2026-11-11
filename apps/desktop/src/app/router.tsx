/**
 * Routes. The global filters live in the root route's search params (ADR-013): they are
 * validated once here and kept when moving between pages.
 */
import {
  createRootRoute,
  createRoute,
  createRouter,
  lazyRouteComponent,
  retainSearchParams,
} from "@tanstack/react-router";

import { validateSessionsSearch } from "@/modules/sessions";
import { FILTER_KEYS, validateSearch } from "@/shared/filters";
import { EmptyState } from "@/shared/ui";

import { AppShell } from "./shell/app-shell";

const rootRoute = createRootRoute({
  validateSearch,
  search: { middlewares: [retainSearchParams([...FILTER_KEYS])] },
  component: AppShell,
  notFoundComponent: () => <EmptyState title="This page does not exist" />,
});

const overviewRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: lazyRouteComponent(() => import("@/modules/overview"), "OverviewPage"),
});

const usageRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/usage",
  component: lazyRouteComponent(() => import("@/modules/usage"), "UsagePage"),
});

const cacheRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/cache",
  component: lazyRouteComponent(() => import("@/modules/cache"), "CachePage"),
});

const modelsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/models",
  component: lazyRouteComponent(() => import("@/modules/models"), "ModelsPage"),
});

const activityRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/activity",
  component: lazyRouteComponent(() => import("@/modules/activity"), "ActivityPage"),
});

const sessionsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/sessions",
  validateSearch: validateSessionsSearch,
  component: lazyRouteComponent(() => import("@/modules/sessions"), "SessionsPage"),
});

const sessionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/sessions/$sessionId",
  component: lazyRouteComponent(() => import("@/modules/sessions"), "SessionDetailPage"),
});

const sourcesRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/sources",
  component: lazyRouteComponent(() => import("@/modules/sources"), "SourcesPage"),
});

const routeTree = rootRoute.addChildren([
  overviewRoute,
  usageRoute,
  cacheRoute,
  modelsRoute,
  activityRoute,
  sessionsRoute,
  sessionRoute,
  sourcesRoute,
]);

export const router = createRouter({ routeTree, defaultPreload: "intent" });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
