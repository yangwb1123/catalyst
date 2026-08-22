# ADR-0064 — Local Digest-Bound Agent Output, Review and Approval

- 状态：已采纳并交付（2026-08；A–D runtime、recovery、scaffold 与独立复审均已关闭）
- 范围：七个 canonical workflow 的 command-mode Agent output、caller-declared L3/L4 Build Reviewer、
  Design/Deploy/Rollback 正向 human approval、checkpoint/chain resume 与 universal scaffold/upgrade
- 关联：ADR-0005、ADR-0037、ADR-0055、ADR-0059、ADR-0063、
  `docs/design/ai-engineering-os/implementation-roadmap.md`

## 背景

ADR-0063 让 caller-declared L3/L4 Build 的 `reviewer_v1` 从 advisory 文本变成 fail-closed 控制信号，
但它明确没有把 verdict 绑定到 Reviewer 实际看到的 source、prompt context、effective policy 或 artifacts。
因此一个通过 Reviewer 后、进入 QA 前的可写自定义 phase 可以让批准过期；普通 Agent 输出、Design human gate 和
既有 release marker 也没有一套共同的四摘要合同。

本 ADR 冻结一条更宽、仍然完全本地的 observation/control contract。只有以下四块全部实现、通过独立复审和完整
acceptance 后，roadmap 的“approval/review/Agent output 绑定 source/context/policy/artifact digests”才能勾选：

1. **A — Agent output：**七个 canonical workflow 中每个成功接受的 command-mode Agent 输出都有 exact local receipt；
2. **B — Review：**L3/L4 Build 的 required Reviewer 使用 challenge-bearing `reviewer_v2`，且批准在 QA 前后保持 fresh；
3. **C — Approval：**Design/Deploy/Rollback 的正向人工批准只接受当前、receipt-bound approval context；
4. **D — Recovery/distribution：**checkpoint、chain、scaffold、upgrade 和旧格式迁移不能降级或伪造上述绑定。

本次交付已同时关闭 A–D；任一后续实现若只保留 pure type、部分测试、一个 Reviewer 或一份 marker，仍不得冒充本 ADR 的
完整绑定边界。

## 决策

### 1. 显式 opt-in，不原地改变旧合同

Workflow 顶层以以下值选择本 ADR：

```yaml
output_binding_contract: local_digest_v1
```

七个 shipped canonical workflow（Discover、Design、Review、Build、Deploy、Rollback、Evolve）最终都必须显式选择它。
新 runtime 对未知非空 selector 失败关闭；缺省 selector 保留 legacy/custom 未绑定行为。旧 host 无法被一个它不认识的
字段反向约束；即使它忽略 selector 并执行成功，该结果也不构成本 ADR 的证据。

Canonical Build 的高 materiality Reviewer 改用新名称：

```yaml
verdict_contract: reviewer_v2
```

`reviewer_v1` 保持 ADR-0063 的 legacy 语义，不能原地扩展或把历史输出升级为 v2。`reviewer_v2` 只可出现在
`local_digest_v1` workflow；对 L3/L4 继续要求唯一、readonly、fresh-context、位于最早 QA 前、失败定向回更早
implementer 且不可被 mode skip。L0–L2 与 `materiality_not_bound` 仍可按既有规则跳过或 advisory 处理 Reviewer，
但实际被 runtime 接受的 canonical command output 仍必须产生本 ADR receipt。

Dry-run 不产生 receipt，也不能满足 reviewer_v2、positive approval 或可恢复的 bound completion。无法暴露 exact prompt、
raw output、semantic output 和 postflight 的 custom/sandbox adapter 在 selector 启用时必须失败关闭。

### 2. Canonical JSON、摘要和 source profile

