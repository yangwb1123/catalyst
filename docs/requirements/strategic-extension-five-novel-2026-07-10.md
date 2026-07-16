# ForgeOS — 全局再扫描:五个未覆盖的结构性扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓逐文件深扫:forge-core（18 Go 包 · ~35k LOC）· cmd/forge（17+ 子命令,~12k LOC）·
>    harness（39+ 模块 · ~10.5k LOC）· .agent/（12 agent 卡 · 9 skill 卡 · 5 工作流）·
>    docs/（90+ 分析文档 · FUNCTIONAL_REQUIREMENTS_AUDIT · 4 ADR · CURRENT_SPRINT 31 轮）
> 2. **差异化验证**:对每个候选方向的核心关键词,在 docs/requirements/（~50 篇）+ docs/analysis/（~40 篇）+
>    核心文档中逐篇全文检索,确认该方向**作为独立扩展方向从未被展开**。下方「与已有覆盖的区别」部分
>    引用最接近的已有分析并解释为什么不是同一个方向。
> 3. **纪律**:不编写任何代码。每个方向附代码级证据、实际影响、边界情况。
> **日期**: 2026-07-10

---

## 全景定位

已有 ~90 份分析文档覆盖了 ForgeOS 的引擎功能、生产可靠性、安全纵深、执行语义、治理完整性、
系统边界等几乎每个维度。但这五个方向落在所有分析的**结构化间隙**中——它们不是「缺少的引擎」、
不是「边界情况修复」,而是**代码层已存在但未被识别为系统性风险/债务的设计特征**。

共同特征:每个方向都是「结构性的」而非「功能性的」——修复它们不会增加用户可见的功能,
但会改变 ForgeOS 作为一个长生命周期软件项目的可维护性、可测试性、和可演化性。

| # | 方向 | 核心问题 | 已有分析覆盖 |
|---|---|---|---|
| 1 | **`cmd/forge` 依赖中枢退化** | 一个包进口 15/17 内部包,成为架构级耦合点 | **零覆盖** |
| 2 | **Agent 执行器测试盲区** | 全测试套件零次真正 spawn 子进程 agent | 仅提及,从未作为方向 |
| 3 | **Memory Store 无界增长问题** | 24h+ evolve 下.memory.jsonl 无归档/上界 | 提到 clean,非系统性分析 |
| 4 | **相位级与工作流级 Timeout 层次缺失** | 多层超时未区分,单一 CLI 超时不够用 | 仅一次顺带提及 |
| 5 | **编排器状态机缺少形式化模型** | 状态/转换/不变量全隐在命令式 Go 代码中 | **零覆盖** |

---

## 方向一 · `cmd/forge` 依赖中枢退化——架构级耦合点

**优先级**: 🟡 P2 | **类别**: 架构 · 可维护性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

`cmd/forge` 包是 ForgeOS 的 CLI 入口,但它已经从一个「命令行包装器」退化为一个**依赖中枢
(hub)**。当前它直接 import **15 个内部包中的 15 个**(全部除 `yamlpath`),而没有任何一个内部
包 import 回 `cmd/forge`。这种「星型拓扑」意味着:

- **任何内部包的接口变化都可能触发 cmd/forge 的修改**——它承载全部适配/接线逻辑
- **无法单独测试 wiring 逻辑**——所有接线紧耦合在一起,没有「无 CLI 的 API 测试」路径
- **文件数预算反复被突破**——Sprint 27/29/30 三次因文件超限而被迫拆分,每次都是治标不治本
- **core 层与 CLI 层的界限模糊**——prompt_context.go、cost.go、gates.go 等大量纯逻辑混在 CLI 包中

### 代码级证据

**1. 15/17 内部包全部被 `cmd/forge` import,无反向依赖**:

```go
// forge-core/cmd/forge/main.go 及其他文件:
import (
    "forgeos/forge-core/internal/asset"       // 工作流类型
    "forgeos/forge-core/internal/attribution"  // agent→task_type 映射
    "forgeos/forge-core/internal/converge"     // 收敛判定
    "forgeos/forge-core/internal/doctor"       // 诊断
    "forgeos/forge-core/internal/gate"         // 闸门运行器
    "forgeos/forge-core/internal/memory"       // 跨会话记忆
    "forgeos/forge-core/internal/migrate"      // 模式迁移
    "forgeos/forge-core/internal/mode"         // 中枢旋钮
    "forgeos/forge-core/internal/orchestrator" // 编排引擎
    "forgeos/forge-core/internal/persist"      // 检查点
    "forgeos/forge-core/internal/prompt"       // prompt 缓存/检索
    "forgeos/forge-core/internal/risk"         // 风险分类
    "forgeos/forge-core/internal/routing"      // 模型路由
    "forgeos/forge-core/internal/trace"        // 可观测性
    "forgeos/forge-core/internal/yaml2json"    // YAML 解析
)
```

