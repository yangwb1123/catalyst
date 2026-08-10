# 治理与知识契约

> 状态：演进契约。0F-A 已交付 EvidenceRecord/KnowledgeClaim strict schema、跨语言 canonical codec 与纯 shadow validator；
> ADR-0046 的 0F-B–1 本地 exact-record journal 已在 SQLite v25/runtime/CLI/compatibility 边界内完成，并通过独立复审与 `forge accept`。
> ADR-0047 另已交付七类 KnowledgeClaim→CognitiveAtom 的确定性 pure shadow projection；它不是新治理记录，不进入 journal。
> ADR-0048 已交付 strict artifact-v1→EvidenceRecord pure shadow adapter；它不读取当前文件、不追加 journal，SQLite 保持 v25。
> ADR-0049 已交付 strict command-observation→gate/test EvidenceRecord pure shadow adapter；它不执行命令、不把 exit=0 当 PASS、
> 不追加 journal，SQLite 仍保持 v25。
> 第 2 节中标注“已交付”的两种 v1 记录、下述 source adapter 与 CognitiveAtom v1 投影属于当前合同；第 1、3 节及其后内容仍是目标态，
> 不声明 truth/authority、Context、Grant、Transition、语义 lifecycle/conflict/freshness view 或知识写回 runtime 已支持。旧 free-text memory 不得静默升级。

## 0. 当前实现边界（0F-A、0F-B–1、ADR-0047 projection 与 ADR-0048/0049 adapters 已完成；其余目标态未实现）

当前可执行的 governance record kind 仍只有 `EvidenceRecord` 和 `KnowledgeClaim` v1：

- strict wire schema：`docs/contracts/governance-evidence-claim-v1.schema.json`；
- canonical/identity/state policy：`.agent/engineering/governance-contracts.yml`；
- cross-language golden：`docs/contracts/fixtures/governance-evidence-claim-v1.json`；
- version decision：[ADR-0045](../../adr/0045-canonical-evidence-claim-contract.md)；
- universal checker：`harness/governance_contract_check.py`。

记录使用 `record_id`（不可变记录）、`aggregate_id`（稳定逻辑身份）、`sequence`（正整数版本）和
`supersedes_record_ids`（同 kind/aggregate 的精确旧记录）；digest 是内容身份，不是记录 ID。时间为整数毫秒，
置信度为 `confidence_micros`，canonical SHA-256 是无前缀小写 hex。当前只接收 registry `shadow_admissibility` 精确矩阵内的状态、
`untrusted_data` 和 `untrusted|observed` source trust；结构有效不等于事实、身份、批准、完成或 hard-gate 资格。

纯 checker 的唯一正结果是：

```text
STRUCTURALLY_VALID (shadow; no truth or authority attestation)
```

Checker 不写 Hub、不导入 Memory/ADR、不签发 authority，也不执行 knowledge apply、lifecycle transition 或 production effect。

### ADR-0048：Artifact provenance → EvidenceRecord v1 pure shadow adapter

[ADR-0048](../../adr/0048-artifact-provenance-evidence-adapter-v1.md) 增加首个 source adapter，但不增加新的 governance record kind 或
改变 EvidenceRecord v1 wire。输入必须是 exact compact canonical
`forgeos.governance.artifact-evidence-adapter/v1` request：`artifact` 精确包含 `_format=forgeos.artifact.v1` 等十一字段，历史空
`_format` 被拒绝；`binding` 显式提供 aggregate/project/scope/context/policy/source-tree/revision/sequence/sensitivity/subjects/supersedes。
duplicate/unknown/noncanonical/float/overflow、控制或 bidi Unicode、非 normalized repo-relative path、非法时间/hash/identifier/list 均失败关闭。

adapter 分别以 `forgeos.governance.artifact-provenance-source.v1\0` 和
`forgeos.governance.artifact-evidence-adapter.request.v1\0` 对 canonical artifact 与完整 request 做 SHA-256 domain separation。它把 artifact
时间点向下取整到非负 Unix 毫秒，固定 shadow tool principal/collector，绑定 artifact content/path、source snapshot 与调用方 subjects/
sensitivity，生成 status=`valid` 的 existing strict EvidenceRecord；完整输出再由 ADR-0045 validator 重新计算 self digest，并执行 exact
re-adaptation comparison。

Schema、golden 与 Universal checker 为：

- `docs/contracts/artifact-evidence-adapter-v1.schema.json`；
- `docs/contracts/fixtures/artifact-evidence-adapter-v1.json`；
- `harness/artifact_evidence_adapter_check.py`。

Python、Go、Rust 独立实现必须对同一 fixture 产生逐字节相同的 canonical source/request/Evidence 与 digest。唯一正结果是：

```text
ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect attestation)
```

