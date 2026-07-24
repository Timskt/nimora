# Nimora Auto Authorization 与无人值守执行规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-24  
> 状态：实现基线（领域 + SQLite + 桌面 Host + CLI grant 管理 + Away Summary 纵切已落地；系统密钥签名、独立 Auto-review、Full Device 真机危险矩阵、原生 visual QA **仍为剩余/未证明**）

## 1. 产品定义

“授权全部权限”是高风险但有真实价值的用户能力，尤其适用于编译、测试、模型生成、批量迁移和夜间任务。Nimora 必须允许用户主动开启，但不得把它实现为无限期、无范围、可被 Prompt/Skill/用户代码修改的布尔开关。

产品名称统一为 **无人值守授权（Unattended Execution Grant）**。UI 可提供“完全设备访问”档位，但底层始终保存不可变、可撤销、可审计的范围凭证。

## 2. 授权档位

| 档位 | 文件 | 审批 | 网络（桌面默认） | 寿命 | 推荐场景 |
| --- | --- | --- | --- | --- | --- |
| Observe | 只读 (`ReadOnly`) | 高风险询问 (`AskRisky`) | 离线或仅回环 | Session | 研究、分析、审查 |
| Workspace | 当前工作区写 (`WorkspaceWrite`) | 写入/外部副作用询问 (`AskRisky`) | 离线或仅回环 | Session | 日常开发 |
| Trusted Workspace | 当前工作区写 | **Grant 内不询问** (`NeverAskWithinGrant`) | 离线或仅回环 | Session | 已信任项目的长任务 |
| Unattended | 指定根目录 (`SelectedRoots` = workspace root) | **Grant 内不询问** | 离线或仅回环 | `UntilTimestamp`（签发 + 8h） | 夜间构建、测试、迁移 |
| Full Device | 全设备 (`FullDevice`) | **Grant 内不询问** | 离线，或在线时 `Unrestricted` | `UntilTimestamp`（签发 + 4h） | 用户明确承担高风险的设备级任务 |

档位只是生成配置的 UI 模板，不是权限判断逻辑。桌面 Host 的 `tier_policy`（`apps/desktop/src-tauri/src/unattended_auto_mode.rs`）把档位映射为 `SandboxScope` / `ApprovalPolicy` / `NetworkPolicy` / `GrantLifetime`；实际判断取以下交集：

```text
Product hard deny
∩ OS policy
∩ Organization policy
∩ User Execution Grant
∩ Goal/Plan/Workspace binding
∩ Module/Tool capability
∩ Current budget and data policy
```

### 2.1 `NeverAskWithinGrant`（sleep-safe 无人值守）

- **语义**：在 Grant **仍然有效**（未撤销、未过期）、且请求与 Goal / Plan revision / Workspace fingerprint / Tool / Provider / Model / 数据等级 / 网络策略 **精确绑定** 时，`AuthorizationGrant::authorize` 返回 `Authorized`，Auto Mode 对本会触发确认的 risk/effect **不再暂停**。
- **Sleep-safe**：用户离开（夜间任务、锁屏、不在控制中心）时，在范围内的可逆/受预算约束写操作可继续推进；**不是**绕过硬禁区、预算、绑定漂移或产品 deny-list。
- **仍会暂停**：无 Grant；`AlwaysAsk` / `AskRisky`；过期 / 撤销 / 无效 Grant；绑定漂移（Goal/Plan/Workspace）；越界工具或数据等级；预算耗尽；`OutOfScope` 映射为 `UnsafeEffect`。
- **代码路径**：`AutoModeSession::evaluate_step_with_grant` + `AutoModeTurnSupervisor::with_authorization_grant`；桌面 Runner 每批通过 `SqliteAuthorizationGrantRepository::get_active_for_goal` 装载活跃 Grant。

### 2.2 Full Device 风险（与 sleep-safe 的交汇）

- Full Device = `SandboxScope::FullDevice` + `NeverAskWithinGrant` + 默认寿命 **4h**；在线时网络可为 `Unrestricted`。
- Sleep-safe **放大** Full Device 风险：用户不在场时范围内动作不弹确认，因此 UI **必须** `tierRequiresDangerAck("full_device")`，并展示 §4 硬风险文案。
- Sleep-safe **不**等于 Full Device：Unattended（SelectedRoots + 8h）同样使用 `NeverAskWithinGrant`，但文件范围仍限制在签发根目录。
- 硬禁区（支付/Secret 外泄/关闭安全机制/扩展自提权等）在任意档位均 fail-closed。


