# Nimora 外接 AI 原生扩展能力面

> 版本：0.1.0-draft  
> 更新日期：2026-07-18  
> 状态：产品、架构与测试基线  
> 目标：定义用户接入自选 AI 后，除编写 Program、Skill 和 Automation 外，还能安全创建、组合、验证和经营哪些长期可扩展能力。

## 1. 设计结论

Nimora 不应把外接 AI 限制为“聊天助手”或“代码生成器”。AI 应成为用户的能力设计伙伴，但不能成为权限主体。它可以理解目标、发现缺口、生成结构化产物、解释机器证据和提出维护 Patch；宿主始终负责 Schema、权限、预算、批准、安装、运行、审计和回滚。

所有能力遵循四条原则：

1. **先复用后生成**：设置能解决时不写代码，Automation 能表达时不创建常驻 Skill。
2. **先模拟后执行**：任何外部副作用都先进入 Mock、Dry-run、回放或数字孪生。
3. **项目事实独立于模型**：Goal、决定、文件、测试、授权和版本属于 Nimora Creator Project，不属于聊天历史。
4. **能力与权限分离**：AI 能设计任意合法能力，但只能通过已注册 Builder Tool 提案，不能直接得到运行权限。

## 2. 十六类新增能力面

### 2.1 多模态感知与情境理解

用户可让 AI 创建屏幕、窗口、摄像头、麦克风、传感器、日历和设备状态的组合感知规则，例如识别直播状态、会议阶段、专注中断或角色互动时机。

标准产物：`Perception Pipeline`、`Situation Model`、`Consent Policy`。

强制边界：

- 默认不采集；每个来源单独授权并持续显示采集状态。
- 优先端侧提取特征，原始音视频默认不持久化、不进入云模型。
- 推断结果携带置信度、来源和有效期；低置信度不得触发高风险动作。
- 生物识别、情绪推断和旁观者数据必须有更严格的禁用与删除策略。

### 2.2 个人 API 与能力门面

AI 可把多个模块、Connector 和用户数据组合成稳定的个人 API，例如“开始直播场景”“准备会议”“生成今日开发简报”，供 CLI、Agent、快捷键或局域网获授权客户端调用。

标准产物：`Personal API Surface`、`Command Facade`、`Client Profile`。

强制边界：接口只能映射已注册 Command/Query；必须声明鉴权、幂等、速率、网络绑定、离线语义和撤销方式。AI 不得生成任意本地 HTTP Handler 绕过 Local Gateway。

### 2.3 个人数据应用与知识产品

AI 可创建本地数据库表单、关系、查询、看板、知识库、时间线和可视化，把零散数据转为可维护的个人应用，而不仅是一次性摘要。

标准产物：`Personal Data App`、`Knowledge Product`、`Data Lifecycle Pack`。

强制边界：Schema、血缘、保留期、导入、导出、删除传播、可逆迁移和字段级分类必须完整；生成查询只能通过有预算的 Data Query Capability。

### 2.4 语义映射与跨应用统一

AI 可学习用户确认的术语映射，将不同服务中的项目、联系人、任务、状态、标签和时间语义统一，减少每条 Automation 重复转换。

标准产物：`Semantic Mapping Pack`、`Entity Resolution Policy`。

强制边界：歧义映射必须保留候选与置信度；身份合并默认可逆；不得仅凭名称自动合并人员、账号或敏感记录。

### 2.5 协议逆向辅助与适配器生成

在用户拥有合法授权的前提下，AI 可从 OpenAPI、JSON Schema、MCP、设备描述、CLI 帮助文本和脱敏请求样本生成 Connector 或 Device Adapter 草案。

标准产物：`Protocol Adapter Project`、`Contract Fixture Pack`、`Compatibility Probe`。

强制边界：未知协议先只读探测；域名、设备、命令和数据方向使用白名单；凭据只使用 Secret Reference；写操作必须有 Mock 和幂等或补偿语义。

### 2.6 示教、流程挖掘与意图抽象

用户可显式录制一次操作，AI 将其提炼为参数化 Recipe，识别可变输入、稳定锚点、敏感字段、等待条件和失败补偿。

