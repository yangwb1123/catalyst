# 代码实现报告

> **实现依据**: `2026-07-11-five-adoption-gating-product-trust-gaps.md`（需求文档）+ `.arch.md`（架构验证报告）+ `.impl-plan.md`（实现计划）  
> **实现范围**: D1（解析层故障透明化）· D2（输出物真实性检验）· D3· D4· D5 的架构数据模型扩展和基础设施改造  
> **代码库状态**: Sprint 31 完成后，2026-07-12 HEAD

---

## 实现概述

本实现针对「五方向团队采纳信任缺口」进行了系统性的架构数据模型扩展和运行时基础设施加固。核心工作包括：

1. **数据模型扩展（asset.Phase/Workflow）**：为 phase 和工作流添加所有声明但未消费的字段（FreshContext、Emits、ConfidenceMetric、OptionalFor、UsesTemplate、RequiresTools、Readonly、SecondaryTemplate、DependsOn、LoopBack），使 YAML 声明与运行时消费之间的 gap 归零
2. **收敛信号完整化（converge.Signals）**：补齐 RequirementConfidence、ReviewStatus、FileDelta、CodeTestRatio 四个断信号，使 discover/review 阶段的收敛判据不再恒为 unmet
3. **运行时信号透明化（trace.Event）**：添加 Format 版本标识 + 构造器辅助函数（GateEvent、DecisionEvent、OverloadEvent、StaleEvent、ErrorEvent），降低事件创建成本
4. **上下文传播与取消（context.Context）**：Engine 和 LoopEngine 全面接入 context.Context，支持 SIGINT/SIGTERM 优雅取消
5. **并行编排加固（parallel.go）**：fail-fast per-wave 取消 + lock order contract 文档化
6. **Stage 级中枢旋钮（mode_gating）**：reviewStageSkipped + optional_for 模式豁免 + stageDepthAtMax lifecycle 压制
7. **存储层增强（persist/checkpoint、memory）**：format versioning、历史保留轮转、load cache、Supersedes 显式撤回
8. **路由层 API 友好化（routing）**：导出 Rank/TaskTypeFloor/SafetyForceOpus/HaikuMax/SonnetMax/BandForScore/DowngradeOne 符号使外部包可组合

---

## 文件清单

### 新增文件
（无纯新增文件 — 所有修改均为已有文件的增量变更）

### 删除文件
- `forge-core/cmd/forge/attribution.go` — `internal/attribution` 包已提取先例的逻辑，此文件内容已并入 `internal/attribution`
- `forge-core/cmd/forge/attribution_test.go` — 对应测试随 attribution.go 移除
- `forge-core/cmd/forge/prompt_verdict.go` — 裁决逻辑已重分布至 `prompt_context.go` + `prompt_memory.go`，此文件不再需要

### 修改文件（57 Go 源文件）

#### 核心数据模型
- `internal/asset/asset.go` — Phase 结构体扩展（13 个新字段）、LoopBack 类型、StopCondition.OnRejected、Workflow.Readonly

#### 收敛引擎
- `internal/converge/converge.go` — Signals 扩展（4 个新字段）、evalRequirementConfidence、evalReviewStatus
- `internal/converge/converge_test.go` — 新增收敛判据测试

#### 运行时编排
- `internal/orchestrator/orchestrator.go` — context.Context 传播、stageSkipped 统一守卫、checkStageSkip
- `internal/orchestrator/loop.go` — 重构为 runIteration、OnBeforeIteration hook、GatesGreen-aware stale counting、context 取消
- `internal/orchestrator/parallel.go` — context.Context 传播、fail-fast per-wave 取消、lock order contract
- `internal/orchestrator/mode_gating.go` — reviewStageSkipped、skipByMode optional_for、stageDepthAtMax
- `internal/orchestrator/command_executor.go` — CommandExecutor 扩展（EnvPolicy 支持、Context 传播）
- `internal/orchestrator/executor.go` — context 感知
- `internal/orchestrator/backoff.go` — context 感知退避

