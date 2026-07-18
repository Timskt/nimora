import { useEffect, useState } from "react";
import { desktopApi, type AgentCatalog, type AgentProviderStatus, type CreatorArtifactKind, type CreatorDraftResult } from "../platform/desktop";

const artifactKinds: Array<{ kind: CreatorArtifactKind; title: string; detail: string }> = [
  { kind: "user-program", title: "用户程序", detail: "用受限 Nimora API 编排交互与逻辑" },
  { kind: "skill", title: "Skill", detail: "创建可复用、可审查的扩展能力" },
  { kind: "automation", title: "自动化", detail: "把事件、条件与可补偿动作组合起来" },
];

export function AiCreatorWorkspace({ disabled }: { disabled: boolean }) {
  const [kind, setKind] = useState<CreatorArtifactKind>("user-program");
  const [requirement, setRequirement] = useState("");
  const [catalog, setCatalog] = useState<AgentCatalog | null>(null);
  const [providerId, setProviderId] = useState("provider:ollama-loopback");
  const [model, setModel] = useState("");
  const [status, setStatus] = useState<AgentProviderStatus | null>(null);
  const [result, setResult] = useState<CreatorDraftResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => { void desktopApi.agentCatalog().then(setCatalog).catch(() => setError("Provider 目录暂时不可用")); }, []);
  useEffect(() => {
    setStatus(null);
    void desktopApi.agentProviderStatus(providerId).then((next) => {
      setStatus(next);
      setModel((current) => current || next.models[0]?.name || (providerId === "provider:deterministic-local" ? "model:echo-v1" : ""));
    }).catch(() => setError("Provider 状态暂时不可用"));
  }, [providerId]);

  async function generate() {
    setBusy(true); setError(null); setResult(null);
    try { setResult(await desktopApi.generateCreatorDraft(kind, requirement.trim(), providerId, model.trim())); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "AI 草案生成失败"); }
    finally { setBusy(false); }
  }

  return <section className="ai-creator-workspace">
    <header className="ai-creator-hero"><div><p className="card-label">AI CREATOR / DRAFT ONLY</p><h2>把想法变成可验证的扩展草案。</h2><p>模型没有工具、文件或安装权限。所有输出先经过生产契约验证，当前只展示草案，不会写盘、启用或发布。</p></div><span>未安装</span></header>
    <div className="ai-creator-layout"><form onSubmit={(event) => { event.preventDefault(); void generate(); }}>
      <fieldset disabled={busy || disabled}><legend>选择创作类型</legend><div className="ai-kind-grid">{artifactKinds.map((item) => <button className={kind === item.kind ? "selected" : ""} key={item.kind} onClick={() => setKind(item.kind)} type="button"><strong>{item.title}</strong><small>{item.detail}</small></button>)}</div></fieldset>
      <label><span>描述你希望实现的能力</span><textarea maxLength={16384} onChange={(event) => setRequirement(event.target.value)} placeholder="例如：专注计时结束时，让角色庆祝并记录今日完成次数……" value={requirement} /></label>
      <div className="ai-provider-row"><label><span>Provider</span><select value={providerId} onChange={(event) => { setProviderId(event.target.value); setModel(""); }}>{catalog?.providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.name}</option>)}</select></label><label><span>模型</span><input list="creator-models" value={model} onChange={(event) => setModel(event.target.value)} /><datalist id="creator-models">{status?.models.map((item) => <option key={item.name} value={item.name} />)}</datalist></label></div>
      <button className="primary-button" disabled={busy || disabled || !requirement.trim() || !model.trim() || status?.state !== "ready"} type="submit">{busy ? "正在生成并验证…" : "生成安全草案"}</button>
      {disabled ? <p className="ai-creator-error">安全或恢复模式下禁止生成。</p> : null}{error ? <p className="ai-creator-error">{error}</p> : null}
    </form><section className="ai-draft-preview" aria-live="polite">{result ? <><div className="ai-draft-heading"><div><small>VALIDATED DRAFT</small><h3>{result.draft.title}</h3></div><i>尚未安装</i></div><p>{result.draft.summary}</p><h4>权限说明</h4>{result.draft.permissionExplanations.length ? result.draft.permissionExplanations.map((item) => <article key={item.capability}><code>{item.capability}</code><span>{item.reason}</span></article>) : <p className="ai-empty">无需声明能力</p>}<h4>结构化产物</h4><pre>{JSON.stringify(result.draft.artifact, null, 2)}</pre><small>{result.usage.inputTokens + result.usage.outputTokens} tokens · {result.finishReason}</small></> : <div className="ai-empty-state"><span>✦</span><h3>等待一份经过验证的草案</h3><p>生成结果会在这里展示 Manifest、权限理由、文件源码或自动化定义。</p></div>}</section></div>
  </section>;
}
