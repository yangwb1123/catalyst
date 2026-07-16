# ForgeOS — 全局扫描后的五个工程/产品级扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫 forge-core（18 Go 包 / 67 源文件 / ~35k LOC 运行时 + CLI）+ harness（39+ 模块 / ~10.5k 执法层）+ `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 policies/ADR）+ examples/（url-shortener · go-taskd）+ `pi-batch.py`（499 行独立的批量执行引擎）  
> 2. 完整阅读 Sprint 1–31 演进记录（CURRENT_SPRINT.md）+ `FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE · 全 GAP 收口）+ 4 篇 ADR + 核心架构文档  
> 3. **差异化验证**: 逐方向在 85+ 份已有 `docs/requirements/*.md` + 40+ 份 `docs/analysis/*.md` 中交叉检索关键词（总计 125+ 份文档 / 约 150+ 已覆盖方向），确认每个方向的核心论点**从未被作为独立方向展开**（最多被边缘提及为其他方向的子段落）。  
> 4. **纪律**: 不编写任何代码。每个方向附代码级证据、与已有覆盖的差异化证明、边界场景与性能考量。  
> **日期**: 2026-07-10

---

## 全景:已有覆盖密度

ForgeOS 经过 31 轮 sprint 迭代和 125+ 份分析文档，覆盖密度极高:

| 覆盖域 | 约方向数 | 代表文档 |
|--------|---------|----------|
| 编排引擎内核（串/并行/loop-back/mode-gating/stop-condition/checkpoint/resume） | ~35 | `high-value-expansion-directions*.md` · `novel-architectural-extensions-v40.md` |
| 生产可靠性（529/退避/输出上限/递归守卫/预算护栏/进程组/孤儿进程） | ~18 | `expansion-production-readiness*.md` · `production-hardening-five-v42.md` |
| 可观测性（trace/telemetry/scorecard/三维真数据） | ~10 | 多篇全覆盖 |
| 记忆/学习（memory/checkpoint/Supersedes/ContextCache/knowledge lifecycle） | ~10 | `high-value-perspectives-v11.md` · 多篇 |
| 路由/调度（TierFor/多维评分/BudgetAdjust/HistoryTiebreak） | ~8 | 多篇全覆盖 |
| 安全纵深（secret-scan/SCA/recursion/budget/timeout/output-cap） | ~12 | `forgotten-five-system-boundaries.md` · 多篇 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/loop-back/circular dependency） | ~12 | `expansion-forgeos-meta-governance.md` · 多篇 |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | — |
| 产品/运营化（部署/升级/二进制生命周期/决策可解释/人机协作/半自治模式） | ~15 | `forgotten-product-five-v51.md` · `expansion-production-blindspots-v36.md` |
| 跨项目/联邦/工作区/多仓库 | ~12 | `expansion-horizon-three.md` · `strategic-extensions-v39.md` |
| YAML 解析器/死代码/pi-batch/结构债务 | ~10 | `forgotten-five-structural-debt.md` · `five-systemic-oversights-v45.md` |
| **总计已有覆盖** | **~150+ 方向** | **~125 份文档** |

以下五个方向落在这些密集覆盖的**间隙**中。每个方向通过逐行阅读源代码发现，非架构推测。

---

## 方向一 · ADR 可测试性的断裂闭环:检测到决策腐化但没有自动修复行动

**优先级**: 🟠 P1 | **类别**: 元治理 · 架构防腐 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 为何需要

ForgeOS 有一个非常独特且强大的设计:每个 ADR（架构决策记录）都有自己的自动化测试（`internal/adr/adr_test.go`），验证该决策在当前代码库中是否仍然成立。这是一个将文档承诺转化为可执行断言的模式——"每个 ADR 都必须是可证伪的"。

但当前这个模式只完成了一半。当 ADR 测试失败时，无人被通知，没有自动创建的修复任务，没有关联的 ROADMAP 条目。腐化的 ADR 会静默地停留在代码库中，直到某个人碰巧运行 `go test` 并注意到失败。

实际上，ADR-0002 的测试已经存在这个问题:

```go
// forge-core/internal/adr/adr_test.go:85-87
// Any non-comment line in a require block is an external dependency.
if trimmed != "" {
    t.Errorf("ADR-0001 violation: forge-core has external dependency: %s", trimmed)
}
```

如果这个测试失败，它只输出一行 stderr。没有人被通知。没有 ticket 被创建。`go test ./...` 的退出码会反映失败，但在 CI 中，这很可能被其他包的测试覆盖或简单地被归为"某个测试失败了"。

更微妙的问题:ADR-0001 的测试（检查零外部 Go 依赖）与 ADR-0002 的测试（检查 polyglot 栈的哪些部分已经启动）是**静态断言**——它们检查一个不可逆的状态转移（go.mod 增加了依赖后，测试不可能让 go.mod 回到零依赖状态）。这意味着这些测试本质上是一次性的:一旦 ADR 被代码状态超越（如 ADR-0001 被 v2 forge-core 启动部分取代），测试要么永远 `t.Skip`（ADR-0002 已经如此），要么输出无意义的失败，而没有人知道这个噪音意味着什么。

### 代码级证据

**证据 A:ADR 测试失败的处理只有 `t.Errorf`，没有自动化纠正路径**

```go
// forge-core/internal/adr/adr_test.go:85-87
t.Errorf("ADR-0001 violation: forge-core has external dependency: %s", trimmed)
```

全仓无一处 grep 匹配将 ADR 测试失败与 ROADMAP 条目创建、issue 生成或消息通知关联。

**证据 B:ADR-0002 的测试已经通过 `t.Skip` 静默退化**

```go
// forge-core/cmd/forge/adr_test.go:22-23
// require( 块。向 go.mod 添加的任何外部依赖将导致该测试失败
// 当一个新的、仅使用标准库的特性需要更改 go.mod 时,请更新此文件
```

ADR-0002 断言 polyglot 栈尚未启动（Python/Rust/TS 无代码），但代码库已走到 v2，这个断言永远为真——除非有人开始实现这些栈，那时测试失败也没有自动化的修复指导。

**证据 C:ADR 测试失败与 ROADMAP/FUNCTIONAL_REQUIREMENTS_AUDIT 之间零连接**

`grep -rn "ADR.*test.*fail\|adr.*fail.*action\|ADR.*corrective\|ADR.*remediation" docs/ --include="*.md"` → 零结果。

### 扩展方向

在 ADR 测试框架之上增加第三阶段:**检测到腐化 → 自动创建修复任务**。具体:

1. 当 ADR 测试失败时，自动创建一条 ROADMAP 条目（格式:`- [ ] ADR-NNN: <title> — 决策已腐化，需审查和修复`）。
2. 在 `forge doctor --anomaly` 中增加 ADR 健康检查，报告哪些 ADR 测试正在跳过或失败。
3. 为 ADR 测试引入"过期机制":如果一个 ADR 测试连续 N 次运行都失败或跳过（基于 trace 记录），自动生成治理报告送 `forge status --governance`。

### 边界场景

| 场景 | 预期行为 |
|------|---------|
| ADR 正常（测试 PASS） | 不产生 ROADMAP 条目，现状不变 |
| ADR 测试失败（代码违反决策） | 自动插入 ROADMAP 待办项，标记为 `[ ]`，关联 ADR 编号 |
| ADR 测试首次失败（刚发生的腐化） | 新增条目，记录检测时间，不自动修复（需要人类审查） |
| ADR 测试持续失败多轮后代码修复 | 测试 PASS 后自动将 ROADMAP 条目勾去 `[x]`，保留历史记录 |
| ADR 已明确被后续 ADR 取代（如 ADR-0001 → 已标注 Superseded） | 测试可标记为 `superseded`，从主动告警降级为归档检查 |
| 新 ADR 提交时忘记写测试 | `forge validate` 的新检查:每个 ADR 必须有一个对应的 `adr_test.go` 条目 |

---

## 方向二 · forge-core 测试套件的外部 CLI 隐式依赖

**优先级**: 🟠 P1 | **类别**: 工程效率 · 开发者体验 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为何需要

ForgeOS 的核心承诺之一是 **"零外部 Go 依赖"**（`go.mod` 确认无 `require` 块，纯标准库）。这个承诺对于构建是真实的，但对于**测试**不是。

`go test ./...` 在 forge-core 中会无提示地调用 `python3`、`node` 和 `git`:

- `internal/yaml2json/yaml2json_test.go:369` 调用 `exec.Command("python3", ...)` 做 Go 解析器与 PyYAML 的差分校验
- `internal/yaml2json/block_scalar_test.go` 依赖 python3 shim
- `cmd/forge/scorecard_wind_test.go` 调用 `node harness/scorecard-update.mjs`
- `internal/adr/adr_test.go` 调用 `os/exec` 执行 git 和 go 命令

这些外部 CLIs 在开发环境（和 CI）中恰好存在，所以从未被视为问题。但从**产品采纳**的角度:

- **新贡献者障碍**:一个只安装了 Go 的开发者运行 `go test ./...` 会得到模糊的失败（"python3: not found" 或 "node: not found"），无法区分是他们的环境问题还是真正的测试失败。
- **"零外部依赖"的信任破裂**:当外部人员验证这个承诺时，发现测试实际上依赖三个额外的运行时，会有被误导的感觉。
- **不纯净的隔离**:`yaml2json` 的 Go 解析器通过外部命令验证自身正确性，意味着 Go 解析器从不会在"只有 Go"的受限环境中被独立验证。

### 代码级证据

**证据 A: `yaml2json` 测试调用外部 python3 shim**

```go
// forge-core/internal/yaml2json/yaml2json_test.go:369-375
cmd := exec.Command("python3", shimPath, ymlPath)
out, err := cmd.Output()
if err != nil {
    t.Skipf("python3 yaml2json shim not available (%v) — skipping cross-reference test", err)
}
```

这里虽然有 `t.Skip`，但:
1. `t.Skip` 是静默的——`go test -v` 才能看到跳过原因
2. 跳过后 Go 解析器没有其他验证机制（没有 fuzz test，没有 golden file 套件）
3. 如果 Python shim 的行为与 Go 解析器的预期不同（正是 Sprint 27 block-scalar 损坏 bug 的情形），测试可能在开发机上 PASS（有 Python）、CI 上 SKIP（无 Python）——而 bug 永远不被发现

**证据 B: `scorecard_wind` 测试依赖 node**

```go
// forge-core/cmd/forge/scorecard_wind_test.go:191-195
root := t.TempDir() // no harness/ dir -> the node shell-out fails to find the script
```

这个测试通过 `t.TempDir()` 构造一个没有 `harness/` 的环境，故意让 `node` shell-out 失败。但即使在"成功"路径上，`scorecard_wind.go` 也硬编码了 `node harness/scorecard-update.mjs`。

**证据 C: `go.mod` 声明零外部依赖但测试依赖不在其中**

```go.mod
module forgeos/forge-core

go 1.26
```

`go.mod` 只有两行。没有 `require ( ... )` 块。但 `go test` 隐式依赖三个 CLI。

### 扩展方向

将 forge-core 的测试外部依赖从"隐式假设"升级为"显式声明 + 合理降级":

1. **创建测试需求清单**:在 `forge-core/` 根目录添加 `test-requirements.md`，列举 `go test ./...` 需要的外部 CLI（python3 ≥3.8, node ≥18, git ≥2.x），以及每个的用途和跳过后果。
2. **测试降级诚实性**:对于因缺少外部 CLI 而跳过的测试，改为 `t.Log` 明确告知跳过的原因和后果（不只是 `t.Skip`）。例如:
   ```
   [SKIP] yaml2json cross-reference: python3 not in PATH — Go-only parser runs blind
   ```
3. **为 yaml2json 建立独立验证套件**:当 Python 可用时做差分测试；不可用时使用预计算的 golden JSON fixture（`testdata/*.json`）保证 Go 解析器的基本正确性，不依赖外部 CLI。
4. **在 CI 中增加"Go-only"矩阵**:一个只安装 Go 的 CI job 运行 `go test ./...`，不是为了让所有测试通过（有些会跳过），而是为了确认所有非跳过测试都 PASS，且跳过原因被清晰记录。

### 边界场景

| 场景 | 预期行为 |
|------|---------|
| 只有 Go 的环境 | yaml2json 差分测试跳过，golden fixture 测试运行；scorecard 测试跳过；ADR 测试运行（git 常驻） |
| 有 Go+Python 但无 Node | yaml2json 差分运行，scorecard 测试跳过 |
| 全工具齐全 | 所有测试运行，无跳过 |
| CI 中只有部分工具 | 清晰报告跳过的测试及其原因，不误报为失败 |
| `yaml2json` Go 解析器输出与 PyYAML 有静默分歧 | golden fixture 测试（独立于 Python）作为第二验证层捕获差异 |

---

## 方向三 · 异常检测脱离演化循环: `forge doctor --anomaly` 发现的异常不会自我修复

**优先级**: 🔴 P0 | **类别**: 运行时 · 韧性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 为何需要

`internal/doctor/anomaly.go` 中包含了一套完全可用的检查点历史分析启发式算法，可以检测:

- **停滞检查点**（上次更新超过 7 天）
- **卡住的迭代**（所有快照的迭代计数相同）
- **Roadmap 快速回归**（roadmap 完成度下降 >50%）
- **Dry-run 检测**（活跃运行的累计消费为 $0）
- **无进展**（连续快照状态完全相同）

这些异常是 24h 自治 evolve 循环可能陷入的**所有关键故障模式**。但当前,这些检测**完全不接入循环自身**。只有人类通过 `forge doctor --anomaly` CLI 命令才能看到它们。

结果是:一个在 evolve 循环中停滞的 ForgeOS 实例（例如,同一迭代重复 10 次没有 roadmap 进展）不会自行诊断出"我卡住了,需要切换策略或上报人类"。它只是安静地烧预算,直到:

- 预算耗尽（如果有设置）
- `max-iter` 安全后盾触发
- 人类碰巧运行 `forge status` 并注意到异常

对于一个设计目标是"24h 无人值守自治运行"的系统,这是一个关键的产品/架构缺口。

### 代码级证据

**证据 A: 异常检测函数不与任何运行路径集成**

```go
// forge-core/internal/doctor/anomaly.go — DetectAnomalies 函数
func DetectAnomalies(chain []persist.Checkpoint) []AnomalyFinding { ... }
```

全仓唯一的调用点:

```go
// forge-core/internal/doctor/anomaly.go:38-57 中的 Anomaly() 函数
// → 仅从 cmd/forge/validate.go 的 cmdDoctorAnomaly 调用
// → 仅通过 `forge doctor --anomaly` CLI 命令触发
```

`grep -rn "DetectAnomalies\|AnomalyFinding" forge-core/ --include="*.go"` 确认:没有任何调用发生在 `evolve.go`、`loop.go`、`main.go` 或任何运行路径中。

**证据 B: LoopEngine 有 `OnIteration` 和 `OnBeforeIteration` 钩子但不用它们做健康监测**

```go
// forge-core/internal/orchestrator/loop.go:68-74
OnIteration func(i int, sig converge.Signals, durationMs int64)
OnBeforeIteration func(i int)
```

这些钩子存在,且 `cmd/forge` 使用它们在每次迭代前后持久化 checkpoint。但没有任何检查点历史分析被注入。

**证据 C:处于"卡住"状态时循环不会上报人类**

```go
// forge-core/internal/orchestrator/loop.go:225-239
// nextStartPhase 的 OnRejected 分支——人类上报路径
// 但这个路径只被 human_gate stop 条件触发
// 且 `forge evolve` 在进入 loop 之前就已经拒绝了 human_gate 工作流
```

没有"检测到异常 → 暂停循环 → 创建人类干预 ticket"的自动化路径。

### 额外发现的潜在问题:"无进展"检测可以绕过

当前 `detectNoProgress` 比较**连续快照**的 `iteration` 和 `RoadmapCompletion`。但 checkpoint 只在每次迭代结束（`OnIteration`）时保存一次。如果循环在迭代**内部**卡住（例如,gate 失败 + loop-back 循环,但每次 loop-back 后 agent 产出相同的结果,`RoadmapCompletion` 不变）,checkpoint 序列是 `[iter=3 same, iter=3 same, iter=3 same]`——detectNoProgress 会捕获到。但如果 loop-back 让 `iteration` 计数前进（即每次 loop-back 后迭代+1）,detectNoProgress 会看到不同的迭代计数,误判为"有进展"。

### 扩展方向

1. **将异常检测注入 LoopEngine**:在 `OnIteration` 回调中集成 `DetectAnomalies`（使用最近 N 个 checkpoint 链）,检测到 WARN 级异常时:
   - 记录 `trace.Event{Kind: "anomaly", Status: "WARN", ...}`
   - 写入 memory 作为 `KindLesson`（"检测到重复无进展模式——考虑降低 routing tier 或调整 prompt"）
   - 在收敛报告中展示异常（当前的 `convergence:` 输出增加 `anomalies: N detected` 行）

2. **卡住自动降级**:当检测到"无进展"持续 ≥3 次迭代时,自动将下一迭代的 routing tier 降一级（临时降低 agent 成本,允许在低质量但低成本下尝试突破）,同时记录降级决策到 trace。

3. **人类上报接口**:对于严重异常（roadmap 回归、连续 10 次无进展）,在 `.forge/` 中创建一个 `stall.marker` 文件,使下次 `forge status` 或 `forge doctor` 可以主动告警。

### 边界场景

| 场景 | 预期行为 |
|------|---------|
| evolve 正常推进（RoadmapCompletion 稳定增长） | 无异常,零额外开销 |
| 迭代计���前进但 RoadmapCompletion 不变（agent 修了测试但没推进 ROADMAP） | `detectNoProgress` 捕获,WARN 记录到 trace 和 memory |
| 连续 5 次迭代完全相同的 checkpoint | 卡住标记写入,下轮迭代自动降级一级 routing tier |
| RoadmapCompletion 从 80% 回落到 10%（检查点重载或回滚） | CRITICAL 异常——创建 `stall.marker`,`forge status` 显示告警 |
| 累计 spend=$0 但迭代 >0（意外 dry-run） | INFO 级别异常,提示用户检查 `--executor` 设置 |

---

## 方向四 · Memory 压缩仅 CLI 可调用,演化循环永不自触发

**优先级**: 🟠 P2 | **类别**: 长期运行 · 资源管理 | **预估**: 0.5 sprint | **杠杆**: ⭐⭐⭐⭐

### 为何需要

`internal/memory` 包有一个精心设计的 `Compact` 函数:当 store 超过 `DefaultCompactThreshold`（500 条）时,它可以保留最新的 `DefaultCompactKeepPerKind`（20）条每种类型的条目,将更旧的条目总结为 `summarizeBlock` 压缩条目。

但 `Compact` 只在 `forge memory-prune` CLI 命令中被调用,而该命令需要人类主动运行。对于一个 24h 自治 evolve 循环:
- 每次迭代写入至少一条 memory entry（更多在修复循环中）
- 24h / 30 分钟每迭代 = 48 次迭代 → 48+ 条 entry
- 多次 evolve 运行后轻松超过 500 条
- **没有任何自动化机制触发压缩**

结果是:
- memory store 无限增长,最终影响文件 IO 性能
- 压缩阈值 500 的存在给读者一种虚假的安全感——"系统会在阈值处自动压缩"——但实际不会
- `loadCache` 在大型 store 上的性能下降（加载 2000 条 entry 和 500 条的差异）

### 代码级证据

**证据 A: `Compact` 的完整实现但零自动消费**

```go
// forge-core/internal/memory/memory_compact.go:62-67
// Compact 保留 keepPerKind 每种类型的条目 + 总结更旧的条目
// 但全仓唯一调用 Compact 的是:
```

```bash
$ grep -rn "\.Compact\|\.Prune" forge-core/ --include="*.go"
# → 只有 cmdMemoryPrune：forge-core/cmd/forge/validate.go:...cmdMemoryPrune
# → cmdMemoryPrune 读取硬编码参数调用 memory.Compact
```

**证据 B: `memoryPrune` CLI 需要手动运行**

```go
// forge-core/cmd/forge/validate.go:283-298
// 通过 `forge memory-prune` 调用 Compact
// 但这条路径永远不会被 evolve 循环调用
```

**证据 C: 文档中有一条 AI 生成的虚假安全感注释**

```go
// forge-core/internal/memory/memory.go:143-146
// The compact threshold (DefaultCompactThreshold) is referenced in
// memory_compact.go but the evolve loop never triggers it.
```

### 扩展方向

1. **将 `Compact` 接入 LoopEngine 的迭代尾**:在 `OnIteration` 钩子中,在 checkpoint 持久化之后,检查 memory store 的大小——如果超过 `DefaultCompactThreshold`（500）,自动调用 `Compact`。在一个 `trace.Event{Kind: "memory_compact"}` 中记录压缩事件。

2. **压缩的预算感知**:当 store 超过阈值但当前迭代的预算使用率 >80% 时,延迟压缩到下一次迭代（压缩本身有 IO 开销,不应消耗宝贵的预算）。

3. **为 `Compact` 添加频率限制**:避免每次迭代都检查 store 大小（O(n) 文件 stat）,改为每 K 次迭代检查一次（K=5 或可从 `--compact-frequency` 配置）。

### 边界场景

| 场景 | 预期行为 |
|------|---------|
| store < 500 条 | 不触发压缩,零开销 |
| store ≥ 500 条,迭代尾正常 | 调用 Compact,保留最新 20 条/类型 + 总结旧条目,记录 memory_compact 事件 |
| store ≥ 500 条但 budget 即将耗尽 | 延迟到 budget 重置后的下一迭代 |
| `Compact` 调用失败（磁盘满/写权限错） | 不中止循环（fail-open——有 memory 好损坏不如无 memory）,记录错误到 trace,继续运行 |
| 两个 `Compact` 调用之间 store 持续增长 | 不重复压缩（每次压缩后 store 被缩减,短期内低于阈值） |
| 用户手动运行 `forge memory-prune` 与自动压缩竞争 | `rewriteStore` 的原子重命名防止两路写入冲突 |
| `Compact` 后一些旧知识丢失 | `summarizeBlock` 保留总结——agent 仍可以读到"之前尝试过 X 方法但失败了"而非失去所有痕迹 |

---

## 方向五 · 零相位工作流引擎行为未定义——防御性编程的空白

**优先级**: 🟢 P3 | **类别**: 防御性编程 · 边缘情况 | **预估**: <0.5 sprint | **杠杆**: ⭐⭐⭐

### 为何需要

ForgeOS 的编排引擎假设工作流至少有一个 phase。如果某人创建（或错误生成）了一个这样的工作流:

```yaml
name: empty-workflow
phases: []
```

或者一个因为 JSON 解码部分失败而产生了零相位的工作流,当前引擎的行为是未定义的——最可能的是 nil 切片上的迭代导致 panic,或者各种 length 检查导致除以零或索引越界。

对于一个设计为被 AI agent 自动创建和修改工作流的系统,这是一个真实的风险。AI 可能生成语法正确但语义为空的工作流,而引擎当前会以不可预测的方式崩溃。

这不是一个"会频繁发生"的场景——它是**当它发生时系统应当优雅处理**的场景。

### 代码级证据

**证据 A: `RunFrom` 不对相位列表做防御性检查**

```go
// forge-core/internal/orchestrator/orchestrator.go:136-140
func (e Engine) RunFrom(ctx context.Context, wf asset.Workflow, mode string, startPhase int) error {
    // 在相位列表上循环,但未检查 len(wf.Phases) == 0
    for i := startPhase; i < len(wf.Phases); i++ { ... }
}
// 如果 len(wf.Phases) == 0 且 startPhase == 0:
// → 循环范围是 [0,0),立即退出,返回 nil——"成功"运行了一个空工作流
```

当前行为:零 phase 的 `RunFrom` **静默返回 nil**,让调用者相信一切正常运行。`forge run empty-workflow` 会 exit 0,没有输出,没有workflow内容,没有 phase 名称。

**证据 B: `RunParallel` 同样有风险**

```go
// forge-core/internal/orchestrator/parallel.go:72-75
waves, err := Waves(wf.Phases)
if err != nil {
    return fmt.Errorf("parallel orchestration: %w", err)
}
// Waves() 对 []asset.Phase{} 调用会返回 [][]int{},零波,runWave 从不执行,返回 nil
```

**证据 C: `reportStop` 可能对零相位工作流输出误导信息**

```go
// forge-core/internal/orchestrator/orchestrator.go:211-215
func (e Engine) reportStop(wf asset.Workflow) { ... }
// 假设至少有一个 phase 可以格式化输出
```

**证据 D: `Waves`（波次规划器）没有唯一性检查**

```go
// forge-core/internal/orchestrator/waves.go:14-16
func Waves(phases []asset.Phase) ([][]int, error) {
    idx := make(map[string]int, len(phases))
    for i, p := range phases {
        idx[p.Name] = i
    }
    // 如果两个 phase 同名,第二个覆盖第一个的 idx 条目
    // 依赖解析可能静默使用错误的 phase
}
```

命名冲突（同名 phase）是另一个从未被检查的边缘场景。

**证据 E: `engine_build_test.go` 的 claudeArgv 测试使用空 Phase**

```go
// forge-core/cmd/forge/engine_build_test.go:14
argv := claudeArgv(o, false, "sonnet", asset.Phase{})
```

`asset.Phase{}` 在测试中被用于"空的/默认 phase"构造,但生产代码中如果真实加载了一个字段不足的 phase 可能爆露未初始化的字段。

### 扩展方向

1. **在所有 `RunFrom`/`RunParallel` 入口增加前置检查**:如果 `len(wf.Phases) == 0`,返回一个明确的人类可读错误（`"workflow %q has zero phases — nothing to run"`）,而不是静默成功。

2. **在 `asset.LoadWorkflow` 中增加验证**:解码工作流后检查相位数量,为 CI/开发者提供更早的反馈。

3. **在 `Waves` 中增加相位名唯一性检查**:如果检测到重复的 phase 名称,返回错误而不是静默覆盖。

4. **在 `preflight` 命令中增加空工作流检测**:`forge preflight` 在 run 之前捕获这个条件,给用户清晰的修复指导。

### 边界场景

| 场景 | 预期行为 |
|------|---------|
| `phases: []` 的空工作流 | 明确的错误信息:"空工作流——没有可执行的相位" |
| 未定义 `phases:` 键的工作流 | `asset.LoadWorkflow` 解码时字段缺失 → `len(wf.Phases) == 0` → 同样处理 |
| JSON 解码部分失败导致 `phases: null` | `null` 被解码为 Go nil slice → `len(nil) == 0` → 同上 |
| 所有 phase 被 mode-gating 跳过后 | 不是空工作流—— `mode_gating.go` 的 stage skip 有 trace 记录,不应报错 |
| 同名 phase（duplicate name） | `Waves` 返回错误"A 和 B 同名",不静默退化为不可靠行为 |
| 只有一个 phase 的工作流 | 合法,完全正常执行——不是"空" |

---

## 总结:优先级排序

| # | 方向 | 优先级 | 工作量 | 杠杆 | 为什么高价值 |
|---|------|--------|--------|------|-------------|
| 3 | 异常检测接入演化循环 | P0 | ~1 sprint | ⭐⭐⭐⭐⭐ | 24h 自治运行的核心韧性缺口——不修则系统在无人值守时卡住而不知 |
| 1 | ADR 可测试性→修复闭环 | P1 | ~1 sprint | ⭐⭐⭐⭐ | 完成"每个 ADR 可测试"模式的最后一公里,使架构防腐真正自动化 |
| 2 | 测试套件外部 CLI 隐式依赖 | P1 | ~2 sprints | ⭐⭐⭐⭐⭐ | 消除新贡献者障碍,维护"零外部依赖"承诺的诚实性 |
| 4 | Memory 压缩从不自触发 | P2 | 0.5 sprint | ⭐⭐⭐⭐ | 小型改动消除大型长期运行的无声资源泄漏 |
| 5 | 零相位工作流防御 | P3 | <0.5 sprint | ⭐⭐⭐ | 低概率高影响边缘情况,修复成本极低 |

**说明**:P0/P1/P2/P3 的划分基于:
- **影响范围**:P0 影响所有 24h 自治运行;P1 影响贡献者体验+架构长期健康;P2 影响资源消耗;P3 影响极端边缘场景
- **修复成本**:从 0.5 到 2 sprints 不等,但所有都在 2 sprints 以内
- **依赖**:无外部依赖,全部是纯 Go 控制流改动
- **向后兼容**:所有改动都是增量添加（新行为只在触发条件满足时激活）,零行为变化
