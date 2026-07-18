# Nimora 全量实现状态与证据矩阵

## 2026-07-18 — Creator Capability Gap 真实纵切

- Creator 模型输出新增并列的严格 `nimora.capability-gap/1` 契约；目标无法由当前 Registry 表达时，模型只能返回缺失能力、所需操作、最低替代和平台提案需求，不能发明 Command、API 或可执行回退代码。
- `creator-draft` 在同一有界 JSON 信任边界解析 Draft 或 Gap，验证字段白名单、文本预算、能力命名、数量、重复项和替代方案；Gap 永远不能进入 Draft 的检查、批准或安装函数。
- Desktop Host 使用 `outcome`、互斥 `draft/capabilityGap` 投影；Creator Studio 为 Gap 提供独立警示界面，不渲染权限批准、原子安装或 Draft Workspace 保存入口。
- 用户可将经过复验的 Gap 原子保存为 `.nimora-drafts/capability-gap-<uuid>.json` 项目事实；报告只有结构化数据，无源码、运行 Grant、Secret 或宿主路径回传，Safe/Recovery 下仍可导出恢复资料。
- Creator Contract 8 项、Desktop Host 126 项、Frontend 42 项测试通过；当前尚未实现基于真实 Capability Catalog 的确定性 Composition Planner，也未实现 Gap 到 L4 Proposal 的评审工作流。

## 2026-07-18 — 外接 AI 原生扩展能力面基线

- 新增多模态感知、个人 API、个人数据应用、语义映射、协议适配、示教、Agent Team、上下文工程、模型路由、策略编译、数字孪生、实验优化、资产流水线、生态迁移、协作发布与数字遗产共十六类目标能力面。
- 所有新增产物继续归一到 `CreatorArtifact` 与可信 `ArtifactHandler` 生命周期；Creator AI 只能使用受控 Builder Tool 生成项目或 Patch，不获得 Node、Tauri、文件、数据库、网络、Provider 或 Secret 原生对象。
- 文档补充了跨能力组合、渐进式 Creator UI、离线与 Safe Mode、预算、取消、未知副作用和完整退役测试门禁。
- 本条是产品与架构基线，不是功能完成声明；通用 Capability Gap、Composition Planner、CreatorArtifact Handler 和 Simulation Pack 尚未形成真实代码纵切。

## 2026-07-18 — Automation 资源与 AI 费用治理可观测纵切

- SQLite Governance 新增按 Automation 和 UTC 日桶聚合的隐私安全快照，只返回活跃租约、最近启动时间及 reserved/settled/indeterminate 费用，不暴露任务内容、事件正文或 Provider 数据。
- 桌面只读 IPC 将当前 Catalog 策略与持久治理事实合并，确定性计算冷却剩余、并发占用和当日可用预算；Browser Preview 返回带真实 Schema 的空态，不伪造本机账本。
- Automation Workspace 新增资源治理卡片，分别展示并发、冷却、已结算、执行中预留、未知费用占用和可用预算；未知费用以显著警告说明不会自动按零释放。
- 用户可看到并发、冷却和预算拒绝的本地化原因，运行历史优先显示生产 Engine 的终态原因；SQLite 9 项治理测试、Desktop 125 项宿主测试和前端 40 项测试通过。
- 当前仍缺少未知费用的人工对账操作和不可变决议审计，不得把只读健康展示误报为完整费用争议处理。

## 2026-07-18 — Automation 持久并发、冷却与 AI 费用治理

- Automation 策略新增 `maxConcurrentRuns`、`cooldownMs` 与 `dailyCostBudgetMicrounits`，生产校验和 Creator/桌面契约使用同一硬上限。
- 真实运行在 Run Journal、活跃运行和 Backend 之前通过 SQLite Immediate 事务获取租约；并发上限与冷却按 Automation 隔离，批准等待阶段不占租约。
- Agent 子任务在 Journal 与 Provider 前按 `maxCostMicrounits` 原子预留当日预算；预留不是实际费用，Completed 只用累计 `AgentTask.usage.costMicrounits` 结算。
- Tool Confirmation 等待期间保留预留；Provider 失败或进程重启导致费用未知时标记 `indeterminate` 并继续占用完整当日预算，禁止按零费用自动重试。
- 运行租约在正常、Engine 拒绝和 Journal 失败后尽力释放；启动恢复释放旧进程租约、保留冷却状态，并把所有未结算费用预留收敛为未知终态。
- SQLite 测试覆盖跨连接并发唯一胜者、冷却边界与重启、费用竞争、真实结算、超预留拒绝和未知费用不释放；Desktop 124 项宿主测试通过。

## 2026-07-18 — 外接 AI 能力开发平台目标契约

