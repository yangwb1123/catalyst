# ForgeOS: 五个系统性盲区——全局扫描后的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫 forge-core（18 Go 包 / 63 非测试源文件 / ~33k 生产代码）+ harness（39+ 模块 / ~10.5k 行）+ `.agent/` 完整治理骨架 + `pi-batch.py` + `examples/` + 全部 sprint 演进记录（1-31）  
> 2. 逐篇通读 **49 份 `docs/requirements/*.md` + 40 份 `docs/analysis/*.md` + FUNCTIONAL_REQUIREMENTS_AUDIT + 核心架构文档**——合计约 150+ 已覆盖方向方向  
> 3. 交叉验证每个方向的关键词指纹，确保本文 5 个方向在所有已有分析中 **零命中或仅边缘语境提及**（非独立方向）  
> 4. **纪律**: 不编写任何代码。每个方向附代码级证据、边界情况矩阵、与已有分析的差异化证明。  
> **日期**: 2026-07-10  
> **版本**: v45（基于前 44 版需求分析的全局去重扫描）

---

## 已有分析覆盖全景与本文定位

ForgeOS 的文档体系在过去数周产生了 **80+ 篇分析文档**（49 篇 requirements + 40 篇 analysis + 若干核心文档），覆盖 **约 150+ 个独立方向**。这些方向集中在以下维度：

| 覆盖维度 | 约方向数 | 代表文档 | 本文不重复 |
|----------|---------|----------|-----------|
| 功能引擎补齐（编排/路由/记忆/收敛/信号/并行/波/自适应/Reflect） | ~35 | `high-value-expansion-directions*.md` · `novel-architectural-extensions-v40.md` | ✅ |
| 执行语义形式化（原子性/幂等/回滚/版本演化/因果一致性） | ~15 | `execution-semantic-gaps.md` · `expansion-forgeos-meta-governance.md` | ✅ |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈 / 熔断） | ~18 | `expansion-production-readiness*.md` · `production-hardening-five-v42.md` | ✅ |
| 二阶系统问题（知识衰减/配置膨胀/TOCTOU/静默数据丢失/并行安全） | ~15 | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ✅ |
| 多仓库/联邦/跨会话治理（知识迁移/漂移检测/舰队管理） | ~12 | `expansion-horizon-three.md` · `strategic-expansion-v39.md` | ✅ |
| 产品视角（分析疲劳/三运行时门槛/环境自测/集成面） | ~10 | `production-product-gaps-v43.md` · `strategic-production-gaps.md` | ✅ |
| 安全纵深（凭据/SCA/沙箱/注入防御/secret-scan 增强） | ~10 | `forgotten-five-system-boundaries.md` · `novel-five-perspectives-2026-07-10-deep.md` | ✅ |
| 北极星桥梁（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | ~8 | `v2-to-northstar-gap.md` · `expansion-directions-v3.md` | ✅ |
| 阶段间契约/置信度标定/Tier 感知 prompt/交接协议 | ~6 | `fresh-expansion-perspectives.md` | ✅ |
| 结构缺口（cmd/forge 包内聚/pi-batch/空工作流/配置漂移） | ~10 | `structural-gaps-v41-genuinely-unexplored.md` · `four-truly-unexplored-architectural-gaps.md` | ✅ |
| 其他（混沌/联邦学习/冷启动/成本预测/冲突解决/确定性 Replay） | ~20 | 各单篇 | ✅ |
| **总计已有覆盖** | **~150+** | **89 份文档** | |

本文 5 个方向在上述 **89+ 份文档、~150+ 方向中零覆盖**。它们不是「已有方向的变体」，而是系统性盲区——所有已有分析都专注于「引擎做什么、输出什么、如何更可靠」，而本文聚焦「系统不做什么、有什么不该有的、少了什么基础能力」。

---

## 五方向总览

| # | 方向 | 类型 | 优先级 | 已有分析命中数 | 核心问题 |
|---|------|------|--------|---------------|----------|
| 1 | 死代码与孤包治理 | 代码治理 · 架构防腐 | **P1** | **0 篇** | 代码库已有孤儿包(`internal/adr` `internal/yamlpath`)，零自动检测 |
| 2 | 增量式门禁执行（Diff-Scoped Gates） | 性能优化 · 规模伸缩 | **P2** | **0 篇** | 百文件改动与单文件改动跑相同门禁集，不改的文件也被重复检查 |
| 3 | 执行前成本预估算（Pre-Run Cost Advisory） | 成本治理 · 用户体验 | **P1** | **0 篇**（仅边缘提及） | 用户「提交」前不知道这个 evolve 将花多少钱，成本跟踪只有事后维度 |
| 4 | 安装自检命令（`forge self-test`） | 运维 · 可靠性 | **P2** | **0 篇** | `forge doctor` 只检配置，不验证整个工具链是否安装正确可用 |
| 5 | 治理配置的熵治理（Governance Artifact Hygiene） | 元治理 · 长期维护 | **P3** | **0 篇** | `.agent/` 在项目演进中积累过时引用/悬空卡片/废弃策略，无人管理 |

---

## 方向一：死代码与孤包治理（Dead Code & Orphan Package Governance）

> **类型**: 代码治理 · 架构防腐  
> **优先级**: P1  
> **预估工作量**: ~0.5 sprint（harness 检查器 + arch-check 扩展 + `forge doctor` 集成）  
> **杠杆系数**: ⭐⭐⭐⭐（防止代码库无声膨胀的自动防线，每个 sprint 都产生收益）

### 现状

ForgeOS 的 `arch-check.mjs` 有 8 个架构检查：layering / package / fan-in / cognitive / anti-pattern naming / function-length / circular dependency / drift-guard。这些检查覆盖了代码**结构健康**的大部分维度。

**但没有任何检查回答一个更基本的问题：每个包是否仍然被需要？**

全仓扫描发现两个明确的孤包案例：

