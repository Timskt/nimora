# Nimora 全量实现状态与证据矩阵

## 2026-07-19 — 经典桌宠手势与轻量菜单

- Pet Overlay 不再在按下瞬间强制原生拖拽：统一手势仲裁使用 6px 移动阈值，轻微指针抖动仍判定为点击，超过阈值后 Drag 获得优先级并抑制后续 Click。
- 单击通过 220ms 窗口与双击消歧；双击取消待执行单击并触发庆祝动作。500ms 长按与右键打开同一轻量桌宠菜单，避免为不同输入方式复制行为链。
- 菜单提供喂食、玩耍、梳理、休息和鼠标穿透，全部复用现有 Runtime/Capability 边界；不调用 AI、不访问网络。菜单动作后自动关闭，Esc 关闭并将焦点返回桌宠，首项获得确定性键盘焦点。
- 浏览器实际检查覆盖菜单视觉层级、动作关闭和 Esc 恢复；纯函数测试覆盖阈值内抖动与阈值边界，前端自动化增至 53 项。原生 OS 拖拽、长按时序及 260×300 窗口仍需 Tauri 真机复验。

## 2026-07-19 — 生命值驱动的非打扰行为反馈

- 自主领域策略现在读取持久 Energy/Mood：Energy ≤ 25 时优先休息；Energy 充足且 Mood ≤ 25 时优先温和互动；健康状态维持原有确定性序列，完全离线且不依赖 Provider。
- Energy 具有更高优先级，避免疲惫桌宠因低心情被迫活跃。Idle 情绪派生为 Sleepy/Sad/Neutral，同时不覆盖 Drag、Work、Interact 等用户或活动状态。
- Overlay 的悬浮/焦点气泡按当前行为和生命值提供温和状态文案，不发送系统通知；内置角色增加困倦眼神与低落嘴型，第三方角色继续通过标准 Sleep/Interact 动作响应。
- 自动化证据覆盖低值优先级、活动状态保护、Idle 情绪派生和非警报式中文文案；Core 测试增至 26 项，前端测试增至 51 项。

## 2026-07-19 — Profile 驱动的桌边吸附

- 原生拖拽释放链路新增桌边吸附规划，距离当前显示器安全边界 32px 内时吸附最近边，阈值外保持用户摆放位置；计算使用宿主物理坐标、真实窗口尺寸和既有顶部/底部/水平安全边距。
- 规则支持负坐标副屏与角落稳定裁决，Renderer、Program、Skill 和 AI 不接触显示器对象，也不能伪造吸附结果；最终坐标通过既有 Runtime 持久命令保存。
- Profile 新增向后兼容的 `edgeSnap` 可选策略，旧数据缺字段默认开启；Profile 管理器可明确选择“桌边吸附”或“自由摆放”。
- Rust 自动化覆盖阈值内、阈值外、底边、角落与负坐标副屏；Schema 保持旧 Profile 读取兼容。系统 Dock/任务栏精确工作区仍需要各平台签名桌面真机证据，不能由浏览器预览替代。

## 2026-07-19 — 离线桌宠自主生命循环第一纵切

- `runtime-core` 新增显式时间驱动、确定性的自主调度契约，持久保存序列、下次触发时间、活动截止时间和语义意图；旧 Pet JSON 通过 `serde(default)` 无损恢复。
- 第一纵切可按稳定序列执行观察、探索、休息，并具备首次空闲延迟、动作时限和冷却；不访问 AI、Provider、网络或 React，因此断网和关闭控制中心后仍可工作。
- 用户拖拽、点击或其它显式状态拥有绝对优先级；自主活动检测到状态被抢占后只清理自身计划，不覆盖用户状态。Safe/Recovery Mode 不启动或不推进自主循环。
- Desktop Host 使用可取消的低频后台线程推进状态，退出时发出停止信号；每次持久转换通过 Runtime 原子保存 Pet 与 Outbox Event，并向 Pet WebView 发送可信事件刷新渲染，不使用高频 IPC 轮询。
- Explore 意图现在不再只是播放 Walking 动画：Desktop Host 根据当前物理显示器、窗口尺寸和持久序列计算确定性目标，以 12 帧短路径平滑移动真实原生窗口；目标保留顶部、底部和水平安全边距，支持负坐标副屏，并复用既有 Moved 防抖持久化。
- 漫游执行逐帧复验 Drag、Safe Mode 与 Pet Walking 状态，用户抓取或安全状态可立即中止剩余路径；宿主不会在领域层保存显示器对象，也不会让 React 获取原生窗口控制权。
- 正常模式下的低频宿主循环会复验窗口与全部当前显示器的物理交集：仍跨屏时保留最大可见面积所在屏，分辨率/DPI 改变时夹取到该屏安全区域，副屏拔出导致完全离屏时明确回到主屏；拖拽与 Safe/Recovery Mode 下不抢夺窗口。
- 当前自动化证据覆盖确定性选择、动作结束、冷却、用户抢占、负坐标副屏、四向边缘夹取、完全离屏主屏恢复、最大交集屏幕选择、分辨率缩小和显示器枚举缺失；真实热拔插观感、可用工作区/系统 Dock 感知、安静时段设置 UI、频率预算与 24 小时真机长稳仍按发布门禁继续实现和验收。

