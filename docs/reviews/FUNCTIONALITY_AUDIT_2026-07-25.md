# Nimora 功能完整性审计（2026-07-25）

> 审计员：功能完整性外部审计（只读）
> 范围：全产品功能面 — 3D 宠物、桌面系统感知、AI Agent/CLI、Skill/Worker/Connector、用户代码、创作工坊/场景、皮肤/模型导入、能力网关/授权。
> 方法：对照 `docs/PRODUCT_SPEC.md`、`docs/IMPLEMENTATION_STATUS.md`、`docs/DESKTOP_PET_EXPERIENCE.md` 逐子系统核对实际代码，标注 file:line 证据。
> 声明：本报告仅静态代码审计，未运行原生 Tauri 包，未做视觉/性能/长稳验收。凡涉及 GPU、真机窗口、真实 Provider 往返的结论均以“须原生验收”标注。

## 执行摘要

代码库整体是**高完成度的“单机纵切”**：本地宠物、Capability Gateway、Agent 单轮、Skill Host、用户代码 Worker、模型导入、创作草案生成安装，都是真实可运行路径而非占位。核心缺口集中在三处规格承诺与实现的**结构性落差**：

1. **Open Gateway 与 Connector 整域缺失**（`PRODUCT_SPEC.md` §4.6 承诺 REST/WS/SSE + HTTP/UDP/MQTT Sink/Source；代码零服务器）。
2. **Skill 生命周期后端完整但无任何前端入口**（安装/授权/启停/执行/审批 9 个命令 UI 完全悬空）。
3. **活动场景（Creative Workshop）纯前端 localStorage 孤岛**，不经后端、不入事件/审计、跨窗口靠 `CustomEvent` 传递，与“Capability Gateway 是平台核心”的架构原则相悖。

下面按子系统给出实现程度与证据，最后是 P0/P1/P2 优先级与前十高价值缺口。

---

## 1. 3D 宠物渲染与生命感 — 实现（局部须原生验收）

- 内置 3D 宠物程序化编舞体量大、非占位：`apps/desktop/src/components/BuiltinPet3D.tsx:1`（1894 行），含 `sampleBuiltinPetMotion`、`vitalityBounceGain`、`microActBiasGain`、`attentionGazeBias` 等纯函数并有单测。
- 生命感数据接线真实：`composeLivingMoment` 的 `attention`/`microAct`/`vitality` 已从引擎接到渲染器（`IMPLEMENTATION_STATUS.md` 顶部多切片记录“丢弃数据接线修复”）。
- 渲染回退链完整：`PetOverlay.tsx:913` 注释与实现为 BuiltinPet3D → fox GLB → SVG，自定义包优先；GLB 走 `GltfRenderer.tsx:1`（364 行，真实 three.js + cross-fade + framing + 资源释放）。
- 宠物窗口在运行期由 `WebviewWindowBuilder`（`apps/desktop/src-tauri/src/lib.rs:13023`）以透明、无边框、置顶、skip_taskbar 创建；`tauri.conf.json` 只声明 `control-center` 一窗，pet 窗口运行时构建 —— 与 `DESKTOP_PET_EXPERIENCE.md` 的“双窗口”一致，非缺陷。

评级：**实现**。缺口是规格自己也承认的原生验收项（多屏/DPI、像素级遮挡、8 小时长稳、真实截图）。审计无法在静态层证伪。

## 2. 桌面系统感知（Rust） — 实现

- macOS 采样器为真实 CoreFoundation/CoreGraphics/IOKit FFI：`crates/desktop-context-macos/src/macos/`（cf/displays/foreground/idle/power/windows.rs 全部存在），入口 `sample()` 在 `crates/desktop-context-macos/src/lib.rs:81`，非 macOS fail-closed 到 `AdapterUnavailable`。
- Windows 采样器为真实 win32：`EnumDisplayMonitors`/`EnumWindows`/`GetForegroundWindow`/`GetSystemPowerStatus`（`crates/desktop-context-windows/src/lib.rs:407,516,568,622`）。
- 全屏/勿扰经 `osascript` 探测：`apps/desktop/src-tauri/src/system_context_sensor.rs:28`。
- 宿主真实接线：`sample_lifeform_environment_inner`（`apps/desktop/src-tauri/src/lib.rs:14105`）按平台调用采样器，失败降级为 `degraded_lifeform_snapshot(PlatformUnavailable)`。

评级：**实现**。真机权限（屏幕录制授权、AX 权限）与热插拔 DPI 属原生验收。

## 3. AI Agent / CLI（goal / auto-mode / context-compaction） — 部分实现

