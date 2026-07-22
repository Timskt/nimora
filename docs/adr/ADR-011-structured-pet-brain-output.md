# ADR-011：AI 输出结构化宠物行为指令（pet_brain）

> 状态：Accepted
> 日期：2026-07-22

## 背景

Milestone 4（智能涌现）要求 AI 不再是纯文本聊天，而是宠物的"大脑皮层"：模型基于桌面上下文与宠物状态，输出 `{ speech, mood, action }` 结构化指令，驱动宠物"想做什么"，而非返回渲染层无法可靠解析的散文。

模型输出是不可信输入，直接喂给渲染层会带来越界动作、超长气泡文本与格式漂移风险。

## 决策

在 `agent-runtime` 新增纯模块 `pet_brain`，承担该契约中必须确定且安全的两半：

1. **解析与校验**：`parse_brain_instruction` 把模型 JSON 解析为 `PetBrainInstruction { speech, mood, action }`。`action` 复用 `runtime-core` 权威封闭词表 `PetAction`；`mood` 使用封闭 `PetMood` 枚举；`deny_unknown_fields`；speech 先 trim 再按 `MAX_SPEECH_BYTES` 上限校验。未知动作/情绪、未知字段、非法 JSON、超长文本全部 fail-closed。
2. **人设注入**：`personality_system_prompt` 把 `PetPersonality`（energy/curiosity/laziness/pride）渲染成 System Prompt 片段，用定性档位（很低…很高）而非原始数字描述性格，并声明输出契约，使模型既保持人设又返回可解析的结构化指令。

理由：
- 复用 `PetAction` 避免第二套动作词表，动作天然受既有回退/取消/Reduced Motion 约束。
- 封闭枚举 + `deny_unknown_fields`，serde 反序列化即 fail-closed，无需额外校验分支。
- 模块纯字符串输入输出，不依赖 Provider、网络或 Tauri，符合架构边界，可隔离单测。

## 后果

- 正面：模型输出经确定性安全网关后才驱动宠物；人设注入锁定语气；未知/越界/超长输出 fail-closed。
- 代价：宿主需在实际 Provider 调用处接入本模块（把 `personality_system_prompt` 注入请求、用 `parse_brain_instruction` 解析响应）——本 ADR 只提交纯领域模块与其单测。
- 边界：Trace 回溯讲故事（M4 第二项）与实际 Provider 接线属于后续独立纵切；本模块不读取 Trace、不发起网络、不落库。
