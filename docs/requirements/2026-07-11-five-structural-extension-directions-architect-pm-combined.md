# ForgeOS — 五个结构性扩展方向:架构师 × 产品经理综合视角

> **范围**:全局扫描至 2026-07-11,覆盖 forge-core(18 Go 包 / ~32k LOC)、harness(~10.5k LOC 执法层)、
> `.agent/`(5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS)、examples/、`.github/workflows/`。
> **先验去重**:对 `docs/requirements/` 下 ~80 份 + `docs/analysis/` 下 ~38 份已有分析文档进行全文关键词检索,
> 确保每个方向的核心机制**未被任何已有文档作为独立系统性方向展开**(被提及在侧的注记不算覆盖)。
> **方法**:逐包逐调用链阅读 asset→mode→routing→risk→orchestrator→converge→memory→trace→prompt→persist→gate→doctor→migrate,
> 每个方向附带精确到 `file:line` 的代码级证据、为什么高价值、为什么此前未被充分看见。
> **纪律**:不编写任何代码。
> **角色**:资深架构师(系统完整性) + 产品经理(商业价值)综合视角。

---

## 已有覆盖 vs 本文方向

已有 ~120 份分析文档的高密度覆盖域(本文不重复):

| 覆盖域 | 估算篇数 | 本文处理 |
|---|---|---|
| 编排引擎(串/并行/loop-back/mode-gating/stop-condition) | ~35 | ✅ 跳过 |
| 生产可靠性(529/超时/退避/输出上限/递归守卫/预算护栏) | ~18 | ✅ 跳过 |
| 学习闭环(trace/telemetry/scorecard/converge/Memory/Context) | ~15 | ✅ 跳过 |
| 安全纵深(secret-scan/SCA/risk/进程组/prompt 注入防御) | ~12 | ✅ 跳过 |
| 治理执法(arch-check 8 检查/check.py/drift-guard) | ~10 | ✅ 跳过 |
| 执行语义(原子性/幂等/TOCTOU/因果一致性/rollback) | ~8 | ✅ 跳过 |
| 第三地平线(多仓库/Web UI/事件驱动/管道组合/daemon) | ~8 | ✅ 跳过 |
| 基础能力(CLI DX/配置/forge-init/tutorial/Shell 集成) | ~6 | ✅ 跳过 |
| HTTP API / SDK / 外部集成面 | ~5 | ✅ 跳过 |
| 跨进程运行时锁/runtime 状态守护 | ~3 | ✅ 跳过 |

**本文 5 个方向全部落在上述饱和覆盖域的间隙中**。每个方向的**核心机制**经检索确认未被已有分析文档作为独立系统性方向展开。检索方法:对该方向核心术语组合在 `docs/requirements/` 全部 `.md` 文件中执行精确字符串搜索,命中 0 篇才确认「未被覆盖」。仅被某篇文档在侧栏提及一句但从未深入的方向,算作「未被展开」而非「已覆盖」。

---

## 方向一 · 路由阈值自校准引擎

> **优先级**: 🟠 **P1** | **类别**: 学习闭环 · 模型路由进化 | **关键词验证**: `self.calibrat` `threshold.adjust` `routing.threshold` `threshold.*drift` `auto.calibrat` — **全部零命中**

### 问题描述

路由系统(`internal/routing/routing.go`)的 tier 分界阈值是**硬编码常量**,永不自适应:

```go
// forge-core/internal/routing/routing.go:37-40
const (
    HaikuMax  = 0.34   // total <= 0.34 -> Haiku
    SonnetMax = 0.69   // 0.34 < total <= 0.69 -> Sonnet
                       // total > 0.69 -> Opus
)
```

这些值是人工预设的静态边界。但 scorecard 系统(`scorecard.go`)持续积累每模型 × 每任务类型的 quality/latency/cost 数据。当前的学习闭环(方向二已完成)做的是**同档择优**(`HistoryTiebreak` 在同档候选里挑历史质量最高的),它从未反过来调整**档位边界本身**。

