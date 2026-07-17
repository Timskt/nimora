# Nimora 用户代码安全边界

> 状态：策略层、独立 Worker、Supervisor、桌面一次性执行编排、Runtime Gateway 与 macOS sidecar 打包已实现，跨平台发布验证仍在进行

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

`nimora-user-code-worker` 使用嵌入式 Boa ECMAScript 引擎在独立二进制中执行 JavaScript。每次运行创建全新 Context，默认不存在 Node.js `process`、`require`、文件系统、网络或 Tauri 全局；源代码上限为 256 KiB，结果必须可转换为 JSON。桌面通过 `execute_user_program` 提供当前的一次性执行模型：读取宠物状态形成深度冻结的 `nimora.input` 快照，Worker 返回最多 32 条结构化命令计划，桌面再逐条经过 Gateway 授权并调用 Runtime Backend；任意一步失败都会停止后续命令。该模型不会把 Core 或 Tauri 回调注入脚本，因而更容易审计、回放与测试。构建脚本会按目标 triple 编译 Worker，Tauri `externalBin` 将其打包为 sidecar，运行时优先从应用可执行目录解析。当前 Worker 尚未注入事件型 SDK，发布版仍不能宣称支持任意脚本能力。

`nimora.input` 是版本化、按授权裁剪的输入契约，而不是完整宿主状态。所有程序都会获得 `schemaVersion: 1`；只有 Manifest 明确声明 `read-pet-state` 时才会读取并加入 `pet` 字段。未授权程序即使尝试访问 `nimora.input.pet` 也只能得到 `undefined`，桌面端不会预先读取再隐藏该数据。后续增加剪贴板、文件选择或连接器上下文时也必须遵守同一原则：先完成授权，再按最小披露构造对应字段。

```json
{
  "schemaVersion": 1,
  "pet": {
    "name": "Aster",
    "state": "idle"
  }
}
```

## 安装与版本生命周期

`nimora-user-code-package` 定义用户程序包的本地安装边界。每个包必须包含与安装请求完全一致的 `manifest.json` 和固定入口 `main.js`，最多 64 个文件、总计不超过 2 MiB；重复路径、Manifest 符号链接逃逸、哈希或大小不一致都会在激活前失败。策略层会在复制前再次校验程序 ID、版本、Capabilities、订阅、命令和预算。安装器还会在 staging 中生成保留文件 `.nimora-integrity.json`，锁定程序 ID、版本、完整文件 inventory、大小和 SHA-256，并与程序文件一起原子激活；包作者不能提供或覆盖该文件。

桌面端通过 `install_user_program` 把验证后的程序原子安装到应用数据目录的 `programs/<program-id>/active`。升级时旧版本先移动为不可变备份，新版本只有在完整复制和二次校验后才会激活；激活失败会恢复旧版本。`rollback_user_program` 会隔离当前失败版本并恢复最近备份。安装和回滚在安全模式中均被拒绝，防止故障处置期间改变可执行内容。

`execute_installed_user_program` 只接受 namespaced 程序 ID，从当前 `active` 目录重新加载并校验 Manifest 与固定入口，不允许 Renderer 再次提交或替换源码。每次执行前必须复验锁文件身份与版本，并递归验证所有已锁定文件的大小和 SHA-256；缺失文件、额外文件、重复锁项、损坏锁文件和任意符号链接都会在 Worker 启动前被拒绝。该路径不依赖 Registry 或网络，已安装且不请求远程能力的程序可以完全离线运行。此机制用于发现安装后的损坏、不同步修改和低权限注入；若攻击者已经控制运行 Nimora 的同一 OS 账户并能同时重写程序和锁文件，则必须结合后续的发行者签名、系统密钥存储与平台代码签名建立更强信任根，不能把本地 SHA-256 锁文件描述为抗主机接管边界。

正式程序权限保存在 SQLite 的 `user_program_permission_grant` 中，授权键由程序 ID、精确版本和完整 Capability 集合共同组成。无 Capability 的纯计算程序不需要授权；有 Capability 的程序在首次运行前必须显式授予。版本变化、能力增加、能力删除或能力名称变化均不会匹配旧授权，`execute_installed_user_program` 会在创建 Worker 前拒绝执行。`user_program_permission_status`、`grant_user_program_permissions` 和 `revoke_user_program_permissions` 只针对完整性校验通过的已安装版本；撤销按程序身份删除所有版本授权。草稿预览不创建正式授权，也不能借用已安装版本的授权身份。

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

当前仓库已完成 Manifest 策略评估、Tauri 会话入口、Worker 准入边界、独立进程 Supervisor、基础 JavaScript Worker、只读输入快照、一次性结构化调用计划、逐请求 Capability Gateway、首个桌面 Runtime 后端、取消/截止时间/输出预算、sidecar 构建配置、原子程序安装与回滚、执行前完整性复验、版本化权限持久化和契约测试；跨平台 sidecar 发布验证、事件型 SDK、操作系统级强制内存限制、WASM 引擎及授权型文件/网络/自动化后端尚未完成，未完成部分不得被 UI 宣称为“可执行任意用户代码”。
