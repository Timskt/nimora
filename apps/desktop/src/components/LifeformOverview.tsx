import type { LifeformSenseSnapshot } from "../platform/desktop";
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from "react";
import type { Pet } from "@nimora/schemas";
import {
  desktopApi,
  type DesktopSnapshot,
  type OverlayStage,
  type PetOcclusion,
  type SystemContextSensorHealth,
} from "../platform/desktop";
import { BuiltinPet } from "./BuiltinPet";
import {
  buildPerfBudgetCards,
  hostProcessBudgetFromDesktopSnapshot,
  LIFEFORM_PERF_EVENT,
  type LifeformHostProcessBudget,
  type LifeformPerfSummary,
} from "./lifeformPerf";
import { RendererErrorBoundary } from "./RendererErrorBoundary";
import { petStatusMessage } from "./petPresentation";

const BuiltinPet3D = lazy(async () => {
  const module = await import("./BuiltinPet3D");
  return { default: module.BuiltinPet3D };
});

export type LifeformPet = Pet;

export interface PersonalityTraits {
  energy: number;
  curiosity: number;
  laziness: number;
  pride: number;
}

/** Loose personality input for pure helpers (host may omit individual axes). */
export type PersonalityInput = Partial<PersonalityTraits> | null | undefined;

export interface PersonalityBar {
  key: keyof PersonalityTraits;
  label: string;
  value: number;
}

export interface DirectiveSummary {
  speech: string | null;
  animation: string | null;
  attention: string | null;
  revision: number | null;
}

/** Optional environmental / host sense signals for Control Center cards. */
export interface LifeformSenseHints {
  occlusion?: Pick<PetOcclusion, "coverage" | "fullyHidden"> | null;
  multiDisplay?: {
    displayCount: number;
    activeLabel?: string | null;
  } | null;
  meeting?: boolean | null;
  batteryPercent?: number | null;
  /** Host power aggregate: true when AC charging is observed. */
  batteryCharging?: boolean | null;
  /** Host power aggregate: true when running on battery. */
  onBattery?: boolean | null;
  idleMinutes?: number | null;
  connector?: {
    connected: number;
    total: number;
    degraded?: number;
  } | null;
  workerBusy?: boolean | null;
  /** App-local unread hint (outbox/approvals) — never titles. */
  notificationUnread?: boolean | null;
}

export type LifeformSenseTone = "neutral" | "ok" | "warn" | "busy" | "danger";

export interface LifeformSenseCard {
  id: string;
  label: string;
  value: string;
  detail?: string;
  tone: LifeformSenseTone;
}

const STATE_LABELS: Record<string, string> = {
  idle: "待机",
  observing: "观察",
  walking: "漫步",
  playing: "玩耍",
  perching: "栖息",
  climbing: "攀爬",
  peeking: "张望",
  stretching: "伸懒腰",
  sleeping: "休息",
  dragged: "被拖动",
  interacting: "互动中",
  working: "专注",
  recovering: "恢复中",
};

const EMOTION_LABELS: Record<string, string> = {
  neutral: "平静",
  happy: "开心",
  sad: "低落",
  angry: "生气",
  surprised: "惊讶",
  focused: "专注",
  sleepy: "困倦",
};

const ATTENTION_LABELS: Record<string, string> = {
  cursor: "鼠标",
  foreground_window: "前台窗口",
  notification_area: "通知区",
  user: "主人",
  idle_scene: "场景",
  obstacle: "障碍",
};

const PERSONALITY_LABELS: ReadonlyArray<{ key: keyof PersonalityTraits; label: string }> = [
  { key: "energy", label: "活力" },
  { key: "curiosity", label: "好奇" },
  { key: "laziness", label: "懒散" },
  { key: "pride", label: "傲娇" },
];

export function clampPercent(value: number | null | undefined, fallback = 0): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(100, Math.max(0, Math.round(value)));
}

export function lifeformStatusLabel(state: string | null | undefined): string {
  if (!state) return STATE_LABELS.idle!;
  return STATE_LABELS[state] ?? state;
}

export function lifeformEmotionLabel(emotion: string | null | undefined): string {
  if (!emotion) return EMOTION_LABELS.neutral!;
  return EMOTION_LABELS[emotion] ?? emotion;
}

