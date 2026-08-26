# ADR-0051 — Local Gate Command Observation Producer v1

- 状态：已采纳并交付（2026-08）
- 范围：Wave 1-D；Unix 本地 `gate|check|accept|acceptance --json` exact command observation production
- 关联：ADR-0020、ADR-0045、ADR-0049、ADR-0050、
  `docs/contracts/local-gate-command-observation-producer-v1.schema.json`

> 维护说明（2026-08-23）：本 ADR 冻结的是 observation producer，不是对 gate JSON 协议的永久兼容承诺。
> 共享的 `ProbeAll` 解析契约后续已 fail-closed 收紧为 exact 11 rows、每行 exact four fields：非零退出搭配至少一个
> `FAIL`/`N-A` 的合法 mixed envelope 仍可解析，非零退出搭配 all-`PASS` envelope 则作为矛盾失败关闭；任一 stream
> 超出 output cap 时优先报告 truncation，不先解释 exit 或 JSON。Standard 与 observed 路径共享该语义；producer wire
> 与非能力边界不变。

## 背景

ADR-0049 已冻结 strict `forgeos.command-observation/v1` wire 与纯 Evidence adapter，但它只接受 caller 提供的 observation：不会执行
命令、重算 stream preimage 或认证 producer。现有 Go gate bridge 会执行真实 harness，却不能从 `gate.Result` 事后诚实恢复 observation：
Result output 已被 trim，并可能附加展示用截断标记；旧 `execbound` 也没有同时保存 stdout、stderr 与 producer drain-order combined 的完整和
retained-prefix digest。

本 ADR 冻结一个显式 opt-in 的 local producer。它使用 secret-scrubbed 的明确 child environment，解析并摘要实际顶层 executable，生成
bounded-interval Git working-source manifest，并在 execution boundary 捕获 raw streams 与 termination。输出是 local observed production package，不是 gate
裁决、身份认证、完成证明或 persistence receipt。

v1 明确是 Unix-family local profile：依赖 Unix executable permission、绝对 `PATH` component、POSIX symlink 与 process-group termination
语义。非 Unix runtime 必须在 profile/snapshot/process spawn 前失败关闭，不得用 Windows `PATHEXT`、drive-relative path 或不同 process
termination 语义伪装成 v1；支持其他平台需要新的显式 profile/version 与独立兼容审查。

## 决策

### 1. Exact production package

顶层只能包含：

```text
api_version, canonicalization, environment_manifest, observation,
source_manifest, tool_manifest
```

- `api_version = forgeos.governance.local-gate-command-observation-production/v1`；
- `canonicalization = forgeos.canonical-json/v1`；
- environment、tool、source manifest 都是 closed-world object；production 顶层不包含 `result`、binding、Evidence、receipt 或 self digest；
- manifest/production canonical bytes 最多 16 MiB；普通 text 同时最多 4,096 Unicode scalar 与 16,384 UTF-8 bytes；禁止 invalid UTF-8、
  Unicode Cc、bidi controls、U+2028/U+2029、signed-int64 overflow 与不规范 path；
- production 的派生 identity 为：

```text
production_sha256 = SHA-256(
  "forgeos.governance.local-command-observation-production.v1\0" ||
  canonical_production_json
)
```

identity 由 API 返回但不写进 package；它不是 receipt 或 authority。

### 2. Closed local command profile

v1 只允许四个 command class 及 exact direct-exec argv：

```text
gate      = ["node", "harness/gate.mjs"]
check     = ["python3", "harness/check.py", "."]
accept    = ["node", "harness/acceptance.mjs"]
probe_all = ["node", "harness/acceptance.mjs", "--json"]
```

logical cwd 固定 `.`，stdin 固定为空，evidence type 固定 `gate_result`。observation producer 固定：

```text
producer_id      = forgeos.local-gate-command-observer
producer_type    = tool
producer_version = v1
```

run id 必须由调用边界显式提供并进入 observation identity。一次真实 process spawn 只生成一条 observation；`acceptance --json` 中的 lint、
test、build、security criterion 是该命令输出的派生判断，不能被复制成多条虚构 process observation。

### 3. Exact scrubbed child environment

environment manifest 精确包含：

```text
api_version, canonicalization, profile_id, variables[{name,value}]
```

固定值：

```text
api_version = forgeos.command-capture.environment/v1
profile_id  = scrubbed-parent-environment-v1
```

producer 读取最多 256 个 parent entries，拒绝非法或重复名称；保留项按 name byte order 唯一排序，并逐项构造实际传给 command 的显式
`name=value` child environment。`PATH` 必须存在。以下 key 必须在 manifest 与 child environment 中彻底剔除：

- 名称含 `API_KEY|AUTH|BEARER|COOKIE|CREDENTIAL|CREDENTIALS|OAUTH|PASSWD|PASSWORD|PRIVATE_KEY|SECRET|SESSION|TOKEN`；
- 名称以 `ANTHROPIC_|AWS_|AZURE_|CLOUDFLARE_|DIGITALOCEAN_|DOCKER_AUTH|GCP_|GCLOUD_|GITHUB_|GITLAB_|
  GOOGLE_|KUBE|OCI_|OPENAI_|SSH_|GPG_|VAULT_` 开头；
