# ForgeOS 高价值扩展方向分析

> 角色:资深架构师/产品经理
> 方法:全局扫描 forge-core(17 internal + cmd/forge) + harness + .agent 全量代码,
> 基于**代码现状**而非文档声明。不编写任何代码,只输出分析。
> 日期:2026-07-10

---

## 方向一:并行执行的安全性验证与死锁防御

**当前状况。** `internal/orchestrator/parallel.go` 已实现基于 Kahn 拓扑排序的依赖波次并行执行引擎,
可并发运行无依赖的 agent phase。当前定义了 **8 个共享可变状态 mutex**(trace.Tracer.mu, runBudget.mu,
loopProbe.mu, gateLedger.mu, phaseOutputLedger.mu, ContextCache.mu, reviewFindingsLedger.mu,
verdictLedger.mu),并行 goroutine 的数量随波次规模线性增长。

**问题。** 这 8 个锁的获取顺序以**注释契约**形式维护(parallel.go:25-46「LOCK ORDER CONTRACT」),
但没有任何**运行时验证**。在以下真实场景中,违反顺序的 Heisenbug 死锁会成为 24h 无人值守运行的致命故障:

- 当 workflow 在 parallel 模式下同时包含 gate phase(取 `loopProbe.mu`)和 agent phase(取 `gateLedger.mu`),
  调用路径的交织可能偏离注释约定的顺序。
- `runPhaseParallel` 在锁外 spawn agent,但 `feed`(cost.go) 和 `observeFor`(prompt_context.go) 的回调
  路径各自获取不同的锁,两条路径并发时顺序不可控。
- Go `sync.Mutex` 不可重入,一次顺序倒置 = 永久阻塞,无超时逃生。而在无人值守场景下,死锁只能靠
  外层 `--timeout` 杀掉,浪费整个迭代的已完成工作。

**为什么需要。** 这是**正确性-安全**维度的问题,不是性能问题。ForgeOS 的核心价值主张是**无人值守 24h 自治**:
一旦并行执行成为默认路径(更多 workflow 声明 `depends_on`),死锁不是"可能不会发生"的边缘情况,
而是迟早会发生的概率问题。当前的手工维护契约无法随并发规模扩展。

**建议的解决思路(不编码):**

- 对每个 mutex 加持锁时栈追踪的记录层(wrap),在测试中通过 `-race` + 压力测试验证所有已知路径的顺序。
- 考虑用 `sync.Map` 替代部分细粒度锁(如 `prompt_context.go` 的多个 ledger),或合并成
  一个 phase-scoped 锁(在 wave 粒度内串行化危险路径)。
- 在 `Engine.RunParallel` 入口加入**拓扑上下文传播**:整个 wave 共用一把"阶段锁",而不是每个共享
  数据结构各自一把。

---

## 方向二:跨项目治理漂移检测与同步机制

**当前状况。** `forge-init`(harness/scaffold/forge-init.mjs)通过复制完整治理资产
(agents/skills/workflows/eval/routing/policies + 全套 harness + CI 配置)来创建新项目,
并在创建时通过 COPIED_FILES 清单 + 完整性自测保证全部被复制。但**这是单向的**:
ForgeOS 自身演进后,已派生的项目不会收到任何更新通知或漂移检测信号。

**代码证据:**

- `harness/scaffold/forge-init.mjs` 定义了 `COPIED_FILES` 清单(Sprint 31 曾因遗漏两个文件而触发自测失败)
- `harness/check.py` 已有 `check_workflow_mode_gating` 漂移守卫,但只在单个 repo 内检
  workflow 与 modes.yml 的一致性,不跨仓库比较
- `docs/adr/0003-agent-os-repo-extraction.md` 的 submodule 方案处于"待拍板"状态,
  远程仓库位置尚未确定

**问题的具体表现:**

1. **策略漂移**:ForgeOS 的 `harness/policies.yml` 增加了新的 `enforce` 维度或调整了阈值,
   但子项目的 `policies.yml` 仍是旧版。新检查永远不会跑,直到某次 `forge accept` 因未通过
   一个子项目毫不知情的新检查而 REJECTED。
2. **检查逻辑漂移**:`check.py` 新增了 `check_workflow_*` 函数(在当前版本中从 9 检查增长到 10),
   但子项目仍在运行旧的 9 检查。`acceptance.mjs` 的 probe 列表也可能不同步。
