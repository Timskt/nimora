# Nimora 需求挖掘报告 — 按用户角色（二次元 / 极客 / 开发者 / 普通用户）

> **文档**：`docs/reviews/REQUIREMENTS_MINING_2026-07-25.md`
> **角色**：需求挖掘 / 产品分析 Agent（只读代码与既有规范，仅写本文件）
> **产品**：Nimora / 灵灵（Tauri 2 原生桌面生命体，仓库 `/Users/sky/code/vibe/gptpet`）
> **日期**：2026-07-25
> **状态**：需求挖掘基线（非实现完成声明，非里程碑证据）

---

## 0. 目的、方法与去重声明

本报告按四类目标用户角色（**二次元 / 极客 / 开发者 / 普通用户**）系统挖掘 Nimora 的**显性、隐性、延展**需求，覆盖任务书四大范围：

1. 桌宠鲜活度与人格（像活的、非脚本感）、QQ 宠物式玩法、拖拽/互动、与真实窗口/系统的物理互动。
2. 创意工坊：用户自制主题场景（世界杯 / LOL 英雄）、自定义模型、第三方模型导入、皮肤、社区共创分享。
3. AI 驱动可扩展性：用户用自选 AI 写 Skill/Worker/脚本；AI CLI/Agent 的 Goal 模式、Auto（全授权）模式、思考等级、上下文压缩/缓存、文件追踪、版本管理（吸收 Codex / Claude Code / opencode 之长）。
4. 系统集成与未来兼容：离线优先、部署、健壮稳定、live2d/live3d、新技术/架构适配、未来用户可能的诉求。

**每条需求给出**：角色 · 优先级（P0/P1/P2）· 既有文档覆盖度（引用文件）· 验收标准草图。

**去重方法**：先通读既有规范再挖掘，凡已被规范充分覆盖者标记 `已覆盖`（并引用来源），仅在语义上**延伸/收紧/补空**；凡规范只有方向而缺可验收定义者标记 `部分覆盖`；全新者标记 `未覆盖`。本报告**不重复**已在 `REQUIREMENTS_CLARIFICATION_2026-07-24.md` 决策看板中拍板的六大主线结论，只在其边界外延伸。

**规范用词**：MUST / SHOULD / MAY 与 `docs/INDEX.md` 一致。优先级与覆盖度是**产品挖掘判断**，不等于工程排期或发版承诺（发版就绪度见 `MILESTONE_REVIEWS.md`、`IMPLEMENTATION_STATUS.md`）。

**主要只读依据**：`PRODUCT_SPEC.md`、`DESKTOP_PET_EXPERIENCE.md`、`DESKTOP_LIFEFORM_CONTEXT.md`、`FEATURE_EVOLUTION.md`、`EXTENSION_ECOSYSTEM.md`、`MODEL_RENDERING_IMPORT.md`、`CUSTOMIZATION_ASSETS.md`、`PROGRAMMABLE_CONTROL.md`、`USER_CODE_CAPABILITY.md`、`USER_CODE_SECURITY.md`、`AI_AGENT_CLI.md`、`AUTO_AUTHORIZATION.md`、`MODEL_REASONING_POLICY.md`、`AGENT_COMPETITIVE_DESIGN.md`、`AI_EXTENSION_FACTORY.md`、`AI_CAPABILITY_DEVELOPMENT_PLATFORM.md`、`AI_NATIVE_EXTENSION_SURFACES.md`、`OFFLINE_DATA_LIFECYCLE.md`、`RELIABILITY_RESILIENCE.md`、`DEPLOYMENT_OPERATIONS.md`、`FUTURE_EVOLUTION_GOVERNANCE.md`、`RISK_REGISTER.md`、`reviews/*`。

---

## 1. 角色画像与核心张力

