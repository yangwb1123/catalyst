现在我已全面了解该文档、项目架构格局和 31 个 sprint 的演进历史。以下是我的分析。

# 架构分析：五个未覆盖架构前沿

## 1. 架构评估

### 当前架构优势

ForgeOS 架构在其问题领域内设计得非同寻常。几个值得强调的根本性优势：

- **声明性治理中心**：`mode×lifecycle` 中枢旋钮是优雅的架构产品——单一设置驱动 Router 档位、Harness 严格度和 Workflow 深度。31 个 sprint 中，工程团队始终抗拒范围蔓延，针对所有三个消费路径进行增量接线。
- **载重墙模式**：认识到"站在所有 CLI 之上"只能执行最弱宿主允许的内容，以及带外执法是真相之源，这是一个经过实战验证的扎实架构决策。它使 ForgeOS 对宿主限制具有弹性。
- **Fresh-Context Reviewer 纪律**：这是项目自己的 AGENTS.md 中最大胆的架构决定——保证 Reviewer 看不到前序输出的决策。实施方式（`verdictLedger`、`feedForward` 隔离、车道抑制）在实践中运行，并在 Sprint 27 的彻底审查中捕获了真实的阻断性 bug。
- **诚实分级**：诚实 N/A 模式、fail-safe 保守默认值、永远不伪造通过——这些价值观嵌入在代码库的架构 DNA 中。它们使工程团队能够信任监控与闸门。
- **自纠循环**：项目展现了第三次重构的非凡纪律——Sprint 29 中将 `gate_resolve.go` 移回 `internal/gate` 并回调 `package.max_files`，Sprint 30 和 31 中的架构自纠。这不是投机性的清理；它是对抗架构漂移的持续投资。

### 关键限制

**1. 零依赖约束已越过成本效益平衡点**

零依赖提供了真实的工程优势：无冲突、小体积、确定性的构建。但收益递减曲线已被越过——成本（手写 YAML 解析器 bug、无结构化日志、8 层手写锁序合约、零模糊测试）现在超过残余收益。关键问题是：*还有哪些部分真正需要零依赖？* 核心编排器、任务路由器和收敛引擎——是的，代码路径必须高度可靠。但 YAML 解析、结构化日志、CLI 框架——这些是基础设施功能，工程师的时间应投资于系统逻辑，而不是重新发明经过战斗检验的格式解析器。

**2. Prompt 装配中的信任域混合**

`buildPrompt` 函数将七个以上的不同信任来源（从人工审核的 AGENTS.md 到不受约束的 Agent 写入 memory 条目）拼接成一个单一的文本平面，没有任何结构分离。随着 ForgeOS 向更长的自主运行和更多的 Agent 写入内容（memory、ROADMAP ticks、feed-forward phases）发展，这个攻击面只会增长。当前的方法——`\n\n` 连接和一个剥离不可打印字符的 sanitizer——在系统信任边界内是不可接受的。

**3. 治理广度与治理深度**

闸门设计精良（8 次分层检查），但治理模型缺乏 N/A 覆盖随时间退化的系统性视图。项目有"允许的闸门"清单，但没有"期望的闸门"合约。"它为什么 N/A"的语义过于粗糙——将 `INAPPLICABLE`（该语言没有此概念）与 `NO_TOOL`（工具可以安装）混为一谈。加上检查结果中没有导致 FAIL 的衰减模型，意味着治理广度可能逐渐缩小而无人注意。

**4. 每个进程的架构限制**

ForgeOS 目前围绕"每个 forge 调用一个进程"模型构建。这产生了冷启动问题（每次 ~12-20 次文件 IO），阻止了跨调用缓存，并使得跨进程交叉引用（trace/memory/checkpoint）变得困难。路线图提到了守护进程模式，但那是 "v3"—同时，当前的每次进程模型产生了许多文件中确定的成本。

### 架构债务摘要

