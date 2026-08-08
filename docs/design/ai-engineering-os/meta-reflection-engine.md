# Meta Reflection Engine：二阶纠查与经验闭环

> 状态：目标设计，`planning_only`。它不是“再问一次模型”，也不是给现有 Review 换名；它审计从目标、假设、方案、执行、
> 验证到结果的整条决策链，并且只能提交受治理的改进提案。

## 1. 当前事实与真实缺口

当前 ForgeOS **不是完全没有纠查**：

- `.agent/workflows/review.yml`、fresh-context reviewer、QA strict verdict 已做交付前独立审查；
- `.agent/workflows/evolve.yml` 已做 evidence scan、gap→plan→implement→review 循环；
- Go Evolve 会把 gate failure、REQUEST_CHANGES 和 trajectory 写成结构化 memory；
- `.agent/architecture/loop-engineering.md` 已把 Reflect 标为“部分落地”，并明确仍缺失败/缓慢根因与路由/流程自适应；
- Operations 规划已有 incident postmortem。

缺的是一个统一、可机器验证的二阶契约：没有 `ReflectionCase/Report`、逐项 Assumption Audit、Decision Path/替代方案复查、
风险分级的 R0/R1/R2、标准化 routing receipt，以及从反思发现到 Claim/Debt/Eval/Rule/ADR/New WorkIntent 的受控闭环。
因此应当整合现有能力，不新增一组互相绕开的 `reflect-*` 命令。

## 2. 与 Review、QA、Postmortem、Evolution 的区别

| 能力 | 核心问题 | 时点 | 输出 |
|---|---|---|---|
| Review/QA | 当前变更是否符合合同并可交付？ | 实现后、发布前 | finding/verdict/closure |
| Postmortem | 一次事件为什么发生、怎样恢复？ | Incident 后 | causal timeline/actions |
| Reflection | 整个决策过程是否遗漏、偏误、过度/不足设计？ | 验证/观察后、关闭前 | meta findings/assumption/strategy audit |
| Evolution | 哪些候选改善值得成为下一轮工作？ | 周期或事件驱动 | prioritized proposal/experiment |

Reflection 复用前三者的证据，但不代替任何门禁。若在尚未发布时发现 load-bearing 问题，进入
`CHANGES_REQUESTED(rework_target=...)`；发布后发现问题则创建 Incident/New WorkIntent，必要时由有权主体 quarantine，
不能在反思阶段偷偷修改已发布代码。

## 3. 生命周期与强度

```text
PLAN → EXECUTE → VERIFY → REVIEW/RELEASE/OBSERVE
                              ↓
                         REFLECTING
                              ↓
                          LEARNING
                              ↓
                            CLOSED
```

- **R0 Quick**：每个 Change 必须；目标是否满足、明显遗漏、假设是否有结论、验证是否真实、规则/范围是否越界；
- **R1 Engineering**：L2/feature 默认；加架构、数据、安全、性能/可靠性、维护、UX、failure mode、复杂度与未来成本；
- **R2 Adversarial**：L3/L4、不可逆、安全/隐私/生产/跨系统；独立 Critic、反方攻击、替代方案再发现、反事实、因果/残余风险。

强度是 floor；新 Evidence 可上调。R0 应快速且主要确定性，不让按钮文案任务进入全套架构辩论。

## 4. ReflectionCase / ReflectionReport v1

```yaml
case_id: reflection-...
change_id: ...
level: R0|R1|R2
source_context_policy_artifact_digests: {}
requirement_and_decision_refs: []
decision_capsule_refs: []
transaction_and_attempt_refs: []
evidence_and_outcome_refs: []
critic: {principal: ..., independence_proof: ..., context_mode: evidence_first}
dimensions: []
assumption_audit: []
strategy_review: {original_options: [], new_options: [], counterfactuals: []}
findings: []
proposals: []
verdict: CLEAR|FOLLOW_UP|REWORK_REQUIRED|ESCALATE|ABSTAIN
```

Finding 使用统一 `finding_severity=BLOCKER/MAJOR/MINOR/INFO` 和独立 `domain_risk`，包含 type、事实/推理、Evidence、failure
scenario、影响、affected contracts、required action、owner、due/trigger 和 validation。类型至少包括 missed requirement、wrong/
stale assumption、over/under-design、architecture drift、missing failure mode、security/privacy、performance/reliability、data、
maintainability、UX/accessibility、evidence gap、reward gaming 和 future debt。

## 5. 十二维纠查

1. Goal alignment：是否解决真实目标而不是症状；
2. Requirement completeness：角色、状态、异常、撤销、权限、并发、恢复是否遗漏；
3. Assumption audit：开始时的假设是否 verified/invalidated/expired，若错影响什么；
4. Decision/alternative audit：为何选当前方案，新证据是否产生方案 C，rejected reason 是否仍成立；
5. Architecture/domain/data：边界、owner、不变量、依赖、兼容/迁移是否健康；
6. Complexity：是否把简单问题平台化，或为赶进度欠缺必要结构；
7. Failure/recovery：timeout、partial failure、duplicate、LOST、rollback/compensation 是否真实；
8. Security/privacy/compliance：权限扩大、信任边界、数据生命周期和 secret 是否改变；
9. Performance/reliability/cost：瓶颈是否有测量，优化是否触因，SLO/容量/运行成本是否退化；
10. Maintainability/operability：变化成本、测试 seam、可观测、runbook、toil 和技术债；
11. UX/accessibility：用户路径、认知负担、反馈、错误恢复和所有 interaction state；
12. Knowledge extraction：哪些 Observation 值得形成限定 scope 的 Lesson/Eval/Debt/Rule/ADR revisit。

## 6. Evidence-first 独立 Critic

