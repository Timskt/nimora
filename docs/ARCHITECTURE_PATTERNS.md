# Nimora 架构模式与工程约束

> 状态：首个稳定版前的强制架构规范  
> 目标：用可验证的模式隔离变化、权限与故障，而不是堆砌抽象。

## 1. 使用原则

设计模式只有在同时满足以下条件时才能引入：存在明确变化轴或安全边界；接口拥有至少一个真实实现和可替换测试实现；失败语义可以稳定描述；增加的间接层少于它消除的耦合。禁止仅为了“正式工程感”创建无行为的 Manager、Factory、Service 或 Repository。

依赖始终指向稳定领域契约。Agent Runtime、资产领域、自动化领域和权限模型不得依赖 Tauri、React、SQLite、网络 SDK、模型 SDK或操作系统句柄。宿主负责装配，Adapter 负责翻译，Gateway 负责授权，Repository 负责持久化，领域对象负责不变量。

## 2. 核心模式矩阵

| 模式 | 应用位置 | 必须保持的不变量 |
|---|---|---|
| 六边形架构 | Agent、自动化、资产、用户程序 | 领域层不依赖宿主技术；端口可用内存实现测试 |
| Adapter | AI Provider、Live2D/VRM/glTF、Git、存储、系统能力 | 第三方错误、对象和凭据不得穿透领域边界 |
| Strategy | 风险评估、上下文压缩、缓存、扫描、模型动作映射 | 策略输入输出有界、确定、可审计；默认策略 fail-closed |
| State Machine | Goal、Plan、Task、Session、Attempt、安装和执行生命周期 | 非法跃迁拒绝；终态不可被迟到结果覆盖 |
| Command | Tool、CLI、用户程序模块调用、自动化 Action | 参数在批准后不可替换；命令具有稳定身份和风险 |
| Chain of Responsibility | Tool Registry → Risk → Approval → Capability Gateway | 任一门禁拒绝后零副作用；后级不能扩大前级权限 |
| Repository | Goal、Checkpoint、Snapshot、Cache、History | 索引与 Payload 复验；陈旧写入 CAS 冲突 |
| Unit of Work | Session/Checkpoint/Attempt/Workspace 联合提交 | 一个业务结果只产生一个原子持久状态 |
| Facade | Auto Mode 单轮 Host、桌面 IPC 应用服务 | Facade 只编排，不复制领域规则或绕过 Gateway |
| Observer / Event Bus | 宠物状态、UI、自动化、扩展事件 | 类型化事件、有界队列、订阅租约和迟到隔离 |
| Microkernel / Plugin | Skill、用户程序、皮肤、角色、Provider、Connector | 核心只暴露版本化 Contribution 和最小 Capability |
| Bulkhead | Worker、Provider、渲染预览、插件执行 | 独立进程/资源预算；单模块故障不拖垮宿主 |
| Circuit Breaker | 网络 Provider、Connector、更新和云同步 | 明确失败阈值、冷却与半开探测；不得吞掉业务失败 |
| Saga / Compensation | 安装、发布、跨模块自动化、外部副作用 | 每一步记录结果；可逆步骤补偿，不可逆步骤先批准 |
| CQRS | Catalog/Inspect 与 Install/Switch/Execute | 查询默认只读；写路径独立准入、审计和幂等控制 |

## 3. Agent 与 CLI 组合

`AutoModeExecutionService` 是应用层 Facade：依次执行 Workspace 预检、Context Strategy/Cache、durable Turn Attempt、Runtime Supervisor 和原子 Commit。`AutoModeTurnSupervisor` 是领域协调器；Provider 与 Tool Backend 是 Adapter；Tool Registry、Risk Evaluator 和 Capability Gateway 构成责任链；Session、Task 与 Attempt 是显式状态机。

`AutoModeLoopService` 在单轮 Facade 之上实现 Cooperative Scheduling：每个宿主批次限制为 `1..=256` 个 Turn，完成、业务暂停、Workspace 漂移或错误立即停止；达到批次上限只返回 `yielded` 并保留持久 Running 状态，不能伪装成 Pause，也不能绕过 Session 自身 Cycle/Budget 上限。下一批次仍必须重新执行完整 Workspace、Cache、Attempt 与 Gateway 流水线。

桌面 `resume_auto_mode_turn` 也是应用层 Facade：负责 Normal/Safe Mode 门禁、恢复、生产依赖装配和一次有界执行，不重新实现风险判断、Tool 准入或持久化不变量。Provider 与 Tool 通过 Registry 发现，真实模块副作用只允许经过 Capability Gateway；Session、Checkpoint、Attempt 与 Workspace 的结果由 Repository 和 Unit of Work 原子提交。该入口不创建第二套 Agent 执行栈，也不把 Tauri、SQLite、Provider SDK 或原生对象泄漏到领域层。

Attempt 创建后，Provider 或 Tool 的未知结果使用“不可确定结果隔离”模式：不自动重试、不释放 continuation、不通过超时租约重新领取。人工对账或确定性恢复是唯一后续入口。该规则优先于可用性和自动推进速度。

上下文压缩与缓存使用 Strategy + Content Addressing：Goal、约束、Plan、证据、Workspace、Provider、模型和消息协议共同组成身份。缓存命中不能扩大数据等级，也不能替代每轮 Workspace 重扫。

