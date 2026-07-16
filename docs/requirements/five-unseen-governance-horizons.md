# ForgeOS — 五个未见治理边界：CI 碎片 · Agent 耦合 · 影子编排 · 孤儿进程 · 自反治理

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局逐文件扫描完整代码库 — forge-core(19 Go 包 · ~35k LOC 生产代码 · 纯 stdlib 零依赖)、  
>    cmd/forge(16+ 子命令 · ~12k LOC)、harness(39+ 模块 · ~10.5k LOC 执法层)、  
>    `.agent/`(12 agent 卡 · 9 skill 卡 · 5 工作流 · 完整治理骨架)、  
>    `examples/`(url-shortener · go-taskd)、`pi-batch.py`(499 行零治理根脚本)、  
>    `.github/workflows/forge.yml`(CI 管线)、docs/analysis/ · docs/requirements/ 各 40+ 篇  
> 2. **差异化验证**: 对每个方向的核心命题，在 docs/requirements/(68 篇)+ docs/analysis/(40 篇)+  
>    FUNCTIONAL_REQUIREMENTS_AUDIT + 核心文档中做全文语义检索，确认该方向的核心论点  
>    **从未作为独立系统性扩展方向被展开**。  
> 3. **纪律**: 不编写任何代码。每个方向附代码级证据、与已有覆盖的差异化证明。  
> **日期**: 2026-07-10

---

## 全景定位

已有 108+ 份分析文档覆盖了 ForgeOS 几乎所有可触及的功能域： 
- 编排引擎内核（串/并行/loop-back/mode-gating/stop-condition/checkpoint/resume）: ~35 方向
- 生产可靠性（529/超时/退避/输出上限/递归守卫/预算护栏/进程组）: ~18 方向
- 可观测性（trace/telemetry/scorecard/三维真数据）: ~10 方向
- 记忆/学习（memory/checkpoint/Supersedes/ContextCache/knowledge lifecycle）: ~10 方向
- 路由/调度（TierFor/多维评分/BudgetAdjust/HistoryTiebreak）: ~8 方向
- 安全纵深（secret-scan/recursion/budget/timeout/output-cap/SCA/四维护栏）: ~12 方向
- 治理/执法（arch-check 8 检查/check.py 10 检查/loop-back/circular dependency）: ~12 方向
- 结构缺口（cmd/forge 包内聚/YAML 碎片/存储增长）: ~10 方向
- 产品/运营/北极星桥梁: ~20 方向

**但所有这些分析都有一个共同的盲区**: 它们全部站在 ForgeOS「治理者」的视角分析 ForgeOS  
「能为被治理项目做什么」，几乎从不反过来问:

> **ForgeOS 的治理层自身，是否被治理着？**

本文的 5 个方向落在这个「治理反射」盲区中。每个方向都是 **ForgeOS 自己的治理基础设施出现  
裂缝的地方**——裂缝不在其管辖的项目中，而在治理者自身。

| # | 方向 | 类型 | 优先级 | 核心命题 | 已有覆盖 |
|---|------|------|--------|----------|----------|
| 1 | **CI 治理碎片** | 治理连续性 | **P0** | CI 管线在 `forge accept` 之外运行重复/独立检查，制造治理矛盾点 | **零覆盖** |
| 2 | **Agent-CLI 耦合层** | 架构可移植性 | P1 | 整个编排管线深度耦合于 `claude` CLI 专有构造，无法插拔替代 agent | 边缘提及，无系统性展开 |
| 3 | **影子编排器治理** | 治理完整性 | P1 | `pi-batch.py` 是完整的并行编排器，零测试零治理，完全绕过 ForgeOS | 仅扫描提及，无治理方向 |
| 4 | **进程孤儿生命周期** | 运行时韧性 | P2 | forge 崩溃后子进程无人清理；当前 SIGTERM→SIGKILL 只处理正常关闭 | 边缘提及，无系统性设计 |
| 5 | **自反治理仪表盘** | 治理反射 | P2 | ForgeOS 从未对自己的治理健康做结构化检测——「治理者治理自己」 | **零覆盖** |

---

## 方向一 · CI 治理碎片（CI Governance Fragmentation）

