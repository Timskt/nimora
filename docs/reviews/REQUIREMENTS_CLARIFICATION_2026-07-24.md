# Nimora 需求澄清纪要

> **文档**：`docs/reviews/REQUIREMENTS_CLARIFICATION_2026-07-24.md`  
> **角色**：需求确认 Agent（只读代码/既有规范，仅写本文件）  
> **产品**：Nimora（桌面生命体平台，仓库 `/Users/sky/code/vibe/gptpet`）  
> **日期**：2026-07-24  
> **状态**：需求澄清基线（非实现完成声明）

---

## 0. 目的与方法

本文对以下六条产品主线做**可工程化**澄清，供产品、设计、实现与验收共用：

1. 原生桌面 vs 浏览器  
2. 宠物主体（Subject）  
3. AutoMode `full_device`  
4. Goal 持续  
5. 本地优先  
6. 性能预算  

**每条统一结构**：业务规则 → 限制 → 验收标准 → **已确认 / 待决策**。

**规范依据（只读）**：`PRODUCT_SPEC.md`、`DESKTOP_PET_EXPERIENCE.md`、`DESKTOP_LIFEFORM_CONTEXT.md`、`AUTO_AUTHORIZATION.md`、`AGENT_COMPETITIVE_DESIGN.md`、`OFFLINE_DATA_LIFECYCLE.md`、`RELIABILITY_RESILIENCE.md`、`ARCHITECTURE.md`、`adr/ADR-001-tauri-rust-core.md`、`MILESTONE_REVIEWS.md`、`README.md`。

**规范用词**：MUST / SHOULD / MAY 语义与 `docs/INDEX.md` 一致。

---

## 1. 原生桌面 vs 浏览器

### 1.1 业务规则

| ID | 规则 | 级别 |
|---|---|---|
| D-BR-01 | **生产运行时**是 Tauri 2 原生桌面宿主 + Rust Core + 本地 SQLite；不是网站、不是 Electron 主路径。 | MUST |
| D-BR-02 | 产品核心是**透明无边框、可置顶/穿透、托盘常驻**的独立桌宠 Overlay，不是“浏览器里的聊天页”。 | MUST |
| D-BR-03 | Browser Preview（`pnpm --dir apps/desktop dev`）仅用于 DOM/焦点/布局/视觉几何预览与契约 UI 开发。 | MUST |
| D-BR-04 | Preview 必须显式降级：返回 `desktop-host-required` 或标注“不会修改系统”；禁止伪造原生能力成功态。 | MUST |
| D-BR-05 | 以下**只能**在 Windows/macOS 真机原生包上验收：透明合成、置顶/穿透、拖拽吸附、多屏 DPI、锁屏/休眠恢复、Dock/任务栏、单实例、登录项、签名包。 | MUST |
| D-BR-06 | 每用户会话只允许一个原生实例；第二次启动聚焦既有控制中心，不新建窗口树/数据库写者。 | MUST |
| D-BR-07 | 关闭控制中心 ≠ 退出；桌宠+托盘继续常驻；退出经托盘“退出 Nimora”或 OS 退出协议。 | MUST |

### 1.2 限制

| ID | 限制 |
|---|---|
| D-LM-01 | 禁止把 Browser Preview 截图/自动化当作原生透明、穿透、DPI、菜单键、GPU 合成证据。 |
| D-LM-02 | Web Demo / Preview 不得成为主运行时或分发路径。 |
| D-LM-03 | Renderer（WebView/Three.js）不得持有 OS 窗口句柄、移窗、登录项、文件系统或网络原生对象。 |
| D-LM-04 | 原生几何与窗口事务唯一归属 Desktop Coordinator / Desktop Host。 |
| D-LM-05 | Preview 验收框固定约 260×300，避免标签页尺寸误导；原生窗口不得带棋盘背景/Preview 边框。 |

### 1.3 验收标准

| ID | 标准 | 证据类型 |
|---|---|---|
| D-AC-01 | 真机：关控制中心后宠物持续运行；重启后位置与稳定状态可恢复。 | 原生 E2E |
| D-AC-02 | 真机：点击、拖拽、穿透、置顶、隐藏、托盘恢复、退出无不可恢复路径。 | 原生 E2E |
| D-AC-03 | 真机：多屏插拔、DPI、睡眠唤醒、锁屏、全屏避让行为确定。 | 原生 E2E |
| D-AC-04 | Preview：原生-only IPC 统一 `desktop-host-required`，无假成功。 | 前端单测 + Preview |
| D-AC-05 | 文档与发布门禁明确区分 Browser / Native 证据边界。 | 文档审查 |

