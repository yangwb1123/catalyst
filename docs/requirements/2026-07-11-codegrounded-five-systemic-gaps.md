# ForgeOS: 五维系统性扩展方向 (代码接地分析)

> 本文档基于 2026-07-11 对完整代码库的全局扫描产生。与之前的所有扩展方向文档不同，本文聚焦于**此前未被覆盖的、可代码证伪的子系统级缺口**。
>
> 方法：逐文件通读 forge-core 全部 13+ Go 包 + harness 全套 + 编排器全部代码路径，
> 追踪每个子系统间的数据流/控制流/状态共享边界，寻找**横切多个组件的结构性缺口**，
> 而非单文件内功能缺失。每个方向附带 `file:line` 精确引用。

---

## 方向一 · 并行编排下的 agent-call 预算幽灵消费

> **定位**: `internal/orchestrator/parallel.go:97–104` · `internal/orchestrator/budget.go:13–27` · `internal/orchestrator/command_executor.go:47–55`

### 诊断

`forge evolve --parallel` 的 wave 内并发执行路径存在一条预算会计缺口。`runPhaseParallel`
在每个 agent phase 真正执行前调用 `checkAgentBudget`（`budget.go`），该函数**不可逆地递增**
agent-call 计数器：

```go
// budget.go:13-27
func (e Engine) checkAgentBudget(calls *int) error {
    *calls++  // ← 一旦递增，永不回退
    if e.MaxAgentCalls > 0 && *calls > e.MaxAgentCalls {
        return error("agent-call budget exhausted")
    }
    return nil
}
```

控制流如下（`parallel.go:97-104`）：

```
runPhaseParallel entry
  ├── (1) ctx.Err() 检查          ← 此时 context 可能正常
  ├── (2) checkAgentBudget()      ← 计数器已递增
  ├── (3) checkRunBudget()
  └── (4) runAgentPhase(ctx, ...) ← 内部 check ctx.Done() → 可能已取消
```

当同一个 wave 中另一个 phase 在 (2) 与 (4) 之间失败，`waveCancel()` 被调用（`parallel.go:59-67`），
当前 phase 到达 `runAgentPhase` 内的 `ctx.Done()` 检查时发现 context 已取消并返回。但
`checkAgentBudget` 的递增**无法回滚**——该 budget slot 被幽灵消费，永不可回收。

**连锁影响**：在 N-phase wave 中，M 个被取消的 phase 持续消耗 `--max-agent-calls` 预算，
使得后续 wave 的有效可用预算从 `cap - N×wave_count` 降至 `cap - N×wave_count + M`，
可能提前触发 budget exhaustion 拒绝正确 phase 的 spawn。

**为什么这是新缺口**：此前所有分析覆盖了预算上限的存在性（方向四 PR4-6 ·
`runBudgetUSD`/`checkRunBudget`），但**从未检查并行路径下预算会计的原子性**。
`RunFrom`（串行路径）不存在此问题——phase 执行是原子的，无并发取消窗口。

### 修复方向

在 `runPhaseParallel` 中，当 phase 因 `ctx.Err()` 被取消后，对 `agentCalls` 做反向递减
（在 `mu` 保护下）。或改为在 phase 实际完成后再递增计数器（而非在执行前「预留」），
但这会改变 agent-call budget 的语义——从「spawn 前查预算」变成「完成后退费」。

### 边界影响

- 纯串行路径（默认）——零影响
- `--parallel` + `--max-agent-calls`=0（无限）——会计仍在运行（统计不准确）但永不触发拒绝
- 只有 `--parallel` + 正数 `--max-agent-calls` 且 wave 内 phase 失败——命中此缺口
- 当前 4 个 workflow 均无 `depends_on`，故并行路径默认休眠——缺口**今天不活跃，但第一
  个接入 `depends_on` 的 workflow 就会激活**

---

## 方向二 · Memory 压缩与并发 Append 的静默数据丢失

> **定位**: `internal/memory/memory_compact.go:44–74` · `internal/memory/memory.go:65–83` ·
> `internal/memory/memory_compact.go:28–50`(rewriteStore)

### 诊断