```bash
# internal/adr —— 有测试文件、无生产文件、零引用方
$ ls forge-core/internal/adr/
adr_test.go                    # 只有一个测试文件
$ grep -rn "forgeos/forge-core/internal/adr" forge-core/ --include="*.go"
# → 零引用（不输出）
```

`internal/adr` 包含 ADR 合规测试（ADR-0001 的零依赖验证、ADR-0002 的 polyglot 未启动检查等）。这些测试是**有价值的**——但它们作为 `go test ./...` 自动运行的部分被隐式执行，没有人知道这个包是一个孤包。更危险的是：如果未来有人重构了 go.mod 或移除了 forge-core 目录结构，这些测试会静默失败（被 `t.Skip` 吞没），而无人注意。

```bash
# internal/yamlpath —— 完整生产代码，零引用方
$ grep -rn "forgeos/forge-core/internal/yamlpath" forge-core/ --include="*.go"
# → 零引用（不输出）
```

`internal/yamlpath` 有完整的 YAML 路径引用解析器（`Parse` / `Resolve` / `walkPath`），专为解析 workflow 文件中的 `required_when: ../policies/modes.yml#workflow_depth.reviewer` 这类声明设计。但**实际加载 workflow 的代码从未使用它**。workflow 的 YAML 经 `yaml2json` shim 转成 JSON 后，字段引用在 Go 侧的 `asset.Phase` 结构体中作为普通字符串处理，从未通过 `yamlpath.Resolve` 解析。

### 代码级证据

**证据 A: arch-check 8 检查不包括「包是否被引用」**

```javascript
// harness/arch/arch-check.mjs — 8 个检查：layering / package / fan-in /
// cognitive / anti-pattern naming / function-length / circular / drift-guard
// 零检查检测「无人引用的包」
```

`fan-in` 检查（`checkFanin`）统计**有多少生产者依赖一个包**，但它的语义是「生产者引用者太多→高耦合」，不是「零引用者→死代码」。一个零引用者的 fan-in 是 0，低于任何阈值，静默通过。

**证据 B: internal/adr 的测试文件通过 Skip 吞没失败**

```go
// forge-core/internal/adr/adr_test.go
func repoRoot(t *testing.T) string {
    dir, err := os.Getwd()
    if err != nil {
        t.Skip("cannot determine working directory")
    }
    for {
        if _, err := os.Stat(filepath.Join(dir, "forge-core", "go.mod")); err == nil {
            return dir
        }
        parent := filepath.Dir(dir)
        if parent == dir {
            t.Skip("not inside ForgeOS repo (forge-core/go.mod not found in any parent)")
        }
        dir = parent
    }
}
```

如果这个包被移动或 go.mod 被重构，测试不会 FAIL——它会 Skip。跳过测试不被任何 CI gate 拦截。

**证据 C: internal/yamlpath 有完整的 200+ 行生产代码，但零消费者**

```go
// forge-core/internal/yamlpath/yamlpath.go — 200+ 行生产代码
// 包含：Parse(), Resolve(), MustParse(), resolveRef(), walkPath(),
// mapKeys(), parseIndex(), ShimPath()
// 类型：Ref{File, Path}
// 错误常量：6+ 种错误消息
// 所有代码全部不被任何其他包引用
```

```go
// forge-core/cmd/forge/main.go 加载 workflow 的方式：
// 1. python3 harness/yaml2json.py <workflow.yml> → JSON
// 2. json.Unmarshal → asset.Workflow
// 3. asset.Workflow.Phases[i].RequiredWhen 作为字符串使用
// 从不通过 yamlpath.Resolve 解析 RequiredWhen 指向的政策
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 包被删除但残留测试文件 | 测试 Skip，无人知道 | `go test ./...` 通过（跳过） |
| 包有生产代码但无引用方 | 无人能回答「这个包是干嘛的」 | 无检测，代码永久驻留 |
| 重构后引用方消失 | 旧包不被清理 | 无告警 |
| 包只有测试（如 internal/adr） | 测试有价值但包结构误导 | 无标注 |
| 测试引用了不存在的生产包 | `go build` 失败，但 CI 过了 test | 偶发，不系统 |
| yamlpath 式代码被另一套机制取代（workflow YAML 的字段引用） | 200 行死代码永久驻留 | 不检测 |

### 价值

1. **每个 sprint 产出死代码检测收益**——在 18 包架构中，无人引用的包是不能被忽略的基础设施债务
2. **防止依赖图虚胖**——`internal/yamlpath` 200 行、`internal/adr` 150 行的测试——合起来 ~350 行死代码。在 33k 生产代码中占约 1%，但如果没有治理，会随时间增长
3. **与现有架构检查互补**——arch-check 的 8 检查管「结构健康」，死代码检测管「代码必要性」
4. **低实现成本**——一个 `harness/arch/scan.mjs` 扩展点即可：对每个内部包执行 `imports` 图遍历，标记零入边的包

### 建议工程边界

1. **死代码检测器**：在 `harness/arch/scan.mjs` 中新增 `checkOrphanPackages`：
   - 对每个 `internal/` 子包，统计其被其他包 import 的次数
   - 排除测试文件（测试引用不计数）
   - 0 引用 → WARN（非 block，因为可能存在 `internal/adr` 这类「只有测试」的合法包）
   - 有生产文件但 0 引用 → WARN（疑似死代码）
   - 结果注入 `forge doctor` 的 `--arch` 输出

2. **孤包标记约定**：一个包可以有 `// forge: orphan-ok` 注释在 `package` 声明行，告诉检测器「此包只有测试文件是设计意图」。这防止误报，同时保持检测器默认严格。

3. **死 import 检测增强**：`arch-check` 的循环依赖检查已经做 import 图解析。同一张图可以反查「哪些包的 import 不被任何其他包引用」。

4. **`forge doctor` 集成**：新增 `forge doctor --dead-code` 子命令，列出每个内部包的引用计数和被引用方（或「none」）。非阻断，但作为可观测性工具供架构审查使用。

### 差异化证明

