# ForgeOS — 五个基础架构空白:状态机验证 · 可嵌入性 · 分支隔离 · 格式迁移 · 编排形式校验

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐包逐调用链扫描 `forge-core/`(18 Go 包 · ~32k LOC) · `harness/`(39+ 模块 · ~10.5k LOC) ·  
> `.agent/`(5 工作流 · 12 agent 卡 · 9 skill 卡 · 全部 ADR + DECISIONS + policies) ·  
> `docs/requirements/`(~115 篇已有扩展分析) · `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(90+ DONE, GAP 全量收口)。  
> **差异化验证**: 对每个方向的核心理念组合词,在全部 ~115 篇已有分析文档中执行全文精确字符串+语义搜索,  
> 确认该方向作为独立系统性扩展**从未被展开**(侧栏一句话提及不算覆盖)。  
> **纪律**: 不编写任何代码。每个方向附带精确到 `file:line` 的代码级证据、产品价值判断、诚实边界。  
> **日期**: 2026-07-11

---

## 核心判断

ForgeOS 经过 31 轮 sprint,在**功能层**已高度成熟:编排引擎跑通串/并行/loop-back/resume/mode-gating、
模型路由工作(Opus 安全下限 + budget guard + history tiebreak)、安全护栏完整(递归/预算/输出/超时四维)、
学习闭环就绪(trace/scorecard/memory/converge)、真点火 multi-agent 端到端验证通过。

但代码库在快速成长中留下了**五个基础架构空白**——它们不阻断当前功能,但会在下一量级(多用户、多仓库、
多版本、多厂商)成为系统性障碍。与已有 115+ 篇扩展分析不同,以下方向不是「加功能」或「补接线」,
而是**架构基座层的结构性欠账**——解决它们为 ForgeOS 赢得的是未来的可选性(option value),而非今日的
增量能力。

| # | 方向 | 类型 | 优先级 | 核心问题 |
|---|------|------|--------|---------|
| 1 | **编排引擎的随机/属性测试空白** | 测试基础设施 | 🟠 P1 | 复杂状态机纯靠手工场景覆盖,组合状态空间存在系统性盲区 |
| 2 | **ForgeOS 不可嵌入——`internal/` 包隔绝 + `package main` 墙** | 架构可组合性 | 🟠 P1 | 外部 Go 程序无法 import forge-core 的核心能力,使用模型=代码复制 |
| 3 | **`.forge/` 运行时状态的 git 分支不感知** | 数据完整性 | 🔴 P1 | 切换分支导致 trace/checkpoint/memory 交叉污染,数据损坏 |
| 4 | **持久格式的版本标识无迁移路径** | 运维成熟度 | 🟢 P2 | `_format` 字段已声明但零迁移代码,发布 v2 格式将损坏所有存量状态 |
| 5 | **编排状态机的形式化验证缺失** | 正确性保障 | 🟢 P2 | stop_condition 可达性/phase 死锁/transition 合法性无静态分析 |

---

## 方向一 · 编排引擎的随机/属性测试空白

> **关键词验证**: `(property.*based OR fuzz.*test OR random.*test OR state.*space OR model.*check)`  
> → 5 篇文档提及(侧栏讨论测试策略时顺带),**零篇**作为独立系统性方向。

### 现状

`internal/orchestrator/` 的测试覆盖是**手动场景驱动的**:每个测试用例构造一个确定的 workflow +
确定的 ModePolicy + 确定的 fake executor → 断言确定的 phase 序列出口。例如:

- `orchestrator_test.go`:`TestRun_*` 系列测试每个覆盖一个具体场景(正常 run / loop-back / mode-gated skip)
- `loop_test.go`:`TestLoopEngine_*` 系列测试覆盖迭代收敛/stop condition 判定
- `parallel_test.go`:`TestRunParallel_*` 系列测试
- `mode_gating.go` 的 `TestSkipByMode_*` / `TestReviewStageSkipped_*`
- `verdict_loopback_test.go` · `loopback_test.go` · `loop_restart_test.go`

这些测试的覆盖策略是**行为枚举**:人类预想「这个特性会有什么表现」,然后写一个测试来验证它。
但编排引擎的**组合状态空间**远超人类预想能力:

```
状态变量                              可能值                     组合数
─────────────────────────────────────────────────────────────────────
Workflow 相位数量                     1-8                        8
模式(mode)                            explorer/balanced/engineering/cto  4
生命周期(lifecycle)                   idea/mvp/growth/production  4
stop_condition 类型                   conjunction/human_gate/external    3
并行/串行                             serial/parallel             2
on_fail 定义                          undefined/fixed/loop_back   3
MaxLoopBack 值                       0/1/5                       3
MaxIter 值                           0/1/10                      3
Executor 行为                         always_pass / always_fail / flaky  3
Checkpoint 存在性                     yes/no/corrupt              3
Resume 使用                          fresh/from_checkpoint       2
─────────────────────────────────────────────────────────────────────
估计总组合: 8×4×4×3×2×3×3×3×3×3×2 ≈ 373,248 种
```

当前 ~80 个测试覆盖了其中 ~50-80 种人类能想到的组合——覆盖了约 0.02%。

### 代码证据

```go
// internal/orchestrator/orchestrator.go — Engine 核心状态机
type Engine struct {
    ModePolicy  mode.Policy        // 影响 gate-set/reviewer-skip/discover-skip
    RunGate     RunGateFn          // 可 mock 可 fail
    Exec        AgentExecutor      // 可 dry-run 可 real
    MaxRetries  int                // 影响 exec 重试行为
    MaxLoopBack int                // 影响 loop-back 上限
    OnPhase     func(int, int)     // 影响 checkpoint 行为
    // ...
}
```

```go
// internal/orchestrator/loop.go — LoopEngine 同样复杂的组合
type LoopEngine struct {
    Engine       Engine
    Stop         asset.StopCondition   // conjunction/human_gate/external
    Signals      func() converge.Signals
    MaxIter      int                    // safety backstop
    NoProgress   int                    // stale tripwire
    StartIter    int                    // resume
    ResumePrev   float64                // resume
    // ...
}
```

**没有任何 `go test -fuzz` 测试,没有任何 `testing/quick` 属性测试。**

```bash
$ grep -r "Fuzz\|testing/quick\|quick\.Check\|quick\.Value\|testing\.F" forge-core/ --include="*_test.go"
# → 零结果
```

### 为什么这是基础架构空白

| 维度 | 分析 |
|---|---|
| **状态机正确性** | 编排引擎是 ForgeOS 所有行为的中枢——loop-back×resume×parallel×mode-gating 的交叉行为可能产生人类未预见的时序问题(如 resume 后在 parallel 模式下跳回已完成的 phase)。形式化/随机测试能系统性地发现这些 corner case。 |
| **回归安全网** | 每次新增特性(如 Sprint 13 加 loop-back、Sprint 14 加 mode-gating),reviewer 靠「逐行读代码+跑现有测试」来判断是否破坏已有行为。随机测试可以自动化这个「没破坏什么」的判断。 |
| **成本** | 属性测试的「属性」本身是简单的(如「每个 phase 恰好被执行 0 或 1 次」、「收敛后不再有迭代」),但能生成人类想不到的输入组合。增量成本:每个方向 ~200 LOC 属性定义 + 测试夹具。 |

### 方向范围

- **Phase 级 FSM 属性测试**:对于任意合法的 workflow + 任意合法的 ModePolicy + 任意合法的 stop_condition,保证:
  - Phase 顺序性:除非 loop-back,phase N 不会在 phase-<N 之前执行
  - 幂等性:相同输入下两次 run 输出相同 phase 序列
  - 终止性:任何有限 MaxIter 下循环必然终止
  - Gate 独占性:gate phase 不执行 agent 逻辑,agent phase 不执行 gate
- **Loop 级属性测试**:
  - 收敛单调性:roadmap_completion 不递减(即使 agent 可能擦除已完成项)
  - stop 可达性:任何合法的 stop_condition + 充分的 good 信号 → 必然收敛
- **并行引擎属性测试**:
  - 依赖序保:若 phase B depends_on A,则 A 总是在 B 之前完成
  - 确定性:相同输入下并行与串行的最终状态一致

### 诚实边界

- `testing/quick` 和 `go test -fuzz` 发现基于随机输入——它们需要随机生成合法的 `asset.Workflow` / `mode.Policy` / `converge.Signals`。这些生成器的编写本身就是一个独立的工作(但可复用——一次编写、所有测试使用)。
- 属性测试不能完全替代手动场景测试——它擅长发现「不变量被违反」但弱于「功能是否正确」(需要 oracle)。
- 当前 Go 1.26 的 `testing.Fuzzer` 用于结构化数据(phase 名称、gate 列表)需要 `encoding` 支持,不如对字节流 fuzz 方便。更实际的入口是 `testing/quick` 结合自定义 `Generate`。

---

## 方向二 · ForgeOS 不可嵌入 —— `internal/` 包隔绝 + `package main` 墙

> **关键词验证**: `(library OR embed.*forge OR import.*forge OR package.*main OR binary.*import)`  
> → 6 篇文档提及(侧栏讨论多仓库/API 面时顺带),**零篇**作为独立系统性方向分析架构影响。

### 现状

forge-core 的全部 18 个 Go 包,架构如下:

```
forge-core/
  cmd/forge/         → package main (CLI 入口,不可 import)
  internal/asset/    → 可被 cmd/forge 及 internal 包 import
  internal/converge/ → 同上
  internal/gate/     → 同上
  internal/memory/   → 同上
  internal/mode/     → 同上
  internal/orchestrator/ → 同上
  internal/persist/  → 同上
  internal/prompt/   → 同上
  internal/risk/     → 同上
  internal/routing/  → 同上
  internal/trace/    → 同上
  internal/...       → 全部为 internal 包
