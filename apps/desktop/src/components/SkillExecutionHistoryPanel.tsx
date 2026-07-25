import { useCallback, useEffect, useState } from "react";
import {
  desktopApi,
  type SkillExecutionHistoryPage,
  type SkillExecutionHistoryRecord,
} from "../platform/desktop";

const statusLabels: Record<SkillExecutionHistoryRecord["status"], string> = {
  waitingForApproval: "等待批准",
  completed: "已完成",
  rejected: "已拒绝",
  cancelled: "已取消",
  failed: "已失败",
};

const statusStates: Record<SkillExecutionHistoryRecord["status"], string> = {
  waitingForApproval: "permission-required",
  completed: "authorized",
  rejected: "permission-required",
  cancelled: "permission-required",
  failed: "permission-required",
};

export function skillStatusLabel(status: SkillExecutionHistoryRecord["status"]): string {
  return statusLabels[status] ?? status;
}

export function isCancellable(record: SkillExecutionHistoryRecord): boolean {
  return record.status === "waitingForApproval";
}

export function summarizeSkillHistoryRecord(record: SkillExecutionHistoryRecord): string {
  const parts = [`${record.commandCount} 命令`];
  if (record.agentTaskCount > 0) parts.push(`${record.agentTaskCount} Agent 任务`);
  if (record.error) parts.push(`错误：${record.error}`);
  return parts.join(" · ");
}

export function formatSkillHistoryTime(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "时间未知";
  return new Date(ms).toLocaleString();
}

export function SkillExecutionHistoryPanel({ disabled }: { disabled: boolean }) {
  const [page, setPage] = useState<SkillExecutionHistoryPage | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const next = await desktopApi.skillExecutionHistory(50);
      setPage(next);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "技能执行历史暂时不可用");
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const guarded = useCallback(
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

  const records = page?.records ?? [];

  if (loaded && records.length === 0 && !error) {
    return (
      <section className="skill-lifecycle-panel">
        <header className="skill-lifecycle-head">
          <div><small>SKILL</small><h3>技能执行历史</h3></div>
          <button className="skill-refresh" disabled={disabled} onClick={() => void refresh()} type="button">刷新</button>
        </header>
        <div className="skill-empty-state">
          <span>✦</span>
          <p>还没有技能执行记录。在上方执行一个技能后，它的执行结果、批准状态与错误会保存在这里，可随时取消待批准的执行或清理历史。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="skill-lifecycle-panel">
      <header className="skill-lifecycle-head">
        <div><small>SKILL</small><h3>技能执行历史</h3></div>
        <div className="skill-head-actions">
          <button
            className="skill-refresh"
            disabled={disabled || busyId !== null || records.length === 0}
            onClick={() => void guarded("__all__", async () => {
              const deleted = await desktopApi.deleteSkillExecutionHistory();
              return `已清理 ${deleted} 条历史记录`;
            })}
            type="button"
          >全部清除</button>
          <button className="skill-refresh" disabled={disabled || busyId !== null} onClick={() => void refresh()} type="button">刷新</button>
        </div>
      </header>

      {error ? <p className="skill-error" role="alert">{error}</p> : null}
      {notice ? <p className="skill-notice" role="status">{notice}</p> : null}

      <ul className="skill-catalog-list">
        {records.map((record) => {
          const rowBusy = busyId === record.executionId;
          return (
            <li className="skill-catalog-card" data-healthy={record.status !== "failed"} key={record.executionId}>
              <div className="skill-card-head">
                <div>
                  <strong>{record.skillId}</strong>
                  <span className="skill-card-meta">{summarizeSkillHistoryRecord(record)}</span>
                </div>
                <span className="skill-status-badge" data-state={statusStates[record.status]}>{skillStatusLabel(record.status)}</span>
              </div>
              <p className="skill-card-hint" style={{ color: "var(--muted)" }}>{formatSkillHistoryTime(record.createdAtMs)}</p>
              <div className="skill-card-actions">
                <button
                  className="skill-action"
                  disabled={disabled || rowBusy || !isCancellable(record)}
                  onClick={() => void guarded(record.executionId, async () => {
                    const cancelled = await desktopApi.cancelSkillExecution(record.executionId);
                    return cancelled ? `${record.skillId} 执行已取消` : `${record.skillId} 无法取消（可能已结束）`;
                  })}
                  type="button"
                >取消</button>
                <button
                  className="skill-action skill-action-danger"
                  disabled={disabled || rowBusy}
                  onClick={() => void guarded(record.executionId, async () => {
                    const deleted = await desktopApi.deleteSkillExecutionHistory(record.executionId);
                    return deleted > 0 ? `已删除 ${record.skillId} 的执行记录` : "记录已不存在";
                  })}
                  type="button"
                >删除</button>
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