`Compact()` 函数执行读-处理-写三步骤，但**写阶段使用基于快照的全量替换**，对并发写入
零容错：

```
// memory_compact.go:44-74
func Compact(path string, ...) (removed int, compacted bool, err error) {
    entries, err := Load(path)         // (1) 读取全部条目 → 快照 A
    ...
    all := append(recent, compacted...)  // (2) 处理快照 A
    ...
    err = rewriteStore(path, all)       // (3) 原子替换文件 → 写入快照 A'
}
```

`rewriteStore` 使用写临时文件+rename 的模式，确保文件本身不损坏。但如果在 (1) 结束时
另一个 goroutine（或 process）调用 `Append()` 写入了一条新条目，该条目落在磁盘文件上
但**不在快照 A 中**。随后 (3) 的 rename 用 `A'`（不含新条目）覆盖了包含新条目的文件。
**新条目永远消失**。

该窗口在以下场景中真实存在：

- **`forge evolve` 迭代边界**：第 N 次迭代的收敛信号采集完成了 memory 的 `Query`，
  同时第 N+1 次迭代的 agent phase 在并行写 memory（`Append` 发现/决策）。
  如果 `Compact` 恰好在第 N 迭代的收尾处触发（例如用户手动 `forge memory-prune`），
  窗口打开。
- **`--parallel` 模式**：多个并发 agent phase 各自独立 `Append` memory，同时
  `Compact` 在任一时间点触发——窗口概率线性放大。

**为什么这是新缺口**：此前方向四覆盖了 checkpoint 的原子性（`persist.Save` 的 temp+rename），
方向五覆盖了 memory 的 append/load 原语。但**从未审查 `Compact` 在并发 Append 下的安全性**。
`Compact` 的 `invalidateLoadCache()` 只影响内存缓存，不修复磁盘数据丢失。

### 修复方向

三个方向之一：
1. **文件级锁**：在 `Compact` 期间获取一个与 memory 文件绑定的排他锁（如 `flock` 或
   `LockFile`），阻止并发 `Append`——但这会阻塞 evolve 循环。
2. **增量压缩**：不重写整个文件，而是在文件末尾追加一个「compress to N entries」指令，
   由 Load 在读取时按指令折叠旧条目——类似 LSM-Tree 的 compaction。
3. **读-改-写校验和**：`Load` 时记录文件 mtime 和 size，`rewriteStore` 前检查两者是否改变；
   若改变则重新 Load/merge，重试直到成功（乐观锁）。

### 边界影响

- 不使用 `forge memory-prune`（或 `Compact` 未达到 `DefaultCompactThreshold=500`）
  的运行——零影响（Compact 是 no-op）
- 短运行（< 500 memory entries）——零影响
- 24h evolve 长跑 + 密集 memory 写入——首次触发 Compact 时即可能命中
- 手动 `forge memory-prune` 与 evolve 循环并发——最高风险窗口

---

## 方向三 · `loadWorkflow` 双解析器语义不对称（未检测的静默偏差）

> **定位**: `forge-core/cmd/forge/main.go:136–167` ·
> `forge-core/internal/yaml2json/normalize.go:57-96`(block scalar handling) ·
> `forge-core/internal/yaml2json/scalar.go:15-58`(boolean/number parsing)

### 诊断

`loadWorkflow` 维护一条双解析器链：Go 原生的 `yaml2json` 先尝试，失败后回退到
`python3 harness/yaml2json.py`。但两个解析器**处理不同 YAML 子集，且没有交叉验证**：

```go
// main.go:136-167
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    // 先试 Go 解析器
    val, err := yaml2json.Decode(f)
    if err == nil {
        ... return wf, nil  // Go 解析成功 → 直接使用，无二次校验
    }
    // 失败后回退到 Python shim
    out, execErr := exec.Command("python3", shim, ymlPath).Output()
    return asset.LoadWorkflowJSON(out)
}
```

已知的分歧点（通过代码阅读而非猜测确认）：

