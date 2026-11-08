import type { CommandMap, CommandName } from "@veyra/contracts";

export type CommandRequest<C extends CommandName> = CommandMap[C]["request"];
export type CommandResponse<C extends CommandName> = CommandMap[C]["response"];

/**
 * Typed gateway to the application layer. Modules depend on this interface only, never
 * on a concrete transport (Tauri IPC today; HTTP for localhost/Hub access later).
 */
export interface ApiClient {
  call<C extends CommandName>(command: C, request: CommandRequest<C>): Promise<CommandResponse<C>>;
}
