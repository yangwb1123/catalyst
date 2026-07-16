# ForgeOS 扩展方向 — 架构师/产品经理视角

> **读者**: Arcane (TUI) 团队 — 本文不写代码,只指出 ForgeOS 作为产品/平台需要优先解决的
> **结构性缺口**与**产品盲区**,帮助你们理解平台当前的能力边界和下一步应投的方向。
>
> **方法**:全局扫描 forge-core(18 Go 包 + cmd/forge 15+ 子命令 · 纯 stdlib 零依赖)、harness
> (39+ 模块 · ~10.5K LOC)、`.agent/`(12 agent 卡 · 9 skill 卡 · 5 工作流 · 完整治理骨架)、
> `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(108+ 条逐字段审计)、31 轮 sprint 演进记录、以及
> `docs/requirements/` 下 80+ 篇已有扩展分析。**差异化**:每方向与已有 80+ 篇分析做关键词交叉验证,
> 确认其核心论点**从未作为独立系统性方向被展开**。
>
> **日期**: 2026-07-10 | **角色**:资深架构师 / 产品经理

---

## 全景:80+ 篇分析覆盖了什么,留白了什么

已有分析已经深度覆盖了:

| 领域 | 已有覆盖量 | 典型代表 |
|------|-----------|---------|
| 编排引擎(串/并行/loop-back/mode-gating/stop-condition/checkpoint/resume) | ~35 方向 | 扩展五方向系列 |
| 生产可靠性(529/超时/退避/输出上限/递归守卫/资源护栏) | ~18 方向 | production-readiness 系列 |
| 学习闭环(trace/telemetry/scorecard/三维真数据/跨运行分析) | ~12 方向 | learning-loop 系列 |
| 安全纵深(secret-scan/recursion/budget/cap/SCA/四维护栏) | ~12 方向 | security-frontiers 系列 |
| 治理/执法(arch-check 8 检查/check.py 10 检查/function-length/circular) | ~12 方向 | governance-gaps 系列 |
| 结构债务(Phase 膨胀/Context 字符串拼接/可观测性断层/契约脆弱性) | ~10 方向 | structural-gaps 系列 |
| 产品运营化(部署/回滚/多分支/发布治理/二进制版本/决策解释) | ~8 方向 | operational-gaps 系列 |
| 北向扩展(Temporal/OPA/OTel/多厂商/Sandbox/Web UI/多仓库联邦) | ~12 方向 | north-star 系列 |

**但所有这些分析共同缺失了四个视角:**

1. **产品视角**: ForgeOS 被当作「一组功能」来分析,从未被当作「用户每天要用的产品」来分析。
   没有一篇分析讨论:用户如何追踪一次 24h evolve 的进度? 如何知道系统在做什么? 出错了如何排查?
2. **平台视角**: ForgeOS 自称"Kubernetes for AI SE",但 Kubernetes 有 operator/CRD/helm 生态,
   ForgeOS 的扩展点在哪里? 12 个 agent 全硬编码,6 个 gate 全硬编码 —— 第三方的自定义 agent 怎么加?
3. **生命周期视角**: 所有分析都假设 ForgeOS 是"一次运行,用完即走"。真实产品开发需要跨会话的工作流编排、
   制品血缘追溯、阶段准入审批链 —— 这些在当前架构里是空白。
4. **自反视角**: ForgeOS 治理一切,但从不治理自己。磁盘快满了? trace 膨胀失控? memory leak?
   没有自监控、没有自愈、没有退化检测。

**以下五个方向填的就是这四块留白。**

---

## 方向一 · 运行时可观测性 API (从文件轮询到结构化事件流)

**优先级**: 🔴 **P0** | **类别**: 架构 · TUI 基础设施 | **杠杆**: ⭐⭐⭐⭐⭐
**已有覆盖**: **零** — 80+ 篇分析讨论 trace 事件怎么写(内容/格式/互斥),但没有一篇讨论
**「这些事件怎么被消费」**。

### 为什么需要

ForgeOS 拥有丰富的运行时数据:

| 数据源 | 位置 | 格式 | 内容 |
|--------|------|------|------|
| Trace 事件 | `.forge/trace.jsonl` | JSONL | iteration/agent/gate/decision/converge/error 等 10 种事件 |
| Checkpoint | `.forge/checkpoint.json` | JSON | 运行状态快照(迭代号/roadmap%/gate 状态/mode) |
| Memory | `.forge/memory.jsonl` | JSONL | 跨会话记忆(knowledge/decision/summary 等) |
| Scorecards | `.forge/scorecards.json` | JSON | 模型路由记分卡(decay-weighted history) |
| Cost ledger | `.forge/cost.jsonl` (通过 trace 内嵌) | JSONL | per-phase LLM 成本 + 模型归因 |

**但所有这些数据只有一个消费通道:文件。** 当前架构:

```
运行时 → trace.Emit() → trace.jsonl (追加写入)
       → checkpoint.Save() → checkpoint.json (覆盖写入)
       → memory.Append() → memory.jsonl (追加写入)