## 2026-07-19 — Profile 驱动的桌宠自主频率

- Desktop Host 不再使用固定默认自主策略；每个一秒 Tick 都从活动 Profile 解析策略，因此切换无需重启且至多一秒生效。
- `proactiveFrequency` 已接入可审计的五档首次延迟与冷却预算，0% 明确关闭自主互动；Profile 界面同步显示“关闭自主互动”并为滑杆提供可访问名称。
- Focus 与 Presentation 会即时抑制仍由自主循环控制的动作并回到 Idle；若用户已拖拽或触发其它高优先级状态，只清理自主计划而不覆盖用户状态。
- Offline Profile 不被静默禁用，桌宠自主生命继续完全本地运行；Rust 测试覆盖频率单调性、安静模式、离线语义和抑制状态收敛，前端测试覆盖 0% 文案。

## 2026-07-19 — 持久离线生命值第一纵切

- Pet Snapshot 新增向后兼容的生命更新时间，领域层按显式时间和十分钟粒度确定性演化 Energy/Mood；首次只建基线，离线追赶最多 24 小时，避免长时间关机后的惩罚性归零和启动风暴。
- 点击互动现在真实提升 Mood 与 Affinity，并使用饱和运算封顶 100；生命状态仍不依赖 AI、Provider、网络或控制中心窗口。
- `RuntimeService::tick_vitals` 将 Snapshot 与 `pet.vitals.changed` Outbox Event 原子保存，持久化失败不改变内存或事件总线；Desktop Host 仅在 Normal Mode 推进并向 Pet Overlay、控制中心发送可信刷新事件。
- 控制中心已移除硬编码 Energy、等级和演示文案，改为展示生产 Snapshot；Rust、Schema 和前端契约测试覆盖旧字段兼容、有界衰减、互动封顶、事务失败与事件关联。

## 2026-07-19 — 桌宠照料闭环第一纵切

- 新增可扩展的 `PetCareAction::{Feed, Play, Groom}` 领域语义，各自具有不同 Energy、Mood、Affinity 收益并全程使用 0–100 饱和运算。
- 三类照料共享 30 秒持久冷却；Drag 状态拒绝照料，自主行为可被合法照料安全中止，避免用户操作和后台状态机互相覆盖。
- `RuntimeService::care_pet` 原子保存 Pet Snapshot 与 `pet.care.performed` Event，失败不消耗冷却、不改变内存、不发布事件；Safe/Recovery Mode 在调用前拒绝。
- Pet Overlay 和控制中心均提供喂食、玩耍、梳理入口、成功文案与冷却反馈，控制中心按钮支持窄宽换行；浏览器预览仅更新隔离的预览 Snapshot。

## 2026-07-19 — 运行级 Provider 推理策略纵切

- Provider Descriptor 新增显式、版本化推理能力声明；`auto` 不能伪装为 Provider 具体能力，空集合和非法 Mapping Version 在注册前拒绝。
- Provider Request 可绑定宿主解析后的 requested/actual/provider value/mapping version；Registry 在 Adapter 与任何计费副作用前复验声明、实际等级和版本，未声明能力、越界等级及版本漂移均失败关闭。
- Compacted Context 与持久 Context Cache 的内容寻址身份纳入完整推理映射，不同等级、Provider 参数或映射版本不会错误共享缓存；旧无推理上下文保持可读取兼容。
- OpenAI-compatible 持久 `effort -> provider value` 配置现由 Desktop 宿主解析为一次运行级 Mapping；普通 Agent、Auto Mode 单轮、后台 Loop、工具确认续跑、Context Anchor 和真实 Provider Request 保持同一 Mapping，Worker 仅接收 Provider value。
- Agent Workspace 仅对 Descriptor 明确声明能力的 Provider 展示“自动、节省、极致”和具体固定等级；固定等级不静默降级，Browser Preview 不伪造能力。前端目录同时改为使用 Rust 的真实 `displayName/capabilities` 契约。
- Runtime 42 项、Auto Host 20 项、Persistence 81 项、Desktop 142 项和 Frontend 48 项测试通过；生产构建与分包预算通过。新增测试直接捕获 Adapter 前的 Mapping、验证 Provider value/version 解析、推理变化缓存隔离和三条 IPC 策略传输。
- 尚待后续独立纵切：Anthropic/本地 Adapter 映射、动态 Adaptive 推荐、持久默认策略、CLI 选择器和完整审计展示。

## 2026-07-19 — 真实桌面启动迁移与 macOS Keychain 演练