后果:

- **模型能力漂移**:Sonnet-4 发布半年后能力可能超过当年 Opus-3,但阈值不动 → 本该走 Sonnet 的任务被升级到 Opus,浪费钱;或本该走 Opus 的任务被降档到 Sonnet,牺牲质量。
- **项目特性漂移**:一个项目积累 1000 个 ADR 后,`context_size` 维度得分系统性偏高 → 所有任务被抬高一档,但阈值从不偏移补偿。
- **冷启动偏见**:0.34/0.69 是通用预设值,对特定项目(比如纯 CRUD 内部工具 vs 金融核心)的偏差无校准机制。

### 代码证据链

1. **阈值定义**:`internal/routing/routing.go:37-40` — `HaikuMax`、`SonnetMax` 是 `const`。
2. **消费点**:`BandForScore`(routing.go:94-100) 和 `TierForScore`(routing.go:110-140) 直接读这些常量。
3. **Scorecard 数据可用但未回灌**:`Scorecard` 结构体(`scorecard.go:48-60`)携带 `QualityScore`、`PassRate`、`AvgIterations`、`ReworkRate`,路由系统读它只做 `HistoryTiebreak`(`scorecard.go:155-200`),从不做阈值校准。
4. **BudgetAdjustTier**:`routing.go:195-240` 在预算紧张时降档,但阈值的「该不该降」仍由静态边界决定——如果 `SonnetMax` 应该变成 0.75,当前机制永远发现不了。
5. **维度权重也是静态**:`policy.yml` 声明了 6 维权重(complexity=0.25, risk=0.25, ...),但 `Score()` 函数(`routing.go:77-88`)只做加权求和,权重从未被 scorecard 反馈校准过。

### 为什么高价值

这是学习闭环的**水平扩展**:当前闭环学的是"同档选谁",自校准学的是"档位本身是否合理"。后者是前者的超集——档位错了,同档择优毫无意义。对运营成本的影响:ForgeOS 的账单大头是模型 API 费用,阈值偏保守 5% 就可能导致每年多烧数千美元;阈值偏激进则产出质量下降,reviewer 阶段消耗更多迭代来补偿。

### 建议扩展范围

- v1:在 `forge scorecard rebuild` 路径加阈值建议输出(`--calibrate` flag),对比当前阈值 vs scorecard 数据建议值,只报告不动手。
- v2:引入 `CalibratedThresholds` 类型自动调整 `BandForScore` 的分段,绑定 `min_samples` 防噪声,每 N 次 scorecard update 重算一次。
- v3:维度权重自校准——如果 `complexity` 维度对所有任务的区分度很低(几乎所有任务都得 0.5-0.6),降低其权重,提高区分度高的维度权重。

---

## 方向二 · 预测性运行估算引擎

> **优先级**: 🟠 **P1** | **类别**: 成本可观测 · 操作可信度 | **关键词验证**: `predict.*estim` `budget.*projection` `duration.*predict` `cost.*forecast` `wallclock.*predict` — **零命中为独立方向**(仅 3 篇在侧栏提及概念)

### 问题描述

ForgeOS 有**反应性**预算护栏(Agent-Call budget guard、Run-Level budget caps、`BudgetAdjustTier`),但**零预测性**。在开始一个 `forge evolve` 之前,操作者无法回答:

- "这个 run 预期花多少钱?" (total_cost_usd)
- "跑完要多久?" (wall-clock time)
- "预期跑几轮迭代?" (iterations to converge)
- "哪个阶段最烧钱?" (expensive phase)

当前可从 trace/scorecard 历史数据回答这些问题——系统已经收集了每 phase 的 `duration_ms`、`cost_usd_micros`、`model`、`status`——但**没有任何代码消费这些数据做预测**。

### 代码证据链