### 1.4 已确认 vs 待决策

**已确认**
- 主路径 = 原生 Tauri 桌面（ADR-001 Accepted）。
- Preview 只是开发/视觉工具，不能冒充真机。
- 桌宠是独立生命体窗口，不是聊天 SPA。
- 单实例、托盘常驻、控制中心可关。

**待决策**
- [ ] 首个公开发布是否同时签 Windows + macOS，还是单平台先行（建议：双平台门禁同版，可分渠道灰度）。
- [ ] Linux 是否进入首个稳定版（建议：首稳后，不阻塞 Win/mac）。
- [ ] Preview 是否保留“离线模拟传感器”教学开关（建议：仅 dev，默认关，永不写生产库）。

---

## 2. 宠物主体（Subject）

### 2.1 业务规则

| ID | 规则 | 级别 |
|---|---|---|
| P-BR-01 | 宠物是桌面 **Subject**（主动生命体），不是控制中心附属装饰，也不是聊天窗口替身。 | MUST |
| P-BR-02 | 权威状态在 **Runtime Core** 的 Pet Snapshot / FSM；Overlay 只投影，不写权威领域状态。 | MUST |
| P-BR-03 | Desktop Host 将活动 Profile 投影为只读 `petPresentation`；Overlay 不直读 Profile Repository。 | MUST |
| P-BR-04 | 原生窗口坐标仅由 Desktop Coordinator 读取、约束、持久化、执行；Renderer 无移窗权。 | MUST |
| P-BR-05 | 自主漫游使用弹簧-阻尼位移；禁止“剩余距离线性插值”冒充平滑移动。 | MUST |
| P-BR-06 | AI/Agent/Skill/用户代码/Automation 只能经 **Capability Gateway + 封闭 Action Catalog / `StructuredPetDirective`** 建议动作，不得注入任意动画名或任意气泡文本。 | MUST |
| P-BR-07 | 落点语义唯一：Bottom/底角→`pet.perch`，Left/Right→`pet.climb`，Top/顶角→`pet.peek`，Free→`pet.idle`。 | MUST |
| P-BR-08 | 瞬态 `dragged` / `interacting` / `recovering` 不可跨进程延续；加载时归一 neutral idle。 | MUST |
| P-BR-09 | 照料/成长/纪念与 Event 同事务原子提交；失败时内存、磁盘、总线一致，无“界面已解锁重启消失”。 | MUST |
| P-BR-10 | 无 AI Key、断网、关控制中心时，启动/拖动/照料/菜单/自主/离线持久化仍完整可用。 | MUST |
| P-BR-11 | 默认角色呈现为可识别的陪伴生命体（产品叙事：灵灵 / Q 版可爱造型）；换皮不改变交互语义与安全边界。 | MUST |
| P-BR-12 | 用户反馈优先于 Ambient；`suppressAutonomy`（全屏/游戏/勿扰/演示/Safe）可拒 Ambient，但不得剥夺点击、照料、恢复入口。 | MUST |

### 2.2 限制

| ID | 限制 |
|---|---|
| P-LM-01 | 禁止第二套并行 Pet 状态机（React 动画反推、Skill 私有状态、Agent 旁路写窗）。 |
| P-LM-02 | 气泡是可丢弃瞬时 UI，不写入 Pet Snapshot / 事件流 / 日志。 |
| P-LM-03 | Reduced Motion：禁新自主漫游，进行中移动在帧边界停；交互与显式回家仍可用。 |
| P-LM-04 | 桌面情境采样可驱动避障/hints，默认不采集窗口标题、屏幕像素、会议正文、键鼠内容。 |
| P-LM-05 | lifeform 情境栈与 Presence 栈证据不得混写（见 `DESKTOP_LIFEFORM_CONTEXT.md` / `SYSTEM_CONTEXT_ADAPTERS.md`）。 |
| P-LM-06 | 旧资源缺动作时确定性回退 `pet.idle`（VRM 仅白名单 Preset），不白屏、不冻、不执行任意表达式。 |
| P-LM-07 | 登录自动启动默认关；开启后只恢复离线桌宠宿主，不自动跑 Provider/Agent/AutoMode/Automation/Skill/用户代码/网络。 |

### 2.3 验收标准

