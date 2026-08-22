# ADR-0065 — Authority-free GraphSnapshot v1 contract and ADR-0053 projector

- 状态：已采纳并交付（Schema、fixture、Go/Python pure projector、CLI、strict checker、registry、scaffold 与独立复审已关闭）
- 范围：Wave 2-C；通用 GraphSnapshot v1 core shape，以及 exact ADR-0053 observation 到恒为 PARTIAL/UNKNOWN 的纯投影 profile
- 关联：ADR-0037、ADR-0053、ADR-0062、
  `docs/contracts/graph-snapshot-v1.schema.json`

## 背景与切片边界

ADR-0053 只观察一个 selected Go module 的 all-regular-file lexical package/import surface；ADR-0062 只在这份 exact observation
内部计算 local package reverse closure。二者都不能表达可扩展 node/edge identity、extractor provenance、跨 surface coverage 或
snapshot freshness。目标态 System Knowledge Graph 又包含 API/event、DB/migration、deployment、ADR/owner、runtime 等来源，不能由一份 Go
observation 冒充。

本 ADR 冻结两层合同：第一层是可由后续 profile 复用的 `GraphSnapshot v1` node/edge/taxonomy/provenance/coverage shape；第二层是当前唯一
受支持的 `adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1` projector。该 projector 是 pure caller-bytes computation，必须把所有
未观察面、所有非 local dependency、diagnostic、nested module 与 freshness 缺口显式保存。它不交付多 surface extractor、完整 System
Knowledge Graph、ChangeImpactReport、Cost、Risk 或 Assessment Join。

## 1. Exact envelope、版本与 profile

最终 wire 顶层只能包含：

```text
api_version, canonicalization, envelope_sha256, request, snapshot
```

固定值：

```text
envelope.api_version = forgeos.governance.local-go-graph-snapshot-projection/v1
request.api_version  = forgeos.governance.local-go-graph-snapshot-projection-request/v1
snapshot.api_version = forgeos.governance.graph-snapshot/v1
canonicalization     = forgeos.canonical-json/v1
projector_profile_id = adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1
```

request 精确包含：

```text
api_version, canonicalization, graph_observation_base64url,
graph_observation_sha256, project_id, projector_profile_id,
request_sha256, run_id
```

`graph_observation_base64url` 是完整 exact compact ADR-0053 `graph_observation` bytes 的 RFC 4648 URL-safe、无 padding、最短 Base64；decoded
bytes、ADR-0053 domain digest 和 `producer.run_id` 必须分别等于 request 声明。`project_id` 使用 ADR-0045 identifier lexical grammar，最多
160 UTF-8 bytes，但只是 caller-declared project namespace，不认证 repository、tenant 或 principal。

v1 只支持上述一个 profile、ADR-0053 graph API v1 与其 exact profile。未知 envelope/request/snapshot version、input profile 或 projector
profile 必须返回 `unsupported_profile` 并在产生任何 snapshot/envelope 前失败；不得 fallback、猜 alias、按较新版本宽松读取或只靠 Schema
接受。`unsupported_profile` 是 API error class，不进入成功 wire。

snapshot 精确包含：

```text
adr_0062_node_crosswalk, canonicalization, coverage, coverage_sha256,
crosswalk_set_sha256, edge_set_sha256, edges, extractor_set_sha256,
extractors, freshness, node_set_sha256, nodes, profile_id, project_id,
request_sha256, result, snapshot_id, snapshot_identity_sha256,
snapshot_sha256, source_set_sha256, sources, system_knowledge_status,
system_unknown_reason_codes, unresolved_edge_set_sha256, unresolved_edges,
unresolved_node_set_sha256, unresolved_nodes, api_version
```

## 2. Canonical bytes、digest 与无自环 ID

所有 digest 都是 lowercase bare SHA-256。domain 后的 `\0` 是一个 NUL byte：

