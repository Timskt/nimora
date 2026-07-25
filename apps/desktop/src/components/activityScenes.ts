/**
 * Activity Scene Creative Workshop — user-authored themed stages for 灵灵.
 * Scenes decorate the desktop lifeform (speech / props / palette / particle bias)
 * without requiring network or third-party IP assets.
 *
 * Storage is local-first: the Rust capability base (system data store) is the
 * source of truth, mirrored into localStorage as a synchronous 60fps cache for
 * the overlay. Host writes are write-through (localStorage first for instant UI,
 * then async host persistence). Without a Tauri host we fall back to localStorage.
 */
import { desktopApi } from "../platform/desktop";

function hostSyncEnabled(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export type SceneCategory = "sports" | "esports" | "festival" | "season" | "geek" | "custom";
export type SceneParticle = "none" | "confetti" | "sparkles" | "snow" | "embers" | "pixels";
export type SceneMoodBias = "happy" | "excited" | "calm" | "curious" | "proud";
export type SceneStageDecor = "none" | "pitch" | "arena" | "lanterns" | "stars" | "lab";
export type ScenePropId =
  | "soccer_ball"
  | "trophy"
  | "flag"
  | "sword"
  | "crystal"
  | "lantern"
  | "firecracker"
  | "keyboard"
  | "coffee"
  | "star"
  | "shield"
  | "crown";

export interface ActivityScenePalette {
  primary: string;
  secondary: string;
  accent: string;
  ground: string;
  sky: string;
}

export interface ActivityScene {
  id: string;
  name: string;
  description: string;
  category: SceneCategory;
  icon: string;
  palette: ActivityScenePalette;
  particle: SceneParticle;
  propIds: ScenePropId[];
  speechLines: string[];
  moodBias: SceneMoodBias;
  animationBias: string[];
  stageDecor: SceneStageDecor;
  createdAt: number;
  updatedAt: number;
  source: "builtin" | "user";
  enabled: boolean;
}

export interface ActivitySceneInput {
  name: string;
  description?: string;
  category?: SceneCategory;
  icon?: string;
  palette?: Partial<ActivityScenePalette>;
  particle?: SceneParticle;
  propIds?: ScenePropId[];
  speechLines?: string[];
  moodBias?: SceneMoodBias;
  animationBias?: string[];
  stageDecor?: SceneStageDecor;
}

export const ACTIVITY_SCENES_STORAGE_KEY = "nimora.activity-scenes/v1";
export const ACTIVE_ACTIVITY_SCENE_KEY = "nimora.active-activity-scene/v1";

export const SCENE_PROP_CATALOG: Record<ScenePropId, { label: string; glyph: string }> = {
  soccer_ball: { label: "足球", glyph: "⚽" },
  trophy: { label: "奖杯", glyph: "🏆" },
  flag: { label: "旗帜", glyph: "🚩" },
  sword: { label: "光剑", glyph: "🗡️" },
  crystal: { label: "水晶", glyph: "💎" },
  lantern: { label: "灯笼", glyph: "🏮" },
  firecracker: { label: "烟花", glyph: "🎆" },
  keyboard: { label: "键盘", glyph: "⌨️" },
  coffee: { label: "咖啡", glyph: "☕" },
  star: { label: "星星", glyph: "⭐" },
  shield: { label: "护盾", glyph: "🛡️" },
  crown: { label: "王冠", glyph: "👑" },
};

export const SCENE_CATEGORY_LABEL: Record<SceneCategory, string> = {
  sports: "体育赛事",
  esports: "电竞对决",
  festival: "节日庆典",
  season: "季节氛围",
  geek: "极客/开发",
  custom: "自定义",
};

const DEFAULT_PALETTE: ActivityScenePalette = {
  primary: "#F7D117",
  secondary: "#2E7DB5",
  accent: "#7362E4",
  ground: "#3A6B3E",
  sky: "#E8F2FF",
};

function now(): number {
  return Date.now();
}

function newId(prefix: string): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return `${prefix}.${crypto.randomUUID()}`;
  }
  return `${prefix}.${now().toString(36)}.${Math.random().toString(36).slice(2, 8)}`;
}

