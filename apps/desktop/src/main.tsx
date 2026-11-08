import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "@tanstack/react-router";
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";

import { router } from "@/app/router";
import { initTheme } from "@/app/theme";
import {
  ApiClientProvider,
  createQueryClient,
  createTauriApiClient,
  isTauri,
  type ApiClient,
} from "@/shared/api";
import "@/styles/global.css";

/**
 * The core over Tauri IPC in the app. In a plain browser (development and UI tests only)
 * a synthetic fixture client stands in; it is never part of a production build.
 */
async function createClient(): Promise<ApiClient> {
  if (isTauri()) return createTauriApiClient();
  if (import.meta.env.DEV || import.meta.env.MODE === "fixtures") {
    const { createFixtureApiClient } = await import("@/shared/api/fixtures/fixture-api-client");
    return createFixtureApiClient({
      fresh: new URLSearchParams(window.location.search).has("fresh"),
    });
  }
  throw new Error("Veyra must run inside its desktop application.");
}

initTheme();
const root = document.getElementById("root");
if (root === null) throw new Error("Missing #root element");
const client = await createClient();
const queries = createQueryClient();

createRoot(root).render(
  <StrictMode>
    <ApiClientProvider client={client}>
      <QueryClientProvider client={queries}>
        <RouterProvider router={router} />
      </QueryClientProvider>
    </ApiClientProvider>
  </StrictMode>,
);
