# ForgeOS — 基于当前代码库的五个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局扫描 `forge-core/`(18 Go 包)· `harness/`(39+ 模块)· `pi-batch.py`· `.agent/`(5 工作流 / 12 agent 卡 / 9 skill 卡)·  
> `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`· `docs/requirements/`(~130 份已有扩展分析)  
> **纪律**: 不编写任何代码。只分析。每个方向附 `file:line` 代码证据、边界情况、与已有分析的关系。  
> **日期**: 2026-07-11

---

## 核心判断

ForgeOS 经过 31 轮 sprint，在**能力层**已高度成熟：编排引擎跑通串/并行 · 模型路由带安全下限 · 安全护栏四维完整 · 学习闭环 trace/scorecard/memory 就绪 · 真点火 multi-agent 端到端验证通过。已有 ~130 份扩展分析覆盖了绝大多数可预见的功能缺口和架构债务项。

但代码库中存在**五个被已有分析始终未作为独立方向处理的系统性缺口**——它们不是「加新功能」，而是**底层运行模型缺陷**：有的会在第一次 24h 无人值守运行时暴露为故障，有的在第一次外部集成时暴露为不可用，有的是零外部依赖原则的隐性成本。

| # | 方向 | 类型 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **`.forge/` 状态目录无运行身份隔离 (Run Identity)** | 数据完整性 · 并发安全 | **P0** | 无 run-id/session-nonce，并发进程相互覆盖 trace/checkpoint/memory，跨运行污染 |
| 2 | **`forge detect` 单通道分析独立于路由/风险/模式系统** | 集成缺口 · 能力浪费 | P1 | 结构检测信号从不注入路由决策或风险评分，重亏 v1.5 语义 |
| 3 | **无结构化输出契约 (`--output json` 覆盖率不足 20%)** | 集成能力 · 自动化 | P1 | 17 个子命令中仅 2 个支持 `--json`，ForgeOS 不可被 CI/CD 工具链程序化消费 |
| 4 | **零依赖原则的测试摩擦累积** | 开发者体验 · 治理 | P2 | 30+ 测试文件全手写断言，Python shim 需 stub，文件系统需 mock，无 quickcheck/fuzz |
| 5 | **编排引擎无确定性重放 (Deterministic Replay) 契约** | 调试 · 审计 | P2 | 给定同一 workflow + 同一 agent 输出，两次 run 可能因文件 mtime/随机/路径不同产生不同结果 |

---

## 方向一 · `.forge/` 状态目录无运行身份隔离

> **差异化验证**: 搜索 `docs/requirements/` 中 `run_id`/`session_id`/`nonce`/`run_identity` 作为独立方向的证明：
> 在全部 ~130 份已有分析中，运行身份隔离曾在 `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 的「多会话运行时协调」中被**作为一个子问题提到**，但从未作为独立方向展开。搜索 `run.*nonce\|session.*scoping\|process.*isolation\|concurrent.*forge` 在已有分析标题中**零命中**。

### 为什么需要

#### 当前状态

`.forge/` 目录使用**裸文件名**，没有任何运行身份标识：

```
.forge/
  checkpoint.json      ← 每次 Save 覆盖，毫无隔离
  trace.jsonl          ← 所有进程 O_APPEND 同文件
  memory.jsonl         ← 同上
  scorecards.json      ← 每 iteration 覆盖写入