| 角色 | 一句话诉求 | 主要成功度量 | 最易被现规范忽视的隐性诉求 |
|---|---|---|---|
| 普通用户 | 可爱、会自己玩、拖得动、会说话，像真养在桌面 | 每日主动看它、愿意长期开机常驻 | 「零配置也不无聊」；不打扰但存在感强；卸载/搬机不丢感情 |
| 二次元 | 换成我推的角色、我做的皮肤、导入 VRM/GLB | 能把喜欢的角色养在桌面并分享 | 版权与来源可交代；换装/表情适配「不塌」；场景化（活动主题） |
| 极客 | 可扩展、本地模型、全权限无人值守、插件生态 | 能把桌面变成可编程自治终端 | 可组合可回放；不被安全边界「卡到无法玩」；自托管 Registry |
| 开发者 | Agent/CLI/Skill/用户代码，能力网关、可审计 | 能像用 Codex/Claude Code 一样干活并复用 | 可脚本化 CI 集成、稳定契约、可 diff/回滚、机器可读一切 |

**跨角色核心张力（挖掘结论）**：Nimora 规范在**安全与治理**维度极其完备（Gateway、Grant、fail-closed、审计），但在**「好玩 / 有温度 / 可炫耀」的情感与创作闭环**上，多数能力仍停留在 `Foundation/Exploration` 方向条目，缺**可验收的玩法级定义**。四类角色的最高价值增量，恰在这条「治理已强、乐趣待补」的缝隙上。

---

## 2. 范围一：桌宠鲜活度、人格与物理互动

### 2.1 已覆盖（仅延伸/收紧）

| ID | 需求 | 角色 | 优先级 | 覆盖度 | 验收标准草图 |
|---|---|---|---|---|---|
| L1-01 | 透明无框置顶穿透、拖拽、边缘吸附、多屏 DPI、锁屏/唤醒恢复 | 全部 | P0 | 已覆盖 `DESKTOP_PET_EXPERIENCE.md`、`REQUIREMENTS_CLARIFICATION_2026-07-24.md` §1 | 真机 E2E：拖拽落点分类为 perch/climb/peek/idle，唤醒后位置可恢复 |
| L1-02 | 封闭动作枚举 + 语义指令驱动（perch/climb/peek/idle） | 全部 | P0 | 已覆盖 `DESKTOP_PET_EXPERIENCE.md`（边缘/攀爬/探头/收敛） | Core 仅接受被动落点动作；缺失资源确定性回退 idle，不白屏 |
| L1-03 | Agent 工作陪伴信号（思考/运行/等待/完成/失败/取消 六态） | 普通/开发者 | P1 | 已覆盖 `DESKTOP_PET_EXPERIENCE.md`（`nimora.agent-companion-signal/1`） | 信号仅状态+TaskID+时间戳；终态 4.2s 回权威生命状态 |
| L1-04 | 情境话术优先于「本地陪伴中」干瘪默认（会议/低电/未读） | 普通 | P0 | 部分覆盖 `MULTI_AGENT_PRODUCT_OPTIMIZATION_2026-07-24.md` §5 | 至少 3 类系统情境产生可区分中文短句；无网/无 AI 仍有本地情境句 |

### 2.2 挖掘的隐性/延展需求（重点补空）

