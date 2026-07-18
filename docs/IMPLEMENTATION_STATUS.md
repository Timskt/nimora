# Nimora 全量实现状态与证据矩阵

> 更新日期：2026-07-18
> 适用阶段：首个稳定版之前  
> 原则：优先级只决定施工顺序，不缩减产品范围；“已完成”必须同时具备真实实现、失败边界、自动化证据和同步文档。

## 状态定义

| 状态 | 判定标准 |
|---|---|
| 已验证 | 真实实现、正常与异常测试、文档和适用运行证据齐全 |
| 部分实现 | 已有可运行纵切，但规格中仍有明确能力或平台证据缺口 |
| 未实现 | 只有规格、契约占位或尚无可运行代码 |
| 外部验证受阻 | 实现可继续，但当前环境缺少设备、签名身份或可连接的浏览器/平台实例 |

## 全量能力矩阵

| 领域 | 当前状态 | 已有证据 | 必须继续闭环的缺口 |
|---|---|---|---|
| 桌宠窗口与交互 | 部分实现 | Tauri 透明双窗口、拖拽、置顶、穿透、托盘、安全模式、Click/Drag FSM 与 Rust 测试 | Windows/macOS 真机冒烟、多屏/DPI、WebView 崩溃恢复、长稳验证 |
| UI 与设计系统 | 部分实现 | Control Center、Creator Studio、Overlay、Token 与组件样式、前端单测和构建 | 浏览器真实截图、键盘/读屏/200% 缩放、关键状态视觉回归、跨平台像素审查 |
| Profile 与离线状态 | 部分实现 | 唯一 SQLite Schema、离线 Profile、Online Backup 调度与原子恢复、损坏数据库隔离启动、统一写门禁、恢复 UI、脱敏诊断摘要导出、Rust/TS 故障测试与 Chrome 实测 | 休眠与时钟异常、恢复模式真实桌面截图、跨平台真机故障注入、人工数据提取与跨设备备份 |
| Event 与持久 Outbox | 部分实现 | 事务写入、租约、ACK、重试、死信、清理、健康状态和自动化测试 | 具体幂等消费者、跨重启投递恢复、Connector 投递审计 |
| Sprite 角色与皮肤 | 部分实现 | 严格包契约、安全导入导出、序列/图集真实渲染、动作 fallback | 独立预览实例、命中区编辑、连续切换泄漏与性能门禁 |
| glTF/GLB 角色 | 部分实现 | 独立 Worker 探测、命名动画报告、可编辑标准动作映射、原子安装、受控协议、Three.js 真渲染、cross-fade、framing、释放与失败回退 | 独立预览、持续切换与 GPU 压测、真实截图和跨平台验证 |
| VRM 与 Live2D | 未实现 | Manifest 可识别并显式安全回退 | 独立 Adapter、格式验证、动作/表情映射、许可证策略、隔离与资源释放测试 |
| 主题、声音与行为包 | 未实现 | 规格和统一 Asset Manifest 基础 | 严格子契约、编辑预览、原子安装切换、权限和回退实现 |
| 用户代码执行 | 部分实现 | 独立 JS Worker、预算、强制取消、安装完整性、版本精确授权、Capability Gateway | 授权型文件/网络/自动化后端、调试器、录制回放、跨平台 sidecar 发布验证、OS 资源硬限制 |
| 事件驱动程序 | 部分实现 | 可信 Rust 订阅、`serial`/`drop`/`cancel-previous` Supervisor、迟到完成隔离 | 桌面纯 Supervisor 集成测试、执行历史与 Creator Studio 可视诊断 |
| 扩展与 Skill 生态 | 部分实现 | `skill-runtime` 实现严格 Manifest、精确 `commandAllowlist`、`onEvent:*` 与 `subscribe-events` 强绑定、命名空间 Contribution、精确授权、激活租约、恢复/quarantine/卸载和 `skill:<id>` requester；独立 `skill-host`/`skill-worker` 实现版本化 JSONL、清空环境、真实进程隔离、Boa JavaScript、取消/超时/输出预算、结构化 Command/Agent Task 计划及 Worker 故障驱动的 Contribution 撤销；`skill-package` 实现原子安装、完整库存与 SHA-256 复验、备份回滚；Desktop 实现安装、目录、授权、启停、回滚和执行 IPC，执行绑定 Activated Manifest 租约，Agent Task 接入统一 Module Adapter 与 Agent History，Command 整批预检 allowlist/注册风险后将 Safe/Low 经共享 Capability Gateway 执行，Medium/High 使用 SQLite 保存参数绑定的五分钟一次性整批批准；Journal 原子 claim、单事务拒绝/过期、完成/失败终态、重启 pending 恢复与 executing 中断、待批准列表 IPC；获准 Activated Skill 的事件声明使用独立有界 Runtime Event Bus 订阅与串行调度，Host 重建、停用、升级、回滚、Safe Mode 和故障撤销会话并取消在途 Worker/Provider，代际 ID 隔离迟到线程；活跃执行支持按 execution 取消 Worker、Command 后续副作用及当前 Provider Agent；Skill 执行元数据历史支持等待/完成/拒绝/取消/失败状态收敛、稳定游标分页与单条/全部隐私删除，取消终态不可被迟到结果覆盖，且不保存输入、源码、命令参数或 Agent 正文；拒绝/过期/重复批准 fail-closed，未知/未声明在副作用前拒绝，Recovery Mode 隔离扩展 | 跨文件系统与 SQLite 的安装崩溃一致性 Journal、发布者签名、OS CPU/内存沙箱、事件会话可视诊断与 UI/Tool 等 Contribution、管理 UI、官方番茄钟与提醒 |
| 自动化引擎 | 部分实现 | 独立 `automation-runtime`；版本化 Event Trigger、JSON Pointer Condition、顺序 Action 与 Policy；有界取消/超时；幂等且 retry-safe 的瞬时重试；失败逆序补偿；结构化 Run 结果；Backend 获得宿主生成且覆盖动作/重试/补偿的 Run/Automation/Action/Event/Trace 因果上下文；Engine 支持宿主预分配稳定 Run ID；SQLite Run Journal 在副作用前记录 running、原子完成、严格身份/状态转换、桌面启动恢复遗留 running 为 interrupted，原生 IPC 与 TS 平台支持按 Run ID 查询；Live Run 宿主取消注册表、原生取消 IPC 与 TS 契约会置位父 Run 并按持久 Journal 级联取消活跃 Agent 子任务和 Provider Worker；原生 IPC 与浏览器同契约零副作用测试运行；自动化工作区 UI；Desktop Live IPC 通过共享 Capability Gateway 执行宿主批准的 Pet Action，Safe/Recovery、未知动作和风险低报 fail-closed；`automation-agent-bridge` 将固定 `agent.task.run` 经风险、幂等、不可信上下文门禁和统一 AgentTaskGateway 转为以 Automation Run 为根的共享 Trace/层级预算子任务；桌面 Submitter 已复用 Provider/Coordinator/Tool/Gateway/审批/历史链路，显式模型与 Tool Allowlist 贯穿确认续跑，根 Run + 幂等键阻止内存生命周期内重复提交；Rust/TS 测试 | 持久异步结果回填、跨进程运行中恢复、运行列表/保留删除、Context Admission/Prompt Injection 检测、并发/冷却/费用门禁、不可取消副作用补偿；扩展 Profile/Character/Program Action 宿主策略；Interval/日历/快捷键等触发器、分支与并行汇合、持久规则、虚拟时间与事件回放 |
| 开放 Gateway 与 Connector | 未实现 | Capability Gateway 为进程内用户代码提供窄能力边界 | 配对、Token/Scope、REST/WS/SSE、HTTP/UDP Sink、Source Connector、2 秒安全停机 |
| AI Agent 与 CLI | 部分实现 | Tool Registry、风险与批准、任务硬预算；Provider/Tool 单步协调；Provider 续跑的 Assistant Call 与 Tool Result 强关联协议；多调用 Turn 完整聚合器；Ollama Worker 结构化续跑载荷及真实独立 Worker 双轮 Tool Call 自动化；十三项生产模块工具，自动化定义零副作用验证、角色渲染能力脱敏摘要、Runtime 动作词汇发现、原生策略事务化 Profile 切换、资产复验与刷新回滚型角色切换、完整性复验且路径脱敏的用户程序发现、绑定精确版本批准的隔离程序执行、可扩展只读能力集合及共享 Capability Gateway 固定映射；Automation Context Admission 将受信目标与来源化不可信数据分离，实施段数/字节预算、高置信中英文注入拒绝，并把不可信任务限制为 draft 且无工具，桌面 Provider 保留 untrusted 消息位；完成任务历史的版本化 SQLite 仓储、稳定游标分页和隐私删除；桌面完成态写入、降级不篡改结果、Recovery 内存隔离、分页/删除 IPC、最近历史 UI 与 Preview 会话实现；CLI 完成态旁路写入、显式数据库导出、成对游标分页和单条/全部删除；统一桌面 Agent 结果契约；Provider 等待确认、多确认项展示、部分批准后剩余项回填、最后批准后 Provider 续跑与整组拒绝 UI；构建期嵌入 Ollama Manifest 信任摘要，桌面启动时复验 Manifest 与 Worker 后自动注册；受隔离 Worker 的 `/api/tags` 健康探测、严格模型目录预算、去重排序、真实 Provider 状态与模型选择；桌面宿主对写调用整组批准前零副作用，拒绝/过期级联撤销，全部批准后按原始顺序执行；Safe/Recovery Mode fail-closed；CLI 工具发现与 Ollama 接线；独立 sidecar、loopback-only SSRF、Worker 完整性发现、有界协议、超时取消强杀和跨进程 mock 测试 | 用户本机实际 Ollama 模型桌面验收、继续扩展生产工具覆盖面、OpenAI-compatible Adapter、发布者数字签名、持久计划恢复、跨来源完整 Prompt Injection 防护与 Unicode/编码混淆检测 |
| 包签名与 Registry | 未实现 | SHA-256 本地完整性锁和原子回滚 | 发布者签名、信任根、撤销、Registry、兼容检测、更新策略、离线 CLI 验证 |
| 安全与隐私 | 部分实现 | 安全模式、精确授权、路径/符号链接防护、Worker 隔离、审计边界文档 | OS 沙箱、系统密钥存储、网络目标策略、隐私面板、威胁模型自动门禁 |
| 稳定性与可观测性 | 部分实现 | 结构化错误、Trace、Outbox 健康、故障回退、版本化脱敏诊断摘要包、14 天/1 MiB/64 段有界固定事件日志、跨重启恢复、损坏尾行跳过、存储失败内存降级、用户可取消事件导出及单元/集成测试 | 自由文本日志与筛选、用户选择 Trace、指标、崩溃循环恢复、资源预算监控、soak/chaos 和 SLO 门禁 |
| 部署与发布 | 部分实现 | pnpm/Rust 构建、Tauri 配置、CI 规范 | Windows/macOS 签名安装包、公证、更新回滚、SBOM、许可证、发布演练 |
| Git 与工程治理 | 部分实现 | Git 规范、Conventional Commits、pnpm-only 门禁、全量本地质量命令 | 受保护分支/PR 实际仓库配置、跨平台 CI、依赖与安全扫描证据 |