- 使用现存真实应用数据库启动 Tauri 时发现 `user_version=1` 数据库缺少后续加入的 `automation_run_journal`，导致 setup hook 崩溃；根因是同一数据库版本内的幂等 Schema Extension 漏项，而不是数据库损坏。
- 持久层现会为既有版本 1 数据库幂等补建 Automation Run Journal 及索引，并新增最小旧 Schema 回归测试；不删除、不重建、不覆盖现有用户表，也不会把可迁移数据库误送入 Recovery Mode。
- 修复后使用同一数据库重新启动真实 Tauri 进程成功，进程持续运行且无 setup panic；验证结束后通过开发进程中断正常退出。
- `secret-store` 新增默认忽略、必须显式设置 `NIMORA_RUN_SYSTEM_SECRET_STORE_TEST=1` 的系统凭据生命周期门禁，使用唯一合成凭据验证 Missing → Put → Present → Resolve → Delete → Missing → Idempotent Delete，并在成功或失败路径末尾再次清理。
- 本机 macOS Keychain 真机测试 1/1 通过，合成凭据已撤销；普通 Secret Store 测试 3/3、持久层迁移回归和相关 Clippy `-D warnings` 通过。未签名开发窗口未被 Computer Use 枚举，因此本轮不宣称完成 Provider 表单的原生 UI 自动化。

## 2026-07-19 — 通用 Agent Provider Worker 能力清单

- 将旧的 Ollama 单 Provider 清单升级为唯一的 `nimora.provider-worker-manifest/1`，同一受信 Worker 显式声明 Ollama loopback 与 OpenAI-compatible 两项协议能力。
- 构建脚本、Tauri 资源、编译期信任摘要、运行时发现函数和环境变量全部使用通用 Agent Provider Worker 命名，不再从 Ollama 文件名或固定 Provider ID 推断能力。
- 清单严格拒绝未知字段、缺失能力、重复或乱序能力、协议版本偏差、路径穿越、符号链接以及 Manifest/Executable 摘要或大小不一致。
- `.sha256` 仍只作为发布流水线产物；桌面宿主信任编译期注入的 Manifest 摘要，不会信任 Worker 同目录的可写摘要文件。
- 验证：清单契约测试 5/5、CLI 集成测试 15/15、Desktop Host 测试 142/142 通过；Worker、CLI 与 Desktop 全目标 Clippy `-D warnings`、Rustfmt、脚本语法和双层发布摘要核验全部通过。

## 2026-07-19 — 桌面 OpenAI-compatible Provider 管理纵切

- 桌面运行时已将 SQLite Provider 配置仓储和 System Secret Store 接入正常启动路径；恢复模式使用隔离的内存配置与密钥实现，不接触主数据库或系统凭据。
- Provider 凭据只通过配置绑定的精确 Secret Reference 解析，系统密钥返回的 `Zeroizing<String>` 所有权直接移交 Worker payload；任务、IPC 响应、SQLite、日志和 Debug 输出均不包含 Secret 明文。
- 新增配置列表、CAS 新增/更新、凭据写入、凭据撤销和 CAS 删除命令。Safe/Recovery Mode 禁止配置及凭据写入，凭据撤销保持可用；删除配置与撤销凭据刻意分离，避免伪造 SQLite 与系统密钥库的跨存储事务。
- 动态 Provider Registry、Agent Admission allowlist、Automation、Creator 与 Module Agent Adapter 均从当前启用配置生成，不再限制为两个硬编码 Provider。活跃 Agent 任务记录精确 Provider ID，对应配置在任务结束前禁止删除。
- Provider 状态契约已通用化，统一表达 locality、凭据存在性、Worker 验证、服务可达性和可选模型元数据；OpenAI-compatible Probe 通过隔离 Worker 执行，失败只返回稳定状态，不暴露 Endpoint、Header、Secret 或响应正文。
- 普通 Agent 与 AI Creator 的网络传输改为每次显式授权；默认保持离线，Auto Mode 严格尊重请求中的 `offline`，不再因存在凭据而静默放宽网络策略。
- Agent 工作区新增“模型连接”视图，提供新增、编辑、启停、密钥写入、凭据撤销、删除确认、数据出境说明、浏览器只读状态及窄屏布局。API Key 不回填，提交后立即清除 React 输入状态。
- 浏览器预览已完成 DOM 与全页截图检查，确认导航、标题层级、空状态、只读门禁、表单标签和响应式布局可见；真实系统 Keychain 与网络 Probe 仍必须在签名后的 Tauri 桌面环境完成端到端验证。
- 尚待后续独立纵切：将 Ollama 命名的 Sidecar Manifest 升级为通用 Worker capability manifest、真实桌面跨重启/Keychain 演练、Anthropic 原生协议适配及 Provider 级网络治理指标。

## 2026-07-19 — OpenAI-compatible 隔离 Worker 子纵切

