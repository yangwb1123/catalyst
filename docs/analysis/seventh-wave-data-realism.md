# ForgeOS — 第七次架构扫描：持久化层的数据真实性与可靠性债

> **扫描基准**：`b0c80e4`  
> **视角**：专门考察持久化层（trace / memory / checkpoint）在「真实流量下」的行为验证，而非合成测试  
> **核心方法**：查看测试数据、实际 `.forge/` 内容、代码中的数据处理路径，问「这些路径被用真实数据跑过吗？」

---

## 一、发现：持久化层从未被真实数据验证

### 1.1 `.forge/` 的实际内容分析

当前 `.forge/` 的三个文件：

```
checkpoint.json (184B) — 1 个 JSON 对象
trace.jsonl    (1350B) — 12 行 JSONL
memory.jsonl   (2093B) — 14 行 JSONL
```

对每个文件的内容进行定性分析：

**trace.jsonl 特征**（12 条事件）：

| 特征 | 值 | 含义 |
|------|-----|------|
| `kind` 种类 | 仅 `"iteration"` | 无 phase 事件、无 decision 事件、无 error 事件、无 doctor 事件 |
| `duration_ms` 分布 | 0（9条）或 ~4400-4700（3条） | 9 条是干跑（没真正执行），3 条是真实执行 |
| `detail` 内容 | `"roadmap=100% gates_green=true/false"` | 无人 reviewer VERDICT、无 overload 信息 |
| `status` 值 | 仅 `"ok"` | 无 `"fail"`、无 `"timeout"`、无 `"violation"` |
| `seq` 值 | 多个 `seq=1` | **每条 seq 都在每次运行重置**——重启后 seq 计数器归零，不是全局唯一 |

**memory.jsonl 特征**（14 条记录）：

| 特征 | 值 | 含义 |
|------|-----|------|
| `kind` 种类 | 仅 `"lesson"` | 无 `"gap"`、无 `"decision"`、无 `"insight"` |
| 来源 | 全部带 `(dry-run trajectory)` 后缀 | **100% 来自干跑，零来自真实 agent 输出** |
| 内容模板 | `"iter N: roadmap=X%, gates_green=Y (dry-run trajectory)"` | 模板化字符串，无自由文本 |
| 时间跨度 | `created_at_unix: 1781956677 → 1782516099` | 约 6.4 天，约 2 条/天 |

**checkpoint.json 特征**（1 个对象）：

| 字段 | 值 | 含义 |
|------|-----|------|
| `phase_index` | 缺失（omitempty） | **未启用 per-phase checkpoint**——只有 iteration 级 checkpoint |
| `spent_usd_micros` | 缺失（omitempty） | **未启用 run budget**——干跑没有真实成本 |
| `retain 历史` | 仅 1 个文件 | 无历史 checkpoint——不能做趋势分析 |

### 1.2 测试数据与现实数据的鸿沟

| 维度 | 测试数据 | 现实数据 | 差距 |
|------|---------|---------|------|
| **trace 事件数量** | 3-10 条/测试 | 50-200 条/真实 evolve run | 1-2 个数量级 |
| **trace 事件种类** | `iteration` / `gate` / `converge` | 同上（应为多种） | 无 phase 测试 |
| **memory 记录内容** | 结构化测试参数 | 自由文本 agent 输出 | **格式未测试** |
| **checkpoint 保留** | 未测试 retain 参数 | 默认无 retain | 功能存在但零覆盖 |
| **并发写入** | 串行 | 并行 phase 执行中 | 未测试 `sync.Mutex` 路径 |
| **agent 输出真实性** | 短字符串（"planner balanced"） | 长文本 + 代码块 + Markdown | **截断/编码未测试** |
| **文件轮换** | 未测试 | 未使用 | 功能不存在 |

### 1.3 关键路径的「信任接力」

追踪一条 memory 记录从生成到消费的完整路径：

```
Agent 输出（真实 LLM）
  → cmd/forge: parsePhaseOutput → memory.Append (prompt_memory.go)
    → 写入 .forge/memory.jsonl
      → memory.Load() (prompt_memory.go: boundMemory)
        → promptContext 注入（prompt_context.go）
          → Agent prompt 构建（buildPrompt）
```

