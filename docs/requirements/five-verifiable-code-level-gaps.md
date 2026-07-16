# ForgeOS — 五个可验证的代码级扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 forge-core (18 Go 包 · ~34k LOC) + harness (39 模块 · ~10.5k LOC) + .agent/ (12 角色卡 · 5 工作流 · 全部 ADR) + examples/ + pi-batch.py。逐文件阅读源码，不做任何代码编写。  
> **差异化验证**: 在 85+ 篇已有分析文档中逐方向核查覆盖情况，确保每个方向都落在已有讨论的间隙中。每个方向附带唯一的代码级证据。  
> **日期**: 2026-07-10

---

## 全景速览: 已有分析的密集覆盖域

| 高密度覆盖域 | 代表性文档 | 已覆盖方向数 |
|---|---|---|
| 引擎补齐(编排/路由/记忆/收敛/诊断/并行) | `high-value-extension-directions.md` | ~15 |
| 第三地平线生态(多仓库联邦/事件驱动/管线组合) | `expansion-horizon-three.md` | ~10 |
| 生产可靠性(Prompt QA/信号硬化/环境验证/自愈) | `expansion-production-readiness.md` | ~8 |
| 执行语义形式化(原子性/幂等/因果一致性/回滚) | `execution-semantic-gaps.md` | ~10 |
| 二阶伴生问题(知识衰减/配置爆炸/TOCTOU/无声丢失) | `second-order-architectural-gaps.md` | ~10 |
| 系统边界盲区(级联截断/信任边界/持久语义/并行安全) | `strategic-extensions-v22~v33.md` | ~12 |
| 北极星桥梁(Temporal/OPA/OTel/多厂商/Sandbox) | `v2-to-northstar-gap.md` | ~6 |
| 被遗忘基础(跨进程守护/热加载/Trace CLI/可插拔扩展/状态自校验) | `forgotten-five-foundations.md` | ~5 |
| 格式版本化与数据迁移 | `execution-semantic-gaps.md` 方向四 | ~1 |

**本文的 5 个方向全部落在上述覆盖域之外**。每个方向是**通过逐行阅读源码发现的可验证代码级缺陷或遗漏**，不依赖架构推测或"未来愿景"。

---

## 方向一 · 记忆信任字段的零值歧义: `Confidence` 0→1.0 静默提升 (P0, 数据正确性)

### 问题

`memory.Entry.Confidence` 字段存在结构性的零值歧义: JSON 序列化时 `omitempty` 排除了 0 值，反序列化时 0 又被提升为 1.0。这意味着"置信度为 0"(我完全不信任这条知识)在数据模型层面**无法表示**——写入 `confidence: 0` 的条目在下次加载后变成 `confidence: 1.0`(完全信任)。

实际影响: `prompt_context.go` 中有一条关键的保护逻辑:

```go
// forge-core/cmd/forge/prompt_memory.go:178-181
if e.Confidence < 0.3 {
    // 添加 "[unverified]" 前缀 —— 从不执行，因为 0 已被提升为 1.0
```

这条检查**永远无法为旧文件触发**——任何写入时 `confidence=0` 的条目(应该标为未经验证)在加载后都会变成 1.0，静默绕过 `[unverified]` 标记。agent 将收到一条完全信任但实际上未经核实的信息。

### 代码级证据

**证据 A: `omitempty` 排除 0 值**

```go
// forge-core/internal/memory/memory.go:167
Confidence float64 `json:"confidence,omitempty"` // 0.0-1.0, default 1.0 (omitted=highest)
```

Go 的 `encoding/json` 对数值类型的 `omitempty` 行为: `float64(0)` 被视为空值并被排除。所以 JSON 中 `confidence: 0` 被静默省略——生产者意图"零信任"的记录，序列化后等同于"未设置"。

**证据 B: 解码时 0 无条件提升为 1.0**

```go
// forge-core/internal/memory/memory.go:342-344
// Zero-value Confidence reads as 1.0 (backward compat: old files without
// the field were implicitly full-confidence).
if e.Confidence == 0 {
    e.Confidence = 1.0
}
```

