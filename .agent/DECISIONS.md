# ForgeOS — Decisions

## 已锁定(默认,可推翻)
- **D1 技术栈**:**目标 = Go-核心 polyglot**(`forge-core`=Go 编排/调度/路由/workflow · `forge-ai`=Python 智能层 · `forge-runtime`=Rust Agent Loop + Conversation/Run 持久化 + 沙箱边界 · `forge-web`=TS/Next UI)。**时序**:v0–v1 不写 forge-core——编排=Claude Code 本体,只写声明式 agent 卡/workflow/policy + 薄胶水(`gate.mjs` 现用 Node,够用)+ polyglot harness 适配器。**Go 已在 v2 进入**(见 D6);用户于 2026-07-27 提前授权 Rust 的离线 Agent Loop(ADR 0006)及 local-first Conversation Hub(ADR 0007)首切片。SQLite Project/Conversation/Group/Prompt ledger 已落地；真实 provider、自动历史回放/Run 恢复、远程账号同步、写工具与生产沙箱仍按后续阶段推进。Temporal/Postgres/NATS 仍未起。harness CLI 未来固化为 **Go 静态二进制**(目标仓零运行时依赖)。
- **D6 v2 forge-core 启动 (2026-06-19)**:核心循环已在 CC 上经 `examples/url-shortener` dogfood 端到端验证稳定(architect→3 implementer→fresh reviewer→fix,被 `forge accept` 实际 gate)→ 触发 ADR-0001 的取代条件,**正式开建自研 Go 运行时 `forge-core`**。启动时 7 包；当前为纯 Go 标准库、**零外部依赖**的 29 包，CLI 已扩展到 run/evolve/init/trace/approve/reject/preflight/doctor/gate/check/accept 等，含 durable chain、产物/审批/拒绝契约与多语言验收。gate 阶段执行真实 harness；路由带硬 Opus 底线；`forge evolve` 的收敛由 live signals 实算。**诚实边界**:agent 阶段默认 dry-run / 不调 LLM；真实执行器须显式 `--executor command --agent-cmd claude` 并具备 CLI、凭证与预算。workflow 先由原生零依赖 Go YAML 子集解析器处理，Python shim 仅为兼容回退。ADR-0001 据此置 Superseded。
- **D7 生产交付边界 (2026-07-27)**:采纳 ADR 0005。`deploy`/`rollback` 只生成、验证并由人批准声明式 `docs/release` 交付包；不持有生产凭证、不执行远程动作。release command 使用最小固定 prompt、精确 emit 权限、operator-pinned Claude executable bytes(非供应商身份)、receipt/source/artifact freshness 与 durable human marker；远程应用始终归外部 CI/operator。
- **D2 编排**:v0–v1 先复用 CC 原生(subagents/hooks/skills)；自研 runtime 已由 D6 在 v2 启动。
- **D3 执法**:带外 gate 为真相之源;CC hook 为加速器。AGENTS.md 只引导。
- **D4 路由**:v1 限 Claude 档(Haiku/Sonnet/Opus);跨厂商池 = v3。
- **D5 范围**:v0 只做 Context + Harness(止血)。

## 待你拍板 (open)
- **O1 目录改名**:`catalyst/` → `forgeos/`?(现保持 catalyst,产品名 ForgeOS)
- **O2 全局共享机制**:**机制已由 ADR 0003 定 = git submodule**(否决 symlink / npm / subtree / vendoring;双层覆盖 + 路径解析改造设计就绪)。**推荐暂缓**至「被治理项目 ≥ 2~3 且治理资产仍高频演进」触发条件;**仍待你拍板**:远程仓位置 + 批准不可逆迁移 + now-vs-暂缓(见 `../docs/adr/0003-agent-os-repo-extraction.md`「## 待拍板」)。

## 已结的历史 open
- **O3 第一条垂直切片**:已选并完成 `examples/url-shortener` dogfood。
- **O4 enforce**:已切 `block`；函数行数/循环依赖等由 arch-check 机器执法，整仓门禁全绿。
