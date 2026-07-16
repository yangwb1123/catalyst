# 实操示例：Stage 0 — Gateway Engine 产品发现

> 下面是一个完整的 Stage 0 评审示例。
> 上半部分是「填好的 Context」，下半部分是「期望的 Agent 产出」。
> 你可以直接把这个文件的全部内容（上半部分）复制粘贴到任意 AI Agent。

---

# 第一部分：你要喂给 Agent 的内容（复制以下全部内容）

---

# Stage 0 — Product Discovery

## ROLE

You are conducting a product discovery review for a production-grade software system.

You are simultaneously acting as:

- **Senior Product Manager** — Responsible for product value, business impact, user experience
- **Business Analyst** — Responsible for domain modeling, business rules, process flows
- **UX Designer** — Responsible for operator experience, configuration ergonomics

You are NOT an engineer in this stage. You do NOT discuss technology.

Your only job is to answer: **Should this feature exist?**

---

## OBJECTIVE

Determine whether the proposed subsystem is justified by real user needs,
is appropriately scoped, and is worth the engineering investment.

---

## CONTEXT

```
Project:              ForgeOS — AI-native software engineering platform
                      forge-core 是 Go 标准库零依赖运行时，当前 13 个包
                      已有: Orchestrator / Model-Router / Context-Engine / Memory-Engine / Evaluation-Engine
                      未建: Gateway / Agent-Runtime / Knowledge-Engine / Sandbox / Web-UI
Subsystem:            Gateway Engine（forge-core 新增模块）
Current Sprint Goal:   评估 Gateway 是否为当前阶段(mvp lifecycle)必做项
Proposed Feature:     在 forge-core 中实现统一 Gateway，承接所有外部模型 API 调用
                      (Claude / OpenAI / Gemini / 自部署模型)，提供:
                      - 统一请求接口（屏蔽厂商差异）
                      - 负载均衡（同厂商多 API key 轮转）
                      - 自动 fallback（主厂商故障切换备厂商）
                      - Rate limiting（per-vendor / per-key）
                      - 预算控制（per-call / per-run / per-iteration 美元上限）
User Scenarios:       1. forge-core 当前硬编码 Claude API，无法调用其他厂商模型
                      2. Claude API 529 过载时，当前只能重试同一厂商（Sprint 22 已修重试逻辑）
                      3. 企业客户希望限制每月 AI 总支出不超过 $X
                      4. 不同 phase 需要不同厂商（planner 用 Opus，implementer 用 Sonnet）
Product Goals:        G3 自动模型调度 — 多维路由（复杂度/风险/阶段/预算/上下文/历史）
                      v3 目标: 跨厂商模型池（ROADMAP.md 明确标注 "跨厂商池 = v3"）
Relevant Documents:   - ROADMAP.md: "routing: claude-tier-only — 跨厂商池 = v3"
                      - ARCHITECTURE.md: 引擎列表含 Gateway
                      - ADR-0002: Go core polyglot stack（零依赖原则）
                      - forge-core/internal/routing/: 当前路由只支持 Claude 三档
                      - forge-core/internal/orchestrator/: CommandExecutor 直接调 claude CLI
```

---

## TASKS

### Task 1 — Problem Validation

Answer each question with evidence, not opinion.

Classification:
- Critical — Users cannot complete core workflow without this
- Important — Users can work around it but it costs significant time
- Nice to Have — Minor convenience improvement
- Premature — No real users need this yet
- Fake Requirement — Based on hypothetical, not observed, user behavior

### Task 2 — Market Reality Check

How do production platforms handle multi-model routing?

### Task 3 — Consequence Analysis

If we NEVER implement this subsystem, what realistic production problems would occur?

### Task 4 — Scope Rationalization

Identify: duplicated functionality, unnecessary abstraction, unnecessary configuration,
hidden maintenance costs, product complexity.

### Task 5 — MVP Definition

Define the smallest deliverable that solves the core problem.

---

# 第二部分：期望的 Agent 产出（这是 AI Agent 应该返回的内容）

---

## Product Discovery Report

### Problem Statement

forge-core 当前硬编码 Claude CLI 作为唯一模型调用通道，无法利用其他厂商模型，
且在 Claude 服务不可用时无自动降级能力，直接阻断无人值守流水线。

### Requirement Classification

**Important**（非 Critical，原因如下）

**Evidence:**
- 当前真点火已端到端跑通（Sprint 24-26），Claude 单厂商下 MVP 闭环已验证
- 529 过载已通过 retry + backoff 缓解（Sprint 22 `KindOverloaded`），但只 retry 同厂商
- forge-core 的 routing 模块已有 tier 分级（Haiku/Sonnet/Opus），但只映射到 Claude 模型名
- **当前 workaround**: 用户手动切换 `--agent-cmd`，或外部包一层 LiteLLM

**为什么不是 Critical:**
- forge-core 的核心价值是治理层（harness + orchestrator + converge），不是模型调用
- Claude 单厂商在 MVP lifecycle 下够用
- 无人值守流水线的阻断问题已通过 retry 缓解（非完全无防护）

