# ForgeOS — 深度扫描后的三个未被已有分析覆盖的架构级缺口

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局逐文件扫描 `forge-core/`（18 Go 包, ~35k LOC）· `harness/`（39+ 模块, ~10.5k LOC）·  
> `.agent/`（12 agent 卡 / 5 workflow / 全部 policies+ADR+DECISIONS）· `examples/`· `docs/`（含 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 200+ 条目）·  
> 全部 124 篇已有 `docs/requirements/` 扩展分析。  
> **差异化验证**: 对每个方向的核心概念组合在全部 124+ 篇已有分析中进行全文关键词搜索,确认其从未作为独立扩展方向被系统分析过。  
> **纪律**: 不编写任何代码。不重复已有分析。每个方向附带精确到 `file:line` 的代码级证据、产品价值判断、边界情况。

---

## 核心判断

经过 31 轮 sprint，ForgeOS 在**功能层**已高度成熟：编排引擎（串/并行/loop-back/mode-gating/resume/checkpoint）、模型路由（Opus 安全下限 + budget guard + history tiebreak + 多维打分）、安全护栏（递归/预算/输出/超时 四维完整）、学习闭环（trace/scorecard/memory/converge 全链路就绪）、真点火 multi-agent 端到端验证通过。124 篇已有扩展分析覆盖了绝大多数可预见的功能窗口和架构债务。

但代码库中存在**三个系统性空白**——它们不是「加功能」或「补接线」，而是**运行模型的二阶缺口**:当前功能完备，但缺少在更大规模、更长周期、更复杂集成场景下必要的支撑结构。

| # | 方向 | 类型 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **收敛震荡检测与稳定化机制** | 运行时可靠性 | 🔴 **P1** | 收敛判定是二值的（已达标/未达标）,无法检测跨迭代的达标↔未达标震荡,也无法判断收敛趋势是稳定上升还是随机漂移 |
| 2 | **阶段产物契约层（Structured Artifact Contract）** | 数据流架构 | 🔴 **P1** | `emits:` 声明的产物文件以纯文本注入下游 prompt,无结构契约、无验证、无机读数据流,跨 workflow 的 PRD→Design→Build 管线存在隐含断裂 |
| 3 | **运行时外部集成面（Runtime Integration Surface）** | 平台化能力 | 🔴 **P1** | 零网络代码、零 HTTP API、零 event stream、零 metrics export——ForgeOS 是一个只能被人类从终端操作的单体 CLI,无法被 CI/CD/外部系统程序化消费 |

---

## 方向一 · 收敛震荡检测与稳定化机制

> **关键词验证**: `oscillat` 在全部 124 篇已有分析的全文中 **仅在 1 篇的某个子方向中出现过**（作为「输出稳定性契约」的附属叙述,非独立方向）。  
> `converge.*traject\|converge.*stabil\|converge.*flap` **零命中**。

### 为什么需要

#### 现状

`internal/converge` 的收敛判定模型是**纯静态二值函数**：

```go
// forge-core/internal/converge/converge.go:183-213
func (c Convergence) Evaluate(sig Signals) Result {
    for _, criterion := range c.AllOf {
        r := evalOne(criterion, sig)
        if r.Met == false {
            return Result{Met: false, Detail: r.Detail}
        }
    }
    return Result{Met: true, Detail: "all criteria met"}
}
```

每个迭代测量一次 Signals（RoadmapCompletion, GatesGreen, ReviewStatus, RequirementConfidence, FileDelta…），Evaluate 返回 Met=true/false。LoopEngine 据此决定继续还是停止：

```go
// forge-core/internal/orchestrator/loop.go:107-114
if conv.Met {
    l.logf("convergence MET after %d iterations", i)
    return conv, nil
}
```

这是**快照模型**——只看当前迭代的快照,不看历史趋势。这意味着：