已实现（真实路径）：
- Agent Runtime 子系统齐全：`crates/agent-runtime/src/` 有 `goal.rs`、`auto_mode.rs`、`auto_execution.rs`、`context_management.rs`、`checkpoint.rs`、`coordinator.rs`、`task_gateway.rs`、`workspace.rs`、`reasoning.rs`。
- CLI 覆盖 goal CRUD、plan replace、status、auto start/status/pause/resume/cancel/away-summary、grant issue/list/show/revoke、run（`apps/cli/src/lib.rs:100` 起的 match 表）；CLI 集成测试真实（`apps/cli/tests/cli.rs`，含 provider probe 真实本地请求、sidecar 完整性 fail-closed、history 分页/删除）。
- 桌面 Auto Mode 有持久 job 宿主循环 `auto_mode_runner.rs:86` + 无人值守授权档位 UI（`AgentWorkspace.tsx` 的 tier 文案 427–483）。
- Provider：Ollama + OpenAI-compatible 双实现（`crates/agent-provider-worker/src/lib.rs:20`，`openai_compatible.rs`），loopback-only SSRF 防护。

缺口：
- **Anthropic Provider 缺失**：`PRODUCT_SPEC.md` §4.7 承诺 “OpenAI-compatible、Anthropic 和本地”；全仓 `rg -i anthropic` 零命中。当前只有 Ollama + OpenAI-compatible。
- **`run_local_agent` 前端默认锁定在 deterministic-local/echo**（`apps/desktop/src/platform/desktop.ts:2190`），真实模型选择依赖 Provider Settings；文档承认“用户本机实际 Ollama 桌面验收未完成”。
- 桌面持久 Cache 的 Auto Host Loop 接入、Session 显式重绑定、双仓储原子应用、桌面 Goal/Plan/Attempt UI 均由 `IMPLEMENTATION_STATUS.md:1002,1019` 自述未闭环。

评级：**部分实现**（与文档一致，但 Anthropic 是明确的规格未兑现项）。

## 4. Skill / Worker / Connector “宠物化” — 后端实现 / 前端悬空 / Connector 缺失

- Skill 后端完整：`skill-runtime`（851 行）、`skill-host`（629 行）、`skill-worker`（284 行）、`skill-package`（486 行），真实进程隔离 + Boa JS + 租约 + 原子安装 + 回滚。
- 桌面注册 9 个 Skill 命令：`install_skill`、`skill_catalog`、`authorize_skill`、`set_skill_enabled`、`rollback_installed_skill`、`execute_skill`、`pending_skill_approvals`、`approve_skill_execution`、`reject_skill_execution`（见 `lib.rs` invoke_handler 列表）。
- **前端零调用**：这 9 个命令在 `apps/desktop/src/platform/desktop.ts` 与所有组件中均无引用（仅 `skill_execution_history_list`/`delete`/`cancel` 三个历史命令有 UI）。用户无法安装/授权/启停/执行 Skill —— 只能通过 AI 创作工坊生成后原子安装（`install_creator_draft` 的 Skill 分支 `lib.rs`），且安装后 `authorized:false, enabled:false`，**没有任何 UI 能把它授权启用**。这是一条明确的死路。
- **Connector 完全未实现**：`PRODUCT_SPEC.md` §4.6 承诺 Sink（Webhook/WS/UDP/MQTT）与 Source（SSE/WS/MQTT）Connector。代码里 “connector” 只指 `companion_directive.rs` 的 `ConnectorSenseKind`（在线/离线感知的宠物情绪反应，`apps/desktop/src-tauri/src/companion_directive.rs:260`），**不是真正的连接器运行时**。`IMPLEMENTATION_STATUS.md:1002` 自述“Connector Runtime 的模块反向调用链尚未贯通”。

评级：Skill = **后端实现 / UI 缺失（死路）**；Connector = **未实现**（文档一致）。

## 5. 用户代码能力（user-code） — 部分实现 / 前端仅测试可达

- 后端体量足：`user-code-gateway`（792）、`user-code-policy`（687）、`user-code-package`（568）、`user-code-host`（322）、`user-code-storage`（269）、`user-code-worker`（222），真实独立 JS Worker + 预算 + 强制取消 + 版本精确授权。
- 桌面注册约 20 个 user_program 命令（install/validate/execute/start/stop/grant/revoke/event session…），平台层 `desktop.ts` 亦有绑定（`desktop.ts:2306` 起）。
- **前端无用户程序管理 UI**：`install_user_program` 等在任何 `components/*.tsx` 中均无引用，仅 `platform/desktop.test.ts` 与 AI 创作工坊的原子安装路径触及。与 Skill 同病：装得进、跑不起来（无授权/启停界面）。
- 授权型文件/网络/自动化后端、调试器、录制回放、OS 硬资源限制未实现（`IMPLEMENTATION_STATUS.md:985` 自述）。

