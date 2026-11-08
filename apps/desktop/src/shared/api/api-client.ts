import type { CommandMap, CommandName, EventMap, EventName } from "@veyra/contracts";

export type CommandRequest<C extends CommandName> = CommandMap[C]["request"];
export type CommandResponse<C extends CommandName> = CommandMap[C]["response"];

/** Stops a subscription. */
export type Unsubscribe = () => void;

/**
 * Typed gateway to the application layer. Modules depend on this interface only, never
 * on a concrete transport (Tauri IPC today; HTTP for localhost/Hub access later).
 */
export interface ApiClient {
  call<C extends CommandName>(command: C, request: CommandRequest<C>): Promise<CommandResponse<C>>;
  /** Receives events the core emits (see `for_each_event!`). */
  subscribe<E extends EventName>(event: E, handler: (payload: EventMap[E]) => void): Unsubscribe;
}