1. **震荡不可见**:如果 RoadmapCompletion 在迭代 7 达到 100%（MET），迭代 8 因新 gap 发现退回 90%（NOT MET），迭代 9 又回到 100%（MET），系统只报告「迭代 9 MET」——没有人知道它在 7↔9 之间震荡过。
2. **假性收敛**:RoadmapCompletion=100% 但 GatesGreen=false（上一次迭代的修改破坏了 lint）。快照测量时如果 gates 恰好 green，判定 MET；但如果 gates flapping（一会儿绿一会儿红），MET 状态不可靠。
3. **无收敛趋势**:`LoopEngine.NoProgress` 只检测「RoadmapCompletion 连续 N 轮无增长」这一种停滞模式。无法区分「缓慢但稳定上升」「随机漂移」「先升后降震荡」。

#### 代码证据

```go
// forge-core/internal/orchestrator/loop.go:85-92
prevCompletion := l.ResumePrev
for i := l.StartIter; i <= l.MaxIter || l.MaxIter == 0; i++ {
    // ... run iteration ...
    sig := l.Signals()
    currCompletion := sig.RoadmapCompletion
    
    // NoProgress tripwire: only checks monotonic increase
    if currCompletion <= prevCompletion {
        stale++
    } else {
        stale = 0
    }
    if stale >= l.NoProgress && l.NoProgress > 0 {
        return fmt.Errorf("no progress after %d iterations", stale)
    }
    prevCompletion = currCompletion
```

NoProgress 检测是**唯一的历史感知机制**,但它只追踪一个指标（RoadmapCompletion）且只检测停滞——不检测震荡、不检测倒退、不检测收敛质量。

```go
// forge-core/internal/converge/converge.go:28-52
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    ReviewStatus      string
    RequirementConfidence float64
    FileDelta         float64
    HumanApproved     bool
    // ...    
}
```

所有信号都是**瞬态快照**。没有 `ConvergenceTrend`、`StabilityScore`、`OscillationCount` 这类历史衍生的元信号。

### 产品价值

在一个 24h 无人值守的 `forge evolve` 循环中,收敛震荡是最危险的静默故障模式之一：

| 场景 | 影响 | 当前处理 |
|------|------|----------|
| 新实现引入 lint 警告→GatesGreen=false→修复→true→下一轮又复发 | 循环始终无法稳定收敛,但每轮都报告「接近达标」 | MaxIter 硬停,报告虚假的「超时停止」而非「震荡停」 |
| implementer 和 reviewer 互相推翻（implementer 加功能→reviewer 要求改→implementer 重写→reviewer 又发现新问题） | 无限 loop-back 烧穿 budget,不产生净价值 | MaxLoopBack 硬停,但无法识别这是「有害震荡」 |
| 两个 agent 交替修改同一个文件的不同部分（planner 加 A → implementer 改 B → planner 发现 A 需调整 → …） | RoadmapCompletion 在 80%-100% 之间来回跳,系统不收敛 | NoProgress 不触发（因为 completion 在变）,MaxIter 兜底,浪费大量预算 |
| evolve 的 scan phase 每次都发现「新」gap（实际是之前已修但 scan 工具因非确定性输出误报） | 系统持续「发现→修复→再发现」同一个问题 | 无检测机制 |

### 建议扩展骨架

1. **历史收敛缓冲区**:在 `LoopEngine` 内维护一个固定长度的迭代结果环形缓冲区（例如最近 10 轮）,记录每轮的 `Signals` 快照和 `Evaluate` 结果。

2. **震荡指标**:
   - `OscillationCount(MET)`:过去 N 轮中 Met→NotMet→Met 的转换次数
   - `GatesFlapRate`:过去 N 轮中 GatesGreen 翻转的频率
   - `CompletionVariance(RoadmapCompletion)`:过去 N 轮完成度的方差——高方差意味着进展不稳定
   - `Regressions`:RoadmapCompletion 相比峰值下降的次数

