# AI Engineering OS：能力中心化的软件工程组织蓝图

> 状态：**目标设计仍以规划为主；已采纳的窄 contract/shadow 与 producer 切片持续按独立 ADR 交付**。决策见
> [ADR-0037](../../adr/0037-capability-centric-ai-engineering-operating-model.md)、
> [ADR-0038](../../adr/0038-aadm-decision-kernel-and-meta-reflection.md)、
> [ADR-0039](../../adr/0039-default-off-device-aware-execution-fabric.md)、
> [ADR-0040](../../adr/0040-machine-readable-agent-engineering-governance.md)、
> [ADR-0041](../../adr/0041-backend-decision-contract-and-persistence-gate.md)、
> [ADR-0042](../../adr/0042-frontend-design-decision-contract.md)、
> [ADR-0043](../../adr/0043-frontend-code-architecture-governance.md)、
> [ADR-0044](../../adr/0044-business-ui-geometry-contract.md)、
> [ADR-0045](../../adr/0045-canonical-evidence-claim-contract.md)、
> [ADR-0046](../../adr/0046-local-governance-record-journal.md) 至
> [ADR-0061](../../adr/0061-knowledge-update-proposal-v1-contract-only.md) 与
> [ADR-0068](../../adr/ADR-0068-authority-neutral-capability-registry-v1.md)；当前覆盖与分期见
> [implementation-roadmap.md](implementation-roadmap.md)。运行时代码、测试和现有 `.agent/` 契约仍是当前事实源。

当前已交付的窄切片位于 `.agent/engineering/`：activation、14 学科状态、原子规则、`forge accept` detector 接线、
typed Context 路由、W0–W3 保障覆盖层和 TaskEvidencePackage 契约由 `harness/check.py` 机器校验，并随 scaffold/legacy
upgrade 继承。其 `activation: shadow` 只强制合同和接线完整性；ADR-0068 已交付唯一 staged entry 的 exact
Capability Registry validator/resolver，ADR-0069 已交付 140-item logical-only ownership projection，但 CapabilityInvocation、physical Skill/adapter generation、Grant/PDP、
plugin/runtime routing、AADM solver、R0–R2 Reflection 和 Device Fabric 仍未实现。TaskEvidencePackage 不含完成判定。

ADR-0067 Proposed-only ADR v2 另为“新建 Proposed 文档”交付严格 frontmatter/body/digest wire、Python checker、Go `writes_adr` 候选验证、golden、registry v22、Skill 与 scaffold。Universal checker 不扫描 repository；Go 仍对 ADR 目录做既有 baseline-integrity fingerprint，但不会把旧 ADR 当作 v2 parse/retro-validation/migration 对象。owner/approver/Claim/Evidence/affected-node 仍只是 caller/author 声明；该切片也不生成 ApprovalRecord、Graph coverage、Accepted 状态、immutability/supersession/compliance、persistence 或 lifecycle authority。

ADR-0068 authority-neutral Capability Registry v1 保持 ADR `proposed` 与 wire `staged`。Go/Python strict evaluator、显式输入 CLI、physical checker 和 exact golden 只验证/解析 `local-go-package-impact-prescan/1`；`resolved_exact` 不认证 Registry/owner/test/implementation，不读取 ambient repository/catalog，不选择或执行 implementation，也不产生 Grant/PDP、CapabilityInvocation、plugin/runtime-routing、persistence、transition 或 effect authority。

ADR-0069 Planning Capability Ownership Projection v1 同样保持 ADR `proposed`，但已交付独立 Python/Go pure projector、exact golden、`forge capability-ownership project --catalog FILE|- --mapping FILE|-` 与 source-only scaffold。它只证明 supplied exact planning sources 中 140 unique capabilities 对 38 declared packages 完整且唯一的 primary-owner coverage，并派生 unresolved logical `.agent/skills/*.md` refs；不解析/生成物理 Skill/host adapter、不修改 ADR-0068 Registry，也不产生 owner authentication、Grant/PDP、CapabilityInvocation、runtime routing 或 effect authority。