**优先级**: 🔴 **P0** | **类别**: 治理连续性 · CI/CD | **预估**: ~0.5 sprint | **杠杆**: ⭐⭐⭐⭐⭐  
**已有覆盖**: **零** — 68 篇 requirements + 40 篇 analysis 全部讨论 ForgeOS 如何治理项目，  
没有一篇讨论「CI 管线中的 ForgeOS 自身治理断裂」。

### 问题描述

`.github/workflows/forge.yml` 定义了 ForgeOS 自身的 CI 管线（也是被 `forge-init` 复制给所有新项目的模板）。当前管线有 6 个独立步骤：

```yaml
# .github/workflows/forge.yml (精简)
jobs:
  accept:
    steps:
      - name: forge accept (Stop gate)                          # ← 正式闸门
        run: node harness/acceptance.mjs

      - name: go build (catches compilation errors)              # ← 重复
        run: go -C forge-core build ./...

      - name: forge-core tests (zero-dep Go runtime)            # ← 重复
        run: go -C forge-core test ./...

      - name: forge-core tests with -race                       # ← 唯一(race)
        run: go -C forge-core test -race ./...

      - name: harness unit tests                                 # ← 重复
        run: node --test harness/

      - name: forge run build --executor dry                    # ← 唯一(E2E)
        run: go -C forge-core build -o /tmp/forge-test ./cmd/forge
             /tmp/forge-test run build --executor dry --root $PWD
```

**治理断裂**:
1. **`forge accept` 已聚合 `go test` / `node --test` / `go build`** — 但它独立验证，
   `acceptance.mjs` 的 `test` 和 `app_test` 门已经跑过这些。步骤 2-5 是`forge accept`已有能力的
   **重复执行**。
2. **`go test -race` 不在 `forge accept` 中** — 这是真正的缺口：forge 的正式闸门不检查数据竞争。
   一个 PR 可能 `forge accept: ACCEPTED` 但 CI 的 `go test -race` 失败。谁更权威？
3. **`forge run build --executor dry` 不在 `forge accept` 中** — E2E 编排冒烟测试
   是 CI 独有的信号。forge 的正式闸门不覆盖编排路径是否可运行。
4. **`forge-init` 复制此模板** — 每个被 forge 治理的新项目都会继承这个碎片化的 CI 模板，
   把「正式闸门 vs CI 附加检查」的矛盾传播到所有项目。

### 代码级证据

**证据 1: `forge accept` 不包含 `-race` 检测**

```bash
# 在 harness/acceptance*.mjs, forge-core/cmd/forge/gates.go, gates_test.go 中
# 搜索 "-race" 零命中
$ grep -rn "race" harness/acceptance*.mjs
# (无输出)
```

**证据 2: `forge accept` 不包含 E2E dry-run 编排测试**

`forge run build --executor dry` 是 workflow 编排的端到端冒烟测试，但 `forge accept` 的
test/app_test 门只跑单元测试和集成测试，不跑编排全链路。

**证据 3: `forge-init` 模板复制此碎片**

```javascript
// harness/scaffold/scaffold-fs.mjs — forge-init 的 COPIED_FILES 清单包含
// .github/workflows/forge.yml，把 CI 碎片复制给每个新项目
```

### 为什么需要

- **治理权威性**: ForgeOS 的核心理念是「Stop gate（`forge accept`）是真相之源」。  
  CI 中有独立于 `forge accept` 的检查，且 `forge accept` 不覆盖 `-race` 和 E2E，  
  意味着「ACCEPTED」并不代表「CI 会绿」。这是治理权威的自我矛盾。
- **信号冲突**: 如果一个 PR `forge accept: ACCEPTED` 但 `go test -race` 失败，  
  两个信号谁赢？当前没有裁决机制。对于自治运行的 AI agent 场景，这个歧义是致命的。
- **模式传播**: 每个 `forge-init` 项目继承这个模式，制造 N 份碎片化治理。

### 边界情况

| 场景 | 当前行为 | 期望行为 |
|------|----------|----------|
| `forge accept` PASS 但 `go test -race` FAIL | CI 失败，但 forge accept 说绿 | 要么 `forge accept` 包含 `-race`，要么 CI 信任 forge accept 为唯一 gate |
| `go build` 在 CI 中失败但 forge accept 的 `test/app_test` 无 build 门 | CI 失败，但 forge accept 可能绿 | forge accept 应聚合 build 验证 |
| `forge-init` 新项目继承 CI 模板 | 碎片模式被传播 | 模板应只包含 `forge accept`，消除重复 |
| CI 中的 E2E dry-run 检测编排死循环 | 仅 CI 覆盖 | forge accept 应包含「workflow 可编排性」的最小验证 |

