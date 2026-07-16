import type { AsterCommand, AsterEvent, Pet } from "@asterpet/schemas";
import { invoke } from "@tauri-apps/api/core";

export const petActions = ["idle", "walk", "sleep", "work", "celebrate"] as const;
export type PetAction = (typeof petActions)[number];

export interface DesktopSnapshot {
  pet: Pet;
  clickThrough: boolean;
}

export interface DesktopApi {
  readonly native: boolean;
  snapshot(): Promise<DesktopSnapshot>;
  drainEvents(): Promise<AsterEvent[]>;
  movePet(x: number, y: number): Promise<AsterCommand | null>;
  playAction(action: PetAction): Promise<AsterCommand | null>;
  setClickThrough(enabled: boolean): Promise<void>;
}

type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const previewSnapshot: DesktopSnapshot = {
  pet: {
    id: "00000000-0000-4000-8000-000000000001",
    name: "Aster",
    state: "idle",
    emotion: "neutral",
    position: { x: 80, y: 80 },
    energy: 86,
    mood: 82,
    affinity: 34,
  },
  clickThrough: false,
};

export function isNativeDesktop(scope?: Window): boolean {
  const browserWindow = scope ?? (typeof window === "undefined" ? undefined : window);
  return browserWindow !== undefined && "__TAURI_INTERNALS__" in browserWindow;
}

export function createDesktopApi(native: boolean, invokeCommand: Invoke = invoke): DesktopApi {
  if (!native) {
    return {
      native: false,
      async snapshot() { return structuredClone(previewSnapshot); },
      async drainEvents() { return []; },
      async movePet() { return null; },
      async playAction() { return null; },
      async setClickThrough() {},
    };
  }

  return {
    native: true,
    snapshot: async () => await invokeCommand("desktop_snapshot") as DesktopSnapshot,
    drainEvents: async () => await invokeCommand("drain_runtime_events") as AsterEvent[],
    movePet: async (x, y) => await invokeCommand("move_pet", { request: { x, y } }) as AsterCommand,
    playAction: async (action) => await invokeCommand("play_pet_action", { action }) as AsterCommand,
    setClickThrough: async (enabled) => { await invokeCommand("set_click_through", { enabled }); },
  };
}

export const desktopApi = createDesktopApi(isNativeDesktop());
