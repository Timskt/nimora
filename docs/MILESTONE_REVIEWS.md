# Nimora 里程碑回顾

## 2026-07-19 — Native-size Pet Visual Harness

- 结果：Browser Preview 现在用 260×300 验收框呈现桌宠，不再因标签页尺寸让径向菜单和角色比例产生误导性截图。
- 隔离：纯函数明确区分 Tauri 与 Browser Body Class；原生透明窗口不包含棋盘背景、边框或 Preview 阴影。
- 证据：路由与环境分支测试覆盖三种输入，前端测试增至 76 项，生产构建和 Bundle Budget 通过；浏览器需在热更新后补菜单截图回归。
- 边界：本 Harness 不模拟原生拖拽、透明合成、DPI、置顶和鼠标穿透，相关发布门禁保持不变。

## 2026-07-19 — Native Pet Window Recovery

- 结果：Desktop Host 已对 `pet` 窗口销毁建立有限恢复，重建继续复用统一窗口构造器、持久位置和实时 Presence 策略，桌宠不再因单次 WebView 异常永久消失。
- 鲁棒性：60 秒滚动窗口最多 3 次，1/2/4 秒退避；原子门保证单 Worker，应用退出屏障保证正常退出不被误判为崩溃。
- 降级：预算耗尽后保留 Core、托盘和控制中心，不无限创建窗口；成功/耗尽使用稳定无内容诊断码，不采集桌面或角色内容。
- 证据：纯状态机覆盖退避、预算恢复、并发去重与退出抑制；诊断码序列化契约、Desktop Clippy 和格式门禁通过。
- 剩余：真实 WebView 强杀、GPU Context Loss、macOS/Windows 签名包与 8 小时长稳仍属于发布真机门禁，当前不把纯状态机测试冒充跨平台运行证据。

## 2026-07-19 — macOS Fullscreen Presence Adapter

- 结果：macOS Adapter 已通过 Accessibility `AXFullScreen` 采样前台全屏事实，接入 15 秒续租、指数退避、过期清理和 Desktop Presence Coordinator。
- 隐私与稳定性：只读取布尔属性，不读取窗口标题、描述或内容；每次调用 2 秒硬超时，超时后杀死并回收子进程。
- 并发：新增 Presence Transition 串行门，Profile、三档覆盖、Safe Mode、托盘恢复和后台 Sensor 共用同一可逆窗口事务。
- 可观察性：Desktop Snapshot 暴露版本化 Sensor Health，控制中心明确显示正常、降级、不可用或停止，权限缺失不再静默。
- 证据：Sensor Host 4 项、macOS Adapter 2 项、Desktop Host 153 项和前端 74 项测试通过；生产构建、Clippy、Format 与架构门禁通过。
- 剩余：macOS 屏幕共享、游戏、免打扰和全部 Windows Adapter 尚未实现，不宣称完整跨平台情境感知。

## 2026-07-19 — System Context Sensor Host Contract

- 结果：新增宿主无关 `nimora-system-context-sensor`，统一采样间隔、单次超时、15 秒续租、最高 30 秒指数退避、停止语义和可序列化健康快照。
- 架构裁决：平台 Adapter 只返回布尔事实；调度器不依赖 Tauri 或平台 API，不能操作窗口，后续 macOS/Windows Adapter 必须复用同一控制器。
- 鲁棒性：无成功记录时失败保持 unavailable，有历史成功时降级为 degraded；成功清零失败并续租，停止后不再产生调度。
- 证据：四项纯测试覆盖续租与恢复、退避上限、终止调度和非法租约；Clippy 与 Workspace Format 作为提交门禁。
- 诚实状态：此里程碑完成 Sensor Host 契约，不代表任何平台原生采样已经完成。

## 2026-07-19 — Desktop Presence Coordinator

- 结果：Desktop Host 已将活动 Profile、系统情境、用户覆盖与 Safe Mode 合并为唯一 `PresenceDecision`，并通过既有可逆窗口事务应用；控制中心提供三档可访问呈现设置。
- 安全：命令仅允许控制中心调用；Safe Mode 强制恢复，托盘恢复不再直接操作窗口，屏幕共享隐私仍高于普通“始终显示”。
- 稳定性：Profile 切换与退出 Safe Mode 都重新求值，避免恢复陈旧快照；原生应用或提交失败保持窗口和领域状态一致回滚。
- 证据：前端测试、生产构建、Rust 151 项宿主测试、Clippy、格式与架构边界通过；Browser Preview 已验证三档 radiogroup、状态原因和窄屏单列布局。
- 诚实状态：macOS/Windows Sensor 尚未实现，当前完成的是协调器和用户覆盖，不宣称自动检测全屏、游戏、共享或免打扰。

## M-2026-07-19 系统情境策略领域基础