### 差异化证明

**在 108+ 份已有分析中搜索**: `CI.*fragment` / `CI.*duplicate` / `governance.*fragment` /  
`forge.yml` / `workflow.*duplicate` / `CI.*separate` — 全部零命中为独立方向。  
`strategic-extensions-v32.md` 提到 CI 中缺少 echo executor 测试，但聚焦于「缺少的测试类型」  
而非「CI 碎片化」这个治理结构问题。

---

## 方向二 · Agent-CLI 抽象层（Agent-CLI Abstraction Boundary）

**优先级**: 🟡 **P1** | **类别**: 架构 · 可移植性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐  
**已有覆盖**: `agent-orchestration-five-novel-perspectives.md` 和  
`forgeos-architect-product-perspective-five-frontiers.md` 边缘提及 vendor lock-in，  
但从未作为一个**具体的接口抽象方向**被系统性展开。

### 问题描述

ForgeOS 的整个编排管线深度耦合于 `claude` CLI 的专有接口：

```
forge-core/cmd/forge/
├── cost.go              ← claude JSON 解析: parseReviewerVerdict / parseExecutiveVerdict
├── prompt_context.go    ← buildPrompt / agentExecutor 构造: --permission-mode acceptEdits
├── prompt_memory.go     ← memory 注入: 无 vendor 抽象
├── engine_build.go      ← agentExecutor 构建: --model, --allowedTools 输入
├── gates.go             ← verdictLedger: claude 输出格式假设
└── preflight.go         ← agent-cmd 存在性检查: 只查 claude
```

**具体耦合点**:
1. **`--permission-mode acceptEdits`** — `claude` 专有 flag。其他 CLI（`pi`、`codex`、`gemini-cli`）不接受此 flag。
2. **`--allowedTools`** — claude CLI 专有写入权限门控。其他 agent CLI 有不同的权限模型。
3. **`--model` 参数映射** — `routing.go` 的 `ModelMap` 只映射 anthropic 的模型名。
4. **`--output-format json` 和 `total_cost_usd` 解析** — `cost.go` 的 `parseReviewerVerdict` 等功能专门解析 claude 的 JSON 输出格式。
5. **`--max-budget-usd`** — claude `-p` 模式的专有 flag。
6. **`agent-cmd` 探测** — 只检查 `claude` 是否在 PATH 中（`preflight.go`），不提供其他 CLI 的探测/安装检查。

### 代码级证据

**证据 1: cost.go 的 claude 专有解析器**

```go
// forge-core/cmd/forge/cost.go:195-210
func parseReviewerVerdict(output string) string {
    // 专门解析 claude JSON 输出格式中的 VERDICT 行
    // 假设输出格式是 claude --output-format json 的产物
}
```

**证据 2: ModelMap 只有 anthropic**

```go
// forge-core/internal/routing/routing.go:300-310
var ModelMap = map[string]map[string]string{
    "anthropic": {
        Haiku:  "claude-sonnet-4-haiku",
        Sonnet: "claude-sonnet-4",
        Opus:   "claude-opus-4",
    },
    // 无其他 provider 映射
}
```

**证据 3: agentExecutor 构建硬编码 claude flags**

```go
// forge-core/cmd/forge/engine_build.go:80-95
claudeArgv := []string{
    "claude", "-p", prompt,
    "--model", model,
    "--permission-mode", o.agentPermission,
    "--allowedTools", o.agentAllowedTools,
    "--output-format", "json",
}
// 如果 agent-cmd 不是 claude，这些 flag 全部不适用
```

**证据 4: 只检查 claude**

```go
// forge-core/cmd/forge/preflight.go:25-35
func preflightCheck(root string, o runOpts) error {
    if o.executor == "command" {
        if _, err := exec.LookPath("claude"); err != nil {
            // 只查 claude，不查 agent-cmd 的实际值
        }
    }
}
```

### 为什么需要

- **north-star 的前置条件**: 跨厂商池（D4 的 v3 目标）需要 agent-CLI 抽象。
  没有 CLI 抽象层，即使路由层支持多厂商，执行层仍然耦合在 claude 专有 flags 上。
