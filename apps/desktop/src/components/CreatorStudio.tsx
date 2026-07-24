import { useEffect, useMemo, useState, type CSSProperties } from "react";
import type { ActiveCharacterSnapshot, ActiveThemeSnapshot, ActiveVoiceSnapshot, AssetCatalogSnapshot, AssetPackageSummary, AssetPreviewReport, ModelAnimationBinding, ModelProbeReport, ThemeDescriptor } from "../platform/desktop";
import { desktopApi } from "../platform/desktop";

const backends = ["Sprite Atlas", "Live2D Cubism", "VRM", "glTF"] as const;
type Backend = (typeof backends)[number];

const checks = [
  { label: "Manifest 版本契约", detail: "nimora.asset/1", status: "通过" },
  { label: "标准动作回退", detail: "pet.idle · pet.click · pet.talk", status: "通过" },
  { label: "可访问性声明", detail: "动画、闪烁、音效", status: "待补充" },
  { label: "许可证与署名", detail: "未检测到 NOTICE.md", status: "待补充" },
] as const;

const modelActions = [
  { action: "pet.idle", label: "待机", looped: true, aliases: ["idle", "stand", "breath"] },
  { action: "pet.observe", label: "张望", looped: true, aliases: ["observe", "look", "curious"] },
  { action: "pet.walk", label: "行走", looped: true, aliases: ["walk", "locomotion"] },
  { action: "pet.play", label: "自主玩耍", looped: true, aliases: ["play", "toy", "frolic"] },
  { action: "pet.perch", label: "栖息", looped: true, aliases: ["perch", "sit", "ledge"] },
  { action: "pet.climb", label: "攀爬", looped: true, aliases: ["climb", "wall", "edge"] },
  { action: "pet.peek", label: "探头", looped: true, aliases: ["peek", "head", "top-edge"] },
  { action: "pet.stretch", label: "伸懒腰", looped: false, aliases: ["stretch", "yawn", "wake-up"] },
  { action: "pet.click", label: "互动", looped: false, aliases: ["click", "wave", "greet", "happy"] },
  { action: "pet.drag", label: "拖拽", looped: true, aliases: ["drag", "grab", "hold"] },
  { action: "pet.sleep", label: "睡眠", looped: true, aliases: ["sleep", "rest"] },
  { action: "pet.work", label: "工作", looped: true, aliases: ["work", "type", "study"] },
] as const;

export function suggestAnimationMap(names: readonly string[]): Record<string, ModelAnimationBinding> {
  const result: Record<string, ModelAnimationBinding> = {};
  for (const definition of modelActions) {
    const animation = names.find((name) => {
      const normalized = name.toLocaleLowerCase();
      return definition.aliases.some((alias) => normalized.includes(alias));
    });
    if (animation) result[definition.action] = { animation, looped: definition.looped };
  }
  return result;
}


/** Manifest capability IDs accepted by user-code policy (kebab-case). */
export type UserCodeManifestCapability =
  | "read-pet-state"
  | "read-profile-state"
  | "subscribe-events"
  | "invoke-safe-commands"
  | "store-local-data"
  | "invoke-agent-tasks";

/** First-party safe commands registered for pet control via Capability Gateway. */
export type UserCodeSafeCommand =
  | "safe.pet.animate"
  | "safe.pet.care"
  | "safe.pet.move"
  | "safe.pet.directive";

/** Gateway / module read-surface capability names (dot notation). */
export type UserCodeReadSurface =
  | "pet.state"
  | "pet.action.catalog"
  | "character.state"
  | "program.catalog"
  | "runtime.health";

export type UserCodeCapabilityChipTone = "read" | "act" | "agent" | "meta";

export type UserCodeCapabilityChipGroup = "read" | "command" | "agent" | "manifest";

export interface UserCodeCapabilityChip {
  id: string;
  labelZh: string;
  detailZh: string;
  group: UserCodeCapabilityChipGroup;
  tone: UserCodeCapabilityChipTone;
}

/** Chinese labels for Manifest capability IDs. */
export const USER_CODE_MANIFEST_CAPABILITY_LABELS: Record<UserCodeManifestCapability, string> = {
  "read-pet-state": "读取宠物状态",
  "read-profile-state": "读取 Profile",
  "subscribe-events": "订阅事件",
  "invoke-safe-commands": "调用安全命令",
  "store-local-data": "本地命名空间存储",
  "invoke-agent-tasks": "调用 Agent 任务",
};

/** Chinese labels for first-party pet safe commands. */
export const USER_CODE_SAFE_COMMAND_LABELS: Record<UserCodeSafeCommand, string> = {
  "safe.pet.animate": "播放动作",
  "safe.pet.care": "照料互动",
  "safe.pet.move": "移动位置",
  "safe.pet.directive": "结构化指令",
};

/** Chinese labels for gateway read surfaces. */
export const USER_CODE_READ_SURFACE_LABELS: Record<UserCodeReadSurface, string> = {
  "pet.state": "宠物状态",
  "pet.action.catalog": "动作目录",
  "character.state": "角色状态",
  "program.catalog": "程序目录",
  "runtime.health": "运行时健康",
};