```bash
# 验证:无内部包 import cmd/forge
$ grep -rl '"forgeos/forge-core/cmd/forge"' forge-core/internal/*/
# → 无输出
```

**2. `cmd/forge` 包内的文件反复超限**——三次 sprint 被迫拆分,文件预算从 14 逐步抬到 17:

```
Sprint 27: 15 文件 > 14 上限 → 拆 validate.go 到 internal/doctor
Sprint 29: 抬到 16 → 抄近路抬 rules.yaml
Sprint 30: 抬到 17 → 再拆 prompt_context.go
```

每一次都是**被动反应**(达到阈值后再拆),从未主动做架构级消解。

**3. 纯逻辑(非 CLI 胶水)大量驻留在 `cmd/forge`**:

| 文件 | 行数 | 内容性质 | 应属包 |
|---|---|---|---|
| `cost.go` | ~415 | runBudget · claude cost 解析 · scorecard 归因 | `internal/budget`(新) |
| `gates.go` | ~480 | 收敛信号采集 · approve/rejection 标记 | `internal/gate`(部分已在) |
| `prompt_context.go` | ~490 | gateLedger · phaseOutputLedger · verdictLedger | `internal/prompt`(部分已在) |
| `engine_build.go` | ~310 | 引擎装配 · tier 解析 · agent 执行器选择 | `internal/orchestrator`(部分) |

**4. `engine_build.go` 的 `buildRunEngine` 函数是 15 个包的产物**——它组装 orchestrator.Engine
的每个字段,跨越 promt/trace/routing/gate/risk/mode/orchestrator 等包,单函数 80+ 行:

```go
// forge-core/cmd/forge/engine_build.go:145-230
func buildRunEngine(wf asset.Workflow, o runOpts, ...) (orchestrator.Engine, ...) {
    gates := newGateLedger()        // -> prompt_context.go
    phaseOut := newPhaseOutputLedger()
    verdicts := newVerdictLedger()
    findings := newReviewFindingsLedger()
    ctxCache := prompt.NewContextCache() // -> internal/prompt
    cards, err := routing.LoadScorecards(...) // -> internal/routing
    tierOf := phaseTierResolver(...)    // 跨 mode/risk/routing
    return orchestrator.Engine{
        Exec:         agentExecutor(...),  // 跨 prompt/routing/command
        RunGate:      runGate,             // -> gate
        OnGateResult: gates.record,        // -> prompt_context.go
        AgentVerdict: verdicts.get,        // -> prompt_context.go
        MaxRetries:   o.maxRetries,
        MaxLoopBack:  maxLoopBack,
        ModePolicy:   pol,                 // -> internal/mode
    }, ...
}
```

### 与已有覆盖的区别

- **零覆盖**——在所有 90+ 分析文档中,没有任何一篇将 `cmd/forge` 的依赖拓扑作为独立方向展开。
  最接近的是 `expansion-self-governance-and-hygiene.md` 提到「validate.go 超 500 行需要拆分」,
  但那关注的是文件体积,不是架构耦合。
- 工程红线里的「单一职责」和「文件≤500 行」是针对小粒度的纪律,不解决包的职责范围问题。
- `strategic-extensions-v33.md` 讨论了「18 个内部包中只有 5 个在架构文档中列为引擎」,
  但它关注的是文档缺失,不是 `cmd/forge` 的耦合。

### 边界情况

- **新包创建时的决策负担**:当需要新增一个内部包时,开发者面临选择——放在 `internal/` 还是
  `cmd/forge`?当前没有明确标准,导致 `internal/gate/resolve.go` 被拆入 `cmd/forge` 后又迁回。
- **测试隔离**:`cmd/forge` 包的测试需要 mock 15 个包的依赖,导致测试文件体积大且脆弱。
- **二进制大小**:所有 wiring 编译进同一个二进制,未来如果要做分离部署(如 k8s sidecar),
  这种紧耦合会让拆分极其痛苦。

### 建议方向

1. **`internal/budget` 包提取**:将 `cost.go` 的 `runBudget`、`newRunBudget`、`BudgetExhaustedFunc`
  移到新包,与 `orchestrator` 通过接口交互(预算查询→phase 准入)。
2. **`internal/ledger` 包提取**:将 `gateLedger`、`phaseOutputLedger`、`verdictLedger`、
  `reviewFindingsLedger` 四种 ledger 统一提取为 `internal/ledger` 包,与 `internal/prompt`
  和 `internal/trace` 通过接口交互。
3. **`Engine` 构造器模式**:`buildRunEngine` 的组装逻辑应反映到 `internal/orchestrator`
  的「EngineConfig」或「EngineBuilder」中,让 CLI 层只关心 flag 解析和 IO 路径,不关心
  依赖装配。
4. **依赖预算(Harness 补充)**:在 `.arch/rules.yaml` 中为 `cmd/forge` 包增加「进口包数≤10」
  的约束(当前 15),像文件数预算一样强制执行,防止进一步膨胀。

