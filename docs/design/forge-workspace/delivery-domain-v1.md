# Delivery Domain v1

> 状态：R0-C3 candidate；Go 内部 pure domain，不是跨语言 wire、持久化服务或用户入口。

## 1. 目的

FC-05 为后续单 WorkItem 垂直闭环和 pure Reconciler 提供一个确定、可测试的 Delivery Domain：

- Objective 描述期望 Outcome，不包含执行命令；
- Change 把 Objective、目标 ProjectSnapshot、AcceptanceCriteria、policy declaration 和总预算绑定；
- WorkGraph 把 Change 分解为 snapshot-bound WorkItem DAG；
- 状态函数只校验声明的边，不读取 current state、不授权也不推进状态；
- caller-supplied snapshot comparison 只报告声明差异，不证明 filesystem 或 repository freshness。

删除这个 domain 会使后续 Objective 创建、Change planning 和 Reconciler 输入重新退化为 transport map 或旧 graph 协议，因此它是 FC-06 之前的最小 Go 业务边界。

## 2. 包与依赖

```text
forge-core/internal/delivery/domain
  └── platformcorecontract + platformcorecontract/state
```

Delivery 生产代码只允许审查过的标准库和上述 Platform Core 包。direct exact import allowlist 扫描所有非测试 `.go` 文件；
另一个 dependency-source guard 扫描当前 Platform Core reference/state/wireprofile 生产源码的全部 direct import path，并明确
拒绝新增 `os|os/exec|net|database/sql|time|math/rand|crypto/rand` 等路径。它只约束依赖路径，不证明已允许的 `fmt`、`io`
或其他 package 中每个 API call 都无副作用；当前被调用 validator 的行为仍由源码审查与测试证明。包目录只允许 canonical、single-link、
non-symlink regular Go source（测试 `testdata` 除外），拒绝 assembly、native object 和子 package；检查从打开的 file identity
读取并做前后 identity/metadata 复核。它是当前 completion tree 的 repository guard，不是 compiler-byte attestation、原子
check-to-use 证明、调用级 effect analysis 或 runtime sandbox。所有源码检查都要求同一棵 frozen、quiescent tree；它们
不形成内部 atomic snapshot，不能对抗扫描期间同 inode、同 size 且恢复 timestamp 的写入或目录先改后还原。
目录遍历通过每批最多 128 项的 `ReadDir` 完成，并在排序或读取源码前执行全局上限：最多 16,384 个目录项、
12,288 个非目录文件、64 层深度、256 MiB stable-size Go 源码预算；单文件仍最多 1 MiB。每个源码文件在
open/read 前先按 `Lstat` stable size 预留聚合预算，读取后再要求 exact size 与前后 identity/metadata 一致。
每个上限都接受 exact limit、拒绝 one-over-limit，计数使用先减后加的 overflow-safe 检查。Unix 从 `Stat_t.Nlink`
读取 link count，Windows 从打开句柄的 `ByHandleFileInformation.NumberOfLinks` 读取，不把 `Lstat` 的属性缓存误当 link metadata。

本切片不创建 `application`、`store`、API 或 projection 包，不修改 `control.db` schema v1，也不产生 canonical Command/Event。
领域值是 caller-owned mutable draft；调用方必须让完整 reachable slice/pointer graph 在整个校验调用期间保持 race-free、
exclusive-stable。validator 不同步、不 deep-copy，也不形成 atomic snapshot；成功结果只描述该稳定输入，不是可缓存的
validated token。机器检查禁止
R0-C3 出现 production consumer：全源码 lexical scan 不跳过 `testdata`、拒绝 module symlink 并覆盖所有 build-tag source，
Linux/Darwin/Windows 的离线 `go list -e -json ./...` 在 empty HOME/module/build cache 下再解析本地 production package
的 direct-import metadata；外部 module 不可用时由 `-e` 保留本地 metadata，网络/VCS 路径则关闭。Go tool 通过
支持 `waitid(WNOWAIT)` 的 Linux 上 `Setpgid` + 与 reap 串行化的 group `SIGKILL`、其余目标 direct-child kill，以及
全平台 parent-reader drain deadline 的 bounded executor 运行；正常退出不会触发后代清理。aggregate
output 与错误文本另有限额；输出总数使用 saturating signed-64 count 加显式 overflow flag，任何 count overflow 都在解析前
失败。执行器只接受最多 16 MiB 的内存 stdin，不接受可能永久阻塞的任意 Reader。process group 只是 best-effort teardown
handle，不是 containment；escaped 或正常父进程退出后仍运行的 descendant 仍可存活。drain deadline 只关闭父进程侧
capture readers，不关闭后代持有的 writer；此时暴露 `DrainIncomplete` 并记录独立警告。
同一 shared-runner 复核还把 sandbox completion/cancellation 的线性化点固定在 Runner 返回后的立即 context sample：
此时已可见的 cancel/deadline 一律压过 runner 的 nil/zero 回报，并禁止 validation、durable commit 与 Observe。
Docker/Firecracker 的 host byte count 使用 signed-64 saturation；Firecracker 不再接受 guest 可写 marker 或串口 sentinel，
而由受信 PID 1 在随机 root-only 目录打开输出、以 no-new-privs/空 capabilities 的 uid/gid 65534 执行 workload，
等待 VMM 退出后再由 host bounded 导出 strict status/output。rootfs 的兼容 `setpriv` 是明确 TCB/prerequisite，
该 command-only runner 仍不提供 repository exchange、credential channel 或 artifact sync-back。
resolved `GOROOT/bin/go` 只是 path-pinned host TCB；检查不证明该 path 的 open-file identity
或 digest。后续 consumer 必须先决定 defensive ownership，或在实际 operation boundary 对将使用的 exact value 重新校验。

