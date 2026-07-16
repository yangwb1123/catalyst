# ForgeOS — 深度扩展方向分析（第 15 轮）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库全量扫描（forge-core 35+ Go 文件、18+ CLI 子命令、harness 26+ 模块/测试、  
>   .agent 完整治理骨架 40+ 文件、examples 2 个真实 dogfood 项目），与已有的 27+ 份分析文档  
>   **交叉核对**以确保证据链闭环、不重复已有覆盖（已有覆盖在 §0 列出）  
> **纪律**: 不写代码；每个方向从代码级微观证据出发，到架构级缺口，再到边界情况  
> **基线**: Sprint 26 全状态（真 claude multi-agent 端到端坐实、Learning loop 三维真数据、  
>   parallel 模式含锁顺序契约、四维资源安全护栏完备、gate ledger feed-forward 闭环）

---

## §0. 已有 27+ 份分析已覆盖的域（本文不重复）

| 已有覆盖域 | 对应文档 |
|---|---|
| 并行编排竞态（fail-fast 短路） | `edgecases-and-perf.md` §1.1 → 已修复（per-wave context cancel） |
| 锁顺序书面契约 | `edgecases-and-perf.md` §1.3 → 已文档化在 `parallel.go` 头部 |
| trace/memory 文件无限增长 | `edgecases-and-perf.md` §2.1–2.2 |
| 收敛理论隐藏陷阱（门闩效应/零相位/空心收敛） | `edgecases-and-perf.md` §3 |
| YAML shim 进程开销 + 零依赖修复 | `edgecases-and-perf.md` §4.3 → 已原生 Go（`yaml2json.go`） |
| Scorecard 不感知 mode（切 mode 时历史偏见） | `edgecases-and-perf.md` §5.4 |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` |
| Self-testing 缺口 / dogfooding 纪律 | `self-testing-and-dogfooding.md` |
| cmd/forge 膨胀 / growth bottlenecks | `growth-bottlenecks-and-scalability.md` |
| ForgeOS 自身治理差距 | `expansion-forgeos-meta-governance.md` |
| 分岔/回滚引擎 | `expansion-core-five-2026-07-01.md` 方向四 |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` 方向一 |
| 统一验证引擎 / 三语言债务 | `expansion-core-five-2026-07-01.md` 方向二 |
| 实时可观测性层 / 流式仪表 | `expansion-core-five-2026-07-01.md` 方向三 |
| 跨工作流管道编排 | `expansion-core-five-2026-07-01.md` 方向五 |
| 多项目拓扑编排 / 跨仓治理 | `expansion-core-five.md` 方向一 |
| 架构-代码漂移持续检测 | `expansion-core-five.md` 方向二 |
| 预热启动 / 知识图谱缓存 | `expansion-core-five.md` 方向三 |
| 自愈循环（不可达 ROADMAP 条目） | `expansion-core-five.md` 方向四 |
| 预算前瞻规划（执行前成本估算） | `expansion-core-five.md` 方向五 |
| 多租户安全隔离 / Agent 权限模型 | `expansion-gaps-v7-novel.md` 方向四 |
| 确定性 Replay / 调试引擎 | `expansion-gaps-v7-novel.md` 方向三 |
| 置信度感知决策引擎 | `expansion-directions-v6.md` 方向二 |
| 自愈层运行时 | `expansion-directions-v6.md` 方向四 |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6.md` 方向一 |
| 元学习闭环 / 闸门自省 | `high-value-extensions.md` 方向二 |
| 增量式治理执行 / git-diff 执法 | `high-value-extensions.md` 方向三 |
| 运行时模型质量自适应 | `expansion-gaps-v7-novel.md` 方向二 |
| MQTT / Wasm 集成 | `mqtt-and-wasm-integration.md` |

---

## §1. 方向一：收敛信号证据链——从「是否满足」到「为什么不满足」的诊断基础设施

### 代码级证据

`internal/converge/converge.go` 的 `Evaluate` 函数输出一个纯二值数组——每个 criterion 只有 `Met bool`：

```go
type Result struct {
    Expr   string // human-readable rendering of the criterion
    Met    bool
    Detail string
}
```

当 `Met == false` 时，`Detail` 的内容截然不同：

| Criterion | 不满足时的 Detail 示例 | 诊断价值 |
|---|---|---|
| `roadmap_completion >= 100` | `roadmap_completion=45%` | 中等——知道数字但不知道哪些条目未完成 |
| `gates_status == green` | `a required gate is not green` | **极低**——不知道哪个 gate、为什么红 |
| `test_pass == PASS` | `test_pass=FAIL` | 低——不知道哪个测试、什么错误 |
| `architecture` | `architecture=FAIL` | 低——不知道哪个包、什么违规 |

代码级证据链：

1. **`evalOne` 的 case `gates_status`**（`converge.go:155`）：
   ```go
   case c.Metric == "gates_status":
       met := c.Value == "green" && sig.GatesGreen
       return Result{render(c), met, greenDetail(sig)}
   ```
   `greenDetail` 只渲染了「多少 gate 绿了 + 哪些被豁免」，但没有「为什么这个 gate 是红的」。

2. **`evalCriterion`**（`converge.go:183`）：
   ```go
   func evalCriterion(c asset.Criterion, sig Signals) Result {
       status, ok := sig.Criteria[c.Metric]
       if !ok {
           return Result{render(c), false, fmt.Sprintf("%s: no verdict (probe absent) — treated as unmet", c.Metric)}
       }
       met := status == "PASS"
       return Result{render(c), met, fmt.Sprintf("%s=%s", c.Metric, status)}
   }
   ```
   只暴露了 `PASS`/`FAIL`/`NA`，没有具体原因。

3. **`GateProof`**（`converge.go:63`）已经部分解决了这个问题——`Exemptions` 字段列出了豁免的 gate 和原因。但这是**只针对 gate 的**，且只有「为什么没查」的解释，没有「为什么 FAIL」的解释。

4. **`gatherSignals`**（`main.go` 某处）在 `probeStatuses` 调用中已经拿到了 per-gate 的详细输出（`acceptance.mjs --json` 返回的每行有 `detail` 字段），但这个 **detail 被丢弃了**——`ProbeAll` 返回的 `map[string]string` 只保留了 `status`，丢掉了每行的 `detail`：
   ```go
   // gate.go:126
   for _, row := range rows {
       statuses[row.Criterion] = normStatus(row.Status)
       categories[row.Criterion] = row.Category
       // ← Detail 被丢弃了！
   }
   ```

5. **`acceptance.mjs` 的 `--json` 输出**每行实际上包含了足够的信息：
   ```json
   {"criterion": "test_pass", "status": "FAIL", "detail": "1/15 test(s) FAILED: test/http.test.mjs#shortcode roundtrip fails (expected 201 got 500)", "category": "applicable"}
   ```
   但这个 **detail 字符串在上游 `gate.go` 中被丢弃**，下游 `converge.go` 永远看不到。

6. **`RoadmapCompletion` 的计算**（`converge.go:236`）只拿了百分比：
   ```go
   func RoadmapCompletion(markdown string) float64 {
       done, total := 0, 0
       for _, line := range strings.Split(markdown, "\n") {
           switch t := strings.TrimSpace(line); {
           case strings.HasPrefix(t, "- [x]"), strings.HasPrefix(t, "- [X]"):
               done++; total++
           case strings.HasPrefix(t, "- [ ]"), strings.HasPrefix(t, "- [~]"):
               total++
           }
       }
       return float64(done) / float64(total)
   }
   ```
   输出 `float64`——去掉了**哪些条目**已经完成、**哪些条目**还待办。当收敛失败时，operator 只看到 `roadmap_completion=45%` 但不知道剩下哪 55% 没完成。

### 高价值扩展

**收敛信号证据链（Convergence Evidence Chain）**——在 `converge` 包中增加一个透明的「不满足原因」层，让每次收敛检查不仅输出 `Met bool`，还输出一个**结构化的、可机器消费的不满足原因树**。

新增 data types：

```go
// internal/converge/evidence.go

