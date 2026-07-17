# Nimora AI Agent 与 CLI 架构规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-18
> 状态：实现基线

## 1. 产品边界

AI 是 Nimora 的可选增强运行时，不是桌宠、自动化、用户代码或本地数据的启动依赖。桌面 UI、`nimora ai` CLI、Automation、Skill 和其它宿主模块共享同一个 Agent Runtime；任何入口都不得建立绕过权限、风险确认或审计的第二条执行路径。

Agent 能力包括对话、计划、工具调用、任务暂停/继续/取消、历史、Provider 切换、Agent Pack、受控记忆和保存为自动化。没有网络、账户、API Key 或可用 Provider 时，非 AI 能力继续工作并返回稳定降级状态。

## 2. 跨模块交互

完整的双向调用契约、模块能力矩阵、递归控制、离线降级和测试门禁见 [`AI_MODULE_INTERACTIONS.md`](AI_MODULE_INTERACTIONS.md)。本文继续定义 Agent Runtime、Provider 与 CLI 的实现细节。

```mermaid
flowchart LR
  UI[Desktop UI] --> TASK[Agent Task Service]
  CLI[nimora ai CLI] --> TASK
  MOD[Automation / Skill / Module] --> TASK
  EVENT[Trusted Event Admission] --> TASK
  TASK --> PROVIDER[Provider Adapter]
  PROVIDER --> PLAN[Plan and Tool Requests]
  PLAN --> REG[Tool Registry]
  REG --> RISK[Argument Risk Evaluator]
  RISK --> APPROVAL[Approval Binding]
  APPROVAL --> CAP[Capability Gateway]
  CAP --> COMMAND[Command / Query / Module Adapter]
  COMMAND --> RESULT[Typed Result and Audit]
  RESULT --> TASK
```

### 2.1 AI 调用其它模块

- 模块通过 Contribution Manifest 注册 Tool Descriptor，不向 Agent 暴露数据库、内部对象、文件路径或系统句柄。
- Tool 必须声明输入/输出 Schema、基础风险、数据分类、副作用、幂等性、取消支持和超时。
- 实际风险取 Manifest、Capability、底层 Permission、调用参数和当前环境风险的最大值；模型不能降低风险。
- Read-only Safe/Low 工具可按用户策略自动执行；所有写入或外部副作用默认确认，Medium 及以上即使只读也必须确认。
- 用户批准与 `taskId + invocationId + traceId + toolId + risk + arguments` 指纹绑定；参数变化后批准失效。
- 调用最终进入 Capability Gateway，并映射为统一 Command、Query 或专用模块 Adapter；禁止直接调用模块内部函数。

### 2.2 其它模块调用 AI

- Desktop、CLI、Automation、Skill、Module 和受信 Event Admission 均可创建 Agent Task。
- 请求方必须具有 `agent.task.create` Capability，并声明允许的 Provider、Tool allowlist、数据分类、主动性和预算上限。
- 模块可以查询任务摘要、订阅状态、提交用户批准、暂停、继续或取消自己创建的任务；不能读取其它命名空间的 Prompt、记忆或结果正文。
- Event 不能直接成为 Prompt。先经过来源信任、Schema、速率、去重、数据分类和 Prompt Injection 标记，再生成受界定上下文。
- Agent 结果若触发模块动作，仍必须重新进入 Tool Registry 和 Capability Gateway，不能把模型文本当作已授权 Command。

## 3. Tool 契约

当前 Rust 基线位于 `crates/agent-runtime`：

