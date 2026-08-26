# ForgeOS — Architecture

> 本文 = **当前/脊柱**(v0–v1 在 Claude Code 上的形态)。
> **目标架构(分布式 HA 微服务全貌)** 见 [`architecture/north-star.md`](architecture/north-star.md) + [`architecture/ha-security-rollout.md`](architecture/ha-security-rollout.md)。

## Proposed-only ADR v2 边界

ADR-0067 为当前 `writes_adr` 唯一新增候选与显式 universal checker 提供 exact canonical JSON frontmatter、固定 Markdown body 和 domain-separated digest 验证。Universal checker 不扫描 repository；Go 保留既有 legacy-baseline integrity snapshot，但不把旧 ADR 当作 v2 解析、retro-validate 或迁移。它不认证 owner/approver，不解析 ApprovalRecord/Claim/Evidence/Graph 引用，不产生 Graph edge/coverage，也不实现 Accepted immutability、supersession/compliance、persistence 或 lifecycle authority。

## Authority-neutral Capability Registry v1 边界

ADR-0068 只交付一个 wire `status=staged` 的 singleton Registry：唯一 entry 是 `local-go-package-impact-prescan/1`。Go/Python 独立重建 canonical bytes 与 content-set→contract→entry→registry→request→assessment digest chain；CLI 只消费显式 Registry/Request 输入，physical checker 只读取 Registry 声明的 refs。`resolved_exact` 仅表示声明引用匹配，不认证 Registry/owner，不投影 140-item planning catalog，不生成 catalog→package adapter，不选择/执行 implementation，也不激活 Grant/PDP、构造 CapabilityInvocation、加载 plugin 或做 runtime routing/persistence/transition/effect。

## Planning Capability Ownership Projection v1 边界

ADR-0069 保持 Proposed，独立 Python/Go pure projector 只消费 caller-supplied exact catalog/mapping bytes，重算 17 nodes、145 occurrences、140 unique capabilities、38 declared packages 与 140 unique primary-owner bindings，并派生 unresolved `.agent/skills/<owner>.md` locator。产品 CLI 只有 `forge capability-ownership project --catalog FILE|- --mapping FILE|-`。它不修改 ADR-0068 singleton Registry，不解析或生成物理 Skill/adapter，不认证 owner/authority，也不产生 Grant/PDP、CapabilityInvocation、implementation selection/execution、plugin/runtime routing、persistence、transition 或 effect。

## Local Project Source Snapshot v1 边界

ADR-0070 保持 Proposed，交付一个 Linux-only、显式 opt-in 的两端点 local Git worktree source observation producer、source-portable pure Python decoder 与 closed portable `project-snapshot` Skill。它只投影 allowed regular bytes、tracked-absent facts、hashed pre-read path-policy exclusions、ignored count 与 exact PARTIAL/UNKNOWN coverage；Git/HEAD 未认证，path policy 不是 content DLP，观察非 atomic/current/complete。Scaffold 只分发 source package，不复制 Catalyst Go runtime、不安装宿主 Skill；unsupported host 或 runtime 不存在固定 exit 3/`not_executed`，已存在但不兼容/执行失败固定 exit 1。它不生成 GraphSnapshot，不修改 ADR-0068 Registry，也不产生 Grant/PDP/CapabilityInvocation、truth、permission、persistence、routing 或 effect authority。

## Portable Context Engineering Skill 边界

ADR-0071 保持 Proposed，并把 ADR-0055 的 exact caller-supplied ContextPackage v1 pure projection 包装为 closed source package 与零参数 stdin adapter。Registry v26 只增加 delivery/ref/pin；既有 wire、Schema、golden 和 Python/Go/Rust semantics 不变。Package 不发现 repository/ambient source，不调用 live provider/model，不编译 prompt，不安装 host Skill，也不提供 publisher authentication、check-to-use atomicity、Grant/PDP/Approval、truth/instruction/completion/persistence/routing/effect authority；Python `-I` 也不隔离 system site、stdlib、interpreter startup 或 host。

## Portable Evidence Claim Validation Skill 边界

ADR-0072 保持 Proposed，仅将 ADR-0045 既有 pure Python structural validator 包装为 closed 18-file source package 和零参数 exact-stdin adapter。Registry v27 只增加 validation-only delivery/ref/pin，ADR-0045 wire、Schema、golden 与 Python/Go/Rust semantics 不变。Package 不观察或 author records，不修复、排序、补 digest、返回 record bytes、读 ambient source 或持久化；portable prose 不进入 authenticated routes，source-only scaffold 不安装 host Skill。结构有效不证明 truth、provenance、completeness、Grant/PDP/Approval、routing、transition、execution 或 effect authority。

## Portable Policy Authority Declaration Assessment 边界