- 新增外接 AI 四级扩展模型：配置、组合、SDK 扩展和平台能力提案；AI 必须选择最低充分复杂度，不能通过自建 Handler 扩权。
- 定义 Provider 无关 Builder API、结构化 `CapabilityGap`、多模型项目接力、确定性证据链、Canary 运营与完整退役协议。
- 补充语义桥接、个人数据应用、模型/协议适配、上下文工程、仿真、形式验证、隐私增强、资源治理、协作交接和数字遗产等可由用户委托 AI 创建的能力族。
- 当前实现仍仅覆盖 User Program、Skill 和 Automation 的部分 Creator 纵切；通用 Builder Tool、Gap 提案、Connector/Agent/UI/数据应用生成和持续运营闭环均保持未实现，不得误报。

## 2026-07-18 — Automation 参数绑定的运行期批准

- Automation 主动作与补偿动作在任何副作用前整批预检，统一采用宿主注册表计算的有效风险；未知命令失败关闭，Critical 不进入普通批准链路。
- Safe/Low 立即运行；任一 Medium/High 会生成五分钟、一次性、SQLite 持久化的不可变批准计划，并进入 `waiting_for_approval`，此时不创建 Run Journal、不注册活跃运行，也不调用 Backend。
- 批准计划精确绑定 Automation 定义与版本、事件快照、来源、预分配 Run ID，以及每个动作的命令、实际参数和有效风险；批准时重新验证完整计划与已安装版本，漂移即失败关闭。
- 批准采用原子 claim 并使用同一 Run ID 从头执行；拒绝、过期、失败、重启中断和重复批准均有明确终态，重启只恢复未过期 pending，绝不自动重放 executing。
- Catalog 读取边界会把独立持久化的有效启用态同步进运行 Definition，避免已启用事件规则被错误跳过；待批期间停用或升级会使旧计划失败终结，Safe Mode 在 claim 前拒绝并保留 pending 供退出后明确处理。
- 桌面工作区已提供待批准计数、列表、完整参数审查、批准与拒绝交互；浏览器预览只展示空状态，不伪造宿主批准或执行。

## 2026-07-18 — Creator 摘要绑定的一次性批准

- Creator 审查绑定完整结构化草案的 canonical SHA-256，并返回相对未安装基线的新增 Capability Diff 与最高风险。
- 保存前必须获得五分钟、一次性、服务端持有的批准凭证；保存时重跑生产契约和隔离行为检查，再原子消费凭证。
- 过期、摘要不匹配、重放、Safe Mode 与浏览器预览均失败关闭；保存仍只是草案操作，不代表安装或启用。
- 新增外接 AI 能力目录，覆盖行为、资产、UI、声音、Automation、Program、Skill、Connector、Agent、CLI、知识、诊断、测试和迁移。
- User Program 与 Skill 草案可在批准后由宿主构建受控临时包并复用生产原子安装器；Automation 定义新增严格版本并进入 SQLite 原子 Catalog。三类产物安装后都保持未授权、未启用，升级保留上一版本；Automation 工作区已提供目录、显式启停和回滚控制。
- Creator 升级审查会重新加载并复验当前安装版本，展示版本基线、Capability `added/removed` 及事件、命令、Contribution、运行预算 `scope-changed`；批准同时绑定草案与完整审查摘要，安装基线漂移即失败关闭并消费旧凭证；损坏安装包不能降级成首次安装，所有升级都明确要求重新授权。

> 更新日期：2026-07-18
> 适用阶段：首个稳定版之前  
> 原则：优先级只决定施工顺序，不缩减产品范围；“已完成”必须同时具备真实实现、失败边界、自动化证据和同步文档。

## 状态定义

| 状态 | 判定标准 |
|---|---|
| 已验证 | 真实实现、正常与异常测试、文档和适用运行证据齐全 |
| 部分实现 | 已有可运行纵切，但规格中仍有明确能力或平台证据缺口 |
| 未实现 | 只有规格、契约占位或尚无可运行代码 |
| 外部验证受阻 | 实现可继续，但当前环境缺少设备、签名身份或可连接的浏览器/平台实例 |

## 全量能力矩阵