/** Map host attention tokens to compact Chinese labels for Control Center. */
export function attentionLabel(attention: string | null | undefined): string | null {
  if (!attention) return null;
  return ATTENTION_LABELS[attention] ?? attention;
}

export function directiveSummary(
  pet: Pick<LifeformPet, "lastDirectiveSpeech" | "lastDirectiveAnimation" | "lastAttention" | "directiveRevision"> | null | undefined,
  overrideSpeech?: string | null,
): DirectiveSummary | null {
  if (!pet && (overrideSpeech == null || overrideSpeech === "")) return null;
  const speech = (overrideSpeech ?? pet?.lastDirectiveSpeech ?? null) || null;
  const animation = pet?.lastDirectiveAnimation ?? null;
  const attention = pet?.lastAttention ?? null;
  const revision = typeof pet?.directiveRevision === "number" ? pet.directiveRevision : null;
  if (!speech && !animation && !attention && (revision == null || revision <= 0)) return null;
  return { speech, animation, attention, revision };
}

export function personalityBars(personality: PersonalityInput): PersonalityBar[] {
  if (!personality || typeof personality !== "object") return [];
  return PERSONALITY_LABELS.flatMap(({ key, label }) => {
    const raw = personality[key];
    if (typeof raw !== "number" || !Number.isFinite(raw)) return [];
    return [{ key, label, value: clampPercent(raw) }];
  });
}

export function overlayStageSummary(stage?: OverlayStage | null): string | null {
  if (!stage) return null;
  const width = Math.round(stage.width);
  const height = Math.round(stage.height);
  if (width <= 0 && height <= 0) return null;
  const originX = Math.round(stage.originX);
  const originY = Math.round(stage.originY);
  return `原点 (${originX}, ${originY}) · ${width}×${height}`;
}

const ANIMATION_LABELS: Record<string, string> = {
  idle: "待机",
  observe: "观察",
  work: "干活",
  work_busy: "出汗干活",
  perch: "栖息",
  celebrate: "庆祝",
  rest: "休息",
  sleep: "睡觉",
  walk: "漫步",
  play: "玩耍",
  stretch: "伸懒腰",
  peek: "张望",
  climb: "攀爬",
};

/** Compact Chinese label for host animation tokens (`pet.work` → 干活). */
export function animationTokenLabel(token: string | null | undefined): string | null {
  if (!token) return null;
  const raw = token.startsWith("pet.") ? token.slice(4) : token;
  if (!raw) return null;
  return ANIMATION_LABELS[raw] ?? raw;
}

export function occlusionSenseLabel(occlusion?: Pick<PetOcclusion, "coverage" | "fullyHidden"> | null): string | null {
  if (!occlusion) return null;
  if (occlusion.fullyHidden) return "完全遮挡";
  const coverage = clampPercent(occlusion.coverage * 100);
  if (coverage <= 0) return "视野开阔";
  if (coverage >= 85) return `重度遮挡 ${coverage}%`;
  if (coverage >= 35) return `部分遮挡 ${coverage}%`;
  return `轻微遮挡 ${coverage}%`;
}

export function batterySenseLabel(
  batteryPercent?: number | null,
  options?: { charging?: boolean | null; onBattery?: boolean | null } | null,
): string | null {
  const charging = options?.charging === true;
  if (typeof batteryPercent === "number" && Number.isFinite(batteryPercent)) {
    const value = clampPercent(batteryPercent);
    if (charging) return `充电中 ${value}%`;
    if (value <= 15) return `电量告急 ${value}%`;
    if (value <= 30) return `电量偏低 ${value}%`;
    if (options?.onBattery === false) return `电源接通 ${value}%`;
    return `电量 ${value}%`;
  }
  if (charging) return "充电中";
  return null;
}

export function idleSenseLabel(idleMinutes?: number | null): string | null {
  if (typeof idleMinutes !== "number" || !Number.isFinite(idleMinutes) || idleMinutes < 0) return null;
  const minutes = Math.round(idleMinutes);
  if (minutes < 1) return "刚活跃";
  if (minutes < 60) return `空闲 ${minutes} 分钟`;
  const hours = Math.floor(minutes / 60);
  const rem = minutes % 60;
  return rem > 0 ? `空闲 ${hours} 小时 ${rem} 分` : `空闲 ${hours} 小时`;
}

