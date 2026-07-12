现在我已全面掌握项目上下文。以下是我的架构分析。

---

# 架构分析：v38 扩展方向及其在 ForgeOS 演进中的位置

## 1. 架构评估

### 当前架构的核心优势

ForgeOS 当前的架构是我见过的「从自举项目成长起来的框架」中架构最清晰的之一。几个关键优势值得先明确：

**1.1 中枢旋钮（mode×lifecycle）是优秀架构抽象**

将 mode（explorer/balanced/engineering/cto）和 lifecycle（idea/mvp/growth/production）合为一处设置，同时驱动 Router 档位、Harness 严格度、Workflow 深度三处行为——这是从 k8s 的 `PodSpec` 和 Terraform 的 `workspace` 中汲取灵感的**控制面等价物**。它把「多维度独立旋钮」的复杂度压缩成了一个可理解的 2D 矩阵，极大降低了用户心智负担。

验证：Sprint 15 完成了中枢旋钮完整覆盖（discover/design/adr/reviewer/evolve）+ Sprint 18 完成 Harness 严格度接入——五个维度全部由同一个 `project.yml mode: x lifecycle: y` 驱动。这在少于 5000 行 Go 代码中完成，是架构经济性的证据。

**1.2 载重墙（Loading-wall）原则诚实且可落地**

> "只能强制最弱宿主允许的东西" → 带外执法为真相之源，CC hook 为加速器。

这个决策在 Sprint 20-22 构建四维资源护栏（深度 × 数量 × 时间 × 内存）时得到了实证验证：每一层护栏都设计为「宿主不可跳过」而非「假设宿主配合」。相比之下，许多 AI 治理框架的错误在于假设 Agent 会忠实地报告自己的行为。

**1.3 `converge.Signals` 的信号收敛模式是正确抽象**

整个引擎的核心收敛判断（roadmap_done + gates_green + human_approved + review_status + ...）被建模为一个 8 字段的 `Signals` 结构体，每个字段有独立的 `evalOne` 实现，然后由 `Evaluate` 组合判断。这是**策略级熔断器**的正确抽象——不是检查「第几轮了」，而是检查「目标达成没有」。Sprint 29 系统性审计全部 8 个字段并修复 2 个断信号后，这个抽象目前是完整的。

**1.4 诚实标注文化是架构债务的首要防护**

这个项目最不同寻常的一点是：Sprint 记录中**系统性地记录了每个 sprint 结束时未满足的 gap、性能瓶颈、延迟决策**。这导致了 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的诞生——一份从项目自身声明源头推导出的需求清单，而非外部规格。这让我可以准确地知道哪些是「已完成」，哪些是「设计上推迟」，哪些是「真缺口」——而这在绝大多数项目中是不可能的。

### 当前架构的限制（诚实标注）

**限制 1：单 repo 绑定根深蒂固**

`gate.RepoRoot()` 的假设穿透了哈ness 层、路由层、workflow 层。即便 ADR-0003 的设计已定（submodule + 双层覆盖 + 路径解析改造），它的执行需要改变**每条执法热路径的锚定方式**——这不是增量重构，是架构级改造。v38 分析的方向一察觉到了问题，但没有足够强调改造范围。

**限制 2：收敛判定缺少「成本感知」维度**

`stop_condition` 当前只检查 binary 指标（roadmap_done? gates_green? review_approved?）。但真实自治运行的核心运营问题是「这个迭代跑的值不值它的成本」。当前没有 `cost_effectiveness` 或 `value_per_dollar` 收敛信号——这意味着系统可能在某次迭代中花费 $50 去修改一个不重要的拼写错误，然后宣布收敛 MET。这是 ROADMAP.md 和 ARCHITECTURE.md 中从未提到但真实存在的 gap。

**限制 3：phase 间的依赖契约停留在 YAML 声明层面**

`Emits` 和 `DependsOn` 是静态字符串。没有类型化的「我产生什么类型的 artifact，我消费什么前置条件」的契约系统。这使得方向二（自适应循环组装）和方向五（智能门控）都依赖于从申明式 YAML 构建运行时 DAG 的能力——而这在当前只有 `waves.go` 的 Kahn 排序一个浅层实现。