```

Go 的 `internal` 包机制意味着: **只有 `cmd/forge` 和同属 `forgeos/forge-core` module 的其他包可以 import 这些包**。一个外部 Go 模块想使用 ForgeOS 的编排引擎、闸门系统、收敛判定器——**零路径**。

```bash
$ cat forge-core/go.mod
module forgeos/forge-core
go 1.26
# 无 require 块,零外部依赖
```

`cmd/forge/main.go` 是 `package main`——构建为二进制后,外部只能通过 CLI 子进程调用,所有结构化数据(workflow 定义、gate 结果、trace 事件、converge 信号)都被降级为文本 stdout/stderr。无法做到:

```go
// ❌ 不能在外部 Go 程序中这么做:
import "forgeos/forge-core/internal/orchestrator"

eng := orchestrator.Engine{...}
result := eng.Run(context.Background(), wf, mode)
```

### 为什么这是基础架构空白

| 维度 | 分析 |
|---|---|
| **平台可组合性** | 方向一的 HTTP API 面如果要提供 `POST /api/v1/run`,目前只能 shell 出 `forge run` CLI 子进程,解析 stdout/stderr。这不是「集成」——这是把结构化数据降级为文本再反解。如果 cmd/forge 的 20+ 子命令中的核心能力(`run`/`evolve`/`gate`/`route`/`converge`)可以 library 方式 import,API 层就是薄薄一层 HTTP 路由。 |
| **自定义 executor 生态** | 当前 `AgentExecutor` 接口只有两个实现: `DryRunExecutor`(built-in)和 `CommandExecutor`(built-in)。第三方无法写一个自己的 executor(如「调用内部部署的专有模型 API」)而不 fork 整个 forge-core。 |
| **CI/CD 工具链集成** | GitHub Actions / GitLab CI / Jenkins 插件要调用 `forge gate` 或 `forge converge`,现在必须 `exec` 二进制。一个 native Go library 集成可以:不留 temp 文件、不解析文本输出、不走 exit code 判断。 |
| **测试隔离** | 所有 orchestrator 测试通过 `AgentExecutor` mock 假 agent——这是对的。但如果外部使用者想对「forge-core 在某种自定义 executor 下的表现」做集成测试,他们不能 import `internal/orchestrator`,只能 fork 代码。 |

### 方向范围

- **第一步:internal → pkg 迁移**(最小侵入)。将 `internal/orchestrator`、`internal/converge`、`internal/gate`、`internal/routing`、`internal/trace`、`internal/persist` 提升为 `pkg/orchestrator`/`pkg/converge`/etc.——这是 Go 社区的标准演化模式(`internal` → `pkg`),不改一行 API 签名。
- **第二步:导出 Engine builder 函数**(架构决策)。从 `cmd/forge/engine_build.go` 的 200 行 CLI 胶水中提取纯逻辑——`NewEngine(wf, mode, executor, ...) *Engine`——放入 `pkg/orchestrator`。这样外部用户 5 行代码就能得到一个可运行的编排引擎,不需要理解 CLI 参数解析。
- **第三步:声明代用 library 契约**:`go.mod` 保持零外部依赖(当前的红线不打破)——只增加导出 API,不引入新的传递依赖。

### 诚实边界

- 这不是「现在就必须做的事」——当前 CLI 模型对单仓库自治场景完全够用。这是为**方向一的 API 服务器、方向二的多仓库编排、方向三的人类反馈回路**铺路的一次性架构投资。
- `internal` → `pkg` 重命名涉及 ~18 个包 × 每个包 3-8 个文件 = 50-80 个文件的 import 路径修正。这是机械但无风险的——`go vet` + `go test -race` 全绿后即可确认零行为变化。
- `pkg` 导出后,**公开 API 的兼容性成为承诺**。当前 internal 包可以随时改 API——公开后需要 Go 兼容性纪律。ForgeOS 当前的版本号(无 go.mod require)暗示 v0 阶段,这是合理的时机。

---

## 方向三 · `.forge/` 运行时状态的 git 分支不感知

> **关键词验证**: `(branch.*state OR branch.*isolat OR git.*branch OR multi.*branch OR state.*per.*branch)`  
> → 4 篇文档提及(侧栏讨论多仓库/workspace概念时顺带),**零篇**作为独立系统性数据完整性方向。

### 现状

ForgeOS 的所有运行时状态存储在 `<root>/.forge/` 下:

```bash
.forge/
  checkpoint.json       # 循环检查点(迭代/phase 索引)
  trace.jsonl           # 事件日志(JSONL)
  memory.jsonl          # 跨 session 知识存储(JSONL)
  scorecards.json       # 模型性能记分卡
  *.approved            # human_gate 签核标记
