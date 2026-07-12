Now I have a comprehensive understanding of both the four structural gaps, their verification results, and the broader project context. Let me produce the architectural analysis.

---

# 架构分析：ForgeOS 四方向结构债的验证后评估与扩展设计

## 一、架构评估

### 1.1 当前架构的核心优势

ForgeOS 经过 31 轮 sprint 迭代，在**功能成熟度**上已达到相当高度：

- **零外部依赖的 Go 运行时**：13 个包、~32k LOC 的 `forge-core` 全部基于纯 Go 标准库，`go.mod` 零 require 条目。这是一种极高的工程纪律，在 Go 生态中极为罕见。
- **自我执法的治理层**：arch-check 执行 8 项架构检查（layering、包结构、扇入、认知复杂度、反模式命名、函数长度、循环依赖、drift-guard），加上 secret-scan、check.py 治理检查和测试闸门。相比大多数声称"架构优先"的项目，ForgeOS 是少数真正用代码执法架构的项目。
- **诚实的 N/A 报告**：在任何无法执行的检查项上诚实标记 N/A、绝不伪造通过——这在自动化治理系统中是关键的信任基石。
- **清晰的分层架构**：`internal/` 包群（18 个包）与 `cmd/forge` CLI 层的分离，虽然执行上有偏移，但意图和方向是正确的。
- **生产安全护栏**：递归深度守卫、Agent 调用预算守卫、超时、输出截断、mode-gating——这些不是空文档声明，而是真正在 `forge-core` 中实现并测试的。

### 1.2 当前架构的关键局限

**局限一：架构成熟度滞后于功能成熟度（根本矛盾）**

验证报告确认的四方向结构债，本质是一个根本矛盾的症状：ForgeOS 的功能面（workflow 语义完整可表达、五大引擎声明、trace 基础设施丰富）已经远超其**架构面的成熟度**（schema 碎在 6 个位置、Context Engine 是字符串拼接、零结构化日志、进程外集成为 fork/exec + 字符串解析）。

| 维度 | 功能面（已做到） | 架构面（债务所在） |
|------|----------|--------------|
| Schema 管理 | Phase 支持 ~40+ 语义字段 | 6 个点各自维护，遗漏即 GAP |
| Context Engine | 声明为五大引擎之一 | 26 行有效逻辑、字符串拼接 |
| 可观测性 | Trace 结构化事件、6+ Event kinds | 零条结构化日志 |
| 多 CLI 支持 | "站在所有 CLI 之上"的愿景 | 23 处 os/exec + 5+ 文本解析器 |
| CLI 层 | 15 条命令、完整功能入口 | 12,513 行 / 16 文件，违反自身 SLAP |

**局限二：`cmd/forge` 包结构债务（层间倒置）**

验证报告已多次点出但未被纳入四个方向的核心问题：`cmd/forge` 包平均 ~782 行/文件，远超过本仓库自身的 `max_file_lines:500` 红线。核心业务逻辑（cost 解析、prompt 构建、4 种 ledger 管理）存在于 CLI 层而非 `internal/` 包中。`prompt_context.go`（454 行）是 `internal/prompt/prompt.go`（59 行有效逻辑）的 **7 倍**——这是明确的层间倒置。

**局限三：YAML 解析器三重碎片化**

验证报告补充了方向四 os/exec 问题规模为 13 个文件（原分析写 6+），但还有一个**未被四个方向覆盖但同样严重的问题**：YAML 解析路径三重碎片化。Go 原生解析器（YAML 子集）、Python shim 回退（完整 YAML 1.x）、`internal/yamlpath`（仅 Python shim）——三条路径可能对同一文件产生不同的结构化结果。ForgeOS 作为"治理 OS"，自身配置数据的确定性是基本要求。

### 1.3 关键设计决策是否合理

