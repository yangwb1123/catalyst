# AI Engineering OS：能力中心化的软件工程组织蓝图

> 状态：**目标设计仍以规划为主；首个 contract/shadow 治理切片已实现**。决策见
> [ADR-0037](../../adr/0037-capability-centric-ai-engineering-operating-model.md)、
> [ADR-0038](../../adr/0038-aadm-decision-kernel-and-meta-reflection.md)、
> [ADR-0039](../../adr/0039-default-off-device-aware-execution-fabric.md)、
> [ADR-0040](../../adr/0040-machine-readable-agent-engineering-governance.md)、
> [ADR-0041](../../adr/0041-backend-decision-contract-and-persistence-gate.md)、
> [ADR-0042](../../adr/0042-frontend-design-decision-contract.md)；当前覆盖与分期见
> [implementation-roadmap.md](implementation-roadmap.md)。运行时代码、测试和现有 `.agent/` 契约仍是当前事实源。

当前已交付的窄切片位于 `.agent/engineering/`：activation、14 学科状态、原子规则、`forge accept` detector 接线、
typed Context 路由、W0–W3 保障覆盖层和 TaskEvidencePackage 契约由 `harness/check.py` 机器校验，并随 scaffold/legacy
upgrade 继承。其 `activation: shadow` 只强制合同和接线完整性；Context selector runtime、Capability invocation
Registry、AADM solver、R0–R2 Reflection 和 Device Fabric 仍未实现。TaskEvidencePackage 不含完成判定。

ADR-0041 又交付了一个窄的后端 shadow 切片：14 维 BackendDecisionPolicy、无裁决权 BackendDecisionPackage、十张密集
Skill adapter、data/backend Context route 和独立对抗 validator。它把持久化与低可逆决策前置，但不会从 diff 自动生成
package，也尚未成为 coding 前的 runtime gate；详细标准见 [backend-decision-standard.md](backend-decision-standard.md)。

ADR-0042 交付了前端设计 shadow slice：[AFDS](frontend-design-standard.md)、byte-pinned policy、Profile/Pattern catalog、
FrontendDesignPackage schema 与三张沿用既有 capability ownership 的 canonical Skill adapter。分类、IA、flow/state/action/permission、
Design System、跨平台映射与多源证据已接入 Context route、shadow detector、对抗 validator、scaffold 和 legacy upgrade；它仍没有
自动 diff compiler、可信视觉服务、pre-code runtime authority 或完成权威。

## 1. 目标

ForgeOS 要解决的不是“让一个 Agent 更快写代码”，而是让一组受治理的 Agent 像长期维护系统的资深团队一样：

- 知道系统当前是什么、为什么如此、哪些地方不能动；
- 在动手前说明直接与传递影响、变化成本、风险、未知项；
- 把事实、决策、推理、假设和提案分开保存；
- 以最小权限、职责分离和 fresh-context Review 工作；
- 用测试、门禁、运行数据和用户反馈驱动持续演化；
- 维护架构决策、技术债、工程宪法和健康趋势，而不是只维护代码。

## 2. 核心模型：节点 × 能力 × 权限，不按 Agent 数量建组织

```text
Work Intent
   │
   ▼
Lifecycle Decision Node ──选择──▶ Capability Set
   │                                  │
   ├──装配 ContextPackage             ├──规则 / 模板 / 检测器
   ├──提交 GrantRequest               └──输入 / 输出 / 验收契约
   └──非 Agent Kernel/PDP 签发 CapabilityGrant 后分配临时 Agent Instance
                                      │
                                      ▼
                           Evidence-backed Artifact
```

四个概念必须分开：

| 概念 | 定义 | 例子 |
|---|---|---|
| 生命周期节点 | 一个需要作出明确决策的流程关口 | 需求基线、架构批准、发布 Go/No-Go |
| 角色 | 独立责任与审查视角，不天然拥有执行权 | 产品经理、安全工程师、DBA、SRE |
| 能力/Skill | 可复用的做事方法、规则、模板和检测器 | 影响分析、威胁建模、数据库迁移、重构 |
| Agent 实例 | 某次 Run 中临时绑定角色、能力、上下文和权限的执行者 | `reviewer + code-review + read-only` |

因此，不为每个名词永久创建一个 Agent。一个角色可以装配多个能力；同一能力可被多个节点复用；权限由本次
`CapabilityGrant` 决定，不由角色名称暗示。

## 3. 生命周期节点

