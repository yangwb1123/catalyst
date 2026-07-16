# ADR 0002 — 技术栈:Go-核心 polyglot,分期引入

- 状态:Accepted
- 日期:2026-06-19

## 背景
ForgeOS 核心问题是**编排**(大量 IO/RPC/并发地调度 Agent 舰队),非高性能推理。
候选:全 Rust(过早优化性能)、全 Python(规模化编排痛)、Go-核心 polyglot。

## 决策
**目标 = Go-核心 polyglot**:`forge-core`=Go(编排/调度/路由/workflow)、`forge-ai`=Python(智能层,未排期——同 `forge-runtime`一样分期未开工,尚无目标版本 / no target version assigned yet)、
`forge-runtime`=Rust(沙箱)、`forge-web`=TS/Next(UI)。
**时序**:v0–v1 无 forge-core(见 ADR 0001),只声明式 + Node 胶水;Go 在 v2 进(+ Temporal 治理人审持久 wait + Postgres/NATS);Rust v3。
harness CLI 未来固化为 **Go 静态二进制**(目标仓零运行时依赖)。

## 后果
- 优点:Go 的 goroutine 正合扇出调度;采购 Temporal/LiteLLM/Qdrant 等,自研火力压在护城河。
- 代价:多语言 monorepo 的边界(Go↔Python 用 gRPC/HTTP,`forge-ai` 保持无状态)。
- 反对:四语言 day-1 全立 = 镀金。

参见 `../../.agent/DECISIONS.md` D1。