3. **震荡触发动作**:
   - `oscillation_count >= 2` → 触发「收敛不稳定」告警,记录 trace event `kind="converge_oscillation"`
   - `oscillation_count >= 3` → 主动暂停 evolve（类似 NoProgress tripwire）,输出诊断:哪些指标在震荡、哪些 phase 的产出被后续 phase 推翻
   - `gates_flap_rate > 0.3` → 标记对应 gate 为「flaky」,在收敛报告中单独标注

4. **收敛质量评分**:不仅仅是 Met/NOT MET,增加 `ConvergenceQuality` = f(stability, iterations_to_converge, regression_count) 的元评分,让用户能比较不同配置的收敛表现。

### 不受影响

- `Signals` 结构体不变——缓冲区只是引擎层的额外追踪,不修改收敛契约
- `Evaluate` 函数不变——震荡检测是 `LoopEngine` 的职责,不是 `converge` 包的
- 所有现有测试不受影响——震荡检测默认关闭（零值时不追踪历史）
- `forge run` 不受影响（单次执行无迭代,不产生震荡）

### 代码入口点

| 文件 | 行号 | 对应的改动点 |
|------|------|-------------|
| `internal/orchestrator/loop.go` | 85-107 | 在迭代循环中增加震荡检测缓冲区 |
| `internal/orchestrator/loop.go` | 107-114 | 在 `conv.Met` 判断外增加震荡告警逻辑 |
| `internal/converge/converge.go` | 28-52 | （可能）增加 `Signals` 的稳定性元字段 |
| `internal/trace/trace.go` | 30-50 | 增加 `kind="converge_oscillation"` event 类型 |
| `internal/doctor/anomaly.go` | — | 检测 checkpoint 链中的收敛震荡模式 |

---

## 方向二 · 阶段产物契约层（Structured Artifact Contract）

> **关键词验证**: `artifact.*contract\|phase.*output.*schema\|output.*structur\|phase.*contract.*valid`  
> 在全部 124 篇已有分析中 **零篇作为独立方向**。

### 为什么需要

#### 现状

当前阶段间数据流模型是**纯文本注入**。每个 phase 的 `emits:` 声明它产生哪些文件,下游 phase 通过 `buildPromptWithEmits` 将文件内容作为 free text 注入 prompt：

```go
// forge-core/cmd/forge/prompt_context.go:338-350
func buildPromptWithEmits(..., emitsFiles []string) string {
    ctx := ...
    ctx = appendArtifactContext(ctx, repoRoot, emitsFiles, p.UsesTemplate, p.SecondaryTemplate)
    // ...
}
```

`appendArtifactContext` 读文件内容、拼成 `[context:emit:文件路径]` 块,直接塞进 agent 的 prompt。**没有对文件内容做任何结构验证、契约检查或字段提取**。

再看跨 workflow 的数据流。Discover 的产出（`prd.md`、`gap-report.md`）只存在于文件系统中。Design 的 architect phase 通过 `emits:` 读到这些文件的内容——但完全依赖于：
1. 文件路径正确（无验证:文件不存在时静默跳过,不报错）
2. 内容格式 agent 能理解（无验证:PRD 缺少关键章节时 agent 只能自己猜测）
3. 命名约定一致（无验证:如果 discover.yml 改了 `emits:` 路径,design.yml 不会得到任何通知）

```yaml
# .agent/workflows/discover.yml:50-51
- name: requirement-discovery
  emits:
    - docs/discovery/prd.md
  # 但没有任何地方声明 prd.md 必须包含哪些章节
```

```yaml
# .agent/workflows/design.yml:32-34
- name: solution-architect
  model_tier: opus
  # 通过 emits 机制读到 prd.md,但无法验证其完整性
```

#### 代码证据

```go
// forge-core/cmd/forge/prompt_artifacts.go
func appendArtifactContext(ctx []string, root string, emitsFiles []string, ...) []string {
    for _, path := range emitsFiles {
        data, err := os.ReadFile(filepath.Join(root, path))
        if err != nil {
            continue // ⚠ 文件不存在时静默跳过,不报错、不告警
        }
        ctx = append(ctx, fmt.Sprintf("[context:emit:%s]\n%s", path, data))
    }
    // ...
}
```