ADR-0073 保持 Proposed，只将 ADR-0056 CapabilityGrant 与 ADR-0059 ApprovalRecord 的既有 pure declared evaluator 包装为 closed 30-file source package 和两个独立零参数 explicit-EOF exact-stdin adapter。Registry v28 只增加 delivery/ref/pin，两个 wire、Schema、golden 与 Python/Go/Rust semantics 不变，也不扩大 runtime scope。Package 不签发或激活 Grant、不使 Approval 生效、不读取 live policy/identity/approval/revocation/usage state、不调用 Kernel/PDP/PEP/ADR-0057/0058 runtime；portable prose 不进入 authenticated routes，source-only fresh/legacy scaffold 只复制 source 且不安装 host Skill。Positive relation 不证明 authorization、permission、transition、execution 或 effect authority。

ADR-0074 保持 Proposed，只将 ADR-0067 既有 Proposed ADR v2 pure validator 包装为 closed 25-file source package 与 exactly-one-basename-argument、explicit-EOF exact-stdin adapter。Registry v29 只增加 delivery/ref/pin，ADR-0067 wire、Schema、golden、Python/Go semantics 与 shipped evaluator scope 不变。Caller lexical basename 保留独立字符串相等语义但不证明 physical file/repository identity；package 不扫描 repository、不 author/repair/reseal/accept/supersede/persist ADR、不复制 Go `writes_adr` runtime。Portable prose 不进入 authenticated routes，source-only fresh/legacy scaffold 只复制 source 且不安装 host Skill；结构有效不证明 identity、ownership、approval、truth、compliance、lifecycle、execution 或 effect authority。

## 脊柱:Idea → Production
```
DISCOVER  (深度由 mode 裁决)
  Requirement-Discovery → 置信度% + 缺失信息   stop: confidence ≥ 80%
  Market-Research       → 竞品/能力矩阵         (必须真实检索+引用,防自信虚构)
  Product-Designer      → MVP / 高级 分层
DESIGN
  Solution-Architect    → 架构(按 lifecycle 分阶段,非峰值 QPS)
  Proposal-Generator    → 1页方案+成本+风险 ──▶ ★ HUMAN APPROVAL ★
                                                 └ 批准 → 仅解锁 REVIEW（产物由 phase 契约负责）
REVIEW  (深度由 mode 裁决;对齐 AI-SDLC Stage 2-6)
  Security-Engineer     → STRIDE 威胁建模 + RFC 合规矩阵
  Distributed-Engineer  → 故障模式矩阵 + 一致性策略 + 重试策略
  Performance-Engineer  → 性能预算 + 生产就绪检查清单
  CTO                   → 综合裁决(Approve/Simplify/Redesign/Delay/Reject)
BUILD
  Planner → Implementer → [Harness 闸门] → Reviewer → QA(`qa_v1` strict verdict)   stop: ROADMAP 100%
DEPLOY  (声明式生产交付边界;不访问凭证/不执行远程部署)
  Release-Engineer      → Manifest + Plan + Runbook + Go/No-Go + Validation
  External CI/Operator  → 实际应用 ──▶ ★ HUMAN APPROVAL MARKER ★
ROLLBACK  (独立按需,不接主链)
  Release-Engineer      → Rollback Plan + Runbook + Checklist + Validation
  External CI/Operator  → 实际应用 ──▶ ★ HUMAN APPROVAL MARKER ★
EVOLVE
  Scan(`evolve_scan_v1`) → Gap → Roadmap → Implement → Harness → Review → Evaluate → (loop)
```

## 中枢旋钮:mode × lifecycle
一个设置同时驱动三处:**Router 档位 · Harness 严格度 · Workflow 深度**。
- mode: explorer(快/省/跳 Discover) · balanced · engineering(全闸门) · cto(只出 PRD/Arch,人确认)
- lifecycle: idea → mvp → growth → production
- explorer→engineering = 一次「创业→企业」状态迁移:自动收紧 harness + 派生补测试/CI/监控任务。
- 持久 `lifecycle→production` 只能经 `forge migrate --to-lifecycle production --apply`；Explorer 在同一可恢复事务中触发上述 mode 迁移，run/evolve 临时 flags 永不写盘。terminal receipt 是执行前双检的治理 floor。

### Evolve 内容契约
shipped Evolve workflow 的首 phase 以 `scan_contract: evolve_scan_v1` 冻结有效
`EvolveDepth`：opportunistic 仅允许直接证据支持的明显机会，standard 不声称全覆盖，
thorough 必须逐一覆盖 code/dependencies/security/performance/architecture drift/test coverage
且从每个 finding 派生 candidate task，advisory 明示限制且不授予实现权限。最终非空行必须是
`EVOLVE_SCAN_V1: {compact JSON}`；证据只能指向当前仓库内已有、非 symlink、≤1 MiB 的
UTF-8 regular file 和有效行号。完整 canonical report（≤64 KiB）不经摘要截断地 feed forward。
checkpoint v3 同时持久化 phase cursor、scan report、整数微美元预算/花费、Agent-call 与
loop-back 计数；串行 resume 重验后恢复 report 且不重放已完成 scan，native parallel
只在 iteration boundary checkpoint，中断 iteration 可以整体重放。结构、coverage 声明和
locator 可机器复验，但不把 Agent 的“clear”判断或机会价值冒充事实认证。