| 债务 | 位置 | 影响 | 严重性 |
|--------|----------|--------|----------|
| 手写 YAML 解析器 | `internal/yaml2json` | 已产生生产 bug（注入 `>` 前缀） | **高** |
| 无结构化日志 | `internal/orchestrator` | 无法在生产中监控 ForgeOS 本身 | **高** |
| 无 CLI 框架 | `cmd/forge` | flag 处理不一致 | 中 |
| 包级全局状态 | `memory`、`prompt` | 测试隔离问题，并发风险 | 中 |
| 8 层锁序合约 | 跨 4 个文件 | 无静态验证，死锁 Heisenbug 风险 | 中 |
| 零模糊测试 | 全仓 | 解析边界未覆盖 | **高** |
| 无跨存储交叉引用 | trace/memory/checkpoint | 无法进行取证分析 | 低（但增长中） |

---

## 2. 扩展方向

### 方向 A：Prompt 装配安全层（P1）

**为什么需要：**
Agent 写入的内容（memory、ROADMAP ticks、feed-forward 输出）作为 prompt 输入的比例正在增长，且只会加速。随着 ForgeOS 发展至 24 小时自主运行，注入攻击面从"理论上的可能性"转变为"真实风险"。在 `buildPrompt` 中实现结构性安全边界直接影响系统完整性——不是锦上添花。

**核心挑战：**

1. **结构分隔符设计**：LLM 在解释 XML 风格标签与 markdown 分区时存在差异。分隔符必须足够健壮，以防止 LLM 将数据误认为指令，同时不能显著膨胀 token 使用量。一个推荐的模式是 `<context-source type="memory" trust="low">`... `</context-source>`——它建立了清晰的层次结构，Claude 和 GPT 系列都能可靠地遵循。

2. **Per-Lane Token 预算**：每条 context lane（memory、feed-forward、findings）需要一个硬 cap，按模型 context window 的比例分配。AGENTS.md（硬约束）必须有保证的配额。剩余的按比例分配。如果 memory 超过预算，必须按相关性排序截断，而不是整体丢弃。

3. **输出验证**：在 prompt 发送到模型之前，必须验证：总长度 ≤ 模型 context window 的 80%，所有必要的 section 存在（AGENTS.md 约束、当前阶段描述、gate 裁决），结构分隔符是平衡的。如果验证失败，必须确定性地拒绝或截断——绝不静默传递。

4. **ROADMAP 读取消毒**：这是最活跃的注入向量（Agent 可以 tick `[x]`）。Gather 路径读取 ROADMAP 全文时，应将 Agenter 可修改的 ticking 部分包裹在结构分隔符中，并应用与 Agent 写入内容相同的 sanitizer。

**预期的架构变更：**

```
buildPrompt (当前)                    buildPrompt (目标)
    ↓                                    ↓
7+ append calls with \n\n           LaneRegistry (声明式)
                                         ├── high: AGENTS.md, gate裁决
                                         ├── medium: ADR, agent cards
                                         ├── low: memory, feed-forward
                                         └── untrusted: ROADMAP ticks
                                       每一段：结构标签 + 预算 + 验证
```

- 新增：`internal/prompt/lane.go`（LaneRegistry + LaneConfig）
- 新增：`internal/prompt/budget.go`（per-lane token 预算 + 截断策略）
- 新增：`internal/prompt/validate.go`（输出验证器）
- 修改：`cmd/forge/prompt_context.go`（消费 LaneRegistry 替代原始 append）

**对现有系统的影响：**
向后兼容性很高——新的隔离开关可以默认关闭（"plain mode"），在安全审计后切换为开启。重构可以将当前 `buildPrompt` 的所有 7+ lane 迁移到声明式 LaneRegistry，无需更改任何 call site。

---

### 方向 B：依赖合理化（P1）

**为什么需要：**
零依赖约束成本合理化的时机已经成熟。该文档正确识别了关键痛点——但要跨越边界，我们需要一个具体的*替换策略*，而不是笼统的"添加 go.mod 依赖"。YAML 解析器是明确的优先事项：它是唯一已经产生生产 bug 的零依赖决策。结构化日志紧随其后。

**核心挑战：**

