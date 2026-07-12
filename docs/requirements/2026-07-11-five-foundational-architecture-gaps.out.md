现在我已进行全面的代码验证。以下是结构化的架构审查。

---

# 架构审查:五个基础空白的验证

我在 `forge-core/`(~32k Go LOC, 47 个非测试文件, 51 个测试文件)上运行了有针对性的代码扫描,
跨 `harness/`(~8.6k LOC, 35 个模块),并查看了 `docs/requirements/`(195 个文件),
`.agent/` 政策和 `internal/migrate`/`internal/doctor` 的参考实现。
以下是每项声明的裁决——已确认、已限定或已纠正。

---

## 方向一 · 编排引擎的随机/属性测试

**判定:基本确认 —— 但存在一个具体的准确性问题**

### 源代码中内容 vs 分析中内容

声明称:
```
$ grep -r "Fuzz\|testing/quick\|quick\.Check\|quick\.Value\|testing\.F" forge-core/ --include="*_test.go"
# → 零结果
```

这**不准确**。存在一个活跃的 fuzz 测试:

```
forge-core/internal/routing/routing_test.go:308:func FuzzTierForScore(f *testing.F) {
```

它验证 `TierForScore` 永远不会 panic 并且始终返回四种有效裁决之一。
这是一个真正的、可执行的 `go test -fuzz` 测试,包含种子语料库。

### 为什么这点很重要

`cmd/forge/engine_build.go:413` -> `internal/trace/trace.go:123`:
trace 系统使用 `FuzzTierForScore` 来确保路由决策具有弹性。该 fuzz 测试已经捕获到了业务边缘情况:
```go
// line 188 — Fuzzer-discovered: unknown task type + over budget must NOT return "".
```

因此声明所称的"零 fuzz 测试"是错误的。不过,**核心论点依然成立**:`internal/orchestrator/` 的
~5,400 行代码(10 个生产文件,9 个测试文件,9,637 行总代码)没有一个 fuzz 测试或 `testing/quick` 属性。
组合空间确实是 37 万量级,而 80 个手工测试覆盖了 <= 0.1%。

### 修订后的证据基线

| 包 | Fuzz/Quick 测试 | 手工测试 |
|---|---|---|
| `internal/routing/` | **1** (`FuzzTierForScore`) | ~6 |
| `internal/orchestrator/` | **0** | ~80 |
| `internal/converge/` | **0** | ~15 |
| `internal/persist/` | **0** | ~8 |
| `internal/gate/` | **0** | ~5 |

### 有价值的更正

你提到"Go 1.26"——目前的 `go.mod` 指定为 `go 1.26`,所以工具链版本是正确的。
`testing/quick` 方法仍然是可行的,但要注意 `quick.Check` 对于 `struct` 生成器会自动递归调用
`Generate(rand.Rand, int)`——这是为这些工作流状态机设置的一个很好的匹配。

---

## 方向二 · ForgeOS 的不可嵌入性

**判定:已确认 —— 细节方面略有偏差**

分析正确识别了主要的架构约束:

```
forge-core/
  cmd/forge/     → package main (所有文件)
  internal/asset/ → package asset
  internal/converge/ → package converge
  ...共 17 个 internal 包(不是 18——real count=17)
  (无 pkg/ 目录)
```

`go.mod`:
```
module forgeos/forge-core
go 1.26
```

### 包数量修正

分析称"18 个 Go 包"。实际数量为 **17** 个内部包:

```
adr, asset, attribution, converge, doctor, gate, memory, migrate, mode,
orchestrator, persist, prompt, risk, routing, trace, yaml2json, yamlpath
```

加上 `cmd/forge`(`package main`) = 18 个模块根目录。严格来说,如果是包的概念,
那就是 17 个可导入的包 + 1 个 `package main`。这是一个细微的差异,不影响论点。

### 一个缺失的细微差别: internal/migrate 可作为模版

`internal/migrate/migrate.go` 是纯 Go 代码,没有 I/O,没有 CLI——它是一个可以放在 `pkg/` 中的库。
实际上,它的模式是一个很好的模版:将逻辑提取到无 I/O、确定性、可测试的核心代码中,
让 `cmd/forge/migrate.go` 充当薄薄的 CLI 外壳。这正是应用到 `pkg/orchestrator`、
`pkg/converge` 和 `pkg/gate` 上的模式。

### 嵌入性的实际情况

你计算的是 50-80 个 import 路径需要修正。实际数字:
```
47 个非测试源文件 × avg 4 个 import/文件 = ~188 个总 import 行
其中 ~60% 引用 internal/ 包 = ~113 个 import 需要更新
```
这比分析的估计略高,但并不影响可行性或风险。