## 载重墙(对原始构想的修正)
"站在所有 CLI 之上" ⇒ 只能强制最弱宿主允许的东西。因此:
- **真相之源 = 带外执法层**(Sandbox / CI runner 跑 harness 闸门),host-independent。
- 各工具的 hook(CC 的 PostToolUse/Stop 等)= **加速器适配器**(编辑器内快速失败),非地基。
- 每个宿主一个薄 adapter;无阻断能力处优雅降级为 advisory。
- **生产交付边界**:`deploy`/`rollback` 只生成与验证精确声明的 `docs/release/*`；
  command-mode 使用最小固定 prompt、operator-pinned Claude executable bytes(非供应商身份)、整树 postflight 和
  receipt/source/artifact freshness。这里的 source 是排除 `.forge/**`、`docs/release/**`
  和 commit metadata 的 product 工作树摘要，不是 Git commit identity。云/K8s 凭证和远程执行始终归外部 CI/operator；
  人核对外部证据后写 approval marker，agent 不得自证发布成功。

## 引擎 (Engines)
Gateway · Orchestrator · Agent-Runtime · **Model-Router** · Context-Engine · Memory-Engine ·
Knowledge-Engine · **Evaluation-Engine** · **Sandbox(载重墙)** · Web-UI
> **v2 现状**:`go list ./...` 当前实测 **61 包**（59 个 internal（含嵌套包）+ `cmd/forge` + 独立非 Agent `cmd/forge-kernel`，纯标准库零依赖；仅含测试的 `internal/adr` 不是产品运行时切片）,已落地 5 个核心引擎与可工作的本地 Agent-Runtime 切片:
> **Orchestrator**(`internal/orchestrator`)· **Model-Router**(`routing`)· **Context-Engine**(`prompt`)·
> **Memory-Engine**(`memory`)· **Evaluation-Engine**(`converge`);Agent-Runtime 已具备本地命令执行、预算/超时/进程组、最小环境、stdin prompt、产物溯源与 run lock。`forge run --chain` 以版本化状态跨 Discover→Design→Review→Build→Deploy→Evolve 持久恢复，拒绝/cycle/max-stage/策略 halt 均失败关闭。
> `forge-runtime/` 现有 Rust 原生多轮模型/工具循环、SQLite local-first Conversation Hub 与 durable Project Run：无路径 Global、有路径 Project、Group 联动；execution-bound Project Run、append-only event journal、O(1) 增量语义 cursor、同快照 inspection、严格有界 causal user/assistant history、Run 原子授权 assistant 写回、terminal/incomplete/pending-tool 判定均跨进程持久化。Group dossier 可被原子冻结为独立 prepared Group Run，幂等重放精确旧快照且不查询最新历史；独立 Group Execution 能纯本地验证冻结输入并恢复 content-free integrity receipt。其后的 two-phase Group analysis 在 SQLite v5 先原子准备 exact、零工具、`store:false` 请求，再经当次显式同意、claim 前凭证/目标预检和单赢家 authority 至多外发一次；claim 后不自动重发，只有完整 provider terminal 能原子提交结果，默认输出隐藏正文。SQLite v6 又能把同一 frozen source 的 2–8 份 completed 分析按声明顺序冻结为本地 canonical panel，同 key 精确重放并在 show 时重验所有来源，默认不显示结果正文；这仍只是并排组装。SQLite v7 再以独立 consent/claim/result journal 对一个 exact panel 做单模型综合：唯一 user message 是 canonical panel manifest，不重发原始 dossier，单赢家外发且 uncertainty 不自动重试，固定本地 artifact/no-writeback，并明确不冒充讨论、共识或事实验证。SQLite v8 进一步持久化 exact Group Run 上的 manager 指令、frozen member task assignments、dependency edges 与 deterministic waves，作为 `forge-core` 唯一调度器和 Rust 单任务 Agent Loop 之间的 immutable interchange artifact；SQLite v9 被动接收 Go `forge graph-plan` 生成的 canonical Core Plan并冻结 `awaiting_execution_contract` Run；SQLite v10 再由 Rust 导出 exact private control snapshot、Go 唯一选择 `plan.waves[0][0]` 并生成 canonical Node Execution Contract、Rust 以 seq/head CAS 登记唯一契约和第二事件，把 Run 推到 `awaiting_core_dispatch`；SQLite v11 使用现有 Responses 纯 codec 固定 exact provider body 与 content-addressed Node Dispatch Request，再以 seq-2/head CAS 追加第三事件并停在 `awaiting_dispatch_authorization`。Go 仍是唯一调度 owner；v11 准备链不释放 authority，也不把 topology waves、契约或 request presence 冒充执行。SQLite v12 只对严格单节点/单 wave/零 edge Graph 开放完整 effectful 生命周期：seq-3/head 与 Hub-global Project lane 原子 claim 后一次消费 exact body，bounded 收集 terminal/EOF 或 uncertainty，Go Core 从真实 v4 control 产生 terminal receipt，Rust 再原子保存 artifact/receipt、追加 seq 5、释放 lane 并进入 completed/failed/failed_uncertain。默认 deterministic/offline；显式 Project Run `--live` 默认零工具，仅 exact `--allow-read` 授权，并启用固定 HTTPS origin、无 redirect/隐式 retry、`store:false` 完整 validated output-item 回放、phase-aware final projection、terminal status/item identity 校验及 transport/SSE/token/output 全套上限；incomplete 永不释放工具。SQLite open→PRAGMA/WAL→schema 有统一 5 秒重试，DB/WAL/SHM 私有权限及 workspace capability 失败有并发/反例回归。后续 v17–v24 已交付非初始节点候选、per-node request/lifecycle 存储、predecessor receipt/content disclosure、wave-ready/admit、本地 hard-crash adjudication与 8 MiB successor candidate 持久化上限。Project Run 另已提供显式有界 resume、content-free explain 与 SQLite v28 root-input branch/direct-parent lineage；branch 原子创建 child + lineage + fresh seed，仅在显式 resume 后执行。当前仍缺顶层整图执行循环、任意 event-prefix branching、远程账号与同步、共享 ACL/Group 多 Agent discussion、受控写/进程工具及 Rust runtime 自身的 OS 沙箱整合。
> Sprint 51 在该切片当时不改变 SQLite v11/v3，并补上 effect-free release handshake：Rust 从完整重验的 durable aggregate 导出 private canonical release control，Go 独立重建 base control 与 scheduler/request binding 并产生 content-addressed authorization，Rust 再对当前 state 严格验证。该 artifact 不落库、不等于 consent/claim/signature，所有 provider/network/lane/result/writeback/advance effect 均为 false；它同时约束后续 effectful Graph 切片必须把 seq-3/head + global Project lane claim 与 bounded terminal result/receipt/lane release/advance 作为不可拆分生命周期设计。
> Sprint 52 在该切片当时继续保持 SQLite v11、Run v3/seq 3 与 authority=false：Go 生成固定官方 Responses destination 的 canonical `operator_asserted` pricing snapshot；Rust 以相同 digest domain 和逐项向上取整整数算法复验 exact bytes、authorization binding 与 frozen cost budget，并提供只读 readiness CLI。Rust production registry 另提供纯 metadata resolve + 显式 header-safe credential provider construction，但 readiness 不读环境凭证、不构造 provider。pricing 无 vendor attestation，也不是实时价格或账单保证；该切片据此要求后续 Result/Core receipt 必须绑定真实 claim/head/lane evidence。
> Sprint 53 以 ADR 0024 一次性交付该完整单节点边界：公共 `dispatch execute` 强制 fresh consent、显式 authorization/pricing 与 operator-pinned Go Core binary；SQLite v12 以 approved service path 的 non-`Clone` authority 和全 Hub Project lane 保证本地单赢家，可信 store adapter 属于进程内 TCB。Linux bridge 从密封并复验后的匿名 Core memfd 执行，其他 host 失败关闭。可捕获的 provider/protocol/timeout 与显式 application cancellation 等失败会固化 uncertainty 并进入 v5 `failed_uncertain`；CLI v1 的 OS signal 或其他 hard crash、Core 失败、最终提交不确定性才停在 v4 quarantine，所有 claim 后路径均禁止重发或 lease 自动释放。Result/uncertainty、usage/cost、claim/lane/control/receipt/seq-5 均为跨 Go/Rust canonical binding；只在最终原子事务中释放 lane。该协议不声称 remote exactly-once，也不执行 frontend/backend/SSO 多节点图。
> Sprint 54 以 ADR 0025 在不触碰上述 terminal fence 的前提下补齐多节点策略前置条件：Go Core 从 exact private v1 control 生成 content-addressed `GraphExecutionSchedule`，固定 serial wave-then-authored order、每 node Project lane、direct-predecessor future receipt slots、initial frontier 及 fail-fast/no-dataflow policy；Rust/SQLite v13 只新增 one-per-Run immutable sidecar 和 metadata inventory。Admission 不写 Graph 主 journal、不改变 Run/head、不创建 contract/request/result，不读 credential 或构造 provider，也不访问 network/workspace/tool/writeback；四个 progress/authority flag 固定 false。该 sidecar 不是多节点执行，后续 contract v2 必须显式绑定 schedule digest 与真实 predecessor receipts 才能改变 successor fence。
> Sprint 55 以 ADR 0026 交付该 contract-v2 的严格初始空前驱子集：Go Core 从同一 exact control 独立重建 schedule、只接受其 digest 并唯一选择 ordinal 0，冻结 pristine seq-1 head、exact Prompt/provider/budget 和空 predecessor/receipt；Rust/SQLite v14 把它保存为 immutable passive candidate sidecar。candidate v2 与 legacy lifecycle contract v1 在 immediate transaction 中互斥，但 admission 不改变 Run/main journal，也不创建 provider request 或释放任何 authority。默认 view 隐藏 candidate plaintext 并声明不包含 current lifecycle；receipt v1 不得作为中间 predecessor，candidate 不能 dispatch，非初始 contract/terminal/successor/progress 仍需新协议。
> Sprint 56 以 ADR 0027 为该 candidate 增加独立的 passive exact-byte request sidecar：Rust 的确定性纯 Responses codec 从完整重验的 frozen logical request 生成并复验 body，SQLite v15 在不改变 Run v1/seq-1/main journal 的情况下原子保存。它不进入 legacy dispatch discovery，也不收集 consent、读取 credential、构造 provider/transport、联网、访问 workspace 或释放任何 authority；默认 view 隐藏 request，只有显式 reveal 返回 exact bytes。legacy `dispatch execute` 也先完成 source/consent/readiness 预检，成功后才允许启动 operator-pinned Core bridge，因此 scheduled-only Run 无法借旧入口触发本地进程或外部效果。
> Sprint 57 以 ADR 0028 为该 scheduled request 增加 effect-free Rust→Go→Rust release handshake：Rust 从完整重验的 current v15 aggregate 与 exact provider codec bytes 导出 private canonical control，Go 独立重建 schedule/candidate/request 及全部 lane/budget/failure binding 并生成 content-addressed authorization，Rust 再对 fresh state 严格验证。decision 只描述未来 exact lifecycle admission + execution/dispatch release；当前 Run 仍为 pristine v1/seq-1，schema v15 与数据库不变，且没有 consent、credential、provider、network、lane、progress、receipt、successor 或 writeback effect。
> Sprint 58 以 ADR 0029 复用 ADR 0023 的 source-neutral Go canonical pricing snapshot，为 scheduled ordinal-zero request 增加 effect-free destination/pricing readiness。Rust 先从 current v15 aggregate 重验 exact scheduled authorization，再以独立 scheduled registry port 只解析官方 destination 的 quote，并用 Domain 共享的 checked、component-rounded integer 算法核对 frozen budget；registry 不返回 build-capable provider readiness。CLI 在 Hub open 前对 authorization/pricing 双工件作有界 UTF-8 exact decode，成功只输出脱敏 point-in-time metadata。schema、Run、journal 和 sidecar 不变，且无 consent/credential/provider/network/workspace/tool/lane/send/result/receipt/writeback/successor effect；未来 effectful slice 必须把 fresh preflight、共享全局 lane claim、单次 send 与 terminal evidence/release 作为不可拆分协议。
>
> Sprint 59 以 ADR 0030 实现 scheduled ordinal-zero 的独立 effectful sidecar。Rust 在 fresh authorization/pricing、registered destination、header-safe credential 与 pinned Go Core handshake 通过后，才以 SQLite v16 immediate transaction 原子 claim exact prepared body 与全局 Project lane；`provider_request_sha256`（prepared envelope）和 `request_body_sha256`（实际 body）始终分开绑定。bounded provider evidence 形成 result/uncertainty artifact，scheduled terminal control 交给 Go Core 产生一个中间 receipt，第二个事务原子保存 artifact/control/receipt 并释放 lane；Core/commit failure 只保留 artifact-only quarantine，禁止 resend/retry/resume/lease/health/successor。scheduled Run 仍是 v1/seq-1，legacy lifecycle discovery 被拒绝；多节点 successor、predecessor dataflow 与 hard-crash adjudication 仍需独立协议。
>
> **Gateway · 完整 Knowledge-Engine · Web-UI 仍为路线图；Go Docker/Firecracker runner 已落地，完整 coding-workspace 交换仍待后续。**