```

`persist.Save` 有 `retain` 参数（保留历史 checkpoint 快照），但 trace/memory 是纯粹的追加日志——**没有文件名分片、没有前缀、没有进程级隔离**。

#### 代码级证据

**1. trace 路径恒定：所有进程写同一个文件**

```go
// forge-core/cmd/forge/scorecard_wind.go:58
func tracePath(root string) string {
    return filepath.Join(root, ".forge", "trace.jsonl")
}
// forge-core/cmd/forge/main.go:458
func memoryPath(root string) string { return filepath.Join(forgeDir(root), "memory.jsonl") }
```

两个 `forge run build` 进程同时在同一个项目目录启动：
- 进程 A 写 trace event 1-10
- 进程 B 写 trace event 1-15
- 结果：trace.jsonl 的行序列乱序交错，`seq` 字段冲突，下游 scorecard/latency 计算出错

**2. checkpoint 无进程锁**

```go
// forge-core/internal/persist/checkpoint.go:102
func Save(path string, cp Checkpoint, retain int) error {
    // 写入 .tmp → fsync → rename 覆盖原文件
    // 但两个进程同时 Save → 后 rename 胜利，前一个完全丢失
}
```

`Save` 的原子性设计（`rename(2)`）防止了崩溃时的半写损坏，但**不防止并发覆盖**——两个进程的 checkpoint 轮番写入，最终状态由最后一个 rename 决定，相当于「丢掉了其中一个进程的全部进度」。

**3. memory Append 在并发下产生交错条目**

```go
// forge-core/internal/memory/memory.go:185-203
// 使用 os.O_APPEND 单行追加
// 内核保证单次 write() 原子写入（但多进程各行交错）
```

内核的 `O_APPEND` 保证单次 `write()` 原子写入，但**不保证**两个进程写入的行不发生交错：进程 A 写入一行，进程 B 写入一行，虽然各自行内完整，但行序列乱序——`seq` 字段无法保证单调递增跨进程，trace/event 流的因果序被破坏。

**4. `forge status` 的 JSON 输出已有 `size_bytes`/`age` 字段，但缺少运行时进程身份**

```go
// forge-core/cmd/forge/validate.go:33-47
type statusJSON struct {
    Project           string   `json:"project"`
    DotForge          string   `json:"dot_forge"`
    Checkpoint        *statFile `json:"checkpoint,omitempty"`
    // ...没有 run_id, no running_process 字段
}
```

#### 边界情况

| 场景 | 当前行为 | 预期行为 |
|------|---------|---------|
| 两个 `forge run build` 并行执行 | trace/memory 文件内容交错+seq 冲突 | 各有独立 run 子目录(`.forge/runs/<run-id>/`) |
| `forge evolve --resume` 在两轮 evolve 之间手动创建了 checkpoint | 无缝覆盖，旧信息丢失 | checkpoint 版本化，可回溯上次状态 |
| 用户同时在两个终端跑 discover 和 build | 内存中 cross-session 数据混合 | 每个 run 有隔离的 memory 范围(attach to run-id) |
| CI + 本地同时跑 `forge accept` | scorecards.json 和数据损坏 | 各自读自己的 `.forge/<run-id>/` 状态 |
| 多个开发者共享 NFS 上的 `.forge/` | 同上，横跨多主机 | 通过 run-id + 主机 hostname 隔离 |

### 建议方向

1. **引入 run-id（UUID v7 或者时间戳+随机数）**：每次 `forge run/evolve` 生成一个 run-id，`.forge/` 下的路径改为 `.forge/runs/<run-id>/trace.jsonl` 等。
2. **`.forge/current` 符号链接**：指向最新的 run 目录，供需要「当前运行状态」的工具使用。
3. **可选进程锁**：在 checkpoint 和 trace 路径级别使用 `flock(2)` 或 `LockFile`，防止并发进程覆盖（同时保留隔离运行的能力）。
4. **`forge status` 新增 `running_runs` 字段**：展示当前 `.forge/runs/` 下的活跃 run 目录，让 operator 知道有哪些并发的运行。

---

## 方向二 · `forge detect` 检测信号不注入路由/风险/模式系统

> **差异化验证**: 搜索已有分析中 `forge detect` 作为集成方向的覆盖。`forge detect` 在 `five-uncovered-product-frontiers.md` 中被提到（作为「forge detect 语言检测限」），在 `strategic-extension-five-novel.md` 中被提到（作为「项目初始化流程」的一部分）——但从未作为「检测信号 → 路由/风险/模式回灌」的独立方向展开。已有分析聚焦的是 detect 作为 CLI 命令的完整性和语言覆盖，而非其与引擎其他子系统（routing/risk/mode）的集成缺失。

### 为什么需要

#### 当前状态

`forge detect` 是一个**独立分析工具**，输出结构化的项目剖析（`projectProfile`）并建议一个 workflow + mode + lifecycle，然后由用户（或 `forge evolve auto`）手动执行：

```go
// forge-core/cmd/forge/detect_parsers.go:244
func suggestWorkflow(p projectProfile) workflowSuggestion {
    // 纯规则引擎：基于 Language/HasTests/HasCI 做决策
    // 输出一个静态建议字符串
}
```

但这些检测信号——**语言类型、是否有测试、是否有 CI、包依赖密度**——从不反馈到以下系统：

**1. Model Router (`internal/routing`)**

```go
// forge-core/internal/routing/routing.go:67
func TierFor(agent, mode string) string {
    // 接收: agent 角色 + mode
    // 但从不接收: language complexity, test maturity, CI presence
}
```

一个 Go 静态编译项目（高测试覆盖率 + CI 完善 + 二进制体积大）与一个 Python 脚本项目（JSON-only + 零测试），在目前的路由器中得到完全相同的 tier 分配——只要 mode 相同。但这些差异恰恰是模型选择的信号：Go 项目更需要 Opus 的架构 reasoning，Python 脚本的 CRUD 代码可能 sonnet 就足够了。

**2. Risk Classifier (`internal/risk`)**

```go
// forge-core/internal/risk/risk_diff.go:64
func FromChangedPaths(paths []string) (Signals, []string) {
    // 只读改动的文件路径 → 推风险特征
    // 从不读 projectProfile 中的 HasTests/HasCI
}
```

一个没有测试覆盖的项目（`HasTests=false`）的任何改动都应该携带**更高的风险评级**——因为无测试覆盖的改动只能靠人审。目前无法建模。

**3. Mode Policy (`internal/mode`)**

`modes.yml` 的 mode 切换是显式的（`forge run --mode engineering`）。但 `forge detect` 可以自动建议初始 mode——当一个项目被检测为「有 CI、有测试、Go 编译型」时，默认的 `explorer` 或 `balanced` 可能不是最优的初始设置。

#### 边界情况

| 场景 | 当前行为 | 预期行为（如果检测信号回灌） |
|------|---------|---------------------------|
| Python 脚本项目（零测试）跑 `forge route` | tier 取决于 mode，与项目类型无关 | 从 detect 读取 `HasTests=false` → risk+1 档 → 自动升 tier |
| 大型 Go monorepo 跑 `forge run` | 流程与小型 JS 项目无异 | 从 detect 读取语言 → 路由偏好不同模型 |
| detect 发现 `project.yml` 不存在 | 用 `balanced`/`mvp` 默认值 | 基于检测信号的更精确初始建议 |
| 检测到 Rust 项目（Cargo.toml + tests） | 语言提示 "rust"，无其他影响 | Rust 编译+测试 信号可以影响 mode 选择（engineering 更合适） |
| `forge evolve auto` 在已有项目上跑 | 每轮都重新 detect（冷启动代价） | detect 结果缓存到 `.forge/`，减半每次 evolve 开销 |

### 建议方向

1. **Detect Pipeline**: 将 `detectProject` 的输出注入 `routing.TierFor` 和 `risk.FromChangedPaths` 的**可选上下文参数**中——让 detect 信号成为路由/风险评分的可选「加层」。
2. **`projectProfile` 缓存**: `forge detect` 结果可以写入 `.forge/detected.json`，被 `forge run/evolve` 读取——避免每次运行都重复文件扫描。
3. **Mode 自动建议**: detect 检测到「无 CI + 无测试 + 脚本语言」→ 建议 `balanced`；「有 CI + 测试 + 编译型」→ 建议 `engineering`。
4. **`forge status` 的 detect 集成**: `forge status` 可以展示上次 detect 结果，并在项目结构变化时（新加 `go.mod`）提示重新 detect。

---

## 方向三 · 结构化输出契约覆盖率不足 (`--output json`)

> **差异化验证**: 搜索已有分析中关于结构化输出契约的讨论：
> - `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向三讨论「统一结构化输出协议」——但聚焦的是每个子命令的 `--output json` **有无**。
> - 本文延伸更广：不仅评估覆盖率，还提出**跨子命令的输出一致性契约**（schema、error format、exit codes、pagination），以及「为什么 CI/CD 集成已被阻塞」。