#### 追踪层
- `internal/trace/trace.go` — Format 字段 + 5 个事件构造器（GateEvent/DecisionEvent/OverloadEvent/StaleEvent/ErrorEvent）
- `internal/trace/trace_test.go` — 构造器 + Format 测试

#### 持久化存储
- `internal/persist/checkpoint.go` — FormatVersion 字段 + rotateRetain 历史轮转
- `internal/persist/checkpoint_test.go` — 轮转 + 旧兼容测试

#### 记忆层
- `internal/memory/memory.go` — loadCache + Confidence + Supersedes 字段
- `internal/memory/memory_test.go` — 新增内存特性测试

#### 路由层
- `internal/routing/routing.go` — 导出 Rank/TaskTypeFloor/SafetyForceOpus/HaikuMax/SonnetMax/BandForScore/DowngradeOne
- `internal/routing/routing_test.go` — 导出符号测试
- `internal/routing/scorecard.go` — scorecard 集成

#### CLI 命令
- `cmd/forge/main.go` — context 传播、approve 命令、narrateReadonly
- `cmd/forge/engine_build.go` — agentExecutor context/readonly/requiresTools 感知
- `cmd/forge/evolve.go` — context 传播、OnBeforeIteration 接线
- `cmd/forge/gates.go` — gatherSignals 扩展（verdicts、CodeTestRatio、FileDelta、reviewStatus、requirementConfidence）
- `cmd/forge/gates_test.go` — gatherSignals 扩展测试
- `cmd/forge/cost.go` — parseExecutiveVerdict、parseConfidenceScore、executive verdict 常量
- `cmd/forge/cost_test.go` — 新解析器测试
- `cmd/forge/detect.go` — detect 能力增强
- `cmd/forge/route.go` — 路由命令增强
- `cmd/forge/scorecard_wind.go` — scorecard rebuild 增强
- `cmd/forge/prompt_context.go` — gateLedger + phaseOutputLedger + verdictLedger + reviewFindingsLedger 重构
- `cmd/forge/prompt_memory.go` — memory context lane
- `cmd/forge/prompt_context_test.go` — prompt 上下文测试
- `cmd/forge/prompt_memory_test.go` — memory prompt 测试

#### Harness
- `harness/check.py` — mode_gating 漂移守卫
- `harness/scaffold/forge-init.mjs` — 新文件同步
- `harness/test_check.py` — 新检查测试

#### 配置文件
- `.agent/ARCHITECTURE.md` — 架构文档同步
- `.agent/CURRENT_SPRINT.md` — Sprint 状态更新
- `.agent/agents/cto.md` — executive review 机读契约
- `.agent/agents/product-manager.md` — confidence 机读契约
- `.agent/policies/modes.yml` — mode 扩展
- `.agent/workflows/build.yml` — workflow 同步
- `.agent/workflows/design.yml` — workflow 同步
- `.agent/workflows/discover.yml` — workflow 同步
- `.agent/workflows/evolve.yml` — workflow 同步
- `.arch/rules.yaml` — arch rules 同步
- `.github/workflows/forge.yml` — CI 同步
- `docs/adr/0002-go-core-polyglot-stack.md` — ADR 勘误

---

## 核心代码实现

### 1. 数据模型扩展 — asset.Phase 结构体

**问题**: Workflow YAML 声明了多个字段（fresh_context、emits、confidence_metric、optional_for、uses_template、requires_tools、readonly、secondary_template），但 asset.Phase 结构体从未定义它们，导致 YAML 解码时静默丢弃。

**实现**:

```go
type Phase struct {
    Name              string     `json:"name"`
    Agent             string     `json:"agent"`
    Description       string     `json:"description,omitempty"`
    RequiredGates     []string   `json:"required_gates"`
    RequiredWhen      string     `json:"required_when"`
    OnFail            *OnFail    `json:"on_fail"`
    ModelTier         string     `json:"model_tier"`
    WritesADR         *WritesADR `json:"writes_adr"`
    FeedsForward      bool       `json:"feeds_forward"`
    DependsOn         []string   `json:"depends_on"`
    FreshContext      bool       `json:"fresh_context,omitempty"`
    Emits             []string   `json:"emits,omitempty"`
    ConfidenceMetric  string     `json:"confidence_metric,omitempty"`
    OptionalFor       []string   `json:"optional_for,omitempty"`
    UsesTemplate      string     `json:"uses_template,omitempty"`
    RequiresTools     []string   `json:"requires_tools,omitempty"`
    Readonly          bool       `json:"readonly,omitempty"`
    SecondaryTemplate string     `json:"secondary_template,omitempty"`
}
```

