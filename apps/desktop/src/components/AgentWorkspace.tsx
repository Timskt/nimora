import { useEffect, useState } from "react";
import { desktopApi, type AgentCatalog, type AgentHistoryPage, type AgentProviderStatus, type AgentToolResult, type LocalAgentResult } from "../platform/desktop";

interface AgentWorkspaceProps {
  safeMode: boolean;
  recoveryMode: boolean;
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

export function AgentWorkspace({ safeMode, recoveryMode, onNotice }: AgentWorkspaceProps) {
  const [catalog, setCatalog] = useState<AgentCatalog | null>(null);
  const [prompt, setPrompt] = useState("总结一下当前可用的本地能力");
  const [providerId, setProviderId] = useState("provider:deterministic-local");
  const [model, setModel] = useState("model:echo-v1");
  const [result, setResult] = useState<LocalAgentResult | null>(null);
  const [providerStatus, setProviderStatus] = useState<AgentProviderStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [toolBusy, setToolBusy] = useState(false);
  const [toolResult, setToolResult] = useState<AgentToolResult | null>(null);
  const [turnCancelled, setTurnCancelled] = useState(false);
  const [history, setHistory] = useState<AgentHistoryPage | null>(null);
  const activeProviderId = result?.task.providerId ?? providerId;
  const activeProvider = catalog?.providers.find((provider) => provider.id === activeProviderId);

  useEffect(() => {
    void desktopApi.agentCatalog().then(setCatalog).catch(() => onNotice("Agent 工具目录暂时不可用"));
    void desktopApi.agentHistory(5).then(setHistory).catch(() => onNotice("Agent 历史暂时不可用"));
  }, [onNotice]);

  async function refreshHistory() {
    setHistory(await desktopApi.agentHistory(5));
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
    }).catch(() => current && setProviderStatus({ spec: "nimora.desktop-agent-provider-status/1", providerId, state: "unavailable", workerVerified: false, serviceReachable: false, models: [], message: "Provider 状态不可用" }));
    return () => { current = false; };
  }, [providerId]);

  async function run() {
    if (!prompt.trim() || busy) return;
    setBusy(true);
    setTurnCancelled(false);
    try {
      const next = await desktopApi.runLocalAgent(prompt.trim(), providerId, model.trim());
      setResult(next);
      if (next.status === "completed") await refreshHistory();
      onNotice("离线 Agent 任务已完成");
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
    const argumentsValue = toolId === "pet.animation.play" ? { action: "celebrate" } : toolId === "pet.position.move" ? { x: 120, y: 120 } : {};
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

  return <section className="agent-workspace" aria-labelledby="agent-heading">
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
          <label><span>Provider</span><select aria-label="Agent Provider" disabled={busy} value={providerId} onChange={(event) => { const nextProviderId = event.target.value; setProviderId(nextProviderId); setModel(defaultModelForProvider(nextProviderId)); }}>{catalog?.providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.name}</option>)}</select></label>
          <label><span>模型</span><input aria-label="Agent 模型" disabled={busy} list="agent-provider-models" maxLength={128} value={model} onChange={(event) => setModel(event.target.value)} /><datalist id="agent-provider-models">{providerStatus?.models.map((item) => <option key={item.name} value={item.name} />)}</datalist></label>
        </div>
        <textarea value={prompt} maxLength={32768} onChange={(event) => setPrompt(event.target.value)} aria-label="Agent 任务内容" />
        <div><span>{providerStatus?.message ?? "正在检查 Provider"}</span><button className="primary-button" disabled={busy || !prompt.trim() || !model.trim() || providerStatus?.state !== "ready" || !providerStatus.models.some((item) => item.name === model)} type="submit">{busy ? "运行中…" : "运行任务"}</button></div>
      </form>
    </div>

    <aside className="agent-inspector" aria-label="Agent 运行检查器">
      <div className="inspector-title"><div><p className="card-label">运行检查器</p><h3>能力与边界</h3></div><span>{catalog?.tools.length ?? 0} tools</span></div>
      <div className="provider-tile"><span className="provider-glyph">⌁</span><div><strong>{activeProvider?.name ?? activeProviderId}</strong><p>{activeProvider?.locality === "network" ? "网络 Provider · 受数据策略约束" : "本地 Provider · 可离线运行"}</p></div><i>{providerStatusLabel(providerStatus)}</i></div>
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
  </section>;
}
