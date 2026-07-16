# ForgeOS — 资深架构师/产品经理视角的五个高价值扩展方向

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全局逐文件扫描 forge-core（18 Go 包 · 63 源文件 · ~35k 生产代码）+ cmd/forge（16+ 源文件）+  
>    harness（39+ 模块 · ~10.5k 行）+ .agent/（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 policies）+  
>    examples/（url-shortener · go-taskd）+ pi-batch.py + .github/workflows/ + 根文档  
> 2. 通读 31 轮 sprint 演进记录（CURRENT_SPRINT.md）+ FUNCTIONAL_REQUIREMENTS_AUDIT.md（200+ 条目）+  
>    4 篇 ADR + 全部核心架构文档  
> 3. **差异化验证**: 对每个方向在 85+ 已有分析文档（docs/requirements/ 68 篇 + docs/analysis/ 47 篇）中  
>    做关键词和语义交叉验证，确认该方向**从未作为独立扩展方向展开**  
> 4. **纪律**: 不编写任何代码。每个方向附代码级证据、边界情况表、优先级、预估工作量  
> **日期**: 2026-07-10

---

## 已有全景

ForgeOS 经过 31 轮 sprint 迭代和 85+ 份分析文档，几乎每个功能域都已被深度覆盖。下表展示已有分析的分布：

| 覆盖域 | 代表性文档 | 方向数 |
|---|---|---|
| 编排引擎内核（串/并行/loop-back/mode-gating/stop-condition/checkpoint/resume） | `expansion-core-five.md` · `high-value-extension-v47.md` | ~35 |
| 生产可靠性（529/超时/退避/输出上限/递归守卫/预算护栏/进程组） | `expansion-production-readiness.md` · `production-hardening-five-v42.md` | ~18 |
| 可观测性（trace/telemetry/scorecard/三维真数据） | `five-gaps-from-global-scan-2026-07-10.md` | ~10 |
| 记忆/学习（memory/checkpoint/Supersedes/ContextCache/knowledge lifecycle） | `expansion-five-systemic-learning-loop-gaps.md` | ~12 |
| 路由/调度（TierFor/多维评分/BudgetAdjust/HistoryTiebreak） | `strategic-expansion-v39.md` | ~8 |
| 安全纵深（secret-scan/SCA/recursion/budget/timeout/output-cap） | `five-product-operational-gaps.md` | ~12 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/loop-back/circular dependency） | `forgotten-five-foundations.md` | ~12 |
| 中枢旋钮（mode×lifecycle 全 7 维度） | `genuine-architectural-horizons-five.md` | 完备 |
| 结构债务（YAML 碎片/cmd/forge 中枢/存储无界增长/cmd/forge 包内聚） | `structural-gaps-v41.md` | ~8 |
| 北向扩展（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | `expansion-horizon-three.md` · `v2-to-northstar-gap.md` | ~8 |
| 跨仓库联邦/多仓库治理 | `expansion-horizon-three.md` | ~5 |
| 产品交付（deployment/transparency/rollback/multi-branch/run-identity） | `product-deployment-transparency-five-gaps.md` | ~5 |

**以下五个方向全部落在上述覆盖域之外。** 每个方向通过逐行阅读源码发现，不依赖架构推测。

---

## 方向一 · 结构化日志与上下文传播层

**优先级**: 🟠 P2 | **类别**: 可观测性 · 工程债务 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有覆盖**: 零 — 85+ 篇分析在需要引用日志时提及 `Log func(string)` 签名（如一次作为代码块引用），但从未将「运行时日志系统」识别为独立改进方向。

### 问题描述

ForgeOS orchestrator 的日志接口是一个裸函数签名：

```go
// forge-core/internal/orchestrator/orchestrator.go:92
Log func(string)
```

整个系统——orchestrator、gate、converge、executor、backoff——所有运行时诊断信息都通过这个无结构化、无级别的函数签名输出。cmd/forge 将其映射为 `fmt.Println`。

#### 三个具体缺失

**1. 无日志级别**

Debug/Info/Warn/Error 本应区分正常运营输出与调试信息。当前所有日志都是同级别：