```text
request                 forgeos.governance.local-go-graph-snapshot-projection-request.v1\0
source identity         forgeos.governance.graph-snapshot-source-identity.v1\0
source record           forgeos.governance.graph-snapshot-source.v1\0
extractor identity      forgeos.governance.graph-snapshot-extractor-identity.v1\0
extractor record        forgeos.governance.graph-snapshot-extractor.v1\0
node identity           forgeos.governance.graph-snapshot-node-identity.v1\0
node record             forgeos.governance.graph-snapshot-node.v1\0
edge identity           forgeos.governance.graph-snapshot-edge-identity.v1\0
edge record             forgeos.governance.graph-snapshot-edge.v1\0
unresolved node identity forgeos.governance.graph-snapshot-unresolved-node-identity.v1\0
unresolved node record  forgeos.governance.graph-snapshot-unresolved-node.v1\0
unresolved edge identity forgeos.governance.graph-snapshot-unresolved-edge-identity.v1\0
unresolved edge record  forgeos.governance.graph-snapshot-unresolved-edge.v1\0
source/extractor/node/edge sets
                        forgeos.governance.graph-snapshot-{source|extractor|node|edge}-set.v1\0
unresolved sets         forgeos.governance.graph-snapshot-unresolved-{node|edge}-set.v1\0
ADR-0062 crosswalk set  forgeos.governance.graph-snapshot-adr-0062-node-crosswalk-set.v1\0
coverage                forgeos.governance.graph-snapshot-coverage.v1\0
snapshot identity       forgeos.governance.graph-snapshot-identity.v1\0
snapshot record         forgeos.governance.graph-snapshot.v1\0
envelope                forgeos.governance.local-go-graph-snapshot-projection.v1\0
```

request/envelope self digest 将自身 digest 字段替换为空字符串。每个 source/extractor/node/edge/unresolved record 先从不含其 ID/self digest 的
独立 identity projection 计算 `*_identity_sha256`，再形成最终 `*_id`，最后以最终 ID 和 identity digest 已填、仅 `*_sha256=""` 的完整 record
计算 self digest。snapshot 同理：identity projection 精确为
`{coverage_sha256,crosswalk_set_sha256,edge_set_sha256,extractor_set_sha256,node_set_sha256,profile_id,project_id,request_sha256,
source_set_sha256,unresolved_edge_set_sha256,unresolved_node_set_sha256}`；
`snapshot_id="graph-snapshot-" + snapshot_identity_sha256`；然后在 final ID 已填、仅 `snapshot_sha256=""` 时计算 snapshot digest。任何 ID
不得依赖其自己的 self digest，任何 self digest 都必须绑定 final ID。

其它 identity projection 与 ID prefix 也逐字段冻结：

```text
source = {graph_api_version,graph_observation_sha256,graph_profile_id,
          observer_run_id,source_revision,source_tree_sha256,source_type}
         -> graph-source-<source_identity_sha256>
extractor = {extractor_type,extractor_version,producer_id,projector_profile_id}
            -> graph-extractor-<extractor_identity_sha256>
node = {identity_namespace,identity_profile_id,node_type,project_id,
        qualified_name_components}
       -> graph-node-<node_identity_sha256>
edge = {category_axes,from_node_id,identity_profile_id,import_discriminator,
        parallel_discriminator,relation,source_role,to_node_id}
       -> graph-edge-<edge_identity_sha256>
unresolved node = {candidate_identity_namespace,candidate_identity_profile_id,
                   candidate_qualified_name_components,kind,project_id,reason_code}
                  -> graph-unresolved-node-<unresolved_node_identity_sha256>
unresolved edge = {category_axes,from_node_id,identity_profile_id,
                   import_discriminator,parallel_discriminator,project_id,
                   reason_code,relation,resolution,resolution_detail,source_role,
                   target_candidate}
                  -> graph-unresolved-edge-<unresolved_edge_identity_sha256>
```

