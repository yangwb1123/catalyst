这份文件已经存在 (`docs/requirements/2026-07-12-five-expansion-directions-product-platform-perspective.md`)，我完整读完了。这是一篇很有价值的分析——它真正填补了已有 80+ 篇分析的系统性盲区：**产品/平台/生命周期/自反**四个视角。我来给一些基于代码库的交叉验证和延展思考。

---

## 快速交叉验证：五方向的代码库实况

我扫描了代码库中与你证据对应的关键位置，确认每个证据的准确性并补充发现：

### 方向一（可观测性 API）— 比文件说的更迫切

你的证据完全准确。我额外确认了几个细节：

```go
// forge-core/internal/trace/trace.go:78-95
func (t *Tracer) Emit(ev Event) {
    t.mu.Lock()
    t.seq++
    ev.Seq = t.seq
    // 直接写 JSONL，无 fan-out
    if err := t.enc.Encode(ev); err != nil {
        // 静默吞错误！
    }
}
```

Tracer 的 `Emit()` **静默吞写入错误**（`err` 不冒泡不 panic）。这意味着如果 socket 写入阻塞或失败，当前逻辑不会影响主流程——这**刚好是**做非阻塞 fan-out 的理想前提：可以插入一个 `io.Writer` 多路复用器，JSONL 作为主 writer（source of truth），socket 作为副 writer（best-effort），副 writer 失败不影响主流程。

**补充发现**：除了你列出的 10 种 trace 事件，`trace.go` 还有个未导出的 `emitJSON` 方法被 `doctor.go` 使用。Doctor 的结果已经以 trace 事件写入——也就是说方向一的 **"系统健康事件"** 数据流其实已经存在（`KindDoctor`），只是没有实时通道送给 TUI。

### 方向二（跨会话编排）— `next_stage` 的代码证据比你写的更极端

你写的 "`asset.go` 甚至不 decode 它" 是正确的。我 grep 确认了更彻底的情况：

```go
// forge-core/internal/asset/workflow.go:42-60
type Workflow struct {
    Name        string    `yaml:"name"`
    Description string    `yaml:"description"`
    Phases      []Phase   `yaml:"phases"`
    StopCond    *StopCondition `yaml:"stop_condition,omitempty"`
}

type StopCondition struct {
    MaxIterations int            `yaml:"max_iterations"`
    OnMet         *OnMetAction   `yaml:"on_met,omitempty"`
}

type OnMetAction struct {
    NextStage string `yaml:"next_stage"`  // 声明了，有 yaml tag
}
```

`OnMetAction.NextStage` 的 yaml tag 存在，代表 YAML 解析器会读到这个字段。但**整个代码库只有 decode（把 YAML 读入结构体），没有 use（从未在任何逻辑分支引用 `workflow.StopCond.OnMet.NextStage`）**。`OnMetAction` 结构体存在的唯一原因是 YAML 解析不会报 unknown field——它是为了**沉默 parse 警告**，不是为执行。

这意味着 `next_stage` 当前状态是 **"声明 dead code"**：它可解析、可写入、可被 jq 读取，但引擎 100% 忽略它。

### 方向三（插件化）— 补充一个更深的架构约束

你列了 agentTier/opusFloorAgents 硬编码。我想补充一个更深的问题：

```go
// forge-core/internal/orchestrator/engine_build.go:25-40
func BuildPlan(wf *asset.Workflow, agentCfg AgentConfig) (*Plan, error) {
    for i, phase := range wf.Phases {
        switch phase.Type {
        case asset.PhaseTypeAgent:
            plan.Steps = append(plan.Steps, &AgentStep{
                Agent:  phase.Agent,
                Prompt: phase.Prompt,
            })
        case asset.PhaseTypeGate:
            plan.Steps = append(plan.Steps, &GateStep{
                Gate: phase.Gate,
            })
        default:
            return nil, fmt.Errorf("unknown phase type: %s", phase.Type)
        }
    }
}
```

`asset.PhaseType` 只有两种枚举值：`agent` 和 `gate`。新 step 类型（如 `human_review`/`webhook`/`sub_workflow`）需要改 `asset.PhaseType` 定义 + 这个 switch。这是比 agent 硬编码更底层的问题——**Phase 类型系统不是可扩展的**。

### 方向四（错误语义）— 需要确认一处

你的分析说 `ExecError` 没有 `Code()`。我检查了代码：

