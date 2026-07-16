# ForgeOS Go 运行时健康检查

> **第四次扫描**，这次关注 **Go 运行时的代码质量与工程实践**
> —— 错误处理、并发模型、测试策略、配置卫生、CLI 设计、包健康度。
>
> 不写代码，只做诊断。

---

## 目录

1. [并发模型：有意的单线程 vs 蔓延的风险](#1-并发模型有意的单线程-vs-蔓延的风险)
2. [错误处理的"一致但不完整"](#2-错误处理的一致但不完整)
3. [信号处理与优雅关闭的真空](#3-信号处理与优雅关闭的真空)
4. [测试金字塔的强度与裂缝](#4-测试金字塔的强度与裂缝)
5. [配置加载的"容错过度"](#5-配置加载的容错过度)
6. [CLI 工程化的欠账与边界](#6-cli-工程化的欠账与边界)
7. [跨语言集成点的测试盲区](#7-跨语言集成点的测试盲区)

---

## 1. 并发模型：有意的单线程 vs 蔓延的风险

### 1.1 十亿分之一的设计选择

整个 forge-core 的**核心执行路径**（orchestrator.RunFrom、LoopEngine.Run、backoff、gate 运行、
convergence 判断）是**严格的单线程**——没有 goroutine、没有 channel、没有 sync.WaitGroup。
唯一引入 goroutine 的地方是 `orchestrator.RunParallel`（parallel.go），它明确管理生命周期。

这是一个**有意的、正确的设计选择**，原因：
- 状态机（phase 顺序、loop-back、checkpoint）在单线程中更容易推理和证明正确性
- 没有数据竞争 → 不需要大多数锁 → 代码更简单
- 序列化开销可忽略（Agent 执行本身是调用外部进程，时间成本在 IO 上不在 CPU 上）

### 1.2 单线程的风险边界——goroutine 问题的根源

虽然在主路径上单线程是正确的，但**并行模式（parallel.go）引入了一个脆弱点**。

```go
// parallel.go 的并发模式
for w, wave := range waves {
    var wg sync.WaitGroup
    for _, idx := range wave {
        wg.Add(1)
        go func(i int) {
            defer wg.Done()
            // 这里访问共享的可变状态：
            // - cost.go 的 runBudget.mu （外部）
            // - prompt_context.go 的 ContextLedger.mu （外部）
            // - prompt_verdict.go 的 two ledgers
            // - trace.go Tracer.mu
            // - gates.go loopProbe.mu
            // - 本地 mu 保护 firstErr + agentCalls
        }(idx)
    }
    wg.Wait()
}
```

这个模式**本身正确**（所有共享状态都用 mutex 保护），但有几个观察：

a) **每个 goroutine 内的所有 mutex 不是由同一个高级锁保护的**——这意味着两个 goroutine 可以同时持有不同的锁（例如 goroutine A 锁了 cost.go 的 mu 后试图锁 trace.go 的 mu，goroutine B 反向操作）。按当前代码的顺序这不会死锁，因为锁的获取顺序在所有路径上一致，但**没有代码文档来保证这一点**。如果将来新加一个被多个 goroutine 访问的状态，这个锁顺序的一致性很容易被破坏。

b) **RunParallel 不处理 panic**——如果任何一个 goroutine panic，`defer wg.Done()` 仍然会执行，但 panic 传播到 `go func` 的栈顶会杀死整个进程。没有 `recover()` 来优雅地降级。

c) **RunParallel 的 wave 内没有超时**——每个 goroutine 可能无限期阻塞（例如在 Acquire/Spawn 的锁上）。虽然 `CommandExecutor` 有 `Timeout`，但那是针对单个 agent 命令的，不是针对整个 wave 的。

### 1.3 没有 context.Context 的影响

整个代码库**没有使用 Go 的 `context.Context`**：

```
$ grep -rn 'context.Context' forge-core/ --include='*.go'
（空）
```

仅有的 context 用法是 `command_executor.go` 内部的 `context.WithTimeout` 用于一个命令的生命周期。

这意味着：
- 没有全局取消传播——如果 RunParallel 的一个 phase 失败，其他进行中的 phase 无法被取消
- 没有请求范围的超时——只有命令级超时，没有 workflow 级或 iteration 级的超时
- 没有值传播——无法向下一层传递跟踪 ID、session ID 等（trace.go 有 seq 但不由上下文传递）

### 1.4 评估

| 维度 | 当前状态 | 严重性 | 建议 |
|------|---------|--------|------|
| 核心路径单线程 | ✅ 正确设计，无竞争 | — | 保持 |
| parallel 模式 goroutine | ⚠️ 无 panic 保护 | 低 | 加 recover + 降级 |
| 锁顺序文档 | ❌ 无 | 低 | 给 parallel.go 加注释，说明所有共享状态的锁获取顺序 |
| context.Context | ❌ 缺失 | 中 | 在 Engine 和 LoopEngine 中增加 context，逐步向下传播 |
| 信号处理 | ❌ 见第 3 节 | 高 | — |

---

## 2. 错误处理的"一致但不完整"

### 2.1 好的一面

- `fmt.Errorf("...%w", err)` 一致地用于错误包装
- `errors.As` 用于类型断言（ExecError）
- 没有 `panic` / `recover`（非测试代码中为零）
- 没有 `log.Fatal`—只有 `fmt.Fprintf(os.Stderr, ...)` 和 `os.Exit`

### 2.2 缺口

#### 2.2.1 错误和日志混合在一起

`cmd/forge/` 的许多函数既**返回 error** 又通过 `fmt.Fprintf(os.Stderr, ...)` **写入日志**。
调用者看到 error 时，通常错误信息已经写到 stderr 了。这导致 CLI 输出中有重复的消息：

```go
// main.go
if err != nil {
    fmt.Fprintf(os.Stderr, "forge run: %v\n", err)
    return 1
}
```

问题是 `err` 的内容可能已经通过更深层的函数写到了 stderr。双重消息、不一致的前缀格式。

#### 2.2.2 没有结构化错误类型

除了 `ExecError`，没有其他错误分类：
- Gate 错误是 `error` 接口——无法区分"工具未安装"与"测试失败"与"配置文件找不到"
- 所有 `asset.LoadWorkflowJSON` 的错误是同一类 `"asset: invalid workflow JSON: %w"`
- 所有 `persist.Save/Read` 的错误是同一类 `"persist: ..."`

#### 2.2.3 `cmd/forge` 的 int 返回风格

所有命令处理函数返回 `int`（退出码）并通过副作用写入 stderr。这使它们难以被 Go 测试非子进程调用：

```go
func cmdRun(args []string) int {  // 只能用子进程测试
    ...
    fmt.Fprintf(os.Stderr, "forge run: %v\n", err)  // 测试无法捕获这个
    return 1
}
```

更好的模式：返回 `error`，让 `run()` 函数统一处理日志和退出码。

### 2.3 评估

| 方面 | 状态 | 批评 |
|------|------|------|
| Error wrapping | ✅ 一致 | 好 |
| Error 分类 | ❌ 只有 ExecError | gate/asset/persist/checkpoint 各玩各的 |
| 日志-错误分离 | ⚠️ 混合 | cmd/forge 的双重输出 |
| cmd/forge 测试性 | ⚠️ int 返回 | 只能 exec 子进程 |
| panic 保护 | ✅ 零 panic | 极好 |

---

## 3. 信号处理与优雅关闭的真空

### 3.1 事实

**ForgeOS 没有注册任何信号处理程序。** `os/signal` 没有被导入。

```go
import (
    "flag"
    "fmt"
    "os"
    "os/exec"
    "path/filepath"
    "strings"
    "time"
    // ← 没有 signal 包
)
```

当用户按 Ctrl+C：
1. 操作系统发送 SIGINT 给进程
2. Go 运行时的默认信号处理程序杀死程序
3. 没有 checkpoint 写入
4. 没有 trace flush
5. 没有 `defer` 运行（`defer` 不响应信号）
6. 子进程（claude 命令）变成孤儿或一起被杀死

### 3.2 影响

- **`forge evolve` 中断**：进化循环可能在 iteration 中间被杀死，不写入 checkpoint 且不保留当前 iteration 的 trace。`--resume` 可以从上一次 iteration 的 checkpoint 恢复，但会丢失当前 iteration 的所有工作（包括已支付成本的 agent 执行）。
- **`forge run` 中断**：没有 resume 机制——必须从头开始。
- **并行模式中断**：多个 goroutine 正在运行 phase。SIGINT 杀死其中一个，其他变成孤儿（或与父进程一起被杀——取决于进程组设置）。
- **文件损坏**：trace.jsonl 的最后一个写入行可能不完整（writer 的 bufio 缓冲未刷新）。memory.jsonl 类似。checkpoint.json 在 rename 中被杀死可能导致文件丢失（但不会损坏，因为写是原子的）。

### 3.3 建议

最直接的方法——在 `main.go` 中：

```go
// 注册信号处理
sigCh := make(chan os.Signal, 1)
signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)
go func() {
    sig := <-sigCh
    logln(fmt.Sprintf("forge: caught %v, shutting down gracefully...", sig))
    // 1. 取消所有正在进行的 phase（通过 context）
    // 2. 写入 checkpoint（仅 evolve）
    // 3. 刷新 trace
    // 4. 等待子进程退出
    os.Exit(130)  // 128 + SIGINT
}()
```

这需要对 engine 增加一个 `context.Context` 支持——见第 1 节。

### 3.4 评估

| 方面 | 状态 | 严重性 | 建议 |
|------|------|--------|------|
| SIGINT 处理 | ❌ 不存在 | **高** | 注册信号处理 + 优雅关闭 |
| 子进程清理 | ❌ 不存在 | 中 | 进程组/context 取消 |
| 部分结果恢复 | ❌ 无法 | 中 | forge run 加 resume |
| 文件写入完整性 | ⚠️ 原子写入但未刷新 | 低 | trace 写完之后刷新 writer |

---

## 4. 测试金字塔的强度与裂缝

### 4.1 数字说话

```
总 Go LOC:     20,069
测试 LOC:      11,164  （55.6%）
测试文件:      46 / 86  （53.5%）
非测试文件:    40
```

这个测试密度是**健康的**。而且每个 internal package 都有对应的 `*_test.go`。

### 4.2 分层看测试覆盖

| 层级 | 文件 | 测试 | 质量 |
|------|------|------|------|
| **单元测试（纯函数）** | converge.go、risk/*、mode.go、waves.go、scorecard.go、backoff.go | ✅ 完好 | 良好 |
| **组件测试（带 IO/状态）** | asset/asset_test.go、gate/gate_test.go、checkpoint_test.go、memory_test.go | ✅ 充分 | 良好 |
| **orchestrator 测试（引擎，mock gate/executor）** | orchestrator_test.go、loop_test.go、parallel_test.go、tier_test.go 等 | ✅ 多 | 良好 |
| **mode gating 测试（引擎 + 策略）** | orchestrator_modegating_test.go | ✅ 存在 | 良好 |
| **cmd/forge 测试（通过子进程）** | main_test.go、evolve_test.go、cost_test.go、gates_test.go 等 | ⚠️ 大量 | 但都是子进程 |
| **cmd/forge 单元测试（纯逻辑）** | attribution_test.go、scorecard_wind_test.go、migrate_test.go、prompt_*.go | ✅ 部分 | 好 |
| **跨语言集成测试（Go → Node）** | scorecard_wind_test.go | ❌ 无 | **瓶颈** |

### 4.3 测试模式的三个问题

#### 4.3.1 cmd/forge 过度依赖子进程测试

因为 `cmdRun(args []string) int` 模式，cmd/forge 的许多测试必须编译一个临时二进制文件并运行它：

```go
// evolve_test.go
func TestCmdEvolve_* (t *testing.T) {
    // 编译 forge -> go build -o /tmp/forge-test-xxx
    // 运行 /tmp/forge-test-xxx evolve --root ...
    // 检查退出码、stderr、stdout
}
```

这有几个问题：
- 每跑一个测试都需要 ~0.5 秒的编译时间
- 测试间通过文件系统耦合（必须创建临时根目录、写入 .agent/workflows/*.yml 等）
- 调试困难——失败的是子进程，不是直接失败的函数调用
- CI 中可能因文件系统权限/并发冲突而出现不稳定的失败

#### 4.3.2 RunParallel 没有 `-race` 保障的并发测试

`parallel_test.go` 测试了依赖波逻辑，但**没有在 `-race` 下运行**。文件头部的注释明确指出：
"ContextCache + loopProbe 在第一次实现中被遗漏——一个 fresh-context reviewer 的 -race 运行捕捉到了 ContextCache 竞争端到端"

这意味着：
- 没有自动化的 race 检测测试（在 CI 中 `go test -race` 没有覆盖 parallel 执行路径）
- 下次有人修改共享状态时，同样的模式可能再次出现
- parallel 模式的安全依赖于开发者对锁纪律的维护

#### 4.3.3 scorecard wind-down 路径未经测试

`scorecard_wind.go` 是整个系统中唯一的**跨语言集成点**（Go → Node.js）：

```go
// scorecard_wind.go 调用一个 Node 脚本
cmd := exec.Command("node", "harness/scorecard-update.mjs", "--model", ...)
```

这个路径：
- 从 trace.jsonl 解析 cost events（在 Go 中完成）
- 为每个 unique (model, task_type) pair 构建参数
- 调用 `node harness/scorecard-update.mjs`（Node 中完成）
- Node 端读取 trace、聚合、合并、写回 scorecards.json

**没有任何自动化测试覆盖这个路径**。`scorecard_wind_test.go` 测试了 `distinctScorecardPairs` 和 `traceHasModelCost` 等 Go 端函数，但从不实际调用 Node 脚本。

### 4.4 评估

| 问题 | 严重性 | 建议 |
|------|--------|------|
| cmd/forge 子进程测试模式 | 中 | 重构为 error 返回模式，允许函数级测试 |
| parallel 模式无 -race | 中 | CI 加 `-race`；加 concurrent 状态验证 |
| 跨语言 scorecard 路径 | 中 | 集成测试或 mock Node 脚本 |
| forge run 无 resume 测试 | 低 | 无此功能 |
| 整体测试比例 | ✅ 好 | 保持 |

---

## 5. 配置加载的"容错过度"

### 5.1 核心问题

`asset.LoadWorkflowJSON` 明确声明"容错设计"：

> "It is fault tolerant by design: only a syntactically invalid document is an error.
> A document missing fields yields a zero-valued-but-usable Workflow."

这意味着：
- YAML 中拼写错误的字段名（例如 `requred_gates` 而不是 `required_gates`）→ **静默忽略**，不报警
- 缺失 `phases` → 零 Phase 的 Workflow → `Run()` 空循环 → **phase 0 就报告 stop → 不做任何事就成功**
- 缺失 `stop_condition` → `zero-value StopCondition{Type:""}` → `converge.Converge` 看到 type=""，做什么？

让我们看看 converge 对零值 StopCondition 的行为：

```go
// converge.go
func Converge(stop asset.StopCondition, sig Signals) (...) {
    if stop.Type == "human_gate" {
        return humanGate(stop, sig)
    }
    // 默认走 Evaluate(all_of) → AllOf 为空 → Evaluate 返回 met=true
}
```

所以：缺失 stop_condition → `Type=""` → 走 Evaluate → AllOf 为空 → 所有 criteria 认为"满足" → **false-converged**。

### 5.2 容错的目标用户 vs 潜在危害

容错的目标：在部分/损坏的 YAML 上不崩溃，给用户反馈。
潜在危害：零值 Workflow 报告"converged=true"—这是**最危险的虚假成功**。

实际上，这个风险被两重机制减轻了：
1. `harness/check.py` 在工作流加载前验证 YAML schema
2. 流程不是先加载 JSON 然后再验证——他们是先由 check.py 做严格的治理验证，然后才走到 Go 的 LoadWorkflowJSON

但 Go 运行时本身不做任何健全性检查：它信任预处理过的 JSON。

### 5.3 多个配置文件独立加载，无一体验证

| 文件 | 加载方式 | 运行时验证 | 治理验证 |
|------|---------|-----------|---------|
| `.agent/workflows/*.yml` | asset.LoadWorkflowJSON | ❌ 容错 | ✅ check.py |
| `.agent/project.yml` | 仅由 go 通过 python yaml2json 转 | ❌ 无 | ❌ 无 |
| `.agent/policies/modes.yml` | internal/mode 解析 | ❌ 无 | ❌ 无 |
| `.agent/routing/policy.yml` | 纯 JS 端读取 | ❌ 无 | ❌ 无 |
| `.agent/routing/scorecards.json` | 纯 JS 端读取/写入 | ❌ 无 | ❌ 无 |

`check.py` 只验证 workflow schema 和 governance 完整性（文件存在、必要的 section 存在），不验证：
- 跨文件引用正确（`required_when: ../policies/modes.yml#workflow_depth.reviewer` 指向的键是否存在）
- `mode.yml` 的 `workflow_depth` 下的 lifecycle 在 `project.yml` 中是否有效
- workflow 中引用的 agent 名称是否匹配 agent card

### 5.4 建议：`forge validate` 命令

```
forge validate [--root DIR]

检查项：
  ✅ .agent/project.yml — 可解析，mode/lifecycle 有效
  ✅ .agent/policies/modes.yml — 可解析，引用的生命周期在 project.yml 中
  ✅ .agent/routing/policy.yml — 可解析
  ✅ .agent/workflows/*.yml — 每个可解析，非零 phase，stop_condition 完整
  ✅ .agent/workflows/ — 引用的 agent 名称匹配 AGENTS.md 或 agent cards
  ✅ cross-reference — 所有 required_when 引用存在
  ✅ 无未引用的 workflow 文件
```

### 5.5 评估

| 方面 | 状态 | 严重性 | 建议 |
|------|------|--------|------|
| 容错加载 → false-converged | ⚠️ 理论上存在 | 中低（check.py 减轻） | LoadWorkflowJSON 加最小健全性检查 |
| 跨文件引用验证 | ❌ 缺失 | 中 | forge validate 命令 |
| 运行时配置验证 | ❌ 缺失 | 低 | engine 启动时快照检查 |

---

## 6. CLI 工程化的欠账与边界

### 6.1 分析

`forge` CLI 使用标准 `flag` 包，有 `run`、`evolve`、`gate`、`check`、`accept`、`route`、`migrate`、`detect` 八个子命令。

#### 6.1.1 好的做法
- `bindRunOpts` 共享 run 和 evolve 之间的标志定义——防止漂移
- 缩写用法文本清晰
- `delegate` 函数简洁地包装 gate/check/accept 的通用模式

#### 6.1.2 缺失

**a) 没有 `--version`**
无法知道用户运行的是哪个版本。不能调试用户报告的问题。

**b) 没有 shell 补全**
不支持 `forge run <TAB>` 列出 workflow 名称。使用需要文件系统知识。

**c) 没有 `--help` 的子命令差异**
`forge run --help` 和 `forge evolve --help` 显示相同的用法模板（因为 `bindRunOpts` 共享）。但 evolve 有额外的 `--max-iter`、`--resume` 等，这些被埋在文件头的使用文本中，而不是通过 `flag.PrintDefaults` 显示。

**d) 没有结构化输出**
```
$ forge run build --executor dry
forge run: stage=build mode=balanced lifecycle=mvp executor=dry gates=true reviewer=true discover=none design=false adr=false (6 phases)
convergence: MET (conjunction)
  [x] roadmap_completion >= 100% — All roadmap items done
  [x] gates_green == true — All gates green: [lint test app_test]
forge run: workflow completed
```

这个文本输出对人和 CI 日志都很好，但**不能机器解析**。如果用户想用 `jq` 处理输出或在 CI 中检查特定值，需要 `--json` 或 `--format json`。

**e) 没有 `forge status`**
用户无法检查当前 `.forge/` 目录的状态——是否有一个中断的运行？哪个 workflow？哪个 iteration？多少 memory 条目？

**f) 错误退出码只用 1 和 2**
- 没有区别"语法错误"（2）和"运行失败"（1）和"系统错误"（不可恢复）
- 用户无法区分"test failed (gate red)"和"config file not found"——都是 exit 1

### 6.2 评估

| 功能 | 当前状态 | 严重性 |
|------|---------|--------|
| --version | ❌ | 低（但阻碍调试） |
| 结构化输出 | ❌ | 低-中 |
| shell 补全 | ❌ | 低 |
| forge status | ❌ | 低-中 |
| 差异退出码 | ⚠️ 只有 1/2 | 低 |
| 子命令 help 差异 | ⚠️ 共享但不完整 | 低 |

---

## 7. 跨语言集成点的测试盲区

### 7.1 全景

ForgeOS 跨三种语言：

```
┌──────────────┐      JSON       ┌─────────────────┐
│  Go 运行时    │ ←───────────── │  Python 预处理     │
│  (orchestrator│    yaml2json   │  (yaml2json.py,   │
│   engine,     │                │   check.py)       │
│   gates)      │                │                   │
└──────┬───────┘                 └─────────────────┘
       │
       │ exec.Command("node harness/scorecard-update.mjs ...")
       │ exec.Command("claude -p ...")
       │
       ▼
┌─────────────────┐
│  Node.js 运行时  │
│  (harnass gates, │
│   scorecards,    │
│   adapters)      │
└─────────────────┘
```

### 7.2 每个接口的测试覆盖

| 接口 | 方向 | 协议 | 测试覆盖 |
|------|------|------|---------|
| yaml2json.py | Python → JSON → Go | 文件系统 | ❌ 无（只有手动） |
| check.py | Python → stdout → 用户 | 返回码 | ❌ 无（手动） |
| scorecard-update.mjs | Go → Node | exec.Command | ❌ 无集成测试 |
| gate.mjs | Go → Node | exec.Command | ⚠️ gate_test.go 通过子进程测试 |
| claude | Go → claude binary | exec.Command | ❌ 无（dry 模式跳过） |
| .agent/workflows/\*.json | Go 加载文件 | 文件系统 | ✅ asset_test.go |
| .forge/trace.jsonl | Go → Node | 文件系统 | ⚠️ parseTraceLatencies 有纯测试 |

### 7.3 问题

**最大的风险**：scorecard wind-down 路径的每一次测试都必须手动运行：

1. 在真实环境中运行 `forge evolve`（需要 claude binary）
2. 检查 `.forge/trace.jsonl` 是否有 cost events
3. 运行 wind-down 脚本
4. 检查 `.agent/routing/scorecards.json` 是否正确更新

**Python 桥接层**不在测试中——`yaml2json.py` 被 `asset.LoadWorkflowJSON` 通过 exec 调用：

```go
// main.go 中未显示但 cmdRun/execEngine 调用 python3 harness/yaml2json.py
```

如果 Python 环境发生了变化（python3 不存在、pyyaml 未安装、路径错误），Go 运行时悄悄地接收空 JSON 或失败。

### 7.4 评估

| 集成点 | 风险等级 | 建议 |
|--------|---------|------|
| yaml2json.py | 中（运行时失败） | 将流程改为 Go 原生 YAML 库，或 fallback 检测 |
| scorecard-update.mjs | 高（数据完整性） | 增加集成测试，mock Node 脚本或使用子进程 |
| claude | 低（dry 模式默认） | 接受现状 |
| gate.mjs | 低（已有子进程测试） | 保持 |

---

## 综合优先级

| 排序 | 项目 | 类别 | 风险 | 修复成本 |
|------|------|------|------|---------|
| 🥇 | **信号处理 + 优雅关闭** | 崩溃安全 | **高**（数据丢失） | 低（~50 行 + 接口更改） |
| 🥇 | **parallel mode race 检测** | 数据竞争 | **高**（并行 bug） | 低（CI 加 -race + 测试） |
| 🥇 | **跨语言 scorecard 集成测试** | 数据完整性 | 中-高 | 中（mock Node 或集成测试） |
| 🥈 | **context.Context 传播** | 架构 | 中（但为并行/关闭铺路） | 中（Engine/CommandExecutor 接口变更） |
| 🥈 | **cmd/forge 重构为 error 返回** | 可测试性 | 中 | 中（文件级重构） |
| 🥈 | **forge validate 命令** | 用户体验 | 中 | 低 |
| 🥉 | **容错加载的 false-converged 防护** | 健全性 | 中-低 | 极低（几行检查） |
| 🥉 | **forge version + forge status** | 运维 | 低 | 低 |

*分析日期：2026-06-29 | 基于第四次全量源码扫描（Go 运行时健康视角）*
