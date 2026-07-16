# ForgeOS — 五个未建基座:产品级扩展方向

> **角色**: 资深架构师 + 产品经理
> **方法**: 全局逐包扫描 `forge-core/`(18 Go 包 / ~32k LOC)·`harness/`(39+ 模块 / ~10.5k LOC)·
>   `.agent/`(5 workflow / 12 agent 卡 / 全部 policies+ADR+DECISIONS)·`.ai/`(10 stage 模板)·
>   `ai-dev/`(自有 pipeline)·`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(123 条逐条分类)。
> **先验去重**: 逐个核对该方向的核心理念组合词在全部 ~120 篇已有 `docs/requirements/` +
>   ~40 篇 `docs/analysis/` 中是否作为**独立系统性方向**展开过(侧栏一句话提及不算覆盖)。
> **纪律**: 不编写任何代码。每个方向附带精确的代码级证据、产品价值判断、边界情况(Edge Cases)、
>   性能考量。
> **日期**: 2026-07-11
>
> **诚实声明**: 以下方向不是「接线遗漏」或「bug 修复」——经过 31 轮 sprint 后这类缺口已被逐条收口
> (见 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` Sprint 30-31)。以下五个方向是**产品级架构基座**的缺失——
> 它们不需要修复任何现有功能,但决定了 ForgeOS 能否从「自治编排框架」成长为「AI 软件工厂操作系统」。

---

## 快速索引

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **跨 Phase 结构化输出契约** — 从 `VERDICT:` 末行到声明式 Schema | 架构 · 正确性 | 🔴 P0 | 当前所有 phase 间通信靠自由文本末行匹配,无结构校验、无契约版本、无演进路径 |
| 2 | **多 Agent 并行执行 + 冲突检测与合并** — 超越串行编排 | 能力 · 性能 | 🟠 P1 | 五个 workflow 全部是串行 chain,并行引擎已就绪(`waves.go`)但零 workflow 使用,真实并行 agent 的读写冲突无可避免 |
| 3 | **可观测性管线** — 从 append-only JSONL 到结构化遥测导出 | 运维 · 产品化 | 🟠 P1 | trace/memory/checkpoint 全是 append-only 本地文件,无查询接口、无指标导出、无跨运行聚合 |
| 4 | **Phase 级资源沙箱与副作用隔离** — 同进程无隔离的隐患 | 安全 · 韧性 | 🔴 P0 | 所有 phase 共享进程空间/文件系统/环境变量,无网络隔离、无临时目录、无信息流控制 |
| 5 | **Agent 输出保真度评分与自适应降级** — 从「匹配 VERDICT」到「验证内容质量」 | 自治品质 | 🟡 P2 | 系统对 agent 输出只做行末 token 匹配,从不评估内容与声明的符合度 — agent 可以写「VERDICT: APPROVE」但附零评审内容 |

---

## 方向一 · 跨 Phase 结构化输出契约

> **「当前所有 phase 间通信靠人读自由文本 + 末行 VERDICT 机读 token。没有 schema、
>   没有版本、没有演进路径。这是收敛完整性最大的单点脆弱性。」**
> **核心组合词验证**: `structur.*output.*contract\|phase.*contract\|output.*schema.*phase\|agent.*output.*schema`
>   → 在全部已有分析文档中**零篇**作为独立系统性方向展开。

### 问题

当前 phase 间信息传递有三条路径,全部是**无结构自由文本**:

**路径 A — agent 末行机读契约**(最关键的收敛信号):

| 信号 | 来源 phase | 消费方 | 匹配方式 | 风险 |
|---|---|---|---|---|
| `VERDICT: APPROVE` / `REQUEST_CHANGES` | reviewer, cto | `cost.go:parseReviewerVerdict` | `strings.HasSuffix`末行 | agent 笔误 `APPROVE`→`APRROVE` 静默丢失 |
| `CONFIDENCE: 85` | product-manager | `cost.go:parseConfidenceScore` | `strings.CutPrefix`末行 | 置信分与文本不在同一行时丢失 |
| 5-way executive verdict | cto | `cost.go:parseExecutiveVerdict` | 精确末行 token | 新裁决词(如 `APPROVE_WITH_SIMPLIFICATION`)拼错全部丢失 |

