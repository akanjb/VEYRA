import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@/app/App";
import { ApiClientProvider, createTauriApiClient } from "@/shared/api";
import "@/styles/global.css";

const root = document.getElementById("root");
if (root === null) throw new Error("Missing #root element");

createRoot(root).render(
  <StrictMode>
    <ApiClientProvider client={createTauriApiClient()}>
      <App />
    </ApiClientProvider>
  </StrictMode>,
);