### 为什么需要

#### 当前状态

`forge-core` 有 17 个子命令（`run`/`evolve`/`gate`/`check`/`accept`/`route`/`migrate`/`detect`/`scorecard`/`validate`/`status`/`doctor`/`adr`/`approve`/`reject`/`preflight`/`memory-prune`），但支持 `--json` 结构化输出的只有：

| 子命令 | `--json` 支持 | 证据 |
|--------|-------------|------|
| `forge detect` | ✅ 完整 | `detect.go:cmdDetectJSON` |
| `forge status` | ✅ 完整 | `validate.go:statusJSON` |
| `forge validate` | ❌ 纯文本 | `validate.go:cmdValidate` 只有 `fmt.Println` |
| `forge run` | ❌ 纯文本 | `main.go:cmdRun` 全程 `fmt.Printf/Logf` |
| `forge evolve` | ❌ 纯文本 | `evolve.go` 同上 |
| `forge route` | ❌ 纯文本 | `route.go` （人类可读的 tier 表格） |
| `forge accept` | ❌ 纯文本 | `acceptance.mjs`（不是 forge-core 子命令） |
| `forge approve` | ❌ 纯文本+交互 | `approve.go` |
| `forge migrate` | ❌ 纯文本 | `migrate.go` |
| `forge gate` | ❌ 纯文本 | `gate.mjs`（外部脚本） |
| `forge check` | ❌ 纯文本 | `check.py`（外部脚本） |