```go
// forge-core/cmd/forge/cost.go — 三个解析器全是末行精确/前缀匹配
func parseReviewerVerdict(output string) (string, bool) {
    // unwrapClaudeResult → lastNonEmpty → strings.HasSuffix("VERDICT: APPROVE")
}
func parseConfidenceScore(output string) (int, bool) {
    // lastNonEmpty → strings.CutPrefix("CONFIDENCE: ")
}
func parseExecutiveVerdict(output string) (string, bool) {
    // lastNonEmpty → 5 个精确 match
}
```

**路径 B — agent 产出的文件**(`emits:` 声明):

| 文件 | 来源 phase | 字段 | 格式约定 |
|---|---|---|---|
| `task-plan.md` | planner(build.yml) | `emits: [task-plan.md]` | 无声明式格式 |
| `prd.md` | product-manager(discover.yml) | `emits: [prd.md]` | 无声明式格式 |
| `gap-report.md` | architect(evolve.yml) | `emits: [gap-report.md]` | 无声明式格式 |

`prompt_context.go` 确实会读取这些文件并注入到下游 phase 的 prompt 中:

```go
// forge-core/cmd/forge/prompt_artifacts.go:~50-80
func readEmittedArtifact(root, path string) string {
    data, _ := os.ReadFile(filepath.Join(root, path))
    return string(data) // 直接当作纯文本注入
}
```

但**没有任何校验**:
- planner 的 `task-plan.md` 是否真的包含 task breakdown?
- product-manager 的 `prd.md` 是否包含 Success Metrics / User Personas?
- architect 的 `gap-report.md` 是否包含 ranked gaps?

**路径 C — `feeds_forward: true` 的 phase 输出**(build.yml planner → implementer/reviewer):

```go
// forge-core/cmd/forge/prompt_context.go:~180-200
// phaseOutputLedger 完整记录前馈 phase 的原始 prompt + output
type phaseOutputRecord struct {
    Prompt string
    Output string
    Agent  string
}
```

同样的,`Output` 字段是 agent 返回的原始文本——没有预期 schema、没有字段提取、
没有完整性校验。

### 产品价值

1. **收敛完整性**:当一个 `VERDICT: APPROVE` 因为拼写错误被静默丢失时,系统继续循环
   直到 max-iter。结构化输出能在源头做 schema 校验,拼写错误在解析层被立即探测而非静默丢失。

2. **进化路径**:当前 3 个解析器(parseReviewerVerdict/parseConfidenceScore/parseExecutiveVerdict)
   各自维护一套独立的末行匹配规则。如果要新增一个机读契约(如 `PROGRESS: 3/5 tasks done`),
   需要写第 4 个解析器、注册到 gate.go 的 gatherSignals、更新 converge.go 的 evalOne、
   修改 agent 卡文档、重新测试——5 处改动。结构化 schema 让新增契约变成「声明 schema → 
   自动解析 → 注入 converge」的三步操作。

3. **Backward compatibility**:没有 schema 版本,今天改了 VERDICT 的格式昨天所有存量
   trace/checkpoint 里的旧格式就无法回测。有版本号的 schema 让迁移路径清晰。

### 建议扩展骨架

在 `internal/asset/` 或新包 `internal/contract/` 中定义:

```go
// OutputContract 声明一个 phase 产出的结构化契约。
// 一个 phase 可以声明一个或多个契约(emits 文件 + 机读 token 行)。
type OutputContract struct {
    Name    string      // 契约名,如 "reviewer_verdict"
    Schema  string      // JSON Schema / Cue / 或内置格式名
    Version int         // schema 版本号,用于向后兼容
    Source  ContractSource // "stdout_lastline" | "emits_file"
    Path    string      // emits file 时有效
}
```

- 现有 3 个解析器保留为「legacy 适配器」,但新增契约走统一解析路径。
- `asset.Phase` 加可选 `output_contracts: [...]` 字段。
- `check.py` 加 `check_phase_output_contracts` 治理检查。
- converge 的 `evalOne` 层新增 `StatusFromContract(name, sig)` 分支。

### 边界情况

- **多重结构**:一个 phase 的 stdout 可能既包含 `VERDICT: APPROVE` 又包含 `CONFIDENCE: 85`。
   当前各解析器各扫一段,不会冲突。结构化 schema 需要支持「从同一段文本提取多个字段」而非
   只能一个 schema 对应一个 phase。