export function multiDisplaySenseLabel(
  multiDisplay?: { displayCount: number; activeLabel?: string | null } | null,
): string | null {
  if (!multiDisplay || !Number.isFinite(multiDisplay.displayCount) || multiDisplay.displayCount < 1) return null;
  const count = Math.max(1, Math.round(multiDisplay.displayCount));
  if (count === 1) return multiDisplay.activeLabel?.trim() || "单显示器";
  const base = `${count} 块显示器`;
  const active = multiDisplay.activeLabel?.trim();
  return active ? `${base} · ${active}` : base;
}

export function connectorSenseLabel(
  connector?: { connected: number; total: number; degraded?: number } | null,
): string | null {
  if (!connector) return null;
  const total = Math.max(0, Math.round(connector.total));
  const connected = Math.max(0, Math.min(total || Math.round(connector.connected), Math.round(connector.connected)));
  if (total <= 0 && connected <= 0) return "无连接器";
  const degraded = Math.max(0, Math.round(connector.degraded ?? 0));
  if (degraded > 0) return `${connected}/${total || connected} 连接 · ${degraded} 降级`;
  if (total > 0 && connected < total) return `${connected}/${total} 已连接`;
  return total > 0 ? `${connected}/${total} 正常` : `${connected} 已连接`;
}

/** Build dense, scannable sense cards from optional host hints (skips empty). */
export function buildLifeformSenseCards(hints?: LifeformSenseHints | null): LifeformSenseCard[] {
  if (!hints) return [];
  const cards: LifeformSenseCard[] = [];

  const occlusion = occlusionSenseLabel(hints.occlusion);
  if (occlusion) {
    const fullyHidden = Boolean(hints.occlusion?.fullyHidden);
    const coverage = clampPercent((hints.occlusion?.coverage ?? 0) * 100);
    cards.push({
      id: "occlusion",
      label: "遮挡",
      value: occlusion,
      tone: fullyHidden || coverage >= 85 ? "danger" : coverage >= 35 ? "warn" : "ok",
    });
  }

  const displays = multiDisplaySenseLabel(hints.multiDisplay ?? null);
  if (displays) {
    cards.push({
      id: "multi-display",
      label: "显示器",
      value: displays,
      tone: (hints.multiDisplay?.displayCount ?? 1) > 1 ? "ok" : "neutral",
    });
  }

  if (hints.meeting === true) {
    cards.push({
      id: "meeting",
      label: "会议",
      value: "进行中",
      detail: "降低打扰",
      tone: "warn",
    });
  }

  if (hints.notificationUnread === true) {
    cards.push({
      id: "notification",
      label: "通知",
      value: "有未读",
      detail: "仅队列计数 · 无正文",
      tone: "busy",
    });
  } else if (hints.notificationUnread === false) {
    cards.push({
      id: "notification",
      label: "通知",
      value: "已清空",
      detail: "Outbox / 审批",
      tone: "ok",
    });
  }

  const battery = batterySenseLabel(hints.batteryPercent, {
    charging: hints.batteryCharging ?? null,
    onBattery: hints.onBattery ?? null,
  });
  if (battery) {
    const pct = typeof hints.batteryPercent === "number" ? clampPercent(hints.batteryPercent) : 100;
    const charging = hints.batteryCharging === true;
    cards.push({
      id: "battery",
      label: "电量",
      value: battery,
      tone: charging ? "ok" : pct <= 15 ? "danger" : pct <= 30 ? "warn" : "ok",
    });
  }

  const idle = idleSenseLabel(hints.idleMinutes);
  if (idle) {
    const minutes = Math.round(hints.idleMinutes ?? 0);
    cards.push({
      id: "idle",
      label: "空闲",
      value: idle,
      tone: minutes >= 30 ? "warn" : "neutral",
    });
  }

  const connector = connectorSenseLabel(hints.connector ?? null);
  if (connector) {
    const degraded = hints.connector?.degraded ?? 0;
    const total = hints.connector?.total ?? 0;
    const connected = hints.connector?.connected ?? 0;
    const offline = total > 0 && connected <= 0;
    cards.push({
      id: "connector",
      label: "感官",
      value: offline ? "感官离线" : connector,
      detail: offline ? "Connector · 伙伴有点失落" : "Connector",
      tone: offline ? "danger" : degraded > 0 ? "warn" : total > 0 && connected < total ? "warn" : "ok",
    });
  }

  if (hints.workerBusy === true) {
    cards.push({
      id: "worker",
      label: "体力",
      value: "出汗忙碌",
      detail: "Worker · 本地推理中",
      tone: "busy",
    });
  } else if (hints.workerBusy === false) {
    cards.push({
      id: "worker",
      label: "体力",
      value: "空闲",
      detail: "Worker",
      tone: "neutral",
    });
  }

  return cards;
}

