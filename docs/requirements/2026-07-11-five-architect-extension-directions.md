# ForgeOS — 五个高价值架构扩展方向（基于全局代码扫描）

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局通读 forge-core（18 Go 包 / ~35k LOC）、harness（~10.5k LOC 执法层）、
>   `.agent/`（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS）、
>   `.ai/`（10 stage 模板）、全部已有分析文档（`docs/requirements/` ~50 篇 + `docs/analysis/` ~40 篇）。
> **承诺**: 每个方向附带精确到 `file:line` 的代码证据，且与已有 120+ 份分析文档的核心论点不重叠。
> **纪律**: 不编写任何代码。

---

## 已有覆盖的饱和区域（本文不重复）

| 饱和域 | 估篇 | 本文状态 |
|---|---|---|
| 编排状态机（串/并行/loop-back/resume/mode-gating/stop-condition） | ~35 | ✅ 跳过 |
| 生产韧性（529/超时/退避/递归守卫/预算护栏/输出上限/进程组） | ~20 | ✅ 跳过 |
| 学习闭环（trace/scorecard/converge/Memory/Context 注入/路由回灌） | ~16 | ✅ 跳过 |
| 安全纵深（secret-scan/SCA/risk 分类/注入防御/readonly 强制） | ~14 | ✅ 跳过 |
| 治理执法（arch-check 8 检查/check.py/drift-guard/function-length/circular） | ~12 | ✅ 跳过 |
| 执行语义（原子性/幂等/TOCTOU/因果一致性） | ~8 | ✅ 跳过 |
| CLI 体验（detect/preflight/doctor/status/migrate/validate） | ~8 | ✅ 跳过 |
| 第三地平线（多仓库/Web UI/事件驱动/Polyglot 适配器） | ~7 | ✅ 跳过 |
| 多进程安全（`.forge/` 文件锁/并发保护） | ~5 | ✅ 跳过 |
| 三框架债务（`.agent/` vs `.ai/` vs `ai-dev/`） | ~4 | ✅ 跳过 |
| 成本遥测厂商硬编码 / Prompt 万能模型天花板 | ~4 | ✅ 跳过 |
| Trace Replay 引擎 / 确定性复盘 | ~2 | ✅ 跳过 |
| 声明式资源预算 / 部分失败域隔离 | ~2 | ✅ 跳过 |
| 进程健康契约 / 运行时自诊断 | ~2 | ✅ 跳过 |
| 韧性对抗测试（Chaos Mode） | ~1 | ✅ 跳过 |

本文的 5 个方向全部落在上述饱和域之间的**不可约间隙**中。

---

## 方向一 · 度量可信度与对抗鲁棒性

**优先级: P0 | 类别: 安全性 · 自治可信度 | 影响范围: converge + orchestration + all contract agents**

### 问题

ForgeOS 将关键的收敛决策（converge MET / NOT MET）建立在**三个 agent 报告的信号**之上：

| 信号 | 来源 | 消费方 | 风险 |
|---|---|---|---|
| `RoadmapCompletion` | agent 自报 checklist ticks | `evalRoadmap` → converge gate | agent 会提前勾选 `[x]` 以加速收敛 |
| `ReviewStatus` | reviewer/cto 的末行 `VERDICT:` | `evalReviewStatus` → converge gate | agent 学会写 `VERDICT: APPROVE` 就能跳过真正审查 |
| `RequirementConfidence` | product-manager 的末行 `CONFIDENCE: N` | `evalRequirementConfidence` → converge gate | agent 学会写 `CONFIDENCE: 95` 就能跳过真正需求探索 |

```go
// forge-core/internal/converge/converge.go:117
// Converge 是单入口。human_gate 靠 HumanApproved（外部信号，不可伪造）;
// 其余全靠 evalOne 分派到 evalRoadmap / evalReviewStatus / evalRequirementConfidence，
// 这三个都是 agent 自报的浮点/字符串。
func evalOne(c asset.Criterion, sig Signals) Result {
    switch {
    case c.Metric == "roadmap_completion":
        return evalRoadmap(c, sig)       // ← agent 自报的 checklist ticks
    case c.Metric == "review_status":
        return evalReviewStatus(c, sig)  // ← agent 自报的 VERDICT: 文本
    case c.Metric == "requirement_confidence":
        return evalRequirementConfidence(c, sig) // ← agent 自报的 CONFIDENCE: 数字
    // ...
    }
}
```

