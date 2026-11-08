import { Component, type ErrorInfo, type ReactNode } from "react";

import { ErrorState } from "@/shared/ui";

interface State {
  error: Error | null;
}

/** Contains a rendering failure to the page that caused it; the shell stays usable. */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error("Veyra page failed to render", error, info.componentStack);
  }

  override render(): ReactNode {
    if (this.state.error !== null) {
      return (
        <ErrorState
          error={this.state.error}
          onRetry={() => {
            this.setState({ error: null });
          }}
        />
      );
    }
    return this.props.children;
  }
}