- exact proxy names `HTTP_PROXY|HTTPS_PROXY|ALL_PROXY|NO_PROXY`，匹配大小写不敏感。

该 scrub 是 closed name policy，不是 value DLP；调用方不得把 secret 放入看似普通的名称。保留 value 会进入 canonical manifest，因此仍是
internal-sensitive observation material，不得默认输出到日志。command 的 environment digest 为：

```text
environment_sha256 = SHA-256(
  "forgeos.governance.local-command-environment-profile.v1\0" ||
  canonical_environment_manifest
)
```

### 4. Resolved top-level tool manifest

tool manifest 精确包含：

```text
api_version, bytes, canonicalization, final_path, mode, profile_id,
requested_path, resolved_path, sha256, symlink_hops[{path,target}]
```

固定值：

```text
api_version = forgeos.command-capture.tool/v1
profile_id  = resolved-top-level-executable-v1
```

producer 从 scrubbed environment 的 exact `PATH` 解析 `node|python3`，要求每个 PATH component 是 normalized absolute path，最多跟随 32 个
symlink hop，拒绝 cycle，并要求 final path 是 executable regular file。manifest 记录 requested/resolved/final path、permission mode、完整 file
bytes/SHA-256 与每个 symlink path/target；顶层 executable 最多 1,073,741,824 bytes。observed execution 使用该 resolved executable path，
而不是重新按 ambient environment 猜测。

```text
tool_snapshot_sha256 = SHA-256(
  "forgeos.governance.local-command-tool-profile.v1\0" ||
  canonical_tool_manifest
)
```

该摘要只覆盖顶层 executable，不覆盖动态库、Node/Python module graph、package manager、kernel、container 或供应商身份；read→exec 仍存在本地
TOCTOU，故它不是 executable pinning 或 remote attestation。

### 5. Full Git working-source manifest

source manifest 精确包含：

```text
api_version, canonicalization, entries, profile_id, source_revision
```

固定值：

```text
api_version = forgeos.command-capture.source-tree/v1
profile_id  = git-worktree-source-tree-v1
```

producer 要求 root 是 canonical Git toplevel，使用 scrubbed child environment 和 hardened Git flags 枚举：

- 全部 tracked index stage-0 entries；
- 全部 non-ignored untracked entries；
- `docs/release` 明确包含；仅 canonical **untracked** `.forge` 被排除，tracked `.forge` 必须失败关闭；
- gitlink、unmerged stage、`.git` control path、noncanonical/escaping/backslash path、tracked index/working-tree kind drift 与 unsupported file
  type 必须拒绝。

最多 65,536 entries、每个 regular file 最多 1,073,741,824 bytes、symlink target 受通用 text limit 约束、累计 content 最多 8 GiB，
并按 canonical repo-relative path byte order 唯一排序。每项精确包含：

```text
bytes, content_sha256, executable, index_mode, kind, path,
symlink_target, tracking
```

- regular：完整 bytes/digest、executable bool、null symlink target；
- symlink：target UTF-8 bytes/digest、`executable=false`、target string；
- tracked deleted：zero bytes，content/executable/target 为 null；
- tracked 带 exact index mode，untracked 的 index mode 为 null。

Git object format 只允许 sha1/sha256，revision 分别编码为 `git-sha1:<40 lower hex>` 或 `git-sha256:<64 lower hex>`。

```text
source_tree_sha256 = SHA-256(
  "forgeos.governance.local-command-source-tree-profile.v1\0" ||
  canonical_source_manifest
)
```

observation.source revision/tree 必须逐字节等于 manifest revision 与上述 digest。该 manifest 是一次 Git inventory 加逐项读取形成的有界区间
观察，不是 Git tree object、原子文件系统快照、clean-worktree 声明、submodule content 证明或 source author attestation。pre/post 两次
source scan 与 exact equality 只检测两次区间观察之间可见的 drift；inventory 之后才出现的 entry，或已读取 entry 的后续变化，可能不属于该次
manifest，即使两次结果相等也不代表 execution-time source 被 pin。协调的本地 namespace/content 替换后恢复同样属于 residual TOCTOU；
端点相等不证明 command 执行期间 source 从未短暂漂移或从未经 symlink 读取仓外内容。调用方需要更强保证时，必须先 quiesce writers 或
定义新的 filesystem snapshot/sandbox/CAS/FD-bound execution profile，不能提升本 v1 package 的语义。

### 6. Raw execution observation

`observation` 完整沿用 ADR-0049 `#CommandObservationV1` shape 与 semantic validation，不新建第二种 command wire。Schema 在同一文件复制
所需 `$defs`，避免离线 validator 因跨文件 `$ref` 支持差异而失效。

opt-in observed execution 对 stdout、stderr 和 producer-serialized combined drain events 分别记录完整与 retained-prefix bytes/SHA-256：