adapter 不读取 artifact path 的当前文件，不证明 manifest 来自受信 Store，不认证 agent/model/principal/collector，不创建
KnowledgeClaim/CognitiveAtom，不满足 hard gate，不 append GovernanceRecordJournal、不写 SQLite/Memory/Knowledge，也不产生 network/process/
device/production effect。若调用方随后要求 durability，必须另行使用 ADR-0046 journal 并取得其 `stored|exact_replay` receipt；
`ADAPTED_SHADOW` 不能冒充 persistence。该切片没有 migration/backfill，Hub schema 和 SQLite 保持 v25；gate/test observation 由
ADR-0049 的独立版本承担，Evolve locator 仍需另行设计。

### ADR-0049：Command observation → Gate/Test EvidenceRecord v1 pure shadow adapter

[ADR-0049](../../adr/0049-command-observation-evidence-adapter-v1.md) 冻结 exact
`forgeos.command-observation/v1` envelope 和显式 Governance binding，再确定性映射为既有 `gate_result|test_run` EvidenceRecord。
observation 绑定 direct-exec argv、normalized cwd、opaque environment/tool/source-tree digest 声明、producer/run、整数毫秒时间、真实 exit、
stdout/stderr/combined 的 full/retained 摘要与截断计数。combined 只表示 producer 记录的 drain-event 顺序，不证明 OS emission 顺序；
`gate_result|test_run` 也只是 caller-declared 分类，不包含 criterion verdict。

Observation wire 能诚实保存 `exited|timed_out|cancelled`，但现有 Evidence command locator 只能无损保存真实非负 exit code，所以 adapter
只投影 `exited`；timeout/cancel/signal/spawn-failed 不得用负 sentinel 或猜测值伪装。command、observation、完整 request 和最终 Evidence
分别使用独立 digest domain；created-by 使用 request-derived `command-adaptation-<request_sha256>`，collector 仅复制 producer 声明。
Schema、golden 与 Universal checker 为：

- `docs/contracts/command-observation-evidence-adapter-v1.schema.json`；
- `docs/contracts/fixtures/command-observation-evidence-adapter-v1.json`；
- `harness/command_observation_evidence_adapter_check.py`。

Python、Go、Rust 独立实现必须产生逐字节相同的 canonical command/observation/request/Evidence 和四类 digest。唯一正结果是：

```text
ADAPTED_SHADOW (observation mapping only; no execution, pass, completion, truth, authority, claim, atom, persistence, or effect attestation)
```

adapter 不 spawn 命令、不读取 cwd/stdin/output/current tree、不验证 stream preimage、environment/tool/tree digest profile 或 producer 身份，
也不把 exit=0/PASS 文本映射为 gate authority。它不创建 Claim/Atom、不满足 hard gate、不 append journal、不写 SQLite/Knowledge，
不产生 process/network/device/production effect。该切片没有 migration/backfill；真实 gate/execbound producer integration 必须先冻结版本化、
无秘密 digest-preimage profile，Evolve locator 则使用另一 source contract。

### ADR-0047：CognitiveAtom v1 pure shadow projection

[ADR-0047](../../adr/0047-shadow-cognitive-atom-projection-v1.md) 在不改变 Evidence/Claim v1 wire 或 journal v1 的前提下，增加
`forgeos.aadm.cognitive-atom/v1` 的确定性 KnowledgeClaim→CognitiveAtom 重投影。投影前必须对整个 exact canonical
closed record set 重新执行 ADR-0045 的 shadow admissibility、reference、subject、supersession、cycle 与 digest 验证。

只有 `fact`、`constraint`、`decision`、`inference`、`assumption`、`hypothesis`、`unknown` 七类 Claim 生成 Atom；
`lesson` 和 `proposal` 只可作 source closure 成员。每个 Atom 逐字段复制已验证的 source identity、proposition、
epistemic state、references、validity 与适用的 integer confidence，并强制
`projection_mode=shadow`、`hardness=none`、`authority_ref=null`、`instruction_allowed=false`。schema、golden 与 universal checker 为：

- `docs/contracts/cognitive-atom-projection-v1.schema.json`；
- `docs/contracts/fixtures/cognitive-atom-projection-v1.json`；
- `harness/cognitive_atom_contract_check.py`。

它们的摘要与 Python/Go/Rust 参考路径由 `.agent/engineering/governance-contracts.yml` v6 固定。唯一正结果是：

```text
PROJECTED_SHADOW
(no truth, authority, instruction, hard-guard, transition, completion or effect attestation)
```

CognitiveAtom v1 只证明 pure reprojection 的 canonical bytes、identity、digest 和 source closure 一致；它不认证 principal/
collector/reviewer，不确认 truth，不发出 instruction/hard guard/Grant/Approval，不推进 transition/completion，不执行 effect。
它不是 `EvidenceRecord|KnowledgeClaim` 的第三种 journal record，不写 GovernanceRecordJournal、SQLite、Knowledge 或其他持久化。
prompt/model compiler、journal adapter、新 Atom source/type、hardness/authority 与完整 Kernel ABI 均仍是后续版本化目标。

### 0F-B–1：本地 GovernanceRecordJournal v1

