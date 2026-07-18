import { useEffect, useMemo, useState } from "react";
import type { AutomationApprovalCatalog, AutomationCatalogEntry, AutomationCostReconciliationCatalog, AutomationCostReconciliationReason, AutomationDefinition, AutomationEventHealthSnapshot, AutomationGovernanceCatalog, AutomationJournalEntry, AutomationRun } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";

const sampleDefinition: AutomationDefinition = {
  spec: "nimora.automation/1",
  id: "local.focus.on-build",
  version: "1.0.0",
  name: "构建完成后庆祝",
  enabled: true,
  trigger: { eventType: "dev.build.finished" },
  conditions: [{ pointer: "/succeeded", equals: true }],
  actions: [{
    id: "celebrate",
    command: "pet.animation.play",
    arguments: { action: "celebrate" },
    risk: "low",
    retrySafe: true,
    idempotencyKey: "preview-build-celebrate",
    compensation: {
      command: "pet.animation.play",
      arguments: { action: "idle" },
      risk: "low",
    },
  }],
  policy: {
    timeoutMs: 5_000,
    failure: "compensate",
    maxConcurrentRuns: 1,
    cooldownMs: 0,
    dailyCostBudgetMicrounits: 0,
  },
};

export function AutomationWorkspace({ disabled, onNotice }: { disabled: boolean; onNotice(message: string): void }) {
  const [succeeded, setSucceeded] = useState(true);
  const [busy, setBusy] = useState(false);
  const [run, setRun] = useState<AutomationRun | null>(null);
  const [catalog, setCatalog] = useState<AutomationCatalogEntry[]>([]);
  const [history, setHistory] = useState<AutomationJournalEntry[]>([]);
  const [historyExhausted, setHistoryExhausted] = useState(false);
  const [eventHealth, setEventHealth] = useState<AutomationEventHealthSnapshot["sessions"]>([]);
  const [governance, setGovernance] = useState<AutomationGovernanceCatalog["entries"]>([]);
  const [costReconciliation, setCostReconciliation] = useState<AutomationCostReconciliationCatalog>({ spec: "nimora.automation-cost-reconciliation-catalog/1", pending: [], decisions: [] });
  const [costInputs, setCostInputs] = useState<Record<string, string>>({});
  const [costReasons, setCostReasons] = useState<Record<string, AutomationCostReconciliationReason>>({});
  const [costConfirmed, setCostConfirmed] = useState<Record<string, boolean>>({});
  const [approvals, setApprovals] = useState<AutomationApprovalCatalog["approvals"]>([]);
  const definition = useMemo(() => sampleDefinition, []);

  async function refreshCatalog() {
    try {
      setCatalog(await desktopApi.automationCatalog());
    } catch {
      setCatalog([]);
    }
  }

  async function refreshHistory(append = false) {
    try {
      const cursor = append ? history.at(-1) : undefined;
      const page = await desktopApi.automationRunHistory(20, cursor ? { startedAtMs: cursor.startedAtMs, runId: cursor.runId } : undefined);
      setHistory((current) => append ? [...current, ...page.records] : page.records);
      setHistoryExhausted(page.records.length < 20);
    } catch {
      if (!append) setHistory([]);
    }
  }

  async function refreshEventHealth() {
    try {
      setEventHealth((await desktopApi.automationEventHealth()).sessions);
    } catch {
      setEventHealth([]);
    }
  }

  async function refreshGovernance() {
    try {
      setGovernance((await desktopApi.automationGovernanceCatalog()).entries);
    } catch {
      setGovernance([]);
    }
  }

  async function refreshCostReconciliation() {
    try {
      setCostReconciliation(await desktopApi.automationCostReconciliationCatalog());
    } catch {
      setCostReconciliation({ spec: "nimora.automation-cost-reconciliation-catalog/1", pending: [], decisions: [] });
    }
  }

  async function refreshApprovals() {
    try {
      setApprovals((await desktopApi.pendingAutomationApprovals()).approvals);
    } catch {
      setApprovals([]);
    }
  }

  useEffect(() => { void Promise.all([refreshCatalog(), refreshHistory(), refreshEventHealth(), refreshGovernance(), refreshCostReconciliation(), refreshApprovals()]); }, []);

  async function reconcileCost(entry: AutomationCostReconciliationCatalog["pending"][number]) {
    const actualCostMicrounits = Number(costInputs[entry.taskId]);
    if (!Number.isSafeInteger(actualCostMicrounits) || actualCostMicrounits < 0 || !costConfirmed[entry.taskId]) {
      onNotice("请输入非负整数费用，并确认已核对外部账单证据");
      return;
    }
    setBusy(true);
    try {
      await desktopApi.reconcileAutomationCost({
        taskId: entry.taskId,
        expectedUpdatedAtMs: entry.updatedAtMs,
        actualCostMicrounits,
        reason: costReasons[entry.taskId] ?? "provider_statement",
      });
      setCostInputs((current) => ({ ...current, [entry.taskId]: "" }));
      setCostConfirmed((current) => ({ ...current, [entry.taskId]: false }));
      await Promise.all([refreshCostReconciliation(), refreshGovernance()]);
      onNotice("未知费用已原子结算，不可变决议审计已写入");
    } catch {
      await refreshCostReconciliation();
      onNotice("费用账本已变化或决议冲突，未修改任何记录");
    } finally {
      setBusy(false);
    }
  }

  async function resolveApproval(approvalId: string, approve: boolean) {
    setBusy(true);
    try {
      if (approve) {
        const result = await desktopApi.approveAutomationRun(approvalId);
        setRun(result);
        onNotice(result.status === "succeeded" ? "自动化已按已审查参数执行" : "自动化已执行并记录终态");
      } else {
        await desktopApi.rejectAutomationRun(approvalId);
        onNotice("自动化运行已拒绝，未产生任何副作用");
      }
      await Promise.all([refreshApprovals(), refreshHistory(), refreshEventHealth(), refreshGovernance()]);
    } catch (error) {
      await refreshApprovals();
      onNotice(automationFailureNotice(error));
    } finally {
      setBusy(false);
    }
  }

  async function deleteHistory(runId?: string) {
    setBusy(true);
    try {
      const deleted = await desktopApi.deleteAutomationRunHistory(runId);
      await refreshHistory();
      onNotice(deleted > 0 ? "自动化运行历史已删除" : "运行中记录受到保护，未删除");
    } catch {
      onNotice("运行历史删除失败，审计记录保持不变");
    } finally {
      setBusy(false);
    }
  }

  async function toggleAutomation(entry: AutomationCatalogEntry) {
    setBusy(true);
    try {
      await desktopApi.setAutomationEnabled(entry.definition.id, !entry.enabled);
      await Promise.all([refreshCatalog(), refreshEventHealth(), refreshGovernance()]);
      onNotice(!entry.enabled ? "自动化已显式启用" : "自动化已停用");
    } catch {
      onNotice("自动化状态更新失败，未改变目录状态");
    } finally {
      setBusy(false);
    }
  }

  async function rollbackAutomation(entry: AutomationCatalogEntry) {
    setBusy(true);
    try {
      const receipt = await desktopApi.rollbackAutomation(entry.definition.id);
      await refreshCatalog();
      onNotice(`已回滚到 ${receipt.version}，出于安全考虑保持停用`);
    } catch {
      onNotice("没有可用的上一版本，未执行回滚");
    } finally {
      setBusy(false);
    }
  }

  async function testRun() {
    setBusy(true);
    try {
      const result = await desktopApi.testAutomation(definition, "dev.build.finished", { succeeded });
      setRun(result);
      onNotice(result.status === "planned" ? "测试运行完成，没有产生真实副作用" : "测试事件未满足自动化条件");
    } catch {
      onNotice("自动化定义无效，未执行任何动作");
    } finally {
      setBusy(false);
    }
  }

  return <section className="automation-workspace" aria-labelledby="automation-heading">
    <div className="automation-hero">
      <div>
        <p className="eyebrow">LOCAL AUTOMATION</p>
        <h2 id="automation-heading">让事件安全地驱动每个模块。</h2>
        <p>规则、用户代码和 Agent 共用 Command、Event 与 Capability Gateway。测试运行只生成计划，绝不会触发真实副作用。</p>
      </div>
      <span>离线可用</span>
    </div>
    <div className="automation-grid">
      <section className="automation-catalog automation-approvals" aria-label="待批准自动化运行">
        <div className="section-heading"><div><p className="card-label">RUNTIME APPROVAL</p><h3>待批准运行</h3></div><button disabled={disabled || busy} onClick={() => void refreshApprovals()} type="button">刷新</button></div>
        {approvals.length === 0 ? <p>没有等待批准的运行。中高风险动作会在任何副作用发生前出现在这里。</p> : approvals.map((approval) => <article className="automation-approval-card" key={approval.approvalId}>
          <div className="automation-approval-heading"><div><strong>{approval.automationId}</strong><small>版本 {approval.automationVersion} · {new Date(approval.expiresAtMs).toLocaleTimeString()} 过期</small></div><code>{approval.runId}</code></div>
          <div className="automation-safety-note"><strong>整次执行尚未开始</strong><p>批准绑定当前版本、事件和下列实际参数；参数或宿主风险策略变化后旧批准自动失效。</p></div>
          <div className="automation-risk-list">{approval.risks.map((risk) => <div className="automation-risk-row" key={`${risk.actionId}:${risk.command}`}>
            <div><strong>{risk.actionId}</strong><code>{risk.command}</code></div>
            <span data-risk={risk.effectiveRisk}>{risk.effectiveRisk.toUpperCase()}</span>
            <pre>{JSON.stringify(risk.arguments, null, 2)}</pre>
          </div>)}</div>
          <div className="automation-approval-actions"><button disabled={disabled || busy} onClick={() => void resolveApproval(approval.approvalId, false)} type="button">拒绝</button><button className="primary-button" disabled={disabled || busy} onClick={() => void resolveApproval(approval.approvalId, true)} type="button">批准整次执行</button></div>
        </article>)}
      </section>
      <section className="automation-catalog" aria-label="已安装自动化目录">
        <div className="section-heading"><div><p className="card-label">INSTALLED CATALOG</p><h3>已安装自动化</h3></div><span>{catalog.length} 项</span></div>
        {catalog.length === 0 ? <p>尚未安装自动化。可在“扩展”工作区让外接 AI 生成、审查并原子安装。</p> : catalog.map((entry) => <article className="automation-catalog-entry" key={entry.definition.id}>
          <div><strong>{entry.definition.name}</strong><code>{entry.definition.id} · {entry.definition.version}</code><small>{entry.previousVersion ? `可回滚：${entry.previousVersion}` : "首次安装"}</small></div>
          <span className={entry.enabled ? "automation-enabled" : "automation-disabled"}>{entry.enabled ? "已启用" : "已停用"}</span>
          <button disabled={disabled || busy} onClick={() => void toggleAutomation(entry)} type="button">{entry.enabled ? "停用" : "启用"}</button>
          <button disabled={disabled || busy || !entry.previousVersion} onClick={() => void rollbackAutomation(entry)} type="button">回滚</button>
        </article>)}
      </section>
      <section className="automation-catalog automation-governance" aria-label="自动化资源与费用治理">
        <div className="section-heading"><div><p className="card-label">RESOURCE GOVERNANCE</p><h3>资源与 AI 费用</h3></div><button disabled={busy} onClick={() => void refreshGovernance()} type="button">刷新</button></div>
        {governance.length === 0 ? <p>尚无可展示的已安装 Automation 治理状态；浏览器预览不会伪造本机账本。</p> : governance.map((entry) => {
          const hasUnknownCost = entry.indeterminateCostCount > 0;
          return <article className="automation-governance-card" data-alert={hasUnknownCost} key={entry.automationId}>
            <div className="automation-governance-heading"><div><strong>{entry.automationId}</strong><small>{entry.activeRuns} / {entry.maxConcurrentRuns} 个运行租约</small></div><span className={hasUnknownCost ? "automation-governance-alert" : "automation-enabled"}>{hasUnknownCost ? `${entry.indeterminateCostCount} 项待对账` : entry.cooldownRemainingMs > 0 ? "冷却中" : "准入正常"}</span></div>
            <dl className="automation-governance-metrics">
              <div><dt>并发</dt><dd>{entry.activeRuns} / {entry.maxConcurrentRuns}</dd></div>
              <div><dt>冷却剩余</dt><dd>{formatDuration(entry.cooldownRemainingMs)}</dd></div>
              <div><dt>今日已结算</dt><dd>{formatMicrounits(entry.settledCostMicrounits)}</dd></div>
              <div><dt>执行中预留</dt><dd>{formatMicrounits(entry.reservedCostMicrounits)}</dd></div>
              <div><dt>未知费用占用</dt><dd>{formatMicrounits(entry.indeterminateCostMicrounits)}</dd></div>
              <div><dt>今日可用</dt><dd>{entry.dailyCostBudgetMicrounits === 0 ? "仅零费用任务" : formatMicrounits(entry.availableCostMicrounits)}</dd></div>
            </dl>
            {hasUnknownCost && <div className="automation-governance-warning"><strong>费用状态未知，预算不会自动释放</strong><p>Provider 或宿主未能证明最终费用。系统保持失败关闭，避免按零费用重复执行。</p></div>}
          </article>;
        })}
      </section>
      <section className="automation-catalog automation-reconciliation" aria-label="未知 AI 费用人工对账">
        <div className="section-heading"><div><p className="card-label">COST RECONCILIATION</p><h3>未知费用对账</h3></div><button disabled={busy} onClick={() => void refreshCostReconciliation()} type="button">刷新账本</button></div>
        {costReconciliation.pending.length === 0 ? <p>没有待对账费用。历史决议仍保留在不可变审计中。</p> : costReconciliation.pending.map((entry) => <article className="automation-reconciliation-card" key={entry.taskId}>
          <div className="automation-governance-heading"><div><strong>{entry.automationId}</strong><small>Task {entry.taskId} · Run {entry.runId}</small></div><span className="automation-governance-alert">预留 {formatMicrounits(entry.reservedCostMicrounits)}</span></div>
          <div className="automation-reconciliation-grid">
            <label>实际费用（微单位）<input inputMode="numeric" min="0" step="1" value={costInputs[entry.taskId] ?? ""} onChange={(event) => setCostInputs((current) => ({ ...current, [entry.taskId]: event.target.value }))} /></label>
            <label>证据来源<select value={costReasons[entry.taskId] ?? "provider_statement"} onChange={(event) => setCostReasons((current) => ({ ...current, [entry.taskId]: event.target.value as AutomationCostReconciliationReason }))}><option value="provider_statement">Provider 账单声明</option><option value="billing_export">账单导出文件</option><option value="operator_conservative_estimate">人工保守估算</option></select></label>
          </div>
          <label className="automation-reconciliation-confirm"><input checked={costConfirmed[entry.taskId] ?? false} onChange={(event) => setCostConfirmed((current) => ({ ...current, [entry.taskId]: event.target.checked }))} type="checkbox" />我已核对外部证据；此决议不可修改或删除。</label>
          <button className="primary-button" disabled={disabled || busy || !costConfirmed[entry.taskId]} onClick={() => void reconcileCost(entry)} type="button">写入不可变决议</button>
        </article>)}
        {costReconciliation.decisions.length > 0 && <div className="automation-reconciliation-audit"><h4>最近决议审计</h4>{costReconciliation.decisions.map((entry) => <article key={entry.decisionId}><div><strong>{entry.automationId}</strong><small>{new Date(entry.decidedAtMs).toLocaleString()} · {entry.reason}</small></div><span>{formatMicrounits(entry.actualCostMicrounits)} / {formatMicrounits(entry.reservedCostMicrounits)}</span></article>)}</div>}
      </section>
      <section className="automation-catalog" aria-label="自动化运行历史">
        <div className="section-heading"><div><p className="card-label">RUN HISTORY</p><h3>最近运行</h3></div><button disabled={disabled || busy || history.length === 0} onClick={() => void deleteHistory()} type="button">清空终态记录</button></div>
        {history.length === 0 ? <p>暂无真实运行记录。测试运行不会写入这里。</p> : history.map((entry) => <article className="automation-catalog-entry" key={entry.runId}>
          <div><strong>{entry.automationId}</strong><code>{entry.result?.status ?? entry.status}</code><small>{entry.result?.reason ?? `${new Date(entry.startedAtMs).toLocaleString()} · Event ${entry.eventId}`}</small></div>
          <span className={entry.status === "running" ? "automation-enabled" : "automation-disabled"}>{entry.status === "running" ? "运行中" : entry.status === "interrupted" ? "已中断" : "已完成"}</span>
          <button disabled={disabled || busy || entry.status === "running"} onClick={() => void deleteHistory(entry.runId)} type="button">删除</button>
        </article>)}
        {history.length > 0 && !historyExhausted && <button disabled={disabled || busy} onClick={() => void refreshHistory(true)} type="button">加载更早记录</button>}
      </section>
      <section className="automation-catalog" aria-label="自动化事件会话健康">
        <div className="section-heading"><div><p className="card-label">EVENT HEALTH</p><h3>事件会话</h3></div><button disabled={disabled || busy} onClick={() => void refreshEventHealth()} type="button">刷新</button></div>
        {eventHealth.length === 0 ? <p>当前没有活跃事件会话；这不代表已安装自动化处于健康状态。</p> : eventHealth.map((session) => <article className="automation-catalog-entry" key={session.sessionId}>
          <div><strong>{session.automationId}</strong><code>{session.sessionId}</code><small>已执行 {session.executed} · 丢弃 {session.dropped} · 失败 {session.failures}</small></div>
          <span className={session.dropped > 0 || session.failures > 0 ? "automation-disabled" : "automation-enabled"}>{session.dropped > 0 ? "队列有丢弃" : session.failures > 0 ? "会话失败" : session.active ? "会话活跃" : "正在停止"}</span>
        </article>)}
      </section>
      <article className="automation-rule-card">
        <div className="section-heading">
          <div><p className="card-label">示例规则</p><h3>{definition.name}</h3></div>
          <span className="automation-enabled">已启用</span>
        </div>
        <dl>
          <div><dt>触发器</dt><dd><code>{definition.trigger.eventType}</code></dd></div>
          <div><dt>条件</dt><dd><code>/succeeded == true</code></dd></div>
          <div><dt>动作</dt><dd><code>pet.animation.play</code></dd></div>
          <div><dt>失败策略</dt><dd>逆序补偿 · 5 秒超时</dd></div>
        </dl>
        <label className="automation-event-toggle">
          <input type="checkbox" checked={succeeded} onChange={(event) => setSucceeded(event.target.checked)} />
          测试事件：构建成功
        </label>
        <button className="primary-button" type="button" disabled={disabled || busy} onClick={() => void testRun()}>
          {busy ? "正在验证…" : "测试运行"}
        </button>
      </article>
      <aside className="automation-preview" aria-live="polite">
        <p className="card-label">运行预览</p>
        <h3>{run ? statusLabel(run.status) : "等待测试事件"}</h3>
        {!run && <p>运行后将在这里展示条件判定和预计步骤。</p>}
        {run?.reason && <p>{run.reason}</p>}
        {run?.steps.map((step, index) => <div className="automation-step" key={step.actionId}>
          <span>{index + 1}</span>
          <div><strong>{step.actionId}</strong><code>{step.command}</code></div>
          <em>仅计划</em>
        </div>)}
        {run?.status === "planned" && <div className="automation-safety-note"><strong>零副作用</strong><p>未调用 Renderer、Worker、网络或桌面 Command Backend。</p></div>}
      </aside>
    </div>
  </section>;
}

function statusLabel(status: AutomationRun["status"]): string {
  if (status === "planned") return "计划验证通过";
  if (status === "condition_not_matched") return "条件未满足";
  if (status === "trigger_not_matched") return "触发器未匹配";
  if (status === "waiting_for_approval") return "等待参数级批准";
  if (status === "succeeded") return "运行成功";
  if (status === "cancelled") return "运行已取消";
  if (status === "timed_out") return "运行超时";
  if (status === "compensation_failed") return "动作与补偿均失败";
  if (status === "failed") return "运行失败";
  return "运行未完成";
}

function formatMicrounits(value: number): string {
  return `${value.toLocaleString("zh-CN")} μu`;
}

function formatDuration(value: number): string {
  if (value === 0) return "无";
  if (value < 1_000) return `${value} ms`;
  return `${Math.ceil(value / 1_000)} 秒`;
}

function automationFailureNotice(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (message.includes("concurrent run limit")) return "并发运行名额已满，未开始新的自动化";
  if (message.includes("cooldown is active")) return "自动化仍在冷却期，未开始执行";
  if (message.includes("cost budget is exhausted")) return "今日 AI 费用预算不足，Provider 未被调用";
  return "审批已过期、被处理、计划发生变化或资源准入失败，未执行自动化";
}
