import type { ProfileMode, ProfilePolicy, ProfileSnapshot } from "@nimora/schemas";
import { useEffect, useState } from "react";
import { desktopApi } from "../platform/desktop";

const initialPolicy: ProfilePolicy = {
  mode: "companion",
  alwaysOnTop: true,
  clickThrough: false,
  soundEnabled: true,
  proactiveFrequency: 25,
};

const profileModes: ReadonlyArray<{ value: ProfileMode; label: string }> = [
  { value: "companion", label: "日常陪伴" },
  { value: "work", label: "工作" },
  { value: "focus", label: "深度专注" },
  { value: "creator", label: "创作" },
  { value: "developer", label: "开发调试" },
  { value: "presentation", label: "演示与直播" },
  { value: "offline", label: "离线优先" },
];

function profileModeLabel(mode: ProfileMode): string {
  return profileModes.find((item) => item.value === mode)?.label ?? "自定义";
}

export function normalizedProfileName(value: string): string | null {
  const name = value.trim();
  return name.length > 0 && name.length <= 64 ? name : null;
}

interface ProfileManagerProps {
  safeMode: boolean;
  onNotice(message: string): void;
}

export function ProfileManager({ safeMode, onNotice }: ProfileManagerProps) {
  const [snapshot, setSnapshot] = useState<ProfileSnapshot | null>(null);
  const [name, setName] = useState("");
  const [policy, setPolicy] = useState(initialPolicy);
  const [busy, setBusy] = useState(false);
  const [expanded, setExpanded] = useState(false);

  async function refresh() {
    setSnapshot(await desktopApi.profiles());
  }

  useEffect(() => {
    void refresh().catch(() => onNotice("Profile 暂时无法读取"));
  }, [onNotice]);

  async function createProfile() {
    const normalizedName = normalizedProfileName(name);
    if (!normalizedName) {
      onNotice("Profile 名称需要 1–64 个字符");
      return;
    }
    setBusy(true);
    try {
      await desktopApi.createProfile(normalizedName, policy);
      await refresh();
      setName("");
      setExpanded(false);
      onNotice(`Profile「${normalizedName}」已创建`);
    } catch {
      onNotice("Profile 创建失败，现有配置未改变");
    } finally {
      setBusy(false);
    }
  }

  async function switchProfile(profileId: string, profileName: string) {
    setBusy(true);
    try {
      await desktopApi.switchProfile(profileId);
      await refresh();
      onNotice(`已切换到「${profileName}」`);
    } catch {
      onNotice(safeMode ? "安全模式下不能切换 Profile" : "Profile 切换失败，窗口策略已回滚");
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="profile-card" aria-labelledby="profile-heading">
      <div className="section-heading">
        <div>
          <p className="card-label">场景配置</p>
          <h2 id="profile-heading">Profile</h2>
        </div>
        <button className="text-button" type="button" onClick={() => setExpanded((value) => !value)}>
          {expanded ? "收起" : "新建 Profile"}
        </button>
      </div>

      <div className="profile-list" aria-live="polite">
        {snapshot?.profiles.map((profile) => {
          const active = profile.id === snapshot.activeProfileId;
          return (
            <article className={active ? "profile-item active" : "profile-item"} key={profile.id}>
              <span className="profile-glyph" aria-hidden="true">{active ? "✦" : "◇"}</span>
              <div>
                <strong>{profile.name}</strong>
                <p>
                  {profileModeLabel(profile.policy.mode)}
                  {profile.policy.alwaysOnTop === false ? " · 普通窗口" : " · 保持置顶"}
                  {profile.policy.soundEnabled === false ? " · 静音" : " · 声音开启"}
                  {` · 主动频率 ${profile.policy.proactiveFrequency ?? 25}%`}
                </p>
              </div>
              <button
                type="button"
                disabled={active || busy || safeMode}
                onClick={() => void switchProfile(profile.id, profile.name)}
              >
                {active ? "使用中" : "切换"}
              </button>
            </article>
          );
        }) ?? <p className="profile-loading">正在读取本地 Profile…</p>}
      </div>

      {expanded && (
        <form className="profile-form" onSubmit={(event) => { event.preventDefault(); void createProfile(); }}>
          <label className="profile-name">
            <span>名称</span>
            <input
              value={name}
              maxLength={64}
              placeholder="例如：安静创作"
              onChange={(event) => setName(event.target.value)}
              autoFocus
            />
          </label>
          <label className="profile-name">
            <span>场景类型</span>
            <select
              value={policy.mode}
              onChange={(event) => setPolicy({ ...policy, mode: event.target.value as ProfileMode })}
            >
              {profileModes.map((mode) => (
                <option key={mode.value} value={mode.value}>{mode.label}</option>
              ))}
            </select>
          </label>
          <label className="profile-check">
            <input
              type="checkbox"
              checked={policy.alwaysOnTop ?? true}
              onChange={(event) => setPolicy({ ...policy, alwaysOnTop: event.target.checked })}
            />
            保持角色置顶
          </label>
          <label className="profile-check">
            <input
              type="checkbox"
              checked={policy.soundEnabled ?? true}
              onChange={(event) => setPolicy({ ...policy, soundEnabled: event.target.checked })}
            />
            启用声音
          </label>
          <label className="profile-frequency">
            <span>主动互动频率 <strong>{policy.proactiveFrequency}%</strong></span>
            <input
              type="range"
              min="0"
              max="100"
              value={policy.proactiveFrequency ?? 25}
              onChange={(event) => setPolicy({ ...policy, proactiveFrequency: Number(event.target.value) })}
            />
          </label>
          <button className="primary-button" type="submit" disabled={busy}>保存 Profile</button>
        </form>
      )}
    </section>
  );
}
