# Nimora（灵栖）Platform

> 文档版本：0.1.0-draft  
> 更新日期：2026-07-17  
> 状态：开发基线

Nimora Platform 是一个跨平台桌面生命体运行时。它将桌宠体验、自动化、技能扩展、开放连接器和可控 AI Agent 组合为一个本地优先、权限可控、可审计的桌面平台。

## 产品原则

- **先是生命体，再是工具**：陪伴感、动作反馈和长期人格是用户价值入口。
- **能力全部注册**：命令、动作、触发器、工具、资源和 UI 贡献均通过稳定契约注册。
- **官方能力使用公开 API**：官方技能与第三方技能遵循相同扩展边界。
- **本地优先且默认安全**：开放入口默认关闭，密钥进入系统安全存储，高风险动作强制确认。
- **契约版本化**：事件、扩展、资源包、API 和配置均可独立演进。
- **可降级**：无网络、无 AI、单个技能故障时，核心桌宠仍可运行。
- **可编程但不越权**：用户代码通过 Manifest 声明能力，经隔离 Worker 与 Capability Gateway 调用模块；自动事件支持 `serial`、`drop`、`cancel-previous` 三种宿主监督策略。

## 文档入口

所有开发工作从 [`docs/INDEX.md`](docs/INDEX.md) 开始。该文件定义文档优先级、术语和变更流程。

| 文档 | 用途 |
|---|---|
| [`docs/GREENFIELD_BASELINE.md`](docs/GREENFIELD_BASELINE.md) | 全新项目事实、首版唯一契约与禁止历史包袱规则 |
| [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) | 全量能力状态、验证证据与必须继续闭环的缺口 |
| [`docs/PRODUCT_SPEC.md`](docs/PRODUCT_SPEC.md) | 产品范围、用户体验、功能域、版本规划 |
| [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) | 系统边界、模块、进程、依赖和技术选型 |
| [`docs/CUSTOMIZATION_ASSETS.md`](docs/CUSTOMIZATION_ASSETS.md) | 皮肤、动画、主题、音效、行为与创作者规范 |
| [`docs/MODEL_RENDERING_IMPORT.md`](docs/MODEL_RENDERING_IMPORT.md) | Live2D、实时 3D、VRM/glTF、模型导入与渲染适配 |
| [`docs/PROGRAMMABLE_CONTROL.md`](docs/PROGRAMMABLE_CONTROL.md) | 用户代码控制、脚本沙箱、SDK、调试与分发 |
| [`docs/DESIGN_AESTHETICS.md`](docs/DESIGN_AESTHETICS.md) | 品牌视觉、宠物动作、动效、声音、文案与设计验收 |
| [`docs/UI_DESIGN_SYSTEM.md`](docs/UI_DESIGN_SYSTEM.md) | UI Token、组件、布局、状态、视觉回归与发布门禁 |
| [`docs/FEATURE_EVOLUTION.md`](docs/FEATURE_EVOLUTION.md) | 新功能储备、创新方向、优先级与进入路线图门槛 |
| [`docs/EXTENSION_ECOSYSTEM.md`](docs/EXTENSION_ECOSYSTEM.md) | Skill、Connector、Agent Pack、Registry 生态规范 |
| [`docs/SECURITY_PRIVACY.md`](docs/SECURITY_PRIVACY.md) | 权限、沙箱、网络、密钥、审计和隐私策略 |
| [`docs/RELIABILITY_RESILIENCE.md`](docs/RELIABILITY_RESILIENCE.md) | 稳定性、故障域、恢复、背压、长稳和混沌测试 |
| [`docs/OFFLINE_DATA_LIFECYCLE.md`](docs/OFFLINE_DATA_LIFECYCLE.md) | 离线承诺、本地优先、当前数据结构和生命周期 |
| [`docs/DEPLOYMENT_OPERATIONS.md`](docs/DEPLOYMENT_OPERATIONS.md) | 构建签名、安装更新、分阶段发布和灾难恢复 |
| [`docs/OBSERVABILITY_DIAGNOSTICS.md`](docs/OBSERVABILITY_DIAGNOSTICS.md) | 日志、指标、Trace、诊断包和遥测边界 |
| [`docs/API_CONTRACT_GOVERNANCE.md`](docs/API_CONTRACT_GOVERNANCE.md) | API、事件、错误、幂等与首版唯一契约治理 |
| [`docs/ACCESSIBILITY_I18N.md`](docs/ACCESSIBILITY_I18N.md) | 无障碍、国际化、本地化和文化适配 |
| [`docs/DEVELOPMENT_GOVERNANCE.md`](docs/DEVELOPMENT_GOVERNANCE.md) | 模块边界、代码审查、依赖和文档治理 |
| [`docs/GIT_WORKFLOW.md`](docs/GIT_WORKFLOW.md) | Git 分支、提交、PR、合并、标签、Hotfix 与安全规则 |
| [`docs/RUNBOOK.md`](docs/RUNBOOK.md) | 启动、资源、扩展、网络、数据和安全故障恢复 |
| [`docs/RISK_REGISTER.md`](docs/RISK_REGISTER.md) | 产品、技术、生态、安全和交付风险登记 |
| [`docs/DELIVERY_TESTING.md`](docs/DELIVERY_TESTING.md) | 工作分解、质量门禁、测试矩阵和发布标准 |
| [`docs/FUNCTIONAL_TEST_PLAN.md`](docs/FUNCTIONAL_TEST_PLAN.md) | 分功能、分平台、可执行的详细验收用例 |
| [`docs/adr/`](docs/adr/) | 关键架构决策及其权衡 |

