import { describe, expect, it } from "vitest";
import { agentTaskCancellable, agentTaskStatusLabel, agentTaskStatusRefreshNotice, journalStatusLabel, liveRunCancellable, liveRunOutcomeNotice, mergeAgentTaskStatus, statusLabel } from "./AutomationWorkspace";
import type { AutomationAgentJournalEntry, AutomationRun } from "../platform/desktop";

function run(status: AutomationRun["status"]): AutomationRun {
  return {
    spec: "nimora.automation-run/1",
    runId: "run-1",
    automationId: "local.focus.on-build",
    traceId: "trace-1",
    eventId: "event-1",
    mode: "live",
    status,
    steps: [],
    reason: null,
  };
}

describe("statusLabel", () => {
  it("maps run statuses to Chinese labels", () => {
    expect(statusLabel("planned")).toBe("计划验证通过");
    expect(statusLabel("waiting_for_approval")).toBe("等待参数级批准");
    expect(statusLabel("succeeded")).toBe("运行成功");
    expect(statusLabel("compensation_failed")).toBe("动作与补偿均失败");
  });
});

describe("journalStatusLabel", () => {
  it("maps journal statuses to Chinese labels", () => {
    expect(journalStatusLabel("running")).toBe("运行中");
    expect(journalStatusLabel("completed")).toBe("已完成");
    expect(journalStatusLabel("interrupted")).toBe("已中断");
  });
});

describe("liveRunOutcomeNotice", () => {
  it("explains a successful real run wrote to the audit journal", () => {
    expect(liveRunOutcomeNotice(run("succeeded"))).toContain("写入审计记录");
  });

  it("clarifies that a waiting-for-approval run produced no side effect yet", () => {
    const notice = liveRunOutcomeNotice(run("waiting_for_approval"));
    expect(notice).toContain("批准队列");
    expect(notice).toContain("尚未产生真实副作用");
  });

  it("covers non-executing terminal statuses", () => {
    expect(liveRunOutcomeNotice(run("condition_not_matched"))).toContain("条件未满足");
    expect(liveRunOutcomeNotice(run("trigger_not_matched"))).toContain("触发器未匹配");
    expect(liveRunOutcomeNotice(run("timed_out"))).toContain("超时");
    expect(liveRunOutcomeNotice(run("compensation_failed"))).toContain("补偿均失败");
    expect(liveRunOutcomeNotice(run("failed"))).toContain("逆序补偿");
  });
});


function agentTask(status: AutomationAgentJournalEntry["status"]): AutomationAgentJournalEntry {
  return {
    spec: "nimora.automation-agent-journal/1",
    runId: "run-1",
    idempotencyKey: "run-1:agent.task.run",
    admission: {
      spec: "nimora.agent-task-admission/1",
      task: {
        spec: "nimora.agent-task/1",
        id: "run-1-agent-0",
        traceId: "trace-1",
        requester: "automation",
        providerId: "provider:local",
        status: "running",
      },
      rootTaskId: "run-1",
      parentTaskId: null,
      callDepth: 0,
    },
    model: "preview.local.reasoner",
    status,
    submittedAtMs: 1_000,
    updatedAtMs: 2_000,
    error: null,
  };
}

describe("agentTaskStatusLabel", () => {
  it("maps agent task statuses to Chinese labels", () => {
    expect(agentTaskStatusLabel("submitted")).toBe("已提交");
    expect(agentTaskStatusLabel("waiting_for_confirmation")).toBe("等待确认");
    expect(agentTaskStatusLabel("completed")).toBe("已完成");
    expect(agentTaskStatusLabel("failed")).toBe("已失败");
    expect(agentTaskStatusLabel("cancelled")).toBe("已取消");
    expect(agentTaskStatusLabel("interrupted")).toBe("已中断");
  });
});

describe("agentTaskCancellable", () => {
  it("only allows cancellation while a task is still in flight", () => {
    expect(agentTaskCancellable("submitted")).toBe(true);
    expect(agentTaskCancellable("waiting_for_confirmation")).toBe(true);
    expect(agentTaskCancellable("completed")).toBe(false);
    expect(agentTaskCancellable("failed")).toBe(false);
    expect(agentTaskCancellable("cancelled")).toBe(false);
    expect(agentTaskCancellable("interrupted")).toBe(false);
  });
});

describe("liveRunCancellable", () => {
  it("allows cancelling in-flight runs regardless of child tasks", () => {
    expect(liveRunCancellable("waiting_for_approval", [])).toBe(true);
    expect(liveRunCancellable("planned", [])).toBe(true);
  });

  it("allows cancelling a terminal run only when a child task is still cancellable", () => {
    expect(liveRunCancellable("succeeded", [])).toBe(false);
    expect(liveRunCancellable("succeeded", [agentTask("completed")])).toBe(false);
    expect(liveRunCancellable("succeeded", [agentTask("submitted")])).toBe(true);
    expect(liveRunCancellable("failed", [agentTask("waiting_for_confirmation")])).toBe(true);
  });

  it("never offers cancellation for non-cancellable terminal runs", () => {
    expect(liveRunCancellable("condition_not_matched", [agentTask("submitted")])).toBe(false);
    expect(liveRunCancellable("cancelled", [agentTask("submitted")])).toBe(false);
  });
});


function agentTaskWithId(status: AutomationAgentJournalEntry["status"], id: string): AutomationAgentJournalEntry {
  const base = agentTask(status);
  return { ...base, admission: { ...base.admission, task: { ...base.admission.task, id } } };
}

describe("mergeAgentTaskStatus", () => {
  it("replaces the matching task in place when a fresh entry returns", () => {
    const tasks = [agentTaskWithId("submitted", "task-a"), agentTaskWithId("submitted", "task-b")];
    const latest = agentTaskWithId("completed", "task-a");
    const merged = mergeAgentTaskStatus(tasks, latest, "task-a");
    expect(merged).toHaveLength(2);
    expect(merged[0]!.status).toBe("completed");
    expect(merged[1]!.status).toBe("submitted");
  });

  it("drops the task when the ledger no longer has it", () => {
    const tasks = [agentTaskWithId("submitted", "task-a"), agentTaskWithId("submitted", "task-b")];
    const merged = mergeAgentTaskStatus(tasks, null, "task-a");
    expect(merged).toHaveLength(1);
    expect(merged[0]!.admission.task.id).toBe("task-b");
  });

  it("appends a late-arriving entry that was not yet tracked", () => {
    const tasks = [agentTaskWithId("submitted", "task-a")];
    const latest = agentTaskWithId("waiting_for_confirmation", "task-c");
    const merged = mergeAgentTaskStatus(tasks, latest, "task-c");
    expect(merged).toHaveLength(2);
    expect(merged[1]!.admission.task.id).toBe("task-c");
  });
});

describe("agentTaskStatusRefreshNotice", () => {
  it("reports removal when the ledger no longer tracks the task", () => {
    expect(agentTaskStatusRefreshNotice("submitted", null)).toContain("移除");
  });

  it("announces a status change with the new label", () => {
    const latest = agentTaskWithId("completed", "task-a");
    expect(agentTaskStatusRefreshNotice("submitted", latest)).toContain("已完成");
  });

  it("confirms no change when the status is unchanged", () => {
    const latest = agentTaskWithId("submitted", "task-a");
    expect(agentTaskStatusRefreshNotice("submitted", latest)).toContain("无变化");
  });
});
