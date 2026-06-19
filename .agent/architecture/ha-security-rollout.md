# ForgeOS — HA · Security · Rollout

> 配套 [north-star.md](north-star.md)。同为**目标**,非现状。

## 分布式高可用
- **无 SPOF**:服务无状态、≥3 副本、跨 AZ;状态外置到 HA 存储(Postgres Patroni/托管、Temporal 集群、Qdrant/NATS/Redis 集群)。
- **编排持久性**:Temporal 让 workflow 跨重启/重试可恢复;**人审等待几小时/几天不丢**。
- **数据面弹性**:Runner 临时、可抢占、工作幂等可重试;按队列深度自动伸缩(KEDA);失败重新调度。
- **背压**:控制面↔数据面用队列削峰,过载排队而非雪崩。
- **优雅降级**:跨厂商模型挂→Router 降档;Eval 过慢→gate 降级 advisory 并打标。
- **多区**:控制面 active-active / active-passive;数据驻留按租户。
- **SLO 分治**:控制面与数据面分别定义可用性目标、分别熔断。

## 自治执行安全(make-or-break)
> 自主运行 AI 生成代码的系统,安全是生死线。
- **microVM 强隔离**(Firecracker):每任务一次性 VM,零控制面凭据,最小权限。
- **出口允许列表**:防外泄 / 防 prompt-injection 驱动的外联。
- **仓库内容与工具输出视为不可信输入**:risky 操作经 Policy 引擎 + 人审闸门。
- **Secrets**:Vault 短时令牌、按租户 KMS;绝不进沙箱(除非 scoped+ephemeral)。
- **供应链**:SBOM、依赖扫描、security-review gate。
- **不可篡改审计日志**:每个 Agent 动作可追溯。

## 目标 ↔ 现在:桥接
设计北极星的回报 = v0 每块都朝它对齐、不返工。
gate 已 host-independent(将来即 PDP 执行点);`.agent/` 已 tool-agnostic(适配器即插即用)。

| 能力 | 北极星 | v0(已落地) | 路径 |
|---|---|---|---|
| 约束执法 | Policy PDP + 带外 harness 集群 | `gate.mjs`(行数/根目录) ✓ | v1 函数/循环依赖 → v2 PDP |
| 编排 | Temporal 集群 | = Claude Code 本体 | v2 forge-core(Go) |
| 模型路由 | Router + LiteLLM + 记分卡 | — | v1 Claude 档 → v3 跨厂商 |
| 执行隔离 | Firecracker microVM 池 | 本机 CC 会话 | v3 forge-runtime(Rust) |
| 上下文 | Context Engine + RAG | `.agent/` 文档 ✓ | v2 RAG |
| 评估 | Eval Engine + 记分卡闭环 | acceptance(计划) | v1 → v2 闭环 |
| 可观测 | OTel/Grafana + tripwire | — | v2 → v3 |
| UI | Next.js 控制台 | — | v3 |

> 我们在"v0 列"。单人此刻能推进的,仍是 v1 那条垂直切片——在 Claude Code 上验证编排与治理是否成立,是通往左列的唯一诚实起点。