| 决策 | 合理性 | 当前状态 |
|------|--------|----------|
| 零外部依赖 | **合理但代价在累积**。排除了依赖管理的复杂性，但无法使用 protobuf、OpenAPI codegen、zap/zerolog 等成熟工具。当前 13 个包的规模下仍然可控，但继续增长可能导致"自己造轮子"的成本超过引入外部依赖的收益。| 需要定期复审：是否仍值得零依赖？ |
| fork/exec + 字符串解析作为 Agent 集成模式 | **在产品验证阶段合理，但已到迁移节点**。在 v0-v1 中这是对抗不确定性的务实选择。但验证报告确认 23 处 `os/exec`，意味着已从"所有集成点"变为"耦合的扩散"。 | 已到架构迁移的触发点 |
| Phase 作为单一通用载体 | **已超出合理边界**。~40 字段的单一结构体承载从权限到模板到反馈方向的全部语义。这违反了单一职责原则——Phase 应当是一个稳定的核心对象，而非所有属性的垃圾桶。 | 已到拆分节点 |
| trace 优先于日志 | **合理但单向发展**。结构化的 trace 事件是 ForgeOS 可观测性的亮点。但从"只做 trace"到"不做日志"的跳跃是设计盲区——trace 回答"发生了什么事件"，日志回答"为什么"，两者互补。 | 应补日志而非替代 trace |

### 1.4 架构债务与技术债

我将结构债分为三个层级：

**红色层级（P0，立即影响演进能力）**：

1. **Phase struct 碎片化**（方向一）：每次新字段=6 处修改，已造成 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 中一半的 GAP 记录。这是代码库中最昂贵的单点维护税。
2. **Agent CLI 契约脆弱性**（方向四）：23 处 `os/exec` + 5+ 文本解析器全部依赖 claude 私有输出格式。格式变化=静默数据丢失，且多 CLI 支持的复制成本呈线性增长。

**橙色层级（P1，限制运营效率和架构可信度）**：

3. **字符串拼接 Context Engine**（方向二）：架构文档写五大引擎，实际是 `fmt.Fprintf` + `strings.Join`。限制 token 管理、prompt 结构化、跨厂商适配。
4. **零结构化日志**（方向三）：Go 1.26 已可用 `log/slog`（零依赖），但全仓 0 条结构化日志。24h 无人值守 evolve 循环的排障仍然是人肉 grep 模式。

**黄色层级（P2，特定场景触发）**：

5. **YAML 解析器三重碎片化**：三个路径可能产生不同结果，从未被系统检测。
6. **`cmd/forge` 包结构债务**：CLI 层违反自身红线。
7. **长运行时存储累积**：tracel + memory.jsonl 无限增长。
8. **跨进程文件冲突**：无 flock/LOCK_EX。

---

## 二、高价值架构扩展方向

以下扩展方向基于四个已确认的结构债和 broader codebase 扫描，按**业务价值 × 技术杠杆**排序。

### 方向 A：Schema Registry + Struct Codegen（地址方向一 + YAML 碎片化）

**严重程度**: 🔴 **P0** — 直接影响未来每一个 sprint 的交付效率

**为什么需要**：

当前 Phase struct 的~40 字段、6 处同步位置是所有功能团队成员的共同痛点。验证报告确认的 13 个 GAP 中一半源于 schema 碎片化。每加一个字段，涉及 YAML 源 + Go struct + mode gating + gate resolution + converge signal + check.py = 6 处修改。这不是"代码风格问题"，而是**维护速度的 O(n) 退化函数**——n=字段数。

**核心挑战**：

1. **零外部依赖约束**：不能引入 protobuf、OpenAPI codegen、flatbuffers——必须纯 stdlib。
2. **向后兼容**：已有 ~8 个工作流 YAML 文件不能破坏。新旧 schema 并存期的转化逻辑需要周全设计。
3. **Go 代码生成**：stdlib 的 `go generate` 可用，但 codegen 工具本身不能引入外部依赖。可考虑 Go `text/template` + `parser`/`ast` 包做生成器。

**预期架构变更**：