/** Pet Subject narrative: Skill=技能树 · Worker=体力 · Connector=感官. */
export interface LifeformSubjectLink {
  id: "skill" | "worker" | "connector";
  system: "Skill" | "Worker" | "Connector";
  petLabel: string;
  value: string;
  detail?: string;
  tone: LifeformSenseTone;
}

export function skillTreeLabel(affinity?: number | null): string {
  const value = clampPercent(affinity, 0);
  if (value >= 80) return `熟练 ${value}`;
  if (value >= 40) return `成长中 ${value}`;
  if (value > 0) return `萌芽 ${value}`;
  return "待启程";
}

/** Honest worker-busy signal from pet lifecycle (working only). */
export function workerBusyFromPet(pet?: Pick<LifeformPet, "state"> | null): boolean | null {
  if (!pet?.state) return null;
  return pet.state === "working" ? true : null;
}

/** Map host system-context sensors → Connector (感官) counts. No inventing sensors. */
export function connectorFromSystemSensors(
  sensors?: readonly SystemContextSensorHealth[] | null,
): NonNullable<LifeformSenseHints["connector"]> | null {
  if (!sensors || sensors.length === 0) return null;
  const total = sensors.length;
  const connected = sensors.filter((sensor) => sensor.availability === "available").length;
  const degraded = sensors.filter((sensor) => sensor.availability === "degraded").length;
  return { connected, total, degraded };
}

/**
 * Build sense hints from a desktop snapshot when the host provides real data.
 * Never fabricates connector/worker/meeting rows that the snapshot does not support.
 */
export function buildLifeformSenseHintsFromSnapshot(
  snapshot?: Pick<
    DesktopSnapshot,
    "systemContextSensors" | "pet" | "presenceDecision" | "lifeformSense" | "overlayStage"
  > | null,
): LifeformSenseHints | null {
  if (!snapshot) return null;
  const hints: LifeformSenseHints = {};
  const workerBusy = workerBusyFromPet(snapshot.pet);
  if (workerBusy === true) hints.workerBusy = true;
  const connector = connectorFromSystemSensors(snapshot.systemContextSensors);
  if (connector) hints.connector = connector;

  const sense = snapshot.lifeformSense ?? null;
  if (sense) {
    applyLifeformSenseSnapshot(hints, sense, snapshot.overlayStage);
  }

  // Presence reason "do_not_disturb" is a real host signal — surface as quieter meeting-like mode.
  // Prefer explicit OS meeting sense when present.
  if (hints.meeting == null && snapshot.presenceDecision?.reason === "do_not_disturb") {
    hints.meeting = true;
  }
  return Object.keys(hints).length > 0 ? hints : null;
}

/** Prefer first defined boolean among loose IPC aliases. */
function readOptionalBoolean(...values: unknown[]): boolean | null {
  for (const value of values) {
    if (value === true || value === false) return value;
  }
  return null;
}

