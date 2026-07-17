# ADR-006：用户代码经沙箱 Host 与统一 Command 执行

> 状态：Accepted  
> 日期：2026-07-17

## 背景

用户需要自己编写代码控制宠物与自动化。如果直接执行任意 Node 程序，文件、网络、进程和密钥权限将绕过平台授权，也无法获得可靠取消、审计与故障隔离。

## 决策

提供 TypeScript/JavaScript 本地脚本和完整扩展 SDK。所有代码运行在受配额的独立 Host 中，只能通过版本化 SDK、Command Registry 与 Capability Broker 使用能力；不开放 Node 敏感内建模块。声明式自动化与代码扩展共享相同事件、权限和审计模型。Supervisor 通过版本化 JSONL 协议启动和管理 Worker，并负责超时、取消、输出预算与进程终止；进程内线程不能被当作沙箱。

## 结果

- 正面：保留可编程性，同时实现权限说明、跨模型控制、离线运行、回滚和故障隔离。
- 代价：与普通 Node 环境不完全兼容，SDK 需提供模拟器和调试器。
- 缓解：提供 Creator Studio、CLI、类型生成、事件回放和未来 WASM 运行时扩展点；具体引擎必须接入同一 Worker 协议，不能绕过 Gateway。