request/source/extractor/node/edge/unresolved-node/unresolved-edge/snapshot/envelope 的 self-empty 字段分别只能是
`request_sha256/source_sha256/extractor_sha256/node_sha256/edge_sha256/unresolved_node_sha256/unresolved_edge_sha256/snapshot_sha256/
envelope_sha256`；identity digest 和 final ID 绝不置空。

每个 set digest 的 preimage 不是裸数组，而是 exact structured object：
`{item_count,items}`；coverage 使用 `{status,surface_count,surfaces}`。item 是完整 final record，数组按对应 ID raw UTF-8 bytes 严格排序。
source、extractor、node、edge、unresolved-node、unresolved-edge、crosswalk 各使用上列独立 domain，不能跨 set 重放。
crosswalk 没有另造 item ID，固定按 `graph_node_id` 排序；其它 set 的 sort key 分别是
`source_id/extractor_id/node_id/edge_id/unresolved_node_id/unresolved_edge_id`。

Canonical JSON 必须是 exact compact UTF-8、ASCII snake_case byte-sorted keys、signed int64；拒绝 float、bool-as-int、duplicate/unknown field、
invalid UTF-8、surrogate、Unicode Cc/bidi/U+2028/U+2029。标注排序的数组不得由 validator 静默排序、去重或 Unicode normalize。JSON Schema
的 `maxLength`/`uniqueItems` 只是近似；UTF-8 byte limit、canonical order、digest、derivation 与 collision 必须由 semantic checker 重验。

## 3. Caller-declared project-scoped semantic-name identity

node identity projection 精确包含：

```text
identity_namespace, identity_profile_id, node_type, project_id,
qualified_name_components
```

当前 profile 只生成：

- module：namespace `go`、profile `go-module-path-v1`、type `module`、components `[module_path]`；
- package：namespace `go`、profile `go-package-module-relative-directory-name-v1`、type `package`、components
  `[module_path,module_relative_directory,package_name]`。

`module_relative_directory` 的算法冻结为：若 package directory 等于 selected module directory，结果是 literal `.`；否则 package directory
必须是以 component boundary 位于 module directory 下的 descendant，并逐字节移除 `module_directory + "/"`。selected module 为 root `.` 时，
root package 仍是 `.`，descendant 保留其原 canonical directory；不得 path-clean、case-fold 或猜 symlink/rename。projector 生成一个 module node 和
ADR-0053 每个 package 的一个 package node，包括 test-only package。

该 ID 只能称为 **caller-declared project-scoped semantic-name-stable**：文件 content、revision 或同一 package 内文件 rename 不改变 ID；
`project_id`、module path、module-relative directory 或 package name 变化会形成 delete+add。它不是全局实体身份、authenticated identity 或 rename
lineage。node ID 为 `graph-node-<node_identity_sha256>`。

所有 identity projections、final IDs 和 union sets 必须全局唯一。hash collision、相同 ID 对应不同 identity、module/package union collision、
edge union collision、悬挂 endpoint 或同一 upstream item 映射多次都使整份投影失败；不得保留任一 partial set。

### ADR-0062 deterministic crosswalk

每个 package node 必须恰好有一个 crosswalk：
`{adr_0062_node_id,adr_0062_node_sha256,graph_node_id}`，按 `graph_node_id` 排序。ADR-0062 digest 仍按其原 domain 和
`{directory,import_path,module_path,package_name}` projection 重算。crosswalk 是确定性对应，不表示两种 ID、domain 或语义等价；module node 没有
ADR-0062 counterpart，任一 ID 都不得代替另一方作 endpoint。

## 4. Node/edge base shape 与 taxonomy

通用 node record 从 v1 起就包含 stable identity、`source_locators`、source/extractor references，以及
`owner_node_ids, owner_status, lifecycle_status, data_classification, claim_record_ids, evidence_record_ids, validity_status, freshness_status,
provenance_status`。当前 authority-free graph-only profile 对后三组认识字段固定为 `[]` 或 `unknown`；显式 source/extractor reference 只支持
可重建性，不把 provenance 升级为认证事实。

