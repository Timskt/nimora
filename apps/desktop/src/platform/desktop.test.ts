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
    const startDragging = vi.fn(async () => undefined);
    const api = createDesktopApi(true, invoke, startDragging);
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
    await api.enterSafeMode();
    await api.exitSafeMode();
    await api.movePet(24, 42);
    await api.playAction("work");
    await api.clickPet(12, 24, "left");
    await api.dragPet();
    await api.setClickThrough(true);
    expect(invoke.mock.calls).toEqual([
      ["drain_runtime_events"],
      ["profile_snapshot"],
      ["create_profile", { name: "Focus", policy }],
      ["switch_profile", { profileId: "00000000-0000-4000-8000-000000000010" }],
      ["enter_safe_mode"],
      ["exit_safe_mode"],
      ["move_pet", { request: { x: 24, y: 42 } }],
      ["play_pet_action", { action: "work" }],
      ["click_pet", { request: { x: 12, y: 24, button: "left" } }],
      ["begin_pet_drag"],
      ["finish_pet_drag"],
      ["set_click_through", { enabled: true }],
    ]);
    expect(startDragging).toHaveBeenCalledOnce();
  });

  it("recovers runtime drag state when native dragging fails", async () => {
    const invoke = vi.fn(async () => null);
    const api = createDesktopApi(true, invoke, async () => {
      throw new Error("native drag failed");
    });
    await expect(api.dragPet()).rejects.toThrow("native drag failed");
    expect(invoke.mock.calls).toEqual([
      ["begin_pet_drag"],
      ["finish_pet_drag"],
    ]);
  });
});
