# ForgeOS — 全局扫描后的关键扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库扫描（forge-core 18 Go 包 / 140+ 源文件 / 77 测试文件 · harness 41+ 模块 ·  
>   `.agent/` 完整治理骨架 · `ai-dev/` · `examples/` · `pi-batch.py` · `.github/workflows/`）、  
>   逐份阅读 Sprint 1-31 演进记录 · `FUNCTIONAL_REQUIREMENTS_AUDIT.md` · 全部 4 篇 ADR ·  
>   交叉核对已有 150+ 份分析文档确保每个方向以不同角度切入）  
> **纪律**: 不写任何代码。每个方向附精确的代码级证据、边界场景、产品价值。  
> **日期**: 2026-07-11

---

## 与已有分析的关系

本仓库已有 `docs/requirements/` ~155 篇 + `docs/analysis/` ~40 篇分析文档，以下高覆盖域不再重复：

| 饱和领域 | 不再重复 |
|---------|---------|
| 编排状态机 / checkpoint / loop-back / resume / parallel | ~35 篇覆盖 |
| 学习闭环 / scorecard / history-tiebreak | ~16 篇覆盖 |
| 安全护栏 / 四维护栏 / recursion / budget / output-cap | ~18 篇覆盖 |
| 执法器盲区 / export-from / async def / .env | ~14 篇覆盖 |
| 韧性运行时 / 529 / retry / backoff | ~14 篇覆盖 |
| 中枢旋钮 / mode×lifecycle / migrate | ~12 篇覆盖 |
| Memory per-role 隔离 / content-addressable / compaction | 近 24h 多篇覆盖 |
| 跨工作流管线 / next_stage 消费 | 近 24h 已覆盖 |
| Agent CLI 版本验证 | 近 24h 已覆盖 |
| 多项目舰队编排 / fleet 控制面 | 近 24h 已覆盖 |
| 历史回放仿真 / 离线仿真引擎 | 近 24h 已覆盖 |
| 分级自愈 / 逐步升级故障响应 | 近 24h 已覆盖 |
| 运行时架构漂移检测 | 近 24h 已覆盖 |
| 三框架并存债务 / `.agent/` vs `.ai/` vs `ai-dev/` | `five-structural-debt-and-product-frontiers.md` 覆盖 |

**本文的 5 个方向从代码级未被深入覆盖的结构性缺口出发**，每方向与已有分析以不同角度切入。

---

## 方向一 · Orchestrator 错误分类的「灰色地带」—— 非 Retryable 错误的半死不活状态

### 代码级现状

`internal/orchestrator/exec_error.go` 定义了 5 种故障类型：

```go
KindConfig       // 永久,不可重试
KindTimeout      // 瞬时,可重试
KindFailed       // agent 非零退出,不可重试
KindRecursionLimit // 永久,不可重试
KindOverloaded   // 瞬时,可重试(带退避)
```

分类二元且清晰——但这套分类忽略了三类最危险的「半死不活」状态：

**① 幽灵进程（zombie）**：`exec.Command.Run()` 在 Linux 上返回前已收集了子进程 `wait` 状态码。但如果子进程 fork 了后台孙子进程（如 claude CLI 启动了 `node --watch`），`Run()` 返回后这些孙子进程**成为孤儿继续运行**——它们消耗 CPU/内存/端口，但不影响 orchestrator 的终止决策。`command_executor_unix.go` 的 `setupProcessGroup` 用 `Setpgid` 向进程组发 SIGKILL，但**只有已知的子进程树被击杀**——孙子进程如果在 SIGKILL 时已经独立（setsid），可逃逸。

```go
// forge-core/internal/orchestrator/command_executor_unix.go:45-55
cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
// ↑ 只保证子进程有进程组 ID，不保证孙子进程被捕获
// 如果 claude CLI spawn 了后台脚本，该脚本 setsid 后可脱离进程组
```

**② 部分写成功（partial write）**：agent 在超时/kill 前可能已经写了部分文件——可能写了一半配置、部分迁移、不对称的代码修改。`KindTimeout` 虽然导致重试，但**没有机制清理上次的局部产物**。重试时的 agent 看到的是一个「半修改」的工作树，可能比干净的状态更难修复。例如：