- `agent-provider-worker` 新增独立 OpenAI-compatible HTTPS 传输模块；公网端点只接受 HTTPS，本地开发服务只接受 literal loopback/localhost HTTP，并拒绝 userinfo、路径、查询和 fragment。
- 单次 Worker 协议支持 `/v1/models` 与 `/v1/chat/completions`，禁用重定向，限制超时、响应体、模型数、名称和 Tool Call 数；解析文本、Usage、停止原因和 JSON 字符串函数参数，并保留宿主 Request ID。
- API Key 使用脱敏、Drop 清零的 Worker-only payload；不进入命令行和环境变量。宿主 Adapter 要求任务 Credential Reference 与配置精确绑定，通过受限 Resolver 解析，并在写入 Worker stdin 后清零序列化缓冲区。
- 非成功响应不读取或回传正文、Endpoint 与 Header；认证、限流、超时和不可用映射为稳定错误。真实独立 Worker 进程测试证明 Bearer Header、模型排序去重、Tool Call/Usage 映射、重定向拒绝和错误脱敏。
- 本子纵切的 SQLite 配置、System Secret Store 桌面接线、Provider 管理 UI 与 Safe/Recovery Mode 命令门禁已由后续桌面纵切完成；通用 Sidecar capability Manifest 仍需继续实现。
- 验证：Workspace Clippy `-D warnings` 通过；完整串行 Workspace 测试除 Provider/Skill 两个真实进程监督目标在统一构建中出现一次既有 sidecar 争用外其余通过，随后两个目标分别串行复跑 3/3 与 6/6 通过。

## 2026-07-18 — Semantic Contract 与有界 Composition Graph 纵切

- 新增纯基础 crate `capability-contract`，定义严格 `nimora.capability-semantic-contract/1`：精确 capability ID、排序去重的 `requires/produces/preconditions`、数据等级、副作用、成本单位和离线可用性；未知字段、空输出、非法命名、非规范顺序和越界成本失败关闭。
- `creator-composition` 新增摘要绑定的 `nimora.capability-composition-graph/1` 与 `nimora.capability-semantic-plan/1`，在节点 256、深度 8、扩展状态 2048 和请求项 32 的硬上限内执行确定性最低成本搜索；搜索只读契约，不执行 Tool，也不从 Schema、标题、描述或模型文本推断语义。
- 内建生产 Tool 现在都有显式宿主维护的语义契约，一致性测试证明 Tool ID 与 Contract ID 一一对应；Character、Program 等路径显式声明安装、完整性和授权前置条件。
- Skill Agent Tool Contribution 可选携带语义契约，旧 Manifest 保持兼容；安装前复验 Contract ID、effect 和输出命名空间，第三方 Skill 不能冒充平台语义事实。
- Desktop 从同一实时 Tool Registry 合并内建契约与当前激活 Skill 的已验证契约；Skill 暂停后图节点立即撤销且摘要变化。Creator 的受信 System Message 只接收实现无关的 Catalog 与 Semantic Graph 快照。
- Gap Schema 已接收排序、去重、有界的 `availableSemanticInputs/requiredSemanticOutputs` 候选；模型无权声明前置条件已满足。宿主使用固定数据等级、副作用、成本策略和空可信前置事实运行确定性 Graph Planner，若找到完整路径则拒绝模型的伪缺口。
- Creator 结果同时投影 Exact-ID 与 Semantic Plan；Gap 保存时重新读取实时 Registry 和 Graph、重算双计划，并写入 `nimora.persisted-capability-gap/2`。该证据只覆盖候选语义映射和当前宿主事实，不宣称自然语言理解绝对完备。
- 经双重复验且明确要求平台扩展的 Gap 可由用户提交为 `nimora.capability-proposal/1`，原子写入 Workspace 的 `.nimora-proposals` 待评审队列；提交时再次重建实时 Catalog/Graph，不信任前端计划。Proposal 没有批准 ID、可执行标志、Handler、Grant 或 Registry 修改路径。

## 2026-07-18 — Creator Catalog Snapshot 与精确组合核验纵切

- 新增独立纯 Rust `creator-composition`：从生产 `ToolRegistry` 投影只有宿主复验 ID 与 effect、无第三方标题/描述、无 Schema、无 Backend、无原生对象的有界 `nimora.capability-catalog-snapshot/1`，按能力 ID 稳定排序并生成 SHA-256 摘要；内建工具与当前已激活 Skill Agent Tool Contribution 使用同一动态目录事实。
- Creator Provider 的受信 System Message 现在携带本次只读 Catalog Snapshot，并被要求只把快照中的精确 ID 视为已注册事实；模型仍无 Tool、文件、安装或执行权限。
- Gap 接受前由宿主运行确定性 Exact-ID Composition Planner；模型把当前已注册能力报告为缺失时失败关闭，真实缺失项返回 `nimora.capability-composition-plan/1`、目录摘要和分离的 resolved/missing 集合。
- Gap 保存时不信任前端或生成时旧证明：宿主重新读取当前动态 Registry、重新规划，再将 Gap 与 Composition Plan 原子保存为 `nimora.persisted-capability-gap/1`。
- UI 明确区分“精确能力 ID 已由宿主核验缺失”和“尚未穷尽自然语言目标的其他组合路径”。本纵切不是语义规划器、图搜索器或目标不可实现证明；后续仍需引入带前置/后置条件的数据流 Composition Graph。

## 2026-07-18 — Creator Capability Gap 真实纵切

