import type { ApiError } from "@veyra/contracts";

/** The backend executed the command and returned a typed {@link ApiError}. */
export class ApiCallError extends Error {
  override readonly name = "ApiCallError";

  constructor(
    readonly command: string,
    readonly error: ApiError,
  ) {
    super(`${command} failed: ${error.kind}`);
  }
}

/** The command never produced a typed result (IPC failure, permission denied, …). */
export class TransportError extends Error {
  override readonly name = "TransportError";

  constructor(
    readonly command: string,
    readonly reason: string,
  ) {
    super(`${command} could not be delivered: ${reason}`);
  }
}

/**
 * Narrows a rejection value to an {@link ApiError}. The Rust side is a trusted,
 * in-process boundary, so the tagged-union shape is sufficient evidence.
 */
export function isApiError(value: unknown): value is ApiError {
  return (
    typeof value === "object" && value !== null && "kind" in value && typeof value.kind === "string"
  );
}

/** Converts any rejection from a command into one of the two typed error classes. */
export function toCallError(command: string, reason: unknown): ApiCallError | TransportError {
  if (isApiError(reason)) return new ApiCallError(command, reason);
  return new TransportError(command, reason instanceof Error ? reason.message : String(reason));
}
