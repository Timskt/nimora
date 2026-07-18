# Nimora 里程碑回顾

> 更新日期：2026-07-18  
> 状态：持续维护  
> 目的：在可演示纵切、每五个生产提交、公开契约变化或重大偏差时校准方向

## 1. 回顾规则

每次回顾必须记录范围、证据、偏差、用户价值、安全与隐私、离线行为、UI 影响、稳定性、Actions 分钟影响和下一纵切。回顾不是进度宣传；未通过真实测试或真实宿主验证的能力必须继续列为缺口。

以下情况必须立即回顾并修正：

- 实现开始复制已有链路，形成第二套 Provider、Tool、权限或持久化通道。
- UI、CLI 与模块调用对同一能力产生不同安全语义。
- 为追求“自动”而吞掉未知结果、扩大授权或绕过人工复核。
- CI 失败依赖反复推送试错，或跨平台矩阵在无平台风险时自动运行。
- 新需求能显著提升大众用户、创作者或开发者价值，却未进入权威规格与验收。

## 2. M-2026-07-18 Agent 自主执行基线

### 已完成证据

- 持久 Goal/Plan、Workspace Snapshot、Context Compaction/Cache、Auto Mode Session/Checkpoint/Attempt 和公平有界 Loop 已形成领域链路。
- 桌面单轮 Resume 已接入生产 Provider Registry、Tool Registry、Capability Gateway、真实工作区重扫和持久提交。
- Execution Grant 将沙箱、批准、网络、数据、工具、预算与寿命正交建模；Plan 或 Workspace 漂移后授权失效。
- 推理等级与 Provider 映射可审计，不支持的显式等级 fail-closed。
- 常规 CI 收敛为 PR 上按代码路径触发的 Ubuntu 单套质量门禁；直接推送不重复运行，纯文档 PR 不消耗构建分钟；macOS/Windows 矩阵仅在里程碑、发布候选或平台风险时手动触发。

### 竞品吸收与 Nimora 增强

- 吸收 Codex 的 Goal 持续推进、Sandbox/Approval 分离、文件追踪和可恢复执行，但完成结论必须由当前 Plan 的逐项证据证明。
- 吸收 Claude Code 的分层配置、Auto Mode、模型选择与 Away Summary，但无人值守批准使用范围绑定 Grant，不提供永久无边界开关。
- 吸收 OpenCode 的工具级 `allow/ask/deny` 与可替换 Provider/Agent，但统一落入 Capability Gateway，禁止 Agent 自建旁路。
- Nimora 新增桌面伴侣事件、Automation、Skill、用户程序与 Agent 的双向能力图；模块调用 AI 必须经过 `module-agent-adapter` 与 `AgentTaskGateway`。
- Nimora 新增未知结果隔离、工作区/计划漂移自动失效、自动审查不扩权和面向离开用户的结构化归来摘要。

### 本次纠偏

桌面 `resume_auto_mode_turn` 是同步单轮恢复接口，不能在线程中重复调用来冒充后台 Auto Mode：首轮继续后 Session 保持 Running，而下一次恢复只接受 Paused Session。真正后台实现必须直接装配 `AutoModeLoopService`，在一个批次内持有领域续体，并在批次 `yielded` 后由 Supervisor 公平续调。暂停、取消和退出必须原子收敛持久 Session/Task/Attempt，不能只改变内存 UI 状态。

### 下一纵切硬验收

1. 同一 Session 最多一个活跃 Job，重复 Start 原子拒绝。
2. Job 使用版本化快照，支持 Start、Status、Pause、Cancel，不暴露 Provider 或 Tauri 原生对象。
3. 每批有限 Turn；完成、漂移、业务暂停和未知结果立即停止；Yield 后公平续调。
4. Pause/Cancel 传递到当前 Provider/Tool，并持久化业务状态；未知执行结果标记 `indeterminate`，不得自动重放。
5. Safe/Recovery Mode 与应用退出取消所有 Job，并进行有界等待或隔离未收敛结果。
6. 浏览器预览只返回 `desktop-host-required`，不得伪造后台执行。
7. Rust 并发/退出测试、TypeScript 契约测试、桌面构建和文档验收全部通过。

### GitHub Actions 分钟复核