1. **Trace 数据丰富但只做记录**:`trace.Event`(`trace/trace.go:58-90`)包含 `DurationMs`、`CostUsdMicros`、`Kind`、`Status`、`Model`。每 phase 每迭代都在 JSONL 中有记录。
2. **Scorecard 汇聚了历史均值**:`Scorecard`(`routing/scorecard.go:48-60`)有 `QualityScore`、`PassRate`、`AvgIterations`、`ReworkRate`。这些是天然的预测输入。
3. **Run budget 是纯反应式**:`budget.go`(`orchestrator/budget.go`)在注册命令前检查剩余预算,但无法回答「这个 workflow 历史上平均花多少」。
4. **Phase 维度无成本档案**:`asset.Phase`(`asset/asset.go:40-100`)没有 `ExpectedCostUsd` 或 `ExpectedDurationMs` 字段——每个 phase 的成本是运行时才知道的。

### 边界场景

- **冷启动**:首次运行的仓库没有任何历史数据——预测引擎需要 fallback 到「按 mode × lifecycle 的经验基线」(从 ForgeOS 自身运行数据聚合)。
- **单次异常**:一次因 reviewer API 故障导致重复重试的 run 不应扭曲后续预测——需要离群值剔除或百分位数而非均值。
- **模型升级**:当路由切换到新模型(例如 Sonnet-4→Sonnet-5),旧模型的历史数据对新模型零参考价值——预测器必须感知 `model` 维度,而不是聚合所有模型。
- **mode 切换**:同一个 workflow 在 `explorer` 和 `engineering` 模式下的成本/时长差异可达 10 倍——预测必须按 `(mode, lifecycle, workflow)` 三元组分桶。

### 为什么高价值

1. **预算确定性**:用户可以在 CI 调用 `forge evolve` 之前得到「预计花费 $0.30-0.50,预计耗时 2-4 分钟」,决定是否继续。这是无人值守 24h 的前提——没有预算焦虑就没有无人值守。
2. **异常检测**:当实际成本超过预测的 3 倍时,自动触发告警/暂停——比固定预算上限更敏感。
3. **ROI 计算**:跨 run 追踪「每美元产生的 roadmap 完成度变化」,量化 ForgeOS 自身的投资回报率。

### 建议扩展范围

- v1:`forge run --dry-run` 输出预测报告。纯 CLI 文本,从 `.forge/trace.jsonl` + `.agent/routing/scorecards.json` 聚合历史数据,按 `(model, task_type, mode)` 分桶输出均值/中位数/p90。
- v2:预测注入运行时的 budget guard。`checkRunBudget` 在超过预测值的 2 倍时触发 advisory warning,3 倍时触发 fail-closed。
- v3:自适应预算分配——基于预测结果,在 run 开始时自动设置 `--max-agent-calls` 和 `--run-budget-usd` 的合理默认值,无需用户手动估算。

---

## 方向三 · 静态分析驱动的风险实提取

> **优先级**: 🔴 **P0** | **类别**: 路由安全 · 智能路由 | **关键词验证**: `diff.aware.*risk` `content.based.*risk` `ast.*risk` `static.*analys.*risk` `code.*content.*classif` — **全部零命中**

### 问题描述

当前风险分类器 `internal/risk/risk.go` 的文档**诚实承认**它是一个基于路径启发式的规则分类器,不读代码内容:

```go
// forge-core/internal/risk/risk.go:27-30
// HONESTY — what this is and is NOT (do not oversell it):
//   - This is a RULE-BASED classifier: it maps DECLARED feature flags to a level
//     via fixed, auditable thresholds. It does not learn, infer, or guess.
//   - The features themselves are taken as EXPLICIT INPUT (supplied by the
//     orchestrator / CLU). v1 deliberately does NOT auto-extract them.
```

而 `risk.FromChangedPaths`(`internal/risk/risk_diff.go`)更是只读**文件路径字符串**,完全不解析文件内容:

```go
// forge-core/internal/risk/risk_diff.go:15-20
// FromChangedPaths ... It is a HEURISTIC — it reads only the basename and
// directory prefix of each path, never the file CONTENT.
```

这意味着:

- **假阳性浪费**:`internal/payment/handler.go` 改了行注释 → `TouchesPayment=true` → 路由强制 Opus。一次不必要的 10x 成本。
- **假阴性危险**:`internal/utils/credit_card.go` 新增了 Luhn 校验 → 路径不含 `payment`/`billing` → `TouchesPayment=false` → 路由分配 Haiku → critical 逻辑走便宜模型。
- **多维评分空洞**:`policy.yml` 声明了 6 维评分(complexity/dependency_change/security/risk/context_size/business_impact),但实际只有 `risk` 维度有 `Classify()` 的规则实现,其他维度(`complexity` 需求的 cyclomatic/complexity 信号、`dependency_change` 需求的 lockfile_delta/cross_module_edges)根本没有信号生产函数。

### 代码证据链

1. **FromChangedPaths 是纯路径启发式**:`internal/risk/risk_diff.go:38-100` 整个函数只做 `strings.Contains(filepath.Base(path), "payment")` 类匹配,零文件读取。
2. **Score 函数只有维度名,无信号生产**:`routing.Score()`(routing.go:77-88)的调用者(`cmdRoute` 在 `route.go:200-220`)构造 `dims` map 时,complexity/dependency_change/context_size/business_impact 全部硬编码为 0.5——没有调用过圈复杂度计算、依赖图差分、或文件 scope 计算。
3. **TierForScore 的风险降级是单点**:只有 `risk == "critical"` 触发 Opus 硬下限(`routing.go:115-120`),而 risk 当前只能从路径启发式或手工 `--risk flag` 获得。
4. **Policy.yml 的 scoring.signals 全面未接入**:`policy.yml`(`.agent/routing/policy.yml:20-48`)列出了 `signals: [loc_delta, files_touched, cyclomatic, new_abstractions]` 等——全都没有对应的 Go 代码生产这些信号。

### 边界场景

- **多语言**:AST 分析需要感知语言(Go vs TypeScript vs Python vs Rust),forge-core 目前零语言检测(除 `detect.go` 的浅尝辄止)。
- **生成代码**:`forge evolve` 中 agent 写的代码在被 gate 检查前没有持久化 diff——风险分析必须在 agent 写完但 gate 没跑前做,时间窗口极窄。
- **diff 粒度**:全文件 AST > 行 diff > 路径启发式。全文件 AST 在 CI 场景可行但本地实时场景太慢;需要可配置的深度与速度取舍。
- **被删代码**:删了支付代码的 PR 应该降低风险,但路径启发式看到 `payment/` 仍会上调风险。

### 为什么高价值

这是 G3「自动模型调度」的**最后一环**。声明说路由是多维的(complexity/risk/security/context/business_impact),但实际执行只有「agent 角色 + mode 默认档 + risk 路径启发式」。其他维度是空的。没有真实风险提取,安全下限(`risk=critical → Opus`)就只是一个被保护得很好的空壳——它在等一个永不会来的输入。

### 建议扩展范围

- v1:扩展 `FromChangedPaths` 的轻量级内容嗅探——检测文件是否包含 `import "payment"`,或 `func.*[Cc]redit`——不需完整 AST,仅正则级模式的组合(类似 arch-check 的 JS import 检测模式 `extractJsImports`)。
- v2:按语言接入圈复杂度计算(`gocyclo`/`lizard`/`radon` via harness adapters pipe),让 `complexity` 维度有真实信号。
- v3:跨语言的轻量级 AST 风险分析——检测函数级别 diff(新增的 `func processPayment` 比修改注释的 `func getOrder` 风险高得多),利用已有 harness adapter 框架的语言能力。

---

## 方向四 · 跨运行失效模式分类引擎