| ID | 需求 | 角色 | 优先级 | 覆盖度 | 验收标准草图 |
|---|---|---|---|---|---|
| **L2-01** | **「非脚本感」活力预算**：idle 表演需程序化变体（微动作、注视、打盹、无聊自娱），避免固定循环被用户一眼看穿为脚本 | 普通/二次元 | **P0** | 部分覆盖：`FEATURE_EVOLUTION.md` 有「更灵动 idle」方向，无可验收定义 | 定义 idle variation 目录 + 伪随机不重复窗口（如 60s 内不重复同一微动作）；Reduced Motion 降级为静态姿态；有「上次互动距今」驱动的行为漂移 |
| **L2-02** | **真窗口物理互动**：踩窗口标题栏、被最大化窗口「推开」、沿活动窗口边缘行走、躲避全屏 | 普通/极客 | **P1** | 部分覆盖：`FEATURE_EVOLUTION.md` 空间桌面=Expansion；`DESKTOP_LIFEFORM_CONTEXT.md` 有几何采样但边界未接线 | 用最小平台窗口元数据（非持续截屏）驱动避让/踩边；无权限或采样失败时回退自由漫游；不读取窗口标题内容 |
| **L2-03** | **QQ 宠物式照料成长闭环**：喂食/清洁/玩耍/睡眠 → hunger/energy/mood/affinity 变化 → 关系等级/纪念解锁 | 普通 | **P0** | 部分覆盖：`PRODUCT_SPEC.md` §4.2 有属性与关系；`OFFLINE_DATA_LIFECYCLE.md` 有纪念持久化；缺完整玩法级验收 | 定义每种照料动作的属性影响曲线、冷却、边际递减；离线可玩；不制造羞辱式/成瘾式激励（对齐 `FEATURE_EVOLUTION.md` §6） |
| **L2-04** | **迷你互动玩法**：点头/戳一戳反应、丢球接球、跟随光标、口袋小游戏 | 普通/二次元 | P2 | 未覆盖（仅 `PRODUCT_SPEC.md` Interaction Pack 抽象） | 至少 1 个内置轻互动；作为 Interaction Pack 可扩展；命中区与拖拽命中互不冲突 |
| **L2-05** | **多宠编队与互动**：两只以上宠物在桌面共处、避让、简单社交动作 | 二次元/极客 | P2 | 部分覆盖：`FEATURE_EVOLUTION.md` 多宠系统=Foundation（仅要求 petId 预留） | 每宠独立资源预算与身份；空间避让确定；单宠 UI 可隐藏多宠复杂度 |
| **L2-06** | **人格随长期陪伴自然漂移且可解释**：性格倾向随互动缓慢变化，用户可查看/纠正/回退 | 普通/极客 | P1 | 部分覆盖：`PRODUCT_SPEC.md` §4.2（记忆可查看纠正、人格不得静默扩权） | 人格变化有可视时间线；任何变化不改隐私/权限；可一键回到基础人格 |
| **L2-07** | **声音/语音存在感**：轻量音效反馈 + 可选 TTS 声线包 + 麦克风持续指示 | 二次元/普通 | P2 | 部分覆盖：`FEATURE_EVOLUTION.md` 语音交互=Expansion；`PRODUCT_SPEC.md` Voice Pack | 本地优先；音频保留可配置；无声线包时静默降级不报错 |

---

## 3. 范围二：创意工坊（模型 / 皮肤 / 主题场景 / 社区共创）

### 3.1 已覆盖（仅延伸/收紧）

| ID | 需求 | 角色 | 优先级 | 覆盖度 | 验收标准草图 |
|---|---|---|---|---|---|
| W1-01 | 第三方模型导入隔离流水线（glTF/GLB/VRM 0.x→1.0，OBJ/FBX 创作者转换） | 二次元/开发者 | P0 | 已覆盖 `MODEL_RENDERING_IMPORT.md` §3/§7 | 导入禁执行脚本与任意 URI；缺专有 Runtime 显式回退不静默下载 |
| W1-02 | 统一 Character Model + 语义动作/表情回退链 | 二次元 | P0 | 已覆盖 `MODEL_RENDERING_IMPORT.md` §4/§5 | 缺映射按 `自定义→同类→neutral/idle→默认角色` 回退，Creator Studio 出告警 |
| W1-03 | Asset Pack 分层（Character/Skin/Theme/Voice/Behavior/Interaction），继承/组合/热重载/回退 | 二次元 | P0 | 已覆盖 `PRODUCT_SPEC.md` §4.3、`CUSTOMIZATION_ASSETS.md` | Skin 在兼容角色上替换视觉/音效；Behavior 仅声明式不执行代码 |
| W1-04 | 角色健康检查报告（动作完整度/表情覆盖/许可证/性能/无障碍/安全扫描） | 二次元/开发者 | P1 | 已覆盖 `FEATURE_EVOLUTION.md` §5.3 | 报告用户可读且作为 Registry 可验证元数据 |
| W1-05 | 社区生态（收藏/评价/兼容报告/精选/举报撤回），角色共创非破坏性合并 | 二次元 | P1 | 已覆盖 `FEATURE_EVOLUTION.md`（社区生态/角色共创=Expansion） | 不以付费/热度降低审核；保留来源、许可证、合并历史 |

### 3.2 挖掘的隐性/延展需求（重点补空）

