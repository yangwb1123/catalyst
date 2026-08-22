# 治理与知识契约

> 状态：演进契约。0F-A 已交付 EvidenceRecord/KnowledgeClaim strict schema、跨语言 canonical codec 与纯 shadow validator；
> ADR-0046 的 0F-B–1 本地 exact-record journal 已在 SQLite v25/runtime/CLI/compatibility 边界内完成，并通过独立复审与 `forge accept`。
> ADR-0047 另已交付七类 KnowledgeClaim→CognitiveAtom 的确定性 pure shadow projection；它不是新治理记录，不进入 journal。
> ADR-0048 已交付 strict artifact-v1→EvidenceRecord pure shadow adapter；它不读取当前文件、不追加 journal，SQLite 保持 v25。
> ADR-0049 已交付 strict command-observation→gate/test EvidenceRecord pure shadow adapter；它不执行命令、不把 exit=0 当 PASS、
> 不追加 journal，SQLite 仍保持 v25。
> ADR-0050 已交付 strict Evolve repo-locator→EvidenceRecord pure shadow adapter；它不读取当前 repo/report 或确认扫描结论。
> ADR-0051、ADR-0052 与 ADR-0053 已交付显式 opt-in Unix local gate/Evolve/Go lexical dependency observation producer，默认 capture 关闭且不签发 PASS、扫描结论、selected build、architecture/impact judgment 或完成裁决。
> ADR-0054 已在 SQLite v27/runtime/CLI 交付可重建 local semantic view、authority-free lifecycle subset、显式 caller-time
> validity/freshness 标签、冲突候选与 Assumption/Hypothesis validation schedule；它不签发 truth、winner、verdict、Grant 或 transition authority。
> ADR-0055 已交付 strict authority-free `ContextPackage v1` pure builder；它不读取 source、不调用 provider、不持久化或签发任何 authority。
> ADR-0056 已交付 `CapabilityGrant v1` contract-only wire 与 authority-neutral declared assessment；它不认证 issuer/proof/principal/policy/Approval，
> 不评价 revocation/usage，也不签发 authorization、permission、pre/postflight、receipt、persistence、execution 或 effect authority。
> ADR-0057 只交付一个 operator 部署、repo 外 pin root/key/state 的 authenticated bootstrap `repository-reader/v1` + `repo.read` Grant issuance profile；
> 它不开放 plan finalization、其他 effect/capability/environment、Approval、revocation、usage、pre/postflight、PEP 或 effect execution。
> ADR-0058 已交付单一 authenticated manifest-bound repo-read execution profile；`forge accept` 的完成裁决不是 execution authority。
> ADR-0059 的 ApprovalRecord v1 contract-only 实现已按窄边界交付；它不提供 effective approval authority。
> ADR-0060 的 TransitionReceipt v1 contract-only 实现已按窄边界交付；它不提供 authoritative state、ledger 或 transition authority。
> ADR-0061 的 KnowledgeUpdateProposal v1 contract-only 实现已按窄边界交付；它不提供 authoritative current head、adoption、apply 或 Knowledge authority。
> ADR-0062 已交付 exact ADR-0053 graph 上的 authority-free local Go package reverse ImpactPreScan；package 缺口与 system impact 保持 UNKNOWN，且不提供 GraphSnapshot、final Impact/Cost/Risk 或 Assessment authority。
> ADR-0065 已交付一个 authority-free GraphSnapshot v1 partial projector/checker profile；它只重建 exact ADR-0053 selected-module lexical module/package 子图，coverage 为 PARTIAL，system/freshness 为 UNKNOWN，不满足 G3 或 Assessment Join。
> ADR-0066 已交付独立的 Local Go lexical test-source GraphSnapshot profile；它只增加 package-scoped test source-set nodes 与 module→test edges，Go/test coverage 均为 PARTIAL，不解析或执行 tests，也不产生 result、coverage 或 verification authority。
> ADR-0067 Proposed-only ADR v2 已交付新文档 strict frontmatter/body/digest checker 与 current-`writes_adr` candidate validation；声明的 owner/approver/Claim/Evidence/Graph refs 不认证、不解析。Go 保留既有 ADR baseline-integrity snapshot，但旧文档不做 v2 parse/retro-validation/migration；Accepted/immutability/supersession/compliance/persistence/lifecycle authority 均未交付。
> 第 2 节中标注“已交付”的两种 v1 记录、下述 source adapter/local producer/CognitiveAtom 投影、第 9 节 ADR-0056–0061 窄子集、第 11 节 ADR-0055 子集与 ADR-0068 staged singleton Registry evaluator 属于当前合同；
> 其余内容仍是目标态，不声明 truth/authority-bearing Grant、权威 Transition、provider prompt 或知识写回 runtime 已支持。旧 free-text memory 不得静默升级。

## 0. 当前实现边界（0F-A、0F-B–1、ADR-0047–0068 的限定切片已完成；其余目标态未实现）

### ADR-0067 Proposed-only ADR v2

`forgeos.architecture-decision-record/v2` 只验证一个显式 supplied 新 Proposed Markdown 文档：LF-only framing 中的一行 exact compact canonical JSON frontmatter、固定且非空的 Context/Decision/Consequences/Validation/Limitations body、物理 basename/ADR ID/H1 title 一致性，以及 body/self 两个 domain-separated digest。Schema 只是 closed shape；strict checker 还复算排序、cross-reference、UTF-8 byte bounds、locator、heading 和完整 digest semantics。

