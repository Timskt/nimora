# Nimora 系统架构

> 版本：0.1.0-draft  
> 更新日期：2026-07-18
> 状态：开发基线

## 1. 架构目标

- 保证桌宠窗口和核心互动始终可用。
- 允许资源、功能、协议和 AI Provider 独立扩展。
- 将不可信代码与 Core 隔离，并通过能力代理访问系统。
- 保持公开契约与语言无关，避免生态绑定内部实现。
- 允许 Windows 与 macOS 使用不同平台适配，但共享领域语义。

## 2. 总体结构

下图是目标拓扑，不代表每个方框都已实现。当前实现事实必须以 [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) 为准；尚未落地的 Local Gateway、Connector Runtime 和部分 OS Adapter 不得被调用方假设存在。

```mermaid
flowchart TB
  UI[Pet Window / Control Center / Palette / Tray]
  IPC[Typed IPC Boundary]
  CORE[Runtime Core\nPet / Command / Event / Profile / Policy]
  AUTO[Automation Engine]
  ASSET[Asset Runtime]
  SUP[Extension Supervisor]
  BROKER[Capability Broker]
  GW[Local Gateway]
  CONN[Connector Runtime]
  AGENT[Agent Runtime]
  STORE[(SQLite / Files / Secure Store)]
  OS[Windows / macOS Adapters]

  UI --> IPC --> CORE
  CORE --> AUTO
  CORE --> ASSET
  CORE --> SUP
  SUP --> BROKER --> OS
  CORE --> GW
  CORE --> CONN
  CORE --> AGENT
  CORE --> STORE
  CORE --> OS
```

## 3. 进程模型

| 进程 | 职责 | 故障策略 |
|---|---|---|
| Desktop/Core | 窗口、领域状态、策略、持久化、IPC | 必须保持存活；进入降级模式 |
| WebView UI | 渲染与配置界面 | 可重载，不持有唯一业务状态 |
| Skill Worker | 运行获准 Skill 的受限 JavaScript | 单执行独立进程；取消、超时或崩溃即终止并可 quarantine |
| User Code Worker | 运行用户程序的受限 JavaScript | 单执行独立进程；无 Node/Tauri/原生对象，仅返回结构化计划 |
| Model Importer Worker | 单次探测暂存区内的不可信模型 | 超时强杀；崩溃或拒绝不影响 Core |
| Agent Worker | Provider 请求、计划和工具循环 | 超时终止，不直接访问 OS |

独立进程只提供崩溃隔离，不等于操作系统安全沙箱。所有敏感能力必须经过当前统一的 Capability Gateway；“Broker”仅作为目标架构中的职责名称，不是第二条调用路径。

## 4. 模块边界

AI 模块能力不建立旁路。`crates/agent-tools` 负责生产 Tool Catalog 和固定 Adapter，`crates/agent-runtime` 负责 Tool admission、参数风险、批准指纹、任务预算与单步协调，`crates/user-code-gateway` 当前承载用户代码与 Agent 共用的 Capability Gateway。Agent Policy 与用户代码 Execution Policy 相互独立：Agent 使用 Task/Trace 关联和固定命令 allowlist，不继承程序私有存储命名空间；两者最终调用同一个 `CapabilityBackend`。桌面内部对象、Repository、Tauri 状态和任意命令字符串均不得进入 Provider 数据视图。

### 4.1 Runtime Core

Core 包含纯领域逻辑：Pet、Command、Event、Profile、Policy、Permission、Package Identity。Core 不依赖 Tauri、PixiJS、HTTP 框架或具体数据库驱动。

### 4.2 Platform Adapters

平台适配实现窗口、托盘、全局热键、通知、Secure Store、前台应用和系统空闲时间。适配器只能实现 Core 定义的 Port。

### 4.3 Asset Runtime

负责资源解析、继承合并、兼容检查、纹理缓存、动画图和回退。资源包不能执行代码。

第三方模型必须经过隔离 Importer 探测、校验和规范化，再交给版本化 Renderer Adapter。Pet Runtime 只依赖统一动作与表达语义，不直接依赖 Live2D、VRM 或 glTF 私有结构。

