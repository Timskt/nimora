# Nimora 全量实现状态与证据矩阵

> 更新日期：2026-07-18
> 适用阶段：首个稳定版之前  
> 原则：优先级只决定施工顺序，不缩减产品范围；“已完成”必须同时具备真实实现、失败边界、自动化证据和同步文档。

## 状态定义

| 状态 | 判定标准 |
|---|---|
| 已验证 | 真实实现、正常与异常测试、文档和适用运行证据齐全 |
| 部分实现 | 已有可运行纵切，但规格中仍有明确能力或平台证据缺口 |
| 未实现 | 只有规格、契约占位或尚无可运行代码 |
| 外部验证受阻 | 实现可继续，但当前环境缺少设备、签名身份或可连接的浏览器/平台实例 |

## 全量能力矩阵

| 领域 | 当前状态 | 已有证据 | 必须继续闭环的缺口 |
|---|---|---|---|
| 桌宠窗口与交互 | 部分实现 | Tauri 透明双窗口、拖拽、置顶、穿透、托盘、安全模式、Click/Drag FSM 与 Rust 测试 | Windows/macOS 真机冒烟、多屏/DPI、WebView 崩溃恢复、长稳验证 |
| UI 与设计系统 | 部分实现 | Control Center、Creator Studio、Overlay、Token 与组件样式、前端单测和构建 | 浏览器真实截图、键盘/读屏/200% 缩放、关键状态视觉回归、跨平台像素审查 |
| Profile 与离线状态 | 部分实现 | 唯一 SQLite Schema、离线 Profile、Online Backup 调度与原子恢复、损坏数据库隔离启动、统一写门禁、恢复 UI、脱敏诊断摘要导出、Rust/TS 故障测试与 Chrome 实测 | 休眠与时钟异常、恢复模式真实桌面截图、跨平台真机故障注入、人工数据提取与跨设备备份 |
| Event 与持久 Outbox | 部分实现 | 事务写入、租约、ACK、重试、死信、清理、健康状态和自动化测试 | 具体幂等消费者、跨重启投递恢复、Connector 投递审计 |
| Sprite 角色与皮肤 | 部分实现 | 严格包契约、安全导入导出、序列/图集真实渲染、动作 fallback | 独立预览实例、命中区编辑、连续切换泄漏与性能门禁 |
| glTF/GLB 角色 | 部分实现 | 独立 Worker 探测、命名动画报告、可编辑标准动作映射、原子安装、受控协议、Three.js 真渲染、cross-fade、framing、释放与失败回退 | 独立预览、持续切换与 GPU 压测、真实截图和跨平台验证 |
| VRM 与 Live2D | 未实现 | Manifest 可识别并显式安全回退 | 独立 Adapter、格式验证、动作/表情映射、许可证策略、隔离与资源释放测试 |
| 主题、声音与行为包 | 未实现 | 规格和统一 Asset Manifest 基础 | 严格子契约、编辑预览、原子安装切换、权限和回退实现 |
| 用户代码执行 | 部分实现 | 独立 JS Worker、预算、强制取消、安装完整性、版本精确授权、Capability Gateway | 授权型文件/网络/自动化后端、调试器、录制回放、跨平台 sidecar 发布验证、OS 资源硬限制 |
| 事件驱动程序 | 部分实现 | 可信 Rust 订阅、`serial`/`drop`/`cancel-previous` Supervisor、迟到完成隔离 | 桌面纯 Supervisor 集成测试、执行历史与 Creator Studio 可视诊断 |
| 扩展与 Skill 生态 | 未实现 | 规格、Manifest/Gateway 设计及用户代码基础设施 | Extension Host、贡献点、安装停用卸载、崩溃隔离、官方番茄钟与提醒 |
| 自动化引擎 | 未实现 | Command/Event、结构化用户程序计划基础 | Trigger/Condition/Action/Policy、事务执行历史、重试补偿、测试运行和事件回放 |
| 开放 Gateway 与 Connector | 未实现 | Capability Gateway 为进程内用户代码提供窄能力边界 | 配对、Token/Scope、REST/WS/SSE、HTTP/UDP Sink、Source Connector、2 秒安全停机 |
| AI Agent 与 CLI | 部分实现 | Tool Registry、风险与批准、任务硬预算；Provider/Tool 单步协调；Provider 续跑的 Assistant Call 与 Tool Result 强关联协议；多调用 Turn 完整聚合器；Ollama Worker 结构化续跑载荷及真实独立 Worker 双轮 Tool Call 自动化；十项生产模块工具、角色渲染能力脱敏摘要、Runtime 动作词汇发现、原生策略事务化 Profile 切换、资产复验与刷新回滚型角色切换、可扩展只读能力集合及共享 Capability Gateway 固定映射；完成任务历史的版本化 SQLite 仓储、稳定游标分页和隐私删除；桌面完成态写入、降级不篡改结果、Recovery 内存隔离、分页/删除 IPC、最近历史 UI 与 Preview 会话实现；CLI 完成态旁路写入、显式数据库导出、成对游标分页和单条/全部删除；统一桌面 Agent 结果契约；Provider 等待确认、多确认项展示、部分批准后剩余项回填、最后批准后 Provider 续跑与整组拒绝 UI；构建期嵌入 Ollama Manifest 信任摘要，桌面启动时复验 Manifest 与 Worker 后自动注册；受隔离 Worker 的 `/api/tags` 健康探测、严格模型目录预算、去重排序、真实 Provider 状态与模型选择；桌面宿主对写调用整组批准前零副作用，拒绝/过期级联撤销，全部批准后按原始顺序执行；Safe/Recovery Mode fail-closed；CLI 工具发现与 Ollama 接线；独立 sidecar、loopback-only SSRF、Worker 完整性发现、有界协议、超时取消强杀和跨进程 mock 测试 | 用户本机实际 Ollama 模型桌面验收、继续扩展生产工具覆盖面、OpenAI-compatible Adapter、发布者数字签名、持久计划恢复、完整 Prompt Injection 防护 |
| 包签名与 Registry | 未实现 | SHA-256 本地完整性锁和原子回滚 | 发布者签名、信任根、撤销、Registry、兼容检测、更新策略、离线 CLI 验证 |
| 安全与隐私 | 部分实现 | 安全模式、精确授权、路径/符号链接防护、Worker 隔离、审计边界文档 | OS 沙箱、系统密钥存储、网络目标策略、隐私面板、威胁模型自动门禁 |
| 稳定性与可观测性 | 部分实现 | 结构化错误、Trace、Outbox 健康、故障回退、版本化脱敏诊断摘要包、14 天/1 MiB/64 段有界固定事件日志、跨重启恢复、损坏尾行跳过、存储失败内存降级、用户可取消事件导出及单元/集成测试 | 自由文本日志与筛选、用户选择 Trace、指标、崩溃循环恢复、资源预算监控、soak/chaos 和 SLO 门禁 |
| 部署与发布 | 部分实现 | pnpm/Rust 构建、Tauri 配置、CI 规范 | Windows/macOS 签名安装包、公证、更新回滚、SBOM、许可证、发布演练 |
| Git 与工程治理 | 部分实现 | Git 规范、Conventional Commits、pnpm-only 门禁、全量本地质量命令 | 受保护分支/PR 实际仓库配置、跨平台 CI、依赖与安全扫描证据 |

## 执行规则

1. 每次开发从本矩阵选择一个或多个可形成真实纵切的缺口，但不得删除其余缺口。
2. 新发现的有价值能力必须加入矩阵或对应权威规格，不依赖聊天记忆。
3. 只有证据满足状态定义时才能改为“已验证”；单元测试不能替代真实 UI、平台或安全边界验证。
4. 首个稳定版前直接维护唯一当前契约，不为尚未发布的设计制造迁移链或兼容包袱。
5. 每个切片完成后更新矩阵、功能测试计划、相关专题文档，并提交推送主仓库。