Critic 第一遍只消费 requirement/decision、current source/artifact、tests/gates、runtime outcome 和 relevant ADR/debt，不接收
Executor 的辩护/总结，避免 framing/confirmation bias。先锁定初步 findings digest；第二遍才读取 Executor rationale，作为待验证
Claim，用于解释或发现信息遗漏，不能覆盖直接 Evidence。

R2 Critic 必须与主要 Planner/Implementer 分离，不能写产品代码、批准自己的风险或修改证据。模型退出 0、报告存在或多数
Agent 赞成均不构成 CLEAR；required dimensions 缺证据时 ABSTAIN/BLOCKED。

## 7. AssumptionAudit v1

每项保存 statement、source、confidence at plan/current、impact_if_wrong、reversibility、verification cost/plan/owner/expiry、
Evidence、`open/testing/validated/invalidated/expired`、affected decisions/contracts/tasks 和 next action。

处理策略：低风险用可逆默认并记录；中风险先调查；高风险先模拟/实验/权威确认；极高风险阻止不可逆 effect。Reflection
必须检查“未验证但被当作 guard/事实使用”的违规，并使依赖它的 Decision/Approval 失效或回流。

## 8. Strategy Review 与反方攻击

复查 original candidates、hard-constraint rejects、Pareto frontier、chosen sensitivity、实际成本/风险/结果。R1/R2 基于新
Evidence 至少问：是否存在新方案、是否优化了症状、是否存在更小充分方案、是否遗漏 no-change/rollback、六个月后的变化成本。

Adversarial lens 尝试构造最小反例、越权路径、竞争条件、数据破坏、依赖失效、恶意输入、用户误操作和 observation/reward
gaming。它输出可复现实验/测试提案，而不是无边界怀疑。

## 9. 输出路由：全部 proposal-first

```text
ReflectionReport
 ├─ fact/assumption correction → KnowledgeUpdateProposal → Governance Kernel
 ├─ causal hypothesis          → EvolutionCandidate / experiment
 ├─ repeated defect            → EvalCandidate / regression proposal
 ├─ maintainability finding    → TechnicalDebtItem proposal
 ├─ decision driver changed    → ADR revisit / superseding ADR proposal
 ├─ repeated lesson            → CandidateRule → shadow/promotion governance
 └─ required work              → New WorkIntent / CHANGES_REQUESTED
```

每条 route 有 `ReflectionRoutingReceipt`：finding/proposal、destination、accepted/rejected/deferred、owner、reason、created artifact
和 target state。生成报告而没有处理每个 required finding 不能进入 LEARNING/CLOSED。

Truth/Claim、hard rule、Accepted ADR、Eval gate 和生产状态都不能由 Critic 直接修改。一次观察不能升级为全局规则；Rule 晋升遵循
candidate→repeated lesson→heuristic→shadow→policy→gate→invariant，并有反例、范围、owner、审批、expiry/rollback。

## 10. Outcome 与学习防护

Reflection 关联 `DecisionCapsule → OutcomeObservation`，Outcome 保存 observation window、release/task/transaction identity、指标/
用户/incident 数据、baseline/counterfactual、confounders、missingness、delayed outcome 和 data quality。短期 PASS 不等于长期成功。

自适应策略只在 shadow/champion-challenger 中比较，防止把代理指标当目标、挑选有利样本或由同一 Agent自评分。权重/规则变化
有版本、效果假设、guardrail、drift monitor、rollback 和批准；发现 reward gaming、分布漂移或伤害率上升立即停用候选策略。

## 11. Capability，而不是命令蔓延

Canonical Registry 中定义 `reflection.meta`，input/output/risk/evidence/permissions/version 稳定；CLI 可以提供一个
`forge reflect` adapter，但 quick/architecture/security/adversarial/assumption 应是 profile/lens 参数，不是六套不同真值路径。
Web/API/Workflow 使用同一 Capability 和 receipts。

Node 16 是业务 owner；Node 10–13 提供专业 finding；Governance Kernel 应用知识/规则；00 接收新 WorkIntent。低风险可由单个
fresh Critic 装配多 lens；只有 MultiAgentBenefit（并行+专业+独立收益−协调/同步/合并风险）为正才拆多个 Critic。

## 12. 实施切片

1. 冻结 ReflectionCase/Report、AssumptionAudit、RoutingReceipt、OutcomeObservation schema 与 fixtures；
2. 只读 adapter 汇集现有 capsule/events/gates/review/QA/Evolve memory，先 R0 report-only；
3. 接入 `REFLECTING→LEARNING→CLOSED` 状态和 required route closure；
4. 增加 evidence-first fresh Critic，R1 覆盖两个真实 feature 场景；
5. R2 adversarial/alternative/counterfactual 与 human escalation；
6. proposal-only 接 Truth/Claim、Debt、Eval、ADR、Rule shadow 和 WorkIntent；
7. 用 replay/champion-challenger 校准发现率、误报、成本、用户打断和实际 outcome，才考虑策略自适应。

## 13. 验收

- R0 能发现“验收未执行却声称通过”、未关闭假设和明显 scope drift；
- R1 能区分 over-design 与必要结构，并把 finding 绑定到代码/合同/Evidence；
- R2 第一遍不受 Executor rationale framing，能生成至少一个可验证反例或诚实 ABSTAIN；
- invalidated assumption 使依赖 Decision/Grant/Approval 回流，不被 confidence 总分抵消；
- required finding 都有 RoutingReceipt；Critic 无法直接写 code/Truth/rule/ADR/production；
- 同一 capsule replay 不产生 effect；新模型/规则结果成为 branch，可比较而不改历史；
- 一次成功不会升级 hard rule；delayed/confounded outcome 不被当成因果；
- Reflection 循环有预算/进展/tripwire，不能成为永不结束的“无限反思”。
