import { useMemo, useState } from "react";

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
  const completion = useMemo(() => checks.filter((check) => check.status === "通过").length, []);

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

      <div className="creator-checks">
        <div className="section-heading"><div><p className="card-label">PACKAGE HEALTH</p><h3>发布前检查</h3></div><strong>{completion}/{checks.length} 已通过</strong></div>
        <ul>{checks.map((check) => <li key={check.label}><span className={check.status === "通过" ? "check-icon pass" : "check-icon pending"}>{check.status === "通过" ? "✓" : "!"}</span><div><strong>{check.label}</strong><p>{check.detail}</p></div><span className="check-status">{check.status}</span></li>)}</ul>
      </div>
    </section>
  );
}
