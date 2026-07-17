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
  "engines": { "deskpet": ">=1.0.0 <2.0.0" },
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
    "hitboxes": "interactions/hitboxes.json"
  },
  "capabilities": ["pet.walk", "pet.drag", "pet.sleep"],
  "fallbacks": { "pet.happy": "pet.idle" },
  "locales": ["zh-CN", "en"],
  "integrity": { "algorithm": "sha256", "files": "integrity.json" }
}
```

`name`、描述和商店标签必须可本地化。许可证、作者和第三方素材声明为发布必填项。

## 5. 渲染后端

第一阶段必须支持 `sprite-sequence` 和 `sprite-atlas`。版本化 Renderer Adapter 进一步支持 Lottie、Spine、Live2D Cubism、glTF/GLB 与 VRM；OBJ/FBX 仅作为隔离转换输入。任何格式不得改变 Pet Runtime 的动作语义，完整规则见 [`MODEL_RENDERING_IMPORT.md`](MODEL_RENDERING_IMPORT.md)。

资源包声明渲染能力，Core 只使用标准动作：

```text
pet.idle pet.walk pet.sleep pet.drag pet.drop
pet.click pet.happy pet.sad pet.hungry pet.talk
```

自定义动作使用 `<publisher>.<action>`。缺失标准动作时按 `fallbacks` 回退，最终回退 `pet.idle`。

## 6. 动画图

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

## 12. 创作者工具

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

## 13. 生态与版权

- 商店必须展示作者、许可证、AI 生成声明和依赖资源。
- 发布者必须声明是否允许二次创作、商业使用和模型训练。
- 平台支持下架、撤销签名、侵权申诉和已购用户保留策略。
- 资源评分应包含兼容性、性能和可访问性标签，不只展示人气。
