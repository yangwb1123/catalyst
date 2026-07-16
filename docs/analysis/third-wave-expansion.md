# ForgeOS — 第三次架构扫描：四个未触及的高杠杆方向

> **扫描基准**：`b0c80e4`  
> **视角**：本次刻意避开所有已被已有分析覆盖的领域，专攻「没有人写过但问题真实存在」的方向  
> **方法论**：全量文件遍历 + 跨模块引用追踪 + 声明 vs 实现的逐条比对

---

## 发现：三条此前未被任何分析文档记录的事实

### 事实 1：`discover.yml` 和 `review.yml` 永远无法收敛

这是本次扫描发现的最具体的 gap。

`converge.go` 的 `evalOne` 只识别三类 metric：

```go
case c.Metric == "roadmap_completion":     // ✅ build.yml 用
case c.Metric == "gates_status":           // ✅ build.yml 用
case acceptanceMetrics[c.Metric]:          // test_pass, app_test_pass, architecture...
```

但 `.agent/workflows/` 中声明了另外两个 metric：

```yaml
# discover.yml — 需求发现
stop_condition:
  all_of:
    - metric: requirement_confidence   ← evalOne 不认识
      operator: ">="
      threshold: 80

# review.yml — 全维度评审
stop_condition:
  all_of:
    - metric: review_status            ← evalOne 不认识
      operator: "=="
      value: approved
```

这两个 metric 在 `evalOne` 中走 `default` 分支，返回 `unknownDetail(c)`，**永远 `Met: false`**。

**影响**：
- `forge run discover` → stop condition 永远是 NOT MET → converge 永远不会 true
- `forge run review` → 同上
- 这两个 workflow 当前只能靠 `--max-iter` 安全背停止，不能靠真实收敛停止
- 且这两个 workflow 没有 `on_unmet` 的声明式循环保护（build.yml 有 `loop_to_next_roadmap_item`；discover.yml 有 `on_unmet` 但 convergence 根本不走那个分支）

**根因**：`evalOne` 是写死的 switch，不是可扩展的 metric registry。每次加 workflow 加 metric 都要改 converge.go。

### 事实 2：架构执法（arch-check）不在 `forge run` 的关键路径上

`forge run` 执行引擎时，`arch-check.mjs` 的 8 项检查（layering / 包 / 扇入 / 认知 / 反模式命名 / 函数长度 / 循环依赖 / drift-guard）**只通过 `forge accept` 运行**，不在 `forge run` 的 gate phases 中。

这意味着：`forge run build` 可以在一个有架构违规的仓库上顺利跑完——只要 test 绿了、roadmap 勾了。架构违规只被 `forge accept` 在事后拦住。

对于 24h 无人值守的 `forge evolve`，架构违规要到 iteration 结束才被发现——中间已经烧了 N 轮 agent 调用的钱。

### 事实 3：`.ai/` 目录的 17 个角色与 `.agent/workflows/` 的 6 个 agent card 之间存在零桥接

`.ai/` 包含：

| 资产 | 数量 |
|------|------|
| SDLC 阶段模板 | 10 个（Stage 0–9） |
| 专业角色 | 17 个（Product Manager、Security Engineer、Distributed Engineer…） |
| 共享上下文 | 4 个（engineering-principles, output-format, review-checklists, role-definitions） |
| 评审产物示例 | 1 个（example-gateway-stage0） |

`.agent/agents/` 包含：

| 资产 | 数量 |
|------|------|
| Agent role cards | 9 个（planner, implementer, reviewer, qa, architect, cto…） |
| Workflow 编排 | 5 个（discover, design, build, review, evolve） |

`.ai/` 的模板是**人用**的（人在 Claude Code 中调用 prompt 模板），`.agent/workflows/` 是**系统用**的（编排引擎自动驱动 agent 卡）。两者描述的是同一套 SDLC，但没有任何代码或流程桥接它们。

---

## 四个高价值扩展方向

### 方向 1：收敛指标注册系统（让所有 workflow 真收敛）

**当前状态**：
`evalOne` 是硬编码 switch。每加一种 metric 都要改 `converge.go`。当前已有 2 个声明的 metric（`requirement_confidence`、`review_status`）未被任何代码处理。