// Evidence 是单个 criterion 不满足时的结构化原因
type Evidence struct {
    Criterion   string   // 哪个 criterion
    Met         bool     
    Reason      string   // 简要人类可读原因
    // Detail 是层次化的具体证据，可展开
    Details     []EvidenceDetail
    // SubEvidence 支持 AND/OR 组合的嵌套证据树
    SubEvidence []Evidence
}

type EvidenceDetail struct {
    Key     string // "gate_name" / "test_name" / "roadmap_item"
    Value   string // "test_pass" / "shortcode roundtrip" / "implement pubsub retry"
    Status  string // "PASS" / "FAIL" / "NA" / "DONE" / "PENDING"
    Detail  string // 人类可读详因，如 "expected 201 got 500"
}
```

**为什么高价值**：

1. **当前的生产问题**：在一次 24h 的无人值守运行中，如果最终的 `forge evolve` 报告「NOT MET — test_pass=FAIL」，operator 没有任何信息知道**是哪个测试失败了、在哪次 iteration 引入的、是 flaky 还是真实回归**。没有 `git bisect` 的信息，operator 只能 rerun 整个 24h run。

2. **当前的架构缺口**：`gate.ProbeAll` 已经拿到了 detail 但丢掉了它——这是一个**数据在管道中丢失**的模式。`acceptance.mjs --json` 的每个 row 都包含 detail 字段，但 `gate.go:126` 只保留了 status + category。

3. **Agent 循环中的价值**：当 `forge evolve` 中的 LoopEngine 检测到收敛失败时，它输出一行日志然后开始下一次迭代。如果有 Evidence 树，下一次迭代的 **planner prompt 可以直接注入「为什么上次迭代失败」的结构化证据**（不是人类写的描述，而是机器验证的事实），让 planner 在下一次 sprintsplit 中针对性地解决。

4. **跨 iteration 模式匹配**：有了 Evidence 链，可以在迭代历史中检测模式：「test_pass 在 iteration 3-5 连续 FAIL，在 iteration 6 突然 PASS」——这可能是 flaky test。当前没有数据来判断。

### 边界情况

- **Evidence 大小爆炸**：一个有 1000 个测试的项目可能产生巨大的 Evidence 树。需要分页/摘要策略——默认只输出前 5 条失败细节，完整的通过 `--verbose-evidence` 获取。
- **嵌套循环证据**：如果 `gates_status` 因为 `test_pass` FAIL 而 red，而 `test_pass` 又因为 `shortcode test` FAIL——不要在三层 Evidence 树中重复同一信息。需要引用链（`test_pass` 的 SubEvidence 引用 `shortcode test`，而不是复制）。
- **安全敏感**：detail 可能泄露测试输出中的敏感数据（密码、token）。EvidenceDetail 应该有一个 `redacted` 标记或最大长度截断。
- **向后兼容**：现有的 `Result.Met` 和 `Result.Detail` 继续存在。Evidence 是**附加的**（新字段 `Result.Evidence`），不改变任何现有的使用模式。

### 与已有分析的区别

`edgecases-and-perf.md` §5.1 讨论了「测试计数下降不被发现」——这是**跨 iteration 的计数趋势**，不是**单次 FAIL 的根因**。`expansion-core-five-2026-07-01.md` 方向一（跨周期收敛状态机）关注的是**趋势分类**（progressing/plateauing/stuck），不是**单次不满足的原因解剖**。本文方向一关注的是收敛失败的**根因可追溯性**——一个正交维度。

---

## §2. 方向二：文件级分岔回滚——从 checkpoint 回退到真写入文件回滚

### 代码级证据

`internal/persist/checkpoint.go` 的 `Save` 已经支持历史版本保留（`rotateRetain`）：

```go
// checkpoint.go （推断，基于 rotateRetain 的参数存在性）
func Save(path string, cp Checkpoint, retain int) error {
    // 写 path -> path.1 -> path.2 -> … -> path.N
}
```

但 **整个代码库没有 `LoadVersion(path, N)` 或 `RollbackTo(path, N)` 函数**。历史版本存在但永远不被读取。

更有意思的是 checkpoint 中保存的数据：

```go
// 从 forge status 的输出看，checkpoint 包含：
type Checkpoint struct {
    PhaseIndex      int     // 当前完成到的 phase
    RoadmapComplete float64 // roadmap 完成度
    Iteration       int
    // ... 但没有 GIT_TREE_HASH
}
```

**关键缺口**：checkpoint 不记录造成当前状态的 git commit hash。这意味着即使实现了 `LoadVersion` 和文件级回滚，也**不知道应该 `git checkout` 到哪个 commit**。

代码级证据链：

1. **`internal/persist/checkpoint.go`** 写 `.forge/checkpoint.json` 和 `.forge/checkpoint.json.1` ~ `.N`，但没有任何读取历史版本的公共接口。

2. **`LoopEngine` 的 `staleOutcome`** 在触发 no-progress tripwire 时只 abort：
   ```go
   func (l LoopEngine) staleOutcome(i int) LoopOutcome {
       return LoopOutcome{i, false, "no-progress tripwire (anti doom-loop)"}
   }
   ```
   没有任何「建议回滚到上一个好的 checkpoint 并尝试不同路线」的逻辑。

3. **`forge status`**（在 `validate.go` 中）只显示当前 checkpoint，不显示历史：
   ```
   forge status:
     Iteration:  7/20
     Phase:      5/6
     Roadmap:    80%
     Gates:      green
     // ← 没有 "last-green-iteration: 5" 等回溯信息
   ```

4. **`cmdForget`**（也在 `validate.go` 中）只能「忘记所有 memory」——不能忘记当前的 checkpoint 并回退到上一个版本。

5. **`internal/memory/memory.go` 的 `Supersedes`** 字段是 Fork/Rollback 的微基建——一个 Entry 可以标记自己「替代了」之前的 Entry。但这个字段是 topic-level 的，没有 branch/checkpoint 版本概念。

6. **`forge evolve --resume`** 只能从 iteration 边界 resume，不支持从「上一个全绿的 iteration」resume：
   ```
   forge evolve build --resume           # 从最后的 checkpoint resume
   # 但不支持：
   forge evolve build --rollback=2       # 回退到 iteration 2 的 checkpoint 并从那里 resume
   forge evolve build --fork-at=3        # 从 iteration 3 fork 一个新分支
   ```

7. **真实世界的数据**：真 claude 跑 multi-agent 验证了「agent 写的代码可能把项目搞坏到需要从头重来的程度」。Sprint 25 的报告中明确写了「方向错了一次 N 轮迭代全部浪费」。

### 高价值扩展

**文件级分岔/回滚引擎（File-Level Fork & Rollback Engine）**——不只是 checkpoint 回读，而是**连 git 操作一起**，让 checkpoint 保存 `GitHash` 字段，使回滚不仅是状态回退，也是**代码回退**。

```go
// internal/persist/checkpoint.go 新增
type Checkpoint struct {
    Iteration       int     `json:"iteration"`
    PhaseIndex      int     `json:"phase_index"`
    RoadmapComplete float64 `json:"roadmap_complete"`
    GatesGreen      bool    `json:"gates_green"`
    // 新增：
    GitHash         string  `json:"git_hash,omitempty"` // git commit or stash ref
    // 分支/分岔支持：
    ForkParent      string  `json:"fork_parent,omitempty"` // parent checkpoint path
    ForkLabel       string  `json:"fork_label,omitempty"`  // human-readable label
}

