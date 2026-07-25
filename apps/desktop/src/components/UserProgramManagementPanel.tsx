import { useCallback, useEffect, useState } from "react";
import {
  desktopApi,
  type UserProgramCatalogEntry,
  type UserProgramExecutionReceipt,
} from "../platform/desktop";

const capabilityLabels: Record<string, string> = {
  "read-pet-state": "读取宠物状态",
  "read-profile-state": "读取档案状态",
  "subscribe-events": "订阅事件",
  "invoke-safe-commands": "调用安全命令",
  "store-local-data": "本地存储",
  "invoke-agent-tasks": "调用 Agent 任务",
};

export function describeProgramStatus(entry: UserProgramCatalogEntry): string {
  if (!entry.permissionGranted) return "待授权";
  return "已授权";
}

export function summarizeProgramReceipt(
  programId: string,
  receipt: UserProgramExecutionReceipt,
): string {
  return `${programId} 执行完成 · ${receipt.responses.length} 条能力响应`;
}

export function formatMemoryBudget(bytes: number): string {
  if (bytes <= 0) return "无内存预算";
  const mib = bytes / (1024 * 1024);
  if (mib >= 1) return `${mib.toFixed(mib >= 10 ? 0 : 1)} MiB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KiB`;
}

export function UserProgramManagementPanel({ disabled }: { disabled: boolean }) {
  const [programs, setPrograms] = useState<UserProgramCatalogEntry[]>([]);
  const [rejected, setRejected] = useState(0);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const catalog = await desktopApi.userProgramCatalog();
      setPrograms(catalog.programs);
      setRejected(catalog.rejected);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "用户程序目录暂时不可用");
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const runGuarded = useCallback(
    async (id: string, action: () => Promise<string | null>) => {
      setBusyId(id);
      setError(null);
      setNotice(null);
      try {
        const message = await action();
        if (message) setNotice(message);
        await refresh();
      } catch (reason) {
        setError(reason instanceof Error ? reason.message : "操作失败");
      } finally {
        setBusyId(null);
      }
    },
    [refresh],
  );

  if (loaded && programs.length === 0 && !error) {
    return (
      <section className="skill-lifecycle-panel">
        <header className="skill-lifecycle-head">
          <div><small>USER PROGRAM</small><h3>已安装的用户程序</h3></div>
          <button className="skill-refresh" disabled={disabled} onClick={() => void refresh()} type="button">刷新</button>
        </header>
        <div className="skill-empty-state">
          <span>✦</span>
          <p>还没有安装任何用户程序。在上方用 AI Creator 生成并安装一个程序包后，它会出现在这里等待授权与执行。</p>
        </div>
        {rejected > 0 ? <p className="skill-card-hint">{rejected} 个损坏的程序条目已被跳过。</p> : null}
      </section>
    );
  }

  return (
    <section className="skill-lifecycle-panel">
      <header className="skill-lifecycle-head">
        <div><small>USER PROGRAM</small><h3>已安装的用户程序</h3></div>
        <button className="skill-refresh" disabled={disabled} onClick={() => void refresh()} type="button">刷新</button>
      </header>
      {error ? <p className="skill-error" role="alert">{error}</p> : null}
      {notice ? <p className="skill-notice" role="status">{notice}</p> : null}
      <ul className="skill-catalog-list">
        {programs.map((entry) => {
          const rowBusy = busyId === entry.programId;
          return (
            <li className="skill-catalog-card" data-healthy={true} key={entry.programId}>
              <div className="skill-card-head">
                <div>
                  <strong>{entry.programId}</strong>
                  <span className="skill-card-meta">v{entry.version} · {entry.commands.length} 命令 · {formatMemoryBudget(entry.memoryBytes)}</span>
                </div>
                <span className="skill-status-badge" data-state={entry.permissionGranted ? "authorized" : "permission-required"}>{describeProgramStatus(entry)}</span>
              </div>
              <div className="skill-capability-chips">
                {entry.capabilities.length
                  ? entry.capabilities.map((capability) => (
                      <span className="skill-capability-chip" key={capability}>{capabilityLabels[capability] ?? capability}</span>
                    ))
                  : <span className="skill-capability-chip skill-capability-empty">无需能力</span>}
              </div>
              <div className="skill-card-actions">
                <button
                  className="skill-action"
                  disabled={disabled || rowBusy || entry.permissionGranted}
                  onClick={() => void runGuarded(entry.programId, async () => {
                    await desktopApi.grantUserProgramPermissions(entry.programId);
                    return `${entry.programId} 已授权，现在可以执行`;
                  })}
                  type="button"
                >{entry.permissionGranted ? "已授权" : "授权"}</button>
                <button
                  className="skill-action"
                  disabled={disabled || rowBusy || !entry.permissionGranted}
                  onClick={() => void runGuarded(entry.programId, async () => {
                    await desktopApi.revokeUserProgramPermissions(entry.programId);
                    return `${entry.programId} 已撤销授权`;
                  })}
                  type="button"
                >撤销</button>
                <button
                  className="skill-action skill-action-run"
                  disabled={disabled || rowBusy || !entry.permissionGranted}
                  onClick={() => void runGuarded(entry.programId, async () => {
                    const receipt = await desktopApi.executeInstalledUserProgram(entry.programId);
                    if (!receipt) return `${entry.programId} 未返回执行结果`;
                    return summarizeProgramReceipt(entry.programId, receipt);
                  })}
                  type="button"
                >执行</button>
                <button
                  className="skill-action skill-action-danger"
                  disabled={disabled || rowBusy}
                  onClick={() => void runGuarded(entry.programId, async () => {
                    const receipt = await desktopApi.rollbackUserProgram(entry.programId);
                    if (receipt?.quarantinedFailedVersion) {
                      return `${entry.programId} 已回滚，失败版本已隔离`;
                    }
                    return `${entry.programId} 已回滚`;
                  })}
                  type="button"
                >回滚</button>
              </div>
              {entry.subscriptions.length ? (
                <p className="skill-card-hint">订阅事件：{entry.subscriptions.join("、")}</p>
              ) : null}
            </li>
          );
        })}
      </ul>
      {rejected > 0 ? <p className="skill-card-hint">{rejected} 个损坏的程序条目已被跳过。</p> : null}
    </section>
  );
}