- full digest 继续覆盖 cap 后被丢弃的 raw bytes；retained digest 只覆盖实际保留的 prefix；
- combined bytes 必须等于 stdout+stderr，combined ordering 只声明 producer writer serialization，不声明 kernel/child emission order；
- trim、parse、日志前缀和展示用 `…[output truncated: ...]` marker 不得进入摘要 preimage；
- real nonnegative exit → `exited`；controller deadline/cancel → `timed_out|cancelled + null`；
- not-started、spawn-failed、signaled、wait-failed、incomplete drain 或 signed-int64 count overflow 必须产生零 production，不可造 sentinel。

timeout/cancel observation 可由 ADR-0049 observation validator 接受，但其 Evidence adapter 仍只投影 `exited`。非零 exit 仍是合法 process
observation，不代表 gate truth；exit zero 同样不等于 PASS。

### 7. Optional Evidence adaptation

producer package 不包含 Governance binding。只有调用方另行提供合法的 project/scope/subjects/context/policy/sequence/supersession，才可把 exact
observation 放入 ADR-0049 request 并调用 pure adapter。该步骤最多得到 `ADAPTED_SHADOW`，不会认证本 producer、environment、tool、source 或
command output。

## 非能力边界

唯一正 producer result 文案为：

```text
OBSERVED_LOCAL_PROCESS (local process capture only; no pass, criterion,
completion, truth, authority, identity, persistence, or external-effect attestation)
```

本 producer/package：

- 不把 exit、PASS/FAIL 文本、acceptance criterion 或 Evidence `valid` 状态解释为 gate truth、task completion 或 `forge accept` authority；
- 不认证 OS user、producer、collector、binary vendor、source author、environment、toolchain 或 timestamp；
- 不创建 Claim/Atom，不满足 hard guard，不签发或消费 Grant/Approval；
- 不 append GovernanceRecordJournal，不写 SQLite/Knowledge/Memory；它会执行仓库控制的本地命令，且自身不提供 sandbox、egress、device 或
  production-effect containment，因此既不授权、阻止或隔离命令可能产生的外部 effect，也不对这些 effect 作出 attestation；调用方必须在
  opt-in 前另行完成命令授权与所需的运行隔离；
- producer 层不独立改写 gate Result、criterion parse、N/A exemption、convergence 或 CLI exit contract；standard/observed
  路径均消费本 ADR 顶部维护说明所述的当前共享 `ProbeAll` 契约；
- 不接线 Evolve locator producer。

SQLite 保持 v25，无 migration、backfill 或 automatic append。capture disabled 必须继续使用 legacy execution path并保持 stdout、stderr、
error、Result 与 exit byte-exact。

## 兼容、交付与验收

本 ADR 的晋级门槛已由真实 golden、完整测试、两份独立 CLEAN 复审和最终 `forge accept` 满足；Roadmap/Audit 与 registry 因而标记为
`DONE`/`shipped_producers`。该状态只表示本节 closed-world producer 已交付，不扩大其非能力边界；没有真实生成值时仍不得提交占位 fixture。

验收至少覆盖：

- exact canonical/duplicate/unknown/float/overflow/Unicode/path/list/limit rejection；
- secret/cloud/auth/proxy key 从 manifest 与 actual child env 同时消失，普通变量 value/order/duplicate drift 受控；
- tool requested/resolved/final/symlink/mode/bytes/digest 与 source revision/all-entry manifest 的 exact drift；
- stdout-only、stderr-only、interleaved、empty、nonzero、cap overflow、timeout、cancel、spawn/signaled/wait/drain failure；
- full digest 不受 retention cap 或展示 marker 污染；
- Gate/Check/Accept/Probe observed API 与 disabled standard path parity；`ProbeAll` 的 exact 11-row/four-field 解析不漂移，
  nonzero mixed `FAIL`/`N-A` 仍是合法 verdict，nonzero all-`PASS` 失败关闭，且任一 stream 的 cap overflow
  均优先报告 truncation；
- run/evolve cache 与并发路径保证每个 actual spawn 只产生一条 production；
- explicit ADR-0049 adaptation exact replay，但不产生 PASS/truth/completion/authority/persistence 语义。

## 被拒方案

1. 从 `gate.Result.Output` 事后 hash：它不是 raw stream；
2. 只 hash retained prefix：cap 后不同输出会错误合并；
3. 继承 ambient environment 但只记录变量名：manifest 与 actual child process 不一致；
4. 将 secret value 本身 hash 进 profile：会形成离线猜测 oracle；
5. 每个 acceptance criterion 伪造独立 process observation；
6. capture 默认开启，或 capture failure 改写 disabled legacy result；
7. producer 自行发明 Governance binding、写 journal 或把 local observation 提升为 authority。

## 重审触发器

- ADR-0049 新版本能表达 spawn/signaled/not-started/incomplete-drain；
- scrub policy、PATH/executable resolution、source inventory/limit/Git algorithm 或 digest domain 改变；
- 需要完整 module/dynamic-library/environment value attestation 或 executable pinning；
- remote/sandbox runner 需要独立 identity、clock 和 source profile；
- operator 授权 observation sidecar、journal persistence 或 cross-machine verification。
