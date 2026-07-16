# AI-SDLC — AI Software Development Lifecycle

> **一套模板，适用于所有项目。** 不绑定 Claude Code / Codex / Gemini CLI / OpenCode 中的任何一个。
> 按 SDLC 阶段拆模板，每个模板包含多个专业角色，避免单一视角偏差和 Prompt 过长导致的注意力分散。

## 核心理念

```
一个 Stage（阶段）= 多个 Reviewer（角色）
一个模板 = 固定骨架 + 可替换 Context
一个项目 = 按阶段逐步推进，每步有明确产出物
```

## 流程图

```
┌─────────────────────┐
│ Stage 0             │
│ Product Discovery   │ → Should this exist?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 1             │
│ Architecture Review │ → How should it be structured?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 2             │
│ Security & RFC      │ → Is it safe and compliant?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 3             │
│ Distributed Review  │ → Will it hold under failure?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 4             │
│ Implementation      │ → Is the code maintainable?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 5             │
│ Performance Review  │ → Will it meet latency/memory budget?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 6             │
│ Production Ready    │ → Can we deploy safely?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 7             │
│ Sprint Planning     │ → What is the execution plan?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 8             │
│ Post Sprint Review  │ → Did we meet DoD? What debt?
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│ Stage 9             │
│ CTO Decision        │ → Approve / Redesign / Reject
└─────────────────────┘
```

## 17 个专业角色

| Role | Responsibility | Active Stages |
|------|---------------|---------------|
| Product Manager | Requirement validation, user value | 0, 7, 9 |
| Business Analyst | Domain modeling, business rules | 0, 7 |
| UX Designer | Admin workflow, usability | 0 |
| Solution Architect | Module boundaries, DDD, event flow | 1, 7 |
| Backend Architect | API, storage, service design | 1, 4, 7 |
| Security Engineer | Auth, threat modeling, STRIDE | 2, 6 |
| Protocol Expert | OAuth2/OIDC/WebAuthn/RFC compliance | 2 |
| Distributed Systems Engineer | Consistency, concurrency, networking | 3 |
| Database Architect | Schema, indexing, migration | 3, 7 |
| Performance Engineer | Latency, throughput, memory, GC | 5 |
| SRE / Platform Engineer | Observability, deployment, rollback | 6, 8 |
| QA Lead | Test strategy, fuzzing, regression | 6, 8 |
| DevOps Engineer | CI/CD, release pipeline | 6 |
| Compliance Officer | GDPR, SOC2, ISO27001 | 2, 6 |
| Staff Engineer | Code quality, maintainability | 4, 8 |
| CTO | Technology strategy & ROI | 1, 9 |
| Principal Reviewer | Final trade-off decisions | 9 |

## 目录结构

```
.ai/
├── prompts/
│   ├── 00-product-discovery.md
│   ├── 01-architecture-review.md
│   ├── 02-security-rfc-review.md
│   ├── 03-distributed-review.md
│   ├── 04-implementation-review.md
│   ├── 05-performance-review.md
│   ├── 06-production-readiness.md
│   ├── 07-sprint-planning.md
│   ├── 08-post-sprint-review.md
│   ├── 09-cto-review.md
│   └── shared/
│       ├── role-definitions.md
│       ├── output-format.md
│       ├── review-checklists.md
│       └── engineering-principles.md
├── adrs/              # Architecture Decision Records
├── reviews/           # Stage output (review artifacts)
└── sprint/            # Sprint planning artifacts
```

## 使用方法

### 1. 选择一个子系统

确定要评审的模块（例如：SSO、AI Gateway、RBAC、Workflow Engine）。

### 2. 从 Stage 0 开始

```bash
# 打开 00-product-discovery.md
# 替换 {{Context}} 部分：
#   - Project: 项目名
#   - Subsystem: 子系统名
#   - Current Sprint Goal: 本迭代目标
#   - Architecture Summary: 当前架构摘要
#   - Relevant Code: 关键代码片段
#   - Relevant Documents: 相关 RFC/ADR/Spec
```

### 3. 将 Prompt 喂给 AI Agent

将填好 Context 的模板整体粘贴到 Claude Code / Codex / Gemini CLI 等。

### 4. 收集产出物

将 AI 输出保存到 `.ai/reviews/` 对应目录：

```
.ai/reviews/
├── sso/
│   ├── 00-product-discovery.md
│   ├── 01-architecture-review.md
│   └── ...
├── ai-gateway/
│   └── ...
```

### 5. 按阶段推进

- Stage 0-1：决定**做什么、为什么做**（Level 1: Architecture Review）
- Stage 2-4：决定**怎么做才可靠**（Level 2: Implementation Review）
- Stage 5-6：决定**是否可以上线**（Level 3: Production Readiness Review）
- Stage 7-9：**执行、复盘、决策**（Execution & Governance）

### 6. 阶段间传递

每个 Stage 的输出是下一个 Stage 的输入。例如：

- Stage 1 的 ADR → Stage 2 的 Security Review 参考
- Stage 3 的 Failure Matrix → Stage 6 的 Rollback Plan 依据
- Stage 7 的 Sprint Backlog → Stage 8 的 DoD 对照基线

### 7. 最终决策

Stage 9 产出唯一结论：

- **Approve** — 按计划实施
- **Approve with Simplification** — 削减范围后实施
- **Redesign** — 架构需要重做
- **Delay** — 时机未到
- **Reject** — 不应实施

## 三层级快速索引

| Level | Stages | Focus | Decision |
|-------|--------|-------|----------|
| L1 Architecture | 0-1 | What & Why | Should we build it? |
| L2 Implementation | 2-4 | How | Can we build it reliably? |
| L3 Production | 5-6 | When | Can we ship it safely? |
| Execution | 7-9 | Who & When | How do we execute? |

## 约束假设

所有模板默认假设：

- 3 名工程师
- 两周 Sprint
- 生产系统
- 企业客户
- Kubernetes 部署
- PostgreSQL + Redis
- Go 和/或 Rust
- Cloud Native 架构

如不满足，在 Context 中明确标注差异。

## 工程原则（所有 Stage 共享）

每条建议必须满足至少一项：

1. 增加业务价值
2. 降低工程复杂度
3. 降低运维成本
4. 提升可靠性
5. 提升安全性
6. 提升可维护性

否则拒绝。不推荐无收益的额外复杂度。