- 月度硬预算：2000 分钟。
- 本纵切开发必须先本地运行定向测试，再运行完整质量门禁，最后只推送一次可验证提交。
- `main` 推送不自动触发全量 CI；开发者在合并前使用 PR 门禁，确需验证主干、里程碑或风险修复时通过 `workflow_dispatch` 填写原因手动运行。
- 常规提交不触发手动跨平台矩阵；后台线程、退出生命周期或平台 API 出现真实平台风险时，才在里程碑提交后手动运行一次。
- 不通过削弱安全、契约或发布验证节省分钟；应优先采用路径过滤、并发取消、依赖缓存和失败快速终止。

## 3. M-2026-07-18 后台 Auto Mode 与崩溃恢复

### 触发原因

- 后台 Job 已形成可演示纵切，并累计超过五个生产提交。
- 新增 `start/status/pause/cancel/history` 公开桌面契约，必须复核跨重启语义。

### 完成证据

- 独立 Runner 直接装配有界 Auto Host Loop；Supervisor 保证单 Session 唯一、控制传播、单调进度与终态释放。
- Exit 与 Safe Mode 使用统一有界排空；超时分别隔离为 `shutdown-timeout` 和 `safe-mode-timeout`，迟到 Runner 不能覆盖终态。
- 正常启动把崩溃遗留 Running Session 原子转为 `paused/restarted`，Active Attempt 转为 `indeterminate`；恢复投影不占用 Session、不创建线程且不触发 Provider/Tool。
- 专用可恢复查询不会被普通历史挤出；超过 256 条时失败关闭，不静默漏掉未决 Attempt。
- 真实 SQLite 跨桌面状态重启测试、仓储测试、Supervisor 测试、严格 Clippy、TypeScript 与生产构建构成当前证据链。

### 偏差与纠正

- 原入口文件承载约 300 行 Runner 编排，已提取为显式依赖的独立模块并删除临时超长函数豁免。
- Safe Mode 最初只取消 Provider、Skill 与用户程序，遗漏后台 Auto Job；现已纳入统一排空协议。
- 最初恢复方案按最近 256 条普通历史扫描，可能漏掉更老 Attempt；已改为专用可恢复集合并检测溢出。
- Job 投影不是事实源，不能反向覆盖 Session/Checkpoint/Attempt，也不能被描述为自动续跑。

### 安全、离线与稳定性

- 恢复完全读取本地 SQLite，不需要网络；任何未证明的外部结果永久进入人工对账状态。
- Recovery Mode 使用空 Supervisor 并拒绝新 Job；数据库异常不会降级为内存自动执行。
- 退出等待有界，不因 Provider 卡死永久阻塞桌面关闭。

### UI 与无障碍

- 宿主历史契约已具备，但 Goal/Plan/Attempt/Job Control Center 尚未完成，`indeterminate` 也没有可操作的人工对账界面。
- 浏览器预览继续拒绝伪造宿主 Job；真实交互、键盘路径、读屏状态播报和桌面截图仍需后续纵切证明。

### Actions 分钟

- 本里程碑继续本地全量验证后单次推送，不手动触发 macOS/Windows 矩阵。
- `GltfRenderer` 大块警告与本纵切无关，登记后独立处理，避免混合提交和重复 CI。

### 下一纵切硬验收

1. 为 `indeterminate` Attempt 提供只读详情、明确风险解释与参数绑定的人工对账命令。
2. 对账只能选择“确认未执行后重试”或“确认已执行并接受结果”等受约束决议，禁止删除证据后直接续跑。
3. 决议必须原子更新 Attempt、Session、Task 与 Checkpoint，并保留不可变审计事件。
4. Goal/Plan/Attempt/Job 控制中心展示恢复历史、暂停原因、预算、Checkpoint 和下一安全动作。
5. 浏览器完成视觉预览，真实 Tauri 完成宿主交互与跨重启端到端验证。

## 4. M-2026-07-18 未知执行结果人工对账

### 完成证据

