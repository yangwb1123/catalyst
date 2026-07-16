# ForgeOS — 五个尚未被任何已有分析覆盖的结构性扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫: forge-core 18+ Go 包 / `cmd/forge` 16+ 子命令 / harness 39+ 模块 /  
>    `.agent/` 完整治理骨架（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies）  
> 2. Sprint 1–31 完整演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE + GAP 全收口）  
> 3. **逐篇通读 33 份 `docs/requirements/*.md` + 41 份 `docs/analysis/*.md` + 其余 docs 共 116 份文档**，  
>    逐方向核对核心论点——**排除所有已被覆盖的方向**  
> 4. **差异化证明**: 每个方向明确引用代码级证据，说明为什么未被现有 70+ 方向覆盖  
> **纪律**: 不编写任何代码。  
> **日期**: 2026-07-10

---

## 已有 70+ 方向全景（本文不重复）

| 已被充分覆盖的域 | 代表性文档 | 方向数 |
|-----------------|-----------|--------|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行） | `high-value-extension-directions.md` · `v3` · `v34` · `v33` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级/修正学习） | `expansion-horizon-three.md` · `novel-five-frontiers-v34.md` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约） | `expansion-production-readiness.md` · `v34` | ~8 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` · `v33` 方向一二 | ~10 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性/并行安全） | `strategic-extensions-v22~v33.md` · `v38` · `uncovered-frontiers-v25.md` | ~12 |
| 被遗忘的基础（跨进程守护/状态自校验/Governance 热加载/Trace 查询 CLI/可插拔扩展） | `forgotten-five-foundations.md` | ~5 |
| Gate 执行经济学 / 记忆去重 / 墙钟预算 / `forge plan` | `novel-architectural-extensions-v40.md` 方向一~四 | ~5 |
| 治理策略测试框架 / 阶段间一致性守卫 / 契约履约监控 | `novel-five-highvalue-extensions.md` 方向一~五 | ~5 |
| Model-Tier-Aware Context 预算 / 自动生命周期推进 / 跨运行 Trace 分析 | `strategic-extensions-v32.md` 方向一 · `genuinely-uncovered-five-frontiers.md` | ~5 |
| 其他单篇覆盖方向（知识启动协议/冷启动/外部SDLC集成/Agent输出回归检测/编排器Hook契约/存储抽象层/提示可观测性等） | 各单篇文档 | ~10 |
| **总计已有覆盖** | **75 份文件** | **~85+ 方向** |

---

## 核心判断

在通读全部 116 份已有文档后，本文识别出的这 5 个方向**在已有分析中从未作为独立方向展开、未被写入代码、没有设计文档**。每个方向都带差异化证明，说明与已有覆盖的明确边界。

这 5 个方向的共同特征：它们不是「功能缺失」或「性能优化」，而是**结构性的架构债务**——当前设计没有为这些需求留出空间，但随着 ForgeOS 从 v2 单体向 north-star 分布式架构演进，这些债务将从「今天可以忽略」变成「明天必须拆除的墙」。

---

## 目录

1. [方向一：Go 库 API 边界契约 · 从 CLI-sandwich 到可消费的库](#方向一go-库-api-边界契约)
2. [方向二：测试质量元治理 · 谁来守卫守卫者](#方向二测试质量元治理)
3. [方向三：韧性验证框架 · 混沌工程 for ForgeOS 引擎](#方向三韧性验证框架)
4. [方向四：产物质量治理 · 从过程执法到产出执法](#方向四产物质量治理)
5. [方向五：配置 Schema 版本化与迁移管线](#方向五配置-schema-版本化与迁移管线)

---

## 方向一：Go 库 API 边界契约（从 CLI-sandwich 到可消费的库）

**类型**: 架构 · API 设计 · 可演化性  
**优先级**: P1（north-star 分布式化的结构性前提）  
**差异化证明**: 70+ 已有方向全部将 forge-core 视为**CLI 工具**或**内部包集合**。没有一个方向讨论 forge-core 作为 **Go 库**的 API 契约——这是向 north-star 演进的盲区。

### 现状：代码级证据

ForgeOS v2 的包结构遵循 Go 惯例：

```
forge-core/
  cmd/forge/       ← CLI 入口（package main）
  internal/        ← 所有引擎逻辑（包路径含 internal，Go 强制不可外部导入）
    asset/
    converge/
    gate/
    memory/
    mode/
    orchestrator/
    prompt/
    risk/
    routing/
    trace/
    ...
```

**证据 A：所有引擎包都是 `internal/`，外部无法导入**

```go
// forge-core/go.mod
module forgeos/forge-core
// ← 包路径是 forgeos/forge-core，不是可消费库路径
// ← 无 `go doc` 导出文档
// ← 无 `internal/` 包的公开契约（Go 编译器强制拒绝外部导入）
```

这意味着：任何 north-star 架构中的独立服务（Router Service、Eval Engine、Policy Service）如果需要引用 forge-core 的路由/收敛/策略逻辑，**只有两条路**：

1. **复制代码**（fork 整个包 → 双份维护，无版本对齐）
2. **CLI 子进程调用**（`exec forge route --json` → 进程开销 + JSON 序列化脆弱）

**证据 B：`cmd/forge` 持有所有引擎的装配逻辑，外部无程序化入口**

```go
// forge-core/cmd/forge/main.go:101-180
func run(args []string) int {
    // ← 全部装配代码硬编码在 main 包中
    // ← Engine 的构建、模式的解析、trace 的配置……
    // ← 没有 NewEngine() / NewRouter() / NewEvaluator() 导出函数
}