Universal checker 只读 caller 指定文件或 byte-pinned physical golden；Catalyst Go validator 只接入 current `writes_adr` attempt 唯一新增候选，同时保留原有 legacy baseline-integrity snapshot。旧 ADR 不被当作 v2 parse/retro-validation/migration 对象。owner/approver refs 是 responsibility/required-review declarations，不是 authenticated principals、SoD 或 ApprovalRecord；Claim/Evidence/Graph-node refs 只校验 shape，不解析 truth 或 coverage。空引用数组只表示未声明。该切片不生成 graph edge，不接受或持久化决策，也不实现 immutable Accepted body、supersession/compliance state machine、transition、execution 或 effect。

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
ADR-0049 的独立版本承担，Evolve locator 则由 ADR-0050 的独立 pure shadow adapter 承担。

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
不产生 process/network/device/production effect。ADR-0051 另以显式 observed API 交付 secret-scrubbed environment、resolved executable、
bounded-interval Git working-source 与 raw stream capture；它会执行固定本地命令但不提供 sandbox/effect containment，只产生 local observation
package。ADR-0052 的 Evolve producer 另绑定完整 canonical report preimage、共享 `git-worktree-source-tree-v1` bounded-interval 非原子 source observation 与 zero-or-more locator（跨 relation/path 不去重）；固定 read-only Git subprocess 不认证 binary 或证明 sandbox/egress/effect，且不自动调用 ADR-0050。
ADR-0053 已交付 producer 只用标准库 parser 形成 `selected-module-all-regular-go-files-union-v1` lexical graph；runtime platform 固定 Unix、no Go command/module-cache/network profile，非 Unix 在观察前失败关闭，Git 未认证且无 sandbox/effect containment；它不解析 selected build、module availability、architecture/impact，也不创建 Governance record 或 persistence。
ADR-0062 只消费 caller 提供的 exact ADR-0053 graph bytes/digest/run ID 与 changed paths，不调用 live producer；它只产生 local package lexical
reverse fixed point、完整 induced local edges 与 deterministic shortest witnesses。任何 graph/path gap 都保持 UNKNOWN，system impact 永远 UNKNOWN，
不创建 GraphSnapshot、final ChangeImpactReport/Cost/Risk/AssessmentReceipt、Governance record 或 persistence。
ADR-0065 另以独立 exact envelope 把同一类 caller bytes 投影为 module/package GraphSnapshot 子图：stable identity、source/extractor declarations、
resolved/unresolved 双射、ADR-0062 crosswalk、11-surface coverage 与 freshness 都必须由 semantic checker 完整重建。它不读取 live repository 或
clock，不把 declared provenance 升级为认证事实；其它十个 surface 未观察，system/freshness 恒 UNKNOWN，不能替代 final impact assessment。
ADR-0066 以独立 request/envelope API 复用该 foundation，精确为每个 `test_files` 非空 package 增加一个 lexical test source-set node 与
module→test structural edge；旧 ADR-0065 API/Schema/golden bytes 不变。Go/test resolved records 被互斥计数到两个 PARTIAL surface；不生成
package→test、`verified_by`/`observed_by`，也不从文件名/import 推断 test case、execution、outcome、coverage 或 verified subject。

### ADR-0050：Evolve repository locator → EvidenceRecord v1 pure shadow adapter

[ADR-0050](../../adr/0050-evolve-repo-locator-evidence-adapter-v1.md) 把 caller-declared exact repo locator/content/scan context/producer/source
observation 与显式 Governance binding 确定性映射为既有 `repo_locator` EvidenceRecord；locator/source/request/Evidence 使用分离 digest domain。
Schema/golden/checker 是 `docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json`、`docs/contracts/fixtures/evolve-repo-locator-evidence-adapter-v1.json`
与 `harness/evolve_repo_locator_evidence_adapter_check.py`；唯一正结果为 `ADAPTED_SHADOW (locator mapping only; no file/report verification, scan judgment, completion, truth, authority, claim, atom, persistence, or effect attestation)`。
它不读取 current repo/report、不解析 symlink、不验证 digest preimage、不确认 scan judgment/completion，不创建 Claim/Atom、不满足 hard gate、
不 append journal、不写 SQLite/Knowledge 或产生 effect；ADR-0052 与 ADR-0053 的交付都不把这些非能力升级为真实验证或 authority。

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

它们的摘要与 Python/Go/Rust 参考路径由当前 `.agent/engineering/governance-contracts.yml` v15 固定。唯一正结果是：

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

Journal persistence 本身在 SQLite v25 additive 创建 empty tables，不回填 ADR/Memory/旧 Hub 数据；v26 随后只放宽 endpoint pinning，ADR-0054
再把 current schema 提升为 v27。Read-only journal/semantic 命令要求 current v27 且不迁移；append 可把受支持的 v24、canonical journal
v25、历史 endpoint-only v25 或 v26 原子迁移到 v27。普通 journal read 继续使用拒绝 sidecar 的 immutable opener；semantic read 使用 exact-v27
`mode=ro + query_only` live opener 和单一 Deferred snapshot，不执行 Hub 逻辑写，但 SQLite 可创建/移除 transient empty WAL/SHM sidecar 或协调 SHM
read-lock bytes，因此 fully read-only filesystem 可以返回 unavailable。ADR-0046 的狭窄 runtime、migration、CLI、compatibility 与 adversarial tests 已完成并通过
独立复审和 `forge accept`；它本身只关闭 0F-B–1 structural persistence，semantic 行为由下面独立版本承担。

`forge-init`/`forge-upgrade` 只分发 contract、Skill 和 shadow checker 资产，不安装 Rust `forge-runtime` executable 或 SQLite journal。只有检测到项目批准、
兼容 `forgeos.governance-journal/v1` 的 `forge-runtime` 后，Agent 才能运行 `forge-runtime governance journal ...`；缺失或不兼容必须记
`not_executed`，没有匹配的 `stored|exact_replay` receipt 就不得声称已持久化。

### ADR-0054：Local Governance Semantic View v1

[ADR-0054](../../adr/0054-local-governance-semantic-view-v1.md)、
`docs/contracts/governance-semantic-view-v1.schema.json` 与对应 golden fixture 在 exact journal 之上冻结
`forgeos.governance-semantic-view/v1`。每个 structural aggregate tail 都重投影为带 exact record identity/digest、project/scope、declared state、
validity interval 与 update time 的 semantic head；Claim 另带 type/subject/predicate、object/conflict-key digest、review time 和完整 validation plan。
投影 digest 使用独立 domain，materialized row 不是唯一语义来源：read/replay/successor append 会从 exact immutable record 重算并逐字段比对，缺失、
多余或漂移均按 corruption 失败关闭。

SQLite v27 在同一 append transaction 写 exact records、structural heads、semantic heads 与 validation jobs；v26→v27 会先重验全部 durable batch、
每个 aggregate 的完整历史与 reference relation，再从 exact journal records 原子 backfill；任何 relation/lifecycle/digest/cardinality/schema/
final-validation 错误都完整回滚到 v26。Public read 不迁移数据库，内部 rebuild 不作为 CLI mutation 暴露。当前 CLI 为：

```text
forge-runtime governance journal view KIND AGGREGATE_ID --as-of-unix-ms N
forge-runtime governance journal conflicts --as-of-unix-ms N [--limit N]
forge-runtime governance journal validation-jobs --as-of-unix-ms N [--due-only] [--limit N]
```

