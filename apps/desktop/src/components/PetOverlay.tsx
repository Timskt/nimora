import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import {
  desktopApi,
  EMPTY_PET_OCCLUSION,
  type CharacterRendererSnapshot,
  type DesktopSnapshot,
  type PetAction,
  type PetCareAction,
  type PetDirectiveEvent,
  type PetItemId,
  type PetOcclusion,
  type PetSurface,
} from "../platform/desktop";
import { RendererErrorBoundary } from "./RendererErrorBoundary";
import {
  PET_BODY_HEIGHT_PX,
  PET_BODY_WIDTH_PX,
  clampPetScreenToStage,
  isStageWorkAreaReady,
  normalizeDirectiveSpeech,
  occlusionMutesAmbient,
  occlusionPresentation,
  petFacing,
  petLifeformTokens,
  petLocalPosition,
  petStatusMessage,
  resolveOverlayStage,
  resolvePetRenderState,
  resolvePetSubjectMotion,
} from "./petPresentation";
import {
  appendPetGesturePoint,
  createPetGestureTrail,
  exceedsPetDragThreshold,
  isPetStroke,
  petClickResolution,
  shouldNoticePet,
  PET_LONG_PRESS_MS,
  PET_SINGLE_CLICK_DELAY_MS,
  type PetGestureTrail,
} from "./petGesture";
import { SpriteRenderer } from "./SpriteRenderer";
import { petInventoryQuantity, petItemPresentation } from "./petItems";
import { focusMenuItem, isPetMenuShortcut, nextMenuItemIndex } from "./petMenu";
import { agentCompanionPresentation } from "./agentCompanion";
import { canPresentPetBubble, usePetBubble } from "./petBubble";
import { subscribeReducedMotion } from "./reducedMotion";
import { BuiltinPet } from "./BuiltinPet";
import { BUILTIN_FOX_RENDERER } from "./builtinFox";

const GltfRenderer = lazy(async () => {
  const module = await import("./GltfRenderer");
  return { default: module.GltfRenderer };
});
const BuiltinPet3D = lazy(async () => {
  const module = await import("./BuiltinPet3D");
  return { default: module.BuiltinPet3D };
});
const PET_WINDOW_HEARTBEAT_INTERVAL_MS = 15_000;

