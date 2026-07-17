import type {
  NimoraCommand,
  NimoraEvent,
  Pet,
  PointerButton,
  ProfilePolicy,
  ProfileSnapshot,
  SafetySnapshot,
  SpriteClips,
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

export interface OutboxSnapshot {
  pending: number;
  leased: number;
  delivered: number;
  deadLetter: number;
}

export interface BackupRecord {
  id: string;
  createdAtMs: number;
  bytes: number;
}

export interface BackupHealth {
  due: boolean;
  latest: BackupRecord | null;
  available: BackupRecord[];
  pendingRestore: string | null;
  lastError: string | null;
}

export interface InstallAssetRequest {
  sourcePath: string;
}

export interface ExportAssetRequest {
  sourcePath: string;
  destinationPath: string;
}

export interface InspectModelRequest {
  sourcePath: string;
}

export interface ImportModelRequest extends InspectModelRequest {
  assetId: string;
  name: string;
  license: string;
  animationMap: Record<string, ModelAnimationBinding>;
}

export interface ModelProbeReport {
  spec: "nimora.model-probe-report/1";
  format: "glb";
  formatVersion: "2.0";
  bytes: number;
  jsonBytes: number;
  binaryBytes: number;
  nodes: number;
  meshes: number;
  materials: number;
  textures: number;
  animations: number;
  animationNames: string[];
  skins: number;
}

export interface ModelAnimationBinding {
  animation: string;
  looped: boolean;
}

export interface ModelAnimationMap {
  spec: "nimora.animation-map/1";
  clips: Record<string, ModelAnimationBinding>;
}

export interface InstallPackageFile {
  relativePath: string;
  bytes: number;
  sha256: string;
}

export interface AssetInstallReceipt {
  assetId: string;
  replacedPrevious: boolean;
}

export interface AssetRollbackReceipt {
  assetId: string;
  quarantinedFailedVersion: boolean;
}

export interface AssetPackageSummary {
  id: string;
  assetType: "character" | "skin" | "theme" | "behavior" | "voice" | "interaction" | "bundle";
  version: string;
  name: Record<string, string>;
  publisher: string;
  license: string;
  rendererBackend: "sprite-sequence" | "sprite-atlas" | "live2d" | "vrm" | "gltf" | null;
  fileCount: number;
  totalBytes: number;
}

export interface AssetPreviewImage {
  mediaType: "image/png" | "image/webp";
  bytes: number[];
  width: number;
  height: number;
}

export interface AssetPreviewReport {
  summary: AssetPackageSummary;
  poster: AssetPreviewImage | null;
}

export interface AssetCatalogSnapshot {
  assets: AssetPackageSummary[];
  rejected: Array<{ directory: string; reason: string }>;
}

export interface ActiveCharacterSnapshot {
  assetId: string;
  source: "built-in" | "installed";
  fallbackReason: string | null;
}

export interface CharacterRendererSnapshot {
  spec: "nimora.renderer/1";
  assetId: string;
  assetBaseUrl: string | null;
  backend: "built-in" | "sprite-sequence" | "sprite-atlas" | "gltf";
  canvas: { width: number; height: number };
  anchor: { x: number; y: number };
  defaultScale: number;
  pixelArt: boolean;
  fallbacks: Record<string, string>;
  clips: SpriteClips | null;
  model: string | null;
  animationMap: ModelAnimationMap | null;
  fallbackReason: string | null;
}

export type UserCodeCapability =
  | "read-pet-state"
  | "read-profile-state"
  | "subscribe-events"
  | "invoke-safe-commands"
  | "store-local-data";

export interface UserProgramManifest {
  id: string;
  version: string;
  capabilities: readonly UserCodeCapability[];
  subscriptions: readonly string[];
  eventConcurrency: "serial" | "drop" | "cancel-previous";
  eventQueueCapacity: number;
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
  files: InstallPackageFile[];
}

export interface UserProgramInstallReceipt {
  programId: string;
  version: string;
  replacedPrevious: boolean;
}

export interface UserProgramRollbackReceipt {
  programId: string;
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
  | { type: "readProfileState" }
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
  | { type: "profileState"; value: ProfileSnapshot }
  | { type: "localData"; value: unknown | null }
  | { type: "localDataWritten" }
  | { type: "localDataDeleted"; deleted: boolean }
  | { type: "commandAccepted"; value: NimoraCommand };

export interface DesktopApi {
  readonly native: boolean;
  snapshot(): Promise<DesktopSnapshot>;
  drainEvents(): Promise<NimoraEvent[]>;
  outboxSnapshot(): Promise<OutboxSnapshot>;
  backupHealth(): Promise<BackupHealth>;
  createBackup(): Promise<BackupRecord | null>;
  requestDatabaseRestore(backupId: string): Promise<void>;
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
  assetCatalog(): Promise<AssetCatalogSnapshot>;
  activeCharacter(): Promise<ActiveCharacterSnapshot>;
  activeCharacterRenderer(): Promise<CharacterRendererSnapshot>;
  activateCharacter(assetId: string): Promise<ActiveCharacterSnapshot>;
  previewAsset(request: InstallAssetRequest): Promise<AssetPreviewReport | null>;
  exportAsset(request: ExportAssetRequest): Promise<AssetPackageSummary | null>;
  inspectModel(request: InspectModelRequest): Promise<ModelProbeReport | null>;
  importModel(request: ImportModelRequest): Promise<AssetInstallReceipt | null>;
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
      mode: "companion",
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
      async outboxSnapshot() { return { pending: 0, leased: 0, delivered: 0, deadLetter: 0 }; },
      async backupHealth() { return { due: true, latest: null, available: [], pendingRestore: null, lastError: null }; },
      async createBackup() { return null; },
      async requestDatabaseRestore() {},
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
      async assetCatalog() { return { assets: [], rejected: [] }; },
      async activeCharacter() { return { assetId: "builtin.aster", source: "built-in", fallbackReason: null }; },
      async activeCharacterRenderer() {
        return {
          spec: "nimora.renderer/1",
          assetId: "builtin.aster",
          assetBaseUrl: null,
          backend: "built-in",
          canvas: { width: 320, height: 360 },
          anchor: { x: 0.5, y: 1 },
          defaultScale: 1,
          pixelArt: false,
          fallbacks: {},
          clips: null,
          model: null,
          animationMap: null,
          fallbackReason: null,
        };
      },
      async activateCharacter(assetId) { return { assetId, source: assetId === "builtin.aster" ? "built-in" : "installed", fallbackReason: null }; },
      async previewAsset() { return null; },
      async exportAsset() { return null; },
      async inspectModel() { return null; },
      async importModel() { return null; },
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
    outboxSnapshot: async () => await invokeCommand("outbox_snapshot") as OutboxSnapshot,
    backupHealth: async () => await invokeCommand("backup_health") as BackupHealth,
    createBackup: async () => await invokeCommand("create_backup") as BackupRecord,
    requestDatabaseRestore: async (backupId) => { await invokeCommand("request_database_restore", { backupId }); },
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
    assetCatalog: async () => await invokeCommand("asset_catalog") as AssetCatalogSnapshot,
    activeCharacter: async () => await invokeCommand("active_character") as ActiveCharacterSnapshot,
    activeCharacterRenderer: async () => await invokeCommand("active_character_renderer") as CharacterRendererSnapshot,
    activateCharacter: async (assetId) => await invokeCommand("activate_character", { assetId }) as ActiveCharacterSnapshot,
    previewAsset: async (request) => await invokeCommand("preview_asset", { request }) as AssetPreviewReport,
    exportAsset: async (request) => await invokeCommand("export_asset", { request }) as AssetPackageSummary,
    inspectModel: async (request) => await invokeCommand("inspect_model", { request }) as ModelProbeReport,
    importModel: async (request) => await invokeCommand("import_model", { request }) as AssetInstallReceipt,
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
