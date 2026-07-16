# ForgeOS — 全局扫描后识别的新扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 —— forge-core 18 Go 包 / 195+ 源文件 / 707+ 测试 /  
>   harness 39 模块 / `.agent/` 完整骨架（12 agent 卡 + 9 skill 卡 + 5 workflow）/  
>   Sprint 1–31 完整演进 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90 DONE + GAP 全部收口）/  
>   **交叉核查 40+ 篇 `docs/analysis/*.md` + 14 篇 `docs/requirements/*.md`（~60 个已有方向）**  
> **核心承诺**: 每个方向在全部 ~60 个已有方向中**零覆盖**或不重复其核心论点  
> **纪律**: 不编写任何代码  
> **日期**: 2026-07-09

---

## 已覆盖全景（本文不重复）

| 维度 | 代表文档 | 方向数 |
|------|----------|--------|
| 功能引擎补齐（路由/编排/记忆/收敛/诊断/自适应装配） | `high-value-extension-directions.md`, `v3`, `expansion-production-readiness.md` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级/修正学习） | `expansion-horizon-three.md`, `expansion-gaps-v7-novel.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 双解析器） | `expansion-production-readiness.md` | ~8 |
| 执行语义形式化（原子性/幂等性/因果一致性/版本演化/收敛定量） | `execution-semantic-gaps.md`, `expansion-forgeos-meta-governance.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | `second-order-architectural-gaps.md`, `systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/YAML 分歧/信任边界/持久语义/可移植性） | `strategic-extensions-v22-v32.md`, `uncovered-frontiers-v25.md` | ~10 |
| 安全/凭据/secret 生命周期/沙箱/SCA | `genuinely-novel-expansion-directions.md` | ~5 |
| CLI DX / shell 集成 / daemon 模式 / 增量采纳 / tutorial | `systemic-expansion-v26.md`, `expansion-self-governance.md` | ~5 |
| 并行编排 / 迭代跳过 / 收敛可见性 / YAML 差分测试 | `high-value-extension-directions-v3.md` | ~5 |
| 经济治理 / cost 智能 / 跨运行审计 / 结构化输出协议 | `next-five-frontiers.md`, `forgotten-frontiers-five.md` | ~8 |
| **总计已有覆盖** | | **~60 方向** |

---

## 方向一：工作流组合引擎（Workflow Composition Engine）

**类型**: 编排 · 架构  
**优先级**: P1（关乎 ForgeOS 能否从「单步 CLI 工具」升级为「自治流水线平台」）  
**代码影响**: 新 `internal/composer/` 包 · `cmd/forge` 入口扩展 · 复用 `internal/orchestrator`/`internal/asset`/`internal/converge`

### 现状

ForgeOS 有 5 个独立 workflow YAML（discover → design → review → build → evolve），每个是一个自包含的 spine 阶段。但它们之间的衔接完全靠**操作员手动编排**：

```bash
# 操作员手动执行流水线：
forge run discover --executor command --mode engineering 
# → 等 discover 收敛 → 读输出
forge run design   --executor command --mode engineering
# → 等 human_approval → 用户 --approve
forge run build    --executor command --mode engineering
# → 等 build 收敛 → 🎉
```

代码中：
- `design.yml` 的 `stop_condition.human_approval` + `on_approved.next_stage: "build"` 声明了「审批通过后进 build」的**意图**（`asset.go` 第 225 行 `OnApproved.NextStage`），但 `converge.go` 的 `Converge` 函数对 `on_approved` 仅返回 met/unmet，从不触发下一阶段。
- `cmd/forge/` 的 `runWorkflow` / `cmdEvolve` 均单次调用 `orchestrator.Engine.Run` / `LoopEngine.Run`，没有任何跨 workflow 编排逻辑。
- North-star 架构（`north-star.md` 第 32-38 行）的 `Orchestrator(Temporal)` 描述了一个 durable workflow 引擎，但这是目标态，v2 完全没有增量路径。

### 为什么需要

1. **操作摩擦**: 从 idea 到 production 需要 4-5 个手动 `forge run` 命令 + 等待 + 读输出 + 判断下一步。一个 24h 自治平台的愿景要求一次 `forge evolve` 就能从 discover 直通 build，中间的人审作为 durable wait 嵌入。
2. **收敛丢失**: 当前手动编排下，workflow 间的**信号传递完全断裂**——design 输出的架构决策、review 的批准、discover 的需求置信度，build 阶段一概不知。`feeds_forward` 只在 *phase 之间*工作，不在 *workflow 之间*。
3. **声明意图已有**: `on_approved.next_stage` 字段已在 `asset.StopCondition` 中存在（`asset.go` 第 223-225 行），它是一个「已声明零消费」的字段——同 Sprint 30 审计出的 4 处死字段同类。但它是**唯一一个有实际管道语义**的字段：命名了一个明确的下一阶段名。把它从「死字段」变成「真驱动管道」的增量路径非常清晰。
4. **phase 复用模式已建立**: `LoopEngine.Run` 已经演示了如何驱动一个多迭代收敛循环。Workflow 组合可以看作是外层 LoopEngine——内层单次 Run 的两层嵌套，复用同一套接口。

### 关键设计边界

- **不引入 Temporal / 外部 durable 引擎**: v2 仍是纯 Go stdlib 零依赖。组合引擎用 `converge.Converge` + 文件系统签核标记（已经过 Sprint 31 `on_rejected` 验证的 `.forge/<stage>.approved` 模式）。
- **首次实现只做线性链**: discover→design→build→evolve，不支持分支/并行/条件跳转（那是 v3 north-star）。
- **信号传递通过文件系统**: 前序 workflow 的 `memory.jsonl` + `checkpoint.json` + `trace.jsonl` 天然可供后续 workflow 读入。Composer 只需在链间注入 `--resume` / `--load-memory` 参数。
- **保持 `forge run` 单步兼容**: 组合引擎是**可选入口**（`forge pipeline run`），不等于 `forge run` 的行为。用户仍可单步执行。

---

## 方向二：跨运行知识生命周期管理（Cross-Run Knowledge Lifecycle）

**类型**: 持久化 · 治理 · 数据生命周期  
**优先级**: P2（长期自治运行的关键基础，直接影响 24h+ 运行的可靠性）  
**代码影响**: `internal/memory/` · `internal/persist/` · `internal/trace/` · 新 `internal/retention/`

### 现状

`internal/memory` 包实现了**运行内**的跨迭代知识积累（gap/decision/lesson），采用 JSONL 追加写，有新知识就 Append。但存在两个关键缺口：

1. **无界增长**: `memory.jsonl` 从未自动裁剪。`Prune` 和 `Compact` 函数存在（`memory_compact.go`），但必须显式调用。一个运行 1000 迭代的 evolve 会积累数万条 Entry，每条都要被 `Load` 反序列化、被 `filterSuperseded` 扫描——O(n) 随运行时长线性增长（`memory.go` 第 327-349 行 `decode` + `filterSuperseded`）。
2. **无跨运行继承**: 新 `forge run` 启动时，`memory.Load` 返回 (nil, nil)——从零开始。上一轮运行积累的知识（"payment 模块曾经有竞态条件"、"上次 reviewer 指出测试覆盖率不足"）全部丢失。`trace.jsonl` 和 `checkpoint.json` 同样没有跨运行继承机制。
3. **trace 文件无界**: `trace.jsonl` 在每个 evolve 迭代追加 Events（`trace.go` 第 10-12 行，`kind: iteration/agent/gate/decision/converge/error`）。一个 24h run 可轻松产生数千行。`trace` 包没有任何轮转/压缩/保留策略。

存在一个 `loadCaches sync.Map`（`memory.go` 第 42-49 行）缓存加载的 Entry，但这个缓存也是**永不过期**——它随着程序的运行持续增长，从不收缩。

### 为什么需要

1. **长期运行的退化**: 一个 1000 迭代的 evolve 会在 memory 中积累 ~3000-10000+ 条 Entry。每次 `Load` 全量解析 + 全量过滤，phase 越多此开销越明显。Prune 只按数量裁切（保留最后 N 条），不按时间或按重要性裁切——有价值的历史知识可能在一次大批量 Append 后被裁掉。
2. **飞轮停滞**: ForgeOS 的「学习闭环」愿景（PROJECT.md G5: 「持续演化——Scan→Gap→Roadmap→Implement→Review→Evaluate→Scan」）要求知识能跨运行流动。如果每轮 `forge evolve` 都从空白记忆开始，系统无法识别"这个 gap 上次已分析过"、"这个方案上次试过但失败了"——每次都重新发现同一个问题。
3. **已有基础设施**: `Entry` 结构体已有 `CreatedAtUnix`、`Source`、`Confidence`、`Supersedes` 字段。时间戳驱动的 TTL 策略可以直接复用这些字段，无需新增 schema 变更。`trace.Event` 也有 `DurationMs` + `CostUsdMicros`，可以按时间窗口聚合。
4. **k8s 式数据生命周期是平台级基础**: 如果要支持多项目/多租户（north-star 愿景），每个项目积累的 memory 不能无限膨胀。定义一个 retention policy（TTL / max-entries / adaptive compaction）是平台工程的基本功。

### 关键设计边界

- **不自动继承**: 跨运行继承必须是**显式操作**（如 `forge memory import --from <prev-run-dir>`），不可默认——防止一个运行的有毒 Entry 污染另一个运行。
- **不引入外部存储**: 仍是纯文件系统，不做 Qdrant/PG（north-star 目标态）。
- **TTL 策略配置化**: 通过 `project.yml` 或新 `retention.yml` 配置，非 hardcode。默认 30 天（同 scorecard recency half-life）。

---

## 方向三：Phase 输出契约验证（Phase Output Contract Verification）

**类型**: 正确性 · 治理  
**优先级**: P2（agent 自治边界的重要护栏，防「planner 不写计划、implementer 直接改代码」式信号断裂）  
**代码影响**: 新 `internal/contract/` 包 · `asset.go Phase` 新字段 · `orchestrator` 接线

### 现状

`asset.Phase` 有 `Emits []string` 字段（`asset.go` 第 86 行），列出 phase 声明的产出文件路径（如 `["docs/discovery/requirement-draft.md"]`）。但是：

- Emits 只是**字符串数组**——没有说明「这个文件应该长什么样」。
- 没有任何机制验证一个 phase **实际是否产生了**它声明要产出的文件。`forge run` 不检查文件是否存在。
- 没有任何机制验证产出文件的**结构完整性**——planner 输出的 task-plan.md 是否真的有 Goals / Implementation Plan / Acceptance Criteria 三个章节。

代码证据：

```go
// asset.go 第 86-87 行
Emits []string `json:"emits,omitempty"`
// 只有路径名，没有 schema，没有校验
```

对比之下，agent 卡的机读契约已经建立了先例：

```markdown
<!-- .agent/agents/reviewer.md: VERDICT: APPROVE / VERDICT: REQUEST_CHANGES 机读契约 -->
<!-- .agent/agents/product-manager.md: CONFIDENCE: <0-100> 机读契约 -->
```

Phase 产出需要同样的**声明式可验证契约**，但当前没有任何等价物。

### 为什么需要

1. **自治运行的可审计性**: 当 ForgeOS 24h 无人值守运行时，operator 回来需要确认每个 phase 确实干了它该干的事。如果 planner phase 声明 `emits: [task-plan.md]` 但文件不存在（或内容为空），operator 无从知晓——当前收敛信号（`GatesGreen`、`RoadmapCompletion`）完全不反映产出完整性。
2. **下游 phase 的输入保证**: implementer 依赖 planner 的 task-plan.md。如果该文件缺失或不完整，implementer 在真 agent 场景下会 workaround（自己编一个计划）或烧 budget 问用户——这正是 Sprint 24 修复的「任务注入 gap」的同模式缺口（`prompt_context.go` 被加注三车道，但只注入了 ROADMAP.md，没有注入 phase 间产出）。
3. **已有基础设施可复用**: `emits` 字段已在 Phase 上声明。prompt 注入已有 `prompt_context.go` 的 `injectPhaseOutputs` 路径。contract 验证可以复用 `harness/check.py` 的治理检查模式（纯 Python 脚本，零外部依赖）。
4. **渐进增强路径**: 第一步只检查**文件存在性**（"emits 的文件是否真实存在"），第二步检查**结构签名**（通过正则/关键字检查章节标题），第三步才检查**语义一致性**（需要 LLM 辅助）。每一步都可独立交付。

### 关键设计边界

- **不阻塞执行**: contract 验证是**可观测检查**（记录 `violation` 到 trace + 警告日志），**不阻断 phase 执行**。阻断会导致"一个非关键文件缺失就整条管道卡死"的反面模式。仅在 `forge accept` 中可选为 load-bearing。
- **不发明 schema DSL**: 第一阶段对每个 emitted 文件只检查存在性 + 非空性。第二阶段用简单的 YAML/JSON 契约文件（`.agent/contracts/<phase-name>.yml`）描述结构要求。不重新发明 JSON Schema 或 Cue。
- **从已有 `writes_adr` 模式借鉴**: `writes_adr` 已经演示了如何声明「这个 phase 会写 ADR」并 verify ADR 是否存在——contract 是它的泛化。

---

## 方向四：并行执行资源治理（Concurrent Phase Resource Governance）

**类型**: 可靠性 · 性能  
**优先级**: P1（`forge run --parallel` 已经实现，但没有治理维度——真实用户开启后可能无意识烧穿预算）  
**代码影响**: `internal/orchestrator/parallel.go` · `internal/orchestrator/waves.go` · 新 `internal/parallel/policy.go`

### 现状

`parallel.go` 实现了依赖波次并发执行（第 33-42 行 `runWave`），但**没有任何资源治理**：

- **无并发上限**: 一个 wave 可以包含任意数量的独立 phase。如果 100 个 phase 都声明 `depends_on: []`，它们会被放入同一个 wave 同时执行（`waves.go` 第 37 行 `len(wave) == len(phases)`），100 个 claude 进程同时启动。
- **无 budget 感知**: `budget.go` 的 `checkRunBudget` 是 run-level 总封顶。并行模式下 100 个 phase 同时扣 budget，谁先跑到上限说不清——当前 budget 检查在 `runPhaseParallel` 第 143-153 行的 `mu.Lock()` 下做，是串行化的，但**不保证公平**：一个后启动但先拿到锁的 phase 可能耗掉最后一点 budget，让之前已完成 80% 工作的 phase 在 `checkAgentBudget` 时失败。
- **无重试隔离**: 并行 wave 中一个 phase 遇到 `KindOverloaded` 会调用 `overloadBackoff` 并 sleep（`backoff.go` 第 37-42 行），但**这个 sleep 阻塞了整个 goroutine 但不阻塞 wave**——wave context 不会等待它，其他 phase 继续竞争 backend。多个并行 phase 同时遇到 overload 会**各自独立退避**，失去了聚合退避的效果。
- **无降级策略**: 当 wave 中某些 phase 因 budget/gate 失败，当前实现 `waveCancel()` 终止波次中所有剩余 phase（`parallel.go` 第 109-115 行）。这是 fail-fast 的安全行为，但对于非关键失败的 phase（如 docs 生成），完全丢弃已完成工作的成本可能高于让它们跑完。

### 为什么需要

1. **真实的经济风险**: 100 个并行 claude 调用可能在一个 request 周期内烧掉 $200+ 的 budget。当前没有任何 throttle——`command_executor.go` 的 `cappedBuffer` 只防 OOM 不防并发量。`MaxAgentCalls` 是 run-level 的**总量**限制，不是**速率**限制。
2. **backoff 群聚效应**: 100 个 goroutine 在 2s base 的 exponential backoff 下会产生完美的 thundering herd——它们都以相同的 2s→4s→8s 时序回退（因为 `overloadBackoff` 是确定性函数，无 jitter，`backoff.go` 第 61 行注释明确说「v1 single-run: NO JITTER」）。这恰恰是 overload 场景最不想要的。
3. **已有基础设施部分就绪**: `waves.go` 的拓扑排序已经准备好了 wave 结构，`Phase.DependsOn` 可以表达依赖，但治理策略（max_concurrency、fair_budget、wave_priority）还是零。
4. **生产就绪的必经之路**: 没有 resource governance 的 parallel mode 在单项目小型 pipeline 中可能没有问题（3-5 个独立 implementers），但 ForgeOS 的愿景（north-star 服务目录中的 "Agent Reg & Scheduler: 角色↔宿主映射,调度/bin-pack/配额"）要求它从一开始就正确限额。

### 关键设计边界

- **不引入信号量/rate-limiter 库**: 纯 Go channel + context 实现（同当前 backoff 的零依赖路径）。一个简单的 `maxConcurrency int` 通过限制 goroutine 启动速率（buffered channel per-wave）就覆盖了大部分风险。
- **不改变串行路径**: 治理只影响 `--parallel` 模式。串行 `RunFrom` 不需要任何改变。
- **jitter 可作为独立增量**: 当前 backoff 无 jitter 是刻意的（`backoff.go` 第 61 行注释）。并行模式下加 jitter 是低成本高收益的一次改动，可在 governance 周期内独立交付。

---

## 方向五：失败智能与自动修复建议（Failure Intelligence & Automated Remediation）

**类型**: 可观测性 · 自治  
**优先级**: P2（将 trace 从「事后审计日志」升级为「自愈知识库」）  
**代码影响**: `internal/trace/` 新 query 功能 · 新 `internal/remediation/` 包 · `cmd/forge` 新子命令 `forge diagnose`

### 现状

ForgeOS 收集了丰富的运行时诊断数据，但**只用了一次就被丢弃**：

- `trace.jsonl` 记录了每一个 event 的 `Kind`、`Status`、`DurationMs`、`CostUsdMicros`、`Model`、`Detail`（`trace.go` 第 38-61 行）。
- `trace.Event` 有 `error` kind 记录了每一个 recoverable/fatal 错误（`trace.go` 第 204-205 行 `ErrorEvent`），包括 `["overload"]`、`["timeout"]`、`["config"]` 等分类。
- `backoff.go` 的 overload retry 记录了每一次退避（`trace.go` 第 199-200 行 `OverloadEvent`）和退避时长。
- `orchestrator/exec_error.go` 定义了完善的错误分类体系（`KindOverloaded`、`KindTimeout`、`KindFailed`、`KindConfig`、`KindRecursionLimit`）。

但是：

- **没有跨运行聚合**: `trace.jsonl` 在每一轮 `forge run` 或 `forge evolve` 后被覆盖或追加，但从不阅读上一轮的 trace 数据来做分析。
- **没有失败模式识别**: 如果一个项目持续在同一个 phase（如 `security-review`）遇到 `KindTimeout`，系统不会意识到这是一个模式，也不会建议调参。
- **没有自动修复建议**: backoff 参数（`overloadBackoffBase=2s`、`overloadBackoffCap=60s`、`Engine.MaxRetries`）是**全局 hardcode**（`backoff.go` 第 56-61 行），不根据历史数据自适应。
- **`scorecard` 只关注成功指标**: `scorecard` 系统的 p95_latency、avg_cost_usd 只记录成功 phase 的统计数据，不分析失败模式。

### 为什么需要

1. **自治运行的自适应能力**: 一个 24h 无人值守的 evolve loop 遇到反复 overload 时，如果 backoff 参数是静态的（max 60s），而 backend 恢复需要 90s，系统会在「retry→wait 60s→retry→fail」的死循环中耗尽所有 retry budget。一个有失败智能的系统会注意到「最近 5 次 retry 都失败了」并降低频率或建议 operator 增加预算。
2. **trace 数据的二次利用**: 当前 trace 是「写入后永不读取」的 append-only 日志。同类系统（k8s、Temporal）都有基于事件数据的健康仪表盘和告警。ForgeOS 的 trace 格式已经高度结构化（JSONL，每行自描述），就差一个聚合查询层。
3. **与 converge 信号互补**: `converge.Signals` 只回答"当前状态是否满足收敛条件"，不回答"为什么不满足"或"过去 10 次为什么都不满足"。失败智能填补这个诊断空白。
4. **从静默退化到主动告警**: 当前如果一个 phase 持续 KindTimeout，系统默默地 retry 然后 fail——operator 只在最终 `forge accept` REJECTED 时看到。主动告警（如 `forge diagnose` 显示"security-review phase 在过去 3 次运行中 100% timeout，建议：① 检查网络延迟 ② 考虑提升 model tier 减少 retry"）将用户体验从被动处理故障提升到主动预防。

### 关键设计边界

- **不做 LLM 辅助诊断（v1）**: 纯规则引擎（阈值匹配 + 模式计数）。例如「同一 phase/kind 连续失败 N 次 → 标记为模式」。不调用 LLM 做根因分析——那是 v3 的 AIOps 方向。
- **数据不离开本机**: trace.jsonl 的读取和聚合都在进程内完成，不发送到外部服务。
- **输出为结构化 Report**: `forge diagnose` 输出可解析的 JSON 和可读的文本，供 CI/CD 集成。
- **修复建议是 advisory 非自动执行**: 建议 operator "考虑调整 --max-agent-calls 为 50" 但不自动修改参数——operator 控制权第一。