- `nimora.agent-task-request/1` 与 `nimora.agent-task-admission/1`：模块创建任务的统一准入契约；`AgentTaskGateway` 对调用方、来源、Provider、Tool allowlist、数据等级、主动性、调用深度和层级预算执行交集授权。
- `nimora.agent-tool/1`：模块工具描述。
- `nimora.agent-tool-invocation/1`：单次具体参数调用。
- `nimora.agent-tool-approval/1`：与调用及风险绑定的批准证明。
- Registry 最多加载 512 个 Tool，单个输入或输出 Schema 最大 64 KiB。
- Tool ID 使用至少三段的小写点分命名，例如 `core.pet.state-read`、`skill.timer.session-start`。
- Tool Backend 只能收到描述、受控参数、Trace 和超时，不获得 Provider 凭据或 Agent 内部记忆。
- `AgentCoordinator` 把模型推进与工具执行拆成独立的确定性单步：Provider 返回的 Tool Call 先转换为新的 `ToolInvocation`，再经过 Registry admission；模型响应不能直接触发 Backend。
- Provider 续跑消息保留 Assistant 的结构化 Tool Call、Provider Call ID 与 Tool ID；Tool Result 必须引用先前尚未解析的同一调用。未知工具、错配工具、孤立结果和重复结果在进入 Adapter 前拒绝，避免模型把无关模块输出冒充成已授权调用结果。
- `ProviderToolTurn` 以 Provider 原始调用顺序聚合一个 Turn 的结果；未完成、错配和重复结果不能生成续跑消息。宿主因此可以并行执行只读调用、逐项等待写操作确认，但只能在全部调用成功后把完整结果交回 Provider。
- 工具执行单步必须校验 Task/Trace 归属，在真正调用模块 Capability Gateway Backend 前扣减工具预算，并重新验证批准指纹。

首个生产工具目录位于 `crates/agent-tools`，当前公开十三项工具：`asset.catalog.read`、`automation.definition.validate`、`character.state.read`、`pet.action.catalog.read`、`pet.state.read`、`profile.state.read`、`program.catalog.read`、`runtime.health.read`、`pet.animation.play`、`pet.position.move`、`profile.active.switch`、`character.active.switch` 与 `program.installed.execute`。目录只包含 Tool Descriptor 和固定模块 Adapter，不暴露 `DesktopState`、Repository、Tauri Command、任意命令字符串或文件路径。只读授权使用可扩展能力集合；自动化验证接收有界定义与测试事件，只调用 `AutomationEngine` 的 Dry-run 模式并返回 `planned`、不匹配或校验失败结果，Backend 若收到任何真实 Command 会失败；资源目录只返回已验证资产摘要，角色状态只返回当前 Asset ID、渲染后端、画布/锚点和能力布尔值，主动剔除模型路径与资源 URL；动作目录直接从 Runtime 的 `PetAction` 当前词汇生成，并指明对应写工具与参数，避免 Provider 猜测动作；程序目录只返回完整性复验通过的已安装程序身份、Manifest 声明、预算与精确版本授权摘要，损坏项只计数，不返回源码、安装路径、Worker 路径或宿主句柄；运行健康只返回启动、安全、Outbox 与备份摘要，不包含日志、用户正文、路径或密钥。

### 3.1 模块互动覆盖矩阵

| 模块 | 模块向 AI 提供能力 | AI 向模块发起操作 | 当前状态 |
| --- | --- | --- | --- |
| Pet Runtime | 状态、动作目录 | 播放动作、移动位置 | 已贯通 Gateway |
| Profile | 当前 Profile 与策略摘要 | 切换已有 Profile | 已贯通 Gateway，写操作确认 |
| Character / Asset | 当前角色、渲染能力、已安装资产 | 切换已验证角色 | 已贯通 Gateway，路径脱敏 |
| User Program | 已安装程序与精确版本授权摘要 | 执行指定程序版本 | 已贯通隔离 Worker，外部副作用确认 |
| Runtime Health | Safe/Recovery、事件和备份健康摘要 | 无写入口 | 已贯通只读 Gateway |
| Automation | 有界定义与测试事件 | 零副作用验证和 Dry-run | `automation.definition.validate` 已贯通；规则持久化、启停和真实运行尚未开放，AI 不得绕过自动化仓储和 Action Gateway |
| Diagnostics / Backup | 脱敏诊断、备份健康 | 导出诊断、创建或恢复备份 | 仅健康摘要已提供；导出与恢复应采用高风险专用 Tool |
| Extension / Skill | Contribution Catalog、Skill 状态 | 安装、启停、执行 Skill | Extension Host 未实现，禁止预留任意命令工具 |
| Connector | 已配对连接与 Scope 摘要 | 发送、订阅、断开连接 | Gateway 尚未实现；网络目标、数据分类和用户确认必须叠加 |
| Notification / Calendar / Shortcut | 可用通道与授权状态 | 通知、日历、快捷键动作 | 尚未实现；必须由 OS 权限 Adapter 承担，不向 Provider 暴露系统句柄 |