- 结果：新增宿主无关 `nimora-system-context`，统一全屏、屏幕共享、游戏与免打扰四类有界事实，提供 30 秒租约、倒序拒绝、过期恢复、用户覆盖和 Safe Mode 决策。
- 隐私裁决：屏幕共享优先于普通强制显示；策略输入不允许窗口标题、进程命令行、屏幕像素或会议内容，输出只包含可见性、自主抑制和稳定原因。
- 架构裁决：Sensor 只能产生事实，领域包不依赖 Tauri、Profile、Pet 或持久层；后续 Desktop Coordinator 是唯一窗口执行者，并必须复用可逆原生事务。
- 诚实状态：macOS/Windows 原生 Adapter 与桌面协调器尚未接入，因此矩阵只标记领域策略完成，不宣称系统自动感知可用。
- 证据：5 项领域测试覆盖共享隐私、显式覆盖、Safe Mode、过期/否定信号、超长租约与倒序拒绝；Clippy `-D warnings` 和格式门禁通过。

## M-2026-07-19 桌宠工作陪伴闭环

- 结果：控制中心到桌宠新增有限、版本化的任务陪伴信号，思考、运行、等待确认、完成、失败与取消均有一致动作和低压力气泡反馈，形成桌宠与 Agent 的双向入口和反馈闭环。
- 隔离裁决：信号是可丢弃的 UI 表现，不进入 Core 宠物状态，不携带用户内容，也不使 Agent 或 Provider 成为桌宠运行依赖；发布失败不会改变任务结果。
- 扩展裁决：未来 Auto Mode、Automation 和模块 Agent 只能复用封闭状态枚举，不能发送任意动画或文本；任务真相继续由各自 Runtime 权威持有。
- 恢复裁决：工作态可被后续信号替换，完成、失败和取消在短暂展示后恢复宠物权威状态，避免覆盖睡眠、需求和自主行为。
- Actions：新增纯映射单测并继续本地执行前端全量测试、生产构建和 Bundle 门禁，不增加 GitHub Actions 触发频率。

## M-2026-07-19 OpenAI-compatible Provider Worker 边界复盘

- 结果：外部 Provider 的网络和凭据使用已进入独立进程，复用现有有界 stdin/stdout、超时、取消强杀和稳定错误边界，没有把 HTTPS Client 放入 Desktop、Runtime、Skill 或用户代码。
- 安全裁决：Secret Reference 由任务携带但必须与 Provider 配置精确匹配；明文只由宿主受限 Resolver 解析后单次传给 Worker，命令行、环境变量、响应、错误和日志均无明文通道。
- 兼容裁决：目标是 OpenAI-compatible Chat Completions 当前契约，不宣称支持 Responses API、流式输出或任意 Provider 私有扩展；新增能力必须通过版本化 Worker 协议和契约测试演进。
- 未完成边界：当前没有配置持久化、系统密钥 UI、运行时注册和生产 Sidecar 发现，因此不得在产品状态中显示可用。下一纵切必须完成配置、密钥、状态探测和 Desktop 管理闭环。

## M-2026-07-18 AI 核心场景 Profile Creator 完整纵切

- 结果：Creator Studio 新增第五类真实产物 `profile`，外接 AI 可生成既有场景模式及窗口、穿透、声音和主动频率覆盖，并走严格解析、独立审查、一次性批准、Workspace 保存和事务化创建完整链路。
- 身份边界：AI 不提供运行时 ID；草案目录使用内容摘要，生产 Profile UUID 只由 `ProfileService` 生成。创建不调用 `switch_active`，因此不会静默改变当前窗口和声音策略。
- 证据：Creator Contract 覆盖合法 Profile 与越界频率拒绝；Workspace 覆盖摘要目录和双文件原子保存；Desktop 真实 SQLite 测试覆盖创建持久化、UUID 收据和活动 Profile 不变；前端覆盖策略预览与不切换提示。
- 架构裁决：这是已有 Core Profile Runtime 的配置产物，不是拥有 Provider、Tool、Memory 或委派能力的 Agent Profile；后者继续保持未实现，不能因名称相近而误报。
- UI：普通用户看到直观开关与频率，开发者仍可查看结构化 JSON；创建按钮、完成状态和收据统一使用“创建/未切换”，不复用可执行扩展的“授权/启用”语义。
- 离线与稳定性：生成步骤可使用本地 Provider；审查、保存、创建和后续切换全部离线可用。SQLite 保存失败时领域状态与事件均不发布。
- Actions：本纵切继续使用本地全量门禁，未增加高频 GitHub Actions 触发。
- 下一缺口：Agent Profile 需要独立身份、Provider/Model、Tool Scope、Memory、预算、委派和停用生命周期；Connector 与声明式 Widget 仍缺生产 Runtime，不能直接注册 Creator Handler。

> 更新日期：2026-07-18  
> 状态：持续维护  
> 目的：在可演示纵切、每五个生产提交、公开契约变化或重大偏差时校准方向

## M-2026-07-18 AI Theme Creator 完整纵切

