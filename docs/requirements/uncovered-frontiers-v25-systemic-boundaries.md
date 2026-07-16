# ForgeOS — 系统性边界与盲区的五个高价值扩展方向

> **视角**: 资深架构师 / 产品经理  
> **基线**: Sprint 31 全状态，FUNCTIONAL_REQUIREMENTS_AUDIT.md 全部 GAP 已收口，5 引擎落地  
> **前提**: 我已完整通读 40+ 份 `docs/analysis/*.md` 和 `docs/requirements/*.md`，**本报告的每个方向都确认未被任何已有分析覆盖**。核心原则：已有分析聚焦于「加什么引擎 / 加什么功能」，本报告聚焦于**已有代码中隐含的系统性边界假设和数据完整性裂缝**——不是功能缺失，是「系统在它没想过的条件下会怎么失败」。  
> **纪律**: 不写任何代码。每个方向标注具体代码位置 + 证伪已有覆盖。  
> **生成日期**: 2026-07-09

---

## 前言：40+ 分析之外的盲区

已有分析的共同特征：

| 分析类型 | 关注点 | 共同盲区 |
|----------|--------|----------|
| 功能扩展（expansion-*） | 加什么新能力 | **已有代码隐含的假设与边界条件** |
| 质量审计（gap/audit） | 声明 vs 实现差异 | **数据管道在压力下的完整性** |
| 性能分析（perf） | 延迟与吞吐 | **无声的数据丢失** |
| 安全分析（security） | 注入/RFC 合规 | **持久化语义的不一致性** |
| 生产就绪度（production-readiness） | 组合测试与健壮性 | **系统自身的可恢复性** |

**本文的五方向全部位于这些盲区的交集**——它们是数据管道中的裂缝，而非功能层面的缺口。

---

## 方向一：Memory 缓存的 TOCTOU 竞争——一个活的数据新鲜度 bug

> **核心判断**: 这不是「未来的改进空间」，而是**当前代码中一个活的竞态条件**，影响 memory 数据管道的正确性。

### 代码位置

```go
// forge-core/internal/memory/memory.go: Append()
func Append(path string, e Entry) error {
    // ...
    invalidateLoadCache() // ← 第 91 行: 缓存在写之前失效！
    // ...open file, write, close... (第 97-112 行)
}
```

### 问题本质

`memory.Append()` 的缓存失效发生在**文件写入之前**。在 `invalidateLoadCache()` 调用之后、`os.OpenFile(..., O_APPEND|O_CREATE, ...)` 写入完成之前的时间窗口中：

```
goroutine A (Append)              goroutine B (Load)
  │                                 │
  ├─ invalidateLoadCache()          │
  │   (cache is now empty)          │
  │                                 ├─ loadFromCache() → miss
  │                                 ├─ os.ReadFile(path)
  │                                 │   (reads STALE file — Append's
  │                                 │    data not yet written)
  │                                 ├─ decode(data) → old data
  │                                 ├─ storeToCache(entries, err)
  │                                 │   (cache now holds STALE data)
  │                                 └─ returns old entries
  │
  ├─ os.OpenFile(...) ← write here
  │
  (cache is now STALE until next
   invalidation or another Load)
```

**结果**: 即使 Append 成功完成，并发 Load 可能在**整个后续的 memory 访问序列中持续返回过期数据**，因为缓存重新植入了旧数据。

### 为什么未被已有分析覆盖

所有 40+ 分析中，当讨论 cache 时都假设它是**性能优化**。但这里的 cache 不是性能问题——它是**正确性问题**：当 prompt builder (`prompt_context.go` 的 `memoryContext()`) 调用 `memory.Load()` 而另一线程刚完成 `memory.Append()` 时，agent 可能收到过时的知识，做出基于已淘汰决策的推理。

`expansion-deep-analysis.out.md §6a` 分析了并行 checkpoint 的竞争（`checkpoint.json` 的并发 Save），但 memory 的缓存竞争是完全不同的机制——一个是文件级并发写入（写-写竞争），一个是读-写之间的缓存一致性问题。

### 边界情况