新增模块不得通过扩大一个“万能 Tool”接入。每项能力必须同时提供最小 Tool Schema、参数风险评估、固定 Gateway Adapter、Capability Policy、脱敏返回值、取消与超时语义、审计事件和失败测试。其它模块调用 AI 时也必须通过 `agent.task.create` 和任务预算；Automation Action、用户代码或 Skill 不能把任意 Prompt、任意 Tool allowlist 或已有批准转交给新任务。

五项写入或外部副作用工具固定映射到 `safe.pet.animate`、`safe.pet.move`、`safe.profile.switch`、`safe.character.switch` 和 `safe.program.execute`，模型无法把参数中的字符串提升为 Gateway 命令。Profile 切换复用桌面原生窗口策略预应用、持久化和失败回滚用例；角色切换只接受内置或已安装且复验通过的 Character Asset ID，持久化选择后热刷新 Pet Renderer，刷新失败回滚原选择；程序执行必须把 `programId + version` 共同绑定到批准，执行瞬间重新加载 active 安装并复验完整性、精确版本与该版本持久授权，再经既有隔离 Worker 和 Capability Gateway 调用程序声明的模块能力。Agent 专属程序目录能力不会下放给用户程序，避免程序递归枚举或启动其它程序。三类原生操作没有受控桌面上下文时都在状态写入或 Worker 启动前失败。Invocation ID 作为幂等键、Task/Trace 作为关联上下文进入共享 Capability Gateway。只读工具允许 Safe 自动执行，其余五项工具必须绑定实际参数批准。

### 3.2 桌面任务历史生命周期

- 桌面宿主只在 Provider 得到最终完成结果后写入 `SqliteAgentHistoryRepository`；等待确认、拒绝、取消和未完成 Turn 不生成伪历史。
- 记录保留 Task、Provider、模型、最初用户 Prompt、最终 Response、Finish Reason、Usage 与完成时间，并使用 Task ID 保证只写一次。
- 历史写入是旁路持久化：失败只设置 `historyDegraded`，不得把已完成任务或已经发生的工具副作用改报为失败；后续成功写入会清除降级标记。
- 桌面 IPC 只提供有界稳定游标分页、单条删除和全部删除。游标的 `createdAtMs` 与 `taskId` 必须同时提供，避免不稳定翻页。
- Recovery Mode 使用独立内存仓储，不读取不可用主库；历史删除不影响角色、Profile、任务状态或工具结果。
- Prompt 与 Response 不自动进入诊断包、事件日志或模型可调用工具，也不暴露给用户代码 Gateway。浏览器 Preview 使用同契约的会话内存实现，仅用于离线 UI 验证。

## 4. 任务生命周期与预算

```text
pending → planning → running → succeeded | failed | cancelled
                   ↘ waiting-for-confirmation ↗
running → paused → running
任何活动状态 → budget-exhausted
```

每个任务必须同时限制：

- 最大计划/Provider 步骤数。
- 最大 Tool 调用数。
- 最大墙钟时间，使用饱和时间差处理系统时钟回退。
- 最大输入和输出 Token。
- 最大费用微单位；本地免费 Provider 仍记录 `0`，不能跳过其它预算。
- 最大并发、上下文字节、单次响应字节和历史保留期由宿主策略补充。

任务元数据不保存 Prompt 正文，只记录稳定 ID、来源、请求方、Provider、状态、预算、用量与时间。Prompt、附件、记忆和 Tool 结果按独立数据分类与生命周期存储。

## 5. Provider Adapter

当前 `nimora.agent-provider/1` Rust 契约已覆盖能力集合发现、本地/网络属性、结构化 Tool Call、取消、Token 用量、费用、上下文窗口、有界请求响应和稳定错误分类。能力使用可扩展集合而非固定布尔字段，新增 Provider 能力不要求破坏描述结构。流式事件协议和 OpenAI-compatible Adapter 尚未实现。首批适配目标：

- OpenAI-compatible HTTPS Provider。
- Ollama 与其它显式配置的本地回环 Provider。
- 测试用确定性 Mock、超时、畸形响应和 Prompt Injection Provider。

Provider 只能看到任务授权的数据视图，不接触 Secure Store。凭据由宿主按 Provider ID 注入请求 Adapter；错误返回稳定类别，不把 Key、完整请求或底层网络细节写入 UI 和诊断包。