ADR-0070 Local Project Source Snapshot v1 继续保持 ADR `proposed`，并只实现 38-package parent 下的 `project-snapshot` 窄子项。Linux-only Go producer 对 tracked stage-zero 与 nonignored-untracked worktree 做 fixed-policy two-endpoint observation；source-portable pure Python decoder、exact golden、portable closed Skill、`.agent` adapter 与 fresh/legacy scaffold 已交付。它不是 atomic/current/complete/secret-free Project 或 Graph snapshot；Git/HEAD 未认证，scaffold 不复制 runtime 或安装宿主 Skill，unsupported host 或 runtime 不存在为 exit 3/`not_executed`，已存在但不兼容/执行失败为 exit 1，也不产生 Registry/Grant/PDP/CapabilityInvocation/routing/persistence/effect authority。

ADR-0071 Portable Context Engineering Skill 继续保持 ADR `proposed`，并只实现同一 38-package parent 下的 `context-engineering` 窄子项。Closed 16-file source package、零参数 exact-stdin adapter、strict manifest checker、registry v26 shadow wiring 与 fresh/legacy scaffold 已交付；ADR-0055 Schema/golden/wire 和 Python/Go/Rust semantics 不变。它不发现 source、调用 provider/model、编译 live prompt、安装 host Skill、认证 publisher 或提供 check-to-use atomicity、Grant/PDP/Approval、truth/instruction/completion/persistence/routing/effect authority；parent 与其余 36 items 保持开放。

ADR-0072 Portable Evidence Claim Validation Skill 继续保持 ADR `proposed`，并只实现同一 parent 下的 `evidence-claim-management` 窄子项。Closed 18-file source package、零参数 explicit-EOF exact-stdin validator、strict manifest checker、registry v27 shadow wiring 与 source-only fresh/legacy scaffold 已交付；ADR-0045 Schema/golden/wire 和 Python/Go/Rust semantics 不变。它只验证 already-authored bytes，不观察或 author/repair/persist records，不访问 journal/semantic view/proposal，不安装 host Skill 或提供 truth/instruction/Grant/PDP/Approval/completion/routing/transition/execution/effect authority；parent 与其余 35 items 保持开放。

ADR-0073 Portable Policy Authority Declaration Assessment Skill 继续保持 ADR `proposed`，并只实现同一 parent 下的 `policy-authority` 窄子项。Closed 30-file source package、两个独立零参数 explicit-EOF exact-stdin adapter、strict manifest checker、registry v28 shadow wiring 与 source-only fresh/legacy scaffold 已交付；ADR-0056/0059 Schema/golden/wire 和 Python/Go/Rust semantics 不变。它不新增 combined envelope，不签发或激活 Grant、不使 Approval 生效、不访问 live policy/identity/approval/revocation/usage state、不调用 Kernel/PDP/PEP/ADR-0057/0058 runtime，且不安装 host Skill 或提供 authorization/permission/persistence/routing/transition/execution/effect authority；parent 与其余 34 items 保持开放。

ADR-0074 Portable ADR Governance Proposed Document Validation Skill 继续保持 ADR `proposed`，并只实现同一 parent 下的 `adr-governance` 窄子项。Closed 25-file source package、exactly-one-basename-argument explicit-EOF exact-stdin validator、strict manifest checker、registry v29 shadow wiring 与 source-only fresh/legacy scaffold 已交付；ADR-0067 Schema/golden/wire 与 Python/Go semantics 不变。Caller basename 仅为独立 lexical label，不证明 physical file/repository identity；package 不新增 envelope，不扫描 repository，不 author/repair/reseal/accept/supersede/persist ADR，不复制 Go `writes_adr` runtime，不安装 host Skill 或提供 identity/approval/truth/compliance/lifecycle/execution/effect authority；parent 与其余 33 items 保持开放。

ADR-0041 又交付了一个窄的后端 shadow 切片：14 维 BackendDecisionPolicy、无裁决权 BackendDecisionPackage、十张密集
Skill adapter、data/backend Context route 和独立对抗 validator。它把持久化与低可逆决策前置，但不会从 diff 自动生成
package，也尚未成为 coding 前的 runtime gate；详细标准见 [backend-decision-standard.md](backend-decision-standard.md)。