标准产物：`Demonstration Recipe`、`Interaction Fixture`、`Automation Candidate`。

强制边界：录制前明确范围；密码、Token 和个人消息实时遮蔽；坐标点击必须尽可能升级为语义目标；发布前用回放证明不依赖偶然窗口位置。

### 2.7 Agent 团队、专家角色与交接协议

AI 可生成规划、研究、编码、测试、审查、资产制作和运营等角色组成的 Agent Team，并为每个角色配置最小 Tool、预算、停止条件和交接 Schema。

标准产物：`Agent Team Graph`、`Role Profile`、`Handoff Contract`。

强制边界：子 Agent 不继承父 Agent 全部权限；递归深度、并发、费用和时间有硬上限；多数投票不能覆盖确定性安全检查或用户批准。

### 2.8 Prompt、上下文、记忆与缓存工程

AI 可为某类任务生成版本化 Prompt、文件选择器、压缩器、记忆提取器、缓存键和失效策略，并通过评测证明其质量。

标准产物：`Context Pack`、`Prompt Pack`、`Memory Policy`、`Cache Policy`。

强制边界：每段上下文保留来源与数据分类；摘要不能冒充原文；缓存按 Provider、模型、工具版本、策略和数据等级隔离；用户可查看、纠正、遗忘和彻底清除。

### 2.9 模型评测、路由与结果仲裁

AI 可构建用户自己的评测集，比较云模型、本地模型和企业网关的质量、成本、延迟、隐私、工具调用和离线表现，再生成路由与降级策略。

标准产物：`Eval Pack`、`Model Routing Policy`、`Arbitration Policy`。

强制边界：评测样本版本锁定并去污染；模型不能给自己修改评分标准；路由先满足数据驻留与能力约束，再优化价格和速度；自动仲裁保留分歧和选择原因。

### 2.10 策略编译与合规配置

用户、家庭或组织可用自然语言描述隐私、未成年人、费用、安静时段、数据驻留、发布和审批要求，AI 将其编译为可审查的确定性策略草案。

标准产物：`Policy Pack`、`Approval Workflow`、`Compliance Test Pack`。

强制边界：自然语言不是运行时授权；策略必须编译为受版本控制的规则，显示冲突、覆盖范围和默认拒绝行为；任何放宽都产生显著 Diff 和重新批准。

### 2.11 仿真、数字孪生与合成数据

AI 可构建虚拟时间、网络、Provider、设备、事件源和用户行为模型，用合成数据验证长期 Automation、IoT、直播和 Agent 工作流。

标准产物：`Simulation Pack`、`Digital Twin`、`Synthetic Fixture Set`。

强制边界：仿真通道与生产 Capability 物理分离；合成数据有明显标记；测试通过不等于生产授权；故障模型覆盖超时、重复、乱序、部分成功和恢复。

### 2.12 实验、个性化与价值优化

AI 可设计本地可撤销实验，比较提醒频率、动作反馈、模型路由和界面布局，帮助用户找到更舒适的体验，而不是无边界追求使用时长。

标准产物：`Experiment Plan`、`Personalization Policy`、`Value Report`。

强制边界：默认单用户、端侧、短周期；禁止暗黑模式和成瘾指标；优化目标由用户选择；样本不足必须显示不确定性；实验结束自动恢复或请求采纳。

### 2.13 资产创作、转换与质量修复流水线

AI 可辅助生成或修复 Live2D、VRM、glTF、Sprite、动作、表情、材质、声音、字幕、主题和无障碍替代资产，并自动生成映射与降级方案。

标准产物：`Asset Build Project`、`Import Profile`、`Quality Repair Patch`。

强制边界：记录来源、许可证、模型与编辑历史；隔离 Importer 做格式和资源探测；自动减面、贴图压缩或骨骼映射必须可预览并保留原件；生成资产不能携带执行代码。

### 2.14 生态迁移与技术雷达

AI 可监测已安装扩展所依赖的 SDK、Provider、协议、模型格式和平台 API，生成兼容报告、迁移 Patch、双轨验证和退出方案。

标准产物：`Technology Radar`、`Migration Project`、`Exit Plan`。

强制边界：外部公告只是证据来源，不直接触发升级；迁移不得静默扩权或改变数据驻留；旧版本在新版本通过 Canary 前保持可回滚。

