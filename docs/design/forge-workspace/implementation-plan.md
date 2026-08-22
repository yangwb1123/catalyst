# Forge Workspace 总体实施计划

> 状态：**Proposed / 计划，不代表 Roadmap 已采纳**
> 日期：2026-08-21
> 依赖：[目标架构蓝图](../ai-engineering-os/product-control-plane-blueprint.md) · [详细架构](architecture-plan.md) · [功能交互](functional-interaction-design.md)

## 1. 实施目标

把现有可信执行资产收敛成一个本地优先产品：用户在 Space 中提出 Objective，系统形成 Change 和 WorkGraph，通过 Codex/Claude 等 Agent 执行，实时展示 Action，独立验证结果，在策略允许时继续推进，并生成可追溯 Outcome。

首个可用版本必须完成一条端到端纵向链路，不能只新增协议或底层 graph 子步骤。

## 2. 交付假设

- 初期是单用户、本地 daemon、一个 Space 可包含多个本地 Git 项目；
- Go、Rust、Node/Python Harness 继续共存，不做大爆炸重写；
- 初期只接入两个 Agent Adapter：Codex CLI 与 Claude Code；
- 不启用远程 Runner、生产部署、云端多租户和自动策略晋升；
- 工作量使用 `S/M/L/XL` 表示相对复杂度，不构成日期承诺；
- 每个包都必须能独立回滚，并保持旧 CLI/Runtime 协议兼容窗口。

经[十轮对抗式分析](adversarial-analysis-10-rounds.md)校准，当前只建议批准到 R0 可观测交付垂直切片。Platform Core v1 首期限定为 common event、Runtime command/action event、ArtifactRef 和两类 Receipt 等六个直接服务 R0 的 wire family；WorkGraph 自动推进、跨项目图谱和 Evolution 必须分别通过后续 Go/No-Go Gate。

## 3. 工作流与依赖

```text
F0 边界冻结
 ├── F1 Platform Core contracts
 │    ├── F2 Runtime Action journal + Agent adapter
 │    │    └── F3 App Server ingestion + Timeline projection
 │    │         └── F4 CLI/TUI/App observable vertical slice
 │    └── F5 Objective/Change/WorkGraph domain
 │         └── F6 Reconciler + Approval + Verification
 │              └── F7 multi-work-item automatic progression
 ├── F8 Harness scope convergence
 └── F9 Space graph extractors
      └── F10 Evolution proposal and shadow evaluation
```

关键路径：`F0 → F1 → F2 → F3 → F4 → F5 → F6 → F7`。`F8` 可在 F1 后并行；F9/F10 不得阻塞首个可观测交付闭环。

## 4. Work Packages

### F0 — 责任边界冻结（S）

交付：

- Go/Rust/Harness/Platform Core owner matrix；
- 当前 CLI、数据库、事件、合同和 Harness 模块 inventory；
- 暂停新增顶层 graph protocol CLI 和普通三语言手写合同的规则；
- 兼容策略：旧命令保留、内部标记、迁移期 deprecation policy。

验收：一个 WorkItem 的选择、执行、记录、验证和完成分别只有一个 owner；任何新模块都能映射到 owner。

### F1 — Canonical Platform Core v1（L）

交付：

- ID 与关联模型：Space/Project/Snapshot/Objective/Change/WorkItem/Attempt/Session/Turn/Action；
- command envelope、durable event envelope、ArtifactRef、ExecutionReceipt、VerificationReceipt；
- WorkItem/Attempt/Action 最小状态机；
- schema compatibility、canonical fixture、Go/Rust binding 和 Harness conformance；
- 大 payload → ArtifactRef、敏感字段 redaction 和 retention 分类。

验收：相同 fixture 在 Go/Rust/Harness 中产生相同 canonical digest；未知版本和未知字段按协议策略失败；schema 有大小和数量上限。

### F2 — Runtime 可观测执行（XL）

交付：

- Runtime 内的 Session/Turn/Action 聚合与 append-only journal；
- Codex CLI、Claude Code Adapter 的统一事件映射；
- Tool request/approval/start/chunk/finish、文件、命令、成本和 terminal 事件；
- Runtime 本地 RPC/stdio 协议：StartAttempt、ApproveAction、Interrupt、GetEvents；
- CAS 与 ArtifactRef；
- uncertain effect、crash、resume 和不可重发边界。

验收：deterministic fake adapter 与至少一个真实本地 CLI fixture 能产生可重放 timeline；断电/进程终止后不丢 durable terminal boundary；不解析 stdout 冒充不可见内部动作。

### F3 — App Server 与查询投影（L）

交付：

- Go `forge-server` 生命周期、单实例锁和健康检查；
- `control.db`、runtime command client、event inbox/cursor；
- Timeline、Run summary、cost、artifact、approval read model；
- Command API、Query API、SSE/WebSocket 或本地等价订阅；
- 客户端断线重连和 cursor replay。

验收：CLI/TUI/App 不读 SQLite；重复 Runtime event 幂等；Projection 删除后可从 durable source 重建；服务重启后 cursor 连续。

### F4 — 首个用户垂直闭环（L）

交付：