1. **每个包的分析**：不是"零依赖与有依赖"的二元选择——而是逐包分析。`internal/gate`（零依赖）和 `internal/converge`（零依赖）保持优势。`internal/yaml2json` 和 `internal/orchestrator` 的日志记录是自然候选。

2. **版本策略**：首次 `go.mod require` 行设置了一个先例。添加 `gopkg.in/yaml.v3` 是安全的选择（成熟、API 稳定），但 ForgeOS 需要决定是否允许所有次要/补丁版本或固定特定版本。建议：对所有依赖使用 `v0` 风格的宽松约束（如 `v3.0`），这能最大化兼容性，同时避免未宣布的破坏性变更。

3. **结构化日志接口**：forgo 运行时当前使用 `func(string)` 作为唯一的日志抽象。引入结构化日志意味着定义一个项目特定的接口层：

```
type Logger interface {
    Info(msg string, fields ...Field)
    Warn(msg string, fields ...Field)
    Error(msg string, fields ...Field)
    With(fields ...Field) Logger  // 用于衍生上下文日志器
}
```

这应该直接放在 `internal/orchestrator` 或一个名为 `internal/log` 的新基础包中——零外部依赖，但提供语义级别的结构。*实现*可以是项目特定的（到 `slog`、`logrus` 或 `zap` 的桥梁），但类型保证来自标准库。

**预期的架构变更：**

- 新增：`go.mod` 中 `require gopkg.in/yaml.v3`
- 修改：`internal/yaml2json` 使用 `yaml.v3` 替换手写解析器（保留 normalize.go 以兼容当前行为，因为 sprint 27 的修复与 PyYAML 对齐——回归测试应验证）
- 新增：`internal/log/logger.go`（Logger 接口 + 上下文实现）
- 修改：`cmd/forge/main.go`、`internal/orchestrator/*.go` 从 `func(string)` 迁移到 `log.Logger`
- 考虑但建议推迟：CLI 框架（`cobra`/`urfave/cli`）——当前 forge-core 的 flag 处理不一致但功能正常，收益与风险比例较低

**对现有系统的影响：**

YAML 解析器替换需要零外部行为变更——回归测试应证明 `yaml2json` 对所有 7 个真实 workflow 文件的输出在替换前后一致。Logger 迁移会产生更大范围的代码变更，但接口提供清晰的向后兼容路径（适配器从现有 `func(string)` 回调到新 Logger）。

---

### 方向 C：Gate 覆盖完整性系统（P2）

**为什么需要：**
文档正确识别了"期望的闸门清单"概念——但我们可以走得更远。这不是二阶漂移风险；这是一个真正的治理缺口，随着项目扩展到 50+ forge-managed 仓库，将会被持续放大。目前无法回答"我的覆盖范围在缩小吗？"这个问题。

**核心挑战：**

1. **期望的闸门清单**：在 `project.yml` 中声明 `expected_gates`——但这绝对不应是重复的声明。最佳模式：在 `policy.yml` 或 `modes.yml` 中建模为一个单独的 `gates_required` 键，明确区分"允许运行这些闸门"（当前 `Gates`）与"期望这些闸门产生信号"（新 `ExpectedGates`）。当期望的闸门报告 N/A 时，它将输出警告（engineering 模式）或 FAIL（production 模式）。

2. **N/A 分类细化**：当前 `Inapplicable` 枚举语义太广。建议三类：
   - `LANGUAGE_INAPPLICABLE`：该语言没有此概念（Python 项目中无 `go test`——永久 N/A）
   - `TOOL_MISSING`：工具可以安装但缺失（`eslint` 未安装——可修复的 N/A）
   - `TOOL_CONFIG_GONE`：工具已安装但配置被删除（`eslint` 可用但无 `.eslintrc.js`——可修复的 N/A，通常意味着退化）
   
   `TOOL_MISSING` 和 `TOOL_CONFIG_GONE` 应在 production 模式下触发警告或 FAIL。

3. **N/A 趋势追踪**：`forge accept` 应记录每次运行的 N/A 计数和类别。当计数从基线增长时（"之前 1 个 N/A，现在 3 个"），输出可操作的警告。这不需要新的持久化——trace.jsonl 已经包含 gate 裁决；添加时间感知的聚合查询即可。