| 领域 | 当前状态 | 已有证据 | 必须继续闭环的缺口 |
|---|---|---|---|
| 桌宠窗口与交互 | 部分实现 | Tauri 透明双窗口、拖拽、置顶、穿透、托盘、安全模式、Click/Drag FSM 与 Rust 测试 | Windows/macOS 真机冒烟、多屏/DPI、WebView 崩溃恢复、长稳验证 |
| UI 与设计系统 | 部分实现 | Control Center、Creator Studio、Overlay、Token 与组件样式、前端单测、生产构建；目标控制中心宽屏截图与语义树已验证 | 键盘完整路径、读屏实机、200% 缩放、关键状态视觉回归、跨平台像素审查 |
| Profile 与离线状态 | 部分实现 | 唯一 SQLite Schema、离线 Profile、Online Backup 调度与原子恢复、损坏数据库隔离启动、统一写门禁、恢复 UI；Profile 切换与进入/退出 Safe Mode 共用 Tauri-free 可逆原生事务协调器；进入 Safe Mode 后使用独立 fail-closed 协调器全尝试 Auto Mode、用户程序、Skill、Agent、策略缓存和 Renderer 收敛，失败保持 Safe 并写固定 Security 诊断；Backup Service 统一手动/定时备份、健康状态、错误收敛和恢复请求；诊断服务统一版本、运行状态和 fail-closed 隐私声明，支持脱敏导出；Rust/TS 故障测试与 Chrome 实测 | 退出 Safe Mode 的 `RecoveryPending/Degraded` 恢复补偿状态、休眠与时钟异常、恢复模式真实桌面截图、跨平台真机故障注入、人工数据提取与跨设备备份 |
| Event 与持久 Outbox | 部分实现 | 事务写入、租约、ACK、重试、死信、清理、健康状态和自动化测试 | 具体幂等消费者、跨重启投递恢复、Connector 投递审计 |
| Sprite 角色与皮肤 | 部分实现 | 严格包契约、安全导入导出、序列/图集真实渲染、动作 fallback | 独立预览实例、命中区编辑、连续切换泄漏与性能门禁 |
| glTF/GLB 角色 | 部分实现 | 独立 Worker 探测、命名动画报告、可编辑标准动作映射、原子安装、受控协议、Three.js 真渲染、cross-fade、framing、释放与失败回退 | 独立预览、持续切换与 GPU 压测、真实截图和跨平台验证 |
| VRM 与 Live2D | 部分实现 | VRM 1.0 已实现 GLB 结构/扩展/版本/meta/humanoid 安装复验、`.vrm` Inventory、按需 `@pixiv/three-vrm` Adapter、固定公共动作到标准 Expression Preset 的安全映射、逐帧物理更新、GPU 资源释放和独立 Bundle Budget；Live2D 因专有 Runtime/许可证尚未接入并由 Installer 提前拒绝 | VRM 用户映射、look-at、lip sync、动作重定向与真实 GPU 长稳；Live2D 许可证感知 Adapter、参数映射、隔离和资源释放测试 |
| 主题包 | 部分实现 | `nimora.theme/1` 严格 Token、透明色合成后的 WCAG 最低对比度门禁、安装前局部预览、原子安装/激活、显式恢复内置主题、Safe Mode 宿主与 UI 同步回退 | 完成高对比度人工审查、主题编辑器、签名和跨平台视觉验收 |
| 声音包 | 部分实现 | `nimora.voice/1`、WAV/OGG 头复验、Clip/Cue/字幕/增益预算、安装前预览、原子激活、Quiet Mode 门禁、Safe Mode 静音回退和逐次重开完整性复验 | 声音编辑器、录音/裁剪、混音总线、TTS 映射、许可证元数据和真实设备长稳 |
| 行为包 | 未实现 | 统一 Asset Manifest 与动作语义规格 | 严格静态子契约、无代码动作图、编辑预览、资源预算、回退和 Renderer 兼容测试 |
| 用户代码执行 | 部分实现 | 独立 JS Worker、预算、强制取消、安装完整性、版本精确授权、Capability Gateway | 授权型文件/网络/自动化后端、调试器、录制回放、跨平台 sidecar 发布验证、OS 资源硬限制 |
| AI 辅助扩展创作 | 部分实现 | 专用无工具 Draft Agent、严格 JSON/生产校验、独立 Worker/DryRun、摘要绑定批准与 Workspace 原子保存；Program/Skill 复用生产安装器；Automation 使用版本化 SQLite Catalog，支持命令与行为 Diff、原子安装、强制停用、升级保留上一版、目录启停和回滚；已安装 Automation 的 Medium/High 运行采用参数、事件快照、定义版本、来源和有效风险绑定的一次性批准 | 多事件/虚拟时间/Mock 行为矩阵、文件/依赖/迁移 Diff、真实事件创作调试、删除与历史 UI、Connector/Agent/UI/资产生成、监控和 AI 修复闭环 |
| 事件驱动程序 | 部分实现 | 可信 Rust 订阅、`serial`/`drop`/`cancel-previous` Supervisor、迟到完成隔离 | 桌面纯 Supervisor 集成测试、执行历史与 Creator Studio 可视诊断 |
| 扩展与 Skill 生态 | 部分实现 | `skill-runtime` 实现严格 Manifest、精确 `commandAllowlist`、`onEvent:*` 与 `subscribe-events` 强绑定、命名空间 Contribution、精确授权、激活租约、恢复/quarantine/卸载和 `skill:<id>` requester；独立 `skill-host`/`skill-worker` 实现版本化 JSONL、清空环境、真实进程隔离、Boa JavaScript、取消/超时/输出预算、结构化 Command/Agent Task 计划及 Worker 故障驱动的 Contribution 撤销；`skill-package` 实现原子安装、完整库存与 SHA-256 复验、备份回滚；Desktop 实现安装、目录、授权、启停、回滚和执行 IPC，执行绑定 Activated Manifest 租约，Agent Task 接入统一 Module Adapter 与 Agent History，Command 整批预检 allowlist/注册风险后将 Safe/Low 经共享 Capability Gateway 执行，Medium/High 使用 SQLite 保存参数绑定的五分钟一次性整批批准；Journal 原子 claim、单事务拒绝/过期、完成/失败终态、重启 pending 恢复与 executing 中断、待批准列表 IPC；获准 Activated Skill 的事件声明使用独立有界 Runtime Event Bus 订阅与串行调度，Host 重建、停用、升级、回滚、Safe Mode 和故障撤销会话并取消在途 Worker/Provider，代际 ID 隔离迟到线程；活跃执行支持按 execution 取消 Worker、Command 后续副作用及当前 Provider Agent；Skill 执行元数据历史支持等待/完成/拒绝/取消/失败状态收敛、稳定游标分页与单条/全部隐私删除，取消终态不可被迟到结果覆盖，且不保存输入、源码、命令参数或 Agent 正文；拒绝/过期/重复批准 fail-closed，未知/未声明在副作用前拒绝，Recovery Mode 隔离扩展 | 跨文件系统与 SQLite 的安装崩溃一致性 Journal、发布者签名、OS CPU/内存沙箱、事件会话可视诊断与 UI/Tool 等 Contribution、管理 UI、官方番茄钟与提醒 |
| 自动化引擎 | 部分实现 | 独立 `automation-runtime`；版本化 Event Trigger、JSON Pointer Condition、顺序 Action 与 Policy；有界取消/超时；幂等且 retry-safe 的瞬时重试；失败逆序补偿；结构化 Run 结果；Backend 获得宿主生成且覆盖动作/重试/补偿的 Run/Automation/Action/Event/Trace 因果上下文；Engine 支持宿主预分配稳定 Run ID；SQLite Run Journal 在副作用前记录 running、原子完成、精确 interrupt、严格身份/状态转换，桌面启动恢复遗留 running 为 interrupted；主动作与补偿动作在副作用前整批执行宿主有效风险预检，未知命令失败关闭，Safe/Low 立即执行，Medium/High 进入参数、事件快照、定义版本、来源和 Run ID 绑定的五分钟一次性 SQLite 批准，Critical 拒绝；批准前不创建 Run Journal、不注册活跃运行、不调用 Backend，原子 claim 后使用同一 Run ID 从头执行；拒绝、过期、失败与 executing 重启中断有持久终态；待批准目录、批准/拒绝 IPC 与桌面 UI 已贯通；Live Run 宿主取消注册表、原生取消 IPC 与 TS 契约会置位父 Run 并按持久 Journal 级联取消活跃 Agent 子任务和 Provider Worker；原生 IPC 与浏览器同契约零副作用测试运行；自动化工作区 UI；Desktop Live IPC 通过共享 Capability Gateway 执行宿主批准的 Pet Action，Safe/Recovery、未知动作和风险低报 fail-closed；`automation-agent-bridge` 将固定 `agent.task.run` 经风险、幂等、不可信上下文门禁和统一 AgentTaskGateway 转为以 Automation Run 为根的共享 Trace/层级预算子任务；桌面 Submitter 已复用 Provider/Coordinator/Tool/Gateway/审批/历史链路，显式模型与 Tool Allowlist 贯穿确认续跑，根 Run + 幂等键阻止内存生命周期内重复提交；Rust/TS 测试 | 持久异步结果回填、跨进程运行中恢复、运行保留策略、并发/冷却/费用门禁、不可取消副作用补偿；扩展 Profile/Character/Program Action 宿主策略；Interval/日历/快捷键等触发器、分支与并行汇合、持久规则、虚拟时间与事件回放 |
| 开放 Gateway 与 Connector | 未实现 | Capability Gateway 为进程内用户代码提供窄能力边界 | 配对、Token/Scope、REST/WS/SSE、HTTP/UDP Sink、Source Connector、2 秒安全停机 |
| AI Agent 与 CLI | 部分实现 | Tool Registry、风险与批准、任务硬预算；Provider/Tool 单步协调；Provider 续跑的 Assistant Call 与 Tool Result 强关联协议；多调用 Turn 完整聚合器；Ollama Worker 结构化续跑载荷及真实独立 Worker 双轮 Tool Call 自动化；十三项生产模块工具，自动化定义零副作用验证、角色渲染能力脱敏摘要、Runtime 动作词汇发现、原生策略事务化 Profile 切换、资产复验与刷新回滚型角色切换、完整性复验且路径脱敏的用户程序发现、绑定精确版本批准的隔离程序执行、可扩展只读能力集合及共享 Capability Gateway 固定映射；Automation Context Admission 将受信目标与来源化不可信数据分离，实施段数/字节预算、高置信中英文注入拒绝，并把不可信任务限制为 draft 且无工具，桌面 Provider 保留 untrusted 消息位；完成任务历史的版本化 SQLite 仓储、稳定游标分页和隐私删除；桌面完成态写入、降级不篡改结果、Recovery 内存隔离、分页/删除 IPC、最近历史 UI 与 Preview 会话实现；CLI 完成态旁路写入、显式数据库导出、成对游标分页和单条/全部删除；统一桌面 Agent 结果契约；Provider 等待确认、多确认项展示、部分批准后剩余项回填、最后批准后 Provider 续跑与整组拒绝 UI；构建期嵌入 Ollama Manifest 信任摘要，桌面启动时复验 Manifest 与 Worker 后自动注册；受隔离 Worker 的 `/api/tags` 健康探测、严格模型目录预算、去重排序、真实 Provider 状态与模型选择；桌面宿主对写调用整组批准前零副作用，拒绝/过期级联撤销，全部批准后按原始顺序执行；Safe/Recovery Mode fail-closed；CLI 工具发现与 Ollama 接线；独立 sidecar、loopback-only SSRF、Worker 完整性发现、有界协议、超时取消强杀和跨进程 mock 测试 | 用户本机实际 Ollama 模型桌面验收、继续扩展生产工具覆盖面、OpenAI-compatible Adapter、发布者数字签名、持久计划恢复、跨来源完整 Prompt Injection 防护与 Unicode/编码混淆检测 |
| 包签名与 Registry | 未实现 | SHA-256 本地完整性锁和原子回滚 | 发布者签名、信任根、撤销、Registry、兼容检测、更新策略、离线 CLI 验证 |
| 安全与隐私 | 部分实现 | 安全模式、精确授权、路径/符号链接防护、Worker 隔离、审计边界文档 | OS 沙箱、系统密钥存储、网络目标策略、隐私面板、威胁模型自动门禁 |
| 稳定性与可观测性 | 部分实现 | 结构化错误、Trace、Outbox 健康、故障回退、版本化脱敏诊断摘要包、14 天/1 MiB/64 段有界固定事件日志、跨重启恢复、损坏尾行跳过、存储失败内存降级、用户可取消事件导出及单元/集成测试 | 自由文本日志与筛选、用户选择 Trace、指标、崩溃循环恢复、资源预算监控、soak/chaos 和 SLO 门禁 |
| 部署与发布 | 部分实现 | pnpm/Rust 构建、Tauri 配置、CI 规范 | Windows/macOS 签名安装包、公证、更新回滚、SBOM、许可证、发布演练 |
| Git 与工程治理 | 部分实现 | Git 规范、Conventional Commits、pnpm-only 门禁、全量本地质量命令 | 受保护分支/PR 实际仓库配置、跨平台 CI、依赖与安全扫描证据 |

