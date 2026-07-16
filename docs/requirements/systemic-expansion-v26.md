# ForgeOS — 系统性扩展方向 v26：已有功能之外的五个高价值盲区

> **视角**: 资深架构师／产品经理  
> **方法**: 全局深度扫描（18 Go 包 · 130+ 源文件 · 5 工作流 · 39 harness 模块 ·  
>   `.agent` 完整治理骨架 · pi-batch.py · examples/ · 全部 40+ `docs/analysis/*.md` 交叉核对）  
> **基线**: Sprint 31 全状态（FUNCTIONAL_REQUIREMENTS_AUDIT 全部 GAP 已收口，5 引擎落地，  
>   forge-core 零外部依赖全绿，真点火端到端坐实）  
> **核心原则**: **绝不与 40+ 份已有分析文档的核心论点重叠。** 每方向附「未被覆盖的证据」  
>   以证明新颖性。不写代码。  
> **生成日期**: 2026-07-09

---

## 已有 40+ 份分析已覆盖的域（本文不再重复）

全部 31 个 sprint 交付物、`docs/analysis/` 下 40+ 独立分析文档、  
`docs/requirements/` 下 10 份合成分析均已覆盖以下方向（不限于）：

多进程并发安全 · 持久化耐久性语义 · 可观测管道无声故障 · 跨平台可移植性 ·  
运行时自我修复 · Memory 缓存 TOCTOU 竞争 · Prompt 编译器质量保证 ·  
自适应工作流引擎 · 闸门自省 · 增量式治理执行 · 跨项目依赖图谱 ·  
人工决策质量追踪 · 多维模型路由 · 并行调度引擎 · 记忆存储规模演进 ·  
跨厂商模型池 · 统一验证引擎 · 实时可观测性 · 分岔/回滚引擎 ·  
跨工作流管道编排 · 生产就绪度（组合测试/双解析器/故障部分容忍） ·  
边界情况与性能（收敛竞态/长运行泄漏/锁顺序契约） ·  
持久化层数据真实性 · 工程化运营缺口（版本/基准/评分卡） ·  
多实例竞态 · 自适应装配 · Reflect 自分析 · 冷启动分数卡 ·  
信号处理优雅关闭 · 跨相位故障归因 · 执行器多样性 ·  
元治理（自身治理） · 增长瓶颈与包膨胀 · 等等

**本文不再重复上述任何方向。** 本文关注的 5 个方向是系统性的「非功能」盲区——  
它们不是加一个新引擎或新功能，而是**已有功能在规模、时间、多项目和运维视角下的结构性缺口**。

---

## 方向一：运行时数据生命周期管理——持久化状态有生无死

### 类型
运维 · 数据管理 · 容量规划  
**优先级: P1（长期运行的 silently critical）**  
**代码影响**: `internal/trace/` · `internal/memory/` · `internal/persist/` · `cmd/forge/` · 新增 `internal/data/`

### 现状

ForgeOS 在每次运行中生成三种持久化数据，**全部只增不减**：

| 文件 | 生成速率 | 当前容量上限 | 清理机制 |
|------|---------|-------------|---------|
| `.forge/trace.jsonl` | 每 iteration ~5-20 事件 | **无上限** | **无** — 从不删除 |
| `.forge/memory.jsonl` | 每 phase ~1-3 条目 | **无上限** | `memory-prune` 命令（仅手动） |
| `.forge/checkpoint.json.N` | 每 iteration 1 个 | `retain: 3`（唯一有硬上限的） | 自动 rotate（`Save` 的 `retain` 参数） |
| `.forge/scorecards.json` | 每 run 1 次更新 | **无上限** | **无** — 覆盖写入但 Append 不清理 |

代码级证据：

```go
// internal/trace/trace.go: Emit — 永远 append，永不 truncate
func (t *Tracer) Emit(ev Event) error {
    // ...
    _, err := t.w.Write(line) // ★ 无限增长
    return err
}

// internal/memory/memory.go: Append — 同样无限增长
func Append(path string, e Entry) error {
    // ...os.OpenFile(..., O_APPEND|O_CREAT|O_WRONLY...)
    // ★ 每次运行都在追加，没有总大小上限
}
```

```go
// cmd/forge/main.go: cmdMemoryPrune — 唯一清理命令，但只清理 memory
func cmdMemoryPrune(args []string) int {
    // 只处理 memory.jsonl，不碰 trace/checkpoint/scorecard
}
```

### 为什么需要

