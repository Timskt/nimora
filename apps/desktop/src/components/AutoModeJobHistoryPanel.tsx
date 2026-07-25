import { useCallback, useEffect, useState } from "react";
import { desktopApi, type DesktopAutoModeJobSnapshot } from "../platform/desktop";
import { autoModePhaseLabel } from "./agentCompanion";

const statusStates: Record<DesktopAutoModeJobSnapshot["status"], string> = {
  starting: "authorized",
  running: "authorized",
  pausing: "permission-required",
  cancelling: "permission-required",
  paused: "permission-required",
  completed: "authorized",
  cancelled: "permission-required",
  failed: "permission-required",
  indeterminate: "permission-required",
};

export function autoModeJobStatusLabel(status: DesktopAutoModeJobSnapshot["status"]): string {
  return autoModePhaseLabel(status);
}

export function isTerminalFailureJob(job: DesktopAutoModeJobSnapshot): boolean {
  return job.status === "failed" || job.status === "indeterminate";
}

export function summarizeAutoModeJob(job: DesktopAutoModeJobSnapshot): string {
  const parts = [`${job.turnsExecuted} 轮`, `缓存命中 ${job.cacheHits}`, `检查点 #${job.checkpointSequence}`];
  if (job.pauseReason) parts.push(`暂停原因：${job.pauseReason}`);
  if (job.errorCode) parts.push(`错误码：${job.errorCode}`);
  return parts.join(" · ");
}

export function formatAutoModeJobTime(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "时间未知";
  return new Date(ms).toLocaleString("zh-CN");
}

export function partitionAutoModeJobs(
  jobs: DesktopAutoModeJobSnapshot[],
): { failed: DesktopAutoModeJobSnapshot[]; others: DesktopAutoModeJobSnapshot[] } {
  const failed: DesktopAutoModeJobSnapshot[] = [];
  const others: DesktopAutoModeJobSnapshot[] = [];
  for (const job of jobs) {
    if (isTerminalFailureJob(job)) failed.push(job);
    else others.push(job);
  }
  return { failed, others };
}

export function AutoModeJobHistoryPanel({ disabled }: { disabled: boolean }) {
  const [jobs, setJobs] = useState<DesktopAutoModeJobSnapshot[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const next = await desktopApi.autoModeJobHistory();
      setJobs(next);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "无人值守任务历史暂时不可用");
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const inspect = useCallback(async (jobId: string) => {
    setBusyId(jobId);
    setError(null);
    setNotice(null);
    try {
      const snapshot = await desktopApi.autoModeJobStatus(jobId);
      setJobs((current) => current.map((job) => (job.jobId === jobId ? snapshot : job)));
      setNotice(`已刷新任务 ${jobId} 的状态：${autoModeJobStatusLabel(snapshot.status)}`);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "刷新任务状态失败");
    } finally {
      setBusyId(null);
    }
  }, []);

  const { failed, others } = partitionAutoModeJobs(jobs);
  const ordered = [...failed, ...others];

  if (loaded && jobs.length === 0 && !error) {
    return (
      <section className="skill-lifecycle-panel" aria-label="无人值守任务历史">
        <header className="skill-lifecycle-head">
          <div><small>AUTO MODE</small><h3>无人值守任务历史</h3></div>
          <button className="skill-refresh" disabled={disabled} onClick={() => void refresh()} type="button">刷新</button>
        </header>
        <div className="skill-empty-state">
          <span>✦</span>
          <p>还没有无人值守任务。启动一个目标后，它的运行事实、失败与「结果未知」记录都会保存在这里——即使控制中心因会话未持久化而看不到它们。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="skill-lifecycle-panel" aria-label="无人值守任务历史">
      <header className="skill-lifecycle-head">
        <div>
          <small>AUTO MODE</small>
          <h3>无人值守任务历史</h3>
        </div>
        <div className="skill-head-actions">
          {failed.length > 0 ? <span className="skill-status-badge" data-state="permission-required">{failed.length} 个需关注</span> : null}
          <button className="skill-refresh" disabled={disabled || busyId !== null} onClick={() => void refresh()} type="button">刷新</button>
        </div>
      </header>

      {error ? <p className="skill-error" role="alert">{error}</p> : null}
      {notice ? <p className="skill-notice" role="status">{notice}</p> : null}

      <ul className="skill-catalog-list">
        {ordered.map((job) => {
          const rowBusy = busyId === job.jobId;
          const failing = isTerminalFailureJob(job);
          return (
            <li className="skill-catalog-card" data-healthy={!failing} key={job.jobId}>
              <div className="skill-card-head">
                <div>
                  <strong>{job.jobId}</strong>
                  <span className="skill-card-meta">{summarizeAutoModeJob(job)}</span>
                </div>
                <span className="skill-status-badge" data-state={statusStates[job.status]}>{autoModeJobStatusLabel(job.status)}</span>
              </div>
              <p className="skill-card-hint" style={{ color: "var(--muted)" }}>会话 {job.sessionId} · {formatAutoModeJobTime(job.updatedAtMs)}</p>
              {failing ? (
                <p className="skill-card-hint" role="alert">
                  {job.status === "indeterminate"
                    ? "外部执行结果未知，禁止自动重试；请在控制中心完成对账。"
                    : "任务已失败；控制中心可能因会话未完整持久化而看不到它，可在此复核错误码。"}
                </p>
              ) : null}
              <div className="skill-card-actions">
                <button
                  className="skill-action"
                  disabled={disabled || rowBusy}
                  onClick={() => void inspect(job.jobId)}
                  type="button"
                >{rowBusy ? "刷新中…" : "刷新状态"}</button>
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