所有 read 始终选择 exact current structural tail；`as_of_unix_ms` 只评价该 tail 的 declared interval，绝不回选历史 sequence。调用方必须显式提供
非负 caller time，禁止隐式 wall clock。Temporal precedence 固定为 `not_yet_valid`、`validity_expired`、
`validation_overdue`、`review_overdue`、`fresh`；这些词只比较记录声明的时间。Conflict 只分组同 type/project/scope/subject/predicate 且 active-at-time、
object digest 不同的 Claim tail，不选择 winner。Validation job 只为当前 Assumption/Hypothesis 的完整计划确定性排期，不执行方法、不采集 Evidence、
不认证 owner，也不签发 verdict。Public list/group 上限为 100，Claim-head integrity scan 上限为 10,000。单 aggregate 的完整 history、transitive
reference closure 与所有 owning-batch siblings 共用 1,024 unique records/16 MiB canonical bytes；multi-head scan 另共用 65,536 records/256 MiB/
1,000,000 work units。任何超限都报告 unavailable，不能返回 partial/empty、corrupt 或“无冲突”。

Durable lifecycle 只覆盖 ADR-0045 无 authority 的 subset：sequence one 可取该 Claim type 已允许的任一 shadow state，successor 支持 same-state
continuity、Fact `candidate↔contested`、
Assumption/Hypothesis `open→testing`、Proposal `draft→submitted`、Unknown `open→investigating`；semantic identity 字段不可在 successor 中漂移。
confirmed/accepted/active/waived/validated/resolved/adopted 等需要 authority 的 promotion 仍被拒绝。所有公开结果固定
`interpretation=semantic_projection_only_no_truth_or_authority`，不能满足 hard gate、确认事实、采纳知识、批准 Decision、授权 effect 或替代
`forge accept`。

## 1. 通用 Governance Envelope（目标态，未实现）

目标上，Atom、Claim、Evidence、GraphSnapshot、Report、Grant、Review、Approval、KnowledgeUpdate 和 Transition 共享一组信封职责。
ADR-0047 的 CognitiveAtom v1 只是独立 Claim 投影 wire，ADR-0048–0050 也只是把三种互不相同的 source 映射到既有 Evidence wire；
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

## 9. Capability Registry / CapabilityGrant / ApprovalRecord v1

### ADR-0068 authority-neutral Capability Registry v1 evaluator（已交付）

[ADR-0068](../../adr/ADR-0068-authority-neutral-capability-registry-v1.md) 与
`docs/contracts/capability-registry-v1.schema.json` 冻结一个 `status=staged` 的 singleton physical profile，唯一 entry 是
`local-go-package-impact-prescan/1`。Go/Python strict validator/resolver 独立重建 content-set→contract→entry→registry→request→assessment
digest chain；`forge capability-registry` 只接受显式 Registry/Request 输入，physical checker 只读取 Registry 明确声明的 refs。Registry
semantic digest 为 `23b9acd4133598cd1404c78c71f694b4a99c398652e95c21896a507be5ecacf4`；Schema 与 golden physical pin 分别为
`f5c5c5abc68e9c5f5d80dce66bb5b97e4e4dedc8cc69189bcc28612991f1ea81` 与
`0ce4929ad82ce70ef0520be80b7bd3eaf47f5ff1205d0a53e12fbe1115ed11b5`。

治理元数据把 evaluator/CLI 标为 delivered，但 ADR v2 frontmatter 仍是 `proposed`，wire 仍是 `staged`，不会伪造 acceptance 或
lifecycle promotion。`resolved_exact` 只表示 declared reference/contract bytes 匹配；它不认证 Registry、owner、tests 或 implementation，
不导入/投影 planning-only 140-item catalog，不生成 catalog→package adapter，不选择/执行 implementation，不激活 Grant/PDP，不构造
CapabilityInvocation，不加载 plugin，不做 runtime routing、persistence、transition 或 effect。历史 `repository-reader/1` 仍按普通未注册 ID
返回 negative assessment，而不是 bootstrap authority。

### ADR-0069 Planning Capability Ownership Projection v1（已交付）

[ADR-0069](../../adr/ADR-0069-planning-capability-ownership-projection-v1.md) 与
`docs/contracts/planning-capability-ownership-projection-v1.schema.json` 冻结一个
`planning_only` pure projection。Python/Go 独立 strict parser/projector 从 caller-supplied
exact `capability-catalog.v1.yml` 与 `capability-skill-map.v1.yml` bytes 重建 source→request→binding→projection
digest chain；同一 golden 精确得到 17 nodes、145 occurrences、140 unique fine capabilities、38 declared packages 与
140 bindings。每个 capability 恰有一个 primary owner，重复 lifecycle occurrence 保留全部 node IDs；logical adapter ref
只按 `.agent/skills/` + owner + `.md` 派生，并固定 `physical_resolution:not_performed`、
`skill_availability:not_evaluated`。

产品 CLI 只有 `forge capability-ownership project --catalog FILE|- --mapping FILE|-`；option 可交换且恰好一个 stdin。
Usage error 返回 2，input/semantic rejection 返回 1，三者在首个 stdout write 前完成且失败 stdout 为零；成功输出 exact
canonical projection+LF。底层 stdout short/write failure 返回 1，但 stream 不可事务化，任何 partial bytes 都不是有效 artifact。
Python `validate`/`--golden` 只是 universal/internal checker。Universal scaffold 复制 exact sources、ADR/Schema/golden 与 Python
checker/tests，不复制 Catalyst Go runtime，不从 38 owner names 生成物理 Skill/host adapter，也不把已有同名 Markdown 当作
resolution evidence。

治理 Registry v24 将它只列为 shipped pure evaluator；ADR 本身仍为 `proposed`。该交付关闭的唯一 roadmap 项是 complete unique
primary-owner coverage 与 logical adapter-ref derivation。它不修改 ADR-0068 singleton Registry，不认证 source/owner/repository，不生成或
验证 portable `SKILL.md`/package/adapter，不激活 Grant/PDP，不构造 CapabilityInvocation，不选择/执行 implementation，不加载 plugin，
不做 runtime routing、persistence、transition 或 effect attestation。

### ADR-0070 Local Project Source Snapshot v1（窄 package 切片已交付）