node type 是以下 31 项的闭集：

```text
actor, adr, aggregate, api, bounded_context, business_capability, business_rule,
column, debt, deployment_unit, domain_event, entity, environment, event_contract,
gate, incident, job, journey, module, owner, package, policy, queue, requirement,
runtime_signal, schema, symbol, table, test, use_case, value_object
```

集合语义为（上述 wire enum 仍按 raw bytes canonicalize）：business=`actor,business_capability,business_rule,journey,requirement`；
domain=`aggregate,bounded_context,domain_event,entity,use_case,value_object`；contract=`api,event_contract`；data=`column,queue,schema,table`；
code=`job,module,package,symbol`；delivery=`deployment_unit,environment`；governance=`adr,debt,gate,owner,policy`；
verification/runtime=`incident,runtime_signal,test`；`any` 是 31 项完整 union。

edge identity projection 精确包含：

```text
category_axes, from_node_id, identity_profile_id, import_discriminator,
parallel_discriminator, relation, source_role, to_node_id
```

它刻意排除 locator、source revision/tree、extractor record 和 evidence/claim refs，使同一语义 edge 在 source byte 或 revision 变化时保持 ID；
full edge self digest 仍绑定这些变化。`source_role` 只能是 nullable `compile|test`；`import_discriminator` nullable。当前 contains edge 固定两者 null、
`parallel_discriminator="contains"`；Go dependency edge 固定 `source_role` 与 exact import path，并令
`parallel_discriminator=source_role + ":" + import_path`（ADR-0053 canonical import path 禁止 colon）。edge ID 为
`graph-edge-<edge_identity_sha256>`。

`category_axes` 是排序唯一非空数组，闭集为
`data, ownership, policy, runtime, static_source, structural, verification`。它不是单一“truth category”。20 个 relation 的方向、endpoint family 与
允许 axes 冻结如下；`same_type` 要求两端 node_type 相同，`subject` 是 any，`executable`=`api,job,module,package,symbol,use_case`，
`container`=`bounded_context,business_capability,deployment_unit,environment,module,schema`：

| relation | direction / endpoints | allowed axes |
|---|---|---|
| owns | `actor|owner -> subject` | ownership |
| contains | `container -> subject` | structural |
| realizes | `domain|code|contract -> business|domain` | structural, static_source |
| implements | `code|contract -> requirement|business_rule|use_case|api|event_contract` | structural, static_source |
| exposes | `module|package|deployment_unit -> api|event_contract` | structural, static_source |
| calls | `executable -> api|job|module|package|symbol|use_case` | runtime, static_source |
| depends_on | `subject -> subject` | data, policy, runtime, static_source, structural, verification |
| reads | `executable -> column|queue|runtime_signal|schema|table` | data, runtime, static_source |
| writes | `executable -> column|queue|runtime_signal|schema|table` | data, runtime, static_source |
| persists_to | `aggregate|entity|executable -> queue|schema|table` | data, runtime, static_source |
| publishes | `domain_event|event_contract|executable -> event_contract|queue` | runtime, static_source |
| consumes | `executable -> api|event_contract|queue` | runtime, static_source |
| deployed_as | `api|job|module|package -> deployment_unit` | structural |
| constrained_by | `subject -> business_rule|gate|policy|requirement` | policy |
| verified_by | `subject -> gate|runtime_signal|test` | verification |
| observed_by | `subject -> runtime_signal|test` | runtime, verification |
| governed_by | `subject -> adr|policy` | policy |
| decided_by | `subject -> adr` | policy |
| supersedes | `same_type -> same_type` | structural, policy |
| affects | `subject -> subject` | data, ownership, policy, runtime, static_source, structural, verification |