当前 Importer 探测 GLB 2.0 与严格 VRM 1.0：桌面宿主先将绝对普通源文件复制到一次性暂存区，再以清空环境、关闭 stdin、固定工作目录的子进程运行，限制 80 MiB 输入、1 MiB JSON、64 KiB 协议输出、执行截止时间以及节点、网格、材质和纹理数量。Worker 拒绝外部 URI、data URI、路径逃逸、错误 chunk 顺序和长度不一致；VRM 还必须声明 `VRMC_vrm`、使用 1.0 `specVersion` 并包含 meta 与 humanoid。安装确认重新探测新副本，Asset Installer 再重开复验格式与完整性后原子激活。Pet Overlay 使用按需 Three.js Adapter；VRM Runtime 独立动态加载并更新物理，卸载深度释放资源，失败回退内置角色。该实现仍不代表 Renderer 独立进程或 OS/GPU 沙箱，也未完成网格/纹理重写、许可证扫描、统一 expression/look-at/动作语义或 Live2D Runtime。

### 4.4 Extension Supervisor

负责安装状态、进程监督、激活事件、配额、心跳、升级、回滚和崩溃处理。扩展只能通过版本化 Host API 通信。

### 4.5 Capability Broker

统一执行文件、网络、通知、剪贴板、应用启动等敏感操作。Broker 校验扩展身份、Capability、Permission、目标策略、用户授权和调用参数，并写入审计。

### 4.6 Event 与 Command

- Event 表示已经发生的事实，不用于请求执行。
- Command 表示有意图的操作，返回结构化结果。
- Query 读取状态，不产生隐式副作用。
- Agent Tool、Automation Action 和 Gateway Endpoint 最终都映射到 Command。

当前 M0 运行时在状态变更前生成领域 Event，事件与发起 Command 共享 `traceId`。当前唯一 SQLite Schema 将宠物或 Profile 快照及对应 Event 在同一事务中写入 `event_outbox`，任一写入失败则整体回滚；事务成功后，事件再进入容量为 256 的有界进程内缓冲区并通过类型化桌面 IPC 消费。该缓冲区满时淘汰最旧事件，只负责进程内即时 UI。持久 Outbox 提供有界有序领取、消费者租约、过期重领、所有权 ACK、延迟重试、最大尝试次数死信、健康计数和有界已确认清理；领取使用 SQLite Immediate 事务防止并发消费者重复占有同一记录。具体 Connector 仍需实现幂等投递与退避策略，因此当前只能证明可构建 at-least-once 消费，不宣称 exactly-once 或已有外部投递服务。

这条规则防止四套执行逻辑分裂。

## 5. 数据与持久化

| 数据 | 存储 | 策略 |
|---|---|---|
| 设置/Profile | SQLite 或版本化文档 | 事务迁移、自动备份 |
| 宠物状态/关系 | SQLite | 定期快照，事件不作为唯一事实 |
| 资源包 | 内容寻址文件仓库 | hash 校验、原子切换 |
| 扩展配置 | 扩展命名空间 | 配额、Schema、迁移 |
| 密钥 | OS Secure Store | 配置仅保存引用 |
| 审计 | 轮转 JSONL 或 SQLite | 保留期和导出可配置 |
| 临时缓存 | Cache 目录 | 可安全删除和重建 |

当前宠物与 Profile 状态实现遵循 `runtime-core → runtime-app → persistence-sqlite` 依赖方向：领域层定义状态与不变量，应用层通过 `PetRepository`、`ProfileRepository` 端口组织用例，SQLite 适配器负责事务、版本校验、持久 Outbox 和 Online Backup API。状态与对应 Event 原子提交后才发布到内存与共享事件缓冲区，具体决策见 [`adr/ADR-008-versioned-sqlite-snapshots.md`](adr/ADR-008-versioned-sqlite-snapshots.md)。尚未发布的数据库采用唯一首版 Schema，一次事务创建宠物快照、Profile 快照、事务 Outbox 和用户程序权限表，不保留开发期中间迁移；测试覆盖初始化、未知版本拒绝、事件载荷反序列化、重复事件 ID 整体回滚、租约/ACK/重试/死信/清理，以及 WAL 状态在线备份恢复。真实版本升级前的备份调度、具体 Outbox 消费者和只读安全模式仍是后续工作。

Profile 激活属于“持久状态 + 原生窗口副作用”的复合操作。桌面适配器先应用候选窗口策略，再提交 Profile 快照；持久化失败时恢复原窗口策略。安全模式使用独立应用服务和共享事件总线，桌面菜单、IPC 和后续 Gateway、Connector、Agent Host 必须读取同一状态，不得维护各自的安全开关。

