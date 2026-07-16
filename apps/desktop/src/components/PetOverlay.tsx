import { useEffect, useState } from "react";
import type { DesktopSnapshot, PetAction } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";

export function PetOverlay() {
  const [snapshot, setSnapshot] = useState<DesktopSnapshot | null>(null);
  const [message, setMessage] = useState("正在醒来…");

  useEffect(() => {
    void desktopApi.snapshot().then((value) => {
      setSnapshot(value);
      setMessage(desktopApi.native ? "本地运行" : "浏览器预览");
    });
  }, []);

  async function play(action: PetAction) {
    setMessage(action === "celebrate" ? "今天也很棒！" : "收到");
    await desktopApi.playAction(action);
    const next = await desktopApi.snapshot();
    setSnapshot(next);
  }

  async function toggleClickThrough() {
    const enabled = !snapshot?.clickThrough;
    await desktopApi.setClickThrough(enabled);
    if (snapshot) setSnapshot({ ...snapshot, clickThrough: enabled });
    setMessage(enabled ? "已开启鼠标穿透，可从托盘恢复" : "可以互动啦");
  }

  return (
    <main className="pet-overlay" aria-label="Nimora 桌面宠物">
      <button className="overlay-drag-region" type="button" data-tauri-drag-region aria-label="拖动 Aster">
        <span className="overlay-status">{message}</span>
        <span className={`overlay-pet ${snapshot?.pet.state ?? "idle"}`} aria-hidden="true">
          <i className="overlay-ear left" /><i className="overlay-ear right" />
          <i className="overlay-star">✦</i>
          <i className="overlay-eye left" /><i className="overlay-eye right" />
          <i className="overlay-mouth" />
        </span>
        <span className="overlay-shadow" aria-hidden="true" />
      </button>
      <div className="overlay-actions" aria-label="宠物快捷操作">
        <button type="button" onClick={() => void play("celebrate")} aria-label="和 Aster 互动">✦</button>
        <button type="button" onClick={() => void play("sleep")} aria-label="让 Aster 休息">☾</button>
        <button type="button" onClick={() => void toggleClickThrough()} aria-label="切换鼠标穿透">⌁</button>
      </div>
    </main>
  );
}
