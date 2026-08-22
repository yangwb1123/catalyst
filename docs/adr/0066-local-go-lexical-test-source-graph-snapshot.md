# ADR-0066 — Local Go lexical test-source GraphSnapshot profile v1

- 状态：已采纳并交付（Schema、fixture、Go/Python pure projector、CLI、strict checker、registry、Skill、scaffold 与窄 roadmap closure 已关闭）
- 范围：Wave 2-C；从 exact ADR-0053 caller bytes 纯投影 module/package/test-source lexical partial GraphSnapshot
- 关联：ADR-0053、ADR-0062、ADR-0065、
  `docs/contracts/graph-snapshot-go-test-source-v1.schema.json`

## 背景与切片边界

ADR-0065 的首个 profile 只生成 module/package nodes。ADR-0053 已经把成功解析文件按 exact `(directory,package_name)` 分组，并将
`compile_files` 与 `test_files` 分离，所以后续 pure projector 可以在不读取 repository、不重新解析 Go、也不运行测试的前提下，诚实补充
“某 package 分组有这组 lexical `_test.go` source files”。这仍不知道 test function、suite、case、fixture、build selection、compile、execution、
PASS/FAIL、coverage、flakiness 或被测对象。

本 ADR 冻结新 profile
`adr-0053-selected-go-module-lexical-package-test-source-partial-graph-snapshot-v1`。它完整保留 ADR-0065 首 profile 的 module/package、resolved/
unresolved dependency、diagnostic/boundary 与 ADR-0062 crosswalk 投影，再为每个 `test_files` 非空的 package 增加一个 `test` node 和一条
module→test structural `contains` edge。ADR-0065 的旧 profile、Schema、fixture、API 与 golden bytes 均不改变；本切片通过独立 registry entry、
Skill 与 scaffold 接线，不扩大旧 profile 的 wire 或语义。

## 1. 独立 transport 与 exact input negotiation

wire 顶层仍精确为 `api_version,canonicalization,envelope_sha256,request,snapshot`，但 transport 使用新 API：

```text
envelope.api_version = forgeos.governance.local-go-test-source-graph-snapshot-projection/v1
request.api_version  = forgeos.governance.local-go-test-source-graph-snapshot-projection-request/v1
snapshot.api_version = forgeos.governance.graph-snapshot/v1
canonicalization     = forgeos.canonical-json/v1
projector_profile_id = adr-0053-selected-go-module-lexical-package-test-source-partial-graph-snapshot-v1
```

选择独立 request/envelope API 是显式 profile negotiation：ADR-0065 transport v1 和已交付 checker 对未知 profile 必须失败关闭；在同一 strict
single-profile Schema 下加入第二个 const 会把旧 API 变成未声明的 tagged union，并可能让旧实现错误 fallback。新 endpoint 令 client 必须主动选择
test-source semantics；任一 endpoint 都不得猜 alias、协商“最近版本”或把另一个 profile 当 fallback。

request 精确包含：

```text
api_version, canonicalization, graph_observation_base64url,
graph_observation_sha256, project_id, projector_profile_id,
request_sha256, run_id
```

input 是完整 exact compact ADR-0053 `graph_observation` caller bytes 的 RFC 4648 URL-safe、无 padding、最短 Base64，而不是 production、source
manifest 或 live repository。decoded bytes 必须通过 ADR-0053 strict validation；domain digest、graph API/profile、`producer.run_id` 必须分别等于
request 声明。只支持 `forgeos.go-package-dependency-graph-observation/v1` /
`selected-go-module-lexical-dependency-graph-v1`。未知/错配 version/profile 必须在任何成功 wire 产生前返回 API error class
`unsupported_profile`；error 不进入 envelope。

`project_id` 与 `run_id` 使用 ADR-0045 identifier grammar，均最多 160 UTF-8 bytes。project 是 caller-declared namespace，不认证 repository、
tenant、principal 或 fork lineage。projector 只消费 caller bytes；不得读取 filesystem、Git、clock、environment、process、provider、network、DB、Hub、
journal 或 Memory，也不得自动调用 ADR-0053 live producer。

## 2. Canonical bytes、digest 与 profile binding

继续逐字节复用 ADR-0065 的 canonical JSON、signed-int64、forbidden Unicode、raw UTF-8 ordering、independent identity projection、final ID、
self-empty-field、structured set digest、snapshot identity 与 collision rules。record/set/snapshot domains 不变；只有新 transport 使用独立 domains：

