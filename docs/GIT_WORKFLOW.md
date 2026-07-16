# Nimora Git 开发与协作规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-17  
> 状态：强制工程基线

## 1. 目标

本规范确保代码、Schema、文档、资源、构建产物和发布版本可评审、可追溯、可回滚。项目采用受保护主干与短生命周期分支，不使用长期 `develop` 分支，也不采用需要多层合并的传统 Git Flow。

## 2. 仓库基本原则

- `main` 始终保持可构建、可测试、可生成候选版本。
- 禁止直接向 `main` 推送，所有变更通过 Pull Request 合并。
- 一个 PR 只解决一个清晰问题，不混入无关格式化、重命名或依赖升级。
- 公共契约、数据迁移、安全边界、进程模型和技术栈变化必须关联 ADR/RFC。
- 提交历史必须说明意图，不能依赖聊天记录才能理解。
- 禁止提交密钥、Token、证书私钥、真实用户数据、诊断包和受限模型素材。
- JavaScript/TypeScript 依赖只使用 `pnpm`，唯一锁文件为 `pnpm-lock.yaml`。

## 3. 分支模型

### 3.1 长期分支

| 分支 | 用途 | 规则 |
|---|---|---|
| `main` | 唯一开发主干 | 受保护、必须 PR、必须通过全部门禁 |
| `release/x.y` | 可选稳定版维护 | 仅在需要并行维护候选版或旧稳定版时创建 |

`release/x.y` 只能接收发布修复、版本元数据、迁移修复和发布文档，不接收新功能。版本发布结束后按维护策略保留或关闭。

### 3.2 工作分支

```text
feat/<issue>-<short-name>
fix/<issue>-<short-name>
docs/<issue>-<short-name>
refactor/<issue>-<short-name>
test/<issue>-<short-name>
chore/<issue>-<short-name>
security/<private-id>-<short-name>
release/<version>
hotfix/<version>-<short-name>
```

- 名称使用小写 ASCII、数字和连字符，不使用个人姓名或含糊名称。
- 分支从最新 `main` 创建，目标生命周期通常不超过五个工作日。
- 大功能使用 Feature Flag 和多个可独立验证的 PR，不长期保留巨型分支。
- 安全漏洞使用私有仓库、私有分支或安全公告流程，分支名不泄露漏洞细节。

## 4. 提交规范

采用 Conventional Commits：

```text
<type>(<scope>): <summary>

<body>

<footer>
```

### 4.1 Type

| Type | 用途 |
|---|---|
| `feat` | 新增用户或开发者能力 |
| `fix` | 修复缺陷 |
| `docs` | 仅文档变化 |
| `refactor` | 不改变外部行为的重构 |
| `perf` | 性能改进 |
| `test` | 测试与夹具 |
| `build` | 构建系统或依赖 |
| `ci` | CI/CD 配置 |
| `chore` | 其他维护 |
| `revert` | 回滚已有提交 |
| `security` | 安全加固或修复 |

### 4.2 Scope

Scope 使用稳定模块名，例如 `core`、`desktop`、`ui`、`renderer`、`assets`、`automation`、`sdk`、`gateway`、`extension-host`、`creator`、`docs`、`ci`。禁止使用人员姓名或临时项目代号。

### 4.3 提交内容

- Summary 使用祈使语气，描述结果，不写“update files”“fix stuff”等无信息内容。
- Body 解释为什么修改、关键权衡和非显然行为，不逐行复述 diff。
- 关联 Issue 使用 `Refs:`；确认关闭时使用 `Closes:`。
- 破坏性变更使用 `BREAKING CHANGE:`，说明影响、迁移和兼容窗口。
- 每个提交应保持逻辑原子性；代码与使其成立的测试、Schema、迁移可以同一提交。
- 禁止把失败尝试、临时调试、无关格式化和生成缓存带入最终历史。

示例：

```text
feat(automation): add cancellable script runs

Route cancellation through the command execution context so local scripts,
skills, and agent tools share the same cleanup semantics.

Refs: #184
```

## 5. 本地变更流程

