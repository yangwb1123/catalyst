Now I have a thorough understanding of the project context. Let me provide the architectural analysis.

---

# 架构分析：「五方向高价值扩展」深度审查

## 1. 架构评估

### 当前架构的优势

**a) 纯洁的技术基础**
`forge-core` 纯 Go 标准库、零外部依赖（`go.mod` 无 `require`），这是极少数项目在 v2 阶段能维持的纪律。好处是可量化的：
- 无供应链漏洞面（零 transitive dependency）
- 二进制体积可控，`go build` 即产出
- CI/CD 无版本锁——永远不遇 `go mod tidy` 与上游冲突

这是架构护城河的第一层，也是「载重墙」哲学的体现——如果执法层自身依赖一堆第三方包，谁执法？

**b) 声明即实现的收敛模型**
`mode × lifecycle` 中枢旋钮同时驱动 Router 档位、Harness 严格度、Workflow 深度——这是少数真正贯彻「策略即数据」的 Go 工程实践。不是 OPA/Rego，但策略在 YAML 中声明并在 Go 结构体中反射的设计，比硬编码 `if mode == "engineering"` 分支树更可审计。

**c) 「诚实」作为一等架构属性**
代码注释中频繁出现 `HONESTY` 标记、`N/A` 不是 FAIL 的规避路径、fresh-context reviewer 独立于实现者——这些都是架构的非功能性属性通过工程纪律落地，而非仅写在 README 里。

**d) 逐步交付的技术债管理**
Sprint 22-31 的系统性模式：审计 → 暴露 gap → 分类（真 gap / deferred-by-design / blocked-external）→ 收口或诚实标注。这比大多数项目「知道有债但从未系统梳理」好一个数量级。

### 局限性（架构债务与技术债）

**a) Python shim 作为 YAML 解析层**
`harness/yaml2json.py` 是架构中最大的一块临时脚手架。它尚未被替换为 Go 原生实现。虽然对当前运行无碍（Python 3 在几乎所有开发环境中可用），但这是零依赖哲学的一个缺口——forge-core 的零依赖是绝对的，但运行时依赖外部 Python 解释器来解析 workflow 定义。

**严重度**：中。不是功能性阻塞，但每个新环境多一个故障点（Python 缺失、版本不兼容、pip 依赖冲突）。

**b) CLI 包的文件数预算反复突破**
`cmd/forge` 的文件数上限从 14 → 16 → 17，每次都是「拆到极限再微量上调」。这表明 CLI 层面缺少更粗粒度的模块化——`internal/` 包的拆法（doctor、attribution、mode、migrate）是正确方向，但 CLI 胶水层的增长表明 `cmd/forge` 承担了太多「指挥者」职责。

**严重度**：低（目前可控，但需持续监控）。每次 PR 后检查 `cmd/forge` 文件数是可接受的治理开销。

**c) 多维路由的「空洞维度」**
`policy.yml` 声明了 6 个评分维度（complexity、risk、dependency_change、security、context_size、business_impact），但实际只有 `risk` 维度有信号生产函数。其他维度硬编码为 0.5。这是架构中最值得警惕的技术债——**声明的架构与实际的架构之间存在显着差距**，且当前没有任何 drift-guard 检测到这一点（check.py 检查引用一致性但不检查信号生产是否存在）。

**严重度**：高。声明 6 维但执行 1 维的差距会在系统复杂度上升时侵蚀信任——当新开发者读 `policy.yml` 以为路由是全维度评分的，实际执行只会让他困惑。

**d) 带外执法层 vs 编辑器内加速器的分层职责未正式化**
Architecture.md 提到了「真相之源 = 带外执法层，CC hook = 加速器适配器」，但代码中没有正式的接口定义这两者的职责边界。`gate.mjs` 与 `arch-check.mjs` 的部分检查重叠（8 检查中的 function-length 和 circular 由 arch-check 执法，但 policies.yml 的原始声明在 gate.mjs 消费）。目前靠人不犯错来维持不重叠。

**严重度**：低-中。当前规模可控（8 检查），但扩展时可能产生 drift。

