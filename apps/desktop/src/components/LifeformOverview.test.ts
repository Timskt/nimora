import { describe, expect, it } from "vitest";
import {
  animationTokenLabel,
  attentionLabel,
  batterySenseLabel,
  buildLifeformSenseCards,
  buildLifeformSenseHintsFromSnapshot,
  applyLifeformSenseSnapshot,
  buildLifeformSubjectLinks,
  clampPercent,
  connectorFromSystemSensors,
  connectorSenseLabel,
  directiveSummary,
  idleSenseLabel,
  lifeformEmotionLabel,
  lifeformStatusLabel,
  mergeLifeformSenseHints,
  multiDisplaySenseLabel,
  occlusionSenseLabel,
  overlayStageSummary,
  personalityBars,
  skillTreeLabel,
  workerBusyFromPet,
} from "./LifeformOverview";
import {
  buildPerfBudgetCards,
  shouldWarnIdleBudget,
} from "./lifeformPerf";

describe("lifeformStatusLabel", () => {
  it("maps known lifecycle states to Chinese labels", () => {
    expect(lifeformStatusLabel("idle")).toBe("待机");
    expect(lifeformStatusLabel("working")).toBe("专注");
    expect(lifeformStatusLabel("observing")).toBe("观察");
    expect(lifeformStatusLabel("sleeping")).toBe("休息");
  });

  it("falls back safely for empty or unknown states", () => {
    expect(lifeformStatusLabel(undefined)).toBe("待机");
    expect(lifeformStatusLabel("moonwalk")).toBe("moonwalk");
  });
});

describe("lifeformEmotionLabel", () => {
  it("maps emotions and defaults to calm", () => {
    expect(lifeformEmotionLabel("happy")).toBe("开心");
    expect(lifeformEmotionLabel("focused")).toBe("专注");
    expect(lifeformEmotionLabel(null)).toBe("平静");
    expect(lifeformEmotionLabel("curious")).toBe("curious");
  });
});

describe("clampPercent", () => {
  it("clamps finite numbers into 0–100", () => {
    expect(clampPercent(-4)).toBe(0);
    expect(clampPercent(140)).toBe(100);
    expect(clampPercent(66.6)).toBe(67);
    expect(clampPercent(Number.NaN, 42)).toBe(42);
    expect(clampPercent(undefined, 10)).toBe(10);
  });
});

describe("attentionLabel", () => {
  it("maps host attention tokens to Chinese labels", () => {
    expect(attentionLabel("cursor")).toBe("鼠标");
    expect(attentionLabel("foreground_window")).toBe("前台窗口");
    expect(attentionLabel("notification_area")).toBe("通知区");
    expect(attentionLabel("user")).toBe("主人");
    expect(attentionLabel("idle_scene")).toBe("场景");
    expect(attentionLabel("obstacle")).toBe("障碍");
  });

  it("returns null for empty attention and passes unknown tokens through", () => {
    expect(attentionLabel(null)).toBeNull();
    expect(attentionLabel(undefined)).toBeNull();
    expect(attentionLabel("")).toBeNull();
    expect(attentionLabel("desk_edge")).toBe("desk_edge");
  });
});

describe("directiveSummary", () => {
  it("returns null when no directive data is present", () => {
    expect(directiveSummary({})).toBeNull();
    expect(directiveSummary({ directiveRevision: 0 })).toBeNull();
    expect(directiveSummary(undefined)).toBeNull();
  });

  it("prefers live speech override and keeps animation/attention/revision", () => {
    expect(directiveSummary({
      lastDirectiveSpeech: "旧台词",
      lastDirectiveAnimation: "pet.celebrate",
      lastAttention: "user",
      directiveRevision: 3,
    }, "完成啦！")).toEqual({
      speech: "完成啦！",
      animation: "pet.celebrate",
      attention: "user",
      revision: 3,
    });
  });

  it("surfaces speech-only overrides without a pet snapshot", () => {
    expect(directiveSummary(undefined, "先休息一下")).toEqual({
      speech: "先休息一下",
      animation: null,
      attention: null,
      revision: null,
    });
  });
});

describe("personalityBars", () => {
  it("returns empty when personality is missing", () => {
    expect(personalityBars(undefined)).toEqual([]);
    expect(personalityBars(null)).toEqual([]);
  });

  it("builds compact Chinese trait bars for present axes only", () => {
    expect(personalityBars({
      energy: 80,
      curiosity: 55,
      laziness: 20,
      pride: 40,
    })).toEqual([
      { key: "energy", label: "活力", value: 80 },
      { key: "curiosity", label: "好奇", value: 55 },
      { key: "laziness", label: "懒散", value: 20 },
      { key: "pride", label: "傲娇", value: 40 },
    ]);
    expect(personalityBars({ energy: 12, pride: 200 })).toEqual([
      { key: "energy", label: "活力", value: 12 },
      { key: "pride", label: "傲娇", value: 100 },
    ]);
  });
});

