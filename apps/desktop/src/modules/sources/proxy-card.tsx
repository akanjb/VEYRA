/** The opt-in local capture proxy for API usage (ADR-039). */
import type { CaptureProxyInfo } from "@veyra/contracts";

import { useCommand, useCommandMutation } from "@/shared/api";
import { fmt } from "@/shared/format";
import { Card, ErrorState, FormError, Skeleton, Switch } from "@/shared/ui";

function statusText(info: CaptureProxyInfo): string {
  if (info.error !== null) return info.error;
  if (!info.listening) return "Off. No port is open.";
  const last =
    info.lastRelayedAt === null ? "nothing yet" : `last at ${fmt.dateTime(info.lastRelayedAt)}`;
  return `Listening on ${info.endpoint}: ${fmt.integer(info.relayed)} requests relayed and ${fmt.integer(info.records)} usage records stored since Veyra started, ${last}.`;
}

function Providers({ info }: { info: CaptureProxyInfo }) {
  return (
    <div className="grid gap-2 text-xs text-muted">
      <p>Give a client the base URL of its provider instead of the official one:</p>
      <ul className="grid gap-1.5">
        {info.providers.map((provider) => (
          <li key={provider.name} className="grid gap-0.5">
            <span className="text-text">{provider.name}</span>
            <code className="overflow-x-auto rounded-control border border-border bg-gray-3 px-2 py-1 font-mono text-text">
              {provider.baseUrl}
            </code>
            <span>Forwarded to {provider.upstream}</span>
          </li>
        ))}
      </ul>
      <ul className="grid list-disc gap-1 pl-4">
        <li>
          API keys pass through to the provider and are never stored. Prompts and replies are never
          stored.
        </li>
        <li>
          Streamed OpenAI Chat Completions report tokens only when the request sets
          stream_options.include_usage; without it the request is counted with tokens marked
          Partial.
        </li>
        <li>
          Claude Code requests are counted once even when its logs are also read. Other tools
          already read from their logs, such as Codex or Gemini CLI, would be counted twice.
        </li>
      </ul>
    </div>
  );
}

export function ProxyCard() {
  const proxy = useCommand("get_capture_proxy", {});
  const toggle = useCommandMutation("set_capture_proxy");
  if (proxy.isPending) return <Skeleton className="h-28" />;
  if (proxy.isError) return <ErrorState error={proxy.error} onRetry={() => void proxy.refetch()} />;
  const info = proxy.data;
  return (
    <Card title="API capture proxy">
      <div className="grid gap-3">
        <div className="flex flex-wrap items-start gap-3">
          <p className="min-w-0 flex-1 text-sm text-muted">
            Apps and scripts that call the OpenAI, Anthropic, Gemini or OpenRouter APIs can send
            their requests through Veyra, which forwards them unchanged and records the usage each
            response reports. Veyra listens on this computer only.
          </p>
          <label className="flex items-center gap-2 text-sm">
            <Switch
              checked={info.enabled}
              disabled={toggle.isPending}
              label="Relay API requests"
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
        {info.listening && info.lastFailure !== null ? (
          <p className="text-xs text-warning">The latest request failed: {info.lastFailure}.</p>
        ) : null}
        <FormError error={toggle.error} />
        {info.listening ? <Providers info={info} /> : null}
      </div>
    </Card>
  );
}