- Creator 模型输出新增并列的严格 `nimora.capability-gap/1` 契约；目标无法由当前 Registry 表达时，模型只能返回缺失能力、所需操作、最低替代和平台提案需求，不能发明 Command、API 或可执行回退代码。
- `creator-draft` 在同一有界 JSON 信任边界解析 Draft 或 Gap，验证字段白名单、文本预算、能力命名、数量、重复项和替代方案；Gap 永远不能进入 Draft 的检查、批准或安装函数。
- Desktop Host 使用 `outcome`、互斥 `draft/capabilityGap` 投影；Creator Studio 为 Gap 提供独立警示界面，不渲染权限批准、原子安装或 Draft Workspace 保存入口。
- 用户可将经过复验的 Gap 原子保存为 `.nimora-drafts/capability-gap-<uuid>.json` 项目事实；报告只有结构化数据，无源码、运行 Grant、Secret 或宿主路径回传，Safe/Recovery 下仍可导出恢复资料。
- Creator Contract 8 项、Desktop Host 131 项、Frontend 43 项测试通过；最初仅有精确 ID 核验，后续已补齐 Semantic Plan 与不可执行 L4 Proposal 提交队列。平台维护者的评审、状态流转与实现项目关联仍未实现。

## 2026-07-18 — 外接 AI 原生扩展能力面基线

- 新增多模态感知、个人 API、个人数据应用、语义映射、协议适配、示教、Agent Team、上下文工程、模型路由、策略编译、数字孪生、实验优化、资产流水线、生态迁移、协作发布与数字遗产共十六类目标能力面。
- 所有新增产物继续归一到 `CreatorArtifact` 与可信 `ArtifactHandler` 生命周期；Creator AI 只能使用受控 Builder Tool 生成项目或 Patch，不获得 Node、Tauri、文件、数据库、网络、Provider 或 Secret 原生对象。
- 文档补充了跨能力组合、渐进式 Creator UI、离线与 Safe Mode、预算、取消、未知副作用和完整退役测试门禁。
- 本条是产品与架构基线，不是功能完成声明；通用 Capability Gap、Composition Planner、CreatorArtifact Handler 和 Simulation Pack 尚未形成真实代码纵切。

## 2026-07-19 — Automation 未知 AI 费用人工对账纵切

- SQLite 新增未知费用待办查询与不可变 `automation_cost_reconciliation` 决议审计；仅 `indeterminate` 任务可通过任务更新时间 CAS 对账，单任务和 Decision ID 均只能成功一次。
- 决议与费用账本在 Immediate 事务中原子提交；实际费用允许高于预留以如实记录已发生支出，结算后的真实费用继续参与每日预算封锁，不能通过低报或清零绕过治理。
- 桌面宿主生成 UUIDv7 Decision ID，Safe Mode 与 Recovery Mode 禁止决议；Browser Preview 只返回同 Schema 空态且拒绝写入，不伪造本机账本。
- Automation Workspace 提供逐任务实际费用、三类证据来源、不可修改/删除确认和最近决议审计；浏览器扩展连接正常但当前配置无可控标签页，真实卡片视觉验收已登记为待恢复项，不影响持久层、IPC 与前端契约门禁。
- SQLite 79 项、Desktop 142 项、前端 48 项及 TypeScript、生产构建、Bundle Budget、Clippy、Rustfmt 与 Diff Check 已通过；平台适配器另补 Browser 空态和两条原生 IPC 命令映射断言。

## 2026-07-18 — Automation 资源与 AI 费用治理可观测纵切

