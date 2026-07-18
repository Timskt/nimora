import { useState } from "react";
import { desktopApi, type DesktopSnapshot, type PresenceOverride } from "../platform/desktop";

const options: ReadonlyArray<{ value: PresenceOverride; label: string; detail: string }> = [
  { value: "automatic", label: "自动避让", detail: "按场景与系统情境降低干扰" },
  { value: "force_visible", label: "始终显示", detail: "覆盖全屏、游戏和免打扰；共享隐私除外" },
  { value: "force_hidden", label: "始终隐藏", detail: "从托盘或安全模式恢复" },
];

export const reasonLabels: Record<DesktopSnapshot["presenceDecision"]["reason"], string> = {
  base_policy: "当前 Profile",
  user_forced_visible: "用户要求显示",
  user_forced_hidden: "用户要求隐藏",
  safe_mode_recovery: "安全恢复",
  do_not_disturb: "系统免打扰",
  fullscreen: "全屏应用",
  game: "游戏会话",
  screen_share_privacy: "屏幕共享隐私",
};

interface PresenceSettingsProps {
  snapshot: DesktopSnapshot;
  disabled: boolean;
  onChanged(snapshot: DesktopSnapshot): void;
  onNotice(message: string): void;
}

export function PresenceSettings({ snapshot, disabled, onChanged, onNotice }: PresenceSettingsProps) {
  const [busy, setBusy] = useState(false);

  async function update(value: PresenceOverride) {
    if (busy || disabled || value === snapshot.presenceOverride) return;
    setBusy(true);
    try {
      await desktopApi.setPresenceOverride(value);
      onChanged(await desktopApi.snapshot());
      onNotice(value === "automatic" ? "桌宠已恢复自动避让" : value === "force_visible" ? "桌宠会保持显示；共享隐私仍优先" : "桌宠已隐藏，可从托盘恢复");
    } catch {
      onNotice("呈现策略应用失败，窗口状态已回滚");
    } finally {
      setBusy(false);
    }
  }

  return <section className="presence-settings" aria-labelledby="presence-settings-heading">
    <div><p className="card-label">桌面呈现</p><h2 id="presence-settings-heading">什么时候陪在桌面</h2><span className={snapshot.presenceDecision.visible ? "visible" : "hidden"}>{snapshot.presenceDecision.visible ? "正在显示" : "当前隐藏"}</span></div>
    <p>当前依据：{reasonLabels[snapshot.presenceDecision.reason]}。系统感知只处理布尔状态，不读取窗口标题、屏幕内容或会议信息。</p>
    <div className="presence-options" role="radiogroup" aria-label="桌宠呈现策略">
      {options.map((option) => <button type="button" role="radio" aria-checked={snapshot.presenceOverride === option.value} disabled={busy || disabled} key={option.value} onClick={() => void update(option.value)}><strong>{option.label}</strong><small>{option.detail}</small></button>)}
    </div>
  </section>;
}
