# ADR-0050 — Evolve Repository Locator → EvidenceRecord Shadow Adapter v1

- 状态：已接受（2026-08）
- 范围：Wave 1 的第三个 source adapter；Evolve repository locator → `EvidenceRecord` v1
- 关联：ADR-0020、ADR-0045、ADR-0046、ADR-0048、ADR-0049、
  `docs/contracts/evolve-repo-locator-evidence-adapter-v1.schema.json`

## 背景

ADR-0020 已要求 Evolve scan 的 `finding`、`clear` 与 opportunity 使用有界的仓库相对路径、可选行号和说明定位当前文本证据。
ADR-0045 的 `EvidenceRecord` 已预留 `repo_locator`，但两条 wire 之间没有确定性转换。直接把 scan JSON 当作 Governance Evidence
会丢失文件内容摘要、采集身份、观察时间、源树和 report identity；把 `finding`、`clear` 或 opportunity 字样直接解释成事实，又会
越过尚未交付的 producer authentication、Context/PDP、Claim lifecycle 和 review authority。

本 ADR 只交付一个纯函数、无持久化的 shadow adapter。调用方必须把原 Evolve locator 和缺失的 capture/provenance 声明封装为
严格 observation；adapter 保存差异并映射成既有 `repo_locator` EvidenceRecord。适配成功只证明 exact bytes 可确定性重投影，
不证明文件、报告、说明或扫描判断真实、当前、完整或获批。

## 决策

### 1. Strict request 与 observation wire

请求顶层只能包含：

```text
api_version, binding, canonicalization, observation
```

- `api_version = forgeos.governance.evolve-repo-locator-evidence-adapter/v1`；
- `canonicalization = forgeos.canonical-json/v1`；
- `observation.api_version = forgeos.evolve-repo-locator/v1`；
- `observation` 精确包含 `api_version, canonicalization, content, locator, observed_at_unix_ms, producer,
  scan_context, source`；
- `binding` 精确包含 `aggregate_id, context_sha256, policy_sha256, project_id, scope, sensitivity,
  sequence, subjects, supersedes_record_ids`。

`content` 显式声明 `bytes` 与 `sha256`。`bytes` 必须为 `1..1 MiB`，保持 ADR-0020 的非空有界文本文件上限；adapter 不读取
对应 path，也不验证 digest preimage。`locator` 精确保存 ADR-0020 的 `path, line, detail`：`line=0` 表示文件级定位，正整数
表示单行定位；path 必须是 canonical forward-slash repository-relative text，禁止 absolute、drive-relative、反斜杠、空段、`.`、
`..`、尾斜杠以及 ASCII 大小写不敏感的 `.git`/`.forge` 控制根。该文本检查不声称路径存在、是 regular file、未经过
symlink 或行号在当前文件范围内。

`scan_context` 精确包含：

```text
contract, depth, dimension, opportunity_id, relation, report_sha256
```

- `contract = evolve_scan_v1`；
- depth 为 `advisory|opportunistic|standard|thorough`；
- dimension 使用 ADR-0020 的六个固定维度；
- relation 为 `finding|clear|opportunity`；
- relation 为 opportunity 时必须给出 ADR-0020 的 1–64 bytes ASCII `[a-z0-9][a-z0-9._-]*` `opportunity_id`，其他 relation 必须为 null；
- `report_sha256` 是调用方声明的完整 canonical Evolve report digest，adapter 不验证 report preimage 或 locator membership；
- `unavailable` 没有 repository evidence，因此不能形成本 observation。

`producer` 精确声明 `producer_id, producer_type, producer_version, run_id, parameters_sha256`；type 仅为 `service|tool`。
`parameters_sha256` 原样映射为 Evidence collector parameters，不被解释为已认证的 prompt、policy 或 tool digest。`source` 声明
`source_revision` 与 `source_tree_sha256`。数组必须按 UTF-8 byte order 唯一排序，subjects 非空。所有整数属于 signed-int64，所有
hash 为 lowercase bare SHA-256；禁止 duplicate/unknown field、float、overflow、非 canonical bytes、控制/bidi/U+2028/U+2029
和超限输入。

### 2. Locator、source 与 request identity 分离

对 exact locator、完整 observation 和完整 request 分别计算：

```text
locator_sha256 = SHA-256(
  "forgeos.governance.evolve-repo-locator.locator.v1\0" || canonical_locator_json
)

source_snapshot_sha256 = SHA-256(
  "forgeos.governance.evolve-repo-locator-source.v1\0" || canonical_observation_json
)

request_sha256 = SHA-256(
  "forgeos.governance.evolve-repo-locator-evidence-adapter.request.v1\0" || canonical_request_json
)
```

`record_id = "evolve-locator-evidence-" + request_sha256`；
`snapshot_id = "evolve-locator-" + source_snapshot_sha256`；adapter principal run ID 为
`"evolve-locator-adaptation-" + request_sha256`。path/line/detail 变化只直接改变 locator identity；content、scan、producer、source
或 observation time 变化会改变 source identity；任何 binding 变化会继续改变 request、Evidence bytes 和 record identity。

### 3. 确定性 Evidence 映射

输出必须是既有 `forgeos.governance/v1` `EvidenceRecord`，不得新增 kind 或字段：

