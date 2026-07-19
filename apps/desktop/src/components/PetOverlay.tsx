import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import type { CharacterRendererSnapshot, DesktopSnapshot, PetAction, PetCareAction, PetItemId, PetSurface } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";
import { RendererErrorBoundary } from "./RendererErrorBoundary";
import { petFacing, petStatusMessage } from "./petPresentation";
import {
  appendPetGesturePoint,
  createPetGestureTrail,
  exceedsPetDragThreshold,
  isPetStroke,
  PET_LONG_PRESS_MS,
  PET_SINGLE_CLICK_DELAY_MS,
  type PetGestureTrail,
} from "./petGesture";
import { petStateAction, SpriteRenderer } from "./SpriteRenderer";
import { petInventoryQuantity, petItemPresentation } from "./petItems";
import { nextMenuItemIndex } from "./petMenu";
import { agentCompanionPresentation } from "./agentCompanion";

const GltfRenderer = lazy(async () => {
  const module = await import("./GltfRenderer");
  return { default: module.GltfRenderer };
});

export function PetOverlay() {
  const [snapshot, setSnapshot] = useState<DesktopSnapshot | null>(null);
  const [renderer, setRenderer] = useState<CharacterRendererSnapshot | null>(null);
  const [rendererFailed, setRendererFailed] = useState(false);
  const [surface, setSurface] = useState<PetSurface | null>(null);
  const [message, setMessage] = useState("正在醒来…");
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuPage, setMenuPage] = useState<"root" | "more" | "inventory" | "rename">("root");
  const [nameDraft, setNameDraft] = useState("");
  const gestureTrail = useRef<PetGestureTrail | null>(null);
  const dragging = useRef(false);
  const suppressClick = useRef(false);
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const singleClickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const petButton = useRef<HTMLButtonElement | null>(null);
  const menu = useRef<HTMLDivElement | null>(null);
  const [stroking, setStroking] = useState(false);
  const [companionAction, setCompanionAction] = useState<PetAction | null>(null);
  const companionResetTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const facing = snapshot ? petFacing(snapshot.pet) : "neutral";

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
      setNameDraft(value.pet.name);
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
      void desktopApi.onPetSurfaceChanged(() => {
        void desktopApi.petSurface().then((value) => {
          if (!disposed) setSurface(value.surface);
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => undefined);
    }
    void desktopApi.petSurface().then((value) => {
      if (!disposed) setSurface(value.surface);
    }).catch(() => undefined);
    void desktopApi.onAgentCompanionSignal((signal) => {
      if (disposed) return;
      const presentation = agentCompanionPresentation(signal.status);
      if (companionResetTimer.current) clearTimeout(companionResetTimer.current);
      setCompanionAction(presentation.action);
      setMessage(presentation.message);
      if (!presentation.persistent) {
        companionResetTimer.current = setTimeout(() => {
          if (disposed) return;
          setCompanionAction(null);
          void desktopApi.snapshot().then((value) => {
            if (!disposed) setMessage(petStatusMessage(value.pet));
          });
        }, 4200);
      }
    }).then((disposeListener) => {
      if (disposed) disposeListener();
      else listeners.push(disposeListener);
    }).catch(() => undefined);
    return () => {
      disposed = true;
      listeners.forEach((unlisten) => unlisten());
      if (companionResetTimer.current) clearTimeout(companionResetTimer.current);
    };
  }, [refreshRenderer]);

  useEffect(() => {
    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        if (menuPage !== "root") {
          setMenuPage("root");
        } else {
          setMenuOpen(false);
          petButton.current?.focus();
        }
      }
    }
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("keydown", closeOnEscape);
      if (longPressTimer.current) clearTimeout(longPressTimer.current);
      if (singleClickTimer.current) clearTimeout(singleClickTimer.current);
    };
  }, [menuPage]);

  useEffect(() => {
    if (!menuOpen) return;
    if (menuPage === "rename") menu.current?.querySelector<HTMLInputElement>("input")?.focus();
    else menu.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, [menuOpen, menuPage]);

  const handleRendererFailure = useCallback(() => {
    setRendererFailed(true);
    setMessage("角色渲染失败，已使用内置角色");
  }, []);

  async function play(action: PetAction) {
    setMessage(action === "celebrate" ? "今天也很棒！" : action === "sleep" ? "晚安，我会慢慢恢复体力" : "收到");
    await desktopApi.playAction(action);
    const next = await desktopApi.snapshot();
    setSnapshot(next);
  }

  async function interact(x: number, y: number) {
    setMessage("今天也很棒！");
    await desktopApi.clickPet(x, y, "left");
    setSnapshot(await desktopApi.snapshot());
  }

  async function stroke(distancePx: number, durationMs: number, reversals: number) {
    setMessage("好舒服，再摸摸我吧");
    await desktopApi.strokePet(
      Math.min(240, distancePx),
      Math.min(2_000, durationMs),
      Math.min(12, reversals),
    );
    setSnapshot(await desktopApi.snapshot());
    window.setTimeout(() => setStroking(false), 420);
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

  async function useItem(itemId: PetItemId) {
    try {
      await desktopApi.usePetItem(itemId);
      const next = await desktopApi.snapshot();
      setSnapshot(next);
      setMessage(`已使用${petItemPresentation(itemId).label}`);
    } catch {
      setMessage("道具不可用或正在冷却");
    }
  }

  async function renamePet() {
    const name = nameDraft.trim();
    if (!name || [...name].length > 64) {
      setMessage("名字需要 1–64 个字符");
      return;
    }
    try {
      await desktopApi.renamePet(name);
      const next = await desktopApi.snapshot();
      setSnapshot(next);
      setNameDraft(next.pet.name);
      setMenuPage("root");
      setMessage(`以后就叫我${next.pet.name}吧`);
    } catch {
      setMessage("现在不能改名，原来的名字还在");
    }
  }

  async function drag() {
    setMessage("抓稳啦…");
    await desktopApi.dragPet();
    setSnapshot(await desktopApi.snapshot());
    setMessage("安全落地");
  }

  async function setHome() {
    try {
      await desktopApi.setPetHome();
      setSnapshot(await desktopApi.snapshot());
      setMessage("记住啦，这里就是家");
    } catch {
      setMessage("现在不能设置家位置");
    }
  }

  async function returnHome() {
    try {
      await desktopApi.returnPetHome();
      setSnapshot(await desktopApi.snapshot());
      setMessage("回到家啦");
    } catch {
      setMessage("回家路线暂时不可用");
    }
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
    setMenuPage("root");
    setMenuOpen(true);
    setMessage("想做什么呢？");
  }

  function handleMenuKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]'));
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = nextMenuItemIndex(Math.max(0, current), items.length, event.key);
    if (next === null) return;
    event.preventDefault();
    items[next]?.focus();
  }

  function handlePointerDown(event: React.PointerEvent<HTMLButtonElement>) {
    if (event.button !== 0) return;
    setMenuOpen(false);
    gestureTrail.current = createPetGestureTrail(
      { clientX: event.clientX, clientY: event.clientY },
      performance.now(),
    );
    dragging.current = false;
    suppressClick.current = false;
    setStroking(false);
    event.currentTarget.setPointerCapture(event.pointerId);
    clearLongPress();
    longPressTimer.current = setTimeout(() => {
      gestureTrail.current = null;
      suppressClick.current = true;
      openPetMenu();
    }, PET_LONG_PRESS_MS);
  }

  function handlePointerMove(event: React.PointerEvent<HTMLButtonElement>) {
    if (!gestureTrail.current || dragging.current) return;
    const nextTrail = appendPetGesturePoint(gestureTrail.current, {
      clientX: event.clientX,
      clientY: event.clientY,
    });
    gestureTrail.current = nextTrail;
    if (nextTrail.distancePx >= 6) {
      clearLongPress();
      setStroking(true);
    }
    if (!exceedsPetDragThreshold(nextTrail.origin, event.clientX, event.clientY)) return;
    clearLongPress();
    dragging.current = true;
    suppressClick.current = true;
    gestureTrail.current = null;
    setStroking(false);
    void drag();
  }

  function finishPointerGesture() {
    clearLongPress();
    const trail = gestureTrail.current;
    gestureTrail.current = null;
    if (!dragging.current && trail && isPetStroke(trail, performance.now())) {
      suppressClick.current = true;
      void stroke(
        trail.distancePx,
        Math.round(performance.now() - trail.startedAtMs),
        trail.reversals,
      ).catch(() => {
        setStroking(false);
        setMessage("现在不方便抚摸，请稍后再试");
      });
    } else if (!dragging.current) {
      setStroking(false);
    }
    dragging.current = false;
  }

  function cancelPointerGesture() {
    clearLongPress();
    gestureTrail.current = null;
    dragging.current = false;
    suppressClick.current = true;
    setStroking(false);
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
    <main className={`pet-overlay${companionAction ? " companion-active" : ""}`} aria-label="Nimora 桌面宠物">
      <button
        ref={petButton}
        className={`overlay-drag-region${stroking ? " is-stroking" : ""}`}
        type="button"
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={finishPointerGesture}
        onPointerCancel={cancelPointerGesture}
        onClick={handlePetClick}
        onContextMenu={(event) => {
          event.preventDefault();
          clearLongPress();
          suppressClick.current = true;
          openPetMenu();
        }}
        aria-label={`与 ${snapshot?.pet.name ?? "Aster"} 互动、抚摸或拖动`}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
      >
        <span className="overlay-status">{message}</span>
        <span className={`pet-character-stage facing-${facing} surface-${surface ?? "free"} state-${snapshot?.pet.state ?? "idle"}`}>
          {renderer && renderer.backend !== "built-in" && !rendererFailed ? (
            ["gltf", "vrm"].includes(renderer.backend) ? (
              <RendererErrorBoundary resetKey={renderer.assetId} onFailure={handleRendererFailure}>
                <Suspense fallback={<GltfLoadingPlaceholder descriptor={renderer} />}>
                  <GltfRenderer descriptor={renderer} action={companionAction ?? petStateAction(snapshot?.pet.state ?? "idle")} onFailure={handleRendererFailure} />
                </Suspense>
              </RendererErrorBoundary>
            ) : (
              <SpriteRenderer
                descriptor={renderer}
                action={companionAction ?? petStateAction(snapshot?.pet.state ?? "idle")}
                onFailure={handleRendererFailure}
              />
            )
          ) : (
            <span className={`overlay-pet ${companionAction ?? snapshot?.pet.state ?? "idle"} emotion-${snapshot?.pet.emotion ?? "neutral"}`} aria-hidden="true">
              <i className="overlay-ear left" /><i className="overlay-ear right" />
              <i className="overlay-star">✦</i>
              <i className="overlay-eye left" /><i className="overlay-eye right" />
              <i className="overlay-mouth" />
            </span>
          )}
        </span>
        <span className="overlay-shadow" aria-hidden="true" />
      </button>
      {menuOpen ? (
        <div ref={menu} className={`overlay-pet-menu ${menuPage}-page`} role={menuPage === "rename" ? "dialog" : "menu"} aria-label={menuPage === "inventory" ? "随身背包" : menuPage === "rename" ? "修改伙伴名字" : menuPage === "more" ? "更多宠物操作" : "宠物径向菜单"} onKeyDown={handleMenuKeyDown}>
          {menuPage === "root" ? (
            <div className="radial-menu-items">
              <button className="radial-item radial-1" type="button" role="menuitem" onClick={() => { setMenuOpen(false); void care("feed"); }}><span>◒</span><b>喂食</b></button>
              <button className="radial-item radial-2" type="button" role="menuitem" onClick={() => { setMenuOpen(false); void care("play"); }}><span>✧</span><b>玩耍</b></button>
              <button className="radial-item radial-3" type="button" role="menuitem" onClick={() => { setMenuOpen(false); void care("groom"); }}><span>♢</span><b>梳理</b></button>
              <button className="radial-item radial-4" type="button" role="menuitem" onClick={() => setMenuPage("inventory")}><span>▣</span><b>背包</b><small>{petInventoryQuantity(snapshot?.pet.inventory ?? [])}</small></button>
              <button className="radial-item radial-5" type="button" role="menuitem" onClick={() => { setMenuOpen(false); void returnHome(); }}><span>⌂</span><b>回家</b></button>
              <button className="radial-item radial-6" type="button" role="menuitem" onClick={() => setMenuPage("more")}><span>•••</span><b>更多</b></button>
              <button className="radial-close" type="button" role="menuitem" aria-label="关闭宠物菜单" onClick={() => { setMenuOpen(false); petButton.current?.focus(); }}>×</button>
            </div>
          ) : menuPage === "more" ? (
            <>
              <button className="inventory-back" type="button" role="menuitem" onClick={() => setMenuPage("root")}><span>‹</span>返回径向菜单</button>
              <button type="button" role="menuitem" onClick={() => void desktopApi.openControlCenter("agent_chat")}><span>◌</span>和我聊天</button>
              <button type="button" role="menuitem" onClick={() => void desktopApi.openControlCenter("agent_task")}><span>▶</span>开始任务</button>
              <button type="button" role="menuitem" onClick={() => { setNameDraft(snapshot?.pet.name ?? "Aster"); setMenuPage("rename"); }}><span>✎</span>改名字</button>
              <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void setHome(); }}><span>⌖</span>这里设为家</button>
              <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void play("perch"); }}><span>⌄</span>在边缘栖息</button>
              <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void play("climb"); }}><span>↥</span>沿侧边攀爬</button>
              <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void play("sleep"); }}><span>☾</span>休息</button>
              <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void toggleClickThrough(); }}><span>⌁</span>鼠标穿透</button>
              <button type="button" role="menuitem" onClick={() => void desktopApi.openControlCenter("settings")}><span>⚙</span>设置</button>
            </>
          ) : menuPage === "inventory" ? (
            <>
              <button className="inventory-back" type="button" role="menuitem" onClick={() => setMenuPage("root")}><span>‹</span>返回宠物菜单</button>
              {snapshot?.pet.inventory.map((stack) => {
                const item = petItemPresentation(stack.itemId);
                return <button className="inventory-item" type="button" role="menuitem" key={stack.itemId} onClick={() => void useItem(stack.itemId)}><span>{item.glyph}</span><b>{item.label}<small>{item.effect}</small></b><em>×{stack.quantity}</em></button>;
              })}
              {(snapshot?.pet.inventory.length ?? 0) === 0 ? <p className="overlay-inventory-empty">背包空空的<br />已有物品不会过期</p> : null}
            </>
          ) : <form className="overlay-rename-form" onSubmit={(event) => { event.preventDefault(); void renamePet(); }}>
            <label htmlFor="overlay-pet-name">新的名字</label>
            <input id="overlay-pet-name" maxLength={64} value={nameDraft} onChange={(event) => setNameDraft(event.target.value)} autoFocus />
            <div><button type="button" onClick={() => setMenuPage("root")}>返回</button><button type="submit">保存</button></div>
          </form>}
        </div>
      ) : null}
      {!menuOpen ? <div className="overlay-actions" aria-label="宠物快捷操作">
        <button type="button" onClick={() => void care("feed")} aria-label={`给 ${snapshot?.pet.name ?? "Aster"} 喂食`}>◒</button>
        <button type="button" onClick={() => void care("play")} aria-label={`陪 ${snapshot?.pet.name ?? "Aster"} 玩耍`}>✧</button>
        <button type="button" onClick={() => void care("groom")} aria-label={`为 ${snapshot?.pet.name ?? "Aster"} 梳理`}>♢</button>
        <button
          type="button"
          onClick={(event) => void interact(event.screenX, event.screenY)}
          aria-label={`和 ${snapshot?.pet.name ?? "Aster"} 互动`}
        >✦</button>
        <button type="button" onClick={() => void play("sleep")} aria-label={`让 ${snapshot?.pet.name ?? "Aster"} 休息`}>☾</button>
        <button type="button" onClick={() => void toggleClickThrough()} aria-label="切换鼠标穿透">⌁</button>
      </div> : null}
    </main>
  );
}

function GltfLoadingPlaceholder({ descriptor }: { descriptor: CharacterRendererSnapshot }) {
  return <span className="gltf-renderer gltf-renderer-loading" aria-hidden="true" style={{ aspectRatio: `${descriptor.canvas.width} / ${descriptor.canvas.height}` }} />;
}
