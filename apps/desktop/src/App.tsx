import { useCallback, useEffect, useState, type CSSProperties } from "react";
import { ProfileManager } from "./components/ProfileManager";
import { LazyWorkspace } from "./components/LazyWorkspace";
import { petItemPresentation } from "./components/petItems";
import { petGrowth } from "./components/petGrowth";
import type { ActiveThemeSnapshot, AssetPreviewAudio, DesktopSnapshot, OutboxSnapshot, PetCareAction, PetItemId, ThemeDescriptor } from "./platform/desktop";
import { desktopApi } from "./platform/desktop";

const loadCreatorStudio = () => import("./components/CreatorStudio").then((module) => ({ default: module.CreatorStudio }));
const loadAgentWorkspace = () => import("./components/AgentWorkspace").then((module) => ({ default: module.AgentWorkspace }));
const loadAutomationWorkspace = () => import("./components/AutomationWorkspace").then((module) => ({ default: module.AutomationWorkspace }));
const loadAiCreatorWorkspace = () => import("./components/AiCreatorWorkspace").then((module) => ({ default: module.AiCreatorWorkspace }));
const loadDataProtection = () => import("./components/DataProtection").then((module) => ({ default: module.DataProtection }));

export const navigation = ["概览", "角色", "Agent", "自动化", "扩展", "活动", "设置"] as const;

export function navItemClassName(isActive: boolean): string {
  return isActive ? "nav-item active" : "nav-item";
}

export function voiceGain(gainDb: number): number {
  return Math.min(1, Math.max(0, 10 ** (gainDb / 20)));
}

const keepsakeMetadata = {
  first_hello: { glyph: "✦", label: "第一次回应" },
  caring_hands: { glyph: "♡", label: "温柔照料" },
  trusted_companion: { glyph: "◇", label: "可信伙伴" },
  hundred_moments: { glyph: "✺", label: "百刻相伴" },
} as const;

export function keepsakePresentation(id: keyof typeof keepsakeMetadata) {
  return keepsakeMetadata[id];
}

export function itemPresentation(id: PetItemId) {
  return petItemPresentation(id);
}

async function playVoiceCue(cue: "pet.celebrate" | "pet.work", quiet: boolean): Promise<void> {
  if (quiet) return;
  const clip = await desktopApi.activeVoiceClip(cue);
  if (!clip) return;
  await playVerifiedAudio(clip);
}

async function playVerifiedAudio(clip: AssetPreviewAudio): Promise<void> {
  const url = URL.createObjectURL(new Blob([new Uint8Array(clip.bytes)], { type: clip.mediaType }));
  const audio = new Audio(url);
  audio.volume = voiceGain(clip.gainDb);
  const release = () => URL.revokeObjectURL(url);
  audio.addEventListener("ended", release, { once: true });
  audio.addEventListener("error", release, { once: true });
  try {
    await audio.play();
  } catch {
    release();
  }
}

