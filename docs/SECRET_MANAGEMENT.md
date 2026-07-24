# Nimora 密钥与凭据管理

> 契约：`nimora.secret-reference/1`  
> 更新日期：2026-07-19  
> 状态：核心存储层已实现，模块接线进行中

## 1. 目标

Provider API Key、Connector Token、Webhook Secret、缓存加密密钥和授权签名材料必须共用一个宿主拥有的 Secret Store。配置、SQLite、日志、诊断、IPC、扩展包、Workspace 和模型上下文只保存非敏感引用，不能保存明文。

## 2. 引用契约

引用采用 `secret:<domain>:<name>`，例如 `secret:provider:openai-work`。每段只接受小写 ASCII、数字、`.`、`_` 和 `-`；总长度不超过 160 字节。引用可进入配置和审计，但不得包含账号、Token 前后缀或其它可推断密钥的信息。

## 3. 宿主边界

- `SystemSecretStore` 使用操作系统原生密钥后端：macOS Keychain、Windows Credential Manager、Linux Secret Service。
- 不提供明文枚举、导出或日志接口；UI 只能查询 `present/missing`。
- 读取结果使用自动清零内存，空值、NUL 和超过 64 KiB 的值失败关闭。
- 删除是幂等操作；撤销 Provider 或 Connector 时必须先阻止新任务，再删除引用并终结使用该引用的租约。
- Program、Skill、Connector Worker、WebView、Provider 响应和用户脚本永远不能获得 Secret Store 对象；只有宿主 Adapter 在启动隔离 Worker 前解析精确引用。

## 4. Provider 接线规则

OpenAI-compatible Provider 配置只能保存 `credentialReference`。宿主必须在请求释放前复验 Provider、端点、模型、数据等级和引用绑定；密钥仅通过 Worker 标准输入传入一次，不能出现在命令行、环境变量、错误文本或 Worker 输出。网络目标变化、重定向到不同 Origin、引用缺失或系统后端锁定均在发送任何请求前失败。

## 5. 恢复与迁移

- 备份默认不包含 Secret；恢复后相关能力显示“需要重新绑定”。
- 跨设备迁移只迁移引用和缺失状态，不复制凭据。
- Safe Mode 禁止使用第三方网络凭据，但允许删除和撤销。
- 系统密钥后端不可用时不能降级为文件明文；已有离线核心能力继续运行。

## 6. 测试门禁

1. 引用路径逃逸、Unicode 混淆、超长引用、空值、NUL 和超大 Secret 均拒绝。
2. Put、Resolve、Presence 和幂等 Delete 使用可替换内存后端完成确定性测试。
3. 日志、错误、诊断、序列化 DTO 和 Debug 输出不得包含 Secret。
4. Provider Worker 测试必须证明命令行和环境无 Secret，重定向不跨 Origin，取消/超时后子进程被终止。
5. 真机发布前分别验证 Keychain、Credential Manager 和 Secret Service 的锁定、拒绝、删除和应用卸载行为。

本机系统密钥门禁默认忽略，避免常规测试意外创建系统凭据。显式验收命令为：

```bash
NIMORA_RUN_SYSTEM_SECRET_STORE_TEST=1 cargo test -p nimora-secret-store system_store_round_trip_is_present_resolvable_and_revocable -- --ignored --test-threads=1
```

该测试只使用唯一的合成值，不读取用户已有凭据，并在断言结果前执行二次幂等删除。macOS 证据不能替代 Windows Credential Manager 与 Linux Secret Service 的对应发布机验证。

## 7. 当前实现边界

`nimora-secret-store` 已实现严格引用、系统后端、零化读取、内存测试后端和显式真机门禁。桌面 Provider 配置仓储、凭据设置与撤销 UI、OpenAI-compatible 隔离 Worker、动态 Registry 和逐请求联网确认均已接线；API Key 不进入 SQLite、IPC 响应、命令行、环境变量或日志。Auto Mode Context Cache 使用固定非敏感引用 `secret:cache:auto-mode-context-v1` 管理独立 256-bit 系统密钥；每条记录采用随机 nonce 的 XChaCha20-Poly1305 信封，并以 Cache Key、Provider、模型、Workspace fingerprint、Plan revision、数据等级和时间边界作为 AAD。旧明文版本直接失效删除，错误密钥或元数据篡改 fail-closed，系统密钥不可用时不会降级写入明文。

**Authorization Grant at-rest key**：桌面宿主通过 `authorization_grant_key` 从系统密钥后端加载或生成固定引用 `secret:cache:authorization-grant-v1`（256-bit，hex 编码存储），并传入 `SqliteAuthorizationGrantRepository::open_with_key`。Grant payload 以 `nimora.encrypted-authorization-grant/1`（XChaCha20-Poly1305）落库；issue/list/revoke/control-center/auto-mode 均走该密钥。密钥串不可用或解析失败时 fail-closed 回退 `AuthorizationGrantKey::app_local_default`（仅本地/测试环境，不写明文 Grant）。密钥材料永不进入 IPC 响应、日志、模型上下文或 FE。

仍未完成的是 Windows Credential Manager、Linux Secret Service 和签名桌面包的发布机验收。
