# Nimora 角色模型、渲染与导入规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-17

## 1. 目标与边界

Nimora 将静态序列帧、2D 骨骼模型和实时 3D 角色视为同等的一等角色载体。“Live3D”统一指可实时驱动的 3D 角色能力，首选开放的 VRM/glTF 生态，不定义私有 Live3D 文件格式。

模型包默认是数据，不执行代码。模型导入、动作语义和渲染实现相互隔离；第三方格式必须经 Importer 转换或规范化，再由 Renderer Adapter 加载，不能绕过资源校验、性能预算和许可证检查。

## 2. 用户与工作模式

| 模式 | 默认界面 | 核心能力 |
|---|---|---|
| Companion | 极简 | 一键安装角色、自动兼容与安全回退 |
| Character | 角色优先 | Live2D/VRM、换装、表情、语音与收藏 |
| Power User | 高密度 | 参数覆盖、动作映射、性能档位和诊断 |
| Creator | 工作台 | 导入、绑定、时间轴、热重载、校验与发布 |
| Developer | API 优先 | Renderer/Importer SDK、模拟器、CLI 和契约测试 |

模式只改变信息密度和默认入口，不建立互不兼容的产品分支。用户可以逐项开启高级能力。

## 3. 格式支持矩阵

| 格式 | 支持级别 | 导入策略 | 备注 |
|---|---|---|---|
| PNG/WebP 序列帧、Atlas | 核心 | 原生 | 基础回退格式 |
| Lottie | 扩展 | Renderer Adapter | 限制表达式与外部资源 |
| Spine | 扩展 | Renderer Adapter | 用户需满足运行时许可证 |
| Live2D Cubism | 扩展 | Renderer Adapter | SDK/runtime 不随意再分发，遵守 Cubism 条款 |
| glTF 2.0/GLB | 核心 3D | 原生或规范化 | 禁止导入时执行脚本与任意 URI |
| VRM 0.x/1.0 | 核心 3D | 规范化为统一角色模型 | 支持 humanoid、expression、Spring Bone、MToon |
| OBJ/FBX | 创作者导入 | 隔离转换为 glTF/GLB | 不作为运行时分发格式 |

所有格式能力必须声明平台、GPU 后端、最大格式版本和降级路径。缺失专有运行时时，产品展示安装说明或回退角色，不静默下载受限组件。

## 4. 统一角色模型

规范化后的 Character Model 至少包含：

- 身份、作者、来源、许可证、内容哈希和格式版本。
- 2D 画布或 3D 坐标、缩放、锚点、朝向和安全边界。
- 标准动作：`idle`、`walk`、`drag`、`drop`、`sleep`、`work`、`celebrate`、`error`。
- 标准表达：`neutral`、`happy`、`sad`、`angry`、`surprised`、`focused`、`sleepy`。
- 目光、眨眼、口型、呼吸、物理系统和动作混合能力声明。
- 命中区、附件插槽、换装层、音效和可访问性替代信息。
- 多边形、纹理、骨骼、Draw Call、内存和帧率预算。

模型不必实现所有语义。缺失映射按 `模型自定义 → 同类表达 → neutral/idle → 默认角色` 回退，并在 Creator Studio 中形成可发布告警。

## 5. 动作与实时驱动

- Pet Runtime 只发送语义动作，不依赖 Live2D 参数名、VRM 骨骼名或具体 Clip。
- Renderer Adapter 负责动作混合、过渡、安全打断和完成事件。
- Lip Sync 接受归一化音素/viseme 与音量包络；无精确口型时回退开合嘴。
- Gaze 接受屏幕目标和强度，必须限速，避免眩晕与监视感。
- VRM 支持 humanoid retarget、expression、look-at、Spring Bone 和 MToon；不支持项必须显式报告。
- Live2D 参数映射保存在角色清单中，不把厂商内部参数泄漏为平台公共契约。
- 外部摄像头、麦克风、面捕或动捕是独立 Capability，默认关闭并持续显示使用状态。