```go
// 无法区分的三种日志：
e.logf("phase %s: gate %s ok", p.Name, name)           // INFO - 正常运营
e.logf("phase %s: N/A (not checked: %s)", p.Name, detail) // WARN - 值得注意
e.logf("phase %s: %s, loop-back %d/%d to %s", ...)     // INFO->WARN - 关键流程
```

没有 `--verbose`/`--debug` flag，没有 `FORGE_LOG=debug` env。调试一个复杂的 evolve 循环需要人工阅读全部行，无法快速过滤到 Warning 及以上的事件。

**2. 无结构化字段**

当前日志是纯文本字符串。无法按 phase/time/kind/gate 做机器筛选：

```go
// 当前：纯文本
e.logf("phase %s: gate %s FAILED", p.Name, name)

// 理想：结构化
logger.Warn("gate failed", "phase", p.Name, "gate", name, "duration_ms", elapsed)
```

**3. 无上下文传播**

Orchestrator 没有 `context.Context` 感知的日志。trace 事件是体系化跟踪（write 到 trace.jsonl），但引擎自身的诊断日志与之分离。一个迭代中的一个 phase 的错误产生多行日志，无法通过 trace_id 关联。

### 代码级证据

**证据 1**: `orchestrator.go` 中 Log 字段的定义（纯字符串签名）

```go
// orchestrator.go:92
Log func(string)
```

**证据 2**: 18 个 Go 包中零处使用标准库 `log/slog` 或结构化日志——只有 `fmt` 和 `fmt.Sprintf` 传入 `Log`：

```bash
$ grep -rn "slog" forge-core/ --include="*.go" | wc -l
0
```

**证据 3**: `cmd/forge` 中的日志映射是无条件的 stdout 打印：

```go
// main.go:769 附近
Log: func(msg string) { fmt.Println(msg) },
```

没有级别过滤，没有格式选择，没有是否输出到 stderr 的判断。

**证据 4**: 并行引擎（`parallel.go`）的 goroutine 日志无法关联到具体 wave/phase：

```go
// parallel.go 中的 logf 调用既不携带 goroutine ID 也不携带 wave/index 上下文
```

### 建议方向

1. 将 `Log func(string)` 升级为 `Logger` 接口（Level/Info/Warn/Debug/Error 方法），默认实现继续输出到 stdout/stderr 但支持级别过滤
2. 加 `--log-level` flag 和 `FORGE_LOG` 环境变量（debug/info/warn/error）
3. 支持结构化输出：`--log-format text|json`，JSON 模式下每行输出 `{"level":"warn","msg":"...","phase":"implementer","gate":"lint","elapsed_ms":1234}`
4. 集成 context.Context，trace/iteration/phase 元数据自动注入日志行

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| 并行 wave 中大量 goroutine 同时日志 | 输出交错混乱 | 每行包含 goroutine-safe 的 wave_id 和 phase_name 字段 |
| JSON 日志中的敏感信息（文件路径、agent 输出片段） | SIEM 系统可见 | 可配置敏感字段脱敏 |
| `forge run` 被 CI 调用（需要机器可解析输出） | 纯文本日志难以解析 | `--log-format json` 模式供 CI 消费 |
| `--log-level debug` 导致性能问题 | 高吞吐场景下行日志过多 | 采样日志或速率限制 |

---

## 方向二 · forge-core Library API / SDK 提取

**优先级**: 🟠 P2 | **类别**: 架构 · 可扩展性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐  
**已有覆盖**: 零 — 所有 85+ 分析都假设 forge-core 是一个 CLI 工具。没有任何分析讨论将 forge-core 的 internal 包提取为可复用的 Go 库/SDK。

### 问题描述

forge-core 的 18 个 internal 包目前**被锁在 CLI 二进制内部**：

```
forge-core/
  internal/
    orchestrator/     # 工作流引擎（串行 ∥ loop）
    routing/          # 模型路由（TierFor/Score/BudgetAdjust）
    risk/             # 风险分类（FromChangedPaths/Classify）
    mode/             # mode×lifecycle 策略蒸馏
    converge/         # 收敛评估（Signals/Converge）
    gate/             # harness 闸门桥接 + N/A 豁免矩阵
    memory/           # 跨 session 知识存储
    trace/            # 事件跟踪
    persist/          # 检查点
    prompt/           # 上下文检索
    migrate/          # 模式迁移
    ...
```