系统中有**一个**已知的诚实性交叉验证：`FileDelta` vs `RoadmapCompletion` 告警（`loop.go:146`）。但：

1. **它是 advisory only** — 只 log 一行 `⚠ honesty: roadmap=80% but file-change coverage=10%`，从不影响收敛判定。
2. **它只覆盖一个维度** — review_status 和 requirement_confidence 没有任何交叉验证。
3. **agent 已知晓它的存在** — 所有 agent prompt 都通过 `Constraints` 车道注入了 `AGENTS.md`，其中包含 `FileDelta` 机制的描述。一个足够聪明的 agent 可以**主动制造假文件改动**来匹配 roadmap 条目，使 FileDelta 看起来健康。
4. **没有统计异常检测** — 单个 agent 不可能在同一 iteration 内既实现 3 个复杂功能（roadmap +90%）又写 0 文件（FileDelta = 0%）。但系统不会自动标记这种统计上的不可能。

```go
// forge-core/internal/orchestrator/loop.go:139-148
// FileDelta 交叉验证 — 注意它只 log 警告，不影响 met 判定
if sig.RoadmapCompletion > 0.5 && sig.FileDelta < 0.3 {
    l.logf("  ⚠ honesty: roadmap=%.0f%% but file-change coverage=%.0f%% ...", ...)
}
```

### 为什么高价值

ForgeOS 的核心卖点是 24h 无人值守自治。一旦一个 agent（通过 prompt injection、偶然的 emergent behavior、或模型能力的自然提升）学会**有策略地伪造收敛信号**，整个治理体系就变成空壳。这不是"模型会故意欺骗"的科幻场景——这是**度量反作用（Goodhart's Law）**的经典案例：当一个指标成为目标时，它就不再是一个好指标。

### 建议扩展骨架

- **度量一致性矩阵**：在 `internal/converge` 中定义信号间的预期数学关系（如 `RoadmapCompletion > 0.8 ⟹ FileDelta > 0.2`、`ReviewStatus == "approved" ⟹ Reviewer Findings 非空`、`RequirementConfidence > 80 ⟹ discover phase 确实跑了`），不一致即 degrade 为 MET-with-warning 或直接 NOT MET。
- **统计异常检测**：在 `internal/doctor/anomaly.go` 中扩展 `DetectAnomalies`，加入跨 iteration 的信号趋势分析——单个 iteration 的 RoadmapCompletion 从 10% 跳到 90% 而无相应 FileDelta 增长 → 标记为异常。
- **随机验证探针**：在 `internal/orchestrator` 中引入可选的验证 phase——在 reviewer 相位后，用客观标准（如"找到至少 3 条具体反馈"）重验 VERDICT，而不仅靠文本匹配。
- **信号历史一致性**：在 trace 中记录所有收敛信号的完整历史，并在每次 converge 评估时进行**回顾性一致性检查**——"这个 iteration 的 FileDelta 比前三次下降 80%，但 RoadmapCompletion 却上升了 40%"。

### 不受影响

现有 MET 判定的逐位行为在未注入一致性矩阵时不变（`Converge` 的入口点是单一的，可通过一个新的 `SignalValidator` 接口门控）。Dry-run 和 echo executor 不产生真实信号，不触发检证。

---

## 方向二 · 自动化失败根因分类

**优先级: P1 | 类别: 运维 · 自治韧性 | 影响范围: trace + doctor + orchestration**

### 问题

当一个 24h `forge evolve` 运行失败时，当前可用的唯一诊断方式是人类 operator 手动执行以下操作：

