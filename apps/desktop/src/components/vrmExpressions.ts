export const VRM_EXPRESSION_PRESETS = [
  "happy",
  "sad",
  "surprised",
  "relaxed",
] as const;

export type VrmExpressionPreset = (typeof VRM_EXPRESSION_PRESETS)[number];

export interface VrmExpressionBinding {
  name: VrmExpressionPreset;
  weight: number;
}

export interface VrmExpressionController {
  getExpression(name: string): unknown | null;
  resetValues(): void;
  setValue(name: string, value: number): void;
}

const ACTION_EXPRESSIONS: Readonly<Record<string, VrmExpressionBinding>> = {
  "pet.observe": { name: "surprised", weight: 0.22 },
  "pet.perch": { name: "relaxed", weight: 0.28 },
  "pet.climb": { name: "surprised", weight: 0.18 },
  "pet.peek": { name: "surprised", weight: 0.42 },
  "pet.stretch": { name: "happy", weight: 0.32 },
  "pet.click": { name: "happy", weight: 0.85 },
  "pet.celebrate": { name: "happy", weight: 1 },
  "pet.drag": { name: "surprised", weight: 0.55 },
  "pet.sleep": { name: "relaxed", weight: 0.7 },
  "pet.error": { name: "sad", weight: 0.65 },
};

export type VrmExpressionOverrides = Readonly<Record<string, {
  preset: VrmExpressionPreset;
  weight: number;
}>>;

export function resolveVrmExpression(
  action: string,
  overrides?: VrmExpressionOverrides | null,
): VrmExpressionBinding | null {
  const override = overrides?.[action];
  return override ? { name: override.preset, weight: override.weight } : ACTION_EXPRESSIONS[action] ?? null;
}

export function applyVrmExpression(
  controller: VrmExpressionController | null | undefined,
  action: string,
  overrides?: VrmExpressionOverrides | null,
): boolean {
  if (!controller) return false;

  try {
    controller.resetValues();
    const binding = resolveVrmExpression(action, overrides);
    if (!binding) return true;
    if (!controller.getExpression(binding.name)) return false;
    controller.setValue(binding.name, Math.min(Math.max(binding.weight, 0), 1));
    return true;
  } catch {
    return false;
  }
}
