# ADR-0049 — Command Observation → Gate/Test EvidenceRecord Shadow Adapter v1

- 状态：已接受（2026-08）
- 范围：Wave 1 的第二个 source adapter；exact terminal command observation → `EvidenceRecord` v1
- 关联：ADR-0020、ADR-0040、ADR-0045、ADR-0046、ADR-0048、
  `docs/contracts/command-observation-evidence-adapter-v1.schema.json`

## 背景

ADR-0045 已冻结 strict `EvidenceRecord` wire，ADR-0048 只适配 `forgeos.artifact.v1`。ForgeOS 现有
`TaskEvidencePackage.verification_receipt`、gate bridge 和 acceptance harness 已能描述或产生真实命令观察，但没有共同、版本化、
digest-bound 的 command capture：argv、cwd、source tree、producer、run、时间、环境/toolchain snapshot、exit code、输出摘要和截断边界
仍散落在不同结构里。直接把 `PASS` 文本或 `gate.Result` 映射为 Evidence 会丢失来源差异，也会把 process exit、criterion verdict、
`forge accept` 完成裁决和 Governance truth/authority 混为一谈。

本 ADR 因此冻结一个最小 command observation，并用纯函数把其中真实 exited 的 observation 适配为既有
`test_run|gate_result` EvidenceRecord。capture 可以诚实表达 timeout/cancel；现有 Evidence v1 无法无损表达时 adapter 必须拒绝。
adapter 只映射 caller 提供的 exact observation，不 spawn 命令、不读取当前工作树、不认证 producer/runner/environment/toolchain，
也不重新判定 PASS。

## 决策

### 1. Strict request 与 command observation

请求顶层只能包含：

```text
api_version, binding, canonicalization, observation
```

- `api_version = forgeos.governance.command-observation-evidence-adapter/v1`
- `canonicalization = forgeos.canonical-json/v1`
- `observation.api_version = forgeos.command-observation/v1`，且 observation 自带相同 canonicalization。
- observation 精确包含：`api_version, canonicalization, command, ended_at_unix_ms, evidence_type, producer, source,
  started_at_unix_ms, streams, termination`。
- command 精确包含 `argv, cwd, environment_sha256, stdin_bytes, stdin_sha256, timeout_ms, tool_snapshot_sha256`。
  `argv` 为 1–64 个 exact UTF-8 字符串，argv[0] 非空，后续空参数保持为合法参数；adapter 对它做 direct-exec array 记录，不做
  shell parsing/normalization。显式 `['sh','-c',...]` 仍按原 argv 记录，不获得额外可信度。非 UTF-8 POSIX argv/cwd 不属于 v1。
  `cwd` 只能是 `.` 或 normalized repo-relative forward-slash path。timeout 为 null（无 command deadline）或
  1..86,400,000 milliseconds。
- `evidence_type` 只能是 `gate_result|test_run`。producer 精确包含 `producer_id, producer_type, producer_version, run_id`，
  type 只允许 tool/service；包括 evidence type 在内的这些值仍是 caller 声明，不证明该命令确属 gate/test，也不包含 criterion/verdict。
- source 精确包含 source revision/tree digest。streams 精确包含 combined/stdout/stderr；每个 stream 精确包含 `bytes,
  retained_bytes, retained_sha256, sha256`。full digest 覆盖 producer 观察并 drain 的完整 raw stream，retained digest 覆盖实际保留
  前缀；`0 <= retained_bytes <= bytes`。combined bytes 必须等于 stdout+stderr bytes；combined digest preimage 是按 producer 记录的
  drain-event 顺序依次拼接的 raw chunks，不声称证明 OS/子进程原始 emission order；空 stream 使用
  SHA-256(empty)，retained bytes 为零时 retained digest 也必须是 SHA-256(empty)，完整保留时 full/retained digest 必须相同。
- termination 是 strict union：`exited + nonnegative real exit_code`，或 `timed_out|cancelled + null exit_code`。`cancelled` 只表示
  producer/controller 已请求取消，不能从负 return code 或 signal 自行推断。Observation wire 可
  诚实保存三种状态，但 EvidenceRecord v1 的 command locator 强制真实 non-null exit code，所以 adapter 只接受 exited。timeout/cancel
  必须拒绝并产生零 Evidence，绝不使用 `-1/-2` sentinel；not-executed、spawn-failed 和未由已请求取消解释的 signaled termination
  不属于本 observation v1，也不得被编码成 `exited` 或 shell `128+signal` 猜测值。exit code 限于可移植的非负 signed-int32 子集；
  超过 `2,147,483,647` 的平台特有 exit code 必须拒绝而非截断。