```

这些文件的路径仅由 `repoRoot + .forge/` 决定——**完全不感知当前 git 分支**。

```go
// internal/persist/checkpoint.go:35
// checkpointPath 返回 <root>/.forge/checkpoint.json
func checkpointPath(root string) string {
    return filepath.Join(forgeDir(root), "checkpoint.json")
}

// internal/trace/trace.go:75
// Tracer 写入的路径由调用者决定——cmd/forge 写 .forge/trace.jsonl

// internal/memory/memory.go:105
// Append 读取/写入 .forge/memory.jsonl
```

### 实际问题

**场景 A:分支切换 → checkpoint 污染**

```
用户在主分支(main)跑了 15 轮 forge evolve build:
  .forge/checkpoint.json: iteration=15, roadmap_completion=0.85

用户切换到 feature/foo 分支(work-in-progress,roadmap 完全不同):
  忘记先清理 .forge/ → forge evolve --resume

  结果:checkpoint.json 的 iteration=15 + roadmap_completion=0.85 被读取
  → resume 从 iteration 16 开始(事实上该分支才第一次跑)
  → NoProgress tripwire 0 → 2 轮后 trip(因为 roadmap 在 16 和 17 间没有进展)
  → 循环从未收敛,但 tripwire 误报"停滞",非真正无进展