### 关键设计决策评估

| 决策 | 合理性 | 备注 |
|------|--------|------|
| D6: v2 启动 forge-core | ✅ 正确时机 | ADR-0001 的取代条件（dogfood 闭环验证稳定）满足后才启动，非过早镀金 |
| 零外部依赖 | ✅ 正确 | 与「载重墙」哲学一致；但 Python shim 是例外，需要明确决议 |
| Trace/Scorecard 分离 | ✅ 正确 | trace 是事件流，scorecard 是聚合视图，分离符合 CQRS 直觉 |
| `asset.Phase` 无补偿原语 | ⚠️ 当前合理但需追踪 | 支持补偿需要状态机复杂度，当前编排模型没到那个阶段 |
| fresh-context reviewer 作为强制纪律 | ✅ 卓越 | 这是架构决策中最被低估的一项——大多数项目在「代码审查」上忽略 context 污染问题 |

---

## 2. 扩展方向

基于上面分析的架构现状和所述五个方向的审查结果，我给出以下 5 个扩展方向的架构评估——**不以审查文档本身的方向为蓝本，而是从架构完整性出发独立推导**。

### 方向 A：路由阈值的自适应校准（P1）

**为什么需要**
路由系统是 ForgeOS 的经济引擎——它决定了哪个任务走哪个模型，直接控制成本/质量比。当前硬编码阈值（HaikuMax=0.34, SonnetMax=0.69）的架构含义是：**路由机制不信任自身的历史数据**。Scorecard 系统积累了大量质量/延迟/成本数据，但只用于同档择优，从不反向校准档位边界。

**核心挑战**
1. **冷启动**：新项目零历史数据 → 需要按 `(mode, lifecycle, workflow)` 分桶的基线
2. **非平稳分布**：模型升级（Sonnet-4→Sonnet-5）使历史数据失效 → 需要检测分布漂移并重置桶
3. **反馈延迟**：质量信号（reviewer APPROVE/REQUEST_CHANGES）的到达比成本信号（trace 即时落盘）滞后整个迭代 → 校准需要对齐时间窗口
4. **校准震荡**：如果每次 scorecard 更新都调阈值，系统可能振荡 → 需要阻尼（如指数移动平均 + `min_samples` 守卫）

**预期的架构变更**
```
当前: const (HaikuMax, SonnetMax) → BandForScore() → TierForScore()
未来: CalibratedThresholds{mu, sigma, min_samples} → 由 calibration loop 周期性调整
      calibration loop: scorecard.update → drift detection → threshold recalculation
```
- 新增 `internal/routing/calibrate.go`（或 `internal/routing/threshold.go`）
- `BandForScore` 从读 `const` 改为读 `*ThresholdSet`（通过接口或配置注入）
- `calibrate` 触发点：`forge scorecard rebuild` 可加 `--calibrate` flag；v2 可定期自动触发
- `threshold_config.go`（或 `policy.yml` 的阈值覆盖段）定义 `min_samples`、`damping_factor`、`reset_on_model_change`

**对现有系统的影响**
- 向后兼容：无 `CalibratedThresholds` 配置时使用当前常量值
- 不影响 `HistoryTiebreak` 路径（同档择优逻辑不变）
- `forge scorecard rebuild` 的输出格式需要扩展（新增 `suggested_thresholds` 字段）

### 方向 B：预测性运行估算引擎（P1）

**为什么需要**
这是从「反应式护栏」到「主动式预算管理」的架构跃迁。当前四维资源护栏（深度/数量/时间/内存）都是硬截止——它们被动阻断灾难但不提供「是否值得开始」的决策支持。Sprint 24-26 的真点火实验已经证明：用户最大的心理障碍不是技术可行性，而是「我不知道这个 run 要花多少钱」。

