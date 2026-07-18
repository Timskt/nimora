# Nimora 模型推理等级与路由策略

> 版本：0.1.0-draft  
> 更新日期：2026-07-19
> 状态：实现基线

## 1. 统一语义

用户配置的是跨 Provider 的意图，不是厂商参数。统一等级为：

```text
auto | minimal | low | medium | high | very_high | maximum
```

统一策略为：

- `adaptive`：规划、架构、安全审查和失败诊断提高等级；机械编辑和摘要降低等级。
- `quality_first`：选择模型支持的最高合法等级。
- `cost_saver`：选择模型支持的最低合法等级。
- `fixed`：严格使用指定等级，不支持即 fail-closed。

## 2. 配置优先级

```text
Organization hard policy
> Plan Step / Reviewer / Subagent explicit policy
> Goal policy
> Authorization Profile
> User default
> Provider default
```

低层配置不得突破组织费用、数据、模型 allowlist 或最大推理等级。

## 3. Provider Adapter 映射

- OpenAI Adapter 映射到当前模型公开支持的 reasoning effort。
- Anthropic Adapter 映射到 thinking 开关和 Token budget。
- Google、本地模型及未来 Provider 通过 capability discovery 或版本化 descriptor 报告等级。
- `auto` 允许在能力集合内安全降级；显式等级默认不静默降级。
- Adapter 必须返回 requested、actual、provider value、是否降级和能力版本。

不能把 `very_high` 永久等同某个厂商的 `xhigh`，也不能假定所有模型支持同一枚举。

## 4. 自适应判定输入

Adaptive Strategy 可使用任务类型、风险、Plan step 类型、最近失败次数、上下文压力、工具副作用、用户质量偏好、费用剩余和延迟预算。Prompt 内容只能影响推荐等级，不能突破策略上限。

典型建议：

- Minimal/Low：格式化、稳定结构提取、短摘要、确定性工具编排。
- Medium：一般问答、普通代码编辑、测试修复。
- High：架构、复杂调试、迁移、安全与权限决策。
- Very High/Maximum：用户明确要求且模型支持的关键规划、独立审查或高难推理。

## 5. 缓存与审计

请求和 Context Cache fingerprint 必须包含：Provider、模型、请求策略、实际等级、Provider 映射版本、Plan revision、Workspace fingerprint、工具 schema 和消息内容。不同推理等级不得错误共享生成结果缓存。

审计记录 requested/actual、策略来源、降级理由、输入输出 Token、费用、延迟和完成质量证据，不保存模型隐藏推理内容。

## 6. UI 与 CLI

- 普通用户看到“自动、节省、均衡、深入、极致”及费用/速度提示。
- 开发者可查看统一等级、Provider 实际参数和降级原因。
- CLI 支持 Goal、Profile、单步和 Subagent 级覆盖，并提供 JSON 输出。
- 不支持的显式等级应在 Provider 调用前报错，不能产生已计费但无效的请求。

## 7. 当前实现

`agent-runtime` 已提供 `ReasoningEffort`、`ReasoningStrategy`、`ModelReasoningPolicy` 和 `ReasoningMapping`，覆盖 Adaptive/Quality First/Cost Saver/Fixed 解析、显式不支持 fail-closed 和实际映射审计。

Provider Descriptor 现可显式声明不包含 `auto` 的具体等级集合与有界 Mapping Version；默认 Provider 不获得虚构推理能力。`ProviderRequest` 可携带宿主已解析的完整 Mapping，Registry 在 Adapter 和计费调用前复验能力集合及版本，未声明能力、等级越界和映射版本漂移均失败关闭。Context Cache 内容身份同步绑定 requested、actual、Provider value 和 Mapping Version，不同推理配置不得共享条目。

OpenAI-compatible Provider 已支持用户显式配置 `effort -> provider value` 映射及 Mapping Version。配置默认关闭、旧记录无损兼容，空映射、`auto`、控制字符和越界值在保存前拒绝；Adapter Descriptor 只投影具体等级集合与版本。隔离 Worker 仅转发 Registry 已复验的 `provider_value` 到 `reasoning_effort`，不会把 requested、actual、Mapping Version 或隐藏推理发送给外部服务；未携带 Mapping 时该字段完全缺失。

尚未完成 Anthropic/本地 Adapter 参数映射、自动 capability discovery、Auto Loop 策略解析、持久授权与默认策略、CLI 选择器及映射审计展示；桌面 Provider 设置目前只负责可信能力声明，不能伪装成 Agent Run 已自动选择推理等级。
