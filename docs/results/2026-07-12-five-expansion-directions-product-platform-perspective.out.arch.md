现在我有了充分的代码基础。以下是全面的架构分析：

---

# 架构分析：ForgeOS 五方向扩展战略

## 1. 架构评估

### 1.1 当前架构优势

ForgeOS 的架构在核心领域做得非常出色，尤其在以下方面：

- **纯净的解耦层**：Go 核心（`forge-core/`）零外部依赖，运行时代码约束严格。CLI dispatch（`cmd/forge`）和内部逻辑（`internal/`）之间的分离是清晰的——`doctor.go` 是一个典型的例子：它返回纯数据，由其消费者负责格式化。
- **Trace 系统设计精良**：注入式 `Now`、独立的 `encode()` 纯函数、Seq 单调递增语义、`Event` 构造辅助函数（`GateEvent`/`DecisionEvent`/`OverloadEvent` 等）——这表明封装的作者对可测试性和可演化性有深入的思考。
- **错误分类基础扎实**：`ExecError` 拥有 `ExecKind` 枚举（不仅仅是自由文本）、`Unwrap()` 用于 `errors.Is/As` 穿透、以及清晰的区分——`config` vs `timeout` vs `failed` vs `recursion` vs `overloaded`。这比大多数在标准库错误之上的 Go 项目要优越得多。
- **运行时路径上最少的状态**：引擎是无状态的，每次迭代从头开始重放工作流，只在 `checkpoint.json` 中保留状态。这使得注入健康检查和观测性挂接（方向五/一）变得简单——没有需要维护的长时间运行的内存状态机。

### 1.2 关键架构债务

| 债务 | 严重程度 | 代码证据 | 对扩展的影响 |
|------|----------|----------|--------------|
| **事件缺少会话身份** | 高 | `trace.Event` 没有 `SessionID`/`RunID`/`WorkflowID` | 方向二（跨会话）和方向四（错误聚合）需要它 |
| **无运行时消费层** | 高 | 所有运行时数据只通过文件消费 | 方向一（可观测性API）就是对此的回应 |
| **硬编码调度属性** | 高 | `opusFloorAgents`/`agentTier`/`modeDefault` 是 Go 映射 | 方向三（插件化）要求这些变为声明式 |
| **错误元数据裸奔** | 中 | `ExecError` 没有 `Code()` 或 `Severity()` | 方向四（错误语义）从代码和严重性开始 |
| **自我监控缺失** | 高 | `traceCheck` 只在医生运行时运行，而非在运行时持续运行 | 方向五（自我监控）是针对此的功能需求 |
| **Phase 引擎是封闭调度** | 中 | 引擎的 `switch` 或 if-else 链隐式硬编码了阶段类型 | 方向三需要这里有一个 registry |
| **NextStage 被解码但未被执行** | 低 | `OnApproved.NextStage` 已解码并显示，但从未被用于自动流水线触发 | 方向二需要将信息转化为行动 |

### 1.3 关于交叉验证声明的一个关键更正

交叉验证文档在几个代码声明上存在不准确之处：

1. **`Tracer.Emit()` 不会静默吞错误**——它返回 `error`。跨层次混淆了 `Emit()` 和 `Span()`，后者确实吞错误（`_ = t.Emit(...)`）。`Span` 的文档甚至说明了为什么："一条丢失的 trace 行绝不能掩盖真实工作的结果"。这很重要——它意味着一个 `Emit` 级别的插入点可以返回错误而不破坏向后兼容性。

2. **`NextStage` 在 `asset.go` 中被解码**——它位于 `OnApproved`（第228行），并且被 `main.go` 的 `nextStageLabel` 用于在 `forge run` 的人类门报告中显示。它不仅仅是用于"静默化解析警告"——它实际上被解析并显示。真正的差距在于没有任何东西**执行**它以实现自动流水线执行。

3. **不存在 `PhaseType` 枚举**——重新查看 `asset.go:143-170`，`Phase` 是一个单一的扁平结构体，带有可选字段，没有 `Type` 字段。阶段在引擎级别通过检查字段（如 `Agent` vs harness 阶段）来区分，而不是通过类型标签。这使得方向三的"新步骤类型需要新的 `PhaseType` 值"这一论点不那么有力——事实上，添加一个带有新字段的新阶段类型可以通过在 `Phase` 上添加一个可选字段来实现，而不需要类型枚举。

这些差异实际上强化了基础论点——它们只是证明基础设施比所述更好，但结构性缺口同样真实。

