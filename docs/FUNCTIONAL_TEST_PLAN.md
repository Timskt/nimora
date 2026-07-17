# Nimora 功能测试计划

> 版本：0.1.0-draft  
> 更新日期：2026-07-17  
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
| PET-006 | DPI 切换 | 跨不同 DPI 屏幕拖动 | 尺寸稳定、画面清晰、命中区一致 | P0 |
| PET-007 | 锁屏恢复 | 锁屏、等待、解锁 | 状态合理恢复，无动画时间跳变 | P1 |
| PET-008 | 全屏避让 | 打开全屏应用 | 按 Profile 靠边、隐藏或静默 | P1 |

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
| AGT-029 | 生产 Tool Catalog | CLI 与 Provider 请求获得相同四项模块工具；Descriptor 的 Schema、风险和副作用稳定且不暴露内部对象或任意命令入口 | P0 |
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

## 17. 安装、升级与卸载

| ID | 场景 | 预期结果 | P |
|---|---|---|---:|
| INS-001 | 全新安装 | 默认资源、数据目录和安全设置正确 | P0 |
| INS-002 | 应用升级 | 用户数据、扩展和资源正确迁移 | P0 |
| INS-003 | 升级中断 | 下次启动恢复或回滚，不处于半安装状态 | P0 |
| INS-004 | 版本回滚 | 兼容数据可读取，不兼容时使用只读恢复模式 | P0 |
| INS-005 | 卸载 | 按用户选择保留或删除数据和密钥引用 | P1 |

## 18. 回归策略

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