## AI Engineering OS 治理知识层（ADR 0045–0066 的限定切片已交付）

ADR 0037 将下一层组织冻结为「生命周期决策节点 × 可复用 Capability/Skill × 显式 CapabilityGrant」：Agent
instance 只是一次 Run 中的临时装配，不因角色名自动取得权限。目标 Governance Kernel 先统一
Evidence/Claim/Context/Permission/Transition，再建设可重建 System Knowledge Graph、Change Impact/Cost/Risk、
ADR/Technical Debt/Software Health，最后把 `.agent`、`docs/reviews` 与 `docs/ai-batch` 适配到同一能力语义。

ADR 0038 进一步把 AADM 冻结为节点内部的 Decision Kernel ABI（Atom/Transaction/Capability/Rule Field/
DiscretionEnvelope/rolling controller），并在 Node16 加入 evidence-first R0–R2 Meta Reflection；ADR 0039 把
ExecutionTarget/Attempt/Artifact/Lease 定义成独立、默认 OFF 的 Device Fabric。Decision Kernel 只决定做什么/是否允许，
Execution Fabric 只执行获准 TaskSpec，Evidence/Verifier 和 Governance Kernel 独立裁决结果与学习，避免同一 Agent 自决自验。

ADR 0045 已实现严格 EvidenceRecord/KnowledgeClaim v1、跨 Go/Rust/Python canonical golden 与 universal shadow checker；它只验证
候选记录的字节、摘要、状态和引用，不认证身份、不形成 durable truth、不授权、不推进 lifecycle。ADR 0046 冻结本地
GovernanceRecordJournal v1：只原子追加 exact v1 bytes、返回 `stored|exact_replay`、默认读取 metadata，并维护可重建的
`structural_sequence_only` head；引用闭包最多 1,024 dependency records、16,777,216 candidate+closure bytes 和 256 derivation edges，三者只作
resource-exhaustion admissibility。Rust domain/application/store、SQLite v25、CLI、migration/compatibility 与对抗门禁已完成，并经独立复审和
`forge accept` 验收；scaffold 只继承治理资产，不安装 `forge-runtime`，缺兼容 binary 时必须记 `not_executed`。该完成状态不扩张
contract 边界，structural head 仍不表示 truth、authority、freshness、conflict resolution 或 current knowledge。ADR 0054 在其上增加
SQLite v27 可重建 declared semantic view：显式 caller-time temporal labels、authority-free lifecycle subset、conflict candidates 与
Assumption/Hypothesis validation schedules；exact records 仍是重算源，结果固定无 truth/authority，不选择 winner、执行 validation 或推进权威状态。
完整 00–16 节点 SOP、工程模式适用条件、治理数据契约和分阶段验收见
[`docs/design/ai-engineering-os/`](../docs/design/ai-engineering-os/README.md)。该目录和其中
`capability-catalog.v1.yml`、AADM/Reflection/Device 文档当前都是 `planning_only`。ADR 0055 另交付 strict `ContextPackage v1` 的
authority-free pure builder：它只消费 caller-supplied exact request，确定性产生 typed lanes、omission/redaction/token/digest/cache receipts，
不读取仓库、不调用 provider、不持久化且不授予 instruction/permission/approval/effect。ADR 0056 又冻结 `CapabilityGrant v1` contract-only
wire、最小 effect/scope vocabulary 与 declared-relation assessment；其 pure evaluator 仍不认证 authority。ADR 0057 只补一个 Catalyst-only
`authenticated_bootstrap_repo_read_grant_issuance_v1`：operator 部署的独立非 Agent `forge-kernel` 以 repo 外 pinned root/key/state 认证 signed policy/request，
只为 `bootstrap_planning` 的 `repository-reader/v1` + `repo.read` exact-path、小预算/TTL、`local|development|test` 签发 signed Grant 与 durable signed receipt。
ADR 0058 已交付的 Catalyst-only `authenticated_bootstrap_repo_read_execution_v1` 再以独立 execution root、signed policy/invocation 与 Linux-only `openat2`
把该 Grant 限定为 manifest-bound exact-byte single-use read；reservation/intent/terminal durable ledger 不保存 raw，重放 receipt-only 且禁止 reread。
它不提供 hard timeout、管理员 state rollback resistance、secure erasure/process isolation/HSM 或 general PDP authority。
ADR 0059 再交付 contract-only `ApprovalRecord v1` 的 strict record/target/request/assessment、detached-proof identity 和 Grant ref
projection；它从不导入本地 marker/flag/actor hint，不认证 approver/authority/proof/SoD，不验证 condition/RiskAcceptance/revocation，且不产生
effective approval、authorization、permission、persistence、transition 或 effect。它的 Accepted 状态只表示合同切片通过 `forge accept`，不扩大 runtime authority。
ADR 0060 再交付 contract-only `TransitionReceipt v1` 的 closed state graph、receipt/target/request/assessment identity、显式 predecessor、
applicability/rework/resume 声明及 Grant/Approval reference comparison。Listed edge、PASS/NA、reference match 或 caller-time continuity 都不认证
controller/current state/precondition/waiver，不授权或执行 transition，不 append ledger、持久化、完成任务或产生 effect；其 Accepted 状态只表示合同切片通过 `forge accept`，不扩大 runtime authority。
ADR 0061 又交付 contract-only `KnowledgeUpdateProposal v1` 的 proposal/target/request/assessment identity、exact Evidence/Claim closure、
create/supersede declarations 与 Grant/Context/artifact declared compatibility。它不认证 proposer/Grant/Context/Evidence，不读取 authoritative current
head、不仲裁 conflict/freshness/policy，不产生 truth/adoption/authorization/permission/persistence/apply/receipt/execution/effect；其 Accepted 状态只表示
合同切片通过 `forge accept`，不扩大 Knowledge/runtime authority，scaffold 也不安装任何 Knowledge runtime/state/key。
ADR 0062 又交付 exact ADR-0053 graph 上的 authority-free Local Go Package ImpactPreScan：changed paths 严格分区为 resolved/unresolved seeds，
只沿 local import edges 计算反向闭包、完整 induced edge set 与 deterministic shortest witness。package graph 缺口显式 UNKNOWN，system impact 恒为
UNKNOWN；它不读取 live repo、不调用 producer、不形成 selected build/System Knowledge Graph/final Impact/Cost/Risk/Assessment，也不持久化或授权。
ADR 0063 再把 caller-declared L3/L4 Build 的 `reviewer_v1` 变成串行、不可跳过、exact-final-line、失败回 implementer 的
fail-closed orchestration boundary；L0–L2 与 `materiality_not_bound` 保持既有 advisory/fail-open 兼容。materiality 不被自动推断或
认证；Reviewer/模型/provider 身份、review 质量、cryptographic SoD 及 source/context/policy/artifact digest binding 均不在本切片。
旧 runtime 无法识别新 contract 时必须失败关闭；checkpoint/chain 只提供 crash consistency，same-UID/admin 可替换或回滚 state，
不得冒充 authenticated/tamper-proof resume。
ADR 0064 已交付 `local_digest_v1`：七个 canonical workflow 的 accepted command output、L3/L4 Build `reviewer_v2` 和
Design/Deploy/Rollback positive approval 均通过 runtime-computed product-source、exact prebinding prompt-context、完整 effective
local policy 与 declared artifact manifests 形成 challenge-bearing receipt/context/marker，并由 checkpoint/chain v5 防止恢复降级。
这些 same-UID local records 不是 ADR-0059 ApprovalRecord、身份/SoD、signed PDP/Grant、truth 或 effect authority，也不提供原子
repository snapshot 或管理员 rollback resistance。ADR 0065 又交付 authority-free GraphSnapshot v1 foundation 及 exact ADR-0053
module/package lexical PARTIAL/UNKNOWN profile；ADR 0066 再以独立 transport/profile 加入 package-scoped lexical test source-set node 与
module→test edge，保持旧 API/Schema/golden 不变，并令 Go/test coverage 形成互斥 PARTIAL partition。test source set 不表示 test case、
execution、result、coverage 或 verification；两个 profile 都没有 live capture、完整 multi-surface graph、Impact/Cost/Risk、G3 或 Assessment authority。
该 runtime 仅支持 Unix；authority/state directory exact `0700`，封闭相对 leaf exact `0600`/euid-owned/single-link/无特殊权限位；authority/repository resolved endpoint 按 ancestor filesystem identity 双向不重叠，repository source/resolution/opened identity 全 session 绑定；非 Unix 在读取 authority 材料前失败关闭。这些文件/effective-UID 约束不是 OS principal/HSM 隔离；ledger high-water 也只相对当前 signed snapshot，V1 无 external monotonic anchor，
不抵抗可替换 state 的本地管理员回放旧 snapshot。Scaffold 不安装 runtime/root/key/state，`forge accept` 也不是 issuer。完整 Knowledge-Engine、
plan-finalization/authenticated Approval/revocation/usage/pre/postflight/PEP/effect、真实 Context Router/prompt compiler、authority-bearing Knowledge apply/receipt、完整 Governance Kernel/PDP、Meta Reflection、planning ownership 的 physical Skill/adapter generation 与 authority-bearing CapabilityInvocation Registry
与远程 Device Fabric 仍未实现，不得从 pure package、单一签发 profile、文档存在或已有本地 runner 推断为已接线。

