# Nimora 用户代码安全边界

> 状态：策略层、独立 Worker、Supervisor、桌面一次性执行编排与 Runtime Gateway 已实现，sidecar 打包仍在开发中

## 目标

用户可以编写代码控制角色、订阅事件并调用其它模块暴露的能力，但代码不能直接访问文件系统、网络、进程、窗口句柄或任意 Tauri API。所有能力必须经过版本化 Manifest 和运行时策略评估。

## Manifest

```json
{
  "id": "studio.example.focus",
  "version": "1.0.0",
  "capabilities": ["read-pet-state", "subscribe-events", "invoke-safe-commands"],
  "subscriptions": ["pet.example.clicked"],
  "commands": ["safe.example.notify"],
  "timeoutMs": 5000,
  "memoryBytes": 8388608
}
```

`nimora-user-code-policy` 当前强制执行：

- 程序 ID、事件类型和命令必须是小写 namespaced identifier。
- 只允许 `safe.*` 命令命名空间；高风险系统命令不会进入用户代码 API。
- 订阅和调用必须同时声明对应能力。
- 单次执行超时不得超过 30 秒，内存预算不得超过 64 MiB。
- 订阅最多 32 个，命令最多 32 个。

桌面端 `validate_user_program` 命令已接入该策略层。Creator Studio 或未来 CLI 提交 Manifest 后，会获得 `grantedCapabilities`、实际超时和内存预算组成的授权报告；安全模式会直接拒绝校验和后续执行准备。授权报告使用能力列表而非固定布尔字段，新增能力时保持 IPC 契约可扩展。

`ExecutionController` 已提供 Worker 准入边界：默认最多同时执行 8 个用户程序，每个执行句柄携带取消令牌、绝对截止时间、Manifest 内存预算和 1 MiB 输出上限。Worker 必须在事件循环和能力调用前执行 `checkpoint`，并通过 `record_output` 计量日志、返回值和标准输出；句柄释放时自动归还并发槽位。

`nimora-user-code-host` 提供独立进程 Supervisor 和版本化 JSONL 协议。Supervisor 不在进程内执行用户代码：它启动外部 Worker，关闭标准错误继承，限制协议输出，等待终态消息，并在取消、超时、输出超限或 Worker 崩溃时终止对应进程。Supervisor 本身不授予能力；Worker 的请求仍必须经过 Capability Gateway 和策略层。

`nimora-user-code-gateway` 已实现逐次能力授权。每个请求必须携带与准入句柄一致的 `executionId`、非空 `traceId` 和可选 `idempotencyKey`；读取宠物状态需要 `read-pet-state`，调用命令需要 `invoke-safe-commands`，且命令必须出现在当前程序版本的 Manifest 白名单中。Gateway 只把 JSON 值交给 `CapabilityBackend`，因此其它模块可以通过后端适配器暴露语义能力，而不会把 Core、数据库、Renderer 或 Tauri 句柄交给用户代码。

桌面端已提供 `start_user_program`、`invoke_user_program_capability` 和 `stop_user_program` 会话入口，并注册首个 Runtime 后端：读取宠物快照、`safe.pet.animate` 和 `safe.pet.move`。进入安全模式会取消并移除全部活动会话；Gateway 传入的 Trace ID 与幂等键会写入实际 Runtime Command。未注册命令即使名称位于 `safe.*` 且出现在 Manifest 中，也会被后端拒绝。

`nimora-user-code-worker` 使用嵌入式 Boa ECMAScript 引擎在独立二进制中执行 JavaScript。每次运行创建全新 Context，默认不存在 Node.js `process`、`require`、文件系统、网络或 Tauri 全局；源代码上限为 256 KiB，结果必须可转换为 JSON。桌面通过 `execute_user_program` 提供当前的一次性执行模型：读取宠物状态形成深度冻结的 `nimora.input` 快照，Worker 返回最多 32 条结构化命令计划，桌面再逐条经过 Gateway 授权并调用 Runtime Backend；任意一步失败都会停止后续命令。该模型不会把 Core 或 Tauri 回调注入脚本，因而更容易审计、回放与测试。集成测试会由 Supervisor 启动真实 Worker，并验证普通结果返回和 `while (true) {}` 死循环在截止时间后被操作系统终止。当前 Worker 尚未注入事件型 SDK，也尚未作为 Tauri sidecar 打包，因此发布版 UI 仍不能宣称脚本执行已可用。

## 模块调用模型

```text
User Code
  → Capability Gateway
    → Manifest Policy
      → Command Risk Check
        → Runtime Core / Connector / Skill
```

模块只暴露语义能力，不暴露内部对象或 Rust/Tauri 句柄。每次调用都携带 `traceId`、`executionId` 和 `idempotencyKey`，拒绝未知版本、未授权能力和超出预算的请求。

## 执行沙箱要求

后续执行器必须使用独立 Worker/进程，并具备：

- 无网络、无任意路径读写、无动态原生模块加载。
- CPU 时间片、堆内存、输出大小和并发数限制。
- 超时强制终止与崩溃隔离。
- 事件订阅背压、取消和清理。
- 审计日志、失败重试上限和安全模式联动。
- 离线环境中仍可运行已安装代码，但不能绕过本地策略。

当前仓库已完成 Manifest 策略评估、Tauri 会话入口、Worker 准入边界、独立进程 Supervisor、基础 JavaScript Worker、只读输入快照、一次性结构化调用计划、逐请求 Capability Gateway、首个桌面 Runtime 后端、取消/截止时间/输出预算和契约测试；Tauri sidecar 打包、事件型 SDK、操作系统级强制内存限制、WASM 引擎、授权型文件/网络/自动化后端和用户代码安装生命周期尚未完成，未完成部分不得被 UI 宣称为“可执行任意用户代码”。
