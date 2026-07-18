# Nimora 扩展与生态规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-18

## 1. 扩展类型

| 类型 | 是否执行代码 | 主要贡献 |
|---|---:|---|
| Asset Pack | 否 | 角色、皮肤、主题、声音、行为数据 |
| Skill | 是 | 命令、自动化、工具、UI、宠物行为修饰 |
| Connector Provider | 是 | Source/Sink/Duplex 协议适配 |
| Agent Pack | 默认否 | Persona、Tool allowlist、记忆和主动策略 |
| Bundle | 否 | 固定多个包及配置模板 |
| Local Script | 是 | 用户编写的事件处理、宠物控制与个人自动化 |

Agent Pack 默认只包含数据。如果需要代码能力，必须拆分为独立 Skill 并单独授权。

Local Script 是比完整 Skill 更轻的正式产品入口，但使用相同 Host、Capability、Command、审计和配额边界；规范见 [`PROGRAMMABLE_CONTROL.md`](PROGRAMMABLE_CONTROL.md)。模型格式扩展遵守 [`MODEL_RENDERING_IMPORT.md`](MODEL_RENDERING_IMPORT.md)。

## 2. 统一包模型

每个包必须包含：

- 唯一 `id` 和语义化版本。
- `spec`、类型、发布者和兼容范围。
- 本地化元数据、许可证和隐私声明。
- 依赖、可选依赖和冲突声明。
- 文件完整性列表和可选签名链。
- 配置 Schema 和迁移声明。
- 权限、Capability 和 Contribution。

包安装必须是事务性的：解析 → 校验 → 授权 → 暂存 → 健康检查 → 激活。任一步失败不得破坏已安装版本。

## 3. Skill 生命周期

```text
installed → resolved → authorized → activated
          ↘ incompatible
          ↘ permission-required
activated → suspended → activated
activated → crashed → quarantined
any → upgrading → activated | rolled-back
```

- 激活由 `onStartup`、`onCommand:*`、`onEvent:*` 等声明触发；声明任一 `onEvent:*` 必须同时申请并获得精确版本的 `subscribe-events` Capability，Host 不得从源码、Worker 输出或运行参数临时扩大订阅集合。
- 扩展停用时平台自动撤销所有 Contribution 和订阅。
- `onTick` 不是通用能力；高频行为必须使用专用行为 API。
- 扩展升级前先迁移配置；迁移必须幂等并支持回滚备份。

当前 `crates/skill-runtime` 已实现宿主无关生命周期核心：`nimora.skill/1` Manifest 严格校验、命名空间 Contribution、精确版本与精确 Capability 授权、激活快照、暂停撤销、显式崩溃恢复、五分钟三次崩溃 quarantine、用户显式解除隔离和非活跃卸载。`crates/skill-host` 与 `crates/skill-worker` 已实现版本化 JSONL 协议、每次激活独立进程、清空环境、取消、截止时间、输出预算和真实 Boa JavaScript Worker；启动前必须取得当前 Active Skill 的精确 Manifest 租约，单独持有一个语法合法的 Manifest 不能启动 Worker。Worker 只生成 Command 与 Agent Task 结构化请求计划，不持有 Provider、Gateway 或原生对象。超时、进程崩溃、协议与输出违规会进入生命周期崩溃计数并同步撤销 Contribution，宿主取消不误计为崩溃。

`crates/skill-package` 与 Desktop 已实现原子安装、宿主生成完整性锁、完整库存与 SHA-256 复验、旧 active 备份和回滚，以及安装目录、持久授权状态和 Runtime Host 的桌面管理 IPC。安装、升级和回滚均不自动授权；授权精确绑定 `skill_id + version + capabilities`，启用与授权分离。正常启动重新复验包与持久状态后重建 Host，损坏、丢失或状态不匹配的 Skill 不恢复；Recovery Mode 使用隔离目录、内存状态和空 Host。Desktop `execute_skill` 已通过真实独立 Worker执行复验源码，绑定 Activated Manifest 租约，把 Agent Task 计划接入统一 Module Adapter；Manifest 精确 `commandAllowlist` 中注册的命令经整批预检后送入共享 Capability Gateway，并按宿主真实风险批准。获准 Activated Skill 还可凭独立 `contribute-agent-tools` Capability 声明有界 Agent Tool：Tool ID 必须位于 Skill 命名空间，输入/输出 Schema、标题、描述、风险与副作用必须通过 Manifest 校验，映射命令必须同时出现在精确 `commandAllowlist`。Desktop 每次构建 Agent Catalog、Provider Tool Turn 或批准续跑时，从当前 Activated Contribution 租约生成“内建 + Skill”统一 Registry；声明风险低于宿主命令真实风险、ID 冲突、未知命令或已撤销 Skill 会在 Gateway 前失败。动态 Backend 只把 Tool ID 映射到已注册 safe Command，并仍经参数绑定批准和共享 Capability Gateway，不向 Skill 或 Provider暴露命令注册表与宿主对象。获准 Activated Skill 的 `onEvent:*` 由 Desktop 注册为宿主拥有的独立有界 Runtime Event Bus 订阅并串行调度；Host 重建、停用、升级、回滚、Safe Mode 或故障会撤销旧会话、Tool Contribution，并取消在途 Worker 和 Provider。独立 Skill Execution History 保持 metadata-only。尚未实现跨文件系统与 SQLite 的安装崩溃一致性 Journal、发布者签名、操作系统级 CPU/内存沙箱与管理 UI。

