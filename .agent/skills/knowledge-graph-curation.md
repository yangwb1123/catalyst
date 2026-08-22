# Knowledge Graph Curation

## 职责与触发

当任务需要从一份已经冻结的 source observation 构造可重建图、检查 stable identity、显式记录 unresolved/staleness/coverage，或评估图是否足以供后续 impact 使用时，采用本 Skill。ADR-0065 与 ADR-0066 当前交付 `forgeos.governance.graph-snapshot/v1` 的两个显式协商窄 profile：基础 profile 把 caller 提供的 exact ADR-0053 selected Go module lexical graph bytes 纯投影为 module/package 子图；独立 test-source profile 在保持旧 profile API、Schema、fixture 与 golden 不变的前提下，增加 package-scoped lexical test source-set nodes。

两个 profile 都恒为 PARTIAL，system knowledge 与 freshness 恒为 UNKNOWN。test-source node 只是 `_test.go` lexical source set，不是 test case、execution、result、coverage 或 verified subject。两者都不是 live extractor、事实库、完整 System Knowledge Graph、ChangeImpactReport、Cost、Risk、AssessmentReceipt、G3 或 Assessment Join。

## 输入契约

- exact compact canonical ADR-0053 `forgeos.go-package-dependency-graph-observation/v1` bytes、原 domain digest 与 producer run ID；
- caller-declared `project_id`；它只提供 project-scoped namespace，不认证 repository、tenant 或 principal；
- 显式选择 `adr-0053-selected-go-module-lexical-partial-graph-snapshot-v1` 或独立 transport 的 `adr-0053-selected-go-module-lexical-package-test-source-partial-graph-snapshot-v1`；不得 alias、fallback 或隐式升级；
- 所有输入必须满足 byte/depth/count/path/identifier bounds，Base64URL 必须无 padding 且 round-trip exact。

不得读取 current repository、Git、clock、environment、credential、process、provider、network、database、Hub、journal 或 Memory 来补齐输入。缺失、超限、未知 profile/version、digest drift 或 malformed canonical bytes 必须在产生任何 snapshot 前失败关闭。

## 执行 SOP

1. 用中立 ADR-0053 graph validator 重验完整 decoded bytes、canonical form、graph digest 与 run ID；schema-only 检查不够。
2. 分别构造 upstream source record 与 projector extractor record；两者只是可重建的 declared provenance，不能冒充 authenticated identity。
3. 用 caller-declared project namespace、module path、module-relative directory（root 必须是 literal `.`）和 package name 形成独立 node identity projection；先算 identity SHA，再形成 ID，最后只清空 self digest 计算完整 record SHA。
4. 基础 profile 只生成 module/package node、module→package `contains` 和 exact local package→package `depends_on` edge；test-source profile 另为每个且仅每个 `test_files` 非空 package 生成一个独立 test source-set node 与一条 module→test `contains` edge。`p` 与 `p_test` 不得合并，diagnostic 不得猜 package，也不得生成 package→test、`verified_by` 或 `observed_by`。compile/test dependency parallel edge 保持分离。Edge stable identity 不含 locator/revision，但完整 record digest 必须绑定它们。
5. 为每个 package 生成一个 ADR-0062 crosswalk；crosswalk 是 deterministic mapping，不表示两个 ID/domain/语义等价。
6. diagnostic 与 nested-module boundary 必须逐项双射为 unresolved node；每个非-local dependency 必须逐项双射为 unresolved edge，保留 reason、candidate、role/import 与 locators。不得 silent drop、猜 target 或把 unresolved 合并进 resolved set。
7. 对 source、extractor、node、edge、unresolved node/edge、crosswalk 分别计算 domain-separated structured set digest，并检查 raw UTF-8 byte order、唯一性、collision 和 dangling endpoint。
8. 精确生成 11 个 coverage surface：基础 profile 的 Go lexical surface 为 partial、另外十面为 not_observed；test-source profile 的 Go lexical 与 test verification 两面均为 partial、另外九面为 not_observed。新 profile 的 Go/test node 与 resolved-edge counts 必须构成互斥完整 partition；条件原因按 compile/test role 分流，不能双计或漏计。
9. freshness 只复制 declared observation time，expiry 与之相等，固定 UNKNOWN；system UNKNOWN reasons 必须完整，不能因零 diagnostic、零 unresolved 或 valid digest 缩减。
10. 重建 request、snapshot identity/final ID/self digest 与 envelope self digest，对 complete canonical envelope 做 byte-for-byte comparison。

## 输出契约