**建议方案**：

```go
// internal/converge 增加注册机制
type MetricEvaluator func(criterion asset.Criterion, sig Signals) Result

var registry = map[string]MetricEvaluator{}

func RegisterMetric(name string, fn MetricEvaluator) {
    registry[name] = fn
}

func evalOne(c asset.Criterion, sig Signals) Result {
    if fn, ok := registry[c.Metric]; ok {
        return fn(c, sig)
    }
    // fallback to built-in metrics
    switch {
    case c.Metric == "roadmap_completion": ...
    case c.Metric == "gates_status": ...
    }
}
```

同时为 `requirement_confidence` 和 `review_status` 实现真实的 evaluator——这两个 metric 不需要调 LLM，可以基于已收集的信号计算（confidence 来自 Discover 阶段的结构化输出评分，review 状态来自 reviewer 的 VERDICT 输出）。

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **正确性** | 两个 workflow 当前无法收敛 → `forge run discover` 和 `forge run review` 实际上不可用。这是 bug 级别的 gap |
| **可扩展性** | 注册机制让用户自定义 metric 成为可能（见下个方向）。这是平台化的前置条件 |
| **声明-实现一致** | workflow YAML 声明了 `stop_condition`，但运行时不消费——这是 Sprint 12 审计要抓的「声明 vs 实现漂移」的活体案例 |

**实施难度**：低。纯代码重构 + 两个 metric 的 evaluator 实现，不涉及架构变更。

---

### 方向 2：运行时架构执法（把 arch-check 拉进 gate phase）

**当前状态**：
`forge run build` 的 `harness-gates` phase 运行 `[lint, test, build, complexity, arch, security]` 六大门。但 `arch` 在 build.yml 中对应的实际 gate 并不包括 `arch-check.mjs`——它是被 `forge accept` 调用，不是 `forge run`。

真实路径：
```
forge run build
  → orchestrator.runGates()
    → 只跑了 acceptance probe（lint, test, complexity...）
    → arch-check.mjs 不在其中
  → iteration ends
  → converge check (NOT MET)
  → next iteration
  → ...
  → forge accept（事后：跑 arch-check + secret-scan + SCA）
```

**建议方案**：

在 `cmd/forge/gates.go` 的 `harnessRunner` 中，增加 `forge run` 阶段的架构检查：

```go
// 在每次 runGates 之前或同时，增加一次架构快照
// 不是等 end-of-run 的 forge accept 才查
func runArchCheck(root string) gate.Result {
    // 调用 node harness/arch/arch-check.mjs --root <root>
    // PASS = 架构合规，继续
    // FAIL = 架构违规 → gate red → loop-back 或 abort（依 on_fail 而定）
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **成本** | 架构违规在 evolve iteration 结束时才被发现，中间 N 轮 agent 调用的钱已经烧了。提前到 gate phase 里，loop-back 可以早修复 |
| **一致性** | 「架构规则在 forge accept 才执行」与「forge run 可以通过有违规的仓库」之间的矛盾——治理层的职责是尽早失败 |
| **dogfood** | ForgeOS 自己的红线（max_file_lines: 500、circular_dependency_count: 0）应该在自己的每次 `forge run` 中实时执行，而非等到 CI 的 `forge accept` |

**边界情况**：

1. **架构检查性能**：`arch-check.mjs` 扫描全仓可能耗时 200-500ms。在 evolve 的每个 iteration 跑一次是可以接受的（vs agent phase 的分钟级），但在多次 loop-back 场景下可能累积
2. **False positive 导致的 loop-back storm**：如果一个 false-positive 架构违规在每个 iteration 都触发 loop-back，形成死循环。需要架构检查的 warn vs block 分层（同 `policies.yml` 的 `enforce`）
3. **大仓扫描退化**：10 万+ 文件的仓库，arch-check 可能从 200ms 退化到 5s+。需要增量架构扫描（只扫描 changed files 的依赖图）

---

### 方向 3：通知/推送基础设施（让 24h 无人值守能触达人类）

**当前状态**：
零通知代码。整个代码库中唯一与「通知」相关的模式是 `secret-scan.mjs` 的 `slack-token` 检测模式——它扫描 Slack token 是为了防止泄露，不是用来发通知的。

一个 24h 的 `forge evolve`：

```
开始：用户 SSH 进去，敲 forge evolve build --executor command --agent-cmd claude
中午：evolve 遇到 human_gate，停在「awaiting human approval」
下午：用户没看终端 → evolve 就那么等着
晚上：evolve 收敛了，exit 0
      但用户不知道 → 第二天才发现「哦，昨晚就完成了」
