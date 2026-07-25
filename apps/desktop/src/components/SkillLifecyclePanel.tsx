import { useCallback, useEffect, useState } from "react";
import {
  desktopApi,
  type SkillApprovalCatalogEntry,
  type SkillCatalogEntry,
  type SkillExecutionReceipt,
} from "../platform/desktop";

const capabilityLabels: Record<string, string> = {
  "invoke-agent-tasks": "调用 Agent 任务",
  "invoke-commands": "调用命令",
  "contribute-agent-tools": "贡献 Agent 工具",
  "store-local-data": "本地存储",
  "subscribe-events": "订阅事件",
};

const statusLabels: Record<string, string> = {
  "permission-required": "待授权",
  authorized: "已授权",
  activated: "运行中",
  suspended: "已暂停",
  crashed: "已崩溃",
  quarantined: "已隔离",
};

export function describeStatus(entry: SkillCatalogEntry): string {
  if (!entry.healthy) return "清单已失效";
  if (entry.runtimeStatus) return statusLabels[entry.runtimeStatus] ?? entry.runtimeStatus;
  if (!entry.authorized) return "待授权";
  if (!entry.enabled) return "已授权 · 未启用";
  return "已启用";
}

export function summarizeSkillReceipt(receipt: SkillExecutionReceipt): string {
  if (receipt.status === "waitingForApproval") return `${receipt.skillId} 触发了待批准的命令`;
  if (receipt.status === "rejected") return `${receipt.skillId} 执行被拒绝`;
  return `${receipt.skillId} 执行完成 · ${receipt.commandResults.length} 条命令 · ${receipt.agentResults.length} 个 Agent 结果`;
}

export function SkillLifecyclePanel({ disabled }: { disabled: boolean }) {
  const [skills, setSkills] = useState<SkillCatalogEntry[]>([]);
  const [approvals, setApprovals] = useState<SkillApprovalCatalogEntry[]>([]);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [catalog, approvalCatalog] = await Promise.all([
        desktopApi.skillCatalog(),
        desktopApi.pendingSkillApprovals(),
      ]);
      setSkills(catalog.skills);
      setApprovals(approvalCatalog.approvals);
      setError(null);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Skill 目录暂时不可用");
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

  if (loaded && skills.length === 0 && approvals.length === 0 && !error) {
    return (
      <section className="skill-lifecycle-panel">
        <header className="skill-lifecycle-head">
          <div><small>SKILL LIFECYCLE</small><h3>已安装的 Skill</h3></div>
          <button className="skill-refresh" disabled={disabled} onClick={() => void refresh()} type="button">刷新</button>
        </header>
        <div className="skill-empty-state">
          <span>✦</span>
          <p>还没有安装任何 Skill。在上方用 AI Creator 生成并安装一个 Skill 草案后，它会出现在这里等待授权与启用。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="skill-lifecycle-panel">
      <header className="skill-lifecycle-head">
        <div><small>SKILL LIFECYCLE</small><h3>已安装的 Skill</h3></div>
        <button className="skill-refresh" disabled={disabled} onClick={() => void refresh()} type="button">刷新</button>
      </header>
      {error ? <p className="skill-error" role="alert">{error}</p> : null}
      {notice ? <p className="skill-notice" role="status">{notice}</p> : null}
      <ul className="skill-catalog-list">
        {skills.map((entry) => {
          const rowBusy = busyId === entry.skillId;
          return (
            <li className="skill-catalog-card" data-healthy={entry.healthy} key={entry.skillId}>
              <div className="skill-card-head">
                <div>
                  <strong>{entry.skillId}</strong>
                  <span className="skill-card-meta">v{entry.version} · {entry.publisher || "未知发布者"}</span>
                </div>
                <span className="skill-status-badge" data-state={entry.runtimeStatus ?? (entry.authorized ? "authorized" : "permission-required")}>{describeStatus(entry)}</span>
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
                  disabled={disabled || rowBusy || !entry.healthy || entry.authorized}
                  onClick={() => void runGuarded(entry.skillId, async () => {
                    await desktopApi.authorizeSkill(entry.skillId);
                    return `${entry.skillId} 已授权，现在可以启用`;
                  })}
                  type="button"
                >{entry.authorized ? "已授权" : "授权"}</button>
                <button
                  className="skill-action"
                  disabled={disabled || rowBusy || !entry.healthy || !entry.authorized}
                  onClick={() => void runGuarded(entry.skillId, async () => {
                    const next = !entry.enabled;
                    await desktopApi.setSkillEnabled(entry.skillId, next);
                    return `${entry.skillId} 已${next ? "启用" : "停用"}`;
                  })}
                  type="button"
                >{entry.enabled ? "停用" : "启用"}</button>
                <button
                  className="skill-action skill-action-run"
                  disabled={disabled || rowBusy || !entry.healthy || !entry.enabled}
                  onClick={() => void runGuarded(entry.skillId, async () => {
                    const receipt = await desktopApi.executeSkill(entry.skillId, "manual.trigger");
                    return summarizeSkillReceipt(receipt);
                  })}
                  type="button"
                >试运行</button>
                <button
                  className="skill-action skill-action-danger"
                  disabled={disabled || rowBusy || !entry.healthy}
                  onClick={() => void runGuarded(entry.skillId, async () => {
                    const receipt = await desktopApi.rollbackInstalledSkill(entry.skillId);
                    return `${entry.skillId} 已回滚到 v${receipt.restoredVersion}，需重新授权`;
                  })}
                  type="button"
                >回滚</button>
              </div>
              {!entry.healthy ? <p className="skill-card-hint">清单与已登记能力不一致，请回滚或重新安装后再授权。</p> : null}
            </li>
          );
        })}
      </ul>
      {approvals.length ? (
        <div className="skill-approval-queue">
          <h4>待批准的命令</h4>
          <ul>
            {approvals.map((approval) => {
              const rowBusy = busyId === approval.approvalId;
              return (
                <li className="skill-approval-card" key={approval.approvalId}>
                  <div className="skill-approval-head">
                    <strong>{approval.skillId}</strong>
                    <span>{approval.commands.length} 条命令 · {new Date(approval.expiresAtMs).toLocaleTimeString()} 前有效</span>
                  </div>
                  <ul className="skill-approval-commands">
                    {approval.commands.map((command) => (
                      <li key={command.commandId}><code>{command.commandId}</code><span className="skill-risk-chip" data-risk={command.risk}>{command.risk}</span></li>
                    ))}
                  </ul>
                  <div className="skill-approval-actions">
                    <button
                      className="skill-action skill-action-run"
                      disabled={disabled || rowBusy}
                      onClick={() => void runGuarded(approval.approvalId, async () => {
                        const receipt = await desktopApi.approveSkillExecution(approval.approvalId);
                        return summarizeSkillReceipt(receipt);
                      })}
                      type="button"
                    >批准</button>
                    <button
                      className="skill-action skill-action-danger"
                      disabled={disabled || rowBusy}
                      onClick={() => void runGuarded(approval.approvalId, async () => {
                        await desktopApi.rejectSkillExecution(approval.approvalId);
                        return `${approval.skillId} 的命令已拒绝`;
                      })}
                      type="button"
                    >拒绝</button>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
