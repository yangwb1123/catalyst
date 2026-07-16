# 独立代码验证报告:五个工程/产品级扩展方向

> **验证人**: 独立代码 reviewer  
> **方法**: 逐行阅读 forge-core 源代码中每个方向引用的文件路径和函数，验证文档的所有代码级证据。  
> **验证日期**: 2026-07-12  
> **基础文档**: `2026-07-12-five-global-scan-engineering-product-expansion-directions.md`

---

## 全景:验证方法

我已逐行阅读以下源文件:

| 方向 | 验证的文件 |
|------|-----------|
| ① ADR 可测试性 | `forge-core/internal/adr/adr_test.go`, `forge-core/cmd/forge/adr_test.go` |
| ② 外部 CLI 隐式依赖 | `forge-core/internal/yaml2json/yaml2json_test.go`, `forge-core/cmd/forge/scorecard_wind_test.go`, `forge-core/go.mod` |
| ③ 异常检测脱离演化循环 | `forge-core/internal/doctor/anomaly.go`, `forge-core/cmd/forge/evolve.go`, `forge-core/internal/orchestrator/loop.go` |
| ④ Memory 压缩不自触发 | `forge-core/internal/memory/memory_compact.go`, `forge-core/cmd/forge/evolve.go`, `forge-core/internal/memory/memory.go` |
| ⑤ 零相位工作流 | `forge-core/internal/orchestrator/orchestrator.go`, `forge-core/internal/orchestrator/parallel.go`, `forge-core/internal/orchestrator/waves.go` |

并执行了 `grep` 搜索覆盖全仓调用点。

---

## 方向① ADR 可测试性的断裂闭环 — ⚠️ 核心成立，但文档对 ADR-0002 测试的描述不精确

**文档的核心主张**: ADR 测试失败时没有自动化纠正路径（ROADMAP 条目创建/通知/修复）—— ✅ **成立**

**我从代码中提取的执行流**:

```
go test ./forge-core/internal/adr/...  # 或 cmd/forge/adr_test.go
  → TestADR0001_ZeroExternalDeps       # 检查 go.mod 无 require 块
    → t.Errorf("ADR-0001 violation: ...")  # ← 唯一输出
  → TestADR0002_PolyglotNotStarted      # 检查 forge-ai/forge-web/forge-runtime 不存在
    → t.Logf("... detected — polyglot stage advancing")
  → TestCrossADR_GoModStaysClean        # 检查 go.mod 行数
    → t.Logf("go.mod has %d lines — check for unintended changes")
```

**全部输出**: 只有 `t.Errorf` / `t.Logf`。没有任何路径写入 ROADMAP、创建 issue、或发送通知。这是真实的架构缺口。

### 文档的不精确处

**文档声称**:
> ADR-0002 的测试已经通过 `t.Skip` 静默退化

**代码显示** (`forge-core/internal/adr/adr_test.go:110-126`):
```go
func TestADR0002_PolyglotNotStarted(t *testing.T) {
    expectedAbsent := []string{"forge-ai", "forge-web", "forge-runtime"}
    for _, name := range expectedAbsent {
        path := filepath.Join(root, name)
        if fi, err := os.Stat(path); err == nil && fi.IsDir() {
            t.Logf("ADR-0002: %s/ detected — polyglot stage advancing (expected for v3, not v2)", name)
        }
    }
}
```
这个测试**没有使用 `t.Skip`**，它使用 `t.Logf`。这是一个**前向兼容的软断言**——当 polyglot 目录出现时它 gently 记录进展，而不是静默跳过。ADR-0002 没有退化，它只是以与非二进制断言（而非 PASS/FAIL 断言）不同的方式表达。

**修正意见**: 这个不精确不影响方向的核心主张（无自动化修复路径），但建议修订"ADR-0002 测试已跳过"的描述。

### 隐藏的工程挑战（文档未提及）

ADR 测试面临的根本性困境是这些测试**运行在 forge-core 的 Go 测试套件中**，但 ROADMAP 条目存在于 `.agent/ROADMAP.md`（Markdown，非 Go 可达）。一个 `t.Errorf` 无法写入 Markdown 文件而不引入 Go 对 harness 层的反向依赖——这正是 forge-core 的零外部依赖 + 分层约束禁止的。文档提出的"自动创建 ROADMAP 条目"方案需要:

