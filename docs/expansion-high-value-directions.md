# ForgeOS — 高价值扩展方向与架构边界分析

> **视角**: 资深架构师 / 产品经理。基于全局代码扫描 + 设计文档 + ADR + 现有分析产物，
> 提出 3-5 个「当前未实现但一旦落地即解锁全新能力层级」的扩展方向。
> 不重复 ROADMAP 已有计划(v3 跨厂商池/Sandbox/Web-UI),不写代码。
>
> **原则**: 每个方向必须有明确的「为什么现在不做会限制下一个量级」的推理。
> 边界情况(edge case)和性能点融入各方向,不单独列为"小优化"。

---

## 目录

1. [方向一:Agent 输出质量闸门(相位级后验证)](#方向一agent-输出质量闸门相位级后验证)
2. [方向二:多仓组合治理与组织级控制平面](#方向二多仓组合治理与组织级控制平面)
3. [方向三:结构化故障诊断与自修复收敛](#方向三结构化故障诊断与自修复收敛)
4. [方向四:增量收敛 / 可交付里程碑切片](#方向四增量收敛--可交付里程碑切片)
5. [方向五:运行时实时可观测性 + 异常自愈](#方向五运行时实时可观测性--异常自愈)

---

## 方向一:Agent 输出质量闸门(相位级后验证)

### 核心洞察

当前系统在 **相位层面** 对 agent 产出的验证本质上是信任模型:agent 写完代码 → 写到磁盘 →
直到**整个 workflow 末尾**的 harness gate 聚合检查(test/lint/build/complexity)才被客观验证。
中间的 plan→implement→review→qa 流程中,一个 implementer 产出的坏代码会直接:
1. 被 `phaseOutputLedger.feedForward()` 传递给下游相位(包括 reviewer)
2. 被 checkpoint 持久化 (灾难后 resume 沿用)
3. 浪费下游相位的 LLM 预算来审视/修复本可在源头快速拒绝的问题

**当前防护的盲区**:
- `buildPrompt` 注入 gate 宣告但那是**前序**闸门结果,不是当前相位产出的验证
- `sanitizeAgentOutput` 只做控制字符清洗,不做语义/结构性验证
- `parseReviewerVerdict` 只提取 VERDICT 令牌,不评估产出语法正确性
- 编译/测试 gate 在 workflow 末尾,但一个多相位 workflow 可能中间已产生了 30 分钟的坏代码

### 设计建议:PhaseGate

在每个 agent phase **完成后、fed-forward 前**插入一个可选的后验证钩子。类似于
`orchestrator.Engine` 的 `OnGateResult` 模式——一个注入的回调,在 agent 输出被接受前
做快速、确定性的检查:

```go
// PhaseGate runs AFTER an agent phase completes and BEFORE its output is fed
// forward. It is OPTIONAL (nil = no gating, legacy behavior). A PhaseGate can
// inspect the phase output (text from the agent) AND the file-system changes
// (git diff) and return a verdict: allow, reject-and-retry, or reject-and-abort.
type PhaseGate func(phase asset.Phase, output string) PhaseVerdict

type PhaseVerdict int
const (
    PhasePass PhaseVerdict = iota
    PhaseRetry             // reject output, loop-back this phase (with diagnosis)
    PhaseFail              // reject output, abort the run (fail-closed)
)
```

### 典型实现场景(非代码,仅框架设计)

| Gate 类型 | 检查内容 | 快速失败价值 |
|-----------|----------|-------------|
| **编译门** | `go build ./...` / `tsc --noEmit` | 30s 内在源头拒绝"代码无法编译",不让 reviewer 审 |
| **语法门** | AST 层面的基本正确性(无死 import、无未使用变量) | 微秒级,零成本 |
| **测试门** | 新增代码是否**有配套测试**(`CodeTestRatio` 现有但仅在 converge 报告) | 阻止"只写代码不写测试"的 agent 行为模式 |
| **模块边界门** | 验证 agent 修改的文件是否在 `ARCHITECTURE.md` 允许的层内 | 防止架构腐化在相位级别发生,而非等到 `arch-check` |
| **secret 门** | 新写入文件含不含硬编码凭据(复用 `secret-scan.mjs`) | 在合并前就阻止,而非事后扫描 |

### 边界情况

- **幂等性**:PhaseGate 可能对同一个输出跑多次(loop-back 重跑)。设计上 PhaseGate 必须是
  纯函数(读文件系统 + 输入 → 判定),不能有副作用。这使得它在 `orchestrator.RunFrom` 的
  retry/loop-back 中安全重入。
- **性能**:编译门需要子进程(类似 `CommandExecutor`)。如果波内有 N 个并发 agent 相位,
  N 个并行编译进程是合理负载,但需要一个 `ConcurrencyLimiter` (如 `weighted.Semaphore`)
  防止机器 OOM。选择:每个 PhaseGate 获得一个 `context.Context`,gate 内部监听取消信号。
- **非确定性**:同样的 agent 输出跑两次编译可能一次通过一次不通过(Flaky test)。
  PhaseGate 框架应记录每次运行的结果,连续两次 FAIL 才触发 retry/abort,一次 PASS 即通过。
- **跳过权**:某些 phase 不应被门后验证(如 `planner` 输出的是 markdown 文档,不是代码)。
  PhaseGate 应通过 asset.Phase 的 `gate_phase` 字段或 agent 名称(如 `planner`)跳过。

### 为什么不现在做(但为什么将来必须做)

当前系统能工作的原因是**所有 harness 闸门在末尾统一跑**——这对 3 相位、5-10 分钟的单次 run
来说足够。但对一个 24h 自治的 evolve 循环来说,**单一相位失败造成的浪费被放大了**:
一个 implementer 花了 5 分钟写坏代码 + $0.20 → reviewer 花 3 分钟审坏代码 + $0.30 →
qa 花 2 分钟测坏代码 + $0.15 → gate FAIL 在末尾 → loop-back → **总计 $0.65 和 10 分钟浪费**。
PhaseGate 可以在 30 秒内($0)拒绝之。

### 与现有体系的关系

- 不替代 `runGates`(workflow 末尾的聚合闸门)——它们仍是最终把关者。
- 不替代 `reviewer` 角色——reviewer 做语义性、设计性的审查,PhaseGate 做语法的、确定性的检查。
- 复用现有的 `gate.Result`/`gate.ProbeAll` 架构——PhaseGate 的编译门底层就是 `exec.Command` 调用。

---

## 方向二:多仓组合治理与组织级控制平面

### 核心洞察

ForgeOS 目前是**单仓库(Single-Repo)** 的控制平面:
- `.agent/` 目录绑定到一个 git repo
- `harness/` 工具集按 repo 部署
- `forge-init` 创建单个新项目
- `forge migrate` 迁移单个项目的治理级别

但在一个组织中,N 个微服务团队各自运行 ForgeOS,完全没有:
- **聚合视图**:CEO/CTO 无法看到所有项目的收敛进度
- **跨仓标准**:无法声明"所有 Go 仓必须通过 arch-check 的 8 项检查"
- **中心化预算**:每个项目独立 `--run-budget-usd`,没有组织级总封顶
- **跨项目学习**:项目 A 的 scorecard 不贡献给项目 B 的 `HistoryTiebreak`
- **依赖关系感知**:修了仓 A 的一个 API → 自动触发仓 B/C/D 的 evolve 验证流程

### 设计建议:forge-hub (组织级服务)

`forge-hub` 是一个轻量级(纯 Go,零外部依赖)的中央聚合器,每个仓库的 ForgeOS 实例定期向它
报告关键状态。它不控制仓库的运行时(每个仓库自治),而是提供**聚合 + 策略下发 + 跨仓编排**。

```
┌─────────────────────────────────────────────────┐
│                 forge-hub                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ │
│  │ 状态聚合  │ │ 策略下发  │ │ 跨项目调度       │ │
│  │ (收敛/门) │ │ (gates模板)│ │ (依赖性回测)     │ │
│  └──────────┘ └──────────┘ └──────────────────┘ │
│  ┌──────────────────────────────────────────────┐│
│  │ 组织级 Scorecard 合并(跨项目学习)            ││
│  └──────────────────────────────────────────────┘│
└──────────────┬──────────────────────────────────┘
               │ HTTP/SSE (轻量,非侵入)
     ┌─────────┼─────────┐
     ▼         ▼         ▼
  ┌──────┐ ┌──────┐ ┌──────┐
  │Repo A│ │Repo B│ │Repo C│
  │evolve│ │evolve│ │evolve│
  └──────┘ └──────┘ └──────┘
```

### 关键设计决策

1. **非入侵式设计**:仓库的 `forge evolve` 不依赖 hub 的存在才能运行。hub 挂了,每个仓库
   独立自治。hub 是观测+策略叠加层,不是运行时依赖。
2. **策略下发是"建议"不是"命令"**:hub 声明策略模板,仓库的 `project.yml` 可覆盖(本地自治终局)。
   一个仓库选择 "engineering" 模式时,hub 的建议是"你应该启用全部 6 个 gate",但仓库可以否决。
3. **跨项目 scorecard 合并**:当仓库 A 在 `implementation` task-type 上积累了 100 个样本、
   quality_score=0.85 时,仓库 B(刚启动)可以直接从 hub 下载这张跨项目 scorecard,避免冷启动
   的盲目模型选择。这是**学习闭环的横向扩展**——让团队 A 的经验帮助团队 B。

### 边界情况

- **隐私与安全**:不是所有组织都想共享 scorecard。hub 应支持按 namespace 隔离 scorecard、
  支持 `--no-report` 标志让敏感项目完全离网运行。hub 收到的心跳应是可选的、可开关的。
- **网络分区**:仓库在离线状态下(CI 环境无外网)必须完全功能。hub 在线时才发送心跳,
  离线时一切照常。心跳丢失不应导致仓库被"暂停"。
- **策略版本冲突**:hub 下发了 v2 策略但仓库的 `project.yml` 引用了已废弃的 gate 名称。
  应保持向后兼容:未知 gate 名被忽略(日志警告),不阻止运行。
- **大规模聚合**:100 个仓库每 5 分钟发送一次心跳,每个心跳 ~1KB。hub 每秒处理 0.33 个请求——
  负载可忽略,但**长时间序列存储**需要 SQLite 或 bolt 嵌入。不建议引入 PostgreSQL。

### 为什么不现在做(但为什么将来必须做)

ForgeOS 的 dogfood(`examples/url-shortener`) 是单仓库,所以单仓控制平面是充足的。
但产品化的下一个阶段是**一个组织中的多个产品线**——那时没有跨项目视图就是一个 blocker。
v3 ROADMAP 的「Web-UI」点到此方向但不具体。本方向把 web-ui 定位为**结果展示层**,
核心基础设施是聚合 + 策略下发 + 跨项目学习。

---

## 方向三:结构化故障诊断与自修复收敛

### 核心洞察

当前 loop-back 机制(`on_fail: {action: loop_back, target_phase: implementer}`)是盲跳:
- gate FAIL → 跳回 implementer → 打印"gate still red after N/M loop-backs" → implementer
  收到一个模糊的任务"修复代码让 gate 变绿"
- reviewer REQUEST_CHANGES → 跳回 implementer → implementer 从 findings ledger 收到一段
  自由文本"reviewer says X is wrong"但**没有结构化根因分析**
- **没有故障分类**:编译失败、测试失败、架构违规、安全漏洞——全都是笼统的"gate FAILED"
- **没有重试策略差异**:一个 `KindTimeout`(超时)和一个 `KindFailed`(逻辑错误)用同样的方式重试
- **没有收敛速度检测**:如果同一个 gate 连续 3 次 loop-back 都失败,系统不识别"当前策略可能无效"

### 设计建议:StructuredFailure + AdaptiveLoopBack

分三层:

**第一层:故障结构化**(`internal/failure` 新包)

把 `gate.Result` 的输出转换为机器可读的分类:

```go
type Failure struct {
    Gate      string            // "test", "lint", "arch", "complexity"
    Category  FailureCategory   // CompileError | TestFailure | ArchViolation | etc.
    Severity  FailureSeverity   // Warning | Error | Fatal
    Detail    string            // human-readable (from gate output)
    Location  string            // file:line where applicable
    Fingerprint string          // hash of the normalized error (for dedup)
}

type FailureCategory int
const (
    CompileError FailureCategory = iota  // code doesn't compile
    TestFailure                          // test assertion failed
    TestRegression                       // existing test broke (was green)
    ArchViolation                        // layering/package/circular
    ComplexityViolation                  // function too long, file too large
    SecurityFinding                      // secret/hardcoded credential
    UnknownFailure                       // can't classify -> overestimate severity
)
```

**第二层:故障注入到 agent prompt**

当前 `gateLedger` 只注入 verdict (`ok`/`N/A`/`FAILED`),不注入 FAIL 的结构化详情。
改造后,loop-back 时 implementer 收到的 prompt 包含:

```
## 前序闸门结果(本轮自动检测)
- test: FAILED
  → 分类: 测试失败
  → 文件: src/handler/user_test.go:42
  → 失败详情: expected status 200, got 500
  → 指纹: a3f2b9c (之前 2 次也看到同一失败)
  → 建议: 检查 src/handler/user.go 第 88 行的数据库连接路径
```

**第三层:自适应的 loop-back 策略**

当前 loop-back 策略是线性的(每次 loop-back 一个单位,max 3 次后 abort)。
自适应策略根据**故障类型和重试历史**动态调整:

| 场景 | 当前行为 | 自适应行为 |
|------|---------|-----------|
| 第一次编译失败 | 跳回 implementer,重试 | 同左 |
| 第二次同一编译失败(指纹匹配) | 再跳回 implementer | 升级 agent tier(haiku→sonnet)或切换到不同 agent |
| 第三次同一编译失败 | abort | 自动创建 human 审批票,记录"系统无法修复,需人工介入" |
| 测试回归(老测试挂了) | 跳回 implementer | 增加 `--reason="现有测试 X 回归,禁止修改 X,只允许修源代码或新增测试"` |
| 架构违规 | 跳回 implementer | 增加 `--reason="违反层规则:internal 包导入了 cmd 包" + 注入违规的具体 import 路径` |

### 边界情况

- **指纹碰撞**:不同的失败可能产生相同指纹(如两个不同的编译错误在同一个文件)。
  解决方案:Fingerprint 加入 Location 字段,`Failure{Fingerprint, Location}` 二元组去重。
- **误分类**:一个 `panic: runtime error: invalid memory address` 可能被分类为
  `UnknownFailure` 而非 `TestFailure`。设计上应该**倾向误报(FN→FP)而非漏报(FP→FN)**:
  无法分类的按最高严重度处理,不静默放行。一个 `UnknownFailure` 触发 escalation 而非 retry。
- **agent 破坏证据**:agent 写坏代码后可能尝试 `git checkout` 还原失败或修改 gate 输出。
  结构化 Failure 的数据源必须是**harness 的原始 stdout**,不可由 agent 触及。
  `gateLedger` 是只读的(在 orchestrator 内),agent 无法篡改。

### 为什么不现在做(但为什么将来必须做)

当前的盲跳 loop-back 在简单场景(一个测试挂了、一行代码错了)下有效。
但真实世界的 evolve 场景是**10+ 轮迭代、多个 gate 相继红、编译+测试+架构问题交织**:
- 一个无结构化 feedback 的 implementer 在 prompt 里收到 3 段"gate FAILED"和一段
  "reviewer REQUEST_CHANGES",没有优先级区分,可能修复了次要的门而忽略了主要的。
- 在有 5+ 个依赖波(concurrent waves)的复杂 workflow 中,同时多个 gate 红可能导致
  **交叉 loop-back**:implementer 修复 A 门 → A 绿 → B 门先变红然后 A 又变红 → 振荡。
  结构化故障能让 orchestrator 按严重度和依赖序决定"先修什么"。

---

## 方向四:增量收敛 / 可交付里程碑切片

### 核心洞察

现系统收敛条件:
```yaml
stop:
  type: conjunction
  all_of:
    - metric: roadmap_completion
      operator: ">="
      threshold: 100
    - metric: gates_status
      value: green
```

**全有或全无**:要么 ROADMAP 100% → converge,要么 <100% → 继续迭代。

这有五个实际问题:

1. **MVP 停滞**:一个 20 项 ROADMAP 到第 18 项时,团队想发版,但系统阻止——必须做完最后 2 项。
   这违背了「可工作的软件是进度的首要度量」的敏捷原则。

2. **死胡同回滚**:如果在第 18 项时发现架构决策在第 5 项有重大问题,无法在第 18 项处"分叉",
   必须 rollback 到起点。

3. **生产反馈循环延迟**:一个功能上线后才知道用户不需要它——但系统要到全部 20 项完成才上线。
   无法获取早期用户反馈。

4. **生命周期迁移阻塞**:`forge migrate --to production` 要求所有 gate 全绿、ROADMAP 全完成。
   但一个 growth 阶段的项目**本来就没打算做完所有功能**——它用 feature flags 管理不完整。

5. **`staleCount` 的误触发**:单一指标 `roadmap_completion` 可能停在 80% 不是因为无进展,
   而是因为剩下的 20% 是高风险/高成本项,团队在做跟它们相关的技术基础。`NoProgress` tripwire
   不应该把"在准备而非直接完成"当作停滞。

### 设计建议:Milestone Convergence

**第一层:ROADMAP 的结构化分层**

ROADMAP.md 格式扩展到支持里程碑标记:

```markdown
## v0 (milestone: mvp-slice)
- [x] user can register              ← 计时从这里开始
- [x] user can log in
- [x] user can view profile
  (gate: test_pass + security_findings; 可单独收敛)

## v0 (milestone: core-features)
- [x] user can create post
- [ ] user can edit post
- [ ] user can delete post
  (gate: test_pass + arch + complexity; 可单独收敛)
```

每个里程碑声明自己的**收敛条件**(可覆盖或继承全局条件)。
converge 按**当前活跃里程碑**评估,而非整个 ROADMAP。

**第二层:里程碑状态机**

```
  ┌──────────┐   里程碑 N 收敛完毕
  │ active   │ ──────────────────→ ┌──────────┐
  │ milestone│                     │ completed│
  │ N        │                     │ milestone│
  └──────────┘                     └──────────┘
       │                                 │
       │ 超时/放弃                      │ 自动推进(或人工确认)
       ▼                                 ▼
  ┌──────────┐                     ┌──────────┐
  │  parked  │ ← 放弃的里程碑不会   │ active   │
  │ milestone│   被重新拾起         │ milestone│
  │ N        │                     │ N+1      │
  └──────────┘                     └──────────┘
```

- **active**:当前正在工作的里程碑。`RoadmapCompletion` 只计算活跃里程碑中的 items。
- **completed**:里程碑收敛条件已满足。系统自动推进到下一个里程碑(或等人工确认)。
- **parked**:已放弃(scope cut)。不收敛但也不阻塞——显示的 gap 但系统不因此停止。

**第三层:里程碑聚合收敛**

`forge evolve` 的最终收敛条件是:所有非 parked 里程碑都 completed(或达到 `max-iter`)。
但每个里程碑可以独立地产生一个**可交付的增量**:

```
forge evolve build 的循环:
  iter 1-3:  milestone "mvp-slice" converged → 产出 v0.1-alpha
  iter 4-6:  milestone "core-features" converged → 产出 v0.2-alpha
  iter 7-10: milestone "polish" converged → 产出 v1.0-rc
```

每个收敛点,系统可以触发 CI/CD pipeline,部署一个真实可用的增量版本。

### 边界情况

- **里程碑间依赖**:`core-features` 可能依赖 `mvp-slice` 的用户系统。
  如果先收敛了 `core-features` 但 `mvp-slice` 还在跑,两个里程碑的代码可能冲突。
  解决方案:里程碑声明 `depends_on`(类似 `asset.Phase.DependsOn`),收敛顺序必须遵循依赖图。
- **并发里程碑**:两个独立的里程碑(如前端组件库 + 后端 API)可以并行收敛。
  `forge evolve --parallel` 的 wave 概念天然适配并行里程碑。
- **部分收敛的 checkpoint**:如果 milestone 1 已完成但 milestone 2 还在进行中,crash 后 resume
  时,系统必须**不重做已完成里程碑的 agent 阶段**。需扩展 `persist.Checkpoint` 以记录
  `completed_milestones []string`。

### 为什么不现在做(但为什么将来必须做)

当前全有或全无的设计适合**净室重写/绿地项目**(一个版本,一个 ROADMAP,一次性发布)。
但 ForgeOS 适配的生命周期模型(`idea→mvp→growth→production`)天然要求增量交付:

- **mvp 阶段**:最小可行功能 → 收敛 → 获得用户反馈
- **growth 阶段**:增量特性 → 每个版本可部署
- **production 阶段**:持续演进,每个 CI 构建都应通过 gate 且可部署

没有里程碑切片,mvp→growth 的迁移是一个全有或全无的事件——这恰恰是敏捷开发要避免的。
ROADMAP 的 `forge migrate --to engineering` 在 Sprint 8 已经实现,但那是**治理级别**迁移,
不是**发布粒度**的迁移。它们应该解耦:一个工程级别(engineering)的项目仍然可以增量发布。

---

## 方向五:运行时实时可观测性 + 异常自愈

### 核心洞察

ForgeOS 的可观测性现状是**完全事后(batch/post-hoc)**:

| 数据源 | 存储位置 | 何时可读 | 是否实时 |
|--------|---------|---------|---------|
| `trace.jsonl` | `.forge/` | run 结束后 | ❌ (写完才读) |
| `memory.jsonl` | `.forge/` | iterate 后 | ❌ (每 iterate 后写) |
| checkpoint | `.forge/` | crash 恢复时 | ❌ (只在恢复时读) |
| scorecard | `.agent/routing/` | run 结束后 | ❌ (wind-down 时写) |
| `forge approve list` | `.forge/*.approved` | 用户主动查询 | ❌ (CLI 轮询) |

对一个**24h 无人值守自治系统**,这是根本性的盲区:

- 如果 evolve 在凌晨 2 点进入无限 loop-back(由于一个 bug),直到早上 8 点才被发现?
  → **6 小时**的 LLM 费用烧穿预算 + 没产生任何价值。
- 如果 converge 停滞在第 80% 但 `NoProgress=2` 的 tripwire 需要 2 次迭代才触发,
  而每次迭代 30 分钟 → **1 小时**的浪费才被自动停止。
- 如果 budget 即将用尽但还剩一个关键修复 → 系统需要主动降级而不是默默在 `checkRunBudget`
  处硬停。

**缺失的核心能力**:

1. **活动 run 的实时状态**:哪个 workflow 在跑?当前相位?已花多少钱?预计还需多少?
2. **异常检测**:不在 `staleCount` 范围内的异常行为(如 gate 失败率突然从 10% 升到 80%)
3. **通知/告警**:run 完成(收敛/失败)时的 webhook/email/Slack
4. **主动预算管理**:预算到 60% 时通知,80% 时主动降级模型,95% 时暂停非关键相位
5. **系统健康自检**:`forge doctor` 已经实现了单次快照诊断,但没有持续的活性检测

### 设计建议:ForgeOS Runtime Agent (FRTA)

一个可选的、同进程或伴生的轻量级守护进程(Go,仍是零外部依赖),提供 Liveness API +
告警投递。

**同进程模式(最简单)**

```go
// forge run/evolve 内部启动一个 HTTP endpoint
// 不依赖外部进程,只在不阻塞主循环的前提下提供状态快照
func startHealthEndpoint(root string, eng *orchestrator.Engine) *http.Server {
    mux := http.NewServeMux()
    mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
        json.NewEncoder(w).Encode(map[string]interface{}{
            "workflow": currentWorkflow,
            "phase":    currentPhase,
            "budget_spent": budgetSpent,
            "iterations":  currentIter,
            "converged":   converged,
            "uptime":      time.Since(startTime).String(),
        })
    })
    // ... run in a goroutine
}
```

**告警投递(可插拔)**

```go
// Notifier 是一个接口,可以有 Slack/email/PagerDuty/文件日志等实现。
// 在关键的运行时事件点触发,不阻塞主流程。
type Notifier interface {
    Notify(ctx context.Context, event RuntimeEvent) error
}

type RuntimeEvent struct {
    Level   string  // "info" | "warn" | "error" | "critical"
    Title   string  // "forge evolve: budget at 80%"
    Body    string  // "Current spend: $4.20/5.00, phase: implementer"
    RunID   string
}
```

**触发点(不侵入主循环)**

- `loopProbe.refresh()` 探测结果变化时:gate FAIL 率突变 → 告警
- `budget.feed()` 消耗超过阈值时:60%/80%/95% → 不同级别的通知
- `LoopEngine.Run` 每次 iteration 结束时:报告进展,notify 当 `stale >= NoProgress-1`(即将 tripwire)
- `checkRunBudget` 拒绝 spawn 之前:发出 "critical: budget exhausted, run stopping" 告警
- `cmdEvolve` 收到 SIGINT/SIGTERM 时:发出 "run cancelled by signal" 但不等确认

### 边界情况

- **告警风暴**:在一次 systemd restart 场景下,10 个 evolve 同时恢复并同时触发 budget 告警。
  需要 dedup:每个 `RuntimeEvent` 携带 `Fingerprint`(runID + eventType),同 fingerprint
  的告警在 5 分钟内合并。
- **健康端口暴露安全**:HTTP 端口不应暴露敏感信息(如 LLM API keys)。
  FRTA 默认监听 `127.0.0.1` 而非 `0.0.0.0`,且只返回聚合指标(不返回原始 trace 数据)。
- **性能叠加**:健康检查 HTTP handler 读取共享状态时需要一个 `sync.RWMutex`——但主循环
  已经持有 `trace.Tracer.mu`/`loopProbe.mu` 等,必须防止反向锁依赖。
  **设计规则**:健康端口的读锁获取必须超时(1ms),超时则返回 "status: busy",
  绝不阻塞主循环。健康检查必须可中断。
- **进程级守护**:不能在 CLI 进程中启动 HTTP server? 可选模式:`forge daemon` 启动一个
  独立的守护进程,`forge run/evolve` 通过 unix socket 向其报告状态。
  这种模式下,run/evolve 进程可以正常退出,守护进程保持活性和日志。
  `forge daemon` 本身是零业务逻辑的——它只做事件聚合和转发。

### 为什么不现在做(但为什么将来必须做)

当前的可观测性在**开发体验**阶段(dogfood,单个开发者运行)是够用的:开发者看着终端,
看到 `forge run` 的输出,看到 trace 文件,可以 `tail -f .forge/trace.jsonl`。

但 ForgeOS 的愿景是**24h 无人值守自治软件工厂**——没有人在终端前看。
那时,系统必须:
1. 主动通知人类"有事情需要你注意"(approval gate 等待、budget 快耗尽)
2. 自动记录异常以回溯
3. 提供 CI/CD 集成的健康状态 API

这些是**生产化**的最后一步。v3 的 Sandbox(Firecracker)和跨厂商池是执行层的生产化,
而可观测性是**管理层的生产化**——两者缺一不可。

---

## 总结:优先级与依赖关系

| 方向 | 价值 | 复杂度 | 对其他方向的依赖 | 推荐时机 |
|------|------|--------|-----------------|---------|
| 方向一:相位级质量闸门 | 高:直接节省 LLM 费用和等待时间 | 中:基于现有 gate 框架 | 无(独立增强) | **立即开始** |
| 方向二:多仓组合治理 | 高:打开组织级市场 | 极高:需新服务 + 网络协议 | 方向五(需要 FRTA 上报状态) | v3/v4 |
| 方向三:结构化故障诊断 | 高:让 loop-back 真正智能 | 中:分解为 `internal/failure` 包 | 方向一(PhaseGate 可作为故障收集器) | **方向一之后** |
| 方向四:里程碑收敛 | 高:解锁敏捷交付 | 高:需改造 ROADMAP 格式 + converge | 无(与现有 converge 并行存在) | evolve 深度完善后 |
| 方向五:实时可观测性 | 中-高:生产化前提 | 中:同进程 HTTP endpoint | 无(可独立开始) | **建立 CI/CD 之前必备** |

**推荐执行顺序**:
1. 方向一(快速赢) → 方向三(结构化反馈让方向一更智能)
2. 方向五(生产化前提,可与方向一并行)
3. 方向四(evolve 深度完善后,迈入增量交付)
4. 方向二(高价值但需要基础设施积累)

---

*分析日期:2026-07-01 | 基于 forge-core 全量源码扫描 + 28+ Go 包 / 11 Node harness 工具 / docs 分析产物*
*分析范围:cmd/forge(15 子命令)· internal(13 包)· harness(6 核心工具)· .agent(9 角色卡 + 4 workflow + 策略)*
*不重复 ROADMAP v3 已有计划:跨厂商池(LiteLLM)· Sandbox(Firecracker)· Web-UI*
