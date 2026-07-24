# Nimora 桌面生命体情境与运动规划

> 状态：纯库 + Host 已接线（transparent AOT overlay stage、逻辑宠物体、采样、遮挡事件、lifeform 自主意图、弹簧逻辑位姿、Agent/Auto Mode/Skill/OS/Connector directive 路径、Control Center LifeformOverview + `desktop_snapshot.lifeformSense` 隐私聚合）；**原生视觉 QA / 签名包 / idle CPU 未证明**  
> 日期：2026-07-24（诚实性同步：petization factories + stage/body 260×300）  
> 相关：[`DESKTOP_PET_EXPERIENCE.md`](DESKTOP_PET_EXPERIENCE.md)、[`SYSTEM_CONTEXT_ADAPTERS.md`](SYSTEM_CONTEXT_ADAPTERS.md)、[`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md)

## Recent deltas（2026-07-24）

- **lifeformPerf 产品化**：`BuiltinPet3D` 帧采样 + `nimora:lifeform-perf`；Control Center **渲染预算** HUD
- **processBudget IPC**：`DesktopSnapshot` 挂载 Host RSS/CPU 预算；macOS `task_info`、Linux `/proc`、Windows fail-closed stub
- **notification unread sensory**：outbox/approvals 计数边沿 → `notification_sensory_directive`；`LifeformSense.notification_unread`（无标题/正文）
- **Grant at-rest crypto**：`AuthorizationGrantKey` + `nimora.encrypted-authorization-grant/1`（XChaCha20-Poly1305）；legacy dual-read；写路径加密
- **跨屏抛光**：五档 park slot；每隔一次 wander 优先跨屏；短距可选 Jump
- **Q-minion 抛光**：无方框、ellipse hit-area、更活 idle、双层接触阴影
- **仍未证明**：原生视觉 QA / 签名 Windows 包 / 空闲 CPU 真机达标 — **不得 claim goal complete**

## 1. 目标

把“桌面生命体”从聊天窗口错觉推进为可离线、可验证、可 fail-closed 的主体：

1. **观察桌面**而不读取用户内容；
2. **规划位姿与运动**并用弹簧-阻尼（或跳跃）轨迹驱动**逻辑宠物体**位姿（stage 为 AOT 透明 overlay，body ≠ OS 窗口），而非一次性瞬移；
3. **发出有界结构化动作**（宠物为 Subject of action），而不是只吐自由文本。

Presence 降扰（全屏 / DND / 游戏 / 投屏）仍以 `system-context` 为准，职责不得与本栈混用。

## 2. 分层

```text
Platform Sampler (macOS / Windows)
        │ EnvironmentSample (facts; titles cleared)
        ▼
Desktop Host (apps/desktop/src-tauri)
        │ sample_lifeform_environment ~5s, 1.5s timeout
        │ map → DesktopSnapshot → lifeform_env
        ▼
nimora-desktop-context (pure)
        │ freshness · obstacles · plan_wander · spring-damper
        ▼
Host Free-surface wander
        │ plan_lifeform_wander_goal → spring_position_frames → set_position
        ▼
Renderer (WebView) · logical .pet-subject on AOT stage · BuiltinPet3D / fox · contact shadow
```

| 层 | 位置 | 职责 | 禁止 |
| --- | --- | --- | --- |
| Snapshot + motion pure | `crates/desktop-context` | 版本化快照、新鲜度、障碍、`plan_wander`、弹簧积分 | 读 OS、持窗口句柄、网络、AI |
| macOS sampler | `crates/desktop-context-macos` | 有界超时：窗口几何 / idle / 会议进程名 / IOKit 电源 | 存标题正文、读像素、OCR |
| Windows sampler | `crates/desktop-context-windows` | `cfg(windows)` Win32 样本；非 Windows → `AdapterUnavailable` | 同上；≠ `system-context-windows` |
| Behavior pure | `crates/runtime-core` `behavior` | `PersonalityProfile`、`StructuredPetDirective`、`select_autonomous_intent` | 直接驱动原生窗口 |
| Host mapping + motion | `desktop_lifeform` + `lib.rs` | 采样、映射、缓存、`spring_position_frames` | 把规划下放到 Renderer/AI |
| Presence | `system-context` | 显隐 / 降扰 | 几何漫游 |

说明：生效路径为 `apps/desktop/src-tauri/src/desktop_lifeform.rs`（`mod desktop_lifeform`）。

## 3. `nimora-desktop-context`

### 3.1 Snapshot

- Spec：`nimora.desktop-context/1`
- 字段：`windows`、`foreground`、`displays`、`power`、`idle_ms`、`meeting`、`cursor`、`observed_at_ms`、`expires_at_ms`、`freshness`、`degradation_reason`
- 最大租约：`MAX_SNAPSHOT_LIFETIME_MS = 30_000`（Host 成功样本常用 ~10s 软租约）
- `Fresh` / `Stale` / `Degraded`；仅 Fresh 且未过期时 `obstacles_usable`
- 陈旧或降级：`plan_wander_from_snapshot` fail-closed 到无障碍自由漫游

### 3.2 Spring-damper

- `SpringParams` 默认 stiffness/damping/mass = 48 / 14 / 1
- `integrate` / `integrate_to_target`：固定步长，**非**位置 lerp
- 辅助：`bounce_on_bounds`、`jump_parabola`、`squash_stretch_scale`

### 3.3 Obstacle + plan_wander

- 规划几何避障，**不是**真实窗口像素遮挡渲染精修；overlay stage 架构另见 §Fullscreen Overlay Stage

## 4. 平台采样

### 4.1 macOS

- `sample(timeout)`：worker + 墙钟超时；零超时 / 权限 / 列表不可用 → 稳定错误
- 窗口：CGWindowList；过滤 shell/零尺寸；**默认不存标题**
- idle：系统空闲毫秒；失败可退化
- meeting：进程/应用名子串（Zoom / Teams / Meet / Webex…），无内容检查
- power：IOKit `IOPSCopyPowerSources*`；失败 → `PowerFact::unavailable()`（fail-closed，**不是**“永远不可用”）

### 4.2 Windows

- `cfg(windows)`：窗口枚举 / idle / 电源 / 会议名启发；过滤 shell、过小、本进程窗口；清标题
- 非 Windows 构建：`AdapterUnavailable`
- 与 Presence 用的 `system-context-windows` **并行存在**，契约不同

## 5. Desktop Host 接线状态（2026-07-24）

**已接线**

- `Cargo.toml` path 依赖 `nimora-desktop-context` + 平台采样 crate
- `mod desktop_lifeform`；状态机 `lifeform_env: Mutex<Option<DesktopSnapshot>>`
- 主循环约每 5s、`sample(..., 1.5s)`；失败 → `degraded_lifeform_snapshot`
- **Transparent AOT overlay stage** = 当前显示器 work area；逻辑宠物体 **260×300**（body ≠ OS window）
- 漫游：`plan_lifeform_wander_goal` → `spring_position_frames` → **逻辑位姿 emit**（`pet-position-changed`）；多屏 **mid-walk** stage rebind
- 帧循环尊重 Reduced Motion、拖拽、Safe Mode、Walking 状态
- 会议 `active` 时会写 `presence_decision.suppress_autonomy = true`（粗粒度；非完整 Presence 信号模型）
- 自主 Tick：`tick_autonomy_with_lifeform` + `DesktopBehaviorHints`；IPC `apply_pet_directive` + `nimora://pet-directive-changed`
- Subject 产品路径：Agent Workspace / Auto Mode `companion_directive`（work/wait/complete/fail/cancel）/ Skill worker busy-done / OS sensory offline·degraded·restored / connector `EventReceived`（`connector.*`，**8s 节流**，automation/skill event session + admitted path when `AppHandle` present）
- Control Center：`LifeformOverview`（lazy BuiltinPet3D、vitals、personality、directive 摘要、中文 attention 标签；soft WebGL fallback）
- **`desktop_snapshot.lifeformSense`（隐私聚合 IPC）**：Host 从 `lifeform_env` 投影有界 OS 感官摘要供 Control Center / FE 使用；**不**下发窗口标题、前景 app 正文或像素。字段：`batteryPercent` / `onBattery` / `charging`；`idleMs`；`meetingActive` / `meetingHint`（仅 `zoom|teams|meet|webex|unknown`）；`displayCount`；`notification_unread`（计数边沿，无标题正文）；`freshness` / `degradationReason`；`observedAtMs` / `expiresAtMs`。并列 **`processBudget`**（RSS/CPU 近似；平台见 Recent deltas）。FE：`LifeformSenseSnapshot` + 渲染预算 / `hostProcessBudgetFromSense`（`App.tsx` 绑定 `desktopSnapshot`）
- Schema：`personality` / `lastDirective*` / `lastAttention` / `directiveRevision`；preview snapshot 已 enrich
- 本地门禁：`desktop_lifeform` **11** tests green（FE related ~75、`tsc` clean — 见 status 文档）

**未完成 / 边界**

- 边缘 Surface（Perch/Climb/Peek 等）目标仍走 `plan_surface_wander_target`，再套弹簧帧
- `current_lifeform_displays`：**DONE** — `available_monitors` 枚举全部显示器（几何 + work area + scale）；供 stage rebind / 跨屏 wander
- 跨屏“走过去”体验打磨、遮挡像素级合成精修
- **原生视觉 QA**、**signed Windows package**、**idle CPU budget measurement**：**未证明** — 不得声称 lifeform goal complete
- 性能预算：FE 帧采样 + 渲染预算 HUD + Host `processBudget` IPC **已产品化**；真机 idle CPU/RSS **达标证明**与 Windows 真采样仍开放

## 6. runtime-core 生命体指令

| 类型 | 作用 | Host 使用 |
| --- | --- | --- |
| `PersonalityProfile` | 0–100 四轴；挂 `Pet` 可持久化 | Schema + Control Center 展示；偏置规划产品化仍开放 |
| `DesktopBehaviorHints` | crowding / idle / battery / meeting / suppress | Host Tick / sensory 输入 |
| `select_autonomous_intent` | 确定性离线意图 | 经 `tick_autonomy_with_lifeform` 接入 |
| `StructuredPetDirective` | `nimora.pet_directive/1` + 动画白名单 | Agent / Auto Mode / Skill / OS / Connector → `apply_pet_directive`；FE 类型已对齐；原生视觉闭环未验收 |
| Schema extras | `lastDirective*` / `lastAttention` / `directiveRevision` | 快照 + preview enrich；revision 供 UI 一致性 |

### 6.1 Domain petization factories（`crates/runtime-core` behavior）

权威工厂（mood 为 `i8`，非 `Option`；输出均为有界 `StructuredPetDirective`）：

| Factory | 输入 | 典型 Host 接线 |
| --- | --- | --- |
| `agent_status_directive` | `AgentCompanionPhase` | Agent Workspace / `companion_phase_directive` 委托同 token |
| `auto_mode_directive` | `AutoModePetEvent` | Auto Mode job 阶段（start/step/pause/budget/done/crash） |
| `user_code_directive` | `UserCodePhase` | User Program 沙箱运行 / 批准 / 拒绝 / 崩溃 |
| `automation_directive` | `AutomationPhase` | Automation 触发 / 成功 / 失败 / 节流 |
| `battery_directive` | `level_pct` + `charging` | OS sensory 电量阈值节流 |
| `idle_user_directive` | `idle_secs` | OS sensory 空闲阈值节流 |
| `meeting_sensory_directive` | `active` + privacy `hint` | 会议安静陪同 / 结束后 soft observe |
| `grant_directive` | `GrantPetEvent` | Grant 签发 / 撤销 / 过期 / FullDevice 警告表情 |

Host 侧仍保留/并用：`connector_sensory_directive`（连接器相位）、skill worker done/busy 路径、`companion_directive`（与 FE `agentCompanionDirective` 同 token）。Unattended issue/revoke 经 `companion_directive::apply_grant_event` → domain `grant_directive`（Host 可按档位润色 speech）。**权限与展示解耦**：unattended Grant 决定能否 sleep-safe 推进；宠物化只表达状态。

#### Meeting sensory host edge-trigger

| 层 | 行为 |
| --- | --- |
| Domain | `meeting_sensory_directive(active, hint)` — active → `Rest` + 中文安静陪同（Zoom/Teams/Meet/Webex/unknown/generic）；cleared → `Observe`「会议结束了，我回来啦」；**不**接受标题/正文 |
| Pure gate | `nimora-desktop-context::meeting_should_emit(prev, next)` 仅在 `active` 翻转时为 true（同态不发） |
| Host | `publish_lifeform_meeting_sensory` 在 ~5s OS 采样后：比较上次相位；**首次健康 inactive 保持安静**（不冷启动刷「结束」）；仅 inactive→active / active→inactive 调用 domain factory → `apply_lifeform_directive_from_host` |
| 自主 | `DesktopBehaviorHints.meeting_active` 另硬门 `select_autonomous_intent` → quiet rest（与 sensory speech 互补，不是重复触发） |

#### Position debounce persistence（逻辑宠）

| 路径 | 预期 |
| --- | --- |
| `move_pet` / `finish_pet_drag` | 更新 Runtime `position` + emit 逻辑位姿后调用 `schedule_position_persistence` |
| Debounce | `POSITION_WRITE_DEBOUNCE` ≈ 200ms；revision 门闩：仅最新 revision 且**非拖拽中**才写盘（`PositionPersistenceMode::DebouncedMove`） |
| Overlay | 源真相是 logical pet `position`，**不是** stage OS window outer origin |
| 原生 | 手感/磁盘时序属 Host 集成；纯域测试不要求 native 窗口 |

### 6.2 Stage / body 几何（防回归）

| 概念 | 值 | 说明 |
| --- | --- | --- |
| Overlay stage | 当前显示器 **work area** | 透明 AOT WebView；**不是**宠物体本身 |
| Logical body | 固定 **260×300** | `.pet-subject`；`Pet.position` = 屏幕物理 top-left |
| Local CSS | `--pet-local-x/y` | `local = position - stage.origin` |
| 运动 | spring-damper / jump | **emit 逻辑位姿**；禁止把 body 当 OS 窗口 `set_position` 滑移 |

## 7. Frontend 观感

- **BuiltinPet3D secondary motion**：`springToward` 弹簧-阻尼（非 lerp）驱动姿态通道 — **DONE**
- **Unattended / grant UX**：AgentWorkspace 档位、危险确认、badge、list/revoke — **DONE**（权限执行见 `AUTO_AUTHORIZATION.md`）

- **Stage ≠ body**：透明 AOT overlay stage；逻辑 `.pet-subject` **260×300** 经 `--pet-local-x/y` 放置
- `BuiltinPet3D`：Q-minion 暖黄；idle micro-acts `yawn` / `digNose` / `countAnts`；soft WebGL fallback
- Fox GLB 路径仍保留：`frameGroundedModel` + `cameraDistanceForGroundedPet` + 接触阴影
- 透明 chrome：pet 路由 / `.builtin-pet`；原生 `decorations(false)` / `transparent(true)` / `shadow(false)` / `skip_taskbar(true)`
- Control Center `LifeformOverview`：3D 预览 + vitals + personality + directive 摘要 + 中文 attention 标签
- `applyPetDirective` FE 类型与 `nimora.pet_directive/1` 对齐

## 8. 与 `system-context` 边界

| | desktop-context / lifeform | system-context |
| --- | --- | --- |
| 输入 | 窗口几何、idle、会议启发、电源 | fullscreen / DND / game / screen_share |
| 输出 | 漫游目标 + 弹簧帧 | PresenceDecision → 显隐 |
| 隐私 | 无标题正文、无像素 | 无截图 OCR |

会议启发当前可粗暴抑制自主，**不得**替代正式 Presence 信号与用户覆盖语义。

## 9. 验收原则

- 库测试 + Host 接线 + local gates（`desktop_lifeform` 11 / FE ~75 / tsc） **≠** 桌面生命体体验完成
- Browser Preview 不能证明原生透明、多屏 Work Area、弹簧手感、点击穿透
- 电源 fail-closed 不得写成“无电源 API”或“已完成电池策略产品化”
- Windows 路径存在，真机样本质量与权限矩阵仍须签名包验收
- **禁止**在未完成原生视觉 QA / signed package / idle CPU 测量前宣称 lifeform goal complete 或 production-ready


## Host integration status (updated)

- Module: `apps/desktop/src-tauri/src/desktop_lifeform.rs` (registered in `lib.rs`)
- Cargo deps: `nimora-desktop-context` + platform adapters
- Motion: spring-damper only → logical body pose on AOT stage (Walk/Perch spring-damper, Jump parabola); body is **not** the OS window
- Sensing: platform `sample` every ~5s → `lifeform_env`; OS sensory + connector `EventReceived` (8s throttle)
- Fail-closed: sample errors → `Freshness::Degraded` snapshot; obstacles not used when unusable
- Directives: Agent / Auto Mode `companion_directive` / Skill worker / OS / connector → structured pet directives
- Remaining unproven: native visual QA, signed Windows package, idle CPU **measurement under budget** (instrumentation/HUD landed ≠ proven), multi-monitor **hand-feel** polish, occlusion pixel refine; Windows processBudget real sampler still stub


## Fullscreen Overlay Stage（已实现）

| 概念 | 含义 |
| --- | --- |
| Stage window | 透明、无边框、Always-on-Top WebView，尺寸 = 当前显示器 **work area** |
| Logical body | 固定 **260×300**；`Pet.position` = 屏幕物理 top-left |
| Local CSS | `local = position - stage.origin` → `--pet-local-x/y` on `.pet-subject` |
| Drag | 逻辑拖拽（pointer → `move_pet`），**不是** OS `startDragging` |
| Hit-test | 默认 `ignore_cursor_events`；光标在宠物体上或拖拽中才可交互 |
| Events | `pet-position-changed` · `pet-stage-changed` · `pet-occlusion-changed` · `pet-directive-changed` |

运动路径：`plan_lifeform_wander_goal` → `spring_position_frames` → **emit logical pose**（禁止把 body 当 OS 窗口 `set_position` 移动）。

### Occlusion

- 纯函数：`compute_pet_occlusion` / `occluders_from_snapshot` / `pet_occlusion_for_pose`（不可用样本 fail-closed 为空）
- Host：`sample_lifeform_environment` 后 `publish_lifeform_occlusion`
- 前端：`occlusionClipPath` 隐藏被盖区域；coverage > 0.85 静音 ambient bubble

### Multi-monitor

- `overlay_stage_for_pet` 按宠物体中心选择 display work area
- Host 在采样 / move / wander 结束 rebind stage 并 `emit_pet_stage`

### Behavior Subject

- `select_autonomous_intent` + `tick_autonomy_with_lifeform(DesktopBehaviorHints)`
- `apply_pet_directive` → 结构化 speech/mood/action/animation → UI bubble

## 2026-07-24 integration notes

### Subject product paths
| Source | Host path | Pet effect |
|---|---|---|
| Agent Workspace | `applyPetDirective(agentCompanionDirective)` | work / perch / celebrate / rest |
| Auto Mode job | `companion_directive` + `apply_lifeform_directive_from_host` | same tokens, phase de-duped；unattended 启动可附带 Grant（权限与展示解耦） |
| Skill execute | busy → done/fail; approval → wait perch | sweat / celebrate / crash / perch |
| OS sample | `publish_lifeform_sensory_from_snapshot` → battery/idle 阈值节流 + **meeting edge** → `meeting_sensory_directive` | 电量/空闲 + 会议安静陪同（同态不刷） |
| Connector | `connector.*` → `EventReceived` + `connector_sensory_directive` | petized notice, **8s throttle** |
| User Program | `user_code_directive` | sandbox / approve / deny / crash |
| Grant lifecycle | `grant_directive` | issued / revoked / expired / full_device warning |
| Autonomy tick | `tick_autonomy_with_lifeform` + stamp | offline Chinese speech + animation |

### Control Center
`LifeformOverview` is the authoritative companion card for 概览 — lazy `BuiltinPet3D`, vitals, personality, directive summary, Chinese attention labels, overlay stage summary; soft WebGL fallback. Not a decorative ear mock.

**`lifeformSense` IPC（隐私安全聚合 / privacy-preserving aggregates）**

| 层 | 行为 |
| --- | --- |
| Host | `desktop_snapshot` 读取 `lifeform_env`，经 `lifeform_sense_from_env` 投影 `LifeformSenseSnapshot`；采样缺失时为 `null`/`None` |
| 字段 | 电量：`batteryPercent` / `onBattery` / `charging`；空闲：`idleMs`；会议：`meetingActive` + `meetingHint`（**仅** `zoom` / `teams` / `meet` / `webex` / `unknown` — **无标题**）；`displayCount`；`freshness` / `degradationReason`；`observedAtMs` / `expiresAtMs` |
| FE | `apps/desktop/src/platform/desktop.ts` 类型；`buildLifeformSenseHintsFromSnapshot` + `applyLifeformSenseSnapshot` → 感官卡片；`App.tsx`：`senseHints={buildLifeformSenseHintsFromSnapshot(desktopSnapshot)}` |
| 隐私 | 序列化结果无 `windows` / `foreground` / 标题字段；会议 hint 为封闭枚举标签，不是窗口名 |
| 测试 | Host：`lifeform_sense_from_env_projects_privacy_safe_aggregates`；FE：投影电量/空闲/会议/多屏 |

English: Control Center consumes **bounded OS aggregates only** via `desktop_snapshot.lifeformSense`. Never surface window titles, foreground text, or screen pixels through this path.

### Schema + preview
Pet schema optional: `personality`, `lastDirective*`, `lastAttention`, `directiveRevision`. Preview snapshot enriched for Control Center / FE consumers.

### Renderer idle micro-acts
`BuiltinPet3D`: `yawn`, `digNose`, `countAnts`; Q-minion warm yellow.

### Non-goals / unproven
Native visual QA, signed Windows package, and idle CPU budget measurement remain **open**. Subject product paths are wired in code; do **not** claim lifeform goal complete or production-ready visual proof.


## 2026-07-24 sensory path honesty (OS → directive)

**Implemented in pure crates / host helpers (this pass):**

- Stable sensory bands + throttle: battery / idle rise-only / meeting edge (`nimora-desktop-context::sensory`, re-exported as `lifeform_*` in `desktop_lifeform`)
- Presence boolean hold gate + sample cadence helpers (`system_context_sensor::PresenceBooleanGate`) — pure; host loop wiring in `lib.rs` still owns when to call sample APIs
- Multi-monitor work-area stages, union bounds, `plan_cross_display_target` / `plan_cross_display_walk_frames` (real planner + multi-display spring bounds)
- Occlusion: `occluders_from_snapshot` (+ optional DPI path), window sanitize caps (64), z-sort, fullscreen candidates, minimized drop (macOS/Windows filters)
- Meeting process-name hygiene: Teams `ms-teams`, safer Meet markers, FaceTime/Discord → Unknown soft signal
- Behavior factory: `meeting_sensory_directive` re-exported from `nimora-runtime-core` root; Host `publish_lifeform_meeting_sensory` **edge-triggers** on OS sample (~5s): first inactive sample quiet, only active-flag flips emit Zoom/Teams/Meet/Webex/unknown Chinese quiet-rest or cleared observe (see §6.1 Meeting sensory host edge-trigger)

**Still unproven on real OS (do not claim complete):**

- Native visual QA / transparent stage feel across displays
- Mixed-DPI multi-monitor occlusion pixel accuracy
- Signed Windows package + real permission matrices (Screen Recording / Accessibility)
- Idle CPU budget of the ~5s sample loop
- Cross-display walk “feel” polish
- Host consolidating its inlined band/throttle copies onto the pure helpers (read-only for this task)