- 结果：Creator Studio 新增第四类真实产物 `theme`，外接 AI 可生成本地主题元数据与固定视觉 Token，并走生成、严格解析、独立审查、一次性批准、Workspace 保存和原子安装完整链路。
- 复用：宿主把 Theme 转换为标准 `nimora.asset/1` 包，生成 Manifest、Descriptor 和 SHA-256 Inventory，再调用既有 Asset Installer；没有新增文件写入、安装或激活旁路。
- 美学与可访问性：只接受九个语义 Token、`#RRGGBB/#RRGGBBAA`、深浅模式、三档圆角与动效策略；正文、弱文本、强调、成功和危险色必须满足既有 WCAG 门禁。
- 安全：身份限定 `theme.local.*`，安装不会覆盖发布者命名空间、不会自动激活，也不继承 Creator Agent 权限；许可证是用户声明，不被模型文字冒充权属证明。
- 证据：Asset Installer 覆盖生成包安装与命名空间拒绝，Creator Contract 覆盖合法主题和低对比度拒绝，Desktop 覆盖已安装版本基线与空权限 Diff，前端类型覆盖第四类产物。
- 架构裁决：优先接入已有完整 Runtime 的 Theme，而不为尚无生产 Runtime 的 Connector/Agent Profile 制造假安装闭环；后两者必须先建立独立生命周期宿主，再注册到统一 Creator 产物面。

## M-2026-07-18 Capability Proposal 待评审队列纵切

- 结果：用户可将宿主双重核验且明确需要平台扩展的 Gap 提交为版本化 Capability Proposal，原子进入所选 Workspace 的独立待评审队列。
- 安全：提交命令实时重建 Catalog 与 Semantic Graph 并重算计划；Proposal 不携带批准、执行、Handler、Grant 或 Registry 修改能力。
- UI：Gap 中新增独立“提交平台能力提案”动作和待评审反馈，明确它不会自动实现能力；保存报告与提交提案互不冒充。
- 证据：持久层覆盖状态、契约、无执行字段、无需提案时拒绝和原子写入；前端命令映射与不可执行文案进入回归测试。
- 后续结果：维护者队列浏览、单向终态裁决与理由审计已在下一条治理纵切实现；去重聚类、优先级和实现项目关联仍未实现。

## M-2026-07-18 Capability Proposal 维护者治理纵切

- 结果：维护者可从选定 Workspace 读取完整队列，查看精确与语义缺口、候选路径、内容摘要和历史裁决，并将待评审项单向裁决为接受分析、拒绝或重复。
- 安全：读取逐条重验严格契约、Gap 双计划、文件名绑定、普通文件、1 MiB 上限和 SHA-256 内容一致性；裁决理由严格限制，状态只能从 `pending-review` 进入不可逆终态，Safe/Recovery Mode 禁止写入。
- 边界：摘要只检测意外或离线内容篡改，不是维护者身份签名；`accepted` 只进入可行性分析，不创建 Handler、不修改 Registry、不授予 Grant、不执行代码。
- UI：Creator Studio 增加独立维护者治理区、Workspace 切换/刷新、完整缺口证据、终态历史和显式不可逆边界，Browser Preview 保持 `desktop-host-required`。
- 证据：Rust 覆盖提交、读取、一次性裁决、摘要篡改、非法理由、重命名文件和符号链接失败关闭；TypeScript 覆盖两条专用 IPC 映射，前端类型检查覆盖治理组件。
- 后续结果：确定性去重聚类、重复目标绑定和基于出现次数的分诊信号已在下一条纵切实现；维护者身份签名、业务影响评分、实现项目关联、多人远程协作和导出仍未实现。

## M-2026-07-18 Capability Proposal 确定性聚类与分诊纵切

- 结果：宿主以排序后的精确缺口 ID 与语义缺失输出生成稳定聚类键；同簇最早提交记录成为规范提案，队列按同类出现次数优先展示。
- 可信语义：`normal/elevated/high` 只对应 1、2–3、4+ 次同类提交，是可复算的需求重复信号，不代表模型判断的用户价值、商业优先级、安全影响或实现成本。
- 重复裁决：`duplicate` 必须结构化绑定一个已存在、非自身、同聚类的 `duplicateOfProposalId`；规范提案不能在 UI 中指向自身，接受与拒绝状态禁止携带重复目标。
- 完整性：队列读取除逐文件摘要与双计划复验外，还复验所有重复引用仍存在且聚类一致；删除规范记录或制造跨簇引用会使整个队列失败关闭，不展示不完整治理事实。
- UI：维护者可看到同类数量、分诊等级、规范提案、聚类摘要和持久重复目标；标题或自然语言摘要变化不会干扰确定性聚类。
- 证据：Rust 覆盖改标题同簇、规范记录选择、分诊阈值、合法重复绑定、自指/缺失/跨簇目标拒绝和悬空引用检测；TypeScript IPC 测试覆盖结构化目标映射。
- 下一缺口：真实业务影响与实现成本需要独立受信输入和维护者证据，不能从提交次数推断；仍需项目关联、签名身份、聚类拆分/合并治理和远程协作。

## M-2026-07-18 Creator Semantic Gap Verification 纵切

- 结果：Gap 增加严格语义输入/输出候选，Creator 同时接收受信 Catalog 与实现无关 Semantic Graph；模型不能提交宿主前置事实。
- 裁决：宿主以固定安全策略运行确定性 Planner；当前图若能完全产生目标输出则拒绝 Gap，未解析结果才与 Exact-ID 证据一同返回。
- 持久化：保存命令重新构建实时 Catalog 与 Graph、重算双计划，报告升级为 `nimora.persisted-capability-gap/2`，不接受前端旧摘要或旧计划。
- UI：展示候选语义、能力路径、成本、搜索状态与缺失输出，并明确证据范围不等于自然语言理解绝对完备。
- 复盘：外接 AI 现在可以发现已有能力组合而不只生成新代码，同时仍不能扩权、伪造前置条件或获得模块原生对象。

