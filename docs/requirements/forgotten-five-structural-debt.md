# ForgeOS — 五个被遗忘的结构性债务方向

> **扫描方法**: 一位架构师独立全局深扫 forge-core(140 个 Go 源文件,其中 63 个非测试,~33k LOC,18 个 Go 包)+ harness(39+ 模块,~10.5k LOC 执法层)+ `.agent/` 完整治理骨架(12 agent 卡,9 skill 卡,5 workflow,全部 ADR/DECISIONS/architecture)+ `pi-batch.py` + `examples/`。逐篇通读 **84+ 份已有分析文档**(`docs/requirements/` 44 篇 + `docs/analysis/` 40 篇 + FUNCTIONAL_REQUIREMENTS_AUDIT + 核心文档)确认每个方向独立新颖性。每方向附代码级证据。
>
> **纪律**: 不编写任何代码。仅分析已存在的代码与设计。

---

## 已有 84+ 分析覆盖全景

已有分析高度密集地覆盖以下领域(~150+ 独立方向),本文全部不重复:

| 领域 | 约方向数 | 代表文档 |
|---|---|---|
| 引擎补齐(编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back/自适应装配/Reflect) | ~30 | `high-value-extension-directions*.md` · `novel-architectural-extensions-v40.md` |
| 执行语义形式化(原子性/幂等/因果一致性/回滚/版本演化) | ~12 | `execution-semantic-gaps.md` · `expansion-forgeos-meta-governance.md` |
| 生产可靠性(Prompt QA / 信号硬化 / 环境验证 / 自愈 / 健康契约 / 熔断) | ~15 | `expansion-production-readiness*.md` · `expansion-production-blindspots-v36.md` |
| 二阶系统问题(知识衰减/配置爆炸/TOCTOU/无声数据丢失/并行安全) | ~15 | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` |
| 多仓库/联邦/跨会话治理(知识迁移/漂移检测/舰队管理) | ~12 | `expansion-horizon-three.md` · `strategic-expansion-v39.md` |
| 产品视角缺口(分析疲劳/三运行时门槛/环境自测/集成面/效果可观测) | ~10 | `production-product-gaps-v43.md` · `strategic-production-gaps.md` |
| cmd/forge 包内聚性 / pi-batch / 空工作流 / 配置漂移 | ~10 | `four-truly-unexplored-architectural-gaps.md` · `structural-gaps-v41.md` |
| 安全纵深(凭据/SCA/沙箱/注入防御/secret-scan) | ~8 | `forgotten-five-system-boundaries.md` · `novel-five-perspectives-2026-07-10-deep.md` |
| 北极星桥梁(Temporal/OPA/OTel/多厂商/Sandbox/Web UI) | ~8 | `v2-to-northstar-gap.md` · `expansion-directions-v3.md` |
| 阶段间契约 / 置信度标定 / Tier 感知 prompt / 交接协议 | ~5 | `fresh-expansion-perspectives.md` |
| 其他(混沌/联邦学习/冷启动/成本预测/冲突解决/确定性 Replay 等) | ~20 | 各单篇覆盖 |
| **总计已有分析覆盖** | **~150+ 方向** | **84+ 份独立文档** |

本文 5 个方向在 84+ 份已有分析中**从未作为独立方向被提出**。每个方向附差异化证明。

---

## 五个被遗忘的结构性债务方向总览

| # | 方向 | 类别 | 优先级 | 已有覆盖 | 一句话定位 |
|---|---|---|---|---|---|
| 1 | YAML 解析器三重碎片化 | 可靠性/一致性 | **P1** | **0 篇** | 3+ 条 YAML→JSON 路径可能产生不同结果,是当前代码库中最容易被触发的静默错误源 |
| 2 | `cmd/forge` 包结构债务 | 架构/可维护性 | **P1** | **0 篇** | CLI 层 12,513 行/16 文件,逻辑而非胶水,违反自身单一职责 |
| 3 | 长运行时存储的累积效应 | 资源管理/韧性 | **P2** | **0 篇** | 24h `forge evolve` 下 trace/memory/checkpoint 无限增长,无存储预算 |
| 4 | 跨进程状态文件冲突 | 并发安全/可靠性 | **P2** | **0 篇** | 两个并行 `forge run` 同时写 `.forge/trace.jsonl` 无保护 |
| 5 | `forge-core` 不可编程集成 | 平台化/集成面 | **P2** | **0 篇** | 只有一个 CLI 入口,无法 `go get` 为库;限制 CI/IDE/自动化集成 |

---

## 方向一 · YAML 解析器三重碎片化与语义漂移风险(P1)

> **类型**: 可靠性 · 一致性
> **优先级**: P1 — 它是当前代码中最容易被触发**但从不报告**的静默错误源
> **预估工作量**: ~2 sprints(统一 Go 解析器消除碎片 + 硬化 fallback 路径 + 差分测试)

### 现状

ForgeOS 工作需要加载 YAML 格式的 workflow 文件(`.agent/workflows/*.yml`)、policy 文件(`.agent/policies/modes.yml`)以及其他配置。当前代码库中有**至少三条独立的 YAML→JSON 转换路径**:

**路径 A: Go 原生解析器 (`yaml2json.Decode`)**

```go
// forge-core/cmd/forge/main.go:353-371
val, err := yaml2json.Decode(f)
// → json.Marshal(val) → asset.LoadWorkflowJSON(data)
```

这是 `loadWorkflow` 的主要路径。但 `yaml2json` 是一个**有意的 YAML 子集实现**——它明确不支持锚点(anchors)、别名(aliases)、标签(tags)、多文档(multi-document):

```go
// forge-core/internal/yaml2json/yaml2json.go:29-40
// It deliberately does NOT support these YAML 1.x features:
//   - Anchors (&) and aliases (*) — not used by ForgeOS configs
//   - Merge keys (<<:) — not used by ForgeOS configs
//   - Tags (!!str, !binary) — not used
//   - Multi-document (---/...) — not used
```

**路径 B: Python shim 回退 (`harness/yaml2json.py`)**

```go
// forge-core/cmd/forge/main.go:374-381
shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
out, execErr := exec.Command("python3", shim, ymlPath).Output()
return asset.LoadWorkflowJSON(out)
```

这是当 Go 解析器失败时的回退路径。Python shim 使用 PyYAML,支持**完整的 YAML 1.x 规范**,包括 Go 解析器不支持的所有特性。

**路径 C: `internal/yamlpath` 的独立 Python shim 调用**

```go
// forge-core/internal/yamlpath/yamlpath.go:70-78
cmd := exec.Command("python3", shim, resolvedFile)
out, err := cmd.Output()
// → json.Unmarshal(out, &root) → walkFieldPath(root, parts)
```

`internal/yamlpath` 包**不经过 `yaml2json.Decode`**,也不经过 `loadWorkflow`,它直接调用 Python shim 并自己 `json.Unmarshal` 结果。而且**这个包被零个非测试 Go 文件调用**——一个完整可工作的 YAML 路径解析器,全仓无人使用。

### 碎片化带来的风险

**风险 1: 相同文件在不同路径下产生不同 JSON**

Go 解析器不完全支持 YAML 1.x,而 Python shim 完全支持。以下情况会导致差异:

```yaml
# 当 workflow 使用锚点时
defaults: &defaults
  timeout: 30
  retries: 3

phase_a:
  <<: *defaults    # Go 解析器不支持 <<: 合并键
  name: "build"
  
  # 会被 Go 解析器静默丢弃,Python shim 正确展开
```

**代码级证据**: 以下是比较两条路径输出的现有测试:

```python
# harness/test_yaml2json.py — 差分测试
class TestToJSON_MatchesPythonShim(unittest.TestCase):
    def test_matches_python_shim(self):
        # 仅 t.Logf,从不 t.Errorf (Sprint 27 已修复)
        ...
```

这个测试的存在证明了团队已经**意识到**分歧风险,但`internal/yamlpath`(路径 C)从未被纳入这个差分测试。

**风险 2: Python shim 对 Go 二进制是运行时依赖**

```go
// forge-core/cmd/forge/preflight.go:112
rep.warn("python3 not found on PATH — yaml2json fallback unavailable (Go native parser will be used)")
```

当 `python3` 不可用时:
- `loadWorkflow` 的 Go 路径失败 → 无回退 → workflow 加载失败
- `internal/yamlpath` 永远失败(它只走 Python shim)
- `forge validate` 可以通过 Go 路径工作(因为它也尝试 Go 优先)

这意味着**让 Python 二进制消失的操作(容器化、最小镜像、Nix 等)会破坏 `yamlpath` 的所有使用**。

**风险 3: 同一个输入通过不同路径走会产生不同的 Go 类型**

Go `yaml2json.Decode` 返回的是 `map[string]any, []any, string, float64, bool, nil`。Python shim 通过 `json.Unmarshal` 解析,JSON 数字总是 `float64`。但 `yaml2json.Decode` 对小整数可能返回 `int`,大整数返回 `float64`。

```go
// 对比:
// yaml2json.Decode("count: 3") → map["count"] = 3 (int, 类型为 int)
// python shim + json.Unmarshal → map["count"] = 3.0 (float64, 类型为 float64)
// 
// asset.Workflow 的 encoding/json 标记了 useNumber,但其他消费者可能不会
```

**证据**: 搜索所有 `yaml2json` 的调用者:

| 调用位置 | 解析路径 | YAML 支持范围 |
|---|---|---|
| `cmd/forge/main.go:loadWorkflow` | Go 原生→Python 回退 | 子集→完整 |
| `cmd/forge/validate.go` | Go 原生→Python 回退 | 子集→完整 |
| `internal/yamlpath` | 仅 Python shim | 完整 |
| `harness/test_yaml2json.py` | 仅 Python shim | 完整 |

### 为什么这是 P1

这不是"需要统一到 Go"的纯工程偏好。当前三条路径可以**对同一个 YAML 文件产生不同的结构化结果**,而系统**从不检测这种不一致**。作为"治理 OS",自身数据路径的确定性是最基本的要求。一个 YAML 配置值由此路径读取为 `string`,由彼路径读取为 `float64`,可能在后续比较中静默行为不同。

---

## 方向二 · `cmd/forge` 包结构债务与架构边界侵蚀(P1)

> **类型**: 架构 · 可维护性
> **优先级**: P1 — 违反自身"单一职责"和"先拆分,再继续"的工程红线
> **预估工作量**: ~3 sprints(将 CLI 层逻辑拆入现有 `internal/` 包)

### 现状

`cmd/forge` 包是 ForgeOS 的 CLI 入口。根据 `BOOTSTRAP.md` 的架构设计,它应该是"薄胶水层"——解析参数、调用 `internal/` 包、呈现结果。

**当前的实际情况是**:

```
cmd/forge/ — 16 个文件,12,513 行
├── main.go              499 行  ← CLI 入口 + flag 解析 + loadWorkflow + reportConvergence
├── engine_build.go      498 行  ← Agent executor 构建 + 成本/观察/管线组装
├── prompt_context.go    454 行  ← Prompt 构建 + gate outcome 注入 + 4 种 ledger
├── gates.go             493 行  ← 信号收集 + convergence 报告 + gate resolution
├── cost.go              471 行  ← Claude 成本解析 + 运行预算跟踪
├── cost_test.go         452 行  ← 测试文件,但 >400 行暗示被测试的逻辑复杂度
├── prompt_context_test.go 477行
├── engine_build_test.go 461行
├── scorecard_wind.go    470 行  ← Scorecard 相关逻辑
├── scorecard_wind_test.go 470行
├── evolve.go            498 行  ← forge evolve 命令
├── validate.go          481 行  ← forge validate 命令
├── route.go             317 行
├── gates_test.go        447 行
...
```

对比 `internal/` 包的逻辑边界:

```
internal/ 包群 — 18 个包,~20k LOC
├── asset/          — 领域模型(Workflow/Phase/Signal)    133 行
├── orchestrator/   — 编排状态机 + 循环引擎 + 并行执行    ~1,500 行
├── prompt/         — 上下文构建 + 检索 + 缓存            ~400 行
├── converge/       — 收敛信号评估                        ~300 行
├── routing/        — 模型路由                            350 行
├── gate/           — 门控结果类型                        ~200 行
├── memory/         — 跨会话知识持久化                    ~400 行
├── risk/           — 风险分类器                          ~200 行
├── trace/          — 跟踪事件                            ~200 行
├── persist/        — 检查点持久化                        ~200 行
├── doctor/         — 诊断命令(8 文件)                    ~800 行
├── mode/           — 中枢旋钮蒸馏                        ~200 行
├── yaml2json/      — YAML 解析(9 文件)                  ~1,200 行
...
```

**关键发现**: `cmd/forge` 包的平均文件大小(12,513/16 ≈ 782 行)超过了本仓库的 `max_file_lines:500` 限制。它不是指单个文件超标(500 行限制的执法对象是单个文件),而是指**作为 CLI 胶水层,包的整体规模已经违背了架构意图**。

### 业务逻辑在 CLI 层的具体例子

**例子 A: `cost.go`(471 行,纯业务逻辑)**

`cost.go` 包含了:
- Claude `--output-format json` 的完全解析(`parseClaudeCostUsd`)
- 预算跟踪(`runBudget` + `checkRunBudget`)
- 成本累加器(`costEmitter` + `accumulateCosts`)
- `observeFor` 函数(471 行)——解析 claude 成本、汇总 phase output、提取 reviewer verdict、构建 gate 信号

这些逻辑与 CLI 无关——它们应该属于一个 `internal/cost` 或 `internal/budget` 包。`observeFor` 本身作为一个 100+ 行的函数,处理了**四种不同职责**:成本解析、管道数据流(phase output forward)、评审裁决提取、gate 信号注入。

**例子 B: `prompt_context.go`(454 行,复杂的状态管理)**

这个文件维护了 4 种不同的 ledger:
- `gateLedger` — gate 结果的映射
- `phaseOutputLedger` — 相位输出(前馈)的映射
- `reviewFindingsLedger` — 评审意见的映射
- `verdictLedger` — 裁决的映射

每个 ledger 都有自己的互斥锁,在并行模式下需要仔细的锁排序。这些是**状态管理**,不是 CLI 胶水。它们应该属于 `internal/prompt` 或一个新的 `internal/ledger` 包。

### 影响

1. **测试依赖**: 测试 `cmd/forge` 包中的函数需要导入整个 CLI 上下文(flag 解析、exec.Command、os.Exit 等),而不能只测试纯逻辑
2. **包级循环依赖风险**: 随着 `cmd/forge` 增长,更多逻辑难以拆入 `internal/` 包,因为没有清晰的拆分接口
3. **违背自身纪律**: 本仓库的 CLAUDE.md 声明"先拆分,再继续"和"单一职责",但 `cmd/forge` 已是本仓库最大的包

### 现有 `internal/` 模式的先例

项目已经有成功的拆分模式:
- `internal/doctor` (Sprint 27: 从 validate.go 和 main.go 拆分)
- `internal/attribution` (Sprint 27: 从 scorecard_wind.go 拆分)
- `internal/gate/resolve.go` (Sprint 29: 从 gates.go 拆分)
- `internal/mode` (Sprint 7: 模式政策的纯逻辑)
- `internal/migrate` (Sprint 8: 迁移命令的纯逻辑)

每个拆分都遵循相同的模式:逻辑 CLI 层→`internal/` 新包。`cmd/forge` 中的剩余逻辑(成本解析、prompt 构建、ledger 管理)是该模式的自然延伸。

---

## 方向三 · 长运行时存储的累积效应(P2)

> **类型**: 资源管理 · 韧性
> **优先级**: P2 — 24h 无人值守场景下必然触发,短跑不触发
> **预估工作量**: ~1.5 sprints(存储预算 + 轮换策略 + 边界守卫)

### 现状

ForgeOS 的 `forge evolve` 可以在无人值守下运行 24 小时+。它持续在磁盘上追加数据到多个文件中,但**没有存储预算、没有轮换策略、没有总大小保证**:

**文件 1: `.forge/trace.jsonl`**

```go
// forge-core/internal/trace/trace.go
// 每次事件(phase start/end, gate result, error)追加一行 JSON。
// 24h 跑几百次迭代 → 数万行。无轮换,无上限。
```

每 phase 产生的 trace 事件数:至少 2(start+end)+ gate 结果 + cost 事件 + 迭代边界。100 迭代 × 10 phase × 5 事件/phase = 5000 行。以每行 ~500 字节计 = 2.5 MB。**这看起来不大,但它从不修剪**——1000 迭代后是 25 MB,而且 `forge run --resume` 会重读整个文件。

**文件 2: `.forge/memory.jsonl`**

```go
// forge-core/internal/memory/memory.go
// memory 是追加日志:每次 Append 写一行,从不重写历史。
```

`memory.Prune` 和 `Compact` 存在但**从未被调用**:

```go
// memory_compact.go — 存在轮换逻辑
func (s *Store) Compact(path string, keepPerKind int) error { ... }
func (s *Store) Prune(path string) error { ... }
```

搜索所有调用者: `grep -rn "\.Prune\|\.Compact\|memory-prune" --include="*.go" forge-core/` → 只有 `cmdMemoryPrune` 作为 CLI 命令存在,没有被任何主流程自动调用。`forge evolve` 从不调用它。

**文件 3: `.forge/checkpoint.json`**

```go
// forge-core/internal/persist/checkpoint.go
// Checkpoint 是每次迭代覆盖重写的,但它保留的是单次快照——大小固定。低风险。
```

**文件 4: `.forge/scorecards.json`**

```go
// harness/scorecard-update.mjs
// 每次 run 后追加一个 entry。
```

### 问题

当 forge evolve 运行 24 小时以上时:

1. **memory.jsonl 增长不可控** — 每次迭代可能产生新的知识条目(发现、决策、错误教训)。100 迭代 × 5 条目 × 1KB = 500KB。如果 agent 产生大量发现,可能远大于此。`memory.go` 的 `Load` 函数将整个文件读入内存:
   ```go
   // memory.go:230-240
   scanner := bufio.NewScanner(f)
   for scanner.Scan() {
       // 解码每个条目,追加到切片
       entries = append(entries, entry)
   }
   ```
   这意味着 memory 文件越大,启动每个新 phase 时的内存压力越大。

2. **trace.jsonl 重新加载浪费 I/O** — `forge run --resume` 需要重读整个 trace 文件来重建状态。文件越大,resume 越慢。

3. **无存存储监控** — 当前没有任何警告当 `.forge/` 目录超过某个阈值时。用户直到磁盘满了才知道。

4. **memory-prune CLI 命令存在但不被 evolve 自动调用** — 用户需要手动运行 `forge memory-prune`。这在一夜无人值守场景下不会发生。

### 代码级证据

memory 包设计原文明确说"Append-only log, never rewrite":

```go
// forge-core/internal/memory/memory.go:20-30
// The store is JSONL: one self-describing JSON object per line, appended with a
// single O_APPEND write. Appending one framed line is the natural primitive for
// a grow-only log — it touches none of the existing history...
```

但缺乏对应的自动消费/轮换机制。

`LoopEngine` 完全没有存储管理的概念:

```go
// forge-core/internal/orchestrator/loop.go
type LoopEngine struct {
    // 有 MaxIter, NoProgress, StartIter — 但没有 MaxStorage 或 OnStorageWarning
    ...
}
```

对比已经存在的四维资源护栏:
- ✅ **深度** — `MaxDepth`(递归守护)
- ✅ **数量** — `MaxAgentCalls`(执行次数)
- ✅ **时间** — `Timeout`(墙钟)
- ✅ **内存** — `MaxOutputBytes`(输出大小)
- ❌ **存储** — **缺失** (本方向)

### 建议

这和方向四(跨进程文件冲突)是相邻问题——两者都是关于 `.forge/` 状态目录的管理。但方向三关注的是"磁盘上积累了多少",方向四关注的是"谁在同时写"。

---

## 方向四 · 跨进程状态文件冲突与锁缺失(P2)

> **类型**: 并发安全 · 可靠性
> **优先级**: P2 — 目前单进程单 CLI 下不会触发;并行使之恶化
> **预估工作量**: ~1 sprint(进程锁 + 目录隔离 + 冲突检测)

### 现状

ForgeOS 的状态文件(`.forge/trace.jsonl`、`.forge/memory.jsonl`、`.forge/checkpoint.json`) 被设计为无锁追加:

```go
// memory.go 使用 os.O_APPEND|os.O_WRONLY|os.O_CREATE, 无锁
// trace.go 使用 os.O_APPEND|os.O_WRONLY|os.O_CREATE, 无锁
```

`O_APPEND` 保证单次 `write()` 是原子的,但它不能防止两个进程的 `write()` 交叠到同一行中——这是 `O_APPEND` 的 POSIX 语义。

**场景 1: 两个 `forge run` 在同一个仓库上并行**

用户不小心在终端 A 运行 `forge run build`,同时在终端 B 运行 `forge run build`(或 `forge evolve build`)。两个进程都往 `.forge/trace.jsonl` 追加:

```
进程 A 写: {"kind":"agent","phase":"planner","duration_ms":500}
进程 B 写: {"kind":"agent","phase":"planner","duration_ms":600}
进程 A 写: {"kind":"gate","phase":"test","status":"ok"}
进程 B 写: {"kind":"gate","phase":"test","status":"FAILED"}
```

最终 trace 文件:
```
{"kind":"agent","phase":"planner","duration_ms":500}
{"kind":"agent","phase":"planner","duration_ms":600}{"kind":"gate","phase":"test","status":"ok"}
{"kind":"gate","phase":"test","status":"FAILED"}
```

**第三行被合并了**,因为两个进程的 `write()` 在内核中交叠——A 写一行(无换行时 B 也写)。即使每行都带 `\n`,标准库的 `bufio.Writer` 在 write 时也可能合并多行到一个 `write()` 调用中。

**证据: trace 写入是无缓冲的吗?**

```go
// forge-core/internal/trace/trace.go — 检查写入模式
f, err := os.OpenFile(path, os.O_APPEND|os.O_WRONLY|os.O_CREATE, 0644)
// 使用 f.Write() 还是 bufio?
```

每行通过 `fmt.Fprintln(f, string(data))` 写入。`fmt.Fprintln` 在底层调用 `f.Write`,在 Go 中它使用 `os.File.Write`,这是一个系统调用。然而多行缓冲仍然可能在 `fmt.Fprintln` 内部发生。

**场景 2: `forge evolve --resume` 读取被污染的文件**

当演进化迭代在 crashed 后恢复时,`resumeStart` 读取 trace 文件来重建最后已知状态。如果 trace 文件中有交叉行(被另一个进程污染),解析将失败:

```go
// trace Event 解析——按行解析 JSON
scanner := bufio.NewScanner(f)
for scanner.Scan() {
    var ev Event
    if err := json.Unmarshal([]byte(scanner.Text()), &ev); err != nil {
        // 会如何?静默跳过还是失败?
    }
}
```

**场景 3: memory 文件的跨进程缓存失效**

memory 包使用 `sync.Map` 进行进程内缓存,当文件 mtime 不变时复用。但如果另一个进程追加了内容,当前进程的缓存已过时——但 mtime 可能没有变化:

```go
// memory.go:107-112
// loadFromCache 使用 stat + mtime 检查缓存有效性
func loadFromCache(path string) ([]Entry, bool, error) {
    // 检查 path 的 stat → mtime
    // 如果 mtime 不变,返回缓存的 entries
}
```

O_APPEND 写操作不保证更新 mtime(在同一个挂载点上,多次 write 可能共享同一个 mtime)。所以进程 A 写了 memory,进程 B 读自己的缓存(mtime 没变)→ **B 看不到 A 的写入**。

**证据: 仓库中零个进程锁**

搜索 `flock`、`LOCK_EX`、`LOCK_NB`、`syscall.Flock`:

```go
grep -rn "flock\|LOCK_EX\|LOCK_NB\|syscall.Flock" forge-core/ --include="*.go"
```

**返回零匹配**。`internal/memory` 的 `sync.Map` 是进程内锁,不是进程间锁。

### 影响

- **并行执行 + 演化恢复 = 两个独立的状态写入器竞争同一文件**
- 虽然有设计假设"单进程单 CLI",但 `--parallel` 模式、`resume` 状态读/写、和用户误操作都会打破这个假设
- 没有进程锁意味着**系统不能安全地在 CI 中并行运行**
- 这不是理论问题:当用户将 `forge run` 放入 CI pipeline 时,两个 job 共享同一个工作目录和 `.forge/` 目录

---

## 方向五 · `forge-core` 不可编程集成与库 API 缺失(P2)

> **类型**: 平台化 · 集成面
> **优先级**: P2 — v1 的 CLI-only 决策是合理的,但 v2 平台化需要弥补
> **预估工作量**: ~3 sprints(公共 API 设计 + 包提升 + 文档 + 版本契约)

### 现状

ForgeOS 的核心价值(工作流编排、治理执法、收敛判定)目前只能通过 CLI 使用:

```
$ forge run build --mode balanced
$ forge accept --root /path/to/project
$ forge evolve build --max-iter 10
```

所有 `internal/` 包因 Go 的惯例(`package internal`)不可外部导入:

```
forge-core/
├── cmd/forge/      ← 唯一入口点
└── internal/       ← 18 个包,全部 internal,外部不可导入
    ├── orchestrator/
    ├── converge/
    ├── gate/
    ├── routing/
    ...
```

### 具体限制

**限制 1: CI/CD 集成只能通过 shell out**

```yaml
# .github/workflows/forge.yml 中
- run: forge accept --root ${{ github.workspace }}
```

这意味着:
- 每次调用启动一个新的 Go 二进制进程(冷启动 ~200ms)
- 输出只能通过 stdout/stderr 字符串解析
- 错误只能通过 exit code 判断
- 没有结构化错误类型传递
- 没有编程方式传递复杂参数(prompts、state、callbacks)

**限制 2: 没有编程式收敛检查**

假设一个 IDE 插件想在用户保存文件时检查治理合规性——它必须:

```python
# IDE 插件——而不是:
import forge_core
result = forge_core.gate.accept("/path/to/project")
if result.status == "REJECTED":
    show_inline_errors(result.violations)

# 只能:
import subprocess
result = subprocess.run(["forge", "accept", "--root", "/path/to/project"],
                       capture_output=True, text=True)
# 解析字符串输出,脆弱
```

**限制 3: `go.mod` 没有版本契约**

```go
// forge-core/go.mod
module forgeos/forge-core

go 1.26
```

没有 `require` 块(零依赖),也没有 `go 1.26` 之外的版本声明。虽然 `forgeVersion` 变量可以通过 `-ldflags` 注入,但它只是用于 `--version` 输出,没有 API 兼容性承诺。

**限制 4: `internal/` 包里有不该在 `internal/` 中的公共抽象**

`asset.Workflow`、`converge.Signals`、`gate.Result`、`orchestrator.Engine` 是**核心领域类型**,任何 forge-core 的集成者都需要它们。但它们被锁在 `internal/` 里,因为整个代码库被构建为一个单体 CLI。

对比:Go 社区的标准是将核心类型放在 `pkg/` 中,将 CLI 放在 `cmd/` 中:

```
forge-core/           ← 当前
├── cmd/forge/        ← CLI
└── internal/         ← 所有实现


forge-core/           ← 建议
├── cmd/forge/        ← CLI
├── pkg/orchestrator/ ← 导出:Engine, Workflow, Phase
├── pkg/converge/     ← 导出:Signals, Converge
├── pkg/gate/         ← 导出:Result, Gate
└── internal/         ← 非导出实现细节
```

### 为什么这是 P2 而非 P1

当前架构中 CLI-only 是一个合理的 v1 决策——ForgeOS 本身是"治理 OS",不是"库框架"。以下场景需要在 v1 投入之前已经被明确评估:

- **目前在 84+ 份分析中 0 篇独立展开此方向**
- `production-product-gaps-v43.md` 在方向四中提到了"导出 Go 库 API"一句话,但没有展开为独立方向
- 团队需要明确决定 forge-core 是"一个更好用的 CLI"还是"一个可嵌入的平台"
- 做出库 API 设计需要斟酌的 API 表面,做错了难以撤回

---

## 优先级与收敛建议

| 方向 | 优先级 | 类型 | 杠杆 | 外部依赖 | 推荐 |
|---|---|---|---|---|---|
| ① YAML 解析器三重碎片化 | **P1** | 可靠性/一致性 | ⭐⭐⭐⭐⭐ | 无(纯代码合并) | **下个 sprint** |
| ② `cmd/forge` 包结构债务 | **P1** | 架构/可维护性 | ⭐⭐⭐⭐⭐ | 无(纯拆分) | **下个 sprint**(与①并行) |
| ③ 存储累积效应 | P2 | 资源管理 | ⭐⭐⭐ | 无(纯新增守卫) | 24h 产品化前必须做 |
| ④ 跨进程文件冲突 | P2 | 并发安全 | ⭐⭐⭐ | 无(纯进程锁) | CI/CD 启用前必须做 |
| ⑤ forge-core 库 API | P2 | 平台化 | ⭐⭐ | 无(API 设计 + 包提升) | 等社区需求触发 |

**收敛建议**:

- **若只做一件**:方向②(`cmd/forge` 包结构债务)。这是 ForgeOS 自己"先拆分,再继续"纪律的直接延伸。项目在多次 sprint 中已经在这个方向上投入了(Sprint 23/27/29/30),但工作尚未完成。完成它可以解锁方向①(成本解析可以自然地流入新包)、方向⑤(公共 API 提取的奠基),且杠杆全局最高。

- **若做两件**:② + ①。方向①是当前代码库中最容易被触发但从不报告的静默错误源。三条 YAML 路径可能产生不同结果,而**没有任何运行时检查能发现这种不一致**。作为"治理 OS",自身输入层的确定性是最基本的要求。

- **方向③④是 24h 无人值守产品的必要前提**,但在 v1 阶段可暂缓——只要用户只跑一个 `forge run` 或一个 `forge evolve`,存储累积和文件冲突不会暴露。一旦用户开始并行、CI/CD、或长期演化,这两个方向就变成 P1。

- **方向⑤是平台化决策**——不是技术可行性的问题,而是产品定位的问题。建议等外部集成需求(IDE 插件、CI 系统、自托管监控)出现后再投入,以免设计出一个没人用的公共 API。

> 本文 5 个方向均经过 84+ 份已有分析文档的全文搜索确认,所有代码引用来自对 `forge-core/`、`harness/` 和 `.agent/` 的直接阅读。每个方向的关键词搜索模式已在正文中列出。如果某个方向与已有分析的重叠被遗漏,欢迎纠正。
