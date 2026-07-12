Now I have a thorough understanding of the ForgeOS architecture. Let me provide my architectural analysis.

---

# 架构分析报告：ForgeOS 扩展五方向

## 1. 架构评估

### 当前架构的优势

**1.1 诚实优先（Honesty-First）的设计哲学深入人心**

ForgeOS 最突出的架构特质是将"诚实"作为一等设计约束贯穿全线——从 `converge.Signals` 中 `RequirementConfidence` 默认 0 永不静默收敛、`gate.Result` 的三态（PASS/FAIL/NA），到 `evalOne` 对未知 metric 的 `unmet-by-default` 策略。这不是表面装饰，而是实实在在的架构决策：每个 N/A 都如实标注而非伪造为 PASS，每个断信号都 fail-closed 而非假设安全。这在 AI 自治系统中极其稀有且价值巨大——防止了"假绿"带来的错误信任。

**1.2 零外部依赖的 Go 核心**

`forge-core` 纯 Go 标准库、`go.mod` 零 `require` 行（Sprint 29 验证），这是有意识的设计约束，带来了可审计的供应链安全、无 license 风险、跨平台单二进制部署。与大多数 AI 编排系统（依赖大量 npm/pip 包）形成鲜明对比。

**1.3 中枢旋钮（mode × lifecycle）统一驱动三平面**

这是最优雅的架构创新：一个设置同时驱动 Router 档位、Harness 严格度和 Workflow 深度（Sprint 15 完成全维度化）。`production` 生命周期一票否决制（Sprint 18：`enforce: block` + Sprint 15：深度强制 full）确保了安全下限不被宽松 mode 绕过。

**1.4 非轮数收敛（Round-Count-Free Convergence）**

`converge.Evaluate` 不依赖轮数，而是通过 `RoadmapCompletion × GatesGreen` 等真实信号判定收敛（Sprint 25-26 真 claude 验证了增量级和版本级 MET）。这是 AI 自治系统正确性的关键机制。

**1.5 架构执法由自身 dogfood**

`arch-check.mjs` 的 8 检查（layering/package/fanin/cognitive/anti-pattern/function-length/circular-dependency/drift-guard）应用于 ForgeOS 自身，且在生产中真实抓到过 113 行的测试函数（Sprint 5）、God 文件（validate.go 994 行，Sprint 27）——这是元框架可信度的铁证。

### 当前架构的局限

**2.1 单机架构的可扩展性天花板**

当前所有状态（memory/trace/checkpoint/scorecard）均基于文件系统。`memory.Load` 的 mtime 缓存（`memory.go:140-175`）虽能缓解同一 iteration 内的重复读，但跨进程共享不存在——两个 `forge evolve` 实例在同一仓库上操作会互相覆盖 checkpoint 和 trace。Memory 的 `invalidateLoadCache` 使用 `sync.Map` 的 `Delete`（`memory.go:120-128`），但注释诚实承认了跨进程竞争。这是向 north-star 分布式架构（Temporal + Postgres + Qdrant）迁移时必须解决的根本问题。

**2.2 JSONL 知识存储的语义贫瘠**

Memory 是纯追加 JSONL（`memory.go:100-110`），`Query` 只做 `kind+topic` 精确匹配（`memory.go:200-215`），`Prune` 是纯计数保留（`memory_compact.go:27-30`），`Compact` 基于年龄而非语义（`memory_compact.go:70-90`）。没有语义去重、没有矛盾检测、没有相关性排序。这在 v1 场景下（少量条目）是可接受的，但随着 24h 自治运行积累数百上千条知识条目，检索噪声将急剧上升——验证分析中方向 3 的核心论点完全成立。

**2.3 Python YAML shim 是架构薄弱点**

Go stdlib 不包含 YAML 解析器，当前方案是通过 `python3 harness/yaml2json.py` 在运行时 shell 出转码（ROADMAP.md 明确标注了这一点）。这不仅增加了运行时依赖（python3 必须存在），还引入了进程间通信的脆弱性。Sprint 27 的 `yaml2json` block-scalar 损坏 bug（`consumeBlockScalar` 把 `"> "` 前缀注入解码值）正是这一问题在犬儒层面的体现——如果使用原生 Go YAML 库，这类问题可以在编译期捕获。