**预期的架构变更：**

- 修改：`internal/gate/resolve.go` 细化 `NAReason` 枚举（三值替代当前单一 N/A）
- 修改：`internal/mode/mode.go` 增加 `ExpectedGates` 字段
- 新增：`internal/gate/trend.go`（跨运行 N/A 趋势检测器，从 trace 读取）
- 最小接口变更——GateProof 已包含 `Category`；只细化语义

**对现有系统的影响：**
对现有 gate 行为完全向后兼容——当前 N/A 门在详细模式外不可见，并免于收敛。新的分类和趋势是附加层。

---

### 方向 D：启动缓存层（P2）

**为什么需要：**
文档正确识别了每次 forge 调用的 ~12-20 次文件 IO。在 CI 场景中（每次提交触发 `forge accept`），重复解析成本显著。在赋能 CI 场景和多项目管理之前，简单的文件系统缓存提供高杠杆增益。

**核心挑战：**

1. **缓存失效正确性**：缓存层必须对 mtime 变化做出正确反应。Workflow YAML 更改→缓存失效。ADR 目录更改→缓存失效。关键是不变量：*陈旧缓存不得导致行为变更*。以确定性方式实现这一点意味着缓存键是（路径，mtime，size）的元组。

2. **无守护进程的持久化**：文档建议 `.forge/workflow_cache.json`——这很好，但需要处理并发（两个 forge 进程写入同一缓存）。乐观锁定（写入时比较 mtime）和原子重命名（写入临时文件 → `os.Rename`）足以处理非竞争场景。

3. **缓存粒度**：问题不是"缓存所有内容"，而是"什么变化最快，什么值得缓存"：
   - 工作流 YAML：几乎从不变化（在 evolve 运行期间）→ 缓存高价值
   - modes.yml：很少变化 → 缓存高价值
   - 路由 policy：很少变化 → 缓存高价值
   - ADR 标题：偶尔变化（新 ADR）→ 缓存中等价值，mtime 检查
   - AGENTS.md/agent cards：偶尔变化 → 缓存中等价值，mtime 检查

**预期的架构变更：**

- 新增：`internal/cache/disk.go`（mtime 检查的 JSON 持久化缓存）
- 修改：`cmd/forge/main.go` 加载路径——尝试缓存查询，仅在 mtime 变化时回退到完整解析
- 新增：`internal/cache/context.go`（用于 ADR/AGENTS/agent card 的预计算项目上下文缓存）

**对现有系统的影响：**
缓存层必须是完全透明的——如果缓存丢失或损坏，系统应以与从未缓存相同的方式运行。接口：`cache.Get(key, validator func(mtime) bool, builder func() interface{})`——builder 仅在缓存未命中时调用。

---

### 方向 E：三存储交叉引用系统（P3）

**为什么需要：**
Trace、memory 和 checkpoint 存储功能完整，但缺乏关联查询能力。这使得取证调查变得困难（"哪个 checkpoint 对应这个 gate FAIL？"），并抑制了回放和审计分析的新用例。

**核心挑战：**

1. **RunID 分配和注入**：每个 `forge run`/`forge evolve` 调用需要分配一个全局唯一 ID（ULID 或 UUIDv7），该 ID 在整个进程生命周期内保持不变。这个 ID 必须在创建时注入到所有三个存储系统——trace 每行事件、memory 每个条目、每个 checkpoint。

2. **Checkpoint 跟踪最后 trace seq**：Checkpoint 需要记录 `LastTraceSeq` 和 `TraceLineOffset`，以便恢复后，系统能精确知道 checkpoint 对应 trace 中的哪个区间。

3. **Memory 条目溯源**：Memory 条目应记录 `RunID`、`Iteration`（如果适用）以及 `TraceSeq`（可选的，在 compaction 时可为空）。这使知识能够回溯到产生它的运行。

4. **查询接口**：`forge trace query` 子命令提供跨存储搜索——"查找在 gate=test FAIL 的迭代期间添加的所有 memory 条目"。这应该是轻量级的（文件扫描 + 过滤），不需要专门的数据库。