- SQLite Governance 新增按 Automation 和 UTC 日桶聚合的隐私安全快照，只返回活跃租约、最近启动时间及 reserved/settled/indeterminate 费用，不暴露任务内容、事件正文或 Provider 数据。
- 桌面只读 IPC 将当前 Catalog 策略与持久治理事实合并，确定性计算冷却剩余、并发占用和当日可用预算；Browser Preview 返回带真实 Schema 的空态，不伪造本机账本。
- Automation Workspace 新增资源治理卡片，分别展示并发、冷却、已结算、执行中预留、未知费用占用和可用预算；未知费用以显著警告说明不会自动按零释放。
- 用户可看到并发、冷却和预算拒绝的本地化原因，运行历史优先显示生产 Engine 的终态原因；SQLite 9 项治理测试、Desktop 125 项宿主测试和前端 40 项测试通过。
- 未知费用人工对账与不可变决议审计已在 2026-07-19 纵切补齐；Provider 账单抓取、外部证据附件留存与真实 Tauri 跨重启视觉/操作演练仍是后续独立验证范围。

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
- User Program 与 Skill 草案可在批准后由宿主构建受控临时包并复用生产原子安装器；Automation 定义新增严格版本并进入 SQLite 原子 Catalog；Theme 草案使用 `theme.local.*` 身份、固定 Token、WCAG 对比度门禁、生成完整性清单并复用 Asset 原子安装器。代码型三类产物安装后保持未授权、未启用，Theme 安装后保持未激活且不伪造权限；升级均保留上一版本。
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
| 桌宠窗口与交互 | 部分实现 | Tauri 透明双窗口、拖拽、置顶、穿透、托盘、安全模式、Click/Drag FSM、经典单击/双击/长按/右键/抚摸仲裁、三类照料、30 秒冷却；抚摸具备有界轨迹识别、独立命令/事件、隐私摘要与视觉反馈；Affinity/BondPoints 双轨长期成长、旧快照迁移、SQLite 原子持久化与真实关系卡均有 Rust/TS 测试；`DESKTOP_PET_EXPERIENCE.md` 已确立经典 QQ 宠物式独立桌面生命体基线 | 径向菜单、更多抚摸区域/角色响应映射、饱腹/清洁等完整 Needs、任务协作成长、关系阶段与纪念册、环境降扰、Windows/macOS 多屏/DPI、WebView 崩溃恢复与 8 小时长稳验证 |
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
| AI 辅助扩展创作 | 部分实现 | 专用无工具 Draft Agent、严格 JSON/生产校验、独立 Worker/DryRun、摘要绑定批准与 Workspace 原子保存；Program/Skill 复用生产安装器；Automation 使用版本化 SQLite Catalog，支持命令与行为 Diff、原子安装、强制停用、升级保留上一版、目录启停和回滚；已安装 Automation 的 Medium/High 运行采用参数、事件快照、定义版本、来源和有效风险绑定的一次性批准；真实 Capability Gap 经精确与语义双核验后可进入内容摘要绑定的本地提案队列，维护者可一次性接受分析、拒绝或绑定同簇规范提案标记重复；宿主提供确定性聚类和基于出现次数的分诊信号 | 多事件/虚拟时间/Mock 行为矩阵、文件/依赖/迁移 Diff、真实事件创作调试、删除与历史 UI、Connector/Agent/UI/资产生成、监控和 AI 修复闭环；提案业务影响/成本评分、签名身份、实现项目关联、聚类拆分合并与远程协作 |
| 事件驱动程序 | 部分实现 | 可信 Rust 订阅、`serial`/`drop`/`cancel-previous` Supervisor、迟到完成隔离 | 桌面纯 Supervisor 集成测试、执行历史与 Creator Studio 可视诊断 |
| 扩展与 Skill 生态 | 部分实现 | `skill-runtime` 实现严格 Manifest、精确 `commandAllowlist`、`onEvent:*` 与 `subscribe-events` 强绑定、命名空间 Contribution、精确授权、激活租约、恢复/quarantine/卸载和 `skill:<id>` requester；独立 `skill-host`/`skill-worker` 实现版本化 JSONL、清空环境、真实进程隔离、Boa JavaScript、取消/超时/输出预算、结构化 Command/Agent Task 计划及 Worker 故障驱动的 Contribution 撤销；`skill-package` 实现原子安装、完整库存与 SHA-256 复验、备份回滚；Desktop 实现安装、目录、授权、启停、回滚和执行 IPC，执行绑定 Activated Manifest 租约，Agent Task 接入统一 Module Adapter 与 Agent History，Command 整批预检 allowlist/注册风险后将 Safe/Low 经共享 Capability Gateway 执行，Medium/High 使用 SQLite 保存参数绑定的五分钟一次性整批批准；Journal 原子 claim、单事务拒绝/过期、完成/失败终态、重启 pending 恢复与 executing 中断、待批准列表 IPC；获准 Activated Skill 的事件声明使用独立有界 Runtime Event Bus 订阅与串行调度，Host 重建、停用、升级、回滚、Safe Mode 和故障撤销会话并取消在途 Worker/Provider，代际 ID 隔离迟到线程；活跃执行支持按 execution 取消 Worker、Command 后续副作用及当前 Provider Agent；Skill 执行元数据历史支持等待/完成/拒绝/取消/失败状态收敛、稳定游标分页与单条/全部隐私删除，取消终态不可被迟到结果覆盖，且不保存输入、源码、命令参数或 Agent 正文；拒绝/过期/重复批准 fail-closed，未知/未声明在副作用前拒绝，Recovery Mode 隔离扩展 | 跨文件系统与 SQLite 的安装崩溃一致性 Journal、发布者签名、OS CPU/内存沙箱、事件会话可视诊断与 UI/Tool 等 Contribution、管理 UI、官方番茄钟与提醒 |
| 自动化引擎 | 部分实现 | 独立 `automation-runtime`；版本化 Event Trigger、JSON Pointer Condition、顺序 Action 与 Policy；有界取消/超时；幂等且 retry-safe 的瞬时重试；失败逆序补偿；结构化 Run 结果；Backend 获得宿主生成且覆盖动作/重试/补偿的 Run/Automation/Action/Event/Trace 因果上下文；Engine 支持宿主预分配稳定 Run ID；SQLite Run Journal 在副作用前记录 running、原子完成、精确 interrupt、严格身份/状态转换，桌面启动恢复遗留 running 为 interrupted；主动作与补偿动作在副作用前整批执行宿主有效风险预检，未知命令失败关闭，Safe/Low 立即执行，Medium/High 进入参数、事件快照、定义版本、来源和 Run ID 绑定的五分钟一次性 SQLite 批准，Critical 拒绝；批准前不创建 Run Journal、不注册活跃运行、不调用 Backend，原子 claim 后使用同一 Run ID 从头执行；拒绝、过期、失败与 executing 重启中断有持久终态；待批准目录、批准/拒绝 IPC 与桌面 UI 已贯通；Live Run 宿主取消注册表、原生取消 IPC 与 TS 契约会置位父 Run 并按持久 Journal 级联取消活跃 Agent 子任务和 Provider Worker；原生 IPC 与浏览器同契约零副作用测试运行；自动化工作区 UI；Desktop Live IPC 通过共享 Capability Gateway 执行宿主批准的 Pet Action，Safe/Recovery、未知动作和风险低报 fail-closed；`automation-agent-bridge` 将固定 `agent.task.run` 经风险、幂等、不可信上下文门禁和统一 AgentTaskGateway 转为以 Automation Run 为根的共享 Trace/层级预算子任务；桌面 Submitter 已复用 Provider/Coordinator/Tool/Gateway/审批/历史链路，显式模型与 Tool Allowlist 贯穿确认续跑，根 Run + 幂等键阻止内存生命周期内重复提交；Rust/TS 测试 | 持久异步结果回填、跨进程运行中恢复、运行保留策略、并发/冷却/费用门禁、不可取消副作用补偿；扩展 Profile/Character/Program Action 宿主策略；Interval/日历/快捷键等触发器、分支与并行汇合、持久规则、虚拟时间与事件回放 |
| 开放 Gateway 与 Connector | 未实现 | Capability Gateway 为进程内用户代码提供窄能力边界 | 配对、Token/Scope、REST/WS/SSE、HTTP/UDP Sink、Source Connector、2 秒安全停机 |
| AI Agent 与 CLI | 部分实现 | Tool Registry、风险与批准、任务硬预算；Provider/Tool 单步协调；Provider 续跑的 Assistant Call 与 Tool Result 强关联协议；多调用 Turn 完整聚合器；Ollama Worker 结构化续跑载荷及真实独立 Worker 双轮 Tool Call 自动化；十三项生产模块工具，自动化定义零副作用验证、角色渲染能力脱敏摘要、Runtime 动作词汇发现、原生策略事务化 Profile 切换、资产复验与刷新回滚型角色切换、完整性复验且路径脱敏的用户程序发现、绑定精确版本批准的隔离程序执行、可扩展只读能力集合及共享 Capability Gateway 固定映射；Automation Context Admission 将受信目标与来源化不可信数据分离，实施段数/字节预算、高置信中英文注入拒绝，并把不可信任务限制为 draft 且无工具，桌面 Provider 保留 untrusted 消息位；完成任务历史的版本化 SQLite 仓储、稳定游标分页和隐私删除；桌面完成态写入、降级不篡改结果、Recovery 内存隔离、分页/删除 IPC、最近历史 UI 与 Preview 会话实现；CLI 完成态旁路写入、显式数据库导出、成对游标分页和单条/全部删除；统一桌面 Agent 结果契约；Provider 等待确认、多确认项展示、部分批准后剩余项回填、最后批准后 Provider 续跑与整组拒绝 UI；构建期嵌入 Ollama Manifest 信任摘要，桌面启动时复验 Manifest 与 Worker 后自动注册；受隔离 Worker 的 `/api/tags` 健康探测、严格模型目录预算、去重排序、真实 Provider 状态与模型选择；桌面宿主对写调用整组批准前零副作用，拒绝/过期级联撤销，全部批准后按原始顺序执行；Safe/Recovery Mode fail-closed；CLI 工具发现与 Ollama 接线；独立 sidecar、loopback-only SSRF、Worker 完整性发现、有界协议、超时取消强杀和跨进程 mock 测试 | 用户本机实际 Ollama 模型桌面验收、继续扩展生产工具覆盖面、OpenAI-compatible Adapter、发布者数字签名、持久计划恢复、跨来源完整 Prompt Injection 防护与 Unicode/编码混淆检测 |
| 包签名与 Registry | 未实现 | SHA-256 本地完整性锁和原子回滚 | 发布者签名、信任根、撤销、Registry、兼容检测、更新策略、离线 CLI 验证 |
| 安全与隐私 | 部分实现 | 安全模式、精确授权、路径/符号链接防护、Worker 隔离、审计边界文档；统一 `nimora-secret-store` 已提供严格非敏感引用、macOS/Windows/Linux 系统密钥后端、零化读取、幂等删除与可替换内存测试后端；Provider 凭据和 Auto Mode Context Cache 系统密钥已接线，缓存使用版本化 XChaCha20-Poly1305 信封与完整身份 AAD | Connector 凭据绑定与授权签名；OS 沙箱、网络目标策略、隐私面板、威胁模型自动门禁 |
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

