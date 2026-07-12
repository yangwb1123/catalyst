现在我已掌握全面信息。以下是我的架构分析。

---

# ForgeOS 架构分析报告

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构设计具有几个值得称赞的特点：

**A) 清晰的分层与关注点分离**

代码库展示了一个严格的分层结构，这在自研编排系统中是罕见的：

```
asset（声明式工作负载模型）→ converge（收敛评估）
                              → orchestrator（有状态执行引擎）
                              → gate（治理闸门）
                              → memory / trace / persist（有状态持久化）
```

每一层都严格单向依赖。`asset.Phase` 是一个纯数据载体，不了解收敛、编排或执行器。`converge.Converge` 是一个纯函数——它接收 `StopCondition` 和 `Signals`，返回结果和布尔值，没有副作用，没有 I/O，没有外部调用。`orchestrator.Engine` 通过 Go 接口（`AgentExecutor`、`RunGate`、`OnGateResult`、`AgentVerdict`、`BudgetExhausted`）注入其所有副作用，使其在无模拟的情况下即可单元测试。

**B) 诚实的边界标注**

该代码库持续且细致地标注了哪些是真实的，哪些是占位符：

- 每个 `Phase` 字段文档都包含「ADDED HERE ONLY: this field is decoded, but nothing reads it yet」
- `orchestrator.go` 中关于 `durable_wait` 的注释承认：「v1 does NOT implement a durable cross-process wait」
- 关于 `Readonly` 执行的注释承认它已解码但未强制执行
- `ROADMAP.md` 中的「明确遗留缺口（诚实标注，不夸大）」部分

这种纪律防止了镀金，并让未来的开发者知道哪些行为是有意为之，哪些只是尚未实现。

**C) 收敛模型是正确的**

不允许轮次计数的终止——工作在满足实际标准时收敛，而不是在 N 轮迭代后。这是许多编排系统中容易出错的地方，ForgeOS 在这方面做得对。`converge.go` 中的 `Signals` 结构体包含 `RoadmapCompletion`、`GatesGreen`、`RequirementConfidence`、`ReviewStatus`、`HumanApproved` 和 `Criteria`——每一个要么被机械计算，要么来自一个可验证的 agent 输出解析器。没有软性信号。

**D) 中枢旋钮（mode × lifecycle）是优雅的设计**

一个设置控制三个维度：Router 档位、Harness 严格度和 Workflow 深度。`production` lifecylce 对三者都施加全开执行，松散模式无法绕过它。`explorer → engineering` 迁移（`forge migrate`）是一个真实的有状态转换，会生成补债任务。

**E) 工程纪律内化为代码**

500 行文件限制、50 行函数限制、零循环依赖、禁止上帝对象——这些不是写在文档中的愿望，而是由 `arch-check` 和 `gate.mjs` 自动执行的。代码库本身遵守自己的规则。

### 1.2 当前架构的局限性

**A) 工作流隔离：最大的架构缺口**

当前架构将每个工作流视为独立单元。`forge run design` 和 `forge run build` 是独立调用，没有机制将它们的执行链式连接。`StopCondition.OnApproved.NextStage` 字段已声明但未被任何消费。没有 `PipelineEngine` 或 `WorkflowComposer`。

问题不在于 `converge.Converge` 忽略 `next_stage`——它不应该处理它，这是一个纯收敛函数。问题在于 `cmd/forge/main.go` 和 `cmd/forge/evolve.go` 中的编排循环接收收敛结果但除了记录之外没有对 `next_stage` 做任何操作。需要一个新的编排层位于各个引擎之上。

**B) 无状态是虚构的，但持久化是脆弱的**

所有持久化状态（trace.jsonl、memory.jsonl、checkpoint.json、`.forge/<stage>.approved` 标记）都是文件系统 JSONL，没有任何并发控制。两个并发的 `forge evolve` 进程可以：

- 导致对 trace.jsonl 的交错追加（在 O_APPEND 下逐行安全，但不能原子地读取截断）
- 从 memory.jsonl 读取不一致的快照
- 静默覆盖彼此的检查点

在 v0/v1 的单用户背景下，这可能是可以接受的，但随着系统朝着多租户和自治运行的方向发展，这最终会导致数据丢失。方向一和方向四（组合引擎和并行治理）使此问题恶化，因为它们创建了更多并发状态访问模式。

**C) cmd/forge 层过厚**

尽管经过多次拆分，`cmd/forge` 在 16 个文件中包含约 3500 行代码，并且包含：

- 提示构建（将编排状态转换为 agent 提示的层）
- 输出解析（解析 `VERDICT: APPROVE`、`CONFIDENCE: 85` 的层）
- 检查点接线（将 Engine.OnPhase 连接到持久化的层）
- 工作流迁移逻辑

分层原则表明这应该是一个薄 CLI 适配器，真正的编排逻辑在 `internal/` 包中。`repeated` 文件数预算违规（Sprint 27、29、30 均因 cmd/forge 超出文件限制而触发架构自纠）是一个症状，而不是原因。根本原因是自然边界（engine→prompt→parse→checkpoint）被一个模拟的「薄 CLI」层模糊了，而该层实际上承载着大量的编排智能。

**D) YAML Python shim 是一个序列化边界**

Go 标准库没有 YAML 解析器，这迫使在 Python 中进行带外转码（`harness/yaml2json.py`）。这引入了：

- 对 Python 运行时的构建时/运行时依赖（与零外部依赖要求相矛盾）
- 一个序列化边界，使 Go 类型与 Python 的 YAML 解析结果之间的类型转换变得脆弱（块标量损坏 bug 就是一个具体的例子）
- 无法进行工作流资产的原生 Go 验证

**E) 回退参数不一致**

`overloadBackoffBase` 和 `overloadBackoffCap` 是包级常量（`backoff.go:70-71`），硬编码为 2s 和 60s。但 `MaxRetries` 是 `Engine` 结构体中的一个字段，可通过 `--max-retries` 进行配置。它们位于同一层（引擎的容错策略），但配置方式不一致。要么两者都应该是引擎字段，要么两者都应该是包级常量。