---

## 2. 扩展方向

### 方向一：运行时可观测性 API（P0）

**架构评估**：正确优先级。这是 TUI 基础设施，也是方向二、四、五的前提条件。

**核心技术挑战**：
- 消费者之间的非阻塞扇出，不影响 JSONL 主写入路径
- 在 `forge run` 进行中完成，不引入守护进程
- 没有事件数据模型变更的实时驱动（当前 `Event` 没有 `SessionID`）

**推荐方法**：在 `trace.go` 和引擎之间的 `io.Writer` 边界处插入一个 `io.MultiWriter` 风格的适配器。具体来说：

```
引擎 → tracer.Emit() → obs.MultiWriter{
                          jsonlWriter (现有 .forge/trace.jsonl)
                          unixSocketWriter (新增 → TUI)
                        }
```

**这之所以正确的关键架构洞察**：`Tracer` 已经接受一个 `io.Writer`。添加一个多路复用的 `Write()` 实现是插入式的——不需要更改 `trace.go` 本身。`MultiWriter.Write()` 返回的错误处理策略应该对 JSONL writer 采用"行为失败-关闭"，对 socket writer 采用"尽力而为"（静默吞 socket 错误）。

**更深入的问题**：TUI 在运行时中途连接时的历史重放。TUI 连接 socket → socket writer 向其发送自运行开始的所有追加快照 → TUI 赶上 → 切换为仅实时。这意味着 socket 端需要维护一个环形缓冲区，或者 TUI 需要能够从 JSONL 回读。

**边缘案例**：TUI 在 `forge run` 之后连接。解决方案：socket 写入失败 ∈ 内存 ≤ 失败 ∈ 尽力而为。TUI 在连接时从 JSONL 进行追赶。

### 方向二：跨会话工作流编排（P0）

**架构评估**：从 CLI 工具到平台的最大飞跃。交付周期最长。

**真正的技术瓶颈不是代码变更，而是数据模型**：`trace.Event` 缺少 `SessionID`/`RunID`/`WorkflowID`。将此添加到 `Event` 结构中会改变磁盘格式——每个现有 trace 文件都会缺少该字段。兼容性策略：v1 格式 = 该字段为空（向后兼容读取器将其视为"未知会话"），v2 事件写入该字段。

**核心洞察：SessionID 应来自引擎，而不是来自 trace 包**。引擎调用 `tracer.Emit()`——它应该在每个 `Emit` 调用上将 `SessionID` 注入到 `Event` 中。这意味着：
- `Tracer` 获得一个 `SessionID string` 字段
- `Emit` 在序列化之前填充 `ev.SessionID`
- 现有的 trace 文件（旧格式）保持原样，没有该字段 → 读取器优雅地回退

**制品目录设计**：在 `.forge/artifacts/` 中使用一个纯 JSONL 文件。每次 `forge run` 在 Artifact Catalog 中注册，包含 `{ run_id, workflow, stage, phases: [...] }`。阶段执行调用 `run_phase(p)` 注册 `{ run_id, phase, agent, artifacts: [...] }`。这不需要数据库——只是附加到 JSONL，且只读时全量加载。

**安全风险**：每次 `forge run` 产生一个新的 `RunID`，然后所有内容都引用该 ID。一个拥有数百次运行的目录会变慢。缓解措施：在 30 天或 500 次运行后自动旋转 `.forge/artifacts/`。

### 方向三：插件化 Agent/Gate/Router 扩展系统（P1）

**架构评估**：技术上最简单，但组织上最复杂。从"我们的工具"到"他人的平台"的转变。

**正确的切入点不是"加载第三方代码"**，而是**将当前 Go 映射提升为声明式资产**。当前的硬编码映射：
```
var opusFloorAgents = map[string]bool{...}  // Go
var agentTier = map[string]string{...}       // Go
```

应该成为：
```
# .agent/routing/floors.yml
opus_floor:
  - architect
  - cto
  - reviewer

agent_tier:
  planner: sonnet
  implementer: sonnet
  ...
```

这**不需要改变引擎的执行模型**——只需将加载从编译时移到运行时。路由逻辑（`TierFor`、`BudgetAdjustTier`）保持不变——它们只读取这些映射。

