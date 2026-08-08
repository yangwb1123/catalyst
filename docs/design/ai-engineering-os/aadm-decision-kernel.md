# AADM：原子化自适应决策内核

> 状态：已采纳的目标模型，`planning_only`。本文定义 Decision Kernel 的稳定语义，不声明当前 runtime 已有 Atom
> compiler、约束求解器、Pareto planner、Controller 或学习器。AADM 位于 00–16 节点之下；它不绕过治理状态机、
> CapabilityGrant、Review、Release 或外部 operator。

## 1. 统一模型与边界

```text
Prompt / Runtime Signal
  → Cognitive Atoms → Typed Task Hypergraph
  → Decision Transactions → Capability Registry
  → Rule Field → Discretion Envelope
  → Candidate Plans → Constraint Filter → Pareto Selection
  → Rolling Execute / Verify / Compensate
  → Evidence → Reflection → Governed Learning
```

可概括为：

`AgentDecision = ConstraintSolve + MultiObjectiveOptimize + BoundedAutonomy + RollingFeedback + EvidenceLearning`

- 大模型负责语义理解、提出假设/反例、生成差异化候选与解释；
- 确定性 Kernel 负责 schema、规则激活、权限、状态机、预算、冲突、排序、receipt 和重放；
- 工具/Execution Fabric 负责真实 effect；Verifier 产生证据；
- Governance Kernel 负责认识状态、授权与长期规则变更。

00–16 是宏观责任图；AADM 是节点内部的决策 ABI；Capability 是可复用契约；Agent 是临时执行主体；
ExecutionTarget 是物理/服务执行位置。五者不得合并成一个“万能 Agent”。

## 2. 最小语义单元：CognitiveAtom v1

Atom 是当前任务中的单一命题投影：

```yaml
atom_id: atom-...
atom_type: goal|actor|object|operation|fact|constraint|preference|unknown|assumption|hypothesis|inference|risk|acceptance|evidence|observation|decision
proposition: {subject: ..., predicate: ..., object: ...}
source: {principal_or_artifact: ..., evidence_ids: []}
claim_ref: ...
epistemic_state: ...
supporting_evidence_ids: []
contradicting_evidence_ids: []
authority_ref: ...
projection_confidence: 0.0
hardness: none|invariant|contract|required|preferred|advisory
scope: {project: ..., module: ..., object: ...}
validity: {source_revision: ..., valid_from: ..., valid_until: ...}
trust: {source_class: ..., instruction_allowed: false}
```

Atom type × hardness 是封闭矩阵：Constraint 可为 invariant/contract/required/preferred/advisory，但 invariant/contract 必须
引用 active Constitution/accepted contract；Goal/Acceptance 可 required/preferred/advisory；Preference 只 preferred/advisory；
Decision 只有绑定 accepted ADR/Approval 时可 required，否则 advisory；Fact/Actor/Object/Operation/Evidence/Observation 使用
none；Assumption/Hypothesis/Inference/Unknown/Risk 使用 none/advisory，永远不能满足 hard guard。Guard 的强前提只接受当前
confirmed Fact、active Constraint、accepted Decision 或实际 verified Observation；projection confidence 不能替代 Evidence。

`projection_confidence` 只允许 Assumption/Hypothesis/Inference，且必须等于源 KnowledgeClaim 的有版本值；其它 Atom 省略。

Atom 必须只有一个可判定命题，保留 support/contradiction；hard Fact/Constraint 不由模型自报；外部/仓库正文默认是不可信
数据，不是指令。Atom 是 Task 的运行期投影，长期写回必须转换成 `KnowledgeUpdateProposal`，经过 KnowledgeClaim 的状态/
证据规则，不能直接成为 confirmed Fact。

## 3. 最小行为单元：DecisionTransaction v1

Transaction 先形成不含 Grant 的 immutable `TransactionProposal`（actor/owner、reads/guards、options/selected action、writes、
proof/verifier/compensation、budget、idempotency 与 proposal digest），供 PDP/Approver 裁决。Grant/Approval 精确绑定该 proposal
digest 后，Kernel 生成不可变 `AuthorizedTransactionSpec`；随后才创建 append-only attempt/receipt。这样避免把可变 status 或
receipt ref 放入 canonical spec，也避免 Grant 与 spec 的哈希环：

