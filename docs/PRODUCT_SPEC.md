# DeskPet 产品规格

> 版本：0.1.0-draft  
> 更新日期：2026-07-17  
> 状态：开发基线

## 1. 产品定义

DeskPet 是本地常驻的桌面生命体平台。用户无需编程即可获得陪伴、提醒和自动化；创作者可发布角色资源、技能、连接器和 Agent Pack；第三方软件可在授权后读取事件或调用能力。

产品不是单纯聊天窗口，也不是无约束的本地脚本执行器。宠物体验是入口，能力注册与安全执行是平台核心。

## 2. 用户层级

| 层级 | 典型用户 | 核心体验 |
|---|---|---|
| L0 | 普通用户 | 安装、选择角色、点击互动、启用提醒 |
| L1 | 进阶用户 | Profile、模板、连接器、自动化表单 |
| L2 | 高级用户 | YAML、事件调试、私有源、自定义资源 |
| L3 | 创作者 | SDK、Creator Studio、打包、签名、发布 |
| L4 | 厂商/团队 | 私有 Registry、设备策略、集成与审计 |

## 3. 核心体验循环

```text
看见宠物状态
  → 互动或接受建议
  → 宠物产生有上下文的反馈
  → 完成任务或形成习惯
  → 解锁关系、动作和个性变化
  → 用户继续自定义和扩展
```

必须同时满足：即时反馈、长期成长、不过度打扰、用户始终可控制。

## 4. 功能域

所有功能 UI 必须遵守 [`UI_DESIGN_SYSTEM.md`](UI_DESIGN_SYSTEM.md)；未覆盖完整状态矩阵、无障碍和视觉回归的功能不视为完成。候选扩展方向见 [`FEATURE_EVOLUTION.md`](FEATURE_EVOLUTION.md)，不因列入储备而自动进入承诺范围。

### 4.1 Pet Runtime

- 透明无边框窗口、置顶、穿透、拖拽、缩放、边缘吸附。
- Windows 与 macOS 多屏、DPI、锁屏恢复、全屏避让。
- 动画图驱动的 idle、walk、sleep、drag、interact 等状态。
- hunger、energy、mood、affinity 及可扩展自定义属性。
- 行为优先级、打断、冷却、时间感知和 Profile 覆盖。
- 托盘、右键菜单、气泡、通知和控制中心。

### 4.2 个性与关系

- 基础人格、长期倾向、近期状态和场景共同影响决策。
- 关系等级、领养日、连续陪伴、成就和纪念事件。
- 记忆分为会话、偏好、事件和用户固定记忆。
- 所有推断记忆可查看、纠正、删除和禁用。
- 人格变化不得静默扩大权限或改变隐私设置。

### 4.3 自定义与资源

- Character Pack：角色身份、骨架/画布、默认动画和属性。
- Skin Pack：在兼容角色基础上替换视觉、音效和局部动作。
- Theme Pack：气泡、控制中心配色、字体、图标和声音主题。
- Behavior Pack：只包含声明式状态图、权重与台词，不执行任意代码。
- Voice Pack：音色、音频素材和 TTS 配置引用。
- Interaction Pack：点击区域、手势语义、粒子和反馈组合。
- 资源继承、组合、预览、热重载、回退和兼容检测。
- 支持 Live2D Cubism、glTF/GLB、VRM 0.x/1.0 及隔离的第三方模型导入；支持换装、附件、动作/表情映射和性能档位。

详细规则见 [`CUSTOMIZATION_ASSETS.md`](CUSTOMIZATION_ASSETS.md) 与 [`MODEL_RENDERING_IMPORT.md`](MODEL_RENDERING_IMPORT.md)。

### 4.4 Command 与 Automation

- 所有可执行能力注册为 Command，并声明参数、风险和可撤销性。
- 自动化模型为 Trigger → Conditions → Actions → Policy。
- 支持分支、并行、等待、重试、超时、补偿、确认和队列。
- 支持表单与 YAML；流程图编辑器在规则模型稳定后提供。
- 提供执行历史、逐步调试、事件回放和虚拟时间。
- 成功的 Agent 任务可保存为自动化模板。
- 用户可使用可视化规则、YAML、本地 TypeScript/JavaScript 脚本和完整 SDK 控制宠物及外部联动。
- 用户代码只能通过统一 Event、Query、Command 与 Capability Broker 执行，详细规则见 [`PROGRAMMABLE_CONTROL.md`](PROGRAMMABLE_CONTROL.md)。

