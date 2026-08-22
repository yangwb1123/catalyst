# ADR-0062 — Local Go Package ImpactPreScan v1

- 状态：已实现（2026-08；v1 合同、pure local Go runtime、strict Python checker 与 exact fixture 已交付）
- 范围：Wave 2-B；对一份 exact ADR-0053 Go package dependency observation 做 bounded、
  authority-free、local-only 的反向词法依赖预扫描
- 关联：ADR-0037、ADR-0053、
  `docs/contracts/local-go-package-impact-prescan-v1.schema.json`

## 背景

ADR-0053 已能显式捕获一个 selected Go module 的全部 current regular `.go` files，形成绑定 source revision/tree、
package nodes、compile/test lexical imports、resolution、diagnostics 与 coverage 的 exact observation。它故意不做 change
impact。Wave 2 的完整 `GraphSnapshot` 和 `ChangeImpactReport` 又要求 API/event、DB/migration、deployment、ADR/owner、
runtime 等多源图，不能从一个 Go import observation 冒充得到。

本 ADR 冻结中间最小切片：调用方提供一份完整、逐字节 canonical 的 ADR-0053 `graph_observation`、它的 domain digest、
原 observation run ID，以及一组已 canonical 排序的 changed paths；纯本地 evaluator 只计算 selected-module lexical
package graph 中的已知反向依赖闭包。结果能回答“在这份 observation 的精确 local import edges 内，哪些 package 已知可能
受影响”，同时把缺失种子和图缺口显式标为 UNKNOWN。它不是完整 System Knowledge Graph，也不能满足 final Assessment Join。

## 决策

### 1. 单一 exact envelope

最终 wire 只能包含：

```text
api_version, canonicalization, envelope_sha256, report, request
```

固定值：

```text
api_version      = forgeos.governance.local-go-package-impact-prescan/v1
canonicalization = forgeos.canonical-json/v1
```

`request` 是输入；`report` 是完全由 request 重新推导的输出。envelope 不包含签名、Grant、Approval、Evidence、Claim、
GraphSnapshot、ChangeImpactReport、Cost、Risk、receipt、persistence 或 effect fact。任何字段不得由 caller 任意补写；
validator 必须从 exact observation 和 changed paths 重建 report 后逐字节比较。

### 2. Exact ADR-0053 observation 输入

request 精确包含：

```text
api_version, canonicalization, changed_paths,
graph_observation_base64url, graph_observation_sha256,
request_sha256, run_id
```

固定 API 为 `forgeos.governance.local-go-package-impact-prescan-request/v1`。`graph_observation_base64url` 是 ADR-0053
`graph_observation` 的**完整 exact canonical JSON bytes**经 RFC 4648 URL-safe、无 padding Base64 编码后的字符串；不接受标准
Base64 alphabet、`=` padding、非最短编码或 decode 后重新编码不一致。decoded bytes 必须：

1. 是单个 exact compact UTF-8 canonical JSON object；
2. 完整满足 ADR-0053 `forgeos.go-package-dependency-graph-observation/v1` 的 strict graph contract；
3. canonical re-encode 后与 decoded bytes 逐字节相同；
4. 用 ADR-0053 原 domain 重算出 request 声明的 `graph_observation_sha256`；
5. 其 `producer.run_id` 与 request `run_id` 完全相同。

本合同不接受只给 parsed object、截断 observation、仅 package/edge 摘要或由 caller 重述的 source facts。它也不读取
ADR-0053 顶层 production、source manifest 或当前 repository，因此不重新证明 live capture、Git 身份或 source freshness。

`changed_paths` 必须有 1..256 个 canonical repository-relative paths，按原始 UTF-8 bytes 严格升序且全局唯一。evaluator
不得静默排序、去重、改 separator、Unicode normalize 或猜测 rename；违反即整份失败。路径使用 ADR-0053 的 lexical
repository-path domain，禁止 absolute/drive path、backslash、`.`/`..` component、empty component、trailing slash、控制/bidi
字符，以及首 component 的 ASCII-case-insensitive `.git`/`.forge` control alias。

路径位于 selected module 当且仅当 module directory 为 `.`，或 path 等于 module directory，或 path 以
`module_directory + "/"` 开头；nested boundary 使用相同的 component-boundary 规则。`not_a_go_file` 当且仅当已位于 selected
module、未进入 nested boundary且不以 exact lowercase `.go` 结尾。不得用裸字符串前缀把 `service2` 误判为 `service` 内部。

### 3. Package node 与 edge identity

每个 observation package 形成一个候选 node。node identity projection 精确包含：

```text
directory, import_path, module_path, package_name
```

其中 `module_path` 来自 observation module；其余字段逐字节来自 package。`import_path` 对 test-only package 可为 null。
node SHA 与 ID 为：

```text
node_sha256 = SHA-256(
  "forgeos.governance.local-go-package-impact-prescan-node.v1\0" ||
  canonical_node_identity_projection)

node_id = "go-package-node-" || node_sha256
```