// 新增接口
func LoadVersion(path string, version int) (Checkpoint, error)    // 读取 checkpoint.json.N
func RollbackTo(path string, version int, repoRoot string) error  // 回退 checkpoint + git checkout
func Fork(path string, fromVersion int, label string) error       // 从历史检查点创建新分支
```

CLI 扩展：

| 命令 | 行为 |
|---|---|
| `forge checkpoint list [--all]` | 列出所有历史 checkpoint（版本号 + 时间 + roadmap% + gate 状态） |
| `forge checkpoint show <version>` | 查看特定版本 checkpoint 的详细内容 |
| `forge rollback <version>` | 回退到版本 N 的 checkpoint + `git checkout` 对应 hash |
| `forge fork <version> --label "try-different-arch"` | 从版本 N 创建分支 checkpoint（不修改当前状态） |
| `forge evolve --rollback-on-stale` | 在 stale tripwire 时不 abort，自动 rollback 到上一全绿 checkpoint |

**为什么高价值**：

1. **当前 checkpoint 历史是「写了但没消费」的完美案例**——`rotateRetain` 已经保留了历史版本，`persist.Save` 已经实现了轮换，但没有任何命令/逻辑读取它们。**工程上这是已支付成本但零收益。**

2. **真 agent 场景的必然需求**：Sprint 25 显示，claude implementer 可能在 iteration 5 写了一段代码把项目的测试架构破坏了，iteration 6-8 都在修它带来的问题。如果 iteration 5 结束时有一个 checkpoint，operator 可以 rollback 到 iteration 4 的 checkpoint + 对应的 git 状态，然后 `forge evolve --fork-at=4 --mode engineering` 用更严格的 gate 绕过那个损坏的 implementer 决策——而不是手动 `git reset --hard` + 重新配置。

3. **与 `parallel` 模式的互补**：parallel 是**同一 iteration 内**的 phase 级并行。Fork 是**跨 iteration** 的路径级并行。两者结合可以真正并行探索多个架构方向。

4. **安全约束**：rollback 必须 git-aware——只回退 checkpoint 而不回退文件是没有意义的（agent 已写的代码还在）。因此 `GitHash` 字段是关键依赖。

### 边界情况

- **未提交的更改**：如果 git workspace 有未提交的更改，rollback 前需要 `git stash` push，rollback 后 pop。否则会丢失 operator 的未保存工作。
- **并发 fork**：两个进程同时 fork（`forge evolve` 和 operator 手动 `forge fork`）可能冲突。需要 fork 目录的 `O_EXCL` 原子创建。
- **Fork 后的 converge**：fork 分支收敛后怎么合并 memory？两个分支可能对同一 topic 有冲突的 `Supersedes`——需要 `forge merge` 命令来手动解决 memory 冲突。
- **GitHash 为空**：第一次 checkpoint（还没有任何 agent 写过文件）的 GitHash 为空——rollback 时只回退 checkpoint，不做 `git checkout`。

### 与已有分析的区别

`expansion-core-five-2026-07-01.md` 方向四（分岔/回滚引擎）提出了 fork/rollback 的概念，但**重点在 checkpoint 读取和 LoopEngine 的 fork 决策**，没有讨论**文件级回滚的 git 集成**。本文的方向二聚焦在 `GitHash` 落地和 `checkpoint list/show/rollback` CLI 命令上——这是使分岔概念可操作化的关键基础设施。没有文件级回滚，分岔只是「状态回读」，不是「真实回退」。

---

## §3. 方向三：工作流片段组合与阶段复用——消除 workflow YAML 的大量重复

### 代码级证据

当前 `.agent/workflows/` 下的 5 个 workflow YAML 文件存在大量模式重复：

**证据 1：build.yml 和 evolve.yml 共享同一组 gate 阶段**

```yaml
# build.yml（简化）
phases:
  - name: planner
    agent: planner
    feeds_forward: true
  - name: implementer
    agent: implementer
  - name: harness-gates
    required_gates: [lint, test, build, complexity, arch, security]
    on_fail: { action: loop_back, target_phase: implementer }
  - name: reviewer
    agent: reviewer
    required_when: "#reviewer"
  - name: qa
    agent: qa
    required_gates: [lint, test, build]
