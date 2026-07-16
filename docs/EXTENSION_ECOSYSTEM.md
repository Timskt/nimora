# DeskPet 扩展与生态规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-17

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

- 激活由 `onStartup`、`onCommand:*`、`onEvent:*` 等声明触发。
- 扩展停用时平台自动撤销所有 Contribution 和订阅。
- `onTick` 不是通用能力；高频行为必须使用专用行为 API。
- 扩展升级前先迁移配置；迁移必须幂等并支持回滚备份。

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
interface DeskPetExtensionContext {
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
- Agent Tool 必须声明输入 Schema、输出 Schema、风险、预览和补偿能力。
- Automation Action 必须声明幂等性、超时、重试安全性和取消语义。
- Connector Provider 必须声明方向、传输安全、重试和数据分类支持。

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
