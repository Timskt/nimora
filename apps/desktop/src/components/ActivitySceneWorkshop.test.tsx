import { renderToStaticMarkup } from "react-dom/server";
import { beforeEach, describe, expect, it } from "vitest";
import { ActivitySceneWorkshop, RuntimeActivityPanel } from "./ActivitySceneWorkshop";
import { createUserActivityScene, persistActiveSceneId, persistScenes } from "./activityScenes";

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

describe("ActivitySceneWorkshop", () => {
  beforeEach(() => {
    (globalThis as { localStorage?: Storage }).localStorage = new MemoryStorage() as unknown as Storage;
  });

  it("renders the workshop hero, designer, and builtin gallery cards", () => {
    const markup = renderToStaticMarkup(<ActivitySceneWorkshop />);
    expect(markup).toContain("scene-workshop");
    expect(markup).toContain("给灵灵做一场专属活动场景");
    expect(markup).toContain("场景设计器");
    expect(markup).toContain('data-active="false"');
    expect(markup).toContain("日常陪伴");
    // Builtins are seeded into the gallery under the "全部" filter.
    expect(markup).toContain("绿茵狂欢夜");
    expect(markup).toContain("峡谷出征夜");
    expect(markup).toContain("scene-card");
  });

  it("reflects a persisted active scene in the hero pill", () => {
    const scene = createUserActivityScene({ name: "决赛夜", icon: "🏆" })!;
    persistScenes([scene]);
    persistActiveSceneId(scene.id);
    const markup = renderToStaticMarkup(<ActivitySceneWorkshop />);
    expect(markup).toContain('data-active="true"');
    expect(markup).toContain("决赛夜");
    expect(markup).toContain("退出场景");
    expect(markup).toContain("使用中");
  });

  it("keeps the empty gallery hint out when scenes exist", () => {
    const markup = renderToStaticMarkup(<ActivitySceneWorkshop />);
    expect(markup).not.toContain("这一分类还没有场景");
  });
});

describe("RuntimeActivityPanel", () => {
  it("renders bounded counts and the privacy note without conversation content", () => {
    const markup = renderToStaticMarkup(
      <RuntimeActivityPanel
        outbox={{ pending: 2, leased: 1, delivered: 9, deadLetter: 0 }}
        activities={[{ title: "本地任务", meta: "已完成", tone: "ok" }]}
      />,
    );
    expect(markup).toContain("本地健康（隐私安全）");
    expect(markup).toContain("运行健康");
    expect(markup).toContain("仅本地");
    expect(markup).toContain("只展示有界计数");
  });

  it("flags dead-letter backlog as needing attention", () => {
    const markup = renderToStaticMarkup(
      <RuntimeActivityPanel outbox={{ pending: 0, leased: 0, delivered: 0, deadLetter: 3 }} />,
    );
    expect(markup).toContain("需要查看");
    expect(markup).toContain("attention");
  });

  it("shows a reading state before outbox counts arrive", () => {
    const markup = renderToStaticMarkup(<RuntimeActivityPanel outbox={null} />);
    expect(markup).toContain("读取中");
  });
});