```text
request  forgeos.governance.local-go-test-source-graph-snapshot-projection-request.v1\0
envelope forgeos.governance.local-go-test-source-graph-snapshot-projection.v1\0
```

request/envelope 只在自身 `*_sha256` 置空时求 self digest。source/extractor/node/edge/unresolved/snapshot 的 identity、ID prefix、self digest 与 set
preimage 精确按 ADR-0065；snapshot identity 仍包含 `profile_id` 与 `request_sha256`。兼容 checker 必须从 exact input 唯一重建完整 canonical
envelope，再逐字节比较，不能只重算顶层 hash、静默排序/去重/normalize 或信任 Schema。

同一 `project_id` 与 exact ADR-0053 input 下，两个 profiles 的 source identity/record 完全相同。module/package node、既有 resolved edge、unresolved
node/edge 的 identity projections 不含 projector profile，因此其 IDs 必须跨 profile 相同；ADR-0062 crosswalk items 也相同。新 extractor identity
绑定新 `projector_profile_id`，而所有 node/edge/unresolved full records 都绑定其 `extractor_sha256s`，所以这些稳定 ID 对应的 full record SHA 必须
profile-bound 且与旧 profile 不同。包含 profile-bound records 的非空 node/edge/unresolved set digest 必须随之改变；空 set 继续使用同一
domain 与 canonical `{item_count:0,items:[]}` preimage，因而精确保持相同 digest。request、snapshot 与 envelope digests 必须随 profile 改变。
不得以跨 profile stable ID 冒充
record bytes 相等，也不得把不同 profile record SHA 当 collision。

## 3. Exact module/package/test-source topology

module/package nodes、module→package `contains` edges、local `depends_on` edges、全部 nonlocal unresolved edges、diagnostic/boundary unresolved nodes 和
package-only ADR-0062 crosswalk 逐项复用 ADR-0065 的 exact derivation。test-role dependency 仍从 ADR-0053 的 package node 指向 local package node；
本 profile 不重写 endpoint，因此既有 dependency edge IDs 稳定。

对 ADR-0053 `packages` 中每个且仅每个 `test_files` 非空 item，生成一个 test node：

```text
identity_namespace = go
identity_profile_id = go-test-source-set-module-relative-directory-package-name-v1
node_type = test
qualified_name_components = [module_path,module_relative_directory,package_name]
```

`module_relative_directory` 精确使用 ADR-0065 literal `.` / component-boundary strip 算法；不得 clean、case-fold、resolve symlink 或猜 rename。
test node 与 package item 必须双射，`source_locators` 恰为该 item 的全部 `test_files`，每项 role=`test`、content digest 从 ADR-0053 exact file
record恢复，并按 locator key 排序唯一。它不包含 compile files。package `p` 与 external-test package `p_test` 是两个 exact package items，package
name 位于 identity components，因而必须形成不同 test IDs；不得按 directory 聚合、去掉 `_test` 或把 test-only package归并到 compile package。

每个 test node 恰有一条 module→test edge：

```text
relation=contains, category_axes=[structural], epistemic_status=derived
identity_profile_id=graph-edge-semantic-endpoints-v1
source_role=null, import_discriminator=null, parallel_discriminator=contains
```

edge locators 精确复用 target test node locators。新 profile 不生成 package→test、test→package、`verified_by`、`observed_by` 或其它 relation；不得从
filename/package/import 推断被测对象。test node 只是 lexical source-set node，不是 test case、execution、result 或 verification evidence。

ADR-0053 diagnostic 只有 `{code,path}`，没有成功 package clause。即使 path 以 `_test.go` 结尾，也只按 ADR-0065 生成 role=`test` 的
`go_file_diagnostic` unresolved node；不得猜 package、生成 test node、追加 contains edge或把失败文件 locator 塞进某个成功 source set。
nonregular exclusions同样只有 aggregate count，不能伪造 locator/node。nested boundaries 与每个非local dependency 继续保持 exact 双射。

## 4. Provenance、taxonomy 与 knowledge fields

sources 恰好一项 ADR-0053 observer declaration；extractors 恰好一项
`forgeos.local-go-graph-snapshot-projector/tool/v1` declaration并绑定新 profile。两者均不认证 tool/principal。node/edge source IDs 与 extractor
SHA 恰好各一；所有 locator、record、set 与 endpoint 必须 exact、无悬挂、全局 identity/ID 唯一。