```

```yaml
# evolve.yml（简化）
loop:
  phases:
    - name: scanner
      agent: scanner
    - name: planner
      agent: planner
    - name: implementer
      agent: implementer
    - name: harness-gates
      required_gates: [lint, test, build, complexity, arch, security]
      on_fail: { action: loop_back, target_phase: implementer }
    - name: reviewer
      agent: reviewer
    - name: evaluator
      agent: evaluator
```

**harness-gates 阶段的 `required_gates` 和 `on_fail` 完全重复**。如果有一天需要加一个新的 gate（比如 `benchmark`），必须手动同步修改 2-5 个文件。

**证据 2：discover.yml 的三个 phase 共享同一模式**

```yaml
phases:
  - name: requirement-discovery
    agent: product-manager
    model_tier: opus
    confidence_metric: requirement_confidence
  - name: market-research
    agent: researcher
    model_tier: opus
    optional_for: [balanced]
  - name: product-capability
    agent: explorer
    model_tier: opus
    optional_for: [balanced]
```

每个 phase 都设 `model_tier: opus`——如果在三个文件里多写了一个 workflow 调用这三个 role，就要重复三遍。

**证据 3：`asset.Phase` 结构体本身已经承载着片段化的可能——但没有片段化的语法**

```go
type Phase struct {
    Name             string     // 唯一标识
    Agent            string     // 角色
    RequiredGates    []string   // 门列表——高度重复
    OnFail           *OnFail    // 门失败策略——高度重复
    ModelTier        string     // 模型档——重复的常见模式
    FeedsForward     bool       // 前馈标记
    DependsOn        []string   // 依赖——重复
    // ...
}
```

但 YAML 语法中没有「引用/复用」机制。不能写：

```yaml
# ❌ 当前不支持
phase_templates:
  harness-gates:
    required_gates: [lint, test, build, complexity, arch, security]
    on_fail: { action: loop_back, target_phase: implementer }

phases:
  - name: build-harness
    use_template: harness-gates    # 引用模板
