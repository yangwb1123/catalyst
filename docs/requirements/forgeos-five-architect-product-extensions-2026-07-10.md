# ForgeOS — 资深架构师/产品经理视角的五个高价值扩展方向

> **角色**: 资深架构师 + 产品经理  
> **方法**:  
> 1. 全局深扫: forge-core（18 包 · 63 非测试 Go 源文件 · 32k+ LOC · 纯 stdlib 零依赖）+  
>    cmd/forge（40+ 子命令 · ~12k LOC）+ harness（39+ 模块 · ~10.5k LOC 执法层）+  
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR+DECISIONS+architecture）+  
>    `examples/`（url-shortener · go-taskd）+ `pi-batch.py` + `.github/workflows/forge.yml`  
> 2. 通读 Sprint 1–31 全部演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（200+ 条目，0 GAP）  
> 3. **差异化验证**: 对每个方向在 93 份已有分析文档（docs/requirements/）中做全文关键词检索 +  
>    语义比对，确认该方向的核心论点**从未被作为独立系统性方向展开**，或展开角度本质不同。  
> 4. **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、实际影响、边界场景。  
> **日期**: 2026-07-10

---

## 全景判断

ForgeOS 经过 31 轮 sprint 迭代和 93+ 份独立分析文档的覆盖，几乎所有功能域都已被深度扫描。

**已有覆盖全景（本文不重复的域）**：

| 域 | 覆盖度 | 代表文档 |
|---|---|---|
| 功能引擎补齐（编排/路由/记忆/收敛/信号/并行/回灌） | ~30 方向 | `high-value-extension-directions*.md` 等 |
| 生产可靠性（超时/重试/护栏/进程组/自愈/熔断/输出限流） | ~18 方向 | `expansion-production-readiness*.md` 等 |
| 学习闭环（trace/telemetry/scorecard/自适应收敛/知识反哺） | ~15 方向 | `expansion-five-systemic-learning-loop-gaps.md` 等 |
| 安全纵深（secret-scan/递归/预算/SCA/sandbox/readonly 强制） | ~15 方向 | `five-novel-architectural-frontiers*.md` 等 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/漂移守卫） | ~12 方向 | `expansion-five-codelevel-architect-gaps.md` 等 |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | — |
| 结构债（Phase 膨胀/Context Engine/非结构化日志/cmd/forge 包） | ~10 方向 | `architect-product-perspective-four-structural-gaps.md` |
| 执行语义（原子性/幂等/回滚/因果一致性/undo/workspace side-effect） | ~12 方向 | `execution-semantic-gaps.md` |
| 运营可信度（Run Identity/状态隔离/审计/健康检查/自检） | ~8 方向 | `forgeos-trust-operational-maturity.md` |
| 二阶伴生（配置爆炸/知识衰减/TOCTOU/无声数据丢失） | ~10 方向 | `second-order-architectural-gaps.md` |
| 二进制/发布/输出/会话/数据生命周期 | ~5 方向 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` |
| CLI 配置管理/`forge config` | 1 方向 | `five-uncovered-product-frontiers-2026-07-10.md` |

**以下五个方向落在 93+ 份已有分析的间隙中**。它们不是「缺失的组件」，而是 **「产品就从 95% 到 100% 所需的关键桥梁」** —— 每个方向都对应具体的代码级证据，且其核心命题在已有分析中从未被作为独立方向展开。

---

## 方向一 · Forge 二进制生命周期管理（自我治理缺口）

**优先级**: 🔴 P0 | **类别**: 发布工程 · 运维 · 信任 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 定位为「AI-native 软件工厂操作系统」。每个操作系统都需要更新机制。当前，ForgeOS 的治理资产有 `forge-upgrade`（`harness/scaffold/forge-upgrade.mjs`）来同步 70% 的治理模板，但 **Forge 二进制本身没有任何生命周期管理**。

这是一个根本性的自我治理缺口：一个宣称 24h 自治运行的系统，无法自治更新自己。

### 代码级证据

**证据 A: 二进制版本默认为 "dev"**

```go
// forge-core/cmd/forge/main.go:20-24
var forgeVersion = "dev"      // ← 无发布流程时永远 "dev"
var forgeCommit = ""           // ← commit SHA 需手动注入
```

版本和 commit 都通过 `-ldflags` 注入，纯 `go build`（没有 Makefile、没有 goreleaser、没有 CI release job）得到的是 `forgeVersion="dev"`。这意味着：

- `forge version` 在用户环境中永远输出 `dev`，无法追溯二进制来源
- 没有 semver 版本号可供用户做兼容性判断
- 没有 release note 或 changelog 与版本关联

**证据 B: forge-upgrade 只覆盖治理资产，不覆盖二进制**

```
harness/scaffold/forge-upgrade.mjs
  — 文件自身 SCOPE 段诚实标注:
    "upgrade ADDRESSES exactly ONE of two kinds of drift:
     (A) COPIED harness/asset lag → upgrade FIXES this.
     (B) forge-core (Go) BINARY behavior change → upgrade CANNOT fix this."
  — 二进制升级被显式排除在工具范围外
