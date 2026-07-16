# ForgeOS — 全局深扫后的五个运行时边界盲区

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓逐文件深扫 — forge-core（18+ Go 包 · ~35k 生产代码）、cmd/forge（17+ 子命令）、  
>    harness（39+ 模块 · ~10.5k 行执法层）、.agent/（12 agent 卡 · 9 skill 卡 · 5 工作流）、  
>    examples/（url-shortener · go-taskd）、pi-batch.py  
> 2. 完整阅读 Sprint 1–31 演进记录 + FUNCTIONAL_REQUIREMENTS_AUDIT（90+ DONE · 0 GAP）  
> 3. **交叉验证**: 对每个方向的关键词组合，在 80+ 份已有分析文档（docs/requirements/ +  
>    docs/analysis/ + 核心架构文档 + ADR + CURRENT_SPRINT + ROADMAP）中逐篇检索，  
>    确认该方向的**核心命题从未作为独立方向被展开**  
> 4. **不编写任何代码**。每个方向附代码级证据、与已有覆盖的差异化证明  
> **日期**: 2026-07-10

---

## 全景定位

已有 ~90 份分析文档覆盖了 ForgeOS 几乎所有功能域：引擎补齐（~35 方向）、执行语义形式化（~15）、
生产可靠性（~18）、二阶系统问题（~15）、多仓库/联邦/跨会话治理（~12）、产品视角（~10）、
安全纵深（~10）、北极星桥梁（~8）、阶段间契约（~6）、结构缺口（~10）、以及其他散点（~20）。

**本文 5 个方向的共同特征**: 不是「缺少的引擎」「性能优化」或「架构新层」——而是**当前设计中已存在机制、
但机制之间有未覆盖的边界状态、且这些边界只有在真实压力下才会暴露的缺口**。它们不会被单测或 echo/dry-run
测试覆盖，只在真 LLM 长跑、多进程并发、跨会话积累、或环境故障时显现。

| # | 方向 | 类型 | 优先级 | 已有分析覆盖 |
|---|---|---|---|---|
| 1 | 厂商无关的每调用美元成本护栏 | 成本治理 · 安全 | **P1** | 0 篇独立覆盖 |
| 2 | 声明式输出契约的自动化验证 | 治理 · 契约完整性 | **P2** | 0 篇独立覆盖 |
| 3 | 三存储跨会话一致性审计 | 可靠性 · 数据完整性 | **P1** | 0 篇独立覆盖 |
| 4 | 预算感知的相位执行梯度决策 | 成本优化 · 运行时 | **P2** | 0 篇独立覆盖 |
| 5 | 并发相位资源争用检测与缓解 | 可靠性 · 并行安全 | **P2** | 0 篇独立覆盖 |

---

## 方向一 · 厂商无关的每调用美元成本护栏

**优先级**: 🟠 P1 | **类别**: 成本治理 · 安全 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 拥有四维资源护栏：深度（`MaxDepth`/递归守卫）、数量（`MaxAgentCalls`/调用次数上限）、
时间（`Timeout`/超时）、内存（`MaxOutputBytes`/输出上限）。这四个护栏全部**在 forge-core 层独立实现**，
不依赖底层 agent CLI 的能力。

但**第五维——每 agent 调用的美元成本上限——却是 vendor-specific 的透传**。`--agent-max-budget-usd`
被直接拼入 `claude` 的 `--max-budget-usd` 参数（`engine_build.go:109-110`）：

```go
// forge-core/cmd/forge/engine_build.go:109-110
if o.agentMaxBudgetUSD != "" {
    argv = append(argv, "--max-budget-usd", o.agentMaxBudgetUSD)
}
```

对于非 claude 的 agent CLI——`--agent-cmd echo`（用于测试）、自定义 agent、或未来的其他厂商 CLI——
**这个参数被静默丢弃**。forge-core 本身从不拦截超出预算的调用。

### 场景与影响

