# ForgeOS

**The operating system for AI-native software engineering.**

ForgeOS 不替代 Claude Code / Codex / Gemini CLI / OpenCode / OpenHands —— 它站在它们之上,
提供统一的工程治理:需求探索、架构推导、模型调度、上下文与约束执法,让 AI 能长期(24h)
自治推进「Idea → operator-gated production handoff」,而不写出「上帝文件」、不让架构腐化。
真实云/K8s 变更由带外 CI/operator 执行，不由 ForgeOS agent 直接完成。

- 设计与决策的唯一事实源:[`.agent/`](.agent/)
- 工程红线(本仓库自身也遵守):[`.agent/AGENTS.md`](.agent/AGENTS.md)
- 完整验收真相源:`node harness/acceptance.mjs`
- 快速编辑信号:`node harness/gate.mjs`
- Go 编排/链状态/审批控制面:[`forge-core/`](forge-core/)
- Rust 本地会话 Hub、durable Project Run 与默认离线/显式 live Agent Runtime:
  [`forge-runtime/`](forge-runtime/)
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
Graph 首节点还可通过 Go 生成 immutable `operator_asserted` pricing snapshot，
再由 Rust `group graph run dispatch readiness verify` 把当前 release authorization、
固定官方 destination、exact pricing bytes 与 frozen cost budget 合并复验。该命令
保持 current SQLite v15/Run v3 不变，不读凭证、不构造 provider、不 claim/send/result/advance；
定价也不带 vendor attestation，不代表实时厂商价格或账单保证。
多节点 Graph 可把同一份 exact private control 交给 Go
`graph-execution-schedule`，冻结 serial authored-order、Project lane identity、
直接前驱 receipt slots 与 fail-fast policy；Rust `group graph run schedule admit/show/list`
只在 SQLite v15 保存 immutable sidecar，不占用 Graph 主 journal seq，也不创建 contract、
观察 progress、推进 successor 或接触 credential/provider/network/workspace/tool/result。
它是后续多节点协议的可验证策略前置条件，不代表 frontend/backend/SSO 已经执行。
已冻结的 scheduled initial-node request 可通过 Rust
`scheduled-contract provider-request release-control export` 导出私有 canonical control，
交给 Go `graph-scheduled-node-dispatch-authorize --control FILE|-` 生成 content-addressed
decision，再由 Rust `scheduled-contract provider-request authorization verify` 对 fresh Hub
state 复验。control/authorization 含 Prompt、Project、provider 与 request binding，应只走受保护的
pipe/file；verify 输出会脱敏。该握手只授权未来的 exact lifecycle admission、execution-authority
release 与 dispatch-authority release，
当前仍未 admission、consent、claim、send、记录 receipt 或推进 successor。
同一 request 还可通过 `scheduled-contract provider-request readiness verify` 同时提交
exact authorization 与既有 Go `graph-node-pricing-snapshot` 工件；Rust 会重新读取 current
v15 Hub，复验官方 registered destination、逐项向上取整的整数成本上界与 frozen budget。
结果只含脱敏元数据，pricing 仍是 operator assertion、没有 vendor attestation；命令不缓存
readiness，也不读取 credential、构造 provider、联网、claim lane、send、落库或推进 successor。
未来 effectful dispatch 仍必须在 fresh consent 后紧邻原子 claim 重做全部检查。
对严格单节点 Graph，`group graph run dispatch execute` 再要求本次 fresh consent、
exact authorization/pricing 与 SHA-256 固定的 Go Core binary；该 effectful 命令当前
仅支持 Linux，并从密封、复验后的匿名 executable memfd 执行 Core。SQLite v15 沿用
v12 lifecycle 表原子 claim
全 Hub Project lane，只在 approved service path 把 non-`Clone` exact request authority
交给一个赢家（可信 store adapter 属于进程内 TCB）；一次派发后 bounded
收集 result/uncertainty，由 Go Core 生成 terminal receipt，最终事务同时持久化证据、
追加 seq 5 并释放 lane。claim 后崩溃或 Core/commit 失败会保留 v4 quarantine 且禁止
自动重发；当前协议不执行 frontend/backend/SSO 等多节点 Graph。claim、result 或
bounded partial output、artifact 与 receipt 都以本地 SQLite plaintext 保存；默认输出
只含 metadata，只有 `--include-result` 才揭示完整复验后的结果。fresh consent 仅授权
这一份 exact request，不授权 workspace/tool、Conversation/Prompt/memory/task writeback、
其他 node 或 retry；它只保证 Hub-local single-consumption，不声称 remote exactly-once。
`group analysis prepare GROUP_RUN_ID` 可在本地冻结一份精确、零工具的
OpenAI Responses 请求；只有后续 `group analysis send ANALYSIS_ID
--confirm-off-machine` 才读取环境凭证并释放一次外发。SQLite claim 一旦提交，
任何超时、崩溃或落库失败都保持 `dispatch_unknown` 且禁止自动重发；结果正文
默认隐藏，仅 `show/send --include-result` 显式显示已验证的单模型结果。

> 目录名暂为 `catalyst`,产品名 **ForgeOS**(是否改名见 `.agent/DECISIONS.md`)。