const MANIFEST_DETAILS: Record<UserCodeManifestCapability, string> = {
  "read-pet-state": "注入只读 pet 快照（nimora.input）",
  "read-profile-state": "读取当前 Profile 策略快照",
  "subscribe-events": "有界事件订阅（Manifest 声明）",
  "invoke-safe-commands": "执行 Manifest commands 白名单",
  "store-local-data": "程序私有 KV，无原始 FS",
  "invoke-agent-tasks": "需授权；draft + 空 Tool allowlist",
};

const COMMAND_DETAILS: Record<UserCodeSafeCommand, string> = {
  "safe.pet.animate": "按动作 ID 播放 pet.* 动画",
  "safe.pet.care": "喂食 / 抚摸等照料动作",
  "safe.pet.move": "在桌面安全区域内移动",
  "safe.pet.directive": "nimora.pet_directive/1 结构化驱动",
};

const READ_DETAILS: Record<UserCodeReadSurface, string> = {
  "pet.state": "情绪、位置、状态等只读快照",
  "pet.action.catalog": "宿主动作与 directive 契约目录",
  "character.state": "当前角色 / 资源身份",
  "program.catalog": "已安装用户程序目录",
  "runtime.health": "启动、安全模式与备份健康",
};

/**
 * Compact capability matrix for Creator / AI Creator surfaces.
 * Pure helper — safe to unit-test without DOM.
 */
export function userCodeCapabilityChips(): UserCodeCapabilityChip[] {
  const reads: UserCodeCapabilityChip[] = (Object.keys(USER_CODE_READ_SURFACE_LABELS) as UserCodeReadSurface[]).map((id) => ({
    id,
    labelZh: USER_CODE_READ_SURFACE_LABELS[id],
    detailZh: READ_DETAILS[id],
    group: "read" as const,
    tone: "read" as const,
  }));
  const commands: UserCodeCapabilityChip[] = (Object.keys(USER_CODE_SAFE_COMMAND_LABELS) as UserCodeSafeCommand[]).map((id) => ({
    id,
    labelZh: USER_CODE_SAFE_COMMAND_LABELS[id],
    detailZh: COMMAND_DETAILS[id],
    group: "command" as const,
    tone: "act" as const,
  }));
  const agent: UserCodeCapabilityChip[] = [{
    id: "invoke-agent-tasks",
    labelZh: USER_CODE_MANIFEST_CAPABILITY_LABELS["invoke-agent-tasks"],
    detailZh: MANIFEST_DETAILS["invoke-agent-tasks"],
    group: "agent",
    tone: "agent",
  }];
  const manifest: UserCodeCapabilityChip[] = (Object.keys(USER_CODE_MANIFEST_CAPABILITY_LABELS) as UserCodeManifestCapability[])
    .filter((id) => id !== "invoke-agent-tasks")
    .map((id) => ({
      id,
      labelZh: USER_CODE_MANIFEST_CAPABILITY_LABELS[id],
      detailZh: MANIFEST_DETAILS[id],
      group: "manifest" as const,
      tone: "meta" as const,
    }));
  return [...reads, ...commands, ...agent, ...manifest];
}

/** Resolves a capability / command / read-surface id to its Chinese label. */
export function labelForUserCodeCapability(id: string): string | null {
  if (id in USER_CODE_MANIFEST_CAPABILITY_LABELS) {
    return USER_CODE_MANIFEST_CAPABILITY_LABELS[id as UserCodeManifestCapability];
  }
  if (id in USER_CODE_SAFE_COMMAND_LABELS) {
    return USER_CODE_SAFE_COMMAND_LABELS[id as UserCodeSafeCommand];
  }
  if (id in USER_CODE_READ_SURFACE_LABELS) {
    return USER_CODE_READ_SURFACE_LABELS[id as UserCodeReadSurface];
  }
  return null;
}

export interface UserProgramPlanSnippet {
  storage: unknown[];
  commands: Array<{
    command: string;
    arguments: Record<string, unknown>;
    idempotencyKey: string;
  }>;
  agentTasks: unknown[];
}

/**
 * Minimal Worker capability plan that drives the pet via `safe.pet.directive`
 * (`nimora.pet_directive/1`) with Chinese speech.
 */
export function samplePetDirectiveProgramPlan(): UserProgramPlanSnippet {
  return {
    storage: [],
    commands: [{
      command: "safe.pet.directive",
      arguments: {
        spec: "nimora.pet_directive/1",
        speech: "专注完成啦，休息一下吧！",
        action: "celebrate",
        animation: "pet.celebrate",
        attention: "user",
        moodDelta: { mood: 6 },
      },
      idempotencyKey: "focus-done-celebrate-1",
    }],
    agentTasks: [],
  };
}