## M-2026-07-18 Semantic Composition Graph 基础纵切

- 实现：共享 Semantic Contract、摘要绑定 Graph、确定性有界最低成本搜索、13 个内建 Tool 显式契约、Skill 可选语义 Contribution 与 Desktop 实时合并已贯通。
- 安全：图只读取宿主复验事实，不执行 Tool；第三方输出必须处于 Skill 命名空间，ID/effect 必须与可执行 Contribution 一致，暂停即撤销。
- 鲁棒性：节点、深度、状态、请求和成本均有硬上限；前置条件、数据等级、副作用和离线约束均参与路径准入，摘要篡改失败关闭。
- UI：Creator Gap 同时展示 Catalog Snapshot 与 Semantic Graph 摘要，明确图已绑定但自然语言到语义输出的映射尚未核验。
- 证据：基础契约 3 项、组合器 6 项、Agent Tool 一致性 1 项、Skill Runtime 8 项、Desktop 动态撤销定向测试和前端 42 项测试通过；全工作区 Clippy、架构、TypeScript 与构建门禁通过。
- 剩余：Gap 严格语义候选字段、宿主执行 Graph Plan、持久报告语义证据与 UI 路径可视化尚未形成下一完整纵切。

## M-2026-07-18 外接 AI 能力面再审

- 发现：现有能力工厂覆盖“生成扩展”，但对“生成用户专属能力 API、能力虚拟化、跨设备交接、合成环境和可解释层”的产品表达不够显式。
- 修正：能力目录新增 Capability Facade、Virtualization Profile、Temporal Graph、Handoff Profile、Simulation World、Explanation Pack 与 Interaction Mapping，并定义调用其它模块的正式路径。
- 架构边界：生成物只能引用实时 Registry 中经过验证的语义能力；AI 不得从自然语言或 JSON Schema 猜语义，不得获得模块内部对象，也不得创建新的权限或信任根。
- 鲁棒性：动态 Contribution 撤权或失效后组合节点立即撤销，已保存组合进入降级态；替代路径若扩大权限、数据范围或副作用则禁止自动切换。
- 测试：功能计划增加组合图、权限保守并集、虚拟化降级、仿真零生产出口、解释事实复验和多输入映射测试；这些是目标验收，当前不得标记为已实现。
- 后续结果：版本化 Semantic Contract 与有界确定性 Composition Graph 已在本文件后续里程碑完成；严格自然语言语义候选映射仍是下一缺口。

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

## M-2026-07-18 外接 AI 原生扩展能力面复核

- 发现：原有 Creator 已覆盖 Program、Skill、Automation 和通用能力工厂，但对感知、个人 API、策略编译、实验、数字孪生、本地分叉与完整退役的产品边界仍分散。
- 裁决：新增十六类能力面，全部复用四级扩展模型、`CreatorArtifact`、可信 `ArtifactHandler` 与 Capability Gateway；不为新产物建立权限或安装旁路。
- 安全：补充多模态持续同意、Agent Team 权限不继承、模型不可修改自身评分标准、仿真与生产物理隔离、AI 不持有签名密钥和数字遗产撤权收据。
- 产品：Creator Studio 从单纯生成器升级为“发现—设计—构建—验证—授权—安装—观察—维护—退役”工作台，并为普通用户、二次元创作者、极客、开发者和团队提供同能力的渐进视图。
- 实施边界：本里程碑只建立正式工程基线；下一步需逐个实现通用 Gap、Composition、Artifact Handler 和 Simulation 纵切，不能将文档目录误报为可运行功能。

## M-2026-07-18 Creator Capability Gap 纵切

- 问题：原 Creator 只能接受三类可安装 Draft；Registry 无法表达目标时只能报通用解析错误，模型容易被诱导发明能力或让用户误以为生成失败等于需求无效。
- 裁决：Draft Schema 保持不变，新增并列且不可执行的 `nimora.capability-gap/1`；宿主响应显式互斥，Gap 不能复用任何批准、保存 Draft 或安装通道。
- 持久事实：Gap 可在用户选择的 Workspace 中原子保存为唯一 UUID 报告，保留换模型接力和未来平台提案依据；不需要运行授权，也不包含源码或 Secret。
- UI：缺失能力、必要操作、替代方案和平台提案需求使用警示语义展示，明确说明不能执行；仅提供“保存缺口报告”。
- 证据：Creator Contract 8 项、Desktop Host 126 项、Frontend 42 项测试覆盖严格解析、重复/非法能力、原子保存、符号链接、IPC 映射和无批准安装入口。
- 下一缺口：当前 Gap 仍由无 Tool Creator 模型根据受信指令提出，尚未用最小 Catalog Slice 做确定性存在性核验；下一纵切必须引入 Catalog Snapshot 与 Composition Planner，不能把模型判断升级为宿主事实。