[ADR-0046](../../adr/0046-local-governance-record-journal.md) 与
`docs/contracts/governance-record-journal-v1.schema.json` 定义 narrow local journal：一次 append 接收 1–256 条、总计不超过 1 MiB 的 exact
canonical record set 和不超过 256 UTF-8 bytes 的 idempotency key；先在事务外完成 v1 codec/语义验证，再原子保存 batch、exact records 与
可重建 structural head。首写 receipt 为 `stored`，同 key/同 bytes 返回原 append metadata 和 `exact_replay`；同 key/不同 bytes 或换 key 重复
同 records 均 conflict，不产生部分写入。

引用闭包的 public admissibility limits 是：最多 1,024 条 distinct stored dependency records；候选批次与已加载 dependency closure 的 canonical
bytes 合计最多 16,777,216；从候选沿 `derived_from_claim_record_ids` 到传递性 premise 最多 256 条边。三者只用于防资源耗尽；通过限制不证明
Evidence 充分、Claim 正确、producer 可信或状态具有 authority，超限则在原子 append 前失败关闭。

`GovernanceStructuralHead` 只表示 `(record_kind, aggregate_id)` 下已保存的最高连续 sequence，固定
`interpretation=structural_sequence_only`。它不解释 Claim status、Evidence validity、时效、冲突、producer/collector 身份、知识采纳、authority 或
hard gate。Inspection 默认只返回 batch/ordinal/identity/digest/byte-count/time metadata；exact record 必须显式 `--include-record`，且 reveal 后仍按
不可信数据处理。

持久化迁移只允许 additive SQLite v25 empty tables，不回填 ADR/Memory/旧 Hub 数据。Read-only journal 命令要求 current v25 且不创建/迁移；append
可把受支持的 v24 数据库迁到 v25。该狭窄 runtime、migration、CLI、compatibility 与 adversarial tests 已完成并通过独立复审和
`forge accept`；这只关闭 0F-B–1 structural persistence，不实现 truth/lifecycle/conflict/freshness/authority。

`forge-init`/`forge-upgrade` 只分发 contract、Skill 和 shadow checker 资产，不安装 Rust `forge-runtime` executable 或 SQLite journal。只有检测到项目批准、
兼容 `forgeos.governance-journal/v1` 的 `forge-runtime` 后，Agent 才能运行 `forge-runtime governance journal ...`；缺失或不兼容必须记
`not_executed`，没有匹配的 `stored|exact_replay` receipt 就不得声称已持久化。

## 1. 通用 Governance Envelope（目标态，未实现）

目标上，Atom、Claim、Evidence、GraphSnapshot、Report、Grant、Review、Approval、KnowledgeUpdate 和 Transition 共享一组信封职责。
ADR-0047 的 CognitiveAtom v1 只是独立 Claim 投影 wire，ADR-0048/0049 也只是把两种互不相同的 source 映射到既有 Evidence wire；
它们都不表示该通用信封或完整 Kernel ABI 已实现。
下图只是**不可序列化的概念图**，不是 `forgeos.governance/v1` wire shape，也不得输入当前 checker；任何共同信封实现都必须采用新版本并另作兼容决策：

```yaml
future_envelope_concept:
  versioned_identity: immutable record + stable aggregate + sequence
  provenance_binding: project + scope + source + policy + context + principal + run
  kind_specific_payload: strict version-owned schema
  lifecycle_state: kind-scoped state + reasons + validity window
  integrity_binding: version-owned canonicalization + domain-separated digest
```

规则：严格拒绝未知/重复字段；ID 和 digest 有 domain separation；摘要排除自身后计算；载重输入任一 digest 改变，
review/approval/grant/cache 自动失效；敏感级别在投影前执行，不把 secret/PII 先放进 Prompt 再“要求不要泄露”。

**规范词汇。** Schema 和 adapter 只使用 `ContextPackage`、`CapabilityGrant`、`ApprovalRecord`、
`KnowledgeUpdateProposal`；`ContextManifest`、`AuthorityGrant`、`AgentCapabilityGrant` 仅作为历史迁移 alias，严格 v1
输入拒绝 alias。本文后文写 “Grant” 时专指 `CapabilityGrant`。Capability、Agent、Role、Lifecycle Node 和
ExecutionTarget 是不同实体，名称不能互相暗示权限或物理位置。

## 2. 认识论模型：Evidence 不等于 Fact

### 已交付 EvidenceRecord v1