3. **安全修复**:如果 ForgeOS 发现 `secret-scan.mjs` 有假阴性 bug 并修复,
   子项目会继续跑旧版本,直到被人注意到。

**为什么需要。** 如果 ForgeOS 治理框架自身的项目都不能保证跨版本继承的治理一致性,
那么 core value prop——让 AI 在一致治理下自治——就存在结构性断裂。这不是"未来会需要"的功能,
而是当前实际存在但未被工具检测到的风险。

**建议的解决思路(不编码):**

- 轻量级**版本锚点**:在 `forge-init` 生成的 `.agent/project.yml` 中记录 creator 版本
  (如 `forgeos_version: v2.5.0`),`forge doctor` 新增 `--governance-drift` 检查,
  对比本地治理SHA与上游最新版本的差异。
- **增量补丁**:不要复制整个文件,而是复制基础资产 + 项目的增量覆盖层(继承模式),
  类似 ADR-0003 的 submodule 思路但更轻量——一个只读的 `.forgeos-upstream/` 目录,
  CI 每天检查更新。
- **告警而非阻断**:初期只做 `forge status --governance-drift` 的 advisory 输出,
  不强制升级,给项目团队迁移时间。

---

## 方向三:24h+ 长运行时 Memory 引擎的性能退化和收缩策略

**当前状况。** `internal/memory` 包采用 JSONL(O_APPEND 逐行追加)格式的累积日志,
设计为"只增不减"。`Load()` 读整个文件到内存,`Query()` 在内存中做 O(n) 过滤。
`Compact()` 方法存在但需要**手动触发**(`forge memory-prune` CLI)。

**代码证据:**

- `memory.go:119-130`: `Load` 在 `invalidateLoadCache` 之外的每次调用都全读+全解析
- `memory_bench_test.go` 存在但未看到针对 10K+ 条目的 benchmark 结果
- `memory.go:28`: `loadCache` 使用 `sync.Map` 避免 O(N) 重读——但只缓存到文件 mtime 变化,
  在 mtime 粒度为 1s 的文件系统上,紧挨着的两次 Append 可能触发无效重读
- `memory_compact.go` 的 `Compact()` 需要手动 `forge memory-prune`,
  而且**只被 `forge memory-prune` CLI 调用**,loop 中无自动触发

**退化场景(24h+ 自治运行):**

- 每轮 evolve iteration 产生约 5-20 条 memory entry(决策+教训+gap)。100 轮 iteration = 500-2000 条。
  每条 Entry 含 JSON 序列化的字符串 + `Tags`/`Source`。全量 Load 的内存占用和反序列化成本随轮数线性增长。
- `Load` 在每个 agent phase 的 prompt 构建路径上被调用(每 phase 一次),100 轮 × 5 phase/轮 = 500 次全量读取。
- Compact 只去重+压缩,不按时间窗口裁剪:一条 3 个月前已不相关的 memory entry(比如"了解了项目结构")
  仍会被 Load 并注入每个 agent 的 prompt 中,稀释注意力。

**为什么需要。** 项目的 Roadmap 明确声明的目标是**24h 无人值守闭环**。在 hour 18,一个加载了
6000 条 memory entry 的 prompt 构建不仅浪费 token(每条都注入),更会因为无关上下文的稀释
而降低 agent 输出质量。这是一个在**当前 codebase 中已存在但未被触发的性能悬崖**。

**建议的解决思路(不编码):**

- **TTL-based 自动裁剪**:在 memory entry 的 schema 中加 `expires_at` 或 `relevance_decay` 字段,
  `Load` 时自动过滤掉超过 TTL 的条目。默认 TTL 可设为 7 天或 50 轮迭代。
- **分层存储**:活跃条目(最近 N 轮/打标 `critical`)在内存常驻;历史条目归档到磁盘冷存储,
  只在显式查询时反序列化。
- **自动 compaction**:在 `Append` 调用计数达到阈值(如 100)时,自动触发后台 compaction,
  用独立的 goroutine 异步重写 JSONL 文件,避免阻塞主 loop。
- **prompt 注入时的 Semantic routing**:不是在 `memory.go` 层裁剪,而是在
  `cmd/forge/prompt_memory.go` 构建 prompt 时,只注入与当前 phase 标签匹配的条目
  (利用 memory 的 Tag/Score 字段),避免全部 dump。

---

## 方向四:Agent 输出解析的鲁棒性 —— 告别末行精确匹配

**当前状况。** forge-core 从 agent 输出中提取结构化信号(VERDICT/CONFIDENCE)的方式,
在三个独立解析器中**全部依赖末行精确 token 匹配**:

