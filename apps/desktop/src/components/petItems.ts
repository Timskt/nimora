import type { DesktopSnapshot, PetItemId } from "../platform/desktop";

export const petItemMetadata: Record<PetItemId, { glyph: string; label: string; effect: string }> = {
  berry_bite: { glyph: "●", label: "莓果小食", effect: "饱腹 +35 · 精力 +30" },
  star_ball: { glyph: "✦", label: "星星球", effect: "心情 +18 · 陪伴 +3" },
  bubble_soap: { glyph: "○", label: "泡泡皂", effect: "清洁 +45 · 心情 +8" },
};

export function petItemPresentation(itemId: PetItemId) {
  return petItemMetadata[itemId];
}

export function petInventoryQuantity(inventory: DesktopSnapshot["pet"]["inventory"]) {
  return inventory.reduce((total, stack) => total + stack.quantity, 0);
}
