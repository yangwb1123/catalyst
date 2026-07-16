# ForgeOS — 下一前沿：未被触及的五个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深扫（forge-core 17 包 100 Go 源文件 · harness 26+ 模块 · `.agent/` 全套治理 · 22 份 docs/analysis 已有分析文档逐一核对）  
> **原则**: 每方向带 `file:line` 代码证据链，不写实现代码，不与已有分析重叠  
> **基线**: Sprint 26 全状态（方向一~五全交付 + 真点火 multi-agent 端到端坐实）  
> **日期**: 2026-07-01

---

## 已有 22 份分析已覆盖的域（确认不重复）

逐一核对后确认以下方向已有深度分析，本文不再展开：

| 域 | 对应文档 |
|----|---------|
| Agent 沙箱 / Firecracker 隔离 | `next-horizons.md` 方向二、多篇 expansion |
| 跨厂商模型池 / Provider 抽象 | `next-horizons.md` 方向一、多篇 expansion |
| 语义检索 / Embedding pipeline | `next-horizons.md` 方向三、`edgecases-and-perf.md` §4 |
| 跨 Workflow 编排 / 脊柱自动串联 | `next-horizons.md` 方向四 |
| 混沌工程 / 崩溃恢复测试 | `next-horizons.md` 方向五 |
| 事件驱动 / Webhook / 异步外部触发 | `expansion-directions-v4.md` 方向一 |
| 并行 Agent 输出合并与冲突解决 | `expansion-directions-v4.md` 方向二 |
| 人类反馈分析系统 | `expansion-directions-v4.md` 方向三 |
| 确定性 Replay / Agent 行为调试 | `expansion-directions-v4.md` 方向四 |
| 成本预测与预算规划 | `expansion-directions-v4.md` 方向五 |
| 并行编排竞态与 errgroup 短路 | `edgecases-and-perf.md` §1 |
| Trace 无限增长 / 轮换策略 | `edgecases-and-perf.md` §2.1 |
| Memory 每相位全量读文件性能 | `edgecases-and-perf.md` §2.2 |
| 收敛门闩效应 / 假收敛 / 零相位 | `edgecases-and-perf.md` §3 |
| Prompt 构建序列化瓶颈 | `edgecases-and-perf.md` §4 |
| 测试退化 / 代码无测试 / 治理盲区 | `edgecases-and-perf.md` §5 |
| Cross-Agent Prompt 注入防护 | `expansion-directions-v6.md` 方向一 |
| 置信度感知决策引擎 | `expansion-directions-v6.md` 方向二 |
| 自愈层运行时 | `expansion-directions-v6.md` 方向四 |
| 架构度量趋势分析 | `expansion-directions-v6.md` 方向五 |
| 持久化格式版本化 | `scan-new-angles.md` 方向一 |
| 元治理 / 自身治理 vs 产出治理差距 | `expansion-forgeos-meta-governance.md` |
| 增长瓶颈（cmd/forge 膨胀 / Engine 字段增长） | `growth-bottlenecks-and-scalability.md` |

> **结论**: 绝大多数「明显方向」已被前人覆盖。本文的 5 个方向是 **系统在无人触及的深层架构假设中藏着的隐藏缺口**。

---

## 目录