## 模型路由 (v1 限 Claude 档)
classify → score(复杂度/风险/依赖/安全/上下文) → tier(mode,lifecycle)
→ ⚠ risk≥critical 强制 ≥Opus → 💰 预算守卫 → 📈 历史择优(来自 Eval 记分卡)。
档位:Haiku=文档/CRUD/测试 · Sonnet=常规实现 · Opus=架构/安全 + 所有 Reviewer。跨厂商池 = v3。

ADR-0075 保持 Proposed；Registry v30 只增加 Portable Knowledge Graph Curation 的 source-only delivery/ref/manifest pin。ADR-0065/0066 wire、Schema、golden、projector semantics 与 runtime scope 不变；package 不增加 wrapper/union/dispatcher、live capture、route、persistence、test outcome/coverage/verification、impact 或 authority。

ADR-0076 保持 Proposed；Registry v31 只增加 Portable Change Impact Cost Risk lexical prescan 的 source-only delivery/ref/manifest pin。ADR-0053/0062 wire、Schema、golden、lexical closure semantics 与 runtime scope 不变；package 不增加 raw graph/wrapper/dispatcher、live capture、route、persistence、完整 Impact/Cost/Risk、materiality/safety 或 authority。

ADR-0078 WorkIntent v1 Proposed candidate governance 只把 ADR-0077 的 exact Python/Go/Rust parity 记录为 Registry v32 candidate metadata，并接入 checker-only shadow 与 source-only Python distribution 边界。Scope arrays byte-semantics 不变，Go/Rust 保持 Catalyst-only，WorkIntent 不进入 context route、evaluator、producer、consumer 或 runtime；该 Proposed candidate 不接受 semantic authority、不关闭 G0，也不创建 Run/RunJournal/lifecycle、Approval/Grant、persistence 或 effect。

