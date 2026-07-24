import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  formatSampleProgramPlanJson,
  labelForUserCodeCapability,
  samplePetDirectiveProgramPlan,
  suggestAnimationMap,
  USER_CODE_MANIFEST_CAPABILITY_LABELS,
  USER_CODE_READ_SURFACE_LABELS,
  USER_CODE_SAFE_COMMAND_LABELS,
  userCodeCapabilityChips,
  UserCodeCapabilityMatrix,
} from "./CreatorStudio";

describe("suggestAnimationMap", () => {
  it("maps recognized names with action-specific loop behavior", () => {
    expect(suggestAnimationMap(["Idle", "WalkCycle", "MorningStretch", "FriendlyWave"])).toEqual({
      "pet.idle": { animation: "Idle", looped: true },
      "pet.walk": { animation: "WalkCycle", looped: true },
      "pet.stretch": { animation: "MorningStretch", looped: false },
      "pet.click": { animation: "FriendlyWave", looped: false },
    });
  });

  it("does not invent bindings for unrelated names", () => {
    expect(suggestAnimationMap(["Take 001", "Animation"])).toEqual({});
  });
});

describe("user code capability labels", () => {
  it("maps manifest capabilities, safe commands, and read surfaces", () => {
    expect(USER_CODE_MANIFEST_CAPABILITY_LABELS["read-pet-state"]).toBe("读取宠物状态");
    expect(USER_CODE_MANIFEST_CAPABILITY_LABELS["invoke-agent-tasks"]).toBe("调用 Agent 任务");
    expect(USER_CODE_SAFE_COMMAND_LABELS["safe.pet.animate"]).toBe("播放动作");
    expect(USER_CODE_SAFE_COMMAND_LABELS["safe.pet.care"]).toBe("照料互动");
    expect(USER_CODE_SAFE_COMMAND_LABELS["safe.pet.move"]).toBe("移动位置");
    expect(USER_CODE_SAFE_COMMAND_LABELS["safe.pet.directive"]).toBe("结构化指令");
    expect(USER_CODE_READ_SURFACE_LABELS["pet.state"]).toBe("宠物状态");
    expect(USER_CODE_READ_SURFACE_LABELS["pet.action.catalog"]).toBe("动作目录");
    expect(USER_CODE_READ_SURFACE_LABELS["character.state"]).toBe("角色状态");
    expect(USER_CODE_READ_SURFACE_LABELS["program.catalog"]).toBe("程序目录");
    expect(USER_CODE_READ_SURFACE_LABELS["runtime.health"]).toBe("运行时健康");
  });

  it("resolves labels via pure helper and rejects unknowns", () => {
    expect(labelForUserCodeCapability("safe.pet.directive")).toBe("结构化指令");
    expect(labelForUserCodeCapability("pet.action.catalog")).toBe("动作目录");
    expect(labelForUserCodeCapability("invoke-safe-commands")).toBe("调用安全命令");
    expect(labelForUserCodeCapability("fs.read")).toBeNull();
  });

  it("lists chips covering read surfaces, pet commands, agent, and manifest", () => {
    const chips = userCodeCapabilityChips();
    const ids = chips.map((chip) => chip.id);
    expect(ids).toContain("pet.state");
    expect(ids).toContain("pet.action.catalog");
    expect(ids).toContain("character.state");
    expect(ids).toContain("program.catalog");
    expect(ids).toContain("runtime.health");
    expect(ids).toContain("safe.pet.animate");
    expect(ids).toContain("safe.pet.care");
    expect(ids).toContain("safe.pet.move");
    expect(ids).toContain("safe.pet.directive");
    expect(ids).toContain("invoke-agent-tasks");
    expect(ids).toContain("read-pet-state");
    expect(ids).toContain("invoke-safe-commands");
    expect(chips.every((chip) => chip.labelZh.length > 0 && chip.detailZh.length > 0)).toBe(true);
  });
});

describe("samplePetDirectiveProgramPlan", () => {
  it("emits a minimal plan using safe.pet.directive with Chinese speech", () => {
    const plan = samplePetDirectiveProgramPlan();
    expect(plan.commands).toHaveLength(1);
    expect(plan.commands[0]?.command).toBe("safe.pet.directive");
    expect(plan.commands[0]?.arguments.spec).toBe("nimora.pet_directive/1");
    expect(plan.commands[0]?.arguments.speech).toBe("专注完成啦，休息一下吧！");
    expect(plan.commands[0]?.arguments.action).toBe("celebrate");
    expect(plan.commands[0]?.arguments.animation).toBe("pet.celebrate");
    expect(plan.commands[0]?.arguments.attention).toBe("user");
    expect(plan.commands[0]?.arguments.moodDelta).toEqual({ mood: 6 });
    expect(plan.commands[0]?.idempotencyKey).toBe("focus-done-celebrate-1");
    expect(plan.storage).toEqual([]);
    expect(plan.agentTasks).toEqual([]);

    const json = formatSampleProgramPlanJson(plan);
    expect(json).toContain("safe.pet.directive");
    expect(json).toContain("nimora.pet_directive/1");
    expect(json).toContain("专注完成啦，休息一下吧！");
  });
});

describe("UserCodeCapabilityMatrix", () => {
  it("renders Chinese capability chips and sample plan for Creator", () => {
    const markup = renderToStaticMarkup(createElement(UserCodeCapabilityMatrix));
    expect(markup).toContain("USER CODE · CAPABILITY MATRIX");
    expect(markup).toContain("用户程序如何驱动宠物与模块");
    expect(markup).toContain("safe.pet.directive");
    expect(markup).toContain("结构化指令");
    expect(markup).toContain("pet.action.catalog");
    expect(markup).toContain("invoke-agent-tasks");
    expect(markup).toContain("nimora.pet_directive/1");
    expect(markup).toContain("专注完成啦，休息一下吧！");
    expect(markup).toContain("Gateway · fail-closed");
  });

  it("hides full manifest group in compact mode used by AI Creator", () => {
    const compact = renderToStaticMarkup(createElement(UserCodeCapabilityMatrix, { compact: true }));
    expect(compact).toContain("safe.pet.directive");
    expect(compact).toContain("pet.state");
    expect(compact).not.toContain("Manifest 能力 · Capabilities");
    const full = renderToStaticMarkup(createElement(UserCodeCapabilityMatrix));
    expect(full).toContain("Manifest 能力 · Capabilities");
  });
});