/** Built-in showcase scenes (original theming — no third-party brand assets). */
export function builtinActivityScenes(): ActivityScene[] {
  const t = 1_700_000_000_000;
  return [
    {
      id: "scene.builtin.worldcup-night",
      name: "绿茵狂欢夜",
      description: "世界杯/足球赛季氛围：草坪、足球、奖杯与庆祝话术。用户可改名与扩展台词。",
      category: "sports",
      icon: "⚽",
      palette: {
        primary: "#F7D117",
        secondary: "#1B5E20",
        accent: "#FFD54F",
        ground: "#2E7D32",
        sky: "#B3E5FC",
      },
      particle: "confetti",
      propIds: ["soccer_ball", "trophy", "flag"],
      speechLines: [
        "进球了吗？我先在草坪热热身～",
        "今晚的绿茵好热闹，要不要一起欢呼？",
        "奖杯在发光……我先保管好气氛组！",
        "角球位我占好啦，你来主罚～",
      ],
      moodBias: "excited",
      animationBias: ["celebrate", "hop", "wave", "play"],
      stageDecor: "pitch",
      createdAt: t,
      updatedAt: t,
      source: "builtin",
      enabled: true,
    },
    {
      id: "scene.builtin.rift-night",
      name: "峡谷出征夜",
      description: "电竞/峡谷主题（原创氛围，非任何厂商素材）：水晶、护盾、出征口号。",
      category: "esports",
      icon: "🛡️",
      palette: {
        primary: "#7C5CFF",
        secondary: "#1A237E",
        accent: "#00E5FF",
        ground: "#12182B",
        sky: "#1A237E",
      },
      particle: "pixels",
      propIds: ["sword", "crystal", "shield", "crown"],
      speechLines: [
        "集合集合！今晚峡谷见～",
        "水晶还在闪，我先帮你盯着小地图。",
        "团战前记得买装备……我负责加油！",
        "胜利的光效我已经在脑内预演三遍了。",
      ],
      moodBias: "proud",
      animationBias: ["wave", "celebrate", "observe", "hop"],
      stageDecor: "arena",
      createdAt: t,
      updatedAt: t,
      source: "builtin",
      enabled: true,
    },
    {
      id: "scene.builtin.spring-festival",
      name: "新春团圆",
      description: "春节灯笼与烟花氛围，适合拜年话术与喜庆动作。",
      category: "festival",
      icon: "🧧",
      palette: {
        primary: "#FFD54F",
        secondary: "#C62828",
        accent: "#FF8A65",
        ground: "#5D1A1A",
        sky: "#FFF3E0",
      },
      particle: "sparkles",
      propIds: ["lantern", "firecracker", "star", "crown"],
      speechLines: [
        "恭喜发财～红包我只看不抢（大概）。",
        "灯笼亮了，新年愿望写给我吧。",
        "烟花好看，但记得护眼哦。",
        "团圆饭香味……我先练习夹菜动作。",
      ],
      moodBias: "happy",
      animationBias: ["wave", "celebrate", "play"],
      stageDecor: "lanterns",
      createdAt: t,
      updatedAt: t,
      source: "builtin",
      enabled: true,
    },
    {
      id: "scene.builtin.geek-lab",
      name: "极客调试夜",
      description: "开发者主题：键盘、咖啡、像素粒子，适合写 Skill / 跑 Agent 时的陪伴感。",
      category: "geek",
      icon: "⌨️",
      palette: {
        primary: "#F7D117",
        secondary: "#263238",
        accent: "#69F0AE",
        ground: "#102027",
        sky: "#1C2833",
      },
      particle: "pixels",
      propIds: ["keyboard", "coffee", "star"],
      speechLines: [
        "编译中…我先帮你数帧率。",
        "这杯咖啡是给我还是给你的？",
        "日志看起来很安静，是好兆头。",
        "Skill 写完记得提交，我看着你。",
      ],
      moodBias: "curious",
      animationBias: ["observe", "work", "look_around"],
      stageDecor: "lab",
      createdAt: t,
      updatedAt: t,
      source: "builtin",
      enabled: true,
    },
    {
      id: "scene.builtin.quiet-pond",
      name: "安静摸鱼塘",
      description: "低刺激日常场景：少道具、轻话术，适合专注或摸鱼。",
      category: "season",
      icon: "🍃",
      palette: {
        primary: "#F7D117",
        secondary: "#81C784",
        accent: "#A5D6A7",
        ground: "#4E6B4E",
        sky: "#E8F5E9",
      },
      particle: "none",
      propIds: ["star", "coffee"],
      speechLines: [
        "嘘——摸鱼中，别出声。",
        "阳光刚刚好，我数到第三片叶子了。",
        "你忙你的，我在旁边发呆。",
      ],
      moodBias: "calm",
      animationBias: ["idle", "yawn", "look_around"],
      stageDecor: "none",
      createdAt: t,
      updatedAt: t,
      source: "builtin",
      enabled: true,
    },
  ];
}