这意味着：

1. **CI/CD 不能程序化解析 `forge run` 的结果**——如果 `forge evolve` 报 convergence: NOT MET，CI 脚本只能通过 `grep` 文本输出来判断，不可靠。
2. **IDE 插件/Web UI/监控面板无法消费 ForgeOS 的输出**——任何集成都需要先解析彩色的控制台文本。
3. **错误格式不一致**：当前错误可能是 `fmt.Fprintf(os.Stderr, ...)`（预检）、`os.Exit(1)`（运行时）、`panic`（极少数），无统一 JSON error envelope。
4. **`forge run --json` 可作为机器间协议**：支持 `forge orchestrate --json-events`（每 phase 打印一行 JSON event）的话，外部系统可以实时监听运行状态——这曾是 ROADMAP 中 Sandbox/Web UI 的前提。

#### 边界情况

| 场景 | 当前行为 | 预期行为 |
|------|---------|---------|
| CI 调用 `forge accept` 并解析结果 | `grep "ACCEPTED\|REJECTED"` | 不需要解析彩色文本 |
| VS Code 插件显示 `forge run` 进度 | 无法程序化读取 | `forge run --json-events` 输出每行一个 JSON event |
| dashboard 展示所有 open run | 无 API | `forge status --json` 已支持；但运行时的 event stream 缺失 |
| `forge route` 结果接入外部 router | 人工读取 tier 表格 | `forge route --json` 输出 tier + reason + criteria score |
| 错误传递：exit code 无法区分「workflow not found」(1) 和「gate failed」(2) | 都是 exit 1 | 统一 JSON error envelope 包含 error code + message + cause |

### 建议方向

1. **`forge run --json` 和 `forge evolve --json`**: 在所有最终输出（convergence report、phase summary、gate results）使用结构化 JSON 替代/补充纯文本。
2. **`forge route --json`**: 输出 tier + 打分依据（复杂度/风险/依赖/安全/上下文分项）。
3. **`--json-events` 模式**: 每个 phase 完成时向 stdout 写一行 JSON event（包含 phase name/duration/status/model/cost），让外部工具可以实时流式监听运行状态。
4. **统一 JSON Error Envelope**: 所有子命令的错误输出统一为 `{"error":{"code":"...","message":"...","cause":"..."}}`。

---

## 方向四 · 零依赖原则的测试摩擦累积

> **差异化验证**: 搜索已有分析中关于测试基础设施/零依赖成本作为独立方向的讨论：在全部 ~130 份分析中，测试质量/基础设施只在 `structural-gaps-v41-genuinely-unexplored.md` 的「测试质量元治理」中被讨论（focus 是「哪些东西没被测试」）。而零依赖原则**本身**带来的测试可持续性成本从未作为独立方向被分析过。

### 为什么需要

#### 当前状态

ForgeOS 规定 `forge-core` 纯 Go 标准库**零外部依赖**（`go.mod` 无 `require`）。这个决策为二进制分发（无 `go.sum` 问题）和可靠性（无供应链攻击面）带来了极大优势，但**测试层也在同一个约束下**：

```go
// go.mod
module forgeos/forge-core

go 1.26
// 无 require — 零外部依赖
```

**零依赖测试的隐性成本**：

