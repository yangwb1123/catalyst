# ForgeOS

**The operating system for AI-native software engineering.**

ForgeOS 不替代 Claude Code / Codex / Gemini CLI / OpenCode / OpenHands —— 它站在它们之上,
提供统一的工程治理:需求探索、架构推导、模型调度、上下文与约束执法,让 AI 能长期(24h)
自治推进「Idea → operator-gated production handoff」,而不写出「上帝文件」、不让架构腐化。
真实云/K8s 变更由带外 CI/operator 执行，不由 ForgeOS agent 直接完成。

- 设计与决策的唯一事实源:[`.agent/`](.agent/)
- 工程红线(本仓库自身也遵守):[`.agent/AGENTS.md`](.agent/AGENTS.md)
- 完整验收真相源:`node harness/acceptance.mjs`。正式验收在 Linux 上采用
  descriptor-relative no-follow 遍历与覆盖整个探测窗口的独立文件事件日志；
  要求初始 user namespace 与受信本地文件系统，无法提供这些保证时会
  fail closed，不以路径级快照回退宣称正式通过
- 快速编辑信号:`node harness/gate.mjs`
- Go 编排/链状态/审批控制面:[`forge-core/`](forge-core/)
- Go R0 App Server：`forge-server --state-dir /absolute/private/dedicated-leaf`；首次启动创建 identity-bound
  专用目录，并在 instance lock 后、listener/readiness 前初始化 descriptor-bound private SQLite `control.db`
  v1。当前公开面仍只有 literal-loopback `GET|HEAD /api/v1/health`，尚无产品 Command/Query API、Runtime/
  Harness transport、projection 或 Web UI；只允许 `GOOS=linux`（排除 Android）启动，其他目标在 state access
  前失败关闭。Go 内部 Workspace Catalog v1 已开始提供 event-backed Space/Project/declared Snapshot-reference
  create/get/list，但不读取 Project path、没有认证或公开 route。私有存储与 catalog 契约见
  [`control-store-v1.md`](docs/design/forge-workspace/control-store-v1.md) 与
  [`workspace-catalog-v1.md`](docs/design/forge-workspace/workspace-catalog-v1.md)
- Go 内部 Delivery Domain 与 pure pre-effect Reconciler 已提供 bounded Objective/Change/WorkGraph 校验和一个确定性
  passive WorkItem candidate；Policy/Approval/Budget 仍是 declared-only，`ReadyWorkItem` 不授权状态迁移或执行。
  当前仍无产品 route、Runtime/Harness bridge 或 completion join；边界见
  [`delivery-domain-v1.md`](docs/design/forge-workspace/delivery-domain-v1.md) 与
  [`reconciler-v1.md`](docs/design/forge-workspace/reconciler-v1.md)
- Proposed Platform Core Envelope/Receipt v1 已冻结 typed ID、Scope/Actor/Record reference、`ArtifactRef`、
  `CommandEnvelope`、`EventEnvelope`、Execution/Verification Receipt、WorkItem/Attempt/Action 状态边及
  broad rejection code 的 Go/Rust/Python 严格合同；Go `controlstore` 现只消费 canonical Command/Event bytes
  并持久化 journal/version/idempotency/outbox/inbox，Receipt/state binding 仍不生成或持久化回执、不执行检查、
  不推进领域状态，也未接入 Runtime transport 或 UI。可分别用
  `python3 -I -B harness/platform_core_contract/check.py --golden .` 和 `--receipt-golden .` 独立复验
  共享 golden；协议见 [`docs/contracts/platform-core-envelope-v1.md`](docs/contracts/platform-core-envelope-v1.md)
  与 [`docs/contracts/platform-core-receipt-v1.md`](docs/contracts/platform-core-receipt-v1.md)
- Rust 本地会话 Hub、durable Project Run 与默认离线/显式 live Agent Runtime:
  [`forge-runtime/`](forge-runtime/)
- 已采纳的企业级 AI Engineering OS 目标规划（00–16 节点、AADM/Meta Reflection、140→38 Capability/Skill
  ownership、default-off Device Fabric）:
  [`docs/design/ai-engineering-os/`](docs/design/ai-engineering-os/)（`planning_only`，不代表 runtime 已实现）
- 生产交付边界:[`docs/release/README.md`](docs/release/README.md)（只生成/验证交付包；
  不访问云/K8s 凭证，不执行远程部署）

主链为 Discover → Design → Review → Build → Deploy → Evolve；Rollback 是独立恢复入口。

