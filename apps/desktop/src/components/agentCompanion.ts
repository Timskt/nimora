import type {
  AgentCompanionSignal,
  AgentCompanionStatus,
  PetAction,
  StructuredPetDirective,
} from "../platform/desktop";

export interface AgentCompanionPresentation {
  action: PetAction;
  message: string;
  persistent: boolean;
}

/** Visual tone for control-center companion strip bubbles. */
export type AgentCompanionTone = "thinking" | "running" | "waiting" | "success" | "danger" | "idle";

/** User-facing pet mood chip (Chinese) for Agent/Auto Mode phases. */
export type AgentCompanionMoodLabel =
  | "好奇"
  | "专注"
  | "期待"
  | "开心"
  | "低落"
  | "平静";

/** User-facing action chip (Chinese) — not host snake_case tokens. */
export type AgentCompanionActionLabel =
  | "观察思考"
  | "出汗干活"
  | "栖息等待"
  | "庆祝完成"
  | "休息调整"
  | "安静待命";

export interface AgentCompanionNarrative {
  status: AgentCompanionStatus;
  /** Compact phase chip, e.g. 思考中. */
  phaseLabel: string;
  /** Pet speech bubble copy. */
  speech: string;
  /** Understandable action for owners (not work_busy). */
  actionLabel: AgentCompanionActionLabel;
  /** Understandable mood for owners. */
  moodLabel: AgentCompanionMoodLabel;
  tone: AgentCompanionTone;
  /** Host pet pose used by soft presentation layer. */
  pose: PetAction;
  persistent: boolean;
}

export interface AgentCompanionBubble {
  status: AgentCompanionStatus;
  label: string;
  tone: AgentCompanionTone;
  message: string;
  persistent: boolean;
  /** Pet narrative chips for Control Center strip. */
  actionLabel: AgentCompanionActionLabel;
  moodLabel: AgentCompanionMoodLabel;
}

const presentations: Record<AgentCompanionStatus, AgentCompanionPresentation> = {
  thinking: { action: "observe", message: "我在想…", persistent: true },
  running: { action: "work", message: "正在陪你干活", persistent: true },
  waiting_for_confirmation: { action: "perch", message: "需要你确认一下", persistent: true },
  completed: { action: "celebrate", message: "完成啦！", persistent: false },
  failed: { action: "idle", message: "没关系，我们再试", persistent: false },
  cancelled: { action: "idle", message: "已停下，我还在", persistent: false },
};