**预期的架构变更：**

- 新增：`internal/trace/runid.go`（ULID 生成 + 注入到所有存储）
- 修改：`internal/trace/trace.go`：携带 RunID
- 修改：`internal/memory/memory.go`：`Entry` 加 RunID、Iteration、TraceSeq
- 修改：`internal/persist/checkpoint.go`：加 LastTraceSeq、TraceLineOffset
- 新增：`cmd/forge/trace_query.go`：跨存储查询命令

**对现有系统的影响：**
现有 trace、memory 和 checkpoint 文件与新的交叉引用字段向后兼容——旧文件根本没有这些字段，查询时优雅降级。

---

## 3. 接口设计建议

### 关键模块接口原则

**1. 最小依赖方向**

ForgeOS 架构遵循 `interfaces → application → domain` 的规则。新的接口应尊重这一规则：

```
// 好——domain 包定义接口，不依赖导入者
// internal/converge/signals.go
type SignalGatherer interface {
    GatherSignals(ctx context.Context, wf *asset.Workflow, state *RunState) (*Signals, error)
}
```

这使实现者（`cmd/forge`、harness）反转依赖——domain 不导入 CLI 或适配器逻辑。

**2. 每个包一个关注点**

当前模式（每个 Go 包一个目录，出口点是接口+实现）运作良好。任何新包应遵循：

```
internal/log/          — Logger 接口/实现
internal/cache/        — 磁盘缓存抽象
internal/prompt/       — 装配、lane 注册、预算、验证
```

**3. 首选选项模式，而非重载参数**

`buildPrompt` 当前接受一个原始字符串切片。新的 LaneRegistry 应使用选项模式：

```go
type PromptOption func(*LaneRegistry)

func WithMemory(memoryEntries []memory.Entry) PromptOption {
    return func(r *LaneRegistry) {
        r.AddLane(LaneConfig{
            Name:      "memory",
            TrustLevel: TrustLow,
            Budget:     TokenBudget{Max: 4000, Strategy: TruncateByRelevance},
            Content:    formatMemory(memoryEntries),
        })
    }
}
```

这使 `buildPrompt` 调用者通过传递选项来声明其意图，远离参数排序错误的 bug。

### 需要新的抽象层

**1. 项目缓存层（`internal/cache`）**

一个核心包，提供跨 Go 进程的 mtime 感知 JSON 磁盘缓存。

```
// internal/cache/cache.go
type Cache struct {
    root string  // .forge/ 目录
}

func (c *Cache) GetOrBuild(key string, deps []string, ttl time.Duration, builder func() ([]byte, error)) ([]byte, error)
```

`deps` 参数接受文件路径——缓存检查每个文件的 mtime，如果任何文件变化则调用 builder。

**2. Lane Registry（`internal/prompt/lane.go`）**

替换 `buildPrompt` 的当前原始 append 方法：

```go
type TrustLevel int
const (
    TrustHigh TrustLevel = iota       // 人工审核，长期稳定
    TrustMedium                       // 机器生成，经过验证
    TrustLow                          // Agent 写入，已知注入面
    TrustUntrusted                    // Agent 可修改，无 sanitizer（当前 ROADMAP ticks）
)

type LaneConfig struct {
    Name       string
    Trust      TrustLevel
    Budget     TokenBudget
    Content    string
}

type LaneRegistry struct {
    lanes []LaneConfig
}

func (r *LaneRegistry) AddLane(cfg LaneConfig) { ... }
func (r *LaneRegistry) Assemble(ctx context.Context, model ModelInfo) (string, error) { ... }
```

`Assemble` 方法是执行所有结构分隔、预算截断和输出验证的地方——赋予 `buildPrompt` 一个单一的 exit 点，用于经过验证的结构化 prompt。

**3. 结构化日志接口（`internal/log`）**

```go
type Logger interface {
    Info(msg string, fields ...Field)
    Warn(msg string, fields ...Field)
    Error(msg string, fields ...Field)
    With(fields ...Field) Logger
}

type Field struct {
    Key   string
    Value interface{}
}

// StdLogger 是当前 func(string) 的适配器
type StdLogger struct {
    fn func(string)
}
```