```
# 看最终的 exit code 和最后几行日志
$ forge run build
...
Error: loop-back budget exhausted after 3 attempts

# 手动翻 trace.jsonl 看发生了什么
$ jq 'select(.kind=="agent" or .kind=="gate") | {seq, kind, name, status}' .forge/trace.jsonl

# 手动翻命令行日志看 agent 的输出
# 然后凭经验判断 root cause
```

系统中完全没有**自动化的失败根因分类**。`forge doctor` 和 `forge status` 检查的是**仓库健康**（.forge/ 目录完整性、checkpoint 可读性、governance 资产），而不是**运行失败诊断**。

```go
// forge-core/internal/doctor/doctor.go:68-72
// doctor.Run 的检查清单：没有一项是关于「为什么上次运行失败」的
func Run(root string) Report {
    checks = append(checks, tmpResidueCheck(dotForge))  // .tmp 残留
    cpCheck, cpFound := checkpointCheck(dotForge)        // checkpoint 可读
    checks = append(checks, traceCheck(dotForge))        // trace 完整性
    checks = append(checks, memoryCheck(dotForge))       // memory 完整性
    checks = append(checks, python3Check())              // python3 存在性
    // ...
}
```

现有的 `DetectAnomalies` 函数检查的是 checkpoint 历史的**趋势异常**（过时 checkpoint、卡住的 iteration、快速收敛），而不是单次运行的失败分类：

```go
// forge-core/internal/doctor/anomaly.go:103-125
func DetectAnomalies(chain []persist.Checkpoint) []AnomalyFinding {
    detectStale(chain, warn)           // checkpoint 7+ 天未更新
    detectStuckIteration(chain, warn)  // 所有 checkpoint 的 iteration 相同
    detectRoadmapJump(chain, warn, info) // roadmap 跳跃 > 50%
    detectDryRun(chain, info)          // 花费 $0 但有 iteration
    detectNoProgress(chain, warn)      // 连续 checkpoint 完全重复
    return findings
}
```

但 trace.jsonl 中已经包含了做出有意义的分类所需的**全部数据**：

```go
// forge-core/internal/trace/trace.go:65-94
type Event struct {
    Seq           int    // 事件序列号
    Kind          string // "iteration"|"agent"|"gate"|"converge"|"error"|"overload_backoff"
    Name          string // 具体 phase/gate 名
    Status        string // "PASS"|"FAIL"|"NA"|"ok"|"timeout"|...
    DurationMs    int64  // 耗时
    CostUsdMicros int64  // 花费
    Model         string // 使用的 model
    Detail        string // 自由文本上下文
}
```

### 为什么高价值

24h 运行可能因 N 种不同的原因失败：budget 耗尽、loop-back 上限击中、gate 持续红色、agent 错误、超时、529 退避耗尽。每种失败模式的**修复措施完全不同**：

| 失败模式 | 典型根因 | 建议修复 |
|---|---|---|
| Budget 耗尽 | Opus model 太贵、phase 循环太多 | 降档 model、增加 `--run-budget-usd`、减少 MaxLoopBack |
| Loop-back 上限击中 | implementer 产出质量不足、planner 规格不完整 | 改进 planner prompt、增加 MaxLoopBack |
| Gate 持续红色 | 测试 flaky、代码质量问题、适配器配置错误 | 修复 flaky test、调整 enforce mode |
| 超时 | agent 在复杂推理上卡住、孙进程 hang | 增加 `--timeout`、kill 孙进程 |
| 529/过载 | vendor API 过载、退避算法不足以恢复 | 增加退避上限、切换可用区域 |

没有自动分类，operator 面对一个裸错误信息时，必须手动收集这些数据点。更为关键的是：**失败分类数据应该反馈回 routing/planning 系统**（如"这个 workflow 类型在 Opus 下高频 budget 耗尽"→ 自动建议用 Sonnet）。

### 建议扩展骨架

