文件不存在于磁盘中，纯以 Prompt 内嵌文档为输入。我已完整阅读。以下是架构层分析。

---

# 架构分析报告：ForgeOS 学习循环闭合性

> 基于对 `expansion-five-systemic-learning-loop-gaps.md` 的审查，从架构层面进行深入分析。

---

## 1. 架构评估

### 1.1 当前架构的优势

| 架构特性 | 当前状态 | 评价 |
|---------|---------|------|
| **路由分层** | `PhaseTier` → `BudgetAdjustTier` → `HistoryTiebreak` | 正确的关注点分离，layering 清晰 |
| **管道模型** | Phase → lane → step → wave → signal | 基于 DAG 的并行化设计合理 |
| **记分卡系统** | Scorecard → storage → CLI query | 数据收集层建好了，是后续闭环的基础 |
| **形式化闸门** | `gate.mjs` + `arch-check.mjs` + `check.py` + `secret-scan` | 从治理角度约束了架构退化 |

### 1.2 核心局限（架构债务）

**P0（必须还）：**

1. **数据层 → 决策层断连** —— scorecard 数据已完整采集，但对 `BudgetAdjustTier` 不可见。数据采集是架构投资，不从数据中产生决策回报，等于投资沉没。这是最核心的架构债务。

2. **可观察性缺失** —— 三个关键维度（route explain → budget explain → tier decision explain）均无公共可见性。`trace.Event` 连 `DecisionChain` 字段都没有。可观察性不是功能需求，它是**运维层需求**——没有它，自适应系统变成黑箱，用户不信任，最终关掉特性。

3. **验证缺失** —— `emits:` 不提验证，无正式规格；ADR 格式无自动检查。这不是「没时间做」，这是**设计阶段没有将验证嵌入模型定义**。asset model 的定义元数据没有「可验证断言」。

**P1（应还）：**

4. **跨 phase 自洽性零检测** —— `gateLedger` 收集了所有 gate 结果，但没做 contradiction detection。如果 phase A pass 的约束在 phase B 被违反，系统不告警。这在大 workflow 中会逐渐腐蚀一致性。

5. **并行协调协议薄弱** —— `Waves` 只有 `depends_on`，缺少集体熔断（collective circuit breaker）。一个 lane 耗时异常拖死整个 wave。

### 1.3 关键设计决策回顾

| 决策 | 当时选项 | 选择 | 5 年后看是否正确 |
|------|---------|------|----------------|
| Route 分层：Budget → History | 两阶段决策（先 budget 淘汰，再 history 选优） | 分离 | ✅ 正确，但有缺口（Budget 不感知 history） |
| Safety-floor agents 锁定 Opus | 允许降级 vs 硬锁定 | 硬锁定 | ⚠️ 权宜之计，核心审阅者应可以基于数据降级 |
| `ForgeFile` 松散结构 | 强类型 schema vs 宽松 | 宽松 | ⚠️ 灵活性换来了 `emits:` 不可验证 |
| 记分卡存储为独立 kv store | SQLite vs flat files | JSON 文件 | ⚠️ 可扩展到一定程度，但无查询优化 |

---

## 2. 扩展方向

### 方向 A：Scorecard-Aware Routing（P0）

**业务价值**：消除「采集了数据但不用」的 ROI 缺口。直接提升下游成本/质量比。

**技术挑战**：
- `BudgetAdjustTier` 当前只读 `spendRatio`，需要扩展接口接收 `map[tier]modelQuality` 输入
- 需要定义「质量」的量化指标：当前 `PassRate` 是唯一可用信号，但 PassRate 对 task_type 的粒度是否够？
- 冷启动阈值问题——新 task_type 需要多少 samples 才开始使用历史数据？

**架构变更**：
```
// 当前:
BudgetAdjustTier(arenaCost map[string]CostMatrix, tx tx.Tally) Adjustment

// 建议:
BudgetAdjustTier(arenaCost map[string]CostMatrix, tx tx.Tally, 
                  qualityMap map[string]map[TaskType]QualityMetrics) Adjustment
```