只有 ADR-0053 `resolution=local`、`resolution_detail=null` 且 exact target package 存在的 dependency 才形成 traversable edge。
edge identity projection 精确包含：

```text
from_node_id, import_path, relation, role, source_paths, to_node_id
```

所有字段逐字节由 observation dependency 与 resolved package nodes 重建：

```text
edge_sha256 = SHA-256(
  "forgeos.governance.local-go-package-impact-prescan-edge.v1\0" ||
  canonical_edge_identity_projection)

edge_id = "go-package-edge-" || edge_sha256
```

`local` 仍只是 ADR-0053 的 selected-module lexical resolution，不认证 dependency availability、compile success、selected build
或 runtime reachability。其它 resolution 不生成虚构 edge。

### 4. Seed resolution

每个 changed path 必须恰好进入一个 `resolved_seeds[*].changed_paths` 或一个 `unresolved_seeds[*].changed_path`，不能遗漏、
重复或跨两者出现。

成功解析的 observation file 按 exact `(directory, package_name)` 映射到唯一 package node。同一 package 的多个 changed paths
合并为一个 resolved seed；resolved seeds 按 `node_id` 排序，内部 changed paths 按 UTF-8 bytes 排序。
v1 是有意的 package-granularity over-approximation：同 package 的 `_test.go` 与 compile file 都将整个 exact package node 作为 seed；
它不声称 file/symbol/call sensitivity，也不能从 test-only change 推断 production runtime reachability。

无法映射时只允许以下封闭原因，按此优先级判定：

1. `outside_selected_module`；
2. `inside_nested_module_boundary`；
3. `not_a_go_file`；
4. `go_file_diagnostic`；
5. `not_in_observed_file_or_diagnostic`。

只有 `go_file_diagnostic` 携带非 null `diagnostic_code`，且必须等于 observation 对该 path 的唯一稳定 ADR-0053 diagnostic；
其它原因的 `diagnostic_code` 必须为 null。unresolved seeds 按 changed path 排序。由于输入没有 source manifest，第 5 类不再
猜测 absent、deleted、nonregular 或未库存路径的更细原因。

### 5. Exact reverse-reachability closure

令 `G=(V,E)` 为上述 package nodes 和 exact local edges，edge 方向保持 ADR-0053 的 importer `from → to dependency`。令 `S`
为 resolved seed nodes。reachable set `R` 是以下规则的最小不动点：

```text
S ⊆ R
若 edge (u → v) ∈ E 且 v ∈ R，则 u ∈ R
```

`reachable_nodes` 必须恰好是 R，按 node ID 排序。`reachable_edges` 必须恰好是两端均在 R 的 E，按 edge ID 排序；不能只留
shortest-path edges，也不能加入 ambiguous/unresolved/nested/external/stdlib/cgo/unsupported candidate。resolved seeds 为空时，
两者必须同时为空。

### 6. 每个 node 的 deterministic shortest witness

每个 reachable node 必须且只能携带一个 witness。witness 的 `node_ids` 从 seed 到该 reachable node 排列；`edge_ids` 按相同
traversal 顺序排列，但每条原 edge 的方向是后一个 node `from` 指向前一个 node `to`。必须满足：

```text
len(node_ids) = hop_count + 1
len(edge_ids) = hop_count
node_ids[0]   = seed_node_id
node_ids[-1]  = reachable node_id
```

seed 自身 witness 固定为 hop 0、一个 node ID、空 edge IDs。其它 node 从全部 resolved seeds 做 multi-source BFS，先最小化
`hop_count`；同长度候选依次按 `seed_node_id`、完整 `edge_ids` sequence、完整 `node_ids` sequence 的原始 UTF-8 byte
lexicographic order 选唯一最小项。不得依赖 map iteration、filesystem order、caller path order或并发完成顺序。

### 7. Closure 与 system UNKNOWN

`package_lexical_closure_status` 只有：

- `complete_within_observation`：所有 changed paths 均 resolved，observation 无 diagnostic，且 dependency 中没有
  `ambiguous_local`、`unresolved_local`、`nested_module_boundary` 或 `unsupported`；
- `unknown`：其它任何情况。

`closure_reason_codes` 是从完整 request/observation 精确重建的排序唯一集合，只允许：

```text
ambiguous_local_dependency_present
changed_path_unresolved
go_file_diagnostic_present
nested_module_boundary_dependency_present
unresolved_local_dependency_present
unsupported_import_dependency_present
```

complete 状态要求空 reason set；unknown 要求非空且不得漏掉适用 reason。`external_candidate`、`stdlib_candidate` 与
`cgo_pseudo` 不加入 selected-module exact-local closure gap，但仍不授予任何跨 module 或 runtime 结论。

`system_impact_status` 永远是 `unknown`。`system_unknown_reason_codes` 永远是以下 canonical 顺序的完整数组：

```text
api_event_contract_surfaces_not_observed
call_and_runtime_reachability_not_observed
data_and_migration_surfaces_not_observed
deployment_and_operations_surfaces_not_observed
owner_adr_policy_surfaces_not_observed
selected_build_semantics_not_observed
```