运行时当前强制：最多 64 个 Adapter、256 条消息、256 KiB 消息正文、1 MiB 响应正文、32 个 Tool Call 和 10 分钟单次超时；离线模式在调用 Adapter 前拒绝网络 Provider。Provider 返回的未知 Tool、非对象参数、错配 Request ID、超出输出预算或不一致 Finish Reason 全部 fail-closed。续跑对话中的 Assistant Tool Call 与 Tool Result 也执行调用 ID、工具名、先后关系和单次解析校验。

`crates/agent-provider-worker` 已实现真实 Ollama `/api/chat` 非流式 Adapter。HTTP 只能由独立 sidecar 发往 IPv4/IPv6 loopback，禁止远程地址、凭据和重定向；Worker 协议、HTTP Header、Body、stdout、超时与取消均有硬边界。宿主并发读取有限 stdout，防止管道背压死锁，并在取消或超时后强制终止进程。

CLI 已接入 `provider:ollama-loopback`，但只接受经 `nimora.provider-sidecar/1` Manifest 验证的 Worker。调用方必须同时提供 `--sidecar-root` 与由宿主或发行系统信任的 `--sidecar-manifest-sha256`；Manifest 和 Worker 均拒绝符号链接、路径逃逸、大小或 SHA-256 不匹配。构建脚本生成的 `.sha256` 只是供 CI、签名或宿主嵌入信任锚使用的发布素材，不能从同一可写目录读取后自行信任，也不等价于发布者数字签名。桌面自动发现和发布者签名信任根仍待接线。

## 6. CLI

正式 CLI 名称为 `nimora`，AI 子命令必须与桌面使用同一运行时和数据目录锁协议：

```text
nimora ai chat
nimora ai run --input task.json --output json
nimora ai task list|show|cancel|resume
nimora ai provider list|probe
nimora ai tool list|describe
nimora ai history export --database <path> [--limit <1..200>] [--before-created-at-ms <timestamp> --before-task-id <uuid>]
nimora ai history delete --database <path> (--task-id <uuid>|--all)
```

共享 SQLite 层已提供 `nimora.agent-history/1` 完成记录仓储：Task ID 唯一、强类型 Provider/Usage/Finish Reason、单条内容 256 KiB 上限、最多 200 条的稳定时间游标分页，以及按任务或全部删除。桌面任务生命周期、历史 UI 和 CLI `run|export|delete` 均已接入该仓储；CLI 通过显式 `--history-database` 写入，通过显式 `--database` 查询或删除，不猜测、不回显系统数据路径。历史写入失败作为 `history.degraded` 旁路状态呈现，不能在工具副作用已经完成后把整个 Agent 任务伪报为失败。

当前首个可运行 CLI 基线位于 `apps/cli`，已实现 `provider list|probe`、`tool list|describe`、历史 `export|delete` 与非交互 `run`。`tool list|describe` 返回生产 Tool Registry，Provider 请求获得同一目录。内置 `provider:deterministic-local` 是无网络、无凭据、零费用的确定性诊断 Provider，用于证明 CLI、任务状态、预算、Provider Registry 和离线策略的真实端到端路径；它不是通用语言模型。Ollama 已通过受验证 Worker 接入非交互运行，OpenAI-compatible 尚未实现。CLI 当前没有持有桌面模块 Backend，因此 Tool Call 只进入待确认/不可执行结果，不会在桌面进程外伪造模块副作用。

- 交互终端显示计划、实际 Tool 参数、风险、Provider 数据预览和实时预算。
- 非交互模式遇到需确认操作必须退出并返回结构化 `confirmation-required`，禁止默认同意。
- `--yes` 只能覆盖明确允许自动批准的 Safe read-only Tool，不能覆盖写入、外部副作用或 Medium 以上风险。
- `--offline` 禁止网络 Provider，并只选择已验证本地 Provider。
- JSON 输出保持 stdout 机器可读，进度和诊断写 stderr；退出码稳定且有文档。
- `run` 输入文件或 stdin 最大 256 KiB，拒绝未知字段；当前稳定退出码为 `2` 用法错误、`3` 输入错误、`4` 资源不存在、`5` 需要确认、`10` 运行时错误。

## 7. 安全不变量