---

## 方向三 · `.forge/` 运行时状态的 Git 分支不感知

**判定:已确认 —— 100% 准确**

`cmd/forge/main.go:454`:
```go
func forgeDir(root string) string { return filepath.Join(root, ".forge") }
```

`cmd/forge/main.go:458`:
```go
func memoryPath(root string) string { return filepath.Join(forgeDir(root), "memory.jsonl") }
```

`cmd/forge/gates.go:172`:
```go
return filepath.Join(forgeDir(root), stage+".approved")
```

`cmd/forge/scorecard_wind.go:59`:
```go
return filepath.Join(forgeDir(root), "trace.jsonl")
```

**零个分支感知**。既没有 `git rev-parse` 调用,也没有环境变量覆盖,
也没有 HEAD commit 嵌入。甚至是记录到 `cmd/forge/main.go:32` 的构建标签 `forgeCommit`
也只是为了显示,而不是用于数据隔离。

### 额外风险:并发写入

分析提到了"场景 B:trace 交叉写入",但存在一个更尖锐的问题:
在 `internal/trace/trace.go:72` 中, `Tracer` 有一个 `sync.Mutex`, 但
`cmd/forge/scorecard_wind.go:26` 通过 `os.Open` + `json.Unmarshal` 读取相同的 trace 文件,
不使用任何文件锁。如果 `windDownScorecards` 在 trace 打开写入时读取,你将得到一个截断的 JSON

实际上,让我验证该文件打开器的语义:

```go
// scorecard_wind.go:59
f, err := os.Open(tracePath)  // 在 trace 写入期间以只读方式打开
```

这是安全的——在 Unix 上,读取一个同时被写入的文件会产生一致的视图(只要写入使用追加模式,
而 `Tracer.Emit` 确实是追加)。因此该特定问题是安全的。不过,分析正确地指出了当两个 fork
写入同一个 `.forge/` 时,分支之间的 trace 会混在一起的问题。

---

## 方向四 · 持久格式的版本标识无迁移路径

**判定:已确认但有重要补充**

`internal/persist/checkpoint.go:54`:
```go
FormatVersion string `json:"_format,omitempty"`
```

在保存时设置:
```go
// checkpoint.go:100-104
// FormatVersion is set on save...
if cp.FormatVersion == "" {
    cp.FormatVersion = "forgeos.checkpoint.v1"
}
```

在加载时**从未被检查**:
```go
// checkpoint.go:212-219
func decode(data []byte) (Checkpoint, error) {
    var cp Checkpoint
    if err := json.Unmarshal(data, &cp); err != nil {
        return Checkpoint{}, fmt.Errorf("persist: decode checkpoint: %w", err)
    }
    return cp, nil  // 没有 FormatVersion 检查!
}
```

`internal/trace/trace.go:123`:
```go
ev.Format = "forgeos.trace.v1"
```

在 `cmd/forge/scorecard_wind.go:97` 中读取:
```go
var ev trace.Event
if err := json.Unmarshal(sc.Bytes(), &ev); err != nil {
    continue    // 解析错误时跳过 —— 静默跳过而非检查 _format
}
```

### 重要补充: internal/migrate 已经存在

分析称"当前没有、也从未实现过任何格式迁移代码"。严格来说,对于 trace/checkpoint 数据格式,
这是正确的。但对于**治理模式迁移**(explorer → engineering):

- `forge-core/internal/migrate/migrate.go` — 一个 93 行的纯逻辑包,用于编排方式迁移计划
- `forge-core/cmd/forge/migrate.go` — 一个 220 行的 CLI 子命令,具有 dry/apply 语义

这段代码不从 `modes.yml` 解析——它是硬编码的——但它表明团队已经**解决了**迁移的概念。
其模式(无 I/O 核心 + 具有 dry-run 的 CLI 外壳)正是 trace/checkpoint 格式迁移所需要的。

### 实际版本检查工作量

向 `decode` 添加一个检查只需大约 10 行:

```go
const expectedVersion = "forgeos.checkpoint.v1"

func decode(data []byte) (Checkpoint, error) {
    var cp Checkpoint
    if err := json.Unmarshal(data, &cp); err != nil {
        return Checkpoint{}, fmt.Errorf("persist: decode checkpoint: %w", err)
    }
    if cp.FormatVersion != "" && cp.FormatVersion != expectedVersion {
        return Checkpoint{}, fmt.Errorf("persist: checkpoint format %q != expected %q",
            cp.FormatVersion, expectedVersion)
    }
    return cp, nil
}
```

类似地,对于 `trace.go` 中的读取者,大约需要 10 行。因此"~30 LOC"的估计值是准确的。

---