ADR-0080 Authenticated ADR approval v1 Proposed candidate governance 只把 ADR-0079 caller-supplied structure/digests/relations 记录为 Registry v33 candidate metadata、checker-only shadow 与 dependency-free Python source-only distribution。完整 scope mapping 的 canonical SHA-256 保持 `8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c`；无 Ed25519 verification、authentication、authorization、receipt issuance、external-root/time/revocation currentness、CAS/durability、Accepted lifecycle、G0 closure、route/runtime/evaluator/producer、persistence 或 effect，且不复制 future Go service、keys/state。

ADR-0083 Authenticated ADR lifecycle v1 Proposed candidate governance 把 ADR-0081 的已审核 Go approval authority 明确记录为 Catalyst-only evidence，同时只分发 ADR-0082 的 Python structural core/schema/golden/fixtures。Registry v34 的完整 scope mapping SHA-256 仍为 `8ba82b638e8031f0d1be2b9ea6d522a4b9cf064a4ed532e1f0d3281f2dfe874c`；一个 lifecycle checker-only shadow 不构成 route/runtime。生成项目不含 Go authority、production key/state，structural lifecycle state 不构成 source mutation、Accepted ADR、compliance、permission 或 effect。

ADR-0085 lifecycle authority evidence 将 ADR-0084 exact44 Go authority 记录为 Catalyst-repository-only evidence，并以 Registry v35 保持相同 scope hash 与既有 checker-only shadow。Source-only exact4 只含 ADR-0084、ADR-0085 和 governance module/test；Go contract/authority、trust material、state、route、Skill 与 runtime 均不分发。

