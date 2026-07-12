现在我已深入了解代码库。以下是我的完整架构分析。

---

# ForgeOS 架构分析：基于「高价值扩展方向（v38）」评审

## 1. 架构评估

### 1.1 当前架构的优势

| 优势 | 证据 | 架构意义 |
|---|---|---|
| **声明式优先** | 5 个 YAML workflow + mode×lifecycle 中枢旋钮，路由/门控/收敛全策略化 | 行为变化不需改 Go 代码；降低采用门槛；支持「全知」审计 |
| **零外部依赖核心** | `go.mod` 零 require；18 个 Go 包全部 stdlib | 供应链攻击面为 0；`go vet/test -race` 在隔离环境中即时运行；CI 零 `go mod download` |
| **诚实架构（Honest Architecture）** | N/A 框架遵循 honesty-first 模式；自动门控是 host-independent 的真实源头，而非加速器；`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` | 「已知未知」本身是架构构件；无幽灵功能 |
| **分层严格** | `arch-check` 8 项检查机器执行；`internal/` 包遵循接口→应用→领域单向规则；循环依赖 = 0 | 架构规则自动执行，即使是在自治 agent 竞赛期间；无单体腐化 |
| **渐进式计算** | `DryRunExecutor`（默认安全）< `CommandExecutor`（真实 agent）< 四维资源护栏（递归/预算/超时/输出上限） | 用户可以逐渐通过信任级别演进，而无需一次性提交全部内容 |
| **中枢旋钮模式** | `mode×lifecycle` 驱动 router 档位 + harness 严格度 + workflow 深度 + 迁移 + per-phase model_tier | 单一旋钮改变行为。不是 N 个独立的 --flags（每个都有激进的默认值） |

### 1.2 当前架构的限制

**限制 1：会话内架构，非跨会话架构**
Memory Engine 已存在（`internal/memory`，JSONL 积累存储），但 LoopEngine 未在每次迭代开始时自动调用 `memory.Load` → prompt 注入。knowledge 写入是手动且临时的（仅 `evolve.go` 一处 + 测试文件）。

- **技术债务**：知识管道缺失。TF-IDF 检索器（`internal/prompt/retrieve.go`）已经是一个纯函数、零依赖的 BM25 风格评分器，但只用于约束检索，未用于 knowledge。
- **影响**：24 小时循环在小时 8 推倒了小时 2 建好的墙，因为不记得它发现的差距。
- **修复成本**：约 2 周（见第 2 节）。

**限制 2：单仓库，非多仓库**
`gate.RepoRoot()` 假设一个仓库一个世界。Checkpoint 写入 `root/.forge/checkpoint.json`——monorepo 子项目共享同一个目录互相覆盖。`PolicyStack`（继承链）未实现，尽管 ADR 0003 设计了 submodule 机制。

- **技术债务**：`project.yml` 的 `extends` 字段已声明但零解析器。字段空置比缺失更危险——用户可能以为配置了继承但实际没有。
- **影响**：组织采用需要项目 A → 项目 B 策略继承+ checkpoint 隔离。在 monorepo 场景中 checkpoint 损坏是最紧迫的风险。

**限制 3：静态 workflow 装配**
5 个 YAML workflow 是静态的。`forge detect` 产生包含丰富信号的 `projectProfile`（语言、生命周期、hasTests、hasCI），但没有 `internal/composer` 包根据 profile→phase 映射表选择/排序/配置 phase。

- **技术债务**：`Phase.DependsOn`、`Phase.Emits`、`StopCondition`（含 `conjunction`/`disjunction`/`external` 类型）已完全声明，但零处消费为动态装配。声明式元数据就绪，执行引擎未连接。
- **影响**：产品价值被锁定。静态 workflow 是每个 CI 工具都有的；动态 workflow assembly 是 AI-native 软件工厂独有的。

**限制 4：路径级风险分析，非 AST 级**
`risk.FromChangedPaths` 只读路径不读内容。`BlastRadius` 硬编码为文件数。`go/ast` + `go/parser` 可用（stdlib）但未被利用。

- **技术债务**：假阳性率高。知识引擎作为基准历史不存在，无法校准影响分析。
- **影响**：在非 production mode 下，基于 `Score()`/`TierForScore` 的智能旁路过早跳过有风险的 gate。

**限制 5：嵌套一致性的丢失**
有几个地方已经有声明式基础设施但未被消费：

