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

export interface AssetRollbackReceipt {
  assetId: string;
  activePath: string;
  quarantinedFailedVersion: boolean;
}

export type UserCodeCapability =
  | "read-pet-state"
  | "subscribe-events"
  | "invoke-safe-commands"
  | "store-local-data";

export interface UserProgramManifest {
  id: string;
  version: string;
  capabilities: readonly UserCodeCapability[];
  subscriptions: readonly string[];
  eventConcurrency?: "serial" | "drop" | "cancel-previous";
  eventQueueCapacity?: number;
  commands: readonly string[];
  timeoutMs: number;
  memoryBytes: number;
}

export interface ProgramPolicyReport {
  programId: string;
  grantedCapabilities: UserCodeCapability[];
  timeoutMs: number;
  memoryBytes: number;
}

export interface UserProgramSessionReceipt {
  executionId: string;
  programId: string;
  timeoutMs: number;
  memoryBytes: number;
}

export interface UserProgramExecutionReceipt {
  executionId: string;
  responses: UserProgramCapabilityResponse[];
}

export interface InstallUserProgramRequest {
  sourcePath: string;
  manifest: UserProgramManifest;
  files: InstallAssetRequest["files"];
}

export interface UserProgramInstallReceipt {
  programId: string;
  version: string;
  activePath: string;
  replacedPrevious: boolean;
}

export interface UserProgramRollbackReceipt {
  programId: string;
  activePath: string;
  quarantinedFailedVersion: boolean;
}

export interface UserProgramPermissionStatus {
  programId: string;
  version: string;
  capabilities: UserCodeCapability[];
  granted: boolean;
}

export interface UserProgramEventSessionReceipt {
  subscriptionId: string;
  programId: string;
  version: string;
  eventTypes: string[];
  queueCapacity: number;
}

export interface UserProgramEventBatch {
  events: NimoraEvent[];
  dropped: number;
}

export interface UserProgramEventExecutionReceipt {
  execution: UserProgramExecutionReceipt | null;
  dropped: number;
}

export interface UserProgramEventSessionStatus {
  subscriptionId: string;
  programId: string;
  automatic: boolean;
  executed: number;
  dropped: number;
  lastError: string | null;
}

export type UserProgramCapabilityRequest =
  | { type: "readPetState" }
  | { type: "readLocalData"; key: string }
  | { type: "writeLocalData"; key: string; value: unknown }
  | { type: "deleteLocalData"; key: string }
  | { type: "invokeCommand"; command: string; arguments: unknown };

export interface UserProgramGatewayEnvelope {
  executionId: string;
  traceId: string;
  idempotencyKey?: string;
  request: UserProgramCapabilityRequest;
}

