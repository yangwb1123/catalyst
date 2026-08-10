# ADR-0053 — Local Go Package Dependency Graph Observation Producer v1

- 状态：已采纳并交付（DONE，2026-08）
- 范围：Wave 2-A；Unix 本地 selected-module lexical Go package dependency observation
- 关联：ADR-0045、ADR-0046、ADR-0051、ADR-0052、
  `docs/contracts/local-go-package-dependency-graph-observation-producer-v1.schema.json`

## 背景

现有架构审查只能读取 caller 声明的依赖关系，或从一次 selected build 推断局部图。前者不能重验 source binding，后者会受 GOOS、GOARCH、
build tags、test selection、workspace、vendor、replace、module cache 与 dependency availability 影响，不能诚实表示 repository 中全部 regular
Go files 的 lexical import surface。

本 ADR 冻结一个显式 opt-in 的 Catalyst Go producer：选择一个含 regular `go.mod` 的 module directory，复用 ADR-0051 working-source
manifest，在 source-derived nested-module boundary 之外读取所有 regular `.go` 文件，仅解析 package clause 与 import header，形成 package nodes、
lexical dependency candidates、稳定 diagnostics 和 selection accounting。结果用于后续观察与审查输入，不直接作 architecture judgment、impact
closure、Evidence、Claim、Atom、持久化或发布授权。

## 决策

### 1. Exact production 与 identity

顶层只能包含：

```text
api_version, canonicalization, graph_observation, parameters_manifest,
source_manifest
```

固定值：

```text
api_version      = forgeos.governance.local-go-package-dependency-graph-observation-production/v1
canonicalization = forgeos.canonical-json/v1
```

production 不包含 result、self digest、Governance binding、Evidence、Claim、receipt 或 persistence fact。canonical production 最多
16 MiB；JSON 只接受 signed-int64，拒绝 float、duplicate/unknown field、invalid UTF-8、Unicode Cc/bidi/U+2028/U+2029 与不规范 path。

四个 digest identity 为：

```text
parameters_sha256 = SHA-256(
  "forgeos.governance.local-go-package-dependency-graph-parameters.v1\0" ||
  canonical_parameters_manifest)

graph_sha256 = SHA-256(
  "forgeos.governance.local-go-package-dependency-graph-observation.v1\0" ||
  canonical_graph_observation)

source_tree_sha256 = SHA-256(
  "forgeos.governance.local-command-source-tree-profile.v1\0" ||
  canonical_source_manifest)

production_sha256 = SHA-256(
  "forgeos.governance.local-go-package-dependency-graph-observation-production.v1\0" ||
  canonical_production)
```

source identity 逐字节复用 ADR-0051，不因 consumer 不同另造摘要域。所有 identity 都不是 signature、authority、durability 或 completion
receipt。

### 2. Parameters 与 selected module

`parameters_manifest` 精确包含：

```text
api_version, canonicalization, file_selection_profile_id,
import_resolution_profile_id, module_directory, module_profile_id,
parser_profile_id, source_profile_id
```

固定 profile：

```text
api_version                 = forgeos.go-package-dependency-capture.parameters/v1
file_selection_profile_id   = selected-module-all-regular-go-files-union-v1
import_resolution_profile_id= selected-module-lexical-import-resolution-v1
module_profile_id           = selected-go-mod-module-directive-v1
parser_profile_id           = go-parser-imports-only-no-partial-facts-v1
source_profile_id           = git-worktree-source-tree-v1
```

`module_directory` 只能为 `.` 或 canonical repository-relative directory。它自己的 `<directory>/go.mod` 必须是 source manifest 中当前
regular file、最多 1 MiB；`module.directory/go_mod_path/go_mod_bytes/go_mod_content_sha256` 必须逐项绑定 source。producer 从该文件提取一个
canonical `module_path`。v1 不解析 `go.work`、workspace、require、replace、exclude、retract、vendor 或 module graph。