| 构造 | Go yaml2json | Python PyYAML | 影响 |
|---|---|---|---|
| `key: No` | `false` (Go 的 yes/no/on/off 集合) | 字符串 `"No"` (PyYAML 默认只认 `true/false/yes/no`) | 字段值语义改变 |
| `key: OFF` | `false` | 字符串 `"OFF"` 或 `false`（依赖版本） | 同上 |
| `key: \n  \n  val` | `"\nval"` (前导空行保留) | 行为 YAML 1.1 vs 1.2 不同 | 描述文本多前导换行 |
| `key: >+` | 正确保留最终换行 | 正确保留最终换行 | 一致（但基于代码检查确认） |
| `key: \n    \n    v` | `"v"` (文档模式，`normalize.go` 行号连续) | 同上 | 一致 |
| `key: "192.168.1.1"` | 字符串（引号保留） | 字符串 | 一致 |
| `key: 192.168.1.1` | 未加引号→isNumeric 失败→字符串 | 字符串 | 一致 |
| `key: 1_000_000` | `isNumeric` 含 `_` 失败→字符串 | PyYAML→int | Go 解析为字符串，Python 为数字 |

Go 解析器**无验证性回退**——只要 Go 不报错，结果直接被采用，Python 永不介入。
在 CI 环境（可能缺少 `python3`）中，Python 回退甚至**不可用**，Go 解析器的
任何边缘语义偏差都是静默的、不可恢复的。

**为什么这是新缺口**：此前所有文档覆盖了 Sprint 27 的 yaml2json block-scalar 修复和
差分测试（`TestToJSON_MatchesPythonShim`）。但差分测试只在单测中跑一组固定 fixture，
**不覆盖整个双解析器生产路径**——它不回答「当 Go 与 Python 对同一 YAML 产生不同
解析树时，哪个值被送进了 orchestrator？」

### 修复方向

1. **交叉验证**：Go 解析成功后，如果 Python shim 可用，也运行它并 `json.DeepEqual` 比较
   两个结果。不一致时发出明确警告（不阻断，但可观测）。
2. **单事实源迁移**：当 Go 解析器成熟到覆盖所有 ForgeOS 内 workflow 时，移除双重路径——
   消除永久的差异可能性。
3. **fixture 扩展**：将 forge-core 自身的所有 5 个 workflow 文件纳入差分测试套件，
   而不仅是示例 fixture。

### 边界影响

- 本仓的 5 个 workflow 当前在两个解析器下均通过（Sprint 27 已验证）——**今天零影响**
- 用户自定义 workflow 使用 YAML 边缘构造——静默语义偏差
- CI 环境缺 `python3`——回退路径不可用，Go 解析器输出为唯一真相

---

## 方向四 · `yaml2json` 未实现显式缩进指示符（`|2`/`>+2` 等 YAML 1.1 构造）

> **定位**: `forge-core/internal/yaml2json/normalize.go:57-96`(`isBlockScalarIndicator` · `parseBlockHeader`)

### 诊断

YAML 1.1 规范（ForgeOS 的 workflow 目前使用 YAML 1.1 风格）允许在块标量指示符
`|`/`>` 后附加显式缩进指示符（数字）和/或截断指示符（`-`/`+`）。合法组合包括：

```
key: |2        # 内容缩进为 2 的 literal block
key: >+        # folded block，保留尾随换行
key: |-        # literal block，移除尾随换行
key: |+2       # literal block，缩进 2，保留尾随换行
key: >-1       # folded block，缩进 1，移除尾随换行
```

当前 `isBlockScalarIndicator` 仅识别这些后缀（`normalize.go:62`）：

```go
var blockScalarSuffixes = []string{"|-", "|+", ">-", ">+", "|", ">"}
```

`|2`（不带截断指示符的纯数字）**不在列表中**。当 `trimmed` 为 `"description: |2"` 时：
- `isBlockScalarIndicator` 检查 `strings.HasSuffix(trimmed, "|")` → **匹配**（因为
  `"description: |2"` 以 `|` 结尾？不对，它以 `2` 结尾）。

等一下，让我重新检查。`trimmed` 是 `"description: |2"`。`strings.HasSuffix(trimmed, "|")`
检查是否以 `"|"` 结尾——答案是否。它以 `"2"` 结尾。但 `strings.HasSuffix(trimmed, ">")`
也否。所以**所有显式缩进指示符都不匹配**。

