# 用户代码能力面 · User Code Capability Guide

> 状态：Creator / AI Creator 产品面已公开能力矩阵；桌面 Runtime 后端提供 `safe.pet.*`（含 `safe.pet.directive`）与只读 Gateway 面；未在 Manifest 声明的能力 **fail-closed**。

本文档面向会编写 User Program 的开发者与 AI Creator 流水线。安全红线见 [`USER_CODE_SECURITY.md`](USER_CODE_SECURITY.md)；可编程模型总览见 [`PROGRAMMABLE_CONTROL.md`](PROGRAMMABLE_CONTROL.md)。

## 1. 目标

用户程序（User Program）可以：

1. **读取**宠物与宿主只读面（state / catalog / health）。
2. **驱动宠物**：`safe.pet.animate`、`safe.pet.care`、`safe.pet.move`、**`safe.pet.directive`**（结构化 `nimora.pet_directive/1`）。
3. 在 Manifest 授予 `invoke-agent-tasks` 时 **提交 Agent 任务**（draft、空 Tool allowlist）。
4. 调用 Manifest `commands` 白名单中的其它 **`safe.*` 命令**（经 Capability Gateway 路由到模块适配器）。

用户程序 **不能** 直接访问原始文件系统、网络、进程、窗口句柄或任意 Tauri API。

## 2. Allowlist 模型

能力授权是 **声明式 + 运行时双重闸门**：

```text
manifest.json
  capabilities[]     → 策略层 ExecutionPolicy 布尔位
  commands[]         → 可 invoke 的 safe.* 命令白名单
  subscriptions[]    → 事件订阅（需 subscribe-events）
        │
        ▼
Capability Gateway
  executionId / traceId / idempotencyKey
  capability denied → fail-closed
  undeclared command → fail-closed
        │
        ▼
CapabilityBackend（桌面 Runtime 适配器）
  只返回 JSON 值，不返回 Core / DB / Renderer 句柄
```

### 2.1 Manifest capabilities（kebab-case）

| Capability ID | 中文 | 作用 |
| --- | --- | --- |
| `read-pet-state` | 读取宠物状态 | 一次性执行时注入只读 `nimora.input.pet` 快照；Gateway `ReadPetState` |
| `read-profile-state` | 读取 Profile | 读取当前 Profile 策略快照 |
| `subscribe-events` | 订阅事件 | 允许 Manifest 声明的有界事件订阅 |
| `invoke-safe-commands` | 调用安全命令 | 允许执行 `commands[]` 白名单中的命令 |
| `store-local-data` | 本地命名空间存储 | 程序私有 KV，**无原始 FS** |
| `invoke-agent-tasks` | 调用 Agent 任务 | 允许 `agentTasks[]`；宿主强制 draft + 空 Tool allowlist |

安装前策略层校验 Manifest；执行前完整性复验（`.nimora-integrity.json`）。能力或命令集合变化要求重新授权。

### 2.2 只读面 · Read surfaces（Gateway / 模块）

下列 **capability names** 描述宿主可读语义面（点分 ID）。Agent / 模块 Gateway 通过 `read_capabilities` 集合授权；User Program 路径以 Manifest `read-pet-state` / `read-profile-state` 为主，动作目录等可经已授权模块命令或输入快照间接使用。

| Surface ID | 中文 | 典型用途 |
| --- | --- | --- |
| `pet.state` | 宠物状态 | 情绪、位置、vital、directive 修订 |
| `pet.action.catalog` | 动作目录 | 合法 action / animation / directive 契约 |
| `character.state` | 角色状态 | 当前角色与资源身份 |
| `program.catalog` | 程序目录 | 已安装 User Program 列表 |
| `runtime.health` | 运行时健康 | 启动、安全模式、备份与 outbox 健康 |

> 未列入策略的只读请求在 Gateway 层返回 `CapabilityDenied`，不静默降级。

### 2.3 安全命令 · Safe commands

| Command | 中文 | 参数要点 | 风险 |
| --- | --- | --- | --- |
| `safe.pet.animate` | 播放动作 | `{ "action": "pet.work" }` 等动作 ID | Safe |
| `safe.pet.care` | 照料互动 | 照料动作枚举 | Low |
| `safe.pet.move` | 移动位置 | 桌面安全区域坐标 | Low |
| `safe.pet.directive` | **结构化指令** | `nimora.pet_directive/1` 对象 | Safe |

其它模块命令只要：

1. 以 `safe.` 前缀注册于 Runtime Backend；
2. 出现在 **当前程序版本** Manifest `commands[]`；
3. 具备 `invoke-safe-commands`；

即可经 Gateway 调用。未注册后端即使名称形如 `safe.*` 也会被 Backend 拒绝。

## 3. 结构化宠物指令 · `nimora.pet_directive/1`

`safe.pet.directive` 是驱动 Subject（桌宠）的推荐入口：一次表达 speech、情绪增量、动作、动画与注意力源。

### 3.1 载荷字段

| Field | Type | Required | 说明 |
| --- | --- | --- | --- |
| `spec` | string | 是 | 必须为 `nimora.pet_directive/1` |
| `speech` | string \| null | 否 | 气泡文案；长度有宿主上限（当前约 120 字符） |
| `moodDelta` | `{ "mood": number }` \| null | 否 | 情绪增量；绝对值有宿主上限 |
| `action` | string | 是 | 指令动作（如 `celebrate`、`observe`、`work`） |
| `animation` | string \| null | 否 | 动画 token（如 `pet.celebrate`）；需在白名单内 |
| `attention` | string | 是 | 注意力焦点（如 `user`、`idle_scene`） |

参数可写在 `arguments` 根上，或包在 `arguments.directive` 内；宿主会规范化 `spec` 并 `validate()`。