- 创建 Space、注册 Project、创建 Objective；
- 将 Objective 临时映射为单 WorkItem；
- 选择 Codex/Claude、预览权限、启动/暂停/终止；
- CLI/TUI/App Run Timeline；
- Harness VerificationReceipt；
- Outcome 页面和失败/不确定状态说明。

验收：用户不需要 graph-node 命令即可完成一次受控代码任务；能看到每个已暴露 Action、文件 diff、命令、费用、Artifact 和验证结果；不能看到的宿主内部状态明确标注。

### F5 — Delivery Domain（L）

交付：

- Objective、AcceptanceCriteria、Change、ImpactAssessment、WorkGraph、WorkItem；
- desired/observed state 分离；
- WorkGraph DAG validation、Snapshot binding、Unknown 和 blocker；
- Change Cockpit projection；
- 旧 workflow/graph plan 到 WorkGraph 的兼容适配器。

验收：WorkGraph 无环、所有 work item 有验收和 snapshot；Run terminal 不能直接完成 Change；变更快照漂移使执行回到 blocked/replan。

### F6 — Reconciler、策略与独立验证（XL）

交付：

- 确定性 ready-node selector；
- Policy/Approval/Budget/Freshness precondition；
- 幂等 dispatch、Attempt claim、terminal reconcile；
- Harness verification request/receipt；
- blocked、failed、uncertain、replan 和 manual override；
- 决策原因进入 timeline。

验收：相同 observed state 产生相同 next decision；无审批、过期快照、预算不足和 uncertain predecessor 全部 fail closed；Agent 不直接写 WorkItem 状态。

### F7 — 多任务自动推进（XL）

交付：

- 串并行 wave、资源/路径冲突和最大并发；
- predecessor Artifact/Receipt disclosure；
- retry policy、checkpoint、resume 和 integration queue；
- 全 Change acceptance join；
- 用户可配置 autonomy level。

验收：多节点崩溃恢复不重复不安全 effect；并行写冲突被阻止或显式整合；只有所有必需验收满足时 Change 才 completed。

### F8 — Harness 收敛（L，持续）

交付：

- 模块分类：Keep/Move/Generate/Deprecate；
- canonical fixture/conformance runner；
- VerificationRequest/Receipt；
- scaffold 从 Harness 迁入 `tools/` 的兼容计划；
- 产品合同 evaluator 回归唯一 owner。

验收：Harness 可以在产品损坏时独立运行；不读取 Control/Runtime 私有数据库；不推进状态或授予产品 authority。

### F9 — Space 跨项目图谱（XL）

交付：

- Project registry、ProjectSnapshot、stable node ID；
- manifest/import、API/event、DB migration、deployment、test、ADR/owner extractor；
- Graph projection、coverage/freshness/unresolved；
- impact path query 和 Graph UI/TUI；
- inferred edge 单独存储 confidence/evidence/expiry。

验收：所有事实边可追到 snapshot/extractor；推断边不会参与硬 gate，除非经独立验证升级；图谱可删除重建。

### F10 — 受控 Evolution（L）

交付：

- Outcome/Failure/Intervention observation；
- EvolutionProposal、baseline、target metric；
- historical replay 和 shadow evaluation；
- proposal review、promotion/rollback record；
- Evolution Queue。

验收：系统只能自动提案和执行无 effect 的影子评估；真实策略晋升需要独立审批；Harness/PDP 不可由被评估 Agent 修改后自验。

## 5. 垂直发布切片

| Release slice | 用户价值 | 包含 | 不包含 |
|---|---|---|---|
| R0 Developer Preview | 可看清一次 Agent 执行 | F0–F4 最小子集 | 自动多任务、跨项目图谱 |
| R1 Controlled Delivery | 从目标推进到受控验收 | F5–F6 | 无人值守多波次 |
| R2 Autonomous Local | 本地多任务自动推进 | F7 + F8 | 远程/多租户 |
| R3 Workspace Intelligence | 跨项目影响分析 | F9 | 自动事实晋升 |
| R4 Learning Control Plane | 受控改进策略 | F10 | 无审批自修改安全边界 |

## 6. Definition of Done

每个 work package 必须同时满足：

- 用户旅程和错误旅程已演示；
- public API/schema 版本化并有兼容测试；
- 当前源码 snapshot、测试、Artifact 和 Receipt 绑定；
- CLI/TUI/App 读取同一 projection；
- 日志、成本、取消、超时、恢复和安全边界有测试；
- 文档不把 fixture、structure valid 或 shadow 结果写成真实 authority；
- 独立 Review 和完整 `forge accept` 通过。

## 7. 暂停条件

出现以下情况必须停止自动推进并回到设计或人工裁决：

- Go 与 Rust 对状态或 owner 的解释不一致；
- 无法重放或解释某个用户可见状态；
- Adapter 不能可靠区分失败与不确定 effect；
- Artifact/Receipt 无法绑定当前 Snapshot；
- Harness 需要读取产品私有状态才能得出 Verdict；
- 图谱 coverage 不足却被要求给出确定的跨项目影响结论；
- 新增协议工作没有对应的用户闭环或风险关闭目标。
