# ADR-0048 — Artifact Provenance → EvidenceRecord Shadow Adapter v1

- 状态：已接受（2026-08）
- 范围：Wave 1 的第一个 source adapter；`forgeos.artifact.v1` → `EvidenceRecord` v1
- 关联：ADR-0045、ADR-0046、ADR-0047、
  `docs/contracts/artifact-evidence-adapter-v1.schema.json`

## 背景

ADR-0045 已冻结严格的 Evidence/Claim wire，ADR-0046 只保存 exact record，ADR-0047 只从 Claim 重投影 Atom；三者都没有把
ForgeOS 现有 artifact provenance 变成 Evidence。直接把 `.forge/artifacts.jsonl` 的任意一行送入 journal 会绕过 duplicate/
unknown-field、canonical bytes、source snapshot 与 Governance binding；把 artifact 的 `agent`、`model` 或 `PASS` 字样当作可信
身份或事实又会越过尚未交付的 PDP、Grant、Approval 和 producer attestation。

本 ADR 因此只交付一个可逆、纯函数、无持久化的 shadow adapter。它保存原始 provenance 的全部身份差异，并要求调用方显式
提供 provenance 本身没有的 project/context/policy/tree/aggregate/subject binding。适配成功只说明两种既有 wire 之间的映射可
重放，不说明 artifact 内容真实、当前、充分或获批。

## 决策

### 1. Strict request wire

请求顶层只能包含：

```text
api_version, artifact, binding, canonicalization
```

- `api_version = forgeos.governance.artifact-evidence-adapter/v1`
- `canonicalization = forgeos.canonical-json/v1`
- `artifact` 必须是明确的 `forgeos.artifact.v1`，且精确包含 `_format, run_id, workflow, phase, agent, model,
  path, sha256, size, created_at, prompt_sha256` 十一个字段。artifact loader 的历史空 `_format` 兼容不进入本 adapter。
- `binding` 精确包含 `aggregate_id, context_sha256, policy_sha256, project_id, scope, sequence, sensitivity,
  source_revision, source_tree_sha256, subjects, supersedes_record_ids`。

请求必须是 compact、UTF-8、exact canonical JSON。通用 key 仍为 ASCII snake_case；仅 `request.artifact._format` 作为现有
artifact v1 wire 的局部例外，不能放宽 Evidence/Claim 或其他 canonical codec。禁止 duplicate/unknown field、float、int64
overflow、控制/bidi/U+2028/U+2029、非 normalized repo-relative path、无界字符串/数组/对象/深度。`subjects` 非空，两个数组均
须按 UTF-8 词典序唯一。所有 Governance identifier/hash/sensitivity 必须满足 ADR-0045。

`created_at` 接受 artifact v1 的 RFC3339Nano 时间点并必须落在非负 Unix 范围。Evidence 的整数时间为该时间点向下取整到 Unix
毫秒；原始时间字符串仍进入 source/request digest，因此亚毫秒或 offset 表示变化不会静默取得同一 source identity。

### 2. Source 与 request identity 分离

先按上述局部规则 canonicalize 十一个 artifact 字段：

```text
source_snapshot_sha256 = SHA-256(
  "forgeos.governance.artifact-provenance-source.v1\0" || canonical_artifact_json
)
```

再对完整 exact request 计算：

```text
request_sha256 = SHA-256(
  "forgeos.governance.artifact-evidence-adapter.request.v1\0" || canonical_request_json
)
```

两者都是 lowercase bare hex。`record_id = "artifact-evidence-" + request_sha256`；
`snapshot_id = "artifact-snapshot-" + source_snapshot_sha256`。record identity 因任何 binding 改变而改变，source snapshot identity
只因 artifact provenance 改变。artifact content digest、Governance record ID、aggregate ID、request identity 不得互换。

### 3. 确定性 Evidence 映射

输出必须是现有 `forgeos.governance/v1` 的 `EvidenceRecord`，不得新增 kind 或字段：

- metadata 的 project/aggregate/sequence/scope/revision/tree/context/policy/supersedes 逐项取自 binding；
- `created_at_unix_ms = observed_at_unix_ms = valid_from_unix_ms = floor(artifact.created_at)`，`valid_until_unix_ms = null`；
- `created_by` 固定为 `principal_type=tool`、`authority_domain=shadow`、
  `principal_id=forgeos.artifact-evidence-adapter`、`role=evidence-adapter`、`run_id=artifact.run_id`；