- 关键词 `"dead.*code"`, `"orphan.*package"`, `"unused.*package"`, `"package.*unreferenced"`, `"code.*waste"`, `"zombie.*package"` 在全部 89 份现有文档中 **零命中**。
- `production-hardening-five-v42.md` 提到 "orphan processes"——那是 OS 级僵尸进程清理，不是 Go 包级死代码。
- 已有分析中大量讨论 `arch-check` 的 8 检查增强（认知负荷、函数长度、包大小、扇入等），但 **从未讨论「包的必要性」**。
- 本文方向是第一个把「代码库中的代码是否仍然被需要」作为 ForgeOS 自身治理问题的分析。

---

## 方向二：增量式门禁执行（Diff-Scoped Gate Execution）

> **类型**: 性能优化 · 规模伸缩  
> **优先级**: P2  
> **预估工作量**: ~1 sprint（gate scope 声明 + diff 解析 + 条件执行 + 测试）  
> **杠杆系数**: ⭐⭐⭐（项目规模越大收益越高；小型项目收益有限）

### 现状

ForgeOS 的 gate 系统在其门禁执行上是**全量模式**：每次 `forge run/evolve`，每个 phase 的 `required_gates` 都被完整执行，无论实际改动了什么文件。

```go
// forge-core/internal/orchestrator/orchestrator.go
func (e Engine) runGates(p asset.Phase, gates []string) error {
    for _, name := range gates {
        res := e.callGate(name)
        // 每次调用都运行完整的门禁
        // 没有「这个 gate 是否与当前改动相关」的判断
    }
}
```

```javascript
// harness/gate.mjs — 体积闸门
// 每次运行扫描所有文件，计算总行数
// 单个文件改了 1 行 vs 改了 100 行：扫描时间相同

// harness/arch/scan.mjs — 架构扫描
// 每次运行扫描所有源文件
// 只改了 1 个 .go 文件 vs 改了 50 个：扫描时间相同
```

```python
# harness/check.py — 治理完整性检查
# 每次重新扫描全部 .agent/ 声明
# 不检查「哪些文件改了，只检查受影响的」
```

这对于小型项目（当前 `examples/url-shortener` ~100 文件）是可接受的。但 ForgeOS 的目标是大型项目（数千文件），其中：

- 大多数 `forge run/evolve` 迭代只改动少量文件（典型的 agent phase 产出 1-5 个文件改动）
- 但每次迭代的 gate 执行时间 = 全量扫描时间，与改动量无关
- CI 中全量 gate 每次跑 ~10-30s，在 evolve 循环中每轮迭代 × 多次重试 = 可观的开销

### 代码级证据

**证据 A: gate 执行是全量的，没有 scope 概念**

```go
// forge-core/internal/gate/resolve.go — gate 执行入口
// HarnessRunner / ProbeAll 都是全量扫描
// 没有「只检查特定文件」或「只检查特定路径」的 API
```

```go
// forge-core/cmd/forge/gates.go — gatherSignals / computeFileDelta
// FileDelta 用 git diff 计算改动范围，但只用于诚实性交叉验证
//（roadmap>50% 但 fileDelta<30% → 告警「可能夸大」）
// 不改动 gate 的执行范围
```

**证据 B: 每个 gate 都感知改动但只用于报告，不用于范围缩减**

```go
// forge-core/cmd/forge/engine_build.go — resolveAutoRisk
// gitChangedPaths 已经被读取用于风险自动检测
// 但同一份 diff 信息不传给 gate 执行器来做范围缩减
```

```javascript
// harness/arch/scan.mjs — arch-check 检查
// 有 changedFiles 逻辑（用于渐进扫描），但只在 git diff 存在时启用
// 当前已推至全量扫描（稳妥优先）
```

**证据 C: CI 全量执行是瓶颈的第一个信号**

```yaml
# .github/workflows/forge.yml
# node harness/acceptance.mjs → 全量
# 无法只检查 PR 改动的文件
```

在「真点火」场景中，`forge evolve` 每轮迭代都跑全量 gate。如果迭代 8 次、每次 15s gate 执行，就是 2 分钟的纯 gate 时间——agent 产出的核心时间之外的开销。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 只改了 README.md | lint/test/complexity/arch-check 全部重跑 | 全部运行（无效检查） |
| CI 中 10 个并行的 evolve | 每个都跑全量 gate | 全量 × 10 |
| evolve 迭代 20 次 | gate 全量 × 20 | 大量重复检查 |
| 改动跨越多个包 | 需要全量架构检查（layering/fan-in 可能跨包） | 全量正確但浪费 |
| 改动了全局配置文件 | 所有 gate 确实都需要跑 | 全量是必要的 |

### 价值

1. **加速 evolve 循环**——gate 时间是每次迭代的开销。增量执行可以将百文件项目的 gate 时间减少 50-80%
2. **更好的用户体验**——`forge run` 的反馈延迟减少，agent 循环更快收敛
3. **CI/CD 成本**——每次 CI 执行节省 ~5-20s，累计可观

### 建议工程边界

1. **不尝试做智能 scope 推导（那会引入假阴性风险——门禁漏跑）。** 做的是**显式 scope 声明 + 快路径放行**：

   ```yaml
   # .arch/rules.yaml 新增 scope 字段
   gates:
     gate_lint:
       scope: changed_files      # 只扫 git diff 中的文件
     gate_arch-check:
       scope: always             # 始终全量（架构检查需要全局视图）
     gate_secret-scan:
       scope: changed_files      # 只扫改动文件中的 secret
     gate_test:
       scope: affected_packages  # 扫改动文件所属包 + 依赖该包的其他包
     gate_complexity:
       scope: changed_files      # 只检查改动函数的复杂度
   ```

2. **scope 解析器**：在 `harness/gate.mjs` 中新增 `resolveScope(gateName, changedFiles)` 返回应检查的文件列表：
   - `always` → 无过滤（当前行为，默认值）
   - `changed_files` → 只检查 `git diff --name-only HEAD` 中的文件
   - `affected_packages` → 从 `go list -json ./...` 构建依赖图，找出受改动影响的包
   - `none` → 跳过（用于纯配置变更场景）