/** Map privacy-safe host lifeform aggregates into Control Center sense cards. */
export function applyLifeformSenseSnapshot(
  hints: LifeformSenseHints,
  sense: LifeformSenseSnapshot,
  overlayStage?: DesktopSnapshot["overlayStage"],
): void {
  if (typeof sense.batteryPercent === "number" && Number.isFinite(sense.batteryPercent)) {
    hints.batteryPercent = sense.batteryPercent;
  }
  hints.batteryCharging = sense.charging;
  hints.onBattery = sense.onBattery;

  if (typeof sense.idleMs === "number" && Number.isFinite(sense.idleMs) && sense.idleMs >= 0) {
    hints.idleMinutes = sense.idleMs / 60_000;
  }

  if (sense.meetingActive) {
    hints.meeting = true;
  }

  // Host serializes camelCase; accept snake_case for defensive preview/IPC shapes.
  const notificationUnread = readOptionalBoolean(
    sense.notificationUnread,
    (sense as { notification_unread?: unknown }).notification_unread,
  );
  if (notificationUnread === true) {
    hints.notificationUnread = true;
  } else if (notificationUnread === false) {
    hints.notificationUnread = false;
  }

  if (typeof sense.displayCount === "number" && sense.displayCount > 0) {
    const stageLabel = overlayStage
      ? `${overlayStage.width}×${overlayStage.height}`
      : null;
    hints.multiDisplay = {
      displayCount: sense.displayCount,
      activeLabel: stageLabel,
    };
  }
}

/** Merge explicit props over snapshot-derived hints; pet.working fills worker when unknown. */
export function mergeLifeformSenseHints(
  primary?: LifeformSenseHints | null,
  secondary?: LifeformSenseHints | null,
  pet?: Pick<LifeformPet, "state"> | null,
): LifeformSenseHints | null {
  const merged: LifeformSenseHints = {
    ...(secondary ?? {}),
    ...(primary ?? {}),
  };
  if (merged.workerBusy == null) {
    const fromPet = workerBusyFromPet(pet);
    if (fromPet === true) merged.workerBusy = true;
  }
  const keys = Object.keys(merged) as Array<keyof LifeformSenseHints>;
  if (keys.length === 0) return null;
  const meaningful = keys.some((key) => {
    const value = merged[key];
    if (value == null) return false;
    if (typeof value === "boolean") return true;
    if (typeof value === "number") return true;
    if (typeof value === "object") return true;
    return Boolean(value);
  });
  return meaningful ? merged : null;
}

export function buildLifeformSubjectLinks(options?: {
  affinity?: number | null;
  energy?: number | null;
  workerBusy?: boolean | null;
  connector?: LifeformSenseHints["connector"];
} | null): LifeformSubjectLink[] {
  if (!options) return [];
  const links: LifeformSubjectLink[] = [];
  const affinity = options.affinity;
  if (typeof affinity === "number" && Number.isFinite(affinity)) {
    const value = clampPercent(affinity);
    links.push({
      id: "skill",
      system: "Skill",
      petLabel: "技能树",
      value: skillTreeLabel(value),
      detail: `亲密度 ${value} · Skill 成长`,
      tone: value >= 80 ? "ok" : value >= 40 ? "neutral" : "warn",
    });
  }

  if (options.workerBusy === true) {
    links.push({
      id: "worker",
      system: "Worker",
      petLabel: "体力",
      value: "出汗忙碌",
      detail: typeof options.energy === "number"
        ? `能量 ${clampPercent(options.energy)} · 本地推理中`
        : "Worker · 本地推理中",
      tone: "busy",
    });
  } else if (options.workerBusy === false || typeof options.energy === "number") {
    const energy = typeof options.energy === "number" ? clampPercent(options.energy) : null;
    links.push({
      id: "worker",
      system: "Worker",
      petLabel: "体力",
      value: energy == null ? "待命" : energy <= 25 ? `偏低 ${energy}` : `充沛 ${energy}`,
      detail: "Worker 空闲",
      tone: energy != null && energy <= 25 ? "warn" : "neutral",
    });
  }

  const connector = options.connector;
  if (connector) {
    const label = connectorSenseLabel(connector);
    if (label) {
      const degraded = connector.degraded ?? 0;
      const total = connector.total ?? 0;
      const connected = connector.connected ?? 0;
      const offline = total > 0 && connected <= 0;
      links.push({
        id: "connector",
        system: "Connector",
        petLabel: "感官",
        value: offline ? "感官离线" : label,
        detail: offline ? "伙伴有点失落 · 等待感官恢复" : "OS 感知汇总",
        tone: offline ? "danger" : degraded > 0 || (total > 0 && connected < total) ? "warn" : "ok",
      });
    }
  }

  return links;
}