```yaml
kind: AuthorizedTransactionSpec
transaction_id: dt-...
task_id: ...
proposal_sha256: ...
authorization_receipt_id: ...
kernel_signature: ...
actor_principal: {type: agent|service|human|operator, id: ...}
accountable_owner: ...
trigger_atom_ids: []
read_set: []
read_versions: []
goal_atom_id: ...
guard_atom_ids: []
option_ids: []
selected_action_id: ...
write_set: []
write_preconditions: {expected_world_version: ..., compare_and_swap: []}
completion_condition: ...
proof_obligations: []
verifier: {capability_id: ..., principal: ..., independence_proof: ..., timeout: ...}
compensation: {action: ..., applicability: ..., required_capability: ..., required_grant: separate, escalation: ...}
budget: {time: ..., calls: ..., tokens: ..., cost: ..., output: ...}
capability_grant_id: ...
grant_issuance_receipt_id: ...
approval_policy: {record_ids: [], required_threshold: ..., authority_domains: [], separation_of_duties_proof: ...}
idempotency_key: ...
spec_sha256: ...
```

Kernel 生成 spec 时必须从已批准 proposal 做 canonical、byte-for-byte field projection，逐项复验 actor/read/guards/options/
selected action/write/completion/proofs/verifier/compensation/budget/idempotency；任何字段新增、删除、重排语义或 digest 漂移都使
Grant/Approval 无效。`authorization_receipt_id` 和 Kernel signature 同时绑定 proposal/spec digest、Grant/Approval 与 policy，
防止把合法 ID 塞入篡改后的 spec。`spec_sha256` 计算时排除自身与 signature；receipt 使用非内容寻址 stable ID，正文再引用
spec digest，因此不形成摘要环。

一个 Transaction 必须只有一个主要目标、一个可描述的主要状态变化、一个 accountable owner、一个明确完成条件，并能
独立验证及安全重试/补偿。权限修改、创建文件、发送通知、写审计、迁移审批状态若具有不同原子性/失败语义，应拆成多个
Transaction；需要同一原子提交的状态变更和 audit/outbox 则以一个受事务边界保护的复合 write set 表达。

read set 的期望版本/CAS、idempotency key、actor/owner、Grant issuance/consumption、Approval（若需）和 verifier independence 是
v1 必填；纯只读/N/A 字段也要显式声明 applicability。补偿是新的 effect，必须取得仍有效且独立 scope 的 Grant，不能沿用
失败 action 的隐含权限。外部系统 effect 使用 intent/outbox、effect receipt 与 reconciliation，不声称和本地 write set
跨系统原子提交。

每个 DecisionTransaction 维护一条聚合 receipt chain，并一对多引用 Device/Fabric 契约定义的物理 `ExecutionAttempt`；
physical Attempt 的 CREATED/PLACED/.../LOST/CLEANED lifecycle 以 Device Fabric 为唯一权威，AADM 不另定义第二套。聚合 chain
追加 GrantConsumption、Observation、Effect、Commit/Abort、Verification、Compensation，并引用全部 physical attempt receipt
chains；各 receipt 按 `prior_receipt_sha256` 串联。receipt digest 排除
自身字段并引用 spec digest，不让 spec 反向引用 receipt；最终 DecisionCapsule 引用完整 receipt chain 和 outcome。

proposal 状态是 `draft→proposed→authorized/rejected`；`DecisionTransactionState` 聚合投影为
`authorized→executing→observed→verified`，失败可到 `failed/rolled_back/quarantined`，并由所引用的物理 Attempt/Effect receipts
确定。只有
补偿证据成立才到 `rolled_back`；未知外部 effect 到 `quarantined`。没有有效 Grant 不得进入 authorized；没有 verifier
Evidence 不得进入 verified；重试复用幂等键但必须取得有效 lease/fencing token。

每次 physical claim 和每个 effect 前都重新验证全部 required ApprovalRecords、CapabilityGrant、issuance/consumption、
proposal/spec 与 source/context/policy/artifact digests、expiry/revocation/risk/SoD。Approval 或 Grant drift 一律 fail closed，
按当前 Governance 状态进入合法的 BLOCKED/CHANGES_REQUESTED；若已在 RELEASING 或 effect 不确定则 QUARANTINED。

## 4. InteractionEvent 与 Decision Capsule