[ADR-0070](../../adr/ADR-0070-local-project-source-snapshot-v1.md) 与
`docs/contracts/project-source-snapshot-v1.schema.json` 冻结一个 Linux-only、显式 opt-in
的 local Git worktree source observation。Producer 对 tracked stage-zero 与 nonignored
untracked universe 做两次完整端点 capture，只在 final sealed manifests 与 counts 完全相同后
产生 production。Allowed single-link regular bytes、tracked-absent facts、metadata-only hashed
exclusions、ignored count 与 exact 12-surface coverage 进入 content-addressed result；excluded raw
path、symlink target、raw content、ignored locator、Graph/configuration/deployment semantics不进入。

固定 policy 只约束 collector worktree-leaf reader：matched sensitive/control path 在 leaf
lstat/open/read/readlink 前排除。它不是 content DLP；arbitrary allowed names 仍可能含 secret，
PATH-selected Git 未认证/未 sandbox，repository config/control-metadata 读取在该 guarantee 之外。
两端点相等也不是 atomic filesystem snapshot、current HEAD、writer quiescence 或 system
completeness 证明。结果固定 `atomic:false`，freshness/currentness/system completeness UNKNOWN，
tracked/untracked/ignored-count coverage PARTIAL，authority/permission/truth/persistence/effect 全 false。

`skills/project-snapshot/` 是 closed source-distributed portable package；pure decoder 只承诺
source-portable，`.agent/skills/project-snapshot.md` 只是 Linux capture 路由 adapter。Universal
init/upgrade 复制 package、ADR、Schema、golden、strict Python checker/tests，但不复制 `forge-core`
producer、不安装宿主 Skill、不提供 worktree/process permission。Unsupported host 或 runtime 不存在
固定 exit 3/`not_executed`，已存在但不兼容/执行失败固定 exit 1；package checker 缺
descriptor-relative no-follow primitive 时同样 fail closed 为 exit 1。禁止 `find`/Git
archive/status/hash-loop fallback。Registry v25 将 strict checker列为 shipped evaluator、Linux capture 列为 shipped local
producer，但不修改 ADR-0068 singleton Capability Registry，且不生成 GraphSnapshot、Grant/PDP、
CapabilityInvocation、runtime routing、persistence、transition 或 effect。

Implementation roadmap 只勾选 38-package parent 下的 `project-snapshot` nested item；parent 与其余
37 packages、formal role/cross-reference runtime、plugin manifest/signature/sandbox/upgrade lifecycle
及 risk-level routing/permission/review eval 保持开放。

### ADR-0071 Portable Context Engineering Skill（窄 package 切片已交付）

[ADR-0071](../../adr/ADR-0071-portable-context-engineering-skill.md) 在不修改 ADR-0055
ContextPackage v1 wire、Schema、golden、bounds、digest domain 或 Python/Go/Rust semantics 的前提下，
交付 closed `skills/context-engineering/` source package。内部 assembler 只接受 exact canonical stdin
bytes 和冻结 fixture counter，零参数、成功输出一个 canonical package 加 LF；它不发现 repository、
workspace、environment、network、provider、database、clock 或 model context，不编译 live prompt。

独立 checker 以 manifest 绑定 16 个 package files 的 path/type/link/mode/size/hash/direct-ref 与重复
identity observation；缺 descriptor-relative no-follow primitive 固定 exit 1 fail closed。Python `-I`
只从 import source 排除 script/current directory、`PYTHONPATH` 与 user site，不禁用、认证或隔离
system site、stdlib、interpreter startup、host 或 publisher。一次 checker success 与随后独立 assembly
不存在原子 check-to-use 绑定，host 仍须在自己的保护边界内防止 mutation 或重新验证。

Registry v26 只增加 portable delivery metadata、Skill/manifest/ADR refs、manifest physical pin 与独立
shadow/non-load-bearing integrity detector；不增加 producer、runtime profile、live tokenizer/provider/model、
Grant/PDP/Approval、truth/instruction/completion/persistence/routing/effect authority。Universal init/upgrade
复制 ADR、closed package 与既有 ContextPackage universal assets，不复制 Catalyst Go/Rust runtime，也不安装
host Skill。Implementation roadmap 只新增勾选 `context-engineering` nested item；parent 与其余 36 packages
继续开放。

### ADR-0072 Portable Evidence Claim Validation Skill（窄 package 切片已交付）

[ADR-0072](../../adr/ADR-0072-portable-evidence-claim-validation-skill.md) 不修改 ADR-0045
EvidenceRecord/KnowledgeClaim v1 wire、Schema、golden、bounds、digest domain 或 Python/Go/Rust
semantics，只交付 closed `skills/evidence-claim-management/` source package。内部 validator 以零参数
adapter 读取 already-authored exact canonical record-set stdin 直到 explicit EOF，只输出固定
structural shadow marker；它不观察或 author records，不修复、排序、补 digest、返回 record bytes
或读取 repository/environment/network/provider/model/database/clock/subprocess/journal/semantic view/proposal。

独立 checker 以 manifest 绑定 18 个 package files 的 path/type/link/mode/size/hash/direct-ref 与重复
identity observation；缺 descriptor-relative no-follow primitive 固定 exit 1 fail closed。Python `-I`
只排除 script/current directory、`PYTHONPATH` 与 user site，不禁用、认证或隔离 system site、
stdlib、interpreter startup、host 或 publisher。Checker 与随后 validation 不存在 atomic check-to-use。

Registry v27 只增加 validation-only delivery metadata、Skill/manifest/ADR refs、manifest pin 与 shadow/
non-load-bearing integrity detector；portable prose 故意不进入 authenticated context routes。Source-only
init/upgrade 复制 ADR、closed package 与治理 checker，不安装 host Skill，不增加 observation/
authorship/journal/persistence/runtime profile，也不产生 truth/instruction/Grant/PDP/Approval/completion/
routing/transition/execution/effect authority。Implementation roadmap 只勾选 `evidence-claim-management`
nested item；parent 与其余 35 packages 继续开放。

### ADR-0073 Portable Policy Authority Declaration Assessment（窄 package source-governance 切片已交付）

[ADR-0073](../../adr/ADR-0073-portable-policy-authority-declaration-assessment-skill.md)
不修改 ADR-0056 CapabilityGrant 或 ADR-0059 ApprovalRecord 的 wire、Schema、golden、bounds、
digest domain 与 Python/Go/Rust semantics，只交付 closed `skills/policy-authority/` source package。
两个独立零参数 adapter 各自读取 exact canonical request 到 explicit EOF，并只输出 computed canonical
assessment + one LF；没有 combined dispatcher，也不读取 ambient repository/environment/clock/identity/
policy/approval/revocation/usage/runtime。

