import { useCommand } from "@/shared/api";
import { Card, EmptyState, ErrorState, Skeleton } from "@/shared/ui";

import { PrivacyPromise } from "./privacy";
import { ReceiverCard } from "./receiver-card";
import { SourceCard } from "./source-card";

/** Tools Veyra cannot read, and why (docs/planning/00-research.md). */
const UNSUPPORTED = {
  name: "Windsurf, Antigravity, Zed, ChatGPT desktop",
  status: "Unsupported",
  reason: "No usable local usage data; only enterprise analytics APIs exist.",
};

/** Supported tools without logs on this device, then tools Veyra cannot read. */
function otherTools(notFound: readonly string[]) {
  const missing = {
    name: notFound.join(", "),
    status: "Not found",
    reason:
      "Supported, but no usage logs were found on this device. Use the tool once, then rescan.",
  };
  return notFound.length === 0 ? [UNSUPPORTED] : [missing, UNSUPPORTED];
}

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
            Veyra reads {sources.data.notFound.join(", ")}. Use one of them once, then come back.
          </EmptyState>
        </Card>
      ) : (
        sources.data.sources.map((s) => <SourceCard key={s.id} source={s} />)
      )}
      <ReceiverCard />
      <Card title="Other tools">
        <ul className="grid gap-3 text-sm">
          {otherTools(sources.data?.notFound ?? []).map((t) => (
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
