import { useCallback, useEffect, useRef, useState } from "react";

export const PET_BUBBLE_DURATION_MS = 4200;
export const PET_STATUS_COOLDOWN_MS = 8000;
export const PET_BUBBLE_MAX_CHARACTERS = 42;

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
  channel: PetBubbleChannel;
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

export function nextPetBubblePresentation(current: PetBubblePresentation, message: string, channel: PetBubbleChannel): PetBubblePresentation {
  return { message: normalizePetBubbleText(message), channel, revision: current.revision + 1, visible: true };
}

export function shouldHidePetBubbleForPolicy(presentation: PetBubblePresentation, statusEnabled: boolean): boolean {
  return presentation.visible && presentation.channel === "status" && !statusEnabled;
}

export function normalizePetBubbleText(message: string): string {
  const normalized = message.trim().replace(/\s+/gu, " ");
  const characters = [...normalized];
  if (characters.length <= PET_BUBBLE_MAX_CHARACTERS) return normalized;
  return `${characters.slice(0, PET_BUBBLE_MAX_CHARACTERS - 1).join("")}…`;
}

export function usePetBubble(initialMessage: string, statusEnabled = true) {
  const [presentation, setPresentation] = useState<PetBubblePresentation>({
    message: initialMessage,
    channel: "status",
    revision: 0,
    visible: true,
  });
  const history = useRef<PetBubbleHistory>({
    lastStatusAtMs: Number.NEGATIVE_INFINITY,
    protectedUntilMs: Number.NEGATIVE_INFINITY,
  });
  const statusEnabledRef = useRef(statusEnabled);
  statusEnabledRef.current = statusEnabled;

  const presentBubble = useCallback((text: string, channel: PetBubbleChannel = "feedback") => {
    const nowMs = performance.now();
    const request = { text, channel } satisfies PetBubbleRequest;
    if (channel === "status" && !statusEnabledRef.current) return false;
    if (!shouldAcceptPetBubble(history.current, request, nowMs)) return false;
    if (channel === "status") history.current.lastStatusAtMs = nowMs;
    else history.current.protectedUntilMs = nowMs + PET_BUBBLE_DURATION_MS;
    setPresentation((current) => nextPetBubblePresentation(current, text, channel));
    return true;
  }, []);

  useEffect(() => {
    setPresentation((current) => shouldHidePetBubbleForPolicy(current, statusEnabled)
      ? { ...current, visible: false }
      : current);
  }, [statusEnabled]);

  useEffect(() => {
    const timer = setTimeout(() => {
      setPresentation((current) => current.visible ? { ...current, visible: false } : current);
    }, PET_BUBBLE_DURATION_MS);
    return () => clearTimeout(timer);
  }, [presentation.revision]);

  return { ...presentation, presentBubble };
}
