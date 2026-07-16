# ADR-002：扩展通过独立 Host 与 Capability Broker 运行

## 状态

Accepted

## 背景

第三方 TypeScript 扩展需要生态友好，但独立 Node 进程只能隔离崩溃，不能阻止文件、网络和进程访问。

## 决策

扩展运行在独立 Host 中，默认禁止 Node 内建敏感模块。文件、网络、通知和应用启动等能力必须通过 Capability Broker。包含原生代码的扩展进入单独高风险进程。

## 后果

- 正面：权限可解释、可审计，单扩展故障不影响 Core。
- 负面：Host API 设计和跨平台沙箱实现成本较高。
- 缓解：早期仅开放官方和 Verified 扩展；逐步扩大第三方能力。