**2.4 Scorecard 学习闭环的可观测性大于实际效果**

`HistoryTiebreak`（`scorecard.go:110-145`）在 v1 单候选模型（claude-only）下几乎是死路径——每个 tier 只有一个候选，历史择优退化为"冷启动走默认"。`CandidatesForTier`（`routing.go:155-170`）为 Opus 返回 `[opus, sonnet, haiku]`，但在无跨厂商池时的实际效果为零。代码是全的、决策链是完整的、测试是绿的，但真正的学习闭环收益要到 v3（LiteLLM 跨厂商池）才能兑现。这是一个合理的"先铺管道，后通水"策略，但需要在架构文档中诚实标注当前阶段的真实 ROI。

**2.5 架构债务积累与偿还模式**

从 CURRENT_SPRINT.md 可以看出一个模式：功能开发积累行数 → 触线（500 行/函数 50 行）→ 陷入架构自纠 → 拆包/迁移。Sprint 27 的 `validate.go` 994 行拆出 `internal/doctor`、Sprint 29 的 `gate_resolve.go` 逻辑迁入 `internal/gate` 后又回调 `package.max_files`、Sprint 30 的 `prompt_context.go` 拆出 `prompt_artifacts.go`。这个模式的积极面是约束有效执行，消极面是**每一次拆包都是在密集并行开发压力下发生的，缺少事先的架构规划**——拆包决策往往是"这个文件超了，挪到哪去"而非"这一组职责的合理归属是什么"。

### 关键设计决策评估

| 决策 | 评价 | 风险 |
|---|---|---|
| 零外部依赖 | ✅ 供应链安全，可审计 | 限制了生态集成，增加了自研成本 |
| JSONL 追加日志式存储 | ✅ 简单可靠，灾难可恢复 | 无索引，查询 O(n)，不适合大规模 |
| mode × lifecycle 中枢旋钮 | ✅ 优雅的统一抽象 | 新增维度时改动面广（5+ 处） |
| converge 驱动而非轮数驱动 | ✅ AI 系统正确性关键 | 信号质量高度依赖真实数据链路完整性 |
| scorecard 学习闭环先铺管道 | ✅ 前瞻性设计 | v1 实际收益低，需抵抗镀金诱惑 |
| 文件系统状态持久化 | ⚠️ 简单但限制多 | 跨进程协同时有问题，分布式不可能 |

---

## 2. 扩展方向

### 方向 A：知识语义基础（P1 — 当前 Memory 包的架构升级）

**为什么需要**

当前 Memory 包（`internal/memory`）的 JSONL + 精确匹配设计在条目数 < 100 时够用。但 24h 自治循环在 Sprint 26 真点火验证中已生成约 30-50 条/iteration 的知识条目（trajectory + reviewer findings + gate failures + recurring decisions），经过 10 个 iteration 后 Memory 的 `Query` 返回 300-500 个条目。全量注入 agent prompt 会达到 token 预算上限，且精确匹配无法找到语义相关但字面不同的条目（如 "test gap" 与 "missing coverage"）。

**核心挑战**

1. **语义去重**——不需要完整 embedding pipeline，但需要轻量级指纹（如 TF-IDF 余弦相似度）。v2 可在纯 stdlib 内实现（Sprint 29 已诚实标注 TF-IDF 已工作但未集成去重）。
2. **矛盾检测**——两个 entry 的 `Topic` 相同但语义矛盾时（如 `"use postgres"` 和 `"use sqlite"`），Memory 当前只通过 `Supersedes` 字段手动标记（`memory.go:50-58`），无自动检测。
3. **衰减策略与 Memory 的 Compact 集成**——当前 Compact 基于 ageSeconds 分组（`memory_compact.go:70-90`），如果与语义重要性结合（如高 confidence 的 Decision 保留更久），需要改动 Compact 的 predicate。