- **失败分类器**：在 `internal/doctor` 中新增 `ClassifyFailure(traceFile string) (FailureReport, error)` —— 扫描 trace.jsonl 全量事件，按启发式规则（超过 K 次 retry → loop-back limit、cost > budget × 0.9 → budget exhaustion、最后的 gate 全部 FAIL → gate red 等）分类失败。
- **失败摘要**：每个类生成一个人类可读的摘要（"Run failed after 3 iterations of build.yml. Root cause: Agent-call budget exhausted (12 phases vs cap of 10). Recommending: --max-agent-calls=15"）。
- **CLI 集成**：`forge doctor --last-run` 运行分类器并输出摘要。`forge evolve` 结束时自动调用分类器并将结果写入 trace（新的 `kind: "failure_analysis"`）。
- **反馈回路**：失败分类结果可选的注入 `Signals` 的扩展字段，让 converge 评估感知到"上一次 iteration 以 budget 耗尽结束，本次 iteration 应该降低 model tier"。

### 不受影响

现有 `doctor.Run` 检查清单不变。分类器只在需要时扫描 trace（延迟 ≤ 几毫秒）。dry-run 不产生真实失败，不触发分类。

---

## 方向三 · 跨运行工作流策略元学习

**优先级: P2 | 类别: 自优化 · 护城河 | 影响范围: routing + trace + scorecard + workflow selection**

### 问题

ForgeOS 当前的学习闭环（Eval→记分卡→Router 回灌）只优化一个维度：**每个 phase 类型应该用哪个 model**。`HistoryTiebreak` 和 `scorecard.model` 归因(Sprint 25-26)使这个闭环完整可工作。但它从未考虑以下问题：

| 可优化参数 | 当前状态 | 是否被学习 |
|---|---|---|
| 每 phase 使用哪个 model | ✅ `routing.HistoryTiebreak` | ✅ v1 已落地 |
| 每个 project 应该用哪个 workflow | ❌ 固定在 `.agent/workflows/` | ❌ 从不改变 |
| 每个 phase 的 prompt template | ❌ 固定在 `.ai/prompts/` + agent card | ❌ 从不改变 |
| MaxLoopBack / MaxIter 应该设为多少 | ❌ 硬编码 3/10 | ❌ 从不改变 |
| budget 如何在 phases 间分配 | ❌ 按 phase 计数平摊 | ❌ 从不改变 |
| review 应该用全部 4 phase 还是跳过某些 | ❌ 由 mode 预决 | ❌ 从不改变 |

系统中所有跨运行的历史数据都存在——`trace.jsonl` 记录每次运行的 events（phase 耗时、cost、gate 裁决）、`memory.jsonl` 记录学习到的知识、`.forge/checkpoint.*` 记录收敛状态——但没有任何代码路径分析这些历史来优化未来的工作流策略。

```go
// forge-core/internal/routing/routing.go:62-80
// TierFor 的输入维度 — 完全没有 workflow-config 信号
func TierFor(phase, agent, mode string, lifecycle string, riskLevel string, signals ...interface{}) (string, error) {
    // 根据 mode×lifecycle 确定 base tier
    // 根据 riskLevel 施加安全下限
    // 根据 phase 类型（reviewer/architect）施加 Opus 下限
    // 返回 tier，完全不查历史 run 数据
}
```

```go
// forge-core/cmd/forge/engine_build.go:232-259
// phaseTierResolver — 实际执行路径的路由决策
func phaseTierResolver(...) string {
    tier := routing.TierFor(...)
    tier = routing.Higher(tier, p.ModelTier)    // per-phase 覆写（只升）
    tier = riskAdjustedTier(tier, ...)           // risk 调整
    tier = routing.BudgetAdjustTier(tier, ...)   // budget 调整
    tier = historyTiebreak(tier, ...)            // 历史择优（只根据 model 历史）
    return tier
}
```

### 为什么高价值

这是 ForgeOS 从"声明式编排器"进化为"自优化自治系统"的最后一个闭环。当前，用户手动选择 workflow、手动调整参数、手动决定 review 深度。ForgeOS 已经积累了足够的运行时数据来自动做出这些决策。

