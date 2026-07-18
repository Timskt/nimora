export const PET_DRAG_THRESHOLD_PX = 6;
export const PET_LONG_PRESS_MS = 500;
export const PET_SINGLE_CLICK_DELAY_MS = 220;

export interface PointerOrigin {
  clientX: number;
  clientY: number;
}

export function exceedsPetDragThreshold(origin: PointerOrigin, clientX: number, clientY: number): boolean {
  return Math.hypot(clientX - origin.clientX, clientY - origin.clientY) >= PET_DRAG_THRESHOLD_PX;
}
