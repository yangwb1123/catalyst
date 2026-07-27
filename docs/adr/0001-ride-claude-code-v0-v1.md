# ADR 0001 — v0–v1 骑 Claude Code,自研运行时推迟到 v2

- 状态:Superseded
- 日期:2026-06-19
- 取代说明(2026-06-19;现状更新 2026-07-27):本 ADR 的取代条件——「核心循环在 CC 上验证稳定」——已由 `examples/url-shortener` dogfood 触发(architect→3 implementer→fresh reviewer→fix 全脊柱端到端建成,被 `forge accept` 实际 gate)。据此 v2 自研零依赖 Go 运行时 `forge-core` 已落地并继续扩展到 durable chain、审批/拒绝、trace、声明式 release 与完整 acceptance CLI。详见 `../../.agent/DECISIONS.md` D6/D7。本决策的 v0–v1 部分仍是历史事实记录。

## 背景
ForgeOS 目标是分布式 HA 微服务的「AI 软件工厂」(见 `../../.agent/architecture/north-star.md`)。
但 day-1 就建 Go forge-core + Temporal + Firecracker = 在验证一个 Agent 闭环前先花数周接基础设施。

## 决策
v0–v1 **不自研运行时**:编排器 = Claude Code 本体(原生 subagents / hooks / skills / Todo)。
本阶段只产出声明式资产(`.agent/` 下 agent 卡 / skills / workflows / policies)+ 薄胶水(`harness/gate.mjs`)。
自研 Go 运行时(forge-core)推迟到 v2,且仅当核心循环已在 CC 上验证稳定后。

## 后果
- 优点:最快验证编排是否成立;不返工——载重墙 gate 已 host-independent,`.agent/` 已 tool-agnostic。
- 代价:v0–v1 无自动 24h 循环,需人 / CC 会话驱动 workflow。
- 取代条件:核心循环验证稳定 + 需要真正无人值守 → 触发 v2 forge-core。

参见 `../../.agent/DECISIONS.md` D2、`../../.agent/ROADMAP.md`。