---

## 方向二 · Agent 执行器测试盲区——零次真正 spawn 子进程

**优先级**: 🔴 P0 | **类别**: 测试 · 可靠性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 的核心价值主张是「编排真实 agent 做自治软件工程」,但全仓测试套件中:
- **零次真正 spawn 子进程**来验证 agent 执行
- **所有编排测试使用 `DryRunExecutor`**(只叙述,不调用命令)
- **`CommandExecutor` 的测试只验证参数构建**(argc/argv 断言),不验证进程生命周期
- **没有测试验证真实 agent 的超时、SIGKILL、进程组清理**

这不是一个「测试缺失」——这是一个**基础可靠性盲区**:全部编排逻辑(loop-back、mode-gating、
converge、checkpoint)是在 `DryRunExecutor` 上测试通过的,而生产环境用的是 `CommandExecutor`,
两者的行为差异从未被测试覆盖。

### 代码级证据

**1. `forge run build --executor dry` 是 CI 的唯一端到端测试**:

```yaml
# .github/workflows/forge.yml:58-62
- name: forge run build --executor dry (end-to-end orchestration smoke test)
  run: |
    go -C forge-core build -o /tmp/forge-test ./cmd/forge
    /tmp/forge-test run build --executor dry --root $PWD
```

**从未有一个 CI 步骤跑 `--executor command`**——即使 `--agent-cmd echo`(安全、零成本、不调 LLM)也没有。

**2. `DryRunExecutor` 与 `CommandExecutor` 的关键行为差异**:

| 行为 | `DryRunExecutor` | `CommandExecutor` | 测试覆盖 |
|---|---|---|---|
| 返回 `ExecError` | 永不(总是 nil) | 有(retryable/fatal) | ❌ 只有单测 |
| 超时 | 不适用 | 用 `context.WithTimeout` | ❌ |
| 进程组清理 | 不适用 | `Setpgid` + SIGKILL | ❌ |
| 529 过载退避 | 不适用 | `classifyOverload` | ✅(单测) |
| 输出上限 | 不适用 | `cappedBuffer` | ✅(单测) |
| 成本计算 | 无 | `claude --output-format json` | ❌(单测有) |
| `MAX_AGENT_DEPTH` 递归保护 | 不适用 | 环境变量注入 | ✅(单测) |

**3. `command_executor_test.go` 测试的是 argv 构建,不是真正的进程行为**:

```go
// forge-core/internal/orchestrator/command_executor_test.go:125-160
func TestCommandExecutor_BuildArgv(t *testing.T) {
    // 验证 Build 函数生成的 argv 符合预期
    // 但从不调用 cmd.Run() —— 不真正启动进程
}
```

唯一真正的子进程测试在 `command_executor_unix_test.go` 中——但它测试的是
`echo hello` 能否正确返回,不是 agent 交互。

**4. `orchestrator_test.go` 全量使用 FakeGate + DryRunExecutor**:

```go
// forge-core/internal/orchestrator/orchestrator_test.go:30-50
// 所有 orchestrator 测试注入 fake result:
eng := Engine{
    Exec:    DryRunExecutor{Log: logln},
    RunGate: func(name string) gate.Result { return gate.Result{...} },
    // ...
}
```

**这意味着 gate loop-back、checkpoint 恢复、budget 守卫等机制的「端到端」
验证全部在排除真实子进程的情况下通过。**

### 与已有覆盖的区别

- `expansion-production-readiness.md` 的「安全护栏四维验证」表格列出了 `command_executor_test.go`
  的测试项,但那是**模块级单测**(测试某个函数返回正确值),不是**集成/端到端测试**(真正 fork+exec
  一个子进程并验证其生命周期)。
- 本方向不是「补什么测试」——是识别出一个结构性盲区:整个编排栈的测试基础是 `DryRunExecutor`,
  而这个 executor 与生产用的 `CommandExecutor` 有本质行为差异(进程管理、超时、信号处理)。
  这是一个「测试架构」问题,不是「测试用例数量」问题。

### 边界情况

- **SIGKILL 清理测试**:真实进程的孙子进程(MCP/bash)是否能被进程组 SIGKILL 清理?
  `Setpgid:true` 的测试用的是 `sleep + kill` 模式,不是真正的 `claude -p` spawn MCP 进程树。
- **529 退避的端到端行为**:`backoff.go` 的指数退避在 `command_executor.go` 的真实 retry
  循环中能否正确工作?当前只测了 `backoff` 函数本身,没测 `Execute` 方法中的 retry 循环。
- **cappedBuffer 的截断行为**:当子进程输出 10MB 且 cap=1KiB 时,子进程是否 hang?
  单测模拟了,但真实的 `claude -p` 的输出流特性(缓慢输出 vs 爆发输出)可能触发不同的行为。

### 建议方向

