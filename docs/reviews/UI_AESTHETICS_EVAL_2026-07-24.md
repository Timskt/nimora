# UI Aesthetics Evaluation — 2026-07-24

> 角色：UI Aesthetics 子轨  
> 产品：Nimora / 灵灵（`/Users/sky/code/vibe/gptpet`）  
> 依据：实际 CSS / 组件源码（`styles.css`、`PetOverlay`、`BuiltinPet3D`、`BuiltinPet`、`LifeformOverview`、`AgentWorkspace`、`ProviderSettings`）+ `docs/UI_DESIGN_SYSTEM.md` / `docs/DESIGN_AESTHETICS.md`  
> 气质目标：**cute Q-minion desktop lifeform** — 轻盈、温暖、克制、可信；角色是情感焦点，控制面是安静舞台

---

## 1. 评估范围与方法

| 表面 | 关键文件 | 验收焦点 |
| --- | --- | --- |
| 桌宠 Overlay | `PetOverlay.tsx`, `styles.css` (`.pet-*`) | 260×320 shell、无方框、ellipse hit、气泡/径向菜单层级 |
| Q 生命体 | `BuiltinPet3D.tsx`, `BuiltinPet.tsx` | 暖黄胶囊、双护目镜、棕虹膜、全身可见、无 chrome plate |
| 控制中心舞台 | `LifeformOverview.tsx` + `.lifeform-stage*` | pedestal 环境光、overflow visible、fallback 柔和 |
| Agent 工作区 | `AgentWorkspace.tsx` | 层级、空态/锁态、授权档位视觉语义 |
| Provider | `ProviderSettings.tsx` | 信任区、列表/编辑器双栏、空态质感 |
| 全局壳 | `.app-shell`, tokens in `:root` | 柔和工作台、圆角与间距节奏 |

方法：对照设计系统 token 与「角色 > 工具」层级做代码走查；本迭代对 hit target / 空态做了 **&lt;80 LOC** 视觉润色（见 §8）。

---

## 2. 总体评分（美学）

| 维度 | 分 ( /10) | 一句话 |
| --- | --- | --- |
| 布局与构图 | 8.0 | Overlay 绝对舞台 + CC 双栏清晰；Agent 信息密度偏高 |
| 间距与节奏 | 7.5 | 4/8 基线大体遵守；多处 8–9px 字挤压呼吸感 |
| 色彩与材质 | 8.5 | 暖中性 canvas + 紫点缀；Q-minion 暖黄正确，无科技青眼 |
| 字体与排版 | 6.5 | 中文可读下限被 8px/9px 大量破坏 |
| 视觉层级 | 8.0 | 宠物主体突出；CC 英文 card-label 略抢戏 |
| 产品气质 | 8.5 | 桌宠侧可爱到位；工具侧偏「安全工作台」略冷静 |
| 状态设计 | 7.5 | empty/loading 有骨架，质感不均 |
| **综合** | **7.8** | 可发「陪伴向」预览；视觉 polish 需压字号与密度 |

---

## 3. 桌宠 Overlay 美学

### 3.1 Shell 与「无方框」约束 — **通过（架构正确）**

源码契约明确：

- 逻辑身体：`--pet-body-width: 260px; --pet-body-height: 320px`（`body.pet-window`）
- `.pet-character-shell`：`overflow: visible`，`background/border/box-shadow` 全部强制透明/无
- 注释与结构：Speech / stage / hit button **兄弟节点**，ellipse `clip-path` **只作用 hit-testing**，不裁 3D canvas  
  （`PetOverlay.tsx`：「ellipse clip-path never trims the lifeform」）

这是正确的生命体呈现：视觉在 clip 外完整；交互贴身体。

**风险残留**

- 浏览器 Preview 棋盘背景（`.pet-browser-preview`）易被误当成「有方框」截图证据——文档已区分，评审时需标注环境。
- 遮挡 `clip-path`（`--pet-occlusion-clip`）若宿主传矩形，会瞬时「切成方块」——属 occlusion 语义，需真机确认边缘羽化而非硬方框。

### 3.2 角色气质（Q-minion）

`BuiltinPet3D` 显式锁定：

- 暖黄 `#F7D117` / `#FFE94A`，禁止 chrome plate  
- 双护目镜 + 暖可可虹膜（非科技青）  
- 高胶囊胶囊比例校验 helpers  

SVG `BuiltinPet` 走淡紫「小兽」回退，与 3D 小黄人品牌略分裂，但作为 WebGL 失败兜底可接受；fallback 文案已改为「灵灵先用示意出现」降低惊吓感。

动效：呼吸 / 眨眼 / 拖拽 squash、抚摸 ♥ 粒子，气质偏 QQ 宠物。`prefers-reduced-motion` 有全局与菜单收敛。

### 3.3 气泡与快捷条

- 气泡：毛玻璃、不对称圆角、小三角，hover / `bubble-visible` 显隐 —— 轻盈  
- `.overlay-actions`：胶囊 dock，hover 浮现；本迭代按钮 **29→34px**，更易点、略更「玩具感」  
- 径向菜单 54px 圆钮 + 中文标签（本迭代 **9→10px**）—— 亲和

**问题**

- 快捷条 6 个符号按钮（◒✧♢✦☾⌁）**缺可见文字标签**，美观克制但学习成本见 Usability 报告  
- 气泡默认句「本地陪伴中，随时叫我」偏产品说明，不够「小黄人在桌面上碎碎念」

---

## 4. Control Center · Lifeform Stage

