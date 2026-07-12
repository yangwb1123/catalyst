下面是对 **ForgeOS 全局扫描新增 5 个扩展方向**的完整架构分析。

---

# 架构分析报告：ForgeOS 扩展方向评估

## 1. 架构评估

### 1.1 当前架构优势

ForgeOS v2 体现了罕见的架构纪律。以下优势值得明确识别：

| 优势 | 表现 | 评级 |
|------|------|------|
| **纯 Go stdlib 零依赖** | 18 个 internal 包无外部 import；避免了 Go 生态常见的依赖地狱 | ★★★★★ |
| **分层清晰** | `domain (asset) → converge/memory/trace → orchestrator/cmd` 的依赖方向严格向内 | ★★★★ |
| **闸门体系完整** | `gate.mjs`(体积)+`arch-check.mjs`(架构 8 检)+`check.py`(治理)+`secret-scan.mjs`(安全) 四扇闸门形成防护矩阵 | ★★★★ |
| **增量演进证据确凿** | `parallel.go` 从 `RunFrom` 零侵入分支出行，`prompt_context.go` 三车道注入路径渐进叠加 | ★★★★★ |
| **trace 数据结构化程度高** | `trace.Event` 含 Kind/Status/DurationMs/CostUsdMicros/Model/Detail，已是可分析级别 | ★★★★ |
| **锁顺序契约** (parallel.go 第 15-35 行) | 8 级互斥锁获取顺序文档化，并行路径的并发安全有据可查 | 罕见 |

这是一套**为长期演进而设计**的系统。v2 没有走捷径。

### 1.2 当前架构局限性

五个方向诊断出的缺口定位精准，本文从架构视角补充根因分析：

| 局限性 | 根因 | 严重度 |
|--------|------|--------|
| **workflow 间无组合语义** | `cmd/forge` 是扁平命令，`orchestrator.Engine.Run` 无嵌套能力；on_approved.next_stage 字段声明但零消费 | 高（阻碍自治愿景） |
| **memory 无界增长 + 无跨运行继承** | `Load` 全量反序列化 O(n)；`Prune` 按数量不按时间/重要性；无跨 run 继承路径 | 中（长期退化） |
| **并行执行无资源治理** | `runWave` goroutine 启动无 throttle；backoff 无 jitter 导致 thundering herd；budget 检查无公平性 | 高（经济风险） |
| **trace 数据无二次利用** | `trace.jsonl` 是 append-only，从不读取分析；scorecard 只覆盖成功路径 | 中（诊断盲区） |
| **phase 产出无契约校验** | `Emits []string` 只有路径名没有 schema，零验证 | 低中（信号断裂） |

### 1.3 技术债务盘点

该文档识别的"4 处死字段"（Sprint 30 审计）值得关注。从架构债务角度：

| 债务类别 | 具体 | 严重度 | 建议 |
|----------|------|--------|------|
| **已声明零消费字段** | `on_approved.next_stage`, `Emits` 无校验 | 低 | 方向一/三直接消除 |
| **backoff 无 jitter** | `backoff.go` 第 61 行注释故意为之，但并行模式下有害 | 低 | 方向四可附带修复 |
| **trace 文件无限增长** | 无轮转/压缩/保留策略 | 中 | 方向二可覆盖 |
| **memory.Compact 不自动调用** | 显式调用才能触发，无守护协程 | 中 | 方向二覆盖 |

**评估结论**：这是一套健康但即将面临规模拐点的架构。5 个方向的识别时机准确——在进入 24h 自治运行之前补齐。

---

## 2. 扩展方向

### 2.1 方向一：工作流组合引擎 (Workflow Composition Engine)

**优先级：P1** | **影响：新 `internal/composer/` + `cmd/forge` 新入口**

#### 为什么需要

