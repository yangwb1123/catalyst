# ForgeOS — 全局扫描后四个源自代码级观察的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐文件扫描 `forge-core/` (18 Go 包, ~35k LOC) · `harness/` (39+ 模块, ~10.5k LOC) ·  
> `.agent/` (12 agent 卡 / 5 workflow / 全部 policies+ADR+DECISIONS) · `.ai/` (10 prompt 模板) ·  
> `examples/` (2 dogfood 应用) · `docs/` (ADR · 分析 · 需求) · 全部 126+ 篇已有 `docs/requirements/`  
> 扩展分析 + 40 篇 `docs/analysis/` 深层分析交叉核对。  
> **差异化验证**: 对每个方向的核心机制在全部已有分析文档中执行精确或模糊关键词搜索,
> 确认其核心理念从未作为独立系统性方向被论证 (侧栏提及或附属性质的分析不计为覆盖)。  
> **纪律**: 不编写任何代码。不重复已有分析。每个方向附带精确到 `file:line` 的代码级证据、
> 产品价值判断与诚实边界。  
> **日期**: 2026-07-11

---

## 核心判断

经过 31+ sprint 迭代, ForgeOS 在**运行时引擎层**已高度成熟:
- 编排引擎: 串行/并行/loop-back/mode-gating/resume/checkpoint phase 级粒度 全到位
- 模型路由: Opus 安全下限 + BudgetGuard + HistoryTiebreak + 多维打分 + 成本感知降档
- 安全护栏: 递归深度 · 执行次数 · 墙钟 · 输出大小 四维完整
- 学习闭环: trace → scorecard → memory → converge 全链路就绪, 含 3D 遥测
- 治理执法: 8 项架构检查 + secret-scan + SCA + N/A 豁免矩阵 + lifecycle 感知

但 126+ 篇已有扩展分析文档已覆盖了绝大多数可预见的功能窗口。本文聚焦的是 **代码级微观观察 —— 结构与数据流中的「无形的墙」**: 那些当前代码 "能工作" 但因为缺少某一层抽象而限制了平台化潜力的设计决策。

| # | 方向 | 类型 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **Agent Output 形式化契约层** | 数据流架构 | 🔴 **P1** | 当前只有 3 个机读输出契约 (VERDICT/CONFIDENCE), 由 ad-hoc 字符串匹配解析, 无正式 schema 系统; 每个 phase 的 stdout 本质上是 "文本黑洞" —— 下游以纯文本消费, 无法被程序化校验或交叉引用 |
| 2 | **跨阶段结构化数据管线** | 数据流架构 | 🔴 **P1** | `feeds_forward` 以纯文本注入下游 prompt, 无结构化中间表示; planner 输出的任务列表、implementer 的变更清单、reviewer 的逐条发现 — 都沉没在自由文本中, 不能被下游 phase 程序化引用或溯源 |
| 3 | **Agent 输出落地验证与 Claim 接地** | 运行时信任 | 🟠 **P1** | `emits: [plan.md]` 声明了但永不验证文件是否实际产生; `readonly: true` 声明了但永不验证 agent 是否真没写文件; planner 说 "改 X" + implementer 说 "已改 X" = 没有任何检查 X 是否真的被改 |
| 4 | **声明-执行-遥测 三层状态版本对齐** | 运行时一致性 | 🟠 **P2** | 工作流定义、agent 卡、路由策略在运行时从当前文件系统读取; resume 不保存 workflow 快照、不保存 routing 决策、不保存 git commit; 同一个 checkpoint 在环境不同时产生不同的执行结果 |

---

## 现有分析中与本文件最接近但不同的方向 (以此证明新颖性)

