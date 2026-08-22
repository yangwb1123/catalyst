# Forge Agent Delivery Control Plane — 产品与目标架构蓝图

> 状态：**Proposed / 目标态蓝图，非已实现声明**
> 日期：2026-08-21
> 范围：`forge-core`、`forge-runtime`、`harness`，以及其上的 CLI、TUI、App 产品层
> 约束：本文不授予运行、审批、知识写入或完成裁决权限；现状以代码、测试、正式 ADR 和 `forge accept` 为准。

## 1. 执行摘要

Forge 当前不是无序堆砌，而是一个“可信执行与合同治理强、产品控制面弱”的工程内核。三块代码的技术方向基本成立：

- `forge-core` 已具备编排、路由、预算、审批、检查点和治理接线基础；
- `forge-runtime` 已具备本地持久执行、Session/Run、SQLite journal、Agent Loop 和 Artifact 基础；
- `harness` 建立了宿主无关、带外验证的正确原则。

但当前代码组织还不能直接支撑目标产品。主要问题不是 Go、Rust 或 Node/Python 的语言选择，而是所有权不清：Go 与 Rust 都在表达图调度和生命周期；Go checkpoint/trace/memory、Rust journal 与 Harness receipt 分散了运行事实；Harness 又逐渐承载产品领域合同和脚手架业务。

建议将 Forge 明确分成两层：

1. **ForgeOS Platform**：继续作为 Agent 之上的可信工程控制与执行平台；
2. **Forge Workspace（工作名）**：在 Platform 之上提供 CLI、TUI、App，面向用户交付“目标到验收”的产品闭环。

目标架构应采用本地优先的模块化单体：Go 是唯一产品控制面，Rust 是执行面，Harness 是外部验证面；三者通过版本化命令、事件、Artifact 与 Receipt 协作。现阶段不需要微服务化，也不应重写为单一语言。

## 2. 产品定义

### 2.1 产品命题

Forge Workspace 是位于 Codex、Claude Code 等编码 Agent 之上的自治软件交付控制塔。它把用户目标转换为可验证的项目变更，完整呈现 Agent 的可观测行为，在策略和预算约束下自动推进工作，并在一个 Space 内理解多个项目之间的关联。

它交付的不是“一次成功的 Agent 对话”，而是一个可验收的 Outcome：

```text
用户目标
  → 需求与约束
  → 影响分析
  → Change 与 WorkGraph
  → Agent 执行
  → 独立验证
  → Artifact / Receipt
  → Outcome
  → 反馈与受控改进
```

### 2.2 目标用户

- 同时维护一个或多个代码库的个人开发者、技术负责人；
- 需要让 Agent 长时间执行，但不能接受黑盒运行的团队；
- 需要理解跨仓库影响、架构关系、变更风险和完成证据的项目负责人；
- 后续阶段的工程平台团队和多项目产品组织。

### 2.3 核心用户任务

用户需要能够：

1. 在一个 Space 中注册、分组并理解多个项目；
2. 用业务目标而不是底层 graph/node 命令发起工作；
3. 查看系统如何理解目标、影响、未知项和验收标准；
4. 实时看到 Agent 使用了什么工具、修改了什么、产生了什么证据；
5. 在高风险动作前审批、限制、暂停、恢复或终止；
6. 知道为什么推进、为什么阻塞、为什么完成；
7. 让系统基于历史 Outcome 改进路由、上下文、工作流和验证策略；
8. 分析一个变更对其他项目、API、数据、部署和 Owner 的影响。

### 2.4 非目标

首个产品阶段不应承诺：

- 替代 Codex、Claude Code 等 Agent 宿主；
- 展示模型未公开的内部状态或原始隐藏思维链；
- 在无人授权下自动部署生产、读取生产凭证或修改安全策略；
- 一开始就提供云端多租户、HA、远程 Runner 和企业 IAM；
- 使用 LLM 推断替代确定性代码、合同、测试和部署关系；
- 让系统直接修改自己的 Harness、PDP 或审批边界后自行宣布有效。

## 3. 当前架构评估

### 3.1 总体结论