- source/environment/tool/stdin/stream 全部使用 lowercase bare SHA-256；`stdin_bytes=0` 时 stdin digest 必须为 SHA-256(empty)。时间为
  非负 signed-int64 Unix milliseconds，ended 不早于 started。binding 精确包含 `aggregate_id, context_sha256, policy_sha256,
  project_id, scope, sensitivity, sequence, subjects,
  supersedes_record_ids`。

请求必须是 compact UTF-8 exact canonical JSON。禁止 duplicate/unknown field、float、int64 overflow、控制/bidi/U+2028/U+2029、
不规范 path、无界 string/array/object/depth。普通 text/path/argv item 同时限制为最多 4,096 个 Unicode scalar 与 16,384 UTF-8
bytes；identifier 为最多 160 个 ASCII 字符。subjects 非空；subjects/supersedes 必须按 UTF-8 bytes 严格排序且唯一。
JSON Schema 的 `#CommandObservationV1` anchor 允许三种 observation terminal kind，而根 request 额外只允许 `exited`。Schema 只冻结
shape；时间、stream 计数/空摘要、UTF-8 byte cap、排序和 exact canonical bytes 必须由 Python/Go/Rust semantic validator 执行，
Schema-only validation 不足以接受输入。

### 2. Identity 分离

对 exact command、observation 和完整 request 分别计算：

```text
command_sha256 = SHA-256(
  "forgeos.governance.command-observation.command.v1\0" || canonical_command_json
)

source_snapshot_sha256 = SHA-256(
  "forgeos.governance.command-observation-source.v1\0" || canonical_observation_json
)

request_sha256 = SHA-256(
  "forgeos.governance.command-observation-evidence-adapter.request.v1\0" || canonical_request_json
)
```

`record_id = "command-evidence-" + request_sha256`；
`snapshot_id = "command-observation-" + source_snapshot_sha256`。command parameters、stream content、source tree、environment/tool
snapshot、source observation、request identity 和 Evidence self digest 各自承担不同语义，不得互换。

### 3. Deterministic Evidence mapping

输出继续使用现有 `forgeos.governance/v1` `EvidenceRecord`，不增加字段或 kind：

- metadata 的 project/aggregate/sequence/scope/context/policy/supersedes 来自 binding；revision/tree 来自 observation.source；
- `created_at_unix_ms = observed_at_unix_ms = valid_from_unix_ms = observation.ended_at_unix_ms`，valid-until 为 null；这里的
  created-at 是 deterministic logical projection time，不是离线 adapter 的实际墙钟执行时间；
- created-by 固定为 shadow tool `forgeos.command-observation-evidence-adapter` / role `evidence-adapter`；其 run id 确定性派生为
  `command-adaptation-<request_sha256>`，不得借用被观察 producer 的 run id；
  该合成 id 只关联一次 deterministic adaptation，不是认证 execution receipt；
- collector 逐项保留 observation producer 的声明 id/type/version/run，parameters digest 为 command digest；这仍不是身份认证；
- `evidence_type` 来自 observation，`directness=direct`、`source_trust=observed`、`content_role=untrusted_data`；
- EvidenceRecord v1 的历史 `artifact_sha256` 字段与 runtime source snapshot digest 都等于 observation source digest。前者是既有
  `status=valid` 语义对 captured evidence material digest 的兼容要求；它不把 source kind 改称 Artifact，也不得被解释为被观察命令
  生成了某个业务产物；
- command locator ref 为 `command-observation:<source digest>`，content digest 为 full combined stream digest，exit code 为真实 process
  exit code，line 字段均为 null；
- source snapshot type 固定 `runtime`；subjects/sensitivity 来自 binding；status 固定 `valid`、空 reason codes。

这里的 `valid` 只表示 Evidence v1 状态矩阵中 captured material digest 存在且没有 adapter-level reason；它不验证 stream preimage。
exit=0 仍只是 observation 中的 untrusted process result，
不等于 criterion 真实、gate 满足、task 完成或 `forge accept` 接受。完整 Evidence 必须由 ADR-0045 strict validator 重新计算 self digest并
逐字节重验；candidate output 还必须与 deterministic re-adaptation 完全一致。

### 4. 唯一正结果与非能力边界