## 执行规则

Desktop 组合根治理已开始：Character、Theme、Voice 的选择 Policy、版本化存储、Safe Mode/损坏记录解析和原子写已迁移到 Tauri-free `asset_selection` Application Module；资产 URI 的 Window/Method/Host/Query/Path/活动角色/Inventory 复验和媒体响应已迁移到 Tauri-free `asset_protocol` Application Module，Tauri 仅翻译 HTTP 类型。机器门禁阻止宿主类型回流。`src-tauri/lib.rs` 仍包含大量 IPC、DTO、Agent、Skill 与资产编排，必须继续拆分，当前不得标记治理完成。

AI 双向模块交互的当前审计与完整实施蓝图见 [`AI_MODULE_INTERACTIONS.md`](AI_MODULE_INTERACTIONS.md)。现阶段 AI 到 Pet、Profile、Character、Asset、Program、Diagnostics 和 Automation Validation 已形成生产工具链；宿主无关的 `AgentTaskGateway` 已完成调用方、Provider、Tool、数据等级、主动性、递归深度和父级剩余预算交集准入。统一 `module-agent-adapter` 已封装可信模块身份、Provider allowlist、固定 Draft/无工具策略、Context Admission、相关 Trace 和 Provider 消息边界；用户程序与 Skill 均已成为生产调用方。Skill Runtime、独立 Worker、Package Core、Desktop IPC、启动恢复和 SQLite 精确版本授权/启用状态仓储已贯通 Manifest、授权、贡献租约、崩溃隔离、原子安装、完整性复验、回滚、Command 调用、Agent Task 提交、活跃执行取消、脱敏历史和 `onEvent:*` 自动调度。获准 Activated Skill 的 Agent Tool Contribution 已动态汇入 Desktop Catalog、Provider Tool Turn、独立调用与批准续跑共用的 Registry；独立 Capability、命名空间、Schema/元数据预算、精确命令映射、宿主风险复核、参数绑定批准、Capability Gateway 执行和停用撤销均已贯通。Coding Agent 级持久 Goal/Plan 核心、SQLite 双表修订仓储和机器可读 CLI 已贯通，完成必须由当前计划逐项证据证明。Auto Mode 已完成策略/会话领域层、逐步风险与预算准入、SQLite 乐观并发持久化、重启安全暂停、跨进程 CLI 控制面、整轮预检与安全只读 Tool continuation，以及版本化 Checkpoint 领域对象和 SQLite 单调序号 CAS 仓储；Checkpoint 精确绑定 Task/Goal/Plan/Workspace/Policy 并复验索引元数据，且不保存 Approval 或宿主对象。独立 Auto Host 恢复服务已跨 Goal/Session/Checkpoint/Workspace 仓储复验绑定并真实重扫工作区，只释放 paused Task 与 continuation 数据，文件漂移、缺失状态或非暂停 Session 均 fail-closed，且不调用 Provider/Tool。Agent Runtime 已新增可追溯 Context Compactor、内容寻址 TTL/LRU 内存 Cache，以及不可变 Workspace 文件快照与父指纹版本链；Tool continuation 不会被压缩拆分，路径逃逸、重复文件、篡改指纹和错误父版本均 fail-closed。SQLite Context Cache 已实现事务化 TTL 清理、稳定 LRU 条目/总字节治理、数据等级命中门禁、内容与索引复验及按 Workspace fingerprint 失效。独立 Workspace Host 已贯通 canonical root、symlink 拒绝、有界扫描、Git/Nimora ignore、TOCTOU 复核、Git HEAD/index/worktree 状态与 `ai workspace inspect` 机器可读 CLI，且对外只暴露相对路径与不可逆 root fingerprint。SQLite Workspace Snapshot 仓储已实现 revision/parent/root/fingerprint CAS 和索引/Payload 复验；Auto Mode 启动改为宿主真实扫描绑定，恢复时复扫根目录，内容漂移则保存后继快照、保持暂停并稳定返回 `workspace-changed`，不再接受调用方伪造 revision。尚未连接桌面 Gateway 持久宿主循环、恢复候选到显式 Resume/Provider 下一轮的原子提交、持久 Cache 的 Auto Host Loop 接入与系统密钥加密、Auto Mode 桌面每轮扫描、Session 显式重绑定与双仓储原子应用服务。尚未贯通的是 Contribution 与事件会话管理 UI、Connector Runtime 的模块反向调用链，以及多 Agent 调度、Auto Mode 桌面监督和桌面 Goal UI。

