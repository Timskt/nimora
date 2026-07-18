import type { AgentCompanionSignal, AgentCompanionStatus, PetAction } from "../platform/desktop";

export interface AgentCompanionPresentation {
  action: PetAction;
  message: string;
  persistent: boolean;
}

const presentations: Record<AgentCompanionStatus, AgentCompanionPresentation> = {
  thinking: { action: "work", message: "我在陪你一起想…", persistent: true },
  running: { action: "work", message: "正在陪你完成任务", persistent: true },
  waiting_for_confirmation: { action: "idle", message: "有一步需要你确认", persistent: true },
  completed: { action: "celebrate", message: "完成啦，辛苦了！", persistent: false },
  failed: { action: "idle", message: "没关系，我们再试一次", persistent: false },
  cancelled: { action: "idle", message: "任务已停下，我还在这里", persistent: false },
};

export function agentCompanionPresentation(status: AgentCompanionStatus): AgentCompanionPresentation {
  return presentations[status];
}

export function createAgentCompanionSignal(status: AgentCompanionStatus, taskId: string | null = null): AgentCompanionSignal {
  return { spec: "nimora.agent-companion-signal/1", status, taskId, updatedAtMs: Date.now() };
}