| 字段 | 规则 |
|---|---|
| `metadata.record_id` | 不可变记录的有界 identifier；与 `aggregate_id`、`sequence` 和内容 digest 分工 |
| `spec.evidence_type` | `repo_locator/test_run/gate_result/runtime_metric/external_source/human_attestation/artifact` |
| `subjects` | Evidence 覆盖的 subject / graph-node identifier；必须覆盖被引用 Claim 的 `spec.subject`，不是 Claim record ID |
| `collector` | `human/operator/service/tool`、collector ID/version、run 与参数 SHA-256；只是声明，不是身份认证 |
| `observed_at/source_snapshot` | 观察时间与代码/环境/部署版本 |
| `locator` | 严格六字段：`locator_type/locator_ref/content_sha256/exit_code/line_start/line_end`；type 必须与 evidence type 映射，repo 行号对可选 |
| `status.state/reason_codes` | `valid/invalid/unavailable/expired`；缺工具只能 unavailable，不能 PASS |
| `status.valid_* / spec.sensitivity` | 有效窗口和 `public/internal/confidential/restricted` |
| `source_trust/content_role` | trust domain/level、`trusted_control/untrusted_data`；外部、仓库正文、日志默认 `untrusted_data`，不得解释为指令 |
| `artifact_sha256` | 原始有界证据产物摘要，正文按权限另存 |

#### Post-v1 locator enrichment（目标态，尚未版本化）

symbol、command argv digest/cwd、metric query/window/sample/unit、publisher/retrieved-at 等更丰富定位信息尚不是 v1 字段。
若未来需要，必须通过新版本或独立有摘要的 locator artifact 引入，不得把这些名称塞进 strict v1 记录。

Evidence 只说明“观察到了什么”。一个 test PASS 支持特定行为 claim，不证明整个模块正确；一个文档存在不证明实现符合。
`directness` 必须编码为 `direct/derived/attested`，并由 evidence type × collector policy 决定，不能靠自由文本声称“直接”。
不可信正文中的“忽略规则/运行命令”等内容始终是数据；只有由受信 policy lane、当前用户授权或签名控制产物产生的
instruction atom 才可控制执行。高风险外部内容先隔离/审查，任何 snippet 都不能覆盖宪法、Grant 或任务合同。

### 已交付 KnowledgeClaim v1

| 字段 | 规则 |
|---|---|
| `claim_type` | `fact/constraint/decision/inference/assumption/hypothesis/lesson/proposal/unknown` |
| `spec.subject/predicate/object_type/object_value` | 可稳定查询的主语、关系、类型和值；object type 必须与值一致 |
| `status.state` | 唯一 kind-scoped state；不得再复制一份 `epistemic_state` |
| `spec.supporting/contradicting_evidence_record_ids` | 证据引用分离、排序、互斥；冲突不能静默覆盖 |
| `spec.derived_from_claim_record_ids/reasoning` | inference 保留前提与推导，不伪装为直接观察 |
| `spec.confidence_micros` | 只允许 assumption/hypothesis/inference 显式使用；整数 0..1,000,000，null 不等于 1.0 |
| `spec.owner/validation_plan/review_by_unix_ms` | 假设必须说明由谁、何时、怎样验证以及若为假有什么影响 |
| `status.valid_* / metadata.supersedes_record_ids` | 过期或被替代 claim 不进入强约束 Context |

当前 shadow 门禁：即使声明了 direct Evidence，也拒绝 `fact.confirmed`；Assumption/Inference/Proposal/Unknown 和所有其它 Claim
均不能满足 hard gate；Decision authority 只保留未来字段形状，不代表批准。Unknown 必须进入 question/risk queue。0F-B–1 exact journal 与
replay 仍不足以启用 confirmed/accepted/waived；必须先交付 authenticated identity、Approval/Grant、SoD、语义 lifecycle/conflict/freshness、
revocation，并另作版本决策。

合法的 claim type × state 是封闭矩阵：Fact 为 `candidate/confirmed/contested/stale/retracted/superseded`；Constraint 为
`candidate/active/waived/expired/superseded`；Decision 为 `proposed/accepted/rejected/deprecated/superseded`；Inference 为
`candidate/supported/contested/invalidated/expired`；Assumption/Hypothesis 为
`open/testing/validated/invalidated/expired`；Lesson 为 `candidate/observed/repeated/retired/promoted`；Proposal 为
`draft/submitted/adopted/rejected/superseded`；Unknown 为 `open/investigating/resolved/accepted_risk`。其它组合严格拒绝。

## 3. System Knowledge Graph v1

Graph 是由权威源可重建的索引，不是覆盖代码、schema、ADR 和部署清单的第二事实源。

**节点类型。** `business_capability, actor, journey, requirement, business_rule, bounded_context, aggregate, entity,
value_object, domain_event, use_case, api, event_contract, schema, table, column, module, package, symbol, job, queue,
deployment_unit, environment, policy, gate, adr, test, runtime_signal, incident, debt, owner`。

**边类型。** `owns, contains, realizes, implements, exposes, calls, depends_on, reads, writes, persists_to, publishes,
consumes, deployed_as, constrained_by, verified_by, observed_by, governed_by, decided_by, supersedes, affects`。

每个 Node 使用不依赖可变文件名的 stable ID，保存 type、qualified name、owner、lifecycle、source locators、data
classification、claim/evidence refs 和 validity。每个 Edge 保存 from/to/relation、compile/runtime/data/ownership/policy
类别、`confirmed/derived/assumed` 认识状态、extractor/version、evidence 和 freshness。