```

用户可以通过 `forge-upgrade` 同步最新的 `acceptance.mjs` 和 `.agent/` 模板，但即使同步了，旧的 `forge` 二进制可能不认识新的 YAML schema 字段。这是一个静默兼容性风险——`forge run` 可能解析失败、静默忽略新字段、或产生未定义行为。

**证据 C: CI 没有发布流水线**

```yaml
# .github/workflows/forge.yml — 完整 CI 文件
# 只有: build → test → race-test → smoke-dry-run
# 没有: goreleaser · 多平台编译 · artifact upload · GitHub Release · 签名
```

发现的全部 CI job：
- `forge accept`
- `go build ./...`
- `go test ./...`
- `go test -race ./...`
- `node --test harness/`
- `forge run build --executor dry`

没有多平台编译（linux/amd64、linux/arm64、darwin/amd64、darwin/arm64、windows/amd64），没有 artifact 发布，没有自动创建 GitHub Release，没有 checksum 签名。

**证据 D: 无版本兼容性检查**

```go
// forge-core/cmd/forge/main.go:356-383 — loadWorkflow 解析 YAML
// 不检查 .agent/workflows/*.yml 的 schema 版本
// 不检查 harness 工具与二进制的版本兼容性
```

```
grep -rn "version.*check\|compat\|schema.*version" forge-core/cmd/forge/ --include="*.go" | grep -v "_test.go"
# → 零匹配
```

二进制和 YAML schema 之间没有版本契约。如果 v2.6 的 `modes.yml` 新增了字段而 v2.5 的 `forge` 不识别，运行时行为取决于 Go 的 YAML 解析器（或 python shim）如何处理未知字段——**静默丢弃**。

**证据 E: `.forge/` 状态文件没有二进制版本戳**

```go
// forge-core/internal/persist/checkpoint.go
// forge-core/internal/trace/trace.go
// forge-core/internal/memory/memory.go
// 三个文件都写入 .forge/ 目录，但无一携带 forgeVersion
```

当用户报告 `.forge/trace.jsonl` 中的可疑数据时，无法回答最基本的取证问题：「这个文件是哪个 forge 版本写的？」

### 实际影响

1. **外部团队无法采用**：没有二进制发布渠道、没有版本号、没有升级路径——安全更新无法分发
2. **版本漂移导致静默错误**：CI runner 的 `forge` 版本和项目期望版本不一致时，无告警
3. **回滚不可能**：`forge-upgrade` 有备份机制，但二进制没有「降级到已知好版本」的路径
4. **审计追溯缺失**：`.forge/trace.jsonl` 记录谁、什么模型、什么成本，但**不记录用什么版本的 forge 运行的**

### 边界场景

- **离线环境**：GitHub Release 不可达，需要内置降级策略（从 USB/NFS 本地更新）
- **版本回退**：新版本引入 bug 后需要 `forge self-update --version=v2.4.0`
- **安全更新**：紧急安全修复需要强制更新机制（类似 `git` 的 `git update` 安全警告）
- **多平台发布**：Go 交叉编译支持 20+ 平台，但 CI 只测 linux/amd64
- **CI runner 版本管理**：CI runner 上的 `forge` 二进制由谁更新、何时更新、如何验证
- **金丝雀发布**：大型团队可能需要 staged rollout（10% 机器先升级）

---

## 方向二 · 工作区状态隔离与 Run Identity

**优先级**: 🔴 P0 | **类别**: 可靠性 · 并发安全 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

`.forge/` 目录是 ForgeOS 的内部状态目录——存储 trace、checkpoint、memory、scorecard。但它是一个**扁平共享命名空间**，没有任何运行隔离机制。

这是一个时间炸弹：一旦有用户/CI 在两个上下文中同时运行 `forge`，状态污染将是静默且不可逆的。

### 代码级证据

**证据 A: `.forge/trace.jsonl` 是单文件追加，无分区键**

```go
// forge-core/internal/trace/trace.go:42-57
// Append 打开 .forge/trace.jsonl，追加一行 JSON，关闭
// 没有 session_id 或 run_id 字段区分不同运行的事件
```

```
trace.Event{Kind:"agent", Phase:"implementer", DurationMs:5000}
trace.Event{Kind:"agent", Phase:"implementer", DurationMs:8000}  // ← 哪个运行的？
trace.Event{Kind:"gate",   Phase:"test",      DurationMs:200}
trace.Event{Kind:"agent", Phase:"implementer", DurationMs:3000}  // ← 第二个运行的混入
```

如果两个 `forge run` 同时执行（或一个 `forge evolve` 的迭代写入与另一个 `forge run` 同时写入），事件流不可分割——无法区分哪些事件属于哪个逻辑运行。

**证据 B: checkpoint 是单一文件**

```go
// forge-core/internal/persist/checkpoint.go:59
// 固定路径 .forge/checkpoint.json
// 没有 run_id 或 workspace_id 前缀
```

如果 `forge evolve --resume` 在另一个运行已覆盖 checkpoint 后执行，恢复的是**错位的状态**——可能跳到错误的 phase。

**证据 C: memory 是单一 JSONL 文件**

```go
// forge-core/internal/memory/memory.go
// 固定路径 .forge/memory.jsonl
// 没有命名空间隔离
```

一个低优先级的探索运行产生的 memory 知识，可能静默污染高优先级的生产运行的 prompt 注入（虽然 `memory_cap` 限制条目数，但不限制来源）。

**证据 D: scorecard 是读写同一文件**

```go
// .agent/routing/scorecards.json — 真实文件
// forge-core/internal/routing/scorecard.go — 读取并更新
// 两个 evolve 同时更新同一文件 → 最后写入者胜 → 数据丢失
```

**证据 E: CI 矩阵作业共享工作区**

```yaml
# .github/workflows/forge.yml — 单作业
# 如果扩展为矩阵（如不同 Go 版本），同一工作区的 .forge/ 会被覆盖
```

### 实际影响

1. **CI 可靠性**：并发 CI job（矩阵构建、并行阶段）静默损坏 trace/checkpoint/memory/scorecard 数据
2. **分支切换污染**：用户 `git checkout feature-a && forge evolve` 然后 `git checkout feature-b && forge evolve`，`.forge/` 的状态交错
3. **调试验证困难**：`forge doctor` 看到的状态是多个运行的交集，不是单一运行的可信快照
4. **审计不可靠**：trace 文件无法作为可信的运行记录，因为不知道哪些事件属于哪个 run

### 边界场景

- **CI 并发**：GitHub Actions 矩阵策略（os: [ubuntu, macos], go: [1.25, 1.26]）可能共享 checkout 目录
- **Git worktree**：用户使用 `git worktree add` 时，多个工作树共享同一个 `.forge/`（在根目录下）
- **`forge evolve` 长时间运行**：一个 evolve 运行 30 分钟，中间另一个 `forge run build` 启动 -> trace 交错
- **`forge resume` 跨运行**：resume A 时 checkpoint 已被运行 B 覆盖 -> 跳到相位混乱
- **NFS 共享工作区**：团队开发环境中多人共享同一工作区
- **Docker volume 共享**：容器化 CI 中 `.forge/` 通过 volume 持久化到宿主机

---

## 方向三 · 错误信息质量与结构化诊断架构

**优先级**: 🟡 P1 | **类别**: 开发者体验 · 可观测性 · 运维 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

ForgeOS 正在从「自用基础设施」走向「外部团队可采用的平台」。当前，`forge run`/`evolve` 的错误输出对于**了解系统的人来说足够用，但对于第一次使用的人来说几乎不可调试**。

这不是「文档不够好」的问题，而是「系统在出错时没有帮助用户理解发生了什么」的问题。

### 代码级证据

**证据 A: 错误格式不一致**

```
// forge-core/cmd/forge/cost.go:66-69 — fmt.Errorf（好，带 %w）
return nil, fmt.Errorf("--run-budget-usd %q is not a number: %w", flagVal, err)