| ID | 需求 | 角色 | 优先级 | 覆盖度 | 验收标准草图 |
|---|---|---|---|---|---|
| **W2-01** | **主题场景包（Themed Scene Pack）作为一等可分享产物**：世界杯/LOL 英雄等「角色+皮肤+主题+动作+台词+布景」一键组合并分享 | 二次元/普通 | **P0** | 部分覆盖：`FEATURE_EVOLUTION.md` §5.1 场景导演聚焦 Profile/自动化，`AI_EXTENSION_FACTORY.md` 工作流模板；**缺「可分享的娱乐主题场景包」类型定义** | 新增 Scene Pack 组合既有 Asset/Behavior/Theme；安装前预览全部副作用；不复制全量配置；不修改被引用原包 |
| **W2-02** | **版权/肖像/IP 合规护栏**：热门 IP（球星、游戏英雄）导入与分享的许可证声明、来源标注、侵权举报与下架 | 二次元/厂商 | **P0** | 部分覆盖：`MODEL_RENDERING_IMPORT.md` 要求许可证；社区有举报撤回；**缺针对知名 IP 的显式合规流程与用户提示** | 上传/导入时强制来源与许可证声明；无授权 IP 有醒目提示与可下架路径；官方源与社区源信任级区分（对齐 `EXTENSION_ECOSYSTEM.md` Registry 信任级） |
| **W2-03** | **应用内 Creator Studio 可视化创作（非仅 SDK/CLI）**：拖拽绑定动作、时间轴、表情映射、实时预览 | 二次元/创作者 | **P1** | 部分覆盖：`MODEL_RENDERING_IMPORT.md` 提到 Creator 模式、`AI_EXTENSION_FACTORY.md` 提「引导式创建」；**缺可视化创作工具的用户级验收** | 无需写代码即可完成导入→绑定→预览→打包；热重载；生成健康检查告警；导出为标准包 |
| **W2-04** | **一键换装 / 附件 / 配饰组合**（节日帽、队服、皮肤局部替换）无需重导整模 | 二次元/普通 | P1 | 部分覆盖：`PRODUCT_SPEC.md` §4.3 提换装/附件槽 | 附件插槽与换装层可即时切换预览；不兼容组合给出可读原因并回退 |
| **W2-05** | **场景/资源市场的离线安装与本地源**：无网也能从本地/局域网导入创作包 | 极客/二次元 | P1 | 部分覆盖：`EXTENSION_ECOSYSTEM.md` Registry 含 Local/Private；`PROGRAMMABLE_CONTROL.md` 离线可启动已安装版本 | 本地包原子安装+完整性校验；离线不依赖 Registry；私有源信任级独立 |
| **W2-06** | **创作产物的一键分享与导入体验**（导出可分享文件、扫码/链接导入、预览卡片） | 二次元/普通 | P2 | 未覆盖（`FEATURE_EVOLUTION.md` 数字商品=Exploration，偏交易而非分享体验） | 生成含预览海报的自包含分享文件；导入前展示权限/来源/健康检查；用户数据与许可证分离 |
| **W2-07** | **Live2D Cubism 合规启用路径**：明确何时/如何合法启用（当前 Installer 提前拒绝） | 二次元 | P2 | 已覆盖边界 `MODEL_RENDERING_IMPORT.md`（扩展未启用，缺合规 Runtime 时拒绝）；**缺启用条件的产品化说明** | 缺合规 Runtime 时明确提示与替代；不伪装支持；许可证感知 Adapter 就绪后才开放 |

---

## 4. 范围三：AI 驱动可扩展性（Skill/Worker/脚本 + Agent/CLI）

### 4.1 已覆盖（仅延伸/收紧）