> **优先级**: 🟢 **P2** | **类别**: 可观测性 · 运维智能 | **关键词验证**: `trace.*pattern.mining` `cross.run.*classif` `failure.*pattern.*cluster` `trace.*analytics` `anomaly.*detect.*trace` — **零命中为独立方向**(6 篇在侧栏有相关概念)

### 问题描述

`trace.Tracer`(`internal/trace/trace.go`)系统向 `.forge/trace.jsonl` 写入丰富的结构化事件流。每个事件包含 `Kind`、`Status`、`DurationMs`、`CostUsdMicros`、`Model`。但**全仓没有一个子系统消费这些历史 trace 来回答跨运行的问题**:

- "过去一周 ForgeOS 最常见的失效模式是什么?"
- "gate:test 失败和 reviewer:REQUEST_CHANGES 之间有相关性吗?"
- "哪些 workflow 阶段的失败率在上升?"
- "op overload 错误集中在每天的什么时段?"

`doctor` 包(`internal/doctor/`)只检查**当前运行状态**的健康——checkpoint 可读、trace 完整——不做任何跨运行的分析。

### 代码证据链

1. **Trace 数据格式完备但无消费者**:`trace.Event`(`trace/trace.go:58-90`)有 Format/Seq/Kind/Status/DurationMs/CostUsdMicros/Model——所有跨运行分析需要的维度都已存在。
2. **Doctor 只有单快照检查**:`doctor.Run()`(`doctor/doctor.go:50-70`)返回当前 `.forge/` 目录的快照健康,不分析历史趋势。
3. **Scorecard 只看模型质量**:`scorecard` 系统聚合的是「模型 × 任务类型」的性能,不是「运行本身」的健康趋势。`PassRate`/`AvgIterations` 是模型维度的,不是 workflow 维度的。
4. **Conversions 不存在**:没有任何代码把 `trace.jsonl` 转成可聚合的结构供分析。当前观测路径:如果想知道上周的失败趋势,只能用 `jq` 手动处理 JSONL。
5. **Tagging 不存在**:trace 事件没有「run_id」或「session_id」标签(seq 是每个 Tracer 独立的,不是运行间稳定的标识符)。两条不同运行的 trace 行无法可靠关联到各自的运行边界。

### 边界场景

- **Trace 文件旋转**:长时间运行的 `.forge/trace.jsonl` 会无限增长。当前无旋转/归档/截断。跨运行分析需要一个稳定的 trace 归档机制。
- **运行边界识别**:两次 `forge evolve` 的 trace 事件在同一个文件里只用 Tracer 重启的 `seq=1` 区分——没有分隔标记。恢复运行边界需要启发式(超时阈值+N 秒无事件)。
- **多仓库交叉分析**:如果 ForgeOS 管理 10 个仓库,每个仓库的 trace 在各自 `.forge/` 下。跨仓库模式分析需要集中式 trace 收集。
- **隐私与安全**:trace 包含 phase name、gate verdict 等可能有敏感信息的字段。跨运行聚合时必须匿名化或做访问控制。

### 为什么高价值

1. **运维盲区**:ForgeOS 的卖点是「24h 无人值守」。但无人值守不意味着不观察——恰恰相反,无人值守系统**必须**有自动模式识别来替代人类巡逻。当前运维者只能「在失败时收到 exit code」,无法看到「成功率在下降」的趋势。
2. **门控自动调整**:如果趋势分析发现 `gate:test` 在过去一周的 fail 率从 5% 上升到 25%,可以自动触发 `forge evolve` 来诊断测试脆化——这是自治系统的自治Ops。
3. **容量规划**:trace 中的 `CostUsdMicros` 累计是预算规划的核心输入。「上个月 ForgeOS 花了多少钱?花在了哪些 workflow 上?」当前只能算总 exit code 数量,无法回答。

### 建议扩展范围

