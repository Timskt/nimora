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
  "pet.click": { name: "happy", weight: 0.85 },
  "pet.celebrate": { name: "happy", weight: 1 },
  "pet.drag": { name: "surprised", weight: 0.55 },
  "pet.sleep": { name: "relaxed", weight: 0.7 },
  "pet.error": { name: "sad", weight: 0.65 },
};

export function resolveVrmExpression(action: string): VrmExpressionBinding | null {
  return ACTION_EXPRESSIONS[action] ?? null;
}

export function applyVrmExpression(
  controller: VrmExpressionController | null | undefined,
  action: string,
): boolean {
  if (!controller) return false;

  try {
    controller.resetValues();
    const binding = resolveVrmExpression(action);
    if (!binding) return true;
    if (!controller.getExpression(binding.name)) return false;
    controller.setValue(binding.name, Math.min(Math.max(binding.weight, 0), 1));
    return true;
  } catch {
    return false;
  }
}