v1 将 module path 和 import path 的 lexical canonical profile 冻结为一个保守的、跨 Go/Python 一致的 ASCII 子集：只允许
ASCII 字母数字、`-._~+` 和 `/` 分隔；拒绝整体以 `/` 或 `-` 开头、任意组件以 `.` 开头、尾部 `/`、`//`、空组件、全点组件、
组件尾点、连续 `..`、Windows reserved basename（`CON/PRN/AUX/NUL/COM1..9/LPT1..9`，不区分大小写）与
`~<digits>` short-name suffix。这是 v1 contract，不随 host Go toolchain 或 `x/mod` 版本漂移。
module directive scanner 只接受括号深度为零的顶层 `module`；它识别 quoted token 与 `//` line comment，对 `/* */`、
嵌套或不平衡 directive block 及未闭合 quote 失败关闭，但不因此声称已验证其他 go.mod directive 语义。

selected directory 下任何 descendant `go.mod`，只要 source kind 为当前 `regular` 或 `symlink`，都形成 nested-module boundary 并排除完整
subtree；`deleted go.mod` 不形成 boundary。`nested_modules` 只能由 source 重算，精确按 `(directory,go_mod_path)` byte order 排序，每项仅有
`directory,go_mod_path,kind`，caller 不能任意声明或遗漏 symlink boundary。

### 3. All-regular-Go-file lexical union

在 selected subtree 内，先排除 nested boundaries，再按 source manifest 选择所有 current regular `.go` 文件。symlink、deleted 等非-regular
Go entry 不读取；其 exclusion 进入 coverage。v1 故意忽略 filename suffix、GOOS、GOARCH、build constraints、build tags、cgo enablement、
test selection 与 generated/vendor/testdata meaning：它观察全部 regular-file lexical union，不声称任何 selected build。

每个成功文件精确包含：

```text
bytes, content_sha256, imports, package_name, path, role
```

`bytes/content_sha256` 必须匹配 source；`role=test` 当且仅当 path 以 `_test.go` 结束，否则为 `compile`。producer 以 Go parser
imports-only mode 读取 package clause 与 import specs；v1 为避免 Go/Python Unicode table 漂移，只接受非 keyword 的 ASCII Go package
identifier `[A-Za-z_][A-Za-z0-9_]*`，其他 parser-valid package name 形成 `go_file_unsupported_text` diagnostic。imports 在单文件内去重并按
UTF-8 byte order 排序。它不 type-check、resolve、compile、
execute或调用 `go` 命令。

一个文件失败时不得发布任何 partial package/import fact，只发布恰好一个 `{code,path}` diagnostic。稳定 code 只有：

```text
go_file_exceeds_parser_limit
go_file_import_limit_exceeded
go_file_invalid_utf8
go_file_parse_error
go_file_unsupported_text
```

parser 原始错误文本不得进入 wire。diagnostics 按 `(path,code)` 排序且 path 全局唯一；oversize code 当且仅当 source bytes 超过 4 MiB。

### 4. Package nodes

成功文件按 exact `(directory,package_name)` 分组，节点精确包含：

```text
compile_files, directory, import_path, name, test_files
```

两组 file paths 分别排序。只有该 exact node 的 `compile_files` 非空，`import_path` 才是
`module_path + repository-relative-directory-suffix`；test-only external package（例如 `foo_test`）的 `import_path` 必须为 null，不能冒充可被
local import 命中的 compile package。同一 directory 可有多个 compile-bearing package node；这被诚实保留，供 local edge 标记 ambiguous，
而不是把 compile-invalid directory 合并或删除。

### 5. Lexical dependency candidates

每个成功文件的每个 unique import 形成 occurrence，再按
`(from_directory,from_package_name,role,import_path)` 聚合 `source_paths`。edge 精确包含：

```text
from_directory, from_package_name, import_path, relation, resolution,
resolution_detail, role, source_paths, target_directory, target_package_name
```

`relation=depends_on`；edges 按 source directory、package、role rank `compile<test`、import path 排序。resolution 是 closed lexical
classification：

| resolution | detail | target |
|---|---|---|
| `local` | null | directory + sole compile package name |
| `unresolved_local` | `no_compile_package` | lexical directory only |
| `ambiguous_local` | `multiple_compile_packages` | lexical directory only |
| `nested_module_boundary` | `nested_module_boundary` | nested lexical directory only |
| `stdlib_candidate` | null | null |
| `external_candidate` | null | null |
| `cgo_pseudo` | null | null |
| `unsupported` | `noncanonical_import_path` | null |

