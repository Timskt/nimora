# ADR-004：区分 Command、Event 与 Query

## 状态

Accepted

## 背景

Gateway、Agent、Automation 和 Skill 都需要执行能力。如果各自实现调用逻辑，会造成权限、审计、幂等和错误语义分裂。

## 决策

Event 只描述已发生事实；Command 表示有副作用的执行意图；Query 只读取状态。Agent Tool、Automation Action 和 Gateway 操作最终映射到统一 Command Registry。

## 后果

- 正面：权限、风险、确认、撤销、审计和测试可以统一。
- 负面：简单功能也需要正式注册 Command。
- 缓解：SDK 提供类型安全的注册助手和代码生成。

