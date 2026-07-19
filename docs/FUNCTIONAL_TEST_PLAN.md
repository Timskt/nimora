# Nimora 功能测试计划

## DP-TRAY-RECALL — 托盘恢复与离屏召回

| ID | 场景 | 预期结果 |
|---|---|---|
| DP-TRAY-RECALL-001 | 宠物可见且在有效 Work Area 内 | 恢复交互只取消鼠标穿透并解除最小化，位置不抖动，`positionRecovered=false` |
| DP-TRAY-RECALL-002 | 宠物完全位于已断开的显示器 | 夹回主显示器安全边界，窗口完整可见，`positionRecovered=true` |
| DP-TRAY-RECALL-003 | 宠物跨越两个仍在线显示器 | 选择重叠面积最大的 Work Area，不无故跳回主屏 |
| DP-TRAY-RECALL-004 | 分辨率或 Dock/任务栏 Work Area 缩小 | 保留尽可能接近的原位置并夹入新边界，不覆盖系统栏 |
| DP-TRAY-RECALL-005 | 负坐标副屏 | 保留合法负坐标，不把负值误判为离屏或强制迁回主屏 |
| DP-TRAY-RECALL-006 | 位置发生纠正 | Moved 事件防抖后把最终原生坐标写回 Runtime，重启不再次恢复旧离屏坐标 |
| DP-TRAY-RECALL-007 | 显示器枚举或原生 set_position 失败 | 发布 `desktop.tray.action-failed`，不伪造召回成功，不访问网络或等待 AI |
| DP-TRAY-RECALL-008 | Presence 原本隐藏宠物 | 托盘显式恢复使用既有 ForceVisible 决策；屏幕共享隐私等更高优先级规则仍按统一协调器裁决 |
| DP-TRAY-RECALL-009 | Renderer、Agent、Skill 或用户代码尝试召回 | 无原生窗口句柄；只能通过受控 Capability 请求，不能绕过 Desktop Coordinator |
| DP-TRAY-RECALL-010 | macOS/Windows 热插拔、混合 DPI 与自动隐藏系统栏 | 使用签名 Tauri 包检查窗口完整可见、无跳屏和坐标持久一致，Browser Preview 不得替代证据 |
| DP-TRAY-RECALL-011 | Pet WebView 已销毁或崩溃 | 一次托盘操作写入 ForceVisible 与取消穿透意图，发布恢复请求并启动唯一有界 Recovery Worker，不要求用户重复点击 |
| DP-TRAY-RECALL-012 | 缺失窗口的持久坐标已离屏 | 重建后、启用交互前先夹回有效 Work Area；不得出现“窗口已恢复但仍找不到宠物” |
| DP-TRAY-RECALL-013 | 重建后 Work Area 复验失败 | 销毁半构建窗口并进入下一次有界退避；不得留下无心跳的幽灵窗口或误报恢复完成 |
| DP-TRAY-RECALL-014 | Shutdown 与托盘恢复并发 | Lifecycle Gate 或 Recovery Host 永久拒绝新 Worker，明确退出后宠物不得复活 |
| DP-TRAY-RECALL-015 | 初始坐标、Work Area 或点击穿透配置失败 | build 后任一失败都销毁半成品窗口且不记录心跳；Worker 可继续下一次有界重试 |
| DP-TRAY-RECALL-016 | 所有原生初始化步骤成功 | 保留唯一 Pet Window、记录新心跳且不执行清理，不产生重复窗口 |

## ACTIVITY-REFRESH — 活动健康自动刷新

| ID | 场景 | 预期结果 |
|---|---|---|
| ACTIVITY-REFRESH-001 | 首次进入活动工作区 | 立即读取一次权威本地 Outbox Snapshot，不等待首个定时周期 |
| ACTIVITY-REFRESH-002 | 活动页保持前台可见 | 每 10 秒最多发起一次刷新，计数由最新成功快照更新 |
| ACTIVITY-REFRESH-003 | 文档进入 hidden 状态 | 周期触发不读取数据库、不发起网络请求、不修改现有结果 |
| ACTIVITY-REFRESH-004 | hidden 恢复 visible | 立即刷新，不额外等待 10 秒 |
| ACTIVITY-REFRESH-005 | 离开活动工作区 | 清理 interval 与 visibilitychange listener，后台不继续刷新 |
| ACTIVITY-REFRESH-006 | 上一次读取超过 10 秒 | 单次飞行保护拒绝重入，不堆积并发读取 |
| ACTIVITY-REFRESH-007 | Outbox Snapshot 读取失败 | 保留上次成功计数并展示降级提示，不把失败伪装成全零 |
| ACTIVITY-REFRESH-008 | 完全离线且无 AI Provider | 刷新完整可用，不访问网络、不启动 Agent |
| ACTIVITY-REFRESH-009 | 活动页隐私检查 | 只显示有界计数和固定健康描述，不显示事件正文、Prompt、路径或桌面内容 |
| ACTIVITY-REFRESH-010 | 810px 与窄宽度布局 | 刷新说明可读、摘要不横向溢出、当前导航通过语义状态暴露 |

## ACCESSIBILITY-MOTION-001 跨层减少动态效果

- 启动时系统偏好为 Reduce，确认 Pet Overlay 立即上报 `set_reduced_motion(true)`；Sprite、glTF/VRM、气泡、菜单及控制中心非必要动画停止，原生自主漫游不设置窗口位置。
- 在 12 帧原生自主移动中切换为 Reduce；最多允许当前帧完成，后续帧停止，不得跳到目标位置或继续后台移动。
- 运行时切回 No Preference，下一次合规自主 Explore 可恢复平滑移动；不得重放被取消的旧路径。
- 卸载或重建 Pet WebView 后确认旧媒体查询监听器移除，新监听器只上报一次初值，不形成重复 IPC 风暴。
- Reduced Motion 下验证单击、双击、抚摸、照料、键盘菜单、用户拖拽和明确回家仍可用；不得把无障碍偏好误解释为点击穿透或隐藏宠物。
- Browser Preview 可验证媒体查询与视觉降级，但原生窗口是否停止移动必须用签名 macOS/Windows 构建和坐标追踪证明。

## DESKTOP-LIFECYCLE-004 系统休眠与唤醒恢复

- 在宠物可见、置顶开启/关闭、点击穿透开启/关闭的组合下休眠并唤醒 macOS 与 Windows；恢复后必须重新施加权威策略，不能短暂获得错误交互能力。
- 唤醒前切换分辨率、主显示器、Dock/任务栏位置或断开宠物所在显示器；可见宠物应立即回到有效 Work Area，支持负坐标与混合 DPI，不能等待下一次自主动作。
- Presence/Profile 策略隐藏宠物后休眠唤醒；恢复协议不得显示宠物、抢焦点或打开控制中心。
- 在休眠期间终止 Pet WebView/窗口，唤醒后仅允许一个有界 Recovery Worker 重建；注入连续失败时按既有预算降级，不形成重建风暴。
- 让 Resume 与明确退出、系统注销并发 100 次；Shutdown gate 返回后不得重放窗口策略、创建宠物或启动恢复线程，失败仅记录 `desktop.application.resume-failed`。
- Browser Preview 不作为系统休眠、原生置顶、点击穿透、Work Area、DPI 或窗口重建证据；必须使用签名 macOS/Windows 包验证。

## DESKTOP-LOGIN-001 登录后自动陪伴

- 全新安装默认关闭；控制中心首次加载读取系统权威状态，不以本地缓存猜测。
- 开启和关闭后分别复查系统状态为 `true` / `false`；系统拒绝、权限撤销、只读环境或插件失败时不乐观显示成功，并保留或重读原状态。
- Browser Preview 仅在内存中可逆模拟，明确不会修改系统；Pet Window 不具备查询、开启或关闭权限。
- 自动启动后验证桌宠、托盘与控制中心可离线运行，Provider、Agent、Auto Mode、Automation、Skill、用户程序和网络请求均未被自动触发。
- 登录启动时控制中心从创建阶段保持隐藏，不出现启动闪烁或抢焦点；桌宠与托盘正常出现。普通快捷方式/应用图标启动必须显示、取消最小化并聚焦控制中心，Recovery Mode 即使来自登录项也必须显示恢复界面。
- 仅接受参数形态 `[executable, --nimora-login-launch]`；缺少、重复、混入额外参数或未知参数全部视为普通交互启动。登录项更新后验证 macOS LaunchAgent 与 Windows 登录项确实持久化该参数。
- 模拟领域事件或审计存储失败，普通启动与 Recovery 首屏仍通过纯原生窗口恢复显示；运行期托盘、桌宠深链和第二实例继续报告审计失败，不能把启动可用性绑定到非关键事件写入。
- macOS 真机验证 LaunchAgent 的开启、重登启动、关闭、外部删除与卸载清理；Windows 真机验证对应登录项、重登、策略拒绝与卸载行为。Linux 仅按官方插件实际支持范围声明，不用 Preview 代替。
- 验证 Safe/Recovery Mode 仍可关闭登录启动；系统状态读取失败时提供可访问错误信息，键盘、读屏、200% 缩放、窄窗口和 Forced Colors 下开关状态可辨认。

## DESKTOP-INSTANCE-001 单实例与重复启动恢复

- 在控制中心显示、隐藏、最小化以及桌宠点击穿透时分别重复启动 Nimora；只保留一个进程级宿主、一个托盘和一个桌宠窗口，既有控制中心被显示、取消最小化并聚焦。
- 并发启动 10 次、登录项与用户点击竞争、应用启动尚未完成时再次启动；不得出现两个数据库写入者、重复 Tick、重复 Agent/Automation/Skill Host 或第二套窗口。
- 从终端传入超长参数、未知 URL、文件路径、控制字符与不同工作目录；当前版本全部忽略，事件和诊断不得记录参数、路径或工作目录，也不得触发站内导航和任何 Capability。
- 主实例控制中心异常不可用时，第二实例退出且主实例记录 `desktop.single-instance.activation-failed`；不得为恢复焦点创建未审计窗口或启动第二个 Runtime。
- macOS 与 Windows 安装包真机验证用户会话隔离、快速重复点击、登录重登和升级后稳定性；Browser Preview 不宣称提供进程互斥证据。

## DESKTOP-LIFECYCLE-001 Dock 与应用激活恢复

- macOS 签名安装包中分别关闭和最小化控制中心，再点击 Dock 图标；既有控制中心必须显示、取消最小化并聚焦，桌宠、托盘、Runtime 与数据库写入者数量保持为一。
- Pet Window 已可见、点击穿透开启、Focus/Presentation/Safe/Recovery Mode 各状态下点击 Dock；均不得因“已有可见窗口”跳过控制中心恢复，也不得改变桌宠语义状态或安全模式。
- 快速重复点击 Dock，确认入口幂等，不创建新窗口、不重复初始化 Runtime，不触发 Provider、Agent、Auto Mode、Automation、Skill、用户程序或网络请求。
- 模拟控制中心缺失、原生显示失败和事件存储失败；应用记录 `desktop.application.reopen-failed` 后继续常驻。窗口已恢复但审计失败时不得再次隐藏窗口或退出 Runtime。
- Windows 重复启动继续验证 `DESKTOP-INSTANCE-001` 的单实例恢复，不以 macOS `Reopen` 冒充任务栏激活协议；两平台均需签名真机记录，Browser Preview 不作为原生焦点证据。

## DESKTOP-LIFECYCLE-002 明确退出与恢复竞态

- 移动桌宠后立即从托盘选择“退出 Nimora”；进程退出前同步保存最终原生位置，重启后恢复该落点，退出期间不得重新创建 Pet Window。拖拽尚未收到结束事件时退出，必须按 Profile 吸附策略原子收敛为 Idle/Perch/Climb/Peek，重启不得保持 `Dragged`。
- 在 Pet Window Recovery 已排队、正在退避和刚开始创建窗口的三个时点触发托盘退出；shutdown intent 一经发布，所有后续恢复尝试必须停止且不能消耗新的恢复预算。
- 模拟最终位置读取或持久化失败；记录 `desktop.tray.action-failed` 后仍完成退出，不因诊断、AI、网络或用户确认无限等待。
- 验证系统退出、托盘退出和快速连续退出均幂等停止自主循环，并有界静默 Auto Mode 与 Automation Event Session；不得遗留 Worker、托盘、数据库写入者或孤立桌宠窗口。
- macOS 与 Windows 签名包分别观察退出时窗口销毁顺序和进程树；Browser Preview 不能作为原生销毁、恢复线程或进程退出证据。
- macOS 分别通过 `⌘Q`、应用菜单退出和用户会话注销触发 `ExitRequested`，Windows 通过系统会话结束与原生退出请求触发；每条路径都执行 `ShutdownFlush`。注入窗口缺失或存储失败时记录 `desktop.application.shutdown-flush-failed`，不得取消系统退出或无限重试。

## DESKTOP-LIFECYCLE-003 关停期间激活准入

- 在控制中心原生显示操作已准入但尚未返回时触发退出；`begin_shutdown` 必须等待该有限操作结束，随后关闭 gate，不能死锁或中断到半显示状态。
- shutdown intent 返回后并发触发 Dock Reopen、第二实例、托盘双击、桌宠入口和恢复耗尽降级入口；所有请求均返回 `ShutdownInProgress`，不得调用 show/unminimize/focus 或创建窗口。
- 重复退出与高频激活竞争 100 次，确认 gate 单向且幂等，退出后没有控制中心闪现、焦点窃取、第二套 Runtime 或未终止线程。
- 入口失败继续使用各自既有最小诊断事件，不记录第二实例参数、工作目录、窗口标题或用户内容；Browser Preview 不作为原生并发和焦点证据。

## 桌面边缘 Surface Semantic

1. 在带左侧 80px 与顶部 30px 系统保留区的 Work Area 上，分别验证 Free、Left、Right、Top、Bottom 和四个 Corner；分类边界必须叠加 16/24/48px 生命体安全边距，而不是使用整屏边界。
2. 验证 ±2px 原生坐标取整仍命中目标 Surface，3px 及更远保持 Free；负坐标副屏与混合原点不得被归一到主屏。
3. 当前显示器不可用时 IPC 返回 `surface: null`；外层位置或尺寸读取失败必须返回错误，控制中心窗口调用必须被 `WindowForbidden` 拒绝。
4. 12 帧自主漫游期间只在最后一次 Position Revision 稳定且持久成功后发布事件；持久失败不发布，旧 Revision 不发布，拖拽中不发布。
5. Drop 成功清除 Drag 后立即发布一次 Surface 变化；Drop 持久失败继续保持 Drag，不得发布虚假栖息状态。回家、程序移动和离屏恢复沿窗口移动防抖路径收敛。
6. Overlay 必须通过 Platform Port 查询 Surface；内置、Sprite、glTF、VRM 使用同一角色舞台贴边位移，气泡、径向菜单、阴影、点击与拖拽命中区保持不动。
7. macOS/Windows 真机分别覆盖 Dock/任务栏四向布局、自动隐藏、混合 DPI、多屏插拔和跨屏拖拽；Surface 与肉眼所见工作区边缘一致，事件无抖动或高频循环。

## 自主漫游方向反馈

1. 当 Autonomy Sequence 为奇数且 Pet State 为 Walking 时，统一角色舞台必须投影为 Left；偶数必须投影为 Right，并与 Desktop 原生 Wander Target 的水平增量一致。
2. Idle、Observe、Sleep、Work、Interact、Drag 与 Recover 均必须投影 Neutral，不能保留上一次移动方向；旧快照缺少 Autonomy 时 Walking 必须稳定回退 Right。
3. 内置、Sprite Sequence、Sprite Atlas、glTF 和 VRM 必须由同一舞台容器镜像；状态气泡、径向菜单、阴影、点击与拖拽命中区不得被镜像。
4. 拖拽或安全模式抢占 Walking 时，原生移动必须停止，下一次 Snapshot 刷新后角色朝向收敛 Neutral；Renderer 切换不得改变方向规则。
5. `prefers-reduced-motion: reduce` 下取消朝向 Transition，但保留静态 Left/Right，避免用减少动态设置破坏必要空间语义。
6. macOS 与 Windows 真机分别录制至少两轮左右漫游，确认窗口位移、脚步动作与朝向同步，且 Dock、菜单栏、任务栏和副屏负坐标边界仍安全。

## 自主张望语义

- 让确定性自主序列进入 Observe，确认领域状态为 `observing`、动作目录为 `observe`、气泡表达好奇张望，不能播放庆祝跳跃或用户点击反馈。
- 内置角色验证低幅度左右张望；Sprite、glTF 与 VRM 分别验证专用 `pet.observe`、Manifest 回退链和缺动作回退 `pet.idle`，减少动态效果时不得持续位移。
- 用户代码、Agent Tool 与 Automation 读取动作目录应获得同一有序词表；调用 `observe` 必须通过 Capability Gateway、风险计算和标准 Command/Event，不允许直接指定 Renderer 私有动画名。
- 在 Observe 中触发拖拽、安静时段、Focus、Safe Mode 和重启，确认高优先级操作立即抢占，异常恢复 Idle，不残留庆祝情绪或活动截止时间。

## 原生桌面工作区边界

- 在 macOS 分别把 Dock 放在左、右、下方，并切换自动隐藏；拖拽吸附、自主漫游、回家和显示器恢复均不得把桌宠停入菜单栏或当前系统报告的 Dock 保留区。
- 在 Windows 分别使用底部、顶部和侧边任务栏，覆盖自动隐藏、125%/150% 缩放与混合 DPI 多屏；桌宠必须按所在显示器 Work Area 独立夹取，不能沿用主屏任务栏尺寸。
- 在 Linux X11/Wayland 与至少一个常见 Panel/Compositor 组合验证 Work Area；平台未报告保留区时维持整屏加安全边距的可恢复降级，不得隐藏、崩溃或产生伪造坐标。
- 自动化几何测试覆盖左侧 80px、顶部 30px、底部 50px 保留区，确认吸附、漫游和完全离屏恢复都落在叠加安全边距后的工作区内。

## Profile 安全删除

- 删除非活动 Profile，确认其 Snapshot 与 `profile.collection.deleted` 事件原子提交，活动 Profile、原生桌宠窗口、宠物成长、库存、资产和纪念均不改变。
- 删除活动 Profile，分别覆盖中间项、首项和末项；确认优先选择下一相邻项、无下一项时选择上一项，并在同一 Presence Transition 内同步可见性、置顶和鼠标穿透策略。
- 注入接替窗口策略应用失败，确认 Profile 未删除；注入 Snapshot/Event 提交失败，确认原生窗口完整补偿回旧策略、活动项不变且不发布成功事件。
- 尝试删除最后一个 Profile、传入不存在的 ID、非法接替项和非活动删除携带接替项，确认全部 fail-closed 且零副作用；Safe/Recovery Mode 必须拒绝删除。
- 在控制中心验证二次确认文案明确“不会删除宠物、资产或纪念”，最后一个删除按钮不可操作，删除正在编辑项后表单收起；在 macOS/Windows 签名 Tauri 包验证活动删除引发的隐藏、置顶和穿透组合切换。

## Profile 编辑

- 编辑非活动 Profile，确认名称和所有策略持久化、ID 与活动 Profile 不变，并且桌宠窗口没有变化。
- 编辑活动 Profile 的置顶、穿透、Presentation 可见性、主动频率、照料模式和安静时段，确认原生策略与 Snapshot 同步生效；重启后保持一致。
- 注入 Repository 保存失败和原生窗口应用失败，确认前者回滚原生策略、后者不提交 Snapshot，两者都不发布成功事件；Safe/Recovery Mode 禁止编辑。

## Profile 安静时段

- 创建未启用安静时段的 Profile，确认行为与旧版本一致；创建 `09:00–17:00` 和 `22:00–07:00` 两种 Profile，分别验证区间内自主动作停止、区间外恢复。
- 验证开始等于结束、分钟越界和损坏持久数据在 UI、Schema、Rust 领域边界被拒绝，且活动 Profile 与宠物状态不改变。
- 区间内继续执行点击、拖拽、喂食、玩耍、梳理、背包道具和生命值 Tick，确认均可用；切换系统时区、跨夏令时和睡眠唤醒后确认下一 Tick 使用新的本地分钟。

## 桌面启动迁移与系统密钥真机门禁

- 使用 `PRAGMA user_version = 1` 且缺少后续扩展表的最小旧数据库打开任一共享仓储，必须幂等创建 `automation_run_journal` 与索引，保留版本号和既有数据。
- 使用真实应用数据目录启动 Tauri，Schema Extension 可迁移时不得 setup panic、清库或进入 Recovery Mode；不可迁移或完整性失败时才允许隔离恢复。
- 系统密钥测试默认忽略且必须显式设置门禁环境变量；只写入唯一合成引用和值，验证写入、存在性、解析、撤销、缺失和幂等删除，并在任何中间失败后再次清理。
- macOS、Windows、Linux 发布任务分别执行原生 Keychain、Credential Manager、Secret Service 门禁；任一平台证据不得外推替代其它平台。

## Creator 权限 Diff 与一次性批准

- 同一结构化草案重复审查产生相同 `sha256:` 摘要；任意源码、Manifest、权限或自动化动作变化均改变摘要。
- 审查报告展示新增 Capability、文字风险级别、原因与最高风险，不能只靠颜色表达。
- 未审查、审查失败、未批准、批准过期、摘要不匹配、凭证重放和进入 Safe Mode 后保存全部失败关闭。
- 批准命令与保存命令均由宿主重跑生产契约、独立语法检查和无副作用行为沙箱。
- 成功保存只写入 `.nimora-drafts`，不得安装、激活、发布或签发运行 Grant。
- Browser Preview 的生成、批准和保存接口统一返回 `desktop-host-required`，不得伪造安全凭证。
- User Program/Skill 安装由宿主临时包生成 `manifest.json` 和 SHA-256 inventory，复用生产安装器；成功后必须未授权、未启用，失败不得留下半安装目录。
- 同一批准凭证只能选择保存或安装其中一次；安装失败、摘要变化或重放都必须重新审查和批准。
- 升级审查必须从完整性复验后的当前安装 Manifest 计算 `added`、`removed` 和 `scope-changed`；损坏安装包失败关闭，不得按未安装基线处理。
- User Program 比较事件、命令、并发及运行预算；Skill 比较激活事件、命令白名单与 Contributions。升级后旧授权不得继续生效。
- 批准凭证必须同时绑定草案摘要与完整审查摘要；在批准后改变当前安装版本，即使草案完全相同也必须拒绝安装、原子消费旧凭证且不可重放。

## VRM 1.0 纵切增量