- 新增不可变 `auto_mode_attempt_resolution` 事实表与单一原子用例；Session、Task、Checkpoint、Attempt 和 Resolution 在同一 Immediate 事务内收敛。
- 决议严格限定为 `confirmed_not_executed` 与 `accept_external_effect_and_cancel`，不伪造 Provider 成功、不删除证据后续跑、不在决议命令内自动重试。
- 请求绑定 Session、Attempt、Checkpoint sequence、request fingerprint、actor、reason 与宿主时间；陈旧、重放和非 `indeterminate` 状态失败关闭。
- 桌面 IPC 提供详情、风险说明、下一安全动作、100 条有界审计查询与决议入口；浏览器预览稳定返回 `desktop-host-required`。
- Workspace Clippy、Workspace Rust tests、桌面 85 tests、持久化 52 tests、Vitest 31 tests、TypeScript 与生产构建全部通过。

### 架构复核与纠正

- 对账属于持久化应用用例，不放在 UI 或 Supervisor；未来桌面、CLI 与恢复向导复用同一事实契约。
- Agent Runtime 保持纯领域化，未新增“未知成功”伪状态；宿主仍负责时间和数据库路径。
- Resolution 是审计事实，Job Snapshot 仍只是投影。后续控制中心应由 Session/Checkpoint/Attempt/Resolution 组合查询生成恢复视图。
- 已补真实 SQLite 专属夹具：两种决议、陈旧绑定零写入、非未知拒绝、重放、双连接竞争、跨 reopen、分页边界及索引/载荷分叉均有直接证据。

### 下一纵切硬验收

1. 实现 Goal/Plan/Attempt/Job 控制中心，把风险、预算、Checkpoint、不可变历史和安全下一步做成可访问 UI。
2. 用浏览器完成响应式、键盘、读屏与视觉截图审查，并在真实 Tauri 完成宿主交互验证。
3. 独立处理 `GltfRenderer` 约 615 KB 的按需加载与性能预算，不混入 Agent 安全提交。

## 5. 后续回顾模板

```text
里程碑：
触发原因：
用户可演示价值：
完成证据：
未完成与偏差：
安全/隐私/离线：
UI 与无障碍：
稳定性与恢复：
Actions 分钟：
竞品变化与新技术：
新增需求：
下一纵切硬验收：
```

## M-2026-07-18 Goal/Plan/Job 聚合控制中心

- 纠偏：前端分别查询 Job 与恢复事实会产生竞态，也会错误地把内存投影视为事实源。
- 修正：新增宿主侧 `auto_mode_control_center` 有界聚合；Session 精确读取其历史 Plan revision，并同时返回 Goal、Checkpoint、Attempt 与不可变 Resolution。
- UI：Agent 一级工作区新增“对话运行 / 目标控制 / 执行历史”，控制页展示进度、预算边界、Checkpoint、缓存、模型、计划步骤与未知结果风险。
- 验证：Rust 历史 Plan/重启聚合测试、TypeScript 类型检查和浏览器宽屏截图/语义树通过；720px 响应式规则已落地，浏览器当前未暴露视口切换能力。
- 后续：补齐真实 Tauri Pause/Cancel/人工对账交互面，随后独立处理 `GltfRenderer` 分包与性能预算。

## M-2026-07-18 3D 渲染加载与性能预算

- 审计：`PetOverlay` 已使用 React lazy/Suspense，615 KB 警告来自 Three.js 单体异步入口，并非首屏同步回归；盲目按符号拆分没有改变真实产物。
- 修正：Three.js 使用显式渲染原语导入；Vite 输出 Manifest，并由 `scripts/check-bundle-budget.mjs` 校验入口关系与原始字节预算。
- 证据：主入口 285,916 / 350,000 bytes；独立 GLTF 动态入口 615,183 / 650,000 bytes；生产构建不再产生泛化大块警告。
- 边界：GLTF 仍由受控角色描述符触发，加载中使用稳定占位，WebGL 初始化、资源加载或 Context 丢失继续降级至内置角色。
- 后续：真实 Tauri/WebGL 连续模型切换与 GPU 资源释放仍需进入跨平台发布验收，不以 Bundle Budget 替代运行时测试。

## M-2026-07-18 控制中心真实操作与一致性纠偏