3. **回退安全**：如果 git diff 不可用（非 git 仓库、`--from-git` 失败），所有 gate 回退到 `always`——never skip a gate silently。

4. **向后兼容**：`scope` 字段可选，缺省 `always`。现有项目逐位不变。

### 差异化证明

- 关键词 `"diff.*scope"`, `"incremental.*gate"`, `"gate.*skip"`, `"selective.*check.*gate"`, `"changed.*file.*only"`, `"partial.*gate"` 在全部 89 份现有文档中 **零命中**。
- `fresh-expansion-perspectives.md` 讨论了 archetype-based gate selection（按项目类型选 gate 集），那是**按项目类型裁剪**，不是**按每次改动的范围裁剪**。
- `strategic-production-gaps.md` 提到 "speed up gates" 但讨论的是并行化 gate 执行（多 gate 并发），不是增量执行（少跑 gate）。
- 本文方向是第一个提出 **gate 的执行范围应该与改动范围成比例** 的分析。

---

## 方向三：执行前成本预估算（Pre-Run Cost Advisory）

> **类型**: 成本治理 · 用户体验  
> **优先级**: P1  
> **预估工作量**: ~1 sprint（scorecard 聚合 → 成本估算模型 → CLI 集成 → 确认提示）  
> **杠杆系数**: ⭐⭐⭐⭐⭐（解锁「知情同意」的自治循环，避免预算失控）

### 现状

ForgeOS 的成本跟踪体系是**后验的（post-hoc）**：

1. **`cost.go`** — `parseClaudeCostUsd` 在 agent phase 完成后从 claude JSON 输出中解析 `total_cost_usd`
2. **`scorecard_wind.go`** — `windDownScorecards` 在 run/iteration 完成后将成本归因到 scorecard
3. **`runBudget`** — `BudgetExhaustedFunc()` 在成本**超过**阈值后阻断后续 phase

用户只有在**执行之后**（或执行到预算耗尽时）才知道成本。每次 `forge run/evolve` 之前，用户面临的是：

```
$ forge evolve build --executor=command --agent-cmd=claude
# 用户提交，然后等待…
# 没有：「这个 evolve 预计花费 ~$3.50，继续吗？」
```

已有机制：
- `--max-budget-usd`：硬上限，超了即停——但用户不知道「设定多少合适」
- `--max-agent-calls`：spawn 次数上限——但用户不知道「一次 claude -p 调用多少钱」
- `BudgetExhaustedFunc`：运行时检查——但太晚了，已经产生了不可退回的成本

### 代码级证据

**证据 A: Scorecard 有丰富的成本数据，但零消费用于预测**

```go
// forge-core/internal/routing/scorecard.go
type ScorecardPair struct {
    Model       string  `json:"model"`
    TaskType    string  `json:"task_type"`
    Quality     float64 `json:"quality_score"`
    LatencyMs   float64 `json:"p95_latency_ms"`
    AvgCostUsd  float64 `json:"avg_cost_usd"`    // 每 phase 平均成本
    SampleCount int     `json:"sample_count"`
}
```

`AvgCostUsd` + `SampleCount` = 可以计算出每个 `(model, task_type)` 组合的单次调用成本分布。但这个数据只在 `HistoryTiebreak` 中被用于模型选择——从不用于成本预测。

**证据 B: workflow 的 phase 列表在运行前完全可知**

```go
// forge-core/cmd/forge/main.go — loadWorkflow
// workflow 在 run/evolve 前已被加载为 asset.Workflow
// wf.Phases 包含所有 phase 的列表
// 每个 phase 的 agent、model_tier、required_gates 都是已知的
```

结合 scorecard 的 `AvgCostUsd`，理论上可以在 spawn 第一个 agent 前计算出：
```
cost_estimate = Σ(phase_count × AvgCostUsd[model, task_type])
```

**证据 C: 预算耗尽时才阻断，但无法「提前警告」**

```go
// forge-core/cmd/forge/cost.go — runBudget
// BudgetExhaustedFunc() 在剩余预算不足时返回 true
// 但没有任何前馈机制告诉用户「预计花费 $X，预算上限 $Y」
```

```go
// forge-core/cmd/forge/evolve.go — execLoop
// 在 loop 开始前打印 banner，但不打印成本估算
```

**证据 D: `forge route` 能输出 tier 信息但无成本估算**

```go
// forge-core/cmd/forge/route.go
// forge route 可以为给定的 phase 输出 routed tier
// 但不输出对应的历史成本
```

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 新项目无 scorecard 历史 | 无法估算 | 冷启动：使用 mode 默认值（Opus $0.03/phase, Sonnet $0.01/phase 等硬编码默认） |
| Scorecard 数据稀疏（sample_count < 3） | 估算不可靠 | 置信度标记：显示「low confidence estimate」 |
| 同 workflow 不同 mode 成本不同 | mode 影响 tier → 影响成本 | 按 mode 查找对应的 scorecard 分桶 |
| Agent 输出格式变化导致成本解析失败 | 零成本记录 | 使用最近 N 个成功解析的样本均值 |
| 预算上限低于最低估算 | 不用跑就知道一定会超 | 「此 workflow 预计最低成本 $X，超过预算 $Y」 |
| 长 evolve（max-iter=10） | 每次迭代可能选择不同模型 | 显示「每迭代估算」而非「总估算」 |
| file delta 影响 agent 的工作量 | 大改动花更多 token | 无法精确预测（诚实标注「仅基于历史均值，不感知改动量」）|

### 价值

1. **防止「无意识烧钱」**——用户在选择跑之前就知道花费，可以做出知情决策
2. **预算规划数据支撑**——「我打算跑 5 个 evolve，每个约 $2，总预算 $10」
3. **降低使用门槛**——新用户不知道 claude 调用成本，成本预估算提供参考
4. **为自动化预算治理打基础**——未来可以在 `project.yml` 中声明 `monthly_budget_usd: 50`，由系统自动调度