// forge-core/cmd/forge/migrate.go:80-82 — 裸 fmt.Errorf（可接受）
return migrate.Plan{}, fmt.Errorf("--to is required (v1 supports: engineering)")

// forge-core/cmd/forge/main.go:383 — 混合
return asset.Workflow{}, fmt.Errorf("transcoding %s via python shim also failed: %w", ymlPath, execErr)

// forge-core/cmd/forge/engine_build.go:250 — 通过 logln(fmt.Sprintf(...)) 记录（三层嵌套）
logln(fmt.Sprintf("forge: WARNING scorecards unreadable (%v) — continuing...", err))
```

有三种不同的消息构造方式：`fmt.Errorf`（错误值）、`logln(fmt.Sprintf(...))`（文本日志）、以及部分地方直接 `fmt.Printf` 到 os.Stderr。没有统一的路由路径。

**证据 B: 没有结构化输出模式**

```
grep -rn "json.*output\|--json\|format.*json\|output.*format" forge-core/cmd/forge/ --include="*.go" | grep -v "_test.go" | grep -v scorecard | grep -v cost
# → `forge run --json` 或 `forge evolve --json` 不存在
```

没有 `--json` 标志用于机器可消费的输出。CI 集成只能通过解析 stdout/stderr 文本来判断结果是 PASS/FAIL。这是 `five-uncovered-product-frontiers-2026-07-10.md` 方向一「forge config」未覆盖的侧面——输出的机器可读性。

**证据 C: ExecKind 分类优秀但未扩展到包边界之外**

```go
// forge-core/internal/orchestrator/exec_error.go — ExecKind 五分类
// 在 CommandExecutor 层内被一致使用

