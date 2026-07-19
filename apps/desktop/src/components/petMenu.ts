export function nextMenuItemIndex(current: number, count: number, key: string): number | null {
  if (count <= 0) return null;
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  if (key === "ArrowRight" || key === "ArrowDown") return (current + 1) % count;
  if (key === "ArrowLeft" || key === "ArrowUp") return (current - 1 + count) % count;
  return null;
}

export function isPetMenuShortcut(key: string, shiftKey: boolean): boolean {
  return key === "ContextMenu" || (key === "F10" && shiftKey);
}

export function focusMenuItem(item: Pick<HTMLElement, "focus" | "scrollIntoView"> | undefined) {
  if (!item) return;
  item.focus();
  item.scrollIntoView({ block: "nearest", inline: "nearest" });
}