### 1.3 架构债务

| 债务 | 严重程度 | 理由 |
|---|---|---|
| YAML shim 序列化边界 | **高** | 导致真实 bug（块标量损坏），阻塞 Go 原生验证，创建一个脆弱的跨运行时依赖 |
| cmd/forge 层厚 | **中** | 导致反复的文件数预算违规，模糊了 CLI 和编排之间的界限 |
| JSONL 无并发控制 | **中** | 在并发/部分写入/断电下可能导致静默数据丢失（已知问题，已有文档记录但未解决） |
| 硬编码回退常量 | **低** | 与可配置的 MaxRetries 不一致；限制了运维灵活性 |
| on_rejected 扫描但未到达 | **低** | 代码存在且有测试覆盖率，但当前单次 CLI 架构无法触发它 |
| blocking: true 死字段 | **无** | 项目中从未使用过 `blocking: false`；完全不接线是正确的决定 |

---

## 2. 扩展方向

以下是我基于对代码库的深入分析提出的五个高价值架构扩展方向。

### 方向 A：工作流组合引擎（P0 — 最高杠杆）

**为何需要（商业价值）：**

这是从「CLI 工具」到「自治平台」的架构跃迁。目前，用户必须手动编排 `forge run design` → 等待审批 → `forge run build` → `forge run evolve`。组合引擎将此变为一个声明式管道：一份设计文档说明 `stop.on_approved.next_stage: build` 后，组合引擎会自动启动构建。这解锁了 24 小时无人值守的「idea→production」愿景，而这正是该项目的主要目标。

**为何需要（技术价值）：**

组合引擎将激活三个目前已死的基础设施组件：
- `StopCondition.OnApproved.NextStage`（已声明，零消费）
- `.forge/<stage>.approved` 标记基础设施（仅为 `forge run design --approved` 构建，未路由至下一阶段）
- `forge-core/internal/routing` 的路由决策（可用于确定下一阶段使用哪个模型）

这是三处中最高杠杆的——组合引擎将具有最高即时投资回报率(ROI)的声明式管线连接到一起，而无需新声明或新基础设施。

**核心挑战与技术难点：**

1. **状态传递语义**：设计工作流的批准信号必须传递到构建工作流的执行环境。当前 `.forge/<stage>.approved` 标记是一个文件系统信号。组合引擎需要：
   - 轮询或监听文件系统标记
   - 或是采用事件驱动方法（更适合持久化工作流，但当前架构不支持）

2. **故障模式**：如果设计已批准但构建失败，状态是什么？设计需要重新审批吗？组合引擎需要处理部分失败，而不是丢失已批准状态的归因。

3. **上下文传播**：设计工作流的产物（架构文档、ADR）必须对构建工作流可用。目前，`phaseOutputLedger` 仅限于单工作流。组合引擎需要一个「工作流级输出注册表」，在转换时持久化关键产物路径。

4. **跨工作流收敛评估**：目前，`converge.Converge` 评估单个工作流的停止条件。组合引擎需要评估整个管道的进度——不是「设计是否收敛？」而是「整个 idea→production 过程是否取得了适当的进展？」

**预期架构变更：**

```
NEW: PipelineEngine (internal/pipeline/)
  ├── 构建 PipelineState（当前阶段、所有先前阶段的累积结果）
  ├── 调用 Engine.Run（用于内工作流执行）
  ├── 消费 Converge 结果
  └── 根据 OnApproved 路由至下一阶段

cmd/forge/main.go:
  └── runWorkflow: 调用 PipelineEngine.Run 而非直接 Engine.Run
  
internal/persist/:
  └── 新的 pipeline_state.json（每个管道一个，非每个阶段一个）
```

具体来说：

```
// 新的 PipelineEngine 类型（概念性，非最终设计）
type PipelineEngine struct {
    Stages []StageBinding  // 工作流 → 依赖映射
    State  PipelineState   // 当前阶段、累积产物
}

type PipelineState struct {
    CurrentStage  string
    Completed     []StageResult
    Artifacts     map[string][]string  // 阶段 → 产物路径
    Approved      map[string]bool      // 哪些阶段已获审批
}

type StageResult struct {
    Stage      string
    Converged  bool
    NextStage  string  // 从 OnApproved 解析
    Error      error   // 如果失败则为非空
}
```

管道引擎将不同于 LoopEngine——它不驱动单工作流的迭代收敛；它驱动工作流间的有状态转换。

**对现有系统的影响：**

- `cmd/forge/main.go` 和 `cmd/forge/evolve.go` 需要重构，以便组合引擎是一个薄的编排层，在各个引擎之上，而不是更改引擎本身。
- `converge.go` 保持不变（这是一个纯函数）；变更仅发生在消费方。
- 对现有工作流的影响为零——管道引擎仅在创建显式阶段绑定时被激活。没有阶段绑定的 `forge run design` 行为与当前完全一致。

**技术风险：**

- **风险**：组合引擎创建了一个跨越多个工作流的有状态执行——如果进程在管道中间死亡，恢复逻辑变得更加复杂。**缓解**：从现有 `persist.Checkpoint` 模式中汲取经验，为 `PipelineState` 提供每阶段检查点支持。
- **风险**：如果构建工作流开始新迭代时设计工作流的上下文已经改变，则「设计已批准 → 构建运行」的有状态转换可能导致无法恢复的漂移。**缓解**：通过将产物路径快照化到 PipelineState 中来快照化设计上下文，而不是按需重新读取它。

---

### 方向 B：并行资源治理框架（P1 — 经济安全）

**为何需要（商业价值）：**

`--parallel` 标志已实现并合并。没有治理机制地运行它会立即带来经济风险——100 个并发 claude 进程并行启动，每个进程都会产生 API 费用。在企业环境中，这在经济上是不可持续的（成本失控），在技术上也是不安全的（速率限制、资源耗尽）。治理框架是 `--parallel` 在企业中可用的前提条件。

**为何需要（技术价值）：**