| 维度 | 解释 |
|------|------|
| **磁盘压力** | 一个 24h evolve loop 生成 ~50-200KB trace + ~30-100KB memory。每月 ~3-15MB。看起来不大，但 ForgeOS 的目标是**无人值守长期运行**（`forge evolve` 可能连续运行数天/数周）。CI 服务器上多项目叠加，磁盘压力是个真实的运维问题 |
| **性能退化** | `memory.Load()` 读整个文件进内存。1000 条 memory 约 150KB，不构成压力。但 10000 条（几个月正常使用）= 1.5MB → JSON 解析 10000 行成为 O(n) 操作。10 万条 = 不可忽略。更重要的是：prompt 构建时注入全部 memory 到 agent 指令中（虽受 `memoryCap: 32` 限制），但 Load 还是要扫描全部文件做 recency/relevance 排序。O(n) 随文件大小线性增长 |
| **运行时间退化** | 第 1 天 forge evolve: memory 10 条，trace 50 条，Load 几乎零开销。第 30 天: memory 300 条，trace 1500 条，Load 成为 prompt 构建的瓶颈组件 |
| **隐私与合规** | memory 包含 agent 对项目代码、架构决策、安全审查的原始观察。当前无数据保留策略，memory 中的信息永远不消失。对于企业用户，这是数据合规问题（GDPR 的「被遗忘权」、金融监管的数据保留期限、内部敏感信息泄露风险） |
| **已存在的先例** | `checkpoint.json` 已有 `retain` 参数——项目已有「保留最近 N 份」的模式，只是未推广到 trace/memory/scorecard |

### 为什么未被已有分析覆盖

- `strategic-extensions-v24-uncovered-frontiers.md` 讨论了 lease 系统的 pidfile+超时+cleanup，但那是 crash 恢复的进程级生命周期，非数据生命周期
- `sixth-wave-multimodel.md` 方向一讨论了 memory compaction 的性能优化方向，但未讨论**数据保留策略**和**自动 eviction 策略**
- `seven-wave-data-realism.md` 讨论了 trace/memory 的数据内容分析，但未从「数据增长管理」角度审视
- **所有已有分析都假设持久化数据是「堆积的资产」而非「需要管理的负债」**——没有一篇分析从容量规划和数据生命周期角度讨论这个问题

### 建议方向

1. **数据大小上限（hard cap）**: trace.jsonl 达到 10MB 后自动 truncate 前半部分（保持最新 N KB），类似日志轮转。memory 行数达到 5000 行后触发 `Compact()`（已有但未自动化）。
2. **基于 TTL 的过期**: memory 条目添加 `ttl_days`（默认 365）或 `expires_at_unix`。`Compact()` 时自动丢弃过期条目。trace 事件按年龄自动 aggragate（超过 30 天的 trace 归约为月统计）。
3. **统一 cleanup 命令**: `forge cleanup [--dry-run] [--all|--trace|--memory|--checkpoint]`，显示每个文件的大小/条目数/最旧条目年龄，支持自动轮转。镜像 `checkpoint.Save` 的 `retain` 模式。
4. **磁盘用量预警**: `forge doctor` 增加 `diskCheck`，当 `.forge/` 目录超过 100MB 或 trace/memory 行数超过阈值时输出 WARNING。
5. **内存加载的流式或分页**: 对于大 memory 文件，`Load()` 可改为仅加载最近 N 天条目（而非全量），或加入 recency-based 抽样。

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|------|
| `forge cleanup` 运行时 forge 也正写入 | 竞争条件 → 截断正在写入的文件 | cleanup 遵循写时原子 rename 模式：读出→过滤→写入 `.tmp` → `rename` 覆盖 |
| 用户依赖旧 trace/memory 做历史分析 | cleanup 删除后无法恢复 | `forge cleanup` 默认 dry-run；`--backup` 选项在被删文件 rename 到 `.forge/archive/` |
| 高 TTL 与合规要求的冲突 | 监管要求保留 7 年但 TTL 设为 90 天 | TTL 为默认值 + `forge cleanup --policy my-corp-policy.yml` 允许外部策略文件覆盖 |

---

## 方向二：CLI 开发者体验（DX）与 Shell 集成——功能完备但体验稀疏

### 类型
开发者体验 · 可发现性 · 工程化  
**优先级: P2（采用率杠杆）**  
**代码影响**: `cmd/forge/main.go` · `cmd/forge/<cmd>.go` · 新增 shell completion 生成器 · 新增结构化输出格式

### 现状

ForgeOS CLI 的当前状态：

| 维度 | 当前状态 | 行业标准对比 |
|------|---------|-------------|
| Shell 自动补全 | **不存在** | kubectl/gh/docker/brew 全部有 |
| 交互式帮助 | `forge <subcommand>` 的 Usage 文本 + flag 列表 | `forge help <subcommand>` 无示例，无详述 |
| Progress 可视化 | 无进度条/旋转器/时间估计 | 任何长运行命令都有（npm/git/docker） |
| 结构化输出 | 只有 `--json`（部分命令）和纯文本 | 所有现代 CLI 都有 `--output yaml\|json\|wide` |
| 颜色/样式 | `forge doctor` 和 `forge status` 有简单颜色 | 整体不统一 |
| 子命令发现 | `forge` 无参数只打 usage，不列出可用项目 | `forge` 或 `forge help` 应等效 |
| 错误时的 UX | Go 原生错误输出（包含 struct 层级细节） | 错误应包含 error code + 人类可读 + 建议 |