## 目标技术栈

- Desktop shell：Tauri 2
- Core：Rust
- UI 与渲染：TypeScript、WebView、PixiJS
- 扩展 SDK：TypeScript，稳定 JSON/IPC 契约
- Node 工具链：Corepack + pnpm，唯一锁文件 `pnpm-lock.yaml`
- 配置：用户侧 YAML，运行时 JSON Schema 校验
- 本地存储：SQLite + 文件资源仓库 + OS Secure Store
- 自动化测试：Rust tests、Vitest、Playwright、跨平台桌面冒烟测试

## 目标仓库结构

```text
apps/
  desktop/                 Tauri 桌面客户端
  creator-studio/          资源与扩展创作者工具
crates/
  runtime-core/            领域模型、命令、事件、权限
  platform-adapters/       Windows/macOS 系统适配
  extension-supervisor/    扩展进程监督与能力代理
packages/
  schemas/                 全部版本化 JSON Schema
  sdk/                     Skill/Connector/Asset SDK
  renderer/                宠物渲染和动画图
  automation/              规则模型与编辑器共享代码
  client/                  Gateway JS Client
extensions/official/       官方扩展
assets/official/           官方角色和主题资源
docs/                      规范、ADR、指南和运行手册
tests/                     contract、integration、e2e、fixtures
```

## 开发基线

项目当前处于 M1 Creator Foundation 开发阶段。M0 已落地 Rust 领域核心、应用用例层、唯一当前 SQLite Schema、可创建与激活的离线 Profile、Command/Event Trace 关联、事务 Outbox 的租约/ACK/重试/死信/清理协议、自动 SQLite 一致性备份调度、启动前安全恢复、数据库损坏时的隔离恢复模式、控制中心健康状态，以及 Tauri 双窗口、系统托盘和类型化 IPC。M1 已提供 `nimora.asset/1` 严格契约、限额归档导入、确定性导出、静态海报预览、Sprite Sequence/Atlas 真实渲染与回退、GLB 独立 Worker 探测、带 SHA-256 inventory 的本地 Character 包原子安装、`nimora.animation-map/1` 标准动作映射，以及 Pet Overlay 内由 Three.js 驱动的受控 GLB 2.0 WebGL 渲染和动作 cross-fade。具体 Outbox 消费者、VRM/Live2D Adapter、包签名、独立 Renderer 进程、诊断导出与跨重启崩溃循环恢复仍需继续闭环；全量范围和证据以 [`docs/IMPLEMENTATION_STATUS.md`](docs/IMPLEMENTATION_STATUS.md) 为准。

本地开发使用 `pnpm` 与 Rust stable：

```bash
pnpm install --frozen-lockfile
pnpm check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir apps/desktop tauri dev
```

所有 JavaScript/TypeScript 命令仅使用 `pnpm`，禁止使用 npm、Yarn 或 Bun。普通浏览器执行 `pnpm --dir apps/desktop dev` 时自动进入离线预览适配器，不调用伪造的原生能力。