```
当前:
  .agent/workflows/*.yml  ← 手动 →  asset.go Phase struct  ← 手动 →  mode.go / resolve.go / converge.go / check.py

目标:
  .agent/schemas/phase.yaml  ← 单一事实源
       ↓  (codegen → go generate)
  internal/asset/phase_gen.go    ← 自动生成 Phase struct + 解码器
  internal/mode/phase_gen.go     ← 自动生成 mode gating 映射
  internal/gate/phase_gen.go     ← 自动生成 gate resolution
  internal/converge/phase_gen.go ← 自动生成 convergence mapping
  harness/check.py               ← 自动生成 schema 校验（从 YAML schema 生成）
```

**对现有系统的影响**：
- 不影响现有 workflow YAML 格式——Phase struct 字段名和 JSON tag 保持不变
- 影响：asset.go 中 329 行的 Phase 定义将被 codegen 替代（但保持兼容）
- `go generate` 步骤纳入 `forge build` 流程或 CI gate

**方案选项**：

| 方案 | 复杂度 | 维护成本 | 向后兼容 | 推荐 |
|------|--------|----------|----------|------|
| A1: 纯手动拆分 Phase 为多个子结构体 | 低 | 中（仍需多文件维护） | ✅ 最佳 | ❌ 治标 |
| A2: YAML schema + Go codegen (go generate) | 中 | 低（仅维护 schema） | ✅ | ✅ **推荐** |
| A3: Go 反射式 schema 注册（运行时自描述） | 高 | 中 | ⚠️ 反射性能开销 | ❌ 过度设计 |

### 方向 B：Agent Executor 插件接口（地址方向四）

**严重程度**: 🔴 **P0** — 多 CLI 愿景在架构层面被锁定

**为什么需要**：

验证报告确认 23 处 `os/exec`（原分析仅 6+），5+ parser 全部依赖 claude 私有输出格式。每次新增 CLI 支持（Codex / Gemini CLI / OpenHands）的复制成本 ≈ `engine_build.go` + `cost.go` + 5 个 parser + prompt 注入逻辑。这不是"代码复用"问题，是**架构模式决定集成成本的 O(n) 函数**——n=支持的 CLI 数量。

**核心挑战**：

1. **输出格式多样性**：Claude 有 JSON envelope + 散文 VERDICT；Codex 不同；Gemini CLI 不同——抽象接口必须容纳各 CLI 的特异性而不退化为"把所有差异塞进字符串"。
2. **版本协商**：CLI 输出格式不是版本化 API。需要设计轻量级版本声明机制（agent card 中声明 `protocol_version: 1`，parser 按版本分派）。
3. **测试基础设施**：当前集成测试要么 fake script（不真实），要么真 claude（付费且慢）。需要 recording/replay 中间层。

**预期架构变更**：

```
当前:
  cmd/forge/
    engine_build.go  ← 硬编码 claude argv 构造
    cost.go          ← 5 个 claude-specific parser
    prompt_context.go ← 注入逻辑与 CLI 类型耦合

目标:
  internal/executor/
    executor.go        ← AgentExecutor 接口
    claude.go          ← Claude Code 适配器
    codex.go           ← Codex CLI 适配器
    gemini.go          ← Gemini CLI 适配器
    replay.go          ← Recording/Replay 中间件
    contract.go        ← 契约测试框架
  
  cmd/forge/
    engine_build.go    ← 减少为"选适配器 + 调用"
    cost.go            ← 减少为"通用成本聚合"，解析在适配器
```

**AgentExecutor 接口设计**（伪代码级）：

```go
type AgentExecutor interface {
    // Name 返回 CLI 标识符（claude / codex / gemini）
    Name() string
    
    // BuildArgv 将 Phase + Prompt 转为 CLI 参数
    BuildArgv(p PromptParams) (argv []string, opts RunOptions)
    
    // ParseOutput 解析 CLI stdout/stderr 为结构化结果
    ParseOutput(stdout, stderr []byte) ExecutorResult
    
    // ParseCost 从输出提取成本信息
    ParseCost(output []byte) (costUsd float64, ok bool)
    
    // ParseVerdict 从输出提取评审裁决
    ParseVerdict(output []byte) (verdict Verdict, confidence float64, ok bool)
    
    // DetectOutputFormat 检测输出格式版本（若 CLI 支持版本协商）
    DetectOutputFormat(output []byte) string
}
```