代码级证据：

```go
// forge-core/cmd/forge/main.go: usage()
func usage() {
    fmt.Fprintf(os.Stderr, `Usage: forge <subcommand> [flags]

Subcommands:
  run           Run a workflow (one pass)
  evolve        Run a workflow to convergence (unattended loop)
  gate          Run a single harness gate
  ...
`)
    // ★ 只有子命令名 + 一行描述，没有示例，没有链接到文档
}
```

```go
// cmd/forge/main.go: run() — 子命令接受后直接 dispatch
// 任何未识别的子命令只打印 usage 和 exit 2
// 没有「你是指 migrate 吗？」的模糊匹配，没有 --help 对所有 flag 的详细说明
```

运行 `forge run --help` 输出一堆 flag 和它们的类型默认值，但**没有示例**。
运行 `forge`（无参数）和 `forge help` 当前不等效（`forge help` 不存在）。

`pi-batch.py`（独立批处理脚本）有完全不同的 CLI 风格（argparse + YAML），与 forge-core 的 CLI 无任何共享。

### 为什么需要

| 维度 | 解释 |
|------|------|
| **入门门槛** | 一个新的开发者要开始使用 ForgeOS，当前路径是：读 BOOTSTRAP → 读 CLAUDE.md → 读 README → `forge init` → `forge run`。没有 `forge tutorial`、没有 `forge demo`、没有交互式引导。这在「前 5 分钟体验」中失去潜在用户 |
| **长运行的可观测性** | `forge evolve` 可能运行数小时。当前输出只有逐条 log 行。用户不知道：「大概还剩多久？」「当前阶段是什么？」「预计还要跑多少轮？」一个简单的 rotator/progress bar 能极大改善信心 |
| **CI 集成** | CI 场景需要机器可读的输出。当前 `--json` 部分支持但不统一。`forge run build --output json` 应该输出结构化 JSON（包含每个 phase 的名称/状态/时长/资源消耗），使 CI 可以提取指标并决策 |
| **命令发现** | 15+ 子命令。新用户不知道有哪些可用。`forge` → `forge help` → `forge help <cmd>` 的渐进式发现是 CLI 应用的标配 |
| **已有外部命令缺失** | `forge init` 有 scaffold，`forge upgrade` 有 update，`forge run`/`evolve` 有核心循环。但 `forge help`（统一帮助）、`forge version`（已实现但无 `--version` 兼容）、`forge man`（生成 man page）不存在 |

### 为什么未被已有分析覆盖

- `configuration-surface-and-adoption.md` 讨论了配置面的采用障碍（forge tutorial 作为降低门槛的手段）和「如何让用户更容易配置」——但从未系统性地审视整个 CLI 体验
- `fifth-wave-operational.md` 提到 `forge init --interactive`——只是一个命令的交互模式，而非 CLI DX 的全局视角
- `five-extensions-v10-distinct.md` 提到 `forge evolve build --interactive`——同样是单一命令的交互模式
- `execution-semantic-gaps.md` 讨论了 CLI 语义执行的一致性（dry-run vs real），但未触及 UX
- **已有分析讨论的是「命令行功能是什么」，不是「用户用起来怎么样」**——没有人从「CLI 作为产品界面」的角度评估 forge 的可用性

### 建议方向

1. **Shell 自动补全**: 为 bash/zsh/fish 生成 completion script。`forge completion bash` 输出可 source 的脚本。这是 Go CLI 生态的标配（`github.com/spf13/cobra` 内置支持，纯手写 flag 也可自行生成）。
2. **统一结构化和颜色输出**: 所有 `forge <subcommand>` 的子命令支持 `--output text|json`。`forge status --output json` / `forge doctor --output json`。颜色输出从 `forge doctor` 推广到所有命令（`--color auto|always|never`）。
3. **交互式 evolve 仪表盘**: `forge evolve --interactive` 在终端显示实时仪表盘：当前迭代/总迭代、当前 phase、已耗时、预计剩余、gate 状态矩阵、已消耗预算/总预算。使用 ANSI escape codes 原地刷新。
4. **渐进式帮助系统**: `forge` → 列出所有子命令 + 每个一行说明；`forge help <subcommand>` → flag + 示例 + 常见用法；`forge help <subcommand> --examples` → 只显示示例。
5. **模糊子命令匹配**: 当输入未注册的子命令时，进行编辑距离匹配，提示「你是指 `migrate` 吗？」——无需第三方库，Go 标准库的 `strings` 足以实现简单的 Levenshtein。

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|------|
| `--output json` 与已有 `--json` 不一致 | 部分命令已用 `--json`，新系统增加别名 | `--json` 保留为 `--output json` 的 alias，过渡期间都支持 |
| 交互式仪表盘在非 TTY 环境 | ANSI codes 输出乱码 | `--interactive` 在非 TTY 时自动降级为普通 log 输出 |
| Shell completion 生成随版本变化过时 | user 的 .bashrc 引用的是旧版本的 completion | `forge completion` 总是从当前二进制生成，不做预编译 completion 文件 |