| 场景 | 影响 | 严重度 |
|------|------|--------|
| evolve loop 的 iteration N 写入 memory（gap/decision），iteration N+1 立即读取 | iteration N+1 可能看不到 N 的知识 | **高** — 知识遗忘，可能重复发现同一 gap |
| `forge run` 的 reviewer phase 写入 memory verdict，后续 QA phase 读取 | QA phase 可能看不到最新 verdict | 中 — 只在 phase 间并发时触发 |
| 长时间运行的 evolve（24h+）中 memory Append 频繁调用 | 累计效应：每个 Append 后都有短暂的时间窗口 | 低 — 窗口极小（~1ms），但 24h 内累积机会大 |

### 建议方向

1. **修复缓存失效时序**: 将 `invalidateLoadCache()` 移到文件写入和关闭**之后**（而非之前）。这是最小修复（约 3 行代码移动）。
2. **增加竞争测试**: 测试 goroutine A Append + goroutine B Load 的并发场景，验证 Load 始终返回最新数据。
3. **考虑写入后直接 update cache**: 如果 Append 的数据可以直接插入内存缓存（而非简单失效），则可消除竞争窗口。

---

## 方向二：持久化层之间的耐久性语义不一致——Checkpoint 与 Memory 有不同的崩溃安全契约

> **核心判断**: 系统的两个持久化子系统（`persist` 和 `memory`）对数据耐久性做出了不同的隐含承诺，但没有文档化。在 evolve loop 的 crash-recovery 路径中，这种不一致导致**恢复不完整**。

### 代码位置

```go
// forge-core/internal/persist/checkpoint.go: writeSynced()
func writeSynced(path string, data []byte) error {
    // ...Write... → f.Sync() → f.Close() ...
    // 同步文件数据，但不同步父目录的 dentry
}

// forge-core/internal/persist/checkpoint.go: Save()
func Save(path string, cp Checkpoint, retain int) error {
    // ...writeSynced(tmp) → os.Rename(tmp, path) ...
    // Rename 后无目录 fsync
}

// forge-core/internal/memory/memory.go: Append()
func Append(path string, e Entry) error {
    // ...os.OpenFile(..., O_APPEND|O_CREATE) → f.Write(line) → f.Close() ...
    // 注意：没有 f.Sync() — 无 fsync!
}
```

### 问题本质

三种不同的耐久性保证，两个子系统：

| 操作 | fsync 数据 | fsync 目录 | 崩溃后恢复保证 |
|------|-----------|-----------|---------------|
| `persist.Save()` | ✅ `f.Sync()` | ❌ 无 `dir.Sync()` | 数据已落盘，但 rename 后目录元数据可能未同步。如果 rename 的目录 entry 在 crash 中丢失，checkpoint 变**不可见但存在**（inode 有数据，无目录项指向它） |
| `memory.Append()` | ❌ 无 `f.Sync()` | ❌ 无 `dir.Sync()` | **数据在 OS page cache 中但未持久化**。crash 后最近一次 Append 的条目丢失 |
| `persist.Load()` | N/A | N/A | 无此问题 |

**关键不对称**: `persist.Save()` 用于**演进的 checkpoint**（可恢复运行位置），有 fsync。`memory.Append()` 用于**累积的知识库**（gap/decision/lesson），**没有 fsync**。Crash 后，evolve loop 可以从 checkpoint 恢复正确的位置（`persist` 保证），但会丢失最新的 memory 条目——导致恢复后的 loop 缺少关键的上下文（如"方向 A 在迭代 12 已被证明无效"）。

### 为什么未被已有分析覆盖

`expansion-gaps-v7-novel.out.md §4` 确认了 `persist.Save` 的原子写机制（temp+fsync+rename），但从未将其与 memory 的缺少 fsync 做**对比分析**。`edgecases-and-perf.md` 讨论了 fsync 的延迟开销（~1-10ms），但只从性能角度而非数据完整性角度。**不同子系统耐久性保证的不一致性本身是一个系统性风险**，而非某个子系统的单独缺口。

### 边界情况