```

**证据 4：`asset.LoadWorkflowJSON` 的 fault-tolerant 设计**

因为 asset 是 fault-tolerant 的，它可以接受一个有 `use_template` 或 `extends` 字段的阶段而不崩——当前 undefined 字段被静默忽略。这意味着**模板语法可以在 asset 层 backward-compatible 地引入**，老的 workflow 保持原样。

**证据 5：`yaml2json.go` 的 YAML→Go 管道**

shim 的输出是 `any`（`map[string]any` 或 `[]any`），当前 `asset.LoadWorkflowJSON` 用 `encoding/json.Unmarshal` 消费。这意味着模板解析可以在**两条路径之一**实现：
- 在 YAML 层（shim 或 yaml2json.Decode 之后、Marshal 之前）——摘掉 `use_template` 的 key，把 template 内容 inline 进去
- 在 asset 层（Marshal 之后、Unmarshal 之前）——修改中间 JSON

### 高价值扩展

**工作流片段组合系统（Workflow Fragment Composition System）**——借鉴 Kubernetes 的 `kustomize` 模式而不是 Helm 的模板替换：

```
.agent/
├── workflows/
│   ├── build.yml             ← 组合 + 局部覆写
│   ├── evolve.yml            ← 组合 + 局部覆写
│   ├── discover.yml          ← 独立（无复用）
│   ├── design.yml            ← 独立（无复用）
│   └── review.yml            ← 独立（无复用）
└── fragments/                ← 新增：可复用片段
    ├── harness-gate.yml      ← {required_gates, on_fail}
    ├── planner-ff.yml        ← {agent: planner, feeds_forward: true}
    ├── reviewer.yml          ← {agent: reviewer, required_when: "#reviewer"}
    ├── qa-gates.yml          ← {required_gates: [lint, test, build]}
    └── model-tier-opus.yml   ← {model_tier: opus}
```

语法扩展（在 `asset.Phase` 中新增 `UseFragments []string`）：

```yaml
# build.yml 使用 fragments 的版本
phases:
  - name: planner
    fragments: [planner-ff, model-tier-opus]   # 展开自 fragments/planner-ff.yml + fragments/model-tier-opus.yml
  - name: implementer
    agent: implementer
    model_tier: sonnet
    fragments: [model-tier-sonnet]
  - name: harness-gates
    fragments: [harness-gate]
  - name: reviewer
    fragments: [reviewer]
  - name: qa
    fragments: [qa-gates]
```

展开规则（纯数据操作，无模板引擎）：

```
1. 取 fragments 列表中的每个 name
2. 读 .agent/fragments/<name>.yml
3. JSON merge（浅层：fragments 中的字段作为 defaults，上层 YAML 的字段覆盖它）
4. 将合并后的字段作为 phase 的最终值
```

**为什么高价值**：

1. **当前的手工同步成本**：`required_gates: [lint, test, build, complexity, arch, security]` 在 5 个 workflow 文件中出现了至少 5 次。每次加 gate、改 gate、调 `on_fail` 策略都要全改——无约束的注解重复。

2. **与 YAML shim 的兼容**：`yaml2json.go` 已经在 `Decode` 后、`Marshal` 前做了一层处理。fragment 展开可以在这层做——完全不改变 `asset.LoadWorkflowJSON` 的接口。

3. **门槛极低**：不需要模板引擎、不需要 `{{ }}` 语法、不引入外部依赖。只是**声明式 merge**——与 `kustomize` 的资源叠加一致的哲学。

4. **未来扩展路径**：一旦 fragment 系统建立，可以向 `forge detect` 推荐 fragment 组合（检测到 Go 项目 → 自动应用 `go-harness-gates` fragment 代替手动列 gate 列表）。

### 边界情况

- **fragment 循环引用**：禁止 `fragment A 引用 fragment B 引用 fragment A`——在 `yaml2json` 层做一次 DAG 检查。
- **fragment 展开与 `fragments` 字段的 JSON 表示**：`asset.Phase` 需要增加 `Fragments []string` 字段，但它是 transient 的（expand 后丢弃）。在 `LoadWorkflowJSON` 中不展开是因为 JSON step 没有文件系统访问能力。展开应在 `loadWorkflow`（cmd/forge）级别，在 `yaml2json.Decode` 和 `json.Marshal + asset.LoadWorkflowJSON` 之间做。
- **多层覆盖**：fragment → phase-level override → mode gating 三层覆盖。每层只能缩小/覆盖，不能组合/新增 arrays——`required_gates` 的 fragment 值是 `[lint, test]`，phase 级覆盖写 `required_gates: [lint, test, build]` 会替换而不是追加。这是简化，不是限制。
- **fragment 不存在**：展开时 fragment 文件缺失 → load 失败并给出明确错误信息（如 `fragment "planner-ff" not found in .agent/fragments/`）——fail-fast，不静默跳过。

### 与已有分析的区别

所有已有分析都没有讨论 workflow YAML 级别的组件复用。`growth-bottlenecks-and-scalability.md` 讨论了 cmd/forge 的文件膨胀，但没有涉及 `.agent/workflows/` 的 YAML 重复。`configuration-surface-and-adoption.md` 关注配置项的跨文件一致性，但完全从运行时配置值角度出发（gate 阈值、mode 开关），不是从 workflow 结构的可组合性角度。

---

## §4. 方向四：secret-scan 的负载硬化——从 N/A 可选项到 LOAD_BEARING 闸门

### 代码级证据

**证据 1：`secret-scan.mjs` 的存在但非负载状态**

```javascript
// harness/secret-scan.mjs
// 已有的文件，但 acceptance.mjs 中它的状态完全是环境依赖的
```

在 Sprint 26 的 `CURRENT_SPRINT.md` 中明确声明：
```
security_findings N/A（非 LOAD_BEARING）
```

**证据 2：`harness/acceptance.mjs` 中 secret-scan 的分类**

在 `test_acceptance.mjs` 的 fixture 中，`dependency_vulnerabilities` 和 `security_findings` 被分类为 `no_tool`，因为需要外部数据库/工具。secret-scan 虽然有自己的 `secret-scan.mjs` 实现，但因为没有预配置的规则集，实际执行时返回 N/A。

**证据 3：`.agent/AGENTS.md` 硬闸门中的声明**

```markdown
## 硬闸门 — harness 自动拦截
- **安全与质量**:无硬编码 secret · test / app-test 全绿 · lint / typecheck / build / coverage
```

`.agent/AGENTS.md` **宣称** secret-scan 是硬闸门之一，但实际上一份没有配置任何 secret 规则集的 repo 也会 PASS——因为 secret-scan 返回 N/A，而 N/A 不阻断。**声明和实际之间存在 gap**。

**证据 4：`harness/policies.yml` 的 gate_catalog 只列出了 6 个 gate**

```yaml
gate_catalog:
  lint:       静态风格/语法
  test:       单元+集成测试
  build:      可编译可打包
  complexity: 体积/圈复杂度
  arch:       依赖方向/循环依赖/分层
  security:   依赖扫描/SAST/secret 扫描
