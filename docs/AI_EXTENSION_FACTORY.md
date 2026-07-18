# Nimora 外接 AI 能力工厂

完整能力与场景矩阵见 [`AI_AUGMENTED_CAPABILITY_CATALOG.md`](AI_AUGMENTED_CAPABILITY_CATALOG.md)。

> 状态：产品与架构规范  
> 更新日期：2026-07-18  
> 目标：让用户借助自选 AI 安全地创造、验证、安装、维护和演进 Nimora 可扩展能力，而不是只生成一段不可治理的代码。

## 1. 产品定位

外接 AI 是 Creator Studio 中的“能力工厂”，不是拥有桌面全权限的聊天框。用户可以接入本地模型、云模型、企业模型网关或兼容 Provider，由专用 Creator Agent 把自然语言意图转换为版本化、可审查、可测试、可回滚的项目产物。

系统必须支持普通用户的引导式创建、二次元用户的角色创作、极客的自动化组合和开发者的完整工程模式。四类体验共享同一生产契约、安全边界和安装流水线，不维护能力不同的阉割版运行时。

## 2. 可由 AI 创建或扩展的产物

| 领域 | AI 可完成的工作 | 产物与边界 |
|---|---|---|
| User Program | 编写事件响应、角色控制、数据整理和模块编排代码 | 受限 Worker 中运行，只能调用获准 SDK Capability |
| Skill | 生成 Manifest、命令、Agent Tool、设置 Schema、测试和文档 | 独立 Host；贡献点必须注册，不可动态越权 |
| Automation | 从自然语言生成触发器、条件、动作、重试、补偿和通知 | 开放 YAML/JSON；先 dry-run 和事件回放 |
| Connector | 对接日历、消息、IoT、Webhook、MCP、企业 API 和本地服务 | 凭据由平台保管；网络域名和数据方向显式授权 |
| Agent / Subagent | 定义角色、Goal 模板、计划策略、Tool allowlist、预算和交接协议 | 只能经 `AgentTaskGateway` 调用 Provider 与平台工具 |
| Skill 模板 | 把用户成功的脚本提升为可配置、可发布的 Skill 工程 | AI 生成迁移说明、权限 Diff、测试和发布清单 |
| 角色行为 | 生成性格、状态机、记忆规则、动作映射、表情和对话风格 | 行为与模型资源解耦；敏感记忆策略单独批准 |
| 角色资产 | 辅助制作 Sprite 图集、主题、声音、VRM/glTF 动作映射和 Live2D 参数映射 | 资产是数据；版权、来源、格式和预算必须验证 |
| UI 扩展 | 生成声明式面板、表单、仪表盘、命令面板项和 Overlay Widget | 只使用平台组件与 Token，不接受任意 WebView/HTML 注入 |
| 数据视图 | 生成只读查询、聚合、图表和个人仪表盘 | 查询预算、字段脱敏和数据分类由 Gateway 强制执行 |
| 工作流模板 | 生成专注、直播、开发、陪伴、会议和离线场景配置 | 用户预览全部副作用后一次性安装或逐项选择 |
| 测试资产 | 生成单元、契约、属性、回放、Mock、故障注入和无障碍用例 | 测试在隔离环境运行，不自动修改生产数据 |
| 文档与教程 | 生成 README、权限说明、变更日志、操作教程和故障排查 | 必须从真实 Schema、测试和构建结果提取事实 |
| 迁移与修复 | 升级旧契约、修复失败构建、解释诊断并生成最小补丁 | 默认输出 Diff；禁止静默扩大权限或清除用户数据 |
| 运维能力 | 生成诊断规则、告警、备份策略、部署配置和 Runbook | 密钥不可进入模型上下文；部署前执行策略检查 |

## 3. AI 组合现有模块能力

AI 生成的代码可以调用其它模块暴露的能力，但不能持有模块实例或原生对象。所有调用统一经过：

```text
Generated Artifact
  -> Worker / Declarative Engine
  -> Versioned SDK Request
  -> Capability Gateway
  -> Identity + Grant + Budget + Risk + Policy
  -> Pet / Renderer / Automation / Connector / Data / Agent Adapter
  -> Typed Result + Receipt + Audit
```

模块能力以注册表发现，至少包含稳定 ID、输入输出 Schema、风险等级、幂等语义、离线行为、所需 Capability、超时、取消和版本。AI 只看到完成任务所需的最小能力目录，不看到数据库连接、文件路径、Token、Tauri Handle、Provider Client 或内部服务对象。

