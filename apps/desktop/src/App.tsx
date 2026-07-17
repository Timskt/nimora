import { useCallback, useEffect, useState } from "react";
import { ProfileManager } from "./components/ProfileManager";
import { CreatorStudio } from "./components/CreatorStudio";
import { DataProtection } from "./components/DataProtection";
import type { OutboxSnapshot } from "./platform/desktop";
import { desktopApi } from "./platform/desktop";

const navigation = ["概览", "角色", "自动化", "扩展", "活动", "设置"] as const;

export function navItemClassName(isActive: boolean): string {
  return isActive ? "nav-item active" : "nav-item";
}

export function App() {
  const [active, setActive] = useState<(typeof navigation)[number]>("概览");
  const [quiet, setQuiet] = useState(false);
  const [safeMode, setSafeMode] = useState(false);
  const [safetyBusy, setSafetyBusy] = useState(false);
  const [outbox, setOutbox] = useState<OutboxSnapshot | null>(null);
  const [notice, setNotice] = useState(desktopApi.native ? "原生运行时已连接" : "浏览器预览模式");
  const updateNotice = useCallback((message: string) => setNotice(message), []);

  useEffect(() => {
    void Promise.all([desktopApi.snapshot(), desktopApi.outboxSnapshot()]).then(([snapshot, nextOutbox]) => {
      setSafeMode(snapshot.safety.mode === "safe");
      setOutbox(nextOutbox);
    }).catch(() => {
      setNotice("运行时状态暂时不可用");
    });
  }, []);

  async function runAction(action: "celebrate" | "work") {
    await desktopApi.playAction(action);
    setNotice(action === "celebrate" ? "Aster 正在回应你" : "专注场景已启动");
  }

  async function toggleSafeMode() {
    setSafetyBusy(true);
    try {
      if (safeMode) {
        await desktopApi.exitSafeMode();
        setSafeMode(false);
        setNotice("已退出安全模式");
      } else {
        await desktopApi.enterSafeMode();
        setSafeMode(true);
        setNotice("安全模式已启用，受限操作已被阻止");
      }
    } catch {
      setNotice("安全模式切换失败，运行状态未改变");
    } finally {
      setSafetyBusy(false);
    }
  }

  return (
    <main className="app-shell">
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
        <section className={safeMode ? "runtime-card safe" : "runtime-card"} aria-label="运行状态">
          <span className={safeMode ? "status-dot safe" : "status-dot"} aria-hidden="true" />
          <div>
            <strong>{safeMode ? "安全模式" : "本地运行"}</strong>
            <p>{safeMode ? "受限操作已被运行时阻止" : "网络不是启动依赖"}</p>
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

        {active === "角色" || active === "扩展" ? <CreatorStudio /> : active === "设置" ? <DataProtection onNotice={updateNotice} /> : <div className="dashboard-grid">
          <section className="pet-stage" aria-labelledby="pet-heading">
            <div className="stage-copy">
              <span className="pill">{notice}</span>
              <h2 id="pet-heading">晚上好，我一直在这里。</h2>
              <p>所有核心能力都在本机运行。你可以先和 Aster 打个招呼，或者开启一个安静的专注场景。</p>
              <div className="stage-actions">
                <button className="primary-button" type="button" onClick={() => void runAction("celebrate")}>开始互动</button>
                <button className="secondary-button" type="button" onClick={() => void runAction("work")}>进入专注</button>
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
            <div className="metric-row"><strong>86</strong><span>/ 100</span></div>
            <div className="progress-track"><span style={{ width: "86%" }} /></div>
            <p className="supporting">精力充沛，适合轻量互动</p>
          </section>

          <section className="metric-card affinity-card">
            <p className="card-label">陪伴关系</p>
            <div className="metric-row"><strong>Lv. 3</strong><span>熟悉</span></div>
            <p className="supporting">再相处 42 分钟解锁新动作</p>
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
            <p className="card-label">快速开始</p>
            <h2>让 Aster 帮你做点什么</h2>
            <div className="quick-grid">
              <button type="button"><span>◷</span>开始计时</button>
              <button type="button" onClick={() => void runAction("celebrate")}><span>✦</span>播放动作</button>
              <button type="button"><span>⌘</span>运行命令</button>
              <button type="button"><span>＋</span>添加能力</button>
            </div>
          </section>

          <ProfileManager safeMode={safeMode} onNotice={updateNotice} />
        </div>}
      </section>
    </main>
  );
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