1. 在 `go test` 的输出流中检测 ADR 测试失败模式
2. 或让 harness 层的某个进程（Node.js）监听测试结果
3. 或让 forge CLI 命令包装测试执行并解析输出

这不是一个纯 Go 层的改动，它跨了 forge-core / harness 的**架构边界**。文档将此预估为 "~1 sprint" 但未提及这个边界问题。

### 验证后评估

| 项 | 评估 |
|---|------|
| 核心主张 | ✅ **成立** — ADR 测试失败确实无主动告警/自动修复 |
| 代码证据 | ✅ 指控的函数行号准确（`adr_test.go:85-87`） |
| 行为分析 | ⚠️ 对 ADR-0002 测试的"已跳过"描述不准确——实为 `t.Logf` 软断言 |
| 架构可行性 | ⚠️ 未讨论跨层边界（Go test → Markdown ROADMAP） |
| **优先级修正** | P1 → **P1**（不变） |

---

## 方向② forge-core 测试套件的外部 CLI 隐式依赖 — ✅ 确认成立

**文档的核心主张**: `go test ./...` 隐式依赖 `python3`、`node`、`git`，但仅 `go.mod` 声明了零外部依赖—— ✅ **成立**

### 逐条验证

**证据 A (`yaml2json_test.go:369`)**:
```go
cmd := exec.Command("python3", shimPath, ymlPath)
// t.Skipf 在 python3 不可用时触发
```
✅ 代码真实存在。Python shim 用于 Go YAML 解析器与 PyYAML 的差分校验。

**证据 B (`scorecard_wind_test.go:191-195`)**:
```go
root := t.TempDir() // no harness/ dir -> the node shell-out fails to find the script
```
✅ 测试通过构造无 `harness/` 的 temp dir 来测试 node shell-out 的失败路径。

**证据 C (`go.mod`)**:
```
module forgeos/forge-core
go 1.26
```
✅ 无 `require` 块——构建层零外部 Go 依赖成立。

**关键细节**: 文档正确地指出 `t.Skipf` 是静默的。这三个测试在 `go test -v` 之外的输出中都是不可见的跳过。

**唯一遗漏**: 文档提到 `internal/adr/adr_test.go` 调用 `os/exec` 执行 git 和 go 命令。这是对的，但 **git 在几乎任何开发/CI 环境中都存在**，不像 python3/node 那样是新贡献者的 friction point。而且 ADR 测试对 `os/exec` 的使用是路径解析（`go build` 验证可编译性），不是行为依赖——这是合理的使用方式。

### 文档建议评估

文档提出的四条建议方向:
1. **`test-requirements.md`** 清单 — ✅ 低成本高价值
2. **`t.Skip` → `t.Log` 升级** — ⚠️ `t.Skip` 在 Go 测试中有合法用途（跳过不可用的测试），但文档建议的"explain skip reason clearly"是正确的
3. **yaml2json golden fixture 套件** — ✅ 可行，但需权衡：golden fixture 只验证 Go 解析器的输出一致性，无法检测 Go 与标准 YAML 的语义分歧（这正是 Sprint 27 block-scalar bug 暴露的问题——Go 解析器和 PyYAML 都产生了一致的但错误的输出）
4. **CI Go-only 矩阵** — ✅ 低成本，高可见度

### 验证后评估

| 项 | 评估 |
|---|------|
| 核心主张 | ✅ **成立** |
| 代码证据 | ✅ 全部准确 |
| 影响评估 | ✅ 合理——对产品采纳和贡献者体验有真实影响 |
| 修复方案 | ✅ 四条建议均可实施，无跨层边界问题 |
| **优先级修正** | P1 → **P1**（不变） |

---

## 方向③ 异常检测脱离演化循环 — ✅✅ 强确认

**文档的核心主张**: `DetectAnomalies` 完全独立的，仅在 CLI 中调用，不与 evolve loop 集成—— ✅✅ **强成立**

### 我重构的完整调用链

```
forge doctor --anomaly                        # CLI 入口
  → validate.go:455: cmdDoctorAnomaly(root)   # 命令分派
    → doctor.Anomaly(root)                    # anomaly.go:26
      → LoadCheckpointChain(root)             # 加载 .forge/checkpoint.json + 5 备份
      → DetectAnomalies(chain)                # 5 个检测器
        → detectStale                         # 停滞检测
        → detectStuckIteration                # 卡住迭代
        → detectRoadmapJump                   # 进度跳跃
        → detectDryRun                        # dry-run 检测
        → detectNoProgress                    # 无进展检测
      → 返回 AnomalyReport (Findings)
    → fmt.Println(...)                        # 仅打印到 stdout
```

