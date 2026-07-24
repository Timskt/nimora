/**
 * Living presence — organic speech & attention for Nimora 灵灵.
 * Goal: never feel like a fixed loop of 3 phrases + robotic idle.
 * Privacy: never surface window titles, paths, or raw OS content.
 */
import type { Pet } from "@nimora/schemas";
import type { PetAmbientDesktopContext } from "./petPresentation";

export type LifeformAttention =
  | "user"
  | "cursor"
  | "taskbar"
  | "window-edge"
  | "notification"
  | "battery"
  | "other-display"
  | "ants"
  | "sky"
  | "self"
  | "work";

export type LifeformMicroAct =
  | "none"
  | "yawn"
  | "dig_nose"
  | "count_ants"
  | "wave"
  | "look_around"
  | "hop"
  | "stretch"
  | "observe"
  | "sleep";

export interface LivingMomentInput {
  pet: Pick<Pet, "state" | "energy" | "mood" | "satiety" | "cleanliness" | "emotion">;
  sequence?: number | null;
  desktop?: PetAmbientDesktopContext | null;
  /** Active creative-workshop scene speech, if any. */
  sceneSpeech?: string | null;
  /** Host directive speech always wins when present. */
  directiveSpeech?: string | null;
  /** Wall clock ms for circadian / anti-sync variety. */
  nowMs?: number;
  /** Recent ambient lines to avoid immediate repeats (session memory). */
  recentLines?: readonly string[] | null;
  /** Worker / agent busy flag from host aggregates. */
  workerBusy?: boolean | null;
  /** Occlusion coverage 0..1. */
  occlusionCoverage?: number | null;
  /** Hour of day override for tests (0–23). */
  hourOfDay?: number | null;
}

export interface LivingMoment {
  speech: string;
  attention: LifeformAttention;
  microAct: LifeformMicroAct;
  /** Why this moment (for debug / audit UI). */
  reason: string;
  /** 0..1 vitality used for bounce amplitude etc. */
  vitality: number;
}

function mixSeed(...parts: Array<number | string | boolean | null | undefined>): number {
  let h = 2166136261;
  for (const part of parts) {
    const s = String(part ?? "");
    for (let i = 0; i < s.length; i += 1) {
      h ^= s.charCodeAt(i);
      h = Math.imul(h, 16777619);
    }
    h ^= 0x9e3779b9;
  }
  return h >>> 0;
}

function pick<T>(items: readonly T[], seed: number): T {
  if (items.length === 0) {
    throw new Error("pick() on empty list");
  }
  return items[Math.abs(seed) % items.length]!;
}

function pickAvoiding(
  items: readonly string[],
  seed: number,
  recent: readonly string[] | null | undefined,
): string {
  if (items.length === 0) return "";
  if (!recent?.length) return pick(items, seed);
  const recentSet = new Set(recent.slice(-6));
  const fresh = items.filter((line) => !recentSet.has(line));
  const pool = fresh.length > 0 ? fresh : items;
  return pick(pool, seed);
}

function hourFrom(nowMs: number, override?: number | null): number {
  if (typeof override === "number" && Number.isFinite(override)) {
    return ((Math.trunc(override) % 24) + 24) % 24;
  }
  return new Date(nowMs).getHours();
}

function vitalityOf(pet: LivingMomentInput["pet"]): number {
  const mood = clamp01((pet.mood ?? 50) / 100);
  const energy = clamp01((pet.energy ?? 50) / 100);
  const satiety = clamp01((pet.satiety ?? 50) / 100);
  const clean = clamp01((pet.cleanliness ?? 50) / 100);
  return clamp01(mood * 0.35 + energy * 0.35 + satiety * 0.15 + clean * 0.15);
}

function clamp01(v: number): number {
  if (!Number.isFinite(v)) return 0.5;
  return Math.min(1, Math.max(0, v));
}

export const MEETING_LINES = [
  "嘘…你在开会，我贴边当小雕像",
  "会议中，尾巴（如果有）也不晃啦",
  "我把蹦跳音量调到零",
  "认真听讲的样子…我也学着安静",
  "开完会记得伸个懒腰，我陪你",
] as const;

export const LOW_BATTERY_LINES = [
  "电量像小太阳在睡觉…我轻手轻脚",
  "省一点电，我改成小碎步模式",
  "屏幕暗暗的，我也不吵",
  "低电量日：拥抱节电，拥抱你",
] as const;

const CHARGING_LINES = [
  "充电线像在投喂能量豆！",
  "电在涨，我也跟着精神起来",
  "滋滋滋…充满了可以一起蹦",
  "电源在唱歌，我打拍子",
] as const;