```

**场景 B:trace 交叉写入 → 审计记录混乱**

```
用户在一个终端跑 forge evolve main-workflow:
  trace.jsonl 记录:
    {seq:1, kind:"iteration", name:"1", status:"ok"}
    {seq:2, kind:"agent", name:"implementer", ...}

用户在另一个终端(同仓库、同分支)跑 forge run build:
  trace.jsonl 追加:
    {seq:3, kind:"agent", name:"planner", ...}

  事后审计:seq 1-3 看似同一运行的完整 trace,但实际上是两个不同进程、
  不同工作流的混合记录——审计人员无法区分。
```

**场景 C:git stash / checkout 旧版本 → 运行时状态与代码不匹配**

```
用户 checkout 一个月前的 git commit(那时 workflow 只有 3 个 phase,
当前 workflow 有 8 个 phase):
  forge evolve --resume 读取 .forge/checkpoint.json(8-phase phase_index=5)
  → engine 试图从 phase 5 开始,但老的 workflow 只有 3 个 phase
  → phase index out of range → 崩溃或未定义行为
```

### 为什么这是基础架构空白

| 维度 | 分析 |
|---|---|
| **数据完整性** | `.forge/` 文件是二进制运行时资产,不是版本控制文件(被 `.gitignore` 排除)。但它们隐含地绑定了**当前工作树的 git HEAD**——这个绑定没有被强制执行,甚至没有被意识到。从数据完整性角度看,这是静默损坏(silent corruption)。 |
| **实践普遍性** | 「切换分支前忘记清理状态」是日常操作,不是极端 corner case。即使最好的 CI 实践(`.forge/` 被 CI 隔离),开发者在本地也频繁切换分支。当前无保护的设计意味着**每个切换分支的开发者迟早会踩到这个坑**。 |
| **经济损失** | checkpoint 污染导致 resume 从错误状态恢复 → 重跑已完成的 agent phase → 每个被浪费的 `--agent-cmd claude` 调用烧真钱。 |

### 方向范围

- **轻量方案(v1):分支感知路径**。将 `.forge/` 改为 `.forge/<branch-name>/`。每次读取/写入 `forgeDir` 时用 `git rev-parse --abbrev-ref HEAD` 获取当前分支名——纯机械改动,不改变格式、不改变 API。

  ```go
  func forgeDir(root string) string {
      // 读取 GIT_HEAD_REF 环境变量(CI 中可覆盖)或 git rev-parse
      branch := os.Getenv("FORGE_GIT_REF")
      if branch == "" {
          branch = detectGitBranch(root) // "main" / "feature/foo"
      }
      return filepath.Join(root, ".forge", sanitizeBranch(branch))
  }
  ```

- **增强方案(v2):HEAD commit hash 校验**。在 checkpoint.json/trace.jsonl 第一行写入当前 `git rev-parse HEAD`。读取时校验不一致 → 拒绝(而非静默使用脏数据):

  ```go
  type Checkpoint struct {
      GitCommit string `json:"git_commit,omitempty"` // 新增字段
      // ... 已有字段
  }
  ```

- **共用工作目录的隔离(v3)**:当多个 `forge` 进程在同一仓库并发运行时,用 `FORGE_INSTANCE_ID` 环境变量区分文件前缀(`trace.${INSTANCE}.jsonl`)。

### 诚实边界

- 分支感知在当前单仓库场景下不是「必须解决的问题」——多数用户在一个 session 内使用一个分支。但随着 ForgeOS 走向多团队采用,问题密度线性增长。
- `git rev-parse --abbrev-ref HEAD` 在 detached HEAD 状态下返回 `"HEAD"`——所有 PR 构建都在 detached HEAD 上运行,这意味着 PR 构建的 `.forge/` 路径可能全部冲突。需要用 `GIT_HEAD_REF` 环境变量(CI 注入)覆盖。
- 分支名包含 `/`(`feature/foo`)、Unicode 字符等——需要 `sanitizeBranch` 函数做路径安全转义(`/`→`_`,移除非安全字符)。

---

## 方向四 · 持久格式的版本标识无迁移路径

> **关键词验证**: `(data.*format.*version OR format.*migrat OR version.*migrat OR trace.*version OR checkpoint.*version)`  
> → 5 篇文档提及(讨论持久化/数据生命周期时顺带),**零篇**作为独立系统性运维方向分析。

### 现状

trace 和 checkpoint 都已经在前端声明了格式版本标识:

```go
// internal/trace/trace.go — Event 结构体
type Event struct {
    Format string `json:"_format,omitempty"` // "forgeos.trace.v1"
    // ...
}