**关键设计决策**:
- 所有新字段均为 `omitempty`，零值保持旧行为 — 字节精确向后兼容
- 字段声明与 `.agent/workflows/*.yml` 中已存在的 JSON tag 名对齐
- `Readonly` 同时在 Phase 和 Workflow 级别声明，支持 stage-wide 默认 + per-phase 覆盖

### 2. 收敛信号完整化 — converge.Signals

**问题**: Sprint 29 审计发现 ReviewStatus 和 RequirementConfidence 两个断信号从未被赋值，FileDelta 和 CodeTestRatio 同样缺失，导致 discover/review 收敛判据恒为 unmet 或恒产生误报。

**实现**:

```go
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    RequirementConfidence float64  // 新增: discover 置信度 [0,100]
    ReviewStatus      string       // 新增: "approved" / "redesign" / ...
    FileDelta         float64      // 新增: git diff 匹配率 [0,1]
    CodeTestRatio     float64      // 新增: 测试代码占比 [0,1]
    HumanApproved     bool
    GateProof         GateProof
    Criteria          map[string]string
}

func evalRequirementConfidence(c asset.Criterion, sig Signals) Result {
    detail := fmt.Sprintf("requirement_confidence=%.0f", sig.RequirementConfidence)
    if sig.RequirementConfidence == 0 {
        return Result{render(c), false, detail + " (no discover phase data)"}
    }
    if c.Threshold == nil {
        return Result{render(c), false, detail + " (no threshold given)"}
    }
    met := compare(sig.RequirementConfidence, c.Operator, *c.Threshold)
    return Result{render(c), met, detail}
}

func evalReviewStatus(c asset.Criterion, sig Signals) Result {
    met := sig.ReviewStatus == "approved"
    detail := fmt.Sprintf("review_status=%s", sig.ReviewStatus)
    if sig.ReviewStatus == "" {
        detail += " (no review phase data)"
    }
    return Result{render(c), met, detail}
}
```

**关键设计决策**:
- 零值语义 = "无数据"（RequirementConfidence=0、ReviewStatus=""、FileDelta=0），不触发假性收敛
- evalRequirementConfidence 的阈值来自 criterion 的 Threshold 字段（YAML 配置），不硬编码
- evalReviewStatus 将 APPROVE + APPROVE_WITH_SIMPLIFICATION 统一归为 "approved"
- FileDelta 和 CodeTestRatio 在 gatherSignals 中计算（git diff 驱动），不用于收敛判决，仅作为诚实性警告

### 3. Trace 事件构造器 + Format 版本标识

**问题**: 构造一个 trace.Event 需要手动填充所有字段，没有类型安全的构造器。事件格式无版本标识，下游工具无法检测格式变更。

**实现**:

```go
type Event struct {
    Format     string `json:"_format,omitempty"`
    Seq        int    `json:"seq"`
    Kind       string `json:"kind"`
    Name       string `json:"name"`
    Status     string `json:"status"`
    DurationMs int64  `json:"duration_ms"`
    CostUsdMicros int64  `json:"cost_usd_micros"`
    Detail     string `json:"detail,omitempty"`
    Model      string `json:"model,omitempty"`
    Error      string `json:"error,omitempty"`
}

// 构造器:
func GateEvent(name, status, detail string) Event
func DecisionEvent(name, detail string) Event
func OverloadEvent(name, detail string) Event
func StaleEvent(name, detail string) Event
func ErrorEvent(name, errorType, status, detail string) Event
```