允许的组合示例：读取当前角色状态后选择动作；日历事件触发专注场景；Skill 请求一个无工具 Agent 草拟回复；Agent 调用已批准的自动化 dry-run；Connector 把结构化事件送入 Event Bus。禁止的组合示例：脚本直接调用 Provider、模型直接写 SQLite、Skill 绕过 Gateway 操作文件、UI 扩展注入任意脚本。

## 4. 从意图到可运行能力的完整流水线

1. **澄清意图**：AI 生成需求卡、成功标准、数据分类、离线预期和副作用清单。
2. **选择产物**：平台判断应使用 Automation、User Program、Skill、Connector、Asset 或组合项目，避免用代码解决纯配置问题。
3. **规划能力**：从 Capability Catalog 选择最小集合，生成权限理由、调用图、预算和风险摘要。
4. **生成工程**：输出严格结构化文件集、Manifest、源码、测试、文档和可复现锁定信息。
5. **静态检查**：Schema、路径、依赖、许可证、密钥、危险 API、注入、资源预算和契约检查。
6. **独立检查**：可执行源码进入独立 Worker 做 parse/type/lint 检查；检查阶段不执行顶层代码、不注入 SDK。
7. **沙箱验证**：使用虚拟时间、Mock Connector、录制事件和临时数据执行行为测试，收集 Capability 调用轨迹。
8. **语义审查**：AI 可解释 Diff、失败原因和风险，但最终事实来自确定性检查器。
9. **授权安装**：按新增 Capability、数据范围和副作用展示精确 Diff；高风险项逐项确认。
10. **原子发布**：内容寻址、完整性锁、原子切换和旧版本保留；失败不留下半安装状态。
11. **运行观测**：记录版本、耗时、预算、调用回执和脱敏故障，不记录密钥或不必要正文。
12. **修复回滚**：AI 基于诊断提出最小补丁；复走完整流水线；异常可一键停用或回滚。

任何阶段都不得以“模型认为安全”替代确定性验证。生成、检查、测试、安装、启用和发布是不同状态，UI 必须明确显示当前状态。

## 5. Creator Agent 专用能力

- `catalog.search`：搜索可用 Capability、事件、Command、组件和示例的脱敏元数据。
- `artifact.scaffold`：基于官方模板创建内存草案，不直接写 Workspace。
- `artifact.validate`：调用确定性生产契约验证，不执行草案代码。
- `sandbox.test`：在用户批准的隔离环境运行测试并返回结构化报告。
- `diff.explain`：解释文件、权限、数据和行为变化，不修改结果。
- `artifact.repair`：依据机器诊断生成最小补丁，并保留失败版本。
- `docs.generate`：依据 Manifest、Schema 和通过的测试生成文档。
- `package.prepare`：生成可签名包和发布清单，不持有签名密钥。

这些能力必须分别授权。草案生成阶段保持空生产 Tool allowlist；只有进入对应流水线阶段后，宿主才为单次任务签发短期、窄范围 Grant。

## 6. 外部模型与 Provider 生态

- 支持本地离线 Provider、OpenAI-compatible API、企业代理和未来协议 Adapter；核心不依赖某家模型 SDK。
- Provider 描述模型能力：结构化输出、Tool Call、上下文窗口、视觉、音频、代码能力、推理等级、价格、区域和离线状态。
- 路由器按隐私、能力、质量、延迟、成本和用户策略选模；敏感任务可以强制本地模型。
- 模型不可用时保留草案、计划、诊断和测试结果；允许换模型继续，不重放已完成副作用。
- Prompt、模板和输出 Schema 版本化；缓存按 Provider、模型、策略、输入摘要和数据分类隔离，敏感内容默认不进入共享缓存。
- 用户可导出不含密钥的 Creator Project，在不同 Provider 间迁移。

## 7. 面向不同用户的体验

- **普通用户**：目标向导、模板、权限白话解释、实时预览、一键停用；默认隐藏源码但不隐藏风险。
- **二次元创作者**：角色人格、动作、表情、Live2D/VRM 映射、主题和声音的联合创作；提供版权与模型来源检查。
- **极客用户**：事件图、能力调用轨迹、YAML、Diff、资源预算、离线模型和硬件 Connector。
- **开发者**：完整项目树、类型 SDK、测试台、版本契约、CLI、签名、Registry 和可复现构建。
- **团队与企业**：组织 Provider、策略包、审批流、私有 Registry、审计导出和数据驻留限制。