export type UserProgramCapabilityResponse =
  | { type: "petState"; value: unknown }
  | { type: "localData"; value: unknown | null }
  | { type: "localDataWritten" }
  | { type: "localDataDeleted"; deleted: boolean }
  | { type: "commandAccepted"; value: NimoraCommand };

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
  rollbackAsset(assetId: string): Promise<AssetRollbackReceipt | null>;
  validateUserProgram(manifest: UserProgramManifest): Promise<ProgramPolicyReport | null>;
  installUserProgram(request: InstallUserProgramRequest): Promise<UserProgramInstallReceipt | null>;
  rollbackUserProgram(programId: string): Promise<UserProgramRollbackReceipt | null>;
  userProgramPermissionStatus(programId: string): Promise<UserProgramPermissionStatus | null>;
  grantUserProgramPermissions(programId: string): Promise<UserProgramPermissionStatus | null>;
  revokeUserProgramPermissions(programId: string): Promise<void>;
  openUserProgramEventSession(programId: string): Promise<UserProgramEventSessionReceipt | null>;
  drainUserProgramEvents(subscriptionId: string): Promise<UserProgramEventBatch>;
  executeNextUserProgramEvent(subscriptionId: string): Promise<UserProgramEventExecutionReceipt | null>;
  startUserProgramEventLoop(subscriptionId: string): Promise<void>;
  userProgramEventSessionStatus(subscriptionId: string): Promise<UserProgramEventSessionStatus | null>;
  closeUserProgramEventSession(subscriptionId: string): Promise<void>;
  startUserProgram(manifest: UserProgramManifest): Promise<UserProgramSessionReceipt | null>;
  executeUserProgram(manifest: UserProgramManifest, source: string): Promise<UserProgramExecutionReceipt | null>;
  executeInstalledUserProgram(programId: string): Promise<UserProgramExecutionReceipt | null>;
  invokeUserProgramCapability(envelope: UserProgramGatewayEnvelope): Promise<UserProgramCapabilityResponse | null>;
  stopUserProgram(executionId: string): Promise<void>;
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
      async rollbackAsset() { return null; },
      async validateUserProgram() { return null; },
      async installUserProgram() { return null; },
      async rollbackUserProgram() { return null; },
      async userProgramPermissionStatus() { return null; },
      async grantUserProgramPermissions() { return null; },
      async revokeUserProgramPermissions() {},
      async openUserProgramEventSession() { return null; },
      async drainUserProgramEvents() { return { events: [], dropped: 0 }; },
      async executeNextUserProgramEvent() { return { execution: null, dropped: 0 }; },
      async startUserProgramEventLoop() {},
      async userProgramEventSessionStatus() { return null; },
      async closeUserProgramEventSession() {},
      async startUserProgram() { return null; },
      async executeUserProgram() { return null; },
      async executeInstalledUserProgram() { return null; },
      async invokeUserProgramCapability() { return null; },
      async stopUserProgram() {},
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
    rollbackAsset: async (assetId) => await invokeCommand("rollback_asset", { assetId }) as AssetRollbackReceipt,
    validateUserProgram: async (manifest) => await invokeCommand("validate_user_program", { manifest }) as ProgramPolicyReport,
    installUserProgram: async (request) => await invokeCommand("install_user_program", { request }) as UserProgramInstallReceipt,
    rollbackUserProgram: async (programId) => await invokeCommand("rollback_user_program", { programId }) as UserProgramRollbackReceipt,
    userProgramPermissionStatus: async (programId) => await invokeCommand("user_program_permission_status", { programId }) as UserProgramPermissionStatus,
    grantUserProgramPermissions: async (programId) => await invokeCommand("grant_user_program_permissions", { programId }) as UserProgramPermissionStatus,
    revokeUserProgramPermissions: async (programId) => { await invokeCommand("revoke_user_program_permissions", { programId }); },
    openUserProgramEventSession: async (programId) => await invokeCommand("open_user_program_event_session", { programId }) as UserProgramEventSessionReceipt,
    drainUserProgramEvents: async (subscriptionId) => await invokeCommand("drain_user_program_events", { subscriptionId }) as UserProgramEventBatch,
    executeNextUserProgramEvent: async (subscriptionId) => await invokeCommand("execute_next_user_program_event", { subscriptionId }) as UserProgramEventExecutionReceipt,
    startUserProgramEventLoop: async (subscriptionId) => { await invokeCommand("start_user_program_event_loop", { subscriptionId }); },
    userProgramEventSessionStatus: async (subscriptionId) => await invokeCommand("user_program_event_session_status", { subscriptionId }) as UserProgramEventSessionStatus,
    closeUserProgramEventSession: async (subscriptionId) => { await invokeCommand("close_user_program_event_session", { subscriptionId }); },
    startUserProgram: async (manifest) => await invokeCommand("start_user_program", { manifest }) as UserProgramSessionReceipt,
    executeUserProgram: async (manifest, source) => await invokeCommand("execute_user_program", { manifest, source }) as UserProgramExecutionReceipt,
    executeInstalledUserProgram: async (programId) => await invokeCommand("execute_installed_user_program", { programId }) as UserProgramExecutionReceipt,
    invokeUserProgramCapability: async (envelope) => await invokeCommand("invoke_user_program_capability", { envelope }) as UserProgramCapabilityResponse,
    stopUserProgram: async (executionId) => { await invokeCommand("stop_user_program", { executionId }); },
  };
}

export const desktopApi = createDesktopApi(isNativeDesktop());