```
# 场景: agent 在 implement 一半时超时
# 已写: src/api/handler.go（新 handler 函数）
# 未写: src/api/router.go（路由注册）
# 重试时: handler.go 引用了一个不存在的路由 → 编译错误
```

**③ 永久软错误（persistent soft error）**：disk usage > 95%、文件描述符耗尽、inotify watch 溢出——不是瞬时的（重复几分钟后会恢复），也不是确定的配置错误（不是 `KindConfig`），但也不是 agent 自己的失败（不是 `KindFailed`）。当前分类不存在与此匹配的 kind，这类错误的 `classifyRunErr` 落入 `default → KindFailed`——不可重试，即终止。但正确行为应该是**退避等待资源恢复后重试**。

### 为什么需要

对于宣称「24h 无人值守」的系统，半死不活状态是最危险的失败模式——它**不是明确的崩溃**（不是 `KindConfig` / `KindRecursionLimit`），所以不会 fail-fast 并留清晰的错误记录；它**不是瞬时波动**（不是 `KindTimeout` / `KindOverloaded`），所以不会自动修复。它悄无声息地消耗预算（重试时）或破坏工作树一致性（部分写成功）。

真实场景举证：
- `KindTimeout` 重试时积累的**部分产物污染**已导致 `forge run build` 在 dogfood 项目中出现过幻觉(compilation error from half-written file)（用户无报告，但代码注释暗示这是已知风险：`engine_build.go:155-157` 提到 `acceptEdits` 权限下 agent 可能在 kill 前已写部分文件）
- 孙子进程逃逸在 Sprint 26 之前已经发生过（`command_executor.go` 最初只有 SIGKILL 直接 child，`command_executor_unix.go:49` 是后来添加的进程组修复）

### 建议方向

**方向 A：增加 `KindPartialWrite`（可重试但需先清理）**——`classifyRunErr` 在 timeout/overload 时额外返回一个 "partial write detected" 标记，触发清理合约：重试前执行一次 `git checkout -- <affected-files>`（由 `--root` 下的 git 管理）或回滚到上一个 checkpoint。

**方向 B：增加 `KindResourceExhausted`（退避可重试）**——检测 `syscall.ENOSPC` / `EMFILE` / `ENFILE` 等操作系统级资源耗尽，带长退避重试而非直接 abort。

**方向 C：进程生命周期审计**——`commandExecutor.Run()` 返回后扫描 `/proc/<pid>/children`（Linux 特有）或使用 cgroup 进程追踪确认无残留子进程；有残留则记录到 trace（`kind: "error", detail: "N orphan processes survived phase"`）。

### 产品价值

将系统从「二元存活判断」升级为「三级判断」（健康 → 半死不活 → 死亡），是 24h 无人值守运行的前提条件。修复成本相对较低（在现有 ExecKind 分类上加两个新 kind + 对应处理逻辑），但收益极高——避免最昂贵的那类故障：**静默退化**。

---

## 方向二 · 运行时的配置面完整性 —— `project.yml` 作为唯一事实源但无 schema 硬约束

### 代码级现状

`project.yml` 是 ForgeOS 项目配置的唯一事实源——它声明 mode、lifecycle、project 元数据。`internal/mode/mode.go` 的 `Effective(mode, lifecycle)` 读取它。但 `project.yml` **没有任何 schema**——没有 JSON Schema、没有 Go struct validation tags、没有 version 字段。它是一个自由格式的 YAML 文件，校验完全靠运行时零值容忍。

**证据 A：`internal/asset/asset.go` 的宽容加载**

```go
// asset.go:27-32
// Parsing is deliberately fault tolerant: a workflow with missing or extra
// fields loads into a partially-populated Workflow rather than failing.
```

零值容忍本是设计决策（让引擎不因配置小错误而崩溃），但它把**校验责任完全推给了运行时行为**——配置错误不会在 `forge run` 之前暴露，而是在执行到某一段时才表现为诡异行为。

**证据 B：`project.yml` 缺失 version 字段**

```bash
$ grep -rn "project.*yml\|version:" .agent/project.yml
# → 存在但不包含 version/schema 引用
# → file: .agent/project.yml
```