describe("overlayStageSummary", () => {
  it("formats origin and size for the desktop overlay stage", () => {
    expect(overlayStageSummary({ originX: 12.4, originY: 8.9, width: 1280, height: 800 }))
      .toBe("原点 (12, 9) · 1280×800");
  });

  it("hides empty or missing stages", () => {
    expect(overlayStageSummary(null)).toBeNull();
    expect(overlayStageSummary(undefined)).toBeNull();
    expect(overlayStageSummary({ originX: 0, originY: 0, width: 0, height: 0 })).toBeNull();
  });
});

describe("animationTokenLabel", () => {
  it("maps host animation tokens to Chinese pet poses", () => {
    expect(animationTokenLabel("pet.celebrate")).toBe("庆祝");
    expect(animationTokenLabel("work")).toBe("干活");
    expect(animationTokenLabel("pet.work_busy")).toBe("出汗干活");
    expect(animationTokenLabel("pet.observe")).toBe("观察");
    expect(animationTokenLabel(null)).toBeNull();
  });

  it("passes unknown tokens through after stripping pet. prefix", () => {
    expect(animationTokenLabel("pet.moonwalk")).toBe("moonwalk");
    expect(animationTokenLabel("custom_spin")).toBe("custom_spin");
  });
});

describe("lifeform sense labels", () => {
  it("describes occlusion coverage in Chinese", () => {
    expect(occlusionSenseLabel(null)).toBeNull();
    expect(occlusionSenseLabel({ coverage: 0, fullyHidden: false })).toBe("视野开阔");
    expect(occlusionSenseLabel({ coverage: 0.2, fullyHidden: false })).toBe("轻微遮挡 20%");
    expect(occlusionSenseLabel({ coverage: 0.5, fullyHidden: false })).toBe("部分遮挡 50%");
    expect(occlusionSenseLabel({ coverage: 0.9, fullyHidden: false })).toBe("重度遮挡 90%");
    expect(occlusionSenseLabel({ coverage: 1, fullyHidden: true })).toBe("完全遮挡");
  });

  it("labels battery and idle hints", () => {
    expect(batterySenseLabel(null)).toBeNull();
    expect(batterySenseLabel(12)).toBe("电量告急 12%");
    expect(batterySenseLabel(28)).toBe("电量偏低 28%");
    expect(batterySenseLabel(88)).toBe("电量 88%");
    expect(idleSenseLabel(undefined)).toBeNull();
    expect(idleSenseLabel(0)).toBe("刚活跃");
    expect(idleSenseLabel(12)).toBe("空闲 12 分钟");
    expect(idleSenseLabel(75)).toBe("空闲 1 小时 15 分");
    expect(idleSenseLabel(120)).toBe("空闲 2 小时");
  });

  it("labels multi-display and connector sense", () => {
    expect(multiDisplaySenseLabel(null)).toBeNull();
    expect(multiDisplaySenseLabel({ displayCount: 1 })).toBe("单显示器");
    expect(multiDisplaySenseLabel({ displayCount: 2, activeLabel: "副屏" })).toBe("2 块显示器 · 副屏");
    expect(connectorSenseLabel(null)).toBeNull();
    expect(connectorSenseLabel({ connected: 0, total: 0 })).toBe("无连接器");
    expect(connectorSenseLabel({ connected: 2, total: 3 })).toBe("2/3 已连接");
    expect(connectorSenseLabel({ connected: 2, total: 2, degraded: 1 })).toBe("2/2 连接 · 1 降级");
    expect(connectorSenseLabel({ connected: 1, total: 1 })).toBe("1/1 正常");
  });

  it("builds beautiful sense cards and skips empties", () => {
    expect(buildLifeformSenseCards(null)).toEqual([]);
    expect(buildLifeformSenseCards({})).toEqual([]);
    const cards = buildLifeformSenseCards({
      occlusion: { coverage: 0.4, fullyHidden: false },
      multiDisplay: { displayCount: 2, activeLabel: "主屏" },
      meeting: true,
      batteryPercent: 18,
      idleMinutes: 5,
      connector: { connected: 1, total: 2 },
      workerBusy: true,
    });
    expect(cards.map((card) => card.id)).toEqual([
      "occlusion",
      "multi-display",
      "meeting",
      "battery",
      "idle",
      "connector",
      "worker",
    ]);
    expect(cards.find((card) => card.id === "meeting")?.value).toBe("进行中");
    expect(cards.find((card) => card.id === "worker")?.tone).toBe("busy");
    expect(cards.find((card) => card.id === "battery")?.tone).toBe("warn");
    expect(cards.find((card) => card.id === "battery")?.value).toContain("18");
  });

  it("shows idle worker as neutral empty-busy state with pet 体力 label", () => {
    expect(buildLifeformSenseCards({ workerBusy: false })).toEqual([
      { id: "worker", label: "体力", value: "空闲", detail: "Worker", tone: "neutral" },
    ]);
  });

  it("surfaces busy worker as 出汗忙碌 pet narrative", () => {
    expect(buildLifeformSenseCards({ workerBusy: true })).toEqual([
      { id: "worker", label: "体力", value: "出汗忙碌", detail: "Worker · 本地推理中", tone: "busy" },
    ]);
  });

  it("marks fully offline connector as sad 感官离线 danger", () => {
    const cards = buildLifeformSenseCards({ connector: { connected: 0, total: 3 } });
    expect(cards.find((card) => card.id === "connector")).toMatchObject({
      label: "感官",
      value: "感官离线",
      detail: "Connector · 伙伴有点失落",
      tone: "danger",
    });
  });

  it("maps connector sense card to 感官 pet label", () => {
    const cards = buildLifeformSenseCards({ connector: { connected: 2, total: 2 } });
    expect(cards.find((card) => card.id === "connector")).toMatchObject({
      label: "感官",
      detail: "Connector",
      tone: "ok",
    });
  });
});

