# ADR-010：Skill Agent Tool 绑定宠物行为（pet_behavior）

> 状态：Accepted
> 日期：2026-07-22

## 背景

Milestone 3（技能具象化）要求 Skill 不再是被动的后台函数，而是宠物的"技能树"：每个 Skill 的能力应当绑定一个宠物动作（例如 coding 技能绑定敲键盘动画），使宠物在执行技能时表现出对应的身体语言。

现有 `SkillAgentToolContribution` 已描述工具的 id、标题、命令、schema、风险与副作用，但没有任何表现层语义，宠物无法据此选择动画。

## 决策

在 `SkillAgentToolContribution` 增加可选字段 `pet_behavior: Option<PetAction>`，复用 `runtime-core` 既有的封闭 `PetAction` 词表（idle/observe/walk/play/perch/climb/peek/stretch/sleep/work/celebrate），而不是新增一套字符串动作名。

理由：

- `PetAction` 已是 Desktop、用户代码、Automation 与 Agent Action Catalog 的唯一权威动作词表；复用它避免第二套不一致的动作名，也让 Skill 绑定的动画天然受既有回退、取消与 Reduced Motion 规则约束。
- 使用封闭枚举而非自由字符串，serde 在反序列化时自动拒绝未知动作，无需额外校验代码即可 fail-closed。
- 字段可选（`#[serde(default)]`），旧 Manifest 无需迁移即可继续解析，符合"首版唯一契约、不制造迁移包袱"的原则。

## 后果

- 正面：Skill 能声明执行时的宠物身体语言，复用权威动作词表与既有安全规则；旧 Manifest 向后兼容；未知动作 fail-closed。
- 代价：`skill-runtime` 现在依赖 `runtime-core` 的 `PetAction`（此前已依赖 `CommandRisk`，依赖方向不变）。
- 边界：本 ADR 只增加声明字段并使其流经 `ActiveSkill` 与 `agent_tool_catalog` IPC；桌宠径向菜单据此"掏出"技能面板已在同一纵切实现，但实际执行 Skill 时播放该动画仍属后续 Desktop 宿主纵切。