- Verify VRM 安装必须使用 `.vrm`、`model/gltf-binary`、GLB 2.0、声明的 `VRMC_vrm`、1.0 `specVersion`、meta 与 humanoid；普通 GLB 伪装 VRM、VRM 0.x 和未声明扩展全部拒绝。
- Verify glTF 只加载 Three 基础图，VRM 才动态加载独立 Runtime；GLTF 依赖图与 VRM 增量分别执行 Bundle Budget，禁止拆块规避总图计费。
- Verify WebGL Context 丢失、组件卸载和模型切换停止 Mixer/VRM 更新并释放纹理、材质、场景与 Context；第三方模型失败回退内置角色。
- Verify `pet.click`/`pet.celebrate`、`pet.drag`、`pet.sleep`、`pet.error` 只映射到固定 VRM Preset 且权重归一化；每次切换先 reset，缺失 Preset 或损坏 Manager 不泄漏异常和旧表情。
- Verify 无 Animation Clip 的合法 VRM 仍可接收 Expression；Reduced Motion 冻结连续 Mixer/VRM 更新但不阻止一次性静态表情投影；`pet.work`、未知动作和厂商私有名称回到 neutral。

> 版本：0.1.0-draft  
> 更新日期：2026-07-18
> 状态：测试基线

## 1. 目的

本文将产品能力转换为可执行功能测试。测试人员必须同时验证正常路径、边界、权限、故障恢复和跨平台差异。性能、安全和长稳测试的门禁见 [`DELIVERY_TESTING.md`](DELIVERY_TESTING.md)。

## 2. 测试环境矩阵

| 维度 | 最低覆盖 |
|---|---|
| Windows | Windows 10 22H2、Windows 11 当前稳定版 |
| macOS | macOS 12、当前稳定版；Intel 与 Apple Silicon |
| 显示 | 单屏、双屏、主屏切换、100%/150%/200% DPI |
| 网络 | 离线、正常、延迟、断连、代理、DNS 失败 |
| 主题 | 浅色、深色、高对比度、减少动画 |
| 安装状态 | 全新安装、升级、降级恢复、损坏配置 |

## 3. 测试数据

- 官方默认 Character、Skin、Theme。
- 缺失动作、超预算、hash 错误、路径穿越的资源包。
- Safe、Sensitive、Danger 三类测试 Skill。
- Source、Sink、Duplex 测试 Connector。
- OpenAI-compatible mock Provider 和超时/错误 Provider。
- 合法、过期、Scope 不足和已撤销 Gateway Token。

## 4. 用例格式

每条正式用例必须包含：

```text
ID / 标题 / 优先级 / 前置条件
测试数据 / 操作步骤 / 预期结果
平台差异 / 自动化状态 / 关联需求 / 缺陷链接
```

优先级：P0 阻断发布；P1 核心回归；P2 扩展回归；P3 探索性。

## 5. Pet Runtime

| ID | 场景 | 关键步骤 | 预期结果 | P |
|---|---|---|---|---:|
| PET-001 | 首次启动 | 全新安装后启动 | 默认宠物在可用区域出现；不遮挡系统关键 UI | P0 |
| PET-002 | 拖拽与持久化 | 拖到副屏后重启 | 位置、显示器和缩放恢复 | P0 |
| PET-003 | 显示器拔出 | 宠物位于副屏时拔出 | 宠物回到主屏可见区域 | P0 |
| PET-004 | 点击穿透恢复 | 开启穿透后尝试交互 | 桌面可点击；托盘和热键可恢复 | P0 |
| PET-005 | 置顶 | 切换置顶并打开其他窗口 | 行为符合平台约定；设置状态正确 | P1 |
| PET-006 | 独立桌宠生命周期 | 关闭控制中心并持续互动 | 宠物窗口继续运行，聊天或 AI 不构成依赖 | P0 |
| PET-007 | 离线桌宠体验 | 断网并禁用全部 Provider 后运行 24 小时 | 自主行为、照料、互动和持久化完整可用 | P0 |
| PET-008 | 自主行为调度 | 观察待机、漫游、休息、提醒并切换专注模式 | 行为有冷却和频率预算，专注模式立即降扰 | P0 |
| PET-009 | 手势仲裁与落地 | 连续执行点击、长按、抚摸、拖拽和跨屏释放 | 无误触风暴；姿态正确；落点始终可恢复 | P0 |
| PET-010 | 显示环境变化 | 插拔副屏、改变 DPI、睡眠唤醒并进入全屏 | 宠物不丢失、不遮挡关键界面并确定性恢复 | P0 |
| PET-011 | 长稳与渲染降级 | 不同 Renderer 连续运行 8 小时并注入崩溃 | 无明显泄漏；有限重建；最终回退内置角色 | P0 |
| PET-012 | 低压力照料 | 长期不投喂后恢复互动 | 不死亡、不惩罚、不制造焦虑，状态可解释 | P1 |
| PET-013 | 自主动作抢占 | 自主探索期间开始拖拽、点击或显式动作 | 用户状态立即优先；自主计时结束不得覆盖用户状态 | P0 |
| PET-014 | 自主计划重启恢复 | 安排动作后退出并在到期前后分别重启 | 序列与截止时间持久恢复；无启动行为风暴或重复事件 | P0 |
| PET-015 | 安全模式抑制 | 自主动作前后进入 Safe/Recovery Mode | 不启动新动作；恢复后按持久计划安全收敛 | P0 |
| PET-016 | 原生窗口自主漫游 | 等待 Explore 并在主屏、左侧负坐标副屏及四边附近触发 | Walking 与窗口位移同步；短路径平滑且始终夹取在当前显示器安全区域 | P0 |
| PET-017 | 漫游中断 | 平滑移动期间开始拖拽、进入 Safe Mode 或由用户切换动作 | 下一帧前停止宿主位移，不回弹、不覆盖用户状态；最终位置防抖持久化 | P0 |
| PET-020 | Profile 自主频率 | 依次切换 0、20、21、40、41、60、61、80、81、100% Profile | 0% 不启动；频率越高首次延迟与冷却单调不增加；切换后下一次一秒 Tick 生效 | P0 |
| PET-021 | Profile 即时降扰 | 自主漫游中切换 Focus 或 Presentation，再在拖拽中重复 | 自主控制状态立即回 Idle；拖拽等用户状态保持不变；清理活动计划且无行为风暴 | P0 |
| PET-022 | Offline Profile 自主生命 | 禁网、禁用 Provider 并切换 Offline Profile | 仍按配置完成观察、漫游和休息，不产生 Provider 或网络调用 | P0 |
| PET-023 | 离线生命值演化 | 首次启动、运行 30 分钟、退出 6 小时及退出超过 24 小时后恢复 | 首次不衰减；按十分钟粒度演化；离线追赶最多 24 小时且无启动写入风暴 | P0 |
| PET-024 | 互动成长 | 连续点击互动并在 Mood/Affinity 接近 100 时重复 | 每次互动产生稳定增益，所有值封顶 100，用户状态和动画保持合法 | P0 |
| PET-025 | 生命值事务失败 | 在 Snapshot/Event 原子保存处注入失败 | 持久状态、内存状态和事件总线均保持原值；下一次 Tick 可安全重试 | P0 |
| PET-026 | 双窗口生命值同步 | 控制中心打开或关闭时触发生命 Tick 与互动 | Overlay 和控制中心读取同一 Snapshot；关闭控制中心不影响桌宠生命循环 | P0 |
| PET-027 | 三类照料收益 | 分别执行 Feed、Play、Groom 并观察前后 Snapshot | 收益符合领域表；Play 正确消耗 Energy；所有生命值保持 0–100 | P0 |
| PET-028 | 照料冷却与防刷 | 30 秒内跨 Overlay、控制中心重复照料，再在边界时刻重试 | 全入口共享宿主冷却；提前请求零状态变化；边界请求只成功一次 | P0 |
| PET-029 | 照料优先级 | 自主漫游和用户拖拽期间分别照料 | 自主动作可被照料安全中止；拖拽绝不被照料覆盖 | P0 |
| PET-030 | 照料事务失败 | 注入 Snapshot/Event 保存失败后执行任一照料 | 生命值、冷却时间、内存和事件均不改变，可立即安全重试 | P0 |
| PET-031 | 桌边阈值吸附 | 将桌宠分别释放在四条安全边界 32px 内及阈值外 | 阈值内吸附最近边；阈值外保持释放位置；不发生跨边跳跃 | P0 |
| PET-032 | 多屏与负坐标吸附 | 在左右、上下排列且含负坐标的副屏拖放桌宠 | 使用当前显示器物理区域与窗口尺寸计算，结果始终位于该屏安全范围 | P0 |
| PET-033 | 角落吸附确定性 | 在水平与垂直边距离相同和不同时反复释放 | 选择距离更近的边；同距按稳定水平优先级处理，不产生抖动 | P1 |
| PET-034 | Profile 关闭吸附 | 创建 `edgeSnap=false` Profile，切换后在边缘释放并重启 | 保留自由摆放位置；旧 Profile 缺字段时默认安全开启吸附 | P0 |
| PET-035 | 低精力行为优先级 | 将 Energy 设为 25、Mood 同时设为低值并推进到自主触发点 | 稳定选择 Rest/Sleep；Energy 优先于 Mood；不触发 AI、网络或通知 | P0 |
| PET-036 | 低心情温和反馈 | 保持 Energy > 25、将 Mood 设为 25 后推进自主行为 | 选择 Observe/互动语义，不产生惩罚、扣分或强制弹窗 | P0 |
| PET-037 | Idle 情绪派生 | 分别进入低 Energy、低 Mood、健康 Idle，并在活动状态中改变生命值 | Idle 显示 Sleepy/Sad/Neutral；活动状态表情不被生命值 Tick 覆盖 | P0 |
| PET-038 | 非打扰状态气泡 | 在低生命值、睡眠、漫游与工作状态悬浮或键盘聚焦桌宠 | 文案优先表达当前动作且语气温和；未交互时气泡隐藏；无系统通知 | P1 |
| PET-039 | 点击与拖拽仲裁 | 分别轻微抖动、移动 5px、移动 6px 及正常拖拽后释放 | 阈值内只触发一次互动；达到阈值只拖拽且不追加点击；释放后状态稳定 | P0 |
| PET-040 | 单击双击消歧 | 单击一次后等待、快速双击、双击后立即拖拽 | 单击只互动一次；双击取消待执行单击并庆祝；拖拽优先且无延迟误触 | P0 |
| PET-041 | 长按与右键菜单 | 分别长按 500ms、右键、菜单外再次操作桌宠 | 两种入口打开同一轻量菜单；不出现浏览器默认菜单；重新交互可安全关闭 | P0 |
| PET-042 | 桌宠菜单键盘与边界 | 用 `ContextMenu` 与 `Shift+F10` 分别从桌宠主按钮打开菜单，以方向键/Home/End 遍历、Esc 关闭，并在 260×300 原生窗口检查 | 两种标准入口均阻止系统菜单并打开同一径向菜单；首项获得焦点；所有动作可操作；Enter/Space 仍执行普通互动；Esc 焦点回到桌宠；菜单不被窗口裁切 | P0 |
| PET-043 | 长期陪伴持续成长 | 将 Affinity 提升至 100 后继续单击、Feed、Play、Groom | Affinity 保持 100；BondPoints 分别持续增加 1、1、2、3；等级每 50 点稳定提升 | P0 |
| PET-044 | 旧关系数据迁移 | 加载缺少 BondPoints 且 Affinity=34 的旧快照，再合法互动一次 | 加载不失败；有效累计基线为 34；互动后 BondPoints 精确为 35，不丢失且不双计数 | P0 |
| PET-045 | 成长原子性与拒绝路径 | 在冷却、拖拽状态及 Snapshot/Event 保存失败时执行互动或照料 | BondPoints、Affinity、冷却、内存和事件均不产生假增益；恢复后可安全重试 | P0 |
| PET-046 | 关系卡契约与视觉 | 分别用 0、49、50、84 点及旧快照打开控制中心并缩窄窗口 | 等级、累计陪伴、升级进度和关系温度语义分离；进度条准确且文案不溢出 | P1 |
| PET-047 | Profile 关闭自主气泡 | 活动 Status 气泡展示时关闭 `statusBubblesEnabled`，再触发漫游、点击、错误和 Agent 反馈 | 当前 Status 立即退场且后续 Status 被抑制；Feedback/Error 正常展示，无持久事件或日志 | P0 |
| PET-048 | 自主气泡迁移与切换 | 加载缺字段旧 Profile，并在开启/关闭 Profile 间反复切换和重启 | 旧 Profile 默认开启；切换后一次 Snapshot 收敛，无监听风暴、闪现或反馈静音 | P0 |
| PET-047 | 抚摸轨迹识别 | 按住桌宠在 12px 内往返移动至少 32px/160ms，再测试短时、单向和无反转轨迹 | 只有合格往返轨迹触发一次抚摸；其它轨迹不产生抚摸成长或事件 | P0 |
| PET-048 | 抚摸与拖拽仲裁 | 抚摸中逐步移出 12px、直接快速拖出、完成抚摸后释放 | 达到边界立即且只进入原生拖拽；拖拽不追加 Click/Stroke；合法抚摸不移动窗口 | P0 |
| PET-049 | 抚摸成长与隐私 | 完成抚摸并检查 Snapshot、Event、双窗口与持久化故障路径 | Mood +4、Affinity/BondPoints +2；双窗口刷新；只保存有界摘要，不保存逐点轨迹；失败零增益 | P0 |
| PET-050 | 抚摸反馈与可访问性 | 在内置、Sprite、GLTF/VRM Renderer 上抚摸，并启用减少动态效果 | 指针和爱心反馈清晰但不遮挡角色；状态文案可读；减少动态时动画近乎即时停用 | P1 |
| PET-051 | 悬停注视与冷却 | 鼠标进入角色，反复移入移出，再用触摸与菜单打开状态重试 | 首次短暂 Observing/Surprised；8 秒内不重复提交；触摸、菜单和手势期间不触发 | P1 |
| PET-052 | 悬停无成长与抢占 | 记录成长值后悬停，并在 900ms 内点击、拖拽或睡眠 | 成长值不变；新动作不被延迟收敛覆盖；事件只保存有界坐标和表现状态 | P0 |
| PET-053 | 瞬时反馈双窗口收敛 | 分别执行照料、道具、单击、双击、抚摸和悬停并等待反馈时限 | Pet Overlay 与控制中心都重读 Idle/权威后继状态；Renderer 无永久 Interacting/Observing | P0 |
| PET-054 | 延迟收敛抢占 | 反馈时限内开始拖拽、睡眠或自主动作 | 旧 Finish 失败关闭且不广播伪恢复；新动作与文案保持不变 | P0 |
| PET-055 | Agent 任务协作成长 | 完成一个本地用户 Agent 任务，再重复提交同一完成结果，并尝试 Skill/Automation/Renderer 完成信号 | 首次原子增加 2 陪伴、1 亲密、3 心情并发布关联事件；重复为 `already_awarded`；非 Desktop 本地用户来源零成长 | P0 |
| PET-056 | 协作成长持久化故障 | 在 Agent 已成功后注入 Pet Repository 保存失败 | Agent 仍返回完成；`companionGrowth.status=unavailable`；Pet 数值、收据、纪念和事件均不改变 | P0 |
| PET-084 | 自主行为持久频率预算 | 将五档主动频率持续运行至预算耗尽，重启后继续，并模拟时钟回拨和伪造 Start | 容量单调为 2/6/12/20/30 次每小时且平滑补充；重启不重置、回拨不增发、伪造不可透支；手动互动与照料不受影响 | P0 |
| PET-085 | 宿主全局注意力预算 | 并发提交自主 Motion/气泡、Agent 气泡和声音请求，再切换全屏或勿扰并提交用户反馈与 Safety | Ambient 通道共享 12 令牌/分钟并按通道计费；环境抑制时拒绝 Ambient；用户反馈与 Safety 始终可用且不消耗令牌；扩展不能重置预算 | P0 |
| PET-086 | Pet WebView 心跳自愈 | 停止 Overlay 心跳、隐藏后长时间等待、重新显示、模拟时钟回拨并执行系统退出 | 可见窗口失联 90 秒后由宿主销毁并按既有有界退避重建；隐藏时不恢复，重新可见获得完整宽限；回拨不误判；退出后永不复活 | P0 |
| PET-055 | 同状态反馈代次隔离 | 在首个 600ms 反馈结束前连续单击、照料或抚摸，并让首个 Timer 先到期 | 每次 Command 取得不同安全整数代次；旧 Finish 拒绝且第二个 Interacting 保持；仅当前代次可恢复并广播双窗口状态 | P0 |
| PET-056 | 反馈代次迁移与边界 | 加载缺少两项代次字段的旧快照，再构造负数、非整数、超 JS 安全整数和上限后新反馈 | 旧数据迁移为 0/无活跃反馈；非法跨端输入拒绝；上限后确定性回到 1 且不出现 0 或精度损失 | P0 |
| DP-STRETCH-001 | 自主伸懒腰领域轮转 | 健康宠物将自主序列推进至 Stretch，再测试低精力、低需求、Quiet、Focus 与拖拽抢占 | 进入 Stretching/Happy 后按时恢复；不增加成长；低值优先级不变；抑制和抢占不覆盖用户状态 | P0 |
| DP-STRETCH-002 | 伸懒腰跨 Renderer 与扩展 | 从更多菜单、用户代码、Automation、Agent 分别请求 Stretch，并测试内置、Sprite、glTF、VRM 和缺动作角色包 | 所有来源进入同一 Action Catalog；内置姿态清晰且 Reduced Motion 安全；第三方播放 `pet.stretch` 或确定性回退 Idle；VRM 只用白名单有界表达式 | P0 |
| UI-021 | 控制中心工作区按需加载 | 首次打开概览，再依次进入角色、Agent、自动化、扩展和设置 | 概览不等待非首屏代码；各工作区仅在首次进入时加载，完成后完整可用 | P0 |
| UI-022 | 工作区 Chunk 故障恢复 | 阻断任一非首屏 Chunk 后进入对应导航，再恢复资源并点击重新加载 | 显示有语义的局部故障页；桌宠与导航保持运行；无需重启应用即可重试 | P0 |
| UI-023 | 恢复模式设置入口分包 | 以 Recovery Preview 和损坏主库启动，观察自动导航及数据恢复中心 | 恢复横幅立即可见；设置 Chunk 可独立载入；失败时仍保留导航、状态说明和重试入口 | P0 |
| PET-051 | 饱腹与清洁离线演化 | 建立时间基线后推进 6 个周期，再模拟 24 小时离线追赶 | Satiety 下降 3、Cleanliness 下降 1；追赶有界且不死亡、不倒扣关系、不产生网络依赖 | P0 |
| PET-052 | 照料需求真实语义 | 在低饱腹/低清洁状态依次 Feed、Groom、Play | Feed 恢复 25 饱腹；Groom 恢复 30 清洁；Play 消耗 3 饱腹/2 清洁；所有值限定 0–100 | P0 |
| PET-053 | 新需求旧快照迁移 | 加载没有 Satiety/Cleanliness 的旧 Pet Snapshot 和旧前端 Schema 输入 | 两项均默认 100；原状态、关系和时间戳不变；保存后可稳定往返 | P0 |
| PET-054 | 需求事件原子性与低压力反馈 | 注入 Snapshot/Event 保存失败，并分别构造低精力、低饱腹、低清洁、低心情 | 失败时四项需求、冷却和事件均不改变；气泡按优先级温和表达，不发送系统通知 | P0 |
| PET-055 | 手动睡眠真实恢复 | 将 Energy 设为 40，进入 Sleep 后推进 6 个生命周期，再将 Energy 设为 99 重复 | Energy 恢复至 52 并在第二次饱和至 100；其它需求继续低压演化；标准生命事件原子记录 | P0 |
| PET-056 | 自主休息完成与抢占 | 以 Energy=20 完成一次 Rest，再分别在 Rest 中 Drag、进入静默策略 | 正常完成精确恢复 8 Energy；抢占与抑制不发放完成收益；用户状态不被覆盖 | P0 |
| PET-057 | 宠物窗口销毁恢复 | 终止 `pet` WebView、连续注入四次销毁，再执行正常退出 | 前三次按 1/2/4 秒单 Worker 重建并恢复当前位置与 Presence；第四次不重启风暴、打开控制中心且 Core/托盘存活；正常退出零重建 | P0 |
| PET-058 | 原生尺寸浏览器验收框 | 分别在 Browser Preview 与 Tauri 打开 `?view=pet`，展开根菜单、背包、更多和改名 | Preview 内容区固定 260×300 且无裁切；Tauri 无棋盘背景或 Preview 边框；浏览器结果不冒充原生拖拽、透明合成和 DPI 证据 | P1 |
| PET-018 | 副屏拔出恢复 | 宠物位于副屏时拔出显示器并等待守卫周期 | 完全离屏后回到主屏安全区域；不依赖重启或控制中心 | P0 |
| PET-019 | 分辨率与 DPI 恢复 | 改变当前屏幕分辨率、缩放并让窗口横跨两屏 | 保留最大可见面积所在屏并重新夹取；拖拽期间不抢夺 | P0 |
| PET-006 | DPI 切换 | 跨不同 DPI 屏幕拖动 | 尺寸稳定、画面清晰、命中区一致 | P0 |
| PET-007 | 锁屏恢复 | 锁屏、等待、解锁 | 状态合理恢复，无动画时间跳变 | P1 |
| PET-008 | 全屏避让 | 打开全屏应用 | 按 Profile 靠边、隐藏或静默 | P1 |

当前自动化证据：领域测试验证首次延迟、确定性 Observe 动作、动作截止、冷却、用户拖拽抢占、安静策略即时抑制、生命基线、24 小时有界追赶与互动增益封顶；Runtime 测试证明生命值与标准事件原子持久化，注入失败时不发布内存状态或事件；Desktop 纯函数测试验证 Profile 五档频率及多屏几何；Persistence 测试证明版本 1 Pet Snapshot 可往返且新增时间字段对旧 JSON 使用默认值兼容。PET-007 的离线 24 小时、PET-010 多屏/DPI、PET-011 8 小时长稳及 PET-016 至 PET-026 的真实原生窗口观感仍需真机执行，不得仅凭单元测试标记通过。

## 6. FSM、数值与人格

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| FSM-001 | Idle 随机行为 | 权重和冷却生效，不发生高频状态抖动 | P0 |
| FSM-002 | Drag 打断 Sleep | Drag 立即获得高优先级，Drop 后安全恢复 | P0 |
| FSM-003 | 缺失动画 | 按资源 fallback 链回退到 `pet.idle` | P0 |
| FSM-004 | 数值边界 | 所有属性始终保持声明范围，重复操作不溢出 | P0 |
| FSM-005 | 夜间策略 | 模拟时钟后 Sleep 权重按 Profile 生效 | P1 |
| FSM-006 | 人格趋势 | 相同长期输入产生可解释的倾向变化 | P2 |
| FSM-007 | 记忆删除 | 删除记忆后查询、导出和 Agent 上下文均不再包含 | P0 |