## 执行规则

AI 双向模块交互的当前审计与完整实施蓝图见 [`AI_MODULE_INTERACTIONS.md`](AI_MODULE_INTERACTIONS.md)。现阶段 AI 到 Pet、Profile、Character、Asset、Program、Diagnostics 和 Automation Validation 已形成生产工具链；宿主无关的 `AgentTaskGateway` 已完成调用方、Provider、Tool、数据等级、主动性、递归深度和父级剩余预算交集准入。统一 `module-agent-adapter` 已封装可信模块身份、Provider allowlist、固定 Draft/无工具策略、Context Admission、相关 Trace 和 Provider 消息边界；用户程序与 Skill 均已成为生产调用方。Skill Runtime、独立 Worker、Package Core、Desktop IPC、启动恢复和 SQLite 精确版本授权/启用状态仓储已贯通 Manifest、授权、贡献租约、崩溃隔离、原子安装、完整性复验、回滚、Command 调用、Agent Task 提交、活跃执行取消、脱敏历史和 `onEvent:*` 自动调度。获准 Activated Skill 的 Agent Tool Contribution 已动态汇入 Desktop Catalog、Provider Tool Turn、独立调用与批准续跑共用的 Registry；独立 Capability、命名空间、Schema/元数据预算、精确命令映射、宿主风险复核、参数绑定批准、Capability Gateway 执行和停用撤销均已贯通。Coding Agent 级持久 Goal/Plan 核心、SQLite 双表修订仓储和机器可读 CLI 已贯通，完成必须由当前计划逐项证据证明。Auto Mode 已完成策略/会话领域层、逐步风险与预算准入、SQLite 乐观并发持久化、重启安全暂停、跨进程 CLI 控制面、整轮预检与安全只读 Tool continuation，以及版本化 Checkpoint 领域对象和 SQLite 单调序号 CAS 仓储；Checkpoint 精确绑定 Task/Goal/Plan/Workspace/Policy 并复验索引元数据，且不保存 Approval 或宿主对象。独立 Auto Host 恢复服务已跨 Goal/Session/Checkpoint/Workspace 仓储复验绑定并真实重扫工作区，只释放 paused Task 与 continuation 数据，文件漂移、缺失状态或非暂停 Session 均 fail-closed，且不调用 Provider/Tool。Agent Runtime 已新增可追溯 Context Compactor、内容寻址 TTL/LRU 内存 Cache，以及不可变 Workspace 文件快照与父指纹版本链；Tool continuation 不会被压缩拆分，路径逃逸、重复文件、篡改指纹和错误父版本均 fail-closed。SQLite Context Cache 已实现事务化 TTL 清理、稳定 LRU 条目/总字节治理、数据等级命中门禁、内容与索引复验及按 Workspace fingerprint 失效。独立 Workspace Host 已贯通 canonical root、symlink 拒绝、有界扫描、Git/Nimora ignore、TOCTOU 复核、Git HEAD/index/worktree 状态与 `ai workspace inspect` 机器可读 CLI，且对外只暴露相对路径与不可逆 root fingerprint。SQLite Workspace Snapshot 仓储已实现 revision/parent/root/fingerprint CAS 和索引/Payload 复验；Auto Mode 启动改为宿主真实扫描绑定，恢复时复扫根目录，内容漂移则保存后继快照、保持暂停并稳定返回 `workspace-changed`，不再接受调用方伪造 revision。尚未连接桌面 Gateway 持久宿主循环、恢复候选到显式 Resume/Provider 下一轮的原子提交、持久 Cache 的 Auto Host Loop 接入与系统密钥加密、Auto Mode 桌面每轮扫描、Session 显式重绑定与双仓储原子应用服务。尚未贯通的是 Contribution 与事件会话管理 UI、Connector Runtime 的模块反向调用链，以及多 Agent 调度、Auto Mode 桌面监督和桌面 Goal UI。