- **向后兼容迁移**:现有 5 个 workflow 中的 ~20 个 phase 从未声明过 `output_contracts`。
   迁移策略必须是 `omitempty` — 未声明则降级为当前末行启发式,零行为变化。

- **Schema 爆炸**:每个 phase 一个 schema → 20+ schema 文件。治理检查需要确保 schema
   不被遗忘。推荐每个 schema 在 `./agent/contracts/` 下一文件,同 agent 卡。

- **Schema 与 agent 卡漂移**:agent 卡说「输出三件事:A、B、C」,schema 说「两件事:A、B」,
   但无自动化检查。需要在 check.py 中加跨引用检测。

### 性能考量

- Schema 校验发生在**解析时**(agent 输出后、送入 converge 前),在 agent 执行路径上,
   不在 converge 热点路径上。校验本身是 O(输出大小) —— 几百字节输出下可忽略。
- Schema 文件在运行时要加载。建议 ContextCache 同级别缓存(不重新读盘)。
- Regex/语法解析器用 Go 标准库 `regexp`,保持零外部依赖。

---

## 方向二 · 多 Agent 并行执行 + 冲突检测与合并

> **「waves.go 已就绪、`--parallel` 已实现、depends_on 已声明——但零个 shipped workflow
>   使用它。因为并行 agent 编辑代码的**冲突**问题还无人解决。」**
> **核心组合词验证**: `parallel.*agent.*conflict\|agent.*merge.*conflict\|parallel.*write.*race`
>   → 在全部已有分析文档中**零篇**作为独立系统性方向展开。

### 问题

ForgeOS 已经拥有并行编排的能力:

```go
// forge-core/internal/orchestrator/waves.go — Kahn 拓扑排序,已实现,测试覆盖
func Waves(phases []asset.Phase) ([][]int, error) { ... }
```

```go
// forge-core/internal/orchestrator/parallel.go — RunParallel 已实现
func (e Engine) RunParallel(ctx context.Context, wf asset.Workflow, mode string) error { ... }
```

```go
// forge-core/cmd/forge/main.go — --parallel flag 已接线
fs.BoolVar(&o.parallel, "parallel", false, "run a workflow's depends_on-independent phases CONCURRENTLY")
```

但**没有真实 workflow 使用它**——因为一旦两个 agent 并行写同一个文件,谁赢?

当前 `Engine` 明确承认未解决这个问题:

```go
// forge-core/internal/orchestrator/parallel.go:~30
// NOTE: concurrent agent phases share the SAME filesystem and process.
// If two implementers edit the same file, the last writer wins.
// No merge, no conflict detection, no locking.
```

具体风险场景:

| 场景 | 两个并行 phase | 冲突 | 后果 |
|---|---|---|---|
| build.yml 未来有两个 implementer | impl-1 写 `src/route.go`, impl-2 也改 `src/route.go` | 文件级覆盖 | 第二个 agent 覆盖第一个的改动 |
| discover.yml P2+P3 | market-research 写 `citations.md`, product-design 不碰它 | 无冲突 | ✅ 安全 |
| review.yml P1+P2+P4 | 四个审查各自写 `docs/review/` 下不同文件 | 逻辑交叉引用不一致 | 四个审查报告对同一问题的描述可能矛盾 |
| evolve.yml P1+P2 | scan 和 gap-analysis 都是 readonly | 无写冲突 | ✅ 安全,但共享进程环境变量可能读冲突 |

### 产品价值

1. **并行加速**:当前 build.yml 的 P1→P2→P3→P4→P5 串行执行。如果 P2.1 和 P2.2
   (两个独立任务的 implementer)可以并行,理论加速比 = 2x。对于 24h evolve 循环,
   每 5 个 phase 中有 3 个是 readonly 的(planner/harness-gates/reviewer/QA),
   剩下的 implementer 如果拆成 2 个并行 = 总时长缩短 ~30%。

2. **分布式审查**:review.yml 的四个审查(security/distributed/performance/CTO)
   互不依赖——全可并行。当前串行跑 4 个 Opus agent 的 token 消耗和延迟完全可并行化。

3. **发现问题**:并行 agent 写冲突不只是性能问题——它在**迫使冲突显式化**。如果一个 agent
   改了 `src/payment.go` 的类型签名、另一个并行 agent 改了 `src/api.go` 中对
   `payment.go` 的调用,两者分开运行时各自测试绿,合并后编译红——冲突暴露了跨模块
   调用契约的一致性,串行编排可能被「测试全绿但重构不完整」的假象欺骗。