所有 user/agent/tool/system 交互以 append-only `InteractionEvent` 保存：event/task/transaction、principal、target、
verb（request/propose/approve/execute/observe/verify/reject/rollback）、object、correlation/causation、source/context/policy/
artifact digest、confidence、logical sequence 和时间。

每个 Transaction 关闭时生成可重放 `DecisionCapsule`：读取的 atoms/claims/snapshots、激活/抑制规则、候选与拒绝理由、
最小冲突集、选中方案、profile/envelope、Grant/Approval，以及该 Transaction **全部** attempt chains（含 failed/LOST/retry）、
Evidence、compensation 和 Reflection refs；不能只保留最终成功 attempt。
Replay 默认只重算/比较，不重放 effect；换模型/规则/世界版本生成新的 evaluation branch，不篡改历史。

## 5. Typed Property Hypergraph

普通单边图不能表示“多个 guard 共同成立”。Task graph 使用带属性的有向超边：

- 节点：atom、transaction、capability、artifact、rule、agent/principal、evidence、execution target；
- 关系：contains、depends_on、conflicts_with、supports、refines、abstracts、guards、realizes、verifies、causes、
  compensates、alternative_to、owned_by；
- 超边保存多 from/to、condition、weight、认识状态、evidence、version、freshness。`causes` 只能引用 validated causal
  KnowledgeClaim，且该 Claim 绑定 experiment/counterfactual/confounder analysis 与 supporting Evidence；Evidence 本身只记录
  观察。未验证关系必须标 `causal_hypothesis/inference`，certainty/confidence 不能把相关性升级成因果事实。

“审批人有权限 + 申请待审批 + 未被并发处理 + 意见合法 → 状态迁移”是一个 guard hyperedge。缺输入产生 UnknownAtom，
不把 false、missing 和 unknown 合并。

标准组合算子：AND、OR、SEQUENCE、PARALLEL、GUARD、LOOP、VERIFY、COMPENSATE、ESCALATE、REFINE。每个 LOOP 有
最大轮数/成本/时间、进展度量和 tripwire；PARALLEL 需证明无 state/path/resource conflict；COMPENSATE 不等于回滚成功，
仍需验证。

## 6. Capability Registry：Kernel ABI

Capability 不是 CLI 命令：

```yaml
capability_id: graph.impact
version: 1.0.0
owner: {module: ..., team: ...}
domain: reasoning|planning|execution|verification|governance|device
input_schema: ...
output_schema: ...
preconditions: []
postconditions: []
effects: []
proof_obligations: []
failure_modes: []
rollback_or_compensation: ...
observability: []
risk_floor: ...
permission_requirements: []
implementations: {cli: ..., api: ..., agent_callable: ...}
tests: []
```

CLI/Web/API/Agent-to-Agent 都是 adapter，共享相同 schema 和语义。Kernel 首批冻结 ABI 是 Governance Envelope、Atom/
Claim/Evidence、DecisionTransaction、InteractionEvent、Capability invocation、ArtifactRef、Transition/Execution receipts；
新增命令不得成为新的隐式真值或权限路径。Breaking change 走版本/迁移 ADR，不靠全局同步升级。

插件只通过 `CapabilityPluginManifest` 扩展 Registry：plugin/publisher/version/signature、Kernel/API compatibility、provided
capabilities/rules/checks/adapters、requested permissions/effect ceiling、dependencies、fixtures、migration/uninstall/rollback。插件默认
不可信、显式安装、隔离执行，不能注册/覆盖 invariant、直接写 Kernel ledger 或借 host CLI 绕过 Grant；禁用/升级先做 dangling
reference 与 replay compatibility 检查。Marketplace 是分发/发现层，不是信任根。

## 7. TaskProfile：多维画像，不用单一复杂度分

最小向量：`Clarity, Scope, Coupling, StateComplexity, Risk, Uncertainty, Reversibility, Novelty, Testability,
EvidenceCoverage, Observability, Value, TimePressure, ChangeSurface`，每项保存 observed inputs、公式版本、区间/置信度和
UNKNOWN，不只保存 0–1 数值。

建议公式仅是可校准 policy，不是事实：