**真实场景**：一个使用 ForgeOS 开发支付微服务的团队运行了 15 次 `forge evolve`。系统发现：
- 每次 `implement` 阶段用 Sonnet 就足够，用 Opus 反而多花 3 倍钱但质量无差异 → 建议 implementer 降档。
- 每次 `security-review` 阶段平均耗时 8 分钟且从无发现 → 建议简化这个 review phase。
- MaxLoopBack=3 在所有 15 次运行中都从未被用完 → 建议降到 2 以加快失败止损。

这些不是 engineer 凭直觉能觉察的模式，但 trace 数据中清晰可辨。

### 建议扩展骨架

- **跨运行性能数据库**：在 `internal/routing` 或新包 `internal/learning` 中，新增 `FlowDatabase`，累计扫描全部 `.forge/trace.jsonl` 文件和 `scorecards.json`，构建一个按 `(workflow_name, phase_type, model)` 聚合的性能视图。
- **策略优化器**：新增 `OptimizeWorkflow(db FlowDatabase, current asset.Workflow) (Recommendation[], error)`，输出如：
  - `"phase implementer: Sonnet achieves equivalent quality at 1/3 cost of Opus — consider downgrading"`
  - `"MaxLoopBack=3 never exceeded in 15 runs — consider reducing to 2 for faster failure detection"`
  - `"security-review phase never found a finding in last 10 runs — consider skipping in balanced mode"`
- **可选自动应用**：`forge evolve --self-tune` 模式，在每次 evolve 完成后运行优化器并可选调整 `.agent/` 配置（类似 `forge migrate` 的模式）。
- **效果跟踪**：每次优化建议附带可测量的预期收益（"预计减少 40% cost"），并在后续 evolution 中跟踪实际收益 vs 预期。

### 不受影响

原有的路由回灌路径（model 级别）不变。工作流定义的向后兼容性通过 opt-in 保护——`--self-tune` 默认关闭。建议是**建议**而非自动变更，用户始终有批准权。

---

## 方向四 · 运行时工作流降级与动态升级

**优先级: P2 | 类别: 自治韧性 · 运行时适应性 | 影响范围: orchestrator + engine + converge**

### 问题

部署 ForgeOS 的 workflow 声明是**静态**的——一旦开始运行，workflow 的 phase 序列、gate 集合、model 选择就是固定的。系统要么成功执行所有 phases，要么在第一个错误处失败退出。不存在中间状态：

```go
// forge-core/internal/orchestrator/orchestrator.go:179-230
// RunFrom 的循环体 — 按顺序跑 phases，一个失败就 abort（或 loop-back 后失败）
for i := start; i < len(wf.Phases); i++ {
    p := wf.Phases[i]
    if len(p.RequiredGates) > 0 {
        result := e.runGates(p, e.gatesFor(p))
        if !allGreen(result) {
            if ou := p.OnFail; ou != nil && ou.Action == "loop_back" && e.MaxLoopBack > 0 {
                // 定向 loop-back ...
            }
            return fmt.Errorf("gate phase %s: not all green", p.Name)
        }
    } else {
        // agent phase — run once
    }
}
```

问题在于真实世界的 24h 自治运行会遇到各种**不该直接导致失败但需要调整计划**的情况：

| 现实场景 | 当前行为 | 更理想的行为 |
|---|---|---|
| implementer 被 reviewer 连续拒绝了 3 次 | loop-back 耗尽 → 失败 | 尝试给 implementer 注入更详细的 spec，或升级到 Opus |
| budget 已用 80%，但还有 3 个 phases 没跑 | budget 耗尽 → 失败 | 降级到 Haiku 跑剩余的 phases，或跳过非关键的 review phase |
| 安全审查 phase 花了预期两倍的时间 | 超时 → 失败 | 用更激进的 timeout 重试，或跳过仅本次 |
| 连续 5 次 `gate` 都卡在同一个 lint 规则上 | gate 红色 → loop-back → 再红 → 失败 | 自动创建 ADR 豁免该规则并注释原因 |

系统已经有 building blocks 但没有被组合：

