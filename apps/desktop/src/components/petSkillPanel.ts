/**
 * Skill-panel view model for the desktop lifeform's radial menu.
 *
 * Milestone 3 ("skills made concrete") asks the pet to "pull out" a skill panel
 * from its right-click / radial menu — the Skills the creature can perform,
 * each bound to the animation it plays while working (the `pet_behavior` field
 * added to the Skill manifest). This module is the pure view-model builder for
 * that panel: given the active skills the host reports, it produces a bounded,
 * sorted, deduplicated list of menu entries the overlay renders.
 *
 * It is DOM-free and host-independent — no React, no window, no IPC — so it
 * unit-tests in isolation and stays inside the architecture boundary. The
 * caller owns fetching the active skills and dispatching the chosen action.
 */

/** The pet animation a skill plays while it works. Mirrors `PetAction`. */
export type SkillPetBehavior =
  | "idle"
  | "observe"
  | "walk"
  | "play"
  | "perch"
  | "climb"
  | "peek"
  | "stretch"
  | "sleep"
  | "work"
  | "celebrate";

/** One agent-tool a skill contributes, as the panel needs to show it. */
export interface SkillPanelTool {
  /** Fully-qualified tool id (e.g. `publisher.skill.tool`). */
  id: string;
  /** Human title shown in the menu. */
  title: string;
  /** Animation the pet plays while running this tool, if the manifest bound one. */
  petBehavior: SkillPetBehavior | null;
}

/** An active skill with its invocable tools. */
export interface ActiveSkillView {
  id: string;
  tools: SkillPanelTool[];
}

/** A single, ready-to-render entry in the pet's skill panel. */
export interface SkillPanelEntry {
  /** Tool id — stable key and dispatch target. */
  toolId: string;
  /** Owning skill id. */
  skillId: string;
  /** Title shown in the menu. */
  title: string;
  /** Animation cue to preview / play, or `null` if none was bound. */
  petBehavior: SkillPetBehavior | null;
}

/** Upper bound on panel entries so a misbehaving host can never flood the menu. */
export const MAX_SKILL_PANEL_ENTRIES = 32;

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

/**
 * Builds the pet's skill-panel entries from the active skills the host reports.
 *
 * Entries are flattened across skills (one per invocable tool), deduplicated by
 * tool id (first occurrence wins), sorted by title then tool id for a stable
 * menu order, and capped at {@link MAX_SKILL_PANEL_ENTRIES}. Tools with a blank
 * id or title are dropped rather than rendered as empty menu items. A missing or
 * unrecognized `petBehavior` becomes `null` (the pet just performs the skill
 * without a bound animation) rather than breaking the entry.
 */
export function buildSkillPanel(skills: readonly ActiveSkillView[]): SkillPanelEntry[] {
  const seen = new Set<string>();
  const entries: SkillPanelEntry[] = [];
  for (const skill of skills) {
    if (!isNonEmptyString(skill.id)) continue;
    for (const tool of skill.tools) {
      if (!isNonEmptyString(tool.id) || !isNonEmptyString(tool.title)) continue;
      if (seen.has(tool.id)) continue;
      seen.add(tool.id);
      entries.push({
        toolId: tool.id,
        skillId: skill.id,
        title: tool.title.trim(),
        petBehavior: normalizeBehavior(tool.petBehavior),
      });
    }
  }
  entries.sort((a, b) => a.title.localeCompare(b.title) || a.toolId.localeCompare(b.toolId));
  return entries.slice(0, MAX_SKILL_PANEL_ENTRIES);
}

const BEHAVIORS = new Set<SkillPetBehavior>([
  "idle",
  "observe",
  "walk",
  "play",
  "perch",
  "climb",
  "peek",
  "stretch",
  "sleep",
  "work",
  "celebrate",
]);

/** Coerces an arbitrary value to a known behavior, or `null` if unrecognized. */
function normalizeBehavior(value: unknown): SkillPetBehavior | null {
  return typeof value === "string" && BEHAVIORS.has(value as SkillPetBehavior)
    ? (value as SkillPetBehavior)
    : null;
}

/** Whether the panel has any entries to show (drives an empty-state message). */
export function skillPanelIsEmpty(entries: readonly SkillPanelEntry[]): boolean {
  return entries.length === 0;
}
