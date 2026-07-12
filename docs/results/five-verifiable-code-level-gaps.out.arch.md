以下是我的架构分析报告。

---

# 架构分析：ForgeOS 结构盲区与扩展方向

> 分析范围：v2 forge-core（13 Go 包，纯 stdlib，~7.3k LOC 非测试代码）+ harness 层（~10.5k LOC）
> 输入基础：2026-07-12 验证报告（5/5 方向全确认）+ 已有 120+ 份分析文档 + ROADMAP + ARCHITECTURE + AGENTS 红线

---

## 一、架构评估

### 1.1 优势

ForgeOS v2 的架构在同类系统中极为罕见地干净。几个关键决策值得肯定：

- **零外部依赖策略**（`go.mod` 无 `require`）：不是教条主义，而是精确匹配「编排引擎不应依赖自己编排不了的运行时」这一核心命题。这在工程上强制了每一行代码都为存在理由而被选择。
- **五引擎分层**（Orchestrator / Model-Router / Context-Engine / Memory-Engine / Evaluation-Engine）：责任边界清晰，没有 God Package。`internal/asset` 的类型定义被所有引擎共享但不被任一引擎拥有——这是正确的依赖方向。
- **带外执法架构**（out-of-band gate）：不信任 agent 自身的 introspection，将 gate 跑在独立的 Sandbox 或 CI runner 中。这是整个系统抗腐化的根基。
- **声明-实现分离**："写 agent 卡 / workflow / policy，而不是写代码"——将意图与实现解耦，使系统的行为可以通过 YAML 变更调整，无需改 Go 代码。

### 1.2 结构性局限

上述优势掩盖了三个跨域的结构性问题：

**问题 A：接口契约的「最小化正确 × 最大化脆弱」两难**

`AgentExecutor` 当前是单方法接口（`Exec(ctx, phase, mode)` → `(output, err)`），这在只有一个实现（`DryRunExecutor`）时正确且最小。但任何接口如果只匹配当前唯一实现，就不是抽象——只是间接调用。

这不是「过度设计」与「欠设计」之争：当接口只服务于单一实现时，它没有捕获任何**跨实现的共性**。生命周期管理（Init / Shutdown / Rollback / Health）没有进入接口，不是因为它们不需要，而是因为第一个实现不需要。**第一个实现从不暴露第二个实现的需求。**

**问题 B：解析层有隐式契约，无显式门禁**

Agent 输出解析（`cost.go` 的四个解析函数）是 ForgeOS 中**错误密度最高的代码**：exact-match 行尾、`strconv.Atoi` 拒绝百分号、JSON 要求单行无尾缀。这在单个解析函数看是合理的容错选择，但作为一个**子系统**看，它缺少：

1. 统一的解析框架（每个解析器自建循环）
2. 输出格式的机器可读声明（schema）
3. 格式偏离时的反馈回路（fail-open 意味着信号静默丢失）

**问题 C：数据路径上的写入探险**

- `loadCache`（`sync.Map`）无界增长——不是 LRU 的问题，是缺少 entry-level TTL + 容量上限 + backpressure 的问题。`sync.Map` 是并发安全工具，不是缓存设计。
- Checkpoint / Memory / Scorecard 的版本标记写了但不查——不是「忘记查」，而是「写版本是为了什么」这个元问题没有被回答。如果版本标记不用于迁移决策，那它们只是装饰性元数据；如果用于，那缺失读取端验证是正确性 bug。

### 1.3 架构债务分类

| 类别 | 项 | 严重性 | 修复窗口 |
|------|----|--------|---------|
| **正确性风险** | Agent 输出解析 fail-open 静默丢失信号 | P1 | 本 sprint |
| **正确性风险** | 双 YAML 解析器无交叉验证 | P1 | 本 sprint |
| **数据完整性** | Confidence 零值歧义：0 → 1.0，旧文件永远 `<0.3` | P2 | 下 sprint |
| **安全缺口** | `PhaseIndex` 无负值 / Nil 守卫，checkpoint 注入可 panic | P2 | 下 sprint |
| **资源泄漏** | `loadCache` 无界增长，长 run 内存单调递增 | P3 | 2 sprints |
| **架构债** | AgentExecutor 单方法接口，无生命周期契约 | P2 | 接入真正 agent 前 |
| **治理债** | 4 个 `ADDED HERE ONLY` 字段无消费端 | P2 | 3 sprints |

