# Nimora 用户编程控制与自动化规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-17

## 1. 产品定位

用户自己写代码控制宠物、系统行为和外部设备是原始核心能力之一。平台同时提供四级入口：可视化规则、YAML 自动化、本地脚本、完整 Skill/Connector SDK。它们共用 Event、Query、Command Registry 和 Capability Broker，不形成多套权限旁路。

## 2. 能力阶梯

| 层级 | 面向用户 | 能力 | 安全边界 |
|---|---|---|---|
| Rules | 普通用户 | 模板、表单、流程图 | 仅预注册 Action |
| Automation as Code | 极客用户 | YAML、表达式、版本控制 | 声明式、无任意系统调用 |
| Local Script | 会编程用户 | TypeScript/JavaScript 函数与 REPL | 沙箱 Host + 显式 Capability |
| Extension SDK | 开发者 | Skill、Connector、Renderer/Importer | 独立 Host、签名与审核 |

“本地脚本”不等于启动任意 Node 程序。脚本只能使用平台注入的 SDK，不开放 Node 内建文件、网络、进程、动态原生模块或任意 `eval`。

## 3. 用户场景

- 编写宠物动作：收到事件后播放动作、说话、切换表情或移动到安全位置。
- 自动化桌面：时间、快捷键、窗口状态、Webhook、MQTT 或设备事件触发 Command。
- 开发辅助：构建失败时反馈、Git 状态提醒、专注计时、日志或 CI 事件联动。
- 角色联动：用统一语义驱动 Live2D/VRM，不直接依赖模型私有参数。
- 外部控制：通过 Gateway Client 编写本机或局域网控制器，Scope 与速率受限。
- 自定义面板：声明式 UI Contribution 展示状态与表单，不注入控制中心 DOM。

## 4. 编程模型

```ts
import { defineScript } from "@deskpet/sdk";

export default defineScript({
  id: "local.build-companion",
  capabilities: ["pet.animate", "notification.send"],
  triggers: [{ event: "dev.build.finished" }],
  async run(context, event) {
    await context.commands.execute("pet.expression.set", {
      expression: event.payload.succeeded ? "happy" : "sad"
    });
  }
});
```

脚本 API 仅包含：

- `events.subscribe`：订阅有 Schema 的事件，支持过滤、去抖和背压策略。
- `queries.get`：读取授权状态，不产生副作用；当前已接入宠物状态与 Profile 快照适配器。
- `commands.execute/preview/undo`：执行注册命令并继承风险、确认、审计和幂等规则。
- `storage`：脚本命名空间的配额存储。
- `schedule`：持久化定时任务，支持时区、休眠补偿和错过策略。
- `ui`：气泡、通知、表单和受限面板贡献。
- `log/metrics`：脱敏日志和配额指标。

API 不暴露 Core 对象、数据库连接、Renderer 实例或 Secure Store。模型操作使用语义 Command；只有经过审核的 Renderer Adapter 才接触格式私有参数。

实现层通过 `CapabilityBackend` 适配其它模块：Worker 只发送版本化 JSON 请求，Gateway 验证运行身份、Capability、Manifest 命令白名单、取消与截止时间后，才把请求路由到 Pet、Renderer、Connector、Skill 等模块适配器。模块不能向 Worker 返回内部句柄，也不能自行绕过 Gateway 建立第二条调用通道。

## 5. 生命周期与执行语义

```text
draft → validated → authorized → enabled
enabled → running → completed | failed | cancelled | timed-out
enabled → suspended | quarantined
any → disabled → deleted
```

- 每次运行有 Run ID、Trace ID、来源、版本快照、超时和取消信号。
- 并发策略为 `drop`、`queue`、`parallel` 或 `cancel_previous`，必须显式声明。
- 可变更 Command 使用幂等键；重试只用于声明为安全的操作。
- 脚本版本升级保留配置迁移和最近可用版本，失败自动回滚。
- 当前程序包格式固定包含 `manifest.json` 与 `main.js`；安装采用清单哈希校验、暂存、原子激活和最近版本回滚，离线环境不依赖 Registry 即可启动已安装版本。
- 正式运行使用 `execute_installed_user_program(programId)` 从激活版本加载源码；执行前根据安装器生成的 `.nimora-integrity.json` 复验程序身份、版本、完整 inventory、文件大小和 SHA-256，缺失、篡改、额外文件或符号链接都会阻止 Worker 启动，升级与回滚版本携带各自的锁文件。直接提交源码的入口仅用于 Creator Studio 草稿预览，不能替代安装版本身份。
- 休眠恢复、时钟跳变和离线期间按声明的 missed-run policy 处理。
- 事件订阅使用独立有界队列，不消费 UI 或其他程序的事件；当前每会话 64 条、全局 32 个会话，满载时丢弃该会话最旧事件并报告 `dropped`，安全模式、撤权、升级与回滚会取消会话。事件过滤器只能来自已安装 Manifest，Renderer 不能注入事件正文。
- Manifest 必须使用 `eventConcurrency` 声明 `serial`、`drop` 或 `cancel-previous`，并以 `eventQueueCapacity` 声明 1–64 的每程序触发队列；字段缺失或无效时直接拒绝安装，不进行隐式补全。该策略由 Rust 调度器执行，不能由 Renderer 临时放宽。