| 已有分析中的相近方向 | 来源 | 与本文的区别 |
|---|---|---|
| **结构化产物契约 (Structured Artifact Contract)** | `forgeos-three-architectural-gaps.md` 方向二 | 关注的是 `emits:` 产生的 FILE 的结构化内容 (如 JSON schema 验证)。本文关注的是 agent 的 STDOUT 响应 — phase 输出的运行时结构化, 不是写入磁盘的文件格式 |
| **Output Merging & Conflict Resolution** | `expansion-directions-v4.md` 方向二 | 关注并行 phase 的文件级合并策略。本文关注的是单个 agent phase 输出如何被形式化描述和验证 |
| **Promise-Keeping 审计** | `five-uncovered-architectural-frontiers.md` | 关注 roadmap completion 的 honesty warning (FileDelta 交叉验证)。本文关注的是跨 phase 间的声明一致性 (planner→implementer→reviewer) |
| **Agent 输出自我一致性** | `five-uncovered-architectural-frontiers.md` 方向二 | 关注 agent 自身输出与注入 context 的矛盾 (hallucination 审计)。本文关注的是跨 phase 的宣言-执行-验证链条 |
| **输出版本化 / Verdict 格式漂移** | `second-order-architectural-gaps.md` 方向四 | 关注 VERDICT 字符串格式在 agent 版本间的兼容性。本文关注的是**更广泛的** phase 输出形式化——不限于 VERDICT, 而是所有 agent phase 的输出都能被结构化 |
| **工作流快照 / Rollback** | `strategic-expansion-and-edge-cases.md` / `expansion-directions-v4.md` 方向四 | 关注 checkpoint 快照用于回滚或调试 replay。本文方向四关注的是执行一致性: 相同 checkpoint 在不同环境下是否产生相同结果 |
| **配置表面积优化** | `configuration-surface-and-adoption.md` | 关注配置跨文件一致性校验。本文方向四关注的是**运行时**而非静态配置的版本对齐 |

---

## 方向一 · Agent Output 形式化契约层

> **代码证据**: `cmd/forge/cost.go` 中 3 套 ad-hoc 解析器 · `asset.asset.go` 中 Phase 结构体无输出 schema 字段  
> **性质**: 数据流架构缺口 · **优先级: P1**  
> **已有覆盖验证**: 关键词 `agent output contract` / `输出契约` / `输出合约` 在 126+ 篇已有分析中零命中 (结构化产物契约方向 ≠ agent stdout 契约)

### 现状

当前 ForgeOS 中, agent phase 的输出是 **非结构化文本**。其中有两条约定俗成的「机读最后一行」契约, 由 `cmd/forge/cost.go` 中的 ad-hoc 解析器提取:

```go
// forge-core/cmd/forge/cost.go:345-352
// VERDICT: APPROVE / VERDICT: REQUEST_CHANGES — 评审者输出
func parseReviewerVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    switch last {
    case "VERDICT: " + VerdictApprove:      return VerdictApprove, true
    case "VERDICT: " + VerdictRequestChanges: return VerdictRequestChanges, true
    default: return "", false  // 缺失/错误 → 无信号
    }
}
```

```go
// forge-core/cmd/forge/cost.go:383-395
// CONFIDENCE: <N> — 产品经理置信度
func parseConfidenceScore(output string) (score float64, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    numStr, hasPrefix := strings.CutPrefix(last, confidenceContract)
    if !hasPrefix { return 0, false }
    // ... Atoi + [0,100] 范围校验
}
```

```go
// forge-core/cmd/forge/cost.go:363-380
// 5 种执行裁决: APPROVE / APPROVE_WITH_SIMPLIFICATION / REDESIGN / DELAY / REJECT
func parseExecutiveVerdict(output string) (verdict string, ok bool) {
    // 结构同上, 五选一匹配
}
```

**它们是合理的临时方案, 但存在 4 个结构性限制:**

**限制 1: 无 schema 系统, 每新契约需要新解析器。** 如果要给 planner 添加一个结构化的任务分割输出 (例如 `"tasks": [{"id": "T1", "desc": "..."}]`), 需要新增第 4 个 ad-hoc 解析器。当前 3 个解析器 (`parseReviewerVerdict` · `parseConfidenceScore` · `parseExecutiveVerdict`) 都是手写的字符串匹配, 没有公共基类或注册机制。

**限制 2: 解析器位置跨层分散。** `parseReviewerVerdict` 在 `cmd/forge/cost.go` — 一个负责**成本遥测**的文件, 却同时承担了评审者裁决的解析。这是功能归属的错位: 成本遥测层不应该知道 reviewer 的裁决格式, 就像它不应该知道 confidence 格式一样。