| ID | 标准 | 证据类型 |
|---|---|---|
| P-AC-01 | 真机：宠物可拖、点、边缘栖息/攀爬/探头；Free 回 Idle；坐标与 Snapshot 一致。 | 原生 E2E |
| P-AC-02 | 领域：点击/拖拽/Drop/照料事务失败零副作用；Trace 关联 Command/Event。 | Rust 单测 |
| P-AC-03 | 跨源同一 Action Catalog：菜单、用户代码、Automation、Agent 语义一致。 | 契约 + 集成 |
| P-AC-04 | 断网 24h：基础互动、照料、自主、本地持久化可用（`DESKTOP_PET_EXPERIENCE` §8.4）。 | 真机离线 |
| P-AC-05 | 重启：无残留 Dragged；最终落点与 Surface 语义可恢复。 | 跨重启 |
| P-AC-06 | 视觉：全身可见、无错误裁切方框；Preview 不冒充原生手感。 | 视觉 QA |

### 2.4 已确认 vs 待决策

**已确认**
- 宠物 = Subject，产品核心入口。
- Core 权威 + Host 窗口 + Renderer 表现三层分离。
- 封闭动作词表与 Gateway；扩展不得越权移窗。
- 离线陪伴基线独立于 AI。
- 弹簧漫游与边缘语义（Perch/Climb/Peek）为产品规则。

**待决策**
- [ ] 默认角色最终视觉：内置 Q 版“灵灵”是否锁定为唯一出厂，还是允许首启多选（建议：出厂默认灵灵，设置可换包）。
- [ ] 单宠 vs 多宠：M7 才有“多宠”；首稳是否 **硬限制 1 活动宠物**（建议：首稳单活动 Subject，数据模型可预留 ID）。
- [ ] `StructuredPetDirective` 用户可见程度：仅调试 vs 控制中心“意图时间线”（建议：首稳调试+简摘要，完整时间线随后）。
- [ ] 遮挡/避让窗口的激进程度：轻避让 vs 强势找空位（建议：Profile 可调，默认轻避让 + 会议/全屏抑制）。

---

## 3. AutoMode `full_device`

### 3.1 业务规则

| ID | 规则 | 级别 |
|---|---|---|
| A-BR-01 | 产品能力名是 **无人值守授权（Unattended Execution Grant）**；UI 可称“完全设备访问”，底层永远是不可变、可撤销、可审计的范围凭证，不是永久布尔开关。 | MUST |
| A-BR-02 | 五档仅为 UI 模板：Observe / Workspace / Trusted Workspace / Unattended / **Full Device**；权限判断取交集，不取档位标签。 | MUST |
| A-BR-03 | Full Device 映射：`SandboxScope::FullDevice` + `NeverAskWithinGrant` + 默认寿命 **签发 + 4h**；网络可为离线，或在线时 `Unrestricted`。 | MUST |
| A-BR-04 | 实际授权 = `Product hard deny ∩ OS ∩ Org ∩ User Grant ∩ Goal/Plan/Workspace 绑定 ∩ Module/Tool ∩ 预算与数据策略`。 | MUST |
| A-BR-05 | `NeverAskWithinGrant`（sleep-safe）：Grant 有效且绑定精确时，范围内本会确认的 risk/effect **不暂停**；用户离开/锁屏仍可推进。 | MUST |
| A-BR-06 | Sleep-safe **≠** Full Device：Unattended（SelectedRoots + 8h）同样 NeverAsk，但文件范围仍限签发根。 | MUST |
| A-BR-07 | 启动 Full Device / Unattended 前 UI 必须 `tierRequiresDangerAck`，展示硬风险文案；用户确认后仍不可绕过硬禁区。 | MUST |
| A-BR-08 | 绑定漂移（Goal、Plan revision、Workspace fingerprint、Provider、模型、工具集、Capability schema、数据等级、预算扩大、档位、硬策略、过期）→ Grant 失效或须重确认。 | MUST |
| A-BR-09 | 撤销即时生效（工作区/任务中心/CLI/紧急停止）；阻止新派发；可取消工具收 cancellation；未确认外部结果 → `indeterminate`，禁止自动重试。 | MUST |
| A-BR-10 | Skill/Plugin/模型输出/用户代码只能**请求**授权，不能签发、扩大或延长 Grant。 | MUST |
| A-BR-11 | 硬禁区（确认后仍禁止）：支付与身份确认、泄露 Secret、关闭产品安全机制、扩展自行提权、未签名更新、未知结果自动重放、组织强制策略。 | MUST |
| A-BR-12 | Away Summary 至少含：Goal 进度、Session/暂停原因、Grant 状态、预算投影、中文 highlights、风险备注（含 unattended/full_device）、可撤销 Grant 列表；禁止 Secret/隐藏推理。 | MUST |