| 场景 | agent CLI | 防护状态 | 风险 |
|------|-----------|----------|------|
| `forge run --agent-cmd claude --agent-max-budget-usd 10` | claude | ✅ 透传为 `--max-budget-usd 10` | claude SDK 会中断超预算调用 |
| `forge run --agent-cmd my-llm --agent-max-budget-usd 10` | 自定义 CLI | ❌ forge-core 不强制 | 自定义 CLI 可能无视参数，费用失控 |
| `forge run --agent-cmd echo`（测试用） | echo（stub） | ❌ forge-core 不强制 | 测试中逃逸，但 echo 不花钱所以无影响 |
| `forge run --executor dry` | dry-run | ❌ 不适用 | 无成本，安全 |
| 未来多厂商路由后的非 claude 路径 | LiteLLM/OpenAI | ❌ forge-core 不强制 | 费用失控的高风险面 |

### 差异化证明

- `high-value-extension-directions.md` 方向一的表格将 `--agent-max-budget-usd` 列为已就绪的
  「per-call 美元上限」，但**未区别 vendor-specific 透传和 forge-core 自有的强制**——它认为
  `--agent-max-budget-usd` 就是已交付的解决方案，但实际它只在 claude 上工作
- `expansion-direction-analysis.md` 讨论「预算借贷」和 run-level 预算，聚焦于跨迭代的**累计**预算，
  不是每调用的**瞬发**预算
- 方向四（本文）的「预算感知相位执行」是近邻但不重叠——方向四关于**梯度行为**（接近预算时降档），
  方向一是关于**每调用有绝对上限**，防止单次调用把整轮预算烧穿

### 代码级证据

1. **`engine_build.go:62-111`** — `claudeArgv` 构建函数，只在 CLI 为 claude 时将
   `--agent-max-budget-usd` 映射为 `--max-budget-usd`

2. **`engine_build.go:110`** — 该参数由 `o.agentMaxBudgetUSD` 字段承载，这个字段来自
   `runOpts.agentMaxBudgetUSD`，在 `main.go:181-185` 定义，但**只被 `claudeArgv` 消费**。

3. **`executor.go:26-31`** — `AgentExecutor` 接口没有任何预算约束：
   ```go
   type AgentExecutor interface {
       Execute(ctx context.Context, p asset.Phase, mode string) error
   }
   ```
   没有 `Budget() float64`、没有 `CostCap() float64`——预算约束完全在 `cmd/forge` 层透传，
   不在核心 `orchestrator` 层强制。

4. **`command_executor.go:82-100`** — `CommandExecutor.Build` 是函数字段，forge-core
   自己不校验其输出指令的成本含义。

### 边界情况

- **0 值语义**: `--agent-max-budget-usd ""` 表示无上限。当 CLI 不是 claude 时，这是唯一行为。
  如果有人以为 `""` 意味着 forge-core 在守卫，他是错的。
- **多 vendor 共存**: 未来跨厂商路由时，不同 CLI 有不同的预算接口。forge-core 需要一个
  vendor-agnostic 的自有上限，在 AgentExecutor.Execute 之前拦截。
- **与 run-level 预算的关系**: `--run-budget-usd` 拦截累计总花费。每调用上限是**互补**维度——
  一个昂贵调用在累计预算耗尽前已烧尽单次允许的上限。
- **与 claude 自身 `--max-budget-usd` 的交互**: forge-core 的自有上限应 ≤ 透传给 claude 的
  `--max-budget-usd`，或者完全替代它。

### 价值

ForgeOS 的架构承诺是「治理层独立于底层 CLI」。但成本保护是目前唯一**依赖 vendor 具体实现**
的护栏维度。填补此缺口后，所有 **5 个资源维度**都在 forge-core 层独立强制，不取决于 agent CLI
的合规程度。这是向真正的 multi-vendor 治理迈出的必要一步。

---

## 方向二 · 声明式输出契约的自动化验证

**优先级**: 🟢 P2 | **类别**: 治理 · 契约完整性 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