- 不足：Pause/Cancel 原先主要依赖 UI 禁用，缺少宿主门禁回归证据；Job 投影与持久 Session 状态可能短暂分叉，React 被迫猜测事实。
- 修正：Pause、Cancel、人工对账统一经过 Normal/Safe/Recovery 宿主门禁；对账理由必填并绑定 Attempt、Checkpoint、指纹与固定桌面用户主体。
- 一致性：控制中心契约升级为 `/2`，由宿主返回持久化 `effectiveStatus` 和 `projectionStale`，UI 明示收敛状态且绝不自动重试未知外部操作。
- 证据：Normal Pause→Cancel 升级、Safe/Recovery 零状态变化、空理由持久化前拒绝、31 项前端测试、TypeScript，以及浏览器宽屏截图与语义树均通过。
- 边界：浏览器只验证预览只读态；真实 Tauri 的鼠标、键盘、跨重启对账和操作系统级辅助技术仍必须单独验收。

## M-2026-07-18 安全主题资产纵切

- 不足：统一 Asset Manifest 过去没有 Theme 严格子契约，任意 CSS 式主题会形成注入面；角色专用选择锁命名也错误暗示不能扩展。
- 修正：新增 `nimora.theme/1` 九 Token 白名单、完整性复验、安装前局部预览、原子激活记录和 Safe Mode/损坏包回退；选择写锁泛化为 Asset Selection。
- UI：App Shell 只映射固定 CSS Variables，支持深浅模式、三档圆角和减少动态；主题不能控制权限、危险、错误或恢复语义。
- 证据：Asset Installer 30 项、Desktop Host 90 项中的主题回退测试、前端 31 项、TypeScript 生产构建与 Bundle Budget 通过。
- 边界：签名 Registry、高对比度自动检查、主题编辑器与真实 Tauri 深色跨平台截图仍未完成，不提前标记完整主题生态。

## M-2026-07-18 主题可访问性与状态一致性纠偏

- 不足：合法 Hex 不代表可读；低对比度主题仍可隐藏正文和危险态。Safe Mode 宿主虽回退内置主题，React 也可能保留切换前的陈旧 Token。
- 修正：安装器对 RGBA 合成结果执行正文 `4.5:1`、弱文本与语义色 `3:1` 门禁；Creator Studio 提供显式恢复内置主题，安全模式切换后重新读取宿主主题事实。
- 证据：新增低对比度正文与危险色拒绝测试，Installer 测试增至 31 项；前端构建和全量 Workspace 门禁随本里程碑验证。
- 边界：最低数学门槛不等于完整无障碍验收，色觉差异、200% 缩放、真实桌面深色截图和读屏仍保留为发布门禁。

## M-2026-07-18 安全字幕声音资产纵切

- 不足：通用 Inventory 媒体扩展校验错误携带 Sprite 语义；仅校验 Cue 格式也无法阻止第三方伪装平台权限声音。
- 修正：媒体校验改为资产无关边界；新增 `nimora.voice/1`、WAV/OGG Header、2 MiB/32 Clip 预算、字幕与增益门禁，以及固定宠物 Cue 注册表。
- 宿主：新增原子活动声音选择、`builtin.silent`、Safe Mode 静音回退和逐次重开复验；WebView 只获得无路径 Descriptor 与有界字节。
- UI：Creator Studio 支持安装前本地音频预览、声音激活和恢复静音；动作成功后才异步播放，Quiet Mode 在 Clip 请求前阻断，播放失败不影响动作。
- 证据：Asset Installer 36 项、Desktop Host 91 项、前端 32 项、TypeScript 生产构建与 Bundle Budget 已通过专项验证。
- 架构收敛：角色、主题、声音已复用类型化 `AssetSelectionPolicy`；统一 Schema、Safe Mode、错误分类和原子写，同时保留各资产独立内容复验及 Character Renderer 回滚，Desktop Host 测试增至 95 项。

## M-2026-07-18 VRM 1.0 开放角色纵切

- 修正能力漂移：Manifest 不再接受尚无合规 Runtime 的 Live2D；VRM 只有通过真实格式复验后才进入 Renderer。
- 安全边界：Importer 识别 GLB 2.0 内声明的 `VRMC_vrm`，强制 1.0、meta、humanoid、无外部 URI与既有资源预算；Installer 重开内容复验并拒绝普通 GLB 伪装。
- 渲染边界：`@pixiv/three-vrm` 仅在活动 VRM 时动态加载，驱动 MToon/Spring Bone 更新；卸载时深度释放 VRM 场景，失败沿用内置角色回退。
- 性能边界：Bundle Gate 按依赖图计算 GLTF 基础图，并单独限制 VRM 增量，不能通过代码拆块隐藏真实成本。

