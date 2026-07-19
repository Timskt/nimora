import { useCallback, useEffect, useRef, useState } from "react";

export const PET_BUBBLE_DURATION_MS = 4200;
export const PET_STATUS_COOLDOWN_MS = 8000;

export type PetBubbleChannel = "status" | "feedback" | "error";

export interface PetBubbleRequest {
  text: string;
  channel: PetBubbleChannel;
}

export interface PetBubbleHistory {
  lastStatusAtMs: number;
  protectedUntilMs: number;
}

export interface PetBubblePresentation {
  message: string;
  revision: number;
  visible: boolean;
}

export interface PetBubbleVisibilityContext {
  menuOpen: boolean;
  pointerActive: boolean;
}

export function canPresentPetBubble(context: PetBubbleVisibilityContext): boolean {
  return !context.menuOpen && !context.pointerActive;
}

export function shouldAcceptPetBubble(history: PetBubbleHistory, request: PetBubbleRequest, nowMs: number): boolean {
  if (request.text.trim().length === 0) return false;
  return request.channel !== "status" || (
    nowMs >= history.protectedUntilMs
    && nowMs - history.lastStatusAtMs >= PET_STATUS_COOLDOWN_MS
  );
}

export function nextPetBubblePresentation(current: PetBubblePresentation, message: string): PetBubblePresentation {
  return { message, revision: current.revision + 1, visible: true };
}

export function usePetBubble(initialMessage: string) {
  const [presentation, setPresentation] = useState<PetBubblePresentation>({
    message: initialMessage,
    revision: 0,
    visible: true,
  });
  const history = useRef<PetBubbleHistory>({
    lastStatusAtMs: Number.NEGATIVE_INFINITY,
    protectedUntilMs: Number.NEGATIVE_INFINITY,
  });

  const presentBubble = useCallback((text: string, channel: PetBubbleChannel = "feedback") => {
    const nowMs = performance.now();
    const request = { text, channel } satisfies PetBubbleRequest;
    if (!shouldAcceptPetBubble(history.current, request, nowMs)) return false;
    if (channel === "status") history.current.lastStatusAtMs = nowMs;
    else history.current.protectedUntilMs = nowMs + PET_BUBBLE_DURATION_MS;
    setPresentation((current) => nextPetBubblePresentation(current, text));
    return true;
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => {
      setPresentation((current) => current.visible ? { ...current, visible: false } : current);
    }, PET_BUBBLE_DURATION_MS);
    return () => clearTimeout(timer);
  }, [presentation.revision]);

  return { ...presentation, presentBubble };
}