### 3.2 限制

| ID | 限制 |
|---|---|
| A-LM-01 | 禁止“授权全部权限”永久全局开关；禁止 Prompt 改 Grant。 |
| A-LM-02 | Auto-review（若启用）继承相同或更窄 Sandbox/Grant，不得改参数、扩工具或网络。 |
| A-LM-03 | 浏览器 Preview 不得签发真实 Full Device Grant 或伪造后台 Auto 执行。 |
| A-LM-04 | Recovery/Safe Mode：拒绝新 Job；DB 异常不得降级为内存自动执行。 |
| A-LM-05 | 系统密钥签名 Grant 为发布级安全闭环的一部分；密钥失败不得静默降级为可预测本地密钥（待安全加固项）。 |
| A-LM-06 | Full Device 真机危险矩阵与原生 visual QA 在基线中仍为**剩余/未证明**，不得宣称安全闭环完成。 |

### 3.3 验收标准

| ID | 标准 | 证据类型 |
|---|---|---|
| A-AC-01 | 五档 `tier_policy` 映射正确；Full Device = full_device + never_ask + 4h。 | 单元 + Host |
| A-AC-02 | 无 danger ack 不能启动 unattended/full_device。 | FE + Host |
| A-AC-03 | 绑定漂移后下一步必暂停/重确认，不继续 NeverAsk。 | 领域单测 |
| A-AC-04 | 撤销后无新派发；in-flight 可取消；未知 → indeterminate。 | 集成 |
| A-AC-05 | 硬禁区在 Full Device + 用户已确认下仍拒绝。 | 安全矩阵 |
| A-AC-06 | Away Summary 字段完整且无 Secret。 | CLI + UI |
| A-AC-07 | Preview 路径只读/拒绝签发。 | FE 契约 |
| A-AC-08 | （发布门禁）Full Device 真机危险操作矩阵 + 撤销/过期/漂移用例全绿。 | 真机安全 |

### 3.4 已确认 vs 待决策

**已确认**
- Grant 模型与五档模板；Full Device 高风险、短寿命（4h）、须危险确认。
- NeverAskWithinGrant 的 sleep-safe 语义与非绕过硬禁区。
- 精确绑定与可撤销；模型/扩展不可签发。
- Away Summary 为归来体验的一部分。

**待决策**
- [ ] Full Device 入口默认：始终可见 vs 设置中“解锁危险档位”后显示（建议：**默认隐藏，设置解锁**，降低误触）。
- [ ] Full Device 在线 `Unrestricted` 是否再拆“仅回环 / 用户域名 allowlist / 无限制”（建议：首稳保留 Unrestricted + 审计，后续加 allowlist 子策略）。
- [ ] 4h 是否允许用户在签发时缩短/在硬上限内延长（建议：可缩短；延长 ≤4h 且需再次 danger ack；禁止超过产品硬上限）。
- [ ] 组织策略是否允许彻底禁用 Full Device（建议：允许，OS/Org 优先于用户 Grant）。
- [ ] Auto-review 是否进入首稳（基线仍为剩余；建议：首稳可不做，文档保持可选）。

---

## 4. Goal 持续

### 4.1 业务规则

