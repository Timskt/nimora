# Nimora 文档控制中心

> 版本：0.1.0-draft  
> 更新日期：2026-07-18
> 所有者：产品与架构负责人

## 1. 文档优先级

发生冲突时按以下顺序裁决：

1. 已接受的 `docs/adr/ADR-*.md`
2. `docs/SECURITY_PRIVACY.md` 中的安全红线
3. `docs/ARCHITECTURE.md` 中的系统边界与依赖规则
4. `docs/ARCHITECTURE_PATTERNS.md` 中的模式选择、扩展点与反模式
5. `docs/PRODUCT_SPEC.md` 中的功能与验收要求
6. 专项契约文档
7. 根目录原始需求文档

原始需求统一保存在 `archive/original-docs/`，只用于追溯。新开发只能以本目录文档和已接受 ADR 为依据。

全量开发进度、证据和真实缺口以 [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) 为入口。优先级只决定施工顺序，不允许从该矩阵静默删除有价值能力。

AI 与其它模块的双向调用以 [`AI_MODULE_INTERACTIONS.md`](AI_MODULE_INTERACTIONS.md) 为开发约束；Desktop、Automation、Skill、Connector 和用户代码不得直接调用 Provider 或建立绕过 Capability Gateway 的执行旁路。

面向广大用户的持续需求发现、新技术准入、Adapter 替换、稳定版前后兼容和退出演练统一遵循 [`FUTURE_EVOLUTION_GOVERNANCE.md`](FUTURE_EVOLUTION_GOVERNANCE.md)。

Coding Agent 竞品能力吸收、持久 Goal、无人值守预授权和模型推理策略分别以 [`AGENT_COMPETITIVE_DESIGN.md`](AGENT_COMPETITIVE_DESIGN.md)、[`AUTO_AUTHORIZATION.md`](AUTO_AUTHORIZATION.md) 与 [`MODEL_REASONING_POLICY.md`](MODEL_REASONING_POLICY.md) 为实现基线。

## 2. 规范用词

- **MUST / 必须**：违反即不兼容或不能发布。
- **SHOULD / 应该**：除非有记录明确的理由，否则必须遵守。
- **MAY / 可以**：可选能力。
- **Core**：不依赖 UI、Tauri 或具体网络库的核心领域与契约。
- **Capability**：用户可理解的业务能力，例如 `calendar.read`。
- **Permission**：实现 Capability 所需的底层权限，例如 `net.http`。
- **Contribution**：扩展向平台注册的命令、动作、触发器、工具、资源或 UI。
- **Asset Pack**：不执行代码的资源包，包括 Character、Skin、Theme、Voice 等。
- **Extension**：执行代码或提供协议适配的 Skill、Connector Provider 等。
- **Source Connector**：从外部系统导入事件。
- **Sink Connector**：向外部系统投递事件。
- **Character Model**：格式无关的角色身份、动作、表达、渲染能力和资源预算模型。
- **Renderer Adapter**：将统一角色语义映射到序列帧、Live2D、VRM/glTF 等具体运行时的版本化适配器。
- **Importer**：在隔离边界内探测、校验、转换和规范化第三方模型的组件。
- **Local Script**：用户编写、运行于受限 Host、仅通过平台 SDK 调用能力的代码扩展。
- **Live3D**：产品语境中的实时 3D 角色能力，优先采用 VRM/glTF，不代表新的私有格式。
- **Agent Tool**：模块通过 Manifest 注册、由参数风险与 Capability Gateway 共同约束的 AI 可调用能力。

## 3. 契约版本规则

- 每个公开契约使用独立版本：`nimora.event/1`、`nimora.skill/1`、`nimora.asset/1`。
- 首个稳定版前，契约变更直接修改唯一当前 Schema、实现、夹具和文档，不保留开发快照适配层。
- 首个稳定版真实发布后，才以已发布契约和真实生态为依据制定版本演进规则。
- Schema 文件是机器事实来源，Markdown 负责说明语义和示例。
- 所有持久化配置必须包含 `schemaVersion`；首版前未知版本直接拒绝，开发数据按当前 Schema 重建。

## 4. 变更流程

1. 修改需求或公开契约前创建 ADR 或 RFC。
2. 同时更新 Schema、示例和测试夹具；首版前不得为开发快照编写迁移。
3. 运行文档链接检查、Schema 校验和契约测试。
4. 首版前在变更日志中标记 Added、Changed 或 Security；真实稳定版发布后才启用 Deprecated 与 Removed 生命周期。
5. 安全红线只能通过新的 Accepted ADR 调整。
6. 所有规范变更按 [`GIT_WORKFLOW.md`](GIT_WORKFLOW.md) 经受保护分支、评审和可追溯提交进入主干。

## 5. 实现阶段生成的文档

- `docs/API_GATEWAY.md`：从实际 OpenAPI 自动生成，并补充可执行鉴权示例。
- `docs/SCHEMA_CATALOG.md`：从版本化 Schema 自动生成，禁止手工复制字段表。
- `docs/CREATOR_GUIDE.md`：从可运行 Creator Studio 和示例包生成操作教程。
- `docs/USER_GUIDE.md`：从稳定界面和经过验证的用户路径生成最终手册。

这些文档依赖真实代码或界面产物，不属于当前规格缺口。对应功能实现的 Definition of Done 必须包含生成和校验这些文档。

## 6. 完整性检查维度

任何新功能评审必须覆盖：产品价值、视觉与交互、设计令牌与组件状态、数据模型、权限隐私、离线行为、失败恢复、性能预算、可观测性、跨平台、无障碍、国际化、部署升级、测试和生态影响。首个稳定版前只维护唯一当前契约，不评审假想兼容迁移。UI 质量以 [`UI_DESIGN_SYSTEM.md`](UI_DESIGN_SYSTEM.md) 为强制门禁，功能储备以 [`FEATURE_EVOLUTION.md`](FEATURE_EVOLUTION.md) 管理，长期需求与技术替换以 [`FUTURE_EVOLUTION_GOVERNANCE.md`](FUTURE_EVOLUTION_GOVERNANCE.md) 管理；施工顺序不构成范围删除或发布完成声明。