- **业务价值**：从"工具"到"平台"的飞跃。当前 5 个 workflow（discover→design→review→build→evolve）是离散的 CLI 调用。一个 `forge evolve` 如果不能从 discover 直通 build，ForgeOS 就无法实现其核心价值主张（"让 AI 长期无人值守"）。
- **技术价值**：`on_approved.next_stage` 是已声明零消费字段，消除它本身就是架构整洁性提升。同时，`LoopEngine.Run` 已演示了多迭代收敛驱动模式——workflow 组合可以看作外层 LoopEngine 嵌套内层 Engine.Run，复用现有接口而不引入新范式。

#### 核心挑战

1. **跨 workflow 信号传递**：当前 `feeds_forward` 只在 phase 之间工作。workflow 之间的上下文（架构决策、批准、置信度）如何序列化/反序列化？
2. **失败语义**：如果 discover 不收敛到置信度≥80%，pipeline 应该阻塞等待、降级使用最佳成果、还是整体失败？这需要声明式策略，而非 hardcode。
3. **human_approval 的 durable wait**：当前人审是基于文件系统签核的同步轮询。组合引擎中，一个 workflow 可能等待数小时。需要定义超时/超时后的默认行为。

#### 预期的架构变更

```
┌─────────────────────────────────────────┐
│            Composer (新)                 │
│  ┌─────────────────────────────────────┐│
│  │  PipelineStage: name, workflow,     ││
│  │    depends_on, signal_map, timeout  ││
│  │  Pipeline: ordered []PipelineStage  ││
│  │  Compose(ctx, pipeline) → Report    ││
│  └─────────────────────────────────────┘│
│  ┌─────────────────────────────────────┐│
│  │  SignalBridge: 从上一 stage 的      ││
│  │  memory/checkpoint/trace 中提取     ││
│  │  信号注入下一 stage 的 ctx           ││
│  └─────────────────────────────────────┘│
└─────────────────────────────────────────┘
         ↕ 复用
┌───────────┐ ┌─────────┐ ┌──────────┐
│Engine.Run │ │converge │ │memory    │
│(serial)   │ │.Converge│ │.Append   │
└───────────┘ └─────────┘ └──────────┘
```

#### 对现有系统的影响

| 维度 | 影响 |
|------|------|
| 向后兼容 | ✅ `forge run` 行为不变；`forge pipeline run` 是新可选入口 |
| 现有 workflow YAML | ✅ 无需修改（composer 读取同一组 YAML 作为阶段） |
| north-star 对齐 | ✅ 朝 Temporal 方向走第一步，但不引入外部依赖 |

#### 架构决策选项

| 选项 | 方案 | 权衡 |
|------|------|------|
| A | 线性链，文件系统信号传递（文档推荐） | 简单、零依赖；但分支/并行/条件跳转要 v3 |
| B | 声明式 Pipeline YAML（定义 stages/transitions/error_handling） | 更灵活、可版本控制；但需要 schema 设计 + 迁移现有 workflow |
| C | 复用 `converge.Signals` 作为跨 workflow 信号 | 复用已有接口；但 converge 当前语义是 phase 级，需要扩展 |

**建议**：选 A 作为 v1，B 作为 v1.5 增量。C 的设计可以提前做 API 规划，避免 v2 对 converge 的侵入式改动。

---

### 2.2 方向二：跨运行知识生命周期管理 (Cross-Run Knowledge Lifecycle)

**优先级：P2** | **影响：`internal/memory/`, `internal/persist/`, `internal/trace/` + 新 `internal/retention/`**

#### 为什么需要

- **业务价值**：G5「持续演化」闭环要求知识跨运行流动。没有它，每轮 `forge evolve` 都从空白记忆开始，系统无法积累经验，永远停留在"每次都重新发现"的阶段。这是数据飞轮的起点。
- **技术价值**：当前 `memory.jsonl` 的 O(n) Load 退化会在 1000+ 迭代后成为性能瓶颈。`Prune` 按数量裁剪而非按价值裁剪，可能导致高价值知识被低价值知识淹没。