**预期的架构变更**

- `internal/memory` 包新增 `semantic.go`：提供 `Similarity(a, b Entry) float64` 函数
- `internal/memory/query.go`：新增 `Search(entries, query string, topK int)` 方法，使用倒排索引或 TF-IDF
- `Compact` 函数的 predicate 从纯年龄过渡到 age × confidence × kind 加权
- 新增 `memory_contradiction.go`：自动检测 `filterSuperseded` 无法覆盖的语义冲突

**对系统的影响**

- 向后兼容：所有新增功能默认不启用，现有 `Query`/`Append`/`Load` 行为不变
- 无需新外部依赖：TF-IDF 和余弦相似度可在纯 stdlib 内实现（Sprint 26 已预备）
- 性能：`Search` 引入 O(n) 扫描，但可缓存倒排索引

### 方向 B：分布式状态与执行分离（P1 — 为 north-star 架构铺路）

**为什么需要**

当前单机模型是架构的最大可扩展性瓶颈。文件系统状态 + shell 出子进程的执行模型限制了：
1. 多项目并发管理（每个项目的 `.forge/` 目录隔离，但 host 级别无法共享资源）
2. 跨进程 checkpoint/resume（Sprint 25-26 验证了单进程可靠，但进程崩溃后不能由另一进程接管）
3. 多人团队协作（审批信号是文件系统标记，无法跨工作树传播）

**核心挑战**

1. **状态外移策略**——不应一步到位迁移到 Temporal + Postgres。合理的中间态是将 checkpoint 从文件系统迁移到 SQLite（零依赖，Go stdlib 配合 `modernc.org/sqlite` CGo-free 实现），然后在 v3 过渡到 Temporal。
2. **执行与编排分离**——当前 `orchestrator.Engine` 同时负责编排和执行。分离后 Engine 只生成执行计划，由独立的 Runner 服务执行。这是 north-star 架构中控制面/数据面分离的前提。
3. **API 契约的版本化**——一旦状态跨进程共享，文件格式升级需要迁移路径。当前 `trace.Event` 已有 `Format` 字段（`"forgeos.trace.v1"`），但 Memory 的 `_format` 未被下游消费。

**预期的架构变更**

- 新增 `internal/store` 包：抽象存储接口（`Store.Load/Save/List/Delete`），初期实现文件系统 + SQLite，远期实现 Postgres
- `internal/persist` 和 `internal/memory` 改走 `store.Store` 接口
- `orchestrator.Engine` 与 `runner.Runner` 分离：Engine 输出可序列化的 `ExecutionPlan`，Runner 消费
- `forge run --remote` 标志：将执行计划提交到远程 Runner

**对系统的影响**

- 这是一个**大方向，需要分 2-3 个 sprint 渐进实现**
- 引入 SQLite 意味着打破零外部依赖（除非使用纯 Go 实现），这是需要 CTO 级别决策的 trade-off
- Backward compatibility：文件系统状态读取器需要保留至少一个 major version

### 方向 C：Agent 输出信任管线（P1 — 从 discover 到 build 到 review 的置信度传播）

**为什么需要**

验证分析确认了方向 2 的核心论点：`AgentVerdict`（`orchestrator.go:118-122`）只有 `(verdict string, ok bool)`，没有置信度维度。`converge.Signals` 虽然有 `RequirementConfidence`（收敛阈值 80%），但当前仅用于 discover 阶段，且 `gatherSignals` 的赋值在 Sprint 29 才修复（之前恒为 0 导致永远 unmet）。没有一个泛化的机制将 agent 输出的置信度传递给决策链的下一环。

**核心挑战**

