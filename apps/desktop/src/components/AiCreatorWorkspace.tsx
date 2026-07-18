import { useEffect, useState } from "react";
import { desktopApi, type AgentCatalog, type AgentProviderStatus, type CreatorArtifactKind, type CreatorDraftApprovalReceipt, type CreatorDraftCheckReport, type CreatorDraftResult } from "../platform/desktop";
import { CapabilityProposalGovernance } from "./CapabilityProposalGovernance";

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
  const [proposalNotice, setProposalNotice] = useState<string | null>(null);
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

  function resetReview() {
    setResult(null);
    setCheckReport(null);
    setApproval(null);
    setSaveNotice(null);
    setInstallNotice(null);
    setProposalNotice(null);
  }

  async function generate() {
    setBusy(true);
    setError(null);
    resetReview();
    try { setResult(await desktopApi.generateCreatorDraft(kind, requirement.trim(), providerId, model.trim())); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "AI 草案生成失败"); }
    finally { setBusy(false); }
  }

  async function checkDraft() {
    if (!result?.draft) return;
    setBusy(true); setError(null); setSaveNotice(null); setApproval(null);
    try { setCheckReport(await desktopApi.checkCreatorDraft(kind, requirement.trim(), result.draft)); }
    catch (reason) { setCheckReport(null); setError(reason instanceof Error ? reason.message : "草案检查失败"); }
    finally { setBusy(false); }
  }

  async function approveDraft() {
    if (!result?.draft || checkReport?.status !== "passed") return;
    setBusy(true); setError(null); setSaveNotice(null); setApproval(null);
    try { setApproval(await desktopApi.approveCreatorDraft(kind, requirement.trim(), result.draft, checkReport.draftDigest)); }
    catch (reason) { setError(reason instanceof Error ? reason.message : "草案批准失败"); }
    finally { setBusy(false); }
  }

  async function saveDraft() {
    if (!result?.draft) return;
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
    if (!result?.draft || !approval || kind === "automation") return;
    setBusy(true); setError(null); setSaveNotice(null); setInstallNotice(null);
    try {
      const receipt = await desktopApi.installCreatorDraft(kind, requirement.trim(), result.draft, approval.approvalId);
      setInstallNotice(`${receipt.artifactId} ${receipt.version} 已原子安装${receipt.replacedPrevious ? "并保留上一版本" : ""}；当前未授权、未启用`);
      setApproval(null);
    } catch (reason) { setApproval(null); setError(reason instanceof Error ? reason.message : "草案安装失败，请重新审查并批准"); }
    finally { setBusy(false); }
  }

  async function saveGap() {
    if (!result?.capabilityGap) return;
    setBusy(true); setError(null); setSaveNotice(null);
    try {
      const workspaceRoot = await desktopApi.pickDirectory("选择 Capability Gap Workspace");
      if (!workspaceRoot) return;
      const receipt = await desktopApi.saveCapabilityGap(workspaceRoot, result.capabilityGap);
      setSaveNotice(`缺口报告已原子保存到 ${receipt.relativeFile}`);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "缺口报告保存失败"); }
    finally { setBusy(false); }
  }

  async function submitProposal() {
    if (!result?.capabilityGap?.platformProposalRequired) return;
    setBusy(true); setError(null); setProposalNotice(null);
    try {
      const workspaceRoot = await desktopApi.pickDirectory("选择 Capability Proposal Workspace");
      if (!workspaceRoot) return;
      const receipt = await desktopApi.submitCapabilityProposal(workspaceRoot, result.capabilityGap);
      setProposalNotice(`提案 ${receipt.proposalId} 已进入待评审队列`);
    } catch (reason) { setError(reason instanceof Error ? reason.message : "Capability Proposal 提交失败"); }
    finally { setBusy(false); }
  }

  const draft = result?.draft ?? null;
  const gap = result?.capabilityGap ?? null;

  return <section className="ai-creator-workspace">
    <header className="ai-creator-hero"><div><p className="card-label">AI CREATOR / REVIEW PIPELINE</p><h2>把想法变成可验证的扩展草案。</h2><p>模型没有工具、文件或安装权限。能力不足时必须报告结构化缺口，不能发明命令或绕过平台。</p></div><span>Creator Agent · zero tools</span></header>
    <div className="ai-creator-layout"><form className="ai-creator-form" onSubmit={(event) => { event.preventDefault(); void generate(); }}>
      <fieldset><legend>1 · 选择产物类型</legend>{artifactKinds.map((item) => <button className={kind === item.kind ? "selected" : ""} key={item.kind} onClick={() => { setKind(item.kind); resetReview(); }} type="button"><strong>{item.title}</strong><span>{item.detail}</span></button>)}</fieldset>
      <label><span>2 · 描述目标与验收标准</span><textarea onChange={(event) => { setRequirement(event.target.value); resetReview(); }} placeholder="例如：专注计时结束时，让角色庆祝并记录今日完成次数……" value={requirement} /></label>
      <div className="ai-provider-row"><label><span>Provider</span><select value={providerId} onChange={(event) => { setProviderId(event.target.value); setModel(""); resetReview(); }}>{catalog?.providers.map((provider) => <option key={provider.id} value={provider.id}>{provider.name}</option>)}</select></label><label><span>模型</span><input list="creator-models" value={model} onChange={(event) => { setModel(event.target.value); resetReview(); }} /><datalist id="creator-models">{status?.models.map((item) => <option key={item.name} value={item.name} />)}</datalist></label></div>
      <button className="primary-button" disabled={busy || disabled || !requirement.trim() || !model.trim() || status?.state !== "ready"} type="submit">{busy ? "正在生成并验证…" : "生成安全草案"}</button>
      {disabled ? <p className="ai-creator-error">安全或恢复模式下禁止生成。</p> : null}{error ? <p className="ai-creator-error">{error}</p> : null}
    </form><section className="ai-draft-preview" aria-live="polite">
      {gap && result ? <CapabilityGapPreview disabled={busy} gap={gap} onSave={() => void saveGap()} onSubmitProposal={() => void submitProposal()} proposalNotice={proposalNotice} result={result} saveNotice={saveNotice} /> : null}
      {draft ? <><div className="ai-draft-heading"><div><small>CONTRACT-VALIDATED DRAFT</small><h3>{draft.title}</h3></div><i>{installNotice ? "已安装 · 未启用" : "尚未安装"}</i></div><p>{draft.summary}</p><h4>权限说明</h4>{draft.permissionExplanations.length ? draft.permissionExplanations.map((item) => <article key={item.capability}><code>{item.capability}</code><span>{item.reason}</span></article>) : <p className="ai-empty">无需声明能力</p>}<h4>结构化产物</h4><pre>{JSON.stringify(draft.artifact, null, 2)}</pre>{checkReport ? <div className={`ai-check-report ${checkReport.status}`}><strong>{checkReport.status === "passed" ? "独立审查通过" : "独立审查未通过"}</strong>{checkReport.proposedVersion ? <span className="ai-version-diff">{checkReport.installedVersion ? `升级 ${checkReport.installedVersion} → ${checkReport.proposedVersion}` : `首次安装 ${checkReport.proposedVersion}`}{checkReport.requiresReauthorization ? " · 安装后必须重新授权" : ""}</span> : null}<span>最高风险：{checkReport.highestRisk}</span>{checkReport.permissionDiff.length ? checkReport.permissionDiff.map((item) => <span className={`ai-permission-diff ${item.risk} ${item.change}`} key={`${item.change}:${item.capability}`}><b>{item.change === "added" ? "新增" : item.change === "removed" ? "移除" : "范围变化"} · {item.risk}</b><code>{item.capability}</code>{item.reason}</span>) : <span>权限与作用域未发生变化</span>}{checkReport.checks.map((check) => <span key={`${check.id}:${check.file ?? "artifact"}`}>{check.status === "passed" ? "✓" : "!"} {check.id === "sandbox-behavior" ? "行为沙箱" : check.id === "javascript-syntax" ? "语法" : "生产契约"} · {check.file ? `${check.file} · ` : ""}{check.message}</span>)}</div> : null}{approval ? <p className="ai-approval-notice">一次性批准已签发 · {new Date(approval.expiresAtMs).toLocaleTimeString()} 前有效；保存或安装会消费它</p> : null}<div className="ai-draft-actions"><small>{result ? `${result.usage.inputTokens + result.usage.outputTokens} tokens · ${result.finishReason}` : ""}</small><div><button disabled={busy || disabled} onClick={() => void checkDraft()} type="button">{checkReport?.status === "passed" ? "重新运行审查" : "运行独立审查"}</button><button disabled={busy || disabled || checkReport?.status !== "passed" || Boolean(approval)} onClick={() => void approveDraft()} type="button">{approval ? "已批准一次" : "批准此权限与行为审查"}</button><button disabled={busy || disabled || !approval || approval.draftDigest !== checkReport?.draftDigest || Boolean(saveNotice) || Boolean(installNotice)} onClick={() => void saveDraft()} type="button">{saveNotice ? "已保存" : "保存到 Workspace"}</button><button className="ai-install-button" disabled={busy || disabled || !approval || approval.draftDigest !== checkReport?.draftDigest || Boolean(saveNotice) || Boolean(installNotice)} onClick={() => void installDraft()} type="button">{installNotice ? "已安装" : checkReport?.installedVersion ? "原子升级（重新授权）" : "原子安装（不启用）"}</button></div></div>{saveNotice ? <p className="ai-save-notice">{saveNotice}</p> : null}{installNotice ? <p className="ai-install-notice">{installNotice}</p> : null}</> : null}
      {!result ? <div className="ai-empty-state"><span>✦</span><h3>等待一份经过验证的草案</h3><p>可表达的目标会显示 Manifest 与权限；能力不足时会显示不可执行的结构化缺口。</p></div> : null}
    </section></div>
    <CapabilityProposalGovernance disabled={disabled} />
  </section>;
}