**对现有系统的影响**：
- 高（中期）——需要将 5 个 parser、arg 构造、prompt 注入逻辑逐个迁移到适配器
- 低（短期，并行期）——可以创建 `DefaultClaudeExecutor` 作为现有逻辑的"包裹"，再将现有调用点逐步迁移
- 向后兼容：现有 claude 输出格式继续支持，通过版本检测分支处理

### 方向 C：结构化 Prompt Pipeline（地址方向二）

**严重程度**: 🟠 **P1** — 目前够用但已到天花板

**为什么需要**：

验证确认 Context Engine 核心仅 26 行（`prompt.go:Gather`）有效逻辑，CLI 层 454 行。随着 agent 交互复杂度增长（memory 积累 100+ 条、gate 信号丰富化、多厂商 prompt 格式差异），字符串拼接架构的局限性将变成系统性质量问题。验证报告还发现 `adrTopK=6` 的硬编码——当 ADR 数量增长后，基于子串匹配的检索会有语义盲区。

**核心挑战**：

1. **Token 记账**：不能引入 tiktoken（外部依赖），需要纯 Go 实现 token 近似估算（可基于字符/字节映射 bound，接受误差）。
2. **Context Lane 优先级模型**：安全约束（始终注入）> gate 结果 > memory > ADR > 产物。需要形式化的优先级排序而非当前的手动 `append` 顺序。
3. **冲突解决**：当 memory 说"方案 A 失败"、gate 说"方案 A 不适用"、prompt 却说"方案 A 可用"——当前等权拼接让 agent 自行判断。结构化 pipeline 可以注入"三个信息来源的冲突摘要"。
4. **跨厂商格式**：Claude 的 system prompt / XML 标签格式与 OpenAI 的 messages API 格式不同。当前纯字符串模型无法适配。

**预期架构变更**：

```
当前:
  internal/prompt/
    prompt.go (59 loc)  ← strings.Join(ctx, "\n\n") 是核心
  
  cmd/forge/
    prompt_context.go (454 loc)  ← 收集 4 种 ledger + 构建 lanes

目标:
  internal/prompt/
    pipeline.go         ← PromptPipeline 编排器
    lane.go             ← ContextLane 类型（优先级/预算/格式器）
    token.go            ← Token 估算
    conflict.go         ← Lane 间冲突检测
    template.go         ← Phase 类型特定模板
  
  cmd/forge/
    prompt_context.go   ← 只做数据收集，不再做字符串拼接
```

**方案选项**：

| 方案 | 复杂度 | 收益 | 推荐 |
|------|--------|------|------|
| B1: 拆分 ContextLane 类型 + token 估算 | 中 | 中等（立即可得） | ✅ **阶段一** |
| B2: B1 + 优先级/冲突解决 | 中高 | 高（决策质量提升） | ✅ **阶段二** |
| B3: B2 + 跨厂商格式适配 | 高 | 高（多 CLI 前提） | ⏳ 阶段三 |

### 方向 D：结构化日志基础设施（地址方向三）

**严重程度**: 🟠 **P1** — Go 1.26 已有 `log/slog`，零外部依赖

**为什么需要**：

验证确认 Go 1.26（分析原写 1.24），`log/slog` 在标准库中完全可用。当前 100+ 处非结构化 `fmt.Printf` / `fmt.Fprintf(os.Stderr)` / `log.Printf` 混杂无规律。对 24h 无人值守 evolve 循环，排障需要关联 trace 事件与操作日志——但当前 trace 与日志之间**零关联**（trace.Event 无 `CorrelationID`、无 `LogRef`）。

**核心挑战**：

