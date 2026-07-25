# Nimora UI 视觉质量审计 — 控制中心 & 浮层
> 日期：2026-07-25 · 审计者：UI/视觉设计评审（只读）
> 基线：docs/UI_DESIGN_SYSTEM.md、docs/DESIGN_AESTHETICS.md
> 范围：React 控制中心 + 浮层。**未**改动任何文件；本文件为主 Agent 提供可直接套用的 styles.css 修改建议。
> 约束：不编辑 apps/desktop/src/styles.css（主 Agent 正在改动），不提交/推送。

---

## 0. 执行摘要（TL;DR）

当前控制中心的**布局骨架、栅格、响应式断点、状态语义 Token（`--status-*`）体系都相当扎实**，深浅色都有 re-derive，值得肯定。但整体"精致 / 高级 / 可信"的观感被三类系统性问题拖垮：

1. **字号全面击穿规范下限。** 设计系统明确规定"辅助信息不低于 12px、主要正文 13–16px、常规正文不低于 13px"。实测 styles.css 中有 **80 处 `8px`、99 处 `9px`、76 处 `10px` 字号**，集中在 扩展/Agent/自动化 三个模块。8px/9px 正文在 macOS/Windows 上直接触发"廉价、拥挤、像未完成的调试面板"观感，也是 200% 缩放与可读性一票否决项的高风险区。这是**当前最大、最普遍、修复收益最高**的问题。
2. **令牌旁路严重（硬编码 hex）。** 全文件约 **579 处 hex 字面量**，扩展模块尤甚（`#fffefa`×14、`#5344a0`×9、`#8f342c`×8……）。同一种"紫色强调 / 琥珀警告 / 薄荷成功"在不同模块用了略微不同的 hex，破坏跨主题一致性，深色模式下不会跟随皮肤。
3. **组件状态覆盖不完整。** 扩展模块的 `.ai-kind-grid button` 缺 hover/focus-visible/disabled；`.ai-draft-actions button`、`.ai-gap-footer button`、`.tool-catalog article > button` 等大量自定义按钮缺 `:hover`/`:focus-visible`；`.ai-creator-hint` 在组件中被引用但**CSS 未定义**；`var(--display)` 字体 Token 被引用 4 次但**从未定义**。

按设计系统第 13 节 0–2 分制粗评：**层级 1.5、对齐节奏 1.5、状态完整 1、文案 2、动效 1.5、无障碍 1（字号+对比度触发一票否决风险）、情感品质 1。当前不达 13/16 候选发布线**，主因就是字号与令牌旁路。

---

## 1. 概览 / 概览仪表盘（Dashboard + LifeformOverview）

**评价：全站最成熟的一块。** `.dashboard-grid` 用 `minmax(0,1.45fr) minmax(280px,.72fr)` + `max-width:1320px; margin:0 auto`，栅格干净；`.pet-stage` 渐变舞台是合理的情感焦点；`.lifeform-vitals` 的 inset 高光 + 柔和阴影有质感；vitals 各 `data-tone` 用不同渐变而非纯色，符合"不仅靠颜色"的间接表达。

问题：
- `.metric-row strong { font-size:34px }` 与 `.activity-summary strong{font-size:25px}`、`.section-heading h2{19px}` 之间跳档偏大，但可接受。
- `.chevron { color:#a8a89f }`、`.lifeform-name{color:#2a2f28}`、`.lifeform-chip` 系列、`.activity-icon.mint/violet/amber` 仍用硬编码 hex（后者已部分改用 `--status-*-soft`，是好方向）。
- `.lifeform-vital.compact .lifeform-vital-head{font-size:9.5px}` — 出现**小数字号 9.5px**，既低于下限又不在 4px 节奏上。
- `.pet-stage` 的 `.stage-actions` 里 `.secondary-button` 背景 `rgba(255,255,255,.56)` 在浅渐变上对比偏弱。

**气质：** 这块已接近"柔和数字工作台"目标，主要靠把 vitals/chip 的 hex 收进 Token 再上一个台阶。

---

## 2. 扩展 / AI 扩展工坊（AiCreatorWorkspace）—— 最需要抢救