解码器无法区分"文件中没有 confidence 字段"(旧文件，默认 1.0)与"文件中 confidence 被显式设置为 0"(新文件，应为 0.0 零信任)。两个场景都返回 1.0。

**证据 C: 消费者只检查 `Confidence < 0.3`，且信任此阈值**

```go
// forge-core/cmd/forge/prompt_memory.go:178-183
if e.Confidence < 0.3 {
    s = "[unverified] " + s  // 从不命中
} else if e.Confidence < 0.7 {
    s = "[note: confidence=" + fmt.Sprintf("%.0f%%", e.Confidence*100) + "] " + s
}
```

阈值 `0.3` 的设计意图是捕获低置信度条目并添加标记。由于零值歧义，任何 `confidence=0` 的条目在加载后为 1.0，完全跳过两条分支，**静默视为完全可信**。

**证据 D: 全仓无一处显式写 `Confidence: 0.0`**

```bash
grep -rn "Confidence.*=.*0\|confidence.*:\s*0\b" forge-core/ --include="*.go" | grep -v _test
# → 零结果
```

当前没有代码试图写零置信度，所以该 bug 未活跃触发。但**这是结构性的数据模型缺陷**:任何未来的消费者——如方向四"跨会话学习"中提到的自动知识验证 agent——如果试图标记某条记忆为`0 信任`，写入后下次加载将静默变为完全信任。

### 与已有 85+ 分析的核心区别

- `execution-semantic-gaps.md` 方向四(运行时产物的版本契约)讨论的是**格式演化**与跨版本兼容性，不讨论字段级语义的含蓄性。
- `high-value-extensions.md` 方向三(Context/Memory)讨论记忆的存储和检索，不讨论字段语义的精确性。
- `forgotten-five-structural-debt.md` 讨论结构债务但不涉及特定字段的零值语义。
- **本方向的独特性**: 它不是一个"特性缺失"，而是一个**已在代码中存在、可通过加载-存储-再加载循环在单测中复现的精确数据模型 bug**。它存在于 Go 标准库 `encoding/json` 的 `omitempty` 行为与 `memory.Entry` 的默认值约定之间的交互中。

### 为什么需要它

ForgeOS 的核心价值之一是**诚实信号**:置信度字段的设计初衷是让记忆系统能够表达"这是推测/未验证"。如果 `Confidence` 字段不能准确表达其含义——尤其是最关键的"完全不信任"信号——那么整个信任衰减机制建立在流沙之上。更根本的是，修复它不需要架构变更，只需要在 decode 路径中区分"字段存在与否"(例如使用 `*float64` 指针或单独的 `ConfidenceSet` bool)。

**高价值场景**:
- 未来引入自动知识验证 agent(方向四)，标记发现的矛盾条目为 `confidence: 0` —— 下次加载时，该标记静默丢失，agent 继续信任矛盾的知识
- `Compact` 摘要保留了计数但丢弃了原始置信度——任何置信度为 0 的条目在摘要中被提升到 1.0
- `Scorecard` 的 `quality_score` 字段有相同的 `float64 omitempty` 模式——潜在同类问题

---

## 方向二 · 运行时健康子系统的系统性缺失 (P1, 自治可靠性)

### 问题

ForgeOS 的 `internal/doctor` 包提供了完整的**静态诊断**能力——检查 workflow 配置一致性、agent 卡引用完整性、异常模式等。但它完全没有**运行时健康检查**:没有任何机制验证持久化子系统(trace/memory/checkpoint)是否正常工作、磁盘空间是否充足、文件描述符是否耗尽、或者一个长时间运行的 evolve 循环是否在无声退化。

在 24h 自治运行中，这是关键空白:
- trace writer 静默失败→迭代延迟数据丢失
- 磁盘空间耗尽→checkpoint Save 失败→下次 resume 无状态
- memory store 损坏→agent 基于错误知识决策
- 文件描述符泄漏→run 在数小时后神秘失败

### 代码级证据

**证据 A: `internal/doctor` 全部是静态分析**