## M-2026-07-18 Creator Catalog Snapshot 与精确组合核验

- 问题：严格 Gap Schema 只能约束模型输出形状，不能证明模型声称缺失的能力确实不在当前生产 Registry；保存的报告也缺少可复验目录基线。
- 裁决：不建立第二套可执行 Registry。新增纯投影层，从桌面生产 Tool Registry（含当前激活 Skill Contribution）只提取已复验 ID 与 effect，第三方标题/描述和实现对象均不进入受信提示；快照稳定摘要后由纯函数 Planner 做 Exact-ID 集合判定。
- 信任边界：Snapshot 进入受信 System Message；Provider 无目录查询 Tool。宿主解析 Gap 后重新匹配，任何已注册 ID 被声称缺失都失败关闭；前端只接收摘要与计划，不参与判定。
- 持久化：保存时再次读取实时 Registry 并重算计划，原子文件升级为 `nimora.persisted-capability-gap/1`，同时保留 Gap、目录摘要和 missing/resolved 证据。
- UI：新增“宿主目录已核验”证据卡和摘要，但同时明确“未证明自然语言目标不存在其他组合路径”，避免将精确集合判断包装成语义完备性。
- 证据：`creator-composition` 3 项覆盖稳定快照、精确分离和篡改/重复拒绝；Desktop 新增 3 项覆盖 Skill 动态目录、矛盾 Gap 拒绝与真实缺失证明；Frontend 保持 42 项并增加核验语义断言。
- 下一缺口：Exact-ID Planner 不理解能力输入输出、前置条件、数据分类、授权范围和成本，不能发现多步转换路径；下一阶段应建立只读 Composition Graph 与有界搜索，而不是让模型自行宣称可组合。

## M-2026-07-18 AI 辅助扩展草案

- 目标：让外接 AI 帮用户创建 User Program、Skill 与 Automation，同时不继承普通 Agent 的生产 Tool 权限。
- 实现：新增 `nimora.creator-draft/1` 严格契约、专用无工具 Draft Agent 和独立桌面创作工作区；可信规则、用户需求和模型输出分级处理。
- 防线：路径与文件预算、权限解释精确匹配、生产 Manifest/Policy/Engine 复用、Safe/Recovery 门禁，以及“尚未安装”显式状态。
- 未闭环：本里程碑不写盘、不安装、不启用；Workspace、静态检查、沙箱测试、Diff 批准、监控修复和回滚必须作为后续真实纵切完成。

## M-2026-07-18 Creator Workspace 原子草案保存

- 修正：已验证草案可由用户显式选择 Workspace 保存，不再依赖复制模型文本或绕过契约。
- 鲁棒性：保存 IPC 重新执行完整生产校验；Writer 只创建 `.nimora-drafts/<artifact-id>`，拒绝覆盖、符号链接和非目录根，失败清理 staging。
- 边界：保存结果明确保持“尚未安装”，不授予 Capability、不启用、不发布；静态检查、沙箱、Diff 批准和安装仍是后续强制阶段。

## M-2026-07-18 Creator 独立草案检查

- 修正：User Program 与 Skill 不再借正常执行路径验证源码；新增显式 `Validate/Validated` Worker 协议，只解析 JavaScript，不运行顶层代码、不注入 SDK 或生产能力。
- 纵深防御：桌面返回 `nimora.creator-draft-check/1` 逐文件报告；UI 仅在通过后开放保存，保存 IPC 仍重新执行生产契约与独立 Worker 检查。
- 契约边界：Skill Validate 校验协议、Execution ID 与 Manifest，但安装前不要求 Active Lease；Skill Run 的 Lease 与 Activation Event 约束保持不变。Automation 继续复用生产 Engine 校验。
- 范围校准：外接 AI 的完整产品范围扩展为 User Program、Skill、Automation、Connector、Agent、角色资产、声明式 UI、测试、迁移与运维能力工厂；当前仅前三类草案生成和前两类 parse-only 检查已实现，不能误报完整流水线完成。
- 后续硬门禁：沙箱行为测试、权限与行为 Diff、风险批准、原子安装启用、运行监控、AI 最小修复和回滚。

## M-2026-07-18 Creator 无副作用行为沙箱

- 实现：User Program 新增版本化 `Sandbox/Sandboxed` Worker 协议，顶层代码在独立进程执行但不要求 JSON 返回值；Skill 使用临时内存 Lease 通过正常 Worker Admission，只收集 Command/Agent Task 计划；Automation 使用生产 DryRun Backend。
- 安全：三类行为检查均不调用真实 Capability Gateway、Provider 或模块 Backend，不获得 Node、Tauri、文件、网络和数据库对象；保存 IPC 在写盘前重新执行语法与行为两阶段审查。
- 工程修正：按 Automation、User Program、Skill 拆分桌面审查服务，避免继续膨胀 Tauri 编排函数；严格 Clippy 阻止回退。
- 剩余边界：当前只使用空输入、首个 Skill Activation Event 和单个 Automation 事件；多事件夹具、虚拟时间、Connector Mock、调用参数 Diff、风险批准与安装仍未完成。

## M-2026-07-18 Automation 参数绑定运行期批准