| ID | 规则 | 级别 |
|---|---|---|
| G-BR-01 | **Goal 是持久领域对象**（SQLite），不是仅内存 Prompt 线程；跨重启可发现、可恢复、可审计。 | MUST |
| G-BR-02 | 状态机：`Draft → Active → Plan revision N → Auto Session Running → Turn Attempt → Continue \| Yield \| Pause \| Completed \| Cancelled`。 | MUST |
| G-BR-03 | **Completion Proof**：模型不得自行宣称完成；`Completed` 仅当当前 Plan 逐步可验证证据成立时提交。 | MUST |
| G-BR-04 | `Continue`：完整 continuation 后，下一轮仍须重验 Workspace、Grant、预算、Cache key。 | MUST |
| G-BR-05 | `Yield`：公平让出 CPU/Provider 配额，不改变业务状态；Supervisor 可续批。 | MUST |
| G-BR-06 | `Pause`：缺输入、需批准、预算耗尽、Workspace/Plan/Grant 漂移、Provider 不可用、未知结果。 | MUST |
| G-BR-07 | 进程退出前请求收敛；无法确认的 Attempt → `indeterminate`，禁止自动重放。 | MUST |
| G-BR-08 | 崩溃遗留 Running Session 在正常启动时原子转为 `paused/restarted`；Active Attempt → indeterminate；恢复投影不占 Session、不建线程、不触发 Provider/Tool。 | MUST |
| G-BR-09 | 同一 Session 最多一个活跃 Job；重复 Start 原子拒绝。 | MUST |
| G-BR-10 | Job Snapshot 只是投影，不是事实源；不得反向覆盖 Session/Checkpoint/Attempt。 | MUST |
| G-BR-11 | 后台 Auto Mode 必须装配 `AutoModeLoopService` + Supervisor；禁止用同步 `resume_auto_mode_turn` 循环冒充后台。 | MUST |
| G-BR-12 | Pause/Cancel/对账经 Host 门禁；对账理由必填，绑定 Attempt、Checkpoint sequence、fingerprint、actor、时间。 | MUST |
| G-BR-13 | 分辨率/对账原子更新 Attempt、Session、Task、Checkpoint，并保留不可变审计（如 `auto_mode_attempt_resolution`）。 | MUST |
| G-BR-14 | Offline-first Agent：本地规则、Goal、计划、Checkpoint、审计、恢复在断网可用；联网能力显式降级。 | MUST |

### 4.2 限制

| ID | 限制 |
|---|---|
| G-LM-01 | 禁止“完成”仅来自模型自然语言或前端乐观状态。 |
| G-LM-02 | 未知结果隔离：命令/网络/工具结果不明时不得自动重放。 |
| G-LM-03 | Safe/Recovery 与退出：取消 Job 并有界等待；超时隔离，迟到 Runner 不得覆盖终态。 |
| G-LM-04 | Browser Preview 后台 Job 一律拒绝/只读。 |
| G-LM-05 | 新 Provider/工具协议经 Adapter 注册，不修改 Goal 核心状态机。 |
| G-LM-06 | Goal 奖励/成长若对接宠物，必须独立可信完成凭证，禁止复用前端信号绕过 Pet 事务。 |

### 4.3 验收标准

| ID | 标准 | 证据类型 |
|---|---|---|
| G-AC-01 | 杀进程后重启：Session 可查询；Running→paused/restarted；indeterminate 可人工对账。 | SQLite 跨重启 |
| G-AC-02 | 无逐步证据不能 Completed。 | 领域单测 |
| G-AC-03 | 单 Session 单 Job；Pause/Cancel 持久收敛。 | Supervisor 测 |
| G-AC-04 | Yield 后续批不丢续体、不双跑。 | 并发测 |
| G-AC-05 | 控制中心展示 Goal/Plan/Checkpoint/Attempt/预算/暂停原因/下一步。 | UI E2E（Native） |
| G-AC-06 | 断网可创建/查看 Goal 与 Checkpoint；需网步骤明确降级。 | 离线测 |

### 4.4 已确认 vs 待决策

**已确认**
- Goal 持久化 + Completion Proof + Grant 绑定。
- 跨重启恢复与 indeterminate 人工对账。
- 后台 Loop/Supervisor 模型；Job 为投影。
- 离线可恢复 Goal 链路。

**待决策**
- [ ] 活跃 Goal 并发上限（建议：默认 1 个 Auto 运行 Goal，其余排队；Power User 可提但共享预算）。
- [ ] 长期 Goal 保留期（建议：Completed/Cancelled 默认 90 天可配；Draft 30 天归档提示）。
- [ ] 是否允许“计划外自动改写 Plan revision”而不暂停（建议：**默认暂停确认**；Trusted/Unattended 下仅允许同 fingerprint 的细化，不允许改目标语义）。
- [ ] Subagent/多 Agent 是否首稳（建议：首稳单主 Agent；Team 后续且权限不继承）。

---

## 5. 本地优先

### 5.1 业务规则