Automation、User Program 与 Skill 已复用共享 Context Admission，具备来源专属硬上限、Unicode 混淆检测、稳定拒绝原因和无正文结构化审计。Desktop 仅持久化脱敏来源类别、计数及 Run/Trace/Automation/Action/Command Execution 或 Module/Module Execution 关联 ID；Prompt Injection 原文不进入 Provider、History、序列化或 Journal，审计写入故障保持 fail-closed。Connector、Clipboard、Files 与 Vision 接入同一生产准入和审计通路仍是明确缺口。

1. 每次开发从本矩阵选择一个或多个可形成真实纵切的缺口，但不得删除其余缺口。
2. 新发现的有价值能力必须加入矩阵或对应权威规格，不依赖聊天记忆。
3. 只有证据满足状态定义时才能改为“已验证”；单元测试不能替代真实 UI、平台或安全边界验证。
4. 首个稳定版前直接维护唯一当前契约，不为尚未发布的设计制造迁移链或兼容包袱。
5. 每个切片完成后更新矩阵、功能测试计划、相关专题文档，并提交推送主仓库。
### Auto Mode 原子恢复补充

Auto Host 已进一步实现显式恢复原子提交：Session timestamp 与 Checkpoint sequence 双 CAS 在同一 SQLite Immediate 事务中将 Session/Task 同时恢复为 `running`，任一竞争整体回滚，且不触发 Provider/Tool、不复用 Approval。尚未完成的是恢复后的单轮执行结果持久提交、每轮 Workspace 重扫、持久 Cache Host Loop 接入与系统密钥加密。