## 3. Objective

Objective 包含：

- `obj_` ID、Space、title、desired outcome；
- 0..32 条 constraints；
- 1..16 个唯一 target Project；
- 1..32 条唯一 success measures；
- caller-declared creator、时间、state 和正 version。

文本按 UTF-8 byte 长度计数，必须是 valid UTF-8，拒绝首尾 Unicode whitespace、control、全部 `Cf` format、
`Zl` line separator 和 `Zp` paragraph separator scalar（因此也拒绝 bidi/zero-width/BOM format）；
validator 保留 caller 的 exact bytes，不做 Unicode normalization、截断或修复。target Project 必须是 `prj_` Platform ID。

Objective state vocabulary：

```text
draft  → active | cancelled
active → satisfied | cancelled
```

边校验不代表 current-state authority；尤其 `satisfied` 不证明 Outcome。

## 4. Change

Change 必须引用一个已通过校验的 Objective，并保持相同 Space/Objective identity 与 exact parent version。它包含：

- `chg_` ID、title、`ObjectiveVersion`；
- 每个 Objective target Project 恰好一个 `prj_ → psn_` snapshot binding；
- 1..64 条 AcceptanceCriterion，criterion ID 唯一；
- 可选 unresolved ImpactAssessment `RecordRef`，其 `record_type` 必须为
  `forge.delivery.impact_assessment`；
- policy profile declaration；
- 总 attempt/duration/cost budget；
- caller-declared proposer、时间、desired state、observed state 和正 version。

Desired state 与 observed state 分离。Desired vocabulary 是
`proposed|awaiting_approval|approved|active|paused|cancelled`；Observed vocabulary 是
`not_started|in_progress|blocked|verifying|completed|failed|uncertain|cancelled`。exact edges 是：

```text
desired:
proposed          → awaiting_approval | cancelled
awaiting_approval → approved | cancelled
approved          → active | cancelled
active            → paused | cancelled
paused            → active | cancelled

observed:
not_started → in_progress | blocked | cancelled
in_progress → blocked | verifying | failed | uncertain | cancelled
blocked     → in_progress | failed | uncertain | cancelled
verifying   → in_progress | blocked | completed | failed | uncertain | cancelled
uncertain   → blocked | failed | cancelled
```

未列出的 edge 全部拒绝，`completed|failed|cancelled` observed state 没有 outgoing edge。两个 transition validator
只检查 supplied pair；它们不解析 Approval、Receipt、Evidence 或 Outcome，也没有 current-state read 或 mutation effect。

## 5. WorkGraph 与 WorkItem

WorkGraph 必须引用同一个 Space/Objective/Change、exact `ChangeVersion`，并逐项复制 Change 的 exact snapshot binding。
Graph 包含 1..128 个唯一 WorkItem、最多 512 条 dependency edge、author、时间、state 和正 version。版本相等只绑定
caller-supplied parent revision，不读取或证明 durable current head。

每个 WorkItem 必须：

- 使用 `wki_` ID，绑定 Graph 中一个 exact ProjectSnapshot；
- 有 bounded purpose、至少一个有效 AcceptanceCriterion ref 和至少一个 verification requirement；
- 依赖只指向同 Graph 的其他 WorkItem，且不得重复或自环；
- 声明 requested effects、risk、agent requirements、可选 context ArtifactRef 和自己的预算；
- 使用 Platform Core WorkItem state vocabulary。

Risk vocabulary 固定为 `low|medium|high|critical`。Policy profile、criterion ID、requested effect、criterion/work-item
verification requirement 都是 1..64-byte lower ASCII token：首尾只能为 `a-z0-9`，中间另允许 `._-`；它们只是
caller declaration，不解析 registry、policy 或 verifier。ActorType 与 WorkItem state 的闭集由 Platform Core validator
拥有，Delivery 不复制其值集合。

所有 Change criterion 至少被一个 WorkItem 覆盖；该 criterion 的每条 verification requirement 还必须出现在至少一个
引用它的 WorkItem verification declaration 中。WorkItem attempt/cost 总预算不得超过 Change 总预算；DAG 上按
WorkItem duration ceiling 计算的最长依赖路径不得超过 Change duration。并行分支不相加，所有路径计算均 overflow-safe。
Requested effects 是未授权声明，不是 Grant 或 permission。

