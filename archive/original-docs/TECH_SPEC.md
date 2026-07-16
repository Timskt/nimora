# DeskPet Platform — 技术实现总纲（原始需求稿）

> **文档状态：已被新版标准文档体系取代。** 新开发请从 `docs/INDEX.md` 开始；本文件保留用于追溯原始技术需求。

> 本文件是**单一事实来源（Single Source of Truth）**，用于让 AI 或工程师在没有额外澄清的情况下，
> 理解项目目标、架构、功能需求、设计哲学、完成标准与任务拆分，并据此逐任务落地代码。
>
> 产品代号：**DeskPet Platform**
> 一句话定位：**本地常驻的桌面生命体运行时 + 可插拔技能平台 + 本地开放网关 + 可审计数据出站平面 + 可调用工具的 AI Agent**。

---

## 0. 设计哲学（为什么这样设计，不是过家家）

1. **平台优于玩具**：宠物本体只是外壳；真正护城河是**事件总线 + 技能 + 出入站连接器 + Agent**。
2. **一切皆注册**：命令、菜单、动作、触发器、工具、事件订阅都先注册到 Registry，UI 只负责呈现贡献点。
3. **官方功能即技能**：官方实现的番茄钟、提醒等必须以 Skill 形式存在，逼迫公开 API 稳定。
4. **默认安全**：开放能力默认关闭；出站数据默认仅本机；高危动作默认需确认；所有外部访问可审计可撤销。
5. **统一信封**：所有对外事件采用稳定 `deskpet.event/1` 信封，协议（HTTP/WS/SSE/UDP）只是 transport。
6. **崩溃隔离**：Skill Host / Agent 与主进程隔离；单技能挂不掉主程序。
7. **可观测**：日志、配额、成功率、审计是产品必备，不是附加。
8. **渐进但不欠债**：可分期交付，但架构边界从第一天按最终形态预留。

---

## 1. 项目目标（做到什么程度算完成）

### 1.1 必须交付的可运行能力（MVP 完成定义）

- 一个**可安装可运行的桌面客户端**（先以 Web + Node 运行时演示核心，最终壳用 Tauri 2 + Rust Core），启动后桌面上出现可互动宠物。
- 内部存在 **Event Bus**，宠物状态变化会产生标准事件。
- **数据出站（Data Plane）全部实现**：HTTP Webhook、WebSocket Client、SSE、UDP 四种出站 Connector 均可用，支持重试、限流、过滤、模板、审计。
- **Local Gateway 实现**：REST + WebSocket + SSE 入站，支持 Token 鉴权与 Scope。
- **第三方 AI 配置实现**：用户可配置多家云端 LLM（OpenAI 兼容 / Anthropic / 自定义 base_url），Agent 能调用平台工具（命令/技能/自动化/连接器）。
- **CLI 工具**：`deskpet` 提供 `status / say / api / pack / sink` 等子命令，验证开放能力。
- **一份可运行 Demo**：宠物状态变化 → 同时被本地 SSE 客户端、Webhook mock、UDP 监听收到。

### 1.2 完成验收（Definition of Done）

- [ ] `npm run dev` 启动后宠物可见、可点击互动。
- [ ] 触发一次 `pet.click` 或 `pet.stats.changed`，在以下四端同时验证收到标准信封：
  - 入站 SSE 客户端（`deskpet api sse` 或浏览器 EventSource）
  - 入站 WS 客户端
  - 出站 Webhook（指向 `deskpet sink http` mock）
  - 出站 UDP（监听 `127.0.0.1:39789`）
- [ ] 配置第三方 AI Key 后，能在命令面板用自然语言让 Agent 执行一个已注册工具（如“开始番茄钟”）。
- [ ] 安全模式一键断开：停 Gateway + 停全部出站 Connector。
- [ ] 所有出站目标、Token 使用在审计日志可见。

---

## 2. 总体架构

```text
┌──────────────────────── UI Layer ────────────────────────┐
│ Pet Window(透明) │ Control Center │ Cmd Palette │ Tray    │
└─────────────┬───────────────────────┬────────────────────┘
              │ IPC                   │
┌─────────────▼───────────────────────▼────────────────────┐
│                     DeskPet Core (Runtime)                │
│  Window/Platform │ PetFSM │ Config │ Profile │ SecureStore│
│                         Event Bus (内部) ◄───────────────┐│
└───────┬─────────────────┬───────────────────┬───────────│┘
        │                 │                   │
┌───────▼──────┐  ┌───────▼───────┐   ┌───────▼───────────┐
│ Skill Host   │  │ Automation    │   │ Local Gateway     │
│ (Node/TS)    │  │ Engine        │   │ HTTP/WS/SSE(in)   │
│ permissions  │  │ cron/events   │   │ Auth+Scope        │
└───────┬──────┘  └───────┬───────┘   └───────┬───────────┘
        │                 │                   │
        └────────────┬────┴───────────────────┘
                     ▼
              Connector Runtime（出站/入站适配器）
        HTTP Webhook │ WS Client │ SSE │ UDP │ (MQTT P2)
                     ▼
              外部系统 / 第三方 App / 灯带 / OBS / 自建后端
                     ▲
              Agent Runtime（LLM Provider + Tool Router）
```