// forge-core/cmd/forge/ — ExecKind 理论可通过 errors.Is/As 获取
// 但包外没有统一的错误分类模式
```

`KindConfig`、`KindTimeout`、`KindFailed`、`KindRecursionLimit`、`KindOverloaded` 是优秀的分类，但只在 orchestrator 包中使用。`cmd/forge` 层的错误（文件未找到、YAML 解析失败、权限拒绝、预算超限）没有对应的分类。

**证据 D: 没有错误码目录**

```
grep -rn "ErrCode\|ErrorCode\|errCode\|errorCode\|error_code\|errno" forge-core/ --include="*.go" | grep -v "_test.go"
# → 零匹配（除了 ExecKind 的 iota）
```

没有项目级的错误码命名空间。错误只能通过文本 grep 来区分，而不是通过机器可读的代码。

### 实际影响

1. **CI 集成困难**：`forge run` 的 exit code 只有 0（成功）或 1（失败），无法区分「测试失败」（可重试）和「配置错误」（不可重试）
2. **调试体验差**：用户收到 `error: workflow not found`，但不知道哪个 workflow、在哪里找、哪个模式匹配失败
3. **自动化不友好**：工具链（如 Jenkins、Argo Workflows）需要解析文本输出来决定下一步动作
4. **多语言受众不可达**：英文错误消息对非英语用户是额外认知负担

### 边界场景

- **`--json --pretty` 双模式**：机器消费（JSON）和人阅读（彩色文本）需要共存
- **错误聚合**：并行模式中多个 phase 同时失败，错误需要聚合而非最后一个覆盖前面的
- **静默检测**：`forge doctor` 发现多个警告（SCA DB 缺失、适配器未配置），需要结构化的健康检查输出
- **i18n**：错误消息模板化（非硬编码字符串）是未来国际化的前提
- **trace 关联**：错误消息应携带 `run_id` 或 `trace_id`（当方向二的 Run Identity 实现后）以关联 trace 事件

---

## 方向四 · 并行执行的生产就绪度

**优先级**: 🟡 P1 | **类别**: 可靠性 · 性能 · 安全 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

`RunParallel`（`forge-core/internal/orchestrator/parallel.go`）是 ROADMAP 方向五的产物——让无依赖的 phase 可以并发执行。它在架构上有清晰的锁契约和波次调度（Kahn 拓扑排序），但**作为「已交付」功能，它有几个关键的生产就绪度缺口**。

这不是「写更多代码」的问题，而是「验证已写代码在生产环境中的行为是否可预测」的问题。

### 代码级证据

**证据 A: 并行模式明确禁用了定向 loop-back**

```go
// forge-core/internal/orchestrator/parallel.go:37-40
// "NO directed loop-back. Loop-back (...) is a SEQUENTIAL-SPINE feature;
//  a fan-out wave has no single 'back' target. So in parallel mode a red
//  gate ABORTS the run (fail-closed) rather than looping."
```

这意味着如果一个 implementer phase 的测试 gate 失败，串行模式会 loop-back 到 implementer 重试（这是 build.yml 的正常恢复路径），但并行模式直接 abort。用户切换到 `--parallel` 后，丢失了自动恢复能力。

**证据 B: 8 把互斥锁的锁顺序只有文档约束，没有机器执法**

```go
// forge-core/internal/orchestrator/parallel.go:26-33
// ACQUISITION ORDER (from outermost/earliest to innermost/latest):
//  1. trace.Tracer.mu
//  2. runBudget.mu
//  3. loopProbe.mu
//  4. gateLedger.mu
//  5. phaseOutputLedger.mu
//  6. ContextCache.mu
//  7. reviewFindingsLedger.mu
//  8. verdictLedger.mu
```

8 把来自不同包的 mutex，按固定顺序获取——任何违规都会产生 schedule 依赖的死锁（Heisenbug）。但：
- 没有锁顺序校验器（类似 `go test -race` 检测数据竞争，但没有等效工具检测锁顺序违规）
- 没有文档说明每个 mutex 在什么条件下被哪个 goroutine 获取
- 新增的 mutex 需要手动添加到契约中

**证据 C: 没有锁竞争的可观测性**

```go
// forge-core/internal/orchestrator/parallel.go — 零锁竞争指标
// 没有：等待时间、锁持有时间、争用计数
```

当并行 phase 数量增加时（当前只有一个 phase 不需要 fan-out），锁争用会成为性能瓶颈。没有指标就不知道瓶颈在哪。

**证据 D: 没有资源自适应**

```go
// forge-core/internal/orchestrator/parallel.go — RunParallel
// 当前行为：无限制并发
// wave 内的所有 phase 同时启动
```

没有 `--parallel-workers` 限制（当前只使用 `RunParallel` 自己的工作池）。如果工作流声明了 20 个 implementer phase 且无依赖，所有 20 个会同时启动，可能耗尽系统资源（CPU、内存、API 速率限制）。

**证据 E: 并行模式测试覆盖率有缺口**

```bash
# forge-core/internal/orchestrator/parallel_test.go
# 当前测试用例:
TestRunParallel_FanOutRunsConcurrently         # ✓
TestRunParallel_RespectsDependencyWaveOrder    # ✓
TestRunParallel_GateFailureAborts              # ✓ (但是 fail-closed)
TestRunParallel_CycleErrorsBeforeAnyPhase      # ✓
TestRunParallel_AgentBudgetEnforcedConcurrently # ✓
TestRunParallel_ExplorerSkipsReviewStage        # ✓
TestRunParallel_BalancedRunsReviewStage         # ✓
TestRunParallel_ExplorerDoesNotSkipNonReviewNonDiscoverStage # ✓
TestWaves_NoDeps_SingleFanOutWave               # ✓
TestWaves_LinearChain                           # ✓