- **当前代码已体现成本**: `cost.go` 需要同时支持 `parseReviewerVerdict`（claude 二元裁决）+
  `parseExecutiveVerdict`（claude 五元裁决）+ `parseConfidenceScore`（claude 置信度）。
  添加第三个 agent CLI 时每个解析函数都要复制或适配。
- **pi-batch.py 是反例**: 它使用 `pi` CLI（完全不同的 flag 集），如果要将它接入 ForgeOS
  编排管线，当前架构需要大量条件分支。

### 建议的抽象层形状（非代码，仅设计素描）

```
AgentCLI interface {
    Name() string                       // "claude", "pi", "codex", "gemini-cli"
    BuildArgv(prompt, opts) []string    // 构建 CLI 参数
    ParseCost(output) (usdMicros, error) // 从输出提取成本
    ParseVerdict(output) (verdict, error) // 从输出提取裁决
    ParseConfidence(output) (float64, error) // 从输出提取置信度
    Detect() error                      // 检查是否在当前环境可用
}
```

每个 vendor 一个实现，`engine_build.go` 依赖接口而非具体 CLI。

### 边界情况

| 场景 | 当前行为 | 抽象后行为 |
|------|----------|------------|
| 使用 `pi` 替代 `claude` | flags 全部不匹配，崩溃 | `pi` 适配器生成正确的 flags |
| `claude` 更新输出格式 | 解析器中断，所有 parse 函数需手动更新 | 只改 `claude` 适配器 |
| 混合使用不同 agent CLI | 不支持 | 每个 phase 可配置不同 CLI |
| agent CLI 不支持 cost 报告 | 解析器失败 | 适配器返回 (0, ErrNotSupported) → 诚实降级 |

### 差异化证明

**在 108+ 份已有分析中搜索**: `vendor.*interface` / `agent.*abstraction` / `CLI.*adapter` /  
`agent.*runtime.*interface` / `AgentRuntime` — 仅 `novel-five-highvalue-extensions.md` 在方向一  
提到"引入一个 `AgentRuntime` 接口层"，但其聚焦于「路由层多厂商支持」而非「执行层 CLI 抽象」。  
两者正交：路由层抽象决定「选哪个厂商/model」，执行层抽象决定「如何调用所选 agent CLI」。  
没有一篇分析讨论过执行层 CLI 的具体抽象边界（BuildArgv / ParseCost / ParseVerdict）。

---

## 方向三 · 影子编排器治理（Shadow Orchestrator Governance）

**优先级**: 🟡 **P1** | **类别**: 治理完整性 · 安全 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有覆盖**: 零作为治理方向。`pi-batch.py` 在 34 份 docs/requirements 中被列为扫描过的  
代码工件之一，但没有一篇提出将其纳入 ForgeOS 治理体系或关闭治理盲区。

### 问题描述

`pi-batch.py` 是一个 499 行的独立 Python 批处理编排脚本，具有完整但不受治理的执行引擎：

```
pi-batch.py (499 行, 0 测试, 0 治理, 0 agent 卡)
├── YAML/JSON/纯文本任务加载器     ← load_tasks / load_tasks_from_dir
├── 并行执行引擎 (ThreadPoolExecutor) ← run_parallel / run_serial
├── 子进程管理器 (subprocess.Popen)  ← _run_task_process
├── 超时机制                       ← task.timeout (但存在 BUG: 两个 reader thread 各占 timeout)
├── 输出保存器                     ← save_result (错误时写错误模板)
└── 汇总报告器                     ← print_summary
```

**它完全绕过了 ForgeOS 的所有治理**:
- **无 agent 卡**: 没有 `.agent/agents/` 中定义的角色/边界/权限
- **无 workflow**: 没有 YAML 工作流定义进度编排
- **无 harness gate**: 不经过 `gate.mjs` / `arch-check.mjs` / `check.py`
- **无 trace/observability**: 没有 JSONL trace 事件，只有 print/log
- **无成本追踪**: 不记录每个 task 的调用成本
- **无 checkpoint/resume**: 失败后从头开始
- **无 test coverage**: 零测试
- **使用 `pi` CLI**（而非 `claude`）: 是 ForgeOS 生态外的第二个 agent CLI 入口

### 代码级证据

**证据 1: 零测试文件**