// forge-core/internal/orchestrator/orchestrator.go
type Engine struct {
    Exec          AgentExecutor  // ← 接口, 但结构体不导出
    RunGate       func(string) gate.Result  // ← 函数字段, 不是接口方法
    ModePolicy    mode.Policy
    MaxRetries    int
    MaxLoopBack   int
    MaxAgentCalls int
}
// ← 没有 NewEngine() 构造器——必须通过 CLI main 函数拼装
// ← 没有 godoc、没有 API 兼容性承诺
```

**证据 C：导出符号数量极少，且不为外部消费设计**

```bash
$ grep -rn "^func \|^type \|^var \|^const " forge-core/internal/ --include="*.go" | grep -v "_test.go" | grep -v "^func.*internal" | wc -l
# 大量导出符号但全部被 internal/ 挡住

$ grep -rn "package internal" forge-core/ --include="*.go"
# 全部引擎包都是 internal → 对仓库外部的项目完全不可见
```

**证据 D：无版本化 API 文档、无弃用策略、无 semver**

```bash
$ grep -rn "Deprecated\|deprecated\|BREAKING\|semver\|v1\.\|v2\." forge-core/internal/ --include="*.go" | head -5
# → 零（无版本标记、无弃用注释、无 API 变更策略）
```

### 为什么 north-star 架构需要它

North-star 架构将引擎拆分为独立服务：

```
当前 (v2)                       north-star (目标)
┌─────────────────┐           ┌──────────  ──────────┐
│   forge CLI     │           │ Router(svc) │ Eval(svc) │
│  ┌───────────┐  │           ├────────── ──────────┤
│  │ orchestrator│ │           │ Orchestrator (Temporal) │
│  │ routing    │  │           ├──────────────────────┤
│  │ converge   │  │           │ Policy/Gov (OPA)     │
│  │ memory     │  │           ├──────────────────────┤
│  │ trace      │  │           │ Context Engine (RAG) │
│  └───────────┘  │           └──────────────────────┘
└─────────────────┘
```

每个 north-star 服务需要用**共享的路由/收敛/策略逻辑**——不复制代码、不走 CLI 子进程。这意味着 `internal/` 的包必须变成**具有稳定 API 契约的可导入库**。

### 需要什么

| 组件 | 当前状态 | 需要 |
|------|---------|------|
| 包可见性 | 全部 `internal/` | 将稳定 API 提升到 `pkg/`（按 Go 惯例）或非 `internal` 的导出包 |
| 构造器 | 硬编码在 `main.go` | 导出 `NewEngine()`、`NewRouter()`、`NewConverge()` 等 |
| API 版本化 | 无 | 声明 API 稳定性等级（稳定/实验性/内部），引入 semver 标签 |
| 弃用机制 | 无 | `Deprecated: use X since v2.1` 注释 + 编译时警告机制 |
| 接口化 | Engine 用函数字段 | 用接口类型替代函数字段，让外部可注入 mock / alternative impl |
| 文档 | `package doc` 面向内部 | godoc 面向库消费者，注明线程安全/生命周期/错误处理契约 |
| 依赖注入 | 隐式全局/硬编码 | 构造函数显式接收依赖，不依赖包级变量 |

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 外部包调用内部 API | Go 编译器拒绝编译 | 将稳定 API 提升到非 `internal` 包，不稳定 API 留在 `internal` |
| API 变更破坏外部消费者 | 下游服务编译失败 | 声明稳定 API 的 semver 兼容性策略；实验性 API 标记为 `// Experimental` |
| 循环依赖（两个服务互相导入） | Go 无法编译 | 按 layering 单向依赖设计 API 边界，同现有架构纪律 |
| 第三方贡献者不知道 API 约定 | 错误使用内部结构 | godoc + 示例测试 + CONTRIBUTING.md 的 API 契约段 |
| 测试需要 mock 引擎 | 测试代码复杂 | 导出 `Engine` 接口（非具体 struct）+ mock 生成器 |

### 价值

1. **north-star 分布式化的前提**: 没有稳定的库 API，每个微服务都只能 fork 代码——这正是当前 `internal/` 架构的死角
2. **生态可扩展**: 第三方可以写 ForgeOS 插件（自定义 Executor、自定义 Router 策略），而不必修改 forge-core 源代码
3. **可测试性提升**: 导出接口让 forge-core 单元测试可以 mock 外部依赖，摆脱当前"全量集成测试"的模式
4. **架构纪律**: API 边界即模块边界——强迫团队在变更之前思考"这是公共接口还是内部实现"，减少无意中的耦合

---

## 方向二：测试质量元治理（谁来守卫守卫者）

**类型**: 质量保障 · 治理连续性  
**优先级**: P1（治理系统的自反完整性）  
**差异化证明**: 已有分析中超过 10 个方向讨论「治理执法」、5+ 方向讨论「测试框架」、3 个方向讨论「自检」。但没有一个方向讨论 **测试代码本身的质量治理**——即 ForgeOS 对其自身测试套件的完整性、健壮性和演化的治理。