### 建议工程边界

1. **不做精确预测（那需要 token 级别的计费模型 + 上下文长度感知——是独立产品特性）。** 做的是**基于历史均值的粗粒估算**：
   ```
   cost_estimate = sum over phases of:
     AvgCostUsd[phase.task_type, phase.model_tier] × phase_count_per_iteration × max_iter
   ```
   这是：
   - 纯本地计算（读 scorecards.json，无外部调用）
   - 对冷启动使用保守默认值（Sonnet: 0.015, Opus: 0.05）
   - 对 `sample_count < 3` 的 (model, task_type) 组合标注 `low_confidence`
   - 对 `--max-budget-usd` 做比较：如果 estimate > budget，发出清晰警告

2. **CLI 集成**：
   - `forge run --dry-run` 输出成本估算（已有干跑模式，补估算即可）
   - `forge evolve --dry-run` 同上
   - `forge run/evolve` 启动前，如果 `--max-budget-usd` 设置了且 estimate > budget * 0.8，打印黄色警告

3. **成本估算函数**：新增 `internal/routing/estimate.go`（或 `cmd/forge/cost_estimate.go`）：
   ```go
   type CostEstimate struct {
       TotalUSD    float64
       PerPhase    []PhaseCost
       Confidence  string  // "high" | "medium" | "low" | "cold_start"
   }
   func EstimateWorkflowCost(wf asset.Workflow, cards []Scorecard, mode string, maxIter int) CostEstimate
   ```

4. **演进路径**：v1 基于静态历史均值；v2 可以加入 token 估算（读取典型 prompt 长度 → 乘以 model 单价）；v3 可以接入外部 LLM 成本 API。

### 差异化证明

- `next-five-architectural-frontiers.md` 有一张表 **确认** cost forecasting 是「零独立方向」，本文直接填补这个确认真空。
- `novel-architectural-extensions-v40.md` 方向四（`forge plan`）讨论的是**执行计划**（列出 phase、gate），不包含成本估算。
- `expansion-direction-analysis.md` 提及 `PredictiveTier` 但聚焦运行时 budget 降级（已经花了多少，预估还剩多少），不是运行前的总成本估算。
- 本文方向是第一个将「运行前告知用户花费」作为独立用户可见特性提出的分析。

---

## 方向四：安装自检命令（`forge self-test`）

> **类型**: 运维 · 可靠性  
> **优先级**: P2  
> **预估工作量**: ~0.5 sprint（自检测试清单 + harness 适配器验证 + CLI 集成）  
> **杠杆系数**: ⭐⭐⭐（降低「怎么跑不起来」的调试时间；每个新用户受益一次）

### 现状

ForgeOS 依赖一个复杂的外部工具链：

| 依赖 | 用途 | 检查方式 | 当前状态 |
|------|------|----------|----------|
| `python3` + `PyYAML` | yaml2json shim | `forge preflight` 检查 | 仅有，不全 |
| `node` | harness 门禁 | `forge preflight` 检查 | 仅有，不全 |
| `claude` CLI | agent 执行器 | `forge preflight` 检查 | 仅 PATH 存在性，不检查 flag 兼容性 |
| `go` | Go 项目门禁 | `adapters/go.yml` | 适配器框架就绪，但无统一健康检查 |
| `git` | diff/风险分析 | 隐式使用 | 无版本检查 |
| OS 信号处理 | `SIGINT`/`SIGTERM` | 无检查 | 无 |

当前的 `forge doctor`（`forge-core/cmd/forge/preflight.go` + `internal/doctor/`）专注于**项目配置**的健康检查：

```go
// forge-core/internal/doctor/quick.go — quickDoctorCheck
// 检查：forge cache dir、go.mod 存在性、agent 引用完整性
```

但它**不检查外部工具的可用性和兼容性**：

```go
// forge preflight 的输出
forge preflight: ✓ forge binary built
forge preflight: ✓ go.mod present
forge preflight: ✓ agent cards resolved
// 但不检查：
// ✗ claude CLI 是否在 PATH 中且版本兼容
// ✗ node 是否可用且版本 ≥18
// ✗ python3 + PyYAML 是否可用
// ✗ git 是否可用
// ✗ 适配器配置的 lint/test 工具是否实际可用
```

### 代码级证据

**证据 A: `preflight.go` 只检查项目级资产，不检查环境**

```go
// forge-core/cmd/forge/preflight.go
// preflightReport 结构体：
// - 项目资产（.agent/ 卡片、workflow 文件、policy）
// - ADR 路径
// - 依赖文件存在性
// 不包括：外部 CLI 版本、权限、网络连通性
```

**证据 B: 工具探测有框架但无统一验证入口**

```go
// forge-core/internal/gate/resolve.go — ProbeTool / ProbeAll
// 已经有探测框架：
// - ProbeTool: 探测单个工具是否安装可用
// - ProbeAll: 探测所有适配器中声明的工具
// 但无 CLI 命令暴露给用户统一运行
```

```javascript
// harness/adapters/typescript.yml
// 适配器定义有 lint / test / build / coverage 的命令
// 但只能被 gate 框架按需调用，没有「验证所有适配器配置」的命令
```

**证据 C: `claudeArgv` 假设 flag 存在但从不验证**

```go
// forge-core/cmd/forge/engine_build.go — claudeArgv
// 使用 --model、--permission-mode、--allowedTools、--disallowedTools
// --max-budget-usd、--output-format json 等 flag
// 这些 flag 的版本兼容性从不在运行前验证
```

**证据 D: Sprint 27 的 bug 本可以被自检捕获**