| 场景 | 当前行为 | 影响 |
|------|----------|------|
| Crash 发生在 `memory.Append` 的 Write 之后、Close 之前 | 条目在 OS buffer 中但未持久化 → **丢失** | 丢失最近的 gap/decision/lesson 条目 |
| Crash 发生在 `persist.Save` 的 Rename 之后、目录 sync 之前 | 数据在磁盘上但目录项可能丢失 → **checkpoint 不可见** | `forge evolve --resume` 看到旧 checkpoint，回退到之前的状态 |
| 磁盘写入顺序重排（写回缓存） | `f.Sync()` 强制顺序，但 `os.Rename` 的目录更新可能先于数据刷盘 | **极低**（ext4/XFS 的 journal 保证事务顺序，但 NFS 不能） |
| High-frequency Append（evolve 每 iteration 写 3+ 条 memory） | 每条完全无 fsync | 累积数据丢失风险：一个 crash 可能丢失~最近 3-5 条 |

### 建议方向

1. **增加 `memory.Append` 的可选 fsync**：增加 `Sync bool` 参数或 `AppendSync()` 方法。evolve loop 的 per-iteration memory 写入走同步路径，非关键写入保持无 fsync。
2. **持久化规格文档化**：在 `persist/` 和 `memory/` 的 package doc 中明确标注每操作的崩溃保证级别（"crash-safe" / "best-effort" / "no guarantee"）。
3. **`persist.Save` 补充目录 fsync**：rename 后对父目录调用 `dir.Sync()`（或 `syscall.Fsync(int(dir.Fd()))`），关闭 rename 的元数据窗口。这是标准 Go 持久化库（etcd/bolt 等）的惯例做法。
4. **Crash recovery 增加 memory 完整性告警**：`forge doctor` 在 memory 文件最后一行未完整结束时告警（类似 `traceCheck` 的逻辑），让操作者知晓可能有数据丢失。

---

## 方向三：可观测性管道的无声故障——观测能力自身不可观测

> **核心判断**: ForgeOS 最昂贵的投资之一是 trace/scorecard/telemetry 管道（Sprint 24-26 真 `--agent-cmd=claude` 端到端坐实）。但这个管道**无法检测自身是否正常工作**。当 trace writer 失败时，所有人继续操作，但数据消失了。

### 代码位置

```go
// forge-core/internal/trace/trace.go: Span()
func (t *Tracer) Span(kind, name string) func(status, detail string) {
    start := t.Now()
    return func(status, detail string) {
        dur := t.Now().Sub(start)
        _ = t.Emit(Event{  // ← ★ 错误被静默丢弃
            Kind: kind, Name: name, Status: status,
            DurationMs: dur.Milliseconds(), Detail: detail,
        })
    }
}
```

以及：

```go
// forge-core/cmd/forge/scorecard_wind.go: readTraceCostEvents()
// 在没有 cost 事件时静默跳过整个 wind-down:
//   "GATE-ON-REAL-COST: if the trace holds NO model-bearing cost event..."
// 但它无法区分「没有成本事件（因为 dry-run）」和「trace writer 失败了所以事件丢了」
```

```python
# harness/check.py: doctor.traceCheck 的等效逻辑
# 只检查最后一行是否以 } 结尾 — 一个被截断但恰巧以 } 结尾的文件通过
```

### 问题本质

`trace.Span()` 返回的闭包用 `_ = t.Emit(...)` 丢弃错误。这是 Go 中处理「不可失败」操作的习惯（如 `fmt.Fprintf`），但 `Emit` 实际进行 JSON 序列化 + 文件写入——两者都可能失败（磁盘满、权限变更、文件描述符耗尽、文件系统只读挂载）。

当 Emit 失败时：
1. **trace 行丢失** — 但调用者完全不知。scorecard 看到更少的成本事件、更少的 iteration 记录、更少的 gate 结果。
2. **doctor 无法检测** — `traceCheck` 只检查文件结构，不验证 Seq 序列的连续性。如果一个中期 iteration 的 trace 丢失了但文件结构完整，doctor 报告「正常」。
3. **scorecard 静默偏置** — scorecard 基于现有的 trace 事件计算 P50/P95 latency、avg_cost_usd。如果 trace 事件丢失，统计值偏向于「幸存」的事件——通常是最便宜、最快、最简单的 phase，因为昂贵的 phase 有更大的时间窗口失败。

`trace.Span` 是系统中最广泛使用的记录方法——它被用于 gate 结果、agent phase 计时、iteration 边界。如果它的错误被静默丢弃，**整个可观测性管道的完整性就是脆弱的**。

### 为什么未被已有分析覆盖