**1. 全手写断言**

没有 `testify/assert`——所有测试等价于：

```go
if got != want {
    t.Fatalf("expected %v, got %v", want, got)
}
```

这不是大问题。但更关键的是：**没有 `quick`/`fuzz`/`cmp`** 等标准库以外的一切测试库——Go 1.26 标准库的 `testing/quick` 只有最基本的属性检查，`cmp` 不在其中。

**2. Python shim 需在测试中 stub**

```go
// forge-core/cmd/forge/evolve_test.go:158
// loadWorkflow 需要 `python3 yaml2json.py <yml>` ——
// 测试必须创建假的 yaml2json.py 脚本
writeFile(t, filepath.Join(root, "harness", "yaml2json.py"), shim)
```

每个与 workflow 交互的测试都需要建立一个临时 `harness/` 目录 + 一个伪造的 yaml2json.py。如果不 stub，测试就依赖宿主机的 Python + PyYAML，**不可复制**。

**3. 文件系统交互被测试函数内部调用**

`persist.Save`、`memory.Append`、`trace.Tracer` 都直接向文件系统写数据。虽然 `persist` 和 `memory` 有意将编码解码与 I/O 分离（`encode`/`decode` 纯函数可单测），但**完整路径的集成测试**仍然需要真正的文件系统，且可能在 CI 中因权限问题/磁盘满/并发写入而 flaky。

**4. 无 fuzz 测试**

Go 标准库的 `testing/f` 支持 fuzz，但 forge-core 的复杂嵌套状态（workflow JSON 解码 + mode 解析 + converge 评估）非常适合 fuzz 测试——目前仅 `routing_test.go` 下有一个 `FuzzTierForScore` 入口，其他包可见零 fuzz 入口：

```bash
$ grep -rn "Fuzz" forge-core/ --include="*_test.go"
forge-core/internal/routing/routing_test.go:308:func FuzzTierForScore(f *testing.F) {
# 仅此一个
```

**5. 黄金文件测试可用但不系统**

少数测试使用黄金文件（`asset_test.go` 的 JSON fixture），但大多数测试手动构造输入/断言。没有统一的 fixture 管理、快照测试框架、或 golden file diff 工具。

#### 边界情况

| 场景 | 当前行为 | 预期 |
|------|---------|------|
| 新增一个 JSON 解析器行为测试 | 手写 `json.Marshal/Unmarshal` + 断言 | 可以用 fixture 文件驱动 |
| 测试 converge 状态机的大量组合场景 | 手写 N 个 test function | 表格驱动测试+quickcheck |
| 确保 yaml2json Go 解析器与 Python 一致 | 半自动：每 sprint 手动跑一次 `block_scalar_test` | 自动跨验证的回归套件 |
| `persist.Save` 在磁盘满时的行为 | 手动构造一个只读目录 | 通过 `os.IsPermission` 测试，但无系统级测试 |
| memory 文件 > 2GB | 无测试 | 通过 fuzz 检测边界 |

### 建议方向

1. **不对零依赖放水**：测试的零依赖纪律应保持（不放 `testify` 等进来）。但可以：
   - 标准化一个 `testutil` 辅助包（手写 `AssertEq`/`AssertNil`/`Require` 等，仅用于测试文件）
   - 使用 `io/fs` 接口注入 fake filesystem（当前 `persist.Save` 直接调 `os.OpenFile`，无法 mock）
2. **系统化黄金文件测试**：对 asset 解码、yaml2json block-scalar、risk diff 等稳定边界，建立 `.testdata/` 下的黄金输入/输出对。
3. **Fuzz 入口**：为 `asset.LoadWorkflowJSON`、`converge.Converge`、`risk.FromChangedPaths` 添加 fuzz 测试。
4. **Python shim 依赖的隔离**：将 `harness/yaml2json.py` 提取到 `testdata/` 下或嵌入 `testutil`，使测试不依赖真实文件系统上的外部脚本。
5. **`gosimports` + `gofmt` 持续集成**：利用 forge-core 是纯 Go 标准库这一事实，将 gofumpt/linting 通过 `go generate` 本地化。

---

## 方向五 · 编排引擎无确定性重放契约