`GraphSnapshot` 必须绑定 source revision/tree digest、node/edge digest、extractor versions、各语言/模块/API/DB/部署/
运行覆盖率、unresolved nodes/edges 和 expiry。缺边或 stale snapshot 时 Impact 输出 `UNKNOWN`，不能输出“无影响”。

## 4. Change Impact / Cost / Risk v1

`ChangeImpactReport` 至少包含：

- change/requirement IDs、baseline/candidate snapshot、proposed operations；
- seed nodes 与每条 direct/transitive `impact_path` 的完整 edge path、evidence 和 certainty；
- business/domain、API/event consumer、data/migration/history、authorization/privacy/audit、backend/frontend、
  deployment/operation、test/docs surfaces；
- compatibility：`compatible/additive/breaking/unknown`；reversibility 与 rollback/forward-fix；
- assumptions、open questions、counterfactuals、knowledge coverage 和 stale/missing edge；
- required roles、gates、human approvals、DAG constraints 和 recommendation。

`ChangeCostEstimate` 分开估算 implementation、test/review、migration/backfill、cross-team coordination、rollout/
rollback、operations/support、future maintenance。每项给 range、basis、confidence、unknown 和 `low/medium/high/critical`
band；不以伪精确单点工时取代范围。

`RiskAssessment` 的 hazard 记录 scenario、likelihood、impact、blast radius、reversibility、exposure、detectability、
security/privacy/data、inherent severity、risk owner、mitigation owner/target/due trigger、mitigation evidence/validation status、
residual severity、acceptance authority/expiry 和 `RiskAcceptance` ref。Risk 用最高强制 floor，不用平均数稀释 Critical；
缓解没有当前有效证据前 residual risk 不下降；无 owner/期限/验收权限的 mitigation 仍是 proposal；“改动小”不等于“风险低”。

评估分两阶段，避免生产者死锁：00 Intake 只产生用于路由的 `ImpactPreScan`，不得满足 G3；适用的 02–07 产物完成后，
00 的 **Assessment Join** 由只读 impact assessor 重新读取冻结设计和当前 Graph，产出最终 `ChangeImpactReport`、
`ChangeCostEstimate`、`RiskAssessment` 与 `AssessmentReceipt`。L3/L4 assessor 不得是主要设计者。G3 只接受这四个绑定同一
source/context/design digest 的 final artifact；08 只能细化执行估算和缓解任务，不能成为首次风险生产者。L0/L1 若设计
节点不适用，也必须由 Assessment Join 给出逐项 N/A 证据和 final assessment，不能把 pre-scan 改名冒充 final。

建议基础 materiality：

| Level | 典型范围 | 最低流程 |
|---|---|---|
| L0 Trivial | 文案/注释，无行为变化 | impact pre-scan + relevant gate |
| L1 Local | 单模块，无公共合同/数据影响 | plan + dev + fresh review + test |
| L2 Feature | 完整功能或跨前后端 | 01–14 的相关分支 |
| L3 Systemic | 跨模块、DB、公共 API、安全/SLO | full impact + 专项设计/review + human gate |
| L4 Critical | 生产数据、支付、跨租户、隐私、不可逆迁移 | separation of duties + 多人批准 + rehearsal/observation |

级别是策略 floor；发现更高风险时只能上调。低成本多租户 seam 可记录为 option，但“未来也许需要”不能自动添加
`tenant_id`、事件总线或抽象层。

## 5. ADR v2

Markdown 保留可读正文，机器 frontmatter 至少包含：

```yaml
adr_id: ADR-...
status: proposed|accepted|rejected|superseded|deprecated
scope: []
owners: []
approvers: []
accepted_at: ...
acceptance_id: approval-...
context_claim_ids: []
decision_drivers: []
decision: ...
alternatives: []
assumption_ids: []
evidence_ids: []
affected_node_ids: []
consequences: []
risks: []
compatibility: ...
rollout: ...
rollback: ...
validation_plan: []
implementation_refs: []
supersedes: []
superseded_by: []
expires_at: ...
revisit_triggers: []
```

Accepted ADR 不原地重写结论；新 ADR 负责 supersede，反向索引维护 `superseded_by`。`accepted_at/acceptance_id` 绑定真实
批准，而不是创建时间；临时决策必须有 expiry。ADR 证明“做过决定”，不证明代码已遵守；Architecture Fitness/
ADR Compliance 独立验证。Decision 与 Fact 分开，Rejected alternative 和少数意见保留，避免下一 Agent 重犯旧讨论。

## 6. TechnicalDebtItem v1

必需字段：ID/title/type（architecture/code/test/security/data/ops/docs/dependency）、locations/graph nodes、source finding/
ADR/incident、evidence、root cause、deferred reason、principal/remediation scope、interest signals、impact/risk、owner、
created_at、due date/trigger、plan、acceptance、status、waiver/expiry、related items。