| ID | 需求 | 角色 | 优先级 | 覆盖度 | 验收标准草图 |
|---|---|---|---|---|---|
| A1-01 | 四级编程阶梯（Rules/YAML/Local Script/SDK）共用 Event/Query/Command/Capability Broker | 开发者/极客 | P0 | 已覆盖 `PROGRAMMABLE_CONTROL.md`、`USER_CODE_CAPABILITY.md` | 无第二条绕过 Gateway 的执行路径；脚本仅用注入 SDK |
| A1-02 | 持久 Goal + 可修订 Plan + 完成需当前 Plan 逐项证据 | 开发者 | P0 | 已覆盖 `AI_AGENT_CLI.md`、`AGENT_COMPETITIVE_DESIGN.md` §3.1 | 模型不能自称完成；跨重启对账；CLI `goal create/list/show/plan replace/status set` JSON |
| A1-03 | Auto Mode（范围绑定，非「跳过确认」）+ 五档 Grant + sleep-safe NeverAsk | 极客/开发者 | P0 | 已覆盖 `AUTO_AUTHORIZATION.md`、`AI_AGENT_CLI.md` | 写/外部副作用/出境/凭据/装码/Medium+ 仍逐项确认或绑定不可变计划；预算耗尽/漂移必暂停 |
| A1-04 | 统一推理等级 + 策略（adaptive/quality_first/cost_saver/fixed）+ Adapter 审计 | 开发者 | P1 | 已覆盖 `MODEL_REASONING_POLICY.md` | 返回 requested/actual/provider value/降级/能力版本；不支持等级调用前 fail-closed |
| A1-05 | 上下文压缩 + 内容寻址缓存 + Workspace 快照/文件追踪 + Checkpoint/Resume | 开发者 | P0 | 已覆盖 `AI_AGENT_CLI.md`（Compactor/Cache/Workspace Snapshot/Checkpoint） | 缓存键绑定 Provider/模型/Plan/Workspace/消息；漂移以 `workspace-changed` 暂停 |
| A1-06 | 外接 AI 能力工厂：自然语言→版本化/可测/可回滚项目产物 | 开发者/极客 | P1 | 已覆盖 `AI_EXTENSION_FACTORY.md`、`AI_CAPABILITY_DEVELOPMENT_PLATFORM.md` | AI 仅提案；宿主做契约校验/权限/风险/安装/审计；输出 Diff 不静默扩权 |
| A1-07 | Away Summary（离开期摘要 + 一键撤销 Grant，无 Secret） | 极客/开发者 | P1 | 已覆盖 `AUTO_AUTHORIZATION.md` §7（Host+FE+CLI 已接线） | 四态 loading/error/empty/ready；不暴露 credential/隐藏推理 |

### 4.2 挖掘的隐性/延展需求（重点补空）

| ID | 需求 | 角色 | 优先级 | 覆盖度 | 验收标准草图 |
|---|---|---|---|---|---|
| **A2-01** | **独立 Auto-review 流水线接线**（当前仅枚举 `AutoReview`，未接线；不可用需 fail-closed 回退询问） | 开发者/极客 | **P0** | 部分覆盖：`AI_AGENT_CLI.md` §6 明列为剩余项 | reviewer 继承同或更窄 Sandbox/Grant，不能改参数/扩范围；不可用/超时/不确定按原策略暂停；模型/等级/理由入审计 |
| **A2-02** | **Grant 系统密钥签名/加密 at rest**（keychain 失败不得静默降级为确定性密钥） | 极客/开发者 | **P0** | 部分覆盖：`AUTO_AUTHORIZATION.md` §8.3 剩余项、`SECRET_MANAGEMENT.md` 基线；`RISK_REGISTER.md` 列为安全事故 | payload 非明文可篡改；轮换可审计；keychain 不可用时 fail-closed 而非退化密钥 |
| **A2-03** | **Skill/脚本「养宠」创作模板与示例库**：让开发者/极客快速写出「构建失败→宠物沮丧」类联动 | 开发者/极客 | **P1** | 部分覆盖：`PROGRAMMABLE_CONTROL.md` 有 `defineScript` 示例（且示例 import 仍为旧 `@deskpet/sdk`）；缺成套模板库 | 提供 ≥5 个可直接安装示例脚本；SDK 包名与 `nimora.*` 契约一致；REPL 可试跑 |
| **A2-04** | **本地模型 / 自托管 Provider 一等支持**（Anthropic/本地 Adapter 参数映射、capability discovery 仍未完成） | 极客 | **P1** | 部分覆盖：`MODEL_REASONING_POLICY.md` §7 明示 Anthropic/本地 Adapter 未完成 | Ollama 类本地 Provider 断网可用；能力发现动态上报；不虚构推理能力 |
| **A2-05** | **CLI 作为一等自动化面**（脚本化集成 CI、机器可读 JSON 全覆盖、退出码稳定） | 开发者 | **P1** | 部分覆盖：`AI_AGENT_CLI.md` 有多命令 JSON；缺推理选择器 CLI、统一退出码/错误契约声明 | 所有 Goal/Grant/Workspace/Away 命令有稳定 JSON 与非零退出码语义；可在无 GUI 环境运行 |
| **A2-06** | **多 Agent / Subagent 编排与交接协议**（Primary/Subagent、Build/Plan Agent 借鉴 opencode） | 开发者/极客 | P2 | 部分覆盖：`AGENT_COMPETITIVE_DESIGN.md` §2 提及、`REQUIREMENTS_CLARIFICATION_2026-07-24.md` §8「Subagent 首稳不做」 | 首稳可不做，但需预留 Scheduler Strategy/Agent Profile 契约不改 Tool/Capability 边界 |
| **A2-07** | **Provider 费用/Token 预算可视与硬上限**（面向长任务无人值守成本失控风险） | 极客/开发者 | P1 | 部分覆盖：`AUTO_AUTHORIZATION.md` 有预算枚举、`MODEL_REASONING_POLICY.md` 审计费用；缺用户级预算面板/硬阈值告警 | 达阈值暂停并结构化说明；控制中心只读展示 Token/费用/周期预算投影 |
| **A2-08** | **未知结果对账（Unknown Outcome Isolation）用户可见处理**（命令/网络结果未知不得自动重放） | 开发者 | P1 | 已覆盖原则 `AGENT_COMPETITIVE_DESIGN.md` §3.7；缺用户面对账 UI | indeterminate Attempt 进入对账态并给中文可行动下一步；不自动重放 |