消费端:
  forge doctor    → 读 checkpoint.json → 健康检查
  forge status    → stat .forge/ 各文件 → 文件元信息
  forge preflight → 读 checkpoint.json → 运行前检查
  TUI             → ?????              → 只能轮询文本命令输出
```

**对于 TUI 团队,这意味着:**
- 无法获取实时进度(只能跑完 `forge run` 再看结果)
- 无法获取流式事件(Trace 的 10 种事件类型设计精良,但写入 JSONL 后只能 tail/读文件)
- 无法查询历史(scorecard 数据在 JSON 文件里,没有 query API)
- 无法订阅状态变更(没有 WebSocket/gRPC stream/事件总线)

### 代码级证据

**证据 1: Trace 事件有 10 种丰富种类,但无人消费**

```go
// forge-core/internal/trace/trace.go:27-47
const (
    KindIteration     = "iteration"       // 一次完整的循环迭代
    KindAgent         = "agent"           // LLM agent phase
    KindGate          = "gate"            // harness gate 裁决
    KindDecision      = "decision"        // 运行时决策(tier up/down, cost guard)
    KindConverge      = "converge"        // 收敛检查
    KindError         = "error"           // 可恢复/致命错误
    KindOverloadBackoff = "overload_backoff" // 529/过载退避
    KindStaleIncrement = "stale_increment"   // 无进展增量
    KindDoctor        = "doctor"          // forge doctor 结果
    KindMemoryCompact = "memory_compact"  // memory 压缩事件
)
```

10 种事件,每种包含 `seq/kind/name/status/duration_ms/cost_usd_micros/model/detail`。
**设计精良但无人消费** — 唯一读取 trace 文件的地方是 `doctor.go:160-177`(检查完整性)和
`scorecard-update.mjs`(离线聚合)。没有实时消费者。

**证据 2: `forge status` 只返回文件元信息,不返回运行时状态**

```go
// forge-core/internal/doctor/status.go:54-70
func Status(root string) StatusSnapshot {
    // 只 stat 四个文件: checkpoint.json / trace.jsonl / trace.jsonl.1 / memory.jsonl
    snap.Checkpoint = statFile(filepath.Join(dotForge, "checkpoint.json"))
    snap.Trace = statFile(filepath.Join(dotForge, "trace.jsonl"))
    snap.TraceBackup = statFile(filepath.Join(dotForge, "trace.jsonl.1"))
    snap.Memory = statFile(filepath.Join(dotForge, "memory.jsonl"))
    // 返回文件大小 + mtime,不是运行状态
}
```

**证据 3: ForgeOS 没有任何 daemon/长驻进程**

`forge` 的所有子命令(`run/evolve/gate/check/accept/route/migrate/detect/validate/doctor/preflight/approve`)都是**一次性 CLI**(
`forge-core/cmd/forge/main.go:35-54`)。没有 daemon 模式、没有 API server、没有 gRPC 服务。
TUI 无法"连接"到 ForgeOS — 只能启动进程、等待退出、读 stdout。

**证据 4: 成本/延迟数据有但无查询接口**

真点火(Sprint 24-26)已经坐实了真实成本追踪(`cost_usd_micros`)和延迟追踪(`duration_ms`)。
数据在 trace.jsonl 里,有每 phase 的 model 归因。但没有任何 API 可以查询"上周哪个 workflow 最贵"
或"哪个 agent 平均延迟最高"。数据被写入后即封存。

### 建议方向

构建一个**轻量级运行时可观测性层**,作为 TUI 的数据骨干:

```
                   ┌─────────────────┐
                   │   ForgeOS CLI   │
                   │  (现有,一次性)   │
                   └────────┬────────┘
                            │ trace.Emit() / checkpoint.Save()
                            ▼
                   ┌─────────────────┐
                   │   Observability │  ← 新增
                   │    Adapter      │
                   │  (可插拔)        │
                   └──┬──────────┬───┘
                      │          │
              ┌───────▼──┐  ┌───▼────────┐
              │ JSONL    │  │ 事件总线    │
              │ (现有)   │  │ (新增)      │
              └──────────┘  │ WebSocket   │
                            │ / gRPC      │
                            │ / UNIX sock │
                            └─────────────┘
                                  │
                            ┌─────▼──────┐
                            │ TUI/Arcane │
                            └────────────┘