Profile 具有严格且可扩展的场景类型：`companion`、`work`、`focus`、`creator`、`developer`、`presentation` 与 `offline`。类型表达用户意图并为导航、动效、通知和资源预算提供默认值，但不是 Capability 黑名单；实际文件、网络、代码和模块访问仍由独立权限网关判断。用户可继续覆盖窗口、声音和主动频率，未来临时授权及自动切换规则也必须叠加在 Profile 之上，而不能由 `work` 等标签永久删减功能。只有独立的全局安全模式可以强制终止高风险能力。场景类型是当前契约的必填字段，未知值必须拒绝，避免隐式降级。

原生窗口移动事件不得逐帧写入 SQLite。桌面适配器使用单调递增 revision 对连续移动进行 200ms trailing-edge 合并，仅在窗口稳定后读取最终原生坐标并持久化；相同坐标不产生重复 Command/Event。托盘退出会在进程终止前同步刷新最终位置，兼顾拖拽流畅度、SSD 写放大控制和落点恢复可靠性。

托盘不是绕过应用层的特权入口。打开控制中心和恢复宠物交互在原生副作用成功后分别发布 `desktop.window.control-center-opened` 与 `pet.window.interaction-restored`；失败发布 `desktop.tray.action-failed` 诊断事件。恢复交互必须先显示窗口并关闭原生鼠标穿透，再提交内存窗口策略，不能仅修改 UI 或缓存状态。

Pet 交互状态转换由 Core 定义，而不是由 React 动画反推。点击进入 `interacting` 并发布 `pet.interaction.clicked`，600ms 后仅在状态仍未被新操作替换时回到 `idle`；拖拽进入最高优先级 `dragged`，原生拖拽结束后以一次持久化更新最终位置并回到 `idle`，发布 `pet.window.drag.started` 与 `pet.window.dragged`。Command 与对应 Event 共享 Trace ID，失败不得留下假事件。

`dragged`、`interacting`、`recovering` 属于不可跨进程延续的瞬态。运行时加载持久快照时会将这些状态归一化为 neutral idle，并在对外提供状态前重新持久化；恢复写入失败则启动失败，不允许以内存状态掩盖磁盘不一致。

## 6. 事件契约修正

事件 `source` 必须支持以下命名空间：

```text
core
skill:<package-id>
automation:<rule-id>
agent:<agent-id>
connector:<connector-id>
gateway:<client-id>
system:<adapter-id>
```

Source Connector 导入的外部事件先验证、规范化并分配新的本地 `id`；原始标识保存在 `data.externalId`。Sink 重试使用同一事件 `id` 和独立 `deliveryId`，避免混淆事件幂等与投递尝试。

## 7. 连接器分类

| 类型 | 方向 | 示例 |
|---|---|---|
| Source | 外部 → Event Bus | SSE Client、MQTT Subscribe |
| Sink | Event Bus → 外部 | HTTP Webhook、UDP、MQTT Publish |
| Duplex | 双向 | WebSocket、NATS |
| Gateway | 外部客户端调用本地能力 | REST、WS Server、SSE Server |

监听地址使用 `listenAddress`；远程目标使用 `destination`；本地网卡选择使用 `localBindAddress`。三个概念不得复用同一字段。

## 8. 扩展点

- `commands`
- `automation.triggers`
- `automation.conditions`
- `automation.actions`
- `agent.tools`
- `pet.behaviorModifiers`
- `pet.interactions`
- `ui.menuItems`
- `ui.settingsPanels`
- `ui.widgets`
- `connectors.providers`
- `assets.resolvers`

公开扩展点必须拥有 Schema、风险等级、生命周期和兼容策略。

用户脚本是正式扩展点：运行于独立受配额 Host，只能调用 SDK 注入 API。它与 Skill、Automation、Agent Tool 共用 Command Registry 和 Capability Broker，不允许直接访问 Node 敏感内建模块。

## 9. 失败与降级

- 资源失败：回退官方默认角色并报告资源诊断。
- UI 失败：重载 WebView，Core 状态不丢失。
- 扩展失败：停止扩展，撤销注册项，核心继续运行。
- 数据库迁移失败：恢复备份并以只读安全模式启动。
- Agent 失败：终止任务，保留命令和规则功能。
- 网络失败：连接器熔断，不阻塞 Event Bus。
- 事件风暴：按来源配额、背压和丢弃策略隔离。

