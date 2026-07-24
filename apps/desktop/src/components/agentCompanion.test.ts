import { describe, expect, it } from "vitest";
import {
  agentCompanionActionLabel,
  agentCompanionBubble,
  agentCompanionDirective,
  agentCompanionMoodLabel,
  agentCompanionNarrative,
  agentCompanionPresentation,
  agentCompanionStatusLabel,
  agentCompanionTone,
  autoModeCompanionDirective,
  autoModePhaseLabel,
  companionAnimationLabel,
  companionBubbleFromAutoMode,
  connectorCompanionDirective,
  connectorCompanionNarrative,
  connectorCompanionPhaseFromStatus,
  createAgentCompanionSignal,
  companionStatusFromAutoMode,
  resolvePetWorkVisual,
  resolveSubjectMotionForWorkerFeedback,
  workerCompanionDirective,
  workerCompanionNarrative,
  workerCompanionPhaseFromStatus,
} from "./agentCompanion";
import type { AgentCompanionStatus } from "../platform/desktop";
import { isAgentCompanionSignal } from "../platform/desktop";

describe("agent companion presentation", () => {
  it("maps active work to a persistent work pose", () => {
    expect(agentCompanionPresentation("running")).toEqual({
      action: "work",
      message: "正在陪你干活",
      persistent: true,
    });
  });

  it("maps thinking to observe and waiting to perch", () => {
    expect(agentCompanionPresentation("thinking").action).toBe("observe");
    expect(agentCompanionPresentation("waiting_for_confirmation")).toEqual({
      action: "perch",
      message: "需要你确认一下",
      persistent: true,
    });
  });

  it("maps terminal states to short non-persistent speech", () => {
    expect(agentCompanionPresentation("completed").persistent).toBe(false);
    expect(agentCompanionPresentation("failed").message).toBe("没关系，我们再试");
    expect(agentCompanionPresentation("cancelled").message).toBe("已停下，我还在");
  });

  it("creates a host-validated companion signal without display content", () => {
    const signal = createAgentCompanionSignal("waiting_for_confirmation", "task-1");
    expect(signal).toMatchObject({
      spec: "nimora.agent-companion-signal/1",
      status: "waiting_for_confirmation",
      taskId: "task-1",
    });
    expect(Object.keys(signal)).toEqual(["spec", "status", "taskId", "updatedAtMs"]);
    expect(isAgentCompanionSignal(signal)).toBe(true);
  });

  it("rejects unknown states and injected display content", () => {
    expect(isAgentCompanionSignal({
      spec: "nimora.agent-companion-signal/1",
      status: "dance",
      taskId: null,
      updatedAtMs: 1,
    })).toBe(false);
    expect(isAgentCompanionSignal({
      ...createAgentCompanionSignal("running"),
      message: "untrusted",
    })).toBe(false);
  });
});

describe("companion strip labels and tones", () => {
  it("maps statuses to clear Chinese labels", () => {
    expect(agentCompanionStatusLabel("thinking")).toBe("思考中");
    expect(agentCompanionStatusLabel("running")).toBe("执行中");
    expect(agentCompanionStatusLabel("waiting_for_confirmation")).toBe("等待确认");
    expect(agentCompanionStatusLabel("completed")).toBe("已完成");
    expect(agentCompanionStatusLabel("failed")).toBe("失败");
    expect(agentCompanionStatusLabel("cancelled")).toBe("已取消");
  });

  it("maps statuses to scannable color tones", () => {
    expect(agentCompanionTone("thinking")).toBe("thinking");
    expect(agentCompanionTone("running")).toBe("running");
    expect(agentCompanionTone("waiting_for_confirmation")).toBe("waiting");
    expect(agentCompanionTone("completed")).toBe("success");
    expect(agentCompanionTone("failed")).toBe("danger");
    expect(agentCompanionTone("cancelled")).toBe("idle");
  });

  it("builds bubble models with label, tone, message, and pet narrative chips", () => {
    expect(agentCompanionBubble("thinking")).toEqual({
      status: "thinking",
      label: "思考中",
      tone: "thinking",
      message: "我在想…",
      persistent: true,
      actionLabel: "观察思考",
      moodLabel: "好奇",
    });
    expect(agentCompanionBubble("waiting_for_confirmation").label).toBe("等待确认");
    expect(agentCompanionBubble("waiting_for_confirmation").tone).toBe("waiting");
    expect(agentCompanionBubble("failed").message).toBe("没关系，我们再试");
    expect(agentCompanionBubble("running").actionLabel).toBe("出汗干活");
    expect(agentCompanionBubble("running").moodLabel).toBe("专注");
  });
});

