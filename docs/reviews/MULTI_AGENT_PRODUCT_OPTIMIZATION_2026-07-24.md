# Nimora 多 Agent 产品优化总控报告

> 日期：2026-07-24  
> 角色：总控汇总（功能 / 界面 / 易用性 / 需求挖掘 / 需求确认 / 需求把控）  
> 说明：并行子 Agent 遭遇 API 429，本报告由主 Agent 在代码与既有质量评审（Turing ~44% 发版就绪）基础上完成；分轨明细见同目录各 EVAL 文件。

## 1. 项目目标

Nimora（灵栖）是 **Tauri 2 原生桌面生命体**：透明无边框置顶 Overlay + Three.js Q 版小黄人（灵灵）为 **Subject**，驱动 Agent / Goal / AutoMode / Skill / Worker / Connector / 用户代码。本地优先，断网可用；不是浏览器 demo，不是 Electron 弹窗。

## 2. 目标用户

| 角色 | 核心诉求 |
| --- | --- |
| 普通用户 / QQ 宠物情怀 | 可爱、会自己玩、拖得动、会说话、像真的养在桌面 |
| 开发者 | Agent/CLI/Skill/用户代码、能力网关、可审计 |
| 极客 | 可扩展、本地模型、全权限无人值守、插件生态 |
| 二次元 | 自定义模型/皮肤、Live3D/GLB 导入、角色切换 |

## 3. 核心场景

1. 桌面常驻陪伴 + 自主玩耍 / 避让窗口  
2. Goal + Auto Mode 长任务（睡眠安全 NeverAsk / full_device 风险明示）  
3. Agent 工作区 + Provider + 上下文压缩  
4. Skill/Worker 体力表现（冒汗/庆祝/晕倒）  
5. Connector 感官（断网沮丧、文件变化警觉）  
6. 用户代码经 Capability Gateway 驱动宠物  

## 4. 功能清单（状态摘要）

| 域 | 状态 | 缺口 / 动作 |
| --- | --- | --- |
| 原生 Overlay | 部分 | 需真机透明/穿透/拖拽/遮挡签字 |
| Q-minion 3D | 增强中 | 全身帧、无方框、更灵动 idle、情境话术 |
| 命名 | 基本统一 | 默认 **灵灵** / `builtin.nimora`；文档 deskpet→nimora |
| Agent/Goal/Grant | 可用切片 | 空状态可行动文案、RTL 路径测、revoke 竞态 |
| Skill→宠物 | 已接线 | busy/done 已 emit pet_directive |
| Worker/Connector 宠物化 | 映射+Host 部分 | 持续事件路径与 UI 芯片对齐 |
| 性能预算 | 插桩 | 缺真机 idle 证据 |
| Grant 密钥 | 有风险 | keychain 失败不可静默降级确定性密钥 |

## 5. 界面优化（可落地）

- **禁止** Control Center / Overlay 出现正方形裁切框；stage `overflow: visible`、无硬边框  
- 逻辑身体 **260×320**，Ortho 更大 frustum + 略缩 scale，保证靴+影+发完整  
- Q 小黄人：更大护目镜、暖黄胶囊、经典棕虹膜，禁止科技青眼  
- 气泡中文短句，情境（会议/低电/未读）优先于「本地陪伴中」干瘪默认  

## 6. 易用性优化

- 所有锁态/空态必须中文 **下一步**（请先填任务 / 请开桌面端 / 请接 Provider）  
- Auto Mode 风险档位：NeverAsk / full_device 在设置与签发前二次确认  
- Away Summary：loading / error / empty / ready 四态  
- 浏览器预览明确降级，禁止假装有系统感知  

## 7. 需求确认（已确认 vs 待决策）

**已确认（用户历史）**  
- 原生桌面主体，非浏览器  
- 宠物是 Subject，驱动能力  
- Q 版小黄人默认；名字 灵灵 / 产品 Nimora  
- 用户可写代码控制；pnpm only  
- full_device + 睡眠免打扰权限可配置  
- 有价值功能做全，不做「下版本再说」的空头  

**待决策（不阻塞本周 P0）**  
- 商店签名与分发渠道  
- 第三方角色市场审核策略  
- 默认是否开启 full_device 档位入口（建议默认隐藏，设置中解锁）  

## 8. MVP 范围（本阶段）

**必须**  
1. 灵灵全身可见、可拖、无方框、idle 表演 + 情境话术  
2. 命名/契约 `nimora.*` 一致  
3. Agent/Goal/Grant 可走通一条真实任务  
4. Skill/Worker/Connector → pet_directive  
5. 本地优先与安全网关 fail-closed 原则  

**明确不做（本阶段）**  
- 公开应用商店上架  
- 完整插件市场 UI  
- 浏览器版系统感知  

## 9. 迭代计划

| 波次 | 主题 | 出口 |
| --- | --- | --- |
| W0（当前） | 宠物观感+命名+情境智能+评估文档 | 可 dogfood 原生截图 |
| W1 | Grant fail-closed + Agent RTL + 空态打磨 | 内部 alpha |
| W2 | 多屏遮挡真机门禁 + 性能报告 | 可控 beta |
| W3 | 扩展生态样本 + 模型导入打磨 | 公开预览 |

## 10. 开发优先级

1. P0 宠物可见/可爱/可拖/会说情境话  
2. P0 安全 Grant 密钥  
3. P0 功能可走通（Agent/Skill）  
4. P1 原生 QA 证据  
5. P1 宿主 lib.rs 拆分  
6. P2 生态与商店  

## 11. 验收标准（摘要）

- 默认名 UI 仅 **灵灵**；无用户可见 Aster/DeskPet  
- Overlay 透明无框；全身+影；拖动与穿透可用  
- 会议/低电/未读产生可区分气泡  
- Agent 锁态有中文 next-step  
- `pnpm` 测试门禁绿；CI 不因文档 push 刷量  

## 12. 风险

- 429 导致并行评审不稳定 → 串行/主 Agent 兜底  
- 19k 行 lib.rs 变更风险  
- 真机未证先宣称完成 = 发布事故  
- 确定性 Grant 密钥 = 安全事故  

---

**结论：不标记 goal complete。** 继续 W0 交付与原生视觉证据。
