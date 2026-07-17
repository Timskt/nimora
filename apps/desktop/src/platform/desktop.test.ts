import { describe, expect, it, vi } from "vitest";
import { createDesktopApi, type UserProgramExecutionReceipt } from "./desktop";

describe("desktop platform adapter", () => {
  it("keeps browser preview fully offline", async () => {
    const api = createDesktopApi(false);
    expect(api.native).toBe(false);
    expect((await api.snapshot()).pet.name).toBe("Aster");
    await expect(api.drainEvents()).resolves.toEqual([]);
    await expect(api.outboxSnapshot()).resolves.toEqual({ pending: 0, leased: 0, delivered: 0, deadLetter: 0 });
    expect((await api.previewDiagnosticReport()).privacy).toEqual({
      includesLogs: false,
      includesUserContent: false,
      includesSecrets: false,
      includesFilePaths: false,
      automaticallyUploaded: false,
    });
    expect((await api.previewDiagnosticReport()).sources).toEqual({ eventCount: 2, eventRetentionDays: 14 });
    expect((await api.profiles()).profiles[0]?.name).toBe("Default");
    expect((await api.agentCatalog()).tools).toHaveLength(13);
    expect((await api.agentCatalog()).providers).toHaveLength(2);
    const result = await api.runLocalAgent("离线检查");
    expect(result.usage?.costMicrounits).toBe(0);
    expect((await api.agentHistory()).records[0]).toMatchObject({
      task: { id: result.task.id },
      prompt: "离线检查",
      response: "[model:echo-v1] 离线检查",
    });
    await expect(api.playAction("celebrate")).resolves.toBeNull();
  });

  it("keeps module Agent results in the user program receipt contract", () => {
    const receipt: UserProgramExecutionReceipt = {
      executionId: "018f0000-0000-7000-8000-000000000001",
      responses: [],
      agentResults: [{
        spec: "nimora.desktop-agent-result/1",
        status: "completed",
        task: { id: "018f0000-0000-7000-8000-000000000002", status: "completed", providerId: "provider:deterministic-local" },
        content: "summary",
        finishReason: "stop",
        usage: { inputTokens: 8, outputTokens: 2, costMicrounits: 0 },
        pendingTools: [],
      }],
    };
    expect(receipt.agentResults[0]?.pendingTools).toEqual([]);
  });

  it("previews automation plans without executing commands", async () => {
    const api = createDesktopApi(false);
    const definition = {
      spec: "nimora.automation/1" as const,
      id: "local.focus.on-build",
      name: "Build companion",
      enabled: true,
      trigger: { eventType: "dev.build.finished" },
      conditions: [{ pointer: "/succeeded", equals: true }],
      actions: [{ id: "celebrate", command: "pet.animation.play", arguments: { action: "celebrate" }, risk: "low" as const, retrySafe: true, idempotencyKey: "preview", compensation: null }],
      policy: { timeoutMs: 5_000, failure: "stop" as const },
    };
    const planned = await api.testAutomation(definition, "dev.build.finished", { succeeded: true });
    expect(planned).toMatchObject({ mode: "dry_run", status: "planned" });
    expect(planned.steps).toEqual([expect.objectContaining({ command: "pet.animation.play", attempts: 0, status: "pending" })]);
    const skipped = await api.testAutomation(definition, "dev.build.finished", { succeeded: false });
    expect(skipped.status).toBe("condition_not_matched");
    expect(skipped.steps).toEqual([]);
  });

  it("preserves an explicitly selected preview provider and model", async () => {
    const api = createDesktopApi(false);
    const result = await api.runLocalAgent("检查模型选择", "provider:preview-scripted", "qwen3:8b");

    expect(result.task.providerId).toBe("provider:preview-scripted");
    expect(result.content).toBe("[qwen3:8b] 检查模型选择");
  });

  it("previews an atomic provider tool turn through confirmation", async () => {
    const api = createDesktopApi(false);
    const waiting = await api.runLocalAgent("请移动桌宠并展示工具确认");

    expect(waiting.status).toBe("waitingForConfirmation");
    expect(waiting.task.providerId).toBe("provider:preview-scripted");
    expect(waiting.pendingTools.map((tool) => tool.invocation.toolId)).toEqual([
      "pet.animation.play",
      "pet.position.move",
    ]);
    const firstTool = waiting.pendingTools.at(0);
    if (!firstTool) throw new Error("expected the first preview tool");

    const afterFirstApproval = await api.confirmAgentRunTool(firstTool.invocation.invocationId);
    expect(afterFirstApproval.status).toBe("waitingForConfirmation");
    expect(afterFirstApproval.pendingTools).toHaveLength(1);
    const remainingTool = afterFirstApproval.pendingTools.at(0);
    if (!remainingTool) throw new Error("expected the remaining preview tool");
    expect(remainingTool.invocation.toolId).toBe("pet.position.move");

    const completed = await api.confirmAgentRunTool(remainingTool.invocation.invocationId);
    expect(completed.status).toBe("completed");
    expect(completed.task.providerId).toBe("provider:preview-scripted");
    expect(completed.pendingTools).toEqual([]);
    expect(completed.content).toBe("模块操作已经安全完成。");
    expect((await api.agentHistory()).records[0]).toMatchObject({
      task: { id: completed.task.id },
      prompt: "请移动桌宠并展示工具确认",
      response: "模块操作已经安全完成。",
    });
  });

  it("paginates and deletes preview agent history", async () => {
    const api = createDesktopApi(false);
    const first = await api.runLocalAgent("第一条");
    const firstHistory = await api.agentHistory(1);
    while (Date.now() <= firstHistory.records[0]!.task.createdAtMs) {
      await new Promise((resolve) => setTimeout(resolve, 1));
    }
    const second = await api.runLocalAgent("第二条");

    const latest = await api.agentHistory(1);
    expect(latest.records.map((record) => record.task.id)).toEqual([second.task.id]);
    const older = await api.agentHistory(1, {
      createdAtMs: latest.records[0]!.task.createdAtMs,
      taskId: latest.records[0]!.task.id,
    });
    expect(older.records.map((record) => record.task.id)).toEqual([first.task.id]);
    await expect(api.deleteAgentHistory(first.task.id)).resolves.toBe(1);
    await expect(api.deleteAgentHistory()).resolves.toBe(1);
    await expect(api.agentHistory()).resolves.toMatchObject({ records: [] });
  });

  it("cancels the whole preview provider turn when one tool is rejected", async () => {
    const api = createDesktopApi(false);
    const waiting = await api.runLocalAgent("展示工具确认");
    const rejectedTool = waiting.pendingTools.at(0);
    if (!rejectedTool) throw new Error("expected a preview tool to reject");

    await api.rejectAgentTool(rejectedTool.invocation.invocationId);

    const restarted = await api.runLocalAgent("展示工具确认");
    expect(restarted.pendingTools).toHaveLength(2);
  });

  it("maps typed calls to the Tauri command contract", async () => {
    const invoke = vi.fn(async (command: string) => command === "delete_agent_history"
      ? { spec: "nimora.desktop-agent-history-delete/1", deleted: 1 }
      : null);
    const startDragging = vi.fn(async () => undefined);
    const api = createDesktopApi(true, invoke, startDragging);
    await api.automationAgentTaskStatus("018f0000-0000-7000-8000-000000000008");
    await api.automationRunAgentTasks("018f0000-0000-7000-8000-000000000009");
    await api.cancelAutomationRun("018f0000-0000-7000-8000-000000000011");
    await api.cancelAgentTask("018f0000-0000-7000-8000-000000000010");
    await api.agentCatalog();
    await api.agentHistory(25);
    await api.deleteAgentHistory("018f0000-0000-7000-8000-000000000007");
    await api.runLocalAgent("检查本地能力");
    await api.prepareAgentTool("pet.animation.play", { action: "celebrate" });
    await api.confirmAgentTool("018f0000-0000-7000-8000-000000000004");
    await api.confirmAgentRunTool("018f0000-0000-7000-8000-000000000006");
    await api.rejectAgentTool("018f0000-0000-7000-8000-000000000005");
    await api.drainEvents();
    await api.outboxSnapshot();
    await api.backupHealth();
    await api.createBackup();
    await api.requestDatabaseRestore("runtime-1700000000000.sqlite3");
    await api.previewDiagnosticReport();
    await api.exportDiagnostics("/tmp/support.nimora-diagnostics.zip", true);
    await api.profiles();
    const policy = {
      mode: "focus" as const,
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
    await api.assetCatalog();
    await api.activeCharacter();
    await api.activeCharacterRenderer();
    await api.activateCharacter("character.example.mochi");
    await api.previewAsset({ sourcePath: "/tmp/nimora-import" });
    await api.exportAsset({
      sourcePath: "/tmp/nimora-source",
      destinationPath: "/tmp/mochi.nimora",
    });
    await api.inspectModel({ sourcePath: "/tmp/character.glb" });
    await api.importModel({
      sourcePath: "/tmp/character.glb",
      assetId: "character.local.aurora",
      name: "Aurora",
      license: "CC-BY-4.0",
      animationMap: {},
    });
    await api.installAsset({
      sourcePath: "/tmp/nimora-import",
    });
    await api.rollbackAsset("character.example.mochi");
    const manifest = {
      id: "studio.example.focus",
      version: "1.0.0",
      capabilities: ["read-pet-state", "subscribe-events", "invoke-safe-commands", "invoke-agent-tasks"] as const,
      subscriptions: ["pet.example.clicked"],
      eventConcurrency: "serial" as const,
      eventQueueCapacity: 16,
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
    await api.startUserProgramEventLoop(subscriptionId);
    await api.userProgramEventSessionStatus(subscriptionId);
    await api.closeUserProgramEventSession(subscriptionId);
    await api.startUserProgram(manifest);
    await api.executeUserProgram(manifest, "({ agentTasks: [] })");
    await api.executeInstalledUserProgram(manifest.id);
    const envelope = {
      executionId: "018f0000-0000-7000-8000-000000000001",
      traceId: "018f0000-0000-7000-8000-000000000002",
      idempotencyKey: "action-1",
      request: { type: "invokeCommand" as const, command: "safe.pet.animate", arguments: { action: "work" } },
    };
    await api.invokeUserProgramCapability(envelope);
    await api.invokeUserProgramCapability({
      ...envelope,
      request: { type: "readProfileState" as const },
    });
    await api.stopUserProgram(envelope.executionId);
    expect(invoke.mock.calls).toEqual([
      ["automation_agent_task_status", { taskId: "018f0000-0000-7000-8000-000000000008" }],
      ["automation_run_agent_tasks", { runId: "018f0000-0000-7000-8000-000000000009" }],
      ["cancel_automation_run", { runId: "018f0000-0000-7000-8000-000000000011" }],
      ["cancel_agent_task", { taskId: "018f0000-0000-7000-8000-000000000010" }],
      ["agent_catalog"],
      ["agent_history_list", { request: { beforeCreatedAtMs: null, beforeTaskId: null, limit: 25 } }],
      ["delete_agent_history", { request: { taskId: "018f0000-0000-7000-8000-000000000007" } }],
      ["run_local_agent", { request: { prompt: "检查本地能力", providerId: "provider:deterministic-local", model: "model:echo-v1" } }],
      ["prepare_agent_tool", { request: { toolId: "pet.animation.play", arguments: { action: "celebrate" } } }],
      ["confirm_agent_tool", { request: { invocationId: "018f0000-0000-7000-8000-000000000004" } }],
      ["confirm_agent_run_tool", { request: { invocationId: "018f0000-0000-7000-8000-000000000006" } }],
      ["reject_agent_tool", { request: { invocationId: "018f0000-0000-7000-8000-000000000005" } }],
      ["drain_runtime_events"],
      ["outbox_snapshot"],
      ["backup_health"],
      ["create_backup"],
      ["request_database_restore", { backupId: "runtime-1700000000000.sqlite3" }],
      ["preview_diagnostic_report"],
      ["export_diagnostics", { request: { destinationPath: "/tmp/support.nimora-diagnostics.zip", includeEvents: true } }],
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
      ["asset_catalog"],
      ["active_character"],
      ["active_character_renderer"],
      ["activate_character", { assetId: "character.example.mochi" }],
      ["preview_asset", { request: { sourcePath: "/tmp/nimora-import" } }],
      ["export_asset", { request: {
        sourcePath: "/tmp/nimora-source",
        destinationPath: "/tmp/mochi.nimora",
      } }],
      ["inspect_model", { request: { sourcePath: "/tmp/character.glb" } }],
      ["import_model", { request: {
        sourcePath: "/tmp/character.glb",
        assetId: "character.local.aurora",
        name: "Aurora",
        license: "CC-BY-4.0",
        animationMap: {},
      } }],
      ["install_asset", { request: {
        sourcePath: "/tmp/nimora-import",
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
      ["start_user_program_event_loop", { subscriptionId }],
      ["user_program_event_session_status", { subscriptionId }],
      ["close_user_program_event_session", { subscriptionId }],
      ["start_user_program", { manifest }],
      ["execute_user_program", { manifest, source: "({ agentTasks: [] })" }],
      ["execute_installed_user_program", { programId: manifest.id }],
      ["invoke_user_program_capability", { envelope }],
      ["invoke_user_program_capability", { envelope: {
        ...envelope,
        request: { type: "readProfileState" },
      } }],
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
