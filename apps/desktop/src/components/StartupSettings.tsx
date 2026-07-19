import { useEffect, useState } from "react";
import { desktopApi } from "../platform/desktop";

export function loginLaunchStatusLabel(enabled: boolean): string {
  return enabled ? "登录后自动陪伴" : "需要时手动启动";
}

export function loginLaunchDetail(native: boolean): string {
  return native
    ? "由系统登录项管理；登录后安静唤醒桌宠与托盘，不弹出控制中心，也不会自动开启 AI、Agent、自动化、技能或网络请求。"
    : "浏览器预览只演示设置界面，不会修改系统登录项。";
}

interface StartupSettingsProps {
  onNotice(message: string): void;
}

export function StartupSettings({ onNotice }: StartupSettingsProps) {
  const [enabled, setEnabled] = useState(false);
  const [busy, setBusy] = useState(true);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    void desktopApi.loginLaunchEnabled().then((value) => {
      if (active) setEnabled(value);
    }).catch(() => {
      if (active) setFailed(true);
    }).finally(() => {
      if (active) setBusy(false);
    });
    return () => { active = false; };
  }, []);

  async function toggle() {
    if (busy) return;
    const requested = !enabled;
    setBusy(true);
    setFailed(false);
    try {
      const confirmed = await desktopApi.setLoginLaunchEnabled(requested);
      setEnabled(confirmed);
      if (confirmed !== requested) {
        setFailed(true);
        onNotice("系统未接受登录项变更，已显示实际状态");
      } else {
        onNotice(confirmed ? "已开启登录后自动启动" : "已关闭登录后自动启动");
      }
    } catch {
      setFailed(true);
      onNotice("系统登录项更新失败，原设置保持不变");
      try { setEnabled(await desktopApi.loginLaunchEnabled()); } catch { /* retain last confirmed state */ }
    } finally {
      setBusy(false);
    }
  }

  return <section className="startup-settings" aria-labelledby="startup-settings-heading">
    <div className="startup-settings-copy">
      <p className="card-label">常驻陪伴</p>
      <h2 id="startup-settings-heading">登录后自动陪伴</h2>
      <p>登录系统后自动启动 Nimora；默认关闭，可随时撤销。</p>
      <small>{loginLaunchDetail(desktopApi.native)}</small>
      {failed && <span role="alert">无法确认系统登录项，请检查系统设置后重试。</span>}
    </div>
    <button className="startup-switch" type="button" role="switch" aria-checked={enabled} aria-busy={busy} disabled={busy} onClick={() => void toggle()}>
      <i aria-hidden="true" />
      <span>{busy ? "正在确认系统状态" : loginLaunchStatusLabel(enabled)}</span>
    </button>
  </section>;
}