### 保持向后兼容性

| 变更 | 兼容性策略 |
|--------|-----------|
| YAML 解析器替换 | 并行实现，回归测试验证所有现有 YAML 的输出逐位一致 |
| 结构化日志 | `func(string)` 回调可作为 `Logger` 适配器的后端适配——零调用者变更 |
| Lane Registry | 对调用者隐藏——`buildPrompt` 继续导出 `func(...) []string`，内部构建 LaneRegistry |
| Gate 分类细化 | `exemptNA` 返回 `NAReason` 枚举作为 `N/A` 的追加字段——`GatesGreen` 保持 N/A 豁免 |
| 存储交叉引用 | 新字段可选——旧文件在缺少字段时优雅处理，查询不报错 |
| 启动缓存 | builder Must 在缓存丢失时回退——未检测到行为差异 |

---

## 4. 技术选型

### 依赖引入决策矩阵

| 功能 | 候选库 | 理由 | 风险 | 决策 |
|----------|-----------|---------|------|--------|
| YAML 解析 | `gopkg.in/yaml.v3` | 事实上的标准，Go 社区经过战斗检验 | 低——API 稳定，无破坏性变更 | **推荐：准入** |
| 结构化日志 | `log/slog`（Go 1.21+） | 标准库，零外部依赖 | 低——标准库，Go 版本门槛 1.21 | **推荐：立即采用** |
| CLI 框架 | `spf13/cobra` | ForgeOS CLI 正在增长（17+ 子命令）并且 `--help`/flag 一致性让工程团队付出代价 | 中——当前 `flag.NewFlagSet` 模式虽然不优雅但功能正常；迁移需要大量测试 | **建议推迟到 v3** |
| UUID 生成 | `google/uuid` 或 `oklog/ulid` | 用于 RunID/SessionID 注入 | 低——小型库，API 稳定 | **推荐：准入** |
| 死锁检测 | Go 的 `-race` 检测器 | 已经在 `go test -race` 中使用 | 低——已使用 | **已在用——扩大覆盖** |

### 自建 vs. 采购决策框架

YAML 解析和 UUID 生成等明显案例（清晰地支持采购已成熟的库）之外，以下是为 ForgeOS 建立决策框架的方法：

**首选自建情况：**
- 核心领域逻辑，其中接口稳定性通过实现确定性驱动（`internal/converge`、`internal/gate`）
- 跨包契约，其中外部依赖会创建循环导入（`internal/asset` 类型）
- 宿主适应性，其中行为因操作系统或宿主 CLI 而异（prompt 构建、参数组装）

**首选采购情况：**
- 格式解析，其中社区已经解决了边缘情况（YAML、JSON、语义化版本）
- 日志框架，其中性能、轮转和结构化查询是经过解决的需求
- CLI 框架，其中 flag 解析、帮助文本自动生成和补全生成都是非差异化功能

**边界情况（自建带采购证据）：**
- ForgeOS 自身的 trace 格式（JSONL）不需要解析器——标准 `encoding/json` 可以处理
- Memory 序列化相同——`json.Marshal`/`Unmarshal` 足够
- 锁排序检测：Go 的 `-race` 检测器捕获竞争条件，但不会验证锁排序。一个专用的锁排序验证器最好自建（作为内部检测工具），因为外部库无法理解 ForgeOS 特定包依赖结构

### 推荐的依赖准入检查清单

在向 `go.mod` 添加第一个非标准库依赖之前，应建立：
1. **仅限运行时包**——无用于测试的依赖（必须使用 `_test.go` 模式）
2. **许可证兼容性**——仅 MIT/BSD/Apache 2.0
3. **API 稳定性**——首选 `v1` / `v3`+ stable 库（无 `v0.x`）
4. **版本固定**——添加的依赖应使用宽松的次要版本约束（`v3.0`），并在推送到 Go 模块代理时使用 `go.sum` 完整性
5. **审计跟踪**——每个新的 `go.mod require` 行应由 ADR 或政策文件记录，说明为什么选择此依赖而不是自建