这条路径上每一步的「信任接力」：

| 步骤 | 信任假设 | 当前验证 |
|------|---------|---------|
| Agent 输出能被正确截断 | `cappedBuffer` 保留 <10MB | ✅ 测了 |
| Agent 输出能被正确转义为 JSON | `json.Marshal(Event{Detail: output})` | ✅ 标准库 |
| 换行在 JSONL 中正确处理 | Detail 中的 `\n` 不破坏 JSONL 格式 | ⚠️ **未测试** |
| Memory 的 `kind` 正确分类 | `gap`/`lesson`/`decision` 三分类 | ❌ **只有 lesson 被用过** |
| boundMemory 能处理 1000+ 条 | O(N) 过滤性能可接受 | ❌ **无性能测试** |
| 长 memory 注入 prompt 不超限 | prompt 构建限制上下文窗口 | ❌ **未测试** |

---

## 二、五个数据可靠性扩展方向

### 方向 1：Trace 事件模型的完备性检查与迁移

**当前状态**：
trace 只有 `kind: "iteration"` 事件。系统有大量的运行时信号从不在 trace 中记录：

| 信号 | 是否存在 | 是否在 trace 中 | 开发时是否可用 |
|------|---------|---------------|--------------|
| gateresult → verdict | ✅ `OnGateResult` | ❌ | 仅在 stderr log |
| phase completion + latency | ✅ `Observe` | ❌ | 仅在 stderr log |
| agent tier decision | ✅ `phaseTierResolver` | ❌ | 仅在 stderr log |
| overload backoff | ✅ `backoff.go` | ❌ | 仅在 stderr log |
| stale count increment | ✅ `loop.go` | ❌ | 仅 stdout log |
| dead iteration (convergence check) | ✅ `loop.go` | ❌ | 仅在 stdout log |
| `forge doctor` 诊断结果 | ✅ `validate.go` | ❌ | CLI 输出 |

**trace 事件种类应有 8 种，当前只有 1 种**。

**建议方案**：

新增事件 kind：

```json
{"kind":"iteration","phase":"planner","status":"ok","duration_ms":45000,"detail":"...","model":"sonnet-4.2","cost_usd_micros":15000}
{"kind":"decision","phase":"","decision":"downtier","from":"sonnet","to":"haiku","reason":"spend_ratio=0.85"}
{"kind":"gate","phase":"harness-gates","gate":"lint","result":"PASS","detail":"0 issues"}
{"kind":"error","phase":"implementer","error_type":"overload","retryable":true,"detail":"529 overload, retried after 15s"}
{"kind":"doctor","check":"trace.jsonl","result":"PASS","detail":"12 events, last line complete"}
{"kind":"overload_backoff","phase":"reviewer","wait_ms":15000,"attempt":2}
{"kind":"stale_increment","iteration":3,"reason":"roadmap_flat + gate_unchanged","prev_completion":0.92,"gates_green":false}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **事后诊断** | 当前 trace 只能回答「发生了多少次 iteration」——不能回答「为什么 iteration 3 比 iteration 2 慢 10 倍」（可能是 overload_backoff）|
| **趋势分析** | 只有 iteration 事件时，`forge status --trend` 永远是平的 |
| **成本归因** | `cost_usd_micros` 字段存在但只出现在 iteration 事件上——真实成本应该关联到具体的 agent phase（planner vs reviewer），不是整个 iteration |

**边界情况**：

1. **向后兼容**：现有 trace 只有 kind=iteration。新增 kind 后旧 trace + 新代码 = 跳过未知 kind。需要明确的行为
2. **trace 文件格式版本**：当前无 `_format` 字段（checkpoint 有）。trace 需要 `_format` 避免演化后读错
3. **事件顺序保证**：decision 事件必须在引发它的 phase 事件之前还是之后？维护因果顺序对重放至关重要

---

### 方向 2：Memory 系统的真实数据验证与压缩

**当前状态**：
memory.jsonl 中的 14 条记录全部是干跑轨迹模板字符串。memory 的代码路径用真实数据测试过：

- `kind: "gap"` 和 `kind: "decision"` 的 `memory.Append` 从未被执行过
- `boundMemory`（新鲜度+相关性过滤）从未处理过超过 20 条记录
- `prompt_memory.go` 的 MemoryToPrompt 从未格式化过包含代码块的 agent 输出
- 没有 memory 压缩/合并——`Append` 目前只增加，不汇总

**实际数据量估算**：

一个真实的 24h evolve 循环可能产生：

```
每次 iteration: ~3-5 agent phases
每条 phase 可能生成: 1-3 memory 条目
每 iteration: ~10 memory 条目
一天 50 次 iteration: ~500 memory 条目/天
一个月: ~15000 条
```

当前 memory 是**纯线性增长的**。没有压缩、没有过期、没有合并。

**memory.jsonl 的当前注入行为**（`boundMemory`）：

```go
// prompt_memory.go
func boundMemory(entries []Memory, limit int) []Memory {
    // 按新鲜度排序 → 截断前 N 条
    // 在同一新鲜度窗口内，按相关性再过滤
}
```

问题是：这个设计假设 memory 记录的新鲜度随时间递减。但如果一个 agent 输出了 500 字的 gap 描述，新的 gap 会把它挤出窗口——**长尾重要的信息在一次 evolve 循环后就丢失了**。

**建议方案**：

**Phase 1：memory 压缩（`wind` 操作）**：
当 memory.jsonl 超过 500 条时，触发压缩：

```
原始 1000 条 memory:
  - 500 条 "iter N: roadmap=X%..." (高度重复的信息)
  - 200 条 gap 描述（多种多样）
  - 150 条 decision（技术选型）
  - 150 条 lesson（经验教训）

