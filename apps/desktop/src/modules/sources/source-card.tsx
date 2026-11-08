import type { SourceSummary } from "@veyra/contracts";
import { CircleAlert, FolderOpen, RefreshCw } from "lucide-react";

import { useCommandMutation } from "@/shared/api";
import { fmt } from "@/shared/format";
import { Button, Stat, Switch, Tip } from "@/shared/ui";

import { HealthDot, health } from "./health";
import { issueLabel } from "./issues";

function Heading({ source }: { source: SourceSummary }) {
  const state = health(source);
  return (
    <div className="min-w-0 flex-1">
      <div className="flex items-center gap-2">
        <h2 className="text-md font-semibold">{source.displayName}</h2>
        <span className="inline-flex items-center gap-1.5 text-xs text-muted">
          <HealthDot tone={state.tone} />
          {state.label}
        </span>
      </div>
      <p className="mt-1 flex items-center gap-1.5 font-mono text-xs break-all text-muted">
        <FolderOpen size={14} className="shrink-0" aria-hidden />
        {source.root}
      </p>
      {source.documentedFormat ? null : (
        <p className="mt-1 text-xs text-subtle">
          The vendor does not document this format; Veyra validates every update against its
          fixtures.
        </p>
      )}
    </div>
  );
}

/** Consent (the switch) and a manual rescan. */
function Controls({ source }: { source: SourceSummary }) {
  const toggle = useCommandMutation("set_source_enabled");
  const rescan = useCommandMutation("request_rescan");
  return (
    <div className="flex items-center gap-3">
      <Button
        size="sm"
        variant="ghost"
        disabled={!source.enabled || rescan.isPending}
        onClick={() => {
          rescan.mutate({});
        }}
      >
        <RefreshCw size={14} aria-hidden /> Rescan
      </Button>
      <label className="flex items-center gap-2 text-sm">
        <Switch
          checked={source.enabled}
          disabled={toggle.isPending}
          label={`Read ${source.displayName} usage`}
          onCheckedChange={(enabled) => {
            toggle.mutate({ sourceId: source.id, enabled });
          }}
        />
        {source.enabled ? "Enabled" : "Off"}
      </label>
    </div>
  );
}

function Figures({ source }: { source: SourceSummary }) {
  const removed = source.filesMissing > 0 ? ` (+${fmt.integer(source.filesMissing)} removed)` : "";
  return (
    <dl className="mt-4 grid grid-cols-2 gap-4 border-t border-border pt-4 sm:grid-cols-4">
      <Stat label="Requests stored" value={fmt.integer(source.events)} />
      <Stat label="Log files" value={`${fmt.integer(source.filesActive)}${removed}`} />
      <Stat
        label="Last read"
        value={source.lastIngestedAt === null ? "—" : fmt.ago(source.lastIngestedAt)}
      />
      <Stat
        label="Consent given"
        value={source.consentedAt === null ? "—" : fmt.dateTime(source.consentedAt)}
      />
    </dl>
  );
}

/** Scan failure and the problems found recently, counted per kind (never content). */
function Problems({ source }: { source: SourceSummary }) {
  const total = source.recentIssues.reduce((n, i) => n + i.occurrences, 0);
  return (
    <>
      {source.lastError === null ? null : (
        <p role="alert" className="mt-3 flex gap-2 text-sm text-warning">
          <CircleAlert size={16} className="mt-0.5 shrink-0" aria-hidden />
          {source.lastError}
        </p>
      )}
      {total === 0 ? null : (
        <details className="mt-3 text-sm">
          <summary className="cursor-pointer text-muted">
            {fmt.integer(total)} problem{total === 1 ? "" : "s"} found in the last 7 days
          </summary>
          <ul className="mt-2 grid gap-1 text-xs text-muted">
            {source.recentIssues.map((i) => (
              <li key={i.kind}>
                <Tip
                  content={`${i.kind}: counted per kind; Veyra never stores the offending content.`}
                >
                  <span tabIndex={0}>{issueLabel(i.kind)}</span>
                </Tip>{" "}
                <span data-numeric>× {fmt.integer(i.occurrences)}</span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </>
  );
}

/** A connector's consent toggle, folder, freshness and diagnostics. */
export function SourceCard({ source }: { source: SourceSummary }) {
  return (
    <section
      aria-label={source.displayName}
      className="rounded-card border border-border bg-surface p-5"
    >
      <div className="flex flex-wrap items-start gap-3">
        <Heading source={source} />
        <Controls source={source} />
      </div>
      {source.enabled ? <Figures source={source} /> : null}
      <Problems source={source} />
    </section>
  );
}