---

## 5. 范围四：系统集成与未来兼容

### 5.1 已覆盖（仅延伸/收紧）

| ID | 需求 | 角色 | 优先级 | 覆盖度 | 验收标准草图 |
|---|---|---|---|---|---|
| S1-01 | 离线优先：本地 SQLite 权威、断网可陪伴可恢复、启动不等网 | 全部 | P0 | 已覆盖 `OFFLINE_DATA_LIFECYCLE.md`、`REQUIREMENTS_CLARIFICATION_2026-07-24.md` §5 | 无网/无账户/无 AI 时非 AI 能力稳定降级；首稳无云同步 |
| S1-02 | 健壮性/稳定性 NFR（24h 无崩溃、空闲内存<10% 增长、扩展故障隔离、2s 安全模式） | 全部 | P0 | 已覆盖 `RELIABILITY_RESILIENCE.md` | 故障域矩阵；Supervisor 管理后台任务；write-rename 原子持久化 |
| S1-03 | live2d/live3d 统一为可实时驱动能力，优先 VRM/glTF 不建私有格式 | 二次元 | P0 | 已覆盖 `MODEL_RENDERING_IMPORT.md` §1、`FEATURE_EVOLUTION.md` §6 | Live3D=实时 3D 能力语义；WebView/GPU context 丢失可重建或回退内置角色 |
| S1-04 | 未来兼容治理：新 Provider/权限/Agent 架构/上下文技术经 Adapter/Policy/Strategy 扩展，不改核心状态机 | 开发者/厂商 | P1 | 已覆盖 `AGENT_COMPETITIVE_DESIGN.md` §5、`FUTURE_EVOLUTION_GOVERNANCE.md` | 未知权限默认拒绝；新能力过价值/隐私/离线/性能/UI/兼容/退出评审门槛 |
| S1-05 | 部署/发布：多通道、签名/公证、SBOM、四个 sidecar worker 验证、单实例标识稳定 | 厂商/开发者 | P0 | 已覆盖 `DEPLOYMENT_OPERATIONS.md` | 每平台验证 external binary 存在/架构匹配/可启动；本机产物不代替 CI 目标验证 |
| S1-06 | 登录后常驻陪伴（默认关、用户显式开、安静启动、只恢复离线宿主） | 普通 | P1 | 已覆盖 `DESKTOP_PET_EXPERIENCE.md`、`DEPLOYMENT_OPERATIONS.md` | 写入后重查权威状态；不自动跑 Provider/Agent/网络；Safe Mode 可关 |

### 5.2 挖掘的隐性/延展需求（重点补空）