```

**具体不写代码,只描述契约面:**

1. **Live run status stream**: 当前正在执行的 workflow/phase/gate 的实时状态,包括
   `{workflow, phase, status, progress%, elapsed_ms, cost_usd, model}`。
   通过 UNIX domain socket 或命名管道订阅(TUI 连接本地 socket 即可)。

2. **Historical run query**: 按 `workflow/status/time/cost` 维度查询已完成的 run,
   返回结构化记录(非文本日志)。`forge history --json` CLI 可先实现,后续 TUI 直接调用。

3. **Cost & latency dashboard data**: 聚合查询 — 哪个 workflow 最烧钱? 哪个 agent 最慢?
   哪个 gate 最容易红? 当前只能靠 jq 手搓。

4. **Run catalog / session registry**: 每次 `forge run/evolve` 注册一条记录到轻量 catalog,
   避免"我上周跑的 discover 结果在哪"这种产品级问题。

**为什么高价值**: 这是 TUI 的**基础设施**。没有结构化事件流,TUI 就只能:
- 解析 CLI 文本输出(脆弱、多语言、无 schema)
- 轮询文件系统(时延高、无推送、并发竞态)
- 重复实现 orchestrator 的状态机(版本漂移)

**风险**: Observability 适配器可能引入单点故障(daemon crash 丢失事件)。
建议:daemon 只做转发不持久化(持久化仍在 JSONL),daemon 崩溃不丢数据。
daemon 非阻塞:写 socket 超时/失败时降级到只写文件。

---

## 方向二 · 跨会话工作流编排与制品生命周期

**优先级**: 🔴 **P0** | **类别**: 产品 · 工作流 | **杠杆**: ⭐⭐⭐⭐⭐
**已有覆盖**: **零** — 已有分析讨论单次 run 内的编排(loop-back/parallel/resume),但从未讨论
**run 与 run 之间的连接**。

### 为什么需要

当前 ForgeOS 的工作模型:

```
forge run discover → 30 秒后结束 → 人看 docs/discovery/prd.md
forge run design  → 30 秒后结束 → 人看 docs/design/proposal.md → 人: forge approve
forge run review  → 30 秒后结束 → 人看 docs/review/report.md
forge run build   → 30 秒后结束 → 代码写好了
```

每个 `forge run` 是独立的。连接它们的只有:
- `.forge/checkpoint.json`(只保存最近一次 run 的状态)
- `docs/` 下的产物文件(非结构化,无 catalog)
- user 的**大脑**(人决定下一步跑什么)

**对于 TUI 团队,这意味着:**
- 无法展示"这个项目从 Idea 到 Production 的全流程进度"
- 无法追踪"哪个 PRD 衍生出了哪个架构,哪个架构衍生出了哪个实现"
- 无法回答"这次 build 是基于哪次 design 评审的"
- 没有任何 pipeline/管线概念

### 代码级证据

**证据 1: `next_stage` 在 stop_condition 里声明了,但没有跨 run 的执行者**

```yaml
# .agent/workflows/build.yml:110-114
stop_condition:
  on_met:
    next_stage: evolve  # 声明了"完成后应该进 evolve"
