import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import type { CharacterRendererSnapshot, DesktopSnapshot, PetAction, PetCareAction } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";
import { RendererErrorBoundary } from "./RendererErrorBoundary";
import { petStateAction, SpriteRenderer } from "./SpriteRenderer";

const GltfRenderer = lazy(async () => {
  const module = await import("./GltfRenderer");
  return { default: module.GltfRenderer };
});

export function PetOverlay() {
  const [snapshot, setSnapshot] = useState<DesktopSnapshot | null>(null);
  const [renderer, setRenderer] = useState<CharacterRendererSnapshot | null>(null);
  const [rendererFailed, setRendererFailed] = useState(false);
  const [message, setMessage] = useState("正在醒来…");

  const refreshRenderer = useCallback(async () => {
    const descriptor = await desktopApi.activeCharacterRenderer();
    setRenderer(descriptor);
    setRendererFailed(false);
  }, []);

  useEffect(() => {
    let disposed = false;
    const listeners: Array<() => void> = [];
    void Promise.all([desktopApi.snapshot(), desktopApi.activeCharacterRenderer()]).then(([value, descriptor]) => {
      if (disposed) return;
      setSnapshot(value);
      setRenderer(descriptor);
      setRendererFailed(false);
      setMessage(desktopApi.native ? "本地运行" : "浏览器预览");
    }).catch(() => {
      if (disposed) return;
      setMessage("角色资源不可用，已使用内置角色");
    });
    if (desktopApi.native) {
      void desktopApi.onCharacterRendererChanged(() => {
        void refreshRenderer().catch(() => {
          if (!disposed) {
            setRendererFailed(true);
            setMessage("角色资源不可用，已使用内置角色");
          }
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => {
        if (!disposed) setMessage("角色更新监听不可用");
      });
      void desktopApi.onPetAutonomyChanged(() => {
        void desktopApi.snapshot().then((value) => {
          if (!disposed) setSnapshot(value);
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => {
        if (!disposed) setMessage("自主行为更新监听不可用");
      });
      void desktopApi.onPetVitalsChanged(() => {
        void desktopApi.snapshot().then((value) => {
          if (!disposed) setSnapshot(value);
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => {
        if (!disposed) setMessage("生命状态更新监听不可用");
      });
    }
    return () => {
      disposed = true;
      listeners.forEach((unlisten) => unlisten());
    };
  }, [refreshRenderer]);

  const handleRendererFailure = useCallback(() => {
    setRendererFailed(true);
    setMessage("角色渲染失败，已使用内置角色");
  }, []);

  async function play(action: PetAction) {
    setMessage(action === "celebrate" ? "今天也很棒！" : "收到");
    await desktopApi.playAction(action);
    const next = await desktopApi.snapshot();
    setSnapshot(next);
  }

  async function interact(x: number, y: number) {
    setMessage("今天也很棒！");
    await desktopApi.clickPet(x, y, "left");
    setSnapshot(await desktopApi.snapshot());
  }

  async function care(action: PetCareAction) {
    const labels: Record<PetCareAction, string> = {
      feed: "吃饱啦，谢谢你！",
      play: "一起玩最开心！",
      groom: "整理得漂漂亮亮！",
    };
    try {
      await desktopApi.carePet(action);
      setSnapshot(await desktopApi.snapshot());
      setMessage(labels[action]);
    } catch {
      setMessage("让我缓一会儿再照料吧");
    }
  }

  async function drag() {
    setMessage("抓稳啦…");
    await desktopApi.dragPet();
    setSnapshot(await desktopApi.snapshot());
    setMessage("安全落地");
  }

  async function toggleClickThrough() {
    const enabled = !snapshot?.windowPolicy.clickThrough;
    await desktopApi.setClickThrough(enabled);
    if (snapshot) setSnapshot({
      ...snapshot,
      windowPolicy: { ...snapshot.windowPolicy, clickThrough: enabled },
    });
    setMessage(enabled ? "已开启鼠标穿透，可从托盘恢复" : "可以互动啦");
  }

  return (
    <main className="pet-overlay" aria-label="Nimora 桌面宠物">
      <button
        className="overlay-drag-region"
        type="button"
        onPointerDown={(event) => {
          if (event.button === 0) void drag();
        }}
        aria-label="拖动 Aster"
      >
        <span className="overlay-status">{message}</span>
        {renderer && renderer.backend !== "built-in" && !rendererFailed ? (
          ["gltf", "vrm"].includes(renderer.backend) ? (
            <RendererErrorBoundary resetKey={renderer.assetId} onFailure={handleRendererFailure}>
              <Suspense fallback={<GltfLoadingPlaceholder descriptor={renderer} />}>
                <GltfRenderer descriptor={renderer} action={petStateAction(snapshot?.pet.state ?? "idle")} onFailure={handleRendererFailure} />
              </Suspense>
            </RendererErrorBoundary>
          ) : (
            <SpriteRenderer
              descriptor={renderer}
              action={petStateAction(snapshot?.pet.state ?? "idle")}
              onFailure={handleRendererFailure}
            />
          )
        ) : (
          <span className={`overlay-pet ${snapshot?.pet.state ?? "idle"}`} aria-hidden="true">
            <i className="overlay-ear left" /><i className="overlay-ear right" />
            <i className="overlay-star">✦</i>
            <i className="overlay-eye left" /><i className="overlay-eye right" />
            <i className="overlay-mouth" />
          </span>
        )}
        <span className="overlay-shadow" aria-hidden="true" />
      </button>
      <div className="overlay-actions" aria-label="宠物快捷操作">
        <button type="button" onClick={() => void care("feed")} aria-label="给 Aster 喂食">◒</button>
        <button type="button" onClick={() => void care("play")} aria-label="陪 Aster 玩耍">✧</button>
        <button type="button" onClick={() => void care("groom")} aria-label="为 Aster 梳理">♢</button>
        <button
          type="button"
          onClick={(event) => void interact(event.screenX, event.screenY)}
          aria-label="和 Aster 互动"
        >✦</button>
        <button type="button" onClick={() => void play("sleep")} aria-label="让 Aster 休息">☾</button>
        <button type="button" onClick={() => void toggleClickThrough()} aria-label="切换鼠标穿透">⌁</button>
      </div>
    </main>
  );
}

function GltfLoadingPlaceholder({ descriptor }: { descriptor: CharacterRendererSnapshot }) {
  return <span className="gltf-renderer gltf-renderer-loading" aria-hidden="true" style={{ aspectRatio: `${descriptor.canvas.width} / ${descriptor.canvas.height}` }} />;
}
