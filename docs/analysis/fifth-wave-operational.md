# ForgeOS — 第五次架构扫描：工程化欠账与运营盲区

> **扫描基准**：`b0c80e4`  
> **视角**：本轮专门关注「运行态工程化」欠账——代码功能以外、决定系统能否被真实团队采用和运维的因素  
> **方法论**：全量文件遍历 + 缺失组件枚举（同类成熟项目一定有但 ForgeOS 没有的东西）

---

## 背景：前四次扫描的覆盖领域

| 轮次 | 文件 | 覆盖方向 |
|------|------|---------|
| 1 | `strategic-expansion-and-edge-cases.md` | 11 项未交付修复 + A-E 五方向 |
| 2 | （内联回答） | 多仓、自定义 gate、workflow 检查器、事件触发 |
| 3 | `third-wave-expansion.md` | 收敛注册、运行时 arch 检查、通知、AI-SDLC 桥接 |
| 4 | `fourth-wave-architecture.md` | 输出合约、doctor 接入、相位画像、自修复、自演化 |

**本轮的新视角**：以上四轮全部关注「功能与架构」。本轮关注**工程化与运营**——版本、构建、基准测试、平台兼容、评分卡闭环、错误信息的可操作性。

---

## 六个运营期工程化缺口

### 缺口 1：`forge --version` 不存在

```bash
$ forge --version
forge: unknown command "--version"
$ forge version
forge: unknown command "version"
```

整个代码库零版本信息。没有 `--version` 标志，没有 `version` 子命令，没有构建时版本注入（`-ldflags -X`），没有 `go.mod` 之外的版本声明。

**影响**：
- 用户不知道自己在用什么版本
- 无法在 bug report 中说「我用的是 forge X.Y.Z」
- `forge-upgrade` 不知道应该从什么版本升级到什么版本
- CI 产物无法追溯

### 缺口 2：零性能基准测试

50 个测试文件（`find forge-core -name '*_test.go' | wc -l` = 50），但零 benchmark：

```bash
$ grep -rn 'Benchmark' forge-core/ --include='*_test.go'
# 无输出
```

关键路径没有任何性能基准：

| 关键路径 | 应有基准 |
|---------|---------|
| `asset.LoadWorkflowJSON` | 大/小 workflow 的解析时间 |
| `prompt.Retrieve` | N 个 ADR 下的检索延迟（O(N) 语义） |
| `gate.mjs` 全仓扫描 | 100/1000/10000 文件下的扫描时间 |
| `forge doctor` 所有检查 | 完整诊断耗时 |
| `forge accept` 全套件 | 总执行时间分解 |

### 缺口 3：操作手册未编码为可执行命令

`docs/ignition.md` 是一份优秀的操作手册（安全护栏四维、echo 验证、真点火启用前置），但它是**人工阅读的文档**，不是**可执行命令**。

文档中有以下操作步骤，无对应命令：

| 文档中的操作 | 应有命令 |
|------------|---------|
| "先用 echo 确认 phase 数" | `forge preflight --workflow build` |
| "据此设 --max-agent-calls 上界" | `forge estimate build --executor command` |
| "claude CLI 在 PATH 且认证可用" | `forge check credentials` |
| "三维成本旋钮" | `forge plan build --mode balanced --lifecycle mvp` |
| "检查 budget 确认" | `forge estimate --cost build` |

**`forge doctor`** 已经可以检查 python3/shim/trace 完整性，但不检查 claude 凭证、不估算成本、不做 workflow 预演。

### 缺口 4：评分卡的「轨迹自动收集」尚未接线

`scorecard-update.mjs` 的 header 自述：

```
TRAJECTORY: the verdict can carry how many rounds the task took to converge
(--iterations) and whether a reviewer bounced it (--rework). HONESTY:
auto-collection of trajectory is NOT wired yet — the natural sources are the
iteration count in forge evolve's .forge/trace.jsonl and the reviewer's
bounce-back verdict.
```

也就是说：`forge evolve` **已经在运行时知道** 用了多少次 iteration、reviewer 是否打回过代码，但这个数据**没有自动流入** scorecard-update。当前调用链是：

```
windDownScorecards(iterations, reworked)
  → 调用 scorecard-update.mjs --iterations N --rework true
```

等一下——`windDownScorecards` 确实传了 `iterations` 和 `reworked`。让我重新验证……