#### 核心挑战

1. **Entry 价值评估**：按什么标准决定保留/裁剪？`CreatedAtUnix`（时效性）、`Confidence`（置信度）、`Supersedes` 引用数（被替代关系）、`Source`（来源可信度）——需要组合策略。
2. **跨运行继承的冲突解决**：如果两个运行对同一 topic 有矛盾的 Entry（一个说"payment 模块有竞态"，另一个说"已修复"），如何裁决？时间戳排序太简单，需要 `Supersedes` 链的完备实现。
3. **增量 Load**：当前 `Load` 全量反序列化全量过滤。跨运行继承后，memory 体积更大，必须改为按需加载 + 索引。这是侵入性重构。

#### 预期的架构变更

```
internal/retention/         (新包)
  policy.go                 — RetentionPolicy: MaxEntries, TTL, MinConfidence
  compactor.go              — 按 policy 执行 Compact + Archive
  inheritor.go              — ImportEntries(prevRunDir, filter) → []Entry

internal/memory/            (改动)
  memory.go                 — Load 增加 index + lazy decode（非全量反序列化）
  memory_compact.go         — 自动守护协程调用，结合 retention.Policy
  memory_index.go           (新) — 按 Source/CreatedAt/Confidence 建立索引
```

#### 对现有系统的影响

| 维度 | 影响 |
|------|------|
| 向后兼容 | ⚠️ `Load` 返回值变更（可能返回 `(nil, nil)` 改为 `(empty, nil)`）；需要 deprecated bridge |
| 持久格式 | ✅ JSONL 格式不变，新包读取已有格式 |
| north-star 对齐 | ✅ 内存/知识引擎的目标态包含 retention，增量路径合理 |

#### 架构决策选项

| 选项 | 方案 | 权衡 |
|------|------|------|
| A | 定时器驱动 Compact，Import 为 `forge memory import` 子命令（文档推荐） | 简单，用户可控；跨运行继承需手动，飞轮效率降低 |
| B | 自动继承最近一次运行的 memory（`forge evolve` 自动加载 `../prev_run/memory.jsonl`） | 飞轮自动转；污染风险（有毒 Entry 传播）需额外过滤 |
| C | sqlite 替换 JSONL | 支持高效查询/索引/压缩；违背**零外部依赖**红线 |

**建议**：选 A + B 的混合——默认自动继承仅 high-confidence Entry（Confidence ≥ 0.8），全量继承需 `--import-all`。C 搁置到 north-star（那时 Qdrant/PG 是合理选择）。

---

### 2.3 方向三：Phase 输出契约验证

**优先级：P2** | **影响：新 `internal/contract/` + `asset.go Phase` 扩展**

#### 为什么需要

- **业务价值**：自治运行的审计基线。Operator 回来需要确认每个 phase 确实干了该干的事，而非仅仅是"收敛了"。contract 验证填补了`converge.Signals` 不覆盖的缺失维度。
- **技术价值**：`Emits` 字段目前是"零消费定义"，消除它是架构整洁性提升。同时为下游 phase 提供了输入契约保证——planner 没有产出 task-plan.md，implementer 就不该启动。

#### 核心挑战

1. **契约的表达力 vs 复杂度**：只检查存在性太弱，检查完整语义需要 LLM 辅助。需要找到正确的平衡点。
2. **不阻塞执行的哲学 vs 强契约的冲突**：文档说"不阻断 phase 执行"，但如果下游 phase 缺少关键输入，不阻断会导致更糟糕的错误——LLM 在缺失输入时的"虚构填补"。
3. **与已有基础设施的接线**：`prompt_context.go` 的 `injectPhaseOutputs` 路径是自然接入点，但 inject 和 verify 应该解耦。

#### 预期的架构变更

