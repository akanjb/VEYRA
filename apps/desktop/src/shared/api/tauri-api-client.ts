import { invoke } from "@tauri-apps/api/core";

import type { ApiClient, CommandResponse } from "./api-client";
import { toCallError } from "./api-errors";

/** Signature of Tauri's `invoke`, injectable for tests. */
export type InvokeFn = (command: string, args: Record<string, unknown>) => Promise<unknown>;

/** Creates an {@link ApiClient} that talks to the Rust core over Tauri IPC. */
export function createTauriApiClient(invokeFn: InvokeFn = invoke): ApiClient {
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
  };
}