ADR-0042 交付了前端设计 shadow slice：[AFDS](frontend-design-standard.md)、byte-pinned policy、Profile/Pattern catalog、
FrontendDesignPackage schema 与三张沿用既有 capability ownership 的 canonical Skill adapter。分类、IA、flow/state/action/permission、
Design System、跨平台映射与多源证据已接入 Context route、shadow detector、对抗 validator、scaffold 和 legacy upgrade；它仍没有
自动 diff compiler、可信视觉服务、pre-code runtime authority 或完成权威。

ADR-0043 增加独立但不抢占 capability ownership 的
[前端代码架构治理](frontend-code-architecture-standard.md)：项目显式声明模块所有权、依赖矩阵和 public/test entrypoint；
TypeScript 由项目 Compiler API 解析真实 import graph，Vue/Dart 在适配器缺失时诚实返回 inconclusive。确定性边界与
God/目录/API 预算等审查信号分离，detector 保持 shadow，不替代 `forge accept`。

ADR-0044 又在 AFDS 内增加 Business UI Geometry 扩展：条件化 `ui-geometry` 只是一张 supporting procedural Skill，既有三张
canonical Skill 的 capability ownership 不变。layout decision 可绑定 digest-pinned `business_ui_composition`，把角色、任务、状态、
数据语义和操作风险追溯到区域、轴、分组、间距、线条、形状及响应式关系；项目真实 Runner 还可提供可选
`geometry_measurement_receipts`。当前 validator 只校验严格 JSON、引用、摘要和声明一致性，report 也只是声明式执行观察；二者均不
证明浏览器/原生 Runner 真执行、视觉平衡、光学校正、任务成功或可信 producer/reviewer，最终完成仍只属于 `forge accept`。

ADR-0045 交付 Governance/Decision Kernel 的 0F-A 前置片：[Evidence/Claim v1](governance-contracts.md) strict schema、
`record_id/aggregate_id/sequence` 身份、kind-separated canonical SHA-256、Go/Rust/Python golden 与 universal shadow checker。
它只允许 registry `shadow_admissibility` 精确矩阵内的 Claim 和 untrusted/observed Evidence，明确拒绝自证 confirmed/accepted/waived、Hub 写入、旧 Memory/ADR
自动升级、Grant/Approval/Transition 与 hard-gate 效力；后续 0F–1 仍需 authenticated identity、ledger、replay/revocation 和 SoD。