/** Pretty-printed sample plan for UI / docs copy blocks. */
export function formatSampleProgramPlanJson(plan: UserProgramPlanSnippet = samplePetDirectiveProgramPlan()): string {
  return JSON.stringify(plan, null, 2);
}

const CHIP_TONE_STYLE: Record<UserCodeCapabilityChipTone, CSSProperties> = {
  read: { color: "#3d5f7a", background: "#eaf4fb", borderColor: "rgba(61,95,122,.18)" },
  act: { color: "#5344a0", background: "#f0ecff", borderColor: "rgba(83,68,160,.18)" },
  agent: { color: "#6d552e", background: "#fff4dc", borderColor: "rgba(109,85,46,.2)" },
  meta: { color: "#4a6b3d", background: "#eef6e8", borderColor: "rgba(74,107,61,.18)" },
};

const GROUP_TITLE_ZH: Record<UserCodeCapabilityChipGroup, string> = {
  read: "只读面 · Read surfaces",
  command: "安全命令 · Safe commands",
  agent: "Agent 任务 · Agent tasks",
  manifest: "Manifest 能力 · Capabilities",
};

/**
 * Compact Chinese capability matrix for Creator product surfaces.
 * Uses existing design tokens via inline styles (no styles.css change).
 */
export function UserCodeCapabilityMatrix({ compact = false }: { compact?: boolean }) {
  const chips = userCodeCapabilityChips();
  const groups: UserCodeCapabilityChipGroup[] = compact
    ? ["read", "command", "agent"]
    : ["read", "command", "agent", "manifest"];
  const sample = formatSampleProgramPlanJson();

  return (
    <section
      className="creator-capability-matrix"
      aria-labelledby="user-code-capability-heading"
      style={{
        display: "grid",
        gap: compact ? 12 : 14,
        padding: compact ? "14px 16px" : "16px 18px",
        border: "1px solid rgba(115,98,228,.14)",
        borderRadius: 16,
        background: "linear-gradient(160deg, rgba(245,241,251,.95), rgba(255,253,250,.98))",
      }}
    >
      <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 12, flexWrap: "wrap" }}>
        <div>
          <p className="card-label" style={{ margin: 0 }}>USER CODE · CAPABILITY MATRIX</p>
          <h3 id="user-code-capability-heading" style={{ margin: "4px 0 0", fontSize: compact ? 14 : 16 }}>
            用户程序如何驱动宠物与模块
          </h3>
        </div>
        <span
          style={{
            padding: "6px 10px",
            borderRadius: 999,
            color: "#5344a0",
            background: "#eeebff",
            fontSize: 9,
            fontWeight: 900,
            whiteSpace: "nowrap",
          }}
        >
          Gateway · fail-closed
        </span>
      </div>
      <p style={{ margin: 0, color: "var(--muted)", fontSize: 11, lineHeight: 1.65, maxWidth: 720 }}>
        用户代码经 Capability Gateway 访问只读面与 Manifest 白名单中的 <code>safe.*</code> 命令；
        用 <code>safe.pet.directive</code>（<code>nimora.pet_directive/1</code>）一次表达 speech · moodDelta · action · animation · attention。
        未声明能力一律拒绝。
      </p>
      {groups.map((group) => {
        const items = chips.filter((chip) => chip.group === group);
        return (
          <div key={group} style={{ display: "grid", gap: 8 }}>
            <strong style={{ fontSize: 10, letterSpacing: ".02em", color: "var(--muted)" }}>{GROUP_TITLE_ZH[group]}</strong>
            <div className="action-chips" style={{ flexWrap: "wrap", gap: 6 }} role="list">
              {items.map((chip) => (
                <span
                  key={chip.id}
                  role="listitem"
                  title={`${chip.id} · ${chip.detailZh}`}
                  style={{
                    display: "inline-flex",
                    alignItems: "center",
                    gap: 6,
                    padding: "7px 11px",
                    border: "1px solid",
                    borderRadius: 999,
                    fontSize: 10,
                    fontWeight: 750,
                    lineHeight: 1.2,
                    ...CHIP_TONE_STYLE[chip.tone],
                  }}
                >
                  <code style={{ fontSize: 9, opacity: .85 }}>{chip.id}</code>
                  <span>{chip.labelZh}</span>
                </span>
              ))}
            </div>
          </div>
        );
      })}
      <div className="creator-note" style={{ flexDirection: "column", gap: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <span aria-hidden="true">⌘</span>
          <p style={{ margin: 0, fontWeight: 750 }}>示例计划 · sample program plan（含中文 speech）</p>
        </div>
        <pre
          style={{
            margin: 0,
            padding: "10px 12px",
            borderRadius: 10,
            background: "rgba(255,255,255,.72)",
            border: "1px solid rgba(115,98,228,.1)",
            fontSize: 10,
            lineHeight: 1.55,
            overflow: "auto",
            maxHeight: compact ? 160 : 220,
          }}
        >
          <code>{sample}</code>
        </pre>
      </div>
    </section>
  );
}