**第二层：接线注册**。Phase 接线（当前在 `agentExecutor` 中用于 agent 的硬编码，以及 gate 的 `gate.HarnessRunner` 和 `RunGate` 回调）通过在命名约定中发现新资产的约定来解耦：
- `.agent/gates/<name>.mjs` → 自动作为门注册
- `.agent/agents/<name>.md` 与 frontmatter → 自动在路由层可见
- `.agent/plugins/<name>/` → 一个包含 gate、agent 或路由器维度的自包含包

**技术挑战：恐慌隔离**。Go 子进程（goroutine）中的第三方插件崩溃必须隔离，不 kill 主引擎。对于 gate 脚本（Node.js 子进程），这已经成立——引擎运行 `exec.Command` 并捕获退出代码。对于 agent 类型，如果"agent executor"是外部命令（如 claude CLI），也是成立的。如果 ForgeOS 将来支持进程内插件，它们需要各自的 `recover()` 边界。

**大问题**：路由维度钩子。当前 `Score()` 是固定维度的。添加 `--score-hook` 作为一个子进程（接收 JSON，返回额外的维度和分数）是一个干净的模式，但将引擎的关键路径延迟绑定到一个可能需要数百毫秒的外部命令——它需要超时、缓存和失败降级语义。

### 方向四：错误语义与故障目录（P1）

**架构评估**：高 ROI，低风险。可以逐步添加，与现有错误分类兼容。

**即将发生的架构合并**：`ExecError` 目前有一个 `Kind`（`ExecKind` 枚举）和一个 `Retryable()` 方法。方向四考虑添加 `Code() string`、`Severity() string`、`RecoveryHint() string`、`Component() string`。这些可以添加为新的可选方法，无需中断——实现类型通过类型断言检测，调用者优雅地回退到默认值。

**错误优先级的现有先例**：`check_mode_priorities.go` 中的 `errorPriorities` 映射（`map[string]int{"lint": 1, "test": 2, "build": 3}`）——这展示了将元数据附加到组件/错误名称的现有模式。方向四应该将其形式化为一个标准的错误目录，而不是重新发明。

**推荐设计**：

```
故障目录 (.forge/faults.jsonl)
├── run_id: string
├── session_id: string
├── faults: [
│   {
│     code: "E_AGENT_TIMEOUT"    // 机器可读、稳定、版本化
│     severity: "error"          // fatal | error | warning | info
│     component: "executor"      // orchestrator | executor | gate | router | doctor
│     kind: "timeout"            // 现有 ExecKind.String()
│     phase: "implementer"       // 来源相位
│     message: "..."             // 人类可读，自由文本
│     recovery: "retry"          // retry | skip | fix_config | escalate
│     count: 3                   // 此次运行中此类实例的数量
│     first_at: "..."            // 首次出现时间
│     last_at: "..."             // 最后出现时间
│   }
│ ]
```

**为什么是 `first_at`/`last_at`/`count`**：这是方向四的价值所在——不是"错误发生了"，而是"在 30 次迭代中，同一个相位发生了 15 次相同的错误类型"。聚合发生在写入时（引擎知道计数），而不是在 TUI 通过 grep 读取时。

### 方向五：自监控与退化检测（P0）

**架构评估**：最容易产生可见价值的，也是最容易过度设计的。用硬编码的阈值规则保持简单，而不是规则引擎。

**代码就绪性**：`traceCheck` 已经警告了 100MB 的 trace 文件。提升这个阈值——在迭代循环内添加一个每迭代的健康检查——是一个很小的改动。`doctor.go` 和 `trace.go` 中的现有基础设施意味着大约一半的阈值规则已经隐含在当前的诊断中——只是未在运行时被主动检查。

**架构模式：健康检查作为迭代后钩子**。

```
main loop:
  for each iteration:
    runPhases()
    healthCheck()       ← 新增：每迭代注入
    checkConvergence()
```

`healthCheck()` 调用 `trace.Emit()` 带有 `KindSystemHealth` 事件（新种类）。诊断数据会通过方向一的 socket 自动流向 TUI，无需新的事后机制。

**阈值规则（最低可行，数量限制为 5）**：
1. 磁盘可用空间 < 20% → WARN（< 10% → FAIL）
2. trace.jsonl > 50MB → WARN（> 100MB → 轮转并重新开始）
3. memory.jsonl 在迭代 *n* 时行数 > 在迭代 *n-5* 时行数 × 2 且无新知识 → WARN（重复积累）
4. 连续 3 次迭代持续时间 > 前 3 次平均值的 200% → WARN（退化信号）
5. 进程 RSS > 500MB → WARN（潜在内存泄漏）

