# Nimora Agent 竞品能力吸收与超越规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-18  
> 状态：实现基线

## 1. 目标

Nimora 不复制某个 Coding Agent 的界面，而是吸收 Codex、Claude Code、OpenCode 已验证的交互和工程机制，再按桌面伴侣、自动化、模块生态与普通用户场景统一设计。所有能力必须同时服务桌面 UI、CLI、Automation、Module Agent、Skill 和受控用户代码，且不能形成绕过 Capability Gateway 的第二执行路径。

## 2. 官方能力对照

| 能力 | Codex | Claude Code | OpenCode | Nimora 结论 |
| --- | --- | --- | --- | --- |
| 权限与沙箱 | Sandbox 与 Approval 独立；支持只读、工作区写、全访问及按需/从不询问 | Managed/User/Project/Local 多层配置；权限、Sandbox、Auto Mode 规则分离 | `allow/ask/deny`，工具和参数通配规则，Agent 可覆盖 | 权限、沙箱、网络、数据、工具、预算和寿命必须为正交维度 |
| 无人值守 | `never` Approval 可配合 Sandbox；Full Access 明确高风险 | Auto Mode、自动分类、问题超时与 Away Summary | 可全局 allow，也可会话内 always 批准模式 | 提供范围绑定 Execution Grant，不提供无边界永久越权 |
| Goal/持续工作 | Prompt 可表达 Goal，支持计划、自动审查与长期任务形态 | Agent、Task、Hooks、自动压缩和记忆支持长会话 | Primary/Subagent、Build/Plan Agent | Goal 是持久领域对象，完成必须有当前计划证据 |
| 推理等级 | `model_reasoning_effort`，支持值取决于模型 | 模型与 Thinking/预算由模型配置控制 | Provider 模型 options 与 variants 支持 reasoning/thinking | 统一等级，Adapter 映射并审计实际等级 |
| 上下文 | 压缩、缓存和文件追踪 | Auto Compact、Auto Memory、Away Summary | Session、模型变体、Agent 配置 | 压缩、缓存键、Workspace 版本和恢复绑定必须一致 |
| 扩展 | Skills、MCP、Agent、Hooks | Skills、Plugins、MCP、Hooks、Agent SDK | Agents、Commands、Skills、MCP、Plugins | 所有扩展只得到能力句柄，不得到原生宿主对象 |

## 3. Nimora 必须超越的部分

1. **Goal Completion Proof**：模型不能自行宣称完成；当前 Plan 的每一步必须包含可验证证据。
2. **Execution Grant Binding**：预授权绑定 Goal、Plan revision、Workspace fingerprint、Provider、模型、工具、数据等级、预算和过期时间。
3. **Desktop Away Experience**：用户回来时展示修改文件、测试结果、预算、自动授权使用、失败重试和暂停原因。
4. **Cross-module Agent Fabric**：模块通过 `AgentTaskGateway` 请求 AI，AI 通过 Tool Registry 和 Capability Gateway 调模块，双方均不可直连 Provider 或原生能力。
5. **Provider-neutral Reasoning**：用户选择语义等级和策略，Provider Adapter 决定合法参数，并记录 requested/actual/provider value。
6. **Offline-first Agent**：本地 Provider、索引、规则、Goal、计划、Checkpoint、审计和恢复在断网时仍可工作；联网能力显式降级。
7. **Unknown Outcome Isolation**：命令、网络或工具结果未知时不得自动重放，必须进入对账状态。

## 4. Goal Automode 状态机

```text
Draft Goal
  -> Active Goal
  -> Plan revision N
  -> Auto Session Running
  -> Turn Attempt
  -> Provider/Tool/Commit
  -> Continue | Yield | Pause | Completed | Cancelled
```

- `Continue`：当前 Turn 形成完整 continuation，下一轮仍需重新验证 Workspace、Grant、预算和 Cache key。
- `Yield`：公平让出 CPU/Provider 配额，不改变业务状态，后台调度器可继续下一批。
- `Pause`：缺少输入、需要批准、预算耗尽、Workspace/Plan/Grant 漂移、Provider 不可用或存在未知结果。
- `Completed`：只在 Goal 当前计划逐项证据成立时提交。
- 进程退出前必须请求收敛；无法确认结果的 Attempt 标记为 indeterminate。

## 5. 未来兼容原则

- 新 Provider、新推理参数、新工具协议通过 Adapter/Strategy 注册，不修改 Goal 核心状态机。
- 新权限类型通过 Policy Evaluator 扩展，默认未知即拒绝。
- 新 Agent 架构可作为 Scheduler Strategy 或 Agent Profile 加入，不改变 Tool/Capability 边界。
- 新上下文技术必须参与内容寻址、数据等级、Workspace 和推理策略指纹。
- 所有远程自治、群体 Agent、视觉操作和设备控制沿用同一 Execution Grant 与审计模型。

## 6. 资料基线

设计核验基于 2026-07-18 可访问的官方文档：OpenAI Codex Manual、Claude Code Settings/Permissions/Model Configuration、OpenCode Permissions/Models/Agents。实现时以 Provider 当前官方契约和 Capability discovery 为准，不硬编码竞品未承诺的行为。