## 6. Renderer Adapter 契约

Adapter 必须实现版本化接口：`probe`、`load`、`instantiate`、`play`、`setExpression`、`setGaze`、`setLipSync`、`attach`、`snapshot`、`dispose`。

每个调用必须可取消、可超时并返回结构化错误。Adapter 必须：

- 在独立渲染边界内运行，不能直接读取文件系统、网络或密钥。
- 声明支持的格式、特性、GPU 能力和资源上限。
- 处理 WebView/GPU context 丢失并可重建实例。
- 严格释放纹理、缓冲区、音频与事件订阅。
- 支持同一语义测试夹具，确保不同格式行为一致。

## 7. Importer 契约与安全流水线

```text
选择来源 → 复制到隔离暂存区 → 类型探测 → 解包限额
→ 路径与 URI 校验 → 隔离解析/转换 → 内容与许可证扫描
→ 规范化 → 性能分析 → 用户预览 → 原子安装 | 回滚
```

- 不信任扩展名和 MIME；使用魔数、结构和 Schema 联合探测。
- 防止 Zip Bomb、路径穿越、符号链接逃逸、超大纹理、畸形网格、递归引用和解析器 DoS。
- glTF 外部 URI 仅可引用包内文件；禁止 `file:`、远程 URL、data URI 超限和脚本内容。
- 转换器在低权限独立进程中运行，限制 CPU、内存、文件数、输出大小和时间。
- 安装使用内容寻址暂存与原子切换；失败保留可脱敏诊断，不留下半安装状态。
- 来源 URL、作者、许可证和修改记录随派生包保留；无法确认授权时仅允许本地私用并显示警告。

## 8. 换装、附件与组合

- Outfit、Hair、Accessory、Prop 使用声明式插槽和兼容约束。
- 3D 附件绑定标准 humanoid 骨骼或显式 socket；2D 附件绑定画布层、参数或追踪点。
- 组合前检测穿模风险、骨骼缺失、许可证冲突和性能预算。
- 用户覆盖保存在独立配置层，不修改原始包，升级后可重新应用和回滚。
- 允许创作者发布适配补丁，但必须声明目标模型哈希范围。

## 9. 性能档位

| 档位 | 目标 |
|---|---|
| Eco | 低帧率、低纹理、暂停非必要物理与后台动画 |
| Balanced | 默认画质与自适应帧率 |
| Quality | 高分辨率纹理、完整物理和高级后处理 |
| Capture | 稳定帧率、透明输出、录制/直播优化 |

Runtime 根据电池、温度、前后台、掉帧和多宠数量动态降级。降级只能减少视觉成本，不能改变权限提示、任务状态或可恢复路径。

## 10. Creator Studio 与 CLI

Creator Studio 提供导入向导、骨骼/参数映射、动作与表达预览、换装插槽、性能分析、许可证表单、跨平台模拟和发布检查。所有功能必须有可自动化的 CLI 等价能力。

```bash
pnpm deskpet model inspect ./character.vrm
pnpm deskpet model convert ./source.fbx --to glb
pnpm deskpet model validate ./character-pack
pnpm deskpet model preview ./character-pack --profile balanced
pnpm deskpet pack build ./character-pack
```

仓库和官方模板仅使用 `pnpm`，不提供 `npm`、Yarn 或 Bun 版本命令。

## 11. 验收标准

- 同一标准动作可在序列帧、Live2D、glTF 和 VRM 上得到可接受映射或明确回退。
- 恶意、损坏或超预算模型不能终止 Core，也不能污染已安装资源。
- 模型热切换、context 丢失和 Adapter 崩溃后可恢复默认角色。
- 连续切换与卸载模型不存在持续纹理、内存、监听器或临时文件增长。
- 离线状态可加载、切换、编辑和预览全部本地模型。
- 发布检查覆盖格式、性能、许可证、安全、动作完整性和可访问性。

## 12. 当前实现状态