| 声明字段 | 消费者 | 状态 |
|---|---|---|
| `StopCondition` 组合子类型 | `engine_build.go` | 已消费 |
| `Phase.DependsOn` | 串行引擎 | 忽略（并行引擎配置存在） |
| `Phase.Emits` | prompt builder | 设计就绪但未连线 |
| `Phase.ModelTier` override | 路由 | 已修复（Sprint 14） |
| `StopCondition.OnUnmet` loop-back | LoopEngine | 已消费（Sprint 13） |
| `project.yml:extends` | 零 | **空字段** |
| `modes.yml:priorities` | `check.py` 验证 + `forge route` 展示 | 零路由语义 |

### 1.3 关键架构决策评估

**D1 零外部依赖核心（强度：✅ 已证明，成本：⚠️ 中等）**
- **正确性**：对于编排运行时来说是正确的决定。Go stdlib 包含 `go/ast`、`go/parser`、`net/http`、`encoding/json`、`sync`。缺失的是 YAML——但 python shim 是诚实的临时方案，API 墙是 `LoadWorkflowJSON`（不是 YAML），因此引擎可以在 shim 就位的情况下工作。
- **成本**：18 个包中人为的零依赖限制了跨包调用图分析（本可以使用 `golang.org/x/tools/go/callgraph`）和内联向量检索。对于 v1 来说这是可接受的成本——扩展界限已明确标注。

**D2 带外执法为真实源头（强度：✅ 架构上声音）**
- **正确性**：通过将 `harness/gate.mjs` 设为 host-independent 的真实源头（与脆弱的 CC PostToolUse 加速器相对），ForgeOS 在 sandbox/CI 环境中保持真实——即使宿主 CLI 没有丰富的 hook API。这是「站在所有 CLI 之上」约束的正确架构响应。

**D3 mode×lifecycle 作为中枢旋钮（强度：✅ 已验证）**
- **正确性**：一个设置现在驱动 router 档位 + harness 严格度 + workflow 深度 + 迁移。Production lifecycle 一票否决任何宽松的 mode。Sprint 15（中枢旋钮完整）通过 8 个实时 `forge run` 变体已验证。这是 ForgeOS 最强大的抽象。

**D4 依赖注入的诚实边界（强度：✅ 架构优雅）**
- **设计选择**：reviewer 是 fresh-context（看不到 implementer 的输出）；planner feeds_forward 给 implementer。这种不对称在 prompt 级别强制执行新鲜度约束——是静态类型系统无法表达的，但 DSL 可以。Metadata 上的 `FreshContext` bool 是优美的：默认 false（向后兼容），当切换时，prompt builder 跳过前馈。

### 1.4 需要关注的架构债务

1. **yaml2json shim 层**：python shim 是 CI 的额外运行时依赖。需要 Go YAML 库，但零外部依赖政策与它冲突。可能的解决方案：stdlib `encoding/json` 只能解析 JSON——`internal/yaml2json`（Go 实现）已存在，但只能处理 YAML 子集。如果标准需要 JSON 作为中间格式，python shim 可以替换为 Go YAML 库（打破零依赖）。
2. **`cmd/forge` 包大小**：即使经过多轮拆分，`cmd/forge` 仍处于 15-16 个文件，接近 16 的预算。这是持续关注的。
3. **无功能需求清单**：Sprint 30 通过 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 解决了这个问题——但现在它是一个文档制品，需要在每次 sprint 后维护。

---

## 2. 扩展方向

### 方向 1：知识引擎与语义检索（P0）

**为什么需要**
- 被列为 ARCHITECTURE.md 10 引擎之一，但未实现
- TF-IDF 检索器已就绪（`internal/prompt/retrieve.go`），但仅用于约束检索，未用于 knowledge
- 没有 knowledge 引擎，影响分析（方向 5）无法校准其假阳性率——没有历史基准，就没有人敢信任旁路

**核心挑战**
- 不是构建 TF-IDF（已有），而是焊接管道：**写入**（harvest.go 从 trace/scorecard/gate 结果创建 memory 条目）和**注入**（wiring.go 在每次迭代时调用 memory.Load → prompt）
- 未解决的设计问题：memory 条目 TTL（decay.go）与 Prune 调度——memory 是累积 JSONL 日志，无删除，因此它会无限增长
- Trace `SpanID` + `ParentSpanID` 惰序列化——knowledge 需要追踪条目是由哪个 phase/iteration 生成的，以支持上下文感知检索