这是用户点名"看起来还很糙"的模块，实测确实是**全站字号最小、hex 最密、状态最缺**的区域。

### 2.1 字号灾难（阻断级）
`.ai-creator-hero p` **11px**、`.ai-creator-layout legend/label>span/h4` **9px**、`.ai-kind-grid strong` **10px** / `small` **8px**、`.ai-creator-layout textarea/select/input` **10px**（输入框正文 10px 尤其难受）、`.ai-draft-heading small` **8px**、`.ai-draft-preview article` **9px**、`.ai-draft-preview>small` **8px**、`.ai-empty-state p` **9px**、`.ai-check-report` **9px**、`.ai-draft-actions button` **9px** / `small` **8px**、`.ai-permission-diff b` **8px**、`.ai-gap-footer button` **9px**。

整块几乎没有一个 ≥12px 的正文。这是"糙"的**直接主因**——不是布局问题，是字号问题。

### 2.2 令牌旁路（最严重的模块）
`#fffefa`×14、`#5344a0`×9、`#8f342c`×8、`#e6eee5`×7、`#7565ce`、`#7362e4`、`#7b6be0`、`#6657c7`、`#6556c5`、`#56499f`、`#3e7559`、`#245f50`、`#9a5c27`…… 一个模块里出现了**至少 5 种不同的"紫"和 3 种不同的"绿"**，全是手写 hex，深色模式完全不跟随。应全部映射到 `--accent` / `--status-run-*` / `--status-ok-*` / `--status-warn-*`。

### 2.3 状态覆盖缺口
- `.ai-kind-grid button` 只有默认态 + `.selected`；**缺 `:hover`、`:focus-visible`、`:disabled`**（disabled 由 `disabled` prop 控制但无视觉）。
- `.ai-creator-hint` 在 AiCreatorWorkspace.tsx:170 被 `role="status"` 引用，**CSS 中无定义** → 阻塞原因提示无样式，退化为浏览器默认黑色小字。
- `.ai-draft-actions button`、`.ai-install-button`、`.ai-gap-footer button` 均**无 `:hover`/`:focus-visible`**，只有 `:disabled`。
- 空态 `.ai-empty-state`（min-height:420px + `✦` 32px 单字符）过于空旷冷清，缺"下一步引导"的温度。

### 2.4 布局
- `.ai-creator-layout { grid-template-columns:minmax(0,.92fr) minmax(360px,1.08fr) }` 本身合理；但右栏 `pre` 用 `#25222f` 深底 + `#e9e4ff` 9px 等宽字，在浅色工作台里像一块突兀的黑色补丁，圆角 14px 与卡片 20px 不一致。
- `.ai-provider-row` 两列在窄屏 720px 才塌成一列，880–1000px 区间 Provider/模型两个下拉会挤。

---

## 3. 活动 / 活动场景工坊（ActivitySceneWorkshop / scene-workshop）

**评价：布局是全站第二好，明显比扩展模块用心。** `.scene-workshop-layout` 双栏 `minmax(280px,.95fr) minmax(0,1.15fr)`、1080px 单栏、720px hero 竖排，断点合理；`.scene-card` 有 `:hover`（translateY -2px）、`.active`（2px focus ring）；`.scene-prop`/`.scene-particle-chip` 有 `.selected`/`[aria-pressed]` 态；卡片圆角 18–22px 层级清晰。这块**字号也基本守住了 12–13px 下限**（`.scene-field 13px`、`.scene-card-body>p 12px`、`.scene-workshop-hero p 13px`），是正面样板。

问题：
- **状态仍缺 focus-visible。** `.scene-prop`、`.scene-particle-chip`、`.scene-filter-row .chip`、`.scene-card` 均**只有 hover/selected，无键盘 `:focus-visible`**（可交互卡片键盘不可见焦点，属无障碍缺口）。
- **hex 旁路。** `.scene-filter-row .chip.active{background:#343d2e}`、`.scene-card.active` 用 `rgba(91,140,78,…)`、`.scene-particle-chip.selected` 用 `rgba(115,98,228,…)` 与 `#4d3fad`/`#f0edff`、`.scene-source{background:#eef2ea}` 等，绿色选中态与紫色选中态混用（应统一到 `--accent`），且都未走 Token。
- `.scene-card-icon{font-size:32px}` 是 Emoji 图标 + `drop-shadow`——设计系统禁止"用 Emoji 代替核心产品图标"。功能性场景类型图标应换成几何图标（至少标注为已知偏离）。
- `.scene-notice{color:#4d704b}` 成功绿硬编码；应 `var(--status-ok-text)`。
- `.scene-workshop-active-pill>span{font-size:22px}` 又是 Emoji 图标同问题。

