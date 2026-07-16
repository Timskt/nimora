import { useState } from "react";

const navigation = ["概览", "角色", "自动化", "扩展", "活动", "设置"] as const;

const activities = [
  { title: "专注场景已准备", meta: "本地自动化 · 刚刚", tone: "mint" },
  { title: "角色资源健康", meta: "默认角色 · 2 分钟前", tone: "violet" },
  { title: "离线能力可用", meta: "核心运行时 · 5 分钟前", tone: "amber" },
] as const;

export function navItemClassName(isActive: boolean): string {
  return isActive ? "nav-item active" : "nav-item";
}

export function App() {
  const [active, setActive] = useState<(typeof navigation)[number]>("概览");
  const [quiet, setQuiet] = useState(false);

  return (
    <main className="app-shell">
      <aside className="sidebar" aria-label="主导航">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">✦</span>
          <span>AsterPet</span>
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
        <section className="runtime-card" aria-label="运行状态">
          <span className="status-dot" aria-hidden="true" />
          <div>
            <strong>本地运行</strong>
            <p>网络不是启动依赖</p>
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
            <button className="quiet-toggle" type="button" onClick={() => setQuiet((value) => !value)}>
              <span className={quiet ? "toggle-track on" : "toggle-track"} aria-hidden="true">
                <span />
              </span>
              安静模式
            </button>
            <button className="avatar" type="button" aria-label="打开个人设置">SK</button>
          </div>
        </header>

        <div className="dashboard-grid">
          <section className="pet-stage" aria-labelledby="pet-heading">
            <div className="stage-copy">
              <span className="pill">状态 · 平静</span>
              <h2 id="pet-heading">晚上好，我一直在这里。</h2>
              <p>所有核心能力都在本机运行。你可以先和 Aster 打个招呼，或者开启一个安静的专注场景。</p>
              <div className="stage-actions">
                <button className="primary-button" type="button">开始互动</button>
                <button className="secondary-button" type="button">进入专注</button>
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
              {activities.map((activity) => (
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
              <button type="button"><span>✦</span>播放动作</button>
              <button type="button"><span>⌘</span>运行命令</button>
              <button type="button"><span>＋</span>添加能力</button>
            </div>
          </section>
        </div>
      </section>
    </main>
  );
}