export function App() {
  const [active, setActive] = useState<(typeof navigation)[number]>("概览");
  const [quiet, setQuiet] = useState(false);
  const [safeMode, setSafeMode] = useState(false);
  const [recoveryMode, setRecoveryMode] = useState(false);
  const [safetyBusy, setSafetyBusy] = useState(false);
  const [outbox, setOutbox] = useState<OutboxSnapshot | null>(null);
  const [desktopSnapshot, setDesktopSnapshot] = useState<DesktopSnapshot | null>(null);
  const [activeTheme, setActiveTheme] = useState<ActiveThemeSnapshot | null>(null);
  const [notice, setNotice] = useState(desktopApi.native ? "原生运行时已连接" : "浏览器预览模式");
  const updateNotice = useCallback((message: string) => setNotice(message), []);
  const relationship = petGrowth(desktopSnapshot?.pet.bondPoints, desktopSnapshot?.pet.affinity ?? 0);

  useEffect(() => {
    void Promise.all([desktopApi.snapshot(), desktopApi.outboxSnapshot(), desktopApi.activeTheme()]).then(([snapshot, nextOutbox, nextTheme]) => {
      setSafeMode(snapshot.safety.mode === "safe");
      setDesktopSnapshot(snapshot);
      const recovering = snapshot.startup.mode === "recovery";
      setRecoveryMode(recovering);
      if (recovering) {
        setActive("设置");
        setNotice("主数据库不可用，已进入隔离恢复模式");
      }
      setOutbox(nextOutbox);
      setActiveTheme(nextTheme);
    }).catch(() => {
      setNotice("运行时状态暂时不可用");
    });
  }, []);

  useEffect(() => {
    if (!desktopApi.native) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void desktopApi.onPetVitalsChanged(() => {
      void desktopApi.snapshot().then((snapshot) => {
        if (!disposed) setDesktopSnapshot(snapshot);
      });
    }).then((value) => {
      if (disposed) value();
      else unlisten = value;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function runAction(action: "celebrate" | "work") {
    if (recoveryMode) {
      setNotice("恢复模式下互动与自动化保持暂停");
      return;
    }
    await desktopApi.playAction(action);
    void playVoiceCue(action === "celebrate" ? "pet.celebrate" : "pet.work", quiet).catch(() => undefined);
    setNotice(action === "celebrate" ? "Aster 正在回应你" : "专注场景已启动");
  }

  async function runCare(action: PetCareAction) {
    if (recoveryMode) {
      setNotice("恢复模式下照料保持暂停");
      return;
    }
    const labels: Record<PetCareAction, string> = {
      feed: "已为 Aster 补充能量",
      play: "Aster 玩得很开心",
      groom: "Aster 已经整理好啦",
    };
    try {
      await desktopApi.carePet(action);
      setDesktopSnapshot(await desktopApi.snapshot());
      setNotice(labels[action]);
    } catch {
      setNotice("照料正在冷却，请稍后再试");
    }
  }

  async function useItem(itemId: PetItemId) {
    if (recoveryMode) {
      setNotice("恢复模式下背包保持只读");
      return;
    }
    try {
      await desktopApi.usePetItem(itemId);
      setDesktopSnapshot(await desktopApi.snapshot());
      setNotice(`已使用${itemPresentation(itemId).label}`);
    } catch {
      setNotice("道具不可用或正在冷却，请稍后再试");
    }
  }

  async function toggleSafeMode() {
    setSafetyBusy(true);
    try {
      if (safeMode) {
        await desktopApi.exitSafeMode();
        setSafeMode(false);
        setActiveTheme(await desktopApi.activeTheme());
        setNotice("已退出安全模式");
      } else {
        await desktopApi.enterSafeMode();
        setSafeMode(true);
        setActiveTheme(await desktopApi.activeTheme());
        setNotice("安全模式已启用，受限操作已被阻止");
      }
    } catch {
      setNotice("安全模式切换失败，运行状态未改变");
    } finally {
      setSafetyBusy(false);
    }
  }

  return (
    <main className={`app-shell theme-${activeTheme?.theme.cornerStyle ?? "rounded"} motion-${activeTheme?.theme.motion ?? "full"}`} data-theme-mode={activeTheme?.theme.mode ?? "light"} style={activeTheme ? themeStyle(activeTheme.theme) : undefined}>
      <aside className="sidebar" aria-label="主导航">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">✦</span>
          <span>Nimora</span>
        </div>
        <nav className="navigation">
          {navigation.map((item) => (
            <button
              className={navItemClassName(active === item)}
              key={item}
              onClick={() => setActive(item)}
              type="button"
            >
              <span className="nav-dot" aria-hidden="true" />
              {item}
            </button>
          ))}
        </nav>
        <section className={recoveryMode || safeMode ? "runtime-card safe" : "runtime-card"} aria-label="运行状态">
          <span className={recoveryMode || safeMode ? "status-dot safe" : "status-dot"} aria-hidden="true" />
          <div>
            <strong>{recoveryMode ? "数据恢复模式" : safeMode ? "安全模式" : "本地运行"}</strong>
            <p>{recoveryMode ? "主数据库未被修改" : safeMode ? "受限操作已被运行时阻止" : "网络不是启动依赖"}</p>
          </div>
        </section>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <p className="eyebrow">COMPANION SPACE</p>
            <h1>{active}</h1>
          </div>
          <div className="top-actions">
            <button
              className={safeMode ? "safety-button active" : "safety-button"}
              type="button"
              disabled={safetyBusy}
              onClick={() => void toggleSafeMode()}
              aria-pressed={safeMode}
            >
              {safeMode ? "退出安全模式" : "安全模式"}
            </button>
            <button className="quiet-toggle" type="button" onClick={() => setQuiet((value) => !value)}>
              <span className={quiet ? "toggle-track on" : "toggle-track"} aria-hidden="true">
                <span />
              </span>
              安静模式
            </button>
            <button className="avatar" type="button" aria-label="打开个人设置">SK</button>
          </div>
        </header>

        {recoveryMode && <section className="recovery-banner" role="status" aria-labelledby="recovery-heading">
          <span className="recovery-symbol" aria-hidden="true">◇</span>
          <div>
            <p className="card-label">受保护的故障启动</p>
            <h2 id="recovery-heading">数据恢复模式</h2>
            <p>主数据库保持原样，Nimora 正使用隔离的临时状态。请选择一份已验证备份，安排在完全退出并重启后恢复。</p>
          </div>
        </section>}

        {active === "角色" ? <LazyWorkspace loader={loadCreatorStudio} name="角色工作室" componentProps={{ onThemeChange: setActiveTheme }} /> : active === "扩展" ? <LazyWorkspace loader={loadAiCreatorWorkspace} name="AI 扩展工坊" componentProps={{ disabled: safeMode || recoveryMode }} /> : active === "Agent" ? <LazyWorkspace loader={loadAgentWorkspace} name="Agent 工作区" componentProps={{ safeMode, recoveryMode, onNotice: updateNotice }} /> : active === "自动化" ? <LazyWorkspace loader={loadAutomationWorkspace} name="自动化工作区" componentProps={{ disabled: safeMode || recoveryMode, onNotice: updateNotice }} /> : active === "设置" ? <LazyWorkspace loader={loadDataProtection} name={recoveryMode ? "数据恢复中心" : "设置与数据保护"} componentProps={{ recoveryMode, onNotice: updateNotice }} /> : <div className="dashboard-grid">
          <section className="pet-stage" aria-labelledby="pet-heading">
            <div className="stage-copy">
              <span className="pill">{notice}</span>
              <h2 id="pet-heading">晚上好，我一直在这里。</h2>
              <p>所有核心能力都在本机运行。你可以先和 Aster 打个招呼，或者开启一个安静的专注场景。</p>
              <div className="stage-actions">
                <button className="primary-button" type="button" disabled={recoveryMode} onClick={() => void runAction("celebrate")}>开始互动</button>
                <button className="secondary-button" type="button" disabled={recoveryMode} onClick={() => void runAction("work")}>进入专注</button>
                <button className="secondary-button" type="button" disabled={recoveryMode} onClick={() => void runCare("feed")}>喂食</button>
                <button className="secondary-button" type="button" disabled={recoveryMode} onClick={() => void runCare("play")}>玩耍</button>
                <button className="secondary-button" type="button" disabled={recoveryMode} onClick={() => void runCare("groom")}>梳理</button>
              </div>
            </div>
            <div className="pet-visual" aria-label="默认角色 Aster，当前状态平静">
              <div className="orbit orbit-one" />
              <div className="orbit orbit-two" />
              <div className="pet-shadow" />
              <div className="pet-body">
                <span className="ear left" />
                <span className="ear right" />
                <span className="face">
                  <i className="eye left" />
                  <i className="eye right" />
                  <i className="mouth" />
                </span>
              </div>
            </div>
          </section>

          <section className="metric-card energy-card">
            <p className="card-label">今日状态</p>
            <div className="metric-row"><strong>{desktopSnapshot?.pet.energy ?? 100}</strong><span>/ 100</span></div>
            <div className="progress-track"><span style={{ width: `${desktopSnapshot?.pet.energy ?? 100}%` }} /></div>
            <p className="supporting vital-summary">
              <span>心情 {desktopSnapshot?.pet.mood ?? 70}</span>
              <span>饱腹 {desktopSnapshot?.pet.satiety ?? 100}</span>
              <span>清洁 {desktopSnapshot?.pet.cleanliness ?? 100}</span>
              <span>完全离线持续演化</span>
            </p>
          </section>

          <section className="metric-card affinity-card">
            <p className="card-label">陪伴关系</p>
            <div className="metric-row"><strong>Lv. {relationship.level}</strong><span>累计陪伴 {relationship.bondPoints}</span></div>
            <div className="progress-track relationship-progress" aria-label={`当前等级进度 ${relationship.levelProgress}/${relationship.pointsPerLevel}`}>
              <span style={{ width: `${relationship.progressPercent}%` }} />
            </div>
            <p className="supporting relationship-detail">
              <span>升级进度 {relationship.levelProgress} / {relationship.pointsPerLevel}</span>
              <span>关系温度 {desktopSnapshot?.pet.affinity ?? 0} / 100</span>
            </p>
            <div className="keepsake-collection" aria-label={`已收藏 ${desktopSnapshot?.pet.keepsakes.length ?? 0} 件陪伴纪念`}>
              {(desktopSnapshot?.pet.keepsakes ?? []).map((id) => {
                const keepsake = keepsakePresentation(id);
                return <span className="keepsake-chip" key={id} title={keepsake.label}><i aria-hidden="true">{keepsake.glyph}</i>{keepsake.label}</span>;
              })}
              {(desktopSnapshot?.pet.keepsakes.length ?? 0) === 0 && <span className="keepsake-empty">第一次互动后会留下纪念</span>}
            </div>
          </section>

          <section className="activity-card">
            <div className="section-heading">
              <div><p className="card-label">最近活动</p><h2>一切运行良好</h2></div>
              <button className="text-button" type="button">查看全部</button>
            </div>
            <ul>
              {runtimeActivities(outbox).map((activity) => (
                <li key={activity.title}>
                  <span className={`activity-icon ${activity.tone}`} aria-hidden="true" />
                  <div><strong>{activity.title}</strong><p>{activity.meta}</p></div>
                  <span className="chevron" aria-hidden="true">›</span>
                </li>
              ))}
            </ul>
          </section>

          <section className="quick-card">
            <p className="card-label">随身背包</p>
            <h2>本地拥有，离线也能使用</h2>
            <div className="inventory-grid" aria-label={`背包中有 ${desktopSnapshot?.pet.inventory.length ?? 0} 种道具`}>
              {(desktopSnapshot?.pet.inventory ?? []).map((stack) => {
                const item = itemPresentation(stack.itemId);
                return (
                  <button type="button" key={stack.itemId} disabled={recoveryMode} onClick={() => void useItem(stack.itemId)}>
                    <i aria-hidden="true">{item.glyph}</i>
                    <span><strong>{item.label}</strong><small>{item.effect}</small></span>
                    <b aria-label={`剩余 ${stack.quantity} 个`}>×{stack.quantity}</b>
                  </button>
                );
              })}
              {(desktopSnapshot?.pet.inventory.length ?? 0) === 0 && <p className="inventory-empty">背包空空的。已有收藏不会过期，新的奖励也不会依赖联网。</p>}
            </div>
          </section>

          <ProfileManager safeMode={safeMode} onNotice={updateNotice} />
        </div>}
      </section>
    </main>
  );
}

export function themeStyle(theme: ThemeDescriptor): CSSProperties {
  return {
    "--canvas": theme.colors.surface,
    "--surface": theme.colors.surfaceElevated,
    "--surface-strong": theme.colors.surfaceElevated,
    "--ink": theme.colors.text,
    "--muted": theme.colors.textMuted,
    "--line": theme.colors.border,
    "--accent": theme.colors.accent,
    "--accent-soft": theme.colors.accentSoft,
    "--success": theme.colors.success,
    "--danger": theme.colors.danger,
  } as CSSProperties;
}

export function runtimeActivities(outbox: OutboxSnapshot | null) {
  if (!outbox) return [{ title: "正在读取持久事件健康", meta: "SQLite Outbox · 本地诊断", tone: "amber" }] as const;
  const queueHealthy = outbox.deadLetter === 0;
  return [
    { title: queueHealthy ? "持久事件队列健康" : `${outbox.deadLetter} 条事件需要处理`, meta: `${outbox.pending} 待投递 · ${outbox.leased} 租约中`, tone: queueHealthy ? "mint" : "amber" },
    { title: "角色资源健康", meta: "默认角色 · 本地复验", tone: "violet" },
    { title: "离线能力可用", meta: "核心运行时 · 无网络启动依赖", tone: "amber" },
  ] as const;
}
