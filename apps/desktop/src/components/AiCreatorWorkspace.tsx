import { useEffect, useState } from "react";
import { desktopApi, type AgentCatalog, type AgentProviderStatus, type CreatorArtifactKind, type CreatorDraftApprovalReceipt, type CreatorDraftCheckReport, type CreatorDraftResult } from "../platform/desktop";

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
  const [saveNotice, setSaveNotice] = useState<string | null>(null);
  const [installNotice, setInstallNotice] = useState<string | null>(null);
  const [checkReport, setCheckReport] = useState<CreatorDraftCheckReport | null>(null);
  const [approval, setApproval] = useState<CreatorDraftApprovalReceipt | null>(null);

  useEffect(() => { void desktopApi.agentCatalog().then(setCatalog).catch(() => setError("Provider 目录暂时不可用")); }, []);
  useEffect(() => {
    setStatus(null);
    void desktopApi.agentProviderStatus(providerId).then((next) => {
      setStatus(next);
      setModel((current) => current || next.models[0]?.name || (providerId === "provider:deterministic-local" ? "model:echo-v1" : ""));
    }).catch(() => setError("Provider 状态暂时不可用"));
  }, [providerId]);

  async function generate() {
    setBusy(true); setError(null); setResult(null); setSaveNotice(null); setInstallNotice(null); setCheckReport(null); setApproval(null);
    try { setResult(await desktopApi.generateCreatorDraft(kind, requirement.trim(), providerId, model.trim())); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "AI 草案生成失败"); }
    finally { setBusy(false); }
  }

  async function checkDraft() {
    if (!result) return;
    setBusy(true); setError(null); setSaveNotice(null); setApproval(null);
    try { setCheckReport(await desktopApi.checkCreatorDraft(kind, requirement.trim(), result.draft)); }
    catch (reason) { setCheckReport(null); setError(reason instanceof Error ? reason.message : "草案检查失败"); }
    finally { setBusy(false); }
  }

  async function approveDraft() {
    if (!result || checkReport?.status !== "passed") return;
    setBusy(true); setError(null); setSaveNotice(null); setApproval(null);
    try { setApproval(await desktopApi.approveCreatorDraft(kind, requirement.trim(), result.draft, checkReport.draftDigest)); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "草案批准失败"); }
    finally { setBusy(false); }
  }

  async function saveDraft() {
    if (!result) return;
    setBusy(true); setError(null); setSaveNotice(null);
    try {
      const workspaceRoot = await desktopApi.pickDirectory("选择草案 Workspace");
      if (!workspaceRoot) return;
      if (!approval) throw new Error("请先批准当前权限与行为审查");
      const receipt = await desktopApi.saveCreatorDraft(workspaceRoot, kind, requirement.trim(), result.draft, approval.approvalId);
      setSaveNotice(`已原子保存 ${receipt.filesWritten} 个文件到 ${receipt.relativeDirectory}`);
      setApproval(null);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "草案保存失败"); }
    finally { setBusy(false); }
  }

  async function installDraft() {
    if (!result || !approval || kind === "automation") return;
    setBusy(true); setError(null); setSaveNotice(null); setInstallNotice(null);
    try {
      const receipt = await desktopApi.installCreatorDraft(kind, requirement.trim(), result.draft, approval.approvalId);
      setInstallNotice(`${receipt.artifactId} ${receipt.version} 已原子安装${receipt.replacedPrevious ? "并保留上一版本" : ""}；当前未授权、未启用`);
      setApproval(null);
    } catch (reason) { setApproval(null); setError(reason instanceof Error ? reason.message : "草案安装失败，请重新审查并批准"); }
    finally { setBusy(false); }
  }

  return <section className="ai-creator-workspace">
    <header className="ai-creator-hero"><div><p className="card-label">AI CREATOR / REVIEW PIPELINE</p><h2>把想法变成可验证的扩展草案。</h2><p>模型没有工具、文件或安装权限。草案先经过生产契约、独立语法检查和无真实副作用的行为沙箱，仍不会自动安装、启用或发布。</p></div><span>未安装</span></header>
    <div className="ai-creator-layout"><form onSubmit={(event) => { event.preventDefault(); void generate(); }}>
      <fieldset disabled={busy || disabled}><legend>选择创作类型</legend><div className="ai-kind-grid">{artifactKinds.map((item) => <button className={kind === item.kind ? "selected" : ""} key={item.kind} onClick={() => { setKind(item.kind); setResult(null); setCheckReport(null); setApproval(null); setSaveNotice(null); setInstallNotice(null); }} type="button"><strong>{item.title}</strong><small>{item.detail}</small></button>)}</div></fieldset>
      <label><span>描述你希望实现的能力</span><textarea maxLength={16384} onChange={(event) => { setRequirement(event.target.value); setCheckReport(null); setApproval(null); setSaveNotice(null); }} placeholder="例如：专注计时结束时，让角色庆祝并记录今日完成次数……" value={requirement} /></label>
      <div className="ai-provider-row"><label><span>Provider</span><select value={providerId} onChange={(event) => { setProviderId(event.target.value); setModel(""); setResult(null); setCheckReport(null); setApproval(null); setSaveNotice(null); setInstallNotice(null); }}>{catalog?.providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.name}</option>)}</select></label><label><span>模型</span><input list="creator-models" value={model} onChange={(event) => { setModel(event.target.value); setResult(null); setCheckReport(null); setApproval(null); setSaveNotice(null); setInstallNotice(null); }} /><datalist id="creator-models">{status?.models.map((item) => <option key={item.name} value={item.name} />)}</datalist></label></div>
      <button className="primary-button" disabled={busy || disabled || !requirement.trim() || !model.trim() || status?.state !== "ready"} type="submit">{busy ? "正在生成并验证…" : "生成安全草案"}</button>
      {disabled ? <p className="ai-creator-error">安全或恢复模式下禁止生成。</p> : null}{error ? <p className="ai-creator-error">{error}</p> : null}
    </form><section className="ai-draft-preview" aria-live="polite">{result ? <><div className="ai-draft-heading"><div><small>CONTRACT-VALIDATED DRAFT</small><h3>{result.draft.title}</h3></div><i>{installNotice ? "已安装 · 未启用" : "尚未安装"}</i></div><p>{result.draft.summary}</p><h4>权限说明</h4>{result.draft.permissionExplanations.length ? result.draft.permissionExplanations.map((item) => <article key={item.capability}><code>{item.capability}</code><span>{item.reason}</span></article>) : <p className="ai-empty">无需声明能力</p>}<h4>结构化产物</h4><pre>{JSON.stringify(result.draft.artifact, null, 2)}</pre>{checkReport ? <div className={`ai-check-report ${checkReport.status}`}><strong>{checkReport.status === "passed" ? "独立审查通过" : "独立审查未通过"}</strong>{checkReport.proposedVersion ? <span className="ai-version-diff">{checkReport.installedVersion ? `升级 ${checkReport.installedVersion} → ${checkReport.proposedVersion}` : `首次安装 ${checkReport.proposedVersion}`}{checkReport.requiresReauthorization ? " · 安装后必须重新授权" : ""}</span> : null}<span>最高风险：{checkReport.highestRisk}</span>{checkReport.permissionDiff.length ? checkReport.permissionDiff.map((item) => <span className={`ai-permission-diff ${item.risk} ${item.change}`} key={`${item.change}:${item.capability}`}><b>{item.change === "added" ? "新增" : item.change === "removed" ? "移除" : "范围变化"} · {item.risk}</b><code>{item.capability}</code>{item.reason}</span>) : <span>权限与作用域未发生变化</span>}{checkReport.checks.map((check) => <span key={`${check.id}:${check.file ?? "artifact"}`}>{check.status === "passed" ? "✓" : "!"} {check.id === "sandbox-behavior" ? "行为沙箱" : check.id === "javascript-syntax" ? "语法" : "生产契约"} · {check.file ? `${check.file} · ` : ""}{check.message}</span>)}</div> : null}{approval ? <p className="ai-approval-notice">一次性批准已签发 · {new Date(approval.expiresAtMs).toLocaleTimeString()} 前有效；保存或安装会消费它</p> : null}<div className="ai-draft-actions"><small>{result.usage.inputTokens + result.usage.outputTokens} tokens · {result.finishReason}</small><div><button disabled={busy || disabled} onClick={() => void checkDraft()} type="button">{checkReport?.status === "passed" ? "重新运行审查" : "运行独立审查"}</button><button disabled={busy || disabled || checkReport?.status !== "passed" || Boolean(approval)} onClick={() => void approveDraft()} type="button">{approval ? "已批准一次" : "批准此权限与行为审查"}</button><button disabled={busy || disabled || !approval || approval.draftDigest !== checkReport?.draftDigest || Boolean(saveNotice) || Boolean(installNotice)} onClick={() => void saveDraft()} type="button">{saveNotice ? "已保存" : "保存到 Workspace"}</button><button className="ai-install-button" disabled={busy || disabled || !approval || approval.draftDigest !== checkReport?.draftDigest || Boolean(saveNotice) || Boolean(installNotice)} onClick={() => void installDraft()} type="button">{installNotice ? "已安装" : checkReport?.installedVersion ? "原子升级（重新授权）" : "原子安装（不启用）"}</button></div></div>{saveNotice ? <p className="ai-save-notice">{saveNotice}</p> : null}{installNotice ? <p className="ai-install-notice">{installNotice}</p> : null}</> : <div className="ai-empty-state"><span>✦</span><h3>等待一份经过验证的草案</h3><p>生成结果会在这里展示 Manifest、权限理由、文件源码或自动化定义。</p></div>}</section></div>
  </section>;
}