### 现状：代码级证据

ForgeOS 有大量测试，但对测试本身没有任何质量门控：

**证据 A：测试规模——77 Go 测试文件 + 28 个 harness 测试，但零测试质量指标**

```bash
$ find forge-core -name "*_test.go" | wc -l
77

$ find harness -name "test_*" -type f | wc -l
28
```

没有任何机制追踪：
- 测试覆盖率趋势（覆盖度是升还是降？）
- 测试执行时间趋势（测试是否因增长而变慢？）
- 测试稳定性（是否有 flaky 测试？）
- 测试覆盖率与代码变更的相关性（新功能是否带了测试？）

**证据 B：`forge accept` 测试套件是 load-bearing，但自身不承受同样的治理**

```javascript
// harness/acceptance.mjs — 编排全套 gate 并判 ACCEPTED/REJECTED
// 但该文件自身没有承受以下治理：
//   □ 行数限制（当前 345 行，但没有测试该文件是否接近 500 行阈值）
//   □ 函数长度限制（acceptance.mjs 的函数未被 scan-functions.mjs 覆盖）
//   □ 循环依赖检查（独立的 acceptance-quality.mjs 和 acceptance-kernel.mjs 的无环性未被验证）
```

**证据 C：测试覆盖率数据有框架但无趋势追踪**

```bash
# .agent/project.yml 声明了 coverage_min: 80
# harness/adapters.mjs 能调用覆盖率工具
# 但没有：
#   - 覆盖率历史的追踪（上周 82% → 这周 79% → 无人感知）
#   - 覆盖率下降的阻断（gate 不检查覆盖率趋势）
#   - 覆盖率 per-package 的分解（某些包 90%、某些包 0%，平均 80% 掩盖了极端）
```

**证据 D：Flaky 测试零检测**

```bash
# forge-core 没有 flaky 测试检测机制
$ grep -rn "flake\|rerun\|retry.*test\|test.*stability" forge-core/ --include="*.go"
# → 零
```

77 个 Go 测试文件中如果有一个 flaky 测试，CI 会间歇性失败，团队会逐渐忽视红色 CI（警戒疲劳）——这正是治理系统自己不该发生的「治理债务」。

**证据 E：无测试与代码的关联性门控**

```bash
# forge-core/internal/converge/converge.go 有 304+ 行
# converge_test.go 的覆盖率是多少？无人追踪
# 新加的 converge 逻辑是否必有配套测试？无强制

# harness/secret-scan.mjs 上次修改后是否新增了测试？
# 无 diff-level 测试完整性守卫
```

### 需要什么

**第一层：测试质量可观测性（~0.5 sprint）**

新增 `forge test --quality` 命令，报告：

```
Test Quality Report for forge-core (2026-07-10)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Coverage:         81.3% (↑0.7% from last week)
  internal/converge: … 94.2%
  internal/routing:  … 88.1%
  cmd/forge:           … 72.4% (↓2.1% — WARN: trending down)
Test count:       707 (+12 from last week)
Test time:        4.2s (↑0.3s — stable)
Flaky tests:      0  (last 10 runs)
Untested packages: 0
New code without tests: 3 files (see forge test --audit)
```

**第二层：测试完整性守卫（~0.5 sprint）**

接入现有 `policies.yml` 或新建 `test-policies.yml`：

```yaml
test_governance:
  coverage_trend:
    max_delta_per_week: 2    # 每周覆盖率下降超过 2% → WARN
    max_delta_per_sprint: 5  # 每个 sprint 下降超过 5% → FAIL
  flaky_detection:
    rerun_count: 3           # CI 自动 rerun flaky 测试
    max_flaky_ratio: 0.01    # flaky 测试不能超过总测试的 1%
  new_code_test_requirement:
    min_test_per_new_file: 1 # 每个新增生产文件必须有至少一个测试文件
  gate_redundancy:
    check: true              # 检查测试 gate 自身是否被治理 gate 覆盖
```

**第三层：自反治理（~1 sprint）**

测试 `harness/` 自身的治理完整性——即 `forge accept` 的测试是否被 `forge accept` 的治理覆盖。

```
┌─────────────────────────────────────────┐
│  forge accept                            │
│    ├ gate.mjs ── 检查行数/根目录数       │
│    ├ arch-check ── 架构 8 检查           │
│    ├ secret-scan ── 硬编码 secret        │
│    ├ check.py ── 治理完整性              │
│    ├ go test ── Go 测试                  │
│    └ node --test ── harness 测试         │
│                                          │
│  但谁检查 forge accept 自身？            │
│    → gate.mjs 自己不被 gate.mjs 检查行数 │
│      （gate.mjs 自己没有"不超过 500 行"  │
│       的自动检测——需要人工维护）          │
│    → acceptance-kernel.mjs 的循环依赖    │
│      不被任何 gate 检查                  │
│    → check.py 的测试覆盖率无人跟踪        │
└─────────────────────────────────────────┘
```

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 测试质量 gate 自身没测试 | 递归无底 | 新 gate 只做可观测 + WARN，不做 blocking，需跑起来积累数据 |
| Flaky 测试误判 | 良好测试被标记为 flaky | 需要至少 3 次失败 / 10 次运行才认定 flaky |
| 覆盖率下降有正当理由（如大规模重构） | 覆盖率门控阻碍重构 | 允许在 sprint 计划阶段声明 `coverage_exemption` |
| 测试时间增长 | CI 变慢 | 新增 `forge test --fast`（只跑受影响包的测试，类似 select-tests 模式） |