`expansion-directions-v14-operational-trust.md` 讨论了 trace 的哈希链（防篡改/截断检测），但**哈希链不能检测静默丢弃**——一个被丢弃的 trace 事件本来就不存在，没有哈希链来验证它。`execution-semantic-gaps.md` 讨论了 SpanID/ParentSpanID 的结构化改进，但同样假设 trace 事件到达了。已有分析都假设「trace 要么完整到达，要么被检测到损坏」——但这里的问题是**既未到达，也未检测**。

### 边界情况

| 场景 | 当前行为 | 正确行为 |
|------|----------|----------|
| 磁盘满（ENOSPC）：Emit 写入失败 | `_ = t.Emit(...)` — 静默丢弃 | 至少 log 一次告警（非每 event——避免 log 风暴） |
| trace 文件被外部删除（`rm .forge/trace.jsonl`） | 新 Emit 写入已删除文件的 inode（文件仍可写直到 fd 关闭） | 检测 fd 的 link count 变化（`fstat` 的 `nlink`） |
| 并发 parallel mode 中 trace 写交错 | trace.Tracer 有锁，单行正确 | **已正确处理**——但 Span 的错误仍静默丢弃 |
| 管道 writer（非文件 io.Writer）关闭 | Emit 返回 EPIPE，静默丢弃 | 同上 |
| 文件权限变更（`chmod 000`） | 新 Emit 返回 EACCES，静默丢弃 | 同上 |

### 建议方向

1. **`Span` 增加非静默错误路径**: `Span` 接受一个可选的 `onError func(error)` 回调，或通过 `Tracer.OnError` 字段注入告警处理。当 `Emit` 失败时，调用此回调。默认无回调保持向后兼容（仍是 `_ =`），但上层可注入一个 `log.Printf` 或 counter 递增。

2. **doctor 增加 trace 序列连续性检查**: 读取 trace 文件的 Seq 序列，检查是否存在跳跃或中断。当 Seq 序列不连续时报告 WARNING（而非 PASS）。

3. **scorecard 数据完整性标注**: 在 scorecard 中增加 `trace_events_total` 和 `trace_events_expected`（由 LoopEngine 的理论 emit 次数估算）的对比。当实际事件数显著少于预期时，标记 scorecard 数据为 `trust=low`。

4. **可选的数据完整性 watchdog**: LoopEngine 维护一个发射跟踪计数器（expected_events），在 `OnIteration` 回调中对比 trace 文件的实际事件计数。差异超过阈值时输出告警。

---

## 方向四：跨平台可移植性债务——"Host-Independent" 下的 Unix 假设

> **核心判断**: CLAUDE.md 和 BOOTSTRAP.md 声明宿主机无关（"host-independent"），但 forge-core 和 harness 大量使用 POSIX/Linux 系统调用和行为假设。Go 的跨编译能力使问题不被察觉——直到有人在 Windows 上运行。

### 代码位置

| 假设 | 代码位置 | 问题 |
|------|----------|------|
| 信号取消（SIGINT/SIGTERM） | `forge-core/cmd/forge/main.go`: `withSignalCancellation()` | Windows 没有 SIGINT/SIGTERM 信号；使用 `os.Signal` 但 `syscall.SIGTERM` 在 Windows 上是未定义的整数常量 |
| Python 路径硬编码 | `forge-core/cmd/forge/main.go`: `exec.Command("python3", shim, ymlPath)` | Windows 使用 `python`，不是 `python3` |
| 类 Unix 文件权限 | `forge-core/cmd/forge/main.go` + `forge-core/internal/memory/memory.go` + `forge-core/internal/persist/checkpoint.go`: `os.MkdirAll(dir, 0o755)`, `os.OpenFile(..., 0o644)` | Windows 忽略 Unix 权限位，但 `os.FileMode` 类型的行为在跨平台时有微妙差异 |
| `O_APPEND` 原子性 | `forge-core/internal/memory/memory.go`: `os.OpenFile(path, os.O_WRONLY\|os.O_APPEND\|os.O_CREATE, 0o644)` | Windows 的 `O_APPEND` 不是原子的（NFS 也不是）——多进程/线程并发 append 可能交错 |
| `os.Rename` 原子性 | `forge-core/internal/persist/checkpoint.go`: `os.Rename(tmp, path)` | NFS/FUSE/Windows 上的 rename-over-existing 不是原子的。如果目标文件已存在，rename 失败或部分覆盖 |
| `exec.Command` 跨平台路径 | `forge-core/cmd/forge/main.go` + harness | `filepath.Join` 已正确处理，但 `exec.LookPath("python3")` 在 Windows 上失败 |
| JSONL 行终结符 | 所有 trace/memory/checkpoint 文件 | 使用 `\n`（Unix 约定）。Windows 上的行终结符是 `\r\n`。跨平台时文本模式 vs 二进制模式的行为差异可能导致双换行或不可解析的行 |