关键问题:
- **行 5**:静默跳过缺失文件——phase 声称产生但实际没产生的文件,下游完全不知道
- **行 6**:纯文本注入——没有结构提取、没有字段校验、没有 schema 验证
- **无返回值**:下游 phase 的 agent 只能「自己理解」文件内容,无法声明它期望什么结构

```go
// forge-core/internal/asset/asset.go:130-143
type Phase struct {
    Emits []string `json:"emits,omitempty"`    // 只是文件路径列表,没有结构约束
    // ...
}
```

`Emits` 字段只是一个 `[]string`——没有 schema 引用、没有必需字段声明、没有预期格式标记。

### 产品价值

ForgeOS 的脊柱（Discover→Design→Review→Build→Evolve）依赖阶段间的信息传递。当前的纯文本模型在以下场景中会产生静默故障：

| 场景 | 影响 | 当前处理 |
|------|------|----------|
| requirement-discovery phase 产出的 PRD 缺少「非功能需求」章节 | 下游 architect 不知道性能/安全约束,设计出不可部署的架构 | **静默**:architect 读到内容,如同 PRD 本来就没有这个信息 |
| solution-architect phase 产出的 proposal 没有明确的技术选型理由 | downstream reviewer 无法验证选型是否正确 | **静默**:reviewer 只能对已有内容做评审,不知道缺少关键决策记录 |
| discover 改版后 prd.md 的格式变化（例如从 markdown 转为 YAML frontmatter + body） | 所有依赖旧格式的 workflow 破裂 | **静默**:无版本号、无 schema 声明、无迁移检测 |
| plan phase 产出 task-breakdown.md,但 implementer 无法程序化提取「哪个文件需要修改」 | implementer 必须自己重新理解需求文档,浪费 token | **无结构化接口**:只能用自然语言描述,不能声明式声明依赖 |

### 建议扩展骨架

1. **产物 Schema 声明**:在 `emits` 列表的每个条目后增加可选的 schema 引用(`emits: [{path: "docs/discovery/prd.md", schema: "prd-schema-v1"}]`)。Schema 定义在 `.agent/schemas/` 下,描述该文件预期的结构、必需字段、数据类型。

2. **结构验证层**:在 `appendArtifactContext` 之前增加验证步骤:
   - 文件存在性检查（缺失 → 告警,非静默跳过）
   - Schema 验证（注册了 schema 时,按 schema 校验内容完整性）
   - 结构提取（agent 可读的结构化数据块,而非纯文本）

3. **跨 Workflow 产物依赖声明**:在 workflow 级别声明 `consumes:`，引用上游 workflow 的 `emits:` 产物：
   ```yaml
   # .agent/workflows/design.yml
   consumes:
     - workflow: discover
       artifact: docs/discovery/prd.md
       schema: prd-schema-v1
   ```
   启动时验证:上游 workflow 的 emit 路径存在、schema 兼容。

4. **版本化 Schema 迁移**:每个 schema 带版本号。当上游 emit 的 schema 版本与下游期望的版本不兼容时,阻止运行并输出清晰的迁移路径。

### 不受影响

- 现有 workflow 的 `emits: [paths...]` 格式保持向后兼容（不带 schema 时跳过验证）
- 现有 `appendArtifactContext` 行为不变（无 schema 时仍注入纯文本）
- 文件存在性检查从静默跳过改为告警但不阻断（防止现有 workflow 因缺失文件而崩溃）
- 跨 workflow 依赖是 opt-in：不为现有 workflow 增加新要求

### 代码入口点