## 10. 依赖规则

```text
UI / Adapters / Extension Hosts
          ↓
Application Services
          ↓
Domain Core
```

- Domain Core 不导入外层框架。
- 模块通过公开接口和 Schema 通信，禁止访问其他模块内部文件。
- 网络发送、系统调用和密钥读取必须经过统一端口。
- 禁止创建无边界的 `common` 或 `utils` 包。

## Agent 控制中心读模型

- React 不并发拼装 Job、Session、Goal、Plan、Checkpoint、Attempt 与 Resolution；Tauri 宿主通过 `auto_mode_control_center` 返回一次有界聚合。
- Job 是可重建的进程内投影，Session、Goal、不可变 Plan revision、Checkpoint、Attempt 与 Resolution 才是持久化事实；响应结构保留两者，禁止互相冒充。
- Session 必须读取其启动时绑定的历史 Plan revision，不能用 Goal 当前 Plan 替代；缺失绑定、损坏载荷或元数据分叉全部失败关闭。
- 聚合只属于宿主应用服务层，不进入纯领域 Agent Runtime，也不向 UI 暴露数据库连接、Provider、文件句柄或原生对象。
- 控制中心查询只读；Pause、Cancel 与 Attempt 对账继续使用独立命令、精确身份绑定和安全模式门禁。
- 聚合契约 `/2` 显式返回 `effectiveStatus` 与 `projectionStale`：前者只取持久化 Session 事实，后者表示进程内 Job 投影尚未收敛。React 不得自行推导或用 Job 覆盖事实状态。
- Pause、Cancel 与人工对账必须先经过宿主 `ensure_normal_mode` 和 Safe Mode 门禁；浏览器禁用按钮只属于体验防线，不能替代 IPC 授权边界。
- 人工对账理由在打开数据库前完成非空校验，决议成功后只重新读取事实，不自动重放 Provider、Tool 或外部副作用。

## 主题资产边界

- Theme 是数据驱动的 Asset 子类型，不是 CSS 或前端插件；Asset Installer 负责 Schema、Inventory、媒体类型和颜色白名单验证。
- Tauri 宿主持有主题选择事实并使用统一 Asset Selection 写锁串行化角色与主题切换；React 只消费 `ActiveThemeSnapshot`，不读文件系统。
- App Shell 通过固定 Token Adapter 映射主题，安装预览使用独立局部变量作用域，避免未确认资源污染全局状态。
- 安全模式、损坏选择和复验失败统一回退内置主题。主题故障不得阻止应用启动，也不得削弱权限、恢复和危险状态。

## Asset Selection 生命周期与当前收敛要求

角色、主题、声音及后续交互资产共享同一种生命周期：验证安装包、串行写入选择事实、每次使用前复验、按运行模式降级、原子持久化和返回无路径快照。宿主已落地类型化 `AssetSelectionPolicy` 与统一 Envelope Resolver/Persister，集中处理 Schema、内置 ID、Safe Mode、NotFound、损坏记录、非法 ID、真实 I/O 错误和临时文件原子替换，避免子类型语义分叉。

每个资产类型仍独立负责 `assetType`、内容复验、白名单与安全快照构造；Character 额外保留 Renderer 事件失败后的选择回滚和双错误报告。不得以动态字符串或无类型 `serde_json::Value` 换取表面复用，也不得让通用服务绕过 Voice、Theme、Character 各自的内容白名单。新增可选择资产类型必须声明静态 Policy，并复用统一生命周期契约测试。

Voice 是静态、无代码资产：`nimora.voice/1` 只允许 Inventory 内的有界 WAV/OGG Clip、动作 Cue、字幕与有限增益。UI 与用户代码只能获得已复验字节和元数据，不能获得文件路径、URL、解码器、网络、TTS Provider 或宿主对象。平台权限、危险、错误与恢复提示音不进入第三方 Cue 命名空间；Quiet Mode 必须在最终播放边界强制执行。TTS 属于独立 Provider Capability，不得伪装成 Voice 资产字段。

Inventory 媒体类型与扩展校验属于 Asset Installer 通用安全边界，Sprite 只能在其上增加图片白名单。任何新增媒体类型必须同时声明扩展、Header/容器探测、单文件预算、总包预算和负向测试，禁止复用带有具体资产语义的错误函数。
