import { useEffect, useMemo, useState } from "react";
import type { ActiveCharacterSnapshot, AssetCatalogSnapshot, AssetPackageSummary } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";

const backends = ["Sprite Atlas", "Live2D Cubism", "VRM", "glTF"] as const;
type Backend = (typeof backends)[number];

const checks = [
  { label: "Manifest 版本契约", detail: "nimora.asset/1", status: "通过" },
  { label: "标准动作回退", detail: "pet.idle · pet.click · pet.talk", status: "通过" },
  { label: "可访问性声明", detail: "动画、闪烁、音效", status: "待补充" },
  { label: "许可证与署名", detail: "未检测到 NOTICE.md", status: "待补充" },
] as const;

export function CreatorStudio() {
  const [backend, setBackend] = useState<Backend>("Sprite Atlas");
  const [skinTone, setSkinTone] = useState("#d8d0f0");
  const [draftSaved, setDraftSaved] = useState(false);
  const [catalog, setCatalog] = useState<AssetCatalogSnapshot | null>(null);
  const [catalogError, setCatalogError] = useState(false);
  const [activeCharacter, setActiveCharacter] = useState<ActiveCharacterSnapshot | null>(null);
  const [activationError, setActivationError] = useState<string | null>(null);
  const [activating, setActivating] = useState<string | null>(null);
  const completion = useMemo(() => checks.filter((check) => check.status === "通过").length, []);

  useEffect(() => {
    void Promise.all([desktopApi.assetCatalog(), desktopApi.activeCharacter()])
      .then(([nextCatalog, nextActiveCharacter]) => {
        setCatalog(nextCatalog);
        setActiveCharacter(nextActiveCharacter);
      })
      .catch(() => setCatalogError(true));
  }, []);

  async function activate(assetId: string) {
    setActivating(assetId);
    setActivationError(null);
    try {
      setActiveCharacter(await desktopApi.activateCharacter(assetId));
    } catch (error) {
      setActivationError(error instanceof Error ? error.message : "角色激活失败");
    } finally {
      setActivating(null);
    }
  }

  function saveDraft() {
    setDraftSaved(true);
    window.setTimeout(() => setDraftSaved(false), 1800);
  }

  return (
    <section className="creator-studio" aria-labelledby="creator-heading">
      <div className="creator-header">
        <div>
          <p className="card-label">CREATOR STUDIO · LOCAL DRAFT</p>
          <h2 id="creator-heading">把角色变成你的风格</h2>
          <p>所有预览在本机完成。先调整外观与渲染后端，再导出可验证的资源包。</p>
        </div>
        <button className="secondary-button" type="button" onClick={saveDraft}>
          {draftSaved ? "草稿已保存" : "保存草稿"}
        </button>
      </div>

      <div className="creator-layout">
        <div className="creator-preview" aria-label={`${backend} 角色预览`}>
          <div className="preview-grid" aria-hidden="true" />
          <div className="creator-pet" style={{ background: `linear-gradient(145deg, #fffaf0 8%, ${skinTone} 100%)` }}>
            <span className="creator-ear left" />
            <span className="creator-ear right" />
            <span className="creator-star">✦</span>
            <span className="creator-eye left" />
            <span className="creator-eye right" />
            <span className="creator-mouth" />
          </div>
          <span className="preview-badge">{backend} · 预览</span>
        </div>

        <div className="creator-controls">
          <label className="creator-field">
            <span>渲染后端</span>
            <select value={backend} onChange={(event) => setBackend(event.target.value as Backend)}>
              {backends.map((option) => <option key={option}>{option}</option>)}
            </select>
          </label>
          <label className="creator-field">
            <span>主色调</span>
            <span className="color-control"><input type="color" value={skinTone} onChange={(event) => setSkinTone(event.target.value)} /><code>{skinTone}</code></span>
          </label>
          <div className="creator-field">
            <span>动作预览</span>
            <div className="action-chips"><button type="button">idle</button><button type="button">click</button><button type="button">talk</button></div>
          </div>
          <div className="creator-note"><span>⌘</span><p>导入 Live2D / VRM 后，Nimora 仍使用统一的 Pet Runtime 动作语义。</p></div>
        </div>
      </div>

      <section className="asset-catalog" aria-labelledby="asset-catalog-heading">
        <div className="section-heading">
          <div><p className="card-label">INSTALLED ASSETS</p><h3 id="asset-catalog-heading">本机资源目录</h3></div>
          <strong>{catalog?.assets.length ?? 0} 个可用</strong>
        </div>
        {catalogError ? <p className="catalog-empty error">资源目录暂时不可读取，当前角色不受影响。</p> : null}
        {!catalogError && catalog === null ? <p className="catalog-empty">正在验证已安装资源…</p> : null}
        {catalog?.assets.length === 0 ? <p className="catalog-empty">尚未安装第三方资源，默认角色继续离线可用。</p> : null}
        {activeCharacter?.fallbackReason ? <p className="catalog-empty error">已安全回退默认角色：{activeCharacter.fallbackReason}</p> : null}
        {activationError ? <p className="catalog-empty error">{activationError}</p> : null}
        {catalog && catalog.assets.length > 0 ? <ul className="asset-list">
          {catalog.assets.map((asset) => <AssetCatalogItem asset={asset} active={activeCharacter?.assetId === asset.id} activating={activating === asset.id} key={asset.id} onActivate={activate} />)}
        </ul> : null}
        {catalog && catalog.rejected.length > 0 ? <details className="rejected-assets">
          <summary>{catalog.rejected.length} 个资源未通过健康检查</summary>
          <ul>{catalog.rejected.map((asset) => <li key={asset.directory}><strong>{asset.directory}</strong><span>{asset.reason}</span></li>)}</ul>
        </details> : null}
      </section>

      <div className="creator-checks">
        <div className="section-heading"><div><p className="card-label">PACKAGE HEALTH</p><h3>发布前检查</h3></div><strong>{completion}/{checks.length} 已通过</strong></div>
        <ul>{checks.map((check) => <li key={check.label}><span className={check.status === "通过" ? "check-icon pass" : "check-icon pending"}>{check.status === "通过" ? "✓" : "!"}</span><div><strong>{check.label}</strong><p>{check.detail}</p></div><span className="check-status">{check.status}</span></li>)}</ul>
      </div>
    </section>
  );
}

function AssetCatalogItem({ asset, active, activating, onActivate }: { asset: AssetPackageSummary; active: boolean; activating: boolean; onActivate(assetId: string): Promise<void> }) {
  const displayName = assetDisplayName(asset);
  return <li>
    <span className="asset-kind">{asset.assetType.slice(0, 1).toUpperCase()}</span>
    <div><strong>{displayName}</strong><p>{asset.id} · {asset.version}</p></div>
    {asset.assetType === "character" ? <button className="text-button" type="button" disabled={active || activating} onClick={() => void onActivate(asset.id)}>{active ? "当前角色" : activating ? "验证中…" : "设为角色"}</button> : <span className="asset-backend">{asset.rendererBackend ?? "无渲染后端"}</span>}
  </li>;
}

export function assetDisplayName(asset: AssetPackageSummary): string {
  return asset.name["zh-CN"] ?? asset.name.en ?? Object.values(asset.name)[0] ?? asset.id;
}