### 为什么未被已有分析覆盖

`strategic-extensions-v24-uncovered-frontiers.md` 的「方向一」讨论了 Windows Job Object 用于孤儿进程管理（一组特定的 Windows 能力），但未讨论**整个代码库的系统性 Unix 假设**。`expansion-directions.md` 的 sandbox 讨论提到了 `sandbox_darwin.go` / `sandbox_other.go`，同样关注的是特定平台适配而非系统性审计。

**已有分析都是增量式「加一个平台适配」的思维，而不是退后一步审视「整个代码库在非 Linux 平台上会怎么失败」**。

### 建议方向

1. **系统性跨平台审计**: 遍历 forge-core 18 个 Go 包 + harness 39 个模块，标记每个 Unix 假设点。输出一个 `PORTABILITY.md` 清单，按严重度分类：
   - **Blocking**: Windows 上直接编译失败或运行时 panic
   - **Semantic**: 行为不同但不会崩溃（如 `O_APPEND` 原子性）
   - **Cosmetic**: 路径/权限兼容但结果不符合预期

2. **Windows CI 门**: 在 `.github/workflows/forge.yml` 中增加 `GOOS=windows go build ./...` 步骤（不运行测试——没有 Windows runner），确保至少编译不失败。目标：`go vet` 在任何 `GOOS` 下全绿。

3. **平台抽象层最简化**: 对于真正差异大的操作（信号、原子 rename、O_APPEND），抽取到 `internal/platform/` 包，用 build tags 选择实现。形态：
   ```
   internal/platform/
     signal.go          // withSignalCancellation 的平台抽象
     rename.go          // 原子 rename（Unix=os.Rename, Windows=MoveFileEx）
     permissions.go     // 创建文件/目录的权限掩码
   ```

4. **测试文件路径兼容性**: 当前测试使用硬编码 `/` 分隔符。改为 `filepath.Join`，确保在 Windows 上 `os.Open` 不会因反斜杠路径报错。

---

## 方向五：ForgeOS 运行时的自我修复——Doctor 只诊断不治疗

> **核心判断**: ForgeOS 的 evolve loop 可以为所治理的项目自动修复代码（loop-back、retry、checkpoint/resume），但 ForgeOS **自身运行时**没有等价的自我修复能力。`forge doctor` 能诊断问题但从不尝试修复，使得长时间无人值守运行的可靠性受限于运行时数据完整性的最弱链路。

### 代码位置

```go
// forge-core/internal/doctor/doctor.go:
// 每个 Check 返回 (Name, OK, Detail) — OK=false 时不提供 Recovery 建议

// forge-core/internal/doctor/doctor.go: memoryCheck()
func memoryCheck(dotForge string) Check {
    entries, err := memory.Load(memPath)
    if err != nil {
        return Check{Name: "memory.jsonl", OK: false, Detail: err.Error()}
        // ← 检测到 corruption，但不了之——不修复，也不提供修复选项
    }
}

// forge-core/internal/memory/memory.go: Load()
// 整体 fail: 一行损坏 = 全部丢失
func Load(path string) ([]Entry, error) {
    // ...一行 json.Unmarshal 失败 → 返回 error，不清除已解析的行
}
```

以及：

```go
// forge-core/cmd/forge/main.go: tmpResidueCheck()
// 检测到 .forge/*.tmp 残留，但无清理命令
// forge doctor --fix 不存在
```

### 问题本质

当前 `forge doctor` 是**只读诊断**。它报告问题但从不修复。考虑这些真实运维场景：