```

security 是整体概念，secret-scan 是它的一部分但**没有独立入口**。如果用户想只启用 secret-scan 而不启用完整的 SAST/依赖扫描，没有明确的路径。

**证据 5：真 agent 场景中 secret 泄露的真实风险**

Sprint 25-26 的真 claude 端到端测试中，agent 被授予了文件写入权限（`acceptEdits`）来写代码。如果 agent 无意中写了一个包含 API key 的测试文件或硬编码了 internal URL，当前没有任何机制可以检测和阻断。

### 高价值扩展

**secret-scan 负载硬化（Secret-Scan Load-Bearing Hardening）**——从「有则跑、无则N/A」升级为「默认阻断基线」：

```
阶段一：模式硬化（当前基线 → 最小可用）
  - secret-scan.mjs 增加内置默认规则集（不依赖外部配置即可运行）
    · AWS/GCP/Azure key 格式检测（默认开启）
    · GitHub token/SSH key 格式检测
    · Generic `-----BEGIN.*PRIVATE KEY-----` 检测
    · `password\s*=\s*['"].+['"]`、`api_key\s*=\s*['"].+['"]` 等模式
  - secret-scan.mjs 在没有自定义规则文件时使用内置规则集
  - 当内置规则都不匹配时返回 N/A（而不是假装检查了）

阶段二：与 acceptance Load-BEARING 集成
  - secret-scan 成为独立的 acceptance criterion（不再是 security 的模糊子集）
  - 在 engineering + production mode 下强制为 LOAD_BEARING
  - 阻断级别可配置：block / warn / off

阶段三：git-diff 增量扫描（性能优化）
  - 全仓扫描大项目时可能慢（扫描 10k 文件的 git 历史）
  - 增量模式：只扫描 git diff 中变更的文件（与 direction 三的增量治理一致）
  - 全量扫描：`forge scan --full`（CI 中的初始配置）

阶段四：git pre-commit hook 集成
  - `forge init --hooks` 安装 git pre-commit hook 调用 secret-scan
  - 在 commit 之前拦截 secret 泄露，不等 CI
```

**为什么高价值**：

1. **AGENTS.md 声称 secret-scan 是硬闸门，但实际 N/A 使它不是**——这是一个**治理可信度问题**。如果 AGENTS.md 列出的 6 道硬闸门中有 1 道是事实上可选的，那么整个闸门系统的公信力受损。

2. **真 agent 写代码提高了泄露风险**：Sprint 24 解除了 `acceptEdits` 权限。agent 写的代码不会被人类 review（这是 automous 的卖点），所以 secret 泄露的检测必须**完全自动化**——不能依赖人。

3. **token/secret 的格式是跨语言、跨框架的**——不需要理解 AST，只需要正则匹配。这使得 secret-scan 是所有 gate 中**最容易做到零误报/低漏报**的。

4. **当前的安全合规需求**：RSA key 硬编码、AWS key 泄露是最常见的 OSS 安全事故。ForgeOS 作为治理层，不能声称「生产就绪」的同时默认不扫描 secret。

### 边界情况

- **误报处理**：测试代码中可能包含 `-----BEGIN CERTIFICATE-----` 作为测试数据。需要 `.secretskip` 文件或内联注释 `// forge:secret-scan-ignore` 来跳过已知的误报。
- **大文件性能**：一个 10MB 的 `.env.example` 文件被完整扫描可能耗时数秒。需要文件大小上限（`--max-secret-scan-bytes`，默认 1MB），超过的文件跳过并记录 WARNING。
- **Base64 编码的 secret**：`echo $AWS_KEY | base64` 编码后的 secret 不会被正则匹配到——这是设计上的诚实约束（声明：只扫明文、常见格式的 secret，不承诺检测编码/加密后的 secret）。
- **git 历史扫描的风险**：扫描 `git log -p` 会暴露历史中的所有 secret——可能触发大量发现。需要 `--scan-history` 显式 opt-in，默认只扫描 working tree 的内容。
- **内置规则与自定义规则的优先级**：内置规则是安全基线，不可关闭。自定义规则只能增加、不能减少。确保用户不能意外地 `disable_by_default: true` 来关掉 AWS key 检测。

### 与已有分析的区别

`expansion-gaps-v7-novel.md` 方向四（多租户安全隔离 / Agent 权限模型）讨论的是**Agent 对系统资源的访问控制**，不是代码产物中的 secret 泄露。本文方向四关注的是**产物（代码）的静态安全扫描**——一个完全正交的维度。`edgecases-and-perf.md` §5（治理盲区）讨论了测试退化、代码/测试比例等质量信号，但没有讨论安全扫描。