评级：**部分实现，前端能力管理缺失**。

## 6. 创作工坊 / 活动场景 — 拆成两块，结论不同

### 6.1 AI 扩展工坊（AiCreatorWorkspace） — 实现
- `generate → check（隔离 DryRun）→ approve（摘要绑定）→ install（原子）` 全链路真实：`install_creator_draft`（`lib.rs`）对 UserProgram/Skill/Automation/Theme/Profile 五类分别走生产安装器。
- 阻断原因诚实暴露：`creatorGenerateBlockReason`（`AiCreatorWorkspace.tsx:38`）。
- 能力提案治理有独立组件：`CapabilityProposalGovernance.tsx:25`。
- 评级：**实现**。缺 Anthropic/云 Provider 与真实模型验收。

### 6.2 活动场景工坊（ActivitySceneWorkshop） — 前端孤岛（架构缺陷）
- 全部状态存 localStorage：`persistScenes`/`persistActiveSceneId`（`activityScenes.ts:366,382`，键 `nimora.activity-scenes/v1`）。
- 跨窗口传递靠浏览器 `CustomEvent("nimora:activity-scene-changed")`（`ActivitySceneWorkshop.tsx:67` 派发，`PetOverlay.tsx:283` 监听）。
- **不经任何 Tauri 命令、不入 Outbox/事件、不受 Safe Mode/Profile 治理、不入审计**。与 `PRODUCT_SPEC.md` “所有可执行能力注册为 Command / 用户代码只能经 Gateway” 的核心原则相悖。控制中心与 pet 是两个独立 WebView，localStorage 是否共享取决于同源；跨窗口一致性脆弱。
- 评级：**部分实现，架构性缺陷**。功能能用（宠物会换话术/道具），但绕过平台契约。

## 7. 自定义皮肤 / 模型导入 — 实现（局部）

- 模型导入真实：独立 `model-importer` Worker（623 行）+ `inspect_model`/`import_model` 命令 + CreatorStudio UI（`CreatorStudio.tsx:507,529`），GLB 容器解析、资源预算、URI/路径安全有单测（`IMPLEMENTATION_STATUS.md` 顶部 model-importer 12 passed）。
- Skin Pack：schema 与 installer 认 `"skin"` 类型（`packages/schemas/src/index.ts:220`，`crates/asset-installer/src/lib.rs:1895`），sprite/skin 强制 `entrypoints.clips`。
- VRM 1.0 真实接入（`@pixiv/three-vrm` 按需 Adapter）；Live2D 被 Installer 提前拒绝（许可证）；Behavior Pack **未实现**（schema 认 `"behavior"` 但无动作图运行时，`IMPLEMENTATION_STATUS.md:984`）。

评级：**实现（模型/皮肤/主题/声音）**；Behavior Pack **未实现**；VRM look-at/lip-sync 待补。

## 8. 能力网关 / 授权 — 实现

- Capability Gateway 是真实进程内窄边界：`user-code-gateway`、`agent-tools` 的 `GatewayToolBackend` 把 Provider Tool → `CapabilityRequest` → `safe.*` 命令（`IMPLEMENTATION_STATUS.md:90`）。
- Automation → Gateway 命令映射与风险取大：`automation-capability-bridge`（424 行）。
- 授权 Grant 有 SQLite 仓储 + at-rest XChaCha20-Poly1305 信封（`IMPLEMENTATION_STATUS.md` 顶部 authorization_grant 10 passed）。
- 授权档位 UI 完整（observe/workspace/trusted_workspace/unattended/full_device，`AgentWorkspace.tsx:427`）。

评级：**实现（进程内）**。跨进程网关（Open Platform 配对/Token/Scope）随 §4 Connector 一并缺失。

---

## 破损流程 / 死路 / UI-后端错配

| # | 现象 | 证据 | 影响 |
|---|---|---|---|
| A | Skill 装了无法授权/启用/执行 | 9 个 skill_* 命令零前端调用；`install_creator_draft` 返回 `authorized:false,enabled:false` | 用户生成的 Skill 永久处于禁用态，功能死路 |
| B | 用户程序装了无法管理 | `install_user_program` 等仅测试可达 | 同上 |
| C | 活动场景绕过平台 | localStorage + CustomEvent，无 Tauri 命令 | 无审计/无 Safe Mode 治理，跨窗口脆弱 |
| D | Anthropic Provider 承诺未兑现 | 全仓零 anthropic 命中 | §4.7 规格缺口 |
| E | Open Gateway/Connector 整域缺失 | 无 REST/WS/SSE 服务器；connector=情绪感知 | §4.6 规格缺口（文档已诚实标未实现）|
| F | Behavior Pack 只有 schema | `asset-installer` 认类型但无运行时 | §4.3 资源类型缺口 |
| G | 包签名/Registry 未实现 | `IMPLEMENTATION_STATUS.md:992` | M5 阶段缺口 |

