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
