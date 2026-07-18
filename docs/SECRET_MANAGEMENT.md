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

## 7. 当前实现边界

`nimora-secret-store` 已实现严格引用、系统后端、零化读取、内存测试后端和基础门禁。桌面凭据设置 UI、Provider 配置仓储、OpenAI-compatible Worker、缓存加密和授权签名尚未接线，因此不能宣称网络 Provider 已支持 API Key。