export function defaultPalette(): ActivityScenePalette {
  return { ...DEFAULT_PALETTE };
}

export function normalizeSceneName(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed || [...trimmed].length > 32) return null;
  return trimmed;
}

export function createUserActivityScene(input: ActivitySceneInput): ActivityScene | null {
  const name = normalizeSceneName(input.name);
  if (!name) return null;
  const stamp = now();
  const lines = (input.speechLines ?? [])
    .map((line) => line.trim())
    .filter(Boolean)
    .slice(0, 12);
  return {
    id: newId("scene.user"),
    name,
    description: (input.description ?? "用户在创意工坊创建的活动场景").trim().slice(0, 240),
    category: input.category ?? "custom",
    icon: (input.icon ?? "✨").trim().slice(0, 4) || "✨",
    palette: { ...DEFAULT_PALETTE, ...input.palette },
    particle: input.particle ?? "sparkles",
    propIds: uniqueProps(input.propIds ?? ["star"]),
    speechLines: lines.length ? lines : ["新场景已就位，陪你一起玩～"],
    moodBias: input.moodBias ?? "happy",
    animationBias: (input.animationBias ?? ["wave", "celebrate"]).slice(0, 6),
    stageDecor: input.stageDecor ?? "stars",
    createdAt: stamp,
    updatedAt: stamp,
    source: "user",
    enabled: true,
  };
}

function uniqueProps(ids: ScenePropId[]): ScenePropId[] {
  return [...new Set(ids)].slice(0, 6);
}

export function mergeSceneCatalog(stored: ActivityScene[] | null | undefined): ActivityScene[] {
  const builtins = builtinActivityScenes();
  const builtinIds = new Set(builtins.map((scene) => scene.id));
  const userScenes = (stored ?? []).filter((scene) => scene.source === "user" && !builtinIds.has(scene.id));
  // Allow user overrides of builtin enabled flag via stored mirror entries
  const overrides = new Map(
    (stored ?? [])
      .filter((scene) => scene.source === "builtin" && builtinIds.has(scene.id))
      .map((scene) => [scene.id, scene] as const),
  );
  const mergedBuiltins = builtins.map((scene) => {
    const override = overrides.get(scene.id);
    if (!override) return scene;
    return {
      ...scene,
      enabled: override.enabled,
      // users may customize speech/name locally without losing builtin identity
      name: override.name?.trim() ? override.name : scene.name,
      speechLines: override.speechLines?.length ? override.speechLines : scene.speechLines,
      propIds: override.propIds?.length ? uniqueProps(override.propIds) : scene.propIds,
      particle: override.particle ?? scene.particle,
      updatedAt: Math.max(scene.updatedAt, override.updatedAt ?? 0),
    };
  });
  return [...mergedBuiltins, ...userScenes].sort((a, b) => b.updatedAt - a.updatedAt);
}

export function pickSceneSpeech(scene: ActivityScene | null | undefined, sequence = 0): string | null {
  if (!scene?.enabled) return null;
  const lines = scene.speechLines.filter((line) => line.trim().length > 0);
  if (!lines.length) return null;
  const index = Math.abs(sequence) % lines.length;
  return lines[index] ?? lines[0] ?? null;
}

export function sceneCssVariables(scene: ActivityScene | null | undefined): Record<string, string> {
  if (!scene) return {};
  return {
    "--scene-primary": scene.palette.primary,
    "--scene-secondary": scene.palette.secondary,
    "--scene-accent": scene.palette.accent,
    "--scene-ground": scene.palette.ground,
    "--scene-sky": scene.palette.sky,
  };
}