合法状态：`open/accepted/planned/in_progress/verified/closed/wont_fix`。无 owner、到期触发器和验收不得进入 accepted；
Critical security/data-integrity debt 不可普通延期；关闭必须有当前证据，不因文件删除自动关闭。典型 interest signal 包括
touch frequency、change lead time、defect/incident、coupling growth、manual toil 和 exception age。

## 7. EngineeringConstitutionRule v1

每条规则有 `rule_id/title/category/level/applies_when/measurement/tool/comparator/threshold/enforcement/rationale/
remediation/owner/evidence/waiver/effective window/supersedes/tests`。

三层必须分开：

- `hard_gate`：可确定执行的安全、数据、依赖与基础体积红线，失败即 block；
- `review_trigger`：复杂度/耦合/模式适用性等信号，要求分析，不机械砍代码；
- `guidance`：默认实践，可用证据充分的替代方案。

Waiver 必须有 scope、理由、风险 owner、approver、expiry、compensating control 和 debt link；到期默认失效。现有
`.agent/AGENTS.md` 与 harness 继续作为当前硬门，新增目录不能绕开它们。

## 8. SoftwareHealthSnapshot v1

默认维度可使用 Architecture、Code Quality、Security/Privacy、Performance、Reliability、Test、Operability 和
Documentation/Knowledge；权重按系统类型配置且总和为 100。Performance 与 Reliability 必须保留独立 verdict，
Operability/Documentation 不得因权重为零从证据面消失。每项 metric 都有 `PASS/FAIL/NA/UNKNOWN`、observed/target、
evidence、freshness、coverage、dimension score 和 trend。

Snapshot 同时输出：revision/window、total、`evidence_coverage`、unknown count、hard blockers、release eligible、debt
count/age/interest、与上期可比性和 recommended actions。

规则：全 N/A/UNKNOWN 不得绿色；任何 Critical blocker 都使 `release_eligible=false`；综合分不能抵消单维红线；缺失
证据不按满分；score 只用于趋势/排序，不替代 acceptance、review 或发布批准。

## 9. CapabilityGrant / ApprovalRecord v1

权限默认 deny。`CapabilityGrant` 绑定：

- `principal_type/id`：`agent/service/human/operator`，以及 run/change/task/node/capability；
- effects：`repo.read/write, process.exec, network.read/write, secrets.read, migration.generate/apply,
  release.plan/execute, approval.request/decide, knowledge.propose/apply, policy.propose/write, placement.plan,
  target.inventory/probe/reserve/execute`；
- exact paths/commands/origins/environments、declared emits 和 deny list；
- timeout/output/token/cost/call budgets；
- source/context/policy digest；issued/approved by、issued/expiry、`transferable:false`；
- separation-of-duty constraints 和 consumed-action receipts。

Grant 只能由非 Agent 的 Governance Kernel/PDP 按已签名 policy 和 authenticated bootstrap identity 签发。两次 issuance
不可混淆：00 先提交 discovery/design/planning `GrantRequest`，Kernel 签发最小只读/产物/proposal Grant；Device dry-run 若适用，
它只是标准 `CapabilityGrant(effect=placement.plan, reserve=false, execute=false)`。08 在 plan finalization 再提交 effectful
`GrantRequestSet`，Kernel 校验 SoD、final risk、budget、scope 后签发/拒绝执行 Grant，并写 `GrantIssuanceReceipt`，随后才允许
进入 AUTHORIZED。Agent、role 或 workflow 不能自签；bootstrap authority 仅允许读取 intake 输入和提交首个请求，不允许扩大
自身权限。

`ApprovalRecord` 独立于 Grant，至少含 approval ID、`approve/reject/abstain`、human/operator principal identity、权威来源、
change/gate/effect/environment scope、source/context/policy/plan/impact/risk/artifact digests、conditions、RiskAcceptance refs、
issued/expiry/revoked time、signature/attestation 与 SoD proof。批准只对精确摘要有效，不可转让或重放；撤销、过期、风险变化
立即失效。

典型边界：Planner/Reviewer 只读；Coding 只写任务开发路径；Migration 默认只生成 SQL，可在低环境另授权验证；Release
只生成/验证批准交付包。**ForgeOS 不签发 production `migration.apply/release.execute`**，只校验外部 authority domain
签发的 operator grant/approval 并导入结果 receipt；Approver 不能批准自己的 L3/L4 实现或风险接受。执行前校验 Grant，
执行后做工作树/外部 effect postflight；越界或 effect 不确定进入 quarantine。

## 10. KnowledgeUpdateProposal / Receipt v1

节点字段 `memory_updates` 始终表示提交 `KnowledgeUpdateProposal`，绝不表示直接修改事实账本。Proposal 对每个 mutation
保存 operation、target claim/node/debt/ADR、before/after、proposed epistemic type/state、evidence/contradiction、reason、
source/context/artifact digest 和 proposer grant。所有节点若要提交，必须显式持有 `knowledge.propose`；只有 Governance
Kernel 的独立 service principal 可持有 `knowledge.apply`。