Automation、User Program 与 Skill 已复用共享 Context Admission，具备来源专属硬上限、Unicode 混淆检测、稳定拒绝原因和无正文结构化审计。Desktop 仅持久化脱敏来源类别、计数及 Run/Trace/Automation/Action/Command Execution 或 Module/Module Execution 关联 ID；Prompt Injection 原文不进入 Provider、History、序列化或 Journal，审计写入故障保持 fail-closed。已启用 Automation Catalog 会在桌面正常模式建立独立有界 Event Bus 订阅，保留来源 Event/Trace ID，经真实 Capability/Agent Backend 执行并写入 SQLite Journal；停用、升级、回滚、安全模式与退出会取消会话和共享取消令牌，激活失败补偿为停用。桌面工作区现提供有界最近运行、结果状态、终态历史删除和 `(startedAtMs, runId)` 稳定游标翻页，运行中记录始终保留；活跃事件会话提供 executed/dropped/failures 饱和计数健康快照与警告 UI，不暴露事件正文，也不把无会话误报为健康。队列指标当前为进程内活跃会话事实，持久趋势、告警阈值和自动隔离仍未完成。Connector、Clipboard、Files 与 Vision 接入同一生产准入和审计通路仍是明确缺口。

1. 每次开发从本矩阵选择一个或多个可形成真实纵切的缺口，但不得删除其余缺口。
2. 新发现的有价值能力必须加入矩阵或对应权威规格，不依赖聊天记忆。
3. 只有证据满足状态定义时才能改为“已验证”；单元测试不能替代真实 UI、平台或安全边界验证。
4. 首个稳定版前直接维护唯一当前契约，不为尚未发布的设计制造迁移链或兼容包袱。
5. 每个切片完成后更新矩阵、功能测试计划、相关专题文档，并提交推送主仓库。
### Auto Mode 原子恢复补充