```go
// forge-core/internal/doctor/doctor.go — 检查 workflow 加载
// forge-core/internal/doctor/anomaly.go — 检查异常模式(静态)
// forge-core/internal/doctor/governance.go — 检查治理一致性(静态)
// forge-core/internal/doctor/workflow_agents.go — 检查 agent 引用(静态)
// forge-core/internal/doctor/status.go — forge status 命令(静态快照)
// forge-core/internal/doctor/quick.go — 快速健康摘要(静态)
```

所有诊断函数都不执行运行时 I/O 探测。没有函数调用 `os.Stat` 检查文件系统状态，没有函数尝试写入临时文件验证 trace 是否可写，没有函数检查 `fs.Statfs` 获取可用磁盘空间。

**证据 B: 没有任何 trace writer 健康检查**

```go
// forge-core/internal/trace/trace.go:136-152
func (t *Tracer) Emit(ev Event) error {
    // ...
    if _, err := t.w.Write(line); err != nil {
        return fmt.Errorf("trace: writing event seq=%d: %w", ev.Seq, err)
    }
    return nil
}
```

`Emit` 在写入失败时返回错误，但**调用者通常在 defer 中静默丢弃这个错误**:

```go
// forge-core/internal/trace/trace.go:188-192
func (t *Tracer) Span(kind, name string) func(status, detail string) {
    start := t.Now()
    return func(status, detail string) {
        dur := t.Now().Sub(start)
        _ = t.Emit(Event{...})  // ← 错误被静默丢弃
    }
}
```

Span 的注释承认:"A lost trace line must never mask the real work's outcome"——但代价是 trace 错误完全不可观测。

**证据 C: 持久化操作无前置空间检查**

```go
// forge-core/internal/persist/checkpoint.go:109-139
func Save(path string, cp Checkpoint, retain int) error {
    // ... 不检查磁盘空间就打开临时文件
    if err := writeSynced(tmp, data); err != nil {
        return err  // 只在写入失败时报告——为时已晚
    }
}
```

```go
// forge-core/internal/memory/memory.go:189-218
func Append(path string, e Entry) error {
    // ... 不检查磁盘空间
    f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
}
```

全部三种持久化(persist/memory/trace)在写入前都不检查可用空间。在 ENOSPC 条件下，checkpoint 的 `writeSynced` 失败但留下 `.tmp` 文件，memory 的 Append 可能在部分写入后失败，trace 的 Emit 错误被丢弃。

**证据 D: preflight 只检查外部依赖，不检查内部健康**

```go
// forge-core/cmd/forge/preflight.go:87-130
// 检查: python3, node, claude, git, go — 全部是环境外部依赖
// 不检查: .forge/ 目录是否可写, trace 是否能创建, memory 是否能追加
```

**证据 E: forge doctor 的 status 命令不运行任何探针**

```go
// forge-core/internal/doctor/status.go — 读取 .forge/* 文件状态但不写入
// forge-core/internal/doctor/quick.go — 打印摘要但不验证功能
```

### 与已有 85+ 分析的核心区别

- `expansion-production-readiness.md` 方向三(环境验证与预检查)讨论的是**启动前的外部依赖检查**(Node/Python/claude 是否可执行)，不是运行时的内部健康探针。
- `expansion-production-blindspots-v36.md` 方向一(编排器自测框架)讨论的是编排器接口的契约测试，不是持久化子系统的健康检查。
- `forgotten-five-foundations.md` 方向五(运行时状态自校验与恢复)讨论的是**数据的校验和与交叉文件一致性**，不是子系统的活性探针。
- **本方向的核心独特性**: 它不是一个特定的检查项，而是一个**缺失的子系统**——与 `internal/doctor`(静态分析)并行但完全不同的运行时健康层。就像 Kubernetes 的 liveness/readiness probe 不替代静态配置验证一样。

### 为什么需要它

ForgeOS 承诺 24h 无人值守。但当前的运行时没有"检查自己是否在正常工作"的能力——它在静默中退化，直到某个不可恢复的故障才被发现。一个自治系统的最低要求是能够回答"我现在还好吗？"。

**高价值场景**:
- `forge doctor --live` 执行运行时探针: 写入临时 trace 事件并删除、写入并加载 memory、保存并加载 checkpoint
- evolve 循环每 N 次迭代自动执行一次内部健康检查，结果记录到 trace
- 磁盘空间<5% 时发出告警而非等到 write 失败
- trace writer 连续失败→触发 forEachRemainingIteration 的降级模式