## 7. 自定义资源与皮肤

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| AST-001 | 安装 Character | 校验、预览、确认、原子安装成功 | P0 |
| AST-002 | 切换 Skin | 保留宠物位置、数值和当前可映射状态 | P0 |
| AST-003 | 不兼容 Skin | 安装前明确阻止并说明兼容条件 | P0 |
| AST-004 | 损坏纹理 | 健康检查失败，当前资源不受影响 | P0 |
| AST-005 | hash 错误 | 拒绝安装并记录诊断，不提供绕过按钮 | P0 |
| AST-006 | 路径穿越 | 拒绝含 `..`、绝对路径或逃逸链接的包 | P0 |
| AST-007 | 资源超预算 | 发布校验失败；开发预览显示具体超限项 | P1 |
| AST-008 | 热重载 | 开发模式更新资源后刷新且不泄漏纹理 | P1 |
| AST-009 | Bundle 回滚 | 组合包任一依赖失败时全部恢复原版本 | P0 |
| AST-010 | 多语言回退 | 当前 locale 缺失时按声明顺序回退 | P1 |
| AST-011 | 减少动画 | 系统设置启用后资源不能强制大幅运动 | P0 |
| AST-012 | 版权元数据 | 缺少许可证或发布者信息时不能发布 | P1 |
| AST-013 | Renderer 伪造身份或清单 | IPC 不接受 Asset ID 与文件清单；宿主从包内 Manifest 和完整性文件推导并复验 | P0 |
| AST-014 | Catalog 遇到损坏包 | 损坏包进入 rejected 诊断，其它包和默认角色继续可用，WebView 不获得资源路径 | P0 |
| AST-015 | 激活健康 Character | 宿主复验类型与完整性后原子保存 Asset ID，重启后仍选中且不暴露路径 | P0 |
| AST-016 | 活动 Character 损坏或缺失 | 读取时自动回退 `builtin.aster`，返回诊断且其它资源不受影响 | P0 |
| AST-017 | 安全模式角色策略 | 安全模式始终使用内置角色，并拒绝第三方角色激活 | P0 |
| AST-018 | Sprite Clips 契约 | 拒绝缺少 `pet.idle`、路径逃逸、非法动作名、空帧、超限帧数和非法时长 | P0 |
| AST-019 | 宿主 Renderer Descriptor | 复验 Sprite Manifest、Clips、后端与 inventory 后返回无文件系统路径的 `nimora.renderer/1`；失败明确回退内置角色 | P0 |
| AST-020 | 只读角色资源协议 | 仅 Pet 窗口可读取活动包清单内图片；拒绝未知 Host、查询参数、非 GET、编码穿越、非图片、非活动包和安全模式请求 | P0 |
| AST-021 | Sprite Sequence 实际渲染 | Pet Overlay 按每帧 `durationMs` 切换受控图片 URL，非循环动作停在末帧，动作切换从首帧开始 | P0 |
| AST-022 | Sprite Atlas 实际渲染 | Canvas 按描述符裁切 Atlas 帧，尊重画布、锚点、缩放与 pixel-art 设置，不暴露文件系统路径 | P0 |
| AST-023 | Sprite 渲染失败回退 | 图片加载失败、Canvas context 缺失或帧超出图片实际边界时立即显示内置 Aster，Pet 交互保持可用 | P0 |
| AST-024 | 动画资源清理与减少动画 | 减少动画时固定首帧；动作、角色切换和卸载后无遗留 Timer、Image handler 或重复监听器 | P0 |
| AST-025 | 活动角色热切换 | 激活角色及进入/退出安全模式后只向 Pet 窗口发出描述符刷新事件，已打开 Overlay 无需重启即可切换或回退 | P0 |
| AST-026 | 安装前安全预览 | 仅 Control Center 可通过系统文件选择器选择 `.nimora`；宿主返回复验后的身份、许可和预算，确认安装时再次完整复验并拒绝预览后篡改 | P0 |
| AST-027 | 导入 `.nimora` 归档 | 限额展开合法单层包；拒绝路径逃逸、链接、特殊文件、重复条目、嵌套归档和异常压缩倍率；失败不改变活动资源 | P0 |
| AST-028 | 确定性导出 `.nimora` | 完整验证展开目录后原子写出；相同输入字节一致且可重新预览、安装；无效输入不覆盖已有目标 | P0 |
| AST-029 | 安装前静态海报预览 | 仅 Control Center 可读取 Manifest 明确声明且受 inventory 保护的 PNG/WebP；拒绝伪造头、路径逃逸、超过 2 MiB 或 4096 px 的图片；取消和换包后释放 Blob URL | P0 |
| MODEL-001 | 导入合法 Live2D/VRM/glTF 模型 | 完成校验、映射与预览，可原子安装 | P0 |
| MODEL-002 | 导入路径穿越、远程 URI、Zip Bomb 或畸形网格 | 拒绝且 Core 与已安装资源不受影响 | P0 |
| MODEL-003 | Adapter 崩溃或 GPU context 丢失 | 恢复默认角色，可重建渲染实例 | P0 |
| MODEL-004 | 连续切换、卸载模型 | 纹理、内存、监听器和临时文件无持续增长 | P1 |
| MODEL-005 | 脚本发送标准表情与动作 | 序列帧、Live2D、VRM 均映射或明确回退 | P1 |
| MODEL-006 | GLB 探测 Worker 隔离 | 合法 GLB 返回有界结构报告；远程 URI、路径逃逸和畸形 chunk 被拒绝；超时 Worker 被强杀，崩溃不影响 Core | P0 |
| MODEL-007 | 桌面 GLB 隔离检查 | 仅 Control Center 和非安全模式可选择绝对普通 `.glb`；宿主拒绝链接与 80 MiB 超限文件，复制为固定暂存名后调用 sidecar，成功、拒绝、崩溃和超时均清理暂存目录，报告不含宿主路径 | P0 |
| MODEL-008 | GLB 规范化安装 | 对同一暂存文件重新执行 Worker 探测后，宿主生成 `nimora.asset/1` Character Manifest、`entrypoints.model` 与 SHA-256 inventory，并通过正式安装器原子激活 | P0 |
| MODEL-009 | 本地模型命名空间 | Creator Studio 生成的模型只能使用 `character.local.*`，不能覆盖第三方发布者命名空间；无效 ID、名称或许可证不改变资源目录 | P0 |
| MODEL-010 | 受控 GLB 资源协议 | 仅 Pet WebView 可用 GET 从受控 Host 读取当前活动 Asset 的唯一 `entrypoints.model`；拒绝 query、错误 Host、非活动 Asset、Manifest、Integrity 和其它 inventory 路径 | P0 |
| MODEL-011 | GLB 实际渲染与自动 framing | Three.js 加载验证后的 GLB，按包围盒居中缩放、设置相机和灯光，透明画布适配尺寸与高 DPI；加载失败回退内置角色 | P0 |
| MODEL-012 | GLB context 丢失和完整资源释放 | context 丢失时阻止默认行为并回退；切换或卸载后取消帧循环、停止 Mixer、断开观察器并释放几何体、材质、纹理和 WebGL context | P0 |
| MODEL-013 | GLB 动画映射与减少动画 | Worker 只返回有界命名动画；Creator 可编辑标准动作绑定且有命名动画时必须提供 `pet.idle`；Renderer 精确匹配、fallback、循环/一次性语义和 180 ms cross-fade，减少动画时暂停 Mixer | P0 |
| MODEL-014 | 动画映射完整性与缺失片段 | `nimora.animation-map/1` 必须在 inventory 内并要求合法 `pet.idle`；篡改、未知动画、fallback 环或缺失动作不得导致任意首动画播放或 Renderer 崩溃 | P0 |

## 8. Command 与命令面板

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| CMD-001 | 模糊搜索 | 标题、ID、别名和扩展名均可检索 | P1 |
| CMD-002 | 参数校验 | 非法参数不执行，错误定位到字段 | P0 |
| CMD-003 | 权限拒绝 | Command 返回结构化拒绝，状态不改变 | P0 |
| CMD-004 | 取消执行 | 可取消 Command 及时终止并清理状态 | P1 |
| CMD-005 | Undo | 支持撤销的 Command 恢复前一状态并写审计 | P1 |
| CMD-006 | 冲突注册 | 重复 ID 的 Contribution 被拒绝并定位来源 | P0 |

## 9. Automation

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| AUT-001 | Interval 触发 | 在允许误差内触发且遵守冷却 | P0 |
| AUT-002 | 条件短路 | 未满足条件时动作不执行，历史显示原因 | P0 |
| AUT-003 | 分支与并行 | 分支选择正确，并行结果按策略汇合 | P1 |
| AUT-004 | 超时和重试 | 仅对声明重试安全的动作重试 | P0 |
| AUT-005 | 取消前序 | `cancel_previous` 正确终止旧实例 | P1 |
| AUT-006 | 非幂等保护 | 重放重复事件不会重复执行非幂等动作 | P0 |
| AUT-007 | 测试运行 | 展示预计步骤，不产生真实外部副作用 | P0 |
| AUT-008 | 虚拟时间 | 快进时间可稳定复现定时规则 | P1 |
| AUT-009 | Agent 转规则 | 保存后生成可编辑、可验证的规则 | P2 |

当前自动化证据：`nimora-automation-runtime` 单元测试覆盖条件短路、dry-run 零 Backend 调用、幂等重试门禁、瞬时错误三次尝试、失败后的逆序补偿、取消与超时前零执行；桌面 Rust 测试验证 IPC 与 Agent `automation.definition.validate` 工具都只返回 `planned` 步骤且尝试次数为零，Agent 测试同时证明无需确认、无待处理项、无 Runtime 状态变化且普通用户程序不能继承专用 Query；前端测试验证匹配与不匹配事件的离线预览。Interval、分支/并行、持久调度、真实 Action Gateway、运行历史和事件回放仍不得标记通过。

事件驱动补充证据：桌面测试证明真实 Event Bus 来源的 `eventId/traceId` 不被重建并同时进入 Run 与 SQLite Journal；Safe Mode 收敛测试将 Automation Event Session 作为独立隔离步骤，即使前序隔离失败也不会跳过。SQLite 测试覆盖历史有界倒序读取、非法上限拒绝、运行中记录不可删除，以及相同毫秒 UUID 排序、跨页无重复和较新并发插入不改变旧游标链。事件会话指标测试覆盖 executed/dropped/failures 累计与饱和不回绕；桌面健康 DTO 不包含事件正文，Browser Preview 返回空会话而不伪造健康。仍需补真实 Tauri AppHandle 的队列溢出端到端、跨重启启用恢复、参数级批准、持久健康趋势和回放测试。

## 10. Skill 与扩展宿主

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| EXT-001 | 安装 Safe Skill | 权限摘要正确，启用后贡献项出现 | P0 |
| EXT-002 | 扩大权限升级 | 升级暂停并要求重新授权 | P0 |
| EXT-003 | 直接文件访问 | 扩展无法绕过 Capability Broker | P0 |
| EXT-004 | 崩溃隔离 | Host 崩溃不影响 Core，贡献项被撤销 | P0 |
| EXT-005 | 连续崩溃 | 达阈值后进入 quarantine，不无限重启 | P0 |
| EXT-006 | CPU/内存超额 | 扩展被限流或终止，用户看到原因 | P0 |
| EXT-007 | 扩展配置事务应用失败 | 不发布半写入配置，保持应用前状态并报告原因 | P0 |
| EXT-008 | 卸载保留数据 | 用户选择正确执行，密钥引用被清理 | P1 |
| EXT-009 | 精确 Skill 授权 | 版本或 Capability 集合不一致时 fail-closed，无 Contribution 生效 | P0 |
| EXT-010 | Contribution 租约撤销 | 暂停、崩溃或 quarantine 后 Command 快照与 AI requester 同步消失 | P0 |
| EXT-011 | Skill 请求 AI | 仅已激活且声明 Agent Task Contribution 的 Skill 获得 `skill:<id>`，随后进入统一 Module Adapter | P0 |
| EXT-012 | Skill 绕过 AI 网关 | 未激活、未声明或尝试直连 Provider 时在 Provider 前拒绝且无副作用 | P0 |
| EXT-013 | 独立 Skill Worker | JavaScript 在真实子进程执行且无 Node、Tauri、文件、网络原生对象 | P0 |
| EXT-014 | Skill Worker 失控 | 无限循环、取消或输出超限时 Supervisor 强制回收进程 | P0 |
| EXT-015 | Worker 故障撤销 | 超时、崩溃或协议违规进入 crash window，Contribution 与 AI requester 立即撤销 | P0 |
| EXT-016 | Worker Active 租约 | 未安装、未授权、暂停或伪造不同版本 Manifest 均在进程启动前拒绝 | P0 |
| EXT-017 | Skill 原子升级与回滚 | v2 激活前备份 v1；加载 v2 后回滚恢复 v1 的 Manifest 与源码，不出现半安装目录 | P0 |
| EXT-018 | Skill 安装后完整性复验 | 修改源码、Manifest 或完整性锁，新增未跟踪文件、符号链接或路径逃逸时 fail-closed，Worker 不启动 | P0 |
| EXT-019 | Package 到 Runtime 精确租约 | 只有复验返回的 `ValidatedSkillManifest` 可安装进 Host；授权与激活后 `active_manifest` 必须逐字段一致 | P0 |
| EXT-020 | Skill 包库存边界 | 缺少动态 entrypoint、重复/保留/非 UTF-8 路径、超过 256 文件或 16 MiB 均在切换 active 前拒绝 | P0 |
| EXT-021 | Skill 授权与启用持久化 | 安装不自动授权；授权绑定精确版本和完整 Capability 集，未授权不能保存 enabled；升级替换旧授权，删除状态同时撤销恢复资格 | P0 |
| EXT-022 | Desktop Skill 生命周期 IPC | 安装后为未授权且停用；授权后仍停用；启用后才 Activated；停用立即撤销 Contribution 租约 | P0 |
| EXT-023 | Desktop Skill 启动恢复复验 | 重启逐包复验完整库存、版本和 Capability；只有 authorized + enabled 的健康包恢复 Activated | P0 |
| EXT-024 | Desktop Skill 篡改恢复 | 已启用 Skill 被篡改、缺失或状态不匹配时重启不进入 Host，目录仅返回脱敏 unhealthy 状态 | P0 |
| EXT-025 | Desktop Skill 升级与回滚授权 | 升级或回滚后强制清零授权和启用状态，旧版本授权不得跨版本复用 | P0 |
| EXT-026 | Recovery Mode 扩展隔离 | 不读取正常 Skill Store、不恢复 Worker 或 Contribution，所有 Skill 写 IPC fail-closed | P0 |
| EXT-027 | Skill 双向模块调用 | Worker Command 计划只能进入共享 Command Registry、风险批准与 Capability Gateway；Agent Task 计划只能凭激活 requester 进入 Module Adapter | P0 |
| EXT-028 | Desktop Skill Worker 执行 | `execute_skill` 重新复验 active 包，Worker 请求 Manifest 必须与当前 Activated 租约逐字段一致，未激活、篡改或未声明 activation event 时进程前拒绝 | P0 |
| EXT-035 | Skill 事件订阅授权 | Manifest 声明任一 `onEvent:*` 必须同时声明 `subscribe-events`；缺失 Capability 在安装验证阶段拒绝，授权精确绑定包含该 Capability 的版本 | P0 |
| EXT-036 | Skill Runtime Event 自动调度 | Activated Skill 仅订阅 Manifest 中 `onEvent:*` 的精确事件类型；独立 32 项队列、串行 Worker 调度，事件以版本化 JSON 输入传入且不暴露 Event Bus | P0 |
| EXT-037 | Skill 事件会话生命周期 | 停用、升级、回滚、Host 重建、Safe Mode 或 Worker 故障撤销订阅并取消在途 Worker/Provider；旧线程迟到退出不得删除替代会话 | P0 |
| EXT-038 | Skill Agent Tool Manifest 门禁 | Tool ID 必须属于 Skill 命名空间，Schema 与元数据有界，映射命令必须在精确 `commandAllowlist`，并要求独立 `contribute-agent-tools` Capability | P0 |
| EXT-039 | Skill Agent Tool 风险复核 | Desktop 构建 Registry 时比较 Tool 声明风险与宿主命令真实风险；任何低报、未知命令或 ID 冲突在 Provider/Gateway 前拒绝 | P0 |
| EXT-040 | Skill Agent Tool 动态执行与撤销 | Activated Tool 出现在 Catalog、Provider allowlist 和独立 Tool 入口，参数绑定批准后只经共享 Capability Gateway 执行；停用、升级、回滚或重建后立即消失 | P0 |
| EXT-029 | Desktop Skill Agent 回执 | Agent 计划固定使用 `Module + skill:<id> + draft + no-tools`，上下文注入检测、Provider allowlist、Agent History 与用户程序共用生产链路 | P0 |
| EXT-030 | Skill Command 整批准入 | Worker 计划必须逐项命中精确 `commandAllowlist` 与宿主风险注册；Safe/Low 直接经 Module Gateway 执行，Medium/High 返回完整参数与风险并进入五分钟一次性整批批准；未知、未声明、拒绝、过期或重复批准在任何副作用与 Agent Task 前失败 | P0 |
| EXT-031 | Skill Command Manifest 授权 | `commandAllowlist` 只接受有界 `safe.*` 标识并要求 `invoke-commands`；升级变更 allowlist 后安装状态回到未授权、停用 | P0 |
| EXT-032 | Skill Command 因果回执 | 同次执行共享宿主生成 Trace，每条命令获得稳定幂等键与结构化 Gateway 回执，不能由 Worker 覆盖执行身份或 Trace | P0 |
| EXT-033 | Skill 批准持久恢复 | pending 计划重启后仍可通过列表 IPC 查询和决策；遗留 executing 必须变为 interrupted，过期项不可批准 | P0 |
| EXT-034 | Skill 批准并发终态 | 同一批准仅一个调用能原子 claim；拒绝、完成、失败、过期和中断均为不可逆终态，不得产生重复模块副作用 | P0 |
| SCRIPT-001 | 用户脚本调用已授权 Command | 正常执行并产生 Run、Trace 和审计记录 | P0 |
| SCRIPT-002 | 脚本访问未授权文件、网络、进程 | Host 拒绝，不能绕过 Capability Broker | P0 |
| SCRIPT-003 | 死循环、内存泄漏、事件递归 | 对应实例被限流或终止，Core 继续运行 | P0 |
| SCRIPT-004 | 脚本升级扩大权限 | 保持停用并要求重新授权，可回滚旧版本 | P0 |
| SCRIPT-005 | 已安装程序缺少版本授权 | Worker 不启动，返回权限待确认；授权后仅精确版本与完整 Capability 集合可运行 | P0 |
| SCRIPT-006 | 撤销程序权限 | 所有历史版本授权被删除，后续正式执行在 Worker 启动前失败 | P0 |
| SCRIPT-007 | 多程序订阅同一事件 | 各会话独立收到事件，UI drain 和其他程序 drain 不相互消费 | P0 |
| SCRIPT-008 | 订阅消费者持续落后 | 队列保持 64 条上限、丢弃最旧事件并准确报告 dropped，不阻塞 Core | P0 |
| SCRIPT-009 | 安全模式、撤权、升级或回滚 | 对应事件会话立即取消，旧订阅 ID 不再可读取 | P0 |
| SCRIPT-010 | Renderer 伪造事件 | IPC 不接受事件正文，过滤器只能来自已安装 Manifest | P0 |
| SCRIPT-011 | 可信事件执行 | 每次只消费 Rust 队列最旧一条，保留余下事件；执行前复验 active 版本完整性与精确权限，并只读注入 `nimora.input.trigger` | P0 |
| SCRIPT-012 | 自动事件循环三策略 | `serial` 有界保留最新待执行事件且失败后不继续队列，`drop` 忙时丢新事件，`cancel-previous` 取消旧 Worker 并立即启动最新事件 | P0 |
| SCRIPT-013 | 后台事件循环 | 同一会话重复启动不创建重复线程；关闭、撤权、升级、回滚及安全模式后循环退出，状态接口保留执行数、订阅与调度丢弃总数及最后错误 | P0 |
| SCRIPT-014 | Manifest 队列容量 | Rust 订阅实际采用 `eventQueueCapacity`；非法容量安装失败，Renderer 无法覆盖容量 | P0 |
| SCRIPT-015 | `cancel-previous` 迟到完成隔离 | 连续替换 Worker 后，旧完成不增加 executed、不覆盖 last_error、不终止当前执行 | P0 |
| SCRIPT-016 | 会话撤销与 Worker 强制取消 | 对无限循环 Worker 关闭会话、撤权、升级、回滚或进入安全模式，在 Manifest 超时前强杀回收，且无遗留 active 注册 | P0 |
| SCRIPT-017 | 程序本地数据隔离 | 程序 A 只能读写自己的命名空间，不能通过请求指定程序 B 身份 | P0 |
| SCRIPT-018 | 本地数据权限 | 缺少 `store-local-data` 时读写删均被 Gateway 拒绝 | P0 |
| SCRIPT-019 | 本地数据配额与原子性 | 单值、总配额、覆盖旧值、异常中断和符号链接场景均保持不变量 | P0 |
| SCRIPT-020 | 结构化计划总预算 | 命令与存储操作合计超过 32 时在 Gateway 前拒绝，后续操作不执行 | P0 |
| SCRIPT-005 | 离线、休眠、时钟回拨 | 按 missed-run policy 执行，无任务风暴 | P1 |

## 11. Gateway 与鉴权

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| GAT-001 | 默认状态 | 新安装 Gateway 关闭，无监听端口 | P0 |
| GAT-002 | 本地配对 | 用户确认后发放最小 Scope Token | P0 |
| GAT-003 | Scope 不足 | 返回 403，Command 不执行，审计完整 | P0 |
| GAT-004 | Token 撤销 | 已连接客户端及时失去访问能力 | P0 |
| GAT-005 | 过期 Token | 返回标准错误，不泄露内部信息 | P0 |
| GAT-006 | 局域网监听 | 必须显式确认并展示当前地址和风险 | P0 |
| GAT-007 | WS/SSE 断线 | 释放订阅和资源，重连不重复泄漏 | P1 |
| GAT-008 | 速率限制 | 超限返回明确信息并写审计 | P0 |

## 12. Connector

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| CON-001 | HTTP Sink 成功 | 收到标准 envelope，审计记录 delivery ID | P0 |
| CON-002 | HTTP 429 | 遵守 `Retry-After`，不阻塞 Event Bus | P0 |
| CON-003 | HTTP 非幂等重试 | 事件 ID 不变，delivery ID 变化 | P0 |
| CON-004 | UDP 超包 | 按策略拒绝或分片；默认不静默截断 | P0 |
| CON-005 | SSE Source | 外部数据规范化为新本地事件 | P0 |
| CON-006 | 重复 external ID | 根据配置去重并显示统计 | P1 |
| CON-007 | 未授权目标 | DNS/IP/端口不符合策略时拒绝连接 | P0 |
| CON-008 | 脱敏 | 坐标、对话和密钥字段按分类移除 | P0 |
| CON-009 | 熔断恢复 | 达阈值后停止请求，冷却后半开测试 | P1 |
| CON-010 | Profile 切换 | Connector 按 Profile 启停且无残留连接 | P1 |