describe("lifeform subject narrative", () => {
  it("labels skill tree growth from affinity", () => {
    expect(skillTreeLabel(null)).toBe("待启程");
    expect(skillTreeLabel(12)).toBe("萌芽 12");
    expect(skillTreeLabel(55)).toBe("成长中 55");
    expect(skillTreeLabel(90)).toBe("熟练 90");
  });

  it("links Skill=技能树, Worker=体力, Connector=感官", () => {
    const links = buildLifeformSubjectLinks({
      affinity: 62,
      energy: 88,
      workerBusy: true,
      connector: { connected: 1, total: 2, degraded: 0 },
    });
    expect(links.map((link) => [link.system, link.petLabel])).toEqual([
      ["Skill", "技能树"],
      ["Worker", "体力"],
      ["Connector", "感官"],
    ]);
    expect(links.find((link) => link.id === "skill")?.value).toContain("成长中");
    expect(links.find((link) => link.id === "worker")?.value).toBe("出汗忙碌");
    expect(links.find((link) => link.id === "worker")?.tone).toBe("busy");
    expect(links.find((link) => link.id === "connector")?.tone).toBe("warn");
  });

  it("marks connector offline subject as sad 感官离线", () => {
    const links = buildLifeformSubjectLinks({
      connector: { connected: 0, total: 2 },
    });
    expect(links.find((link) => link.id === "connector")).toMatchObject({
      petLabel: "感官",
      value: "感官离线",
      tone: "danger",
    });
  });

  it("skips empty subject inputs", () => {
    expect(buildLifeformSubjectLinks(null)).toEqual([]);
    expect(buildLifeformSubjectLinks({})).toEqual([]);
  });
});

