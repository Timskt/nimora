# Nimora Auto Authorization 与无人值守执行规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-18  
> 状态：实现基线

## 1. 产品定义

“授权全部权限”是高风险但有真实价值的用户能力，尤其适用于编译、测试、模型生成、批量迁移和夜间任务。Nimora 必须允许用户主动开启，但不得把它实现为无限期、无范围、可被 Prompt/Skill/用户代码修改的布尔开关。

产品名称统一为 **无人值守授权（Unattended Execution Grant）**。UI 可提供“完全设备访问”档位，但底层始终保存不可变、可撤销、可审计的范围凭证。

## 2. 授权档位

| 档位 | 文件 | 审批 | 网络 | 推荐场景 |
| --- | --- | --- | --- | --- |
| Observe | 只读 | 高风险询问 | 默认离线 | 研究、分析、审查 |
| Workspace | 当前工作区写 | 写入/外部副作用询问 | 域名白名单 | 日常开发 |
| Trusted Workspace | 当前工作区写 | Grant 内自动 | 域名白名单 | 已信任项目的长任务 |
| Unattended | 指定根目录 | Grant 内自动、独立 reviewer | 显式策略 | 夜间构建、测试、迁移 |
| Full Device | 全设备 | Grant 内自动 | 显式无限制可选 | 用户明确承担高风险的设备级任务 |

档位只是生成配置的 UI 模板，不是权限判断逻辑。实际判断取以下交集：

```text
Product hard deny
∩ OS policy
∩ Organization policy
∩ User Execution Grant
∩ Goal/Plan/Workspace binding
∩ Module/Tool capability
∩ Current budget and data policy
```

## 3. Execution Grant 必备字段

- `goalId`、`planRevision`、`workspaceFingerprint`。
- Sandbox：只读、工作区写、指定根、全设备。
- Approval：每次询问、风险询问、自动审查、Grant 内不询问。
- Network：离线、仅回环、域名白名单、无限制。
- Tool/Capability、Provider、模型 allowlist。
- 最大数据等级、步骤/工具/时间/Token/费用/并发预算。
- 寿命：一次动作、一次 Turn、Session、截止时间。
- 签发、到期、撤销时间和不可变 fingerprint。

## 4. Full Device 警告

设置页和启动页必须明确展示：

- 可读取、覆盖或删除本机文件。
- 可运行命令、安装和执行第三方代码。
- 允许联网时数据可能离开设备。
- 工具可能读取凭据或影响登录状态。
- 外部 API、发布、部署和账户操作可能不可逆。
- Prompt Injection 或供应链内容可能诱导错误操作。

用户确认后仍不能绕过硬禁区：支付和身份确认、泄露 Secret、关闭产品安全机制、扩展自行提权、未签名更新、未知结果自动重放，以及组织强制策略。

## 5. 失效与撤销

以下任一变化使 Grant 失效或重新确认：Goal、Plan revision、Workspace fingerprint/root、Provider、模型、工具集合、Capability schema、数据等级、预算扩大、授权档位、系统硬策略或过期时间。

- 用户可从任务栏、任务中心、CLI 和紧急停止快捷键即时撤销。
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

## 7. Away Summary

无人值守任务每批持久化增量摘要，用户回来时至少显示：Goal 进度、计划修订、文件变化、命令与测试、网络域名、自动批准动作、Token/费用/时间、失败与重试、当前暂停原因、下一步以及一键撤销入口。

## 8. 当前领域实现

`agent-runtime` 已提供 `AuthorizationGrant`、`AuthorizationRequest`、Sandbox/Approval/Network/Lifetime 枚举和精确绑定校验。下一纵切是 SQLite Grant 仓储、系统密钥签名/加密、Auto Loop 每轮准入、桌面风险确认和 CLI 管理面。