独立 checker 以 manifest 绑定 30 个 package files 的 path/type/link/mode/size/hash/direct-ref 与重复
identity observation；三入口都要求 `-I/-B`，但这些 flags 不认证 system site、stdlib、interpreter、
host 或 publisher，checker 与随后 assessment 也不是 atomic check-to-use。Fixture 只含 key identifier 与
proof-shaped declaration，不含 public-key verification material。

Registry v28 只增加 declaration-assessment delivery metadata、Skill/manifest/ADR refs、manifest pin 与
shadow/non-load-bearing integrity detector；scope 不变，两个 assessment adapter 不是 detector，portable
prose 故意不进入 authenticated context routes。Source-only fresh/legacy scaffold 只复制 source；本切片不安装 host Skill，
不签发/批准/激活 Grant，不使 Approval 生效，不调用 ADR-0057/0058、Kernel/PDP/PEP，也不产生
authorization/permission/completion/persistence/routing/transition/execution/effect authority。Implementation
roadmap 只勾选 `policy-authority` nested item；parent 与其余 34 packages 继续开放。

### ADR-0074 Portable ADR Governance Proposed Document Validation（窄 package source-governance 切片已交付）

[ADR-0074](../../adr/ADR-0074-portable-adr-governance-proposed-document-validation-skill.md)
不修改 ADR-0067 Proposed-only ADR v2 wire、Schema、golden、bounds、digest domain 或
Python/Go semantics，只交付 closed `skills/adr-governance/` source package。Validator 接受
exactly one caller-supplied lexical basename argument，并从 stdin 读取 exact document bytes 到
explicit EOF；basename 保持 ADR-0067 独立字符串相等语义，但不证明 physical file、repository
path 或 identity，也没有新增 JSON/base64 request envelope。

独立 checker 以 manifest 绑定 25 个 package files 的 path/type/link/mode/size/hash/direct-ref 与
重复 identity observation。两个入口都要求 `-I/-B`，但 flags 不认证或隔离 system site、stdlib、
interpreter、host 或 publisher，checker 与随后 validation 也不是 atomic check-to-use。Portable
adapter 不读 path、不扫描 repository、不 author、repair、normalize、reseal、accept、supersede 或
persist ADR；Go `writes_adr` 仅作为 Catalyst semantic parity evidence，未复制进 package。

Registry v29 只增加独立 Proposed-document validation delivery metadata、Skill/manifest/ADR refs、
manifest pin 与 shadow/non-load-bearing integrity detector；`architecture_decision_record_v2` block、
scope 与 route 不变，validator 不是 detector，portable prose 故意不进入 authenticated routes。
Source-only fresh/legacy scaffold 只复制 source，不安装 host Skill 或 runtime，也不产生 identity、
ownership、approval、truth、Graph、acceptance、compliance、immutability、lifecycle、persistence、
transition、execution 或 effect authority。Implementation roadmap 只勾选 `adr-governance` nested
item；38-package parent 与其余 33 packages、Accepted lifecycle/compliance、DECISIONS query 与 legacy
import 继续开放。

### ADR-0056 已交付的 contract-only subset

[ADR-0056](../../adr/0056-capability-grant-v1-contract-only.md) 与
`docs/contracts/capability-grant-v1.schema.json` 冻结 strict `CapabilityGrant v1` envelope、单 effect vocabulary、typed allow/deny scope、
budget/source/context/policy/task/time/SoD bindings 及 authority-neutral declared assessment。Python、Go、Rust 对同一 exact golden 重建相同 canonical
bytes、digests 与 declared relations；deny、scope miss、binding drift、budget overflow 或 declared window 外都不能产生 authorization decision。

该 evaluator 固定不认证 issuer、proof、principal、policy、ApprovalRecord 或 digest preimage，不评价 revocation、usage、reservation/consumption，
不做 preflight/postflight，不写 journal/Hub，不生成 audit receipt，不持久化或执行 effect。Structurally valid envelope、inside-window、scope-covered 与
budget-within-ceiling 都不是 `authorized`、`allowed`、`active` 或 permission。Registry 将 `CapabilityGrant` 保持为
`shipped_contract_only_kinds`；它既不是普通 shipped runtime kind，也不再是未冻结的 planned wire。

### ADR-0059 ApprovalRecord v1 contract-only（已交付）

[ADR-0059](../../adr/0059-approval-record-v1-contract-only.md) 与 `docs/contracts/approval-record-v1.schema.json` 冻结 closed record、
declared target/request/assessment、detached-proof content identity 和与 ADR-0056 `ApprovalRef` 完全兼容的
`(approval_id, approval_sha256, authority_domain)` 投影。Python、Go、Rust 对同一 golden 重现四个 exact digest 与 assessment bytes；
Registry v16 将 `ApprovalRecord` 与 `CapabilityGrant` 同列 `shipped_contract_only_kinds`；该合同切片已经正式 `forge accept` 验收，
但 Accepted/DONE 只表示 strict wire 与 pure evaluator 交付，不产生 runtime authority。

Evaluator 只消费 caller-supplied canonical bytes 与显式 `evaluated_at_unix_ms`；不读取 `.forge/<stage>.approved`、`--approved`、
`actor_hint`、workflow/session/environment/ambient clock。Principal、proof、authority、condition、RiskAcceptance、revocation 与 SoD 只校验
shape/声明一致性，不认证现实身份、签名、权限或生命周期。唯一结果是 `ASSESSED_APPROVAL_DECLARATIONS_ONLY`；effective approval、
authorization、permission、persistence、transition、execution 与 effect 均不可用。Reference match 不改变 CapabilityGrant 的
`approval_state=not_evaluated`。

### ADR-0060 TransitionReceipt v1 contract-only（已交付）

[ADR-0060](../../adr/0060-transition-receipt-v1-contract-only.md) 与 `docs/contracts/transition-receipt-v1.schema.json` 冻结 closed 23-state
vocabulary、TransitionReceipt/target/request/assessment identity、显式 predecessor、applicability/rework/resume 声明，以及 ADR-0056 Grant 与
ADR-0059 Approval reference compatibility。Registry v17 将 `TransitionReceipt` 加入 `shipped_contract_only_kinds`，只把
`KnowledgeUpdateProposal` 留在 `planned_kinds`；该分类表示 strict wire/纯 evaluator 已交付，不是 shipped runtime 或权威状态机。

