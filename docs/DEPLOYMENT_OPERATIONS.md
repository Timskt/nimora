# Nimora 构建、部署、发布与运维规范

> 版本：0.1.0-draft  
> 更新日期：2026-07-17

## 1. 发布模型

Nimora 是桌面应用，不依赖中心服务即可运行。发布体系包含桌面客户端、Schema/SDK、官方扩展、Registry 元数据和可选在线服务。

## 2. 发布通道

| 通道 | 受众 | 更新策略 |
|---|---|---|
| dev | 开发人员 | 每次成功构建 |
| nightly | 测试和创作者 | 每日，不保证数据兼容 |
| beta | 自愿测试用户 | 签名，提供迁移和回滚 |
| stable | 普通用户 | 分阶段发布，严格兼容 |
| enterprise | 组织设备 | 版本锁定、延迟更新、私有源 |

## 3. CI/CD 流水线

```text
format/lint
 → unit/schema/contract
 → Rust + TypeScript build
 → SBOM/license/security scan
 → platform integration tests
 → signed package
 → install/upgrade/uninstall tests
 → notarization or signing verification
 → staged release
```

发布流水线必须可复现并锁定依赖。签名密钥只存在于受保护的 CI 环境。

桌面构建前必须运行 `pnpm build:sidecars`，按 `TAURI_ENV_TARGET_TRIPLE` 为当前目标编译并放置 `nimora-user-code-worker` 与 `nimora-model-importer-worker`。每个平台发布任务必须验证两个 external binary 均存在、架构匹配、可启动并被最终安装包包含；本机产物不能代替 Windows、macOS Intel 或其它目标的 CI 验证。

## 4. Windows 交付

- 支持 Windows 10 22H2 和 Windows 11。
- 产物使用代码签名，安装器和可执行文件均验证签名。
- 明确安装范围：per-user 默认，enterprise 可选 per-machine。
- 卸载器支持保留或删除用户数据。
- 验证 WebView2 安装和兼容策略。
- 更新进程不得绕过 UAC 或静默扩大权限。

## 5. macOS 交付

- 构建 Apple Silicon 与 Intel，或提供经过验证的 Universal Binary。
- 使用 Developer ID 签名和 notarization。
- 启用 Hardened Runtime，只申请必要 entitlement。
- 验证首次启动、Gatekeeper、登录项和多 Space 行为。
- 更新包和应用本体都必须校验签名链。

## 6. 自动更新

- 默认自动检查、用户确认安装；企业策略可关闭。
- 更新清单经过签名，包含版本、通道、最低系统、hash 和发布时间。
- 分阶段按 1% → 10% → 50% → 100% 推送，并支持远程暂停。
- 更新前检查磁盘、备份配置、停止扩展和完成中的事务。
- 启动健康检查失败时自动回滚或进入恢复模式。
- 安全更新可以提高提示等级，但仍必须说明影响。

## 7. 官方 Registry 与在线服务

- 静态元数据优先使用 CDN，可离线缓存。
- 包文件内容寻址并不可变；新版本创建新对象。
- Registry API 与客户端运行解耦，服务故障不影响已安装功能。
- 发布、撤回、签名吊销和安全公告使用独立权限。
- 服务器不接收本地宠物数据，除非用户启用明确的云功能。

## 8. 企业部署

支持：

- MSI/PKG 或组织认可的安装格式。
- MDM、Intune 等配置分发。
- 私有 Registry 和包 allowlist。
- 禁止外部 Provider、限制 Gateway、固定更新通道。
- 组织策略与用户设置分层，用户不能降低强制安全策略。
- 静默安装不等于静默采集，隐私选项仍需明确。

## 9. 配置与环境

构建期、运行期和用户配置必须分离。禁止通过未记录的环境变量改变安全策略。开发者模式使用单独安装标识和数据目录，不能污染 stable 数据。

## 10. 发布前检查

- 双平台签名验证。
- 全新安装、覆盖升级、失败回滚、卸载测试。
- Schema 和数据迁移测试。
- 默认离线启动测试。
- 默认端口、权限和遥测状态检查。
- SBOM、许可证和第三方资源版权清单。
- Release Notes、已知问题和恢复说明。

## 11. 回滚与召回

- 客户端发布保留上一稳定安装包和迁移前快照。
- 包或扩展可通过 Registry 标记撤回和安全严重度。
- Critical 问题可停止更新、禁用受影响包并发布安全公告。
- 不允许远程删除用户数据或无说明禁用本地核心能力。

## 12. 灾难恢复

| 事件 | 恢复措施 |
|---|---|
| 签名密钥泄露 | 吊销、轮换、发布新信任链和公告 |
| Registry 被篡改 | 停止发布、验证不可变对象、恢复签名元数据 |
| 坏版本大面积崩溃 | 暂停阶段发布、回滚、提供离线修复包 |
| 迁移损坏数据 | 启动只读恢复、使用本地快照和修复工具 |
| 在线服务不可用 | 客户端离线运行，展示服务状态但不阻塞启动 |