> **差异化验证**: 搜索已有分析中确定性重放的覆盖率：
> - `execution-semantic-gaps.md` 讨论了**执行原子性**和**幂等性**（「replay same input → same output」作为执行语义的一部分）。
> - `strategic-extension-five-novel.md` 方向四讨论了**Trace 查询**和**审计追溯**。
> - 但**编排引擎级别的确定性重放**（给定 workflow + 相同 agent 输出、相同文件状态、相同 git ref → 产生完全一致的 trace/converge verdict 和 gate 结果）从未作为独立方向。

### 为什么需要

#### 当前状态

ForgeOS 的编排结果取决于**大量非确定因素**：

**1. 文件系统 mtime/atime 敏感性**

```go
// forge-core/internal/memory/memory.go:59-100
// loadCache 使用 path + mtime 作为缓存键
// 同一文件两次读取，如果 mtime 不同（哪怕内容相同）→ 缓存失效 → 不同行为
```

`memory.Load` 的缓存键包含 `mtime`。如果 CI 中两次 `git checkout` 导致 mtime 变化，memory 的查询结果可能不同——虽然内容相同。

**2. 目录遍历顺序不确定**

`detect_parsers.go:329` 使用 `filepath.WalkDir`——Go 标准库不保证 WalkDir 的回调顺序跨平台一致。`prompt.go:99` 使用 `os.ReadDir` 读取目录条目，其返回顺序依赖于文件系统的目录项布局。虽然大多数情况下最终结果可能是顺序无关的，但**传播路径**可能间接影响确定性。

**3. `map[string]any` 的 JSON 序列化顺序**

Go 的 `json.MarshalIndent` 对 `map[string]any` 按 key 字母序输出。但 `asset.LoadWorkflowJSON` 将 YAML 转成 `map[string]any` 之后，**YAML 中 phase 的顺序**在类型化后的 Workflow struct 中保持（因为 `Phase` 是数组），但**元数据字段**（如 `loop` 的 `loop_back_to`）可能在 JSON 中字母序排列——两次 Marshal 理论上给出相同 JSON，但在复杂嵌套场景下可能引入不确定性。

**4. Trace Event 的 Seq 非确定性**

```go
// forge-core/internal/trace/trace.go
type Tracer struct {
    mu  sync.Mutex
    seq int
    Now func() time.Time
}
// seq 是进程级别的单调递增计数器
// 但不同进程（或同一进程不同 run）seq 从 0 开始
```

同一个 workflow 两次运行，trace event 的 seq 值完全相同（假设 phase 数和顺序一致）。这看起来是确定的，但**如果任何路径依赖于时钟**（`time.Now`、`duration_ms`、`updated_at_unix`），两次运行就会产生不同的 trace——因为它们发生在不同时间。

**5. `persist.checkpoint.UpdatedAtUnix` 每次运行不同**

```go
// forge-core/internal/persist/checkpoint.go:50-70
UpdatedAtUnix int64 `json:"updated_at_unix"` // caller-supplied, not auto-set
```

这个字段是调用方注入了 `time.Now().Unix()`，导致 checkpoint 文件每次不同——即使所有业务字段完全一致。

#### 为什么这是一个问题

对于「软件工厂」这个角色，确定性重放有两层要求：

1. **调试**：用户报告「forge run build 在 iteration 3 收敛了」，你想在另一台机器上复现——但 trace 不同、时间戳不同、mtime 不同，你无法验证「是同一个 bug」。
2. **审计**：合规要求证明「这个软件在不施加人为干预下通过了 10 轮 evolve」。如果有非确定性因素（gate 超时、文件扫描顺序变化、map 键序遍历不同），审计员不能独立验证。

#### 边界情况

| 场景 | 当前行为 | 期望的确定性重放行为 |
|------|---------|-------------------|
| 对同一 repo 跑两次 `forge run build --executor dry` | trace.jsonl 的 `duration_ms` 和 `updated_at_unix` 不同 | 所有业务字段完全一致 |
| 审计员从 CI 工件拿到 trace 和 checkpoint | mtime + 时间戳不同，无法验证是否与 CI 结果一致 | `FORGE_DETERMINISTIC=1` 模式使时间戳冻结 |
| 架构师想要「playback」历史 run | trace 缺少状态快照，无法重建输入状态 | replay 命令输入 trace + git ref 重建相同环境 |
| `forge validate` 发现 phase 列表变化 | 相位顺序确定，但额外字段（duration）不可比 | 只比较 schema 相关字段，排除时间噪音 |
| CI 跑 `forge evolve` 和本地跑结果不一致 | 无法判断是编排器差异还是环境差异 | 确定性模式消除环境噪音 |