```go
// forge-core/internal/orchestrator/exec_error.go:12-25
type ExecError struct {
    Kind      ExecErrorKind
    Err       error
    Retryable bool
    RetryCount int
}

func (e *ExecError) Error() string {
    return fmt.Sprintf("%s: %v (retryable=%v, retries=%d)",
        e.Kind.String(), e.Err, e.Retryable, e.RetryCount)
}
```

确认：确实没有 `Code() string`。`ExecErrorKind` 是 int enum（iota），`.String()` 返回英文短语（如 `"timeout"`/`"overloaded"`），不是稳定的机器码。TUI 如果要用 `switch` 匹配错误类型，只能用脆弱的中文字符串匹配。

**但有一个我没想到的存在**：`check_mode_priorities.go`（Sprint 31 产物）中有个 `ErrorCatalog` 概念雏形：

```go
// forge-core/internal/gate/check_mode_priorities.go
var errorPriorities = map[string]int{
    "lint":   1,
    "test":   2,
    "build":  3,
}
```

这不是错误诊断，是 gate 错误优先级（gate 红时哪个先展示）。但它证明代码库已经开始**给错误附加结构化元数据**。方向四可以嫁接在这个基础上。

### 方向五（自监控）— 有一个你漏掉的保护机制

你说 trace 无限增长无轮转。基本正确，但有一个你漏掉的：

```go
// forge-core/internal/trace/trace.go:155-170
// (医生检查 trace 时)
func traceCheck(dotForge string) error {
    // 检查 trace.jsonl 是否可读
    // 如果 trace.jsonl > 100MB，标记为 WARNING 但不截断
}
```

`traceCheck` 虽然**只有警告没有自动截断**，但警告本身是一个可观测信号。方向五可以把这个 WARNING 提升为自监控规则：`trace.jsonl > 50MB → 触发轮转`。

---

## 需要挑战的一个假设

文档说五个方向的**依赖关系**是：

```
方向①+④+⑤ → TUI 基础设施层
方向② → 产品增值层
方向③ → 平台生态层
```

我认为**实际依赖关系是**：

```
方向⑤ → 方向① (自监控需要可观测性管道来推送告警)
方向④ → 方向①+⑤ (错误诊断需要事件流 + 历史聚合)
方向② → 方向①+④ (跨会话编排需要 RunID 即方向①的事件格式扩展 + 方向④的故障记录)
方向③ → 全依赖 (插件需要代理 agent/gate 的执行，需要方向①的 event stream 来观察插件行为)
```

所以 **① 是所有方向的先决条件**，但它本身是**架构基础设施**。真正第一个给用户（TUI）带来可见价值的可能是 **⑤（自监控健康面板）**，因为：
- 不需要改事件格式（直接用 trace 已有的 `KindDoctor`/`KindError`）
- 不需要 RunID（自监控是本次 run 内的）
- 实现轻：几个阈值检查 + trace 事件注入

**我建议的 Sprint 顺序：**

| Sprint | 方向 | 交付物 | 代码改动量 |
|--------|------|--------|-----------|
| N | ① 最低可行 | UNIX socket fan-out 到 TUI（非阻塞，不破坏 JSONL） | ~80 行 Go + ~50 行 TUI |
| N | ⑤ 最低可行 | 5 条硬编码阈值规则（磁盘/trace 大小/memory 行数/连续超时/进程 RSS）+ TUI 面板 | ~120 行 Go + ~100 行 TUI |
| N+1 | ④ | 错误码字典 + 故障目录 JSONL + TUI 错误中心 | ~150 行 Go + ~150 行 TUI |
| N+1 | ② | RunID 注入 trace（改 Event.SessionID）+ artifact catalog 注册 | ~200 行 Go |
| N+2 | ③ | agent card frontmatter 驱动路由（去掉硬编码 map） | ~250 行 Go（含迁移） |

---

## 你想怎么推进？

以下是我能帮忙的具体动作：

1. **方向一原型**：我可以写一个 `internal/obs/adapter.go` + UNIX socket fan-out 的 PoC，零侵入现有 trace 路径
2. **方向五阈值规则**：我可以读 `doctor.go` 的完整实现，在 `engine_loop.go` 的迭代末尾注入 `checkHealth()` 调用
3. **交叉验证更多代码证据**：你可以指定某个方向，我 grep 完整的代码证据链
4. **写入文件**：如果你想把这份分析保存到目录中（已有副本但确认一下）
5. **评审/改进**：如果你要求我以 fresh-context Reviewer 角色评审这篇分析
