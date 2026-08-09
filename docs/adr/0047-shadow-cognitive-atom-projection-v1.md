# ADR-0047 — Shadow CognitiveAtom Projection v1

- 状态：已接受（2026-08）
- 范围：ADR-0038 Kernel ABI 的第一个窄切片；确定性 KnowledgeClaim→CognitiveAtom 投影
- 关联：ADR-0038、ADR-0045、ADR-0046、
  `docs/contracts/cognitive-atom-projection-v1.schema.json`

## 背景

ADR-0038 已接受 AADM 的目标语义与 `ABI/schema/golden` 优先顺序，但其 CognitiveAtom 示例不是可执行 wire
contract：`projection_confidence: 0.0` 与现有禁止浮点的 canonical JSON v1 冲突，Atom identity、closed enums、
source closure、digest、兼容边界和错误结果也未冻结。若直接实现 prompt compiler，模型可以把候选内容伪装成 hard Fact；若一次
冻结 DecisionTransaction、Capability invocation、Artifact/Execution receipt，又会提前绑定尚未交付的 Grant、Approval、PDP、
effect vocabulary、lease/fencing 和 Device Fabric 语义。

因此先冻结一个更窄、可逆、无 effect 的 ABI：只从 ADR-0045 已严格验证的 shadow KnowledgeClaim record set 做确定性投影。
该投影用于让后续 graph/compiler 有稳定的运行期最小命题载体，不创建新的事实、权限或持久化真值。

## 决策

### 1. v1 只投影已有 Claim 的交集类型

`forgeos.aadm.cognitive-atom/v1` 接受且同名投影：

- `fact`
- `constraint`
- `decision`
- `inference`
- `assumption`
- `hypothesis`
- `unknown`

KnowledgeClaim 的 `lesson`、`proposal` 可作为引用闭包成员，但 v1 不为其生成 Atom；它们不是 ADR-0038 的 CognitiveAtom
闭集。`goal/actor/object/operation/preference/risk/acceptance/evidence/observation` 缺少已冻结的 source contract，也不在 v1
中猜测。输入没有任何可投影 Claim 时失败关闭。

v1 必须先用 ADR-0045 validator 对整个 exact canonical source record set 重新执行 shadow admissibility、引用、subject、
supersession、cycle 和 digest 校验。它不接受 authoritative Claim 状态，也不把 journal structural head 当作 current truth。

### 2. Exact wire 与逐字节投影

Atom 顶层字段严格为：

```text
api_version, integrity, kind, metadata, source, spec
```

- `api_version = forgeos.aadm.cognitive-atom/v1`
- `kind = CognitiveAtom`
- `metadata.context_sha256/policy_sha256/project_id/scope/source_revision/source_tree_sha256`
  从源 Claim metadata 精确复制；`task_id` 是调用方提供的 bounded identifier；`atom_id` 按 §4 派生。
- `source` 精确绑定源 Claim record identity、digest、aggregate、sequence 及其最小引用闭包 identity。
- `spec.atom_type` 同名复制 source `claim_type`；`epistemic_state` 复制 source state。
- `spec.proposition` 精确复制 `subject/predicate/object_type/object_value`。
- `spec.supporting_evidence_record_ids`、`contradicting_evidence_record_ids`、
  `derived_from_claim_record_ids` 精确复制，保持已验证的词典序唯一数组。
- `spec.validity` 精确复制 source status 的 `valid_from_unix_ms/valid_until_unix_ms`。
- `projection_confidence_micros` 取代未冻结的浮点示例；Assumption/Hypothesis/Inference 必须精确复制非空
  `confidence_micros`，其他类型必须为 `null`。
- `projection_mode = shadow`、`hardness = none`、`authority_ref = null`、
  `instruction_allowed = false`，均不可由输入覆盖。

允许状态矩阵完全复用 ADR-0045 shadow 状态：Fact `candidate|contested`；Constraint `candidate`；Decision
`proposed`；Inference `candidate`；Assumption/Hypothesis `open|testing`；Unknown `open|investigating`。

### 3. Source closure

每个 Atom 的 source closure 从源 Claim 开始，递归遍历：

- supporting / contradicting Evidence references；
- derived-from Claim references；
- 每个已纳入记录的 supersedes references。

闭包包含源 Claim，按 `record_id` 词典序排列并编码成 exact canonical Governance record-set array。其 count 包含源记录，
byte count 是完整 UTF-8 array 的字节数，digest 为：

```text
SHA-256("forgeos.governance.record-set.v1\0" || canonical_closure_bytes)
```

