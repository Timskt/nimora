import type {
  NimoraCommand,
  NimoraEvent,
  Pet,
  PointerButton,
  ProfilePolicy,
  ProfileSnapshot,
  SafetySnapshot,
} from "@nimora/schemas";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const petActions = ["idle", "walk", "sleep", "work", "celebrate"] as const;
export type PetAction = (typeof petActions)[number];

export interface DesktopSnapshot {
  pet: Pet;
  windowPolicy: {
    alwaysOnTop: boolean;
    clickThrough: boolean;
  };
  safety: SafetySnapshot;
}

export interface InstallAssetRequest {
  assetId: string;
  sourcePath: string;
  files: Array<{ relativePath: string; bytes: number; sha256: string }>;
}

export interface AssetInstallReceipt {
  assetId: string;
  activePath: string;
  replacedPrevious: boolean;
}

export interface DesktopApi {
  readonly native: boolean;
  snapshot(): Promise<DesktopSnapshot>;
  drainEvents(): Promise<NimoraEvent[]>;
  profiles(): Promise<ProfileSnapshot>;
  createProfile(name: string, policy: ProfilePolicy): Promise<NimoraCommand | null>;
  switchProfile(profileId: string): Promise<NimoraCommand | null>;
  enterSafeMode(): Promise<NimoraCommand | null>;
  exitSafeMode(): Promise<NimoraCommand | null>;
  movePet(x: number, y: number): Promise<NimoraCommand | null>;
  playAction(action: PetAction): Promise<NimoraCommand | null>;
  clickPet(x: number, y: number, button: PointerButton): Promise<NimoraCommand | null>;
  dragPet(): Promise<NimoraCommand | null>;
  setClickThrough(enabled: boolean): Promise<void>;
  installAsset(request: InstallAssetRequest): Promise<AssetInstallReceipt | null>;
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
  windowPolicy: { alwaysOnTop: true, clickThrough: false },
  safety: { mode: "normal", reason: null },
};

const previewProfiles: ProfileSnapshot = {
  schemaVersion: 1,
  activeProfileId: "00000000-0000-4000-8000-000000000010",
  profiles: [{
    id: "00000000-0000-4000-8000-000000000010",
    name: "Default",
    policy: {
      alwaysOnTop: true,
      clickThrough: false,
      soundEnabled: true,
      proactiveFrequency: 25,
    },
  }],
};

export function isNativeDesktop(scope?: Window): boolean {
  const browserWindow = scope ?? (typeof window === "undefined" ? undefined : window);
  return browserWindow !== undefined && "__TAURI_INTERNALS__" in browserWindow;
}

export function createDesktopApi(
  native: boolean,
  invokeCommand: Invoke = invoke,
  startDragging: () => Promise<void> = async () => await getCurrentWindow().startDragging(),
): DesktopApi {
  if (!native) {
    return {
      native: false,
      async snapshot() { return structuredClone(previewSnapshot); },
      async drainEvents() { return []; },
      async profiles() { return structuredClone(previewProfiles); },
      async createProfile() { return null; },
      async switchProfile() { return null; },
      async enterSafeMode() { return null; },
      async exitSafeMode() { return null; },
      async movePet() { return null; },
      async playAction() { return null; },
      async clickPet() { return null; },
      async dragPet() { return null; },
      async setClickThrough() {},
      async installAsset() { return null; },
    };
  }

  return {
    native: true,
    snapshot: async () => await invokeCommand("desktop_snapshot") as DesktopSnapshot,
    drainEvents: async () => await invokeCommand("drain_runtime_events") as NimoraEvent[],
    profiles: async () => await invokeCommand("profile_snapshot") as ProfileSnapshot,
    createProfile: async (name, policy) => await invokeCommand("create_profile", { name, policy }) as NimoraCommand,
    switchProfile: async (profileId) => await invokeCommand("switch_profile", { profileId }) as NimoraCommand,
    enterSafeMode: async () => await invokeCommand("enter_safe_mode") as NimoraCommand,
    exitSafeMode: async () => await invokeCommand("exit_safe_mode") as NimoraCommand,
    movePet: async (x, y) => await invokeCommand("move_pet", { request: { x, y } }) as NimoraCommand,
    playAction: async (action) => await invokeCommand("play_pet_action", { action }) as NimoraCommand,
    clickPet: async (x, y, button) => await invokeCommand("click_pet", {
      request: { x, y, button },
    }) as NimoraCommand,
    dragPet: async () => {
      await invokeCommand("begin_pet_drag");
      try {
        await startDragging();
      } catch (error) {
        await invokeCommand("finish_pet_drag");
        throw error;
      }
      return await invokeCommand("finish_pet_drag") as NimoraCommand;
    },
    setClickThrough: async (enabled) => { await invokeCommand("set_click_through", { enabled }); },
    installAsset: async (request) => await invokeCommand("install_asset", { request }) as AssetInstallReceipt,
  };
}

export const desktopApi = createDesktopApi(isNativeDesktop());