Evaluator 只消费 caller-supplied canonical bytes 与显式 `evaluated_at_unix_ms`。Listed edge、chain/continuity、PASS/NA、applicability、recovery、
Grant/Approval ref match 都只是 declared relation；controller/actor/current state/precondition/Evidence/Waiver/Grant/Approval 不认证，policy/authorization
decision 固定 `none`，permission/persistence/transition/execution/effect/completion attestation 固定 false。它不导入 workflow/marker/flag/actor hint/
local journal/Hub state，不添加 ADR-0056 effect，不 append ledger、执行 recovery、推进 lifecycle、持久化或产生 effect。该切片已经正式
`forge accept` 验收；Accepted/DONE 只表示 contract-only 交付，完成裁决不会成为 Transition authority。

### ADR-0061 KnowledgeUpdateProposal v1 contract-only（已交付）

[ADR-0061](../../adr/0061-knowledge-update-proposal-v1-contract-only.md) 与
`docs/contracts/knowledge-update-proposal-v1.schema.json` 冻结 closed proposal、七字段 target、request/assessment identity、exact ADR-0045
Evidence/Claim reachable closure，以及按 aggregate 排序且唯一的 `create|supersede` 声明。Registry v18 把 `KnowledgeUpdateProposal` 加入
`shipped_contract_only_kinds` 并清空 `planned_kinds`；这表示 wire 已作为 contract-only slice 交付，不是 shipped runtime、authoritative current
knowledge、adoption 或 apply。

Evaluator 只消费 caller-supplied canonical bytes 与显式 `evaluated_at_unix_ms`。Matching Grant ref、Context binding、record-set closure、mutation、
scope、artifact 与 time 都只是 declared relation；proposer/Grant/Context 不认证，Evidence truth、current head、conflict、freshness、policy/authority
不评价。结果固定 `ASSESSED_KNOWLEDGE_UPDATE_DECLARATIONS_ONLY`；truth/adoption/authorization/permission/persistence/apply/receipt/execution/effect
全部不可用。Universal scaffold 只复制 ADR/schema/fixture/Python pure checker/tests/governance wiring，不安装 Go/Rust runtime、journal/database/head state、
keys、Kernel 或 Knowledge apply/receipt。该切片已经正式 `forge accept` 验收；Accepted/DONE 只表示 contract-only 交付，完成裁决不会成为 Knowledge authority。

### ADR-0057 已交付的 authenticated bootstrap repo-read profile

[ADR-0057](../../adr/0057-authenticated-bootstrap-repo-read-grant-issuance.md) 不扩大 ADR-0056 的 pure evaluator，而是另增
`authenticated_bootstrap_repo_read_grant_issuance_v1` runtime profile。只有 operator 以非 Agent authority 部署独立 `forge-kernel`，并在 repo 外显式 pin
trust root、issuer key 与 durable state 后，Kernel 才可认证 signed policy 和 signed GrantRequest；输出严格限于 `bootstrap_planning`、
`repository-reader/v1`、单一 `repo.read`、exact paths、bounded budget/TTL、`local|development|test` 的 signed `CapabilityGrant` 与 durable signed
`GrantIssuanceReceipt`。Result 的 `delivery_disposition` 只有 `stored|exact_replay`，证明精确签发记录已持久化或精确重放，不证明 repository read 已执行。

该 runtime 仅支持 Unix，非 Unix 在读 authority 输入/key 前失败关闭。Authority root/state 必须 euid-owned exact `0700`，root 为全祖先无 symlink 的绝对规范路径；authority/repository endpoint 按祖先 filesystem identity 双向不重叠，caller repository absolute source、首次 resolved path 与 opened directory identity 在整个 session 持续绑定，不能只比较 path text。state/leaf 为封闭相对路径，叶子为 euid-owned、single-link、无特殊权限位的 exact `0600` regular file。本地 `0600` 与 effective-UID 检查只缩窄文件访问，不提供独立 OS principal、HSM 或 production key custody；same-UID loopback 测试也不能证明完整
production trust boundary。Kernel 不生成 trust root/policy/key，scaffold/upgrade 不安装 `forge-kernel`、root、key 或 state；兼容外部 runtime 缺失时只能
`not_executed`。结构 checker/detector 仍为 shadow/non-load-bearing，`forge accept` 是完成裁决而不是 issuer。
Golden 的三把 private seed 是公开测试材料；生产 Kernel 拒绝 exact fixture root、任何携带 fixture public key 的 root 及 fixture issuer key，运行时测试使用临时生成的非公开 key。
Ledger high-water 只相对当前 signed snapshot 检测 wall-clock rollback；V1 没有 TPM、remote witness 或 external monotonic anchor，因而不抵抗可替换
authority state 的本地管理员回放旧但签名合法的 snapshot。

Plan finalization、其余 20 effects/其他 capabilities、staging/production、Approval、revocation、usage/reservation、pre/postflight、PEP/effect execution、
ContextPackage/provider/Transition/Knowledge integration、key provisioning/rotation、remote/HA/multitenancy 与完整 Governance Kernel/PDP 均保持目标态。

### ADR-0058 authenticated bootstrap repo-read execution profile

[ADR-0058](../../adr/0058-authenticated-bootstrap-repo-read-execution.md) 不把 issuance policy 重解释为 effect consent。它要求一个与 ADR-0057 root/key
完全分离、repo 外 operator-pinned 的 execution root，并分别认证 exact signed execution policy 与 invocation。`allow/activate_once` 才可预留；
`deny/do_not_activate` 不产生 usage state。唯一 effect 仍是 `bootstrap_planning` 的 `repository-reader/v1` + `repo.read`，environment 仅
`local|development|test`，且 1..16 个排序 exact path 必须逐项匹配 manifest 的 regular/raw-byte length/SHA-256。

ExecutionPolicy/Invocation key 各只签对应 artifact；`execution_receipt_sign` 恰好只签 domain-separated UsageReceipt 与 complete UsageLedger snapshot，
不得签 Policy、Invocation、issuance 或其它 artifact。新增 key/usage 必须新 profile 并重新外部 pin。

该 runtime 只支持 Linux amd64/arm64，并以 `openat2` 的 `BENEATH|NO_XDEV|NO_SYMLINKS|NO_MAGICLINKS` 组合打开 leaf，无 fallback walker。
Single-use ledger 固定 reservation→intent→terminal，每一阶段 durable persist/reopen；active orphan 只 quarantine，永不 resume/reread。Reservation 必须
fresh；已开始的 terminal 可越 Invocation expiry，但 success elapsed 仍受 timeout。Ledger 不存 raw；完成后首送才含 raw。Exact replay 可用 canonical
policy+invocation pair 或双 64hex self-digest，但 digest mode 仅查 terminal，miss/mixed 在 manifest 前失败；命中只返回相同 receipt/metadata。Cooperative timeout 不保证 blocked syscall
硬截止；管理员整体替换 signed state 可回滚。Mutable bytes 的 best-effort clear 不证明 strings/GC/kernel/downstream secure erasure，也不提供 process
isolation/HSM。Production 必须拒绝 fixture root/keys。