# 缺少:
# - 混合串行+并行工作流测试（depends_on 部分声明）
# - 高锁争用下的行为测试
# - 资源耗尽（OOM、fd 耗尽）下的行为
# - 与 checkpoint/resume 的交互
```

### 实际影响

1. **可靠性降级**：gate 失败时并行模式直接 abort 而非 loop-back，用户被迫手动重跑
2. **死锁风险**：锁顺序违规产生的死锁只在高负载下偶现——极难复现、极难调试
3. **性能黑盒**：无法判断并行 phase 是真正并发执行还是被锁争用串行化了
4. **资源风险**：大规模 fan-out（如 review.yml 的 4 个 reviewer phase 并行）可能同时打同一个 API

### 边界场景

- **资源枯竭**：16 个 implementer 同时启动 -> CPU 峰值 1600%、OOM killer 介入、`cmd.Run` 被信号终止
- **API 速率限制**：所有 reviewer 同时调 claude API -> 429 风暴 -> `KindOverloaded` 退避 -> 但退避可能重新同步导致 thundering herd
- **部分失败**：wave 中 3 个 phase 成功、1 个失败 -> context 取消其他 phase -> 但已写入磁盘的 side-effect 不可逆
- **Checkpoint + Parallel**：当前不 checkpoint 单个 phase，只 checkpoint iteration。长迭代中并行 phase 完成但未写入 checkpoint -> crash 后重跑整个 iteration
- **Mixed 工作流**：一个工作流中部分 phase 有 `depends_on`、部分没有 -> 混合调度需要同时处理串行和并行路径

---

## 方向五 · 测试基础设施成熟度与确定性

**优先级**: 🟡 P1 | **类别**: 工程质量 · 可持续性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 有令人印象深刻的测试覆盖：77 个 Go 测试文件 vs 63 个生产源文件、699 个测试函数、19 个 harness 测试文件。但**测试数量不等于测试基础设施成熟度**。

当前测试基础设施的主要问题是**模式不统一**：没有共享的测试工具包、没有黄金文件框架、测试环境因文件而异。这不是「缺少测试」的问题，而是「测试本身没有经过充分的设计」。

### 代码级证据

**证据 A: 没有共享测试工具包**

```bash
# forge-core/ 下没有 internal/testutil 或类似包
# 每个包自己实现测试辅助函数，模式各不同
```

```
# cmd/forge/main_test.go — 有 mkdir/writeFile 辅助函数（包内 private）
# internal/memory/memory_test.go — 有自己的 setup 模式
# internal/orchestrator/parallel_test.go — 用 barrierExec 自定义结构体
```

公共模式（如创建临时工作区的 `forge` 可执行文件、设置 YAML 夹具、创建 `.forge/` 目录结构）在每个测试文件中重复实现。这导致：
- 新测试的门槛更高（需要了解包特有的辅助函数）
- 夹具模式不一致（有些用 `t.TempDir()`，有些手动 `os.MkdirTemp`）
- 行为差异（一个包的 `setupRepo` 踩 git 文件，另一个包的 setup 建 YAML）

**证据 B: 黄金文件（golden file）模式缺失**

```go
// forge-core/cmd/forge/prompt_context_test.go — 测试 prompt 渲染
// 没有将渲染结果与预期 golden file 对比
// 当前做法：断言 URL 字符串包含特定片段
```

当一个 prompt 模板的逻辑变化时（例如加了新的 context lane），现有测试可能仍通过（因为只断言特定字符串）。一个 golden file 测试会在 `buildPrompt` 输出变化时产生 diff，让开发者注意到「prompt 发生了变化」。

考虑到 prompt 内容是 ForgeOS 的核心产出——这些文本直接决定了 agent 的行为——没有黄金文件保护意味着 prompt 渲染的任何重构都没有回归检测。

**证据 C: 测试使用真实的、不可控的时间**

```go
// forge-core/internal/memory/memory_test.go:368
now := time.Now().Unix()