压缩后 100 条:
  - 1 条概要: "共 500 次 iteration, roadmap 从 25% 到 100%, 平均进度 1.5%/iter"
  - 50 条 gap（去重，按优先级排序）
  - 30 条 decision（去重，保留 votes）
  - 19 条 lesson（按 topic 聚类）
```

```go
// 新增 Compaction 操作
type CompactedMemory struct {
    StartAtUnix int64
    EndAtUnix   int64
    Summary     string              // LLM 生成的总结
             GapCount    int
    DecisionCount int
    LessonCount   int
    // 保持最重要的条目
    TopGaps      []Memory
    TopDecisions []Memory
    TopLessons   []Memory
}

func Compact(entries []Memory, limit int) ([]Memory, error) {
    if len(entries) <= limit {
        return entries, nil
    }
    // 1. 按 kind 分组
    // 2. 每种 kind 保留最重要的 limit/3 条
    // 3. 生成摘要条目（kind=summary）
    // 4. 返回 ≤ limit 条
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **上下文预算** | 每条 memory 条目在 prompt 构建中都占用 token。15000 条 memory（即使 boundMemory 过滤到 20 条）其中 95% 可能是高度重复的冗长模板字符串。压缩后保留的信息密度高很多 |
| **LLM 友好性** | 一个 LLM 读 20 条「iter N: roadmap=X%...」比读一条「进度从 25% 到 100%」的总结要难得多 |
| **磁盘增长** | 15000 条 × ~200 字节/条 ≈ 3MB/月。一年 36MB 对当今磁盘来说很小，但无上限增长最终需要运维 attention |

**边界情况**：

1. **压缩触发时机**：在 evolve 迭代之间触发压缩？如果压缩花了 2 秒，迭代间隔增加了 2 秒。如果异步压缩，压缩期间有人写 memory 怎么办
2. **摘要的质量**：LLM 生成的 summary 丢失细节。需要保留原始条目 ID 引用：「关于 payment 模块的 3 个 gap 压缩为 1 条，引用源自 seq=12,45,87」
3. **不可变时间窗口**：最近 24 小时的 memory 不应压缩（可能用于当前 prompt）。只压缩超过 24 小时的旧条目

---

### 方向 3：Checkpoint 历史追溯与临障分析

**当前状态**：
`checkpoint.go` 的 `Save` 函数完全支持 `retain > 0`，但**没有任何调用者使用它**。代码库中所有 `Save` 调用都使用 `retain=0`。

这意味着 checkpoint 总是只有一个文件。没有历史、无法回滚、无法做收敛趋势分析。

`forge doctor` 检查 checkpoint 是否可读，但不检查 checkpoint 是否合理（比如 `iteration=1` 持续了 14 天——暗示 evolve 一直无法推进）。

**检查 checkpoint.gap**：

```
// 当前 checkpoint.json（2026-06-27）
workflow: evolve, iteration: 1, roadmap_completion: 1, gates_green: true
```

这把 checkpoint 当作最新快照。但如果 evolve 在 iteration=7 时 crash 了、用户在 iteration=1 时—resume，checkpoint 知道 resume 到 iteration=1。但 checkpoint 本身没有被设计来回答「crash 之前跑到了 iteration=7，发生了什么？」。

**建议方案**：

**Phase 1：启用 retain**
```go
persist.Save(path, cp, 5)  // 保留最近 5 个 checkpoint
```

产生文件：

```
.forge/checkpoint.json    # 当前
.forge/checkpoint.json.1  # 上一次
.forge/checkpoint.json.2  # 上上次
.forge/checkpoint.json.3
.forge/checkpoint.json.4
.forge/checkpoint.json.5
```

**Phase 2：checkpoint 趋势分析**
```bash
forge status --checkpoint-history
  checkpoint age: 3 天（最近一次 evolve 为 2026-06-27）
  历史 5 个 checkpoint 的趋势：
    iteration: 1 → 1 → 1 → 1 → 1 → 1（无变化——始终只跑了一次 iteration）
    roadmap_completion: 1.0 → 1.0 → ...（始终 100%——任务已满，evolve 在停机待命）
    gates_green: true → true → ...（始终绿）
  诊断：evolve 在 3 天前跑了一次 iteration 后就处于 external stop 状态。
  这是正常的外部停机行为（no_gaps_found），无需 action。
```

**Phase 3：异常检测**
```bash
forge doctor --anomaly
  [WARN] checkpoint: iteration=1 持续 14 天（最后 3 次 checkpoint 都相同）
         → 建议检查 workflow 是否卡在 iteration 1
  [INFO] checkpoint: roadmap_completion 从 0.25 → 1.0 在 2 次 iteration 内
         → 正常收敛速度
  [INFO] checkpoint: spent_usd_micros=0（干跑模式，无真实成本）
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **崩溃归因** | 没有 checkpoint 历史，无法知道 crash 前系统跑了多少次 iteration、进展如何 |
| **收敛速度分析** | 迭代次数 vs roadmap_completion 的曲线是系统效率的核心指标——「多少次 iteration 达到 100%」|
| **恢复验证** | `forge resume` 后能恢复 checkpoint 链。当前 `resume` 只恢复最新快照，不知道它是不是完整的 |

**边界情况**：

1. **磁盘使用**：retain=5 × 每个 ~200 字节 = ~1KB。没问题
2. **敏感信息**：checkpoint 当前不包含任何敏感数据（没有 API key，没有用户数据）。但如果将来添加了 cost 数据，历史 checkpoint 可能暴露成本趋势
3. **检查点过多**：如果每个 phase 都写 checkpoint（`phase_index > 0`），iteration 10 × phase 6 = 60 个文件/run。retain 需要 apply 到文件数，不是 iteration

---

### 方向 4：故障注入与持久层健壮性测试（Chaos Engineering for Data）

**当前状态**：
测试覆盖了「正常路径」和「简单异常路径」（文件不存在 → 返回 ok）。但没有覆盖以下「数据层故障」场景：

| 故障场景 | 当前测试 | 后果 |
|---------|---------|------|
| trace.jsonl 最后一行不完整（截断） | forge doctor ✅ 检测 | 无法检测——如果 doctor 不跑 |
| trace.jsonl 中间行损坏 | ❌ 无测试 | 后续解析跳过损坏行还是报错？未定义 |
| memory.jsonl 某行不是有效 JSON | ❌ 无测试 | Load 是否停止解析还是跳过？ |
| checkpoint.json 某字段类型错误（如 `iteration: "abc"`） | ❌ 无测试 | JSON Unmarshal 返回 error，系统是否 fallback？ |
| 两个 forge 进程同时写 trace.jsonl | ❌ 无测试 | 并发写 → 内容交错 → 损坏 |
| `.forge/` 目录被删除 | forge doctor ✅ 检测 | 运行时创建？还是 Fatal？ |
| 磁盘满 | ❌ 无测试 | Write → error → 系统行为未定义 |
| trace 文件被其他工具意外写入 | ❌ 无测试 | 追加非 JSON 内容 → 解析中断 |

**建议方案**：

一个 `forge-core/internal/persist/fault_test.go` 文件：

```go
func TestTrace_CorruptedLastLine(t *testing.T) {
    // 模拟 trace.jsonl 最后一行是不完整的 JSON
    // 验证 Load 行为：跳过最后一行？报错？自动截断？
}

func TestTrace_MidFileCorruption(t *testing.T) {
    // 模拟第 5 行是纯文本 "hello world"
    // 验证后续解析：跳过第 5 行继续？报错停止？
}

func TestCheckpoint_WrongType(t *testing.T) {
    // 模拟 iteration 字段是 string "abc"
    // 验证 Unmarshal 错误不静默吞掉
}

func TestMemory_ConcurrentAppend(t *testing.T) {
    // 多个 goroutine 同时 Append
    // 验证 JSONL 不被交叠写入
}

func TestCheckpoint_DiskFull(t *testing.T) {
    // 模拟写入失败
    // 验证 Save 不破坏原始文件（原子性承诺）
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **数据完整性** | ForgeOS 承诺 checkpoint 是「原子写入——从不半写」。但承诺需要测试保证。故障注入是唯一验证手段 |
| **自愈能力** | `forge doctor` 可以检测问题。但如果问题发生在 evolve 循环中间（而非启动时），doctor 不跑。系统需要内建恢复——比如 trace 的截断自动修复 |
| **操作信任** | 用户信任 `.forge/` 目录中的数据。如果一个 evolve 跑了 6 小时后因为 trace.jsonl 损坏而报错「无法解析 trace」——用户会失去信任 |

**边界情况**：

1. **故障恢复的层级**：自动修复 vs 报告 vs panic。需要每类故障的协议
2. **幂等修复**：重复跑修复不应产生累积变化
3. **数据丢失与数据错误**：丢失一条 memory 条目比从错误的 memory 条目恢复更好。后者可能注入误导性的 prompt 上下文

---

### 方向 5：真实数据回放（Replay Testing from `.forge/` 目录）

**当前状态**：
测试套件是纯合成构造的——用 `fakeRepo` 创建最小目录结构，用简短字符串模拟 agent 输出。系统的**数据路径从未用真实数据验证过**。

**建议方案**：

```bash
# 捕获真实运行的数据
forge run build --executor command --agent-cmd claude --save-trajectory .forge/replay/trajectory-001/

# 回放验证
forge replay .forge/replay/trajectory-001/
```

replay 验证的路径：

```go
// replay_test.go — 从真实运行数据再现数据路径
// 种子：用 `forge run` 输出构建 fixture（仅一次）
// 验证：每次 CI 跑保证代码变动不破坏数据路径

func TestReplay_TraceLoad(t *testing.T) {
    data := readFixture("trajectory-001/trace.jsonl")
    tracer, err := trace.Load(data)
    // 验证 87 条事件全部正确解析
    // 验证 kind 分布: 3 iteration + 15 phase + 8 gate + ...
    // 验证 seq 增长: 1..87
}

func TestReplay_MemoryInjection(t *testing.T) {
    entries := readFixture("trajectory-001/memory.jsonl")
    // 验证 42 条 memory 被正确分类
    // 验证 boundMemory(50) 返回 ≤50 条
    // 验证 MemoryToPrompt 格式正确
}

func TestReplay_CheckpointResume(t *testing.T) {
    cp := readFixture("trajectory-001/checkpoint.json")
    // 验证 resume 到 iteration=3 时，phase_index=2
    // 验证 resume 后 roadmap_completion=0.75
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **回归保护** | 当前 50 个测试文件都使用合成数据。如果 trace.go 的 JSONL 解析器被重构了，100% 的测试还会通过，但真实 trace 可能解析失败。Replay 测试用真实数据保护回归 |
| **fixture 的持续更新** | 每次重大功能增加，产出一条 replay fixture。随时间推移积累各种真实场景：长 trace、大 memory、crash checkpoint |
| **fixture 的人类可审查性** | 真实运行数据包含敏感信息吗？replay fixture 可以审查和脱敏 |

**Phase 1 的极低成本实现**：

不需要 `forge replay` 命令——只需一组硬编码的 fixture 文件：

```bash
forge-core/internal/persist/testdata/
  replay/
    evolve-dry-run/        # 现有的干跑数据
      trace.jsonl          # 12 events
      memory.jsonl         # 14 entries
      checkpoint.json      # 1 object
    evolve-real/           # 未来的真实运行数据
    build-real/
    discover-real/
```

测试代码：

```go
func TestReplay_TraceParsing(t *testing.T) {
    fixtures := []string{"evolve-dry-run", "build-real", ...}
    for _, name := range fixtures {
        data, _ := os.ReadFile("testdata/replay/" + name + "/trace.jsonl")
        events, err := trace.Load(bytes.NewReader(data))
        if err != nil {
            t.Fatalf("%s: Load: %v", name, err)
        }
        if len(events) == 0 {
            t.Errorf("%s: got 0 events", name)
        }
    }
}
```

**边界情况**：

1. **敏感数据泄露**：replay fixture 中的真实 agent 输出可能包含 API key、密码、用户数据。需要自动化脱敏（替换 `sk-*` → `sk-REDACTED`）
2. **fixture 膨胀**：每次 run 产出一条 5KB fixture，100 次 run = 500KB。可控
3. **版本锁定**：fixture 对 trace 格式变更敏感。版本变更时需要迁移 fixture 或标记为旧格式

---

## 三、已分析的 16 篇 doc 覆盖了什么

```
asset-runtime-gap.md               Workflow YAML vs runtime consumer gap
configuration-surface-and-adoption.md  27 个配置文件的复杂度
edgecases-and-perf.md              并行编排竞态、trace 轮换、治理盲区
expansion-directions.md            Agent 沙箱、多模型路由等
fifth-wave-operational.md          版本/构建/基准/preflight/错误 UX/交互 init
fourth-wave-architecture.md        输出合约/doctor 接入/相位画像/自修复/自演化
go-runtime-health.md               并发模型/配置加载/CLI 工程/跨语言盲区
growth-bottlenecks-and-scalability.md  Go 包指标/测试策略/Node.js 依赖风险
hidden-feedback-and-pipeline-gaps.md  Memory-prompt 循环/运行期表面退化
mqtt-and-wasm-integration.md       MQTT 事件总线/WASM 插件系统
roadmap-blindspots.md              ROADMAP 盲点（贡献体验、文档）
self-testing-and-dogfooding.md     三套测试套件全景
sixth-wave-multimodel.md           三模型平行宇宙漂移
strategic-expansion-and-edge-cases.md  11 项未交付 + 5 方向
third-wave-expansion.md            收敛注册表/运行时 arch/通知/AI-SDLC 桥接
v2-to-northstar-gap.md             15 服务北极星 vs v2
```

**本轮（第 7 次）的独有角度**：以上 16 篇全部关注功能/架构/运维/模型。**没有一篇关注「持久化层的数据真实性和系统从未被真实数据验证过」这一具体缺口**。

---

## 优先级矩阵

| 方向 | 影响面 | 成本 | 前置依赖 | 推荐 |
|------|--------|------|---------|------|
| **1. Trace 事件模型完备** | 诊断/分析：高 | 中 | trace_test.go 扩展 | Sprint n+1 |
| **2. Memory 压缩** | 上下文质量：中-高 | 中 | memory.append 增加触发 | Sprint n+2 |
| **3. Checkpoint 历史** | 运维：中 | **极低**（`Save(..., 5)` 一行改动） | 无 | **Sprint n** |
| **4. 故障注入测试** | 可靠性：高 | 中 | 无 | Sprint n+1 |
| **5. 真实数据回放** | 正确性：中 | 低（一组 fixture + 测试） | 需要一次真实运行产出 | **Sprint n** |

---

*分析日期：2026-06-30 | 第七次全量扫描：从 16 篇既有文档的覆盖域外寻找盲区——持久化层的数据真实性与可靠性债*
