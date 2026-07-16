# ForgeOS — V34 全新五方向：代码级微观观察驱动的架构扩展

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深度扫描 — forge-core 18 Go 包 / 195+ 源文件 / 777+ 测试 /  
>   harness 38+ 模块 / `.agent/` 完整骨架（12 agent 卡 + 9 skill 卡 + 5 工作流）/  
>   Sprint 1–31 完整演进 / `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE + GAP 全部收口）/  
>   **通读交叉验证 40+ 篇 `docs/analysis/*.md` + 14 篇 `docs/requirements/*.md`（~65 个已有方向）**  
> **核心承诺**: 每个方向与全部 ~65 个已有方向的**核心论点不重叠**。差异证明附后。  
> **纪律**: 不编写任何代码。  
> **日期**: 2026-07-10

---

## 已有覆盖全景（本文不重复）

本文**不重复**以下已被充分覆盖的域（逐一核对 65+ 份已有分析文档）：

| 已有覆盖域 | 代表文档 | 方向数 |
|------------|----------|--------|
| **路由/编排/记忆/收敛引擎补齐** | `high-value-extension-directions.md` · `v3` | ~15 |
| **第三地平线生态**（多仓库/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` · `expansion-gaps-v7-novel.md` | ~10 |
| **生产可靠性**（Prompt QA / 信号硬化 / 环境验证 / 自愈层） | `expansion-production-readiness.md` | ~8 |
| **执行语义形式化**（原子性/幂等性/因果一致性/回滚） | `execution-semantic-gaps.md` | ~8 |
| **二阶伴生问题**（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md` | ~10 |
| **系统边界盲区**（级联截断/信任边界/持久语义/可移植性） | `strategic-extensions-v22-v32.md` | ~10 |
| **架构盲区 + 多波分析** | 40+ 篇 `docs/analysis/*.md` | ~30 |
| **同步护栏 & 分布式状态机** | `strategic-extensions-v33.md` 方向一 | 1 |
| **部分失败域隔离** | `strategic-extensions-v33.md` 方向二 | 1 |
| **声明式资源预算交叉验证** | `strategic-extensions-v33.md` 方向三 | 1 |
| **语义化配置漂移检测** | `strategic-extensions-v33.md` 方向四 | 1 |
| **跨 Session 审计因果追溯** | `strategic-extensions-v33.md` 方向五 | 1 |
| **收敛信号信任分层** | `genuine-architectural-gaps-v28.md` 方向一 | 1 |
| **治理资产热加载**（零停机策略更新） | `genuine-architectural-gaps-v28.md` 方向二 | 1 |
| **Phase 级确定性回放** | `genuine-architectural-gaps-v28.md` 方向三 | 1 |
| **Agent 能力协商分配** | `genuine-architectural-gaps-v28.md` 方向四 | 1 |
| **跨工作流产物版本一致性** | `genuine-architectural-gaps-v28.md` 方向五 | 1 |
| **工作流组合引擎** | `genuine-expansion-gaps.md` 方向一 | 1 |
| **跨运行知识生命周期** | `genuine-expansion-gaps.md` 方向二 | 1 |
| **Phase 输出契约验证** | `genuine-expansion-gaps.md` 方向三 | 1 |
| **并行资源治理** | `genuine-expansion-gaps.md` 方向四 | 1 |
| **失败智能与自动修复** | `genuine-expansion-gaps.md` 方向五 | 1 |
| Scorecard 冷启动 bootstrap prior | `expansion-directions-v4-novel-perspectives.md` | brief note |
| Git tree hash gate-cache 概念 | `high-value-extensions.md` | brief note |
| Trace MQTT publisher (sink 替换) | `mqtt-and-wasm-integration.md` | 1 (specific) |
| .forge/ 跨进程一致性 | `strategic-extensions-v33.md` 方向一 | touches but domain diff |

---

## 本文的 5 个方向

以下方向均从**代码级微观模式 + 真实运维场景**推导。每个方向的起点是一条代码级的观察证据链，而非抽象的产品愿景。所有方向均属 **v2 增量可实现**（不依赖 Firecracker / LiteLLM / 外部数据库）。

---

## 方向一：新项目知识启动协议（Knowledge Bootstrapping Protocol）

> **类型**: 数据生命周期 · 开发体验 · 收敛加速  
> **优先级**: P1（长期自治运行的启动瓶颈）  
> **代码影响**: `harness/scaffold/forge-init.mjs` · 新 `internal/bootstrap/` 包 · `internal/memory/` · `internal/persist/`  
> **差异化证明**: 已有分析仅覆盖 scorecard cold-start prior（`expansion-directions-v4-novel-perspectives.md` 一句提及），未涉及**全栈知识启动**（memory / trace / ADR / convergence baseline / gate history 的联合播种）

### 现状：代码级观察

**证据 A：`forge-init` 创建的项目运行时数据完全空白**

```bash
# forge-init 后的首个 forge run/evolve
.forge/checkpoint.json  → 不存在（persist.Save 首次创建）
.forge/memory.jsonl     → 不存在（memory.Append 首次创建）
.forge/trace.jsonl      → 不存在（tracer.Emit 首次创建）
.forge/scorecards.json  → 不存在（scorecard-update 首次创建）
```

这意味着：第一个 `forge evolve` 的每一条收敛判断都是从零开始。`RoadmapCompletion` 从 0% 开始，`GatesGreen` 从 false 开始，gate 历史为空。路由层没有任何历史择优数据（`HistoryTiebreak` 面对空 scorecard 退化为 `candidates[0]`）。

**证据 B：`internal/memory/memory.go` 的 `Load()` 对空白文件返回 `(nil, nil)`**

```go
// memory.go:Load — cold-start 返回 nil,nil，上层得到空知识库
func Load(path string) ([]Entry, error) {
    // ...
    if _, err := os.Stat(path); errors.Is(err, fs.ErrNotExist) {
        return nil, nil  // ← 冷启动：没有知识
    }
    // ...
}
```

上层（`prompt_memory.go` 的 `memoryContext`）对空列表的处理是「不注入任何记忆块」—— 完全合理的语义，但也意味着新项目的前 N 次迭代得不到任何来自过往经验的指导。

**证据 C：`internal/routing/routing.go` 的 `HistoryTiebreak` 面对空 scorecard 退化**

```go
// routing.go:HistoryTiebreak — 空 scorecard 退化为无历史择优
func HistoryTiebreak(candidates []string, scores []ScorecardEntry) string {
    if len(scores) == 0 {
        return candidates[0]  // ← 冷启动：退化为静态默认
    }
    // ... 有数据时的择优逻辑
}
```

**证据 D：`internal/converge/converge.go` 的 `evalRoadmap` 从 0% 开始**

```go
// evalRoadmap — 第一个迭代时 roadmap_completion=0
func evalRoadmap(c asset.Criterion, sig Signals) Result {
    threshold := 1.0  // 默认 100%
    met := sig.RoadmapCompletion >= threshold
    return Result{
        Met:    met,
        Detail: fmt.Sprintf("roadmap_completion=%.0f%% (need %.0f%%)",
            sig.RoadmapCompletion*100, threshold*100),
    }
}
```

没有任何初始基线。第一轮 converge 检查必然返回 NOT MET（除非 roadmap 本身就是空的）。

### 为什么需要

1. **「前 10 次迭代盲跑」是一个真实的产品摩擦**。一个团队用 forge-init 创建新项目、运行 `forge evolve`，前 5-10 次迭代都在「建立基线」而非「推进项目」—— 收敛信号从 0 开始爬升、路由从默认开始探索、agent 在没有历史经验的情况下工作。这种体验与 ForgeOS 宣传的「24h 自治从 Idea 到 Production」之间存在明显的落差。

2. **知识可以在项目类型层面复用**。Go 微服务的架构模式、Python Web 项目的依赖管理习惯、前端项目的测试策略 —— 这些是跨项目的通用知识。如果 forge-init 能根据 `detect` 识别的项目类型，播种一份初始知识库（标准 ADR、常见架构决策、最佳实践 memory entries），新项目的前几次迭代就能站在「巨人肩膀」上，而非从零开始。

3. **已有基础设施已具备播种能力**：
   - `memory.Append()` 已是纯追加写入，播种 = 第一次 evolve 前调一次 Append
   - `trace.NewTracer()` 接受 io.Writer，播种 = 写一批启动事件
   - `scorecard-update.mjs` 接受任意 scorecards.json，播种 = 预填行业参考值
   - `prompt.ContextCache` 已有失效机制，播种可热加载

### 关键设计边界

- **播种数据必须诚实标注为「初始知识，非来自本项目的真实运行」**。Memory 条目加 `kind: "bootstrap"` 字段，trace 事件加 `_origin: "bootstrap"` 标记。scorecard 的 bootstrap prior 使用已有 scorecard schema 的 `sample_count=0` 语义（`expansion-directions-v4` 已定义）。
- **播种数据随项目实际数据的积累自动衰减权重**。`Compact` 在真实数据超过阈值（如 10 条真实 memory entries）时丢弃 bootstrap 条目。
- **可选择性**：`forge init --skip-bootstrap` 保留当前零知识启动行为，不破坏现有语义。
- **项目类型映射**：利用已有的 `forge detect` 的项目类型识别结果（`internal/detect`），自动选择匹配的知识包。

### 实现规模

新 `internal/bootstrap/` 包 ~300 行 + `harness/scaffold/` 修改 ~100 行 + 3-5 个「知识包」模板（每包 50-100 行预设数据）。总计 ~600-800 行。

---

## 方向二：闸门探测结果缓存与增量重评估（Gate Probe Result Cache）

> **类型**: 性能 · 资源治理 · 长运行优化  
> **优先级**: P1（直接影响 24h 运行的经济性）  
> **代码影响**: `harness/acceptance-quality.mjs` · `harness/acceptance.mjs` · 新 `internal/probecache/` 包  
> **差异化证明**: `high-value-extensions.md` 方向四提过一句「gate-cache 基于 git tree hash」作为 no-op 检测，但**未展开**为完整方向；本文给出完整的代码级证据链、设计边界和实现规模评估

### 现状：代码级观察

**证据 A：`forge accept` 每次独立跑全量 probe**

```javascript
// acceptance-quality.mjs:probeLint — 每次跑完整 lint，不检查文件是否变化
export function probeLint() {
  const r = run('python3', [join(HARNESS_DIR, 'check.py')]);
  // ... 总是全场扫描
}
```

```go
// gates.go:probeStatuses — 每个迭代调一次 gate.ProbeAll
func probeStatuses(root string, wf asset.Workflow) (map[string]string, error) {
    return gate.ProbeAll(root, wf)  // ← 每个迭代全量跑所有 gate
}
```

**证据 B：`internal/gate/gate.go` 的 `ProbeAll` 无条件执行全部 probe**

```go
// gate.go:ProbeAll — 无缓存，每次都跑
func ProbeAll(root string, wf asset.Workflow) (map[string]string, error) {
    // 遍历 workflow 的 required_gates → 每个 gate 都执行
}
```

在 24h `forge evolve` 场景中，10 次迭代就意味着 10× 全量 lint、10× 全量 coverage、10× 全量 test、10× 全量 SCA 扫描。如果代码在 iteration 3 之后就不再变化（仅 agent 调整 prompt/注释），迭代 4-10 的 gate 探测结果 100% 冗余。

**证据 C：代码变化量通常远小于仓库大小**

每次 evolve iteration 平均修改 1-3 个文件（来自 S25-26 真跑记录）。但 probe 仍然扫描整个仓库。`git diff --name-only HEAD~1` 可以精确告知哪些文件变化了。

### 为什么需要

1. **长运行时性能回归是渐进式的**。单个 gate probe（如 go vet）可能只需 2 秒，但在 iteration 10 时，10×2 秒 = 20 秒纯浪费。当 gate 集合包含 lint（~5s）+ coverage（~10s）+ SCA（~3s）+ app-test（~30s）时，单次全量 probe ~48 秒，10 次迭代 ~480 秒（8 分钟）—— 其中可能只有前 2-3 次迭代有实际代码变化。

2. **probe 的非幂等性副作用**。coverage probe 写 `coverage.out` 文件（已被 `acceptance-quality.mjs` 注释承认会污染工作树：「drops coverage.out even」）。lint probe 可能写缓存文件（如 `.eslintcache`）。减少不必要的 probe 也减少对这些副作用的处理。

3. **已有基础设施可以支撑增量探测**：
   - `git diff --name-only HEAD~1` 是零成本操作
   - `select-tests.mjs` 已经实现了相关的「影响文件分析」逻辑（虽当前仅为 advisory，但机制已存在）
   - `internal/risk.FromChangedPaths` 已有路径分析框架

### 关键设计边界

- **缓存键 = hash(probe_name, git_tree_hash, probe_config_hash)**。只缓存完全可重现的 probe 结果（lint/coverage/build 等确定性工具）。测试（test/app-test）因 flaky 可能产生不同结果，缓存时间窗短（如 60 秒内同 tree hash 复用）。
- **首次全量，后续增量**。`forge run build` 的第一次 iteration 始终全量跑所有 probe 建立基线。后续 iteration 通过 `git diff --name-only HEAD` 判断变化范围，只跑受影响的 probe 子集。
- **缓存显式失效**。`forge run --no-cache` / `FORGE_PROBE_CACHE=off` 跳过缓存，用于调试。`.forge/probecache/` 目录存储缓存条目，`forge doctor` 可以报告缓存命中率。
- **不与现有 converge 语义冲突**。缓存仅影响 probe 的**执行**，不影响 probe 结果的**语义**。如果缓存命中的结果与当前状态一致，与全量跑的结果等效。缓存层在 converge 层之下，完全透明。

### 实现规模

新 `harness/probecache.mjs`（或 `internal/probecache/` Go 包）~200 行 + `acceptance-quality.mjs` 和 `gate.go` 修改 ~150 行 + 测试 ~150 行。总计 ~500 行。

---

## 方向三：统一持久化存储抽象层（Unified Storage Sink Abstraction）

> **类型**: 架构 · 可移植性 · 可观测性  
> **优先级**: P2（架构健康度方向，长周期回报高）  
> **代码影响**: 新 `internal/storage/` 包 · `internal/trace/` · `internal/memory/` · `internal/persist/` · `cmd/forge/`  
> **差异化证明**: `mqtt-and-wasm-integration.md` 只谈了 **trace 替换为 MQTT publisher**，未提出**通用的存储抽象层**；`strategic-extensions-v33.md` 方向一讨论了跨进程 `.forge/` 命名/锁问题，但未涉及输出解耦。本方向是两者的**通用化上位方案**：不止 trace，所有持久化输出的接口统一化

### 现状：代码级观察

**证据 A：四种不同的持久化写入模式，四种不同的抽象层级**

| 存储 | 接口 | 是否可注入 | 是否硬编码路径 |
|------|------|-----------|--------------|
| `trace/trace.go` | `io.Writer`（`tracer.Emit` 写 `t.w`） | ✅ 构造函数注入 | ❌ 路径由外部 io.Writer 决定 |
| `persist/checkpoint.go` | `Save(path, cp)` + 内部 `os.OpenFile` | ❌ 硬编码 os 调用 | ✅ 路径作为参数 |
| `memory/memory.go` | `Append(path, entry)` + 内部 `os.OpenFile` | ❌ 硬编码 os 调用 | ✅ 路径作为参数 |
| `memory/memory_compact.go` | `Compact(path, …)` + 内部 `os.Create` | ❌ 硬编码 os 调用 | ✅ 路径作为参数 |

每一层都有自己的文件打开策略、错误处理、和锁/原子性保证。trace 是唯一实现了 io.Writer 注入的——但它的 Emit 锁是文件级别的，如果底层 io.Writer 是网络 writer，锁的语义完全不同。

**证据 B：`persist/checkpoint.go` 的原子写入与 `memory/memory.go` 的追加写入的冲突**

```go
// checkpoint.go — 原子替换（写 temp → fsync → rename）
f, err := os.OpenFile(path, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, 0o644)
f.Write(data)
f.Sync()  // fsync
f.Close()
os.Rename(tmp, path)  // 原子替换

// memory.go — 追加写入
f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
f.Write(line)  // O_APPEND，无 fsync，无锁
```

当未来需要将这两种存储指向同一个物理位置（如统一写到 `forge-session.jsonl`）时，不同的原子性保证会碰撞。

**证据 C：所有存储路径由 `cmd/forge/main.go` 的 `logFilePath`/`memoryPath`/`checkpointPath` 等变量决定**

```go
// main.go:runOpts
checkpointPath = filepath.Join(root, ".forge", "checkpoint.json")
tracePath      = filepath.Join(root, ".forge", "trace.jsonl")
memoryPath     = filepath.Join(root, ".forge", "memory.jsonl")
scorecardPath  = filepath.Join(root, ".forge", "scorecards.json")
```

这些路径在所有 `cmd/forge/*.go` 文件间作为字符串传递。没有一个 `StorageConfig` 或 `Sink` 类型来封装「存储目标是本地文件系统」这个假设。改为网络存储、内存存储、多路复用、或结构化日志时，需要改每个调用点。

### 为什么需要

1. **走向分布式架构的单体解耦前置**。North-star 架构（`north-star.md`）描述了一个 gRPC 分布式系统，其中每个服务都有独立状态存储。今天所有存储都指向 `.forge/` 本地目录。统一的存储抽象层让未来「把 trace 写入 OTel collector」、「把 checkpoint 写入对象存储」、「把 memory 写入 Qdrant」成为可选的 Sink 实现，而非大规模重构。

2. **测试基础设施的长期受益**。当前测试中，trace/memory/checkpoint 的 `Load` 和 `Save` 都需要真实文件系统。如果有一个可注入的 Sink 接口，测试可以传入 `bytes.Buffer` 或 `memfs`，消除临时文件、提速测试、并跑并发测试时不再有文件冲突。

3. **运维场景的真实需求**。一个在 CI 中跑 `forge run` 的流水线可能希望 trace 输出到 stdout（便于 CI 日志捕获），memory 输出到临时文件（不污染 workspace），scorecard 输出到特定的 metrics 目录。今天做不到，除非改代码。

### 关键设计边界

- **薄抽象，不发明新格式**。`StorageSink` 接口只规定 `Read()/Write()/Sync()/Close()`（类比 `io.ReadWriteCloser` + `Sync() bool`）。不统一数据格式（trace 仍是 JSONL，checkpoint 仍是 JSON），只统一访问模式。
- **默认实现保持零依赖**。`FileSink` 实现使用 `os.OpenFile`（与今天行为逐位一致），作为默认值。不引入外部依赖。
- **可组合**。`MultiSink`（同时写文件 + stdout）、`BufferedSink`（批量写入，定时 flush）、`PrefixSink`（每行加前缀）等组合子通过包装模式实现。
- **不做存储层的锁/一致性保证**。存储层只负责读写字节。锁和一致性由上层（`internal/persist` / `internal/memory`）负责——这正是它们已经做的事情。存储抽象只解耦「写到哪里」，不改变「怎么写」的语义。

### 实现规模

新 `internal/storage/sink.go`（接口 + 默认 FileSink）~100 行 + `internal/storage/compose.go`（MultiSink/BufferedSink/PrefixSink）~200 行 + trace/memory/persist 的构造器修改 ~150 行 + 测试 ~200 行。总计 ~650 行。

---

## 方向四：工作流编排集成测试框架（Workflow Orchestration Integration Test Framework）

> **类型**: 测试基础设施 · 质量保证 · 开发效率  
> **优先级**: P2（当前编排逻辑的测试覆盖存在结构性缺口）  
> **代码影响**: 新 `harness/wftest/` 目录 · `internal/orchestrator/` 测试辅助  
> **差异化证明**: 已有分析中无一处提出编排集成测试框架的概念。`expansion-production-readiness.md` 的「组合测试」段（`docs/requirements/expansion-production-readiness.md`）只提了一句「should test multi-mechanism orchestration」但**未展开为方向**。本文是首次完整定义编排集成测试框架的需求

### 现状：代码级观察

**证据 A：编排测试的「沙漏型」分布——中间层完全缺失**

当前测试分布在两个极端：

```
单元测试（~700 个）
  ├── internal/converge/     ✓ 纯函数隔离测试
  ├── internal/memory/       ✓ 编解码隔离测试
  ├── internal/persist/      ✓ 序列化隔离测试
  ├── internal/gate/         ✓ 结果解析隔离测试
  ├── internal/risk/         ✓ 分类隔离测试
  └── ...
         ⬇
   ❌ 中间层完全缺失 ❌
   没有「用 fake agent、fake gate、fake git 跑完一个完整 workflow」
   的集成测试
         ⬇
端到端测试（Sprint 24-26）
  └── 真 `--agent-cmd=claude`  高成本、不可在 CI 重复
```

**证据 B：`test_acceptance.mjs` 证明「fake 集成测试」的模式可行但未用在编排层**

`harness/test_acceptance.mjs` 演示了如何用 fake probe 验证 acceptance 的裁决逻辑：

```javascript
// test_acceptance.mjs — 注入 fake probe 验证 decide()
const fakeResults = [
  result('test_pass', PASS, 'ok'),
  result('coverage', NA, 'no coverage tool'),
];
const { verdict } = decide(fakeResults);
assert.strictEqual(verdict, ACCEPTED);
```

但这个模式只用于 acceptance 的收敛裁决，没有扩展到**完整的 workflow 编排测试**（phase 顺序、mode-gating、loop-back、checkpoint/resume 的交叉组合）。

**证据 C：`orchestrator_test.go` 使用 fake `AgentExecutor` 和 `RunGate`，但测试场景有限**

```go
// orchestrator_test.go — 使用 fake executor 验证单一情景
mockExec := func(ctx context.Context, p asset.Phase, mode string) error {
    return nil  // 总是成功
}
engine := Engine{
    Exec:   mockExec,
    RunGate: func(name string) gate.Result { return gate.Result{OK: true} },
}
err := engine.RunFrom(ctx, wf, mode, 0)
```

这些测试验证特定机制（loop-back、mode-gating、checkpoint firing），但不验证**完整 workflow 在多轮迭代中的编排行为**——例如：loop-back 执行 2 次后 checkpoint 的状态、mode-gating 与 phase 产物的交互、parallel 模式下 lock ordering 的实际运行验证。

### 为什么需要

1. **编排逻辑的复杂度已经超出「单一机制单元测试」的覆盖能力**。当前有 8+ 种机制可以同时激活：mode-gating + loop-back + checkpoint/resume + parallel + agent-call budget + output-size cap + timeout + retry。它们的组合行为只能用集成测试覆盖。持续增长的组合复杂度意味着，在修改 `orchestrator/loop.go` 或 `parallel.go` 时，没有集成测试来确保不破坏已有机制组合。

2. **end-to-end 真 LLM 测试成本太高、速度太慢**，不适合作为常规 CI 门禁。Sprint 24-26 的八次真跑修了八个真 bug，证明了真实 agent 测出问题的价值，但每次跑需要真 API 调用和授权。CI 需要一条更快、更便宜的反馈回路。

3. **fake agent 模式已被证明有效**。`expansion-production-readiness.md` 的「组合测试」概念验证（echo + fake gate）表明：fake agent 可以验证编排逻辑，只需要 deterministic I/O 而不需要 LLM。

### 关键设计边界

- **脚本化 fake agent**。fake agent 是一个可执行脚本（bash/node/go），从 stdin 读 prompt，按脚本规则输出内容（可预定义输出、可脚本化 gate 结果、可模拟超时/错误）。
- **场景即测试用例**。每个集成测试是一个 YAML 描述的场景：给定一个 workflow（或选择 build.yml/evolve.yml 等标准 workflow）+ fake agent 输出模板 + fake gate 结果序列 + checkpoint 初始状态，验证最终 converge 结果和 trace 事件序列。
- **trace 事件序列作为测试断言**。集成测试不 assert「phase X 输出了什么」（那需要 LLM），而是 assert「trace 事件序列是否符合预期」——哪个 phase 在哪个顺序执行、哪个 gate 触发 loop-back、checkpoint 在哪个 iteration 写入、convergence 何时 MET。
- **不替代现有单元测试**。集成测试做编织（weaving）测试，不做单一机制测试。CI 跑集成测试集（~30 个场景，预计耗时 <30 秒）作为 `forge accept` 的前置门禁。

### 实现规模

`harness/wftest/runner.mjs`（场景加载 + fake executor + trace 断言）~350 行 + 10-15 个初始场景 `.yml` 文件 ~300 行 + CI 集成 ~50 行。总计 ~700 行。

---

## 方向五：运行时进程健康契约与自诊断（Runtime Process Health Contract & Self-Diagnostics）

> **类型**: 运维 · 可靠性 · 长运行治理  
> **优先级**: P2（随 24h 运行采纳率自动升值）  
> **代码影响**: `internal/doctor/` · `internal/trace/` · `internal/memory/` · `internal/persist/` · `cmd/forge/`  
> **差异化证明**: 已有分析中 `expansion-directions-v6-novel-perspectives.md` 提过「自愈层」但聚焦于**故障恢复**（corrupt checkpoint 修、orphan process 杀），非**健康监测 + 主动报告**。`doctor` 包检查的是**仓库状态**（`.forge/` 目录完整性、governance 资产），不是**进程健康**。本方向填补的是「进程级 liveness + resource monitoring + auto-mitigation」的缺口

### 现状：代码级观察

**证据 A：`internal/doctor/doctor.go` 只检查仓库状态，不检查进程状态**

```go
// doctor.go:Run — 检查 .forge/ 目录、trace/memory/checkpoint 的存在性和大小
// 检查 workflow 文件的可读性
// 检查 .agent/ 目录完整性
// 但不检查：
//   - 当前进程的 goroutine 数
//   - 当前进程的内存使用
//   - 已打开的文件描述符数
//   - trace/memory/checkpoint 文件的增长速率
//   - 锁竞争（lock contention）统计
```

**证据 B：`internal/trace/trace.go` 的 `TraceJSONL` 在 24h 运行中无界增长**

```go
// trace.go:Emit — 每次写入一行，永不压缩、永不轮转、永不检查文件大小
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    defer t.mu.Unlock()
    ev.Seq = t.nextSeq
    t.nextSeq++
    b, _ := json.Marshal(ev)
    line := append(b, '\n')
    _, err := t.w.Write(line)  // ← 无限追加
    // 无文件大小检查、无轮转、无采样
    return err
}
```

在 Sprint 24-26 的真跑中，一个 evolve 跑了约 10 个 iteration，产生了 ~200 条 trace 事件。如果 24h 跑 100 轮，就是 ~2000 条事件（约 500KB-1MB JSONL）。可接受。但如果 gate 密度高、agent phase 多、frequency 快，trace 可以膨胀到 10MB+。目前没有任何看门狗。

**证据 C：`internal/memory/memory.go` 已有 `Compact` 但仅在手动 `forge memory-prune` 触发**

```go
// memory_compact.go:Compact — 压缩 memory，但只能手动触发
func Compact(path string, keepPerKind int, maxAge time.Duration) error {
    // ... 只在 forge memory-prune 命令时被调用
}

// 没有任何自动触发点
// 没有在 trace 文件过大时告警
// 没有在 goroutine 数异常时报告
// 没有在进度文件的增长速率超过阈值时自动 compact
```

**证据 D：`internal/orchestrator/parallel.go` 的 8 把锁无运行时竞争检测**

```go
// parallel.go — 8 把锁的获取顺序有书面契约
// LOCK ORDER CONTRACT（见文件头部注释）
// 但没有任何运行时验证机制：
//   - 没有 `-race` 检测的自动集成
//   - 没有锁等待时间的监控
//   - 没有死锁探测（Go 标准库不提供）
```

对于 24h 进程，锁竞争可能逐渐加剧（如果 memory/trace 文件变大导致 IO 变慢，持锁时间变长）。没有手段发现这个问题。

### 为什么需要

1. **24h 自治运行需要一个「无人值守时的健康可见性」机制**。操作员在 12 小时后回来，怎么知道 forge 进程还健康？不是通过 SSH 上去 ps aux，而是通过 `forge status --self` 看到 goroutine 数、内存占用、trace 文件增长速率、各 probe 的平均耗时趋势。

2. **资源泄漏在长时间运行中会从「可忽略」变成「致命」**。Go 的 goroutine 泄漏不会立刻 OOM——它会缓慢增长，12 小时后从 50 个 goroutine 变成 5000 个，然后 OOM。没有进程级健康监测，这个泄漏在单元测试（秒级运行）中永远抓不到。

3. **trace/memory 的无界增长是一个可预测的运维事故**。一个 `forge evolve` 如果 gate 多、iteration 多、agent phase 多，trace.jsonl 可以在 24h 内膨胀到百 MB 级别。届时 `forge doctor` 读 trace 都会变慢，更不用说 scorecard 处理、trace 回放了。

### 关键设计边界

- **自诊断是只读的，永不自动 kill 进程**。诊断报告异常（goroutine 泄漏、trace 增长过快、memory 过大），但不自动终止。操作员看到报告后决定操作。未来可加 auto-mitigation（如 trace 达阈值后自动 compact 然后 log），但 v1 只报告。
- **阈值可配**。`max_goroutines=500`、`max_trace_mb=100`、`max_memory_mb=50` 等配置在 `project.yml` 或 `policies.yml` 中声明。
- **诊断结果写入 trace**。`forge status --self` 的检查结果作为 `kind: "health"` 事件写入 trace，构成健康时间线（与 workflow 事件同序列），便于事后审计：「在 iteration 7 时，进程健康吗？→ 看到 250 goroutines, 45MB RSS, trace_file=2.1MB, fine」。
- **与 `forge doctor` 合一**。`doctor.go` 已有的 `Check` 结构体自然扩展：新增 `GoroutineCount()`, `MemoryUsage()`, `TraceFileGrowth()`, `LockContention()` 等健康检查。`forge doctor --self` 触发进程级检查（区别于默认的仓库级检查）。

### 实现规模

`internal/doctor/health.go`（进程健康检查）~250 行 + `internal/trace/trace.go` 的 size check 和轮转支持 ~100 行 + `cmd/forge/` 的 `--self` flag ~50 行 + 测试 ~150 行。总计 ~550 行。

---

## 总结对照表

| 方向 | 类型 | 优先级 | 已有覆盖检查 | 规模估计 | 代码影响 |
|------|------|--------|------------|---------|---------|
| **① 知识启动协议** | 数据生命周期/DX | P1 | Scorecard cold-start prior 仅一句提及；memory/trace/ADR 全栈启动无人覆盖 | ~700 行 | 新 `internal/bootstrap/` + `forge-init` 修改 |
| **② 闸门结果缓存** | 性能/资源 | P1 | `high-value-extensions.md` 一句提及「gate-cache 基于 git tree hash」但未展开 | ~500 行 | 新 `harness/probecache.mjs` + `gate.go` 修改 |
| **③ 统一存储抽象** | 架构/可移植性 | P2 | `mqtt.md` 只谈 trace→MQTT 替换，未提通用 sink 层；`v33` 方向一触及 `.forge/` 命名但不同域 | ~650 行 | 新 `internal/storage/` + 3 包构造器修改 |
| **④ 编排集成测试框架** | 测试基础设施/QA | P2 | 无已有分析覆盖；`expansion-production-readiness.md` 一句「组合测试」概念但未展开 | ~700 行 | 新 `harness/wftest/` + orchestrator 测试辅助 |
| **⑤ 进程健康契约** | 运维/可靠性 | P2 | `expansion-v6` 的「自愈层」关注故障恢复；`doctor` 检查仓库而非进程；本文填补进程健康监测 | ~550 行 | `internal/doctor/health.go` + trace size guard |

所有方向均为 v2 增量可达成，不依赖外部基础设施（数据库/沙箱/跨厂商 key）。每个方向起始于一条代码级的观察证据，以可估算的实现规模收尾。