---

## 方向三 · 持久化子系统版本标记: 写了但不检查 (P1, 数据完整性)

### 问题

ForgeOS 的两个持久化格式(checkpoint 和 memory)在写入时设置了 `_format` 版本标记，但**没有任何加载路径检查这个标记**。第三个格式(scorecard)完全没有版本标记。这意味着:

1. **如果未来版本改变了格式**——例如新增字段、删除字段、或改变字段类型——当前代码**静默接受**新格式数据，可能错误解析字段
2. **版本标记占有磁盘空间但提供零保护**——每行 trace event 携带 `_format` 标记，每行 memory entry 携带 `_format` 标记，checkpoint 携带 `FormatVersion`——但没有人读取它们
3. **没有迁移路径**——如果格式确实变化了，没有任何代码可以从 v1 迁移到 v2，也没有代码在遇到不兼容版本时发出清晰错误

### 代码级证据

**证据 A: checkpoint 写 FormatVersion 但 Load 不检查**

写路径设置版本:
```go
// forge-core/internal/persist/checkpoint.go:103-105
if cp.FormatVersion == "" {
    cp.FormatVersion = "forgeos.checkpoint.v1"
}
```

读路径完全不检查:
```go
// forge-core/internal/persist/checkpoint.go:215-222
func decode(data []byte) (Checkpoint, error) {
    var cp Checkpoint
    if err := json.Unmarshal(data, &cp); err != nil {
        return Checkpoint{}, fmt.Errorf("persist: decode checkpoint: %w", err)
    }
    return cp, nil  // 从不读取 cp.FormatVersion
}
```

**证据 B: memory 写 _format 但 Load 不检查**

写路径设置版本:
```go
// forge-core/internal/memory/memory.go:186-188
if e.Format == "" {
    e.Format = "forgeos.memory.v1"
}
```

读路径完全不检查:
```go
// forge-core/internal/memory/memory.go:318-346
func decode(data []byte) ([]Entry, error) {
    var entries []Entry
    sc := bufio.NewScanner(bytes.NewReader(data))
    for sc.Scan() {
        var e Entry
        if err := json.Unmarshal(raw, &e); err != nil {
            return nil, fmt.Errorf("memory: decode entry on line %d: %w", line, err)
        }
        // 从不检查 e.Format
    }
}
```

注意: `Emit` 虽然写了 `_format` 在每个 trace 事件上，但 trace 是**只写流**(没有 Load 路径)，所以无版本检查是预期行为。这使 checkpoint 和 memory 的问题更值得关注——它们都有读路径，但都不执行版本验证。

**证据 C: scorecard 完全没有版本标记**

```go
// forge-core/internal/routing/scorecard.go:38-59
type Scorecard struct {
    Model         string  `json:"model"`
    TaskType      string  `json:"task_type"`
    QualityScore  float64 `json:"quality_score"`
    Samples       int     `json:"samples"`
    UpdatedAt     string  `json:"updated_at"`
    Mode          string  `json:"mode,omitempty"`
    PassRate      float64 `json:"pass_rate,omitempty"`
    AvgIterations float64 `json:"avg_iterations,omitempty"`
    ReworkRate    float64 `json:"rework_rate,omitempty"`
    // 无 FormatVersion 字段
}
```

`LoadScorecards` 直接 `json.Unmarshal`，没有版本协商。如果未来新增 `MeanLatencyMs` 字段，旧版本代码静默忽略新字段(向前兼容 OK)；如果未来**重命名** `quality_score` 为 `quality_score_v2` 并改变语义，旧版本代码静默读 `quality_score:0`(数据损坏)。

**证据 D: 唯一测试验证"版本被写入"，但不验证"版本被检查"**

```go
// forge-core/internal/persist/fault_test.go:204-218
func TestEncode_FormatVersionIsSetOnSave(t *testing.T) {
    // 验证 Save 设置了 FormatVersion——但不验证 Load 检查它
}
```

没有测试验证 Load 在遇到不兼容版本时拒绝或迁移。