每个 workflow phase 都可以声明 `emits` 字段，列出该 phase 预期产生的文件路径。`prompt_context.go`
和 `prompt_artifacts.go` 读取这些声明并用于两件事：
1. 将已存在的 emit 文件内容注入下游 phase 的 prompt（`emitsContext`）
2. 在 readonly phase 的 narration 中告知 agent 允许写哪些目录（`engine_build.go:197-201`）

但 **没有任何机制验证声明是否被履行**。如果 planner phase 声明 `emits: [task-plan.md]`
但实际 agent 未创建该文件：
- 下游 phase 的 prompt 不会收到 task-plan.md 内容（`emitsContext` 静默跳过缺失文件）
- 不会产生告警、不会失败、不会记录到 trace
- 上下游之间的信息传递缺口完全透明

### 代码级证据

1. **`prompt_artifacts.go:37-55`** — `emitsContext` 读取 emit 文件，缺失时仅通过可选的 `logln`
   回调输出 WARNING，**从不返回错误或影响控制流**：
   ```go
   // prompt_artifacts.go:37-55
   for _, path := range emits {
       data, err := os.ReadFile(fullPath)
       if err != nil {
           if logln != nil {
               logln(fmt.Sprintf("forge: WARNING emits %q not found (%v)", fullPath, err))
           }
           continue // missing file is not an error
       }
       // ...
   }
   ```

2. **`asset.go:87-91`** — `Emits` 字段的文档明确说它是 `OPTIONAL` 且使用方应该信任它：
   ```go
   // Emits is an OPTIONAL list of file paths that this phase is declared to produce...
   // When populated, the prompt builder can read and inject the actual content...
   ```

3. **`engine_build.go:197-201`** — 声明式输出仅用于 readonly 权限的叙述，不验证：
   ```go
   if len(p.Emits) > 0 {
       emits = strings.Join(p.Emits, ", ")
   }
   logln(fmt.Sprintf("phase %s: readonly=true ... MAY still write its declared emits: %s",
       p.Name, emits))
   ```

4. **`engine_build_test.go:308`** — 现有测试只验证 narration 文本包含 emits 路径声明，
   不验证 emits 路径的文件是否真的存在。

### 差异化证明

- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向一讨论
  「结构化输出协议」，那是关于 **agent 响应格式**的标准化（如何让不同 agent 输出可解析的结果），
  不是关于**声明式文件产出**的验证
- `strategic-extensions-v22-silent-failure-modes.md` 讨论无声失败模式——emits 缺失的静默跳过
  本应是它覆盖的案例，但在该文档的搜索中未被识别为独立方向
- `forgotten-five-system-boundaries.md` 方向四「产生物验证」提到验证产出文件，但聚焦于
  **质量验证**（测试、lint）而非**存在性验证**

### 场景与影响

| 场景 | 当前行为 | 应行为 |
|------|----------|--------|
| planner 未产出 task-plan.md | 下游 phase 无 planner 上下文，继续执行 | 至少 WARN 到 trace，可选择 fail-closed |
| reviewer 未产出 review-findings.md | 下游 qa 无评审意见，形同虚设 | 检测缺失，记录到收敛信号 |
| agent 产出但路径与 emits 声明不一致 | 下游读不到，声明与实际漂移 | 检测未声明文件，或声明文件未产出 |
| 跨迭代 evolve 中 emit 文件被意外删除 | 下游读不到，静默跳过 | 可检测到「之前存在→现在缺失」的变化 |

### 建议方向

- **Phase 级 emits 验证钩子**: 在 phase 执行完毕后，读取声明的 emits 路径并记录存在性到 trace
- **声明-实现漂移检测**: 比对 `emits` 声明与实际在 phase 产出时间窗内创建的文件集，报告差异
- **收敛信号的发射源**: 将缺失的核心产出（如 task-plan.md、prd.md）纳入 converge 信号，
  让输出契约成为收敛条件的一部分
- **下游 phase 的输入质量告警**: 当下游 phase 期望读取的 emit 文件不存在时，在 prompt 中明确
  标注此上下文不可用，而非静默跳过

---

## 方向三 · 三存储跨会话一致性审计

**优先级**: 🟠 P1 | **类别**: 可靠性 · 数据完整性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 在 `.forge/` 下维护三个独立的持久化存储：