| ID | 决策节点 | 主责结果 | 详细 SOP |
|---:|---|---|---|
| 00 | Intake & Orchestration | 基线/ImpactPreScan/DAG/GrantRequest；设计后 final Assessment Join | [discovery-design-sop.md](discovery-design-sop.md) |
| 01 | Requirement / BA | 可追踪需求、业务规则、边界与验收标准 | 同上 |
| 02 | Product | 产品范围、用户价值、指标、状态机与发布策略 | 同上 |
| 03 | UX/UI | 研究证据、信息架构、交互、视觉与无障碍规范 | 同上 |
| 04 | Domain | 统一语言、上下文、聚合、不变量、命令与事件 | 同上 |
| 05 | Architecture | 当前/目标架构、选项权衡、ADR 与演进触发器 | 同上 |
| 06 | Data | 数据所有权、模型、事务、索引、迁移与生命周期 | 同上 |
| 07 | API / Integration | 契约、兼容性、幂等、错误、事件与消费方约束 | 同上 |
| 08 | Planning | 垂直增量、依赖 DAG、成本/风险与 Definition of Done | [delivery-sop.md](delivery-sop.md) |
| 09 | Development | 最小兼容实现、测试、遥测、迁移与工程证据 | 同上 |
| 10 | Review / Refactoring | 独立 findings、重构方案、复审 verdict | 同上 |
| 11 | Security / Privacy / Compliance | 威胁模型、控制矩阵、残余风险与签核条件 | 同上 |
| 12 | QA / Verification | 风险驱动测试、需求追踪、缺陷与验收证据 | 同上 |
| 13 | Performance / Reliability | SLO、预算、基准、容量与故障恢复证据 | 同上 |
| 14 | Release / Change | 可审计交付包、迁移顺序、Go/No-Go、回滚 | 同上 |
| 15 | Operations / SRE | 可观测性、告警、事件响应、恢复和运营反馈 | [operations-evolution-sop.md](operations-evolution-sop.md) |
| 16 | Reflection & Evolution | R0–R2 纠查、健康/债务/漂移、受控学习与下一轮提案 | 同上 |

节点不是固定串行瀑布。00 先构造 DAG；03–07 可按依赖并行；10–13 在实现后独立并行审查；安全、测试、
可运维性同时作为每个更早节点的横切约束。14 只能消费已解决的阻断 finding，16 只能提出下一轮工作，不能
借“自我优化”取得实现或生产权限。

## 4. 每个节点统一的 14 段契约

所有节点和未来 Skill 必须回答同一组问题：

1. `purpose`：本节点负责作出什么决策；
2. `entry_criteria`：什么证据齐备后才可进入；
3. `inputs`：消费哪些版本化产物与系统快照；
4. `activities`：必须执行的细项职能；
5. `capabilities`：需要装配哪些可复用 Skill；
6. `outputs`：产生哪些有 schema 的产物；
7. `rules`：必须遵守的业务、架构和工程规则；
8. `quality_gates`：哪些检查可机器执行，哪些需独立判断；
9. `forbidden`：本节点明确不能做什么；
10. `authority`：允许的 effect、路径、环境、工具、预算和时限；
11. `escalation`：哪些未知、冲突或风险必须升级；
12. `exit_criteria`：什么条件可判定完成，不能以轮数代替；
13. `handoff`：向哪些下游节点交付什么；
14. `memory_updates`：写回哪些事实、决策、债务、证据和运行反馈。

机读规划目录见 [capability-catalog.v1.yml](capability-catalog.v1.yml)。当前标记为 `planning_only`，在 schema、
validator、runtime adapter 和测试完成前，不能把它描述为已执行的工作流。

## 5. 三个治理平面

### Epistemic Plane：知道“我们究竟知道什么”

- `FACT` 必须绑定可复验 Evidence、代码/数据版本和新鲜度；
- `DECISION` 必须指向已批准 ADR 或等价记录；
- `INFERENCE` 必须列出前提、推理和置信度；
- `ASSUMPTION` 必须有 owner、到期时间、验证计划和“若为假”的影响；
- `PROPOSAL` 不得被下游当成已采纳范围；
- `UNKNOWN` 必须进入问题或风险队列，不能被默认值静默吞掉。

### Governance Plane：决定“谁可以做什么”

默认拒绝；授权绑定 `run/change/node/agent/capability/path/environment/tool/effect/budget/expiry`。规划、实现、
审查、批准和生产操作分权；Agent 不能自授予权限、自审自己的变更或把一次 Review 冒充批准。

### Feedback Plane：判断“系统是否在变好”

带外 gate、fresh reviewer、QA、生产 SLI、事件、用户反馈、技术债和健康趋势共同形成反馈。健康分是趋势信号，
不能覆盖 Critical finding、缺失证据或发布闸门。