## 13. AI Agent

- Automation Live Run 的 `agent.task.run` 必须在 Submitter 前经过调用方、Provider、Tool、数据等级、主动性、调用深度和父级剩余预算准入。
- 标记为不可信的 Automation 动态上下文在 Context Admission 与注入检测完成前必须 fail-closed，且永久拒绝不得重试。
- Automation AI Action 必须要求 Medium 以上风险和稳定幂等键；普通 Command 必须继续进入原 Automation Backend。
- Automation 规则不能声明或扩大宿主准入时间与根剩余预算；伪造 `nowMs`、`rootRemainingBudget` 或其它未知字段必须在 Submitter 前拒绝。
- Automation 动作、重试和补偿必须获得同一 `runId/traceId`，并带有精确 `automationId/actionId/eventId`；AI 子任务根 ID 必须绑定 Automation Run 而非随机 Action Command。
- Live Automation 必须在任何副作用前持久化 `running`；完整结果只能从同身份 `running` 原子转为 `completed`，重复完成或身份错配拒绝。
- 桌面重启必须把遗留 `running` 标记为 `interrupted` 且保留 Run/Automation/Event/Trace 身份；已完成记录不得被恢复流程改写。

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| AGT-001 | Safe Tool | 展示计划并按用户设置执行 | P0 |
| AGT-002 | Danger Tool | 无论模型建议如何都逐项确认实际参数 | P0 |
| AGT-003 | Prompt Injection | 外部内容不能改变系统策略或扩大权限 | P0 |
| AGT-004 | Provider 超时 | 任务可取消，核心和已有状态不受影响 | P0 |
| AGT-005 | 最大步骤 | 达限制后停止并解释未完成部分 | P0 |
| AGT-006 | 部分失败 | 展示已完成、失败和已回滚步骤 | P0 |
| AGT-007 | 无 Key 降级 | 本地命令和规则仍可执行 | P1 |
| AGT-008 | 记忆策略 | 禁止记忆的数据不进入存储或请求上下文 | P0 |
| AGT-009 | Provider 数据预览 | 可查看将发送的数据分类和目标 Provider | P1 |
| AGT-010 | 保存为自动化 | 生成规则不包含未授权临时参数或密钥 | P1 |
| AGT-011 | 模块注册 Tool | 重复/非法 ID、超限 Schema 和无效超时被拒绝，模块内部对象不暴露给 Agent | P0 |
| AGT-012 | 实际参数风险提升 | 参数或环境提高风险后必须重新确认，模型声明和基础 Manifest 不能降低风险 | P0 |
| AGT-013 | 批准参数绑定 | Tool ID、参数、风险、任务或 Trace 变化后旧批准立即失效 | P0 |
| AGT-014 | 其它模块创建任务 | Automation、Skill 和宿主模块只能用获准 Provider、Tool allowlist、数据分类和预算创建任务 | P0 |
| AGT-015 | CLI 非交互确认 | 需确认操作返回结构化错误且不执行；`--yes` 不能覆盖写入、外部副作用或 Medium 以上风险 | P0 |
| AGT-016 | Provider 离线边界 | `--offline` 在 Adapter 调用前拒绝网络 Provider，本地 Provider 可继续运行 | P0 |
| AGT-017 | Provider 畸形 Tool Call | 未注册 Tool、非对象参数、错配 Request ID 和 Finish Reason 不一致均 fail-closed | P0 |
| AGT-018 | AI 调用模块 | Provider Tool Call 只生成待门禁 Invocation；未确认写操作不扣工具执行预算且不进入 Backend | P0 |
| AGT-019 | 跨任务调用隔离 | Task ID 或 Trace ID 不匹配的 Invocation 在 Capability Gateway 前拒绝 | P0 |
| AGT-020 | 单步协调恢复 | Provider 与 Tool 每次仅推进一个确定性步骤，暂停或崩溃后不会隐式重放副作用 | P0 |
| AGT-021 | CLI stdout/stderr | 成功时 stdout 仅一个 JSON 文档且 stderr 为空；失败时 stdout 为空且 stderr 为稳定 JSON 错误 | P0 |
| AGT-022 | CLI 离线 stdin | 256 KiB 内任务可由 stdin 离线执行；超限、未知字段和缺失 `--output json` 被稳定拒绝 | P0 |
| AGT-023 | Ollama Worker 隔离 | Provider Registry 经真实 sidecar 访问 loopback mock，桌面 Core 不直接建立 HTTP 连接 | P0 |
| AGT-024 | Ollama SSRF 边界 | IPv4/IPv6 loopback 可用；非 loopback、零端口、凭据和超时越界在连接前拒绝 | P0 |
| AGT-025 | Worker 输出背压 | stdout 被并发有界读取；超限、畸形 JSON、异常退出、超时和取消均终止且不泄漏传输细节 | P0 |
| AGT-026 | Ollama Tool Call | Function name/arguments 转为 Runtime Tool Call，后续仍经过 Tool Registry 和 Capability Gateway | P0 |
| AGT-027 | Provider Sidecar 信任 | Manifest 名称、可信摘要、Provider ID、协议、普通文件、根目录约束、文件大小和 Worker 摘要任一不符均在启动前 fail-closed | P0 |
| AGT-028 | CLI Ollama 发现 | Sidecar root 与可信 Manifest 摘要必须成对提供；缺少 Sidecar、摘要无效和完整性失败分别返回稳定机器错误且 stdout 为空 | P0 |
| AGT-029 | 生产 Tool Catalog | CLI 与 Provider 请求获得相同十项模块工具；Descriptor 的 Schema、风险和副作用稳定且不暴露内部对象或任意命令入口 | P0 |
| AGT-046 | Agent 资源目录读取 | `asset.catalog.read` 只经显式 Gateway 读能力返回已验证资产摘要，空参数外输入拒绝，不暴露资源根路径 | P0 |
| AGT-047 | Agent 运行健康读取 | `runtime.health.read` 只返回启动、安全、Outbox 与备份健康摘要，不含日志、正文、路径、密钥或任意诊断文件 | P0 |
| AGT-048 | 可扩展读能力策略 | Agent Task/Trace 必须精确匹配，未列入 `read_capabilities` 的读取在 Backend 前拒绝；用户程序不自动继承 Agent 专用读能力 | P0 |
| AGT-049 | Ollama Worker 双轮闭环 | 真实独立 Worker 首轮解析结构化 Tool Call；宿主按原 Call ID 回填 Tool Result 后再次经过 Worker 请求 `/api/chat`，最终回答、Finish Reason 与关联载荷均正确 | P0 |
| AGT-050 | Agent 历史仓储 | 完成任务以 Task ID 只写一次；版本化载荷、内容上限、稳定时间游标分页、单条删除与全部删除均可验证，删除不影响运行状态 | P0 |
| AGT-051 | 桌面历史生命周期 | 无工具任务与完整工具 Turn 只在最终成功后写入；等待、拒绝和取消不写入；历史写失败只展示降级，不改变任务结果或重复工具副作用 | P0 |
| AGT-052 | 桌面历史 IPC 与 UI | 成对游标校验、有界分页、单条/全部删除、Recovery 内存隔离、空状态、最近五条和清除反馈正确；Prompt/Response 不进入诊断包或 Agent Tool Catalog | P0 |
| AGT-053 | CLI 历史生命周期 | `run --history-database` 仅持久化最终完成任务，失败返回独立 degraded 状态但任务仍成功；`history export|delete` 使用显式数据库、稳定 JSON、成对游标、1..200 上限及单条/全部删除 | P0 |
| AGT-054 | Agent 角色状态读取 | `character.state.read` 只经显式 Agent Gateway Capability 返回当前角色与渲染能力摘要；模型路径、资源 URL、仓储对象和用户程序权限均不暴露 | P0 |
| AGT-055 | Agent 动作能力发现 | `pet.action.catalog.read` 从 Runtime 当前 `PetAction` 词汇返回精确动作列表及对应写工具；普通用户程序不继承该 Agent Capability | P0 |
| AGT-056 | Agent Profile 切换 | `profile.active.switch` 必须绑定实际 Profile ID 批准，只映射到 `safe.profile.switch`；桌面预应用原生窗口策略，持久化失败回滚原生策略，无原生上下文时零状态写入 | P0 |
| AGT-057 | Agent 角色切换 | `character.active.switch` 必须绑定实际 Asset ID 批准，只映射到 `safe.character.switch`；仅激活内置或复验通过的 Character，刷新失败回滚原选择，无原生上下文时零状态写入 | P0 |
| AGT-058 | Agent 程序目录 | `program.catalog.read` 只返回完整性复验通过的已安装程序身份、声明、预算和精确版本授权摘要；损坏项只计数，不暴露源码、安装路径、Worker 路径或系统句柄；普通用户程序不继承该 Agent Capability | P0 |
| AGT-059 | Agent 已安装程序执行 | `program.installed.execute` 必须绑定 `programId + version` 批准，只映射到 `safe.program.execute`；执行前重验 active 安装、完整性、精确版本和持久授权，仅经隔离 Worker 与 Capability Gateway 执行，无原生上下文时零副作用 | P0 |
| AGT-060 | 持久 Goal 与 Plan 分离 | Goal 可跨会话恢复，Plan 可演进但不能单独证明 Goal 完成；完成必须关联逐项证据 | P0 |
| AGT-061 | Auto Mode 权限不扩张 | 自动循环只能在 Capability、Tool、数据、费用、时间、步骤和并发预算交集内推进，不能借 `--yes` 绕过写入或数据出境确认 | P0 |
| AGT-061A | Provider 推理能力声明 | 推理集合非空且不包含 `auto`；Mapping Version 为空、超长或漂移时在 Adapter 调用前失败关闭 | P0 |
| AGT-061B | 推理请求前置复验 | 未声明能力、actual effort 越界或 Mapping Version 不一致不得调用 Provider、产生网络请求或费用 | P0 |
| AGT-061C | 推理缓存身份隔离 | 消息、模型、Plan 与 Workspace 相同时，改变 actual effort、Provider value 或 Mapping Version 仍生成不同缓存键且不能命中旧条目 | P0 |
| AGT-062 | Checkpoint 安全恢复 | 恢复保留任务因果、预算和结果摘要，但旧批准证明、原生句柄与版本已变化的工具租约必须失效 | P0 |
| AGT-063 | 上下文压缩可追溯 | 压缩前后保留 Goal、约束、未完成项、关键证据和来源引用；不可信正文不能被提升为系统约束 | P0 |
| AGT-064 | 多 Agent 权限与预算继承 | 子 Agent 继承 Trace、最小 Tool allowlist 和父级剩余预算，不能重置深度、费用或主动性，也不能读取兄弟任务私有上下文 | P0 |
| AGT-066 | Agent 自动化定义验证 | `automation.definition.validate` 仅接受对象定义、事件类型和对象事件数据；经专用 Agent Gateway Query 调用自动化 Dry-run，返回计划或不匹配状态且尝试次数为零，不创建确认项、不调用 Command Backend；普通用户程序不继承该能力 | P0 |
| AGT-067 | 用户程序创建模块 Agent 任务 | 未声明 `invoke-agent-tasks` 时在 Provider 前拒绝；精确版本授权后固定使用 `Module` Origin、`program:<id>` requester、`draft`、空 Tool Allowlist 与宿主预算，结果进入回执和 Agent History | P0 |
| AGT-068 | 用户程序不可信 Context Admission | `context[]` 计入统一 32 操作预算并通过共享来源、段数、字节和注入检测；Prompt Injection 不进入 Provider/History，诊断只含来源类别及 Trace/Module/Execution 关联 ID | P0 |
| AGT-069 | 用户程序 Agent 审计故障 fail-closed | Context 被拒绝但安全 Journal 不可写时返回审计不可用，Provider、History 与模块 Backend 零调用，攻击正文和密钥不出现在序列化诊断中 | P0 |
| AGT-070 | 通用 Module Agent Adapter | Program、Skill、Connector 共用 Adapter 固定 `Module + Personal + draft + no-tools`；越权 Provider 在 Context/Provider 前拒绝，合法 Context 分离为 trusted instruction 与 untrusted data message | P0 |
| AGT-071 | Module Context Trace 相关性 | Adapter 先生成 Gateway Task/Trace 再做 Context Admission；拒绝错误携带同一 Trace 和无正文审计，宿主不得在准入后修改 Task Trace | P0 |
| AGT-072 | Goal 双表事务仓储 | Goal 当前状态与不可变 Plan 修订分表持久化；创建、修订和状态变化保持事务一致，陈旧修订、跨 Goal 绑定、索引元数据/Payload 不一致与未知版本 fail-closed | P0 |
| AGT-073 | Goal CLI 跨进程闭环 | `goal create|list|show`、`goal plan replace`、`goal status set` 使用显式数据库和有界 JSON；跨进程恢复修订，缺少逐步证据时完成失败且 stdout 为空，补齐证据后完成成功 | P0 |
| AGT-074 | Auto Mode 逐步准入 | Safe/Low 只读且在精确 Tool/Data/预算范围内才可继续；未知 Tool、超限数据、Medium+ 风险、任何写入或外部副作用均在调用前结构化暂停 | P0 |
| AGT-075 | Auto Mode 恢复绑定 | Resume 必须重新匹配 Goal、当前 Plan 修订、Workspace revision 和 Policy fingerprint；任一变化都不能恢复执行 | P0 |
| AGT-076 | Auto Mode 会话仓储 | Payload 与 Goal、Plan、状态、暂停原因和时间索引一致；陈旧更新冲突，同一 Goal 不允许两个 running Session | P0 |
| AGT-077 | Auto Mode 重启安全 | 持久 Running Session 在宿主重启后转为 `paused/restarted`，不得自动调用 Provider、Tool 或复用旧批准 | P0 |
| AGT-078 | Auto Mode CLI 控制面 | `goal auto start|status|pause|resume|cancel` 使用显式数据库和有界 JSON；跨进程保持状态，绑定变化时 stdout 为空且 stderr 为稳定 JSON 错误 | P0 |
| AGT-079 | Auto Mode 整轮工具预检 | Provider Turn 的全部 Tool 在首个 Backend 调用前完成 allowlist、数据、风险、副作用和预算预检；任一调用需确认时整轮零 Tool 副作用，全部安全只读时才按原顺序执行并生成严格关联 continuation | P0 |
| AGT-080 | Auto Mode Checkpoint CAS | Checkpoint 只保存有界 Task、Provider continuation 和 Goal/Plan/Workspace/Policy 绑定，不含 Approval 或宿主对象；SQLite 每 Session 仅保留最新序号，跳号、陈旧替换、未知版本和索引/Payload 不一致均 fail-closed | P0 |
| AGT-081 | 上下文压缩协议完整性 | 压缩必须保留全部可信 System 消息与结构化 Goal/约束/待办/证据 Anchor；Assistant Tool Call 与对应 Tool Result 作为原子单元保留或整体移除，预算不足时拒绝而非截断 | P0 |
| AGT-082 | Context Cache 隔离与治理 | Cache Key 绑定 Provider、模型、Plan revision、Workspace fingerprint 与压缩消息；TTL 到期不可命中，容量超限按 LRU 淘汰，不允许跨资源版本复用 | P0 |
| AGT-083 | Workspace 文件版本链 | Snapshot 拒绝绝对路径、逃逸、反斜杠、重复路径、超限文件和篡改指纹；后继 revision 必须绑定父指纹，并稳定输出 Added/Modified/Deleted 和可执行位变化 | P0 |
| AGT-084 | 宿主安全 Workspace 扫描 | canonical root 下不跟随 symlink，遵循 Git、通用与 Nimora ignore，并受文件数、单文件、总字节、深度和墙钟限制；读取期间 metadata 或文件身份变化 fail-closed | P0 |
| AGT-085 | Git 工作区版本检查 | 无 Shell 调用读取 HEAD commit/tree、index tree、分支 ahead/behind 以及 staged/unstaged/untracked/conflict；超时、超量输出、非仓库和畸形协议稳定失败 | P0 |
| AGT-086 | Workspace CLI 信息边界 | `ai workspace inspect` 输出稳定 JSON、相对路径、快照与 Git 指纹，不返回 canonical 绝对根路径；revision 大于一时强制绑定父 fingerprint | P0 |
| AGT-087 | Workspace 快照持久版本链 | SQLite 仅允许 revision 1 创建；后继 revision、parent fingerprint、root fingerprint 和前序 fingerprint 必须 CAS 匹配，读取时索引元数据与 Payload 不一致、未知版本或陈旧追加均 fail-closed | P0 |
| AGT-088 | Auto Mode 启动真实扫描绑定 | `goal auto start` 只接受 `workspaceRoot`，由宿主扫描 revision 1、持久化快照并将 Policy 绑定其 fingerprint；调用方不能提交自定义 workspace revision | P0 |
| AGT-089 | Auto Mode 恢复漂移阻断 | `resume --workspace-root` 对相同根与相同内容成功恢复；根变化立即拒绝，内容变化持久化绑定旧 fingerprint 的后继快照并返回 `workspace-changed`，Session 保持 paused | P0 |
| AGT-090 | 持久 Context Cache 隔离治理 | SQLite Cache 命中必须复验内容地址、Provider、模型、Plan、Workspace、数据等级、TTL 与索引/Payload；过期项删除，容量超限按稳定 LRU 淘汰，旧 Workspace 可显式整体失效且不影响其他分区 | P0 |
| AGT-091 | Checkpoint 安全恢复应用服务 | 恢复候选跨 Goal、Session、Checkpoint、Workspace 仓储复验 ID、Plan revision、Policy 与 Workspace fingerprint，并真实重扫根目录；只返回 paused Task 和 continuation，不调用 Provider/Tool、不携带 Approval，文件漂移时零 continuation 释放 | P0 |
| AGT-092 | Auto Mode 显式恢复原子提交 | 只有显式 Resume 才可把 Session 与 Checkpoint Task 同时恢复为 running；提交使用 Session timestamp 与 Checkpoint sequence 双 CAS，任一竞争或陈旧写入必须整体回滚，且提交阶段不得调用 Provider/Tool 或恢复 Approval | P0 |
| AGT-093 | Auto Host 持久上下文缓存接入 | 恢复轮次先按有界策略压缩完整 Provider 协议单元，再以 Provider、模型、Plan revision、Workspace fingerprint 与压缩消息内容查询 SQLite；仅精确身份与数据等级允许命中，miss 写入受 TTL/LRU/容量治理的持久条目 | P0 |
| AGT-094 | Auto Mode 每轮 Workspace 漂移门禁 | 每次 Provider 轮次前必须真实有界重扫 Workspace；未变化才释放 continuation，变化时在单一事务中以 Session timestamp、Checkpoint sequence、Workspace revision/fingerprint 三重 CAS 同时暂停 Session/Task 并追加 successor，任何竞争整体回滚 | P0 |
| AGT-095 | Auto Mode 单轮结果原子提交 | Continue、Paused、Completed 必须将 Session/Task 生命周期、完整 continuation 与 Checkpoint sequence 以 Session timestamp + Checkpoint sequence 双 CAS 原子提交；陈旧结果整体回滚，Provider/Tool 已执行后的提交失败标记为不确定且禁止自动重放 | P0 |
| AGT-096 | Auto Mode durable Turn Attempt | Provider/Tool 前创建精确绑定 Session、Checkpoint sequence、Session timestamp 与请求指纹的唯一 Attempt，禁止过期重领；结果事务必须原子消费 Attempt，提交失败或崩溃遗留转为 indeterminate，恢复不得释放 continuation 或自动重放 | P0 |
| AGT-097 | Auto Host 持久单轮执行流水线 | 真实 Workspace 预检、持久 Context Cache、durable Attempt、Provider/Tool Supervisor 与结果 Commit 按序组合；安全只读 Tool 实际派发并持久 continuation，写 Tool 整批零派发暂停，Provider 失败隔离为 indeterminate，Workspace 漂移在 Provider 与 Attempt 前退出 | P0 |
| AGT-098 | Context Cache 系统密钥加密 | SQLite Payload 不得出现 Prompt、消息或 Workspace 明文；每条记录使用随机 nonce 的 XChaCha20-Poly1305，AAD 绑定 Cache Key、Provider、模型、Workspace、Plan revision、数据等级、创建与过期时间；错误密钥、密文或元数据篡改必须 fail-closed，旧明文版本失效删除，Secret Store 不可用时禁止明文降级 | P0 |
| AGT-098 | 桌面 Auto Mode 持久单轮控制入口 | Tauri IPC 必须复用生产 Provider/Tool Registry 与 Capability Gateway，显式 Resume 后只执行一个有界 Turn；默认离线并填充稳定预算，Safe/Recovery Mode 与非法预算在 Provider 前拒绝，TypeScript 平台契约精确映射版本化请求和结果 | P0 |
| AGT-099 | Auto Host 公平有界监督循环 | 每批只允许 `1..=256` 个持久 Turn；Continue 可进入下一轮，Completed、Paused、Workspace Drift 或错误必须立即停止；达到宿主上限返回 `yielded` 并保持 Running，不伪造业务暂停，下一批仍经过每轮 Workspace/Attempt/Commit 门禁 | P0 |
| AGT-030 | Gateway 固定映射 | Agent 写工具无批准时不调用 Backend；批准后只映射到固定安全命令，并携带 Task、Trace 与 Invocation 幂等键 | P0 |
| AGT-031 | Agent Gateway 关联隔离 | Gateway Policy 的 Task 或 Trace 与 Invocation 不一致、命令不在 allowlist、Agent 请求程序私有存储时均在 Backend 前拒绝 | P0 |
| AGT-032 | 桌面离线工作台 | 桌面展示同一生产 Tool Catalog、风险与确认要求；确定性 Provider 在无网络和无凭据时回显任务、完成状态、Token 与零费用 | P0 |
| AGT-033 | 桌面 Agent 输入边界 | 空 Prompt 与超过 32768 bytes 的 Prompt 在 Provider 前拒绝，且不触发模块 Backend | P0 |
| AGT-034 | 桌面参数绑定确认 | 写工具在确认前无副作用；宿主不向前端暴露 Approval；确认后只经固定 Gateway 映射执行一次，再次确认失败 | P0 |
| AGT-035 | 桌面拒绝与强停 | 拒绝、过期、Safe Mode 和 Recovery Mode 均撤销或拒绝待确认 Invocation；退出 Safe Mode 后旧确认不可恢复 | P0 |
| AGT-036 | Provider Tool Result 关联 | 续跑消息保留 Assistant Tool Call；Tool Result 必须匹配先前 Call ID 与 Tool ID，孤立、错配和重复结果均在 Provider Adapter 前拒绝 | P0 |
| AGT-037 | Ollama Tool 续跑载荷 | Worker 向真实 loopback `/api/chat` 发送结构化 Assistant Tool Call 及关联 Tool Result，而不是无关联文本；响应继续经过有界协议校验 | P0 |
| AGT-038 | 多调用 Turn 原子续跑 | 同一 Provider Turn 按原始顺序聚合结果；缺失、未知、工具错配或重复结果均不能生成下一 Provider Step 的消息 | P0 |
| AGT-039 | 桌面多写调用原子批准 | 同一 Provider Turn 的写调用全部批准前均无副作用；任一拒绝或过期级联撤销整组，全部批准后才按原始顺序经 Gateway 执行并续跑 | P0 |
| AGT-040 | 桌面确认组续跑 UI | 等待态展示全部实际参数；部分批准仅保留剩余项；最后批准回填同一 Provider 的最终回答；拒绝不展示部分 Tool Result | P0 |
| AGT-041 | 桌面 Ollama Worker 发现 | 构建嵌入可信 Manifest 摘要；运行时 Manifest 或 Worker 缺失、换包、篡改、符号链接或越界均不注册 Provider；验证通过才加入 Catalog | P0 |
| AGT-042 | 桌面 Provider 与模型选择 | UI 只列 Registry Provider；任务携带显式 Provider ID 与模型；未知 Provider、空值和超过 128 bytes 的模型在 Adapter 前拒绝 | P0 |
| AGT-043 | Ollama 健康与模型目录 | `/api/tags` 只能经受验证 Worker 访问 loopback；模型去重排序，非 200、chunked、长度错配、超预算和畸形字段 fail-closed | P0 |
| AGT-044 | Provider 状态语义 | Worker 验证、服务可达和模型可用分别展示；服务离线或模型未安装时禁用运行，切换 Provider 清除旧状态 | P0 |
| AGT-045 | 安全模式探测边界 | Safe/Recovery Mode 不启动 Provider Worker；状态返回稳定不可用且不泄漏路径、摘要或原始网络错误 | P0 |

