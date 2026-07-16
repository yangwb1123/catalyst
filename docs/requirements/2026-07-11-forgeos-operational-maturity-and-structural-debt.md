# ForgeOS — 运营成熟度与结构性债务：代码级扫描后的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫：forge-core 18 Go 包（~35k LOC）· cmd/forge 16 生产文件 + 15 测试文件 ·  
>    harness 39+ 模块（Node/Python）· 完整 `.agent/` 治理骨架（5 workflow · 12 agent 卡 · 全部策略）  
> 2. 通读 Sprint 1–31 演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` + 所有 ADR  
> 3. **与 ~180 份已有 `docs/requirements/*.md` + `docs/analysis/*.md` 全文交叉验证**，  
>    确保每个方向的核心论点在已有分析中**从未作为独立系统性方向展开**  
> 4. **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、产品价值判断、边界场景  
> **日期**: 2026-07-11

---

## 全景定位：已有 180+ 份分析覆盖的饱和区域（本文不重复）

以下域已被大量已有分析充分覆盖，本文不再展开：

| 饱和域 | 代表性文档 |
|---|---|
| 编排引擎（串/并行/loop-back/resume/mode-gating/checkpoint） | ~35+ 方向 |
| 学习闭环（trace/scorecard/converge/memory/context/routing） | ~20+ 方向 |
| 生产韧性（529/退避/预算/递归守卫/进程组/超时） | ~20+ 方向 |
| 安全纵深（secret-scan/SCA/risk/readonly 强制/注入防御） | ~15+ 方向 |
| 治理执法（arch-check 8 检查/check.py/function-length/circular） | ~12+ 方向 |
| 跨项目联邦与多仓库编排 | ~10+ 方向 |
| 数据生命周期（trace 旋转/memory 紧缩/checkpoint 历史） | ~8+ 方向 |
| 并行写冲突与并发安全 | ~6+ 方向 |
| YAML 解析器可靠性（yaml2json Go-native vs Python shim） | ~5+ 方向 |
| 结构化输出（`--json` flag 缺失） | ~5+ 方向 |
| 热加载/自愈/daemon 模式 | ~4+ 方向 |
| 跨会话知识衰减/TTL | ~4+ 方向 |
| 三框架债务（`.agent/` vs `.ai/` vs `ai-dev/`） | ~4+ 方向 |
| 二阶伴生（TOCTOU/配置爆炸/无声数据丢失） | ~3+ 方向 |
| 多维度模型路由（complexity/dependency/context/business-impact） | ~3+ 方向 |

本文的 4 个方向全部落在上述饱和域之间的**不可约间隙**中：**结构性债务的累积模式**与**运营成熟度缺口**。

---

## 方向一 · `cmd/forge` 包的「围墙花园」债务 —— 没有系统性地包提取策略

**优先级: P1 | 类别: 架构 · 可维护性 | 影响范围: cmd/forge 全包**

### 代码级证据

ForgeOS 经过 31 轮 sprint，`cmd/forge` 已从单一的 `main.go` 分裂成 **16 个生产 Go 文件**，**但每一份文件都在 400–500 行**：

```
forge-core/cmd/forge/
  main.go                499  ← 几近闸门极限
  engine_build.go        498
  evolve.go              496
  gates.go               493
  validate.go            488
  cost.go                471
  prompt_context.go      454
  scorecard_wind.go      451
  prompt_memory.go       416
  route.go               374
  detect.go              338
  preflight.go           289
  prompt_artifacts.go    237
  gate_resolve.go        200  (已被拆至 internal/gate)
  migrate.go             114  (薄 CLI 胶水，逻辑在 internal/migrate)
  approve.go             122
```

关键观察：

1. **每个文件都是"即将超过 500"的状态** —— 16 个生产文件里，8 个在 450–499 行，占 50%。这意味着下一个小修复就可能触发 `gate.mjs` 的 block。每次超线都触发一次紧急拆分，而不是系统性规划。

2. **拆分模式是应急的（piecemeal），不是架构驱动的**。已有 4 轮拆分：
   - `engine_build.go` ← 从 `main.go` 拆出（`engine` 组装逻辑）
   - `prompt_artifacts.go` ← 从 `prompt_context.go` 拆出（artifact context 注入）
   - `gate_resolve.go` → `internal/gate/resolve.go`（Sprint 29 纠正：纯逻辑应该进 internal 包）
   - `approve.go` ← 从 `gates.go` 拆出（`forge approve list` CLI）

   **每次都是"超线才拆"，没有"这个职责属于 internal/ 的哪个包"的架构判断**。代价是 Sprint 29 暴露的：`gate_resolve.go` 先被留在 cmd/forge，然后才被纠正移到 `internal/gate`。

3. **cmd/forge 承载了 7+ 种不同的职责**，但全部混在一个包名 `package main` 下：

| 职责 | 文件 | 应归属 |
|---|---|---|
| CLI 入口与 dispatch | `main.go` | cmd/forge（合理） |
| Engine 组装（prompt/executor/gate wiring） | `engine_build.go` | cmd/forge（合理） |
| 自治循环（evolve loop） | `evolve.go` | cmd/forge（合理） |
| Gate 结果收集与收敛信号 | `gates.go` | 部分已拆至 `internal/gate/resolve.go` |
| 预算追踪与成本遥测 | `cost.go` | 可沉入 `internal/orchestrator` |
| Prompt 组装（context/memory/artifact） | `prompt_context.go`, `prompt_memory.go`, `prompt_artifacts.go` | 可沉入 `internal/prompt` |
| Scorecard 写入与灾难恢复 | `scorecard_wind.go` | 可沉入 `internal/routing` |
| 项目检测与建议 | `detect.go` | cmd/forge（合理） |
| Workflow 验证与状态查询 | `validate.go` | 混合（部分已拆至 `internal/doctor`） |
| 模型路由 CLI | `route.go` | 薄 CLI（合理） |
| 预检 | `preflight.go` | 薄 CLI（合理） |
| 审批列表 | `approve.go` | 薄 CLI（合理） |
| 状态迁移 | `migrate.go` | 薄 CLI（合理，逻辑在 `internal/migrate`） |

**问题不是"cmd/forge 文件太多"，而是"太多职责堆在一个包名空间内，没有包级隔离"**。`prompt_context.go` 中的 `promptLedger`、`verdictLedger`、`reviewFindingsLedger` 等类型被 `cost.go`、`gates.go`、`scorecard_wind.go` 等直接引用，因为它们在同一个 `package main` 中。这违反了 ForgeOS 自身的「单向依赖」纪律：这些类型没有包边界保护。

### 具体证据行

```go
// forge-core/cmd/forge/prompt_context.go:35-85
type verdictLedger struct { ... }      // 被 cost.go 引用
type gateLedger struct { ... }         // 被 engine_build.go 引用
type phaseOutputLedger struct { ... }  // 被 engine_build.go 引用
type reviewFindingsLedger struct { ... }

// forge-core/cmd/forge/cost.go:42-165
type runBudget struct { ... }          // 被 engine_build.go、evolve.go 引用
func newRunBudget(...)                // 在 evolve.go 中调用
func (b *runBudget) feed(...)         // 在 engine_build.go 中调用

// forge-core/cmd/forge/gates.go:275-330
type loopProbe struct { ... }         // 被 buildLoop 使用
```

这些类型和函数跨越 `cmd/forge` 中几乎每个文件，构成了一张**隐式内部依赖网**。提取任何一个需要理解整张网。

### 为什么需要

- **维护成本**: 新 contributor 要修改 `cost.go` 中的 budget 逻辑，必须理解 `prompt_context.go` 中的 ledgers、`gates.go` 中的 probe、`engine_build.go` 中的 wiring。没有包边界，认知负荷随文件数超线性增长。
- **测试隔离**: 当前 `runBudget` 的测试要 import `cmd/forge` 整个包（通过 main_test.go 的子进程模式）。如果 `runBudget` 在 `internal/orchestrator/budget.go` 里，可以写纯单元测试。
- **闸门反复触发**: `.arch/rules.yaml` 中 `package.max_files` 已从 14 调整到 17（现在 16 文件）。如果不系统性拆分，下次调整在 3–5 个 sprint 内必然再次发生。

### 建议方向

**系统性包提取（Strategic Package Extraction）**：不是"超线时拆一个函数"，而是**先映射依赖图，再按架构层批量提取**：

| 阶段 | 提取目标 | 目标包 |
|---|---|---|
| 1 | `runBudget` + 成本遥测 | `internal/orchestrator/budget.go`（已有 `orchestrator` 包） |
| 2 | `verdictLedger` / `gateLedger` / `phaseOutputLedger` / `reviewFindingsLedger` | `internal/prompt`（已有 `prompt` 包） |
| 3 | `loopProbe` / `gatherSignals` / `reportConvergence` | `internal/converge`（已有 `converge` 包） |
| 4 | `quickDoctorCheck` / `cmdDoctor` 中非 CLI 逻辑 | `internal/doctor`（已有 `doctor` 包） |

**原则**:
- 每次提取后，cmd/forge 减少 1–2 文件，internal/* 包增加 0–1 文件
- 提取后的包不引入新外部依赖（纯 stdlib）
- 提取后的包不改变现有 CLI 行为（零行为变化，靠 git diff 空输出验证）

### 边界情况

- **提取不等于"消灭 cmd/forge"**：薄 CLI 胶水是合理的存在。目标是让 cmd/forge 只持有 [CLI dispatch + 纯编排胶水]，把 [prompt 组装 / cost 追踪 / gate 裁决 / converge 信号] 移到各自的 internal 包。
- **提取不能破坏 forge-init 的 copy-anywhere**：引用了被提取类型的测试文件也需要同步
- **中间兼容**：在全部提取完成前，cmd/forge 可以持有 wrapper 类型（薄类型别名 + 过时注释），逐步迁移调用方

### 已有覆盖检查

`expansion-five-product-blindspots.md` 的方向一（Post-Acceptance 治理管线）和方向五（自评价元认知循环）提及「ForgeOS 自身应遵守自己的纪律」，但从未系统分析 `cmd/forge` 包的结构性债务。`strategic-extensions-v33.md` 提到 `cmd/forge` 大小问题，但仅作为"性能瓶颈"而非"包边界侵蚀"。**本文是第一个从「包提取策略缺失」角度定位系统性债务的分析。**

---

## 方向二 · 状态文件的「无声版本演化」—— 零值 = 错误语义的静默风险

**优先级: P1 | 类别: 可靠性 · 数据完整性 | 影响范围: internal/asset + internal/persist + internal/trace**

### 代码级证据

ForgeOS 的状态数据结构体以 **容错（fault-tolerant）设计** 加载：

```go
// forge-core/internal/asset/asset.go:168-180
func LoadWorkflowJSON(data []byte) (Workflow, error) {
    var wf Workflow
    // 容错：缺失字段零值，不报错
}
```

这意味着每次给 `Workflow` 或 `Phase` 加新字段，所有已有 `.yml` 文件**静默获得零值语义**。有些零值正确，有些则不是：

| 字段 | 零值语义 | 何时错误 |
|---|---|---|
| `FeedsForward: false` | 不向前传递输出 | 如果新 workflow 依赖前序注入但 YAML 没声明 |
| `FreshContext: false` | 看见前面的上下文 | reviewer 本应 fresh context 但旧 workflow 没写 `fresh_context: true` |
| `Readonly: false` | 可以写文件 | discover.yml/review.yml 声明了 `readonly: true` 但旧 schema 没有该字段时被默认可写 |
| `RequiresTools: nil` | 不需要工具 | market-research phase 声明了 web_search 依赖但旧 workflow 没有该字段时不检查 |
| `DependsOn: nil` | 无依赖 → 先于任何 phase 执行 | 并行编排下，旧 workflow 的 phase 会乱序 |

**当前已有字段也面临同样风险**：

```go
// forge-core/internal/asset/asset.go:220-224
type StopCondition struct {
    Type        string            `json:"type"`
    AllOf       []Criterion       `json:"all_of,omitempty"`
    OnApproved  *OnApprovedAction `json:"on_approved,omitempty"`
    OnRejected  *OnRejectedAction `json:"on_rejected,omitempty"`
    OnUnmet     *OnUnmetAction    `json:"on_unmet,omitempty"`
    // 没有 SchemaVersion, 没有 FormatVersion, 没有 MinCompatVersion
}

// forge-core/internal/asset/asset.go:96-150
type Workflow struct {
    Stage    string        `json:"stage"`
    Phases   []Phase       `json:"phases"`
    Loop     *LoopBody     `json:"loop"`
    Stop     StopCondition `json:"stop_condition"`
    Readonly bool          `json:"readonly,omitempty"`
    // 没有 _schema_version, 没有 _format, 没有 _min_compat_version
}
```

**同样的模式在 persist 和 trace 中**：

```go
// forge-core/internal/persist/checkpoint.go:45
type Checkpoint struct {
    FormatVersion string `json:"_format,omitempty"` // 有，但只在 encode 时写入、decode 时不校验
    // ...
}

// forge-core/internal/trace/trace.go:37-43
type Event struct {
    Format string `json:"_format,omitempty"` // 有类似字段，同样消费为零验证
    // ...
}
```

`FormatVersion`/`Format` 字段存在，但**没有任何迁移引擎**。没有版本协商、没有前向/后向兼容测试、没有"格式 X 不能加载到引擎 Y"的保护。

### 证据：`checkpoint.json` 的向后兼容实际依赖 Go 零值语义

```go
// forge-core/internal/persist/checkpoint.go:76-81
type Checkpoint struct {
    PhaseIndex    int    `json:"phase_index,omitempty"`   // Sprint 27 新增
    SpentUsdMicros int64 `json:"spent_usd_micros,omitempty"` // Sprint 26 新增
    // 旧 checkpoint 没有这些字段 → 零值
    // PhaseIndex=0 → "从 phase 0 开始"（正确）
    // SpentUsdMicros=0 → "没花过钱"（错误——会把真实花费清零）
}
```

`SpentUsdMicros` 的零值语义实际上是**危险的**：旧 checkpoint 在该字段不存在时解码为 0，意味着 resume 后的 budget 重新从 $0 开始，已花费的真实预算被清零。代码通过 `seed()` 解决了这个问题（`cost.go:113-122`），但这个 fix 的存在本身就是"零值语义危险"的活证据。

### 为什么需要

- **`.forge/trace.jsonl` 的 `_format` 字段从未被消费**——它只是个元数据标签。如果未来有人决定改变 JSONL 格式（例如重构 Event 结构体），下游工具无法感知文件是新格式还是旧格式。
- **scorecards.json 也没有版本标记**（`scorecard.schema.yml` 有 schema 版本，但实际文件只有隐式语义版本）
- **agent 卡的文件格式（`.agent/agents/*.md`）** 的机读契约（`VERDICT:` / `CONFIDENCE:` 行）没有版本声明——一个 agent 卡更新了契约格式，旧解析器静默失效
- 这是 **ForgeOS「声明 vs 实现」审计**（Sprint 12/30/31）反复捕获同类问题的根因——零值语义漂移每次新增字段都会发生，却没有系统性的防护

### 建议方向

**版本化状态引擎（Versioned State Schema Engine）**——一个薄层，管理和进化所有状态文件的格式：

| 组件 | 当前状态 | 建议 |
|---|---|---|
| `asset.Workflow` / `Phase` | 无版本字段 | 加 `_schema_version`，decode 时校验 >= min_compat |
| `persist.Checkpoint` | 有 `_format` 但不校验 | decode 时校验 version >= min_compat，不匹配则报错而非零值 |
| `trace.Event` | 有 `_format` 但不消费 | 相同策略 |
| agent card 机读契约 | 无版本 | 加 `_contract_version: "v1"` 元行，parser 按版本 dispatch |

**设计原则**：
- **不改变现有行为**：当前所有文件不声明版本时，视为 `v1`（向后兼容）
- **不强制全仓升级**：v1 引擎可以加载 v1 文件，但拒绝 v2 文件
- **迁移路径在 caller，不在 loader**：`asset.LoadWorkflowJSON` 继续容错，但版本校验在 `cmdRun`/`cmdEvolve` 中作为前置守卫做

### 边界情况

- **回滚兼容**：v2 引擎应该能加载 v1 文件（前向兼容），但 v1 引擎加载 v2 文件应报错（拒绝旧加载旧格式的零值语义）
- **格式进化 ≠ 功能进化**：版本号只表示序列化格式，不表示功能级别。`_schema_version: 2` 可能只是一个字段重命名，没有新行为
- **不要重新发明 semver**：一个严格递增的整数即可。不需要主/次/补丁

### 已有覆盖检查

`genuinely-uncovered-frontiers.md` 方向一（Workflow 资产版本锁定与运行可复现性）和 `five-code-grounded-architectural-gaps-2026-07-11.md` 方向三（Trace 事件格式进化能力）分别讨论了 workflow 和 trace 的版本化。**本文讨论的版本化是 3 种状态文件（asset + persist + trace）共有的设计模式**，且聚焦在「零值语义的静默风险」而非「格式进化能力」。

---

## 方向三 · 运营积垢 —— `.forge/` 状态目录的无主增长与系统性退化

**优先级: P1 | 类别: 运维 · 可靠性 | 影响范围: .forge/ 下全部产物**

### 代码级证据

ForgeOS 的运营状态存储在 `.forge/` 目录下，4 种状态文件各有独立的清理策略：

| 文件 | 写入方式 | 清理机制 | 触发条件 | 限制 |
|---|---|---|---|---|
| `trace.jsonl` | append-only JSONL | 单层旋转（`.1` 备份） | 超过 10MB | 只有一级轮换；`forge run` 不旋转 |
| `memory.jsonl` | append-only JSONL | Compact（age-aware） | 每 10 iteration + >500 条目 | keepPerKind=20；仅 age>24h 压缩；无绝对大小上限 |
| `checkpoint.json` | 原子重写 | rotateRetain（5 版本） | 每次 Save | 只有 per-file rotation；无全目录清理 |
| `scorecards.json` | 读写（Node 脚本） | **无** | **从不** | 文件被 wind-down 脚本覆写但不清理旧文件 |

**三个关键缺口**：

**① `trace.jsonl` 旋转仅覆盖 `forge evolve`，且只有一级备份**

```go
// forge-core/cmd/forge/evolve.go:478-484
// 只在 `forge evolve` 中旋转，`forge run` 不旋转
const maxTraceBytes int64 = 10 << 20 // 10 MB
if st, err := os.Stat(tp); err == nil && st.Size() > maxTraceBytes {
    os.Rename(tp, tp+".1") // 只有 .1 一级，旧的 .1 被静默覆盖
}
```

问题是：如果一个项目每天跑 3 次 `forge evolve`，每次产生 4MB trace，则约每 3 次 evolve 旋转一次。但 `forge run`（一次性的全流程执行）产生的 trace **永远不旋转**。在 `forge run build` 这种有 5–6 个 agent phase 的 workflow 中，多 run 多次后 trace.jsonl 持续增长。

**② memory compaction 只约束条目数量，不约束物理大小**

```go
// forge-core/internal/memory/memory_compact.go:54-74
const DefaultCompactThreshold = 500       // >500 才触发
const DefaultCompactKeepPerKind = 20      // 每个 kind 保留最近 20 条
const CompactAgeSeconds = 86400           // 只压缩超过 24h 的
```

每个条目平均 200–400 bytes，500 条 ≈ 100–200KB，但如果不触发 compact（条目 < 500），memory.jsonl 可以无限增长。而在活跃项目中，memory 条目可能稳定在 400–500 条之间，永远不会到达 500 条阈值，也永远不会紧缩。

**③ `forge doctor` 报告但不修复**

```go
// forge-core/internal/doctor/doctor.go:90-98
func quickDoctorCheck(...) {
    // 检查 checkpoint 存在、可读、不为空
    // 检查 trace 存在、可读
    // 检查 memory 存在、可读
    // 报告异常，但不修复
}
```

`forge doctor` 能检测到异常（`anomaly.go` 能检测 checkpoint 跳跃/停滞/回退），但**从不尝试修复**。异常被发现后，operator 只能手动删除 `.forge/` 目录。

### 更隐蔽的问题：跨存储一致性

trace、memory、checkpoint **没有共享的运行标识符**：

```
trace.jsonl:     seq (单调递增整数，per-Tracer 实例)
memory.jsonl:   iteration 字段 (per-evolve 实例)
checkpoint.json: Iteration 字段 + PhaseIndex 字段
```

如果一次 evolve run 产生 seq 1-100 的 trace events 后崩溃，然后 operator 执行 `--resume`，新的 tracer 从 seq=1 重新开始。trace 中有两段完全相同的 seq 范围，分别对应崩溃前和 resume 后的运行——但 **无法区分它们**（没有 RunID，没有 session 边界）。

```go
// forge-core/internal/trace/trace.go:110-118
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    t.seq++               // 每次 tracer 创建从 0 开始
    ev.Seq = t.seq
    // ...
}
```

### 为什么需要

- **长期运维不可能忽略磁盘增长**：一个项目运行 6 个月后，`.forge/` 典型大小可能在 200MB–2GB 之间，取决于使用频率。没有统一的生命周期策略，operator 只能手动 `rm -rf .forge/` 解决问题——这会**丢失所有跨会话知识**（memory、scorecard、trace 历史）。
- **缺乏 cross-stroke RunID** 使得任何跨会话的 trace 分析都不可靠——`forge scorecard rebuild` 从 trace 重建 scorecards 时，无法区分同一 seq 范围对应哪次运行。
- **跨存储不一致**目前不被 `forge doctor` 检测：checkpoint 说 "iteration 3 gates green"，memory 说 "iter 5: roadmap 85%"，trace 说 "seq 42: agent start"——但没有任何一个工具能回答"这三个状态文件彼此一致吗"。

### 建议方向

**.forge/ 目录生命周期管理（`.forge/` Lifecycle Management）**——统一的清理、归档、一致性保障策略：

1. **跨存储运行标识符（RunID）**：每次 `forge run`/`forge evolve` 生成一个 UUID 或时间戳运行 ID，注入所有三条写入路径：

   ```
   trace:     {"run_id": "2026-07-11T...", "seq": 1, ...}
   memory:    {"run_id": "2026-07-11T...", "iteration": 1, ...}  
   checkpoint: {"run_id": "2026-07-11T...", "iteration": 1, ...}
   ```

2. **统一配额策略**（可配置）：

   | 存储 | 默认上限 | 达到上限行为 |
   |------|---------|------------|
   | trace.jsonl | 50MB（或 3× 备份） | 旋转，保留最近 3 个备份 |
   | memory.jsonl | 20MB（或 2000 条目） | 强制 compact，再超限则逐出最旧条目 |
   | checkpoint 历史 | 10 版本 | 保留 N 个历史版本 |
   | `.forge/` 总计 | 500MB（可配置） | `forge doctor` 告警 + `forge status` 显示 |

3. **`forge maintain` 命令**（类比 `git gc`）：

   ```
   forge maintain [--dry-run] [--quota 500MB] [--keep-checkpoints 10]
   
   操作清单:
     ✅ 验证 checkpoint/trace/memory 的 RunID 一致性
     ✅ 清理孤立/超龄文件
     ✅ 强制执行磁盘配额
     ✅ 压缩超过阈值的 trace（旧 events → 摘要）
     ✅ 重建 scorecard 一致性（从 trace 和 checkpoint）
     ✅ 报告 .forge/ 健康摘要
   ```

4. **`forge doctor --fix`**：目前 doctor 只报告不修复。`--fix` 模式自动修复已知异常模式（损坏的 checkpoint -> 回退到上一个备份、截断的 trace -> 修复 JSONL 格式、memory 超限 -> 强制 compact）。

### 边界情况

- **RunID 的进程级唯一性**：多个并发 forge 进程（如果未来支持并行操作）不能生成相同的 RunID——建议用 `hostname + pid + timestamp + 随机数`
- **`forge doctor --fix` 的安全边界**：自动修复不删除数据（可以回滚）。修复前备份被修改的文件
- **配额不是硬限制**：超过配额时 `forge doctor` 告警 + `forge run`/`forge evolve` 输出 WARNING，但**不阻止执行**——宁可让磁盘多占一点，也不能阻止 operator 的紧急修复
- **trace 压缩≠数据丢失**：压缩后的 trace 可以失去 event 级粒度，但保留统计摘要（时间范围、event 计数、总 cost）。v1 可只旋转不压缩

### 已有覆盖检查

`genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向五（状态数据生命周期管理）和 `2026-07-11-forgeos-five-unbuilt-product-architectural-extensions.md` 方向三（`.forge/` 运行时产物生命周期）覆盖了生命周期管理的基础方向，但**没有讨论跨存储 RunID 一致性、磁盘配额统一管理、以及 `forge maintain` 命令**。本文聚焦在「运营积垢的系统性退化」而非「单个文件的清理策略」。

---

## 方向四 · 代理输出契约的「沉默退化」—— 逐行文本匹配的累积脆弱性

**优先级: P2 | 类别: 可靠性 · 解析韧性 | 影响范围: cost.go + prompt_context.go + gates.go**

### 代码级证据

ForgeOS 通过**从 agent 输出中解析特定文本模式**来驱动核心控制流：

```
从 reviewer 输出解析: VERDICT: APPROVE / VERDICT: REQUEST_CHANGES
从 cto 输出解析:      VERDICT: APPROVE / REDESIGN / DELAY / REJECT
从 product-manager 输出解析: CONFIDENCE: 85
```

这些解析全部基于**末行精确匹配**：

```go
// forge-core/cmd/forge/cost.go:330-350
func parseReviewerVerdict(output string) string {
    // 反包裹 claude JSON 输出
    unwrapped := unwrapClaudeResult(output)
    // 倒序遍历行，找 VERDICT: 模式
    lines := strings.Split(unwrapped, "\n")
    for i := len(lines) - 1; i >= 0; i-- {
        trimmed := strings.TrimSpace(lines[i])
        if trimmed == "VERDICT: APPROVE" {
            return "APPROVE"
        }
        if trimmed == "VERDICT: REQUEST_CHANGES" {
            return "REQUEST_CHANGES"
        }
    }
    return ""
}
```

```go
// forge-core/cmd/forge/cost.go:370-400
func parseExecutiveVerdict(output string) string {
    lines := strings.Split(unwrapped, "\n")
    for i := len(lines) - 1; i >= 0; i-- {
        trimmed := strings.TrimSpace(lines[i])
        if trimmed == "VERDICT: APPROVE" {
            return "APPROVE"
        }
        if trimmed == "VERDICT: APPROVE_WITH_SIMPLIFICATION" {
            return "APPROVE_WITH_SIMPLIFICATION"
        }
        // ...REDESIGN, DELAY, REJECT
    }
    return ""
}
```

```go
// forge-core/cmd/forge/cost.go:410-430
func parseConfidenceScore(output string) int {
    lines := strings.Split(unwrapped, "\n")
    for i := len(lines) - 1; i >= 0; i-- {
        trimmed := strings.TrimSpace(lines[i])
        if strings.HasPrefix(trimmed, "CONFIDENCE: ") {
            // 提取数值...
        }
    }
    return 0
}
```

**这个模式有 4 个脆弱性**：

1. **格式依赖**：`claude -p --output-format json` 产生的输出用 `\n\\n` 分隔上下文；`unwrapClaudeResult` 依赖这个分隔符格式。如果未来 claude CLI 更改了输出格式（例如增加一个 wrapper JSON），整个解析器失效。

2. **单一归约行**：agent 可能在长输出末尾因 token 截断而无法生成完整的 `VERDICT:` 行。这种情况下 `parseReviewerVerdict` 返回 `""`，被调用方解释为"无裁决"→ 控制流失踪。

3. **大小写敏感**：`"VERDICT: APPROVE"` 全大写。如果 agent 输出 `"Verdict: Approve"` 或 `"VERDICT: Approved"`，解析器静默返回 `""`，收敛判据永远达不到。

4. **无备用路径**：如果 parse 返回空字符串（不可解析），`verdictLedger` 中就没有对应 entry。`reviewStatus(verdicts)`（gates.go）返回 `""` → `evalReviewStatus` 判 NOT MET。但这是因为解析失败而不是 agent 真实裁决——**operator 无法区分"解析失败"和"agent 没有输出裁决"**。

### 代码级证据：解析器用三个 fallback 层级处理同一输出

```go
// forge-core/cmd/forge/cost.go:440-480
func observeFor(name string, output string) {
    // 第一级：二元 reviewer 契约
    if v := parseReviewerVerdict(output); v != "" {
        l.ledger[name] = v
        return
    }
    // 第二级：五元 executive 契约
    if v := parseExecutiveVerdict(output); v != "" {
        l.ledger[name] = v  
        return
    }
    // 第三级：置信度契约
    if c := parseConfidenceScore(output); c > 0 {
        l.ledger[name] = fmt.Sprintf("conf=%d", c)
        return
    }
    // 全部失败 → 无记录（静默丢弃）
}
```

这三级 fallback 证明了系统意识到底层格式的不确定性。但**fallback 本身没有质量度量**：第一级成功解析比第三级成功解析更可靠吗？系统不知道，只是取第一个匹配的。

### 脆弱的根本原因

**输出契约不是真正的契约——它们是模式猜测**。真正的契约（"agent 卡声明了这个 agent 的输出格式"）存在于 `.agent/agents/*.md` 文件中，但解析器不读 agent 卡——它只是对所有 agent 的输出尝试所有解析器。

```go
// 实际解析路径（不查 agent 卡）:
output → unwrapClaudeResult → tryReviewer → tryExecutive → tryConfidence → 丢弃

// 应有的路径:
agent_card → 读取契约格式 → 选择对应解析器 → 解析
```

### 为什么需要

- **这不是理论风险**：`unwrapClaudeResult` 已经依赖 `\n\\n` 分隔符。如果 claude CLI 的一个更新改变了 JSON 输出结构（例如添加了 `"thinking"` 字段或 `"metadata"` 嵌套），每一行解析都会失效。真点火验证是在某一时刻通过的，但**没有回归测试保证下一次更新后仍然通过**。
- **随着更多 agent 卡加入机读契约，模式匹配的组合数增加**：每增加一个 `VERDICT:` 变体，要么扩展现有解析器（增加隐式耦合），要么加一个新的 fallback 层级（增加模糊性）。系统需要的是显式的契约选择而不是隐式的 fallback 链。
- **在无人值守的 24h 运行中**，解析失败 = 收敛失败 = 迭代重来 = 花费更多成本。operator 在事后检查 trace 时看到的只是 `convergence: NOT MET`，不知道是因为 gate 真实失败还是契约解析失败。

### 建议方向

**显式输出契约框架（Declared Output Contract Framework）**——将隐式行级模式匹配升级为 agent 卡声明的显式契约：

| 当前 | 建议 |
|---|---|
| `parseReviewerVerdict` 硬编码 "VERDICT: APPROVE" | 从 agent 卡读取契约格式：`contract: "VERDICT: APPROVE|REQUEST_CHANGES"` |
| `observeFor` 三级 fallback 静默丢弃 | 根据 agent 卡选择解析器：`parseFor(phase, card.ContractType)` |
| 解析失败静默返回 `""` | 解析失败 → 结构化的 `ParseError{Phase, ContractType, OutputSnippet}`，写入 trace 作为 `kind: "contract_parse_failed"` 事件 |

**架构变化**：

```
当前: output → (隐式 fallback 链) → verdict string → verdictLedger
建议: output → card.ContractType → ParserFor(card) → verdict (结构化的) → verdictLedger
                                        ↓ 失败
                                   ParseError → trace Event(kind="contract_parse_failed")
                                                  ↓
                                             operator 可在 trace 中 grep 发现
```

**最低成本 v1**：不改变解析器，但解析失败时**显式记录**到 trace（当前静默丢弃）。让 operator 至少知道发生了解析失败：

```go
// 在当前 "全部失败 → 无记录" 的地方加一行：
if v == "" && c == 0 {
    // 记录解析失败到 trace（非静默丢弃）
    tracer.Emit(trace.Event{
        Kind: "contract_parse_failed",
        Name: name,  // phase name
        Detail: fmt.Sprintf("phase=%s output len=%d first_line=%s", name, len(output), firstLine(output)),
    })
}
```

### 边界情况

- **假阴性 > 假阳性**：宁可解析失败（fallback 到 NOT MET）也不解析错误（误把 "not approved" 判成 APPROVED）。后者会导致系统认为已经收敛而实际没有。
- **v1 不做校验和**：v1 不需要让 agent 对输出加 checksum——那是 CS 论文级别，对实际系统过于沉重。v1 只要在解析失败时**不沉默**。
- **`unwrapClaudeResult` 依赖 `\n\\n`**：如果 claude CLI 更改输出格式，这个函数是最先断裂的地方。v1 可加一个 `assert` 检查：如果 unwrap 后的输出和原始输出完全相同（即 wrapper 格式不被识别），记录警告但不改变行为。

### 已有覆盖检查

`2026-07-11-forgeos-five-structural-capillary-gaps.md` 方向五（输出契约的「沉默退化」风险）是最接近的已有方向，核心论点高度重叠：都识别了行级文本匹配的累积脆弱性。**本文的差异点**：(1) 提供了 `observeFor` 三级 fallback 的具体代码路径和脆弱性分析；(2) 提出了最小的 v1 动作（解析失败时写入 trace，不沉默），不是全面重构；(3) 将问题与 24h 无人值守运行的成本浪费连接起来。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类型 | 影响范围 | 修复成本估计 | 一句话杠杆 |
|---|---|---|---|---|---|
| **一: cmd/forge 系统性包提取** | **P1** | 结构性债务 | cmd/forge 16 文件 | ~2–3 sprints | 消除单个包 >50% 文件濒临超限的系统性风险，使新功能提取有架构依据而非应急 |
| **二: 状态文件版本演化** | **P1** | 可靠性 | asset/persist/trace | ~1 sprint | 消除新增字段时零值语义静默改变的根因——"声明 vs 实现"漂移的系统性防护 |
| **三: .forge/ 运营积垢管理** | **P1** | 运维成熟度 | .forge/ 目录 | ~1.5 sprint | 跨存储 RunID + 磁盘配额 + maintain 命令让 24h 自治系统可长期运维 |
| **四: 输出契约声明化** | **P2** | 解析韧性 | cost.go 三层解析 | ~0.5 sprint (v1) | 解析失败从静默丢弃 → trace 可见，消除 24h 自治运行中"不收敛"的一个隐藏原因 |

### 收敛建议（若资源有限）

**优先级 A（必做）**：

- **方向一**：`cmd/forge` 的状况在 Sprint 29 已经导致过一次错误（`gate_resolve.go` 留在 cmd/forge 后才纠正）。在当前 16 文件 50% 濒临 500 行的状态下，下一次自然增长就会触发紧急拆分。**系统性的提取方案比应急拆分节省 2x 时间。**
- **方向三 v1（最小可落地子集）**：跨存储 RunID——仅增加一个 UUID 字段到 trace/memory/checkpoint，不涉及配额和 maintain 命令。成本 ~0.25 sprint，但为未来的数据生命周期管理奠定了基础，且立即解决"trace resume 后 seq 重复"的问题。

**优先级 B（高杠杆）**：

- **方向二**：版本校验的前置守卫——在 `cmdRun`/`cmdEvolve` 中检查 `Workflow` 的版本字段，如果版本低于引擎要求则报错而非静默加载。成本 ~0.3 sprint，消除最大的"无声数据损坏"风险源。
- **方向四 v1**：解析失败写入 trace。成本 ~0.1 sprint（一个函数 + 一个 trace event kind）。

**优先级 C（长期）**：

- **方向三 maintain 命令 + 磁盘配额**：依赖 RunID 基础设施就绪后，作为独立的运维工具开发。
- **方向四 显式契约框架**：依赖方向二（版本化）的基础设施——agent 卡需要有版本化的契约声明。

---

> **本文所有方向均来自 2026-07-11 的代码库真实状态扫描，每个断言都指向具体的 `file:line` 坐标。**
> 与 ~180 份已有 `docs/requirements/` + `docs/analysis/` 分析全文交叉验证，确认没有重复已有方向的核心论点。
> 不编写任何代码——仅做判断与路径设计。