interface LifeformOverviewProps {
  pet?: LifeformPet | null | undefined;
  overlayStage?: OverlayStage | null | undefined;
  /** Live directive speech override from host events. */
  directiveSpeech?: string | null | undefined;
  /** Optional occlusion / multi-display / battery / connector sense from host. */
  senseHints?: LifeformSenseHints | null | undefined;
  /** Optional render budget summary (tests / direct injection). Live source prefers CustomEvent. */
  perfSummary?: LifeformPerfSummary | null | undefined;
  /** Optional host process budget override (memory / idle CPU). */
  hostProcessBudget?: LifeformHostProcessBudget | null | undefined;
}

export function LifeformOverview({
  pet,
  overlayStage,
  directiveSpeech,
  senseHints,
  perfSummary: perfSummaryProp,
  hostProcessBudget: hostProcessBudgetProp,
}: LifeformOverviewProps) {
  const [webglFailed, setWebglFailed] = useState(false);
  const [snapshotHints, setSnapshotHints] = useState<LifeformSenseHints | null>(null);
  const [livePerfSummary, setLivePerfSummary] = useState<LifeformPerfSummary | null>(null);
  const [snapshotProcessBudget, setSnapshotProcessBudget] = useState<LifeformHostProcessBudget | null>(null);
  const handleFailure = useCallback(() => setWebglFailed(true), []);

  useEffect(() => {
    // Always pull host snapshot sense (battery/idle/meeting/display). Explicit
    // senseHints still win via mergeLifeformSenseHints; never freeze on props alone.
    let cancelled = false;
    async function pullSnapshotHints() {
      try {
        const snapshot = await desktopApi.snapshot();
        if (cancelled) return;
        setSnapshotHints(buildLifeformSenseHintsFromSnapshot(snapshot));
        // Prefer DesktopSnapshot.processBudget (always sampled); never hide behind lifeformSense.
        setSnapshotProcessBudget(hostProcessBudgetFromDesktopSnapshot(snapshot));
      } catch {
        if (!cancelled) {
          setSnapshotHints(null);
          setSnapshotProcessBudget(null);
        }
      }
    }
    void pullSnapshotHints();
    // Host lifeform_env samples ~5s; keep Control Center cards honest without full page reload.
    const timer = window.setInterval(() => {
      void pullSnapshotHints();
    }, 5_000);
    let unlisten: (() => void) | undefined;
    void desktopApi.onSystemContextChanged(() => {
      void pullSnapshotHints();
    }).then((dispose) => {
      unlisten = dispose;
    }).catch(() => undefined);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    // Live render budget from BuiltinPet3D (throttled CustomEvent). Prop override still wins in render.
    const onPerf = (event: Event) => {
      const detail = (event as CustomEvent<LifeformPerfSummary>).detail;
      if (!detail || typeof detail !== "object") return;
      if (typeof detail.sampleCount !== "number" || detail.sampleCount <= 0) return;
      setLivePerfSummary({
        fps: Number(detail.fps) || 0,
        avgFrameMs: Number(detail.avgFrameMs) || 0,
        maxFrameMs: Number(detail.maxFrameMs) || 0,
        sampleCount: detail.sampleCount,
      });
    };
    window.addEventListener(LIFEFORM_PERF_EVENT, onPerf);
    return () => {
      window.removeEventListener(LIFEFORM_PERF_EVENT, onPerf);
    };
  }, []);

  const effectiveHints = useMemo(
    () => mergeLifeformSenseHints(senseHints, snapshotHints, pet),
    [senseHints, snapshotHints, pet],
  );

  const name = pet?.name?.trim() || "Aster";
  const state = pet?.state ?? "idle";
  const emotion = pet?.emotion ?? "neutral";
  const energy = clampPercent(pet?.energy, 100);
  const mood = clampPercent(pet?.mood, 70);
  const satiety = clampPercent(pet?.satiety, 100);
  const cleanliness = clampPercent(pet?.cleanliness, 100);
  const affinity = clampPercent(pet?.affinity, 0);
  const statusText = pet
    ? petStatusMessage(pet)
    : "本地陪伴中";
  const directive = directiveSummary(pet, directiveSpeech);
  const traits = personalityBars(pet?.personality);
  const stageText = overlayStageSummary(overlayStage ?? null);
  const senseCards = buildLifeformSenseCards(effectiveHints);
  const effectivePerfSummary = perfSummaryProp ?? livePerfSummary;
  const effectiveHostBudget = hostProcessBudgetProp ?? snapshotProcessBudget;
  const perfCards = buildPerfBudgetCards(effectivePerfSummary, effectiveHostBudget);
  const subjectLinks = buildLifeformSubjectLinks({
    affinity: typeof pet?.affinity === "number" ? pet.affinity : null,
    energy: pet ? energy : null,
    workerBusy: effectiveHints?.workerBusy ?? workerBusyFromPet(pet),
    connector: effectiveHints?.connector ?? null,
  });
  const resetKey = `${state}:${emotion}:${webglFailed ? "fallback" : "3d"}`;
  const attentionText = attentionLabel(directive?.attention);
  const animationText = animationTokenLabel(directive?.animation);

  return (
    <div
      className="lifeform-overview"
      aria-label={`生命体概览 ${name}，状态 ${lifeformStatusLabel(state)}，情绪 ${lifeformEmotionLabel(emotion)}`}
    >
      <div className="lifeform-stage" data-state={state} data-emotion={emotion}>
        <div className="lifeform-stage-glow" aria-hidden="true" />
        <div className="lifeform-stage-floor" aria-hidden="true" />
        <div className="lifeform-stage-canvas">
          {webglFailed ? (
            <div className="lifeform-fallback" role="status">
              <BuiltinPet state={state} emotion={emotion} mood={mood} animation={pet?.lastDirectiveAnimation ?? state} />
              <p>3D 预览暂不可用，已切换柔和示意</p>
            </div>
          ) : (
            <RendererErrorBoundary resetKey={resetKey} onFailure={handleFailure}>
              <Suspense
                fallback={(
                  <BuiltinPet
                    state={state}
                    emotion={emotion}
                    mood={mood}
                    animation={pet?.lastDirectiveAnimation ?? state}
                  />
                )}
              >
                <BuiltinPet3D state={state} emotion={emotion} onFailure={handleFailure} />
              </Suspense>
            </RendererErrorBoundary>
          )}
        </div>
      </div>

      <div className="lifeform-meta">
        <div className="lifeform-identity">
          <strong className="lifeform-name">{name}</strong>
          <div className="lifeform-chips" role="list" aria-label="状态标签">
            <span className="lifeform-chip state" role="listitem">{lifeformStatusLabel(state)}</span>
            <span className="lifeform-chip emotion" role="listitem">{lifeformEmotionLabel(emotion)}</span>
            <span className="lifeform-chip affinity" role="listitem">亲密度 {affinity}</span>
            {effectiveHints?.workerBusy ? (
              <span className="lifeform-chip worker-busy" role="listitem">体力出汗</span>
            ) : null}
            {effectiveHints?.meeting ? (
              <span className="lifeform-chip meeting" role="listitem">会议中</span>
            ) : null}
            {effectiveHints?.connector && (effectiveHints.connector.total ?? 0) > 0 && (effectiveHints.connector.connected ?? 0) <= 0 ? (
              <span className="lifeform-chip connector-offline" role="listitem">感官离线</span>
            ) : null}
          </div>
          <p className="lifeform-status-line">{statusText}</p>
        </div>

        <div className="lifeform-vitals" aria-label="生命体征">
          <VitalBar label="心情" value={mood} tone="mood" />
          <VitalBar label="能量" value={energy} tone="energy" />
          <VitalBar label="饱腹" value={satiety} tone="satiety" />
          <VitalBar label="清洁" value={cleanliness} tone="clean" />
        </div>

        {subjectLinks.length > 0 ? (
          <div className="lifeform-subject-links" aria-label="主体叙事：技能树 · 体力 · 感官">
            <span className="lifeform-section-label">主体叙事</span>
            <ul className="lifeform-subject-cards">
              {subjectLinks.map((link) => (
                <li key={link.id} className={`lifeform-subject-card tone-${link.tone}`}>
                  <span className="lifeform-subject-system">{link.system}</span>
                  <strong className="lifeform-subject-pet">{link.petLabel}</strong>
                  <span className="lifeform-subject-value">{link.value}</span>
                  {link.detail ? <small className="lifeform-subject-detail">{link.detail}</small> : null}
                </li>
              ))}
            </ul>
            <p className="lifeform-subject-legend">Skill → 技能树 · Worker → 体力 · Connector → 感官 · 从真实快照生长</p>
          </div>
        ) : null}

        {directive ? (
          <div className="lifeform-directive" aria-label="最近指令">
            <span className="lifeform-section-label">最近指令</span>
            {directive.speech ? (
              <p className="lifeform-speech" role="status">
                <span className="lifeform-speech-chip">「{directive.speech}」</span>
              </p>
            ) : (
              <p className="lifeform-directive-empty">暂无台词，动作与注意仍会更新</p>
            )}
            <div className="lifeform-directive-meta">
              {animationText ? <span className="lifeform-meta-chip">动画 {animationText}</span> : null}
              {attentionText ? <span className="lifeform-meta-chip attention">注意 {attentionText}</span> : null}
              {directive.revision != null && directive.revision > 0 ? (
                <span className="lifeform-meta-chip">修订 #{directive.revision}</span>
              ) : null}
            </div>
          </div>
        ) : null}

        {traits.length > 0 ? (
          <div className="lifeform-personality" aria-label="性格特质">
            <span className="lifeform-section-label">性格</span>
            <div className="lifeform-personality-grid">
              {traits.map((trait) => (
                <VitalBar key={trait.key} label={trait.label} value={trait.value} tone="trait" compact />
              ))}
            </div>
          </div>
        ) : null}

        {senseCards.length > 0 ? (
          <div className="lifeform-sense-grid" aria-label="环境与系统感知（感官 / OS）">
            <span className="lifeform-section-label">感官 · OS</span>
            <ul className="lifeform-sense-cards">
              {senseCards.map((card) => (
                <li key={card.id} className={`lifeform-sense-card tone-${card.tone}`}>
                  <span className="lifeform-sense-label">{card.label}</span>
                  <strong className="lifeform-sense-value">{card.value}</strong>
                  {card.detail ? <small className="lifeform-sense-detail">{card.detail}</small> : null}
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        {perfCards.length > 0 ? (
          <div className="lifeform-sense-grid lifeform-perf-grid" aria-label="渲染预算">
            <span className="lifeform-section-label">渲染预算</span>
            <ul className="lifeform-sense-cards lifeform-perf-cards">
              {perfCards.map((card) => (
                <li key={card.id} className={`lifeform-sense-card lifeform-perf-card tone-${card.tone}`}>
                  <span className="lifeform-sense-label">{card.label}</span>
                  <strong className="lifeform-sense-value">{card.value}</strong>
                  {card.detail ? <small className="lifeform-sense-detail">{card.detail}</small> : null}
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        {stageText ? (
          <div className="lifeform-overlay-stage" aria-label="桌面 Overlay 舞台">
            <span className="lifeform-section-label">桌面 Overlay 舞台</span>
            <p>{stageText}</p>
          </div>
        ) : null}
      </div>
    </div>
  );
}

function VitalBar({
  label,
  value,
  tone,
  compact = false,
}: {
  label: string;
  value: number;
  tone: "mood" | "energy" | "satiety" | "clean" | "trait";
  compact?: boolean;
}) {
  return (
    <div className={`lifeform-vital${compact ? " compact" : ""}`} data-tone={tone}>
      <div className="lifeform-vital-head">
        <span>{label}</span>
        <strong aria-label={`${label} ${value}%`}>{value}</strong>
      </div>
      <div
        className="lifeform-vital-track"
        role="progressbar"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={value}
        aria-label={label}
      >
        <span style={{ width: `${value}%` }} />
      </div>
    </div>
  );
}
