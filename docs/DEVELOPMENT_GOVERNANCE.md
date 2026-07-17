# Nimora 开发规范与工程治理

> 首版阶段规则：项目尚无已发布用户数据与公共兼容承诺。首个稳定版前直接演进唯一当前契约，不为开发快照保留适配器、弃用层或迁移链；发布后治理仅在产生真实发布基线后生效。

> 版本：0.1.0-draft  
> 更新日期：2026-07-17

## 1. 目标

保证 Core、UI、扩展 SDK 和生态契约可以由多人长期维护，避免模块耦合、文档漂移和安全绕路。

## 2. 分支与变更

完整 Git 分支、提交、PR、合并、标签、Hotfix 和安全修复流程见 [`GIT_WORKFLOW.md`](GIT_WORKFLOW.md)，该文件属于强制工程基线。

- 每个变更关联需求、缺陷、ADR 或 RFC。
- 公共契约变更必须包含 Schema diff 和兼容结论。
- 安全边界、技术栈和进程模型变更必须新增 ADR。
- 功能 PR 必须同时更新测试和适用文档。
- 禁止在功能 PR 中混入无关重构。
- 默认采用受保护 `main`、短生命周期分支和 squash merge，不维护长期 `develop` 分支。

## 3. 代码边界

- Domain Core 不依赖 UI、Tauri、HTTP 和数据库框架。
- Adapter 实现 Port，不向 Core 泄露平台类型。
- UI 不直接写数据库或读取 Secure Store。
- 扩展和 Agent 不直接访问 OS 能力。
- 每个模块公开最小 API，内部实现不可跨包引用。

## 4. Rust 规范

- 错误使用稳定领域错误，不在库层直接展示用户文案。
- 异步任务可取消并由 Supervisor 持有。
- 禁止在 Core 热路径使用无界 channel。
- `unsafe` 代码需要独立审查、说明不变量和测试。
- 平台差异集中在 adapter crate。

## 5. TypeScript 规范

### 5.1 唯一包管理器

- 仓库唯一 JavaScript/TypeScript 包管理器为 `pnpm`，通过 Corepack 固定版本。
- 根目录提交 `pnpm-lock.yaml` 与 `packageManager` 字段；CI 使用 `pnpm install --frozen-lockfile`。
- 禁止提交 `package-lock.json`、`npm-shrinkwrap.json`、`yarn.lock`、`bun.lock` 或 `bun.lockb`。
- 文档、脚本、模板、CI 和发布流程禁止使用 npm、Yarn 或 Bun 命令。
- Workspace、catalog、patch 和 overrides 统一由 `pnpm-workspace.yaml` 与根配置治理。

- 启用 strict，不使用隐式 `any`。
- IPC、Schema 和外部数据在边界校验。
- UI 状态与 Core 状态明确区分，不复制唯一事实。
- 扩展 SDK API 返回可处理的结构化错误。
- 资源和订阅统一使用 Disposable 生命周期。

## 6. 依赖治理

- 新依赖记录用途、许可证、维护状态、体积和替代方案。
- 锁定依赖并生成 SBOM。
- 原生依赖、网络库、解析器和加密库需要安全审查。
- 禁止在多个模块引入解决同一问题的重复库，除非 ADR 批准。

## 7. Code Review

Review 必须检查：需求覆盖、依赖方向、失败路径、取消、权限、隐私、迁移、跨平台、离线、可测试性和文档。安全关键代码至少两人审查。

## 8. Feature Flag

- 未完成的大功能使用明确 Feature Flag，不保留隐藏后门。
- Flag 有所有者、默认值、目标移除版本和遥测指标。
- 安全策略不能通过普通 Feature Flag 关闭。
- 稳定发布前清理过期 Flag。

## 9. 文档治理

- `docs/INDEX.md` 是文档入口。
- Schema 是字段事实来源，Markdown 不手工维护重复全量字段。
- 原始需求只保存在 `archive/original-docs/`。
- 每个稳定版本检查链接、术语、版本和弃用内容。

## 10. 完成标准

代码、测试、Schema、迁移、日志、用户错误、文档和回滚策略缺一不可。仅“正常路径可运行”不视为完成。
