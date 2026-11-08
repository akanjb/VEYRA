import { useCommand } from "@/shared/api";
import { Card, EmptyState, ErrorState, Skeleton } from "@/shared/ui";

import { PrivacyPromise } from "./privacy";
import { SourceCard } from "./source-card";

/** Tools Veyra will read later, or cannot read at all, and why (docs/planning/00-research.md). */
const OTHER_TOOLS = [
  {
    name: "Codex CLI, Gemini CLI, OpenCode, Copilot CLI",
    status: "Planned",
    reason: "Local session logs; connectors arrive in milestone M4.",
  },
  {
    name: "Windsurf, Antigravity, Zed, ChatGPT desktop",
    status: "Unsupported",
    reason: "No usable local usage data; only enterprise analytics APIs exist.",
  },
];

export function SourcesPage() {
  const sources = useCommand("list_sources", {});
  return (
    <div className="grid max-w-5xl gap-4">
      {sources.isPending ? (
        <Skeleton className="h-36" />
      ) : sources.isError ? (
        <ErrorState error={sources.error} onRetry={() => void sources.refetch()} />
      ) : sources.data.sources.length === 0 ? (
        <Card title="No supported tools found">
          <EmptyState title="Veyra found no usage logs on this device">
            Claude Code writes to <code className="font-mono">~/.claude/projects</code>, or to
            <code className="font-mono"> $CLAUDE_CONFIG_DIR/projects</code> when that variable is
            set. Use a supported tool once, then come back.
          </EmptyState>
        </Card>
      ) : (
        sources.data.sources.map((s) => <SourceCard key={s.id} source={s} />)
      )}
      <Card title="Other tools">
        <ul className="grid gap-3 text-sm">
          {OTHER_TOOLS.map((t) => (
            <li key={t.name} className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
              <span className="font-medium">{t.name}</span>
              <span className="rounded-full border border-border px-1.5 text-[11px] text-muted">
                {t.status}
              </span>
              <span className="w-full text-muted">{t.reason}</span>
            </li>
          ))}
        </ul>
      </Card>
      <Card title="What Veyra reads">
        <PrivacyPromise />
      </Card>
    </div>
  );
}
