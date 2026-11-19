/** The opt-in local OpenTelemetry receiver (ADR-038). */
import type { TelemetryReceiverInfo } from "@veyra/contracts";

import { useCommand, useCommandMutation } from "@/shared/api";
import { fmt } from "@/shared/format";
import { Card, ErrorState, FormError, Skeleton, Switch } from "@/shared/ui";

/** Claude Code's documented settings for sending its usage events to `endpoint`. */
function claudeCodeSetup(endpoint: string): string {
  return [
    "CLAUDE_CODE_ENABLE_TELEMETRY=1",
    "OTEL_LOGS_EXPORTER=otlp",
    "OTEL_EXPORTER_OTLP_LOGS_PROTOCOL=http/json",
    `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT=${endpoint}/v1/logs`,
  ].join("\n");
}

function statusText(info: TelemetryReceiverInfo): string {
  if (info.error !== null) return info.error;
  if (!info.listening) return "Off. No port is open.";
  const last =
    info.lastReceivedAt === null ? "nothing yet" : `last at ${fmt.dateTime(info.lastReceivedAt)}`;
  return `Listening on ${info.endpoint}: ${fmt.integer(info.exports)} exports and ${fmt.integer(info.records)} requests received since Veyra started, ${last}.`;
}

function Setup({ endpoint }: { endpoint: string }) {
  return (
    <div className="grid gap-2 text-xs text-muted">
      <p>
        Set these environment variables where Claude Code runs, then start a new Claude Code
        session:
      </p>
      <pre className="overflow-x-auto rounded-control border border-border bg-gray-3 p-3 font-mono text-text">
        {claudeCodeSetup(endpoint)}
      </pre>
      <p>
        Requests Veyra also reads from Claude Code&apos;s logs are counted once. Prompt or tool
        content Claude Code may be set to send is discarded, never stored.
      </p>
    </div>
  );
}

export function ReceiverCard() {
  const receiver = useCommand("get_telemetry_receiver", {});
  const toggle = useCommandMutation("set_telemetry_receiver");
  if (receiver.isPending) return <Skeleton className="h-28" />;
  if (receiver.isError)
    return <ErrorState error={receiver.error} onRetry={() => void receiver.refetch()} />;
  const info = receiver.data;
  return (
    <Card title="OpenTelemetry receiver">
      <div className="grid gap-3">
        <div className="flex flex-wrap items-start gap-3">
          <p className="min-w-0 flex-1 text-sm text-muted">
            Claude Code can send each request&apos;s usage to Veyra as it happens, through its
            official OpenTelemetry export. Veyra listens on this computer only.
          </p>
          <label className="flex items-center gap-2 text-sm">
            <Switch
              checked={info.enabled}
              disabled={toggle.isPending}
              label="Receive OpenTelemetry usage"
              onCheckedChange={(enabled) => {
                toggle.mutate({ enabled });
              }}
            />
            {info.enabled ? "Enabled" : "Off"}
          </label>
        </div>
        <p
          role="status"
          className={info.error === null ? "text-xs text-muted" : "text-xs text-danger"}
          data-numeric
        >
          {statusText(info)}
        </p>
        {info.listening && info.lastRefusal !== null ? (
          <p className="text-xs text-warning">
            The latest export was refused: {info.lastRefusal}. Check the settings below.
          </p>
        ) : null}
        <FormError error={toggle.error} />
        {info.listening ? <Setup endpoint={info.endpoint} /> : null}
      </div>
    </Card>
  );
}
