# Nimora 里程碑回顾

> 更新日期：2026-07-18  
> 状态：持续维护  
> 目的：在可演示纵切、每五个生产提交、公开契约变化或重大偏差时校准方向

## 1. 回顾规则

每次回顾必须记录范围、证据、偏差、用户价值、安全与隐私、离线行为、UI 影响、稳定性、Actions 分钟影响和下一纵切。回顾不是进度宣传；未通过真实测试或真实宿主验证的能力必须继续列为缺口。

以下情况必须立即回顾并修正：

- 实现开始复制已有链路，形成第二套 Provider、Tool、权限或持久化通道。
- UI、CLI 与模块调用对同一能力产生不同安全语义。
- 为追求“自动”而吞掉未知结果、扩大授权或绕过人工复核。
- CI 失败依赖反复推送试错，或跨平台矩阵在无平台风险时自动运行。
- 新需求能显著提升大众用户、创作者或开发者价值，却未进入权威规格与验收。

## 2. M-2026-07-18 Agent 自主执行基线

### 已完成证据

- 持久 Goal/Plan、Workspace Snapshot、Context Compaction/Cache、Auto Mode Session/Checkpoint/Attempt 和公平有界 Loop 已形成领域链路。
- 桌面单轮 Resume 已接入生产 Provider Registry、Tool Registry、Capability Gateway、真实工作区重扫和持久提交。
- Execution Grant 将沙箱、批准、网络、数据、工具、预算与寿命正交建模；Plan 或 Workspace 漂移后授权失效。
- 推理等级与 Provider 映射可审计，不支持的显式等级 fail-closed。
- 常规 CI 收敛为 Ubuntu 单套质量门禁；macOS/Windows 矩阵改为里程碑、发布候选或平台风险时手动触发。

### 竞品吸收与 Nimora 增强

- 吸收 Codex 的 Goal 持续推进、Sandbox/Approval 分离、文件追踪和可恢复执行，但完成结论必须由当前 Plan 的逐项证据证明。
- 吸收 Claude Code 的分层配置、Auto Mode、模型选择与 Away Summary，但无人值守批准使用范围绑定 Grant，不提供永久无边界开关。
- 吸收 OpenCode 的工具级 `allow/ask/deny` 与可替换 Provider/Agent，但统一落入 Capability Gateway，禁止 Agent 自建旁路。
- Nimora 新增桌面伴侣事件、Automation、Skill、用户程序与 Agent 的双向能力图；模块调用 AI 必须经过 `module-agent-adapter` 与 `AgentTaskGateway`。
- Nimora 新增未知结果隔离、工作区/计划漂移自动失效、自动审查不扩权和面向离开用户的结构化归来摘要。

### 本次纠偏

桌面 `resume_auto_mode_turn` 是同步单轮恢复接口，不能在线程中重复调用来冒充后台 Auto Mode：首轮继续后 Session 保持 Running，而下一次恢复只接受 Paused Session。真正后台实现必须直接装配 `AutoModeLoopService`，在一个批次内持有领域续体，并在批次 `yielded` 后由 Supervisor 公平续调。暂停、取消和退出必须原子收敛持久 Session/Task/Attempt，不能只改变内存 UI 状态。

### 下一纵切硬验收

1. 同一 Session 最多一个活跃 Job，重复 Start 原子拒绝。
2. Job 使用版本化快照，支持 Start、Status、Pause、Cancel，不暴露 Provider 或 Tauri 原生对象。
3. 每批有限 Turn；完成、漂移、业务暂停和未知结果立即停止；Yield 后公平续调。
4. Pause/Cancel 传递到当前 Provider/Tool，并持久化业务状态；未知执行结果标记 `indeterminate`，不得自动重放。
5. Safe/Recovery Mode 与应用退出取消所有 Job，并进行有界等待或隔离未收敛结果。
6. 浏览器预览只返回 `desktop-host-required`，不得伪造后台执行。
7. Rust 并发/退出测试、TypeScript 契约测试、桌面构建和文档验收全部通过。

### GitHub Actions 分钟复核

- 月度硬预算：2000 分钟。
- 本纵切开发必须先本地运行定向测试，再运行完整质量门禁，最后只推送一次可验证提交。
- 常规提交不触发手动跨平台矩阵；后台线程、退出生命周期或平台 API 出现真实平台风险时，才在里程碑提交后手动运行一次。
- 不通过削弱安全、契约或发布验证节省分钟；应优先采用路径过滤、并发取消、依赖缓存和失败快速终止。

## 3. 后续回顾模板

```text
里程碑：
触发原因：
用户可演示价值：
完成证据：
未完成与偏差：
安全/隐私/离线：
UI 与无障碍：
稳定性与恢复：
Actions 分钟：
竞品变化与新技术：
新增需求：
下一纵切硬验收：
```
