# Nimora 自定义资源与皮肤规范

> 契约：`nimora.asset/1`
> 版本：0.1.0-draft  
> 更新日期：2026-07-17

## 1. 设计目标

自定义系统必须同时服务普通用户、画师、动画师和开发者。资源包默认不执行代码，可以安全预览、热切换、组合、继承、回滚和发布。

## 2. 包类型

| 类型 | 作用 | 可依赖 |
|---|---|---|
| Character | 完整角色身份、画布、动作和默认属性 | Theme、Voice |
| Skin | 替换兼容 Character 的视觉与局部动作 | Character |
| Theme | 气泡、控制中心、字体、图标和颜色 | 无 |
| Behavior | 声明式状态图、权重、台词和时间策略 | Character |
| Voice | 音频素材、音色元数据和 TTS 引用 | Character |
| Interaction | 命中区、手势、粒子、反馈组合 | Character/Skin |
| Bundle | 锁定多个包版本的一键组合 | 上述任意包 |

## 3. 标准目录

```text
character.example.mochi/
  manifest.json
  preview/
    thumbnail.webp
    poster.webp
  sprites/
    idle.webp
    walk.webp
  atlases/
    main.json
    main.webp
  animations/
    graph.json
    clips.json
  interactions/
    hitboxes.json
    gestures.json
  behaviors/
    defaults.json
    dialogue.zh-CN.json
  audio/
    click.ogg
  locales/
    zh-CN.json
    en.json
  licenses/
    NOTICE.md
```

包名使用反向域名或可验证命名空间。文件路径必须相对包根目录，禁止 `..`、绝对路径和符号链接逃逸。

## 4. Manifest

```json
{
  "spec": "nimora.asset/1",
  "id": "character.example.mochi",
  "type": "character",
  "version": "1.0.0",
  "name": { "zh-CN": "糯米", "en": "Mochi" },
  "publisher": "publisher.example",
  "license": "LicenseRef-Commercial",
  "engines": { "nimora": ">=0.1.0 <1.0.0" },
  "render": {
    "backend": "sprite-atlas",
    "canvas": { "width": 512, "height": 512 },
    "anchor": { "x": 0.5, "y": 1.0 },
    "defaultScale": 0.5,
    "pixelArt": false
  },
  "entrypoints": {
    "animationGraph": "animations/graph.json",
    "clips": "animations/clips.json",
    "hitboxes": "interactions/hitboxes.json",
    "previewPoster": "preview/poster.webp"
  },
  "capabilities": ["pet.walk", "pet.drag", "pet.sleep"],
  "fallbacks": { "pet.happy": "pet.idle" },
  "locales": ["zh-CN", "en"],
  "integrity": { "algorithm": "sha256", "files": "integrity.json" }
}
```

`name`、描述和商店标签必须可本地化。许可证、作者和第三方素材声明为发布必填项。

安装器同时生成版本化文件清单：每个文件记录相对路径、SHA-256、字节数和媒体类型；路径不得重复，`totalBytes` 必须等于清单求和。清单必须覆盖 `manifest.json`，但不得记录自身，否则会形成无法构造的自引用哈希。Manifest 声明的完整性文件必须实际存在并由宿主直接读取。依赖项必须声明包 ID、版本约束和是否可选，解析器在进入预览前拒绝重复文件、哈希格式错误、路径逃逸和总大小不一致。

共享包 `@nimora/asset-kit` 的 `createImportPlan` 只接收已经隔离的文件清单，执行无副作用的 Schema 校验并返回安装计划；它不会解包、复制或激活文件。Rust 宿主在专用临时目录完成限额解包、链接与路径检查、哈希计算和资源预算限制，最终通过原子目录切换安装。未来独立 Importer 进程仍复用该资产契约，不改变 WebView 无权解包和激活文件的边界。

Rust 侧 `nimora-asset-installer` 同时承担受限容器展开、完整包验证和原子安装边界：它只复制权威清单中的相对文件，拒绝绝对路径、父目录、缺失文件及任何额外文件；已有版本先移动为备份，暂存目录切换失败时恢复旧版本。签名信任链尚未实现，因此当前 UI 只能报告 Manifest 发布者和许可证，不能展示“已验证发布者”。