Kernel 先做 schema/provenance/conflict/freshness/policy 校验，再按认识上限处理：Agent 可提出 candidate fact、inference、
assumption、hypothesis、lesson、proposal 和 unknown；不可确认业务/法律事实、接受决策、豁免规则或修改 hard rule。可复验工具
观察可由 policy-controlled verifier 确认为限定 scope 的 repo/runtime fact；业务事实由 domain owner、人类 attestation 或权威
系统确认；Decision 必须绑定 Approval/ADR。结果写 `KnowledgeUpdateReceipt(applied/rejected/needs_review)`，保存理由、旧/新
版本和 reviewer。冲突不会 last-write-wins；CLOSED 必须引用 receipt，而不是只引用提交意图。

## 11. ContextPackage v1

Context Engine 输出可审计 manifest，Prompt 只是其有界投影。Package 包含 task/change/phase/role、source snapshot、
requirement/acceptance、confirmed facts、assumptions/unknowns、relevant ADR、impact subgraph、API/data/deployment contracts、
active debt/findings、constitution rules、permissions/prohibitions、input digests、selected evidence snippets、选择理由、明确
排除/截断及原因、token budget/actual、redactions、freshness/expiry 和 canonical context hash。

每个 snippet 另带 source/trust/instruction classification、normalization/delimiter 和 `instruction_allowed`（默认 false）。
Context builder 将 system/policy/user-authorized instruction lane 与 untrusted data lane 物理分开；仓库、网页、日志、issue、
tool output 内的命令性文字不可升级到 instruction。疑似 prompt injection 或跨信任域载重内容进入 quarantine/人工审查。

装配优先级：当前任务与验收 → 硬约束/权限 → confirmed facts/active decisions → impact paths/contracts → relevant code/tests →
debt/runtime evidence → 可选历史。Contradicted/stale claim 不静默进入；遗漏载重上下文时 fail；每个 Agent 输出绑定
`context_sha256`。

## 12. ReviewCase / ConflictResolution v1

`ReviewCase` 绑定 change/source/context/artifact digests、reviewer identity/role/model、independence proof、review dimensions、
实际运行 vs 推断 checks。Finding 使用两个正交字段：`finding_severity=BLOCKER/MAJOR/MINOR/INFO` 表示交付处置，
`domain_risk=CRITICAL/HIGH/MEDIUM/LOW/NA` 表示安全/数据/运营危害；同时保存 title/location、fact vs inference、evidence、
failure scenario、likelihood、required/optional、fix/test、owner/status。任何 BLOCKER，或 policy 映射为 required 的
CRITICAL/HIGH domain risk，使聚合 verdict 不能 APPROVE；不同 domain 的原始等级不得互相改名。Verdict 为
`APPROVE/CHANGES_REQUESTED/BLOCKED/ABSTAIN`。

Review Loop：

```text
Generate → deterministic gates → fresh role reviews
         → normalized findings → adjudicate/assign
         → Fix → affected gates + regression → fresh re-review
         → approve or fail-closed
```

不以多数票裁决。优先级：法律/宪法硬门/人身与数据安全 → 已确认需求与数据完整性 → 安全、可逆、兼容和生产风险 →
Accepted ADR/所有权边界 → 可测 SLO → 成本速度偏好。Critical、不可逆、waiver、事实争议必须交人。

`ConflictResolution` 保存互斥 positions、role/claim/evidence/rule refs、assumptions、option cost/risk/reversibility、chosen、
rejected reasons、dissent、adjudicator authority、approval、expiry/revisit 和 validation plan；结果写 ADR/plan，而不是只写聊天结论。

## 13. 状态机与 TransitionReceipt

```text
DRAFT → NEEDS_EVIDENCE → BASELINED → DESIGN_DRAFTED → ASSESSED → DESIGNED
      → PLANNED → AUTHORIZED → IMPLEMENTING → VERIFYING → REVIEWING → RELEASE_READY
      → RELEASING → OBSERVING → REFLECTING → LEARNING → CLOSED
```

低风险流程不跳状态：ApplicabilityDecision 可让某一阶段以带理由和证据的 N/A 快速通过，仍写 TransitionReceipt；因此 L0
也能合法表达“无设计/无生产发布”，而不是从 ASSESSED 非法跳 CLOSED。