### 2.15 生态发布、协作定制与本地分叉

AI 可把私人能力去身份化后包装成模板，为团队或市场生成文档、翻译、示例、权限说明、SBOM 和发布候选；用户也可安全维护自己的本地分叉。

标准产物：`Collaboration Pack`、`Release Candidate`、`Local Overlay Patch`。

强制边界：自动剥离 Secret、个人路径和真实样本；AI 不持有签名密钥；上游升级与本地 Patch 必须三方 Diff；来源与许可证不明确时禁止公开发布。

### 2.16 支持、自愈、成本与数字遗产治理

AI 可解释脱敏诊断，提出最小修复、降级、合并、停用和退役建议；识别长期无价值、失效或无人维护的能力，并协助导出与删除。

标准产物：`Repair Plan`、`Resource Optimization Patch`、`Retirement Plan`、`Support Bundle`。

强制边界：修复默认只生成 Patch；自动修复有次数和费用上限；未知外部副作用不得自动重试；退役必须撤销 Grant、Secret、订阅、后台任务和缓存，并生成可验证收据。

### 2.17 角色人格、关系与行为编排

AI 可根据用户明确选择的语气、世界观、边界、关系阶段和场景，生成可版本化人格与行为包；同一角色可为工作、直播、陪伴和隐私场景采用不同 Profile，而不是把人格永久固化在 Prompt 中。

标准产物：`Persona Pack`、`Relationship Policy`、`Behavior Graph`、`Conversation Boundary Pack`。

强制边界：人格不得冒充真人或医疗专业身份；关系状态必须可查看、重置、导出和删除；行为图只引用 Registry 中的动作与事件；敏感记忆、主动打扰、拟人化依赖和未成年人模式分别受策略门禁。

### 2.18 桌面组件、信息卡片与交互面板

AI 可生成宠物气泡、状态卡、仪表盘、快捷面板、设置表单和只读数据视图，让普通用户用自然语言定制界面，也让开发者输出可复用 Widget。

标准产物：`Declarative Widget`、`Dashboard Layout`、`Form Schema`、`Interaction Story Pack`。

强制边界：只允许声明式组件目录与设计 Token，不接受任意 HTML、脚本、远程字体或全局 CSS；每个交互绑定已注册 Command；必须验证键盘导航、缩放、溢出、多语言、空态、错误态、Reduced Motion 和窄屏布局。

### 2.19 语音、字幕与会话体验编排

AI 可生成语音风格配置、发音词典、字幕模板、轮次策略、打断规则和多语言切换方案，也可把用户合法拥有的音频规范化为 Voice Asset 草案。

标准产物：`Voice Experience Pack`、`Pronunciation Lexicon`、`Turn-taking Policy`、`Caption Style Pack`。

强制边界：声音克隆必须有可验证同意与撤销；原始录音默认本地处理；字幕是无障碍必需输出而非装饰；唤醒词、持续监听和云端发送分别授权；静音、Quiet Mode 与急停优先于 Agent 意图。

### 2.20 无障碍、国际化与认知适配

AI 可审计并生成高对比主题、简化操作流、键盘映射、屏幕阅读语义、色觉安全图表、易读文本、字幕和区域化资源，为不同能力和语言用户创建可切换适配包。

标准产物：`Accessibility Profile`、`Localization Pack`、`Cognitive Assistance Flow`、`Input Adaptation Map`。

强制边界：机器翻译和易读改写必须标记来源与置信度；关键安全含义不得静默改变；适配包必须通过 WCAG、焦点顺序、文本扩展、RTL、复数规则和无鼠标任务测试，且不能降低安全确认强度。

### 2.21 测试、认证与可复现证据生成

AI 可从契约和用户故事生成测试矩阵、属性测试候选、故障注入场景、视觉基线、Mock Provider、兼容性夹具和发布证据包，并分析失败而不直接篡改门禁。

标准产物：`Test Pack`、`Fault Scenario Pack`、`Compatibility Certificate Draft`、`Release Evidence Bundle`。