---

## 4. Agent 工作台（AgentWorkspace）

**评价：信息架构（主次双栏 + 900/720 断点、`agent-view-tabs`、companion-strip、danger-matrix）符合设计系统"Agent 工作台验收补充"的要求，状态 Token 用得最规范**（`.control-status-chip`、`.control-companion-chip` 全量走 `--status-*`，深浅跟随皮肤，值得作为其他模块的范例）。

问题：
- **字号大面积击穿。** `.control-status-chip{font-size:8px!important}`、`.control-effort-chip 8px`、`.tool-domain-chip 8px`、`.control-budget-slices small 8px/b 9px`、`.away-summary-metrics small 8px`、`.away-summary-footer 8px`、`.agent-message small 9px`、`.turn-tool-meta span 8px`、`.tool-catalog code 8px` / `button 8px`、`.agent-history article p 8px`、`.tool-confirmation>p 8px`、`.tool-complete p 8px`、`.usage-card dt 8px`、`.control-entry footer 8px`、`.provider-tile p 9px`…… 这些是被主用户读的运行数据/工具 ID/预算，8px 严重伤害可读与可信。**注意：`.control-status-chip` 用了 `font-size:8px!important`，主 Agent 提值时要一并去掉 `!important` 或覆盖它。**
- `.agent-composer textarea{font-size:12px}`（勉强达标）；但 `.agent-runtime-controls select/input{font-size:10px}` 低于下限——Provider/模型选择器正文应 ≥13px。
- `var(--display)` 在 `.agent-hero h2`、`.inspector-title h3`、`.agent-section-header`（共 4 处）被 `font-family:var(--display)` 引用，但**该 Token 全文件未定义** → 标题静默回退到继承字体，等于无效声明。要么定义 `--display`，要么删除引用。
- hex 旁路：`.agent-view-tabs button[aria-current]{background:#35382f}`、`.agent-message>span{background:var(--accent)}`（好）但 `small{color:#766e87}`、turn-request 一整套 `#8a552b/#f6dfc4/#806b58/#875231` 琥珀色、`.tool-confirmation{border:#e7cdb5;background:#fff8ef}` 都应映射到 `--status-warn-*`。
- `.usage-card dl div{background:#f7f6f2}`、`.tool-catalog article>span{color:#6253c6}` 等仍硬编码。

---

## 5. 角色工作室 / Creator Studio

**评价：预览画布（`.creator-preview` + `.preview-grid` 网点 + `.creator-pet` 呼吸动画）是有巧思的情感化设计。** 布局 `minmax(300px,1.1fr) minmax(260px,.9fr)` 合理。

问题：
- 字号：`.creator-header p 12px`（达标边缘）、`.creator-field 11px`、`.action-chips button 10px`、`.creator-note p 11px`、`.creator-checks li strong 11px / p 10px`、`.check-status 10px`、`.preview-badge 10px`、`.model-package-form label 10px` — 普遍 10–11px，需抬到 12–13px。
- hex 旁路极密：`#7362e4`（紫强调，与扩展模块的 `#7b6be0`/`#7565ce` 又不一样）、`#3e365f`、`#665d82`、`#f5f1fb`、`#f4f1fa`、`#625d75`、`#fbfaf7`、`#e6f0e3`、`#f7ecd8` …… 整个 Creator 的"紫"应统一到 `--accent` 或一个 `--creator-accent` Token。
- `.creator-checks li` 的 `.check-icon.pass{#52744e}` / `.pending{#9c7441}` 成功/警告色硬编码，应走 `--status-ok-*`/`--status-warn-*`。
- 状态：`.action-chips button`、`.creator-field select` 缺 `:hover`/`:focus-visible`。

