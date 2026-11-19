/** The local listeners (OpenTelemetry receiver and capture proxy) in the fixture client. */
import type { CommandName } from "@veyra/contracts";

import type { CommandRequest, CommandResponse } from "../api-client";

type Handlers<C extends CommandName> = {
  [K in C]: (request: CommandRequest<K>) => CommandResponse<K>;
};

/** The receiver, as if it started on the standard port; nothing arrives in a browser. */
function telemetryHandlers(): Handlers<"get_telemetry_receiver" | "set_telemetry_receiver"> {
  let enabled = false;
  const info = () => ({
    enabled,
    listening: enabled,
    endpoint: "http://127.0.0.1:4318",
    error: null,
    exports: 0,
    records: 0,
    lastReceivedAt: null,
    lastRefusal: null,
  });
  return {
    get_telemetry_receiver: info,
    set_telemetry_receiver: (r) => {
      enabled = r.enabled;
      return info();
    },
  };
}

/** The capture proxy, as if it started on its port; nothing is relayed in a browser. */
function proxyHandlers(): Handlers<"get_capture_proxy" | "set_capture_proxy"> {
  let enabled = false;
  const endpoint = "http://127.0.0.1:4320";
  const providers = [
    ["OpenAI API", "openai", "/v1", "https://api.openai.com"],
    ["Anthropic API", "anthropic", "", "https://api.anthropic.com"],
    ["Gemini API", "gemini", "", "https://generativelanguage.googleapis.com"],
    ["OpenRouter", "openrouter", "/v1", "https://openrouter.ai/api"],
  ].map(([name, prefix, suffix, upstream]) => ({
    name: name ?? "",
    baseUrl: `${endpoint}/${prefix ?? ""}${suffix ?? ""}`,
    upstream: `${upstream ?? ""}${suffix ?? ""}`,
  }));
  const info = () => ({
    enabled,
    listening: enabled,
    endpoint,
    error: null,
    relayed: 0,
    records: 0,
    lastRelayedAt: null,
    lastFailure: null,
    providers,
  });
  return {
    get_capture_proxy: info,
    set_capture_proxy: (r) => {
      enabled = r.enabled;
      return info();
    },
  };
}

/** Handlers of both listeners' commands, each with its own on/off state. */
export function listenerHandlers() {
  return { ...telemetryHandlers(), ...proxyHandlers() };
}
