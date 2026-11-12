import { useEffect, useState } from "react";
import { Link } from "@tanstack/react-router";
import type { SourceSummary } from "@veyra/contracts";
import { Command, Download } from "lucide-react";

import { HealthDot, overallHealth } from "@/modules/sources";
import { ExportDialog, defaultPreset } from "@/shared/export";
import { fmt } from "@/shared/format";
import { Button, Kbd, Tip } from "@/shared/ui";

import type { Page } from "../navigation";

/** Re-renders periodically so relative times ("12s ago") stay current. */
function useNow(intervalMs: number): number {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = setInterval(() => {
      setNow(Date.now());
    }, intervalMs);
    return () => {
      clearInterval(timer);
    };
  }, [intervalMs]);
  return now;
}

function Status({
  sources,
  lastChange,
}: {
  sources: readonly SourceSummary[];
  lastChange: number | null;
}) {
  const now = useNow(15_000);
  const enabled = sources.filter((s) => s.enabled);
  const status = overallHealth(sources);
  const updated = Math.max(lastChange ?? 0, ...enabled.map((s) => s.lastSuccessAt ?? 0));
  return (
    <div className="flex items-center gap-4 text-xs text-muted">
      {updated > 0 ? <span data-numeric>Updated {fmt.ago(updated, now)}</span> : null}
      <Tip content="Open Sources & Health">
        <Link to="/sources" className="inline-flex items-center gap-1.5 hover:text-text">
          <HealthDot tone={status.tone} />
          {status.label}
        </Link>
      </Tip>
    </div>
  );
}

/** Page title and purpose, with the status cluster and command palette (§2.2). */
export function Header({
  page,
  sources,
  lastChange,
  onOpenPalette,
}: {
  page: Page;
  sources: readonly SourceSummary[];
  lastChange: number | null;
  onOpenPalette: () => void;
}) {
  const [exporting, setExporting] = useState(false);
  return (
    <header className="flex h-14 shrink-0 items-center gap-4 border-b border-border bg-surface px-6">
      <div className="min-w-0">
        <h1 className="truncate text-md font-semibold">{page.title}</h1>
        <p className="truncate text-xs text-subtle">{page.purpose}</p>
      </div>
      <div className="ml-auto flex items-center gap-4">
        <Status sources={sources} lastChange={lastChange} />
        {page.scoped ? (
          <Button
            size="sm"
            variant="secondary"
            onClick={() => {
              setExporting(true);
            }}
            aria-label="Export report"
          >
            <Download size={14} aria-hidden />
            <span className="hidden md:inline">Export</span>
          </Button>
        ) : null}
        <Button
          size="sm"
          variant="secondary"
          onClick={onOpenPalette}
          // Contains the visible text, so speech users can say what they see (WCAG 2.5.3).
          aria-label="Open command palette with Ctrl K"
        >
          <Command size={14} aria-hidden />
          {/* The shortcut label shows on wider screens; narrow headers keep room for the title. */}
          <span className="hidden md:inline">
            <Kbd>Ctrl K</Kbd>
          </span>
        </Button>
      </div>
      {exporting ? (
        <ExportDialog
          initialPreset={defaultPreset(page.to)}
          onClose={() => {
            setExporting(false);
          }}
        />
      ) : null}
    </header>
  );
}