## 14. 信任中心与安全模式

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| TRU-001 | 活动总览 | 正确展示扩展、端口、连接、权限和 Agent 任务 | P0 |
| TRU-002 | 撤销权限 | 后续调用立即失败，运行中任务按策略停止 | P0 |
| TRU-003 | 数据预览 | 展示目标、类型、敏感字段和频率 | P0 |
| TRU-004 | 安全模式 | 2 秒内关闭 Gateway、Connector、Agent 和第三方 Host | P0 |
| TRU-005 | 安全模式恢复 | 用户逐项恢复，不自动恢复高危能力 | P0 |
| TRU-006 | 审计脱敏 | 导出文件不包含 Token、Key 和默认受限正文 | P0 |
| TRU-007 | Safe Mode 提交后隔离收敛 | 任一早期隔离、策略缓存或 Renderer 通知失败时仍按固定顺序尝试所有后续步骤；Safety 保持 Safe，返回值只含固定步骤码，并尽力记录 `safe-mode-convergence-failed` Security 事件 | P0 |

## 15. Profile、备份与迁移

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| CFG-001 | Profile 优先级 | 安全策略不能被 Profile 或扩展覆盖 | P0 |
| CFG-002 | 配置导出 | 不包含真实密钥，包含版本和依赖摘要 | P0 |
| CFG-003 | 合并导入 | 冲突预览准确，可取消且不改变现状 | P1 |
| CFG-004 | 损坏配置 | 备份原文件，恢复默认或最近快照 | P0 |
| CFG-005 | Schema 升级 | 迁移幂等，重复启动结果一致 | P0 |
| CFG-006 | 磁盘写入失败 | 不覆盖有效配置，提示恢复方案 | P0 |
| CFG-007 | Work 场景能力 | 切换为 `work` 只应用用户配置与默认呈现，不永久隐藏或拒绝已授权能力 | P0 |
| CFG-008 | 场景类型契约 | Profile 必须包含合法 `mode`；缺失或未知类型拒绝且不覆盖当前配置 | P0 |
| CFG-009 | 场景类型扩展 | Companion、Work、Focus、Creator、Developer、Presentation、Offline 均可保存、恢复和切换 | P1 |
| CFG-010 | Outbox 租约与 ACK | 并发领取不重复占有；租约过期可重领；旧所有者 ACK/失败回报被拒绝；成功进入 delivered | P0 |
| CFG-011 | Outbox 重试与死信 | 失败在 `availableAt` 前不可重领，达到最大尝试次数进入 dead-letter，死信不会自动再次投递 | P0 |
| CFG-012 | Outbox 健康与清理 | Control Center 只显示状态计数且 Pet 窗口无权读取；清理有批量上限并且只删除截止时间前的 delivered 记录 | P0 |
| CFG-013 | 自动备份调度 | 每 15 分钟检查；未满 6 小时不重复创建，达到间隔后生成经 Schema/完整性验证的一致备份 | P0 |
| CFG-014 | 备份轮换 | 默认只保留 12 份，但挂起恢复指向的旧备份不可被删除；UI 不暴露绝对路径 | P0 |
| CFG-015 | 安全恢复 | 运行期只写恢复请求；下次启动首个 SQLite 连接前 staged 验证并原子激活，成功后状态与备份一致 | P0 |
| CFG-016 | 损坏恢复输入 | 非法 JSON、未知 Schema、路径逃逸、未知 ID 或损坏 SQLite 均拒绝，当前数据库与有效恢复请求不被静默破坏 | P0 |
| CFG-017 | 备份写入故障 | 临时文件、校验、同步、发布或轮换失败时不覆盖现有数据库和已发布备份，并在设置页显示最近错误 | P0 |
| CFG-018 | 损坏数据库故障启动 | 写入损坏 SQLite 后启动；进入数据恢复模式，原文件 hash 不变，自动备份和所有正常写操作被拒绝 | P0 |
| CFG-019 | 恢复模式受控恢复 | 恢复模式仍可选择已验证备份并写入耐久恢复请求；完全退出并重启后原子激活，临时内存状态不落盘 | P0 |
| CFG-020 | 恢复模式信息边界 | UI 展示隔离状态、原数据库保留及重启要求；不暴露数据库绝对路径和原始 SQLite 错误 | P0 |
| CFG-021 | 脱敏诊断预览 | 正常、安全和恢复模式均可预览；只含版本、模式、Schema 与计数，不含正文、密钥、路径和数据库内容 | P0 |
| CFG-022 | 诊断包原子导出 | 生成 ZIP、验证 manifest SHA-256；目标已存在、写入或发布失败时拒绝覆盖并清理临时文件 | P0 |
| CFG-023 | 诊断包无自动上传 | 断网可完整导出，导出过程不创建网络请求，UI 明确说明由用户自行决定是否分享 | P0 |
| CFG-024 | 结构化事件选择 | 预览显示条目数；用户取消后 ZIP 不含 `events.json`，选中后仅含固定事件码/等级/组件/时间且 manifest hash 匹配 | P0 |
| CFG-025 | 诊断事件预算 | 内存日志最多 256 条并丢弃最旧项；任一可选来源超过独立字节预算时拒绝整个导出且不发布目标 | P0 |
| CFG-026 | 诊断事件跨重启恢复 | 重启后恢复保留期内有效固定事件；损坏 JSON 行和截断尾行被跳过且其它事件仍可导出 | P0 |
| CFG-027 | 诊断事件轮转清理 | 默认每段不超过 1 MiB、最多 64 段并清理 14 天前分段；时钟回退不误删当前活动分段 | P0 |
| CFG-028 | 诊断存储安全降级 | 普通文件、符号链接、不可写目录或同步失败不阻止正常/恢复启动；当次事件仍进入有界内存快照且 UI 不暴露路径或原始 I/O 错误 | P0 |
| CFG-029 | 删除非活动 Profile | Snapshot 与删除事件原子提交，活动项和原生桌宠窗口策略不变 | P0 |
| CFG-030 | 删除活动 Profile | 确定性选择相邻接替项，并通过可逆窗口事务同步可见性、置顶与穿透 | P0 |
| CFG-031 | 最后 Profile 保护 | 最后一个 Profile 不可删除；宠物、成长、库存、资产和纪念不受 Profile 删除影响 | P0 |
| CFG-032 | 删除失败补偿 | 原生应用失败不提交；持久提交失败恢复旧窗口策略、旧活动项且不发布成功事件 | P0 |
| CFG-033 | 删除模式门禁 | Safe/Recovery Mode、非法 ID 与非法接替关系全部 fail-closed 且零副作用 | P0 |

## 16. UI、设计与可访问性

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| UI-001 | 键盘导航 | 控制中心和命令面板可完整键盘操作 | P0 |
| UI-002 | 焦点可见 | 所有可交互元素具有清晰焦点状态 | P0 |
| UI-003 | 200% 缩放 | 无关键内容截断，无横向强制滚动 | P1 |
| UI-004 | 减少动画 | 取消位移/闪烁，功能状态仍清晰 | P0 |
| UI-005 | 深浅主题 | 文本、图标、角色边缘和状态色可辨识 | P1 |
| UI-006 | 危险确认 | 显示主体、动作、目标、影响和撤销方式 | P0 |
| UI-007 | 错误状态 | 说明原因、影响和下一步，不只显示错误码 | P1 |
| UI-008 | 气泡避让 | 不超出屏幕，不长期遮挡主要工作区域 | P1 |
| UI-009 | 组件全状态 | 默认、Hover、Pressed、Focus、Loading、Disabled、Error 完整且布局稳定 | P0 |
| UI-010 | Design Token | 页面不存在未登记颜色、字号、间距、阴影和 z-index | P0 |
| UI-011 | 多模式密度 | Companion、Character、Power User、Creator、Developer 主次清晰 | P1 |
| UI-012 | 跨平台截图 | Windows/macOS 浅深主题关键页无未批准视觉偏差 | P0 |
| UI-013 | 长文本与极值 | 中英文长文案、空数据和极端数据不破坏布局 | P1 |
| UI-014 | 人工设计评分 | 无 0 分项且总分不低于 13/16 | P0 |
| UI-015 | 控制中心中等视口 Hero | 在 810×607 及 720–860px 代表宽度打开概览页，检查中文标题、角色和五个操作 | 标题无 1–2 字孤行；操作自然换行且主操作优先；角色不遮挡正文或丢失 | P1 |
| UI-016 | 控制中心窄宽度重排 | 在 720、480px 和 200% 缩放下遍历全部一级页面、主导航、运行状态及顶栏操作 | 页面级无固定 720px 最小宽度；侧栏转为横向可滚动导航；内容与 Hero 单列重排；关键入口不隐藏、不裁切且无页面级横向强制滚动 | P0 |
| UI-017 | 控制中心导航闭环 | 分别点击一级“活动”、概览“查看全部”和顶栏头像，并检查当前项语义 | 前两者进入独立活动工作区，头像进入设置；导航当前项暴露 `aria-current=page`；不存在看似可点却无行为的入口 | P0 |

## 17. 安装、升级与卸载

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| INS-001 | 全新安装 | 默认资源、数据目录和安全设置正确 | P0 |
| INS-002 | 应用升级 | 用户数据、扩展和资源正确迁移 | P0 |
| INS-003 | 升级中断 | 下次启动恢复或回滚，不处于半安装状态 | P0 |
| INS-004 | 版本回滚 | 兼容数据可读取，不兼容时使用只读恢复模式 | P0 |
| INS-005 | 卸载 | 按用户选择保留或删除数据和密钥引用 | P1 |

## 18. 回归策略

### 17.1 Automation 与 Agent 生命周期

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| AAG-001 | Live Automation 创建 Agent 子任务 | Provider 调用前持久写入根 Run、Task、幂等键、准入快照和模型，不保存 Prompt | P0 |
| AAG-002 | Agent 请求写模块能力 | Journal 进入等待确认，批准前 Runtime 与模块无副作用 | P0 |
| AAG-003 | 批准后 Provider 续跑 | 工具经共享 Capability Gateway 执行，Provider 完成后 Journal 进入完成态 | P0 |
| AAG-004 | 用户拒绝或 Safe Mode 撤销 | 同 Turn 待批准项全部撤销，Journal 进入取消态且无副作用 | P0 |
| AAG-005 | Provider 续跑失败 | Journal 从活跃态进入失败态并保存有界错误，不永久停留在等待态 | P0 |
| AAG-006 | 桌面进程重启 | submitted/waiting 统一恢复为 interrupted，不自动重放 Prompt 或工具副作用 | P0 |
| AAG-007 | 按 Task/Run 查询 | Task 返回唯一生命周期；Run 最多返回 64 项并按提交时间稳定排序 | P1 |
| AAG-008 | 同 Run 幂等重试 | submitted/waiting 返回 DuplicateActive，completed 返回 DuplicateCompleted；failed/cancelled/interrupted 永久失败且不二次调用 Provider | P0 |
| AAG-009 | 用户按 Task 取消运行中 Provider | 同一共享取消令牌到达当前 Provider step；本地 Adapter 退出，Worker Provider 强杀子进程 | P0 |
| AAG-010 | Safe Mode 遇到尚未请求工具的 Provider | 活跃注册表中的任务全部收到取消，不依赖待批准队列是否已有项目 | P0 |
| AAG-011 | 递归与批准后续跑取消 | 每一轮复用原 Task 的取消令牌，取消后不得创建下一 Provider step 或模块副作用 | P0 |
| AAG-012 | Bridge 错误分类 | 永久提交错误只尝试一次；明确瞬态宿主错误按 Action 策略重试，错误字符串不参与分类判断 | P0 |
| AAG-013 | 用户取消根 Automation Run | 父 Run 取消令牌置位；submitted/waiting Agent 子任务转为 cancelled，运行中 Provider/Worker 收到同一任务取消且未知或终态 Run 返回 false | P0 |
| AAG-014 | Automation 不可信 Context Admission | 合法来源数据作为独立 untrusted User Message；段数与字节预算、非法来源、高置信中英文注入、非 draft 或任何 Tool Allowlist 均在 Submitter 前永久拒绝 | P0 |
| AAG-015 | Context Admission 脱敏安全审计 | 拒绝事件使用稳定原因枚举，只保存来源类别、计数和 Run/Trace/Automation/Action/Command 关联 ID；序列化与持久 Journal 均不含攻击正文、Prompt、路径或密钥 | P0 |
| AAG-016 | Context 审计故障 fail-closed | Journal 锁、序列化或持久写入失败时 Automation 永久失败且只尝试一次，Agent Submitter、Provider 和模块 Backend 零调用 | P0 |
| AAG-017 | Context 正常路径无误报 | 合法有界外部数据获准并保持 untrusted 标记，不产生 `context-admission-rejected` 诊断事件 | P0 |

### 18.2 Skill 执行历史与隐私

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| SKH-001 | Skill 等待高风险批准 | 执行元数据立即写入历史，状态为 `waitingForApproval`，不保存输入、源码、命令参数或 Agent 正文 | P0 |
| SKH-002 | 批准后执行完成 | 同一 `executionId` 原地收敛为 `completed`，创建时间和分页位置不变 | P0 |
| SKH-003 | 批准后执行失败 | 状态收敛为 `failed`，仅保存最大 4 KiB 的脱敏错误，重复批准不生成第二条历史 | P0 |
| SKH-004 | 用户拒绝执行 | 状态原地收敛为 `rejected`，Command 与 Agent Task 均无副作用 | P0 |
| SKH-005 | 历史稳定分页 | 使用 `(createdAtMs, executionId)` 成对游标，新到旧稳定分页且限制每页 1–200 条 | P1 |
| SKH-006 | 隐私删除 | 支持按 execution 删除或全部删除；删除历史不执行、取消或恢复任何 Skill | P0 |
| SKH-007 | 活跃 Skill 取消传播 | 按 `execution_id` 取消会同时设置 Worker 取消令牌与当前 Provider `CancellationFlag`，并在下一条 Command 前阻断副作用 | P0 |
| SKH-008 | 取消终态竞态 | 历史立即进入 `cancelled`；迟到的 completed/failed 不得覆盖，未知或已终态 execution 返回 false | P0 |
| SKH-009 | 等待批准与运行取消分离 | pending 计划使用 reject，未进入活跃注册表；cancel 不得把尚未执行的批准计划伪装成运行中取消 | P0 |

- 每次提交：Unit、Schema、核心 Contract。
- 每次合并：核心 Integration、Pet/Asset/Permission P0。
- 每夜：完整功能回归、恶意包、网络故障和 UI 截图对比。
- 每个候选版：双平台 E2E、安装升级、24 小时 Soak、安全扫描。
- 每个稳定版：人工探索性测试、Creator Studio 发布闭环和商店兼容抽样。

## 19. 发布退出标准

- 所有 P0 用例通过。
- P1 通过率不低于 98%，其余缺陷有批准的风险说明。
- 无未关闭 Critical/High 安全问题。
- Windows 与 macOS 目标矩阵均有签名测试报告。
- 安装、升级、回滚、安全模式和默认资源回退全部通过。
## Agent Goal、无人值守与推理策略增量验收

| ID | 场景 | 验收标准 | 优先级 |
| --- | --- | --- | --- |
| AGT-100 | 持久 Goal 后台推进 | Goal 未满足且预算、策略和授权有效时，跨批次 Yield 后继续；完成只能由当前 Plan 全步骤证据触发 | P0 |
| AGT-101 | 范围绑定预授权 | Grant 精确绑定 Goal、Plan、Workspace、Tool、Provider、模型、数据和预算；任一漂移在派发前拒绝 | P0 |
| AGT-102 | Full Device 警告与硬禁区 | UI 完整提示文件、命令、联网、凭据、供应链和不可逆副作用；硬禁区不可绕过 | P0 |
| AGT-103 | Auto-review 不扩权 | Reviewer 不能扩大 Sandbox、网络、工具或参数；不可用或不确定时恢复人工审批 | P0 |
| AGT-104 | 推理等级映射 | Adapter 报告支持等级并记录 requested/actual/provider value；显式不支持在调用前失败 | P0 |
| AGT-104A | 兼容 Provider 显式能力配置 | 旧配置无字段可读；空映射、`auto`、非法版本和值拒绝；CAS 保存与重启精确恢复等级到厂商值映射 | P0 |
| AGT-104B | Worker 推理参数隔离 | 真实 loopback HTTP 仅收到已验证 `provider_value`；无 Mapping 时字段缺失，Mapping Version、隐藏推理和宿主策略不出进程边界 | P0 |
| AGT-104C | 运行级策略贯通 | 普通 Agent、Auto Mode 单轮和后台 Loop 使用同一策略；确认续跑、多轮执行与 Context Anchor 的 Mapping 不漂移 | P0 |
| AGT-104D | 推理选择器能力门禁 | 仅支持推理的 Provider 显示合法等级；固定等级禁止降级；Browser Preview 不伪造能力；窄屏标签完整 | P0 |
| AGT-105 | Adaptive 与缓存隔离 | 推荐等级受任务、风险、费用上限约束；策略、实际等级或映射版本变化不命中旧生成缓存 | P1 |
| AGT-106 | Away Summary | 展示文件、测试、网络、预算、自动授权、失败重试和暂停原因，不暴露 Secret 或隐藏推理 | P1 |
| AGT-107 | 授权撤销与未知结果 | 撤销阻止新派发；无法确认的在途结果进入 `indeterminate` 且不自动重放 | P0 |
| AGT-108 | Desktop 后台 Job 唯一性 | 同一 Session 只能原子创建一个活跃 Job；终态释放 Session 后允许新 Job，旧快照仍可查询 | P0 |
| AGT-109 | Job 跨批次进度 | 每批 Turn、Cache Hit 与 Checkpoint sequence 单调累计；Yield 不伪装成 Pause 或终态 | P0 |
| AGT-110 | Job Pause/Cancel 控制 | Pause 与 Cancel 使用独立控制信号；Cancel 可覆盖未收敛 Pause，终态后任何控制稳定拒绝 | P0 |
| AGT-111 | Job 退出收敛 | Safe/Recovery Mode、应用退出与宿主崩溃均收敛持久 Session/Task/Attempt；超时结果进入 `indeterminate` | P0 |
| AGT-112 | Job 版本化快照 | Desktop、TypeScript 与 UI 使用 `nimora.desktop-auto-mode-job/1`，浏览器预览不得伪造宿主执行 | P0 |
| AGT-113 | Job 全量取消隔离 | 应用退出只向活跃 Job 发布共享取消信号，已完成历史不改变；控制广播不持有锁等待 Runner，时间倒退整体拒绝 | P0 |
| AGT-114 | 在途取消确定性 | Provider/Tool 在途收到 Pause/Cancel 后，只有可证明未产生副作用或已原子提交的结果可写确定终态；其余 Attempt 与 Job 标记 `indeterminate`，不得自动重放 | P0 |
| AGT-115 | Job 启动契约 | 原生 Start 原子保留 Session 并立即返回 Starting 快照，默认每批 8 Turn、512 输出 Token、离线执行；浏览器预览稳定拒绝 `desktop-host-required` | P0 |
| AGT-116 | 控制竞争记账 | 批次执行期间进入 `pausing/cancelling` 后，已原子提交的 Turn、Cache Hit 与 Checkpoint 仍单调写入快照，再执行终态收敛 | P0 |
| AGT-117 | 有界退出排空 | 应用退出向全部活跃 Job 取消并通过 Condvar 最多等待 2 秒；正常终态唤醒排空，超时统一隔离为 `indeterminate/shutdown-timeout`，迟到 Runner 不得覆盖且 Session 可重新启动 | P0 |
| AGT-118 | Safe Mode 后台封锁 | 进入 Safe Mode 与应用退出复用同一 Auto Job 排空协议；超时使用独立 `safe-mode-timeout` 未知态，释放 Session 且共享 Provider CancellationFlag 已取消；Recovery Mode 不存在可继承活跃 Job 并拒绝 Start | P0 |
| AGT-119 | 崩溃投影重建 | 使用真实 SQLite 构造 Running Session 后重启桌面；Session 原子恢复为 `paused/restarted`，历史 IPC 可见同 Session ID 的终态 Job 投影、活跃列表为空且不触发 Provider；未决 Active Attempt 必须先转 `indeterminate/restart-attempt-indeterminate`，可恢复记录超过上限时失败关闭而非静默截断 | P0 |
## Auto Mode indeterminate-attempt reconciliation

