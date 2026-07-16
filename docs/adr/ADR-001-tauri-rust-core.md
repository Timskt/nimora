# ADR-001：使用 Tauri 2 与 Rust Core

## 状态

Accepted

## 背景

项目需要透明常驻窗口、托盘、跨平台系统能力、较低资源占用和可隔离扩展。先开发纯 Web Demo 再迁移桌面壳会推迟发现窗口、IPC 和平台差异。

## 决策

从 M0 开始使用 Tauri 2。领域 Core 使用 Rust，UI、渲染和扩展 SDK 使用 TypeScript。Web Demo 只能作为渲染与协议测试工具，不作为主运行时。

## 后果

- 正面：尽早验证真实桌面行为；Core 性能和系统能力稳定。
- 负面：初期构建和跨语言调试复杂；团队需要 Rust 能力。
- 缓解：公开边界采用 Schema 和 typed IPC，UI 不直接依赖 Rust 内部模型。