1. **置信度提取的泛化**——当前 `parseConfidenceScore`（`cost.go:374`）只处理 `CONFIDENCE: <N>` 格式，且只在 discover 阶段使用。需要将置信度提取做成通用的 `observeFor` 回退链的一部分（类似 `parseReviewerVerdict → parseExecutiveVerdict → parseConfidenceScore` 的三路回退，Sprint 28-29）。
2. **置信度的路由影响**——如果 build 阶段的 implementer 输出置信度为 30%，是否应该自动触发升级到 Opus 路由？当前 `routing.TierFor` 没有置信度输入维度。
3. **虚假置信度**——Agent 可能系统性地高估自己的置信度。Sprint 25 的真点火验证揭示 implementer 在 `acceptEdits` 无 Bash 下不诚实勾 ROADMAP。置信度信号应与客观信号（gate 结果、FileDelta）交叉验证。

**预期的架构变更**

- `internal/converge` 新增 `ConfidenceSignals` 结构体：每个 agent phase 的置信度与其源阶段关联
- `orchestrator.AgentVerdict` 从二元升级为 `(verdict string, confidence float64, ok bool)`
- `internal/routing` 新增 `ConfidenceAdjustTier`：当置信度低于阈值时自动提升 tier
- `observeFor` 的通用置信度提取成为 `internal/orchestrator` 的标准回调

**对系统的影响**

- 低风险：`AgentVerdict` 返回三元组时，`ok=false` 的旧路径保持向后兼容
- `converge.Signals` 的新字段不会改变现有收敛逻辑（默认 0 仍为 unmet）
- 路由影响需要设计决策：是否真的应该用 agent 的自评置信度去调整路由 tier？

### 方向 D：演化自举基础设施（P0 — CI/CD 完整闭环）

**为什么需要**

验证分析确认了方向 1 的核心发现：CI 中无 `forge evolve`（`forge.yml` 只跑 `forge accept + go build/test + forge run --executor dry`），`cmdEvolve` 无 `--dry-run` 扩展到输出预估（`evolve.go:25-65`），且 `enforce: block` 在 `policies.yml:14`。但更重要的是，**ForgeOS 自身尚未被其自身的演化闭环所管理**——这既是机会也是风险。

**核心挑战**

1. **递归闸门问题**——`harness/acceptance.mjs` 跑 `gate.mjs`（体积闸门）和 `arch-check.mjs`（架构检查），但 `acceptance.mjs` 自身的修改不会被这些闸门覆盖（它不被 `arch-check.mjs` 当作源文件分析）。验证分析中方向 1 的 ⚠️ 标记已暗示了这是测试层面的重叠，但本质上是自举困境：用于治理代码的工具自身需要被治理。
2. **CI 中的 `forge evolve` 安全**——在 CI 中使用 `--executor command --agent-cmd claude` 意味着 CI job 需要 API key、有写权限、有成本预算。这与 CI 的"可重现、低成本"哲学存在张力。
3. **dry-run 模式的进化性**——`forge run --executor dry` 只叙述不做事，对代码库无修改。如果不与真实 agent 结合，演化闭环不会产出可验证的改进。

**预期的架构变更**

- `forge.yml` 新增 `forge evolve --executor dry` 环节：验证 evolve 管线本身的完整性（不计产出，只验证不崩溃）
- `cmdEvolve` 新增 `--dry-run` 扩展：输出迭代轮数预估、成本预估、收敛判据
- 递归闸门：`harness/acceptance.mjs` 自身注册到 `forge-init` 的 COPIED_FILES 中，使其接受一致的治理
- 可选：nightly CI job 使用 `--agent-cmd claude` 在受控仓库上跑真 evolve，输出到观察性 artifact

**对系统的影响**

- CI 改动需协调 `.github/workflows/forge.yml` 和 harness
- 真 agent CI 需要 OAuth 凭据管理和预算控制（Sprint 21-22 的四维安全护栏正好为此预备）
- 不应该成为发布阻断闸门——自举是增强型，非载重型

### 方向 E：治理资产生命周期管理（P2 — 资产版本收敛与精简）

**为什么需要**

验证分析确认了方向 5 是最纯粹的新颖方向——零重叠。`cmdValidate` 不检查 lifecycle 适配度（`validate.go:30-70`），`doctor.Governance` 不计算 lifecycle 比率（`governance.go:45-80`），`forge-upgrade.mjs` 无三路合并（`forge-upgrade.mjs:100-120`），全仓无 `governance.yml` 或 lifecycle 映射。