### 4.5 Extension Platform

- Skill、Connector Provider、Agent Pack 和 Asset Pack 使用独立包类型。
- 官方功能遵守公开 SDK，内部特权必须有显式 ADR。
- 扩展支持安装、启停、升级、回滚、迁移和隔离故障。
- Registry 支持 Official、Verified、Community、Private 和 Local 信任级别。
- 不可信代码默认不能直接访问文件、网络、系统命令或密钥。

### 4.6 Open Platform

- Gateway 提供 REST、WebSocket 和 SSE Server，默认仅监听回环地址。
- Sink Connector 提供 HTTP Webhook、WebSocket Client、UDP、MQTT 等投递。
- Source Connector 提供 SSE Client、WebSocket Client、MQTT 等事件导入。
- 所有入口使用配对、Token、Scope、速率限制和审计。
- 所有出站使用目标策略、数据分类、脱敏和用户可见预览。

### 4.7 AI Agent

- 支持 OpenAI-compatible、Anthropic 和本地 Provider。
- Agent 只能调用 Registry 中已授权的工具。
- 执行流程包含计划、风险计算、确认、执行、验证和汇报。
- 可变更状态的工具应该提供 `preview` 和 `undo` 或补偿策略。
- 无 AI 时，命令、自动化和本地规则仍可独立工作。
- 不同 Agent Pack 可以拥有独立人设、工具白名单和记忆策略。

### 4.8 可观测与信任中心

- 展示技能、连接器、Agent、权限和网络活动。
- 支持事件查询、链路追踪、失败重试、审计导出和诊断包。
- 展示 CPU、内存、网络、磁盘、事件速率和 AI Token 预算。
- 安全模式停止 Gateway、外部连接器和第三方代码，但保留核心桌宠。

## 5. Profile 模型

Profile 是可组合策略，而不是配置副本。优先级为：

```text
系统安全策略
  > 用户临时覆盖
  > 当前 Profile
  > 扩展建议
  > 全局默认
```

Profile 可覆盖窗口、音量、主动频率、启用扩展、规则、连接器、角色行为和隐私暴露级别。安全策略不能被 Profile 降级。

## 6. 非功能目标

| 类别 | 发布目标 |
|---|---|
| 空闲性能 | CPU P95 < 3%，常驻内存 P95 < 180 MB |
| 启动 | 已安装资源下冷启动 P95 < 2.5 秒 |
| 稳定 | 24 小时 soak test 无崩溃，内存增长 < 10% |
| 故障隔离 | 单扩展崩溃不终止 Core，30 秒内可禁用 |
| 离线 | 无网络时桌宠、资源、命令和本地自动化可用 |
| 可访问性 | 键盘可操作、减少动画、字幕、对比度和缩放支持 |
| 国际化 | UI、资源文本和商店元数据支持 locale fallback |
| 数据迁移 | 任意稳定版配置可升级，失败可回滚 |

## 7. 产品阶段

| 阶段 | 可发布成果 |
|---|---|
| M0 Runtime | 原生透明宠物、事件、命令、默认角色、托盘 |
| M1 Creator Foundation | Character/Skin/Theme 包、预览器、热重载 |
| M2 Skills | Skill Host、官方番茄钟与提醒、命令面板 |
| M3 Automation | 规则引擎、模板、执行历史和调试器 |
| M4 Open Platform | Gateway、HTTP/UDP、审计、JS Client |
| M5 Ecosystem | 包签名、Registry、更新、回滚、Creator Studio |
| M6 Agent | Tool Calling、确认、Provider、任务历史 |
| M7 Advanced | 更多连接器、多宠、同步、团队和私有源 |

每个阶段必须独立可用，不允许以“未来功能”弥补当前体验缺失。

## 8. 核心成功指标

- 首次启动 60 秒内完成一次有效互动。
- 用户七日内至少启用一个非默认资源或功能扩展。
- 提醒完成率和主动建议关闭率保持平衡。
- 扩展崩溃不影响核心会话的比例达到 99.9%。
- 权限和数据出站页面能够回答“谁在访问什么、发送到哪里”。
产品提供 Companion、Character、Power User、Creator、Developer 五种渐进工作模式；模式改变默认入口与信息密度，不限制底层能力，也不创建割裂版本。