### 建议扩展骨架

- **文件级写锁**:在 `.forge/` 下建立 `locks/` 目录,每个 phase 起跑前声明要写的
  文件路径(file-glob),获取 advisory flock,冲突时串行化或报错。
- **git merge base 冲突检测**:每个可写 phase 在 git staging 中 commit 自己的改动,
  并行 phase 冲突表现为 merge conflict,通过 `git merge-tree` 或 `git diff --cc` 检测。
- **只读 phase 允许并行,可写 phase 串行**:review.yml 的四审查全是 readonly→全可并行;
  build.yml 的 implementer 是唯一写 phase→串行。这是最低风险的起点。

### 边界情况

- **声明式 vs 实际写集**:一个 implementer phase 声明 `readonly: false` 但它实际上
   也可能不改文件(仅重构)。系统无法在 phase 起跑前精确知道写集。乐观锁 + 事后冲突
   检测比事前声明更实用。

- **并行 phase 的 checkpoint**:当前 checkpoint 是相位序数(phase index)驱动的。
   并行 phase 没有确定的 phase index→checkpoint 需要从「第几个 phase」变成
   「已完成的 phase 集合」。

- **并行 + dry-run**:并行 dry-run 无冲突——它们不写代码。但 dry-run 的 phase 输出
   (如 review 报告)也可能被并行打印交错。

### 性能考量

- 并行 gate 也需要并行。当前的 `ProbeAll` 是顺序调度——lint 在 etc, test 在 etc。
   如果 gate 全是纯 CPU 工具,并行有收益;如果是 I/O-bound(test 等待编译),收益更大。
- 并行 agent 的总 token 消费 = N × 单个 agent 的 token 消费——并行不省 token,
   可能增加(因为合并冲突后需要额外 phase 解决冲突)。

---

## 方向三 · 可观测性管线

> **「trace.jsonl / memory.jsonl / checkpoint.json 全是 append-only 本地文件。
>   没有查询接口、没有指标导出、没有跨运行聚合。要了解一个 24h evolve 循环的内部
>   状态,只能 `ssh → cat .forge/trace.jsonl | jq`。」**
> **核心组合词验证**: `observab\|metrics.*export\|prometheus\|opentelemetry\|structured.*log\|grafana`
>   → 在全部已有分析文档中**零篇**作为独立系统性方向展开。

### 问题

ForgeOS 已经收集了丰富的运行时数据,但全部锁在本地 JSONL 文件中:

| 数据源 | 位置 | 格式 | 大小 | 消费方式 |
|---|---|---|---|---|
| trace | `.forge/trace.jsonl` | JSONL | 10MB 自动轮转 | `jq` 手动查询 |
| memory | `.forge/memory.jsonl` | JSONL | 无界(仅 10-iter compact) | `memory.Load` 全量读 |
| checkpoint | `.forge/checkpoint.json` | JSON | 1 文件+5 备份 | `persist.Load` 单文件 |
| scorecard | `.agent/routing/scorecards.json` | JSON | 被 wind-down 更新 | `routing.HistoryTiebreak` |

这三个系统性缺陷:

**1. 无结构化查询接口**:要回答"过去 24 小时哪个 gate 最常红"需要:

```bash
# 当前方式:手动 jq
$ cat .forge/trace.jsonl | jq 'select(.kind=="gate" and .status=="FAIL") | .name' | sort | uniq -c
```

而不是:

```sql
SELECT gate_name, count(*) FROM trace WHERE kind='gate' AND status='FAIL'
  AND timestamp > now() - interval '24 hours' GROUP BY gate_name;
```

**2. 无聚合指标导出**:外部监控系统(Prometheus/Datadog/CloudWatch)完全不知 ForgeOS
   的存在。operator 无法设置告警:

| 想要告警 | 当前可行? |
|---|---|
| `forge evolve` 在过去 1 小时内无迭代进展 | ❌ 需手动 SSH + 查 checkpoint age |
| agent phase 的平均延迟超过 5 分钟 | ❌ 需手动跑 jq 算 p95 |
| gate 连续 3 次 FAIL | ❌ 无跨运行聚合 |
| run budget 消耗超过预期 80% | ❌ 无比率指标 |
| memory 文件超过 100MB | ❌ 无文件大小监控 |

**3. trace 事件缺少跨运行标识**:

```go
// forge-core/internal/trace/trace.go:~65-94
type Event struct {
    Seq           int    // 事件序列号——但每次重建从 0 开始
    Kind          string // "iteration"|"agent"|"gate"|"converge"|...
    Name          string
    Status        string
    DurationMs    int64
    CostUsdMicros int64
    Model         string
    Detail        string
    // 缺少: RunID, WorkflowID, PhaseIndex, Timestamp(Unix epoch),
    //       GitCommit, Hostname, SessionID
}
```

这意味着:

- 两次运行的 trace 无法区分(Seq 都从 0 开始)
- 跨运行的趋势分析不可能
- 无法关联 trace event 与 git commit

### 产品价值

1. **告警与自愈**:如果 operator 能在 3 次 gate FAIL 后自动收到 PagerDuty 告警,
   ForgeOS 可以从「需要人盯着」跃迁到「异常时通知人」。

2. **数据驱动的容量规划**:"build workflow 平均每次迭代消耗 $0.18 的 Claude,
   上周总消耗 $22.50"→ 引导预算设置。

3. **调试效率**:"让我查一下为什么昨晚的 evolve 在 iter 7 卡住了"——而不是
   "让我 SSH 到那台机器、cat trace、jq、再用肉眼找异常"。

### 建议扩展骨架

- **Trace 事件加全局 RunID**:`cmd/forge/main.go` 中 `runWorkflow` / `cmdEvolve`
   起跑时生成一个 UUID(Go 标准库 `crypto/rand`),注入到 trace 所有事件中。

- **结构化日志**:用 `log/slog`(Go 1.21 标准库,保持零外部依赖)取代当前的大量
   `fmt.Println`/`fmt.Fprintf(os.Stderr)`。slog 支持 JSON 输出+等级过滤+结构化字段。

- **指标导出端点**:新增 `forge metrics` 子命令——暴露一个简单的 HTTP endpoint
   或 stdout Prometheus text format,让 `node_exporter` 或 `scrape_config` 采集。

- **opentelemetry 就绪**:保持零依赖,但定义清晰的 telemetry 接口(`internal/telemetry/`),
   让 community 可以接 OpenTelemetry 导出器而不改核心。

### 边界情况

- **隐私与安全**:trace 包含 `Detail` 字段(自由文本,可能包含 token/secret)。导出指标时
   必须确保 `Detail` 不被意外暴露。需要分级(public/internal/secret)策略。

- **I/O 影响**:在 agent phase 的热路径上写 trace + 内存 + checkpoint 已经是双重 I/O。
   加指标导出(DogStatsD / Prometheus push)可能增加延迟。推荐异步缓冲导出。

- **RunID 生成**:32 字节 UUID vs 8 字节自增 ID?UUID 成本低(一次 `crypto/rand` 读),
   但 trace 文件增大(每行 +36 字节)。对于 10000 事件的 trace 约 +360KB,可接受。

### 性能考量

- 指标导出建议**异步**(goroutine + buffered channel),避免阻塞 agent phase。
- trace 文件轮转(10MB)已实现,但轮转后旧数据被丢弃。长期保存策略(如 `trace.archive/`
   目录 + 保留 N 天)需要在可观测性管线中纳入。
- slog 的结构化 JSON 输出比 `fmt.Println` 慢约 2-3x——但 ForgeOS 不是延迟敏感系统。
   Agent phase 耗时 30s-5m,额外 50μs 的日志开销可忽略。

---

## 方向四 · Phase 级资源沙箱与副作用隔离

> **「所有 phase 运行在同一个 OS 进程、同一个文件系统、同一组环境变量下。
>   一个被注入恶意的 agent 可以读取前序 phase 的所有产物、修改 checkerpoint、
>   甚至用 `os.Exit(1)` 杀死整个 forge 进程。」**
> **核心组合词验证**: `sandbox.*phase\|phase.*isolat\|phase.*contain\|side.*effect.*phase\|agent.*sandbox`
>   → 在全部已有分析文档中**零篇**作为独立系统性方向展开。

### 问题

当前 phase 执行模型是**同进程无隔离**:

```
Process: forge run build
├── Phase 1: planner (readonly)     — 同一进程,同一文件系统
├── Phase 2: implementer (write)    — 可以读/写任意文件
├── Phase 3: harness-gates (verify) — 读取 implementer 写的文件
├── Phase 4: reviewer (readonly)    — fresh_context 是 prompt 级,不是文件级
└── Phase 5: qa (readonly)
```