## 4. Capability 与 Permission

用户授权 Capability，平台内部校验 Permission。

| Capability | 可能映射的 Permission |
|---|---|
| `notification.send` | `os.notification` |
| `calendar.read` | `net.http` 或系统日历适配器 |
| `pet.animate` | `pet.control.animation` |
| `external.webhook.send` | `net.http` + destination policy |
| `application.open` | `os.application.launch` |

风险不能只由扩展声明。最终风险为 Tool、Capability、底层 Permission、参数和当前环境风险的最大值。

## 5. Host API

```ts
interface NimoraExtensionContext {
  extension: { id: string; version: string }
  commands: CommandRegistry
  events: EventClient
  storage: ScopedStorage
  capabilities: CapabilityClient
  subscriptions: DisposableStore
}
```

Host API 必须可序列化、可取消、可超时。扩展不能获得数据库连接、Secure Store 实例或 Core 内部对象。

## 6. Contribution 规则

- Contribution ID 使用 `<package-id>.<name>`。
- 注册时进行 Schema 和冲突检测。
- UI 贡献运行在隔离 iframe/WebView，使用主题变量和消息协议。
- Agent Tool 必须声明输入 Schema、输出 Schema、风险和副作用；当前 Skill Tool 仅能映射 Manifest 精确 allowlist 中的宿主注册命令，声明风险不得低于宿主风险。未来增加预览或补偿时必须扩展唯一当前 Schema，不得以可选旁路绕开批准。
- Automation Action 必须声明幂等性、超时、重试安全性和取消语义。
- Connector Provider 必须声明方向、传输安全、重试和数据分类支持。
- Skill 只有在 Manifest 声明 `agent_tasks`、精确授予 `invoke-agent-tasks` 且当前处于 Activated 时，Host 才能发放 `skill:<id>` requester；暂停、崩溃、quarantine 或卸载会立即撤销该身份。Skill 不得获得 Provider Registry 或直接执行模型，宿主必须用该身份进入 `module-agent-adapter`。

## 7. Registry

| 信任级别 | 要求 |
|---|---|
| Official | 平台签名、人工审核、持续测试 |
| Verified | 身份验证、签名、自动扫描、抽样审核 |
| Community | 签名或 hash、自动扫描、用户警示 |
| Private | 组织管理、可配置策略 |
| Local | 开发者模式，默认不自动更新 |

Registry 元数据包括兼容版本、权限变化、包大小、性能报告、隐私标签、SBOM、签名状态和撤回状态。

## 8. 更新与兼容

- Stable 默认只接收兼容更新。
- 权限扩大、发布者变化、包含原生代码或主版本升级必须重新确认。
- 保留最近一个可运行版本用于回滚。
- 撤回包停止新安装；是否禁用已安装版本由安全严重度决定。
- 平台提供 API deprecation 日志和兼容测试套件。

## 9. 创作者生态

- SDK、模板、Schema、模拟器和官方示例必须开源或公开可读。
- 提供本地 Registry 和 CI 校验命令。
- 支持付费、打赏、免费和组织内分发，但运行时不依赖商店在线。
- 商店排序综合质量、兼容、性能、维护活跃度和用户反馈。
- 官方扩展必须作为 SDK 的 dogfood，并通过同一套契约测试。

## 10. 推荐生态方向

- Productivity：番茄钟、会议、日历、待办。
- Creator：OBS、Stream Deck、直播互动、OSC。
- Smart Home：Home Assistant、MQTT、灯光。
- Developer：GitHub、构建状态、日志、IDE 状态。
- Companion：角色、皮肤、声音、人格和互动包。
- Enterprise：私有源、策略模板、团队提醒和审计。

## 11. Skill 包安装与运行租约

- Skill 包必须包含严格的 `nimora.skill/1` `manifest.json` 与 Manifest 动态声明的 JavaScript entrypoint；当前预算为最多 256 个文件和 16 MiB。
- `crates/skill-package` 生成宿主拥有的 `.nimora-skill-integrity.json`，记录包身份、版本、库存路径、大小与 SHA-256；包不能自行提供或覆盖该文件。
- 安装使用 staging、原 active 备份和原子切换；升级失败不得破坏当前 active，用户可恢复最近备份版本。
- 每次加载都复验完整库存，拒绝缺失、篡改、额外文件、重复路径、非 UTF-8 路径、符号链接和路径逃逸。
- 复验得到的 `ValidatedSkillManifest` 是 `SkillHost` 安装与 Worker Active Manifest 租约的唯一来源；不得从 UI、缓存或 Worker 回传重新构造 Manifest。
- Package Core 不代表发布信任已经完成；发布者签名、Registry 信任根、撤销、Desktop 安装 IPC、持久授权和管理 UI 仍需在同一契约上实现。
- `SqliteSkillStateRepository` 持久化精确 `skillId + version + capabilities`，并分离 `authorized` 与用户期望的 `enabled`；安装不得自动授权，且 `enabled` 不能在未授权时成立。启动时不得信任上次运行态，而要重新复验包并重建 Runtime 租约。
