import { describe, expect, it } from "vitest";
import {
  buildSkillPanel,
  MAX_SKILL_PANEL_ENTRIES,
  skillPanelIsEmpty,
  type ActiveSkillView,
} from "./petSkillPanel";

const SKILLS: ActiveSkillView[] = [
  {
    id: "acme.coding",
    tools: [
      { id: "acme.coding.type", title: "敲代码", petBehavior: "work" },
      { id: "acme.coding.review", title: "审阅", petBehavior: "observe" },
    ],
  },
  {
    id: "acme.play",
    tools: [{ id: "acme.play.dance", title: "跳舞", petBehavior: "celebrate" }],
  },
];

describe("buildSkillPanel", () => {
  it("flattens tools across skills into panel entries", () => {
    const entries = buildSkillPanel(SKILLS);
    expect(entries).toHaveLength(3);
    expect(entries.map((entry) => entry.toolId).sort()).toEqual([
      "acme.coding.review",
      "acme.coding.type",
      "acme.play.dance",
    ]);
  });

  it("carries the bound pet behavior for each tool", () => {
    const entries = buildSkillPanel(SKILLS);
    const typing = entries.find((entry) => entry.toolId === "acme.coding.type");
    expect(typing?.petBehavior).toBe("work");
    expect(typing?.skillId).toBe("acme.coding");
  });

  it("sorts entries by title then tool id for a stable menu order", () => {
    const entries = buildSkillPanel(SKILLS);
    // 审阅 / 敲代码 / 跳舞 — localeCompare ordering is deterministic.
    const titles = entries.map((entry) => entry.title);
    expect(titles).toEqual([...titles].sort((a, b) => a.localeCompare(b)));
  });

  it("deduplicates tools by id, first occurrence wins", () => {
    const dup: ActiveSkillView[] = [
      { id: "a.one", tools: [{ id: "a.one.t", title: "First", petBehavior: "work" }] },
      { id: "b.two", tools: [{ id: "a.one.t", title: "Second", petBehavior: "play" }] },
    ];
    const entries = buildSkillPanel(dup);
    expect(entries).toHaveLength(1);
    expect(entries[0]!.title).toBe("First");
    expect(entries[0]!.skillId).toBe("a.one");
  });

  it("drops tools with a blank id or title rather than rendering empties", () => {
    const entries = buildSkillPanel([
      {
        id: "acme.s",
        tools: [
          { id: "", title: "no id", petBehavior: null },
          { id: "acme.s.blank", title: "   ", petBehavior: null },
          { id: "acme.s.ok", title: "Fine", petBehavior: null },
        ],
      },
    ]);
    expect(entries).toHaveLength(1);
    expect(entries[0]!.toolId).toBe("acme.s.ok");
  });

  it("skips skills with a blank id", () => {
    const entries = buildSkillPanel([
      { id: "  ", tools: [{ id: "x.y.z", title: "orphan", petBehavior: null }] },
    ]);
    expect(entries).toHaveLength(0);
  });

  it("normalizes an unknown pet behavior to null", () => {
    const entries = buildSkillPanel([
      {
        id: "acme.s",
        tools: [{ id: "acme.s.t", title: "T", petBehavior: "moonwalk" as never }],
      },
    ]);
    expect(entries[0]!.petBehavior).toBeNull();
  });

  it("caps the panel at the maximum entry count", () => {
    const many: ActiveSkillView = {
      id: "acme.big",
      tools: Array.from({ length: MAX_SKILL_PANEL_ENTRIES + 10 }, (_unused, index) => ({
        id: `acme.big.t${String(index).padStart(3, "0")}`,
        title: `Tool ${index}`,
        petBehavior: "work" as const,
      })),
    };
    expect(buildSkillPanel([many])).toHaveLength(MAX_SKILL_PANEL_ENTRIES);
  });

  it("returns an empty panel for no skills", () => {
    expect(buildSkillPanel([])).toEqual([]);
  });
});

describe("skillPanelIsEmpty", () => {
  it("reports empty and non-empty panels", () => {
    expect(skillPanelIsEmpty([])).toBe(true);
    expect(skillPanelIsEmpty(buildSkillPanel(SKILLS))).toBe(false);
  });
});
