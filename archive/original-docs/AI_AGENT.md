# DeskPet — 第三方 AI 配置与 Agent 规格（原始需求稿）

> **文档状态：需求来源。** 当前 Agent 产品边界见 `docs/PRODUCT_SPEC.md`，安全要求见 `docs/SECURITY_PRIVACY.md`。

> 配套 `TECH_SPEC.md`。本文件定义如何让用户配置任意云端/本地 LLM，并使 Agent 成为
> “会调用平台工具的宠物大脑”。Agent 不是独立聊天窗，而是编排层，复用命令/技能/自动化/连接器。

---

## 1. 设计原则

1. **BYOK 优先**：不绑定任何厂商；用户自备 Key，存 SecureStore。
2. **OpenAI 兼容为主干**：绝大多数云端/本地模型走 `openai_compatible`（base_url + key + model）。
3. **Tool-calling 为核心能力**：Agent 的价值 = 自然语言串联已有平台能力。
4. **默认半自动**：提出计划 → 用户确认 → 执行；可信工具可设免确认。
5. **无 Key 降级**：规则意图匹配到命令，保证核心可用。
6. **权限不越级**：Agent 可用工具 = 用户已授权范围；高危必确认。

---

## 2. Provider 配置

### 2.1 支持的 kind
| kind | 说明 |
|------|------|
| `openai_compatible` | OpenAI 及兼容端点（DeepSeek/月之暗面/Mistral/Ollama 等） |
| `anthropic` | Claude 系列 |
| `google` | Gemini（后期，先走兼容网关亦可） |
| `local` | 同 openai_compatible 但 base_url 指向本机 |

### 2.2 配置结构（用户可见 YAML，运行时 JSON Schema 校验）
```yaml
ai:
  enabled: true
  defaultProvider: openai
  autoExecuteTools: false
  planConfirmRequired: true
  providers:
    - id: openai
      kind: openai_compatible
      baseUrl: https://api.openai.com/v1
      apiKey: ${SECRET:openai_key}
      model: gpt-4o-mini
      temperature: 0.7
      maxSteps: 8
    - id: deepseek
      kind: openai_compatible
      baseUrl: https://api.deepseek.com/v1
      apiKey: ${SECRET:deepseek_key}
      model: deepseek-chat
    - id: ollama
      kind: openai_compatible
      baseUrl: http://127.0.0.1:11434/v1
      apiKey: none
      model: llama3.1
  memory:
    enabled: true
    maxItems: 200
  proactive:
    enabled: true
    maxPerHour: 3
```

### 2.3 密钥安全
- `apiKey` 仅存 `${SECRET:<name>}` 引用。
- 真实值由 SecureStore 提供（演示用文件加密或 env 注入）。
- 配置导出时自动剥离 Secrets，仅留引用。

---

## 3. Agent 架构

```
用户输入(命令面板/对话气泡/自动化/外部API)
        │
        ▼
  Agent Router
   - 选择 provider
   - 组装 system（来自 Agent Pack persona）
   - 注入可用 Tools（commands+skills+automation+connector.test）
        │
        ▼
  LLM (tool_calls)
        │
        ▼
  Tool Executor（受权限/确认约束）
        │
        ▼
  执行结果回灌模型 → 循环至 maxSteps 或 finished
        │
        ▼
  宠物演出反馈（say/anim）+ 审计
```

### 3.1 Tool 注册来源（自动聚合）
```ts
interface AgentTool {
  id: string
  description: string
  inputSchema: JSONSchema
  risk: "safe" | "sensitive" | "danger"
  run(args: any): Promise<{ ok: boolean; result?: any }>
}
```
- `commands.*` → 自动成为 safe 工具（除非命令本身敏感）
- `skills.contributes.tools` → 按技能权限定 risk
- `automation.run` → sensitive
- `connector.test` / `connector.emit` → sensitive
- `pet.*` 控制 → safe
- 打开应用/安装技能/改系统设置 → danger（必确认）

### 3.2 确认流
```
agent.task.started
  → LLM 返回 plan = [tool_a, tool_b]
  → 若 planConfirmRequired：UI 弹出“将执行：①… ②…” [执行][修改][取消]
  → 每个 danger 工具单独确认
  → 执行 → agent.task.finished（带 steps 与结果摘要）
```

### 3.3 降级（无 Key / 模型不可达）
- 意图匹配：预置规则 `/(番茄|专注)/ → pomodoro.start`、`/(喝水|休息)/ → remind.water`。
- 命中则直接执行并宠物说话；未命中提示“未配置 AI 或无法理解”。

---

## 4. Agent Pack（可发包的人设）

```text
agent.official.secretary/
  package.json (type=agent)
  persona.md              # 人设与语气
  system_prompt.md        # 系统提示（可引用 ${pet.name} 等变量）
  tools.allowlist.json    # 允许的工具 id 前缀
  memory.policy.json      # 记忆策略
```
用户可在控制中心切换 Agent Pack；官方提供：软萌陪伴 / 严厉督导 / 秘书 / 极客。

---

## 5. 主动智能（Proactive）

- 由 Automation 触发 `agent.decide`，而非 Agent 自启轮询。
- 频率受 `proactive.maxPerHour` 限制；用户“别烦我”冷静 2h。
- 建议型动作只走 pet 演出 + 可选通知，不直接改系统设置。

---

## 6. 对外 AI API（第三方也可驱 Agent）

```
POST /v1/agent/chat   { message, sessionId? }
  → { reply, proposedActions, needConfirm }
POST /v1/agent/run    { goal, autoConfirm }
GET  /v1/agent/sessions
POST /v1/agent/memory/query
DELETE /v1/agent/memory/:id
```
Scope：`agent.chat` / `agent.run`。受同一 Token 与审计体系约束。

---

## 7. 实现任务（对应 TECH_SPEC T10）

- [ ] `packages/schemas/ai/v1.json` 校验
- [ ] `packages/agent/src/provider.ts`：openai_compatible + anthropic 适配
- [ ] `packages/agent/src/router.ts`：选 provider、装 system、聚 tools
- [ ] `packages/agent/src/executor.ts`：确认流 + 降级
- [ ] `packages/agent/src/memory.ts`：本地记忆
- [ ] 控制中心 AI 设置页 + Agent Pack 切换
- [ ] 命令面板 `AI: <自然语言>` 接入
- [ ] 入站 `/v1/agent/*` 接入 Gateway

---

## 8. 完成验收

- 配 OpenAI Key → 命令面板“进入工作状态” → Agent 调 `profile.switch`+`pomodoro.start` 并宠物说话。
- 无 Key → “番茄”命中降级规则仍可执行。
- danger 工具（安装技能）强制确认。
- 审计记录 agent 任务与工具调用。
- 切换 Agent Pack 后语气与可用工具变化。