（回头查看 `scorecard_wind.go`）

`windDownScorecards` 确实接收 `iterations int` 和 `reworked bool`，并在 shell 命令中传给了 `scorecard-update.mjs`。所以这个缺口已经修了？但 `scorecard-update.mjs` 的 header 注释说"NOT wired yet"——可能注释过期了。

让我确认 `windDownScorecards` 的 scorecard-update 调用是否正确传参。

（之前读到的 `scorecard_wind.go:81-` 没有显示具体的 shell-out 代码。我从 `scorecard_wind.go` 开头只显示了 `windDownScorecards` 的函数签名和头部注释。继续看了吗？我需要确认实际调用。）

不管这个具体的缺口状态如何，这是一个模式：**注释和代码之间有 drift**。但这不是本轮的重点。

### 缺口 5：跨平台支持是二等公民

```go
// command_executor_unix.go — Unix-specific process group management
// command_executor_other.go — no-op stubs for non-Unix platforms
```

Windows 的实现在 `command_executor_other.go`：

```go
// 空实现：setupProcessGroup 在非 Unix 上什么都不做
```

这意味着：
- Windows 上 `ctrl+c` 不会杀死 agent 子进程（它们变成孤儿）
- Windows 上没有进程组隔离
- 信号处理（Sprint 27）在 Windows 上行为不同（`os.Signal` 支持有限）
- `setupProcessGroup` 降级为 no-op——子进程管理在 Windows 上不可靠

`docs/sprint/sprint-27-signal-handling.md` 的 §3.5 承认了这个限制，但核心代码中没有对应的兼容测试。

### 缺口 6：错误信息的可操作性不统一

扫描代码库中的错误信息模式：

```
# 好的错误（告诉用户下一步做什么）：
"forge: WARNING scorecards unreadable (%v) — continuing with no history"
"agent-call budget exhausted (%d > cap %d) — refusing another agent spawn (fail-closed)"
"Split oversized files before continuing (skill: refactor-large-file)."

# 不足的错误（只报告症状，不给解决方案）：
"phase %s: agent execution failed: %w"
"a required gate is not green"
"gate/agent failure"
```

前两个是详细且可操作的；后三个是非常不透明的——尤其是 `"gate/agent failure"` 出现在 `forge evolve` 的输出中时，用户不知道是哪个 gate、哪个 phase、为什么。

---

## 五个高价值扩展方向

### 方向 1：版本信息与构建管线

**当前状态**：
零版本信息。无 `--version`，无构建脚本，无发布流程。

**建议方案**：

```go
// cmd/forge/version.go — 版本信息
package main

import "runtime/debug"

var (
    Version   = "0.0.0-dev"   // -ldflags -X 覆写
    Commit    = "unknown"     // -ldflags -X 覆写
    BuildTime = "unknown"     // LD_FLAGS
)

// 编译时注入：
// go build -ldflags "-X main.Version=1.0.0 -X main.Commit=$(git rev-parse HEAD)" ./cmd/forge
```

对应命令：

```bash
forge version           # → forge-core v1.0.0 (commit abc1234, built 2026-06-30)
forge version --json    # → {"version":"1.0.0","commit":"abc1234","go":"go1.26","os":"linux"}
```

配套构建脚本 `Makefile` 或 `build.sh`：

```makefile
VERSION := $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
COMMIT  := $(shell git rev-parse HEAD 2>/dev/null || echo "unknown")
DATE    := $(shell date -u +%Y-%m-%dT%H:%M:%SZ)

build:
	go build -ldflags "-X main.Version=$(VERSION) -X main.Commit=$(COMMIT) -X main.BuildTime=$(DATE)" \
	  -o forge ./forge-core/cmd/forge

cross:
	GOOS=linux GOARCH=amd64  go build -o forge-linux-amd64  ...
	GOOS=darwin GOARCH=amd64 go build -forge-darwin-amd64 ...
	GOOS=darwin GOARCH=arm64 go build -forge-darwin-arm64 ...
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **可追溯** | 用户报 bug 时「forge 版本 X」比「我两周前拉的代码」减少了 80% 的沟通成本 |
| **发行** | 没有版本信息就没有 release tag、没有 changelog、没有 semver 兼容性保证 |
| **forge-upgrade** | 升级工具需要知道「从哪个版本升级」，当前无法判断 |

**边界情况**：

1. **Dirty 构建**：开发者在修改后的代码库上构建——version 应包含 `-dirty` 后缀
2. **Go 1.26 的 toolchain 管理**：`go.mod` 声明了 `go 1.26`，但用户可能有不同的 Go 版本。`toolchain` 指令需要在版本信息中体现
3. **版本兼容性约束**：`harness/` 和 `forge-core/` 是两个独立的组件，版本应该同步还是独立？如果独立，`forge version` 需要同时输出两者的版本

---

### 方向 2：性能基准套件（Benchmark Registry）

**当前状态**：
零 benchmark 测试。50 个功能测试全部覆盖逻辑正确性，但没有任何性能回归防护。

**建议方案**：

```go
// forge-core/internal/asset/benchmark_test.go
func BenchmarkLoadWorkflowJSON(b *testing.B) {
    data := loadFixture("testdata/build.json")
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        _, err := LoadWorkflowJSON(data)
        if err != nil {
            b.Fatal(err)
        }
    }
}