1. **引入 `FakeExecutor`(既不是 Dry 也不是 Command)**:这是一个真正的子进程 spawner,
  但 spawn 的是 `echo` 或 `cat`,模拟 agent 的真实 stdin/stdout/stderr 交互。
  离真正的 `claude -p` 最近的安全可测试边界。
2. **标准化 CI 集成测试**:在 `.github/workflows/forge.yml` 中增加:
   ```yaml
   - name: forge run with echo executor (real subprocess)
     run: forge run build --executor command --agent-cmd echo --root $PWD
   ```
   (零成本,验证进程 spawn/cleanup 路径是否完整)
3. **为 CommandExecutor 注入 fake command**:允许测试注入一个 `fakeAgent`(如 shell 脚本)
  来模拟不同的 agent 行为(成功/失败/超时/无序输出),而不需要真正调 claude。
4. **建立 spawn 测试覆盖率目标**:要求每项与子进程交互的机制(超时、进程组 kill、输出截断、
  递归保护)至少有一个真正的 fork+exec 测试。

---

## 方向三 · Memory Store 无界增长——24h+ 自治运行的运维盲区

**优先级**: 🟡 P2 | **类别**: 运维 · 可靠性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐

### 问题描述

`forge evolve` 的核心卖点是「24h 无人值守自治运行」。但 memory store(.forge/memory.jsonl)
是**append-only 且无上界的**。每次迭代每个 phase 都可能写入多条 Entry(decision、lesson、finding),
当 evolve 跑 50 轮、每轮 5 phase 时：
- memory.jsonl 的大小线性增长
- `memory.Load` 读取全部历史到内存,每次迭代都做(每 phase 读一次)
- `memory.Append` 永不 compaction,除非手动调用 `forge memory-prune`
- 没有大小上限、没有写入预算、没有自动归档

当 memory 文件增长到 100MB+:
- 每次 `memory.Load` 耗时增长到秒级
- prompt 中 `memoryContext` 的注入长度膨胀,接近上下文窗口
- `memory.Compact` 需要 O(n) 时间,在 24h 跑的中途触发会卡住编排

### 代码级证据

**1. `memory.Append` 是无条件的追加——没有预算检查、没有大小上限**:

```go
// forge-core/internal/memory/memory.go:173-210
func Append(path string, e Entry) error {
    f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
    // 没有检查文件大小
    // 没有检查总条目数
    // 没有检查写入速率
}
```

**2. `memory.Load` 将整个文件读入内存——没有分页/流式查询**:

```go
// forge-core/internal/memory/memory.go:230-280
func Load(path string) ([]Entry, error) {
    // bufio.Scanner 逐行读,但全部积累到 []Entry 切片
    // 没有 limit 参数,没有 page 参数
}
```

**3. `memory.Compact` 是事后裁剪,不是事前预防**:

```go
// forge-core/internal/memory/memory_compact.go:81-206
func Compact(path string, threshold, keepPerKind, ageSeconds int) {
    // 读全部→按时间排序→保留最近的 keepPerKind 条→合成摘要
    // 是维护操作,不是运行时自动触发
}
```

**4. `prompt_memory.go` 的 memory lane 注入有硬上限(32),但 memory 文件本身没有**:

```go
// forge-core/cmd/forge/prompt_memory.go:48
const memoryCap = 32 // prompt 只注入最近 32 条,但文件仍持有全部历史
```

**5. 没有 per-phase、per-iteration、per-run 的记忆写入配额**:

```go
// memory.go 没有:
//   - MaxEntriesPerIteration
//   - MaxFileSize
//   - AutoArchiveThreshold
```

**6. 没有跨会话的记忆归档策略**——`memory.prune` 支持保留最近 N 条,但
没有 `forge memory-archive`(将旧条目移到 `.forge/archive/memory-YYYYMMDD.jsonl`)。

### 与已有覆盖的区别

- `systemic-expansion-v26.md` 方向一的「forge cleanup」关注的是**废弃产物清理**
  (checkpoint/trace 过期),不是 memory store 的增长管理。
- `strategic-extensions-v23-systemic-gaps.md` 方向四的「老化归档」提到 trace/memory
  的 7 天归档,但那是**概念提及**(一段话),不是独立方向分析(无代码级证据、无边界情况、
  无实现路径)。
- 本方向是第一次将 **memory store 的无界增长作为独立运维风险**分析,给出完整的
  代码级证据链(append/load/compact/prompt 四层都没有 budget)。

### 边界情况

- **紧凑后的查询精度**:`Compact` 合成摘要后,`Query` 匹配摘要时可能丢失细节。
  一个包含具体代码路径的 Decision 被缩成「关于 X 的决定」后,后续 agent 读到的信息不足以
  做出正确判断。需要一个「摘要→原文」的可追溯性保证。