Future profiles must choose a nonempty subset of the relation row, preserve direction, and define an exact parallel discriminator. `epistemic_status` 的闭集为
`assumed|confirmed|derived`，但当前 profile 只能输出 `derived`；它没有 authority 将 observation 提升为 confirmed，也不接受 caller-supplied assumed
edge。当前 projector 只生成 module→package `contains` with `[structural]`，以及每个 ADR-0053 `resolution=local` 的 package→package
`depends_on` with `[static_source]`；compile/test parallel edges 保持分离。

## 5. Distinct observer/projector provenance 与 locators

`sources` 当前恰好一项 `adr_0053_graph_observation` record，分别绑定 graph digest、source revision/tree、observed time、upstream
`producer_id/type/version/run_id/parameters_sha256`。这是 upstream observer declaration。`extractors` 当前恰好一项独立
`forgeos.local-go-graph-snapshot-projector` / `tool` / `v1` record，绑定 input source ID/API/profile 和 projector profile。这是 projector
declaration。两者不得合并、互相冒充或被称为 authenticated principal/tool identity。

source locator 精确为 `{content_sha256,path,role,source_id}`。module node 使用 go.mod；package node 使用其全部 compile/test files；contains edge
复用 target package locators；dependency edge 使用 exact `source_paths` 并从 ADR-0053 files 恢复相同 role/content digest。locator 不参与 stable
node/edge identity，却参与 full record/set/snapshot digest。所有 locator 按 `(role,path,content_sha256)` 排序且唯一：字符串按 raw UTF-8 bytes，
nullable digest 的 null rank 在字符串之前。

## 6. Exact unresolved 双射

每个 ADR-0053 diagnostic 必须产生一个 `go_file_diagnostic` unresolved node，每个 nested-module boundary 必须产生一个
`nested_module_boundary` unresolved node；两集合合并后按 ID 排序且双射。nonregular excluded entries 只有 aggregate count，没有 locator，不能伪造
unresolved node。unresolved-node identity 绑定 project、kind、candidate namespace/profile/components 与 reason；full record 再绑定 locator、diagnostic
code、source/extractor refs。

diagnostic candidate 固定 namespace/profile 为 `go_source_path/go-source-path-v1`、components
`[module_path,module_relative_file_path]`；boundary 固定为 `go_module_boundary/go-module-boundary-v1`、components
`[module_path,module_relative_directory]`。relative path/directory 使用第 3 节相同的 literal `.` 与 component-boundary strip，不能 clean 或猜 rename。
diagnostic locator 的 role 仅由 path literal suffix 决定：以 `_test.go` 结尾时为 `test`，否则为 `compile`；其 content digest 为 null
（graph-only input 没有失败文件 preimage）。boundary go.mod locator role 固定为 `go_mod`，content digest 同样为 null。

每个且仅每个非 `local` dependency 必须产生一个 unresolved edge，保留 from node、role/import、resolution/detail、candidate target、所有
source locators 和 exact closed reason；`ambiguous_local` 的 candidate target IDs 是 target directory 的全部 compile package IDs，其余 resolution
不得伪造 target node。映射为：

```text
ambiguous_local       -> multiple_compile_packages
unresolved_local      -> no_compile_package
nested_module_boundary-> nested_module_boundary
stdlib_candidate      -> stdlib_candidate_not_resolved
external_candidate    -> external_candidate_not_resolved
cgo_pseudo            -> cgo_pseudo_not_resolved
unsupported           -> noncanonical_import_path
```

`ambiguous_local|unresolved_local|nested_module_boundary` target 固定 namespace/profile 为
`go/go-package-directory-candidate-v1`、components `[module_path,module_relative_target_directory]`；只有 ambiguous 的 `target_node_ids` 包含该
directory 全部 compile-bearing package node IDs，按 ID 排序。`stdlib_candidate|external_candidate|cgo_pseudo|unsupported` target 固定为
`go_import_candidate/go-import-candidate-v1`、components `[import_path]`、空 target IDs。unresolved identity 包含 upstream resolution/detail 和上述
完整 target candidate，因而不同 candidate 不得折叠。

