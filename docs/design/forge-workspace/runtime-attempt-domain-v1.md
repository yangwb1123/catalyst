# Runtime Attempt Request Domain v1

> 状态：R0-C5 implemented / Sprint 148 DONE；只关闭 FR-03a pure request 子集，ADR-0107 仍为 Proposed。
> 日期：2026-09-01

## 1. 目的

R0-C4 已能在 Go 中确定性选择一个 passive `ReadyWorkItem`，但它不会创建或 dispatch Attempt。`forge-runtime`
已有 Platform Core Rust binding，却仍缺一个由 Runtime owner 解释的、不可变且有界的 Attempt 请求值。直接建设
FC-07 会越过尚未交付的 FR-04 execution journal 和 FR-06 local protocol；直接实现完整 FR-03 又会同时冻结
Session、Turn、Action、持久化和 lifecycle authority。

R0-C5 因此只建立 FR-03a：验证 caller-supplied `AttemptRequestInput`，成功后返回拥有自身存储的
`AttemptRequest`。它为后续 Runtime aggregate 提供初始 `requested` 输入，但本身不是 aggregate、command、wire、
ack、claim、dispatch 或状态推进器。

## 2. 物理与依赖边界

实现位于 `forge-runtime` 的 domain crate，并只依赖 Rust 标准库与现有 Platform Core Rust binding。它可以复用
Platform Core 的 `ScopeRef`、`EntityRef`、`RecordRef`、`ArtifactRef`、`ExecutorDescriptor`、`AttemptState` 和
idempotency validation，但不得依赖 application、infrastructure、interfaces、Hub store、SQLite、provider、tool、
filesystem、process、clock、random 或 network。

该模块不新增 serde derive、canonical JSON、digest domain、schema、fixture wire 或跨语言 binding。已有 legacy
Run/Group/Graph execution 也不是它的 adapter 或 persistence owner。

为避免把带 caller diagnostic label 的 helper 扩成公共拒绝服务面，本切片复用的 Platform Core validators 只提升为
crate-private visibility，不从 `forge_runtime_domain::platform_core_contract` 的公共 API re-export。Attempt 模块只传入
固定或由 bounded collection index 形成的内部 label。

## 3. 值模型

`AttemptRequestInput` 是 caller-facing construction input。`AttemptRequest::try_from(&input)` 完整校验后，必须
防御性复制所有可变拥有值；成功对象的字段保持 private，只通过只读 getter 暴露。调用者随后修改原 `String`、
`Vec`、reference、budget 或 descriptor，不得改变已构造的 request。

请求包含：

- 一个完整 Attempt scope，以及与 scope 精确一致的 explicit Attempt、WorkItem、Project、ProjectSnapshot refs；
- `ControlVersionBinding` 中正数且有上限的 Objective、Change、WorkGraph 和 WorkItem versions；
- caller-declared executor adapter descriptor；
- 可选 context `ArtifactRef`，其完整 source-snapshot `EntityRef` 必须与 explicit ProjectSnapshot ref 相等；
  producer Attempt 必须不同于正在请求、尚未创建的 Attempt，以拒绝自因果输入；同 snapshot 的其他 producer 仍只是
  caller declaration，其 source scope、存在性、freshness 与 authority 留给任何 consumer 前的独立 resolution；
- 可选 workspace-capability `RecordRef`，record type 固定为 `forge.runtime.workspace_capability`；
- 可选 capability-grant `RecordRef`，record type 固定为 `forge.control.capability_grant`；
- 有界 Approval refs，record type 固定为 `forge.control.approval_record`；
- 零到有界数量、唯一的 lowercase requested-effect tokens；owned value 按 byte order 排序归一；
- `AttemptBudget`：duration、cost、model call、tool call、input token、output token、output byte 和 network byte ceilings；
- 正数 timeout，且不得超过 duration budget；
- 复用 Platform Core 的 16–128 visible-ASCII idempotency key；
- 构造器派生、固定为 Platform Core `requested` 的初始 Attempt state。

Executor、workspace capability、Grant、Approval、effect 和 budget 都是 caller declaration。结构合法不认证其
producer、issuer、approver、principal、currentness、scope authority、revocation 状态、余额或 permission。

冻结边界如下：

| 字段 | 合法范围 |
|---|---|
| Control aggregate version | 每项 `1..=1_000_000_000` |
| Approval refs | `0..=16`，`record_id` 唯一；owned value 按 `(record_id, record_sha256, record_type)` 排序 |
| requested effects | `0..=32`；每项 `1..=64` ASCII bytes，首字符 lowercase，尾字符 lowercase/digit，中间只允许 lowercase/digit/`.`/`_`/`-` |
| duration | `1..=31_536_000_000 ms` |
| model/tool calls | 每项 `0..=1_000_000_000` |
| cost/input tokens/output tokens/output bytes/network bytes | 每项 `0..=1_000_000_000_000_000` |
| timeout | `1..=max_duration_ms` |
| idempotency key | Platform Core `16..=128` visible-ASCII bytes |

## 4. 关系与失败关闭不变量

构造过程必须验证以下全部 Platform Core 结构与 R0-C5 跨字段关系。编号只用于归组，不冻结检查先后、首个错误或
diagnostic message；实现可以在不返回部分对象的前提下按内部依赖调整 fail-fast 顺序：

1. `ScopeRef` 必须是从 Space 到 Attempt 的完整 contiguous scope，并包含 Project、ProjectSnapshot、Objective、
   Change、WorkGraph、WorkItem 和 Attempt；Session、Turn、Action 必须为空。