## 6. AADM、Reflection 与 Execution Fabric

[AADM](aadm-decision-kernel.md) 是所有节点内部共同的决策 ABI：Atom → DecisionTransaction → Capability → Rule Field →
DiscretionEnvelope → candidate/Pareto → rolling verify。它保留受约束裁量，不替代 00–16、PDP 或状态机。

[Meta Reflection](meta-reflection-engine.md) 在关闭前审计整条决策链和假设：每次 R0、L2 R1、L3/L4 R2；所有知识、规则、
Eval、ADR、Debt 与后续工作都 proposal-first。

[Device Fabric](device-aware-execution-fabric.md) 是默认 OFF 的独立执行平面。当前只采纳接口和分期；已有本地 runner 不等于
Device Registry/remote Runner/Placement/Lease/Migration 已实现。Decision Kernel 决定“做什么/是否允许”，Fabric 只决定
“获准任务在哪里及如何可靠执行”。

## 7. 产物链

```text
WorkIntent + ProjectSnapshot
  → ContextPackage + ImpactPreScan
  → RequirementBaseline + Product/UX/Domain/Architecture/Data/API contracts
  → final ChangeImpact/Cost/Risk + AssessmentReceipt
  → DeliveryPlan(DAG) + GrantRequestSet → Kernel/PDP CapabilityGrants
  → ChangeSet + Test/Build/Provenance evidence
  → ReviewFindings + Security/QA/Performance verdicts
  → ReleasePackage + Human Approval
  → RuntimeEvidence + Incident/Postmortem
  → R0/R1/R2 Reflection + RoutingReceipts
  → HealthSnapshot + DebtPortfolio + EvolutionProposal
  → 新 WorkIntent
```

每条下游产物必须保存 `source_ids` 和版本摘要。缓存只保存可重建结果；任一输入摘要、决策状态、权限或代码快照
变化时失效，绝不以“命中了缓存”代替当前事实复验。

## 8. Skill 包装规范

每项能力先有宿主无关的 Capability Contract，再由适配器输出 ForgeOS role/skill card 或具体宿主 Skill。
一个成熟 Skill 至少包含：

- 简短触发说明和严格输入/输出契约；
- 决策步骤、适用/不适用条件、失败关闭规则和升级条件；
- 规则参考、正反例、模板；
- 可确定执行的检测/校验脚本及其测试；
- Review checklist 与最小 eval cases；
- 只加载任务相关引用的渐进式上下文，而不是把整套方法论塞入 Prompt。

实际创建可移植 Skill 时遵循 `SKILL.md + agents/openai.yaml + references/scripts/assets` 的最小结构，主文件保持
精炼，详细规则放入直接引用的 reference；生成后做结构校验，并用 fresh-context Agent 在真实任务上前向测试。

## 9. 文档导航

- [discovery-design-sop.md](discovery-design-sop.md)：00–07 的完整职责、规则与交接；
- [delivery-sop.md](delivery-sop.md)：08–14 的开发、审查、测试和发布 SOP；
- [operations-evolution-sop.md](operations-evolution-sop.md)：15–16 与持续治理；
- [governance-contracts.md](governance-contracts.md)：知识图谱、影响分析、ADR、技术债、权限、Review Loop；
- [engineering-standards.md](engineering-standards.md)：God File、复杂度、OOP/DI/AOP/DDD、数据与前端重构；
- [backend-decision-standard.md](backend-decision-standard.md)：后端业务/领域/模型角色、持久化、算法、网络、并发、迁移、生产就绪与长期架构决策；
- [frontend-design-standard.md](frontend-design-standard.md)：AFDS 规则权威、场景 Profile/页面 Pattern、flow/state/permission、跨平台适配与证据链；
- [skill-specifications.md](skill-specifications.md)：38 个可组合 Skill 包的触发、产物、规则、自动化与禁止项；
- [capability-skill-map.v1.yml](capability-skill-map.v1.yml)：140 个细粒度 capability 到 38 个包的唯一主 ownership；
- [aadm-decision-kernel.md](aadm-decision-kernel.md)：原子决策、规则场、裁量包络、Capability ABI 与滚动控制；
- [meta-reflection-engine.md](meta-reflection-engine.md)：R0–R2 二阶纠查、假设审计、Outcome 与 proposal routing；
- [device-aware-execution-fabric.md](device-aware-execution-fabric.md)：default-off 执行目标、放置、lease、证据与恢复；
- [implementation-roadmap.md](implementation-roadmap.md)：现状覆盖、分期、Skill backlog 与验收场景；
- [capability-catalog.v1.yml](capability-catalog.v1.yml)：规划期机读节点目录。