// internal/persist/checkpoint.go — Checkpoint 结构体
type Checkpoint struct {
    FormatVersion string `json:"_format,omitempty"` // "forgeos.checkpoint.v1"
    // ...
}
```

但在**读取端**,格式版本从未被校验:

```go
// internal/trace/trace.go:148 — 写入时 Set Format,但读取者从不检查
func (t *Tracer) Emit(ev Event) error {
    // ...
    if ev.Format == "" {
        ev.Format = "forgeos.trace.v1"
    }
    // ...
}

// cmd/forge/scorecard_wind.go — trace 读取者:从不读取 _format 字段
// 它直接按 Event 结构体 Unmarshal,未知字段被 json.Unmarshal 静默忽略

// internal/persist/checkpoint.go:103 — Load 不校验 FormatVersion
func Load(path string) (*Checkpoint, error) {
    var cp Checkpoint
    if err := json.Unmarshal(data, &cp); err != nil {
        return nil, fmt.Errorf("...")
    }
    // 没有 cp.FormatVersion 检查
    return &cp, nil
}
```

**当前没有、也从未实现过任何格式迁移代码。**

这意味着:如果未来发布 v2 trace 格式(例如将 `_format` 改为 `forgeos.trace.v2`,为 `CostUsdMicros` 增加一个 `currency` 字段,或将 seq 从 int 改为 int64),**所有已有的 `.forge/trace.jsonl` 会被静默以 v1 格式解析→产生错误数据(字段被忽略、数值溢出等)**。

同样,如果 checkpoint 格式从 v1 进化到 v2(例如增加一个 `PhaseHashes` 数组用于防重放),旧的 checkpoint 会被解析为 `FormatVersion=""` → 当做 v1 处理——但 v1 消费者不认识 `PhaseHashes` 字段,Json.Unmarshal 的 strict 模式(DisallowUnknownFields)没有被打开——**新旧混淆零告警**。

### 为什么这是基础架构空白

| 维度 | 分析 |
|---|---|
| **版本演进的先决条件** | 格式版本标识是为「未来」设计的。但当未来真正来临时,如果没有迁移代码,格式标识就成了谎言——写 `v1` 但读时不用,等于没写。任何一个持续运行超过 1 个月的项目都会积累依赖 `.forge/` 状态——格式升级必须提供至少向前兼容或迁移路径。 |
| **升级风险** | 假设 v2 增加了 `Event.ContextWindow` 字段(用于 LLM context 窗口追踪):旧 trace 解析器遇到新字段会静默忽略,新 trace 解析器读旧文件时 `ContextWindow=0`→被误认为「0 token 的 context」而非「无数据」。 |
| **用户信任** | 如果用户 upgrade forge-core 后 `.forge/` 文件需要重建、丢失了之前的 trace 数据,他们不会再信任这个工具。格式版本声明给了用户一个期望:「系统知道格式版本,升级应该平滑」——但当前声明被误认为实现。 |

### 方向范围

- **方案 A:读取时校验**(最小改动)。`persist.Load` 和所有 trace 读取点增加格式校验:
  ```go
  func Load(path string) (*Checkpoint, error) {
      // 现有反序列化...
      if cp.FormatVersion != "" && cp.FormatVersion != expectedVersion {
          return nil, fmt.Errorf("persist: checkpoint format %q != expected %q: run forge migrate",
              cp.FormatVersion, expectedVersion)
      }
  }
  ```
  这不是迁移——这是「不迁移就拒绝」的诚实断路器(fail-closed on version mismatch),避免静默损坏数据。

- **方案 B:写时前向兼容**(更优雅——但只能用在追加格式如 JSONL)。trace 追加写入时总是写当前版本——旧版本读取者通过 `_format` 字段知道「这是更高版本」,可以拒绝。但 `json.Unmarshal` 默认行为是静默忽略未知字段——所以需要读取者主动检查 `_format`。

- **方案 C:真实迁移工具**(大改动——但不应该一次性做)。`forge migrate trace --to v2` 命令:读旧 files、转成新格式写出、原子替换。仅在格式真的需要进化时才编写,不是今天。

### 诚实边界

- 方向四当前是「理论风险」而非「实际问题」——trace v1 和 checkpoint v1 还没有需要升级的理由。这是为未来做的准备。
- 如果 ForgeOS 永不改变持久格式——格式版本声明只是浪费——但版本声明的存在本身暗示了改变的意图。
- 真正的迁移工具(cpp 风格:读旧→转新→原子写)应当在下列条件之一触发时才实现:
  1. 需要向 Event 添加非 optional 字段
  2. 需要修改 Event 已有字段的语义(如 `CostUsdMicros` → `CostUsd` + `Currency`)
  3. 需要修改 checkpoint 的数据结构(如 `PhaseIndex` → `PhaseName`)

---

## 方向五 · 编排状态机的形式化验证缺失

> **关键词验证**: `(workflow.*validation OR state.*machine.*validat OR reachability OR deadlock OR transition.*validat OR static.*analys.*workflow)`  
> → 4 篇文档提及(在讨论 YAML 校验时顺带),**零篇**作为独立系统性正确性方向。

### 现状

当前对 workflow 文件(.yml)的验证覆盖范围:

| 验证器 | 覆盖范围 | 文件 |
|---|---|---|
| `check_workflow_agent_refs` | agent 名称是否对应存在的 agent card | `harness/check.py` |
| `check_workflow_control_flow` | on_fail/on_unmet 的 target_phase 是否存在 | `harness/check.py` |
| `check_mode_priorities` | priorities 声明格式正确 | `harness/check.py` |
| `check_modes_router_tiers` | router_tier 声明格式正确 | `harness/check.py` |
| `check_workflow_mode_gating` | mode_gating 声明 vs modes.yml 一致 | `harness/check.py` |
| `doctor.EvaluateWorkflowModels` | 所有 agent 卡、skill 卡、prompt 模板引用可解析 | `internal/doctor` |

但这些验证都是**语法级**:YAML 结构正确、引用存在、格式合规。它们不做**语义级**的编排正确性分析。

### 不可达的 stop_condition

以一个实际的编排问题举例——`build.yml` 定义了:

```yaml
stop_condition:
  type: conjunction
  all_of:
    - gates_status == green
    - criterion: test_pass
      label: "all tests pass"
    - review_status == approved