**为什么是 5 条规则而不是 50 条**：方向五的架构约束是"self-monitoring 必须在核心引擎中是内联的，轻量级的"。5 条简单的 O(1) 检查在每次迭代中增加 < 1ms 的开销。50 条规则与可观察性代理竞争——这应该是一阶段的自监控，适用于 `v2`。

**自监控的反身性问题**：如果健康检查失败，应该怎么办？选项：
- **静默失败**：健康检查失败 → 记录到 trace，继续运行。最简单的选择，最安全。
- **软停止**：FAIL 检查 → 停止当前 run，记录致命错误。对于磁盘满的情况是正确的，但对于 RSS 峰值是错误的。
- **降级**：WARN 检查 → 降低并发度，进入"节省资源"模式。高风险——过度设计。

**推荐**：从静默失败 + 通过方向一的 socket 向 TUI 传播开始。TUI 可以尝试断开健康面板，但引擎运行应该只在磁盘满的情况下终止（现有行为——`os.Write` 失败会被追踪到）。

---

## 3. 接口设计建议

### 3.1 事件格式演进

当前合约：`trace.Event` 是一个扁平结构体，带有 JSON 标签。变更是通过可选字段向后兼容的。

```
v1（当前）：
  { seq, kind, name, status, duration_ms, cost_usd_micros, model, detail }

v1.5（方向一+二）：
  + session_id?    // 可选，向后兼容
  + severity?       // "info" | "warning" | "error" | "fatal"
  + error_code?     // 机器可读错误代码

v2（方向四完整）：
  + error_catalog: [
      { code, component, severity, recovery_hint, count }
    ]
```

**关键设计原则**：`omitempty` 确保格式演进不会破坏现有的事件消费者。旧的 trace 文件格式对新读取器仍然可读（缺少的字段回退为零值）。

### 3.2 可观测性适配器接口

如果将其实现为 Go `io.Writer` 多路复用器，则不需要新的 Go 接口。关键设计决策是 socket 协议。

**推荐协议**：新行分帧的 JSON（与 JSONL 相同）。这使得字节级的复用成为可能——socket writer 只需调用 `socket.Write(jsonLine)`，与 JSONL writer 调用 `file.Write(jsonLine)` 的方式相同。这意味着实现者是一个 30 行的 `io.Writer`。

**替代方案（不推荐）**：gRPC 流、WebSocket、命名管道。每种方案都增加了依赖关系（gRPC 需要 protobuf，WebSocket 需要 gorilla/websocket），而 ForgeOS 当前实现了零外部依赖。UNIX socket 是标准库原生的（`net.Listen("unix", ...)`）。

### 3.3 错误目录合约

错误目录不应是一个 Go 接口——它应该是一个**纯数据构造体**，带有用于向后兼容的可选方法：

```go
// FaultDetail 是一个结构化的错误记录，用于故障目录。
// 在路径之外，它被实现为 map[string]any 或 JSON 序列化。
// 关键：它被设计成可被 TUI 以结构化形式消费，而不是被 Go 的类型系统消费。
type FaultDetail struct {
    Code         string `json:"code"`                    // "E_AGENT_TIMEOUT"
    Severity     string `json:"severity"`                // "fatal" | "error" | "warning" | "info"
    Component    string `json:"component"`               // "executor" | "gate" | "router" | "doctor"
    RecoveryHint string `json:"recovery_hint,omitempty"` // "retry" | "skip" | "fix_config"
    Phase        string `json:"phase,omitempty"`         // 来源相位
}
```

`ExecError` 获得了这些方法（通过类型断言或向 `ExecError` 添加字段向下兼容的消费者检测）：

```go
func (e *ExecError) Code() string         // 根据 e.Kind 映射到稳定代码
func (e *ExecError) Severity() string     // 根据 e.Kind 映射到严重性
func (e *ExecError) Component() string    // 总是 "executor"
```

**为什么选择方法而不是字段**：它允许不同类型的 `ExecError` 变体在未来添加，实现相同的"代码"/"严重性"行为。像 `KindConfig` 这样的永久错误返回 `"error"` 严重性，而 `KindOverloaded` 返回 `"warning"`。这使得语义归因是动态的，而不是静态的。

---

## 4. 技术选型

### 4.1 需要引入的内容