```bash
git switch main
git pull --ff-only
git switch -c feat/184-cancellable-script-runs
corepack enable
pnpm install --frozen-lockfile
pnpm test
git status --short
git diff --check
```

- 拉取主干使用 fast-forward only，避免无意创建 merge commit。
- 合并前将工作分支更新到最新 `main`；允许 rebase 工作分支，但禁止改写共享分支和已发布标签。
- 提交前检查暂存区内容，禁止使用不经审查的全目录提交习惯。
- 临时实验使用本地分支或 stash，不把编辑器备份和调试输出加入 `.gitignore` 来掩盖问题。

## 6. Pull Request 规范

### 6.1 PR 必须包含

- 问题、目标和不在本次范围内的内容。
- 实现方案与重要权衡。
- 用户可见变化及 UI 前后截图/录屏；UI 必须覆盖浅色、深色和关键状态。
- 测试证据、执行命令和未覆盖风险。
- 权限、隐私、离线、失败恢复、性能与兼容影响。
- Schema/API/数据迁移变化及回滚方式。
- 关联 Issue、ADR/RFC 和文档。

### 6.2 PR 大小

- 推荐控制在 400 行有效人工 diff 内，生成文件、锁文件和快照单独标识。
- 超过 800 行必须在描述中解释拆分困难，并提供评审顺序。
- 大规模重命名、格式化、生成产物与行为修改分开提交或分开 PR。
- Draft PR 可用于早期架构和 UI 方向反馈，但不得作为绕过完整说明的方式。

### 6.3 评审要求

- 至少一名代码所有者批准；安全、公共契约、数据迁移和发布系统需要对应领域所有者批准。
- 作者不得批准自己的 PR；机器人检查不能替代人工领域评审。
- 所有阻塞评论必须解决或由评论者明确解除。
- 变更在批准后发生实质修改时自动撤销旧批准。
- 评审关注正确性、安全、边界、可读性、测试、UI 细节和长期维护，不只检查代码风格。

## 7. 合并策略

- 默认使用 squash merge，使一个 PR 在 `main` 上形成一个可回滚单元。
- Squash 标题必须符合 Conventional Commits，并作为发布说明来源。
- 对需要保留多个独立迁移步骤的 PR，可经维护者批准使用 rebase merge。
- 禁止普通功能 PR 产生 merge commit；`release/*` 与 `hotfix/*` 回合主干时可例外保留拓扑。
- CI 未通过、Review 未完成、分支过期或存在未解决冲突时禁止合并。
- 合并后删除工作分支；Issue、发布说明和项目状态由自动化更新。

## 8. 主干保护与必需检查

`main` 至少启用：

- 禁止 force push、删除和直接推送。
- 要求分支为最新或使用 merge queue。
- 要求签名提交或平台验证身份；Bot 使用独立最小权限身份。
- 要求 Code Owner 与最少批准数量。
- 要求会话消失后重新认证高危管理操作。
- 要求 Rust、TypeScript、Schema、契约、UI、文档和安全检查通过。
- 要求 `pnpm-lock.yaml` frozen install，拒绝其他包管理器锁文件。
- 要求密钥扫描、依赖审计、许可证检查和恶意文件检测。

## 9. CODEOWNERS

建议按责任而非个人便利划分：

```text
/crates/runtime-core/           @core-owners
/crates/extension-supervisor/   @security-owners @core-owners
/packages/schemas/              @contract-owners
/packages/sdk/                  @sdk-owners @contract-owners
/packages/renderer/             @render-owners
/apps/desktop/                  @desktop-owners @design-owners
/apps/creator-studio/           @creator-owners @design-owners
/docs/adr/                      @architecture-owners
/.github/                       @devops-owners @security-owners
```

关键目录必须由团队/角色拥有，避免唯一个人离开后无法评审。

## 10. 文档、Schema 与生成文件

- Markdown 相对链接、标题层级和术语由 CI 检查。
- Schema 是字段事实来源；生成目录不得手工修改。
- 生成文件必须记录生成命令、工具版本和源文件，并可在干净环境复现。
- 生成结果发生变化时，源文件与生成结果在同一 PR 提交。
- 大量快照变化必须由对应领域所有者审核，禁止无差别“更新全部快照”。
- 原始需求只能位于 `archive/original-docs/`；新开发不得重新引用为规范来源。

