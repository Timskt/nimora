import { useEffect, useMemo, useState } from "react";
import {
  type ActivityScene,
  type SceneCategory,
  type SceneParticle,
  type ScenePropId,
  SCENE_CATEGORY_LABEL,
  SCENE_PROP_CATALOG,
  createUserActivityScene,
  hydrateScenesFromHost,
  loadActiveSceneId,
  loadStoredScenes,
  persistActiveSceneId,
  persistScenes,
  removeUserScene,
  resolveActiveScene,
  sceneTemplateForPrompt,
  updateUserScene,
} from "./activityScenes";

const PARTICLES: Array<{ id: SceneParticle; label: string }> = [
  { id: "none", label: "无粒子" },
  { id: "confetti", label: "彩带" },
  { id: "sparkles", label: "星光" },
  { id: "snow", label: "飘雪" },
  { id: "embers", label: "余烬" },
  { id: "pixels", label: "像素点" },
];

const CATEGORIES = Object.entries(SCENE_CATEGORY_LABEL) as Array<[SceneCategory, string]>;

export function ActivitySceneWorkshop({
  onActiveSceneChange,
}: {
  onActiveSceneChange?: (scene: ActivityScene | null) => void;
}) {
  const [scenes, setScenes] = useState<ActivityScene[]>(() => loadStoredScenes());
  const [activeId, setActiveId] = useState<string | null>(() => loadActiveSceneId());
  const [prompt, setPrompt] = useState("");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [category, setCategory] = useState<SceneCategory>("custom");
  const [icon, setIcon] = useState("✨");
  const [particle, setParticle] = useState<SceneParticle>("sparkles");
  const [propIds, setPropIds] = useState<ScenePropId[]>(["star"]);
  const [speechText, setSpeechText] = useState("新场景登场，陪你一起玩～");
  const [notice, setNotice] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | SceneCategory | "active">("all");

  const activeScene = useMemo(() => resolveActiveScene(scenes, activeId), [scenes, activeId]);

  useEffect(() => {
    onActiveSceneChange?.(activeScene);
  }, [activeScene, onActiveSceneChange]);

  useEffect(() => {
    let cancelled = false;
    void hydrateScenesFromHost().then((state) => {
      if (cancelled) return;
      setScenes(state.scenes);
      setActiveId(state.activeSceneId);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const visibleScenes = useMemo(() => {
    if (filter === "all") return scenes;
    if (filter === "active") return scenes.filter((scene) => scene.id === activeId);
    return scenes.filter((scene) => scene.category === filter);
  }, [scenes, filter, activeId]);

  function commit(next: ActivityScene[], nextActive = activeId) {
    setScenes(next);
    persistScenes(next);
    setActiveId(nextActive);
    persistActiveSceneId(nextActive);
    try {
      window.dispatchEvent(new CustomEvent("nimora:activity-scene-changed", { detail: { activeId: nextActive } }));
    } catch {
      /* ignore non-DOM hosts */
    }
  }

  function applyTemplateFromPrompt() {
    const template = sceneTemplateForPrompt(prompt || name || "自定义");
    setName(template.name ?? "我的活动场景");
    setDescription(template.description ?? "");
    setCategory(template.category ?? "custom");
    setIcon(template.icon ?? "✨");
    setParticle(template.particle ?? "sparkles");
    setPropIds(template.propIds ?? ["star"]);
    setSpeechText((template.speechLines ?? []).join("\n"));
    setNotice(`已根据「${prompt || "灵感"}」填充模板，可继续编辑后创建。`);
  }

  function toggleProp(id: ScenePropId) {
    setPropIds((current) =>
      current.includes(id) ? current.filter((item) => item !== id) : [...current, id].slice(0, 6),
    );
  }

  function handleCreate() {
    const scene = createUserActivityScene({
      name,
      description,
      category,
      icon,
      particle,
      propIds,
      speechLines: speechText.split("\n"),
    });
    if (!scene) {
      setNotice("场景名称不能为空，且不超过 32 字。");
      return;
    }
    commit([scene, ...scenes], scene.id);
    setNotice(`已创建并启用「${scene.name}」——灵灵会换上对应话术与道具氛围。`);
    setName("");
    setDescription("");
    setSpeechText("新场景登场，陪你一起玩～");
  }

  function handleActivate(id: string) {
    const scene = scenes.find((item) => item.id === id);
    if (!scene?.enabled) {
      setNotice("请先启用该场景。");
      return;
    }
    commit(scenes, id);
    setNotice(`灵灵已进入「${scene.name}」活动场景。`);
  }

  function handleDeactivate() {
    commit(scenes, null);
    setNotice("已退出活动场景，回到日常陪伴。");
  }

  function handleToggleEnabled(id: string) {
    const scene = scenes.find((item) => item.id === id);
    if (!scene) return;
    const next = updateUserScene(scenes, id, { enabled: !scene.enabled });
    const nextActive = !scene.enabled === false && activeId === id ? null : activeId;
    commit(next, nextActive && next.some((item) => item.id === nextActive && item.enabled) ? nextActive : null);
  }

  function handleDelete(id: string) {
    const scene = scenes.find((item) => item.id === id);
    if (!scene || scene.source !== "user") {
      setNotice("内置场景不能删除，可禁用或本地改台词。");
      return;
    }
    const next = removeUserScene(scenes, id);
    commit(next, activeId === id ? null : activeId);
    setNotice(`已删除「${scene.name}」。`);
  }

  function handleDuplicate(id: string) {
    const scene = scenes.find((item) => item.id === id);
    if (!scene) return;
    const copy = createUserActivityScene({
      name: `${scene.name} · 副本`.slice(0, 32),
      description: scene.description,
      category: scene.category === "custom" ? "custom" : scene.category,
      icon: scene.icon,
      palette: scene.palette,
      particle: scene.particle,
      propIds: scene.propIds,
      speechLines: scene.speechLines,
      moodBias: scene.moodBias,
      animationBias: scene.animationBias,
      stageDecor: scene.stageDecor,
    });
    if (!copy) return;
    commit([copy, ...scenes], copy.id);
    setNotice(`已复制为「${copy.name}」，可继续魔改。`);
  }

  return (
    <section className="scene-workshop" aria-labelledby="scene-workshop-heading">
      <header className="scene-workshop-hero">
        <div>
          <p className="eyebrow">CREATIVE WORKSHOP / 创意工坊</p>
          <h2 id="scene-workshop-heading">给灵灵做一场专属活动场景</h2>
          <p>
            世界杯之夜、峡谷出征、新春团圆、极客调试……用原创氛围包驱动桌宠话术、道具与粒子。
            本地优先，不依赖网络；不绑定任何厂商官方素材，你可以随时自定义扩展。
          </p>
        </div>
        <div className="scene-workshop-active-pill" data-active={activeScene ? "true" : "false"}>
          <span aria-hidden="true">{activeScene?.icon ?? "🌱"}</span>
          <div>
            <strong>{activeScene ? activeScene.name : "日常陪伴"}</strong>
            <small>{activeScene ? SCENE_CATEGORY_LABEL[activeScene.category] : "未启用活动场景"}</small>
          </div>
          {activeScene ? (
            <button type="button" className="text-button" onClick={handleDeactivate}>退出场景</button>
          ) : null}
        </div>
      </header>

      <div className="scene-workshop-layout">
        <section className="scene-editor card-panel" aria-labelledby="scene-editor-heading">
          <div className="section-heading">
            <div>
              <p className="card-label">新建 / 灵感</p>
              <h3 id="scene-editor-heading">场景设计器</h3>
            </div>
          </div>

          <label className="scene-field">
            <span>一句话灵感（可自动套模板）</span>
            <div className="scene-inline-row">
              <input
                value={prompt}
                onChange={(event) => setPrompt(event.target.value)}
                placeholder="例如：世界杯决赛夜 / 峡谷排位 / 春节拜年 / 通宵写 Skill"
              />
              <button type="button" className="secondary-button" onClick={applyTemplateFromPrompt}>套用模板</button>
            </div>
          </label>

          <div className="scene-editor-grid">
            <label className="scene-field">
              <span>场景名称</span>
              <input value={name} onChange={(event) => setName(event.target.value)} placeholder="我的绿茵夜" maxLength={32} />
            </label>
            <label className="scene-field">
              <span>图标</span>
              <input value={icon} onChange={(event) => setIcon(event.target.value)} maxLength={4} />
            </label>
            <label className="scene-field">
              <span>分类</span>
              <select value={category} onChange={(event) => setCategory(event.target.value as SceneCategory)}>
                {CATEGORIES.map(([id, label]) => (
                  <option key={id} value={id}>{label}</option>
                ))}
              </select>
            </label>
            <div className="scene-field">
              <span>粒子氛围</span>
              <div className="scene-particle-row" role="group" aria-label="粒子氛围">
                {PARTICLES.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    className={particle === item.id ? "scene-particle-chip selected" : "scene-particle-chip"}
                    aria-pressed={particle === item.id}
                    onClick={() => setParticle(item.id)}
                  >
                    {item.label}
                  </button>
                ))}
              </div>
            </div>
          </div>

          <label className="scene-field">
            <span>描述</span>
            <textarea value={description} onChange={(event) => setDescription(event.target.value)} rows={2} placeholder="这场活动想给灵灵什么氛围？" />
          </label>

          <fieldset className="scene-prop-field">
            <legend>道具（最多 6 个，会出现在桌宠周围）</legend>
            <div className="scene-prop-grid">
              {(Object.keys(SCENE_PROP_CATALOG) as ScenePropId[]).map((id) => {
                const meta = SCENE_PROP_CATALOG[id];
                const selected = propIds.includes(id);
                return (
                  <button
                    key={id}
                    type="button"
                    className={selected ? "scene-prop selected" : "scene-prop"}
                    aria-pressed={selected}
                    onClick={() => toggleProp(id)}
                  >
                    <span aria-hidden="true">{meta.glyph}</span>
                    <strong>{meta.label}</strong>
                  </button>
                );
              })}
            </div>
          </fieldset>

          <label className="scene-field">
            <span>灵灵台词（每行一句）</span>
            <textarea
              value={speechText}
              onChange={(event) => setSpeechText(event.target.value)}
              rows={4}
              placeholder={"进球了吗？我先热身～\n今晚一起欢呼吧！"}
            />
          </label>

          <div className="scene-editor-actions">
            <button type="button" className="primary-button" onClick={handleCreate}>创建并启用场景</button>
            {notice ? <p className="scene-notice" role="status">{notice}</p> : null}
          </div>
        </section>

        <section className="scene-gallery card-panel" aria-labelledby="scene-gallery-heading">
          <div className="section-heading">
            <div>
              <p className="card-label">场景库</p>
              <h3 id="scene-gallery-heading">内置 + 我的创作</h3>
            </div>
            <div className="scene-filter-row" role="tablist" aria-label="场景筛选">
              <button type="button" className={filter === "all" ? "chip active" : "chip"} onClick={() => setFilter("all")}>全部</button>
              <button type="button" className={filter === "active" ? "chip active" : "chip"} onClick={() => setFilter("active")}>当前</button>
              {CATEGORIES.slice(0, 4).map(([id, label]) => (
                <button key={id} type="button" className={filter === id ? "chip active" : "chip"} onClick={() => setFilter(id)}>{label}</button>
              ))}
            </div>
          </div>

          <ul className="scene-card-list">
            {visibleScenes.map((scene) => {
              const isActive = scene.id === activeId;
              return (
                <li key={scene.id} className={isActive ? "scene-card active" : "scene-card"} data-category={scene.category}>
                  <div className="scene-card-top" style={{
                    background: `linear-gradient(135deg, ${scene.palette.sky}, ${scene.palette.ground})`,
                  }}>
                    <span className="scene-card-icon" aria-hidden="true">{scene.icon}</span>
                    <div className="scene-card-props" aria-hidden="true">
                      {scene.propIds.slice(0, 4).map((id) => (
                        <i key={id}>{SCENE_PROP_CATALOG[id]?.glyph ?? "✨"}</i>
                      ))}
                    </div>
                  </div>
                  <div className="scene-card-body">
                    <div className="scene-card-title-row">
                      <strong>{scene.name}</strong>
                      <span className="scene-source">{scene.source === "builtin" ? "内置" : "我的"}</span>
                    </div>
                    <p>{scene.description}</p>
                    <div className="scene-card-meta">
                      <span>{SCENE_CATEGORY_LABEL[scene.category]}</span>
                      <span>{PARTICLES.find((item) => item.id === scene.particle)?.label ?? scene.particle}</span>
                      <span>{scene.speechLines.length} 句台词</span>
                    </div>
                    <div className="scene-card-actions">
                      <button type="button" className="primary-button" disabled={!scene.enabled || isActive} onClick={() => handleActivate(scene.id)}>
                        {isActive ? "使用中" : "启用"}
                      </button>
                      <button type="button" className="text-button" onClick={() => handleToggleEnabled(scene.id)}>
                        {scene.enabled ? "禁用" : "启用开关"}
                      </button>
                      <button type="button" className="text-button" onClick={() => handleDuplicate(scene.id)}>复制魔改</button>
                      {scene.source === "user" ? (
                        <button type="button" className="text-button danger" onClick={() => handleDelete(scene.id)}>删除</button>
                      ) : null}
                    </div>
                  </div>
                </li>
              );
            })}
          </ul>
          {visibleScenes.length === 0 ? (
            <p className="scene-empty">这一分类还没有场景，去左侧创建一个吧。</p>
          ) : null}
        </section>
      </div>
    </section>
  );
}

export type RuntimeActivityItem = {
  title: string;
  meta: string;
  tone: string;
};

export function RuntimeActivityPanel({
  outbox,
  activities = [],
}: {
  outbox: {
    pending: number;
    leased: number;
    delivered: number;
    deadLetter: number;
  } | null;
  activities?: readonly RuntimeActivityItem[];
}) {
  const queueHealthy = outbox?.deadLetter === 0;
  return (
    <section className="activity-timeline scene-runtime-panel runtime-activity-panel" aria-labelledby="activity-health-heading">
      <div className="section-heading">
        <div>
          <p className="card-label">运行记录</p>
          <h3 id="activity-health-heading">本地健康（隐私安全）</h3>
        </div>
        <span className={queueHealthy ? "healthy-tag" : "attention-tag"}>
          {outbox ? (queueHealthy ? "运行健康" : "需要查看") : "读取中"}
        </span>
      </div>
      <div className="activity-summary compact">
        <article><strong>{outbox?.pending ?? "—"}</strong><span>待投递</span></article>
        <article><strong>{outbox?.leased ?? "—"}</strong><span>处理中</span></article>
        <article><strong>{outbox?.delivered ?? "—"}</strong><span>已投递</span></article>
        <article className={outbox?.deadLetter ? "attention" : ""}><strong>{outbox?.deadLetter ?? "—"}</strong><span>需处理</span></article>
      </div>
      {activities.length > 0 ? (
        <ul className="runtime-activity-list">
          {activities.map((activity) => (
            <li key={activity.title}>
              <span className={`activity-icon ${activity.tone}`} aria-hidden="true" />
              <div>
                <strong>{activity.title}</strong>
                <p>{activity.meta}</p>
              </div>
              <span className="activity-local-badge">仅本地</span>
            </li>
          ))}
        </ul>
      ) : null}
      <p className="scene-runtime-note">只展示有界计数，不显示对话正文、路径或桌面内容。活动场景本身完全本地。</p>
    </section>
  );
}