v1 的文件投影输入复用 ADR-0045 closed record-set 边界：1–256 records、最多 1,048,576 bytes。ADR-0046 journal 的跨 batch
1,024-record / 16-MiB dependency admissibility 不在本切片实现；未来 read-only journal adapter 必须新版本化，验证 owning
batch 与 exact bytes，不能只信投影列。

### 4. Canonical bytes 与 identity

Atom 和 Atom set 使用 `forgeos.canonical-json/v1`：compact UTF-8、ASCII snake_case keys 字节排序、signed int64、禁止
float/重复或未知字段/控制字符/bidi/U+2028/U+2029，不做 Unicode normalization。单 Atom 最多 131,072 bytes；Atom set
含 1–256 atoms，最多 1,048,576 bytes；通用 depth/field/array/string 限制保持 16/64/256/16,384 UTF-8 bytes。

Atom digest 在只把 `integrity.canonical_sha256` 置空后计算：

```text
SHA-256("forgeos.aadm.cognitive-atom.v1\0" || canonical_payload_bytes)
```

Atom ID 为 `atom-` 加下式的完整 lowercase hex。`u64be` 是 unsigned 64-bit big-endian 长度；hash 字段按 32 raw bytes
拼接，而不是 hex text：

```text
SHA-256(
  "forgeos.aadm.cognitive-atom-id.v1\0"
  || u64be(len(task_id_utf8)) || task_id_utf8
  || source_claim_digest_raw32
  || context_digest_raw32
  || policy_digest_raw32
  || source_tree_digest_raw32
  || u64be(len(source_revision_utf8)) || source_revision_utf8
)
```

Atom set 按 `metadata.atom_id` 词典序唯一排列；set digest 为：

```text
SHA-256("forgeos.aadm.cognitive-atom-set.v1\0" || canonical_atom_set_bytes)
```

同一 task/binding/source Claim 只产生一个 Atom。相同输入必须跨 Python/Go/Rust 得到完全相同 payload、record、ID、
Atom digest、closure digest、set bytes 与 set digest。

### 5. 结果与权限边界

唯一正结果是：

```text
PROJECTED_SHADOW
(no truth, authority, instruction, hard-guard, transition, completion or effect attestation)
```

实现不得用 `accepted/authorized/confirmed/verified/completed/stored` 描述该结果。本契约：

- 不认证 principal、Evidence collector 或 source truth；
- 不启用 authoritative state，不满足 hard guard 或 `forge accept`；
- 不签发/消费 Grant 或 Approval，不推进 lifecycle；
- 不写 Knowledge，不把 Atom 加入 GovernanceRecordJournal v1；
- 不执行文件、进程、网络、设备或生产 effect；
- 不实现 prompt/model→Atom compiler，只实现 Claim→Atom pure projection。

## 兼容、迁移与交付

- Evidence/Claim v1 wire、digest domains、golden 与 journal v1 均保持不变。
- SQLite 继续是 v25；没有 migration、backfill、旧 Memory/ADR import 或数据库读取副作用。
- 新增独立 JSON Schema、golden、Python universal shadow checker，以及 Go/Rust repository reference codecs。
- scaffold/upgrade 复制 schema、golden 与 Python checker；它不安装 Go/Rust binary。
- 后续新增 hardness、authority、其他 Atom source/type、persistence、journal adapter 或 model compiler 必须走新版本/ADR，
  不得重释 v1 bytes。

## 验收

必须覆盖跨语言 exact golden、全部允许类型/状态、confidence 边界、object type/value、source/closure/projection 任一漂移、
dangling/wrong-kind/subject/cycle/unsorted/duplicate reference，以及 duplicate/unknown/float/int64 overflow/Unicode control/
noncanonical/oversize/depth。source context/policy/tree/revision/task 任一改变必须使 Atom ID 或 digest 改变。Scaffold 必须能
独立运行 Python golden/instance checker，且不得宣称 installed runtime 或持久化能力。

## 被拒方案

1. 一次冻结完整 Kernel ABI：会把未冻结的授权与执行语义固化到低可逆协议；
2. 直接从 prompt/model 生成 hard Atom：绕过 Evidence/Claim shadow 约束；
3. 让 Atom 进入 journal v1：会把 structural record persistence 偷换成知识或 authority ledger；
4. 使用浮点 confidence：破坏 canonical JSON v1 跨语言确定性；
5. 将 lesson/proposal 强行映射到不相同的 Atom type：改变业务语义。

## 重审触发器

- 两个真实 shadow compiler 场景无法由七种 Claim-backed Atom 表达；
- 256-record closed source set 无法覆盖真实单任务投影，且已有安全的 journal snapshot reader；
- 后续 Governance/PDP 已能证明 authority/hardness，而不是仅携带声明引用；
- full Kernel ABI 需要统一 Artifact/Execution identity 或 Claim projection 不能无损重放。