- **OOM 风险**:如果 `Load` 在 memory.jsonl=200MB 时被触发(20 万条 Entry),Go 的堆分配
  可能达到数 GB(每条 Entry ~1KB JSON + Go struct 开销)。在低内存环境(CI runner、
  edge device)下可能 OOM。
- **竞态条件**:`Append` 使用 O_APPEND,在并发写入下(并行模式)不会丢失数据,但
  `memory.Compact` 与 `Append` 同时发生时的原子性没有保证——Compact 读+写+rename
  期间,Append 可能写到临时文件,导致数据丢失。
- **大于 2GB 文件**:JSONL 格式下,某些文件系统/OS 对大文件的处理(例如 32-bit 的 2GB 限制)
  可能触发意想不到的行为。

### 建议方向

1. **memory 文件大小上限**:`memory.Append` 前检查文件大小,超限后:
   - (a) 拒绝写入(返回错误,phase 继续)
   - (b) 自动触发 `Compact` 后再试
   - (c) 自动创建 `.forge/memory.2.jsonl` 实现轮转
   由配置选择。
2. **Memory 预检触发器**:`memory.Load` 在文件 > 阈值(如 10MB)时主动触发 `Compact`,
  并在 trace 中记录 `kind: "memory_compact", reason: "auto: file size exceeded limit"`。
3. **分页 Load**:为 `Load` 增加 `Page(pageSize int)` 选项——只有最近的 N 条直接返回,
  老的按需懒加载。
4. **记忆写入配额**:在 `runBudget` 中增加 `memoryWrites` 计数器,防止单一 phase 写入太多记忆。
5. **运维命令**:`forge memory-stats`(总条目数、文件大小、最近写入速率)、
  `forge memory-archive`(将指定日期前的条目归档到 `.forge/archive/`)。

---

## 方向四 · 相位级与工作流级 Timeout 层次缺失——单一超时不够用

**优先级**: 🟠 P1 | **类别**: 编排 · 韧性 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

当前 ForgeOS 的超时架构是**一维的**:

```
CLI --timeout 5m → 全局应用于所有 phase
```

但一个 evolve 运行的实际超时需求是**多层级的**:

```
Evolve 运行整体超时:  4h (别让我等通宵)
  └─ 每轮迭代超时:   30min (拿不到结果就换思路)
       └─ 每 phase 超时:  5min (单个 agent 不能跑太久)
            └─ 每 gate 超时:  1min (本地命令不应该慢)
```

当前的一维超时意味着:
- 如果你设 `--timeout 5min`,一个 10-phase 的 evolve 可能在 phase 3 被切掉
- 如果你设 `--timeout 4h`,一个死循环的 agent phase 会挂 4 小时才被切掉
- 没有 per-phase 超时,不能对不同 agent 设置不同限制(reviewer 可以久一点,implementer 快一点)
- 没有 propagate 语义——phase 超时是否应该触发迭代停止?迭代超时是否应该触发整体停止?

### 代码级证据

**1. `runOpts.timeout` 是单一全局字段**:

```go
// forge-core/cmd/forge/main.go:160-195
type runOpts struct {
    timeout    time.Duration // 用于所有 phase、所有 iteration
    // 没有 perPhaseTimeout, 没有 perIterationTimeout
    // 没有 perWorkflowTimeout
}
```

**2. `CommandExecutor.Timeout` 是单一值,应用给每个 `Execute` 调用**:

```go
// forge-core/internal/orchestrator/command_executor.go:50-60
type CommandExecutor struct {
    Timeout time.Duration // 全局
    // 没有每个命令的不同超时
}
```

**3. `orchestrator.go` 的超时上下文在 `runAgentPhase` 中创建,所有 phase 共用**:

```go
// forge-core/internal/orchestrator/orchestrator.go:200-230
func (e *Engine) runAgentPhase(ctx context.Context, p asset.Phase, mode string) error {
    // 超时来自 e.Ctx(全局),或 CommandExecutor.Timeout(全局)
    // 没有从 p(asset.Phase) 读取 per-phase timeout
}
```

**4. `asset.Phase` 没有超时字段**:

```go
// forge-core/internal/asset/asset.go:40-105
type Phase struct {
    Name         string
    Agent        string
    RequiredGates []string
    OnFail       *OnFail
    ModelTier    string
    Readonly     bool
    RequiresTools *RequiresTools
    // 没有 Timeout time.Duration 字段
    // 没有 ExpectedMaxDuration 字段(仅用于预测)
}
```

**5. `LoopEngine` 没有迭代超时**:

```go
// forge-core/internal/orchestrator/loop.go:120-145
type LoopEngine struct {
    // 没有 PerIterationTimeout
    // 没有 MaxWallClockDuration
    Ctx context.Context // 唯一的超时来源
}
```

**6. 超时传播语义完全缺失**:
- phase 超时 → 是否应该跳过当前迭代的剩余 phase?当前:是(返回 error,循环终止)
- phase 超时 → 是否应该降级 agent tier 重试?当前:否(直接失败)
- 迭代超时 → 是否应该触发 checkpoint 并继续下一轮?当前:否(循环终止)