桌面端通过类型化 `install_asset` IPC 命令调用安装器，WebView 只提交系统文件选择器返回的绝对来源路径，不提交 Asset ID、文件清单或安装目标，也不提供自由文本路径输入。Rust 宿主读取 `manifest.json` 获取身份，从 Manifest 指定的完整性文件读取权威清单，并从应用数据目录和合法 namespaced Asset ID 推导目标目录。每个文件在复制前必须匹配清单中的字节数和小写 SHA-256；单包最多 10,000 个条目、展开文件总量不超过 512 MiB、单条目压缩倍率不超过 200 倍。安全模式拒绝预览和安装。

Creator Studio 已接入 Tauri 官方文件选择器，只显示 `.nimora`，且权限只授予 `control-center` 窗口。`preview_asset` 使用与安装相同的 Rust 权威校验器，返回身份、类型、版本、发布者、许可证、渲染后端、文件数和总大小；用户明确确认后，`install_asset` 重新打开归档、限额展开并完整复验，再执行原子安装，因此预览后替换源文件不能绕过校验。安装与回滚回执只返回包身份和操作结果，不向 WebView 暴露内容仓库路径。当前完成的是安全元数据预览与确认安装，尚未从未安装包解码海报或模型，不能称为视觉模型预览。

当前实现完成的是**宿主权威 `.nimora` 包导入**，同时保留展开目录作为开发测试入口。`.nimora` 是单层 ZIP 容器：Rust 安装边界限制条目数、单包展开总量和压缩倍率，拒绝路径逃逸、反斜杠歧义、重复文件、符号链接、特殊文件和嵌套归档，并在宿主临时目录展开后复用同一 Manifest、Integrity、资源预算与原子安装验证。预览和安装分别重新打开、展开和完整复验源文件，失败不会替换活动资源。独立低权限 Importer 进程及针对复杂 3D 格式的解析预算隔离仍是后续门禁，当前不得把归档解压等同于模型解析沙箱。

Creator Studio 也支持从展开目录导出 `.nimora`。宿主在写出前完整复验 Manifest、Integrity、精确目录树和全部哈希，只打包权威清单及 Integrity 文件；条目按相对路径排序，使用固定时间戳、固定权限和 Stored 压缩方法，因此相同输入产生字节一致的归档。输出先写入同目录临时文件并同步，再原子替换用户通过系统保存对话框选择的目标；源目录无效、目标位于源目录内或打包失败时不修改既有目标。该功能是本地可重复打包，不代表签名、发布者认证或 Registry 发布已经完成。

`asset_catalog` 每次读取都重新验证 Manifest、完整性清单、全部文件哈希和精确目录树。额外文件、符号链接、身份与安装目录不一致或未知渲染后端的包进入 `rejected` 诊断集合，不影响其它资源和默认角色；Creator Studio 分开展示可用资源与拒绝原因。Catalog 只返回本地化名称、类型、版本和渲染后端等最小元数据，不向 WebView 暴露资源目录路径。

安装器在切换前重新校验暂存目录，避免源文件在首次校验后被修改。每次替换都会保留带时间戳的上一版本；`rollback_asset` 只接受合法 Asset ID，恢复最新备份并将当前故障版本移动到 `failed` 隔离目录，避免回滚过程再次覆盖证据。

## 5. 渲染后端

第一阶段必须支持 `sprite-sequence` 和 `sprite-atlas`。版本化 Renderer Adapter 进一步支持 Lottie、Spine、Live2D Cubism、glTF/GLB 与 VRM；OBJ/FBX 仅作为隔离转换输入。任何格式不得改变 Pet Runtime 的动作语义，完整规则见 [`MODEL_RENDERING_IMPORT.md`](MODEL_RENDERING_IMPORT.md)。

资源包声明渲染能力，Core 只使用标准动作：

```text
pet.idle pet.walk pet.sleep pet.drag pet.drop
pet.click pet.happy pet.sad pet.hungry pet.talk
```

自定义动作使用 `<publisher>.<action>`。缺失标准动作时按 `fallbacks` 回退，最终回退 `pet.idle`。

## 6. 动画图

Sprite 资源的 `entrypoints.clips` 必须指向 `nimora.sprite-clips/1` JSON。`live2d`、`vrm` 与 `gltf` 后端则必须声明包内 `entrypoints.model`，且 Sprite 后端不得夹带模型入口、模型后端不得夹带 Clips 入口。当前 Creator Studio 生成的 GLB Character 固定使用 `models/character.glb`；宿主从暂存文件生成该路径和完整性清单，WebView 不能提交目标文件路径或 inventory。