```
internal/contract/
  check.go                 — PhaseContract: EmittedFile + ExistenceCheck + StructureCheck
  structure.go             — 正则/关键字检查章节标题（v1）
  semantic.go              — LLM 辅助语义检查（v3, 非本次）

asset/asset.go             (改动)
  Phase 增加 ContractRef string  — 指向 .agent/contracts/<phase>.yml

orchestrator/              (改动)
  run.go / parallel.go     — phase 完成后调用 contract.Check → trace.Violation
```

#### 对现有系统的影响

| 维度 | 影响 |
|------|------|
| 向后兼容 | ✅ 无 schema 变更；Emits 继续工作，contract 层可选 |
| trace 格式 | 新增 `ContractViolation` event kind |
| north-star 对齐 | ✅ 治理平面扩展 |

---

### 2.4 方向四：并行执行资源治理

**优先级：P1** | **影响：`internal/orchestrator/parallel.go`, `waves.go` + 新 `internal/parallel/policy.go`**

#### 为什么需要

- **业务价值**：`forge run --parallel` 已实现但无治理。100 个 phase 同时启动 100 个 claude 进程可以在一个 request 周期内烧掉 $200+。这不仅是经济问题，也是信任问题——用户不敢开启 parallel 模式。
- **技术价值**：当前 backoff 无 jitter 在并行模式下造成完美的 thundering herd（100 个 goroutine 以相同时序退避）。这是"串行中无害，并行中致命"的经典案例。

#### 核心挑战

1. **公平性 vs 吞吐量**：budget 检查在 `mu.Lock()` 下串行化（parallel.go 第 143-153 行），保证正确但不保证公平。当一个 wave 包含"关键路径 phase"和"可选 phase"时，谁优先获得 budget？
2. **降级语义**：`waveCancel()` 终止整个 wave（parallel.go 第 109-115 行）。但对于非关键 phase（如 docs 生成），取消已完成的 80% 工作的成本 > 让它们跑完的成本。需要 phase-level 的 criticality 标注。
3. **聚合退避**：N 个 goroutine 同时遇到 `KindOverloaded` 时各自独立退避，失去了"减少对 backend 的并发压力"这个退避的核心目的。需要一个 wave-level 的聚合退避状态。

#### 预期的架构变更

```
internal/parallel/         (新包)
  policy.go                — ParallelPolicy: MaxConcurrency, FairBudget, CriticalityDefault
  throttle.go              — wave-level semaphore (chan struct{})
  jitter.go                — 加抖退避计算

internal/orchestrator/
  parallel.go              — 接入 policy, 加 throttle channel
  waves.go                 — Phase 增加 Criticality 字段
  backoff.go               — 并行路径加 jitter（独立增量）
```

#### 对现有系统的影响

| 维度 | 影响 |
|------|------|
| 向后兼容 | ✅ 串行路径零影响；--parallel 默认可选（policy 有默认值） |
| 新字段 | `asset.Phase` 增加 `Criticality: "critical" | "optional"` |
| north-star 对齐 | ✅ 朝 Agent Registry & Scheduler 的 bin-pack/配额方向走 |

#### 架构决策选项

| 选项 | 方案 | 权衡 |
|------|------|------|
| A | 全局 max_concurrency + per-wave channel semaphore（文档推荐） | 简单、零依赖；粒度粗（不能 per-phase 区别对待） |
| B | phase-level criticality + 优先级调度 + weighted fair budget | 更精细的治理；需要 PriorityQueue，复杂度显著增加 |
| C | 不做治理，依赖用户手动限制 wave 大小 | 零开发成本；pipeline 设计者必须了解底层 limit，DX 差 |

**建议**：选 A 做 v1，B 做 v2。A 的 bufered channel semaphore（`maxConcurrency int`）在 phase 启动前 acquire 即可覆盖 80% 的风险。jitter 加抖可独立提前交付。

---

### 2.5 方向五：失败智能与自动修复建议

**优先级：P2** | **影响：`internal/trace/` 新 query + 新 `internal/remediation/` + `forge diagnose`**

#### 为什么需要