新增模式必须证明具体变化轴。Provider、模型、角色渲染器、缓存、上下文压缩和 Workspace 扫描适合 Strategy/Adapter；Goal、Task、Attempt、安装和扩展生命周期适合 State Machine；跨模块动作适合 Command + Gateway。禁止用全局 Service Locator 隐藏依赖，禁止让 UI 直接访问 Repository，禁止为了“以后可能用到”提前制造只有一个空实现的接口。

## 4. 扩展点标准

新增 Provider、角色格式、皮肤、Skill、Connector 或用户程序能力时，必须提供：版本化 Manifest；纯数据 Descriptor；Adapter；Capability 声明；风险与数据分类；资源预算；生命周期和撤销；故障回退；兼容性探测；Contract Test；至少一个损坏输入测试。扩展不得获得原生数据库连接、Tauri Handle、Provider 凭据、Node 对象或任意文件/网络能力。

新增模块调用 AI 时，统一经过 `module-agent-adapter` 与 `AgentTaskGateway`。新增 AI 调用模块时，统一经过 Tool Registry、风险/批准和 Capability Gateway。双向调用均继承 Trace、预算、取消和审计，不允许模块直连 Provider。

## 5. UI 模式

UI 使用状态容器 + View Model + Design Token，而不是让组件直接调用原生能力。异步操作必须表达 idle、loading、success、empty、partial、waiting-for-confirmation、offline、recovery 和 error。危险动作使用参数预览与可撤销反馈；长任务使用可取消进度；桌宠 Overlay 与 Control Center 共享领域状态但不共享组件内部可变状态。

主题和自定义皮肤使用 Token Strategy，不允许插件注入任意全局 CSS。角色渲染使用 Renderer Adapter；Sprite、glTF、VRM、Live2D 共享动作词汇和生命周期，不共享第三方引擎对象。

## 6. 禁止的反模式

- 禁止 Service Locator、跨模块全局可变单例和无边界事件广播。
- 禁止把数据库行、SDK Response、Tauri Command 参数直接当领域模型。
- 禁止“万能 Manager”、布尔参数驱动的巨型 Service 和跨领域 Repository。
- 禁止捕获错误后静默继续、无限重试、未知副作用自动重放。
- 禁止为了尚未发布的旧 Schema 制造兼容层；稳定版前维护唯一当前契约。
- 禁止仅 Mock 自身实现的测试；关键边界必须包含真实 SQLite、进程、文件或渲染证据。

## 7. 架构适应性测试

CI 必须逐步加入依赖方向检查、禁止依赖扫描、协议 Schema Contract、Repository 索引/Payload 篡改测试、状态机性质测试、Gateway 零副作用拒绝测试、Worker 进程故障测试和扩展兼容套件。代码审查必须回答：变化轴在哪里、权限在哪里收敛、未知结果如何处理、资源如何释放、怎样证明替换 Adapter 不改变领域规则。

## 8. 模式决策记录

跨域新模式或依赖方向变化必须新增 ADR，至少记录 Context、Decision、Alternatives、Consequences、Security、Migration 和 Verification。局部重构无需 ADR，但不得违反本文件和 `docs/ARCHITECTURE.md`。模式一旦不再减少耦合，应删除而不是保留“历史包袱”。

## 9. 当前架构不足与强制纠偏

当前方向正确，但还没有达到可长期扩展的完成态，以下不足必须作为架构工作而非普通待办处理：

1. `apps/desktop/src-tauri/src/lib.rs` 同时承担装配、IPC、生命周期、多个领域应用服务和大量测试，已经接近 Modular Monolith 的组合根上限。新增纵切必须优先抽取领域 Application Service 与版本化 DTO 模块，Tauri 层只保留参数翻译、State 获取和错误映射，禁止继续加入跨域业务规则。
2. `AutoModeLoopService` 已有领域续体，但桌面尚无统一 Job Supervisor。Supervisor 必须管理 Job 身份、Session 唯一性、调度公平、取消传播、退出收敛、休眠恢复和版本化快照；不得用 detached thread 或反复调用同步 Resume Facade 代替。
3. `AuthorizationGrant` 与推理策略是正确领域模型，但尚未成为所有执行入口的强制 Admission Context。接入前任何“全部权限无人值守”只能描述为设计能力，不能标记已完成。
4. 当前 Registry/Gateway 边界能阻止明显旁路，但缺少自动架构适应性测试。必须加入禁止模块直连 Provider、禁止扩展依赖 Tauri/SQLite/Node 原生对象、禁止 UI 访问 Repository 的依赖扫描。
5. 状态、Outbox、Agent History、诊断与审计已经分别可靠，但跨存储删除、导出、保留期和 Trace 一致性尚未形成 Data Lifecycle Coordinator。用户执行“删除记忆/历史”时必须证明缓存、索引、导出与投影同步失效。
6. Renderer Adapter 支持扩展方向，但 glTF 大包体警告说明前端资源治理尚未闭环。Live2D/VRM/glTF 必须按需加载，并建立 CPU、内存、显存、帧率、首开时间和后台降频预算。
7. 常规 CI 已节省分钟，但路径过滤和风险驱动平台验证仍需自动生成“建议矩阵”，由里程碑负责人一次触发；不能依赖记忆决定是否跑 macOS/Windows。

每项纠偏都必须包含可执行测试或机器检查。只有文档、接口或空实现不能关闭风险；关闭证据记录到 [`MILESTONE_REVIEWS.md`](MILESTONE_REVIEWS.md) 和 [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md)。