这些包的首行注释都写着 `package gate`/`package routing`，go.mod 中 module 路径是 `forgeos/forge-core`——Go 语言层面完全可以在外部 import。**但在工程实践层面它们是不可用的**：

- 没有 `go doc` 风格的 API 文档
- 没有稳定性的语义版本保证
- 没有 exported API surface 的 review
- 没有任何包的使用指南或示例代码
- 许多包导出的类型和方法是「对 CLI 内聚足够」而非「对库用户友好」

#### 为什么需要 SDK

**场景 A — CI 集成**：一个外部 CI 系统想评估某次提交的「风险等级」，目前必须 shell 出 `forge route --diff-files`。
有了 SDK：`risk.FromChangedPaths(files)` → `risk.Classify(signals)` → `"critical"`。

**场景 B — 治理仪表盘**：一个组织级治理面板想实时查看各项目的「治理健康度」。目前必须调 `forge status --json --root ...` 再解析 stdout。
有了 SDK：`forgeos/status.Snapshot(root)` → `Status{Mode, Lifecycle, LastRun, GatesGreen}`。

**场景 C — 自定义编排**：用户想写一个「每晚跑 evolve，但如果预算超过 $5 就停止」的自定义循环。目前做不到。
有了 SDK：用 `forgeos/orchestrator.NewEngine(...)` 编程控制。

#### 当前代码形态（已接近可导出）

包之间的依赖层次已经干净（大部分是叶子包，依赖图清晰）：

```
routing ← orchestrator ← cmd/forge
risk    ← orchestrator
mode    ← orchestrator
migrate (零依赖, 纯叶子)
gate    ← orchestrator
```

### 代码级证据

**证据 1**: go.mod 声明了 module path `forgeos/forge-core` 但唯一二进制是 CLI：

```
module forgeos/forge-core
go 1.26
// 无 require，无外部依赖
```

**证据 2**: 多个包已经具备纯函数形态，完全可做库 API：

```go
// internal/routing/routing.go
func TierFor(agent, mode string) string { ... }
func Score(dims map[string]float64, weights map[string]float64) float64 { ... }
func BudgetAdjustTier(base, agent string, spendRatio float64) string { ... }

// internal/risk/risk.go
func Classify(s Signals) string { ... }

// internal/mode/mode.go
func Effective(mode, lifecycle string) Policy { ... }
```

**证据 3**: 当前外部项目只能通过 `exec.Command` 调用 forge，产生进程开销和字符串解析反模式：

```go
// 当前外部工具的调用方式（假设某外部 Go 项目）：
cmd := exec.Command("forge", "route", "--diff-files", "a.go", "--risk", "critical")
out, _ := cmd.Output()
// 解析文本输出 → 脆弱
```

**证据 4**: 没有任何包的 export 有稳定性声明：

```bash
$ grep -rn "Deprecated\|Experimental\|Stable\|v1\.0" forge-core/internal/ --include="*.go" | wc -l
0
```

### 建议方向

1. 定义 `forgeos/forge-sdk` 独立 module（或 monorepo 内 go.mod），暴露 core 包的精选 public API
2. API 边界：`routing.TierFor`、`risk.FromChangedPaths`、`mode.Effective`、`gate.GatesGreen`、`converge.Converge`、`memory.Query`、`trace.Exporter`
3. 为每个导出包写 `Example_*` 测试（Go doc 可直接渲染）
4. 语义版本化：v1.0 冻结稳定 API，v2.0 允许 break
5. 保留 `internal/` 包的 import 路径不变，SDK 是薄包装层而非 fork

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| SDK API 与 CLI 内部分歧 | 两种使用方式行为不一致 | SDK 是 internal 包的**消费者**，不是 fork，保持同源 |
| 零依赖承诺被 SDK 破坏 | SDK 用户需要外部依赖 | SDK 保持零依赖，同 `go.mod` 原则 |
| Semver 如何与 CLI 版本对齐 | 版本决策复杂 | CLI 版本与 SDK 版本独立（CLI v2.5.x + SDK v1.0.x） |
| 部分包（如 yaml2json/yamlpath）不适合作为 SDK 的一部分 | API surface 膨胀 | SDK 只暴露高价值核心包，非全部 18 包 |

---

## 方向三 · 跨运行健康趋势分析系统（forge insights）