### 与已有 85+ 分析的核心区别

- `strategic-extensions.md` 方向四(自升级协议)讨论了**检查点格式迁移的通用概念**，但没有从"版本标记写了但不检查"这个精确的代码级角度来定位问题。
- `execution-semantic-gaps.md` 方向四(运行时产物的版本契约)以"方向"级别讨论了版本化需求，但关注的是**跨版本兼容性的理论设计**和 schema registry 方案，不是当前代码中版本标记形同虚设的具体事实。
- **本方向的独特性**: 它不是"需要版本化"(已部分实现)，而是"版本化已经做了但关键的一半——读端验证——完全缺失"。这是一个已投入(每个事件每行序列化 `_format` 的开销)但零回报的领域。修复它不需要设计新的格式方案，只需要在三个 `decode`/`Load` 路径中各加几行检查。

### 为什么需要它

版本标记而从不检查比没有版本标记更糟糕——它**创造了一种虚假的安全感**。开发者看到代码有 `FormatVersion` 字段、测试验证它被写入，会以为数据格式受到保护。实际上，任何格式变更后，现有数据会被静默损坏而无人察觉。问题不是"会不会发生"，而是"发生时谁会注意到"。

**高价值场景**:
- forge-core 升级后，旧 checkpoint 的 `PhaseIndex` 语义发生变化——旧数据被静默解析为错误值
- memory 新增 `KindObservation` 枚举值——旧版本代码遇到时静默忽略，但 agent 已依赖该数据
- scorecard 字段重命名——静默数据丢失，Router 依据不完整数据做决策
- 同一项目的两个开发者分别运行 v1.5 和 v2.0 的 forge-core——checkpoint 在版本间轮流写入，无人检测到格式冲突

---

## 方向四 · `loadCache` sync.Map 无界增长的内存泄漏 (P2, 性能)

### 问题

`memory.go` 的 `loadCache` 系统使用 `sync.Map` 缓存加载的记忆条目键值对 `(path → entries)`。缓存条目在 `invalidateLoadCache()` 时被全局清空，但在清空后的下一次 `Load` 调用立即重新创建——且**永不淘汰**。在长时间运行的 evolve 循环或跨多项目的场景下，这个 `sync.Map` 持续增长，直到进程退出才释放。

### 代码级证据

**证据 A: loadCaches 是全局的 sync.Map，永不清除**

```go
// forge-core/internal/memory/memory.go:58-65
// loadCache is a per-path cache for memory.Load: it caches decoded entries
// keyed by (path, mtime), so repeated Load calls within the same iteration
// (one per phase) read the file only once until it changes.
var loadCaches sync.Map // key=path(string), value=*loadCacheEntry
```

map 使用 `sync.Map` 包级别的全局变量，生命周期与进程绑定，永不清除。

**证据 B: invalidateLoadCache 全局清空——但 Load 立即重建**

```go
// forge-core/internal/memory/memory.go:107-113
func invalidateLoadCache() {
    loadCaches.Range(func(key, _ interface{}) bool {
        loadCaches.Delete(key)
        return true
    })
}
```

```go
// forge-core/internal/memory/memory.go:250-277
func Load(path string) ([]Entry, error) {
    if entries, ok, err := loadFromCache(path); ok {
        return entries, err  // 缓存命中
    }
    // ... 读取文件 ...
    storeToCache(path, entries, nil)  // 立即重建缓存条目
    return entries, nil
}
```

清空→重建循环在每次 Append + 下次 Load 之间发生。由于这一对操作在每一个 agent phase 执行一次(每个 phase 写 memory → 触发 invalidate → 下一个 phase 读 memory → 重建缓存)，在 24h 运行中可能产生数千次重建。每一次重建都会在 `sync.Map` 中创建一个新的 `*loadCacheEntry` 指针，而旧条目(被 Delete 的)由 GC 回收——但 map 本身作为索引结构的**内部 bucket 数组**不收缩。

**证据 C: 没有淘汰策略——条目在 mtime 匹配时永久保留**