Agent Runtime 已新增 Provider 无关的推理策略领域契约，统一 `auto/minimal/low/medium/high/very_high/maximum` 语义和 Adaptive/Quality First/Cost Saver/Fixed 策略；显式不支持等级 fail-closed，并可审计 requested、actual 与 Provider value。范围绑定 `AuthorizationGrant` 已覆盖 Sandbox、Approval、Network、寿命、Goal/Plan/Workspace、Tool、Provider、模型、数据等级和预算，支持过期、撤销、绑定漂移与范围外拒绝。当前尚未接入 SQLite Grant 仓储、系统密钥保护、Auto Loop 每轮准入、Provider capability/mapping、Context Cache 推理指纹、桌面授权 UI 与 CLI 控制面。

桌面 Tauri 宿主已接入 `agent-auto-host` 的持久单轮 Facade，并复用现有生产 Provider Registry、动态 Skill Tool Registry、Gateway Tool Backend 与 Capability Gateway。版本化 IPC 和 TypeScript 平台层支持显式 Session、Workspace、约束、输出预算与离线策略；Safe Mode、Recovery Mode 和非法预算均在 Provider 前 fail-closed。当前仍是用户触发的一次 Resume + 单轮执行，后台有界监督循环、桌面 Goal/Plan/Attempt UI、不确定 Attempt 对账和网络数据出境确认尚未闭环，因此 AI Agent 与 CLI 领域继续标记为“部分实现”。