**限制 3: 解析失败吞没无声。** 三个解析器在不可解析的输出上统一返回 `ok=false`。当 reviewer 的输出格式因 agent 版本升级而产生细微变化时 (例如 `"VERDICT:APPROVE"` 无空格、或者 `"Verdict: APPROVE"` 大小写), 解析器返回 false → 引擎 fail-open → loop-back 失效。这在 trace 中没有任何 `"verdict_parse_failed"` 事件记录。

**限制 4: 无结构化输出复用。** planner phase 输出了一份详细的任务分割计划, 但 implementer phase 的 prompt 只能以纯文本重新注入, 无法做「planner 列出了 3 个任务, implementer 完成了 2 个, 请补充第 3 个」这类结构化跟踪。

### 方向建议

引入一个轻量的 **Phase Output Contract 注册系统**:

```
forge-core/internal/output/
├── contract.go       # Contract 接口: Name() string + Parse(output string) (Result, error)
├── registry.go       # 按 phase name 注册 / 查找 Contract
├── reviewer.go       # VERDICT: APPROVE|REQUEST_CHANGES
├── executive.go      # 5 种执行裁决
├── confidence.go     # CONFIDENCE: <N>
└── planner.go        # (未来) 结构化任务列表
```

关键设计决策:
- **Contract 是 opt-in**: 只有声明了输出契约的 phase 才会被解析; 无契约的 phase 输出原样传递 (向后兼容)
- **Result 是泛型接口**: 每个契约定义自己的 Result 类型, 由消费者 (converge/reviewer loop-back/feeds_forward) 按需断言
- **解析失败显式记录**: 不可解析的输出产生 `Kind: "decision", Detail: "output contract XXX parse failed for phase YYY"` trace 事件, 并返回 `ok=false` 而非静默吞没
- **位置修正**: 从 `cmd/forge/cost.go` 迁移到独立的 `internal/output/` 包, 回归正确的功能归属

### 产品价值

1. **可扩展性**: 新增一个结构化输出契约 = 实现一个 ~20 行的 `Contract` 接口 + 注册。不再需要写 ad-hoc 字符串匹配 + 跨文件散落定位
2. **可诊断性**: `forge trace --kind decision --detail "parse failed"` 直接看到所有解析失败的 phase, 而非从「reviewer loop-back 没触发」反向推测
3. **未来能力预备**: 有了契约系统后, 一个 phase 的输出可以结构化地传递给另一个 phase (方向二), 并且可以被验证 (方向三)
4. **测试性**: 每个 Contract 的实现可以独立单元测试, 不再需要 `cost_test.go` 中硬编码字符串 fixture

### 诚实边界

- 不改变现有 phase 的输出格式 — 所有现有 agent 卡仍输出 `VERDICT: ` / `CONFIDENCE: `, 在新系统中通过包装器兼容
- 一次只迁移一个契约。迁移顺序: reviewer (已最成熟) → confidence → executive → (可选) planner
- 契约解析失败仍然 fail-open (proceed), 与当前行为一致; 改变的是 **可观测性**: 失败会被记录, 不再沉默

---

## 方向二 · 跨阶段结构化数据管线

> **代码证据**: `asset.Phase.FeedsForward` 纯 bool · `prompt_context.go` `buildPrompt` 以 `\n` 拼接 feeds_forward 文本  
> **性质**: 数据流架构缺口 · **优先级: P1**  
> **已有覆盖验证**: 关键词 `structured feeds_forward` / `结构化数据流` / `跨阶段数据管线` 在 126+ 篇已有分析中零命中

### 现状

`asset.Phase.FeedsForward` 是一个 `bool`, 语义是「此 phase 的输出应该被记住并注入下游 prompt」。

当前实现路径:
1. `Observe` sink 在 `command_executor.go` 中收到 phase 的输出文本
2. 如果 `feedsForward(phaseName)` 为 true, `phaseOut.record(name, output)` 将**纯文本**存入 `phaseOutputLedger`
3. `buildPrompt` 在组装下一个 phase 的 context 时, 调用 `phaseOut.get(name)` 获取该文本, 以 `\n---\n` 拼接注入 prompt