**核心挑战**
1. **多模态预测输入**：`(mode, lifecycle, workflow, model_tier, phase_count)` 构成高维特征空间 → 需要足够的样本量才能得到有意义的桶
2. **离群值抑制**：一次因 API 故障导致的重复重试不应扭曲后续预测 → 百分位数（p50/p90）优于均值
3. **模型 ID 感知**：`Sonnet-4` 和 `Sonnet-5` 虽都是 `sonnet` tier，但性能/成本不同 → 预测桶必须以模型确切 ID 而非 tier 为键
4. **诚实性标注**：预测本质是不确定估计 → 输出必须附带置信区间（如「预计 $0.30-0.50, 90% 置信区间」），而非单值

**预期的架构变更**
```
新增 internal/predict 包（或 internal/trace 的 forecast 子包）：
  - PredictRun(wf, mode, lifecycle, model_tier) → RunEstimate{duration, cost, iterations, confidence_interval}
  - 数据源：.forge/trace.jsonl（历史事件）+ .agent/routing/scorecards.json（质量聚合）
  - 桶策略：分组于 (model_id, task_type, mode, lifecycle)
  - 冷启动：从 forge-core 内嵌的经验基线（按 mode×lifecycle 的默认值）
```
- `forge run --dry-run` 调用预测器输出报告
- `forge evolve` 可以在 start 之前打印预测（类似 `terraform plan` 的成本估算）
- `predict` 包不应引入外部统计库——纯 Go 实现分位数/EMA/箱线图离群值剔除

**对现有系统的影响**
- 纯加法：不影响 trace/scorecard 现有结构
- Scorecard 的 `AvgIterations` 和 `PassRate` 字段已是预测输入，无需扩展 schema
- 对 `checkRunBudget` 无影响（反应式护栏不变）

### 方向 C：相位级补偿原语——编排状态机的事务化（P1）

**为什么需要**
这是审查文档中方向二（补偿撤销）的架构深化版。当前编排模型本质上是**前向执行器**：
- `RunFrom(index)` 跳回目标相位
- `on_fail.loop_back` 定向重跑
- `on_rejected` 拒绝后回退

但缺少的是一个正式的状态机语义：**每个 phase 不再只是「执行→成功→前进」的线性步骤，而是一个可逆的原子操作**，有 `do` / `undo` 两条路径。这不是 `forge rollback` 这种事后 CLI 命令，而是运行时原语。

**为什么需要现在做**
当前编排的 loop-back 机制已有跳回能力，但每个 phase 没有「副作用已经泄漏出去」的意识。如果 phase 3 写了 git commit、打了 tag、推了远程，然后 phase 5 失败触发 loop_back 到 phase 2——那 phase 3 的远端副作用就成了悬挂状态。这不是「未来问题」：Sprint 25 的真点火已经展示了 agent 会写 git commit。

**核心挑战**
1. **副作用范围界定**：一个 phase 有哪些副作用？git commit 是显式的，但 agent 修改了工作树文件也是副作用——而且无法避免（agent 的本职就是改文件）
2. **补偿的原子性**：`undo` 也可能失败——补偿的补偿怎么办？需要 at-most-once / best-effort 语义
3. **声明的 vs 隐式的副作用**：workflow 文件声明了 `emits:` 产物，但 agent 可能写到其他地方
4. **与 checkpoint 的交互**：一个已 checkpoint 的 phase 被补偿后，checkpoint 应被标记为 stale

**预期的架构变更**
```
asset.Phase 扩展：
  - CompensatePhase string    // undo 阶段的 phase name（不是函数，是另一个 phase）
  - CompensateMode  CompMode  // automatic | manual | best_effort

orchestrator 状态机：
  - RunFrom 若 detect 补偿路径 → 先跑 CompensatePhase（从后往前链式执行）
  - CompensatePhase 有自己的 MaxRetry、timeout、on_fail

新 internal/compensate 包（可选）：
  - 管理补偿链的执行
  - 记录补偿结果到 trace（新 Kind: Compensate）
```
- **不**引入事务日志或 write-ahead log（架构过于复杂）
- **不**引入两阶段提交（与实际工程场景不匹配）
- **不**自动补偿 git 远端推送（那是手动操作——补偿阶段可打印恢复说明）