export function CreatorStudio({ onThemeChange }: { onThemeChange(theme: ActiveThemeSnapshot): void }) {
  const [backend, setBackend] = useState<Backend>("Sprite Atlas");
  const [skinTone, setSkinTone] = useState("#d8d0f0");
  const [draftSaved, setDraftSaved] = useState(false);
  const [catalog, setCatalog] = useState<AssetCatalogSnapshot | null>(null);
  const [catalogError, setCatalogError] = useState(false);
  const [activeCharacter, setActiveCharacter] = useState<ActiveCharacterSnapshot | null>(null);
  const [activeTheme, setActiveTheme] = useState<ActiveThemeSnapshot | null>(null);
  const [activeVoice, setActiveVoice] = useState<ActiveVoiceSnapshot | null>(null);
  const [activationError, setActivationError] = useState<string | null>(null);
  const [activating, setActivating] = useState<string | null>(null);
  const [pendingImport, setPendingImport] = useState<{ sourcePath: string; report: AssetPreviewReport } | null>(null);
  const [previewPosterUrl, setPreviewPosterUrl] = useState<string | null>(null);
  const [previewAudioUrl, setPreviewAudioUrl] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [importNotice, setImportNotice] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  const [modelCandidate, setModelCandidate] = useState<{ sourcePath: string; report: ModelProbeReport } | null>(null);
  const [modelError, setModelError] = useState<string | null>(null);
  const [checkingModel, setCheckingModel] = useState(false);
  const [installingModel, setInstallingModel] = useState(false);
  const [modelAssetId, setModelAssetId] = useState("character.local.custom");
  const [modelName, setModelName] = useState("My GLB Character");
  const [modelLicense, setModelLicense] = useState("LicenseRef-Proprietary");
  const [modelAnimationMap, setModelAnimationMap] = useState<Record<string, ModelAnimationBinding>>({});
  const completion = useMemo(() => checks.filter((check) => check.status === "通过").length, []);

  useEffect(() => {
    void Promise.all([desktopApi.assetCatalog(), desktopApi.activeCharacter(), desktopApi.activeTheme(), desktopApi.activeVoice()])
      .then(([nextCatalog, nextActiveCharacter, nextActiveTheme, nextActiveVoice]) => {
        setCatalog(nextCatalog);
        setActiveCharacter(nextActiveCharacter);
        setActiveTheme(nextActiveTheme);
        setActiveVoice(nextActiveVoice);
      })
      .catch(() => setCatalogError(true));
  }, []);

  useEffect(() => {
    const poster = pendingImport?.report.poster;
    if (!poster) {
      setPreviewPosterUrl(null);
      return;
    }
    const url = URL.createObjectURL(new Blob([new Uint8Array(poster.bytes)], { type: poster.mediaType }));
    setPreviewPosterUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [pendingImport]);

  useEffect(() => {
    const preview = pendingImport?.report.voicePreview;
    if (!preview) {
      setPreviewAudioUrl(null);
      return;
    }
    const url = URL.createObjectURL(new Blob([new Uint8Array(preview.bytes)], { type: preview.mediaType }));
    setPreviewAudioUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [pendingImport]);

  async function activate(assetId: string) {
    setActivating(assetId);
    setActivationError(null);
    try {
      setActiveCharacter(await desktopApi.activateCharacter(assetId));
    } catch (error) {
      setActivationError(error instanceof Error ? error.message : "角色激活失败");
    } finally {
      setActivating(null);
    }
  }

  async function activateTheme(assetId: string) {
    setActivating(assetId);
    setActivationError(null);
    try {
      const nextTheme = await desktopApi.activateTheme(assetId);
      setActiveTheme(nextTheme);
      onThemeChange(nextTheme);
    } catch (error) {
      setActivationError(error instanceof Error ? error.message : "主题激活失败");
    } finally {
      setActivating(null);
    }
  }

  async function activateVoice(assetId: string) {
    setActivating(assetId);
    setActivationError(null);
    try {
      setActiveVoice(await desktopApi.activateVoice(assetId));
    } catch (error) {
      setActivationError(error instanceof Error ? error.message : "声音激活失败");
    } finally {
      setActivating(null);
    }
  }

  async function selectPackage() {
    setImportError(null);
    setImportNotice(null);
    setImporting(true);
    try {
      const selected = await desktopApi.pickFile({
        title: "选择 Nimora 资源包",
        name: "Nimora 资源包",
        extensions: ["nimora"],
      });
      if (typeof selected !== "string") return;
      const report = await desktopApi.previewAsset({ sourcePath: selected });
      if (!report) throw new Error("当前环境不支持资源包预览");
      setPendingImport({ sourcePath: selected, report });
    } catch (error) {
      setPendingImport(null);
      setImportError(error instanceof Error ? error.message : "资源包未通过安全检查");
    } finally {
      setImporting(false);
    }
  }

  async function confirmInstall() {
    if (!pendingImport) return;
    setImporting(true);
    setImportError(null);
    try {
      const receipt = await desktopApi.installAsset({ sourcePath: pendingImport.sourcePath });
      if (!receipt) throw new Error("当前环境不支持资源包安装");
      setCatalog(await desktopApi.assetCatalog());
      setPendingImport(null);
      setImportNotice(`${pendingImport.report.summary.id} 已完成复验并原子安装`);
    } catch (error) {
      setImportError(error instanceof Error ? error.message : "资源包安装失败");
    } finally {
      setImporting(false);
    }
  }

  async function exportPackage() {
    setExporting(true);
    setExportError(null);
    setExportNotice(null);
    try {
      const sourcePath = await desktopApi.pickDirectory("选择已展开的 Nimora 资源目录");
      if (typeof sourcePath !== "string") return;
      const preview = await desktopApi.previewAsset({ sourcePath });
      if (!preview) throw new Error("当前环境不支持资源包导出");
      const destinationPath = await desktopApi.saveFile({
        title: "导出 Nimora 资源包",
        defaultPath: `${preview.summary.id}-${preview.summary.version}.nimora`,
        name: "Nimora 资源包",
        extensions: ["nimora"],
      });
      if (typeof destinationPath !== "string") return;
      const exported = await desktopApi.exportAsset({ sourcePath, destinationPath });
      if (!exported) throw new Error("当前环境不支持资源包导出");
      setExportNotice(`${exported.id} 已复验并确定性打包`);
    } catch (error) {
      setExportError(error instanceof Error ? error.message : "资源包导出失败");
    } finally {
      setExporting(false);
    }
  }

  async function inspectModel() {
    setCheckingModel(true);
    setModelError(null);
    setModelCandidate(null);
    try {
      const sourcePath = await desktopApi.pickFile({
        title: "选择 GLB 2.0 / VRM 1.0 模型",
        name: "3D 角色模型",
        extensions: ["glb", "vrm"],
      });
      if (typeof sourcePath !== "string") return;
      const report = await desktopApi.inspectModel({ sourcePath });
      if (!report) throw new Error("当前环境不支持模型隔离检查");
      setModelCandidate({ sourcePath, report });
      setModelAnimationMap(suggestAnimationMap(report.animationNames));
    } catch (error) {
      setModelError(error instanceof Error ? error.message : "模型未通过隔离结构检查");
    } finally {
      setCheckingModel(false);
    }
  }

  async function installModel() {
    if (!modelCandidate) return;
    setInstallingModel(true);
    setModelError(null);
    try {
      const receipt = await desktopApi.importModel({
        sourcePath: modelCandidate.sourcePath,
        assetId: modelAssetId.trim(),
        name: modelName.trim(),
        license: modelLicense,
        animationMap: modelAnimationMap,
      });
      if (!receipt) throw new Error("当前环境不支持模型规范化安装");
      setCatalog(await desktopApi.assetCatalog());
      setImportNotice(`${receipt.assetId} 已规范化并原子安装`);
      setModelCandidate(null);
    } catch (error) {
      setModelError(error instanceof Error ? error.message : "模型规范化安装失败");
    } finally {
      setInstallingModel(false);
    }
  }

  function saveDraft() {
    setDraftSaved(true);
    window.setTimeout(() => setDraftSaved(false), 1800);
  }

  return (
    <section className="creator-studio" aria-labelledby="creator-heading">
      <div className="creator-header">
        <div>
          <p className="card-label">CREATOR STUDIO · LOCAL DRAFT</p>
          <h2 id="creator-heading">把角色变成你的风格</h2>
          <p>所有预览在本机完成。先调整外观与渲染后端，再导出可验证的资源包。</p>
        </div>
        <button className="secondary-button" type="button" onClick={saveDraft}>
          {draftSaved ? "草稿已保存" : "保存草稿"}
        </button>
      </div>

      <UserCodeCapabilityMatrix />

      <div className="creator-layout">
        <div className="creator-preview" aria-label={`${backend} 角色预览`}>
          <div className="preview-grid" aria-hidden="true" />
          <div className="creator-pet" style={{ background: `linear-gradient(145deg, #fffaf0 8%, ${skinTone} 100%)` }}>
            <span className="creator-ear left" />
            <span className="creator-ear right" />
            <span className="creator-star">✦</span>
            <span className="creator-eye left" />
            <span className="creator-eye right" />
            <span className="creator-mouth" />
          </div>
          <span className="preview-badge">{backend} · 预览</span>
        </div>

        <div className="creator-controls">
          <label className="creator-field">
            <span>渲染后端</span>
            <select value={backend} onChange={(event) => setBackend(event.target.value as Backend)}>
              {backends.map((option) => <option key={option}>{option}</option>)}
            </select>
          </label>
          <label className="creator-field">
            <span>主色调</span>
            <span className="color-control"><input type="color" value={skinTone} onChange={(event) => setSkinTone(event.target.value)} /><code>{skinTone}</code></span>
          </label>
          <div className="creator-field">
            <span>动作预览</span>
            <div className="action-chips"><button type="button">idle</button><button type="button">click</button><button type="button">talk</button></div>
          </div>
          <div className="creator-note"><span>⌘</span><p>导入 Live2D / VRM 后，Nimora 仍使用统一的 Pet Runtime 动作语义。</p></div>
        </div>
      </div>

      <section className="asset-import" aria-labelledby="asset-import-heading">
        <div className="section-heading">
          <div><p className="card-label">SAFE IMPORT</p><h3 id="asset-import-heading">验证后再安装</h3></div>
          <button className="secondary-button" type="button" disabled={!desktopApi.native || importing || exporting} onClick={() => void selectPackage()}>
            {importing && !pendingImport ? "正在验证…" : desktopApi.native ? "选择资源包" : "桌面版可用"}
          </button>
        </div>
        <p className="asset-import-intro">系统文件选择器只接受 .nimora 包；宿主限额展开并验证 Manifest、精确文件树、大小和 SHA-256，确认安装时再次完整复验。</p>
        {importError ? <p className="catalog-empty error" role="alert">{importError}</p> : null}
        {importNotice ? <p className="catalog-empty success" role="status">{importNotice}</p> : null}
        {pendingImport ? <div className="asset-preview-report">
          {previewPosterUrl ? <figure className="asset-preview-poster"><img src={previewPosterUrl} alt={`${assetDisplayName(pendingImport.report.summary)} 资源包预览海报`} /><figcaption>{pendingImport.report.poster?.width} × {pendingImport.report.poster?.height} · 已验证静态预览</figcaption></figure> : null}
          <div className="asset-preview-title"><span className="asset-kind">{pendingImport.report.summary.assetType.slice(0, 1).toUpperCase()}</span><div><strong>{assetDisplayName(pendingImport.report.summary)}</strong><p>{pendingImport.report.summary.id} · {pendingImport.report.summary.version}</p></div></div>
          <dl>
            <div><dt>发布者</dt><dd>{pendingImport.report.summary.publisher}</dd></div>
            <div><dt>许可证</dt><dd>{pendingImport.report.summary.license}</dd></div>
            <div><dt>渲染后端</dt><dd>{pendingImport.report.summary.rendererBackend ?? "无"}</dd></div>
            <div><dt>包内容</dt><dd>{pendingImport.report.summary.fileCount} 个文件 · {formatBytes(pendingImport.report.summary.totalBytes)}</dd></div>
          </dl>
          {pendingImport.report.theme ? <ThemePreview theme={pendingImport.report.theme} /> : null}
          {pendingImport.report.voicePreview && previewAudioUrl ? <div className="voice-preview">
            <div><strong>{pendingImport.report.voicePreview.cue}</strong><span>{pendingImport.report.voicePreview.mediaType} · {formatBytes(pendingImport.report.voicePreview.bytes.length)} · {pendingImport.report.voicePreview.gainDb} dB</span></div>
            <p>{pendingImport.report.voicePreview.captions["zh-CN"] ?? pendingImport.report.voicePreview.captions.en ?? Object.values(pendingImport.report.voicePreview.captions)[0]}</p>
            <audio controls preload="metadata" src={previewAudioUrl}>当前环境不支持音频预览。</audio>
          </div> : null}
          {!pendingImport.report.poster ? <p className="asset-preview-warning">资源包未声明静态预览海报；元数据已验证，但安装前无法展示包内视觉内容。</p> : null}
          {pendingImport.report.theme ? <p className="asset-preview-warning">主题预览只作用于此卡片；主题包不能注入 CSS、脚本、字体或网络资源，也不能改变权限和危险状态语义。</p> : null}
          {pendingImport.report.voice ? <p className="asset-preview-warning">声音预览来自完整性验证后的本地字节，不执行代码、不访问网络；平台权限、危险、错误与恢复提示音不可被第三方声音替换。</p> : null}
          {pendingImport.report.summary.rendererBackend && !["sprite-sequence", "sprite-atlas"].includes(pendingImport.report.summary.rendererBackend) ? <p className="asset-preview-warning">该后端当前只能验证和安装，Pet Overlay 尚不能真实渲染，将使用内置角色。</p> : null}
          <div className="asset-preview-actions"><button className="text-button" type="button" disabled={importing} onClick={() => setPendingImport(null)}>取消</button><button className="primary-button" type="button" disabled={importing} onClick={() => void confirmInstall()}>{importing ? "正在复验…" : "确认并安装"}</button></div>
        </div> : null}
      </section>

      <section className="asset-import" aria-labelledby="asset-export-heading">
        <div className="section-heading">
          <div><p className="card-label">VERIFIED EXPORT</p><h3 id="asset-export-heading">生成可分发资源包</h3></div>
          <button className="secondary-button" type="button" disabled={!desktopApi.native || exporting || importing} onClick={() => void exportPackage()}>
            {exporting ? "正在打包…" : desktopApi.native ? "导出 .nimora" : "桌面版可用"}
          </button>
        </div>
        <p className="asset-import-intro">选择展开目录后，宿主先验证完整资产契约，再以稳定顺序和固定元数据原子写出可重复构建的 .nimora 包。</p>
        {exportError ? <p className="catalog-empty error" role="alert">{exportError}</p> : null}
        {exportNotice ? <p className="catalog-empty success" role="status">{exportNotice}</p> : null}
      </section>

      <section className="asset-import model-lab" aria-labelledby="model-lab-heading">
        <div className="section-heading">
          <div><p className="card-label">MODEL LAB · OFFLINE</p><h3 id="model-lab-heading">隔离检查 GLB 模型</h3></div>
          <button className="secondary-button" type="button" disabled={!desktopApi.native || checkingModel || installingModel || importing || exporting} onClick={() => void inspectModel()}>
            {checkingModel ? "正在检查…" : desktopApi.native ? "选择 GLB" : "桌面版可用"}
          </button>
        </div>
        <p className="asset-import-intro">模型先复制到一次性暂存目录，再由独立 Worker 在硬限额和截止时间内解析；全程离线，不上传原文件。</p>
        {modelError ? <p className="catalog-empty error" role="alert">{modelError}</p> : null}
        {modelCandidate ? <>
          <ModelProbeReportCard report={modelCandidate.report} />
          <div className="model-package-form">
            <label>包标识<input value={modelAssetId} onChange={(event) => setModelAssetId(event.target.value)} placeholder="character.local.name" /></label>
            <label>角色名称<input value={modelName} onChange={(event) => setModelName(event.target.value)} /></label>
            <label>许可证<select value={modelLicense} onChange={(event) => setModelLicense(event.target.value)}><option value="LicenseRef-Proprietary">保留所有权利</option><option value="CC0-1.0">CC0 1.0</option><option value="CC-BY-4.0">CC BY 4.0</option><option value="MIT">MIT</option></select></label>
            <div className="model-animation-map" aria-label="标准动作动画映射">
              <strong>标准动作映射</strong>
              <p>{modelCandidate.report.animationNames.length > 0 ? "已按名称给出建议，可逐项修正。" : "模型没有可识别的命名动画，将作为静态角色安装。"}</p>
              {modelActions.map((definition) => <label key={definition.action}>{definition.label}<select value={modelAnimationMap[definition.action]?.animation ?? ""} onChange={(event) => setModelAnimationMap((current) => {
                const next = { ...current };
                if (event.target.value) next[definition.action] = { animation: event.target.value, looped: definition.looped };
                else delete next[definition.action];
                return next;
              })}><option value="">不映射</option>{modelCandidate.report.animationNames.map((name) => <option value={name} key={name}>{name}</option>)}</select></label>)}
            </div>
            <button className="primary-button" type="button" disabled={installingModel || !modelAssetId.trim() || !modelName.trim() || (modelCandidate.report.animationNames.length > 0 && !modelAnimationMap["pet.idle"])} onClick={() => void installModel()}>{installingModel ? "正在生成并安装…" : "生成 Character 包并安装"}</button>
          </div>
        </> : null}
      </section>

      <section className="asset-catalog" aria-labelledby="asset-catalog-heading">
        <div className="section-heading">
          <div><p className="card-label">INSTALLED ASSETS</p><h3 id="asset-catalog-heading">本机资源目录</h3></div>
          <div className="asset-catalog-actions"><strong>{catalog?.assets.length ?? 0} 个可用</strong>{activeTheme?.source === "installed" ? <button className="text-button" type="button" disabled={activating !== null} onClick={() => void activateTheme("builtin.nimora")}>恢复内置主题</button> : null}{activeVoice?.source === "installed" ? <button className="text-button" type="button" disabled={activating !== null} onClick={() => void activateVoice("builtin.silent")}>恢复静音声音</button> : null}</div>
        </div>
        {catalogError ? <p className="catalog-empty error">资源目录暂时不可读取，当前角色不受影响。</p> : null}
        {!catalogError && catalog === null ? <p className="catalog-empty">正在验证已安装资源…</p> : null}
        {catalog?.assets.length === 0 ? <p className="catalog-empty">尚未安装第三方资源，默认角色继续离线可用。</p> : null}
        {activeCharacter?.fallbackReason ? <p className="catalog-empty error">已安全回退默认角色：{activeCharacter.fallbackReason}</p> : null}
        {activationError ? <p className="catalog-empty error">{activationError}</p> : null}
        {catalog && catalog.assets.length > 0 ? <ul className="asset-list">
          {catalog.assets.map((asset) => <AssetCatalogItem asset={asset} active={asset.assetType === "theme" ? activeTheme?.assetId === asset.id : asset.assetType === "voice" ? activeVoice?.assetId === asset.id : activeCharacter?.assetId === asset.id} activating={activating === asset.id} key={asset.id} onActivateCharacter={activate} onActivateTheme={activateTheme} onActivateVoice={activateVoice} />)}
        </ul> : null}
        {catalog && catalog.rejected.length > 0 ? <details className="rejected-assets">
          <summary>{catalog.rejected.length} 个资源未通过健康检查</summary>
          <ul>{catalog.rejected.map((asset) => <li key={asset.directory}><strong>{asset.directory}</strong><span>{asset.reason}</span></li>)}</ul>
        </details> : null}
      </section>

      <div className="creator-checks">
        <div className="section-heading"><div><p className="card-label">PACKAGE HEALTH</p><h3>发布前检查</h3></div><strong>{completion}/{checks.length} 已通过</strong></div>
        <ul>{checks.map((check) => <li key={check.label}><span className={check.status === "通过" ? "check-icon pass" : "check-icon pending"}>{check.status === "通过" ? "✓" : "!"}</span><div><strong>{check.label}</strong><p>{check.detail}</p></div><span className="check-status">{check.status}</span></li>)}</ul>
      </div>
    </section>
  );
}

export function ModelProbeReportCard({ report }: { report: ModelProbeReport }) {
  return <div className="asset-preview-report model-probe-report" role="status">
    <div className="asset-preview-title"><span className="asset-kind">3D</span><div><strong>{report.format.toUpperCase()} {report.formatVersion}</strong><p>{formatBytes(report.bytes)} · 已完成隔离结构检查</p></div></div>
    <dl>
      <div><dt>容器分区</dt><dd>JSON {formatBytes(report.jsonBytes)} · BIN {formatBytes(report.binaryBytes)}</dd></div>
      <div><dt>场景复杂度</dt><dd>{report.nodes} 节点 · {report.meshes} 网格</dd></div>
      <div><dt>材质资源</dt><dd>{report.materials} 材质 · {report.textures} 纹理</dd></div>
      <div><dt>动态能力</dt><dd>{report.animations} 动画 · {report.skins} 骨骼蒙皮</dd></div>
    </dl>
    <p className="asset-preview-warning">结构验证通过后可生成带 SHA-256 清单的标准 Character 包，并在激活后由 Pet Overlay 真实渲染 GLB；此检查本身不是安装前 3D 预览，也不证明版权、许可证、发布者签名或 OS/GPU 沙箱隔离。</p>
  </div>;
}

function ThemePreview({ theme }: { theme: ThemeDescriptor }) {
  return <div className={`theme-preview theme-${theme.cornerStyle}`} style={themePreviewStyle(theme)}>
    <div><strong>{theme.mode === "dark" ? "深色主题" : "浅色主题"}</strong><span>{theme.cornerStyle} · {theme.motion === "reduced" ? "减少动态" : "完整动态"}</span></div>
    <div className="theme-swatches" aria-label="主题语义色板">{Object.entries(theme.colors).map(([token, color]) => <span title={`${token}: ${color}`} style={{ background: color }} key={token} />)}</div>
    <button type="button">局部按钮预览</button>
  </div>;
}

function themePreviewStyle(theme: ThemeDescriptor): CSSProperties {
  return { "--preview-surface": theme.colors.surfaceElevated, "--preview-text": theme.colors.text, "--preview-muted": theme.colors.textMuted, "--preview-border": theme.colors.border, "--preview-accent": theme.colors.accent, "--preview-accent-soft": theme.colors.accentSoft } as CSSProperties;
}

function AssetCatalogItem({ asset, active, activating, onActivateCharacter, onActivateTheme, onActivateVoice }: { asset: AssetPackageSummary; active: boolean; activating: boolean; onActivateCharacter(assetId: string): Promise<void>; onActivateTheme(assetId: string): Promise<void>; onActivateVoice(assetId: string): Promise<void> }) {
  const displayName = assetDisplayName(asset);
  return <li>
    <span className="asset-kind">{asset.assetType.slice(0, 1).toUpperCase()}</span>
    <div><strong>{displayName}</strong><p>{asset.id} · {asset.version}</p></div>
    {asset.assetType === "character" ? <button className="text-button" type="button" disabled={active || activating} onClick={() => void onActivateCharacter(asset.id)}>{active ? "当前角色" : activating ? "验证中…" : "设为角色"}</button> : asset.assetType === "theme" ? <button className="text-button" type="button" disabled={active || activating} onClick={() => void onActivateTheme(asset.id)}>{active ? "当前主题" : activating ? "验证中…" : "设为主题"}</button> : asset.assetType === "voice" ? <button className="text-button" type="button" disabled={active || activating} onClick={() => void onActivateVoice(asset.id)}>{active ? "当前声音" : activating ? "验证中…" : "设为声音"}</button> : <span className="asset-backend">{asset.rendererBackend ?? "无渲染后端"}</span>}
  </li>;
}

export function assetDisplayName(asset: AssetPackageSummary): string {
  return asset.name["zh-CN"] ?? asset.name.en ?? Object.values(asset.name)[0] ?? asset.id;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`;
  if (bytes < 1_048_576) return `${(bytes / 1_024).toFixed(1)} KiB`;
  return `${(bytes / 1_048_576).toFixed(1)} MiB`;
}
