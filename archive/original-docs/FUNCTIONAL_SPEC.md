# DeskPet Platform — 功能规格说明书（原始需求稿）

> **文档状态：需求来源。** 当前功能基线见 `docs/PRODUCT_SPEC.md`，资源与皮肤规范见 `docs/CUSTOMIZATION_ASSETS.md`。

> 配套 PRD.md / TECH_SPEC.md。本文定义每个一级功能的**详细行为、界面、交互、边界、异常**，
> 作为开发实现与测试验收的直接依据。

---

## FS-01 核心桌宠（Pet Core）

### 1.1 窗口
- 启动后创建无边框、背景透明的常驻窗口，默认右下角 1/4 屏内随机位置。
- 顶栏/系统按钮不可见；仅宠物可交互区域接收鼠标。
- 「置顶」开关：开启时 `alwaysOnTop=true`；mac 全屏 Space 下允许被盖（已知差异）。
- 「点击穿透」开启时窗口 `ignoreMouseEvents=true`（mac）/ 等效 API（Win）；仅托盘/快捷键可恢复互动。
- 拖拽：在宠物主体 24px 命中区内按下拖动，实时跟随；松开落点记忆。
- 多显示器：记录 `displayId + x + y`；插拔后回到原屏，无则主屏靠边。
- 缩放：设置 50%–200%，步长 25%；DPI 感知重绘。

### 1.2 状态机
状态：idle, walk, sleep, sit, look_around, drag, click, hungry, happy
- 初始 idle；按权重随机进入 walk/sit/look。
- walk 到屏幕边缘（留 8px）回头反向。
- 系统时间 23:00–07:00 权重提升 sleep。
- drag 优先级最高；drop 后回 idle。
- click：播放 click 动画 600ms 后回 idle。
- hungry（hunger<30）插播 beg 动画，可打断。

### 1.3 数值与养成
- 数值：hunger/energy/mood/affinity（0–100）。
- 每 10min hunger-2、energy 随时间波动；互动+affinity。
- 喂食：hunger+30；抚摸：mood+10、affinity+5。
- 连续互动 7 天解锁成就「形影不离」，解锁 special 动作。

### 1.4 异常
- 皮肤资源损坏：回退默认 + Toast「皮肤加载失败，已使用默认」。
- 配置损坏：备份坏文件 `config.bak` 并重建默认。
- 锁屏恢复：位置/状态保持。

---

## FS-02 高自定义（Customization）

### 2.1 皮肤包
目录结构：
```
my-skin/
  manifest.json   # id,name,author,version,frameSize,animations[]
  preview.png
  sprites/*.png
  audio/*.wav
  meta/hitboxes.json
```
- 切换即热重载；失败回退默认。

### 2.2 交互绑定
设置项：单击=互动动画 / 双击=打开互动面板 / 右键=菜单 / 拖拽=移动。
支持按 Profile 不同绑定。

### 2.3 Profile
预设：work / relax / night。切换时套用 settingsOverride（置顶/穿透/缩放/启用技能/规则）。
一键切换入口：托盘、命令面板、自动化动作 `profile.switch`。

### 2.4 配置导入导出
导出 `deskpet-config-<date>.zip` 含 config/profiles/skills-override/automations（不含密钥）。
导入前预览差异，支持覆盖/合并。

---

## FS-03 自动化与提醒（Automation）

### 3.1 规则模型
```
trigger → conditions[] → actions[] → cooldown + priority
```
- trigger：interval/cron/event/hotkey/command/webhook
- conditions：time_range/weekdays/not_fullscreen/profile_in/random/pet_state
- actions：pet.say/pet.playAnim/pet.moveTo/pet.setMode/notify.system/notify.pet/open.url/open.app/profile.switch/automation.enable/command.run/skill.invoke/script.eval/chain/wait

### 3.2 编辑器
三视图同步：表单 / 流程图 / YAML。保存校验 schema；测试运行不落库。

### 3.3 提醒模板
内置：喝水(60m)、久坐(45m)、眼保健(20m)、会议前(5m)、下班(18:30)。
提醒演出：宠物跑角落挥手+气泡+可选系统通知；按钮 [去走走][+5min][今日关闭]。
贪睡记 `reminder.snoozed`；完成记 `reminder.done` 并 +affinity。

### 3.4 智能勿扰
当检测到前台全屏应用或 `profile=focus` 时，提醒延后至退出后 5min，最多延 3 次。

---

## FS-04 快捷指令（Command Palette）

### 4.1 交互
- 全局热键唤出（默认 Ctrl/Cmd+Shift+P）。
- 输入框模糊搜：命令标题、id、别名、技能名。
- 选中命令若有 argsSchema 弹出二次输入。
- 最近使用 5 条置顶。
- Esc 关闭；方向键导航；Enter 执行。

### 4.2 命令来源
内置命令 + 技能注册 commands + 用户自定义（绑定到脚本/规则）。

---

## FS-05 技能系统（Skill）

### 5.1 包结构
```
skill.<id>/
  skill.json        # 清单
  main.js / dist/
  ui/ assets/ locales/
  signature.sig
```