**对现有系统的影响**
- 中间过渡：先加 `CompensatePhase` 字段但不强制消费（workflow 不声明则行为不变）
- loop-back 是触发补偿的信号源——`on_fail.loop_back` 到 target 前自动跑从当前到 target 之间所有 phase 的补偿链
- checkpoint/resume 需感知补偿——补偿后的 phase checkpoint 应标 invalid

### 方向 D：静态分析驱动的多维风险提取（P1/P2）

**为什么需要**
这是方向 A（自校准路由）的前置条件。路由的 6 维评分目前只有 1 维有信号，不是因为设计者忘记实现，而是因为信号生产有真实的技术难度。但 **「信号缺席」不是一个临时状态——它是一个架构半衰期问题**：时间越长，新开发者越可能假设「声明即实现」，从未意识到路由运行的维度只有 `risk` 一维。

**核心挑战**
1. **语言多样性**：forge-core 本身是 Go，但工作目录的代码可以是任何语言。AST 分析需要语言感知，而所有的语言检测器（`detect.go`）目前只有文件扩展名级探测。
2. **时序窗口**：`forge evolve` 中 agent 写的代码在被 gate 检查前没有持久化 diff → 风险分析需要在 agent 写完但 gate 没跑前的时间窗口内完成，极窄。
3. **启发式 vs 精确分析**：从路径字符串启发式升级到文件内容扫描（方向三的 v1 建议）已经是增量进步，但需要明确的诚实边界——不夸大内容分析的准确度。
4. **复杂度维度的信号来源**：圈复杂度需要外部工具（gocyclo/lizard/radon）→ harness adapter 框架已有，但复杂度信号消费是空缺的。

**预期的架构变更**
```
internal/risk 扩展：
  - FromChangedPaths 保留（向后兼容基线）
  - FromFileContent(paths []string) → map[string]float64  // 轻量内容嗅探
    - 检测 import/require/include 语句 → dependency_change 维度有信号
    - 检测函数声明模式 → complexity 维度有雏形信号（非圈复杂度，纯行数启发式）
    - 检测敏感关键词 → security 维度增强
  
routing.Score() 重构：
  - 不再是固定加权求和，而是 pluggable scorer 接口
  - 每个维度可独立开关（demension_enabled boolean）
  - Scorecard 历史数据可选择作为维度的校准输入

harness adapter 管道集成：
  - coverage adapter 的姐妹：complexity adapter
  - probe 检测 gocyclo/lizard → 输出圈复杂度聚合 → routing.consumer
```
- `scoring.signals` 从 policy.yml 的声明字段变为消费字段——每个 signal 必须有对应的 producer
- `check.py` 或 `check.go` 添加 drift guard：检测 policy.yml 声明的 signal 是否有对应的 producer 实现

**对现有系统的影响**
- 向后兼容：现有路径启发式作为最低基线保留
- 内容嗅探是增量（不改变 `FromChangedPaths` 的已有行为）
- 复杂度 adapter 是 harness 层面的加法，对 forge-core 核心无影响

### 方向 E：梯度响应系统——从二值开关到三区护栏（P2）

**为什么需要**
这是审查文档中方向五的核心洞察：当前所有护栏（MaxDepth、MaxOutputBytes、Timeout、MaxAgentCalls、MaxLoopBack）返回 error/nil 二值。但工程现实是：

- 输出接近 8MB（阈值 10MB）——这不是「正常」也非「错误」，它是 **warning 区**
- 迭代数达到 max-iter 的 80%——这不是「失败」但也不是「正常收敛」
- 预算用到 90%——未来 10% 内的相位可能需要降档而非 block

现有的 `BudgetAdjustTier` 已经是梯度思想的雏形，它没有被推广到其他护栏。

**核心挑战**
1. **阈值层次设计**：对于每个护栏，需要定义三个区：
   - **green**（正常，无动作）
   - **amber**（接近阈值 → 降档、告警、降级运行模式）
   - **red**（达到硬截止 → block）
