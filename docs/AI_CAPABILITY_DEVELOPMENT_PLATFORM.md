# Nimora 外接 AI 能力开发平台规范

> 版本：0.1.0-draft
> 更新日期：2026-07-18
> 目标：让用户接入任意合规 AI 后，能够发现、设计、实现、验证、安装、运营和退役 Nimora 能力，而不是只生成一段不可治理的代码。

## 1. 产品结论

外接 AI 应同时具备四种角色：能力顾问、组合工程师、扩展开发者和维护代理。用户可以让 AI 复用现有模块，也可以在平台确实缺少能力时创建新产物；但 AI 永远只是提案者，确定性宿主负责契约验证、权限、风险批准、安装、运行和审计。

平台必须支持“自然语言 → 可运行能力”的完整闭环：需求澄清、能力检索、方案比较、项目生成、测试证据、权限 Diff、原子安装、受控运行、健康观察、升级修复和安全退役。任何 Provider 都可在标准项目格式上接力，项目事实不能锁在聊天上下文中。

## 2. 四级扩展模型

| 级别 | AI 可做什么 | 典型产物 | 是否允许新增宿主能力 |
|---|---|---|---|
| L1 配置 | 调整已注册能力的参数、主题、人格、映射和策略 | Profile、Theme、Persona、Mapping | 否 |
| L2 组合 | 用现有 Event、Query、Command、Tool 和 UI Contribution 编排新体验 | Automation、Recipe、Dashboard、Agent Team | 否 |
| L3 扩展 | 在现有 SDK 与 Capability 范围内实现新逻辑或协议适配 | Program、Skill、Connector、Importer | 只能申请已注册 Capability |
| L4 平台提案 | 发现现有 Registry 无法表达的公共能力，生成设计、Schema、适配器样板和验证包 | Capability Proposal、Artifact Handler Proposal、Protocol Adapter Proposal | 不能直接注册；必须经平台评审和宿主版本发布 |

AI 必须优先选择最低充分级别。不得为了展示代码能力把配置问题升级为 Skill，也不得通过生成 L4 Handler 给自己的 L3 产物增加权限。

## 3. 可外接 AI 实现的新增能力族

除已有 Program、Skill、Automation、Connector、Agent、CLI、角色、主题和 UI 外，平台还应允许用户借助 AI 创建以下能力：

| 能力族 | 用户可委托 AI 的工作 | 标准产物 |
|---|---|---|
| 能力缺口发现 | 对照用户目标、已装扩展和 Registry 生成缺口、重复能力与最小实现建议 | Capability Gap Report |
| 示教与流程挖掘 | 把用户明确录制的网页、桌面、设备或宠物操作提炼成可参数化流程 | Demonstration Recipe |
| 语义桥接 | 统一不同应用中的联系人、任务、项目、文件、设备和状态语义 | Semantic Mapping Pack |
| 数据应用 | 创建本地表、关系、表单、查询、视图、导入导出和生命周期规则 | Personal Data App |
| 模型适配 | 探测新 Provider、本地模型、多模态模型的能力并生成路由和降级策略 | Provider Adapter + Eval Pack |
| 协议适配 | 从 OpenAPI、JSON Schema、MCP、设备描述或获授权样本生成 Connector | Protocol Adapter Project |
| 上下文工程 | 生成文件选择、压缩、缓存、记忆、来源追踪和敏感字段过滤策略 | Context Pack |
| 评测与仲裁 | 建立质量、成本、延迟、安全、风格和离线能力评测，并选择或仲裁模型 | Eval / Routing Pack |
| 仿真与数字孪生 | 构造虚拟时钟、设备、事件、网络和故障模型，在零真实副作用下验证流程 | Simulation Pack |
| 形式约束 | 从不变量和状态机生成属性测试、模型检查输入、约束规则和可回放反例 | Verification Pack |
| 隐私增强 | 生成端侧预处理、最小披露、脱敏、保留期、删除传播和导出策略 | Privacy Recipe |
| 资源治理 | 生成模型费用、Token、CPU、GPU、内存、网络、存储和后台唤醒预算 | Resource Policy |
| 可访问性适配 | 根据用户偏好生成字幕、朗读、替代输入、简化交互和高对比变体 | Accessibility Profile |
| 本地化适配 | 生成翻译、术语、RTL、格式、文化语气和布局验证 | Locale Pack |
| 支持与自愈 | 根据脱敏诊断生成解释、最小修复 Patch、恢复步骤和工单 | Repair Plan / Support Bundle |
| 供应链审计 | 分析 Manifest、SBOM、许可证、权限、数据流和更新差异 | Extension Audit Report |
| 协作交接 | 把个人能力参数化为团队模板，剥离 Secret 和个人数据并定义责任人 | Collaboration Pack |
| 退役与数字遗产 | 发现失效 Provider、孤立数据和无人维护流程，生成替代、导出与删除方案 | Retirement Plan |