强制边界：AI 生成测试不得自证实现正确；期望值必须来自 Schema、人工批准样例或独立 Oracle；Flaky 测试只能隔离并登记，不能静默删除；认证结论由确定性 Runner 和签名证据产生。

### 2.22 数据迁移、备份与灾难恢复助手

AI 可生成导入映射、数据清理建议、备份策略、恢复演练、跨设备迁移和第三方服务退出计划，帮助用户长期拥有自己的角色、记忆、自动化和资产。

标准产物：`Migration Project`、`Backup Policy`、`Restore Drill`、`Service Exit Pack`。

强制边界：迁移先生成不可变清单与 Dry-run Diff；覆盖、删除和格式降级必须显式批准；备份加密密钥不进入模型；恢复必须在隔离目录验证完整性、版本、权限和 Secret 缺失，再允许原子切换。

### 2.23 团队模板、教学与能力共享

AI 可把成熟能力转化为带分步解释、练习夹具、风险提示和可替换参数的教学模板；组织可发布受策略约束的角色、工作流和开发环境基线。

标准产物：`Guided Template`、`Learning Path`、`Organization Blueprint`、`Policy-bound Starter Kit`。

强制边界：模板不得携带作者 Secret、路径、私有数据或默认高权限；导入后必须重新解析依赖和权限；组织策略与个人数据分离；学习模式中的模拟执行不能误触真实 Connector。

### 2.24 本地模型适配与端侧智能优化

AI 可依据设备资源和用户任务生成模型路由、量化候选、提示模板、检索索引、端云切换与质量评测配置，使离线和隐私优先模式持续改进。

标准产物：`Local Model Profile`、`Routing Policy`、`Quantization Evaluation`、`Offline Intelligence Pack`。

强制边界：模型权重与许可证先验证；量化、微调和索引构建在独立 Worker 中受磁盘、内存、GPU、温度与时间预算约束；质量下降必须由固定 Eval 暴露；离线失败不得静默回退到云端。

## 3. 跨能力组合示例

### 3.1 AI 直播搭档

`Perception Pipeline` 识别直播状态，`Situation Model` 判断空场或高互动阶段，角色行为 Skill 选择动作，字幕和语音资产负责反馈，Automation 只调用已批准的直播 Connector。用户可在数字孪生中回放整场直播，并限制摄像头数据不离开设备。

### 3.2 AI 开发伙伴

Context Pack 追踪工作区文件和 Git 版本，Agent Team 分工实现、测试与审查，Personal API 暴露“检查当前变更”，Policy Pack 限制工作区和命令，Eval Pack 在不同模型间路由。最终 Patch、测试和提交建议均可审查，AI 不直接获得任意 Shell 或仓库外文件权限。

### 3.3 AI 家庭与设备管家

Protocol Adapter 对接 Home Assistant 和本地设备，Semantic Mapping Pack 统一房间与设备名称，Digital Twin 模拟断网和重复事件，Automation 执行有急停的动作。高风险设备命令需要参数绑定批准，儿童或访客 Profile 使用更严格策略。

### 3.4 AI 角色创作工作室

Asset Build Project 处理 VRM/Live2D 资产，AI 生成动作与表情映射、主题、声音和人格候选；Importer、性能预算、版权检查和视觉预览提供确定性证据。行为代码仍作为独立 Skill 安装，不混入资产包。

### 3.5 AI 个人知识与行动系统

Knowledge Product 摄取用户选择的数据，Semantic Mapping Pack 统一项目实体，Context Pack 控制检索和遗忘，Personal Data App 提供看板，Agent 只能生成行动草案；发送消息、修改日历或操作文件仍经过各自 Capability 和批准。

## 4. 统一产物契约

新增能力不得各自发明安装通道。所有产物归一为 `CreatorArtifact`，至少声明：

```text
identity / artifactKind / version / provenance
dependencies / compatibility / capabilityRequests / dataFlows
schemas / files / migrations / tests / documentation
budgets / offlineSemantics / cancellation / failurePolicy
activation / observation / rollback / uninstall / retirement
```

每个新 `artifactKind` 由可信宿主 `ArtifactHandler` 实现：

```text
validate -> diff -> simulate -> install -> activate
observe -> update -> rollback -> uninstall -> retire
```