**预期架构变更**
- 新增包：`forge-core/internal/knowledge/`（三种文件）
  - `harvest.go`：从 trace.Event → memory.Entry 的转换器
  - `wiring.go`：LoopEngine 集成（在 `runPhase` 之前注入 knowledge）
  - `decay.go`：基于时间的 Prune 调度程序
- 对现有包的更改：
  - `internal/prompt/retrieve.go`：将约束查询与 knowledge 查询合并为一个统一的检索步骤
  - `internal/memory/memory.go`：如果需要，添加 TTL 感知的 Prune
  - `internal/trace/event.go`：添加 `SpanID` + `ParentSpanID`（作为未来扩展的基础设施）
- 无需新的 Go 依赖；零外部包

**影响评估**
| 元数据 | 值 |
|---|---|
| 代码变更估计 | ~600 行 Go |
| 涉及包 | 4 新增/修改 |
| 向后兼容性 | 完全向后兼容；无 memory 文件的旧循环继续工作 |
| 依赖影响 | 零添加 |
| 风险 | 低（TF-IDF 是纯函数，已有测试） |

### 方向 2：多项目治理联邦（P1）

**为什么需要**
- 组织采用：1 repo = 1 世界是单团队约束。企业有 50 个具有共享政策继承的仓库
- ADR 0003 设计存在（submodule + 双层覆盖），但未实现
- `project.yml:extends` 已声明但零解析器——字段空置本身就是技术债务

**核心挑战**
- **Checkpoint 隔离先于政策**：`persist.go` 写入 `root/.forge/checkpoint.json`。Monorepo 子项目共享 `root/.forge/` 会互相覆盖 checkpoint。这必须在 `PolicyStack` 之前解决（~200 行，零风险）
- **政策继承语义**：继承是覆盖还是合并？生产政策可以降低 explorer 的宽松度吗？`PolicyStack` 需要 resolve 顺序（子→父→祖父？子覆盖父，父提供默认值？）
- **路径解析改造**：`extends: ../shared/.agent/project.yml` 必须从相对路径解析，而不是从 CWD 解析

**预期架构变更**
- `internal/persist/persist.go`：添加 per-subproject checkpoint 命名空间（`checkpoint_<subproject>.json` 或一个子目录）
- 新增包：`internal/policy/`（解析 `extends` + 解析政策 DAG）
- `internal/asset/asset.go`：如果政策路径存在，则加载政策
- `internal/mode/mode.go`：继承层的 model_tier 解析（子 mode + 父 mode 覆盖？）

**影响评估**
| 元数据 | 值 |
|---|---|
| 代码变更估计 | ~800 行 Go |
| 涉及包 | 3 新增/修改 |
| 向后兼容性 | 完全向后兼容；单仓库/单项目配置不受影响 |
| 依赖影响 | 零添加 |
| 风险 | 中等（政策解析中的循环继承必须得到保护） |

### 方向 3：自动变更影响分析与智能门控（P1）

**为什么需要**
- 70% 成本节约声明的 ROI 最高；`Score()`/`TierForScore` 已就绪，只缺 AST 扫描 + gate 选择器
- 当前 `FromChangedPaths` 只读路径不读内容（高假阳性）
- `determineGateSet`（根据风险定制的 gate 集合）不存在

**核心挑战**
- **跨包调用图在零外部依赖下**：`go/ast` + `go/parser` 是 stdlib，可以解析单文件的函数声明和调用。但跨文件/跨包的调用图需要解析 `import` graph + 构建完整的符号表。V1 应限制在**同一包内**的函数引用链。跨包范围需要重复实现 `golang.org/x/tools/go/callgraph`——代价大，推迟到 v2。
- **production lifecycle 冲突**：`mode=production` + `lifecycle=production` 强制全 gate（`applyLifecycle` 的生产 floor `[]*policyCheck` 全类型），因此智能门控在 production 模式下价值为零——只有在非 production mode（explorer/balanced/quality）下才有旁路空间。
- **校准**：没有知识引擎作为历史基准，门控无法校准。方向 3 是方向 5 的前提条件，而不是独立的。
- **假阴性 vs 假阳性**：当不确定时，**放过但标记**（`classifyClaudeOverload` 的「宁愿错过也不误触发」原则，`cost.go:156-158`）。这是正确的默认值——但需要知识引擎来随着时间的推移收紧松散的阈值。

