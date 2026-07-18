import { Component, lazy, Suspense, useMemo, useState, type ComponentType, type ReactNode } from "react";

interface WorkspaceErrorBoundaryProps {
  children: ReactNode;
  name: string;
  resetKey: number;
  onRetry(): void;
}

interface WorkspaceErrorBoundaryState {
  failed: boolean;
}

class WorkspaceErrorBoundary extends Component<WorkspaceErrorBoundaryProps, WorkspaceErrorBoundaryState> {
  state: WorkspaceErrorBoundaryState = { failed: false };

  static getDerivedStateFromError(): WorkspaceErrorBoundaryState {
    return { failed: true };
  }

  componentDidUpdate(previousProps: WorkspaceErrorBoundaryProps) {
    if (this.state.failed && previousProps.resetKey !== this.props.resetKey) {
      this.setState({ failed: false });
    }
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <section className="workspace-load-state workspace-load-error" role="alert">
        <span aria-hidden="true">◇</span>
        <p className="card-label">工作区加载中断</p>
        <h2>{this.props.name}暂时无法打开</h2>
        <p>桌宠与本地核心运行不受影响。请重试此工作区；无需重启整个应用。</p>
        <button className="secondary-button" type="button" onClick={this.props.onRetry}>重新加载</button>
      </section>
    );
  }
}

interface LazyWorkspaceProps<Props extends object> {
  loader(): Promise<{ default: ComponentType<Props> }>;
  name: string;
  componentProps: Props;
}

export function LazyWorkspace<Props extends object>({ loader, name, componentProps }: LazyWorkspaceProps<Props>) {
  const [attempt, setAttempt] = useState(0);
  const Workspace = useMemo(() => lazy(loader), [loader, attempt]);
  return (
    <WorkspaceErrorBoundary name={name} resetKey={attempt} onRetry={() => setAttempt((value) => value + 1)}>
      <Suspense fallback={<WorkspaceLoading name={name} />}>
        <Workspace {...componentProps} />
      </Suspense>
    </WorkspaceErrorBoundary>
  );
}

export function WorkspaceLoading({ name }: { name: string }) {
  return (
    <section className="workspace-load-state" role="status" aria-live="polite">
      <span className="workspace-load-orbit" aria-hidden="true" />
      <p className="card-label">按需唤醒</p>
      <h2>正在载入{name}</h2>
      <p>本地运行时保持在线，工作区准备完成后会自动显示。</p>
    </section>
  );
}