// forge-core/internal/orchestrator/parallel_test.go:36
case <-time.After(2 * time.Second):

// forge-core/internal/orchestrator/command_executor_unix_test.go:45
time.Sleep(50 * time.Millisecond)
```

`time.Now()` 在测试中产生不可重复的时间戳。`time.After` 和 `time.Sleep` 在 CI 负载高时导致 flaky 测试（边界过期、超时误判）。虽然 `trace.Now` 和 `orchestrator.Engine.Sleep` 有注入模式，但并非所有时间敏感的代码路径都使用了注入。

**证据 D: 测试状态共享**

```go
// forge-core/internal/gate/gate_test.go:91
t.Setenv(EnvRoot, "/from/env")
// 对使用同一 testing.T 的其他测试可见
// 如果 t.Parallel() 被添加，这可能污染并行测试
```

```go
// 全局变量（如 routing 包的 tier 常量）在测试之间共享
// 没有 reset 模式
```

虽然 Go 测试框架本身不会在包间共享状态，但包内测试可能通过全局变量、环境变量、文件系统路径产生隐式依赖。

**证据 E: Harness 测试在真实工作区运行**

```bash
# harness/test_*.mjs — 所有 19 个测试文件
# 运行在 forge 自己的工作区中，而非临时副本
```

当前工作区的文件结构和 `.forge/` 状态会影响测试结果。测试可能会因为：
- 未提交的文件变化（`gate.mjs` 检查 466 个文件）
- `.forge/` 目录中的遗留状态
- 环境变量（`FORGE_ROOT`、`PATH` 等）

而在 CI 中，每次都是干净 checkout——这就产生了「开发机器测试通过、CI 失败」或反之的 flaky 模式。

### 实际影响

1. **重构阻力**：没有 golden file 保护，prompt 渲染路径的重构风险高于应有水平
2. **新贡献者门槛**：每个包的测试模式不同，新手需要学习 N 种模式才能写测试
3. **CI flakiness**：时间依赖和状态共享的测试在负载变化时间歇失败，降低对 CI 门禁的信任
4. **回退到人工验证**：如果测试套件不被信任，开发者会手动验证——消解了自动化门禁的价值

### 边界场景

- **并行测试执行**：`go test -parallel 8 ./...` 在某些测试中暴露全局状态竞争
- **Git 状态依赖**：`file_delta_test.go` 依赖 `git diff` 输出，如果在 dirty 工作区运行则行为不确定
- **环境差异**：macOS 的 `mktemp` 行为、CI 的 `TMPDIR` 设置、不同 Go 版本的 `t.TempDir` 清理行为
- **测试顺序依赖**：TestA 创建了全局状态，TestB 假设该状态存在——这在一个测试文件中隐式成立，但如果未来测试被重新排序或拆分，就会失败
- **Harness 测试的自我引用**：harness 测试测试自己（例如 `test_forge-init.mjs` 测试 `forge-init.mjs` 能复制自身）——这是强大的自检，但复制后的版本如果因为工作区状态不同而有不同行为，测试会间歇失败

---

## 优先级和实施路线图

| 方向 | 优先级 | 风险 | 依赖 | 建议时机 |
|------|--------|------|------|----------|
| ① 二进制生命周期 | P0 | 外部采用的前提条件 | 无 | **Sprint 32** |
| ② 工作区状态隔离 | P0 | 并发安全：时间炸弹 | Run Identity 设计 | **Sprint 32-33** |
| ③ 错误信息质量 | P1 | 开发者体验债务 | 方向②的 Run Identity | Sprint 34 |
| ④ 并行执行就绪度 | P1 | 长期可靠性 | 方向②的锁顺序可观测性 | Sprint 34-35 |
| ⑤ 测试基础设施 | P1 | 工程可持续发展 | 无 | **与②③④并行** |

### 快速胜利（0.5 sprint 内可完成）

- **⑤ 测试基础设施**: 创建 `internal/testutil` —— 从现有测试中提取公共模式（TempDirWithFixture、MakeFakeWorkflow、CreateForgeStateDir），建立 golden file 框架（`testutil.AssertGolden`，输出差异时更新）
- **③ 错误信息质量**: 新增 `--json` 输出模式到 `forge run`/`forge evolve`；定义首批 10 个错误代码（`E_CONFIG`、`E_TIMEOUT`、`E_BUDGET`、`E_GATE`、`E_WORKFLOW` 等）

### 需要设计的工作

- **① 二进制生命周期**: 需要决定发布渠道策略（stable/beta/nightly）、更新协议（GitHub Releases vs 自建）、兼容性契约（二进制版本必须 >= 生成 YAML 的版本？）
- **② 工作区状态隔离**: 需要设计 Run Identity 格式（UUID v7? `<hostname>-<timestamp>-<random>`?）、隔离策略（目录分区 vs 文件内标记）、兼容性（旧 `.forge/` 目录不加 Run Identity 时如何退化）
- **④ 并行执行就绪度**: 需要设计锁顺序自动验证器（在 `-race` 模式或单独的 lint 中执行）

---

## 与已有分析的关系

| 本文方向 | 最接近的已有分析 | 差异 |
|----------|-----------------|------|
| ① 二进制生命周期 | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | 已有分析讨论二进制签名和 `forge self-update` 命令的 CLI 设计；本文聚焦于**发布工程、版本兼容性契约、CI release pipeline 缺失**的系统性治理缺口 |
| ② 工作区状态隔离 | `forgeos-trust-operational-maturity.md` §「Run Identity」 | 已有分析从运营可信度角度讨论 Run Identity（可追溯性）；本文从**并发安全**和**状态污染**角度分析——两个并行运行的产物混在一起的数据损坏问题 |
| ③ 错误信息质量 | `execution-semantic-gaps.md` §「错误分类」 | 已有分析讨论错误分类的 domain 建模；本文聚焦于**消费端的结构化输出、错误码目录、一致性格式**——不是如何分类错误，而是如何呈现错误 |
| ④ 并行执行就绪度 | `expansion-horizon-three.md` | 已有分析将并行执行作为路线图功能点提及；本文聚焦于**已交付功能的可靠性缺口**——loop-back 不可用、锁顺序不可校验、资源不可自适应 |
| ⑤ 测试基础设施 | `five-genuinely-uncovered-architectural-frontiers-2026-07-10.md` 方向一「Harness 测试夹具化」 | 已有分析讨论 harness 测试的自举风险（测试修改自身源文件）；本文聚焦于**Go 测试的代码模式、确定性、可维护性**——共享工具包、golden file、时间注入 |

---

## 结语

ForgeOS 的技术治理完备度已极高：31 轮 sprint 构建了从编排到路由、从记忆到收敛、从信号到安全护栏的全栈基础设施。`forge accept: ACCEPTED` 在任何改动后都是真实的、多维度验证。

但产品的完整不仅是「功能完备」，也是 **「可以被信任地部署、操作、诊断、升级」**。这五个方向——二进制生命周期、工作区隔离、错误诊断、并行可靠性、测试基础设施——就是通往那个状态的最后几座桥梁。

它们不需要发明新机制（不像 Learning Loop 或 Context Engine 那样需要新的运行时），它们需要的是**把已有的优秀工程实践制度化、统一化、产品化**。