---

## 6. 令牌旁路：最严重的 offender 清单（供主 Agent 优先消除）

按"影响面 × 出现频次"排序，主 Agent 合并到 Token 时优先处理：

| 优先级 | 选择器 / 模块 | 硬编码值 | 应替换为 |
|---|---|---|---|
| P0 | 扩展 `.ai-creator-layout` 全套紫色（`#7b6be0 #7565ce #6657c7 #6556c5 #56499f #5344a0`） | 6+ 种紫 | `--accent` / `--status-run-*` |
| P0 | 扩展/Creator 绿色（`#3e7559 #245f50 #47604d #52744e #e6eee5`） | 5+ 种绿 | `--status-ok-*` / `--success` |
| P0 | 扩展/Agent 琥珀（`#8a5a14 #765b22 #875231 #9a5c27 #f7e6d5 #fff8ea #f6dfc4`） | 多种 | `--status-warn-*` |
| P0 | 扩展/Agent 危险红（`#8f342c #983c35 #a33f35 #9b5555 #b44a42`） | 多种 | `--status-danger-*` / `--danger` |
| P1 | `#fffefa`×14 / `#fffdf8 #fffdfa #fbfaf7 #faf9f5` 表面白 | 多种"白" | `--surface-strong` / `--surface` |
| P1 | Creator `.creator-pet` 系列 `#3e365f #665d82 #7362e4` | 角色紫 | 单独 `--creator-*` Token |
| P2 | `.scene-filter-row .chip.active{#343d2e}`、`.agent-view-tabs button[aria-current]{#35382f}` | 深墨绿 | 统一一个 `--ink-invert-bg` Token |

---

## 7. 状态覆盖缺口汇总（hover / focus / loading / empty / error）

| 模块 | hover | focus-visible | disabled | loading | empty | error |
|---|---|---|---|---|---|---|
| 概览 | ✅ | ⚠️（仅全局 button） | ⚠️ | n/a | ✅ | n/a |
| 扩展 | ❌ ai-kind/draft/gap 按钮 | ❌ | ✅（部分） | ✅ 文案态 | ⚠️ 过空 | ✅ ai-creator-error（但 `.ai-creator-hint` 未定义） |
| 活动场景 | ✅ card | ❌ prop/chip/card 全缺 | ✅ | n/a | ✅ scene-empty | ⚠️ 无显式 error 态 |
| Agent | ✅（部分） | ✅（away/tabs/companion 有集中定义） | ✅ | ✅ workspace-load | ✅ control-empty | ✅ away-summary-error |
| Creator | ❌ chips/select | ❌ | ✅ | n/a | ⚠️ | ⚠️ |

关键阻断项：`.ai-creator-hint` 未定义、`var(--display)` 未定义、场景卡与道具键盘焦点不可见。

---

## 8. 优先修复清单（选择器级，可直接粘贴到 styles.css）

> 说明：以下均为**加法或改值**，不涉及删除结构。主 Agent 套用时注意 styles.css 里同名规则多为**单行压缩**，改值时在对应压缩行内替换。字号统一按 4px 节奏抬到 12/13px。

### A. 扩展模块字号抬升（最高收益）
```css
/* .ai-creator-hero p:not(.card-label) 11px -> 13px */
.ai-creator-hero p:not(.card-label) { font-size: 13px; }
/* legend / label>span / h4：9px -> 12px（保留字重与字距） */
.ai-creator-layout legend,
.ai-creator-layout label > span,
.ai-draft-preview h4 { font-size: 12px; letter-spacing: .04em; }
.ai-kind-grid strong { font-size: 13px; }
.ai-kind-grid small { font-size: 12px; }
.ai-creator-layout textarea,
.ai-creator-layout select,
.ai-creator-layout input { font-size: 13px; }
.ai-draft-heading small { font-size: 11px; }        /* eyebrow 允许 11 */
.ai-draft-preview article { font-size: 12px; }
.ai-draft-preview > p { font-size: 13px; }
.ai-draft-preview > small,
.ai-draft-actions small { font-size: 11px; }
.ai-empty-state p, .ai-empty { font-size: 12px; }
.ai-check-report { font-size: 12px; }
.ai-draft-actions button { font-size: 12px; }
.ai-gap-footer button, .ai-gap-footer small { font-size: 12px; }
```

