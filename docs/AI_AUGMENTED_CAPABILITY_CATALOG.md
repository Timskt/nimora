# Nimora 用户外接 AI 能力目录

> 目标：让用户借助自选 AI 创建、组合、测试、迁移和维护扩展能力；AI 负责提案，确定性宿主负责契约、安全、授权、安装与运行。

## 1. 能力不只等于生成代码

平台应选择最小充分产物：能用设置完成就不生成脚本，能用 Automation 完成就不创建常驻 Skill，只有需要新协议或复杂状态机时才创建 Connector 或完整扩展工程。AI 还可生成配置、资产映射、测试、迁移、诊断和操作计划。

## 2. 完整能力矩阵

| 能力域 | AI 可完成的工作 | 产物与门禁 |
|---|---|---|
| 宠物行为 | 对话、动作、情绪、作息、主动交互、场景状态机 | Automation / Program / Skill；事件与命令白名单、回放 |
| 人格与记忆 | 人格包、语气、记忆提取、遗忘和隐私策略 | Persona / Memory Policy；数据分类、保留期、可解释删除 |
| 角色资产 | Live2D、VRM、GLTF、Sprite 动作表情映射和降级 | Asset / Import Profile；来源、许可证、格式、性能预算 |
| 主题与 UI | Token、皮肤、布局、Widget、面板、动效、无障碍变体 | Theme / UI Schema；Token 沙箱、可访问性、视觉回归 |
| 声音与多模态 | TTS 配置、提示音、字幕、口型和情绪映射 | Voice / Mapping；同意、版权、字幕、静音策略 |
| 自动化 | 事件—条件—动作、补偿、幂等、定时和场景编排 | Automation Graph；Dry-run、循环检测、风险 Diff |
| 用户程序 | Nimora SDK 脚本、状态逻辑、事件处理和测试 | User Program；独立 Worker、预算、Capability Grant |
| Skill | 命令、设置 Schema、Agent Tool Contribution、测试和文档 | Skill Package；签名、独立 Host、命名空间和租约 |
| Connector | 日历、Home Assistant、IDE、Git、Webhook、硬件和企业服务 | Connector；Secret 引用、网络域、速率和数据出口 |
| Agent | 角色 Agent、Subagent、Goal、规划器和交接协议 | Agent Profile / Team Graph；Tool、预算、递归和停止条件 |
| CLI | 命令、参数、交互向导、机器输出和 Shell 补全 | CLI Contribution；Schema、无 TTY、退出码契约 |
| 工作台与微应用 | 表单、看板、时间线、仪表盘、编辑器、浮窗工具与多步骤向导 | Workspace App / Widget；声明式 UI、状态 Schema、无障碍与渲染预算 |
| 数据处理 | 解析、清洗、转换、分类、摘要、脱敏、导入导出与可视化 | Data Recipe / Pipeline；类型契约、样本预览、血缘、可逆写入 |
| 知识与数据 | 本地摄取、索引、RAG、同步和删除传播 | Knowledge Pipeline；来源、去敏、离线降级 |
| 通信与协作 | 邮件、消息、日程、工单、审批、团队通知和人机交接 | Connector + Workflow；收件人预览、草稿优先、幂等发送与撤销策略 |
| 设备与环境 | 智能家居、串口、蓝牙、MIDI、Stream Deck、传感器和机器人 | Device Adapter；设备租约、指令白名单、速率、急停与仿真模式 |
| 开发工程 | 仓库理解、代码补丁、重构、Issue、Release、CI 优化和开发环境操作 | Dev Skill / Agent Project；Workspace 边界、Patch 审查、测试证据与 Git 策略 |
| 教育与创作 | 教程、练习、互动故事、直播辅助、素材流水线和创作反馈 | Content Project；来源、版权、年龄分级、事实与生成内容标识 |
| 可访问性 | 字幕、朗读、替代文本、简化交互、输入适配和个性化辅助 | Accessibility Profile；用户控制、可逆覆盖、隐私与 WCAG 门禁 |
| 运维诊断 | 日志解释、诊断查询、修复、备份和恢复演练 | Runbook / Repair Patch；只读优先、证据、回滚 |
| 测试质量 | 契约、属性、故障、UI、无障碍和性能测试 | Test Pack；AI 测试不替代人工安全不变量 |
| 迁移现代化 | SDK、Manifest、Provider、模型和资产格式升级 | Migration Patch；兼容矩阵、备份、双读、回滚 |
| 生态发布 | 包装、文档、示例、翻译、兼容检查、签名请求和商店发布草案 | Release Candidate；来源、SBOM、许可证、信誉与人工签名 |
| 上下文工程 | 为任务生成上下文选择器、压缩器、摘要策略、文件追踪规则、记忆分层和缓存策略 | Context Pack；来源引用、敏感字段过滤、失效规则、Token 与成本预算 |
| 模型评测与路由 | 构建模型能力探测、离线评测集、质量/成本/延迟路由、降级链和结果仲裁 | Model Policy / Eval Pack；固定样本、盲测、防提示污染、预算和人工基线 |
| Prompt 与协议包 | 生成版本化 System Prompt、结构化输出 Schema、MCP/Agent 协议映射和兼容适配 | Prompt/Protocol Pack；不可覆盖宿主指令、版本锁定、注入测试和回滚 |
| 安全与策略治理 | 生成组织策略、数据分类器、脱敏规则、权限建议、审批流和合规检查清单 | Policy Pack；AI 只能提案，宿主策略上限、授权和审计不可由生成物改写 |
| 身份与授权体验 | 设计角色模板、最小权限建议、临时授权、委托边界和权限解释 | Grant Template；主体、资源、作用域、期限和撤销语义由宿主强校验 |
| 浏览器与桌面操作 | 生成网页流程、桌面操作计划、表单填充、截图核验和可恢复任务 | Computer-use Recipe；窗口/站点范围、敏感操作确认、视觉证据和幂等恢复 |
| 数字孪生与仿真 | 为宠物、设备、家庭、直播或工作流建立可重放状态模型和故障场景 | Simulation Pack；虚拟时钟、确定性 Seed、无真实副作用和现实/仿真强标识 |
| 空间与可穿戴交互 | 生成多屏、AR、空间锚点、手势、眼动、穿戴设备与环境感知适配 | Spatial Profile；隐私区域、传感器租约、舒适度、无障碍和设备降级 |
| 数据契约与语义映射 | 从第三方 Schema 生成类型、字段映射、单位转换、兼容层和数据质量规则 | Schema Mapping；血缘、精度、时区、缺失值、破坏性变化和双读验证 |
| 隐私增强处理 | 生成本地预处理、最小披露、匿名化、保留期、删除传播和可验证导出方案 | Privacy Recipe；原始数据默认不离端、可逆性评估和重识别风险门禁 |
| 扩展市场治理 | 辅助审核扩展、聚类重复能力、检测恶意更新、生成信誉与兼容报告 | Registry Review；签名、封禁、信誉裁决和最终上架权归平台或管理员 |
| 用户支持与自愈 | 从用户授权的诊断包生成解释、最小修复 Patch、恢复步骤和支持工单 | Support Bundle / Repair Plan；默认脱敏、证据可追踪、修复可撤销且有限重试 |
| 本地化与文化适配 | 生成翻译、术语、RTL、日期数字格式、语气与文化敏感性变体 | Locale Pack；占位符契约、布局回归、人工术语库和区域策略 |
| 实验与个性化 | 生成本地实验、偏好学习、推荐规则和可解释个性化策略 | Experiment/Profile Patch；明确同意、最小样本、退出/重置和禁止暗黑模式 |