| ID | 需求 | 角色 | 优先级 | 覆盖度 | 验收标准草图 |
|---|---|---|---|---|---|
| **S2-01** | **真机性能签字证据闭环（idle CPU/RSS）**：当前 NFR 有数字但**未交付真机 idle 测量证据** | 全部 | **P0** | 部分覆盖：`REQUIREMENTS_CLARIFICATION_2026-07-24.md` §6 明示未完成、`MULTI_AGENT_...` §4 | 固定参考机型 + 进程私有 RSS 口径；真机 idle CPU<3%/内存<180MB 签字；Preview 不作证 |
| **S2-02** | **数据可携权：一键导出/导入/搬机**（含宠物成长、纪念、资源、脚本、Grant 历史脱敏） | 普通/极客 | **P1** | 部分覆盖：`OFFLINE_DATA_LIFECYCLE.md` 纪念可导出删除、`FEATURE_EVOLUTION.md` §6 反对破坏迁移权；缺整机导出/导入验收 | 导出自包含存档并可在新机恢复；许可证与用户数据分离；导入前预览与冲突处理 |
| **S2-03** | **Linux 支持路线明确化**（当前只 Win/mac，Linux 待决策） | 极客/开发者 | P2 | 部分覆盖：`REQUIREMENTS_CLARIFICATION_2026-07-24.md` §1 待决策「首稳后」 | 明确首稳后时间表与能力差异表达；透明合成/穿透在 Linux 的降级路径声明 |
| **S2-04** | **GPU/WebView 丢失与低端机降级档（Eco/低电量）用户可感知** | 普通 | P1 | 部分覆盖：`RELIABILITY_RESILIENCE.md` 有 context 丢失重建、`REQUIREMENTS_CLARIFICATION_2026-07-24.md` §6.4 Eco 待决策 | 低电量建议 Eco（不强制改档除非授权）；context 丢失回退内置角色且提示；性能档位可见 |
| **S2-05** | **可观测性/诊断：控制中心只读健康面板 + 可回放脱敏诊断** | 极客/开发者 | P1 | 部分覆盖：`FEATURE_EVOLUTION.md` §5.5 可回放自动化、`OBSERVABILITY_DIAGNOSTICS.md`（存在但本次未细读）、`REQUIREMENTS_CLARIFICATION_2026-07-24.md` R-AC-07 | 展示 CPU/内存/事件速率/Token 预算；可导出最小脱敏诊断场景不泄露真实文件/窗口标题/消息 |
| **S2-06** | **首次运行引导与「零配置也不无聊」冷启动体验** | 普通 | P1 | 未覆盖（规范聚焦运行时与安全，缺 onboarding 玩法级定义） | 首启 60s 内产生可爱互动与一次成功照料；不强制登录/联网/配置 Provider 即可玩 |
| **S2-07** | **跨设备遥控/局域网面板**（手机看状态、远程通知） | 极客 | P2 | 已覆盖方向 `FEATURE_EVOLUTION.md`（跨设备=Expansion） | 桌面端离线独立；配对+E2E 加密+最小 Scope；不共享万能 Token |
| **S2-08** | **遥测默认关闭且透明**（隐私默认最小） | 全部 | P1 | 部分覆盖：`REQUIREMENTS_CLARIFICATION_2026-07-24.md` §6.4「遥测默认关」、`SECURITY_PRIVACY.md` | 默认不外发；开启需显式同意并可见数据分类与脱敏预览 |

---

## 6. 跨范围结构性观察

1. **治理强、乐趣弱的失衡**：安全/权限/审计/契约版本化极完备；而「像活的」「好玩」「可炫耀创作」多为 `Foundation/Exploration` 方向词，缺玩法级可验收定义。四类角色的最高感知价值增量集中于此。
2. **命名/契约债尾巴**：`PROGRAMMABLE_CONTROL.md` 示例仍用 `@deskpet/sdk`，与已统一的 `nimora.*` / `@nimora/schemas` 不一致，影响开发者第一印象与文档可信度（`MULTI_AGENT_...` 已指出命名统一，但示例遗漏）。
3. **「已接线 ≠ 已验收」反复被强调**：多份文档（Grant 密钥、Auto-review、真机 QA、idle CPU）诚实标注剩余项。挖掘时**不得**把方向条目当作已交付；本报告据此把这些列为 P0/P1 补空而非「已覆盖」。
4. **创作分享闭环缺一等类型**：Scene Pack（娱乐主题场景）、可视化 Creator Studio、一键分享/导入尚无与安全同级的产物定义，是二次元/普通用户拉新与留存的关键缺口。
5. **未来用户可能诉求（前瞻）**：AI 生成角色/台词的版权可信链、XR/空间桌面、动捕面捕、数字商品交易——均在 `AI_NATIVE_EXTENSION_SURFACES.md` / `FEATURE_EVOLUTION.md` 预留方向，但需在稳定核心后按 `FUTURE_EVOLUTION_GOVERNANCE.md` 门槛逐项验证价值与伦理，不宜提前进入承诺范围。