---

## 5. 实施路线图

### 优先级合理化

文件将 5 个方向的优先级定为 P1/P1/P2/P2/P3。我同意这个排序，但有以下精炼意见：

| # | 方向 | 优先级 | 理由 |
|---|----------|----------|---------|
| 1 | Prompt 装配安全（方向二） | **P0** | 信任域安全是一个*正确的* P1——当 LLM 吞吐量从几个 prompt/会话增长到数千时，注入面会成倍增加。在 sprint 27 交付之前，这也是可操作的：边界隔离和 per-lane 预算不需要外部依赖，只需要重构。 |
| 2 | 依赖合理化——YAML 解析器（方向一的子集） | **P1** | 已经产生了生产 bug。Sprint 27 的 block-scalar bug 是一类错误，如果使用 yaml.v3 不可能会发生。低风险、高回报。 |
| 3 | N/A 覆盖完整性（方向三） | **P1** | 随着 forge-managed 仓库的增长，此问题会成倍放大。期望的闸门合约和细化的 N/A 分类可以在 ~1 sprint 内交付，并解除监控空白。 |
| 4 | 依赖合理化——结构化日志（方向一的子集） | **P2** | 重要但范围大。YAML 解析器修复修复了已得到证实的 bug；日志重构将触及 15+ 文件并且需要 careful 的回调迁移。 |
| 5 | 启动缓存（方向四） | **P2** | 高开发者体验收益，但仅在 CI/多项目管理是首要任务时才被阻止。如果没有缓存层，devex 在 ~10-15 个仓库时才会成为瓶颈，而不是现在。 |
| 6 | 三存储交叉引用（方向五） | **P3** | 正确的优先级。在达到稳定的运行节奏和监控到位之前，取证分析不会增加足够价值来证明紧急优先。 |
| 7 | 依赖合理化——CLI 框架（方向一的子集） | **P3** | 推迟至 v3。当前 CLI 虽然不优雅但功能正常，且没有因 flag 处理不正确而产生生产问题。 |

### 精炼优先级矩阵

```
                 影响高
                   │
     P1: YAML    P0: Prompt 安全
     解析器       边界隔离
       │              │
       └──────────────┘
        │              │
     P2: 启动缓存  P2: 结构化日志
     P2: N/A 覆盖
        │              │
     P3: 交叉引用  P3: CLI 框架
        │              │
                  影响低
```

### 阶段划分

**阶段 A（Sprint 32）——核心安全 + 依赖合理化**

里程碑：P0 prompt 安全 + P1 YAML 解析器替换，两者均完全集成并通过 `forge accept` 验证。

| 步骤 | 描述 | 工作估算 |
|------|-------------|-----------|
| A1 | 设计 LaneRegistry 接口 + 信任域分类 | 1 人·天 |
| A2 | 实现 per-lane token 预算 + 结构分隔符 | 2 人·天 |
| A3 | 迁移 `buildPrompt` 的 7+ lane 到 LaneRegistry | 2 人·天 |
| A4 | 添加输出验证（总长度、必选 section） | 1 人·天 |
| A5 | 添加 ROADMAP Gather 消毒 | 1 人·天 |
| A6 | 实现 yaml.v3 替换并行测试（7 个真实文件 + 边缘情况） | 2 人·天 |
| A7 | 回归测试和 `forge accept` 验证 | 1 人·天 |
| **总估算** | | **~10 人·天** |

风险：A6 需要验证零行为差异。这包括手写解析器当前的 7 个 YAML 文件输出与 yaml.v3 输出之间的逐字节比较。任何差异都要进行审查和记录。

**阶段 B（Sprint 33）——治理完整性 + 启动缓存**

里程碑：N/A 覆盖趋势 + 期望的闸门合约 + 工作流缓存，全部由 `forge accept` 执守。