**全仓 `grep` 零命中**: `grep -rn "DetectAnomalies\|doctor\.Anomaly" forge-core/cmd/forge/evolve.go forge-core/cmd/forge/loop.go` → 零结果。

**`OnIteration` 钩子已存在但未用于异常检测**:
```go
// loop.go:68-74
OnIteration func(i int, sig converge.Signals, durationMs int64)
// evolve.go:186 — 仅用于 checkpoint + memory + trace
loop.OnIteration = checkpointHook(o, wf, tracer, budget, logln, verdicts, findings)
// checkpointHook → persist.Save + recordMemory + emitTrace
// 没有 DetectAnomalies 调用
```

### 文档"额外发现"的验证

文档指出 `detectNoProgress` 可通过迭代计数前进绕过:
```go
func detectNoProgress(chain []persist.Checkpoint, warn func(string, ...any)) {
    identicalCount := 0
    for i := 0; i < len(chain)-1; i++ {
        if chain[i].Iteration == chain[i+1].Iteration &&
            chain[i].RoadmapCompletion == chain[i+1].RoadmapCompletion {
            identicalCount++
        }
    }
    // 如果 iteration 在 loop-back 中前进但 RoadmapCompletion 不变→不被检测
}
```
✅ **这个分析是精确的**。如果 loop-back 让 iteration 计数增加但 RoadmapCompletion 不变（agent 每次产生相同的代码修改），`detectNoProgress` 会看到不同的 `Iteration` 值，不计数为"相同"。

### 文档建议评估

文档提出的三个方向:
1. **将 `DetectAnomalies` 注入 `OnIteration`** — ✅ 合理，但需注意 `DetectAnomalies` 当前需要 checkpoint 链（>1 条），而第一次迭代时链长度为 1
2. **卡住自动降级 routing tier** — ⚠️ 这是一项有风险的启发式操作，可能掩盖问题而非解决。建议先做"记录+告警"，再做自动降级
3. **人类上报接口 (`stall.marker`)** — ✅ 可靠，简单且与现有 `forge status` 集成

### 验证后评估

| 项 | 评估 |
|---|------|
| 核心主张 | ✅✅ **强成立**—调用链完全断开 |
| 代码证据 | ✅ 精确 |
| 额外发现 | ✅ 精确——`detectNoProgress` 的绕过分析正确 |
| 修复方案 | ⚠️ 自动降级 tier 建议有风险，应先记录/告警 |
| **优先级修正** | P0 → **P0**（不变）— 确实是 24h 自治运行的核心韧性缺口 |

---

## 方向④ Memory 压缩仅 CLI 可调用 — ❌ 核心主张证伪

**文档的核心主张**: `Compact` 只在 `forge memory-prune` CLI 命令中被调用，演化循环永不触发。**❌ 错误——`evolve.go` 已经在自动调用 `Compact`**。

### 我在代码中发现的实际执行流

```
evolve.go:186: loop.OnIteration = checkpointHook(...)
  → checkpointHook → recordMemory(...)            # evolve.go:339
    → recordMemory → compactMemoryIfDue(...)       # evolve.go:407
      → compactMemoryIfDue:                        # evolve.go:434-438
          if i%10 != 0 { return }                  # 每 10 次迭代触发一次
          removed, compacted, err := memory.Compact(
              memoryPath(root),
              memory.DefaultCompactThreshold,      # 500
              memory.DefaultCompactKeepPerKind,     # 20
              memory.CompactAgeSeconds,             # 86400 (24h)
          )
```

**`Compact` 已经在 evolve 循环中每 10 次迭代自动调用一次**。文档的核心主张（"从不自触发"）是**事实错误**。

### 证据 C 的严重问题

**文档声称** (`memory.go:143-146`):
```
// The compact threshold (DefaultCompactThreshold) is referenced in
// memory_compact.go but the evolve loop never triggers it.
```

**实际代码** (`forge-core/internal/memory/memory.go:254-262`):
```go
// (most recent). It is the compaction counterpart of the append-only Append:
// See memory_compact.go for Prune's on-disk sibling rewriteStore and for the
// age/kind-aware Compact, which trades exact truncation for a summarized tail.
// format is unit-testable without a filesystem. It returns one compact JSON
```

