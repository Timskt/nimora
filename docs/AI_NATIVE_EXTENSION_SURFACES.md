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

## 2. 四十类新增能力面

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

### 2.25 浏览器协作与网页能力封装

AI 可把用户示教的网页任务抽象为可复验的浏览器流程，为没有正式 API 的站点生成读取、填表、下载、截图和辅助操作方案，并优先建议迁移到稳定 Connector。

标准产物：`Browser Workflow`、`Site Adapter Draft`、`DOM Evidence Pack`、`Browser-to-Connector Migration Plan`。

强制边界：浏览器扩展和桌面宿主使用独立身份；页面、DOM、下载和扩展消息全部是不可信输入；密码、验证码、支付确认和隐私授权不得交给模型读取或代签；写操作逐次预览目标、字段和最终提交动作；站点变化必须失败关闭而非猜测元素。

### 2.26 空间计算、XR 与环境化角色

AI 可生成角色在多屏、AR、VR 和空间桌面中的锚点、视线、距离、遮挡、手势与舒适度策略，并把同一角色语义映射到二维桌面和三维空间表现。

标准产物：`Spatial Presence Profile`、`XR Interaction Map`、`Environment Anchor Pack`、`Comfort Policy`。

强制边界：相机、空间网格、手势和房间布局分别授权；原始环境数据默认不出设备；必须提供无 XR 降级、晕动保护、边界区、急停和旁观者隐私提示；AI 只生成策略，不直接控制未经注册的设备动作。

### 2.27 数字身份、代理委托与同意收据

AI 可帮助用户建立不同场景的公开身份、别名、签名偏好、联系范围和代理规则，并解释某个 Agent 在何时能代表用户做什么。

标准产物：`Identity Profile`、`Delegation Policy`、`Consent Receipt`、`Representation Boundary Pack`。

强制边界：模型不能持有私钥、签署法律同意或自行扩大代理范围；授权必须绑定主体、能力、目标、期限和撤销方式；高影响通信、账号变更、合同、医疗、金融和身份核验始终需要用户本人完成宿主确认。

### 2.28 受控交易、采购与订阅治理

AI 可比较商品或服务、生成预算、采购草案、续费提醒、订阅清单和取消计划，也可为团队形成审批工作流，但不默认成为自主付款代理。

标准产物：`Purchase Draft`、`Budget Policy`、`Subscription Ledger`、`Procurement Workflow`。

强制边界：金额、币种、商户、商品、周期、税费和退款条件进入不可变批准摘要；支付凭据不进入模型；最终支付使用系统或服务方可信界面；重复提交、价格漂移、暗黑模式、自动续费和未知结果必须检测并阻止自动重试。

### 2.29 长期目标、时序规划与主动协助

AI 可把长期 Goal 分解为带依赖、截止时间、证据、复盘点和停止条件的滚动计划，结合日历、任务和用户节律提出主动协助，而不是无限循环执行。

标准产物：`Temporal Goal Graph`、`Proactive Assistance Policy`、`Review Cadence`、`Evidence Checklist`。

强制边界：计划变更与目标完成由证据驱动；主动打扰受 Quiet Mode、频率、场景和健康边界约束；错过期限不会自动扩大权限或费用；长期任务必须支持暂停、接管、导出、跨模型交接和可解释终止。

### 2.30 群体智能、社区评测与可信复用

AI 可在不上传私人内容的前提下比较多个匿名能力方案、聚合兼容性结果、推荐成熟模板，并帮助维护者从真实失败模式中改进扩展。

标准产物：`Federated Evaluation Pack`、`Community Compatibility Report`、`Reputation Evidence`、`Template Recommendation`。

强制边界：默认只共享最小统计和可公开夹具；禁止上传 Prompt、记忆、路径、Secret 与用户内容；评分必须区分签名机器证据、维护者声明和用户评价；防刷票、投毒、女巫身份和多数派错误，社区结果不能替代本地策略。

### 2.31 模型蒸馏、偏好学习与个人适配

AI 可从用户明确选择和经过脱敏的本地样本生成偏好数据集、轻量适配器或蒸馏评测方案，让本地小模型学习风格、路由和分类任务。

标准产物：`Preference Dataset`、`Adapter Training Project`、`Distillation Plan`、`Regression Eval Pack`。

强制边界：训练来源、许可、删除传播和用途必须逐项记录；私人对话不得默认成为训练数据；训练 Worker 与推理 Worker 隔离；适配器必须对隐私泄漏、记忆复现、偏见、能力回退和基础模型版本漂移做固定评测并可彻底删除。

### 2.32 跨设备连续体验与边缘协同

AI 可规划桌面、手机、可穿戴设备、家庭节点和离线边缘设备之间的任务接力、状态投影、模型放置和低带宽降级，让角色与工作流连续而非复制全部数据。

标准产物：`Device Continuity Plan`、`State Projection Schema`、`Edge Placement Policy`、`Handoff Receipt`。

强制边界：每台设备独立配对、授权和撤销；同步采用最小投影而非数据库镜像；冲突有确定性合并或人工裁决；离线队列、设备丢失、时钟漂移、版本不兼容和重放攻击必须有测试；接力收据不包含敏感正文。

### 2.33 个人知识策展、研究与事实维护

AI 可把用户选择的文件、网页、笔记、对话和结构化数据整理为带来源、时间和置信状态的知识空间，持续发现冲突、过期事实、重复内容与待验证结论，并生成研究路线而非直接把模型回答写成事实。

标准产物：`Knowledge Curation Project`、`Source Ledger`、`Claim Graph`、`Research Queue`、`Staleness Policy`。