### 与已有覆盖的区别

- `novel-architectural-extensions-v40.md` 方向三「生产韧性」的表格里有一行
  「phase timeout（120s）允许,但整体执行时间没上限」——这只是一个观察,
  没有作为独立方向展开(无代码级证据链、无传播语义分析、无建议实现路径)。
- `fourth-wave-architecture.md` 提到「timeout propagation」但没有具体展开。
- 本方向是第一次将 **timeout 层次缺失**作为编排模型的系统性缺口分析。

### 边界情况

- **超时继承规则**:如果 phase 声明了 `timeout: 2min`,但迭代超时=1min,应该谁赢?
  建议:更严格的一方赢(总超时 1min),以保护整体预算。
- **agent 卡级别的默认超时**:reviewer(opus,更贵)应该比 implementer(sonnet)有更短的超时?
  还是更长的超时(因为 opus 思考更慢)?这需要可配置的默认值。
- **超时后的不同动作**:超时不一定是「硬失败」——可以是:
  - `timeout_action: retry_with_lower_tier` — 用 haiku 重试
  - `timeout_action: skip_and_continue` — 跳过这个 phase,继续迭代
  - `timeout_action: skip_iteration` — 跳过整轮迭代
  - `timeout_action: abort` — 默认,立即终止
- **超时与 cost 的交互**:如果一个 phase 已经花了 $0.50 然后超时了,
  这个成本是否应该计入 runBudget?当前:计入(因为 `feed` 在 Execute 返回前已被调用)。
  可能需要「成本回滚」语义(见方向二的补偿原语)。

### 建议方向

1. **`Phase.Timeout` 字段**:在 workflow YAML 的 phase 声明中增加可选的 `timeout` 字段:
   ```yaml
   phases:
     - name: implement
       agent: implementer
       timeout: 5m
   ```
2. **`workflow.timeout`/`evolve.timeout`**:在 workflow/evolve 声明中增加运行级超时,
  作为 CLI `--timeout` 的工作流内声明式替代。
3. **超时传播语义**:定义 `hard_timeout`(不可重试,直接失败)和 `soft_timeout`
  (触发降级/重试/跳过,记录到 trace)两种超时类型。
4. **超时=>降级回退**:当 phase 连续 N 次 soft timeout,自动降低其 model tier(TTT:
  sonnet→haiku)以更快完成。这需要与方向五(梯度响应)和方向二(补偿)协同。
5. **trace 中的超时事件**:当前 `trace.go` 有 `KindTimeout` 作为 exec_error 的分类,
  但没有专门的 timeout trace event。增加 `trace.Event{Kind:"timeout", Detail:"timeout after 5m"}`。

---

## 方向五 · 编排器状态机缺少形式化模型——正确性依赖人力审查

**优先级**: 🟡 P2 | **类别**: 正确性 · 架构 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的编排器(`internal/orchestrator`)是一个**隐式状态机**——它的状态、转换、和守卫条件
全部隐藏在命令式 Go 代码中:

- `RunFrom` 是一个 200 行的 `for` 循环 + `switch on phase type`
- `LoopEngine.Run` 是另一个 `for` 循环,与 `RunFrom` 的交互通过 `startPhase` 参数
- `startPhase` 的解析链涉及 `resolveRejectionStartPhase`(gates.go)→`nextStartPhase`(loop.go)
  → `loopStart`(loop.go),跨三个文件
- `on_fail.loop_back` 的跳转目标需要追溯 `orchestrator.go:343-358` 的 `loopBackTo`
- `mode_gating` 的 skip/run 决策涉及 `mode_gating.go` 的 4 个函数、`mode_policy.go` 的
  3 个深度枚举、和 `runPhaseParallel`/`RunFrom` 两条路径

结果是:
- **没有任何机器可读的状态转换表**——每个 agent 的行为只能通过阅读精心编写的文档注释来理解
- **无法证明关键安全属性**:「human_gate 永不旁路」「loop-back 上限始终遵守」「production 覆盖永不被宽松 mode 绕过」
- **每次新增机制(如 review_depth、mode_gating)都要手动分析对全部现有属性的影响**

### 代码级证据

**1. `RunFrom` 是一个 200+ 行的 `for`+`switch`——没有集中式状态定义**:

```go
// forge-core/internal/orchestrator/orchestrator.go:230-475
func (e *Engine) RunFrom(wf asset.Workflow, mode string, startPhase int) error {
    for i := startPhase; i < len(wf.Phases); i++ {
        p := wf.Phases[i]
        // 四个隐式状态分支:
        if e.checkStageSkip(wf) { /* Discover/Review 阶段跳过 */ }
        if e.skipByMode(p, wf.Stage) { /* mode 门控跳过 */ }
        if len(p.RequiredGates) > 0 { /* gate phase 路径 */ }
        /* agent phase 路径 */
    }
}
```

