import type { AppInfo } from "@veyra/contracts";
import { useEffect, useState } from "react";

import { useApiClient } from "@/shared/api";

export type AppInfoState =
  { status: "loading" } | { status: "ready"; info: AppInfo } | { status: "error"; message: string };

/** Loads basic application facts once. Replaced by TanStack Query in M3. */
export function useAppInfo(): AppInfoState {
  const client = useApiClient();
  const [state, setState] = useState<AppInfoState>({ status: "loading" });

  useEffect(() => {
    let active = true;
    client.call("get_app_info", {}).then(
      (info) => {
        if (active) setState({ status: "ready", info });
      },
      (error: unknown) => {
        if (active) setState({ status: "error", message: String(error) });
      },
    );
    return () => {
      active = false;
    };
  }, [client]);

  return state;
}