Sprite Clips 文档使用 `backend` 判别联合，首版只接受：

- `sprite-sequence`：每个动作声明 `loop` 与 1–1000 个 `{ file, durationMs }` 帧；文件必须是包内安全相对路径。
- `sprite-atlas`：文档声明单一 `image`，每个动作声明 `loop` 与 1–1000 个 `{ x, y, width, height, durationMs }` 帧。
- 所有文档必须包含 `pet.idle`；帧时长为 16–60,000 ms，尺寸与坐标有硬上限，动作名必须是 namespaced ID。
- Renderer 不接受脚本、远程 URL、CSS 或任意 HTML。共享 `@nimora/schemas` 的 `spriteClipsSchema` 是 Creator、Importer 和 Renderer 的统一契约。
- 宿主 Rust 安装器对同一契约进行独立严格解析：Sprite Manifest 必须声明 `entrypoints.clips`，Clips 后端必须与 Manifest 一致，所有 JSON 与图片引用必须存在于已验证 inventory，图片仅接受受控 MIME。`active_character_renderer` 返回不含安装目录的 `nimora.renderer/1` 描述符；安全模式、损坏包、非 Character 或尚未实现的 Live2D/VRM/glTF Adapter 都明确回退内置角色。
- `nimora-asset://localhost/<asset-id>/<relative-path>` 是专用只读图片协议（Windows 映射为 `http://nimora-asset.localhost/...`）。协议只接受 Pet WebView 的 `GET`，只读取当前活动第三方角色、每次复验完整包、inventory、hash、路径、MIME 与扩展名，并返回 `nosniff` 与 `no-store`；安全模式、非活动包、查询参数、编码穿越和其它窗口全部拒绝。
- 当前 Pet Overlay 已消费可信 Renderer Descriptor：Sequence 使用受控 `<img>` 逐帧切换，Atlas 使用 Canvas 裁切绘制；动作缺失按 Manifest fallback 链解析并检测循环，最终回退 `pet.idle`。资源 URL 对每个路径段编码，图片加载、解码上下文或 Atlas 实际尺寸越界失败时立即显示内置 Aster。`prefers-reduced-motion` 下固定首帧，角色激活与安全模式切换通过定向 Tauri Event 刷新描述符，监听器、Timer 和 Image handler 在卸载或切换时清理。
- Live2D、VRM 与 glTF/GLB 当前仅完成 Catalog 契约识别和显式回退，尚无可执行 Renderer Adapter，不得宣称这些模型已经真实渲染。

Sequence 示例：

```json
{
  "spec": "nimora.sprite-clips/1",
  "backend": "sprite-sequence",
  "clips": {
    "pet.idle": {
      "loop": true,
      "frames": [
        { "file": "sprites/idle/0001.webp", "durationMs": 100 }
      ]
    }
  }
}
```

动画图是声明式有限状态图，包含：

- 状态、Clip、循环模式和播放速度。
- 进入与退出条件。
- 优先级、打断等级、最短持续时间和冷却。
- 参数：方向、速度、情绪、是否拖拽、当前 Profile。
- 事件标记：脚步、音效、粒子和气泡触发点。

动画资源不得直接调用命令、网络或系统 API。需要业务行为时发布语义事件，由 Core 策略决定是否执行。

## 7. 命中区与交互

- 命中区支持矩形、多边形和逐帧引用。
- 区域使用语义名称：`body`、`head`、`hand`、`tail`。
- 手势绑定到语义 Interaction，不直接绑定任意脚本。
- 必须提供最小可交互区域，透明像素不能阻塞整个桌面。
- Creator Studio 必须提供命中区可视化和点击穿透测试。

## 8. 继承与组合

资源解析顺序为：

```text
Character 基础
  → Skin 覆盖
  → Behavior 覆盖
  → Interaction 覆盖
  → Voice 覆盖
  → Profile 临时覆盖
  → 用户无障碍设置
```

- 覆盖以逻辑资源 ID 为单位，不按文件路径猜测。
- Bundle 使用 lockfile 固定依赖版本。
- 不兼容覆盖必须在安装或预览阶段被拒绝。
- 安全和无障碍设置拥有最高优先级，资源包不能覆盖。