**限制 4：知识学习的反馈回路未关闭**

Memory-Engine 记录（gap/decision/lesson），但这些记录**只被人类读取，不被系统自动消费**。`buildPrompt` 的 `retrieveKnowledge`（当前是一个 TF-IDF 风格的检索器）只在 `prompt_context.go` 中有骨架但不完整——方向三的 v38 分析准确地抓住了这一点。

---

## 2. 扩展方向评估（基于 v38 分析 + 2026-07-12 最新状态）

### 方向一：多项目治理联邦

**当前项目状态（2026-07-12）：设计就绪，等待执行信号。**

| 维度 | 状态 |
|------|------|
| 设计文件 | ADR-0003（Proposed，机制/拆分边界/覆盖语义/迁移计划已定） |
| 代码改动 | 零——ADR 明确写「用户定位置+批准前，不创建任何仓、不改任何代码」 |
| 触发条件 | 被治理项目 ≥ 2~3 **且** 治理资产仍高频演进（当前：0 外部项目） |
| 关键依赖 | 路径解析改造（`FORGE_PROJECT_ROOT` 环境变量）是最大单点工作量 |

**v38 分析校正点：**

- ✅ ADR-0003 未被引用的校正是正确的。但需要更深入：不是「补充 ADR-0003 引用」就能解决的核心问题是 **「谁」拥有做这个决策的权利**。ADR 明确标注为「Proposed + 需用户拍板」——这意味着方向一的启动不是一个技术决策，是一个**组织决策**。架构分析应该明确指出这个依赖。

- ✅ 成本修正（3-4 周 → 1.5-2 周）是对的——但我认为 1.5-2 周仍然太紧。它的最大单点工作量（路径解析改造）触碰了**每条执法热路径**——改坏即假绿，需要至少 1 周的回归测试专用时间。更准确的估计是 **2-3 周 + 1 周压力测试**。

- ❌ v38 分析的一个真正的架构缺失是：它没有讨论 `forge-init` 的当前复制模式与 ADR-0003 的 submodule 模式之间的**互操作期**。已经用 `forge-init` 初始化的项目是「快照」——它们不自动接收治理更新。如果中心治理更新了，这些项目必须**手动选择迁移到 submodule 模式或继续用快照**。这个双轨运行期的管理成本没有被评估。

**我的评估：**

| 维度 | 评分 |
|------|------|
| 产品价值 | 高（进入企业场景） |
| 架构完整性收益 | 中（ADR-0003 设计已定，缺口从「没有解决方案」变为「方案就绪等批准」） |
| 成本 | 2-3 周 + 1 周测试（比 v38 估计高） |
| 实际可启动性 | **阻塞在用户决策**——ADR-0003 的第 1 条待拍板是「远程仓库位置」，第 2 条是「批准不可逆迁移」，第 3 条是「now vs 暂缓」。架构师无法替用户选。 |

**建议**：将方向一的交付物从「实现 submodule」修正为「启动 ADR-0003 的 Phase A（本地可逆原型）」。原型不需要用户选定远程位置（本地 bare repo 即可），不需要批准不可逆迁移（不删本仓目录），只需要**用户说 yes**。这是当前真正的瓶颈。

### 方向二：自适应循环组装

**当前项目状态：复杂度被低估最多的方向。**

