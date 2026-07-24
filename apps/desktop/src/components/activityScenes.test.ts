import { describe, expect, it, beforeEach } from "vitest";
import {
  builtinActivityScenes,
  createUserActivityScene,
  loadActiveSceneId,
  loadStoredScenes,
  mergeSceneCatalog,
  persistActiveSceneId,
  persistScenes,
  pickSceneSpeech,
  removeUserScene,
  resolveActiveScene,
  sceneCssVariables,
  sceneTemplateForPrompt,
  updateUserScene,
} from "./activityScenes";

class MemoryStorage {
  private store = new Map<string, string>();
  get length(): number {
    return this.store.size;
  }
  clear(): void {
    this.store.clear();
  }
  getItem(key: string): string | null {
    return this.store.has(key) ? (this.store.get(key) as string) : null;
  }
  setItem(key: string, value: string): void {
    this.store.set(key, String(value));
  }
  removeItem(key: string): void {
    this.store.delete(key);
  }
  key(index: number): string | null {
    return [...this.store.keys()][index] ?? null;
  }
}

describe("activityScenes workshop domain", () => {
  beforeEach(() => {
    (globalThis as { localStorage?: Storage }).localStorage = new MemoryStorage() as unknown as Storage;
  });

  it("ships original themed builtins including sports and esports", () => {
    const scenes = builtinActivityScenes();
    expect(scenes.length).toBeGreaterThanOrEqual(5);
    expect(scenes.some((scene) => scene.category === "sports")).toBe(true);
    expect(scenes.some((scene) => scene.category === "esports")).toBe(true);
    expect(scenes.every((scene) => scene.source === "builtin")).toBe(true);
    const names = scenes.map((scene) => scene.name);
    for (const required of ["绿茵狂欢夜", "峡谷出征夜", "新春团圆", "极客调试夜", "安静摸鱼塘"]) {
      expect(names).toContain(required);
    }
  });

  it("creates user scenes with clamped props and speech", () => {
    const scene = createUserActivityScene({
      name: "  我的决赛夜  ",
      propIds: ["soccer_ball", "trophy", "flag", "star", "crown", "shield", "coffee"],
      speechLines: ["a", "", "b", "c"],
    });
    expect(scene).not.toBeNull();
    expect(scene!.name).toBe("我的决赛夜");
    expect(scene!.source).toBe("user");
    expect(scene!.propIds.length).toBeLessThanOrEqual(6);
    expect(scene!.speechLines).toEqual(["a", "b", "c"]);
  });

  it("rejects empty names", () => {
    expect(createUserActivityScene({ name: "   " })).toBeNull();
  });

  it("merges builtins with user scenes", () => {
    const user = createUserActivityScene({ name: "自定义夜" })!;
    const merged = mergeSceneCatalog([user]);
    expect(merged.some((scene) => scene.id === user.id)).toBe(true);
    expect(merged.some((scene) => scene.id.startsWith("scene.builtin."))).toBe(true);
  });

  it("picks speech by sequence and resolves active", () => {
    const scene = builtinActivityScenes()[0]!;
    const line = pickSceneSpeech(scene, 1);
    expect(line).toBe(scene.speechLines[1 % scene.speechLines.length]);
    expect(resolveActiveScene([scene], scene.id)?.id).toBe(scene.id);
    expect(resolveActiveScene([{ ...scene, enabled: false }], scene.id)).toBeNull();
  });

  it("updates and removes user scenes only", () => {
    const user = createUserActivityScene({ name: "临时" })!;
    const builtin = builtinActivityScenes()[0]!;
    const updated = updateUserScene([user, builtin], user.id, { name: "新名" });
    expect(updated.find((scene) => scene.id === user.id)?.name).toBe("新名");
    const removed = removeUserScene(updated, user.id);
    expect(removed.some((scene) => scene.id === user.id)).toBe(false);
    expect(removeUserScene(updated, builtin.id).some((scene) => scene.id === builtin.id)).toBe(true);
  });

  it("templates world cup and rift prompts", () => {
    expect(sceneTemplateForPrompt("世界杯决赛").category).toBe("sports");
    expect(sceneTemplateForPrompt("英雄联盟排位").category).toBe("esports");
    expect(sceneTemplateForPrompt("写 skill 通宵").category).toBe("geek");
  });

  it("exposes palette as scene css variables and empty for null", () => {
    const scene = builtinActivityScenes()[0]!;
    const vars = sceneCssVariables(scene);
    expect(vars["--scene-primary"]).toBe(scene.palette.primary);
    expect(vars["--scene-secondary"]).toBe(scene.palette.secondary);
    expect(vars["--scene-accent"]).toBe(scene.palette.accent);
    expect(vars["--scene-ground"]).toBe(scene.palette.ground);
    expect(vars["--scene-sky"]).toBe(scene.palette.sky);
    expect(sceneCssVariables(null)).toEqual({});
  });

  it("never surfaces speech for a disabled scene", () => {
    const scene = builtinActivityScenes()[0]!;
    expect(pickSceneSpeech({ ...scene, enabled: false }, 0)).toBeNull();
    expect(pickSceneSpeech(null, 0)).toBeNull();
    expect(pickSceneSpeech({ ...scene, speechLines: ["  ", ""] }, 0)).toBeNull();
  });

  it("round-trips user scenes through localStorage persistence", () => {
    localStorage.clear();
    const user = createUserActivityScene({ name: "存档夜" })!;
    persistScenes([user, builtinActivityScenes()[0]!]);
    const restored = loadStoredScenes();
    expect(restored.some((scene) => scene.id === user.id)).toBe(true);
    expect(restored.some((scene) => scene.id.startsWith("scene.builtin."))).toBe(true);
  });

  it("round-trips active scene id and clears on null", () => {
    localStorage.clear();
    persistActiveSceneId("scene.user.demo");
    expect(loadActiveSceneId()).toBe("scene.user.demo");
    persistActiveSceneId(null);
    expect(loadActiveSceneId()).toBeNull();
  });

  it("loads a merged catalog when storage is empty or corrupt", () => {
    localStorage.clear();
    expect(loadStoredScenes().length).toBe(builtinActivityScenes().length);
    localStorage.setItem("nimora.activity-scenes/v1", "not-json{");
    expect(loadStoredScenes().length).toBe(builtinActivityScenes().length);
  });

  it("applies stored builtin override enabled flag without losing identity", () => {
    const builtin = builtinActivityScenes()[0]!;
    const merged = mergeSceneCatalog([{ ...builtin, enabled: false, updatedAt: builtin.updatedAt + 10 }]);
    const overridden = merged.find((scene) => scene.id === builtin.id);
    expect(overridden?.enabled).toBe(false);
    expect(overridden?.source).toBe("builtin");
    expect(overridden?.name).toBe(builtin.name);
  });
});