const statusLabels: Record<AgentCompanionStatus, string> = {
  thinking: "思考中",
  running: "执行中",
  waiting_for_confirmation: "等待确认",
  completed: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

const statusTones: Record<AgentCompanionStatus, AgentCompanionTone> = {
  thinking: "thinking",
  running: "running",
  waiting_for_confirmation: "waiting",
  completed: "success",
  failed: "danger",
  cancelled: "idle",
};

const actionLabels: Record<AgentCompanionStatus, AgentCompanionActionLabel> = {
  thinking: "观察思考",
  running: "出汗干活",
  waiting_for_confirmation: "栖息等待",
  completed: "庆祝完成",
  failed: "休息调整",
  cancelled: "安静待命",
};

const moodLabels: Record<AgentCompanionStatus, AgentCompanionMoodLabel> = {
  thinking: "好奇",
  running: "专注",
  waiting_for_confirmation: "期待",
  completed: "开心",
  failed: "低落",
  cancelled: "平静",
};

/** Host-facing directive action / attention tokens (snake_case serde). */
type CompanionDirectiveBlueprint = {
  action: StructuredPetDirective["action"];
  attention: StructuredPetDirective["attention"];
  speech: string;
  animation: string;
  moodDelta?: { mood: number };
};

const directiveBlueprints: Record<AgentCompanionStatus, CompanionDirectiveBlueprint> = {
  thinking: {
    action: "observe",
    attention: "user",
    speech: "我在想…",
    animation: "pet.observe",
    moodDelta: { mood: 1 },
  },
  running: {
    action: "work_busy",
    attention: "user",
    speech: "正在陪你干活",
    animation: "pet.work",
  },
  waiting_for_confirmation: {
    action: "perch",
    attention: "user",
    speech: "需要你确认一下",
    animation: "pet.perch",
  },
  completed: {
    action: "celebrate",
    attention: "user",
    speech: "完成啦！",
    animation: "pet.celebrate",
    moodDelta: { mood: 6 },
  },
  failed: {
    action: "rest",
    attention: "idle_scene",
    speech: "没关系，我们再试",
    animation: "pet.idle",
    moodDelta: { mood: -2 },
  },
  cancelled: {
    action: "rest",
    attention: "idle_scene",
    speech: "已停下，我还在",
    animation: "pet.idle",
  },
};

/** Chinese chips for structured pet animation tokens (Subject micro-performances). */
const animationTokenLabels: Record<string, string> = {
  "pet.idle": "安静待命",
  "pet.observe": "观察思考",
  "pet.walk": "四处走走",
  "pet.play": "玩耍",
  "pet.perch": "栖息等待",
  "pet.work": "出汗干活",
  "pet.celebrate": "庆祝完成",
  "pet.sleep": "休息调整",
  "pet.yawn": "打哈欠",
  "pet.dig_nose": "抠鼻子",
  "pet.count_ants": "数蚂蚁",
  "pet.wave": "招手",
  "pet.look_around": "四处张望",
  "pet.hop": "轻跳",
};

/** Map a host animation token to a short Chinese chip owners can scan. */
export function companionAnimationLabel(token: string | null | undefined): string {
  if (!token) return "安静待命";
  const raw = token.trim();
  if (!raw) return "安静待命";
  if (raw in animationTokenLabels) return animationTokenLabels[raw]!;
  const withPrefix = raw.startsWith("pet.") ? raw : `pet.${raw}`;
  if (withPrefix in animationTokenLabels) return animationTokenLabels[withPrefix]!;
  return "小表演";
}

export function agentCompanionPresentation(status: AgentCompanionStatus): AgentCompanionPresentation {
  return presentations[status];
}

/** Compact Chinese label for control-center companion strip chips. */
export function agentCompanionStatusLabel(status: AgentCompanionStatus): string {
  return statusLabels[status];
}

/** Color tone token for companion bubble styling. */
export function agentCompanionTone(status: AgentCompanionStatus): AgentCompanionTone {
  return statusTones[status];
}

/** Action chip owners can understand (出汗干活 / 栖息等待 / …). */
export function agentCompanionActionLabel(status: AgentCompanionStatus): AgentCompanionActionLabel {
  return actionLabels[status];
}

/** Mood chip owners can understand (好奇 / 专注 / 低落 / …). */
export function agentCompanionMoodLabel(status: AgentCompanionStatus): AgentCompanionMoodLabel {
  return moodLabels[status];
}

/**
 * Map Agent / Auto Mode / Worker phases onto a pet narrative:
 * speech + mood + action tokens users can scan without host jargon.
 */
export function agentCompanionNarrative(status: AgentCompanionStatus): AgentCompanionNarrative {
  const presentation = agentCompanionPresentation(status);
  return {
    status,
    phaseLabel: agentCompanionStatusLabel(status),
    speech: presentation.message,
    actionLabel: agentCompanionActionLabel(status),
    moodLabel: agentCompanionMoodLabel(status),
    tone: agentCompanionTone(status),
    pose: presentation.action,
    persistent: presentation.persistent,
  };
}

/** Full bubble model: Chinese label + tone + speech + pet narrative chips. */
export function agentCompanionBubble(status: AgentCompanionStatus): AgentCompanionBubble {
  const narrative = agentCompanionNarrative(status);
  return {
    status,
    label: narrative.phaseLabel,
    tone: narrative.tone,
    message: narrative.speech,
    persistent: narrative.persistent,
    actionLabel: narrative.actionLabel,
    moodLabel: narrative.moodLabel,
  };
}

/** Map agent/auto companion status onto a host-accepted structured pet directive. */
export function agentCompanionDirective(status: AgentCompanionStatus): StructuredPetDirective {
  const blueprint = directiveBlueprints[status];
  const directive: StructuredPetDirective = {
    spec: "nimora.pet_directive/1",
    speech: blueprint.speech,
    action: blueprint.action,
    animation: blueprint.animation,
    attention: blueprint.attention,
  };
  if (blueprint.moodDelta) {
    directive.moodDelta = blueprint.moodDelta;
  }
  return directive;
}


/**
 * Map Auto Mode / Goal control-center job-like status onto companion phases.
 * Host has no separate goal companion IPC — FE derives pet language from job status.
 */
export function companionStatusFromAutoMode(
  status: string | null | undefined,
  options?: { pauseReason?: string | null; indeterminate?: boolean },
): AgentCompanionStatus {
  if (options?.indeterminate) return "waiting_for_confirmation";
  const token = (status ?? "").toLowerCase();
  const pauseReason = (options?.pauseReason ?? "").toLowerCase();
  if (token === "running" || token === "starting") return "running";
  if (token === "pausing" || token === "paused") {
    // Revoked grant mid-run is a failure surface, not a soft wait.
    if (pauseReason === "grant_revoked") return "failed";
    // Paused jobs need owner attention (confirm / budget / manual pause).
    return "waiting_for_confirmation";
  }
  if (token === "completed" || token === "done") return "completed";
  if (token === "failed" || token === "indeterminate") return "failed";
  if (token === "cancelled" || token === "cancelling") return "cancelled";
  if (token === "waiting_for_confirmation" || token === "submitted") return "waiting_for_confirmation";
  return "thinking";
}

/**
 * Auto Mode job-status chip (Running / Paused / Failed) for control-center pet phase.
 * Distinct from agentCompanionStatusLabel so manual pause is not mislabeled as 等待确认.
 */
export function autoModePhaseLabel(
  status: string | null | undefined,
  options?: { pauseReason?: string | null; indeterminate?: boolean },
): string {
  if (options?.indeterminate) return "结果未知";
  const token = (status ?? "").toLowerCase();
  const pauseReason = (options?.pauseReason ?? "").toLowerCase();
  if (token === "starting") return "启动中";
  if (token === "running") return "执行中";
  if (token === "pausing") return "暂停中";
  if (token === "paused") {
    if (pauseReason === "confirmation_required") return "等待确认";
    if (pauseReason === "grant_revoked") return "失败";
    if (pauseReason === "budget_exhausted" || pauseReason === "max_cycles" || pauseReason === "token_budget") {
      return "预算暂停";
    }
    if (pauseReason === "user_paused") return "已暂停";
    return "已暂停";
  }
  if (token === "completed" || token === "done") return "已完成";
  if (token === "failed") return "失败";
  if (token === "indeterminate") return "结果未知";
  if (token === "cancelling") return "取消中";
  if (token === "cancelled") return "已取消";
  if (token === "waiting_for_confirmation" || token === "submitted") return "等待确认";
  return "思考中";
}

/**
 * Owner-facing companion bubble for Auto Mode focus rows.
 * Phase chip reflects Running/Paused/Failed; pet action/mood stay host-safe.
 */
export function companionBubbleFromAutoMode(
  status: string | null | undefined,
  options?: { pauseReason?: string | null; indeterminate?: boolean },
): AgentCompanionBubble {
  const companionStatus = companionStatusFromAutoMode(status, options);
  const base = agentCompanionBubble(companionStatus);
  const phase = autoModePhaseLabel(status, options);
  const token = (status ?? "").toLowerCase();
  const pauseReason = (options?.pauseReason ?? "").toLowerCase();

  if (token === "running" || token === "starting") {
    return {
      ...base,
      label: phase,
      message: token === "starting" ? "自动模式正在启动…" : "自动模式执行中，我陪着你",
    };
  }

  if (token === "pausing" || token === "paused") {
    if (pauseReason === "confirmation_required") {
      return { ...base, label: phase, message: "需要你确认高风险步骤" };
    }
    if (pauseReason === "grant_revoked") {
      return { ...base, label: phase, message: "授权已撤销，自动执行已停下" };
    }
    if (pauseReason === "budget_exhausted" || pauseReason === "max_cycles" || pauseReason === "token_budget") {
      return { ...base, label: phase, message: "预算到顶了，等你看看再继续" };
    }
    if (pauseReason === "user_paused") {
      return {
        ...base,
        label: phase,
        message: "已安全暂停，Checkpoint 还在",
        moodLabel: "平静",
      };
    }
    return {
      ...base,
      label: phase,
      message: "任务已暂停，随时可回来处理",
      moodLabel: "平静",
    };
  }

  if (token === "failed" || token === "indeterminate") {
    return {
      ...base,
      label: phase,
      message: token === "indeterminate" ? "外部结果未知，需要你对账" : "自动模式失败了，我们再试",
    };
  }

  if (token === "cancelled" || token === "cancelling") {
    return {
      ...base,
      label: phase,
      message: token === "cancelling" ? "正在取消任务…" : "任务已取消，我还在",
    };
  }

  if (token === "completed" || token === "done") {
    return { ...base, label: phase, message: "自动模式完成啦！" };
  }

  return { ...base, label: phase };
}

export function createAgentCompanionSignal(status: AgentCompanionStatus, taskId: string | null = null): AgentCompanionSignal {
  return { spec: "nimora.agent-companion-signal/1", status, taskId, updatedAtMs: Date.now() };
}

// ---------------------------------------------------------------------------
// Worker / Connector petization (mirrors host companion_directive wrappers)
// ---------------------------------------------------------------------------

/** Skill / Worker lifecycle phases that map to pet body language. */
export type WorkerCompanionPhase = "busy" | "done" | "failed" | "idle";

/** Connector / sensory phases that map to pet body language. */
export type ConnectorCompanionPhase =
  | "offline"
  | "degraded"
  | "restored"
  | "event"
  | "healthy"
  | "unknown";

/** Owner-facing Chinese chips for Worker (体力) phases. */
export type WorkerCompanionActionLabel = "出汗干活" | "庆祝完成" | "晕倒缓缓" | "安静待命";
export type WorkerCompanionMoodLabel = "疲惫" | "开心" | "低落" | "平静";

/** Owner-facing Chinese chips for Connector (感官) phases. */
export type ConnectorCompanionActionLabel =
  | "休息调整"
  | "警戒观察"
  | "庆祝完成"
  | "观察思考"
  | "安静待命";
export type ConnectorCompanionMoodLabel = "低落" | "警惕" | "开心" | "好奇" | "平静";

/** Visual feedback for BuiltinPet3D sweat / dizzy binding. */
export type PetWorkVisual = "sweat" | "dizzy" | "none";

export interface WorkerCompanionNarrative {
  phase: WorkerCompanionPhase;
  phaseLabel: string;
  speech: string;
  actionLabel: WorkerCompanionActionLabel;
  moodLabel: WorkerCompanionMoodLabel;
  /** Host action token when a directive should be emitted; null when fail-closed quiet. */
  hostAction: StructuredPetDirective["action"] | null;
}

export interface ConnectorCompanionNarrative {
  phase: ConnectorCompanionPhase;
  phaseLabel: string;
  speech: string;
  actionLabel: ConnectorCompanionActionLabel;
  moodLabel: ConnectorCompanionMoodLabel;
  hostAction: StructuredPetDirective["action"] | null;
}

const workerPhaseLabels: Record<WorkerCompanionPhase, string> = {
  busy: "出汗忙碌",
  done: "已完成",
  failed: "失败晕倒",
  idle: "空闲",
};

const connectorPhaseLabels: Record<ConnectorCompanionPhase, string> = {
  offline: "感官离线",
  degraded: "信号不稳",
  restored: "线路恢复",
  event: "有新动静",
  healthy: "感官正常",
  unknown: "感官未知",
};

/**
 * Fail-closed Worker phase parser.
 * Unknown / empty tokens map to `idle` and must not invent busy/crash acts.
 */
export function workerCompanionPhaseFromStatus(
  status: string | null | undefined,
): WorkerCompanionPhase {
  const token = (status ?? "").trim().toLowerCase();
  if (!token) return "idle";
  if (token === "busy" || token === "running" || token === "working" || token === "starting") {
    return "busy";
  }
  if (token === "done" || token === "completed" || token === "ok" || token === "succeeded" || token === "success") {
    return "done";
  }
  if (
    token === "failed"
    || token === "fail"
    || token === "error"
    || token === "crash"
    || token === "crashed"
    || token === "panic"
  ) {
    return "failed";
  }
  if (token === "idle" || token === "cancelled" || token === "canceled") return "idle";
  // Fail-closed: never invent worker stress from mystery tokens.
  return "idle";
}

/**
 * Fail-closed Connector phase parser.
 * Unknown tokens map to `unknown` and do not emit host directives.
 */
export function connectorCompanionPhaseFromStatus(
  status: string | null | undefined,
): ConnectorCompanionPhase {
  const token = (status ?? "").trim().toLowerCase();
  if (!token) return "unknown";
  if (token === "offline" || token === "disconnected" || token === "down") return "offline";
  if (token === "degraded" || token === "flaky" || token === "unstable") return "degraded";
  if (token === "restored" || token === "online_restored" || token === "online-restored" || token === "reconnected") {
    return "restored";
  }
  if (token === "event" || token === "event_received" || token === "event-received") return "event";
  if (token === "healthy" || token === "online" || token === "connected" || token === "ok") return "healthy";
  return "unknown";
}

/** Worker narrative chips (Chinese) aligned with host skill_worker_* directives. */
export function workerCompanionNarrative(
  phase: WorkerCompanionPhase,
  skillName?: string | null,
): WorkerCompanionNarrative {
  switch (phase) {
    case "busy": {
      const label = (skillName ?? "").trim();
      const speech = label
        ? `「${label.slice(0, 40)}」跑起来了`
        : "技能跑起来了";
      return {
        phase,
        phaseLabel: workerPhaseLabels.busy,
        speech,
        actionLabel: "出汗干活",
        moodLabel: "疲惫",
        hostAction: "work_busy",
      };
    }
    case "done":
      return {
        phase,
        phaseLabel: workerPhaseLabels.done,
        speech: "搞定啦！",
        actionLabel: "庆祝完成",
        moodLabel: "开心",
        hostAction: "celebrate",
      };
    case "failed":
      return {
        phase,
        phaseLabel: workerPhaseLabels.failed,
        speech: "刚才绊了一下",
        actionLabel: "晕倒缓缓",
        moodLabel: "低落",
        hostAction: "work_crash",
      };
    case "idle":
    default:
      return {
        phase: "idle",
        phaseLabel: workerPhaseLabels.idle,
        speech: "体力空闲，随时开工",
        actionLabel: "安静待命",
        moodLabel: "平静",
        hostAction: null,
      };
  }
}

/** Connector narrative chips (Chinese) aligned with host connector_sensory_* directives. */
export function connectorCompanionNarrative(
  phase: ConnectorCompanionPhase,
): ConnectorCompanionNarrative {
  switch (phase) {
    case "offline":
      return {
        phase,
        phaseLabel: connectorPhaseLabels.offline,
        speech: "线路好像断了",
        actionLabel: "休息调整",
        moodLabel: "低落",
        hostAction: "rest",
      };
    case "degraded":
      return {
        phase,
        phaseLabel: connectorPhaseLabels.degraded,
        speech: "信号不太稳",
        actionLabel: "警戒观察",
        moodLabel: "警惕",
        hostAction: "observe",
      };
    case "restored":
      return {
        phase,
        phaseLabel: connectorPhaseLabels.restored,
        speech: "线路通了",
        actionLabel: "庆祝完成",
        moodLabel: "开心",
        hostAction: "celebrate",
      };
    case "event":
      return {
        phase,
        phaseLabel: connectorPhaseLabels.event,
        speech: "有新动静",
        actionLabel: "观察思考",
        moodLabel: "好奇",
        hostAction: "observe",
      };
    case "healthy":
      return {
        phase,
        phaseLabel: connectorPhaseLabels.healthy,
        speech: "感官正常",
        actionLabel: "安静待命",
        moodLabel: "平静",
        hostAction: null,
      };
    case "unknown":
    default:
      return {
        phase: "unknown",
        phaseLabel: connectorPhaseLabels.unknown,
        speech: "感官状态未知",
        actionLabel: "安静待命",
        moodLabel: "平静",
        hostAction: null,
      };
  }
}

/**
 * Build host-accepted `nimora.pet_directive/1` for a Worker phase.
 * Returns `null` when fail-closed (idle / unknown → no emit).
 */
export function workerCompanionDirective(
  phase: WorkerCompanionPhase,
  skillName?: string | null,
): StructuredPetDirective | null {
  const narrative = workerCompanionNarrative(phase, skillName);
  if (!narrative.hostAction) return null;
  if (phase === "busy") {
    return {
      spec: "nimora.pet_directive/1",
      speech: narrative.speech,
      action: "work_busy",
      animation: "pet.work",
      attention: "user",
      moodDelta: { mood: 2 },
    };
  }
  if (phase === "done") {
    return {
      spec: "nimora.pet_directive/1",
      speech: narrative.speech,
      action: "celebrate",
      animation: "pet.celebrate",
      attention: "user",
      moodDelta: { mood: 8 },
    };
  }
  if (phase === "failed") {
    return {
      spec: "nimora.pet_directive/1",
      speech: narrative.speech,
      action: "work_crash",
      animation: "pet.work",
      attention: "user",
      moodDelta: { mood: -8 },
    };
  }
  return null;
}

/**
 * Build host-accepted `nimora.pet_directive/1` for a Connector phase.
 * Returns `null` when fail-closed (healthy / unknown → no emit).
 */
export function connectorCompanionDirective(
  phase: ConnectorCompanionPhase,
): StructuredPetDirective | null {
  const narrative = connectorCompanionNarrative(phase);
  if (!narrative.hostAction) return null;
  if (phase === "offline") {
    return {
      spec: "nimora.pet_directive/1",
      speech: narrative.speech,
      action: "rest",
      animation: "pet.idle",
      attention: "idle_scene",
      moodDelta: { mood: -10 },
    };
  }
  if (phase === "degraded") {
    return {
      spec: "nimora.pet_directive/1",
      speech: narrative.speech,
      action: "observe",
      animation: "pet.observe",
      attention: "idle_scene",
      moodDelta: { mood: -6 },
    };
  }
  if (phase === "restored") {
    return {
      spec: "nimora.pet_directive/1",
      speech: narrative.speech,
      action: "celebrate",
      animation: "pet.celebrate",
      attention: "user",
      moodDelta: { mood: 10 },
    };
  }
  if (phase === "event") {
    return {
      spec: "nimora.pet_directive/1",
      speech: narrative.speech,
      action: "observe",
      animation: "pet.observe",
      attention: "notification_area",
      moodDelta: { mood: 4 },
    };
  }
  return null;
}

/**
 * Auto Mode job status → structured pet directive (Running / Paused / Failed).
 * Always emits `nimora.pet_directive/1` for known mapped phases.
 */
export function autoModeCompanionDirective(
  status: string | null | undefined,
  options?: { pauseReason?: string | null; indeterminate?: boolean },
): StructuredPetDirective {
  const companionStatus = companionStatusFromAutoMode(status, options);
  const base = agentCompanionDirective(companionStatus);
  const bubble = companionBubbleFromAutoMode(status, options);
  // Prefer owner-facing Auto Mode speech while keeping host action tokens.
  return {
    ...base,
    speech: bubble.message,
  };
}

/**
 * Resolve sweat / dizzy VFX intent from host directive + lifecycle.
 *
 * Host stores both busy and crash animation as `pet.work`, so lifecycle /
 * action tokens are required to distinguish sweat vs dizzy.
 */
export function resolvePetWorkVisual(input: {
  directiveAction?: string | null;
  directiveAnimation?: string | null;
  lifecycleState?: string | null;
  emotion?: string | null;
}): PetWorkVisual {
  const action = (input.directiveAction ?? "").trim().toLowerCase();
  const animation = (input.directiveAnimation ?? "").trim().toLowerCase();
  const lifecycle = (input.lifecycleState ?? "").trim().toLowerCase();
  const emotion = (input.emotion ?? "").trim().toLowerCase();

  if (
    action === "work_crash"
    || action === "crash"
    || action.includes("crash")
    || action === "recovering"
    || lifecycle === "recovering"
    || emotion === "dizzy"
    || emotion === "dazed"
    || emotion === "wounded"
  ) {
    return "dizzy";
  }

  if (
    action === "work_busy"
    || action === "working"
    || lifecycle === "working"
    || animation === "pet.work"
    || animation === "work"
    || animation === "work_busy"
  ) {
    // pet.work alone without crash signals → sweat (busy / tired).
    return "sweat";
  }

  return "none";
}

/**
 * Subject motion token for overlay / BuiltinPet3D.
 *
 * Prefer crash/dizzy over generic `pet.work` so WorkCrash does not render as sweat.
 * Prefer `work_busy` when lifecycle is working so sweat VFX stays explicit.
 */
export function resolveSubjectMotionForWorkerFeedback(input: {
  directiveAnimation?: string | null;
  directiveAction?: string | null;
  companionAction?: string | null;
  lifecycleState?: string | null;
  emotion?: string | null;
}): string {
  const visualInput: {
    directiveAction?: string | null;
    directiveAnimation?: string | null;
    lifecycleState?: string | null;
    emotion?: string | null;
  } = {};
  if (input.directiveAction !== undefined) visualInput.directiveAction = input.directiveAction;
  if (input.directiveAnimation !== undefined) visualInput.directiveAnimation = input.directiveAnimation;
  if (input.lifecycleState !== undefined) visualInput.lifecycleState = input.lifecycleState;
  if (input.emotion !== undefined) visualInput.emotion = input.emotion;
  const visual = resolvePetWorkVisual(visualInput);
  if (visual === "dizzy") return "work_crash";
  if (visual === "sweat") {
    // Keep explicit busy token when host only published pet.work.
    const anim = (input.directiveAnimation ?? "").trim();
    if (!anim || anim === "pet.work" || anim === "work") return "work_busy";
    return anim;
  }

  for (const candidate of [
    input.directiveAnimation,
    input.directiveAction,
    input.companionAction,
    input.lifecycleState,
  ]) {
    if (typeof candidate === "string" && candidate.trim()) return candidate.trim();
  }
  return "idle";
}