`C` 固定为 cgo pseudo。canonical import 的 first segment 含 dot 时为 external candidate，否则为 stdlib candidate；等于 selected
module path 或以其加 `/` 为前缀时，映射到 selected directory 后按 nested boundary 与 compile-bearing nodes 分类。这里的 local、stdlib、
external 全是 lexical candidate，不证明 package 存在、可下载、可编译、可测试或 runtime reachable。

### 6. Coverage 与 bounded failure

coverage 精确包含：

```text
go_entries_excluded_by_nested_module
go_entries_excluded_nonregular
go_entries_in_selected_subtree
regular_go_files_parsed
regular_go_files_selected
regular_go_files_with_diagnostics
```

checker 从 source、nested boundaries、files 与 unique diagnostic paths 重算，并强制：

```text
in_selected_subtree = excluded_nested + excluded_nonregular + selected
selected            = parsed + diagnostics
```

上限为：selected regular files 16,384；nested modules 1,024；单 Go parser input 4 MiB；所有会被读取/尝试解析且不 oversize 的 selected
regular files累计 64 MiB；imports/file 1,024；import occurrences 65,536；packages 16,384；edges 65,536；diagnostics 16,384；文本
4,096 Unicode scalars/16 KiB；run ID 160 bytes。共享 source 仍为 65,536 entries、单 entry 1 GiB、累计 8 GiB。单文件命中上述五类
受支持解析失败时形成唯一 diagnostic；集合限额、selected go.mod 解析、source/read/binding/canonical seal 失败才使整次 live production 为零。
不得截断并冒充完整 graph。

### 7. Graph binding 与 universal checker

`graph_observation` 精确包含：

```text
api_version, canonicalization, profile_id, coverage, dependencies,
diagnostics, files, module, observed_at_unix_ms, packages, producer, source
```

固定 graph API/profile 为 `forgeos.go-package-dependency-graph-observation/v1` /
`selected-go-module-lexical-dependency-graph-v1`。producer 固定为
`forgeos.local-go-package-dependency-graph-observer` / `tool` / `v1`，携带 caller run ID 和 parameters digest；source binding 携带 shared
revision/tree digest。timestamp 只是未认证 local Unix-ms sample，不是可信时间。

对任意 production，bounded Python universal checker 会重验 canonical JSON、parameters/source 的嵌入 digest bindings、source profile/binding、
nested boundaries、coverage partition、package grouping 和完整 edge derivation；`--golden` 另将 parameters/source/graph/production 的 canonical
bytes 和四个 domain digests 与 fixture expected 逐字节比较。checker 明确不解析 Go，也不从 source preimage 重新判断 package/import syntax；只有 live Go producer 负责
Go parser observation。production 不携带 source file preimage，因此 checker 对 selected `go.mod` 只能重验 regular source metadata
binding 与 observed `module_path` 的 bounded canonical shape，不能从 digest 反推出或重新验证 module directive；该事实仍属于未认证 local
producer observation。golden fixture 的 Go text 只用于逐字节重验 source bytes/digests，标注 `PURE_CONTRACT_FIXTURE`，不得冒充 live capture。

### 8. Explicit Unix-local capture boundary

普通 governance、Evolve、review 或 gate path 永不自动调用本 producer。调用方必须显式提供 canonical repository-root path、module
directory、run ID 与所需本地授权；producer 内部执行 rooted open 与 identity binding。v1 仅支持 Unix；非 Unix必须在
repository/process/file observation 前失败关闭。

producer 逐字节复用 ADR-0051 source owner：parent environment 只接受一个 bounded nonempty `PATH`；Git child 使用固定 read-only
`rev-parse`/`ls-files` argv 与 scrubbed `HOME=/`、`LANG=C`、`LC_ALL=C`、exact PATH、Git no-config/no-lock/no-pager/no-prompt flags。
producer 本身不调用 `go`、不访问 module cache、也不按 profile 发起网络请求。但 PATH 解析的 Git binary 未认证，且没有 sandbox、egress、
device、credential 或 external-effect containment，因此不能把 read-only argv 提升为无副作用证明。

