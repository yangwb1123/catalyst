# ForgeOS — Decisions

## 已锁定(默认,可推翻)
- **D1 技术栈**:**目标 = Go-核心 polyglot**(`forge-core`=Go 编排/调度/路由/workflow · `forge-ai`=Python 智能层 · `forge-runtime`=Rust 沙箱 · `forge-web`=TS/Next UI)。**时序**:v0–v1 不写 forge-core——编排=Claude Code 本体,只写声明式 agent 卡/workflow/policy + 薄胶水(`gate.mjs` 现用 Node,够用)+ polyglot harness 适配器(shell 出 eslint/golangci-lint/radon…)。**Go 已在 v2 进入**(见 D6:`forge-core` 已落地);Temporal/Postgres/NATS 仍未起,Rust 在 v3。harness CLI 未来固化为 **Go 静态二进制**(目标仓零运行时依赖)。
- **D6 v2 forge-core 启动 (2026-06-19)**:核心循环已在 CC 上经 `examples/url-shortener` dogfood 端到端验证稳定(architect→3 implementer→fresh reviewer→fix,被 `forge accept` 实际 gate)→ 触发 ADR-0001 的取代条件,**正式开建自研 Go 运行时 `forge-core`**。已落地:纯 Go 标准库、**零外部依赖**(`go.mod` 无 `require`),7 个包,CLI `forge run/evolve/gate/check/accept`;gate 阶段 shell 出真实 harness;路由带硬 Opus 底线;`forge evolve` 为无人值守闭环入口,收敛由 `converge` 按 ROADMAP 完成度 + acceptance gate 实算。**诚实边界**:agent 阶段默认 dry-run / 不调 LLM,真实执行器经 `--executor command --agent-cmd claude` 提供(限制 = agent-CLI 安装 + 凭证/预算);YAML 经 `harness/yaml2json.py` python shim 转码(临时脚手架)。ADR-0001 据此置 Superseded。
- **D2 编排**:先复用 CC 原生(subagents/hooks/skills),v2/v3 才上自研 runtime。
- **D3 执法**:带外 gate 为真相之源;CC hook 为加速器。AGENTS.md 只引导。
- **D4 路由**:v1 限 Claude 档(Haiku/Sonnet/Opus);跨厂商池 = v3。
- **D5 范围**:v0 只做 Context + Harness(止血)。

## 待你拍板 (open)
- **O1 目录改名**:`catalyst/` → `forgeos/`?(现保持 catalyst,产品名 ForgeOS)
- **O2 全局共享机制**:git submodule(推荐,可复现) / symlink(最简) / npm 包(最现代)?
- **O3 第一条垂直切片目标**:link-shortener API(我的建议) / 你指定 / dogfood ForgeOS 自己?
- **O4 enforce**:**已切 `block`(已结)** —— O3 切片(url-shortener)跑通后,`harness/policies.yml` 已置 `enforce: block`,整仓 block 模式全绿。注:函数行数 / 循环依赖适配器仍为 v0.1 TODO(见 policies.yml),其余红线已实际执法。
