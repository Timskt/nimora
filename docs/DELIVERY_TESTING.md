# Nimora 开发交付与测试规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-17

## 1. 开发原则

- 每个里程碑产生可安装、可演示、可回滚的成果。
- 先冻结契约和测试夹具，再实现多个适配器。
- 平台相关功能必须在 Windows 和 macOS 分别验收。
- 安全、迁移、诊断和卸载不是发布后的补充工作。

## 2. 工作流

所有变更必须遵守 [`GIT_WORKFLOW.md`](GIT_WORKFLOW.md)，通过受保护分支、Pull Request、必需评审和 CI 门禁进入主干。

```text
需求/缺陷
 → ADR 或契约变更
 → Schema 与测试夹具
 → Core 实现
 → Adapter/UI 实现
 → Contract/Integration/E2E
 → 性能与安全门禁
 → 可安装产物
```

## 3. 里程碑验收

### M0 Runtime

- 原生透明窗口可见、可拖拽、置顶和切换穿透。
- 默认角色完成 idle、walk、click、drag、sleep。
- Command、Event、Profile 和持久化可用。
- 托盘可以恢复穿透窗口并进入安全模式。
- Windows 10/11 与 macOS 12+ 冒烟通过。

安全模式的 M0 自动化证据至少包括：状态与原因不变量、重复进入/退出不发布假事件、Command/Event Trace 关联、IPC 映射、仓库策略检查，以及 Windows、macOS、Linux 编译测试。涉及真实窗口菜单和点击穿透的行为仍需在 Windows 与 macOS 发布候选产物上执行人工冒烟。

窗口拖拽冒烟必须覆盖连续移动期间无明显卡顿、停止移动后最终落点可恢复，以及拖动后立即从托盘退出再启动仍恢复最终位置。移动事件采用 200ms trailing-edge 合并，测试不得假设每个系统 `Moved` 事件都会产生持久化事件。

Click/Drag FSM 自动化证据必须覆盖：Sleep 被 Drag 抢占、重复 Begin 被拒绝、Drop 原子更新位置并恢复 Idle、无 Drag 时 Drop 被拒绝、点击携带坐标与按钮、600ms 收尾不覆盖更新的 Work/Sleep 状态，以及所有交互事件与 Command 的 Trace 关联。

### M1 Creator Foundation

- Character、Skin、Theme 包 Schema 和解析器可用。
- Creator Studio 可预览动画、命中区和资源预算。
- 损坏或不兼容资源不会破坏当前角色。
- 正式包支持原子安装、切换和回滚。

### M2 Skills

- Extension Host 与 Capability Broker 可用。
- 官方番茄钟和提醒作为 Skill 安装运行。
- 扩展崩溃、超额、停用时贡献项正确清理。
- 高危能力无法绕过 Broker。

### M3 Automation

- Trigger、Condition、Action、Policy 契约可用。
- 执行历史显示每步输入、输出和失败原因。
- 支持测试运行、取消、超时、重试和事件回放。

### M4 Open Platform

- Gateway 配对、Token、Scope、REST、WS、SSE Server 可用。
- HTTP Sink、UDP Sink 和至少一个 Source Connector 可用。
- 每次外部访问和投递都有脱敏审计。
- 安全模式在 2 秒内停止所有外联。

### M5 Ecosystem

- 包签名、Registry、兼容检测、更新和回滚可用。
- 权限扩大更新要求重新授权。
- CLI 与 CI 可以离线校验所有包类型。

### M6 Agent

- 多 Provider、Tool Registry、计划确认、任务历史可用。
- 风险根据实际 Tool 和参数计算。
- 失败任务可解释，不留下未知中间状态。
- 无 Key 或 Provider 故障时核心能力不受影响。

## 4. 测试分层

| 层级 | 覆盖内容 |
|---|---|
| Unit | FSM、策略、合并、迁移、过滤、风险计算 |
| Schema | 正反例、兼容性、旧版本迁移 |
| Contract | Host API、IPC、Gateway、Connector、包格式 |
| Integration | Core + Adapter、Host + Broker、Event + Connector |
| E2E | 用户关键路径、权限、升级、回滚、安全模式 |
| Soak | 24/72 小时内存、句柄、事件风暴、断网重连 |
| Security | 路径穿越、SSRF、权限绕过、恶意包、Prompt Injection |
| Compatibility | Windows/macOS、DPI、多屏、不同 WebView 版本 |

## 5. 必测场景

- 点击宠物产生 `pet.interaction.clicked`，官方技能响应，UI 和审计可追踪同一 Trace ID。
- 切换 Skin 时保留位置和状态，缺失动作正确回退。
- 扩展申请网络能力但调用未授权目标时被拒绝。
- Source Connector 重放重复外部事件时不重复执行非幂等 Command。
- HTTP Sink 重试保留事件 ID，并使用新的 delivery ID。
- 配置升级失败后恢复上个快照。
- WebView 崩溃后重载，Pet 状态不丢失。
- Agent 遇到恶意外部内容时不能越权调用工具。
- 安全模式后所有监听端口和外部连接关闭。

## 6. 质量门禁

- UI 必须通过 Design Token lint、组件状态测试、键盘与无障碍检查、Windows/macOS 关键截图回归。
- 每个关键流程按 [`UI_DESIGN_SYSTEM.md`](UI_DESIGN_SYSTEM.md) 人工评分；任一维度为 0 或低于 13/16 不得进入候选发布。
- 权限、恢复路径、焦点可见性、对比度和 200% 缩放为一票否决项。
- 主干拒绝直接推送、force push、未批准 PR、未签名发布标签、密钥泄露和来源不明的大型二进制资源。
- PR 合并标题必须符合 Conventional Commits，破坏性变化必须附迁移、兼容和回滚说明。

- 公开 Schema 100% 具有正例和反例测试。
- Core 新代码单元覆盖率不低于 80%，安全策略分支 100%。
- 不存在 Critical/High 依赖漏洞或未处理的高危安全发现。
- 空闲性能、启动时间、包体和 soak test 达到产品 NFR。
- 所有新增公共接口有版本、文档、示例和弃用策略。
- 安装、升级、降级、卸载和数据保留行为均被测试。

## 7. 发布产物

- Windows 签名安装包。
- macOS 签名、公证 Universal 或双架构安装包。
- SBOM、依赖许可证、hash 和签名信息。
- Schema、SDK、CLI 和官方示例版本。
- 迁移说明、已知问题、回滚说明和安全公告渠道。

## 8. Definition of Done

一个功能只有在以下条件全部满足时完成：

- 行为和边界符合产品规格。
- 公开契约已更新且兼容性明确。
- 权限、隐私、失败和降级路径已实现。
- 自动化测试覆盖正常与异常路径。
- Windows 与 macOS 的适用行为已验证。
- 用户文档、开发文档和诊断信息已更新。
- CI 验证 Corepack 与锁定的 pnpm 版本，安装必须使用 `pnpm install --frozen-lockfile`。
- 检测并拒绝 npm/Yarn/Bun 命令及 `package-lock.json`、`npm-shrinkwrap.json`、`yarn.lock`、`bun.lock*`。
- Renderer Adapter、Importer 和用户脚本 Host 必须通过契约、安全、资源泄漏与故障隔离测试。