Auto Host 上下文准备已接入持久 Context Cache：完整 continuation 经协议安全压缩后，以 Provider、模型、Plan revision、Workspace fingerprint、消息内容和数据等级进行精确命中；miss 写入沿用 SQLite TTL/LRU/容量治理。缓存 Payload 已升级为随机 nonce 的 XChaCha20-Poly1305 密文，索引身份、数据等级和时间边界全部绑定 AAD；桌面宿主通过系统 Secret Store 跨重启持有独立零化密钥，旧明文记录失效删除。尚未完成缓存轮换控制面与指标控制面。

Auto Host 每轮 Workspace 门禁已实现真实有界重扫；一致时才允许继续，漂移时以 Session/Checkpoint/Workspace 三重 CAS 原子暂停 Session 与 Task 并追加 successor，竞争写入整体回滚且零 Provider 释放。单轮结果提交服务已支持 Continue、Paused、Completed 三类结果，将 Session/Task 生命周期、完整 continuation 与单调 Checkpoint sequence 以 timestamp + sequence 双 CAS 原子提交。调用前 durable Turn Attempt 已精确绑定 Session、Checkpoint、timestamp 与请求指纹且禁止过期重领；结果事务原子消费 Attempt，重复 Begin、陈旧提交与崩溃遗留均 fail-closed，遗留 Attempt 转为 indeterminate 并阻断 continuation 自动恢复。该链路已接入桌面持久监督 Runner、聚合控制中心与人工处置 UI，持久缓存已完成系统密钥 AEAD 加密；剩余缺口是真实跨崩溃桌面演练和更完整的指标控制面。