实际效果：对 `"description: |2"`，`isBlockScalarIndicator` 返回 `false`。
`normalizeLines` 将其作为普通行处理（`trimmed != ""`，进入 else 分支），
作为标量值 `"|2"` 附加到 `description` 键。下一行（原本是块内容）被当作独立
的映射条目或序列项处理，**造成完全错误的解析树**。

如果 Python shim 可用且 Go 解析器对 `|2` 这样的非标准行报错（格式错误 YAML），
fallback 链可以救回来。但 Go 解析器对 `description: |2` **不报错**——它合法地
解析为 `{"description": "|2"}`，然后解析下一缩进行失败或产生错误嵌套。
这**不会触发 Python fallback**（因为 Go 解析器没有返回 `err`）。

**为什么这是新缺口**：Sprint 27 修复了块标量的前缀注入（`> ` → `""`），但从未
触及显式缩进指示符。ForgeOS 自身 workflow 不使用该构造，但它是一个合法的 YAML 1.1
构造，任何用户 workflow 使用它都会静默得到错误解析。

### 修复方向

1. 扩展 `parseBlockHeader` 以处理可选的数字后缀：先用正则去掉尾随的 `[-+][0-9]*` 模式，
   再匹配基础指示符。
2. 提取数字作为 `contentIndent` 覆盖值传递给 `consumeBlockScalar`（替代自动从第一行
   推导）。
3. 无显式缩进时保持当前自动推导行为不变（后向兼容）。

### 边界影响

- forge-core 自身 5 个 workflow——零影响（均不使用显式缩进）
- 使用简单 `|`/`>` 的用户 workflow——零影响
- 使用 `|2`/`>-`/`>+` 等的用户 workflow——解析完全错误，静默产生错误
  的 phase 描述/字段值
- Python shim fallback：Go 解析成功（但产生错误值）时不触发——双重失效

---

## 方向五 · 复合信号管道中的数据竞争：`FileDelta`/`CodeTestRatio` 与并发 Agent 编辑的竞争

> **定位**: `forge-core/cmd/forge/gates.go:260-306`(`computeCodeTestRatio`) ·
> `gates.go:308-394`(`computeFileDelta`) · `forge-core/internal/converge/converge.go:23-40`(`Signals`)

### 诊断

`gatherSignals` 在每个迭代结束时计算两个独立诚实性信号——`FileDelta` 和
`CodeTestRatio`——均通过 `git diff HEAD` 采集。这两个函数在串行路径下表现良好。
但在 `--parallel` 模式下，**同一 iteration 内的多个 agent phase 正在同时编辑工作区
文件**。`gatherSignals` 在迭代末尾被调用，其 `git diff HEAD` 输出捕获的是**不确定的
多 agent 编辑中间快照**：

```
示意图（`forge evolve --parallel` 单次迭代）：

  时间轴 →
  相位 A ──────[编辑 payment.go]──[完成]──
  相位 B ──[编辑 auth.go]────[完成]────
  相位 C ──────[编辑 tests]───[完成]──
                                ↓
                  gatherSignals() ← git diff HEAD
                  ↓
                  FileDelta / CodeTestRatio 基于不确定快照
```

这产生三个具体问题：

1. **`CodeTestRatio` 的不确定值**：若相位 A 的代码编辑被 diff 捕获但相位 C 的测试
   编辑尚未完成，ratio 偏向生产代码→触发诚实性假阳性警告
   （`"test-gap warning — changed lines are 100% production code"`）。

2. **`FileDelta` 的假阴性/假阳性**：完成快的 agent 的 roadmap 勾选被计入，但其对应
   代码改动尚未进入 diff→`FileDelta<0.3` 触发
   `"roadmap high but file changes low"` 警告。