### 3.2 示例 Program Plan（Worker 返回）

Worker 不直接调用 Tauri：它返回结构化 **capability plan**，桌面逐条经 Gateway 执行。任意一步失败即停止后续命令。

```json
{
  "storage": [],
  "commands": [
    {
      "command": "safe.pet.directive",
      "arguments": {
        "spec": "nimora.pet_directive/1",
        "speech": "专注完成啦，休息一下吧！",
        "action": "celebrate",
        "animation": "pet.celebrate",
        "attention": "user",
        "moodDelta": { "mood": 6 }
      },
      "idempotencyKey": "focus-done-celebrate-1"
    }
  ],
  "agentTasks": []
}
```

对应最小 Manifest 片段：

```json
{
  "id": "studio.example.focus-celebrate",
  "version": "1.0.0",
  "capabilities": ["read-pet-state", "invoke-safe-commands"],
  "subscriptions": [],
  "eventConcurrency": "serial",
  "eventQueueCapacity": 16,
  "commands": ["safe.pet.directive"],
  "timeoutMs": 5000,
  "memoryBytes": 8388608
}
```

## 4. 模块调用路径 · Gateway

```text
User Code (Boa Worker)
  → JSON plan: storage[] · commands[] · agentTasks[]
    → Desktop host 解析 UserProgramPlan
      → Capability Gateway.dispatch(policy, execution, envelope)
        → Manifest Policy + Command Allowlist
          → CapabilityBackend.invoke_command / read_*
            → Runtime Core / Profile / Catalog / Agent Adapter
```

要点：

- 每次调用携带 `executionId`、`traceId`、可选 `idempotencyKey`。
- Gateway 只转发 JSON，**不注入** Core 对象或 Tauri 句柄。
- 模块之间互调同样走 Gateway；禁止第二条旁路通道。
- 取消 / 超时 / 输出超限由 Supervisor 终止 Worker，fail-closed。

### 4.1 Agent 任务

仅当 Manifest 含 `invoke-agent-tasks` 且该精确版本获用户授权：

- `agentTasks[]` 项字段：`providerId`、`model`、静态 `instruction`、可选 `context[]`。
- 宿主强制：`Module` Origin、`program:<id>` requester、`draft` 主动性、**空 Tool allowlist**、收紧预算。
- 任务只产出结果文本；副作用仍须程序自己的 `commands[]` 完成。
- 外部正文只能进入 `context[]`，不得拼进 `instruction`。

### 4.2 其它 safe 命令

程序可声明例如 `safe.profile.switch`、`safe.character.switch` 等（以宿主已注册后端为准）。Creator UI 能力矩阵优先展示宠物驱动命令；完整注册表以运行时 Backend 与动作目录为准。

## 5. 沙箱边界

| 边界 | 行为 |
| --- | --- |
| 执行隔离 | 独立进程 Worker（Boa ECMAScript），无 Node `process` / `require` |
| 文件系统 | **无原始 FS**；仅 `store-local-data` 命名空间 KV |
| 网络 | 默认无；Agent Provider 由宿主代发且受 Provider 策略约束 |
| 能力 | 未声明 / 未授权 → `CapabilityDenied` |
| 命令 | 未在 Manifest `commands[]` → `CommandNotDeclared` |
| 预算 | 源码 ≤ 256 KiB；单次 plan 操作数有上限（存储 + 命令 + Agent 合计） |
| 安全模式 | 取消并移除全部活动会话 |

详见 [`USER_CODE_SECURITY.md`](USER_CODE_SECURITY.md)。

## 6. Creator / AI Creator 产品面

桌面 **Creator Studio** 与 **AI Creator** 展示紧凑中文 **Capability Matrix**（chips，非长文）：

- 只读面：`pet.state` · `pet.action.catalog` · `character.state` · `program.catalog` · `runtime.health`
- 安全命令：`safe.pet.animate` · `safe.pet.care` · `safe.pet.move` · **`safe.pet.directive`**
- Agent：`invoke-agent-tasks`
- Manifest 能力（完整矩阵）：`read-pet-state` 等

前端纯函数（便于单测）：

- `userCodeCapabilityChips()` / `labelForUserCodeCapability(id)`
- `samplePetDirectiveProgramPlan()` / `formatSampleProgramPlanJson()`
- 组件 `UserCodeCapabilityMatrix`

AI 生成 User Program 草案时 **不得** 发明未注册命令；能力不足应走 Capability Gap 路径（见 AI Creator 缺口预览）。

## 7. 安全注意事项（MUST）

1. **Fail-closed**：能力缺失、命令未声明、校验失败、安全模式开启时拒绝执行，不部分成功吞错。
2. **无原始 FS / 网络 / 进程**：不得把路径、socket 或子进程句柄交给用户代码。
3. **最小授权**：只声明实际需要的 capabilities 与 commands；升级扩大权限需重新授权。
4. **幂等**：可变更命令应带 `idempotencyKey`，避免事件重放副作用。
5. **审计**：Gateway 与 Agent 上下文准入拒绝只记原因类别与计数，不落敏感正文。
6. **与 Asset Pack 区分**：角色资源包不执行代码；User Program 才走本能力面。

## 8. 相关文档

- [`USER_CODE_SECURITY.md`](USER_CODE_SECURITY.md) — 策略、Worker、安装与完整性
- [`PROGRAMMABLE_CONTROL.md`](PROGRAMMABLE_CONTROL.md) — 编程模型与生命周期
- [`DESKTOP_LIFEFORM_CONTEXT.md`](DESKTOP_LIFEFORM_CONTEXT.md) — 结构化指令与生命体情境
- [`SECURITY_PRIVACY.md`](SECURITY_PRIVACY.md) — 产品级安全隐私基线
