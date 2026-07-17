import { Component, type ErrorInfo, type ReactNode } from "react";

interface RendererErrorBoundaryProps {
  children: ReactNode;
  resetKey: string;
  onFailure(): void;
}

interface RendererErrorBoundaryState {
  failed: boolean;
}

export class RendererErrorBoundary extends Component<RendererErrorBoundaryProps, RendererErrorBoundaryState> {
  state: RendererErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): RendererErrorBoundaryState {
    return { failed: true };
  }

  componentDidCatch(_error: Error, _info: ErrorInfo): void {
    this.props.onFailure();
  }

  componentDidUpdate(previous: RendererErrorBoundaryProps): void {
    if (previous.resetKey !== this.props.resetKey && this.state.failed) {
      this.setState({ failed: false });
    }
  }

  render(): ReactNode {
    return this.state.failed ? null : this.props.children;
  }
}