| 问题 | doctor 报告 | 当前能做什么 | 应该能做什么 |
|------|-----------|------------|------------|
| Memory 文件第 500 行损坏 | `[FAIL] memory.jsonl — decode entry on line 500` | 手动删除第 500 行（丢失整条 memory） | `forge doctor --fix` 自动隔离损坏行，保留 499 行 |
| Trace 文件最后一行被截断 | `[FAIL] trace.jsonl — last line may be truncated` | 手动编辑文件删除最后一行 | `forge doctor --fix` 自动截掉最后不完整行 |
| `.forge/*.tmp` 残留 | `[FAIL] no .tmp residue — 2 leftover temp file(s)` | `rm .forge/*.tmp` | `forge doctor --fix` 自动清理 |
| Checkpoint 文件损坏 | `[FAIL] checkpoint.json — <decode error>` | 从 checkpoint.N 备份恢复或丢弃 | `forge doctor --fix` 自动回退到最近的可用备份 |
| 锁文件残留 | 未检测（无实现） | N/A | `forge doctor --fix` 自动清理过期锁 |

**核心不对称**: ForgeOS 的 evolve loop 可以处理项目代码中的损坏（loop-back = 重新生成损坏的代码；retry = 重新运行失败的 phase），但遇到自己运行时状态的损坏时**只能报告，不能修复**。在 24h+ 无人值守场景中，这意味着任何一次运行时数据损坏都会导致循环终止。

### 为什么未被已有分析覆盖

`expansion-forgeos-meta-governance.md` 方向四（「治理资产健康与衰减检测」）讨论了检测 agent 卡、workflow YAML、policy 文件的衰减——这是**治理资产**的健康。本方向关注的是**运行时数据**（memory/trace/checkpoint/临时文件）的自修复——完全不同的数据类别。

`high-value-extensions.md` 方向一（「闸门自省——谁执法执法者？」）讨论了 gate false-positive 追踪——这是闸门质量的自省。本方向关注的是 ForgeOS 自身进程的崩溃恢复能力。

`fourth-wave-architecture.md` 明确承认 `trace.jsonl: cannot fix (last line corruption is permanent data loss)`——这表明项目已知这个问题但尚未解决。

### 建议方向

1. **`forge doctor --fix` (最小版本)**: 为现有 Check 类型实现修复路径：
   - `memory.jsonl` 损坏 → 逐行重新解码，隔离不可解析的行到一个 `.corrupt` 文件，重新写入干净的版本
   - `trace.jsonl` 最后一行不完整 → 截断到最后完整行
   - `.forge/*.tmp` 残留 → 清理所有 `.tmp` 文件
   - Checkpoint 损坏 → 自动回退到 `checkpoint.json.1`（如果有历史备份）

2. **Memory 加载从「全有或全无」改为「尽最大努力」**: `memory.Load()` 的 `decode()` 函数当前碰到第一行错误就返回全部失败。改为：
   - 跳过损坏的行，收集错误消息
   - 返回所有可解析的条目 + 错误统计
   - 调用方（prompt builder）在注入 memory 时增加 `⚠ memory 文件有 N 行损坏` 的诚实告警
   - `Compact()` 或专门的修复命令写入干净版本

3. **自修复入口**: 在 `forge evolve` 的 loop 中，每次 `memory.Load()` 失败后自动触发修复路径（而非中止循环）：
   ```
   1. memory.Load() 返回 error
   2. 触发 memory.Compact() 或 memory.Repair()
   3. 如果修复成功，记录 trace event kind=memory_repair，继续循环
   4. 如果修复也失败，才中止循环
   ```

4. **Lock file + Graceful shutdown**: 增加一个 `.forge/lock` 文件，在 forge run 启动时写入 PID，退出时删除。崩溃后重新运行时 `doctor` 检测到 stale lock 并清理。这是从「孤儿进程检测」方向的另一种实现路径——非检测孤儿，而是防止孤儿。

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|------|
| `--fix` 自动修复 memory 时写入又崩溃 | 修复过程本身留下损坏 | 修复使用原子写入（temp+rename） |
| 多个 corruption 发生在不同位置 | 修复后数据可能语义上不一致（memory 中缺失第 500 行导致「前一条 supersede 了不存在的条目」） | `filterSuperseded` 已正确处理悬空引用（不阻塞） |
| Checkpoint 和 memory 同时损坏 | evolve resume 从旧 checkpoint 开始，但 memory 被修复到不一致的状态 | 修复顺序：先 memory，再 checkpoint（memory 可以独立重建） |
| 用户故意破坏文件测试自我修复 | 用户可能利用自修复绕过数据完整性 | `--fix` 需要显式调用（非自动），且所有修复记录到 trace event |

