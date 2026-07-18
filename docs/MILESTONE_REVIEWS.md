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

## 3. M-2026-07-18 后台 Auto Mode 与崩溃恢复

### 触发原因

- 后台 Job 已形成可演示纵切，并累计超过五个生产提交。
- 新增 `start/status/pause/cancel/history` 公开桌面契约，必须复核跨重启语义。

### 完成证据

- 独立 Runner 直接装配有界 Auto Host Loop；Supervisor 保证单 Session 唯一、控制传播、单调进度与终态释放。
- Exit 与 Safe Mode 使用统一有界排空；超时分别隔离为 `shutdown-timeout` 和 `safe-mode-timeout`，迟到 Runner 不能覆盖终态。
- 正常启动把崩溃遗留 Running Session 原子转为 `paused/restarted`，Active Attempt 转为 `indeterminate`；恢复投影不占用 Session、不创建线程且不触发 Provider/Tool。
- 专用可恢复查询不会被普通历史挤出；超过 256 条时失败关闭，不静默漏掉未决 Attempt。
- 真实 SQLite 跨桌面状态重启测试、仓储测试、Supervisor 测试、严格 Clippy、TypeScript 与生产构建构成当前证据链。

### 偏差与纠正

- 原入口文件承载约 300 行 Runner 编排，已提取为显式依赖的独立模块并删除临时超长函数豁免。
- Safe Mode 最初只取消 Provider、Skill 与用户程序，遗漏后台 Auto Job；现已纳入统一排空协议。
- 最初恢复方案按最近 256 条普通历史扫描，可能漏掉更老 Attempt；已改为专用可恢复集合并检测溢出。
- Job 投影不是事实源，不能反向覆盖 Session/Checkpoint/Attempt，也不能被描述为自动续跑。

### 安全、离线与稳定性

- 恢复完全读取本地 SQLite，不需要网络；任何未证明的外部结果永久进入人工对账状态。
- Recovery Mode 使用空 Supervisor 并拒绝新 Job；数据库异常不会降级为内存自动执行。
- 退出等待有界，不因 Provider 卡死永久阻塞桌面关闭。

### UI 与无障碍

- 宿主历史契约已具备，但 Goal/Plan/Attempt/Job Control Center 尚未完成，`indeterminate` 也没有可操作的人工对账界面。
- 浏览器预览继续拒绝伪造宿主 Job；真实交互、键盘路径、读屏状态播报和桌面截图仍需后续纵切证明。

### Actions 分钟

- 本里程碑继续本地全量验证后单次推送，不手动触发 macOS/Windows 矩阵。
- `GltfRenderer` 大块警告与本纵切无关，登记后独立处理，避免混合提交和重复 CI。

### 下一纵切硬验收

1. 为 `indeterminate` Attempt 提供只读详情、明确风险解释与参数绑定的人工对账命令。
2. 对账只能选择“确认未执行后重试”或“确认已执行并接受结果”等受约束决议，禁止删除证据后直接续跑。
3. 决议必须原子更新 Attempt、Session、Task 与 Checkpoint，并保留不可变审计事件。
4. Goal/Plan/Attempt/Job 控制中心展示恢复历史、暂停原因、预算、Checkpoint 和下一安全动作。
5. 浏览器完成视觉预览，真实 Tauri 完成宿主交互与跨重启端到端验证。

## 4. M-2026-07-18 未知执行结果人工对账

### 完成证据

- 新增不可变 `auto_mode_attempt_resolution` 事实表与单一原子用例；Session、Task、Checkpoint、Attempt 和 Resolution 在同一 Immediate 事务内收敛。
- 决议严格限定为 `confirmed_not_executed` 与 `accept_external_effect_and_cancel`，不伪造 Provider 成功、不删除证据后续跑、不在决议命令内自动重试。
- 请求绑定 Session、Attempt、Checkpoint sequence、request fingerprint、actor、reason 与宿主时间；陈旧、重放和非 `indeterminate` 状态失败关闭。
- 桌面 IPC 提供详情、风险说明、下一安全动作、100 条有界审计查询与决议入口；浏览器预览稳定返回 `desktop-host-required`。
- Workspace Clippy、Workspace Rust tests、桌面 85 tests、持久化 52 tests、Vitest 31 tests、TypeScript 与生产构建全部通过。

### 架构复核与纠正

- 对账属于持久化应用用例，不放在 UI 或 Supervisor；未来桌面、CLI 与恢复向导复用同一事实契约。
- Agent Runtime 保持纯领域化，未新增“未知成功”伪状态；宿主仍负责时间和数据库路径。
- Resolution 是审计事实，Job Snapshot 仍只是投影。后续控制中心应由 Session/Checkpoint/Attempt/Resolution 组合查询生成恢复视图。
- 已补真实 SQLite 专属夹具：两种决议、陈旧绑定零写入、非未知拒绝、重放、双连接竞争、跨 reopen、分页边界及索引/载荷分叉均有直接证据。

### 下一纵切硬验收

1. 实现 Goal/Plan/Attempt/Job 控制中心，把风险、预算、Checkpoint、不可变历史和安全下一步做成可访问 UI。
2. 用浏览器完成响应式、键盘、读屏与视觉截图审查，并在真实 Tauri 完成宿主交互验证。
3. 独立处理 `GltfRenderer` 约 615 KB 的按需加载与性能预算，不混入 Agent 安全提交。

## 5. 后续回顾模板

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