- **业务价值**：trace 数据当前是"写入后永不读取"的 append-only 日志。将其升级为**可诊断知识库**能显著改善 operator 体验——从被动处理故障到主动预防。
- **技术价值**：ForgeOS 有完善的错误分类（`KindOverloaded`/`KindTimeout`/`KindFailed`/`KindConfig`/`KindRecursionLimit`），有每次 retry 的记录，有耗时的度量，但数据之间没有关联分析。

#### 核心挑战

1. **模式识别的鲁棒性**：纯规则引擎（"同一 phase/kind 连续失败 N 次"）会误报和漏报。需要最小化 false positive，否则 operator 会 ignore 诊断输出。
2. **跨运行聚合的性能**：一个 24h 运行的 trace.jsonl 可能有数千行。跨 10 个运行就是数万行。纯内存分析不可行，需要 streaming 或 bounded aggregation。
3. **建议的产生逻辑**：从失败模式到修复建议的映射需要领域知识（"KindTimeout → 检查 backend 延迟或提升 model tier"）。这需要一张可维护的规则表。

#### 预期的架构变更

```
internal/remediation/      (新包)
  matcher.go               — FailurePattern: PhasePattern, KindPattern, CountThreshold
  catalog.go               — 模式→建议映射表（hardcode v1, YAML v2）
  report.go                — DiagnoseReport: []Pattern + []Suggestion

internal/trace/            (改动)
  query.go                 (新) — Query(Phase, Kind, TimeRange) → []Event, 聚合计数
  analyze.go               (新) — 跨运行扫描 + 模式匹配

cmd/forge/                 (改动)
  diagnose.go              — forge diagnose [--run-dir <dir>] [--window <duration>]
```

#### 对现有系统的影响

| 维度 | 影响 |
|------|------|
| 向后兼容 | ✅ 完全不影响现有执行路径 |
| trace 格式 | ✅ 不变，只读不写 |
| north-star 对齐 | ✅ 朝可观测性平台方向走第一步 |

#### 架构决策选项

| 选项 | 方案 | 权衡 |
|------|------|------|
| A | 纯规则引擎 + 硬编码模式→建议映射（文档推荐，v1） | 简单可靠；模式表维护成本随积累增加 |
| B | YAML 驱动的可扩展模式目录（v1.5） | 用户可自定义；需 schema + 文档 |
| C | LLM 辅助根因分析（v3） | 能解析模糊错误；引入不确定性 + 成本 + 延迟 |

**建议**：严格走 A→B→C 三阶段路线图。过早引入 LLM 辅助会让系统不可解释、不可测试。

---

## 3. 接口设计建议

### 3.1 核心设计原则

```
1. 复用已有类型,不发明平行宇宙
   └─ composer.PipelineStage 复用 asset.Phase(或 embedding)而非新类型
   └─ contract.Check 接收 asset.Phase + string(files) 而非新对象

2. 可观测性是一等公民
   └─ 每个新包将结构化事件写入 trace(复用 trace.Event),不另起日志体系

3. 默认安全(default-safety)
   └─ parallel.Policy.MaxConcurrency 默认 5,不是 0(无限)
   └─ memory.Retention.MaxEntries 默认 5000,不是 0(无限)
   └─ contract 默认 advisory 模式,非 blocking

4. 所有配置从 project.yml(或 retention.yml)来,非环境变量/hardcode
```

### 3.2 是否需要新的抽象层

| 抽象层 | 需要？ | 理由 |
|--------|--------|------|
| **Composer** 作为 orchestrator 的外层 | ✅ 是 | 不能把组合逻辑塞入 Engine.Run——Engine 是单 workflow 驱动，组合是跨 workflow 编排 |
| **RetentionPolicy** 接口 | ✅ 是 | 不同数据形态（memory/trace/checkpoint）需要不同的 retention 策略 |
| **ContractChecker** 接口 | ⚠️ 谨慎 | v1 只要函数签名 `Check(Phase, []string) → []Violation`，不需要接口。v2 需要接口来支持可插拔 checker |
| **RemediationMatcher** 接口 | ✅ 是 | 模式匹配在 v1 就可以接口化，便于 v2 从硬编码切换为 YAML 驱动 |
| **ResourceGovernor** 接口 | ⚠️ 谨慎 | v1 只要 `NewPolicy(maxConcurrency int) → Policy`，接口会增加理解成本 |