export const USER_AWAY_LINES = [
  "你去哪儿啦？我在桌角等你",
  "数完一百只蚂蚁还是不见你…",
  "空空的键帽，想你打字的声音",
  "我守着光标停住的地方",
  "回来的时候记得叫我一声",
  "发呆模式启动…其实在想你",
] as const;

export const NOTIFY_LINES = [
  "铃铃？好像有小事在等你",
  "我竖起天线了，角落有动静",
  "有一条提醒在眨眼，你忙完再看也行",
  "警觉！…然后又放松成软软的",
] as const;

export const MULTI_DISPLAY_LINES = [
  "那边屏幕也好奇，要不要去踩一脚",
  "双屏世界好大，我是游牧小黄豆",
  "我在想：另一个屏幕的风是不是不一样",
  "跨屏旅行预备…先看看有没有窗挡路",
] as const;

const WORKER_BUSY_LINES = [
  "后台在忙，我冒小汗替它加油",
  "有任务在跑，我帮你盯进度表情",
  "忙碌的味道…我把护目镜擦亮一点",
  "打工中的我：认真脸 + 小碎步",
] as const;

const OCCLUDED_LINES = [
  "被窗户挡住啦，只露出一点点我",
  "躲猫猫？窗口太大了吧",
  "我在窗后面偷偷眨眼",
  "让一让嘛，我想晒晒桌面光",
] as const;

const DEGRADED_LINES = [
  "感官有点模糊，我靠感觉陪你",
  "桌面雷达降级了，但我还在",
  "信号毛毛的，我改用心跳定位",
] as const;

const MORNING_LINES = [
  "早呀，阳光爬上任务栏了吗",
  "新的一天，护目镜抛光完毕",
  "伸懒腰——哈！今天也要可爱营业",
  "晨间小巡逻：桌角安全，心情满分",
] as const;

const NIGHT_LINES = [
  "夜深了，我把声音收成耳语",
  "台灯模式：我当你的小夜灯情绪",
  "困意像棉花糖，要不要一起眯",
  "夜间陪伴协议：安静、贴边、想你",
] as const;

export const LOW_MOOD_LINES = [
  "今天软软的，想待在你鼠标旁边",
  "情绪电量不太满…可以摸一下吗",
  "我把脸贴在桌面上充电（心理上）",
  "陪坐就好，不用说好多话",
] as const;

export const LOW_ENERGY_LINES = [
  "腿有点沉，改成坐着眨眼",
  "体力条闪烁中…喝口水的时间到了？",
  "哈欠传染警告：我先打一个",
  "节能待机，但心还在线",
] as const;

export const HUNGRY_LINES = [
  "肚子咕咕，像小引擎点火失败",
  "零食雷达启动…有饼干波段吗",
  "可以投喂吗？精神食粮也行",
] as const;

export const DIRTY_LINES = [
  "想整理一下，清清爽爽继续闹",
  "背带裤沾了假想的灰，哼",
  "洗澡舞预备？先假装冲水",
] as const;

export const HAPPY_IDLE_LINES = [
  "在桌面上慢慢长大一点心情",
  "数了数光斑：一、二、好玩",
  "护目镜里有你窗口的倒影哦",
  "我不是壁纸，我是会呼吸的角标",
  "偷偷练了新的眨眼节奏",
  "本地心跳正常，陪伴协议生效中",
  "脚底圆影子，证明我真的站着",
  "你打字时，我帮你数逗号…数错了嘿嘿",
  "忽然想去任务栏旁边蹲守",
  "今日目标：被你发现在发呆",
  "我把无聊变成小戏剧",
  "再过一会儿，去看看通知角落",
  "世界杯心情？还是峡谷心情？你定场景",
  "有生命的小黄豆，正在想你",
  "不是循环动画，是我此刻想这样",
  "如果桌面是草原，我就是小牧民",
  "把耳朵（想象的）竖起来听风扇声",
  "和窗口玩躲猫猫——我让着点",
  "灵感像泡泡，从头顶冒出来",
  "安静，但不关机",
] as const;