| 维度 | 当前判断 | 面向目标产品的缺口 |
|---|---|---|
| Agent 执行 | 基础较强 | 缺统一的多宿主 Session/Turn/Action 协议 |
| 持久恢复 | 局部较强 | Go 文件状态和 Rust journal 尚未形成统一用户视图 |
| 治理与验证 | 很强 | 合同数量增长快，Harness 范围过大 |
| 全过程可观测 | 部分具备 | 不是每个 Action 都进入统一、可回放事件流 |
| 自动推进 | 局部具备 | 缺顶层 Objective/Change/WorkGraph Reconciler |
| 跨项目分析 | 早期基础 | 缺稳定跨源身份、完整关系图和影响闭包 |
| 自我进化 | 初级 | 主要是文本记忆和提案，缺实验、评估、晋升、回滚 |
| CLI/TUI/App 产品 | 尚未闭环 | 缺统一 App Server、查询投影和结果导向的信息架构 |

### 3.2 `forge-core`

当前优势：

- 已形成串行/并行工作流、预算、重试、loopback、checkpoint 和 gate 接线；
- 适合继续承担策略、调度、路由和产品应用服务；
- Go 对本地 daemon、CLI、TUI 和控制循环较合适。

当前问题：

- CLI 同时暴露产品命令与大量 graph/node/protocol 细节，见 [`cli_dispatch.go`](../../../forge-core/cmd/forge/cli_dispatch.go)；
- Orchestrator 通过大量 callback 组装行为，难以形成稳定应用服务、事件边界和测试替身，见 [`orchestrator.go`](../../../forge-core/internal/orchestrator/orchestrator.go)；
- Agent executor 以 `Execute(...) error` 为主，表达不了 Session、Turn、Action、Artifact 和 Receipt，见 [`executor.go`](../../../forge-core/internal/orchestrator/executor.go)；
- checkpoint、trace 和 memory 分别持久化，适合作为过渡实现，不适合作为 App 的统一事实查询面；
- 当前命令适配仍带有宿主特定逻辑，不是完整的异构 Agent Adapter 层。

目标定位：`forge-core` 成为唯一的 Delivery Control Plane，只决定“应该做什么、何时做、是否允许做”，不直接拥有 Agent 工具循环。

### 3.3 `forge-runtime`

当前优势：

- Domain/Application/Infrastructure/Interface 的依赖方向总体合理；
- SQLite journal、Session/Run、provider、tool loop、artifact 和恢复语义具有长期价值；
- 对 claim、uncertainty、exact replay、content-addressed request/receipt 的谨慎设计符合可信执行目标；
- Global/Project/Group Hub 可作为 Space 项目目录和会话聚合的基础。

当前问题：

- domain 公共导出面和 Group Agent Graph 协议面持续增大，见 [`domain/src/lib.rs`](../../../forge-runtime/crates/domain/src/lib.rs)；
- Runtime 中存在 scheduling、wave、node lifecycle，与 Go 控制面的顶层编排职责重叠；
- 当前事件能描述运行和工具开始/结束，但还不是产品级统一事件信封，见 [`event.rs`](../../../forge-runtime/crates/domain/src/event.rs)；
- SQLite schema 的持续扩张反映了“按协议切片积累”强于“按产品聚合根收敛”；
- Hub 的 Global/Group 是目录和对话视图，不等于跨项目语义知识图谱。

目标定位：`forge-runtime` 成为 Execution Plane。它执行一个已授权 WorkItem Attempt，捕获 Action，管理 Sandbox/Tool/Provider，并生成不可抵赖的执行事件和 Receipt；它不决定整个项目下一步执行哪个节点。

### 3.4 `harness`

当前优势：

- 宿主无关、带外执法是正确的载重墙；
- acceptance、架构规则、安全扫描和跨语言 conformance 能防止产品代码自证通过；
- Go 只调用 Harness 而不重复 gate 逻辑的原则正确，见 [`gate.go`](../../../forge-core/internal/gate/gate.go)。

当前问题：

- Harness 已同时承载 gate、合同领域实现、治理工具、工程知识、scaffold 和升级逻辑；
- 普通产品合同也出现 Go/Rust/Python 多套手写实现，维护成本和错误面持续增长；
- 验证层正在变成第二套产品内核，使“谁拥有业务语义”难以回答；
- 文件行数和合同数量等治理指标开始反向塑造代码结构，阈值无法替代模块内聚性判断。

目标定位：Harness 只拥有 Acceptance、Security、Architecture 和 Conformance。普通领域规则归 Core/Runtime 的唯一 owner；跨语言使用 canonical schema、golden vectors 和生成绑定。只有安全、身份、授权等信任边界保留真正独立的多实现验证。

## 4. 目标产品模型

### 4.1 核心聚合关系