零 reachable dependent、complete-within-observation 或 schema/digest valid 均不得改写为 `no_impact`、`safe`、`low_risk` 或 final。

### 8. Canonical bytes 与 domain-separated digest

除复用 ADR-0053 graph domain 外，新 digest domain 为：

```text
request  = forgeos.governance.local-go-package-impact-prescan-request.v1\0
node     = forgeos.governance.local-go-package-impact-prescan-node.v1\0
edge     = forgeos.governance.local-go-package-impact-prescan-edge.v1\0
report   = forgeos.governance.local-go-package-impact-prescan-report.v1\0
envelope = forgeos.governance.local-go-package-impact-prescan.v1\0
```

request/report/envelope self digest 分别在完整对象中仅把自身 digest 字段替换为空字符串，再 canonical encode 后加对应 domain
计算。report 必须逐字节绑定 request SHA、graph observation SHA 和 run ID；envelope 同时包含 final request/report，因而绑定完整输入与
输出。任何 graph bytes、changed path、node、edge、witness、status、reason、排序或摘要变化都必须改变相应上层 digest。

Canonical JSON 只接受 exact compact UTF-8、ASCII-snake-case byte-sorted keys、signed int64、无 float/duplicate/unknown field，
禁止 Unicode Cc/bidi/U+2028/U+2029/surrogate。所有标为 sorted 的数组按原始 UTF-8 bytes 严格升序；schema 的 `uniqueItems`
不是排序或 canonicality 的替代。

### 9. Bounds 与失败关闭

v1 上限为：graph observation decoded bytes 16 MiB、其 unpadded Base64URL 22,369,622 bytes、request 24 MiB、report 16 MiB、
envelope 48 MiB、changed paths/resolved seeds/unresolved seeds 各 256、package nodes 16,384、local edges 65,536、单 edge
source paths 16,384、单 witness 1,024 hops、aggregate witness hops 65,536、path 4,096 scalars/16 KiB、run ID 160 bytes、
JSON depth 16。整数只允许 signed int64。

超限、非法 UTF-8/Base64URL/JSON/path、graph contract/digest/run mismatch、输入未排序或重复、seed partition 不闭合、node/edge
identity 错误、closure 不完整、witness 不最短/不确定、status/reason 不一致、self digest 错误时，整份 evaluator 必须在返回任何 report
前失败。不得截断、跳过、部分发布、best-effort 修复或把资源耗尽转换成 complete/unknown 报告。

## Local-only 与 authority 边界

本合同 evaluator 是 deterministic pure local computation：只消费 caller 已提供的 bounded bytes，不读取 repository、Git、clock、
environment、credential、process、provider、network、database、Hub、journal 或 Memory，也不调用 ADR-0053 live producer。若调用方需要当前
repository observation，必须在本合同之外显式运行 ADR-0053，并把其 exact graph bytes 作为新 request 输入。

唯一正结果为：

```text
LOCAL_GO_PACKAGE_IMPACT_PRESCAN_ONLY (exact ADR-0053 lexical reverse dependency
closure; system impact unknown; no selected-build, truth, authority, completion,
persistence, execution, or effect attestation)
```

本合同不认证 Git/producer/principal，不确认事实，不创建 Evidence/Claim/Atom/GraphSnapshot/Context/Grant/Approval，不满足 hard gate，
不派生 Cost/Risk/materiality/role/gate/human approval/DAG，不写 SQLite/Knowledge，不执行工具或 effect，不产生 completion、truth、
authorization、permission 或 production decision。完整 GraphSnapshot、final ChangeImpactReport/Cost/Risk/AssessmentReceipt 与其工作流接线仍为
后续独立协议。

## 交付边界与验收条件

本 ADR、Schema、pure local Go runtime/`forge go-impact-prescan` bytes-only CLI、strict Python checker、exact fixture 与
scaffold/governance 接线已交付。Go API/CLI 只消费 caller 提供的 graph bytes/digest/run ID/changed paths；Python CLI 只检查 caller 提供的
bounded envelope file 或 golden wrapper。三者均不读取 repository 或隐式调用 ADR-0053 live producer，也不因此升级为完整 Impact Closure。交付测试证明：

- exact ADR-0053 rich fixture 可重建 diamond/multi-seed reverse closure；
- compile/test edge 保留、cycle 有界、多个同长 witness 使用固定 tie-break；
- deleted/absent/non-Go/nested/diagnostic path 不产生假 seed；
- 零 dependent 仍保持 system UNKNOWN；
- graph/request/node/edge/witness/status/digest 任一 tamper 失败关闭；
- limit 前一项成功、越界项零 report；
- 运行时无 repository/process/network/database/provider/credential 访问。

未交付 live-capture 组合命令、GraphSnapshot、final ChangeImpactReport/Cost/Risk/AssessmentReceipt、journal/SQLite/Knowledge 持久化或
任何 authority/effect wiring；它们仍须由后续独立合同实现。`forge accept` 只验证本地交付门禁，不改变上述非能力边界。
