import { describe, expect, it } from "vitest";
import { reasonLabels, systemContextSensorPresentations } from "./PresenceSettings";
import type { DesktopSnapshot } from "../platform/desktop";

describe("PresenceSettings", () => {
  it("presents every stable desktop-presence reason", () => {
    expect(Object.keys(reasonLabels).sort()).toEqual([
      "base_policy",
      "do_not_disturb",
      "fullscreen",
      "game",
      "safe_mode_recovery",
      "screen_share_privacy",
      "user_forced_hidden",
      "user_forced_visible",
    ]);
    expect(reasonLabels.screen_share_privacy).toBe("屏幕共享隐私");
    expect(reasonLabels.safe_mode_recovery).toBe("安全恢复");
  });

  it("reports missing and degraded native sensors honestly", () => {
    const snapshot = { systemContextSensors: [] } as unknown as DesktopSnapshot;
    expect(systemContextSensorPresentations(snapshot)).toEqual([expect.objectContaining({
      kind: "system_context",
      status: "当前不可用",
      availability: "unavailable",
    })]);
    snapshot.systemContextSensors = [{
      spec: "nimora.system-context-sensor-health/1",
      descriptor: { kind: "fullscreen", source: "operating_system" },
      availability: "degraded",
      consecutiveFailures: 2,
      lastSuccessAtMs: 100,
      lastErrorCode: "fullscreen-sample-failed",
      nextSampleAtMs: 200,
    }];
    expect(systemContextSensorPresentations(snapshot)).toEqual([expect.objectContaining({
      kind: "fullscreen",
      status: "暂时降级",
      detail: "连续 2 次采样失败，正在自动重试",
    })]);
  });

  it("orders independent Windows sensors without collapsing their health", () => {
    const sensor = (kind: "fullscreen" | "do_not_disturb" | "game", availability: "available" | "degraded") => ({
      spec: "nimora.system-context-sensor-health/1" as const,
      descriptor: { kind, source: "operating_system" as const },
      availability,
      consecutiveFailures: availability === "degraded" ? 3 : 0,
      lastSuccessAtMs: availability === "available" ? 100 : null,
      lastErrorCode: availability === "degraded" ? "activity-sample-failed" : null,
      nextSampleAtMs: 200,
    });
    const snapshot = {
      systemContextSensors: [sensor("game", "degraded"), sensor("fullscreen", "available"), sensor("do_not_disturb", "available")],
    } as unknown as DesktopSnapshot;

    expect(systemContextSensorPresentations(snapshot).map(({ kind, status }) => ({ kind, status }))).toEqual([
      { kind: "fullscreen", status: "运行正常" },
      { kind: "do_not_disturb", status: "运行正常" },
      { kind: "game", status: "暂时降级" },
    ]);
  });
});