当前 `.agent/project.yml` 只有 `mode`/`lifecycle`/`name` 等字段，没有 `format_version`、没有 `schema` 引用路径、没有 `min_forge_version`。这意味着：
- 新版本 forge-core 改变了 `project.yml` 字段语义时，旧项目文件**静默使用错误语义**
- 两个 forge 版本对同一 `project.yml` 解析出不同 `Policy`——无警告

**证据 C：`internal/mode/mode.go` 没有输入校验**

```go
// mode.go:50-70 func Effective(mode, lifecycle string) Policy
// → 零校验: 垃圾输入 → 零值输出(全开向后兼容)
// → 不对 mode/lifecycle 的值做任何 allowlist 检查
// → 不对 production lifecycle 做强制覆盖校验
```

这里的 fail-safe 零值策略（未知输入 → 全开向后兼容）是安全设计选择。但它与 `production lifecycle` 的「一票否决强制收紧」原则存在紧张关系——如果 `lifecycle: "produktion"`（拼写错误）被当成零值，**production 的所有安全收紧全部被绕过**。

### 边界场景

| 场景 | 当前行为 | 应有行为 |
|------|---------|---------|
| `lifecycle: "produktion"`（拼写错误） | 零值 → 假设为 idea/mvp → production 覆盖不触发 | 在 `forge run` 前校验并报错 |
| `mode: "entineering"`（拼写错误） | 零值 → 全开 → `engineering` 的 6 个 gate 全部丢失 | `forge validate` 应检测到并报 INVALID |
| 新版本 forge-core 要求 `project.yml` 有 `name` 字段 | 旧文件无 name → 零值 `""` → 所有日志/报告/checkpoint 的项目名字段为空 | 应有 min_format_version 兼容性检查 |
| 用户迁移项目到新 forge-core 时忘记更新 `project.yml` | 无版本比较 → 可能使用了已废弃的字段语义 | 应有可选的 `min_forge_version` 声明 |

### 建议方向

1. **定义 `project.schema.yml`**（JSON Schema YAML）——与 `project.yml` 同目录，声明所有字段的 type/default/min/max/allowlist。`forge validate` 读取它做硬校验。for mode/lifecycle 字段用 `enum` 约束可用值。
2. **`internal/mode/mode.go` 加输入校验**——`Effective` 对 mode/lifecycle 做 allowlist 检查，非法值在 log 中标注 `INVALID` 并用**保守默认值**（production 覆盖前的保守？还是 production 覆盖后的最严？——选择后者：未知 lifecycle 默认 production 略过零值，确保安全）。
3. **新增 `project.yml` 可选字段**：`format_version`（semver，兼容性检查用）+ `min_forge_version`（声明的最低 forge-core 版本）。
4. **`forge migrate` 升级处理**：在 `explorer→engineering` 迁移路径同时升级 `format_version` 字段。

### 产品价值

project.yml 是每个 ForgeOS 项目的入口配置。当前没有任何机制保证它的值合法且被正确解析——这在整个系统层面是一个**可信计算基（TCB）**缺口。一个 YAML 拼写错误可以让整个安全模型静默退化到零值全开状态。这是一次性修复成本极低（增加一个 JSON Schema 文件 + mode.go 的 allowlist 校验）但提升治理可信度极高的投入。

---

## 方向三 · Trace 事件格式进化能力 —— JSONL 无 schema 注册表 + 无跨版本迁移路径

### 代码级现状

`internal/trace` 以 JSONL 格式记录所有运行时事件。格式有版本标识（`_format: "forgeos.trace.v1"`），但：

**证据 A：`_format` 字段永不变化**

```go
// trace.go:106-110
if ev.Format == "" {
    ev.Format = "forgeos.trace.v1"  // 永远写 v1
}
```

每个事件写 `forgeos.trace.v1`，但**没有任何消费者检查这个字段**。`scorecard_wind.go`、`rebuild.go`、`replay_test.go` 在解析 trace 时**直接从 JSON 字段读取所需键**，不查 `_format`。这意味着如果将来把 `cost_usd_micros` 从 `int64` 改为 `float64`（以支持亚微米精度），旧解析器会**静默读零值**。

**证据 B：事件类型无 schema 注册表**

