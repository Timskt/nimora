import { lazy, Suspense, useEffect, useState } from "react";
import { desktopApi, type AgentCatalog, type AgentHistoryPage, type AgentProviderStatus, type AgentToolResult, type ConcreteReasoningEffort, type DesktopAutoModeControlCenter, type LocalAgentResult, type ModelReasoningPolicy } from "../platform/desktop";

const ProviderSettings = lazy(async () => {
  const module = await import("./ProviderSettings");
  return { default: module.ProviderSettings };
});

type ControlEntry = DesktopAutoModeControlCenter["entries"][number];
type PendingControl =
  | { kind: "pause"; entry: ControlEntry }
  | { kind: "cancel"; entry: ControlEntry }
  | { kind: "resolve"; entry: ControlEntry; decision: "confirmed_not_executed" | "accept_external_effect_and_cancel" };

interface AgentWorkspaceProps {
  safeMode: boolean;
  recoveryMode: boolean;
  initialView?: "run" | "control";
  onNotice(message: string): void;
}

export function agentToolAccessLabel(effect: string): string {
  return effect === "read_only" ? "只读" : "需确认";
}

export function agentUsageTotal(result: LocalAgentResult): number {
  return result.usage ? result.usage.inputTokens + result.usage.outputTokens : 0;
}

export function agentRiskLabel(risk: AgentToolResult["effectiveRisk"]): string {
  return ({ safe: "安全", low: "低风险", medium: "中风险", high: "高风险", critical: "严重风险" })[risk];
}

export function defaultModelForProvider(providerId: string): string {
  return providerId === "provider:ollama-loopback" ? "qwen3:8b" : "model:echo-v1";
}

export function providerStatusLabel(status: AgentProviderStatus | null): string {
  if (!status) return "检测中";
  if (!status.serviceReachable) return "服务离线";
  if (status.models.length === 0) return "无模型";
  return status.providerId === "provider:deterministic-local" ? "可用" : "服务在线";
}

type ReasoningChoice = "adaptive" | "cost_saver" | "quality_first" | `fixed:${ConcreteReasoningEffort}`;

export function reasoningPolicyForChoice(choice: ReasoningChoice): ModelReasoningPolicy {
  if (choice.startsWith("fixed:")) {
    return { strategy: "fixed", requested: choice.slice(6) as ConcreteReasoningEffort, allowAutomaticDowngrade: false };
  }
  return { strategy: choice as "adaptive" | "cost_saver" | "quality_first", requested: "auto", allowAutomaticDowngrade: true };
}