- Verify an indeterminate attempt can be resolved exactly once with all binding parameters unchanged.
- Verify `confirmed_not_executed` leaves no active attempt, pauses Session and Task, increments Checkpoint sequence, and records immutable audit evidence.
- Verify `accept_external_effect_and_cancel` cancels Session and Task without recording a fabricated Provider result.
- Verify stale Session, Attempt, Checkpoint sequence, request fingerprint, non-indeterminate status, replay, invalid actor, and oversized/control-character reason fail with zero partial writes.
- Verify resolution audit survives database reopen and list bounds reject zero or more than 100 records.
- Verify browser preview rejects detail and resolution commands with `desktop-host-required`.
- Verify desktop platform mapping preserves Session、Attempt、Checkpoint sequence、request fingerprint、decision 与必填 reason，不改名、不丢字段；Pause/Cancel 使用独立 Job ID 命令。
- Verify 目标控制中心只在 `indeterminate` Attempt 上显示两种互斥决议，提交前展示绑定身份与永久审计提示，空白理由不能提交，Safe/Recovery/Browser Preview 全部只读。

## Agent control center aggregation

- Verify a restarted Auto Mode job returns its persisted Session, exact historical Plan revision, Goal, optional Checkpoint/Attempt, immutable Resolutions, and rebuilt Job projection in one response.
- Verify revising a Goal does not change the Plan shown for an older Session binding.
- Verify missing Session, Goal, bound Plan revision, corrupt payload, or index/payload divergence fails the entire query without returning partial entries.
- Verify browser preview labels its deterministic sample as preview data and never permits Pause, Cancel, resolution, Provider, Tool, or filesystem side effects.
- Verify the three workspace tabs expose accessible names/current state, preserve safe/recovery write restrictions, and remain usable at the 720px layout breakpoint.
- Verify Normal Mode can request Pause and then escalate the same active Job to Cancel; terminal jobs reject both controls.
- Verify Safe Mode and Recovery Mode reject Pause/Cancel at the IPC application-service boundary and leave the complete Job snapshot byte-for-byte unchanged.
- Verify `None`, empty, and whitespace-only reconciliation reasons fail before any persistence lookup or write.
- Verify control-center `/2` reports persisted `effectiveStatus` separately from `projectionStale`; UI shows a convergence warning and never retries an external operation because a projection is stale.

## Theme Asset 安全与体验

- Verify 外接 AI 可生成 `theme` Creator Draft；只接受 `theme.local.*`、严格版本/发布者/许可证、本地化名称、固定九 Token 和空权限说明，未知字段或发布者命名空间覆盖失败关闭。
- Verify Theme Draft 经独立审查重新执行颜色格式与 WCAG 门禁，批准摘要绑定完整元数据；保存写出结构化主题草案，安装生成 Manifest、Theme 与 SHA-256 Inventory，并复用 Asset 原子安装和版本备份。
- Verify AI Theme 安装不会自动激活、不会继承 Creator Agent 权限；升级识别已安装版本但不提示无意义的“重新授权”，用户仍须从主题选择界面显式激活。
- Verify Theme 缺少入口、错误媒体类型、缺失/额外 Token、非法 Hex、未知字段、CSS/URL/脚本注入均在安装前拒绝且不改变活动主题。
- Verify 安装前预览只作用于 Creator Studio 卡片；取消或安装失败后 App Shell Token 保持不变。
- Verify Theme 激活后全局 Token 一致，选择记录原子持久化，重启后重新复验；包缺失或损坏时回退内置主题并显示原因。

## AI 场景 Profile 创建

- Verify Creator 只接受七种既有 Profile Mode、可空布尔覆盖与 `0..100` 主动频率；未知字段、控制字符、空名称、超长名称和越界频率在模型输出边界拒绝。
- Verify Profile 草案权限说明必须为空，独立审查复用领域校验并明确“创建后不切换”；一次性批准仍与完整草案摘要绑定。
- Verify Workspace 原子写入 `nimora-draft.json` 与 `profile.json`，目录身份来自规范化产物摘要，不允许 AI 控制文件路径或运行时 UUID。
- Verify 创建调用生产 `ProfileService` 和 SQLite 事务，宿主生成真实 UUID、发布标准创建事件，活动 Profile、窗口策略与声音状态保持不变。
- Verify Creator Studio 展示模式、置顶、点击穿透、声音和主动频率预览；空覆盖显示“继承默认”，窄屏和键盘访问保持可用。
- Verify Safe/Recovery Mode 禁止创建；重复创建形成独立宿主 Profile 而不是静默覆盖，用户必须在 Profile 控制中心显式切换。
- Verify Safe Mode 始终使用内置主题且拒绝激活写入；Recovery Mode 拒绝主题切换且选择文件不变化。
- Verify 浅色、深色、高对比度和减少动画在 720px、宽屏、200% 缩放和键盘路径下可辨识，危险态不被主题弱化。
- Verify RGBA 先按主题模式与 Surface 正确合成再计算 WCAG 相对亮度；正文低于 `4.5:1` 或强调/成功/危险低于 `3:1` 时包整体拒绝。
- Verify 已安装主题可显式恢复内置主题；进入 Safe Mode 时 UI 立即同步宿主内置主题，退出后重新读取原选择，不保留陈旧视觉状态。

## Voice Asset 安全、播放与降级

- Verify Voice 缺少入口、非 Voice 声明入口、未知字段、空/超量 Clip、非法或保留 Cue、非法字幕/Locale、NaN/越界增益在安装前拒绝。
- Verify WAV/OGG 媒体类型、扩展名与 Header 必须一致，单 Clip 超过 2 MiB、未声明 Cue 和 Inventory 外路径拒绝。
- Verify `.nimora` 导出、预览、安装和重开保持 Descriptor、字幕与已验证字节一致，篡改后不返回音频。
- Verify 激活选择原子持久化，重启复验；损坏选择、缺失包和 Safe Mode 返回 `builtin.silent`，不暴露文件路径。
- Contract-test Character、Theme、Voice 的类型化选择 Policy：各自 Schema/File/Builtin ID 不串写；NotFound 无告警回退；损坏 JSON、未知 Schema、非法 ID 和 Safe Mode 给出确定原因；非 NotFound I/O 错误向上传播；成功写入无遗留临时文件。
- Verify Creator Studio 不自动播放，展示 Cue、字幕、格式、大小和增益；取消预览释放 Blob URL 且不改变活动声音。
- Architecture gate: `pnpm check:architecture` 必须先通过检测器自检，再证明 UI 不能直接导入 Tauri，纯领域/Policy/Worker/Module Adapter 不能引入 SQLite、Tauri、HTTP Client 或 Provider Worker；任何新增例外必须通过 ADR 改规则，禁止内联忽略。
- Verify Quiet Mode 在 Clip IPC 前阻断；动作成功后音频读取或播放失败不改变动作结果与用户通知。
- Verify 平台权限、危险、错误和恢复提示 Cue 永远不查询第三方 Voice 包。

## AI 辅助扩展创作

- Contract-test `nimora.capability-semantic-contract/1` 拒绝未知字段、非法/重复/未排序语义 ID、空输出、零成本与越界成本；序列化不得含 Backend、Schema、路径、Secret 或运行时对象。
- Verify Composition Graph 摘要对节点增删和语义变化敏感，篡改摘要失败关闭；搜索硬限制节点 256、深度 8、状态 2048、请求项 32，且从不执行 Tool。
- Verify 有界 Planner 在同一输入下稳定选择最低成本、最少步骤、字典序确定路径，并强制前置条件、数据等级、副作用、离线和总成本约束。
- Verify 内建 Tool 与 Semantic Contract 精确一一对应；新增 Tool 缺少契约或多出孤立契约时测试失败。
- Verify Skill 省略 `composition` 时保持兼容但不进入语义图；显式契约 ID/effect 不匹配、冒充平台输出或字段不规范时安装拒绝。
- Verify 激活 Skill 的已验证契约进入实时图，暂停后立即撤销并改变摘要；Creator Gap UI 同时显示 Catalog 与 Graph 摘要且不宣称自然语言目标已获证明。

- Verify Creator Agent 固定使用 `Draft` autonomy、空 Tool allowlist、调用深度 1，且只允许已登记的本地 Provider；模型 Tool Call 必须 fail closed。
- Verify可信 System instruction 与 Personal/Untrusted 用户需求使用独立消息，模型输出永远按不可信输入处理。
- Verify Markdown 包裹、未知字段、类型不匹配、非法 Manifest、权限说明缺失/额外/重复均拒绝，错误不回显模型原文。
- Verify 绝对路径、`..`、反斜杠、重复路径、缺失入口、文件数/单文件/总大小超限均在任何写盘前拒绝。
- Verify User Program、Skill、Automation 分别复用生产校验；Safe/Recovery/浏览器预览禁止生成和保存，已保存结果仍显示“尚未安装”。
- Verify Automation 必须携带严格三段数字版本；Creator 首装进入 SQLite Catalog 且保持停用，升级保存上一版本并展示命令 `added/removed` 与行为 `scope-changed`。
- Verify 同版本覆盖、损坏 Catalog、Safe/Recovery 启停、无上一版回滚均失败关闭；升级和回滚不能继承启用状态，目录跨重启恢复一致。
- Verify Automation 目录启停和回滚只接受已安装 ID；浏览器预览不得伪造 Catalog 写入成功，回滚后再次回滚可恢复刚才替换的版本。
- Verify 保存 IPC 重新校验完整需求与草案，选择的 Workspace 必须是可规范化真实目录；`.nimora-drafts` 符号链接、目标已存在和中途写入失败均不得覆盖或留下已发布的半成品。
- Verify User Program 与 Skill 的每个 JavaScript 文件都由对应独立 Worker 的显式 `Validate/Validated` 协议检查；包含顶层 `throw` 的合法语法通过且不执行，非法语法返回逐文件失败。
- Verify Skill 草案检查校验协议版本、Execution ID 和 Manifest，但安装前不要求 Active Skill Lease；正式 `Run` 仍必须精确匹配 Active Lease 与 Activation Event。
- Verify UI 检查报告使用 `nimora.creator-draft-check/1`，逐文件区分 passed/failed；未通过检查时保存按钮不可用，绕过 UI 直接调用保存 IPC 仍失败关闭。
- Verify AI 生成的 Capability Facade 只能引用实时 Registry 中的精确能力 ID；未知、停用、撤权或租约过期能力使 Facade 安装或运行失败关闭。
- Verify Facade 的权限、数据出口、最高风险和成本为全部可达底层路径的保守并集，AI 声称更低风险不能覆盖宿主计算结果。
- Verify Composition Planner 只读取已验证语义契约，不从标题、描述、Prompt 或 JSON Schema 猜测 `requires`、`produces` 和前置条件。
- Verify 在线/离线、多 Provider 与多设备虚拟化使用同一语义契约；降级路径增加权限、扩大数据范围或改变不可逆副作用时不得自动切换。
- Verify Skill/Connector 停用、升级、崩溃隔离、撤权或租约过期后，其组合图节点立即撤销，已保存组合显示 `degraded` 且不暗中选择高权限替代。
- Verify Simulation World 使用虚拟时钟、固定随机种子和合成 Secret 引用；任何真实 Connector、生产 Gateway 或外部副作用调用都使测试失败。
- Verify Explanation Pack 的调用原因、数据范围、授权和结果可由审计收据逐项复验；AI 推测与真实 Trace 冲突时只展示宿主事实。
- Verify Interaction Mapping 的自然语言、快捷键、手势、MIDI 和语音绑定均需消歧预览、冲突检测、速率限制、可撤销和紧急停用。
- Verify Automation 草案复用生产 Engine 的确定性校验且不会伪装成 JavaScript Worker 检查；检查、保存、安装和启用状态在 UI 中不可混淆。
- Verify User Program `Sandbox/Sandboxed` 在独立进程执行顶层代码但不要求返回值可 JSON 化；声明函数的脚本通过，顶层异常失败，Node/Tauri/文件/网络等原生对象仍不可用。
- Verify Skill 行为沙箱只返回记录的命令与 Agent Task 数量，不调用 Capability Gateway、Provider 或模块 Backend；使用临时精确 Manifest Lease 仅满足 Worker Admission，绝不持久化或继承为安装授权。
- Verify Automation 行为沙箱使用匹配的触发事件和空对象数据运行生产 DryRun Backend；条件不匹配是确定性运行状态而不是检查器故障，任何 Command Backend 调用均失败测试。
- Verify 保存 IPC 在写盘前重新运行 parse-only 与行为沙箱；任一 JavaScript 语法或顶层行为失败均不创建 `.nimora-drafts/<artifact-id>`。

## Automation 参数绑定运行期批准

- Verify Safe/Low 动作直接执行；用户把宿主最低 Low 自报为 Safe 时按宿主有效风险执行，不形成旁路或虚假失败。
- Verify 未注册命令在任何 Journal、活跃运行或 Backend 副作用前失败关闭。
- Verify 任一 Medium/High 主动作或补偿动作使整次 Run 返回 `waiting_for_approval`；此前低风险动作也未执行，Run Journal 不存在。
- Verify 待批目录只返回 Automation/版本/Run ID/过期时间与逐项实际参数，不返回 Event data 正文。
- Verify Catalog 读取会把独立持久化的有效启用态同步进运行 Definition；已启用事件 Automation 不得因安装 Payload 的默认禁用态被错误跳过。
- Verify 批准绑定不可变定义、事件、参数、风险摘要和 `runId`；待批期间安装版本升级或停用后，旧计划进入失败终态且不能执行、重试或创建 Run Journal。
- Verify 批准原子单次 claim；拒绝、过期和重复批准失败关闭；Safe Mode 必须在 claim 前拒绝并保留 pending，退出 Safe Mode 后用户仍可明确批准或拒绝。
- Verify 批准后使用预分配 `runId` 创建并完成真实 Run Journal；Backend 或 Engine 异常收敛为 failed/interrupted，不遗留 running。
- Verify 重启保留未过期 pending、过期 pending 变为 expired、executing 变为 interrupted，且不会自动重放命令。
- Verify Browser Preview 返回空待批目录，并对批准/拒绝明确报“需要桌面运行时”，不得伪造成功。

## Automation 持久运行与费用治理

- Verify 并发与冷却准入在 Run Journal、活跃运行、Agent Task 和 Backend 前完成；拒绝路径零业务副作用。
- Verify 相同 Automation 的跨连接并发竞争只有策略允许的数量成功，不同 Automation 互不占用配额。
- Verify 冷却从真实准入时刻计算，边界时刻允许运行；重启保留冷却事实但释放旧进程运行租约。
- Verify 参数绑定批准等待不获取运行租约；批准 claim 并重验成功后才竞争并发和冷却配额。
- Verify Agent `maxCostMicrounits` 只作为并发预留，不写成真实费用；Provider 前预算不足时不创建 Agent Journal 或调用 Provider。
- Verify Completed 使用累计 `AgentTask.usage.costMicrounits` 原子替换预留，实际费用超过预留时账本保持不变并失败关闭。
- Verify WaitingForConfirmation 保留完整预留；确认后的 continuation 使用累计 Usage 最终结算且重复相同结算幂等。
- Verify Provider 错误、崩溃或重启造成费用不确定时状态为 `indeterminate`，完整预留继续计入当日预算且不能自动重试。
- Verify 两个并发 Agent 费用预留只有不超过当日预算的组合成功；任务 ID 重放不重复预留，身份或金额漂移失败关闭。
- Verify `dailyCostBudgetMicrounits = 0` 只允许最大费用同为零的任务，Safe/Recovery/Browser Preview 不修改治理账本。
- Verify Governance Catalog 按 Automation 隔离聚合活跃租约和当日 reserved/settled/indeterminate 费用；不得返回任务内容、事件正文、Provider 请求或 Secret。
- Verify 桌面 UI 分别展示预留与实际费用，未知费用显著标记并说明预算不会自动释放；预算为零显示“仅零费用任务”，不能误报为无限预算。
- Verify 并发、冷却与每日预算拒绝给出不同的本地化用户原因，同时保持拒绝前零 Backend/Provider 副作用。
- Verify 未知费用 Catalog 只列出 `indeterminate` 任务并携带最新 `updatedAtMs`；Browser Preview 返回同版本 Schema 的空 `pending/decisions`，且任何决议请求都拒绝为 desktop-host-required。
- Verify 人工对账仅接受 Provider 对账单、账单导出或操作员保守估算三类原因，要求非负实际微单位和精确 `expectedUpdatedAtMs` CAS；陈旧页面、已结算任务、重复任务决议和重复 Decision ID 均失败关闭。
- Verify 决议审计与费用状态在同一 Immediate 事务中原子提交；成功后任务由未知转为已结算且不可再次决议，审计记录不能通过应用修改或删除。
- Verify 实际费用可高于预留并按真实值参与每日预算，超限后继续阻止新任务；不得为了释放预算而截断、低报或自动归零未知费用。
- Verify Safe Mode 与 Recovery Mode 可读取待办和审计但不能提交决议；宿主而非前端生成 UUIDv7 Decision ID，调用方不能伪造决议身份或时间。
- Verify 对账 UI 逐任务显示预留费用、实际费用、证据来源及“不可修改或删除”显式确认；未确认、非法数字、提交中重复点击均不能调用 IPC，窄屏布局不丢失风险说明和操作标签。

## 外接 AI 能力开发平台

- Verify Provider 返回 `nimora.creator-draft/1` 或 `nimora.capability-gap/1` 之外的 Schema、混合 Draft/Gap 字段、Markdown、额外字段、重复能力和非法能力 ID 均失败关闭。
- Verify Capability Gap 只包含结构化目标、缺失能力、所需操作、替代方案和平台提案标志；不能进入 Draft 检查、批准、安装或执行路径。
- Verify Gap UI 明确标记 `NON-EXECUTABLE`，显示机器尚未验证的缺口事实，不出现批准和安装入口；报告保存仅写入用户选择 Workspace。
- Verify Gap 报告使用不可覆盖原子文件、拒绝符号链接和非法 Workspace，响应只返回相对文件名且不泄露绝对路径。
- Verify Creator Catalog Snapshot 从同一生产 Tool Registry 投影，按 ID 稳定排序且摘要稳定，不暴露第三方标题/描述、输入输出 Schema、Backend、文件、网络、数据库、Provider 或 Tauri 对象，Skill 元数据不能进入受信 System Message。
- Verify 已激活 Skill Agent Tool 进入 Creator Snapshot，Skill 暂停后立即撤销且摘要变化；生成时目录与保存时目录分别由宿主读取，不能信任前端缓存。
- Verify Capability Gap 声称已注册精确 ID 缺失时失败关闭；真实缺失 ID 生成带 Catalog Digest 的确定性 Composition Plan，resolved/missing 集合不可混淆。
- Verify 持久 Gap 报告包含宿主重新计算的 Composition Plan；伪造、过期或与 Gap ID 不一致的证明不能写入 Workspace。
- Verify UI 将精确 ID 缺失证明与自然语言目标不可实现证明明确区分，不能用绿色核验状态暗示所有组合路径已穷尽。
- Verify Gap 的语义输入/输出必须使用排序、去重、有界的小写命名空间 ID，输出集合不得为空；未知字段和模型自报的“已满足前置条件”失败关闭。
- Verify 宿主以实时 Semantic Graph、空可信前置事实和固定数据等级/副作用/成本上限重算路径；若目标被完整解析，则与 Gap 声明矛盾并拒绝。
- Verify `nimora.persisted-capability-gap/2` 同时保存实时重算的 Exact-ID Plan 与 Semantic Plan；Graph 摘要、缺失输出或 Gap 候选不一致时不得写盘。
- Verify Gap UI 展示候选输入、目标输出、宿主能力路径、成本、扩展状态和缺失输出，并明确模型候选映射不等于自然语言完备证明。
- Verify AI 对同一目标按设置、组合、Automation、Program、Skill、Connector 和 L4 Proposal 比较，选择最低充分层级并给出不自动化理由。
- Verify 多模态 Perception Pipeline 对每个来源单独授权，原始音视频默认不持久化，低置信推断不能触发高风险动作。
- Verify Personal API 只能映射 Registry 中的 Command/Query，声明鉴权、幂等、速率、离线和撤销语义，不能生成任意本地 Handler。
- Verify Agent Team 中子 Agent 不继承父 Agent 权限，多数投票不能覆盖策略、确定性测试或参数绑定批准。
- Verify Prompt、Context、Memory 和 Cache Pack 保留来源与数据分类，摘要不冒充原文，跨 Provider 缓存按策略隔离且支持彻底清除。
- Verify Policy Compiler 输出规则草案和冲突报告；自然语言、聊天同意和模型自评均不能成为授权凭证。
- Verify Simulation Pack 与生产 Capability 物理隔离，覆盖超时、重复、乱序、部分成功和恢复，测试通过不自动启用生产能力。
- Verify Asset Build Project 保留来源、许可证和原件，Importer 隔离探测，资产包不能夹带代码或动态加载可信 Handler。
- Verify 本地分叉升级显示上游、当前本地和新上游三方 Diff，冲突、权限扩大或数据驻留变化失败关闭。
- Verify Retirement Plan 撤销 Grant、Secret、订阅、调度与缓存，导出和删除传播产生可验证收据。

