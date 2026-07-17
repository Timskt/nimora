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
  startup: {
    mode: "normal" | "recovery";
    reason: string | null;
  };
}

export interface OutboxSnapshot {
  pending: number;
  leased: number;
  delivered: number;
  deadLetter: number;
}

export interface AgentToolDescriptor {
  id: string;
  title: string;
  description: string;
  baseRisk: "safe" | "low" | "medium" | "high" | "critical";
  effect: "read_only" | "reversible_write" | "irreversible_write" | "external_side_effect";
}

export interface AgentCatalog {
  spec: "nimora.desktop-agent-catalog/1";
  providers: Array<{ id: string; name: string; locality: "local" | "network" }>;
  tools: AgentToolDescriptor[];
}

export interface AgentProviderStatus {
  spec: "nimora.desktop-agent-provider-status/1";
  providerId: string;
  state: "ready" | "unavailable";
  workerVerified: boolean;
  serviceReachable: boolean;
  models: Array<{ name: string; size: number; modifiedAt: string | null }>;
  message: string;
}

export interface LocalAgentResult {
  spec: "nimora.desktop-agent-result/1";
  status: "completed" | "waitingForConfirmation";
  task: { id: string; status: string; providerId: string };
  content: string | null;
  finishReason: string | null;
  usage: { inputTokens: number; outputTokens: number; costMicrounits: number } | null;
  pendingTools: AgentToolResult[];
}

export interface AgentHistoryRecord {
  spec: "nimora.agent-history/1";
  task: { id: string; createdAtMs: number; providerId: string; status: string };
  model: string;
  prompt: string;
  response: string;
  finishReason: string;
  usage: { inputTokens: number; outputTokens: number; costMicrounits: number };
  completedAtMs: number;
}

export interface AgentHistoryPage {
  spec: "nimora.desktop-agent-history/1";
  records: AgentHistoryRecord[];
  historyDegraded: boolean;
}