### 价值

1. **治理可信度**: ForgeOS 的核心承诺是「治理即真相之源」。不治理测试质量，意味着「真相之源」本身没有质量保证——这是一个自反性的信任漏洞
2. **防止测试债务**: 代码债务可见（gate 检查行数/复杂度），测试债务不可见（无 flaky 追踪、无覆盖率趋势）。不可见的债务积累更快
3. **自愈文化**: 如果 `forge test --quality` 报告覆盖率下降 2%，团队会在 sprint 内修复，而非在半年后发现 80% 掉到了 35%

---

## 方向三：韧性验证框架（混沌工程 for ForgeOS 引擎）

**类型**: 测试基础设施 · 可靠性验证  
**优先级**: P1（可靠性的「可证明性」缺口）  
**差异化证明**: 已有分析中「韧性运行时」（Sprint 5 方向一）、「真长跑韧性」（Sprint 20-26）、「边界场景与性能」（`edgecases-and-perf.md`）全部关注 **实现韧性机制**（retry/backoff/checkpoint/process group/budget guard）。没有任何方向关注 **验证韧性机制的框架**——即系统的方法论来证明「当 X 失败时，系统确实做了 Y 的响应」。

### 现状：代码级证据

ForgeOS 有**韧性机制**但没有**韧性验证**。

**证据 A：存在多种失败恢复路径，但仅通过 happy-path 单元测试验证**

```go
// 存在以下韧性机制：
// - command_executor.go: classifyRunErr → KindOverloaded retryable → backoff
// - command_executor_unix.go: Setpgid → 进程组 SIGTERM → SIGKILL
// - checkpoint.go: persist.Save → atomic rename → crash-safe
// - budget.go: checkAgentBudget → fail-closed abort
// - exec_error.go: ExecError.Retryable → 重试 vs abort 决策

// 但所有测试都是：
// - TestClassifyRunErr_*（用预设错误代码，非真实子进程）
// - TestSaveCheckpoint_*（正常 → rename，非模拟写失败）
// - TestBudgets_*（正常计数，非竞争条件/并发）
// ✓ 验证「机制存在」
// ✗ 验证「机制在真实失败条件下生效」
```

**证据 B：没有故障注入接口**

```go
// forge-core/internal/orchestrator/command_executor.go
type CommandExecutor struct {
    Dir       string
    MaxDepth  int
    MaxOutput int
    // ← 没有故障注入配置
    // ← 无法测试：文件系统满了怎么办？
    // ← 无法测试：子进程 SIGKILL 超时怎么办？
    // ← 无法测试：O_APPEND 写入 halfway 怎么办？
}

// 测试只能靠 mock AgentExecutor（dry-run executor）
// 而 dry-run executor 永远不会失败——所以韧性代码实际上从未被真实执行过
```

**证据 C：`forge evolve` 韧性路径的测试覆盖率是乐观主义**

```go
// forge-core/internal/orchestrator/loop.go — 最复杂的韧性逻辑
// - MaxIter → stop safety bound
// - NoProgress → tripwire
// - checkpoint/resume → 恢复路径
// - budget → fail-closed

// 测试文件 loop_test.go:
// - TestLoop_Basic（happy path）
// - TestLoop_ConvergeCheck（收敛判定）
// - TestLoop_NoProgress（超时触发）
// ✓ 这些都是重要的测试
// ✗ 没有：
//   - 测试中途进程崩溃后 resume 的恢复路径
//   - 测试 checkpoint 文件损坏时的降级行为
//   - 测试 budget 耗尽和递归 guard 同时触发时的优先级
//   - 测试 memory.jsonl 损坏时 evolve 的行为
```

**证据 D：无系统性韧性验证——只能靠真 claude 跑**

```
当前韧性验证方法：
1. 单元测试（mock 失败 → 验证分类逻辑）
2. 集成测试（dry-run executor → 从不真实失败）
3. 真 claude 跑（Sprint 24-26 验证了 8 个真 gap）
   └─ 每次验证成本 ~$0.18/phase，且不可重复

缺少：
4. 故障注入（让 dry-run executor 在指定阶段失败）
5. 混沌测试（随机注入 IO/网络/进程失败）
6. 恢复测试（模拟 crash → resume → 验证状态一致性）
```

### 需要什么

**故障注入接口（~0.5 sprint）**

```go
// 在 CommandExecutor 中增加故障注入支持
type CommandExecutor struct {
    // ... 现有字段 ...

    // FaultConfig 控制模拟故障的注入，仅在测试中使用。
    // 生产代码中 FaultConfig 应始终为零值（即无故障注入）。
    FaultConfig *FaultConfig  // nil = 生产模式（无故障）
}

type FaultConfig struct {
    // FailOnExec 控制在第 N 次 Execute 时返回指定错误
    FailOnExec []FailSpec

    // FailOnSignal 控制在进程组信号发送时失败
    FailOnSignal bool

    // CorruptWrite 控制在写 checkpoint/memory/trace 时注入损坏
    CorruptWrite float64 // 概率 [0,1)
}
```