当前仓库已实现第一个真实 Importer Worker 及桌面检查入口，而不是多格式占位 Adapter：

- `nimora-model-importer-worker` 只接受暂存目录根部的相对 GLB 文件和 `nimora.model-probe/1` 请求。
- GLB 2.0 探测验证魔数、容器长度、JSON/BIN chunk 顺序和边界、`asset.version`、外部 URI 以及节点、网格、材质和纹理预算。
- 宿主 Supervisor 清空 Worker 环境、关闭 stdin、固定工作目录，限制 80 MiB 输入、1 MiB JSON、64 KiB 输出并在截止时间后强制终止。
- 真实进程测试覆盖合法模型、远程 URI、暂存区逃逸、Worker 超时和 Worker 崩溃；这些失败不会进入 Core。
- Creator Studio 的 Model Lab 只允许 Control Center 在非安全模式通过系统对话框选择绝对普通 `.glb` 文件。宿主拒绝符号链接和超限文件，复制为一次性缓存目录内固定的 `character.glb`，同步写入后才调用 Worker，并通过作用域清理保证成功、拒绝、崩溃和超时均回收暂存目录。
- 检查完全本地离线运行，不上传原文件；WebView 只收到格式、分区大小和资源计数，不收到源路径或暂存路径。桌面发布构建通过 `pnpm build:sidecars` 同时打包用户代码 Worker 与模型 Importer Worker。
- Creator Studio 可在结构检查后填写本地包 ID、显示名和许可证，并将 Worker 返回的有界命名动画显式映射到 `pet.idle`、`pet.walk`、`pet.click`、`pet.drag`、`pet.sleep` 与 `pet.work`。名称匹配只提供可编辑建议；有命名动画时必须映射 `pet.idle`，无名动画不会被静默猜测。确认安装时宿主重新复制源文件并对同一暂存 GLB 再次运行 Worker，随后生成 `nimora.asset/1` Character 包：模型固定为 `models/character.glb`，映射写入 `nimora.animation-map/1` 和 `entrypoints.animationGraph`，Integrity 清单覆盖 Manifest、模型与映射字节，最后复用正式 Asset Installer 的精确目录树、哈希复验、备份和原子激活。生成 ID 仅允许 `character.local.*`，避免覆盖第三方发布者命名空间。

当前规范化封装已经通过探测的原始 GLB 和用户确认的动作映射，不进行网格重写、纹理转码或许可证扫描；用户填写的许可证只是包元数据，不代表平台完成权利认证。`nimora.animation-map/1` 将平台动作绑定到精确动画名和循环语义，要求 `pet.idle`，限制 64 项并拒绝空白、超长或控制字符名称；宿主还会把每个绑定与安装时最新 Worker 报告中的动画名逐项比对，拒绝绕过 UI 提交的空映射或不存在片段。Pet Overlay 通过 `nimora.renderer/1` 获取验证后的映射，在模型只加载一次的前提下按状态切换 AnimationAction，以 180 ms cross-fade 过渡；一次性动作使用 `LoopOnce` 并停在末帧，循环动作使用 `LoopRepeat`，缺失动作沿 Manifest fallback 回到 `pet.idle`，映射到模型不存在的动画时拒绝播放而非猜测。减少动画偏好下 Mixer 保持暂停。切换或卸载时停止 Mixer、取消帧循环、断开 ResizeObserver 并释放几何体、材质、纹理和 WebGL context，context 丢失则显式回退内置角色。

上述能力是 GLB 2.0 Renderer，不是 VRM/Live2D 支持，也不是标准动作语义映射完成。inventory 与容器验证不等于发布者签名认证，许可证字段不等于权利扫描，Three.js WebGLRenderer 也不等于独立 Renderer 进程或 OS/GPU 沙箱。Importer Worker 的进程边界只能隔离崩溃、超时和协议输出，不能证明其无法访问其它 OS 资源。GLTF JSON、VRM、Live2D 和其它格式仍未实现，不能据此宣称格式矩阵全部可用。