| 存储 | 格式 | 用途 | 写入模式 | 增长模式 |
|------|------|------|----------|----------|
| `checkpoint.json` | 单文件 JSON | 迭代级快照（iteration/roadmap/gates/spent） | 覆盖写入 | 固定大小（单行 JSON） |
| `trace.jsonl` | JSONL（逐行 JSON） | 事件级可审计记录 | 仅追加 | 线性增长 |
| `memory.jsonl` | JSONL（逐行 JSON） | 跨会话知识积累 | 仅追加 | 线性增长 → Compact |

**三者之间当前没有任何一致性保证**。可能出现以下无人检测的数据漂移：

1. **相位分歧**: trace.jsonl 记录了 5 个 agent phase 的事件，但 checkpoint.json 的 `PhaseIndex`
   指向第 3 个——中间 2 个 phase 的事件无对应的检查点状态
2. **幽灵 memory**: memory.jsonl 包含迭代 7-12 的知识条目，但 checkpoint 历史只有 1-6（
   迭代 7-12 的检查点可能因崩溃从未落盘）
3. **孤 trace 事件**: trace 记录了 `kind:"converge", status:"MET"` 但 checkpoint.json 从未被写入
   （写 checkpoint 的流程在写 trace 之后、落盘之前崩溃）
4. **时间倒流**: memory 条目和 trace 事件的时间戳在跨会话恢复时可能跳跃——`resume` 机制从
   checkpoint 重建 Engine，但 memory 和 trace 是独立追加的，不感知 resume 边界

### 代码级证据

1. **`checkpoint.go`** — `persist.Save` 只写 checkpoint，不涉及 trace 或 memory：
   ```go
   // internal/persist/checkpoint.go:45-80
   func Save(path string, cp Checkpoint) error {
       // 仅写 checkpoint.json + 滚动备份
   }
   ```

2. **`trace.go`** — `Tracer.Emit` 只写 trace.jsonl，不通知 checkpoint：
   ```go
   // internal/trace/trace.go:89-95
   func (t *Tracer) Emit(ev Event) error {
       // 仅写 trace.jsonl，不关联 checkpoint seq
   }
   ```

3. **`memory.go`** — `memory.Append` 只写 memory.jsonl，不带 checkpoint 引用：
   ```go
   // internal/memory/memory.go:173-210
   func Append(path string, e Entry) error {
       // 仅追加 memory.jsonl，无 checkpoint 关联
   }
   ```

4. **`loop.go:96-112`** — `OnIteration` 钩子按顺序执行：RunFrom → signals → OnIteration → 
   checkpoint。但三者不是事务性的——如果 OnIteration（含 trace emit) 完成但 checkpoint 写入
   失败，trace 已有数据但 checkpoint 无对应记录。

5. **`memory_compact.go:130-180`** — `Compact` 的重写路径使用 `.tmp` + `rename` 原子操作，
   保证单文件一致性。但**不涉及** trace 或 checkpoint——压缩后的 memory 与 trace 记录的历史
   不再对应（旧的 memory 条目被摘要取代，但 trace 仍引用原始条目）。

### 差异化证明

- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向三「三存储状态
  生命周期」最接近，但聚焦于**数据文件本身的完整性**（文件系统级），而不是三存储之间的
  **交叉一致性**——它讨论的是「如何知道文件没损坏」，不是「三个数据视图是否相互一致」
- `novel-extensions-v36-deep-architectural.md` 方向三「状态恢复」讨论从崩溃恢复时的状态
  重建，不涉及运行时的三存储交叉校验
- `governance-prod-five-frontiers.md` 方向二「状态目录的灾难恢复」从运维角度讨论 `.forge/`
  备份策略，不是数据一致性
- `forgotten-five-foundations.md` 方向三「持久化状态自校验」最接近，但仅停于
  「检查自身格式可解析」，不涉及跨存储的一致性检核

### 建议方向