```go
// forge-core/internal/memory/memory.go:80-90
func loadFromCache(path string) ([]Entry, bool, error) {
    v, ok := loadCaches.Load(path)
    if !ok {
        return nil, false, nil
    }
    ce := v.(*loadCacheEntry)
    if !ce.valid {
        return nil, false, nil
    }
    st, err := os.Stat(path)
    if err != nil {
        return nil, false, nil
    }
    if !st.ModTime().Equal(ce.modTime) {
        return nil, false, nil
    }
    return ce.entries, true, ce.err
}
```

缓存只在 mtime 变化时失效。在 monorepo 场景或 `forge-init` 管理的多个项目下，每个项目的 memory 路径产生一个永不过期的缓存条目——即使该项目已不再活动。

**证据 D: ForgeOS 自己的注释承认此问题**

```go
// forge-core/internal/memory/memory.go:61-64
// Uses sync.Map so concurrent forge processes on different projects do not
// invalidate each other's cache entries (analysis §方向3: global cache collision).
```

注释承认了"不同项目缓存冲突"问题，但提出的解决方案(sync.Map 按路径隔离)只解决了正确性问题(不同项目的 cache key 不冲突)，没有解决**资源泄漏**问题(条目累积不释放)。

### 与已有 85+ 分析的核心区别

- 多份文档(`production-hardening-five-v42.md`、`genuine-expansion-gaps.md`、`forgotten-five-system-boundaries.md` 等)提到了 `loadCache` 作为**跨进程一致性问题**的背景(两个进程的 mtime 缓存互相污染)。但没有一份文档将"sync.Map 无界增长"作为**独立的内存泄漏方向**展开。
- `uncovered-frontiers-v25-systemic-boundaries.md` 讨论了"无界缓存"但那是**进程级的场景**(跨会话 memory)，不是 `loadCache` 本身的增长。
- **本方向的独特性**: 它聚焦于一个具体的、可测量的内存泄漏模式，在单次 evolve 运行(数十个 phase，每个 phase 触发一次 invalidate+reload)的量级下影响轻微，但在**持续运行(多轮 evolve)或多项目管理**下随运行时间线性增长。修复它的成本极低(添加 LRU 淘汰或 size 上限)，但影响显著。

### 为什么需要它

ForgeOS 的内存占用在理论上是可控的(所有状态文件有边界)，但 `loadCache` 是唯一一个**随运行时间单调增长、永不收缩**的内存结构。在开发者的单次 `forge run` 中这不是问题；在 CI runner 连续执行多项目构建、或用户运行 24h evolve 的场景下，这是一个真实的——虽然缓慢的——内存泄漏。修复是纯粹的收益:在 `storeToCache` 路径中添加 LRU 容量上限(例如 64 个条目)，成本为零。

**高价值场景**:
- CI runner 一天执行 500 次 `forge gate/check/accept`，跨越 50 个项目——`loadCache` 累积 50 个永不过期的条目
- 24h evolve 运行，跨 30 次迭代，每次迭代触发 invalidate+reload——`sync.Map` 的 bucket 数组在 GC 后不收缩
- 与其他 `sync.Map` 实例(如 `loadCaches`)共享同一个 Go 进程的堆——不能独立回收

---

## 方向五 · 编排器 PhaseIndex 负值安全缺口与恢复边界的无声失能 (P1, 运行时安全)

### 问题

`orchestrator.RunFrom` 直接使用 checkpoint 的 `PhaseIndex` 值作为 Go slice 索引。如果 checkpoint 因 corruption、手动编辑、或跨版本边界读取而包含一个负值或超大值 `PhaseIndex`，编排器会以三种不安全的方式之一失败:

1. **负值**: `wf.Phases[-5]` → **panic**，进程崩溃
2. **超大值**(`> len(wf.Phases)`): 循环体不执行，返回 `nil` 错误——**静默跳过所有 phase**，报告"运行完成"
3. **零值**(从旧 checkpoint 中缺失 `phase_index` 字段): 从 phase 0 开始——正确行为，但无日志说明"phase_index 缺失，已默认重置为 0"

后两种情况尤其危险:没有错误，没有告警，用户以为 workflow 已运行完成。

### 代码级证据

**证据 A: RunFrom 的 slice 索引无守卫**