**预期架构变更**
- 新增包：`internal/impact/`（AST 分析、调用图构建、BlastRadius 计算）
- `internal/risk/risk_diff.go`：添加 `AnalyzeAST(paths)` 选项，默认回退到路径级
- `internal/routing/routing.go`：添加 `determineGateSet(risk)` 选择器
- 更改在 `cmd/forge/gates.go` 中，以在非 production mode 下消费裁剪后的 gate 集合

**影响评估**
| 元数据 | 值 |
|---|---|
| 代码变更估计 | ~1000 行 Go |
| 涉及包 | 3 新增/修改 |
| 向后兼容性 | 应通过检测和回退逐字兼容；无 AST 可用时（非 Go 项目），`FromChangedPaths` 是默认值 |
| 依赖影响 | 零添加（仅 stdlib `go/ast` + `go/parser`） |
| 风险 | 中等（AST 解析边界需要额外注意；跨包图是 v2 范围） |

### 方向 4：生产级并行编排安全网（P2）

**为什么需要**
- `RunParallel` 和 `runWave` 存在，但缺少 4 个生产特性：per-wave 超时、资源感知调度、wave 级重试、渐进降级
- 资源感知调度是最紧迫的：goroutine 无界可能导致 OOM（`runWave` 中简单的 `semaphore chan struct{}` 是 ~50 行）

**核心挑战**
- **不是重新发明**：core mutex 缺口已不存在（`runBudget` 已经有 `mu sync.Mutex`，所有路径都加锁）。这是优化，不是安全缺口。
- **波级重试 vs 波级降压**：在瞬态故障上重试 vs 在系统性故障上跳过 wave。这两种情况需要不同的信号——一种简单的重试模式可能会掩盖退化。
- **资源感知调度**：需要 `resource.Profile`（总 goroutine/内存/API 调用预算）——但 `forge-core` 在无沙箱的情况下无法强制执行 OS 级资源隔离。预算应该是对**编排级别的 gate** 的一个提示（如果在预算内，不要启动新的 wave），而不是一个 cgroup。
- **渐进降级**：当 gate 失败时，降级到较低的保真度（例如 Haiku 而不是 Opus）——但降级与 production lifecycle floor（强制 Opus for reviewer）冲突。降级在第一波失败后必须提高，而不是降低。

**预期架构变更**
- `internal/orchestrator/parallel.go`：添加 `semaphore chan struct{}` 用于并发上限 + `context.WithTimeout` 用于 per-wave 超时
- 新增包：`internal/resource/`（轮廓预算+波级准入控制）
- `cmd/forge/cost.go`：添加与 `OnWaveResult` hook 的集成，用于重试/降级决策
- 所有更改向后兼容：`RunSequential`（默认）不受影响

**影响评估**
| 元数据 | 值 |
|---|---|
| 代码变更估计 | ~400 行 Go |
| 涉及包 | 2 修改 |
| 向后兼容性 | 完全向后兼容；`RunSequential` 是默认路径 |
| 依赖影响 | 零添加 |
| 风险 | 低（更改是增量附加的，无 core 重构） |

### 方向 5：自适应循环组装（P2）

**为什么需要**
- 产品价值最高——动态 workflow assembly 是 AI-native 软件工厂独有的。静态 workflow 是每个 CI 工具都有的
- `forge detect` 输出包含丰富的 `projectProfile` 信号（语言、生命周期、hasTests、hasCI），但零处消费
- `Phase.DependsOn`、`Phase.Emits`、`StopCondition` 组合子已声明就绪，但零处用于动态装配——元数据就绪但引擎未连接

**核心挑战**
- **不是写 phase 选择器**（60% 已有的：`internal/mode` 已有 `Effective()` 根据 mode×lifecycle 决策。缺口是 `internal/composer` 包用于 profile→phase 映射表）
- **StopCondition 组合子是真正的设计问题**：从 3 个枚举值（`conjunction`/`disjunction`/`external`）到动态组合表达式（根据差距类型从知识引擎生成的复合条件）。这是 StopCondition 的组合子 DSL，不是简单的查找表
- **与方向 3 的依赖关系**：自适应循环需要差距信号来知道要添加/删除哪些 phase。知识引擎（方向 1）提供差距信号。这两个方向必须一起设计
- **组合子统一**：Phase 组装和 StopCondition 生成是同一个设计问题——声明式执行策略的组合和嵌套。如果分开处理，将会做两次设计

**预期架构变更**
- 新增包：`internal/composer/`（`PhaseSelector` + `StopConditionGenerator`，统一设计）
- `internal/detect/detect.go`：将 `projectProfile` 连接到 composer
- 对 `internal/orchestrator/` 的更改以消费动态生成的 phase 列表（而不是硬编码的 JSON）
- 对 `internal/asset/asset.go` 的更改：phase 组合的 JSON schema 扩展
- **主要设计工作**：Composer DSL 设计（不是~600 行；是新的类型系统）