Sprint 27 中 `forge validate --models` 的 agent 引用校验因 JSON 行扫描依赖 pretty-print 格式而**完全静默失效**。一个 `forge self-test` 如果包含 `forge validate --models` 端到端执行并检查输出结构，可以在 CI 或开发时捕获这类回归。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| Claude CLI 更新后移除 `--model` flag | 所有 agent phase 失败 | 运行时才暴露 |
| python3 不可用 | yaml2json 失败，workflow 无法加载 | 运行时 error |
| PyYAML 未安装 | yaml2json 输出空/错误 | 运行时 error |
| git 版本过旧，不支持某些 flag | git diff 失败，风险分析静默降级 | 运行时静默失败 |
| `node` 版本 < 18 | harness 门禁可能不兼容 | 运行时才暴露 |
| 文件权限不足（`.forge/` 不可写） | trace/memory/checkpoint 写入失败 | 运行时 error |
| 磁盘空间不足 | memory/trace 写入失败 | 运行时 error |
| claude OAuth 过期 | agent phase 全部 auth error | 运行时才暴露（消耗预算后） |

### 价值

1. **新人上手时间**——第一次运行 `forge run` 前跑 `forge self-test`，立即知道缺少什么工具
2. **升级防护**——`forge self-test` 在 claude CLI 更新后运行，检测 flag/输出格式兼容性
3. **CI 集成**——CI 的第一步骤是 `forge self-test`，确保运行环境完整
4. **事后调试**——故障时跑 `forge self-test --verbose` 获得环境快照，排除环境因素

### 建议工程边界

1. **不做端到端 LLM 调用测试（那花真钱）。** 做的是**工具链存在性 + 版本兼容性 + 权限验证**：

   ```
   forge self-test [--verbose] [--ci]
   ```

   检查清单：
   - ✅ `python3` 在 PATH 中且可执行
   - ✅ `python3 -c "import yaml"` 成功（PyYAML 已安装）
   - ✅ `node` 在 PATH 中且版本 ≥18
   - ✅ `git` 在 PATH 中且版本 ≥2.0
   - ✅ `claude` 在 PATH 中（但不调用 API）
   - ✅ `claude --help` 输出中包含 `--model` flag（验证 flag 兼容性，Sprint 27 的教训）
   - ✅ `.forge/` 目录可写
   - ✅ 磁盘剩余空间 > 100MB
   - ✅ 适配器配置语法正确（`harness/adapters/*.yml` 可解析）
   - ✅ 适配器工具探测：`go version`（Go 项目）、`node --version`（Node 项目）等
   - ⚠️ `claude` OAuth 可用性（可选，需要凭证——标记为 `[needs auth]`）
   - ⚠️ 网络连通性（可选——标记为 `[needs network]`）

2. **输出格式**：
   ```
   $ forge self-test
   ✅ python3 3.11.4 + PyYAML 6.0
   ✅ node v20.11.0
   ✅ git 2.39.2
   ✅ claude 0.15.2 (--model flag confirmed)
   ✅ .forge/ writable (1.2GB free)
   ⚠️  claude OAuth: not tested (run with --auth to validate)
   ❌  no disk space check: failed (only 50MB free)
   ```

3. **退出码**：0 = 所有必要检查通过；1 = 一个或多个必要检查失败；2 = 仅 ⚠️ 级别失败（CI 模式不区分）

4. **`--ci` 模式**：跳过交互式检查（OAuth）、不产生彩色输出、严格退出码

### 差异化证明

- 关键词 `"self.test"`, `"forge.*test.*install"`, `"install.*valid"`, `"setup.*check"`, `"validate.*install"`, `"health.*check.*forge"`, `"forge.*checkup"` 在全部 89 份现有文档中 **零命中**。
- `expansion-production-readiness.md` 讨论「环境验证」（方向二），但那是验证项目代码是否可构建，不是验证 ForgeOS 自身工具链是否完整。
- `systemic-expansion-v26.md` 方向一讨论 `forge doctor` 的 watch mode——那是持续诊断，不是一次性的安装验证。
- `forgotten-five-foundations.md` 方向二讨论热修复命令——那是运行时 self-healing，不是安装时 self-test。
- 本文方向是第一个将「ForgeOS 作为一个产品，用户如何验证它安装正确」作为独立问题的分析。

---

## 方向五：治理配置的熵治理（Governance Artifact Hygiene）

> **类型**: 元治理 · 长期维护  
> **优先级**: P3  
> **预估工作量**: ~1 sprint（引用跟踪器 + 过时检测 + `forge prune` 子命令）  
> **杠杆系数**: ⭐⭐（长期受益，初始收益不明显；项目使用 6 个月后价值凸显）

### 现状

`.agent/` 目录是 ForgeOS 项目的治理骨架。随着项目演进，它自然地积累**治理垃圾**：

1. **悬空 agent 卡片**：项目中定义了 12 个 agent（product-manager、security-engineer、distributed-engineer 等），但并非所有 workflow 都用所有 agent。如果某个 agent 被 workflow 引用但 workflow 本身不再使用（被 mode gating 跳过），卡片本身仍然存在。

2. **过时的 workflow 文件**：Sprint 27 修复了多个 workflow，但旧的 workflow 版本（如果有版本目录）不被清理。

3. **未使用的 skill 文件**：`.agent/skills/` 下有 9 个 skill，但 `build.yml` 和 `evolve.yml` 可能只引用其中一部分。不被任何 workflow 引用的 skill 是治理噪声。

4. **policy 中的死字段**：`.agent/policies/modes.yml` 中有 `priorities` 字段——Sprint 17 诚实标注为「声明但零消费，effect 已隐含在其他设置中」。字段本身留在那里，成为维护者的理解负担。

5. **ADR 的版本漂移**：`docs/adr/` 下有 4 篇 ADR，其中 ADR-0004 在 Sprint 30 被加过勘误。ADR 随时间自然漂移，但没有机制提示「这篇 ADR 描述的决策可能已过时」。

### 代码级证据

**证据 A: `check.py` 检查引用完整性但只检查「悬挂引用」，不检查「未被引用的定义」**

