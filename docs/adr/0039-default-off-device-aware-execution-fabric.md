# ADR-0039 — Default-off Device-Aware Execution Fabric

- 状态：已接受（2026-08）
- 范围：执行平面目标抽象与渐进路线；planning-only，不授权任何远程访问
- 关联：ADR-0037、ADR-0038、`docs/design/ai-engineering-os/device-aware-execution-fabric.md`

## 背景

未来任务可能需要本机、异构设备、CI、集群或模型 endpoint，但当前 ForgeOS 只有本地 process、Docker/Firecracker 等基础；
完整 coding-workspace exchange、Rust runtime OS sandbox、远程 registry/runner/placement/lease/migration 均未实现。

若业务代码继续假设本机路径、localhost、共享文件系统和同步 shell，将来接设备会大规模重构；若现在直接用 SSH 拼命令，
又会掩盖分布式失败、身份、数据和副作用风险。

## 决策

1. 引入独立、可关闭、默认 OFF 的 Device-Aware Execution Fabric。接口可位置透明，失败必须显式。
2. 分离 Logical Agent、TaskNode、DecisionTransaction、ExecutionAttempt、ExecutionTarget；Agent 不永久绑定设备。
3. 现在先冻结 ExecutionTarget/Attempt、ArtifactRef、EnvironmentDigest、PlacementConstraint、Effect/Mobility、Lease/Checkpoint/
   Evidence ABI；LocalExecutionTarget 保持现有行为。
4. 设备身份来自 key/certificate/attestation；IP/hostname/SSH alias 仅是 locator。能力带 probe evidence/version/expiry，静态能力
   与动态状态分开。
5. Placement 先做硬过滤，再比较可靠性、数据/模型局部性、DAG/通信，最后才可在同等质量目标间考虑成本。Fabric 不改变
   目标或技术方案。
6. 远程 attempt 使用资源 reservation、lease epoch、fencing token、content-addressed Artifact 和 execution evidence；状态/
   checkpoint 外部化。LOST/取消超时不等于 FAILED/已停止。
7. 不承诺 exactly-once；使用幂等 key、effect receipt、reconciliation、compensation。不可逆或 effect unknown 进入 quarantine，
   禁止自动 retry/migrate/speculate。
8. SSH 只作 enrollment/bootstrap、诊断和早期结构化 MVP；正常路径是主动注册的 mTLS Runner。默认关闭 LAN scan、自动安装、
   arbitrary shell、peer transfer 和迁移。
9. ForgeOS 不因 Fabric 获得 production credential/apply/deploy 权限；生产继续由外部 operator authority。

## 模式与实施顺序

`OFF → INVENTORY → OBSERVE → EXECUTE → MIGRATE → FEDERATE`，每级仍需独立 policy/Grant。实施为：Local ABI → static
inventory/read-only probe/placement dry-run → structured SSH low-risk MVP → Runner/lease/fencing → scheduler/controllers → safe
checkpoint/reconciliation → federation last。

## 权限与诚实边界

- 本 ADR 不允许扫描网络、导入/复制私钥、连接 SSH、注册设备、远程执行或迁移；
- 沙箱不能防不可信设备管理员读取任务，因此 trust/data residency 是硬过滤；
- probe 不能安装/修复；成本默认 observe-only；fallback 必须重新可行性检查和授权；
- external message/real cost/模式升级/数据跨域必须回 00 影响评估和人类授权。

## 后果

正面：现在消除本机隐式假设，未来可以安全接异构算力、跨平台验证和模型服务；任务、证据、资源和失败可审计。成本：需要
identity/PKI、registry、CAS、Runner protocol、scheduler、leases、controllers、reconciliation、zero-trust policy 和 chaos tests。

## 被拒方案

1. `enable_remote=true` + SSH shell：权限粗、注入、状态/取消/恢复不可证明；
2. IP/hostname 当身份：地址变化或复用会混淆主体；
3. 所有设备共享可变目录：一致性、锁、跨平台语义和缓存污染；
4. 所有失败自动 retry/migrate：会重复真实副作用；
5. 一开始做 P2P/Federation：复杂度先于价值和内核稳定性。

## 重审触发器

- LocalExecutionTarget ABI 无法覆盖现有 sandbox/runner 行为；
- 目标 trust/数据驻留无法在 placement 前确定；
- lease/fencing/reconciliation 无法阻止迟到 attempt 提交；
- 远程 Fabric 会扩大已冻结的 production non-goal。