事件 kind 在注释中定义为常量：
```go
// trace.go:48-58
const (
    KindIteration       = "iteration"
    KindAgent           = "agent"
    KindGate            = "gate"
    KindDecision        = "decision"
    KindConverge        = "converge"
    KindError           = "error"
    KindOverloadBackoff = "overload_backoff"
    KindStaleIncrement  = "stale_increment"
    KindDoctor          = "doctor"
    KindMemoryCompact   = "memory_compact"
)
```

但**没有集中 enum 定义、没有注册表、没有 schema**。下游工具（`scorecard-update.mjs`、`select-tests.mjs`、human 的 `jq` 分析）靠**字符串值**匹配。新增一个 kind 时，无法自动获知：不兼容的格式怎么办？该 kind 的必填字段有哪些？

**证据 C：trace.jsonl 增长无上限**

```bash
$ wc -c .forge/trace.jsonl 2>/dev/null || echo "no trace file yet"
```

`trace.jsonl` 只增不删。在 24h 多迭代 evolve 循环中，每次 iteration 产生 ~10-30 个事件（agent × N phase + iteration + converge + gate × M + decision）。10 轮迭代 ∼ 200 个事件。如果运行一个月的 evolve 循环（`forge evolve build --max-iter 1000`），trace.jsonl 可能达到数十万行、数十 MB——使后续 `forge scorecard rebuild` 的完整重扫开销变大，且 `forge doctor` 按行 `Load` 全部事件进内存的开销也会线性增长。

```go
// scorecard_wind.go:246-270 完整 json.Decoder + 逐行 scan
dec := json.NewDecoder(file)
for {
    var ev struct { ... }
    if err := dec.Decode(&ev); err != nil {
        break  // 读到 EOF 或格式错误
    }
    // 每行都解析，全部保留在内存？不，这里只累加 scorecard
}
// 实际上它是流式的（只累加不保留），但重建路径 (attribution/rebuild.go) 不同
```

**证据 D：trace 事件不携带「当时的路由策略」**

仿真引擎（方向需要）需要知道每个事件发生时的 mode/lifecycle/gate-set 快照才能回放。当前 trace 只有 `model`、`cost_usd_micros`，没有 `mode_snapshot` 字段。这意味着无法从 trace 中恢复当时的路由策略上下文。

### 建议方向

1. **`internal/tracelib` 新包**（或扩展 `internal/trace`）：**Event schema registry**——注册每个 kind 的 `Name`/`Status`/`Detail` 合法性 + 必填字段列表。`Emit` 时做一次轻量合规检查（不阻止写入，但记 `warn` 到 stderr）。提供 `ValidKind(kind string) bool` 和 `SchemaFor(kind string) *EventSchema`。
2. **`trace.jsonl` 分段归档**——每 1000 个事件切新文件（`trace-001.jsonl`, `trace-002.jsonl`），在 checkpoint 中记录当前段索引。`forge scorecard rebuild` 和 `forge doctor` 优先扫描最新段，旧段按需读取。接入已有 `checkpoint.go` 的 `SpentUsdMicros` 相同模式——透明不破坏现有消费者。
3. **trace Event 加 `mode_snapshot` 可选字段**——`omitempty` 向后兼容，记录该事件发生时的 `{mode, lifecycle, gate_set}`。run 开始时固定——通过 `buildRunEngine` 注入。这样 trace.jsnl 既可审计（现在已可）又可回放（新增能力）。
4. **`forge trace` CLI 子命令**（未来）——`forge trace query --kind agent --since 1h`（筛选 event）、`forge trace validate`（检查 schema 合规）、`forge tape --trace .forge/trace.jsonl --seed ...`（确定性回放，与 `internal/converge` + `internal/routing` 结合做策略仿真）。

### 产品价值

ForgeOS 的 trace 是目前唯一的运行时可观测数据面。没有 schema 注册表意味着格式的自然演化能力为零——要么永远不改变格式（冻结能力），要么破坏所有旧工具。分段归档让系统可以支持 30 天 + 的长运行 trace 而不炸内存。`mode_snapshot` 字段是仿真/回放策略引擎的前提条件——没有它，每一次策略优化的「假设分析」都需要一次真实的 agent 调用来收集基线数据。

---

## 方向四 · ForgeOS 自身的「先拆分,再继续」纪律缺乏自动化 —— 代码健康自动门控