- `AssumptionRisk=(1-confidence)×impact_if_wrong×(1-reversibility)`；
- hazard risk 以 severity/probability/blast radius/irreversibility/detectability 分量表示，聚合必须保留 max floor；
- complexity 组合 scope/coupling/state/uncertainty/novelty/change surface；
- confidence 只能由 evidence coverage、clarity、consistency、testability 和已验证 precedent 提升。

模型自信不能提高 EvidenceCoverage。公式版本、阈值和校准数据写入 capsule；未校准结果只能用于排序/触发调查，不能单独
批准高风险 action。

## 8. Rule Field：五层规则、动态激活与抑制

1. `invariant`：不得伪造证据、越权、破坏数据完整性、绕过安全、把推断当事实；不可权重抵消或豁免；
2. `contract`：用户/业务/公共接口/平台兼容硬边界；
3. `policy`：组织要求，可经正式、限时、带补偿控制的例外；
4. `heuristic`：最小修改、高内聚、低耦合、复用、可测试/回滚等默认实践；
5. `suggestion`：可创造的体验、模型、长期改善建议，不扩大当前 scope。

Rule 的 authority level（invariant/contract/policy/heuristic/suggestion）与 enforcement mode
（hard_gate/review_trigger/guidance）正交；invariant 必为 hard_gate，contract 的改变走 scope/decision 变更而非 waiver，
heuristic/suggestion 不得伪装成 hard gate。Rule 还包含 scope/trigger/guard/effect/priority/weight/confidence/specificity/
evidence/version/validity。硬规则
一旦适用必须激活；软规则按 semantic match、scope、trust、specificity、freshness 与 expected impact 选择。每次选择同时
记录 `ActivatedRule[]` 和 `SuppressedRule[]`；简单 UI 局部改动可激活几何/design system/a11y，同时抑制 DDD、分布式事务、
服务拆分和事件溯源。

规则冲突先求 `MinimalUnsatisfiedCore`，不静默按最后加载覆盖。只有 policy 可有 Exception；保存 task/rule/scope、source/
context/policy/artifact digests、reason/benefit/added risk、risk owner、compensating controls/proofs、ApprovalRecord、
RiskAcceptance、WaiverReceipt、expiry/debt link；到期自动失效。Invariant 不存在 exception。

## 9. DiscretionEnvelope：受约束的主观能动性

自主性不是 bool，而是 `information/planning/execution/learning` 四维。Kernel 先从 task profile、Rule Field 和 principal
trust 生成 pre-grant `PolicyDiscretionEnvelope`；PDP 只能在其内签发 CapabilityGrant。运行时计算
`EffectiveDiscretionEnvelope = intersection(PolicyEnvelope, CapabilityGrant, run/target budgets)` 并写 receipt，任何维度只能收窄。
Envelope 包含 allowed/forbidden action types/scopes、max risk/cost/change surface、四维 autonomy、required proofs、rollback/
human gate、max parallelism/horizon、network/data/target restrictions 和 stop conditions。

- 高不确定、高风险：允许广泛调查/提出方案，执行空间很小；
- 低风险、可逆、可观察：允许小步实验；
- learning autonomy 始终只允许提交 CandidateRule/Eval/Knowledge proposal；Agent 不得自主修改 hard rule。

Policy Envelope 是 CapabilityGrant 的签发输入，不是 Grant 本身；Effective Envelope 是两者交集，不再反向参与签发，消除
循环。任一 source/context/policy digest 不一致时 fail closed。

## 10. 候选方案与选择

非简单任务至少生成差异化而非凑数的候选：最小改动、沿用当前模式、根因结构方案；trivial 任务可用单方案，但需记录
为何无需比较。

1. 硬约束过滤得到 Feasible set；为空时输出最小冲突集，不编造方案；
2. 在 goal/value/evidence gain/testability/reversibility/risk/complexity/migration/operations cost 上构造 Pareto frontier；
3. 按任务画像动态权重选择“最低充分复杂度”，保存 rejected reason 和 sensitivity；
4. L3/L4、不可逆或差异接近的决策交独立审查/人类裁决，不以总分替代红线。

`planning_exploration` 可随 uncertainty/novelty/option diversity 增长；`action_exploration` 只在 explicit policy 允许的
non-production、low-risk、reversible、observable scope 内启用，并乘以低 risk、高 reversibility 和 observability；任一分量
UNKNOWN 时 action exploration=0。可以大胆思考，不能因此大胆产生不可逆 effect。