**为什么不是 Premature:**
- 多厂商支持是 ROADMAP v3 明确条目，不是临时起意
- 企业客户的预算控制需求是真实签约障碍

### Market Reality Check

| Platform | 做法 | 启示 |
|----------|------|------|
| LiteLLM | 独立代理层，100+ 厂商适配 | **不要自己写 adapter，用成熟方案** |
| OpenRouter | 统一 API 代理，自动 fallback | 验证了 fallback 是真实需求 |
| LangChain | 内置 multi-provider，但被批过度抽象 | 警告：不要做通用 LLM 框架 |
| Vercel AI SDK | 轻量 provider 接口 | 接口抽象是合理的，但不要做完整框架 |

**关键洞察**: 业界已有 LiteLLM 这样的成熟代理层。
forge-core 不需要重新实现 "LiteLLM in Go"。

### Consequence Analysis

**如果不实现:**
1. 企业客户无法设定月度 AI 支出上限 → 签约受阻（真实障碍）
2. Claude 长时间不可用（>30min）时流水线完全停摆 → 无人值守承诺打折
3. 无法利用更便宜的厂商处理低复杂度任务 → 成本优化受限

**不会发生的:**
- 核心治理功能不会受影响（harness/orchestrator/converge 不依赖 Gateway）
- 用户仍然可以手动指定 `--agent-cmd` 切换厂商

### Scope Rationalization

| 功能 | 建议 | 原因 |
|------|------|------|
| 统一请求接口 | **KEEP** (MVP) | 核心抽象，routing 模块需要 |
| 多厂商 adapter | **POSTPONE** | LiteLLM 已解决，forge-core 零依赖原则不允许 |
| 负载均衡 | **POSTPONE** | 单 key 场景先跑通，多 key 轮转是 growth 期需求 |
| 自动 fallback | **SIMPLIFY** → 只做 config-driven failover | 不做健康检查/自动发现，配置声明即可 |
| Rate limiting | **POSTPONE** | 厂商侧已有 rate limit，forge-core 侧不是瓶颈 |
| 预算控制 | **KEEP** (MVP) | 已有 `--max-budget-usd` per-call，只需加 per-run 聚合 |
| 请求/响应标准化 | **SIMPLIFY** → 只标准化 forge-core 内部消费的最小字段 | 不做完整 OpenAI-compatible API |

### MVP Scope

**IN（本 Sprint 做）:**
1. Provider 接口（`Provider` interface + Claude/OpenAI 两个实现）
2. Config-driven failover（主/备厂商声明式配置，故障时切备用）
3. Per-run 预算聚合（在现有 per-call budget 基础上加 run 级累计）
4. Routing 模块扩展（tier → 具体 model name 的映射从 Claude-only → 多厂商）

**OUT（明确不做）:**
- 不实现 LiteLLM 兼容层
- 不做自动健康检查 / 服务发现
- 不做多 key 轮转负载均衡
- 不做请求/响应完整标准化（只标准化 forge-core 消费的字段）
- 不做 WebSocket / streaming 统一

### Non-Goals

1. **不做通用 LLM Gateway 产品** — forge-core 是治理层，不是模型代理
2. **不替代 LiteLLM** — 如果用户需要完整代理能力，建议外部部署 LiteLLM
3. **不做跨厂商模型能力对齐** — 不同厂商模型能力差异由用户自行评估
4. **不做实时模型性能对比** — 不在 Gateway 层做 A/B testing

### Risks

1. **零依赖原则冲突**: forge-core 坚持纯标准库零依赖。
   - 缓解: Provider interface 是纯 Go interface，不引入外部 SDK。
   - HTTP 调用用 `net/http` 标准库，JSON 用 `encoding/json`。
2. **厂商 API 差异大**: Claude Messages API vs OpenAI Chat Completions API 格式不同。
   - 缓解: 只抽象 forge-core 消费的最小字段（role/content/model/stop），不做完整转换。

### Recommendation

**PROCEED WITH REDUCED SCOPE**

Gateway 值得做，但范围应缩减到 MVP 四件事。
核心论证：Provider 接口 + routing 扩展是 routing 模块的自然演进，
不是新建一个 "Gateway Engine"。建议重新定义：

**不是新建 Gateway 模块，而是扩展现有 routing 模块 + 新增 provider 包。**

```
forge-core/internal/
├── routing/          # 现有：tier 计算 → 扩展：tier → provider+model 映射
├── provider/         # 新增：Provider interface + Claude/OpenAI 实现
└── orchestrator/     # 现有：CommandExecutor → 扩展：支持 Provider 直调（非 CLI）
```

---

## Decision

**Proceed with Reduced Scope** → Advance to Stage 1

核心理由：
1. 多厂商支持是真实需求（ROADMAP v3 + 企业签约障碍）
2. 但不需要新建 "Gateway Engine"，只需扩展 routing + 新增 provider
3. 遵循 forge-core 零依赖原则（纯标准库 HTTP + JSON）
4. MVP 范围可在 1 sprint 内由 3 工程师交付