下述 output/source binding API wire 都是无尾随 LF 的 exact compact UTF-8 JSON，并携带
`canonicalization=forgeos.canonical-json/v1`；后文兼容既有控制面的 ApprovalContext/marker 则只允许其明确列出的 strict
字段，不另加该 key。所有对象 key 按 UTF-8 byte lexicographic order，数组不得由 validator
静默排序或去重；整数使用最短十进制，不接受 float、重复/未知 key、非法 Unicode、非 canonical bytes 或 trailing token。
普通 `*_sha256` 是 exact bytes 的小写 SHA-256；下文列出的 self-digest 才使用 domain separation。

每次 preflight 和 postflight 都捕获 `forgeos.product-source-state/v1`：

```text
api_version, canonicalization, entries, profile_id, source_revision
profile_id = product-source-state-v1
entry      = bytes, content_sha256, executable, index_mode, kind, path,
             symlink_target, tracking
```

其 entries 完整复用 hardened `gitworktreesource.SourceEntry` 形状和排序：tracked 加 non-ignored untracked；
regular file 摘要其 exact bytes；symlink 摘要 link target bytes 并记录 target；tracked deletion 显式记录。
Untracked `.forge/**` 与 canonical `docs/release/**` 从 product projection 排除，分别由控制状态与 declared artifact 合同绑定；
tracked Forge control state 不可被排除来掩盖，必须整次失败关闭。Portable alias、gitlink、目录/special inventory entry、
unresolved index、父 symlink、root/file identity drift 和不稳定读取同样失败关闭。

```text
source_state_sha256 = SHA256(
  "forgeos.local-product-source-state.v1\0" || canonical_source_manifest)
```

捕获最多 65,536 entries、8 GiB aggregate、1 GiB per regular file、16 MiB canonical manifest。它是同一稳定
repository-root identity 上的 bounded-interval observation，不是原子 filesystem snapshot、execution pin 或 Git/host 认证。
一次 attempt 的 before/after 必须保持同一 root identity 与 `source_revision`；working-tree product bytes 可以由获准可写
phase 改变，并由不同的 `source_before_sha256`/`source_after_sha256` 如实表示。

### 3. Prompt context、effective policy 与 declared artifacts

`prompt_context_sha256` 是 runtime 在附加本 ADR trailer **之前**，实际编译出的完整 final base prompt bytes 的普通 SHA-256。
它不等于、也不冒充 ADR-0055 `ContextPackage`；v1 不提供 snippet trust lane、token accounting、omission receipt 或 Context
Router。`final_prompt_sha256` 则摘要实际交给 executor 的 base prompt 加 exact trailer 后 bytes。

Effective policy 使用 `forgeos.local-runtime-policy-binding/v1`，canonical 字段顺序为：

```text
adr, agent, api_version, binding_sha256, build_halt, canonicalization,
design_depth, discover_depth, effect, evolve_authority, evolve_depth, executor,
fresh_context, gates, lifecycle, materiality, mode, model,
output_binding_contract, phase, readonly, review_depth, reviewer, stage,
verdict_contract, workflow_sha256
```

`gates` 按 byte order 严格 sorted/unique。该 projection 必须包含 runtime 真正执行的 normalized workflow SHA、stage/phase/
agent/model/executor、mode/lifecycle/materiality、phase flags，以及 `mode.Policy` 的完整十项 effective 值；不得加入实际上未由
runtime 求值的签名 policy、PDP、Grant 或身份声明。计算时暂置 `binding_sha256=""`：

```text
local_runtime_policy_sha256 = SHA256(
  "forgeos.local-runtime-policy-binding.v1\0" || canonical_policy_with_blank_binding)
```

Declared artifact manifest 的 API 为 `forgeos.agent-output-artifact-manifest/v1`，字段顺序为
`api_version,canonicalization,items,manifest_sha256`；item 顺序为 `bytes,path,sha256`。items 是 repository-relative、
slash-normalized、byte-sorted unique 的 current regular files，空集必须编码为 `[]`，不能省略或写 null。计算时暂置
`manifest_sha256=""`：