---

## 汇总：五个方向的影响矩阵

| # | 方向 | 本质类型 | 影响域 | 严重度 | 代码改动量 | 已有分析覆盖 |
|---|------|---------|--------|--------|-----------|-------------|
| 1 | Memory 缓存 TOCTOU 竞争 | **正确性 bug** | 数据新鲜度、记忆一致性 | **高** — 可能丢失关键 evolve 知识 | 极小（3 行修复 + 测试） | **零** |
| 2 | 持久化耐久性语义不一致 | **架构性缺口** | 崩溃恢复的完整性 | **高** — 恢复后丢失上下文 | 适中（fsync + 文档化） | **零** |
| 3 | 可观测管道无声故障 | **运维盲区** | 所有 trace/scorecard/cost 数据的可信度 | **高** — 依赖可观测性做决策时的错误基础 | 极小（Span 错误路径 + doctor 检查） | 哈希链方向（非同类） |
| 4 | 跨平台可移植性债务 | **架构债务** | "host-independent" 声明可信度 | **中** — 当前无 Windows 用户，但抑制了未来的采用路径 | 大（系统性审计 + CI 门） | 散见于各分析但无系统性审计 |
| 5 | 运行时自我修复 | **能力缺口** | 24h 无人值守运行的可靠性 | **中** — 运行时数据损坏概率低但影响致命 | 中（`--fix` + 尽最大努力加载） | `fourth-wave-architecture.md` 承认不可修复但未提方案 |

### 优先级与依赖关系

```
立即修复 (本 sprint):
  方向一 (Memory TOCTOU) —— 3 行代码移动，消除一个活竞争条件
  方向三 (Trace 错误静默) —— Span 增加 onError，约 20 行

1 sprint:
  方向二 (fsync 一致性) —— memory 可选 sync + persist 目录 sync
  方向五 (self-healing 基础) —— forge doctor --fix 的最小功能集

2+ sprints:
  方向四 (跨平台审计) —— 系统性审计，产生 PORTABILITY.md + CI 门
  方向五 (尽最大努力加载) —— memory.Load 改为 partial-failure-tolerant

  方向二 (目录 fsync) ← 无依赖
  方向四 (跨平台) ← 无依赖
  方向五 (self-healing) ← 依赖方向二（修复功能需要 fsync 保证）
```

---

## 被排除的方向与理由

| 方向 | 排除理由 |
|------|----------|
| Prompt 上下文窗口预算 | 已由 `expansion-production-readiness.md` 方向一覆盖 |
| LLM 输出契约履约验证 | 已由 `expansion-production-readiness.md` 方向二覆盖 |
| 核心链路组合测试 | 已由 `expansion-production-readiness.md` 方向三覆盖 |
| YAML 双解析器可靠性 | 已由 `expansion-production-readiness.md` 方向四覆盖 |
| 收敛信号硬化 | 已由 `high-value-expansion-directions.md` 方向一覆盖 |
| 休眠字段激活 | 已由 `high-value-expansion-directions.md` 方向二覆盖 |
| 跨工作流管线编排 | 已由 `high-value-expansion-directions.md` 方向三覆盖 |
| 自适应预算治理与成本智能 | 已由 `high-value-expansion-directions.md` 方向四覆盖 |
| BM25 检索升级 | 已由 `high-value-expansion-directions.md` 方向五覆盖 |
| ForgeOS 元治理（自身治理） | 已由 `expansion-forgeos-meta-governance.md` 完整覆盖 |
| 多进程并发锁契约 | 已在 `parallel.go` 文档化，非扩展方向 |

---

*分析基于 forge-core 全量源代码扫描（18 Go 包 · 130+ 源文件 · 5 工作流 · 39 harness 文件）*
*交叉验证 40+ 篇已有 docs/analysis/*.md 和 docs/requirements/*.md 确认无重复*
*生成日期：2026-07-09 | 不包含任何代码*