AI 只能生成数据和 Patch，不能生成后立即加载新的可信 Handler。L4 平台能力提案必须经过代码评审、人工安全测试和宿主版本发布。

## 5. Builder Tool 扩展

除现有 Catalog、Project、Validate、Simulate、Diff、Install 和 Observe Tool 外，完整平台还需要：

- `gap.analyze`：对照目标与 Registry 生成结构化能力缺口，不发明未知 Command。
- `composition.plan`：生成跨模块调用图、数据流、补偿和最小 Capability 集。
- `fixture.synthesize`：创建明确标记的合成数据与故障样本。
- `eval.run`：在固定数据、预算和评分器下执行可重现评测。
- `policy.compile`：把自然语言约束转换为可检查规则草案并报告冲突。
- `provenance.inspect`：读取来源、许可证、模型、Patch 和测试证据链。
- `migration.plan`：生成版本迁移、双轨验证、回滚和数据处理步骤。
- `artifact.promote`：把 Recipe 提升为 Automation、Program 或 Skill，保留来源和测试。
- `artifact.fork/rebase`：维护本地定制，显示上游、本地和新版本三方 Diff。
- `artifact.retire`：生成撤权、停调度、导出、删除和验证收据计划。

这些 Tool 只操作 Creator Workspace 与可信验证器，不暴露 Node、Tauri、文件路径、数据库连接、网络 Client、Provider Client 或 Secret 值。

## 6. UI 与设计要求

Creator Studio 应提供统一的“发现—设计—构建—验证—授权—安装—观察—维护—退役”轨道，并按用户类型渐进展示：

- 普通用户查看目标、收益、风险、成本和一键停用，不被迫阅读源码。
- 二次元创作者查看角色、动作、表情、材质、声音和版权时间线。
- 极客查看事件图、Capability 图、预算、Trace、YAML 和模拟时钟。
- 开发者查看项目树、Schema、代码、测试、SBOM、签名和三方 Diff。
- 团队查看策略继承、审批、数据驻留、责任人和维护状态。

AI 建议、用户决定、宿主事实和机器证据必须使用不同且可访问的视觉语义。关键风险不能藏在聊天记录；长任务持续显示阶段、预算、检查点、取消和恢复状态。

## 7. 安全与稳定性门禁

- 外部网页、文档、模型响应、资产元数据和 Connector 内容全部视为不可信数据。
- Creator Agent 与生产 Agent 使用不同身份、Tool Catalog、预算和缓存命名空间。
- 任何安装、启用、扩权、Secret 绑定、发布、外部写入和高风险参数都由宿主生成批准摘要。
- 取消传播至 Provider、Worker、验证器和 Tool；部分成功或未知结果进入 `indeterminate`。
- 自动修复、Agent Team 和 Goal 循环有时间、步骤、费用、并发、重试和递归硬上限。
- 离线时项目编辑、验证器、模拟器、本地模型、已装能力、导出和回滚可用；在线步骤明确等待。
- Safe Mode 禁用第三方执行与在线副作用，但保留诊断、导出、回滚、撤权和恢复。

## 8. 功能测试矩阵

每类能力至少验证：

1. 正常创建、换模型接力、保存、导出、安装和回滚。
2. Registry 缺失、Schema 错误、恶意 Prompt、污染样本和伪造测试证据。
3. 最小 Capability、权限 Diff、Secret 引用和数据出口准确性。
4. 离线、超时、取消、崩溃、重启、重复事件、乱序和部分成功。
5. Token、费用、CPU、内存、网络、存储、并发和后台唤醒预算。
6. 键盘、读屏、200% 缩放、浅深色、高对比和减少动画。
7. Provider 消失、协议破坏、SDK 升级、本地分叉冲突和完整退役。
8. AI 自评与聊天文本不能推进 `validated`、`approved` 或 `installed` 状态。

## 9. 实施边界

本文定义目标能力面，不代表全部已经实现。当前实现事实以 [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) 为准。优先形成的后续真实纵切应是通用 `Capability Gap`、受控 Composition Planner、统一 `CreatorArtifact` Handler 契约和 Simulation Pack；在代码、确定性测试、桌面 UI 与视觉验收完成前不得宣称这些能力可用。