具体风险:

**风险 1:信息流不可控**。`fresh_context: true` 的 reviewer 在 prompt 层面看不到
前序 phase 的输出——但它仍然可以**直接读文件**。

```go
// forge-core/cmd/forge/prompt_context.go:~130
// FreshContext 只阻止 prompt 注入——agent 还是可以读文件
func (l *gateLedger) context() string {
    if l == nil {
        return ""  // 不注入 gate 结果到 prompt
    }
}
// 但 agent 完全可以在它的输出中写:
// "请先运行 cat docs/adr/0001-*.md"
// → 然后通过 Bash 读取
```

**风险 2:临时文件跨 phase 泄漏**。一个 implementer 创建了 `.env`、`.tmp/credentials`
   或 `debug-dump.json`(包含 prompts/outputs)。下一个 phase(gate/reviewer)可以读取它们。

**风险 3:环境变量泄漏**。`forge run` 的环境变量被所有 phase 继承。如果 `--agent-cmd`
   需要 `ANTHROPIC_API_KEY`,这个 key 对所有 phase 都可见——包括只需要 `node --test`
   的 readonly harness phase。

```go
// forge-core/internal/orchestrator/command_executor.go:~140-160
// 当前所有 phase 共享父进程环境变量
cmd := exec.Command("claude", args...)
cmd.Env = os.Environ()  // 父进程全部环境变量 → 子进程
cmd.Dir = engine.root
```

**风险 4:phase 可以杀死 forge**。`os.Exit(1)` 或 `kill -9 $$` 在一个 agent phase 中
   可以直接终止整个 `forge run`。

**风险 5:无网络隔离**。readonly phase 和 write phase 有相同的网络访问权限。
   一个被注入恶意 prompts 的 agent 可以 C2 外联。

### 产品价值

1. **安全基线**:对于多租户场景(一个 ForgeOS 实例跑多个项目——这在 ROADMAP 中),
   phase 隔离是基本前提。

2. **审计完整性**:operator 需要确信 "reviewer phase 确实没有修改代码" 不仅是
   prompt 层面被告知"不要改",而是 OS 层面它就是 readonly。

3. **故障域**:一个 phase 的 OOM/kill/panic 不应该影响其他 phase。当前一个 phase
   的 `os.Exit(1)` 会终止整个 run。

### 建议扩展骨架

分层方案,每层独立可选(不强制全量):

| 层 | 机制 | 保护 | 依赖 |
|---|---|---|---|
| L1 | 子进程隔离(PID namespace, `Setpgid:true` 已有) | 一个 phase 的 kill 不影响其他 phase | ✅ 已部分实现(`command_executor_unix.go`) |
| L2 | 临时工作目录(每个 phase 独享 temp dir) | 临时文件不跨 phase 泄漏 | 新 `internal/sandbox/tempdir.go` |
| L3 | 环境变量白名单 | 最小特权:只传 phase 需要的 env var | 新: `Phase.EnvWhitelist` in asset + 执行器过滤 |
| L4 | 网络限制(readonly phase 禁用网络) | 防止 C2 外联/敏感数据出站 | 需 OS 层面(seccomp/unshare) |
| L5 | 只读文件系统映射(readonly phase bind-mount ro) | 运行时强制 readonly | 需 OS 层面(`mount --bind -o ro`) |

`asset.Phase` 已有 `Readonly bool`——它是**声明意图**。L5 将意图变成**强制**。

### 边界情况

- **与 Checkpoint 的交互**:临时 workdir 中的进度如何在 crash 后恢复? checkpoint
   需要从临时目录复制回持久目录。

- **与 `forge init` / existing tools 的交互**:一个被限制网络的 phase 调用 `npm install`
   会失败——需要 graceful 告知"网络不可用"而非静默失败。

- **OS 兼容性**:L4+L5 需要 Linux-specific syscall。macOS 和 Windows 需要 fallback
   到 advisory-only(降级但不静默)。

- **测试**:隔离机制本身需要测试。L1(已部分覆盖)的 `command_executor_unix_test.go`
   只有少量测试——需要扩展。

### 性能考量