describe("lifeform snapshot-derived sense bindings", () => {
  it("derives worker busy only from pet.working (honest)", () => {
    expect(workerBusyFromPet(null)).toBeNull();
    expect(workerBusyFromPet({ state: "idle" })).toBeNull();
    expect(workerBusyFromPet({ state: "working" })).toBe(true);
  });

  it("maps system context sensors to connector counts without inventing sensors", () => {
    expect(connectorFromSystemSensors(null)).toBeNull();
    expect(connectorFromSystemSensors([])).toBeNull();
    expect(connectorFromSystemSensors([
      {
        spec: "nimora.system-context-sensor-health/1",
        descriptor: { kind: "fullscreen", source: "operating_system" },
        availability: "available",
        consecutiveFailures: 0,
        lastSuccessAtMs: 1,
        lastErrorCode: null,
        nextSampleAtMs: 2,
      },
      {
        spec: "nimora.system-context-sensor-health/1",
        descriptor: { kind: "game", source: "operating_system" },
        availability: "degraded",
        consecutiveFailures: 1,
        lastSuccessAtMs: null,
        lastErrorCode: "timeout",
        nextSampleAtMs: null,
      },
      {
        spec: "nimora.system-context-sensor-health/1",
        descriptor: { kind: "do_not_disturb", source: "operating_system" },
        availability: "unavailable",
        consecutiveFailures: 2,
        lastSuccessAtMs: null,
        lastErrorCode: "denied",
        nextSampleAtMs: null,
      },
    ])).toEqual({ connected: 1, total: 3, degraded: 1 });
  });

  it("builds sense hints from desktop snapshot facts only", () => {
    expect(buildLifeformSenseHintsFromSnapshot(null)).toBeNull();
    expect(buildLifeformSenseHintsFromSnapshot({
      pet: { state: "idle" } as never,
      systemContextSensors: [],
      presenceDecision: {
        spec: "nimora.system-context-decision/1",
        visible: true,
        suppressAutonomy: false,
        reason: "base_policy",
        decidedAtMs: 1,
      },
    })).toBeNull();

    const hints = buildLifeformSenseHintsFromSnapshot({
      pet: { state: "working" } as never,
      systemContextSensors: [{
        spec: "nimora.system-context-sensor-health/1",
        descriptor: { kind: "fullscreen", source: "operating_system" },
        availability: "available",
        consecutiveFailures: 0,
        lastSuccessAtMs: 1,
        lastErrorCode: null,
        nextSampleAtMs: 2,
      }],
      presenceDecision: {
        spec: "nimora.system-context-decision/1",
        visible: true,
        suppressAutonomy: true,
        reason: "do_not_disturb",
        decidedAtMs: 1,
      },
    });
    expect(hints).toMatchObject({
      workerBusy: true,
      meeting: true,
      connector: { connected: 1, total: 1, degraded: 0 },
    });
  });

  it("projects lifeformSense battery/idle/meeting/display aggregates", () => {
    const hints = buildLifeformSenseHintsFromSnapshot({
      pet: { state: "idle" } as never,
      systemContextSensors: [],
      presenceDecision: {
        spec: "nimora.system-context-decision/1",
        visible: true,
        suppressAutonomy: false,
        reason: "base_policy",
        decidedAtMs: 1,
      },
      overlayStage: { originX: 0, originY: 0, width: 1512, height: 982 },
      lifeformSense: {
        batteryPercent: 18,
        onBattery: true,
        charging: false,
        idleMs: 5 * 60 * 1000,
        meetingActive: true,
        meetingHint: "zoom",
        displayCount: 2,
        freshness: "fresh",
        degradationReason: null,
        observedAtMs: 1,
        expiresAtMs: 5001,
      },
    });
    expect(hints).toMatchObject({
      batteryPercent: 18,
      idleMinutes: 5,
      meeting: true,
      multiDisplay: { displayCount: 2, activeLabel: "1512×982" },
    });
  });


  it("projects lifeformSense notificationUnread into sense cards", () => {
    const withUnread = buildLifeformSenseHintsFromSnapshot({
      lifeformSense: {
        batteryPercent: 80,
        onBattery: false,
        charging: false,
        idleMs: 0,
        meetingActive: false,
        displayCount: 1,
        notificationUnread: true,
        freshness: "fresh",
        observedAtMs: 1,
        expiresAtMs: 2,
      },
    } as never);
    const cards = buildLifeformSenseCards(withUnread);
    expect(cards.some((c) => c.id === "notification" && c.value === "有未读")).toBe(true);
  });

  it("merges explicit senseHints over snapshot and pet.working", () => {
    expect(mergeLifeformSenseHints(
      { workerBusy: false },
      { workerBusy: true, connector: { connected: 1, total: 1 } },
      { state: "working" },
    )).toMatchObject({
      workerBusy: false,
      connector: { connected: 1, total: 1 },
    });
    expect(mergeLifeformSenseHints(null, null, { state: "working" })).toEqual({
      workerBusy: true,
    });
    expect(mergeLifeformSenseHints(null, null, { state: "idle" })).toBeNull();
  });
});

describe("渲染预算 perf cards", () => {
  it("exposes Chinese labels and warn tone under idle budget pressure", () => {
    const summary = { fps: 40, avgFrameMs: 26, maxFrameMs: 48, sampleCount: 24 };
    expect(shouldWarnIdleBudget(summary)).toBe(true);
    const cards = buildPerfBudgetCards(summary, { processRssMb: 180, idleCpuPercent: 1.2 });
    expect(cards.map((c) => c.label)).toEqual(["帧率", "帧耗时", "空闲预算", "内存", "空闲 CPU"]);
    expect(cards.find((c) => c.id === "fps")?.tone).toBe("warn");
    expect(cards.find((c) => c.id === "idle-budget")?.value).toBe("偏紧");
    expect(cards.find((c) => c.id === "memory")?.tone).toBe("ok");
  });

  it("keeps healthy budget cards on tone-ok", () => {
    const cards = buildPerfBudgetCards(
      { fps: 60, avgFrameMs: 16.4, maxFrameMs: 18, sampleCount: 50 },
      { processRssMb: 120, processCpuPercent: 0.8 },
    );
    expect(cards.every((c) => c.tone === "ok")).toBe(true);
    expect(cards.find((c) => c.id === "idle-budget")?.value).toBe("充裕");
  });
});