describe("agent companion pet narrative", () => {
  it("maps every phase to speech + mood + action tokens owners understand", () => {
    const statuses = [
      "thinking",
      "running",
      "waiting_for_confirmation",
      "completed",
      "failed",
      "cancelled",
    ] as const;
    const expected = {
      thinking: { actionLabel: "观察思考", moodLabel: "好奇", speech: "我在想…" },
      running: { actionLabel: "出汗干活", moodLabel: "专注", speech: "正在陪你干活" },
      waiting_for_confirmation: { actionLabel: "栖息等待", moodLabel: "期待", speech: "需要你确认一下" },
      completed: { actionLabel: "庆祝完成", moodLabel: "开心", speech: "完成啦！" },
      failed: { actionLabel: "休息调整", moodLabel: "低落", speech: "没关系，我们再试" },
      cancelled: { actionLabel: "安静待命", moodLabel: "平静", speech: "已停下，我还在" },
    } as const;

    for (const status of statuses) {
      const narrative = agentCompanionNarrative(status);
      const want = expected[status];
      expect(narrative.actionLabel).toBe(want.actionLabel);
      expect(narrative.moodLabel).toBe(want.moodLabel);
      expect(narrative.speech).toBe(want.speech);
      expect(narrative.phaseLabel).toBe(agentCompanionStatusLabel(status));
    }
  });

  it("exposes action/mood helpers for strip chips", () => {
    expect(agentCompanionActionLabel("running")).toBe("出汗干活");
    expect(agentCompanionMoodLabel("failed")).toBe("低落");
    expect(agentCompanionActionLabel("waiting_for_confirmation")).toBe("栖息等待");
    expect(agentCompanionMoodLabel("completed")).toBe("开心");
  });
});

describe("agentCompanionDirective", () => {
  it("emits a v1 structured directive for each companion status", () => {
    const statuses = [
      "thinking",
      "running",
      "waiting_for_confirmation",
      "completed",
      "failed",
      "cancelled",
    ] as const;

    for (const status of statuses) {
      const directive = agentCompanionDirective(status);
      expect(directive.spec).toBe("nimora.pet_directive/1");
      expect(typeof directive.action).toBe("string");
      expect(typeof directive.attention).toBe("string");
      expect(directive.speech).toBeTruthy();
      expect(directive.animation?.startsWith("pet.")).toBe(true);
    }
  });

  it("maps thinking to observe with mild mood and user attention", () => {
    expect(agentCompanionDirective("thinking")).toEqual({
      spec: "nimora.pet_directive/1",
      speech: "我在想…",
      action: "observe",
      animation: "pet.observe",
      attention: "user",
      moodDelta: { mood: 1 },
    });
  });

  it("maps running to work_busy with work animation", () => {
    expect(agentCompanionDirective("running")).toEqual({
      spec: "nimora.pet_directive/1",
      speech: "正在陪你干活",
      action: "work_busy",
      animation: "pet.work",
      attention: "user",
    });
  });

  it("maps waiting_for_confirmation to perch", () => {
    expect(agentCompanionDirective("waiting_for_confirmation")).toEqual({
      spec: "nimora.pet_directive/1",
      speech: "需要你确认一下",
      action: "perch",
      animation: "pet.perch",
      attention: "user",
    });
  });

  it("maps completed to celebrate with positive mood", () => {
    expect(agentCompanionDirective("completed")).toEqual({
      spec: "nimora.pet_directive/1",
      speech: "完成啦！",
      action: "celebrate",
      animation: "pet.celebrate",
      attention: "user",
      moodDelta: { mood: 6 },
    });
  });

  it("maps failed to rest with small negative mood", () => {
    expect(agentCompanionDirective("failed")).toEqual({
      spec: "nimora.pet_directive/1",
      speech: "没关系，我们再试",
      action: "rest",
      animation: "pet.idle",
      attention: "idle_scene",
      moodDelta: { mood: -2 },
    });
  });

  it("maps cancelled to rest without mood delta", () => {
    expect(agentCompanionDirective("cancelled")).toEqual({
      spec: "nimora.pet_directive/1",
      speech: "已停下，我还在",
      action: "rest",
      animation: "pet.idle",
      attention: "idle_scene",
    });
  });

  it("uses host snake_case action and attention tokens", () => {
    const directive = agentCompanionDirective("running");
    expect(directive.action).toBe("work_busy");
    expect(directive.attention).toBe("user");
    expect(directive.action).not.toMatch(/[A-Z]/);
    expect(agentCompanionDirective("failed").attention).toBe("idle_scene");
  });
});