| 组件 | 技术选择 | 理由 |
|--------|---------------|-------------|
| 事件扇出 | Go `io.MultiWriter` ∈ 自定义多路复用器 | 零新增依赖。两个 40 行的实现。 |
| 到 TUI 的传输 | UNIX domain socket | 标准库原生，无外部依赖，IPC 效率高，使用文件系统权限进行安全保护 |
| 错误代码字典 | Go 包 `internal/faults/codes.go` | 纯常量，零依赖，零运行时开销 |
| 健康阈值 | Go 包 `internal/health/rules.go` 中的硬编码常量 | 没有规则引擎，没有 YAML 膨胀，只有 5 个 `const` |
| 制品目录 | `.forge/artifacts/catalog.jsonl`（JSONL） | 保留零外部依赖的约束，与现有 trace/memory 模式一致 |

### 4.2 有意不引入的内容

| 不引入 | 理由 |
|------------|-------------|
| 用于可观测性的 gRPC/Protobuf | 过度设计。零外部依赖约束的代价很高。方向一可以通过 UNIX socket + JSONL 实现 |
| 用于错误的 Prometheus 指标 | 架构目标是"自包含的 CLI 工具"，而不是"Kubernetes 集群上的微服务"。指标导出器 = v3 关注点 |
| 用于规则的 YAML 配置文件 | 5 个硬编码的健康检查阈值不需要配置系统。YAML 加载器增加了依赖项和复杂性的来源 |
| SQLite/boltdb 用于目录 | 零依赖约束是有价值的。JSONL 最多可扩展至数千次运行 |
| 用于 trace 轮转的日志轮转库 | `os.Rename` + 截断。15 行代码 |

### 4.3 何时自建 vs 采购

对于方向一和五，**自建**显然是正确的选择——每个组件都是 15-50 行且零依赖。

对于方向三，有两个选项：
- **自建**：agent frontmatter 驱动路由 + gate 命名约定注册。大约 250 行 Go，零依赖，完全 ForgeOS 原生。
- **采购/采用**：嵌入一个类似 WASM 的插件运行时以实现真正的隔离。极端的过度设计——ForgeOS 在第一次插件发布时需要的托管数量在 10 左右，而不是 1000 个。

对于方向二，**自建**是唯一的选项——不存在为这种特定用例设计的"会话 ID 注入器"库。

---

## 5. 实施路线图

### 5.1 优先级与依赖关系

实际的依赖图如下所示：

```
              ┌──────────┐
              │ 方向一 ①  │  ← 基础事件消费基础设施
              │ (适配器)   │
              └────┬─────┘
                   │
         ┌─────────┼─────────┐
         ▼         ▼         ▼
     ┌──────┐ ┌──────┐ ┌──────┐
     │方向五⑤│ │方向四④│ │方向二②│
     │(5条   │ │(错误  │ │(会话  │
     │ 规则)  │ │ 目录)  │ │ ID +  │
     └──────┘ └──────┘ │ 管线)  │
                        └──────┘
                              │
                              ▼
                         ┌──────┐
                         │方向三③│
                         │(插件)  │
                         └──────┘
```

- 方向① 是方向⑤、④和②的基础（因为它们需要事件流）
- 方向⑤ 仅依赖方向①，是方向④的累积依赖（错误需要聚合，聚合需要事件流）
- 方向② 依赖方向① + ④（会话需要事件 + 故障记录）
- 方向③ 从架构上依赖所有其他方向（插件需要核心的执行引擎，而核心的执行引擎需要正确的观测通道）

### 5.2 分阶段里程碑

| 阶段 | Sprint | 交付物 | LOC 估计 | 风险 |
|-------|--------|---------|-----------|------|
| **P0 基础** | N | **方向① 最低可行**：UNIX socket 多路复用器 ∈ 诊断事件 → $TUI | `+80 Go, +50 TUI` | 低——`io.Writer` 模式意味着零侵入现有 trace 路径 |
| **P0 基础** | N | **方向⑤ 最低可行**：5 条阈值规则，每迭代 `healthCheck()`，trace 种类 `KindSystemHealth` | `+100 Go` | 低——健康检查是迭代函数的简单组合 |
| **P1 诊断** | N+1 | **方向④ 错误目录**：错误代码 + 严重性 + 组件扩展到 `ExecError`， `faults.jsonl` 编写器，TUI 错误面板 | `+120 Go` | 中——将严重性映射附加到现有的 `ExecKind` 会有一些需要调整的边缘情况 |
| **P1 会话** | N+1 | **方向② 会话 ID**：`SessionID` 注入 → trace 事件，`RunID` → 运行目录，阶段 `forge history --json` | `+200 Go` | 中——格式变更（`Event.SessionID` 使用 `omitempty`）保持了向后兼容性，但需要新的读取器了解该字段 |
| **P2 平台** | N+2 | **方向③ frontmatter 路由**：agent card YAML frontmatter → `routing.go` 共识，移除硬编码映射 | `+250 Go` | 中——迁移需要并排运行旧映射和新 frontmatter 至少一个 sprint |
| **P2 平台** | N+2 | **方向③ 门注册**：`harness/gates/*.mjs` 约定进入 `gate.Registry` | `+50 Go, +50 JS` | 低——现有 `harness/` 结构已经遵循了类似约定 |