---

## 二、扩展方向

### 方向一：Agent 输出契约系统（Contractual Agent Output）

**为什么需要（P1）**

当前 `parseReviewerVerdict` / `parseConfidenceScore` / `parseClaudeCostUsd` 四个解析器各自独立、fail-open、无 schema。这是系统中最危险的非功能缺陷：**信号被静默丢弃而无人知晓**。

一个 `REQUEST_CHANGES` 被误读为 `APPROVE` 意味着不合规的代码直接进入 QA → 可能合并。一个 `CONFIDENCE: 85%` 被丢弃意味着收敛判定认为「无置信度」→ 可能在 60% 时就 stop。

**核心挑战**

- **格式与灵活性之争**：过于严格的 schema 会让 agent 输出「人工感」下降，过于宽松的 schema 会重蹈当前 exact-match 的覆辙。
- **fail-open vs fail-close**：当前 fail-open（解析失败 = 无信号）。改为 fail-close（解析失败 = 要求重试）会提高可靠性但降低吞吐。
- **多 agent 类型**：reviewer 输出 verdict、product-manager 输出 confidence、implementer 输出 diff——每个角色需要自己的输出契约。

**架构变更**

```
当前：                                   目标：
agent 输出 ─→ 文本 ─→ 独立解析器         agent 输出 ─→ 文本 ─→ Schema 验证层
                                                          └→ 失败 → 要求重试 / 降级
                            └→ 每个 agent 卡声明 ResponseSchema
                            └→ SchemaLayer 统一路由到对应的验证器
                            └→ 验证失败写 Scorecard + 不入内存
```

**选项权衡**

| 选项 | 优点 | 缺点 |
|------|------|------|
| **A. JSON Schema 约束** | 精确、可版本化、可测试 | agent 输出自由度降低，需要 prompt 适配 |
| **B. 宽松正则 + 多候选** | 兼容现有 agent 行为 | 正则集随时间膨胀，漏报风险仍在 |
| **C. 两阶段：先 fuzzy 再 exact** | 兼具灵活性 + 精确性 | 两阶段判定逻辑复杂，不容易解释 |

**推荐**：选 A（JSON Schema），因为 ForgeOS 已经通过 agent 卡声明 agent 行为，输出 schema 应作为 agent 卡的扩展字段。对旧 agent 卡向后兼容：如果无 `ResponseSchema`，保持现有解析器行为。

### 方向二：Executor 生命周期契约（Executor Lifecycle Contract）

**为什么需要（P2）**

当前 `AgentExecutor` 只有一个实现（`DryRunExecutor`），不需要生命周期。但以下现实场景注定会到来：

- `CommandExecutor` 需要 `Init()` 建立子进程连接池、`Close()` 释放资源
- `ClaudeExecutor`（通过 SDK）需要 `Auth()`、`RefreshToken()`、`Health()`、`Reset()`
- `SandboxExecutor` 需要 `Provision()`、`Destroy()`、`Snapshot()`、`Rollback()`

每次新 executor 接入都从零解决这些问题，说明接口边界放错了位置。

**核心挑战**

- **接口膨胀 vs 组合**：`Init / Close / Health / Rollback / Reset / State` 六个方法一起加入是膨胀，但分成 `Lifecycle` + `Executable` + `Recoverable` 子接口是组合。
- **有状态 vs 无状态**：真正的 agent CLI 是有状态的（认证、会话、速率限制），但当前接口假定无状态。
- **重试语义**：`Exec` 返回 `error` 后，调用者应该重试吗？调用者应该退避多久？这些现在必须在每个调用者中重复实现。

**架构变更**