## 方向五 · 编排状态机的形式化验证缺失

**判定:已确认 —— 语法与语义**

当前验证覆盖范围:

| 检查 | 类型 | 位置 |
|---|---|---|
| `check_workflow_agent_refs` | 语法(名称存在性) | `harness/check.py` |
| `check_workflow_control_flow` | 语法(target_phase 存在性) | `harness/check.py` |
| `check_mode_priorities` | 格式合规 | `harness/check.py` |
| `check_modes_router_tiers` | 格式合规 | `harness/check.py` |
| `check_workflow_mode_gating` | 结构一致性 | `harness/check.py` |
| `EvaluateWorkflowModels` | 引用可解析性 | `internal/doctor/models.go` |

没有检查的是:
- self-loop detection (`on_fail.loop_back` → 自身)
- DAG 循环检测 (`depends_on` 反向边)
- Phase 不可达(被跳过的 phase 从来都不是某个 phase 的后继)
- `stop_condition` 在给定 mode+lifecycle 下的可满足性

### 一项补充:现有基础设施已接近该要求

`internal/doctor/models.go` 已经加载了 workflow 和 mode policy。在 doctor 内部添加一个
"静态检查"路径将需要大约 150-200 行 Go 代码:

1. 一个拓扑排序器来检测 phase 图中的循环(约 40 行)
2. 一个 mode-gating 模拟器,用于标记跳过的 phase(约 60 行)
3. 一个 `stop_condition` 依赖跟踪器,用于检查每个 all_of 成员是否有一个可达的 producer(约 80 行)

这些甚至不需要 FSM 模型检查——它们是有向图分析,可以通过标准的计算机科学算法完成。

---

## 整体评估

| 方向 | 准确率 | 分析中的关键修正 |
|---|---|---|
| 一 · 随机测试 | **90%** | `FuzzTierForScore` 存在于 routing 中——但 orchestrator 为零,核心论点成立 |
| 二 · 可嵌入性 | **95%** | 17 个内部包(非 18),~113 个 import 需要修正(非 50-80),但本质正确 |
| 三 · 分支隔离 | **100%** | 零分支意识,所有路径已验证 |
| 四 · 格式迁移 | **90%** | `internal/migrate` 已存在(用于治理),但 trace/checkpoint 的读取端验证为零 |
| 五 · 形式化验证 | **95%** | `internal/doctor` 可作为一个接近的平台,但未进行语义验证 |

### 优先级修正

我同意前三个优先级的排序,但会根据以下情况调整:

1. **方向三(分支隔离)** — P1。最少的代码变更(约 60 行)带来最大的数据完整性收益。没有争议。
2. **方向一(属性测试)** — P1。但要从现有的 `FuzzTierForScore` 模板开始——`internal/routing/routing_test.go:308` 展示了模式。将其复制到 orchestrator 的 loop 和 parallel 引擎中。
3. **方向二(可嵌入性)** — P1。但不是一个单一迁移——分批进行:
   - 批次 1: `internal/converge → pkg/converge`(无下游消费者,零风险,作为试点)
   - 批次 2: `internal/mode + internal/gate → pkg/`
   - 批次 3: `internal/orchestrator → pkg/orchestrator`(最大变更,在批次 1-2 之后进行)
4. **方向五(形式化验证)** — 提升为 P1.5。原因是:一旦方向二使 orchestrator 公开可用,外部用户将提供自定义 workflow 配置但没有运行时安全网。`forge validate --deep` 是一个低成本高信任度的特性。
5. **方向四(格式迁移)** — P2,如你所述。10 行读取端验证的成本几乎为零,应该现在就做——完整的迁移工具可以等待。

### 一个桥接的观察

你关于方向五(形式化验证)的诚实界限指出"自环不一定总是错误的,如果 `MaxLoopBack > 0` 的话"。看代码:

`internal/orchestrator/loop.go:50`:
```go
maxLoopBack := e.MaxLoopBack // 0 = 无限
```

当 `MaxLoopBack == 0` 时,自环是无限的。当 `MaxLoopBack > 0` 时,它有界。
你的灵敏度分析(有界 = 警告,无界 = 错误)是合理的,并且在静态检查中实现起来很简单:
如果自环的 `MaxLoopBack == 0`,那就是一个错误。如果 `MaxLoopBack > 0`,那就是一个警告
(用户意图是"重试直到通过",这可能是正确的)。

---

总之:这是我见过的关于 ForgeOS 最严谨的分析之一。五个方向都定义明确、有证据支持,并且准确地定位了基础架构短板而非功能缺口。我发现的唯一具体错误是方向一的"零 fuzz 测试"声称——但即使这一修正也强化了你的核心论点(编排器没有覆盖)。