**韧性场景目录（~0.5 sprint）**

| 故障场景 | 现有机制 | 当前测试覆盖 | 需要验证 |
|---------|---------|-------------|---------|
| 529/过载 | retry + backoff | `TestClassifyRunErr` 验证分类 | 验证退避序列、重试上限耗尽后的降级 |
| 子进程超时 | context cancellation | `TestExecutor_Timeout` 验证取消 | 验证超时后资源清理、process group 回收 |
| 文件系统满 | checkpoint atomic rename | TestSave 验证正常保存 | 验证 rename 失败后原地保留旧文件 |
| Trace 写入失败 | fail-closed → error | 无 | 验证 trace 失败不阻止 gate 继续 |
| checkpoint 损坏 | resume 失败 | 无 | 验证从上一迭代恢复而非整体重启 |
| memory 损坏 | Load 返回 error | 无 | 验证清空 memory 继续而非 abort |
| 并发 evolve | 文件锁不存在 | 无 | 验证第二个进程检测到锁后退出 |
| 子进程泄露 | process group SIGKILL | `TestProcessGroup` 验证组信号 | 验证僵尸进程被回收，不残留 |

**混沌模式（~1 sprint）**

`forge run --chaos`：对所有 I/O 操作（文件写、子进程 spawn、子进程通信）以可配置概率注入随机故障，验证：

```
forge run build --executor dry --chaos
  → chaos: I/O failure probability=0.05
  → checkpoint save FAILED (injected) — fallback to prior state  ✓
  → trace emit FAILED (injected) — continue without trace       ✓
  → gate shell FAILED (injected) — retry 1/3                    ✓
  → gate shell FAILED (injected) — retry 2/3                    ✓
  → gate shell FAILED (injected) — retry 3/3 exhausted, abort   ✓
  → Final: ABORTED (as expected: all retries consumed)

forge run evolve --executor dry --chaos --chaos-corrupt-prob=0.1
  → memory append partially corrupted — skip bad line, log warning ✓
  → checkpoint seq out of sync — correct from trace seq           ✓
  → run: RECOVERED (graceful degradation path exercised)          ✓
```

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 故障注入代码进入生产构建 | 安全漏洞 | 用 build tag `//go:build chaos` 条件编译 |
| 混沌模式造成真实破坏 | 损坏工作目录 | 默认只在 `--executor dry` 下启用，只在 `.forge/` 操作上注入故障 |
| 高故障率导致测试不稳定 | CI 随机失败 | 用固定种子（`--chaos-seed=42`）保证确定性 |
| 假阳性（注入的故障被正常韧性处理了） | 测试误认为通过 | 框架必须断言「故障被注入且被正确处理」，而非「测试跑完无异常」 |

### 价值

1. **可靠性可证明**: 当前只能「证明机制存在」，不能「证明机制在故障下生效」。混沌测试填补了这个证据缺口
2. **降低真跑成本**: 每次用真 claude 验证韧性花费 ~$0.18/phase。混沌框架可以在 dry-run 下批量验证
3. **回归防护**: 引入新代码时不破坏既有韧性（混沌测试作为 CI 步骤）
4. **文档即测试**: 混沌场景大纲就是系统故障模式的官方文档

---

## 方向四：产物质量治理（从过程执法到产出执法）

**类型**: 治理 · 质量保障  
**优先级**: P1（治理系统的产物质量盲区）  
**差异化证明**: 70+ 已有方向全部关注**过程治理**——代码行数、架构层次、secret 扫描、测试绿/红。没有任何方向关注**产出治理**——ForgeOS 自身生成的文件（PRD、ADR、架构文档、ROADMAP 更新）的结构完整性、内容质量和格式合规。

### 现状：代码级证据

**证据 A：`emits:` 声明的产物文件有无不被治理**

```yaml
# .agent/workflows/discover.yml
phases:
  - name: requirement-discovery
    emits:
      - docs/discovery/prd.md
      - docs/discovery/market-analysis.md
    # ...
  - name: market-research
    emits:
      - docs/discovery/prd.md  # ← 故意写同个文件（增量更新）
      - docs/discovery/capability-matrix.md
```

```yaml
# .agent/workflows/design.yml
phases:
  - name: proposal-generation
    emits:
      - docs/design/proposal.md
      - docs/adr/ADR-0004-review-stage.md  # ← ADR 产物声明
```

**emits 声明了「应该产出什么」，但没有任何机制验证：**
- `docs/discovery/prd.md` 是否实际存在
- 产出的 PRD 是否包含必需章节（背景分析、需求列表、置信度）
- 产出的 ADR 是否符合 ADR 模板（标题、状态、Context、Decision、Consequences）
- `emits:` 声明与实际产出的文件集合是否有漂移

**证据 B：ADR 没有结构验证**

```markdown
# docs/adr/ADR-0004-review-stage.md

## Status
Proposed

## Context
...

## Decision
...

## Consequences
...
```