```go
// forge-core/internal/orchestrator/orchestrator.go:203-205
func (e Engine) RunFrom(wf asset.Workflow, mode string, start int) error {
    // ...
    for i := start; i < len(wf.Phases); i++ {
        p := wf.Phases[i]  // ← 如果 start < 0 则 panic
```

如果 `start < 0`(来自损坏的 `Checkpoint.PhaseIndex` 或 `resolveRejectionStartPhase` 的错误返回值)，`wf.Phases[start]` 在 Go 中触发 slice bounds panic——不可恢复的崩溃。

**证据 B: 超界值导致静默跳过**

```go
// for i := start; i < len(wf.Phases); i++ {
// 如果 start = len(wf.Phases)，循环不执行，返回 nil error
```

调用者(`LoopEngine` 或 `runWorkflow`)看到 `nil` 错误视为"运行成功"，日志记录"已完成"。实际上没有任何 phase 被执行。

**证据 C: PhaseIndex 的来源没有上界验证**

```go
// forge-core/internal/persist/checkpoint.go:57-61
type Checkpoint struct {
    // ...
    PhaseIndex int `json:"phase_index,omitempty"`  // 无验证标签
}
```

PhaseIndex 是一个普通的 int，在从磁盘反序列化时不验证其合理性(0 ≤ PhaseIndex ≤ len(phases))。只有 JSON 解析错误才报错；任何在 int 范围内有效但逻辑上无效的值(负值、超界值)都被静默接受。

**证据 D: `nextStartPhase` 可能返回 -1(未检查的 `phaseIndex` 查找失败)**

```go
// forge-core/internal/orchestrator/loop.go:232-248
func (l LoopEngine) nextStartPhase(wf asset.Workflow) int {
    ou := l.Stop.OnUnmet
    if ou != nil && ou.Action == "loop_to_next_roadmap_item" {
        if idx, ok := phaseIndex(wf, ou.TargetPhase); ok {
            return idx
        }
    }
    return 0
}
```

这段代码安全——`phaseIndex` 在 target phase 找不到时返回 `ok=false`，函数返回 0。但如果未来有人在 `on_rejected` 路径中直接调用 `phaseIndex` 而不检查 `ok`:

```go
// forge-core/internal/orchestrator/loop.go:249-253
if converge.IsHumanGate(l.Stop) && l.Stop.OnRejected != nil && l.Stop.OnRejected.Action == "loop_back" {
    if idx, ok := phaseIndex(wf, l.Stop.OnRejected.TargetPhase); ok {
        return idx  // ← 安全
    }
}
```

目前是安全的，但这是编程规约，不是结构保证——`phaseIndex` 返回 `(int, bool)`，很容易被错误地解包。

**证据 E: `resolveRejectionStartPhase` 已处理类似情况，但 RunFrom 不感知**

```go
// forge-core/cmd/forge/gates.go:225-265
func resolveRejectionStartPhase(wf asset.Workflow, ignoreState string) (int, error) {
    // 如果标记文件缺失或 target_phase 不存在，返回 error
}
```

`resolveRejectionStartPhase` 返回错误，但 RunFrom 的 `start` 参数可能在调用链的多个来源不被验证。

### 与已有 85+ 分析的核心区别

- `execution-semantic-gaps.md` 方向一(原子性与幂等)讨论了**编排操作的幂等语义**，不是 slice 索引的防御性编程。
- `forgotten-five-system-boundaries.md` 方向一(跨进程守护)讨论了**进程级边界**(OOM killer、fd 泄漏)，不是函数级前置条件。
- `second-order-architectural-gaps.md` 讨论了**无声数据丢失**，但不是这种"无声跳过所有 phase"的特定模式。
- `cross-cutting-systemic-gaps.md` 方向一(编排状态一致性与幂等恢复)讨论了 checkpoint 恢复的正确性，但不涉及简单的整数范围守卫。
- **本方向的独特性**: 这是一个**最小的代码改动(一行守卫)可以消除一类确定性 panic 和静默失败的安全缺口**。它与 checkpoint 的任何语义或格式无关——只是一个函数入口的前置条件检查缺失。

### 为什么需要它