Auto Host 已将上述独立能力组合为生产单轮执行 Facade：真实 Workspace 预检、加密 Context Cache、durable Attempt、Provider/Tool Supervisor 和原子 Commit 按固定顺序执行。真实 SQLite/Workspace 测试覆盖 Provider 完成、安全只读 Tool continuation、写 Tool 整批零派发暂停、Provider 失败 indeterminate 隔离及漂移前置退出；Checkpoint 同时支持验证历史 Tool Call/Result 结构而不持久化 Tool Descriptor。桌面后台有界监督循环及 Goal/Plan/Attempt 聚合查看、暂停、取消和人工对账 UI 已接通；尚缺 Goal/Plan 完整编辑和跨重启桌面端到端验证。
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

## 2026-07-18 — AI-generated accessible theme vertical slice

- Added `Theme` as a fourth Creator artifact with an exact model-output contract, strict parse, independent host review, one-time digest-bound approval, Workspace persistence and verified Asset Runtime installation.
- Generated themes are restricted to `theme.local.*`, bounded localized names and license declarations, the fixed nine-token `nimora.theme/1` schema, valid hex colors and host-owned WCAG contrast checks.
- Installation builds a standard manifest, theme payload and SHA-256 inventory, then reuses the atomic Asset Installer; it never grants capabilities and never activates the theme automatically.
- Creator Studio renders a token-scoped, inert preview card instead of injecting generated CSS into the application theme. The review and install copy distinguishes permissionless theme upgrades from executable artifact reauthorization.
- Focused evidence covers generated-package verification, low-contrast rejection, metadata control-character and length rejection, installed-version diff, two-file Workspace persistence, isolated preview rendering and TypeScript correctness.
- Connector, Agent Profile, Persona Pack, Widget, Browser Workflow, Spatial Presence Profile, Identity Profile, Purchase Draft, Temporal Goal Graph, Federated Evaluation Pack, Adapter Training Project, Device Continuity Plan and other documented AI-native outputs remain proposals until each has a production Runtime, deterministic validator, install lifecycle and tests; Creator must return a capability gap rather than fabricate them.

## 2026-07-18 — AI-generated core Profile vertical slice

- Added core scene `Profile` as a fifth Creator artifact with an exact `name + ProfilePolicy` contract; it accepts only existing domain modes, nullable bounded overrides and no capability declarations.
- The model cannot choose a runtime identity. Workspace persistence derives a stable draft directory from the canonical profile payload, while creation delegates to `ProfileService`, which generates the real UUID and commits the SQLite snapshot plus event transactionally.
- Independent review reruns both Creator and domain validation. A digest-bound one-time approval is consumed before creation; Safe and Recovery Mode remain fail-closed through the shared Creator entry.
- Creation never calls `switch_active`, so the active Profile and native window/sound policy do not change. Creator Studio exposes an inert policy preview and labels the action `原子创建（不切换）`.
- Focused tests prove strict frequency rejection, two-file atomic Workspace persistence, a real durable Profile creation with an unchanged active identity, and accessible frontend policy rendering.
- This artifact is the existing core scene Profile, not an Agent Profile. Agent tool identity, model routing, memory and delegation profiles still require a separate production contract and lifecycle.

## 2026-07-19 — OpenAI-compatible reasoning capability mapping

- Provider 配置新增默认关闭的显式 `effort -> provider value` 映射与 Mapping Version；旧 `nimora.provider-config/1` JSON 通过默认字段继续读取，不迁移或伪造能力。
- SQLite 在副作用前拒绝空映射、`auto`、空或越界版本、空或越界厂商值及控制字符；CAS 更新和持久恢复保留完整映射。
- 动态 Worker Provider Descriptor 从配置投影具体等级集合与版本，Provider Registry 继续承担请求 Mapping 的能力、等级和版本复验。
- OpenAI-compatible Worker 只把已验证 `provider_value` 写入 `reasoning_effort`；真实 loopback 测试证明无 Mapping 时不发送字段，且 Mapping Version 不越过 Worker 网络边界。
- 桌面 Provider 管理器提供明确风险提示和具体等级选择，浏览器预览仍不伪造本机配置或能力。Agent Run/Auto Mode 的策略选择与映射生成仍是后续可信纵切，当前不得宣称已经自动应用推理等级。