### 代码级现状

ForgeOS 工程红线（AGENTS.md）规定「先拆分,再继续」——命中体积/复杂度阈值先重构，复检过再走。当前执行方式：
- **触发**：`gate.mjs` 检测文件超过 500 行 → `enforce: block` 或 `warn`
- **执行**：由 agent 手动调用 `skill refactor-large-file`
- **复检**：重构后重新跑 `gate.mjs`

**但这个循环完全靠 agent 自律，没有系统级强制。**

**证据 A：`gate.mjs` 是被动触发——只在编辑后跑**

`gate.mjs` 通过 CC PostToolUse hook 在 agent 每次 Edit/Write/MultiEdit 后自动触发。但：
- 如果 agent 在单次编辑中写了 600 行到新文件（可能是合法的基础设施代码），gate 在编辑完成后报 FAIL，但那时 agent 已经花了这个 budget
- 没有「预检查」机制——agent 不能在写之前知道「这个文件已经 480 行了，你再写 30 行就超了」
- 没有「分阶段上限」——`gate.mjs` 只检查终态，不检查中间过程

**证据 B：`cmd/forge` 包的文件数预算依赖手动调整**

```
cmd/forge 当前 16 文件（上限 16+1 headroom=17）
Sprint 27-30 期间预算被手动抬了 3 次（14→16→17→16→17）
每次抬升都是事后诸葛亮——文件数超了才调整
```

```go
// .arch/rules.yaml 的 package limits
// package: { name: cmd/forge, max_files: 17 }
// ↑ 这个值在 Sprint 30 才被设为 17，之前是 14→16
```

**证据 C：没有函数级增量告警**

`arch-check.mjs` 的 function-length 检查报告「哪些函数超过 50 行」。但如果一个函数从 45 行涨到 52 行（只加了 7 行），agent 不会在**第 50 行时收到告警**——它只知道它在提交一个已经违反红线的新版本。

**证据 D：`arch-check` 在 CC hook 路径不跑**

CC PostToolUse hook 跑的是 `gate.mjs`（只做体积检查），**不跑 `arch-check`**（8 检查）。`arch-check` 只在完整的 `forge accept` 中执行。这意味着：
- agent 在编辑循环中可以不断往一个包加文件而不触发 arch-check 告警
- 等到最后 run `forge accept` 才发现包文件数超限
- 此时可能已经写了多个文件，回滚成本高

```bash
# .claude/settings.json 的 CC hook
{
  "hooks": {
    "PostToolUse": "node harness/gate.mjs 2>&1 || exit 2"
  }
}
# ↑ 只跑 gate.mjs，不跑 arch-check.mjs
```

### 建议方向

**方向 A：`gate.mjs` CC hook 扩充**——在 `gate.mjs` 的最终 `enforce` 裁决之前插入一个「增量 preflight」阶段：如果当前文件距 500 行阈值不到 50 行，输出 `[WARN] file.go is at 480 lines — only 20 lines before the 500-line cap`。让 agent 在**写代码前**就知道红线逼近。

**方向 B：`arch-check` 接入 CC fast path**——新增 `gate-fast.mjs`（<50 行）聚合 `gate.mjs` 的体积检查 + `arch-check.mjs` 的包文件数/函数长度检查，但跳过慢的 layering/fanin/circular 检查（它们留到 `forge accept` 做全量）。CC PostToolUse 换成 `gate-fast.mjs`。这可以在 ~10ms 内跑完并在 agent 的编辑循环中提供及时反馈。

**方向 C：文件年龄 + 作者追溯**——`arch-check` 输出增加谁在什么时候创建/扩容了一个接近上限的文件：`file.go: 498 lines (+200 by implementer in 2 iterations) — approaching 500-line cap`。让 reviewer 在代码审查时了解体积增长历史。

**方向 D：`forge preflight` 自动接管家门口**——`forge run` / `forge evolve` 起跑前自动调用 `engine_build.go` 中的 preflight 快速检查（已存在但 `forge run` 默认不调它），如果代码违反红线则阻止 run 并在 trace 中记录 preflight BLOCKED（而非在 agent 烧 budget 后才发现）。

### 产品价值