| ID | 规则 | 级别 |
|---|---|---|
| L-BR-01 | **本地数据库是即时事实来源**；云同步（若有）是可选复制层，不是 Core DB。 | MUST |
| L-BR-02 | 网络**不是**启动前置条件；启动不得等待更新、商店、遥测、AI Provider。 | MUST |
| L-BR-03 | 无网络、无登录、无 AI Key、Registry 不可用时，下列必须可用：Pet Runtime/默认角色/已装资源、本地 Command/Profile/养成/成就/记忆、本地 Automation/提醒/通知、不依赖远程的 Skill、本地 Gateway 与回环 Connector、导入导出/诊断/安全模式。 | MUST |
| L-BR-04 | 每个 Skill/Connector/Agent Pack 声明 `offline: full\|partial\|unavailable` 与降级策略；UI 区分离线不可用 / 权限不足 / 服务故障 / 尚未配置。 | MUST |
| L-BR-05 | 密钥进系统安全存储；Gateway 默认关且仅 `127.0.0.1`/`::1`。 | MUST |
| L-BR-06 | 密钥、原始审计、活动窗口、Restricted 数据**默认不参与**同步。 | MUST |
| L-BR-07 | 导出包含 manifest、Schema/应用版本、时间、分类、hash；默认不含密钥、Restricted 正文、设备标识、不可转让商业资源。 | MUST |
| L-BR-08 | 备份：约 15 分钟检查，满 6 小时 Online Backup，默认最多 12 份；临时写→校验→原子发布。 | MUST |
| L-BR-09 | DB 损坏 → L3 隔离恢复：保留原文件、内存仓、停备份、拒正常写；不得把临时内存回写主库。 | MUST |
| L-BR-10 | 默认拒绝、最小权限、按目标授权、可撤销、可审计。 | MUST |
| L-BR-11 | AI 回复不得作为离线知识缓存自动复用。 | MUST |

### 5.2 限制

| ID | 限制 |
|---|---|
| L-LM-01 | 禁止“必须登录才能用桌宠”。 |
| L-LM-02 | 禁止 Core 以云端记录覆盖未同步的本地权威状态（冲突策略按类型，不用全局 LWW）。 |
| L-LM-03 | 不可信代码默认无文件/网络/系统命令/密钥直访。 |
| L-LM-04 | 缓存删除不得影响启动与用户内容。 |
| L-LM-05 | 登录自动陪伴开启时仍本地优先：不自动联网任务。 |
| L-LM-06 | 自由文本运行日志若未实现，不得假称可导出完整诊断包。 |

### 5.3 验收标准

| ID | 标准 | 证据类型 |
|---|---|---|
| L-AC-01 | 断网冷启动成功；60s 内可完成一次有效互动。 | 真机离线 |
| L-AC-02 | 无 Provider 时桌宠与本地自动化可用；AI 入口明确“未配置/离线”。 | 功能 |
| L-AC-03 | 备份生成与从已验证备份恢复路径可测；失败进健康状态。 | 集成 |
| L-AC-04 | 注入 DB 损坏进入 L3，无静默丢数据。 | 混沌 |
| L-AC-05 | 导出/导入：校验→预览→冲突→快照→事务→健康检查。 | 功能 |
| L-AC-06 | Gateway 默认不监听非回环。 | 安全 |

### 5.4 已确认 vs 待决策

**已确认**
- 本地权威 + 离线可启动/可陪伴。
- 云可选；敏感数据默认不同步。
- 备份与 L3 恢复等级。
- 开放入口默认关闭、最小权限。

**待决策**
- [ ] 首稳是否包含任何云同步 MVP（建议：**不含**；只本地导出/导入）。
- [ ] 多设备同一用户的冲突与设备身份（建议：稳定版后 RFC）。
- [ ] 本地模型（llama 等）是否默认捆绑（建议：不捆绑，可选安装；无模型时离线 Agent 用规则/模板降级）。
- [ ] 遥测：默认全关 vs 可选匿名崩溃报告（建议：**默认全关**，显式 opt-in）。

---

## 6. 性能预算

### 6.1 业务规则