**对现有系统的影响**：
- 接口扩大（不破坏向后，新增参数可选）
- `historyMinSamples` 需要暴露为可配置参数而不是 hardcoded
- 需在 `LoadScorecards` 和 `BudgetAdjustTier` 之间建立数据管道

**选项评估**：

| 选项 | 方法 | 复杂度 | 风险 | 推荐 |
|------|------|--------|------|------|
| A1 | 扩展 BudgetAdjustTier 参数 | 低 | 低，接口扩展 | ✅ |
| A2 | 第三阶段路由（Budget → Quality → History） | 中 | 引入新的 layering 复杂度 | ❌，过度设计 |
| A3 | HistoryTiebreak 前移到 Budget 之前 | 低 | 改变决策语义 | ❌ |

### 方向 B：Route Explainability Layer（P0）

**业务价值**：用户信任的前置条件。没有 explainability 的自适应路由会被人工覆盖或关停。

**技术挑战**：
- 需要定义 `DecisionChain` 的标准格式：`{phase, lane, choices: [{tier, reason, scorecard}]}`
- `trace.Event` 当前结构需要扩展——这是公共 contract，不可轻改
- 输出展示：CLI `--explain` 还是 `forge route explain <id>`？

**架构变更**：

```go
// 新增类型（不需要修改已有结构）：
type DecisionChain struct {
    Phase     string
    Lane      string
    Jumps     []TierJump   // BudgetAdjustTier 的 each drop
    History   *HistoryPick  // HistoryTiebreak 的选择
    Scorecard []ScorecardRow // 相关 scorecard 数据
}

type TierJump struct {
    From       Tier
    To         Tier
    Reason     string       // "budget 85% spent, dropped to Haiku"
    SpendRatio float64
}
```

**向后兼容策略**：`DecisionChain` 以 optional 字段加入 `trace.Event`，不加到时正常。CLI 端 `--explain` 标志默认 false。

### 方向 C：Formal Asset Validation（P0/P1）

**业务价值**：当前 `emits:` 声明无验证，ADR 格式无检查。这不是功能问题——它影响**管道完整性**。如果一个 phase emits 了 A，下游 phase requires A，但实际 A 没产生，下游要么挂要么静默用旧值。

**技术挑战**：
- `emits:` 的验证需要实际运行 phase 后检查输出——这是运行时验证，不是声明时验证
- ADR 格式的一致性需引入 schema 检查器
- 并行模式下（方向五场景），多个 phase 同时 emits 同一文件需要冲突检测

**架构变更**：

```yaml
# ForgeFile 扩展建议
asset:
  emits:
    - path: "adr/003-decision.md"
      format: "adr"           # 新增，声明格式
      required: true          # 必须产生，否则 fail
      schema: "adr/v1"        # 新增，schema 版本
```

**验证时机决策**：

| 选项 | 验证时机 | 优缺点 |
|------|---------|--------|
| C1 | Phase 执行后立即验证 | 失败快速反馈，但增加延迟 |
| C2 | 延迟到最后 `converge` 阶段统一验证 | 批量化高效，但失败晚 |
| C3 | 并行模式改为产生即验证 | 在 wave 结束前可重做 |

**推荐**：C1 + C3 组合。单个 phase 执行后立即验证 emits 完整性（快速失败）；并行模式下产生即验证（给重做留窗口）。

### 方向 D：Cross-Phase Consistency Engine（P1）

**业务价值**：大型 workflow 中 phase 间矛盾是渐进的腐蚀。跨 phase 自洽性检查在 phase 数量 > 5 时变成硬需求。

**技术挑战**：
- 需要定义「自洽性」的具体规则——没有通用规则引擎，需要领域特定检查
- `gateLedger` 记录了 `Entry[]`，但 Entry 之间没有 cross-reference 索引
- 需要区分「矛盾」和「演进」（ADR 003 supersedes ADR 002 是演进，不是矛盾）

**架构变更**：

