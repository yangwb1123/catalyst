# ForgeOS — 五个高价值架构扩展方向（代码级全局扫描）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 forge-core（18 Go 包, ~35k LOC）· harness（~39 模块, ~10.5k LOC）·  
>   `.agent/` 全部 5 workflow × 12 agent 卡 × 9 skill 卡 × ADR + DECISIONS ·  
>   全部 31 轮 sprint 记录 · `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 全部 200+ 条目。  
> **去重验证**: 已审阅 `docs/requirements/` 全部 134 份 + `docs/analysis/` 全部已有分析,  
>   对每个方向的核心理念进行关键词组合搜索,确认零篇系统性论述。  
> **纪律**: 不编写任何代码。每个方向附带精确到 `file:line` 的代码证据与架构/产品价值判断。  
> **日期**: 2026-07-11

---

## 当前架构阶段

经过 31 轮 sprint,ForgeOS 的**运行时引擎层**已经高度成熟:

| 能力层 | 现状 |
|--------|------|
| 编排引擎 | 串行/并行/loop-back/resume/checkpoint/mode-gating 全能力 |
| 安全护栏 | 递归深度·执行次数·墙钟超时·输出上限·进程组隔离·四维资源护栏 |
| 学习闭环 | trace→scorecard→memory→converge 全链路就绪 |
| 治理执法 | 8 项架构检查 + 秘钥扫描 + SCA 框架 + check.py 10 项治理检查 |
| 真点火验证 | multi-agent 端到端通过,含 8 个真 bug 修复 + 三维真数据落盘 |

但 134 份扩展分析绝大多数聚焦在**已有引擎的增量功能或已知领域的深化**。以下 5 个方向全部落在已有分析的间隙中——它们不是在某个成熟子系统中增加一个 knob,而是触及了**当前架构中完全不存在或仅处于萌芽概念层**的系统级缺口。

---

## 方向一 · 跨进程 `.forge/` 运行时目录锁

> **类型**: 数据安全 · 并发安全 · **优先级**: 🔴 **P1 (critical)**  
> **关键词去重验证**: `file.lock 0` · `flock 0` · `cross.process 0` · `.forge.*lock 0` · `race.*forge 0` ——  
>   134 份需求文档中 **零篇** 提到跨进程文件锁。

### 问题

ForgeOS 的 `.forge/` 目录存储全部运行时持久化状态: `checkpoint.json`(覆盖式写入)、`trace.jsonl`(追加写入)、`memory.jsonl`(追加写入)、scorecards(覆盖式写入)。**但这些文件全部没有进程级别的写入锁保护。**

当前代码中:

```go
// forge-core/internal/persist/checkpoint.go:104-124 — Save: 写临时文件 → rename
// 写入步骤之间没有锁,两个进程同时 Save 不会有竞争检测。
func Save(path string, cp Checkpoint, retain int) error {
    // ...
    tmp := path + ".tmp"               // 两个进程会写不同的临时文件
    if err := writeSynced(tmp, data); err != nil { ... }
    if err := os.Rename(tmp, path); err != nil { ... }
    // ❌ 没有锁: 进程 A rename 后,进程 B 的 rename 覆盖它
    // ❌ 但如果有 retain>0,两个进程的 rotateRetain 同时执行会混乱
}
```

| 文件 | 写入方式 | 问题 |
|------|---------|------|
| `checkpoint.json` | 覆盖式(rename) | 两进程先后 rename,后写的覆盖前写,数据不丢但上下文丢失 |
| `trace.jsonl` | O_APPEND 追加 | 两进程同时追加 → 内核级 O_APPEND 保证不交叠,但**事件交错**:seq 编号冲突、解读错误的迭代绑定 |
| `memory.jsonl` | O_APPEND 追加 | 同上:事件交错,且 `loadCaches` 是 in-process sync.Map,进程间互相擦除缓存 |
| `scorecards.json` | 覆盖式 | 两进程轮番覆盖,分数丢失 |

**更严重的问题——`rotateRetain` 中的竞态**:

```go
// forge-core/internal/persist/checkpoint.go:127-139
func rotateRetain(path string, retain int) {
    // ❌ 完全没有锁: 两个进程同时执行 os.Rename,部分备份文件可能丢失或混乱
    for i := retain - 1; i >= 1; i-- {
        os.Rename(older, newer)  // 竞态:两进程同时 Rename 同一个源文件
    }
    os.Rename(path, path+".1")   // 竞态:同时 Rename checkpoint.json → checkpoint.json.1
}
```

**而 memory 包的 in-process 缓存(`sync.Map`)恰好证明了有并发意识,但只覆盖同一进程内的 goroutine**:

```go
// forge-core/internal/memory/memory.go:54-55
var loadCaches sync.Map // key=path(string), value=*loadCacheEntry
// 注释说 "concurrent forge processes on different projects do not
// invalidate each other's cache entries"——但这只解决了不同项目路径的碰撞,
// 对同一项目同一 .forge/ 的两个进程完全没有保护。
```

### 为什么需要

1. **CI + 开发者并发运行**: CI runner 和开发者同时在同一个仓库上跑 `forge evolve`。  
   场景:开发者 `forge evolve build --mode engineering` (跑 15 分钟),CI 同时 `forge run gate --root .` (快速准入)。  
   结果:CI 的 gate 读到了开发者 evolve 中间状态的 checkpoint → 误判 or 读到损坏的半写文件。

2. **`forge accept` 和 `forge evolve` 的 `acceptance-kernel.mjs` 探测使用共享 probe 状态**——两个进程的 probe 结果会互相污染,但当前无机制防止。

3. **Honesty 原则**:如果一个 `forge doctor` 读取 checkpoint 时发现 `_format` 字段不能匹配预期(未来版本升级),目前无法区分"文件损坏"和"另一个进程正在写入"。

### 边界情况

- **死锁恢复**:如果持有锁的进程崩溃,锁必须自动释放(推荐 `flock(2)` 而非手工 pid 文件,因为 `flock` 在 fd 关闭时自动释放,不产生僵尸锁)。
- **只读操作豁免**:`forge doctor`、`forge status`、`forge validate` 等只读命令应该不加锁或加共享锁(flock LOCK_SH),以免读操作阻塞写操作。
- **锁定粒度**:全 `.forge/` 目录一个锁即可——该目录的操作为什么要跨进程并行?关键 paths:checkpoint.json + trace.jsonl + memory.jsonl + scorecards.json。

### 落地路径

```
Go 标准库: syscall.Flock(int(fd), syscall.LOCK_EX) 在 .forge/ 目录上
加锁点: memory.Append · persist.Save · memory.Prune · scorecard 写路径
只读豁免: doctor/status/validate 走 LOCK_SH
```

### 为什么不紧急（但很重要）

这不是一个日常会触发的 bug——并行运行两个 `forge evolve` 需要用户主动做一件不寻常的事。但一旦触发,数据损坏是静默的（没有错误信息、没有崩溃）,用户只会注意到跑了一个 `forge doctor` 发现 trace 文件乱了,或 checkpoint 回退到了上个迭代。这种**静默损坏**与 ForgeOS 的 `honesty` 原则直接冲突。

---

## 方向二 · Memory 知识生命周期:Confidence 字段生而不育

> **类型**: 能力缺失 · 功能断裂 · **优先级**: 🟠 **P2 (high)**  
> **关键词去重验证**: `confidence.*read.*never 0` · `knowledge.*decay 0` ·  
>   `memory.*aging 0` · `confidence.*weight 0` —— 134 份文档中 **零篇** 系统性论述 Confidence 字段全线失联。

### 问题

`memory.Entry` 有一个声明完整的 `Confidence` 字段:

```go
// forge-core/internal/memory/memory.go:133-137
// Confidence is an OPTIONAL caller-supplied signal (default 1.0) that the
// knowledge-entry source can set to annotate how trustworthy the entry is.
// A low-confidence entry (e.g. < 0.3) should be treated as speculation or
// unverified — the prompt layer prefixes it with "[unverified]" to cue the
// agent to independently verify rather than take it as settled truth.
```

**但这个字段在整个代码库中从未被消费。** 全仓搜索 `\.Confidence` 在其他文件中的引用:

| 文件 | 行 | 做了什么 |
|------|-----|---------|
| `memory.go:167` | 定义 | 定义字段 |
| `memory.go:344` | 解码 | 把零值默认为 1.0 (旧文件向前兼容) |
| **(全仓)** | — | **没有任何 Query 过滤/排序/加权用到 Confidence** |
| **(全仓)** | — | **没有任何 prompt 构造根据 Confidence 前缀标记** |

证据链:

```go
// forge-core/internal/memory/memory.go:351-367 — Query 函数
func Query(entries []Entry, kind, topic string) []Entry {
    out := make([]Entry, 0, len(entries))
    for _, e := range entries {
        if kind != "" && e.Kind != kind { continue }
        if topic != "" && e.Topic != topic { continue }
        out = append(out, e)
        // ❌ 没有 e.Confidence < 0.3 过滤
        // ❌ 没有按 Confidence 排序
    }
    return out
}
```

prompt 注入端的消费:

```go
// forge-core/cmd/forge/prompt_memory.go — memoryContext
// memoryContext 调用 memory.Query,然后直接把条目注入 prompt:
// "以下是从 memory 中检索到的历史知识:"
// ❌ 没有按 Confidence 加 "[unverified]" 前缀标记
// ❌ 没有按 Confidence 加权排序
// ❌ 没有过滤低置信度条目
```

同样,`internal/prompt/retrieve.go` 的 BM25 检索器也是**纯关键词匹配**,完全不考虑 Confidence:

```go
// forge-core/internal/prompt/retrieve.go — 全部逻辑基于 TF-IDF 分数
// TF-IDF 分数学语义相关性,但之后没有乘以 confidence 权重
// 一个 Confidence=0.1 的条目和 Confidence=1.0 的条目,只要关键词相关度相同就同等权注入
```

### 为什么需要

1. **"永恒记忆"问题**:memory 是 append-only 日志。第 1 次迭代写入了一个低质量的 gap finding(Agent 调研不够,confidence=0.2),第 50 次迭代仍然会被检索到并注入 prompt。没有置信度衰减,Agent 每次都要和早期的垃圾信息打交道。

2. **Learning loop 的诚实性**:memory 目前的 `filterSuperseded` 机制用 `Supersedes` 字段做显式替代——但这是"新条目手动声明替代旧条目"。如果 Agent 忘了声明替代,旧错误永远不被清除。Confidence 本应提供一条**隐式衰减路径**:条目越老、越少被确认,信心越低——直到低于阈值自动过滤。

3. **产品意义**:用户会说"为什么我的 Agent 一直重复犯同一个错误?"——答案往往是 memory 中一个低置信度的早期错误决定每次都等同权重地喂给 Agent。

### 边界情况

- **降级不删除**:低置信度条目不应该被删除(append-only 日志的完整性),只应该在 Query 中被降权或跳过。
- **衰减曲线**:需要一种策略——基于 age 的指数衰减(同 scorecard 的 `decayWeight`),或基于未被 superseded 的时间,或基于 iter 间距。
- **提示层前缀**:`memoryContext` 对低置信度条目加 `[unverified]` 前缀是最低成本的落地方式(纯字符串操作,不改变调度语义)。

### 落地路径

```
Phase 1 (低改动): prompt_memory.go 在注入前加 Confidence < threshold 过滤 + [unverified] 前缀
Phase 2 (中改动): memory.Query 增加 WithMinConfidence(threshold) 选项
Phase 3 (中改动): memory 增加 age-based decay,写入时自动更新 Confidence
```

---

## 方向三 · 工作流 YAML Schema 版本化与向后兼容契约

> **类型**: 运维债务 · 声明-实现漂移 · **优先级**: 🟡 **P3 (medium)**  
> **关键词去重验证**: `workflow.*schema.*version 0` · `yaml.*version 0` ·  
>   `yaml.*migration 0` · `schema.*compat 0` —— 134 份文档中 **零篇** 触及 Workflow YAML schema 版本化。

### 问题

`.agent/workflows/*.yml` 是 ForgeOS 最核心的声明式资产——它定义了编排的完整骨架。经过 31 轮 sprint,workflow schema 已经膨胀到 20+ 个字段:

```
Phase 结构(asset.go)现有字段:
  Name · Agent · RequiredGates · RequiredWhen · OnFail · OnUnmet · RunMode ·
  ModelTier · ConfidenceMetric · RequiresTools · Readonly · UsesTemplate ·
  SecondaryTemplate · Emits · StopCondition · HumanApproval · Stop · Stage ·
  Adr · DependsOn · OnApproved · OnRejected
```

但 `asset.Workflow` **没有 SchemaVersion 或 FormatVersion 字段**:

```go
// forge-core/internal/asset/asset.go
type Workflow struct {
    Name   string  `json:"name"`
    Phases []Phase `json:"phases"`
    Mode   string  `json:"mode"`
    Lifecycle string `json:"lifecycle"`
    Stop   StopCondition `json:"stop"`
    Stage  string  `json:"stage"`
    // ❌ 没有 SchemaVersion string `json:"_schema_version,omitempty"`
}
```

**后果**:一个由 forge-init v1(2026-06)生成的 workflow YAML,被 v1.2(2026-07)的 forge-core 消费时:

- `requires_tools` 字段被**静默丢弃**(因为 asset.Phase 在 v1.1 才新增该字段,v1 YAML 没有)。
- `secondary_template` 字段被**静默忽略**(v1.2 新字段,v1 YAML 没有)。
- 反过来,一个 v1.2 的 workflow YAML(包含 `secondary_template`),被 v1.0 的 forge-core 加载——`json.Unmarshal` 只是忽略未知字段,不会报错。Agent 缺少了本该有的第二阶段提示模板,但**没有任何可见的失败信号**。

当前 YAML→JSON 转码路径也没有版本校验:

```go
// forge-core/cmd/forge/main.go:131-149 — loadWorkflow
// Go yaml2json.Decode + json.Unmarshal → 未知字段被静默忽略(Unmarshal 默认行为)
// 没有任何 "这个 workflow 文件需要 >= v1.2" 的校验
// 也没有任何 "已知字段清单" 来检测拼写错误
```

### 为什么需要

1. **forge-init 的全局化契约**:forge-init 生成的 scaffold 项目包含自己的 `.agent/workflows/` 文件。这些文件必须与用户本地的 forge-core 二进制兼容。如果用户在 6 月用 forge-init 创建了项目,7 月升级了 forge-core,旧 workflow 文件会产生静默不同的行为——没有错误,只是 Agent 表现不同。

2. **横向扩展(独立 agent-os 仓库)**:ADR-0003 设计了 submodule 机制来全局共享治理资产。如果一个远程仓库的 workflow 文件和本地 forge-core 的字段理解不一致,目前没有任何检测手段。

3. **CI 中的黄金文件校验**:`forge validate --workflow build.yml` 应该能检测未知字段、过期字段、与二进制版本不兼容的声明——但在 schema 版本化完成前无法做。

### 边界情况

- **版本宽松匹配**:semver 的 MAJOR.MINOR——MAJOR 不兼容(字段类型变更/删除)时拒绝,MINOR 兼容(新增字段,有默认值)时允许。
- **向后兼容自动填充**:`json.Unmarshal` 已经用零值填充缺失字段——只要新增字段有合理的零值语义(如 `Readonly=false`),旧 workflow 文件的功能不受影响。版本化的价值在于**显式告知用户**(而非静默)差异的存在。

### 落地路径

```
Phase 1: asset.go Workflow + Phase 加 _schema_version 字段(现在开始写,只提供给未来)
Phase 2: loadWorkflow 加版本校验:workflow 文件声明的 min_version ≤ forge-core 的 schema_version
Phase 3: force-init 在生成文件中写入当前 schema_version
```

---

## 方向四 · Agent 决策可观测性:Chain-of-Thought 捕获

> **类型**: 可观测性缺口 · 调试能力 · **优先级**: 🟠 **P2 (high)**  
> **关键词去重验证**: `chain.*thought 0` · `decision.*rationale 0` · `reasoning.*capture 0` ·  
>   `agent.*why 0` · `why.*decision 0` —— 134 份文档中 **零篇** 触及 Agent 推理路径的结构化捕获。

### 问题

ForgeOS 的 trace 系统记录了**事件的结果**,不记录事件背后的**推理过程**:

```go
// forge-core/internal/trace/trace.go:61-78 — Event 结构
type Event struct {
    Format     string `json:"_format,omitempty"`
    Seq        int    `json:"seq"`
    Kind       string `json:"kind"`        // "iteration" | "agent" | "gate" | "decision" | ...
    Name       string `json:"name"`        // 阶段名/门名
    Status     string `json:"status"`      // "PASS" | "FAIL" | "APPROVE" | ...
    DurationMs int64  `json:"duration_ms"` // 耗时
    CostUsdMicros int64 `json:"cost_usd_micros,omitempty"`  // 成本
    Model      string `json:"model,omitempty"`              // 模型
    Detail     string `json:"detail,omitempty"`             // 自由文本
}
```

记录的内容:
- ✅ `iteration 5: duration=30s, cost=0.18, model=sonnet`
- ✅ `gate lint: PASS`
- ✅ `decision: Downtier to haiku (spend_ratio=0.85)`

**不记录的内容**:
- ❌ `planner 决定从 architecture 开始而非 testing: 理由=..."` 
- ❌ `reviewer APPROVE 的理由=..."`  
- ❌ `implementer 选择了方案 A 而非方案 B: 理由=..."`

**代码级证据:没有一条路径能从 Agent 输出中提取结构化决策理由:**

```go
// forge-core/cmd/forge/cost.go:371-400 — parseConfidenceScore / parseReviewerVerdict / parseExecutiveVerdict
// 这些函数从 Agent 输出中提取的只是最后一行的机读 token:
//   VERDICT: APPROVE           → 只提取 "APPROVE"
//   CONFIDENCE: 85            → 只提取 "85"
// ❌ 没有任何东西提取 Agent 给出这个裁决的"理由"
```

Agent 的**完整输出**被丢弃了:

```go
// forge-core/cmd/forge/cost.go — unwrapClaudeResult
// 提取 token 后,Agent 输出的详细推理内容被丢弃
// trace 中不会出现 "approved because: test coverage is sufficient,
//   architecture follows ADR-0001, no security concerns identified"
```

同样,`buildPrompt` 的 7 条注入车道也没有被存证:

```go
// forge-core/cmd/forge/prompt_context.go — buildPrompt
// 构造完 prompt 后直接返回 string,没有:
// ❌ prompt 全文存证到 trace
// ❌ 每条注入车道的摘要记录
// ❌ prompt hash/fingerprint 供后续对比
```

### 为什么需要

1. **调试 Agent 行为退化**:用户说"上周这个 workflow 跑得很好,这周 reviewer 一直 REQUEST_CHANGES"。当前 trace 只能告诉你"reviewer REQUEST_CHANGES 了 5 次",但无法告诉你"因为 Agent 认为测试覆盖率不足 80%,而只达到了 65%"。没有推理路径,退化原因全靠猜。

2. **Agent 审计**:一个 review workflow 批准了一个包含安全漏洞的 PR。trace 记录是 `review_status=approved`。没有任何记录解释 reviewer 为什么没发现漏洞——是 prompt 中漏掉了安全约束?还是 Agent 自己忽略了?没有 Chain-of-Thought 记录,无法追溯。

3. **质量回溯**:Sprint 24-26 的 8 个真实 gap 修复中,多个 gap 的发现依赖于审查 Agent 的**输出内容**(而非输出 token)。如果 trace 只记录 token 不记录内容,这些问题的根因分析不可能做。

4. **产品意义:Agent 开发的黑箱问题**——ForgeOS 作为"让 AI 自治运行"的平台,不能连自己都看不清 Agent 在做什么。

### 边界情况

- **内容长度**:Agent 的原始输出可能很长(几千 token)。`Detail` 字段应该截断+存摘要(类似 `phaseOutputSummaryCap=800` 的做法),而非完整保存。
- **隐私**:prompt 内容可能包含业务敏感信息。需要灰度策略:默认只存推理摘要,不存原文;通过 `--trace-level full` 开启全文记录。
- **token 提取成本**:当前 unwrapClaudeResult 从 `--output-format json` 提取,json 中已有 `text` 字段。提取成本 = 零额外 API 调用——只需要在丢弃前多存一行。

### 落地路径

```
Phase 1: trace.Event 增加 Reasoning string `json:"reasoning,omitempty"`
Phase 2: cost.go parseReviewerVerdict 等函数额外返回 reasoning行(VERDICT: APPROVE 前面的自然语言解释)
Phase 3: prompt_context.go buildPrompt 返回 prompt 摘要 + 车道列表,存入 trace
Phase 4: agentExecutor 在 Agent 阶段完成后,将推理摘要存入 trace.Event
```

---

## 方向五 · 阶段级预算隔离:防止单阶段饿死整次运行

> **类型**: 资源管理缺口 · 编排韧性 · **优先级**: 🟠 **P2 (high)**  
> **关键词去重验证**: `phase.*budget.*isolation 0` · `budget.*reservation 0` ·  
>   `starvation.*phase 0` · `per.*phase.*allocation 0` ——  
>   134 份文档中 **零篇** 从"阶段间预算饿死"角度系统地分析资源隔离。

### 问题

ForgeOS 现在有三层预算控制,全部是**运行级汇总,不分阶段**:

| 预算层 | 机制 | 作用域 |
|--------|------|--------|
| `--max-agent-calls` | checkAgentBudget | **全运行总计**:所有阶段的总 spawn 次数 |
| `--run-budget-usd` | BudgetExhausted | **全运行总计**:所有阶段的累积美元成本 |
| `--agent-max-budget-usd` | claude --max-budget-usd | **单次调用**:一次 agent 调用的美元上限 |

**缺失的一层:阶段级预算预留(reservation)。**

```go
// forge-core/internal/orchestrator/orchestrator.go:152-162 — runAgentPhaseBudgeted
func (e Engine) runAgentPhaseBudgeted(ctx context.Context, p asset.Phase, mode string, calls *int) error {
    if err := e.checkAgentBudget(calls);  err != nil { return err } // ❌ 全运行看门狗,无阶段分配
    if err := e.checkRunBudget(*calls-1); err != nil { return err } // ❌ 同上
    return e.runAgentPhase(ctx, p, mode)
}
```

**真实场景**:

一个 `forge evolve build` 有 5 个 agent 阶段:
```
planner(Opus, ~$0.50) → implementer(Sonnet, ~$3.00) → 
  harness(gate, $0) → reviewer(Opus, ~$0.50) → qa(Sonnet, ~$1.00)
```

假设 `--run-budget-usd=3.00`:  
- planner 用 $0.50  
- implementer 第一次尝试 $3.00 → budget exhausted($3.50 > $3.00)  
- **reviewer 和 qa 永远跑不了**。  
- 运行以 `gate/agent failure` 结束。  
- 但 `forge accept` 读到的痕迹是:planner 完成了,implementer 失败了——**没有人 reviewer planner 的输出,也没有 qa 验证 implementer 的代码**。

当前防御——BudgetAdjustTier 和 near-budget down-tier——只延迟了耗尽时间,不能保证**关键阶段的完成**:

```go
// forge-core/internal/routing/routing.go:314-328 — BudgetAdjustTier
// near-budget 阶段降档可以延长运行时间,但:
// 1. 降档降低质量(sonnet→haiku)
// 2. 不断档:仍然无法保证 reviewer/qa 的一定被执行
// 3. 降档在 0.80-1.00 区间触发,但如果 implementer 1 次调用就烧到 1.00,降档已经来不及了
```

### 为什么需要

1. **生产者-消费者不对称**:planner 和 implementer 是"生产者"阶段(写代码/文档,消耗大量 token),reviewer 和 qa 是"消费者"阶段(读代码/输出断言,消耗少但重要)。没有阶段隔离,生产者可以耗尽预算,让消费者永远无法核实其工作。

2. **`forge evolve` 的收敛保证**:evolve 循环的目的是收敛到 ROADMAP 100% + gates green。如果 reviewer 或 qa 因为预算耗尽而无法执行,收敛被审计记录证明为"不可能",而不是"正在进展中"。

3. **产品信任**:用户设置 `--run-budget-usd=10.00`,期望的是"完整的 5 阶段 build 流程能跑完"。当前行为是"最先跑的阶段可以吃掉所有预算"——这与用户的预算心理模型不一致(用户认为预算是一次完整运行的"票价",不是第一阶段的奖金池)。

### 边界情况

- **预留 vs 封顶**:阶段预算预留是**保留的额度**(如 "reviewer 至少保留 $0.50"),不是严格的上限(planner 如果 $0.30 完成,省下的给 implementer)。
- **阶段分类**:agent 阶段需要预留,gate 阶段不需要(零成本)。
- **动态预留**:预留额可以根据阶段重要性调整。reviewer(Opus,安全关键)的预留应该高于 implementer(Sonnet,常规任务)。
- **预留耗尽**:如果预留被释放(前序阶段实际消费 < 预留),剩余预算可以给后面阶段用。

### 落地路径

```
Phase 1: runOpts 增加 --phase-budget-reserve="planner=0.50,implementer=1.00,reviewer=0.50,qa=0.50"
Phase 2: runBudget 增加 per-phase reservation map
Phase 3: checkRunBudget 检查:本次阶段消费 ≤ 总预算 - sum(其他阶段预留)
Phase 4: 预留释放:阶段完成后,未用预留 > 0 则释放到共享池
```

---

## 方向优先级汇总

| 优先级 | 方向 | 影响范围 | 触发条件 | 建议阶段 |
|--------|------|---------|---------|---------|
| 🔴 **P1** | **方向一 · 跨进程 .forge/ 锁** | 数据安全 · 并发安全 | 任何双进程同时操作 .forge/ | 下一个 sprint |
| 🟠 **P2** | **方向四 · Chain-of-Thought 捕获** | 可观测性 · 调试能力 | 持续:每次 agent 环节都在丢失推理信息 | 下个 sprint 起 |
| 🟠 **P2** | **方向五 · 阶段级预算隔离** | 资源管理 · 收敛保证 | 任何有预算上限的 `forge evolve` | 下个 sprint |
| 🟠 **P2** | **方向二 · Memory Confidence 激活** | 学习闭环 · 知识质量 | 任何跑超过 5 次迭代的 evolve | Sprint +1 |
| 🟡 **P3** | **方向三 · Workflow Schema 版本化** | 运维债务 · 跨版本兼容 | forge-init 全局化 / 独立 agent-os 仓 | Submodule 迁移前 |

---

## 与现有架构的关系

```
                           forge-core 运行时引擎
                    ┌──────────────────────────────┐
                    │  Orchestrator · Mode · Gate   │
                    │  Trace · Memory · Converge    │
                    │  Risk · Routing · Persist     │
                    └──────────┬───────────────────┘
                               │
           ┌───────────────────┼───────────────────┐
           ▼                   ▼                   ▼
    方向一·跨进程锁      方向二·Confidence     方向四·CoT 捕获
    (所有IO操作加锁)     (Query/prompt注入)    (trace+parse拓展)
           │                   │                   │
           └───────────────────┼───────────────────┘
                               ▼
                   方向五·阶段预算隔离
                   (cost.go + orchestrator)
                               │
                               ▼
                   方向三·Schema 版本化
                   (asset.go + loadWorkflow)
```

五个方向的依赖关系:
- **方向一和方向二独立**,可并行推进
- **方向四**需要 trace Event 结构扩展,不阻塞其他方向
- **方向五**依赖 cost.go 的 runBudget 结构但不需要大幅度改造
- **方向三**是前置条件最少的独立资产,但回报周期最长

---

*分析日期:2026-07-11 | 基于 forge-core ea454c0 全量全局扫描*