## 3. Execution Grant 必备字段

契约：`nimora.authorization-grant/1`（camelCase JSON payload）。

| 字段 | 含义 |
| --- | --- |
| `id` / `goalId` / `planRevision` / `workspaceFingerprint` | 精确绑定 |
| `sandbox` | `read_only` · `workspace_write` · `selected_roots` · `full_device` |
| `approval` | `always_ask` · `ask_risky` · `auto_review` · `never_ask_within_grant` |
| `network` | `offline` · `loopback_only` · `allowlisted`(+domains) · `unrestricted` |
| `selectedRoots` | 仅 `selected_roots` 时非空；其它 sandbox 必须为空 |
| `toolAllowlist` / `providerAllowlist` / `modelAllowlist` | 非空有界集合 |
| `maximumDataClassification` | 数据等级上界 |
| `budget` | 步骤 / 工具 / 时间 / Token / 费用 |
| `lifetime` | `one_action` · `one_turn` · `session` · `until_timestamp` |
| `issuedAtMs` / `expiresAtMs` / `revokedAtMs` | 签发、到期、撤销 |
| `fingerprint()` | 对完整 payload 的 `sha256:…` 不可变指纹 |

桌面摘要契约：`nimora.authorization-grant-summary/1`（列表/控制中心用，不含 allowlist 细节）。

## 4. Full Device 警告

设置页和启动页必须明确展示：

- 可读取、覆盖或删除本机文件。
- 可运行命令、安装和执行第三方代码。
- 允许联网时数据可能离开设备。
- 工具可能读取凭据或影响登录状态。
- 外部 API、发布、部署和账户操作可能不可逆。
- Prompt Injection 或供应链内容可能诱导错误操作。

用户确认后仍不能绕过硬禁区：支付和身份确认、泄露 Secret、关闭产品安全机制、扩展自行提权、未签名更新、未知结果自动重放，以及组织强制策略。

桌面 FE：`tierRequiresDangerAck("unattended" | "full_device")` 要求危险确认后方可启动。

## 5. 失效与撤销

以下任一变化使 Grant 失效或重新确认：Goal、Plan revision、Workspace fingerprint/root、Provider、模型、工具集合、Capability schema、数据等级、预算扩大、授权档位、系统硬策略或过期时间。

- 用户可从 Agent 工作区（`listAuthorizationGrants` / `revokeAuthorizationGrant`）、任务中心、CLI（`nimora ai goal grant list|show|revoke`）和紧急停止快捷键即时撤销。
- 撤销写 `status=revoked` + `revokedAtMs`，更新 fingerprint 与 payload；活跃查询不再返回该 Grant。
- 撤销阻止新派发；正在运行的可取消工具收到 cancellation token。
- 无法确认已停止的外部操作进入 `indeterminate`，不得自动重试。
- Skill、Plugin、模型输出和用户代码只能请求授权，不能签发、扩大或延长授权。

## 6. Auto-review

独立 reviewer 可替用户判断原本需要批准的动作，但：

- reviewer 继承相同或更窄 Sandbox 和 Grant。
- reviewer 不能修改请求参数、扩大工具或网络范围。
- reviewer 输入使用脱敏的动作摘要、风险和参数指纹。
- reviewer 不可用、超时或结论不确定时按原审批策略暂停。
- reviewer 的模型、推理等级、理由和结论进入审计。

**状态**：领域枚举含 `ApprovalPolicy::AutoReview`；独立 reviewer 流水线 **尚未接线**（仍为剩余项）。

## 7. Away Summary

无人值守任务聚合离开期间摘要，用户回来时至少显示：Goal 进度/完成失败计数、Auto Mode 会话与暂停原因、授权 Grant 状态、Token/周期等预算投影、中文 highlights、风险备注（含 unattended / full_device）、以及可撤销 Grant 列表。**不得**暴露 Secret / credential / 隐藏推理。

### 7.1 已落地路径（2026-07-24）