## 3. 扩展“扩展系统本身”

- 将成功 Automation 提炼成模板，再晋升为 Program 或 Skill。
- 从 Registry 与 Schema 生成类型 SDK、Mock、示例和契约测试，但不能生成生产权限。
- 为第三方模块生成 Adapter 骨架，映射到共享 Command/Event/Query Registry，不建立旁路。
- 为新模型协议生成 Provider Adapter 与能力探测器；密钥、网络和进程仍由宿主管理。
- 为新资产格式生成 Import Profile、转换流水线和 Renderer Contribution；解析保持进程隔离。
- 依据本地遥测与崩溃证据提出缓存、性能、降级和修复 Patch，不直接修改生产版本。
- 从自然语言需求生成新的 Skill、Automation、Program、Connector、Widget 和 CLI Contribution；若现有能力足够，优先组合而不是复制实现。
- 读取公开且版本化的 Registry Schema，生成跨模块编排；模块暴露的能力必须经过 Tool Registry 或 AgentTaskGateway，AI 不接触实现对象。
- 为能力创建评测集、回归样本、Mock、故障注入方案和运行健康规则，使生成物具备可持续维护证据。
- 生成发布材料、迁移指南、兼容声明和本地化包，但签名、信任等级与商店上线必须由宿主和用户决定。

### 3.1 用户外接 AI 的五种参与深度

1. **顾问**：解释、诊断和提出方案，不产生可执行副作用。
2. **搭建者**：生成声明式配置、Automation、主题、角色映射和数据 Recipe。
3. **开发者**：在受控 Workspace 生成 Program、Skill、Connector、Widget、CLI 和测试 Patch。
4. **操作者**：在明确 Goal、预算和授权策略下调用已注册能力；每次调用仍经过运行期门禁。
5. **维护者**：依据版本、遥测和故障证据升级、迁移、修复、回滚并提交发布候选。

参与深度不是权限等级。即使处于 Auto Mode，AI 也不能自行签发 Capability、读取 Secret、扩大数据范围、关闭安全策略或绕过不可自动批准的风险类别。

## 4. 组合项目

一个 Creator Project 可包含多种产物。例如“会议助手”由日历 Connector、会议前 Automation、专注人格场景、通知 Widget、总结 Skill 和本地记忆策略组成。统一依赖图描述每个节点的输入、输出、Capability、数据等级、失败语义、补偿、预算和版本约束。

AI 调用平台能力只经 Tool Registry、风险批准和 Capability Gateway；模块请求 AI 只经 `module-agent-adapter` 与 `AgentTaskGateway`。Program、Skill 和 Connector 不得获得 Provider、Node、Tauri、文件、网络或数据库原生对象。

