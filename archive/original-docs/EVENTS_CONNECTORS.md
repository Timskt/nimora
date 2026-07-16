# DeskPet — 事件总线与连接器契约（原始需求稿）

> **文档状态：待迁移契约。** 新架构对 Source、Sink、Duplex 和 Gateway 做了明确区分，见 `docs/ARCHITECTURE.md`。

> 配套 `TECH_SPEC.md`。本文件定义事件信封、事件类型、连接器配置与四种出站传输的精确行为。
> 实现者须严格遵循，禁止私自扩展字段导致生态分裂。

---

## 1. 事件信封（冻结 `deskpet.event/1`）

所有进出系统的事件都必须符合以下结构。任何不符合 `spec === "deskpet.event/1"` 的事件在 `EventBus.publish` 中被拒绝并记录 `schema.invalid`。

```jsonc
{
  "spec": "deskpet.event/1",        // 固定
  "id": "evt_01JABC...",            // ULID 或 uuid
  "ts": "2026-07-16T08:00:00.000Z", // ISO8601 UTC
  "source": "core",                 // core | skill:<id> | agent | automation:<id>
  "type": "pet.click",              // 见 §2
  "petId": "main",                  // 多宠预留
  "profileId": "work",              // 当前场景
  "severity": "user",               // debug | user | system
  "data": { "x": 10, "y": 20 },     // 类型相关
  "traceId": "tr_..."               // 跨系统追踪（可选）
}
```

### 校验规则
- `id` 唯一；重复 id 视为同一事件去重（至少日志告警）。
- `ts` 必须可解析；偏差超过 ±5min 警告（时钟问题）。
- `source` 必须匹配正则 `^(core|skill:.+|agent|automation:.+)$`。
- `data` 任意对象，但敏感字段（坐标、对话原文）受 §5 脱敏策略约束。

---

## 2. 事件类型注册表（v1）

新增类型必须在此登记。未登记类型仍可传输，但会在调试日志标注 `type.unregistered`。

| type | source | data schema | 说明 |
|------|--------|-------------|------|
| `pet.click` | core | `{x:int,y:int,button:"left"|"right"}` | 点击宠物 |
| `pet.drag` | core | `{from:{x,y},to:{x,y}}` | 拖拽结束 |
| `pet.anim.changed` | core | `{name:string}` | 动画切换 |
| `pet.stats.changed` | core | `{mood?,hunger?,energy?,affinity?:int 0-100}` | 数值变化 |
| `pet.mode.changed` | core | `{topmost:bool,clickThrough:bool,visible:bool}` | 模式变化 |
| `pet.moved` | core | `{x:int,y:int,scale:float}` | 位置缩放 |
| `profile.changed` | core | `{id:string}` | 场景切换 |
| `automation.fired` | automation | `{id:string,name:string}` | 规则触发 |
| `skill.installed` | skill-host | `{id:string,version:string}` | 技能安装 |
| `skill.enabled` | skill-host | `{id:string,enabled:bool}` | 技能启用 |
| `agent.task.started` | agent | `{goal:string}` | Agent 任务开始 |
| `agent.task.finished` | agent | `{goal:string,steps:int,ok:bool}` | Agent 任务结束 |
| `connector.emitted` | connectors | `{connectorId:string,transport:string}` | 出站成功 |
| `gateway.access` | gateway | `{method:string,path:string,scope:string,ok:bool,status:int}` | 入站访问 |

---

## 3. Connector 配置（冻结）

```yaml
id: push-to-backend          # 全局唯一
type: http_webhook           # http_webhook | websocket_client | sse_client | udp
enabled: true
events:                      # 订阅的事件 type 白名单；空=全部
  - pet.stats.changed
  - automation.fired
endpoint: https://api.example.com/hook   # http/websocket/sse 用 URL；udp 用 host:port
method: POST                 # http only
headers:
  Authorization: "Bearer ${SECRET:my_token}"
format: json_envelope_v1     # json_envelope_v1 | compact_json | binary_v1
filter:
  profile_in: ["work"]       # 仅这些 profile 时投递
  expr: "data.mood > 80"     # 简单表达式；空=不过滤
template: |                  # 可选：将 envelope 映射为对方结构
  {"event":"${type}","v":${data.mood}}
retry:
  max: 5
  backoff: exp               # exp | linear
  baseMs: 500
rateLimit:
  perSec: 20
maxPacketBytes: 1200         # udp only
bind: 127.0.0.1              # udp/http 绑定/目标限制；非本机需 danger:true
danger: false                # 允许 0.0.0.0/局域网时显式 true
```