export function AgentWorkspace({ safeMode, recoveryMode, initialView = "run", onNotice }: AgentWorkspaceProps) {
  const [view, setView] = useState<"run" | "control" | "providers" | "history">(initialView);
  const [controlCenter, setControlCenter] = useState<DesktopAutoModeControlCenter | null>(null);
  const [pendingControl, setPendingControl] = useState<PendingControl | null>(null);
  const [controlReason, setControlReason] = useState("");
  const [controlBusy, setControlBusy] = useState(false);
  const [catalog, setCatalog] = useState<AgentCatalog | null>(null);
  const [prompt, setPrompt] = useState("总结一下当前可用的本地能力");
  const [providerId, setProviderId] = useState("provider:deterministic-local");
  const [model, setModel] = useState("model:echo-v1");
  const [result, setResult] = useState<LocalAgentResult | null>(null);
  const [providerStatus, setProviderStatus] = useState<AgentProviderStatus | null>(null);
  const [allowNetwork, setAllowNetwork] = useState(false);
  const [reasoningChoice, setReasoningChoice] = useState<ReasoningChoice>("adaptive");
  const [busy, setBusy] = useState(false);
  const [toolBusy, setToolBusy] = useState(false);
  const [toolResult, setToolResult] = useState<AgentToolResult | null>(null);
  const [turnCancelled, setTurnCancelled] = useState(false);
  const [history, setHistory] = useState<AgentHistoryPage | null>(null);
  const activeProviderId = result?.task.providerId ?? providerId;
  const activeProvider = catalog?.providers.find((provider) => provider.id === activeProviderId);
  const reasoningCapabilities = activeProvider?.capabilities.reasoning;

  useEffect(() => {
    void refreshCatalog();
    void desktopApi.agentHistory(5).then(setHistory).catch(() => onNotice("Agent 历史暂时不可用"));
  }, [onNotice]);

  useEffect(() => setView(initialView), [initialView]);

  useEffect(() => {
    if (view !== "control") return;
    void desktopApi.autoModeControlCenter().then(setControlCenter).catch(() => onNotice("目标控制中心暂时不可用"));
  }, [view, onNotice]);

  async function refreshHistory() {
    setHistory(await desktopApi.agentHistory(5));
  }

  async function refreshCatalog() {
    try {
      setCatalog(await desktopApi.agentCatalog());
    } catch {
      onNotice("Agent 工具目录暂时不可用");
    }
  }

  async function refreshControlCenter() {
    setControlCenter(await desktopApi.autoModeControlCenter());
  }

  function prepareControl(action: PendingControl) {
    if (safeMode || recoveryMode || !desktopApi.native) return;
    setControlReason("");
    setPendingControl(action);
  }

  async function executeControl() {
    if (!pendingControl || controlBusy || safeMode || recoveryMode || !desktopApi.native) return;
    if (pendingControl.kind === "resolve" && !controlReason.trim()) return;
    setControlBusy(true);
    try {
      if (pendingControl.kind === "pause") {
        await desktopApi.pauseAutoModeJob(pendingControl.entry.job.jobId);
        onNotice("已请求安全暂停，当前原子步骤结束后生效");
      } else if (pendingControl.kind === "cancel") {
        await desktopApi.cancelAutoModeJob(pendingControl.entry.job.jobId);
        onNotice("已请求取消，正在收敛运行状态");
      } else {
        const attempt = pendingControl.entry.attempt;
        if (!attempt) throw new Error("attempt-missing");
        await desktopApi.resolveAutoModeAttempt({
          sessionId: pendingControl.entry.session.id,
          attemptId: attempt.id,
          checkpointSequence: attempt.checkpointSequence,
          requestFingerprint: attempt.requestFingerprint,
          decision: pendingControl.decision,
          reason: controlReason.trim(),
        });
        onNotice(pendingControl.decision === "confirmed_not_executed" ? "已确认外部操作未执行；任务保持暂停" : "已接受潜在外部副作用并取消任务");
      }
      setPendingControl(null);
      await refreshControlCenter();
    } catch {
      onNotice("控制请求已失效或状态发生变化，没有自动重试");
      await refreshControlCenter().catch(() => undefined);
    } finally {
      setControlBusy(false);
    }
  }

  useEffect(() => {
    let current = true;
    setProviderStatus(null);
    void desktopApi.agentProviderStatus(providerId).then((status) => {
      if (current) {
        setProviderStatus(status);
        const firstModel = status.models[0];
        if (firstModel && !status.models.some((item) => item.name === model)) {
          setModel(firstModel.name);
        }
      }
    }).catch(() => current && setProviderStatus({ spec: "nimora.desktop-agent-provider-status/1", providerId, state: "unavailable", workerVerified: false, serviceReachable: false, locality: "local", credentialPresent: false, models: [], message: "Provider 状态不可用" }));
    return () => { current = false; };
  }, [providerId]);

  async function run() {
    if (!prompt.trim() || busy) return;
    setBusy(true);
    setTurnCancelled(false);
    try {
      const next = await desktopApi.runLocalAgent(
        prompt.trim(),
        providerId,
        model.trim(),
        allowNetwork,
        reasoningCapabilities ? reasoningPolicyForChoice(reasoningChoice) : null,
      );
      setResult(next);
      if (next.status === "completed") await refreshHistory();
      onNotice(allowNetwork ? "网络 Agent 任务已完成" : "离线 Agent 任务已完成");
    } catch {
      onNotice("Agent 任务失败，未执行任何模块操作");
    } finally {
      setBusy(false);
    }
  }

  async function resolveAgentTurnTool(invocationId: string, approved: boolean) {
    if (!result || busy || safeMode || recoveryMode) return;
    setBusy(true);
    try {
      if (approved) {
        const next = await desktopApi.confirmAgentRunTool(invocationId);
        setResult(next);
        if (next.status === "completed") await refreshHistory();
        onNotice(next.status === "completed" ? "模块结果已返回 Provider，任务已完成" : "批准已记录，整轮工具仍等待确认");
      } else {
        await desktopApi.rejectAgentTool(invocationId);
        setResult(null);
        setTurnCancelled(true);
        onNotice("已拒绝本轮模块操作，整组调用均未执行");
      }
    } catch {
      setResult(null);
      setTurnCancelled(true);
      onNotice("Agent 确认已失效，整轮调用未自动重试");
    } finally {
      setBusy(false);
    }
  }

  async function prepareTool(toolId: string) {
    if (toolBusy || safeMode || recoveryMode) return;
    const argumentsValue = toolId === "pet.animation.play" ? { action: "celebrate" } : toolId === "pet.care.perform" ? { action: "feed" } : toolId === "pet.position.move" ? { x: 120, y: 120 } : {};
    setToolBusy(true);
    try {
      const next = await desktopApi.prepareAgentTool(toolId, argumentsValue);
      setToolResult(next);
      onNotice(next.requiresConfirmation ? "模块操作正在等待你的确认" : "只读模块能力已安全完成");
    } catch {
      onNotice("模块能力请求被安全边界拒绝");
    } finally {
      setToolBusy(false);
    }
  }

  async function resolveTool(approved: boolean) {
    if (!toolResult?.requiresConfirmation || toolBusy) return;
    setToolBusy(true);
    try {
      if (approved) {
        const next = await desktopApi.confirmAgentTool(toolResult.invocation.invocationId);
        setToolResult(next);
        onNotice("已确认并通过 Capability Gateway 执行");
      } else {
        await desktopApi.rejectAgentTool(toolResult.invocation.invocationId);
        setToolResult(null);
        onNotice("已拒绝模块操作，未产生副作用");
      }
    } catch {
      setToolResult(null);
      onNotice("确认已失效或执行失败，未自动重试");
    } finally {
      setToolBusy(false);
    }
  }

  async function clearHistory() {
    if (busy || !history?.records.length) return;
    setBusy(true);
    try {
      await desktopApi.deleteAgentHistory();
      await refreshHistory();
      onNotice("Agent 历史已从本机删除");
    } catch {
      onNotice("Agent 历史删除失败，现有记录保持不变");
    } finally {
      setBusy(false);
    }
  }

  const tabs = <nav className="agent-view-tabs" aria-label="Agent 工作区视图">
    {(["run", "control", "providers", "history"] as const).map((item) => <button aria-current={view === item ? "page" : undefined} key={item} onClick={() => setView(item)} type="button">{{ run: "对话运行", control: "目标控制", providers: "模型连接", history: "执行历史" }[item]}</button>)}
  </nav>;

  if (view === "control") return <><header className="agent-section-header"><div><p className="card-label">GOAL CONTROL CENTER</p><h2>目标、计划与每一次执行，都有据可查。</h2></div><span>{controlCenter?.entries.length ?? 0} 个任务</span></header>{tabs}{(safeMode || recoveryMode || !desktopApi.native) && <div className="control-readonly" role="status">{!desktopApi.native ? "浏览器预览仅展示演示数据，真实控制需要桌面宿主" : "当前运行模式仅允许查看，控制操作已锁定"}</div>}<section className="control-center" aria-label="目标控制中心">
    {controlCenter?.entries.length ? controlCenter.entries.map((entry) => <article className={entry.job.status === "indeterminate" ? "control-entry risk" : "control-entry"} key={entry.job.jobId}>
      <header><div><span>{entry.attempt?.status === "indeterminate" ? "indeterminate" : entry.effectiveStatus}</span><h3>{entry.goal.title}</h3><p>{entry.goal.objective}</p></div><strong>Plan r{entry.plan.revision}</strong></header>
      {entry.projectionStale && <p className="projection-warning" role="status">运行投影正在收敛；此处以持久化 Session 事实为准，不会自动重试外部操作。</p>}
      <div className="control-metrics"><div><small>进度</small><b>{entry.session.usage.cycles} / {entry.session.policy.maxCycles}</b></div><div><small>Checkpoint</small><b>#{entry.checkpoint?.sequence ?? 0}</b></div><div><small>缓存命中</small><b>{entry.job.cacheHits}</b></div><div><small>模型</small><b>{entry.checkpoint?.model ?? "待启动"}</b></div></div>
      <div className="control-budget"><span style={{ width: `${Math.min(100, entry.session.usage.cycles / entry.session.policy.maxCycles * 100)}%` }} /></div>
      <ol>{entry.plan.steps.map((step) => <li key={step.id}><i aria-hidden="true" /> <span>{step.description}</span><em>{step.status}</em></li>)}</ol>
      <footer><code>{entry.session.policy.workspaceRevision}</code><span>{entry.job.pauseReason ?? "运行边界正常"}</span></footer>
      {entry.attempt?.status === "indeterminate" && <div className="control-risk" role="alert"><strong>外部执行结果未知，禁止自动重试</strong><p>Attempt {entry.attempt.id} · Checkpoint #{entry.attempt.checkpointSequence}</p><code>{entry.attempt.requestFingerprint}</code><div><button disabled={safeMode || recoveryMode || !desktopApi.native || controlBusy} onClick={() => prepareControl({ kind: "resolve", entry, decision: "confirmed_not_executed" })} type="button">确认未执行并暂停</button><button disabled={safeMode || recoveryMode || !desktopApi.native || controlBusy} onClick={() => prepareControl({ kind: "resolve", entry, decision: "accept_external_effect_and_cancel" })} type="button">接受副作用并取消</button></div></div>}
      {!entry.attempt && ["starting", "running"].includes(entry.job.status) && <div className="control-actions"><button disabled={safeMode || recoveryMode || !desktopApi.native || controlBusy} onClick={() => prepareControl({ kind: "pause", entry })} type="button">暂停</button><button disabled={safeMode || recoveryMode || !desktopApi.native || controlBusy} onClick={() => prepareControl({ kind: "cancel", entry })} type="button">取消任务</button></div>}
      {entry.resolutions.length > 0 && <details className="resolution-history"><summary>不可变对账记录 · {entry.resolutions.length}</summary>{entry.resolutions.map((resolution) => <article key={resolution.id}><strong>{resolution.decision}</strong><p>{resolution.reason}</p><small>{resolution.actor} · {new Date(resolution.resolvedAtMs).toLocaleString("zh-CN")}</small></article>)}</details>}
    </article>) : <div className="control-empty"><strong>还没有 Auto Mode 目标</strong><p>控制中心只展示宿主持久化并验证过的事实；浏览器预览使用明确的演示数据。</p></div>}
  </section>{pendingControl && <div className="control-dialog-backdrop"><section aria-labelledby="control-dialog-title" aria-modal="true" className={pendingControl.kind === "pause" ? "control-dialog" : "control-dialog danger"} role="dialog"><p className="card-label">参数绑定确认</p><h3 id="control-dialog-title">{pendingControl.kind === "pause" ? "暂停这个任务？" : pendingControl.kind === "cancel" ? "取消这个任务？" : pendingControl.decision === "confirmed_not_executed" ? "确认外部操作未执行？" : "接受潜在副作用并取消？"}</h3><p>{pendingControl.kind === "pause" ? "任务会在当前原子步骤结束后暂停，不会丢弃 Checkpoint。" : pendingControl.kind === "cancel" ? "取消不可恢复为同一个运行；已产生的外部副作用不会自动回滚。" : "此决议将绑定当前 Attempt、Checkpoint 与请求指纹，并永久写入审计记录。"}</p><dl><div><dt>Goal</dt><dd>{pendingControl.entry.goal.title}</dd></div><div><dt>Session</dt><dd><code>{pendingControl.entry.session.id}</code></dd></div>{pendingControl.kind === "resolve" && <div><dt>Attempt</dt><dd><code>{pendingControl.entry.attempt?.id}</code></dd></div>}</dl>{pendingControl.kind === "resolve" && <label><span>对账理由（必填）</span><textarea autoFocus maxLength={2048} onChange={(event) => setControlReason(event.target.value)} placeholder="说明你核对了什么证据，以及为什么选择此决议" value={controlReason} /></label>}<div><button disabled={controlBusy} onClick={() => setPendingControl(null)} type="button">返回检查</button><button className="primary-button" disabled={controlBusy || (pendingControl.kind === "resolve" && !controlReason.trim())} onClick={() => void executeControl()} type="button">{controlBusy ? "提交中…" : "确认提交"}</button></div></section></div>}</>;

  if (view === "history") return <><header className="agent-section-header"><div><p className="card-label">LOCAL EXECUTION HISTORY</p><h2>本机记录，清晰、私密、可删除。</h2></div></header>{tabs}<section className="history-page" aria-labelledby="history-page-heading"><div><h3 id="history-page-heading">最近完成</h3><button disabled={busy || !history?.records.length} onClick={() => void clearHistory()} type="button">清除全部</button></div>{history?.records.length ? history.records.map((record) => <article key={record.task.id}><strong>{record.prompt}</strong><p>{record.response || "任务已完成，无文本回答"}</p><small>{record.model} · {record.usage.inputTokens + record.usage.outputTokens} tokens · {new Date(record.completedAtMs).toLocaleString("zh-CN")}</small></article>) : <p>完成一次任务后，记录会安全保存在本机。</p>}</section></>;

  if (view === "providers") return <>{tabs}<Suspense fallback={<div className="provider-page-loading" role="status">正在载入安全 Provider 管理器…</div>}><ProviderSettings disabled={safeMode || recoveryMode} onCatalogChanged={() => void refreshCatalog()} onNotice={onNotice} /></Suspense></>;

  return <>{tabs}<section className="agent-workspace" aria-labelledby="agent-heading">
    <div className="agent-main">
      <header className="agent-hero">
        <div>
          <p className="card-label">LOCAL AGENT WORKSPACE</p>
          <h2 id="agent-heading">把想法交给 Nimora，也看清每一步。</h2>
          <p>本地 Provider 可离线运行；无论选择哪种模型，模块操作都只能经过工具目录、风险确认与 Capability Gateway。</p>
        </div>
        <span className="agent-online"><i /> 本地离线</span>
      </header>

      <div className="conversation-surface">
        <div className="agent-message system-message"><span>✦</span><div><strong>Nimora Agent</strong><p>我不会直接调用内部模块。任何动作都会先展示实际参数和风险。</p></div></div>
        {result?.status === "completed" && <div className="agent-message response-message"><span>AI</span><div><strong>任务已完成</strong><p>{result.content}</p><small>{agentUsageTotal(result)} tokens · ¥0 · {result.task.status}</small></div></div>}
        {result?.status === "waitingForConfirmation" && <div className="agent-turn-request">
          <div className="turn-request-heading"><span>!</span><div><strong>Agent 请求模块操作</strong><p>整组全部批准前不会执行任何写操作；拒绝任意一项会取消整组。</p></div></div>
          <div className="turn-tool-list">{result.pendingTools.map((tool) => <article key={tool.invocation.invocationId}>
            <div className="turn-tool-meta"><span>{agentRiskLabel(tool.effectiveRisk)}</span><code>{tool.invocation.toolId}</code></div>
            <pre>{JSON.stringify(tool.invocation.arguments, null, 2)}</pre>
            <small>{tool.expiresAtMs ? `确认有效至 ${new Date(tool.expiresAtMs).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}` : "等待确认"}</small>
            <div className="turn-tool-actions"><button disabled={busy || safeMode || recoveryMode} onClick={() => void resolveAgentTurnTool(tool.invocation.invocationId, false)} type="button">拒绝整组</button><button className="primary-button" disabled={busy || safeMode || recoveryMode} onClick={() => void resolveAgentTurnTool(tool.invocation.invocationId, true)} type="button">批准此项</button></div>
          </article>)}</div>
        </div>}
        {turnCancelled && <div className="agent-message cancelled-message"><span>×</span><div><strong>本轮操作已取消</strong><p>模块未收到这组写操作，Provider 也不会收到不完整的 Tool Result。</p></div></div>}
      </div>

      <form className="agent-composer" onSubmit={(event) => { event.preventDefault(); void run(); }}>
        <div className="agent-runtime-controls">
          <label><span>Provider</span><select aria-label="Agent Provider" disabled={busy} value={providerId} onChange={(event) => { const nextProviderId = event.target.value; setProviderId(nextProviderId); setModel(defaultModelForProvider(nextProviderId)); setAllowNetwork(false); setReasoningChoice("adaptive"); }}>{catalog?.providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.displayName}</option>)}</select></label>
          <label><span>模型</span><input aria-label="Agent 模型" disabled={busy} list="agent-provider-models" maxLength={128} value={model} onChange={(event) => setModel(event.target.value)} /><datalist id="agent-provider-models">{providerStatus?.models.map((item) => <option key={item.name} value={item.name} />)}</datalist></label>
          {reasoningCapabilities && <label><span>思考强度</span><select aria-label="Agent 思考强度" disabled={busy} value={reasoningChoice} onChange={(event) => setReasoningChoice(event.target.value as ReasoningChoice)}><option value="adaptive">自动 · 均衡</option><option value="cost_saver">节省 · 更快</option><option value="quality_first">极致 · 最深入</option>{reasoningCapabilities.supportedEfforts.map((effort) => <option key={effort} value={`fixed:${effort}`}>{({ minimal: "最少", low: "轻量", medium: "均衡", high: "深入", very_high: "很深入", maximum: "极致" } as const)[effort]} · 固定</option>)}</select><small>固定等级不自动降级；能力映射版本由宿主验证。</small></label>}
        </div>
        {activeProvider?.locality === "network" && <label className="agent-network-consent"><input checked={allowNetwork} disabled={busy} type="checkbox" onChange={(event) => setAllowNetwork(event.target.checked)} /><span><strong>允许本次请求联网</strong><small>任务内容将发送到你配置的第三方服务；模块能力仍受本机网关约束。</small></span></label>}
        <textarea value={prompt} maxLength={32768} onChange={(event) => setPrompt(event.target.value)} aria-label="Agent 任务内容" />
        <div><span>{providerStatus?.message ?? "正在检查 Provider"}</span><button className="primary-button" disabled={busy || !prompt.trim() || !model.trim() || providerStatus?.state !== "ready" || !providerStatus.models.some((item) => item.name === model) || (activeProvider?.locality === "network" && !allowNetwork)} type="submit">{busy ? "运行中…" : "运行任务"}</button></div>
      </form>
    </div>

    <aside className="agent-inspector" aria-label="Agent 运行检查器">
      <div className="inspector-title"><div><p className="card-label">运行检查器</p><h3>能力与边界</h3></div><span>{catalog?.tools.length ?? 0} tools</span></div>
      <div className="provider-tile"><span className="provider-glyph">⌁</span><div><strong>{activeProvider?.displayName ?? activeProviderId}</strong><p>{activeProvider?.locality === "network" ? "网络 Provider · 受数据策略约束" : "本地 Provider · 可离线运行"}{reasoningCapabilities ? ` · ${reasoningCapabilities.supportedEfforts.length} 档推理` : " · 标准推理"}</p></div><i>{providerStatusLabel(providerStatus)}</i></div>
      <div className="boundary-note"><strong>{safeMode ? "安全模式已启用" : recoveryMode ? "恢复模式已启用" : "确认策略正常"}</strong><p>写操作与外部副作用始终要求绑定实际参数的批准。</p></div>
      <div className="tool-catalog"><p className="card-label">模块工具</p>{catalog?.tools.map((tool) => <article key={tool.id}><span>{tool.effect === "read_only" ? "R" : "W"}</span><div><strong>{tool.title}</strong><code>{tool.id}</code></div><button className={tool.effect === "read_only" ? "read-only" : "approval"} disabled={toolBusy || safeMode || recoveryMode} onClick={() => void prepareTool(tool.id)} type="button">{agentToolAccessLabel(tool.effect)}</button></article>)}</div>
      <section className="agent-history" aria-labelledby="agent-history-heading">
        <div><div><p className="card-label">本机历史</p><h4 id="agent-history-heading">最近完成</h4></div><button disabled={busy || !history?.records.length} onClick={() => void clearHistory()} type="button">全部清除</button></div>
        {history?.historyDegraded && <p className="history-warning">最近一次历史写入失败；任务结果不受影响。</p>}
        {history?.records.length ? history.records.map((record) => <article key={record.task.id}><strong>{record.prompt}</strong><p>{record.response || "任务已完成，无文本回答"}</p><small>{record.model} · {record.usage.inputTokens + record.usage.outputTokens} tokens</small></article>) : <p className="history-empty">完成一次任务后，记录会保存在本机。</p>}
      </section>
      {toolResult?.requiresConfirmation && <section className="tool-confirmation" aria-labelledby="tool-confirmation-heading">
        <p className="card-label">参数绑定确认</p>
        <h4 id="tool-confirmation-heading">允许这次模块操作？</h4>
        <code>{toolResult.invocation.toolId}</code>
        <pre>{JSON.stringify(toolResult.invocation.arguments, null, 2)}</pre>
        <p>风险：{toolResult.effectiveRisk} · 仅本次 Invocation 有效 · 5 分钟后失效</p>
        <div><button className="secondary-button" disabled={toolBusy} onClick={() => void resolveTool(false)} type="button">拒绝</button><button className="primary-button" disabled={toolBusy} onClick={() => void resolveTool(true)} type="button">确认执行</button></div>
      </section>}
      {toolResult && !toolResult.requiresConfirmation && <div className="tool-complete" role="status"><strong>Gateway 执行完成</strong><p>{toolResult.invocation.toolId}</p></div>}
      {result?.usage && <div className="usage-card"><p className="card-label">最近任务</p><dl><div><dt>输入</dt><dd>{result.usage.inputTokens}</dd></div><div><dt>输出</dt><dd>{result.usage.outputTokens}</dd></div><div><dt>费用</dt><dd>0</dd></div></dl></div>}
    </aside>
  </section></>;
}
