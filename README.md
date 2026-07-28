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

> 目录名暂为 `catalyst`,产品名 **ForgeOS**(是否改名见 `.agent/DECISIONS.md`)。