**影响评估**
| 元数据 | 值 |
|---|---|
| 代码变更估计 | ~1800 行 Go（包括设计工作） |
| 涉及包 | 3 新增/修改 |
| 向后兼容性 | 必须仔细执行；静态 YAML 应保持与动态 composer 输出同等效力 |
| 依赖影响 | 零添加 |
| 风险 | 高（如果 composer DSL 与 StopCondition 组合子未统一设计，会有重复设计的风险） |

---

## 3. 接口设计建议

### 3.1 关键模块接口原则

**原则 1：pure-domain 函数优于带副作用的接口**
当前 `internal/memory` 是正确的模型：
```go
// 好的现有模式：
type Entry struct { ... }
func Load(path string) ([]Entry, error)     // IO 包装器
func Query(entries []Entry, kind string) []Entry   // 纯函数
func Append(path string, e Entry) error      // IO 包装器
```
- `Query` (pure) 和 `Load` (IO) 是分开的，因此筛选逻辑可以在没有磁盘的情况下进行单元测试
- `retrieve.go` 也是如此：`Retrieve(docs, query, k)` 是纯函数，零 IO
- 这个模式应该扩展到所有新包

**原则 2：显式 io 边界优于隐式（端口/适配器模式）**
- 在零依赖核心中，接口应该很小并且特定于 forge-core，不依赖于第三方
- 示例：知识引擎的 `Harvester` 接口应该接受 `trace.Event` 和 `Entry`，而不是原始字节
- 不要通用的 `ReadWriter` 接口；写领域特定的
- `trace.Sink`（`Observe` 回调）是正确的模式——一个回调式钩子，引擎调用但未实现细节

**原则 3：显式声明不变量优于隐式假设**
当前实践：
```go
// good: asset.go 记录为什么是 fault-tolerant
// good: 注释解释 on_fail 的 nil 指针就是无 loop-back
// good: 诚实边界评论（"非语义"、"仅提供商"）
```
政策应该扩展到新代码：任何包导出的公开函数都应该有记录边界的 doc 注释（fail-closed？zero-value？empty input？）

### 3.2 新的抽象层

**Composer 抽象**（方向 5 所必需的）
```
internal/composer/
    selector.go      // PhaseSelector: Profile → []Phase
    combinator.go    // StopConditionGenerator: Profile × Knowledge → StopCondition
    composer.go      // Composer: 统一编排器（selector + combinator 调用者）
    selector_test.go
    combinator_test.go
```
- `PhaseSelector` 应该实现与 `asset.LoadWorkflowJSON` 相同的接口：返回 `[]Phase`——这使得静态 YAML 和动态选择器可以在引擎看来等效
- `StopConditionGenerator` 应该接受 `[]memory.Entry`（差距历史）+ `Profile` 并返回 `StopCondition`

**知识管道抽象**（方向 1 所必需的）
```
internal/knowledge/
    harvest.go       // Harvester: trace.Event → memory.Entry
    wiring.go         // Wiring: memory.Load → prompt 注入
    decay.go          // Pruner: 基于 TTL 的内存修剪
```
- `Harvester` 应该是引擎的一个 `Observe` 钩子——不是阻塞的，而是附加到 `RunFrom` 循环中
- 输入的初始知识注入应该在 `Discover` 阶段，而不是在每个 phase 之前

### 3.3 向后兼容性策略

**将新结构体字段作为 `interface{}` / optional nil pointer**
这是现有的 `asset.go` 模式：
```go
// good: ModelTier 是 string（"" 表示无配置）
// good: OnFail 是 *OnFail（nil 表示无 loop-back）
// good: Loop 是 *LoopBody（nil 表示不是 loop workflow）
```
所有新字段都应该沿用这个模式——零值必须意味着「与之前行为相同」

**通过检测渐进式采用**
```go
// 从方向 3 开始现有模式：
func FromChangedPaths(paths []string) Risk {
    // 如果 go/ast 可用和分析，使用它。否则回退到路径级。
}
```
新的方向应该使用这个模式：如果新分析没有覆盖（例如，跨包 AST → v2），则回退到保守的路径级默认值

