/**
 * The pages of the dashboard: the single source for the sidebar, page titles, the
 * command palette and the "g" shortcuts (docs/planning/03-ux.md §2.1).
 */
import {
  Activity,
  CalendarClock,
  ChartColumnStacked,
  Cpu,
  DatabaseZap,
  Database,
  FolderGit2,
  LayoutDashboard,
  type LucideIcon,
} from "lucide-react";

export interface Page {
  to: "/" | "/usage" | "/cache" | "/models" | "/activity" | "/projects" | "/sessions" | "/sources";
  title: string;
  /** The question the page answers. */
  purpose: string;
  section: string;
  icon: LucideIcon;
  /** Second key of the "g" shortcut. */
  shortcut: string;
  /** Whether the global query bar applies to this page. */
  scoped: boolean;
}

/** The home page, and the page of any unknown path. */
const OVERVIEW: Page = {
  to: "/",
  title: "Overview",
  purpose: "How am I using AI overall?",
  section: "Veyra",
  icon: LayoutDashboard,
  shortcut: "o",
  scoped: true,
};

export const PAGES: readonly Page[] = [
  OVERVIEW,
  {
    to: "/usage",
    title: "Usage",
    purpose: "What is consumed, and where?",
    section: "Analyze",
    icon: ChartColumnStacked,
    shortcut: "u",
    scoped: true,
  },
  {
    to: "/cache",
    title: "Cache",
    purpose: "Is caching working, and what does it save?",
    section: "Analyze",
    icon: DatabaseZap,
    shortcut: "c",
    scoped: true,
  },
  {
    to: "/models",
    title: "Models & Providers",
    purpose: "Which models do I use and how do they compare?",
    section: "Analyze",
    icon: Cpu,
    shortcut: "m",
    scoped: true,
  },
  {
    to: "/activity",
    title: "Activity",
    purpose: "When do I work with AI?",
    section: "Analyze",
    icon: CalendarClock,
    shortcut: "a",
    scoped: true,
  },
  {
    to: "/projects",
    title: "Projects",
    purpose: "Which projects drive usage, and what happened in them?",
    section: "Explore",
    icon: FolderGit2,
    shortcut: "p",
    scoped: true,
  },
  {
    to: "/sessions",
    title: "Sessions",
    purpose: "Which sessions drove usage, and what happened in them?",
    section: "Explore",
    icon: Activity,
    shortcut: "s",
    scoped: true,
  },
  {
    to: "/sources",
    title: "Sources & Health",
    purpose: "What Veyra reads, how fresh it is, and what went wrong.",
    section: "System",
    icon: Database,
    shortcut: "h",
    scoped: false,
  },
];

/** The page a pathname belongs to (detail pages belong to their list). */
export function pageFor(pathname: string): Page {
  return [...PAGES].reverse().find((p) => p.to !== "/" && pathname.startsWith(p.to)) ?? OVERVIEW;
}