`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 揭示了一些直接影响此方向可行性的底层问题：

1. `stop_condition` 系统有边缘脆弱性：`on_rejected` 曾是死代码（Sprint 31 修复），`conjunction` 的 all_of 评估中有未处理的未知 metric 分支。在扩展动态组装之前，**stop_condition 系统必须先被加固为「可信的计算基础」**。

2. Phase 依赖契约是弱类型字符串。当前 `waves.go` 的 Kahn 排序只做拓扑排序，不做契约检查。动态组装需要 phase 级别的 precondition gate——这等价于为每个 phase 声明「我需要什么先决条件才能跑」。这与当前 `asset.Phase` 结构体（`Emits`/`DependsOn`/`RequiredGates`）的能力差距是：当前缺少**前置条件契约 + 可选项声明 + 回退策略**。

3. `forge detect` 的输出（项目 profile）目前没有消费者。`forge-core` 中没有代码将 detect profile 映射为 workflow 变体。`internal/composer` 包需要从零建设和 `internal/mode` 做职责分界。

**v38 分析的校正点：**

- ✅ 「5 个静态 workflow YAML」不准确的校正是正确的。但需要补充：Sprint 11/15 的 workflow_depth 驱动（reviewer phase 跳/跑、evolve depth 变化）已经在 phase 层面实现了**一定程度的动态化**。准确的说法是「phase 列表是静态的，但每个 phase 的跑/跳决策在运行时由 mode×lifecycle 裁决」。

- ✅ phase 依赖声明的动态版本是重定义了 `asset.Phase` 的契约模型——这不是增量改动，是**架构级决定**。4-6 周的估计确实偏乐观。更接近 **8-10 周**（2-3 周 stop 系统加固 + 3-4 周 phase 契约重设计 + 2-3 周 composer 构建 + 1 周集成测试），前提是不拆其他方向。

- ❌ v38 分析没有讨论一个关键问题：**动态组装的审计可追溯性**。如果系统在某次迭代中自动跳过了 security-review，需要能在 trace 中回答「为什么」。trace 事件 schema（`forgeos.trace.v1`）当前没有 `skipped_phase_reason` 字段——这是一个需要在设计阶段定义的 trace schema 扩展点。

**我的评估：**

| 维度 | 评分 |
|------|------|
| 产品价值 | 高（真正自治的瓶颈） |
| 架构完整性收益 | 高（闭环的最后一公里） |
| 成本 | 8-10 周（5 个方向中最高） |
| 前置依赖 | **多**：stop 系统加固 → phase 契约重设计 → composer 构建 → trace schema 扩展 |
| 建议 | 降至技术准备度 P3，等 stop 系统通过一轮完整的 fuzz/对抗测试后再启动 |

### 方向三：知识引擎与语义检索

**当前项目状态：这是 5 个方向中「现在就可以做」的唯一方向。**

| 维度 | 状态 |
|------|------|
| 任务声明 | `prompt/retrieve.go:19-20` 原称"true semantic retrieval needs external embedding model, v3 work" |
| 实际缺口 | 但纯 TF-IDF 检索**不需要**外部模型——Go 标准库 `container/heap` + `strings` + `sort` 即可实现倒排索引 |
| 之前推迟理由 | 被`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 归类为 DEFERRED-BY-DESIGN，v38 分析有效反驳了那个理由 |

**v38 分析的校正点全部正确**——这是 5 个方向中分析质量最好的一个：

- ✅ Memory-Engine 的状态被准确描述了（不仅是 append-only log，有 Query/Compact/Prune/缓存失效）。
- ✅ `knowledgeCap = memoryCap` 模式的复用是正确设计决策——降低设计成本，保持一致性。
- ✅ 1.5-2 周的估计是 5 个方向中最准确的（TF-IDF 算法 + Go stdlib + internal/memory 做后端 + internal/asset 做文档源）。

**我的评估：**

| 维度 | 评分 |
|------|------|
| 产品价值 | 高（解决非遗忘性——24h+ 自治运行的根本瓶颈） |
| 架构完整性收益 | **最高**（填 ARCHITECTURE.md 10 大引擎的显式缺口） |
| 成本 | 1.5-2 周（边际成本最低） |
| 前置依赖 | 无（memory、prompt、asset 均已就绪） |
| 风险 | 低（TF-IDF 是标准算法，检索质量差只意味着更多的 false positive，不产生正确性 bug） |

**补充建议**：方向三的实施应包含一个**回退契约**——prompt 注入的 knowledge lane 必须在注入前标注「以下内容来自语义检索，可能不完全准确」，并附带精确引用（文件路径 + 行号，来自 memory entry 的 SourceRef）。这适配了项目已有的诚实标注纪律。