没有 `type State int`、没有 `String()`、没有 `Transition()`。

**2. 循环出口条件分散在三层调用栈中**:

```
Exit 位置                       | 文件                    | 条件
─────────────────────────────────────────────────────────
RunFrom 的 loop-back 后重试     | orchestrator.go:343    | MaxLoopBack 耗尽
RunFrom 的 gate FAIL           | orchestrator.go:283    | gate 失败且无 on_fail
LoopEngine.Run 的 converge     | loop.go:170-180        | converge.Met==true
LoopEngine.Run 的 stale        | loop.go:350-365        | staleCount ≥ NoProgress
LoopEngine.Run 的 MaxIter      | loop.go:370-380        | i > MaxIter
LoopEngine.Run 的 ctx canceled | loop.go:135-140        | ctx.Err()!=nil
```

没有一张图、没有一张表说明「从状态 A 在条件 C 下到达状态 B」。

**3. 安全属性的正确性只能靠手动推理——没有形式化断言**:

```go
// 以下属性的正确性只能靠读代码+写注释来保证:
//
// 1. human_gate 永不被 forge evolve 绕过
//    → 验证路径: evolve.go:65-67 rejectHumanGate → 在 buildLoop 前就拒绝
//    → 第二层:  loop.go:155-160 Converge 判 approval 而非 all_of
//    → 没有单元测试或自动化检查能证明「不可能存在第三条路径绕过」
//
// 2. production lifecycle 覆盖不可被宽松 mode 绕过
//    → 验证路径: mode.go:150-170 Effective 中 lifecycle=production→enforce=block
//    → 但不是全局的——review.yml 的 optional_for 分支
//       (mode_gating.go:51-94)在 Sprint 27 之前就存在旁路
//    → 这不是「bug」,是**无法用当前测试架构验证的负向属性**
//
// 3. MaxLoopBack 上限始终被遵守
//    → 验证路径: orchestrator.go:355-358 loopBackTo 在每次跳转前递增 counter
//    → counter 从 0 开始,MaxLoopBack=3 时第 4 次跳转被拒绝
//    → 但 startPhase 输入(通过 rejection/on_unmet)是否绕过这个计数?
```

**4. `asset.Phase` 没有「合法状态」约束——所有字段都是自由文本**:

```go
// forge-core/internal/asset/asset.go:40-105
type Phase struct {
    Name         string // 无合法值约束
    Agent        string // 无 agent 卡引用验证(在 check.py 里,不在类型系统里)
    RequiredGates []string // 无合法 gate 名验证
    OnFail       *OnFail  // 无 action 合法值约束("loop_back" vs "undo" vs "compensate")
    ModelTier    string   // 无合法 tier 约束("haiku"/"sonnet"/"opus")
}
```

这些都通过 `check.py` 在运行时验证,但类型系统本身不阻止非法状态。

### 与已有覆盖的区别

- **零覆盖**——在所有 90+ 分析文档中,没有任何一篇将编排器状态机的形式化模型作为方向。
  最接近的是 `novel-five-perspectives-2026-07-10-deep.md` 方向四的
  `forge validate --state-machine`,但它关注的是**workflow 文件自身**的合法性验证
  (stop_condition、phase 引用、mode_gating 值),不是**编排运行时**的状态机模型。