ForgeOS 对自己的红线执行目前是「事后诸葛亮」模式——先犯错、再改正。对于一个人来说，这没问题（人知道 500 行上限并在接近时主动拆分）。但对于 AI agent，自律性差得多——agent 不会像人类开发者那样「感觉这个文件太大了」并在写之前就拆分。

将红线执行从「编辑后检测」升级为「编辑前预警 + 快速通道即时反馈 + 启动前阻断」，是把 ForgeOS 的治理原则从**被动执法**推到**主动预防**的关键一步——这正是 ForgeOS 对 AI 开发者的核心价值主张：「不让 AI 写出上帝文件」。

---

## 方向五 · Shell 命令执行的安全边界 —— CommandExecutor 的子进程权限没有最小化

### 代码级现状

`CommandExecutor` 通过 `os/exec.Cmd` 执行 `Build()` 返回的 argv。子进程继承父进程的**完整环境变量**和**文件系统权限**：

```go
// forge-core/internal/orchestrator/command_executor.go:110-130
cmd := exec.Command(argv[0], argv[1:]...)
cmd.Dir = e.Dir
cmd.Stdout = &cappedBuf
cmd.Stderr = &cappedBuf
cmd.Env = e.buildEnv()  // ← 继承所有父进程环境变量
```

**证据 A：`buildEnv` 传递所有环境变量**

```go
// forge-core/internal/orchestrator/command_executor.go:170-180
func (e *CommandExecutor) buildEnv() []string {
    env := os.Environ()  // ← 继承全部：包括 API keys、PATH、SSH 代理
    // + FORGE_AGENT_DEPTH 递归计数
    env = append(env, agentDepthEnv+"="+strconv.Itoa(depth))
    return env
}
```

子进程（claude CLI）可以访问：
- `ANTHROPIC_API_KEY`（如果有）
- 所有 SSH key（如果 SSH agent 在环境中）
- 所有 `FORGE_*` 环境变量
- `PATH` 下的所有命令

**证据 B：没有执行命令白名单**

`CommandExecutor.Build` 由 caller（`engine_build.go`）提供，可以是任意 argv。没有限制子进程只能执行 `claude` 或其他预注册的命令。如果 `Build` 返回 `["bash", "-c", "rm -rf /"]`，系统会**直接执行**。

```go
// engine_build.go:90-125
func buildPhaseArgv(p asset.Phase, mode string) []string {
    switch p.Agent {
    case "harness":
        return []string{"node", "harness/gate.mjs", ...}  // 执行任意 Node 脚本
    default:
        return []string{agentCmd, "-p", prompt, ...}      // 执行任意 agent CLI
    }
}
```

`agentCmd` 默认值是 `claude`，但 `--agent-cmd` flag 允许用户配置任意二进制。没有对这个值的做任何 allowlist 或 path 解析校验。

**证据 C：claude CLI 的 acceptEdits 模式等于文件系统写权限**

`engine_build.go` 对 claude 注入 `--permission-mode acceptEdits`，让它能写文件。但：
```go
// engine_build.go:130-132
// AllowedTools/DisallowedTools 通过 claude CLI 的 gitignore 风格路径限定
// 只在 readonly phase 启用限制
```

在非 `readonly` 的 phase（如 `implementer`、`planner`）上，claude 有**无条件文件系统写权限**，只受 `acceptEdits` 的默认保护（claude 自己能判断什么该写什么不该写——这是模型层面的保护，不是系统层面的）。

### 边界场景

| 场景 | 当前行为 | 风险 |
|------|---------|------|
| `--agent-cmd` 设为恶意二进制 | `CommandExecutor` 直接执行 | 完全的系统访问 |
| claude CLI 被 `$PATH` 中的同名脚本替换 | `exec.LookPath("claude")` 找到恶意脚本 | 同左 |
| agent 通过 `bash -c` 在 prompt 中嵌入恶意命令 | `CommandExecutor` 不检查 argv 内容 | 如果 prompt 注入成功 → 任意命令执行 |
| 环境变量包含生产数据库凭证 | 子进程继承全部环境变量 | 凭证泄露给 agent CLI |

### 建议方向