```go
// forge-core/cmd/forge/prompt_context.go (大致路径)
func buildPrompt(...) string {
    // ...
    if prev := phaseOut.get(feedsForwardPhase); prev != "" {
        ctx += fmt.Sprintf("\n---\nPrior phase output (%s):\n%s\n---\n", name, prev)
    }
    // ...
}
```

**这是将 structured knowledge 降级为 plain text 注入, 丢失所有结构信息。**

具体问题:

**1. 无法程序化引用特定项。** planner 输出「任务 T1: 实现认证中间件; 任务 T2: 添加日志」。
implementer 的 prompt 看到的是整段文本。无法做: 「implementer, T1 已完成, 现在做 T2」或
「reviewer, 只检查 T1 的实现」。这全都依赖 LLM 从纯文本中自己理解和拆分 — 这是**认知负荷转嫁**,
也是 token 浪费。

**2. 无版本/无溯源/无增量。** feeds_forward 的输出在每次 iteration 后覆盖 (phaseOutputLedger
是 per-run 的最后一个记录)。无法追溯 T1 在哪次 iteration 中被修改过, 也无法让下游 phase
只看到 delta 而非全量。

**3. 跨 workflow 管线断裂。** ForgeOS 的 vision 是 Discover → Design → Build → Evolve 管线,
但 feeds_forward 只在一个 workflow 内生效。discover 产出的需求文档 → design 的输入是「构建 prompt
时重新读取文件」, 不是 feeds_forward 管线。跨 workflow 的数据流完全不存在。

### 方向建议

引入 **结构化数据节点 (Structured Data Node, SDN)** 系统:

```
.forge/data/                           # 跨 session 持久化
├── T1.status  → "done"               # 任务状态 (机读)
├── T1.owner   → "planner"             # 来源
├── T1.files   → ["auth/middleware.go"] # 相关文件
└── discover-20260711/                 # 跨 workflow 的数据
    ├── requirements.json              # discover 产出
    └── confidence.json                # 置信度评分
```

核心改变:
1. **`FeedsForward` 从 bool 扩展为结构**: `FeedsForward struct { As string; Schema string }` — 声明输出的数据类别 (e.g. `task_plan`) 和可选的 schema 引用
2. **每个数据节点有类型标识**: 不是「纯文本 `\n---\n` 注入」, 而是按类型调用对应渲染器:
   - `task_plan` → 渲染为「当前 iteration 聚焦任务: T2」
   - `review_findings` → 渲染为「未关闭的 findings: F1(status=open) F3(status=open)」
3. **跨 workflow 引用**: `build.yml` 声明 `stage_in: discover` 时, 运行时自动从 `.forge/data/discover-*/` 加载需求数据注入 prompt (而非仅从文件系统重读 spec)

### 产品价值

1. **Token 效率**: 不再将整段原始文本重复注入; 按需选择、渲染数据的结构子集。典型 `feeds_forward` 文本 ~500-2000 token → 渲染后可能 ~100-300 token
2. **跨 iteration 增量**: 第 1 次 iteration 的任务列表不再在第 10 次完整重放; 下游 phase 只看到「哪些是新的, 哪些状态变了」
3. **管线连续性**: `forge run discover` 的产出 → `forge run design` 自动以结构化数据形式可用, 无需靠文件系统约定传参
4. **可调试性**: `forge status --data` 查看所有活跃的数据节点、状态和来源, 而非 trace 中不可解析的自由文本

### 诚实边界

- 纯向后兼容: 没有 `FeedsForward.As` 声明的 phase 仍然以纯文本注入 (当前行为)
- v1 只加渲染器 + 存储, 不加跨 workflow 自动引用 (那是 v2)
- 数据节点存储在 `.forge/data/` 目录, 与 checkpoint/trace/memory 同级; 大型项目需要数据生命周期管理 (方向四的延伸)
- 结构化渲染器是 PROMT-SIDE 的优化, 不改 engine/loop/orchestrator 的任何状态机逻辑

---

## 方向三 · Agent 输出落地验证与 Claim 接地

> **代码证据**: `asset.Phase.Emits` 解析后无人校验 · `readonly` 解析后无人执法 · `converge.FileDelta` 粗糙关键词匹配  
> **性质**: 运行时信任缺口 · **优先级: P1**  
> **已有覆盖验证**: 关键词 `emits.*verify` / `落地验证` / `claim verification` 在 126+ 篇已有分析中零命中