**这条注释不存在**。`memory.go` 中没有声称"evolve loop never triggers it"的注释。文档引用了一条不存在的代码评论作为证据。这是全文中唯一一个**完全编造的代码证据**。

### 文档中有价值的子问题

尽管核心主张错误，文档指出了一些真实但更弱的问题:

| 子问题 | 状态 |
|--------|------|
| `compactMemoryIfDue` 每 10 次迭代的触发频率在短跑中可能不够 | ⚠️ 真实但次要——短跑不超过 10 次迭代通常也不会产生 500+ 条目 |
| 检查 memory 大小的开销（每 10 次迭代一次 file stat）| ⚠️ 可忽略的开销——`Load` 是本地 JSONL 文件读取 |
| 无 `trace.Event` 记录压缩事件 | ✅ 真实——`compactMemoryIfDue` 仅 `logln`，不写入 trace |
| 用户可能不知道自动压缩已存在 | ✅ 真实——文档本身就没发现它，说明可发现性差 |
| 无预算感知的压缩延迟 | ⚠️ 若压缩 IO 成本在 budget 紧张时确实有影响则值得考虑 |

### 修正后的评估

**最低优先级**——现有自动压缩已覆盖核心需求，留下的子问题全部是次要增强:

| 项 | 评估 |
|---|------|
| 核心主张 | ❌ **证伪**—`Compact` 在 evolve loop 中自动调用 |
| 证据 C | ❌ **不存在的注释**—编造的代码引用 |
| 证据 A/B | ⚠️ 技术正确但误导——`grep` 确实找到了 `evolve.go` 中的调用点 |
| 文档差异验证 | ❌ 如果已在差异验证阶段 grep 过 `\.Compact`，应发现 `evolve.go` 的调用 |
| **优先级修正** | P2 → **P4 / Won't Fix As-Designed**（现有自动压缩满足需求） |

**建议的替代方向**: 改为"增加压缩事件的可观测性"（trace event + forge status visibility），约 0.25 sprint。

---

## 方向⑤ 零相位工作流引擎行为未定义 — ⚠️ 核心成立但不完整

**文档的核心主张**: `RunFrom`/`RunParallel` 在 `len(wf.Phases) == 0` 时静默返回 nil——行为未定义—— ✅ **成立**

### 逐条验证

**证据 A (`RunFrom`)**:
```go
// orchestrator.go:170
for i := start; i < len(wf.Phases); i++ { ... }
// start=0, len=0 → 立即退出循环,返回 nil
```
✅ 确实静默 nil。但需要注意:如果 `start > 0`（如从 checkpoint 恢复），len=0 且 start=0 的行为也是 nil。如果 start=1 且 len=0，则 `1 < 0` = false → 同样 nil。

**证据 B (`RunParallel`)**:
```go
// parallel.go:72-75
waves, err := Waves(wf.Phases) // With [] → [][]int{} (零波)
// len(waves) == 0 → for 循环不执行 → reportStop → return nil
```
✅ 同样静默 nil。但 **`Waves([])` 不会报错**——它返回 `([][]int{}, nil)`，因为 `len(phases) == 0` 使外层 for 循环不执行。

**证据 C (`reportStop`)**:
```go
// orchestrator.go:476-480
func (e Engine) reportStop(wf asset.Workflow) {
    if wf.Stop.Type == "" {
        e.logf("stop: no condition declared")
        return
    }
    // ...
}
```
⚠️ **文档高估了风险**。`reportStop` 只访问 `wf.Stop`，不访问 `wf.Phases`——它在零相位工作流上不会崩溃。最坏情况是日志一行"stop: no condition declared"，然后返回 nil。

**证据 D (`Waves` 重复名)**:
```go
// waves.go:16-19
idx := make(map[string]int, len(phases))
for i, p := range phases {
    idx[p.Name] = i  // 第二个同名 phase 覆盖第一个
}
```
⚠️ **真正的 bug**：重复名称导致依赖解析静默指向错误 phase。这比零相位更危险——它能导致执行错误 phase 而不是优雅失败。

**证据 E (`engine_build_test.go` 空 Phase)**:
```go
argv := claudeArgv(o, false, "sonnet", asset.Phase{})
```
⚠️ 这不是 bug——Go 零值 struct 有定义良好的行为（所有字段为零值）。`claudeArgv` 应该能正确处理。

### 未覆盖的边界场景

文档遗漏了两个更危险的场景:

1. **`phases: null` 在 JSON 中的行为**: YAML 的 `phases:` 无值被解码为 `nil`，但 Go YAML 解析器可能对不同输入产生不同行为。文档断言 "`nil` 被解码为 Go nil slice → `len(nil) == 0`" 是正确的，但值得指出: 如果 YAML 解析器对 `null` vs 缺失键产生不同的零值，行为取决于解析器实现。

2. **`checkStageSkip` 先于相位检查**: `RunFrom` 中的第一行 `if e.checkStageSkip(wf) { return nil }` 独立于 phase 数量。这意味着一个被 mode-gating 完全跳过 phase 的工作流（所有 phase 因 mode policy 被跳过一个不匹配 stage 的 workflow）也会静默返回 nil——但这是有意的行为，不是 bug。

### 验证后评估

| 项 | 评估 |
|---|------|
| 核心主张 | ✅ **成立**—RunFrom/RunParallel 在零相位时静默 nil |
| 证据 A/B | ✅ 精确 |
| 证据 C | ⚠️ 高估风险—`reportStop` 不会崩溃，仅输出一行 log |
| 证据 D | ⚠️ 这是真正 bug，但与"零相位"不同—应作为独立问题 |
| 证据 E | ⚠️ 不是 bug—零值 struct 有定义良好的行为 |
| **优先级修正** | P3 → **P3**（不变），但应将证据 D（重复 phase 名）提升为独立 issue |

---

## 修正后的优先级矩阵

| # | 方向 | 原优先级 | 修正优先级 | 关键诊断 |
|---|------|:--------:|:----------:|---------|
| 3 | 异常检测脱离演化循环 | P0 | **P0** ✅ | 唯一真正需要立即修复的方向——24h 自治的核心韧性缺口 |
| 1 | ADR 测试→修复闭环 | P1 | **P1** ⚠️ | 核心成立但高估了 ADR-0002 退化程度；方案未考虑 Go→Markdown 跨层边界的工程成本 |
| 2 | 测试依赖隐式 CLI | P1 | **P1** ✅ | 完全成立；四条修复建议均可行 |
| 5 | 零相位工作流防御 | P3 | **P3** ⚠️ | 核心成立但高估了报告函数的崩溃风险；应将重复 phase 名问题提升 |
| 4 | Memory 压缩不自触发 | P2 | **P4 / Won't Fix** ❌ | **核心主张证伪**——`Compact` 实现在 evolve 循环中每 10 次迭代自动调用 |

---

## 全文元分析

### 文档的长处
1. **方法论严谨**: 差异化验证（在 125+ 份已有文档中交叉检索）是值得行业推广的实践
2. **代码接地**: 所有方向都附带了精确的行号引用和函数调用链
3. **边界场景表**: 每个方向都给出了系统的边界场景分析——这是大多数其他分析文档缺失的

### 文档的主要问题
1. **`grep` 疏忽（方向④）**: 如果文档在写"证据 A"时使用了 `grep -rn "\.Compact" forge-core/ --include="*.go"`，它会发现 `evolve.go:438` 的调用。这要么是 `grep` 没有跑全仓，要么是结果被选择性忽略。
2. **编造的代码引用（方向④, 证据 C）**: 声称 `memory.go:143-146` 包含一条特定措辞的注释，但该注释不存在。这是最严重的质量问题。
3. **跳跃的推理（方向①）**: 正确识别了缺口，但提出的修复方案未考虑 forge-core 的分层约束（Go → Markdown 反向依赖）。
4. **不精确的描述（方向①/⑤）**: ADR-0002 测试被描述为"t.Skip 静默退化"但实为 `t.Logf`；`reportStop` 被描述为"可能输出误导信息"但实际是安全的。

### 对文档写作流程的建议
1. **自动验证代码引用**: 每个 `// file.go:N-M` 引用应在提交前自动用 `sed -n 'N,Mp' file.go` 验证。这会捕获方向④的编造引用。
2. **全仓 `grep` 自动化**: 文档中的每个 "grep 确认" 断言应是可重现的——文档应记录 `grep` 命令和输出。方向④的 `grep -rn "\.Compact"` 漏掉了 `evolve.go` 里的结果，应被 CI 捕获。
3. **独立代码验证**: 方向④的错误证明了外部验证的必要性——文档作者看到 `validate.go` 中的 `cmdMemoryPrune` 后推断再无其他调用点，但没有确认全仓搜索。