`.agent/agents/architect.md` 声明了 ADR 必须包含 5 个标准段（Status/Context/Decision/Consequences/Compliance），但 `forge validate` 不做任何结构检查：

```bash
# forge validate 检查的是：
#   - agent 引用是否有效
#   - workflow 引用是否有效
#   - 模型路由档位是否有效
# 但不检查：
#   - ADR 是否包含 5 个标准段
#   - PRD 是否包含 required sections
#   - emits 声明的文件是否实际存在
```

**证据 C：生成文件不受治理 gate 覆盖**

```
# forge accept 检查：
#   测试绿 → ✓
#   arch-check → ✓
#   secret-scan → ✓
#   治理完整性 → ✓
# 但不检查：
#   docs/discovery/prd.md 是否包含必选章节 → N/A
#   docs/design/proposal.md 是否有明显矛盾 → N/A
#   docs/adr/ADR-NNNN.md 是否符合模板 → N/A
```

这意味着：一个 agent 可以产出一份**格式错误、内容不完整、违反 ADR 模板的文档**，而 `forge accept` 仍然报告 ACCEPTED——只要代码和治理结构合格。

**证据 D：无跨产物一致性检查**

```markdown
# discover.yml 产生的 PRD 说「用 PostgreSQL」
# design.yml 产生的 ADR 说「用 SQLite」
# → 无人发现不一致，因为两篇文档都被 emit 后无人交叉校验
```

### 需要什么

**第一层：产物存在性验证（~0.3 sprint）**

`forge validate --emits`：工作流运行后，验证 `emits:` 声明的每个文件路径是否存在。

```
$ forge run discover --executor dry
...
$ forge validate --emits .agent/workflows/discover.yml
  ✓ docs/discovery/prd.md (256 bytes)
  ✓ docs/discovery/market-analysis.md (1280 bytes)
  ✗ docs/discovery/capability-matrix.md — NOT FOUND (declared but not emitted)
```

**第二层：产物结构验证契约（~1 sprint）**

每类产物声明预期的结构契约：

```yaml
# .agent/policies/artifact-contracts.yml (新文件)
artifact_contracts:
  adr:
    required_sections: [Status, Context, Decision, Consequences, Compliance]
    section_header_pattern: "^## \\w+$"
    max_title_length: 80
    template: docs/adr/ADT.md  # 参考模板路径

  prd:
    required_sections: [背景分析, 目标用户, 功能需求列表, 置信度评估, 排除范围]
    section_header_pattern: "^## "
    min_section_length: 50  # 每段至少 50 字符

  proposal:
    required_sections: [方案, 成本估算, 风险评估, 时间线]
    max_total_length: 5000  # 一页方案
```

验证命令：

```
$ forge validate --artifacts
  docs/adr/ADR-0004-review-stage.md:
    ✓ 5 required sections present
    ✓ Section headers match pattern
    ✓ Title length OK (72 ≤ 80)
  docs/discovery/prd.md:
    ✗ Missing section: 「排除范围」
    ✗ 「置信度评估」section too short (18 < 50 chars)
```

**第三层：跨产物一致性守卫（~1.5 sprint）**

当多个工作流文件提及同一概念时，检查一致性：

```
# discover.yml 的 PRD → "database: PostgreSQL"
# design.yml 的 ADR → "database: SQLite"
# → forge validate --consistency 报告矛盾

# 实现：从产物中提取 key decision markers
# （简单的 key=value 模式匹配，非语义理解）
# → "database: PostgreSQL" vs "database: SQLite" → WARN
```

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 产物是迭代式完成的（第一次 emit 只有骨架） | 过早的完整性检查导致 WARN | 只在 stop_condition MET 后做全量检查 |
| 产物格式自由（如 PRD 没有标准模板） | 无法自动验证 | `artifact_contracts` 是可选的扩展配置，不配则不检查 |
| 跨产物检查误报（PostgreSQL 是 v1、SQLite 是 v2） | 合法倒退被标记为矛盾 | 只检查同一 version/stage 下的产物 |
| ADR 模板自定义（团队有自己的 ADR 格式） | 硬编码检查不适用 | 模板在 `.agent/policies/` 中配置，非硬编码 |

### 价值

1. **完整治理闭环**: 过程治理保证代码质量，产出治理保证文档质量。两者缺一都不是完整的治理
2. **跨 session 一致性**: 没有产出治理，evolve 循环可能生成与前期 ADR 矛盾的文档而不自知
3. **自治可信度**: 用户可以信任 `forge run discover` 产出的 PRD 包含所有必需信息——不需要每次手动打开检查
4. **为 ADR 自动执法铺路**: 产出治理是 Sprint 31 的 `writes_adr` 从 narration 走向真实 ADR 编写的必备基础设施

---

## 方向五：配置 Schema 版本化与迁移管线

**类型**: 架构 · 可演化性  
**优先级**: P1（治理资产的长期可持续性）  
**差异化证明**: `expansion-horizon-three.md` 方向四覆盖了「治理资产生命周期管理」——即从 forge-core 新版本向已 fork 项目推送新 agent 卡/new harness 工具。但**没有任何已有方向覆盖配置 Schema 本身的版本化**——即 `.agent/` 下文件的格式/结构变更的管理。这是两个不同维度：前者是**内容升级**（加一个新规则），后者是**格式升级**（改 schema 结构）。