- **一致性标记**: 在 checkpoint 中记录最新的 trace seq 和 memory entry count，使三者可交叉引用
- **启动审计**: `forge doctor` 扩展为在启动时交叉校验三个存储的边界一致性
- **事务性写入组**: 在关键边界（iteration 完成、converge 判定）将 trace emit + checkpoint save 
  封装为组提交——要么全部成功，要么全部回滚
- **resume 一致性验证**: 从 checkpoint resume 时验证 trace 和 memory 的起点与 checkpoint
  的记录一致，不一致时报告具体差异而非静默继续

---

## 方向四 · 预算感知的相位执行梯度决策

**优先级**: 🟢 P2 | **类别**: 成本优化 · 运行时 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的预算守卫是**二元的**：`checkRunBudget`（`budget.go:76-90`）在每次 agent phase 执行前
询问 BudgetExhausted 回调，一旦返回 true，即**硬停**（hard stop），不再执行任何后续 phase。

```
预算剩余 100% → [执行 phase] → [执行 phase] → 预算用尽 → ✋ HARD STOP
```

**在「预算充足」和「预算耗尽」之间不存在任何梯度行为**。系统不会在接近预算上限时主动调整行为，
比如：
- 使用更便宜的模型降低单 phase 成本
- 减少重试次数（由 `MaxRetries` 全程不变）
- 跳过可选的 review phase
- 缩短 prompt（省略 memory 检索、降低 adrTopK）
- 减少 evolve 的预期迭代深度

这种二元行为的后果：

1. **预算后 10% 的浪费**: 如果 run budget 是 $10，在花掉 $9 后，剩下 $1 不足以跑完一个
   Opus phase（~$0.35 per call）。系统进入 hard stop，最后 $1 永远用不上
2. **成本不可预测**: 用户在提交任务时只知道预算上限，但不知道系统在接近上限时的行为。
   一个需要 $9.50 的任务可能在 $9.00 时因硬停而失败
3. **无主动成本告警**: 当前没有机制在「预算还剩 20%」时通知 operator 即将到达上限

### 代码级证据

1. **`budget.go:76-90`** — `checkRunBudget` 的二元行为：
   ```go
   func (e Engine) checkRunBudget(completed int) error {
       if e.BudgetExhausted == nil || !e.BudgetExhausted() {
           return nil // 预算足够，继续执行
       }
       // 预算耗尽，硬停
       return fmt.Errorf("run budget exhausted: stopped after %d completed agent phase(s)...")
   }
   ```

2. **`routing.go:150-200`** — `BudgetAdjustTier` 是**唯一**的预算感知降档机制，但它只作用于
   **未来 phase 的模型选择**，不影响：
   - 当前 phase 的执行参数（超时、重试次数、输出上限）
   - prompt 的复杂度（memory 检索量、ADR 注入数）
   - phase 的跳过决策

3. **`loop.go:45-60`** — `LoopEngine.MaxIter` 是硬上限，不随预算剩余动态调整：
   ```go
   // MaxIter is a SAFETY backstop — never the goal
   ```

4. **`preflight.go:117-124`** — `checkCostEstimate` 在 run **之前**给出估算，但 run **期间**
   没有动态的剩余预算健康检查。

### 差异化证明

- `high-value-extension-directions-v2.md` 提到「run-level 美元硬上限」和「per-call cost 封顶」，
  但完全聚焦于**预算上限本身**的存在性，不是**接近上限时的动态行为**
- `expansion-direction-analysis.md` 的「预算借贷」讨论跨 iteration 的预算分配，不是单个
  iteration 内的梯度行为
- 所有已经交付的预算文档都假设「上限是 final safety net，达到就停」——这是正确的安全假设，
  但跳过了一个重要 UX 和成本优化层面：「在到达上限之前系统应该做什么」。

### 建议方向

- **预算水位指示器**: 在 Engine 中添加 `BudgetWatermark`（剩余 30%/20%/10% 三级阈值），
  阈值触发时调用可配的回调