`forge-runtime` CLI 不带路径进入 Global Space，带裸路径或 `-C PATH`
进入 Project Space；若项目目录名与 `group`、`prompt`、`run`、`session`
等命令重名，请使用 `./run` 或显式 `-C` 消除歧义。
`group context GROUP_ID` 可在本地原子预览 Group discussion 与成员 Project
的有界 Prompt dossier；默认只显示来源、摘要和容量，显式
`--include-content` 才显示正文，且从不读取项目文件。
`group run prepare GROUP_ID` 会把同一份有界 dossier 原子冻结为可跨进程
幂等重放的本地 prepared snapshot；`show/list` 可检查它，但不会启动模型、
provider、工具或 workspace，也不代表已经完成分析或讨论。
`group execution start GROUP_RUN_ID` 再把一个已验证 snapshot 绑定到独立、
可恢复的本地 execution receipt。这个首切片只验证冻结输入并持久化证据：
不调用模型/provider，不读取 workspace，不开放工具或网络，也不产出分析、
讨论或任务结论。
中断的 Project Run 可由调用者显式执行
`forge-runtime -C PATH run resume RUN_ID` 继续已验证的 durable prefix；已提交的
工具 effect 不会自动重放，未决外部 effect 会拒绝恢复。
仅对 `RunOutcome::Completed` 的已完成终态 Run，同一命令会在 Project 绑定后幂等补齐
assistant Conversation 写回，并在 credential/provider/tool/history 初始化前完成，不读取
workspace 内容；`RunOutcome::Failed`、`RunOutcome::Cancelled` 与
`RunOutcome::LimitExceeded` 终态仍拒绝 resume。
只读的 `forge-runtime --json run explain RUN_ID` 可从同一 durable journal 查询事件证据、
可见上下文边界、显式读能力和安全 continuation；它不会把未快照的历史、外部 effect
或认证 Grant/Approval 推断成事实。
终态 Project Run 可通过
`forge-runtime --idempotency-key KEY -C PATH run restart SOURCE_RUN_ID`
准备成一个新的独立 Run：沿用已持久化的 Prompt 与执行配置，只写入一个确定性 seed，
不复制旧答案或 journal，不触发 provider/tool/workspace/network；随后仍需显式
`run resume NEW_RUN_ID` 才会执行。source 指纹与 key 共同绑定幂等身份，晚到重试会返回
目标 Run 的真实当前状态。
如需保留可查询的来源关系，可对终态 Project Run 执行
`forge-runtime --idempotency-key KEY -C PATH run branch PARENT_RUN_ID`：Hub 会原子创建
child Run、immutable direct-parent lineage 和单个 fresh `run_started`。child 沿用
Project、Conversation、user Prompt 与 exact execution configuration，不复制 parent
journal suffix、result、answer 或 tool events，仍由显式 `run resume CHILD_RUN_ID` 开始执行。
`forge-runtime --json run lineage CHILD_RUN_ID` 以 content-free 只读视图返回
`root_input` 的 direct parent 与 source seq 1；context/workspace snapshot 均未绑定。
Graph 协议当前使用 SQLite v24。legacy 首节点 authorization 路径使用 main Run v3；scheduled
多节点链则刻意保持 main Run v1/seq-1，并把逐节点状态保存在独立 sidecar。Go Core 负责冻结
canonical plan、serial schedule、per-node contract/request/authorization/pricing 与 terminal
receipt；Rust Hub 对同一份 exact bytes 做 fresh-state 复验并持久化 per-node lifecycle。非初始
节点携带的直接前驱 evidence 只接受 exact durable `completed`/result 型 receipt；空直接前驱的
显式目标可携带零 receipt。显式 `--predecessor-content` 会把有界正文纳入 successor Prompt，
并在 admission 时持久化复验；后续 off-machine dispatch 仍需独立的
`--confirm-predecessor-content` 授权。wave-ready/admit、逐节点单次 provider dispatch 与本地 hard-crash
adjudication 已具备，所有 effectful dispatch 仍须 fresh consent、固定 Core binary、Project lane
单赢家和 bounded terminal result。

这套 Graph 协议尚未提供顶层“自动跑完整张图”的循环、任意 event-prefix branching、
automatic recovery 或远程 exactly-once；
Graph 的进度仍由 operator/外层编排逐节点驱动。Prompt、前驱正文、result 与 receipt 会以
本地 SQLite plaintext 保存，默认 CLI 视图隐藏正文；consent 只授权当前 exact request，
不扩大到 workspace/tool、其他节点或自动 retry。`operator_asserted` pricing 也不代表实时
厂商报价、账单保证或 vendor attestation。

`group analysis prepare GROUP_RUN_ID` 可在本地冻结一份精确、零工具的
OpenAI Responses 请求；只有后续 `group analysis send ANALYSIS_ID
--confirm-off-machine` 才读取环境凭证并释放一次外发。SQLite claim 一旦提交，
任何超时、崩溃或落库失败都保持 `dispatch_unknown` 且禁止自动重发；结果正文
默认隐藏，仅 `show/send --include-result` 显式显示已验证的单模型结果。

> 目录名暂为 `catalyst`,产品名 **ForgeOS**(是否改名见 `.agent/DECISIONS.md`)。