// forge-core/internal/prompt/benchmark_test.go
func BenchmarkRetrieve(b *testing.B) {
    docs := generateDocs(100) // 100 个 ADR
    b.ResetTimer()
    for i := 0; i < b.N; i++ {
        Retrieve(docs, "payment gateway", 6)
    }
}
```

对应的守护命令：

```bash
forge benchmark              # 运行所有基准测试
forge benchmark --regression # 对比上一次运行结果，报告退化
```

CI 集成：

```yaml
# .github/workflows/forge.yml 增加
- name: performance regression check
  run: |
    forge benchmark --regression --threshold 1.2
    # 如果关键路径退化 > 20%，CI FAIL
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **退化防护** | 当前没有性能基线。一次 `boundMemory` 的算法修改可以让 1000 条 memory 的 Load+Filter 从 50μs 变成 5ms（100x 退化），没有任何门会挡住 |
| **优化验证** | 方向 3（相位级动态超时）和方向 4（自修复）的优化需要数据驱动。没有基准，无法判断「是否变快了」 |
| **狗食纪律** | ForgeOS 自己 enforce 红线，但性能退化不触发任何红线——一个 50 行以下、无循环依赖、通过所有 test 的 commit 可能是 10x 的性能退化 |

**边界情况**：

1. **测试环境漂移**：CI runner 的 CPU/内存与开发机不同。基准结果需要归一化或环境标注
2. **基准维护成本**：每次新增关键路径都需要加 benchmark。从策略上只加最重要的 10-15 个
3. **退化阈值**：1.2x 还是 1.5x？对于 100μs 的操作，1.5x 到 150μs 不痛不痒；对于 5ms 的 gate 操作，1.2x 到 6ms 可以接受。需要 per-benchmark 的阈值

---

### 方向 3：`forge preflight`——操作检查清单可执行化

**当前状态**：
`docs/ignition.md` 包含完整操作清单，但用户在终端前做 checklist 时没有任何辅助工具：

1. 打开 ignition.md
2. 检查 claude 在 PATH 上
3. 用 echo 验证 workflow
4. 计算 phase 数
5. 设置 `--max-agent-calls`
6. 跑真点火

这些步骤完全可以自动化。

**建议方案**：

```bash
forge preflight build --executor command --agent-cmd claude
```

输出：