2. **降级恢复路径**：amber 区的降档行为（如从 sonnet 降到 haiku）后，如果下一个 phase 负载降低，能否恢复？必须避免「锁死在低档」。
3. **跨护栏联合状态**：如果一个 run 同时接近输出上限（amber）和超时上限（amber），联合状态是否触发比各自独立更严厉的响应？
4. **诚实观测**：amber 区在 `forge run` 输出中必须可见，不能只在日志中安静降级。

**预期的架构变更**
```
internal/budget 或 internal/guard 新包（或拆分到现有 gate 包）：
  - GuardThreshold 结构体：{Level: Green|Amber|Red, Value: float64, Action: Action}

CmdExecutor / LoopEngine 中的护栏检查点统一接口：
  - CheckGuard(metrics) → GuardResult{Level, Actions[], Message}
  - 不再是逐个 if-else 检查

已有护栏升级：
  - MaxOutputBytes: 8MiB→amber, 10MiB→red
  - MaxAgentCalls: limit*0.8→amber, limit→red
  - Timeout: p90+20%→amber, p100→red（如果方向上预测引擎已建，可驱动动态 amber）
  
跨护栏联合：
  - 两个 amber → 等效一个 red 的保守策略
```
- `forge run --dry-run` 输出每个护栏的当前/阈值/区
- `forge evolve` 在 amber 区自动限制 loop-back 迭代数

**对现有系统的影响**
- 这是所有方向中对现有代码侵入性最低的——几乎所有改动是加法（新增 `CheckGuard`）
- 现有 error/nil 消费者保持兼容（error 仍代表 red，nil 代表 green；amber 走新返回路径）
- `BudgetAdjustTier` 可自然迁入梯度框架

---

## 3. 接口设计建议

### 关键模块设计原则

**a) 渐进式复杂性暴露**
ForgeOS 作为编排控制平面，不同用户看到不同复杂层级：
- **CLI 用户**：只看到 `--dry-run` 的预估报告，不了解内部是 3 区护栏还是 2 值开关
- **Workflow 作者**：看到 `CompensatePhase: rollback_payment` 的声明式 YAML 字段，不了解补偿链的状态机实现
- **平台扩展者**：看到 `GuardThreshold` 接口和 `scorer.Scorer` 接口，可插入自定义实现

所有新 API 应遵循「默认无行为变化」原则——新字段不设任何默认值、新接口为空接口时不触发额外行为。

**b) `scorer` 接口设计**

多维路由当前的问题不是「无处安放 scorer」，而是 scorer 太耦合于 `routing` 包。推荐：

```go
// internal/scorer/scorer.go
type SignalProducer interface {
    Signals() []string           // 该 producer 能生成的 signal 名列表
    Score(ctx context.Context, input SignalInput) (map[string]float64, error)
}

type SignalInput struct {
    ChangedFiles  []string
    FileContents  map[string][]byte  // 可选，nil 时不读文件
    Diff          string             // git diff 内容
    ImportGraph   *ImportGraph       // 可选，依赖图
    Language      string             // 主语言
    Scorecard     *routing.Scorecard // 历史数据
}

type Combinator interface {
    Combine(signals map[string]float64, weights Weights) float64
}
```

`Combinator` 与 `SignalProducer` 分离的权衡：
- **优点**：信号生产与信号组合可独立扩展、独立测试
- **缺点**：接口引入间接层，对当前只有 1 维有信号的状态是过度设计
- **结论**：先有 2-3 个 `SignalProducer` 实现再引入接口，不提前抽象

**c) 预测引擎的输出格式**

```go
// internal/predict/estimate.go
type RunEstimate struct {
    DurationMs     Range  "估计耗时(ms)"
    CostUsd        Range  "估计成本(USD)"
    Iterations     Range  "估计迭代数"
    Confidence     string "high|medium|low|insufficient_data"
    PerPhase       []PhaseEstimate
    ModelDetails   map[string]ModelEstimate
}

type Range struct {
    P50  float64
    P90  float64
    Mean float64
    N    int "样本数，用于置信度判断"
}

type PhaseEstimate struct {
    PhaseName string
    DurationMs Range
    CostUsd   Range
}
```