**Gate 并行（并行 vs 串行接口）**
- `RunFrom`（串行，今天的默认值）应保持不变，不受新并行特性的影响
- `RunParallel` 应保持一个单独的代码路径（已经存在），新的特性（semaphore，per-wave-ctx）只能添加在那里
- 这意味着 `RunFrom` 继续是简单的 for 循环；并行特性不会为串行路径增加复杂性

---

## 4. 技术选型

### 4.1 新的技术栈/框架

| 候选 | 对于 | 决定 | 理由 |
|---|---|---|---|
| Go YAML 库（`gopkg.in/yaml.v3`） | 替换 python yaml2json shim | **反对（v2）** | 破坏零依赖政策；当前的 python shim 是诚实的脚手架，有已知的边界（裸 `-`，block scalar），并且在 CI 中使用 `python3` 已经存在 |
| `golang.org/x/tools/go/callgraph` | 跨包影响分析（方向 3） | **反对（v1）** | 零依赖红线；将 v1 的 AST 分析限制在包内。如果直接支持跨包调用图是必须的，重新审视 v2 |
| 向量 embedding 模型 + 数据库 | 语义 knowledge 检索（方向 1 v2） | **推迟到 v3** | 当前 TF-IDF（`retrieve.go`）是 stdlib pure function，零依赖。Embedding 模型需要外部二进制文件或 HTTP 调用——这属于 `forge-ai` Python 层 |
| Firecracker VM（沙箱） | 生产级隔离 | **推迟到 v3** | ROADMAP.md:v3 标签。防火墙二进制文件是 forge-core 零外部依赖无法使用的。沙箱 API 应该是 `Sandbox` 接口（方向 4） |
| Prometheus + Grafana | 生产 observability | **反对（现在）** | 当前 JSONL trace + scorecard 模式对于编排级别的指标来说已经足够。生产可观察性是 v3。 |
| LiteLLM | 交叉提供商模型池 | **推迟到 v3** | ROADMAP.md:v3。供应商特定逻辑（例如 claude `--output-format json`）在 `cmd/forge` 中是诚实处理的。 |

### 4.2 第三方依赖评估标准

项目已经有一种不言而喻的评估模式：

1. **标准库优先**：`go/ast` + `go/parser` > 外部 AST 库。`encoding/json` > YAML 库。
2. **如果是外部依赖，必须证明其合理性**：建立成本 + 与零依赖哲学的冲突 + 采购与自建。
3. **诚实边界标记**：每个选择如何影响「honesty」——shim 必须诚实说明自己的限制。
4. **所有权风险**：依赖是否处于活跃维护状态？对于 go 标准库一直是「yes」。对于更大的第三方生态系统——不保证。

对于新的方向：

#### 4.2.1 `gopkg.in/yaml.v3` 作为唯一依赖

**理由**：替换 python shim；消除 CI 中的 Python 运行时依赖；修复 YAML 语义差异。

**代价**：在 `go.mod` 中引入第一个外部依赖；增加 CI 步骤（`go mod download`）；在审查中增加信任负担。

**建议**：推迟到 v2，直到有可见的采用痛点。目前，yaml2json 的差异已经被记录并且理解良好（Sprint 27 修复了 block scalar 和序列项缺口）。项目自己的 YAML 文件在这些修复之后行为匹配。当外部项目带来真正的解析问题时，再重新考虑。

#### 4.2.2 `golang.org/x/tools` 用于调用图

**理由**：真正的跨包调用分析；零运行时依赖（构建时工具）。

**代价**：`go.mod` 直接更新（`go get golang.org/x/tools`）；在 `internal/impact/` 中增加复杂性。

**建议**：v1 为自己构建包内分析（`go/ast` 仅 stdlib）。跨包图是增量使用 `x/tools`，而不是根本性重写。项目已经在「限制范围内激进」——这种限制应该被明确接受，而不是偷偷引入。

### 4.3 自建 vs 采购的决策框架

| 决策 | 自建 | 采购 | 框架 |
|---|---|---|---|
| PolicyStack 继承 | ✅ 自建 | ❌ 不采购 | 领域核心逻辑；零外部依赖政策排除外部 policy 引擎 |
| Phase composer DSL | ✅ 自建 | ❌ 不采购 | 领域特定（forge-core 概念不存在于外部库中） |
| TF-IDF 检索 | ✅ 自建（完成） | ❌ 不采购 | stdlib pure function；外部库增加零价值 |
| 沙箱执行 | ⚠️ 接口自建 v2；Firecracker v3 | 🌐 采购 v3 | Firecracker 已经是经过实战检验的隔离层。ForgeOS 提供编排，而不是重新实现 VM |
| 跨 CPU 推理路由 | ❌ 不自建 | 🌐 采购 v3（LiteLLM） | 供应商定价+降级+速率限制是非平凡的；LiteLLM 的生态位就是解决这个问题 |