`orchestrator.go` 中的 `Engine` 已经具备预算原语（`MaxAgentCalls`、`BudgetExhausted`、`MaxRetries`、`MaxLoopBack`），但这些是针对串行执行的单维度硬限制。并行执行需要更丰富的治理模型：

- **并发上限**：波中的最大并行阶段数（目前是所有独立阶段，没有限制）
- **每波预算分配**：一阶段失败后，其余阶段应取消——`runWave` 通过上下文取消来实现这一点，但取消不回收已花费的预算
- **速率限制感知调度**：如果超过 API 速率限制，则延迟阶段执行——目前没有机制

**核心挑战与技术难点：**

1. **与运行级预算交互**：`MaxAgentCalls` 目前是一个跨运行增加的原子计数器。在并行模式下，N 个并发的 `runAgentPhaseBudgeted` 调用会竞相消耗同一个预算。需要细粒度的锁或预算的原子递减策略。

2. **成本可见性**：Wave 取消目前会记录「x 阶段被丢弃——潜在成本损失」，但不会追踪已启动但因取消而从未完成阶段的已花费美元金额。被取消阶段的有效 API 成本丢失了。

3. **治理表达**：治理策略目前嵌入在 Engine 结构体中（数字字段）。并行治理需要更丰富的表达——例如「最多 5 个并发 agent，总共最多 20 个 agent 调用，在请求中跨阶段分布」。这会推动一种策略作为数据的模式。

4. **反馈控制的背压**：`RunParallel` 目前的波是静态划分的（依赖于 `depends_on` 声明）。动态治理需要波调度器感知资源可用性——可能需要将可运行阶段的队列化与它们的实际执行分离。

**预期架构变更：**

```
NEW: internal/governance/scheduler.go
  ├── ResourcePool（槽位结构，例如 maxConcurrent=5）
  ├── Schedule（根据预算可用性延迟阶段触发）
  └── BudgetLedger（运行级预算跟踪，跨阶段聚合成本）

NEW: internal/governance/policy.go
  ├── ParallelPolicy（max_concurrent、budget_per_wave、per_model_budget）
  └── ReadPolicy（从 project.yml 或 modes.yml 解析）

orchestrator/parallel.go:
  ├── runWave: 使用 ResourcePool.Acquire 而非无限制的 goroutine 启动
  └── runPhaseParallel: 使用 BudgetLedger 进行每阶段预演预算检查

cmd/forge/engine_build.go:
  └── buildRunEngine: 从 --max-parallel、--max-budget-usd 标志注入治理策略
```

具体设计决策：

```
// 概念性调度器 API
type Scheduler struct {
    pool   chan struct{}    // 控制槽的带缓冲通道
    budget *BudgetLedger    // 跨阶段运行级预算
}

func (s *Scheduler) Acquire(ctx context.Context) error {
    select {
    case s.pool <- struct{}{}:  // 获取槽位
        return nil
    case <-ctx.Done():
        return ctx.Err()
    }
}

func (s *Scheduler) Release() {
    <-s.pool  // 释放槽位
}
```

`pool` 方法相比信号量方法具有优势——它原生支持取消，与并行引擎的现有上下文传播模式契合，且不需要额外的锁。

**对现有系统的影响：**

- 对串行路径（`RunFrom`）无影响。治理框架完全可选，且仅在并行模式下激活。
- `--parallel` 标志需要对并行治理策略有新的必需默认值——没有默认治理的平行路径不应是无限制的。
- 现有的 `MaxAgentCalls` 和 `BudgetExhausted` 保持不变；治理框架位于它们之上，不替代它们。

---

### 方向 C：跨运行知识声明周期管理（P1.5 — 与方向 A 有依赖关系）

**为何需要（商业价值）：**

如果没有跨运行知识，工作流组合就无法有效传递设计架构决策到构建过程。组合引擎（方向 A）依赖于持久化知识——没有它，第二个工作流在第一个工作流的上下文中盲目启动。这使得方向 C 成为方向 A 的自然依赖项。

**为何需要（技术价值）：**

当前状态有缺口：
- `persist/checkpoint.go` 保存 `LoopState`（迭代轮次、已完成的阶段列表、预算余额）——跨运行持久化已完成
- `memory` 包（memory.jsonl）是追加日志——设计用于持久化，但没有任何进入构建工作流的摄入机制
- 没有明确的「跨运行知识图谱」——所有知识都是每次执行本地的

**核心挑战与技术难点：**

1. **知识分类**：并非所有知识都应该跨运行持久化。区别如下：
   - **情景记忆**（此迭代特有的发现，例如「第 3 阶段因预算耗尽而失败」）——不应跨运行持久化
   - **语义知识**（架构决策、通用约束、学习到的模式）——应跨运行持久化
   - **程序知识**（workflow 如何执行的，例如「设计已批准→构建可以开始」）——应作为管线状态的一部分持久化

2. **记忆衰减与压缩**：memory.jsonl 仅在增长，没有压缩机制。100 次迭代后，1000 条条目被存储，但只有最近的 10 条相关。需要一种压缩/汇总机制。

3. **冲突解决**：如果两个独立的 `forge run` 调用产生关于同一主题的矛盾知识，如何解决？ForgeOS 目前没有冲突解决机制——最后写入获胜。

4. **查询接口**：`memory.Query` 是一个内存过滤器。跨运行查询需要从 disk 加载 memory 并应用过滤。对于大型语料库，这变得昂贵。查询需要变得懒惰和基于索引。

**预期架构变更：**

```
NEW: internal/knowledge/store.go
  ├── Store（知识的高层接口，不限于 JSONL）
  ├── Entry 类型（标记化：semantic、episodic、programmatic）
  └── Query（结构化过滤器，例如 type=semantic AND age<7d）

EXTEND: internal/memory/memory.go
  ├── Compact（汇总旧条目为摘要条目，删除单独条目）
  └── Prune（删除超过 age_threshold 或 keepPerKind 的条目）

NEW: internal/knowledge/bridge.go
  ├── LoadForWorkflow（为特定工作流加载知识——在工作流启动时调用）
  └── StoreFromWorkflow（在工作流收敛时存储知识）

cmd/forge/prompt_context.go:
  ├── injectKnowledge（将持久化知识注入到工作流提示中）
  └── 方向 A 的组合引擎调用 bridge.LoadForWorkflow
```

