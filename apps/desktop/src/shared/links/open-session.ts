import { useCallback } from "react";
import { useNavigate } from "@tanstack/react-router";

/** Opens a session's detail page, keeping the global filters. */
export function useOpenSession(): (id: number) => void {
  const navigate = useNavigate();
  return useCallback(
    (id: number) => {
      void navigate({ to: "/sessions/$sessionId", params: { sessionId: String(id) } } as never);
    },
    [navigate],
  );
}