## 6. 权限与信任

- 首次启用显示代码来源、触发条件、数据访问、外部目标和系统影响。
- 权限按脚本身份、精确版本与完整 Capability 集合持久化授予；首次使用、版本变化、能力增加、能力删除或重命名都必须重新确认，撤销会覆盖该程序的全部历史版本。
- 本地代码不因“本地”自动可信，粘贴代码和远程下载代码均经过静态扫描和授权。
- 文件、网络和进程能力只能通过 Broker 的窄接口；目标域名、目录和应用可进一步约束。
- 摄像头、麦克风、键盘监听、屏幕内容和模型动捕属于持续敏感能力，必须有常驻指示与一键停止。
- 安全模式停止全部用户脚本和第三方 Host，但保留查看、导出和修复能力。

## 7. 编辑器、调试器与开发体验

Creator Studio 内置：模板、类型提示、Schema 补全、权限预览、格式化、静态检查、断点、单步、变量检查、虚拟时间、事件录制/回放、Command dry-run、网络 Mock、执行历史和性能火焰图。

调试环境与生产 Host 使用同一 API 和权限判定。调试器不能以开发便利为由绕过确认；测试授权必须明显标记且仅对隔离 Profile 生效。

```bash
corepack enable
pnpm install --frozen-lockfile
pnpm deskpet script create build-companion
pnpm deskpet script validate ./scripts/build-companion
pnpm deskpet script test ./scripts/build-companion
pnpm deskpet script dev ./scripts/build-companion
pnpm deskpet extension pack ./scripts/build-companion
```

## 8. 分发与供应链

- 私人脚本可保存在工作区、导出或 Git 管理；默认不自动同步代码与密钥。
- 分享脚本必须固定依赖和权限清单，生成内容哈希与 SBOM。
- Registry 发布升级为 Skill 包，经过恶意代码、依赖、许可证、隐私和行为审核。
- 禁止运行时从 CDN 拉取并执行代码；依赖必须在构建时锁定、审计和打包。
- 官方示例与模板也使用相同 Host、SDK 和审核门禁。

## 9. 离线与鲁棒性

- 编辑、验证、运行、调试本地脚本不依赖登录、Registry、AI 或网络。
- 脚本崩溃、死循环、内存超限和事件风暴只终止对应实例。
- Host 实施 CPU、内存、定时器、队列、日志、存储和 Command 速率配额。
- 外部依赖不可用时采用超时、熔断、退避和明确降级，不阻塞 Pet Runtime 启动。
- 自动保存采用原子写入；异常退出后恢复草稿，不自动启用未验证版本。

## 10. 契约演进与扩展

- SDK、Schema、Command 和 Event 独立语义化版本；脚本声明兼容范围。
- 首个稳定版发布前只维护当前明确契约，不实现假想旧版本的兼容、迁移或弃用分支。
- 稳定版发布后，只有真实已发布契约发生变化时才基于实际用户数据设计诊断和迁移工具。
- Host 支持未来增加 WASM 等运行时，但必须通过同一 Capability Broker 与契约测试。
- 自动化定义可导出为开放、可读、可 diff 的格式；不将用户锁定在流程图二进制文件中。

## 11. 必测场景

- 未授权脚本无法执行 Command、访问网络、文件、进程或敏感传感器。
- 死循环、内存泄漏、事件递归和日志洪泛触发配额且不影响 Core。
- 取消、超时、重试、幂等、补偿和并发策略符合声明。
- 脚本升级失败可回滚，权限扩大必须再次确认。
- 离线、休眠恢复、时区变化和时钟回拨不造成任务风暴。
- 事件回放可复现结果，密钥和敏感字段不进入日志、快照或导出包。
- 同一脚本可通过语义 Command 控制序列帧、Live2D 和 VRM 角色。