## 11. 滚动规划与 Reconciliation

不一次写死全部步骤。Controller 保存 versioned desired state，观察 actual state，计算最小 delta，提交下一小批
Transaction，验证后更新世界版本并重算 profile/envelope/plan：

`Observe → Compare → Decide → Authorize → Act → Verify → Reflect → Reconcile`

planning horizon 随 confidence×reversibility 增大，随 risk+uncertainty 增大而缩短。实际偏离、风险上升、Evidence/Context/
policy/Grant 漂移立即 fail closed，并按当前 Governance state 选择合法边：IMPLEMENTING/VERIFYING/REVIEWING/OBSERVING/
REFLECTING 可提交 `CHANGES_REQUESTED(rework_target=...)`；AUTHORIZED/PLANNED/RELEASE_READY 等不允许该边的状态进入
BLOCKED 并保存安全 resume target；RELEASING 或任何未知 external effect 进入 QUARANTINED。不得通用跳回
ASSESSED/AUTHORIZED。无进展、预算耗尽、循环或重复失败进入 BLOCKED，不把“跑满轮数”当成功。

Desired State 不能直接触发 effect：Controller 仍需 Transaction、Grant、lease 和 proof。Decision Kernel 决定“做什么/
为什么/是否允许”，Execution Fabric 决定“在哪里/如何可靠执行”，二者通过 immutable TaskSpec/receipts 连接。

## 12. Unknown 与假设处理

Assumption 保存 confidence、impact_if_wrong、reversibility、verification_cost、chosen_default、validation plan、owner/expiry/
status。低风险可用可逆默认并记录；中风险先查代码/文档/日志/测试；高风险先模拟/实验或请权威确认；极高风险阻止
不可逆执行。是否询问用户取决于载重程度和最安全的信息源，不是“有 Unknown 就打断”。

## 13. Reflection 与受控学习

每次 Change 至少 R0 快速反思；L2 为 R1 工程反思；L3/L4 为 R2 独立 adversarial reflection。它审计 goal alignment、
requirement completeness、assumptions、architecture、over/under-design、failure modes、security/privacy、performance/
reliability、maintainability、UX、future cost 和 knowledge extraction。

Critic 第一遍只读 Requirement/Decision/current artifact/tests/runtime evidence，不接收 Executor 自我解释；第二遍可把解释作为
低权重 claim 查证。输出 `ReflectionReport`、AssumptionAudit、CandidateRule、EvalCandidate、Debt/Knowledge proposal 和新
WorkIntent；不直接改 Truth/Rule/Test 或代码。

规则晋升是：Observation → Candidate Lesson → repeated evidence → heuristic → shadow evaluation → soft policy → quality gate →
hard rule。每级有 owner、样本/反例、命中率/伤害率、scope、expiry、审批和 rollback；一次成功/失败不能自动升级全局规则。

学习把 `DecisionCapsule` 与 `OutcomeObservation` 显式关联；Outcome 保存观察窗口、延迟结果、baseline/counterfactual、可能
混杂因素、缺失/选择偏差和数据质量。短期 gate PASS 不是长期价值因果证据。策略只先进入 shadow/champion-challenger，
监控 distribution/concept drift、代理指标投机和伤害 guardrail；任何权重/策略版本都有 owner、Approval、rollback，出现
reward gaming、不可解释 uplift 或 drift 时撤回候选，不能由同一 Agent 自评后自动晋升。

## 14. 最小验收

- 相同全部载重输入（task/profile/rules/options/source/context/policy/Grant/Approval）生成 canonical Atom/AuthorizedTransactionSpec
  digest，Capsule 由确定 receipt chain 可重建；恶意正文不能成为 instruction；
- Agent 无 Grant 无法执行，Verifier 无证据无法标 verified，未知外部 effect 必须 quarantined；
- 冲突合同输出最小冲突集；Critical hazard 不被 Pareto/总分稀释；
- 简单 UI 任务能抑制无关 DDD/分布式规则；高风险数据任务可广泛规划但行动自治被压低；
- 执行偏离触发滚动重规划与旧 Grant 失效；无进展循环有界；
- Replay 默认零 effect，可对比新模型/新规则但不改写历史；
- Reflection 只能提交 proposal，hard rule/confirmed fact/accepted decision 必须经过各自治理权威。