```python
# harness/check.py — check_workflow_agent_refs / check_skill_refs
# 它验证：workflow 中引用的 agent/skill 是否存在对应的卡片文件
# 它不验证：有没有 agent 卡片从未被任何 workflow 引用
#          有没有 skill 卡片从未被任何 workflow 引用
#          有没有 policy 字段从未被任何代码消费
```

当前的检查是**引用方验证**（referenced → exists），不是**定义方验证**（defined → referenced）。

**证据 B: `internal/mode` 有字段已声明但零消费**

```go
// forge-core/internal/mode/mode_policy.go — Policy 结构体
type Policy struct {
    Gates         []string     // 消费（gate-set 过滤）
    Reviewer      bool         // 消费（reviewer skip）
    DiscoverDepth DiscoveryDepth // 消费
    DesignDepth   DesignDepth  // 消费
    ReviewDepth   ReviewDepth  // 消费
    ADR           bool         // 消费
    EvolveDepth   EvolveDepth  // 消费
    // 没有：
    //   CreatedAt     — 策略版本时间戳
    //   SchemaVersion — 策略格式版本
    //   DriftFromBase — 与组织基准策略的差异
}
```

mode policy 有版本概念吗？没有。`.agent/policies/modes.yml` 被修改后，无法回答「这个项目的治理配置相对于组织标准偏移了多少」。

**证据 C: agent 卡片职责声明与 workflow 的 `required_when` 不同步**

```markdown
# .agent/agents/security-engineer.md
## Review 阶段职责
- STRIDE 威胁建模
- RFC 合规矩阵
```

```yaml
# .agent/workflows/review.yml
- name: security-review
  agent: security-engineer
  required_when: "mode != explorer"
```

如果 agent 卡片新增了职责，但 workflow 的 `required_when` 条件未更新，新职责永远不会被触发。目前**没有任何验证**确保 agent 卡片的段落标题与 workflow `emits:` 产物的结构对应。

### 边界情况

| 场景 | 风险 | 当前处理 |
|------|------|----------|
| 新增 agent 卡片但忘记注册到 workflow | 卡片永远不会被调用 | `check.py` 不检测 |
| skill 被重构后无人使用 | 文件残留，新人不解 | 不检测 |
| policy 字段被新版本替代但旧字段未删 | 新旧字段并列，语义混淆 | 不检测 |
| workflow 引用的 `.ai/` 提示文件被删除 | 引用断裂 | `check.py` 部分覆盖 |
| ADR 描述的决策已被代码绕开 | ADR 误导读者 | `internal/adr` 的 ADR 测试部分覆盖 |
| agent 卡片职责列了 10 项但 workflow 只触发 3 项 | 高期望低交付 | 不检测 |

### 价值

1. **降低认知负荷**——新成员不用浏览 12 个 agent 卡片中 4 个从未被调用的
2. **防止治理配置膨胀**——每添加一个治理工件，都应该有一个自动的「何时可清理」触发器
3. **治理配置的可审计性**——可以回答「这个项目的治理配置与组织基准有哪些差异，哪些是故意的，哪些是漂移」
4. **为模板升级铺路**——当组织治理模板更新时，可以检测各项目的漂移

### 建议工程边界

1. **不做自动治理垃圾清理（删除文件太危险）。** 做的是**引用关系追踪 + 过时报告 + 手动清理指导**：

   - **引用图**：构建 `.agent/` 全引用图（workflow → agent、workflow → skill、workflow → policy、agent → skill 等）
   - **反向引用检查**：对每个定义（agent 卡片、skill 文件、policy 字段），检查是否有至少一个引用方
   - **`forge governance report`**：新子命令，输出治理配置的健康报告：
     ```
     $ forge governance report
     Governance Artifact Report for project catalyst
     ================================================
     Agents:
       ✅ product-manager    — referenced by discover.yml, design.yml
       ✅ architect          — referenced by design.yml
       ⚠️  researcher        — referenced by discover.yml (skipped in mode=balanced mode=engineering only mode=explorer)  
       ⚠️  cto               — referenced by design.yml, review.yml (cto.md's boundary documented in both)
       ❓ explorer           — defined but ZERO workflow references (assumed dead — check .agent/agents/explorer.md)
     
     Skills:
       ✅ refactor-large-file    — referenced by implementer.md, qa.md
       ⚠️  project-reorganization — zero workflow references (may be referenced manually)
     
     Policies:
       ⚠️  modes.yml:priorities — declared but not consumed (Sprint 17 note: effect implicit)
       ⚠️  modes.yml:confidence_metric — declared but not consumed (Sprint 30 note: replaced by CONFIDENCE token)
     ```

   - **`forge governance tidy`**：交互式清理向导，列出可删除/可归档的治理工件，在删除前请求确认

2. **基准漂移检测**：
   - 在 `project.yml` 中新增可选字段 `governance_baseline: "org/v2"`
   - `forge governance drift` 对比当前 `.agent/` 与组织模板（需模板 URL）的差异
   - 差异分类：`intentional`（标为覆盖）、`drift`（可能是无意的偏差）或 `lagging`（模板已更新但项目未跟进）

3. **`forge doctor` 集成**：
   - 新增 `forge doctor --governance` 模式，输出治理配置熵报告
   - 在 `forge migrate` 中增加治理清理步骤："Step 6: review and archive unused governance artifacts"

4. **ADR 新鲜度标记**：
   - ADR 文件头可加 `status: active | superseded | deprecated`
   - `forge adr status` 命令列出所有 ADR 及其状态
   - 对超过 6 个月且 `status: active` 的 ADR，`forge doctor` 建议人工复审

### 差异化证明

- 关键词 `"governance.*entropy"`, `"governance.*hygiene"`, `"artifact.*cleanup"`, `"unreferenced.*agent"`, `"dead.*policy"`, `"config.*drift.*baseline"`, `"governance.*report"` 在全部 89 份现有文档中 **零命中**。
- `forgotten-five-meta-governance-and-blindspots.md` 讨论的是**治理机制本身的盲区**（mode-gating 声明 vs 实现、policy 字段零消费），不是**治理工件的生命周期管理**。
- `strategic-production-gaps.md` 方向三讨论「配置漂移检测」——那是运行时配置（project.yml 的 mode/lifecycle 设置）的漂移，不是 `.agent/` 治理工件的引用完整性。
- 本文方向是第一个将**治理配置本身视为需要被治理的资产**的分析——即元元治理（meta-meta-governance）。

