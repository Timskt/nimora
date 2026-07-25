import { describe, expect, it } from "vitest";
import {
  autoModeJobStatusLabel,
  formatAutoModeJobTime,
  isTerminalFailureJob,
  partitionAutoModeJobs,
  summarizeAutoModeJob,
} from "./AutoModeJobHistoryPanel";
import type { DesktopAutoModeJobSnapshot } from "../platform/desktop";

function job(overrides: Partial<DesktopAutoModeJobSnapshot>): DesktopAutoModeJobSnapshot {
  return {
    spec: "nimora.desktop-auto-mode-job/1",
    jobId: "job-1",
    sessionId: "session-1",
    status: "running",
    turnsExecuted: 3,
    cacheHits: 1,
    checkpointSequence: 3,
    pauseReason: null,
    errorCode: null,
    startedAtMs: 1_000,
    updatedAtMs: 2_000,
    ...overrides,
  };
}

describe("autoModeJobStatusLabel", () => {
  it("maps running/failed/indeterminate to Chinese phase labels", () => {
    expect(autoModeJobStatusLabel("running")).toBe("执行中");
    expect(autoModeJobStatusLabel("failed")).toBe("失败");
    expect(autoModeJobStatusLabel("indeterminate")).toBe("结果未知");
  });
});

describe("isTerminalFailureJob", () => {
  it("flags failed and indeterminate jobs", () => {
    expect(isTerminalFailureJob(job({ status: "failed" }))).toBe(true);
    expect(isTerminalFailureJob(job({ status: "indeterminate" }))).toBe(true);
  });

  it("does not flag running/paused/completed jobs", () => {
    expect(isTerminalFailureJob(job({ status: "running" }))).toBe(false);
    expect(isTerminalFailureJob(job({ status: "paused" }))).toBe(false);
    expect(isTerminalFailureJob(job({ status: "completed" }))).toBe(false);
  });
});

describe("summarizeAutoModeJob", () => {
  it("always reports turns, cache hits and checkpoint", () => {
    const summary = summarizeAutoModeJob(job({}));
    expect(summary).toContain("3 轮");
    expect(summary).toContain("缓存命中 1");
    expect(summary).toContain("检查点 #3");
  });

  it("surfaces pause reason and error code when present", () => {
    const summary = summarizeAutoModeJob(job({ pauseReason: "confirmation_required", errorCode: "provider_unreachable" }));
    expect(summary).toContain("暂停原因：confirmation_required");
    expect(summary).toContain("错误码：provider_unreachable");
  });
});

describe("formatAutoModeJobTime", () => {
  it("returns a placeholder for non-positive timestamps", () => {
    expect(formatAutoModeJobTime(0)).toBe("时间未知");
    expect(formatAutoModeJobTime(Number.NaN)).toBe("时间未知");
  });

  it("formats a real timestamp", () => {
    expect(formatAutoModeJobTime(1_700_000_000_000)).not.toBe("时间未知");
  });
});

describe("partitionAutoModeJobs", () => {
  it("splits failing jobs from the rest, preserving order within groups", () => {
    const jobs = [
      job({ jobId: "a", status: "running" }),
      job({ jobId: "b", status: "failed" }),
      job({ jobId: "c", status: "paused" }),
      job({ jobId: "d", status: "indeterminate" }),
    ];
    const { failed, others } = partitionAutoModeJobs(jobs);
    expect(failed.map((entry) => entry.jobId)).toEqual(["b", "d"]);
    expect(others.map((entry) => entry.jobId)).toEqual(["a", "c"]);
  });
});