Reservation 必须早于 bind/verify 等 repository metadata I/O。每个 signed transition 都独立采 wall clock 并推进 high-water；clock failure 不复用旧时间，
active tail 留待下次 quarantine。Content reader 对 statfs/openat2/stat/read/reopen 前间后检查 timeout；grantstate identity revalidation 作为 composite 只在
整体前后检查。任何 blocked kernel op 都可能越预算，返回后 timeout 优先且不交付 raw。

Filesystem allowlist 只冻结直接可见的 superblock magic；它不证明 allowed filesystem 的物理 locality，也不证明 overlayfs lower/upper backing
是 local。Local backing 是 operator 部署前提，`network_bytes=0` 只表示 effect 无显式网络请求。Reservation/intent/terminal 的 pre/post-publish
六格 fault matrix 必须收敛且永不二次读取 repository。

Initial leaf open 与 post-read reopen 都先用 confined `O_PATH|O_CLOEXEC` probe + fstat regular/exact-size/nlink1，再用
`O_RDONLY|O_NONBLOCK|O_CLOEXEC|O_NOATIME|O_NOCTTY` active open并重验 SameFile/invariants。EUID/`CAP_FOWNER`/FS 不支持 noatime 时失败关闭。
静态 FIFO/device 不会 active open，但两次 open 间的并发替换仍可能触发 FIFO rendezvous/driver side effect。无 special nodes、无不可信
`CAP_MKNOD`、执行期间无不可信 namespace writer 均是 operator 前提；v1 不证明这些部署条件或 driver isolation。

Pre-reservation platform check 是纯 build-tag GOOS/arch 判断，不访问 cwd/filesystem。Bound repository 的 visible-superblock/openat2 preflight 只在
durable reservation 后、effect intent 前执行，失败落 signed quarantine。

Pinned execution root、receipt key 与 usage-ledger namespace 是不可分割部署。V1 不支持 root/signing-key rotation、trust-epoch migration 或
usage-state clear/rebase；root/key 变化时旧 ledger 失败关闭。Fresh root/state 不继承 spent history，也不能消费旧 namespace 下的 Grant。需要连续性的
轮换必须新建 profile/ADR，并由外部 witness 迁移完整 single-use history。

Scaffold/upgrade 只复制 ADR/schema/fixture/Python structural checker/governance tests，不安装 Go binary、root、key 或 state；缺 runtime 为
`not_executed`。Detector 仍 shadow/non-load-bearing。Approval/revocation、write/network/process/secret/target、staging/production、Context reassembly、
general PDP、remote/HA/multitenancy 仍不可用；完整 `forge accept` 已裁决该窄 profile 完成，但它不是 execution authority。

### 完整目标态

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

### ADR-0055 已交付的 authority-free pure subset

[ADR-0055](../../adr/0055-shadow-context-package-v1.md) 与
`docs/contracts/context-package-v1.schema.json` 冻结 strict `ContextPackageBuildRequest v1`/`ContextPackage v1`。Builder 只消费 caller
显式提供的 exact canonical request；task/source/time/policy/routes/tree/tokenizer identity 全量进入 request/cache digest。它按固定 category、
priority 与 source ID 顺序先保留 required source，再选择 optional source；required 缺失、不合格或超预算失败关闭，optional 以唯一 omission
reason 收据退出。Declared UTF-8 byte redaction 先于 truncation/selection/token/digest，typed JSON lane 物理分开 `instruction_candidates`、
`trusted_context` 与 `untrusted_data`，所有 snippet 固定 `instruction_allowed=false`。

Python、Go、Rust 对 `docs/contracts/fixtures/context-package-v1.json` 产生 exact 相同的 snippet/projection/context/request/cache digests；token counter
身份必须与 request 完全一致，package/cache hit 必须完整重装配，不能只信 key。唯一正结果为：

```text
ASSEMBLED_SHADOW (no truth, authority, instruction, permission, approval, completion, persistence, or effect attestation)
```

该 builder 无 repository/network/process/provider/database I/O，不认证 source/trust/freshness/redaction discovery，不调用模型、不写 journal/Hub。
真实 Context Router、semantic retrieval、prompt compiler、production tokenizer、Grant/PDP/Approval binding 与 durable context store 仍是后续目标。

### 完整目标态

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

### ADR-0075 Portable Knowledge Graph Curation（窄 package source-governance 切片已交付）

Registry v30 只绑定 closed `knowledge-graph-curation` manifest、两条既有 ADR-0065/0066 projector refs 与 source-only delivery。两个 zero-argument adapter 各自消费现有 exact canonical request；不接受 raw graph、fixture/envelope wrapper、tagged union 或 profile dispatcher。Coverage 仍为 PARTIAL，system/freshness 仍为 UNKNOWN，test source-set 不表示 test execution/outcome/coverage/verification；无 live repository/build/test capture、route、runtime、persistence、impact、truth 或 authority。

### ADR-0076 Portable Change Impact Cost Risk（lexical prescan 窄 package source-governance 切片已交付）

Registry v31 只绑定 closed `change-impact-cost-risk` manifest、既有 ADR-0062 projector ref 与 source-only delivery。唯一 zero-argument adapter 只消费现有 exact canonical request；不接受 raw/parsed graph、fixture/envelope wrapper、tagged union、mode 或 dispatcher。Lexical closure 只在 supplied ADR-0053 observation 内成立，system impact 仍为 UNKNOWN；无 live repository/graph/build/test/cross-surface capture、route、runtime、完整 Impact/Cost/Risk/materiality/safety、persistence、truth 或 authority。

### ADR-0078 WorkIntent v1 Proposed Candidate Governance

Registry v32 仅登记 ADR-0077 的 WorkIntent v1 Proposed cross-language candidate parity、Schema/golden pins、checker-only shadow 与 source-only Python distribution 边界。完整 scope arrays 保持 v31 byte-semantics，Go/Rust 保持 Catalyst-only，所有 WorkIntent refs 均不进入 authenticated context routes。Structural validity 或 digest parity 不表示 Accepted/semantic authority、origin/requester/owner authentication、reference resolution、freshness/materiality/scope assessment、G0 closure、route/runtime/evaluator/producer/consumer、Run/RunJournal/lifecycle、Approval/Grant、permission、persistence、execution 或 effect。