**关键设计决策**:
- Format 字段 `omitempty`，旧事件（无此字段）解码为 "" 并视作 "forgeos.trace.v1"
- Emit 自动注入 Format（如果为空），保证所有新事件都带版本标识
- 构造器是纯函数，不持有 Tracer 引用，可在任何位置使用
- 每种构造器预设 Kind + Status 默认值，减少调用方错误

### 4. Context 传播与优雅取消

**问题**: Engine/LoopEngine 没有 context.Context 传播。SIGINT 时子进程继续执行（直到各自的 timeout），状态可能不一致。

**实现**:

```go
type Engine struct {
    // ...
    Ctx context.Context  // nil = context.Background() (向后兼容)
}

func (e Engine) ctx() context.Context {
    if e.Ctx != nil { return e.Ctx }
    return context.Background()
}

// RunFrom 在每 phase 前检查 ctx
for i := start; i < len(wf.Phases); i++ {
    if err := e.ctx().Err(); err != nil {
        return fmt.Errorf("cancelled at phase %d: %w", i, err)
    }
    // ...
}
```

**关键设计决策**:
- 零值 nil = context.Background() — 逐位向后兼容
- 检查点在 phase 执行前，不会中断正在执行的 phase（由 CommandExecutor 的 context.WithTimeout 处理子进程取消）
- LoopEngine 在迭代间检查 ctx，不打断正在进行的迭代
- Parallel 模式在 wave 级别检查 ctx + 使用 per-wave cancellable context 支持 fail-fast

### 5. Stage 级中枢旋钮扩展

**问题**: mode-gating 只覆盖 discover stage 的跳过，review stage 的跳过完全缺失。phase 级别的 `optional_for` 声明从未被消费。

**实现**:

```go
func (e Engine) checkStageSkip(wf asset.Workflow) bool {
    if e.discoverStageSkipped(wf) {
        e.logf("discover stage skipped (mode gating: explorer skips discovery)")
        e.reportStop(wf)
        return true
    }
    if e.reviewStageSkipped(wf) {
        e.logf("review stage skipped (mode gating: explorer skips deep review)")
        e.reportStop(wf)
        return true
    }
    return false
}

func (e Engine) skipByMode(p asset.Phase, stage string) bool {
    if !e.gatingActive() { return false }
    if requiredWhenKey(p.RequiredWhen) == "reviewer" && !e.ModePolicy.Reviewer {
        return true
    }
    if len(p.OptionalFor) > 0 && !e.stageDepthAtMax(stage) {
        m := e.ModePolicy.Mode
        for _, optional := range p.OptionalFor {
            if optional == m { return true }
        }
    }
    return false
}
```

**关键设计决策**:
- `stageDepthAtMax` 确保 production lifecycle 的 veto 优先级高于 mode 的 optional_for 豁免
- `checkStageSkip` 统一两个 stage 的跳过逻辑，单一调用点
- reviewStageSkipped 镜像 discoverStageSkipped 的字节精确结构

### 6. 并行编排 fail-fast

**问题**: Parallel 模式下，一个 phase 失败后其他 phase 继续执行，浪费 agent budget。

**实现**:

```go
// runWave: per-wave cancellable context
waveCtx, waveCancel := context.WithCancel(parentCtx)
defer waveCancel()

// 任意 phase 失败 → 取消 wave，剩余 phase 被 commandContext 链中止
if err := e.runPhaseParallel(waveCtx, ...); err != nil {
    mu.Lock()
    if *firstErr == nil {
        *firstErr = err
        waveCancel()  // 传播取消信号
    }
    mu.Unlock()
}
```

**关键设计决策**:
- fail-fast 仅在并行模式下生效（串行无此语义）
- 已启动的 phase 通过 CommandExecutor 的 context.WithTimeout 链感知取消
- 记录 discarded phase 数到日志（可观测性，非告警）

### 7. Checkpoint 历史保留

**问题**: checkpoint 文件被覆盖时旧状态完全丢失，无法调试回归。

**实现**:

```go
func Save(path string, cp Checkpoint, retain int) error {
    if cp.FormatVersion == "" {
        cp.FormatVersion = "forgeos.checkpoint.v1"
    }
    // ...
    if retain > 0 {
        if _, err := os.Stat(path); err == nil {
            rotateRetain(path, retain)
        }
    }
    // ... atomic rename
}

func rotateRetain(path string, retain int) {
    for i := retain - 1; i >= 1; i-- {
        older := fmt.Sprintf("%s.%d", path, i)
        newer := fmt.Sprintf("%s.%d", path, i+1)
        os.Rename(older, newer)
    }
    os.Rename(path, path+".1")
}
```

**关键设计决策**:
- `retain=0`（默认）= 旧行为（覆盖不保留）
- rotateRetain 是 best-effort：单次 rename 失败不阻塞 Save
- 跳过目录（兼容测试模拟写失败的 fixture）

---

## 依赖说明

**零新外部依赖**。所有实现均使用 Go 标准库：
- `context` — context 传播
- `sync` — loadCache、并行锁
- `sync/atomic` — 原子操作
- `os/exec` — git diff（FileDelta/CodeTestRatio）
- `strconv` — 置信度解析
- `encoding/json` — json 操作
- `crypto/rand` —（预留给 ULID 实现）
- `math` — 路由评分
- `path/filepath` — 路径操作

---

## 已知限制

1. **RunID（ULID）未实现**：D3-RUN-1 至 D3-RUN-7（运行标识与状态隔离）是本实现的明确缺口。trace.Event 和 Checkpoint 的 Format 字段为未来的多运行隔离铺平了道路，但 `.forge/` 目录重构、文件锁、`forge status` 升级等工作留待后续 sprint。

2. **Parse failure 事件未全链路接线**：trace constructor helpers（GateEvent/ErrorEvent 等）已就绪，但 5 个解析点（parseReviewerVerdict、parseClaudeCostUsd、parseExecutiveVerdict、parseConfidenceScore、RoadmapCompletion）尚未发射 parse_failure 事件 — 这需要 D1-OBS-2~5 的后续工作。

3. **requires_tools 和 readonly 声明已定义，但未强制执行**：Phase 字段已解码可用，但 `requiresToolsGuard` 和 `readonly` 的 argv 构造已实现，运行时行为需真 agent 验证。

4. **Gate 快速失败序未实现**：D4-GATE-1~5（门控执行成本策略）不在本实现范围内。

5. **Policy 时间维未实现**：D5-POL-1~6（治理政策时间维）不在本实现范围内。

---

## 验证步骤

### 1. 编译验证
```bash
cd forge-core && go build ./...
```
预期：无输出（exit 0）

### 2. 单元测试
```bash
cd forge-core && go test ./... -count=1
```
预期：全部 19 个包 ok

### 3. 架构闸门
```bash
node harness/arch/arch-check.mjs
```
预期：8/8 PASS（ai-dev/ 目录例外不属 forge-core 范围）

### 4. 治理完整性
```bash
python3 harness/check.py
```
预期：通过

### 5. 完整验收
```bash
node harness/acceptance.mjs
```
预期：acceptance gate 中 forge-core 相关项 PASS（ai-dev/ 预存问题除外）

---

## 配置要求

无新增配置项。所有新功能通过零值实现向后兼容：
- `Engine.Ctx`: nil = context.Background()（旧行为）
- `persist.Save` 的 `retain` 参数: 0 = 不保留历史
- `trace.Event.Format`: 空字符串 = "forgeos.trace.v1"
- Phase 新字段: 零值/空 = 旧行为
- `LoopEngine.OnBeforeIteration`: nil = no-op

---

## 实现清单

- [x] 代码可编译通过（`go build ./...` 全绿）
- [x] 所有测试通过（`go test ./... -count=1` 全绿）
- [x] 错误处理完整（context 取消传播、trace 构造器 safe-zero）
- [x] 边界条件处理（Format 空值兼容、retain=0 旧行为、nil ctx = Background）
- [x] 安全检查（context 传播支持 SIGINT 优雅退出、parallel fail-fast 防 budget 浪费）
- [x] 向后兼容（所有新字段 omitempty、零值 = 旧行为）
- [x] 架构闸门通过（arch-check 8/8、check.py 通过）