**决策建议：优先启动方向三（P0）。**

### 方向四：生产级并行安全网

**当前项目状态：竞态已修复，其余待建。**

Sprint 25-26 实现了 `orchestrator/parallel.go` 的并行引擎，但深度检查后：

1. **`cost.go` 竞态**：v38 分析提到的 `feed()` 竞态——复查确认缺口在 `route.go` 而非 `cost.go`，但结论正确。从 FUNCTIONAL_REQUIREMENTS_AUDIT 的 DONE 清单看，`BudgetExhausted` 和 `runBudget` 已有锁（`budget.mu.Lock()`），但 `runBudget.feed` 在 `route.go` 中的路径缺少独立锁——这个 bug 目前**仍然是存在的**（在基于 read 的核查中未被列为 RESOLVED）。优先级：高（竞态可导致预算失控）。

2. Per-phase 并行超时：当前 `--timeout` 是共享的，不支持 per-phase。需要 `phase.Timeout` 字段的新增 + 在 `runAgentPhase` 中创建 sub-context。

3. Per-wave checkpoint：当前不兼容的核心原因是 phase 在并行模式下的无序完成——但 wave 边界是有序的（Kahn 排序保证波内 phase 无依赖，波间有拓扑序）。所以 per-wave checkpoint 是行得通的，只是在 `waves.go` 的 `WaveDone` 回调处触发一次 `persist.Save`。这个设计没有根本难度。

4. 波级重试：需要的 `WaveResult` 类型当前不存在——`waveResult` 是 `parallel.go` 的私有结构体，不导出不序列化。这是扩展它的工程难度，不是设计难度。

**v38 分析的校正点：**

- ✅ 成本从 2-3 周修正为 3-4 周是正确的。竞态修复快（1-2 天），但 per-phase 超时（3-4 天）+ goroutine 限流（3-4 天）+ checkpoint（2 天）+ 重试（3-4 天）+ 测试（3 天）= 确实接近 3-4 周。

- ❌ cgroup 的类比以及「比例分配」替代方案是正确方向（v1 无 cgroup），但是 `spendRatio` 已经是比例分配合适的构建块——每个 phase 创建时分配 `phaseBudget = totalBudget / wavePhaseCount`，phase 内用这个子预算创建 sub-context。这是 O(1) 算法，不需要新依赖。

**我的评估：**

| 维度 | 评分 |
|------|------|
| 产品价值 | 中（解锁并行加速，但仅当用户运行 `--parallel` 才体现） |
| 架构完整性收益 | 中（把 functional demo 变成 production-ready） |
| 成本 | 3-4 周 |
| 风险 | 低（大多改动是纯加法，不改变串行路径） |
| 建议优先级 | P1（竞态修复优先做，其余延后到方向三完成后） |

### 方向五：自动变更影响分析与智能门控

**当前项目状态：产品价值最高，工程复杂度中等。**

关键发现：

1. `determineGateSet` 不存在，但等价物已存在——`mode_gating.go` 的 `gatesFor` + `reviewStageSkipped`。这大幅降低了成本。真正需要的是在 `gatesFor` 上加一个 `risk.Level` 参数（低风险 → 子集，高风险 → 全量）。

2. Go AST 分析的精度问题在 v38 分析中已经诚实标注——v1 不做跨包类型解析，只做函数名引用匹配。这足够将 signa-to-noise ratio 从「文件路径子串匹配」（当前 risk.FromChangedPaths）提升到「函数引用级匹配」。这个改进的价值是真实的：改 `auth.go` 中的注释不会触发 `TouchesAuth=true`。

3. FUNCTIONAL_REQUIREMENTS_AUDIT 中 G3（多维模型路由不驱动执行）被改判为 DEFERRED-BY-DESIGN——但方向五需要的不是完整的多维路由，只是在 `gatesFor` 上加一个 `risk.Level` 参数。这两者不要混淆。

**v38 分析的校正点：**