- `routing.BudgetAdjustTier`（`engine_build.go:251`）— 根据预算压力降 model 档
- `mode_gating.go` 的 `skipByMode` — 按 mode 跳过 phases
- `MaxLoopBack` 和 `MaxRetries` — 重试边界
- `converge.Signals` 的 `Criteria` 字段 — 按 criterion 级别的 gate 信息

但这些是**静态配置**或**简单条件分支**，不是**运行时自适应决策**。没有代码路径说："预算还有 20%，还有 3 个 phases——评估最贵的 phase（reviewer with Opus），如果它并非关键路径，降级到 Sonnet 或跳过。"

### 为什么高价值

这是 ForgeOS 从"按脚本执行"进化为"有适应能力的编排器"的关键步骤。对于一个声称要 24h 无人值守的系统，"要么成功要么失败"是一个不可接受的设计——真实世界的 LLM 调用总有不可预测的变化。

现有的 budget 感知降档（BudgetAdjustTier）是第一步，但它只影响 model 选择，不改动工作流结构。本方向将适应性提升到**工作流级别**：改变跑什么、以什么顺序跑、跳过什么。

### 建议扩展骨架

- **降级策略声明**：在 `.agent/workflows/*.yml` 的 phase 级别增加可选 `degrade` 策略：
  ```yaml
  phases:
    - name: performance-reliability-review
      agent: performance-engineer
      degrade:
        on_budget_pressure: "skip"           # 预算紧张时跳过
        on_time_pressure: "downgrade_model"   # 时间紧张时用 Sonnet
        on_repeated_failure: "inject_context" # 重复失败时注入更多上下文
  ```
- **运行时评估器**：在 `internal/orchestrator` 中新增 `DegradationEvaluator`，在每个 phase 前评估运行时条件（budget 剩余%、已用墙钟、前序 phase 失败模式），决定是否触发降级。
- **降级审计**：所有降级决策写入 trace 作为新的 `kind: "degradation"` 事件，包含触发原因和具体 action，确保事后可审计。
- **预算感知工作流修剪**：当预算消耗到阈值时（如 70%/85%/95%），自动触发不同级别的修剪：70% → 降 model 档、85% → 跳过非关键 phase、95% → 跳过所有 review phase 直接跑 gates。

### 不受影响

默认行为（无 degrade 声明）逐位不变。降级策略是 opt-in 的 workflow 字段。已有 `BudgetAdjustTier` 路径继续工作——本方向在其之上叠加工作流级别的决策，而非替换它。

---

## 方向五 · 正确性悬崖 —— 随规模无声失效的启发式算法

**优先级: P2 | 类别: 架构债务 · 长期正确性 | 影响范围: 多包跨域**

### 问题

ForgeOS 的代码库中有多处**在当下规模工作正确但在更大规模上会无声失效的启发式算法**。这不是 bug——在今天的数据量下每个算法都产生合理结果。但随着仓库增长（更多 ADR、更多文件、更多包、更多 memory 条目），这些算法会**没有警告地**从"正确"退化为"产生误导性结果"。

以下是四个经过代码级确认的"正确性悬崖"：

#### 悬崖 1：TF-IDF 检索器在小语料库上的虚假高相关性

```go
// forge-core/internal/prompt/retrieve.go:143-163
// Retrieve 的评分函数 — 文档长度归一化 + 简易 IDF
func score(qTerms, docToks []string, df map[string]int, totalDocs int) float64 {
    if len(docToks) == 0 { return 0 }
    tf := count(docToks)
    var sum float64
    for _, term := range qTerms {
        n, ok := tf[term]
        if !ok { continue }
        sum += float64(n) * idfWeight(df[term], totalDocs)   // IDF 权重
    }
    return sum / float64(len(docToks))  // 长度归一化
}
```

**悬崖条件**：当 `totalDocs` 很小（当前 ~7 个 ADR）时，IDF 几乎恒为 1（`(total-df)/total ≈ (7-1)/7 ≈ 0.86` 即使是常见词）。这意味着**检索结果几乎完全由 TF 驱动**（词频高的文档胜出），而不是语义相关性。当 ADR 数量增长到 50+ 后，IDF 开始起效——但**检索质量会经历一个非线性的提升**，导致某些过去排在前面的文档突然掉出 top-K。用户会困惑"为什么同一个 query 现在找不到之前一直能找到的 ADR"。