```text
manifest_sha256 = SHA256(
  "forgeos.agent-output-artifact-manifest.v1\0" || canonical_manifest_with_blank_digest)
```

`artifact_inputs` 必须恰好包含 runtime 为本 attempt 读取/注入的、由更早已完成 phase 声明并具 current-run provenance 的
emits；`artifact_outputs` 必须恰好包含本 phase 成功 postcondition 验证后的 declared emits。两者都嵌入 receipt，并再保存对应
digest 做交叉校验。Path escape、portable alias、缺失/空文件、symlink/hard-link/special file、尺寸/摘要/identity drift 和
未声明输出失败关闭。该 manifest 只证明观察到的 declared regular-file bytes，不证明 artifact 正确、完整或来自真实模型。

### 4. 每次 attempt 的 preflight challenge

每个 phase attempt、retry 和 loop-back 都必须在 spawn 前从 OS CSPRNG 读取 32 bytes，编码成 64 位小写 hex `challenge`；
生成失败即失败关闭，不执行命令。attempt 从 1 开始且同一 run/phase 单调递增，不能复用 nonce 或旧 binding。
runtime 必须在真正 spawn 前把完整 sealed preflight 作为 exact canonical JSONL 写入 private、bounded、完整校验的
`.forge/agent-output-preflight-claims.jsonl`。Claim 只表示“该 attempt 已被消费”，不是 accepted output；即使进程在 spawn、
validator 或 receipt commit 前崩溃也不得删除或复用。恢复时 attempt 取同一 run/workflow/phase 所有 claim 与 receipt 的最大值，
challenge/binding 在完整 claim/receipt 历史中全局唯一；malformed、截断、回滚或 replay claim 必须失败关闭。
Claim journal 还必须有独立的 exact count + full-prefix SHA-256 head witness；head 先于 journal 发布。非空 journal 缺 head、
head 超前/落后或 prefix 不符一律失败关闭，不能从当前剩余字节自动重建 head。

`forgeos.agent-output-preflight-binding/v1` 的 canonical 字段顺序为：

```text
api_version, artifact_inputs_sha256, attempt, binding_sha256, canonicalization,
challenge, local_runtime_policy_sha256, phase, profile_id, prompt_context_sha256,
run_id, source_before_sha256, workflow, workflow_sha256
```

固定 `profile_id=local_digest_v1`。计算时暂置 `binding_sha256=""`：

```text
binding_sha256 = SHA256(
  "forgeos.agent-output-preflight-binding.v1\0" || canonical_preflight_with_blank_binding)
```

Runtime 把以下 exact ASCII trailer 追加到 prompt-context bytes；开头和结尾各包含所示 LF：

```text
\nFORGE_OUTPUT_CHALLENGE: <challenge>\nFORGE_OUTPUT_BINDING_SHA256: <binding_sha256>\n
```

`final_prompt_sha256` 是上述完整拼接结果的普通 SHA-256。任何不能证明这组 bytes 正是传给 child/stdin 的 executor adapter
不得启用本合同。

### 5. `reviewer_v2` 的 exact binding echo

Claude executor 必须先通过唯一 successful `result` envelope 校验；custom command 的 semantic payload 是其 exact raw output。
L3/L4 `reviewer_v2` semantic payload 的最后两个 non-empty lines 必须按顺序、顶格且各只出现一次：

```text
REVIEW_BINDING_SHA256: <本 attempt 的 binding_sha256>
VERDICT: APPROVE
```

或：

```text
REVIEW_BINDING_SHA256: <本 attempt 的 binding_sha256>
VERDICT: REQUEST_CHANGES
```

findings 可位于此前，verdict 后只允许空行。不接受旧 attempt、错误 challenge 所派生 binding、Markdown wrapper、缩进、
大小写/Unicode 仿形、多个 token 或 trailing prose。APPROVE 和 REQUEST_CHANGES 都写 accepted receipt；只有与当前 source/
policy/artifact/workflow 仍一致的 APPROVE receipt 才能作为 QA 控制信号。REQUEST_CHANGES 仍按声明的定向 loop-back 执行，
新的 Reviewer attempt 必须生成新 challenge。