本 profile 只输出 node types `module|package|test` 和 relations `contains|depends_on`，均是 ADR-0065 31-node/20-relation taxonomy 的闭子集。
contains axes 只能 `[structural]`；dependency axes 只能 `[static_source]`；所有 epistemic status 固定 `derived`。owner/claim/evidence arrays 固定空，
owner/lifecycle/data-classification/validity/freshness/provenance 固定 `unknown`。caller 不能注入 assumed/confirmed、authority、Evidence 或 ownership。

## 5. Disjoint coverage 与恒定 UNKNOWN

coverage 仍精确含 11 个 raw-byte-sorted surfaces。`go_module_package_lexical` 与 `test_verification` 均固定 `partial`；其余九面固定
`not_observed`、zero counts 与唯一 `<surface>_surface_not_observed` reason。resolved node/edge 采用互斥 partition：

```text
Go nodes   = module + package nodes
Test nodes = test nodes
Go edges   = module->package contains + source_role=compile resolved dependencies
Test edges = module->test contains + source_role=test resolved dependencies
```

因此两个 surface 的 node counts 相加必须等于 `len(nodes)`，edge counts 相加必须等于 `len(edges)`，任一 record 不得被重复计数或漏计。
unresolved sets 不混入 resolved counts；按 diagnostic suffix/edge source role 将已知缺口加入对应 surface reasons。nested/nonregular aggregate 无法可靠
恢复 package 或 role时，两个 partial surface 均保守报告缺口，但这不改变互斥 record counts。

Go base reasons 固定为排序集合：

```text
all_regular_go_files_lexical_union_not_selected_build
compile_runtime_reachability_not_observed
go_module_graph_not_resolved
single_selected_go_module_only
source_observation_not_atomic_snapshot
```

Test base reasons 固定为排序集合：

```text
go_test_files_lexical_source_set_only
selected_test_build_not_observed
source_observation_not_atomic_snapshot
test_case_identity_not_observed
test_execution_not_observed
test_outcome_and_coverage_not_observed
```

compile/test role 的 diagnostic 和七类 nonlocal resolution 分别只向对应 surface 追加下列 exact reason；boundary/nonregular aggregate 无法恢复
role，因此最后两项保守追加到两个 surface：

```text
ambiguous_local_dependency_present
cgo_pseudo_dependency_present
external_candidate_dependency_present
go_file_diagnostic_present
nested_module_boundary_dependency_present
nested_module_boundary_present
nonregular_go_entries_not_located
stdlib_candidate_dependency_present
unresolved_local_dependency_present
unsupported_import_dependency_present
```

dependency reason 按 edge `role` 分区；diagnostic reason 按 path literal `_test.go` suffix 分区，不能解析或猜 package。reason 最终按 raw UTF-8
bytes排序唯一，不能因 zero test node 将 test surface 升为 complete/not_observed。

freshness 与 ADR-0065 相同：复制 untrusted observed time、zero-duration expiry、`status=unknown` 和三项固定 reason。system status 恒为
`unknown`；完整 reasons 将旧 `test_and_verification_surfaces_not_observed` 替换为
`test_execution_and_verification_outcomes_not_observed`，其余九类缺口不变。lexical test source presence、全 local imports 或 zero diagnostics 都不能产生
fresh、complete、safe、no-impact、PASS 或 verification conclusion。

唯一正结果为：

```text
PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical
module/package/test-source subgraph only; test nodes are source sets, not tests
or outcomes; coverage partial and system/freshness unknown; no selected-build,
cross-surface completeness, truth, authority, completion, persistence,
execution, verification, impact, or effect attestation)
```

## 6. Bounds 与 atomic failure

decoded graph 16 MiB、Base64URL 22,369,622 bytes、request 24 MiB、snapshot 64 MiB、envelope 96 MiB、depth 16。nodes 上限
32,769=`1 module + 16,384 packages + 16,384 tests`；resolved edge union 上限
98,304=`16,384 package contains + 16,384 test contains + 65,536 resolved dependencies`；aggregate locators 上限 132,097。每 record locators
16,384；unresolved nodes 17,408、unresolved edges 65,536、crosswalk 16,384、sources/extractors各一、coverage surfaces 11，其它 ADR-0065/
ADR-0053 per-field 与 shared dependency-candidate bounds不变。

98,304 edge union 必须先使用专用 walker bound；dependency resolved+unresolved 仍共享 65,536 candidate/occurrence limit，不能各拿一份预算。
32,769/98,304/132,097 是独立上限，不保证三者可在 envelope byte cap 下同时饱和。任何 byte/depth/count/locator/profile/canonical/digest/
identity/order/bijection/coverage drift、collision、resource exhaustion 或 unsupported profile 均产生零 output；不得 truncate、skip、best effort 或返回
“部分成功”的 envelope。