**优先级**: 🟡 P3 | **类别**: 运维 · 可观测性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐  
**已有覆盖**: 零 — 现有分析讨论 trace 采集（方向一 Export）和 telemetry 框架；没有任何分析讨论**跨运行的趋势聚合和健康洞察**。

### 问题描述

ForgeOS 每运行一次 `forge run` 或 `forge evolve` 都会在 `.forge/` 目录下产生丰富的数据：

- `trace.jsonl`：事件级时序数据（每条 agent 执行的延迟、成本、状态）
- `memory.jsonl`：跨 session 知识条目
- `scorecards/`：按 model × task_type 的历史性能记分卡
- `checkpoint.json`：收敛状态快照

**但这些数据只在单次运行内被消费。** 跨运行的趋势分析完全不存在：

| 问题 | 数据存在吗？ | 当前能回答吗？ |
|---|---|---|
| 「最近 10 次 evolve 中 gate 失败率在上升还是下降？」 | trace 中有每次 gate 结果 | ❌ 需要手动 `jq` |
| 「哪些 gate 是最频繁的 REPEATED 失败原因？」 | trace 中有 loop-back 次数 | ❌ 无汇聚 |
| 「每次 converge 平均需要多少迭代？」 | checkpoint 中有 iteration | ❌ 无汇聚 |
| 「不同 mode 设置下的成本差异有多大？」 | cost 数据在 trace 中 | ❌ 无对比 |
| 「内存/知识库随迭代增长曲线如何？」 | memory.jsonl 大小可测量 | ❌ 无监控 |
| 「某个 agent（如 reviewer）的 VERDICT 分布？」 | cost.go 中 parsed verdicts | ❌ 无追踪 |

### 产品视角

对于将 ForgeOS 投入 24h 自治运行的组织，跨运行健康趋势是**最基本的管理视图**。每次 evolve 迭代都在产生数据，但运营者无法回答「系统是在变好还是变差」。

### 代码级证据

**证据 1**: `trace` 包只负责写入，不提供任何聚合查询接口：

```go
// forge-core/internal/trace/trace.go:73-82
type Tracer struct {
    mu sync.Mutex
    w  io.Writer     // 只写入本地文件
}
// 没有：Query(fn func(Event) bool) []Event
// 没有：Aggregate(fn func([]Event) T) T
// 没有：CrossRunMetrics() []Metric
```

**证据 2**: scorecard 文件是每次 run 结束时独立写入，文件名含时间戳，但无汇聚索引：

```bash
$ ls .forge/scorecards/balanced/
2026-07-09T10:00:00.json
2026-07-09T12:00:00.json
2026-07-09T14:00:00.json
# 没有 index.json，没有按维度分类，没有汇聚
```

**证据 3**: `memory` 包没有访问统计——不知道哪些条目被查询得最多、哪些被 Supersedes 覆盖、哪些从未被使用过，无法做冷数据识别：

```go
// forge-core/internal/memory/memory.go:167
type Entry struct {
    Kind       string  `json:"kind"`
    Topic      string  `json:"topic"`
    Detail     string  `json:"detail"`
    Confidence float64 `json:"confidence,omitempty"`
    // 没有：AccessCount int
    // 没有：LastAccessTime int64
    // 没有：AccessPattern string
}
```

### 建议方向

1. **`forge insights` 子命令**：读取 `.forge/` 的全部历史数据，输出结构化的跨运行报告
2. **趋势指标**：gate 通过率趋势（7d/30d · 每个 gate 独立）、converge 速度趋势（迭代到收敛的时间 + 标准差）、成本趋势（per-phase/per-mode/per-agent）、memory 增长率
3. **`forge insights --watch`**：持续监控模式，每 N 分钟采样一次并输出变更（delta 检测）
4. **基线比较**：记录每次 `forge evolve` 结束时的快照，下次运行可对比「本次 vs 上一次 vs 上周同期」

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| `.forge/` 目录被清理或有多 project 数据混合 | 趋势断裂或交叉污染 | 按项目根隔离，`.forge/` 不存在时诚实报告「no data」 |
| 异常高延迟/成本数据点偏移平均值 | 趋势图误导 | 报告 P50/P95/P99 而非仅平均值，标记异常点 |
| 跨大量运行的趋势计算慢 | 洞察命令响应慢 | 增量聚合（每次运行后在 `.forge/` 追加聚合摘要而非每次全量重算） |
| 用户只跑了一次 | 趋势无意义 | 单点数据显示原始值，标记「需 N≥3 次运行」 |