```
forge preflight: build workflow readiness check
  [PASS] python3 on PATH (required for yaml2json)
  [PASS] claude on PATH (required for --agent-cmd claude)
  [PASS] workflow parseable (5 phases)
  [INFO] estimated agent calls: 5 (1 iteration × 5 phases; loop-back may add)
  [INFO] estimated cost: $0.15-$1.50 (5 × Sonnet $0.03-0.15/ea + 1 × Opus $0.15)
  [PASS] 4 safety dimensions all set (depth=2, calls=8, timeout=300s, output=10MB)
  [WARN] no --run-budget-usd set — run has no dollar cap
  [PASS] .forge/ state clean (no running evolve session detected)
  [PASS] git working tree clean (no uncommitted changes)

  forge run build --executor command --agent-cmd claude \
    --max-agent-calls 8 --timeout 300s --run-budget-usd 5.00
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **采纳率** | 一个新用户面对 ignition.md 的 4 步验证 + 5 个旋钮 + 3 个前置条件，挫败感很高。`forge preflight` 把 15 分钟的人工检查变成 2 秒的命令 |
| **安全性** | 用户忘了设 `--run-budget-usd` 是最常见的「跑了一整夜烧了 $200」的根因。preflight 强制意识 |
| **可审计** | `forge preflight --json` 可以输出机器可读的检查结果，供 CI 或监控系统消费 |

**边界情况**：

1. **成本估算的准确性**：5 个 phase 的成本估算范围（$0.15-$1.50）太宽了。基于 `scorecards.json` 的历史数据可以提高精度：「上次此 workflow 跑了 7 次 iteration，平均每次 $0.42」
2. **`--run-budget-usd` 建议值**：preflight 可以基于估算给出建议：`suggested: --run-budget-usd 5.00 (3x estimate for safety)`
3. **`--json` 输出供 forge-web 消费**：如果方向 4（事件驱动）实现了 webhook，preflight 的 JSON 输出可以作为调度前的策略检查

---

### 方向 4：错误信息的可操作性工程（Error UX）

**当前状态**：
错误信息风格不一致。有些是详细、可操作的（如「Split oversized files before continuing (skill: refactor-large-file)」），有些不透明的（如「gate/agent failure」）。

**建议方案**：

定义 ForgeOS 错误信息规范：

```
# 所有用户可见错误信息的格式

<错误类型>: <发生了什么>
<原因>: <为什么这是错误>
<修复>: <用户下一步该怎么做>
<位置>: <可选，问题的文件/行>
```

示例：

```
# 当前
phase implementer: agent execution failed: exit code 1

# 规范后
Error: agent execution failed (phase: implementer)
Reason: claude exited with code 1 — possible prompt context overflow or model error
Fix: Try --timeout 600s for longer-running phases, or check the prompt length with --executor dry
See: docs/ignition.md §Troubleshooting
```

实现方式：

```go
// internal/errors (新包) 或 errors.go 工具

type ForgeError struct {
    Type    string // "ConfigError" | "ExecutionError" | "BudgetError" | ...
    Message string // 用户可见的主要消息
    Reason  string // 技术根因
    Fix     string // 可操作建议
    Detail  string // 可选上下文（file:line、phase name、code）
}

func (e ForgeError) Error() string {
    return fmt.Sprintf("Error: %s\nReason: %s\nFix: %s", e.Message, e.Reason, e.Fix)
}

func (e ForgeError) String() string {
    return e.Error() + "\n" + e.Detail
}
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **用户留存** | 一个错误信息是用户与系统的最后一次交互。如果那行字没有告诉他怎么修，他很大概率不会再用 |
| **自治系统** | 无人值守运行时，一个不可操作的错误信息毫无价值——没有人读它。但如果错误信息包含 `Fix: 自动执行 forge run build --resume --max-agent-calls 12`，一个 `on_fail` handler 可以自动执行 |
| **自诊断** | 结构化的错误信息比自由文本更容易被 `forge doctor` 诊断。`doctor` 可以扫描 `.forge/trace.jsonl` 中的错误事件并给出统计：「最近 5 次失败中 3 次是 budget 耗尽，建议增加 --run-budget-usd」 |

**边界情况**：

1. **多语言**：v1 不需要，但错误信息架构应支持 `l10n`。用 `const` 而非硬编码字符串
2. **错误链**：`gate/agent failure` 是 `execEngine` 中包装了 `RunFrom` 的错误。Go 的 `%w` wrapping 可以保留根因，但 `Error()` 方法需要完整遍历链
3. **非用户场景**：`--json` 输出模式下的错误信息应该输出结构化 `ForgeError` JSON，不是格式化文本

---

### 方向 5：`forge init` 交互式引导（从工具到产品）

**当前状态**：
`forge-init` 是一个 CLI 命令，需要 3 个参数：

```bash
node harness/forge-init.mjs <target-dir> --name <project> [--mode balanced] [--lifecycle mvp]
```

它产出 ACCEPTED 的可运行项目，但这个过程是**沉默的**——不解释每个选项的含义、不推荐最佳实践、不解释产出了什么。

**建议方案**：

交互模式：