export function loadStoredScenes(): ActivityScene[] {
  if (typeof localStorage === "undefined") return mergeSceneCatalog([]);
  try {
    const raw = localStorage.getItem(ACTIVITY_SCENES_STORAGE_KEY);
    if (!raw) return mergeSceneCatalog([]);
    const parsed = JSON.parse(raw) as ActivityScene[];
    if (!Array.isArray(parsed)) return mergeSceneCatalog([]);
    return mergeSceneCatalog(parsed);
  } catch {
    return mergeSceneCatalog([]);
  }
}

export function persistScenes(scenes: ActivityScene[]): void {
  // Persist user scenes + builtin overrides only.
  const payload = scenes.filter((scene) => scene.source === "user" || scene.source === "builtin");
  if (typeof localStorage !== "undefined") {
    // Instant local cache for the 60fps overlay read path.
    localStorage.setItem(ACTIVITY_SCENES_STORAGE_KEY, JSON.stringify(payload));
  }
  // Write-through to the Rust capability base (source of truth). Fire-and-forget:
  // the local cache already reflects the change; host failures fall back gracefully.
  void desktopApi.saveActivityScenes(payload as unknown[]).catch(() => {});
}

export function loadActiveSceneId(): string | null {
  if (typeof localStorage === "undefined") return null;
  try {
    return localStorage.getItem(ACTIVE_ACTIVITY_SCENE_KEY);
  } catch {
    return null;
  }
}

export function persistActiveSceneId(id: string | null): void {
  if (typeof localStorage !== "undefined") {
    if (!id) {
      localStorage.removeItem(ACTIVE_ACTIVITY_SCENE_KEY);
    } else {
      localStorage.setItem(ACTIVE_ACTIVITY_SCENE_KEY, id);
    }
  }
  void desktopApi.setActiveActivityScene(id).catch(() => {});
}

/**
 * Hydrate the local cache from the Rust capability base at startup.
 * The host store is authoritative; when it holds scenes we overwrite the
 * localStorage mirror so the overlay and workshop observe the persisted state.
 * On a non-Tauri host (browser/mock) this is a no-op and localStorage stands.
 * Returns the merged scene catalog and active scene id for the caller to apply.
 */
export async function hydrateScenesFromHost(): Promise<{
  scenes: ActivityScene[];
  activeSceneId: string | null;
}> {
  try {
    const snapshot = await desktopApi.activitySceneState();
    const rawScenes = Array.isArray(snapshot.scenes) ? (snapshot.scenes as ActivityScene[]) : [];
    if (rawScenes.length > 0 && typeof localStorage !== "undefined") {
      const payload = rawScenes.filter((scene) => scene.source === "user" || scene.source === "builtin");
      localStorage.setItem(ACTIVITY_SCENES_STORAGE_KEY, JSON.stringify(payload));
    }
    if (typeof localStorage !== "undefined") {
      if (snapshot.activeSceneId) {
        localStorage.setItem(ACTIVE_ACTIVITY_SCENE_KEY, snapshot.activeSceneId);
      }
    }
    return {
      scenes: mergeSceneCatalog(rawScenes.length > 0 ? rawScenes : null),
      activeSceneId: snapshot.activeSceneId ?? loadActiveSceneId(),
    };
  } catch {
    return { scenes: loadStoredScenes(), activeSceneId: loadActiveSceneId() };
  }
}

export function resolveActiveScene(scenes: ActivityScene[], activeId: string | null): ActivityScene | null {
  if (!activeId) return null;
  return scenes.find((scene) => scene.id === activeId && scene.enabled) ?? null;
}