**关于 `internal/parallel/policy.go` 的设计建议**：不要做一个庞大的 Policy 结构体。而是提供一个 `PolicyOption` 函数式构建模式：

```
// 不要
type ParallelPolicy struct {
    MaxConcurrency int
    FairBudget     bool
    Criticality    string
    // ... 20 个字段
}

// 建议
type Policy func(*policyCtx)
func WithMaxConcurrency(n int) Policy { ... }
func WithJitter(enabled bool) Policy { ... }
func NewDefaultPolicy() *policyCtx { ... }
func ApplyPolicy(ctx context.Context, p ...Policy) context.Context
```

### 3.3 向后兼容性策略

| 变更类型 | 策略 |
|----------|------|
| 新增包 | ✅ 无害 |
| 新增子命令 (`forge pipeline`, `forge diagnose`, `forge memory import`) | ✅ 不破坏现有 `forge run`/`forge evolve` |
| 新增 asset 字段 (`Phase.Criticality`, `Phase.ContractRef`) | ✅ 零值安全（默认 behavior 不变） |
| `memory.Load` 行为变更 | ⚠️ 需要 deprecated 桥接 + 2 个 sprint 过渡期 |
| `parallel.go` 接口签名变更 | ⚠️ 只能在 **Cross Run Parallel** 此类明确的任务中进行，非此方向的内容 |

---

## 4. 技术选型评估

### 4.1 是否需要引入新技术栈

| 方向 | 建议 | 理由 |
|------|------|------|
| 方向一 Composer | **纯 Go stdlib** ✅ | 不引入 Temporal。Composer v1 的线性链不需要 durable 引擎。文件系统信号传递 + `os.Stat` 签核 = 够用 |
| 方向二 Retention | **纯 Go stdlib** ✅ | JSONL stream 读取 + 按策略删除，不需要 sqlite/bbolt |
| 方向三 Contract | **纯 Go stdlib** ✅ | 文件存在性 + 正则检查，不需要 schema DSL |
| 方向四 Governance | **纯 Go stdlib** ✅ | buffered channel + context，不需要信号量库 |
| 方向五 Remediation | **纯 Go stdlib** ✅ | JSONL streaming parse + 规则匹配，不需要 timeseries DB |

**结论**：5 个方向均可在零外部依赖的约束下实现。这与项目红线一致。

### 4.2 需要关注的 "near-0-dep" 红线

若将来某个方向被迫考虑外部依赖，评估标准应该是：

| 标准 | 必须满足 |
|------|----------|
| 纯 Go 实现 | 是（CGo 排除） |
| 类库（非 framework） | 是（不引入控制反转） |
| 零运行时（无 daemon/agent） | 是（纯嵌入式） |
| 浅依赖树 | 传递依赖 ≤ 3 |
| 非 AGPL 许可证 | 是 |
| `go mod why -m <pkg>` 可解释 | 是 |

当前候选库（仅评估，非推荐引入）：

| 候选 | 方向 | 评估 |
|------|------|------|
| `github.com/dominikh/go-tools/...` | 方向三（结构检查） | ⚠️ 静态分析工具，不适合运行时嵌入 |
| `gopkg.in/yaml.v3` | 方向三（契约 YAML） | ✅ 已在 `yaml2json` 中间接使用（但为间接） |
| `github.com/rogpeppe/go-internal/...` | 方向二（lazy file read） | ⚠️ 内部包，不稳定 |

### 4.3 自建 vs 采购

