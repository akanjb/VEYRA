import { useAppInfo, type AppInfoState } from "./use-app-info";

function statusLine(state: AppInfoState): string {
  switch (state.status) {
    case "loading":
      return "Connecting to the Veyra core…";
    case "error":
      return `Could not reach the Veyra core: ${state.message}`;
    case "ready": {
      const { productName, version, os, arch, buildProfile } = state.info;
      return `${productName} ${version} · ${os} ${arch} · ${buildProfile} build`;
    }
  }
}

/** Foundations placeholder. The dashboard shell arrives in milestone M3. */
export function App() {
  const state = useAppInfo();
  return (
    <main className="placeholder">
      <h1>Veyra</h1>
      <p>Local-first AI usage analytics. The dashboard arrives in milestone M3.</p>
      <p className="status" data-status={state.status} role="status">
        {statusLine(state)}
      </p>
    </main>
  );
}
