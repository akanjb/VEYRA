import type { CommandName } from "@veyra/contracts";
import {
  QueryClient,
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useEffect, useState } from "react";

import type { CommandRequest, CommandResponse } from "./api-client";
import { useApiClient } from "./api-context";
import { ApiCallError } from "./api-errors";

/** While the database starts, commands answer "busy": retry with backoff, briefly. */
const MAX_BUSY_RETRIES = 10;

function retryWhileBusy(failureCount: number, error: Error): boolean {
  return (
    error instanceof ApiCallError && error.error.kind === "busy" && failureCount < MAX_BUSY_RETRIES
  );
}

/** The dashboard's query cache. Data refreshes on `data-changed`, not on a timer. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: 60_000,
        refetchOnWindowFocus: false,
        retry: retryWhileBusy,
        retryDelay: (attempt) => Math.min(250 * 2 ** attempt, 2_000),
      },
    },
  });
}

/** The cache key of a command call. */
export function commandKey<C extends CommandName>(command: C, request: CommandRequest<C>) {
  return [command, request] as const;
}

/**
 * Runs a command as a cached query. Previous results stay visible while a new request
 * (for example, after a filter change) loads, so cards never flash empty.
 */
export function useCommand<C extends CommandName>(
  command: C,
  request: CommandRequest<C>,
  options: { enabled?: boolean } = {},
) {
  const client = useApiClient();
  return useQuery<CommandResponse<C>>({
    queryKey: commandKey(command, request),
    queryFn: () => client.call(command, request),
    placeholderData: keepPreviousData,
    enabled: options.enabled ?? true,
  });
}

/** Runs a command that changes state, then refreshes every cached query. */
export function useCommandMutation<C extends CommandName>(command: C) {
  const client = useApiClient();
  const queries = useQueryClient();
  return useMutation<CommandResponse<C>, Error, CommandRequest<C>>({
    mutationFn: (request) => client.call(command, request),
    onSuccess: () => queries.invalidateQueries(),
  });
}

/**
 * Refreshes every query when the core reports that stored usage changed. Returns when
 * that last happened (epoch ms), or `null` before the first change.
 */
export function useLiveRefresh(): number | null {
  const client = useApiClient();
  const queries = useQueryClient();
  const [lastChange, setLastChange] = useState<number | null>(null);
  useEffect(
    () =>
      client.subscribe("data-changed", () => {
        setLastChange(Date.now());
        void queries.invalidateQueries();
      }),
    [client, queries],
  );
  return lastChange;
}
