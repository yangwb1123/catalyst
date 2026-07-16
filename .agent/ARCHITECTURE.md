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
                                                 └ 批准 → 生成 .agent/{PROJECT,ROADMAP,ARCHITECTURE}.md
REVIEW  (深度由 mode 裁决;对齐 AI-SDLC Stage 2-6)
  Security-Engineer     → STRIDE 威胁建模 + RFC 合规矩阵
  Distributed-Engineer  → 故障模式矩阵 + 一致性策略 + 重试策略
  Performance-Engineer  → 性能预算 + 生产就绪检查清单
  CTO                   → 综合裁决(Approve/Simplify/Redesign/Delay/Reject)
BUILD
  Planner → Implementer → [Harness 闸门] → Reviewer → QA   stop: ROADMAP 100%
EVOLVE
  Scan → Gap → Roadmap → Implement → Review → Evaluate → (loop)
```

## 中枢旋钮:mode × lifecycle
一个设置同时驱动三处:**Router 档位 · Harness 严格度 · Workflow 深度**。
- mode: explorer(快/省/跳 Discover) · balanced · engineering(全闸门) · cto(只出 PRD/Arch,人确认)
- lifecycle: idea → mvp → growth → production
- explorer→engineering = 一次「创业→企业」状态迁移:自动收紧 harness + 派生补测试/CI/监控任务。

## 载重墙(对原始构想的修正)
"站在所有 CLI 之上" ⇒ 只能强制最弱宿主允许的东西。因此:
- **真相之源 = 带外执法层**(Sandbox / CI runner 跑 harness 闸门),host-independent。
- 各工具的 hook(CC 的 PostToolUse/Stop 等)= **加速器适配器**(编辑器内快速失败),非地基。
- 每个宿主一个薄 adapter;无阻断能力处优雅降级为 advisory。

## 引擎 (Engines)
Gateway · Orchestrator · Agent-Runtime · **Model-Router** · Context-Engine · Memory-Engine ·
Knowledge-Engine · **Evaluation-Engine** · **Sandbox(载重墙)** · Web-UI
> **v2 现状**:forge-core Go 运行时已落地 **5 引擎**(均构建/全绿,纯标准库零依赖,13 包;与 [`BOOTSTRAP.md`](../BOOTSTRAP.md) §技术栈对齐):
> **Orchestrator**(`internal/orchestrator`)· **Model-Router**(`routing`)· **Context-Engine**(`prompt`)·
> **Memory-Engine**(`memory`)· **Evaluation-Engine**(`converge`);外加 Harness 闸门子集(`harness/gate.mjs`)+ Context 骨架(本 `.agent/`)。
> **Gateway · Agent-Runtime · Knowledge-Engine · Sandbox(载重墙)· Web-UI 仍为路线图。**

## 模型路由 (v1 限 Claude 档)
classify → score(复杂度/风险/依赖/安全/上下文) → tier(mode,lifecycle)
→ ⚠ risk≥critical 强制 ≥Opus → 💰 预算守卫 → 📈 历史择优(来自 Eval 记分卡)。
档位:Haiku=文档/CRUD/测试 · Sonnet=常规实现 · Opus=架构/安全 + 所有 Reviewer。跨厂商池 = v3。