---

## §5. 方向五：`forge compose`——基于 git 工作流的上下文感知多阶段 orchestration

### 代码级证据

**证据 1：`detect.go` 已经可以自动感知项目的大部分上下文**

```go
// cmd/forge/detect.go
type projectProfile struct {
    Language   string   // 检测到的语言（go / node / python / rust / …）
    HasTests   bool
    HasCI      bool
    HasDocker  bool
    Framework  string   // 检测到的框架（express / gin / fastapi / …）
    Lifecycle  string   // 当前 lifecycle
    // …
}
func detectProject(root string) (projectProfile, error) { … }
```

`detect.go` 已经可以检测语言、测试框架、CI 配置、Dockerfile 存在性等。但它只输出到 stdout，**没有任何下游消费者**。`forge migrate` 可以接受 detect 的输出来自动调整 mode，但 detect 本身不是 migrate 的前提条件。

**证据 2：git 工作流状态完全未被消费**

当前 forge-core 读 git 的唯一地方是：
- `risk.FromChangedPaths`（`risk_diff.go`）— 读 git diff 路径列表
- `detect.go` 的 `gitHasChanges` — 检测是否有未 commit 的改动

但 git 的**丰富语义**没有被消费：
- `git diff --cached`：staging area 的状态（即将 commit 的内容）
- `git log --oneline -5`：最近的提交历史（知道项目最近在干什么）
- `git branch --show-current`：当前分支名（feature 还是 main）
- `git status --porcelain`：精准的工作树状态（新建/修改/删除/未跟踪）
- `git rev-list --count main..HEAD`：当前分支领先 main 的 commit 数

**证据 3：`forge run` 的 `--mode` 和 `--lifecycle` 是手动输入的**

每次运行 `forge evolve build --mode engineering --lifecycle mvp`，operator 必须手动输入 mode 和 lifecycle。但同时 `detect.go` 已经可以从项目结构推断 lifecycle（有 production Dockerfile → lifecycle 至少是 growth，还没有测试 → lifecycle 是 idea）。

**证据 4：`converge.Signals` 中 git 信息的缺失**

`gatherSignals`（某个 main.go 函数）收集：
- `RoadmapCompletion` — 读 `.agent/ROADMAP.md`
- `GatesGreen` — 跑 `ProbeAll`
- `FileDelta` — 跑 `git diff --name-only`
- `CodeTestRatio` — 跑 `git diff --stat`

但没收集的 git 信号：
- 当前分支与 main 的偏离程度（`git rev-list --count main..HEAD`）
- 是否有未合并的 PR（通过 git 本地信息无法获取，需要 GitHub CLI / API）
- 最后 commit 的类型（`fix:` / `feat:` / `docs:` — 正则匹配 commit message）

**证据 5：`forge status` 的输出完全基于 `.forge/` 文件系统状态**

没有「这个 run 是基于 git commit abc123 运行的」的记录。当 operator 看到 `forge status` 的输出时，不知道当前的 checkpoint 对应的是代码的哪个状态。

### 高价值扩展

**`forge compose`——基于 git 工作流上下文的智能多阶段编排**。

核心概念：`forge compose` 是一个「编排的编排器」，它：
1. 读取 git 状态（分支名、改动范围、commit 类型）
2. 读取 `detect` 输出的项目 profile
3. 组合这些信号自动选择合适的 workflow 序列 + mode/lifecycle
4. 以**单一 CLI 命令**替代当前的手动 3-5 步编排

```bash
# 当前需要做：
git checkout -b feat/add-redis-cache
# 手动编辑代码...
forge evolve discover --mode explorer
forge evolve design --mode engineering
forge run design --approved
forge evolve build --mode engineering
forge evolve review --mode cto

# 使用 forge compose 后：
git checkout -b feat/add-redis-cache
# 手动编辑代码...
forge compose            # 读取 git 状态 + detect 输出 → 自动选择合适的 workflow 序列和 mode
```

`forge compose` 的决策逻辑：

| 输入信号 | 推导 |
|---|---|
| `detect().Lifecycle == "idea" && branch != "main"` | 新功能分支 → explorer mode + discover→build 管道 |
| `git log --oneline -1 | grep "^fix:"` | Bugfix → balanced mode + build 工作流（跳过 discover） |
| `detect().HasDocker && detect().HasCI` | 生产准备 → engineering mode + 全脊柱 |
| `git diff --name-only | grep "\.proto$"` | API 变更 → 强制启用 security gate |
| `git rev-list --count main..HEAD > 10` | 长运行分支 → 先建议 rebase/merge main 再 evolve |

新增架构：

```
internal/compose/（新包）
├── compose.go          forge compose 入口
├── profile.go          项目画像（包装 detect + git 状态）
├── strategy.go         策略引擎（输入 → 推荐 workfow + mode + lifecycle）
├── workflow.go         工作流序列编排（按序触发 multiple forge evolve）
└── decision.go         决策日志（记录 compose 为什么选择了这个策略）

CLI:
  forge compose              # 交互模式：显示推荐策略并等待确认
  forge compose --apply      # 非交互模式：直接执行推荐策略
  forge compose --dry        # 只显示推荐策略，不执行
  forge compose explain      # 解释为什么做出了这个决策
```

**为什么高价值**：

1. **当前编排的摩擦太高**：从「我要改一个 bug」到 `forge evolve` 开始跑，中间要 3-5 步手动决策（走哪个 mode、哪个 lifecycle、跑哪个 workflow 序列）。`forge compose` 把这些决策自动化了，让 operator 的意图（「改个 bug」）直接映射到正确的编排策略。