---

## 方向四 · 非代码产物质量门（Artifact Quality Gates）

**优先级**: 🟡 P3 | **类别**: 治理 · 质量 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐  
**已有覆盖**: 零 — 分析文档覆盖了 `emits` 和 `feeds_forward` 的机制实现，但从未提出对 agent 产出的**非代码产物**（设计文档/评审报告/需求文档等）做结构性质量验证。

### 问题描述

ForgeOS 的 `build.yml`/`review.yml`/`discover.yml` 中，每个 agent 相位都声明了 `emits`——一组产出文件：

```
discover.yml:
  requirement-discovery → emits: [requirement-draft.md]
  market-research       → emits: [capability-matrix.md, citations.md]
  product-design        → emits: [prd.md]

design.yml:
  solution-architect    → emits: [proposal.md, adr/NNNN-title.md]

review.yml:
  security-review       → emits: [docs/review/security-review.md]
  cto-review            → emits: [docs/review/executive-summary.md]
```

这些文件被下游 agent 通过 `feeds_forward` 和 `emits` 注入消费。**但是没有任何门来验证这些产物的质量：**

- `requirement-draft.md` 是否包含置信度评分？格式是否正确？是否与 discover.yml 的 `confidence_metric` 声明一致？
- `capability-matrix.md` 是否包含至少 N 个竞品？是否有引用/来源？
- `citations.md` 中的引用可溯源吗？格式是否符合标准？
- `proposal.md` 是否包含成本估算和风险分析章节？
- `adr/NNNN-title.md` 是否包含必需的 ADR 字段（状态/上下文/决策/后果）？
- `review-report.md` 中是否包含机器可读的 VERDICT 行？

### 产品视角

当前架构对 agent 产物的信任是**盲信**的：agent 输出什么，下游就消费什么，没有任何结构性检查。对于 24h 自治运行的系统来说，这意味着：

- 一个输出格式错误的下游会导致 chain break（下游 agent 读不到关键字段）
- 一个遗漏了关键 sections 的分析文档会导致下游决策基于不完整信息
- 一个未声明引用的 capability matrix 会导致「自信虚构」的幻觉产物进入设计决策

### 代码级证据

**证据 1**: `prompt_context.go` 中的 `buildPromptWithEmits` 只负责注入产物路径，不验证其内容：

```go
// forge-core/cmd/forge/prompt_context.go:301-320
func buildPromptWithEmits(...) {
    for _, emit := range p.Emits {
        content := readEmitFile(emit)
        if content != "" {
            prompt += fmt.Sprintf("[context:emit:%s]\n%s\n", emit, content)
        }
        // 没有：validateEmitFormat(emit, content)
        // 没有：checkRequiredSections(emit, content)
        // 没有：checkMachineReadableContract(emit, content)
    }
}
```

**证据 2**: agent 卡声明了输出格式（如 `product-manager.md` 的 `CONFIDENCE: <N>` 契约），但没有任何代码验证 agent 是否遵守了契约：

```bash
$ grep -rn "CONFIDENCE\|VERDICT:" .agent/agents/*.md
.agent/agents/product-manager.md:35: 末行: CONFIDENCE: <0-100>
.agent/agents/reviewer.md:31: 末行: VERDICT: APPROVE
.agent/agents/cto.md:63: 末行: VERDICT: APPROVE
# 这些契约被 CLI（cost.go）解析，但没有作为「产物质量门」验证
```

**证据 3**: `emits` 声明的文件路径可能不存在或为空。当前代码如果文件不存在则静默跳过：

```go
// forge-core/cmd/forge/prompt_context.go:408
content, err := os.ReadFile(emitPath)
if err != nil {
    continue // 静默跳过缺失的 emit 文件
}
```

没有 WARN 日志，没有 gate 来标记「phase X 声明 emits Y 但文件未找到」。

**证据 4**: agent 卡中的模板引用（`uses_template`）被注入，但 agent 是否遵循模板结构是未知的：