| from | allowed to | 恢复约束 |
|---|---|---|
| DRAFT | NEEDS_EVIDENCE, NEEDS_INFO, REJECTED, SUPERSEDED | intake 可直接拒绝冲突/越界请求 |
| NEEDS_EVIDENCE | BASELINED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED | baseline 必须有冻结 source |
| BASELINED | DESIGN_DRAFTED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED | 设计 N/A 也进入 DESIGN_DRAFTED 并带 ApplicabilityDecision |
| DESIGN_DRAFTED | ASSESSED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED | final Assessment Join 消费冻结的适用设计草案 |
| ASSESSED | DESIGN_DRAFTED, DESIGNED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED | impact 使方案失效则回 DESIGN_DRAFTED；否则通过 G3 |
| DESIGNED | PLANNED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED | Node 08 生成计划与 GrantRequestSet |
| PLANNED | DESIGNED, AUTHORIZED, NEEDS_INFO, BLOCKED, REJECTED, SUPERSEDED | Kernel/PDP 签发 Grant 后通过 G4；计划变化回 DESIGNED |
| AUTHORIZED | IMPLEMENTING, BLOCKED, QUARANTINED, SUPERSEDED | no-op/只读任务仍进入 IMPLEMENTING 并记录 N/A effect |
| IMPLEMENTING | VERIFYING, CHANGES_REQUESTED, BLOCKED, QUARANTINED, SUPERSEDED | artifact/source 漂移使 Grant 失效 |
| VERIFYING | REVIEWING, CHANGES_REQUESTED, BLOCKED, QUARANTINED, SUPERSEDED | UNKNOWN 不能当 PASS |
| REVIEWING | RELEASE_READY, CHANGES_REQUESTED, BLOCKED, REJECTED, QUARANTINED, SUPERSEDED | required finding 为零 |
| CHANGES_REQUESTED | DESIGN_DRAFTED, ASSESSED, DESIGNED, PLANNED, IMPLEMENTING, VERIFYING, BLOCKED, REJECTED, SUPERSEDED | receipt 必须给唯一 `rework_target`；重算 context/grant/gates |
| RELEASE_READY | RELEASING, BLOCKED, QUARANTINED, SUPERSEDED | 无发布任务以 N/A receipt 通过 RELEASING |
| RELEASING | OBSERVING, BLOCKED, QUARANTINED, SUPERSEDED | LOST/不确定 effect 只能 quarantine |
| OBSERVING | REFLECTING, CHANGES_REQUESTED, BLOCKED, QUARANTINED, SUPERSEDED | 观察绑定实际 artifact/release；非运行任务观察验收证据 |
| REFLECTING | LEARNING, CHANGES_REQUESTED, BLOCKED, SUPERSEDED | 至少 R0 Reflection；重大遗漏开 rework/new Change |
| LEARNING | CLOSED, BLOCKED, SUPERSEDED | 只消费已裁决的 knowledge/rule/eval/debt receipts |
| NEEDS_INFO | receipt.resume_state, BLOCKED, REJECTED, SUPERSEDED | 保存进入前状态；新 Evidence 后只回该状态或更早安全状态 |
| BLOCKED | receipt.resume_state, REJECTED, SUPERSEDED | 仅有权主体解决 blocker、刷新摘要/Grant 后恢复 |
| QUARANTINED | BLOCKED, VERIFYING, REJECTED, SUPERSEDED | 仅 incident/reconciliation authority；禁止自动重试 |

`CLOSED/REJECTED/SUPERSEDED` 为终态，无外出边；后续工作创建新 WorkIntent。任何非终态可由有权 controller 转
SUPERSEDED。每次迁移写 from/to、preconditions 的 PASS/FAIL/NA/UNKNOWN、applicability/rework/resume target、evidence、
waiver、source/context/policy/artifact digests、actor/grant、timestamp/hash。非法跳转失败关闭；approval 与 source/artifact
精确绑定；不确定外部 effect 进入 quarantine 且禁止自动重试。

关键完成门：BASELINED 有冻结快照；DESIGN_DRAFTED 有适用设计草案/N/A receipt；ASSESSED 有 final impact/cost/risk/unknown/
AssessmentReceipt；DESIGNED 通过 G3 且有必要 ADR/migration/rollback/test；PLANNED 有 DAG/WorkPackage/GrantRequestSet；
AUTHORIZED 有 Kernel/PDP grant/approval 并通过 G4；VERIFYING 使用真实当前 gates；REVIEWING 的 required reviewer 独立且阻断项为零；
RELEASE_READY 绑定 artifact/SBOM/migration/rollback/freshness；REFLECTING 完成相应强度的决策链与假设审计；CLOSED 引用
knowledge/debt/health/rule/eval 的 applied/rejected receipts。

## 14. RuntimeObservation / EvolutionCandidate v1

RuntimeObservation 保存 environment/service/version/deployment digest、metric/log/trace/incident/user-feedback type、query/window/
aggregation/sample/value/unit/SLO、evidence digest、sensitivity、observed_at/freshness。

EvolutionCandidate 保存 trigger observation/claim、problem、causal hypothesis、counterfactual、options、expected benefit、experiment/
measurement、guardrail/abort、impact/cost/risk、rollback、required authority、owner/status。`p95=800ms` 只能生成待验证假设，
不能直接推导“加 Redis”；实验结果必须验证/推翻 claim 并更新 debt/ADR/health。
