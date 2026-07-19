import { useState } from "react";
import { desktopApi, type DesktopSnapshot, type PresenceOverride, type SystemContextSensorHealth } from "../platform/desktop";

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

const sensorLabels: Record<SystemContextSensorHealth["descriptor"]["kind"], string> = {
  fullscreen: "全屏感知",
  do_not_disturb: "免打扰感知",
  game: "游戏会话感知",
  screen_share: "屏幕共享感知",
};

const availabilityLabels: Record<SystemContextSensorHealth["availability"], string> = {
  available: "运行正常",
  degraded: "暂时降级",
  unavailable: "当前不可用",
  stopped: "已安全停止",
};

const sensorOrder: ReadonlyArray<SystemContextSensorHealth["descriptor"]["kind"]> = [
  "fullscreen",
  "do_not_disturb",
  "game",
  "screen_share",
];

export interface SystemContextSensorPresentation {
  kind: SystemContextSensorHealth["descriptor"]["kind"] | "system_context";
  label: string;
  status: string;
  detail: string;
  availability: SystemContextSensorHealth["availability"];
}

export function systemContextSensorPresentations(snapshot: DesktopSnapshot): SystemContextSensorPresentation[] {
  if (snapshot.systemContextSensors.length === 0) {
    return [{
      kind: "system_context",
      label: "系统情境感知",
      status: "当前不可用",
      detail: "当前平台未报告可用的原生情境 Sensor",
      availability: "unavailable",
    }];
  }
  return [...snapshot.systemContextSensors]
    .sort((left, right) => sensorOrder.indexOf(left.descriptor.kind) - sensorOrder.indexOf(right.descriptor.kind))
    .map((sensor) => ({
      kind: sensor.descriptor.kind,
      label: sensorLabels[sensor.descriptor.kind],
      status: availabilityLabels[sensor.availability],
      detail: sensor.availability === "degraded"
        ? `连续 ${sensor.consecutiveFailures} 次采样失败，正在自动重试`
        : sensor.availability === "available"
          ? "仅处理本地布尔事实，不采集用户内容"
          : sensor.availability === "stopped"
            ? "桌面运行时已停止该 Sensor"
            : "尚未取得可靠的原生采样结果",
      availability: sensor.availability,
    }));
}

export function PresenceSettings({ snapshot, disabled, onChanged, onNotice }: PresenceSettingsProps) {
  const [busy, setBusy] = useState(false);
  const sensors = systemContextSensorPresentations(snapshot);

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
    <p>当前依据：{reasonLabels[snapshot.presenceDecision.reason]}。系统感知只处理本地布尔状态，不读取窗口标题、屏幕内容、通知正文或会议信息。</p>
    <ul className="presence-sensors" aria-label="系统情境感知健康" aria-live="polite">
      {sensors.map((sensor) => <li key={sensor.kind} data-availability={sensor.availability}>
        <span className="presence-sensor-indicator" aria-hidden="true" />
        <span><strong>{sensor.label}</strong><small>{sensor.detail}</small></span>
        <em>{sensor.status}</em>
      </li>)}
    </ul>
    <div className="presence-options" role="radiogroup" aria-label="桌宠呈现策略">
      {options.map((option) => <button type="button" role="radio" aria-checked={snapshot.presenceOverride === option.value} disabled={busy || disabled} key={option.value} onClick={() => void update(option.value)}><strong>{option.label}</strong><small>{option.detail}</small></button>)}
    </div>
  </section>;
}