### 6. Accepted AgentOutputReceipt v1

本地 journal 路径固定为 `.forge/agent-output-receipts.jsonl`。每行一个
`api_version=forgeos.agent-output-receipt/v1`、`kind=AgentOutputObservation`、`profile_id=local_digest_v1`
的 exact record。Canonical 顶层字段顺序为：

```text
agent, api_version, artifact_inputs, artifact_inputs_sha256, artifact_outputs,
artifact_outputs_sha256, attempt, binding_sha256, canonicalization, challenge,
executor, final_prompt_sha256, kind, ledger_sequence,
local_runtime_policy_sha256, model, observed_at_unix_ms, phase,
prior_receipt_sha256, profile_id, prompt_context_sha256, raw_output_bytes,
raw_output_sha256, receipt_sha256, run_id, runtime_policy,
semantic_output_bytes, semantic_output_sha256, source_after_sha256,
source_before_sha256, source_revision, source_state_profile, verdict, workflow
```

Receipt 嵌入完整 `runtime_policy`、`artifact_inputs`、`artifact_outputs` 并交叉核对其 detached digest；又从这些字段重建
preflight，必须得到同一个 `binding_sha256`。`source_state_profile=product-source-state-v1`。`verdict` 仅在本合同承认的
review control output 上为 `APPROVE|REQUEST_CHANGES`，其它 output 为 JSON null。

`raw_output_*` 摘要未 trim、未 render、未 semantic unwrap 的 exact retained bytes；发生 capture truncation 时不得接受。
`semantic_output_*` 摘要通过 transport extraction 后、交给 phase contract 的 exact payload bytes。Receipt 不保存两份正文，
只保存 byte counts 与 digests；两者 byte count 都在 0..1 GiB，0 必须配对 exact empty bytes 的普通 SHA-256，不能用
缺失值或占位摘要伪装空输出。`observed_at_unix_ms` 是 0..2^53 的 observation time，不是可信时间戳或 ordering authority。

Genesis 固定 `ledger_sequence=1` 且 `prior_receipt_sha256=null`；后续 sequence 连续，并把 prior 设为前一条 exact
`receipt_sha256`。计算 self-digest 时暂置 `receipt_sha256=""`：

```text
receipt_sha256 = SHA256(
  "forgeos.agent-output-receipt.v1\0" || canonical_receipt_with_blank_digest)
```

Artifact manifest 最多 4,096 items/20 MiB canonical bytes，policy/preflight 各最多 128 KiB，receipt 最多 42 MiB，
sequence/attempt/observed time 不超过 2^53。Store 必须在 private `.forge` 下以 bounded、single-link regular-file、完整 JSONL chain、
进程互斥和 crash-safe commit 语义实现；写入失败时 phase 失败且不得发布 accepted output。v1 不允许静默 prune、跳号、
断链或把 malformed tail 当成空 journal。Receipt journal 同样必须有独立 exact count + full-prefix SHA-256 head witness，
并使用 head-first 发布；合法前缀/空文件回滚、缺 head 或 head/journal 不一致均失败关闭，不能重新从 genesis 续写。

### 7. 唯一正确的 executor 发布顺序

CommandExecutor 的 success path 固定为：

```text
durable preflight claim
  → spawn child
  → exit 0
  → ValidateRawOutput(exact Observed bytes；不是 trimmed Rendered)
  → transport unwrap + semantic/phase-output validation
  → declared artifact validation
  → source/policy/artifact postflight recapture + freshness comparison
  → atomic receipt commit
  → accepted Observe（verdict/feed-forward/cost/control publication）
```