## 11. 二进制、模型与大文件

- 源码仓库不存放构建产物、安装包、崩溃转储和可下载缓存。
- 小型测试资源必须最小化、许可证明确、去除元数据并具有内容哈希。
- 大型官方模型、纹理、音频和测试语料优先进入版本化资源仓库或制品存储。
- 确需 Git LFS 时先通过 ADR，定义配额、镜像、离线开发和灾难恢复方案。
- 禁止提交无权再分发的 Live2D、Spine、VRM、字体、声音和训练数据。
- 模型安全测试使用专用恶意夹具，不使用真实用户文件。

## 12. 密钥与敏感数据

- `.env`、签名私钥、证书密码、Provider Token 和真实 Webhook 地址不得提交。
- 仓库只提供 `.env.example`，内容使用显然无效的占位符。
- 密钥扫描在 pre-commit、PR 和主干定期运行；发现泄露时先轮换，再清理历史。
- 单纯删除最新提交不足以处理泄露；必须撤销凭证、评估访问日志并按事件流程响应。
- 如需重写历史，必须由安全负责人组织冻结、通知、镜像清理和重新克隆。

## 13. 版本、标签与发布

- 稳定版本使用签名 annotated tag：`vMAJOR.MINOR.PATCH`。
- 预发布使用 `vMAJOR.MINOR.PATCH-alpha.N`、`beta.N`、`rc.N`。
- 标签只由受保护 CI 或发布负责人创建，禁止移动或复用已发布标签。
- 发布产物由标签对应提交在干净 CI 环境构建，不上传开发者本地产物。
- Release Notes 从提交与变更记录生成，并人工补充迁移、已知问题、安全和回滚说明。
- SDK、Schema、扩展协议和桌面应用版本可以独立演进，但发布清单必须记录兼容矩阵。

## 14. Hotfix 与回滚

```text
稳定标签/维护分支 → hotfix/<version>-<name> → 验证 → 新补丁标签
                                               ↘ 回合 main
```

- 不修改或覆盖旧标签，任何修复发布新的补丁版本。
- Hotfix 保持最小，不混入重构、新功能和常规依赖升级。
- 修复先进入当前受支持维护线，再立即回合 `main`，避免主干再次引入问题。
- 回滚优先使用 `git revert` 保留历史；禁止在共享分支 reset/force push。
- 数据迁移不可仅靠代码回滚，必须执行对应兼容、恢复或只读方案。

## 15. 安全修复流程

- 漏洞通过私有渠道报告和协作，不先创建公开 Issue。
- 使用私有 fork/安全公告开发，限制参与者和日志内容。
- 修复同时包含回归测试、受影响版本、缓解措施、升级建议和披露计划。
- 发布修复后按风险协调公告、Registry 撤回、密钥轮换和扩展隔离策略。
- 安全提交信息在披露前避免暴露可利用细节，但披露后补齐可审计记录。

## 16. Hooks 与自动化边界

- 本地 hook 用于快速反馈，服务端 CI 才是可信门禁。
- Hook 配置进入仓库并可通过 `pnpm` 安装，不依赖开发者全局工具。
- 禁止 hook 静默修改源码后直接提交；格式化后必须让开发者复核 diff。
- `--no-verify` 只允许定位 hook 故障，不得用于合并绕过；CI 仍执行同等检查。
- 自动化 Bot 不直接修改受保护主干，依赖升级和生成变更也必须走 PR。

## 17. 推荐 PR 检查清单

- 分支和提交名称符合规范，提交历史无临时噪声。
- 变更聚焦且可回滚，没有无关格式化或生成物。
- 测试、Schema、迁移、文档和 UI 状态随功能更新。
- 权限、隐私、离线、故障、性能和兼容影响有结论。
- UI 截图符合设计系统且基线变化经过批准。
- 没有密钥、真实数据、未授权资源和非 pnpm 锁文件。
- 合并标题可直接用于 Changelog，破坏性变更包含迁移说明。
