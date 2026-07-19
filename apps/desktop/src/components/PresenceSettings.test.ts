import { describe, expect, it } from "vitest";
import { fullscreenSensorStatus, reasonLabels } from "./PresenceSettings";
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
    expect(fullscreenSensorStatus(snapshot)).toBe("全屏感知不可用");
    snapshot.systemContextSensors = [{
      spec: "nimora.system-context-sensor-health/1",
      descriptor: { kind: "fullscreen", source: "operating_system" },
      availability: "degraded",
      consecutiveFailures: 2,
      lastSuccessAtMs: 100,
      lastErrorCode: "fullscreen-sample-failed",
      nextSampleAtMs: 200,
    }];
    expect(fullscreenSensorStatus(snapshot)).toBe("全屏感知暂时降级");
  });
});