成功只输出所选 transport 的 exact GraphSnapshot envelope，包含 source/extractor、resolved/unresolved sets、ADR-0062 crosswalk、coverage 与 freshness。基础 profile 的正结果保持：

```text
PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical module/package subgraph only; coverage partial and system/freshness unknown; no selected-build, cross-surface completeness, truth, authority, completion, persistence, execution, impact, or effect attestation)
```

test-source profile 的正结果为：

```text
PROJECTED_PARTIAL_GRAPH_SNAPSHOT (exact ADR-0053 selected-module lexical module/package/test-source subgraph only; test nodes are source sets, not tests or outcomes; coverage partial and system/freshness unknown; no selected-build, cross-surface completeness, truth, authority, completion, persistence, execution, verification, impact, or effect attestation)
```

Stable ID 只称为 caller-declared project-scoped semantic-name-stable；它不是全局实体 identity、认证 provenance 或 rename lineage。Checker PASS 只证明 exact pure projection 的结构、identity、digest、排序、双射和 bounds 一致。

## 规则、禁止与权限

- 禁止人工编辑 derived node/edge 来覆盖 source；源变化必须重新投影。
- 禁止把 embedding、模型总结、文件名猜测或 schema acceptance 当作事实或 graph completeness。
- 禁止把 missing edge、unresolved candidate、not_observed surface 或 UNKNOWN freshness 改写为 no-impact、safe、complete、confirmed 或 fresh。
- 禁止把 declared observer/projector/source revision 当作 authenticated principal、current repository 或 atomic snapshot proof。
- 禁止从 `_test.go` 文件名、package/import 或 lexical source-set 推断 test declaration、case、execution、PASS/FAIL、coverage、flakiness、被测对象或 verification edge。
- 禁止由本 profile 创建 Evidence、Claim、Context、Grant、Approval、receipt，或满足 G3/Assessment Join。
- 本 Skill 无 repository/file、clock、process、network、database、provider、journal、persistence 或 effect 权限。
- Rust runtime 未交付；不得用 Schema 或 Python/Go 的成功冒充 Rust cross-language PASS。

## 自动化与验收

```bash
python3 -B harness/graph_snapshot_contract_check.py --golden <repo-root>
python3 -B harness/graph_snapshot_contract_check.py --test-source-golden <repo-root>
python3 -B harness/graph_snapshot_contract_check.py <repo-root> <graph-snapshot.json>
python3 -B harness/graph_snapshot_contract_check.py --test-source <graph-snapshot-test-source.json>
```

Python 与 Go 必须分别对两个 golden 产生 byte-identical envelope 和全部 digests，且 ADR-0065 golden bytes 必须保持不变。正反测试至少覆盖 root `.`、compile/test parallel edge、test source-set bijection、`p`/`p_test` 分离、diagnostic 不猜 package、无 verification/outcome 推断、跨 profile stable ID 与 profile-bound record、locator-only stability、diagnostic/boundary/nonlocal 双射、collision/dangling endpoint、disjoint coverage/freshness reason drift、Unicode/order/canonical/Base64URL、每字段与 aggregate limits、unknown profile/version 以及任一层 digest tamper。最终完成权威仍仅属于 `forge accept`。

## ADR-0075 portable partial-projector branch

仅当 caller 已持有某一冻结 wire 的 exact compact canonical request bytes 时，从 repository root 执行以下 exact argv；每个 projector 都是 zero-argument，stdin 必须读到 explicit EOF，且 request 后不得追加 LF：

```text
python3 -I -B skills/knowledge-graph-curation/scripts/check_package.py
python3 -I -B skills/knowledge-graph-curation/scripts/project_module_package_snapshot.py
python3 -I -B skills/knowledge-graph-curation/scripts/project_go_test_source_snapshot.py
```

两条 projection 命令分别只接受 ADR-0065 或 ADR-0066 已冻结的八字段 request；不得 cross-feed、包装成 fixture/envelope/union、添加 dispatch/profile 参数或从 raw graph 自动构造 request。成功必须是对应现有 canonical envelope 加一个 LF 且 exit 0；其他结果（含 partial stdout）都不是成功。

这是 source-only fresh/legacy scaffold 分发，不安装 host Skill，也不新增 authenticated context route、projector ABI、live producer/evaluator/runtime、repository/build/test capture、graph store、impact analysis、persistence 或 authority。Caller bytes、project/repository/producer/host/publisher 均未认证；coverage 保持 PARTIAL，system/freshness 保持 UNKNOWN，test-source nodes 只表示 lexical source sets，不表示 tests、outcomes、verification 或 coverage。