export interface AgentToolResult {
  spec: "nimora.desktop-agent-tool-result/1";
  task: { id: string; status: string; providerId: string };
  invocation: { invocationId: string; taskId: string; traceId: string; toolId: string; arguments: Record<string, unknown> };
  effectiveRisk: "safe" | "low" | "medium" | "high" | "critical";
  requiresConfirmation: boolean;
  expiresAtMs: number | null;
  output: unknown | null;
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

export interface DiagnosticReport {
  spec: "nimora.diagnostic-report/1";
  generatedAtMs: number;
  application: { name: string; version: string };
  system: { os: string; architecture: string };
  runtime: {
    startupMode: "normal" | "recovery";
    startupReason: string | null;
    safetyMode: "normal" | "safe";
    outboxPending: number;
    outboxDeadLetter: number;
  };
  dataProtection: {
    databaseSchema: number;
    backupCount: number;
    latestBackupAtMs: number | null;
    pendingRestore: boolean;
    lastBackupError: boolean;
  };
  sources: { eventCount: number; eventRetentionDays: number };
  privacy: {
    includesLogs: false;
    includesUserContent: false;
    includesSecrets: false;
    includesFilePaths: false;
    automaticallyUploaded: false;
  };
}

export interface DiagnosticBundleReceipt {
  spec: "nimora.diagnostic-bundle-receipt/1";
  bytes: number;
  sha256: string;
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
  | "invoke-agent-tasks"
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
  agentResults: LocalAgentResult[];
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

export interface AutomationDefinition {
  spec: "nimora.automation/1";
  id: string;
  name: string;
  enabled: boolean;
  trigger: { eventType: string };
  conditions: Array<{ pointer: string; equals: unknown }>;
  actions: Array<{
    id: string;
    command: string;
    arguments: Record<string, unknown>;
    risk: "safe" | "low" | "medium" | "high" | "critical";
    retrySafe: boolean;
    idempotencyKey: string | null;
    compensation: null | {
      command: string;
      arguments: Record<string, unknown>;
      risk: "safe" | "low" | "medium" | "high" | "critical";
    };
  }>;
  policy: { timeoutMs: number; failure: "stop" | "compensate" };
}

export interface AutomationRun {
  spec: "nimora.automation-run/1";
  runId: string;
  automationId: string;
  traceId: string;
  eventId: string;
  mode: "dry_run" | "live";
  status: "trigger_not_matched" | "condition_not_matched" | "planned" | "succeeded" | "failed" | "compensation_failed" | "cancelled" | "timed_out";
  steps: Array<{
    actionId: string;
    command: string;
    status: string;
    attempts: number;
    compensated: boolean;
    error: string | null;
  }>;
  reason: string | null;
}

export interface AutomationJournalEntry {
  spec: "nimora.automation-journal-entry/1";
  runId: string;
  automationId: string;
  traceId: string;
  eventId: string;
  status: "running" | "completed" | "interrupted";
  startedAtMs: number;
  updatedAtMs: number;
  result: AutomationRun | null;
  interruptionReason: string | null;
}

export interface AutomationAgentJournalEntry {
  spec: "nimora.automation-agent-journal/1";
  runId: string;
  idempotencyKey: string;
  admission: unknown;
  model: string;
  status: "submitted" | "waiting_for_confirmation" | "completed" | "failed" | "cancelled" | "interrupted";
  submittedAtMs: number;
  updatedAtMs: number;
  error: string | null;
}

export interface DesktopApi {
  readonly native: boolean;
  snapshot(): Promise<DesktopSnapshot>;
  drainEvents(): Promise<NimoraEvent[]>;
  outboxSnapshot(): Promise<OutboxSnapshot>;
  testAutomation(definition: AutomationDefinition, eventType: string, eventData: unknown): Promise<AutomationRun>;
  runAutomation(definition: AutomationDefinition, eventType: string, eventData: unknown): Promise<AutomationRun>;
  automationRunStatus(runId: string): Promise<AutomationJournalEntry | null>;
  automationAgentTaskStatus(taskId: string): Promise<AutomationAgentJournalEntry | null>;
  automationRunAgentTasks(runId: string): Promise<AutomationAgentJournalEntry[]>;
  cancelAutomationRun(runId: string): Promise<boolean>;
  cancelAgentTask(taskId: string): Promise<boolean>;
  agentCatalog(): Promise<AgentCatalog>;
  agentProviderStatus(providerId: string): Promise<AgentProviderStatus>;
  agentHistory(limit?: number, before?: { createdAtMs: number; taskId: string }): Promise<AgentHistoryPage>;
  deleteAgentHistory(taskId?: string): Promise<number>;
  runLocalAgent(prompt: string, providerId?: string, model?: string): Promise<LocalAgentResult>;
  prepareAgentTool(toolId: string, argumentsValue: Record<string, unknown>): Promise<AgentToolResult>;
  confirmAgentTool(invocationId: string): Promise<AgentToolResult>;
  confirmAgentRunTool(invocationId: string): Promise<LocalAgentResult>;
  rejectAgentTool(invocationId: string): Promise<void>;
  backupHealth(): Promise<BackupHealth>;
  createBackup(): Promise<BackupRecord | null>;
  requestDatabaseRestore(backupId: string): Promise<void>;
  previewDiagnosticReport(): Promise<DiagnosticReport>;
  exportDiagnostics(destinationPath: string, includeEvents: boolean): Promise<DiagnosticBundleReceipt | null>;
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

const previewRecoveryMode = typeof window !== "undefined"
  && new URLSearchParams(window.location.search).get("preview") === "recovery";

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
  startup: previewRecoveryMode
    ? { mode: "recovery", reason: "database-unavailable" }
    : { mode: "normal", reason: null },
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
    let previewAgentPendingTools: AgentToolResult[] = [];
    let previewAgentTask = { id: crypto.randomUUID(), status: "succeeded", providerId: "provider:deterministic-local" };
    let previewAgentPrompt = "";
    let previewAgentModel = "model:echo-v1";
    let previewAgentHistory: AgentHistoryRecord[] = [];
    const recordPreviewAgentHistory = (
      task: { id: string; status: string; providerId: string },
      prompt: string,
      model: string,
      response: string,
      finishReason: string,
      usage: AgentHistoryRecord["usage"],
    ) => {
      const completedAtMs = Date.now();
      const record: AgentHistoryRecord = {
        spec: "nimora.agent-history/1",
        task: { ...task, createdAtMs: completedAtMs },
        model,
        prompt,
        response,
        finishReason,
        usage,
        completedAtMs,
      };
      previewAgentHistory = [record, ...previewAgentHistory.filter((existing) => existing.task.id !== task.id)];
      return record;
    };
    return {
      native: false,
      async snapshot() { return structuredClone(previewSnapshot); },
      async drainEvents() { return []; },
      async outboxSnapshot() { return { pending: 0, leased: 0, delivered: 0, deadLetter: 0 }; },
      async testAutomation(definition, eventType, eventData) {
        const matched = definition.enabled && definition.trigger.eventType === eventType;
        const conditionsMatched = definition.conditions.every(({ pointer, equals }) => {
          if (!pointer.startsWith("/")) return false;
          const value = pointer.slice(1).split("/").reduce<unknown>((current, segment) => {
            if (current && typeof current === "object") return (current as Record<string, unknown>)[segment.replaceAll("~1", "/").replaceAll("~0", "~")];
            return undefined;
          }, eventData);
          return JSON.stringify(value) === JSON.stringify(equals);
        });
        const status = !matched ? "trigger_not_matched" : !conditionsMatched ? "condition_not_matched" : "planned";
        return {
          spec: "nimora.automation-run/1",
          runId: crypto.randomUUID(),
          automationId: definition.id,
          traceId: crypto.randomUUID(),
          eventId: crypto.randomUUID(),
          mode: "dry_run",
          status,
          steps: status === "planned" ? definition.actions.map((action) => ({ actionId: action.id, command: action.command, status: "pending", attempts: 0, compensated: false, error: null })) : [],
          reason: status === "planned" ? null : "测试事件未通过触发器或条件",
        };
      },
      async runAutomation() { throw new Error("Live automation requires the Nimora desktop runtime."); },
      async automationRunStatus() { return null; },
      async automationAgentTaskStatus() { return null; },
      async automationRunAgentTasks() { return []; },
      async cancelAgentTask() { return false; },
      async cancelAutomationRun() { return false; },
      async agentCatalog() {
        return {
          spec: "nimora.desktop-agent-catalog/1",
          providers: [
            { id: "provider:deterministic-local", name: "Deterministic local diagnostic", locality: "local" },
            { id: "provider:preview-scripted", name: "Preview scripted tool provider", locality: "local" },
          ],
          tools: [
            { id: "asset.catalog.read", title: "Read asset catalog", description: "Reads installed character assets and active selection.", baseRisk: "safe", effect: "read_only" },
            { id: "automation.definition.validate", title: "Validate automation definition", description: "Validates and dry-runs an automation definition without executing actions.", baseRisk: "safe", effect: "read_only" },
            { id: "character.active.switch", title: "Switch active character", description: "Switches to an installed character and refreshes the pet renderer.", baseRisk: "low", effect: "reversible_write" },
            { id: "character.state.read", title: "Read character state", description: "Reads the active character and path-free renderer capabilities.", baseRisk: "safe", effect: "read_only" },
            { id: "pet.action.catalog.read", title: "Read pet action catalog", description: "Reads the exact actions accepted by the pet runtime.", baseRisk: "safe", effect: "read_only" },
            { id: "pet.animation.play", title: "Play pet animation", description: "Plays one validated pet action through the Capability Gateway.", baseRisk: "low", effect: "reversible_write" },
            { id: "pet.position.move", title: "Move pet", description: "Moves the pet through the Capability Gateway.", baseRisk: "low", effect: "reversible_write" },
            { id: "pet.state.read", title: "Read pet state", description: "Reads current pet state.", baseRisk: "safe", effect: "read_only" },
            { id: "profile.active.switch", title: "Switch active profile", description: "Switches Profile and applies its native window policy.", baseRisk: "low", effect: "reversible_write" },
            { id: "profile.state.read", title: "Read profile state", description: "Reads active profile state.", baseRisk: "safe", effect: "read_only" },
            { id: "program.catalog.read", title: "Read program catalog", description: "Reads verified installed program identities and permission summaries without source or host paths.", baseRisk: "safe", effect: "read_only" },
            { id: "program.installed.execute", title: "Execute installed program", description: "Executes one exact installed program version through its isolated Worker and the Capability Gateway.", baseRisk: "medium", effect: "external_side_effect" },
            { id: "runtime.health.read", title: "Read runtime health", description: "Reads safety, startup, event delivery, and backup health.", baseRisk: "safe", effect: "read_only" },
          ],
        } as AgentCatalog;
      },
      async agentHistory(limit = 20, before) {
        const records = before
          ? previewAgentHistory.filter((record) => record.task.createdAtMs < before.createdAtMs
            || (record.task.createdAtMs === before.createdAtMs && record.task.id < before.taskId))
          : previewAgentHistory;
        return { spec: "nimora.desktop-agent-history/1", records: structuredClone(records.slice(0, limit)), historyDegraded: false };
      },
      async deleteAgentHistory(taskId) {
        const previousCount = previewAgentHistory.length;
        previewAgentHistory = taskId
          ? previewAgentHistory.filter((record) => record.task.id !== taskId)
          : [];
        return previousCount - previewAgentHistory.length;
      },
      async agentProviderStatus(providerId) {
        const scripted = providerId === "provider:preview-scripted";
        return {
          spec: "nimora.desktop-agent-provider-status/1",
          providerId,
          state: "ready",
          workerVerified: true,
          serviceReachable: true,
          models: [{ name: scripted ? "qwen3:8b" : "model:echo-v1", size: 0, modifiedAt: null }],
          message: scripted ? "预览脚本 Provider 可用" : "内置离线 Provider 可用",
        };
      },
      async runLocalAgent(prompt, providerId = "provider:deterministic-local", model = "model:echo-v1") {
        if (prompt.includes("移动") || prompt.includes("工具确认")) {
          previewAgentTask = { id: crypto.randomUUID(), status: "waiting_for_confirmation", providerId: "provider:preview-scripted" };
          previewAgentPrompt = prompt;
          previewAgentModel = model;
          const expiresAtMs = Date.now() + 300_000;
          previewAgentPendingTools = [
            { spec: "nimora.desktop-agent-tool-result/1", task: previewAgentTask, invocation: { invocationId: crypto.randomUUID(), taskId: previewAgentTask.id, traceId: crypto.randomUUID(), toolId: "pet.animation.play", arguments: { action: "celebrate" } }, effectiveRisk: "low", requiresConfirmation: true, expiresAtMs, output: null },
            { spec: "nimora.desktop-agent-tool-result/1", task: previewAgentTask, invocation: { invocationId: crypto.randomUUID(), taskId: previewAgentTask.id, traceId: crypto.randomUUID(), toolId: "pet.position.move", arguments: { x: 240, y: 160 } }, effectiveRisk: "low", requiresConfirmation: true, expiresAtMs, output: null },
          ];
          return { spec: "nimora.desktop-agent-result/1", status: "waitingForConfirmation", task: previewAgentTask, content: null, finishReason: null, usage: null, pendingTools: structuredClone(previewAgentPendingTools) };
        }
        previewAgentPendingTools = [];
        const task = { id: crypto.randomUUID(), status: "succeeded", providerId };
        const content = `[${model}] ${prompt}`;
        const usage = { inputTokens: Math.max(1, Math.ceil(prompt.length / 4)), outputTokens: Math.max(1, Math.ceil(prompt.length / 4)), costMicrounits: 0 };
        const historyRecord = recordPreviewAgentHistory(task, prompt, model, content, "stop", usage);
        return { spec: "nimora.desktop-agent-result/1", status: "completed", task: historyRecord.task, content, finishReason: "stop", usage, pendingTools: [] };
      },
      async prepareAgentTool(toolId, argumentsValue) {
        const invocationId = crypto.randomUUID();
        const requiresConfirmation = toolId === "pet.animation.play" || toolId === "pet.position.move";
        return { spec: "nimora.desktop-agent-tool-result/1", task: { id: crypto.randomUUID(), status: requiresConfirmation ? "waiting_for_confirmation" : "succeeded", providerId: "provider:deterministic-local" }, invocation: { invocationId, taskId: crypto.randomUUID(), traceId: crypto.randomUUID(), toolId, arguments: argumentsValue }, effectiveRisk: requiresConfirmation ? "low" : "safe", requiresConfirmation, expiresAtMs: requiresConfirmation ? Date.now() + 300_000 : null, output: requiresConfirmation ? null : { preview: true } };
      },
      async confirmAgentTool(invocationId) {
        return { spec: "nimora.desktop-agent-tool-result/1", task: { id: crypto.randomUUID(), status: "succeeded", providerId: "provider:deterministic-local" }, invocation: { invocationId, taskId: crypto.randomUUID(), traceId: crypto.randomUUID(), toolId: "pet.animation.play", arguments: { action: "celebrate" } }, effectiveRisk: "low", requiresConfirmation: false, expiresAtMs: null, output: { preview: true } };
      },
      async confirmAgentRunTool(invocationId) {
        previewAgentPendingTools = previewAgentPendingTools.filter((tool) => tool.invocation.invocationId !== invocationId);
        if (previewAgentPendingTools.length > 0) {
          return { spec: "nimora.desktop-agent-result/1", status: "waitingForConfirmation", task: previewAgentTask, content: null, finishReason: null, usage: null, pendingTools: structuredClone(previewAgentPendingTools) };
        }
        previewAgentTask = { ...previewAgentTask, status: "succeeded" };
        const content = "模块操作已经安全完成。";
        const usage = { inputTokens: 12, outputTokens: 8, costMicrounits: 0 };
        const historyRecord = recordPreviewAgentHistory(previewAgentTask, previewAgentPrompt, previewAgentModel, content, "completed", usage);
        return { spec: "nimora.desktop-agent-result/1", status: "completed", task: structuredClone(historyRecord.task), content, finishReason: "completed", usage, pendingTools: [] };
      },
      async rejectAgentTool() { previewAgentPendingTools = []; },
      async backupHealth() {
        const previewBackup = { id: "runtime-1784294125392.sqlite3", createdAtMs: 1_784_294_125_392, bytes: 2_621_440 };
        return previewRecoveryMode
          ? { due: false, latest: previewBackup, available: [previewBackup], pendingRestore: null, lastError: null }
          : { due: true, latest: null, available: [], pendingRestore: null, lastError: null };
      },
      async createBackup() { return null; },
      async requestDatabaseRestore() {},
      async previewDiagnosticReport() {
        return {
          spec: "nimora.diagnostic-report/1",
          generatedAtMs: Date.now(),
          application: { name: "Nimora", version: "0.1.0" },
          system: { os: "browser-preview", architecture: "web" },
          runtime: { startupMode: previewRecoveryMode ? "recovery" : "normal", startupReason: previewRecoveryMode ? "database-unavailable" : null, safetyMode: "normal", outboxPending: 0, outboxDeadLetter: 0 },
          dataProtection: { databaseSchema: 1, backupCount: previewRecoveryMode ? 1 : 0, latestBackupAtMs: previewRecoveryMode ? 1_784_294_125_392 : null, pendingRestore: false, lastBackupError: false },
          sources: { eventCount: previewRecoveryMode ? 1 : 2, eventRetentionDays: 14 },
          privacy: { includesLogs: false, includesUserContent: false, includesSecrets: false, includesFilePaths: false, automaticallyUploaded: false },
        };
      },
      async exportDiagnostics() { return null; },
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
    testAutomation: async (definition, eventType, eventData) => await invokeCommand("test_automation", { request: { definition, eventType, eventData } }) as AutomationRun,
    runAutomation: async (definition, eventType, eventData) => await invokeCommand("run_automation", { request: { definition, eventType, eventData } }) as AutomationRun,
    automationRunStatus: async (runId) => await invokeCommand("automation_run_status", { runId }) as AutomationJournalEntry | null,
    automationAgentTaskStatus: async (taskId) => await invokeCommand("automation_agent_task_status", { taskId }) as AutomationAgentJournalEntry | null,
    automationRunAgentTasks: async (runId) => await invokeCommand("automation_run_agent_tasks", { runId }) as AutomationAgentJournalEntry[],
    cancelAutomationRun: async (runId) => await invokeCommand("cancel_automation_run", { runId }) as boolean,
    cancelAgentTask: async (taskId) => await invokeCommand("cancel_agent_task", { taskId }) as boolean,
    agentCatalog: async () => await invokeCommand("agent_catalog") as AgentCatalog,
    agentProviderStatus: async (providerId) => await invokeCommand("agent_provider_status", { request: { providerId } }) as AgentProviderStatus,
    agentHistory: async (limit = 50, before) => await invokeCommand("agent_history_list", { request: { beforeCreatedAtMs: before?.createdAtMs ?? null, beforeTaskId: before?.taskId ?? null, limit } }) as AgentHistoryPage,
    deleteAgentHistory: async (taskId) => (await invokeCommand("delete_agent_history", { request: { taskId: taskId ?? null } }) as { deleted: number }).deleted,
    runLocalAgent: async (prompt, providerId = "provider:deterministic-local", model = "model:echo-v1") => await invokeCommand("run_local_agent", { request: { prompt, providerId, model } }) as LocalAgentResult,
    prepareAgentTool: async (toolId, argumentsValue) => await invokeCommand("prepare_agent_tool", { request: { toolId, arguments: argumentsValue } }) as AgentToolResult,
    confirmAgentTool: async (invocationId) => await invokeCommand("confirm_agent_tool", { request: { invocationId } }) as AgentToolResult,
    confirmAgentRunTool: async (invocationId) => await invokeCommand("confirm_agent_run_tool", { request: { invocationId } }) as LocalAgentResult,
    rejectAgentTool: async (invocationId) => { await invokeCommand("reject_agent_tool", { request: { invocationId } }); },
    backupHealth: async () => await invokeCommand("backup_health") as BackupHealth,
    createBackup: async () => await invokeCommand("create_backup") as BackupRecord,
    requestDatabaseRestore: async (backupId) => { await invokeCommand("request_database_restore", { backupId }); },
    previewDiagnosticReport: async () => await invokeCommand("preview_diagnostic_report") as DiagnosticReport,
    exportDiagnostics: async (destinationPath, includeEvents) => await invokeCommand("export_diagnostics", { request: { destinationPath, includeEvents } }) as DiagnosticBundleReceipt,
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