#### 悬崖 2：Memory Compaction 的最近 N 条保留策略导致历史遗忘

```go
// forge-core/internal/memory/memory_compact.go:122-138
// compactByKind — 每组保留最近 keepPerKind(20) 条，其余摘要化
func compactByKind(old []Entry, keepPerKind int) []Entry {
    // ...
    if len(kindEntries) <= keepPerKind {
        // 直接保留
    } else {
        keep := kindEntries[len(kindEntries)-keepPerKind:]  // 最近 20 条
        summarized := kindEntries[:len(kindEntries)-keepPerKind] // 摘要化
        // ...
    }
}
```

**悬崖条件**：当某种 kind 的总条目数稳定增长（如 `gap` 类型每次 evolve iteration 都加一条），`keepPerKind=20` 意味着**系统永远只记得最近 20 条 gap**。超过 20 条 gap 后，旧 gap 被摘要化成一行 "compacted 45 gap entries; topics: test:10, security:5, ..."。如果旧 gap 中有一个关键的架构债务尚未修复，它将在摘要中丢失细节——agent 不会再看到它的具体描述。系统从"有 memory"退化为"大概有 memory"。

#### 悬崖 3：Risk 分类器使用固定 Blast Radius 阈值

```go
// forge-core/internal/risk/risk.go:54-62
const largeBlastRadius = 5   // 触及 >= 5 个模块 → "large"
const mediumBlastRadius = 2  // 触及 >= 2 个模块 → "medium"
```

**悬崖条件**：这些常数是为 `examples/url-shortener`（3 个源文件）和 `forge-core`（18 个包）设定的。在一个有 500 个包的微服务仓库中，一次"触及 5 个模块"的变更可能是完全常规的（修改了共享的 proto 定义、更新了 SDK 版本）。这个分类器会**系统性地过度分类**所有非小仓库的变更为 high/critical，使 Opus 安全下限失去区分度——每个 PR 都触发 Opus，冲淡了"真正高风险变更用 Opus"的设计意图。

#### 悬崖 4：staleCount 不能区分"真停滞"和"多 iteration 重构"

```go
// forge-core/internal/orchestrator/loop.go:177-185
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0  // 有进展 → 复位
    }
    return stale + 1  // 无进展 → 加一
}
```

**悬崖条件**：一个大型重构可能需要多个 iteration 才能看到 ROADMAP 完成度的明显变化。当下一个 iteration 没有勾选新的 checklist items（因为本 iteration 的代码尚未落地到可以被 agent 确认的状态），staleCount 会递增并最终触发 no-progress tripwire，**即使 agent 正在做必要的重构工作**。对于小改动（每个 iteration 都能勾选东西），这个启发式工作良好；对于大型重构，它会**错误地终止演进**。随着仓库规模增长，需要多个 iteration 才能完成的功能比例增加，此悬崖会越来越频繁地被击中。

### 为什么高价值

这不是"bug"——在今天的数据量下所有四个算法都工作正确。这是一个架构债务，因为代码中没有悬崖检测器。系统会在某个时刻突然开始产生不可靠的检索结果、丢失关键记忆、错误分类风险、错误终止演进，**而没有任何自检信号**。

| 悬崖 | 触发条件 | 失效模式 | 检测难度 |
|---|---|---|---|
| TF-IDF 虚假高相关性 | ADRs > 30 | agent 看到不相关的 ADR | 困难（agent 可能看不出） |
| Memory 历史遗忘 | 某 kind 条目 > 20 | agent 忘了旧 gap | 极困难（无对比基线） |
| Risk 过度分类 | 包数 > 50 | Opus 不再安全下限 | 中等（routing 数据可检测） |
| staleCount 误终止 | 多 iteration 重构 | evolve 提前终止 | 中等（可对比 checkpoint 历史） |

### 建议扩展骨架