```text
Space
├── Project
│   ├── ProjectSnapshot
│   └── KnowledgeNode / KnowledgeEdge
├── Objective
│   └── Change
│       ├── AcceptanceCriteria
│       ├── ImpactAssessment
│       └── WorkGraph
│           └── WorkItem
│               └── Attempt
│                   └── AgentSession
│                       └── Turn
│                           └── Action
├── Policy / Budget / Approval
└── EvolutionProposal / Experiment

Action → ArtifactRef → ExecutionReceipt / VerificationReceipt → Outcome
```

### 4.2 关键语义

- **Objective**：用户希望系统实现的结果，不是 prompt 文本；
- **Change**：对一个或多个 ProjectSnapshot 的受控变更单元；
- **WorkGraph**：为了完成 Change 需要执行和验收的依赖图；
- **WorkItem**：可授权、可执行、可单独验收的最小工作单元；
- **Attempt**：WorkItem 的一次执行，失败或不确定时不覆盖历史；
- **Outcome**：AcceptanceCriteria 在特定 Snapshot 上被证据支持后的结果；
- **Unknown**：必须显式管理的未知项，不允许被默认值静默吞掉。

“完成”必须由当前 Snapshot、AcceptanceCriteria、Artifact 和 VerificationReceipt 共同支持，不能由 Agent 的 success 文本或 Run terminal 状态单独推导。

## 5. 目标逻辑架构

```text
                 CLI / TUI / Web App / Desktop
                              │
                   Query API + Command API
                       Durable Event Stream
                              │
┌──────────────────────────────────────────────────────────────┐
│ Forge App Server / Control Plane — Go                        │
│                                                              │
│ Workspace Catalog │ Objective/Change │ WorkGraph/Reconciler  │
│ Policy/Approval   │ Router/Budget    │ Knowledge/Projection  │
└──────────────────────────────┬───────────────────────────────┘
                               │ versioned command
                               ▼
┌──────────────────────────────────────────────────────────────┐
│ Forge Runtime / Execution Plane — Rust                       │
│                                                              │
│ Agent Adapters │ Session/Turn/Action │ Tool/Sandbox/Process  │
│ Runtime Journal│ Artifact/CAS        │ Execution Receipt     │
└──────────────────────────────┬───────────────────────────────┘
                               │ immutable evidence
                               ▼
┌──────────────────────────────────────────────────────────────┐
│ Harness / Independent Verification                           │
│ Acceptance │ Security │ Architecture │ Cross-language Contract│
└──────────────────────────────┬───────────────────────────────┘
                               │ verdict receipt
                               └──────────────→ Control Plane
```

### 5.1 所有权规则

| 能力 | 唯一 owner | 明确禁止 |
|---|---|---|
| 选择下一 WorkItem | Go Control Plane | Rust/Harness 不推进顶层 WorkGraph |
| 风险、预算、审批策略 | Go Policy/Kernel | Agent 不自授予权限 |
| Agent Session/Turn/Tool Loop | Rust Runtime | Go 不解析 stdout 模拟完整执行语义 |
| Tool、进程、网络、Sandbox | Rust Runtime | UI 不直接调用执行器 |
| Acceptance 与独立 Gate | Harness | 产品代码不自证通过 |
| Artifact bytes 与 content identity | Runtime CAS | Event/SQLite 不内联无限大正文 |
| 产品查询视图 | Go Projection | CLI/TUI/App 不直接查询 Runtime DB |
| 知识事实采用 | 独立 Knowledge authority | Agent 只可 propose，不能自行确认 truth |

### 5.2 部署形态

首个产品版本采用本地 daemon 模式：

- 一个 Go `forge-server` 对 CLI、TUI、App 提供统一 API；
- Rust Runtime 作为受控子进程或本地服务运行；
- 双方使用版本化本地协议，不共享内存和数据库表；
- App 通过 SSE/WebSocket 或本地等价流订阅事件；
- 未来迁移到远程服务时保持同一命令和事件语义。

这条路径与目标 HA 架构兼容，但不要求当前引入 Temporal、NATS、Postgres 或微服务。

## 6. 可观测执行模型

### 6.1 统一事件信封

所有 durable semantic event 至少包含：

```text
event_id              全局唯一
schema_version        事件协议版本
space_id              产品空间
project_id            项目；可空但必须说明适用范围
objective_id/change_id/work_item_id
attempt_id/run_id/session_id/turn_id
actor_id               Agent、用户、Kernel、Harness 或系统
sequence               聚合内单调序号
kind                   稳定事件类型
occurred_at            发生时间
correlation_id         跨组件关联
causation_id           直接前因
source_snapshot_ref    观察或执行所绑定的项目快照
payload | artifact_ref 有界 payload 或内容引用
```