export function PetOverlay() {
  const [snapshot, setSnapshot] = useState<DesktopSnapshot | null>(null);
  const [renderer, setRenderer] = useState<CharacterRendererSnapshot | null>(null);
  const [rendererFailed, setRendererFailed] = useState(false);
  const [builtinModelFailed, setBuiltinModelFailed] = useState(false);
  const [builtin3dFailed, setBuiltin3dFailed] = useState(false);
  const [surface, setSurface] = useState<PetSurface | null>(null);
  const [statusBubblesEnabled, setStatusBubblesEnabled] = useState(true);
  const { message, visible: bubbleVisible, presentBubble } = usePetBubble("正在醒来…", statusBubblesEnabled);
  const [pointerActive, setPointerActive] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuPage, setMenuPage] = useState<"root" | "more" | "inventory" | "rename">("root");
  const [nameDraft, setNameDraft] = useState("");
  const gestureTrail = useRef<PetGestureTrail | null>(null);
  const dragging = useRef(false);
  const dragSession = useRef<{ grabOffsetX: number; grabOffsetY: number } | null>(null);
  const suppressClick = useRef(false);
  const [poseOverride, setPoseOverride] = useState<{ x: number; y: number } | null>(null);
  const longPressTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const singleClickTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const petButton = useRef<HTMLButtonElement | null>(null);
  const menu = useRef<HTMLDivElement | null>(null);
  const [stroking, setStroking] = useState(false);
  const [isDragging, setIsDragging] = useState(false);
  const [companionAction, setCompanionAction] = useState<PetAction | null>(null);
  const [directive, setDirective] = useState<PetDirectiveEvent | null>(null);
  const [occlusion, setOcclusion] = useState<PetOcclusion>(EMPTY_PET_OCCLUSION);
  const companionResetTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastCompanionSpeechRef = useRef<string | null>(null);
  const directiveSpeechRef = useRef<string | null>(null);
  // exactOptionalPropertyTypes: speech is string | null, never undefined.
  const directiveSpeech = normalizeDirectiveSpeech(directive?.speech);
  directiveSpeechRef.current = directiveSpeech;
  const ambientMutedRef = useRef(false);
  const lastNoticeAt = useRef(Number.NEGATIVE_INFINITY);
  // Subject rule: directive.animation/action > companion signal > lifecycle.
  const subjectMotion = resolvePetSubjectMotion({
    directiveAnimation: directive?.animation ?? null,
    directiveAction: directive?.action ?? null,
    companionAction,
    lifecycleState: snapshot?.pet.state ?? null,
  });
  const lifeform = petLifeformTokens(
    snapshot?.pet
      ? {
          ...snapshot.pet,
          mood: typeof directive?.mood === "number" ? directive.mood : snapshot.pet.mood,
        }
      : snapshot?.pet,
    subjectMotion,
  );
  // Directive speech keeps neutral attentive facing; walk tokens still flip left/right.
  const facing = snapshot
    ? petFacing(snapshot.pet, {
        animation: lifeform.animation,
        directiveSpeech,
      })
    : "neutral";
  const renderAction = lifeform.animation;
  const renderState = resolvePetRenderState(subjectMotion);
  const screenPosition = poseOverride ?? snapshot?.pet.position ?? { x: 0, y: 0 };
  const stage = resolveOverlayStage(snapshot?.overlayStage);
  // Multi-monitor work area: negative origins OK; clamp body into stage when size is known.
  const { localX, localY } = petLocalPosition(screenPosition, stage, {
    clampToStage: isStageWorkAreaReady(stage),
    bodyWidth: PET_BODY_WIDTH_PX,
    bodyHeight: PET_BODY_HEIGHT_PX,
  });
  const occlusionStyle = occlusionPresentation(occlusion);
  const ambientMuted = occlusionMutesAmbient(occlusion.coverage) || occlusion.fullyHidden;
  ambientMutedRef.current = ambientMuted;
  const subjectStyle = {
    ["--pet-local-x" as string]: `${localX}px`,
    ["--pet-local-y" as string]: `${localY}px`,
    ["--pet-body-width" as string]: `${PET_BODY_WIDTH_PX}px`,
    ["--pet-body-height" as string]: `${PET_BODY_HEIGHT_PX}px`,
    ["--pet-occlusion-clip" as string]: occlusionStyle.clipPath,
    opacity: occlusionStyle.opacity,
    // Keep inline clip-path for hosts that ignore the CSS custom property.
    clipPath: occlusionStyle.clipPath,
    WebkitClipPath: occlusionStyle.clipPath,
  };
  const applySnapshot = useCallback((value: DesktopSnapshot) => {
    setSnapshot(value);
    setStatusBubblesEnabled(value.petPresentation.statusBubblesEnabled);
    if (!dragSession.current) setPoseOverride(null);
  }, []);

  useEffect(() => {
    if (!desktopApi.native) return;
    const beat = () => void desktopApi.petWindowHeartbeat().catch(() => undefined);
    beat();
    const timer = window.setInterval(beat, PET_WINDOW_HEARTBEAT_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, []);

  useEffect(() => {
    if (typeof window.matchMedia !== "function") return;
    const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
    return subscribeReducedMotion(preference, (enabled) => {
      void desktopApi.setReducedMotion(enabled).catch(() => undefined);
    });
  }, []);

  useEffect(() => {
    let disposed = false;
    let disposeListener: (() => void) | undefined;
    void desktopApi.onPetPositionChanged((event) => {
      if (disposed || dragSession.current) return;
      setPoseOverride({ x: event.x, y: event.y });
      setSnapshot((current) => {
        if (!current) return current;
        return {
          ...current,
          pet: { ...current.pet, position: { x: event.x, y: event.y } },
        };
      });
    }).then((dispose) => {
      if (disposed) dispose();
      else disposeListener = dispose;
    }).catch(() => undefined);
    return () => {
      disposed = true;
      disposeListener?.();
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let disposeListener: (() => void) | undefined;
    void desktopApi.onPetOcclusionChanged((event) => {
      if (disposed) return;
      setOcclusion(event);
    }).then((dispose) => {
      if (disposed) dispose();
      else disposeListener = dispose;
    }).catch(() => undefined);
    return () => {
      disposed = true;
      disposeListener?.();
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let disposeListener: (() => void) | undefined;
    void desktopApi.onPetDirectiveChanged((event) => {
      if (disposed) return;
      setDirective(event);
      // Keep companionAction independent — subject rule prefers directive fields first.
      const speech = normalizeDirectiveSpeech(event.speech)?.trim();
      if (speech) {
        if (lastCompanionSpeechRef.current === speech && event.revision != null) {
          // Same speech already shown (e.g. companion signal + directive race).
        } else {
          const channel = event.attention === "user" || event.attention === "cursor" ? "feedback" : "status";
          if (!(channel === "status" && ambientMutedRef.current)) {
            lastCompanionSpeechRef.current = speech;
            presentBubble(speech, channel);
          }
        }
      }
    }).then((dispose) => {
      if (disposed) dispose();
      else disposeListener = dispose;
    }).catch(() => undefined);
    return () => {
      disposed = true;
      disposeListener?.();
    };
  }, [presentBubble]);

  const refreshRenderer = useCallback(async () => {
    const descriptor = await desktopApi.activeCharacterRenderer();
    setRenderer(descriptor);
    setRendererFailed(false);
    setBuiltinModelFailed(false);
    setBuiltin3dFailed(false);
  }, []);

  useEffect(() => {
    let disposed = false;
    const listeners: Array<() => void> = [];
    void Promise.all([desktopApi.snapshot(), desktopApi.activeCharacterRenderer()]).then(([value, descriptor]) => {
      if (disposed) return;
      applySnapshot(value);
      setNameDraft(value.pet.name);
      setRenderer(descriptor);
      setRendererFailed(false);
      presentBubble(desktopApi.native ? petStatusMessage(value.pet, { directiveSpeech: directiveSpeechRef.current }) : "浏览器预览", "status");
    }).catch(() => {
      if (disposed) return;
      presentBubble("角色资源不可用，已使用内置角色", "error");
    });
    if (desktopApi.native) {
      void desktopApi.onCharacterRendererChanged(() => {
        void refreshRenderer().catch(() => {
          if (!disposed) {
            setRendererFailed(true);
            presentBubble("角色资源不可用，已使用内置角色", "error");
          }
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => {
        if (!disposed) presentBubble("角色更新监听不可用", "error");
      });
      void desktopApi.onPetAutonomyChanged(() => {
        void desktopApi.snapshot().then((value) => {
          if (!disposed) {
            applySnapshot(value);
            void desktopApi.requestAttention("autonomy", "bubble", "ambient").then((attention) => {
              if (!disposed && attention.allowed && !ambientMutedRef.current) presentBubble(petStatusMessage(value.pet, { directiveSpeech: directiveSpeechRef.current }), "status");
            }).catch(() => undefined);
          }
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => {
        if (!disposed) presentBubble("自主行为更新监听不可用", "error");
      });
      void desktopApi.onPetVitalsChanged(() => {
        void desktopApi.snapshot().then((value) => {
          if (!disposed) {
            applySnapshot(value);
            if (!ambientMutedRef.current) presentBubble(petStatusMessage(value.pet, { directiveSpeech: directiveSpeechRef.current }), "status");
          }
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => {
        if (!disposed) presentBubble("生命状态更新监听不可用", "error");
      });
      void desktopApi.onProfileChanged(() => {
        void desktopApi.snapshot().then((value) => {
          if (!disposed) applySnapshot(value);
        });
      }).then((disposeListener) => {
        if (disposed) disposeListener();
        else listeners.push(disposeListener);
      }).catch(() => {
        if (!disposed) presentBubble("Profile 更新监听不可用", "error");
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
      const priority = signal.status === "failed" || signal.status === "waiting_for_confirmation" ? "feedback" : "ambient";
      void desktopApi.requestAttention("agent", "bubble", priority).then((attention) => {
        if (disposed || !attention.allowed) return;
        if (companionResetTimer.current) clearTimeout(companionResetTimer.current);
        setCompanionAction(presentation.action);
        // Avoid double-spamming when applyPetDirective already drove the same speech.
        if (lastCompanionSpeechRef.current !== presentation.message) {
          lastCompanionSpeechRef.current = presentation.message;
          presentBubble(presentation.message);
        }
        if (!presentation.persistent) {
          companionResetTimer.current = setTimeout(() => {
            if (disposed) return;
            setCompanionAction(null);
            lastCompanionSpeechRef.current = null;
            void desktopApi.snapshot().then((value) => {
              if (!disposed && !ambientMutedRef.current) presentBubble(petStatusMessage(value.pet, { directiveSpeech: directiveSpeechRef.current }), "status");
            });
          }, 4200);
        }
      }).catch(() => undefined);
    }).then((disposeListener) => {
      if (disposed) disposeListener();
      else listeners.push(disposeListener);
    }).catch(() => undefined);
    return () => {
      disposed = true;
      listeners.forEach((unlisten) => unlisten());
      if (companionResetTimer.current) clearTimeout(companionResetTimer.current);
    };
  }, [applySnapshot, presentBubble, refreshRenderer]);

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
    presentBubble("角色渲染失败，已使用内置角色", "error");
  }, [presentBubble]);

  const handleBuiltin3dFailure = useCallback(() => {
    setBuiltin3dFailed(true);
    presentBubble("3D 加速不可用，已切换轻量角色", "error");
  }, [presentBubble]);

  const handleBuiltinModelFailure = useCallback(() => {
    setBuiltinModelFailed(true);
    presentBubble("专业角色资源不可用，已切换内置 3D 角色", "error");
  }, [presentBubble]);

  async function play(action: PetAction) {
    try {
      await desktopApi.playAction(action);
      setSnapshot(await desktopApi.snapshot());
      presentBubble(action === "celebrate" ? "今天也很棒！" : action === "sleep" ? "晚安，我会慢慢恢复体力" : "收到");
    } catch {
      presentBubble("这个动作现在接不上，再试一次吧", "error");
    }
  }

  async function interact(x: number, y: number) {
    try {
      await desktopApi.clickPet(x, y, "left");
      setSnapshot(await desktopApi.snapshot());
      presentBubble("今天也很棒！");
    } catch {
      presentBubble("慢一点，我还没站稳呢", "error");
    }
  }

  async function doubleInteract(x: number, y: number) {
    try {
      await desktopApi.doubleClickPet(x, y, "left");
      setSnapshot(await desktopApi.snapshot());
      presentBubble("太开心了，再来一次吧！");
    } catch {
      presentBubble("慢一点，我还没站稳呢", "error");
    }
  }

  async function stroke(distancePx: number, durationMs: number, reversals: number) {
    presentBubble("好舒服，再摸摸我吧");
    await desktopApi.strokePet(
      Math.min(240, distancePx),
      Math.min(2_000, durationMs),
      Math.min(12, reversals),
    );
    setSnapshot(await desktopApi.snapshot());
    window.setTimeout(() => setStroking(false), 420);
  }

  function notice(event: React.PointerEvent<HTMLButtonElement>) {
    const now = performance.now();
    if (!shouldNoticePet({
      pointerType: event.pointerType,
      menuOpen,
      gestureActive: gestureTrail.current != null,
      dragging: dragging.current,
      lastNoticeAtMs: lastNoticeAt.current,
      nowMs: now,
    })) return;
    lastNoticeAt.current = now;
    presentBubble("我看到你啦");
    void desktopApi.noticePet(event.screenX, event.screenY)
      .then(async () => setSnapshot(await desktopApi.snapshot()))
      .catch(() => undefined);
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
      presentBubble(labels[action]);
    } catch {
      presentBubble("让我缓一会儿再照料吧", "error");
    }
  }

  async function useItem(itemId: PetItemId) {
    try {
      await desktopApi.usePetItem(itemId);
      const next = await desktopApi.snapshot();
      setSnapshot(next);
      presentBubble(`已使用${petItemPresentation(itemId).label}`);
    } catch {
      presentBubble("道具不可用或正在冷却", "error");
    }
  }

  async function renamePet() {
    const name = nameDraft.trim();
    if (!name || [...name].length > 64) {
      presentBubble("名字需要 1–64 个字符", "error");
      return;
    }
    try {
      await desktopApi.renamePet(name);
      const next = await desktopApi.snapshot();
      setSnapshot(next);
      setNameDraft(next.pet.name);
      setMenuPage("root");
      presentBubble(`以后就叫我${next.pet.name}吧`);
    } catch {
      presentBubble("现在不能改名，原来的名字还在", "error");
    }
  }

  async function startLogicalDrag(screenX: number, screenY: number) {
    const origin = poseOverride ?? snapshot?.pet.position ?? { x: 0, y: 0 };
    dragSession.current = {
      grabOffsetX: screenX - origin.x,
      grabOffsetY: screenY - origin.y,
    };
    presentBubble("抓稳啦…");
    setIsDragging(true);
    try {
      // Prefer host begin/move/finish APIs when present (logical stage drag).
      if (typeof desktopApi.beginPetDrag === "function") {
        await desktopApi.beginPetDrag();
      } else if (typeof desktopApi.dragPet === "function") {
        // Legacy host: optimistic drag without a begin handshake.
        await desktopApi.dragPet();
      }
    } catch {
      dragSession.current = null;
      dragging.current = false;
      setIsDragging(false);
      presentBubble("这次没拖起来，再试一次吧", "error");
    }
  }

  async function moveLogicalDrag(screenX: number, screenY: number) {
    const session = dragSession.current;
    if (!session) return;
    // Optimistic multi-monitor clamp keeps the 260×300 body inside the stage work area.
    const next = clampPetScreenToStage(
      {
        x: screenX - session.grabOffsetX,
        y: screenY - session.grabOffsetY,
      },
      resolveOverlayStage(snapshot?.overlayStage),
      PET_BODY_WIDTH_PX,
      PET_BODY_HEIGHT_PX,
    );
    setPoseOverride(next);
    try {
      if (typeof desktopApi.movePet === "function") {
        await desktopApi.movePet(next.x, next.y);
      }
    } catch {
      // Keep optimistic pose; finish will re-sync from snapshot.
    }
  }

  async function finishLogicalDrag() {
    if (!dragSession.current) return;
    dragSession.current = null;
    setIsDragging(false);
    try {
      if (typeof desktopApi.finishPetDrag === "function") {
        await desktopApi.finishPetDrag();
      }
      const next = await desktopApi.snapshot();
      setSnapshot(next);
      setPoseOverride(null);
      presentBubble("安全落地");
    } catch {
      const next = await desktopApi.snapshot().catch(() => snapshot);
      if (next) {
        setSnapshot(next);
        setPoseOverride(null);
      }
      presentBubble("这次没拖起来，再试一次吧", "error");
    }
  }

  async function setHome() {
    try {
      await desktopApi.setPetHome();
      setSnapshot(await desktopApi.snapshot());
      presentBubble("记住啦，这里就是家");
    } catch {
      presentBubble("现在不能设置家位置", "error");
    }
  }

  async function returnHome() {
    try {
      await desktopApi.returnPetHome();
      setSnapshot(await desktopApi.snapshot());
      presentBubble("回到家啦");
    } catch {
      presentBubble("回家路线暂时不可用", "error");
    }
  }

  async function toggleClickThrough() {
    const enabled = !snapshot?.windowPolicy.clickThrough;
    await desktopApi.setClickThrough(enabled);
    if (snapshot) setSnapshot({
      ...snapshot,
      windowPolicy: { ...snapshot.windowPolicy, clickThrough: enabled },
    });
    presentBubble(enabled ? "已开启鼠标穿透，可从托盘恢复" : "可以互动啦");
  }


  function clearLongPress() {
    if (longPressTimer.current) clearTimeout(longPressTimer.current);
    longPressTimer.current = null;
  }

  function openPetMenu() {
    setMenuPage("root");
    setMenuOpen(true);
    presentBubble("想做什么呢？");
  }

  function handleMenuKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    const items = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="menuitem"]'));
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const next = nextMenuItemIndex(Math.max(0, current), items.length, event.key);
    if (next === null) return;
    event.preventDefault();
    focusMenuItem(items[next]);
  }

  function handlePointerDown(event: React.PointerEvent<HTMLButtonElement>) {
    setPointerActive(true);
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
    if (dragSession.current || dragging.current) {
      void moveLogicalDrag(event.screenX, event.screenY);
      return;
    }
    if (!gestureTrail.current) return;
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
    void startLogicalDrag(event.screenX, event.screenY);
  }

  function finishPointerGesture() {
    clearLongPress();
    const trail = gestureTrail.current;
    gestureTrail.current = null;
    if (dragging.current || dragSession.current) {
      void finishLogicalDrag();
      dragging.current = false;
      setPointerActive(false);
      return;
    }
    if (trail && isPetStroke(trail, performance.now())) {
      suppressClick.current = true;
      void stroke(
        trail.distancePx,
        Math.round(performance.now() - trail.startedAtMs),
        trail.reversals,
      ).catch(() => {
        setStroking(false);
        presentBubble("现在不方便抚摸，请稍后再试", "error");
      });
    } else {
      setStroking(false);
    }
    dragging.current = false;
    setPointerActive(false);
  }

  function cancelPointerGesture() {
    clearLongPress();
    gestureTrail.current = null;
    if (dragSession.current || dragging.current) {
      void finishLogicalDrag();
    }
    dragging.current = false;
    suppressClick.current = true;
    setStroking(false);
    setPointerActive(false);
  }

  function handlePetClick(event: React.MouseEvent<HTMLButtonElement>) {
    if (suppressClick.current) {
      suppressClick.current = false;
      return;
    }
    const resolution = petClickResolution(event.detail);
    if (resolution === "ignore") return;
    if (resolution === "double") {
      if (singleClickTimer.current) clearTimeout(singleClickTimer.current);
      singleClickTimer.current = null;
      void doubleInteract(event.screenX, event.screenY);
      return;
    }
    const { screenX, screenY } = event;
    singleClickTimer.current = setTimeout(() => {
      singleClickTimer.current = null;
      void interact(screenX, screenY);
    }, PET_SINGLE_CLICK_DELAY_MS);
  }

  return (
    <main className={`pet-overlay${menuOpen || pointerActive ? " bubble-suppressed" : ""}${bubbleVisible && canPresentPetBubble({ menuOpen, pointerActive }) ? " bubble-visible" : ""}${isDragging ? " is-dragging" : ""}${occlusion.fullyHidden ? " is-occluded" : ""}`} aria-label="Nimora 桌面宠物" data-lifeform="subject">
      <div
        className="pet-subject"
        style={subjectStyle}
        data-occlusion-coverage={occlusion.coverage.toFixed(3)}
        data-occlusion-hidden={occlusion.fullyHidden ? "true" : "false"}
        data-ambient-muted={ambientMuted ? "true" : "false"}
        data-directive-revision={directive?.revision ?? ""}
      >
      {/* Bubble is a sibling of the body hit-area so ellipse clip-path never trims speech. */}
      <span className="overlay-status pet-speech-bubble" role="status" aria-live="polite" aria-atomic="true">{message}</span>
      <button
        ref={petButton}
        className={`overlay-drag-region pet-hit-area${stroking ? " is-stroking" : ""}${isDragging ? " is-dragging" : ""}`}
        type="button"
        onPointerEnter={notice}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={finishPointerGesture}
        onPointerCancel={cancelPointerGesture}
        onClick={handlePetClick}
        onKeyDown={(event) => {
          if (!isPetMenuShortcut(event.key, event.shiftKey)) return;
          event.preventDefault();
          event.stopPropagation();
          openPetMenu();
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          clearLongPress();
          suppressClick.current = true;
          openPetMenu();
        }}
        aria-label={`与 ${snapshot?.pet.name ?? "Aster"} 互动、抚摸或拖动 · Nimora Q版小黄人伙伴`}
        aria-haspopup="menu"
        aria-expanded={menuOpen}
      >
        <span
          className={`pet-character-stage facing-${facing} surface-${surface ?? "free"} state-${renderState}`}
          data-state={renderState}
          data-emotion={lifeform.emotion}
          data-mood={lifeform.mood}
          data-mood-band={lifeform.moodBand}
          data-animation={lifeform.animation}
          data-facing={facing}
          data-surface={surface ?? "free"}
          data-subject-motion={subjectMotion}
        >
          {/* Default chain: BuiltinPet3D → fox GLB → SVG. Custom packs still win when selected. */}
          {renderer && renderer.backend !== "built-in" && !rendererFailed ? (
            ["gltf", "vrm"].includes(renderer.backend) ? (
              <RendererErrorBoundary resetKey={renderer.assetId} onFailure={handleRendererFailure}>
                <Suspense fallback={<GltfLoadingPlaceholder descriptor={renderer} />}>
                  <GltfRenderer descriptor={renderer} action={renderAction} onFailure={handleRendererFailure} />
                </Suspense>
              </RendererErrorBoundary>
            ) : (
              <SpriteRenderer
                descriptor={renderer}
                action={renderAction}
                onFailure={handleRendererFailure}
              />
            )
          ) : !builtin3dFailed ? (
            <RendererErrorBoundary resetKey="builtin.aster.3d" onFailure={handleBuiltin3dFailure}>
              <Suspense fallback={<BuiltinPet state={renderState} emotion={lifeform.emotion} mood={lifeform.mood} animation={lifeform.animation} />}>
                <BuiltinPet3D
                  state={renderState}
                  emotion={lifeform.emotion}
                  onFailure={handleBuiltin3dFailure}
                />
              </Suspense>
            </RendererErrorBoundary>
          ) : !builtinModelFailed ? (
            <RendererErrorBoundary resetKey={BUILTIN_FOX_RENDERER.assetId} onFailure={handleBuiltinModelFailure}>
              <Suspense fallback={<BuiltinPet state={renderState} emotion={lifeform.emotion} mood={lifeform.mood} animation={lifeform.animation} />}>
                <GltfRenderer
                  descriptor={BUILTIN_FOX_RENDERER}
                  action={renderAction}
                  onFailure={handleBuiltinModelFailure}
                />
              </Suspense>
            </RendererErrorBoundary>
          ) : (
            <BuiltinPet state={renderState} emotion={lifeform.emotion} mood={lifeform.mood} animation={lifeform.animation} />
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
              <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void play("peek"); }}><span>◉</span>从顶部探头</button>
              <button type="button" role="menuitem" onClick={() => { setMenuOpen(false); void play("stretch"); }}><span>↔</span>伸个懒腰</button>
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
      {!menuOpen && !occlusion.fullyHidden ? <div className="overlay-actions" aria-label="宠物快捷操作">
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
      </div>
    </main>
  );
}

function GltfLoadingPlaceholder({ descriptor }: { descriptor: CharacterRendererSnapshot }) {
  return <span className="gltf-renderer gltf-renderer-loading" aria-hidden="true" style={{ aspectRatio: `${descriptor.canvas.width} / ${descriptor.canvas.height}` }} />;
}