| 场景 | 决策 | 依据 |
|------|------|------|
| workflow 编排引擎 | **自建 v1**（零依赖线性链）→ **Temporal v3**（north-star） | v2 的约束 + 未来的分布式目标明确分阶段 |
| 诊断/AIOps | **自建（规则引擎 v1）** | 数据模式是 ForgeOS 特定的，不通用 |
| 契约验证 | **自建** | 语义高度特化（phase 产出 vs 通用 schema） |

---

## 5. 实施路线图

### 5.1 优先级再评估

| # | 方向 | 文档优先级 | 架构师评估 | 理由 |
|---|------|-----------|-----------|------|
| 一 | Composer | P1 | **P0-P1** | 自治基建缺失，阻碍 24h 愿景 |
| 四 | Resource Governance | P1 | **P1** | 经济风险 + parallel 已发布但缺治理 |
| 二 | Knowledge Lifecycle | P2 | **P1** | 长期退化 + 飞轮停滞，24h 运行必须 |
| 五 | Failure Intelligence | P2 | **P2** | 价值高但不阻塞，可后做 |
| 三 | Contract Verification | P2 | **P2** | P2 评级合理，自治基线增强但非必须 |

**调整**：方向二上调至 P1。方向五/三保持 P2。

### 5.2 阶段划分

```
Sprint N        Sprint N+1          Sprint N+2           Sprint N+3
┌─────────┐    ┌─────────┐         ┌─────────┐          ┌─────────┐
│ Composer│    │Governance│         │Retention│          │Diagnose │
│ v1 线性链│──▶│ +Jitter  │──▶     │ v1 TTL   │──▶     │ +Contract│
│ (P0-P1) │    │ (P1)    │         │ +Compact │         │ (P2)    │
└─────────┘    └─────────┘         │ (P1)    │          └─────────┘
                                   └─────────┘
    └── 这 4 个 sprint 中保持:
         - 每 sprint 开始/结束跑全闸门
         - 每次 Composer 改动后跑完整 acceptance
         - 每次 Governance 改动后跑 -race
```

### 5.3 每个方向的增量里程碑

#### 方向一：Composer（P0-P1）

| 里程碑 | 交付物 | 验证 |
|--------|--------|------|
| M1 | `internal/composer/` 包骨架 + `Pipeline` 类型 | `go build ./...` 通过 |
| M2 | 线性链 discover→design→build→evolve（文件系统信号传递） | 端到端 `forge pipeline run` 通过 |
| M3 | `on_approved.next_stage` 从死字段变为真驱动 | 单元测试覆盖 |
| M4 | `forge pipeline run --resume` 支持断点续跑 | 集成测试（模拟中断后重启） |

#### 方向四：Governance（P1）

| 里程碑 | 交付物 | 验证 |
|--------|--------|------|
| M1 | `internal/parallel/policy.go` + `NewDefaultPolicy()` | 单元测试 |
| M2 | buffered channel throttle 接入 `runParallel` | `-race` 无竞争 |
| M3 | backoff jitter 接入 | 测试退避散布 |
| M4 | wave 聚合退避 | 模拟 overload 测试 |

#### 方向二：Retention（P1 调整后）

| 里程碑 | 交付物 | 验证 |
|--------|--------|------|
| M1 | `internal/retention/policy.go` + `RetentionPolicy` | 单元测试 |
| M2 | `compactor.go` 按 TTL + MaxEntries 自动裁剪 | 构造超限 memory 验证 |
| M3 | `inheritor.go` + `forge memory import` | 端到端跨运行继承 |
| M4 | `Load` 增量索引（lazy decode） | 基准测试 1000+ entry 加载 |

#### 方向五：Diagnose（P2）

| 里程碑 | 交付物 | 验证 |
|--------|--------|------|
| M1 | `internal/trace/query.go` + 按 phase/kind 聚合 | 单元测试 |
| M2 | `internal/remediation/matcher.go` + 规则引擎 | 模式匹配测试 |
| M3 | `forge diagnose` 子命令 + JSON 输出 | 集成测试 |