### 现状：代码级证据

**证据 A：所有 Schema 文件无版本标识**

```yaml
# .agent/project.yml — 无 version 字段
mode: engineering
lifecycle: mvp
overrides: {}

# 如果 2.0 版 forge-core 改了 project.yml 的 schema，
# 怎么知道这个文件是 1.0 还是 2.0 格式？
```

```yaml
# .agent/policies/modes.yml — 无 schema version
modes:
  explorer:
    router_tier: haiku
    gates: [size, complexity, basic]
    enforce: warn

# 3.0 版可能改成：
# modes:
#   explorer:
#     router: { tier: haiku, fallback: none }
#     gates: { required: [size, complexity], optional: [basic] }
#     enforcement:
#       level: warn
#       exceptions: []
#
# 没有版本号，解析器无法区分新旧格式
```

```yaml
# .agent/policies/routing.yml — 无版本
# 如果 schema 从 flat map 改为嵌套结构，
# 现有 3 个项目各自使用旧格式，无法自动迁移
```

**证据 B：`yaml2json` 解析器是无版本的——不能处理多版本**

```go
// forge-core/internal/yaml2json/yaml2json.go:55-66
func Decode(r io.Reader) (any, error) {
    data, _ := io.ReadAll(r)
    lines := normalizeLines(string(data))
    val, _, err := parseDocument(lines, 0)
    return val, err
    // ← 总是用同一套解析规则解析所有文件
    // ← 无法处理 schema 版本变化
    // ← 不检查 format_version 字段
}
```

**证据 C：`asset.Phase` 同样的解析结构——无版本兼容**

```go
// forge-core/internal/asset/asset.go
// Workflow struct 的字段是固定的
// 如果 v2 版的 workflow 新增 required field，
// 旧项目直接 decode 会得到零值——静默出错
```

**证据 D：`modes.yml` 的 `version:` 字段存在但不被使用**

```yaml
# .agent/project.yml
# 无 version 字段
# 但 project.yml 是可以加 version 字段的——migrate 可能写新值
# 但没有任何代码读这个 version 字段
```

### 为什么需要 Schema 版本化

ForgeOS 治理资产的演化路径（已发生和可预见的）：

| 版本 | project.yml | modes.yml | workflow.yml | harness 工具 |
|------|------------|-----------|-------------|-------------|
| v1 (2026-05) | mode + lifecycle | router_tier/gates/enforce | on_fail/required_gates | gate.mjs + check.py |
| v2 (2026-06) | + overrides | + workflow_depth/review | + model_tier/feeds_forward/depends_on | + arch-check/secret-scan/sca |
| v3 (预计) | + forge_version | + coverage_threshold | + requires_tools/secondary_template | + test-quality/artifact-contracts |

**问题：v2 工作流文件被 v1 解析器读取会怎样？**

```yaml
# v2 格式（新增了 depends_on 字段）
phases:
  - name: planner
    agent: planner
    depends_on: []  # ← v1 解析器不认识这个字段
```

```go
// v1 asset.go — 用 encoding/json 解析
type Phase struct {
    Name     string   `json:"name"`
    Agent    string   `json:"agent"`
    // ← 没有 DependsOn 字段
    // ← Go 的 encoding/json 静默跳过未知字段
    // ← 所以 depends_on 被丢弃，无警告
}
```

没有 version 字段，解析器无法区分「这是旧格式的正常数据」和「这是新格式的字段被静默丢弃」。

### 需要什么

**第一层：Schema Version Stamp（~0.3 sprint）**

所有核心配置文件增加 `forge_schema_version` 字段：

```yaml
# .agent/project.yml
forge_schema_version: 1    # ← 新增
mode: engineering
lifecycle: mvp
```

```yaml
# .agent/policies/modes.yml
forge_schema_version: 2    # ← 新增（modes 在 v2 改了 schema）
modes:
  ...
```

`forge validate` 检查 schema 版本兼容性：

```
$ forge validate --schema
  .agent/project.yml:        schema_version=1  ✓ (current: 1)
  .agent/policies/modes.yml: schema_version=2  ✓ (current: 2)
  .agent/workflows/build.yml: schema missing   ⚠ (default schema_version=1)
```

**第二层：向后兼容解析器（~1 sprint）**

```go
// internal/schema/registry.go（新包）
type SchemaVersion int
const (
    V1 SchemaVersion = 1
    V2 SchemaVersion = 2
)

// DecodeV1 → 只解析 V1 已知字段，忽略 V2 新字段
// DecodeV2 → 解析 V1+V2 字段，V1 未知字段容错
// DecodeWithVersion → 读 forge_schema_version，分派对应解析器
```

**第三层：自动迁移管线（~1.5 sprint）**

```
forge migrate --schema           # 列出需要 schema 升级的文件
forge migrate --schema --apply   # 自动执行升级

# 迁移：
# project.yml v1 → v2:
#   - 加 forge_schema_version: 2
#   - overrides 段从可选变为标准段
#   - 旧文件缺少 overrides → 补 {}
```

### 边界情况