2. explicit refs 必须使用各自 exact entity type，且 ID 与 scope 中对应 ID 完全相同。
3. 四个 Control versions、timeout 和每项 budget 必须在模块冻结的有界范围内，timeout 不得超过 duration ceiling。
4. executor adapter、record refs、context Artifact 和 requested effects 必须通过各自 structure、type、snapshot、
   uniqueness 与数量检查；Approval refs 与 effects 都按 set 处理，排列等价、重复拒绝，非空 effects 必须声明
   grant ref，但该关系不认证 Grant。
5. 任一错误返回 `InvalidValue` 或 `ReferenceMismatch` domain code，且不返回部分构造对象；message 只作诊断，
   不冻结为 wire error。

R0-C5 不解析 reference 指向的 bytes，不查询任何 current Control version，不证明 scope lineage 存在，不确认 Artifact
存在或 content digest 正确，也不判断 Grant/Approval/effect 是否足以执行。

## 5. 初始状态边界

`requested` 只表示一个通过 pure construction validation 的初始值。调用者不能提供或覆盖 state；模块不暴露
`accept`、`start`、`run`、`interrupt`、`complete`、`fail` 或 `uncertain` reducer，也不调用 Platform Core
transition validator 来推进 current state。

FR-04 才能决定 Attempt aggregate、journal sequence、idempotent durable creation、outbox 和 crash recovery。FR-06
才能冻结 StartAttempt wire、handshake、deadline、stable protocol error 和真实 Go↔Rust process contract。在这些边界
存在前，任何 `AttemptRequest` consumer 都不得把 request presence 当作 accepted、claimed、dispatched、running 或
durable。

## 6. 验证要求

Focused Rust tests 必须覆盖：

- 最小/最大合法 request，以及每个数值、字符串和 collection bound 的相邻反例；
- scope 缺层、越层、explicit-ref type/ID substitution、snapshot mismatch、context self-producer 和错误 record type；
- duplicate/invalid effect、Approval ref substitution、invalid idempotency、timeout/budget contradiction；
- 每类 caller-owned nested input 的代表性 mutation 后 request 内容不变、getter 不泄漏可变 alias、clone/equality 稳定；
- state 永远是 `requested`，API/source boundary 中没有 reducer、persistence、wire 或 effect consumer；
- pure-boundary gate 以 30 秒、4 MiB stdout 上限的 `cargo metadata --no-deps --locked` 动态枚举所有 workspace
  member 与 Cargo target，拒绝 custom build、workspace proc-macro、generated `include!`、绝对路径、未知大写 crate
  root、crate 外模块路径及 import alias/glob；去除注释与字面量后的 exact token allowlist 同时拒绝 ambient
  standard-library path 和 domain direct dependency；
- Cargo.lock、workspace root/member manifests 的 exact SHA-256 与完整 manifest inventory 必须匹配冻结值，repository
  Cargo config、额外 nested manifest 或 lockfile 一律失败关闭；因此依赖 crate target 名、renamed dependency 与已审查
  proc-macro provider 不能在不更新 review boundary 的情况下漂移；
- consumer gate 在硬限制 entry、directory、file、source bytes 和 depth 的 workspace inventory 上扫描
  lib/bin/example/test/bench/helper source，只排除 Attempt 定义和本切片明确列出的 invariant fixture；每个 `#[path]`
  必须是顶层、无转义、根目录内且指向未排除 regular Rust source，raw identifier、use tree、module alias 及
  空白/注释分隔的 `execution::attempt` path 保持敏感；
- derive、attribute 与 function-like macro 只接受冻结的 exact invocation allowlist，unknown proc macro、macro
  metavariable invocation、generated `include!` 和未审查 code-generation path 失败关闭；现有 10 个 local-macro
  source 必须同时匹配 exact workspace-relative path 与 whole-file SHA-256（每个最多 1 MiB），所以 macro definition、
  alias/substitution 或位置漂移都要求重新 review。`AttemptRequest` inherent method 的 public/private inventory 也必须
  与冻结 getter/constructor/helper 集合完全相等，四个 production source 的 exact SHA-256 inventory 则锁住 module
  export、free item 与 trait-impl drift。该证明依赖 Cargo.lock 固定的已审查 proc-macro provider；它不声称已检查
  这些受信 provider 的 expanded MIR 或提供零信任 macro sandbox；
- domain crate normal tests、Rust workspace all-target tests/build、fmt、strict Clippy、架构/治理检查、fresh-context
  architecture/domain 与 security/reliability review，以及正式 `forge accept`。

通过这些检查只证明当前 Rust value 的 bounded construction、ownership 和关系。它不证明 caller 数据真实、
authenticated、fresh、authorized、persisted、idempotently admitted 或 safe to execute。

## 7. 非目标与后续链

R0-C5 明确不交付：

- Attempt lifecycle reducer、transition authority、Session、Turn 或 Action；
- SQLite schema、migration、journal、outbox、CAS、Artifact resolution 或 ExecutionReceipt producer；
- serde/canonical wire、StartAttempt command、protocol server、handshake、ack、cursor 或 Go client；
- workspace capability resolution、Grant/Approval authentication、revocation、budget reservation 或 effect release；
- Reconciler consumer、WorkItem transition、Attempt claim/dispatch、provider/tool/filesystem/process/network effect；
- Harness verification、completion join、API、CLI、TUI、App 或 Timeline。

依赖链保持：`FR-03a request value → FR-03 后续 lifecycle values → FR-04 execution journal/outbox → FR-06 local
protocol → FC-07 RuntimePort client`。每一步都需要独立 ADR、review、测试与正式 acceptance；R0-C5 不能提前勾选
这些后续项，也不关闭 F2、F3、F4、F6、R0 Developer Preview 或 Objective-to-Outcome。
