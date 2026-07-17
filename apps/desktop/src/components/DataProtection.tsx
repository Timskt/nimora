import { useEffect, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import type { BackupHealth, DiagnosticReport } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";

export function formatBackupBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export interface DataProtectionProps {
  recoveryMode: boolean;
  onNotice(message: string): void;
}

export function backupActionDisabled(recoveryMode: boolean, busy: boolean): boolean {
  return recoveryMode || busy;
}

export function DataProtection({ recoveryMode, onNotice }: DataProtectionProps) {
  const [health, setHealth] = useState<BackupHealth | null>(null);
  const [diagnostic, setDiagnostic] = useState<DiagnosticReport | null>(null);
  const [includeEvents, setIncludeEvents] = useState(true);
  const [busy, setBusy] = useState(false);

  async function refresh() {
    const [nextHealth, nextDiagnostic] = await Promise.all([
      desktopApi.backupHealth(),
      desktopApi.previewDiagnosticReport(),
    ]);
    setHealth(nextHealth);
    setDiagnostic(nextDiagnostic);
  }

  async function exportDiagnostics() {
    if (!desktopApi.native) {
      onNotice("浏览器预览不会创建诊断包");
      return;
    }
    const destinationPath = await save({
      title: "导出 Nimora 脱敏诊断包",
      defaultPath: `nimora-${Date.now()}.nimora-diagnostics.zip`,
      filters: [{ name: "Nimora diagnostics", extensions: ["zip"] }],
    });
    if (typeof destinationPath !== "string") return;
    setBusy(true);
    try {
      const receipt = await desktopApi.exportDiagnostics(destinationPath, includeEvents);
      onNotice(receipt ? `脱敏诊断包已保存 · ${formatBackupBytes(receipt.bytes)}` : "诊断包未写入");
    } catch {
      onNotice("诊断包导出失败，目标文件未被覆盖");
    } finally {
      setBusy(false);
    }
  }

  useEffect(() => {
    void refresh().catch(() => onNotice("备份健康状态暂时不可用"));
  }, [onNotice]);

  async function createBackup() {
    setBusy(true);
    try {
      const record = await desktopApi.createBackup();
      await refresh();
      onNotice(record ? "本地一致性备份已验证并保存" : "浏览器预览不会写入备份");
    } catch {
      onNotice("备份失败，现有数据和旧备份未改变");
    } finally {
      setBusy(false);
    }
  }

  async function requestRestore(backupId: string) {
    if (!window.confirm("恢复会在下次启动前替换当前本地状态。继续吗？")) return;
    setBusy(true);
    try {
      await desktopApi.requestDatabaseRestore(backupId);
      await refresh();
      onNotice("恢复请求已验证；完全退出并重新启动 Nimora 后生效");
    } catch {
      onNotice("恢复请求未通过验证，当前数据库未改变");
    } finally {
      setBusy(false);
    }
  }

  return <section className="data-protection" aria-labelledby="data-protection-heading">
    <div className="section-heading">
      <div>
        <p className="card-label">本地数据保护</p>
        <h2 id="data-protection-heading">自动备份与安全恢复</h2>
      </div>
      <button className="primary-button" type="button" disabled={backupActionDisabled(recoveryMode, busy)} onClick={() => void createBackup()}>
        {recoveryMode ? "恢复模式下暂停备份" : busy ? "处理中…" : "立即备份"}
      </button>
    </div>
    <p className="supporting">{recoveryMode
      ? "自动备份已暂停，避免读取或覆盖不可用的主数据库；现有已验证备份仍可用于恢复。"
      : "每 15 分钟检查调度，距上次备份满 6 小时后执行；最多保留 12 份已验证备份。"}</p>
    {health?.pendingRestore && <p className="backup-pending" role="status">已安排恢复：{health.pendingRestore}</p>}
    {health?.lastError && <p className="backup-error" role="alert">自动备份失败：{health.lastError}</p>}
    <div className="backup-list">
      {!health && <p className="supporting">正在读取本地备份…</p>}
      {health?.available.length === 0 && <p className="supporting">尚无可恢复备份。主数据库仍被保留，请勿手动覆盖；请先导出脱敏诊断包，再进入人工数据提取流程。</p>}
      {health?.available.map((record, index) => <article className="backup-row" key={record.id}>
        <div>
          <strong>{index === 0 ? "最新备份" : "历史备份"}</strong>
          <p>{new Date(record.createdAtMs).toLocaleString()} · {formatBackupBytes(record.bytes)}</p>
        </div>
        <button className="secondary-button" type="button" disabled={busy || health.pendingRestore === record.id} onClick={() => void requestRestore(record.id)}>
          {health.pendingRestore === record.id ? "等待重启" : "恢复此备份"}
        </button>
      </article>)}
    </div>
    <section className="diagnostic-panel" aria-labelledby="diagnostic-heading">
      <div>
        <p className="card-label">支持与诊断</p>
        <h3 id="diagnostic-heading">导出前内容预览</h3>
        <p>摘要始终包含版本、运行模式和健康计数；结构化事件可由你取消。</p>
      </div>
      <label className="diagnostic-source-option">
        <input type="checkbox" checked={includeEvents} disabled={!diagnostic?.sources.eventCount} onChange={(event) => setIncludeEvents(event.target.checked)} />
        <span><strong>本次运行的结构化事件</strong><small>{diagnostic?.sources.eventCount ?? 0} 条 · 仅固定事件码和时间，不含文本</small></span>
      </label>
      <ul className="privacy-list" aria-label="诊断包隐私边界">
        <li>不含密钥</li><li>不含用户正文</li><li>不含文件路径</li><li>不会自动上传</li>
      </ul>
      {diagnostic && <p className="diagnostic-summary">{diagnostic.system.os} · {diagnostic.system.architecture} · Schema {diagnostic.dataProtection.databaseSchema} · {diagnostic.runtime.startupMode === "recovery" ? "恢复模式" : "正常模式"}</p>}
      <button className="secondary-button" type="button" disabled={busy || !diagnostic} onClick={() => void exportDiagnostics()}>导出脱敏诊断包</button>
    </section>
  </section>;
}