describe("companionStatusFromAutoMode", () => {
  it("maps job lifecycle to pet companion phases", () => {
    expect(companionStatusFromAutoMode("starting")).toBe("running");
    expect(companionStatusFromAutoMode("running")).toBe("running");
    expect(companionStatusFromAutoMode("completed")).toBe("completed");
    expect(companionStatusFromAutoMode("failed")).toBe("failed");
    expect(companionStatusFromAutoMode("cancelled")).toBe("cancelled");
    expect(companionStatusFromAutoMode("paused", { pauseReason: "confirmation_required" })).toBe("waiting_for_confirmation");
    expect(companionStatusFromAutoMode("paused", { pauseReason: "user_paused" })).toBe("waiting_for_confirmation");
    expect(companionStatusFromAutoMode("paused", { pauseReason: "grant_revoked" })).toBe("failed");
    expect(companionStatusFromAutoMode("running", { indeterminate: true })).toBe("waiting_for_confirmation");
  });

  it("defaults unknown status to thinking rather than inventing host tokens", () => {
    expect(companionStatusFromAutoMode(null)).toBe("thinking");
    expect(companionStatusFromAutoMode("mystery")).toBe("thinking");
  });
});

describe("autoModePhaseLabel + companionBubbleFromAutoMode", () => {
  it("exposes Running / Paused / Failed phase chips in Chinese", () => {
    expect(autoModePhaseLabel("running")).toBe("执行中");
    expect(autoModePhaseLabel("paused", { pauseReason: "user_paused" })).toBe("已暂停");
    expect(autoModePhaseLabel("paused", { pauseReason: "confirmation_required" })).toBe("等待确认");
    expect(autoModePhaseLabel("failed")).toBe("失败");
    expect(autoModePhaseLabel("paused", { pauseReason: "grant_revoked" })).toBe("失败");
  });

  it("builds owner-facing bubbles that distinguish pause reasons", () => {
    expect(companionBubbleFromAutoMode("running").label).toBe("执行中");
    expect(companionBubbleFromAutoMode("running").message).toMatch(/自动模式/);
    const paused = companionBubbleFromAutoMode("paused", { pauseReason: "user_paused" });
    expect(paused.label).toBe("已暂停");
    expect(paused.message).toMatch(/暂停|Checkpoint/);
    expect(paused.moodLabel).toBe("平静");
    const waiting = companionBubbleFromAutoMode("paused", { pauseReason: "confirmation_required" });
    expect(waiting.label).toBe("等待确认");
    expect(waiting.message).toMatch(/确认/);
    expect(companionBubbleFromAutoMode("failed").label).toBe("失败");
    expect(companionBubbleFromAutoMode("failed").tone).toBe("danger");
  });
});

describe("companionAnimationLabel", () => {
  it("maps micro-performance tokens to Chinese chips", () => {
    expect(companionAnimationLabel("pet.yawn")).toBe("打哈欠");
    expect(companionAnimationLabel("pet.dig_nose")).toBe("抠鼻子");
    expect(companionAnimationLabel("pet.count_ants")).toBe("数蚂蚁");
    expect(companionAnimationLabel("pet.wave")).toBe("招手");
    expect(companionAnimationLabel("pet.look_around")).toBe("四处张望");
    expect(companionAnimationLabel("pet.hop")).toBe("轻跳");
    expect(companionAnimationLabel("yawn")).toBe("打哈欠");
    expect(companionAnimationLabel("pet.work")).toBe("出汗干活");
    expect(companionAnimationLabel(null)).toBe("安静待命");
  });
});


describe("worker companion mapping", () => {
  it("maps busy / done / failed with Chinese chips and host tokens", () => {
    expect(workerCompanionPhaseFromStatus("busy")).toBe("busy");
    expect(workerCompanionPhaseFromStatus("WORKING")).toBe("busy");
    expect(workerCompanionPhaseFromStatus("completed")).toBe("done");
    expect(workerCompanionPhaseFromStatus("crash")).toBe("failed");

    const busy = workerCompanionNarrative("busy", "summarize");
    expect(busy.phaseLabel).toBe("出汗忙碌");
    expect(busy.actionLabel).toBe("出汗干活");
    expect(busy.moodLabel).toBe("疲惫");
    expect(busy.speech).toContain("summarize");

    const failed = workerCompanionNarrative("failed");
    expect(failed.actionLabel).toBe("晕倒缓缓");
    expect(failed.moodLabel).toBe("低落");
    expect(failed.speech).toBe("刚才绊了一下");
  });

  it("builds nimora.pet_directive/1 for busy sweat and crash dizzy", () => {
    const busy = workerCompanionDirective("busy");
    expect(busy).toMatchObject({
      spec: "nimora.pet_directive/1",
      action: "work_busy",
      animation: "pet.work",
      attention: "user",
    });
    expect(busy?.speech).toBe("技能跑起来了");

    const crash = workerCompanionDirective("failed");
    expect(crash).toMatchObject({
      spec: "nimora.pet_directive/1",
      action: "work_crash",
      animation: "pet.work",
      moodDelta: { mood: -8 },
    });

    const done = workerCompanionDirective("done");
    expect(done?.action).toBe("celebrate");
  });

  it("fail-closed: unknown status → idle and no directive", () => {
    expect(workerCompanionPhaseFromStatus(null)).toBe("idle");
    expect(workerCompanionPhaseFromStatus("")).toBe("idle");
    expect(workerCompanionPhaseFromStatus("mystery-worker")).toBe("idle");
    expect(workerCompanionDirective("idle")).toBeNull();
    expect(workerCompanionNarrative("idle").hostAction).toBeNull();
  });
});