---

## 5. 实施路线图

### 5.1 优先级（按价值/成本 × 前提条件排序）

| 级别 | 方向 | 价值主张 | 估计 | 前置依赖 |
|---|---|---|---|---|
| **P0** | 方向 1 知识引擎 | 花费 2 周完成 ARCHITECTURE.md 的 10 引擎愿景 | 1.5-2 周 | 无（TF-IDF 已就位） |
| **P1** | 方向 5 影响分析 + 智能门控 | 成本降低 70% 的承诺；`Score()`已就位 | 3-4 周 | P0 方向 1（校准需要历史差距知识） |
| **P1** | 方向 2 联邦治理 | 解锁组织采用；checkpoint 崩溃是当前风险 | 3-4 周 | 无（checkpoint 隔离可以先行） |
| **P2** | 方向 4 并行安全网 | 生产就绪并行，但核心 mutex 问题已解决 | 1-2 周 | 无 |
| **P2** | 方向 3 自适应循环 | 最高产品价值，但实现成本最高 | 6 周 | P0 方向 1（差距信号用于 phase 选择） |

### 5.2 阶段划分

**阶段 1（P0）：知识引擎就绪——6 月初**
里程碑：
- `internal/knowledge/` 包已交付（harvest、wiring、decay）
- `trace.SpanID` + `ParentSpanID` 字段已添加（所有后续方向的基础设施）
- LoopEngine 在每次迭代时自动调用 `memory.Load` → prompt 注入
- Memory Prune 按 TTL（默认 30 天）运行
- 检查点：在 `forge evolve` 循环中，knowledge 注入显示差异（agent 不再重复发现已知差距）

**阶段 2（P1）：多管理员风格 + 影响分析——6 月中旬**
并行：
- **方向 5 影响分析 v1**：`internal/impact/` 包含包内 AST 分析 + 门控选择器
- **方向 2 checkpoint 隔离**：`internal/persist/` 包含 per-subproject checkpoint 命名空间
检查点：monorepo 子项目可以独立存在而不会互相覆盖 checkpoint；`forge route --diff-files` 通过 AST 检测增强

**阶段 3（P1→P2）：联邦政策 + 并行安全网——6 月下旬**
- **方向 2 PolicyStack**：`internal/policy/` 包含 policy DAG 解析 + `extends` 消费
- **方向 4 并行 backstop**：`semaphore` + per-wave timeout + wave-level retry
检查点：组织可以采用跨 repos 的共享政策；并行模式在生产规模上运行

**阶段 4（P2）：自适应循环——7 月**
- **方向 3 composer**：`internal/composer/` 包含 `PhaseSelector` + `StopConditionGenerator`，统一设计
- composer DSL 与 direction 1 的差距类型对齐
检查点：`forge detect` 输出触发动态 phase 排序；`forge build` 在 Go 项目上跳过非 Go 阶段

### 5.3 风险与缓解

| 风险 | 可能性 | 影响 | 缓解 |
|---|---|---|---|
|---|:---:|---|---|
| **方向 3 + 方向 1 设计耦合** | 高 | 如果分开设计会产生重复设计 | 将 composer 和 stop_condition_generator 放在同一个 `internal/composer/` 包中。要求 ADR。使用同一组差距类型 |
| **方向 5 AST 边界限制** | 中等 | 跨包调用图 v1 不受支持 | 明确记录 v1 范围（包内）。为 v2+ 留下 `callgraph` 接口占位符 |
| **方向 2 policy 解析变为图论问题** | 低-中等 | 循环继承使解析器崩溃 | `PolicyStack` 的构建器应检测循环（map[path]bool 遍历集）。fail-closed：如果有循环则拒绝 |
| **知识注入膨胀 prompt 窗口** | 低 | `memory.Load` 在 project 存在时间较长时会返回 1000+ 条目 | Retrieve 已经 top-K。设置硬数量上限（例如 top-10）。prompt builder 应在注入之前检查加上 knowledge 后的总长度 |
| **方向 4 降级与 production lifecycle floor 冲突** | 中等 | 降级将 reviewer 设为 Haiku，但 production floor 要求 Opus | 降级函数必须显式排除 production lifecycle。floor 永远是高，不因降级而低 |
| **方向 1 span_id 添加到 trace 破坏 parse 兼容性** | 低 | 先前 checkpoint/trace 文件具有不同的 schema | `trace.Event` 中的 `SpanID` 是空字符串，向后兼容。parse 代码沿用 json.Unmarshal fault-tolerant |
| **方向 2+ 的工程师带宽瓶颈** | 高 | 所有方向竞争相同的 2 个 Go 工程师 | 利用 parallel sprint 执行（Sprint 24+ 已经证明可以工作）。使用 fresh-context reviewer 进行质量控制 |