| 解析器 | 所在文件 | 匹配方式 |
|---|---|---|
| `parseReviewerVerdict` | `cmd/forge/cost.go` | `strings.TrimSpace(lines[len(lines)-1])` 精确等于 `VERDICT: APPROVE` 或 `REQUEST_CHANGES` |
| `parseExecutiveVerdict` | `cmd/forge/cost.go` | 同上,匹配 5 个 UPPER_SNAKE token |
| `parseConfidenceScore` | `cmd/forge/cost.go` | `strings.TrimSuffix(lastLine, "CONFIDENCE: ")` → `strconv.Atoi` |

**代码证据:**

- `cost.go:330-345`: `parseReviewerVerdict` 对 `lastLine` 做精确的 `== "VERDICT: APPROVE"` 比较
- `cost.go:347-360`: `parseExecutiveVerdict` 同样的精确匹配模式
- `cost.go:362-380`: `parseConfidenceScore` 同样的末行扫描,但用 `TrimSuffix` + `Atoi`

**生产故障模式:**

1. **LLM 添加后记**:Claude 在输出完 `VERDICT: APPROVE` 后加一行空行或"---"分隔线,
   或加一句"Note: I recommend...",导致末行不再是 token 行,匹配失败。
2. **格式漂移**:Claude 在不同 session/temperature 下可能在冒号后加空格,或用 `APPROVED` 代替 `APPROVE`。
3. **多行 token**:agent 可能在被要求"输出 VERDICT"时,在 `VERDICT:` 后另起一行写值,
   而不是在同一行。
4. **headless 模式下 stdout 捕获不完全**:`CommandExecutor` 的 `cappedBuffer` 在达到 `--max-output-bytes`
   阈值时截断输出,如果截断恰好发生在 token 行中间(概率虽低,但在 24h 运行中一定会发生),
   解析器会静默失败,verdict 丢失。

当前这三个解析器的失败方式全部是**静默的**:找不到匹配则返回零值(空字符串/0),
调用链上的 `reviewStatus`/`requirementConfidence` 检测到零值就把 convergence 判为 NOT MET。
结果是一个已经正确完成任务的 agent 因为输出末尾多了一个空行,导致整个 loop 判定"未收敛",
继续下一轮迭代,烧钱又耗时。这可能是**全系统最贵的单个 edge case**。

**为什么需要。** 这不是"代码质量"问题——它是**在自治运行中每轮迭代高达 $0.18-$0.50 成本的 LLM
调用可能因为一个空格而白费**。在当前 Sprint 26 记录的 telemetry 中,一次 claude 调用的
真实成本是 `avg_cost_usd=0.1841`,而一个 evolve loop 的一轮迭代包含 5 phase = ~$0.92。
如果因为解析器在 hour 6 被一个多余空行卡住,然后 loop engine 因为没有收敛而继续迭代 3 轮,
那浪费就是 ~$2.76——小数字,但在 24h × 365d 的自洽运行中累积显著。

**建议的解决思路(不编码):**

- **改用基于 token 的模糊匹配**:扫描 stdout 全文(而非仅末行)匹配 `VERDICT:\s*(APPROVE|REQUEST_CHANGES)`
  正则,避免对行位置的依赖。
- **结构化输出层**:要求 agent 在其 stdout 的最后一段输出一个机读的 JSON block
  (如 `<!--FORGE_VERDICT:{"type":"approve"}-->`),解析器先扫 JSON block,不存在再回退到
  末行精确匹配。这样 LLM 可以在 prose 中自由表达,同时保证信号可解析。
- **多解析器投票**:如果有 3 个不同匹配策略(末行精确、全文正则、JSON block),
  采取"至少一个匹配"或"多数一致"的策略,而非单一策略 fail-closed。
- **解析失败可观测**:`parseReviewerVerdict` 等函数在未匹配时应 produce 一条 WARN 级别的
  trace event(当前 `cost.go` 中的 fallthrough 路径是静默的),
  让 `forge doctor` 或 `forge status --trace` 可以事后排查。

---

## 方向五:多维模型路由的自动特征提取 —— 从路径子串到语义/结构分析