任一步失败都不得调用 accepted Observe、写 verdict ledger、feed forward、满足 gate 或留下可用 receipt。非零 exit、timeout、
cancel、overload、partial/malformed envelope、truncation、dry-run、validator error 和 receipt commit error 都是未接受 attempt。
若产品仍需记录失败调用的成本/延迟，必须使用明确命名的 attempt telemetry，不能复用 accepted-output sink。

Log rendering 不属于 semantic contract，不能通过 trim 掩盖 raw drift。Durable restore 只有在 receipt、journal head、当前
workflow/policy/source/artifacts 全部复验后才可重新发布 accepted output；只保存一段旧 semantic text 不再足够。

### 8. Fresh review 必须保持到 QA 边界

Receipt 首次接受只证明 attempt postflight 时的 observation。只要某 receipt 仍作为当前 prompt input、review control 或
approval context，runtime 就必须重新捕获 current product source、declared artifacts 和 effective workflow/policy，并与其
`source_after_sha256`、manifest/policy digests 比较；故意进入后续获准 mutation 后，较早 receipt 只能成为 historical record，
不能继续当 current approval。

对 L3/L4 Build，Reviewer APPROVE 至少在以下位置重复验证：最早 QA gate 前、QA Agent spawn 前、QA Agent 返回后、Build
stage 返回前，以及 chain 把 Build 记为 completed 前。Reviewer 与 QA 之间的自定义可写 phase、QA 越权写、workflow/policy
变化或 artifact 替换都会使批准过期；runtime 必须失败关闭或回到新的 implement→review attempt，不能沿用旧 verdict。

### 9. ApprovalContext v1 与 positive marker v3

Canonical Design、Deploy、Rollback 的成功 command run 到达人类闸门时，runtime 原子写
`.forge/<stage>.approval-context.json`。Exact compact object 固定字段：

```text
_format=forgeos.approval-context.v1
agent_output_receipt_sha256, artifact_inputs_sha256, artifact_outputs_sha256,
created_at_unix_ms, local_runtime_policy_sha256, prompt_context_sha256, run_id,
source_after_sha256, stage, workflow, workflow_sha256
```

其 detached identity 为：

```text
approval_context_sha256 = SHA256(
  "forgeos.local-approval-context.v1\0" || exact_canonical_context)
```

Context 必须引用 journal 中同一 run/workflow/stage 的 current accepted receipt，且所有摘要逐项相等。`forge approve` 对这三个
canonical stage 的 positive path 必须 native-strict-load context/journal/current workflow，live re-capture source/artifacts，
重建 policy/workflow binding，并确认 prompt-context digest 来自该 exact receipt；缺失、旧格式、冲突、stale 或 unreadable
任一项都返回错误。`--approved` bool 不得再给这些 canonical bound stages 授予正向批准。

Positive marker 使用 `forgeos.approval.v3`，固定字段为：

```text
_format, actor_hint, agent_output_receipt_sha256, approval_context_sha256,
artifact_inputs_sha256, artifact_outputs_sha256, created_at_unix_ms, decision,
local_runtime_policy_sha256, prompt_context_sha256, run_id, source_after_sha256,
stage, workflow, workflow_sha256
```

`decision` 只能是 `approved`；所有 digest/run/workflow 字段必须逐项复制并复验 context。`actor_hint` 仍只是未认证的本机审计
提示，不是 principal。Negative reject marker 可保留 v2 作为 fail-closed rework signal，但它不是正向授权；approve/reject 同时
存在继续视为冲突。Deploy/Rollback validation receipt 升为 v2，必须引用 exact AgentOutputReceipt 与 ApprovalContext；旧 release
receipt v1 和 approval marker v2 对 positive release 仅可诊断，不能原地升级。

Context、marker 和 receipt 都只是 same-UID local observation/control reference，不是 ADR-0059 ApprovalRecord，也不产生
authorization、transition 或 production effect。远程部署/回滚继续由外部 CI/operator 执行。

### 10. Resume、chain 与迁移