**注意事项：**

`memory.go:327-349` 中的 `Compact` 已经存在，有一个已知的 bug（`summarizeBlock` 对单主题计数和总计使用相同的映射键——当 `Topic` 与 `Kind` 字符串碰撞时，会导致静默漏记和重复计数）。在扩展前，此 bug 需要修复。

`memory.go` 的 `Prune` 有一个 `keepPerKind` 夹紧机制——但 `Compact` 没有。这需要在扩展前统一。

**对现有系统的影响：**

- 对独立工作流执行无影响。知识桥仅在跨工作流组合被激活时启用。
- `memory` 包保持向后兼容——现有代码导入 `memory.Append` 和 `memory.Load` 不变。
- `forge evolve` 的迭代内行为不变。变更仅发生在迭代边界（存储）和工作流边界（加载）。

---

### 方向 D：阶段输出契约验证（P2 — 通过优先排序提升至 P2）

**为何需要（商业价值）：**

目前，`Emits []string` 只是没有验证的路径列表。agent 声明其生成 `docs/architecture.md`，但系统不验证该文件是否存在或包含预期结构。在自治运行中，不验证输出的 agent 可能会声称生成了架构文档，但实际上却没有——下游 agent 依赖于不存在的文件。

**为何需要（技术价值）：**

跨阶段契约是任何模块化系统的关键基础设施。目前：
- `Emits` 是 `[]string`——没有模式，没有必需的部分，没有验证
- `writes_adr` 是一个更丰富的结构（`{condition, target}`），展示了一个结构化声明可以如何进行模式化
- `feeds_forward=true` 告诉系统某阶段的输出与后续阶段相关——但输出是否结构正确仍未被验证

**核心挑战与技术难点：**

1. **验证时机**：验证可以在执行后立即进行（agent 完成后检查产物），也可以在消费时进行（使用产物的阶段启动时验证）。执行后验证更快失败，但需要为每个 agent 阶段定义一个模式。消费时验证更懒，但会冒下游 agent 在使用无效输入时失败的风险。

2. **模式演化**：如果阶段 A 声明它生成 `docs/architecture.md`，而阶段 B 期望它包含特定部分，当架构文档模式发生演进时会发生什么？模式版本控制是必需的。

3. **成本与精度权衡**：存在检查可以通过文件存在性检查（低成本，低精度）或语义验证（检查文件包含预期部分——高成本，高精度）来完成。增量路径是先检查存在性，然后在模式稳定后添加语义验证。

4. **声明与执行**：契约验证可以是声明的（在 `Phase.Emits` 中声明，自动检查）或执行的（使用验证器函数手动编码）。声明的更好，因为它保持与声明式工作流范式的一致。

**预期架构变更：**

```
EXTEND: internal/asset/asset.go
  ├── EmitSpec 结构体（替换裸 []string）
  ├── path: string（文件路径）
  ├── schema_ref: string（可选的模式文件路径）
  └── required_sections: []string（可选的必需文本部分）

NEW: internal/validate/contract.go
  ├── VerifyEmits(phase, workdir)（验证所有声明的产物是否存在且结构正确）
  └── VerifyRequiredSections(path, sections)（验证文件包含特定部分）

orchestrator/orchestrator.go:
  └── runAgentPhase: 清理后调用 VerifyEmits

cmd/forge/prompt_context.go:
  └── injectEmits（将验证通过的产物内容注入后续阶段的提示中）
```

`EmitSpec` 的设计应遵循已建立的 `writes_adr` 模式：

```go
// 当前设计（asset.go:155）：
Emits []string `json:"emits,omitempty"`

// 提议设计：
type EmitSpec struct {
    Path             string   `json:"path"`
    SchemaRef        string   `json:"schema_ref,omitempty"`     // 可选的模式路径
    RequiredSections []string `json:"required_sections,omitempty"` // 可选的必需部分
}
```

直接替换 `Emits` 字段将破坏所有现有工作流。相反，`Emits` 应保持为 `[]string` 作为默认值，而 `EmitSpecs` 作为一个新字段被添加：

```go
Emits      []string    `json:"emits,omitempty"`       // 保持向后兼容
EmitSpecs  []EmitSpec  `json:"emit_specs,omitempty"`  // 新的结构化变体
```

装载机应识别两者：如果 `EmitSpecs` 存在，请使用它进行验证；否则，回退到基于 `Emits` 的裸存在性检查。

**对现有系统的影响：**

- 通过将 `Emits` 保留为字符串数组，保持向后兼容。
- 验证可以逐步添加——先是存在性检查，然后是模式验证，最后是内容验证。
- `AgentOutput` 解析器不需要更改——验证发生在 agent 完成后，与输出解析无关。

---

### 方向 E：跨方向可观测性基础（P2 — 新增，未体现在原始五个方向中）

**为何需要（商业价值）：**

这是对原始五个方向中缺失的「可观测性缺口」的补充。当前所有共享状态机制（trace.jsonl、memory.jsonl、checkpoint.json、`.forge/*.approved` 标记）都是具有 TOCTOU 和静默数据丢失风险的文件系统 JSONL。随着组合引擎（方向 A）和并行执行（方向 B）的添加，并发状态访问会显著增加——将问题从理论缺陷放大为实践障碍。如果没有可靠的可观测性，就不可能调试管道故障。

**为何需要（技术价值）：**

目前的可观测性现状：
- `internal/trace` 包发出事件到 trace.jsonl——一次一条记录，通过 O_APPEND，没有事务保证
- `internal/persist` 使用 checkpoint.json——一个覆盖性 JSON 文件，在崩溃时可能部分写入
- `internal/memory` 使用 memory.jsonl——一个追加日志，在加载时严格解析（损坏的行 = 错误）
- 没有结构化日志（引擎打印格式化的行到 `Log func(string)`——仅消费者可观测）
- 没有跨进程锁定协议

