import { describe, expect, it } from "vitest";
import { journalStatusLabel, liveRunOutcomeNotice, statusLabel } from "./AutomationWorkspace";
import type { AutomationRun } from "../platform/desktop";

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