不引入外部统计包（符合零依赖纪律），纯 Go 实现：
- 分位数：`sort.Float64Slice` + 加权百分位数
- 离群值检测：IQR 规则（Q1-1.5*IQR, Q3+1.5*IQR）
- EMA 平滑：`alpha=2/(N+1)`

### 是否需要新抽象层

| 方向 | 新抽象层 | 理由 |
|------|---------|------|
| 路由自校准 | `CalibratedThresholds` 结构体 + `calibration loop` | 当前 `const` 无法被任何外部逻辑修改，需要可变的阈值持有者 |
| 预测引擎 | `internal/predict` 新包 | 与现有 `trace`/`scorecard`/`budget` 都有数据流，但不属于任何一个 |
| 补偿原语 | `asset.CompensatePhase` 字段 + `orchestrator` 的补偿链执行逻辑 | 当前 `asset.Phase` 的 yaml 标记已有但零消费的字段，需要消费逻辑 |
| 多维风险 | `scorer.SignalProducer` 接口（延迟到第 2 个 producer 实现时） | 单个 producer 不需要接口，但第二个 producer 需要组合 |
| 梯度响应 | `GuardThreshold` + `GuardResult` | 从当前 error/nil 二值到三值的跃迁，需要新返回路径 |

### 向后兼容性策略

ForgeOS 的「向后兼容」有以下层次，按严格度排序：

1. **CLI exit code 不变**：`forge run` 的退出码不能因新功能变化（除非显式 `--flag` 请求）
2. **YAML/JSON 配置解析不崩溃**：新字段必须是被解析器静默忽略的（当前 yaml2json 的行为）
3. **trace JSONL schema 只追加不修改**：新字段加在 Event 末尾，旧消费者忽略未知字段
4. **Go API 只扩展不破坏**：新接口方法可加但旧实现编译失败（可选方案：用 functional options 或新接口 embed 旧接口）
5. **Scorecard schema 不可变**：已落盘的 scorecard 文件的字段变更需要迁移路径

---

## 4. 技术选型

### 是否需要引入新技术栈或框架

对于所述 5 个方向，**结论是：都不需要**。理由：

| 方向 | 新技术栈需求 | 说明 |
|------|------------|------|
| 路由自校准 | ❌ 无 | 纯数学运算（EMA/分位数），Go 标准库绰绰有余 |
| 预测引擎 | ❌ 无 | 同上，纯统计操作 |
| 补偿原语 | ❌ 无 | 纯状态机逻辑，Go 原生 |
| 多维风险 | ⚠️ 可选：harness adapter 的复杂性扩展 | 圈复杂度可走 adapter 管道（已建框架），不需额外运行时 |
| 梯度响应 | ❌ 无 | 纯逻辑扩展 |

**不引入**的明确决策：
- 不引入统计学库（`gonum/stat`、`gorgonia` 等）——维持零外部依赖
- 不引入规则引擎（OPA/Rego）——策略已在 YAML 中表达，梯度规则是代码逻辑
- 不引入 golang.org/x/exp 之外的任何包——当前依赖中即使 `x/exp` 也未使用，应当保持

### 自建 vs 采购的决策依据

ForgeOS 的自研/采购分界线已经在 north-star 架构中定义。对于所述方向：

| 能力 | 决策 | 依据 |
|------|------|------|
| 预测引擎核心逻辑 | **自研** | 预测不是 ForgeOS 的核心差异化？不对——**预算是 ForgeOS 的商业模型前提**。没有预算确定性就没有无人值守。这必须是自研核心。 |
| 圈复杂度适配器 | **采购工具**（gocyclo/lizard）→ **自研适配器** | harness adapter 模式已确立，不需要自研复杂度计算器 |
| 补偿状态机 | **自研** | 这是编排编排引擎的核心；任何外部状态机库都会引入不匹配的语义（两阶段提交 vs 补偿的 at-most-once） |
| 文件内容风险嗅探 | **自研轻量级** | 不需要完整 AST 分析器——第一版只是正则 + import 检测，Go 标准 `regexp` 和 `go/parser`（标准库）已足够 |