Graph state vocabulary 是 `draft|proposed|accepted|superseded`，edge 只有
`draft→proposed`、`proposed→accepted|superseded`、`accepted→superseded`。WorkItem edge 完整复用
`platformcorecontract/state`，Delivery 包只包装错误 relation，不复制推进 authority。Graph state 与各 WorkItem state
在本切片中是独立 caller declaration；跨 aggregate current-state consistency 属后续 fold/Reconciler，不由静态 Graph 值猜测。

DAG validator 使用确定性 lexicographic Kahn traversal；输入 item/dependency 顺序不影响输出，cycle、missing dependency、
duplicate ID 和 over-bound edge 全部失败关闭。完整 validator 还覆盖 Objective project、Change/Graph snapshot、criterion、
criterion requirement、criterion ref、effect、agent requirement 与 item verification 等 set-like declaration 的顺序不变性。
Topological order 不是 ready-node selection，FC-06 才拥有选择权。

## 6. Snapshot comparison

`CompareSnapshotBindings` 比较两组 caller-supplied `ProjectID/ProjectSnapshotID` declaration，返回排序后的 missing、changed 和 unexpected Project IDs。
bound set 最多 16 条，current declaration set 最多 32 条，因此 16 个 expected Project 加额外 unexpected Project 仍可表达且保持有界。

空差异只表示两组 supplied identifiers 相同。它不 stat/open Project、不读取 Git、不选择 current Snapshot，也不证明 freshness、completeness、repository identity 或 secret safety。任何 observer/current-head owner 必须在后续边界中单独实现。

同样，binding validation 只验证 typed ID、唯一 Project、唯一 Snapshot 与 caller-declared pair；它不读取 Workspace Catalog，
因此不证明某个 `psn_` durable record 的 parent 确实是该 `prj_`。可选 ArtifactRef 也只做 Platform Core structure 与
declared source Snapshot ID relation 校验，不解析 Artifact bytes、provenance Record 或存在性。

## 7. Limits

```text
target projects                  1..16
snapshot compare bound/current   1..16 / 0..32
constraints / success measures  0..32 / 1..32
acceptance criteria              1..64
criterion verification entries   0..16
work items / dependency edges    1..128 / 0..512
optional item list entries        0..32
criterion refs / verification     1..32 / 1..32
title / purpose                  1..256 / 1..2048 bytes
outcome / criterion description  1..4096 / 1..2048 bytes
constraint/success/agent text     1..1024 / 1..1024 / 1..256 bytes
attempt budget                    1..1024
critical-path duration budget     1..2,592,000,000 ms
cost budget                       0..1,000,000,000,000 micro-USD
```

attempt/cost 加总与 critical-path duration 都先做 overflow-safe bound 校验。Domain validator 不截断、不排序或修复调用方值。

## 8. 稳定错误

```text
ErrInvalidDomain
ErrInvalidTransition
```

错误 relation 稳定，诊断文本不是 future HTTP/CLI wire。输入失败没有副作用。
对于包含多个不同缺陷且 declaration 顺序不同的 malformed set-like list，首条诊断 detail 可以不同；只保证
`errors.Is` relation。criterion map diagnostic 和 WorkItem validation 有局部确定顺序，但这不升级为 canonical error wire。

## 9. 验收

Focused tests 必须覆盖：

- every field boundary、typed ID、reference、Unicode/control/bidi mutation；
- Objective/Change/Graph/WorkItem state vocabulary 和所有合法/非法边；
- target Project 与 snapshot exact coverage；
- criterion scoping/coverage、budget aggregation、diamond critical path 和 Artifact snapshot relation；
- all 24 WorkItem orderings、set-like declaration order、deterministic DAG order、cycle/self/missing/duplicate/edge bounds；
- caller-supplied snapshot missing/changed/unexpected comparison；
- direct/dependency import-path allowlist、module symlink rejection 与三平台 production consumer graph；
- randomized deterministic DAG property suite；
- full normal/race/vet/build、architecture/governance、fresh 双审和正式 acceptance。

## 10. 未交付

- canonical Objective/Change/WorkGraph Command/Event schema 或 journal folds；
- identity/time generation、application service、repository、schema v2、projection；
- local actor authentication、authorization、Approval/Grant/Policy evaluation；
- Impact observer、filesystem/Git Snapshot current-head resolution；
- Reconciler、ready-node selection、Attempt dispatch、Runtime/Harness transport；
- Receipt/AcceptanceCriteria join、Change/WorkItem completion authority；
- HTTP/CLI/TUI/App、Timeline、Change Cockpit 或 Outcome。

当前 mutable aggregate 没有 production consumer；任何未来消费方都不得把一次成功的 `Validate*` 调用保留为随后可变值的
authority；并发 mutation 属调用方 data race，而不是 mixed-value atomicity 保证。源检查不证明被 Go compiler 消费的
bytes 与测试读取 bytes 原子相同，也不证明 allowlisted dependency API 的调用级 effect 行为；有并发 source writer 时
本 guard 不产生可信结论。

因此本候选树一旦通过 fresh 双审与正式验收，只关闭 FC-05 的 pure domain 子集，不关闭 FC-06、F3/F4、
R0 Developer Preview、Objective→Outcome 或完整 App。
