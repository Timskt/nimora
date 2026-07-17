# Nimora 用户代码安全边界

> 状态：策略层已实现，执行沙箱仍在开发中

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

当前仓库已完成 Manifest 策略评估、Tauri Gateway、Worker 准入/取消/截止时间/输出预算和契约测试；独立进程或 WASM/JS 引擎尚未完成，未完成部分不得被 UI 宣称为“可执行任意用户代码”。