### 现状

ForgeOS 的工作流声明了几个与 agent 输出相关的保障性字段, 但它们都**只声明、不执行**:

**证据 A: `emits: [path...]` 声明但永不验证文件是否产生**

```go
// forge-core/internal/asset/asset.go:139-146
// Emits is an OPTIONAL list of file paths that this phase is declared to produce.
// When populated, the prompt builder can read and inject the actual content of
// emitted files into downstream phases that depend on them.
//
// A zero-value (nil/empty) means "this phase emits no declarative artifacts"
Emits []string `json:"emits,omitempty"`
```

代码自述 "the prompt builder can read and inject" — 这是说「如果文件存在, 可以注入」, 不是说「验证文件被创建」。当前路径:
- `buildPrompt` 调用 `appendArtifactContext` 扫描 `emits` 路径
- 对存在的文件: 注入内容
- 对不存在的文件: **静默跳过**, 无 warning, 无 trace 事件

**证据 B: `readonly: true` 声明但永不验证 agent 是否真没写文件**

```go
// forge-core/internal/asset/asset.go:184-191
// Readonly is an OPTIONAL marker...that the phase's agent is expected to
// only read, never write product code or other state.
// ADDED HERE ONLY: ...but nothing in forge-core enforces it yet
Readonly bool `json:"readonly,omitempty"`
```

当前实现只做 `narrateReadonly` (cli 层日志叙述) + `readonlyToolScope` (给 `claude` 传 `--disallowedTools "Edit Write"` 参数)。
这依赖 Claude CLI 自身的执行, 没有 forge-core 层面的独立验证。

**证据 C: `converge.go:FileDelta` 粗糙关键词匹配**

```go
// forge-core/internal/converge/converge.go:122-127
// FileDelta is the fraction in [0,1] of roadmap items that have corresponding
// file-system changes in the git diff. Computed from `git diff --name-only`
// matched against roadmap item keywords.
FileDelta float64
```

这只是一个**软告警** (`" ⚠ honesty: roadmap=100% but file-change coverage=30%"`), 不做为收敛判定。
且匹配是关键词级别的粗糙匹配 — 不是语义验证。

**这三个证据指向同一个结构性缺口: agent 的输出 (尤其是「声称已完成的工作」) 没有 system-level 的验证机制。** 信任完全建立在「agent 不会撒谎」的假设上。

### 方向建议

引入 **Post-Phase Verification 框架** — 一个轻量的、在每个 agent phase 完成后运行的可选验证器:

```
forge-core/internal/verify/
├── verify.go           # Verifier 接口: Verify(phase, output, root) → []Finding
├── emits_check.go      # phase声明 emits: [plan.md] → 检查 plan.md 是否存在
├── readonly_check.go   # phase声明 readonly: true → git diff 检查是否修改了非 emits 文件
├── file_change.go      # phase声明改变文件集 → git diff 检查哪些文件实际变化
└── registry.go         # 按 phase 特性自动激活对应验证器
```

每个验证器的输出是 `[]Finding`:
```go
type Finding struct {
    Verifier string   // "emits_check"
    Phase    string   // "planner"
    Severity string   // "error" | "warn" | "info"
    Message  string   // "declared emits [plan.md] but file was not created"
}
```

集成方式:
- **默认 advisory 模式**: 验证器运行但结果只记录 trace 事件 + 注入 converge 报告, 不阻断 run (向后兼容)
- **可选 blocking 模式**: workflow 声明 `verify: {emits: block, readonly: block}` 时, 验证失败 ⇒ gate 等价于 red
- **Converge 信号扩展**: `Signals{ Verified: bool }` — 「所有声明了验证的项都通过」作为可选的收敛条件

### 产品价值

1. **信任基线**: 从「agent 说做了 → 信」变为「agent 说做了 → 验证 → 确认」。`readonly` 从「请遵守」变为「禁止 + 验证」
2. **归因精度**: 当 implementer 声称改了文件 X 但 git diff 无变化时, 验证器产出机读 Finding。reviewer 可以直接引用这个 finding, 而不是在自由文本中模糊地「我没看到 X 的改动」
3. **收敛质量**: `Verified` 作为收敛条件的一部分, 防止 agent 虚假申报 100% 但实际产出缺失的场景
4. **可审计**: `forge trace --kind finding` 列出所有验证发现。`forge status --findings` 查看当前未关闭的验证项

