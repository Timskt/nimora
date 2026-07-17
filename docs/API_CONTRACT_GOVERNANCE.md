# Nimora API、事件与契约治理规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-17

## 1. 契约范围

- Event Envelope 与事件数据 Schema。
- Command、Query 和 Agent Tool Schema。
- Gateway OpenAPI、WebSocket 和 SSE 协议。
- Host API 与 IPC 消息。
- Asset、Skill、Connector、Agent Pack Manifest。
- 配置、导入导出和 Registry 元数据。

## 2. 命名规则

- 契约标识：`deskpet.<domain>/<major>`。
- Event：过去式事实，如 `pet.stats.changed`。
- Command：动词意图，如 `pet.say`、`profile.switch`。
- Package ID：反向域名或已验证发布者命名空间。
- 字段使用 lowerCamelCase；时间为 ISO 8601 UTC；持续时间明确单位。

## 3. 兼容性

首个稳定版前执行 [`GREENFIELD_BASELINE.md`](GREENFIELD_BASELINE.md)：每类契约只有一个现行版本，变更时同步更新实现、夹具和文档，不保留兼容分支。版本字段只用于严格识别当前输入并拒绝未知输入。

首个稳定版发布后，同主版本才适用兼容规则：允许新增可选字段；新枚举值仅在消费者声明可忽略未知值时加入。禁止删除字段、改变单位、改变默认值语义、将可选改为必填或复用已移除字段。

## 4. 错误模型

```json
{
  "error": {
    "code": "permission.scope_denied",
    "message": "Token lacks commands.execute scope",
    "retryable": false,
    "traceId": "tr_01...",
    "details": { "requiredScope": "commands.execute" }
  }
}
```

错误码稳定、可搜索且不泄露密钥和内部堆栈。HTTP 状态、IPC 错误和 Command Result 映射必须记录在 Schema Catalog。

## 5. 分页、过滤与流

- 列表 API 使用 cursor pagination，不依赖不稳定页码。
- 时间过滤明确时区和闭区间语义。
- WS/SSE 订阅支持事件白名单、恢复游标和心跳。
- 慢消费者触发断开或降采样，不能拖慢 Event Bus。

## 6. 幂等与并发

- 可重试写 Command 接受 idempotency key。
- 状态修改支持版本或 ETag，冲突返回明确错误。
- Event ID 不作为 Command idempotency key 自动复用。
- 批量操作定义全成功、部分成功或事务语义。

## 7. 弃用

- 首个稳定版前不创建弃用 API；旧设计直接从唯一当前契约中删除。
- 稳定版发布后，只有真实公开契约需要替换时才启动弃用流程。
- 届时弃用先在 Schema、SDK 和日志中标记，至少保留一个稳定发布周期，并提供替代接口、迁移示例和截止版本。
- 安全原因可加速移除，但必须发布公告和兼容影响。

## 8. 契约测试

- 每个 Schema 提供合法最小、合法完整和非法夹具。
- SDK 对同一夹具产生一致结果。
- Provider/Extension 通过 conformance suite 才能标记 Verified。
- CI 比较 Schema 差异，阻止未声明的破坏性变更。

## 9. 文档生成

OpenAPI、Schema Catalog、TypeScript 类型和 Rust DTO 应从同一 Schema 来源生成。手写文档负责语义、风险和示例，不复制易漂移的字段表。