ADR-0046 至 ADR-0053 在该 canonical wire 之上分别交付 local exact-record journal、CognitiveAtom/三类 Evidence shadow adapter，
以及 local gate、Evolve locator 和 Go package dependency observation producers。ADR-0054 再增加 exact-v27、显式 caller-time、始终选择
current structural tail 的本地 semantic projection；ADR-0055 增加只消费 exact caller request 的 authority-free ContextPackage pure builder，
固定 typed lanes、required-first/optional omission、redaction/token/digest/cache revalidation，但不读取 source、调用 provider 或持久化。
ADR-0056 再冻结 strict `CapabilityGrant v1` contract-only envelope、最小 effect/scope vocabulary 和 authority-neutral declared assessment；其 pure evaluator 不认证
issuer/proof/principal/policy/Approval。ADR-0057 只增加 operator 部署的独立非 Agent `forge-kernel` 的单一 authenticated bootstrap repo-read profile：repo 外
pinned root/key/state、signed policy/request、`repository-reader/v1` + `repo.read` exact paths、小预算/TTL、local/development/test，以及 signed Grant + durable signed receipt。
ADR-0058 再交付一个窄执行 profile：独立 execution root、signed policy/invocation、Linux-only `openat2` manifest-bound exact-byte read、
single-use reservation/intent/terminal ledger、no-raw persistence 与 receipt-only replay。Timeout 是 cooperative，管理员仍可替换整个 signed state 回滚，
best-effort buffer clear 不证明 secure erasure/process isolation/HSM。`0600`/effective-UID 不等于 OS principal/HSM 隔离，scaffold 不安装 runtime/keys；
ADR-0059 再交付 ApprovalRecord v1 contract-only 的 exact wire、declared assessment 与 Grant reference 投影，但不认证或产生 effective
approval；其 Accepted 状态不扩大 runtime authority。ADR-0060 已交付 TransitionReceipt v1 contract-only slice：只冻结 exact state graph、
receipt/predecessor/recovery declared assessment 与 Grant/Approval reference comparison，既不认证 current state/precondition，也不写 ledger 或推进 transition；
其 Accepted 状态同样不扩大 runtime authority。ADR-0061 已交付 strict KnowledgeUpdateProposal/target/request/assessment、exact
Evidence/Claim reachable closure、create/supersede declarations 与 Grant/Context/artifact declared compatibility，并已通过正式 `forge accept`；它
不认证 proposer/Grant/Context/Evidence，不读取 current head、仲裁 conflict/freshness/policy，不产生 truth/adoption/authorization/permission/
persistence/apply/receipt/execution/effect，其 Accepted 状态不扩大 Knowledge/runtime authority。完整 plan-finalization/authenticated Approval/revocation/general PDP/authority-bearing Transition/Knowledge apply/effects 仍未交付。
ADR-0062 另在 exact ADR-0053 observation 上交付 authority-free local Go package reverse ImpactPreScan、完整 induced local edges 与
deterministic shortest witnesses；package gap 显式 UNKNOWN，system impact 恒为 UNKNOWN。它不读取 live repository、不产生
ADR-0065 已交付 caller-bytes-only 的 partial GraphSnapshot module/package projector/checker；ADR-0066 又以独立 transport/profile 增加
package-scoped lexical test source-set nodes 与 module→test edges，同时保持旧 profile/Schema/golden 不变。Go/test coverage 是互斥 PARTIAL
partition，system/freshness 仍 UNKNOWN；source set 不表示 test case、execution、result、coverage 或 verification。两者都不是完整
GraphSnapshot/final ChangeImpact/Cost/Risk，也不满足 G3 或 Assessment Join；完整 system Impact Closure 仍未交付。
ADR-0063 只把 caller-declared L3/L4 Build 的 `reviewer_v1` 收紧为 required、exact-final-line、串行定向回修且
resume/chain 不可降级的 fail-closed boundary；L0–L2 与 `materiality_not_bound` 仍为 advisory/fail-open 兼容。materiality 不被
自动推断或认证，role/model/provider identity、review quality、cryptographic SoD 与 source/context/policy/artifact digest binding
仍未交付；checkpoint/chain 只提供 crash consistency，same-UID/admin state replacement/rollback 不在防护范围。旧 runtime 不识别
该 contract 时必须拒绝，scaffold 只复制治理资产而不安装 host runtime。
这些边界也不替代 `forge accept`，后者只是完成裁决而非 issuer/execution authority。完整边界、命令与资源预算见
[governance-contracts.md](governance-contracts.md)。

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
- [frontend-design-standard.md](frontend-design-standard.md)：AFDS 规则权威、场景 Profile/页面 Pattern、flow/state/permission、跨平台适配、Business UI Geometry composition/report 与证据边界；
- [frontend-code-architecture-standard.md](frontend-code-architecture-standard.md)：前端模块所有权、依赖/public API、状态/effect 归属、复杂度证据与例外治理；
- [skill-specifications.md](skill-specifications.md)：38 个可组合 Skill 包的触发、产物、规则、自动化与禁止项；
- [capability-skill-map.v1.yml](capability-skill-map.v1.yml)：140 个细粒度 capability 到 38 个包的唯一主 ownership；
- [aadm-decision-kernel.md](aadm-decision-kernel.md)：原子决策、规则场、裁量包络、Capability ABI 与滚动控制；
- [meta-reflection-engine.md](meta-reflection-engine.md)：R0–R2 二阶纠查、假设审计、Outcome 与 proposal routing；
- [device-aware-execution-fabric.md](device-aware-execution-fabric.md)：default-off 执行目标、放置、lease、证据与恢复；
- [implementation-roadmap.md](implementation-roadmap.md)：现状覆盖、分期、Skill backlog 与验收场景；
- [capability-catalog.v1.yml](capability-catalog.v1.yml)：规划期机读节点目录。

ADR-0075 Portable Knowledge Graph Curation source-distributes only the two existing ADR-0065/0066 exact-request partial GraphSnapshot projectors. It preserves their independent wires, PARTIAL/UNKNOWN semantics and non-authority boundary and adds no wrapper, route, live capture, persistence or impact conclusion.

ADR-0076 Portable Change Impact Cost Risk source-distributes only the existing ADR-0062 exact-request lexical ImpactPreScan projector. It preserves UNKNOWN system impact and adds no raw graph wrapper, route, live capture, complete Impact/Cost/Risk, persistence or authority.