```go
// 在 converge.Signals 中新增
type ConsistencyCheck struct {
    PhaseA       string
    PhaseB       string
    ConflictType string       // "gate_contradiction" | "adr_overwrite" | "tier_conflict"
    Severity     Severity     // soft_warning | hard_block
    Supersedes   string       // 如适用
}
```

**最小可行路径**：不建通用引擎，先写 3 条硬规则（ADR supersede cycle detection、gate pass/fail reversion、tier allocation flip-flop），规则编码在 `converge.go` 中，不引入规则 DSL。

### 方向 E：Collective Circuit Breaker for Parallel Waves（P1→P2）

**业务价值**：并行 wave 中一个 lane 挂了，其他 lane 仍在空转耗 budget。需要集体熔断。

**技术挑战**：
- Wave 内的 lane 是独立的 goroutine，需要共享的 cancelation 通道
- 需要定义熔断阈值：等待时间超限 vs 错误率超限 vs budget 消耗超限
- 熔断后的重试策略需要与 `depends_on` 拓扑协调

**架构变更**：

```go
type WaveConfig struct {
    Dependencies  []string
    Timeout       time.Duration  // 新增
    Collective    bool           // 新增，是否集体熔断
    OnFailure     FailAction     // "cancel_wave" | "skip_remaining" | "continue"
}
```

**最小可行路径**：先加 `Timeout` 到 `WaveConfig`，超时后 cancel 整个 wave 的 context。后续再加 `Collective` 布尔标志。

---

## 3. 接口设计建议

### 3.1 核心原则

| 原则 | 说明 |
|------|------|
| **Optional 扩展** | 新功能以 optional 字段/参数加入现有结构，不破坏零值语义 |
| **只加不减** | 已有接口只添加新方法/字段，不改动已有签名 |
| **分层解耦** | 数据层（scorecard）、决策层（route）、展示层（explain）不跨层依赖 |
| **冷启动容忍** | 所有依赖历史数据的接口，在数据不足时必须优雅回退，不 panic |

### 3.2 建议引入的抽象层

1. **`QualityProvider` 接口** —— 屏蔽 scorecard 存储实现，提供给 `BudgetAdjustTier` 和 `HistoryTiebreak` 统一消费入口
2. **`ExplainabilitySink` 接口** —— `trace.Event` 背后的写入抽象，允许 CLI、日志、WebUI 三种 sink
3. **`AssetValidator` 接口** —— 将 `emits:` 验证从 asset model 中解耦，允许按格式注册不同的验证器

### 3.3 需要避免的陷阱

- ❌ **不要加全局状态**：不要设计 `globalScorecardCache`，所有数据通过参数传递
- ❌ **不要在热路径加锁**：`HistoryTiebreak` 是每次路由调用，读 scorecard 数据应该用 snapshot，不读实时 store
- ✅ **使用 snapshot 模式**：`LoadScorecards` 在 engine_build 时一次性 snapshot，后续 HistoryTiebreak 读 snapshot，不读 store

---

## 4. 技术选型

### 4.1 需要引入的新依赖？

**结论：不需要。全部在现有技术栈内可完成。**

| 方向 | 可能的外来依赖 | 替代方案 | 推荐 |
|------|--------------|---------|------|
| 规则引擎（方向 D） | drools, gengine | Go 内硬编码规则 | ✅ 不自建规则 DSL |
| Explainability UI | React, Vue, htmx | CLI 输出即可 | ✅ 先用 CLI，后续再考虑 Web |
| Schema 验证（方向 C） | JSON Schema, CUE | 自定义 `checkAdrFormat()` | ✅ 初期不要依赖外部 schema 引擎 |

**如果非要引入一个**，JSON Schema（`santhosh-tekuri/jsonschema`）对 `ForgeFile` 的 `emits` 扩展有帮助。但当前 20 行以内规则没必要。

### 4.2 自建 vs 采购决策矩阵

此处「采购」指引入开源库。ForgeOS 的约束是 **zero external dependencies**，所以除非有不可抗拒的理由，一律自建。