## 9. 性能预算

| 项目 | 推荐 | 硬限制 |
|---|---|---|
| 单张纹理边长 | ≤ 2048 px | 4096 px |
| 常驻纹理内存 | ≤ 64 MB | 128 MB |
| 默认角色包 | ≤ 25 MB | 80 MB |
| 单 Clip 帧数 | ≤ 120 | 300 |
| 音频单文件 | ≤ 2 MB | 10 MB |
| 默认动画帧率 | 24/30 FPS | 60 FPS |

运行时根据设备预算降采样、卸载非活跃纹理和暂停不可见动画。像素风资源使用 nearest filtering，普通插画默认 linear filtering。

## 10. 可访问性

资源包必须声明闪烁、快速运动、声音和透明度特性。平台提供：

- 减少动画模式。
- 禁用闪烁和屏幕震动。
- 气泡字幕和音效字幕。
- 高对比度主题。
- 最小/最大缩放与大点击区域。
- 不依赖颜色表达唯一含义。

## 11. 安装、预览和回退

1. 解包到隔离临时目录。
2. 校验路径、Schema、hash、签名、许可证和资源预算。
3. 创建静态预览，不激活行为。
4. 用户确认后原子移动到内容仓库。
5. 在测试实例加载并运行健康检查。
6. 原子切换当前资源；失败时恢复上一版本。

热重载只用于开发者模式。正式包切换必须保持当前 Pet 状态并映射到兼容动作。

当前步骤 1、2、4 的宿主验证与原子安装已经实现。步骤 3 已实现受完整性保护的静态海报预览：`entrypoints.previewPoster` 必须指向 inventory 内的 PNG 或 WebP，宿主限制为 2 MiB、最长边 4096 px，仅向 Control Center 返回验证后的图片字节，不暴露临时目录；取消、换包或卸载界面时立即释放浏览器 Blob URL。它不执行资产行为，也不等同于 Live2D、VRM、glTF 或 Sprite 动画的隔离实例预览。步骤 5 的独立测试实例仍待实现。

## 12. 活动角色选择

- 活动角色是宿主拥有的独立状态，只持久化版本化契约和 Asset ID，不保存也不向 WebView 返回文件系统路径。
- 激活第三方角色前，宿主重新验证 Manifest、完整性清单、精确目录树、身份和 `type=character`；Renderer 不能直接修改选择记录。
- 选择记录采用临时文件与原子替换写入，同一进程内的激活请求串行执行；验证或持久化失败时保持原选择。
- 每次读取活动角色都重新检查已安装包。包缺失、损坏、身份不一致、记录损坏或进入安全模式时，立即使用内置 `builtin.aster`，并返回可展示的回退原因。
- 当前激活会为通过完整性复验的 Sprite Sequence/Atlas 或 GLB 2.0 创建 Renderer Descriptor，并由 Pet Overlay 真实绘制。GLB 只能经 Pet 专用资源协议读取唯一 `entrypoints.model`，不暴露文件系统路径；Three.js Adapter 自动 framing、播放首动画回退并在卸载时释放 GPU 资源。选择记录本身仍不等于渲染成功，任何协议、图片、Canvas、模型加载或 WebGL context 失败都会显式回退内置角色。VRM、Live2D 与标准动作映射必须等对应 Adapter 完成后才能标记为“已渲染”。

## 13. 创作者工具

Creator Studio 应提供：

- 包模板和 Manifest 表单。
- Sprite Sheet/Atlas 导入与切分。
- 动画图编辑和状态模拟。
- 命中区编辑、粒子和音频时间轴。
- 多 DPI、多缩放、浅色/深色和多屏预览。
- 性能预算、缺失动作、许可证和本地化检查。
- 一键生成预览、打包、签名和本地安装。

CLI 对应提供：

```bash
nimora asset create
nimora asset validate ./character.example.mochi
nimora asset preview ./character.example.mochi
nimora asset pack ./character.example.mochi
nimora asset sign character.example.mochi.nimora
```

## 14. 生态与版权

- 商店必须展示作者、许可证、AI 生成声明和依赖资源。
- 发布者必须声明是否允许二次创作、商业使用和模型训练。
- 平台支持下架、撤销签名、侵权申诉和已购用户保留策略。
- 资源评分应包含兼容性、性能和可访问性标签，不只展示人气。