1. **双通道分离**：用户可见输出（`forge status`、`forge validate` 的结构化表格）不应变为 JSON 日志行。日志通道和 UI 输出通道必须是不同的 `io.Writer`。
2. **日志路由**：evolve 循环的日志写入文件（`.forge/evolve.log`），CLI 即时命令写 stdout——需要按命令区分。
3. **关联 ID**：trace 事件与日志条目的关联需要一个共享的 correlation ID（trace ID 或 execution ID）。
4. **性能**：高频路径（每 phase 的 trace emits）不应阻塞。slog 的 `Handler` 接口支持异步写入。

**预期架构变更**：

```
当前:
  forge-core/cmd/forge/*.go  →  fmt.Printf / fmt.Fprintf(os.Stderr) / log.Printf 混用

目标:
  internal/telemetry/
    logger.go       ← slog logger 初始化 + 文件路由
    correlation.go  ← correlation ID 生成与传播
    adapter.go      ← 异步文件 Handler（slog.Handler 接口）
  
  所有命令入口:
    slog.Info("evolve iteration started", "iteration", i, "phase", p.Name)
    slog.Error("gate failed", "gate", g.Name, "error", err)
    // 用户可见输出仍用 fmt.Printf（UI 通道）
    fmt.Printf("✓ %s passed\n", g.Name)
```

**对现有系统的影响**：
- 低（增量引入）——不需要一次改所有 100+ 处 `fmt.Printf`，可以在新增代码中优先使用结构化日志
- 不影响用户可见 UI 输出（保留 `fmt.Printf` 作为 UI 通道）
- 不影响 trace 事件流（日志是 trace 的补充，非替代）

### 方向 E：运行时存储生命周期管理（地址 forgotten-gap 方向三 + 方向四）

**严重程度**: 🟡 **P2** — 短跑不触发，24h+ 必然触发

**为什么需要**：

验证已确认 `memory.Prune` / `Compact` 存在于代码但从未被自动调用。`forge evolve` 运行 24 小时以上，trace.jsonl + memory.jsonl 无限增长。同时，跨进程文件冲突（无 flock/LOCK_EX）使并行运行不安全。这两个问题共享同一个根——`.forge/` 状态目录没有生命周期管理。

**核心挑战**：

1. **存储预算**：定义 `.forge/` 目录总大小上限（如 100MB），在 evolve 循环中周期性检查。
2. **自动轮换**：trace.jsonl 按 size 或迭代数轮换（如每 100 迭代轮换一个文件）。
3. **进程锁**：在 `.forge/` 目录上使用 `flock`（`syscall.Flock`，纯 stdlib，零外部依赖）防止跨进程冲突。锁文件 + 非阻塞尝试 + 优雅降级（不能拿到锁则 wait 或 error）。
4. **memory 自动清理**：`LoopEngine` 每 N 迭代触发一次 `memory.Prune`（keep top-K per kind）和 `memory.Compact`。

**预期架构变更**：

```
当前:
  .forge/
    trace.jsonl    ← 只增不删
    memory.jsonl   ← 只增不删（Prune/Compact 函数存在但无人调用）
    checkpoint.json ← 覆盖写（安全）
    
目标:
  internal/storage/
    budget.go      ← 存储预算检查
    rotation.go    ← 轮换策略
    lock.go        ← 进程锁（flock）
  
  internal/orchestrator/loop.go:
    LoopEngine.StorageBudget  ← 新增字段
    LoopEngine.cleanup()      ← 每 N 迭代触发
    
  internal/memory/store.go:
    AutoPrune()    ← LoopEngine 定时调用
```

---

## 三、接口设计建议

### 3.1 关键模块接口设计原则

**原则一：Schema 优先，代码生成**

所有跨包共享的数据结构（Phase、Signal、Workflow、GateResult）应当有**单一事实源**。当前 Go struct 作为事实源是次优选择，因为 YAML 消费端和 Python 消费端无法从 Go struct 派生。推荐：