- Verify 同一用户目标分别存在配置、组合和代码方案时，Creator 返回可比较方案并默认选择最低充分复杂度。
- Verify Registry 无法表达目标时返回结构化 `CapabilityGap`；未知 Command、私有 IPC、任意系统调用和自生成 Handler 均在写盘与执行前拒绝。
- Verify `catalog.search/explain` 只暴露调用方获准的能力元数据，不泄露 Secret、内部句柄、未授权模块或数据正文。
- Verify 所有 Builder Tool 使用版本化 Schema、Task/Trace ID、截止时间、取消、幂等键、稳定错误码和脱敏审计；Provider 不能覆盖宿主身份。
- Verify AI 生成的测试报告、自评结论和聊天同意不能推进 `validated`、`approved`、`installed` 或 `activated` 状态。
- Verify Capability Proposal 只能进入评审队列，不能动态修改当前 Registry、风险等级、安全策略或 Capability Grant。
- Verify Proposal Queue 只接受与 `proposalId` 文件名一致的普通 JSON 文件，拒绝符号链接、未知字段、超过 1 MiB、非法状态、损坏双计划和内容摘要漂移。
- Verify `pending-review` 只能一次性转为 `accepted`、`rejected` 或 `duplicate`，理由必须非空、无首尾空白、无控制字符且不超过 1024 bytes；任何终态重写均失败关闭。
- Verify Safe/Recovery Mode、Browser Preview 和直接 IPC 调用均不能写入维护者裁决；读取失败不得跳过损坏记录后展示不完整队列。
- Verify 治理 UI 明确 `accepted` 仅代表进入可行性分析，不创建 Handler、不修改 Registry、不签发 Grant、不执行代码；终态记录不再显示裁决控件。
- Verify Proposal 的 SHA-256 内容摘要只作为离线一致性检测，不在 UI、日志或文档中描述为身份签名或来源真实性证明。
- Verify Proposal 聚类键只覆盖排序后的精确缺口 ID 与语义缺失输出；标题、摘要和提交顺序变化不改变同一缺口聚类，任一能力或语义输出变化必须形成不同聚类。
- Verify 聚类规范提案由最早 `submittedAtMs`、再按 `proposalId` 确定；分诊等级只按 1、2–3、4+ 条同类记录映射为 `normal/elevated/high`，不得宣传为 AI 价值评分或实现优先级。
- Verify `duplicate` 裁决必须绑定存在、非自身、同聚类的 `duplicateOfProposalId`；接受/拒绝不得携带目标，自指、跨簇、缺失目标和规范记录删除后悬空引用均使整个队列失败关闭。
- Verify 聚类与分诊是只读宿主投影，不写回 Proposal、不改变内容摘要、不自动裁决记录，也不因聚类数量扩大权限、预算或执行范围。
- Verify 仅 `platformProposalRequired=true` 的实时复验 Gap 可提交；宿主提交时重建 Catalog 与 Semantic Graph 并重算双计划，前端摘要和旧计划不能成为提案事实。
- Verify Proposal 使用 `.nimora-proposals/capability-proposal-<uuid>.json` 不可覆盖原子文件，状态固定为 `pending-review`，响应只返回相对路径。
- Verify Proposal 不含批准凭证、可执行代码、Handler、Grant 或自动 Registry 更新入口；提交按钮与 Draft 审查、批准、安装按钮完全分离。
- Verify 两个不同 Provider 可从同一 Creator Project 接力，且 Goal、决定、文件追踪、Diff、测试证据、预算和回滚点不丢失或被摘要伪造。
- Verify Prompt 污染的网页、文档、Schema、模型资产和 Connector 数据保持 Untrusted Data，不能成为 Creator System 指令。
- Verify 生成、验证、自动修复和 Agent 循环分别受费用、Token、时间、步骤、工具、内存和重试硬上限约束；超限后项目保留最后有效检查点。
- Verify 仿真使用虚拟 Clock、Event、Connector 和 Backend 且零真实副作用；仿真结果在 UI 与审计中不能显示为生产执行。
- Verify Canary 失败只产生最小修复候选或回滚，不自动扩大权限、数据范围、预算、网络目的地或主动性。
- Verify Provider 下线、协议破坏和扩展退役会撤销 Grant、Secret 引用、订阅、任务和缓存，保留可读导出并验证删除传播。
- Verify 离线时项目编辑、已缓存契约检查、模拟、导出和回滚可用；需要云模型或在线 Connector 的步骤明确等待且不伪造完成。
- Verify Creator Studio 的方案、能力图、Diff、风险、预算和证据是权威状态；聊天文本不能覆盖，并通过键盘、读屏、200% 缩放和高对比度测试。

### PET-057 Profile 完整照料

- 前置：活动 Profile 为 `careNeedsMode=full`，建立生命时间基线。
- 验证：六个周期后 Energy -6、Mood -2、Satiety -3、Cleanliness -1；不死亡、不生病、不倒扣关系，AI 与网络关闭时结果一致。

### PET-058 Profile 简化照料

- 验证：Energy 与 Mood 按本地周期变化，Satiety 与 Cleanliness 保持不变。
- 验证：Feed、Play、Groom 仍可主动执行并原子持久化。

### PET-059 关闭与重新启用

- 验证：关闭期间四项数值不变但时间基线推进；切回完整模式后不补算关闭期。
- 验证：断网、禁用 Provider、关闭控制中心和应用重启后策略一致。

### PET-060 Profile 迁移与界面

- 验证：旧 Profile 缺少字段时按完整模式解析，当前生命值不重置。
- 验证：创建界面提供三档说明，卡片展示当前模式，键盘和屏幕阅读器可访问。

### PET-061 陪伴纪念阈值

- 验证：陪伴点达到 1、25、50、100 时依次解锁四个稳定标识；单次跨越多个阈值时完整补齐。
- 验证：冷却拒绝、Drag 拒绝、生命 Tick 和纯动画不产生纪念。

### PET-062 收藏持久化与迁移

- 验证：纪念、Pet 状态和 Event 原子保存，失败时内存、界面和事件总线均无副作用。
- 验证：旧快照缺少 `keepsakes` 时为空，重复或非规范顺序的持久集合失败关闭。
- 验证：SQLite 重启后所有权不变，断网、关闭 Provider 和关闭控制中心不影响收藏。

### UI-024 纪念收藏视觉

- 验证：关系卡以紧凑标签显示已拥有纪念，空态说明第一次互动即可获得，不把未拥有内容设计为焦虑倒计时。

### PET-063 Starter Pack 与旧数据迁移

- 验证：新 Pet 与缺少 `inventory`/`lastItemUseMs` 的旧快照均获得三种道具各 3 个且冷却为 0；已存在空背包重启后仍为空，不重复授予。
- 验证：Rust、Zod 与 SQLite 对相同快照得到一致结果；未知标识、重复/乱序 Stack、0 或 1000 数量均失败关闭。

### PET-064 道具使用与原子扣减

- 验证：莓果、星星球、泡泡皂分别产生定义的有界生命值和陪伴收益，每次只扣一个，归零立即移除 Stack。
- 验证：Command 为 `pet.inventory.use`，Event 为 `pet.inventory.used`，Trace 对应并含前后生命值与库存；重启恢复扣减结果。

### PET-065 冷却、耗尽与拒绝

- 验证：5 秒内再次使用、无库存、Drag、Safe/Recovery Mode 和 Repository 保存失败均不扣库存、不改生命值或冷却、不发布 Event。
- 验证：断网、禁用 Provider、关闭控制中心后仍可从原生桌宠入口使用；AI/Program/Skill 不能直接写库存。

### UI-025 随身背包视觉与空态

- 验证：控制中心显示图标、名称、核心效果和剩余数量，点击反馈明确且不会把免费照料伪装成道具消费；键盘焦点、禁用态、窄窗口截断和高对比模式可辨识。
- 验证：空态说明资产不过期且新奖励不依赖联网，不显示焦虑倒计时或诱导购买；Browser Preview 明确只是 UI 模拟。
- 验证：窄宽、200% 缩放、高对比度和长语言标签不遮挡关系进度；读屏可获得收藏总数和每件名称。

### PET-066 桌宠本体背包闭环

- 验证：关闭控制中心后，长按与右键桌宠均可从同一菜单进入背包；入口总数等于所有 Stack 数量之和。
- 验证：点击道具经过原生 `use_pet_item`，数量立即更新且菜单保持可操作；冷却、耗尽、Safe/Recovery 与保存失败显示温和错误且不伪扣库存。

### UI-026 Overlay 背包导航与边界

- 验证：260×300、最小 180×210、200% 缩放和长标签下，菜单与三项 Starter Pack 不被窗口裁切，焦点样式和数量仍可辨识。
- 验证：打开背包后首个“返回”动作获得焦点；Esc 先回宠物菜单，再次 Esc 关闭并聚焦桌宠；空态可读且不创建第二窗口。

### PET-067 宠物改名原子性与迁移

- 验证：首尾空白被规范化，1–64 个 Unicode scalar values 可保存；空名称和 65 字符名称失败且原名称不变。
- 验证：Command 为 `pet.identity.rename`，Event 为 `pet.identity.renamed`，Trace 对应并包含前后名称；SQLite 重启恢复新名称。
- 验证：改名不改变 Pet ID、关系、生命值、背包、纪念、位置、策略或 Renderer；Repository 保存失败不发布 Event 且内存 Snapshot 不变。

### PET-068 多窗口名称同步与失败关闭

- 验证：控制中心和原生 Overlay 任一入口改名后，另一窗口从宿主 Snapshot 更新；断网、禁用 Provider、关闭控制中心后 Overlay 仍可改名。
- 验证：Safe/Recovery Mode、非法输入和宿主失败均显示温和错误，不伪造成功；AI、Program、Skill 和 Renderer 不能直接修改名称。

### UI-027 控制中心与 Overlay 命名体验

- 验证：控制中心关系卡使用紧凑内联表单，Overlay 右键/长按菜单使用独立对话页，不调用浏览器 Prompt 或创建新窗口。
- 验证：Hero、互动反馈、快捷操作和 ARIA 标签全部使用持久名称；改名页输入框自动聚焦，Esc 先返回根菜单，再次关闭并聚焦桌宠。
- 验证：260×300 Overlay、窄控制中心、200% 缩放、高对比度、中文/Emoji/长名称下输入、保存和错误反馈仍清晰可用。

### PET-069 持久家位置与旧快照迁移

- 验证：新 Pet 的家与初始位置一致；旧快照缺少 `homePosition` 时以最后持久位置一次性迁移并保存，后续重启不漂移。
- 验证：设置家只改变 `homePosition`，移动、拖拽和自主漫游只改变 `position`；返回家不重置关系、生命值、背包、纪念或角色资源。
- 验证：Command/Event 分别为 `pet.home.set` / `pet.home.changed` 与 `pet.home.return` / `pet.home.returned`，Trace 对应且 SQLite 重启恢复。

### PET-070 回家原生补偿与多屏恢复

- 验证：原生窗口先移动、Snapshot 后提交；Repository 失败时窗口回滚原位置且不发布 Event，Drag、Safe/Recovery Mode 无副作用。
- 验证：家所在显示器被移除、分辨率或 DPI 改变时，回家目标被约束到最大重叠或主显示器安全区；实际位置持久化但原始家锚点保留。
- 验证：断网、禁用 Provider、关闭控制中心后仍可从桌宠菜单设置家和回家，不调用 AI 或网页导航。

### UI-028 桌宠本体回家入口

- 验证：长按/右键菜单同时提供“回家”和“这里设为家”，操作后菜单关闭并显示温和确认；错误不伪造成功。
- 验证：260×300、最小窗口、200% 缩放和键盘导航下两个入口不被裁切，焦点顺序与读屏名称清晰。

### PET-071 原生窗口身份一致性与补偿

- 验证：应用启动时原生桌宠窗口标题来自持久 Pet Snapshot；改名成功后窗口管理器、辅助技术、Overlay 与控制中心使用同一规范化名称，重启后不回退到内置资源名。
- 验证：非法名称在接触原生窗口前失败；原生标题设置失败时不提交 Snapshot/Event，Repository 提交失败时原生标题回滚旧名称且不伪造成功。
- 验证：原生回滚也失败时同时保留持久提交和回滚两个错误原因；Safe/Recovery Mode、窗口缺失、断网和 Provider 禁用不产生身份分裂。

### PET-072 Presentation Profile 原生降扰

- 验证：切换到 `presentation` 时原生桌宠窗口隐藏、自主互动暂停，控制中心保持可用；以该 Profile 重启时桌宠从创建阶段即保持隐藏，不出现启动闪烁。
- 验证：切回其它 Profile 时桌宠在原持久位置恢复，置顶与穿透采用目标 Profile；任一原生策略步骤或 Profile 保存失败时，可见性、置顶、穿透和活动 Profile 一起回滚。
- 验证：Safe Mode 强制桌宠可见且可交互；系统托盘“恢复宠物交互”是明确用户覆盖，可恢复隐藏桌宠，并在事件中记录前后可见性。
- 验证：隐藏期间自主循环不执行屏幕恢复或漫游，不改变家锚点、关系、生命值、背包和角色资源；断网、关闭 Provider 与控制中心后策略仍成立。

### UI-029 演示与直播降扰说明

- 验证：Profile 创建表单明确说明桌宠会隐藏、自主互动暂停以及托盘恢复路径，不再误称“手动互动仍可用”。
- 验证：Profile 摘要标记“桌宠隐藏”；窄控制中心、200% 缩放、键盘和读屏下说明与保存操作不被裁切或混淆。

### PET-073 权威关系阶段投影

- 在 `BondPoints` 为 `0/24/25/99/100/299/300/999/1000` 时，验证阶段严格落在 `初识/熟悉/信赖/知己/长久相伴` 边界。
- 在旧快照 `BondPoints=0, Affinity=84` 时，验证有效陪伴值为 84、等级为 2、进度为 34、阶段为“熟悉”。
- 在 `MAX_BOND_POINTS` 时，验证阶段为“长久相伴”、无下一阶段且计算不溢出。
- 验证 Renderer 不再计算等级或阶段；Tauri `desktop_snapshot` 同时返回 Pet 原始状态与 Core 派生的 `petRelationship`。

### UI-030 关系阶段与低压力成长展示

- 关系卡首行展示“阶段 · Lv.”和累计陪伴值，进度条拥有可访问名称。
- 存在下一阶段时使用非倒计时、非惩罚文案；最高阶段显示关系仍会继续生长。
- 验证窄控制中心、125%/150% 缩放、长宠物名、零纪念与多纪念状态无裁切或横向溢出。
- 在离线、Provider 全禁用、控制中心重启后验证阶段一致，桌宠互动与成长不依赖 AI。

### PET-074 可访问径向宠物菜单

- 右键与 500ms 长按打开同一六向轮盘，高频操作为喂食、玩耍、梳理、背包、回家与更多；不得出现浏览器默认菜单。
- 验证方向键双向循环、Home/End 首尾跳转、Enter/Space 激活；Esc 从背包、改名或更多页先返回轮盘，再次 Esc 关闭并聚焦宠物。
- “更多”必须完整提供改名、这里设为家、休息和鼠标穿透，不得因径向布局删减原有能力。
- “更多”、背包与未来新增子页高度超过内容区时必须在窗口内纵向滚动，禁止溢出透明窗口或扩大 260×300 默认尺寸；滚轮/触控板滚动不得传递到宿主页。
- 用方向键和 Home/End 从首项遍历到末项，验证每个新焦点自动滚入最近可见区域，末端的休息、鼠标穿透与设置始终可操作；根径向菜单不得因通用滚动规则缩短或产生滚动条。
- 验证 260×300、125%/150%/200% 缩放、键盘焦点环、减少动态效果和读屏顺序；轮盘按钮不得裁切、重叠或遮挡状态恢复入口。

### PET-075 桌宠到控制中心安全深链

- 从“和我聊天”“开始任务”“设置”分别验证隐藏或最小化的控制中心被显示、取消最小化并聚焦，然后进入 Agent、Agent、设置页面。
- 深链仅接受 `agent_chat`、`agent_task`、`settings`；任意 URL、未知页面、额外字段和从非桌宠窗口调用均失败关闭。
- 窗口显示失败时不得伪造导航成功；事件审计来源必须记录 `pet`，托盘打开仍记录 `tray`。
- 断网、Provider 全禁用和控制中心关闭再恢复时，设置入口可用；Agent 页面明确呈现本地或 Provider 不可用状态，不影响桌宠继续运行。

### PET-076 Agent 工作陪伴反馈

- 启动耗时 Agent 任务，验证桌宠依次显示思考与运行文案并使用 `work` 动作；不得显示 Prompt、回答、路径或工具参数。
- 任务进入工具确认时验证桌宠恢复 `idle` 并提醒确认；确认后重新进入工作态，全部完成后进入 `celebrate`，4.2 秒后恢复 Core 当前状态与文案。
- Provider 失败、用户拒绝工具和任务取消分别验证失败/取消映射；终态不得永久覆盖睡眠、需求、情绪或自主行为。
- 关闭控制中心、禁用 Provider、断网及故意丢弃事件，验证桌宠仍可独立启动、拖动、喂食、玩耍、梳理、回家和打开菜单；事件发布失败不得使 Agent 任务失败。
- 在浏览器 Preview 和 Tauri 双窗口验证协议；Payload 严格只有 `spec/status/taskId/updatedAtMs`，未知状态、额外字段、畸形时间戳和错误版本必须在运行时被丢弃。

### PET-077 系统情境策略领域边界

- 验证 `screen_share` 即使遇到普通 `force_visible` 仍优先隐藏，且决策不携带窗口标题、进程名或捕获内容。
- 验证 `force_visible` 可覆盖游戏、全屏和免打扰，`force_hidden` 可覆盖普通自动状态，Safe Mode 始终强制可见并恢复交互。
- 验证信号最长 30 秒、同来源时间不可倒退、过期后自动回到 Profile 基础策略；休眠和 Adapter 崩溃不得造成永久隐藏。
- 验证 Sensor 只能产生策略事实，不能依赖 Tauri、Pet Repository、Profile Service 或直接调用窗口 API。
- 原生 Adapter 与 Desktop Coordinator 完成前，本用例只证明领域策略，不得据此宣称 macOS/Windows 自动感知已完成。

### PET-078 桌面呈现协调器与用户覆盖

- 验证活动 Profile 可见性与当前情境只产生派生决策，不复制或改写 Profile 真相；切换 Profile 后重新求值。
- 验证控制中心仅允许“自动避让 / 始终显示 / 始终隐藏”三档稳定值，其他窗口调用被拒绝；UI 使用标准 radiogroup 并可读地展示当前原因。
- 验证窗口原生应用或状态提交任一步失败时，可见性、置顶、穿透、覆盖值和决策一起回滚，不出现界面宣称成功但桌宠状态未变。
- 验证 Safe Mode 强制桌宠可见可交互；退出时按当前 Profile、Context 与 Override 重新求值，不恢复陈旧窗口快照。
- 验证托盘恢复收口到同一协调器并设置明确显示覆盖，但屏幕共享隐私仍可保持隐藏；不读取窗口标题、屏幕内容或会议信息。
- 当前此用例证明 Desktop Coordinator 与手动覆盖已接线；原生 Sensor 完成前不得宣称自动情境感知可用。

### PET-079 macOS 全屏 Sensor

- 验证只查询前台窗口的 `AXFullScreen` 布尔属性，脚本不包含标题、描述、内容或屏幕捕获字段。
- 验证单次采样最多 2 秒，超时会终止并回收 Adapter 子进程；连续失败按 5/10/20/30 秒有界退避，不产生进程风暴。
- 验证成功信号续租 15 秒；权限撤销、Adapter 错误或休眠后信号过期，自动模式恢复活动 Profile，不永久隐藏桌宠。
- 验证 Profile 切换、Safe Mode、托盘恢复、三档用户覆盖和后台 Sensor 使用同一串行窗口事务，不出现竞态覆盖或部分提交。

### PET-080 Windows 全屏 Sensor

- 验证前台可见、非最小化窗口覆盖其所在 Monitor 边界时产生全屏事实；普通最大化窗口因保留工作区边界不得误报。
- 验证多显示器负坐标、不同 DPI 和最多 2px DWM 框架舍入；超过容差、无前台窗口、边界查询失败均不得隐藏桌宠。
- 验证 `Progman`、`WorkerW`、`Shell_TrayWnd` 等桌面 Shell 表面不会被识别为全屏，不读取窗口标题、进程命令行或屏幕像素。
- 验证 Windows 与 macOS 使用相同的 15 秒租约、5/10/20/30 秒退避、Health Snapshot 和串行 Presence Transition；权限/API 故障必须过期恢复而非永久隐藏。
- 在签名 Windows 11 包上切换窗口化、最大化、无边框全屏、独占全屏和多屏前台，记录窗口可见性与自主行为；非 Windows 主机的源码或几何测试不能替代此真机门禁。
- 验证控制中心明确显示全屏感知正常、降级、不可用或停止；权限缺失不得伪装为“未全屏”。

### PET-081 Windows 游戏与免打扰 Sensor

- 用映射单元测试覆盖 `QUNS_RUNNING_D3D_FULL_SCREEN`、`QUNS_PRESENTATION_MODE`、`QUNS_BUSY`、`QUNS_QUIET_TIME`、`QUNS_NOT_PRESENT`、普通通知状态和未知值；Game 与 Do Not Disturb 必须是独立事实。
- 验证一次 Activity 采样同时续租 Do Not Disturb 与 Game Controller，失败时两者分别进入稳定 `activity-sample-failed` 退避，不能清除 Fullscreen Health。
- 验证停止流程把 Fullscreen、Do Not Disturb、Game 三项全部标记为 stopped；控制中心不得因最后一次更新覆盖而只显示单个 Sensor。
- 在签名 Windows 11 包上切换演示模式、系统 Quiet Time 可复现场景、D3D 独占/无边框全屏与普通窗口；验证自动隐藏、强制可见覆盖和恢复行为无闪烁。
- 分别运行 Vulkan、OpenGL 和窗口化游戏，确认 UI 不把未检测到解释为“没有游戏”；产品文案必须明确 Shell API 能力边界，不得宣传通用游戏识别或完整 Focus Assist 配置读取。

### PET-082 系统情境健康面板

- 向 UI 注入 Fullscreen、Do Not Disturb、Game 的乱序健康集合，验证按稳定产品顺序分别显示，任一项更新不得覆盖或清除其它项。
- 分别注入 available、degraded、unavailable、stopped 和空集合；验证状态文字、非纯颜色提示、失败次数与平台未报告说明准确，不渲染原始错误正文或用户内容。

### PET-083 macOS 勿扰与专注模式 Sensor

- 验证 Adapter 只执行 `/usr/bin/notifyutil -g com.apple.donotdisturb.status`，不读取 `~/Library/DoNotDisturb`、Focus 名称、通知正文、窗口标题、应用名或屏幕像素。
- 解析测试只接受 `com.apple.donotdisturb.status 0` 与 `1`；未知数值、错误名称、缺字段、额外字段和超长输出必须失败关闭。
- 验证单次采样最多 2 秒，超时杀死并回收子进程；失败进入独立 5/10/20/30 秒退避，Fullscreen Sensor 健康和信号不受影响。
- 验证成功续租 15 秒并经 Desktop Presence Coordinator 隐藏和抑制自主行为；关闭 Focus 后恢复，Force Visible、Force Hidden、Safe Mode 与 Screen Share 优先级保持领域矩阵。
- 控制中心打开时切换 Focus，验证 `system-context-changed` 仅提示重新读取 Snapshot，健康状态和当前依据实时收敛且事件 Payload 不包含布尔状态或用户内容。
- 在签名 macOS 13、14、15、26 包上执行真实 Focus 开关、睡眠唤醒和命令不可用演练；当前开发机命令输出与单元测试不能替代跨版本发布门禁。
- 使用读屏检查健康列表名称、逐项能力名、状态和说明；状态刷新通过 polite live region 宣告，不抢占当前焦点。
- 在 1180px、900px、720px 和控制中心最小宽度检查三列、两列、单列收敛，无横向滚动、文本遮挡、徽标挤压或不可读字号。
- Browser Preview 只证明 DOM、响应式和视觉设计；Windows/macOS 签名 Tauri 包仍需验证真实 Sensor 状态刷新、字体渲染、缩放与窗口合成，不得以浏览器截图替代。

## DP-PERCH — 边缘栖息