强制边界：每条结论可回到原始来源和摘录位置；推断、用户陈述、外部事实和模型建议严格分层；删除源数据会传播到索引、摘要与缓存；离线索引可独立重建；外部检索、版权材料和敏感数据出境必须逐源授权。

### 2.34 注意力、通知与主动交互编排

AI 可学习用户明确配置的工作节律，把多个模块的提醒、Agent 建议和设备事件合并为可解释的通知策略，选择延后、聚合、静默、角色动作或跨设备接力，减少打扰而不是提高通知数量。

标准产物：`Attention Policy`、`Notification Routing Graph`、`Interruption Budget`、`Digest Template`。

强制边界：Quiet Mode、系统勿扰和紧急联系人规则优先级最高；AI 不得自行把普通事件升级为紧急；频率、夜间、连续打扰和跨设备重复有硬上限；所有抑制与升级均可查看、撤销并离线执行。

### 2.35 硬件实验、创客与边缘设备能力

AI 可为串口、HID、蓝牙、局域网设备、传感器和机器人生成协议描述、驱动适配草案、校准流程、仿真夹具和安全自动化，让创客通过声明式能力接入实体设备。

标准产物：`Device Adapter Project`、`Protocol Fixture Pack`、`Calibration Recipe`、`Hardware Safety Envelope`。

强制边界：原始设备句柄只属于受信 Adapter Host；写操作受值域、速率、急停、租约和物理确认约束；未知固件、协议漂移、断连重连与失控输出必须失败关闭；高风险机械、电源、门锁和健康设备不得仅凭模型判断执行。

### 2.36 隐私治理、数据主权与可撤销同意

AI 可帮助用户发现数据流、生成最小化策略、安排保留和删除、解释 Connector 与 Provider 的数据去向，并为导出、共享、训练和同步创建用途绑定的同意草案。

标准产物：`Data Inventory`、`Purpose Binding Policy`、`Consent Plan`、`Deletion Propagation Plan`、`Privacy Impact Report`。

强制边界：AI 只能提出策略，不能替用户同意；Secret、身份材料和敏感正文不得进入模型上下文；撤销必须传播到缓存、索引、队列、训练样本和设备副本并生成收据；无法证明删除时状态必须是 `indeterminate` 而非成功。

### 2.37 空间、窗口与桌面环境自动化

AI 可根据场景、显示器、虚拟桌面、应用状态和角色位置生成窗口布局、Overlay 行为、快捷入口与恢复方案，使 Nimora 成为桌面环境编排层而非仅有一个固定窗口。

标准产物：`Workspace Layout Profile`、`Window Choreography`、`Overlay Interaction Map`、`Desktop Recovery Snapshot`。

强制边界：窗口枚举、置顶、穿透、输入捕获和截屏分别授权；布局应用前可预览且可一键撤销；应用消失、显示器变更、分辨率漂移和系统重启必须确定性降级；不得模拟安全输入框或遮挡系统安全提示。

### 2.38 创作者发布、授权与可持续商业化

AI 可把角色、主题、声音、Skill、模板和组合项目整理为可发布产品，辅助生成授权选项、定价实验草案、兼容矩阵、支持材料和升级计划，并维护个人分叉与上游版本关系。

标准产物：`Release Project`、`License Compatibility Report`、`Offering Manifest`、`Support Policy`、`Revenue Experiment Draft`。

强制边界：AI 不得声明其不拥有的版权、商标或再授权权；收费、退款、税务和商店提交必须由用户或组织确认；免费与付费版本不能形成隐蔽权限差异；下架、退款、许可证撤销和依赖停服均有可验证退役路径。

### 2.39 数字健康、关系边界与福祉保护

AI 可生成陪伴强度、休息、睡眠、情绪记录、关系称呼和主动关怀策略，为不同年龄、文化和使用场景提供可配置边界，但不冒充医生、心理治疗师或真实人际关系。

标准产物：`Wellbeing Boundary Profile`、`Companion Intensity Policy`、`Rest Routine`、`Escalation Resource Pack`。

强制边界：不使用操纵性依恋、羞辱、付费诱导或虚假意识表达；危机信号只触发透明、地区适配的支持资源，不进行诊断；健康数据默认本地、可删除且不用于广告；儿童、共享设备和监护场景需要独立策略与清晰身份提示。

### 2.40 可验证决策、方案比较与人类裁决

AI 可把复杂目标转为候选方案、约束、假设、证据、反例、成本和可逆性比较，运行多模型辩论或红队审查，并输出供用户裁决的决策记录，而不是以单一模型语气代替判断。

标准产物：`Decision Dossier`、`Option Matrix`、`Assumption Register`、`Counterfactual Simulation`、`Human Decision Receipt`。

强制边界：事实、预测、价值偏好和未知项必须分离；评分权重由用户确认；模型共识不等于正确，少数意见与证据缺口不可隐藏；医疗、法律、财务和安全关键决策必须标明专业复核要求且不能自动执行最终选择。

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
9. 浏览器站点漂移、支付价格漂移、身份代理越界和最终提交前取消均保持零不可逆副作用。
10. XR 传感器撤权、跨设备离线冲突、联邦评测投毒和个人适配器删除传播均能失败关闭并生成可审计收据。
11. 知识来源删除、同意撤销、通知预算、设备急停、窗口撤销、许可证证明、福祉边界和决策证据缺口均有确定性测试。

## 9. 实施边界

本文定义目标能力面，不代表全部已经实现。当前实现事实以 [`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) 为准。优先形成的后续真实纵切应是通用 `Capability Gap`、受控 Composition Planner、统一 `CreatorArtifact` Handler 契约和 Simulation Pack；在代码、确定性测试、桌面 UI 与视觉验收完成前不得宣称这些能力可用。