3. **`computeFileDelta` 的 ROADMAP.md 读取与 git diff 的非原子性**：ROADMAP.md
   （决定哪些项「已勾选」）在一次 `os.ReadFile` 中读取，而 git diff（决定哪些文件
   改动匹配）在上述读取前后数十毫秒采集——两者不是同一时刻的状态。并行相位同时
   编辑 ROADMAP.md 和代码文件时，两个采集点的时序不一致产生**逻辑上不可能的信号
   组合**（如 roadmap 显示 8 个勾但 diff 显示 2 个文件变动——因为 ROADMAP 检查在
   时间 T 运行，git diff 在 T+50ms 运行，diff 尚未捕获 ROADMAP 本身的写入）。

**为什么这是新缺口**：此前方向五覆盖了 `computeFileDelta` 的存在性，方向一覆盖了
reviewer 裁决的串行回路。但**从未在并行模式上下文中审查信号采集的原子性**。
`--parallel` 模式下这些信号是**读-读不原子**的（ROADMAP 读取在时间 T，git diff 在
T+δ），产生逻辑不可达的收敛报告。

### 修复方向

1. **git stash 式快照**：在迭代开始时 `git stash push --include-untracked` 捕获
   起点状态，迭代结束时 `git diff --name-only stash@{0}` 获取该迭代的精确原子变动集。
   （解决 ROADMAP.md 与代码 diff 的非原子性问题。）
2. **相位级追踪**：为每个并行相位记录其编辑的文件列表（从 `CommandExecutor.Observe`
   的输出推断或从 agent 卡声明的 `emits` 字段获取），在 `gatherSignals` 中合并。
3. **诚实降级**：在 `--parallel` 模式下，对 `FileDelta`/`CodeTestRatio` 添加
   `"(best-effort: computed from non-atomic parallel snapshot)"` 标注，降低其
   告警权重。

### 边界影响

- 串行路径（默认）——零影响，git diff 在 phase 完成后运行，无并发写入。
- `--parallel` + 单 agent phase per wave——零影响（无并发 agent 编辑）。
- `--parallel` + 多 agent phase per wave——所有信号偏差和假阳性。
- 当前 4 个 workflow 均无 `depends_on`——此缺口当前不活跃，但与方向一的 budget
   泄漏同为「第一份 `depends_on` workflow 激活时需一起修复」的预埋技术债。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话风险 | 活跃条件 |
|---|---|---|---|---|
| ① 并行预算泄漏 | **P1** | 正确性 | agent-call budget 在并行取消下不可逆消耗，激活即触发 | `--parallel` + `--max-agent-calls>0` + wave 内 phase 失败 |
| ② Memory 压缩并发丢失 | **P1** | 数据完整性 | 并发 Append 与 Compact 读-改-写周期冲突，数据静默丢失 | `memory.jsonl` > 500 条目 + 触发 Compact |
| ③ 双解析器语义不对称 | P2 | 可靠性 | Go/Python 对同一 YAML 产生不同语义树，无交叉验证 | 用户 workflow 使用边缘 YAML 构造 |
| ④ `yaml2json` 缩进指示符盲区 | P2 | 可靠性 | `|2`/`>+` 等标准 YAML 构造被静默误解析为普通标量 | 用户 workflow 使用显式缩进指示符 |
| ⑤ 并行信号采集非原子性 | P3 | 可观测 | 并行 mode 下 `FileDelta`/`CodeTestRatio` 基于不确定快照，产生假阳性告警 | `--parallel` + 多 agent phase per wave |

### 收敛原则

- **今天拦路虎（方向①+②）**：代价是真实的——数据丢失 + 预算会计失效。建议优先
  修复，因为「第一个 `depends_on` workflow」随时可能出现。
- **可靠性加固（方向③+④）**：代价低（几行正则/几行交叉校验），虽今天不活跃，
  但一旦命中即产生静默错误——比功能缺失更难诊断。
- **信号质量（方向⑤）**：只影响已告警过于激进的可观测性，不破坏正确性。
  可与并行路径扩展一同修复。

> 本文档所有诊断均基于代码阅读（Go 1.24 / Python 3.12 语义），未经运行时
> instrument 验证。`--parallel` 相关的活跃条件（方向①、⑤）需要真 claude 多 agent
> 并行运行才能经验性触发——当前 worktree 无此类 workflow。诚实标注。
