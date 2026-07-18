export const BOND_POINTS_PER_LEVEL = 50;

export interface PetGrowthSnapshot {
  bondPoints: number;
  level: number;
  levelProgress: number;
  pointsPerLevel: number;
  progressPercent: number;
}

export function petGrowth(bondPoints: number | undefined, affinity: number): PetGrowthSnapshot {
  const validBondPoints = Number.isSafeInteger(bondPoints) && (bondPoints ?? -1) >= 0
    ? bondPoints ?? 0
    : 0;
  const effectiveBondPoints = Math.max(validBondPoints, Math.max(0, Math.min(100, Math.trunc(affinity))));
  const levelProgress = effectiveBondPoints % BOND_POINTS_PER_LEVEL;

  return {
    bondPoints: effectiveBondPoints,
    level: Math.floor(effectiveBondPoints / BOND_POINTS_PER_LEVEL) + 1,
    levelProgress,
    pointsPerLevel: BOND_POINTS_PER_LEVEL,
    progressPercent: (levelProgress / BOND_POINTS_PER_LEVEL) * 100,
  };
}