### ADR-0080 Authenticated ADR Approval v1 Proposed Candidate Governance

Registry v33 仅登记 ADR-0079 caller-supplied structure/digests/relations、Schema/golden/proposal pins、checker-only shadow 与 dependency-free Python source-only distribution。完整 scope mapping canonical SHA-256 保持 `8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c`，所有 candidate refs 不进入 authenticated context routes。Structural success 不验证 Ed25519，不认证或授权，不签发 receipt，不消费或证明 external root pin、trusted time/revocation currentness，不提供 CAS/durability/Accepted lifecycle、G0 closure、route/runtime/evaluator/producer、persistence 或 effect；future Go service、production keys/state 不复制。

### ADR-0083 Authenticated ADR Lifecycle v1 Proposed Candidate Governance

Registry v34 使用两个独立 block：ADR-0081 的 Go approval authority 只作为 Catalyst-repository-only Proposed evidence，ADR-0082 只作为 dependency-free Python structural candidate。完整 scope mapping canonical SHA-256 仍为 `8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c`；唯一新增 wiring 是 non-load-bearing lifecycle checker-only shadow，无 Skill、route、kind/evaluator/producer/runtime。Source distribution 不复制 Go contract/authority 或 production root/key/state；structural validity、approval prerequisite 和 `forge accept` 都不接受、拒绝、supersede 或改写 ADR，也不证明 compliance、permission、effect 或 G0 closure。

### ADR-0085 Authenticated ADR Lifecycle Authority Evidence

Registry v35 records ADR-0084 exact44 Go lifecycle authority as Catalyst-repository-only Proposed evidence, relative to explicit external trust and a real opaque `StoredAuthorization`. The complete scope digest and existing checker-only lifecycle shadow remain unchanged. Source-only exact4 distribution contains ADR-0084, ADR-0085 and the governance module/test; it copies no Go authority, key, seed, state, receipt or ledger and adds no route, Skill, service or runtime profile.

### ADR-0087 Legacy Governance Read Import Governance

Registry v36 records ADR-0086's exact supplied-byte Memory/ADR projection as an `unverified_legacy` read-only candidate without changing the complete scope digest. The checker-only shadow truthfully declares `[python3, harness/legacy_governance_read_import_contract_check.py]`; because the detector registry has no stdin field, an operator must pipe the canonical request and close EOF. The candidate supplies thirteen false attestations, persists nothing, and installs no route, Skill, evaluator, producer, service or runtime profile. Source distribution copies Python governance only; the exact10 Go parity package remains Catalyst-repository-only.

### ADR-0089 Kernel Operational Reference Governance

Registry v37 records ADR-0088's five semantic operational objects and nonsemantic closure only as a structural subclosure candidate. The complete scope SHA-256 remains `8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c`; none of the records becomes a Registry kind, evaluator, producer or runtime profile. The checker-only shadow executes exactly `[python3, harness/kernel_operational_contract_check.py, --golden, .]`. Source distribution copies the dependency-free Python core and governance only; exact11 Go and exact13 Rust module parity remain Catalyst-only. The shared Rust `lib.rs` is not whole-file pinned; governance requires only one exact operational module registration so unrelated registrations can coexist.

All fourteen attestations remain false. Structural success authenticates no principal, Grant, artifact content or binding and proves no authorization, permission, event append, persistence, transition, execution, outcome, completion, effect or usage measurement. CognitiveAtom expansion, DecisionTransaction and cross-closure semantics remain absent, so the complete Kernel ABI roadmap parent stays open.

### ADR-0091 Kernel Decision Reference Governance

Registry v38 records ADR-0090's CognitiveAtom v2, DecisionTransaction v1 and one-way operational references only as a structural reference-family candidate. The complete scope mapping and its SHA-256 remain unchanged; none of these records becomes a Registry kind, evaluator, producer or runtime profile. The checker-only shadow executes exactly `[python3, harness/kernel_decision_contract_check.py, --golden, .]`. Source distribution copies exactly sixteen dependency-free Python core files plus the ADR-0091 governance module/test, for exact19 total. Catalyst exact13 Go, flat exact9 Rust and the shared `lib.rs` registration remain repository-only; `lib.rs` is not whole-file pinned.

All 22 attestations remain false. Structural success does not authenticate source, principal, Approval, Grant, artifact content or bindings; declared authority and hardness stay ineffective and instructions stay disabled. It proves no authorization, CAS, event append, persistence, transition, execution, outcome, completion, effect, usage measurement or verifier independence. No Skill, route, PDP, controller, service or runtime is added. The narrowly named structural reference-family repository slice passed formal `forge accept`, so its implementation-roadmap item is complete. Both ADRs remain Proposed, ADR-0038 remains ADOPTED-PARTIAL, and DecisionCapsule, AuthorizedTransactionSpec, authenticated PDP and the rolling controller remain open.

### ADR-0093 Decision Capsule Structural Replay Governance

Registry v39 records ADR-0092's `StructuralReplayManifest`, `DecisionCapsule`, `EvaluationBranch` and `StructuralReplayClosure` only as a pending structural replay repository Candidate. The complete scope mapping and SHA-256 remain unchanged; none becomes a Registry kind, evaluator, producer, projector, adapter, service or runtime profile. The checker-only shadow executes exactly `[python3, harness/decision_capsule_contract_check.py, --golden, .]`. Source distribution copies exactly sixteen dependency-free Python core files plus ADR-0093 and its governance module/test, for exact19 total. Catalyst exact15 Go, exact14 Rust and the shared `lib.rs` registration remain repository-only.

All 32 attestations, both replay controls and all seven completion claims remain false. Structural success resolves no external history or Reflection report, evaluates no model/rule/world state, and authenticates no source, principal, authority, Grant, Approval, result or binding. It proves no authorization, CAS, event append, persistence, transition, execution, outcome, completion, effect, usage measurement, evaluator independence or replay equivalence. No Skill, route, PDP, controller, Reflection consumer or runtime is added. Both ADRs always remain Proposed/null, the narrow roadmap item remains unchecked pending independent review and formal `forge accept`, and ADR-0038, full DecisionCapsule and AuthorizedTransactionSpec remain open.