- ✅ `determineGateSet` 已被 `gatesFor` 替代的判断正确。需要补充的是：`gatesFor` 需要扩展的不仅仅是 risk-level 参数，还需要一个**回退行为**——当 AST 分析不确定时，返回 `nil`（走全量 gate）。这是「when uncertain, be conservative」原则的具体实现。
- ✅ 成本 4-5 周 → 2-3 周的修正正确，前提是只做 AST 级别（函数引用匹配）+ `gatesFor` 插参 + fallback，不做跨包类型解析。

**我的评估：**

| 维度 | 评分 |
|------|------|
| 产品价值 | 最高（90% 变更跳过 3/4 评审相位 → 成本降低 70%） |
| 架构完整性收益 | 高（risk→gate 闭环） |
| 成本 | 2-3 周（核心）+ 可选 2-3 周（跨包 AST 增强） |
| 前置依赖 | 方向三（知识引擎）——因为影响分析的决策上下文需要语义检索找到相似的过去决策 |
| 建议优先级 | P2（方向三完成后启动） |

---

## 3. 接口设计建议

### 3.1 知识引擎（方向三）的接口设计原则

知识引擎（`internal/knowledge` 包）应该设计为**从 `internal/memory` 消费、被 `internal/prompt` 消费**的纯函数库，而非独立服务。

```
memory (Entry store) → knowledge (Indexer + Retriever) → prompt (injectKnowledge lane)
```

**关键接口：**

```go
// Indexer — 增量更新倒排索引
type Indexer interface {
    Add(entries []memory.Entry) error   // 增量添加，非全量重建
    Remove(ids []string) error          // memory.Supersedes 淘汰时调用
}

// Retriever — 查询最相关条目
type Retriever interface {
    Query(ctx context.Context, query string, opts QueryOpts) ([]RankedEntry, error)
}

type QueryOpts struct {
    MaxResults  int      // ≈ knowledgeCap / avgTokenPerEntry
    KindFilter  []Kind   // 只查特定 kind（Gap/Decision/Lesson）
    MinScore    float64  // 相关性阈值，低于此不返回
}

type RankedEntry struct {
    Entry   memory.Entry
    Score   float64       // TF-IDF cosine 或 BM25
    SourceRef string      // 文件路径+行号，供 prompt 注入时引用
}
```

**设计决策：** `Indexer` 和 `Retriever` 接口分离——索引是写路径（`memory.Append` 的钩子里调用），检索是读路径（`prompt.Gather` 的注入点调用）。它们共享 `internal/knowledge` 包内的倒排索引（`invertedIndex`），但各自在独立的 goroutine 中安全访问（索引写后 `sync.RWMutex` 读锁释放）。

**保持向后兼容的关键：** knowledge 的注入必须在 `injectKnowledge` lane 中带诚实标注——"以下内容来自语义检索，可能不完全准确"。当前 `buildPrompt` 的 lane 注入机制（约束 lane / 任务 lane / 记忆 lane）已支持加第四 lane（知识 lane）。

### 3.2 智能门控（方向五）的接口设计

`gatesFor` 的扩展不应引入新的 public API——它应该在 `internal/mode` 包的 `Policy` 结构体上加一个方法：

```go
// 现有签名（简化）
func (p Policy) GatesFor(stage Stage, lifecycle Lifecycle) []gate.Type

// 扩展签名
func (p Policy) GatesFor(stage Stage, lifecycle Lifecycle, risk *risk.Level) []gate.Type
// 当 risk=nil 时，行为与当前相同（全量 gate）——向后兼容零改动
```

`risk.Level` 中新增一个字段 `Confidence`（low/medium/high），由 AST 分析器填入。当 `Confidence==low` 时，`GatesFor` 忽略 risk level，返回全量 gate——这是「uncertain → conservative」的 fail-open 行为。

### 3.3 并行安全网（方向四）的接口设计

关键接口设计决策是**不要引入新的配置标志**。`--parallel` 已经存在。新增的行文应该是 phase-level 的可选超时：

```go
// asset.Phase 新增字段（保持零值向后兼容）
type Phase struct {
    // ... 现有字段 ...
    Timeout time.Duration `yaml:"timeout,omitempty"`  // 每个 phase 独立超时
    BudgetRatio float64   `yaml:"budget_ratio,omitempty"` // phase 级预算分配比例
}
```