unresolved edge 使用自己的 identity/record/set domains，不能进入 resolved edge set。少一项、多一项、重复映射、错误 candidate 或把
stdlib/external/cgo 静默当 complete 都使整份失败。

## 7. Coverage、freshness 与恒定 UNKNOWN

coverage 必须按 raw UTF-8 bytes 精确包含 11 个 surface：

```text
adr_decision, api_event_contract, business_domain, data_schema_migration,
deployment_environment, go_module_package_lexical, operations_runtime_signal,
other_language_module_package, owner_policy, symbol_call_runtime,
test_verification
```

每项包含 `edge_count,node_count,reason_codes,status,surface`。Go surface 恒为 `partial`，计数与 node/resolved/unresolved sets 精确对应，reason set
至少包含 single selected module、lexical union not selected build、module graph absent、compile/test/runtime reachability absent、bounded interval not
atomic，并从 diagnostics/nested/nonregular/每类 unresolved resolution 确定性增加原因。其它十面恒为 `not_observed`、zero counts 和各自唯一
`<surface>_surface_not_observed` reason。overall coverage 恒为 `partial`。

这里的 exact count 是 `go.node_count=len(nodes)`、`go.edge_count=len(edges)`；unresolved counts 由各自 set 的 `item_count` 绑定，不能混进已解析
edge count。Go surface base reason 的排序唯一集合为：

```text
all_regular_go_files_lexical_union_not_selected_build
compile_test_runtime_reachability_not_observed
go_module_graph_not_resolved
single_selected_go_module_only
source_observation_not_atomic_snapshot
```

再按条件加入 `ambiguous_local_dependency_present,cgo_pseudo_dependency_present,external_candidate_dependency_present,
go_file_diagnostic_present,nested_module_boundary_dependency_present,nested_module_boundary_present,nonregular_go_entries_not_located,
stdlib_candidate_dependency_present,unresolved_local_dependency_present,unsupported_import_dependency_present`，最终按 raw UTF-8 bytes 重排。其它
surface reason 逐字面为：

```text
adr_decision_surface_not_observed
api_event_contract_surface_not_observed
business_domain_surface_not_observed
data_schema_migration_surface_not_observed
deployment_environment_surface_not_observed
operations_runtime_signal_surface_not_observed
other_language_module_package_surface_not_observed
owner_policy_surface_not_observed
symbol_call_runtime_surface_not_observed
test_verification_surface_not_observed
```

freshness 精确复制 ADR-0053 `observed_at_unix_ms`，令 `expires_at_unix_ms` 与之相等，固定 `status=unknown` 及排序 reasons：

```text
source_observation_clock_unauthenticated
source_observation_not_atomic_snapshot
zero_duration_expiry_no_freshness_attestation
```

projector 不读取 clock，也不接受 caller TTL。外部 consumer 可在另一版本化合同中把它降为 stale，不能原地改成 fresh。node/edge 的
freshness/validity/provenance 同样固定 unknown。

`system_knowledge_status=unknown`；system reasons 永远完整包含 ADR/owner/policy、API/event、business/domain、call/runtime、data/migration、
deployment/operations、other-language module/package、selected-build、test/verification 与 freshness 十类缺口。零 diagnostic、零 unresolved edge、
valid digest 或所有 local edge 都不能产生 complete、no-impact、safe、low-risk 或 final assessment。

唯一正结果为：

```text
PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical
module/package subgraph only; coverage partial and system/freshness unknown;
no selected-build, cross-surface completeness, truth, authority, completion,
persistence, execution, impact, or effect attestation)
```

## 8. Bounds 与 failure semantics