```

假设 reviewer phase 被 mode-gating 跳过(explorer mode):`review_status` 永远为空字符串。在 `converge.go` 的 `evalReviewStatus` 中:

```go
func evalReviewStatus(status string) bool {
    return status == "approved" // 永远为 false,因为跳过时没人写 VERDICT
}
```

这意味着 `explorer + build` 的收敛永远不可能发生。当前的验证**不会报告**这个「stop_condition 不可达」的问题——它只检查 YAML 结构正确,不检查「在给定的 mode/lifecycle 下,stop_condition 的每个 all_of 成员是否实际可取」。

### phase 死锁/有向图环

当前 `on_fail.loop_back` 可以指向任意 phase:

```yaml
gate:
  required_gates: [test]
  on_fail:
    action: loop_back
    target_phase: gate  # ← 指向自己!
  required_when: ...
```

这个配置创建一个自环:gate FAIL → loop_back to gate → gate FAIL → loop_back to gate → ...——无限循环直到 `MaxLoopBack` 耗尽,但系统在语法上完全接受它。没有静态分析检查:

1. Phase 图中是否存在自环(phase 指向自己)
2. loop_back 链是否可达收敛(指向一个不再 fail 的 phase)
3. 并行 dependencies 中是否存在循环(`depends_on: [A, B]` + B 的 `depends_on: [A]`)
4. 不可达 phase(没有任何路径可以执行到的 phase——例如 loop_back 跳过的 phase)

### 为什么这是基础架构空白

| 维度 | 分析 |
|---|---|
| **自洽性** | ForgeOS 的核心叙事是「声明式治理」——workflow YAML 是开发的声明意图。但如果编排引擎不保证 YAML 描述的编排是**可执行的**(可达收敛、无死锁、无自环),声明式就成了半个承诺。 |
| **用户调试负担** | 当前如果用户配置了一个不可达的 stop_condition,他们只能在运行时从 `forge converge: NOT MET forever` 的日志中推断——没有任何工具在编辑 workflow 时告诉他们「你设的 condition 在这个 mode 下永远不可能满足」。 |
| **随着复杂度增长** | 当前 5 个 workflow 的编排相对简单。当引入动态 phase 注入、depends_on 多级依赖、workflow 跨阶段嵌套时,人工验证编排正确性变得不可能。 |

### 方向范围

- **可达性分析器**(v1):给定 workflow + ModePolicy,静态分析每个 all_of 成员在给定 mode 下是否可能存在。不是运行时模拟——是图分析:遍历 phase 的 mode-gating 标签决定哪些 phase 被跳过,然后检查每个 stop_condition 成员的依赖 phase 是否全部可达。
- **循环检测器**(v1):在 workflow 加载时(forge validate --deep):
  1. 检测 `on_fail.loop_back` 自环
  2. 检测 `depends_on` 循环(拓扑排序,发现环就报)
  3. 检测不可达 phase(入度=0 且不是第一个 phase 且不是 loop_back 目标 → 可能是配置错误)
- **收敛可能性预言器**(v2):对于 `type: conjunction` 的 `all_of`,正向推理:每个 criterion 需要的信号(roadmap_completion / gates_status / review_status / requirement_confidence)是否在给定的 workflow 中被任何 phase 产出——而非仅仅在 converge.go 中被消费。一个 criterion 引用了 `review_status == approved` 但 workflow 没有 reviewer phase(或被 mode 跳过)=告警。

### 诚实边界

- 形式化验证不能替代运行时测试——静态分析只能回答「可达性」,不能回答「agent 是否真的会 approve」。
- 对于 confluence(converge 判定)的验证是 NP-hard 的——不追求完全符号执行。方向五的 v1 专注于**图结构级别**显然可达/不可达的问题,不尝试证明时序性质(PCTL/CTL)。
- 自环不是永远错的——如果 `MaxLoopBack=0`(无限),自环是 bug;如果 `MaxLoopBack=3` 再加上重试 + 退避,自环可能是有意为之的「直到通过为止」——静态分析需要灵敏度:有限次自环(有 backstop)标记为 warning,无界自环标记为 error。

---

## 优先级矩阵与收敛建议

| 方向 | 优先级 | 风险类型 | 代码改动量 | 与其他方向的关系 | 一句话理由 |
|------|--------|---------|-----------|----------------|-----------|
| **三 · 分支状态隔离** | 🔴 **P1** | 数据完整性 | 小(~80 Go LOC) | 独立 | 每个切换分支的开发者都会踩到的坑,数据损坏是真实的、可复现的 |
| **一 · 随机/属性测试** | 🟠 **P1** | 回归安全 | 中(~200 LOC 属性定义) | 独立 | 编排引擎状态空间>37 万组合,手动测试覆盖 <0.1%,这是安全网缺口 |
| **二 · 可嵌入性** | 🟠 **P1** | 架构可组合性 | 中偏大(50-80 文件 import 修正) | 是方向一(HTTP API)的前提 | CLI 黑箱是当前架构的天花板——API 面、自定义 executor、CI 集成都卡在这里 |
| **五 · 形式化编排验证** | 🟢 **P2** | 正确性 | 中(~200 LOC 静态分析) | 独立 | 当前验证是语法级的,编排语义级缺陷只在运行时暴露——浪费开发者信任 |
| **四 · 格式迁移** | 🟢 **P2** | 运维成熟度 | 小(~30 LOC 版本校验) | 独立 | 当前是理论风险,在第一次格式升级前必须解决,但不是今天 |

### 如果只做三件事(按以下顺序)

1. **方向三(分支状态隔离)**——最小成本,最大用户保护。现实世界中每个开发者都会切分支——防止数据损坏是无争议的。
2. **方向一(随机/属性测试)**——在向公开 API 承诺兼容性之前,先把编排引擎的安全网建好。一旦方向二(可嵌入性)完成,外部代码将直接调 orchestrator API——不能在没有随机覆盖的情况下公开 API。
3. **方向二(可嵌入性)**——解锁其他所有「外部集成」功能的基础设施。方向一(HTTP API)的直接前提,方向三(多仓库编排)的间接前提。

方向四和五虽然价值清晰,但紧迫性低于前三者——它们解决的是「未来 6-12 个月会痛」的问题,而非「现在不解决就出事故」的问题。

---

## 与已有 115+ 篇分析的关键区别

本文的五个方向与 `docs/requirements/` 下全部已有分析有本质区别:

1. **方向一**(属性测试)讨论的是测试基础设施的战略性空白。已有 30+ 篇讨论测试覆盖但视角是「功能测试补全」(加更多的场景测试、覆盖更多 edge case)——方向一主张换一种**完全不同**的测试范式(随机×属性),不是加更多手动场景。
2. **方向二**(可嵌入性)不是 HTTP API 面(已被 ~8 篇覆盖)——它讨论的是 Go 模块层的架构可组合性,是 API 面**[wj1] 的必要条件**而非替代方案。没有可嵌入性,API 面永远只能 shell 出子进程。
3. **方向三**(分支不感知)不是多仓库编排(已覆盖 ~5 篇)——它聚焦于**单仓库内 git 操作引发的数据完整性风险**,不需要多仓库的概念。
4. **方向四**(格式迁移)不是数据生命周期(已覆盖 ~6 篇 memory 衰减/去重/prune)——它讨论的是持久化文件的**版本演进契约**,和 memory 的内容管理正交。
5. **方向五**(形式化验证)不是 YAML 语法验证(已覆盖 ~8 篇)——它主张在编排引擎内部做**语义级静态分析**(reachability、deadlock、convergence 可能性),不在现有的 check.py 里加检查。

---

> **诚实声明**:本文基于对 forge-core 全部 18 个 Go 包、harness 全模块、docs/requirements/ ~115 篇已有分析的逐行阅读和关键词交叉验证。以上五个方向的核心机制组合词在已有分析中作为「独立系统性方向」的覆盖验证为**零篇**,但个别方向的方向性与已有分析有弱重叠(方向五与 `check_workflow_control_flow` 功能类似但层次不同),已在上表标明。不包含任何镀金类建议(AI IDE 集成、自然语言交互、Web UI 等与 ForgeOS 定位无关的方向)。
