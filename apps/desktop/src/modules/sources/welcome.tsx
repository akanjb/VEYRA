import type { SourceSummary } from "@veyra/contracts";
import { ShieldCheck } from "lucide-react";

import { Button } from "@/shared/ui";

import { PrivacyPromise } from "./privacy";
import { SourceCard } from "./source-card";

/**
 * First run (UC-01): the privacy promise, then the detected sources to switch on. Nothing
 * under a source folder is opened until its switch is on.
 */
export function Welcome({
  sources,
  onSkip,
}: {
  sources: readonly SourceSummary[];
  onSkip: () => void;
}) {
  return (
    <div className="mx-auto grid max-w-3xl gap-6 py-6">
      <div className="grid gap-2">
        <ShieldCheck size={28} className="text-accent" aria-hidden />
        <h1 className="text-2xl font-semibold tracking-tight">Welcome to Veyra</h1>
        <p className="text-md text-muted">
          One private view of all your AI usage. Everything stays on this computer: no account, no
          telemetry, no servers. Choose which tools Veyra may read; you can change this at any time
          under Sources &amp; Health.
        </p>
      </div>
      <PrivacyPromise />
      <div className="grid gap-3">
        <h2 className="text-sm font-medium">Detected on this computer</h2>
        {sources.length === 0 ? (
          <p className="text-sm text-muted">
            No supported tool has written usage logs yet. Use Claude Code once, then rescan from
            Sources &amp; Health.
          </p>
        ) : (
          sources.map((s) => <SourceCard key={s.id} source={s} />)
        )}
      </div>
      <div>
        <Button variant="ghost" onClick={onSkip}>
          Not now
        </Button>
      </div>
    </div>
  );
}