宿主无关的有界 Loop Facade 已实现连续 Continue、终态停止、业务暂停停止、Workspace Drift 停止和 `1..=256` 批次公平让出；真实 SQLite 测试证明两轮 Tool continuation 到完成、单轮让出后 Running/Checkpoint 保持，以及非法上限在 Provider 前拒绝。

桌面宿主现已新增独立 `auto_mode_jobs` Application Service 核心：使用单一互斥注册状态原子维护 Job 与活跃 Session 索引，保证同一 Session 只有一个活跃 Job；版本化快照记录状态、累计 Turn/Cache Hit、Checkpoint、暂停原因、错误码和时间；Pause/Cancel 使用共享原子控制信号，Cancel 可覆盖未收敛 Pause；终态释放 Session 但保留可查询快照。严格 Clippy 与单元测试已覆盖唯一性、控制传播、单调批次计数、终态释放和历史快照。

Auto Host 已新增 Yield 边界的持久控制服务：Pause/Cancel 在没有活跃 Provider/Tool Attempt 时，以 Session timestamp + Checkpoint sequence 双 CAS 原子更新 Session、Task 与 Checkpoint；Pause 固定为 `user_requested`，Cancel 使用合法 `Cancelled` Task Checkpoint，陈旧并发控制整体拒绝且不能覆盖先到状态。桌面已接入 `start_auto_mode_job` 后台 Runner：恢复并提交 Paused continuation 后直接运行有界 `AutoModeLoopService` 批次，Yield 后继续公平调度，业务暂停/完成/Workspace Drift 写入 Job 终态，宿主 Pause/Cancel 在干净边界复用持久控制服务，在途 Provider/Tool 错误依赖 durable Attempt 隔离并将 Job 标记 `indeterminate`。Runner 编排已从 Tauri 入口提取到独立模块，入口仅保留 DTO、校验、线程启动与依赖装配，模块内部显式声明依赖并拆分控制收敛、策略构造和错误分类。Job Supervisor 提供单 Session 唯一性、单调进度、状态/暂停/取消 IPC、共享 Provider CancellationFlag、活跃枚举与退出全量取消；应用退出与 Safe Mode 共用取消、Condvar 有界等待 2 秒和超时隔离协议，分别保留 `shutdown-timeout` 与 `safe-mode-timeout` 诊断码，迟到 Runner 不能覆盖。正常启动现以 SQLite 为唯一事实源：先把崩溃遗留 Running Session 原子转为 `paused/restarted`，再有界枚举全部 restarted Session 与未决 Attempt；Active Attempt 永久隔离为 `indeterminate`，Supervisor 仅导入不占 Session 的终态投影，绝不自动续跑。目标控制中心已聚合 Job、Session、Goal、绑定 Plan、Checkpoint、Attempt 和不可变 Resolution；参数绑定对账弹窗要求用户填写核验理由，可选择“确认未执行并暂停”或“接受外部副作用并取消”，Safe/Recovery Mode 与浏览器预览保持只读。尚缺真实桌面跨崩溃端到端演练、系统密钥保护和更完整的 Goal/Plan 编辑体验，因此仍不得描述为完整生产后台 Auto Mode。

Auto Host 已进一步实现显式恢复原子提交：Session timestamp 与 Checkpoint sequence 双 CAS 在同一 SQLite Immediate 事务中将 Session/Task 同时恢复为 `running`，任一竞争整体回滚，且不触发 Provider/Tool、不复用 Approval。尚未完成的是恢复后的单轮执行结果持久提交、每轮 Workspace 重扫、持久 Cache Host Loop 接入与系统密钥加密。

Auto Host 上下文准备已接入持久 Context Cache：完整 continuation 经协议安全压缩后，以 Provider、模型、Plan revision、Workspace fingerprint、消息内容和数据等级进行精确命中；miss 写入沿用 SQLite TTL/LRU/容量治理。尚未完成 Provider/Tool 单轮结果提交、每轮 Workspace 重扫、缓存系统密钥加密与指标控制面。

