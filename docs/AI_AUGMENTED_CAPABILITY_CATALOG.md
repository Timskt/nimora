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
| 知识与数据 | 本地摄取、索引、RAG、同步和删除传播 | Knowledge Pipeline；来源、去敏、离线降级 |
| 运维诊断 | 日志解释、诊断查询、修复、备份和恢复演练 | Runbook / Repair Patch；只读优先、证据、回滚 |
| 测试质量 | 契约、属性、故障、UI、无障碍和性能测试 | Test Pack；AI 测试不替代人工安全不变量 |
| 迁移现代化 | SDK、Manifest、Provider、模型和资产格式升级 | Migration Patch；兼容矩阵、备份、双读、回滚 |

## 3. 扩展“扩展系统本身”

- 将成功 Automation 提炼成模板，再晋升为 Program 或 Skill。
- 从 Registry 与 Schema 生成类型 SDK、Mock、示例和契约测试，但不能生成生产权限。
- 为第三方模块生成 Adapter 骨架，映射到共享 Command/Event/Query Registry，不建立旁路。
- 为新模型协议生成 Provider Adapter 与能力探测器；密钥、网络和进程仍由宿主管理。
- 为新资产格式生成 Import Profile、转换流水线和 Renderer Contribution；解析保持进程隔离。
- 依据本地遥测与崩溃证据提出缓存、性能、降级和修复 Patch，不直接修改生产版本。

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
