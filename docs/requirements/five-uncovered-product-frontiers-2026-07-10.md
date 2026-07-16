# ForgeOS — 五个未被已有分析覆盖的产品/架构扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫: forge-core（19 Go 包 · ~35k LOC · 纯 stdlib 零依赖）+ cmd/forge（40+ 子命令）+  
>    harness（39+ 模块 · ~10.5k LOC 执法层）+ `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 ·  
>    全部 policies/ADR/DECISIONS）+ examples/ + pi-batch.py + `.github/workflows/`  
> 2. 通读 Sprint 1–31 完整演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（200+ 条目，0 GAP）  
> 3. **差异化验证**: 对每个方向在 85+ 份已有分析文档（docs/requirements/ ~68 篇 + docs/analysis/ ~47 篇）  
>    中做全文关键词检索 + 语义比对，确认该方向的**核心论点从未被作为独立系统性方向展开**。  
>    如某方向的部分子概念曾在某篇文档的侧栏被提及，本文明确标注该已有出处并说明差异点。  
> 4. **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、实际影响、边界场景。  
> **日期**: 2026-07-10

---

## 全景定位

ForgeOS 经过 31 轮 sprint 迭代和 85+ 份独立分析文档的密集覆盖，几乎所有功能域都已被深度扫描：

| 覆盖域 | 代表文档 | 约方向数 |
|--------|---------|---------|
| 引擎补齐（编排/路由/记忆/收敛/信号/并行/回灌） | `high-value-extension-directions*.md` 等 | ~30 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本） | `execution-semantic-gaps.md` 等 | ~12 |
| 生产可靠性（超时/重试/护栏/进程组/自愈/熔断） | `expansion-production-readiness*.md` 等 | ~15 |
| 安全纵深（secret-scan/递归/预算/SCA/Sandbox） | `five-novel-architectural-frontiers*.md` 等 | ~12 |
| 学习闭环（trace/telemetry/scorecard/自适应收敛） | `expansion-five-systemic-learning-loop-gaps.md` 等 | ~12 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/漂移守卫） | `expansion-five-codelevel-architect-gaps.md` 等 | ~10 |
| 结构债（Phase 膨胀/Context Engine/非结构化日志） | `architect-product-perspective-four-structural-gaps.md` | ~8 |
| 运营可信度（Run Identity/状态隔离/审计/健康检查） | `forgeos-trust-operational-maturity.md` | ~8 |
| 二阶伴生（配置爆炸/知识衰减/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md` | ~10 |
| 第三地平线（多仓库联邦/事件驱动/Web UI/LiteLLM） | `expansion-horizon-three.md` | ~8 |
| 命令行/脚本层覆盖（shell completion/统一输出/退出码） | `systemic-expansion-v26.md` | ~3 |
| 管道/工程化（发布工程/CI 加固/binary 分发/版本治理） | `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | ~5 |

**以下方向落在这 85+ 篇已有分析的间隙中**。它们不是「缺失的组件」，而是 **「系统已具备核心能力但缺少产品化最后一英里」** 的缺口 —— 每个方向都对应一个具体的代码级证据，且其核心论点在已有分析中**从未被作为独立方向展开**。

---

## 方向一 · 统一 CLI 配置管理（`forge config`）— 配置散落六处的产品级缺口

> **优先级**: 🔴 **P1（产品采纳）** | **类别**: CLI 基础设施 · 产品体验  
> **已有覆盖检索**: 85+ 篇分析中有关 `forge config` 或「统一配置命令」的独立方向未找到。  
>    `architect-product-perspective-five-directions.md` 方向三提到「配置爆炸」但聚焦于 policy 组合的可维护性问题，  
>    而非缺失一个统一的 CLI 配置管理入口。`fresh-expansion-perspectives.md` 方向二提到「零配置」但面向的是  
>    首次运行体验而非运行中的配置管理。`systemic-expansion-v26.md` 提到「配置验证」但聚焦于 YAML 校验而非  
>    跨文件配置的统一视图。

### 问题描述

ForgeOS 的运行时配置散落在 **至少 6 个不同文件**中，由不同机制加载，无一统一入口：

| 配置 | 文件位置 | 加载机制 | 用户需知 |
|------|---------|---------|---------|
| 项目生命周期 & mode | `.agent/project.yml` | `forge run/evolve` 读取，可被 CLI flag 覆盖 | 知道文件位置、YAML 格式 |
| 模式 × lifecycle 矩阵 | `.agent/policies/modes.yml` | `internal/mode` 包硬编码映射 | 知道文件位置、知道要改哪个字段 |
| 路由策略 | `.agent/routing/policy.yml` | `internal/routing` 包加载 | 同上 |
| Harness 策略 | `harness/policies.yml` | 直接读，harness 自行消费 | 同上 |
| 评估 schema | `.agent/eval/acceptance.schema.yml` | 声明式（仅供阅读） | 同上 |
| workflow YAML | `.agent/workflows/*.yml` | `loadWorkflow` 经 yaml2json 转码 | 多个文件，需理解编排语法 |

**代码级证据**:

```go
// forge-core/cmd/forge/main.go:211-220 — bindRunOpts 从 3 个来源拼合 mode：
//   1. CLI flag --mode（优先级最高）
//   2. 缺省时读 project.yml（resolveLifecycle/lifecycleFromYAML）
//   3. 都缺时硬编码 "balanced" / "mvp"
// 没有一条路径提供 `forge config modes` 或 `forge config project` 来查看/修改这些值。
```

```go
// forge-core/internal/mode/mode.go — mode.Effective(mode, lifecycle) 的输入
// 来自分散的调用点：cmdRun/o.mode、resolveLifecycle()、project.yml 读取。
// 没有任何中央「配置桩」协调这些输入。
```

**为什么需要**:

1. **产品采纳的第一道门槛**: 新用户运行 `forge run build --mode engineering` 后，想持久化这个 mode 却不知道该改哪个文件。没有 `forge config set mode engineering` 命令，用户必须知道 `.agent/project.yml` 的存在和 YAML 语法。

2. **配置漂移无从检测**: mode 可以通过 CLI flag 覆盖 project.yml 的值，但运行时不会检测漂移（如：project.yml 写 `balanced` 但实际在 `--mode engineering` 下跑了 10 次）。没有 `forge config diff` 来发现「声明 vs 实际」偏差。

3. **跨文件一致性无法保证**: `modes.yml` 和 `routing/policy.yml` 的模式名必须一致，但没有任何机制验证。写错一个名字（如 `enigneering` 而非 `engineering`）不会产生任何错误 —— 该模式名只是永远回退到默认值。

4. **Harness 策略和项目配置分离**: `policies.yml` 的 `enforce: block` 是全局的，但 `project.yml` 的 mode 可能只想 `warn`。当前没有任何机制协调这两个「控制面」之间的关系。

**边界场景**:
- 用户通过 CLI flag 覆盖了 mode，但期望 `forge config` 显示「生效值（覆盖来源）」而非仅文件值
- 多文件同字段冲突（如 project.yml 写 `mode: engineering` 但 modes.yml 对该 mode 的定义已过时）
- `forge config validate` 应在修改前就发现跨文件引用断裂

---

## 方向二 · 结构化 CLI 帮助系统与 Shell Completion — 对「开发者工具」的第一印象

> **优先级**: 🔴 **P1（产品采纳）** | **类别**: CLI 基础设施 · 开发者体验  
> **已有覆盖检索**: `systemic-expansion-v26.md` 方向四提到「交互式帮助」和「shell completion」，  
>    但那篇文档的核心论点是「CLI 一致性与结构化输出」作为一个整体方向，帮助系统只是其中一个子维度。  
>    本文将其拆为独立方向，因为它的重要性、独立可实施性、和具体的代码级痛点值得独立展开。

### 问题描述

当前 `forge` CLI 的帮助系统只有一条路径：调用全局 `usage()` 函数输出一段静态字符串。这意味着：

**证据 A — 无子命令级 `--help`**:
```go
// forge-core/cmd/forge/main.go:103-104
if cmd == "-h" || cmd == "--help" || cmd == "help" {
    usage()  // 永远输出全局 usage，不区分子命令
    return 0
}
```
执行 `forge run --help` 和 `forge --help` 输出**完全相同的文本**。用户看不到 `forge run` 独有的 15+ 个 flag 的说明和默认值。

**证据 B — Flag 默认值不可发现**:
```go
// forge-core/cmd/forge/main.go:322-324
func cmdRun(args []string) int {
    fs := flag.NewFlagSet("run", flag.ContinueOnError)
    var o runOpts
    bindRunOpts(fs, &o)  // 绑定 15+ 个 flag，各有默认值
```
`bindRunOpts` 注册了所有 flag 及其默认值，但 `cmdRun` 从不调用 `flag.PrintDefaults()`。这意味着 `--agent-max-budget-usd` 默认是 `""` 还是 `"5"`、`--max-agent-depth` 默认是 `2` 还是 `0` —— 这些信息只存在于源代码中。

**证据 C — 无 shell completion**:
```go
// 全仓零 shell completion 注册。（grep -rn 'completion\|complete\|compgen' forge-core/ harness/ 返回零命中。）
```

**证据 D — 子命令列表不可交互发现**:
```go
// main.go:88-90 — 无参数时：
if len(args) == 0 {
    usage()  // 输出硬编码的静态字符串
    return 2
}
```
没有 `forge list`、没有 `forge --help` 输出带分类的子命令列表。用户无法从 CLI 发现 `forge doctor`、`forge preflight`、`forge memory-prune` 等不常用但有用的命令。

**为什么需要**:

1. **第一印象即产品品质**: ForgeOS 定位是「开发者工具」，而开发者工具的 CLI 帮助系统是第一信任信号。当前 `forge run --help === forge --help` 是低于行业标准的。

2. **减少支持负担**: 15+ 个 flag 的详细说明只能通过读源代码获得。如果每个 flag 能在 `forge run --help` 中通过 `PrintDefaults()` 展示，用户的试错成本大幅降低。

3. **Shell completion 是 CI/CD 集成的先决条件**: 无人值守 24h 运行的 CLI 工具，如果 `forge run [TAB]` 不能自动补全 workflow 名称或 flag，手动配置和调试场景的效率会显著下降。

**边界场景**:
- `forge <unknown-subcommand> --help` 应输出清晰提示 + 列出相似子命令，而不是静默报错
- `forge run --help` 的 flag 说明应标注哪些 flag 是 evolve-only（如 `--max-iter`、`--resume`）
- completion 规则应考虑 forge-core 的零外部依赖约束（go generate 生成 completion 脚本，运行时零依赖）

---

## 方向三 · State 目录生命周期管理 — `.forge/` 的无界增长问题

> **优先级**: 🟡 **P2（长期运行可靠性）** | **类别**: 运维 · 数据管理  
> **已有覆盖检索**: `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向一讨论过  
>    `memory.jsonl` 和 `trace.jsonl` 的「会话生命周期」问题，但那篇文档聚焦于「session ID 与跨会话关联」  
>    而非「磁盘空间管理与数据轮转」。`forgeos-trust-operational-maturity.md` 方向二提到「状态隔离」，  
>    但关注的是多项目隔离而非单项目的磁盘管理。本文方向是站在这两篇之上的补充而非重复。

### 问题描述

ForgeOS 在 `.forge/` 目录中维护三种运行时状态数据，全部**单向增长、无任何轮转/清理机制**：

**文件 1: `memory.jsonl`** — 每行一条 learning loop 记录，从每一次 `forge run`/`forge evolve` 迭代产生：
```jsonl
// 实际内容（已证实）：
{"kind":"lesson","topic":"evolve","detail":"iter 1: roadmap=100%, gates_green=true...","iteration":1,"created_at_unix":1782516099}
```
当前文件中已有 14 条记录（见文件读取），对应 14 次运行。在一个 24h 无人值守的 evolve 任务中，每次迭代产生至少 1 条（可能是 3-5 条），一天可达 **数百至数千行**。文件不轮转、不压缩、不归档。

**文件 2: `trace.jsonl`** — 每次 phase/gate/checkpoint 产生一条 trace 事件。长跑的一次完整 run 可产生 50-200+ 事件。同样无界增长。

**文件 3: `checkpoint.json`** — 仅保留最新 checkpoint（文件内容已验证），因此不会增长。但旧 checkpoint 数据在内存/ledger 中无归档备份。

**代码证据**:
```go
// forge-core/internal/trace/trace.go — appendEvent 始终附加到 trace.jsonl：
// func (t *Tracer) appendEvent(ev Event) {
//     data, _ := json.Marshal(ev)
//     t.file.Write(append(data, '\n'))  // 永远 append，永不 truncate
// }
```

```go
// forge-core/internal/memory/memory.go — memory 使用 jsonl 文件存储记录：
// func (m *Memory) persist(entry Entry) {
//     data, _ := json.Marshal(entry)
//     m.file.Write(append(data, '\n'))  // 同样单向增长
// }
```

```python
# harness/scorecard-update.mjs — scorecard update 追加到 scorecards.json：
# 只从 trace.jsonl 读取、追加写入 scorecards.json，无合并/去重/版本控制。
```

**为什么需要**:

1. **24h 无人值守的静默杀手**: 一个 evolve 任务跑 24h、每 10 分钟一次迭代 = 144 次迭代。每次迭代产生 ~3 条 memory + ~20 条 trace = ~3312 条 jsonl 行（~200KB+）。持续数周的部署可能积累到 **GB 级**，且无任何告警。

2. **JSONL 格式非压缩，不可随机访问**: 当前格式追加写入简单，但读取时必须从头扫描。`memoryContext()` 在构建 prompt 时需要读取全部 memory 记录，随着文件增长，每次 phase 的 prompt 构建时间线性增加（已确认为 `memory.go` 中的 `LoadAll()` 全扫描）。

3. **数据生命周期无策略**: 没有「保留最近 N 条/天/周」的配置。一旦跑了一个大型 evolve 任务，`.forge/` 的大小就永久增长。`forge memory-prune` 命令存在（cmdMemoryPrune），但仅用于 pruning 单个 memory 条目而非制定轮转策略。

4. **scorecards.json 的版本化缺失**: `scorecard-update.mjs` 每次追加但不保留版本，一旦写入错误的 scorecard 数据（如因归因错误产生的假值），无法回滚到上一个已知正确状态。

**边界场景**:
- 磁盘满导致 `forge run` 在 phase 中途因 `write trace.jsonl: no space left` 失败 —— 非幂等失败，checkpoint 不完整
- 跨 session 的 memory 累积使 prompt 膨胀接近 context window 上限（已有一部分 `--max-output-bytes` 防护，但 memory 的输入侧无防护）
- 多个并发 forge 运行写入同一个 `.forge/` 目录产生的行交错（JSONL 单文件并发写无原子性保证）

---

## 方向四 · 结构化错误分类与机器可读输出协议

> **优先级**: 🟡 **P2（CI/CD 集成）** | **类别**: CLI 基础设施 · 运维  
> **已有覆盖检索**: 85+ 篇分析中，「错误分类」常在「执行语义」「可靠性」的侧栏被提到。  
>    `execution-semantic-gaps.md` 方向四提到「错误分类梯度」但聚焦于执行期间而非 CLI 协议。  
>    `five-gaps-from-global-scan-2026-07-10.md` 方向三提到 exit code 约定但聚焦于 harness 而非 forge-core CLI。  
>    尚无分析将「forge CLI 本身的错误输出协议」作为独立方向展开。

### 问题描述

ForgeOS CLI 当前只有 **3 个 exit code**（0=成功、1=已知失败、2=用法错误），错误消息全是自由文本：
```go
// forge-core/cmd/forge/main.go:92-94 — 当前退出码体系：
os.Exit(run(os.Args[1:]))
// run() 返回 0 | 1 | 2，全仓仅此 3 个值
```

**具体问题**:

1. **失败原因完全编码在人类语言中**:
   ```
   forge run: workflow not found: /path/to/nonexistent.yml    ← exit 1
   converge: NOT MET (gates_status) — gate test: FAILED       ← exit 1
   agent-call budget exhausted: 10 > cap 5                    ← exit 1
   ```
   所有失败都是 human-readable 字符串。CI/CD 管道无法区分「workflow 文件缺失」「gate 失败」「预算耗尽」「LLM API 超时」「LLM 返回格式错误」—— 它们全部返回 exit 1。

2. **`--json` 输出缺乏统一标准**:
   - `forge run` 支持 `--json`（`engin_build.go` 有 `--json` flag）
   - `forge accept` 支持 `--json`
   - `forge detect` 支持 `--json`
   - `forge route` **不支持** `--json`
   - `forge validate` **不支持** `--json`
   - 各 `--json` 输出的 Schema 各自不同，无统一 envelope/status/error 字段

3. **错误消息不可翻译/不可聚合**:
   没有错误代码（如 `E_WORKFLOW_NOT_FOUND`、`E_GATE_FAILED`、`E_BUDGET_EXCEEDED`），运维工具无法做：
   - 错误统计（过去 24h 最常见的失败类型是什么？）
   - 错误关联（x 类型的失败是否总是伴随 y 模式的输入？）
   - 自动缓解（遇到 `E_BUDGET_EXCEEDED` 时自动降低 tier 重试）

**为什么需要**:

1. **CI/CD 中的自动化决策**: 当前 `forge accept` 返回 exit 0 或 1，但 CI 无法区分「测试未通过」和「环境问题（如 python 未安装）」。后者应导致 CI **跳过而非阻断**（当前 fork 的 `run_budget_test.go` 已有类似诚实标记，但 exit code 层面未区分）。

2. **多架构 agent 编排的错误传递**: 当 `forge run --executor command --agent-cmd claude` 运行时，agent 的错误（rate-limit、OAuth 过期、context window 超限）被 `CommandExecutor` 统一归化为 `ExecError`，但 exit code 仍然是 1 —— 上层编排器无从知晓具体失败模式以决定重试策略。

3. **Long-tail 问题分析**: 24h 无人值守运行一段时间后，运维需要回答「是什么在消耗我的预算？最常见的中断原因是什么？」。当前只有文本日志，无法做结构化归因。

**边界场景**:
- EE 错误（error code 已知且可恢复，如 rate-limit）应由 exit code 体现（exit 75 = 临时失败，可重试），与 EE 永久失败（exit 1）区分
- `--json` 输出应有统一字段如 `{"error": {"code": "E_BUDGET_EXCEEDED", "message": "...", "details": {...}}}`
- 向后兼容要求: 增加结构化输出的同时，**不改变**当前 exit 0/1/2 行为和 stdout/stderr 文本格式（新用户加 `--json` 才进结构化模式）
- 新增 error code 需要登记到中央 registry（如 `harness/error-codes.json`），避免不同子命令重复编码

---

## 方向五 · Agent Capability Declaration 与自适应指派 — 从「按名分配」到「按能力匹配」

> **优先级**: 🟡 **P2（架构演进）** | **类别**: 编排 · 路由  
> **已有覆盖检索**: `agent-orchestration-five-novel-perspectives.md` 方向三提到「agent 能力声明」但聚焦于  
>    agent 对齐（agent alignment）而非任务匹配。`five-novel-extension-frontiers-v49.md` 方向三提到  
>    「agent 能力注册」但聚焦于分布式场景的 agent 发现。本文方向聚焦于**单仓库内，根据 phase 声明的能力需求  
>    自动匹配最合适的 agent**，与以上两者的切入角度本质不同。

### 问题描述

当前所有 workflow 中的 agent 指派是 **按名硬编码** 的：
```yaml
# .agent/workflows/build.yml
phases:
  - name: planner
    agent: planner           # ← 硬编码 agent 角色名
  - name: implementer
    agent: implementer       # ← 同上
```

每个 agent 卡声明了该 agent 的职责边界，但这些声明在指派时**从未被读取或匹配**：
```yaml
# .agent/agents/implementer.md
role: "Implementer"
boundary:
  reads: [ROADMAP.md, task-specification]
  writes: [src/, test/]
  allowed_tools: [Edit, Write, Bash(node --test*), Bash(node harness/gate.mjs*)]
```

**代码证据**:
```go
// forge-core/internal/routing/routing.go:62 — TierFor 的逻辑：
func TierFor(agent, mode string) string {
    // 只查看 agent 的「名称」来判定 tier floor（architect/cto/reviewer → Opus）
    // 从不读取 agent 卡的 capability 声明
}
```

```go
// forge-core/internal/asset/asset.go — Phase 结构体只带 Agent 字符串字段：
type Phase struct {
    Name    string   // phase 名
    Agent   string   // agent 角色名（字符串匹配，非结构化查询）
    // 零字段描述该 phase 需要什么能力
}
```

**为什么需要**:

1. **Workflow 的可复用性受限**: 当前 `build.yml` 的 `agent: implementer` 写死了。在一个多语言 monorepo 中（如 `examples/go-taskd` + `examples/url-shortener`），两个项目需要的实施能力完全不同（Go 开发者 vs Node 开发者），但 workflow 无法表达这种差异 —— 要么用同一个 agent 卡（无法体现差异），要么为每种语言写独立的 workflow（重复）。

2. **Agent 卡的声明价值未被释放**: 每个 agent 卡已在 `boundary` 段声明了它读/写/用什么工具。但这个结构化信息目前**仅供人类阅读**。如果编排器能读 agent 卡中的 `reads/writes/allowed_tools`，它就可以：
   - 在指派 agent 前验证该 agent 的声明能力是否覆盖 phase 的需求
   - 当声明能力不满足时**在 run 之前就报错**（而不是让 agent 在运行时撞到权限错误）
   - 为 phase 自动选择最匹配的 agent（如果一个 `test-coverage` phase 可以由 `qa` 或 `implementer` 完成，选当前空闲/成本更优的）

3. **未来「人机协作」的桥梁**: 当 ForgeOS 向 v3 演进，需要支持人类 reviewer 或外部工具作为「agent」参与 workflow 时，agent 的 capability 声明将成为匹配的基础设施。

**边界场景**:
- agent 卡声明 `writes: [src/]` 但 phase 需要写 `infrastructure/` —— preflight 应报警
- 两个 agent 卡同时声明了同一能力（如 `qa` 和 `reviewer` 都能做 quality-check）—— 路由按成本/负载/历史择优
- 向后兼容: 当前所有硬编码 agent 名称的 workflow 应继续工作（当无 capability 匹配机制时，退化为按名查找）
- 跨 project.yml lifecycle 不同，同一个 phase 可能需要不同能力的 agent（mvp 下「implementer」可能是全栈，production 下需要区分 frontend/backend/security）

---

## 优先级总结

| 方向 | 优先级 | 类别 | 一句话杠杆 | 实施范围估计 |
|---|---|---|---|---|
| **一 · `forge config` 统一配置管理** | 🔴 P1 | 产品采纳 | 配置散落 6 个文件，无统一查看/修改入口，阻挡新用户（低成本高回报） | 新 CLI 子命令 ~2-3 文件 |
| **二 · 结构化帮助与 shell completion** | 🔴 P1 | 产品采纳 | `forge run --help === forge --help` 低于行业标准；无 completion（低成本高回报） | `usage()` 重写 + `go generate` completion |
| **三 · `.forge/` 生命周期管理** | 🟡 P2 | 长期可靠性 | JSONL 无界增长，24h 任务可产生数千行；磁盘满导致非幂等失败 | `trace/memory` 包加轮转策略 |
| **四 · 结构化错误分类与输出协议** | 🟡 P2 | CI/CD 集成 | 目前只有 0/1/2 三个 exit code，错误原因编码在人类语言中 | `errors` 包 + CLI 输出层改造 |
| **五 · Agent 能力声明与自适应指派** | 🟡 P2 | 架构演进 | agent 卡的能力声明仅供阅读；指派是硬编码字符串而非结构化匹配 | `asset.Phase` 加 Capability 字段 + 匹配引擎 |

**成本-收益判断**:
- **方向一和方向二**是产品采纳的「低垂果实」: 纯 CLI 层改造，不动编排逻辑，不改运行时行为，但能显著改善第一印象。
- **方向三**是「静默危机」: 在 24h 无人值守场景规模化前，`.forge/` 的管理属于「现在修成本低，以后修成本高」的运维债务。
- **方向四**是「基础设施投资」: 为 CI/CD 和自动化运维铺路，但需要定义完整的 error taxonomy（不仅是加几个常量）。
- **方向五**是「架构演进」: 不是当前阻塞项，但 agent 卡的声明价值目前完全未被利用，且为 v3 的「跨厂商 agent 池」提供基础匹配原语。

> **Honesty 声明**: 本文 5 个方向的核心论点在 85+ 篇已有分析中未被作为独立系统性方向展开。但部分子概念（如方向一的「配置可维护性」、方向二的「CLI 一致性」、方向三的「状态文件增长」、方向四的「退出码约定」、方向五的「agent 能力注册」）在若干分析的边缘段落中有所触及。本文在每方向开头标注了与这些已有覆盖的关系，避免虚假的「从未被提到」声明。