- collector 使用同一固定 tool identity，`collector_version=v1`、`parameters_sha256=request_sha256`；
- `evidence_type=artifact`、`directness=direct`、`source_trust=observed`、`content_role=untrusted_data`；
- `artifact_sha256` 与 locator `content_sha256` 都等于 artifact `sha256`，locator type/ref 为 `artifact`/artifact `path`，
  line/exit 字段均为 null；
- source snapshot type/id/digest 为 `artifact`、上述 snapshot ID 与 source digest；subjects/sensitivity 来自 binding；
- status 固定 `valid`、空 reason codes。这里的 `valid` 仅沿用 ADR-0045 shadow Evidence 结构状态，不是 truth 或 authority。

完整 Evidence 必须由 ADR-0045 strict validator 重新计算 self digest并逐字节重验。adapter 不信任 caller 提供的 output digest。

### 4. 结果与非能力边界

唯一正结果为：

```text
ADAPTED_SHADOW (no truth, authority, claim, atom, persistence, or effect attestation)
```

本 adapter：

- 不读取当前 artifact 文件，不证明 manifest 行来自受信 Store，也不认证 agent/model/collector；
- 不创建 KnowledgeClaim/CognitiveAtom，不确认 Fact，不满足 hard gate 或 `forge accept`；
- 不 append GovernanceRecordJournal、不写 SQLite/文件/Memory/Knowledge；
- 不签发或消费 Grant/Approval，不产生 instruction、transition、completion、network/process/device/production effect；
- 不把 Evolve locator、gate/test result 强塞入 artifact source。它们缺少统一 argv/snapshot/collector/time capture envelope，另行版本化。

调用方未来可显式把 exact EvidenceRecord 交给现有 v25 journal，但必须取得 journal 自己的 `stored|exact_replay` receipt；adapter
不得以 `ADAPTED_SHADOW` 冒充持久化。

## 兼容、迁移与交付

- Artifact v1、Evidence/Claim v1、CognitiveAtom v1 与全部既有 digest/golden 保持逐字节不变。
- SQLite 保持 v25；无 migration、backfill、read side effect 或自动 append。
- 新增独立 Schema/golden、Python universal checker、Go source-owner adapter 与 Rust independent reference adapter。
- scaffold/upgrade 复制 ADR/Schema/golden/Python checker，不安装 Go/Rust runtime，也不提供 authority。
- 未来 authenticated capture、gate/test/Evolve adapter、current/freshness projection 或通用 source envelope 必须新 ADR/version。

## 验收

同一 request 必须跨 Python/Go/Rust 得到 exact 相同 canonical source/request bytes、两条 digest、Evidence bytes与 record digest。
artifact 的 SHA/path/run/time/agent/model/prompt，或 binding 的 context/policy/tree/revision/subject/sequence 任一变化，都必须改变对应
source 或 request/output identity。必须拒绝 legacy/unknown/duplicate/noncanonical/float/overflow/Unicode/path/time/list/digest/
identity drift；输出中的 source trust、collector、state、record ID 或 self digest 漂移也必须被 exact reprojection 抓到。

测试还必须证明适配前后工作树与数据库无写入、失败不产生部分输出、scaffold 可独立跑 golden/checker，且所有正向文字保留完整
shadow 非能力说明。

## 被拒方案

1. 同时适配 artifact/Evolve/gate/test：缺少共同 capture envelope，会把不同来源语义硬编码进一个不可逆 v1；
2. 直接读取当前 artifact 文件并称其 verified：当前字节不等于历史 observation，且会引入 TOCTOU；
3. 把 artifact agent/model 映射为认证 principal：manifest metadata 没有身份签名或 authority receipt；
4. 自动生成 Fact/Atom 或 append journal：把结构转换偷换为 truth、knowledge 或 persistence；
5. 复用 artifact content SHA 作为 Evidence record ID：丢失 run、path、time、prompt 与 Governance binding。

## 重审触发器

- 两个真实 gate/test/Evolve producer 已能提供统一、digest-pinned capture envelope；
- artifact provenance 获得 authenticated producer/collector receipt；
- ContextPackage/PDP 能验证 binding 而非只接受调用方声明；
- 真实使用证明 128-KiB request 或 256 subjects/supersedes 边界不足。