| 场景 | 风险 | 处理建议 |
|------|------|----------|
| 用户自定义了 `.agent/` 中未声明的字段 | 迁移可能删除自定义内容 | 迁移保留未知字段，仅处理已知字段的转换 |
| 不同文件使用不同 schema 版本 | 解析器需同时处理多版本 | 解析器按文件粒度分派版本，不要求全局一致 |
| 降级迁移（schema v2 → v1） | 可能丢失信息 | 只提供 forward migration，不提供 backward |
| 分布式场景下多个 forge-core 版本共存 | 服务间通信的版本协商 | 序列化协议（trace/checkpoint/memory）也要版本化 |

### 价值

1. **长期演化的地基**: 没有 Schema 版本化，每次格式变更都是一个无法自动迁移的断崖——要么 fork 解析器，要么强制所有项目升级
2. **治理资产的可移植性**: 一个有 `forge_schema_version` 的 `.agent/` 目录可以被任意版本的 forge-core 读取（旧版报告"不兼容"而非"静默丢弃字段"）
3. **多版本共存**: CI pipeline 可能运行不同版本的 forge-core，Schema 版本化让旧版工具安全地读取新版配置文件
4. **安全网**: 防止 `encoding/json` 的静默字段丢弃变成隐蔽的配置丢失

---

## 汇总

| # | 方向 | 核心价值 | 与 85+ 已有方向的区别 | 实现量级 |
|---|------|---------|---------------------|---------|
| 1 | **Go 库 API 边界契约** | north-star 分布式化的结构性前提——将内部包变为可导入库 | 70+ 方向全部将 forge-core 视为 CLI 或内部包集合，零方向讨论库 API 契约 | 中（pkg/ 组织 + 接口化 + godoc） |
| 2 | **测试质量元治理** | 治理系统的自反完整性——守卫守卫者 | 10+ 方向讨论执法/测试/自检，零方向讨论测试代码自身的质量门控 | 中（新 `forge test --quality` + policies 扩展） |
| 3 | **韧性验证框架** | 可靠性的可证明性——从「机制存在」到「机制在故障下生效」 | 5+ 方向讨论韧性机制（retry/backoff/checkpoint），零方向讨论韧性验证方法论 | 中（故障注入接口 + 混沌模式） |
| 4 | **产物质量治理** | 产出完整性——从过程执法到产物执法 | 全部方向关注过程治理（代码/架构/secret），零方向关注产物治理（PRD/ADR/设计文档的结构完整性） | 中（artifact-contracts.yml + 结构验证） |
| 5 | **配置 Schema 版本化** | 长期可演化性——让 `.agent/` 配置格式变更不再是断崖 | `horizon-three` 方向四覆盖治理资产内容升级，零方向覆盖配置 Schema 本身的版本化和迁移 | 中（schema registry + 解析器分派 + migrate） |

### 推荐实施顺序

1. **方向一（库 API 边界契约）** — 最基础。没有稳定的 API，后续的方向四（产物治理）和方向五（Schema 版本化）都无法做成独立的可消费模块。建议先从 `pkg/routing` 和 `pkg/converge` 开始试点导出
2. **方向四（产物质量治理）** — 最易落地。`emits:` 验证可以在 0.3 sprint 内实现，立即提升治理完整性
3. **方向五（Schema 版本化）** — 紧迫性随时间指数增长。每新增一个配置格式，就多一份「无版本」的遗留债务。建议在下一次配置格式变更前完成
4. **方向三（韧性验证框架）** — 虽重要但需要方向一完成后的可测试接口才能优雅实现
5. **方向二（测试质量元治理）** — 最深的投资。建议先做可观测层（`forge test --quality` 报告），等数据积累后再加 blocking 守卫

---

## 排除的方向（已有分析已充分覆盖）

| 方向 | 覆盖文档 | 为何排除 |
|------|---------|---------|
| 跨进程运行时守护（文件锁/PID 文件） | `forgotten-five-foundations.md` 方向一 | 已充分展开 |
| 记忆内容去重 | `novel-architectural-extensions-v40.md` 方向二 | 已充分展开 |
| 墙钟预算 | `novel-architectural-extensions-v40.md` 方向三 | 已充分展开 |
| 编排器通用 Hook 契约 | `novel-architectural-extensions-v40.md` 方向五 | 已充分展开 |
| 跨仓库联邦治理 | `expansion-horizon-three.md` 方向二 | 已充分展开 |
| 外部事件驱动触发器 | `expansion-horizon-three.md` 方向三 | 已充分展开 |
| 跨会话修正学习 | `expansion-horizon-three.md` 方向五 | 已充分展开 |
| Gate 执行经济学（缓存/并行） | `novel-architectural-extensions-v40.md` 方向一 | 已充分展开 |
| 执行计划预览 `forge plan` | `novel-architectural-extensions-v40.md` 方向四 | 已充分展开 |
| 治理策略测试框架 | `novel-five-highvalue-extensions.md` 方向一 | 已充分展开 |
| 多项目联邦/组织级治理 | `architectural-extensions-v38.md` 方向一 | 已充分展开 |
| 跨相位产物契约校验 | `fresh-expansion-perspectives.md` 方向三 | 已充分展开 |