- metadata 的 project/aggregate/sequence/scope/context/policy/supersedes 取自 binding；source revision/tree 取自 observation.source；
- `created_at_unix_ms = observed_at_unix_ms = valid_from_unix_ms = observation.observed_at_unix_ms`，valid-until 为 null；
- `created_by` 固定为 shadow tool `forgeos.evolve-repo-locator-evidence-adapter` / `evidence-adapter`，run ID 使用上述
  request-derived synthetic identity，不能冒充 producer；
- collector 的 id/type/version/run/parameters 逐项复制 producer 声明；
- `evidence_type=repo_locator`、`directness=direct`、`source_trust=observed`、`content_role=untrusted_data`；
- `artifact_sha256` 与 locator `content_sha256` 均为 observation.content.sha256；
- locator type/ref 为 `repo`/observation.locator.path，exit 为 null；line=0 映射为 start/end 均 null，正整数映射为相同 start/end；
- source snapshot type/id/digest 为 `repository`、上述 snapshot ID 与 source digest；subjects/sensitivity 取自 binding；
- status 固定 `valid` 与空 reason codes。这里的 valid 只表示 ADR-0045 shadow record 结构状态。

完整 Evidence 必须由 ADR-0045 strict validator 重新计算 self digest并逐字节重验。adapter 不信任 caller 给出的 output 或 digest。

### 4. 唯一结果与非能力边界

唯一正结果为：

```text
ADAPTED_SHADOW (locator mapping only; no file/report verification, scan judgment, completion, truth, authority, claim, atom, persistence, or effect attestation)
```

本 adapter：

- 不读取 repository file、report、Git tree、checkpoint 或 trace，不验证 content/report/tree/parameters digest preimage；
- 不证明 path 存在、未变、是 regular file、没有 symlink/hard-link，或 line/detail 与文件内容相符；
- 不认证 producer/collector，不确认 finding/clear/opportunity，不证明 scan depth coverage 或 candidate task 价值；
- 不创建 KnowledgeClaim/CognitiveAtom，不满足 hard gate、review 或 `forge accept`；
- 不 append GovernanceRecordJournal、不写 SQLite/文件/Memory/Knowledge，不产生 network/process/device/production effect；
- 不接入真实 Evolve runtime producer。生成 observation、文件 digest、report digest 与捕获时点是独立后续版本。

调用方未来可显式把 exact EvidenceRecord 交给 ADR-0046 journal，但只有 journal 的 `stored|exact_replay` receipt 能说明本地结构持久化。

## 兼容、迁移与交付

- ADR-0020 Evolve report、Evidence/Claim v1、既有 Artifact/Command adapters 与所有 frozen golden 保持逐字节不变；
- SQLite 保持 v25；无 migration、backfill、自动 append 或 read side effect；
- 新增独立 Schema/golden、Python universal checker、Go source-owner adapter 与 Rust independent reference adapter；
- scaffold/upgrade 复制 ADR/Schema/golden/Python checker，不安装 Go/Rust runtime，也不提供 authority；
- 真实 Evolve capture integration、producer authentication、current-file verification 或通用 source envelope 必须新 ADR/version。

## 验收

同一 request 必须跨 Python/Go/Rust 得到 exact 相同 canonical locator/observation/request bytes、三条 digest、Evidence bytes 与 record
digest。path/line/detail、content digest/size、scan relation/dimension/depth/report、producer/parameters、source tree/revision/time 或 binding
任一变化必须改变对应 identity。Path 必须非空白、不超过 4,096 Unicode scalar；path/detail 拒绝全部 Unicode control。Opportunity ID 保持
`evolve_scan_v1` 的 1–64 bytes `[a-z0-9][a-z0-9._-]*` 词汇。必须拒绝 duplicate/unknown/noncanonical/float/overflow/Unicode/path/protected-root/line/size/relation/
opportunity/list/hash/identifier/output drift；输出 collector、trust、state、line range、record ID 或 self digest 漂移必须被 exact
reprojection 抓到。

测试还必须证明 adapter 不读 locator path、适配前后工作树与数据库无写入、失败无部分输出、fresh scaffold 可独立跑 golden/checker，
且所有正向文字保留完整 shadow 非能力说明。

## 被拒方案

1. 直接把 ADR-0020 path/line/detail 填入 Evidence：缺 content/report/producer/time/tree identity，无法形成 valid EvidenceRecord；
2. adapter 读取当前文件并计算 digest：当前文件不等于历史 scan observation，会引入 TOCTOU，并把纯映射偷换成 capture；
3. 把 relation 映射成 Fact/Claim：finding、clear 和 opportunity 仍是未认证扫描判断；
4. 复用 Artifact adapter：Evolve locator 没有 artifact manifest 的十一字段与时间语义；
5. 复用 Command adapter：repository line range 与 process termination/stream 是不同 source contract；
6. 自动 append journal：把结构转换偷换为 durability。

## 重审触发器

- Evolve runtime 能原子捕获 exact report、source tree、文件 bytes/digest/identity 与 observation time；
- producer/collector 获得 authenticated receipt；
- ContextPackage/PDP 能验证 binding 与 policy，而非只接受调用方声明；
- 真实使用证明 128-KiB request、1-MiB content 或 256 subjects/supersedes 边界不足。