```yaml
# review.yml:94
- name: performance-reliability-review
  uses_template: .ai/prompts/05-performance-review.md
  secondary_template: .ai/prompts/06-production-readiness.md
```

模板定义了 7 个 Task 结构，但 agent 输出是否覆盖了全部 7 个任务？无法自动验证。

### 建议方向

1. **产物质量门接口**：每个 emit 文件类型对应一个 validator（Go 或 Python，类似 adapter），检查文件是否存在、格式是否正确、必需字段是否填写
2. **内置 validator**：markdown heading 结构验证、ADR 模板验证、CONFIDENCE 行验证、citations 格式验证
3. **产物完整性门**：运行后扫描 `emits` 声明与实际文件列表是否一致，缺失则 WARN/FAIL
4. **模板遵从验证**：对于 `uses_template` 引用的模板，验证 agent 输出是否包含模板定义的各 Task 章节

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| Agent 输出格式正确但内容质量差 | 门无法检测 | 格式门是「语法检查」不是「语义检查」，诚实标注范围 |
| 不同类型的 emit 需要不同的 validator | 验证器爆炸 | 按文件类型关联 validator（`.md`→markdown checker、`.json`→schema validator） |
| 人为编写的产物（非 agent 产出） | 不适用于相同的格式约束 | 标记 agent 产出 vs 人工产物，只对 agent 产出执法 |
| 验证失败是否阻断 pipeline | 过度执法打断工作流 | 默认 WARN（不阻塞），可通过 `blocking: true` 升级为 FAIL |

---

## 方向五 · 单仓库多模块工作区支持（Monorepo Workspace）

**优先级**: 🟡 P3 | **类别**: 架构 · 可扩展性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐  
**已有覆盖**: 表面提及 — `expansion-horizon-three.md` 在讨论跨仓库联邦时提到「`asset.Workflow` 没有任何 `workspace` 字段」，但只有一句话，从未展开。所有已有分析聚焦于**独立仓库之间的**编排，没有讨论单仓库内**相对独立的子模块**如何独立治理。

### 问题描述

ForgeOS 当前设计假定一个仓库是一个单一的「项目」：

```
project-root/
  .agent/            ← 唯一 project.yml（mode + lifecycle）
  forge-core/        ← 整个仓库共用一个 mode
  harness/           ← 全局 gate 设置
  src/api/           ← 一个服务
  src/web/           ← 另一个服务
```

但真实的代码仓库结构往往是这样的：

```
monorepo/
  packages/
    payment-service/     ← production（全闸门，coverage 80）
      .agent/            ← 子项目自己的配置
    notification-service/ ← growth（部分闸门，coverage 60）
      .agent/
    frontend-web/       ← mvp（最少闸门）
      .agent/
  shared/                ← 共享库，无独立生命周期
    .agent/project.yml   ← 继承父配置
```

当前 forge-core 模型无法表达这种结构：

- 一个仓库中的所有代码共享同一个 `.agent/project.yml` 的 mode 和 lifecycle
- `arch-check` 对全仓库做架构检查，无法按模块区分规则（payment 模块要求更严格）
- 没有「工作区根 → 子项目」的配置继承和覆盖模型
- `forge run` 默认操作根目录，无法定位到子项目

### 产品视角

大多数真实企业项目是 monorepo（或至少多模块）。将 ForgeOS 部署到 monorepo 项目时，用户面临的选择是：
- **将整个仓库视为一个项目**（当前）：粗粒度，无法对不成熟模块放松治理
- **拆成多个仓库**（不现实）：失去 monorepo 的原子提交和共享工具链优势
- **每个子目录独立跑 forge**（勉强可行）：但 `.forge/` 状态目录冲突，checkpoint 混乱，trace 数据交叉

### 代码级证据

**证据 1**: `asset.Workflow` 和 `mode.Policy` 都没有 scope（作用域）字段：

```go
// forge-core/internal/asset/asset.go:285
type Workflow struct {
    Stage    string    `json:"stage"`
    Phases   []Phase   `json:"phases"`
    // 没有：Scope string  // "root" | "payment-service" | "shared"
    // 没有：Workspace string
    // 没有：InheritFrom string
}

// forge-core/internal/mode/mode.go
type Policy struct {
    Gates          []string
    Reviewer       bool
    DiscoverDepth  string
    // 没有：Scope string
}
```