```

**建议方案**（最小可行，不引入外部依赖）：

```yaml
# .agent/notifications.yml
notifications:
  on_converged:
    - type: slack
      webhook_url: "${SLACK_WEBHOOK_URL}"
      template: |
        ✅ *forge evolve converged*
        Workflow: {workflow}
        Mode: {mode}
        Iterations: {iterations}
        Cost: ${cost_usd}
        Roadmap: {roadmap_completion}%
  on_human_gate:
    - type: slack
      template: |
        ⏳ *Human approval needed*
        Stage: {stage}
        Run `forge run {workflow} --approved` to continue
  on_failed:
    - type: slack
      template: |
        ❌ *forge evolve failed*
        Error: {error}
```

实现方式：不增加 forge-core 的依赖。在 `cmd/forge` 层加一个 `notifier` goroutine，在关键事件点（converged、human_gate、failed）通过 HTTP POST 发送 JSON payload。纯 Go 标准库 `net/http`，零外部依赖。

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **24h 无人值守的前提** | 如果系统跑 24h 却不能触达人类，它本质上不是「无人值守」而是「人不需要守着但得隔一会回来看看」——这不是 autonomous，这是 batch |
| **human_gate 的激活条件** | 当前 human_gate 需要人盯终端输出才能知道「我在等你批准」。没有通知机制，最高杠杆闸门（Human Approval）的 UX 几乎不可用——人不知道在等他 |
| **成本透明度** | 一个 evolve 跑了 50 轮，花了 $42。用户在当前模式下只能 SSH 进去看 `forge status`。一个推送消息「evolve converged, cost=$42.50, roadmap=100%」是产品的基本体验 |

**边界情况**：

1. **通知失败**：Slack webhook 挂了 → 需要退避重试 + log fallback
2. **通知风暴**：一个 evolve 在 5 分钟内 fail 了 3 次 → 5 条 Slack 消息。需要 dedup 或 rate limit
3. **凭证安全**：webhook URL 不能硬编码到 project.yml。需要 `${ENV_VAR}` 替换或 forge 自身的 credential store
4. **多通道**：Slack + email + webhook 同时通知——需要声明式配置，不是硬编码

---

### 方向 4：AI-SDLC 角色卡与 ForgeOS Agent 卡的桥接（`.ai/` → `.agent/`）

**当前状态**：
`.ai/` 有人工 SDLC 模板体系（10 阶段 × 17 角色 × 4 共享上下文 × 评审产物），`.agent/workflows/` 有自动编排体系（5 工作流 × 9 agent 卡）。它们是同一套流程的两个副本，但**零桥接**。

具体来说：
- `.ai/prompts/01-architecture-review.md` 的内容与 `.agent/agents/architect.md` 高度重叠，但手工维护两个副本，会漂移
- `.ai/prompts/07-sprint-planning.md` 的角色列表比 `.agent/agents/planner.md` 丰富得多（包含了 Business Analyst、UX Designer 等角色），但这些角色在 ForgeOS 中没有对应的 agent 卡
- `.agent/workflows/review.yml` 跑 4 个 phase 的评审（security → distributed → performance → executive），但 `.ai/` 的模板体系有 5 个独立阶段的评审模板（Stage 0-4 各有评审模板），粒度更细

**建议方案**：

```
方向 A（近期，零架构风险）： 
  在 forge-core 中加一个「角色卡对齐检查器」：
  .agent/agents/*.md 中声明的角色 → 检查 .ai/prompts/ 中是否有对应的阶段模板
  如果覆盖率低于 60%，输出 warning
  
  同时生成 .ai/ -> .agent/ 的交叉引用报告：
  forge validate --ai  # 新增子命令
  
方向 B（中期，高价值）：
  将 .ai/prompts/ 的 SDLC 阶段直接映射为 ForgeOS workflow phase：
  把 17 个角色当成可用的 agent 池——不只是当前 9 个 agent card
  
  新 workflow：sdlc.yml
  phases:
    - name: stage-0-product-discovery
      agent: product-manager
      prompt_template: .ai/prompts/00-product-discovery.md
    - name: stage-1-architecture-review
      agent: architect
      prompt_template: .ai/prompts/01-architecture-review.md
    ...
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **资产复用** | `.ai/` 的 10 个阶段模板是已经验证过的 SDLC 流程，ForgeOS 应该消费它而不是复述它。当前两个副本是重复劳动和 drift 源头 |
| **角色扩展** | ForgeOS 当前只用了 9 个 agent 角色，但 `.ai/` 定义了 17 个。UX Designer、Business Analyst、Database Architect 等角色缺失意味着 ForgeOS 在需求发现和架构设计阶段的深度受限 |
| **G1 的深层兑现** | Project.md 的 G1 是「需求自动发现」。17 角色中的 Product Manager、Business Analyst、UX Designer 正是需求发现阶段需要的能力——当前只有 1 个 `product-manager` agent 卡覆盖了这三个角色 |

**边界情况**：

1. **角色重叠**：`.ai/` 有 Backend Architect 和 Solution Architect，`.agent/` 只有 architect。角色合并还是拆分需要决策
2. **模板格式差异**：`.ai/prompts/` 的 prompt 模板是为「人读、人粘贴到 Claude」优化的。直接喂给 agent 作为 system prompt 可能太长或结构不匹配
3. **所有权漂移**：谁维护 `.ai/`？谁维护 `.agent/agents/`？没有桥接机制，两个副本必然 drift

---

## 完整优先级矩阵

| 方向 | 影响面 | 实施成本 | 前置依赖 | 推荐时序 |
|------|--------|----------|---------|---------|
| **1. 收敛指标注册系统** | 正确性：高（2 个 workflow 不能收敛） | **低** | 无 | **Sprint n** |
| **2. 运行时架构执法** | 成本/质量：中-高 | 低-中 | 无 | Sprint n+1 |
| **3. 通知/推送基础设施** | 产品/UX：高 | 低-中 | 无 | **Sprint n** |
| **4. AI-SDLC 桥接** | 资产复用/功能完整：中 | 中-高 | 方向 1（角色对齐检查器） | Sprint n+2 |

**建议的立即行动**（可在 1-2 天内完成）：

1. **修 `requirement_confidence` 和 `review_status` 缺位** — 在 `converge.go` 的 `evalOne` 中加两个 `case`。即使 metric 的值当前来自简单的信号转换（而非 LLM），也远比「永远 NOT MET」正确
2. **通知第一版** — `cmd/forge` 中加一个 `notifySlack(text)` 函数。在 `execEngine` 结束时调用 `notifySlack("forge run completed: ...")`。基于环境变量 `SLACK_WEBHOOK_URL` 可选启用。~50 行 Go 代码

---

## 一条不太明显但很重要的观察

ForgeOS 对「收敛」的定义到今天仍然是**二元状态**：`Converge()` 返回 `(results, met bool)`。一个 workflow 要么收敛了要么没收敛。

但对于一个长期演化的系统，更自然的模型是**连续收敛度**：Roadmap 从 60% → 75% → 88% → 96% → 100%，每一步都是进步的。当前的二元 `met bool` 把「88% 完成、gate 全绿」等同于「60% 完成、一个 gate 红」——两者都是 NOT MET。

如果 `Converge` 返回一个 `completeness float64` 而不是 `met bool`，loop 可以感知到「虽然在收敛但不是 100%，继续」，并且可以把 `staleCount` 的逻辑从「cur == prev → stale」升级为「cur - prev < 阈值 → stale」，更精确地判断死循环 vs 缓慢进展。

这超出了本次分析的 4 个方向的范围，但值得作为远期架构思考记录下来。

---

*分析日期：2026-06-30 | 第三次全量扫描，刻意避开所有已有分析的主题*  
*主要发现：`requirement_confidence` / `review_status` 无法收敛 —— 这是一个真实的、可验证的功能缺口*