**当前状况。** `internal/routing` 包的 `Score()` 函数定义了四个路由维度
(complexity, dependency_change, context_size, business_impact)及其权重体系,
且 `TierForScore()` 完整实现了 policy.yml 的五步决策链(score→band→task_type
floor→safety_override→budget_guard)。**但`Score()` 只在 `forge route` 手动 CLI 中被调用**。
在真实执行路径(`engine_build.go` 的 `phaseTierResolver`)中,维度分值完全缺失,
路由只基于 agent role + per-phase model_tier override + risk + budget + history tiebreak。

**代码证据:**

- `routing/routing.go:190-205`: `Score(dims, weights)` 函数完整实现了 normalized weighted sum,
  但它的调用点点只有 `route.go:178` 的手动 CLI 路径(`--complexity` 等 flag 由操作员手动提供)
- `engine_build.go:232-259`: `phaseTierResolver` 计算 `riskAdjustedTier` 时只用了
  `risk.FromChangedPaths` 的子串启发式,没有调用 `routing.Score()`
- `risk/risk_diff.go:3-28`: 文件头部诚实注明"path-substring matching is a COARSE heuristic",
  并明确说 "Precise extraction needs real signal: AST/call-graph... that is v3"
- `internal/routing/routing.go:1-9`: 包文档自述 "not the full multi-dimensional scorer
  (that is the v2+ Router service)"

**价值缺口的具体表现:**

- 一个包含 50 个文件变更的 PR(`complexity` 高)但只改文档(`business_impact` 低)和
  一个改 1 行支付核心逻辑的 PR(`complexity` 低但 `business_impact` 极高),
  在当前的路由路径下**无法被区分**——因为两个维度的值都未自动计算。
- `risk.FromChangedPaths` 是唯一的自动信号源,但只从文件名子串推断 payment/auth/secret/migration,
  不读文件内容、不看调用图、不分析 diff 的修改行是测试还是生产代码。
  对于"改了一个叫 `payment_test.go` 的文件"的情况,它会误判为 `TouchesPayment=true`。

**为什么需要。** G3 多维模型路由是 `PROJECT.md` 声明的核心能力之一("G3 自动模型调度 — 多维路由
(复杂度/风险/阶段/预算/上下文/历史),贵模型只用在该用处")。当前的 agent-role-only + path-substring
实现覆盖率 G3 声明能力的约 30%。剩下的 70% 需要从"手动 flag"升级为"自动提取"才能兑现。
这不是镀金——这是一个已经以 CLI 形式存在(manual `forge route`)但未被接入执行路径的真实能力。

**建议的解决思路(不编码):**

- 在 `internal/risk` 包的基础上扩展一个 `internal/analysis` 包:在 agent spawn 之前,
  对 git diff 做结构化分析,提取:
  - `complexity`:changed lines / touched files count + 文件类型的认知负荷权重
    (如 `.go` interface 定义比 `.md` 文档 weight 更高)
  - `dependency_change`:是否触及了 go.mod/package.json 等依赖文件
  - `context_size`:累计变更的 function/class 数(简单 scanner,不需要完整 AST)
  - `business_impact`:结合 CODEOWNERS 或 path convention(如 `internal/payment/` vs `examples/`)
    做粗略分级
- 以上提取结果作为 `dims` 输入 `routing.Score()`,将路由决策从 agent-role-only 升级为
  **content-aware**。同时保留 `--complexity` 等手动 flag 作为更高优先级 override。
- 当前 `risk.FromChangedPaths` 的 path-substring 启发式保留作为兜底,
  新的 content-aware 维度只在文件可读时启用,不可读时静默降级(和 lint/coverage adapter 同样的
  honest N/A 模式)。

---

## 总结排期建议

| 方向 | 维度 | 风险等级 | 建议优先级 | 预计人天 |
|---|---|---|---|---|
| ④ Agent 输出解析鲁棒性 | 可靠性/成本 | 高(直接在 24h 运行中产生浪费) | **P0** | 2-3d |
| ① 并行执行锁安全验证 | 正确性 | 高(死锁 = 无人值守全停) | **P0** | 3-5d |
| ② 跨项目治理漂移检测 | 架构完整性 | 中(当前未触发但结构性缺陷) | **P1** | 5-8d |
| ③ Memory 引擎长时性能退化 | 性能 | 中(在 24h+ 运行中逐渐显现) | **P1** | 5-8d |
| ⑤ 多维路由自动特征提取 | 功能完整度 | 中(G3 能力缺口) | **P2** | 10-15d |

> **注**:以上分析仅基于当前代码库的实测证据,未考虑外部依赖或未实现的声明。
> 每个方向均标注了对应的代码引用和当前状态,确保可追溯、可验证。