### 过滤执行顺序
1. `events` 白名单（未命中直接丢弃）
2. `filter.profile_in`（当前 profile 不在列表丢弃）
3. `filter.expr`（用安全表达式求值，禁用函数调用）
4. `rateLimit`（超频丢弃或进队列，默认丢弃并记 `rate.limited`）

### 模板占位符
- `${type}` `${source}` `${petId}` `${profileId}` `${ts}`
- `${data.<path>}` 支持嵌套，如 `${data.mood}`
- 表达式求值器必须是纯数据读取，禁止 `eval`/`Function`。

---

## 4. 四种出站传输精确行为

### 4.1 http_webhook
- 方法 POST/PUT；Body 按 `format` 序列化。
- `json_envelope_v1`：原样 envelope。
- `compact_json`：`{t:type,s:source,d:data,ts}`。
- `template` 存在时以其结果为 Body（必须合法 JSON 或文本）。
- 失败按 `retry` 退避；网络错误/4xx（除 429）/5xx 处理不同：429 尊重 Retry-After。
- 超时 10s；超时报失败重试。

### 4.2 websocket_client
- 建立长连；握手失败按 `retry` 退避重连。
- 连接成功后每个匹配事件 send 序列化文本。
- 断线指数退避重连（最大 30s）；发送前检查 readyState。
- 不支持 binary 帧 v1（后续扩展）。

### 4.3 sse_client（入站订阅上游）
- 作为客户端连上游 SSE（`text/event-stream`）。
- 收到 `data:` 行解析；若含 `spec` 则直接作为 envelope 入总线；否则包装为 `source:connector:<id>` 的 `external.event` 事件。
- 断线重连同 WS。

### 4.4 udp
- `host:port` 来自 `endpoint`（格式 `host:port`）。
- `compact_json`：单行 JSON，≤ `maxPacketBytes`。
- `binary_v1` 帧：`[magic 4B 'DPET'][version 1B][type_id 2B][len 2B][payload]`。
- 默认 `bind/host` 必须为 127.0.0.1 或 `danger:true` 才允许其它。
- 发送失败（如端口不可达）仅日志，不重试（UDP 语义）。

---

## 5. 脱敏策略（默认）

| 字段 | 默认出站 | 开启后 |
|------|----------|--------|
| `data.x/y`（坐标） | 剔除 | 保留 |
| `data` 含对话文本 | 剔除 | 保留（仅显式 scope `data.raw`） |
| `source:agent` 的 prompt | 永不默认 | 需 `data.raw` |
| Token/Key | 永不 | 永不 |

Connector 配置可声明 `expose: [coords, raw]`，但需在审计与 UI 明确标示「高敏」。

---

## 6. 审计日志格式

每条出站尝试写入：
```json
{ "t":"audit.connector", "ts":ISO, "connectorId":str, "transport":str,
  "target":str, "eventType":str, "ok":bool, "err?":str, "bytes?":int }
```
入站访问写 `audit.gateway`（见 TECH_SPEC §6.3）。
审计默认落本地文件 `logs/audit.jsonl`，可在控制中心查看最近 200 条。

---

## 7. 实现检查清单

- [ ] EventBus 拒绝非法 envelope 并记 `schema.invalid`
- [ ] 四种 Connector 均按本契约行为实现
- [ ] 过滤顺序与模板占位符正确
- [ ] UDP 默认禁非本机
- [ ] 审计覆盖每一次出站与入站
- [ ] 脱敏默认生效