### Legacy governance read-only import boundary

ADR-0086/0087 仅把 caller-supplied exact Memory/ADR bytes 投影为 `unverified_legacy` read-only view。Registry v36 scope digest 不变，checker-only shadow 的 argv 只含 Python 与 checker 路径；detector schema 不伪造 stdin，operator 负责 pipe request/EOF。Python source 可分发，Catalyst-only Go parity、ambient path reader、database、state 与 authority 均不分发。

ADR-0088/0089 仅验证 caller-supplied exact Kernel operational reference records 与 acyclic closure。Registry v37 scope digest 不变；checker-only shadow 只对 pinned golden 执行精确 argv，exact18 source distribution 只含 dependency-free Python。Catalyst exact11 Go/exact13 Rust module parity 与共享 `lib.rs` 中唯一 operational registration 均不进入生成项目；`lib.rs` 不整文件 pin。route、Skill、authority、effect 和完整 Kernel ABI 同样不进入生成项目。

ADR-0090/0091 在 Registry v38 登记已交付的 structural reference-family repository slice：CognitiveAtom v2、DecisionTransaction v1 与 operational records 的单向 closure 仅接受 caller-supplied exact bytes；两份 ADR 继续保持 Proposed。完整 scope digest 不变；exact19 source distribution 只含 Python core/governance，Catalyst exact13 Go、flat exact9 Rust 和共享 `lib.rs` registration 不进入生成项目。声明的 source/authority/hardness/binding 均未认证，instruction disabled 且 22 项 attestations 全 false；无 Skill、route、runtime、PDP/controller。该窄 roadmap 项已通过正式 `forge accept` 并完成；ADR-0038 仍为 ADOPTED-PARTIAL，DecisionCapsule、AuthorizedTransactionSpec 与 authority-bearing lifecycle 均未交付。

ADR-0092/0093 在 Registry v39 只登记待验收的 Decision Capsule structural replay repository Candidate：四对象 DAG 对 caller-supplied ADR-0090 closure 做 pure validate/reseal/complete-manifest/compare；专用的后挂载 ReflectionReport refs 仅位于 outer closure 且保持 unresolved，上游 ArtifactRefs 则保持 opaque/uninterpreted。Scope digest 不变；exact19 仅分发 Python，Catalyst exact15 Go、exact14 Rust 和共享 registration 均不分发。32 项 attestations、replay controls 与七项 completion 全 false；无 Skill、route、runtime、model/rule/history/Reflection consumer、persistence、PDP/controller。两份 ADR 始终 Proposed/null，窄项待正式 `forge accept`；ADR-0038、完整 DecisionCapsule 与 AuthorizedTransactionSpec 保持开放。