- 修正：Automation 安装批准不再被视为真实运行授权；生产 Engine 先做零副作用匹配判定，再用宿主策略整批解析主动作与补偿动作的有效风险。
- 实现：Safe/Low 立即执行；Medium/High 使用五分钟不可变 SQLite Approval Journal，返回 `waiting_for_approval`，批准前不创建 Run Journal、不注册活跃运行、不调用 Pet 或 Agent Backend。
- 复验：批准原子 claim 后重新校验定义、事件身份、完整参数、风险摘要和已安装精确版本，并使用同一预分配 `runId` 执行；Critical 不进入普通批准。
- 鲁棒性：拒绝、过期、重复处理和 Safe Mode 均失败关闭；重启保留未过期 pending、将 executing 标记 interrupted；Engine 异常精确中断 Run Journal，避免遗留 running。
- 回顾修正：故障测试发现 Catalog 的独立 `enabled` 列没有同步回运行 Definition，导致已启用事件 Automation 被 Engine 当作禁用并跳过；读取边界现以持久启用态覆盖 Definition，且测试证明待批期间停用或升级会使旧批准失败终结，Safe Mode 则在 claim 前拒绝并保留用户决定权。
- UI：Automation Workspace 显示逐动作有效风险、实际参数和过期时间，明确整次执行尚未开始；Browser Preview 只返回空目录并拒绝伪造批准。

## M-2026-07-18 Auto Mode 未决 Attempt 人工对账收敛

- 回顾：持久决议仓储、桌面详情/决议 IPC 与目标控制中心 UI 已形成同一纵切，不再把人工对账错误列为未实现。
- UI：仅对 `indeterminate` Attempt 显示两种互斥决议；弹窗展示 Goal、Session、Attempt 和永久审计风险，理由必填，提交后刷新持久事实与不可变历史。
- 安全：决议精确绑定 Session、Attempt、Checkpoint sequence、请求指纹、Actor、理由、决策和宿主时间；Safe/Recovery Mode 与 Browser Preview 全部只读，失败不自动重试。
- 证据：真实 SQLite 覆盖两种决议、并发唯一胜者、重放/漂移/损坏拒绝与重启审计；Desktop Host 覆盖空理由前置拒绝，前端平台测试覆盖完整 IPC 参数映射。
- 剩余：仍需真实桌面跨崩溃场景的交互录像与视觉验收，以及 Goal/Plan 完整编辑和缓存系统密钥保护。

## M-2026-07-18 Automation 持久运行与费用治理

- 目标：让 Automation 的并发、冷却与 AI 当日费用在多入口、并发连接和桌面重启后仍保持同一确定性约束。
- 架构裁决：用户批准与资源准入分离；等待参数批准不占运行租约，claim 后仍须在任何 Journal、活跃状态或 Backend 副作用前原子准入。
- 费用裁决：最大费用只用于预留，累计 Provider Usage 才是实际费用；等待确认保留预留，未知结果进入 `indeterminate`，不得按零自动释放。
- 恢复：启动清理旧宿主租约并保留冷却，将未结算费用预留收敛为未知终态；相同结算允许幂等重放，金额漂移失败关闭。
- 可观测性：新增隐私安全 Governance Catalog，把 Catalog 策略与持久租约/费用聚合成同一只读事实；桌面展示并发、冷却、预留、结算、未知费用和可用预算，并区分三类资源拒绝原因。
- 证据：SQLite Governance 9 项测试覆盖跨连接并发、冷却重启、费用竞争、未知费用与聚合快照；Desktop Host 125 项测试覆盖生产编排和 IPC 投影，前端 40 项测试覆盖 Browser Preview 空态与原生命令映射。
- 剩余：未知费用人工对账与不可变决议审计仍未形成完整桌面纵切；在实现与视觉验收前不得误报费用争议处理完成。

## M-2026-07-18 外接 AI 能力面再审计

- 回顾：既有 Creator、能力目录与 AI-native 文档已覆盖代码、自动化、资产、Agent、数据和运维，但浏览器协作、空间计算、数字身份、受控交易、长期时序规划、群体评测、模型蒸馏与跨设备接力缺少统一产物和宿主边界。
- 修正：能力面扩展为 32 类；新增 8 类均定义标准产物、权限边界、离线/撤销/漂移行为和确定性测试，不以“AI 能做到”替代 Runtime 与 Validator。
- 安全：密码、验证码、私钥、支付凭据和最终法律/金融确认不能交给模型；跨设备、联邦评测、XR 感知与个人训练统一采用最小数据、逐端撤销和删除传播。
- 状态：以上新增能力均为目标架构，尚未形成生产纵切；Creator 遇到对应请求必须返回 Capability Gap，不得生成伪可用包或调用私有 IPC。

## M-2026-07-19 统一 Secret Store 核心纵切