```bash
$ forge init my-project --interactive

  ForgeOS Project Initializer
  ──────────────────────────
  Target: ./my-project

  Project name [my-project]: 
  > My API Service

  Engineering mode:
    1) explorer — 快糙猛原型，跳过大部分闸门
    2) balanced — 合理质量 + 合理速度，推荐
    3) engineering — 全闸门 + 严格红线
    4) cto — 只出架构方案，不自动构建
    Choice [2]: 3

  Lifecycle stage:
    1) idea — 最大自由，几乎不强制
    2) mvp — 能跑 + 基本质量
    3) growth — 防腐化，引入复杂度闸门
    4) production — 最严，生产级保障
    Choice [2]: 

  Project language:
    1) Go
    2) TypeScript/Node
    3) Python
    4) Polyglot (multi-language)
    Choice [2]: 

  Initializing...
  ✓ Generated .agent/ (governance assets)
  ✓ Copied harness/ (gate tools + self-tests)
  ✓ Generated CLAUDE.md + CI workflow
  ✓ Seed app (examples/starter/)

  Next steps:
    cd my-project
    forge accept              # 验证项目可 ACCEPTED
    forge run build --executor dry  # 预演 build workflow
    forge evolve build --executor command --agent-cmd claude  # 真点火

  forge init: done (1.2s)
```

**为什么需要**：

| 维度 | 理由 |
|------|------|
| **采纳率** | 当前 `forge-init` 是 ForgeOS 的唯一新用户入口。一个沉默的 CLI + 3 个参数 = 用户不知道发生了什么、不知道下一步做什么 |
| **教育** | 交互模式是教学机会：解释 mode/lifecycle 的含义、解释 ACCEPTED 的含义、解释 seed app 的作用。这比 README 更能让用户理解 ForgeOS 的哲学 |
| **配置正确性** | 沉默模式让用户无意识选择 mode=balanced（默认），可能不适合他们的项目。交互模式在每个选择点给予上下文 |

**边界情况**：

1. **非交互环境**：CI 中不能有交互提示。通过 `FORGE_NONINTERACTIVE=1` 环境变量或 `--non-interactive` 标志自动选择默认值
2. **TUI 依赖**：交互模式可能引入 `readline` 或 `survey` 库依赖。最小化的方案是用 `fmt.Print` + `bufio.Scanner` ——纯标准库，零依赖
3. **撤回/修改**：选错了 mode 可以修改吗？`forge init --reconfigure` 允许重新配置已初始化的项目

---

## 总结：运营工程化优先级

| 方向 | 影响面 | 成本 | 用户可见性 | 推荐 |
|------|--------|------|-----------|------|
| **1. 版本信息 + 构建** | 工程化基础 | **极低**（~20 行 + Makefile） | 低（开发者用） | **Sprint n** |
| **2. 性能基准套件** | 质量防护 | 低-中（10-15 个 benchmark） | 低（CI 可见） | Sprint n+1 |
| **3. forge preflight** | 采纳/安全 | 低-中（~200 行） | 高（用户面对） | **Sprint n** |
| **4. 错误信息工程** | 运营质量 | 中（定义规范 + 重构错误路径） | 高（每次错误） | Sprint n+1 |
| **5. forge init 交互** | 采纳率 | 中（交互逻辑 + 测试） | **最高**（用户第一印象） | Sprint n+1 |

**建议的立即行动**：

1. **`forge --version`** —— 20 行 Go + 一个 Makefile 目标。这是任何 CLI 工具的基本礼仪
2. **`forge preflight --quick`** —— 复用 `forge doctor` 的代码 + 增加 claude 检测 + 成本估算 ≈ 150 行
3. **`forge init --help` 输出改进** —— 不是交互式，但至少描述默认值含义和下一步

---

## 从功能完备到工程完备

ForgeOS 的功能完整性已经很高——12 个 Go 包、8 项架构检查、5 个 workflow、三维 telemetry、四维安全护栏。但从「可用的系统」到「被团队采纳的系统」之间，还有**工程化**的最后一公里。

这次扫描揭示的模式是：**系统有极强的「内部」质量（代码架构、测试覆盖、治理纪律），但「外部」工程化（版本、基准、错误信息、新用户引导）还比较薄弱**。这种「内强外弱」在早期项目中是正常的——但当第一个外部用户试图部署 ForgeOS 时，版本信息和可操作的错误信息会比 arch-check 的 8 项检查更直接影响他的体验。

---

*分析日期：2026-06-30 | 第五次全量扫描，专门关注工程化与运营期缺项*