- v1:`forge trace --summary` 子命令,读取 `.forge/trace.jsonl`,输出按 `(Kind, Status)` 聚合的计数 + 按 `(model, Status)` 聚合的失败率排名。纯本地、零依赖、一次读取全量数据的常量时间。
- v2:引入 `trace rotate` 机制(按文件大小或时间归档)+ `trace archive query` 查询已归档的 trace 历史。归档格式与活跃格式相同(JSONL),只需文件重命名 + gzip。
- v3:可选的远程 trace 后端(PostgreSQL/ClickHouse) + 仪表盘 `/api/v1/trace/trends?since=7d` + 自动告警(当同 phase 的 fail 率 7 日移动均值上升 3σ 时触发 webhook)。

---

## 方向五 · Agent 产出合约验证框架

> **优先级**: 🔴 **P0** | **类别**: 编排可靠性 · 数据完整性 | **关键词验证**: `output.*contract.*valid` `emits.*schema.*enforce` `phase.*output.*schema` `agent.*output.*gate` — **零命中为独立系统性方向**(10 篇提到相关概念但均为单段附注而非独立方向)

### 问题描述

ForgeOS 从 agent 输出中通过**硬编码字符串匹配**提取结构化信息:

```go
// forge-core/cmd/forge/cost.go:330-340
// parseReviewerVerdict does ad-hoc string matching on agent output to find
// "VERDICT: APPROVE" or "VERDICT: REQUEST_CHANGES"
```

同理:

- `parseExecutiveVerdict`(`cost.go:360-380`)匹配 `VERDICT: REDESIGN | DELAY | REJECT | APPROVE_WITH_SIMPLIFICATION`
- `parseConfidenceScore`(`cost.go:390-410`)匹配 `CONFIDENCE: <0-100>`

这些全是**协议脆弱**的:agent 输出中间多了一个空格、大小写不匹配、或者把 `VERDICT` 放在代码块里,解析就静默失败。更根本的问题是:**没有验证 agent 产出的结构性合约**。

每个 workflow phase 的 `emits:` 声明了产出物:

```yaml
# evolve.yml:55-58
- name: gap-analysis
  agent: architect
  emits:
    - gap-report.md
```

但**没有 schema 定义 `gap-report.md` 必须包含什么字段**。一个 planner 的 sprint output 可能包含 `items: [{title, acceptance_criteria, priority}]`,也可能只包含一段散文。下游 phase 两者都能解析吗?完全依赖 agent 的 context 理解,没有结构性保障。

### 代码证据链

1. **Ad-hoc 解析器在高价值的三个路径上**:`cost.go:parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`——全用 `strings.HasSuffix` / `strings.Contains`,每处解析失败都是静默 fallback(返回默认值,agent 的白烧钱产出被丢弃而不报错)。
2. **Emits 声明是纯文档**:`asset.Phase.Emits`(`asset/asset.go:105-115`)被解析和存储,但从未被用来验证 agent 的实际输出。代码仓的输出目录(`docs/discovery/`、`docs/design/`、`docs/review/`)没有任何格式校验器。
3. **VerdictLedger 无类型安全**:`prompt_context.go:200-220` 的 `verdictLedger` 是 `[]string` 切片——所有 veridct 按发现顺序 append,不做 schema 校验。一个 phase 产出了两个 `VERDICT:` 行?取最后一行,前一个静默丢失。
4. **无失败模式**:当 agent 输出格式错误时,`parseReviewerVerdict` 返回空字符串(`cost.go:345-350`),调用者在 `gates.go:250-260` 处检查 `verdict == ""` 后走默认路径——**不报警,不记 trace,不提升给开发者**。系统性 Silent Degradation。
5. **Workflow 定义中的 `on_fail` 不适用于合约违反**:`asset.Phase.OnFail`(`asset/asset.go:65-75`)定义了 gate FAIL 时的跳转,但 agent 产出合约验证失败既不是 gate FAIL 也不是 phase error——它是一个灰色区间:phase 执行成功(exit 0)、agent 输出了内容,但内容不满足合约。目前这种行为被划为"成功"。