`BudgetRatio` 的设计灵感来自 cgroup 的 cpu.shares——不是绝对份额，是相对权重。未设置时所有 phase 平分预算。这个设计的**好处**是：零配置并行，只有需要精细资源控制时才使用这些字段。

### 3.4 通用设计原则

所有方向的接口设计都应遵守以下三条原则：

**原则 1：零值向后兼容**——新结构体字段的零值必须与当前行为逐位一致。

- `Timeout=0` → 继承全局 `--timeout`
- `BudgetRatio=0` → 均分
- `risk=nil` → 全量 gate

**原则 2：诚实注入**——任何检索/推理/近似算法的结果注入到 agent prompt 时，必须附带：
- 精确来源引用（文件路径+行号）
- 不确定性标注（"以下内容可能不精确"）
- fallback（来源不存在时的替代行为）

**原则 3：trace 作为审计事实源**——任何运行时决策（skipped phase / 降级 tier / 跳过的 gate）必须在 `trace.Tracer.Emit` 中记录，带结构化元数据（`reason` / `confidence` / `alternative`）。这是不可协商的纪律——没有 trace 记录的决策等于没做。

---

## 4. 技术选型

### 4.1 关键判断：零外部依赖何时应该打破？

`forge-core` 的零外部依赖（`go.mod` 无 `require`）是项目的核心纪律，但不是教条。从 ADR-0002 可以看到，它是「v0–v1 不写 forge-core」的产物，而非技术宗教。

我对零依赖约束的判断：

| 场景 | 评估 | 建议 |
|------|------|------|
| 方向三 TF-IDF | **不需要打破**——纯 stdlib（`container/heap` + `strings` + `math` + `sort`）足以实现 BM25/TF-IDF | 保持零依赖 |
| 方向五 AST 分析 | **不需要打破**——`go/parser` + `go/ast` + `go/token` 是 Go 标准库 | 保持零依赖 |
| 方向一 路径解析改造 | **不需要打破**——只需 `FORGE_PROJECT_ROOT` 环境变量 + `os.Stat` + `filepath` | 保持零依赖 |
| 方向四 并行安全网 | **不需要打破**——`sync` + `context` 均 stdlib | 保持零依赖 |
| 方向二 phase 契约重设计 | **不需要打破**——纯类型系统改动 | 保持零依赖 |
| 全局知识池（旧 expansion 方向 3） | **可能打破**——SQLite 作为全局池存储 | 建议作为独立 sidecar binary（非 forge-core 包）引入，不打破核心零依赖 |

**结论**：5 个方向在 v1 范围内均不需要打破零依赖。零依赖纪律不应被打破，除非出现明确「不能 without it」的场景（如跨厂商模型池需要 LiteLLM 客户端——但那已是 v3）。

### 4.2 YAML 转码：何时从 Python shim 迁移到 Go 原生？

当前的 YAML 转码（`harness/yaml2json.py`）是 `forge-core` 构建链中唯一的外部语言依赖。Sprint 27 实现了 `internal/yaml2json` Go 原生解析器（修复了 block-scalar 损坏问题），但 `forge run/evolve` 仍然走 Python shim（ROADMAP.md v2 诚实标注）。

**建议**：在启动方向二（自适应循环组装）之前，完成 YAML shim 的替换。因为：
1. 方向二的 phase 契约扩展需要频繁读/写 workflow YAML——Python shim 会成为开发和测试的瓶颈。
2. `internal/yaml2json` 已经可生产使用（7 个真实 workflow 文件逐位匹配 PyYAML）。
3. 移除 Python 依赖意味着 forge-core 的构建链完全 Go 化——这本身就是一个独立价值。

**迁移策略**：不要一次性替换所有 `forge run/evolve` 路径。先在 `forge run` 中增加一个 `--yaml-parser go` 标志（默认 `python`），在 CI 中跑两个 parser 的差分测试，1-2 周后默认切到 Go parser，再 1-2 周后移除 Python shim。