| ID | 规则 | 级别 | 量化目标 |
|---|---|---|---|
| R-BR-01 | 空闲（桌宠可见、无拖拽、无 Agent 重任务、默认角色）资源占用有发布门禁。 | MUST | CPU P95 **< 3%**；常驻内存 P95 **< 180 MB**（`PRODUCT_SPEC` §6） |
| R-BR-02 | 冷启动（已装资源）有上限。 | MUST | P95 **< 2.5 s** |
| R-BR-03 | 长稳：无崩溃/死锁/句柄泄漏；内存增长受限。 | MUST | 24h soak 无崩溃；空闲内存增长 **24h < 10%**；桌宠专条 **连续 8h** 无明显内存增长/GPU 泄漏/CPU 空转/通知风暴 |
| R-BR-04 | 故障隔离：单扩展崩溃不终止 Core。 | MUST | 30s 内可禁用；扩展崩溃不影响核心会话 ≥ 99.9% |
| R-BR-05 | 不可见宠物降帧或暂停渲染；自主在后台/锁屏/低电/热/GPU 压力下自动降频。 | MUST | — |
| R-BR-06 | 降级顺序固定：粒子 → 阴影 → 动画帧率 → 非活跃纹理；**不**先停核心交互。 | MUST | — |
| R-BR-07 | 扩展独立 CPU/内存/定时器/消息预算；越界限流或 quarantine。 | MUST | — |
| R-BR-08 | 网络与 Provider 请求不得在动画主循环；失败不得冻拖拽/菜单/托盘。 | MUST | — |
| R-BR-09 | 资源包/Importer Worker 有硬限额（如 80 MiB 输入、1 MiB JSON、64 KiB 协议输出、节点/纹理上限等）。 | MUST | 见 `MODEL_RENDERING_IMPORT` |
| R-BR-10 | 性能档位：Eco / Balanced /（Quality）/ Capture 等可切换；声明平台 GPU 能力与降级路径。 | SHOULD | — |
| R-BR-11 | Renderer 报告加载、帧率、内存、GPU context 丢失与降级原因；不采集桌面内容。 | MUST | — |
| R-BR-12 | 自主 Motion 前申请环境预算；拒绝则以 Quiet 语义原子收敛，禁止先移窗再补限流。 | MUST | — |

### 6.2 限制

| ID | 限制 |
|---|---|
| R-LM-01 | 禁止以“机器很强”为无预算合并理由。 |
| R-LM-02 | 单元测试通过 ≠ idle CPU/内存门禁通过；须真机采样。 |
| R-LM-03 | Browser Preview 性能数字不得作为发布 NFR 证据。 |
| R-LM-04 | 事件风暴下有界队列/采样/debounce；订阅者慢不得阻塞 Event Bus 发布者。 |
| R-LM-05 | 纹理/音频/日志/事件历史/Agent 会话均设上限。 |
| R-LM-06 | 当前里程碑明确：**idle CPU 预算测量、原生 visual QA、签名包**仍为未证明门禁。 |

### 6.3 验收标准

| ID | 标准 | 证据类型 |
|---|---|---|
| R-AC-01 | 参考机（须文档固定型号）空闲 30min：CPU P95 < 3%，RSS P95 < 180 MB。 | 真机 perf |
| R-AC-02 | 冷启动 P95 < 2.5s（样本 ≥ 20，清缓存策略文档化）。 | 真机 perf |
| R-AC-03 | 8h 桌宠 soak + 24h 日常 soak + 72h 低频 soak 按 `RELIABILITY_RESILIENCE`。 | Soak |
| R-AC-04 | 1h 事件风暴：无 OOM、无限热循环、通知风暴。 | 压测 |
| R-AC-05 | 100 次启动/退出；50 次资源/Profile 热切换无泄漏失控。 | 循环 |
| R-AC-06 | GPU context 丢失可重建或回退内置角色；扩展崩溃可隔离。 | 混沌 |
| R-AC-07 | 控制中心可展示 CPU/内存/事件速率/Token 预算（只读健康）。 | UI |

### 6.4 已确认 vs 待决策

**已确认**
- NFR 数字：CPU < 3%、内存 < 180MB、冷启动 < 2.5s、24h 增长 < 10%、8h 桌宠长稳。
- 降级顺序与扩展隔离预算。
- 真机证据门槛；Preview 不作 NFR 证明。
- 当前实现**尚未**交付 idle CPU 签字证据。

**待决策**
- [ ] 官方参考机型矩阵（建议最低：Win11 核显笔记本 + macOS Apple Silicon 8GB；Intel Mac 是否门禁另定）。
- [ ] 180 MB 是否含 WebView 共享缓存（建议：**进程私有 RSS** 为主指标，并单列 GPU/WebView 附件指标）。
- [ ] Eco 档是否为低电量默认（建议：系统低电量自动建议 Eco，不强制改用户档除非用户授权）。
- [ ] Agent 重任务期间空闲预算是否豁免（建议：交互期允许抬升，但须有 Token/时间预算与“忙碌”可见态；任务结束 60s 内回到空闲预算）。