| 文件 | 行号 | 对应的改动点 |
|------|------|-------------|
| `forge-core/internal/asset/asset.go` | 130-143 | Phase.Emits 从 `[]string` 扩展为 `[]EmitEntry`（含 path + schema 引用） |
| `forge-core/cmd/forge/prompt_artifacts.go` | 第 5 行附近 | 增加存在性检查告警 + schema 验证（可选） |
| `.agent/schemas/` | 新建目录 | 存放 schema 定义（JSON Schema 或简化 DSL） |
| `forge-core/cmd/forge/validate.go` | — | 增加 `forge validate --artifacts` 跨 workflow 产物依赖校验 |

---

## 方向三 · 运行时外部集成面（Runtime Integration Surface）

> **关键词验证**: `integration.*surface\|programmatic.*consum\|extern.*consum.*forge\|forge.*api.*third`  
> 在全部 124 篇已有分析中 **零篇作为独立系统性方向**。  
> `HTTP.*server\|net\.http\|event.*stream.*runtime\|metrics.*endpoint` 在 `forge-core/**/*.go` 中 **零代码**。

### 为什么需要

#### 现状

ForgeOS 的消费模型是**纯 CLI + 纯文件**：

```
# 所有对外接口
forge run            → stdout 文本 + 文件写入 + exit code
forge evolve         → stdout 文本 + .forge/(checkpoint|trace|memory) 文件写入
forge accept         → stdout 文本 + exit code
forge scorecard      → stdout 文本 + scorecards.json 写入
forge doctor/status  → stdout 文本
```

没有任何方式可以程序化地：
- 查询正在运行的 workflow 的状态（一个 `forge evolve` 开始后，外面无法知道它跑到第几轮）
- 触发一个 workflow 并等待结果（需要手动 `forge run`，无法从 CI 调用）
- 订阅运行时事件（gate 失败、phase 完成、收敛达到——都只能事后读 JSONL）
- 集成到现有工具链（Slack 通知、Prometheus 告警、Grafana 面板都只能靠 wrapper shell 脚本解析文本 stdout）

```go
// forge-core/cmd/forge/main.go — 零网络代码
func main() {
    os.Exit(run(os.Args[1:]))    // 纯粹的 CLI 入口
}

// forge-core/cmd/forge/evolve.go:380-460 — 运行时状态只写文件
tracePath := filepath.Join(root, ".forge", "trace.jsonl")
// ...
checkpoint.Save(...)           // 写到 .forge/checkpoint.json
memory.Append(...)             // 写到 .forge/memory.jsonl
```

```go
// forge-core/internal/orchestrator/orchestrator.go — 事件输出的唯一通道
type Engine struct {
    Log func(string)    // 唯一的运行时输出：一行文本
    // ...
}
```

`Log func(string)` 是跑完运行时获取信息的唯一方式——没有结构化回调、没有事件对象、没有流式输出。

```go
// forge-core/internal/orchestrator/loop.go:70-75
type LoopEngine struct {
    // OnIteration 是唯一的运行时观察点
    OnIteration func(i int, sig converge.Signals, durationMs int64)
    // 但这个回调只能被 cmd/forge 使用——非导出,外部不可接入
}
```

#### 为什么这是架构级缺口

ForgeOS 宣称自己是「AI-native 软件工厂的控制平面」（north-star.md:+3），但**控制平面的定义是提供 API**——Kubernetes 有 kube-apiserver，Temporal 有 gRPC 和 Web UI，GitHub Actions 有 REST API + webhooks。ForgeOS 的控制信息封闭在 CLI 进程中，外部世界无法感知。

现有架构中事实上已经存在了构建外部接入面的所有原材料：

| 已有能力 | 当前消费者 | 可暴露的 API |
|---------|-----------|-------------|
| `trace.Event`（结构化运行时事件） | 文件 JSONL | WebSocket event stream / gRPC stream |
| `converge.Signals`（收敛信号快照） | LoopEngine 内部 | REST `/status` 端点 |
| `gate.Result`（闸门裁决） | Engine 内部 + Ledger | REST `/gates` 端点 |
| `doctor.Status`（运行时健康状态） | CLI stdout | REST `/health` 端点 |
| `forge run/evolve`（workflow 执行） | 命令行参数 | REST `/workflows/{name}/run` |
| `checkpoint.Checkpoint`（持久化运行时快照） | crash recovery | REST `/run/{id}` 端点 |

