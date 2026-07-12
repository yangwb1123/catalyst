这是一个极为详实的分析文档。我逐条对照实际代码库做了验证，以下是技术审查结果。

---

## 代码验证结果

### ✅ 方向一 (`internal/adr`) — 关键事实错误需修正

你写的是 `internal/adr/adr.go` 有 `Validate(path) []Result` 等函数。**实际代码库中没有这个文件**——`forge-core/internal/adr/` 下**仅有 `adr_test.go`**（ADR 可测试性测试，检查 ADR 决策没有 decay）。`adr.go` 不存在。

```bash
$ find forge-core/internal/adr -type f
forge-core/internal/adr/adr_test.go  # 唯一文件
```

`adr_test.go` 的测试是对 ADR 决策的**代码级验证**（TestADR0001_ZeroExternalDeps、TestADR0004_ReviewWorkflowExists 等），不是文档格式校验器。这意味着：

- **你描述的「文档格式校验器」功能不存在**（没有 `Validate`/`ValidateAll`）
- **但方向一的核心论点依然成立**：ADR 的内容（机读裁决、决策状态）没有被 forge-core 的编排/路由/gate 引擎消费
- **甚至更突出**：因为连基本的格式校验层都没有，`forge validate` 路径中根本没有调用 `internal/adr`

**修正建议**：重写方向一的现状描述为「ADR 目前只有 `adr_test.go` 做代码级的决策验证测试，没有独立的 ADR 文档格式校验器，也没有 ADR 元数据被运行时消费」。

### ✅ 方向二 (`.forge/` 元数据护照) — 代码引证基本准确

检查点 `Checkpoint` 结构体确实有 `FormatVersion string` (`"forgeos.checkpoint.v1"`)，但缺少 `ForgeVersion`、`RunID`、`CreatedAt`、`Checksum`。你关于 `trace.go` 中 `Event` 缺少 run_id 的分析也正确。

**但需要补充一个已存在的机制**：`Checkpoint` 已有 `UpdatedAtUnix int64` 字段和时间戳，以及 `SpentUsdMicros`——这表明 persist 层已经开始往结构化方向走。方向二利用这个已有模式做 `meta.json` 是自然而然的演进。

### ❌ 方向三 (执行报告) — `reportConvergence` 描述过时

你引用的代码（`cmd/forge/main.go:372-385`）不准确。实际 `reportConvergence` 已远不止一行输出：

```go
// 实际代码 (main.go:400+):
fmt.Printf("convergence: %s (%s)\n", verdict(met), wf.Stop.Type)
for _, r := range results {
    fmt.Printf("  [%s] %s — %s\n", mark(r.Met), r.Expr, r.Detail)
}
```

它已经输出了每个 criterion 的详细结果。`Converge` 函数也会输出 `RoadmapCompletion`、`GatesGreen`、`ReviewStatus`、`FileDelta`、`CodeTestRatio` 等。**但方向三的核心论点依然成立**：没有 `forge report` 子命令、没有跨运行 diff、没有机器可读的聚合输出。

**修正**：方向三的代码引证应指向 `cmd/forge/main.go:428-438`（实际位置），并承认已有的信号丰富度（`FileDelta`、`CodeTestRatio`、`Criteria` map）——这些可在 `forge report` 中复用。

### ❓ 方向四 (文件系统相位隔离) — 表述需调优

你描述的 `CommandExecutor.Dir` 设置为 `o.root` 是正确的。但方向四忽略了两个**已存在的隔离机制**：

1. **`SandboxConfig`**：`command_executor.go` 中已有完整的 `SandboxConfig` 结构体（Type/Image/MemoryMB/TimeoutSec），以及注释说明 "This is the extension point for ROADMAP v3's '带外 Sandbox (Firecracker)' direction"。
2. **`readonly` + `--disallowedTools`**：Sprint 31 已实现写权限隔离。

方向四的 L1（post-phase diff capture）和 L2（per-phase git stash）建议是可操作的，但文档应承认 SandboxConfig 的存在，并将 L3 与已有 north-star 规划对齐而非描述为「已在 north-star 中规划」。

### ⚠️ 方向五 (监督者模式) — 可做但优先级分析需再审视

你将其定为 P2。在 Sprint 24-26 真点火记录中，每次暴露的 gap 都依赖人类观察者确实成立。**但一个中间方案被忽略了**：

当前 `LoopEngine.Run` 是同步阻塞调用，但 `cmd/forge` 中已经整合了 `signal.NotifyContext` 做 SIGINT/SIGTERM 处理。在已有 Context 架构上增加一个监督者 goroutine 的代价比你估算的更低（~300 行而非 500 行），因为：
- 进度心跳可以直接复用 `OnIteration`/`OnPhase` 回调（已在 `LoopEngine` 中作为 hook 存在）
- 资源水位检查可以复用 `doctor.go` 中已有的 `Check` 架构
- Supervisor log 可以复用 `trace.go` 的 `Tracer` 写入器

**修正建议**：方向五规模应下调至 ~300 行，优先级可考虑 P1（如果「无人值守」是近期目标）。

---

## 与实际代码库的整体准确率评估

| 方向 | 核心论点 | 代码引证准确率 | 主要问题 |
|------|---------|--------------|---------|
| ① ADR 可执行化 | ✅ 成立 | ❌ `adr.go` 不存在 | 现状描述与事实不符 |
| ② .forge/ 元数据 | ✅ 成立 | ✅ 基本准确 | 忽略 `UpdatedAtUnix` 已存在 |
| ③ 运行报告 | ✅ 成立 | ⚠️ 代码位置过时 | 已有多信号收敛报告 |
| ④ 相位隔离 | ✅ 成立 | ⚠️ 忽略 SandboxConfig | 漏掉已有 north-star 基础 |
| ⑤ 监督者 | ✅ 成立 | ⚠️ 规模估算偏高 | Context hook 复用可降本 |

**总体评估**：5/5 方向的核心论点在代码审查后均成立。但代码引证有 1 处关键事实错误（方向一的 adr.go 不存在）和若干处可优化的细节。建议修正后再做实现决策。

需要我根据修正后的分析生成可执行的实现计划（按你建议的 P0→P2 优先级序列）吗？