### 5.2 skill.json 关键字段
```json
{
  "id":"skill.com.x.pomodoro",
  "name":"番茄钟",
  "version":"1.2.0",
  "engines":{"deskpet":">=1.0.0"},
  "main":"dist/main.js",
  "permissions":["pet.control","notify","commands.register","automation.register"],
  "contributes":{
    "commands":[{"id":"pomodoro.start","title":"开始番茄钟"}],
    "menus":{"pet.context":[{"command":"pomodoro.start","group":"tools"}]},
    "settings":[{"title":"番茄钟","page":"ui/settings.html"}],
    "automations.actions":["pomodoro.start","pomodoro.pause"],
    "tools":[{"id":"pomodoro.start","description":"开始番茄","risk":"safe"}]
  },
  "activationEvents":["onCommand:pomodoro.start","onStartup"]
}
```

### 5.3 权限分级
- 安全：pet.control/ui.bubble/storage/commands.register
- 敏感：notify/hotkey/window.create/net.http(出站目标白名单内)
- 高危：fs/clipboard/net.(http自由域|udp非本机)/open.app/system.shell
- 首次装高危权限弹窗列明；运行时调用敏感 API 二次确认。

### 5.4 生命周期与隔离
- `activate(ctx)` 注册贡献；`onEnable/onDisable`；`onTick(dt)` 有 16ms/帧配额与超时。
- 独立 Node 进程；崩溃发 `skill.crashed` 事件，Core 标记禁用并通知用户。
- 卸载清理 storage（可保留勾选）。

### 5.5 行为钩子
技能可注册 `beforeStateChange/afterStateChange/onDecideNextAction/onInteract`，影响性格权重。

---

## FS-06 开放平台与数据出站（Open Platform）

### 6.1 Local Gateway（入站）
- 默认关闭；首次需用时在设置启用，绑定 127.0.0.1。
- 端点：REST `/v1/*`、WS `/v1/events`、SSE `/v1/events/sse`、Health `/healthz`。
- 鉴权：Bearer `<token>`；Scope 见 PRD FR-H02。
- 危险：绑 0.0.0.0 需 `danger:true` + 红字警告 + 审计标红。

### 6.2 出站 Connector（详见 EVENTS_CONNECTORS.md）
- 四种：http_webhook / websocket_client / sse_client / udp。
- 统一接口 `onEvent(e)`；支持 filter/template/retry/rateLimit。
- 审计每条出站；一键安全模式停全部。

### 6.3 URI Scheme
`deskpet://install?package=<url>`、`deskpet://command?id=<id>`、`deskpet://pet/say?text=`。
网页按钮一行即可导流。

### 6.4 CLI
```
deskpet status                # 打印运行时状态
deskpet say "你好"            # 宠物说话
deskpet api <method> <path>   # 带 token 调本地 Gateway
deskpet pack <dir>            # 打包技能
deskpet sink http [port]      # 起 mock webhook 接收并打印
deskpet sink udp [port]       # 起 mock udp 监听
```

### 6.5 SDK
`@deskpet/client-js`：`DeskPet.connect({token})` → `pet.say/play/events.on`。

---

## FS-07 第三方 AI 与 Agent（详见 AI_AGENT.md）

### 7.1 Provider 配置
设置页：新增 Provider（kind/baseUrl/apiKey 引用/model/温度/步数）。
密钥存 SecureStore；配置仅 `${SECRET:name}`。

### 7.2 Agent 对话
入口：宠物双击「和我聊聊」/命令面板 `AI:`/外部 `/v1/agent/chat`。
流程：用户输入 → Router 选 Provider+装 system+聚 Tools → LLM tool_calls → Executor（确认流）→ 宠物演出+审计。

### 7.3 确认流
- `planConfirmRequired=true`：先弹计划 [执行][修改][取消]。
- danger 工具（安装技能/打开应用/改系统）单独确认。
- `autoExecuteTools=false` 默认。

### 7.4 降级
无 Key 或模型不可达：规则意图匹配到命令；未命中提示配置 AI。

### 7.5 记忆与主动
记忆本地可查删导出；主动建议接 Automation 触发，≤ `maxPerHour`，可「别烦我」冷静 2h。

---

## FS-08 技能仓库（Repository）P1
- 多源：官方/私有 URL/本地文件/Git。
- 商店页：发现/分类/已装/更新/创作者。
- 包签名：hash+作者签名+商店签名；更新 semver 通道 stable/beta。
- 审核分级：Official/Verified/Community/Local。

---

## FS-09 异常与边界总表
| 场景 | 处理 |
|------|------|
| 皮肤损坏 | 回退默认+Toast |
| 配置坏 | 备份重建 |
| 技能崩溃 | 禁用+通知 |
| 更新失败 | 提示重试不阻断 |
| 权限不足自启 | 引导系统设置 |
| 出站目标不可达 | 重试/审计失败 |
| Token 泄露风险 | 可撤销+最短有效期 |
| Agent 死循环 | maxSteps 截断 |

---

## FS-10 验收场景（E2E 抽样）
1. 启动→宠物可见可点→产生 pet.click 事件。
2. 配 Webhook Connector→点击宠物→mock 收到 envelope。
3. 配 UDP Connector→事件→监听 39789 收到帧。
4. WS/SSE 客户端连 Gateway→实时收事件。
5. 配 OpenAI Key→命令面板「进入工作状态」→Agent 调 profile+番茄并说话。
6. 安全模式→Gateway 拒连、出站停止。
7. 装高危权限技能→确认弹窗→运行隔离。