选择 `local_digest_v1` 的 durable state 必须升级到 checkpoint/chain v5，并至少绑定：

```text
agent_output_receipt_head_sha256
phase_output_receipts       # phase → exact receipt digest
stage_output_receipts       # stage → exact receipt digest
stage_approval_contexts     # approved/waiting stage → exact context digest
```

Maps 的 key 必须 canonical、完整、sorted/unique；cursor 之前所有 load-bearing command phases 都有 receipt reference。Resume 在
trace、Agent、gate 或 approval side effect 前复验完整 journal chain/head、每个 reference、normalized workflow digests、materiality、
mode/lifecycle、source/artifact freshness 和 current approval context。Chain 在 stage completion 和 terminal completion 前再次复验。

旧 checkpoint/chain v4 缺这些 facts；对 opt-in workflow 一律 diagnostic-only，不猜测补值、升级或恢复。缺 selector 的 legacy/
custom workflow 可继续使用旧行为，但其输出/批准不得宣称满足本 ADR。Scaffold/upgrade 复制 ADR、selector、workflow/role contract、
checker 和 ledger manifest；它们不安装 host Go binary、不生成 receipt/context/marker、不迁移历史 verdict 或批准，也不证明目标项目
具备兼容 runtime。

## 分阶段实施与完成门槛

实现按以下顺序交付；四个阶段均已关闭，roadmap 只在整体通过时勾选：

1. pure canonical types、hardened product source、artifact/policy/preflight/receipt validators 与 executor ordering；
2. canonical Build `reviewer_v2`、challenge echo、所有 QA/freshness timing 和 chain Build resume；
3. 七个 canonical workflow 的全部 accepted command output、journal/store 与 checkpoint binding；
4. Design/Deploy/Rollback ApprovalContext、positive marker v3、release receipt v2、scaffold/upgrade 和完整迁移。

最终验收必须覆盖 exact positive golden，以及 duplicate/unknown/noncanonical JSON、nonce failure/reuse、retry/loop-back stale replay、
raw trim/truncation、validator-before-Observe、receipt-commit failure、source 在 spawn/return/QA 前后漂移、policy/workflow drift、每个
artifact path/bytes/identity drift、Reviewer 后插入 writer、QA 越权写、`--approved` bypass、old marker/receipt/state downgrade、
journal truncation/rollback/conflict、scaffold old-host honesty。需运行 Go test/vet/race、Node/Python/Rust workspace gates、arch/gate/check、
scrubbed full acceptance，并由未参与实现的 fresh reviewer 复核合同与绕过面。

## 明确不提供

本 ADR 不提供或声称：

- materiality、operator、Agent、Reviewer、model、provider、credential 或 executor 的身份认证；
- cryptographic separation of duties、Reviewer independence/quality、normalized finding、ReviewCase 或事实正确性；
- ADR-0055 ContextPackage、真实 Context Router、完整遗漏证明或 Agent 任意 filesystem read 的逐项清单；
- signed policy/PDP、CapabilityGrant、ADR-0059 ApprovalRecord、revocation、RiskAcceptance 或 effect authority；
- atomic repository snapshot、process/OS sandbox、remote attestation、provider response signature 或生产执行证明；
- 对 same-UID writer、root/admin、恶意 host binary/executor、整盘/整目录 rollback 的 tamper-proof 防护；
- head-first 两文件提交在 head 已发布、journal 尚未发布时崩溃后的自动恢复；该状态保持失败关闭并需要 operator 诊断，
  不会猜测性回退 head 或重放 attempt；
- Evidence/Claim/Knowledge adoption、lifecycle completion、代码/测试/artifact 质量或 release 成功证明。

Digest 证明的是 runtime 声明的 exact bytes 之间有一致引用，不证明这些 bytes 为真、完整、独立或有权。任何超出本地
observation/control consistency 的解释都必须由后续 authenticated Governance Kernel 合同明确授予。