### 6.2 需要观测的 Action

- Session/Turn 开始、结束、中断和恢复；
- 模型请求元数据、模型响应摘要、token、成本和耗时；
- 计划、决策、假设、Unknown 和依据摘要；
- Tool 请求、审批、启动、输出块、结束和失败；
- 文件读取、写入、diff、路径范围和内容摘要；
- 命令、进程、网络目标和 sandbox decision；
- Artifact 创建、验证和引用；
- Gate、Reviewer、QA、Approval 和 Verdict；
- WorkItem 与 Change 的状态迁移；
- retry、cancellation、timeout、crash 和 uncertain effect。

事件必须在 Agent Adapter、Tool Runtime 和 Sandbox 边界捕获，而不是事后从 stdout 推断。外部 Agent 不暴露的内部状态不能被承诺为可观测；产品展示计划和决策摘要，不存储或展示原始隐藏思维链。

### 6.3 Durable 与 Live 分离

- Durable event：状态变化、动作边界、Artifact、Receipt、审批和成本，可按 cursor 重放；
- Live signal：token delta、stdout chunk、临时进度，可限流、采样或过期；
- UI 断线重连先按 durable cursor 补齐，再恢复 live stream；
- 任何业务状态不得只依赖可能丢失的 live signal。

## 7. 自动推进与状态机

自动推进由确定性的 Reconciler 驱动，而不是由 LLM 无限递归调用自己：

```text
Desired State: Objective + Change + WorkGraph
      │
      ▼
Select ready WorkItem
      │
Policy / Approval / Budget / Snapshot freshness
      │
      ▼
Runtime executes one Attempt
      │
      ▼
Evidence + Harness verification
      │
      ▼
Reconcile observed state
      ├── next ready item
      ├── awaiting approval
      ├── blocked / replan
      ├── failed / uncertain
      └── outcome accepted
```

建议状态集合：

```text
proposed → planned → awaiting_approval → ready → running → verifying
                                                      ├→ completed
                                                      ├→ blocked
                                                      ├→ failed
                                                      └→ uncertain
```

只有以下条件同时满足时才允许自动推进：

- 依赖 WorkItem 已在相同或兼容 Snapshot 上验收；
- 授权、审批、预算和风险策略仍有效；
- 没有阻断级 Unknown 或 finding；
- 当前输出 Artifact 可读取且 digest 匹配；
- VerificationReceipt 新鲜并绑定当前输入和执行身份；
- 上一次 effect 不是未裁决的 uncertain 状态。

## 8. SQLite Journal 与 Artifact Identity

### 8.1 决策

SQLite journal 与 content-addressed Artifact 是合理且必要的本地优先基础，应保留。需要解决的是所有权和查询边界，而不是移除 SQLite。

建议按进程和领域拥有两个逻辑存储：

- `control.db`：Space、Project、Objective、Change、WorkGraph、Policy、Projection、Outbox/Inbox；
- `runtime.db`：Attempt、Session、Turn、Action、ToolApproval、RuntimeEvent、ExecutionReceipt、Artifact metadata。

约束：

- 每个数据库只有一个写入 owner；
- Go 不直接读取 Runtime 表，Rust 不直接读取 Control 表；
- 跨边界使用幂等命令、outbox/inbox 和 durable event；
- Journal 保存有界事实，Projection 可删除后重建；
- 使用 WAL、事务、schema migration、聚合 sequence 和唯一幂等键；
- 大正文、模型输出、日志、diff 和测试报告进入 CAS，不进入事件行。

### 8.2 Artifact 的双重身份

Artifact 应区分：

- `content_id`：对原始 bytes 计算的内容哈希，回答“内容是什么”；
- `logical_id`：在 Space/Project/Change/Run 中的语义身份，回答“它承担什么角色”。

最小 `ArtifactRef` 包含：

```text
content_id / digest_algorithm
logical_id / artifact_kind / media_type
size_bytes
producer / attempt_id
source_snapshot_ref
created_at
provenance_ref
retention_class
```

ExecutionReceipt 与 VerificationReceipt 必须绑定输入 Snapshot、执行器及版本、Policy/Approval、实际 effect、结果状态和 Artifact digest。Artifact 的存在不等于结果正确；Receipt 的结构有效也不自动等于 Outcome 被接受。

## 9. Space 与跨项目知识图谱

### 9.1 模型

建议支持以下节点：