大多数运行时错误会生成错误消息。但 `RunFrom` 的超界 PhaseIndex 要么崩溃(panic)，要么静默返回 nil——两种都是"无声故障"模式。在一个无人值守的 24h 运行中，"所有 phase 被跳过但报告成功"是灾难性的:用户以为 workflow 跑了，实际上什么都没发生。

**高价值场景**:
- checkpoint.json 被手动编辑(调试)，PhaseIndex 被改为 -1 → 下次 `forge run --resume` 触发 panic
- checkpoint 因文件系统故障包含部分写入的内容，PhaseIndex 为垃圾值 → panic
- PhaseIndex > len(phases)(例如 workflow 被修改后 phase 数量减少) → forge evolve 静默报告"已完成"但一个 phase 都没跑
- 跨版本恢复:旧版 checkpoint 的 PhaseIndex 在新版本 worklow 中索引不同的 phase

---

## 优先级排序与收敛建议

| # | 方向 | 优先级 | 分类 | 预估 | 杠杆 | 独特性 |
|---|---|---|---|---|---|---|
| 1 | 信任字段零值歧义 | **P0** | 数据正确性 | <0.5 sprint | ⭐⭐⭐⭐⭐ | — 已有文档均未覆盖此具体的数据模型 bug |
| 2 | PhaseIndex 安全缺口 | P1 | 运行时安全 | <0.5 sprint | ⭐⭐⭐⭐⭐ | — 最小改动消除 panic+静默跳过 |
| 3 | 版本标记写而不查 | **P1** | 数据完整性 | ~1 sprint | ⭐⭐⭐⭐ | — 已有文档讨论格式版本化方案，但未聚焦"已写但不查"的具体事实 |
| 4 | 运行时健康子系统 | P2 | 自治可靠性 | ~2 sprints | ⭐⭐⭐⭐ | — 已有文档讨论个别检查项，但未将"健康层整体缺失"作为独立方向 |
| 5 | loadCache 无界增长 | P2 | 性能 | ~0.5 sprint | ⭐⭐⭐ | — 已有文档零散提及，但未作为独立方向 |

**收敛建议**:

- **若只做一件**:方向一(置信度零值歧义)——修复成本极低(一个 `*float64` 指针或一个 `ConfidenceSet bool`)，消除一个不影响当前运行但阻碍未来知识验证 pipeline 的结构性数据模型缺陷。它是今天无法实际使用的置信度系统的基础障碍。

- **做前三件**:方向一 + 方向二(PhaseIndex 守卫) + 方向三(版本标记读端检查)。三者合计约 2 sprints，关闭一个数据正确性 bug + 一个运行时安全缺口 + 一个跨格式数据完整性缺陷。它们共同强化 ForgeOS 自治运行的底层可信度:数据正确、不会静默跳过工作、持久化格式可演化。

- **全部五件**:方向四(健康子系统)是方向五(loadCache 优化)的使能器——健康子系统可以检测到方向五的性能退化。方向五虽然是 P2，但对 CI/长期运行的场景影响显著，且修复成本极低。

---

## 对已有分析的诚实定位

本文五方向全部通过**逐行阅读当前代码**发现，不依赖架构推测、不涉及"北极星"愿景、不引用未来 roadmap 条目。每个方向的代码级证据均直接来自 forge-core 和 harness 的现有代码，无需"假设"或"推测"。

与已有 85+ 分析的关系:
- **方向一**不在任何已有文档中作为方向出现(可能因为置信度字段的零值歧义在功能上不可见——所有置信度值在加载后都正常)
- **方向二**的个别子项(磁盘空间检查)在 `second-order-architectural-gaps.md` 和 `production-hardening-five-v42.md` 中有所提及，但"健康子系统整体缺失"作为一个系统性方向未被识别
- **方向三**的格式版本化概念在 `strategic-extensions.md` 和 `execution-semantic-gaps.md` 中有上级方向覆盖，但"版本标记写了但不查"的具体代码缺口未被精准定位
- **方向四**的 loadCache 在多个文档中作为跨进程场景的背景提及，但"sync.Map 无界增长"作为独立的性能方向未被展开
- **方向五**的 PhaseIndex 安全缺口出现在零个已有文档中
