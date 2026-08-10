# ADR-0052 — Local Evolve Repository Locator Observation Producer v1

- 状态：已采纳并交付（DONE，2026-08）
- 范围：Wave 1-E；Unix 本地 canonical Evolve report/source/locator observation production
- 关联：ADR-0020、ADR-0045、ADR-0046、ADR-0050、ADR-0051、
  `docs/contracts/local-evolve-repo-locator-observation-producer-v1.schema.json`

## 背景

ADR-0050 已冻结 `forgeos.evolve-repo-locator/v1` 与纯 Evidence adapter，但 observation 仍由 caller 声明；adapter 不读取 report、
repository file 或 source tree，也不验证 content/report/tree/parameters digest preimage。ADR-0020 的真实 Evolve scan 已能解析并校验
`EVOLVE_SCAN_V1: {json}`，却没有把完整 canonical report、当前 working source 和每个 locator file content 收敛成可跨语言重验的
production package。

本 ADR 冻结一个显式 opt-in 的 Catalyst Go producer。它在本地 Unix boundary 上捕获一个已返回的 Evolve report，复用 ADR-0051
的 Git working-source profile，读取 report 中每个 locator 的完整 bounded regular-file bytes，并生成零条或多条 ADR-0050
observation。结果只说明本地 report/source/locator bytes 被按本 profile 观察；不确认 scan 判断、覆盖质量、任务价值、完成、真值、权威、
身份、持久化或外部 effect。

## 决策

### 1. Exact production package

顶层只能包含：

```text
api_version, canonicalization, observations, parameters_manifest,
report_manifest, source_manifest
```

- `api_version = forgeos.governance.local-evolve-repo-locator-observation-production/v1`；
- `canonicalization = forgeos.canonical-json/v1`；
- production 不包含 result、Governance binding、Evidence、Claim、receipt 或 self digest；
- production canonical bytes 最多 16 MiB；所有整数为 signed-int64，禁止 float、bool-as-int、duplicate/unknown field、invalid UTF-8、
  Unicode Cc/bidi/U+2028/U+2029 与不规范 path；
- 派生 identity 为：

```text
production_sha256 = SHA-256(
  "forgeos.governance.local-evolve-repo-locator-observation-production.v1\0" ||
  canonical_production_json
)
```

identity 由 API 返回但不写入 package；它不是 persistence、authority 或 completion receipt。

### 2. Explicit Unix-local capture boundary

普通 Evolve validation path 永远不会调用本 producer。调用方必须显式提供 repository root、完整 scan output、effective
`expected_depth` 和 1–160 bytes bounded `run_id` 才能 opt in。v1 依赖 Unix path、permission、symlink、opened-root 和 process semantics；
非 Unix 必须在 repository/process observation 前失败关闭，不能用另一平台近似行为冒充 v1。

producer 从 parent environment 最多读取 256 entries，只接受恰好一个非空 `PATH`，忽略 `TMPDIR` 与所有其他 parent values。传入共享
source owner 的环境只有 `PATH`；实际 Git child environment 由 source owner 固定为 `HOME=/`、`LANG=C`、`LC_ALL=C`、该 exact
`PATH` 及 `GIT_CONFIG_NOSYSTEM=1`、`GIT_CONFIG_GLOBAL=/dev/null`、`GIT_OPTIONAL_LOCKS=0`、`GIT_PAGER=cat`、
`GIT_TERMINAL_PROMPT=0`。Git argv 固定为只读 `rev-parse`/`ls-files` 路径并关闭 fsmonitor、hooks、global excludes 和 pager。

这些约束只是减少 ambient input，不认证 PATH 解析出的 Git binary，也不固定动态库、kernel 或 filesystem。producer 不提供 sandbox、
network/egress/device/credential 或 production-effect containment；一个被替换或恶意的本地 `git` executable 仍可能产生任意 effect。
调用方必须在 opt-in 前另行授权该本地执行并提供所需隔离。

### 3. Parameters manifest

`parameters_manifest` 精确包含：

```text
api_version, canonicalization, contract, expected_depth,
report_profile_id, source_profile_id
```

固定值为：

```text
api_version       = forgeos.evolve-capture.parameters/v1
canonicalization  = forgeos.canonical-json/v1
contract          = evolve_scan_v1
report_profile_id = evolve-scan-canonical-marker-v1
source_profile_id = git-worktree-source-tree-v1
```