```text
Space / Project / Repository / Snapshot
Module / Package / API / Event / Database
Deployment / Environment / Owner / ADR / Test
Change / Artifact / RuntimeService
```

建议支持以下关系：

```text
contains / depends_on / calls / publishes / consumes
reads / writes / deploys_with / owned_by / governed_by
verified_by / changed_by / supersedes
```

### 9.2 事实与推断边界

- Git、manifest、lockfile、编译图、OpenAPI/AsyncAPI、migration、deployment config 等确定性 extractor 产生 observed edge；
- LLM 只能产生 inferred edge，必须携带 evidence、confidence、scope 和 expiry；
- 每个节点和关系绑定 extractor version、ProjectSnapshot、coverage、freshness 和 unresolved reason；
- 图谱是可重建 Projection，不是无法追溯来源的第二事实库；
- 未解析引用必须显示为 Unknown，不能静默丢弃。

首期使用 SQLite 关系表、递归查询和全文索引即可。只有跨项目规模和查询负载证明必要时，才引入专用图数据库。

## 10. 受控自我进化

自我进化的对象应是可版本化、可评估、可回滚的策略资产，而不是系统任意修改自身代码。

```text
Observation
  → Diagnosis
  → EvolutionProposal
  → Historical Replay / Shadow Evaluation
  → Bounded Sandbox Experiment
  → Independent Review / Approval
  → Promotion
  → Monitoring / Rollback
```

成熟度分级：

| 级别 | 能力 | 默认权限 |
|---|---|---|
| E0 | 记录运行、失败、成本和人工干预 | 自动 |
| E1 | 聚类问题并提出改进建议 | 自动提案 |
| E2 | 对历史任务做影子重放 | 只读、无真实 effect |
| E3 | 在沙箱对低风险任务做小流量实验 | 显式策略授权 |
| E4 | 晋升路由、Prompt、Context 或 Workflow 版本 | 独立审批 |

Harness 核心、安全策略、Approval/PDP 和完成标准不得由被评估的 Agent 自行修改并生效。学习指标必须基于 accepted Outcome，而不是 token 数、Agent 自评分或 Run terminal 数。

## 11. CLI、TUI 与 App 产品结构

三种客户端应共享同一产品语义和 API，仅交互密度不同。

### 11.1 核心界面

- **Space Overview**：项目健康、活动 Change、跨项目关系、阻塞项、预算；
- **Change Cockpit**：目标、验收标准、影响、计划、审批、风险和当前状态；
- **Run Timeline**：Session/Turn/Action、Tool、文件 diff、命令、成本、Artifact 和 Receipt；
- **Knowledge Graph**：节点、边、来源、覆盖率、新鲜度、Unknown 和影响路径；
- **Evolution Queue**：问题、提案、实验、指标、审批、晋升和回滚；
- **Policy & Budget**：自治等级、允许的 effect、路径、网络、模型和费用上限。

### 11.2 CLI 信息架构

稳定产品命令围绕用户对象组织：

```text
forge space ...
forge project ...
forge objective ...
forge change ...
forge run ...
forge inspect ...
forge approve|reject ...
```

graph node handshake、dispatch authorization、低层 contract dump 等命令迁入 `forge debug`、`forge protocol` 或仅内部 API，不再与产品命令处于同一层级。

## 12. 落地与文档关系

代码组织、分阶段路线、产品指标、风险控制和待决 ADR 见配套的
[实施与迁移计划](product-control-plane-delivery-plan.md)。该文档负责“如何从现状迁移”，本文只负责产品定义、边界和目标结构。
更细的系统接口、交互流程和分组件 work package 见
[Forge Workspace 设计与实施包](../forge-workspace/README.md)，其范围已经过[十轮产品与架构对抗式分析](../forge-workspace/adversarial-analysis-10-rounds.md)校准。

- [README.md](README.md) 描述 AI Engineering OS 的生命周期、能力和治理模型；
- [implementation-roadmap.md](implementation-roadmap.md) 记录当前工程切片、真实完成度和治理缺口；
- [delivery-sop.md](delivery-sop.md) 继续定义规划、实现、验证和发布的质量合同；
- [North-Star Architecture](../../../.agent/architecture/north-star.md) 描述远期分布式 HA 平台；
- 本文补充此前缺失的本地优先产品表面、三大代码域所有权、统一事件模型和从 Objective 到 Outcome 的迁移路径。

若本文与当前代码、测试、正式 ADR 或 `forge accept` 结果冲突，应以当前事实为准，并将冲突登记为待决架构项，而不是把目标态描述为已交付能力。