这导致一个实际问题：`forge-init` 复制了全套治理资产（9 agent 卡 + 8 skill 卡 + 4 workflow + 路由策略 + 评估 schema + harness 全套工具），但一个 `lifecycle=idea` 的脚手架项目根本不需要 `security-engineer` agent 卡或 `performance-review` 模板。这些资产成为噪声。

**核心挑战**

1. **资产与 lifecycle 的适配映射**——需要决定「lifecycle=mvp 时哪些 agent/skill/workflow 是必须的，哪些是可选的」。这是一个声明式映射（`governance.yml`），不是代码硬编码。
2. **`forge validate --governance-fit` 的判据设计**——检查发现缺失资产是报警还是阻断？对于 `lifecycle=production` 缺少 `security-engineer` agent 卡应该是阻断，但对于 `lifecycle=idea` 应该是 advisory。
3. **`forge prune --governance` 的安全性**——自动移除"不需要"的资产不能误删用户自定义内容。需要安全机制（diff preview、`--dry-run` 默认、可恢复标记）。

**预期的架构变更**

- 新增 `governance.yml` 文件：声明 lifecycle → required/recommended/optional 资产映射
- `internal/doctor/governance.go` 扩展：检查 governance 适配度并计算覆盖率比率
- `cmdValidate` 新增 `--governance-fit` 标志：读 governance.yml 检查当前资产集适配度
- `harness/scaffold/forge-upgrade.mjs` 的三路合并（mine/theirs/ours）避免覆盖用户调整
- `forge prune --governance` 命令：按 governance 声明筛选并移除不必要资产

**对系统的影响**

- 需要设计 governance.yml schema——这是新的一等公民配置文件
- `forge-init` 的 COPIED_FILES 逻辑需要升级：不是全部复制，而是按 lifecycle 过滤
- `check.py` 需要新 check：校验 governance.yml 的完整性
- **低风险高收益**——不影响任何运行时行为，只影响脚手架和治理资产管理

---

## 3. 接口设计建议

### 3.1 关键模块的接口原则

**Memory 接口升级建议**

当前 `memory` 包没有显式接口——消费者直接调用包函数 `Append/Load/Query/Prune/Compact`。随着分布式存储的引入，应该提取接口：

```go
// internal/store/knowledge.go (建议)
type KnowledgeStore interface {
    Append(ctx context.Context, e memory.Entry) error
    Load(ctx context.Context) ([]memory.Entry, error)
    Query(ctx context.Context, kind, topic string) ([]memory.Entry, error)
    Search(ctx context.Context, query string, opts SearchOpts) ([]memory.Entry, error)
}
```

当前文件系统实现改名为 `JSONLStore` 实现该接口。新增的 `SQLiteStore` 或 `QdrantStore` 实现同一接口。这个接口的提取应该在方向 A 启动之前做，以避免新实现与旧包函数耦合。

**Orchestrator AgentExecutor 接口**

当前 `AgentExecutor` 接口（`orchestrator/executor.go`）是清晰的：

```go
type AgentExecutor interface {
    Exec(ctx context.Context, phase asset.Phase, mode string) error
}
```

建议保持简洁，但增加 `Verdict() (string, float64, bool)` 方法以支持方向 C 的置信度传播。旧 `DryRunExecutor` 返回 `("", 0, false)`，保持向后兼容。

### 3.2 是否需要新的抽象层

**需要：Store 抽象层**

跨 Memory、Trace、Persist、Scorecard 四个包的存储操作应统一到一个抽象层。当前各包各自管理文件打开/关闭/缓存逻辑（`memory.go` 的 `loadCache`、`persist` 包的 `Save/Load`、`trace` 的 `NewTracer`、`routing` 的 `LoadScorecards`）。一个 `internal/store` 包提供统一接口 + 文件系统实现，未来扩展为 SQLite/Postgres 实现，可以大幅减少重复代码和跨进程竞争问题。

**不需要：独立的"路由仲裁器"**