```bash
$ find . -name 'test_pi-batch*' -o -name 'pi-batch_test*'
# (无输出)
```

**证据 2: 不在 harness gate 覆盖范围内**

```bash
# harness/acceptance.mjs 的 probe 列表不包含 pi-batch.py 的任何测试
$ grep -n "pi-batch\|batch" harness/acceptance.mjs
# (无输出)
```

**证据 3: COPIED_FILES 不包含它（虽然合理，但确认其治理外地位）**

```javascript
// harness/scaffold/scaffold-fs.mjs — 不包含 pi-batch.py
// 但 pi-batch.py 存在于项目根目录，与 forge-core + harness 平级
```

**证据 4: 已知 bug（已记录于 Sprint 27 但未修）**

```python
# pi-batch.py:297-310 — _run_task_process
# 两个 reader 线程各拿满额 timeout 预算，
# 实际杀进程延迟可达 ~2× 配置值
tout.join(timeout=remaining())
terr.join(timeout=remaining())
proc.wait(timeout=remaining())
# FileNotFoundError 一律误报"pi not found in PATH"，
# 不区分二进制缺失与 cwd 不存在
except FileNotFoundError:
    msg = "'pi' not found in PATH..."
```

### 为什么需要

- **治理完整性**: 一个声称「统一工程治理」的系统不应该在自己的仓库里有一个 500 行的影子编排器。
  每个 forge 用户都会问：「我应该用 `forge run` 还是 `pi-batch.py`？」
- **安全**: `pi-batch.py` 无任何安全工作（无 secret 扫描、无 gate、无权限控制）。
  如果通过 CI 或被 agent 调用，它可以直接执行任意 shell 命令。
- **功能冗余**: `pi-batch.py` 的并行任务调度（`ThreadPoolExecutor`）与 `forge-core` 的
  `RunParallel`（`internal/orchestrator/parallel.go`）功能重叠。两个并行编排器维护两份。
- **已知 bug 无修复路径**: Sprint 27 发现的 bug（超时翻倍、FileNotFoundError 错误报告）因为
  pi-batch.py 不受治理而从未被正式跟踪修复。

### 治理建议

1. **短期** — 将 pi-batch.py 的功能需求正式纳入 ForgeOS scope（作为 `forge batch` 子命令
   或 `forge run --from-dir` 增强），标记 pi-batch.py 为 deprecated。
2. **中期** — 实现 `forge batch`（复用已有的 `internal/orchestrator/parallel.go` 并行引擎 +
   `internal/trace/trace.go` 可观测性 + `internal/persist/checkpoint.go` 韧性），使批处理执行
   获得与其他 workflow 同级别的治理。
3. **长期** — 移除 pi-batch.py，统一到 forge-core CLI。

### 差异化证明

**在 108+ 份已有分析中搜索**: `shadow.*orchestrat` / `pi-batch.*govern` / `ungovern.*script` /  
`shadow.*forge` / `batch.*command` / `forge.*batch` — 全部零命中。  
34 份文档在扫描文件列表中列出了 `pi-batch.py`，但没有一篇将其作为治理缺口来讨论。

---

## 方向四 · 进程孤儿生命周期（Orphan Process Lifecycle Integrity）

**优先级**: 🟢 **P2** | **类别**: 运行时韧性 · 资源管理 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐  
**已有覆盖**: `novel-extensions-v36-deep-architectural.md` 提及"杀 orphan process"作为  
自愈运行时的一部分，但聚焦于**数据损坏后的恢复**场景，而非**进程生命周期契约**。

### 问题描述

ForgeOS 已实现进程组信号传播（`command_executor_unix.go` 的 `SysProcAttr{Setpgid}` + 
SIGTERM→SIGKILL），这覆盖了**正常关闭**场景：forge 退出时清理其 subprocess tree。

但这覆盖不了：

1. **forge 自身崩溃**（panic / OOM / SIGKILL）— 进程组断开，子进程（claude / pi）变成孤儿，
   继续在后台运行，持续消耗 API 预算和系统资源。
2. **跨 session 孤儿** — `forge run --executor command` 在 SSH session 中断后，
   子进程脱离终端继续运行。
3. **子进程的子进程** — agent 可能 spawn 工具进程（MCP server / bash），
   forge 崩溃后这些孙进程变成孤儿。