- 触发：OpenAI-compatible Provider 需要 API Key，但项目没有安全凭据后端；直接把 Key 放入配置、IPC 或环境变量会违反安全红线。
- 实现：新增 `nimora-secret-store`，统一严格 `secret:<domain>:<name>` 引用、macOS Keychain、Windows Credential Manager、Linux Secret Service、自动清零读取、幂等删除和确定性内存后端。
- 边界：当前只完成核心存储层，不宣称网络 Provider 已支持凭据；桌面设置、Provider 配置、Worker 单次传递、缓存加密和授权签名必须分别形成后续纵切。
- Actions：新增依赖和核心测试先在本地验证；保持完整纵切后单次推送，不增加高频远端触发。

## M-2026-07-19 控制中心工作区分包治理

- 问题：桌面入口已达 348,284/350,000 bytes，继续增加正式能力会突破性能门禁；提高预算会把架构问题延后并拖慢桌宠首屏。
- 实现：保留概览与 Profile 首屏，把角色、Agent、自动化、AI 扩展和数据保护拆成五个直接动态入口；桌宠 Overlay 仍不依赖这些控制中心工作区。
- 恢复：统一加载壳具备状态语义、减少视觉跳变；独立 Error Boundary 将 Chunk 故障限制在当前工作区，并提供无需重启应用的重新加载动作。
- 预算：入口降至 262,845 bytes，释放约 85 KB；五个工作区各自设置 50 KB 原始图预算，原有 GLTF/VRM 预算不放宽。
- 验证：Frontend 61 项测试、TypeScript、Vite 构建与 Bundle Budget 通过；浏览器逐项进入 Agent 和设置，结构与完整截图无视觉回归。

## M-2026-07-19 Profile 级低压力照料

- 回顾：四项生命值让 Nimora 更接近经典 QQ 宠物式桌面生命体，但缺少用户可控照料强度会形成新的强制负担，因此先于物品和商店补齐基础策略。
- 架构：Profile 声明用户选择，纯领域层执行生命演化，Desktop Host 动态映射；Renderer、AI、Program 和 Skill 不拥有生命时间权威。
- 鲁棒性：关闭模式不修改生命值但原子推进时间基线，重新启用不追赶关闭期；旧 Profile 不重置已有状态。
- 体验：三档均不死亡、不倒扣关系，且不禁用主动照料；断网、关闭 Provider 和关闭控制中心仍可运行。
- 验证：核心、应用、Schema 与前端聚焦测试共 140 项通过；原生窗口视觉与重启持久化验收纳入后续真机门禁。

## M-2026-07-19 陪伴纪念收藏基础

- 决策：先建立离线资产所有权和原子收藏契约，再扩展背包、服装、房间或商店，避免 UI 商店先行形成不可迁移的历史包袱。
- 领域：首批四项纪念只由真实陪伴点解锁，稳定标识与本地化展示分离，跨阈值补齐且集合拒绝重复和乱序。
- 数据：收藏嵌入版本化 Pet 快照，与互动和 Event 原子保存；旧数据迁移为空并在下一次合法互动按有效成长补齐。
- 体验：关系卡用低噪声标签呈现拥有事实，没有倒计时、过期、付费赎回或联网依赖。
- 证据：核心、应用、SQLite、Schema、前端测试和生产构建通过；浏览器扩展跨会话 Tab 故障已记录，视觉截图不得误报完成。

## M-2026-07-19 离线随身背包与道具闭环

- 回顾：经典 QQ 宠物式桌面生命体需要本地拥有和可触达的照料物品，但不能先做联网商店或只有外观的 UI 壳。
- 领域：建立稳定物品标识、一次性 Starter Pack、排序唯一库存、独立冷却、耗尽语义和三种有界效果；已有免费照料保持不变。
- 原子性：候选状态先变更并校验，再以 Command/Event 与 Snapshot 一次持久化；任何拒绝或保存失败都不产生库存、生命值、冷却或事件副作用。
- 边界：资产离线、不过期且不可被 Provider 回收；未来 Reward/Creator/Store 只能经 Grant 契约增加所有权，AI/Program/Skill 不能直接写库存。
- 体验：控制中心用真实背包替换占位操作，呈现效果、数量、禁用态和无焦虑空态；Overlay 仍是独立原生桌面生命体，Browser Preview 仅作视觉模拟。

## M-2026-07-19 桌宠本体背包入口

- 偏差修正：背包只在控制中心可用会把桌宠退化成网页控制面板的附属物，不符合经典 QQ 宠物式独立桌面生命体定位。
- 实现：复用长按/右键轻量菜单增加背包总数和二级页，直接调用既有宿主命令；控制中心、Overlay 与领域层共享身份而不复制库存权威。
- 交互：260×300 窗口内完成主菜单、背包、使用与返回；Esc 分层退回，焦点确定，空态无焦虑，不新增窗口或网络依赖。
- 证据：Browser Preview 实际右键、视觉截图、道具扣减和 Esc 返回通过；共享展示与总数计算有自动化覆盖，原生多平台 DPI 继续作为发布门禁。

## M-2026-07-19 持久宠物身份与双入口改名