```go
// 目标接口设计（组合模式）
type AgentExecutor interface {
    Executable
    Lifecycle
}

type Executable interface {
    Exec(ctx context.Context, p asset.Phase, mode string) (ExecResult, error)
}

type Lifecycle interface {
    Init(ctx context.Context, cfg ExecutorConfig) error
    Close(ctx context.Context) error
    Health(ctx context.Context) HealthStatus
}
```

**对现有系统影响**

- `DryRunExecutor` 实现 `Executable`（当前 `Exec`）+ `Lifecycle` 的 `Init` 和 `Close` 为空操作
- `LoopEngine.Run` 在 `engine_cmd` 后 defer `Close()`，在 `engine_cmd` 前调用 `Init()`
- 所有现有测试无需修改（DryRun 的 Lifecycle 方法是 no-op）

### 方向三：增量 Gate 执行框架（Differential Gate Execution）

**为什么需要（P2）**

当前 loop-back 场景中，已 PASS 的 gate（如 `go test ./...`）在 loop-back 后无条件重跑。在 `MaxLoopBack=3` 且 gate 密集的场景下，每次 loop-back 增加 3× gate 时间。实测 `go test` + `arch-check` + `secret-scan` 套件约 20-60s，3 次 loop-back 最多浪费 3 分钟在纯重跑上。

**核心挑战**

- **文件变更跟踪**：自上次 gate 运行后哪些文件变了？需要精确到文件级别的 mtime / hash 跟踪。
- **gate 依赖声明**：不是所有 gate 对所有文件敏感。`arch-check` 只关心 `.go` 文件，`secret-scan` 只关心非 `.gitignore` 文件，`test` 关心所有被测试覆盖的文件。需要每个 gate 声明它关注的文件 glob。
- **结果缓存失效**：文件变了 → 相关的 gate 结果失效 → 其他 gate 可复用。这个依赖图需要声明式建模。

**架构变更**

```
当前：                                   目标：
loop-back ─→ 无条件重跑所有 gate         loop-back ─→ 查文件变更集
                                                   └→ 按 gate 的 glob 过滤
                                                   └→ 只重跑受影响的 gate
                                                   └→ 未受影响的 gate 复用缓存结果
```

**关键设计选择**

| 方案 | 复杂度 | 增益 | 风险 |
|------|--------|------|------|
| **A. 文件 mtime 跟踪 + 结果缓存** | 中 | 中（gate 重跑减少 50-80%） | mtime 跨平台不一致 |
| **B. Git diff-based 增量** | 低 | 高（只比较 committed + staged vs working tree） | 不跟踪 unstaged 变更 |
| **C. 不做（接受现状）** | 零 | 零 | loop-back 多的 workflow 墙钟线性增长 |

**推荐**：选 B，因为 git diff 已经在 `.agent/` 上下文中可用（`checkpoint.go` 依赖 git），且 git diff 的结果与 platform 无关。需要 `gate.yml` 增加 `file_glob` 声明。

### 方向四：结构 Schema 消费完整性治理（Structural Schema Consumption Governance）

**为什么需要（P2）**

`asset.Phase` 中 4 个 `ADDED HERE ONLY` 字段（`RequiresTools`、`Readonly` ×2、`SecondaryTemplate`）已被 YAML 加载和携带，但没有任何代码消费它们。这是「声明-实现间隙」的低级形式——schema 的未来证明（forward-compatibility）和 schema 的幽灵字段（dead weight）之间的边界正在模糊。

**核心挑战**

- **自动化检测**：如何从 Go 结构体中自动提取「有 YAML 解码器但无消费代码」的字段？
- **准入 gate**：新字段加入 asset schema 时，应默认要求提供消费计划（至少是一个 tracked issue，而不是 `/TODO`）。
- **版本化**：`ADDED HERE ONLY` 字段在不同版本间如何迁移为正式字段？

**架构变更**