## 5. AI 协作生命周期

1. **发现**：读取用户明确授权的能力目录与数据范围。
2. **选型**：说明配置、Automation、Program、Skill、Connector 或组合项目的取舍。
3. **设计**：产出 Manifest、数据流、权限、风险、预算和验收标准。
4. **构建**：只在内存草案区或受控 Workspace 生成 Patch。
5. **验证**：执行契约、语法、行为沙箱、多事件 Mock、故障和资源测试。
6. **审查**：展示文件、权限、数据出口、行为和依赖 Diff，以摘要绑定批准。
7. **安装**：宿主重新验证、原子安装并签发运行期最小 Grant。
8. **观察**：展示调用轨迹、成本、延迟、错误、权限使用和健康状态。
9. **维护**：AI 基于机器诊断提出最小 Patch；失败自动回滚，不扩大权限。
10. **演进**：升级先做兼容与迁移报告，支持 Canary、暂停、回滚和换模型续作。

## 6. 用户可直接委托的高级任务

- 从用户选择的本地事件窗口提炼重复操作为自动化。
- 把脚本提升为带 Manifest、设置页、测试、文档、许可证和升级路径的 Skill。
- 导入 Live2D/VRM 并生成动作映射、性能档位和缺失动作降级。
- 为 IDE、智能家居或硬件创建双向 Connector、断线重连和隐私策略。
- 为长期 Goal 生成多 Agent 团队、上下文边界、预算、文件所有权与验收回路。
- 生成离线替代方案，检查本地模型、缓存和依赖并明确降级。
- 审计第三方扩展的权限、数据流、SBOM 与行为，并安排隔离试运行。

## 7. AI 不可代替用户的决定

- 不可自行批准权限、许可证、发布签名、Secret 读取或关闭安全策略。
- 不可把聊天文本或“用户似乎同意”当成结构化执行凭证。
- 不可无限重试付费调用，或为完成 Goal 隐瞒风险、改变验收标准、跳过回滚点。
- AI 的测试、风险说明和文档只是候选证据；机器验证和用户明确决定才是权威状态。

## 8. 面向未来的适配

新模型、Agent 协议、设备、资产格式和 UI Runtime 通过版本化 Contribution Points 接入。Core 只依赖 Provider、Tool、Capability、Event、Command、Query、Asset、Renderer、Connector 与 Agent Task 契约。新生态必须声明能力探测、兼容范围、数据边界、资源预算、取消和降级语义，不能因技术新颖获得旁路权限。

## 9. 验收清单

- 每类产物都能导出、换模型续作、离线保存、取消、Diff、回滚和删除。
- 同一需求展示选型理由，不默认生成最高复杂度代码。
- 组合项目可追踪跨模块数据流、权限传递、调用链和失败补偿。
- AI 修复只提交 Patch，并保留原版本、诊断证据和可重复测试。
- 新 Provider 或 Contribution 无需修改 Core，并通过共享契约与安全测试套件。

## 10. AI 生成能力的统一元模型

为避免每新增一种能力就创建一条特权安装通道，所有 AI 产物都必须归一为同一 `CreatorArtifact` 元模型：

```text
identity + version + artifactKind + sourceProvenance
dependencies + compatibility + capabilities + dataFlows
budgets + offlineSemantics + failurePolicy + rollbackPolicy
schemas + files + tests + migrations + documentation
```

`artifactKind` 可以扩展，但安装器、Diff、批准、完整性、生命周期和审计语义不能分叉。新类型先注册 `ArtifactHandler`，实现确定性的 `validate / diff / simulate / install / activate / observe / rollback / uninstall` 契约；Creator AI 只能调用这些宿主动作，不能通过生成新的 Handler 给自己增加权限。

每个产物还必须携带机器可判定的完成证据：契约检查结果、行为样本、资源上限、风险项、兼容范围、离线结论和回滚演练。这样未来即使出现新的模型架构、Agent 协议、空间设备或代码运行时，也只增加适配器和产物类型，不需要重写 Core 的信任边界。

## 11. 最值得形成产品差异化的组合

- **自然语言到完整扩展工程**：需求、架构、代码、测试、权限 Diff、文档、安装、观察和维护一条链完成，而不是停在代码生成。
- **示教式能力创建**：用户演示一次网页、桌面或宠物操作，AI 提炼为可参数化 Recipe，并用回放证明没有漏掉关键步骤。
- **运行证据驱动修复**：AI 只读取脱敏调用轨迹、错误分类和版本事实，生成最小 Patch；Canary 失败自动回滚。
- **跨形态无损升级**：规则可晋升为 Automation，Automation 可晋升为 Program，Program 可封装为 Skill，始终保留来源、测试和兼容迁移。
- **个人能力市场**：AI 把用户私有流程包装成可分享模板，但自动剥离 Secret、个人路径、身份和私有数据样本。
- **模型可替换的长期项目**：项目状态、Goal、计划、决策、文件追踪和验证证据属于 Nimora，不锁在某个 Provider 的聊天上下文中。