### 诚实边界

- 验证是 OPT-IN: 只有声明了 `emits` / `readonly` 的 phase 被检查; 无声明的 phase 行为不变
- 验证是 ADVISORY (默认): 不阻断 run, 只在 converge 报告中显式标记 「⚠ emits mismatch: X」
- 文件系统验证基于 git diff: 只检查 tracked 文件的变化; untracked/ignored 文件不在范围内
- `readonly` 验证在并行模式下可能误报 (一个并行 phase 的合法文件被另一个 phase 修改) — 需要 wave 级别的文件变更隔离

---

## 方向四 · 声明-执行-遥测 三层状态版本对齐

> **代码证据**: `resumeStart()` 不保存 workflow 版本 · `checkpoint.json` 不保存 routing 决策  
> **性质**: 运行时一致性缺口 · **优先级: P2**  
> **已有覆盖验证**: 关键词 `execution fingerprint` / `执行指纹` / `rerun consistency` / `运行时状态对齐` 在 126+ 篇已有分析中零命中

### 现状

ForgeOS 的运行时一致性依赖以下隐含假设:

**假设 1: 工作流定义在执行期间不变。** 当 `forge evolve` 运行时, 工作流 YAML 在进程启动时从文件系统加载一次。但 LoopEngine 使用同一个 Engine 对象在多个 iteration 间复用。如果用户在 iteration 3 时编辑了 `.agent/workflows/build.yml`, iteration 4 的 phase 列表就可能与 iteration 1-3 不同。Go 结构体已加载到内存中不会自动变化, 但 `resume` 跨进程时会重新加载文件:

```go
// forge-core/cmd/forge/evolve.go:234-240
func execLoop(ctx context.Context, wf asset.Workflow, ...) int {
    // wf 是调用时传入的, resume 时被重新解析
    loop, ... := buildTracedLoop(ctx, wf, o, maxIter, ...)
    // ...
}
```

**假设 2: Resume 时路由决策与原始运行相同。** `resumeStart` 从 `checkpoint.json` 只恢复 iteration、roadmap_completion、gates_green 和 phase_index。不恢复:

```go
// forge-core/internal/persist/checkpoint.go
type Checkpoint struct {
    Workflow           string  `json:"workflow"`
    Mode               string  `json:"mode"`
    Iteration          int     `json:"iteration"`
    RoadmapCompletion  float64 `json:"roadmap_completion"`
    GatesGreen         bool    `json:"gates_green"`
    PhaseIndex         int     `json:"phase_index,omitempty"`     // ← 新加
    SpentUsdMicros     int64   `json:"spent_usd_micros,omitempty"` // ← 新加
    Reason             string  `json:"reason"`
    UpdatedAtUnix      int64   `json:"updated_at_unix"`
}
```

**缺失的关键字段**:
- `WorkflowChecksum string` — 工作流文件的哈希, 用于检测 resume 时定义是否变化
- `RoutingDecisions map[string]string` — 每个 phase 运行时的路由决策 (模型/超参)
- `GitCommit string` — 运行时的 HEAD commit
- `ModelVersion string` — 使用的模型版本 (e.g. `claude-sonnet-4@20260701`)

**假设 3: 遥测数据属于同一个「run identity」。** `.forge/trace.jsonl` 和 `.forge/memory.jsonl` 是按文件追加的, 没有 run 边界标记。多个 runs 写入同一个文件, trace 事件只有 seq 和 timestamp, 没有 run_id 字段来区分:

```go
// forge-core/internal/trace/trace.go:46-63
type Event struct {
    Format  string `json:"_format,omitempty"`
    Seq     int    `json:"seq"`
    Kind    string `json:"kind"`
    Name    string `json:"name"`
    Status  string `json:"status"`
    // ... 没有 run_id
}
```