当前 `routing.TierForScore` + `HistoryTiebreak` + `BudgetAdjustTier` 的组合已经覆盖了完整的决策链。在 v1 单供应商场景下，再抽象一层路由仲裁器（Router Service 接口）只会增加间接层级而无实际收益。这应该推迟到 v3 跨厂商池引入时再决定。

### 3.3 向后兼容策略

1. **新增字段使用 omitempty/指针**——所有 Phase/Workflow/Signals 的新字段都遵循此模式（Sprint 27-31 已证明有效）。
2. **接口提取使用类型别名过渡**——从包函数到接口的过渡可以使用中间类型别名，使现有调用方无感知。
3. **存储格式预留版本号**——`trace.Event.Format` 和 `memory.Entry._format` 已预留，应规范为 `"forgeos.<pkg>.v<major>"` 格式。
4. **弃用标记使用 `NOTE:` 注释**——Sprint 30 确立的模式，死字段加 `NOTE:` 注释而非删除。
5. **新功能默认 opt-in**——方向 A 的 `Search`、方向 C 的置信度传播、方向 E 的 `--governance-fit` 都默认不改变现有行为。

---

## 4. 技术选型

### 4.1 需要引入的新技术

| 方向 | 建议技术 | 理由 | 风险 |
|---|---|---|---|
| 方向 A 语义搜索 | 纯 Go TF-IDF + 余弦相似度 | 零外部依赖，与零依赖承诺一致；Sprint 26 已预备 TF-IDF 实现 | 准确度不及 embedding 方案，但 v2 够用 |
| 方向 B 状态外移 | `modernc.org/sqlite`（CGo-free 纯 Go SQLite） | 零 C 依赖，保留零外部依赖承诺；可作为 Postgres 的前置替代 | 需要打破「纯 stdlib」红线——这是架构决策 |
| 方向 B 分布式执行 | Temporal Go SDK（v3 目标，非当前） | 与 north-star 架构一致；durable wait/retry/backoff 开箱即用 | 引入外部依赖，增加运维复杂度 |
| 方向 E 资产映射 | 纯 YAML（用已有 python shim 转 JSON） | 保持一致性，不引入新格式 | governance.yml 可能较复杂 |

### 4.2 第三方依赖评估标准

根据已有决策框架：

1. **是否纯 Go 实现？**（避免 CGo 交叉编译问题）
2. **License 是否与当前兼容？**（MIT/Apache 2.0/BSD 可接受，GPL 需评估）
3. **是否破坏零依赖承诺？**——这是最关键门槛。`modernc.org/sqlite` 是纯 Go，但仍是外部依赖。建议先进行架构决策（ADR）明确：**零外部依赖是 v1 约束还是永久约束？**
4. **是否引入供应链风险？**——最小化传递依赖。

### 4.3 自建 vs 采购

| 能力 | 建议 | 理由 |
|---|---|---|
| 知识存储 | 自建（如方向 A） | 核心差异化能力；当前 JSONL 够用，语义搜索可增量构建 |
| 分布式编排 | 采购（Temporal） | Temporal 是成熟的开源编排引擎，自建分布式状态机成本极高 |
| 跨厂商模型路由 | 采购（LiteLLM） | 非核心差异化能力；LiteLLM 提供标准化的多供应商接口 |
| 沙箱执行 | 采购（Firecracker） | 隔离执行是强需求但非差异化，Firecracker 是行业标准 |

---

## 5. 实施路线图

### 优先级排序

```
P0 — 保持系统健康（方向 D: 自举基础设施）
P1 — 提升核心能力（方向 A: 知识语义 + 方向 C: 置信度管线）
P2 — 强化治理与规模化（方向 B: 状态分布化 + 方向 E: 资产生命周期）
```

### 阶段划分

**阶段一（2 sprints）—— 基础设施加固**

- **方向 D 子集**：`cmdEvolve` 新增 `--dry-run` 扩展输出预估（方向 1 验证的核心缺失）
- **方向 B 预备**：提取 `internal/store` 接口，文件系统实现不变
- **架构决策 ADR**：零外部依赖的永久性 vs v1 约束——明确 SQLite 引入的条件

