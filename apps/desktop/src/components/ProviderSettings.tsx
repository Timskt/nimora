import { useEffect, useState } from "react";
import { desktopApi, type OpenAiProviderConfig, type UpsertOpenAiProviderRequest } from "../platform/desktop";

interface ProviderSettingsProps {
  disabled: boolean;
  onCatalogChanged(): void;
  onNotice(message: string): void;
}

const emptyProvider: UpsertOpenAiProviderRequest = {
  id: "provider:openai-compatible:custom",
  displayName: "自定义 AI 服务",
  baseUrl: "https://api.openai.com",
  credentialReference: "secret:provider:openai-compatible-custom",
  defaultModel: "gpt-4.1-mini",
  contextWindowTokens: 128_000,
  maxOutputTokens: 8_192,
  enabled: true,
  revision: 0,
};

export function secretReferenceForProvider(providerId: string): string {
  return `secret:${providerId.replace(/^provider:/, "provider-").replaceAll(":", "-")}`;
}

export function ProviderSettings({ disabled, onCatalogChanged, onNotice }: ProviderSettingsProps) {
  const [providers, setProviders] = useState<OpenAiProviderConfig[]>([]);
  const [draft, setDraft] = useState<UpsertOpenAiProviderRequest>(emptyProvider);
  const [credential, setCredential] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);

  async function refresh() {
    setLoading(true);
    try {
      setProviders(await desktopApi.listOpenAiProviders());
    } catch {
      onNotice("Provider 配置暂时不可用");
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { void refresh(); }, []);

  function edit(provider: OpenAiProviderConfig) {
    setSelectedId(provider.id);
    setCredential("");
    setDraft({
      id: provider.id,
      displayName: provider.displayName,
      baseUrl: provider.baseUrl,
      credentialReference: provider.credentialReference,
      defaultModel: provider.defaultModel,
      contextWindowTokens: provider.contextWindowTokens,
      maxOutputTokens: provider.maxOutputTokens,
      enabled: provider.enabled,
      revision: provider.revision,
    });
  }

  function createNew() {
    setSelectedId(null);
    setCredential("");
    setDraft(emptyProvider);
  }

  async function save() {
    if (disabled || busy) return;
    setBusy(true);
    try {
      const saved = await desktopApi.upsertOpenAiProvider({
        ...draft,
        id: draft.id.trim(),
        displayName: draft.displayName.trim(),
        baseUrl: draft.baseUrl.trim(),
        credentialReference: draft.credentialReference.trim(),
        defaultModel: draft.defaultModel?.trim() || null,
      });
      if (credential) await desktopApi.setOpenAiProviderCredential(saved.id, credential);
      setCredential("");
      setSelectedId(saved.id);
      await refresh();
      onCatalogChanged();
      onNotice("Provider 配置已安全保存");
    } catch {
      onNotice("Provider 保存失败，请检查地址、标识与版本冲突");
    } finally {
      setCredential("");
      setBusy(false);
    }
  }

  async function revokeCredential(providerId: string) {
    if (busy || !desktopApi.native) return;
    setBusy(true);
    try {
      await desktopApi.deleteOpenAiProviderCredential(providerId);
      await refresh();
      onNotice("Provider 凭据已从系统密钥库撤销");
    } catch {
      onNotice("凭据撤销失败，现有配置未改变");
    } finally {
      setBusy(false);
    }
  }

  async function remove(provider: OpenAiProviderConfig) {
    if (busy || !desktopApi.native || !window.confirm(`删除“${provider.displayName}”配置？系统凭据需单独撤销。`)) return;
    setBusy(true);
    try {
      await desktopApi.deleteOpenAiProvider(provider.id, provider.revision);
      if (selectedId === provider.id) createNew();
      await refresh();
      onCatalogChanged();
      onNotice("Provider 配置已删除");
    } catch {
      onNotice("Provider 正在使用或配置已变化，未执行删除");
    } finally {
      setBusy(false);
    }
  }

  const hostLocked = disabled || !desktopApi.native;
  return <section className="provider-settings" aria-labelledby="provider-settings-heading">
    <header className="provider-settings-hero">
      <div><p className="card-label">SECURE PROVIDER HUB</p><h2 id="provider-settings-heading">连接你的模型，同时守住数据边界。</h2><p>凭据只进入系统密钥库，模型请求由独立 Worker 发送。Nimora 不会回填或展示 API Key。</p></div>
      <button className="secondary-button" disabled={hostLocked || busy} onClick={createNew} type="button">新增 Provider</button>
    </header>
    {!desktopApi.native && <div className="control-readonly" role="status">浏览器预览不会模拟系统密钥库；请在 Nimora 桌面端管理真实 Provider。</div>}
    {disabled && desktopApi.native && <div className="control-readonly" role="status">安全或恢复模式下配置写入已锁定；你仍可撤销已有凭据。</div>}
    <div className="provider-settings-grid">
      <div className="provider-list" aria-label="已配置 Provider">
        {loading ? <p className="provider-empty">正在读取本机配置…</p> : providers.length ? providers.map((provider) => <article className={selectedId === provider.id ? "selected" : ""} key={provider.id}>
          <button className="provider-card-main" onClick={() => edit(provider)} type="button"><span className="provider-card-icon" aria-hidden="true">⌁</span><span><strong>{provider.displayName}</strong><small>{new URL(provider.baseUrl).origin}</small></span><i data-ready={provider.enabled && provider.credentialPresent}>{!provider.enabled ? "已停用" : provider.credentialPresent ? "凭据就绪" : "缺少凭据"}</i></button>
          <div><button disabled={busy || !provider.credentialPresent || !desktopApi.native} onClick={() => void revokeCredential(provider.id)} type="button">撤销凭据</button><button className="danger-link" disabled={busy || !desktopApi.native} onClick={() => void remove(provider)} type="button">删除配置</button></div>
        </article>) : <div className="provider-empty"><span>◇</span><strong>还没有网络 Provider</strong><p>你仍可完全离线使用内置模型，也可以创建一个 OpenAI-compatible 连接。</p></div>}
      </div>
      <form className="provider-editor" onSubmit={(event) => { event.preventDefault(); void save(); }}>
        <div className="provider-editor-heading"><div><p className="card-label">{selectedId ? "EDIT CONNECTION" : "NEW CONNECTION"}</p><h3>{selectedId ? "编辑安全连接" : "配置兼容服务"}</h3></div><span>{draft.enabled ? "启用" : "停用"}</span></div>
        <label><span>显示名称</span><input disabled={hostLocked || busy} maxLength={80} required value={draft.displayName} onChange={(event) => setDraft({ ...draft, displayName: event.target.value })} /></label>
        <label><span>Provider ID</span><input disabled={hostLocked || busy || Boolean(selectedId)} maxLength={128} required value={draft.id} onChange={(event) => setDraft({ ...draft, id: event.target.value, credentialReference: secretReferenceForProvider(event.target.value) })} /><small>创建后保持稳定，供 Agent、Skill 与自动化引用。</small></label>
        <label><span>API Base URL</span><input disabled={hostLocked || busy} inputMode="url" required value={draft.baseUrl} onChange={(event) => setDraft({ ...draft, baseUrl: event.target.value })} /><small>公网仅允许 HTTPS；请求数据将发送到该服务。</small></label>
        <label><span>默认模型</span><input disabled={hostLocked || busy} maxLength={128} value={draft.defaultModel ?? ""} onChange={(event) => setDraft({ ...draft, defaultModel: event.target.value })} /></label>
        <div className="provider-token-grid"><label><span>上下文窗口</span><input disabled={hostLocked || busy} min={1024} step={1024} type="number" value={draft.contextWindowTokens} onChange={(event) => setDraft({ ...draft, contextWindowTokens: Number(event.target.value) })} /></label><label><span>最大输出</span><input disabled={hostLocked || busy} min={128} step={128} type="number" value={draft.maxOutputTokens} onChange={(event) => setDraft({ ...draft, maxOutputTokens: Number(event.target.value) })} /></label></div>
        <label><span>API Key {selectedId && "（留空则保持不变）"}</span><input autoComplete="new-password" disabled={hostLocked || busy} maxLength={65_536} type="password" value={credential} onChange={(event) => setCredential(event.target.value)} /><small>提交后立即从界面状态清除，不写入 SQLite、日志或浏览器存储。</small></label>
        <label className="provider-toggle"><input checked={draft.enabled} disabled={hostLocked || busy} type="checkbox" onChange={(event) => setDraft({ ...draft, enabled: event.target.checked })} /><span><strong>允许在 Agent 中选择</strong><small>实际联网仍需在每次运行时明确授权。</small></span></label>
        <button className="primary-button" disabled={hostLocked || busy} type="submit">{busy ? "安全保存中…" : "保存 Provider"}</button>
      </form>
    </div>
  </section>;
}