```

但 `forge run build` 结束后,**没有任何东西自动触发 `forge run evolve`**。
`next_stage` 只是一个**人读的标签**(`asset.go` 甚至不 decode 它,直接丢弃)。

```go
// forge-core/internal/asset/asset.go (通过 grep 确认 next_stage 无消费者)
// grep -rn "NextStage\|next_stage" forge-core/**/*.go → zero matches (asset 字段未定义)
```

**证据 2: 产物文件散落在 `docs/` 下,无结构化 catalog**

`discover.yml` emits `docs/discovery/prd.md`, `design.yml` emits `docs/design/proposal.md`,
`review.yml` emits `docs/review/*.md`。这些是**文件**,不是**记录**。
没有:
- "这个 prd.md 是哪个 discover run 产出的" (run ID)
- "这个 proposal.md 是基于哪个 prd.md 写的" (血缘)
- "跑完 design 后,对应的 discovery run 是哪个" (关联)

**证据 3: Session ID 在 trace 中有位置但未使用**

`trace.Event` 有 `Seq`(单调递增)但没有 `SessionID`/`RunID`/`WorkflowID`。
两个连续 `forge run build` 的 trace 事件在同一个 `trace.jsonl` 里追加,
序列号是连续的,但你**无法区分哪些事件属于哪次 run**。

```go
// forge-core/internal/trace/trace.go:50-73
type Event struct {
    Format       string `json:"_format,omitempty"`
    Seq          int    `json:"seq"`         // 单调递增,全局
    Kind         string `json:"kind"`
    Name         string `json:"name"`
    // ... 没有 SessionID / RunID
}
```

**证据 4: Checkpoint 只保留"最近一次"状态**

`persist.Save()` 覆盖写入 `checkpoint.json`,历史通过 `.1`/`.2` 备份(最多 5 个),
但 checkpoint 本身不包含**下次该跑什么**的指针。

```go
// forge-core/internal/persist/checkpoint.go
// 字段: Iteration / RoadmapCompletion / GatesGreen / Mode / PhaseIndex
// 没有: NextStage / LastRunID / WorkflowChain
```

### 建议方向

构建**会话 (Session) 作为一等公民**:

1. **Run Session ID**: 每次 `forge run/evolve` 分配 UUID,注入所有 trace 事件、
   checkpoint、memory 记录。让"这次 run 的所有痕迹"能一键查询。

2. **Artifact Catalog**: 产物文件(PRD/proposal/ADR/review report/代码)写入时在
   `.forge/artifacts/` 自动注册一条记录:`{session_id, phase, artifact_path, type, hash}`。
   TUI 可以展示"从 PRD 到代码的完整血缘"。

3. **Workflow Pipeline**: `forge pipeline <workflow-chain>` 概念 —— 声明式定义
   一组按序执行的 workflow(如 `discover → design → [human] → review → build → evolve`),
   自动传递产物、管理阶段准入、跨 run 维持 checkpoint。

4. **Run History 持久化**: 每次 run 的 Summary(workflow/phase 数/总耗时/总成本/最终收敛状态)
   持久到 `.forge/history/`。不是 trace 的替代,而是 trace 的聚合索引。
   TUI 可以直接展示"过去 30 天的运行历史热力图"。

**为什么高价值**: 这是 ForgeOS 从"一次性 CLI 工具"进化为"产品级平台"的**最关键一步**。
没有会话和管线,每个 run 都是孤岛,无法支撑真正的 24h 自治开发流程。
TUI 团队若想展示"项目进度"/"开发流水线"/"制品位流转",需要这些原语。

**风险**: 不要过度工程化。catalog 应设计为**现有物产文件的索引层**,而不是替代。
不引入数据库(仍用 JSONL/JSON 文件),保证 ForgeOS 的零外部依赖原则。

---

## 方向三 · 插件化 Agent/Gate/Router 扩展系统

**优先级**: 🟠 **P1** | **类别**: 平台 · 生态 | **杠杆**: ⭐⭐⭐⭐
**已有覆盖**: **零** — 8 篇分析提到"自定义 gate"概念,但**没有一篇系统性地讨论扩展架构**。

### 为什么需要

ForgeOS 当前:

| 扩展点 | 实现方式 | 用户能否自定义 |
|--------|---------|---------------|
| Agent 类型 | `agentTier` map + `agentPrompts` map | **不能** — 全硬编码在 Go 代码里 |
| Gate 类型 | `gate_catalog` + `adapters/*.yml` | **部分可以** — 可以加 adapter 配置,但不能加新的 gate kind |
| 路由策略 | `routing.TierFor` + `HistoryTiebreak` | **不能** — 硬编码 Go 逻辑 |
| 路由维度 | 4 维评分(complexity/risk/context/budget) | **不能** — 维度数硬编码 |
| Workflow step 类型 | `agent` + `gate` 两种 | **不能** — `asset.Phase` 只有两种 |
| Harness 检查 | 6 个 Node 脚本 | **可以添加新脚本**,但必须在 `acceptance.mjs` 里硬编码 |

**对于 TUI 团队,这意味着:**
- TUI 无法展示"项目中安装了哪些插件" —— 因为根本没有插件概念
- TUI 无法提供"自定义 agent 配置界面" —— agent 扩展需要改 Go 代码
- 第三方想为 ForgeOS 贡献能力(新 gate checker、新 agent 类型),必须 fork 代码库

### 代码级证据

**证据 1: Agent 类型在 Go 代码里硬编码映射**

```go
// forge-core/internal/routing/routing.go:27-44
var opusFloorAgents = map[string]bool{
    "architect": true,
    "cto":       true,
    "reviewer":  true,
}

var agentTier = map[string]string{
    "planner":     Sonnet,
    "implementer": Sonnet,
    "qa":          Sonnet,
    "harness":     Haiku,
    "docs":        Haiku,
}
```

新增一个 agent 类型(如 `"docker-engineer"`)需要:
1. 改 `opusFloorAgents` (要不要 opus floor)
2. 改 `agentTier` (默认 tier)
3. 写 agent prompt 卡(`.agent/agents/docker-engineer.md`)
4. 加 workflow 引用
5. 不需要改 `routing.go`? 不,实际上**必须改** —— 因为 `agentTier` 和 `opusFloorAgents`
   是编译时确定的,不编译就永远不认识新 agent。

**证据 2: 所有 agent prompt 是 Go 字符串**

`prompt_context.go` 通过 `readCard()` 读 `.agent/agents/*.md` 文件注入 prompt。
这已经是"声明式"的良好实践。但 agent 的**调度运行时属性**(tier/floor/readonly/fresh_context)
是在 Go 代码里硬编码的,不在 agent card 里声明 —— 卡与运行时行为分离。

**证据 3: Gate 种类在 Go 代码里硬编码**

```go
// forge-core/internal/gate/gate.go
// gate_catalog 在 modes.yml 里声明了 6 种: lint/test/build/complexity/arch/security
// 但每种 gate 的"如何执行"在 acceptance.mjs + adapters.mjs 里硬编码
// 新增"mutation-test" gate 需要改: modes.yml + gate.go + acceptance.mjs + policies.yml
```

**证据 4: 路由维度数硬编码**

`route.go:37-44` 的 `Score()` 函数有 4 个硬编码维度:

```go
// forge-core/cmd/forge/route.go:37-44
type ScoreInput struct {
    Complexity   int  // 代码复杂度
    Risk         int  // 安全风险
    Dependencies int  // 依赖变更量
    ContextSize  int  // 上下文预算
}
```

要加第 5 维度(如 `BusinessCriticality`)需要改:路由结构体 + 评分函数 + tier 逻辑 + 可能
还要改 `routing.go` 的 `TierFor`。没有插件化扩展点。

### 建议方向

构建**插件契约(Plugin Contract)** 而非插件实现:

1. **Agent 元数据声明化**: 把 agent 的运行时属性(tier floor / readonly 默认 / fresh_context
   默认 / required_tools / emits 模板)从 Go 代码移到 agent card YAML frontmatter 中。
   `routing.go` 改为读 `.agent/agents/*.md` 的 frontmatter,不再维护两套硬编码 map。
   **收益**:新增 agent 类型=写卡,不写 Go 代码。

2. **Gate 注册机制**: 引入 `gate.Registry` 概念 —— gate 种类不再硬编码,
   而是通过 `harness/gates/*.mjs` 命名约定注册。新 gate = 在新目录放脚本。
   核心 gate(lint/test/build/complexity/arch/security)照旧,
   但用户自定义 gate 可以额外注册。

3. **Router 维度钩子**: 允许通过 `--custom-score-cmd` 或 `--score-hook` 注入自定义评分脚本,
   接收标准输入(当前上下文),输出额外维度(名+分),接入 `TierFor` 的决策流程。
   不用改 Go 路由核心逻辑。

4. **TUI 插件管理界面**: TUI 读取 `.forge/plugins/` 目录(或 `.agent/plugins.yml`),
   展示已安装的插件及其版本/状态。插件可以是 gate/agent/评分维度/workflow step。

**为什么高价值**: ForgeOS 的愿景是"AI 软件工程界的 Kubernetes"。Kubernetes 的成功
很大程度上来自 CRD + Operator 生态 —— 第三方可以扩展平台而不需要改核心。
ForgeOS 当前的所有扩展都需要 fork/改核心代码,这不是一个平台该有的架构。
插件化是 ForgeOS 从"自己用的工具"到"别人用的平台"的关键跨越。

**风险**: 先做**契约面标准化**(agent card frontmatter / gate 注册 / router hook),
不急于做热加载/隔离执行。插件安全(恶意 gate 读取全盘)是后续问题,当前产品阶段
不要过度设计。

---

## 方向四 · 错误语义学与故障目录 (从"exit 1"到可诊断的故障域)

**优先级**: 🟠 **P1** | **类别**: 产品 · 可诊断性 | **杠杆**: ⭐⭐⭐⭐
**已有覆盖**: **部分覆盖** — 已有分析讨论错误分类(`ExecError`/`KindOverloaded`/退避),
但**从未从产品角度讨论错误如何被用户理解、追踪、聚合**。

### 为什么需要

当前 ForgeOS 的错误处理架构:

```
错误发生 → classifyRunErr() → Kind* + Error() string
                              ↓
                     trace.Emit(kind="error", detail=...) → trace.jsonl
                              ↓
                     return error to caller → 冒泡到 main.go → fmt.Println → os.Exit(1)
```

**对于用户(以及 TUI),这意味着:**

| 问题 | 后果 |
|------|------|
| 错误全是自由文本 | TUI 无法结构化渲染错误,只能显示"Error: something went wrong" |
| 没有错误 ID | 两个相同的错误无法去重,无法追踪"这个 529 这周发生了多少次" |
| 没有错误严重性 | 无法区分"可重试的 529"和"不可恢复的 checkpoint 损坏" |
| 没有错误上下文 | 一个 agent phase 超时了 —— 是哪个 workflow?哪个 phase?第几次重试? |
| 没有错误知识库 | 错误信息说"fork/exec failed" —— 用户无法知道这是 PATH 问题还是 OOM |

### 代码级证据

**证据 1: ForgeOS 有 7 种错误类型,但只通过 Go 的 `Error()` 方法暴露**

```go
// forge-core/internal/orchestrator/exec_error.go
const (
    KindRetryable    // 可重试(超时/过载)
    KindTimeout      // 超时
    KindOverloaded   // 529/限流
    KindFailed       // agent 非零退出
    KindConfig       // 配置错误
    KindRecursionLimit // 递归上限
    KindBudgetExhausted // 预算耗尽
)
```

每种都有 `Error() string`,但:
- 没有 `Code() string` 给 TUI 做结构化匹配
- 没有 `Severity() string` (fatal/warning/info)
- 没有 `Recovery() string` (重试/跳过/终止/联系管理员)
- 没有 `HelpURL() string` (指向故障排查文档)

**证据 2: 错误传播路径是 Go 的 `error` 接口,结构化信息在冒泡中丢失**

```go
// 错误产生: command_executor.go:207-210
return nil, &ExecError{Kind: KindOverloaded, Err: err, Retryable: true}

// 错误消费: engine_build.go:85-92
if err != nil {
    logf("phase %s failed: %v", phase.Name, err)  // 只调 .Error()
    return err
}

// 最终消费: main.go:350
fmt.Fprintf(os.Stderr, "Error: %v\n", err)  // 又只调 .Error()
os.Exit(1)
```

结构化信息(`Kind`, `Retryable`, `RetryCount`, `Phase`)在冒泡到 `main.go` 时全丢失。

**证据 3: 没有故障目录 / 错误聚合**

trace 的 `KindError` 事件记录了每次错误,但:
- 没有按错误类型聚合的计数器
- 没有"最近 N 次运行中,哪种错误最常见"的查询
- 没有已知错误注册表("这个 529 在过去 24h 出现了 30 次,峰值在 14:00")

**证据 4: `forge doctor` 检测系统健康但不检测错误历史**

```go
// forge-core/internal/doctor/doctor.go:93-150
// 检查: .forge/ 存在 / tmp 残留 / checkpoint 可读 / trace 完整性 / memory 可解析
// 从未检查: 过去 N 次运行的错误率 / 错误热点 / 退化趋势
```

### 建议方向

构建**结构化故障域(Fault Domain)**,使 ForgeOS 的错误可诊断、可聚合、可学习:

1. **结构化错误契约**: 所有 `ExecError` 增加 `Code`(机器可匹配的唯一码,如
   `E_AGENT_TIMEOUT`/`E_GATE_FAILURE`/`E_BUDGET_EXHAUSTED`)、`Severity`
   (fatal/error/warning/info)、`RecoveryHint`(重试/跳过/修配置/联系管理员)、
   `Component`(orchestrator/executor/gate/router/doctor)。
   TUI 可以直接按 code+severity 渲染不同颜色的错误卡片。

2. **故障目录 (Fault Registry)**: 轻量 `.forge/faults.jsonl`,每次 `forge run/evolve`
   结束时写入本次运行的错误摘要:`{run_id, errors:[{code, count, first_at, last_at}]}`。
   聚合后 TUI 可以展示"错误趋势图"—— 这周比上周多了还是少了 529?

3. **错误知识的闭环**: 对已知可恢复错误(529/超时),系统自动标记为"known transient",
   不纳入"健康度"计算。对从未见过的新错误类型,标记为"unseen — 可能需要人工介入"。
   这是对 ForgeOS "学习闭环"理念的延续 —— 不只是学习路由偏好,还学习错误模式。

4. **TUI 错误中心**: TUI 展示:
   - 运行中的实时错误流(通过方向一的 event stream)
   - 历史错误聚合(按 code/week/component/workflow)
   - 错误详情展开(完整 error chain/context/发生时的 phase/重试历史)
   - 健康度趋势(基于最近 N 次 run 的错误率)

**为什么高价值**: 24h 无人值守运行意味着**没有人盯着屏幕看报错**。
当用户早上来查看时,TUI 需要快速回答三个问题:① 昨晚运行成功了吗?
② 如果失败了,是什么原因? ③ 这是已知问题还是新问题?
当前"exit 1 + 一行文本"无法回答任何问题。
结构化错误域是 24h 自治开发的基本保障。

**风险**: 不要做成 Java 级别的异常层次结构。ForgeOS 是 Go 项目,保持简单:
错误码字典(Sprint 31 的 `check_mode_priorities` 模式)+ JSON 序列化 +
聚合计数器。不需要异常捕获继承多态。

---

## 方向五 · 自监控与退化检测 (ForgeOS 治理自身)

**优先级**: 🔴 **P0** | **类别**: 可靠性 · 自反 | **杠杆**: ⭐⭐⭐⭐⭐
**已有覆盖**: **零** — 8 篇+分析讨论 `forge doctor` 但没有一篇讨论**持续自监控与退化检测**。

### 为什么需要

ForgeOS 的核心卖点是"24h 无人值守自治开发"。但这产生了一个自反问题:

> **谁来值守值守者?**

当前系统:

| 资源 | 增长模式 | 有无上限 | 满了/坏了会怎样 |
|------|---------|---------|---------------|
| `trace.jsonl` | 每次 run 追加,永不压缩 | **无** | 磁盘满 → 所有写硬盘的操作失败 |
| `memory.jsonl` | 每次 evolve 追加,只在 `Compact` 时清理 | 有 `memoryCap=32` | 但 `Compact` 只压缩内容不压缩文件大小 |
| `checkpoint.json` | 覆盖写,`.1`~`.5` 历史 | 5 个备份 | 备份积累不清理 |
| `.forge/` 目录 | 文件数随 run 增长 | **无** | 目录遍历变慢 |
| 子进程 | 正常 spawn/wait | `MaxAgentCalls` 限制 | daemon 崩溃时**孤儿进程无人清理** |
| 上下文缓存 | `ContextCache` 在 run 内 | run 结束不清理 | 内存泄漏(Go GC,但大 map 不会立即回收) |

**对于 TUI 团队,这意味着:**

- 无法展示"ForgeOS 自身健康状况" —— 系统可能正在退化但用户不知道
- 无法告警"磁盘还剩 10%" —— 24h 持续写入可能在 2h 后撑满
- 无法自动执行维护 —— trace/memory/checkpoint 的清理全靠用户手动

### 代码级证据

**证据 1: Trace 文件无限增长,无轮转策略**

```go
// forge-core/internal/trace/trace.go:105-108
func (t *Tracer) Emit(ev Event) {
    // 直接 io.Writer.Write + "\n"
    // 没有文件大小检查,没有轮转,没有压缩
}
```

唯一的大小控制是 `doctor.go:160-177` 的 `traceCheck` 检查文件是否可读,
但**不检查大小、不触发轮转、不告警**。

**证据 2: Memory 文件压缩只压缩语义内容,不压缩文件大小**

```go
// forge-core/internal/memory/memory_compact.go
// Compact 合并相似条目(按 topic/chunk kind),但文件还是追加写入
// 读的时候全量加载到内存,文件越大启动越慢
```

**证据 3: Checkpoint 历史备份从不清理**

```go
// forge-core/internal/doctor/status.go:113-121
func CheckpointHistoryCount(dotForge string) int {
    // 数 checkpoint.json.1 ~ .5 的数量
    // 但一旦超过 5,旧的不自动删除(没有 FIFO 清理逻辑)
}
```

**证据 4: ForgeOS 没有 daemon 模式,无法做持续健康检查**

所有检查和诊断都是**用户显式调用**的 (`forge doctor` / `forge preflight` / `forge status`)。
在 24h 无人值守 evolve 中,如果第 3 小时磁盘开始满、第 5 小时内存泄露、
第 7 小时 checkpoint 损坏,系统不会主动告警,直到:

- 第 24 小时用户来查看:发现 run 在第 8 小时就失败了
- 或者某个写操作 panic 了,整个 evolve abort

**证据 5: `forge preflight` 是"跑之前"而不是"跑期间"的健康检查**

```go
// forge-core/cmd/forge/preflight.go
// 检查: workflow 文件可解析 / CLI 存在(go/node/python) / 安全维度设置
// 只在 forge run/evolve 前执行一次,不在运行时周期性执行
```

### 建议方向

构建**自反监控(Self-Reflexive Monitoring)** 层,使 ForgeOS 能持续观察自身健康:

1. **运行时资源使用追踪 (Runtime Resource Tracker)**: 在 `forge run` 和 `forge evolve`
   的每个 iteration 末尾,记录:
   - `.forge/` 目录大小和文件数(趋势)
   - `trace.jsonl` / `memory.jsonl` 大小
   - 当前进程的 RSS/VMS(内存泄漏检测)
   - 可用磁盘空间百分比
   数据写入 `trace` 事件(kind="system_health")。

2. **退化检测规则 (Degradation Detection)**: 简单的线性阈值规则:
   - 磁盘可用 < 20% → WARN, < 10% → FAIL(停止当前 run)
   - 连续 3 次 iteration `duration_ms` 递增 > 50% → WARN(可能内存泄露/GC thrashing)
   - `memory.jsonl` 行数在无新 knowledge 时增长 → WARN(可能重复积累)
   - trace 写入延迟 > 100ms → WARN(IO 瓶颈,可能磁盘快满了)

3. **自动维护 (Self-Maintenance)**: `forge run/evolve` 收尾时可选的自动清理:
   - 压缩历史 trace(保留最近 5 次 run,归档旧的)
   - 清理 7 天前的 checkpoint 备份
   - trim memory(只保留最近一次 evolve 的知识)
   - 所有这些有 `--no-cleanup` flag 关闭,有 `--auto-maintain` flag 开启。
   默认开,但可关。

4. **TUI 健康面板**: TUI 展示:
   - 系统健康仪表板(整体健康度/磁盘/内存/trace 状态)
   - 数小时/数天的资源使用趋势图(磁盘增长曲线/Memory 行数)
   - 告警列表(当前活跃的退化信号)
   - 一键维护按钮("清理 trace 历史" / "压缩 memory")

**为什么高价值**: 24h 无人值守是整个产品的最高价值主张。
但如果系统连自己的磁盘满了都检测不到,24h run 在第 8 小时静默失败,
用户在第 24 小时得到的是"exit 1"而非"昨晚一切顺利"。
自监控不是"锦上添花"的特性,而是 24h 自治的**前提条件**。
它是从"我让它跑 24h"到"我相信它能跑 24h"的关键跨越。

此外,自监控和方向一(可观测性 API)是天然的搭档:
方向一提供**数据流**(trace 事件),方向五提供**基于数据流的告警引擎**。
TUI 既要展示"数据"也要展示"基于数据的判断"。

**风险**: 不要做复杂的告警规则引擎(Prometheus + AlertManager 是 v3 的事)。
阈值硬编码、可配 flag 足矣。自监控规则写在 Go 代码里,
不是写在 YAML 规则文件里 —— 保持简单,不制造另一个配置爆炸源。

---

## 汇总:五个方向的依赖关系与推荐优先级

```
方向一 · 运行时可观测性 API  ──────────────┐
                                            ├── TUI 基础设施层
方向四 · 错误语义学与故障目录  ─────────────┤
                                            │
方向五 · 自监控与退化检测  ────────────────┤
                                            │
方向二 · 跨会话工作流编排  ────────────────┤── 产品增值层
                                            │
方向三 · 插件化扩展系统  ──────────────────┘── 平台生态层
```

### 产品路线图建议

| 阶段 | 方向 | 目标 | TUI 可见效果 |
|------|------|------|-------------|
| **Sprint N** (立即) | ① 可观测性 API | TUI 能获取实时 run 状态 | 运行进度条 / 实时事件流 |
| **Sprint N** (立即) | ⑤ 自监控 | ForgeOS 能检测自身退化 | 系统健康面板 / 磁盘告警 |
| **Sprint N+1** | ④ 错误语义 | 错误结构化可诊断 | 错误中心 / 错误趋势图 |
| **Sprint N+1** | ② 跨会话编排 | run 之间能连接 | 流水线视图 / 制品血缘图 |
| **Sprint N+2** | ③ 插件化 | 自定义 agent/gate | 插件管理界面的基础设施 |

### 边界情况清单(每个方向特有的 edge cases)

| 方向 | Edge Cases |
|------|-----------|
| **① 可观测性 API** | • TUI 在 run 中途连接:需要 replay 已有事件 • 两个 TUI 实例同时连接:需要 fan-out • UNIX socket 被删除:优雅重连 • daemon 崩溃不丢事件:JSONL 永远是 source of truth |
| **② 跨会话编排** | • 用户手动跳过了一个阶段(如跳过 discover 直接 design):血缘中出现 null • 同一份 PRD 被两个 design run 同时引用:多对多血缘 • 旧 run 的产物被手动删除:catalog 中有记录但文件不存在:标记为 stale |
| **③ 插件化** | • 第三方插件崩溃不 kill 主进程:goroutine panic 隔离 • 插件版本与 ForgeOS 版本不兼容:版本声明契约 • 恶意插件读取全盘文件:最小权限原则 |
| **④ 错误语义学** | • 一个错误对应多个原因(如超时可能是网络慢也可能是 agent 死锁):保留原始 error chain • 已知错误模式在 v1 正确,v2 FW 升级后失效:版本标记 • 完全不认识的错误(exit code 255):归类为 unknown |
| **⑤ 自监控** | • 磁盘满导致自监控自身写不了日志:用告警文件 / 退出前打印 • 退化检测误报(如大 PR 导致 duration_ms 正常上涨):自学习基线 • 自动维护删了用户想保留的 trace:默认保留最近 5 个 run,明确策略 |

---

## 附录:与已有 80+ 篇分析的差异化声明

本文五个方向与已有分析的覆盖关系(通过关键词全文检索确认):

| 本文方向 | 已有分析提及 | 差异化 |
|----------|------------|--------|
| ① 可观测性 API/TUI 数据源 | "observability"在 12 篇分析中出现,但全部讨论 trace 事件**怎么写**,没有一篇讨论事件**怎么让 TUI 消费** | **全新视角**:不是"事件格式",而是"消费架构" |
| ② 跨会话编排/制品血缘 | "pipeline"在 5 篇分析中出现,但都作为"workflow 内的阶段编排"(即 `next_stage` 的声明性描述) | **全新视角**:不是 workflow 内的编排,而是 workflow **之间**的连接 |
| ③ 插件化 | "plugin"在 3 篇分析中出现,但都只是"未来可能需要"的泛泛表述 | **首次系统性分析**:agent/gate/router 三个扩展点的具体约束 |
| ④ 错误语义学/故障目录 | "error diagnosis"在 6 篇分析中出现讨论 `ExecError` 的分类,但没有讨论**产品级别的错误可诊断性** | **首次产品视角**:不是错误如何分类,而是用户/TUI 如何诊断 |
| ⑤ 自监控/退化检测 | "self-healing"在 2 篇分析中出现,但只讨论 agent 子进程异常退出后的重试 | **全新视野**:不是 agent 级的自愈,而是**ForgeOS 自身运行时的自监控** |