---

## 方向三：多项目工作区管理——从单项目治理到项目集群

### 类型
架构 · 多租户 · 运维  
**优先级: P2（组织级采用的瓶颈）**  
**代码影响**: `cmd/forge/main.go` · `internal/persist/` · 新增 `internal/workspace/` · harness 适配

### 现状

当前 ForgeOS 的每个 forge 实例围绕**单个项目根目录**工作：

```
project-root/
  .forge/           ← 全局状态（checkpoint + trace + memory）
  .agent/           ← 治理资产
  harness/          ← 执法器
  src/              ← 项目代码
```

forge-core 的所有路径引用都相对于 `project-root`。`forge run` 和 `forge evolve` 不接受指定不同的项目根。这意味着同时治理两个项目需要两个终端、两套进程、两套 `.forge/` 目录完全独立——没有共享的凭证管理、没有跨项目的预算池、没有集中的路由策略。

关键代码点：

```go
// cmd/forge/main.go: 很多函数硬编码项目根为 cwd
root, _ := os.Getwd()  // ★ 只能治理当前目录

// 或者从 flag 解析，但 runWorkflow 中的路径都相对于 root:
func runWorkflow(root string, wf *asset.Workflow, ...) {
    // 所有文件路径是 root + path
}
```

没有 `forge workspace` 子命令。没有工作区配置文件（类似 `.forge-workspace.yml`）来声明多个项目及其各自的 mode/lifecycle/budget/credentials。

### 为什么需要

| 维度 | 解释 |
|------|------|
| **组织采用** | 一个团队不会只治理一个项目。CI 服务器上同时跑 10 个项目的 `forge evolve` 是真实场景。当前每个项目需要独立的进程管理（自己启动/停止/监控），没有中央仪表盘 |
| **共享资源池** | 组织可能有一个总的月度 AI 预算（例如每个团队 $500/月）。当前每个项目的 budget 是硬编码在 flag 中的。如果项目 A 只用了 $200，项目 B 可以用 $800——但当前无法共享额度。跨项目的路由策略（优先让高价值项目用 Opus）需要集中管理工作区 |
| **凭证隔离** | 不同项目可能有不同的 AI 凭证（claude keys / 不同的 OAuth 授权）。当前凭证从环境变量读取，所有项目共享。无法为项目 A 配置低成本模型、为项目 B 配置高安全模型 |
| **统一运维视图** | `forge status` 只能看当前目录的状态。无法在 CI 控制面板上看到「项目 A: 健康 ✅, 项目 B: 停滞 ⚠️, 项目 C: 预算耗尽 🔴」的聚合视图 |
| **与 ADR-0003 的关系** | ADR-0003（submodule 共享治理资产）解决的是一套治理资产被多个项目**继承**的问题。本方向解决的是多个项目**同时运行**的运行时管理——两者正交互补 |

现有项目中已存在多项目管理的前置条件：
- `internal/persist/` 的 checkpoint/trace/memory 都已经围绕 `root` 参数设计
- `internal/mode/` 的 policy 是纯函数（不依赖全局状态）
- `internal/routing/` 的 routing 同样是纯函数
- 只需要加一个**工作区层**来协调多个 root 的运行时

### 为什么未被已有分析覆盖

- ADR-0003 讨论的是**跨项目治理资产共享**（submodule 机制让多个项目共用一套 `.agent/`），不是**运行时多项目管理**
- `expansion-blind-spots-v16.md` 方向一「多实例并发安全」讨论的是**同一项目**被多个 forge 进程并发操作的风险——与多项目管理是不同问题（一个是并发正确性，一个是组织级工作流设计）
- `expansion-directions-v4-novel-perspectives.md` 的 phase-workspace 概念是**单个 phase 的文件系统隔离**（并行 phase 写不冲突），不是多项目工作区管理
- **已有分析关注「一个项目如何跑得更好」，没有人关注「10 个项目如何一起管」**

### 建议方向