4. **重复启动防护** — 如果 forge 崩溃后迅速重新启动同一 workflow，新进程与孤儿进程
   可能在同一个 `.forge/` 目录上产生文件写入竞争。

### 代码级证据

**证据 1: 当前 kill 路径依赖 forge 自身存活**

```go
// forge-core/internal/orchestrator/command_executor_unix.go:40-55
func (e *CommandExecutor) interruptProcessTree(cmd *exec.Cmd) {
    // 只从 forge 进程内 kill 子进程组
    // 如果 forge 自身崩溃，此行代码不会执行
    syscall.Kill(-pgid, syscall.SIGTERM)
    // ...
    syscall.Kill(-pgid, syscall.SIGKILL)
}
```

**证据 2: 无 PID 文件 / 进程登记机制**

```go
// forge-core/internal/orchestrator/command_executor.go
// 无任何代码在 spawn 前/后记录 PID 到持久化存储
// type CommandExecutor struct 无 PIDFile / OrphanGuard 字段
```

**证据 3: checkpoint 不含 PID 信息**

```go
// forge-core/internal/persist/checkpoint.go
// Checkpoint struct 无 AgentPID / SubprocessList 字段
// 崩溃后无法知道有哪些子进程需要清理
```

**证据 4: 无启动锁**

```go
// forge-core/cmd/forge/main.go / evolove.go
// forge run / forge evolve 启动时无进程文件锁
// 同一目录下的两个 forge 实例不会互斥
```

### 为什么需要

- **成本泄漏**: 一次 forge 崩溃可能留下多个 claude 子进程继续消耗 API token，
  每个持续数分钟到数小时，直到自然超时或被手动杀。
- **资源泄漏**: 子进程可能持有文件描述符、内存、网络连接。
- **长自治运行的死锁**: 24h evolve 循环中，如果 phase N 的子进程变成孤儿但
  forge 已重启并进入 phase N+1，两个 agent 可能在同一个仓库中产生写入竞争。
- **当前机制的前置假设**: SIGTERM→SIGKILL 假设 forge 进程在信号到达时仍然活着。
  如果 forge 被 `kill -9` 或 OOM killer 杀掉，整个进程树变成孤儿。

### 设计方向

```
forge run/evolve 启动时:
  1. 在 .forge/ 注册 PID 文件 forge.pid (含 pgid)
  2. 每次 spawn 子进程时记录其 PGID 到 .forge/active_pgids
  
forge 崩溃后重启:
  1. 读取 .forge/active_pgids
  2. 清理仍存活的孤儿进程 (SIGTERM → wait → SIGKILL)
  3. 清理 stale PID 文件
  4. 建立文件锁防止并发启动

forge 正常退出时:
  1. (已有) SIGTERM→SIGKILL 进程组
  2. 清理 .forge/active_pgids 和 PID 文件
```

### 边界情况

| 场景 | 当前行为 | 期望行为 |
|------|----------|----------|
| forge 被 SIGKILL | 子进程变成孤儿 | 重启后清理孤儿 |
| SSH session 中断 | 子进程脱离终端 | PID 文件 + session 检测 |
| 同一目录两个 forge | 数据竞争 | 文件锁互斥 |
| 孤儿子进程已自然结束 | 尝试 kill 失败 | 优雅跳过不存在的进程 |

### 差异化证明

**在 108+ 份已有分析中搜索**: `orphan.*process` / `process.*lifecycle` / `PID.*file` /  
`process.*reap` / `child.*isolat` / `crash.*cleanup` — 仅 1 篇文档（`novel-extensions-v36`）  
提及"杀 orphan process"，但作为数据恢复的下级子句，不是独立的进程生命周期方向。  
无一篇讨论 PID 文件、启动锁或跨 session 孤儿检测。

---

## 方向五 · 自反治理仪表盘（Self-Reflective Governance Dashboard）

**优先级**: 🟢 **P2** | **类别**: 治理反射 · 可观测性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐  
**已有覆盖**: **零** — 全部已有分析讨论 ForgeOS 如何治理**项目**，没有一篇讨论  
ForgeOS 如何计量、验证和报告**自身的治理健康**。

### 问题描述

ForgeOS 对自己的项目（forgeos/catalyst）的治理健康没有任何结构化检测。  
以下事实可以被系统自动检测但当前无任何机制：