| 功能 | 自建成本 | 外部库价值 | 结论 |
|------|---------|-----------|------|
| ADR 格式验证 | 5 行 regex + 1 个 format 函数 | 无（格式是自定义的） | ✅ 自建 |
| Scorecard 查询缓存 | 10 行 | 无 | ✅ 自建 |
| 路由 explain 输出 | 15 行 `fmt.Sprintf` | 无 | ✅ 自建 |
| 跨 phase 矛盾检测 | 3 条硬规则编码 | 规则太少不值得用引擎 | ✅ 自建 |

### 4.3 关于 HistoryTiebreak 的「已接线」修正的技术影响

方向一的 Evidence B 中我指出 HistoryTiebreak 已在运行时中（v1.5, 6月29日）。这修正了文档的事实，但不影响所有方向的技术选型判断：

- BudgetAdjustTier 仍然不感知 scorecard → 需要参数扩展（见方向 A 的接口变更）
- `historyMinSamples` 值需要审计 → 在 `phaseTierResolver` 初始化处加 logging，不要用配置（当前 0 dependencies）

---

## 5. 实施路线图

### 5.1 优先级排序

```
P0 ──────────────────────────────────────────────► P1 ─────────────► P2
方向C·emits验证(1-2d)         方向D·跨phase自洽(3-5d)       方向E·集体熔断(5-8d)
方向B·route explain(2-3d)     
方向A·scorecard→Budget(3-5d)  
```

**排序依据**：
- P0 的三个方向串联起来构成闭环：**可验证 → 可解释 → 自适应**
- 方向 C（验证）在方向 B（解释）之前，因为 explain 想展示的决策必须基于已验证的数据
- 方向 A（自适应）在方向 B 之后——用户先能看到决策链，才能信任自适应行为
- 方向 D/E 依赖更少的其他方向，可并行

### 5.2 阶段划分

**Phase 1（2 周）—— 基础闭环**
1. 方向 C 的最小验证：`emits:` 的 `format` 字段 + ADR 格式检查器
2. 方向 B 的最小 explain：`DecisionChain` 结构 + `--explain` CLI 标志
3. 方向 A 的接口扩展：`BudgetAdjustTier` 接收 qualityMap 参数，第一次路由评估时无视（数据还不够），接口就位

**Phase 2（2 周）—— 自适应落地**
1. 方向 A 的决策集成：`BudgetAdjustTier` 在 qualityMap 中有数据时，不降级到质量过差的模型
2. Floor agent 软降级（方向 A 子项）：reviewer 单候选策略改为双候选 + scorecard 判断
3. scorecard snapshot 缓存优化

**Phase 3（2 周）—— 一致性与并行**
1. 方向 D：3 条硬规则编码
2. 方向 E：Wave 级别的超时熔断

### 5.3 风险矩阵

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| `historyMinSamples` 门槛过高导致方向 A 冷启动时长不可接受 | 中 | 高 | 审计默认值，设为 3（而非当前可能的 10+）；允许 task_type 级别覆盖 |
| 方向 B 的 `DecisionChain` 字段扩展被误用为公共 contract | 中 | 中 | 在 PR 中标记为 `@internal`，不承诺向后兼容，v2 再定型 |
| 方向 D 的矛盾检测产生过多 false positive | 高 | 中 | 初始只实施 `soft_warning` 等级别，不 `hard_block`；由 Reviewer 标记 false positive 后手动加入白名单 |
| 方向 E 的 Wave timeout 打断长时 phase | 低 | 高 | 默认 timeout 为 ∞（opt-in 模式），只有显式设置 `timeout` 才启用 |

---

## 总结

| 维度 | 判断 |
|------|------|
| **架构健康度** | 中等偏上。layering 清晰，但数据流未闭环导致 ROI 未达成 |
| **最大的架构债务** | Scorecard → Budget 断连 |
| **最紧急的架构价值** | Route Explainability（B）——无信任的自适应系统会被关停 |
| **最小的投入最大回报** | emits 验证（C）——5 行代码解决的问题 |
| **建议立即开始** | 方向 C（1 天完成）→ 方向 B（2 天完成）→ 方向 A（3-5天完成） |

闭环从今天开始。