**方向 A：`CommandExecutor` 加 MinimalEnv**——`buildEnv()` 改为只透传白名单变量（`PATH`、`HOME`、`FORGE_*`、claude 需要的特定变量）。其他变量默认清除。新增 `--preserve-env` flag 允许例外。

```go
// 建议实现
var allowedEnvKeys = map[string]bool{
    "PATH": true, "HOME": true,
    "FORGE_AGENT_DEPTH": true, "FORGE_ROOT": true,
    "ANTHROPIC_API_KEY": true,  // claude 需要
    // ... 明确允许的键
}
```

**方向 B：`Build` 返回的 argv 做 allowlist 检查**——`engine_build.go` 中注册允许的 agent CLI 二进制名（`claude`、`node`、`python3`、`python`）。`CommandExecutor` 在 `Execute` 前检查 `argv[0]` 的 basename 是否在 allowlist 中。用户可以通过 `--agent-cmd` flag 添加自定义允许项——但这会在 log/trace 中生成一条 `decision: agent_cmd_allowed: "custom-binary"` 记录。

**方向 C：文件写权限基于 phase 声明做路径限定**——非 readonly phase 的写权限不应是全目录全开。根据 agent card 的 `emits:` 声明自动推导 allowed write paths。例如：
- `implementer` emits `src/` → 只允许写 `src/` 和 `test/`
- `planner` emits `.agent/ROADMAP.md` → 只允许写 `.agent/ROADMAP.md`
- `reviewer` emits `docs/review/` → 只允许写 `docs/review/`

当前只有 hardcoded readonly 路径（从 agent card 边界段读取）——把它扩展为 **per-phase 写权限声明**。

### 产品价值

ForgeOS 的安全模型当前依赖两件事：(1) claude CLI 自身的 prompt 安全，(2) `--agent-cmd` 指向可信二进制。对于个人项目这足够——你信任你装的 claude。但对于组织部署、CI/CD 集成、和「在不可信环境中运行 AI agent」的场景，这是不够的。

最小权限原则（Principle of Least Privilege）对 AI 自治系统尤其重要——因为 AI 的「意图」不能通过传统的人工安全意识来保证。环境变量最小化（方向 A）+ argv allowlist（方向 B）+ 文件写路径声明（方向 C），三条加在一起，构建了一个**深度防御**的安全边界，而任何一条单独的成本都很低。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 代码影响范围 | 预估 Sprint |
|------|--------|------|------------|------------|
| **一 · 错误灰色地带** | P1 | 韧性/边界 | `internal/orchestrator/exec_error.go` + `command_executor.go` | ~1.5 |
| **二 · project.yml schema** | P1 | 治理完整性 | `.agent/project.schema.yml`(新) + `internal/mode/mode.go` | ~1 |
| **四 · 红线自动门控** | P1 | 治理执法 | `gate.mjs` + `gate-fast.mjs`(新) + `main.go` preflight 接线 | ~1 |
| **三 · Trace 格式进化** | P2 | 可观测性 | `internal/trace` 扩展 + trace 分段归档 | ~2 |
| **五 · 子进程最小权限** | P2 | 安全 | `command_executor.go` + `engine_build.go` | ~1.5 |

**只做一件**：方向二（project.yml schema）——成本最低（一个 JSON Schema 文件 + mode.go 的 20 行校验），收益最高（堵住「拼写错误让整个安全模型静默退化」的缺口），且是 Zero-trust 治理的基线。

**做前三件**：方向二 + 四 + 一——分别解决「配置静默退化」、「红线被动执法」、「错误半死不活」。这三个方向覆盖了 ForgeOS 作为**自治治理系统**的三大根基：配置可信、红线自动、错误可诊断。

方向三和五在团队有真实的长运行 trace 数据（方向三）和多用户部署场景（方向五）后再推进更为合理——它们需要在真实数据/场景中确认优先级。

---

*扫描范围: forge-core 18 Go 包（63 个非测试源文件 + 77 测试文件）· harness 41+ 模块 ·  
`.agent/` 完整骨架（5 workﬂow + 12 agent 卡 + 9 skill 卡）· `ai-dev/` · `examples/` ·  
`pi-batch.py` · `.github/workflows/forge.yml` · 交叉核对 150+ 份已有分析文档 · Sprint 1-31 全记录  
扫描日期: 2026-07-11*