ADR-0078 WorkIntent v1 Proposed Candidate Governance records only the unchanged ADR-0077 Python/Go/Rust exact structural parity in Registry v32, a checker-only shadow and a source-only Python distribution boundary. It preserves all runtime scope arrays, leaves Go/Rust Catalyst-only and adds no acceptance, semantic authority, G0 closure, route, lifecycle, persistence or effect.

ADR-0080 Authenticated ADR Approval v1 Proposed Candidate Governance records only ADR-0079 caller-supplied structure/digests/relations in Registry v33, one checker-only shadow and dependency-free Python source-only distribution. It preserves the complete runtime scope mapping and adds no Ed25519 verification, authentication, authorization, receipt issuance, external-root/time/revocation currentness, CAS, durability, Accepted lifecycle, G0 closure, route, runtime, persistence or effect; no future Go service, keys or state are copied.

ADR-0083 Authenticated ADR Lifecycle v1 Proposed Candidate Governance records ADR-0081 only as Catalyst-repository Go authority evidence and distributes only the ADR-0082 dependency-free Python structural candidate. Registry v34 keeps the complete scope SHA-256 unchanged and adds one checker-only shadow, no Skill, route or runtime. Generated projects receive no Go authority, production key or state, and validation grants no lifecycle transition, Accepted source, compliance, permission or effect authority.

ADR-0085 Authenticated ADR Lifecycle Authority Evidence records the frozen ADR-0084 exact44 Go implementation only as Catalyst-repository evidence. Registry v35 keeps the complete scope SHA-256 and existing checker-only shadow unchanged; source-only exact4 distribution copies decisions and governance checks, never Go authority, keys, state, route, Skill or runtime.

### ADR-0087 Legacy Governance Read Import

Registry v36 records ADR-0086's exact supplied-byte Memory/ADR projector only as an unverified read-only candidate. The complete scope digest remains unchanged. The checker-only shadow declares its honest zero-argument argv; an operator must pipe the request and close EOF. Source distribution is Python-only and excludes the Catalyst Go parity package, routes, Skills, runtime, persistence and authority.

### ADR-0089 Kernel Operational Reference Governance

Registry v37 preserves the complete scope digest and records ADR-0088 only as a structural operational-reference subclosure. Its checker-only shadow validates the pinned golden with exact argv; generated projects receive Python source while Go/Rust parity and runtime registration remain Catalyst-only. Fourteen false attestations deny authority and effect semantics, and the full Kernel ABI item remains open pending CognitiveAtom expansion, DecisionTransaction and cross-closure work.

### ADR-0091 Kernel Decision Reference Governance

Registry v38 preserves the complete scope mapping and digest while recording ADR-0090 only as a structural reference family: CognitiveAtom v2, DecisionTransaction v1 and one-way references to the operational records. Its checker-only shadow validates the pinned golden with exact argv; exact19 generated source is Python-only, while Catalyst exact13 Go, flat exact9 Rust and the shared `lib.rs` registration remain repository-only. All 22 attestations are false, declared authority and hardness are ineffective, and instructions stay disabled. No Skill, route, runtime, PDP or controller is added. The narrow structural reference-family repository slice passed formal `forge accept` and is complete; both ADRs remain Proposed, ADR-0038 remains ADOPTED-PARTIAL, and DecisionCapsule, AuthorizedTransactionSpec, authenticated PDP and the rolling controller remain open.

### ADR-0093 Decision Capsule Structural Replay Governance

Registry v39 preserves the complete scope mapping and digest while recording ADR-0092 only as a pending four-object structural replay repository Candidate. Its checker-only shadow validates the pinned golden with exact argv; exact19 generated source is Python-only, while Catalyst exact15 Go, exact14 Rust and shared `lib.rs` registration remain repository-only. All 32 attestations, both replay controls and all seven completion claims are false. No Skill, route, runtime, model/rule/history/Reflection consumer, persistence, PDP or controller is added. ADR-0092/0093 always remain Proposed/null; the narrow item remains unchecked until independent review and formal `forge accept`, while ADR-0038, full DecisionCapsule and AuthorizedTransactionSpec stay open.