2. **`detect.go` 已经做了 80% 的工作但输出被浪费**——`detect` 的输出只用于 stdout 展示，没有任何编排消费它。`forge compose` 是 detect 的第一个真正的消费者。

3. **与 git flow 的深度绑定**：git 工作流已经是开发者的自然意图表达工具（feature branch = 新功能、fix branch = 修复、main = 发布）。ForgeOS 作为治理层，读 git 的状态是天然而廉价的信号源。

4. **一致性**：当前不同的 operator 会做不同的编排决策（有人对 bugfix 跑 explorer mode，有人跑 engineering mode）。`forge compose` 把最佳实践编码为可执行的策略，消除了人工差异。

5. **AI 原生优势**：`forge compose` 的决策 explain 输出可以用自然语言解释为什么选择了一个特定的策略：「检测到当前是 fix 分支，所以跳过 discover 阶段直接进入 build。项目有 production Dockerfile，所以使用 engineering mode。」——这让新用户不用学习 mode/lifecycle/workflow 的概念就能有效使用 ForgeOS。

### 边界情况

- **策略冲突**：`detect()` 说 lifecycle=idea（新项目），但 `git rev-list --count main..HEAD > 10` 说有 10 个 commit 等待合并——两个信号冲突。策略引擎需要明确的优先级规则（分支长度 > lifecycle 检测）。
- **新项目没有 git 历史**：`git log` 和 `git rev-list` 失败——`forge compose` 降级为只使用 `detect` 的输出。
- **draft PR / WIP commit**：如果 commit message 包含 `WIP:` 或 `draft:`，compose 应该使用更宽松的 explorer mode。
- **`--apply` 的风险**：自动执行推荐的 workflow 序列意味着 compose 可能调用多个 `forge evolve` 命令，每个可能耗时数小时。需要 `--apply` 的安全确认（`forge compose --apply --approved` 或交互式确认）。
- **可解释性**：`forge compose explain` 的输出必须包含**每个决策的理由**（`mode=engineering 因为 lifecycle=growth`），这样 operator 可以理解并信任推荐，或发现 `detect` 的判断错误后手动覆盖。

### 与已有分析的区别

`expansion-core-five-2026-07-01.md` 方向五（跨工作流管道编排）讨论了 `forge pipeline`——一个声明式的、YAML 驱动的管道定义。本文的方向五 `forge compose` 是**互补但不同**的：pipeline 是声明管道定义、手动触发；compose 是 git-context-aware 的智能推荐。Pipeline 告诉你「按顺序跑这些 workflow」；compose 告诉你「根据你的 git 状态，你应该怎么跑」。compose 可以是 pipeline 的前置智能层——`forge compose --apply` 可以生成一个 `.agent/pipeline.yml` 然后用 `forge pipeline run` 执行。

此外，所有已有分析都没有**从 git 工作流角度切入编排决策**。`detect.go` 的输出在当前架构中是孤立的——本文方向五第一次提出将其作为编排输入。

---

## §6. 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 | 风险等级 |
|---|---|---|---|---|
| **一 收敛信号证据链** | P1 | 功能+运维 | 从「是否满足」到「为什么不满足」——24h 无人值守的可调试性前提 | 低（纯新增：`Evidence` 结构体 + 数据保全 + CLI display） |
| **二 文件级分岔回滚** | P1 | 功能+基础设施 | `checkpoint.json.1~.N` 已存在但零消费——给已有的写入加读路径和 git 感知 | 中（git 操作有风险，需 `git stash`+`git checkout` 安全序列） |
| **三 工作流片段组合** | P2 | 架构债务 | 消除 5 个 workflow YAML 中 `required_gates`/`on_fail` 的重复——治理者先治己 | 低（纯声明式：yaml2json 层 inline fragment，无运行时变更） |
| **四 secret-scan 负载硬化** | P0 | 安全+治理 | AGENTS.md 宣称的硬闸门但实为 N/A——治理可信度修复 | 低（内置默认规则集 + acceptance 集成） |
| **五 forge compose** | P2 | 功能+体验 | `detect.go` 输出浪费了 80%——git 上下文驱动的一键编排 | 中（策略引擎需充分的边界情况覆盖） |

**收敛建议（若资源有限）**：

1. **先做方向四（secret-scan 负载硬化）**——这修复了「AGENTS.md 的声明 vs 实现的 gap」，是治理层自身的可信度修复。而且**工程量最小**：内置规则集是正则匹配 + 集成到 acceptance 的 criterion 列表。不需要架构变更。

2. **接着做方向一（收敛信号证据链）**——现有的 `acceptance.mjs --json` 已经输出 detail，只是 `gate.go` 丢掉了它。工作量：修改 `gate.ProbeAll` 的返回值类型（增加 `details` map），修改 `converge.Evidence` 结构体，修改 CLI 渲染。**不改变任何现有逻辑路径**。

3. **然后做方向二（文件级分岔回滚）**——`checkpoint` 历史已经存在，加读路径+`GitHash`+CLI 命令是对已有投入的收益兑现。

4. **方向三（片段组合）和方向五（compose）可并行**——两者都是 YAML 级/策略级的变更，不触及运行时核心。方向三减少 YAML 维护成本，方向五提升整体 UX。

---

> **本文所有方向均从代码级具体证据出发，与 27+ 份已有分析交叉确认新颖性。**
> 每个方向至少有一个「已存在但未消费的基础设施」或「声明与实现之间的 gap」。
> 不写代码——只做判断。每方向标注了架构位置、利用的现有基础设施、以及需要避免的边界情况。