### 2.1 仓库结构（最终态，本期先实现 Node 可运行核心）

```text
deskpet/
  apps/
    desktop/                 # Tauri shell + UI（后期）
    web-demo/                # 本期演示用 Web 客户端（PixiJS 宠物）
  packages/
    core/                    # EventBus + Config + PetFSM + Profile
    gateway/                 # 入站 REST/WS/SSE
    connectors/              # 出站 HTTP/WS/SSE/UDP
    automation/              # 规则引擎
    skill-api/               # @deskpet/skill-api
    client-js/               # 第三方对接 SDK
    schemas/                 # JSON Schema: event/package/rule/ai
    cli/                     # deskpet-cli
    agent/                   # Agent Runtime（tool calling）
  skills/official/           # 官方技能（狗粮）
  docs/                      # 本文档及子文档
  tests/
```

---

## 3. 数据模型与契约（必须先冻结）

### 3.1 事件信封 `deskpet.event/1`

`packages/schemas/event/v1.json`：

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "DeskPetEvent",
  "type": "object",
  "required": ["spec","id","ts","source","type"],
  "properties": {
    "spec":   { "const": "deskpet.event/1" },
    "id":     { "type": "string" },
    "ts":     { "type": "string", "format": "date-time" },
    "source": { "type": "string", "pattern": "^(core|skill:.+|agent|automation:.+)$" },
    "type":   { "type": "string" },
    "petId":  { "type": "string" },
    "profileId": { "type": "string" },
    "severity": { "enum": ["debug","user","system"] },
    "data":   { "type": "object" },
    "traceId": { "type": "string" }
  }
}
```

### 3.2 事件类型清单（v1）

| type | 触发方 | data 示例 |
|------|--------|-----------|
| `pet.click` | core | `{x,y,button}` |
| `pet.drag` | core | `{from,to}` |
| `pet.anim.changed` | core | `{name}` |
| `pet.stats.changed` | core | `{mood,hunger,energy,affinity}` |
| `pet.mode.changed` | core | `{topmost,clickThrough,visible}` |
| `pet.moved` | core | `{x,y,scale}` |
| `profile.changed` | core | `{id}` |
| `automation.fired` | automation | `{id,name}` |
| `skill.installed` | skill-host | `{id,version}` |
| `agent.task.finished` | agent | `{goal,steps}` |
| `connector.emitted` | connectors | `{connectorId,transport}` |
| `gateway.access` | gateway | `{method,path,scope,ok}` |

### 3.3 Connector 配置 schema

```yaml
# packages/schemas/connector/v1.yaml（概念，实现用 JSON Schema/Zod）
id: string              # 唯一
type: enum[http_webhook, websocket_client, sse_client, udp]
enabled: boolean
events: string[]        # 订阅的事件 type
endpoint: string        # URL / host:port
method: enum[POST,PUT]  # http only
headers: object         # 支持 ${SECRET:name} 占位
format: enum[json_envelope_v1, compact_json, binary_v1]
filter:
  profile_in: string[]
  expr: string          # 简单表达式，如 "data.mood > 80"
template: string        # 将 envelope 映射为对方协议
retry: { max: int, backoff: enum[exp,linear], baseMs: int }
rateLimit: { perSec: int }
maxPacketBytes: int     # udp
```

### 3.4 AI Provider 配置 schema

```yaml
ai:
  enabled: boolean
  defaultProvider: string
  autoExecuteTools: boolean      # 默认 false（需确认）
  planConfirmRequired: boolean   # 默认 true
  providers:
    - id: openai
      kind: openai_compatible
      baseUrl: https://api.openai.com/v1
      apiKey: ${SECRET:openai_key}
      model: gpt-4o-mini
      temperature: 0.7
      maxSteps: 8
    - id: anthropic
      kind: anthropic
      baseUrl: https://api.anthropic.com
      apiKey: ${SECRET:anthropic_key}
      model: claude-3-5-sonnet
    - id: local
      kind: openai_compatible
      baseUrl: http://127.0.0.1:11434/v1   # Ollama
      apiKey: none
      model: llama3.1
  memory:
    enabled: boolean
    maxItems: int
  proactive:
    enabled: boolean
    maxPerHour: int