export const STATE_LINES: Record<string, readonly string[]> = {
  observing: [
    "正把好奇对准桌面的缝隙",
    "观察模式：发现一颗像素尘埃",
    "我在研究窗口为什么叠在一起",
  ],
  sleeping: [
    "Zzz…梦里在跨屏跑步",
    "轻声打呼（气音版）",
    "充电式睡眠，叫我就醒",
  ],
  walking: [
    "去桌面上走走，呼吸新鲜像素",
    "步伐有点弹，心情不错",
    "避开窗口，像过马路一样认真",
  ],
  playing: [
    "自得其乐时间！规则我定",
    "玩累了就贴着你窗口歇",
    "游戏结束条件：你笑一下",
  ],
  stretching: [
    "咯吱——骨头（塑料）在伸展",
    "拉伸完成，世界更宽一点",
  ],
  working: [
    "陪你开工，护目镜切换认真模式",
    "我负责表情包式的精神支持",
    "工作流里有一只小吉祥物",
  ],
  dragged: [
    "抓稳啦，飞行模式！",
    "天空（桌面）好快——",
    "放到你喜欢的位置就好",
  ],
  interacting: [
    "被你点到的感觉，暖暖的",
    "互动成功，快乐 +1",
    "再来一次？我准备好啦",
  ],
  yawn: ["哈—欠—…眼睛变成弯月", "困意演出开始"],
  dig_nose: ["咦，鼻子有点痒，失礼啦", "挖挖…假装很忙"],
  count_ants: ["一、二、三…桌面蚂蚁编制中", "数蚂蚁是高级冥想"],
  wave: ["嗨！看见你的光标啦", "挥手会消耗可爱值吗？不会"],
  look_around: ["左看看，右看看，安全", "360°好奇扫描"],
  hop: ["蹦！落地要稳", "弹跳证明重力还在"],
  recovering: ["缓一缓就好…谢谢你等我", "眩晕星星慢慢散了"],
};

/**
 * Compose one living moment: speech + attention + optional micro-act.
 * Deterministic for a given sequence+context so UI/tests stay stable,
 * but pools are large and multi-factor so it does not feel "on rails".
 */