关键交付：`forge evolve --dry-run` 输出迭代轮数/成本预估；`store.Store` 接口提取完成

**阶段二（3 sprints）—— 知识语义升级**

- **方向 A**：`internal/memory` 新增 `semantic.go`（TF-IDF 指纹去重）、`Search` 方法（顶 K 相似检索）
- **方向 A**：`Compact` 的 predicate 升级（age × confidence × kind 加权衰减）
- **方向 A**：矛盾检测——`filterSuperseded` 升级为 `detectContradiction`

关键交付：`memory.Search` 可用；Compact 语义感知；矛盾检测自动告警

**阶段三（2 sprints）—— 置信度管线贯通**

- **方向 C**：`AgentVerdict` 升级为三元组 `(verdict, confidence, ok)`
- **方向 C**：`observeFor` 通用置信度提取（从三路回退升级为四路：reviewer → executive → confidence → default）
- **方向 C**：`routing.ConfidenceAdjustTier`——当置信度低于阈值时自动提示升级 tier（非强制，设计决策）

关键交付：`forge run` 和 `forge evolve` 中每个 agent phase 的置信度在信号链中可见；`forge route` 输出含 confidence 维度

**阶段四（3 sprints）—— 治理资产生命周期管理**

- **方向 E**：`governance.yml` schema 设计与实现
- **方向 E**：`forge validate --governance-fit` 检查
- **方向 E**：`forge prune --governance`（默认 `--dry-run`）
- **方向 E**：`forge-upgrade.mjs` 三路合并（mine/theirs/ours）

关键交付：`forge-init` 实现 lifecycle 感知的资产过滤；治理资产与 project lifecycle 的映射可验证

**阶段五（2-3 sprints）—— 状态分布化起步**

- **方向 B 子集**：`internal/store` 的 SQLite 实现（`modernc.org/sqlite` 纯 Go）
- **核心状态迁移**：Memory 和 Persist 支持 SQLite 后端，文件系统仍为默认
- `forge run --store sqlite` 实验性标志

关键交付：SQLite 后端可用且经过 `forge accept` 全绿验证；两后端切换能力

### 风险点与缓解策略

| 风险 | 级别 | 缓解 |
|---|---|---|
| 方向 A 的 TF-IDF 语义去重准确度不够 | 中 | 增量采用：去重作为 advisory 告警而非自动阻止；与 `Supersedes` 手动机制并行 |
| 方向 B 的 SQLite 引入破坏零外部依赖承诺 | 高 | 需要早期 ADR 明确决策；文件系统实现始终可用，SQLite 是可选后端 |
| 方向 C 的 Agent 虚假置信度导致错误路由 | 中 | `ConfidenceAdjustTier` 预设为"raise-only"（同 `Higher` 安全模式）；增加 FileDelta 交叉验证 |
| 方向 E 的 governance.yml 命名冲突 | 低 | 与 `modes.yml`/`policies.yml` 同级，schema validation 通过 `check.py` 强制 |
| 方向 D 的 CI 自举增加维护负担 | 低 | `forge evolve --executor dry` 不消耗 API 预算；真 agent CI 为可选 nightly job |
| 多方向并行时的架构整合成本 | 中 | 建议方向 A + 方向 C 共享 Sprint（两者都涉及 Memory/Signals 的改动）；方向 D + E 在后 |

---

**总结**：ForgeOS 当前的架构质量在 AI 治理类系统中处于领先地位——诚实原则、零外部依赖、中枢旋钮、非轮数收敛都是重要的架构差异化特性。五个扩展方向各有侧重，其中**方向 E（治理资产生命周期）** 独特性最强、重叠最少、实施风险最低，建议优先在阶段四落地；**方向 A（知识语义升级）** 是当前 Memory 包的最短板，建议在阶段二优先补齐。方向 B 的状态分布化是北极星方向但需要谨慎评估零依赖承诺的代价，建议通过 ADR 明确定义边界后逐步推进。