- 定位：经典 QQ 宠物式桌面生命体必须拥有用户可建立感情的持久名字，不能把内置资源名 `Aster` 当作永久身份。
- 架构：名称归属 Pet Identity；领域校验、应用 Command/Event、Repository 与窗口同步形成完整纵切，Renderer、Profile、AI 和页面均不成为身份权威。
- 鲁棒性：trim 后 1–64 Unicode scalar values；非法输入、Safe/Recovery Mode 与保存失败全部零副作用，改名不重置任何成长或资产。
- 体验：控制中心关系卡和桌宠本体菜单均使用内联表单，动态反馈及 ARIA 同步真实名称，断网和 Provider 禁用不影响使用。
- 证据：Rust 领域/应用原子测试与前端平台/规范化测试通过；生产构建、原生宿主、Clippy、格式和浏览器视觉继续作为本里程碑提交门禁。

## M-2026-07-19 持久桌宠家位置

- 偏差修正：规范已有“回家”，但实现只有当前窗口位置；拖拽和漫游会覆盖位置，因此无法表达稳定归属感。
- 领域：新增独立 `homePosition`，旧快照以最后位置迁移；当前落点、家锚点和角色资产身份互不混淆。
- 原子性：设置家与返回家拥有独立 Command/Event；原生窗口先移动、Repository 后提交，失败执行窗口补偿回滚。
- 多屏：复用最大重叠、主屏回退和安全边距算法，显示器拓扑变化不把桌宠送到屏外，也不篡改用户原始家锚点。
- 体验：桌宠本体菜单直接提供“回家”和“这里设为家”，完全离线，不要求控制中心或 AI 在线。

## M-2026-07-19 原生桌宠身份一致性

- 偏差修正：持久改名已覆盖 Web UI，但原生桌宠窗口仍硬编码内置资源名，会让窗口管理器和辅助技术看到错误身份。
- 架构：领域层公开唯一名称归一化入口，创建、改名和宿主标题共享同一规则；启动窗口直接读取持久 Snapshot，不复制身份权威。
- 原子性：改名先设置原生标题，再原子提交 Snapshot/Event；提交失败自动回滚旧标题，双重失败保留两个原因，通知仅在完整成功后发送。
- 边界：非法输入、Safe/Recovery Mode 或桌宠窗口缺失均在持久写入前失败；身份同步完全离线，不依赖控制中心、Provider、Renderer 或角色资源包。

## M-2026-07-19 Presentation Profile 原生降扰

- 偏差修正：Presentation 原先只抑制自主动作，桌宠仍置顶遮挡演示或直播；UI 还错误声称隐藏场景可继续手动互动。
- 架构：`WindowPolicy` 纳入可见性，Profile 切换统一协调可见、置顶、穿透和持久活动项；未来环境传感器只能提交策略意图，不直接操作窗口。
- 鲁棒性：启动阶段直接按活动 Profile 创建隐藏窗口，避免闪烁；原生应用或持久提交失败执行完整反向补偿，隐藏期间自主循环不做屏幕恢复。
- 恢复：Safe Mode 始终可见；托盘恢复是显式用户覆盖并记录前后可见性，断网、Provider 禁用和控制中心关闭不影响策略。
- 体验：创建表单与 Profile 摘要明确标示“桌宠隐藏”；Chrome Preview 在实际控制中心宽度完成语义树和截图验收，无裁切或误导。

## M-2026-07-19 权威关系阶段

- 偏差修正：成长等级原先由 Renderer 复制 Core 公式，且没有稳定关系称谓，长期演进会产生宿主与界面不一致。
- 架构：Core 从有效 `BondPoints` 派生五个稳定阶段及下一阶段投影；阶段不重复持久化，Tauri Snapshot 和共享 Schema 只传输权威结果。
- 兼容：旧数据继续以 `Affinity` 作为迁移基线；边界覆盖至 JSON 安全最大值，离线、无 AI 和控制中心重启不改变关系结果。
- 体验：关系卡以“阶段 · 等级”建立情感身份，用自然陪伴文案替代签到、倒计时和惩罚式催促；最高阶段仍可持续成长。

## M-2026-07-19 可访问径向宠物菜单

- 偏差修正：长按与右键虽然已直达完整能力，但九项纵向列表缺少经典桌宠的空间感，也挤压 260×300 窗口。
- 交互：六个高频动作形成环绕宠物的轮盘，低频系统操作进入同窗口“更多”页；背包、改名、家位置、休息和穿透能力零删减。
- 无障碍：DOM 顺序保持稳定，方向键双向循环并支持 Home/End，Esc 逐级返回；减少动态效果时关闭轮盘入场动画。
- 工程：键盘索引算法独立为纯函数并覆盖环绕边界，Renderer 与库存权威边界保持不变。

## M-2026-07-19 桌宠控制中心安全深链

- 偏差修正：桌宠菜单能完成本地照料，却不能直接进入聊天、Agent 任务和设置，桌宠仍不是复杂能力的第一入口。
- 架构：新增三项封闭目标枚举；Pet Window 只提交意图，Tauri Host 负责窗口恢复与定向事件，控制中心映射到注册导航，拒绝任意 URL。
- 鲁棒性：原生命令限制调用窗口，窗口恢复成功后才发布导航；托盘与桌宠来源分别进入审计数据，避免可观测性失真。
- 体验：聊天、任务与设置进入“更多”页，保留六向高频轮盘的清晰度；Browser Preview 使用同一查询目标白名单完成独立验收。