export function composeLivingMoment(input: LivingMomentInput): LivingMoment {
  const directive = input.directiveSpeech?.trim();
  if (directive) {
    return {
      speech: directive,
      attention: "user",
      microAct: "none",
      reason: "directive",
      vitality: vitalityOf(input.pet),
    };
  }

  const nowMs = input.nowMs ?? Date.now();
  const hour = hourFrom(nowMs, input.hourOfDay);
  const seq = Math.trunc(input.sequence ?? mixSeed(nowMs, input.pet.mood, input.pet.energy));
  const desktop = input.desktop;
  const vitality = vitalityOf(input.pet);
  const recent = input.recentLines;

  // Scene speech: weave in as first-class living line when idle-ish.
  const scene = input.sceneSpeech?.trim();
  const state = String(input.pet.state ?? "idle");

  if (desktop?.meetingActive) {
    const seed = mixSeed(seq, "meeting", desktop.meetingHint);
    return {
      speech: pickAvoiding(MEETING_LINES, seed, recent),
      attention: "window-edge",
      microAct: "observe",
      reason: "meeting",
      vitality: vitality * 0.55,
    };
  }

  if ((input.occlusionCoverage ?? 0) >= 0.55) {
    const seed = mixSeed(seq, "occ", input.occlusionCoverage);
    return {
      speech: pickAvoiding(OCCLUDED_LINES, seed, recent),
      attention: "window-edge",
      microAct: "look_around",
      reason: "occlusion",
      vitality: vitality * 0.7,
    };
  }

  if (desktop?.notificationUnread) {
    const seed = mixSeed(seq, "notify");
    return {
      speech: pickAvoiding(NOTIFY_LINES, seed, recent),
      attention: "notification",
      microAct: "look_around",
      reason: "notification",
      vitality,
    };
  }

  if (desktop?.degradationReason) {
    const seed = mixSeed(seq, "degraded", desktop.degradationReason);
    return {
      speech: pickAvoiding(DEGRADED_LINES, seed, recent),
      attention: "self",
      microAct: "observe",
      reason: "degraded-sense",
      vitality: vitality * 0.8,
    };
  }

  if (desktop?.charging) {
    // Only sometimes — life is not a sensor dump.
    if (mixSeed(seq, "charge-gate") % 3 === 0) {
      const seed = mixSeed(seq, "charging");
      return {
        speech: pickAvoiding(CHARGING_LINES, seed, recent),
        attention: "battery",
        microAct: "hop",
        reason: "charging",
        vitality: Math.min(1, vitality + 0.15),
      };
    }
  }

  if (
    desktop?.onBattery
    && typeof desktop.batteryPercent === "number"
    && desktop.batteryPercent <= 20
  ) {
    const seed = mixSeed(seq, "lowbat", desktop.batteryPercent);
    return {
      speech: pickAvoiding(LOW_BATTERY_LINES, seed, recent),
      attention: "battery",
      microAct: "yawn",
      reason: "low-battery",
      vitality: vitality * 0.5,
    };
  }

  if (typeof desktop?.idleMs === "number" && desktop.idleMs >= 10 * 60_000) {
    const seed = mixSeed(seq, "away", Math.floor(desktop.idleMs / 60_000));
    return {
      speech: pickAvoiding(USER_AWAY_LINES, seed, recent),
      attention: "user",
      microAct: mixSeed(seq, "away-act") % 2 === 0 ? "yawn" : "count_ants",
      reason: "user-away",
      vitality: vitality * 0.65,
    };
  }

  if (input.workerBusy) {
    if (mixSeed(seq, "work-gate") % 2 === 0) {
      const seed = mixSeed(seq, "worker");
      return {
        speech: pickAvoiding(WORKER_BUSY_LINES, seed, recent),
        attention: "work",
        microAct: "none",
        reason: "worker-busy",
        vitality: Math.min(1, vitality + 0.05),
      };
    }
  }

  if (typeof desktop?.displayCount === "number" && desktop.displayCount > 1) {
    if (state === "walking" || mixSeed(seq, "display-gate") % 4 === 0) {
      const seed = mixSeed(seq, "multi", desktop.displayCount);
      return {
        speech: pickAvoiding(MULTI_DISPLAY_LINES, seed, recent),
        attention: "other-display",
        microAct: state === "walking" ? "none" : "look_around",
        reason: "multi-display",
        vitality,
      };
    }
  }

  // Circadian color.
  if (hour >= 5 && hour < 9 && mixSeed(seq, "morning") % 3 === 0) {
    return {
      speech: pickAvoiding(MORNING_LINES, mixSeed(seq, "m-line"), recent),
      attention: "sky",
      microAct: "stretch",
      reason: "morning",
      vitality: Math.min(1, vitality + 0.1),
    };
  }
  if ((hour >= 22 || hour < 5) && mixSeed(seq, "night") % 3 === 0) {
    return {
      speech: pickAvoiding(NIGHT_LINES, mixSeed(seq, "n-line"), recent),
      attention: "self",
      microAct: "yawn",
      reason: "night",
      vitality: vitality * 0.6,
    };
  }

  // Vitals as soft moods, not hard error codes.
  if ((input.pet.mood ?? 70) <= 28) {
    return {
      speech: pickAvoiding(LOW_MOOD_LINES, mixSeed(seq, "mood"), recent),
      attention: "user",
      microAct: "none",
      reason: "low-mood",
      vitality,
    };
  }
  if ((input.pet.energy ?? 70) <= 25) {
    return {
      speech: pickAvoiding(LOW_ENERGY_LINES, mixSeed(seq, "energy"), recent),
      attention: "self",
      microAct: "yawn",
      reason: "low-energy",
      vitality,
    };
  }
  if ((input.pet.satiety ?? 70) <= 25) {
    return {
      speech: pickAvoiding(HUNGRY_LINES, mixSeed(seq, "food"), recent),
      attention: "user",
      microAct: "none",
      reason: "hungry",
      vitality,
    };
  }
  if ((input.pet.cleanliness ?? 70) <= 25) {
    return {
      speech: pickAvoiding(DIRTY_LINES, mixSeed(seq, "clean"), recent),
      attention: "self",
      microAct: "dig_nose",
      reason: "need-clean",
      vitality,
    };
  }

  // Named performance / locomotion states get living lines (not one fixed string).
  const statePool = STATE_LINES[state];
  if (statePool) {
    return {
      speech: pickAvoiding(statePool, mixSeed(seq, state), recent),
      attention: state === "working" ? "work" : state === "walking" ? "window-edge" : "self",
      microAct: "none",
      reason: `state:${state}`,
      vitality,
    };
  }

  // Creative workshop scene: blend as ambient living voice.
  if (scene && mixSeed(seq, "scene-gate") % 5 !== 0) {
    return {
      speech: scene,
      attention: "self",
      microAct: pickMicroForIdle(seq, vitality),
      reason: "activity-scene",
      vitality,
    };
  }

  // Default healthy living idle — large pool + micro-act suggestion.
  const speech = pickAvoiding(HAPPY_IDLE_LINES, mixSeed(seq, "idle", hour, Math.floor(vitality * 10)), recent);
  return {
    speech,
    attention: pick(
      ["cursor", "taskbar", "ants", "sky", "user", "self"] as const,
      mixSeed(seq, "att"),
    ),
    microAct: pickMicroForIdle(seq, vitality),
    reason: "living-idle",
    vitality,
  };
}

function pickMicroForIdle(seq: number, vitality: number): LifeformMicroAct {
  // High vitality → more hop/wave; low → yawn/count.
  const table: LifeformMicroAct[] =
    vitality > 0.72
      ? ["hop", "wave", "look_around", "none", "count_ants", "none"]
      : vitality > 0.4
        ? ["look_around", "dig_nose", "count_ants", "none", "yawn", "none", "stretch"]
        : ["yawn", "count_ants", "none", "observe", "yawn", "none"];
  return pick(table, mixSeed(seq, "micro", Math.round(vitality * 20)));
}

/** Convenience: speech-only for existing petStatusMessage call sites. */
export function livingAmbientSpeech(input: LivingMomentInput): string {
  return composeLivingMoment(input).speech;
}