具体风险：

| 组件 | 风险 | 发生条件 |
|---|---|---|
| trace.jsonl | 交错写入（逐行安全，非原子写入） | 两个进程同时写入 |
| checkpoint.json | 部分写入（覆盖非原子） | 写入过程中的崩溃 |
| memory.jsonl | 损坏行（追加期间崩溃） | 写入过程中断电 |
| .forge/*.approved | TOCTOU（检查后批准前） | 顺序运行的并发检查 |

**核心挑战与技术难点：**

1. **原子性与崩溃安全**：在没有外部数据库的情况下，文件系统持久化有固有的限制。选项包括：
   - 写入临时文件 + 重命名（原子写入，但对于高频率不够用）
   - 预写日志（复杂，但真正崩溃安全）
   - 仅追加日志（简单，但需要读取时补偿）

2. **跨进程锁定**：如果两个 `forge` 进程同时运行（为不同项目，或为同一项目并行运行），它们需要避免相互的状态文件冲突。文件锁定（`flock`）是可行的，但不可移植，且如果进程在被锁定前崩溃，会留下陈旧的锁。

3. **统一跟踪上下文**：目前，trace 事件有阶段范围上下文，但没有跨阶段/跨运行跟踪 ID。组合引擎需要能够关联一个设计运行及其后续构建运行的 trace 事件。

**预期架构变更：**

```
EXTEND: internal/trace/trace.go
  ├── 添加 RunID（每个工作流执行唯一的 UUID）
  ├── 添加 PipelineID（由组合引擎传播的可选父级跟踪 ID）
  └── 添加 RecoveryMark（指示这是重新启动而不是新运行的检查点标记）

NEW: internal/persist/lock.go
  ├── FileLock（围绕使用 timeout 的 flock 的薄包装）
  ├── Lock(path)（获取路径的排他锁）
  └── Unlock(path)（释放锁）

NEW: internal/persist/atomic.go
  ├── WriteJSONAtomic(path, data)（写入临时文件 + 重命名以确保原子性）
  └── AppendJSONLAtomic(path, data)（使用日志序列号进行崩溃安全的追加）
```

`Atomic append` 需要特别注意——O_APPEND 是逐行原子的，但读取器可能看到部分写入的行。一种常见的模式是在每条记录前加上一个长度前缀：

```
// 一行 trace.jsonl 的当前格式：
{"event":"phase_complete","phase":"implementer","duration_ms":5000}\n

// 提议的崩溃安全格式：
\x01{"event":"phase_complete","phase":"implementer","duration_ms":5000}\n
```

`\x01` 字节充当记录标记——读取器看到一个 `\x01` 但没有换行符的任何内容，都很明显是一部分写入的记录，可以跳过或重试。

**对现有系统的影响：**

- 所有现有的 trace.jsonl 文件需要进行前向兼容——添加记录标记不应破坏旧的读取器。
- `internal/persist` 和 `internal/trace` 中的所有现有写入器将使用新的原子写入辅助函数。变更将限于持久化层，无需更改编排逻辑。
- 锁定是可选的，且默认不激活——仅当检测到并发进程访问同一状态目录时才激活。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：引擎是状态机，不是服务**

`orchestrator.Engine` 的正确心智模型是一个可恢复的状态机：

```
输入：工作流 + 信号 + 执行器
状态：当前阶段索引 + 预算消耗 + 循环回退计数
输出：阶段结果 + 收敛评估 + 已用资源
副作用：通过注入的回调
```

此模型可正确设计，不得更改。`Engine` 不应直接知道：
- 磁盘上的检查点（由 `OnPhase` 和 `OnIteration` 回调处理）
- 提示构建（由 `OnGateResult` 和 `AgentVerdict` 回调处理）
- 资源治理（由 `BudgetExhausted` 回调和 `MaxAgentCalls` / `MaxLoopBack` 字段处理）

**原则 2：回调是扩展点，不是依赖项**

使用回调（`OnPhase`、`OnGateResult`、`AgentVerdict`、`BudgetExhausted`、`Sleep`、`Log`）进行副作用注入的模式是正确的。它保持引擎为纯状态机，其所有副作用都是显式的和可测试的。

对于组合引擎（方向 A），`PipelineEngine` 应遵循相同的模式：

```
PipelineEngine:
  输入：多个工作流 + 组合规则
  状态：当前阶段 + 跨工作流产物表
  回调：OnStageComplete、OnPipelineComplete、OnApprovalRequired
  输出：每个阶段的收敛结果
```

**原则 3：声明式配置，而非编程式接线**

当前设计使用声明式 YAML（`project.yml`、`modes.yml`、`workflows/*.yml`）来表示策略。这对组合引擎和治理框架也应如此：

```yaml
# 提议的 pipeline.yml 示例
stages:
  - workflow: discover
    stop_on: requirement_confidence >= 80
    on_approved:
      next: design
  - workflow: design
    stop_on: human_approval
    on_approved:
      next: build
  - workflow: build
    stop_on: roadmap_completion == 100 AND gates_status == green
    on_complete:
      next: evolve
```

与以编程方式将阶段绑定在 Go 代码中相比，声明式方法（a）保持与现有范式一致，（b）使管道可被 tooling 验证，（c）允许未来添加 UI 支持。

### 3.2 是否需要新的抽象层

是的。需要三个新的抽象层：

**A) PipelineEngine（方向 A）**

位于 `Engine` 之上，编排跨工作流转换。类似于 `LoopEngine` 构建在 `Engine` 上进行迭代收敛的方式，`PipelineEngine` 构建在 `Engine` 之上进行工作流级编排。

```
LoopEngine:    Engine 之上 for 迭代循环
PipelineEngine: Engine 之上 for 工作流链
```

由 `cmd/forge` 根据工作流资产是否存在阶段绑定来决定调用哪一个。

**B) Scheduler（方向 B）**

位于并行引擎和资源治理之间。解耦了「应何时运行此阶段？」（调度决策）与「如何运行此阶段？」（执行）。这使得治理策略可以是可插拔的。

```
RunParallel → Scheduler → ResourcePool + BudgetLedger → runPhaseParallel
```

调度器检查预算、速率限制和并发上限，然后才将阶段分派给执行引擎。此分离使串行路径不受影响。

**C) KnowledgeStore（方向 C）**

位于 memory/persist 包和提示构建器之间。提供结构化查询 API（按类型、年龄、相关性过滤），而不是原始的 `Load(path) []Entry`。知识存储还可以处理压缩和冲突解决，使提示构建器无需知道确切的存储格式。

```
prompt_context.go → KnowledgeStore(Query) → memory.Load(path) → Entry[]
                                    → persist.Load(path) → LoopState
```

### 3.3 如何保持向后兼容性

**组合引擎：**
- 仅在没有阶段绑定或未达到 stopping condition 时激活。`forge run design` 的表现与当前完全一致。
- `PipelineState` 是一个新文件，不会影响现有的 checkpoint.json 或 memory.jsonl。

**调度器：**
- 串行路径（`RunFrom`）完全不受影响。
- 并行路径（`RunParallel`）仅为未配置调度器的情况提供回退默认值。默认值为「无治理」——与当前行为相同（但建议启用某种治理）。

**知识存储：**
- `memory.Append` 和 `memory.Load` 保持不变。
- `KnowledgeStore` 是导入现有 memory 包的其他 Go 代码的可选上层。无导入更改。

**阶段输出契约：**
- `Emits []string` 被保留。新字段 `EmitSpecs []EmitSpec` 可选。
- 验证是选择加入的——仅当 `EmitSpecs` 存在时调用。

**可观测性：**
- trace.jsonl 格式通过添加前缀记录标记进行前向兼容——旧的读取器看到记录标记后跟一个 `{` 的字节，该字节是有效的 JSON 开头，并会读取直到换行符。
- 锁定是可选的，默认不激活。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**不。** 当前堆栈（纯 Go 标准库 + Python YAML shim + Node.js 治理工具）应对提议的扩展绰绰有余。引入外部依赖将：

- 与 ForgeOS 的「零外部依赖」核心原则相矛盾
- 增加构建时间和供应链风险
- 引入当前项目展示出了高技能的团队不必要学习的框架

具体框架决策：

| 框架 | 考虑时间 | 决策 |
|---|---|---|
| Temporal（持久化工作流） | 在方向 A（组合引擎）的背景下 | **否决**于 v1/v2。Temporal 对 durable_wait 和长时间运行的管道非常有价值，但会引入 gRPC + 数据库 + 服务依赖。组合引擎的 v1 实现应使用文件系统信号（`.forge/*.approved` 标记）+ 轮询。Temporal 是方向 A v2 的自然路径。 |
| OPA/Rego（策略引擎） | 在方向 B（治理）的背景下 | **否决**于 v1。治理策略目前足够简单（整数界限），可用 Go 代码处理。OPA 对于更丰富的策略表达式（例如「前 10 次调用可用 Haiku，然后回退到 Sonnet」）会很有价值。模式即数据原则仍可使用简单的 YAML 来应用，无需 OPA。 |
| Qdrant（向量存储） | 在方向 C（知识）的背景下 | **否决**于 v1。目前的知识负载（按 1,000 行/次数量的 JSONL 条目衡量）不需要向量搜索。基于标签的线性扫描就足够了。Qdrant 在知识基数达到数万条目时是有价值的。 |
| LiteLLM（模型网关） | 在模型路由的背景下 | 已否决于 v3。已做出正确决定。 |

**唯一的依赖决策方向**是用 Go YAML 库替换 Python YAML shim。这是一个非平凡的决策：

| 选项 | 优点 | 缺点 |
|---|---|---|
| **维持 Python shim** | 零 Go 依赖，现有实现可运行，利用了 Python 成熟的 YAML 解析器（PyYAML） | 需要安装 Python，脆弱的序列化边界（块标量 bug），无原生 Go 验证 |
| **添加 Go YAML 库**（goccy/go-yaml或ghodss/yaml） | 消除了 Python 依赖，Go 原生验证，更简单的构建 | 违反零外部依赖原则，需要依赖审计，增加构建时间 |
| **手写 Go YAML 解析器** | 保持零依赖，完全控制 YAML 子集 | 工程工作巨大，为解析器引入 bug 的风险高，高维护成本 |

**推荐**：在 v2 中添加 goccy/go-yaml（纯 Go，无 CGo），序列化边界修补在 SPRINT 27 已做。零依赖是一个有价值的纪律，但当前形式（需要一个 Python 运行时进行转码）实际上产生了一个更糟的依赖——它不是一个已声明的构建时依赖，而是一个运行时依赖，如果缺少 Python，会以不透明的方式失败。添加一个 Go YAML 库可以将此依赖前置化，使其可验证，并消除序列化边界。

### 4.2 第三方依赖的评估标准

如果引入第三方依赖（YAML 库是唯一候选），标准应为：

1. **纯 Go，无 CGo**：CGo 破坏了跨编译和静态链接，这两者对于单一的 Go 二进制部署都很重要。
2. **宽松许可**：MIT、BSD、Apache 2.0 可接受。GPL/AGPL 不可接受。
3. **无传递依赖**：如果库本身引入依赖，它们需要相同的审计。如果 library 引入超过 1-2 个传递依赖，最好手写解析器。
4. **API 稳定性**：库应具有稳定的 API（v1+），或处于积极维护中。
5. **许可证审计兼容性**：该库不应引入与 ForgeOS 自身许可冲突的许可证。

### 4.3 自建 vs 采购的决策依据

ForgeOS 已经做出了与 North-Star 架构一致的正确决策：

| 层 | 自建 | 采购 | 理由 |
|---|---|---|---|
| 编排逻辑 | 自建 | — | 核心差异化层；编排 AI agent 的方式是 ForgeOS 的竞争优势 |
| 治理模型 | 自建 | — | 深度绑定到 ForgeOS 自己的模式×生命周期模型 |
| 模型路由 | 自建 | LiteLLM 作为网关 | 路由决策是核心差异化层；LiteLLM 是跨厂商传输的 commodity |
| 沙箱 | — | Firecracker | 隔离执行是一个 solved problem；自建 microVM 没有差异化价值 |
| 策略引擎 | 自建（v1） | OPA（v3） | v1 约束很简单；OPA 对于更丰富的策略表达式很有价值，但不是 v1 必需的 |
| 持久化工作流 | 自建（v1） | Temporal（v3） | 文件系统信号 + 轮询对 v1 组合引擎足够；Temporal 的 durable_wait 对于长时间运行的人机审核有价值，但会引入显著的运维开销 |
| 可观测性 | 自建 | OTel 用于传输 | 跟踪事件格式是自建的，但 OTel 传输适用于未来与 Prom/Loki/Grafana 的集成 |

---

## 5. 实施路线图

### 5.1 优先级总结

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | A（工作流组合引擎） | 架构跃迁点；解锁 24 小时自治管道；激活三个已死的基础设施组件；对方向 C 有最大乘数效应 |
| **P1** | B（并行资源治理） | C 级经济安全；在启用真实并行执行前的前置条件；利用已存在的预算原语 |
| **P1.5** | C（跨运行知识生命周期） | 方向 A 的依赖项——组合引擎需持久化知识以进行跨工作流上下文传递 |
| **P2** | D（阶段输出契约验证） | 对自治 agent 场景有价值，但独立于方向 A 和 B |
| **P2** | E（可观测性基础） | 降低跨方向风险；在组合引擎和并行执行加剧状态并发问题前的前置条件 |

### 5.2 阶段和里程碑

**阶段 1：基础（在下个 sprint 中可交付）**

目标：建立方向 A 和 E 的基础结构，以最小化对其他方向的阻塞。

里程碑 1.1：**可观测性基础**（2-3 天）
- `internal/persist/atomic.go`：WriteJSONAtomic（临时文件 + 重命名）和 AppendJSONLAtomic（前缀记录标记）
- `internal/trace/trace.go`：添加 RunID 和可选的 PipelineID
- 迁移所有现有写入器到原子辅助函数
- 验证：现有测试通过；trace 文件在崩溃后仍有效

里程碑 1.2：**PipelineState 数据模型**（2-3 天）
- `internal/pipeline/state.go`：PipelineState 结构体、StageResult、Save/Load
- 为 PipelineState 增加 checkpoint.json 风格持久化（每阶段原子重写）
- 添加 RunID（从内部/trace 传播）
- 验证：单元测试 PipelineState 的保存/加载/恢复

**阶段 2：组合引擎 v1（下个 sprint）**

目标：在现有 CLI 架构之上的最小可行组合引擎。无新的 Engine 类型——仅通过调用 `forge run`（子进程执行）工作流链的薄编排层。

里程碑 2.1：**连接批准信号到下一阶段**（1 天）
- `cmd/forge/main.go`：在 `runWorkflow` 中，检查收敛结果中的 `OnApproved`——如果 `next_stage` 非空，则解析并 `.forge/<next>.approved` 写入
- `cmd/forge/evolve.go`：对 LoopEngine 的迭代做同样处理
- 验证：`forge run design --approved` 写入 `.forge/build.approved`（手动检查）

里程碑 2.2：**批准标记消费**（1 天）
- `cmd/forge/main.go` 和 `cmd/forge/evolve.go`：启动时，检查 `.forge/<stage>.approved`——如果找到，使用它作为 `HumanApproved=true` 并删除标记
- 验证：`forge run build` 读取并消费 `.forge/build.approved`；标记在运行后消失

里程碑 2.3：**组合 CLI 命令**（2 天）
- `forge compose design build`：按照先设计、等待批准、构建、等待批准的顺序运行工作流
- 使用 PipelineState 持久化进度，以便 `forge compose --resume` 从中断处恢复
- 不实现并发工作流——依次运行
- 验证：完整的端到端无人值守设计 → 构建管道（使用模拟批准/模拟 agent 的干运行模式）

**阶段 3：并行治理（下一版或下下版 sprint）**

目标：使 --parallel 对企业安全可用。最少配置，默认安全。

里程碑 3.1：**治理策略数据模型**（1 天）
- `internal/governance/policy.go`：Policy 结构体（max_concurrent、max_budget_usd_per_agent、rate_limit_window）
- 从 project.yml 或新 governance.yml 解析
- 验证：无治理的解析返回全部默认值；带治理的解析应用值

里程碑 3.2：**资源池**（2 天）
- `internal/governance/scheduler.go`：使用带缓冲通道进行槽位控制
- 集成到 `orchestrator/parallel.go`：`runWave` 在启动阶段之前获取槽位
- 默认 max_concurrent= 无限制（向后兼容）
- 验证：并发阶段的单元测试在达到限制时阻塞

里程碑 3.3：**治理感知的预算跟踪**（2 天）
- `internal/governance/budget.go`：跨阶段聚合成本，累积跟踪
- 集成到 `runPhaseParallel`：在每阶段预算检查之前增加已花费的成本
- 验证：当预算耗尽时，并行阶段不会被触发

**阶段 4：知识桥梁（下一版或下下版 sprint）**

目标：启用跨工作流上下文传递，使组合引擎可有效传递设计决策到构建过程。

里程碑 4.1：**知识分类和查询**（1 天）
- 扩展 `internal/memory`：为 Entry 添加 `Type` 字段（semantic|episodic|programmatic）
- `Query` 过滤器：按 Type、按 Age、按 Tag
- 验证：Semantic 和 Episodic 条目可独立查询

里程碑 4.2：**压缩和修剪**（2 天）
- `Compact`：将旧语义条目汇总为摘要条目；删除单独的旧条目
- `Prune`：删除超过 AgeThreshold 或 keepPerKind 的条目
- 验证：1000 条条目在压缩后缩减为 50 条摘要条目

里程碑 4.3：**工作流边界桥接**（2 天）
- `internal/knowledge/bridge.go`：LoadForWorkflow（在组合转换时调用）、StoreFromWorkflow（在工作流收敛时调用）
- `cmd/forge/prompt_context.go`：`injectKnowledge`（从当前工作流的相关持久化条目构建提示块）
- 验证：设计工作流写入的 ADR 条目在构建工作流的提示中被注入

**阶段 5：阶段输出契约（独立）**

目标：验证 agent 输出，跨工作流边界保证数据稳定性。

里程碑 5.1：**EmitSpec 数据模型**（1 天）
- 将 `Phase.EmitSpecs []EmitSpec` 添加到 `asset.go`
- EmitSpec：path、schema_ref（可选）、required_sections（可选）
- 验证：无 EmitSpecs 的现有工作流不受影响；带 EmitSpecs 的工作流正确加载

里程碑 5.2：**存在性检查**（1 天）
- `internal/validate/contract.go`：`VerifyEmits(phase, workdir)`——检查所有声明的 emit 路径是否存在
- 集成到 `orchestrator.go` 的 `runAgentPhase`（agent 完成后的清理步骤）
- 验证：缺少的 emit 路径以相应的失败消息报告

里程碑 5.3：**可选内容验证**（2 天）
- `VerifyRequiredSections(path, sections)`——验证文件是否包含指定部分
- 模式验证（schema_ref 指向 JSON Schema 文件时）
- 验证：包含所有部分（= 通过）和不包含部分（= 失败）的 fixture 文件

### 5.3 风险和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| 组合引擎创建新数据结构（PipelineState），但 checkpoint.json 仍是单文件——两个持久化格式需要保持同步 | 中 | 高 | 使 PipelineState 格式基于每阶段原子写入（类似 checkpoint.json），并添加恢复验证（检查是否所有必需的文件都存在） |
| 在没有 Temporal 或任何形式持久化工作流支持的情况下处理跨工作流审核 | 中 | 中 | v1 组合引擎不应处理持久化审核；应使用文件系统标记（`.forge/*.approved`）。当工作流需要等待审核时，以非零代码退出，告知用户运行 `forge run <下一阶段> --approved`。Temporal 集成延迟到 v2/v3 |
| 治理策略的膨胀——治理多个独立方向上过于复杂 | 低 | 中 | 从简单开始：max_concurrent（整数）+ max_spend_usd（浮点数）。在需要新语义之前不加策略抽象。OPA 只有在策略数量增长后才是在议程上 |
| 知识存储压缩无法足够快地跟上（100 个条目/迭代，100 次迭代 = 10,000 个条目） | 低 | 低 | 每个迭代压缩是一个 O(n) 扫描。100 次迭代后 10,000 个条目，即使是通过 JSONL 的线性扫描也应该 <100ms。维护可以推迟到 N > 100,000（合理的数据量） |
| 可观测性基础增加了持久化的延迟（原子写入引入 fsync） | 低 | 中 | 使用 O_SYNC 进行原子写入——延迟影响可测量，但减少崩溃窗。如果延迟成为问题，切换为批处理写入器（每 100ms 刷新缓冲的事件） |
| 阶段输出验证在阶段执行结束时增加了可测量的延迟 | 低 | 低 | 存在性检查是一个 stat 系统调用——亚毫秒级。内容验证取决于文件大小，但即使在 1MB 文件上也应 <10ms。在 T 级输出到来之前不是问题 |
| 阶段 1-5 的架构内聚力——五个独立的扩展可能产生冲突的实现策略 | 中 | 高 | **缓解**：每个阶段都参考本架构文档来验证其方向是否一致。在每次 PR 之前交叉验证依赖关系。阶段 1（基础）明确设计为阶段 2-5 的基础 |

### 5.4 依赖图

```
阶段 1（基础）
  ├── 可观测性基础
  └── PipelineState 数据模型
       │
阶段 2（组合引擎 v1）
  ├── 依赖阶段 1
  └── 审批信号路由
       │
阶段 3（并行治理）      阶段 4（知识桥梁）
  ├── 独立于阶段 2       ├── 依赖阶段 1
  └── 可早期交付         └── 最好与阶段 2 协调
       │                    │
       └──────┬─────────────┘
              │
        阶段 5（组合引擎 v2）
          ├── 集成阶段 3（并行治理）
          └── 集成阶段 4（知识桥梁）
```

阶段 3 和阶段 4 可平行推进——它们没有共享的架构依赖项。阶段 5（组合引擎 v2）是对阶段 2 的增强，增加了治理和知识传递，不是独立的实现。

---

## 总结：架构是否健康？

**是的，大体上是健康的。** ForgeOS 架构展现出的正是使长期演进成为可能的那种纪律：

- 明确、诚实的边界标注
- 通过注入进行依赖反转的模式
- 纯函数式收敛引擎
- 可验证的停止条件
- 不越过自己的红线

该架构没有根本性缺陷。缺陷是增量缺口——缺失的连接线、未消费的字段、脆弱的持久化——在 31 个 sprint 的积极开发中自然会积累起来的。

关键机会在于**工作流组合**（P0）。ForgeOS 已经构建了各个引擎（执行、收敛、循环、记忆、跟踪）。缺失的是将它们绑定到自治管道中的编排胶水。这是从「CLI 工具」到「AI 原生软件工厂」的剩余架构跃迁——也是当前架构中杠杆率最高的点。

中程风险在于**可观测性基础**（方向 E）。在解决组合和并行问题之前解决它，可防止在并发访问模式加剧当前持久化的脆弱性时出现未来的调试难题。这就是为什么它在路线图中被列为阶段 1。

长期看，**cmd/forge 瘦身**（将编排逻辑移入 `internal/` 包）将继续产生递增的收益，但这是清洁工作，而不是架构变革。真正的架构工作在于将当前在同一流程中串行执行的内容，转化为一个面向未来的、可组合的、可靠且可观测的持久化管道——在零外部依赖的限制下进行这一切。
