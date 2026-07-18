import { useEffect, useMemo, useState } from "react";
import type { AutomationCatalogEntry, AutomationDefinition, AutomationJournalEntry, AutomationRun } from "../platform/desktop";
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
  policy: { timeoutMs: 5_000, failure: "compensate" },
};

export function AutomationWorkspace({ disabled, onNotice }: { disabled: boolean; onNotice(message: string): void }) {
  const [succeeded, setSucceeded] = useState(true);
  const [busy, setBusy] = useState(false);
  const [run, setRun] = useState<AutomationRun | null>(null);
  const [catalog, setCatalog] = useState<AutomationCatalogEntry[]>([]);
  const [history, setHistory] = useState<AutomationJournalEntry[]>([]);
  const definition = useMemo(() => sampleDefinition, []);

  async function refreshCatalog() {
    try {
      setCatalog(await desktopApi.automationCatalog());
    } catch {
      setCatalog([]);
    }
  }

  async function refreshHistory() {
    try {
      setHistory(await desktopApi.automationRunHistory());
    } catch {
      setHistory([]);
    }
  }

  useEffect(() => { void Promise.all([refreshCatalog(), refreshHistory()]); }, []);

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
      await refreshCatalog();
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
      <section className="automation-catalog" aria-label="已安装自动化目录">
        <div className="section-heading"><div><p className="card-label">INSTALLED CATALOG</p><h3>已安装自动化</h3></div><span>{catalog.length} 项</span></div>
        {catalog.length === 0 ? <p>尚未安装自动化。可在“扩展”工作区让外接 AI 生成、审查并原子安装。</p> : catalog.map((entry) => <article className="automation-catalog-entry" key={entry.definition.id}>
          <div><strong>{entry.definition.name}</strong><code>{entry.definition.id} · {entry.definition.version}</code><small>{entry.previousVersion ? `可回滚：${entry.previousVersion}` : "首次安装"}</small></div>
          <span className={entry.enabled ? "automation-enabled" : "automation-disabled"}>{entry.enabled ? "已启用" : "已停用"}</span>
          <button disabled={disabled || busy} onClick={() => void toggleAutomation(entry)} type="button">{entry.enabled ? "停用" : "启用"}</button>
          <button disabled={disabled || busy || !entry.previousVersion} onClick={() => void rollbackAutomation(entry)} type="button">回滚</button>
        </article>)}
      </section>
      <section className="automation-catalog" aria-label="自动化运行历史">
        <div className="section-heading"><div><p className="card-label">RUN HISTORY</p><h3>最近运行</h3></div><button disabled={disabled || busy || history.length === 0} onClick={() => void deleteHistory()} type="button">清空终态记录</button></div>
        {history.length === 0 ? <p>暂无真实运行记录。测试运行不会写入这里。</p> : history.map((entry) => <article className="automation-catalog-entry" key={entry.runId}>
          <div><strong>{entry.automationId}</strong><code>{entry.result?.status ?? entry.status}</code><small>{new Date(entry.startedAtMs).toLocaleString()} · Event {entry.eventId}</small></div>
          <span className={entry.status === "running" ? "automation-enabled" : "automation-disabled"}>{entry.status === "running" ? "运行中" : entry.status === "interrupted" ? "已中断" : "已完成"}</span>
          <button disabled={disabled || busy || entry.status === "running"} onClick={() => void deleteHistory(entry.runId)} type="button">删除</button>
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
  return "运行未完成";
}