#### 方向三：Contract（P2）

| 里程碑 | 交付物 | 验证 |
|--------|--------|------|
| M1 | `internal/contract/check.go` 文件存在性检查 | 单元测试 |
| M2 | 结构检查（正则/关键字） | 构造异常产出文件验证 |
| M3 | trace `ContractViolation` + 可选 blocking 模式 | 集成测试 |

### 5.4 风险点和缓解策略

| 风险 | 影响方向 | 概率 | 严重度 | 缓解策略 |
|------|----------|------|--------|----------|
| Composer 的跨 workflow 信号传递遗漏关键上下文 | 一 | 中 | 高 | 先做最小信号集（confidence + approval + 产出路径），迭代增加 |
| Governance throttle 导致死锁（新 lock 违反 parallel.go 锁顺序契约） | 四 | 低 | 高 | 新 lock 必须注册到锁顺序文档中；每次改动跑 -race |
| memory 增量索引（lazy decode）侵入 Load 接口，破坏下游 | 二 | 中 | 中 | 新 `LoadIndexed` 函数，旧 Load 加 deprecated 桥接，过渡期 2 sprint |
| Diagnose 规则引擎 false positive 率过高 | 五 | 中 | 低 | v1 阈值保守（连续 5 次相同失败才触发），提供 --sensitivity flag |
| 5 个方向同时开发导致注意力分散 | 全部 | 高 | 中 | 严格执行「一 sprint 最多 2 个方向优先级」纪律 |
| 并行 throttle 与 budget 检查的竞态条件 | 四 | 低 | 高 | budget 检查必须在 throttle acquire **之后**，release **之前**。时序要文档化成锁顺序第 9 级 |

### 5.5 特别风险：锁顺序扩展

`parallel.go` 第 15-35 行定义了 8 级锁顺序。**方向四新增的 throttle channel 和 policy state 必须在锁顺序契约中注册**。建议：

```
ACQUISITION ORDER EXTENSION (parallel.go 修订):
  1. trace.Tracer.mu
  2. runBudget.mu
  3. loopProbe.mu
  4. gateLedger.mu
  5. phaseOutputLedger.mu
  ...
  8. verdictLedger.mu
  [NEW] 9. waveThrottle.mu    ← wave-level StateBackoff + throttle 计数器
```

不遵守此顺序的 PR 应被闸门拦截。

---

## 总结

| 维度 | 评估 |
|------|------|
| **文档质量** | ⭐⭐⭐⭐⭐ 代码行级引用 + 跨 60 方向交叉验证 + 每个方向 4 论据 + 3-4 条边界，架构师可直接进入设计阶段 |
| **方向选择合理性** | ⭐⭐⭐⭐⭐ 5 个方向均指向系统中已被证实存在但未解决的缺口，无"为了新建而新建" |
| **架构影响评估的诚实度** | ⭐⭐⭐⭐ 方向四/五的边界条件列得清楚（"不做 LLM 辅助 v1""不改变串行路径"） |
| **最大风险** | 5 个方向同时推进的管理复杂性 > 任何单一方向的技术风险。**建议每次迭代最多 2 个方向并行** |
| **最大机遇** | Composer（方向一）+ Governance（方向四）的组合交付将使 ForgeOS 第一次具备**自治流水线**的能力——"一次编排，带治理地并行执行" |

**核心建议**：Composer v1（线性链）和 Governance v1（max_concurrency + jitter）应作为下一个迭代的两个优先方向。它们的组合使 ForgeOS 从一个"手动串联单步 CLI"升级为"自动串联并行执行平台"，这在产品叙事上是质变级别的跳跃。其余三个方向（Retention/Diagnose/Contract）是平台级基础设施但不会改变用户对产品的核心认知，可在后续迭代中稳步落地。