---

## 7. 跨主题不变量（已确认）

下列规则跨六大主题同时成立，冲突时以**更严**者为准：

1. **原生权威**：系统能力与窗口事实只在 Desktop Host；Web/Preview 不能证明原生完成。  
2. **宠物是 Subject**：能力系统服务生命体体验，而不是反过来把宠物降级为 Agent 面板挂件。  
3. **Gateway 唯一执行面**：AI、Skill、用户代码、Automation 不得绕过 Capability / 授权 / 预算。  
4. **本地权威数据**：断网可陪伴、可恢复；云可选。  
5. **失败可解释**：Pause / indeterminate / L3 / danger ack 必须给用户中文可行动下一步。  
6. **证据分层**：单元/契约/Preview/真机/签名包不得互相替代。  
7. **禁止虚假完成**：不得因文档存在或局部接线宣称 Goal complete、Grant 安全闭环或 idle CPU 已达标。

---

## 8. 决策看板（待产品拍板）

| 优先级 | 主题 | 待决策项 | 建议默认 | 阻塞？ |
|---|---|---|---|---|
| P0 | full_device | 入口默认隐藏并需设置解锁 | 是 | 否（实现可按建议） |
| P0 | 性能 | 固定参考机型与 RSS 口径 | 见 §6.4 | **是**（无口径则 NFR 无法签字） |
| P0 | 原生 | Win+mac 是否同版门禁 | 同版门禁 | 是（发布范围） |
| P1 | 宠物 | 首稳单活动 Subject | 是 | 否 |
| P1 | Goal | 并发 Auto Goal=1 | 是 | 否 |
| P1 | 本地优先 | 首稳无云同步 | 是 | 否 |
| P2 | full_device | 时长可缩短、延长需再 ack | 是 | 否 |
| P2 | 性能 | 低电量建议 Eco | 是 | 否 |
| P2 | Goal | Subagent 首稳不做 | 不做 | 否 |
| P3 | 原生 | Linux 时间表 | 首稳后 | 否 |

---

## 9. 建议的下一轮确认问题（给产品/用户）

请仅对**待决策**拍板（是/否/改值）：

1. Full Device 档位是否 **默认隐藏**，仅在设置中解锁后显示？  
2. 首个稳定版是否 **强制** Windows + macOS 双平台真机门禁同版发布？  
3. 性能签字参考机是否接受：**Win11 核显笔记本 + Apple Silicon 8GB**，指标用进程私有 RSS？  
4. 首稳是否确认 **不做云同步**，仅本地导出/导入？  
5. 首稳是否确认 **同时仅 1 个 Auto 运行中的 Goal**？

---

## 10. 文档维护

| 项 | 说明 |
|---|---|
| 变更方式 | 需求变更先改本文决策表，再 cro 到对应规范（ADR/PRODUCT_SPEC 等） |
| 非目标 | 本文不替代实现状态（见 `IMPLEMENTATION_STATUS.md`）与里程碑证据（见 `MILESTONE_REVIEWS.md`） |
| 范围 | 只读仓库；**仅**维护本文件路径 |

---

## 11. 摘要（一页纸）

| 主题 | 一句话已确认 | 最大缺口 |
|---|---|---|
| 原生 vs 浏览器 | Tauri 原生是唯一生产运行时；Preview 不冒充 | 双平台真机签字包与 visual QA |
| 宠物主体 | Subject + Core 权威 + 封闭动作 + 离线可玩 | 真机手感与全身可见无裁切 |
| full_device | 4h Grant + danger ack + 硬禁区 + 可撤销 | 密钥签名闭环与真机危险矩阵 |
| Goal 持续 | 持久 Goal + 证据完成 + 跨重启对账 | 并发策略与计划自动改写策略产品化 |
| 本地优先 | 本地 DB 权威，启动不等网 | 明确首稳无云；遥测默认关 |
| 性能预算 | 3% CPU / 180MB / 2.5s / 8h·24h soak | **真机 idle 测量未完成** |

**总评**：六大主题在既有规范中**方向已高度确认**；当前主要不是“要不要做”，而是 **（1）少量产品默认值拍板** 与 **（2）真机/安全/性能证据闭环**。实现与文档深度足够支撑工程，但**不得**将接线完成误报为发布完成。

---

*生成：需求确认 Agent · 2026-07-24 · 中文 · 只写本文件*
