import { useMemo, useState } from "react";
import type { AutomationDefinition, AutomationRun } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";

const sampleDefinition: AutomationDefinition = {
  spec: "nimora.automation/1",
  id: "local.focus.on-build",
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
  const definition = useMemo(() => sampleDefinition, []);

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