这意味着:
- 一次 crash 后 resume 产生的 trace 事件与 crash 前的 trace 事件在同一个 stream 中, 没有 run 边界
- `forge trace --last-run` 无法实现 (不知道 last run 的边界在哪)
- scorecard 关联 trace 中的 cost 和 model 信息时, 可能混入不同 run 的数据

### 方向建议

引入 **Run Identity & Execution Fingerprint** 系统:

```go
// 新增: internal/run/identity.go
type Identity struct {
    ID              string    // uuid
    Workflow        string    // "build"
    WorkflowDigest  string    // sha256 of the loaded workflow JSON
    GitCommit       string    // HEAD commit at run start
    ModelVersions   map[string]string  // agent → model version mapping
    StartedAtUnix   int64
}
// 新增: internal/run/manifest.go — 每次 run 开始写入 .forge/manifests/<run_id>.json
// 新增: internal/run/replay.go — 验证当前环境是否与 manifest 匹配 (允许 resume)
```

关键集成:
1. **每 run 生成 Identity**: `forge run` / `forge evolve` 开始时生成一个 UUID, 写入 `.forge/manifests/<uuid>.json`
2. **Checkpoint 扩展**: 存储 run_id + workflow_digest, resume 时校验是否匹配; 不匹配则拒绝 (或强制 fresh start)
3. **Trace 事件扩展**: 所有事件附加 `run_id`; 文件按 run_id 分段或加 run 边界标记事件 (`kind: "run_start"` / `kind: "run_end"`)
4. **Memory 事件扩展**: 同样加 `run_id`, 使跨 session 查询可过滤到特定 run

### 产品价值

1. **确定性 Resume**: 环境变化 (工作流被编辑、模型版本升级) 被显式检测 → 拒绝或警告, 避免「resume 后行为变化」的隐式错误
2. **可审计性**: 每个 run 有不可变快照 (workflow + routing + git commit), 可以精确回应「这个 run 用的是哪个版本的工作流」
3. **遥测数据清洗**: trace/memory 按 run_id 分区, `forge trace --run <id>` 精确过滤, 不混入其他 run 的数据
4. **Run 生命周期管理**: `forge run list` / `forge run show <id>` / `forge run gc` — 管理运行历史和存储

### 诚实边界

- v1 只加 run_id 和 workflow_digest, 不做跨 resume 的 blocking 校验 (仅 warning)
- 向后兼容: 旧 checkpoint 无 run_id 时视为「legacy run」, resume 时 warn but proceed
- Identity 生成是零外部依赖: Go 标准库 `crypto/rand` + `crypto/sha256`, 不依赖网络或数据库
- `.forge/manifests/` 目录需要生命周期管理: 建议保留最近 30 个 manifest + 按大小自动清理 (或纳入 `forge doctor` 检查)

---

## 附录: 与已有分析的交叉验证方法

本文 4 个方向的核心理念在以下已有分析文档中进行全文搜索验证:

| 核心理念 | 搜索词 | 命中数 | 结论 |
|---------|--------|--------|------|
| Agent stdout 形式化契约 | `output contract`, `输出契约`, `agent output schema` | 0 | 新颖 |
| 跨阶段结构化数据管线 | `structured feeds_forward`, `结构化数据流`, `phase data pipeline` | 0 | 新颖 |
| 声明产出的落地验证 | `emits verify`, `落地验证`, `claim verification` | 0 | 新颖 |
| 执行指纹与运行一致性 | `execution fingerprint`, `rerun consistency`, `run identity` | 0 | 新颖 |
| 已有分析覆盖的输出契约 | `VERDICT contract`, `parser*reviewer`, `机读裁决` | 15+ | 附属性提及, 非完整方向 |
| 已有分析覆盖的 FeedsForward | `feeds_forward`, `phase output`, `prompt context` | 30+ | 功能描述性提及, 非数据流架构讨论 |

**搜索范围**: `docs/requirements/` (126 篇) + `docs/analysis/` (40 篇) 全文精确与模糊匹配。

---

*本文 4 个方向的共同特征: 它们不是「加功能」或「补接线」, 而是当前代码库中因缺乏某一层抽象而产生的结构性缺口。当前系统状态「能工作」, 但每向多 provider、多 workflow、多团队方向前进一步, 都会撞到这层缺失的抽象。*
