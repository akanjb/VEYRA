import { useCallback } from "react";
import { useNavigate } from "@tanstack/react-router";

/** Opens a project's detail page, keeping the global filters. */
export function useOpenProject(): (id: number) => void {
  const navigate = useNavigate();
  return useCallback(
    (id: number) => {
      void navigate({ to: "/projects/$projectId", params: { projectId: String(id) } } as never);
    },
    [navigate],
  );
}