`.lifeform-stage` 明确 **「Ambient desk light only — never a square clip plate」**：

- `border-radius: 0; overflow: visible; box-shadow: none; border: 0`  
- 仅径向环境光 + 地面椭圆模糊 + 暖 glow  
- canvas 全透明强制链与 Overlay 一致  

**通过** 无正方形裁切板。舞台高度 `min(52vw, 340px)` 与 260×320 身体协调。

右侧 meta（姓名、vitals、sense chips）与舞台分离得当；`max-width: 320px` 与产品约束对齐。

---

## 5. AgentWorkspace 美学

**优点**

- 分区卡片 22px 圆角、柔和渐变，符合「柔和数字工作台」  
- 授权档位：selected / never-ask 琥珀 / full_device 危险红 **语义色清晰**  
- Companion chips（action / mood）小胶囊，把 Agent 状态「宠物化」表达

**问题**

1. **字号过密**：大量 `font-size: 8px`（metrics、grant meta、dialog dt、risk 列表）——中文几乎糊成「脚注云」，伤害高级感  
2. **英文 eyebrow**（`GOAL CONTROL CENTER · SUBJECT`、`UNATTENDED EXECUTION GRANT`）与中文正文混排，气质偏 SaaS 后台，与「灵灵」可爱主体略脱节  
3. 五档 `authorization-tier-options` 同屏并排时，小屏会挤成「贴图墙」；危险档与安全档视觉权重需更拉开  
4. `.control-empty` 卡片本身质感好，但与 Provider empty 的「可爱空态」不统一

---

## 6. ProviderSettings 美学

双栏：列表 + 编辑器，圆角 19px，选中淡紫描边，**信任感**表达正确（密钥不回显、浏览器只读条）。

本迭代：

- 空态改为暖径向底 + 圆形图标容器  
- loading 用轨道旋转圆，与 workspace-load 语言统一  

仍偏「安全设置页」：card-label `SECURE PROVIDER HUB` 全大写英式；若产品要统一中文气质，eyebrow 可改为「安全连接 · 模型」类短中文。

---

## 7. 设计系统一致性

| Token / 原则 | 现状 |
| --- | --- |
| Primitive → Semantic | `:root` 有 `--canvas/--surface/--accent` 等；业务仍大量硬编码 hex |
| 4px 间距 | 大体遵守；overlay gap 7px 等奇数可接受 |
| 圆角层级 | 卡片 16–28、按钮胶囊 999 — 亲和 |
| 阴影只表层级 | Overlay 与 CC 基本正确，无大面积霓虹 |
| 状态不全靠颜色 | Grant badge + 文案；Provider `data-ready` 徽章 OK |
| 角色色 vs 平台色 | 桌宠紫/黄 与 CC 森绿 accent 并存，略双主题但可辨 |

**缺口**：未全面消费 `DESIGN_AESTHETICS` 里的 `--dp-*` 令牌命名；长期应收敛，避免「文档 token / 实现 hex」两套。

---

## 8. 本迭代已落地的小 polish（&lt;80 LOC）

| 改动 | 文件 | 美学意图 |
| --- | --- | --- |
| 快捷操作 34×34 + 轻 hover 抬升 | `styles.css` | 更易点、更像玩具控件 |
| 径向中文标签 10px | `styles.css` | 中文清晰、不伤圆钮构图 |
| 背包空态柔和底 + 惊喜文案 | `styles.css`, `PetOverlay.tsx` | 空态也有陪伴感 |
| Provider 空/加载态 | `styles.css`, `ProviderSettings.tsx` | 可爱空桌 + 明确等待 |
| Lifeform 3D fallback 文案 | `LifeformOverview.tsx` + CSS strong | 降级不「坏掉」 |

净增约二十行量级，**未改架构、未加方框、未动 260×320 shell**。

---

## 9. 优先级建议（仅美学）

### P0 — 发版观感

1. **禁止回归方框**：任何 PR 不得给 `.pet-character-shell` / `.lifeform-stage` / canvas 加背景板、1px 边框、`overflow:hidden` 矩形裁切  
2. **真机全身帧**：确认靴/影/发在 260×320 内完整（Ortho frustum 文档要求）  
3. **中文最小字号门禁**：常规 UI ≥12px，辅助 ≥11px；废除 8px 中文正文（可保留 10px 徽章）

### P1 — 气质统一

4. 气泡默认库：情境短句优先（会议轻声 / 低电贴墙 / 未读眨眼），减少说明书句  
5. card-label 中文化或弱化为 10px 中文 eyebrow  
6. SVG fallback 与 3D 小黄人色板对齐（或明确「示意剪影」灰紫）

### P2 — 系统化

7. 收敛 hard-coded 色到 semantic token  
8. Agent 五档授权在窄屏改纵向 stack，避免「五小块贴图」

---

## 10. 结论

Nimora 桌宠侧已经具备 **Q-minion 生命体** 的正确骨架：透明 shell、椭圆 hit、舞台环境光、暖黄双镜、无正方形板。控制中心是合格的「安静工作台」，信任与危险语义清楚，但 **字号过小与英文 SaaS 语气** 削弱可爱与精致。

美学就绪：**陪伴预览 ~78%**；要到「截图即种草」需 P0 字号与真机全身帧，并持续守住 **no square frame**。

---

*关联：`USABILITY_EVAL_2026-07-24.md` · `MULTI_AGENT_PRODUCT_OPTIMIZATION_2026-07-24.md`*