关键决策：**是否引入 `go/parser` 做 Go 代码的 AST 风险分析？**

- 赞成：`go/parser` + `go/ast` 是标准库，不破坏零依赖纪律；对 Go 代码的风险分析准确度远高于正则
- 反对：引入了语言偏向——为什么 Go 项目可以 AST 分析，但 TypeScript/Python/Rust 项目只能正则？
- **结论**：引入 `go/parser` 作为第一个语言感知的分析器，明确标注「Go-only AST analysis」，其他语言退回正则。诚实比均衡更重要。

### 第三方依赖的评估标准

ForgeOS 的零外部依赖哲学意味着任何新依赖的门槛非常高：

1. **绝对必要？** 能不能纯 Go 标准库实现？90% 的情况答案是「能」
2. **引入后谁维护？** 如果上游停更，团队是否有能力 fork？对于统计计算，能（因为算法稳定）；对于规则引擎，可能不能
3. **编译时间影响？** 对于 CLI 工具，编译时间每增加 1 秒都需要论证
4. **二进制体积影响？** 纯 Go 标准库的 forge-core 的二进制体积极小；引入 `gonum/stat` 可能增加 2-3MB
5. **CGo？** 禁止。CGo 破坏跨编译和零依赖纪律

对于所述方向，**无任何新依赖通过以上门槛**。纯标准库实现。

---

## 5. 实施路线图

### 优先级排序和理由

| 方向 | 推荐优先级 | 原名 | 理由 |
|------|-----------|------|------|
| **方向 A** → 路由自适应校准 | **P1** | 方向一（审查稿） | 直接降低成本/质量比；前置条件少；增量验证路径清晰 |
| **方向 B** → 预测性运行估算 | **P1** | 方向二（审查稿） | 解锁无人值守的最后心理障碍；trace 数据已就绪；纯加法 |
| **方向 C** → 相位级补偿原语 | **P1** | 方向二（审查稿的补偿部分） | 编排状态机的完整性缺口；已在真点火中暴露副作用问题 |
| **方向 D** → 多维风险提取 | **P1→P2** | 方向三（审查稿） | 高价值但前置条件多（需要 language adapter 基础）；可先做轻量内容嗅探 |
| **方向 E** → 梯度响应系统 | **P2** | 方向五（审查稿） | 价值确实但当前二值开关已够用；不建议在 amber 区逻辑未经过验证前投入 |

**关于 P2 的说明**：梯度响应系统是「当前已工作的产品的体验升级」，不是新能力。它应该在被 ask-for 或有数据证明二值开关导致问题时才升级为 P1。

### 阶段划分和里程碑

**Phase 1（2-3 sprints）- 路由自校准 + 预测引擎并行推进**

```
Sprint A:
  - routing/threshold.go: 从 const 到可配置 ThresholdConfig
  - routing/calibrate.go: 纯统计函数（分位数、EMA、离群值检测）
  - `forge scorecard rebuild --calibrate` 输出建议阈值
    
Sprint B:
  - internal/predict: 基础预测引擎
  - trace/forecast.go: 从 trace.jsonl 聚合历史数据
  - `forge run --dry-run` 输出预测报告
    
里程碑 M1: forge run 前可预估成本和时间；forge scorecard 可建议路由阈值调整
验证方法: 在 examples/url-shortener 上运行 `forge run --dry-run`，验证输出合理性
```

**Phase 2（2-3 sprints）- 补偿原语 + 轻量内容嗅探**

```
Sprint C:
  - asset.Phase 加 CompensatePhase/CompensateMode 字段（零消费，先声明）
  - orchestator 状态机扩展：loop_back 前检查补偿链
  - 单测验证补偿链执行正确性
    
Sprint D:
  - internal/risk 扩展：FromFileContent（轻量内容嗅探）
  - 连接 risk → routing 的信号管道
    
里程碑 M2: 编排状态机支持补偿原语；路由风险分析基于文件内容而非仅路径
验证方法: 手工构造含支付关键词的文件变更 → 验证风险等级上调
```