| 步骤 | 描述 | 工作估算 |
|------|-------------|-----------|
| B1 | 在 mode.Policy 中定义 `NAReason` 枚举 + `ExpectedGates` | 1 人·天 |
| B2 | 实现 `resolveNA` 三值分类 | 2 人·天 |
| B3 | 添加 `acceptance.mjs` N/A 趋势报告 | 2 人·天 |
| B4 | 在 `internal/cache` 中实现磁盘缓存层 | 2 人·天 |
| B5 | 迁移工作流 + modes.yml + ADR 标题加载到缓存感知路径 | 2 人·天 |
| B6 | 回归测试 + 场景验证（缓存丢失、陈旧缓存、并发写入） | 2 人·天 |
| **总估算** | | **~11 人·天** |

风险：B6 中的并发场景可能是微妙的——两个 forge 进程写入同一个 `.forge/`。测试必须覆盖竞态条件下的无损坏保证。

**阶段 C（Sprint 34）——可观测性 + 结构化日志**

里程碑：跨存储 RunID + 结构化日志，通过 go vet/-race 验证，由 trace 测试覆盖。

| 步骤 | 描述 | 工作估算 |
|------|-------------|-----------|
| C1 | 在 `internal/log` 中定义 Logger 接口 | 1 人·天 |
| C2 | 将 `func(string)` 回调迁移到 `log.Logger` | 3 人·天 |
| C3 | 在 trace/memory/checkpoint 中实现 RunID 生成 + 注入 | 2 人·天 |
| C4 | 在 checkpoint 中添加 LastTraceSeq + TraceLineOffset | 1 人·天 |
| C5 | 在 memory.Entry 中添加 RunID/TraceSeq 字段 | 1 人·天 |
| C6 | 实现 `forge trace query` 跨存储查询 | 2 人·天 |
| **总估算** | | **~10 人·天** |

风险：C2 触及了 15+ 文件。需要仔细的回调到 Logger 适配器，以防止在迁移期间丢失日志行。

### 风险矩阵

| 风险 | 可能性 | 影响 | 缓解 |
|------|----------|----------|------------|
| YAML 解析器替换产生与现有测试的逐字节差异 | 中 | 中 | 并行测试所有 7 个真实 workflow 文件；记录任何差异；在替换前获得团队审查 |
| LaneRegistry 重构将 bug 引入 prompt 装配 | 低 | **高** | Feature-flag 新路径；使用 golden file 测试；并行运行新旧方案并比较输出 |
| 磁盘缓存层在并发 forge 进程下损坏 | 低 | 中 | 在写入时使用 O_EXCL 创建 + 原子重命名；记录"最后一个写入者胜出"语义 |
| N/A 趋势在非 CI 环境中生成误报 | 中 | 低 | 使趋势警告默认开启，但可通过 `--accept --no-na-trend` 抑制 |
| 结构化日志迁移在重构期间静默丢失日志行 | 中 | 中 | 用硬断言（新旧调用计数相同）逐步替换 `func(string)` |
| RunID 注入破坏现有 trace 解析器（下游消费者） | 低 | 中 | RunID 是可选字段；解析器应在缺少时读取且不报错 |

### 里程碑摘要

```
Sprint 32 ── 阶段 A: Prompt 安全 + YAML 解析器替换
                  🏁 forge accept 绿色；所有 7 个 workflow 输出字节一致
                          
Sprint 33 ── 阶段 B: Gate 覆盖完整性 + 启动缓存
                  🏁 N/A 趋势 + 期望闸门 + 缓存；CI 减少 ~12 次文件 IO
                          
Sprint 34 ── 阶段 C: 可观测性 + 结构化日志
                  🏁 RunID 跨三存储；结构化日志；forge trace query 在线
```

### 关于"减少"的注意事项

工程团队应抵制将这三个 sprint 扩展为更大工作的诱惑。每个阶段都有一个明确定义的**停止条件**和在下一个阶段之前可独立验证的交付物。没有追逐"全谱跨存储查询引擎"——只提供足够的交叉引用以回答单一的取证问题。没有"完整的 CLI 框架重写"——只替换 YAML 解析器和日志基础设施。限制范围是 ForgeOS 在 31 个 sprint 中建立的原则，而本路线图正是遵循了这一方法。