- `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 标注了 `stop_condition.on_rejected` 的死代码问题,
  但那是一个具体 bug,不是系统性状态机形式化缺失。
- 本方向与所有已有分析的区别在于层次:已有分析关注「机制是否实现」,本方向关注
  「机制是否正确——且如何证明它正确」。

### 边界情况

- **状态爆炸**:`RunFrom` 的隐式状态涉及 i(phase 索引)、loopBackCount、modePolicy 的
  多个字段、EvolveDepth 的 maxIter——组合起来的状态数可能是 $phase\_count × loop\_back ×
  mode × lifecycle × iter$,需要抽象简化才能做形式化。
- **非确定性输入的建模**:LLM agent 的输出不可预测——形式化模型只能建模编排器的确定性部分
  (gate 结果、phase 顺序、超时),不能建模「agent 是否会写出正确的代码」。
- **模型与实现的不同步**:如果形式化模型在 ADR 中定义但代码实现偏离了它,模型就成了
  一个**新的「声明 vs 实现」漂移源**。需要像 `check.py` 的 workflow 验证一样,
  有机制验证模型与实现的一致性。

### 建议方向

1. **显式状态定义**:在 `internal/orchestrator/state.go` 中定义:
   ```go
   type RunState int
   const (
       StatePhaseExecute   RunState = iota // 正常执行 phase
       StateGateLoopBack                   // gate 触发 loop-back
       StateSkipByMode                     // mode 门控跳过
       StateStageSkipped                   // discover/review 整阶段跳
       StateRejectedRestart                // human_gate rejection 重定向
       StateIterationDone                  // 迭代结束,准备下一轮
       StateIterationConverged             // 收敛
       StateAborted                        // 硬失败
   )
   ```
2. **状态转换映射表**:在 `RunFrom` 中用一个显式的 `stateTransitions` 表(而不是 `for`+`switch`)
  来编码合法转换。每个 `Phase` 的结尾显式计算 `nextState(currentState, phase, gateResult)`,
  让转换关系可枚举、可测试、可审查。
3. **关键安全属性的自动化检查**:在 `orchestrator_test.go` 中增加属性检查风格的测试:
   - `TestInvariant_HumanGateNeverBypassed` — 验证不存在任何路径能在无 approval 时达成 converge
   - `TestInvariant_ProductionOverridesMode` — 验证 production lifecyle 下所有宽松 mode 被压平
   - `TestInvariant_LoopBackBudgetEnforced` — 验证不存在任何入口可绕过 MaxLoopBack
4. **故障插入测试(fault injection)**:不追求完整的形式化模型,而是为最关键的三个属性
  (human_gate 不被绕过、production 覆盖、loop-back 上限)构建**可证伪的测试骨架**:
  每组测试尝试 N 种「攻击」路径来违反属性,任何新路径的发现就自动补入测试。
  这不是 TLA+级别的形式化,但比「靠人读注释保证安全性」前进了一步。
5. **ADR 状态机图**:在 `.agent/architecture/` 下增加状态机 ASCII 图或 Mermaid 图,
  描述 orchestrator 的顶层状态和转换,作为所有未来编排修改的共同参考。

---

## 优先级与收敛建议

| # | 方向 | 优先级 | 杠杆(1-5) | 成本 | 为什么是这个优先级 |
|---|---|---|---|---|---|
| 1 | `cmd/forge` 依赖中枢退化 | P2 | ⭐⭐⭐⭐⭐ | ~3 sprints | 结构债务,短期不影响功能,长期阻碍演化 |
| 2 | Agent 执行器测试盲区 | **P0** | ⭐⭐⭐⭐⭐ | ~2 sprints | **核心价值主张未被测试验证**,一旦真 agent 行为与 dry-run 不同,整条编排链路可能集体失效 |
| 3 | Memory Store 无界增长 | P2 | ⭐⭐⭐ | ~1 sprint | 直接影响 24h+ 运行可靠性,但修复成本低 |
| 4 | 超时层次缺失 | P1 | ⭐⭐⭐⭐ | ~1.5 sprints | 当前一维超时对多 phase 工作流是活跃的可用性问题 |
| 5 | 编排器状态机形式化模型 | P2 | ⭐⭐⭐⭐ | ~2 sprints | 长期正确性投资,短期有「属性检查测试」的低成本起点 |

**若只做一件**:方向二(Agent 执行器测试盲区)。这是**最危险的盲区**——全仓编排逻辑在
`DryRunExecutor` 上验证通过,而生产用的是 `CommandExecutor`,两者的差异可能是灾难性的。
一个 `forge run build --executor command --agent-cmd echo` 的 CI 步骤可以立即以零成本
暴露大部分差异。

**若做三件**:二→四→五。先解决「测试架构盲区」(P0),再解决「可用性缺口」(P1),
再用形式化模型锁定编排器的核心正确性属性(P2)。三者的组合覆盖了「测试→可用性→正确性」
的全栈。

**方向一和三**保留为架构演进的方向:当 ForgeOS 进入跨项目/跨团队部署阶段时,`cmd/forge`
的耦合会成为独立部署的阻碍;当 24h 跑成为常态时,memory 增长会成为运维问题。
两者都不是今天必须解决的,但都应该在当前 sprint 规划中留一个 visibility slot。

---

## 诚实的边界

1. **方向一(依赖中枢)不是「cmd/forge 太胖马上需要拆」**——它今天工作得很好,单文件预算
  纪律已经防止了最坏情况(超 500 行)。方向一的建议是**为 k8s 分离部署/独立组件化做准备**,
  不是修复一个当前故障。
2. **方向二(测试盲区)的 P0 评级的前提假设**是 `DryRunExecutor` 与 `CommandExecutor` 之间
  存在未被测试覆盖的行为差异。如果「echo executor」CI 步骤证明了行为一致,优先级可降至 P2。
3. **方向五(形式化模型)不追求 TLA+/Alloy 级别的完整形式化验证**——追求的是「关键安全属性的
  自动化可证伪」,比「完全形式化」低一个数量级的投入。标记为 P2 是基于这个更窄的范围。
4. 这四个方向全部是**结构性/架构性**的,不是功能性的——用户不会因为方向一~五的落地
  而获得新的 CLI 子命令或新的 workflow 能力。它们让 ForgeOS 更可维护、更可测试、更可靠,
  但不改变它今天能做的事情。