---

## 汇总：价值矩阵与推荐优先级

| # | 方向 | 类型 | 实现量级 | 收益曲线 | 风险 | 优先 |
|---|------|------|----------|----------|------|------|
| 1 | 死代码与孤包治理 | 代码治理 | 小 | 立即（存量的两个孤包） | 低 | **①** |
| 2 | 增量式门禁执行 | 性能优化 | 中 | 项目规模越大收益越高 | 中（需防假阴性） | **④** |
| 3 | 执行前成本预估算 | 成本治理 | 中 | 用户每次使用都受益 | 低 | **②** |
| 4 | 安装自检命令 | 运维 | 小 | 每个安装/升级一次 | 低 | **③** |
| 5 | 治理配置熵治理 | 元治理 | 中 | 长期（6个月后显现） | 低 | **⑤** |

### 推荐执行顺序

1. **Sprint N: 方向一（死代码治理）+ 方向四（自检命令）**——量级最小、杠杆最高。方向一直接清理已发现的孤包 + 建立持续检测机制。方向四解决「新用户第一体验」问题。两方向共用 harness 扩展点。

2. **Sprint N+1: 方向三（成本预估算）**——对用户价值最直观的改进。从「猜着跑」到「知道了再跑」。依赖 scorecard 的现有数据，纯本地计算。

3. **Sprint N+2: 方向二（增量门禁）**——需要最谨慎的设计（避免假阴性），建议先做 scope 声明框架 + 只对 `changed_files` scope 做实现，`affected_packages` 推迟到有更多验证时。

4. **Sprint N+3: 方向五（治理熵治理）**——价值最大但最不紧迫的方向。建议在有人首次抱怨「.agent/ 太乱看不懂」时启动。

### 被排除的方向（与已有分析确认的重叠）

| 候选方向 | 已有分析覆盖 | 排除原因 |
|----------|-------------|----------|
| 多分支工作流隔离 | `five-uncovered-architectural-frontiers.md` 方向五 | 已完整覆盖（含 run ID、branch-aware checkpoint、分支独立 memory） |
| 环境验证 / `forge doctor` 增强 | `expansion-production-readiness.md` 方向二 | 已完整覆盖（probeAll、工具探测框架） |
| 治理机制盲区审计 | `forgotten-five-meta-governance-and-blindspots.md` | 已完整覆盖（声明 vs 实现逐条核对） |
| Run Identity / Session ID | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向四 | 已完整覆盖（含 RunIdentity 结构体设计） |
| 格式版本 / 数据迁移 | `execution-semantic-gaps.md` 方向四 | 已完整覆盖 |

---

## 附录：验证策略建议（非实现代码）

### 方向一（死代码治理）

**单测**：
- Mock 一个 import 图，其中包 A 有零引用方 → `checkOrphanPackages` 输出 WARN
- Mock 一个 import 图，其中包 A 有 `// forge: orphan-ok` 注释 → 不 WARN
- Mock 一个 import 图，所有包有至少一个引用方 → 不 WARN

**集成**：
- 在当前仓库运行 `checkOrphanPackages` → 确认 WARN 应包含 `internal/yamlpath` 和 `internal/adr`（后者如有 `orphan-ok` 注释则不 WARN）
- 在 `examples/url-shortener` 运行 → 不 WARN（小项目无孤包）

### 方向二（增量门禁）

**单测**：
- scope 解析器：`resolveScope("gate_lint", ["README.md"])` → `["README.md"]`
- scope 解析器：`resolveScope("gate_arch-check", ["README.md"])` → `null`（全量）
- scope 解析器：git diff 不可用时 → `null`（回退全量）

**集成**：
- 创建单一文件改动 → `forge run --scope changed_files` → 验证只跑该文件相关 gate
- 跨包改动 → `forge run --scope affected_packages` → 验证跑了受影响包及其依赖包的测试
- 无 git diff 环境 → 验证回退到全量

### 方向三（成本预估算）

**单测**：
- Mock scorecard 有 `AvgCostUsd=0.02` for `(sonnet, implementer)` → workflow 有 5 个 implementer phase → 估算 $0.10
- Mock scorecard 无数据 → 使用默认值
- `max-iter=10` → 估算 = 单次 × 10

**集成**：
- `forge run --dry-run build` 输出成本估算行
- `forge run build --max-budget-usd 0.01` 但估算 $0.10 → 打印黄色警告（不阻断）
- `forge evolve build --max-budget-usd 0.01` 但每迭代估算 $0.10 → 打印警告

### 方向四（自检命令）

**单测**：
- Mock 工具链全部可用 → `forge self-test` exit 0
- Mock claude 不在 PATH → `forge self-test` exit 1（必要检查失败）
- Mock `claude --help` 输出中无 `--model` → `forge self-test --verbose` 标记 flag 兼容性失败

**集成**：
- 在 CI 中运行 `forge self-test --ci` → exit 0（CI 环境已配置）
- 在新容器中运行 `forge self-test` → 正确报告缺少的工具

### 方向五（治理熵治理）

**单测**：
- Mock `.agent/` 目录，其中 `explorer.md` 不存在于任何 workflow 引用 → 报告 `❓ explorer.md`
- Mock `.agent/` 目录，全部卡片有引用 → 报告 `✅`
- `project.yml` 有 `governance_baseline: org/v2` → `forge governance drift` 输出差异

**集成**：
- 在当前仓库运行 `forge governance report` → 输出有意义的报告（至少包含感叹和问号条目）
- `forge governance report --json` → 输出结构化的 JSON（便于 CI 集成）