- System Policy、用户权限、Tool allowlist 和预算不进入模型可修改上下文。
- 外部网页、文件、Connector、Tool 输出和模型文本均标记为不可信数据，不得改变策略层指令。
- Safe Mode 在 2 秒内取消 Agent Worker、撤销工具执行租约并阻止新任务；高风险能力不会自动恢复。
- Tool 结果必须按输出 Schema 和大小预算校验；未知字段、超限、非有限数字和协议错序均拒绝。
- 重试有副作用 Tool 必须具备幂等键或补偿策略；未知执行结果不能自动重放。
- Agent 记忆支持查看、编辑、删除、禁用和按 Profile 隔离；删除后不得继续出现在上下文、导出或索引中。

## 8. 完成标准

完整实现至少证明：桌面与 CLI 任务等价、多 Provider 可替换、本地离线运行、模块双向调用、实际参数风险确认、批准失效、预算终止、Prompt Injection 防护、Safe Mode 强停、历史与记忆删除、Provider 数据预览、故障恢复、跨平台桌面验证和真实 UI 截图。

## 14. 桌面工作台当前纵切

桌面 Control Center 已提供 Agent 一级入口。工作台从宿主读取与 CLI、Provider 请求相同的十项生产 Tool Catalog，明确区分只读能力与必须确认的可逆写能力，并显示本地、无凭据、零费用边界。当前对话路径为 `provider:deterministic-local` 的确定性离线诊断单步，返回真实 Task、Finish Reason 与 Usage；它不伪装成通用对话模型，也不会自行产生 Tool Call。

工作台提供生产 Tool Catalog 的真实执行验证入口：只读工具经 Tool Registry 和共享 Capability Gateway 立即执行；写工具由 Rust 宿主生成参数绑定的 Invocation 与 Approval，并仅在 UI 展示实际 Tool ID、参数、风险和期限。Approval 不交给前端，宿主最多持有 32 个待确认项，5 分钟过期，确认或拒绝时一次性移除后再处理，因此不能换参、重放或在执行失败后隐式重试。进入 Safe Mode 会撤销全部待确认项；Recovery Mode 不允许创建或确认工具调用。

运行时与 Ollama Worker 已能在后续 Provider Step 中完整传递 Assistant Tool Call 和关联 Tool Result，不再把工具结果降格为无关联文本。跨进程测试已覆盖真实独立 Worker 的双轮 `/api/chat`：首轮 Tool Call 转为强类型调用，关联结果进入第二轮请求，最终回答再由 Worker 返回。桌面宿主现已实现 Provider Tool Turn 生命周期：只读调用立即经过共享 Capability Gateway；写调用生成同一 Turn 的参数绑定确认组，全部批准前不执行任何写副作用，全部批准后按 Provider 原始顺序执行并聚合结果，再进入下一 Provider Step；任一拒绝或过期会级联撤销兄弟确认，禁止部分结果被伪装成完整结果。

桌面 IPC 统一返回 `completed` 或 `waitingForConfirmation`，等待态不是错误。工作台会展示同一 Turn 的全部 Tool ID、实际参数、风险和过期时间；部分批准只返回剩余项，最后一项批准后回填 Provider 最终回答，任一拒绝显示整组取消。Approval 仍只存在于 Rust 宿主。浏览器预览使用独立的确定性 Scripted Provider 验证双工具 UI，它不是生产 Provider，也不进入 Tauri 注册表。

生产桌面构建会嵌入 `ollama-provider.json` 的 SHA-256 信任摘要。启动时宿主仅从受控资源候选目录发现 sidecar，并复用 CLI 的 Manifest 路径、Manifest 摘要、普通文件、大小和 Worker 摘要校验；全部通过后才把 `provider:ollama-loopback` 注册到同一 `ProviderRegistry`。工作台只展示 Registry 中真实可用的 Provider，任务显式携带 Provider ID 与模型名，未知 Provider、空模型和越界模型名在调用前拒绝。

桌面健康检查通过同一受验证 Worker 请求 loopback-only `/api/tags`，Tauri Core 与 React 均不直接联网。协议限制 2 秒桌面超时、16 KiB Header、1 MiB Body、256 个模型和 128 bytes 模型名，拒绝 chunked、长度错配、非 200、畸形字段与远程地址；结果去重并稳定排序。UI 分别表达 Worker 完整性、服务可达性和模型可用性，模型目录用于 `datalist` 建议与运行前 fail-closed 校验。Safe/Recovery Mode 禁止启动 Worker 探测。生产 Worker 双轮 Tool Call 已由真实跨进程 mock Ollama 自动化覆盖；使用用户本机实际模型的桌面验收、历史持久化和任务恢复尚未实现。
