import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { ApiClient, CommandResponse } from "./api-client";
import { toCallError } from "./api-errors";

/** Signature of Tauri's `invoke`, injectable for tests. */
export type InvokeFn = (command: string, args: Record<string, unknown>) => Promise<unknown>;

/** Signature of Tauri's `listen`, reduced to what the client needs; injectable for tests. */
export type ListenFn = (
  event: string,
  handler: (event: { payload: unknown }) => void,
) => Promise<() => void>;

/** Whether the page runs inside the Tauri webview (rather than a plain browser). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Creates an {@link ApiClient} that talks to the Rust core over Tauri IPC. */
export function createTauriApiClient(
  invokeFn: InvokeFn = invoke,
  listenFn: ListenFn = listen,
): ApiClient {
  return {
    async call(command, request) {
      try {
        // Every command takes a single `request` argument (see src-tauri/src/commands.rs).
        const response = await invokeFn(command, { request });
        // Trusted boundary: responses are produced by the generated Rust contract.
        return response as CommandResponse<typeof command>;
      } catch (reason) {
        throw toCallError(command, reason);
      }
    },
    subscribe(event, handler) {
      let stop: (() => void) | undefined;
      let stopped = false;
      void listenFn(event, ({ payload }) => {
        // Trusted boundary: payloads are produced by the generated Rust contract.
        handler(payload as Parameters<typeof handler>[0]);
      }).then((unlisten) => {
        if (stopped) unlisten();
        else stop = unlisten;
      });
      return () => {
        stopped = true;
        stop?.();
      };
    },
  };
}