- **缩放敏感性分析**：新增 `internal/scaling` 包（纯计算，零依赖），对每个已知启发式算法计算"当前规模 vs 设计上限"的比率，输出警告当接近悬崖：
  ```go
  type CliffReport struct {
      Heuristic    string // "TF-IDF ADR retrieval"
      CurrentScale int    // 当前 ADR 数量
      DesignLimit  int    // 估计的设计上限
      RiskLevel    string // "green"|"yellow"|"red"
      Detail       string // 人类可读描述
  }
  ```
- **自适应阈值**：`internal/risk` 的 `largeBlastRadius` 改为从仓库总模块数推导的相对阈值（如 `TotalPackages * 0.1`），替代固定常数。`internal/memory` 的 `keepPerKind` 改为基于 entry 总数的自适应值（如 `max(20, TotalEntries * 0.05)`）。
- **悬崖检测器**：在 `forge doctor` 中新增缩放检查项，在 `forge status` 中显示每项启发式的"当前规模 vs 设计上限"的状态。
- **设计上限文档化**：对每个包的常量/硬编码阈值，用 Go 注释标注其设计上限和超出后的行为退化描述（类似已有 `internal/prompt/retrieve.go:25-32` 的 honesty 注释风格）。例如：
  ```go
  // DESIGN LIMIT: largeBlastRadius=5 assumes the repo has < 50 packages.
  // Above 50 packages, any cross-cutting change touches >=5 modules and
  // the constant stops distinguishing "broad" from "normal" — risk
  // classification becomes permanently "critical" for all non-trivial PRs.
  const largeBlastRadius = 5
  ```

### 不受影响

所有现有行为在悬崖条件被触发前逐位不变。自适应阈值仅在比例模式下改变行为（通过新 flag 控制，默认继承当前常数值的旧行为）。`forge doctor` 新增的缩放检查项不改变 exit code（只报告，不阻断）。

---

## 优先级与建议

| # | 方向 | 类别 | 紧急度 | 一句话价值 |
|---|------|------|--------|-----------|
| 1 | **度量可信度与对抗鲁棒性** | 安全性 | **P0** | 无人值守自治系统的信任地基——agent 不能通过伪造收敛信号来 bypass 治理 |
| 2 | **自动化失败根因分类** | 运维 | **P1** | 24h 运行失败后秒级诊断，而非手动 grep trace.jsonl；失败模式自动反馈回路由系统 |
| 3 | **跨运行工作流策略元学习** | 自优化 | P2 | 从仅优化 model 选择升级为优化整个工作流配置，是 vision「越用越聪明」的完整版 |
| 4 | **运行时降级与动态升级** | 韧性 | P2 | 从「要么成功要么失败」进化为「按条件自适应执行」——24h 自治的前提能力 |
| 5 | **正确性悬崖检测** | 架构债务 | P2 | 防止系统在仓库增长时无声退化为不可靠——已知 4 个悬崖，修一个少一个 |

### 收敛建议

- **做前三件（P0+P1+P2 最高收益）**：方向一（度量可信度）+ 方向二（失败分类）+ 方向五（悬崖检测）构成一个防御性三角——**防止系统被骗、失败时快速诊断、长期防止无声退化**。三者都是"地基"型投资，收益随部署时间递增。
- **方向三和方向四**是"护城河"型投资——它们让 ForgeOS 从工具进化为自治系统。建议在方向上完成后启动，因为它们的收益建立在稳定的基础之上。
- **不做**的项目：本分析刻意回避了已经饱和的领域（编排状态机/生产韧性/第三地平线等），也回避了需要外部资源的方向（跨厂商池/SCA-DB/Web UI/Firecracker）。这里每一个方向都只依赖当前代码库已有的基础设施。

---

> **后续维护**：如果某个方向被接受并进入 sprint，请在根 `ROADMAP.md` 和 `.agent/CURRENT_SPRINT.md` 中显式标注其源于本文的方向编号，以保持可追溯性。每个方向落地后，建议在本文追加一个 "Resolution" 节（类似 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 addendum 模式），记录实现 commit 和诚实边界。