### 产品价值

外部集成面是 ForgeOS 从「个人 AI 辅助工具」走向「团队级 / 企业级平台」的**必要非充分条件**：

| 场景 | 当前 | 有 API 后 |
|------|------|-----------|
| CI 流水线集成 | shell 脚本包装 `forge accept`,解析 exit code | webhook 触发 → HTTP POST 回调 → 结构化 JSON 结果 |
| Slack/Teams 通知 | 必须自己写 wrapper 解析 stdout | 注册 webhook URL → 运行时自动推送 gate 失败、收敛事件 |
| Dashboard / 大屏监控 | 必须自己写 cron `forge status --json > /dev/null` | SSE/WebSocket 实时 stream,Prometheus 指标端点 |
| 多项目中央调度 | 不可能——每个项目独立 CLI | `forge-server` 管理多个 root,提供统一 API |
| 人类审批集成 | CLI `--approved` 或文件 marker | API `POST /approve/{runId}`，Slack action 可直接回调 |
| 自动化预算告警 | 无——预算超限只写在 checkpoint 中 | Prometheus `forge_budget_remaining` 指标 + Alertmanager 告警 |

### 建议扩展骨架

这是一个**分阶段扩展**：

**Phase A（最小可行 API, ~1 sprint）**:

1. **只读状态端点**:forge-core 启动一个 Unix domain socket（`/tmp/forge-<pid>.sock`），暴露一组 JSON 只读 API：
   - `GET /status`——当前 workflow / mode / lifecycle / iteration / convergence status
   - `GET /signals`——最新 Signals 快照（RoadmapCompletion / GatesGreen / …）
   - `GET /events`——实时 event stream（SSE 格式）,推送 trace.Event 事件
   - `GET /health`——运行中 / 空闲 / 错误

2. **实现策略**:不引入 HTTP 框架——使用 Go 标准库 `net/http` + `net.UnixListener`（零外部依赖）。作为 `cmd/forge` 的可选启动参数 `--api-socket /tmp/forge.sock`。

3. **安全边界**:Unix socket 的文件权限 0700（同主机其他用户不可访问）。仅监听 `AF_UNIX`，不监听 TCP（防止网络暴露）。

**Phase B（读写 API, ~2 sprints plus）**:

1. **触发端点**:`POST /run` 启动 workflow，返回 run ID
   - `GET /run/{id}` 查询运行状态和结果
   - `POST /run/{id}/approve` 相当于 `--approved`
   - `POST /run/{id}/cancel` 发送取消信号

2. **Webhook 注册**:`POST /webhooks` 注册回调 URL
   - 事件类型：gate_failed / phase_completed / converge_met / budget_exhausted / error
   - 运行时自动向注册的 URL 推送 JSON 事件负载

**Phase C（平台化, v3+）**:

1. **Prometheus 指标端点**:`GET /metrics` 暴露：
   - `forge_gates_total{gate,status}`——gate 结果计数
   - `forge_phase_duration_ms{phase,workflow}`——phase 耗时
   - `forge_budget_spent_usd{project}`——累计花费
   - `forge_iterations_total{workflow}`——迭代计数
   - `forge_convergence_status`——是否达到收敛

2. **长期运行模式**:`forge serve` 作为守护进程运行，保持 `.forge/` 监控、定时 `forge doctor` 健康检查、API 保持可用。

### 不受影响

- CLI 模式和 API 模式并行存在,`--api-socket` 是可选参数
- 单项目本地使用时不需要 API surface，一切行为不变
- 零外部依赖约束保持——Unix socket + `net/http` 是 Go 标准库
- `forge accept` 的 exit code 仍然是 CI 集成的主要接口，API 是可替代的增强
- 安全性：Unix socket + 文件权限是主机本地隔离，不引入网络攻击面

