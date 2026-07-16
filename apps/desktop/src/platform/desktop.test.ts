import { describe, expect, it, vi } from "vitest";
import { createDesktopApi } from "./desktop";

describe("desktop platform adapter", () => {
  it("keeps browser preview fully offline", async () => {
    const api = createDesktopApi(false);
    expect(api.native).toBe(false);
    expect((await api.snapshot()).pet.name).toBe("Aster");
    await expect(api.drainEvents()).resolves.toEqual([]);
    expect((await api.profiles()).profiles[0]?.name).toBe("Default");
    await expect(api.playAction("celebrate")).resolves.toBeNull();
  });

  it("maps typed calls to the Tauri command contract", async () => {
    const invoke = vi.fn(async () => null);
    const api = createDesktopApi(true, invoke);
    await api.drainEvents();
    await api.profiles();
    const policy = {
      alwaysOnTop: true,
      clickThrough: false,
      soundEnabled: true,
      proactiveFrequency: 10,
    };
    await api.createProfile("Focus", policy);
    await api.switchProfile("00000000-0000-4000-8000-000000000010");
    await api.movePet(24, 42);
    await api.playAction("work");
    await api.setClickThrough(true);
    expect(invoke.mock.calls).toEqual([
      ["drain_runtime_events"],
      ["profile_snapshot"],
      ["create_profile", { name: "Focus", policy }],
      ["switch_profile", { profileId: "00000000-0000-4000-8000-000000000010" }],
      ["move_pet", { request: { x: 24, y: 42 } }],
      ["play_pet_action", { action: "work" }],
      ["set_click_through", { enabled: true }],
    ]);
  });
});
