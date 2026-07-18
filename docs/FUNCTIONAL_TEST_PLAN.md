# Nimora 功能测试计划

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

当前自动化证据：`nimora-automation-runtime` 单元测试覆盖条件短路、dry-run 零 Backend 调用、幂等重试门禁、瞬时错误三次尝试、失败后的逆序补偿、取消与超时前零执行；桌面 Rust 测试验证 IPC 与 Agent `automation.definition.validate` 工具都只返回 `planned` 步骤且尝试次数为零，Agent 测试同时证明无需确认、无待处理项、无 Runtime 状态变化且普通用户程序不能继承专用 Query；前端测试验证匹配与不匹配事件的离线预览。Interval、分支/并行、持久调度、真实 Action Gateway、运行历史和事件回放仍不得标记通过。

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

- Verify Theme 缺少入口、错误媒体类型、缺失/额外 Token、非法 Hex、未知字段、CSS/URL/脚本注入均在安装前拒绝且不改变活动主题。
- Verify 安装前预览只作用于 Creator Studio 卡片；取消或安装失败后 App Shell Token 保持不变。
- Verify Theme 激活后全局 Token 一致，选择记录原子持久化，重启后重新复验；包缺失或损坏时回退内置主题并显示原因。
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

- Verify Creator Agent 固定使用 `Draft` autonomy、空 Tool allowlist、调用深度 1，且只允许已登记的本地 Provider；模型 Tool Call 必须 fail closed。
- Verify可信 System instruction 与 Personal/Untrusted 用户需求使用独立消息，模型输出永远按不可信输入处理。
- Verify Markdown 包裹、未知字段、类型不匹配、非法 Manifest、权限说明缺失/额外/重复均拒绝，错误不回显模型原文。
- Verify 绝对路径、`..`、反斜杠、重复路径、缺失入口、文件数/单文件/总大小超限均在任何写盘前拒绝。
- Verify User Program、Skill、Automation 分别复用生产校验；Safe/Recovery/浏览器预览禁止生成和保存，已保存结果仍显示“尚未安装”。
- Verify 保存 IPC 重新校验完整需求与草案，选择的 Workspace 必须是可规范化真实目录；`.nimora-drafts` 符号链接、目标已存在和中途写入失败均不得覆盖或留下已发布的半成品。
- Verify User Program 与 Skill 的每个 JavaScript 文件都由对应独立 Worker 的显式 `Validate/Validated` 协议检查；包含顶层 `throw` 的合法语法通过且不执行，非法语法返回逐文件失败。
- Verify Skill 草案检查校验协议版本、Execution ID 和 Manifest，但安装前不要求 Active Skill Lease；正式 `Run` 仍必须精确匹配 Active Lease 与 Activation Event。
- Verify UI 检查报告使用 `nimora.creator-draft-check/1`，逐文件区分 passed/failed；未通过检查时保存按钮不可用，绕过 UI 直接调用保存 IPC 仍失败关闭。
- Verify Automation 草案复用生产 Engine 的确定性校验且不会伪装成 JavaScript Worker 检查；检查、保存、安装和启用状态在 UI 中不可混淆。