- 新增 `forge validate --schema-consumption` 命令：遍历 `asset.Phase` 的字段注解，报告未被消费的字段及其引入 commit
- 在 `AGENTS.md` 硬闸门中增补：**新 asset 字段必须有消费端或明确标记为 experimental**，否则 `forge accept` 拦截

### 方向五：解析器统一框架 + 双解析器交叉验证（Parser Unification & Cross-Validation）

**为什么需要（P1）**

两个独立的 YAML 解析器（Go `yaml2json` + Python shim）在没有交叉验证的情况下并存。当 Go 解析器对合法 YAML 输出与 Python 不同的 JSON 且未报错时，系统静默使用 Go 版本。这不是维护成本问题——**这是正确性漏洞**。

**核心挑战**

- **golden file 测试**：需要一组覆盖所有边缘情况的 YAML fixture，以及一个断言两个解析器输出 byte-identical JSON 的测试
- **CI 集成**：每次 PR 更改任一解析器时，自动跑交叉验证
- **单解析器迁移路线**：长期目标是淘汰 Python shim，但在此之前必须有交叉验证

**实施建议**

1. 创建 `internal/yaml2json/testdata/fixtures/`：覆盖混合缩进、注释嵌套、多行 block scalar、Unicode BOM、纯数字字符串等边缘情况
2. `yaml2json_test.go` 增加 `TestGoPythonConsistency`：对每个 fixture，分别用 Go 解析器和 `exec Python shim` 解析，断言结果 JSON 严格相等
3. CI（`forge.yml`）将此测试设为 blocking

---

## 三、接口设计建议

### 3.1 核心原则

ForgeOS 当前的接口设计风格是**最小主义**（minimalist）——每个接口只暴露当前实现需要的方法。这在 v2 早期是正确的，但系统已跨越了「证明概念」阶段，进入「确保可靠性」阶段。

建议引入以下三条接口设计准则：

1. **契约优先于实现**：接口应反映消费者需求，而不是提供者的便利。`AgentExecutor` 当前的 `Exec(ctx, phase, mode)` 反映的是 `DryRunExecutor` 的提供能力，不是 `LoopEngine` 的消费需求——`LoopEngine` 需要 `Init`、`Health`、`Close`、`Rollback`。
2. **错误可区分**：当前 `error` 返回是扁平的。调用者无法区分「Executor 初始化失败」（应重试）和「Phase 执行失败」（应 loop-back）和「LLM 调用超时」（应降档）。建议引入 `ExecError` 类型族。
3. **结构化结果**：`Exec` 应返回 `ExecResult`（含输出摘要、文件变更集、agent 元数据），而不是 `(string, error)`——后者把结构化数据挤压到字符串中再在另一端用 `parseXxx` 解析回来。

### 3.2 关键接口重设计

**AgentExecutor 接口族**

```go
// 不破坏现有代码：DryRunExecutor 实现新接口语义等价于当前行为
type AgentExecutor interface {
    Exec(ctx context.Context, p asset.Phase, mode string) (*ExecResult, error)
}

type ExecResult struct {
    // Phase 输出的结构化摘要
    Summary      string
    // Agent 产生的裁决（reviewer → verdict, product-manager → confidence）
    Verdicts     []AgentVerdict
    // Phase 执行期间修改/创建的文件
    ChangedFiles []string
    // Agent 元数据（模型、token 用量、耗时）
    Meta         ExecMeta
}

type ExecError struct {
    Kind    ExecErrorKind  // Init | Exec | Timeout | Auth | RateLimit
    Phase   string
    Retryable bool
    Cause    error
}
```

**Schema 验证接口**

```go
// 每个 agent 卡可声明其输出的预期 schema
type OutputSchema struct {
    // 如 "reviewer" → 验证 VERDICT 字段存在、取值在 APPROVE/REQUEST_CHANGES 内
    AgentRole string `yaml:"agent_role"`
    // JSON Schema 定义（可选，无定义时用内置解析器）
    JSONSchema string `yaml:"json_schema,omitempty"`
    // 定义是否从 agent 输出中提取（如 "last line of output"）
    Extraction string `yaml:"extraction"`
}
```