1. **`forge workspace` 子命令**: 新增 `forge workspace init/create/list/status/rm`。工作区是一个 YAML 文件（`.forge-workspace.yml`）声明多个项目及其各自配置。
2. **工作区级共享 budget 池**: 在 `forge workspace run` 中，所有项目的 agent 调用共享一个总预算池。每个项目可设置自己的 `min-reserve`（最低保留预算），剩余预算按 `priority` 分配。
3. **工作区级统一路由策略**: 在工作区配置中声明全局模型路由规则（如「所有 finance/ 下的项目必须用 Opus」），各项目单独的路由策略叠加其上（联合、只升不降）。
4. **凭证映射**: 工作区为每个项目关联一组环境变量或凭证文件。`forge workspace run project-a` 时自动加载 project-a 的凭证集（通过环境变量注入或临时 profile）。
5. **聚合状态**: `forge workspace status --output json` 输出所有项目的健康状态矩阵：上次 converge 时间、当前 mode/lifecycle、gate 状态、预算使用率、trace 事件摘要。

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|------|
| 两个工作区项目同时写各自的 `.forge/trace.jsonl` | 写了不同的目录，无冲突 | 工作区模式下每个项目有完全隔离的 `.forge/` 目录 |
| 工作区配置与项目级配置冲突 | 项目 A 说 `mode: engineering` 但工作区说 `mode: explorer` | 工作区配置为 overlay：工作区配置可 override 项目配置，但 `production lifecycle` 的否决权永远最高 |
| 共享预算池的公平性问题 | 项目 A 耗完所有预算，项目 B 没预算了 | 每个项目声明 `budget.min_reserve_usd`，工作区强制执行预留 |
| 凭证隔离的 security 边界 | 项目 A 的凭证通过环境变量可能被项目 B 的 agent 读到 | 工作区在 `forge run workspace:<name>` 时，只 export 该项目的凭证集到子进程环境 |

---

## 方向四：错误质量、分类与可操作性——失败时用户知道「接下来做什么」

### 类型
开发者体验 · 运维 · 工程化  
**优先级: P1（24h 无人值守运行的前提）**  
**代码影响**: `cmd/forge/` · `internal/orchestrator/` · `internal/doctor/` · 新增 `internal/errors/`

### 现状

ForgeOS 当前的错误处理是**面向机器的**——它告诉用户「什么」（error string），但不告诉用户「为什么」和「怎么办」。

代码级证据：

```go
// internal/orchestrator/exec_error.go: 错误分类已存在，但只用于内部 retry 决策
type ExecError struct {
    Kind    ExecErrorKind  // KindTimeout / KindOverloaded / KindConfig / KindRecursionLimit / KindFailed
    Stage   string
    Message string
}
// 这个结构体有 Kind 分类但没有 ErrorCode、没有 Remediation、没有 UserMessage
```

```go
// cmd/forge/evolve.go: 当 evolve 失败时
return fmt.Errorf("evolve: iteration %d failed: %w", i+1, runErr)
// 没有：错误码、错误原因分类、建议操作
```

```go
// 当 budget 耗尽时：
return fmt.Errorf("agent-call budget exhausted: %d agent-phase executions exceeds the per-run cap of %d (--max-agent-calls)",
    *calls, e.MaxAgentCalls)
// 这只是告诉用户「超了」，但没告诉用户「可以增大 --max-agent-calls」或「check 是否有不必要的 loop-back」
```

**在 24h 无人值守场景中，用户早上醒来看到的是**：
```
forge evolve: iteration 12 failed: agent-call budget exhausted: 15 executions > cap 10
```
然后用户必须自己读源码或文档才能知道：
- 这个错误意味着什么（是真的超出预算，还是 loop-back 导致计数虚高？）
- 应该怎么解决（增大预算、减少 max-loop-back、还是检查 phase 配置？）
- 这个错误是临时的还是永久的（重试有用吗？）

### 为什么需要

| 维度 | 解释 |
|------|------|
| **降低运维疲劳** | 24h 无人值守意味着用户不是专业人士。每次出错都需要一个「可操作」的错误消息，让用户在不读源码的情况下知道下一步做什么 |
| **趋势分析** | 没有结构化的错误码，无法做聚合分析：「过去 30 天最频繁的错误是什么？」「budget 耗尽占了 60% 的失败，是否需要调高默认预算？」 |
| **CI 集成** | CI 系统（Jenkins/GitHub Actions）根据 exit code 判断成功/失败。但 1 和 2 之间没有任何区别——`budget exhausted`（exit 1）和 `config file not found`（exit 1）在 CI 看来完全相同 |
| **与已有工作的关系** | `exec_error.go` 已经有 `ExecErrorKind` 分类。这不是从零开始建新系统，而是**在已有分类基础上增加用户可见层**。需要的只是：ErrorCode + UserMessage + RemediationSuggestion，三者都在 Go 标准库的容忍范围内 |
| **已存在的先例** | `docs/ignition.md` 是整个项目中最好的 UX 文档——它给出了**操作步骤**而非原理描述。错误消息应该遵循同样的原则 |