1. [方向一：多实例工作区冲突 —— 两把 `forge run` 不能同时跑在同一仓库](#方向一多实例工作区冲突)
2. [方向二：冷启动零到一引导 —— 先天没有 ROADMAP/ADR/Memory 的第一轮](#方向二冷启动零到一引导)
3. [方向三：能力感知 Agent 路由 —— 不只看模型档次，还要看 Agent CLI 能做什么](#方向三能力感知-agent-路由)
4. [方向四：记分卡统计可靠性 —— 当 quality_score 只剩噪声时](#方向四记分卡统计可靠性)
5. [方向五：运行时遥测自检 —— 系统应该能发现自己出问题了](#方向五运行时遥测自检)

---

## 方向一：多实例工作区冲突 —— 两把 `forge run` 不能同时跑在同一仓库

**优先级**: P1  
**类别**: 边界（正确性 · 生产部署）  
**一句话**: 两个 `forge evolve` 同时跑在同一 `.forge/` 目录下，相互污染 trace/checkpoint/memory——当前零防护。

### 现状

ForgeOS 的所有运行时状态都存储在 `<root>/.forge/` 中：

| 文件 | 用途 | 访问模式 |
|------|------|---------|
| `.forge/trace.jsonl` | 所有 phase 的 trace 事件（cost/ latency/ gate） | APPEND，全程保持 open fd |
| `.forge/checkpoint.json` | 当前迭代+phase 索引的恢复点 | 原子 OVERWRITE（temp+rename） |
| `.forge/memory.jsonl` | 跨 session 知识（gap/decision/lesson） | APPEND，每 iteration 写入 |

**无任何机制防止多进程冲突**。

具体冲突模式：

**冲突 1：双 `forge evolve` 交错写入 trace.jsonl**

```go
// forge-core/internal/trace/trace.go (简化)
func (t *Tracer) Emit(e Event) {
    t.mu.Lock()
    defer t.mu.Unlock()
    line, _ := json.Marshal(e)
    t.writer.Write(append(line, '\n'))      // O_APPEND 下单行 <= 4096 是原子的
    t.writer.Sync()                          // 但 Sync 不保证跨进程可见性
}
```

`O_APPEND` 保证 POSIX 下单行写入是原子的（<= PIPE_BUF），但 `Sync()` 不是。进程 A 的 Emit 与进程 B 的 Emit 交织后，两进程各自的 `t.mu` 互不感知——trace.jsonl 中两个 run 的事件交错排列，导致 `scorecard_wind.go` 的 `traceHasModelCost` 全文件扫描读到**属于不同 run 的事件混合**。

**冲突 2：checkpoint.json 覆盖**

```go
// forge-core/internal/persist/checkpoint.go
func (c Checkpoint) Save(path string) error {
    tmp, _ := os.CreateTemp(dir, "checkpoint.*.tmp")
    json.NewEncoder(tmp).Encode(c)
    tmp.Sync()
    os.Rename(tmp.Name(), path)            // 原子替换
}
```

进程 A 完成 iteration 3 写入 checkpoint → 进程 B 完成 iteration 1 写入 checkpoint（`rename` 原子覆盖）。恢复时进程 A 从 checkpoint 看到 iteration 1——丢失了 2 个 iteration 的进度。

**冲突 3：memory.jsonl 中的 session 边界丢失**

`memory.Append` 每行是 `{"kind":"finding", "iteration":3, ...}`。两个 evolve 同时跑会写出 `iteration:3` 的行属于哪个 run？没有 `run_id` 字段。memory 的 `Query`/`Load` 无法区分不同 run 的记忆，一个 run 的 gap analysis 会混入另一个 run 的数据。

### 为什么需要

1. **CI + 本地开发并行**: 开发者本地跑 `forge run build --executor dry` 测试，同时 CI 跑 `forge accept`。两者都用 `.forge/` 目录——CI 的 trace 事件混入本地测试，本地的 checkpoint 覆盖 CI 的。Sprint 26 CI 有明确多步（go build / test / forge accept / forge run），每一步都可能触发 forge 写 `.forge/`。

2. **夜间长跑 + 日内开发**: `forge evolve` 24h 无人值守跑在服务器上，开发者日间本地 `forge run build --approved` 测试新功能——共享 `.forge/` 目录后夜间 run 的 trace 被日间 run 污染。

3. **微服务多仓编排（未来方向）**: 如果 ForgeOS 同时 evolve 多个子项目（仓库 A、B、C），每个都有自己的 `.forge/`。但编排者（runner）自身的状态存在哪？如果也存在某处的 `.forge/`，冲突又会出现。

### 代码证据

| 文件 | 行号 | 证据 |
|------|------|------|
| `forge-core/internal/trace/trace.go` | `Emit` / `NewTracer` | O_APPEND 单文件，无 run_id 字段标识行所属 run |
| `forge-core/internal/trace/trace.go` | `Event` struct | 无 `run_id` / `session_id` |
| `forge-core/internal/persist/checkpoint.go` | `Save` | 固定路径覆盖，无进程锁 |
| `forge-core/cmd/forge/main.go` | `openTracer` | APPEND 模式 open，不排他 |
| `forge-core/cmd/forge/evolve.go` | `cmdEvolve` | 无 pidfile / 文件锁 |
| `forge-core/internal/memory/memory.go` | `Append` / `Load` | 无 session 隔离，iteration 号唯一关联维度 |

### 建议方向

- **短期（零代码）**: 文档化限制——声明 `.forge/` 不支持并发访问，建议每个 run 用 `FORGE_REPO_ROOT` 隔离工作区。
- **中期**: 每 run 加 `run_id`（UUID），trace/memory 行携带 `run_id` 字段，Emit 前过滤。checkpoint 加 `run_id` 标识所有者。
- **长期**: 引入进程级文件锁（`flock(2)` / `LockFile`）——`forge run`/`evolve` 启动时尝试获取 `.forge/.lock`，获取不到则 exit 1（或 `--wait` 排队）。

---

## 方向二：冷启动零到一引导 —— 先天没有 ROADMAP/ADR/Memory 的第一轮

**优先级**: P1  
**类别**: 边界（功能完整性 · 用户体验）  
**一句话**: 一个 `forge init` 出来的新项目，第一次 `forge run build` 产出的 prompt 里 ROADMAP 为空、ADR 零条、memory 空文件——agent 得到的上下文和百次运行后完全一样，但信息量为零。

### 现状

`forge init`（`harness/scaffold/forge-init.mjs`）复制了一整套 `.agent/` 模板：

```yaml
# .agent/ROADMAP.md（脚手架模板）
# ForgeOS ROADMAP — 项目交付计划
# （由 DESIGN 阶段产物自动物化，或在 Build 前手工准备）
```

即只有注释，无实质内容。但 `prompt.Gather` 对 ROADMAP.md 是**无条件读取**：

```go
// forge-core/internal/prompt/prompt.go:49
func currentTask(repoRoot string) string {
    b, _ := os.ReadFile(filepath.Join(repoRoot, ".agent", "ROADMAP.md"))
    return capRunes(strings.TrimSpace(string(b)), taskCap)
}
```

当 ROADMAP 只有注释/模板头时，`currentTask` 返回空字符串。`Gather` 的任务 lane 就是空的——agent 听到了角色卡和约束，但不知道**具体要做什么**。

同理，`relevantADRs` 读 `docs/adr/` 目录，空目录返回 nil——ADR lane 空。`constraints` 读 AGENTS.md，如果 `forge-init` 复制了模板 AGENTS.md 有内容，这是唯一的非空 lane。

**结果**: 第一个 `forge run build` 的 prompt 缺失两个关键信息源（任务 + 架构决策），只剩下通用约束。agent 的输出质量与一个「知道要做什么」的 phase 有系统性差异。

**更隐蔽的影响**:

1. **Scorecard 首次运行不产生有用数据**: 记分卡建立在 `（model, task_type）` 主键上。第一次运行的 quality_score 由「缺乏任务信息的 agent 产出」评测——这个低质量分会被 `exponential decay` 保留数周，影响后续模型路由决策。第一次运行的质量数据是**系统性地偏低的**，但系统不识别「这是第一次」。

2. **Loop-back 在第一次运行中几乎肯定触发**: 没有 ROADMAP 的情况下 agent 写代码（或什么都不写），gate 必然红——loop-back 触发。MaxLoopBack=3 很快就烧完了，第一次运行以 fail-closed abort 结束，用户得到的是「build workflow 失败」——而不是「请在 ROADMAP 中定义任务」。

3. **Memory 从空文件开始积累**: `memory.Load("memory.jsonl")` 读空文件返回 `[]Entry`。`memoryContext` 生成 `""`——没有历史可学，也没有「这是第一次运行」的通知。

### 为什么需要

1. **零经验用户的第一印象**: `forge init` → `forge run build` 应该给出**有意义的输出**而非「gate failed 3 times, abort」。当前行为对尝鲜用户极不友好——没有指导、没有「你需要先定义 ROADMAP」的错误提示。

2. **Scorecard 初次污染不可逆**: 质量分 decay 周期 30 天。一次「无任务盲写」的低质量产出留在记分卡里——30 天内的路由决策都会被它拖低。

3. **自洽性问题**: ForgeOS 的「从 Idea 到 Production」脊柱是 `DISCOVER→DESIGN→BUILD→EVOLVE`。但 `forge init` 创建的项目不在脊柱的起点（DISCOVER），而是在 BUILD 的入口——产品管理者期望的「先做需求探索」被绕过。`forge init` 预设了 mode=engineering、lifecycle=mvp，却没有自动触发 discover 或 design。

4. **ADR 检索在零 ADR 下退化为无意义调用**: `adrTitles` 每相位做 `readdir` 扫描 `docs/adr/`——返回空后 `Retrieve(docs=[], query, 6)` 立即返回空。每次 phase 的 readdir + TF-IDF 全白做。当项目有 ADR 后不浪费，但零 ADR 的 N 次迭代里浪费 N × phases 次 IO。

### 代码证据

| 文件 | 行号 | 证据 |
|------|------|------|
| `forge-core/internal/prompt/prompt.go` | 49-57 | `currentTask` 无条件读 ROADMAP，模板空内容=空字符串 |
| `forge-core/internal/prompt/prompt.go` | 72-75 | `relevantADRs` 空目录=空输出，无降级提示 |
| `forge-core/cmd/forge/prompt_context.go` | `buildPrompt` | 无「首次运行」检测，无补偿注入 |
| `forge-core/cmd/forge/engine_build.go` | `buildRunEngine` | 无上下文丰富度预检 |
| `harness/scaffold/forge-init.mjs` | 全局 | 模板 ROADMAP 仅含注释行 |
| `forge-core/internal/converge/converge.go` | `gatherSignals` | `RoadmapCompletion` 来自 agent 自报告，非文件内容检测 |

### 建议方向

- **首次运行检测**: `persist.Checkpoint.Load` 返回 `(Checkpoint, bool)` 表示「从未运行过」。`forge run` 在首次运行时输出引导信息：「ROADMAP 为空——请先运行 `forge run discover` 做需求探索，或编辑 `.agent/ROADMAP.md` 填入首个任务」，并优雅退出（exit 1 而非 fail-closed abort）。

- **上下文丰富度仪表盘**: 新增 `forge doctor --context` 子命令，报告当前项目上下文的完整度：
  ```
  forge doctor --context
    ROADMAP: ❌ 空（仅模板注释）
    ADRs:    ❌ 0 条（docs/adr/ 目录不存在）
    Memory:  ⚠️ 0 条（未积累）
    Scorecards: ⚠️ 0 条（无历史路由数据）
    → 建议: 先运行 forge run discover 做需求探索
  ```

- **Scorecard 首次标记**: scorecard schema 增加 `first_n` 标记位，前 N 次运行的结果带标记。`HistoryTiebreak` 在无足够非 `first_n` 样本时退化为 tier_default（不依赖首次低质量数据）。

- **引导性 gate 消息**: `callGate` 在检测到空 ROADMAP 时，对 `test`/`build` 等 gate 报告特制的 N/A 消息而非 blind FAIL：「ROADMAP 未定义→无任务可验证→gate 不适用」，与「测试红」语义区分。

---

## 方向三：能力感知 Agent 路由 —— 不只看模型档次，还要看 Agent CLI 能做什么

**优先级**: P2  
**类别**: 核心功能（路由深度）  
**一句话**: 当前路由只看 `model_tier`，但不同 Agent CLI（Claude/Gemini/OpenHands）的能力天差地别——选择 Agent 时真正需要的是能力匹配，不是档次匹配。

### 现状

ForgeOS 的路由模型是一个二维矩阵：

```
risk/complexity → tier (haiku/sonnet/opus)
mode×lifecycle → floor (minimum tier)
budget → adjust (降档/升档)
```

路由输出是一个模型名：`claude-sonnet-4-20250514`。

但现实中，Agent CLI 的能力远不止「智能档次」：

| 能力维度 | Claude `-p` | Claude `--permission-mode=acceptEdits` | Gemini CLI | OpenHands |
|---------|-------------|--------------------------------------|------------|-----------|
| 写文件 | ❌（print 模式只输出文本） | ✅ 自动接受编辑 | ✅ | ✅ |
| 读文件 | ✅（上下文够就能读） | ✅ | ✅ | ✅ |
| 执行命令 | ⚠️ 需 `allowedTools` + 人批准 | ⚠️ 需 `allowedTools` | ✅（默认 Bash） | ✅容器内任意 |
| 网页搜索 | ❌ | ❌ | ✅（Google 搜索） | ❌ |
| 容器管理 | ❌ | ❌ | ❌ | ✅（Docker） |
| 跨文件重构 | ✅ | ✅ | ✅ | ✅ |
| 使用 MCP 工具 | ✅ | ✅ | ⚠️ | ⚠️ |

**当前路由的盲区**:

盲区 1：`build.yml` 的 `planner` phase 需要**只读**——读 ROADMAP + ADRs，输出任务拆分。这个 phase 不需要写文件能力。用 `claude -p`（print mode）就够——但当前强制 `--permission-mode=acceptEdits`（`defaultAgentAllowedTools`），给了一个不需要写文件的 phase「写权限」，增加了攻击面。

盲区 2：`evolve.yml` 的 `scan` phase 如果能做网页搜索（Gemini CLI 的天然能力），可以自动搜索项目依赖的最新 CVE、对比竞品更新——但当前 routing 把所有 phase 送到 claude，Claude 无网页搜索能力，这个价值从未兑现。

盲区 3：`qa` phase 需要运行测试并解析结果。如果 agent 本身不能 `node --test`（print mode 下受限于 `allowedTools`），它只能靠看代码做静态判断——当前的 `gateLedger` 注入 gate 结果弥补了这一点，但 gate 结果的信息量远小于真实运行测试。

### 为什么需要

1. **能力与任务不匹配导致浪费**: planner 不需要写权限却给了 `acceptEdits`（安全隐患）；scan 需要搜索能力但只有 claude（无搜索）；implementer 需要写能力但 print mode 下只能靠 `allowedTools` 白名单曲折实现。**三方向都错了**。

2. **Cross-vendor 的真正价值在能力差异，不在价格差异**: `next-horizons.md` 方向一（跨厂商池）的论证基于成本弹性。但 Gemini 真正不可替代的价值不是便宜——是**它能网页搜索**。如果路由层不理解能力差异，跨厂商只是「多一个便宜模型」，而非「多一个能力维度」。

3. **Advisor 模式的空心化**: `modes.yml` 定义了 `cto: advisory`（只产 PRD/Arch，不写代码）。但当前路由在 advisory 模式下只是降低 tier（haiku），跑 review 的都是 haiku——却用了与 engineering 相同的 `allowedTools`。Advisory 模式的本质是「不修改代码，只输出分析」——应该用只读的工具集（print mode + web search），而非同工具只换模型。

4. **能力声明只有注释**: `allowedTools` 写在 `defaultAgentAllowedTools` 常量的注释里——不是数据，不是架构，只是文字。`harness-gates` phase 甚至不需要 agent（实际是 toolchain runner），但 `agent` 字段设为 `"harness"` 并走了 `runAgentPhase` 路径。

### 代码证据

| 文件 | 行号 | 证据 |
|------|------|------|
| `forge-core/cmd/forge/main.go` | 35-40 | `defaultAgentAllowedTools` 注释自述安全策略——仅是文字 |
| `forge-core/internal/routing/routing.go` | 136-166 | `TierFor` 只返回 tier 字符串，无能力元数据 |
| `forge-core/internal/routing/routing.go` | 170-183 | `ModelMap` 只映射 provider→model name |
| `forge-core/cmd/forge/engine_build.go` | 41-89 | `claudeArgv` 对所有 phase 通用——planner 和 implementer 收到相同权限 |
| `forge-core/cmd/forge/prompt_context.go` | `buildPrompt` | prompt 声明 tier 但不声明可用工具/权限 |
| `.agent/workflows/build.yml` | `harness-gates` | agent 为 `"harness"` 但走了 agent 执行路径 |
| `.agent/workflows/evolve.yml` | scan | agent 为 explorer，但跑在无搜索能力的 claude 上 |

### 建议方向

- **能力声明层**: 在 `.agent/routing/capabilities.yml` 或 `routing.go` 的 `Provider` 结构体中增加能力向量（`can_write_files`、`can_search_web`、`can_run_commands`、`can_use_mcp`），每个 provider 声明自己的真能力。

- **Phase 能力需求**: 在 `asset.Phase` 中增加可选字段 `requires_capabilities: [web_search]`。`forge run` 在调度前做能力匹配——找不到能满足所有要求的 agent 时，降级（输出去能力最接近的）或报错。

- **路由升维**: 当前 `TierFor` 返回 tier 后 `ResolveModel` 给具体模型。改为 `(capabilities, tier) → agent_cmd` 双维匹配——先过滤能力匹配的 providers，再从中选最优 tier。

- **权限粒度下沉**: `allowedTools` 从 `main.go` 常量和 `--agent-allowed-tools` flag 下沉到 `asset.Phase` 或 `routing.Policy`——不同 phase 天生需不同工具集，不应由 CLI flag 一刀切。

---

## 方向四：记分卡统计可靠性 —— 当 quality_score 只剩噪声时

**优先级**: P2  
**类别**: 性能 + 正确性（数据质量）  
**一句话**: 记分卡是学习闭环的核心数据资产，但 quality_score 是单点无置信区间的点估计——一次异常能偏置路由决策数周。

### 现状

记分卡系统（`routing/scorecard.go` + `scorecard_wind.go` + `.agent/routing/scorecards.json`）已全链路接通。数据流：

```
agent phase → cost/quality/latency → scorecard-update.mjs → scorecards.json
                                                                    ↓
next run → LoadScorecards → HistoryTiebreak → pick best model
```

核心数据结构：

```json
{
  "model": "claude-sonnet-4-20250514",
  "task_type": "implement",
  "quality_score": 0.85,
  "avg_cost_usd": 0.18,
  "p95_latency_ms": 2640,
  "sample_count": 12
}
```

**三个统计学问题**：

**问题 1：无置信区间**

`quality_score: 0.85` 是 12 个样本的均值。但标准差是多少？置信区间的宽度是多少？

```
模型 A: quality_score=0.85, 样本=12, 标准差=0.02（很稳定）
模型 B: quality_score=0.84, 样本=12, 标准差=0.15（波动大）
```

`HistoryTiebreak` 看到 A(0.85) > B(0.84) 就选 A。但 A 和 B 的差异 0.01 远小于 B 的单次波动 0.15——统计上不显著。实际结论是「无法区分 A 和 B」，但路由层做出了「A 更好」的确定选择。

**问题 2：无最小样本量门槛**

当前 `LoadScorecards` 不检查 `sample_count`。1 个样本的 `quality_score=1.0` 直接参与择优：

```javascript
// routing/scorecard.js (概念)
function HistoryTiebreak(candidates) {
    // 对每个候选模型，取加权 quality_score
    // 新模型的 1 个样本可能和老模型的 100 个样本平权
}
```

一次幸运的运行可以让一个新模型以 1.0 分碾压老模型的 0.85（100 样本）——实际上新模型的真实质量未知。

**问题 3：异常值无过滤**

一个 phase 因为 prompt 构造错误写了无意义代码 → quality_score 0.2。即使 99 个正常 phase 都是 0.85+，这个 0.2 的异常值通过指数衰减能在 30 天内拖低加权平均。而没有**异常值剪除（outlier trimming）**机制。

### 数值示例

```
场景：Sonnet 平稳运行，第 3 天出现一次 prompt 注入导致的差输出
之前: quality_score=0.85, 样本=50, 指数衰退因子 ~30 天半衰期
第 3 天: quality_score=0.15, 样本=51 → 加权均 ≈ 0.84（1/51 的低分影响不大）
但: 如果第 3-30 天系统切换评估者（新 reviewer agent），质量评分标准变了：
    第 1-2 天: 旧 reviewer 评分宽 → 平均 0.90
    第 3-30 天: 新 reviewer 评分严 → 平均 0.70
    记分卡显示 quality_score 从 0.90→0.70 是"质量下降"？
    还是"评估标准变了"？——系统无法区分。
```

**问题 4：评估者偏移（Rater Drift）**

`quality_score` 来自 `qa` agent 的产出——但 QA agent 本身是 LLM，它的评判标准随版本/温度/prompt 构造变化。同一段代码今天打 0.8、明天打 0.7——变化的是评估者，不是被评估者。

`scorecard.json` 无 `evaluator_version` / `evaluator_model` 字段——无法回溯评分偏移。

### 为什么需要

1. **学习闭环的信噪比**: 如果路由层的「择优」决策建立在统计不显著的噪声上，整个学习闭环（Eval→Scorecard→Route）的信噪比是负的——越学越差。这比「不学」更危险。

2. **跨厂商场景下不可比**: Sonnet 在 sonnet 评估者下的 0.85 vs Gemini 在 gemini 评估者下的 0.82——差异是模型还是评估者？当前记分卡无法回答这个问题。

3. **长期趋势 vs 短期波动**: 记分卡的 `exponential decay` 给出了时间加权，但没有趋势分析——质量是在改善还是在退化？一个模型从 0.85 降到 0.70（90 天趋势）比从 0.85 降到 0.84（单次波动）需要完全不同的路由响应。

### 代码证据

| 文件 | 行号 | 证据 |
|------|------|------|
| `.agent/routing/scorecard.schema.yml` | 全局 | schema 无 `confidence_interval`、`std_dev`、`min_samples`、`evaluator` 字段 |
| `harness/scorecard-update.mjs` | 全文件 | 写入 `quality_score` 为单点值，无方差计算 |
| `harness/scorecard.mjs` | `merge` | 加权平均无异常值剪除 |
| `forge-core/internal/routing/scorecard.go` | `HistoryTiebreak` | 权重比较无统计显著性检查 |
| `harness/scorecard.mjs` | `decayWeight` | 指数衰减半衰期外无常驻 alert |
| `.agent/routing/scorecards.json` | 现有数据 | 无评估者信息 |
| `forge-core/cmd/forge/scorecard_wind.go` | `windDownScorecards` | 无 `evaluator_version` 写入 |

### 建议方向

- **置信区间**: scorecard schema 增加 `quality_stddev` 和 `sample_count`，`HistoryTiebreak` 在 `|μA - μB| < 2×√(σA²/nA + σB²/nB)` 时返回「无显著差异」而非选最优。

- **最小样本量门槛**: `ScorecardEntry` 增加 `min_samples: 5`（跨 mode×task_type 可配）。不足时该条目不参与路由择优——退化为 tier_default。

- **异常值剪除**: `merge` 中实现 Tukey's fences（IQR 法）：低于 Q1 - 1.5×IQR 或高于 Q3 + 1.5×IQR 的 quality_score 在加权前剪除。标记剪除次数以便审计。

- **评估者版本追踪**: scorecard schema 增加 `evaluator: { model, version, prompt_hash }`。`windDownScorecards` 在写入时附上评估者信息。路由层在检测到评估者变化时发出 `[INFO] evaluator changed from X to Y — quality_score trend may reflect rater drift`。

---

## 方向五：运行时遥测自检 —— 系统应该能发现自己出问题了

**优先级**: P1  
**类别**: 边界（运维 · 可靠性）  
**一句话**: ForgeOS 为其治理的每个项目产出详细的 trace/scorecard/memory，但**对自身运行状况零感知**——gate 失败率上升、trace 延迟暴涨、checkpoint 写入失败——系统无从知道「自己病了」。

### 现状

ForgeOS 拥有完备的**面向项目的遥测**：

- `trace.jsonl` → 每 phase 的延迟、成本、gate 结果
- `scorecards.json` → 模型质量历史
- `checkpoint.json` → 迭代进度
- `memory.jsonl` → 跨 session 知识

但**这些数据只有项目级消费路径（路由择优、收敛判定）**，**没有系统级消费路径（自身健康）**。

具体未检测的模式：

**模式 1：Gate 失败率漂移**

```
周 1-3: test gate 通过率 = 98%（211 测试中偶尔 1-2 个时序敏感的失败）
周 4:   test gate 通过率 = 72%（155/211，降了 26 个百分点）
```

可能的原因：依赖更新导致 breakage、硬件变更、并行 run 冲突（方向一）、或测试本身退化。但当前没有指标追踪 gate 通过率趋势——「forge accept 一直绿的」就没人注意到通过率从 98% 降到了 72%。

**模式 2：Trace 延迟异常**

```
正常:   phase 平均墙钟 45s（claude -p print 模式）
第 7 次 run:  phase 平均墙钟 320s（7× 正常）
```

可能的原因：API 降速、网络问题、系统负载高。`scorecard_wind.go` 虽记录了 `p95_latency_ms`，但不做跨 run 的异常检测——没有「延迟 > 3σ 移动平均」的告警。

**模式 3：Checkpoint 写入失败静默**

```go
// forge-core/internal/persist/checkpoint.go
func (c Checkpoint) Save(path string) error {
    tmp, err := os.CreateTemp(dir, "checkpoint.*.tmp")
    if err != nil {
        return fmt.Errorf("checkpoint temp: %w", err)  // 错误返回给调用者
    }
    // ...
}
```

`checkpointHook` 在 `OnIteration` 中被调用：

```go
// forge-core/cmd/forge/evolve.go
l.OnIteration = func(i int, sig converge.Signals, durationMs int64) {
    cp := persist.Checkpoint{...}
    if err := cp.Save(l.checkpointPath); err != nil {
        fmt.Fprintf(os.Stderr, "checkpoint: %v (continuing)\n", err) // FAIL-LOUD-AND-CONTINUE
    }
}
```

CHECK: 我看到注释说它是一个 FLAIL-LOUD-AND-CONTINUE 模式。如果连续 10 次 checkpoint 都写入失败（例如磁盘满），`forge evolve` 继续跑——直到崩溃时发现 checkpoint 丢失，所有 progress 消失。当前没有「checkpoint 写入成功率 < 90% → 熔断」的机制。

**模式 4：Scorecard 老化**

```
quality_score 从 0.85 到 0.65 的 3 周缓慢下降
```

可能的原因是代码库增长、prompt 构造退化、或模型行为漂移（Claude 版本升级）。但系统没有「quality_score 持续 N 周下降 → 自动触发 investigate」的能力——退化被发现时往往已经持续了数周。

### 为什么需要

1. **自治系统的自治运维**: ForgeOS 的卖点是 24h 无人值守。但「无人值守」的前提是系统能自我检测异常——如果运维人员需要手动 `grep` trace.jsonl 来发现问题，那就不算无人值守。

2. **门闩效应的数字孪生**: `edgecases-and-perf.md` §3.1 讨论了收敛的 staleCount 门闩——那是 project 级别的进展卡住。方向五关注的是 **ForgeOS 自身的运行卡住**——系统在正常跑，但跑的方向不对（通过率下降/延迟飙升）。

3. **预算耗尽前的早期预警**: 当前 `runBudget` 是硬性熔断器——花超了才停止。但如果有「每日成本趋势 + 按当前速度预测剩余 budget 可用天数」的预警，可以在 budget 耗尽前调整策略（降 tier / 减 iteration）。

4. **记录但不消费的 telemetry 是浪费**: 项目已经有了 trace/scorecard/checkpoint/memory 的全量遥测——缺的是一个消费层。不做自检，这些数据等于只利用了一半。

### 代码证据

| 文件 | 行号 | 证据 |
|------|------|------|
| `forge-core/internal/trace/trace.go` | 全局 | trace 事件有完整数据，但无人消费作自检 |
| `forge-core/cmd/forge/scorecard_wind.go` | 全局 | 只做写 scorecard，不做 trend analysis |
| `forge-core/cmd/forge/evolve.go` | `checkpointHook` | checkpoint 失败只 log，不自检 |
| `forge-core/internal/persist/checkpoint.go` | `Save` | 无写入成功率统计 |
| `forge-core/cmd/forge/gates.go` | `probeStatuses` | 只做单次 gate 探测，无历史趋势 |
| `forge-core/internal/orchestrator/backoff.go` | 全局 | 退避只服务单 phase，不聚合全局重试率 |

### 建议方向

- **运行健康仪表盘**: 新增 `forge doctor --health` 子命令，聚合以下指标并输出健康等级：
  ```
  forge doctor --health
    ✅ gates:   通过率 97.3%（最近 50 次），正常范围 [90%, 100%]
    ⚠️ latency: 平均 67s（最近 10 run），正常范围 [30s, 90s]——接近上界
    ✅ checkpoint:  写入成功率 100%（最近 100 次）
    ⚠️ scorecard: quality_score 连续 2 周下降（0.82→0.74→0.71）
  ```

- **健康阈值定义**: `.agent/policies/health.yml` 定义系统自身的健康阈值（gate 通过率下限、latency 移动平均窗口、scorecard 趋势检测周期）。与 `policies.yml` 是同一模式——`gate.mjs` 管项目健康，`doctor` 管 ForgeOS 健康。

- **趋势引擎**: 轻量移动平均 + 标准差计算（纯 Go stdlib，零依赖）。消费 `trace.jsonl` 的最后 N 行，计算每个维度的均值/标准差/斜率。斜率异常（如 latency 斜率连续 3 个窗口为正）触发 WARN。

- **自检纳入闸门**: 在 `forge accept` 中增加非 load-bearing 的 `doctor` 检查（类似 N/A gate）——如果系统健康指标异常，在 acceptance 报告中输出 `[INFO] system health: 2 warnings (run forge doctor --health)`。不影响 ACCEPTED/REJECTED 裁定，但让运维人员被动注意到。

- **自动 generate run summary**: 每次 `forge run`/`forge evolve` 结束时，输出一个一行摘要：
  ```
  forge run: workflow=build mode=balanced lifecycle=mvp phases=5 gates_ok=12 gates_na=3 cost=0.92 latency_avg=52s
  ```
  这样逐步积累一个可被 grep 的历史记录。长期看可以 `forge run --json` 输出结构化结果供外部监控消费。

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 | 代码准备度 |
|------|--------|------|-----------|-----------|
| **一 · 多实例并发冲突** | **P1** | 边界/正确性 | 两把 `forge evolve` 同时跑即数据损坏——生产部署前必须解决 | 0%——需 `run_id` + 文件锁 |
| **二 · 冷启动零到一** | **P1** | 边界/用户体验 | 首次 `forge run build` 让用户看到有意义的引导而非 fail-closed abort | 50%——`Checkpoint.Load` 可改造成「首次检测」 |
| **三 · 能力感知路由** | P2 | 核心功能 | 让 scan 能搜索、planner 只需只读、implementer 才写——能力与任务匹配 | 10%——Phase 能力需求字段需新增 |
| **四 · 记分卡统计可靠性** | P2 | 正确性 | 无置信区间的择优是假择优——加标准差+最小样本量可消除噪声误导 | 30%——schema 扩展 + merge 层修改 |
| **五 · 遥测自检** | **P1** | 运维/可靠性 | 24h 无人值守的前提是系统能发现自己病了——telemetry 已全只差消费层 | 20%——`trace.jsonl` 已就绪，缺趋势引擎 |

**收敛建议**:

- **只做一件**: 方向二（冷启动）。成本最低（首次检测+引导消息，核心改动 ~50 行 Go），对用户体验的改善最直接。当前 `forge init` → `forge run build` 的体验断裂是项目最明显的 UX gap。

- **做前三件（全 P1）**: 方向一 + 方向二 + 方向五。三者构成完整的生产就绪三角：方向一保证多进程安全（生产部署的硬前提），方向二保证初体验（新用户的 first impression），方向五保证长期可运维（24h 无人值守的自我监控）。

- **方向三（能力感知路由）** 建议与跨厂商模型池（`next-horizons.md` 方向一）**一并规划**——两者都涉及 `routing.go` 的 Provider 抽象，能力元数据可以嵌入 Provider 结构体中。单独做能力感知但没有第二家厂商，能力差异只有 claude 内部的不同权限模式的差异——价值兑现有限。

- **方向四（统计可靠性）** 是数据质量的保险。在 scorecard 驱动的学习闭环经过 50+ 次运行、积累足够数据后变得关键——建议在 `sample_count` 全局超过 1000 条时主动实施。