### 建议方向

1. **`FORGE_DETERMINISTIC` 环境变量**: 设为 `1` 时，所有时间戳和 runtime 命名一致化：
   - `trace.Event.DurationMs` 恒为 `0`
   - `checkpoint.UpdatedAtUnix` 恒为 `0`
   - `memory.Entry.CreatedAt` 恒为固定值
   - 文件扫描使用稳定排序（`sort.Slice` 保证顺序）
   - 所有 `map` 序列化通过 `sort keys` 保证不变顺序

2. **确定性模式下的 schema 字段标记**: 给 trace/checkpoint/memory 的 struct 字段加 tag `deterministic:"false"`——在 `FORGE_DETERMINISTIC` 模式下排除这些字段。

3. **git ref 锁定**: `forge run --git-ref <sha>` 在 run 前 `git checkout` 目标 ref，保证文件状态一致，运行结束后恢复。

4. **`forge replay <trace.jsonl>`**: 从 trace 逆向推导 run 序列，在人读输出中重建「假设重放」结果（与 `~` 区分，这是真实播放还是推理回放需要文档说清楚）。Honesty：不能完全重建 agent 输出，只能重建编排逻辑序列。

5. **`forge run --seed <int>`**: 在涉及随机性的算法（scorecard decay 中可能有的随机 tie-break、memory.Compact 的选择策略中潜在的 shuffle）中使用固定种子。

---

## 总结：优先级与建议

| 方向 | 优先级 | 类型 | 一句话 | 建议的 Sprint |
|------|--------|------|--------|-------------|
| 一 · 运行身份隔离 | **P0** | 数据完整性 | 第一次有人开两个终端跑 forge 就会遇到文件损毁 | 立即 |
| 二 · detect 信号回灌 | **P1** | 集成缺口 | 已检测的信息白白浪费，现有路基已准备好接入 | 2 sprints |
| 三 · 结构化输出契约 | **P1** | 自动化 · 集成 | CI/CD 和 Web UI 必须手撕文本，这是产品化的天花板 | 2-3 sprints |
| 四 · 测试摩擦管理 | **P2** | 开发者体验 | 不影响用户但在第 40 个 test file 时会显著拖慢开发 | 1 sprint 基础设施 |
| 五 · 确定性重放 | **P2** | 调试 · 审计 · 正确性 | 审计需求触发时再启动，但前置标记（deterministic tag）可以先做 | 2 sprints 可选 |

---

## 与已有 130+ 份分析的关系

本文无意取代或重复已有分析。每个方向都通过关键词搜索+语义阅读确认其**作为独立方向从未在已有 ~130 份 `docs/requirements/*.md` 中展开**。具体来说：

- **方向一**（运行身份隔离）：并发安全和 `.forge/` 数据完整性已在 `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 中被提到（作为「多会话运行时协调」的子问题），但从未被作为独立方向拆开分析。
- **方向二**（detect 回灌）：`forge detect` 已在 `five-uncovered-product-frontiers.md` 和 `strategic-extension-five-novel.md` 中被讨论过，但聚焦的是 detect 的 CLI 功能完整性和语言覆盖，而非其与 routing/risk/mode 的集成缺口。
- **方向三**（结构化输出）：`genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向三「统一结构化输出协议」覆盖了 17 个子命令 `--json` 的**有无**；本文补充延伸至**跨子命令 schema 一致性**和**与 CI/CD/IDE 集成**的经济学。
- **方向四**（测试摩擦）：已有分析覆盖了「什么被测试了/没被测试」，但未从**零依赖原则对测试实践的隐性影响**这个角度分析。
- **方向五**（确定性重放）：`execution-semantic-gaps.md` 从原子性/幂等性角度涉及了重放，但编排引擎级别的确定性重放契约（`FORGE_DETERMINISTIC` mode + `forge replay`）未被覆盖。

每个方向都包含 `file:line` 的精确代码引用——本文不是泛泛的「架构分析」，而是基于可验证的代码事实。