### 5.4 资源分配策略

```
    六月第 1 周  六月第 2 周  六月第 3 周  六月第 4 周  七月
P0:  ████████░░  ░░░░░░░░░░  ░░░░░░░░░░  ░░░░░░░░░░  ░░
     方向 1 知识引擎
     1 个工程师

P1:  ░░░░░░░░░░  ████████░░  ████████░░  ░░░░░░░░░░  ░░
     方向 5 影响分析 ▲ 方向 2 checkpoint+policy
     2 个工程师并行

P2:  ░░░░░░░░░░  ░░░░░░░░░░  ░░████████  ████████░░  ██
     方向 4 并行 backstop + 方向 3 composer
     1-2 个工程师
```

### 5.5 退出标准

对于每个方向，验收标准应该是：

**方向 1 知识引擎（P0）**
1. ✅ `forge evolve` 在迭代 N 时自动查询 memory，并注入顶级 K 个相关条目
2. ✅ 差距历史在 trace 中持久化（`SpanID` 链接到 memory 条目）
3. ✅ `decay.go` 在 N 个循环后清理过时的 memory 条目
4. ✅ `forge accept` 保持绿色（回归测试确认零行为变化）

**方向 5 影响分析（P1）**
1. ✅ `forge route --diff-files` 在 Go 源文件上运行 AST 分析（不仅仅是路径）
2. ✅ `determineGateSet` 在非 production mode 下产生裁剪后的 gate 列表
3. ✅ 知识引擎的差距历史影响旁路校准（旁路决策中的假阳性率指标）

**方向 2 联邦治理（P1）**
1. ✅ monorepo 子项目在共享 `.forge/` 下拥有隔离的 checkpoint
2. ✅ `project.yml:extends` 解析器加载父政策
3. ✅ `PolicyStack` 在具有 3 个级别继承的 fixture 上测试通过

**方向 4 并行 backstop（P2）**
1. ✅ `RunParallel` 有 `semaphore` 用于 goroutine 上限
2. ✅ per-wave `context.WithTimeout` 在 wave 长时间运行时间中止
3. ✅ wave-level 重试适用于瞬态故障（可由 fixture 配置）

**方向 3 自适应循环（P2）**
1. ✅ `forge detect` → `internal/composer` → `PhaseSelector` 根据 project 配置产生不同的 phase 列表
2. ✅ `StopConditionGenerator` 输出根据差距历史变化的条件
3. ✅ 向后兼容性：等同于原始 YAML workflow 的 profile 产生相同的 phase 序列

---

## 结论

在 v38 中进行的评审指出了 ForgeOS 在未来 6 周内正确的架构方向。最关键的认识是：

1. **知识引擎是解锁其他所有事情的唯一瓶颈。** TF-IDF 已就绪——唯一缺失的是两处焊接（收获和提示注入）。2 周，无风险，最高回报。

2. **方向 4 已经被 v38 的「四大缺口」过度恐慌所治愈。** 核心互斥锁问题已经解决，安全检查已经到位，剩下的只是优化——应该这样描述。

3. **方向 5 必须是知识引擎的搭档，而不是替代品。** 没有差距历史，影响分析无法校准。不要在方向 5 之前实现方向 5。

4. **统一设计获胜：** Composer（PhaseSelector）+ StopConditionGenerator 应该是同一个设计练习，而不是分开的。SpanID 基础设施对所有方向都有益。

5. **零依赖现在是竞争护城河，而不是技术债务。** 每个其他编排工具都可以引入无限的外部依赖。ForgeOS 在 18 个包中以零依赖超越了它们——这是一个关键的区别。

建议的 sprint 顺序：知识引擎（Sprint N）→ 影响分析 + 隔离 checkpoint（Sprint N+1）→ 政策堆栈 + 并行后援（Sprint N+2）→ 自适应循环（Sprint N+3-N+4）。五个方向、六个 sprint、全球可达。