## M-2026-07-18 VRM 公共表情语义纵切

- 修正架构缺口：动作分发不再依赖 Animation Clip 存在；只有 Expression 的合法 VRM 同样收到桌宠状态。
- 依赖方向：公共动作到 VRM Preset 的策略位于纯 TypeScript 模块，Three.js 组件只组合动画与表情 Adapter，避免产品语义嵌入渲染生命周期。
- 安全与鲁棒性：只允许固定标准 Preset，每次先 reset；缺失 Preset、Manager 异常、未知动作和私有名称均安全降级 neutral。
- 无障碍语义：Reduced Motion 只冻结连续动画和物理更新，不阻止静态表情状态切换。
- 自动化证据：桌面前端 40 项测试（含无 Animation Player 的组合分发回归）、TypeScript Build、生产 Bundle Budget 与架构边界门禁通过。
- 剩余边界：用户自定义映射、look-at、lip sync、humanoid retarget、真实 VRM 样本截图及 GPU 长稳仍未完成。

## M-2026-07-18 架构边界机器化纠偏

- 不足：目标拓扑与当前实现混写，架构文档仍声称 VRM 未实现并描述不存在的统一 Extension Host；三个 UI 组件直接导入 Tauri，平台 Port 约束只有文字承诺。
- 修正：UI 文件选择、保存与角色变更事件全部收敛到 `platform/desktop.ts`；Creator Studio 同时开放严格 `.glb/.vrm` 选择，不再让页面持有插件协议或原生事件名。
- 门禁：新增自验证 `pnpm check:architecture`，检查 UI 原生导入和关键 Rust 层的禁止依赖，并接入本地 `check` 与低成本 Ubuntu CI。
- 边界：当前实现是确定性文本扫描，不证明传递依赖；后续需基于 Cargo Metadata 与 TypeScript AST 扩展，但新增直接旁路现在会立即失败。

## M-2026-07-18 Desktop 组合根首轮拆分

- 不足：`src-tauri/lib.rs` 超过 11,000 行，资产选择 Policy、存储契约、Safe Mode 解析和原子写与 Tauri IPC 混在同一组合根。
- 修正：新增 Tauri-free `asset_selection` Application Module，统一拥有 Character、Theme、Voice 的类型化 Policy、持久契约、损坏回退和原子替换；组合根只消费结果并保留资产专属复验与 Renderer 回滚。
- 防回退：架构门禁自验证后扫描该模块，拒绝 `tauri::`、`State`、`AppHandle` 和命令宏；不使用空 Trait 或兼容转发层。
- 证据：Desktop Host 95 项真实文件/错误/回退测试保持通过；后续继续拆 DTO、Agent、Skill 和 Asset Application Service，不能将首轮拆分描述为组合根治理完成。

## M-2026-07-18 Asset Protocol 宿主解耦

- 不足：资产 URL 的 Window、Method、Host、Query、路径解码、活动角色和 Inventory 复验直接依赖 Tauri HTTP 类型，安全规则与原生协议注册无法独立演进。
- 修正：新增纯数据 `AssetProtocolRequest/Result/Status` 与 Tauri-free `asset_protocol` Application Module；模块自行复验当前 Character 包和唯一 Inventory Entry，不接受调用方注入“已验证”闭包。

- Adapter：Tauri URI Scheme 仅提取 Method/Host/Path/Query 并映射状态码，不拥有授权或文件读取规则；错误响应保持无路径、无内部原因的固定正文。
- 证据：路径歧义、编码逃逸、错误窗口/Host/Method、Safe Mode、非活动资产、Manifest/Integrity/非入口文件拒绝及真实 GLB 字节读取继续由 Desktop Host 95 项测试覆盖；架构门禁拒绝 Tauri 类型回流。

## M-2026-07-18 Diagnostic Report 应用服务拆分

