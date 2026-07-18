import { useState } from "react";
import {
  desktopApi,
  type CapabilityProposalGovernanceItem,
  type CapabilityProposalRecord,
  type CapabilityProposalStatus,
} from "../platform/desktop";

const reviewActions: Array<{
  status: Exclude<CapabilityProposalStatus, "pending-review">;
  label: string;
}> = [
  { status: "accepted", label: "接受进入可行性分析" },
  { status: "duplicate", label: "标记为重复" },
  { status: "rejected", label: "拒绝提案" },
];

const statusLabels: Record<CapabilityProposalStatus, string> = {
  "pending-review": "待维护者评审",
  accepted: "已接受分析",
  rejected: "已拒绝",
  duplicate: "重复提案",
};

export function CapabilityProposalGovernance({ disabled }: { disabled: boolean }) {
  const [workspaceRoot, setWorkspaceRoot] = useState<string | null>(null);
  const [items, setItems] = useState<CapabilityProposalGovernanceItem[]>([]);
  const [reasons, setReasons] = useState<Record<string, string>>({});
  const [busyProposalId, setBusyProposalId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function openWorkspace() {
    setError(null);
    try {
      const selected = await desktopApi.pickDirectory("打开 Capability Proposal Workspace");
      if (!selected) return;
      setWorkspaceRoot(selected);
      setItems(await desktopApi.capabilityProposalQueue(selected));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "提案队列读取失败");
    }
  }

  async function refresh() {
    if (!workspaceRoot) return;
    setError(null);
    try {
      setItems(await desktopApi.capabilityProposalQueue(workspaceRoot));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "提案队列刷新失败");
    }
  }

  async function review(
    item: CapabilityProposalGovernanceItem,
    status: Exclude<CapabilityProposalStatus, "pending-review">,
  ) {
    if (!workspaceRoot) return;
    const { record, cluster } = item;
    const reason = reasons[record.proposalId]?.trim() ?? "";
    if (!reason) {
      setError("维护者理由不能为空");
      return;
    }
    setBusyProposalId(record.proposalId);
    setError(null);
    try {
      const reviewed = await desktopApi.reviewCapabilityProposal(
        workspaceRoot,
        record.proposalId,
        status,
        reason,
        status === "duplicate" ? cluster.canonicalProposalId : undefined,
      );
      setItems((current) => current.map((currentItem) => currentItem.record.proposalId === reviewed.proposalId ? { ...currentItem, record: reviewed } : currentItem));
    } catch (reasonValue) {
      setError(reasonValue instanceof Error ? reasonValue.message : "提案评审失败");
    } finally {
      setBusyProposalId(null);
    }
  }

  return <section className="capability-proposal-governance">
    <header>
      <div>
        <p className="card-label">MAINTAINER GOVERNANCE / INERT QUEUE</p>
        <h3>平台能力提案治理</h3>
        <p>评审确定真实缺口是否值得进入平台可行性分析，不会创建 Handler、修改 Registry、授予权限或执行代码。</p>
      </div>
      <div className="capability-proposal-toolbar">
        <button disabled={Boolean(busyProposalId)} onClick={() => void openWorkspace()} type="button">{workspaceRoot ? "切换 Workspace" : "打开提案 Workspace"}</button>
        <button disabled={!workspaceRoot || Boolean(busyProposalId)} onClick={() => void refresh()} type="button">刷新</button>
      </div>
    </header>
    {workspaceRoot ? <code className="capability-proposal-root" title={workspaceRoot}>{workspaceRoot}</code> : null}
    {error ? <p className="ai-creator-error" role="alert">{error}</p> : null}
    {workspaceRoot && items.length === 0 ? <div className="capability-proposal-empty"><span>✓</span><strong>当前没有能力提案</strong><small>队列只读取经过宿主双重核验和内容摘要校验的记录。</small></div> : null}
    <div className="capability-proposal-list">
      {items.map((item) => {
        const { record, cluster } = item;
        const pending = record.status === "pending-review";
        const canonical = cluster.canonicalProposalId === record.proposalId;
        return <article className="capability-proposal-card" data-status={record.status} key={record.proposalId}>
          <div className="capability-proposal-heading">
            <div><small>{new Date(record.submittedAtMs).toLocaleString()}</small><h4>{record.gap.title}</h4><code>{record.proposalId}</code></div>
            <div className="capability-proposal-badges"><span data-priority={cluster.triagePriority}>{cluster.triagePriority === "high" ? "高重复需求" : cluster.triagePriority === "elevated" ? "重复需求" : "单次需求"} · {cluster.occurrenceCount}</span><span>{canonical ? "规范提案" : statusLabels[record.status]}</span></div>
          </div>
          <p>{record.gap.summary}</p>
          <dl><div><dt>目标</dt><dd>{record.gap.requestedOutcome}</dd></div><div><dt>精确缺口</dt><dd>{record.compositionPlan.missingCapabilities.join(" · ")}</dd></div><div><dt>语义缺口</dt><dd>{record.semanticCompositionPlan.missingOutputs.join(" · ")}</dd></div><div><dt>候选路径</dt><dd>{record.semanticCompositionPlan.capabilityPath.join(" → ") || "未找到完整路径"}</dd></div></dl>
          <small className="capability-proposal-integrity" title={record.integrityDigest}>内容一致性已绑定 · {record.integrityDigest.slice(0, 23)}…</small>
          {cluster.occurrenceCount > 1 ? <small className="capability-proposal-cluster" title={cluster.clusterKey}>同类提案 {cluster.relatedProposalIds.length} 条 · 规范记录 {cluster.canonicalProposalId}</small> : null}
          {record.review ? <blockquote><strong>{statusLabels[record.review.status]}</strong><p>{record.review.reason}</p>{record.review.duplicateOfProposalId ? <code>重复于 {record.review.duplicateOfProposalId}</code> : null}<small>{new Date(record.review.reviewedAtMs).toLocaleString()}</small></blockquote> : null}
          {pending ? <div className="capability-proposal-review"><label><span>维护者裁决理由</span><textarea disabled={disabled || Boolean(busyProposalId)} maxLength={1024} onChange={(event) => setReasons((current) => ({ ...current, [record.proposalId]: event.target.value }))} placeholder="说明安全边界、用户价值、重复关系或不可行原因……" value={reasons[record.proposalId] ?? ""} /></label><div>{reviewActions.map((action) => <button disabled={disabled || Boolean(busyProposalId) || !(reasons[record.proposalId]?.trim()) || (action.status === "duplicate" && canonical)} key={action.status} onClick={() => void review(item, action.status)} title={action.status === "duplicate" && canonical ? "规范提案不能指向自身；请裁决同簇的非规范提案" : undefined} type="button">{busyProposalId === record.proposalId ? "正在写入裁决…" : action.label}</button>)}</div></div> : null}
        </article>;
      })}
    </div>
    <p className="capability-proposal-boundary"><strong>不可逆边界：</strong>待评审记录只能裁决一次；“已接受分析”不代表能力已实现，也不会自动生成代码、注册能力或扩大任何用户授权。</p>
  </section>;
}