### 5.3 风险与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|----------|--------|--------------|
| 方向① socket 协议与 TUI 开发者期望不匹配 | 中 | 中 | 在 sprint 0 进行协议原型设计（1 天）——一个简单的 JSON 每行协议允许双方迭代，无需耦合发布 |
| 方向② 会话 ID 造成 trace 碎片 | 低 | 高 | 使用 UUID v7（按时间排序）——使得顺序接近 Seq 顺序，即使 ID 不同。旧事件没有 SessionID，被回退为"跟踪事件，但未分配到会话" |
| 方向③ frontmatter 加载引入 YAML 解析器 | 中 | 低 | 使用现有的 Go 原生解析器进行 YAML→json shim（已经存在于工具链中），或为 agent frontmatter 采用 JSON 加 `//` 行注释作为过渡 |
| 方向④ 错误代码被断言为公共 API | 中 | 中 | 前缀错误代码（`E_*`）——这使得它们明显是公共/稳定的。如果它们被广泛采用，就不能轻易重新编号。从 `EG_`（内部）开始，经过一个 sprint 后桥接到公共代码 |
| 方向⑤ 健康检查因误报导致运行中止 | 低 | 高 | 健康检查从 P0 开始始终是 WARN-only 的。FATAL 阈值是可选的 v1.1 升级。第一个实际运行的中止来自磁盘满，这是现有行为（`os.Write` 失败冒泡） |

### 5.4 关于交叉验证文档中建议的 sprint 顺序的说明

文档的依赖修正（⑤→①④→①+⑤④→①+④方向②→全依赖方向③）比原始文档的平坦分组更准确。我的 Sprint 顺序与其不同之处在于：

1. **我不建议方向① 和方向⑤ 完全并行进行**。方向⑤ 的健康检查以 `trace.Emit()` 结束——它们是方向① 的消费者。你应该先构建适配器（方向① 的最低可行产品），然后插入健康检查（方向⑤）。可以在一个 sprint 中完成，但方向① 先于方向⑤。

2. **我提议方向② 使用 `SessionID` 扩展，在方向④ 的错误目录之后进行**。原因是：跨会话价值取决于标记的故障（方向④ 的输出）——如果你想展示"跨会话的故障趋势"，你需要方向④ 先就位以生成结构化的故障记录。纯粹的最小可行产品 `SessionID`（仅用于运行识别）可以在方向④ 之前完成，但你为了跨会话的故障趋势而要等方向④。

3. **我同意方向③ 是最晚的**——但原因略有不同。这不仅仅是因为"全依赖"——更是因为你需要一个稳定、经过验证的模型，以了解插件需要插入哪些扩展点，这需要先运行方向①-②-④-⑤ 的原型，以了解真正的扩展点在哪里。

### 5.5 每个方向的关键架构决策总结

| 方向 | 架构决策 | 更优替代方案 | 为什么选择它 |
|-----------|---------------------|-------------------------|----------------|
| ① | UNIX socket JSONL 扇出 | gRPC 流、WebSocket | 零新增依赖。JSONL 字节兼容意味着主路径和 socket 路径使用相同的编码 |
| ② | `.forge/artifacts/catalog.jsonl` | SQLite、boltdb | 保持零外部依赖。可扩展至数百次运行。 |
| ③ | YAML frontmatter 作为 agent 的运行时属性 | Go 插件、WASM | 声明式——在执行引擎中不需要新的代码加载。Frontmatter 只在启动时读取 |
| ④ | 错误目录作为独立文件（`faults.jsonl`） | 嵌入在 trace.jsonl 中 | 单独聚合。无需 grep 遍历所有 trace 事件即可找到运行级别的错误摘要 |
| ⑤ | 每迭代健康检查 | 运行在独立进程中的分离式健康代理 | 简单——无需 IPC，无需新的进程生命周期管理。与引擎的迭代节奏自然耦合 |