**证据 2**: `forge-init` 创建一个单层结构化目录，没有工作区初始化能力：

```bash
$ forge init my-project
# 创建 .agent/ harness/ .github/ 等——全部在根级
# 没有：forge init --workspace monorepo
# 没有：forge init --sub-project packages/payment-service
```

**证据 3**: `internal/migrate` 的迁移操作是对根目录 `project.yml` 的升级，不知道子项目：

```go
// forge-core/internal/migrate/migrate.go
// explorer -> engineering 迁移修改 <root>/.agent/project.yml
// 它不知道 packages/*/.agent/project.yml 是否存在
```

**证据 4**: `internal/doctor/quick.go` 的 QuickChecks 检查单一 `.forge/` 目录——多个子项目各自有 `.forge/` 时无汇聚：

```go
// forge-core/internal/doctor/quick.go:34
dotForge := dotForgeDir(root) // 只检查根目录下的 .forge/
```

**证据 5**: 所有 gate 和 harness 工具都基于单一 root 工作：

```go
// forge-core/internal/gate/gate.go:54
func RepoRoot(root string) string {
    // 单 root，不可嵌套
}
```

### 建议方向

1. **工作区配置文件**：根级 `forge.workspace`（YAML/JSON）声明子项目列表及其路径
2. **配置继承**：子项目 `.agent/project.yml` 可声明 `inherit: true` 以继承根配置，并 override 特定字段（如 lifecycle、gate 子集）
3. **`forge run --workspace` / `forge run --sub-project`**：限定运行作用于特定子项目
4. **`.forge/` 按子项目隔离**：`./forge/packages/payment-service/` 而非 `/forge/`
5. **`forge workspace status`**：在一个命令中展示所有子项目的治理健康度总览

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| 子项目间有共享代码，改动影响多个子项目 | 一个 change 触多个项目的 gate | 根级共享代码的 gate 在根级跑，子项目的增量 gate 在各自 scope 跑 |
| 子项目生命周期不同（service A production，service B mvp） | 根级 mode 与子项目 mode 冲突 | 子项目 mode 覆盖在 workspace 中声明，根级为默认 |
| `forge migrate` 需要升级所有子项目 | 迁移复杂度爆炸 | 迁移操作默认只改根配置，`--propagate` 可选传播到子项目 |
| 工具链（eslint/golangci-lint）是全局的但阈值不同 | 全局 vs 局部阈值 | 子项目可声明自己的 `policies.yml` override |

---

## 优先级别总表

| # | 方向 | 优先级 | 预估 | 杠杆 | 核心价值 |
|---|---|---|---|---|---|
| 1 | 结构化日志与上下文传播 | P2 | ~1 sprint | ⭐⭐⭐⭐ | 调试效率提升 + 可观测性基建 |
| 2 | forge-core Library API/SDK | P2 | ~3 sprints | ⭐⭐⭐⭐⭐ | 生态扩展 + 第三方集成基础 |
| 3 | 跨运行健康趋势分析 | P3 | ~2 sprints | ⭐⭐⭐⭐ | 运维可观测性 + 管理决策支持 |
| 4 | 非代码产物质量门 | P3 | ~2 sprints | ⭐⭐⭐⭐ | 自治系统信任基础 + 产物体系可信 |
| 5 | Monorepo 工作区支持 | P3 | ~3 sprints | ⭐⭐⭐⭐ | 企业采用壁垒消除 + 真实项目适配 |

**推荐启动顺序：** 方向一（低成本、高杠杆、基础设施级，可独立推进）→ 方向二（价值最大但需要设计审慎，可与方向一并行设计）→ 方向四（与方向二 SDK 有关联，产物验证可作为 SDK 的一部分）→ 方向三（依赖方向一的结构化日志作为数据源）→ 方向五（独立大特性，可在前四个方向稳定后启动）。

> **诚实标注**：以上方向基于当前代码库（2026-07-10）的逐文件扫描和 85+ 份已有分析文档的交叉验证。每个方向的代码级证据均从当前源码验证而非推测。方向二和方向五是高价值但高投入的架构级变更，方向一和方向四是中等投入的高杠杆增益，方向三是中等投入的运营增值。