### 为什么未被已有分析覆盖

- `five-extensions-v10-distinct.md` 提到 AgentExecutor 的错误分类，但那是**内部的 retry 决策**，不是用户可见的错误体验
- `edgecases-and-perf.md` 讨论了并行编排中失败不短路等**架构级错误处理**，但全程不涉及用户看到的错误消息质量
- `expansion-next-frontier.md` 讨论 health dashboard，关注的是系统健康状态而非单次错误的用户体验
- **已有分析讨论错误**时全部从「系统如何处理错误」的内部视角出发，没有一篇从「用户看到错误时怎么办」的外部视角出发

### 建议方向

1. **结构化错误码**: 为 forge-core 和 harness 的每个错误类别分配错误码，格式 `FGE-NNN`（Forge Error）。例如：
   - `FGE-001`: Budget exhausted
   - `FGE-002`: Gate 失败
   - `FGE-003`: Agent 执行超时
   - `FGE-004`: 配置文件损坏
   - `FGE-010`: 未授权（claude 凭证问题）
   - `FGE-011`: 网络错误（API 不可达）
2. **三层错误结构**: 每个错误包含：
   - **What**: 机器可读的错误码 + 简洁描述（如 `FGE-001: Agent-call budget exhausted (15 > 10)`）
   - **Why**: 人类可读的原因（如 `Loop-back from gate "test" re-ran the implementer phase 3 times, consuming 6 of the 15 agent calls`）
   - **How**: 操作建议（如 `To fix: increase --max-agent-calls, or reduce MaxLoopBack, or review the gate configuration to reduce unnecessary loop-backs. See https://forgeos.dev/docs/errors/FGE-001`）
3. **`forge explain <error-code>`**: 新增 `forge explain FGE-001`，输出详细的错误含义、常见原因、解决步骤、相关配置项。这些信息可以嵌入二进制（作为常量字符串）或通过嵌入式 markdown 文档提供。
4. **错误归类到 interface**: 定义 `UserError` interface（`ErrorCode() string; UserMessage() string; Remediation() string; Stack() []Frame`），让所有返回给用户的错误实现这个接口。现有的 `ExecError` 可以包装实现。
5. **JSON 错误输出**: `forge --output json` 模式下所有错误输出为结构化 JSON，包含 error_code、message、remediation、stack（可选）、timestamp。CI 系统可以直接解析。

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|------|
| `UserError` 接口增加 internal 包的依赖 | 当前 internal 包不互相依赖。增加接口会让 internal/orchestrator 依赖一个 errors 包 | 将 `UserError` 定义在 `internal/asset`（最底层的共享包）或独立的 `internal/errors`（新叶子包，零内部依赖） |
| 错误码与社区贡献者的新错误同步 | 新代码可能忘记分配错误码 | `check.py` 增加 `check_error_registration`：强制每个新 `fmt.Errorf` 必须包含一个已注册的错误码，否则 FAIL |
| 错误码膨胀 | 数百个错误码难以管理 | 错误码按模块分层：FGE-0XX（通用）、FGE-1XX（orchestrator）、FGE-2XX（gate/harness）、FGE-3XX（routing/budget）、FGE-9XX（内部/不应暴露给用户） |

---

## 方向五：运行时健康 SLO 跟踪与趋势分析——系统的自省能力

### 类型
运维 · 可观测性 · 持续改进  
**优先级: P2（长期运维的前提）**  
**代码影响**: `internal/doctor/` · `internal/converge/` · `internal/trace/` · 新增 `internal/health/`

### 现状

ForgeOS 为其治理的项目收集大量健康数据（trace events、scorecard、convergence signals、gate results），但**不系统地跟踪自身的运行健康**。

当前可用的健康信号（但未被系统化为 SLO）：

| 信号 | 数据源 | 当前用途 | 我能回答「这是否正常」吗？ |
|------|--------|---------|------------------------|
| 收敛率（converge MET 的百分比） | trace.jsonl + converge.Signals | 仅 per-run 显示 | ❌ 无历史统计 |
| 平均迭代次数 | trace.jsonl | 无 | ❌ |
| Gate 通过率 | trace.jsonl + gateLedger | 仅 per-phase 显示 | ❌ |
| Loop-back 触发频率 | trace.jsonl (kind=gate, detail 分析) | 无 | ❌ |
| 平均 phase 耗时 | trace.jsonl (duration_ms) | scorecard p95 | scorecard 有统计但无趋势 |
| 平均每次运行成本 | cost.go | scorecard avg_cost_usd | scorecard 有统计但无趋势 |
| Memory/磁盘使用 | 文件系统 | 无 | ❌ 只有 `forge doctor` 的快照 |
| 命令失败率 | 无追踪 | 无 | ❌ 完全不追踪 |