### B. 定义缺失的 `.ai-creator-hint`（当前无样式）
```css
.ai-creator-hint {
  margin: 10px 0 0;
  padding: 10px 12px;
  border-radius: 10px;
  color: var(--status-warn-text);
  background: var(--status-warn-surface);
  border: 1px solid var(--status-warn-edge);
  font-size: 12px;
  line-height: 1.5;
}
```

### C. 扩展 `.ai-kind-grid button` 补全状态
```css
.ai-kind-grid button { transition: 120ms ease-out; }
.ai-kind-grid button:hover { border-color: var(--accent); background: rgba(93,115,81,.06); }
.ai-kind-grid button:focus-visible { outline: 3px solid rgba(117,101,206,.28); outline-offset: 2px; }
.ai-kind-grid button:disabled { opacity: .5; cursor: not-allowed; }
```

### D. 扩展按钮组走 Token + 补 hover/focus
```css
.ai-draft-actions button:first-child { color: var(--status-run-text); background: var(--status-run-surface); }
.ai-draft-actions button { background: var(--accent); }
.ai-draft-actions button:hover:not(:disabled) { filter: brightness(.96); }
.ai-draft-actions button:focus-visible,
.ai-gap-footer button:focus-visible { outline: 3px solid rgba(117,101,206,.28); outline-offset: 2px; }
```

### E. 活动场景：补键盘焦点（无障碍阻断）
```css
.scene-prop:focus-visible,
.scene-particle-chip:focus-visible,
.scene-filter-row .chip:focus-visible,
.scene-card:focus-within { outline: 3px solid rgba(91,140,78,.30); outline-offset: 2px; }
.scene-notice { color: var(--status-ok-text); }
.scene-filter-row .chip.active { background: var(--accent); }
```

### F. Agent：抬升运行数据字号 + 去 `!important`
```css
.control-status-chip { font-size: 11px !important; }   /* chip 允许 11 */
.control-effort-chip, .tool-domain-chip,
.control-companion-chip, .control-strip-label { font-size: 11px; }
.control-budget-slices small { font-size: 11px; }
.control-budget-slices b { font-size: 12px; }
.away-summary-metrics small, .away-summary-footer { font-size: 11px; }
.agent-message small { font-size: 11px; }
.agent-runtime-controls select,
.agent-runtime-controls input { font-size: 13px; }
.tool-catalog code, .tool-catalog article > button,
.agent-history article p, .agent-history article small,
.tool-confirmation > p, .tool-complete p,
.usage-card dt, .control-entry footer, .control-entry li em { font-size: 11px; }
.agent-history article strong { font-size: 12px; }
```

### G. `var(--display)` 二选一
```css
/* 方案1：在 :root 定义（推荐，标题会更有品牌感） */
:root { --display: Inter, ui-sans-serif, -apple-system, "Segoe UI", sans-serif; }
/* 方案2：若不想引入，删除 4 处 font-family:var(--display) 即可 */
```

### H. 扩展 `pre` 代码块与卡片圆角/配色统一
```css
.ai-draft-preview pre { border-radius: 16px; font-size: 12px; }   /* 对齐卡片 radius，抬字号 */
```

### I. Creator 状态色走 Token
```css
.check-icon.pass { color: var(--status-ok-text); background: var(--status-ok-soft); }
.check-icon.pending { color: var(--status-warn-text); background: var(--status-warn-soft); }
.creator-field select:focus-visible,
.action-chips button:focus-visible { outline: 3px solid rgba(117,101,206,.28); outline-offset: 2px; }
```

### J. 概览小数字号修正
```css
.lifeform-vital.compact .lifeform-vital-head { font-size: 11px; }   /* 9.5px -> 11px，回到节奏 */
```

---

## 9. 主 Agent 应套用的 Top 12（最高收益、最低风险）

见本文件末尾——已按可直接粘贴顺序整理，全部为改值/加法，不动结构，风险最低。