- 用 YAML/JSON Schema 或简单的**自描述声明格式**定义语义模型
- `go generate` 从 schema 生成 Go 代码（struct + JSON 解码器 + validator）
- 从同一 schema 生成 `check.py` 使用的 Python 校验规则

**原则二：接口隔离 + 适配器模式**

对于三个存在多种后端的系统（Agent CLI、YAML 解析、日志输出），使用适配器接口：

- **AgentExecutor 接口**：Runnable（运行）、Parsable（输出解析）、Costable（成本提取）
- **YAMLParser 接口**：Decode（解码）、Validate（校验）、Diff（差异检测，跨路径一致性校验）
- **LogHandler 接口**：slog.Handler 已定义，直接复用 stdlib 接口

**原则三：依赖方向朝内**

`cmd/forge` 依赖 `internal/`，`internal/` 不依赖 `cmd/forge`。当前 `prompt_context.go`（CLI 层）包含比 `internal/prompt/prompt.go`（核心层）多 7x 的代码是层间倒置——应当将核心逻辑迁移入 `internal/prompt`，CLI 层只做参数解析和数据收集。

### 3.2 是否需要新的抽象层

| 层 | 需要 | 理由 |
|----|------|------|
| `internal/executor/` | ✅ **需要** | Agent CLI 适配器 + replay 测试框架，解决方向四 |
| `internal/schema/` | ✅ **需要** | Schema 注册 + codegen 工具链，解决方向一 |
| `internal/telemetry/` | ✅ **推荐** | 结构化日志基础设施 |
| `internal/storage/` | ⏳ **可暂缓** | 存储生命周期管理，P2 层级 |
| `internal/ledger/` | ⚠️ **不一定需要** | 4 种 ledger 可以拆入各自所属的 `internal/` 子包 |

### 3.3 向后兼容策略

1. **Phase struct 变化**：保留现有字段的 JSON tags，新字段以 `omitempty` 形式添加。旧 YAML 文件自动映射。
2. **AgentExecutor 迁移**：现有 `engine_build.go` + `cost.go` 的解析逻辑保留为默认 `ClaudeExecutor` 实现，新适配器作为增量。
3. **日志引入**：`fmt.Printf` UI 输出保留不动，仅在新增代码中使用 `slog.Info/Error`。渐进式迁移而非一次性替换。
4. **Schema 版本**：建议在 Phase struct 中加 `SchemaVersion int \`json:"schema_version"\`` 字段，默认 0=现有格式，新增的版本化字段在 version>0 时启用。

---

## 四、技术选型

### 4.1 是否需要引入新的技术栈

| 技术 | 需要程度 | 理由 | 复杂度 |
|------|----------|------|--------|
| `log/slog`（stdlib） | **立即需要** | Go 1.21+ 标准库，零外部依赖。无理由不使用 | 低 |
| Codegen 工具（`go generate` + stdlib text/template） | **需要** | 解决 schema 碎片化的核心杠杆。纯 stdlib 可实现 | 中 |
| `syscall.Flock`（stdlib） | **需要** | 进程锁的唯一 stdlib 方案，零依赖 | 低 |
| Go generics 类型参数 | **可选** | 整理 ledger 类型、context lane 类型时可用 | 低 |
| Go `testing/fstest` | **可选** | 存储层的测试模拟 | 低 |

**不建议引入**：

| 技术 | 不建议理由 |
|------|------------|
| protobuf / flatbuffers | 零依赖约束下不可，且对 ForgeOS 内部数据大材小用 |
| zap / zerolog / logrus | slog 已满足全部需求，零依赖替代 |
| tiktoken（OpenAI tokenizer） | 外部依赖 + 仅 OpenAI。纯 Go 字符映射近似方案足够 |
| OpenAPI codegen | 零依赖约束下不可，且 ForgeOS 内部不需要 HTTP API |

### 4.2 第三方依赖评估标准

如果在未来某个节点需要突破零依赖约束，建议按以下标准评估：

