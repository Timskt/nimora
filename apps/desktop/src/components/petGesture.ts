export const PET_DRAG_THRESHOLD_PX = 12;
export const PET_LONG_PRESS_MS = 500;
export const PET_SINGLE_CLICK_DELAY_MS = 220;
export const PET_STROKE_MIN_DISTANCE_PX = 32;
export const PET_STROKE_MIN_DURATION_MS = 160;

export interface PointerOrigin {
  clientX: number;
  clientY: number;
}

export interface PetGestureTrail {
  origin: PointerOrigin;
  previous: PointerOrigin;
  startedAtMs: number;
  distancePx: number;
  reversals: number;
  previousDirection: PointerOrigin | null;
}

export function createPetGestureTrail(origin: PointerOrigin, startedAtMs: number): PetGestureTrail {
  return { origin, previous: origin, startedAtMs, distancePx: 0, reversals: 0, previousDirection: null };
}

export function appendPetGesturePoint(trail: PetGestureTrail, point: PointerOrigin): PetGestureTrail {
  const delta = { clientX: point.clientX - trail.previous.clientX, clientY: point.clientY - trail.previous.clientY };
  const segment = Math.hypot(delta.clientX, delta.clientY);
  if (segment < 1) return trail;
  const direction = { clientX: delta.clientX / segment, clientY: delta.clientY / segment };
  const reversed = trail.previousDirection != null
    && direction.clientX * trail.previousDirection.clientX + direction.clientY * trail.previousDirection.clientY <= -0.35;
  return {
    ...trail,
    previous: point,
    distancePx: trail.distancePx + segment,
    reversals: trail.reversals + (reversed ? 1 : 0),
    previousDirection: direction,
  };
}

export function exceedsPetDragThreshold(origin: PointerOrigin, clientX: number, clientY: number): boolean {
  return Math.hypot(clientX - origin.clientX, clientY - origin.clientY) >= PET_DRAG_THRESHOLD_PX;
}

export function isPetStroke(trail: PetGestureTrail, endedAtMs: number): boolean {
  return endedAtMs - trail.startedAtMs >= PET_STROKE_MIN_DURATION_MS
    && trail.distancePx >= PET_STROKE_MIN_DISTANCE_PX
    && trail.reversals >= 1
    && !exceedsPetDragThreshold(trail.origin, trail.previous.clientX, trail.previous.clientY);
}