- **梯度降档级联**: 在预算水位下降时自动执行以下降档序列：
  1. 剩余 < 30%: 缩短 prompt（减少 memory 检索、降低 `adrTopK`）
  2. 剩余 < 20%: 降低模型 tier（opus → sonnet，对非安全关键 phase）
  3. 剩余 < 10%: 减少 `MaxRetries`，跳过可选 phase（secondary_template）
  4. 剩余 < 5%: `CheckRunBudget` 前通知 operator 即将硬停
- **cost-aware MaxIter**: `LoopEngine.MaxIter` 可根据 `runBudgetUsd / avgPhaseCost`
  动态计算建议值，在 preflight 中给出更精确的估算

---

## 方向五 · 并发相位资源争用检测与缓解

**优先级**: 🟢 P2 | **类别**: 可靠性 · 并行安全 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐

### 问题描述

并行引擎（`RunParallel`/`runWave`）允许同一 wave 内的多个 phase 并发执行。`parallel.go` 有
完善的**锁排序合约**和**fail-fast wave 取消**机制，确保共享数据结构的并发安全。但并行引擎
完全不了解 phase 之间的**外部资源争用**：

1. **文件写入冲突**: 两个 implementer phase 可能同时尝试写同一个文件——例如两个 agent 各自
   被分配实现不同的 feature，但都修改了 `main.go`。当前没有写冲突检测，后写完的会覆盖前者。
2. **共享 git 状态**: 并发 phase 对 git 的状态（staging、commit）没有协调。如果两个 phase
   都自动 commit，会产生冲突的 git 历史。
3. **API 速率限制**: 多个并发 phase 各自调用 agent CLI，可能触发底层 LLM API 的速率限制
   （rate limit），使得所有并发 phase 同时失败——而非逐个退避。
4. **磁盘 I/O 争用**: 并行 phase 同时读取大型文件（如 trace.jsonl 重放、memory.jsonl 加载），
   可能在 IO-bound 场景下比串行更慢。
5. **进程表耗尽**: 每个 phase 生成一个子进程（agent CLI），wave 的并发度可能导致系统进程表
   或文件描述符耗尽。

### 代码级证据

1. **`parallel.go:130-140`** — `runPhaseParallel` 对每个 phase 独立 spawn 子进程，不协调
   子进程的资源使用：
   ```go
   go func(i int) {
       defer wg.Done()
       if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
           // fail-fast 处理，但不关心资源争用
       }
   }(idx)
   ```

2. **`waves.go:45-70`** — Kahn 拓扑排序生成 wave，但 waves 的唯一约束是 `depends_on`，
   不包含任何资源维度（文件亲和性、资源消耗、IO 密集度）

3. **`command_executor.go:110-130`** — 每个 phase 生成独立子进程，使用 `cmd.Run()`，
   系统层面可能达到 `ulimit -u`（最大用户进程数）或 `ulimit -n`（文件描述符数）

4. **`memory.go:120-140`** — `loadCache` 使用 `sync.Map` 在进程内共享缓存，但在并行模式下，
   如果两个 phase 同时 `Append` 到 memory（当前是串行路径），没有防冲突机制。

5. **`command_executor_unix.go:49-60`** — 子进程使用 `Setpgid` 分进程组，但 wave 级取消
   （`waveCancel`）只取消 forge 自己的 goroutine，不协调多个子进程的信号传播顺序。

### 差异化证明

- `parallel.go` 的锁排序合约覆盖了**内部数据结构**（trace/runBudget/ledger/cache），
  但该合约的文档（第 25-50 行）本身明确声明只覆盖共享内存状态，不覆盖外部资源
- `novel-five-perspectives-2026-07-10-deep.md` 方向二「并行 gate 串行」讨论 gate phase
  在并行模式下的串行瓶颈，不是 agent phase 之间的资源争用
- `second-order-architectural-gaps.md` 方向三「并行安全」覆盖数据竞争和死锁，不涉及
  文件系统或外部 API 的争用
- `high-value-perspectives-v11.md` 方向四「相位间契约/交接协议」讨论信息传递，
  不涉及资源争用

### 建议方向

