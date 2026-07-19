export const PET_BUBBLE_DURATION_MS = 4200;

export interface PetBubbleVisibilityContext {
  menuOpen: boolean;
  pointerActive: boolean;
}

export function canPresentPetBubble(context: PetBubbleVisibilityContext): boolean {
  return !context.menuOpen && !context.pointerActive;
}