1. **许可证兼容**：必须 MIT / Apache 2.0 / BSD，GPL/AGPL 绝对不可
2. **零传递依赖**：最好无传递依赖，最多 1 层
3. **Go 版本兼容**：支持 Go 1.26
4. **测试覆盖**：>80%
5. **API 稳定性**：go.mod 有 `v1` 或已承诺向后兼容
6. **维护活跃度**：过去 12 个月有提交

### 4.3 自建 vs 采购的决策依据

ForgeOS 当前的四个方向全部涉及**内部架构问题**，不是外部能力采购。决策维度很简单：

- **核心差异化能力**（schema 管理、Context Engine、Agent 集成）→ 自建
- **非差异化基础设施**（日志、锁、token 估算）→ 用 stdlib 或轻量自建

唯一有"采购"替代方案的方向是方向四的 Agent CLIs——"采购"一个支持标准化协议的新 CLI（如 Codex 或 Gemini）反而会暴露当前架构的脆弱性。先修内部架构，再谈新的集成。

---

## 五、实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 代码名 | 预估 sprint | 依赖 |
|--------|------|--------|-------------|------|
| **P0** | 方向一：Schema 注册 + Codegen | `phase-schema` | 2-3 sprints | 无 |
| **P0** | 方向四：AgentExecutor 接口 | `agent-executor` | 2-3 sprints | 无 |
| **P1** | 方向三：结构化日志 | `structured-logging` | 1-2 sprints | 无 |
| **P1** | 方向二：Context Lane 类型拆分 | `prompt-pipeline-v1` | 2 sprints | 日志（可并行） |
| **P2** | 方向四延伸：Recording/Replay 测试 | `executor-test` | 1-2 sprints | AgentExecutor 接口 |
| **P2** | 方向二延伸：Token 估算 | `token-budget` | 1 sprint | Context Lane 拆分 |
| **P2** | 存储生命周期管理 | `storage-mgmt` | 1-2 sprints | 日志（共享 writer 模式） |
| **P2** | YAML 解析器统一 | `yaml-unify` | 2 sprints | 无 |

### 5.2 阶段划分和里程碑

**Phase 1：架构止血（Sprint A-B）**

目标：消除 P0 方向的最大风险，不引入新功能。

```
Sprint A:
  [phase-schema] 定义 schema 注册包 internal/schema/
                 选择 codegen 方案（推荐 YAML schema → go generate）
                 Phase struct 标记 SchemaVersion 字段（向后兼容）
  [agent-executor] 定义 AgentExecutor 接口
                   DefaultClaudeExecutor 作为现有逻辑的包裹

Sprint B:
  [phase-schema] 实现 codegen → Phase struct + JSON decoder 自动生成
                 迁移 mode.go / resolve.go / converge.go 到生成代码
                 修复 6 个消费端的同步问题
  [agent-executor] 迁移 5 个 parser 到 DefaultClaudeExecutor
                   Recording/Replay 中间件最小实现
```

里程碑 🔴 **"Schema Not Fragmented Anymore"**：新增一个 Phase 字段只需要改 1 个 schema 文件 + `go generate`。

**Phase 2：运营基础设施（Sprint C-D）**

目标：为 24h 无人值守 evolve 提供结构化日志和 prompt 基础改进。

```
Sprint C:
  [structured-logging] 引入 slog，定义 telemetry 包
                       日志路由（文件 vs stdout）
                       trace 与日志的 correlation ID 连接
                       高频路径异步 Handler

Sprint D:
  [prompt-pipeline-v1] ContextLane 类型定义（优先级 / 权重 / 格式器）
                       token 估算器（字符映射 bound）
                       buildPrompt 改为 ContextLane 编排
```

里程碑 🟢 **"24h 可观测"**：evolve 循环产生结构化日志，可关联 trace 事件，可查询 "why gate failed"。

**Phase 3：架构增量改进（Sprint E-F）**

目标：解决剩余 P1/P2 方向，完善测试基础设施。