### 4.3 新包的依赖方向

所有扩展方向引入的新包必须遵守一致的依赖方向：

```
internal/knowledge → internal/memory, internal/asset
internal/attribution → go/parser, go/ast (stdlib)
internal/composer → internal/mode, internal/asset, internal/gate
internal/{mode,orchestrator}.扩展 → 已在包内，无新依赖
```

关键约束：`internal/memory` 必须是**知识层 storage**（存储 entries），`internal/knowledge` 必须是**知识层 indexing/retrieval**（消费 memory 但不被 memory import）。这保持了 layering：存储不依赖检索策略，检索策略依赖存储接口。

---

## 5. 实施路线图（修订版）

### 优先级排序

根据上述分析，我提出以下修订后的优先级：

| 优先级 | 方向 | 理由 |
|--------|------|------|
| **P0** | 方向三：知识引擎 | 最低成本（1.5-2 周）+ 最高架构完整性收益（填 ARCHITECTURE.md 显式引擎缺口）+ 零前置依赖 + v38 分析有效反驳了项目自身的推迟理由 |
| **P0.5** | 方向四：并行安全网——竞态修复 | `route.go` 的 `runBudget.feed` 缺少 `mu.Lock()`——这是一个活跃 bug，不是待优化。1-2 天。应该在启动任何其他方向前完成。 |
| **P1** | 方向一：治理联邦——ADR-0003 Phase A | 启动 ADR-0003 的本地原型阶段（不依赖用户选远程位置）。2-3 周。但阻塞在用户批准——可以先准备技术依赖（路径解析改造），一旦批准立即启用。 |
| **P2** | 方向五：智能门控 | 产品价值最高，但依赖方向三的知识引擎提供影响分析的决策上下文。2-3 周。 |
| **P3** | 方向四：并行安全网（除竞态） | per-phase 超时 + goroutine 限流 + per-wave checkpoint + 波级重试。3-4 周。价值取决于用户是否用了 `--parallel`。 |
| **P4** | 方向二：自适应循环组装 | 复杂度最高（8-10 周）+ 前置条件最多（stop 系统加固 + phase 契约重设计 + composer 构建）。建议先让方向三+五积累运行数据后，用真实数据指导方向二的设计。 |

### 阶段划分

**Phase 0（立即，1-2 天）：竞态修复**
- 修复 `route.go` 中 `runBudget.feed` 的竞态
- 在 `TestRunParallel` 测试套件中增加 `-race` 测试
- 交付物：`forge accept` 在 `-race` 下全绿

**Phase 1（P0，1.5-2 周）：知识引擎**
- 实现 `internal/knowledge` 包：TF-IDF 倒排索引 + BM25 检索 + top-K 截断
- `memory.Append` 钩子 → 增量更新索引
- `buildPrompt` 加第四 lane（知识 lane）+ `knowledgeCap` + 诚实标注
- 交付物：`forge run/evolve` 中 agent prompt 自动注入相关过往决策

**Phase 2（P1，2-3 周）：治理联邦 Phase A**
- 路径解析改造：`FORGE_PROJECT_ROOT` + 自身位置锚定→项目根锚定迁移
- 本地裸仓验证：`forge-init` 改为 `submodule add` 模式
- ForgeOS 本仓 dogfood：本仓切 submodule 后 `forge accept` 仍 ACCEPTED
- 交付物：可移植的治理架构验证通过

**Phase 3（P2，2-3 周）：智能门控 v1**
- `risk.FromChangedPaths` 升级：`go/parser` + `go/ast` 函数引用级扫描
- `gatesFor` 扩展：接受 risk.Level 参数 + `Confidence==low` 时 fallback 到全量
- `risk.ImpactBlastRadius` 注入 `converge.Signals`
- 交付物：90% 小变更自动跳过 3/4 评审相位，cost trace 验证成本降低