`expected_depth` 只允许 `advisory|opportunistic|standard|thorough`，且必须等于 report depth。摘要为：

```text
parameters_sha256 = SHA-256(
  "forgeos.governance.local-evolve-repo-locator-parameters.v1\0" ||
  canonical_parameters_manifest
)
```

每条 observation 的 producer 固定为 `forgeos.local-evolve-repo-locator-observer` / `tool` / `v1`，并携带 caller run ID 与该摘要。
run ID 是本地 correlation text，不是认证 principal、producer receipt 或 unique execution proof。

### 4. Complete canonical report preimage

`report_manifest` 精确包含：

```text
api_version, bytes, canonical_report, canonicalization, profile_id, sha256
```

固定值为 `forgeos.evolve-capture.report/v1`、`forgeos.canonical-json/v1` 与 `evolve-scan-canonical-marker-v1`。
`canonical_report` 是完整且唯一的一行 `EVOLVE_SCAN_V1: {compact JSON}`，包含 marker、不含 narrative/CR/LF；payload 最多 65,536
bytes，连 marker 最多 65,552 bytes。`bytes` 是 exact UTF-8 长度，`sha256` 是完整 marker line raw bytes 的裸 SHA-256。

producer 使用 ADR-0020 parser/validator：拒绝 duplicate/unknown/missing field、额外 JSON value、错误 depth、非法 dimension/status、
locator/path/line/detail、relation/opportunity mapping 与 depth-specific shape。canonical report 按固定 dimension rank
`code,dependencies,security,performance,architecture_drift,test_coverage` 排序；opportunity 按 ASCII ID byte order 排序；各 relation
内部 evidence 顺序保持原样。该检查证明 report 满足结构 contract 且 locator 在该次本地读取中可用，不证明其中 finding、clear、
obvious、candidate task、coverage 或其他 scan 判断正确、完整或获批。

### 5. Shared bounded-interval Git source profile

`source_manifest` 逐字节复用 ADR-0051 `forgeos.command-capture.source-tree/v1` / `git-worktree-source-tree-v1`：tracked stage-0、
non-ignored untracked、tracked deletion、regular/symlink facts、Git sha1/sha256 revision、65,536 entries、单文件 1 GiB、累计 8 GiB 及
canonical path/kind/index-mode rules全部不另造版本。source identity 也直接复用：

```text
source_tree_sha256 = SHA-256(
  "forgeos.governance.local-command-source-tree-profile.v1\0" ||
  canonical_source_manifest
)
```

同一 manifest 不得因 consumer 不同获得第二个 source identity。producer 先 capture source，再校验/canonicalize report、按 rooted opened
directory/file handle 读取 locator regular file 并逐项比对 pre-source entry 的 bytes/digest，随后采样一个共享 local Unix-ms timestamp，
再 capture post source；root、manifest 与 digest 必须 exact equality，否则整次 production 为零。

pre/post equality 仍只是两次 Git inventory+逐项读取形成的 bounded-interval observation，不是原子 filesystem snapshot、Git tree object、
clean-worktree 声明或 execution-time pin。inventory 之后新增、某 entry 已读后的变化，或变化后恢复，都可能不改变 endpoint manifest；
协调的 namespace/content 替换仍是 residual TOCTOU。需要更强保证时必须另行 quiesce writers 或定义 filesystem snapshot/sandbox/
CAS/FD-bound profile，不能提升本 v1 语义。

### 6. Exact zero-or-more locator observations

`observations` 每项完整沿用 ADR-0050 `forgeos.evolve-repo-locator/v1`。最多 240 项，顺序和 multiplicity 固定为：

1. canonical dimension rank；每个非-`unavailable` dimension 的 evidence 原顺序，relation 为 `finding|clear`、opportunity ID 为 null；
2. opportunity ID byte order；每个 opportunity 的 evidence 原顺序，relation 为 `opportunity` 并保留 exact opportunity ID。

同一个 path/line/detail 即使在 dimension 与 opportunity 或不同 relation 重复，也必须生成各自 observation，不得跨 relation/path 去重。
全为 `unavailable` 且没有 opportunity 的合法 partial report 生成空数组；空 observation set 不等于没有扫描、PASS 或完成。

每项 content bytes/SHA-256 必须等于 rooted full-file read，并等于同 path pre-source regular entry；locator/path/line/detail、scan depth/
dimension/relation/opportunity、完整 report digest、parameters digest、source revision/tree、固定 producer 与 caller run ID 全部 exact binding。
整包所有 observation 使用同一 capture timestamp；该值是未认证的本地 clock sample，不证明可信时间或排序权威。