`forge doctor` 提供的是**即时快照**（现在健康吗？），不是**趋势分析**（最近一周健康度在变好还是变坏？）。

```go
// internal/doctor/doctor.go: Run — 只做即时检查
func (d *Doctor) Run() []Check {
    // 每个 check 返回 Name + OK(bool) + Detail
    // ★ 不返回历史趋势，不返回与基线的比较
}
```

`forge status --history` 已经能读取 checkpoint chain 显示收敛历史，但只显示收敛状态，不显示更广泛的健康指标。

### 为什么需要

| 维度 | 解释 |
|------|------|
| **长期运行的退化检测** | 系统长期运行后会出现渐进式退化：memory 增长导致 prompt 构建变慢、trace 增长导致 scorecard 变慢、checkpoint rotate 增加 I/O 延迟。没有趋势数据就无法在用户感知前发现退化 |
| **变化的影响度量** | 当修改 routing 策略或 gate 配置时，如何知道修改是改进还是退步？需要「修改前 7 天 vs 修改后 7 天」的收敛率/成本/延迟对比 |
| **预算与效率优化** | 「上周我们花了 $47 在 agent 调用上，收敛了 12 次，平均 $3.9/次」这类数据能指导预算规划。当前只能看到 per-run 成本，无法回答「正常成本应该是多少」 |
| **故障预测** | 当 gate 通过率从 95% 降至 80%、loop-back 频率翻倍时，这些是系统出问题的早期信号。当前这些信号在发生时就消失了，没有趋势告警 |
| **与已有模式的契合** | `checkpoint.json` 已经有 `retain` 参数保留历史。`trace.jsonl` 已经有 duration_ms 和 cost_usd_micros。`scorecards.json` 已经有历史记录。**数据骨架已存在，只差聚合和分析层** |

### 为什么未被已有分析覆盖

- `expansion-next-frontier.md` 方向六「健康仪表盘」提出了 `forge doctor --health` 的概念，但那是**即时健康聚合**（现在健康吗？），不是**健康趋势与 SLO 跟踪**（长期健康在变好还是变坏？）
- `fourth-wave-architecture.md` 提到 `forge doctor` 结果写入 `.forge/health.jsonl`——只是记录的落盘，没有分析和使用
- `go-runtime-health.md` 分析了 Go 包的规模/耦合度——这是代码架构的健康，不是运行时健康
- `sixth-wave-multimodel.md` 讨论了模型选择的 health 指标——这是 routing 的健康，不是系统自身的健康
- **已有分析讨论健康时，都是「系统现在的状态是什么」的快照视角，没有人问「系统的健康趋势是什么」的趋势视角**。快照和趋势是互补但不同的能力

### 建议方向

1. **SLO 定义**: 定义 ForgeOS 运行时自己的 SLO（每 7 天滑动窗口）：
   - 收敛成功率 ≥ 90%（`converge.Signals.Met == true` 的 iteration / 总 iteration）
   - 平均迭代次数 ≤ 5（好的 converge 应该不用太多轮）
   - Gate 首次通过率 ≥ 70%（gate 在第一次尝试就 PASS 的概率）
   - Loop-back 率 ≤ 20%（有 loop-back 的 iteration / 总 iteration）
   - Memory Load 延迟 ≤ 50ms（p99）
   - 磁盘使用增长率 ≤ 100MB/月
2. **`forge health` 子命令**: 新增 `forge health [--slo|--trend|--report]`：
   - `forge health --slo` → 当前窗口 vs SLO 的对比表（通过/警告/违反）
   - `forge health --trend --days 30` → 过去 30 天关键指标的每日趋势（ASCII 图表或 JSON）
   - `forge health --report --output json` → 输出供 CI 消费的结构化健康报告
3. **健康事件写入 trace**: `LoopEngine.OnIteration` 中额外写入 `kind=health` 的 trace 事件，包含 SLO 相关指标。这样 SLO 数据与 run/evolve 的执行轨迹绑定在一起，便于事后分析。
4. **趋势数据库**: 考虑将 `checkpoint.json.N` 的收敛历史 + `trace.jsonl` 的成本/延迟聚合为**每日健康摘要** `.forge/health.jsonl`（每行一个日级别摘要，而非每 iteration 一个事件——大幅减少数据量但保留趋势信息）。
5. **健康告警**: `forge doctor` 在健康指标超出 SLO 阈值时输出 WARNING。`forge status` 在运行健康异常时输出提示。

### 边界情况