**Phase 4（P4，8-10 周）：自适应循环组装（愿景，非本轮承诺）**
- Stop 系统加固（fuzz 测试 `on_rejected`/`on_unmet`/`conjunction` 所有路径）
- Phase 契约重设计（`Emits`/`DependsOn` → 类型化 precondition gate 声明）
- `internal/composer` 构建：detect profile → phase list + gate list + model tier
- Trace schema 扩展：`skipped_phase_reason` 字段
- 交付物：`forge evolve` 在 gap 数量少时自动跳过 implement→review→qa，直接 evaluate→stop

### 风险矩阵

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|----------|
| 知识引擎检索质量太低（假阳性多），agent 忽略知识注入 | 中 | 低 | TF-IDF 的 `MinScore` 阈值可调；知识注入带诚实标注，agent 自主决定是否采纳 |
| ADR-0003 用户决策不批准，方向二卡住 | 中 | 中 | Phase A（本地可逆原型）可以在不涉及用户远程决策的范围内推进；如果用户选择暂缓，方向一的交付物变为「路径解析改造 + ADR-0003 就绪状态保持」 |
| Go AST 分析在 monorepo 中精度不足 | 中 | 中 | v1 范围明确限定为单包函数引用；跨包分析标记为 v2 增强，不承诺精度 |
| 并行安全网的 per-phase 超时引入新竞态 | 低 | 高 | 每 phase 创建独立 sub-context，不用共享 timer；`-race` CI 闸门必须在 PR 合入前全绿 |
| 方向二自适应组装的 phase 契约重设计造成向后兼容断裂 | 低 | 高 | 契约使用 gradual typing 策略——旧 YAML（无契约声明）被解析为「全开模式」（所有 phase 必跑），新契约只在新 YAML 中生效 |

### 与现有扩展方向文档的整合

`docs/expansion-direction-analysis.md`（Sprint 26 产出）提出了 4 个不同的方向（多 Agent 仲裁、测试自愈、全局知识池、预测性预算）。这些方向与 v38 分析的 5 个方向是**互补而非冲突**的：

- **多 Agent 仲裁**可以嵌套在方向五（智能门控）的 reviewer-false-positive 处理中，作为 gate-bypass 判据的一部分。
- **测试自愈**可以嵌套在方向二（自适应循环组装）的 phase 动态选择中——当 detect 到测试缺口时，自动插入 test-backfill phase。
- **全局知识池**是方向三（知识引擎）的 v2 增强——跨项目聚合不是当前 v1 的知识引擎的范围内，但架构预留了 `ShareLevel` 元数据。
- **预测性预算**是方向四（并行安全网）的自然扩展——竞态修复后，预算跟踪系统稳定了，才能在它之上构建预测器。

**建议**：将 v38 分析和旧 expansion 分析合并为一份统一文档（`docs/strategic-next.md`），按优先级排列 9 个方向（当前的 5 + 旧的 4），标注其嵌套关系和前置依赖。当前 150+ 分析文件已造成噪声膨胀——一份吸纳精华的统一文档比第 151 份独立文件更有价值。

---

## 结论

v38 的扩展分析是一份高质量的架构文档：代码接地、论证三维、边界诚实。在上述校正（ADR-0003 引用、FUNCTIONAL_REQUIREMENTS_AUDIT 分类平行安全网成本重估、Go AST 精度边界声明）之后，它将成为 ForgeOS 下一阶段规划的权威参考。

**我的最终建议：**

1. **立即修复** `route.go` 的竞态（1-2 天）
2. **P0 启动**方向三知识引擎（1.5-2 周）——填 ARCHITECTURE.md 的 10 大引擎缺口
3. **同时准备**方向一的 ADR-0003 Phase A（技术依赖就绪，等用户批准即执行）
4. **方向三完成后**启动方向五智能门控（2-3 周）
5. **方向五稳定后**评估方向二自适应组装的启动条件（stop 系统是否已加固、是否有足够的 trace 数据指导设计）
6. 方向四（并行安全网全量）和方向二（自适应组装）列 P3/P4，作为数据驱动决策的候选，而非首批启动的工程目标
7. **将 v38 分析与旧 expansion 分析合并**为 `docs/strategic-next.md`——结束 150+ 文件的噪声膨胀
