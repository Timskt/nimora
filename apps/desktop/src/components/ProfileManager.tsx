import type { CareNeedsMode, ProfileMode, ProfilePolicy, ProfileSnapshot } from "@nimora/schemas";
import { useEffect, useState } from "react";
import { desktopApi } from "../platform/desktop";

const initialPolicy: ProfilePolicy = {
  mode: "companion",
  alwaysOnTop: true,
  clickThrough: false,
  edgeSnap: true,
  soundEnabled: true,
  proactiveFrequency: 25,
  careNeedsMode: "full",
  quietHours: { enabled: false, startMinute: 1_320, endMinute: 420 },
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

export function proactiveFrequencyLabel(value: number): string {
  return value === 0 ? "关闭自主互动" : `主动频率 ${value}%`;
}

export function careNeedsModeLabel(mode: CareNeedsMode | undefined): string {
  return ({ full: "低压力完整照料", simple: "简化照料", off: "生命衰减关闭" } as const)[mode ?? "full"];
}

export function minuteToTime(value: number): string {
  const minute = Math.max(0, Math.min(1_439, Math.trunc(value)));
  return `${String(Math.floor(minute / 60)).padStart(2, "0")}:${String(minute % 60).padStart(2, "0")}`;
}

export function timeToMinute(value: string): number | null {
  const match = /^(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  return hour < 24 && minute < 60 ? hour * 60 + minute : null;
}

export function quietHoursLabel(policy: ProfilePolicy): string {
  const quiet = policy.quietHours;
  return quiet?.enabled
    ? `安静时段 ${minuteToTime(quiet.startMinute)}–${minuteToTime(quiet.endMinute)}`
    : "无安静时段";
}

export function profileModeGuidance(mode: ProfileMode): string | null {
  if (mode === "presentation") return "切换后桌宠会自动隐藏并暂停自主互动，可从系统托盘恢复。";
  if (mode === "focus") return "此场景会暂停自主互动，手动互动仍然可用。";
  return null;
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
    if (policy.quietHours?.enabled && policy.quietHours.startMinute === policy.quietHours.endMinute) {
      onNotice("安静时段的开始与结束时间不能相同");
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
                  {profile.policy.edgeSnap === false ? " · 自由摆放" : " · 桌边吸附"}
                  {profile.policy.mode === "presentation" ? " · 桌宠隐藏" : ""}
                  {` · ${proactiveFrequencyLabel(profile.policy.proactiveFrequency ?? 25)}`}
                  {` · ${careNeedsModeLabel(profile.policy.careNeedsMode)}`}
                  {` · ${quietHoursLabel(profile.policy)}`}
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
              checked={policy.edgeSnap ?? true}
              onChange={(event) => setPolicy({ ...policy, edgeSnap: event.target.checked })}
            />
            拖动后吸附桌面边缘
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
            <span>主动互动频率 <strong>{proactiveFrequencyLabel(policy.proactiveFrequency ?? 25)}</strong></span>
            <input
              aria-label="主动互动频率"
              type="range"
              min="0"
              max="100"
              value={policy.proactiveFrequency ?? 25}
              onChange={(event) => setPolicy({ ...policy, proactiveFrequency: Number(event.target.value) })}
            />
            {profileModeGuidance(policy.mode) && <small>{profileModeGuidance(policy.mode)}</small>}
          </label>
          <fieldset className="profile-quiet-hours">
            <legend>安静时段</legend>
            <label className="profile-check">
              <input
                type="checkbox"
                checked={policy.quietHours?.enabled ?? false}
                onChange={(event) => setPolicy({
                  ...policy,
                  quietHours: {
                    enabled: event.target.checked,
                    startMinute: policy.quietHours?.startMinute ?? 1_320,
                    endMinute: policy.quietHours?.endMinute ?? 420,
                  },
                })}
              />
              在指定时间暂停自主互动
            </label>
            <div className="profile-time-grid">
              <label>
                <span>开始</span>
                <input
                  aria-label="安静时段开始"
                  type="time"
                  value={minuteToTime(policy.quietHours?.startMinute ?? 1_320)}
                  disabled={!policy.quietHours?.enabled}
                  onChange={(event) => {
                    const minute = timeToMinute(event.target.value);
                    if (minute !== null) setPolicy({ ...policy, quietHours: { enabled: true, startMinute: minute, endMinute: policy.quietHours?.endMinute ?? 420 } });
                  }}
                />
              </label>
              <label>
                <span>结束</span>
                <input
                  aria-label="安静时段结束"
                  type="time"
                  value={minuteToTime(policy.quietHours?.endMinute ?? 420)}
                  disabled={!policy.quietHours?.enabled}
                  onChange={(event) => {
                    const minute = timeToMinute(event.target.value);
                    if (minute !== null) setPolicy({ ...policy, quietHours: { enabled: true, startMinute: policy.quietHours?.startMinute ?? 1_320, endMinute: minute } });
                  }}
                />
              </label>
            </div>
            <small>支持跨午夜；只暂停桌宠主动走动和提醒，不影响手动互动、照料与生命状态。</small>
          </fieldset>
          <label className="profile-name">
            <span>照料强度</span>
            <select
              aria-label="照料强度"
              value={policy.careNeedsMode ?? "full"}
              onChange={(event) => setPolicy({ ...policy, careNeedsMode: event.target.value as CareNeedsMode })}
            >
              <option value="full">完整照料：四项需求自然演化</option>
              <option value="simple">简化照料：仅精力与心情变化</option>
              <option value="off">关闭衰减：暂停时间驱动变化</option>
            </select>
            <small>所有模式都不会死亡、倒扣关系或惩罚离线；切换不会删除当前状态，喂食、玩耍和清洁始终可用。</small>
          </label>
          <button className="primary-button" type="submit" disabled={busy}>保存 Profile</button>
        </form>
      )}
    </section>
  );
}