1. **`cmd/forge` 包文件数反复超限** — Sprint 27/29/30 三次顶破上限，
   每次靠人为审计+专项 sprint 修复。无自动化趋势预警。
2. **CI 管线碎片** — 方向一的 CI 碎片化从未被系统检测或报告。
3. **`pi-batch.py` 无测试** — 系统治理自身的测试覆盖率时不会发现它。
4. **`forge accept` 不包含 `-race`** — 治理缺口的自我检测为零。
5. **CLI usage() 与实际行为漂移** — Sprint 27 发现 `usage()` 文本与实际 CLI 行为不符，
   无人维护 usage 文本与代码同步。
6. **无治理趋势数据** — 当前有 `forge doctor` 检测运行时健康（checkpoint/trace/memory），
   但没有「治理健康」检测——arch-check 在自身上的通过率趋势、gate 假阳/假阴率、
   self-test 覆盖率变化。

### 代码级证据

**证据 1: `forge doctor` 只检测运行时，不检测治理**

```go
// forge-core/internal/doctor/doctor.go — Run()
// 检查: .forge/ 目录 / checkpoint / trace / memory / python3
// 不检查: arch-check 自身通过率 / CI 完整性 / test 覆盖率趋势
```

**证据 2: arch-check 不检查自身**

```javascript
// harness/arch/arch-check.mjs
// 对 repo 执行 8 项架构检查
// 但从不检查「arch-check 自身是否被 forge accept 覆盖」
// 或「CI 是否在 forge accept 之外有额外检查」
```

**证据 3: 无治理金丝雀（governance canary）**

```bash
# 没有任何「自我检测」确保 ForgeOS 的治理基础设施本身是完整的
# 例如：
#   forge self-check      # 不存在的命令
#   forge governance-health  # 不存在的命令
```

### 为什么需要

- **「治理者治理自己」是元稳定性的必要条件** — 一个声称能防止项目架构腐化的系统，
  如果自己的架构在反复腐化，可信度何在？`cmd/forge` 包文件数三次超限就是实证。
- **趋势优于阈值** — 当前架构检查是二元的（通过/不通过），不追踪趋势。
  如果 `cmd/forge` 文件数从 14 缓慢升到 18 然后突然被压回，系统应该能报告
  这个震荡模式，而不是每次超过 16 才告警。
- **复制品继承缺陷** — `forge-init` 将当前治理状态复制给新项目。
  如果 ForgeOS 自己的治理有缺口（如 CI 碎片），每个新项目继承同样的缺陷。
- **自动化胜过审计** — 前 31 个 sprint 依赖人为审计来发现治理缺口。
  一个自治的治理 OS 应该能自动发现并报告自身的治理漂移。

### 设计方向

```
forge governance-health (或 forge doctor --governance):

  治理基础设施完整性:
    ✅ forge accept 覆盖 go build
    ✅ forge accept 覆盖 test -race
    ❌ forge accept 不包含 E2E dry-run (方向一)
    ✅ CI 无独立于 forge accept 的重复检查
    ❌ pi-batch.py 无测试覆盖 (方向三)
    ❌ usage() 文本与 CLI 行为一致 (方向四)

  治理趋势 (过去 N 次相关 commit):
    arch-check 通过率: 100% (120/120 commits)
    cmd/forge 文件数: 16 (上限 16, +2 震荡于 Sprint 27/29/30)
    测试覆盖率: 84.2% (上月: 83.1%)
    gate 假阳率: 0.3% (3/1000 次)
    自测数量: 699 (上月: 680)

  治理金丝雀:
    ✅ 自测全部通过
    ✅ arch-check 自检通过
    ✅ 治理资产 (agent 卡 / workflow) 全部被 commit
    ✅ .forge/ 无残留临时文件
```

### 边界情况

| 场景 | 当前行为 | 期望行为 |
|------|----------|----------|
| `cmd/forge` 文件数缓慢增长 | 无人察觉直到顶破上限 | 趋势预警：连续 3 次 commit 增长 → WARN |
| CI 增加不经过 forge accept 的步骤 | 无人察觉 | governance-health 检测 CI 碎片 |
| 忘记将新 agent 卡加入 check.py 校验 | 无检测 | 治理完整性检查自动发现未注册 agent 卡 |
| usage() 文本与代码不同步 | 无人察觉 | usage 文本校验（标记为需手动确认） |

