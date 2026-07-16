# ADR-0004: REVIEW 阶段 — AI-SDLC 深度评审集成

**Status:** Accepted
**Date:** 2026-06-30
**Supersedes:** N/A

## Context

ForgeOS 脊柱原为 4 段:Discover → Design → Build → Evolve。

Design 阶段产出已批架构后直接进 Build(写代码)。但实践中发现:
- 安全威胁未在写码前识别(架构评审只查模块边界/依赖方向,不做 STRIDE)
- 分布式系统故障模式未文档化(Redis 挂了怎么办?网络分区怎么处理?)
- 性能预算未定义(延迟目标?连接池大小?缓存命中率?)
- 生产就绪检查缺失(监控?回滚?Runbook?)

这些问题在 Build 阶段甚至上线后才暴露,修复成本指数级增长。

AI-SDLC(已在 `.ai/prompts/` 实现 10 阶段模板)提供了完整的评审框架,但未集成到脊柱工作流。

## Decision

在 Design 和 Build 之间插入 **REVIEW 阶段**,对齐 AI-SDLC Stage 2-6:

```
Discover → Design → ★REVIEW★ → Build → Evolve
```

REVIEW 阶段包含 4 个相位:

| 相位 | Agent | 模板 | 产出 |
|------|-------|------|------|
| P1 Security Review | `security-engineer` | `02-security-rfc-review.md` | STRIDE + RFC 合规 + 风险矩阵 |
| P2 Distributed Review | `distributed-engineer` | `03-distributed-review.md` | 故障模式矩阵 + 一致性策略 |
| P3 Performance+Reliability | `performance-engineer` | `05-performance-review.md` + `06-production-readiness.md` | 延迟预算 + Go/No-Go |
| P4 CTO Executive Review | `cto` | `09-cto-review.md` | 综合裁决(Approve/Simplify/Redesign/Delay/Reject) |

### Mode-Gating

| Mode | Review 深度 | 说明 |
|------|-----------|------|
| `explorer` | skip | 跳过 REVIEW,直进 Build |
| `balanced` | standard | 只跑 P1(安全)+ P2(分布式) [corrected 2026-07-02: 实际是 P1+P2+P4 三相位,只跳过 P3(性能/可靠性,`optional_for: [balanced]`);见 `review.yml` 与 `orchestrator_review_gating_test.go::TestRun_BalancedSkipsOptionalReviewPhase`] |
| `engineering` | full | 全 4 相位 |
| `cto` | full | 全评审,产出作为文档交付 |

### 新增资产

1. **Workflow**: `.agent/workflows/review.yml`
2. **Agent Cards** (3 个):
   - `.agent/agents/security-engineer.md`
   - `.agent/agents/distributed-engineer.md`
   - `.agent/agents/performance-engineer.md`
3. **Skill Card**: `.agent/skills/ai-sdlc-review.md`
4. **Policy Update**: `modes.yml` 添加 `review: skip/standard/full/full`
5. **Spine Update**: `design.yml` 的 `next_stage` 从 `build` 改为 `review`
6. **Architecture**: `ARCHITECTURE.md` 脊柱图添加 REVIEW 段

## Consequences

### Positive
- **左移安全**:威胁在写码前识别,修复成本降低 10-100x
- **故障模式显式化**:Fail Open/Closed/Unsafe 分类,杜绝 Fail Unsafe 上线
- **性能预算前置**:延迟目标 + 基准计划,避免上线后救火
- **生产就绪检查**:监控/回滚/Runbook 在 Build 前定义
- **模板复用**:`.ai/prompts/` 的 10 阶段模板通过 skill card 集成,非重复发明

### Negative
- **流程延长**:engineering/cto 模式增加 4 个相位(每相位 ~15-30 分钟)
- **新增 3 个 Agent**:需要维护 agent card + 确保 Opus 路由
- **explorer 模式无评审**:快速原型可能带安全风险上线(可接受,explorer = vibe-code)

### Risks
1. **评审相位超时**:Opus 调用可能触发 budget guard
   - 缓解:每相位独立 budget,超时即停
2. **评审产出不被消费**:review-summary.md 写完后无人读
   - 缓解:feeds_forward=true,CTO 裁决注入 BUILD planner
3. **模板与实际架构脱节**:`.ai/prompts/` 是通用模板,未绑定具体项目
   - 缓解:skill card 明确要求填充 Context,不空跑模板

### Mitigations
- **explorer 跳过 REVIEW**:快速原型不被流程拖慢
- **balanced 只跑关键两维**:安全 + 分布式(最常见故障源),性能/生产就绪可选
- **机读裁决**:每相位末行 `VERDICT: APPROVE/REQUEST_CHANGES`,orchestrator 自动路由
- **fresh-context**:评审 agent 必须是独立上下文,不审自己写的代码

## Alternatives Considered

| Alternative | Pros | Cons | Why Rejected |
|-------------|------|------|--------------|
| 在 Build 的 Reviewer 相位做深度评审 | 不增加新阶段 | Reviewer 已超载(代码评审 + 架构评审混在一起) | 职责不单一,评审质量下降 |
| 在 Evolve 阶段做 retrospective 评审 | 上线后评审更真实 | 修复成本已高,无法左移 | 违反"越早越便宜"原则 |
| 不做深度评审,靠 harness 闸门 | 流程最短 | 闸门只查体积/复杂度/循环依赖,不查安全/分布式/性能 | 闸门 ≠ 评审,覆盖面不足 |
| 只用 `.ai/prompts/` 手动评审 | 不改脊柱 | 未集成到工作流,容易跳过 | 无强制力,依赖人记得跑 |

## References

- AI-SDLC 模板: `.ai/prompts/00~09-*.md`
- AI-SDLC 快速上手: `.ai/QUICKSTART.md`
- AI-SDLC 完整指南: `.ai/README.md`
- Skill Card: `.agent/skills/ai-sdlc-review.md`
- Workflow: `.agent/workflows/review.yml`
- 中枢旋钮: `.agent/policies/modes.yml` (review depth)