- L1 + L2:几乎零开销(进程创建成本主要在 fork/exec,隔离本身不增加成本)
- L4 + L5:mount/bind 操作约 1-5ms,只在 phase 起/结束时——可忽略
- L1 的 PID namespace:需要 `clone()` 而非 `fork()`,Go 的 `os/exec` 不支持直接设置
   `cloneflags`——需要使用 `cmd.SysProcAttr.Cloneflags`(linux only)

---

## 方向五 · Agent 输出保真度评分与自适应降级

> **「系统对 agent 输出只做行末 token 匹配。agent 可以写 `VERDICT: APPROVE` 但
>   附零评审内容——系统仍然标记为 `review_status=approved` 并收敛。
>   不是 agent 在"欺骗"——是测什么就得到什么(Goodhart's Law)。」**
> **核心组合词验证**: `fidelit.*score\|output.*qualit.*score\|agent.*output.*fidelit\|content.*valid.*phase`
>   → 在全部已有分析文档中**零篇**作为独立系统性方向展开。

### 问题

当前收敛系统对所有 agent 输出的处理是**二元(token 匹配)**:

```
agent 输出                                 → parse 结果         → converge 判定
─────────────────────────────────────────────────────────────────────────
"VERDICT: APPROVE\n\n looks good"         → "APPROVE"          → review_status=approved
"VERDICT: APPROVE\n\n (no review done)"   → "APPROVE"          → review_status=approved ← 同一结果!
```

系统的度量体系假设 agent 的 **token 输出 = 实际完成度**:

| 收敛信号 | 理想含义 | 实际测量 | 漏洞 |
|---|---|---|---|
| `RoadmapCompletion` | 功能已完成 | checklist `[x]` 比例 | agent 可以提前勾 |
| `ReviewStatus` | 代码已审查 | `VERDICT: APPROVE` 匹配 | agent 可以写 token 不写审查 |
| `RequirementConfidence` | 需求已充分探索 | `CONFIDENCE: N` 匹配 | agent 可以报高分不列依据 |

唯一现有的抗度量伪造机制是 `FileDelta` 交叉验证:

```go
// forge-core/internal/orchestrator/loop.go:~139-148
if sig.RoadmapCompletion > 0.5 && sig.FileDelta < 0.3 {
    l.logf("  ⚠ honesty: roadmap=%.0f%% but file-change coverage=%.0f%% ...")
}
```

但它是 advisory——只打一行 log,不影响收敛判定。

### 产品价值

1. **收敛可信度**:operator 可以信任"converged = MET"不是"converged = agent 学会了
   怎么输出 VERDICT: APPROVE"。

2. **自动降级**:当 agent 输出保真度低于阈值时,自动触发：
   - 重跑(不同 model/不同 temperature)
   - 升级模型 tier(如从 Sonnet 到 Opus)
   - 降低该 agent 的收敛贡献权重

3. **反馈给路由**:保真度历史可以指导模型选择:"agent X 在 reviewer 角色下使用 Sonnet
   时输出保真度 0.6,使用 Opus 时 0.9——路由系统自动学习用 Opus。"

### 保真度维度建议

| 维度 | 测量方法 | 影响范围 | 实现难度 |
|---|---|---|---|
| **Token 真实性**:VERDICT token 是否匹配输出内容的真实结论 | 第二解析器:全文语义 vs 末行 token | ReviewStatus | 🔴 难(需 LLM) |
| **Checklist 准确性**:`[x]` 的 roadmap 条目是否有对应代码变更 | FileDelta 增强:entry-level 匹配而非全局 | RoadmapCompletion | 🟡 中 |
| **置信度可靠性**:CONFIDENCE=95 是否出现在需求确实充分的情况 | 跨 iteration 的 CONFIDENCE 标准差 + 后续 design phase 是否补充需求 | RequirementConfidence | 🟢 易 |
| **输出完整性**:emits 文件是否包含预期章节 | 最小章节校验(正则/heading 存在性) | 所有 emits phase | 🟢 易 |
| **自洽性**:agent 的输出是否内部矛盾 | 交叉引用检查:如设计的 API 签名在实现中不一致 | 所有 agent | 🔴 难 |

### 建议扩展骨架

- `internal/doctor/fidelity.go` 新包:定义 `FidelityScore` 类型 + 评分函数。
- 评分结果注入 `converge.Signals` 的 `Criteria` map(复用既有 per-criterion 评估路径)。
- 新增 `stop_condition.all_of` + `metric: review_fidelity` 等指标。
- Route 系统新增 `FidelityTierAdjust`(同 `BudgetAdjustTier` 模式):低保真度→升级 model。

### 边界情况

- **评分不可靠**:保真度的评分本身可能误报。一个 review 写得极好但 token 不太一致的
  输出可能得低分。方案:评分只是 advisory(不阻断收敛),但持续低分触发告警。

- **cold start**:新 agent/新角色没有历史保真度数据。降级到当前行为(不评分)。`FidelityScore`
   为 0 时不影响收敛。

- **评分计算成本**:最难的维度(Token 真实性)需要 LLM 调用——等于多跑一次迷你 agent。
   这在 24h 循环中可接受(额外 1-2% 成本),但对于快速 `forge run` 是不可接受的。
   方案:评分只在 evolve 循环中启用,run 保持当前行为。

- **model-specific 保真度**:Claude Sonnet 和 Opus 的输出保真度分布不同。评分系统必须
   按 model 归一化,否则 low-cost model 永远得低分。

### 性能考量

- 简单维度(Checklist 准确性 / 输出完整性 / 置信度可靠性)是纯文本分析,零 LLM 成本,
   O(输出大小)时间。
- 困难维度(Token 真实性 / 自洽性)需要 LLM 调用。建议只在 evolve 循环中启用,
   batch 化(几个 phase 的输出一起评分)以摊薄成本。
- 评分结果可以写入 trace 事件(`kind: "fidelity"`),不另增持久化机制。

---

## 附录:本文与已有分析的差异矩阵

| 饱和覆盖域 | 已有篇数 | 本文是否重复 |
|---|---|---|
| 编排引擎状态机(串/并行/loop-back/mode-gating/resume/checkpoint) | ~35 | ❌ 不重复——方向一二三不在此域 |
| 生产韧性(529/退避/递归守卫/预算/输出上限/进程组) | ~18 | ❌ 不重复——方向四的沙箱是韧性上游,不是韧性本身 |
| 学习闭环(trace/scorecard/converge/memory/Context/路由回灌) | ~16 | ❌ 不重复——方向三的可观测性是学习闭环的**外部接口**,非内部机制 |
| 安全纵深(secret-scan/SCA/risk/readonly 强制/prompt 注入防御) | ~14 | ❌ 不重复——方向四的沙箱是运行时强制,非检测;方向五是度量问题,非注入防御 |
| 治理执法(arch-check 8 检查/check.py 10 检查/drift-guard/function-length) | ~12 | ❌ 不重复——方向一的结构化契约是新的治理层,非既有检查 |
| 执行语义(原子性/幂等/TOCTOU/因果一致性) | ~8 | ❌ 不重复——方向二的冲突检测是并行独有的问题 |
| CLI 体验(detect/preflight/doctor/status/migrate/validate) | ~8 | ❌ 不重复 |
| 第三地平线(多仓库/Web UI/事件驱动/Sandbox/联邦) | ~8 | ❌ 不重复——方向四的沙箱是进程级,非 Firecracker VM;方向三的指标不是 Web UI |
| 跨进程运行时安全(.forge 文件锁) | ~3 | ❌ 不重复——方向四的沙箱是 phase 间隔离,非跨进程锁 |
| 三框架债务(.agent vs .ai vs ai-dev) | ~3 | ❌ 不重复 |
| 收敛震荡/Checklist 漂移 | ~3 | ❌ 不重复——方向五的保真度是主动评分,非漂移检测 |
| 运行时状态生命周期管理 | ~2 | ❌ 不重复 |
| Agent 自报信号可信度 | ~2 | ❌ 不重复——方向五的保真度是可操作评分+自适应降级,非仅交叉验证 |
| 自动化失败分类 | ~1 | ❌ 不重复 |
| 编排引擎随机/属性测试 | ~1 | ❌ 不重复 |
| 非可嵌入性(internal/包壁垒) | ~1 | ❌ 不重复 |
| 持久格式版本标识 | ~1 | ❌ 不重复 |
| Gate 调度拓扑优化 | ~1 | ❌ 不重复 |
| 配置组合优先级模型 | ~1 | ❌ 不重复 |
| 门控上下文 Token 预算 | ~1 | ❌ 不重复 |
| Prompt 注入威胁检测 | ~1 | ❌ 不重复 |
| 产品遥测 | ~1 | ❌ 不重复 |