### 3.3 向后兼容策略

- **所有新接口以 opt-in 方式引入**：旧 `AgentExecutor` 仍可工作，新 `LifecycleAwareExecutor` 在 `type switch` 中被检测后启用高级功能
- **旧 YAML 解析行为不变**：交叉验证只做 warn 日志 + 测试断言，不改变当前加载逻辑
- **`ADDED HERE ONLY` 字段保持兼容**：消除方式不是删除字段，而是引入消费代码（使注释变为 true）

---

## 四、技术选型

### 4.1 当前约束

ForgeOS 的「零外部依赖」策略（Go stdlib only）是**工程原则，不是技术限制**。在 v2 阶段，这个原则为系统带来了：

- 无 CVE 供应链风险
- 无 go.sum 管理负担
- 编译时间稳定在 < 2s
- 二进制体积 < 10MB

**不建议打破这个原则用于常规功能扩展**，但可以考虑有限例外。

### 4.2 有限例外评估

| 方向 | 建议 | 理由 |
|------|------|------|
| JSON Schema 验证 | 自建（~200 行 Go） | JSON Schema 验证的核心逻辑极其简单：类型检查 + 枚举约束 + 正则。Go stdlib 的 `encoding/json` 配合反射可以构造一个微型验证器。 |
| WASM gate 引擎 | 暂不引入 | `wazero` 虽然是纯 Go WASM 运行时且零 CGO，但引入它打开了 WASM 运行时依赖。ROADMAP 中方向 3 建议 POC 阶段再评估。 |
| LiteLLM 多厂商路由 | 作为 Python shim（已存在 python3 依赖） | 不在 Go 侧构建 LLM 客户端——通过 Python shim + HTTP 调用已有模式。Go 侧只负责路由决策，不负责模型调用。 |
| 文件变更跟踪 | `git diff-tree` + 自建缓存 | git 已在系统路径上（`checkpoint.go` 依赖），标准库 `os/exec` 调用，零外部依赖。 |

### 4.3 「自建 vs 采购」决策框架

ForgeOS 的决策应遵循以下优先级（从优到劣）：

1. **Go 标准库解决** — 如信号处理、context 传播、mutex、mtime 比较
2. **调用现有 CLI** — 如 `git diff`、`python3 -c`、`node -e`
3. **自建小型库（≤ 300 行）** — 如微型 JSON Schema 验证器、YAML diff 工具
4. **vendored 零 CGO 依赖** — 如 `wazero`（需讨论打破零外部依赖原则）
5. **外部运行时依赖** — 如 Python LiteLLM、Node 的 ESLint WASM

---

## 五、实施路线图

### 优先级排序

| 优先级 | 方向 | 工作量 | 风险回报比 | 决定因素 |
|--------|------|--------|-----------|---------|
| **P0** | 方向一：Agent 输出契约 | ~1 sprint | ⭐⭐⭐⭐⭐ | 正确性——解析静默失败已在影响评审、收敛、预算三个 load-bearing 路径 |
| **P0** | 方向五：双解析器交叉验证 | ~0.5 sprint | ⭐⭐⭐⭐⭐ | 正确性——YAML 加载是所有资产的基石，两个解析器不同是定时炸弹 |
| **P1** | 方向二：Executor 生命周期 | ~2 sprints | ⭐⭐⭐⭐ | 可扩展性——接入真正 agent 前必须解决，现在做比那时做便宜 10x |
| **P2** | 方向三：增量 Gate 执行 | ~1 sprint | ⭐⭐⭐ | 性能——影响墙钟但不影响正确性，loop-back 少的 workflow 不受影响 |
| **P2** | 方向四：Schema 消费治理 | ~1 sprint | ⭐⭐⭐ | 治理——长期 schema 卫生，短期不修复不会出 bug |

### 阶段划分

**Phase A（Sprint 32 — 立即开始）**