export function updateUserScene(
  scenes: ActivityScene[],
  id: string,
  patch: Partial<ActivitySceneInput> & { enabled?: boolean },
): ActivityScene[] {
  return scenes.map((scene) => {
    if (scene.id !== id) return scene;
    if (scene.source === "builtin") {
      return {
        ...scene,
        name: patch.name ? normalizeSceneName(patch.name) ?? scene.name : scene.name,
        speechLines: patch.speechLines?.length ? patch.speechLines.map((l) => l.trim()).filter(Boolean).slice(0, 12) : scene.speechLines,
        propIds: patch.propIds ? uniqueProps(patch.propIds) : scene.propIds,
        particle: patch.particle ?? scene.particle,
        enabled: patch.enabled ?? scene.enabled,
        updatedAt: now(),
      };
    }
    const name = patch.name ? normalizeSceneName(patch.name) : scene.name;
    if (!name) return scene;
    return {
      ...scene,
      name,
      description: patch.description !== undefined ? patch.description.trim().slice(0, 240) : scene.description,
      category: patch.category ?? scene.category,
      icon: patch.icon ? patch.icon.trim().slice(0, 4) || scene.icon : scene.icon,
      palette: { ...scene.palette, ...patch.palette },
      particle: patch.particle ?? scene.particle,
      propIds: patch.propIds ? uniqueProps(patch.propIds) : scene.propIds,
      speechLines: patch.speechLines?.length
        ? patch.speechLines.map((l) => l.trim()).filter(Boolean).slice(0, 12)
        : scene.speechLines,
      moodBias: patch.moodBias ?? scene.moodBias,
      animationBias: patch.animationBias ?? scene.animationBias,
      stageDecor: patch.stageDecor ?? scene.stageDecor,
      enabled: patch.enabled ?? scene.enabled,
      updatedAt: now(),
    };
  });
}

export function removeUserScene(scenes: ActivityScene[], id: string): ActivityScene[] {
  return scenes.filter((scene) => !(scene.id === id && scene.source === "user"));
}

/** Template for AI / form prefill when user mentions world cup / rift / etc. */
export function sceneTemplateForPrompt(prompt: string): ActivitySceneInput {
  const text = prompt.toLowerCase();
  if (/世界杯|足球|绿茵|world\s*cup|soccer|football/.test(text)) {
    return {
      name: "我的绿茵夜",
      category: "sports",
      icon: "⚽",
      particle: "confetti",
      propIds: ["soccer_ball", "trophy", "flag"],
      speechLines: ["进球！我先翻个跟头庆祝～", "角球交给你，我负责气氛。"],
      moodBias: "excited",
      animationBias: ["celebrate", "hop", "play"],
      stageDecor: "pitch",
      palette: { primary: "#F7D117", secondary: "#1B5E20", accent: "#FFD54F", ground: "#2E7D32", sky: "#B3E5FC" },
    };
  }
  if (/英雄联盟|lol|峡谷|电竞|团战|召唤师/.test(text)) {
    return {
      name: "我的峡谷夜",
      category: "esports",
      icon: "🛡️",
      particle: "pixels",
      propIds: ["sword", "crystal", "shield", "crown"],
      speechLines: ["集合出征！我先帮你看小地图。", "团战倒数三秒——加油！"],
      moodBias: "proud",
      animationBias: ["wave", "celebrate", "observe"],
      stageDecor: "arena",
      palette: { primary: "#7C5CFF", secondary: "#1A237E", accent: "#00E5FF", ground: "#12182B", sky: "#1A237E" },
    };
  }
  if (/春节|新年|元宵|中秋|节日/.test(text)) {
    return {
      name: "我的节日夜",
      category: "festival",
      icon: "🧧",
      particle: "sparkles",
      propIds: ["lantern", "firecracker", "star"],
      speechLines: ["节日快乐！要不要一起许个愿？"],
      moodBias: "happy",
      stageDecor: "lanterns",
    };
  }
  if (/代码|debug|极客|开发|skill|agent/.test(text)) {
    return {
      name: "我的调试夜",
      category: "geek",
      icon: "⌨️",
      particle: "pixels",
      propIds: ["keyboard", "coffee"],
      speechLines: ["编译中…我在旁边守着。"],
      moodBias: "curious",
      stageDecor: "lab",
    };
  }
  return {
    name: "我的活动场景",
    category: "custom",
    icon: "✨",
    particle: "sparkles",
    propIds: ["star"],
    speechLines: ["新场景登场，陪你一起玩～"],
    moodBias: "happy",
    stageDecor: "stars",
  };
}
