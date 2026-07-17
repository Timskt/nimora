import { useEffect, useState } from "react";
import { desktopApi, type AgentCatalog, type LocalAgentResult } from "../platform/desktop";

interface AgentWorkspaceProps {
  safeMode: boolean;
  recoveryMode: boolean;
  onNotice(message: string): void;
}

export function agentToolAccessLabel(effect: string): string {
  return effect === "read_only" ? "只读" : "需确认";
}

export function agentUsageTotal(result: LocalAgentResult): number {
  return result.usage.inputTokens + result.usage.outputTokens;
}

export function AgentWorkspace({ safeMode, recoveryMode, onNotice }: AgentWorkspaceProps) {
  const [catalog, setCatalog] = useState<AgentCatalog | null>(null);
  const [prompt, setPrompt] = useState("总结一下当前可用的本地能力");
  const [result, setResult] = useState<LocalAgentResult | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void desktopApi.agentCatalog().then(setCatalog).catch(() => onNotice("Agent 工具目录暂时不可用"));
  }, [onNotice]);

  async function run() {
    if (!prompt.trim() || busy) return;
    setBusy(true);
    try {
      const next = await desktopApi.runLocalAgent(prompt.trim());
      setResult(next);
      onNotice("离线 Agent 任务已完成");
    } catch {
      onNotice("Agent 任务失败，未执行任何模块操作");
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
          <p>当前使用零费用、本地、确定性的诊断 Provider。模块操作只能经过工具目录、风险确认与 Capability Gateway。</p>
        </div>
        <span className="agent-online"><i /> 本地离线</span>
      </header>

      <div className="conversation-surface">
        <div className="agent-message system-message"><span>✦</span><div><strong>Nimora Agent</strong><p>我不会直接调用内部模块。任何动作都会先展示实际参数和风险。</p></div></div>
        {result && <div className="agent-message response-message"><span>AI</span><div><strong>任务已完成</strong><p>{result.content}</p><small>{agentUsageTotal(result)} tokens · ¥0 · {result.task.status}</small></div></div>}
      </div>

      <form className="agent-composer" onSubmit={(event) => { event.preventDefault(); void run(); }}>
        <textarea value={prompt} maxLength={32768} onChange={(event) => setPrompt(event.target.value)} aria-label="Agent 任务内容" />
        <div><span>不会自动执行写操作</span><button className="primary-button" disabled={busy || !prompt.trim()} type="submit">{busy ? "运行中…" : "运行任务"}</button></div>
      </form>
    </div>

    <aside className="agent-inspector" aria-label="Agent 运行检查器">
      <div className="inspector-title"><div><p className="card-label">运行检查器</p><h3>能力与边界</h3></div><span>{catalog?.tools.length ?? 0} tools</span></div>
      <div className="provider-tile"><span className="provider-glyph">⌁</span><div><strong>Deterministic Local</strong><p>无网络 · 无凭据 · 零费用</p></div><i>已连接</i></div>
      <div className="boundary-note"><strong>{safeMode ? "安全模式已启用" : recoveryMode ? "恢复模式已启用" : "确认策略正常"}</strong><p>写操作与外部副作用始终要求绑定实际参数的批准。</p></div>
      <div className="tool-catalog"><p className="card-label">模块工具</p>{catalog?.tools.map((tool) => <article key={tool.id}><span>{tool.effect === "read_only" ? "R" : "W"}</span><div><strong>{tool.title}</strong><code>{tool.id}</code></div><em className={tool.effect === "read_only" ? "read-only" : "approval"}>{agentToolAccessLabel(tool.effect)}</em></article>)}</div>
      {result && <div className="usage-card"><p className="card-label">最近任务</p><dl><div><dt>输入</dt><dd>{result.usage.inputTokens}</dd></div><div><dt>输出</dt><dd>{result.usage.outputTokens}</dd></div><div><dt>费用</dt><dd>0</dd></div></dl></div>}
    </aside>
  </section>;
}