| 层 | 位置 | 契约 / 命令 |
| --- | --- | --- |
| Host 聚合 | `apps/desktop/src-tauri/src/away_summary.rs` | `build_away_summary` / `load_away_summary` / IPC `get_away_summary`；FE wire `nimora.away-summary/1` |
| FE | `desktopApi.getAwaySummary` + `AwaySummaryPanel`（AgentWorkspace） | 刷新、空态、亮点与指标；preview 不伪造写副作用 |
| CLI | `nimora ai goal auto away-summary --database … --goal-id …` | 输出 `nimora.ai-away-summary/1`（含 sessions / grants / revokeGrantIds / riskNotes） |
| 持久化 | `SqliteAutoModeRepository::list_for_goal` + grant `list_for_goal` | 历史会话与 Grant 发现 |

**状态**：产品面 E2E **已接线**（本地单元 + CLI 集成测试）；**不是**原生 visual QA。更细字段（文件 diff / 网络域名清单等）可随 Checkpoint 投影加深，但不得再写“Away Summary 产品面完全未做”。

## 8. 当前实现（与代码对齐，2026-07-24）

### 8.1 已完成

| 层 | 位置 | 内容 |
| --- | --- | --- |
| 领域 | `crates/agent-runtime/src/authorization.rs` | `AuthorizationGrant` 校验、`authorize` 精确绑定、`NeverAskWithinGrant` → `Authorized` |
| Auto Mode | `auto_mode.rs` / `auto_execution.rs` | `evaluate_step_with_grant`；Supervisor 挂载 Grant 后跳过范围内确认暂停 |
| 持久化 | `crates/persistence-sqlite/src/authorization_grant_store.rs` | 表 `authorization_grant`；`issue` / `get` / `get_active_for_goal` / `revoke` / `list_for_goal` / `list_active`；payload+索引指纹一致性校验 |
| 桌面 Host | `apps/desktop/src-tauri/src/unattended_auto_mode.rs` | 五档 `tier_policy`；`start_unattended_auto_mode` 原子创建 Goal+Session+Checkpoint+Workspace+Grant+Job；`list`/`revoke` IPC |
| Runner | `auto_mode_runner.rs` | `load_active_authorization_grant` → `with_authorization_grant` |
| FE | `platform/desktop.ts` + `AgentWorkspace.tsx` | 档位选择、危险确认、启动/列表/撤销；摘要 `nimora.authorization-grant-summary/1`；`AwaySummaryPanel` |
| CLI | `apps/cli` | `ai goal grant issue\|list\|show\|revoke`；`ai goal auto away-summary`（与桌面同领域契约） |
| Away Summary | `away_summary.rs` + CLI/FE | 离开聚合 + 一键撤销 ID 列表；无 Secret |
| Schema | `packages/schemas` | 授权档位/摘要/完整 Grant 与 `pet_directive` 契约（见 `@nimora/schemas`） |

### 8.2 测试证据（本地门禁占位）

| 套件 | 约数 | 说明 |
| --- | --- | --- |
| auto-host / desktop host 相关 | **21** | unattended 单元 + companion/lifeform 宿主相关门禁合计占位 |
| `authorization_grant_store` | **6** | issue/get/revoke/active/list  round-trip |
| unattended unit | **7** | `tier_policy` / `infer_tier` / summary 状态 |
| runtime auto（grant 路径） | **13** | auto_mode + auto_execution + authorization 相关用例占位 |
| FE vitest（grant/companion 相关） | **58** | AgentWorkspace / desktop API / companion 等占位 |

以仓库当时 `cargo test` / `vitest` 输出为准；上表为文档证据占位，变更后应重跑并更新本表。

### 8.3 剩余项（诚实标准，禁止“下一版本”空话）

| 项 | 完成标准 |
| --- | --- |
| 系统密钥对 Grant 签名/加密 at rest | 使用 `SECRET_MANAGEMENT` 基线；payload 非明文可篡改；轮换可审计 |
| 独立 Auto-review 流水线 | `AutoReview` 路径真实调用 reviewer；不可用时 fail-closed 回退询问 |
| Full Device 真机危险矩阵 | 签名包 + 明确警告文案 + 硬禁区用例全部绿；**不得**用单元测试冒充真机 |
| 组织策略 / OS policy 交集 | 超出个人桌面 Grant 的策略层可观测、可拒绝 |
| 原生 visual QA / idle CPU | 与 lifeform 切片相同：未证明前禁止 production-ready 宣称 |

> **已关闭（勿再标剩余）**：Away Summary 产品面（Host+FE+CLI）；CLI grant `issue/list/show/revoke`。

Skill / Plugin / 模型输出 **不得** 签发 Grant；该硬规则已由 Host 签发路径保证，扩展面回归测试须持续保留。