- 不足：诊断报告规格、运行模式映射、数据保护摘要和隐私声明直接构造在 Tauri 组合根中，未来 CLI/恢复工具复用时容易复制并漂移隐私语义。
- 修正：新增 Tauri-free `diagnostic_report` Application Module，只消费归一化事实并生成 `nimora.diagnostic-report/1`；宿主仅采集 Safety、Outbox、Backup 和 Journal 投影。
- 隐私边界：`includes_logs`、用户内容、Secret、文件路径和自动上传全部在单一服务中 fail closed；调用方不能通过输入扩大这些声明。
- 防回退：架构门禁拒绝该模块引入 Tauri Command、`State` 或 `AppHandle`；独立测试覆盖版本契约、Normal/Recovery/Safe 映射和隐私不变量。
- 证据：Desktop Host 98 项测试通过；组合根仍超过一万行，Agent、Automation、Skill、Profile 与 Backup 服务拆分仍需继续，不能宣称 R-019 已关闭。

## M-2026-07-18 Backup 应用服务收敛

- 不足：手动备份 Command 和后台定时线程分别维护 `backup_last_error`，成功清错、失败记错和 Health 聚合存在两套实现，未来 CLI 入口会进一步分叉。
- 修正：新增 Tauri-free `BackupService`，统一 `health`、`create_now`、`create_if_due` 与 `request_restore`；Tauri Command 和调度线程只负责授权及诊断事件。
- 错误语义：成功操作必须清除旧错误；真实 I/O 失败保留原始错误并以共享投影暴露给所有健康消费者；错误锁损坏时成功路径 fail closed。
- 防回退：架构门禁拒绝 Backup Service 引入 Tauri 类型；稳定的“数据库路径被目录占用”故障夹具同时覆盖手动与定时入口。
- 证据：Desktop Host 102 项测试覆盖手动/定时失败共享投影与成功后清除旧错误；跨休眠调度、磁盘耗尽、并发手动/定时备份和真实恢复重启仍需平台故障注入。

## M-2026-07-18 原生策略可逆事务统一

- 不足：Profile 切换、进入 Safe Mode、退出 Safe Mode 各自复制“原生窗口预应用→领域提交→失败回滚”模板，任一入口都可能漏掉回滚或吞掉次级故障。
- 修正：新增泛型 Tauri-free `run_reversible_transition`，宿主 Adapter 只绑定 `WindowPolicy` 与 `apply_window_policy`；三条入口共享完全相同的事务状态机。
- 鲁棒性：原生预应用失败绝不调用领域提交；领域提交失败必尝试逆向原生变更；回滚失败同时保留 primary 与 rollback 原因；成功路径不会误触发回滚。
- 防回退：架构门禁禁止协调器依赖 Tauri、`State` 或 `AppHandle`；四项纯测试覆盖每条状态路径，Desktop Host 总测试增至 106 项。
- 剩余边界：原生策略事务只覆盖窗口预应用与领域提交；后续子系统收敛由独立 fail-closed 协调器负责，不能把两者合称为完整分布式事务。

## M-2026-07-18 Safe Mode 提交后 Fail-Closed 收敛

- 不足：Safety 已提交 Safe 后，Auto Mode、用户程序、事件会话、Skill、Agent、策略缓存与 Renderer 使用连续 `?`；首个故障会跳过全部后续隔离，产生“界面安全但后台能力仍运行”的严重不一致。
- 修正：新增 Tauri-free `SafeModeConvergenceOperations` 与固定顺序协调器；宿主 Adapter 实现八项隔离/投影步骤，任何失败均继续尝试后续步骤，最后尽力写入 `safe-mode-convergence-failed` Security 诊断。
- 隐私与稳定性：失败结果只保留九个固定步骤码，不保存底层错误、路径或 Secret；诊断写入自身失败也作为固定步骤记录，Safe Mode 领域状态不回滚。
- 证据：纯故障注入覆盖全成功、首项失败后继续、多项失败稳定顺序和底层错误不泄漏；架构门禁拒绝协调器依赖 Tauri，Desktop Host 与 Diagnostics Bundle 测试共同验证接线和稳定事件码。
- 剩余边界：退出 Safe Mode 在领域状态切回 Normal 后仍存在多步恢复失败窗口；必须引入显式 `RecoveryPending/Degraded` 和可重试补偿，不能复用“保持 Safe”的进入语义草率处理。