capture 顺序为 pre-source、rooted bounded file reads/parse、共享 timestamp、post-source；opened root、manifest 与 digest 必须 exact equality。
endpoint equality 仅形成 bounded-interval observation，不是 atomic filesystem snapshot，不能排除 transient drift、late inventory changes 或协调的
namespace/content replacement。需要更强保证时必须定义 filesystem snapshot、CAS/FD-bound profile 或 quiesced writer protocol。

## 非能力边界

唯一正结果为：

```text
OBSERVED_LOCAL_GO_PACKAGE_DEPENDENCY_GRAPH (all-regular-Go-file lexical
import-header/source observation only; no selected build, dependency
availability, compile success, architecture judgment, impact closure,
completeness, truth, authority, claim, atom, persistence, or effect attestation)
```

本 producer/package 不确认 GOOS/GOARCH/build tags/workspace/vendor/replace/module graph、dependency availability、compile/test/runtime
reachability、architecture quality、change impact、coverage completeness、truth、authority、identity、persistence 或 effect containment；不创建
Governance binding、Evidence、Claim、Atom，不 append journal，不写 SQLite/Memory/Knowledge，不满足 hard gate、review 或 release criterion。
Hub 保持当前 SQLite v26；本 ADR 不增加 migration、backfill 或 automatic append。

## 交付与验收

本 ADR 已交付 Schema、rich deterministic fixture、strict Python checker、Catalyst Go producer、governance/Skill 与 scaffold inheritance。
独立复审已完成；最终状态仍以本次真实 `forge accept` 结果为唯一完成裁决，registry/pins 必须保持一致。验收覆盖：

- Go/Python 对 parameters/source/graph/production canonical bytes、四个 digests 与 result 完全一致；
- build-tagged files、compile/test union、test-only null import path、duplicate imports 的 source-path aggregation；
- local、unresolved、ambiguous、nested、stdlib、external、cgo、unsupported 全部 resolution；
- regular 与 symlink nested boundary、deleted go.mod 非-boundary、nonregular exclusions 与 coverage equations；
- duplicate/unknown/float/bool-as-int/Unicode/path/order/profile/digest/source/package/edge drift 失败关闭；
- selected count、aggregate parser input、per-file imports、occurrences、packages、edges、diagnostics 与 production limits；
- 每失败文件唯一 diagnostic、无 partial facts、稳定 code 且无 parser message；
- pre/post drift、intermediate symlink/race、malformed PATH、非 Unix均产生零 production；
- direct script/module CLI 可校验 golden 与 strict canonical production，fresh scaffold/upgrade 可离线执行 checker。

## 被拒方案

1. 只观察 selected build：隐藏其他平台、tag 和 test lexical surface；
2. 运行 `go list`：引入 workspace/module-cache/network/dependency availability 与 toolchain ambient semantics；
3. 把 API DTO 或 caller graph 当 observation：无法重验 source、selection、package 与 edge derivation；
4. test-only package 填 module import path：会把无 compile files 的 external test package冒充 local import target；
5. symlink go.mod 不作 boundary：可能读取本次 source 中明确存在的跨-module subtree；
6. 一个失败文件发布 partial imports：下游无法区分 complete header 与 parser 截断；
7. Python checker再次实现 Go parser：会制造第二套语言语义并产生跨语言漂移；
8. 把 pre/post equality 称为 atomic snapshot：endpoint equality 无法证明期间未变化。

## 重审触发器

- 需要 selected build、GOOS/GOARCH/tag/cgo、workspace/vendor/replace 或 module-graph-aware semantics；
- 需要 compile/test/runtime reachability、architecture adjudication 或 impact closure；
- 需要认证 Git/producer/clock、sandbox/egress/effect containment、remote attestation 或 atomic snapshot；
- Go grammar/parser profile、shared source profile、limits 或任一 wire/digest domain需要变更；
- runtime 需要自动生成 Governance binding、Evidence、journal receipt 或持久化状态。
