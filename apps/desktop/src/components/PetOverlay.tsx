import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import type { CharacterRendererSnapshot, DesktopSnapshot, PetAction, PetCareAction } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";
import { RendererErrorBoundary } from "./RendererErrorBoundary";
import { petStatusMessage } from "./petPresentation";
import { exceedsPetDragThreshold, PET_LONG_PRESS_MS, PET_SINGLE_CLICK_DELAY_MS, type PointerOrigin } from "./petGesture";
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
  const [menuOpen, setMenuOpen] = useState(false);
  const pointerOrigin = useRef<PointerOrigin | null>(null);
  const dragging = useRef(false);
  const suppressClick = useRef(false);
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const singleClickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const petButton = useRef<HTMLButtonElement | null>(null);
  const menu = useRef<HTMLDivElement | null>(null);

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
      setMessage(desktopApi.native ? petStatusMessage(value.pet) : "浏览器预览");
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
          if (!disposed) {
            setSnapshot(value);
            setMessage(petStatusMessage(value.pet));
          }
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => {
        if (!disposed) setMessage("自主行为更新监听不可用");
      });
      void desktopApi.onPetVitalsChanged(() => {
        void desktopApi.snapshot().then((value) => {
          if (!disposed) {
            setSnapshot(value);
            setMessage(petStatusMessage(value.pet));
          }
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

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setMenuOpen(false);
        petButton.current?.focus();
      }
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("keydown", closeOnEscape);
      if (longPressTimer.current) clearTimeout(longPressTimer.current);
      if (singleClickTimer.current) clearTimeout(singleClickTimer.current);
    };
  }, []);

  useEffect(() => {
    if (menuOpen) menu.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, [menuOpen]);

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


  function clearLongPress() {
    if (longPressTimer.current) clearTimeout(longPressTimer.current);
    longPressTimer.current = null;
  }

  function openPetMenu() {
    setMenuOpen(true);
    setMessage("想做什么呢？");
  }

  function handlePointerDown(event: React.PointerEvent<HTMLButtonElement>) {
    if (event.button !== 0) return;
    setMenuOpen(false);
    pointerOrigin.current = { clientX: event.clientX, clientY: event.clientY };
    dragging.current = false;
    suppressClick.current = false;
    event.currentTarget.setPointerCapture(event.pointerId);
    clearLongPress();
    longPressTimer.current = setTimeout(() => {
      pointerOrigin.current = null;
      suppressClick.current = true;
      openPetMenu();
    }, PET_LONG_PRESS_MS);
  }

  function handlePointerMove(event: React.PointerEvent<HTMLButtonElement>) {
    if (!pointerOrigin.current || dragging.current) return;
    if (!exceedsPetDragThreshold(pointerOrigin.current, event.clientX, event.clientY)) return;
    clearLongPress();
    dragging.current = true;
    suppressClick.current = true;
    pointerOrigin.current = null;
    void drag();
  }

  function finishPointerGesture() {
    clearLongPress();
    pointerOrigin.current = null;
    dragging.current = false;
  }

  function handlePetClick(event: React.MouseEvent<HTMLButtonElement>) {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    if (event.detail >= 2) {
      if (singleClickTimer.current) clearTimeout(singleClickTimer.current);
      singleClickTimer.current = null;
      void play("celebrate");
      return;
    }
    const { screenX, screenY } = event;
    singleClickTimer.current = setTimeout(() => {
      singleClickTimer.current = null;
      void interact(screenX, screenY);
    }, PET_SINGLE_CLICK_DELAY_MS);
  }

  return (
    <main className="pet-overlay" aria-label="Nimora 桌面宠物">
      <button
        ref={petButton}
        className="overlay-drag-region"
        type="button"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={finishPointerGesture}
        onPointerCancel={finishPointerGesture}
        onClick={handlePetClick}
        onContextMenu={(event) => {
          event.preventDefault();
          clearLongPress();
          suppressClick.current = true;
          openPetMenu();
        }}
        aria-label="与 Aster 互动或拖动"
        aria-haspopup="menu"
        aria-expanded={menuOpen}
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
          <span className={`overlay-pet ${snapshot?.pet.state ?? "idle"} emotion-${snapshot?.pet.emotion ?? "neutral"}`} aria-hidden="true">
            <i className="overlay-ear left" /><i className="overlay-ear right" />
            <i className="overlay-star">✦</i>
            <i className="overlay-eye left" /><i className="overlay-eye right" />
            <i className="overlay-mouth" />
          </span>
        )}
        <span className="overlay-shadow" aria-hidden="true" />
      </button>
      {menuOpen ? (
        <div ref={menu} className="overlay-pet-menu" role="menu" aria-label="宠物菜单">
          <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void care("feed"); }}><span>◒</span>喂食</button>
          <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void care("play"); }}><span>✧</span>玩耍</button>
          <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void care("groom"); }}><span>♢</span>梳理</button>
          <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void play("sleep"); }}><span>☾</span>休息</button>
          <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void toggleClickThrough(); }}><span>⌁</span>鼠标穿透</button>
        </div>
      ) : null}
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
