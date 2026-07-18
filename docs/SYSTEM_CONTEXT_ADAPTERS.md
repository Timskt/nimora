# Nimora 系统情境感知与桌宠呈现策略

> 状态：领域策略、Desktop Coordinator 与用户覆盖已实现；原生 Adapter 尚未实现
> 权威契约：`crates/system-context`  
> 原则：Sensor 只提交事实，Policy 只产生意图，Desktop Host 才能操作窗口

Sensor 的宿主无关调度与健康契约位于 `crates/system-context-sensor`。默认每 5 秒采样、2 秒超时、15 秒信号续租，失败采用最高 30 秒的指数退避；停止后不再调度。健康快照只暴露稳定错误码、失败次数和时间，不包含窗口或应用内容。

## 1. 目标

Nimora 在全屏演示、屏幕共享、游戏和系统免打扰期间自动降低干扰，同时保留用户明确控制、Safe Mode 恢复和完全离线运行。环境感知不是 AI 能力，不使用 Provider、网络、屏幕截图、OCR 或用户内容。

## 2. 分层边界

1. **Platform Sensor Adapter** 读取操作系统公开状态，只输出 `ContextSignal`。
2. **System Context Policy** 合并来源、淘汰过期值、执行优先级并输出 `PresenceDecision`。
3. **Desktop Coordinator** 把决策合并到活动 Profile 的 `WindowPolicy`，通过现有可逆事务应用窗口与自主行为。
4. **Renderer** 只展示当前结果和恢复入口，不读取系统 API，也不自行隐藏窗口。

Sensor 不得持有 `AppHandle`、WebView、Pet Repository 或 Profile Service；Desktop Host 不得根据进程名、窗口标题或模型猜测敏感状态。

## 3. 稳定信号

`nimora.system-context/1` 只允许：

- `do_not_disturb`：系统明确启用专注或免打扰。
- `fullscreen`：其他前台应用占用当前宠物所在显示器的工作区。
- `game`：操作系统或获准平台集成明确报告游戏会话；禁止仅凭可执行文件名猜测。
- `screen_share`：操作系统或获准捕获框架明确报告当前屏幕/窗口正在共享。

每条信号绑定 `kind/source/active/observedAt/expiresAt`，最长有效期 30 秒。Adapter 必须持续续租；崩溃、权限撤销、系统休眠或 IPC 中断会自然过期，不得留下永久隐藏状态。相同来源和类型的时间戳禁止倒退。

## 4. 优先级与覆盖

1. Safe Mode 始终强制宠物可见、可交互，确保恢复路径存在。
2. 用户“始终隐藏”优先于普通系统情境。
3. 屏幕共享是隐私边界，即使用户普通“始终显示”也先隐藏；用户必须在共享状态提示中单独执行一次明确恢复，不能沿用历史覆盖。
4. 用户“始终显示”可覆盖游戏、全屏和免打扰。
5. 自动模式按 `screen_share > game > fullscreen > do_not_disturb > Profile` 决策。

用户覆盖分为当前会话和持久偏好。会话覆盖在退出、Safe Mode、Profile 切换或系统共享边界变化时失效；持久偏好不得绕过共享隐私门禁。

## 5. 平台 Adapter

### macOS

- 全屏：使用受支持的 Workspace/Window Server 状态判断当前显示器是否被其他应用全屏占用；不读取窗口标题。
- 屏幕共享：优先使用 ScreenCaptureKit/系统共享会话状态；若系统版本不提供可靠状态，则显示“无法自动检测”，不得假装安全。
- 免打扰：使用系统公开 Focus 状态或用户显式授权的自动化集成；权限缺失时降级为未知。
- 游戏：仅接受 Game Mode 或获准平台 Integration 的明确状态；不维护游戏进程黑名单。

### Windows

- 全屏：使用 Shell/窗口管理 API 比较前台窗口与所在显示器工作区，排除桌面、锁屏和 Nimora 自身窗口。
- 屏幕共享：使用 Windows Graphics Capture/系统共享指示器或获准会议应用 Integration；无法证明时返回未知。
- 免打扰：使用 Focus Assist/Do Not Disturb 公共状态；系统 API 不可用时返回未知。
- 游戏：使用 Windows Game Mode 或获准平台 API；不扫描用户进程列表上传或持久化。

Linux 后续 Adapter 必须遵循相同契约，并分别处理 Wayland Portal 与 X11；不得为追求“自动”绕过桌面安全模型。

## 6. 稳定性

- Sensor 运行在独立、可取消的宿主任务；单次采样有超时，错误采用有界退避。
- 每个 Adapter 必须复用 `SensorController`，不得自行定义无限重试、永久租约或不可观察的后台循环。
- 多来源只在领域层合并；Adapter 故障不能直接调用 `hide()` 或覆盖 Profile。
- 系统睡眠后旧信号全部过期，唤醒必须重新采样。
- 策略变化必须走现有 `run_window_policy_transition`；原生应用失败时回滚窗口，领域状态不得伪装成已应用。
- 隐藏时托盘“恢复互动”和 Safe Mode 始终可用；恢复后重新评估而不是永久关闭 Sensor。

## 7. 隐私与诊断

允许记录：信号类型、来源类别、布尔状态、时间、稳定错误码和策略原因。禁止记录窗口标题、应用文档名、会议名称、联系人、屏幕像素、音频、进程命令行或捕获内容。诊断导出沿用现有脱敏策略且默认不包含详细事件。

## 8. 验收证据

- 领域：优先级、过期、倒序拒绝、Safe Mode、用户覆盖和共享隐私测试。
- 桌面：Profile 基础策略与情境决策合并、原生失败回滚、托盘恢复、休眠过期测试。
- macOS/Windows 真机：双屏全屏、开始/停止共享、免打扰切换、游戏状态、权限拒绝、Adapter 崩溃和系统唤醒。
- 长稳：8 小时采样无任务泄漏、CPU 空转、窗口闪烁或事件风暴。

当前 Desktop Coordinator 已把活动 Profile、用户覆盖、Safe Mode 和情境决策统一合并到可逆窗口事务；控制中心提供“自动避让 / 始终显示 / 始终隐藏”，托盘恢复也不能绕过屏幕共享隐私。原生 Adapter 尚未实现，因此不得宣称 macOS/Windows 自动感知完成。