describe("connector companion mapping", () => {
  it("maps offline to sad rest and degraded to alert observe", () => {
    expect(connectorCompanionPhaseFromStatus("offline")).toBe("offline");
    expect(connectorCompanionPhaseFromStatus("degraded")).toBe("degraded");
    expect(connectorCompanionPhaseFromStatus("restored")).toBe("restored");

    const offline = connectorCompanionNarrative("offline");
    expect(offline.phaseLabel).toBe("感官离线");
    expect(offline.actionLabel).toBe("休息调整");
    expect(offline.moodLabel).toBe("低落");
    expect(offline.speech).toBe("线路好像断了");

    const degraded = connectorCompanionNarrative("degraded");
    expect(degraded.actionLabel).toBe("警戒观察");
    expect(degraded.moodLabel).toBe("警惕");
  });

  it("builds structured directives with Chinese speech", () => {
    expect(connectorCompanionDirective("offline")).toMatchObject({
      spec: "nimora.pet_directive/1",
      action: "rest",
      animation: "pet.idle",
      attention: "idle_scene",
      speech: "线路好像断了",
      moodDelta: { mood: -10 },
    });
    expect(connectorCompanionDirective("degraded")?.action).toBe("observe");
    expect(connectorCompanionDirective("restored")?.action).toBe("celebrate");
    expect(connectorCompanionDirective("event")?.attention).toBe("notification_area");
  });

  it("fail-closed: healthy / unknown emit no directive", () => {
    expect(connectorCompanionPhaseFromStatus("healthy")).toBe("healthy");
    expect(connectorCompanionPhaseFromStatus("wat")).toBe("unknown");
    expect(connectorCompanionPhaseFromStatus(null)).toBe("unknown");
    expect(connectorCompanionDirective("healthy")).toBeNull();
    expect(connectorCompanionDirective("unknown")).toBeNull();
  });
});

describe("autoModeCompanionDirective structured emit", () => {
  it("emits nimora.pet_directive/1 for Running / Paused / Failed", () => {
    const running = autoModeCompanionDirective("running");
    expect(running.spec).toBe("nimora.pet_directive/1");
    expect(running.action).toBe("work_busy");
    expect(running.speech).toMatch(/自动模式/);

    const paused = autoModeCompanionDirective("paused", { pauseReason: "user_paused" });
    expect(paused.spec).toBe("nimora.pet_directive/1");
    expect(paused.action).toBe("perch");
    expect(paused.speech).toMatch(/暂停|Checkpoint/);

    const failed = autoModeCompanionDirective("failed");
    expect(failed.spec).toBe("nimora.pet_directive/1");
    expect(failed.action).toBe("rest");
    expect(failed.speech).toMatch(/失败|再试/);
  });
});

describe("pet work visual feedback (sweat / dizzy)", () => {
  it("maps work_busy and working lifecycle to sweat", () => {
    expect(resolvePetWorkVisual({ directiveAction: "work_busy" })).toBe("sweat");
    expect(resolvePetWorkVisual({ lifecycleState: "working" })).toBe("sweat");
    expect(resolvePetWorkVisual({ directiveAnimation: "pet.work" })).toBe("sweat");
  });

  it("maps work_crash and recovering lifecycle to dizzy even when animation is pet.work", () => {
    expect(resolvePetWorkVisual({
      directiveAction: "work_crash",
      directiveAnimation: "pet.work",
    })).toBe("dizzy");
    expect(resolvePetWorkVisual({
      directiveAnimation: "pet.work",
      lifecycleState: "recovering",
      emotion: "sad",
    })).toBe("dizzy");
    expect(resolvePetWorkVisual({ emotion: "dizzy" })).toBe("dizzy");
  });

  it("resolveSubjectMotionForWorkerFeedback prefers crash over pet.work", () => {
    expect(resolveSubjectMotionForWorkerFeedback({
      directiveAnimation: "pet.work",
      lifecycleState: "recovering",
      emotion: "sad",
    })).toBe("work_crash");
    expect(resolveSubjectMotionForWorkerFeedback({
      directiveAnimation: "pet.work",
      lifecycleState: "working",
    })).toBe("work_busy");
    expect(resolveSubjectMotionForWorkerFeedback({
      directiveAnimation: "pet.celebrate",
      lifecycleState: "idle",
    })).toBe("pet.celebrate");
    expect(resolveSubjectMotionForWorkerFeedback({})).toBe("idle");
  });
});