```

---

## 4. 功能需求清单（数据出站 + 第三方 AI，全部实现）

### 4.1 出站 Connector（P0 全做）

| ID | 功能 | 验收 |
|----|------|------|
| OUT-01 | `http_webhook` 出站 POST/PUT | 事件到达后按 envelope 推到 endpoint；支持 headers 与 `${SECRET}` |
| OUT-02 | `websocket_client` 出站 | 长连对端，自动重连（指数退避），事件序列化发送 |
| OUT-03 | `sse_client` 出站（订阅上游再转发） | 能连上游 SSE，收到后作为新事件入总线 |
| OUT-04 | `udp` 出站 | 向 `host:port` 发送 compact_json 或 binary 帧；默认仅 127.0.0.1 |
| OUT-05 | 事件过滤 | 按 `events` 白名单 + `profile_in` + `expr` 过滤 |
| OUT-06 | 载荷模板 | 将 envelope 映射为对方所需结构 |
| OUT-07 | 重试与限流 | 失败指数退避；超 `rateLimit` 丢弃/排队 |
| OUT-08 | 审计日志 | 记录每次发送：connectorId/transport/target/事件/success/错误 |
| OUT-09 | 连接器管理 | 设置页增删启停、测试发送、查看最近投递 |
| OUT-10 | 一键断网 | 安全模式停止全部出站 Connector |

### 4.2 入站 Gateway（P0 全做）

| ID | 功能 | 验收 |
|----|------|------|
| IN-01 | REST `/v1/pet/*`、`/v1/commands/*`、`/v1/automations/*` | Bearer Token + Scope 鉴权 |
| IN-02 | WS `/v1/events` 入站订阅 | 客户端收到实时 envelope |
| IN-03 | SSE `/v1/events/sse` 入站 | 浏览器/脚本用 EventSource 收流 |
| IN-04 | Token 管理 | 生成、撤销、Scope 限定、过期 |
| IN-05 | 配对授权 UX | 第三方首次连接弹宠物确认 |
| IN-06 | 安全默认 | 默认 127.0.0.1；绑 0.0.0.0 需显式危险开关 |

### 4.3 第三方 AI 配置（P0 全做）

| ID | 功能 | 验收 |
|----|------|------|
| AI-01 | 多 Provider 配置 | 支持 OpenAI 兼容 / Anthropic / 自定义 base_url / Ollama |
| AI-02 | 密钥安全存储 | Key 存 SecureStore，配置仅 `${SECRET:name}` |
| AI-03 | Tool Router | Agent 可调 commands / skills / automation / connector.test |
| AI-04 | 确认执行 | 默认 planConfirm=true；高危动作必确认 |
| AI-05 | 降级模式 | 无 Key 时规则意图匹配到命令，核心可用 |
| AI-06 | 记忆 | 本地记忆面板可查/删/导出 |
| AI-07 | 主动建议 | 接自动化触发，频率受限、可冷静 |
| AI-08 | Agent Pack | 人设包（persona/system_prompt/tools allowlist） |

---

## 5. 任务拆分（可执行、可验收、按顺序）

> 每个任务给出：目标、产出文件、完成标准。建议按顺序提交，每步可独立运行验证。

### T0 工程脚手架
- 目标：建立 monorepo 与可运行 demo 入口。
- 产出：`package.json`(workspaces)、`packages/*`、`apps/web-demo`、`tsconfig`、README。
- 完成：根目录 `npm i && npm run dev` 能启动一个页面，控制台打印 "DeskPet core booted"。

### T1 事件总线 + Schema
- 目标：实现 EventBus 与信封校验。
- 产出：`packages/core/src/eventBus.ts`、`packages/schemas/event/v1.json`、Zod 校验。
- 完成：发布一个 `pet.click` 事件，订阅者收到合规 envelope；非法 envelope 被拒。

### T2 Pet FSM 与演示宠物
- 目标：状态机 + PixiJS 渲染可点击宠物，发出事件。
- 产出：`packages/core/src/pet.ts`、`apps/web-demo`。
- 完成：点击宠物产生 `pet.click` 与 `pet.stats.changed`。

### T3 出站 Connector 框架
- 目标：统一 Connector 接口 + 注册 + 过滤 + 审计。
- 产出：`packages/connectors/src/index.ts`、`registry.ts`、`audit.ts`。
- 完成：加载 connector 配置，事件按过滤进入对应发送器；审计写入日志。

### T4 HTTP Webhook 出站
- 产出：`packages/connectors/src/http.ts`。
- 完成：`deskpet sink http` 收到 envelope；失败重试生效。

### T5 WebSocket Client 出站
- 产出：`packages/connectors/src/ws.ts`（含重连）。
- 完成：连 mock WS server 收到事件。

### T6 UDP 出站
- 产出：`packages/connectors/src/udp.ts`（compact_json + binary 帧）。
- 完成：`deskpet sink udp` 监听 39789 收到帧；非 127.0.0.1 被拦（除非危险开关）。

### T7 SSE Client 出站（订阅上游）
- 产出：`packages/connectors/src/sse.ts`。
- 完成：订阅外部 SSE 后转成新事件入总线。

### T8 Local Gateway 入站
- 产出：`packages/gateway/src/{rest,ws,sse}.ts`、Token/Scope（`packages/core/src/security.ts`）。
- 完成：Bearer 调用 `/v1/pet/say` 宠物说话；WS/SSE 收到事件；越权 403。

### T9 CLI
- 产出：`packages/cli/src/index.ts`：`deskpet status|say|api|pack|sink`。
- 完成：命令行可查状态、让宠物说话、起 mock sink。

### T10 第三方 AI 配置 + Agent
- 产出：`packages/agent/src/*`、`packages/schemas/ai/v1.json`、UI 配置页。
- 完成：配 OpenAI Key 后，命令面板“开始番茄钟”→ Agent 调用已注册命令并演出。

### T11 控制中心 UI（出站 + AI 设置）
- 产出：`apps/web-demo/control-center`。
- 完成：可增删连接器、看审计、配 AI、一键安全模式。

### T12 文档与 e2e
- 产出：`docs/`、`tests/e2e`。
- 完成：README 跑通全链路；e2e 验证四端同时收到事件。

---

## 6. 关键接口草案（给实现者直接照写）

### 6.1 EventBus
```ts
type Handler = (e: DeskPetEvent) => void | Promise<void>
interface EventBus {
  publish(e: unknown): Promise<void>          // 校验 envelope 后广播
  subscribe(type: string | "*", h: Handler): () => void
  history(limit?: number): DeskPetEvent[]
}
```

### 6.2 Connector 接口
```ts
interface Connector {
  id: string
  type: ConnectorType
  init(cfg: ConnectorConfig): Promise<void>
  onEvent(e: DeskPetEvent): Promise<void>
  test(): Promise<void>
  dispose(): Promise<void>
}
```

### 6.3 Gateway REST 示例
```
POST /v1/pet/say    {text, duration?}      scope: pet.interact
POST /v1/commands/execute {id, args?}      scope: commands.execute
GET  /v1/pet        scope: pet.read
WS   /v1/events     scope: events.subscribe
GET  /v1/events/sse scope: events.subscribe
```

### 6.4 Agent Tool 形式
```ts
interface AgentTool {
  id: string
  description: string
  inputSchema: JSONSchema
  run(args: any): Promise<{ ok: boolean; result?: any }>
}
// 工具自动来自：commands + skills.contributes.tools + automation + connector.test
```

---

## 7. 安全红线（实现必须遵守）

1. 出站默认仅 `127.0.0.1`；绑 `0.0.0.0`/局域网 = 危险开关 + 红字警告。
2. 出站目标允许列表（域名/IP/CIDR/端口）。
3. 密钥进 SecureStore；配置只引 `${SECRET:name}`。
4. 速率限制 + 熔断，防死循环打爆对端。
5. 载荷脱敏：默认不发坐标明细/对话原文，逐项开启。
6. 完整审计：谁/何时/发到哪/事件/成败。
7. 一键安全模式：停 Gateway + 停全部出站 Connector。
8. Skill 申请 `net.*` 必须声明授权。
9. Agent 不超出用户权限；高危工具强制确认。
10. 不做：默认全局键鼠 hook、裸 eval、无签名进官方源、UDP 传 Token。

---

## 8. 完成度自检表（交付前逐条打勾）

- [ ] T0–T12 全部完成且可运行
- [ ] 四大出站协议实测通过
- [ ] 入站 REST/WS/SSE 实测通过
- [ ] 第三方 AI 配 Key 后 Agent 调工具成功
- [ ] 审计日志完整
- [ ] 安全模式有效
- [ ] README 与 e2e 通过
- [ ] 非法 envelope / 越权请求被正确拒绝

---

## 9. 给 AI 实现者的最后说明

- 不要为了“先跑起来”写死 fetch；所有出站必须走 Connector 框架。
- 不要为不同协议复制事件模型；只复制 envelope 的序列化。
- 每一行外部网络调用都要能出现在审计里。
- 优先保证“无 AI Key 也能用核心”，再叠 AI 能力。
- 配置格式用户可见 YAML，运行时内部 JSON Schema 校验。
- 遇到设计冲突，回到第 0 节设计哲学裁决。

> 本文件即项目宪法。任何实现偏离须在此文档显式记录变更理由。