注：A/B/C 是本次审计**新发现的实现内错配**（文档未明确标注为死路）；D/E/F/G 与文档自述一致，属规格未兑现而非隐藏缺陷。

---

## 优先级与实施建议

### P0（承诺功能死路 / 用户可见断链，代价有界）

1. **打通 Skill 管理 UI**（缺口 A）。在 `AiCreatorWorkspace.tsx` 或新增 `SkillWorkspace.tsx` 里接 `skill_catalog`/`authorize_skill`/`set_skill_enabled`/`execute_skill`/`pending_skill_approvals`/`approve_skill_execution`/`reject_skill_execution`；先在 `apps/desktop/src/platform/desktop.ts` 补 7 个方法（模式照抄现有 automation 方法）。后端已就绪，纯前端接线。目标文件：`platform/desktop.ts`、新增/扩展一个工作区组件。
2. **打通用户程序管理 UI**（缺口 B）。同 P0-1 模式，接 `install/validate/execute/grant/revoke/stop` 与事件会话命令。目标文件：`platform/desktop.ts`（方法已部分存在）、新增组件。

### P1（架构一致性 / 规格核心承诺）

3. **活动场景经后端持久化与治理**（缺口 C）。新增 `activity_scene_catalog`/`set_active_scene` Tauri 命令，落 SQLite（复用 `persistence-sqlite`），经事件总线广播到 pet 窗口而非 `CustomEvent`；保留 localStorage 作离线回退。目标文件：`apps/desktop/src-tauri/src/lib.rs`、`crates/persistence-sqlite/src/lib.rs`、`ActivitySceneWorkshop.tsx`、`PetOverlay.tsx`。
4. **Anthropic Provider Adapter**（缺口 D）。在 `crates/agent-provider-worker/src/` 新增 `anthropic.rs`，仿 `openai_compatible.rs` 的 endpoint/complete/probe 结构，接入 `ProviderWorkerRequest` 枚举；前端 Provider Settings 增类型。目标文件：`agent-provider-worker/src/lib.rs`、新增 `anthropic.rs`、`ProviderSettings.tsx`。

### P2（大体量新域，建议单独里程碑，勿在本轮硬塞）

5. **Open Gateway + Connector Runtime**（缺口 E）。REST/WS/SSE loopback server + Sink/Source connector + 配对/Token/Scope/审计。属 M4 整域，需独立设计与安全评审，不建议在完整性修补轮内实现。
6. **Behavior Pack 运行时**（缺口 F）。声明式状态图解释器 + 资源预算 + Renderer 兼容。
7. **包签名 / Registry**（缺口 G）。发布者签名、信任根、撤销、兼容检测。

---

## 前十高价值功能缺口（按价值×可行性排序）

1. **Skill 无授权/启停/执行 UI** — 生成即死路；后端全就绪，纯前端接线（P0）。
2. **用户程序无管理 UI** — 同上，能力已建好却不可达（P0）。
3. **活动场景绕过 Capability Gateway/审计** — 违背核心架构原则，跨窗口脆弱（P1）。
4. **Anthropic Provider 未实现** — §4.7 明确承诺三类 Provider，实为两类（P1）。
5. **Creator 生成的 Skill/Program 安装后无法授权** — 与 1/2 同源，install 返回 `authorized:false` 且无后续入口（P0）。
6. **Open Gateway（REST/WS/SSE）缺失** — §4.6 整域空白，第三方集成不可能（P2/独立里程碑）。
7. **Sink/Source Connector 缺失** — 对外投递/事件导入无实现（P2）。
8. **Behavior Pack 运行时缺失** — 资源类型承诺未兑现（P2）。
9. **包签名与 Registry 未实现** — 生态分发与信任根缺失，M5 空白（P2）。
10. **桌面 Goal/Plan/Attempt UI 与持久 Cache Auto Host Loop 未闭环** — CLI 有、桌面无，Agent 域体验割裂（P1，文档自述）。

## 审计边界声明

- 未运行 `pnpm exec vitest`、`cargo test`、`cargo clippy` 或原生 Tauri 包；测试计数引用自 `IMPLEMENTATION_STATUS.md` 顶部门禁，未复算。
- 未做 GPU/多屏/DPI/透明穿透/长稳等原生视觉与性能验收。
- 缺口 A/B/C 为静态代码追踪结论（命令注册 vs 前端引用），建议以真机点击复核。