```
Sprint E:
  [executor-test]   契约测试框架：fake CLI → recording → replay
                    Claude Code 的 RECORD/REPLAY 模式
  [token-budget]    token 感知的 context lane 注入
                    自动降级策略（超 budget 时丢弃低优先级 lane）

Sprint F:
  [storage-mgmt]    LoopEngine 集成存储预算
                    trace.jsonl 轮换 + memory.Prune 自动调用
                    flock 进程锁
  [yaml-unify]      统一 YAML 解析路径或增加交叉路径差分检测
```

里程碑 🟢 **"多 CLI Ready"**：新增一个 CLI 适配器只需要 ~200 行（arg builder + output parser + cost extractor），而非复制 5+ 文件。

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Codegen 增加构建复杂度 | 中 | 低 | stdlib 的 `go generate` + `text/template` 已足够成熟；构建步骤仅新增一个 `go generate ./...` |
| AgentExecutor 接口抽象过早 | 中 | 中 | 从现有 claude 适配器提炼接口，避免"预测未来需求"的过度抽象。可先做 `DefaultExecutor` 包裹，其余适配器按需 |
| `log/slog` 迁移被无限期推迟 | 高 | 中 | 从新代码开始强制使用 slog，旧代码不强制迁移但新代码 PR 必须使用。不需要一次改 100+ 处 |
| Schema 版本化导致 YAML 兼容问题 | 低 | 高 | 初始版本设置为 `schema_version: 0`，0 版本使用现有字段名和 JSON tags。新字段使用 `omitempty` |
| 跨包依赖引入循环 | 低 | 高 | 严格遵守 layering：`executor` 可以依赖 `prompt` 但不能反过来；`schema` 是纯数据层，不依赖任何包 |
| 存储管理干扰现有 evolve 流程 | 低 | 高 | 默认行为为"无限制"（向后兼容），存储预算仅在有 `--max-storage-mb` 或配置时生效 |
| AgentExecutor 迁移破坏 claude 集成 | 中 | 高 | 双轨部署：DefaultExecutor 使用现有解析逻辑，新适配器在测试验证后再切换。回归测试覆盖所有 5 个 parser |

### 5.4 不做的明确范围

- **不引入外部依赖**：以上全部方向均可通过 Go 标准库 + `go generate` + 自建 codegen 实现。在 Phase 3 结束时复审零依赖决策的合理性，但不预判。
- **不重写 Context Engine 为完全体**：方向二的 Phase 1 只做 ContextLane 类型拆分 + token 估算。完整的冲突解决和跨厂商格式适配留到受实际需求驱动时（如添加第二个 CLI 后端时）。
- **不自建监控仪表盘**：方向三的日志基础设施是"生产日志"，不包含 Grafana / Datadog / Prometheus 集成——这些属于北向扩展（已纳入路线图但不在本周期）。
- **不重写已有 YAML 文件格式**：所有 schema 变更必须向后兼容。旧 workflow YAML 文件不需要修改。

---

## 总结：根本矛盾与行动呼吁

这四个方向的验证确认（19/20 代码证据准确率、4/4 架构判断正确），加上补充发现（Go 1.26 已到、os/exec 问题 2x 于原估计、Phase 实际 ~40+ 字段），共同指向一个清晰结论：

> **ForgeOS 的功能成熟度已经远超其架构成熟度，结构债正在从"可承受的工程债务"向"制约未来交付速度的硬约束"过渡。**

每个方向都是"不影响用户可见功能"的结构债——修复它们不会在 `forge run` 的输出中增加一行新特性。但它们的累积效应正在：

- 使每个新字段的交付时间从 30 分钟变为 2 小时（方向一）
- 使每个新 CLI 支持的交付成本从 2 天变为 2 周（方向四）
- 使每个生产问题的排障时间从 5 分钟变为 2 小时（方向三）
- 使每个 prompt 优化的尝试都需要重写 `Build` 函数（方向二）

建议立即启动 **Phase 1（P0 方向止血）**，并将 P1 方向的 `structured-logging` 作为先导（slog 是 stdlib 零成本引入，从新代码开始即可获益）。
