# Skill: ai-sdlc-review

**Trigger** — 执行 REVIEW 阶段(Design → Build 之间的深度评审)
**Phase** — Review
**Agent** — security-engineer / distributed-engineer / performance-engineer / cto
**Mode** — engineering(全四维度)/ balanced(安全+分布式)/ explorer(跳过)

## 输入 (consumes)
- `.agent/workflows/review.yml`(评审流程定义)
- `.ai/prompts/` 目录下的 AI-SDLC 模板:
  - `02-security-rfc-review.md` → security-engineer
  - `03-distributed-review.md` → distributed-engineer
  - `05-performance-review.md` → performance-engineer
  - `06-production-readiness.md` → performance-engineer
  - `09-cto-review.md` → cto
- `architect` 产出的架构文档(模块边界 / API / 数据模型)
- `cto` 产出的技术选型(基础设施 / 依赖)

## 执行 (process)
### P1 安全评审 (security-engineer)
1. 读取 `.ai/prompts/02-security-rfc-review.md` 模板
2. 填充 Context(项目名 / 子系统 / 架构摘要 / 代码路径)
3. 执行 7 个 Task:
   - Trust Boundary Mapping(信任边界)
   - STRIDE Threat Model(威胁建模)
   - Protocol Compliance Matrix(RFC 合规)
   - Token & Session Lifecycle
   - Input Validation Review
   - Secret Management
   - Compliance Assessment
4. 产出 `security-review.md` + `threat-model.md`
5. 输出机读裁决: `VERDICT: APPROVE` 或 `VERDICT: REQUEST_CHANGES`

### P2 分布式评审 (distributed-engineer)
1. 读取 `.ai/prompts/03-distributed-review.md` 模板
2. 填充 Context(基础设施拓扑 / 并发模型 / 状态所有权)
3. 执行 7 个 Task:
   - Concurrency Analysis(并发分析)
   - Idempotency Review(幂等性)
   - Failure Mode Matrix(故障模式)
   - Distributed Lock Analysis(分布式锁)
   - Cache Consistency(缓存一致性)
   - Retry & Backoff Strategy(重试退避)
   - Edge Cases & Horror Stories(边缘案例)
4. 产出 `distributed-review.md`
5. 输出机读裁决

### P3 性能+生产就绪评审 (performance-engineer)
1. 读取 `.ai/prompts/05-performance-review.md` + `06-production-readiness.md`
2. 填充 Context(负载估算 / 延迟目标 / 部署拓扑)
3. 执行性能评审 7 Task:
   - Hot Path Identification(热路径)
   - Database Query Analysis(DB 查询)
   - Memory & Allocation Analysis(内存分配)
   - Cache Effectiveness(缓存有效性)
   - Connection Pool & Resource Limits(连接池)
   - Performance Budget(延迟预算)
   - Benchmark Plan(基准计划)
4. 执行生产就绪评审 7 Task:
   - Observability Verification(可观测性)
   - Deployment Strategy(部署策略)
   - Health & Readiness(健康检查)
   - Rollback Plan(回滚计划)
   - Capacity Planning(容量规划)
   - Test Strategy Verification(测试策略)
   - Runbook & Incident Response(Runbook)
5. 产出 `performance-budget.md` + `production-readiness.md`
6. 输出机读裁决

### P4 CTO 综合裁决 (cto)
1. 读取 `.ai/prompts/09-cto-review.md` 模板
2. 填充 Context(所有评审产出 / 业务上下文 / 团队上下文)
3. 回答 5 个问题:
   - Should we build this NOW?
   - Is it over-engineered?
   - Is it maintainable for 5+ years?
   - Can a 3-engineer team own this?
   - Is the ROI justified?
4. 产出 `review-summary.md`(综合裁决 + Top 10 Risks + Non-Goals)
5. 输出最终裁决:
   - `VERDICT: APPROVE` → 解锁 BUILD
   - `VERDICT: APPROVE_WITH_SIMPLIFICATION` → 锁定范围后解锁 BUILD
   - `VERDICT: REDESIGN` → 退回 DESIGN 阶段
   - `VERDICT: DELAY` → 暂停,等待条件成熟
   - `VERDICT: REJECT` → 终止

## 输出 (produces)
- `docs/review/security-review.md`
- `docs/review/threat-model.md`
- `docs/review/distributed-review.md`
- `docs/review/performance-budget.md`
- `docs/review/production-readiness.md`
- `docs/review/review-summary.md`
- 机读裁决(末行 `VERDICT: ...`)

## 硬边界 (Boundaries)
- ❌ **不写/不改代码**:只产评审报告与建议
- ❌ **不重新设计架构**:发现问题退回 `architect`,不自行重设计
- ❌ **不跳过评审阶段**:mode=explorer 时整体跳过,否则必须按序执行
- ❌ **不伪造通过**:未发现真实问题也要说明"未发现",不空编
- ✅ **基于证据评审**:代码/配置/协议规范/性能测量,不凭直觉
- ✅ **机读裁决格式**:末行顶格 `VERDICT: ...`,无包裹

## 交接 / 停止 (handoff / stop)
- 所有评审相位 APPROVE → CTO 综合裁决 → 解锁 BUILD
- 任一相位 REQUEST_CHANGES → 退回架构师重设计
- CTO REDESIGN → 退回 DESIGN 阶段(solution-architect)
- CTO DELAY/REJECT → 终止,记录决策依据

## 模板引用 (template references)
所有 AI-SDLC 模板位于 `.ai/prompts/`:
- `00-product-discovery.md` → Discover 阶段
- `01-architecture-review.md` → Design 阶段
- `02-security-rfc-review.md` → Review P1
- `03-distributed-review.md` → Review P2
- `04-implementation-review.md` → Build Reviewer 参考
- `05-performance-review.md` → Review P3
- `06-production-readiness.md` → Review P3
- `07-sprint-planning.md` → Build Planner 参考
- `08-post-sprint-review.md` → Evolve 阶段
- `09-cto-review.md` → Review P4

## 快速上手 (quickstart)
见 `.ai/QUICKSTART.md` — 三步走:选子系统 → 填 Context → 喂给 Agent
