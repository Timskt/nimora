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
    await api.installAsset({
      assetId: "character.example.mochi",
      sourcePath: "/tmp/nimora-import",
      files: [
        { relativePath: "manifest.json", bytes: 12, sha256: "a".repeat(64) },
        { relativePath: "sprites/idle.webp", bytes: 42, sha256: "b".repeat(64) },
      ],
    });
    await api.rollbackAsset("character.example.mochi");
    const manifest = {
      id: "studio.example.focus",
      version: "1.0.0",
      capabilities: ["read-pet-state", "subscribe-events", "invoke-safe-commands"] as const,
      subscriptions: ["pet.example.clicked"],
      commands: ["safe.example.notify"],
      timeoutMs: 5_000,
      memoryBytes: 8 * 1024 * 1024,
    };
    await api.validateUserProgram(manifest);
    const programRequest = {
      sourcePath: "/tmp/nimora-program",
      manifest,
      files: [
        { relativePath: "manifest.json", bytes: 512, sha256: "c".repeat(64) },
        { relativePath: "main.js", bytes: 64, sha256: "d".repeat(64) },
      ],
    };
    await api.installUserProgram(programRequest);
    await api.rollbackUserProgram(manifest.id);
    await api.userProgramPermissionStatus(manifest.id);
    await api.grantUserProgramPermissions(manifest.id);
    await api.revokeUserProgramPermissions(manifest.id);
    const subscriptionId = "018f0000-0000-7000-8000-000000000003";
    await api.openUserProgramEventSession(manifest.id);
    await api.drainUserProgramEvents(subscriptionId);
    await api.executeNextUserProgramEvent(subscriptionId);
    await api.closeUserProgramEventSession(subscriptionId);
    await api.startUserProgram(manifest);
    await api.executeUserProgram(manifest, "({ commands: [] })");
    await api.executeInstalledUserProgram(manifest.id);
    const envelope = {
      executionId: "018f0000-0000-7000-8000-000000000001",
      traceId: "018f0000-0000-7000-8000-000000000002",
      idempotencyKey: "action-1",
      request: { type: "invokeCommand" as const, command: "safe.pet.animate", arguments: { action: "work" } },
    };
    await api.invokeUserProgramCapability(envelope);
    await api.stopUserProgram(envelope.executionId);
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
      ["install_asset", { request: {
        assetId: "character.example.mochi",
        sourcePath: "/tmp/nimora-import",
        files: [
          { relativePath: "manifest.json", bytes: 12, sha256: "a".repeat(64) },
          { relativePath: "sprites/idle.webp", bytes: 42, sha256: "b".repeat(64) },
        ],
      } }],
      ["rollback_asset", { assetId: "character.example.mochi" }],
      ["validate_user_program", { manifest }],
      ["install_user_program", { request: programRequest }],
      ["rollback_user_program", { programId: manifest.id }],
      ["user_program_permission_status", { programId: manifest.id }],
      ["grant_user_program_permissions", { programId: manifest.id }],
      ["revoke_user_program_permissions", { programId: manifest.id }],
      ["open_user_program_event_session", { programId: manifest.id }],
      ["drain_user_program_events", { subscriptionId }],
      ["execute_next_user_program_event", { subscriptionId }],
      ["close_user_program_event_session", { subscriptionId }],
      ["start_user_program", { manifest }],
      ["execute_user_program", { manifest, source: "({ commands: [] })" }],
      ["execute_installed_user_program", { programId: manifest.id }],
      ["invoke_user_program_capability", { envelope }],
      ["stop_user_program", { executionId: envelope.executionId }],
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
