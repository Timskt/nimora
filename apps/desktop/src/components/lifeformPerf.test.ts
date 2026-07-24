import { describe, expect, it } from "vitest";
import {
  LIFEFORM_PERF_BUDGET,
  LIFEFORM_PERF_EVENT,
  LIFEFORM_PERF_MAX_FRAME_MS,
  buildPerfBudgetCards,
  clampFrameDtMs,
  createLifeformPerfTracker,
  createPerfEmitGate,
  formatCpuPercent,
  formatFps,
  formatFrameMs,
  formatMemoryMb,
  hostProcessBudgetFromSense,
  hostProcessBudgetFromDesktopSnapshot,
  recordFrame,
  shouldWarnIdleBudget,
  summarize,
} from "./lifeformPerf";

describe("lifeformPerf", () => {
  it("summarizes an empty tracker as zeros", () => {
    const tracker = createLifeformPerfTracker();
    expect(summarize(tracker)).toEqual({
      fps: 0,
      avgFrameMs: 0,
      maxFrameMs: 0,
      sampleCount: 0,
    });
    expect(shouldWarnIdleBudget(summarize(tracker))).toBe(false);
  });

  it("tracks steady ~60fps over a rolling window", () => {
    const tracker = createLifeformPerfTracker(1000);
    const frameDtMs = 1000 / 60;
    // 60 samples / 59 intervals stay inside the 1s window without FP prune edge cases
    for (let i = 0; i < 60; i += 1) {
      recordFrame(tracker, i * frameDtMs, frameDtMs);
    }
    const summary = summarize(tracker);
    expect(summary.sampleCount).toBe(60);
    expect(summary.avgFrameMs).toBeCloseTo(frameDtMs, 5);
    expect(summary.maxFrameMs).toBeCloseTo(frameDtMs, 5);
    expect(summary.fps).toBeCloseTo(60, 1);
    expect(shouldWarnIdleBudget(summary)).toBe(false);
  });

  it("detects janky frames and clamps outliers", () => {
    const tracker = createLifeformPerfTracker(1000);
    // Mostly 16.67ms, with a couple of jank spikes
    const frames: Array<[number, number]> = [
      [0, 16.67],
      [16.67, 16.67],
      [33.34, 16.67],
      [50, 50], // jank
      [100, 16.67],
      [116.67, 16.67],
      [133.34, 80], // jank
      [213.34, 16.67],
      [230, 16.67],
      [246.67, 1000], // outlier freeze → clamped
    ];
    for (const [nowMs, dt] of frames) {
      recordFrame(tracker, nowMs, dt);
    }

    expect(clampFrameDtMs(1000)).toBe(LIFEFORM_PERF_MAX_FRAME_MS);
    expect(clampFrameDtMs(Number.NaN)).toBe(0.5);
    expect(clampFrameDtMs(-10)).toBe(0.5);

    const summary = summarize(tracker);
    expect(summary.sampleCount).toBe(frames.length);
    expect(summary.maxFrameMs).toBe(LIFEFORM_PERF_MAX_FRAME_MS);
    expect(summary.avgFrameMs).toBeGreaterThan(22);
    expect(summary.fps).toBeLessThan(45);
    expect(shouldWarnIdleBudget(summary)).toBe(true);
  });

  it("drops samples outside the rolling window", () => {
    const tracker = createLifeformPerfTracker(100);
    recordFrame(tracker, 0, 16);
    recordFrame(tracker, 50, 16);
    recordFrame(tracker, 120, 16);
    const summary = summarize(tracker);
    // 0 is older than 120-100=20, so only 50 and 120 remain
    expect(summary.sampleCount).toBe(2);
    expect(summary.fps).toBeCloseTo(1000 / 70, 1);
  });

  it("warns when fps is low even if avg is acceptable", () => {
    expect(
      shouldWarnIdleBudget({ fps: 40, avgFrameMs: 16, maxFrameMs: 20, sampleCount: 10 }),
    ).toBe(true);
    expect(
      shouldWarnIdleBudget({ fps: 60, avgFrameMs: 25, maxFrameMs: 30, sampleCount: 10 }),
    ).toBe(true);
    expect(
      shouldWarnIdleBudget({ fps: 60, avgFrameMs: 16, maxFrameMs: 20, sampleCount: 10 }),
    ).toBe(false);
  });

  it("formats Chinese-friendly fps / frame / memory labels", () => {
    expect(formatFps({ fps: 59.6, sampleCount: 30 })).toBe("60 帧/秒");
    expect(formatFps({ fps: 0, sampleCount: 0 })).toBe("采样中");
    expect(formatFrameMs(16.666)).toBe("16.7 ms");
    expect(formatFrameMs(120.4)).toBe("120 ms");
    expect(formatMemoryMb(142.3)).toBe("142 MB");
    expect(formatCpuPercent(1.24)).toBe("1.2%");
    expect(LIFEFORM_PERF_EVENT).toBe("nimora:lifeform-perf");
    expect(LIFEFORM_PERF_BUDGET.targetFps).toBe(60);
    expect(LIFEFORM_PERF_BUDGET.memoryTargetMb).toBe(200);
    expect(LIFEFORM_PERF_BUDGET.idleCpuTargetPercent).toBe(2);
  });

  it("throttles perf emit gate to ~500ms", () => {
    const gate = createPerfEmitGate(500);
    expect(gate.shouldEmit(0)).toBe(true);
    expect(gate.shouldEmit(100)).toBe(false);
    expect(gate.shouldEmit(499)).toBe(false);
    expect(gate.shouldEmit(500)).toBe(true);
    expect(gate.shouldEmit(700)).toBe(false);
    expect(gate.shouldEmit(1000)).toBe(true);
    gate.reset();
    expect(gate.shouldEmit(1001)).toBe(true);
  });

  it("builds perf budget cards with warn/ok tones", () => {
    const ok = buildPerfBudgetCards({
      fps: 60,
      avgFrameMs: 16.5,
      maxFrameMs: 18,
      sampleCount: 40,
    });
    expect(ok.map((c) => c.id)).toEqual(["fps", "frame", "idle-budget"]);
    expect(ok.find((c) => c.id === "fps")?.label).toBe("帧率");
    expect(ok.find((c) => c.id === "frame")?.label).toBe("帧耗时");
    expect(ok.find((c) => c.id === "idle-budget")?.label).toBe("空闲预算");
    expect(ok.find((c) => c.id === "fps")?.tone).toBe("ok");
    expect(ok.find((c) => c.id === "idle-budget")?.value).toBe("充裕");

    const warn = buildPerfBudgetCards({
      fps: 38,
      avgFrameMs: 28,
      maxFrameMs: 50,
      sampleCount: 20,
    });
    expect(warn.find((c) => c.id === "fps")?.tone).toBe("warn");
    expect(warn.find((c) => c.id === "frame")?.tone).toBe("warn");
    expect(warn.find((c) => c.id === "idle-budget")?.tone).toBe("warn");
    expect(warn.find((c) => c.id === "idle-budget")?.value).toBe("偏紧");

    const danger = buildPerfBudgetCards({
      fps: 20,
      avgFrameMs: 40,
      maxFrameMs: 80,
      sampleCount: 12,
    });
    expect(danger.find((c) => c.id === "fps")?.tone).toBe("danger");
    expect(danger.find((c) => c.id === "frame")?.tone).toBe("danger");
  });

  it("includes host memory / CPU cards when budget present", () => {
    const cards = buildPerfBudgetCards(
      { fps: 60, avgFrameMs: 16, maxFrameMs: 18, sampleCount: 30 },
      { processRssMb: 142, processCpuPercent: 1.1 },
    );
    expect(cards.map((c) => c.id)).toEqual(["fps", "frame", "idle-budget", "memory", "cpu"]);
    expect(cards.find((c) => c.id === "memory")?.label).toBe("内存");
    expect(cards.find((c) => c.id === "memory")?.value).toBe("142 MB");
    expect(cards.find((c) => c.id === "memory")?.tone).toBe("ok");
    expect(cards.find((c) => c.id === "cpu")?.label).toBe("空闲 CPU");
    expect(cards.find((c) => c.id === "cpu")?.tone).toBe("ok");

    const heavy = buildPerfBudgetCards(null, { processRssMb: 250, idleCpuPercent: 5 });
    expect(heavy.map((c) => c.id)).toEqual(["memory", "cpu"]);
    expect(heavy.find((c) => c.id === "memory")?.tone).toBe("warn");
    expect(heavy.find((c) => c.id === "cpu")?.tone).toBe("warn");
  });

  it("reads host process budget defensively from sense-like objects", () => {
    expect(hostProcessBudgetFromSense(null)).toBeNull();
    expect(hostProcessBudgetFromSense({})).toBeNull();
    expect(hostProcessBudgetFromSense({ processRssMb: 180, processCpuPercent: 0.8 })).toEqual({
      processRssMb: 180,
      processCpuPercent: 0.8,
    });
    expect(
      hostProcessBudgetFromSense({
        processBudget: { rssMb: 210, cpuPercent: 3.2 },
      }),
    ).toEqual({
      processRssMb: 210,
      processCpuPercent: 3.2,
    });
    // Native ProcessBudgetSnapshot shape (rssMb + cpuPercentApprox).
    expect(
      hostProcessBudgetFromSense({
        rssBytes: 96 * 1024 * 1024,
        rssMb: 96,
        cpuPercentApprox: 0.4,
        withinMemoryBudget: true,
        observedAtMs: 1,
      }),
    ).toEqual({
      processRssMb: 96,
      processCpuPercent: 0.4,
    });
    expect(
      hostProcessBudgetFromSense({
        processBudget: { rssMb: 140, cpuPercentApprox: 1.1 },
      }),
    ).toEqual({
      processRssMb: 140,
      processCpuPercent: 1.1,
    });
  });

  it("prefers DesktopSnapshot.processBudget over lifeformSense", () => {
    expect(hostProcessBudgetFromDesktopSnapshot(null)).toBeNull();
    expect(
      hostProcessBudgetFromDesktopSnapshot({
        lifeformSense: { notificationUnread: true, batteryPercent: 50 },
        processBudget: {
          rssBytes: 180 * 1024 * 1024,
          rssMb: 180,
          cpuPercentApprox: 1.5,
          withinMemoryBudget: true,
          observedAtMs: 9,
        },
      }),
    ).toEqual({
      processRssMb: 180,
      processCpuPercent: 1.5,
    });
    // lifeformSense alone still works when processBudget is absent.
    expect(
      hostProcessBudgetFromDesktopSnapshot({
        lifeformSense: { processRssMb: 90, processCpuPercent: 0.2 },
      }),
    ).toEqual({
      processRssMb: 90,
      processCpuPercent: 0.2,
    });
  });

  it("skips empty summary cards", () => {
    expect(buildPerfBudgetCards(null)).toEqual([]);
    expect(buildPerfBudgetCards({ fps: 0, avgFrameMs: 0, maxFrameMs: 0, sampleCount: 0 })).toEqual([]);
  });
});