**Phase 3（2 sprints）- 多维信号扩展 + 梯度响应**

```
Sprint E:
  - harness adapter: complexity probe（检测 gocyclo/lizard/radon）
  - routing.Score() 接入圈复杂度信号
  - check.py drift guard: 检测 policy.yml 声明的 signal 是否有 producer
    
Sprint F:
  - internal/guard: GuardThreshold 三区框架
  - 输出输出/超时/预算护栏升级为三区
  - `forge run --dry-run` 输出护栏状态
    
里程碑 M3: 完整 6 维评分（复杂度维度有真实信号）；护栏三区响应
验证方法: 高圈复杂度文件变更 → 路由自动升档；接近资源上限 → amber 告警
```

### 风险点和缓解策略

| 风险 | 影响 | 概率 | 缓解策略 |
|------|------|------|---------|
| 路由校准导致系统性降档 | 质量全面下降（用户不察觉，但 reviewer REQUESTS_CHANGES 比例上升） | 中 | v1 只报告不动手；v2 自动调整绑定 `min_samples=10` + `max_delta_per_adjustment=0.05` + 发布自动回滚 |
| 预测引擎冷启动值偏差大 | 用户首次看到预测不准确 → 丧失信任 | 高 | 首次运行显示「insufficient_data」而非错误数字；嵌入式基线从 forge-core 自身运行数据聚合 |
| 补偿链执行失败后的悬挂状态 | 工作树进入不可恢复的脏状态 | 低-中 | 补偿本身可能失败——设计原则是 best-effort 而非 atomic；补偿失败后在 trace 中标记、`forge doctor` 可检测 |
| 内容嗅探的假阳性触发不必要的 Opus 路由 | 成本上升但不影响质量 | 中 | 内容嗅探结果与路径启发式 OR 组合（任一触发则升档），但设计为「只升不降」；可通过 policy.yml 设置 demension_enabled=false 关闭 |
| 梯度响应的 amber 区行为过于保守 | 用户在 amber 区频繁被降档，体验比二值开关更差 | 中 | amber 区默认只告警不降档（advisory-only），用户显式开启 `--gradient-action` 才触发降档 |

### 不做（明确不做的三项）

1. **不与外部预算管理系统集成**（如 AWS Budgets、GCP Budget Alerts）——这是 v3 或用户自建的范围
2. **不实现全自动 AST 跨语言风险分析**（方向三的 v3 建议当前过于投机）——保持诚实标注的启发式
3. **不实现基于 ML 的预测**（线性回归/时间序列）——维护零外部依赖，纯统计预测已足够

---

## 总结

ForgeOS 的架构基础扎实——零外部依赖、中枢旋钮设计、诚实标注、fresh-context reviewer 纪律——这些是该系统的真正护城河。所述五个方向不是「重写架构」而是在现有架构缝隙中的**高价值填充**：

- **方向 A + B** 是学习闭环的水平和垂直扩展——从「同档择优」到「档位自校准」、从「反应式护栏」到「主动式预算」
- **方向 C** 是编排状态机的完整性补全——当前前向执行的回路缺少逆操作原语
- **方向 D** 是路由多维评分的信号层补缺——从 1 维到 6 维的渐进路径
- **方向 E** 是护栏设计的体验升级——从二值到三值的人机交互优化

最大的架构建议不是关于任何一个方向本身，而是关于**它们之间的数据流依赖**：
- 预测引擎 ≈ 所有方向的基础设施：它为自校准（A）提供当前基线、为梯度响应（E）提供动态 amber 阈值
- 多维风险（D）≈ 路由自校准（A）的信号输入——校准不改变输入，只调整边界；但输入变化表示系统的底层分布已变
- 补偿原语（C）≈ 独立于其他方向的独立方向——它不需要其他方向的前置条件，但其他方向（如自校准）的失败模式需要补偿

因此推荐从 **Phase 1 的预测引擎 + 路由自校准**并行启动，因为它们共享同一个数据管道（trace → scorecard → estimator/calibrator），彼此无依赖但互为输入——这是最经济的启动路径。