唯一正结果为：

```text
ADAPTED_SHADOW (observation mapping only; no execution, pass, completion, truth, authority, claim, atom, persistence, or effect attestation)
```

本 adapter：

- 不 spawn 命令、不重新读取 stdin/stdout/stderr/current tree，不验证 stream digest 是否对应真实历史字节；
- 不认证 producer、runner、principal、collector、environment 或 toolchain；environment/tool/source tree digest 在 v1 中是 opaque
  producer declaration，没有冻结可跨 producer 比较的 preimage/domain/redaction profile。producer 不得直接 hash secret 或用其摘要
  暗示环境等价；真实接线必须另行冻结无秘密 manifest profile 或升级 ABI；
- 不把 exit=0、输出中的 PASS 字样或 `gate_result` 当成 gate authority，不替代 load-bearing `forge accept`；
- 不创建 Claim/Atom，不确认 Fact，不满足 hard guard，不签发/消费 Grant/Approval；
- 不 append GovernanceRecordJournal，不写 SQLite/file/Memory/Knowledge，不产生 transition/completion/network/process/device/production effect；
- 不适配 Evolve repository locator；后者使用不同 source semantics 和新版本。

调用方未来可把 exact output Record 显式交给 ADR-0046 journal，但只有 journal 自己的 `stored|exact_replay` receipt 才能证明本地
structural persistence。`ADAPTED_SHADOW` 不得冒充 durability。

## 兼容、迁移与交付

- Evidence/Claim v1、CognitiveAtom v1、artifact adapter v1 与全部既有 bytes/digests/goldens 保持不变。
- SQLite 保持 v25；无 migration、backfill、read side effect 或自动 append。
- 新增独立 Schema/golden、Python universal checker、Go source-owner reference 与 Rust independent reference。
- scaffold/upgrade 复制 ADR/Schema/golden/Python checker；不安装 Go/Rust runtime 或 producer integration。
- 当前切片冻结 observation envelope canonicalizer 与 pure adapter ABI，但不改变 gate.Result、execbound、trace 或 acceptance output。
  environment/tool/source tree digest 的 preimage/redaction profile 尚未冻结；真实 producer 接线必须先另行冻结版本化、无秘密 profile，
  再计算 envelope 中的 exact digest。producer 不得另造 envelope canonicalizer，也不得把不同 profile 的 opaque hash 跨 producer 比较。

## 验收

同一 request 必须跨 Python/Go/Rust 得到 exact 相同 canonical command/observation/request/Evidence bytes 与
command/source/request/Evidence digest。gate_result 和 test_run、exit=0 和 nonzero exit 都有覆盖；source revision/tree、argv/order/cwd/
stdin/timeout、producer/run、start/end、environment/tool snapshot、exit/stream digest/size/truncation 或 Governance binding 任一变化必须改变
对应 identity。

必须拒绝 duplicate/unknown/noncanonical/float/overflow/Unicode/path/list/hash/time、stream count/hash/truncation 矛盾、
timeout/cancel/sentinel exit、output/state/collector/locator/digest drift。测试还证明适配前后工作树/数据库不变、失败不产生部分输出、fresh scaffold 可独立运行
golden/checker，并保留完整 shadow 非能力说明。

## 被拒方案

1. 把 gate/test/Evolve/artifact 放进一个通用 union adapter：来源语义、locator 和失效方式不同，v1 会成为不可演进的万能 envelope；
2. `-1/-2` 表示 timeout/cancel：这会把没有 process exit 的状态伪装成 command locator exit code；
3. 只保存 `PASS`/`FAIL`：丢失 argv、snapshot、producer、time、output 与环境差异；
4. adapter 自己重跑命令：重放可能产生不同 effect/结果，也破坏 pure/replay-safe 边界；
5. 把 producer 映射为 authenticated collector，或把 exit=0 映射为 truth/completion：当前没有 identity/PDP/Grant/Review authority；
6. 自动 append journal：结构映射与 durability 必须分别取得各自 receipt。

## 重审触发器

- gate/test producer 能直接输出本 exact capture，并由独立工具重算 source/environment/toolchain/output digest；
- Evidence command locator 新版本能无 sentinel 表达 timeout/cancel/not-executed；
- PDP/ContextPackage 能验证 producer、source tree、environment 与 binding，而非只接受 caller 声明；
- 真实使用证明 128-KiB request、64 argv 或当前 output summary 不足。