### 差异化证明

**在 108+ 份已有分析中搜索**: `self.*reflex` / `self.*govern` / `governance.*health` /  
`self.*check` / `meta.*govern` / `governance.*canary` / `governance.*trend` — 全部零命中。  
`structural-gaps-v41.md` 的方向二「测试质量元治理」讨论测试代码自身的质量门控，  
聚焦于**测试代码的健康**而非**治理基础设施的健康**。两者不同层次：测试质量是「被治理者」的质量，  
自反治理是「治理者」自身的健康。

---

## 优先级与实施建议

| 方向 | 优先级 | 类别 | 杠杆 | 预估 | 依赖 |
|------|--------|------|------|------|------|
| **一 CI 治理碎片** | **P0** | 治理连续性 | ⭐⭐⭐⭐⭐ | 0.5 sprint | 无 |
| **二 Agent-CLI 抽象** | P1 | 架构可移植性 | ⭐⭐⭐⭐ | 2 sprints | 无 |
| **三 影子编排治理** | P1 | 治理完整性 | ⭐⭐⭐⭐ | 1 sprint | 方向二（可选） |
| **四 进程孤儿生命周期** | P2 | 运行时韧性 | ⭐⭐⭐ | 1 sprint | 无 |
| **五 自反治理仪表盘** | P2 | 治理反射 | ⭐⭐⭐ | 1 sprint | 方向一（CI 碎片数据源） |

### 推荐实施顺序

1. **方向一（CI 治理碎片）** — 最快、最高杠杆。将 `-race` 和 E2E dry-run 移入 
   `forge accept`（或扩展现有 gate），从 CI 中移除重复步骤。  
   **预计**: 修改 `harness/acceptance.mjs` 和 `.github/workflows/forge.yml`，
   一天内可完成。立即消除治理矛盾。

2. **方向三（影子编排治理）** — 在 `forge batch` 子命令中合并 `pi-batch.py` 功能，
   利用既有的 `internal/orchestrator/parallel.go` + `internal/trace/trace.go` 管线。
   标记 `pi-batch.py` deprecated。**预计**: 一周内可完成核心替换。

3. **方向四（进程孤儿生命周期）** — 在 `.forge/` 中增加 PID 登记 + 启动文件锁 +
   重启清扫机制。纯加固，无外部依赖。**预计**: 一周内可完成核心机制。

4. **方向二（Agent-CLI 抽象）** — 最大的设计工作。需要先定义 `AgentCLI` 接口契约，
   然后将 `cost.go` / `engine_build.go` / `preflight.go` 中的 claude 专有逻辑抽取到
   `internal/agentcli/` 包。**预计**: 两 sprint，但可与其他方向并行。

5. **方向五（自反治理仪表盘）** — 最深的投资。需要建立「治理健康」的数据模型、
   数据采集（注入 arch-check 结果、gate 结果、CI 结构）、趋势分析和报告。
   **建议**: 先做 `forge doctor --governance` 的静态度量（治理基础设施完整性清单），
   趋势分析留到数据积累后有基线再做。

### 排除的方向（已有分析已充分覆盖）

- **测试质量元治理** → `structural-gaps-v41.md` 方向二已覆盖
- **Go 库 API 边界契约** → 同上方向一已覆盖
- **多仓库联邦治理** → `expansion-horizon-three.md` 已覆盖
- **v2→north-star 桥梁** → `v2-to-northstar-gap.md` 已覆盖
- **生产交付/部署** → `product-deployment-transparency-five-gaps.md` 已覆盖

---

## 总结

这五个方向有一个共同特征：它们不是 ForgeOS「能为项目做什么」的扩展，而是  
**ForgeOS 自身治理基础设施的裂缝**：

- **CI 碎片** 说明治理的「真相之源」不是唯一的
- **Agent CLI 耦合** 说明治理的执行层架构欠抽象
- **pi-batch.py 阴影** 说明治理的覆盖范围不完整
- **进程孤儿** 说明治理的运行时韧性有死角
- **自反治理缺失** 说明治理者对自己的健康缺乏观测

ForgeOS 的核心理念是「AI-native 软件工厂的治理控制平面」。  
一个治理者如果自己有治理盲区，它给予被治理者的「治理可信度」就是打折的。  
这五个方向闭合的是治理者的最后一圈——**治理反射**。