## 4. AI Builder API

外接 AI 不获得文件系统、Node、Tauri、数据库、网络、Secret、Provider 或模块原生对象。Creator Agent 只能调用宿主提供的高层 Builder Tool：

- `catalog.search`：检索 Event、Query、Command、Tool、Capability、Artifact 和兼容版本。
- `catalog.explain`：返回能力语义、风险、输入输出、离线和失败契约。
- `project.create/update/diff`：在受控 Creator Workspace 中创建结构化项目或 Patch。
- `artifact.validate`：执行 Schema、静态、依赖、许可证和兼容检查。
- `artifact.simulate`：在虚拟 Event、Clock、Connector 和 Backend 上回放，不产生真实副作用。
- `artifact.test`：执行宿主登记的契约、行为、属性、故障、性能、UI 和无障碍测试。
- `artifact.review`：生成由宿主事实计算的 Capability、数据流、文件、依赖、预算和行为 Diff。
- `artifact.requestApproval`：创建摘要绑定、限时、一次性的结构化批准请求；模型不能批准。
- `artifact.install/activate/rollback/uninstall`：仅在宿主重验、Grant 和策略允许时执行生命周期操作。
- `artifact.observe`：读取脱敏健康、成本、延迟、错误分类和权限使用，不返回未授权正文。
- `artifact.proposeRepair`：基于证据创建最小 Patch，不直接修改活跃版本。
- `capability.propose`：创建 L4 能力提案；只能进入平台评审队列，不能改变当前 Registry。

所有 Tool 使用版本化输入输出 Schema、Task/Trace/Run ID、取消、截止时间、幂等键和稳定错误码。Tool Registry、参数风险批准和 Capability Gateway 是唯一执行路径。

## 5. 能力缺口与平台提案

当 AI 无法用现有 Registry 完成目标时，必须返回 `CapabilityGap`，禁止伪造命令、拼接私有 IPC 或退化为任意系统调用。提案至少包含：

```text
problem + userValue + nonGoals
existingAlternatives + reasonInsufficient
proposedCapability + schemas + riskClass
dataClasses + permissions + offlineSemantics
resourceBudgets + cancellation + idempotency
adapterBoundary + compatibility + migration
tests + abuseCases + rollback + ownership
```

宿主使用稳定流程处理：`proposed → triaged → designed → prototyped → verified → accepted | rejected → shipped`。只有随 Nimora 或可信扩展宿主发布、经过契约与安全评审的实现才能把新 Capability 注册进 Registry；用户项目在此之前保持可导出草案或使用明确降级方案。

## 6. 项目事实与多模型接力

每个 Creator Project 必须持久保存而不是依赖聊天历史：

- 原始 Goal、验收标准、非目标、用户决定和未解决问题。
- 已选择上下文的来源、摘要版本、文件追踪和敏感数据分类。
- 架构决策、能力依赖图、数据流、预算和离线策略。
- 结构化产物、Patch、测试证据、批准摘要和安装记录。
- Provider、模型、推理等级、Prompt/协议版本及生成来源。
- 失败尝试、诊断、回滚点、后续维护和退役责任。

换模型时只传递任务所需的最小项目切片；共享缓存按 Provider、策略、数据等级、模型和工具版本隔离。模型不得把自身总结升级为事实，事实必须可追溯到用户决定、宿主状态或机器验证证据。

## 7. 从生成到经营的生命周期