- **写冲突检测**: 在并行 phase 执行前/后，对声明的 `emits` 路径和常见的项目文件做更改检测。
  如果两个并发 phase 都修改了同一文件，记录到 trace 和收敛信号中
- **并发度自适应**: 根据 `ulimit` 的剩余容量和 `runtime.NumCPU`/`runtime.GOMAXPROCS` 
  动态决定 wave 内的最大并发 phase 数，而非尽数 spawn
- **wave 级资源预算**: 为每个 wave 分配资源配额（最大进程数、最大并发文件写入数），
  超过时在该 wave 内串行执行剩余 phase
- **文件系统事务标记**: phase 开始时记录受影响的文件快照（git status），phase 结束时
  检查是否有其他 phase 修改了相同文件并报告冲突

---

## 优先级与收敛建议

| 方向 | 优先级 | 杠杆 | 为什么是这个优先级 |
|------|--------|------|-------------------|
| **方向一 · 厂商无关每调用成本护栏** | **P1** | ⭐⭐⭐⭐⭐ | 五个资源维度中唯一一个依赖 vendor 实现的缺口。非 claude 时无防护，费用失控风险高 |
| **方向三 · 三存储跨会话一致性** | **P1** | ⭐⭐⭐⭐⭐ | 三个持久化存储是故障恢复和审计的全部依据。它们不一致时，重建信任的唯一方式是人工介入 |
| 方向二 · 声明式输出契约验证 | P2 | ⭐⭐⭐⭐ | 严重影响上下文的完整性，但目前仍是「无声降级」而非数据丢失。治理完整性的重要补充 |
| 方向四 · 预算感知梯度决策 | P2 | ⭐⭐⭐⭐ | 核心 UX 改进——让用户在费用可控的同时最大化迭代价值。但不是安全或数据完整性缺口 |
| 方向五 · 并发资源争用检测 | P2 | ⭐⭐⭐ | 当前 `depends_on` 为零因此并行路径休眠，目前无实际暴露。需求随并行引擎启用而增长 |

### 收敛建议

- **若只做一件**: **方向三（三存储一致性）**——杠杆最高。在 24h 无人值守长跑中，崩溃恢复后
  数据的可信度直接决定系统是否可重新信任。三个存储之间无一致性标记意味着即使 checker 正常，
  也无法证明数据是完整的。
- **做前三件**: **方向一 + 方向三 + 方向二**——分别解决：vendor 耦合成本保护、数据跨存储
  完整性、隐式输出契约断裂。三者覆盖了「成本安全 + 数据安全 + 治理完整」的三角。
- **方向四和五**是更高阶层的优化——方向四需要先有运行中的预算数据（Sprint 26 真点火数据
  已落盘，条件成熟），方向五需要 `depends_on` 在真实 workflow 中被使用（目前为零）。

---

## 诚实边界

以上五个方向基于对**当前代码库（2026-07-10）**的全局扫描。以下情况可能导致方向的实际价值
低于预估：

1. **方向一**: 如果 ForgeOS 的生产部署永远只使用 claude 作为 agent CLI，vendor-specific 的
   `--max-budget-usd` 透传可能就足够了。但架构目标是多厂商——因此缺口是结构性的，不是理论性的。
2. **方向二**: `emits` 当前只被 5 个 workflow 中的少数 phase 使用（planer's `task-plan.md`、
   discover phases 的 `prd.md` 等）。如果 emits 的采用率不增长，验证的增益也有限。
3. **方向三**: 在短于 1 小时的运行中，三个存储之间的不一致概率很低。漏洞主要在 24h+ 长跑和
   跨会话 evolve 中暴露——这正是 ForgeOS 的目标场景。
4. **方向四**: 梯度过多的降档序列可能使系统行为难以预测。任何实现都必须在「梯度有用」和
   「行为可预测」之间取得平衡，并将所有降档决策记录到 trace。
5. **方向五**: 当前 `depends_on` 在所有 5 个 workflow 中均为空，并行路径实际上处于休眠状态。
   `parallel.go` 的 file header 已诚实标注这一现状。方向五直到有 workflow 启用 depends_on
   才具有紧迫性。