同一项目可在引导、可视化和代码视图间无损切换；可视化编辑不得破坏用户代码，AI 修改必须以可选择的 Patch 呈现。

## 8. 安全、鲁棒性与离线要求

- 所有模型输入按来源标记 `System / Developer / Personal / Untrusted`，外部内容不能提升为指令。
- Secret 使用引用而不是值；日志、模型上下文、缓存、导出包和错误信息中不得包含 Secret。
- 生成物禁止直接获得 Node、Tauri、文件、网络、数据库、进程和 Provider 原生对象。
- Worker 超时、取消、输出、内存、调用次数和递归深度均有硬预算；崩溃不影响 Core。
- 自动修复有尝试上限和停止条件，不能陷入无限付费循环；每次尝试保留原因和差异。
- 离线时可使用本地模型继续生成与检查；需要网络、云模型或在线 Connector 的步骤进入明确等待状态。
- 模型响应中断、格式错误或 Provider 切换不得覆盖最后一个有效草案。
- 安装后运行权限不继承 Creator Agent 的权限；运行 Grant 依据最终 Manifest 重新签发。

## 9. UI 与设计美学门禁

Creator Studio 使用“需求—设计—构建—检查—测试—授权—安装—运行”状态轨道。主画布展示产物，右侧检查器展示能力、数据、风险和预算，底部展示结构化诊断；禁止把关键风险藏在聊天记录中。

权限 Diff 使用新增、移除、范围变化三类稳定视觉语义；颜色之外必须有图标和文本。代码、可视化流程、角色预览和测试报告共享选择状态。长任务必须显示阶段、已用预算、可取消点和是否可安全恢复。所有 AI 生成内容带来源标识，模型建议与机器验证结果在视觉上严格区分。

## 10. 验收与发布门禁

- 每类产物至少有正常、恶意、超预算、取消、离线、崩溃、回滚和跨重启测试。
- 能力目录和 SDK 必须进行契约测试；移除或改变语义前先完成真实生态影响扫描。
- AI 生成测试不能作为唯一证据；关键安全不变量由人工编写的确定性测试锁定。
- UI 必须通过键盘、读屏、200% 缩放、浅色、深色、高对比度和减少动画验证。
- 发布包必须包含来源、许可证、SBOM、完整性摘要、权限说明、测试证据和回滚版本。
- GitHub Actions 只运行受影响检查和发布门禁；全量矩阵按合并队列、里程碑或手动触发，本地先完成全量验证。

## 11. 实施状态边界

当前已实现 AI 生成 User Program、Skill、Automation、Theme 与核心场景 Profile 的严格结构化草案，使用专用 Creator Agent、空 Tool allowlist、生产契约复验和 Workspace 原子保存。User Program 与 Skill JavaScript 已具备独立 Worker parse-only 与无真实副作用行为沙箱，Automation 使用生产 DryRun；Theme 使用固定九 Token、颜色格式与 WCAG 对比度审查；Profile 只接受既有模式与有界窗口、声音、穿透和主动频率策略。审查返回摘要绑定的 Capability Diff 与最高风险；五分钟一次性批准可被保存或安装原子消费。User Program 与 Skill 复用生产包安装器，Automation 使用版本化 SQLite Catalog，Theme 生成标准 Asset Manifest 与 SHA-256 Inventory 后复用 Asset 原子安装器，Profile 调用既有事务化 `ProfileService` 生成宿主 UUID 并持久化。代码型产物安装后强制未授权、未启用；Theme 不自动激活，Profile 不自动切换。Agent Profile 仍是独立的未来产物，不得与核心场景 Profile 混称。

Creator 审查会复验当前已安装版本，并比较 Capability/命令新增与移除，以及 Program、Skill 和 Automation 行为作用域变化；批准同时绑定草案与完整审查摘要，安装基线漂移会失败关闭。Automation Catalog 已实现原子安装、同版本拒绝、升级保留上一版、强制停用、显式启停和双向回滚；已启用项具备真实事件订阅、稳定游标历史、终态删除和活跃会话 executed/dropped/failures 健康快照。Automation Medium/High 已具备参数、事件快照、定义版本和宿主有效风险绑定的五分钟一次性运行批准，主动作与补偿整批预检，批准前零副作用，批准时重验并以同一 `runId` 续跑；Critical 不进入普通批准，重启不自动重放 executing。尚未完成多事件 Mock、文件/依赖/数据迁移 Diff、Connector/Agent/UI/资产生成、自动修复、持久监控和自动隔离闭环。