decoded graph 16 MiB、unpadded Base64URL 22,369,622 bytes、request 24 MiB、snapshot 64 MiB、envelope 96 MiB、depth 16；nodes
16,385（one module + 16,384 packages）；ADR-0053 dependency edges 65,536，contains edges 16,384，resolved edge union 81,920；unresolved nodes
17,408（16,384 diagnostics + 1,024 boundaries）；unresolved edges 65,536；crosswalk 16,384；sources/extractors 恰好 one；coverage 恰好 11；
per node/edge locators 16,384，aggregate locators 131,072；path 4,096 scalars/16 KiB；project/run ID 160 bytes。

81,920 是 resolved edge union 的专用上限；resolved dependency 与 unresolved dependency 共享 ADR-0053 的 65,536 total candidate 上限，不能各自
消费 65,536 后仍被接受。通用 JSON array walker 的 65,536 默认不得误拒合法 `16,384 contains + 65,536 local dependency` 边界，也不得被提高后
套用到其它数组。每字段及共享 aggregate 必须先执行其专用 limit。超限、资源耗尽、非法 bytes/profile、digest/ID/set/count/order/crosswalk/
endpoint/coverage/reason/freshness drift 或 collision 时返回零 output；不得截断、skip、best effort 修补或输出 partial envelope。

## Authority 与 graph-only 非能力边界

projector 只消费 caller bytes，不读取 repository、source manifest、Git、clock、environment、credential、process、provider、network、database、Hub、
journal 或 Memory，也不调用 ADR-0053 live producer。graph-only input 无法重验 parameters/source manifest preimage、Git binary、current tree、producer
identity 或 source freshness；distinct provenance records仍只是被 exact graph 约束的声明。

该结果不创建 Evidence/Claim/Context/Grant/Approval/receipt，不 append journal、不写 SQLite/Knowledge、不产生 truth、authority、permission、
completion、persistence、execution 或 effect。它不是 ChangeImpactReport/Cost/Risk/AssessmentReceipt，不能满足 G3 或 Assessment Join，不派生
materiality、roles、gates、human approval 或 DAG。

## Schema、跨语言与 acceptance

Schema 冻结 wire shape、closed vocabulary、近似 cardinality 和 `x-forgeos-*` 语义；`$comment` 明确 Schema alone 不足。兼容实现必须另用 semantic
checker 从 exact ADR-0053 bytes 重建全部 request/source/extractor/node/edge/unresolved/crosswalk/coverage/freshness/digest 后逐字节比较。Python 与 Go
必须共享 raw UTF-8 byte order、canonical JSON、signed-int64、path/root 算法和每字段 byte bounds；host Unicode locale、map iteration 或 JSON
library defaults 不能改变输出。

本切片已交付 fixture、semantic checker、Go/Python pure projector、显式输入 CLI、registry、Skill、universal scaffold/legacy upgrade 与
adversarial/fresh review wiring；scaffold 不安装 Catalyst-only Go host。Rust 仍未交付，且不得用 Rust 缺实现冒充 cross-runtime ABI。
roadmap 只关闭 stable identity/taxonomy/extractor provenance/GraphSnapshot v1 基础项；多 surface extractor、真实 coverage/staleness、完整 traversal、
Impact/Cost/Risk 与 Assessment Join 继续未完成。

## 被拒方案与重审触发器

拒绝把 source revision/tree 放进 node/edge stable ID（每次改动都会漂移）、用 module path 代替 project namespace（fork collision）、把 locator
放进 edge identity（文件 rename 破坏语义 ID）、把 local graph 称 complete、遗漏 nonlocal candidate、由 caller 填 confirmed/fresh、用 embedding
生成事实、或以 schema-only validation 代替 exact projector reconstruction。

若需要新 extractor/profile、selected-build/module-graph semantics、authenticated project/tool/clock、rename lineage、Evidence admission、durable graph
store、incremental refresh、跨 surface impact traversal 或 Assessment authority，必须新增版本化合同并重新冻结 compatibility、coverage 与 authority。