| 场景 | 风险 | 缓解 |
|------|------|------|
| SLO 配置不合理导致持续告警 | 用户忽略所有告警（告警疲劳） | 默认 SLO 宽松（收敛率 ≥ 70%），告警只定义 WARNING（非阻塞）。用户通过 project.yml 细化 SLO |
| 小样本偏差 | 仅运行 2 次的系统，收敛率 50%（1/2）看起来很差 | `forge health` 在所有百分比指标旁标注样本量（`convergence_rate: 80% (n=25)`）。样本量 < 10 时不输出 SLO 裁决 |
| 健康数据存储膨胀 | health.jsonl 按天写入，10 年才 3650 行，不是问题 | 健康数据本身的数据量可忽略——趋势分析的主要成本是读取 trace 文件（读取 30 天的 trace 可能需要扫描 10K+ 事件），而非存储健康摘要 |

---

## 汇总：五个方向的优先级矩阵

| # | 方向 | 本质类型 | 影响域 | 严重度 | 代码改动规模 | 已有分析覆盖 |
|---|------|---------|--------|--------|-----------|-------------|
| 1 | 数据生命周期管理 | 运维/容量规划 | 所有持久化文件 | **P1** — 长期运行将 silently 退化 | 中（统一 cleanup + 自动 eviction） | **零** |
| 2 | CLI DX 与 Shell 集成 | 产品体验/可发现性 | 整个 forge CLI | **P2** — 采用率杠杆 | 中（completion + 统一输出 + 仪表盘） | 散见于多篇的附属提及 |
| 3 | 多项目工作区管理 | 架构/多租户 | forge-core 运行时 | **P2** — 组织级采用瓶颈 | 大（新增 workspace 包 + CLI） | **零** |
| 4 | 错误分类与可操作性 | 开发者体验/运维 | 所有用户可见错误 | **P1** — 24h 无人值守的前提 | 小（error code 注册 + 三层错误） | 散见于一篇的内部分类 |
| 5 | 运行时健康 SLO 跟踪 | 可观测性/持续改进 | 运行时运维 | **P2** — 长期运维的前提 | 中（SLO 定义 + health 命令 + 趋势） | 仅快照级提及，无趋势视角 |

### 优先级与依赖关系

```
立即（P1，当前 sprint 可做）:
  方向四 (错误分类与可操作性) —— 定义 `FGE-NNN` 错误码 + `UserError` 接口
  方向一 (数据生命周期) 的子集 —— `forge cleanup` 命令的最小实现（只清查不自动）

1 sprint 内:
  方向一 (数据生命周期) 完整 —— cleanup + 自动 eviction + doctor 磁盘告警
  方向五 (健康 SLO) 基础 —— SLO 定义 + `forge health --slo`

2 sprints:
  方向二 (CLI DX) 完整 —— shell completion + 统一输出 + 交互式仪表盘
  方向三 (工作区管理) 最小 —— `forge workspace init/run` 单项目支持

3+ sprints:
  方向三 (工作区管理) 完整 —— 多项目并行 + 共享预算池 + 凭证隔离 + 聚合仪表盘
  方向五 (健康 SLO) 完整 —— 趋势数据库 + 告警 + CI 集成
```

---

## 被排除的方向与理由

| 方向 | 排除理由 |
|------|----------|
| 跨厂商模型池 | 已有成熟分析覆盖（`sixth-wave-multimodel.md`），且标记为 BLOCKED-EXTERNAL（需多厂商 API keys） |
| 持久化层哈希链完整性 | 已在 `expansion-directions-v14-operational-trust.md` 覆盖 |
| 嵌入式语义检索 | 已在 `expansion-core-five-2026-07-01.md` 覆盖，且标记为 gold-plating（TF-IDF 对 v2 足够） |
| 编排引擎的 Temporal 持久化 | 已在 ADR-0002 + `design.yml` 中 DEFERRED-BY-DESIGN（v3 目标） |
| Web UI | 明确列为架构外方向（ARCHITECTURE.md:45：仍为路线图，偏离 CLI/声明式核心） |
| 独立 agent-os 仓库 | ADR-0003 设计就绪，pending 用户决策（非技术缺口） |
| 测试覆盖率工具链 | 已在 `harness/acceptance-quality.mjs` 中实现为适配器框架 |
| 编排引擎的性能基准测试 | 已在 `fifth-wave-operational.md` 中覆盖（零 benchmark 问题） |

---

*分析基于 forge-core 全量源码扫描（18 Go 包 · 130+ 源文件 · 5 工作流 · 39 harness 模块 · pi-batch.py · examples/）*
*交叉验证 40+ 篇 `docs/analysis/*.md` 和 10 篇 `docs/requirements/*.md`，确认所有方向无重复*
*生成日期：2026-07-09 | 不含任何代码*
