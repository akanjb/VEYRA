import { createContext, use, type ReactNode } from "react";

import type { ApiClient } from "./api-client";

const ApiClientContext = createContext<ApiClient | null>(null);

interface ApiClientProviderProps {
  client: ApiClient;
  children: ReactNode;
}

export function ApiClientProvider({ client, children }: ApiClientProviderProps) {
  return <ApiClientContext value={client}>{children}</ApiClientContext>;
}

/** Returns the {@link ApiClient} provided by the nearest {@link ApiClientProvider}. */
export function useApiClient(): ApiClient {
  const client = use(ApiClientContext);
  if (client === null) throw new Error("useApiClient must be used inside <ApiClientProvider>.");
  return client;
}