| ID | 场景 | 预期 |
|---|---|---|
| DP-PERCH-001 | Runtime 播放 `perch` | 状态为 `perching`、情绪为 `neutral`，动作目录包含且仅包含一次 `perch` |
| DP-PERCH-002 | 桌宠菜单触发栖息 | 经 `play_pet_action` 类型化 IPC 执行，不由 Renderer 直接改状态或移动窗口 |
| DP-PERCH-003 | 宠物位于 Bottom/BottomLeft/BottomRight | 统一角色舞台应用底边栖息锚点，气泡、菜单、阴影和命中区不被变换 |
| DP-PERCH-004 | 宠物位于 Free/Left/Right/Top | 动作保持确定性但不伪装攀爬或探头，不越过原生工作区安全边界 |
| DP-PERCH-005 | Sprite/glTF 缺少 `pet.perch` | 按 Manifest 回退链解析，最终稳定回退 `pet.idle` |
| DP-PERCH-006 | VRM 播放 `pet.perch` | 只请求白名单 `relaxed` 表情，缺失 Preset 时安全无表情降级 |
| DP-PERCH-007 | 开启 Reduced Motion | 无循环位移动画，保留可识别的静态栖息姿态 |
| DP-PERCH-008 | 用户代码/Automation/Agent 请求 | 从共同 Action Catalog 发现 `perch`，沿既有授权、确认、审计和 Safe Mode 门禁执行 |
| DP-PERCH-009 | 拖拽、Safe/Recovery 或重启 | 拖拽优先；受限模式拒绝写动作；恢复后为 Idle，不持久化瞬时姿态 |

## DP-CLIMB — 侧边攀爬

| ID | 场景 | 预期 |
|---|---|---|
| DP-CLIMB-001 | Runtime 播放 `climb` | 状态为 `climbing`、情绪为 `focused`，Action Catalog 精确包含一次 `climb` |
| DP-CLIMB-002 | 桌宠位于 Left/TopLeft/BottomLeft | 统一角色舞台使用左侧支点、向墙轻倾并播放有界上下攀爬反馈 |
| DP-CLIMB-003 | 桌宠位于 Right/TopRight/BottomRight | 使用镜像后的右侧支点和相反倾角，菜单、气泡与命中区不镜像或旋转 |
| DP-CLIMB-004 | 桌宠位于 Free/Top/Bottom | 不施加侧边倾角，不把攀爬冒充顶部探头或底边栖息，不移动原生窗口 |
| DP-CLIMB-005 | Sprite/glTF 缺少 `pet.climb` | 按 Manifest 回退链稳定落到 `pet.idle`，Renderer 不白屏、不冻结 |
| DP-CLIMB-006 | VRM 播放 `pet.climb` | 只请求白名单低权重 `surprised`，缺失 Preset 时安全无表情降级 |
| DP-CLIMB-007 | 开启 Reduced Motion | 停止上下循环，保留静态侧边倾角、支点和动作可识别性 |
| DP-CLIMB-008 | 菜单、用户代码、Automation、Agent 请求 | 共享 `climb` 词汇并沿类型化 IPC、授权、确认、审计和 Safe Mode 门禁执行 |
| DP-CLIMB-009 | 拖拽、重启、Renderer 切换 | 拖拽优先；重启恢复 Idle；所有 Renderer 对同一 Snapshot 产生一致动作选择 |

## DP-PEEK — 顶部探头

| ID | 场景 | 预期 |
|---|---|---|
| DP-PEEK-001 | Runtime 播放 `peek` | 状态为 `peeking`、情绪为 `surprised`，Action Catalog 精确包含一次 `peek` |
| DP-PEEK-002 | Top/TopLeft/TopRight Surface | 统一角色舞台使用顶部支点与有界探出反馈，不改变窗口持久坐标 |
| DP-PEEK-003 | Free/Left/Right/Bottom Surface | 不施加顶部探出位移，不冒充攀爬或栖息，不越过 Work Area |
| DP-PEEK-004 | 内置角色动作 | 眼睛和星标表现好奇，探出周期低幅度且不造成窗口点击区漂移 |
| DP-PEEK-005 | Sprite/glTF 缺少 `pet.peek` | 按 Manifest 回退链稳定落到 `pet.idle`，加载和 WebGL 失败继续沿既有安全降级 |
| DP-PEEK-006 | VRM 播放 `pet.peek` | 只请求白名单有界 `surprised`，缺失表达式时安全无表情降级 |
| DP-PEEK-007 | 开启 Reduced Motion | 停止循环探出，保留静态顶部位置、表情和可识别空间语义 |
| DP-PEEK-008 | 菜单、用户代码、Automation、Agent 请求 | 共享 `peek` 词汇，并经过类型化 IPC、授权、确认、审计和 Safe Mode 门禁 |
| DP-PEEK-009 | 拖拽、重启、跨屏与系统栏变化 | 拖拽优先、重启 Idle；Surface 重新分类后不保留错误顶部投影 |

## DP-SETTLE — 拖放自动边缘收敛

| ID | 场景 | 预期 |
|---|---|---|
| DP-SETTLE-001 | Drop 到 Bottom/BottomLeft/BottomRight | 单次持久更新写入最终坐标与 `perching`，事件记录 `settleAction=perch` |
| DP-SETTLE-002 | Drop 到 Left/Right | 单次更新收敛为 `climbing`；Top/Bottom Corner 不因同时靠侧边而选择 Climb |
| DP-SETTLE-003 | Drop 到 Top/TopLeft/TopRight | 单次更新收敛为 `peeking`，顶部 Corner 映射稳定且唯一 |
| DP-SETTLE-004 | Drop 到 Free 或无当前显示器 | 收敛为 Idle，不猜测主屏，不复用上一次 Surface 动作 |
| DP-SETTLE-005 | Core 接收落点动作 | 仅允许 Idle/Perch/Climb/Peek；Walk/Sleep/Work/Celebrate/Observe 均拒绝且保持 Dragged 与原位置 |
| DP-SETTLE-006 | Runtime 保存失败 | 坐标、状态和事件零提交，Pet 保持 Dragged，Desktop 不发布 Surface 变化 |
| DP-SETTLE-007 | 边缘吸附后 Runtime 保存失败 | 原生窗口补偿回拖放前位置；补偿失败显式报错，不宣称事务成功 |
| DP-SETTLE-008 | Drop 成功 | 清除 Drag 标记后只发布一次稳定 Surface 事件，Renderer 从同一 Snapshot 选择动作 |
| DP-SETTLE-009 | 关闭 edge snap | 仍对最终实际坐标分类；仅精确位于 Surface 容差内时自动动作，否则 Idle |

## DP-EDGE-WANDER — 沿边自主移动

| ID | 场景 | 预期 |
|---|---|---|
| DP-EDGE-WANDER-001 | Perching/Climbing/Peeking 到达自主调度时间 | 可以启动 Observe、Explore 或 Rest，并记录对应 Perch/Climb/Peek `resumeAction` |
| DP-EDGE-WANDER-002 | 边缘自主动作正常完成 | 恢复动作开始前的边缘姿态，清空恢复点并进入正常 Cooldown，不回到错误 Idle |
| DP-EDGE-WANDER-003 | Profile Quiet/Focus 在动作中生效 | 立即抑制动作并恢复原边缘姿态；不访问网络、不等待 AI 或权限确认 |
| DP-EDGE-WANDER-004 | 用户在沿边移动中开始拖拽 | Dragged 优先，恢复点被丢弃；后续 Interrupt 不覆盖用户状态 |
| DP-EDGE-WANDER-005 | Top/TopLeft/TopRight Explore | 固定安全 Top 坐标，仅水平移动；顶部 Corner 不切换为侧边攀行 |
| DP-EDGE-WANDER-006 | Bottom/BottomLeft/BottomRight Explore | 固定安全 Bottom 坐标，仅水平移动；底部 Corner 不切换为侧边攀行 |
| DP-EDGE-WANDER-007 | Left/Right Explore | 固定对应安全横坐标，仅垂直移动，不越过顶部或底部生命体安全边距 |
| DP-EDGE-WANDER-008 | 位于轴向端点且序列指向外侧 | 规划器自动反向产生非零有界目标，不发生重复原地 Walking |
| DP-EDGE-WANDER-009 | Free Explore | 保持原二维 Wander 规则、方向序列和跨负坐标显示器安全约束 |
| DP-EDGE-WANDER-010 | 当前显示器消失、Safe/Recovery 或状态被抢占 | 原生移动停止且不跨屏猜测；Desktop Coordinator 仍为唯一窗口执行者 |
| DP-EDGE-WANDER-011 | 持久快照包含非法恢复动作或无活动 Intent 的恢复点 | Core 校验拒绝恢复，不执行 Work/Sleep/Celebrate 等越权姿态 |
| DP-EDGE-WANDER-012 | 重启与旧快照迁移 | 缺失 `resumeAction` 默认 `null`；瞬时自主状态沿启动恢复规则收敛，不伪造边缘移动完成 |

## DP-CURSOR-APPROACH — 鼠标关注靠近

| ID | 场景 | 预期 |
|---|---|---|
| DP-CURSOR-APPROACH-001 | Free Surface 进入自主 Walking，光标在同一 Work Area 远处 | 本轮只采样一次全局光标，宠物向光标方向移动且不持续追踪 |
| DP-CURSOR-APPROACH-002 | 光标在宠物左/右/上/下方向 | 单次水平位移不超过 140px、垂直位移不超过 96px，目标始终位于安全 Work Area |
| DP-CURSOR-APPROACH-003 | 规划后的宠物接近光标 | 窗口中心至少保持“窗口半对角线 + 96px”距离，不覆盖或抢占当前点击点 |
| DP-CURSOR-APPROACH-004 | 光标已经靠近 | 不产生抖动式微移，回退既有确定性 Wander 或保持当前自主调度语义 |
| DP-CURSOR-APPROACH-005 | 光标位于其它显示器、Work Area 外或采样失败 | 不跨显示器追逐，不报错中断自主循环，确定性回退既有 Wander |
| DP-CURSOR-APPROACH-006 | 全局坐标为 NaN、Infinity 或极端值 | 规划器失败关闭，不产生非法转换、越界位置、日志或事件泄漏 |
| DP-CURSOR-APPROACH-007 | 负坐标副屏与多显示器 Work Area | 只在当前宠物 Work Area 内靠近，目标兼容负坐标并保留系统边缘安全区 |
| DP-CURSOR-APPROACH-008 | 宠物处于 Top/Bottom/Left/Right/Corner Surface | 不采样光标；继续使用沿边 Wander，Perch/Climb/Peek 不脱离 Surface |
| DP-CURSOR-APPROACH-009 | 移动中开始拖拽、进入 Safe/Recovery 或状态离开 Walking | 下一动画帧停止原生移动，不覆盖用户拖拽或更新状态 |
| DP-CURSOR-APPROACH-010 | Quiet/Focus/Presentation 或低生命值策略抑制自主行为 | 不进入鼠标采样和靠近路径，不绕过现有 Profile 与健康门禁 |
| DP-CURSOR-APPROACH-011 | 断网、Provider 全禁用、控制中心关闭 | 行为仍完整可用，不启动 AI、Agent、Skill、User Program 或权限确认 |
| DP-CURSOR-APPROACH-012 | 隐私与能力边界审计 | 光标坐标不持久化、不上传、不写 Runtime Event/诊断，不暴露给 Renderer、Sensor、AI、Agent、Skill 或 User Program |
| DP-CURSOR-APPROACH-013 | macOS/Windows 真机多屏、混合 DPI、快速移动光标 | 原生透明窗口平滑有界移动且不跳屏；Browser Preview 不得用于替代此证据 |
| DP-CURSOR-APPROACH-014 | 旧 Profile 缺失 `cursorApproachEnabled` | Schema 与 Rust Merge 均解析为开启，重启后行为保持兼容且不要求迁移写回 |
| DP-CURSOR-APPROACH-015 | 活动 Profile 关闭鼠标关注 | Host 在调用全局光标 API 前分支，Free Surface 使用原确定性 Wander，其它自主动作保持启用 |
| DP-CURSOR-APPROACH-016 | Profile 创建、编辑、切换与 Browser Preview | 复选框、摘要和持久值一致；切换后下一轮自主 Explore 使用新策略，不需重启 |
| DP-CURSOR-APPROACH-017 | AI Creator 生成关闭靠近的 Profile | 严格契约接受布尔值，隔离预览显示关闭；创建不激活、不授予光标或窗口能力 |
| DP-CURSOR-APPROACH-018 | 键盘、读屏、200% 缩放和窄控制中心 | 控件名称、隐私说明、勾选状态和保存操作完整可达且不被裁切 |

## DP-DOUBLE-CLICK — 正式双击互动

| ID | 场景 | 预期 |
|---|---|---|
| DP-DOUBLE-CLICK-001 | 单次左键点击 | 等待 220ms 后只调用 `click_pet`，保持既有 Mood +2、Affinity +1、BondPoints +1 契约 |
| DP-DOUBLE-CLICK-002 | 第二击在窗口内到达 | 取消待处理单击，只调用一次 `double_click_pet`，不额外提交单击成长 |
| DP-DOUBLE-CLICK-003 | 浏览器报告 Click Detail 3/4 | 忽略后续击次，不重复提交双击或重新调度单击 |
| DP-DOUBLE-CLICK-004 | Runtime 双击成功 | 状态 Interacting、情绪 Happy、Mood +5、Affinity +2、BondPoints +3，数值在 100/安全整数边界饱和 |
| DP-DOUBLE-CLICK-005 | Command/Event 契约 | 使用 `pet.interaction.double-click` 与 `pet.interaction.double-clicked`，Trace 相关且记录位置、按键、前后关系投影 |
| DP-DOUBLE-CLICK-006 | Repository 保存失败 | Snapshot、成长和 Event 零提交，不展示已成功的领域状态 |
| DP-DOUBLE-CLICK-007 | Dragged、Safe 或 Recovery | Host/Core 失败关闭，不覆盖拖拽，不发放成长，不产生成功事件 |
| DP-DOUBLE-CLICK-008 | 互动反馈计时结束前发生更新状态 | `finish_interaction` 不能覆盖更新的 Drag/Work/Sleep 等状态 |
| DP-DOUBLE-CLICK-009 | Preview 与 Tauri Platform Port | 方法和参数完全一致；Preview 只模拟数值，Tauri 调用精确类型化命令 |
| DP-DOUBLE-CLICK-010 | 离线、无 Provider、控制中心关闭 | 双击完整可用，不访问网络、不启动 Agent、不读取桌面内容 |
| DP-DOUBLE-CLICK-011 | 内置/Sprite/glTF/VRM/未来 Live2D | 共享 Interacting 语义与统一角色舞台，不要求资产实现双击专属窗口逻辑 |
| DP-DOUBLE-CLICK-012 | 键盘激活按钮 | Click Detail 0 继续按单击处理，不错误识别为双击，保留辅助技术可达性 |
## QQ 宠物式低打扰状态气泡

| ID | 场景 | 预期结果 |
|---|---|---|
| DP-BUBBLE-001 | 启动或本地状态发生变化 | 气泡无需悬浮即可出现，并在 3–5 秒内自动退场 |
| DP-BUBBLE-002 | 菜单打开 | 气泡隐藏，不遮挡菜单项，不改变菜单焦点 |
| DP-BUBBLE-003 | 拖拽、抚摸或其它指针手势进行中 | 气泡隐藏，手势结束后不抢焦点 |
| DP-BUBBLE-004 | 气泡退场后悬浮或键盘聚焦宠物 | 最近一句可重新查看，不触发领域副作用 |
| DP-BUBBLE-005 | 辅助技术监听状态 | 使用 polite、atomic live region，不中断当前朗读 |
| DP-BUBBLE-006 | 系统启用减少动态 | 文本仍可见，入退场过渡被全局 Reduced Motion 规则取消 |
| DP-BUBBLE-007 | 离线且未配置 Provider | 启动、状态和互动气泡完整可用，不产生网络请求 |
| DP-BUBBLE-008 | 重启应用或检查事件日志 | 瞬时气泡不持久化、不写 Pet Snapshot、事件或日志 |
| DP-BUBBLE-009 | 260×300 原生窗口、不同 DPI 和角色资产 | 气泡不裁切、不遮挡角色面部；必须以 Tauri 真机截图验收 |
| DP-BUBBLE-010 | 8 秒内连续收到多个自主状态 | 只接受首个状态，后续状态不重置退场计时、不形成气泡风暴 |
| DP-BUBBLE-011 | 互动或错误反馈展示期间收到普通状态 | 当前反馈完整展示，普通状态不得覆盖 |
| DP-BUBBLE-012 | 连续两次产生相同互动文案 | 两次均生成独立展示修订并重新开始退场计时 |
| DP-BUBBLE-013 | 空白或仅空格表达 | 调度器拒绝，不改变当前文字、可见性或计时 |
| DP-BUBBLE-014 | 64 字宠物名进入改名反馈 | 按 Unicode 字符缩略到 42 字并带省略号，不拆分代码点、不水平溢出 |
| DP-BUBBLE-015 | 260×300、200% 缩放与长词 | 气泡保留双侧安全区、正常换行，尾巴指向角色且正文不被裁切 |
| DP-BUBBLE-016 | Forced Colors | 主体和尾巴使用 Canvas/CanvasText，边界清晰且文字可读 |
| DP-BUBBLE-017 | 自定义主题或角色包 | 只能映射批准的气泡 Token，角色资产不能注入 CSS 或改变命中区 |
| DP-BUBBLE-018 | 自主状态与 Agent 陪伴并发 | 两者先经同一宿主 Attention Gateway；被环境策略或共享预算拒绝时不展示、不抢占现有反馈 |
| DP-BUBBLE-019 | Renderer、Skill 或用户代码尝试绕过 | 只能提交版本化请求，不能直接消费、补充或重置宿主预算，也不能直接移动窗口 |

## DP-OVERLAY — QQ 宠物式桌面 Overlay

| ID | 场景 | 预期结果 |
|---|---|---|
| DP-OVERLAY-001 | 默认停留且无指针/键盘互动 | 仅角色与当前瞬时气泡可见，不常驻网页式工具面板 |
| DP-OVERLAY-002 | 指针进入角色命中区 | 显示紧凑快捷工具条和温和反馈；工具条不遮挡脸部、不扩大原生窗口 |
| DP-OVERLAY-003 | 指针离开 Overlay | 快捷工具条按动效规范收起，角色继续常驻且不触发领域副作用 |
| DP-OVERLAY-004 | 鼠标右键或长按角色 | 打开同一宿主受控菜单，不显示浏览器原生菜单，不导航网页 |
| DP-OVERLAY-005 | 主角色按钮聚焦后按 `Shift+F10` 或 Menu 键 | 打开与鼠标等价的菜单并聚焦首项，读屏获得菜单与动作名称 |
| DP-OVERLAY-006 | 菜单打开时按方向键、Home、End、Enter、Escape | 焦点确定性移动；激活单一动作；Escape 按层级返回并最终聚焦角色 |
| DP-OVERLAY-007 | `260×300`、200% 缩放、长宠物名和 Forced Colors | 角色、气泡、焦点与菜单均可辨识，无页面级滚动和关键操作裁切 |
| DP-OVERLAY-008 | 断网、Provider 禁用、控制中心关闭 | 抚摸、拖动、照料、背包、回家和本地菜单保持可用，不启动 Agent |
| DP-OVERLAY-009 | Renderer、AI、Skill 或用户程序请求移动/置顶/穿透 | 请求不能直接执行；仅 Desktop Coordinator 可在能力与策略检查后改变窗口 |
| DP-OVERLAY-010 | Chrome Browser Preview 审计 | 只记录排版、焦点、ARIA、菜单和 Console 结果，不签署透明合成、置顶、DPI、多屏或 Work Area |
| DP-OVERLAY-011 | Tauri 签名包真机审计 | 分别验证透明合成、置顶、桌面拖拽、点击穿透、托盘召回、离屏夹回与 WebView 自愈 |

## AST-VRM-EXPR — VRM 包级表情映射

| ID | 场景 | 预期结果 |
|---|---|---|
| AST-VRM-EXPR-001 | VRM 包声明合法映射文件 | Installer 从完整性清单复验 JSON，并把映射投影到活动 Renderer Snapshot |
| AST-VRM-EXPR-002 | glTF、Sprite、Theme 或其它非 VRM 包声明映射 | 安装失败，不忽略错误入口、不降级成已安装包 |
| AST-VRM-EXPR-003 | 动作为 `vendor.*`、Preset 为私有名称或存在未知字段 | Schema 与 Installer 均失败关闭，不把任意模型参数带入 Renderer |
| AST-VRM-EXPR-004 | 权重为负数、超过 1、NaN、Infinity 或映射超过 64 项 | 安装失败，旧活动角色和上一版本保持不变 |
| AST-VRM-EXPR-005 | 当前动作存在包级映射 | 每次先 Reset 旧 Expression，再对模型已有的标准 Preset 应用声明权重 |
| AST-VRM-EXPR-006 | 当前动作未在包级映射中 | 使用宿主固定安全默认；无默认时回到 Neutral，不继承上一个动作表情 |
| AST-VRM-EXPR-007 | 模型缺少目标 Preset或 Expression Manager 抛错 | 不执行替代私有参数，不破坏角色交互，安全回退 Neutral/内置角色策略 |
| AST-VRM-EXPR-008 | 活动 3D 角色通过 Tauri Snapshot 加载 | `animationMap` 与 `vrmExpressionMap` 均完整保留，Browser Preview 不伪造第三方映射 |
| AST-VRM-EXPR-009 | AI、Skill、Automation 或用户程序尝试直接改表情 | 只能选择公开 Pet Action；不能获得 Expression Manager、BlendShape 或 Renderer 写权限 |

## PET-3D-BUILTIN — 默认 3D 桌面伙伴

| ID | 场景 | 预期结果 |
| --- | --- | --- |
| PET-3D-001 | 全新离线安装后在 macOS/Windows 原生桌面启动并覆盖 Idle、Observe、Walk、Play、Sleep | 无需运行时下载或导入即可显示专业骨骼 Fox；Survey、Walk、Run 精确映射状态；发布物包含 CC0/CC BY 4.0 来源与署名 |
| PET-3D-002 | 分别在浅色、深色、动态壁纸和多显示器上移动宠物并截图采样窗口四角 | 仅角色、影子、气泡和显式菜单可见；窗口四角 Alpha 为透明；无矩形底色、系统标题栏或窗口阴影 |
| PET-3D-003 | 分别注入 Fox 加载失败、程序化 WebGL Context 创建失败、Context Lost 与 React Renderer 异常 | 依次回退程序化 3D 与本地 SVG，并给一次温和反馈；无空 Canvas、崩溃、重试风暴或网络请求 |
| PET-3D-004 | 将健康宠物自主序列推进至 Play，等待动作结束并重启 | Playing/Happy、Renderer `pet.play` 和状态文案一致；动作有界结束回 Idle；Snapshot 可迁移且离线可用 |
| PET-3D-005 | 构建生产包并校验 `public/models` 与 `dist/models` | `companion-fox.glb` 必须存在且 SHA-256 为 `d97044e701822bac5a62696459b27d7b375aada5de8574ed4362edbba94771f7`；离线断网启动仍可加载 |
| PET-3D-006 | 检查所有公开 `pet.*` 动作映射 | 每个动作均绑定 Survey、Walk 或 Run；未知动作稳定回 Idle，不因缺失 Clip 停止渲染 |