### 代码入口点

| 文件 | 行号 | 对应的改动点 |
|------|------|-------------|
| （新文件）`cmd/forge/api.go` | — | Unix socket HTTP server + 只读端点 |
| `cmd/forge/main.go` | 65-76 | 新增 `--api-socket` flag + 启动 API server goroutine |
| `cmd/forge/evolve.go` | 380-460 | 在 LoopEngine.OnIteration 中推送 event stream |
| `internal/orchestrator/orchestrator.go` | `Log func(string)` | 可扩展为结构化 Event 回调（向后兼容） |
| `internal/trace/trace.go` | 30-50 | event stream 复用 trace.Event 结构,增加序列化格式 |
| `internal/orchestrator/loop.go` | 70-75 | OnIteration 可同时向 API 推送 event |

---

## 边界情况汇总（跨方向）

| # | 场景 | 影响方向 | 说明 |
|---|------|---------|------|
| 1 | discover 的 PRD 生成一半时 forge 崩溃,prd.md 只有前半内容 | 方向二 | 产物存在但不完整,当前静默跳过检查不存在校验 |
| 2 | evolve 第 15 轮 RoadmapCompletion=100%,第 16 轮 GatesGreen 因新代码问题变 false,第 17 轮 true | 方向一 | 震荡被 MaxIter 掩埋,无人知晓 |
| 3 | 两个 `forge evolve` 同一时刻在同一个 .forge/ 目录上运行 | 方向三 | 无 API 层导致进程间协调完全缺失,race condition |
| 4 | CI 环境中调用 `forge run` 但无法等待其完成（超时控制） | 方向三 | 无 HTTP API,只能靠超时 kill 进程,无法优雅取消 |
| 5 | discover.yml 的 `emits:` 路径改了但 design.yml 未更新 | 方向二 | design 的 architect 静默读不到 PRD,只能凭空设计 |
| 6 | NoProgress 不触发但收敛在 85%-95% 之间随机漂移 30 轮 | 方向一 | 方向一的震荡检测可以捕捉这类「dead zone」收敛 |
| 7 | 外部 Slack 命令希望触发 `forge run build` 并获取 gate 结果 | 方向三 | 需要 API surface 才能实现非终端触发 |
| 8 | 多团队共享一个 ForgeOS 实例,各自项目有不同的 budget cap | 方向三 | 需要 API layer 提供多项目隔离和 aggregated 查询 |

---

## 附录:与已有 124 篇分析的关系

| 已有分析高频覆盖域 | 本文不重复 | 方向一（震荡） | 方向二（契约） | 方向三（集成面） |
|-------------------|-----------|--------------|--------------|----------------|
| 编排引擎（串/并行/loop-back/mode-gating/resume） | ✅ | 本方向在引擎已有能力之上增加稳定性检测 | — | — |
| 学习闭环（trace/scorecard/memory/converge） | ✅ | 本方向扩展 converge 的信号模型 | — | 本方向将 trace 从文件消费扩展到实时 stream |
| 生产韧性（529/退避/护栏/超时/递归守卫） | ✅ | 震荡检测是韧性维度之一 | — | — |
| 安全纵深（secret-scan/readonly/SCA/prompt 注入） | ✅ | — | 产物契约有安全含义（验证 agent 输出完整性） | — |
| 二阶伴生（配置爆炸/TOCTOU/无声丢失） | ✅ | — | 产物契约解决「无声丢失」的一种具体形式 | — |
| CLI DX / shell 集成 / 配置管理 | ✅ | — | — | 集成面是对 CLI DX 的补充,非替代 |
| 第三地平线（多仓库联邦/Web UI/事件驱动） | ✅ | — | — | 集成面是 Web UI 和事件驱动的前置条件 |
| 跨仓库 / polyrepo 编排 | ✅ | — | 跨仓库场景依赖产物契约 | 跨仓库场景依赖 API 集成面 |