```
目标：止血——消除正确性漏洞
├── Agent 输出契约：引入 OutputSchema 到 agent 卡，统一 VERDICT/CONFIDENCE 解析到 schema 验证层
│   ├── 建立 AgentVerdict / ConfidenceScore / CostReport 三种 schema
│   ├── 内置验证器（约 200 行 Go）
│   ├── 解析失败改为写 Scorecard + 要求重试（fail-close），不改 fail-open
│   └── 覆盖现有 parseXxx 函数的 golden test
├── 双解析器交叉验证
│   ├── 建立 YAML fixture 集（10-15 个边缘场景）
│   ├── Go vs Python 输出 diff test
│   └── CI 中设为 blocking
└── Confidence 零值修复
    └── memory.go：去掉无条件 `if e.Confidence == 0 { e.Confidence = 1.0 }`，区分「未设置」和「零置信度」
```

**Phase B（Sprint 33-34）**

```
目标：加固——Executor 生命周期 + PhaseIndex 安全
├── AgentExecutor 接口拆分
│   ├── 定义 Executable / Lifecycle / Recoverable 子接口
│   ├── DryRunExecutor 实现所有子接口（no-op）
│   ├── LoopEngine.Run 集成 Init/Close/Health
│   └── ⚠ HUMAN APPROVAL：接口设计评审
├── PhaseIndex 守卫
│   ├── RunFrom 增加 start < 0 || start > len(Phases) 检查
│   └── Checkpoint 加载后验证 PhaseIndex 范围
└── Schema 字段消费审计
    └── forge validate --schema-consumption 子命令
```

**Phase C（Sprint 35+）**

```
目标：优化——增量 Gate + Cache 治理
├── 增量 Gate 执行框架
│   ├── gate.yml 增加 file_glob 声明
│   ├── GateResultCache（基于 git diff-tree + 文件 hash）
│   ├── LoopEngine 在 loop-back 时查询缓存
│   └── fallback：无 file_glob 声明的 gate 保持当前行为
├── loadCache 上限
│   ├── sync.Map → cache.Cache（含 maxEntries + TTL + eviction callback）
│   └── 测试：长 run 内存有界增长
└── 版本标记消费
    ├── Checkpoint.Load 检查 FormatVersion ← "forgeos.checkpoint.v1"
    ├── Memory.Decode 检查 Format
    └── 版本不匹配时写日志 + 走兼容路径（不拒绝加载）
```

### 风险点与缓解

| 风险 | 可能性 | 影响 | 缓解 |
|------|--------|------|------|
| Agent 输出契约 fail-close 降低吞吐 | 中 | 中 | 可配置：默认 fail-close，单条解析失败要求重试，但总重试次数有限制 |
| Executor 接口重设计影响现有测试 | 低 | 低 | DryRunExecutor 的 Lifecycle 方法是 no-op，测试不变 |
| 增量 Gate 的 file_glob 声明不足 | 中 | 低 | 无声明的 gate 保持当前行为，渐进式迁移 |
| 双解析器交叉验证发现实际差异 | 高 | 高 | 这是实际价值——发现差异后按 case 决定：取 Python vs 修 Go vs 取共识 |
| Phase A 和方向 1（韧性运行时）依赖冲突 | 低 | 中 | Phase A 不依赖 context 传播；方向 1 是正交的。两个 Phase A 可并行 |

---

## 总结

ForgeOS v2 的架构基础非常扎实——五引擎分层、零依赖政策、带外执法，都是正确的长期选择。当前的五个结构盲区（Agent 输出契约、双解析器、Executor 生命周期、增量 Gate、Schema 消费治理）中，**前两个是正确性风险，必须在下一 sprint 修复**；中间两个是扩展性债，在接入真正 agent 前必须解决；最后一个是治理卫生，长期可持续改善。

最关键的架构建议不是具体技术方案，而是一个**元决策**：**接口设计应从「当前最小实现」转向「下一个实现的需求预判」**。`AgentExecutor` 的单方法设计不是错误——它是 v2 早期阶段正确的选择。但现在系统已跨越了概念验证，接口不应再为「只有一个实现」而设计，而应为「我们已知至少两个实现将需要什么」而设计。