### 7. No automatic ADR-0050 binding

production 没有 Governance binding，producer 不调用 ADR-0050 adapter，也不生成 EvidenceRecord。调用方未来可以为每条 exact observation
分别提供合法 project/scope/subjects/context/policy/sequence/supersession，再调用纯 adapter；该步骤仍最多得到 `ADAPTED_SHADOW`，且
跨 observation 的 sequence、aggregate 和 supersession 是 caller responsibility。零 observation 不得虚构 Evidence。

## 非能力边界

唯一正结果为：

```text
CAPTURED_LOCAL_EVOLVE_LOCATOR_SET (local report/source capture only;
locator set may be empty; no scan judgment, completion, truth, authority,
claim, atom, persistence, or effect attestation)
```

本 producer/package：

- 不认证 OS user、Git binary、producer、Agent、source author、clock、report author 或 reviewer；
- 不确认 finding、clear、opportunity、obvious、candidate task、scan quality、coverage 或 task completion；
- 不创建 Claim/Atom，不满足 hard gate、review、`forge accept` 或 release criterion；
- 不自动调用 ADR-0050，不创建 binding/Evidence，不 append GovernanceRecordJournal，不写 SQLite/Memory/Knowledge；
- 不提供 sandbox/egress/device/external-effect containment，也不授权、阻止、隔离或证明 Git binary 与文件读取可能产生的 effect；
- 不是原子 source snapshot、clean-tree/current-head 证明、remote attestation、durability receipt 或 authority token。

SQLite 保持 v25，无 migration、backfill 或 automatic append。Scaffold/upgrade 复制 contract、fixture、Python checker、Skill 和 active
governance，但不安装 Catalyst-only Go producer；下游没有兼容 producer runtime 时必须记 `not_executed`。

## 交付、状态与验收

本 ADR 已完成 Schema、deterministic fixture、bounded Python universal checker、Catalyst Go producer、governance/Skill 与 scaffold inheritance，
并通过 cross-language golden、对抗测试和独立 fresh-context CLEAN review。Registry 将其列入 `shipped_producers`；fixture 仍必须标注
`PURE_CONTRACT_FIXTURE`，不得冒充 live capture receipt。交付状态不扩大本 ADR 的非能力边界。

验收必须覆盖：

- Go/Python 对 parameters、report、source、每条 observation 与 production canonical bytes/digests/result 逐字节一致；
- finding + clear + 同 path opportunity 的固定顺序与跨 relation multiplicity，以及合法 zero-observation report；
- duplicate/unknown/float/bool-as-int/noncanonical/Unicode/overflow/limit/report order/relation/mapping/digest/source/report drift 全部失败关闭；
- source intermediate symlink、regular-file identity/content race、pre/post drift、missing/deleted/symlink/oversized locator 与 malformed PATH 失败关闭；
- parent secret/TMPDIR 不进入 Git child，非 Unix 在 capture 前失败；失败产生零 production；
- fresh scaffold 和 legacy upgrade 可离线运行 checker，但不宣称安装 Go runtime、scan truth、persistence 或 authority。

## 被拒方案

1. 只摘要 report：无法重验完整 locator mapping，且摘要不是 preimage；
2. 每个 unique path 只建一条 observation：会丢失 relation、dimension、detail 与 opportunity membership；
3. 为 source 新建 Evolve digest domain：同一 manifest 获得双 identity，破坏跨 producer 可比性；
4. 自动调用 ADR-0050：缺少 caller Governance binding，会把 local capture 偷换成 Evidence admission；
5. 把 read-only Git argv 称为无副作用：未认证 binary 且无 sandbox，不能证明 external effects；
6. 把 pre/post equality 称为原子 snapshot：endpoint equality 无法排除 transient drift 或 late inventory changes。

## 重审触发器

- 需要认证 Git/producer/clock、sandbox/egress/effect containment 或 remote capture；
- 需要原子 source snapshot、CAS/FD-bound execution 或 writer quiescence receipt；
- ADR-0020 report wire、ADR-0050 observation wire或共享 source profile发生 version change；
- 240 observations、64-KiB report、1-MiB locator file或16-MiB production 边界不足；
- 需要由 runtime 自动生成 Governance binding、Evidence 或 journal receipt。