---

## 7. 最高价值的 15 条「新增 / 欠定义」需求（收尾清单）

> 判据：既有规范**未覆盖或仅有方向、缺可验收定义**，且对四类角色的感知价值 / 留存 / 安全闭环影响最大。按优先级与价值排序。

1. **L2-01 非脚本感活力预算（P0，普通/二次元）**——idle 程序化变体 + 伪随机不重复 + 距上次互动的行为漂移；这是「像活的」的核心，现仅有「更灵动」方向词。
2. **W2-01 主题场景包 Scene Pack 作为一等可分享产物（P0，二次元/普通）**——世界杯/LOL 英雄式「角色+皮肤+主题+动作+台词+布景」组合分享；场景导演偏 Profile/自动化，缺娱乐主题场景类型。
3. **A2-01 独立 Auto-review 流水线接线（P0，开发者/极客）**——当前仅枚举未接线；不可用须 fail-closed 回退询问。无人值守安全闭环关键缺口。
4. **A2-02 Grant 系统密钥签名/加密 at rest（P0，极客/开发者）**——keychain 失败不得静默降级为确定性密钥；`RISK_REGISTER` 视为安全事故级。
5. **S2-01 真机性能签字证据闭环（P0，全部）**——固定参考机型 + 进程私有 RSS 口径 + 真机 idle CPU/内存签字；发版前置且当前未交付。
6. **L2-03 QQ 宠物式照料成长玩法级验收（P0，普通）**——喂食/清洁/玩耍/睡眠→属性→关系/纪念的完整曲线、冷却、边际递减；反成瘾/反羞辱护栏。
7. **W2-02 版权/肖像/IP 合规护栏（P0，二次元/厂商）**——知名 IP 导入分享的许可证声明、来源标注、举报下架；缺针对性流程。
8. **S2-06 首次运行引导与「零配置也不无聊」冷启动（P1，普通）**——首启 60s 内可爱互动 + 一次成功照料，不强制登录/联网/Provider。
9. **W2-03 应用内可视化 Creator Studio（P1，二次元/创作者）**——拖拽绑定/时间轴/表情映射/实时预览，非仅 SDK/CLI；降低创作门槛。
10. **L2-02 真窗口物理互动（P1，普通/极客）**——踩窗口边、被最大化推开、沿活动窗口行走、躲全屏；用最小窗口元数据非持续截屏。
11. **A2-04 本地模型/自托管 Provider 一等支持（P1，极客）**——Anthropic/本地 Adapter 参数映射与 capability discovery（当前明示未完成），断网可用。
12. **S2-02 数据可携权：一键导出/导入/搬机（P1，普通/极客）**——含成长/纪念/资源/脚本/脱敏 Grant 历史；新机可恢复，许可证与数据分离。
13. **A2-07 Provider 费用/Token 预算可视与硬上限（P1，极客/开发者）**——长任务无人值守成本失控防护；达阈值暂停 + 只读预算面板。
14. **A2-03 Skill/脚本养宠创作模板与示例库（P1，开发者/极客）**——≥5 个可安装示例 + REPL 试跑 + 修正 `@deskpet/sdk`→`nimora.*` 命名债。
15. **W2-06 创作产物一键分享/导入体验（P2→P1 候选，二次元/普通）**——自包含含预览海报的分享文件 + 导入前权限/来源/健康检查预览；社区拉新关键。

---

*生成：需求挖掘 / 产品分析 Agent · 2026-07-25 · 中文 · 只读代码与既有规范 · 仅写本文件 · 未改任何代码或 `styles.css` · 未提交/推送*