export function CapabilityGapPreview({ disabled, gap, onSave, onSubmitProposal, proposalNotice, result, saveNotice }: { disabled: boolean; gap: NonNullable<CreatorDraftResult["capabilityGap"]>; onSave: () => void; onSubmitProposal?: () => void; proposalNotice?: string | null; result: CreatorDraftResult; saveNotice: string | null }) {
  const plan = result.compositionPlan;
  const semanticPlan = result.semanticCompositionPlan;
  return <div className="ai-capability-gap"><div className="ai-draft-heading"><div><small>CAPABILITY GAP · NON-EXECUTABLE</small><h3>{gap.title}</h3></div><i>{proposalNotice ? "待平台评审" : gap.platformProposalRequired ? "需要平台提案" : "可用现有替代"}</i></div><p>{gap.summary}</p><div className="ai-gap-verification"><strong>宿主双重核验</strong><span>精确能力 ID 不存在，语义目标也未在当前约束下完全解析</span><code title={result.catalogDigest}>{result.catalogDigest.slice(0, 22)}…</code><code title={result.compositionGraphDigest}>{result.compositionGraphDigest.slice(0, 22)}…</code></div><div className="ai-gap-outcome"><span>用户目标</span><strong>{gap.requestedOutcome}</strong></div><h4>语义候选</h4><article><span>输入：{gap.availableSemanticInputs.length ? gap.availableSemanticInputs.join(" · ") : "无"}</span><span>目标：{gap.requiredSemanticOutputs.join(" · ")}</span>{semanticPlan ? <><span>宿主路径：{semanticPlan.capabilityPath.length ? semanticPlan.capabilityPath.join(" → ") : "未找到"}</span><small>成本 {semanticPlan.totalCostUnits} · 搜索 {semanticPlan.expandedStates} 状态 · 缺失 {semanticPlan.missingOutputs.join(" · ")}</small></> : null}</article><h4>缺失能力</h4>{gap.missingCapabilities.map((item) => <article key={item.capability}><code>{item.capability}</code><span>{item.reason}</span><ul>{item.requiredOperations.map((operation) => <li key={operation}>{operation}</li>)}</ul></article>)}{plan ? <small className="ai-gap-proof">已确定性核验 {plan.missingCapabilities.length} 个缺失 ID。语义结论仅覆盖模型候选映射、宿主可信前置事实与当前图，不证明自然语言映射绝对完整。</small> : null}<h4>最低成本替代方案</h4>{gap.closestAlternatives.length ? gap.closestAlternatives.map((alternative) => <article className="ai-gap-alternative" key={`${alternative.kind}:${alternative.title}`}><strong>{alternative.title}</strong><code>{alternative.kind}</code><span>{alternative.tradeoff}</span></article>) : <p className="ai-empty">当前没有不扩大权限的可靠替代方案</p>}<p className="ai-gap-boundary">平台提案只进入人工评审队列，不会创建 Handler、修改 Registry、授予权限或执行代码。</p><div className="ai-gap-footer"><small>{result.usage.inputTokens + result.usage.outputTokens} tokens · {result.finishReason}</small><button disabled={disabled} onClick={onSave} type="button">{saveNotice ? "报告已保存" : "保存缺口报告"}</button>{gap.platformProposalRequired && onSubmitProposal ? <button disabled={disabled || Boolean(proposalNotice)} onClick={onSubmitProposal} type="button">{proposalNotice ? "已提交待评审" : "提交平台能力提案"}</button> : null}</div>{saveNotice ? <p className="ai-save-notice">{saveNotice}</p> : null}{proposalNotice ? <p className="ai-save-notice">{proposalNotice}</p> : null}</div>;
}