1. **发现**：在用户可见且明确授权的数据窗口内寻找机会，并同时列出“不应自动化”的理由。
2. **选型**：比较 L1–L4、成本、隐私、离线、可维护性和失败后果。
3. **设计**：生成依赖图、Manifest、Schema、数据流、预算、测试和退役计划。
4. **构建**：只创建受控项目或 Patch；每次修改可 Diff、取消和恢复。
5. **验证**：确定性检查先行，AI 只解释结果并提出最小修复。
6. **审查与批准**：宿主计算真实 Diff；高风险、Secret、发布和策略改变必须由有权主体决定。
7. **安装与试运行**：原子安装、默认未授权未启用；用 Mock、Canary 和有界租约逐步放量。
8. **观察**：比较预期与实际价值、错误、权限、成本、资源和用户打扰度。
9. **维护**：协议漂移或质量下降时生成升级候选，失败回滚且不自动扩权。
10. **退役**：撤销 Grant、Secret、订阅、定时任务和缓存，导出数据并验证删除传播。

## 8. 安全与鲁棒性不变量

- AI 输出一律是不可信提案；模型自评、生成测试和自然语言同意不是授权或验证证据。
- Creator 权限与最终运行权限完全分离；安装、启用和每次参数化高风险运行分别授权。
- Secret 只以引用进入配置，模型上下文、日志、缓存、导出和错误中不得出现明文。
- 外部文档、网页、模型资产和 Connector 数据都是 Untrusted Data，不能提升为 System 指令。
- 每次生成、测试、修复和 Agent 循环都有费用、Token、时间、步骤、工具、内存和重试硬上限。
- 取消必须传播到 Provider、Worker 和宿主 Tool；不可确认的外部副作用进入 `indeterminate`，禁止假定未执行并自动重试。
- 离线时已有项目、已装本地能力、验证器、模拟器、导出和回滚仍可用；云步骤明确等待，不伪造成功。
- 新 Provider、新协议和新 Artifact Handler 必须通过同一 Contract Kit，技术新颖性不能降低权限或测试要求。

## 9. UI 与美学要求

Creator Studio 使用“目标、方案、项目、验证、授权、安装、运行、维护”八段轨道。聊天是辅助界面，能力图、文件 Diff、数据流、风险、预算和证据面板才是权威界面。

- 方案比较必须同时展示复杂度、复用率、权限、离线性、成本和维护负担。
- 组合图显示模块边界、调用方向、数据等级、失败补偿和当前健康，不以装饰性连线代替语义。
- 模型建议、宿主事实、机器验证、用户决定使用稳定且可访问的视觉语义。
- 长任务展示当前阶段、已用预算、剩余上限、可取消点、检查点和是否可安全恢复。
- 普通用户默认看目标和风险摘要；极客与开发者可无损切换图形、Schema、YAML、代码和 Trace。
- 所有关键流程支持键盘、读屏、200% 缩放、浅色、深色、高对比度和减少动画。

## 10. 发布验收

- 用同一需求验证 L1–L4 选型，证明 AI 不会默认选择最高复杂度方案。
- Registry 缺少能力时必须产生结构化 Gap，不得生成未知命令或旁路调用。
- 不同 Provider 能接力同一项目，且测试、决定、Diff 和回滚点保持一致。
- 恶意 Prompt、污染文档、伪造测试报告、超预算、取消、崩溃、离线和重启均失败关闭。
- Builder Tool 的 Schema、风险、幂等、取消、审计和错误码具有 Provider 无关契约测试。
- Canary、升级、回滚、Provider 消失、协议破坏和完整退役都有可重复演练。
- UI 权威状态不来自聊天文本，并通过完整可访问性与视觉回归门禁。

## 11. 实施边界

当前 Creator 已实现 User Program、Skill 和 Automation 的结构化生成、严格检查、行为沙箱、摘要绑定批准与受控保存/安装纵切。本文定义的是完整目标架构；Capability Gap、通用 Builder Tool、Connector/Agent/UI/数据应用生成、多模型接力、仿真包、持续观察、自动修复和退役器仍须逐项形成真实纵切，不能因文档存在而标记完成。