### 边界场景

- **合约版本化**:agent 卡升级后 emits 格式变化,旧 phase 的新 agent 版本输出了旧格式——合约需要 `schema_version` 字段。
- **部分合规**:一个 phase 产出了 5 个 sprint item,其中 3 个合法、2 个缺字段——应该拒绝全部还是部分接受?取决于配置的严格度。
- **多语言 agent**:claude 的输出格式可能与 gemini 不同——合约验证需要绑定到 provider/tier,而非全局的单一 schema。
- **输出大小写/空格鲁棒性**:合约验证本身必须容忍合理的格式变化——不是用严格 JSON Schema,而是用宽容的断言(「包含 numeric `priority` 字段」而非「`priority` 必须是整数 1-5」)。

### 为什么高价值

这解决一个正在产生实际影响的漂移问题。Sprint 27 的 `VERDICT:` 解析测试已经发现:如果 agent 在 markdown 代码块里输出 verdict、或 verdict 前多了一个空格,全线解析静默失败。目前靠 reviewer 自己调试和测试修补。但真正的解决方案不是打补丁——是**将合约验证建设为第一类检查**,而不是附着在 `cost.go` 的副作用解析上。

### 建议扩展范围

- v1:为每个 `emits:` 产出物定义轻量级合约(在 agent 卡或 workflow 定义中用 `schema:` 字段引用一个简洁的 JSON/YAML 断言文件)。`forge validate --contracts` 子命令检查合约是否存在、是否可解析;运行时嵌入 prompt 让 agent 知道其产出将被验证。
- v2:在 phase 执行完成后、进入下一 phase 前,插入一个可选的 `contract_check` gate——运行针对 `emits:` 产物的合约验证器,验证失败则记录 trace 事件 + 触发 `on_fail` loop-back(如同 gate FAIL)。不阻断 run 但显著延缓收敛,迫使 agent 修复产出格式。
- v3:合约学习——当某个 phase 的 agent 连续 N 次产出不合格合约格式时,自动在 prompt 中插加强调格式的 instruction(system-prompt hardening),无需人工修改 agent 卡。

---

## 优先级与发展路线

| 方向 | 优先级 | 类别 | 一句话杠杆 |
|---|---|---|---|
| **三** 静态分析风险提取 | **P0** | 安全 · 路由正确性 | 风险下限 Opus 目前是空壳——没有真实风险提取,安全下限等一个永远不来的输入 |
| **五** Agent 产出合约验证 | **P0** | 数据完整性 · 编排可靠性 | ad-hoc 字符串解析现实已暴露出静默降级的真 bug(Sprint 27);合约验证是根治 |
| **一** 路由阈值自校准 | P1 | 学习闭环 · 模型路由进化 | scorecard 数据已存在,阈值自校准是学习闭环缺失的最后一维度——不仅选谁,也调边界 |
| **二** 预测运行估算 | P1 | 成本可观测 · 操作可信度 | 历史 trace 数据已丰富,预测是「反应性预算」到「主动性成本管理」的跃迁 |
| **四** 跨运行失效分类 | P2 | 运维智能 · 可观测性 | trace 数据写入但从不消费;实现 V1(`forge trace --summary`)成本极低、杠杆清晰 |

### 做前三件(全 P0+P1)

三 → 五 → 一(或二):先把风险提取从路径启发式升级为内容感知(三),同时把 agent 产出合约验证建成一阶检查(五),再把学习闭环从「选谁」扩展到「边界在哪」(一)。这三个方向各自独立,可并行起步;五的 v1 合约定义加成本最低(加 schema 字段 + 轻量级校验器),可让新 agent 卡从创建第一天就携带合约。

方向二(预测估算)和四(失效分类)是「数据已有,消费未写」的纯收益方向,可在上述三个方向的间隙插入——v1 实现都只需 ~1 sprint 的工作量(`forge trace --summary` 和 `forge run --dry-run --predict`)。