## 7. Authority 与 delivery

该投影不解析 Go source，不确认 `_test.go` 内存在测试，不运行 `go test`，不生成 `verified_by/observed_by`，不签发 PASS/FAIL/coverage、Evidence、
Claim、Context、Grant、Approval、receipt 或 persistence fact；不满足 G3/Assessment Join，也不是 ChangeImpactReport/Cost/Risk/AssessmentReceipt。
source revision/tree、observer/projector/clock/project仍是 caller-bound declarations，不是 authenticated truth。

本 ADR 已交付 strict closed Schema、fixture、semantic checker、Go/Python pure runtime、显式 profile CLI、registry v21、Skill、universal
fresh/legacy scaffold 与 adversarial executable tests。roadmap 只关闭本 lexical test-source 窄项；Wave 2 的多 surface extractor 总项继续未勾。
Rust implementation 仍未交付，不能声称 Rust cross-runtime ABI；完整 System Knowledge Graph、test execution/result/coverage/verification 与 authority
仍不在本切片内。

## 8. 22 项对抗验收矩阵

| # | 必须失败关闭或保持的性质 | 对抗样例 |
|---:|---|---|
| 1 | transport/profile 显式协商 | 用 ADR-0065 endpoint/profile 或未知 alias 调新 projector |
| 2 | exact ADR-0053 caller bytes | padded/non-minimal Base64、digest/run/profile错配 |
| 3 | canonical closed JSON | duplicate/unknown key、float、bool-as-int、非法 Unicode/order |
| 4 | no ambient observation | projector 结果依赖 repo、clock、env、process、network 或 DB |
| 5 | one test node per nonempty test_files package | 少建、多建、为 empty test_files 建 node |
| 6 | package/test bijection | 一个 package 映射多个 test nodes 或多个 package 被合并 |
| 7 | p 与 p_test 分离 | 去掉 `_test`、按 directory 合并或复用同一 test ID |
| 8 | exact test identity | 错 module-relative `.`、clean/case-fold path、漏 package_name |
| 9 | exact lexical source set | test node 混入 compile file、漏/重/伪造 locator 或 digest |
| 10 | diagnostic 不猜 package | 为失败 `_test.go` 建 test node/contains 或绑定成功 package |
| 11 | exact module→test edge | endpoint/direction/relation/axes/discriminator/source_role 漂移 |
| 12 | no inferred verification edges | 生成 package→test、verified_by、observed_by 或被测对象 edge |
| 13 | no execution/outcome claim | 从 filename/import 推出 test case、PASS/FAIL/coverage |
| 14 | legacy topology unchanged | 重写 test-role dependency endpoint或改变旧 edge/node ID |
| 15 | cross-profile stable IDs | 同 project/input 的 module/package/legacy/unresolved ID 漂移 |
| 16 | profile-bound full records | 复用旧 extractor SHA 或错误要求 stable ID 的 record SHA相等 |
| 17 | global collision/endpoint safety | duplicate identity、cross-kind ID collision、dangling endpoint |
| 18 | unresolved exact bijections | diagnostic/boundary/nonlocal dependency 少、多、折叠或静默 resolved |
| 19 | ADR-0062 package-only crosswalk | 给 module/test造 crosswalk 或 package crosswalk不再一一对应 |
| 20 | disjoint Go/test coverage | record double计数、test-role edge计入 Go、两面和不等于 union |
| 21 | UNKNOWN semantics | test source存在使 freshness/system变 fresh/complete/safe/PASS |
| 22 | dedicated aggregate bounds | nodes 32,770、edges 98,305、locators 132,098 或 truncate接受 |

## 被拒方案与重审触发器

拒绝修改 ADR-0065 v1 const 形成隐式 multi-profile transport、把每个文件当 test case、用 basename作 identity、把 `p_test` 合并到 `p`、从
diagnostic 猜 package、把 source presence 称 verification、生成 verified_by/observed_by、双计 coverage、复用 profile-bound record SHA、只靠 Schema
验 digest，或因实现尚缺而勾 roadmap。

若需要 test declaration/case parser、selected build、`go test` execution/result/coverage、package-to-test ownership、verified-by semantics、认证
producer/clock/source、durable graph store、Rust ABI、impact traversal 或 authority admission，必须新增版本化合同并重新冻结 identity、coverage、
provenance、limits 与 failure semantics。