Auto Host 每轮 Workspace 门禁已实现真实有界重扫；一致时才允许继续，漂移时以 Session/Checkpoint/Workspace 三重 CAS 原子暂停 Session 与 Task 并追加 successor，竞争写入整体回滚且零 Provider 释放。单轮结果提交服务已支持 Continue、Paused、Completed 三类结果，将 Session/Task 生命周期、完整 continuation 与单调 Checkpoint sequence 以 timestamp + sequence 双 CAS 原子提交。调用前 durable Turn Attempt 已精确绑定 Session、Checkpoint、timestamp 与请求指纹且禁止过期重领；结果事务原子消费 Attempt，重复 Begin、陈旧提交与崩溃遗留均 fail-closed，遗留 Attempt 转为 indeterminate 并阻断 continuation 自动恢复。该链路已接入桌面持久监督 Runner、聚合控制中心与人工处置 UI；剩余缺口是缓存系统密钥加密、真实跨崩溃桌面演练和更完整的指标控制面。

Auto Host 已将上述独立能力组合为生产单轮执行 Facade：真实 Workspace 预检、Context Cache、durable Attempt、Provider/Tool Supervisor 和原子 Commit 按固定顺序执行。真实 SQLite/Workspace 测试覆盖 Provider 完成、安全只读 Tool continuation、写 Tool 整批零派发暂停、Provider 失败 indeterminate 隔离及漂移前置退出；Checkpoint 同时支持验证历史 Tool Call/Result 结构而不持久化 Tool Descriptor。桌面后台有界监督循环及 Goal/Plan/Attempt 聚合查看、暂停、取消和人工对账 UI 已接通；尚缺缓存系统密钥加密、Goal/Plan 完整编辑和跨重启桌面端到端验证。
## 2026-07-18 — Indeterminate Attempt reconciliation

- Added a parameter-bound manual reconciliation contract for indeterminate Auto Mode attempts.
- `confirmed_not_executed` atomically pauses Session and Task, advances the Checkpoint, archives an immutable resolution, and never retries automatically.
- `accept_external_effect_and_cancel` atomically cancels Session and Task without fabricating a Provider success result.
- Desktop IPC exposes bounded detail/history and resolution commands; browser preview fails closed with `desktop-host-required`.
- Goal Control Center now renders indeterminate Attempt risk, bound Session/Checkpoint/fingerprint facts, two mutually exclusive decisions, a required reconciliation reason, immutable resolution history, and read-only Safe/Recovery/Browser Preview states.
- Resolution binds Session, Attempt, Checkpoint sequence, request fingerprint, actor, reason, decision, and host timestamp. Stale or replayed requests roll back as a unit.
- Five dedicated real-SQLite tests now prove both decisions, zero-write rejection, one-winner concurrent reconciliation, replay rejection, audit persistence across reopen, bounded queries, and index/payload divergence detection.

## 2026-07-18 — GLTF lazy-loading performance gate

- Confirmed the Three.js renderer remains a direct dynamic dependency and is absent from the desktop entry chunk.
- Added a generated Vite Manifest and an executable build budget gate instead of hiding the previous warning without evidence.
- Current raw production sizes are 285,916 bytes for the desktop entry and 615,183 bytes for the lazy GLTF renderer, under hard budgets of 350,000 and 650,000 bytes.
- The existing Suspense placeholder and built-in renderer fallback remain active for module loading, WebGL initialization, asset loading, and context-loss failures.

## 2026-07-18 — Voice asset architecture audit

- Added the strict `nimora.voice/1` static asset contract with bounded WAV/OGG clips, safe cue identifiers, required localized captions, finite gain limits, verified preview bytes, and reopen-before-read behavior.
- Corrected a cross-layer defect where inventory media extension validation carried Sprite-specific assumptions and errors; media extension validation is now asset-neutral while each asset subtype keeps its own allowlist and header checks.
- Voice now has desktop activation, Safe Mode silent fallback, per-read package verification, Creator Studio preview/activation, archive round-trip coverage, and Quiet Mode enforcement before clip retrieval. Full workspace validation and real Tauri audio/UI verification remain required before release completion.
- Implemented typed `AssetSelectionPolicy` lifecycle sharing for Character, Theme, and Voice without weakening subtype verification. Schema handling, Safe Mode, fallback classification, real I/O propagation, and atomic persistence now share one implementation; four cross-policy tests raise the Desktop Host suite from 91 to 95 tests.
- Final local gate passed with 36 Asset Installer tests, 91 Desktop Host tests, 32 frontend tests, production TypeScript/Vite build and bundle budgets, workspace Clippy with warnings denied, and the complete Rust workspace test/doc-test suite. Real Tauri audio output and cross-platform visual inspection remain release-level evidence, not unit-test claims.
