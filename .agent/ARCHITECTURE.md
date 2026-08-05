# ForgeOS — Architecture

> 本文 = **当前/脊柱**(v0–v1 在 Claude Code 上的形态)。
> **目标架构(分布式 HA 微服务全貌)** 见 [`architecture/north-star.md`](architecture/north-star.md) + [`architecture/ha-security-rollout.md`](architecture/ha-security-rollout.md)。

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
> **v2 现状**:forge-core Go 运行时当前 **32 包**（31 个 internal + `cmd/forge`，纯标准库零依赖；仅含测试的 `internal/adr` 不计入运行时包）,已落地 5 个核心引擎与可工作的本地 Agent-Runtime 切片:
> **Orchestrator**(`internal/orchestrator`)· **Model-Router**(`routing`)· **Context-Engine**(`prompt`)·
> **Memory-Engine**(`memory`)· **Evaluation-Engine**(`converge`);Agent-Runtime 已具备本地命令执行、预算/超时/进程组、最小环境、stdin prompt、产物溯源与 run lock。`forge run --chain` 以版本化状态跨 Discover→Design→Review→Build→Deploy→Evolve 持久恢复，拒绝/cycle/max-stage/策略 halt 均失败关闭。
> `forge-runtime/` 现有 Rust 原生多轮模型/工具循环、SQLite local-first Conversation Hub 与 durable Project Run：无路径 Global、有路径 Project、Group 联动；execution-bound Project Run、append-only event journal、O(1) 增量语义 cursor、同快照 inspection、严格有界 causal user/assistant history、Run 原子授权 assistant 写回、terminal/incomplete/pending-tool 判定均跨进程持久化。Group dossier 可被原子冻结为独立 prepared Group Run，幂等重放精确旧快照且不查询最新历史；独立 Group Execution 能纯本地验证冻结输入并恢复 content-free integrity receipt。其后的 two-phase Group analysis 在 SQLite v5 先原子准备 exact、零工具、`store:false` 请求，再经当次显式同意、claim 前凭证/目标预检和单赢家 authority 至多外发一次；claim 后不自动重发，只有完整 provider terminal 能原子提交结果，默认输出隐藏正文。SQLite v6 又能把同一 frozen source 的 2–8 份 completed 分析按声明顺序冻结为本地 canonical panel，同 key 精确重放并在 show 时重验所有来源，默认不显示结果正文；这仍只是并排组装。SQLite v7 再以独立 consent/claim/result journal 对一个 exact panel 做单模型综合：唯一 user message 是 canonical panel manifest，不重发原始 dossier，单赢家外发且 uncertainty 不自动重试，固定本地 artifact/no-writeback，并明确不冒充讨论、共识或事实验证。SQLite v8 进一步持久化 exact Group Run 上的 manager 指令、frozen member task assignments、dependency edges 与 deterministic waves，作为 `forge-core` 唯一调度器和 Rust 单任务 Agent Loop 之间的 immutable interchange artifact；SQLite v9 被动接收 Go `forge graph-plan` 生成的 canonical Core Plan并冻结 `awaiting_execution_contract` Run；SQLite v10 再由 Rust 导出 exact private control snapshot、Go 唯一选择 `plan.waves[0][0]` 并生成 canonical Node Execution Contract、Rust 以 seq/head CAS 登记唯一契约和第二事件，把 Run 推到 `awaiting_core_dispatch`；SQLite v11 使用现有 Responses 纯 codec 固定 exact provider body 与 content-addressed Node Dispatch Request，再以 seq-2/head CAS 追加第三事件并停在 `awaiting_dispatch_authorization`。Go 仍是唯一调度 owner；v11 准备链不释放 authority，也不把 topology waves、契约或 request presence 冒充执行。SQLite v12 只对严格单节点/单 wave/零 edge Graph 开放完整 effectful 生命周期：seq-3/head 与 Hub-global Project lane 原子 claim 后一次消费 exact body，bounded 收集 terminal/EOF 或 uncertainty，Go Core 从真实 v4 control 产生 terminal receipt，Rust 再原子保存 artifact/receipt、追加 seq 5、释放 lane 并进入 completed/failed/failed_uncertain。默认 deterministic/offline；显式 Project Run `--live` 默认零工具，仅 exact `--allow-read` 授权，并启用固定 HTTPS origin、无 redirect/隐式 retry、`store:false` 完整 validated output-item 回放、phase-aware final projection、terminal status/item identity 校验及 transport/SSE/token/output 全套上限；incomplete 永不释放工具。SQLite open→PRAGMA/WAL→schema 有统一 5 秒重试，DB/WAL/SHM 私有权限及 workspace capability 失败有并发/反例回归。多节点 successor/dataflow、hard-crash no-send adjudication、resume/branching、远程账号与同步、共享 ACL/Group 多 Agent discussion、审批、写/进程工具及 OS 沙箱仍未实现。
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
> **Gateway · 完整 Knowledge-Engine · Sandbox runner(载重墙)· Web-UI 仍为路线图。**

## 模型路由 (v1 限 Claude 档)
classify → score(复杂度/风险/依赖/安全/上下文) → tier(mode,lifecycle)
→ ⚠ risk≥critical 强制 ≥Opus → 💰 预算守卫 → 📈 历史择优(来自 Eval 记分卡)。
档位:Haiku=文档/CRUD/测试 · Sonnet=常规实现 · Opus=架构/安全 + 所有 Reviewer。跨厂商池 = v3。