Auto Host 上下文准备已接入持久 Context Cache：完整 continuation 经协议安全压缩后，以 Provider、模型、Plan revision、Workspace fingerprint、消息内容和数据等级进行精确命中；miss 写入沿用 SQLite TTL/LRU/容量治理。尚未完成 Provider/Tool 单轮结果提交、每轮 Workspace 重扫、缓存系统密钥加密与指标控制面。

Auto Host 每轮 Workspace 门禁已实现真实有界重扫；一致时才允许继续，漂移时以 Session/Checkpoint/Workspace 三重 CAS 原子暂停 Session 与 Task 并追加 successor，竞争写入整体回滚且零 Provider 释放。单轮结果提交服务已支持 Continue、Paused、Completed 三类结果，将 Session/Task 生命周期、完整 continuation 与单调 Checkpoint sequence 以 timestamp + sequence 双 CAS 原子提交。调用前 durable Turn Attempt 已精确绑定 Session、Checkpoint、timestamp 与请求指纹且禁止过期重领；结果事务原子消费 Attempt，重复 Begin、陈旧提交与崩溃遗留均 fail-closed，遗留 Attempt 转为 indeterminate 并阻断 continuation 自动恢复。尚未完成该链路与桌面持久监督循环的生产执行接线、人工处置/对账 UI、缓存系统密钥加密与桌面控制面。

Auto Host 已将上述独立能力组合为生产单轮执行 Facade：真实 Workspace 预检、Context Cache、durable Attempt、Provider/Tool Supervisor 和原子 Commit 按固定顺序执行。真实 SQLite/Workspace 测试覆盖 Provider 完成、安全只读 Tool continuation、写 Tool 整批零派发暂停、Provider 失败 indeterminate 隔离及漂移前置退出；Checkpoint 同时支持验证历史 Tool Call/Result 结构而不持久化 Tool Descriptor。尚未完成桌面后台持续监督循环、Goal/Plan/Attempt 人工处置 UI、缓存系统密钥加密和跨重启桌面端到端验证。
